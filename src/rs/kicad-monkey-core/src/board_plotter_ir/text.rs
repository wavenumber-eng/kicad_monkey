//! Board gr_text/gr_text_box record emission with authored render caches.
//!
//! Mirrors the Python `gr_text_to_record`/`gr_text_box_to_record` subset:
//! effects/justify decoding with board CENTER defaults, `${NAME}` text
//! variables (project sidecar overlaid by board properties), authored
//! `render_cache` validation and attachment, and gr_text knockout
//! restructuring. Python-generated font-face caches, Shapely synthetic
//! text-box knockout, and stroke-font text-box wrapping are deferred:
//! the native path deterministically emits the unwrapped resolved text
//! and leaves ops without cache keys where Python would generate one.

use super::{BoardPlotRecord, BudgetTracker, point_limit_error};
use crate::pcb::{PcbGraphic, PcbGraphicKind, PcbPoint, PcbProperty, PcbView};
use crate::plotter_ir::{child, mm_to_nm, model_error, numeric_at, value_at};
use crate::plotter_types::{PlotterFill, PlotterOperation, PlotterRect};
use crate::sexpr::{Error, Position, Sexp, parse};
use std::collections::BTreeMap;

/// Python `FRONT_SILKSCREEN_LAYER` default carried by gr_text/gr_text_box.
const FRONT_SILKSCREEN_LAYER: &str = "F.SilkS";
/// Python `Font` size default in mm.
const DEFAULT_TEXT_SIZE_MM: f64 = 1.27;
/// Python text-box border default width: `mm_to_nm(0.2)`.
const TEXT_BOX_BORDER_DEFAULT_WIDTH_NM: i64 = 200_000;
/// Python `DEFAULT_MIN_PLOT_PEN_WIDTH_NM` floor for nonzero border widths.
const MIN_PLOT_PEN_WIDTH_NM: i64 = 84_700;

/// Python `KiCadHorizAlign` subset emitted for board text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardTextHAlign {
    Left,
    Center,
    Right,
}

impl BoardTextHAlign {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "GR_TEXT_H_ALIGN_LEFT",
            Self::Center => "GR_TEXT_H_ALIGN_CENTER",
            Self::Right => "GR_TEXT_H_ALIGN_RIGHT",
        }
    }
}

/// Python `KiCadVertAlign` subset emitted for board text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardTextVAlign {
    Top,
    Center,
    Bottom,
}

impl BoardTextVAlign {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "GR_TEXT_V_ALIGN_TOP",
            Self::Center => "GR_TEXT_V_ALIGN_CENTER",
            Self::Bottom => "GR_TEXT_V_ALIGN_BOTTOM",
        }
    }
}

/// Attached render cache facts; the serialized `source` is always the
/// Python `existing_file_cache` literal because generated caches are
/// deferred with the outline-font bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTextRenderCache {
    pub text: String,
    pub angle: f64,
    pub exact: bool,
    pub knockout: bool,
    /// polygons -> contours -> points; the first contour is the exterior.
    pub polygons: Vec<Vec<Vec<[i64; 2]>>>,
}

/// One `Text` operation payload; the serialized `color` is always the
/// Python `#000000` default because PCB fonts carry no color attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTextOperation {
    pub x: i64,
    pub y: i64,
    pub text: String,
    pub orient_deg: f64,
    pub size_x_nm: i64,
    pub size_y_nm: i64,
    pub h_align: BoardTextHAlign,
    pub v_align: BoardTextVAlign,
    pub pen_width_nm: i64,
    pub italic: bool,
    pub bold: bool,
    pub multiline: bool,
    pub font_face: String,
    /// Python marker keys serialize only when true.
    pub mirror: bool,
    pub text_as_polygons: bool,
    pub polyline_per_segment: bool,
    pub knockout: bool,
    /// Exterior contours; empty means the key is absent.
    pub render_cache_polygons: Vec<Vec<[i64; 2]>>,
    pub render_cache: Option<BoardTextRenderCache>,
}

/// One `gr_text` record; Python board text never hides, so records carry
/// a constant `hide: false` extra at serialization time.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTextRecord {
    pub uuid: String,
    pub layer: String,
    pub text: String,
    pub operations: Vec<BoardTextOperation>,
}

/// One `gr_text_box` operation in Python emission order: the optional
/// border rect precedes the optional text payload.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardTextBoxOperation {
    Border(PlotterOperation),
    Text(BoardTextOperation),
}

/// One `gr_text_box` record with its border extra.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTextBoxRecord {
    pub uuid: String,
    pub layer: String,
    pub text: String,
    pub border: bool,
    pub operations: Vec<BoardTextBoxOperation>,
}

/// Case-expanded `${NAME}` variables mirroring Python
/// `kicad_text_variables.normalize_text_variables`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BoardTextVariables {
    by_name: BTreeMap<String, String>,
}

impl BoardTextVariables {
    /// Python `_add_variable` per entry: the exact, lowercase, and
    /// uppercase keys are written in that order, empty names are skipped,
    /// and later entries overwrite earlier ones key-by-key.
    pub fn from_entries<N, V>(entries: impl IntoIterator<Item = (N, V)>) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        let mut variables = Self::default();
        for (name, value) in entries {
            variables.insert(&name.into(), &value.into());
        }
        variables
    }

    fn insert(&mut self, name: &str, value: &str) {
        if name.is_empty() {
            return;
        }
        self.by_name.insert(name.to_owned(), value.to_owned());
        self.by_name.insert(name.to_lowercase(), value.to_owned());
        self.by_name.insert(name.to_uppercase(), value.to_owned());
    }

    /// Python `board_text_variables`: board `(property ...)` entries
    /// overlay the project sidecar variables.
    pub(super) fn with_board_properties<'a>(
        &self,
        properties: impl IntoIterator<Item = &'a PcbProperty>,
    ) -> Self {
        let mut variables = self.clone();
        for property in properties {
            variables.insert(&property.name, &property.value);
        }
        variables
    }

    /// Python `substitute_text_variables`: regex `\$\{([^}]+)\}` with
    /// unresolved placeholders kept verbatim.
    pub fn substitute(&self, text: &str) -> String {
        if !text.contains("${") {
            return text.to_owned();
        }
        let mut result = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("${") {
            let after = &rest[start + 2..];
            // The name group requires at least one non-`}` character, so an
            // empty or unterminated placeholder never matches; the regex
            // scan then resumes after the failed `$`.
            match after.find('}') {
                Some(end) if end > 0 => {
                    let name = &after[..end];
                    result.push_str(&rest[..start]);
                    match self.by_name.get(name) {
                        Some(value) => result.push_str(value),
                        None => {
                            result.push_str("${");
                            result.push_str(name);
                            result.push('}');
                        }
                    }
                    rest = &after[end..];
                    rest = &rest[1..];
                }
                _ => {
                    result.push_str(&rest[..=start]);
                    rest = &rest[start + 1..];
                }
            }
        }
        result.push_str(rest);
        result
    }
}

pub(super) fn board_variables(
    view: &PcbView<'_>,
    project_variables: &BoardTextVariables,
) -> Result<BoardTextVariables, Error> {
    let properties = view.properties().collect::<Result<Vec<_>, Error>>()?;
    Ok(project_variables.with_board_properties(&properties))
}

pub(super) fn text_records(
    source: &str,
    view: &PcbView<'_>,
    budget: &mut BudgetTracker,
    variables: &BoardTextVariables,
) -> Result<Vec<BoardPlotRecord>, Error> {
    let mut texts = Vec::new();
    let mut text_boxes = Vec::new();
    for graphic in view.graphics() {
        let graphic = graphic?;
        match graphic.kind {
            PcbGraphicKind::Text => texts.push(graphic),
            PcbGraphicKind::TextBox => text_boxes.push(graphic),
            _ => {}
        }
    }
    let mut records = Vec::new();
    for graphic in texts {
        let record = text_record(source, &graphic, variables, budget.remaining_points()?)?;
        budget.charge(
            record.operations.len(),
            text_point_total(&record.operations),
        )?;
        records.push(BoardPlotRecord::Text(record));
    }
    for graphic in text_boxes {
        let record = text_box_record(source, &graphic, variables, budget.remaining_points()?)?;
        budget.charge(
            record.operations.len(),
            text_box_point_total(&record.operations),
        )?;
        records.push(BoardPlotRecord::TextBox(record));
    }
    Ok(records)
}

fn text_point_total(operations: &[BoardTextOperation]) -> usize {
    operations
        .iter()
        .filter_map(|operation| operation.render_cache.as_ref())
        .flat_map(|cache| cache.polygons.iter())
        .flat_map(|contours| contours.iter())
        .fold(0, |total, contour| total.saturating_add(contour.len()))
}

fn text_box_point_total(operations: &[BoardTextBoxOperation]) -> usize {
    operations
        .iter()
        .filter_map(|operation| match operation {
            BoardTextBoxOperation::Text(value) => Some(value),
            BoardTextBoxOperation::Border(_) => None,
        })
        .filter_map(|operation| operation.render_cache.as_ref())
        .flat_map(|cache| cache.polygons.iter())
        .flat_map(|contours| contours.iter())
        .fold(0, |total, contour| total.saturating_add(contour.len()))
}

/// Python `Effects`/`Font` facts consumed by the board text producers.
struct TextEffects {
    face: Option<String>,
    size_x: f64,
    size_y: f64,
    thickness: Option<f64>,
    bold: bool,
    italic: bool,
    justify: Vec<String>,
}

impl TextEffects {
    /// Python `Font.effective_thickness` normal/bold auto-thickness rules.
    fn effective_thickness(&self) -> f64 {
        if let Some(thickness) = self.thickness {
            return thickness;
        }
        let text_width = if self.size_x.abs() != 0.0 {
            self.size_x.abs()
        } else {
            self.size_y.abs()
        };
        if text_width == 0.0 {
            return 0.15;
        }
        let mut pen_width = if self.bold {
            text_width / 5.0
        } else {
            text_width / 8.0
        };
        let min_size = self.size_x.abs().min(self.size_y.abs());
        if min_size != 0.0 {
            pen_width = pen_width.min(min_size * 0.25);
        }
        pen_width
    }

    /// Python `GrText.get_knockout_margin` in mm.
    fn knockout_margin_mm(&self) -> f64 {
        (self.effective_thickness() / 2.0).max(self.size_y / 9.0)
    }
}

/// Authored `(render_cache "text" angle (polygon (pts ...) ...) ...)` facts
/// in raw mm coordinates.
struct AuthoredRenderCache {
    /// `None` marks a non-string cache text token, which can never match a
    /// resolved request text.
    text: Option<String>,
    angle: f64,
    polygons: Vec<Vec<Vec<[f64; 2]>>>,
}

fn list_values(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text_value(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn children<'a>(form: &'a Sexp, head: &'a str) -> impl Iterator<Item = &'a Sexp> + 'a {
    list_values(form)
        .into_iter()
        .flatten()
        .filter(move |value| {
            list_values(value)
                .and_then(|values| values.first())
                .and_then(text_value)
                == Some(head)
        })
}

/// Python `has_flag`: a bare token among the form's direct values.
fn has_flag(form: &Sexp, name: &str) -> bool {
    list_values(form)
        .into_iter()
        .flatten()
        .any(|value| text_value(value) == Some(name))
}

/// Python `parse_maybe_absent_bool`: bare token or empty form mean true,
/// otherwise only an explicit `yes` value is true.
fn maybe_absent_bool(form: &Sexp, name: &str) -> Option<bool> {
    if has_flag(form, name) {
        return Some(true);
    }
    let element = child(form, name)?;
    if list_values(element).is_none_or(|values| values.len() <= 1) {
        return Some(true);
    }
    Some(value_at(element, 1) == Some("yes"))
}

/// Numeric slot with a Python-style per-index default when absent.
fn numeric_or(form: Option<&Sexp>, index: usize, default: f64) -> Result<f64, Error> {
    match form {
        Some(value) if list_values(value).is_some_and(|values| values.len() > index) => {
            numeric_at(value, index, Position::START)
        }
        _ => Ok(default),
    }
}

fn parse_graphic_span(source: &str, graphic: &PcbGraphic) -> Result<Sexp, Error> {
    let text = source
        .get(graphic.source_range.clone())
        .ok_or_else(|| model_error("Board text span is out of range", Position::START))?;
    parse(text)
}

/// Python `Effects.from_sexp`/`Font.from_sexp` over one text carrier form.
fn text_effects(form: &Sexp) -> Result<TextEffects, Error> {
    let effects = child(form, "effects");
    let font = effects.and_then(|value| child(value, "font"));
    let size = font.and_then(|value| child(value, "size"));
    // KiCad serializes `(size height width)`.
    let size_y = numeric_or(size, 1, DEFAULT_TEXT_SIZE_MM)?;
    let size_x = numeric_or(size, 2, DEFAULT_TEXT_SIZE_MM)?;
    let thickness = match font.and_then(|value| child(value, "thickness")) {
        Some(value) if list_values(value).is_some_and(|values| values.len() > 1) => {
            Some(numeric_at(value, 1, Position::START)?)
        }
        _ => None,
    };
    let flag_or_yes = |name: &str| {
        font.is_some_and(|value| {
            has_flag(value, name)
                || child(value, name).and_then(|child| value_at(child, 1)) == Some("yes")
        })
    };
    // Python treats an empty face string as no face.
    let face = font
        .and_then(|value| child(value, "face"))
        .and_then(|value| value_at(value, 1))
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let justify = effects
        .and_then(|value| child(value, "justify"))
        .and_then(list_values)
        .map(|values| {
            values[1..]
                .iter()
                .filter_map(text_value)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(TextEffects {
        face,
        size_x,
        size_y,
        thickness,
        bold: flag_or_yes("bold"),
        italic: flag_or_yes("italic"),
        justify,
    })
}

/// Python `_effects_to_text_kwargs` justify loop: `center` binds to the
/// horizontal axis first, and the last token per axis wins.
fn alignments(justify: &[String]) -> (Option<BoardTextHAlign>, Option<BoardTextVAlign>) {
    let mut h_align = None;
    let mut v_align = None;
    for token in justify {
        match token.as_str() {
            "left" => h_align = Some(BoardTextHAlign::Left),
            "right" => h_align = Some(BoardTextHAlign::Right),
            "center" => h_align = Some(BoardTextHAlign::Center),
            "top" => v_align = Some(BoardTextVAlign::Top),
            "bottom" => v_align = Some(BoardTextVAlign::Bottom),
            _ => {}
        }
    }
    (h_align, v_align)
}

/// Python `RenderCache.from_sexp`: `None` below three header values.
fn parse_render_cache(
    form: &Sexp,
    max_points: usize,
) -> Result<Option<AuthoredRenderCache>, Error> {
    let Some(cache) = child(form, "render_cache") else {
        return Ok(None);
    };
    let Some(values) = list_values(cache).filter(|values| values.len() >= 3) else {
        return Ok(None);
    };
    let text = match &values[1] {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value.clone()),
        // Python `unquote_string` stringifies scalar tokens.
        Sexp::Integer(value) => Some(value.to_string()),
        _ => None,
    };
    let angle = numeric_at(cache, 2, Position::START)?;
    let mut polygons = Vec::new();
    let mut point_count = 0_usize;
    for polygon in children(cache, "polygon") {
        let mut contours = Vec::new();
        for points in children(polygon, "pts") {
            let mut contour = Vec::new();
            for point in children(points, "xy") {
                // Python skips xy forms without both coordinates.
                if list_values(point).is_some_and(|values| values.len() >= 3) {
                    point_count = point_count
                        .checked_add(1)
                        .filter(|count| *count <= max_points)
                        .ok_or_else(point_limit_error)?;
                    contour.push([
                        numeric_at(point, 1, Position::START)?,
                        numeric_at(point, 2, Position::START)?,
                    ]);
                }
            }
            contours.push(contour);
        }
        polygons.push(contours);
    }
    Ok(Some(AuthoredRenderCache {
        text,
        angle,
        polygons,
    }))
}

/// Python `math.isclose` with the default 1e-9 relative and absolute
/// tolerances used by `RenderCacheRequest`.
fn python_isclose(left: f64, right: f64) -> bool {
    let tolerance = (1e-9 * left.abs().max(right.abs())).max(1e-9);
    (left - right).abs() <= tolerance
}

/// Python `RenderCacheResolver.validate_cache` reasons for board requests,
/// which always carry an angle and never a mirror/offset context.
fn cache_is_valid(cache: &AuthoredRenderCache, request_text: &str, request_angle: f64) -> bool {
    cache.text.as_deref() == Some(request_text)
        && python_isclose(cache.angle, request_angle)
        // Python: empty polygons invalidate the cache for nonempty text.
        && (request_text.is_empty() || !cache.polygons.is_empty())
        && cache.polygons.iter().all(|contours| {
            contours.first().is_some_and(|exterior| exterior.len() >= 3)
                && contours.iter().skip(1).all(|hole| hole.len() >= 3)
        })
}

/// Python `_render_cache_polygons_nm` + `_op_with_render_cache_payload`
/// for a valid authored cache; `exact` is false only under the font-face
/// warning because board requests always provide an angle.
fn attach_authored_cache(
    operation: &mut BoardTextOperation,
    cache: &AuthoredRenderCache,
    exact: bool,
) -> Result<(), Error> {
    let mut typed = Vec::new();
    let mut exteriors = Vec::new();
    for polygon in &cache.polygons {
        let mut contours = Vec::new();
        for contour in polygon {
            if contour.len() < 3 {
                continue;
            }
            let points = contour
                .iter()
                .map(|[x, y]| Ok([mm_to_nm(*x)?, mm_to_nm(*y)?]))
                .collect::<Result<Vec<_>, Error>>()?;
            contours.push(points);
        }
        if contours.is_empty() {
            continue;
        }
        exteriors.push(contours[0].clone());
        typed.push(contours);
    }
    if typed.is_empty() {
        return Ok(());
    }
    operation.render_cache_polygons = exteriors;
    operation.render_cache = Some(BoardTextRenderCache {
        text: cache.text.clone().unwrap_or_default(),
        angle: cache.angle,
        exact,
        knockout: false,
        polygons: typed,
    });
    Ok(())
}

/// Python `_apply_knockout_to_text_op`: coalesce every glyph contour under
/// one polygon whose first contour is the margin-inflated background rect.
fn apply_knockout(operation: &mut BoardTextOperation, margin_nm: i64) {
    let Some(cache) = operation.render_cache.as_mut() else {
        return;
    };
    if cache.polygons.is_empty() {
        return;
    }
    let mut glyph_contours: Vec<Vec<[i64; 2]>> = Vec::new();
    let mut bounds: Option<[i64; 4]> = None;
    for polygon in &cache.polygons {
        for contour in polygon {
            if contour.len() < 3 {
                continue;
            }
            for point in contour {
                bounds = Some(match bounds {
                    None => [point[0], point[1], point[0], point[1]],
                    Some([min_x, min_y, max_x, max_y]) => [
                        min_x.min(point[0]),
                        min_y.min(point[1]),
                        max_x.max(point[0]),
                        max_y.max(point[1]),
                    ],
                });
            }
            glyph_contours.push(contour.clone());
        }
    }
    let Some([min_x, min_y, max_x, max_y]) = bounds else {
        return;
    };
    let background = vec![
        [min_x - margin_nm, min_y - margin_nm],
        [max_x + margin_nm, min_y - margin_nm],
        [max_x + margin_nm, max_y + margin_nm],
        [min_x - margin_nm, max_y + margin_nm],
    ];
    let mut contours = vec![background.clone()];
    contours.extend(glyph_contours);
    cache.polygons = vec![contours];
    cache.knockout = true;
    operation.knockout = true;
    operation.render_cache_polygons = vec![background];
}

/// Python `gr_text_to_record`.
fn text_record(
    source: &str,
    graphic: &PcbGraphic,
    variables: &BoardTextVariables,
    max_cache_points: usize,
) -> Result<BoardTextRecord, Error> {
    let form = parse_graphic_span(source, graphic)?;
    let effects = text_effects(&form)?;
    let angle = numeric_or(child(&form, "at"), 3, 0.0)?;
    let cache = parse_render_cache(&form, max_cache_points)?;
    let knockout = child(&form, "layer").is_some_and(|value| has_flag(value, "knockout"));
    let layer = graphic
        .layer
        .clone()
        .unwrap_or_else(|| FRONT_SILKSCREEN_LAYER.to_owned());
    let uuid = graphic.uuid.clone().unwrap_or_default();
    let raw_text = graphic.text.clone().unwrap_or_default();
    // Python `gr_text_to_op` skips empty text before building the op.
    if raw_text.is_empty() {
        return Ok(BoardTextRecord {
            uuid,
            layer,
            text: raw_text,
            operations: Vec::new(),
        });
    }
    let at = graphic.at.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    let resolved = variables.substitute(&raw_text);
    let face_present = effects.face.is_some();
    let (h_align, v_align) = alignments(&effects.justify);
    let mut operation = BoardTextOperation {
        x: mm_to_nm(at.x)?,
        y: mm_to_nm(at.y)?,
        text: resolved.clone(),
        orient_deg: angle,
        size_x_nm: mm_to_nm(effects.size_x)?,
        size_y_nm: mm_to_nm(effects.size_y)?,
        h_align: h_align.unwrap_or(BoardTextHAlign::Center),
        v_align: v_align.unwrap_or(BoardTextVAlign::Center),
        pen_width_nm: match effects.thickness {
            Some(thickness) => mm_to_nm(thickness)?,
            None => 0,
        },
        italic: effects.italic,
        bold: effects.bold,
        // Python `gr_text_to_op` never passes the multiline kwarg.
        multiline: false,
        font_face: effects.face.clone().unwrap_or_default(),
        mirror: effects.justify.iter().any(|token| token == "mirror"),
        text_as_polygons: !face_present,
        polyline_per_segment: !face_present,
        knockout: false,
        render_cache_polygons: Vec::new(),
        render_cache: None,
    };
    if cache.is_some() || face_present {
        // The gr_text request text equals the resolved text: GrText has no
        // `render_cache_text` wrapping hook.
        if let Some(cache) = cache
            .as_ref()
            .filter(|value| cache_is_valid(value, &resolved, angle))
        {
            attach_authored_cache(&mut operation, cache, !face_present)?;
        }
        // Missing or stale caches with a font face take the Python
        // generation path, deferred with the outline-font bridge; the op
        // then carries no cache keys.
    }
    if knockout {
        let margin_nm = mm_to_nm(effects.knockout_margin_mm())?;
        apply_knockout(&mut operation, margin_nm);
    }
    let text = operation.text.clone();
    Ok(BoardTextRecord {
        uuid,
        layer,
        text,
        operations: vec![operation],
    })
}

/// Python text-box border width: `stroke_width_nm(stroke or Stroke(), 0.2mm)`.
fn border_width_nm(stroke_width: Option<f64>) -> Result<i64, Error> {
    let width = stroke_width.unwrap_or(0.0);
    if width < 0.0 {
        return Ok(0);
    }
    if width == 0.0 {
        return Ok(TEXT_BOX_BORDER_DEFAULT_WIDTH_NM);
    }
    Ok(mm_to_nm(width)?.max(MIN_PLOT_PEN_WIDTH_NM))
}

/// Python `StrokeType(...)` accepts only the known KiCad stroke names.
fn validate_stroke_type(form: &Sexp) -> Result<(), Error> {
    let Some(stroke) = child(form, "stroke") else {
        return Ok(());
    };
    let style = child(stroke, "type")
        .and_then(|value| value_at(value, 1))
        .unwrap_or("default");
    match style {
        "default" | "solid" | "dash" | "dot" | "dash_dot" | "dash_dot_dot" => Ok(()),
        _ => Err(model_error(
            "Unsupported board text-box stroke type",
            Position::START,
        )),
    }
}

/// Python `GrTextBox.from_sexp` start/end fallback: the corner-point
/// bounding box applies only when the start/end pair is incomplete.
fn text_box_corners(graphic: &PcbGraphic) -> (f64, f64, f64, f64) {
    let pair_complete = graphic.start.is_some() && graphic.end.is_some();
    if !graphic.points.is_empty() && !pair_complete {
        let xs = graphic.points.iter().map(|point| point.x);
        let ys = graphic.points.iter().map(|point| point.y);
        (
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        )
    } else {
        let start = graphic.start.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
        let end = graphic.end.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
        (start.x, start.y, end.x, end.y)
    }
}

/// Python `fp_text_box_to_ops` text placement over the normalized box with
/// per-side margins applied by the effective alignment.
fn text_box_text_operation(
    effects: &TextEffects,
    cache: Option<&AuthoredRenderCache>,
    angle: f64,
    corners: (f64, f64, f64, f64),
    margins: [f64; 4],
    resolved: &str,
) -> Result<BoardTextOperation, Error> {
    let face_present = effects.face.is_some();
    let (h_align, v_align) = alignments(&effects.justify);
    let h_align = h_align.unwrap_or(BoardTextHAlign::Center);
    let v_align = v_align.unwrap_or(BoardTextVAlign::Center);
    let (start_x, start_y, end_x, end_y) = corners;
    let x1 = start_x.min(end_x);
    let y1 = start_y.min(end_y);
    let x2 = start_x.max(end_x);
    let y2 = start_y.max(end_y);
    let [margin_left, margin_top, margin_right, margin_bottom] = margins;
    let x = match h_align {
        BoardTextHAlign::Right => x2 - margin_right,
        BoardTextHAlign::Center => (x1 + x2) / 2.0,
        BoardTextHAlign::Left => x1 + margin_left,
    };
    let y = match v_align {
        BoardTextVAlign::Bottom => y2 - margin_bottom,
        BoardTextVAlign::Center => (y1 + y2) / 2.0,
        BoardTextVAlign::Top => y1 + margin_top,
    };
    // Stroke-font wrapping is deferred: the native path always emits
    // the unwrapped resolved text, matching Python whenever the box
    // text has no space or no positive wrap width.
    let mut operation = BoardTextOperation {
        x: mm_to_nm(x)?,
        y: mm_to_nm(y)?,
        text: resolved.to_owned(),
        orient_deg: angle,
        size_x_nm: mm_to_nm(effects.size_x)?,
        size_y_nm: mm_to_nm(effects.size_y)?,
        h_align,
        v_align,
        pen_width_nm: match effects.thickness {
            Some(thickness) => mm_to_nm(thickness)?,
            None => 0,
        },
        italic: effects.italic,
        bold: effects.bold,
        multiline: resolved.contains('\n'),
        font_face: effects.face.clone().unwrap_or_default(),
        // Text boxes never emit the mirror or per-segment markers.
        mirror: false,
        text_as_polygons: !face_present,
        polyline_per_segment: false,
        knockout: false,
        render_cache_polygons: Vec::new(),
        render_cache: None,
    };
    // Python's Shapely synthetic knockout is deferred: without Shapely
    // the oracle leaves the op unchanged, which is the pinned baseline.
    // The request text is the resolved text: FreeType `render_cache_text`
    // line wrapping is deferred.
    if let Some(cache) = cache.filter(|value| cache_is_valid(value, resolved, angle)) {
        attach_authored_cache(&mut operation, cache, !face_present)?;
    }
    Ok(operation)
}

/// Python `gr_text_box_to_record`.
fn text_box_record(
    source: &str,
    graphic: &PcbGraphic,
    variables: &BoardTextVariables,
    max_cache_points: usize,
) -> Result<BoardTextBoxRecord, Error> {
    let form = parse_graphic_span(source, graphic)?;
    let effects = text_effects(&form)?;
    let cache = parse_render_cache(&form, max_cache_points)?;
    let angle = numeric_or(child(&form, "angle"), 1, 0.0)?;
    let border = maybe_absent_bool(&form, "border");
    let stroke_width = match child(&form, "stroke").and_then(|value| child(value, "width")) {
        Some(value) => Some(numeric_or(Some(value), 1, 0.0)?),
        None => None,
    };
    validate_stroke_type(&form)?;
    let margins_form = child(&form, "margins");
    let margins = if margins_form
        .and_then(list_values)
        .is_some_and(|values| values.len() >= 5)
    {
        [
            numeric_at(margins_form.unwrap_or(&form), 1, Position::START)?,
            numeric_at(margins_form.unwrap_or(&form), 2, Position::START)?,
            numeric_at(margins_form.unwrap_or(&form), 3, Position::START)?,
            numeric_at(margins_form.unwrap_or(&form), 4, Position::START)?,
        ]
    } else {
        [0.0; 4]
    };
    let corners = text_box_corners(graphic);
    let (start_x, start_y, end_x, end_y) = corners;
    let layer = graphic
        .layer
        .clone()
        .unwrap_or_else(|| FRONT_SILKSCREEN_LAYER.to_owned());
    let uuid = graphic.uuid.clone().unwrap_or_default();
    let raw_text = graphic.text.clone().unwrap_or_default();

    let mut operations = Vec::new();
    if border == Some(true) {
        operations.push(BoardTextBoxOperation::Border(PlotterOperation::Rect(
            PlotterRect {
                x1: mm_to_nm(start_x)?,
                y1: mm_to_nm(start_y)?,
                x2: mm_to_nm(end_x)?,
                y2: mm_to_nm(end_y)?,
                fill: PlotterFill::NoFill,
                width_nm: border_width_nm(stroke_width)?,
                corner_radius_nm: 0,
                layer: None,
                stroke_color: None,
                fill_color: None,
                line_style: None,
            },
        )));
    }
    if !raw_text.is_empty() {
        let resolved = variables.substitute(&raw_text);
        let operation =
            text_box_text_operation(&effects, cache.as_ref(), angle, corners, margins, &resolved)?;
        operations.push(BoardTextBoxOperation::Text(operation));
    }
    let text = operations
        .iter()
        .find_map(|operation| match operation {
            BoardTextBoxOperation::Text(value) => Some(value.text.clone()),
            BoardTextBoxOperation::Border(_) => None,
        })
        .unwrap_or(raw_text);
    Ok(BoardTextBoxRecord {
        uuid,
        layer,
        text,
        border: border == Some(true),
        operations,
    })
}

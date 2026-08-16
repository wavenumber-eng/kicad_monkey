//! Board gr_text/gr_text_box record emission with authored render caches.
//!
//! Mirrors the Python `gr_text_to_record`/`gr_text_box_to_record` subset:
//! effects/justify decoding with board CENTER defaults, `${NAME}` text
//! variables (project sidecar overlaid by board properties), authored
//! `render_cache` validation and attachment, and gr_text knockout
//! restructuring. Python-generated font-face caches, Shapely synthetic
//! text-box knockout is deferred. Ordinary stroke-font text-box wrapping is
//! native; generated outline-font caches remain deferred.

use super::text_cache::{
    AuthoredRenderCache, apply_knockout, attach_authored_cache, cache_is_valid, parse_render_cache,
};
use super::text_variables::BoardTextVariables;
use super::text_wrap::wrap_text_box;
use super::{BoardPlotLimits, BoardPlotRecord, BudgetTracker, text_limit_error};
use crate::pcb::{PcbGraphic, PcbGraphicKind, PcbPoint, PcbView};
use crate::plotter_ir::{child, mm_to_nm, model_error, numeric_at, value_at};
use crate::plotter_types::{PlotterFill, PlotterOperation, PlotterRect};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Limits, Position, Sexp, parse_with_limits};

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

/// One `Text` operation payload. Ordinary `gr_text` remains black; the
/// text-box source model can carry an RGBA font color.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTextOperation {
    pub x: i64,
    pub y: i64,
    pub text: String,
    pub color: String,
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

pub(super) fn board_variables(
    view: &PcbView<'_>,
    graphics: &[PcbGraphic],
    project_variables: &BoardTextVariables,
) -> Result<BoardTextVariables, Error> {
    let needs_variables = graphics.iter().any(|graphic| {
        matches!(graphic.kind, PcbGraphicKind::Text | PcbGraphicKind::TextBox)
            && graphic
                .text
                .as_deref()
                .is_some_and(|text| text.contains("${"))
    });
    if !needs_variables {
        return Ok(project_variables.clone());
    }
    let mut variables = project_variables.clone();
    for property in view.properties() {
        let property = property?;
        variables.insert(&property.name, &property.value);
    }
    Ok(variables)
}

pub(super) fn text_records(
    source: &str,
    graphics: &[PcbGraphic],
    budget: &mut BudgetTracker,
    variables: &BoardTextVariables,
    limits: BoardPlotLimits,
) -> Result<Vec<BoardPlotRecord>, Error> {
    let mut texts = Vec::new();
    let mut text_boxes = Vec::new();
    for graphic in graphics {
        match graphic.kind {
            PcbGraphicKind::Text => texts.push(graphic),
            PcbGraphicKind::TextBox => text_boxes.push(graphic),
            _ => {}
        }
    }
    let mut records = Vec::new();
    for graphic in texts {
        let record = text_record(
            source,
            graphic,
            variables,
            budget.remaining_points()?,
            budget.remaining_text_bytes()?,
            budget.remaining_operations().unwrap_or(0),
            limits,
        )?;
        budget.charge(
            record.operations.len(),
            text_point_total(&record.operations),
        )?;
        budget.charge_text(text_retained_bytes(&record))?;
        records.push(BoardPlotRecord::Text(record));
    }
    for graphic in text_boxes {
        let record = text_box_record(
            source,
            graphic,
            variables,
            budget.remaining_points()?,
            budget.remaining_text_bytes()?,
            budget.remaining_operations().unwrap_or(0),
            limits,
        )?;
        budget.charge(
            record.operations.len(),
            text_box_point_total(&record.operations),
        )?;
        budget.charge_text(text_box_retained_bytes(&record))?;
        records.push(BoardPlotRecord::TextBox(record));
    }
    Ok(records)
}

fn text_point_total(operations: &[BoardTextOperation]) -> usize {
    operations.iter().fold(0, |total, operation| {
        let cache_points = operation
            .render_cache
            .as_ref()
            .into_iter()
            .flat_map(|cache| cache.polygons.iter())
            .flat_map(|contours| contours.iter())
            .fold(0usize, |count, contour| count.saturating_add(contour.len()));
        let exterior_points = operation
            .render_cache_polygons
            .iter()
            .fold(0usize, |count, polygon| count.saturating_add(polygon.len()));
        total
            .saturating_add(cache_points)
            .saturating_add(exterior_points)
    })
}

fn operation_text_bytes(operation: &BoardTextOperation) -> usize {
    operation.text.len().saturating_add(
        operation
            .render_cache
            .as_ref()
            .map_or(0, |cache| cache.text.len()),
    )
}

fn text_retained_bytes(record: &BoardTextRecord) -> usize {
    record
        .operations
        .iter()
        .fold(record.text.len(), |total, operation| {
            total.saturating_add(operation_text_bytes(operation))
        })
}

fn text_box_retained_bytes(record: &BoardTextBoxRecord) -> usize {
    record
        .operations
        .iter()
        .filter_map(|operation| match operation {
            BoardTextBoxOperation::Text(value) => Some(value),
            BoardTextBoxOperation::Border(_) => None,
        })
        .fold(record.text.len(), |total, operation| {
            total.saturating_add(operation_text_bytes(operation))
        })
}

fn text_box_point_total(operations: &[BoardTextBoxOperation]) -> usize {
    operations
        .iter()
        .filter_map(|operation| match operation {
            BoardTextBoxOperation::Text(value) => Some(value),
            BoardTextBoxOperation::Border(_) => None,
        })
        .fold(0, |total, operation| {
            total.saturating_add(text_point_total(std::slice::from_ref(operation)))
        })
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
    color: String,
}

#[derive(Clone, Copy)]
struct TextOperationLimits {
    cache_points: usize,
    text_bytes: usize,
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

fn resource_limit_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}

fn parse_graphic_span(
    source: &str,
    graphic: &PcbGraphic,
    limits: BoardPlotLimits,
) -> Result<Sexp, Error> {
    let text = source
        .get(graphic.source_range.clone())
        .ok_or_else(|| model_error("Board text span is out of range", Position::START))?;
    parse_with_limits(
        text,
        Limits {
            max_source_bytes: text.len(),
            max_depth: limits.max_depth,
            max_nodes: limits.max_parse_nodes,
            max_decoded_string_bytes: limits.max_source_bytes,
        },
    )
}

fn rgba_color(font: Option<&Sexp>) -> Result<String, Error> {
    let Some(color) = font.and_then(|value| child(value, "color")) else {
        return Ok("#000000".to_owned());
    };
    let Some(_) = list_values(color).filter(|values| values.len() >= 5) else {
        return Ok("#000000".to_owned());
    };
    let channel = |index| -> Result<i64, Error> {
        let value = numeric_at(color, index, Position::START)?;
        Ok((value as i64).clamp(0, 255))
    };
    let alpha = numeric_at(color, 4, Position::START)?;
    if alpha <= 0.0 {
        return Ok("#000000".to_owned());
    }
    let alpha = if alpha <= 1.0 {
        (alpha * 255.0).round_ties_even()
    } else {
        alpha.round_ties_even()
    };
    Ok(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        channel(1)?,
        channel(2)?,
        channel(3)?,
        (alpha as i64).clamp(0, 255)
    ))
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
    let color = rgba_color(font)?;
    Ok(TextEffects {
        face,
        size_x,
        size_y,
        thickness,
        bold: flag_or_yes("bold"),
        italic: flag_or_yes("italic"),
        justify,
        color,
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

/// Python `gr_text_to_record`.
fn text_record(
    source: &str,
    graphic: &PcbGraphic,
    variables: &BoardTextVariables,
    max_cache_points: usize,
    max_text_bytes: usize,
    max_operations: usize,
    limits: BoardPlotLimits,
) -> Result<BoardTextRecord, Error> {
    let raw_text = graphic.text.clone().unwrap_or_default();
    if !raw_text.is_empty() && max_operations < 1 {
        return Err(resource_limit_error(
            "Board plotter operation exceeds configured limits",
        ));
    }
    let resolved = if raw_text.is_empty() {
        None
    } else {
        Some(variables.substitute_bounded(&raw_text, max_text_bytes)?)
    };
    if let Some(resolved) = &resolved {
        ensure_retained_text_bytes(resolved.len(), 2, max_text_bytes)?;
    }
    let form = parse_graphic_span(source, graphic, limits)?;
    let effects = text_effects(&form)?;
    let angle = numeric_or(child(&form, "at"), 3, 0.0)?;
    let cache = parse_render_cache(
        &form,
        max_cache_points,
        limits.max_cache_polygons,
        limits.max_cache_contours,
    )?;
    let knockout = child(&form, "layer").is_some_and(|value| has_flag(value, "knockout"));
    let layer = graphic
        .layer
        .clone()
        .unwrap_or_else(|| FRONT_SILKSCREEN_LAYER.to_owned());
    let uuid = graphic.uuid.clone().unwrap_or_default();
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
    let resolved = resolved.expect("nonempty board text was resolved before parsing");
    let face_present = effects.face.is_some();
    let valid_cache = cache
        .as_ref()
        .filter(|value| cache_is_valid(value, &resolved, angle));
    preflight_knockout(
        valid_cache,
        knockout,
        max_cache_points,
        limits.max_cache_contours,
    )?;
    ensure_retained_text_bytes(
        resolved.len(),
        2 + usize::from(valid_cache.is_some()),
        max_text_bytes,
    )?;
    let mut operation = gr_text_operation(&effects, at, resolved, angle)?;
    if cache.is_some() || face_present {
        // The gr_text request text equals the resolved text: GrText has no
        // `render_cache_text` wrapping hook.
        if let Some(cache) = valid_cache {
            attach_authored_cache(
                &mut operation,
                cache,
                !face_present,
                max_cache_points,
                knockout,
            )?;
        }
        // Missing or stale caches with a font face take the Python
        // generation path, deferred with the outline-font bridge; the op
        // then carries no cache keys.
    }
    if knockout {
        let margin_nm = mm_to_nm(effects.knockout_margin_mm())?;
        apply_knockout(
            &mut operation,
            margin_nm,
            max_cache_points,
            limits.max_cache_contours,
        )?;
    }
    let text = operation.text.clone();
    Ok(BoardTextRecord {
        uuid,
        layer,
        text,
        operations: vec![operation],
    })
}

fn ensure_retained_text_bytes(
    text_bytes: usize,
    occurrences: usize,
    max_text_bytes: usize,
) -> Result<(), Error> {
    text_bytes
        .checked_mul(occurrences)
        .filter(|bytes| *bytes <= max_text_bytes)
        .map(|_| ())
        .ok_or_else(text_limit_error)
}

fn preflight_knockout(
    cache: Option<&AuthoredRenderCache>,
    knockout: bool,
    max_points: usize,
    max_contours: usize,
) -> Result<(), Error> {
    if let (true, Some(cache)) = (knockout, cache) {
        cache.ensure_knockout_limits(max_points, max_contours)?;
    }
    Ok(())
}

fn gr_text_operation(
    effects: &TextEffects,
    at: PcbPoint,
    text: String,
    angle: f64,
) -> Result<BoardTextOperation, Error> {
    let face_present = effects.face.is_some();
    let (h_align, v_align) = alignments(&effects.justify);
    Ok(BoardTextOperation {
        x: mm_to_nm(at.x)?,
        y: mm_to_nm(at.y)?,
        text,
        // The board gr_text parser has no font-color field; color is a
        // gr_text_box-only extension in the established Python model.
        color: "#000000".to_owned(),
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

fn text_box_margins(form: &Sexp) -> Result<[f64; 4], Error> {
    let Some(margins) = child(form, "margins") else {
        return Ok([0.0; 4]);
    };
    if list_values(margins).is_none_or(|values| values.len() < 5) {
        return Ok([0.0; 4]);
    }
    Ok([
        numeric_at(margins, 1, Position::START)?,
        numeric_at(margins, 2, Position::START)?,
        numeric_at(margins, 3, Position::START)?,
        numeric_at(margins, 4, Position::START)?,
    ])
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
    limits: TextOperationLimits,
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
    let size_x_nm = mm_to_nm(effects.size_x)?;
    let wrap_size_x_nm = if size_x_nm == 0 { 1_270_000 } else { size_x_nm };
    let wrapped = wrap_text_box(
        resolved,
        ((x2 - x1) - margin_left - margin_right).max(0.0),
        wrap_size_x_nm,
    );
    ensure_cache_request_text_unchanged(cache, resolved)?;
    let multiline = wrapped.contains('\n');
    // Python replaces the ordinary Newstroke payload with the outline-cache
    // request text whenever a cache or face is present. Until the full outline
    // linebreaker lands, authored caches are supported only when that request
    // text is unchanged; changed cache text is rejected above rather than
    // silently accepting or discarding a semantically different cache.
    let operation_text = if cache.is_some() || face_present {
        resolved
    } else {
        &wrapped
    };
    let valid_cache = cache.filter(|value| cache_is_valid(value, operation_text, angle));
    ensure_retained_text_bytes(
        operation_text.len(),
        2 + usize::from(valid_cache.is_some()),
        limits.text_bytes,
    )?;
    let operation_text = operation_text.to_owned();
    let mut operation = BoardTextOperation {
        x: mm_to_nm(x)?,
        y: mm_to_nm(y)?,
        text: operation_text.clone(),
        color: effects.color.clone(),
        orient_deg: angle,
        size_x_nm,
        size_y_nm: mm_to_nm(effects.size_y)?,
        h_align,
        v_align,
        pen_width_nm: match effects.thickness {
            Some(thickness) => mm_to_nm(thickness)?,
            None => 0,
        },
        italic: effects.italic,
        bold: effects.bold,
        multiline,
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
    if let Some(cache) = valid_cache {
        attach_authored_cache(
            &mut operation,
            cache,
            !face_present,
            limits.cache_points,
            false,
        )?;
    }
    Ok(operation)
}

fn ensure_cache_request_text_unchanged(
    cache: Option<&AuthoredRenderCache>,
    resolved: &str,
) -> Result<(), Error> {
    let Some(cache) = cache else {
        return Ok(());
    };
    if !resolved.contains(' ') && cache.text() == Some(resolved) {
        return Ok(());
    }
    Err(Error::at(
        ErrorPhase::Tree,
        ErrorKind::InvalidBuildValue,
        "Board text-box render-cache wrapping requires the outline-font bridge",
        Position::START,
    ))
}

/// Python `gr_text_box_to_record`.
fn text_box_record(
    source: &str,
    graphic: &PcbGraphic,
    variables: &BoardTextVariables,
    max_cache_points: usize,
    max_text_bytes: usize,
    max_operations: usize,
    limits: BoardPlotLimits,
) -> Result<BoardTextBoxRecord, Error> {
    let raw_text = graphic.text.clone().unwrap_or_default();
    let required_operations =
        usize::from(graphic.border == Some(true)) + usize::from(!raw_text.is_empty());
    if required_operations > max_operations {
        return Err(resource_limit_error(
            "Board plotter operation exceeds configured limits",
        ));
    }
    let resolved = if raw_text.is_empty() {
        None
    } else {
        Some(variables.substitute_bounded(&raw_text, max_text_bytes)?)
    };
    let form = parse_graphic_span(source, graphic, limits)?;
    let effects = text_effects(&form)?;
    let cache = parse_render_cache(
        &form,
        max_cache_points,
        limits.max_cache_polygons,
        limits.max_cache_contours,
    )?;
    let angle = numeric_or(child(&form, "angle"), 1, 0.0)?;
    let border = maybe_absent_bool(&form, "border");
    let stroke_width = match child(&form, "stroke").and_then(|value| child(value, "width")) {
        Some(value) => Some(numeric_or(Some(value), 1, 0.0)?),
        None => None,
    };
    validate_stroke_type(&form)?;
    let margins = text_box_margins(&form)?;
    let corners = text_box_corners(graphic);
    let (start_x, start_y, end_x, end_y) = corners;
    let layer = graphic
        .layer
        .clone()
        .unwrap_or_else(|| FRONT_SILKSCREEN_LAYER.to_owned());
    let uuid = graphic.uuid.clone().unwrap_or_default();
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
        let resolved = resolved.expect("nonempty text box was resolved before parsing");
        let operation = text_box_text_operation(
            &effects,
            cache.as_ref(),
            angle,
            corners,
            margins,
            &resolved,
            TextOperationLimits {
                cache_points: max_cache_points,
                text_bytes: max_text_bytes,
            },
        )?;
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

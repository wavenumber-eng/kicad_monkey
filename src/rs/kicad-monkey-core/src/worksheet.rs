//! Source-backed KiCad worksheet/page-layout model and exact owned writer.

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Limits, Patch, Position, Sexp, TokenKind,
    apply_patches_with_limit, build_with_limit, parse_with_limits, utf8_text,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::worksheet_preflight::preflight_item;
use std::io::{Read, Write};

/// Resource ceilings for one worksheet read or exact write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorksheetLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_items: usize,
    pub max_nodes_per_item: usize,
    pub max_decoded_string_bytes: usize,
    pub max_point_sets_per_polygon: usize,
    pub max_points_per_polygon: usize,
    pub max_justify_tokens: usize,
    pub max_bitmap_data_parts: usize,
    pub max_bitmap_data_bytes: usize,
}

impl Default for WorksheetLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_items: 1_000_000,
            max_nodes_per_item: 4_000_000,
            max_decoded_string_bytes: 64 * 1024 * 1024,
            max_point_sets_per_polygon: 1_000_000,
            max_points_per_polygon: 4_000_000,
            max_justify_tokens: 16,
            max_bitmap_data_parts: 1_000_000,
            max_bitmap_data_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorksheetFormat {
    Modern,
    Legacy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorksheetMetadata {
    pub format: WorksheetFormat,
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorksheetSetup {
    pub text_size_x: f64,
    pub text_size_y: f64,
    pub line_width: f64,
    pub text_line_width: f64,
    pub left_margin: f64,
    pub right_margin: f64,
    pub top_margin: f64,
    pub bottom_margin: f64,
}

impl Default for WorksheetSetup {
    fn default() -> Self {
        Self {
            text_size_x: 1.5,
            text_size_y: 1.5,
            line_width: 0.15,
            text_line_width: 0.15,
            left_margin: 10.0,
            right_margin: 10.0,
            top_margin: 10.0,
            bottom_margin: 10.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorksheetCorner {
    #[default]
    None,
    LeftTop,
    RightTop,
    LeftBottom,
    RightBottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorksheetPoint {
    pub x: f64,
    pub y: f64,
    pub corner: WorksheetCorner,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorksheetRepeat {
    pub count: i64,
    pub increment_x: f64,
    pub increment_y: f64,
    pub increment_label: i64,
}

impl Default for WorksheetRepeat {
    fn default() -> Self {
        Self {
            count: 1,
            increment_x: 0.0,
            increment_y: 0.0,
            increment_label: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorksheetLine {
    pub name: String,
    pub comment: String,
    pub option: String,
    pub start: WorksheetPoint,
    pub end: WorksheetPoint,
    pub line_width: Option<f64>,
    pub repeat: WorksheetRepeat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorksheetRect {
    pub name: String,
    pub comment: String,
    pub option: String,
    pub start: WorksheetPoint,
    pub end: WorksheetPoint,
    pub line_width: Option<f64>,
    pub repeat: WorksheetRepeat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorksheetPolygon {
    pub name: String,
    pub comment: String,
    pub option: String,
    pub position: WorksheetPoint,
    pub rotate: f64,
    pub line_width: Option<f64>,
    pub repeat: WorksheetRepeat,
    pub point_sets: Vec<Vec<(f64, f64)>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorksheetColor {
    pub red: i64,
    pub green: i64,
    pub blue: i64,
    pub alpha: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorksheetFont {
    pub size_x: f64,
    pub size_y: f64,
    pub line_width: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub face: String,
    pub color: Option<WorksheetColor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorksheetText {
    pub text: String,
    pub name: String,
    pub comment: String,
    pub option: String,
    pub position: WorksheetPoint,
    pub font: WorksheetFont,
    pub justify: Vec<String>,
    pub rotate: f64,
    pub repeat: WorksheetRepeat,
    pub max_length: f64,
    pub max_height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorksheetBitmap {
    pub name: String,
    pub comment: String,
    pub option: String,
    pub position: WorksheetPoint,
    pub scale: f64,
    pub repeat: WorksheetRepeat,
    pub data_parts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorksheetItem {
    Line(WorksheetLine),
    Rect(WorksheetRect),
    Polygon(WorksheetPolygon),
    Text(WorksheetText),
    Bitmap(WorksheetBitmap),
}

/// Borrowed, source-ordered worksheet view.
#[derive(Clone, Debug)]
pub struct WorksheetView<'a> {
    source: &'a str,
    root: FormSpan,
    top_level: Vec<FormSpan>,
    limits: WorksheetLimits,
}

impl<'a> WorksheetView<'a> {
    pub fn parse(source: &'a str, limits: WorksheetLimits) -> Result<Self, Error> {
        let selected_limit = limits.max_items.checked_add(16).ok_or_else(limit_error)?;
        let spans = scan_form_spans_with_limits(
            source,
            &Selector {
                min_depth: Some(0),
                max_depth: Some(1),
                ..Selector::default()
            },
            ProjectionLimits {
                max_source_bytes: limits.max_source_bytes,
                max_depth: limits.max_depth,
                max_selected_forms: selected_limit,
                ..ProjectionLimits::default()
            },
        )?;
        let roots = spans
            .iter()
            .filter(|span| span.depth == 0)
            .collect::<Vec<_>>();
        let [root] = roots.as_slice() else {
            return Err(source_error(
                "expected exactly one worksheet root",
                Position::START,
            ));
        };
        if !matches!(root.head.as_deref(), Some("kicad_wks" | "page_layout")) {
            return Err(source_error(
                "expected a kicad_wks or page_layout root",
                root.start,
            ));
        }
        let root = (*root).clone();
        let top_level = spans
            .into_iter()
            .filter(|span| span.depth == 1)
            .collect::<Vec<_>>();
        let item_count = top_level
            .iter()
            .filter(|span| is_item_head(span.head.as_deref()))
            .count();
        if item_count > limits.max_items {
            return Err(limit_error());
        }
        Ok(Self {
            source,
            root,
            top_level,
            limits,
        })
    }

    pub fn metadata(&self) -> Result<WorksheetMetadata, Error> {
        Ok(WorksheetMetadata {
            format: if self.root.head.as_deref() == Some("kicad_wks") {
                WorksheetFormat::Modern
            } else {
                WorksheetFormat::Legacy
            },
            version: self
                .first_scalar("version")?
                .map_or(Ok(0), |value| integer_text(&value))?,
            generator: self.first_scalar("generator")?.unwrap_or_default(),
            generator_version: self.first_scalar("generator_version")?.unwrap_or_default(),
        })
    }

    pub fn setup(&self) -> Result<WorksheetSetup, Error> {
        self.first_form("setup")?
            .as_ref()
            .map_or(Ok(WorksheetSetup::default()), setup)
    }

    pub fn items(&self) -> impl Iterator<Item = Result<WorksheetItem, Error>> + '_ {
        self.top_level
            .iter()
            .filter(|span| is_item_head(span.head.as_deref()))
            .map(|span| self.item(span))
    }

    pub fn item_count(&self) -> usize {
        self.top_level
            .iter()
            .filter(|span| is_item_head(span.head.as_deref()))
            .count()
    }

    fn item(&self, span: &FormSpan) -> Result<WorksheetItem, Error> {
        let value = self.parse_span(span)?;
        match span.head.as_deref() {
            Some("line") => line(value).map(WorksheetItem::Line),
            Some("rect") => rect(value).map(WorksheetItem::Rect),
            Some("polygon") => polygon(value, self.limits).map(WorksheetItem::Polygon),
            Some("tbtext") => text(value, self.limits).map(WorksheetItem::Text),
            Some("bitmap") => bitmap(value, self.limits).map(WorksheetItem::Bitmap),
            _ => Err(source_error("unknown worksheet item", span.start)),
        }
        .map_err(|error| rebase_error(error, span))
    }

    fn first_scalar(&self, head: &str) -> Result<Option<String>, Error> {
        Ok(self
            .first_form(head)?
            .as_ref()
            .and_then(|value| list(value).get(1))
            .map(scalar))
    }

    fn first_form(&self, head: &str) -> Result<Option<Sexp>, Error> {
        let Some(span) = self
            .top_level
            .iter()
            .find(|span| span.head.as_deref() == Some(head))
        else {
            return Ok(None);
        };
        self.parse_span(span).map(Some)
    }

    fn parse_span(&self, span: &FormSpan) -> Result<Sexp, Error> {
        let text = span.text(self.source)?;
        preflight_item(text, self.limits).map_err(|error| rebase_error(error, span))?;
        parse_with_limits(
            text,
            Limits {
                max_source_bytes: self.limits.max_source_bytes,
                max_depth: self.limits.max_depth,
                max_nodes: self.limits.max_nodes_per_item,
                max_decoded_string_bytes: self.limits.max_decoded_string_bytes,
            },
        )
        .map_err(|error| rebase_error(error, span))
    }
}

/// Owned exact worksheet source.
#[derive(Clone, Debug)]
pub struct WorksheetDocument {
    source: String,
    limits: WorksheetLimits,
}

impl WorksheetDocument {
    pub fn parse(source: String, limits: WorksheetLimits) -> Result<Self, Error> {
        WorksheetView::parse(&source, limits)?;
        Ok(Self { source, limits })
    }

    pub fn from_reader(mut reader: impl Read, limits: WorksheetLimits) -> Result<Self, Error> {
        let read_limit = limits
            .max_source_bytes
            .checked_add(1)
            .ok_or_else(limit_error)?;
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(read_limit as u64)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > limits.max_source_bytes {
            return Err(limit_error());
        }
        Self::parse(utf8_text(&bytes)?.to_owned(), limits)
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn view(&self) -> Result<WorksheetView<'_>, Error> {
        WorksheetView::parse(&self.source, self.limits)
    }

    pub fn write_to(&self, mut writer: impl Write) -> Result<(), Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        writer.write_all(self.source.as_bytes()).map_err(io_error)
    }

    /// Update or create the worksheet setup line width and reparse before commit.
    pub fn set_setup_line_width(&mut self, line_width: f64) -> Result<bool, Error> {
        self.check_output_source()?;
        if !line_width.is_finite() {
            return Err(source_error(
                "worksheet line width must be finite",
                Position::START,
            ));
        }
        let replacement = build_with_limit(&Sexp::Float(line_width), self.limits.max_output_bytes)?;
        let view = self.view()?;
        let setup_spans = view
            .top_level
            .iter()
            .filter(|span| span.head.as_deref() == Some("setup"))
            .collect::<Vec<_>>();
        let patch = match setup_spans.as_slice() {
            [] => {
                let form = format!("(setup (linewidth {replacement}))");
                Some(insertion_patch(&self.source, &view.root, &form))
            }
            [setup_span] => self.setup_line_width_patch(setup_span, line_width, &replacement)?,
            _ => {
                return Err(source_error(
                    "worksheet setup form is ambiguous",
                    view.root.start,
                ));
            }
        };
        let Some(patch) = patch else {
            return Ok(false);
        };
        self.commit_patch(patch)
    }

    fn setup_line_width_patch<'a>(
        &self,
        setup_span: &FormSpan,
        line_width: f64,
        replacement: &'a str,
    ) -> Result<Option<Patch<'a>>, Error> {
        let setup = setup_span.text(&self.source)?;
        let spans = scan_form_spans_with_limits(
            setup,
            &Selector {
                heads: Some(["linewidth".to_owned()].into_iter().collect()),
                min_depth: Some(1),
                max_depth: Some(1),
                ..Selector::default()
            },
            ProjectionLimits {
                max_source_bytes: self.limits.max_source_bytes,
                max_depth: self.limits.max_depth,
                max_selected_forms: 2,
                ..ProjectionLimits::default()
            },
        )
        .map_err(|error| rebase_error(error, setup_span))?;
        match spans.as_slice() {
            [] => Ok(Some(insertion_patch(
                &self.source,
                setup_span,
                &format!("(linewidth {replacement})"),
            ))),
            [span] => {
                let (current, range) = scalar_value_range(setup, span)?;
                if number(&current)? == line_width {
                    return Ok(None);
                }
                Ok(Some(Patch::new(
                    setup_span.range.start + range.start,
                    setup_span.range.start + range.end,
                    replacement,
                )))
            }
            _ => Err(source_error(
                "worksheet setup linewidth is ambiguous",
                setup_span.start,
            )),
        }
    }

    fn check_output_source(&self) -> Result<(), Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        Ok(())
    }

    fn commit_patch(&mut self, patch: Patch<'_>) -> Result<bool, Error> {
        let candidate =
            apply_patches_with_limit(&self.source, &[patch], self.limits.max_output_bytes)?;
        WorksheetView::parse(&candidate, self.limits)?;
        self.source = candidate;
        Ok(true)
    }
}

fn scalar_value_range(
    source: &str,
    span: &FormSpan,
) -> Result<(Sexp, std::ops::Range<usize>), Error> {
    let text = span.text(source)?;
    let mut lexer = Lexer::new(text);
    for expected in [TokenKind::Left, TokenKind::Atom] {
        let token = lexer
            .next()
            .transpose()?
            .ok_or_else(|| source_error("worksheet scalar form is incomplete", span.start))?;
        if token.kind != expected {
            return Err(source_error(
                "worksheet scalar form is malformed",
                span.start,
            ));
        }
    }
    let token = lexer
        .next()
        .transpose()?
        .ok_or_else(|| source_error("worksheet scalar value is missing", span.start))?;
    if matches!(token.kind, TokenKind::Left | TokenKind::Right) {
        return Err(source_error(
            "worksheet scalar value is malformed",
            span.start,
        ));
    }
    let value = match token.kind {
        TokenKind::Integer => Sexp::Integer(
            token
                .lexeme
                .parse()
                .map_err(|_| source_error("expected worksheet integer", span.start))?,
        ),
        TokenKind::Float => Sexp::Float(
            token
                .lexeme
                .parse()
                .map_err(|_| source_error("expected worksheet number", span.start))?,
        ),
        _ => Sexp::Atom(token.lexeme.to_owned()),
    };
    Ok((
        value,
        (span.range.start + token.position.offset)
            ..(span.range.start + token.position.offset + token.lexeme.len()),
    ))
}

fn insertion_patch(source: &str, parent: &FormSpan, form: &str) -> Patch<'static> {
    let close = parent.range.end.saturating_sub(1);
    let line_start = source[..close].rfind('\n').map_or(0, |offset| offset + 1);
    let close_prefix = &source[line_start..close];
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let parent_line_start = source[..parent.range.start]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let parent_indent = source[parent_line_start..parent.range.start]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect::<String>();
    let child_indent = format!("{parent_indent}  ");
    let (offset, replacement) = if close_prefix.trim().is_empty() {
        (line_start, format!("{child_indent}{form}{newline}"))
    } else {
        (
            close,
            format!("{newline}{child_indent}{form}{newline}{parent_indent}"),
        )
    };
    Patch::new(offset, offset, replacement)
}

fn setup(value: &Sexp) -> Result<WorksheetSetup, Error> {
    let mut result = WorksheetSetup::default();
    if let Some(values) = child(value, "textsize") {
        result.text_size_x = number_at(values, 1, 1.5)?;
        result.text_size_y = number_at(values, 2, result.text_size_x)?;
    }
    result.line_width = child_number(value, "linewidth", 0.15)?;
    result.text_line_width = child_number(value, "textlinewidth", 0.15)?;
    result.left_margin = child_number(value, "left_margin", 10.0)?;
    result.right_margin = child_number(value, "right_margin", 10.0)?;
    result.top_margin = child_number(value, "top_margin", 10.0)?;
    result.bottom_margin = child_number(value, "bottom_margin", 10.0)?;
    Ok(result)
}

fn line(value: Sexp) -> Result<WorksheetLine, Error> {
    Ok(WorksheetLine {
        name: child_string(&value, "name"),
        comment: child_string(&value, "comment"),
        option: option(&value),
        start: point(&value, "start")?,
        end: point(&value, "end")?,
        line_width: child_optional_number(&value, "linewidth")?,
        repeat: repeat(&value)?,
    })
}

fn rect(value: Sexp) -> Result<WorksheetRect, Error> {
    Ok(WorksheetRect {
        name: child_string(&value, "name"),
        comment: child_string(&value, "comment"),
        option: option(&value),
        start: point(&value, "start")?,
        end: point(&value, "end")?,
        line_width: child_optional_number(&value, "linewidth")?,
        repeat: repeat(&value)?,
    })
}

fn polygon(value: Sexp, limits: WorksheetLimits) -> Result<WorksheetPolygon, Error> {
    let mut point_sets = Vec::new();
    let mut point_count = 0usize;
    for values in children(&value, "pts") {
        let mut points = Vec::new();
        for xy in children_in(values, "xy") {
            if point_count >= limits.max_points_per_polygon {
                return Err(limit_error());
            }
            if xy.len() >= 3 {
                points.push((number_at(xy, 1, 0.0)?, number_at(xy, 2, 0.0)?));
                point_count += 1;
            }
        }
        if !points.is_empty() {
            if point_sets.len() >= limits.max_point_sets_per_polygon {
                return Err(limit_error());
            }
            point_sets.push(points);
        }
    }
    Ok(WorksheetPolygon {
        name: child_string(&value, "name"),
        comment: child_string(&value, "comment"),
        option: option(&value),
        position: point(&value, "pos")?,
        rotate: child_number(&value, "rotate", 0.0)?,
        line_width: child_optional_number(&value, "linewidth")?,
        repeat: repeat(&value)?,
        point_sets,
    })
}

fn text(value: Sexp, limits: WorksheetLimits) -> Result<WorksheetText, Error> {
    let mut justify = Vec::new();
    if let Some(values) = child(&value, "justify") {
        for item in &values[1..] {
            let value = scalar(item);
            if matches!(
                value.as_str(),
                "left" | "center" | "right" | "top" | "bottom"
            ) {
                if justify.len() >= limits.max_justify_tokens {
                    return Err(limit_error());
                }
                justify.push(value);
            }
        }
    }
    Ok(WorksheetText {
        text: list(&value).get(1).map_or_else(String::new, scalar),
        name: child_string(&value, "name"),
        comment: child_string(&value, "comment"),
        option: option(&value),
        position: point(&value, "pos")?,
        font: font(&value)?,
        justify,
        rotate: child_number(&value, "rotate", 0.0)?,
        repeat: repeat(&value)?,
        max_length: child_number(&value, "maxlen", 0.0)?,
        max_height: child_number(&value, "maxheight", 0.0)?,
    })
}

fn font(value: &Sexp) -> Result<WorksheetFont, Error> {
    let Some(font) = child(value, "font") else {
        return Ok(WorksheetFont::default());
    };
    let (size_x, size_y) = child_in(font, "size").map_or(Ok((0.0, 0.0)), |size| {
        let x = number_at(size, 1, 0.0)?;
        Ok((x, number_at(size, 2, x)?))
    })?;
    let color = child_in(font, "color")
        .filter(|color| color.len() >= 5)
        .map(|color| {
            Ok(WorksheetColor {
                red: integer_at(color, 1, 0)?,
                green: integer_at(color, 2, 0)?,
                blue: integer_at(color, 3, 0)?,
                alpha: number_at(color, 4, 0.0)?,
            })
        })
        .transpose()?;
    Ok(WorksheetFont {
        size_x,
        size_y,
        line_width: child_optional_number_from_list(font, "linewidth")?,
        bold: has_atom(font, "bold"),
        italic: has_atom(font, "italic"),
        face: child_string_from_list(font, "face"),
        color,
    })
}

fn bitmap(value: Sexp, limits: WorksheetLimits) -> Result<WorksheetBitmap, Error> {
    let mut data_parts = Vec::new();
    let mut data_bytes = 0usize;
    let data = child(&value, "data").or_else(|| child(&value, "pngdata"));
    if let Some(values) = data {
        for item in &values[1..] {
            if data_parts.len() >= limits.max_bitmap_data_parts {
                return Err(limit_error());
            }
            let part = scalar(item);
            data_bytes = data_bytes.checked_add(part.len()).ok_or_else(limit_error)?;
            if data_bytes > limits.max_bitmap_data_bytes {
                return Err(limit_error());
            }
            data_parts.push(part);
        }
    }
    Ok(WorksheetBitmap {
        name: child_string(&value, "name"),
        comment: child_string(&value, "comment"),
        option: option(&value),
        position: point(&value, "pos")?,
        scale: child_number(&value, "scale", 1.0)?,
        repeat: repeat(&value)?,
        data_parts,
    })
}

fn point(value: &Sexp, head: &str) -> Result<WorksheetPoint, Error> {
    let Some(values) = child(value, head) else {
        return Ok(WorksheetPoint::default());
    };
    Ok(WorksheetPoint {
        x: number_at(values, 1, 0.0)?,
        y: number_at(values, 2, 0.0)?,
        corner: values
            .get(3)
            .map_or(WorksheetCorner::None, |value| corner(&scalar(value))),
    })
}

fn repeat(value: &Sexp) -> Result<WorksheetRepeat, Error> {
    Ok(WorksheetRepeat {
        count: child(value, "repeat").map_or(Ok(1), |values| integer_at(values, 1, 1))?,
        increment_x: child_number(value, "incrx", 0.0)?,
        increment_y: child_number(value, "incry", 0.0)?,
        increment_label: child(value, "incrlabel")
            .map_or(Ok(0), |values| integer_at(values, 1, 0))?,
    })
}

fn option(value: &Sexp) -> String {
    match child_string(value, "option").as_str() {
        "page1only" => "page1only".to_owned(),
        "notonpage1" => "notonpage1".to_owned(),
        _ => String::new(),
    }
}

fn corner(value: &str) -> WorksheetCorner {
    match value {
        "ltcorner" => WorksheetCorner::LeftTop,
        "rtcorner" => WorksheetCorner::RightTop,
        "lbcorner" => WorksheetCorner::LeftBottom,
        "rbcorner" => WorksheetCorner::RightBottom,
        _ => WorksheetCorner::None,
    }
}

fn is_item_head(head: Option<&str>) -> bool {
    matches!(
        head,
        Some("line" | "rect" | "polygon" | "tbtext" | "bitmap")
    )
}

fn list(value: &Sexp) -> &[Sexp] {
    match value {
        Sexp::List(values) => values,
        _ => &[],
    }
}

fn child<'a>(value: &'a Sexp, head: &str) -> Option<&'a [Sexp]> {
    child_in(list(value), head)
}

fn children<'a>(value: &'a Sexp, head: &'a str) -> impl Iterator<Item = &'a [Sexp]> {
    children_in(list(value), head)
}

fn child_in<'a>(value: &'a [Sexp], head: &str) -> Option<&'a [Sexp]> {
    value.iter().find_map(|child| {
        let values = list(child);
        values
            .first()
            .is_some_and(|value| is_atom(value, head))
            .then_some(values)
    })
}

fn children_in<'a>(value: &'a [Sexp], head: &'a str) -> impl Iterator<Item = &'a [Sexp]> {
    value.iter().filter_map(move |child| {
        let values = list(child);
        values
            .first()
            .is_some_and(|value| is_atom(value, head))
            .then_some(values)
    })
}

fn is_atom(value: &Sexp, expected: &str) -> bool {
    matches!(value, Sexp::Atom(value) if value == expected)
}

fn child_string(value: &Sexp, head: &str) -> String {
    child(value, head)
        .and_then(|values| values.get(1))
        .map_or_else(String::new, scalar)
}

fn child_string_from_list(value: &[Sexp], head: &str) -> String {
    value
        .iter()
        .find_map(|child| {
            let values = list(child);
            (values.first().is_some_and(|value| is_atom(value, head)))
                .then(|| values.get(1).map_or_else(String::new, scalar))
        })
        .unwrap_or_default()
}

fn child_number(value: &Sexp, head: &str, default: f64) -> Result<f64, Error> {
    child(value, head).map_or(Ok(default), |values| number_at(values, 1, default))
}

fn child_optional_number(value: &Sexp, head: &str) -> Result<Option<f64>, Error> {
    child(value, head)
        .map(|values| number_at(values, 1, 0.0))
        .transpose()
}

fn child_optional_number_from_list(value: &[Sexp], head: &str) -> Result<Option<f64>, Error> {
    value
        .iter()
        .find_map(|child| {
            let values = list(child);
            values
                .first()
                .is_some_and(|value| is_atom(value, head))
                .then_some(values)
        })
        .map(|values| number_at(values, 1, 0.0))
        .transpose()
}

fn has_atom(value: &[Sexp], atom: &str) -> bool {
    value
        .iter()
        .any(|value| matches!(value, Sexp::Atom(found) if found == atom))
}

fn number_at(values: &[Sexp], index: usize, default: f64) -> Result<f64, Error> {
    values.get(index).map_or(Ok(default), number)
}

fn number(value: &Sexp) -> Result<f64, Error> {
    let parsed = match value {
        Sexp::Integer(value) => *value as f64,
        Sexp::Float(value) => *value,
        _ => scalar(value)
            .parse::<f64>()
            .map_err(|_| source_error("expected worksheet number", Position::START))?,
    };
    if !parsed.is_finite() {
        return Err(source_error(
            "worksheet number must be finite",
            Position::START,
        ));
    }
    Ok(parsed)
}

fn integer_at(values: &[Sexp], index: usize, default: i64) -> Result<i64, Error> {
    values.get(index).map_or(Ok(default), integer)
}

fn integer(value: &Sexp) -> Result<i64, Error> {
    match value {
        Sexp::Integer(value) => Ok(*value),
        _ => integer_text(&scalar(value)),
    }
}

fn integer_text(value: &str) -> Result<i64, Error> {
    value
        .parse()
        .map_err(|_| source_error("expected worksheet integer", Position::START))
}

fn scalar(value: &Sexp) -> String {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => value.clone(),
        Sexp::Integer(value) => value.to_string(),
        Sexp::Float(value) => value.to_string(),
        Sexp::List(_) => String::new(),
    }
}

fn rebase_error(mut error: Error, span: &FormSpan) -> Error {
    if let Some(position) = error.position {
        error.position = Some(Position {
            offset: span.range.start.saturating_add(position.offset),
            line: span
                .start
                .line
                .saturating_add(position.line.saturating_sub(1)),
            column: if position.line == 1 {
                span.start
                    .column
                    .saturating_add(position.column.saturating_sub(1))
            } else {
                position.column
            },
        });
    }
    error
}

fn source_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "worksheet operation exceeds configured limits",
        Position::START,
    )
}

fn output_limit_error() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "worksheet output exceeds max_output_bytes",
    )
}

fn io_error(error: std::io::Error) -> Error {
    Error::build(
        ErrorKind::Io,
        format!("worksheet source I/O failed: {error}"),
    )
}

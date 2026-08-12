//! First TypeSpec-backed footprint plotter-IR vertical slice.

use crate::footprint::{FootprintLimits, FootprintView};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Lexer, Position, Sexp, TokenKind, parse};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
const DEFAULT_FOOTPRINT_VERSION: i64 = 20_260_206;
const DEFAULT_GENERATOR: &str = "pcbnew";
const DEFAULT_GENERATOR_VERSION: &str = "10.0";
const DEFAULT_FOOTPRINT_LAYER: &str = "F.Cu";
const DEFAULT_LINE_LAYER: &str = "F.SilkS";
const DEFAULT_STROKE_WIDTH_NM: i64 = 152_400;
const MIN_PLOT_PEN_WIDTH_NM: i64 = 84_700;

/// Limits for the first footprint plotter operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FootprintPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_operations: usize,
}

impl Default for FootprintPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_operations: 100_000,
        }
    }
}

/// Solid footprint line represented in the established plotter vocabulary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThickSegment {
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub width_nm: i64,
    pub layer: String,
}

/// Typed facts needed to serialize the first footprint plotter document subset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintPlotDocument {
    pub name: String,
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
    pub layer: String,
    pub uuid: String,
    pub descr: String,
    pub tags: String,
    pub attr: Vec<String>,
    pub locked: bool,
    pub placed: bool,
    pub operations: Vec<ThickSegment>,
}

/// Read supported footprint geometry directly from selected forms.
pub fn footprint_plot_document(
    source: &str,
    limits: FootprintPlotLimits,
) -> Result<FootprintPlotDocument, Error> {
    let footprint_limits = FootprintLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        ..FootprintLimits::default()
    };
    let view = FootprintView::parse(source, footprint_limits)?;
    let name = view.name()?.into_owned();
    let max_selected_forms = limits
        .max_operations
        .checked_add(8)
        .ok_or_else(limit_error)?;
    let paths = [
        "version",
        "generator",
        "generator_version",
        "layer",
        "uuid",
        "descr",
        "tags",
        "attr",
        "fp_line",
    ]
    .into_iter()
    .map(|head| vec!["footprint".to_owned(), head.to_owned()])
    .collect();
    let spans = scan_form_spans_with_limits(
        source,
        &Selector {
            paths: Some(paths),
            min_depth: Some(1),
            max_depth: Some(1),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms,
            ..ProjectionLimits::default()
        },
    )?;

    let mut version = None;
    let mut generator = None;
    let mut generator_version = None;
    let mut layer = None;
    let mut uuid = None;
    let mut descr = None;
    let mut tags = None;
    let mut attr = None;
    let mut operations = Vec::new();
    for span in spans {
        match span.head.as_deref() {
            Some("version") if version.is_none() => {
                version = Some(form_integer(source, &span, "version")?);
            }
            Some("generator") if generator.is_none() => {
                generator = Some(form_string(source, &span, "generator")?);
            }
            Some("generator_version") if generator_version.is_none() => {
                generator_version = Some(form_string(source, &span, "generator_version")?);
            }
            Some("layer") if layer.is_none() => {
                layer = Some(form_string(source, &span, "layer")?);
            }
            Some("uuid") if uuid.is_none() => {
                uuid = Some(form_string(source, &span, "uuid")?);
            }
            Some("descr") if descr.is_none() => {
                descr = Some(form_string(source, &span, "descr")?);
            }
            Some("tags") if tags.is_none() => {
                tags = Some(form_string(source, &span, "tags")?);
            }
            Some("attr") if attr.is_none() => {
                attr = Some(form_strings(source, &span, "attr")?);
            }
            Some("fp_line") => {
                if operations.len() >= limits.max_operations {
                    return Err(limit_error());
                }
                operations.push(parse_line(source, &span)?);
            }
            _ => {}
        }
    }
    let (locked, placed) = root_flags(source)?;
    Ok(FootprintPlotDocument {
        name,
        version: version.unwrap_or(DEFAULT_FOOTPRINT_VERSION),
        generator: generator.unwrap_or_else(|| DEFAULT_GENERATOR.to_owned()),
        generator_version: generator_version
            .unwrap_or_else(|| DEFAULT_GENERATOR_VERSION.to_owned()),
        layer: layer.unwrap_or_else(|| DEFAULT_FOOTPRINT_LAYER.to_owned()),
        uuid: uuid.unwrap_or_default(),
        descr: descr.unwrap_or_default(),
        tags: tags.unwrap_or_default(),
        attr: attr.unwrap_or_default(),
        locked,
        placed,
        operations,
    })
}

fn parse_line(source: &str, span: &FormSpan) -> Result<ThickSegment, Error> {
    let form = parse_span(source, span)?;
    let start =
        child(&form, "start").ok_or_else(|| model_error("fp_line requires start", span.start))?;
    let end = child(&form, "end").ok_or_else(|| model_error("fp_line requires end", span.start))?;
    let start_x = mm_to_nm(numeric_at(start, 1, span.start)?)?;
    let start_y = mm_to_nm(numeric_at(start, 2, span.start)?)?;
    let end_x = mm_to_nm(numeric_at(end, 1, span.start)?)?;
    let end_y = mm_to_nm(numeric_at(end, 2, span.start)?)?;
    let layer = child(&form, "layer")
        .and_then(|value| value_at(value, 1))
        .unwrap_or(DEFAULT_LINE_LAYER)
        .to_owned();
    let stroke = child(&form, "stroke");
    let stroke_type = stroke
        .and_then(|value| child(value, "type"))
        .and_then(|value| value_at(value, 1))
        .unwrap_or("default");
    if !matches!(stroke_type, "default" | "solid") {
        return Err(model_error(
            "Initial footprint plotter slice supports only solid fp_line strokes",
            span.start,
        ));
    }
    let width_mm = stroke
        .and_then(|value| child(value, "width"))
        .map(|value| numeric_at(value, 1, span.start))
        .transpose()?
        .unwrap_or(0.0);
    let width_nm = if width_mm < 0.0 {
        0
    } else if width_mm == 0.0 {
        DEFAULT_STROKE_WIDTH_NM.max(MIN_PLOT_PEN_WIDTH_NM)
    } else {
        mm_to_nm(width_mm)?.max(MIN_PLOT_PEN_WIDTH_NM)
    };
    Ok(ThickSegment {
        start_x,
        start_y,
        end_x,
        end_y,
        width_nm,
        layer,
    })
}

fn form_integer(source: &str, span: &FormSpan, head: &str) -> Result<i64, Error> {
    let form = parse_span(source, span)?;
    match metadata_values(&form, head, span.start)?.first() {
        Some(Sexp::Integer(value)) => Ok(*value),
        Some(Sexp::Atom(value)) => value
            .parse::<i64>()
            .map_err(|_| model_error("Expected integer metadata", span.start)),
        _ => Err(model_error("Expected integer metadata", span.start)),
    }
}

fn form_string(source: &str, span: &FormSpan, head: &str) -> Result<String, Error> {
    let form = parse_span(source, span)?;
    metadata_values(&form, head, span.start)?
        .first()
        .and_then(sexp_text)
        .map(str::to_owned)
        .ok_or_else(|| model_error("Metadata value is missing", span.start))
}

fn form_strings(source: &str, span: &FormSpan, head: &str) -> Result<Vec<String>, Error> {
    let form = parse_span(source, span)?;
    let list = list(&form).ok_or_else(|| model_error("Expected list metadata", span.start))?;
    if list.first().and_then(sexp_text) != Some(head) {
        return Err(model_error("Unexpected metadata form", span.start));
    }
    Ok(list
        .iter()
        .skip(1)
        .filter_map(sexp_text)
        .map(str::to_owned)
        .collect())
}

fn metadata_values<'a>(
    form: &'a Sexp,
    head: &str,
    position: Position,
) -> Result<&'a [Sexp], Error> {
    let list = list(form).ok_or_else(|| model_error("Expected metadata list", position))?;
    if list.first().and_then(sexp_text) != Some(head) {
        return Err(model_error("Unexpected metadata form", position));
    }
    Ok(&list[1..])
}

fn parse_span(source: &str, span: &FormSpan) -> Result<Sexp, Error> {
    parse(span.text(source)?).map_err(|error| rebase_error(error, span))
}

fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
    list(form)?.iter().find(|candidate| {
        list(candidate)
            .and_then(|values| values.first())
            .and_then(sexp_text)
            == Some(head)
    })
}

fn list(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn sexp_text(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn value_at(form: &Sexp, index: usize) -> Option<&str> {
    list(form)?.get(index).and_then(sexp_text)
}

fn numeric_at(form: &Sexp, index: usize, position: Position) -> Result<f64, Error> {
    match list(form).and_then(|values| values.get(index)) {
        Some(Sexp::Integer(value)) => Ok(*value as f64),
        Some(Sexp::Float(value)) => Ok(*value),
        Some(Sexp::Atom(value)) => value
            .parse::<f64>()
            .map_err(|_| model_error("Expected numeric coordinate", position)),
        _ => Err(model_error("Expected numeric coordinate", position)),
    }
}

fn mm_to_nm(value: f64) -> Result<i64, Error> {
    let scaled = value * 1_000_000.0;
    if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
        return Err(model_error(
            "Coordinate exceeds plotter integer range",
            Position::START,
        ));
    }
    Ok(scaled.round_ties_even() as i64)
}

fn root_flags(source: &str) -> Result<(bool, bool), Error> {
    let mut depth = 0usize;
    let mut locked = false;
    let mut placed = false;
    for token in Lexer::new(source) {
        let token = token?;
        match token.kind {
            TokenKind::Left => depth = depth.saturating_add(1),
            TokenKind::Right => depth = depth.saturating_sub(1),
            TokenKind::Atom if depth == 1 && token.lexeme == "locked" => locked = true,
            TokenKind::Atom if depth == 1 && token.lexeme == "placed" => placed = true,
            _ => {}
        }
    }
    Ok((locked, placed))
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

fn model_error(message: &'static str, position: Position) -> Error {
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
        "Footprint plotter operation exceeds configured limits",
        Position::START,
    )
}

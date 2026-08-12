//! TypeSpec-backed footprint graphics to plotter-IR conversion.

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
const JAVASCRIPT_SAFE_INTEGER_MAX: i64 = 9_007_199_254_740_991;
const JAVASCRIPT_SAFE_INTEGER_MIN: i64 = -JAVASCRIPT_SAFE_INTEGER_MAX;
const DASH_RATIO: f64 = 11.0;
const GAP_RATIO: f64 = 4.0;
const DOT_RATIO: f64 = 0.2;
const ARC_CHORD_STEP_RADIANS: f64 = std::f64::consts::PI / 360.0;
const MAX_DECOMPOSITION_STEPS: usize = 10_000;

/// Limits for the first footprint plotter operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FootprintPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_metadata_forms: usize,
    pub max_operations: usize,
}

impl Default for FootprintPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_metadata_forms: 256,
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

/// Fill values used by the promoted footprint graphic operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotterFill {
    NoFill,
    FilledShape,
}

/// Solid three-point footprint arc.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcThreePoint {
    pub start_x: i64,
    pub start_y: i64,
    pub mid_x: i64,
    pub mid_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub layer: String,
}

/// Footprint circle represented by its center and diameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotterCircle {
    pub cx: i64,
    pub cy: i64,
    pub diameter_nm: i64,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub layer: String,
}

/// Footprint rectangle with square corners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotterRect {
    pub x1: i64,
    pub y1: i64,
    pub x2: i64,
    pub y2: i64,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub corner_radius_nm: i64,
    pub layer: String,
}

/// Footprint polygon point stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotterPoly {
    pub points: Vec<[i64; 2]>,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub layer: String,
}

/// Promoted non-text footprint graphic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FootprintGraphicOperation {
    ThickSegment(ThickSegment),
    ArcThreePoint(ArcThreePoint),
    Circle(PlotterCircle),
    Rect(PlotterRect),
    PlotPoly(PlotterPoly),
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
    pub operations: Vec<FootprintGraphicOperation>,
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
        .checked_add(limits.max_metadata_forms)
        .and_then(|value| value.checked_add(1))
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
        "fp_arc",
        "fp_circle",
        "fp_rect",
        "fp_poly",
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
    let mut metadata_forms = 0usize;
    let mut line_spans = Vec::new();
    let mut arc_spans = Vec::new();
    let mut circle_spans = Vec::new();
    let mut rect_spans = Vec::new();
    let mut poly_spans = Vec::new();
    for span in spans {
        if !matches!(
            span.head.as_deref(),
            Some("fp_line" | "fp_arc" | "fp_circle" | "fp_rect" | "fp_poly")
        ) {
            metadata_forms = metadata_forms.saturating_add(1);
            if metadata_forms > limits.max_metadata_forms {
                return Err(metadata_limit_error());
            }
        }
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
            Some("fp_line") => line_spans.push(span),
            Some("fp_arc") => arc_spans.push(span),
            Some("fp_circle") => circle_spans.push(span),
            Some("fp_rect") => rect_spans.push(span),
            Some("fp_poly") => poly_spans.push(span),
            _ => {}
        }
    }
    let mut operations = Vec::new();
    for span in line_spans {
        let remaining = remaining_operations(&operations, limits)?;
        let additions = parse_line(source, &span, remaining)?;
        append_operations(&mut operations, additions, limits.max_operations)?;
    }
    for span in arc_spans {
        let remaining = remaining_operations(&operations, limits)?;
        let additions = parse_arc(source, &span, remaining)?;
        append_operations(&mut operations, additions, limits.max_operations)?;
    }
    for span in circle_spans {
        append_operations(
            &mut operations,
            vec![parse_circle(source, &span)?],
            limits.max_operations,
        )?;
    }
    for span in rect_spans {
        append_operations(
            &mut operations,
            vec![parse_rect(source, &span)?],
            limits.max_operations,
        )?;
    }
    for span in poly_spans {
        append_operations(
            &mut operations,
            vec![parse_poly(source, &span)?],
            limits.max_operations,
        )?;
    }
    let (locked, placed) = root_flags(source)?;
    let version = version.unwrap_or(DEFAULT_FOOTPRINT_VERSION);
    ensure_javascript_safe_integer(version)?;
    Ok(FootprintPlotDocument {
        name,
        version,
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

fn parse_line(
    source: &str,
    span: &FormSpan,
    max_operations: usize,
) -> Result<Vec<FootprintGraphicOperation>, Error> {
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
    let stroke = parse_stroke(&form, span.start)?;
    let solid = ThickSegment {
        start_x,
        start_y,
        end_x,
        end_y,
        width_nm: stroke.width_nm,
        layer: layer.clone(),
    };
    if matches!(stroke.style, StrokeStyle::Default | StrokeStyle::Solid) {
        return Ok(vec![FootprintGraphicOperation::ThickSegment(solid)]);
    }
    let pieces = decompose_segment(
        start_x,
        start_y,
        end_x,
        end_y,
        stroke.width_nm,
        stroke.style,
        max_operations,
    )?;
    if pieces.is_empty() {
        return Ok(vec![FootprintGraphicOperation::ThickSegment(solid)]);
    }
    Ok(pieces
        .into_iter()
        .map(|[start_x, start_y, end_x, end_y]| {
            FootprintGraphicOperation::ThickSegment(ThickSegment {
                start_x,
                start_y,
                end_x,
                end_y,
                width_nm: stroke.width_nm,
                layer: layer.clone(),
            })
        })
        .collect())
}

fn parse_arc(
    source: &str,
    span: &FormSpan,
    max_operations: usize,
) -> Result<Vec<FootprintGraphicOperation>, Error> {
    let form = parse_span(source, span)?;
    let start =
        child(&form, "start").ok_or_else(|| model_error("fp_arc requires start", span.start))?;
    let mid = child(&form, "mid").ok_or_else(|| model_error("fp_arc requires mid", span.start))?;
    let end = child(&form, "end").ok_or_else(|| model_error("fp_arc requires end", span.start))?;
    let start_x = mm_to_nm(numeric_at(start, 1, span.start)?)?;
    let start_y = mm_to_nm(numeric_at(start, 2, span.start)?)?;
    let mid_x = mm_to_nm(numeric_at(mid, 1, span.start)?)?;
    let mid_y = mm_to_nm(numeric_at(mid, 2, span.start)?)?;
    let end_x = mm_to_nm(numeric_at(end, 1, span.start)?)?;
    let end_y = mm_to_nm(numeric_at(end, 2, span.start)?)?;
    let layer = graphic_layer(&form);
    let stroke = parse_stroke(&form, span.start)?;
    if matches!(stroke.style, StrokeStyle::Default | StrokeStyle::Solid) {
        return Ok(vec![FootprintGraphicOperation::ArcThreePoint(
            ArcThreePoint {
                start_x,
                start_y,
                mid_x,
                mid_y,
                end_x,
                end_y,
                fill: PlotterFill::NoFill,
                width_nm: stroke.width_nm,
                layer,
            },
        )]);
    }
    let pieces = decompose_arc(
        [start_x, start_y],
        [mid_x, mid_y],
        [end_x, end_y],
        stroke.width_nm,
        stroke.style,
        max_operations,
    )?;
    if pieces.is_empty() {
        return Ok(vec![FootprintGraphicOperation::ArcThreePoint(
            ArcThreePoint {
                start_x,
                start_y,
                mid_x,
                mid_y,
                end_x,
                end_y,
                fill: PlotterFill::NoFill,
                width_nm: stroke.width_nm,
                layer,
            },
        )]);
    }
    Ok(pieces
        .into_iter()
        .map(|[start_x, start_y, end_x, end_y]| {
            FootprintGraphicOperation::ThickSegment(ThickSegment {
                start_x,
                start_y,
                end_x,
                end_y,
                width_nm: stroke.width_nm,
                layer: layer.clone(),
            })
        })
        .collect())
}

fn parse_circle(source: &str, span: &FormSpan) -> Result<FootprintGraphicOperation, Error> {
    let form = parse_span(source, span)?;
    let center = child(&form, "center")
        .ok_or_else(|| model_error("fp_circle requires center", span.start))?;
    let end =
        child(&form, "end").ok_or_else(|| model_error("fp_circle requires end", span.start))?;
    let center_x_mm = numeric_at(center, 1, span.start)?;
    let center_y_mm = numeric_at(center, 2, span.start)?;
    let end_x_mm = numeric_at(end, 1, span.start)?;
    let end_y_mm = numeric_at(end, 2, span.start)?;
    let stroke = parse_stroke(&form, span.start)?;
    Ok(FootprintGraphicOperation::Circle(PlotterCircle {
        cx: mm_to_nm(center_x_mm)?,
        cy: mm_to_nm(center_y_mm)?,
        diameter_nm: mm_to_nm(2.0 * (end_x_mm - center_x_mm).hypot(end_y_mm - center_y_mm))?,
        fill: graphic_fill(&form),
        width_nm: stroke.width_nm,
        layer: graphic_layer(&form),
    }))
}

fn parse_rect(source: &str, span: &FormSpan) -> Result<FootprintGraphicOperation, Error> {
    let form = parse_span(source, span)?;
    let start =
        child(&form, "start").ok_or_else(|| model_error("fp_rect requires start", span.start))?;
    let end = child(&form, "end").ok_or_else(|| model_error("fp_rect requires end", span.start))?;
    let stroke = parse_stroke(&form, span.start)?;
    Ok(FootprintGraphicOperation::Rect(PlotterRect {
        x1: mm_to_nm(numeric_at(start, 1, span.start)?)?,
        y1: mm_to_nm(numeric_at(start, 2, span.start)?)?,
        x2: mm_to_nm(numeric_at(end, 1, span.start)?)?,
        y2: mm_to_nm(numeric_at(end, 2, span.start)?)?,
        fill: graphic_fill(&form),
        width_nm: stroke.width_nm,
        corner_radius_nm: 0,
        layer: graphic_layer(&form),
    }))
}

fn parse_poly(source: &str, span: &FormSpan) -> Result<FootprintGraphicOperation, Error> {
    let form = parse_span(source, span)?;
    let points_form =
        child(&form, "pts").ok_or_else(|| model_error("fp_poly requires pts", span.start))?;
    let points = list(points_form)
        .ok_or_else(|| model_error("fp_poly pts must be a list", span.start))?
        .iter()
        .skip(1)
        .filter(|value| {
            list(value)
                .and_then(|items| items.first())
                .and_then(sexp_text)
                == Some("xy")
        })
        .map(|value| {
            Ok([
                mm_to_nm(numeric_at(value, 1, span.start)?)?,
                mm_to_nm(numeric_at(value, 2, span.start)?)?,
            ])
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let stroke = parse_stroke(&form, span.start)?;
    Ok(FootprintGraphicOperation::PlotPoly(PlotterPoly {
        points,
        fill: graphic_fill(&form),
        width_nm: stroke.width_nm,
        layer: graphic_layer(&form),
    }))
}

fn append_operations(
    operations: &mut Vec<FootprintGraphicOperation>,
    additions: Vec<FootprintGraphicOperation>,
    max_operations: usize,
) -> Result<(), Error> {
    if additions.len() > max_operations.saturating_sub(operations.len()) {
        return Err(limit_error());
    }
    operations.extend(additions);
    Ok(())
}

fn remaining_operations(
    operations: &[FootprintGraphicOperation],
    limits: FootprintPlotLimits,
) -> Result<usize, Error> {
    limits
        .max_operations
        .checked_sub(operations.len())
        .filter(|remaining| *remaining > 0)
        .ok_or_else(limit_error)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StrokeStyle {
    Default,
    Solid,
    Dash,
    Dot,
    DashDot,
    DashDotDot,
}

#[derive(Clone, Copy, Debug)]
struct StrokeSpec {
    width_nm: i64,
    style: StrokeStyle,
}

fn parse_stroke(form: &Sexp, position: Position) -> Result<StrokeSpec, Error> {
    let stroke = child(form, "stroke");
    let width_mm = stroke
        .and_then(|value| child(value, "width"))
        .map(|value| numeric_at(value, 1, position))
        .transpose()?
        .unwrap_or(0.0);
    let width_nm = if width_mm < 0.0 {
        0
    } else if width_mm == 0.0 {
        DEFAULT_STROKE_WIDTH_NM.max(MIN_PLOT_PEN_WIDTH_NM)
    } else {
        mm_to_nm(width_mm)?.max(MIN_PLOT_PEN_WIDTH_NM)
    };
    let style = match stroke
        .and_then(|value| child(value, "type"))
        .and_then(|value| value_at(value, 1))
        .unwrap_or("default")
    {
        "default" => StrokeStyle::Default,
        "solid" => StrokeStyle::Solid,
        "dash" => StrokeStyle::Dash,
        "dot" => StrokeStyle::Dot,
        "dash_dot" => StrokeStyle::DashDot,
        "dash_dot_dot" => StrokeStyle::DashDotDot,
        _ => return Err(model_error("Unsupported footprint stroke type", position)),
    };
    Ok(StrokeSpec { width_nm, style })
}

fn graphic_layer(form: &Sexp) -> String {
    child(form, "layer")
        .and_then(|value| value_at(value, 1))
        .unwrap_or(DEFAULT_LINE_LAYER)
        .to_owned()
}

fn graphic_fill(form: &Sexp) -> PlotterFill {
    match child(form, "fill").and_then(|value| value_at(value, 1)) {
        Some("yes" | "solid") => PlotterFill::FilledShape,
        _ => PlotterFill::NoFill,
    }
}

fn stroke_pattern(style: StrokeStyle, width_nm: i64) -> Result<(Vec<f64>, usize), Error> {
    let width = width_nm as f64;
    let dash = DASH_RATIO * width;
    let gap = GAP_RATIO * width;
    let dot = DOT_RATIO * width;
    match style {
        StrokeStyle::Dash => Ok((vec![dash, gap], 2)),
        StrokeStyle::Dot => Ok((vec![dot, gap], 2)),
        StrokeStyle::DashDot => Ok((vec![dash, gap, dot, gap], 4)),
        StrokeStyle::DashDotDot => Ok((vec![dash, gap, dot, gap, dot, gap], 6)),
        StrokeStyle::Default | StrokeStyle::Solid => Err(model_error(
            "Solid strokes do not require decomposition",
            Position::START,
        )),
    }
}

fn decompose_segment(
    start_x: i64,
    start_y: i64,
    end_x: i64,
    end_y: i64,
    width_nm: i64,
    style: StrokeStyle,
    max_operations: usize,
) -> Result<Vec<[i64; 4]>, Error> {
    let (strokes, wrap) = stroke_pattern(style, width_nm)?;
    let delta_x = (end_x - start_x) as f64;
    let delta_y = (end_y - start_y) as f64;
    let total = delta_x.hypot(delta_y);
    if total <= 0.0 {
        return Ok(Vec::new());
    }
    let unit_x = delta_x / total;
    let unit_y = delta_y / total;
    let mut output = Vec::new();
    let mut current = 0.0;
    let mut index = 0usize;
    while current < total && index < MAX_DECOMPOSITION_STEPS {
        let next = current + strokes[index % wrap];
        if index.is_multiple_of(2) {
            let end_along = next.min(total);
            if end_along > current {
                push_decomposed_segment(
                    &mut output,
                    [
                        rounded_safe_f64(start_x as f64 + unit_x * current)?,
                        rounded_safe_f64(start_y as f64 + unit_y * current)?,
                        rounded_safe_f64(start_x as f64 + unit_x * end_along)?,
                        rounded_safe_f64(start_y as f64 + unit_y * end_along)?,
                    ],
                    max_operations,
                )?;
            }
        }
        current = next;
        index += 1;
    }
    Ok(output)
}

fn decompose_arc(
    start: [i64; 2],
    mid: [i64; 2],
    end: [i64; 2],
    width_nm: i64,
    style: StrokeStyle,
    max_operations: usize,
) -> Result<Vec<[i64; 4]>, Error> {
    let (strokes, wrap) = stroke_pattern(style, width_nm)?;
    let Some((center_x, center_y, radius)) = arc_center_radius(start, mid, end) else {
        return decompose_segment(
            start[0],
            start[1],
            end[0],
            end[1],
            width_nm,
            style,
            max_operations,
        );
    };
    if radius <= 0.0 {
        return Ok(Vec::new());
    }
    let circumference = std::f64::consts::TAU * radius;
    let start_angle_raw = (start[1] as f64 - center_y).atan2(start[0] as f64 - center_x);
    let mid_angle_raw = (mid[1] as f64 - center_y).atan2(mid[0] as f64 - center_x);
    let end_angle_raw = (end[1] as f64 - center_y).atan2(end[0] as f64 - center_x);
    let (start_angle, arc_end_angle) =
        normalize_arc_sweep(start_angle_raw, mid_angle_raw, end_angle_raw);
    let mut output = Vec::new();
    let mut index = 0usize;
    let mut current_angle = start_angle;
    while current_angle < arc_end_angle && index < MAX_DECOMPOSITION_STEPS {
        let segment_length = strokes[index % wrap];
        let theta = std::f64::consts::TAU * segment_length / circumference;
        let next_angle = (current_angle + theta).min(arc_end_angle);
        if index.is_multiple_of(2) {
            let subdivide = style == StrokeStyle::Dash
                || (matches!(style, StrokeStyle::DashDot | StrokeStyle::DashDotDot)
                    && index.is_multiple_of(wrap));
            if subdivide {
                let mut low = current_angle;
                while low < next_angle {
                    let high = (low + ARC_CHORD_STEP_RADIANS).min(next_angle);
                    push_arc_chord(
                        &mut output,
                        center_x,
                        center_y,
                        radius,
                        low,
                        high,
                        max_operations,
                    )?;
                    low = high;
                }
            } else {
                push_arc_chord(
                    &mut output,
                    center_x,
                    center_y,
                    radius,
                    current_angle,
                    next_angle,
                    max_operations,
                )?;
            }
        }
        current_angle = next_angle;
        index += 1;
    }
    Ok(output)
}

fn arc_center_radius(start: [i64; 2], mid: [i64; 2], end: [i64; 2]) -> Option<(f64, f64, f64)> {
    let start_x = start[0] as f64;
    let start_y = start[1] as f64;
    let mid_x = mid[0] as f64;
    let mid_y = mid[1] as f64;
    let end_x = end[0] as f64;
    let end_y = end[1] as f64;
    let a_x = mid_x - start_x;
    let a_y = mid_y - start_y;
    let b_x = end_x - mid_x;
    let b_y = end_y - mid_y;
    let denominator = 2.0 * (a_x * b_y - a_y * b_x);
    if denominator.abs() < 1e-9 {
        return None;
    }
    let start_squared = start_x * start_x + start_y * start_y;
    let mid_squared = mid_x * mid_x + mid_y * mid_y;
    let end_squared = end_x * end_x + end_y * end_y;
    let center_x = (start_squared * (mid_y - end_y)
        + mid_squared * (end_y - start_y)
        + end_squared * (start_y - mid_y))
        / denominator;
    let center_y = (start_squared * (end_x - mid_x)
        + mid_squared * (start_x - end_x)
        + end_squared * (mid_x - start_x))
        / denominator;
    Some((
        center_x,
        center_y,
        (start_x - center_x).hypot(start_y - center_y),
    ))
}

fn normalize_arc_sweep(start: f64, mid: f64, end: f64) -> (f64, f64) {
    let tau = std::f64::consts::TAU;
    let start = start.rem_euclid(tau);
    let mid = mid.rem_euclid(tau);
    let end = end.rem_euclid(tau);
    let counter_clockwise_end = (end - start).rem_euclid(tau);
    let counter_clockwise_mid = (mid - start).rem_euclid(tau);
    if counter_clockwise_end == 0.0 {
        return (start, start + tau);
    }
    if counter_clockwise_mid > 0.0 && counter_clockwise_mid < counter_clockwise_end {
        return (start, start + counter_clockwise_end);
    }
    (end, end + (tau - counter_clockwise_end))
}

fn push_arc_chord(
    output: &mut Vec<[i64; 4]>,
    center_x: f64,
    center_y: f64,
    radius: f64,
    low: f64,
    high: f64,
    max_operations: usize,
) -> Result<(), Error> {
    push_decomposed_segment(
        output,
        [
            rounded_safe_f64(center_x + radius * low.cos())?,
            rounded_safe_f64(center_y + radius * low.sin())?,
            rounded_safe_f64(center_x + radius * high.cos())?,
            rounded_safe_f64(center_y + radius * high.sin())?,
        ],
        max_operations,
    )
}

fn push_decomposed_segment(
    output: &mut Vec<[i64; 4]>,
    segment: [i64; 4],
    max_operations: usize,
) -> Result<(), Error> {
    if output.len() >= max_operations {
        return Err(limit_error());
    }
    output.push(segment);
    Ok(())
}

fn rounded_safe_f64(value: f64) -> Result<i64, Error> {
    if !value.is_finite()
        || value < JAVASCRIPT_SAFE_INTEGER_MIN as f64
        || value > JAVASCRIPT_SAFE_INTEGER_MAX as f64
    {
        return Err(model_error(
            "Graphic coordinate exceeds JavaScript safe-integer range",
            Position::START,
        ));
    }
    Ok(value.round_ties_even() as i64)
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
    if !scaled.is_finite()
        || scaled < JAVASCRIPT_SAFE_INTEGER_MIN as f64
        || scaled > JAVASCRIPT_SAFE_INTEGER_MAX as f64
    {
        return Err(model_error(
            "Coordinate exceeds JavaScript safe-integer range",
            Position::START,
        ));
    }
    Ok(scaled.round_ties_even() as i64)
}

fn ensure_javascript_safe_integer(value: i64) -> Result<i64, Error> {
    if (JAVASCRIPT_SAFE_INTEGER_MIN..=JAVASCRIPT_SAFE_INTEGER_MAX).contains(&value) {
        Ok(value)
    } else {
        Err(model_error(
            "Footprint version exceeds JavaScript safe-integer range",
            Position::START,
        ))
    }
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

fn metadata_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Footprint plotter metadata exceeds max_metadata_forms",
        Position::START,
    )
}

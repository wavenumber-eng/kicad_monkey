//! Shared plotter operations and the source-selected footprint producer.

use crate::footprint::{FootprintLimits, FootprintView};
use crate::footprint_plotter_text::footprint_text_operations;
pub use crate::plotter_types::*;
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Lexer, Position, Sexp, TokenKind, parse};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::ops::Range;
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

/// Limits for the footprint plotter operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FootprintPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_metadata_forms: usize,
    pub max_text_carriers: usize,
    pub max_text_bytes: usize,
    pub max_operations: usize,
    pub max_points: usize,
}

impl Default for FootprintPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_metadata_forms: 256,
            max_text_carriers: 100_000,
            max_text_bytes: 16 * 1024 * 1024,
            max_operations: 100_000,
            max_points: 1_000_000,
        }
    }
}

/// Typed facts needed to serialize the promoted footprint plotter subset.
#[derive(Clone, Debug, PartialEq)]
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
    pub operations: Vec<PlotterOperation>,
}

#[derive(Default)]
struct FootprintGeometrySpans {
    lines: Vec<FormSpan>,
    arcs: Vec<FormSpan>,
    circles: Vec<FormSpan>,
    rects: Vec<FormSpan>,
    polys: Vec<FormSpan>,
    pads: Vec<FormSpan>,
}

/// Read supported footprint geometry directly from selected forms.
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "pre-standard source-selection orchestrator retained under the structural ratchet"
)]
pub fn footprint_plot_document(
    source: &str,
    limits: FootprintPlotLimits,
) -> Result<FootprintPlotDocument, Error> {
    let footprint_limits = FootprintLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_properties: limits.max_text_carriers,
        max_texts: limits.max_text_carriers,
        max_text_boxes: limits.max_text_carriers,
        max_text_carriers: limits.max_text_carriers,
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
        "solder_mask_margin",
        "fp_line",
        "fp_arc",
        "fp_circle",
        "fp_rect",
        "fp_poly",
        "pad",
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
    let mut solder_mask_margin = None;
    let mut metadata_forms = 0usize;
    let mut geometry = FootprintGeometrySpans::default();
    for span in spans {
        if !matches!(
            span.head.as_deref(),
            Some("fp_line" | "fp_arc" | "fp_circle" | "fp_rect" | "fp_poly" | "pad")
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
            Some("solder_mask_margin") if solder_mask_margin.is_none() => {
                solder_mask_margin = Some(form_number(source, &span, "solder_mask_margin")?);
            }
            Some("fp_line") => geometry.lines.push(span),
            Some("fp_arc") => geometry.arcs.push(span),
            Some("fp_circle") => geometry.circles.push(span),
            Some("fp_rect") => geometry.rects.push(span),
            Some("fp_poly") => geometry.polys.push(span),
            Some("pad") => geometry.pads.push(span),
            _ => {}
        }
    }
    let text_operations = footprint_text_operations(&view, limits)?;
    let operations = build_geometry_operations(
        source,
        geometry,
        solder_mask_margin.unwrap_or(0.0),
        text_operations,
        limits,
    )?;
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

fn build_geometry_operations(
    source: &str,
    geometry: FootprintGeometrySpans,
    footprint_mask_margin: f64,
    mut operations: Vec<PlotterOperation>,
    limits: FootprintPlotLimits,
) -> Result<Vec<PlotterOperation>, Error> {
    let mut point_count = 0usize;
    for span in geometry.lines {
        let remaining = remaining_operations(&operations, limits)?;
        let additions = parse_line(source, &span, remaining)?;
        append_bounded_operations(&mut operations, additions, limits, &mut point_count)?;
    }
    for span in geometry.arcs {
        let remaining = remaining_operations(&operations, limits)?;
        let additions = parse_arc(source, &span, remaining)?;
        append_bounded_operations(&mut operations, additions, limits, &mut point_count)?;
    }
    for span in geometry.circles {
        append_bounded_operations(
            &mut operations,
            vec![parse_circle(source, &span)?],
            limits,
            &mut point_count,
        )?;
    }
    for span in geometry.rects {
        append_bounded_operations(
            &mut operations,
            vec![parse_rect(source, &span)?],
            limits,
            &mut point_count,
        )?;
    }
    for span in geometry.polys {
        let remaining_points = limits.max_points.saturating_sub(point_count);
        append_bounded_operations(
            &mut operations,
            vec![parse_poly(source, &span, remaining_points)?],
            limits,
            &mut point_count,
        )?;
    }
    for span in geometry.pads {
        let remaining = limits.max_operations.saturating_sub(operations.len());
        let remaining_points = limits.max_points.saturating_sub(point_count);
        let additions = parse_pad(
            source,
            &span,
            footprint_mask_margin,
            0.0,
            remaining,
            remaining_points,
        )?
        .into_operations();
        append_bounded_operations(&mut operations, additions, limits, &mut point_count)?;
    }
    Ok(operations)
}

/// Footprint graphic conversion shared by standalone and board-embedded
/// producers. The source range is already bounded and owned by a typed PCB
/// view; reparsing just the selected form avoids rebuilding a second tree.
pub(crate) fn footprint_graphic_operations_from_range(
    source: &str,
    range: Range<usize>,
    head: &str,
    max_operations: usize,
    max_points: usize,
    max_depth: usize,
    max_nodes: usize,
) -> Result<Vec<PlotterOperation>, Error> {
    validate_selected_form(source, range.clone(), max_depth, max_nodes)?;
    let span = selected_form_span(range, head);
    match head {
        "fp_line" => parse_line(source, &span, max_operations),
        "fp_arc" => parse_arc(source, &span, max_operations),
        "fp_circle" => Ok(vec![parse_circle(source, &span)?]),
        "fp_rect" => Ok(vec![parse_rect(source, &span)?]),
        "fp_poly" => Ok(vec![parse_poly(source, &span, max_points)?]),
        _ => Err(model_error(
            "Unsupported footprint graphic",
            Position::START,
        )),
    }
}

#[derive(Debug, Default)]
pub(crate) struct FootprintPadOperations {
    pub flash: Vec<PlotterOperation>,
    pub drill: Vec<PlotterOperation>,
}

#[derive(Clone, Copy)]
pub(crate) struct FootprintPadParseLimits {
    pub max_operations: usize,
    pub max_points: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl FootprintPadOperations {
    fn into_operations(mut self) -> Vec<PlotterOperation> {
        self.flash.append(&mut self.drill);
        self.flash
    }
}

/// Pad conversion shared by standalone and board-embedded footprint
/// producers. Embedded footprints pass the negative placement angle so pad
/// flashes retain Python's footprint-local orientation convention.
pub(crate) fn footprint_pad_operations_from_range(
    source: &str,
    range: Range<usize>,
    footprint_mask_margin_mm: f64,
    orient_deg_offset: f64,
    limits: FootprintPadParseLimits,
) -> Result<FootprintPadOperations, Error> {
    validate_selected_form(source, range.clone(), limits.max_depth, limits.max_nodes)?;
    parse_pad(
        source,
        &selected_form_span(range, "pad"),
        footprint_mask_margin_mm,
        orient_deg_offset,
        limits.max_operations,
        limits.max_points,
    )
}

fn validate_selected_form(
    source: &str,
    range: Range<usize>,
    max_depth: usize,
    max_nodes: usize,
) -> Result<(), Error> {
    let selected = source
        .get(range)
        .ok_or_else(|| model_error("Footprint child span is outside source", Position::START))?;
    crate::sexpr::parse_with_limits(
        selected,
        crate::sexpr::Limits {
            max_source_bytes: selected.len(),
            max_depth,
            max_nodes,
            max_decoded_string_bytes: selected.len(),
        },
    )?;
    Ok(())
}

fn selected_form_span(range: Range<usize>, head: &str) -> FormSpan {
    FormSpan {
        head: Some(head.to_owned()),
        path: vec![head.to_owned()],
        depth: 0,
        range,
        start: Position::START,
        end: Position::START,
    }
}

fn parse_line(
    source: &str,
    span: &FormSpan,
    max_operations: usize,
) -> Result<Vec<PlotterOperation>, Error> {
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
        layer: Some(layer.clone()),
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
    };
    if matches!(stroke.style, StrokeStyle::Default | StrokeStyle::Solid) {
        return Ok(vec![PlotterOperation::ThickSegment(solid)]);
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
        return Ok(vec![PlotterOperation::ThickSegment(solid)]);
    }
    Ok(pieces
        .into_iter()
        .map(|[start_x, start_y, end_x, end_y]| {
            PlotterOperation::ThickSegment(ThickSegment {
                start_x,
                start_y,
                end_x,
                end_y,
                width_nm: stroke.width_nm,
                layer: Some(layer.clone()),
                role: None,
                layers: Vec::new(),
                mask_margin_nm: None,
                pad_size_x_nm: None,
                pad_size_y_nm: None,
            })
        })
        .collect())
}

fn parse_arc(
    source: &str,
    span: &FormSpan,
    max_operations: usize,
) -> Result<Vec<PlotterOperation>, Error> {
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
        return Ok(vec![PlotterOperation::ArcThreePoint(ArcThreePoint {
            start_x,
            start_y,
            mid_x,
            mid_y,
            end_x,
            end_y,
            fill: PlotterFill::NoFill,
            width_nm: stroke.width_nm,
            layer: Some(layer),
            stroke_color: None,
            fill_color: None,
            line_style: None,
        })]);
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
        return Ok(vec![PlotterOperation::ArcThreePoint(ArcThreePoint {
            start_x,
            start_y,
            mid_x,
            mid_y,
            end_x,
            end_y,
            fill: PlotterFill::NoFill,
            width_nm: stroke.width_nm,
            layer: Some(layer),
            stroke_color: None,
            fill_color: None,
            line_style: None,
        })]);
    }
    Ok(pieces
        .into_iter()
        .map(|[start_x, start_y, end_x, end_y]| {
            PlotterOperation::ThickSegment(ThickSegment {
                start_x,
                start_y,
                end_x,
                end_y,
                width_nm: stroke.width_nm,
                layer: Some(layer.clone()),
                role: None,
                layers: Vec::new(),
                mask_margin_nm: None,
                pad_size_x_nm: None,
                pad_size_y_nm: None,
            })
        })
        .collect())
}

fn parse_circle(source: &str, span: &FormSpan) -> Result<PlotterOperation, Error> {
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
    Ok(PlotterOperation::Circle(PlotterCircle {
        cx: mm_to_nm(center_x_mm)?,
        cy: mm_to_nm(center_y_mm)?,
        diameter_nm: mm_to_nm(2.0 * (end_x_mm - center_x_mm).hypot(end_y_mm - center_y_mm))?,
        fill: graphic_fill(&form),
        width_nm: stroke.width_nm,
        layer: Some(graphic_layer(&form)),
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    }))
}

fn parse_rect(source: &str, span: &FormSpan) -> Result<PlotterOperation, Error> {
    let form = parse_span(source, span)?;
    let start =
        child(&form, "start").ok_or_else(|| model_error("fp_rect requires start", span.start))?;
    let end = child(&form, "end").ok_or_else(|| model_error("fp_rect requires end", span.start))?;
    let stroke = parse_stroke(&form, span.start)?;
    Ok(PlotterOperation::Rect(PlotterRect {
        x1: mm_to_nm(numeric_at(start, 1, span.start)?)?,
        y1: mm_to_nm(numeric_at(start, 2, span.start)?)?,
        x2: mm_to_nm(numeric_at(end, 1, span.start)?)?,
        y2: mm_to_nm(numeric_at(end, 2, span.start)?)?,
        fill: graphic_fill(&form),
        width_nm: stroke.width_nm,
        corner_radius_nm: 0,
        layer: Some(graphic_layer(&form)),
        stroke_color: None,
        fill_color: None,
        line_style: None,
    }))
}

fn parse_poly(source: &str, span: &FormSpan, max_points: usize) -> Result<PlotterOperation, Error> {
    let form = parse_span(source, span)?;
    let points_form =
        child(&form, "pts").ok_or_else(|| model_error("fp_poly requires pts", span.start))?;
    let point_forms = list(points_form)
        .ok_or_else(|| model_error("fp_poly pts must be a list", span.start))?
        .iter()
        .skip(1)
        .filter(|value| {
            list(value)
                .and_then(|items| items.first())
                .and_then(sexp_text)
                == Some("xy")
        });
    let mut points = Vec::new();
    for value in point_forms {
        if points.len() >= max_points {
            return Err(geometry_limit_error());
        }
        points.push([
            mm_to_nm(numeric_at(value, 1, span.start)?)?,
            mm_to_nm(numeric_at(value, 2, span.start)?)?,
        ]);
    }
    let stroke = parse_stroke(&form, span.start)?;
    Ok(PlotterOperation::PlotPoly(PlotterPoly {
        points,
        fill: graphic_fill(&form),
        width_nm: stroke.width_nm,
        layer: Some(graphic_layer(&form)),
        stroke_color: None,
        fill_color: None,
        line_style: None,
    }))
}

#[derive(Clone, Copy, Debug, Default)]
struct PadDrill {
    oval: bool,
    diameter_mm: Option<f64>,
    width_mm: Option<f64>,
    height_mm: Option<f64>,
    offset_x_mm: f64,
    offset_y_mm: f64,
}

#[derive(Clone, Copy, Debug)]
struct PadOperationContext<'a> {
    x: i64,
    y: i64,
    orient_deg: f64,
    size_x_nm: i64,
    size_y_nm: i64,
    pad_type: &'a str,
    layers: &'a [String],
    mask_margin_nm: i64,
}

#[allow(
    clippy::too_many_lines,
    reason = "pre-standard pad dispatch retained under the structural ratchet"
)]
fn parse_pad(
    source: &str,
    span: &FormSpan,
    footprint_mask_margin_mm: f64,
    orient_deg_offset: f64,
    max_operations: usize,
    max_points: usize,
) -> Result<FootprintPadOperations, Error> {
    let form = parse_span(source, span)?;
    let values = list(&form).ok_or_else(|| model_error("pad must be a list", span.start))?;
    let pad_type = values.get(2).and_then(sexp_text).unwrap_or("");
    let shape = values.get(3).and_then(sexp_text).unwrap_or("");
    let at = child(&form, "at");
    let x = mm_to_nm(optional_numeric_at(at, 1, 0.0, span.start)?)?;
    let y = mm_to_nm(optional_numeric_at(at, 2, 0.0, span.start)?)?;
    let orient_deg = finite_number(
        optional_numeric_at(at, 3, 0.0, span.start)? + orient_deg_offset,
        span.start,
    )?;
    let size = child(&form, "size");
    let size_x_mm = optional_numeric_at(size, 1, 0.0, span.start)?;
    let size_y_mm = optional_numeric_at(size, 2, 0.0, span.start)?;
    let size_x_nm = mm_to_nm(size_x_mm)?;
    let size_y_nm = mm_to_nm(size_y_mm)?;
    let layers = child(&form, "layers")
        .and_then(list)
        .map(|values| {
            values
                .iter()
                .skip(1)
                .filter_map(sexp_text)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let requested_mask_margin = child(&form, "solder_mask_margin")
        .map(|value| numeric_at(value, 1, span.start))
        .transpose()?
        .unwrap_or(footprint_mask_margin_mm);
    let mask_margin_nm = mm_to_nm(requested_mask_margin.max(-size_x_mm.min(size_y_mm) / 2.0))?;
    let drill = parse_pad_drill(&form, span.start)?;
    let suppress_npth_flash = pad_type == "np_thru_hole"
        && shape == "circle"
        && drill
            .diameter_mm
            .is_some_and(|diameter| size_x_mm.max(size_y_mm) <= diameter);
    let context = PadOperationContext {
        x,
        y,
        orient_deg,
        size_x_nm,
        size_y_nm,
        pad_type,
        layers: &layers,
        mask_margin_nm,
    };

    let mut output = FootprintPadOperations::default();
    if !suppress_npth_flash {
        let flash = match shape {
            "circle" => Some(PlotterOperation::FlashPadCircle(FlashPadCircle {
                x,
                y,
                diameter_nm: size_x_nm,
                layers: layers.clone(),
                mask_margin_nm,
            })),
            "oval" => Some(PlotterOperation::FlashPadOval(FlashPadOval {
                x,
                y,
                size_x_nm,
                size_y_nm,
                orient_deg,
                layers: layers.clone(),
                mask_margin_nm,
            })),
            "rect" => Some(PlotterOperation::FlashPadRect(FlashPadRect {
                x,
                y,
                size_x_nm,
                size_y_nm,
                orient_deg,
                layers: layers.clone(),
                mask_margin_nm,
            })),
            "roundrect" => Some(parse_roundrect_flash(
                &form, span.start, context, max_points,
            )?),
            "custom" => Some(parse_custom_pad_flash(
                &form, span.start, context, max_points,
            )?),
            "trapezoid" => {
                let delta = child(&form, "rect_delta");
                let delta_x = mm_to_nm(optional_numeric_at(delta, 1, 0.0, span.start)?)? / 2;
                let delta_y = mm_to_nm(optional_numeric_at(delta, 2, 0.0, span.start)?)? / 2;
                let half_x = size_x_nm / 2;
                let half_y = size_y_nm / 2;
                Some(PlotterOperation::FlashPadTrapez(FlashPadTrapez {
                    x,
                    y,
                    size_x_nm,
                    size_y_nm,
                    corners: [
                        [
                            checked_safe_add(-half_x, -delta_y)?,
                            checked_safe_add(half_y, delta_x)?,
                        ],
                        [
                            checked_safe_add(half_x, delta_y)?,
                            checked_safe_add(half_y, -delta_x)?,
                        ],
                        [
                            checked_safe_add(half_x, -delta_y)?,
                            checked_safe_add(-half_y, delta_x)?,
                        ],
                        [
                            checked_safe_add(-half_x, delta_y)?,
                            checked_safe_add(-half_y, -delta_x)?,
                        ],
                    ],
                    orient_deg,
                    layers: layers.clone(),
                    mask_margin_nm,
                }))
            }
            _ => None,
        };
        if let Some(flash) = flash {
            push_plotter_operation(&mut output.flash, flash, max_operations)?;
        }
    }

    if matches!(pad_type, "thru_hole" | "np_thru_hole")
        && let Some(drill_operation) = pad_drill_operation(context, drill)?
    {
        if output.flash.len() >= max_operations {
            return Err(limit_error());
        }
        push_plotter_operation(
            &mut output.drill,
            drill_operation,
            max_operations - output.flash.len(),
        )?;
    }
    Ok(output)
}

fn parse_roundrect_flash(
    form: &Sexp,
    position: Position,
    context: PadOperationContext<'_>,
    max_points: usize,
) -> Result<PlotterOperation, Error> {
    let round_ratio = child(form, "roundrect_rratio")
        .map(|value| numeric_at(value, 1, position))
        .transpose()?
        .unwrap_or(0.25);
    let corner_radius_nm =
        rounded_safe_f64(context.size_x_nm.min(context.size_y_nm) as f64 * round_ratio)?;
    let chamfer_ratio = child(form, "chamfer_ratio")
        .map(|value| numeric_at(value, 1, position))
        .transpose()?
        .unwrap_or(0.0);
    let chamfer_corners = child(form, "chamfer")
        .and_then(list)
        .map(|values| {
            values
                .iter()
                .skip(1)
                .filter_map(sexp_text)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !chamfer_corners.is_empty() && chamfer_ratio > 0.0 && corner_radius_nm < 1 {
        let polygon = chamfered_pad_polygon(
            context.size_x_nm,
            context.size_y_nm,
            chamfer_ratio,
            &chamfer_corners,
        )?;
        if polygon.len() > max_points {
            return Err(geometry_limit_error());
        }
        return Ok(PlotterOperation::FlashPadCustom(FlashPadCustom {
            x: context.x,
            y: context.y,
            size_x_nm: context.size_x_nm,
            size_y_nm: context.size_y_nm,
            orient_deg: context.orient_deg,
            polygons: vec![polygon],
            polygon_widths_nm: None,
            anchor_shape: None,
            layers: context.layers.to_vec(),
            mask_margin_nm: context.mask_margin_nm,
        }));
    }
    Ok(PlotterOperation::FlashPadRoundRect(FlashPadRoundRect {
        x: context.x,
        y: context.y,
        size_x_nm: context.size_x_nm,
        size_y_nm: context.size_y_nm,
        corner_radius_nm,
        orient_deg: context.orient_deg,
        layers: context.layers.to_vec(),
        mask_margin_nm: context.mask_margin_nm,
    }))
}

fn parse_custom_pad_flash(
    form: &Sexp,
    position: Position,
    context: PadOperationContext<'_>,
    max_points: usize,
) -> Result<PlotterOperation, Error> {
    let mut polygons = Vec::new();
    let mut polygon_widths_nm = Vec::new();
    let mut point_count = 0usize;
    if let Some(primitives) = child(form, "primitives").and_then(list) {
        for primitive in primitives.iter().skip(1) {
            if value_at(primitive, 0) != Some("gr_poly") {
                continue;
            }
            let Some(points) = child(primitive, "pts").and_then(list) else {
                continue;
            };
            let mut polygon = Vec::new();
            for point in points
                .iter()
                .skip(1)
                .filter(|point| value_at(point, 0) == Some("xy"))
            {
                if point_count >= max_points {
                    return Err(geometry_limit_error());
                }
                polygon.push([
                    mm_to_nm(numeric_at(point, 1, position)?)?,
                    mm_to_nm(numeric_at(point, 2, position)?)?,
                ]);
                point_count = point_count.saturating_add(1);
            }
            if polygon.is_empty() {
                continue;
            }
            polygons.push(polygon);
            let width_mm = child(primitive, "width")
                .map(|width| numeric_at(width, 1, position))
                .transpose()?
                .unwrap_or(0.0);
            polygon_widths_nm.push(mm_to_nm(width_mm)?);
        }
    }
    let anchor_shape = child(form, "options")
        .and_then(|options| child(options, "anchor"))
        .and_then(|anchor| value_at(anchor, 1))
        .filter(|anchor| !anchor.is_empty())
        .map(str::to_owned);
    Ok(PlotterOperation::FlashPadCustom(FlashPadCustom {
        x: context.x,
        y: context.y,
        size_x_nm: context.size_x_nm,
        size_y_nm: context.size_y_nm,
        orient_deg: context.orient_deg,
        polygons,
        polygon_widths_nm: Some(polygon_widths_nm),
        anchor_shape,
        layers: context.layers.to_vec(),
        mask_margin_nm: context.mask_margin_nm,
    }))
}

fn chamfered_pad_polygon(
    size_x_nm: i64,
    size_y_nm: i64,
    chamfer_ratio: f64,
    chamfer_corners: &[&str],
) -> Result<Vec<[i64; 2]>, Error> {
    let half_width = size_x_nm as f64 / 2.0;
    let half_height = size_y_nm as f64 / 2.0;
    let shorter = size_x_nm.min(size_y_nm) as f64;
    let chamfer = (chamfer_ratio * shorter).max(0.0);
    let mut corners = vec![
        [-half_width, -half_height],
        [half_width, -half_height],
        [half_width, half_height],
        [-half_width, half_height],
    ];
    if chamfer > 0.0 {
        let corner_names = ["top_left", "top_right", "bottom_right", "bottom_left"];
        let sign = [0.0, 1.0, -1.0, 0.0, 0.0, -1.0, 1.0, 0.0];
        let chamfer_count = corner_names
            .iter()
            .filter(|name| chamfer_corners.contains(name))
            .count();
        let mut position = 0;
        for (corner_index, name) in corner_names.iter().enumerate() {
            if !chamfer_corners.contains(name) {
                position += 1;
                continue;
            }
            corners.insert(position + 1, corners[position]);
            corners[position][0] += sign[(2 * corner_index) & 7] * chamfer;
            corners[position][1] += sign[(2 * corner_index + 6) & 7] * chamfer;
            corners[position + 1][0] += sign[(2 * corner_index + 1) & 7] * chamfer;
            corners[position + 1][1] += sign[(2 * corner_index + 7) & 7] * chamfer;
            position += 2;
        }
        if chamfer_count > 1 && 2.0 * chamfer >= shorter {
            corners.dedup_by(|right, left| {
                (right[0] - left[0]).abs() <= 1e-9 && (right[1] - left[1]).abs() <= 1e-9
            });
            if corners.len() > 1 {
                let first = corners[0];
                let last = corners[corners.len() - 1];
                if (first[0] - last[0]).abs() < 1e-9 && (first[1] - last[1]).abs() < 1e-9 {
                    corners.pop();
                }
            }
        }
    }
    corners
        .into_iter()
        .map(|[x, y]| Ok([rounded_safe_f64(x)?, rounded_safe_f64(y)?]))
        .collect()
}

fn parse_pad_drill(form: &Sexp, position: Position) -> Result<PadDrill, Error> {
    let Some(drill) = child(form, "drill") else {
        return Ok(PadDrill::default());
    };
    let offset = child(drill, "offset");
    let offset_x_mm = optional_numeric_at(offset, 1, 0.0, position)?;
    let offset_y_mm = optional_numeric_at(offset, 2, 0.0, position)?;
    if value_at(drill, 1) == Some("oval") {
        let width_mm = optional_numeric(drill, 2, position)?;
        let height_mm = optional_numeric(drill, 3, position)?;
        return Ok(PadDrill {
            oval: true,
            diameter_mm: width_mm,
            width_mm,
            height_mm,
            offset_x_mm,
            offset_y_mm,
        });
    }
    Ok(PadDrill {
        diameter_mm: optional_numeric(drill, 1, position)?,
        offset_x_mm,
        offset_y_mm,
        ..PadDrill::default()
    })
}

fn pad_drill_operation(
    context: PadOperationContext<'_>,
    drill: PadDrill,
) -> Result<Option<PlotterOperation>, Error> {
    let role = if context.pad_type == "np_thru_hole" {
        "npth_hole"
    } else {
        "pad_drill"
    };
    let offset_x_nm = mm_to_nm(drill.offset_x_mm)?;
    let offset_y_nm = mm_to_nm(drill.offset_y_mm)?;
    let [offset_x_nm, offset_y_nm] = rotate_nm(offset_x_nm, offset_y_nm, context.orient_deg)?;
    let center = [
        checked_safe_add(context.x, offset_x_nm)?,
        checked_safe_add(context.y, offset_y_nm)?,
    ];

    if drill.oval
        && let (Some(width_mm), Some(height_mm)) = (drill.width_mm, drill.height_mm)
        && width_mm > 0.0
        && height_mm > 0.0
    {
        return drill_slot_operation(
            context,
            center,
            mm_to_nm(width_mm)?,
            mm_to_nm(height_mm)?,
            role,
        );
    }
    if let Some(diameter_mm) = drill.diameter_mm.filter(|value| *value > 0.0) {
        return Ok(Some(drill_circle_operation(
            context,
            center,
            mm_to_nm(diameter_mm)?,
            role,
        )));
    }
    if context.pad_type == "np_thru_hole" {
        let fallback = context.size_x_nm.min(context.size_y_nm);
        if fallback > 0 {
            return Ok(Some(drill_circle_operation(
                context, center, fallback, role,
            )));
        }
    }
    Ok(None)
}

fn drill_circle_operation(
    context: PadOperationContext<'_>,
    [cx, cy]: [i64; 2],
    diameter_nm: i64,
    role: &str,
) -> PlotterOperation {
    let npth = role == "npth_hole";
    PlotterOperation::Circle(PlotterCircle {
        cx,
        cy,
        diameter_nm,
        fill: PlotterFill::FilledShape,
        width_nm: 0,
        layer: None,
        role: Some(role.to_owned()),
        layers: context.layers.to_vec(),
        mask_margin_nm: npth.then_some(context.mask_margin_nm),
        pad_size_x_nm: npth.then_some(context.size_x_nm),
        pad_size_y_nm: npth.then_some(context.size_y_nm),
        stroke_color: None,
        fill_color: None,
        line_style: None,
    })
}

fn drill_slot_operation(
    context: PadOperationContext<'_>,
    [cx, cy]: [i64; 2],
    width_nm: i64,
    height_nm: i64,
    role: &str,
) -> Result<Option<PlotterOperation>, Error> {
    let major = width_nm.max(height_nm);
    let minor = width_nm.min(height_nm);
    if major <= 0 || minor <= 0 {
        return Ok(None);
    }
    let mut theta = (-context.orient_deg).to_radians();
    if height_nm > width_nm {
        theta += std::f64::consts::FRAC_PI_2;
    }
    let half_length = (major - minor) as f64 / 2.0;
    let dx = rounded_safe_f64(theta.cos() * half_length)?;
    let dy = rounded_safe_f64(theta.sin() * half_length)?;
    if dx == 0 && dy == 0 {
        return Ok(Some(drill_circle_operation(context, [cx, cy], minor, role)));
    }
    Ok(Some(PlotterOperation::ThickSegment(ThickSegment {
        start_x: checked_safe_add(cx, -dx)?,
        start_y: checked_safe_add(cy, -dy)?,
        end_x: checked_safe_add(cx, dx)?,
        end_y: checked_safe_add(cy, dy)?,
        width_nm: minor,
        layer: None,
        role: Some(role.to_owned()),
        layers: context.layers.to_vec(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
    })))
}

fn rotate_nm(x: i64, y: i64, orient_deg: f64) -> Result<[i64; 2], Error> {
    if orient_deg == 0.0 {
        return Ok([x, y]);
    }
    let theta = (-orient_deg).to_radians();
    Ok([
        rounded_safe_f64(x as f64 * theta.cos() - y as f64 * theta.sin())?,
        rounded_safe_f64(x as f64 * theta.sin() + y as f64 * theta.cos())?,
    ])
}

fn checked_safe_add(left: i64, right: i64) -> Result<i64, Error> {
    left.checked_add(right)
        .and_then(|value| ensure_javascript_safe_integer(value).ok())
        .ok_or_else(|| {
            model_error(
                "Pad coordinate exceeds JavaScript safe-integer range",
                Position::START,
            )
        })
}

fn push_plotter_operation(
    output: &mut Vec<PlotterOperation>,
    operation: PlotterOperation,
    max_operations: usize,
) -> Result<(), Error> {
    if output.len() >= max_operations {
        return Err(limit_error());
    }
    output.push(operation);
    Ok(())
}

fn append_operations(
    operations: &mut Vec<PlotterOperation>,
    additions: Vec<PlotterOperation>,
    max_operations: usize,
) -> Result<(), Error> {
    if additions.len() > max_operations.saturating_sub(operations.len()) {
        return Err(limit_error());
    }
    operations.extend(additions);
    Ok(())
}

fn append_bounded_operations(
    operations: &mut Vec<PlotterOperation>,
    additions: Vec<PlotterOperation>,
    limits: FootprintPlotLimits,
    point_count: &mut usize,
) -> Result<(), Error> {
    let added_points = additions
        .iter()
        .map(operation_point_count)
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(geometry_limit_error)?;
    if added_points > limits.max_points.saturating_sub(*point_count) {
        return Err(geometry_limit_error());
    }
    append_operations(operations, additions, limits.max_operations)?;
    *point_count = point_count.saturating_add(added_points);
    Ok(())
}

fn operation_point_count(operation: &PlotterOperation) -> usize {
    match operation {
        PlotterOperation::PlotPoly(value) => value.points.len(),
        PlotterOperation::FlashPadCustom(value) => {
            value.polygons.iter().map(Vec::len).sum::<usize>()
        }
        _ => 0,
    }
}

fn remaining_operations(
    operations: &[PlotterOperation],
    limits: FootprintPlotLimits,
) -> Result<usize, Error> {
    limits
        .max_operations
        .checked_sub(operations.len())
        .filter(|remaining| *remaining > 0)
        .ok_or_else(limit_error)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StrokeStyle {
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
    if width_nm <= 0 {
        return Err(decomposition_limit_error(
            "Patterned stroke width cannot make forward progress",
        ));
    }
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

pub(crate) fn decompose_segment(
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
        if !next.is_finite() || next <= current {
            return Err(decomposition_limit_error(
                "Patterned segment decomposition cannot make forward progress",
            ));
        }
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
    if current < total {
        return Err(decomposition_limit_error(
            "Patterned segment decomposition exceeds the safety step limit",
        ));
    }
    Ok(output)
}

pub(crate) fn decompose_arc(
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
        if !theta.is_finite() || theta <= 0.0 {
            return Err(decomposition_limit_error(
                "Patterned arc decomposition cannot make forward progress",
            ));
        }
        let next_angle = (current_angle + theta).min(arc_end_angle);
        if !next_angle.is_finite() || next_angle <= current_angle {
            return Err(decomposition_limit_error(
                "Patterned arc decomposition cannot make forward progress",
            ));
        }
        if index.is_multiple_of(2) {
            let subdivide = style == StrokeStyle::Dash
                || (matches!(style, StrokeStyle::DashDot | StrokeStyle::DashDotDot)
                    && index.is_multiple_of(wrap));
            if subdivide {
                let mut low = current_angle;
                while low < next_angle {
                    let high = (low + ARC_CHORD_STEP_RADIANS).min(next_angle);
                    if !high.is_finite() || high <= low {
                        return Err(decomposition_limit_error(
                            "Patterned arc chord decomposition cannot make forward progress",
                        ));
                    }
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
    if current_angle < arc_end_angle {
        return Err(decomposition_limit_error(
            "Patterned arc decomposition exceeds the safety step limit",
        ));
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

fn form_number(source: &str, span: &FormSpan, head: &str) -> Result<f64, Error> {
    let form = parse_span(source, span)?;
    let values = metadata_values(&form, head, span.start)?;
    let value = values
        .first()
        .ok_or_else(|| model_error("Metadata value is missing", span.start))?;
    finite_number(numeric_value(value, span.start)?, span.start)
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

pub(crate) fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
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

pub(crate) fn value_at(form: &Sexp, index: usize) -> Option<&str> {
    list(form)?.get(index).and_then(sexp_text)
}

pub(crate) fn numeric_at(form: &Sexp, index: usize, position: Position) -> Result<f64, Error> {
    let value = list(form)
        .and_then(|values| values.get(index))
        .ok_or_else(|| model_error("Expected numeric coordinate", position))?;
    numeric_value(value, position)
}

fn optional_numeric(form: &Sexp, index: usize, position: Position) -> Result<Option<f64>, Error> {
    list(form)
        .and_then(|values| values.get(index))
        .map(|value| numeric_value(value, position))
        .transpose()
}

fn optional_numeric_at(
    form: Option<&Sexp>,
    index: usize,
    default: f64,
    position: Position,
) -> Result<f64, Error> {
    form.map(|value| optional_numeric(value, index, position))
        .transpose()
        .map(|value| value.flatten().unwrap_or(default))
}

fn numeric_value(value: &Sexp, position: Position) -> Result<f64, Error> {
    match value {
        Sexp::Integer(value) => Ok(*value as f64),
        Sexp::Float(value) => Ok(*value),
        Sexp::Atom(value) => value
            .parse::<f64>()
            .map_err(|_| model_error("Expected numeric coordinate", position)),
        _ => Err(model_error("Expected numeric coordinate", position)),
    }
}

fn finite_number(value: f64, position: Position) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(model_error("Expected finite numeric value", position))
    }
}

pub(crate) fn mm_to_nm(value: f64) -> Result<i64, Error> {
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

pub(crate) fn ensure_javascript_safe_integer(value: i64) -> Result<i64, Error> {
    if (JAVASCRIPT_SAFE_INTEGER_MIN..=JAVASCRIPT_SAFE_INTEGER_MAX).contains(&value) {
        Ok(value)
    } else {
        Err(model_error(
            "Document version exceeds JavaScript safe-integer range",
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

pub(crate) fn model_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

pub(crate) fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Footprint plotter operation exceeds configured limits",
        Position::START,
    )
}

pub(crate) fn text_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Footprint plotter text exceeds max_text_bytes",
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

fn geometry_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Footprint plotter geometry exceeds max_points",
        Position::START,
    )
}

fn decomposition_limit_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}

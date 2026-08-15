//! Board-level plotter IR producer over the selected PCB graphics family.
//!
//! Mirrors the Python `pcb_to_ir` gr_* record subset: layerless graphic-state
//! operations, per-category record ordering, PCB stroke widths without the
//! footprint pen-width clamps, and dash/dot decomposition parity.

use crate::pcb::{
    PcbFamily, PcbGraphic, PcbGraphicKind, PcbLimits, PcbPoint, PcbSelection, PcbView,
};
use crate::plotter_ir::{
    StrokeStyle, child, decompose_arc, decompose_segment, ensure_javascript_safe_integer, mm_to_nm,
    model_error, numeric_at, value_at,
};
use crate::plotter_types::{
    ArcThreePoint, BezierCurve, PlotterCircle, PlotterFill, PlotterOperation, PlotterPoly,
    PlotterRect, ThickSegment,
};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position, parse};

/// Python `EDGE_CUTS_LAYER` default carried by gr_line/arc/circle/rect/poly.
const DEFAULT_GRAPHIC_LAYER: &str = "Edge.Cuts";
/// Python `FRONT_SILKSCREEN_LAYER` default carried by gr_curve.
const DEFAULT_CURVE_LAYER: &str = "F.SilkS";

/// Limits for one board plotter conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_graphics: usize,
    pub max_operations: usize,
    pub max_points: usize,
}

impl Default for BoardPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_graphics: 100_000,
            max_operations: 100_000,
            max_points: 1_000_000,
        }
    }
}

/// Board graphic record kinds promoted in the first board slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardPlotRecordKind {
    GrLine,
    GrArc,
    GrCircle,
    GrRect,
    GrPoly,
    GrCurve,
}

impl BoardPlotRecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GrLine => "gr_line",
            Self::GrArc => "gr_arc",
            Self::GrCircle => "gr_circle",
            Self::GrRect => "gr_rect",
            Self::GrPoly => "gr_poly",
            Self::GrCurve => "gr_curve",
        }
    }
}

/// One board-level graphic record; the carrier layer travels on the record.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardPlotRecord {
    pub uuid: String,
    pub kind: BoardPlotRecordKind,
    pub layer: String,
    pub operations: Vec<PlotterOperation>,
}

/// Typed facts needed to serialize the promoted board plotter subset.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardPlotDocument {
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
    pub thickness_mm: f64,
    pub paper: String,
    pub records: Vec<BoardPlotRecord>,
}

/// Read supported board graphics into per-category plotter records.
pub fn board_plot_document(
    source: &str,
    limits: BoardPlotLimits,
) -> Result<BoardPlotDocument, Error> {
    let pcb_limits = PcbLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_graphics: limits.max_graphics,
        ..PcbLimits::default()
    };
    let view =
        PcbView::parse_selected(source, pcb_limits, PcbSelection::only(PcbFamily::Graphics))?;
    let metadata = view.metadata()?;
    ensure_javascript_safe_integer(metadata.version)?;

    let mut buckets: [Vec<PcbGraphic>; 6] = Default::default();
    for graphic in view.graphics() {
        let graphic = graphic?;
        // Text carriers are produced by the later board-text slice.
        if let Some(slot) = category_slot(graphic.kind) {
            buckets[slot].push(graphic);
        }
    }

    let kinds = [
        BoardPlotRecordKind::GrLine,
        BoardPlotRecordKind::GrArc,
        BoardPlotRecordKind::GrCircle,
        BoardPlotRecordKind::GrRect,
        BoardPlotRecordKind::GrPoly,
        BoardPlotRecordKind::GrCurve,
    ];
    let mut records = Vec::new();
    let mut operation_count = 0usize;
    let mut point_count = 0usize;
    for (kind, bucket) in kinds.into_iter().zip(buckets) {
        for graphic in bucket {
            let remaining = limits
                .max_operations
                .checked_sub(operation_count)
                .filter(|remaining| *remaining > 0)
                .ok_or_else(limit_error)?;
            let operations = graphic_operations(source, kind, &graphic, remaining)?;
            operation_count = operation_count.saturating_add(operations.len());
            if operation_count > limits.max_operations {
                return Err(limit_error());
            }
            point_count = point_count.saturating_add(poly_point_total(&operations));
            if point_count > limits.max_points {
                return Err(point_limit_error());
            }
            records.push(graphic_record(kind, &graphic, operations));
        }
    }

    Ok(BoardPlotDocument {
        version: metadata.version,
        generator: metadata.generator,
        generator_version: metadata.generator_version,
        thickness_mm: metadata.thickness,
        paper: metadata.paper,
        records,
    })
}

/// Map a promoted graphic to its Python category bucket; text carriers are
/// excluded pending the board-text slice.
fn category_slot(kind: PcbGraphicKind) -> Option<usize> {
    match kind {
        PcbGraphicKind::Line => Some(0),
        PcbGraphicKind::Arc => Some(1),
        PcbGraphicKind::Circle => Some(2),
        PcbGraphicKind::Rect => Some(3),
        PcbGraphicKind::Poly => Some(4),
        PcbGraphicKind::Curve => Some(5),
        PcbGraphicKind::Text | PcbGraphicKind::TextBox => None,
    }
}

fn graphic_operations(
    source: &str,
    kind: BoardPlotRecordKind,
    graphic: &PcbGraphic,
    remaining: usize,
) -> Result<Vec<PlotterOperation>, Error> {
    match kind {
        BoardPlotRecordKind::GrLine => line_operations(source, graphic, remaining),
        BoardPlotRecordKind::GrArc => arc_operations(source, graphic, remaining),
        BoardPlotRecordKind::GrCircle => Ok(vec![circle_operation(source, graphic)?]),
        BoardPlotRecordKind::GrRect => Ok(vec![rect_operation(source, graphic)?]),
        BoardPlotRecordKind::GrPoly => Ok(vec![poly_operation(source, graphic)?]),
        BoardPlotRecordKind::GrCurve => curve_operations(source, graphic),
    }
}

fn poly_point_total(operations: &[PlotterOperation]) -> usize {
    operations
        .iter()
        .map(|operation| match operation {
            PlotterOperation::PlotPoly(value) => value.points.len(),
            _ => 0,
        })
        .sum()
}

fn graphic_record(
    kind: BoardPlotRecordKind,
    graphic: &PcbGraphic,
    operations: Vec<PlotterOperation>,
) -> BoardPlotRecord {
    let default_layer = if kind == BoardPlotRecordKind::GrCurve {
        DEFAULT_CURVE_LAYER
    } else {
        DEFAULT_GRAPHIC_LAYER
    };
    BoardPlotRecord {
        uuid: graphic.uuid.clone().unwrap_or_default(),
        kind,
        layer: graphic
            .layer
            .clone()
            .unwrap_or_else(|| default_layer.to_owned()),
        operations,
    }
}

#[derive(Clone, Copy, Debug)]
struct BoardStroke {
    width_nm: i64,
    style: StrokeStyle,
}

/// Resolve the Python `Stroke` semantics: a `stroke` form wins entirely;
/// without one, the legacy top-level `(width ...)` scalar applies with the
/// default style. PCB widths are unclamped, and non-positive widths plot as 0.
fn resolve_stroke(source: &str, graphic: &PcbGraphic) -> Result<BoardStroke, Error> {
    let text = source
        .get(graphic.source_range.clone())
        .ok_or_else(|| model_error("Board graphic span is out of range", Position::START))?;
    let form = parse(text)?;
    let (width_mm, style_name) = if let Some(stroke) = child(&form, "stroke") {
        let width = match child(stroke, "width") {
            Some(value) => numeric_at(value, 1, Position::START)?,
            None => 0.0,
        };
        (
            width,
            child(stroke, "type")
                .and_then(|value| value_at(value, 1))
                .unwrap_or("default"),
        )
    } else {
        let width = match child(&form, "width") {
            Some(value) => numeric_at(value, 1, Position::START)?,
            None => 0.0,
        };
        (width, "default")
    };
    let style = stroke_style(style_name)?;
    let width_nm = if width_mm <= 0.0 {
        0
    } else {
        mm_to_nm(width_mm)?
    };
    Ok(BoardStroke { width_nm, style })
}

fn stroke_style(style_name: &str) -> Result<StrokeStyle, Error> {
    match style_name {
        "default" => Ok(StrokeStyle::Default),
        "solid" => Ok(StrokeStyle::Solid),
        "dash" => Ok(StrokeStyle::Dash),
        "dot" => Ok(StrokeStyle::Dot),
        "dash_dot" => Ok(StrokeStyle::DashDot),
        "dash_dot_dot" => Ok(StrokeStyle::DashDotDot),
        _ => Err(model_error(
            "Unsupported board graphic stroke type",
            Position::START,
        )),
    }
}

fn board_fill(graphic: &PcbGraphic) -> PlotterFill {
    match graphic.fill.as_deref() {
        Some("yes" | "solid") => PlotterFill::FilledShape,
        _ => PlotterFill::NoFill,
    }
}

fn point_nm(point: Option<PcbPoint>) -> Result<[i64; 2], Error> {
    let point = point.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    Ok([mm_to_nm(point.x)?, mm_to_nm(point.y)?])
}

fn layerless_segment(start: [i64; 2], end: [i64; 2], width_nm: i64) -> PlotterOperation {
    PlotterOperation::ThickSegment(ThickSegment {
        start_x: start[0],
        start_y: start[1],
        end_x: end[0],
        end_y: end[1],
        width_nm,
        layer: None,
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
    })
}

fn line_operations(
    source: &str,
    graphic: &PcbGraphic,
    max_operations: usize,
) -> Result<Vec<PlotterOperation>, Error> {
    let stroke = resolve_stroke(source, graphic)?;
    let start = point_nm(graphic.start)?;
    let end = point_nm(graphic.end)?;
    if matches!(stroke.style, StrokeStyle::Default | StrokeStyle::Solid) {
        return Ok(vec![layerless_segment(start, end, stroke.width_nm)]);
    }
    let pieces = decompose_segment(
        start[0],
        start[1],
        end[0],
        end[1],
        stroke.width_nm,
        stroke.style,
        max_operations,
    )?;
    if pieces.is_empty() {
        return Ok(vec![layerless_segment(start, end, stroke.width_nm)]);
    }
    Ok(pieces
        .into_iter()
        .map(|[start_x, start_y, end_x, end_y]| {
            layerless_segment([start_x, start_y], [end_x, end_y], stroke.width_nm)
        })
        .collect())
}

fn arc_operations(
    source: &str,
    graphic: &PcbGraphic,
    max_operations: usize,
) -> Result<Vec<PlotterOperation>, Error> {
    let stroke = resolve_stroke(source, graphic)?;
    let start = point_nm(graphic.start)?;
    let mid = point_nm(graphic.mid)?;
    let end = point_nm(graphic.end)?;
    let solid = PlotterOperation::ArcThreePoint(ArcThreePoint {
        start_x: start[0],
        start_y: start[1],
        mid_x: mid[0],
        mid_y: mid[1],
        end_x: end[0],
        end_y: end[1],
        fill: PlotterFill::NoFill,
        width_nm: stroke.width_nm,
        layer: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    });
    if matches!(stroke.style, StrokeStyle::Default | StrokeStyle::Solid) {
        return Ok(vec![solid]);
    }
    let pieces = decompose_arc(
        start,
        mid,
        end,
        stroke.width_nm,
        stroke.style,
        max_operations,
    )?;
    if pieces.is_empty() {
        return Ok(vec![solid]);
    }
    Ok(pieces
        .into_iter()
        .map(|[start_x, start_y, end_x, end_y]| {
            layerless_segment([start_x, start_y], [end_x, end_y], stroke.width_nm)
        })
        .collect())
}

fn circle_operation(source: &str, graphic: &PcbGraphic) -> Result<PlotterOperation, Error> {
    let stroke = resolve_stroke(source, graphic)?;
    let center = graphic.center.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    let end = graphic.end.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    Ok(PlotterOperation::Circle(PlotterCircle {
        cx: mm_to_nm(center.x)?,
        cy: mm_to_nm(center.y)?,
        diameter_nm: mm_to_nm(2.0 * (end.x - center.x).hypot(end.y - center.y))?,
        fill: board_fill(graphic),
        width_nm: stroke.width_nm,
        layer: None,
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

fn rect_operation(source: &str, graphic: &PcbGraphic) -> Result<PlotterOperation, Error> {
    let stroke = resolve_stroke(source, graphic)?;
    let start = point_nm(graphic.start)?;
    let end = point_nm(graphic.end)?;
    Ok(PlotterOperation::Rect(PlotterRect {
        x1: start[0],
        y1: start[1],
        x2: end[0],
        y2: end[1],
        fill: board_fill(graphic),
        width_nm: stroke.width_nm,
        corner_radius_nm: 0,
        layer: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    }))
}

fn poly_operation(source: &str, graphic: &PcbGraphic) -> Result<PlotterOperation, Error> {
    let stroke = resolve_stroke(source, graphic)?;
    let points = graphic
        .points
        .iter()
        .map(|point| Ok([mm_to_nm(point.x)?, mm_to_nm(point.y)?]))
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(PlotterOperation::PlotPoly(PlotterPoly {
        points,
        fill: board_fill(graphic),
        width_nm: stroke.width_nm,
        layer: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    }))
}

fn curve_operations(source: &str, graphic: &PcbGraphic) -> Result<Vec<PlotterOperation>, Error> {
    // The stroke is validated before the point-count check because the Python
    // parser rejects unknown stroke types even on malformed curves.
    let stroke = resolve_stroke(source, graphic)?;
    // Python tolerates malformed curves: fewer than four control points
    // produce an empty-operation record.
    if graphic.points.len() < 4 {
        return Ok(Vec::new());
    }
    let points = graphic.points[..4]
        .iter()
        .map(|point| Ok([mm_to_nm(point.x)?, mm_to_nm(point.y)?]))
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(vec![PlotterOperation::BezierCurve(BezierCurve {
        start_x: points[0][0],
        start_y: points[0][1],
        ctrl1_x: points[1][0],
        ctrl1_y: points[1][1],
        ctrl2_x: points[2][0],
        ctrl2_y: points[2][1],
        end_x: points[3][0],
        end_y: points[3][1],
        width_nm: stroke.width_nm,
        tolerance_nm: 0,
        layer: None,
        stroke_color: None,
        line_style: None,
    })])
}

fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter operation exceeds configured limits",
        Position::START,
    )
}

fn point_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter geometry exceeds max_points",
        Position::START,
    )
}

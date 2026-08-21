//! Board gr_line/gr_arc/gr_circle/gr_rect/gr_poly/gr_curve record emission.

use super::{
    BoardGraphicRecord, BoardGraphicRecordKind, BoardPlotLimits, BoardPlotRecord, BudgetTracker,
    layerless_segment, poly_point_total,
};
use crate::pcb::{PcbGraphic, PcbGraphicKind, PcbPoint};
use crate::plotter_ir::{
    StrokeStyle, child, decompose_arc, decompose_segment, mm_to_nm, model_error, numeric_at,
    value_at,
};
use crate::plotter_types::{
    ArcThreePoint, BezierCurve, PlotterCircle, PlotterFill, PlotterOperation, PlotterPoly,
    PlotterRect,
};
use crate::sexpr::{Error, Limits, Position, parse_with_limits};

/// Python `EDGE_CUTS_LAYER` default carried by gr_line/arc/circle/rect/poly.
const DEFAULT_GRAPHIC_LAYER: &str = "Edge.Cuts";
/// Python `FRONT_SILKSCREEN_LAYER` default carried by gr_curve.
const DEFAULT_CURVE_LAYER: &str = "F.SilkS";

pub(super) fn graphic_records(
    source: &str,
    graphics: &[PcbGraphic],
    budget: &mut BudgetTracker,
    limits: BoardPlotLimits,
) -> Result<Vec<BoardPlotRecord>, Error> {
    let mut buckets: [Vec<&PcbGraphic>; 6] = Default::default();
    for graphic in graphics {
        // Text carriers are produced by the later board-text slice.
        if let Some(slot) = category_slot(graphic.kind) {
            buckets[slot].push(graphic);
        }
    }

    let kinds = [
        BoardGraphicRecordKind::GrLine,
        BoardGraphicRecordKind::GrArc,
        BoardGraphicRecordKind::GrCircle,
        BoardGraphicRecordKind::GrRect,
        BoardGraphicRecordKind::GrPoly,
        BoardGraphicRecordKind::GrCurve,
    ];
    let mut records = Vec::new();
    for (kind, bucket) in kinds.into_iter().zip(buckets) {
        for graphic in bucket {
            let remaining = budget.remaining_operations()?;
            if kind == BoardGraphicRecordKind::GrPoly {
                budget.ensure_capacity(1, graphic.points.len())?;
            }
            let operations = graphic_operations(source, kind, graphic, remaining, limits)?;
            budget.charge(operations.len(), poly_point_total(&operations))?;
            records.push(BoardPlotRecord::Graphic(graphic_record(
                kind, graphic, operations,
            )));
        }
    }
    Ok(records)
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
    kind: BoardGraphicRecordKind,
    graphic: &PcbGraphic,
    remaining: usize,
    limits: BoardPlotLimits,
) -> Result<Vec<PlotterOperation>, Error> {
    match kind {
        BoardGraphicRecordKind::GrLine => line_operations(source, graphic, remaining, limits),
        BoardGraphicRecordKind::GrArc => arc_operations(source, graphic, remaining, limits),
        BoardGraphicRecordKind::GrCircle => Ok(vec![circle_operation(source, graphic, limits)?]),
        BoardGraphicRecordKind::GrRect => Ok(vec![rect_operation(source, graphic, limits)?]),
        BoardGraphicRecordKind::GrPoly => Ok(vec![poly_operation(source, graphic, limits)?]),
        BoardGraphicRecordKind::GrCurve => curve_operations(source, graphic, limits),
    }
}

fn graphic_record(
    kind: BoardGraphicRecordKind,
    graphic: &PcbGraphic,
    operations: Vec<PlotterOperation>,
) -> BoardGraphicRecord {
    let default_layer = if kind == BoardGraphicRecordKind::GrCurve {
        DEFAULT_CURVE_LAYER
    } else {
        DEFAULT_GRAPHIC_LAYER
    };
    BoardGraphicRecord {
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
fn resolve_stroke(
    source: &str,
    graphic: &PcbGraphic,
    limits: BoardPlotLimits,
) -> Result<BoardStroke, Error> {
    let text = source
        .get(graphic.source_range.clone())
        .ok_or_else(|| model_error("Board graphic span is out of range", Position::START))?;
    let form = parse_with_limits(
        text,
        Limits {
            max_source_bytes: text.len(),
            max_depth: limits.max_depth,
            max_nodes: limits.max_parse_nodes,
            max_decoded_string_bytes: limits.max_source_bytes,
        },
    )?;
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

fn line_operations(
    source: &str,
    graphic: &PcbGraphic,
    max_operations: usize,
    limits: BoardPlotLimits,
) -> Result<Vec<PlotterOperation>, Error> {
    let stroke = resolve_stroke(source, graphic, limits)?;
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
    limits: BoardPlotLimits,
) -> Result<Vec<PlotterOperation>, Error> {
    let stroke = resolve_stroke(source, graphic, limits)?;
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

fn circle_operation(
    source: &str,
    graphic: &PcbGraphic,
    limits: BoardPlotLimits,
) -> Result<PlotterOperation, Error> {
    let stroke = resolve_stroke(source, graphic, limits)?;
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

fn rect_operation(
    source: &str,
    graphic: &PcbGraphic,
    limits: BoardPlotLimits,
) -> Result<PlotterOperation, Error> {
    let stroke = resolve_stroke(source, graphic, limits)?;
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

fn poly_operation(
    source: &str,
    graphic: &PcbGraphic,
    limits: BoardPlotLimits,
) -> Result<PlotterOperation, Error> {
    let stroke = resolve_stroke(source, graphic, limits)?;
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

fn curve_operations(
    source: &str,
    graphic: &PcbGraphic,
    limits: BoardPlotLimits,
) -> Result<Vec<PlotterOperation>, Error> {
    // The stroke is validated before the point-count check because the Python
    // parser rejects unknown stroke types even on malformed curves.
    let stroke = resolve_stroke(source, graphic, limits)?;
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

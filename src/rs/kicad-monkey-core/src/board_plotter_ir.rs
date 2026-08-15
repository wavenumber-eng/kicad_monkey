//! Board-level plotter IR producer over selected PCB families.
//!
//! Mirrors the Python `pcb_to_ir` gr_*/segment/track_arc/via record subset:
//! layerless graphic-state operations, per-category record ordering, PCB
//! stroke widths without the footprint pen-width clamps, dash/dot
//! decomposition parity, reversed track-arc endpoints, and via aperture/
//! drill/mask-hint operation sequences.

use crate::pcb::{
    PcbFamily, PcbGraphic, PcbGraphicKind, PcbLimits, PcbNetRef, PcbPoint, PcbRoutingArc,
    PcbSegment, PcbSelection, PcbVia, PcbView,
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
pub enum BoardGraphicRecordKind {
    GrLine,
    GrArc,
    GrCircle,
    GrRect,
    GrPoly,
    GrCurve,
}

impl BoardGraphicRecordKind {
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
pub struct BoardGraphicRecord {
    pub uuid: String,
    pub kind: BoardGraphicRecordKind,
    pub layer: String,
    pub operations: Vec<PlotterOperation>,
}

/// One `(segment ...)` track record; Python emits exactly one thick segment.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardSegmentRecord {
    pub uuid: String,
    pub layer: String,
    pub locked: bool,
    pub net_id: Option<i64>,
    pub net_name: Option<String>,
    pub operations: Vec<PlotterOperation>,
}

/// One `(arc ...)` routing record; the Python serializer reverses the
/// file-order endpoints so the arc plots end-to-start.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTrackArcRecord {
    pub uuid: String,
    pub layer: String,
    pub net_id: Option<i64>,
    pub net_name: Option<String>,
    pub operations: Vec<PlotterOperation>,
}

/// Python via types serialized in the `via_type` extra.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardViaType {
    Through,
    Blind,
    Buried,
    Micro,
}

impl BoardViaType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Through => "through",
            Self::Blind => "blind",
            Self::Buried => "buried",
            Self::Micro => "micro",
        }
    }
}

/// The role of one operation inside a via record's fixed sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardViaOperationKind {
    /// `FlashPadCircle` with role `via_aperture` on the via copper layers.
    Aperture,
    /// Filled `Circle` with role `via_drill` on the via copper layers.
    Drill,
    /// `FlashPadCircle` with role `via_mask_opening` on one mask layer.
    MaskOpening,
    /// Filled `Circle` with role `via_mask_drill` on one mask layer.
    MaskDrill,
}

/// One via operation; flashes and drills share the circular payload.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardViaOperation {
    pub kind: BoardViaOperationKind,
    pub x: i64,
    pub y: i64,
    pub diameter_nm: i64,
    pub layers: Vec<String>,
}

/// Front/back optional fabrication metadata mirrored from the via carrier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoardViaFabrication {
    pub tenting_front: Option<bool>,
    pub tenting_back: Option<bool>,
    pub covering_front: Option<bool>,
    pub covering_back: Option<bool>,
    pub plugging_front: Option<bool>,
    pub plugging_back: Option<bool>,
    pub capping: Option<bool>,
    pub filling: Option<bool>,
}

impl BoardViaFabrication {
    /// Python emits `ipc4761_metadata` only when any ipc key is present.
    pub const fn any(self) -> bool {
        self.tenting_front.is_some()
            || self.tenting_back.is_some()
            || self.covering_front.is_some()
            || self.covering_back.is_some()
            || self.plugging_front.is_some()
            || self.plugging_back.is_some()
            || self.capping.is_some()
            || self.filling.is_some()
    }
}

/// One `(via ...)` record with its raw-mm extras and operation sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardViaRecord {
    pub uuid: String,
    pub layers: Vec<String>,
    pub drill: f64,
    pub size: f64,
    pub via_type: BoardViaType,
    pub fabrication: BoardViaFabrication,
    pub net_id: Option<i64>,
    pub net_name: Option<String>,
    pub operations: Vec<BoardViaOperation>,
}

/// One record of the promoted board plotter subset in Python emission order.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardPlotRecord {
    Graphic(BoardGraphicRecord),
    Segment(BoardSegmentRecord),
    TrackArc(BoardTrackArcRecord),
    Via(BoardViaRecord),
}

impl BoardPlotRecord {
    pub fn operation_count(&self) -> usize {
        match self {
            Self::Graphic(record) => record.operations.len(),
            Self::Segment(record) => record.operations.len(),
            Self::TrackArc(record) => record.operations.len(),
            Self::Via(record) => record.operations.len(),
        }
    }
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

/// Track record-level operation and point budgets fail-closed.
struct BudgetTracker {
    max_operations: usize,
    max_points: usize,
    operation_count: usize,
    point_count: usize,
}

impl BudgetTracker {
    fn remaining_operations(&self) -> Result<usize, Error> {
        self.max_operations
            .checked_sub(self.operation_count)
            .filter(|remaining| *remaining > 0)
            .ok_or_else(limit_error)
    }

    fn charge(&mut self, operations: usize, points: usize) -> Result<(), Error> {
        self.operation_count = self.operation_count.saturating_add(operations);
        if self.operation_count > self.max_operations {
            return Err(limit_error());
        }
        self.point_count = self.point_count.saturating_add(points);
        if self.point_count > self.max_points {
            return Err(point_limit_error());
        }
        Ok(())
    }
}

/// Read supported board graphics, tracks, and vias into plotter records.
pub fn board_plot_document(
    source: &str,
    limits: BoardPlotLimits,
) -> Result<BoardPlotDocument, Error> {
    let pcb_limits = PcbLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_graphics: limits.max_graphics,
        // The request-level record budget bounds every promoted family.
        max_segments: limits.max_graphics,
        max_vias: limits.max_graphics,
        max_arcs: limits.max_graphics,
        ..PcbLimits::default()
    };
    let selection = PcbSelection::only(PcbFamily::Graphics)
        .with(PcbFamily::Segments)
        .with(PcbFamily::Arcs)
        .with(PcbFamily::Vias);
    let view = PcbView::parse_selected(source, pcb_limits, selection)?;
    let metadata = view.metadata()?;
    ensure_javascript_safe_integer(metadata.version)?;

    let mut budget = BudgetTracker {
        max_operations: limits.max_operations,
        max_points: limits.max_points,
        operation_count: 0,
        point_count: 0,
    };
    let mut records = graphic_records(source, &view, &mut budget)?;
    for segment in view.segments() {
        let record = segment_record(segment?)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::Segment(record));
    }
    for arc in view.arcs() {
        let record = track_arc_record(arc?)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::TrackArc(record));
    }
    let mask_clearance = metadata.pad_to_mask_clearance;
    for via in view.vias() {
        let record = via_record(via?, mask_clearance)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::Via(record));
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

fn graphic_records(
    source: &str,
    view: &PcbView<'_>,
    budget: &mut BudgetTracker,
) -> Result<Vec<BoardPlotRecord>, Error> {
    let mut buckets: [Vec<PcbGraphic>; 6] = Default::default();
    for graphic in view.graphics() {
        let graphic = graphic?;
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
            let operations = graphic_operations(source, kind, &graphic, remaining)?;
            budget.charge(operations.len(), poly_point_total(&operations))?;
            records.push(BoardPlotRecord::Graphic(graphic_record(
                kind, &graphic, operations,
            )));
        }
    }
    Ok(records)
}

/// Python `_net_extras`: `net_id` follows the resolved ordinal and
/// `net_name` only appears for truthy resolved names.
fn net_parts(net: &PcbNetRef) -> (Option<i64>, Option<String>) {
    (net.ordinal, net.name.clone())
}

fn segment_record(segment: PcbSegment) -> Result<BoardSegmentRecord, Error> {
    let (net_id, net_name) = net_parts(&segment.net);
    // Track widths are emitted verbatim (negative values included), unlike
    // the non-positive -> 0 normalization applied to graphic strokes.
    let width_nm = mm_to_nm(segment.width.unwrap_or(0.0))?;
    let operation = layerless_segment(
        [mm_to_nm(segment.start_x)?, mm_to_nm(segment.start_y)?],
        [mm_to_nm(segment.end_x)?, mm_to_nm(segment.end_y)?],
        width_nm,
    );
    Ok(BoardSegmentRecord {
        uuid: segment.uuid.unwrap_or_default(),
        layer: segment.layer.unwrap_or_default(),
        locked: segment.locked,
        net_id,
        net_name,
        operations: vec![operation],
    })
}

fn track_arc_record(arc: PcbRoutingArc) -> Result<BoardTrackArcRecord, Error> {
    let (net_id, net_name) = net_parts(&arc.net);
    // The Python serializer plots routing arcs from the file-order end point
    // back to the start point.
    let operation = PlotterOperation::ArcThreePoint(ArcThreePoint {
        start_x: mm_to_nm(arc.end.x)?,
        start_y: mm_to_nm(arc.end.y)?,
        mid_x: mm_to_nm(arc.mid.x)?,
        mid_y: mm_to_nm(arc.mid.y)?,
        end_x: mm_to_nm(arc.start.x)?,
        end_y: mm_to_nm(arc.start.y)?,
        fill: PlotterFill::NoFill,
        width_nm: mm_to_nm(arc.width.unwrap_or(0.0))?,
        layer: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    });
    Ok(BoardTrackArcRecord {
        uuid: arc.uuid.unwrap_or_default(),
        layer: arc.layer.unwrap_or_default(),
        net_id,
        net_name,
        operations: vec![operation],
    })
}

/// Python `_via_exposed_mask_layers`: a side is exposed only when its
/// tenting option is explicitly `no` and the via reaches outer copper.
fn exposed_mask_layers(via: &PcbVia) -> Vec<&'static str> {
    let reaches_outer = |outer: &str| {
        via.layers
            .iter()
            .any(|layer| layer == outer || layer == "*.Cu")
    };
    let side = |tented: Option<bool>, outer: &str, mask: &'static str| {
        (tented == Some(false) && reaches_outer(outer)).then_some(mask)
    };
    let tenting_front = via.tenting.as_ref().and_then(|tenting| tenting.front);
    let tenting_back = via.tenting.as_ref().and_then(|tenting| tenting.back);
    side(tenting_front, "F.Cu", "F.Mask")
        .into_iter()
        .chain(side(tenting_back, "B.Cu", "B.Mask"))
        .collect()
}

fn via_record(via: PcbVia, mask_clearance: f64) -> Result<BoardViaRecord, Error> {
    let (net_id, net_name) = net_parts(&via.net);
    let x = mm_to_nm(via.at_x)?;
    let y = mm_to_nm(via.at_y)?;
    let size_nm = mm_to_nm(via.size)?;
    // Python falls back to half the pad size for missing or non-positive
    // drill diameters.
    let drill_mm = if via.drill > 0.0 {
        via.drill
    } else {
        via.size * 0.5
    };
    let drill_nm = mm_to_nm(drill_mm)?;
    let mut operations = vec![
        BoardViaOperation {
            kind: BoardViaOperationKind::Aperture,
            x,
            y,
            diameter_nm: size_nm,
            layers: via.layers.clone(),
        },
        BoardViaOperation {
            kind: BoardViaOperationKind::Drill,
            x,
            y,
            diameter_nm: drill_nm,
            layers: via.layers.clone(),
        },
    ];
    let opening_nm = mm_to_nm(via.size + 2.0 * mask_clearance)?;
    for mask in exposed_mask_layers(&via) {
        operations.push(BoardViaOperation {
            kind: BoardViaOperationKind::MaskOpening,
            x,
            y,
            diameter_nm: opening_nm,
            layers: vec![mask.to_owned()],
        });
        operations.push(BoardViaOperation {
            kind: BoardViaOperationKind::MaskDrill,
            x,
            y,
            diameter_nm: drill_nm,
            layers: vec![mask.to_owned()],
        });
    }
    Ok(BoardViaRecord {
        uuid: via.uuid.clone().unwrap_or_default(),
        layers: via.layers.clone(),
        drill: via.drill,
        size: via.size,
        via_type: match via.via_type.as_deref() {
            Some("blind") => BoardViaType::Blind,
            Some("buried") => BoardViaType::Buried,
            Some("micro") => BoardViaType::Micro,
            _ => BoardViaType::Through,
        },
        fabrication: BoardViaFabrication {
            tenting_front: via.tenting.as_ref().and_then(|value| value.front),
            tenting_back: via.tenting.as_ref().and_then(|value| value.back),
            covering_front: via.covering.as_ref().and_then(|value| value.front),
            covering_back: via.covering.as_ref().and_then(|value| value.back),
            plugging_front: via.plugging.as_ref().and_then(|value| value.front),
            plugging_back: via.plugging.as_ref().and_then(|value| value.back),
            capping: via.capping,
            filling: via.filling,
        },
        net_id,
        net_name,
        operations,
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
    kind: BoardGraphicRecordKind,
    graphic: &PcbGraphic,
    remaining: usize,
) -> Result<Vec<PlotterOperation>, Error> {
    match kind {
        BoardGraphicRecordKind::GrLine => line_operations(source, graphic, remaining),
        BoardGraphicRecordKind::GrArc => arc_operations(source, graphic, remaining),
        BoardGraphicRecordKind::GrCircle => Ok(vec![circle_operation(source, graphic)?]),
        BoardGraphicRecordKind::GrRect => Ok(vec![rect_operation(source, graphic)?]),
        BoardGraphicRecordKind::GrPoly => Ok(vec![poly_operation(source, graphic)?]),
        BoardGraphicRecordKind::GrCurve => curve_operations(source, graphic),
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

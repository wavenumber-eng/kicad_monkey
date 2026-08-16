//! Board-level plotter IR producer over selected PCB families.
//!
//! Mirrors the Python `pcb_to_ir` gr_*/segment/track_arc/via/zone record
//! subset: layerless graphic-state operations, per-category record ordering,
//! PCB stroke widths without the footprint pen-width clamps, dash/dot
//! decomposition parity, reversed track-arc endpoints, via aperture/
//! drill/mask-hint operation sequences, zone fill rings, and project
//! sidecar net-class extras.

mod copper;
mod graphics;
mod text;

use crate::pcb::{PcbFamily, PcbLimits, PcbNetRef, PcbSelection, PcbView};
use crate::plotter_ir::ensure_javascript_safe_integer;
use crate::plotter_types::{PlotterOperation, ThickSegment};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position};
use std::collections::BTreeMap;

use copper::{segment_record, track_arc_record, via_record, zone_record};
use graphics::graphic_records;
pub use text::{
    BoardTextBoxOperation, BoardTextBoxRecord, BoardTextHAlign, BoardTextOperation,
    BoardTextRecord, BoardTextRenderCache, BoardTextVAlign, BoardTextVariables,
};

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

/// Python `_with_net_class_extras`: optional exact-match sidecar classes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BoardNetClassExtras {
    pub net_class: Option<String>,
    pub net_classes: Vec<String>,
}

/// One `(segment ...)` track record; Python emits exactly one thick segment.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardSegmentRecord {
    pub uuid: String,
    pub layer: String,
    pub locked: bool,
    pub net_id: Option<i64>,
    pub net_name: Option<String>,
    pub net_classes: BoardNetClassExtras,
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
    pub net_classes: BoardNetClassExtras,
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
    pub net_classes: BoardNetClassExtras,
    pub operations: Vec<BoardViaOperation>,
}

/// One `(zone ...)` record bundling every `filled_polygon` fill ring.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardZoneRecord {
    pub uuid: String,
    pub layers: Vec<String>,
    pub fill_layers: Vec<String>,
    pub fill_island: Vec<bool>,
    pub net_id: Option<i64>,
    pub net_name: Option<String>,
    pub net_classes: BoardNetClassExtras,
    pub operations: Vec<PlotterOperation>,
}

/// One record of the promoted board plotter subset in Python emission order.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardPlotRecord {
    Graphic(BoardGraphicRecord),
    Text(BoardTextRecord),
    TextBox(BoardTextBoxRecord),
    Segment(BoardSegmentRecord),
    TrackArc(BoardTrackArcRecord),
    Via(BoardViaRecord),
    Zone(BoardZoneRecord),
}

impl BoardPlotRecord {
    pub fn operation_count(&self) -> usize {
        match self {
            Self::Graphic(record) => record.operations.len(),
            Self::Text(record) => record.operations.len(),
            Self::TextBox(record) => record.operations.len(),
            Self::Segment(record) => record.operations.len(),
            Self::TrackArc(record) => record.operations.len(),
            Self::Via(record) => record.operations.len(),
            Self::Zone(record) => record.operations.len(),
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

/// Exact net-name to net-class assignments from the project sidecar.
///
/// Mirrors Python `project_net_name_to_classes`: empty net names are
/// skipped, empty class strings are dropped, and later duplicate names
/// overwrite earlier ones.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BoardNetClassAssignments {
    by_net_name: BTreeMap<String, Vec<String>>,
}

impl BoardNetClassAssignments {
    pub fn from_entries<N, C>(entries: impl IntoIterator<Item = (N, Vec<C>)>) -> Self
    where
        N: Into<String>,
        C: Into<String>,
    {
        let mut by_net_name = BTreeMap::new();
        for (net_name, classes) in entries {
            let net_name = net_name.into();
            if net_name.is_empty() {
                continue;
            }
            let classes: Vec<String> = classes
                .into_iter()
                .map(Into::into)
                .filter(|class| !class.is_empty())
                .collect();
            by_net_name.insert(net_name, classes);
        }
        Self { by_net_name }
    }

    /// Python `_with_net_class_extras`: extras appear only for truthy net
    /// names with a non-empty class list; the first class is `net_class`.
    pub fn extras_for(&self, net_name: Option<&str>) -> BoardNetClassExtras {
        let classes = net_name
            .filter(|name| !name.is_empty())
            .and_then(|name| self.by_net_name.get(name))
            .filter(|classes| !classes.is_empty());
        match classes {
            Some(classes) => BoardNetClassExtras {
                net_class: classes.first().cloned(),
                net_classes: classes.clone(),
            },
            None => BoardNetClassExtras::default(),
        }
    }
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

    fn remaining_points(&self) -> Result<usize, Error> {
        self.max_points
            .checked_sub(self.point_count)
            .ok_or_else(point_limit_error)
    }
}

/// Read supported board families into plotter records without sidecar extras.
pub fn board_plot_document(
    source: &str,
    limits: BoardPlotLimits,
) -> Result<BoardPlotDocument, Error> {
    board_plot_document_with_net_classes(source, limits, &BoardNetClassAssignments::default())
}

/// Read supported board graphics, tracks, vias, and zones into plotter
/// records, attaching project-sidecar net-class extras by exact net name.
pub fn board_plot_document_with_net_classes(
    source: &str,
    limits: BoardPlotLimits,
    net_classes: &BoardNetClassAssignments,
) -> Result<BoardPlotDocument, Error> {
    board_plot_document_with_sidecars(source, limits, net_classes, &BoardTextVariables::default())
}

/// Read the supported board families into plotter records with both project
/// sidecar inputs: net-class extras and `${NAME}` text variables (which the
/// board `(property ...)` entries overlay key-by-key).
pub fn board_plot_document_with_sidecars(
    source: &str,
    limits: BoardPlotLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
) -> Result<BoardPlotDocument, Error> {
    let pcb_limits = PcbLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_graphics: limits.max_graphics,
        // The request-level record budget bounds every promoted family.
        max_segments: limits.max_graphics,
        max_vias: limits.max_graphics,
        max_arcs: limits.max_graphics,
        max_zones: limits.max_graphics,
        ..PcbLimits::default()
    };
    let selection = PcbSelection::only(PcbFamily::Graphics)
        .with(PcbFamily::Properties)
        .with(PcbFamily::Segments)
        .with(PcbFamily::Arcs)
        .with(PcbFamily::Vias)
        .with(PcbFamily::Zones);
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
    let variables = text::board_variables(&view, text_variables)?;
    records.extend(text::text_records(source, &view, &mut budget, &variables)?);
    for segment in view.segments() {
        let record = segment_record(segment?, net_classes)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::Segment(record));
    }
    for arc in view.arcs() {
        let record = track_arc_record(arc?, net_classes)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::TrackArc(record));
    }
    let mask_clearance = metadata.pad_to_mask_clearance;
    for via in view.vias() {
        let record = via_record(via?, mask_clearance, net_classes)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::Via(record));
    }
    for zone in view.zones() {
        let record = zone_record(zone?, net_classes)?;
        budget.charge(
            record.operations.len(),
            poly_point_total(&record.operations),
        )?;
        records.push(BoardPlotRecord::Zone(record));
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

/// Python `_net_extras`: `net_id` follows the resolved ordinal and
/// `net_name` only appears for truthy resolved names.
fn net_parts(net: &PcbNetRef) -> (Option<i64>, Option<String>) {
    (
        net.ordinal,
        net.name.clone().filter(|name| !name.is_empty()),
    )
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

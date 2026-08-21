//! Board-level plotter IR producer over selected PCB families.
//!
//! Mirrors the Python `pcb_to_ir` gr_*/segment/track_arc/via/zone record
//! subset: layerless graphic-state operations, per-category record ordering,
//! PCB stroke widths without the footprint pen-width clamps, dash/dot
//! decomposition parity, reversed track-arc endpoints, via aperture/
//! drill/mask-hint operation sequences, zone fill rings, and project
//! sidecar net-class extras.

mod copper;
mod dimension;
mod footprint;
mod graphics;
mod stroke_font_widths;
mod stroke_text_bounds;
mod table;
mod text;
mod text_cache;
mod text_native;
mod text_variables;
pub(crate) mod text_wrap;

use crate::pcb::{
    PcbDimension, PcbFamily, PcbGraphic, PcbLimits, PcbNetRef, PcbSelection, PcbView,
};
use crate::plotter_ir::ensure_javascript_safe_integer;
use crate::plotter_text_cache::{PlotterTextCacheResources, PlotterTextCacheSession};
use crate::plotter_types::{PlotterOperation, ThickSegment};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position};
use std::collections::BTreeMap;
use std::time::Instant;

use copper::{segment_record, track_arc_record, via_operation_count, via_record, zone_record};
use graphics::graphic_records;
pub use stroke_text_bounds::BoardBoundsLimits;
pub use text::{
    BoardTextBoxOperation, BoardTextBoxRecord, BoardTextHAlign, BoardTextOperation,
    BoardTextRecord, BoardTextRenderCache, BoardTextRenderCacheCoordinateSpace,
    BoardTextRenderCacheSource, BoardTextVAlign,
};
pub use text_variables::BoardTextVariables;

/// One operation within a table record. Grid/border segments precede cached
/// cell text, matching the established Python producer.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardTableOperation {
    Segment(PlotterOperation),
    Text(BoardTextOperation),
}

/// One `(table ...)` record with its source cell count and participating
/// layers.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardTableRecord {
    pub uuid: String,
    pub layers: Vec<String>,
    pub cell_count: usize,
    /// Source cell rectangles retained for the all-layer viewport authority.
    pub cell_bounds_nm: Vec<[i64; 4]>,
    pub operations: Vec<BoardTableOperation>,
}

/// One board dimension operation. Stroke-font text and dimension geometry use
/// shared layered geometry operations; faced text retains the board cache
/// payload used by ordinary board text.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardDimensionOperation {
    Geometry(PlotterOperation),
    Text(BoardTextOperation),
}

/// One `(dimension ...)` record in Python text-before-shape order.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardDimensionRecord {
    pub uuid: String,
    pub layers: Vec<String>,
    pub dimension_type: String,
    pub text: Option<String>,
    pub operations: Vec<BoardDimensionOperation>,
}

/// Canonical ownership metadata attached to one embedded-footprint child
/// operation by the established Python producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardFootprintChildMetadata {
    pub label: String,
    pub data_uuid: String,
    pub data_ref: String,
    pub object_id: String,
    pub extra_attrs: BoardFootprintChildAttributes,
}

/// Closed, typed child attributes serialized under `extra_attrs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardFootprintChildAttributes {
    pub component: String,
    pub component_uid: String,
    pub component_uuid: String,
    pub footprint: String,
    pub layer_name: Option<String>,
    pub layer_role: Option<String>,
    pub primitive: String,
    pub footprint_primitive: String,
    pub footprint_object_index: usize,
    pub footprint_subop_index: Option<usize>,
    pub footprint_text_role: Option<String>,
    pub property_name: Option<String>,
    pub fp_text_type: Option<String>,
    pub footprint_graphic_kind: Option<String>,
}

/// Closed string-valued block attributes. Python's `start_block` helper
/// stringifies every retained value and drops empty values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardFootprintBlockAttributes {
    pub primitive: String,
    pub component: Option<String>,
    pub component_uid: Option<String>,
    pub component_uuid: Option<String>,
    pub footprint: Option<String>,
    pub pad_number: Option<String>,
    pub pad_designator: Option<String>,
    pub pad_type: Option<String>,
    pub pad_shape: Option<String>,
    pub layer_names: Option<String>,
    pub net_index: Option<String>,
    pub net_id: Option<String>,
    pub net: Option<String>,
    pub net_class: Option<String>,
    pub net_classes: Option<String>,
    pub hole_owner: Option<String>,
    pub hole_kind: Option<String>,
    pub hole_plating: Option<String>,
    pub hole_render: Option<String>,
    pub hole_diameter_mm: Option<String>,
    pub hole_width_mm: Option<String>,
    pub hole_height_mm: Option<String>,
}

/// One grouping marker surrounding a pad flash or its independently grouped
/// drill operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardFootprintBlock {
    pub label: String,
    pub data_uuid: String,
    pub data_ref: String,
    pub object_id: String,
    pub layers: Vec<String>,
    pub extra_attrs: BoardFootprintBlockAttributes,
}

/// One operation in an embedded board-footprint record. Geometry and text
/// children retain source ownership metadata; pad payloads are bracketed by
/// explicit block markers exactly as in the Python plotter IR.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardFootprintOperation {
    Geometry {
        operation: PlotterOperation,
        metadata: BoardFootprintChildMetadata,
    },
    Text {
        operation: BoardTextOperation,
        metadata: BoardFootprintChildMetadata,
    },
    Pad(PlotterOperation),
    StartBlock(BoardFootprintBlock),
    EndBlock,
}

/// Board-local placement carried separately from footprint-local operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardFootprintPlacement {
    pub x_nm: i64,
    pub y_nm: i64,
    pub angle_deg: f64,
}

/// One PCB-embedded footprint record in canonical child-family order.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardFootprintRecord {
    pub uuid: String,
    pub library_link: String,
    pub reference: String,
    pub value: String,
    pub layer: String,
    pub locked: bool,
    pub descr: String,
    pub tags: String,
    pub attr: Vec<String>,
    pub placement: BoardFootprintPlacement,
    pub operations: Vec<BoardFootprintOperation>,
}

/// Limits for one board plotter conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_graphics: usize,
    pub max_operations: usize,
    pub max_points: usize,
    /// Aggregate bytes of resolved text retained in records.
    pub max_text_bytes: usize,
    /// Aggregate net-class string bytes retained across emitted records.
    pub max_net_class_bytes: usize,
    /// Aggregate embedded-footprint ownership/record metadata bytes retained
    /// after source decoding.
    pub max_metadata_bytes: usize,
    /// Maximum generic S-expression nodes or direct children in one carrier.
    pub max_parse_nodes: usize,
    /// Aggregate decoded input points retained before operation conversion.
    pub max_input_points: usize,
    /// Aggregate decoded input polygons retained before operation conversion.
    pub max_input_polygons: usize,
    /// Maximum authored render-cache polygons in one text carrier.
    pub max_cache_polygons: usize,
    /// Maximum authored render-cache contours in one text carrier.
    pub max_cache_contours: usize,
}

impl Default for BoardPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_graphics: 100_000,
            max_operations: 100_000,
            max_points: 1_000_000,
            max_text_bytes: 16 * 1024 * 1024,
            max_net_class_bytes: 16 * 1024 * 1024,
            max_metadata_bytes: 16 * 1024 * 1024,
            max_parse_nodes: 1_000_000,
            max_input_points: 1_000_000,
            max_input_polygons: 100_000,
            max_cache_polygons: 100_000,
            max_cache_contours: 1_000_000,
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
    Table(BoardTableRecord),
    Dimension(BoardDimensionRecord),
    Zone(BoardZoneRecord),
    Footprint(BoardFootprintRecord),
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
            Self::Table(record) => record.operations.len(),
            Self::Dimension(record) => record.operations.len(),
            Self::Zone(record) => record.operations.len(),
            Self::Footprint(record) => record.operations.len(),
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

    fn extras_for_bounded(
        &self,
        net_name: Option<&str>,
        budget: &mut BudgetTracker,
    ) -> Result<BoardNetClassExtras, Error> {
        let classes = net_name
            .filter(|name| !name.is_empty())
            .and_then(|name| self.by_net_name.get(name))
            .filter(|classes| !classes.is_empty());
        let Some(classes) = classes else {
            return Ok(BoardNetClassExtras::default());
        };
        let payload_bytes = classes
            .iter()
            .try_fold(classes[0].len(), |total, class| {
                total.checked_add(class.len())
            })
            .ok_or_else(net_class_limit_error)?;
        let structural_bytes = classes
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_mul(std::mem::size_of::<String>()))
            .ok_or_else(net_class_limit_error)?;
        let retained_bytes = payload_bytes
            .checked_add(structural_bytes)
            .ok_or_else(net_class_limit_error)?;
        budget.charge_net_class(retained_bytes)?;
        Ok(BoardNetClassExtras {
            net_class: classes.first().cloned(),
            net_classes: classes.clone(),
        })
    }
}

/// Track record-level operation and point budgets fail-closed.
struct BudgetTracker {
    max_operations: usize,
    max_points: usize,
    operation_count: usize,
    point_count: usize,
    max_text_bytes: usize,
    text_bytes: usize,
    max_net_class_bytes: usize,
    net_class_bytes: usize,
    max_metadata_bytes: usize,
    metadata_bytes: usize,
}

impl BudgetTracker {
    fn new(limits: BoardPlotLimits) -> Self {
        Self {
            max_operations: limits.max_operations,
            max_points: limits.max_points,
            operation_count: 0,
            point_count: 0,
            max_text_bytes: limits.max_text_bytes,
            text_bytes: 0,
            max_net_class_bytes: limits.max_net_class_bytes,
            net_class_bytes: 0,
            max_metadata_bytes: limits.max_metadata_bytes,
            metadata_bytes: 0,
        }
    }

    fn remaining_operations(&self) -> Result<usize, Error> {
        self.max_operations
            .checked_sub(self.operation_count)
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

    fn ensure_capacity(&self, operations: usize, points: usize) -> Result<(), Error> {
        self.operation_count
            .checked_add(operations)
            .filter(|total| *total <= self.max_operations)
            .ok_or_else(limit_error)?;
        self.point_count
            .checked_add(points)
            .filter(|total| *total <= self.max_points)
            .ok_or_else(point_limit_error)?;
        Ok(())
    }

    fn remaining_points(&self) -> Result<usize, Error> {
        self.max_points
            .checked_sub(self.point_count)
            .ok_or_else(point_limit_error)
    }

    fn remaining_text_bytes(&self) -> Result<usize, Error> {
        self.max_text_bytes
            .checked_sub(self.text_bytes)
            .ok_or_else(text_limit_error)
    }

    fn ensure_text_capacity(&self, bytes: usize) -> Result<(), Error> {
        self.text_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_text_bytes)
            .ok_or_else(text_limit_error)?;
        Ok(())
    }

    fn charge_text(&mut self, bytes: usize) -> Result<(), Error> {
        self.text_bytes = self.text_bytes.saturating_add(bytes);
        if self.text_bytes > self.max_text_bytes {
            return Err(text_limit_error());
        }
        Ok(())
    }

    fn charge_net_class(&mut self, bytes: usize) -> Result<(), Error> {
        self.net_class_bytes = self
            .net_class_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_net_class_bytes)
            .ok_or_else(net_class_limit_error)?;
        Ok(())
    }

    fn charge_metadata(&mut self, bytes: usize) -> Result<(), Error> {
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_metadata_bytes)
            .ok_or_else(metadata_limit_error)?;
        Ok(())
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
    board_plot_document_with_text_cache_sidecar(source, limits, net_classes, text_variables, None)
}

/// Source-bound board plot facts used by downstream presentation.
///
/// The typed plot document and PCB view are always derived from the same
/// borrowed source, preventing stale or cross-design bounds composition.
pub struct BoardPlotFacts<'a> {
    document: BoardPlotDocument,
    view: PcbView<'a>,
}

/// Opt-in successful-build timings for board Plotter-IR production.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoardPlotBuildProfile {
    pub text_cache_setup_ns: u64,
    pub selected_view_parse_ns: u64,
    pub metadata_ns: u64,
    pub decode_graphics_ns: u64,
    pub decode_tables_ns: u64,
    pub decode_dimensions_ns: u64,
    pub graphic_records_ns: u64,
    pub variables_ns: u64,
    pub text_records_ns: u64,
    pub copper_records_ns: u64,
    pub table_records_ns: u64,
    pub dimension_records_ns: u64,
    pub zone_records_ns: u64,
    pub footprint_records_ns: u64,
}

#[derive(Default)]
struct PreparedTextCache<'a> {
    session: Option<PlotterTextCacheSession<'a>>,
    setup_ns: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoardPlotFactsBuildProfile {
    pub plot: BoardPlotBuildProfile,
    pub bound_pcb_view_parse_ns: u64,
}

impl BoardPlotFacts<'_> {
    pub fn view(&self) -> &PcbView<'_> {
        &self.view
    }

    pub fn bounds(
        &self,
        outline_fonts: Option<&PlotterTextCacheResources<'_>>,
        limits: BoardBoundsLimits,
    ) -> Result<Option<[i64; 4]>, Error> {
        stroke_text_bounds::board_bounds(&self.document, &self.view, outline_fonts, limits)
    }

    pub fn into_document(self) -> BoardPlotDocument {
        self.document
    }
}

/// Build one immutable, source-bound board fact set with project sidecars.
pub fn board_plot_facts_with_sidecars<'a>(
    source: &'a str,
    plot_limits: BoardPlotLimits,
    pcb_limits: PcbLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
) -> Result<BoardPlotFacts<'a>, Error> {
    build_board_plot_facts_internal(
        source,
        plot_limits,
        pcb_limits,
        net_classes,
        text_variables,
        false,
    )
    .map(|(facts, _profile)| facts)
}

pub fn board_plot_facts_with_sidecars_profiled<'a>(
    source: &'a str,
    plot_limits: BoardPlotLimits,
    pcb_limits: PcbLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
) -> Result<(BoardPlotFacts<'a>, BoardPlotFactsBuildProfile), Error> {
    build_board_plot_facts_internal(
        source,
        plot_limits,
        pcb_limits,
        net_classes,
        text_variables,
        true,
    )
}

fn build_board_plot_facts_internal<'a>(
    source: &'a str,
    plot_limits: BoardPlotLimits,
    pcb_limits: PcbLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
    profile_enabled: bool,
) -> Result<(BoardPlotFacts<'a>, BoardPlotFactsBuildProfile), Error> {
    let view_started = profile_enabled.then(Instant::now);
    let view = PcbView::parse_selected(
        source,
        pcb_limits.intersect(board_pcb_limits(plot_limits)),
        PcbSelection::all(),
    )?;
    let bound_pcb_view_parse_ns = elapsed_ns(view_started);
    let (document, plot) = board_plot_document_from_view_internal(
        source,
        &view,
        plot_limits,
        net_classes,
        text_variables,
        PreparedTextCache::default(),
        profile_enabled,
    )?;
    Ok((
        BoardPlotFacts { document, view },
        BoardPlotFactsBuildProfile {
            plot,
            bound_pcb_view_parse_ns,
        },
    ))
}

/// Read the supported board families with project sidecars plus optional,
/// caller-supplied deterministic outline-font/cache-generation resources.
pub fn board_plot_document_with_text_cache_sidecar(
    source: &str,
    limits: BoardPlotLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
    text_cache: Option<&PlotterTextCacheResources<'_>>,
) -> Result<BoardPlotDocument, Error> {
    board_plot_document_with_text_cache_sidecar_internal(
        source,
        limits,
        net_classes,
        text_variables,
        text_cache,
        false,
    )
    .map(|(document, _profile)| document)
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered board render pass keeps every bounded family and its opt-in timings aligned"
)]
fn board_plot_document_with_text_cache_sidecar_internal(
    source: &str,
    limits: BoardPlotLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
    text_cache: Option<&PlotterTextCacheResources<'_>>,
    profile_enabled: bool,
) -> Result<(BoardPlotDocument, BoardPlotBuildProfile), Error> {
    let text_cache_started = profile_enabled.then(Instant::now);
    let text_cache = text_cache.map(PlotterTextCacheSession::new).transpose()?;
    let text_cache_setup_ns = elapsed_ns(text_cache_started);
    let view_started = profile_enabled.then(Instant::now);
    let view = PcbView::parse_selected(source, board_pcb_limits(limits), board_selection())?;
    let selected_view_parse_ns = elapsed_ns(view_started);
    let (document, mut profile) = board_plot_document_from_view_internal(
        source,
        &view,
        limits,
        net_classes,
        text_variables,
        PreparedTextCache {
            session: text_cache,
            setup_ns: text_cache_setup_ns,
        },
        profile_enabled,
    )?;
    profile.selected_view_parse_ns = selected_view_parse_ns;
    Ok((document, profile))
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered board render pass keeps every bounded family and its opt-in timings aligned"
)]
fn board_plot_document_from_view_internal(
    source: &str,
    view: &PcbView<'_>,
    limits: BoardPlotLimits,
    net_classes: &BoardNetClassAssignments,
    text_variables: &BoardTextVariables,
    text_cache: PreparedTextCache<'_>,
    profile_enabled: bool,
) -> Result<(BoardPlotDocument, BoardPlotBuildProfile), Error> {
    let mut profile = BoardPlotBuildProfile {
        text_cache_setup_ns: text_cache.setup_ns,
        ..BoardPlotBuildProfile::default()
    };
    let text_cache = text_cache.session;
    let metadata_started = profile_enabled.then(Instant::now);
    let metadata = view.metadata()?;
    ensure_javascript_safe_integer(metadata.version)?;
    profile.metadata_ns = elapsed_ns(metadata_started);

    let mut budget = BudgetTracker::new(limits);
    // Decode the shared graphics family once, then partition borrowed carriers
    // into the Python category order for geometry and text producers.
    let graphics_started = profile_enabled.then(Instant::now);
    let (graphics, decoded_graphic_points) = decoded_graphics(view, limits)?;
    profile.decode_graphics_ns = elapsed_ns(graphics_started);
    let remaining_input_points = limits
        .max_input_points
        .checked_sub(decoded_graphic_points)
        .ok_or_else(input_point_limit_error)?;
    let tables_started = profile_enabled.then(Instant::now);
    let (tables, table_cells, decoded_table_points) =
        table::decoded_tables(view, remaining_input_points)?;
    profile.decode_tables_ns = elapsed_ns(tables_started);
    let remaining_input_points = remaining_input_points
        .checked_sub(decoded_table_points)
        .ok_or_else(input_point_limit_error)?;
    let dimensions_started = profile_enabled.then(Instant::now);
    let (dimensions, decoded_dimension_points) = decoded_dimensions(view, remaining_input_points)?;
    profile.decode_dimensions_ns = elapsed_ns(dimensions_started);
    let graphic_records_started = profile_enabled.then(Instant::now);
    let mut records = graphic_records(source, &graphics, &mut budget, limits)?;
    profile.graphic_records_ns = elapsed_ns(graphic_records_started);
    let variables_started = profile_enabled.then(Instant::now);
    let dimension_variables = dimensions.iter().try_fold(false, |needed, dimension| {
        if needed {
            Ok(true)
        } else {
            dimension::needs_variables(source, dimension, limits)
        }
    })?;
    let variables = text::board_variables(
        view,
        &graphics,
        table_cells.iter().any(|cell| cell.text.contains("${")) || dimension_variables,
        text_variables,
    )?;
    profile.variables_ns = elapsed_ns(variables_started);
    let text_started = profile_enabled.then(Instant::now);
    records.extend(text::text_records(
        source,
        &graphics,
        &mut budget,
        &variables,
        text_cache.as_ref(),
        limits,
    )?);
    profile.text_records_ns = elapsed_ns(text_started);
    let copper_started = profile_enabled.then(Instant::now);
    append_copper_records(
        view,
        metadata.pad_to_mask_clearance,
        net_classes,
        &mut budget,
        &mut records,
    )?;
    profile.copper_records_ns = elapsed_ns(copper_started);
    let table_records_started = profile_enabled.then(Instant::now);
    records.extend(table::table_records(
        source,
        &tables,
        &table_cells,
        &variables,
        &mut budget,
        text_cache.as_ref(),
        limits,
    )?);
    profile.table_records_ns = elapsed_ns(table_records_started);
    let dimension_records_started = profile_enabled.then(Instant::now);
    append_dimension_records(
        source,
        &dimensions,
        &variables,
        &mut budget,
        text_cache.as_ref(),
        limits,
        &mut records,
    )?;
    profile.dimension_records_ns = elapsed_ns(dimension_records_started);
    let decoded_input_points = decoded_graphic_points
        .checked_add(decoded_table_points)
        .and_then(|count| count.checked_add(decoded_dimension_points))
        .filter(|count| *count <= limits.max_input_points)
        .ok_or_else(input_point_limit_error)?;
    let mut decoded_inputs = (decoded_input_points, 0usize);
    let zones_started = profile_enabled.then(Instant::now);
    append_zone_records(
        view,
        net_classes,
        &mut budget,
        limits,
        &mut decoded_inputs.0,
        &mut decoded_inputs.1,
        &mut records,
    )?;
    profile.zone_records_ns = elapsed_ns(zones_started);
    let footprints_started = profile_enabled.then(Instant::now);
    footprint::append_footprint_records(
        footprint::FootprintAppendContext {
            source,
            view,
            net_classes,
            text_cache: text_cache.as_ref(),
            board_mask_clearance: metadata.pad_to_mask_clearance,
            limits,
        },
        &mut budget,
        &mut decoded_inputs,
        &mut records,
    )?;
    profile.footprint_records_ns = elapsed_ns(footprints_started);
    Ok((
        BoardPlotDocument {
            version: metadata.version,
            generator: metadata.generator,
            generator_version: metadata.generator_version,
            thickness_mm: metadata.thickness,
            paper: metadata.paper,
            records,
        },
        profile,
    ))
}

fn elapsed_ns(started: Option<Instant>) -> u64 {
    started.map_or(0, |started| {
        u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
    })
}

fn append_dimension_records(
    source: &str,
    dimensions: &[PcbDimension],
    variables: &BoardTextVariables,
    budget: &mut BudgetTracker,
    text_cache: Option<&PlotterTextCacheSession<'_>>,
    limits: BoardPlotLimits,
    records: &mut Vec<BoardPlotRecord>,
) -> Result<(), Error> {
    for dimension in dimensions {
        budget.ensure_capacity(0, 0)?;
        let record =
            dimension::dimension_record(source, dimension, variables, budget, text_cache, limits)?;
        budget.charge(
            record.operations.len(),
            dimension::cache_point_total(&record),
        )?;
        budget.charge_text(dimension::retained_text_bytes(&record))?;
        records.push(BoardPlotRecord::Dimension(record));
    }
    Ok(())
}

fn append_zone_records(
    view: &PcbView<'_>,
    net_classes: &BoardNetClassAssignments,
    budget: &mut BudgetTracker,
    limits: BoardPlotLimits,
    decoded_input_points: &mut usize,
    decoded_input_polygons: &mut usize,
    records: &mut Vec<BoardPlotRecord>,
) -> Result<(), Error> {
    for zone in view.zones() {
        let zone = zone.map_err(normalize_input_limit_error)?;
        let zone_polygons = zone
            .polygons
            .len()
            .saturating_add(zone.filled_polygons.len());
        *decoded_input_polygons = decoded_input_polygons
            .checked_add(zone_polygons)
            .filter(|count| *count <= limits.max_input_polygons)
            .ok_or_else(input_polygon_limit_error)?;
        let zone_points = zone
            .polygons
            .iter()
            .map(|polygon| polygon.points.len())
            .chain(
                zone.filled_polygons
                    .iter()
                    .map(|polygon| polygon.points.len()),
            )
            .fold(0usize, usize::saturating_add);
        *decoded_input_points = decoded_input_points
            .checked_add(zone_points)
            .filter(|count| *count <= limits.max_input_points)
            .ok_or_else(input_point_limit_error)?;
        let output_points = zone
            .filled_polygons
            .iter()
            .map(|polygon| polygon.points.len())
            .fold(0usize, usize::saturating_add);
        budget.ensure_capacity(zone.filled_polygons.len(), output_points)?;
        let record = zone_record(zone, net_classes, budget)?;
        budget.charge(
            record.operations.len(),
            poly_point_total(&record.operations),
        )?;
        records.push(BoardPlotRecord::Zone(record));
    }
    Ok(())
}

fn append_copper_records(
    view: &PcbView<'_>,
    mask_clearance: f64,
    net_classes: &BoardNetClassAssignments,
    budget: &mut BudgetTracker,
    records: &mut Vec<BoardPlotRecord>,
) -> Result<(), Error> {
    for segment in view.segments() {
        budget.ensure_capacity(1, 0)?;
        let record = segment_record(segment?, net_classes, budget)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::Segment(record));
    }
    for arc in view.arcs() {
        budget.ensure_capacity(1, 0)?;
        let record = track_arc_record(arc?, net_classes, budget)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::TrackArc(record));
    }
    for via in view.vias() {
        let via = via?;
        budget.ensure_capacity(via_operation_count(&via), 0)?;
        let record = via_record(via, mask_clearance, net_classes, budget)?;
        budget.charge(record.operations.len(), 0)?;
        records.push(BoardPlotRecord::Via(record));
    }
    Ok(())
}

fn board_pcb_limits(limits: BoardPlotLimits) -> PcbLimits {
    PcbLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_top_level_forms: limits.max_parse_nodes,
        max_object_children: limits.max_parse_nodes,
        max_nets: limits.max_graphics,
        max_footprints: limits.max_graphics,
        max_footprint_children: limits.max_parse_nodes,
        max_footprint_attributes: limits.max_parse_nodes.min(256),
        max_footprint_properties: limits.max_graphics,
        max_footprint_graphics: limits.max_graphics,
        max_footprint_texts: limits.max_graphics,
        max_footprint_text_boxes: limits.max_graphics,
        max_text_effect_children: limits.max_parse_nodes,
        max_text_font_children: limits.max_parse_nodes,
        max_text_justify_tokens: limits.max_parse_nodes,
        max_text_box_points: limits.max_input_points,
        max_pad_header_scalars: limits.max_parse_nodes.min(256),
        max_pad_children: limits.max_parse_nodes,
        max_pad_chamfer_corners: limits.max_parse_nodes,
        max_pad_custom_primitives: limits.max_input_polygons,
        max_pad_custom_point_forms: limits.max_input_points,
        max_pad_custom_points: limits.max_input_points,
        max_pads: limits.max_graphics,
        max_graphics: limits.max_graphics,
        max_graphic_points: limits.max_input_points,
        // The request-level record budget bounds every promoted family.
        max_segments: limits.max_graphics,
        max_vias: limits.max_graphics,
        max_arcs: limits.max_graphics,
        max_zones: limits.max_graphics,
        max_dimensions: limits.max_graphics,
        max_tables: limits.max_graphics,
        max_images: limits.max_graphics,
        max_image_data_parts: limits.max_parse_nodes,
        max_table_cells: limits.max_parse_nodes,
        max_table_values: limits.max_parse_nodes,
        max_zone_polygons: limits.max_input_polygons,
        max_zone_points: limits.max_input_points,
        ..PcbLimits::default()
    }
}

fn decoded_graphics(
    view: &PcbView<'_>,
    limits: BoardPlotLimits,
) -> Result<(Vec<PcbGraphic>, usize), Error> {
    let mut graphics = Vec::new();
    let mut point_count = 0usize;
    for graphic in view.graphics() {
        let graphic = graphic.map_err(normalize_input_limit_error)?;
        point_count = point_count
            .checked_add(graphic.points.len())
            .filter(|count| *count <= limits.max_input_points)
            .ok_or_else(input_point_limit_error)?;
        graphics.push(graphic);
    }
    Ok((graphics, point_count))
}

fn decoded_dimensions(
    view: &PcbView<'_>,
    maximum_points: usize,
) -> Result<(Vec<PcbDimension>, usize), Error> {
    let mut dimensions = Vec::new();
    let mut point_count = 0usize;
    for dimension in view.dimensions() {
        let dimension = dimension.map_err(normalize_input_limit_error)?;
        point_count = point_count
            .checked_add(dimension.points.len())
            .filter(|count| *count <= maximum_points)
            .ok_or_else(input_point_limit_error)?;
        dimensions.push(dimension);
    }
    Ok((dimensions, point_count))
}

fn board_selection() -> PcbSelection {
    PcbSelection::only(PcbFamily::Graphics)
        .with(PcbFamily::Properties)
        .with(PcbFamily::Segments)
        .with(PcbFamily::Arcs)
        .with(PcbFamily::Vias)
        .with(PcbFamily::Tables)
        .with(PcbFamily::Dimensions)
        .with(PcbFamily::Zones)
        .with(PcbFamily::Footprints)
        .with(PcbFamily::FootprintProperties)
        .with(PcbFamily::FootprintGraphics)
        .with(PcbFamily::FootprintTexts)
        .with(PcbFamily::FootprintTextBoxes)
        .with(PcbFamily::Pads)
        .with(PcbFamily::Images)
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

fn input_point_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter decoded points exceed max_input_points",
        Position::START,
    )
}

fn input_polygon_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter decoded polygons exceed max_input_polygons",
        Position::START,
    )
}

fn normalize_input_limit_error(error: Error) -> Error {
    if error.message.contains("max_zone_polygons") {
        input_polygon_limit_error()
    } else if error.message.contains("max_zone_points")
        || error.message.contains("max_graphic_points")
    {
        input_point_limit_error()
    } else {
        error
    }
}

fn text_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter retained text exceeds max_text_bytes",
        Position::START,
    )
}

fn net_class_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter retained net-class strings exceed max_net_class_bytes",
        Position::START,
    )
}

fn metadata_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board footprint metadata exceeds max_metadata_bytes",
        Position::START,
    )
}

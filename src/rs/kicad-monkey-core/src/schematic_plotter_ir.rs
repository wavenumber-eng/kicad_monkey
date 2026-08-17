//! Bounded native schematic Plotter-IR foundation.
//!
//! The plotting decoder intentionally reads exact source millimetres instead
//! of reusing the connectivity model's 100 nm grid. Project variables, page
//! context, and worksheet bytes are explicit sidecars;
//! this module performs no filesystem discovery.

mod annotation_render;
mod worksheet_render;

use crate::plotter_ir::{ensure_javascript_safe_integer, mm_to_nm};
use crate::plotter_text_cache::PlotterTextCacheResources;
use crate::plotter_types::{
    PlotterCircle, PlotterFill, PlotterImage, PlotterLineStyle, PlotterOperation, PlotterPoly,
    PlotterText, ThickSegment,
};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Limits, Position, Sexp, parse_with_limits};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::collections::{BTreeMap, BTreeSet};

use annotation_render::append_annotation_records;
use worksheet_render::drawing_sheet_operations;

const DEFAULT_VERSION: i64 = 20_260_306;
const DEFAULT_GENERATOR: &str = "eeschema";
const DEFAULT_GENERATOR_VERSION: &str = "10.0";
pub(super) const DEFAULT_KICAD_VERSION_TEXT: &str = "KiCad E.D.A. 10.0.0-912-gf11d3da677-dirty";
const MIN_PLOT_PEN_WIDTH_NM: i64 = 84_700;
const DEFAULT_WIRE_WIDTH_MM: f64 = 0.1524;
const DEFAULT_BUS_WIDTH_MM: f64 = 0.3048;
const DEFAULT_JUNCTION_DIAMETER_MM: f64 = 0.9144;
const DEFAULT_NO_CONNECT_HALF_MM: f64 = 0.6096;
const WIRE_COLOR: &str = "#009600FF";
const BUS_COLOR: &str = "#000084FF";
const JUNCTION_COLOR: &str = "#009600FF";
const NO_CONNECT_COLOR: &str = "#000084FF";

/// Independent input and retained-output ceilings for one schematic page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_selected_forms: usize,
    pub max_parse_nodes: usize,
    pub max_records: usize,
    pub max_wires: usize,
    pub max_buses: usize,
    pub max_bus_entries: usize,
    pub max_junctions: usize,
    pub max_no_connects: usize,
    pub max_labels: usize,
    pub max_global_labels: usize,
    pub max_hierarchical_labels: usize,
    pub max_netclass_flags: usize,
    pub max_netclass_flag_properties: usize,
    pub max_texts: usize,
    pub max_text_boxes: usize,
    /// Aggregate retained text-box lines. Temporary font-engine linebreak
    /// output is independently bounded by `PlotterTextCacheLimits::linebreak`.
    pub max_text_box_lines: usize,
    pub max_operations: usize,
    pub max_points: usize,
    pub max_input_points: usize,
    pub max_text_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_project_variables: usize,
    pub max_project_variable_bytes: usize,
    pub max_worksheet_bytes: usize,
    pub max_worksheet_items: usize,
    pub max_worksheet_repeats: usize,
    pub max_worksheet_point_sets: usize,
    pub max_worksheet_points: usize,
    pub max_worksheet_bitmap_data_parts: usize,
    pub max_worksheet_bitmap_encoded_bytes: usize,
    pub max_worksheet_bitmap_decoded_bytes: usize,
    pub max_worksheet_bitmap_width_px: usize,
    pub max_worksheet_bitmap_height_px: usize,
    pub max_worksheet_bitmap_pixels: usize,
    pub max_worksheet_bitmap_decode_work: usize,
}

impl Default for SchematicPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_selected_forms: 1_000_000,
            max_parse_nodes: 1_000_000,
            max_records: 100_000,
            max_wires: 100_000,
            max_buses: 100_000,
            max_bus_entries: 100_000,
            max_junctions: 100_000,
            max_no_connects: 100_000,
            max_labels: 100_000,
            max_global_labels: 100_000,
            max_hierarchical_labels: 100_000,
            max_netclass_flags: 100_000,
            max_netclass_flag_properties: 1_000_000,
            max_texts: 100_000,
            max_text_boxes: 100_000,
            max_text_box_lines: 1_000_000,
            max_operations: 100_000,
            max_points: 1_000_000,
            max_input_points: 1_000_000,
            max_text_bytes: 16 * 1024 * 1024,
            max_metadata_bytes: 16 * 1024 * 1024,
            max_project_variables: 100_000,
            max_project_variable_bytes: 16 * 1024 * 1024,
            max_worksheet_bytes: 64 * 1024 * 1024,
            max_worksheet_items: 100_000,
            max_worksheet_repeats: 100_000,
            max_worksheet_point_sets: 100_000,
            max_worksheet_points: 1_000_000,
            max_worksheet_bitmap_data_parts: 100_000,
            max_worksheet_bitmap_encoded_bytes: 64 * 1024 * 1024,
            max_worksheet_bitmap_decoded_bytes: 64 * 1024 * 1024,
            max_worksheet_bitmap_width_px: 1_000_000,
            max_worksheet_bitmap_height_px: 1_000_000,
            max_worksheet_bitmap_pixels: 100_000_000,
            max_worksheet_bitmap_decode_work: 256 * 1024 * 1024,
        }
    }
}

/// Effective project drawing settings supplied by the caller. The transport
/// adapter owns conversion from KiCad's project-file mil representation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SchematicDrawingSettings {
    pub text_offset_ratio: f64,
    pub default_line_width_nm: i64,
}

impl Default for SchematicDrawingSettings {
    fn default() -> Self {
        Self {
            text_offset_ratio: 0.15,
            default_line_width_nm: 152_400,
        }
    }
}

/// Caller-supplied project variables. Later duplicate names overwrite earlier
/// ones, matching JSON object semantics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchematicPlotVariables {
    values: BTreeMap<String, String>,
}

impl SchematicPlotVariables {
    pub fn from_entries<N, V>(entries: impl IntoIterator<Item = (N, V)>) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        Self {
            values: entries
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
}

/// Explicit page and project sidecars. `worksheet_source`, when present, is
/// the already-selected, decompressed worksheet file; absence uses KiCad's
/// built-in default drawing sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicPlotContext {
    pub source_path: Option<String>,
    pub document_id: Option<String>,
    pub sheet_index: usize,
    pub sheet_count: usize,
    pub sheet_path: String,
    pub sheet_name: String,
    pub project_variables: SchematicPlotVariables,
    pub worksheet_source: Option<Vec<u8>>,
}

impl Default for SchematicPlotContext {
    fn default() -> Self {
        Self {
            source_path: None,
            document_id: None,
            sheet_index: 1,
            sheet_count: 1,
            sheet_path: "/".to_owned(),
            sheet_name: String::new(),
            project_variables: SchematicPlotVariables::default(),
            worksheet_source: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicCanvas {
    pub width_nm: i64,
    pub height_nm: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchematicTitleBlock {
    pub title: String,
    pub date: String,
    pub revision: String,
    pub company: String,
    pub comments: BTreeMap<i64, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicSheetHeaderRecord {
    pub uuid: String,
    pub paper_size: String,
    pub paper_width_mm: Option<f64>,
    pub paper_height_mm: Option<f64>,
    pub paper_portrait: bool,
    pub sheet_width_nm: i64,
    pub sheet_height_nm: i64,
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
    pub title_block: Option<SchematicTitleBlock>,
    pub operations: Vec<SchematicPlotOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicConnectivityRecordKind {
    Wire,
    Bus,
    BusEntry,
    Junction,
    NoConnect,
}

impl SchematicConnectivityRecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wire => "wire",
            Self::Bus => "bus",
            Self::BusEntry => "bus_entry",
            Self::Junction => "junction",
            Self::NoConnect => "no_connect",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicConnectivityRecord {
    pub uuid: String,
    pub kind: SchematicConnectivityRecordKind,
    /// Distinguishes no authored junction color from an authored transparent
    /// color, whose normalized value is `None` but whose extra is present.
    pub junction_color_authored: bool,
    pub junction_color: Option<String>,
    pub operations: Vec<SchematicPlotOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicAnnotationRecordKind {
    Label,
    GlobalLabel,
    HierarchicalLabel,
    NetclassFlag,
    Text,
    TextBox,
}

impl SchematicAnnotationRecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::GlobalLabel => "global_label",
            Self::HierarchicalLabel => "hierarchical_label",
            Self::NetclassFlag => "netclass_flag",
            Self::Text => "text",
            Self::TextBox => "text_box",
        }
    }
}

/// Annotation extras are optional only where the corresponding strict record
/// kind does not carry them. Producers populate every required field.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicAnnotationRecord {
    pub uuid: String,
    pub kind: SchematicAnnotationRecordKind,
    pub object_id: String,
    pub text: Option<String>,
    pub shape: Option<String>,
    pub at_x_nm: Option<i64>,
    pub at_y_nm: Option<i64>,
    pub length_nm: Option<i64>,
    pub operations: Vec<SchematicPlotOperation>,
}

/// Schematic-only Text wrapper for operation-local hyperlink context without
/// broadening established producer-neutral Text payloads.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicTextOperation {
    pub text: PlotterText,
    pub hyperlink_href: Option<String>,
}

/// Schematic-only styled segment used by directive/netclass flag markers.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicStyledThickSegment {
    pub segment: ThickSegment,
    pub stroke_color: String,
}

/// Schematic plotting extends the shared vector vocabulary with worksheet
/// raster placement without changing the established board/footprint ABI.
#[derive(Clone, Debug, PartialEq)]
pub enum SchematicPlotOperation {
    Plotter(PlotterOperation),
    PlotImage(PlotterImage),
    Text(SchematicTextOperation),
    StyledThickSegment(SchematicStyledThickSegment),
}

impl From<PlotterOperation> for SchematicPlotOperation {
    fn from(value: PlotterOperation) -> Self {
        Self::Plotter(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SchematicPlotRecord {
    SheetHeader(SchematicSheetHeaderRecord),
    Connectivity(SchematicConnectivityRecord),
    Annotation(SchematicAnnotationRecord),
}

impl SchematicPlotRecord {
    pub fn operation_count(&self) -> usize {
        match self {
            Self::SheetHeader(record) => record.operations.len(),
            Self::Connectivity(record) => record.operations.len(),
            Self::Annotation(record) => record.operations.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicPlotDocument {
    pub source_path: Option<String>,
    pub document_id: String,
    pub canvas: SchematicCanvas,
    pub records: Vec<SchematicPlotRecord>,
}

#[derive(Default)]
struct CarrierSpans {
    wires: Vec<FormSpan>,
    buses: Vec<FormSpan>,
    bus_entries: Vec<FormSpan>,
    junctions: Vec<FormSpan>,
    no_connects: Vec<FormSpan>,
    labels: Vec<FormSpan>,
    global_labels: Vec<FormSpan>,
    hierarchical_labels: Vec<FormSpan>,
    netclass_flags: Vec<FormSpan>,
    texts: Vec<FormSpan>,
    text_boxes: Vec<FormSpan>,
}

pub(super) struct AnnotationSpans {
    labels: Vec<FormSpan>,
    global_labels: Vec<FormSpan>,
    hierarchical_labels: Vec<FormSpan>,
    netclass_flags: Vec<FormSpan>,
    texts: Vec<FormSpan>,
    text_boxes: Vec<FormSpan>,
}

pub(crate) struct PlotBudget {
    limits: SchematicPlotLimits,
    records: usize,
    operations: usize,
    points: usize,
    input_points: usize,
    text_bytes: usize,
    metadata_bytes: usize,
}

impl PlotBudget {
    fn new(limits: SchematicPlotLimits) -> Self {
        Self {
            limits,
            records: 0,
            operations: 0,
            points: 0,
            input_points: 0,
            text_bytes: 0,
            metadata_bytes: 0,
        }
    }

    pub(crate) fn charge(
        &mut self,
        records: usize,
        operations: usize,
        points: usize,
    ) -> Result<(), Error> {
        self.records = checked_limit(self.records, records, self.limits.max_records)?;
        self.operations = checked_limit(self.operations, operations, self.limits.max_operations)?;
        self.points = checked_limit(self.points, points, self.limits.max_points)?;
        Ok(())
    }

    pub(crate) fn charge_text(&mut self, bytes: usize) -> Result<(), Error> {
        self.text_bytes = checked_limit(self.text_bytes, bytes, self.limits.max_text_bytes)?;
        Ok(())
    }

    fn charge_input_points(&mut self, points: usize) -> Result<(), Error> {
        self.input_points = checked_limit(self.input_points, points, self.limits.max_input_points)?;
        Ok(())
    }

    fn charge_metadata(&mut self, bytes: usize) -> Result<(), Error> {
        self.metadata_bytes =
            checked_limit(self.metadata_bytes, bytes, self.limits.max_metadata_bytes)?;
        Ok(())
    }
}

/// Produce the P5_060 page header and connectivity families from raw source.
pub fn schematic_plot_document(
    source: &str,
    limits: SchematicPlotLimits,
    context: &SchematicPlotContext,
) -> Result<SchematicPlotDocument, Error> {
    schematic_plot_document_impl(source, limits, context, None, None)
}

/// Produce the P5_061 page foundation and annotation families. Drawing
/// settings and optional font bytes are explicit sidecars; core never reads a
/// project or discovers a platform font.
pub fn schematic_plot_document_with_annotations(
    source: &str,
    limits: SchematicPlotLimits,
    context: &SchematicPlotContext,
    drawing_settings: SchematicDrawingSettings,
    text_resources: Option<&PlotterTextCacheResources<'_>>,
) -> Result<SchematicPlotDocument, Error> {
    validate_drawing_settings(drawing_settings)?;
    schematic_plot_document_impl(
        source,
        limits,
        context,
        Some(drawing_settings),
        text_resources,
    )
}

fn schematic_plot_document_impl(
    source: &str,
    limits: SchematicPlotLimits,
    context: &SchematicPlotContext,
    drawing_settings: Option<SchematicDrawingSettings>,
    text_resources: Option<&PlotterTextCacheResources<'_>>,
) -> Result<SchematicPlotDocument, Error> {
    validate_context(context, limits)?;
    // The projection below avoids retaining the entire syntax tree, while
    // this one-shot bounded parse makes max_parse_nodes an aggregate source
    // ceiling rather than a per-selected-form ceiling.
    parse_with_limits(
        source,
        Limits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_nodes: limits.max_parse_nodes,
            max_decoded_string_bytes: limits.max_source_bytes,
        },
    )?;
    let spans = selected_spans(source, limits, drawing_settings.is_some())?;
    let roots = spans
        .iter()
        .filter(|span| span.depth == 0)
        .collect::<Vec<_>>();
    if roots.len() != 1 || roots[0].head.as_deref() != Some("kicad_sch") {
        return Err(model_error("Expected exactly one kicad_sch root"));
    }
    let mut version = None;
    let mut generator = None;
    let mut generator_version = None;
    let mut uuid = None;
    let mut paper = None;
    let mut title_block = None;
    let mut carriers = CarrierSpans::default();
    for span in spans.into_iter().filter(|span| span.depth == 1) {
        match span.head.as_deref() {
            Some("version") if version.is_none() => {
                version = Some(form_i64(source, &span, limits)?)
            }
            Some("generator") if generator.is_none() => {
                generator = Some(form_string(source, &span, limits)?)
            }
            Some("generator_version") if generator_version.is_none() => {
                generator_version = Some(form_string(source, &span, limits)?)
            }
            Some("uuid") if uuid.is_none() => uuid = Some(form_string(source, &span, limits)?),
            Some("paper") if paper.is_none() => paper = Some(parse_paper(source, &span, limits)?),
            Some("title_block") if title_block.is_none() => {
                title_block = Some(parse_title_block(source, &span, limits)?)
            }
            Some("wire") => carriers.wires.push(span),
            Some("bus") => carriers.buses.push(span),
            Some("bus_entry") => carriers.bus_entries.push(span),
            Some("junction") => carriers.junctions.push(span),
            Some("no_connect") => carriers.no_connects.push(span),
            Some("label") => carriers.labels.push(span),
            Some("global_label") => carriers.global_labels.push(span),
            Some("hierarchical_label") => carriers.hierarchical_labels.push(span),
            Some("netclass_flag") => carriers.netclass_flags.push(span),
            Some("text") => carriers.texts.push(span),
            Some("text_box") => carriers.text_boxes.push(span),
            _ => {}
        }
    }
    ensure_family_limit(carriers.wires.len(), limits.max_wires)?;
    ensure_family_limit(carriers.buses.len(), limits.max_buses)?;
    ensure_family_limit(carriers.bus_entries.len(), limits.max_bus_entries)?;
    ensure_family_limit(carriers.junctions.len(), limits.max_junctions)?;
    ensure_family_limit(carriers.no_connects.len(), limits.max_no_connects)?;
    ensure_family_limit(carriers.labels.len(), limits.max_labels)?;
    ensure_family_limit(carriers.global_labels.len(), limits.max_global_labels)?;
    ensure_family_limit(
        carriers.hierarchical_labels.len(),
        limits.max_hierarchical_labels,
    )?;
    ensure_family_limit(carriers.netclass_flags.len(), limits.max_netclass_flags)?;
    ensure_family_limit(carriers.texts.len(), limits.max_texts)?;
    ensure_family_limit(carriers.text_boxes.len(), limits.max_text_boxes)?;
    let version = version.unwrap_or(DEFAULT_VERSION);
    ensure_javascript_safe_integer(version)?;
    let generator = generator.unwrap_or_else(|| DEFAULT_GENERATOR.to_owned());
    let generator_version =
        generator_version.unwrap_or_else(|| DEFAULT_GENERATOR_VERSION.to_owned());
    let uuid = uuid.unwrap_or_default();
    let document_id = context
        .document_id
        .clone()
        .or_else(|| (!uuid.is_empty()).then(|| uuid.clone()))
        .unwrap_or_default();
    let paper = paper.unwrap_or_default();
    let (sheet_width_nm, sheet_height_nm) = paper_dimensions(&paper)?;
    let mut budget = PlotBudget::new(limits);
    budget.charge_metadata(
        uuid.len()
            .saturating_add(generator.len())
            .saturating_add(generator_version.len())
            .saturating_add(paper.size.len())
            .saturating_add(title_block.as_ref().map_or(0, title_block_bytes))
            .saturating_add(context.source_path.as_deref().map_or(0, str::len))
            .saturating_add(document_id.len()),
    )?;
    let header_operations = drawing_sheet_operations(
        &paper,
        title_block.as_ref(),
        sheet_width_nm,
        sheet_height_nm,
        context,
        limits,
        &mut budget,
    )?;
    budget.charge(1, 0, 0)?;
    let header = SchematicSheetHeaderRecord {
        uuid: uuid.clone(),
        paper_size: paper.size,
        paper_width_mm: paper.width,
        paper_height_mm: paper.height,
        paper_portrait: paper.portrait,
        sheet_width_nm,
        sheet_height_nm,
        version,
        generator,
        generator_version,
        title_block: title_block.clone(),
        operations: header_operations,
    };
    let annotation_spans = AnnotationSpans {
        labels: std::mem::take(&mut carriers.labels),
        global_labels: std::mem::take(&mut carriers.global_labels),
        hierarchical_labels: std::mem::take(&mut carriers.hierarchical_labels),
        netclass_flags: std::mem::take(&mut carriers.netclass_flags),
        texts: std::mem::take(&mut carriers.texts),
        text_boxes: std::mem::take(&mut carriers.text_boxes),
    };
    let mut records = vec![SchematicPlotRecord::SheetHeader(header)];
    append_polyline_records(
        source,
        carriers.wires,
        SchematicConnectivityRecordKind::Wire,
        DEFAULT_WIRE_WIDTH_MM,
        WIRE_COLOR,
        limits,
        &mut budget,
        &mut records,
    )?;
    append_polyline_records(
        source,
        carriers.buses,
        SchematicConnectivityRecordKind::Bus,
        DEFAULT_BUS_WIDTH_MM,
        BUS_COLOR,
        limits,
        &mut budget,
        &mut records,
    )?;
    append_bus_entry_records(
        source,
        carriers.bus_entries,
        limits,
        &mut budget,
        &mut records,
    )?;
    append_junction_records(
        source,
        carriers.junctions,
        limits,
        &mut budget,
        &mut records,
    )?;
    append_no_connect_records(
        source,
        carriers.no_connects,
        drawing_settings.map(|settings| settings.default_line_width_nm),
        limits,
        &mut budget,
        &mut records,
    )?;
    if let Some(drawing_settings) = drawing_settings {
        append_annotation_records(
            source,
            annotation_spans,
            title_block.as_ref(),
            context,
            drawing_settings,
            text_resources,
            limits,
            &mut budget,
            &mut records,
        )?;
    }
    Ok(SchematicPlotDocument {
        source_path: context.source_path.clone(),
        document_id,
        canvas: SchematicCanvas {
            width_nm: sheet_width_nm,
            height_nm: sheet_height_nm,
        },
        records,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Paper {
    pub size: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub portrait: bool,
}

impl Default for Paper {
    fn default() -> Self {
        Self {
            size: "A4".to_owned(),
            width: None,
            height: None,
            portrait: false,
        }
    }
}

fn selected_spans(
    source: &str,
    limits: SchematicPlotLimits,
    annotations: bool,
) -> Result<Vec<FormSpan>, Error> {
    let mut heads = vec![
        "version",
        "generator",
        "generator_version",
        "uuid",
        "paper",
        "title_block",
        "wire",
        "bus",
        "bus_entry",
        "junction",
        "no_connect",
    ];
    if annotations {
        heads.extend([
            "label",
            "global_label",
            "hierarchical_label",
            "netclass_flag",
            "text",
            "text_box",
        ]);
    }
    let mut paths = BTreeSet::from([vec!["kicad_sch".to_owned()]]);
    paths.extend(
        heads
            .into_iter()
            .map(|head| vec!["kicad_sch".to_owned(), head.to_owned()]),
    );
    scan_form_spans_with_limits(
        source,
        &Selector {
            paths: Some(paths),
            min_depth: Some(0),
            max_depth: Some(1),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: limits.max_selected_forms,
            ..ProjectionLimits::default()
        },
    )
}

fn parse_span(source: &str, span: &FormSpan, limits: SchematicPlotLimits) -> Result<Sexp, Error> {
    let text = span.text(source)?;
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

fn append_polyline_records(
    source: &str,
    spans: Vec<FormSpan>,
    kind: SchematicConnectivityRecordKind,
    default_width_mm: f64,
    default_color: &str,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in spans {
        let form = parse_span(source, &span, limits)?;
        let points = child(&form, "pts").map_or(Ok(Vec::new()), |pts| parse_points(pts))?;
        budget.charge_input_points(points.len())?;
        if points.is_empty() {
            continue;
        }
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let stroke = resolve_stroke(&form, default_width_mm, default_color)?;
        budget.charge_metadata(uuid.len())?;
        budget.charge(1, 1, points.len())?;
        records.push(SchematicPlotRecord::Connectivity(
            SchematicConnectivityRecord {
                uuid,
                kind,
                junction_color_authored: false,
                junction_color: None,
                operations: vec![
                    PlotterOperation::PlotPoly(PlotterPoly {
                        points,
                        fill: PlotterFill::NoFill,
                        width_nm: stroke.width_nm,
                        layer: None,
                        stroke_color: Some(stroke.color),
                        fill_color: None,
                        line_style: Some(stroke.style),
                    })
                    .into(),
                ],
            },
        ));
    }
    Ok(())
}

fn append_bus_entry_records(
    source: &str,
    spans: Vec<FormSpan>,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in spans {
        let form = parse_span(source, &span, limits)?;
        let [x, y] = child(&form, "at").map_or(Ok([0, 0]), parse_point)?;
        let [dx, dy] = child(&form, "size")
            .map_or_else(|| Ok([mm_to_nm(2.54)?, mm_to_nm(2.54)?]), parse_point)?;
        budget.charge_input_points(2)?;
        let end_x = x.checked_add(dx).ok_or_else(limit_error)?;
        let end_y = y.checked_add(dy).ok_or_else(limit_error)?;
        ensure_javascript_safe_integer(end_x)?;
        ensure_javascript_safe_integer(end_y)?;
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let stroke = resolve_stroke(&form, DEFAULT_WIRE_WIDTH_MM, WIRE_COLOR)?;
        budget.charge_metadata(uuid.len())?;
        budget.charge(1, 1, 2)?;
        records.push(SchematicPlotRecord::Connectivity(
            SchematicConnectivityRecord {
                uuid,
                kind: SchematicConnectivityRecordKind::BusEntry,
                junction_color_authored: false,
                junction_color: None,
                operations: vec![
                    PlotterOperation::PlotPoly(PlotterPoly {
                        points: vec![[x, y], [end_x, end_y]],
                        fill: PlotterFill::NoFill,
                        width_nm: stroke.width_nm,
                        layer: None,
                        stroke_color: Some(stroke.color),
                        fill_color: None,
                        line_style: Some(stroke.style),
                    })
                    .into(),
                ],
            },
        ));
    }
    Ok(())
}

fn append_junction_records(
    source: &str,
    spans: Vec<FormSpan>,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in spans {
        let form = parse_span(source, &span, limits)?;
        let [cx, cy] = child(&form, "at").map_or(Ok([0, 0]), parse_point)?;
        budget.charge_input_points(1)?;
        let diameter =
            child(&form, "diameter").map_or(Ok(DEFAULT_JUNCTION_DIAMETER_MM), |value| {
                let value = number_at(value, 1)?;
                Ok(if value > 0.0 {
                    value
                } else {
                    DEFAULT_JUNCTION_DIAMETER_MM
                })
            })?;
        let authored = child(&form, "color").is_some();
        let authored_color = child(&form, "color")
            .map(parse_color)
            .transpose()?
            .flatten();
        let color = authored_color
            .clone()
            .unwrap_or_else(|| JUNCTION_COLOR.to_owned());
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        budget.charge_metadata(
            uuid.len()
                .saturating_add(authored_color.as_deref().map_or(0, str::len)),
        )?;
        budget.charge(1, 1, 0)?;
        records.push(SchematicPlotRecord::Connectivity(
            SchematicConnectivityRecord {
                uuid,
                kind: SchematicConnectivityRecordKind::Junction,
                junction_color_authored: authored,
                junction_color: authored_color,
                operations: vec![
                    PlotterOperation::Circle(PlotterCircle {
                        cx,
                        cy,
                        diameter_nm: mm_to_nm(diameter)?,
                        fill: PlotterFill::FilledShape,
                        width_nm: 0,
                        layer: None,
                        role: None,
                        layers: Vec::new(),
                        mask_margin_nm: None,
                        pad_size_x_nm: None,
                        pad_size_y_nm: None,
                        stroke_color: Some(color.clone()),
                        fill_color: Some(color),
                        line_style: None,
                    })
                    .into(),
                ],
            },
        ));
    }
    Ok(())
}

fn append_no_connect_records(
    source: &str,
    spans: Vec<FormSpan>,
    default_line_width_nm: Option<i64>,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let half = mm_to_nm(DEFAULT_NO_CONNECT_HALF_MM)?;
    let width_nm = match default_line_width_nm {
        Some(width) => width,
        None => mm_to_nm(6.0 * 0.0254)?.max(MIN_PLOT_PEN_WIDTH_NM),
    };
    for span in spans {
        let form = parse_span(source, &span, limits)?;
        let [cx, cy] = child(&form, "at").map_or(Ok([0, 0]), parse_point)?;
        budget.charge_input_points(1)?;
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let coords = [
            cx.checked_sub(half),
            cy.checked_sub(half),
            cx.checked_add(half),
            cy.checked_add(half),
        ];
        if coords.iter().any(Option::is_none) {
            return Err(limit_error());
        }
        let [x0, y0, x1, y1] = coords.map(Option::unwrap);
        for value in [x0, y0, x1, y1] {
            ensure_javascript_safe_integer(value)?;
        }
        budget.charge_metadata(uuid.len())?;
        budget.charge(1, 2, 4)?;
        let poly = |points| {
            SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(PlotterPoly {
                points,
                fill: PlotterFill::NoFill,
                width_nm,
                layer: None,
                stroke_color: Some(NO_CONNECT_COLOR.to_owned()),
                fill_color: None,
                line_style: None,
            }))
        };
        records.push(SchematicPlotRecord::Connectivity(
            SchematicConnectivityRecord {
                uuid,
                kind: SchematicConnectivityRecordKind::NoConnect,
                junction_color_authored: false,
                junction_color: None,
                operations: vec![
                    poly(vec![[x0, y0], [x1, y1]]),
                    poly(vec![[x0, y1], [x1, y0]]),
                ],
            },
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct Stroke {
    width_nm: i64,
    style: PlotterLineStyle,
    color: String,
}

fn resolve_stroke(
    form: &Sexp,
    default_width_mm: f64,
    default_color: &str,
) -> Result<Stroke, Error> {
    let stroke = child(form, "stroke");
    let raw_width = stroke
        .and_then(|value| child(value, "width"))
        .map_or(Ok(0.0), |value| number_at(value, 1))?;
    let width_nm = if raw_width < 0.0 {
        0
    } else if raw_width == 0.0 {
        mm_to_nm(default_width_mm)?.max(MIN_PLOT_PEN_WIDTH_NM)
    } else {
        mm_to_nm(raw_width)?.max(MIN_PLOT_PEN_WIDTH_NM)
    };
    let style = match stroke
        .and_then(|value| child_string(value, "type"))
        .as_deref()
        .unwrap_or("default")
    {
        "solid" => PlotterLineStyle::Solid,
        "dash" => PlotterLineStyle::Dash,
        "dot" => PlotterLineStyle::Dot,
        "dash_dot" => PlotterLineStyle::DashDot,
        "dash_dot_dot" => PlotterLineStyle::DashDotDot,
        "default" => PlotterLineStyle::Default,
        _ => return Err(model_error("Unsupported schematic stroke type")),
    };
    let color = stroke
        .and_then(|value| child(value, "color"))
        .map(parse_color)
        .transpose()?
        .flatten()
        .unwrap_or_else(|| default_color.to_owned());
    Ok(Stroke {
        width_nm,
        style,
        color,
    })
}

fn parse_paper(source: &str, span: &FormSpan, limits: SchematicPlotLimits) -> Result<Paper, Error> {
    let form = parse_span(source, span, limits)?;
    let values = list(&form).ok_or_else(|| model_error("Malformed paper form"))?;
    let size = values.get(1).and_then(text).unwrap_or("A4").to_owned();
    let width = positional_number(values.get(2))?;
    let height = positional_number(values.get(3))?;
    Ok(Paper {
        size,
        width,
        height,
        portrait: values
            .iter()
            .skip(2)
            .any(|value| text(value) == Some("portrait")),
    })
}

fn parse_title_block(
    source: &str,
    span: &FormSpan,
    limits: SchematicPlotLimits,
) -> Result<SchematicTitleBlock, Error> {
    let form = parse_span(source, span, limits)?;
    let mut result = SchematicTitleBlock::default();
    let (mut saw_title, mut saw_date, mut saw_revision, mut saw_company) =
        (false, false, false, false);
    for item in list(&form).into_iter().flatten().skip(1) {
        match list(item).and_then(|values| values.first()).and_then(text) {
            Some("title") if !saw_title => {
                saw_title = true;
                result.title = value_at(item, 1).unwrap_or_default();
            }
            Some("date") if !saw_date => {
                saw_date = true;
                result.date = value_at(item, 1).unwrap_or_default();
            }
            Some("rev") if !saw_revision => {
                saw_revision = true;
                result.revision = value_at(item, 1).unwrap_or_default();
            }
            Some("company") if !saw_company => {
                saw_company = true;
                result.company = value_at(item, 1).unwrap_or_default();
            }
            Some("comment") => {
                if let (Some(number), Some(value)) = (integer_at(item, 1), scalar_at(item, 2)) {
                    result.comments.insert(number, value);
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

fn paper_dimensions(paper: &Paper) -> Result<(i64, i64), Error> {
    let standard = |w: f64, h: f64| {
        (
            ((w * 1000.0 / 25.4 + 0.5) as i64 as f64) * 0.0254,
            ((h * 1000.0 / 25.4 + 0.5) as i64 as f64) * 0.0254,
        )
    };
    let (mut width, mut height) = if paper.size == "User" {
        match (paper.width, paper.height) {
            (Some(w), Some(h)) => (w, h),
            _ => standard(297.0, 210.0),
        }
    } else {
        match paper.size.as_str() {
            "A0" => standard(1189.0, 841.0),
            "A1" => standard(841.0, 594.0),
            "A2" => standard(594.0, 420.0),
            "A3" => standard(420.0, 297.0),
            "A4" => standard(297.0, 210.0),
            "A5" => standard(210.0, 148.0),
            "A" | "USLetter" => (279.4, 215.9),
            "B" => (431.8, 279.4),
            "C" => (558.8, 431.8),
            "D" => (863.6, 558.8),
            "E" => (1117.6, 863.6),
            "USLegal" => (355.6, 215.9),
            "USLedger" => (431.8, 279.4),
            _ => standard(297.0, 210.0),
        }
    };
    if paper.portrait {
        std::mem::swap(&mut width, &mut height);
    }
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(model_error("Paper dimensions must be finite and positive"));
    }
    Ok((mm_to_nm(width)?, mm_to_nm(height)?))
}

fn validate_context(
    context: &SchematicPlotContext,
    limits: SchematicPlotLimits,
) -> Result<(), Error> {
    if context.sheet_index == 0
        || context.sheet_count == 0
        || context.sheet_index > context.sheet_count
    {
        return Err(model_error("Invalid schematic page context"));
    }
    if context.project_variables.iter().len() > limits.max_project_variables {
        return Err(limit_error());
    }
    let variable_bytes = context
        .project_variables
        .iter()
        .try_fold(0usize, |total, (name, value)| {
            total.checked_add(name.len())?.checked_add(value.len())
        })
        .ok_or_else(limit_error)?;
    if variable_bytes > limits.max_project_variable_bytes {
        return Err(limit_error());
    }
    if context
        .worksheet_source
        .as_ref()
        .is_some_and(|source| source.len() > limits.max_worksheet_bytes)
    {
        return Err(limit_error());
    }
    Ok(())
}

fn validate_drawing_settings(settings: SchematicDrawingSettings) -> Result<(), Error> {
    if !settings.text_offset_ratio.is_finite() || settings.text_offset_ratio < 0.0 {
        return Err(model_error(
            "Schematic text_offset_ratio must be finite and non-negative",
        ));
    }
    if settings.default_line_width_nm < MIN_PLOT_PEN_WIDTH_NM {
        return Err(model_error(
            "Schematic default_line_width_nm must be at least 84700 nm",
        ));
    }
    ensure_javascript_safe_integer(settings.default_line_width_nm).map(|_| ())
}

fn ensure_family_limit(count: usize, maximum: usize) -> Result<(), Error> {
    if count > maximum {
        Err(limit_error())
    } else {
        Ok(())
    }
}

fn title_block_bytes(title: &SchematicTitleBlock) -> usize {
    title
        .title
        .len()
        .saturating_add(title.date.len())
        .saturating_add(title.revision.len())
        .saturating_add(title.company.len())
        .saturating_add(title.comments.iter().fold(0usize, |total, (key, value)| {
            total
                .saturating_add(key.to_string().len())
                .saturating_add(value.len())
        }))
}

fn form_string(
    source: &str,
    span: &FormSpan,
    limits: SchematicPlotLimits,
) -> Result<String, Error> {
    Ok(value_at(&parse_span(source, span, limits)?, 1).unwrap_or_default())
}
fn form_i64(source: &str, span: &FormSpan, limits: SchematicPlotLimits) -> Result<i64, Error> {
    integer_at(&parse_span(source, span, limits)?, 1)
        .ok_or_else(|| model_error("Expected schematic integer"))
}
fn parse_points(form: &Sexp) -> Result<Vec<[i64; 2]>, Error> {
    list(form)
        .into_iter()
        .flatten()
        .skip(1)
        .filter(|value| {
            list(value).and_then(|values| values.first()).and_then(text) == Some("xy")
                && list(value).is_some_and(|values| values.len() >= 3)
        })
        .map(parse_point)
        .collect()
}
fn parse_point(form: &Sexp) -> Result<[i64; 2], Error> {
    Ok([
        mm_to_nm(number_at(form, 1)?)?,
        mm_to_nm(number_at(form, 2)?)?,
    ])
}
fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
    list(form)?
        .iter()
        .find(|value| list(value).and_then(|values| values.first()).and_then(text) == Some(head))
}
fn child_string(form: &Sexp, head: &str) -> Option<String> {
    child(form, head).and_then(|value| scalar_at(value, 1))
}
fn list(form: &Sexp) -> Option<&[Sexp]> {
    if let Sexp::List(values) = form {
        Some(values)
    } else {
        None
    }
}
fn text(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}
fn scalar_at(form: &Sexp, index: usize) -> Option<String> {
    list(form)?.get(index).map(|value| match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => value.clone(),
        Sexp::Integer(value) => value.to_string(),
        Sexp::Float(value) => value.to_string(),
        Sexp::List(_) => String::new(),
    })
}
fn value_at(form: &Sexp, index: usize) -> Option<String> {
    scalar_at(form, index)
}
fn integer_at(form: &Sexp, index: usize) -> Option<i64> {
    match list(form)?.get(index)? {
        Sexp::Integer(value) => Some(*value),
        Sexp::Atom(value) | Sexp::Quoted(value) => value.parse().ok(),
        Sexp::Float(value) if value.fract() == 0.0 => Some(*value as i64),
        _ => None,
    }
}
fn number_at(form: &Sexp, index: usize) -> Result<f64, Error> {
    let value = match list(form).and_then(|values| values.get(index)) {
        Some(Sexp::Integer(value)) => *value as f64,
        Some(Sexp::Float(value)) => *value,
        Some(Sexp::Atom(value)) | Some(Sexp::Quoted(value)) => value
            .parse()
            .map_err(|_| model_error("Expected finite number"))?,
        _ => return Err(model_error("Expected finite number")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(model_error("Expected finite number"))
    }
}

fn positional_number(value: Option<&Sexp>) -> Result<Option<f64>, Error> {
    let value = match value {
        Some(Sexp::Integer(value)) => Some(*value as f64),
        Some(Sexp::Float(value)) => Some(*value),
        _ => None,
    };
    if value.is_some_and(|value| !value.is_finite()) {
        Err(model_error("Paper dimensions must be finite"))
    } else {
        Ok(value)
    }
}

pub(crate) fn color_hex(red: i64, green: i64, blue: i64, alpha: f64) -> Option<String> {
    if alpha <= 0.0 {
        return None;
    }
    let component = |value: i64| value.clamp(0, 255);
    let alpha = if alpha <= 1.0 {
        (alpha * 255.0).round_ties_even() as i64
    } else {
        alpha.round_ties_even() as i64
    }
    .clamp(0, 255);
    Some(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        component(red),
        component(green),
        component(blue),
        alpha
    ))
}
fn parse_color(form: &Sexp) -> Result<Option<String>, Error> {
    Ok(color_hex(
        integer_at(form, 1).unwrap_or(0),
        integer_at(form, 2).unwrap_or(0),
        integer_at(form, 3).unwrap_or(0),
        number_at(form, 4)?,
    ))
}
fn checked_limit(current: usize, additional: usize, maximum: usize) -> Result<usize, Error> {
    current
        .checked_add(additional)
        .filter(|value| *value <= maximum)
        .ok_or_else(limit_error)
}
pub(crate) fn model_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Build,
        ErrorKind::InvalidBuildValue,
        message,
        Position::START,
    )
}
pub(crate) fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Build,
        ErrorKind::ResourceLimit,
        "schematic plot exceeds configured limits",
        Position::START,
    )
}

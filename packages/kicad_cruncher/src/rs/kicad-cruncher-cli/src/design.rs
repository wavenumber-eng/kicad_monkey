//! Direct Rust assembly of the nonvisual design-review foundations.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use kicad_monkey_contracts::generated::native_svg_render_request::{
    CanonicalUint64Decimal as SvgUint64, NativeSchematicSvgDocument, NativeSvgPlotDocument,
    NativeSvgPositiveSafeInteger, NativeSvgRenderLimits, NativeSvgRenderRequestA0,
    NativeSvgViewport,
};
use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    CanonicalUint64Decimal, SourceBundleManifestA0, SourceBundleSource, SourceKind, SourceSlot,
};
use kicad_monkey_core::schematic_embedded::{
    SchematicEmbeddedFile, SchematicEmbeddedLimits, schematic_embedded_files,
};
use kicad_monkey_core::{
    BoardBoundsLimits, BoardPlotContractLimits, BoardPlotFactsBuildProfile,
    project_board_plot_document_a0,
};
use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, KiCadDesignJsonPaths,
    KiCadDesignPcb, KiCadDesignSourcePath, KiCadNetlist, KiCadNetlistJsonMetadata,
    KiCadNetlistLimits, KiCadSchematicInstance, PcbLimits, PcbView, PlotterTextCacheLimits,
    PlotterTextCacheResources, PlotterTextFont, ProjectDocument, ProjectLimits,
    SchematicBundleIndex, SchematicBundleLimits, SchematicDocument, SchematicDocumentLimits,
    SchematicDrawingSettings, SchematicPlotBuildProfile, SchematicPlotContext,
    SchematicPlotContractBudget, SchematicPlotContractLimits, SchematicPlotLimits,
    SchematicPlotVariables, SourceBundle, SourceBundleLimits, TokenKind,
    board_plot_facts_with_sidecars, board_plot_facts_with_sidecars_profiled,
    build_kicad_design_facts, build_kicad_design_facts_profiled, build_kicad_design_json,
    build_kicad_design_json_profiled, build_kicad_netlist_json, emit_kicad_netlist,
    schematic_plot_document_budget, schematic_plot_document_json,
    schematic_plot_document_with_sheets, schematic_plot_document_with_sheets_profiled,
    validate_compiled_schematic_graph,
};
use kicad_monkey_svg::{SvgMetrics, render_svg};
use sha2::{Digest, Sha256};

use crate::performance::PerformanceRecorder;

const GRAPH_TOOL: &str = "kicad_cruncher";
const KICAD_STROKE_REGULAR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke.ttf"
));
const KICAD_STROKE_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke-italic.ttf"
));
const KICAD_STROKE_BOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke-bold.ttf"
));
const KICAD_STROKE_BOLD_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke-bold-italic.ttf"
));

struct SchematicFontStyle<'a> {
    face: String,
    bold: bool,
    italic: bool,
    name: String,
    bytes: Cow<'a, [u8]>,
    sha256: String,
    fake_bold: bool,
    fake_italic: bool,
}

struct EmbeddedFontCandidate<'a> {
    file: &'a SchematicEmbeddedFile,
    families: Vec<String>,
    bold: bool,
    italic: bool,
    sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicPlotDocumentsLimits {
    pub max_documents: usize,
    pub max_total_derived_items: usize,
    pub max_total_materialized_bytes: usize,
    pub max_total_output_bytes: usize,
    pub per_document: SchematicPlotContractLimits,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SchematicPlotDocumentsBuildProfile {
    pub embedded_sidecars_ns: u64,
    pub project_sidecars_ns: u64,
    pub requested_font_faces_ns: u64,
    pub font_index_and_selection_ns: u64,
    pub font_resource_setup_ns: u64,
    pub plot_ir: SchematicPlotBuildProfile,
    pub plot_contract_budget_ns: u64,
    pub plot_json_projection_ns: u64,
    pub aggregate_output_serialization_ns: u64,
}

impl SchematicPlotDocumentsBuildProfile {
    fn add_plot_ir(&mut self, profile: SchematicPlotBuildProfile) {
        self.plot_ir.validate_source_parse_ns = self
            .plot_ir
            .validate_source_parse_ns
            .saturating_add(profile.validate_source_parse_ns);
        self.plot_ir.select_and_collect_inputs_ns = self
            .plot_ir
            .select_and_collect_inputs_ns
            .saturating_add(profile.select_and_collect_inputs_ns);
        self.plot_ir.worksheet_header_ns = self
            .plot_ir
            .worksheet_header_ns
            .saturating_add(profile.worksheet_header_ns);
        self.plot_ir.connectivity_ns = self
            .plot_ir
            .connectivity_ns
            .saturating_add(profile.connectivity_ns);
        self.plot_ir.text_resource_setup_ns = self
            .plot_ir
            .text_resource_setup_ns
            .saturating_add(profile.text_resource_setup_ns);
        self.plot_ir.annotations_ns = self
            .plot_ir
            .annotations_ns
            .saturating_add(profile.annotations_ns);
        self.plot_ir.graphics_and_rule_areas_ns = self
            .plot_ir
            .graphics_and_rule_areas_ns
            .saturating_add(profile.graphics_and_rule_areas_ns);
        self.plot_ir.images_ns = self.plot_ir.images_ns.saturating_add(profile.images_ns);
        self.plot_ir.tables_ns = self.plot_ir.tables_ns.saturating_add(profile.tables_ns);
        self.plot_ir.symbols_ns = self.plot_ir.symbols_ns.saturating_add(profile.symbols_ns);
        self.plot_ir.sheets_ns = self.plot_ir.sheets_ns.saturating_add(profile.sheets_ns);
    }
}

impl Default for SchematicPlotDocumentsLimits {
    fn default() -> Self {
        Self {
            max_documents: 4_096,
            max_total_derived_items: 64_000_000,
            max_total_materialized_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
            max_total_output_bytes: 512 * 1024 * 1024,
            per_document: SchematicPlotContractLimits::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SchematicSvgDocumentsLimits {
    pub max_documents: usize,
    pub max_total_svg_bytes: usize,
    pub per_document: SchematicSvgRenderLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicSvgRenderLimits {
    pub max_block_depth: u32,
    pub max_image_encoded_bytes: usize,
    pub max_operations: u32,
    pub max_points: usize,
    pub max_records: u32,
    pub max_render_work: usize,
    pub max_svg_bytes: usize,
    pub max_svg_elements: usize,
    pub max_text_bytes: usize,
}

impl Default for SchematicSvgDocumentsLimits {
    fn default() -> Self {
        Self {
            max_documents: 4_096,
            max_total_svg_bytes: 512 * 1024 * 1024,
            per_document: SchematicSvgRenderLimits {
                max_block_depth: 4_096,
                max_image_encoded_bytes: 256 * 1024 * 1024,
                max_operations: 4_000_000,
                max_points: 16_000_000,
                max_records: 1_000_000,
                max_render_work: 64_000_000,
                max_svg_bytes: 512 * 1024 * 1024,
                max_svg_elements: 8_000_000,
                max_text_bytes: 256 * 1024 * 1024,
            },
        }
    }
}

#[derive(Debug)]
pub struct SchematicBaseSvg {
    pub document_id: String,
    pub source_path: String,
    pub plot_document_sha256: String,
    pub svg: String,
    pub metrics: SvgMetrics,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SchematicBaseSvgBuildProfile {
    pub request_projection_ns: u64,
    pub native_render_ns: u64,
    pub artifact_identity_ns: u64,
}

#[derive(Debug)]
pub struct SchematicPlotDocument {
    pub(crate) value: serde_json::Value,
    pub(crate) instance: KiCadSchematicInstance,
}

#[derive(Debug)]
pub struct BoardPlotDocument {
    pub(crate) value: serde_json::Value,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
    pub(crate) copper_layers: Vec<String>,
    pub(crate) bounds: Option<[i64; 4]>,
}

impl BoardPlotDocument {
    pub fn copper_layer_count(&self) -> usize {
        self.copper_layers.len()
    }

    pub fn record_count(&self) -> usize {
        self.value["records"].as_array().map_or(0, Vec::len)
    }

    pub fn operation_count(&self) -> usize {
        self.value["records"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|record| record["operations"].as_array())
            .map(Vec::len)
            .sum()
    }

    pub fn serialized_bytes(&self) -> Result<usize, DesignError> {
        let mut writer = SerializedSize::default();
        serde_json::to_writer(&mut writer, &self.value)
            .map_err(|error| DesignError::context("could not size board plot document", error))?;
        Ok(writer.written)
    }
}

#[derive(Default)]
struct SerializedSize {
    written: usize,
}

impl io::Write for SerializedSize {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.written = self
            .written
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("serialized board plot size overflowed"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct LimitedSerializedSize {
    written: usize,
    limit: usize,
}

impl io::Write for LimitedSerializedSize {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let total = self
            .written
            .checked_add(buffer.len())
            .filter(|total| *total <= self.limit)
            .ok_or_else(|| io::Error::other("serialized board plot limit exceeded"))?;
        self.written = total;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardPlotDocumentLimits {
    pub max_copper_layers: usize,
    pub max_contract_bytes: usize,
    pub plot: BoardPlotLimits,
    pub bounds: BoardBoundsLimits,
    pub contract: BoardPlotContractLimits,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct BoardPlotDocumentBuildProfile {
    pub project_sidecars_ns: u64,
    pub facts: BoardPlotFactsBuildProfile,
    pub copper_layer_enumeration_ns: u64,
    pub font_face_scan_ns: u64,
    pub font_face_scan_accounted_ns: u64,
    pub embedded_font_extraction_ns: u64,
    pub embedded_font_extraction_accounted_ns: u64,
    pub font_index_and_selection_ns: u64,
    pub font_resource_setup_ns: u64,
    pub bounds_ns: u64,
    pub source_identity_ns: u64,
    pub contract_projection_ns: u64,
    pub contract_serialization_ns: u64,
    pub contract_materialization_ns: u64,
}

impl Default for BoardPlotDocumentLimits {
    fn default() -> Self {
        Self {
            max_copper_layers: 64,
            max_contract_bytes: 512 * 1024 * 1024,
            plot: BoardPlotLimits::default(),
            bounds: BoardBoundsLimits::default(),
            contract: BoardPlotContractLimits::default(),
        }
    }
}

#[derive(Debug)]
pub struct DesignError {
    message: String,
}

impl DesignError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn context(context: &str, error: impl fmt::Display) -> Self {
        Self::new(format!("{context}: {error}"))
    }
}

impl fmt::Display for DesignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DesignError {}

impl From<io::Error> for DesignError {
    fn from(error: io::Error) -> Self {
        Self::context("design-review I/O failed", error)
    }
}

#[derive(Debug)]
pub struct LoadedDesignSources {
    pub input_path: PathBuf,
    pub bundle_root: PathBuf,
    pub bundle: SourceBundle,
    pub source_snapshot_sha256: String,
    pub pcb_path: Option<PathBuf>,
    pub pcb_source: Option<Arc<str>>,
}

#[derive(Debug)]
pub(crate) struct PreparedDesignSources {
    paths: DesignPaths,
    project_source: Option<Vec<u8>>,
    pcb_source: Option<Arc<str>>,
}

#[derive(Debug)]
pub(crate) struct PreparedBoardPlotSources {
    source_path: PathBuf,
    pub(crate) source: Arc<str>,
    net_classes: BoardNetClassAssignments,
    text_variables: BoardTextVariables,
    project_sidecars_ns: u64,
}

#[derive(Debug)]
pub struct StructuredDesignFacts {
    pub compiled_schematic_graph: CompiledSchematicGraphA0,
    pub netlist: KiCadNetlist,
    pub netlist_json: serde_json::Value,
    pub design_json: serde_json::Value,
    pub kicad_netlist: String,
    pub schematic_instances: Vec<KiCadSchematicInstance>,
}

#[derive(Debug)]
struct DesignPaths {
    input: PathBuf,
    root: PathBuf,
    project: Option<PathBuf>,
    root_schematic: PathBuf,
    pcb: Option<PathBuf>,
}

fn resolve_design_paths(input: &Path) -> Result<DesignPaths, DesignError> {
    let input_path = input
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve design input", error))?;
    if !input_path.is_file() {
        return Err(DesignError::new("design input is not a regular file"));
    }

    let suffix = input_path.extension().and_then(|value| value.to_str());
    let (project_path, root_schematic_path, pcb_path) =
        if suffix.is_some_and(|value| value.eq_ignore_ascii_case("kicad_pro")) {
            let schematic = input_path.with_extension("kicad_sch");
            if !schematic.is_file() {
                return Err(DesignError::new(format!(
                    "project root schematic was not found: {}",
                    schematic.display()
                )));
            }
            let pcb = input_path.with_extension("kicad_pcb");
            (
                Some(input_path.clone()),
                schematic,
                pcb.is_file().then_some(pcb),
            )
        } else if suffix.is_some_and(|value| value.eq_ignore_ascii_case("kicad_sch")) {
            (
                find_adjacent_project(&input_path)?,
                input_path.clone(),
                None,
            )
        } else {
            return Err(DesignError::new(
                "design input must end in .kicad_pro or .kicad_sch",
            ));
        };

    let bundle_root = input_path
        .parent()
        .ok_or_else(|| DesignError::new("design input has no parent directory"))?
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve bundle root", error))?;
    let root_schematic_path = root_schematic_path
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve root schematic", error))?;
    ensure_contained(&bundle_root, &root_schematic_path)?;
    Ok(DesignPaths {
        input: input_path,
        root: bundle_root,
        project: project_path,
        root_schematic: root_schematic_path,
        pcb: pcb_path,
    })
}

fn find_adjacent_project(source: &Path) -> Result<Option<PathBuf>, DesignError> {
    let exact = source.with_extension("kicad_pro");
    if exact.is_file() {
        return Ok(Some(exact));
    }
    let parent = source
        .parent()
        .ok_or_else(|| DesignError::new("design input has no parent directory"))?;
    let mut siblings = Vec::new();
    for entry in parent
        .read_dir()
        .map_err(|error| DesignError::context("could not inspect design directory", error))?
    {
        let path = entry
            .map_err(|error| DesignError::context("could not inspect design directory", error))?
            .path();
        if path.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("kicad_pro"))
        {
            if siblings.len() >= 4_096 {
                return Err(DesignError::new(
                    "design directory exceeds adjacent-project candidate limit",
                ));
            }
            siblings.push(path);
        }
    }
    siblings.sort();
    if siblings.len() == 1 {
        return Ok(siblings.pop());
    }
    let source_stem = source.file_stem().and_then(|stem| stem.to_str());
    Ok(source_stem.and_then(|stem| {
        siblings.into_iter().find(|candidate| {
            candidate
                .file_stem()
                .and_then(|candidate_stem| candidate_stem.to_str())
                .is_some_and(|candidate_stem| candidate_stem.eq_ignore_ascii_case(stem))
        })
    }))
}

pub fn load_design_sources(input: &Path) -> Result<LoadedDesignSources, DesignError> {
    let mut performance = PerformanceRecorder::new(false);
    let prepared = prepare_design_sources_internal(input, &mut performance)?;
    finish_design_sources_internal(prepared, &mut performance)
}

pub(crate) fn prepare_design_sources_profiled(
    input: &Path,
    performance: &mut PerformanceRecorder,
) -> Result<PreparedDesignSources, DesignError> {
    prepare_design_sources_internal(input, performance)
}

pub(crate) fn finish_design_sources_profiled(
    prepared: PreparedDesignSources,
    performance: &mut PerformanceRecorder,
) -> Result<LoadedDesignSources, DesignError> {
    finish_design_sources_internal(prepared, performance)
}

pub(crate) fn prepare_board_plot_sources(
    prepared: &PreparedDesignSources,
    profile_enabled: bool,
) -> Result<Option<PreparedBoardPlotSources>, DesignError> {
    let (Some(source_path), Some(source)) =
        (prepared.paths.pcb.as_ref(), prepared.pcb_source.as_ref())
    else {
        return Ok(None);
    };
    let started = profile_enabled.then(std::time::Instant::now);
    let (net_classes, text_variables) = board_project_sidecars(prepared.project_source.as_deref())?;
    Ok(Some(PreparedBoardPlotSources {
        source_path: source_path.clone(),
        source: Arc::clone(source),
        net_classes,
        text_variables,
        project_sidecars_ns: profile_elapsed_ns(started),
    }))
}

fn prepare_design_sources_internal(
    input: &Path,
    performance: &mut PerformanceRecorder,
) -> Result<PreparedDesignSources, DesignError> {
    let paths_started = performance.start();
    let paths = resolve_design_paths(input)?;
    performance.finish_detail("load_design_sources", "resolve_design_paths", paths_started);
    let limits = SourceBundleLimits::default();
    let project_started = performance.start();
    let project_source = paths
        .project
        .as_ref()
        .map(|path| read_bounded(path, limits.max_source_bytes))
        .transpose()?;
    performance.finish_detail(
        "load_design_sources",
        "read_project_source",
        project_started,
    );
    let pcb_started = performance.start();
    let pcb_source = paths
        .pcb
        .as_ref()
        .map(|path| {
            let bytes = read_bounded(path, limits.max_source_bytes)?;
            String::from_utf8(bytes)
                .map(Arc::<str>::from)
                .map_err(|error| DesignError::context("PCB source is not UTF-8", error))
        })
        .transpose()?;
    performance.finish_detail("load_design_sources", "read_pcb_source", pcb_started);
    Ok(PreparedDesignSources {
        paths,
        project_source,
        pcb_source,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "source loading records bounded parser and carrier sub-stages in one flow"
)]
fn finish_design_sources_internal(
    prepared: PreparedDesignSources,
    performance: &mut PerformanceRecorder,
) -> Result<LoadedDesignSources, DesignError> {
    let PreparedDesignSources {
        paths,
        project_source,
        pcb_source,
    } = prepared;
    let limits = SourceBundleLimits::default();
    let schematic_limits = SchematicDocumentLimits {
        parse: SchematicBundleLimits::default(),
        max_output_bytes: limits.max_source_bytes,
    };
    let mut source_rows = Vec::new();
    let mut buffers = Vec::new();
    let mut total_bytes = 0_usize;

    if let Some((project_path, bytes)) = paths.project.as_ref().zip(project_source) {
        let relative = portable_relative(&paths.root, project_path)?;
        push_source(
            &mut source_rows,
            &mut buffers,
            &mut total_bytes,
            relative,
            SourceKind::Project,
            bytes,
            limits,
        )?;
    }

    let root_relative = portable_relative(&paths.root, &paths.root_schematic)?;
    let mut pending = VecDeque::from([paths.root_schematic.clone()]);
    let mut seen = HashSet::new();
    let mut schematic_read_elapsed = std::time::Duration::ZERO;
    let schematic_parse_elapsed = std::time::Duration::ZERO;
    let mut definition_elapsed = std::time::Duration::ZERO;
    let mut discovery_elapsed = std::time::Duration::ZERO;
    let mut carrier_elapsed = std::time::Duration::ZERO;
    while let Some(source_path) = pending.pop_front() {
        let discovery_started = performance.start();
        let relative = portable_relative(&paths.root, &source_path)?;
        if !seen.insert(relative.clone()) {
            discovery_elapsed += performance.elapsed(discovery_started);
            continue;
        }
        discovery_elapsed += performance.elapsed(discovery_started);

        let read_started = performance.start();
        let bytes = read_bounded(&source_path, schematic_limits.parse.max_source_bytes)?;
        schematic_read_elapsed += performance.elapsed(read_started);

        let definition_started = performance.start();
        let (source, definition) = SchematicDocument::from_named_reader_with_definition(
            &relative,
            io::Cursor::new(bytes),
            schematic_limits,
        )
        .map_err(|error| DesignError::context("could not parse schematic", error))?;
        definition_elapsed += performance.elapsed(definition_started);

        let discovery_started = performance.start();
        let source_parent = source_path
            .parent()
            .ok_or_else(|| DesignError::new("schematic has no parent directory"))?;
        for sheet in definition.sheets {
            let child = resolve_child_schematic(&paths.root, source_parent, &sheet.sheet_file)?;
            pending.push_back(child);
        }
        discovery_elapsed += performance.elapsed(discovery_started);

        let carrier_started = performance.start();
        push_source(
            &mut source_rows,
            &mut buffers,
            &mut total_bytes,
            relative,
            SourceKind::Schematic,
            source.into_source().into_bytes(),
            limits,
        )?;
        carrier_elapsed += performance.elapsed(carrier_started);
    }
    performance.record_detail(
        "load_design_sources",
        "read_schematic_sources",
        schematic_read_elapsed,
    );
    performance.record_detail(
        "load_design_sources",
        "parse_schematic_documents",
        schematic_parse_elapsed,
    );
    performance.record_detail(
        "load_design_sources",
        "extract_schematic_definitions",
        definition_elapsed,
    );
    performance.record_detail(
        "load_design_sources",
        "discover_schematic_hierarchy",
        discovery_elapsed,
    );
    performance.record_detail(
        "load_design_sources",
        "insert_schematic_source_carriers",
        carrier_elapsed,
    );

    let bundle_started = performance.start();
    let project_relative = paths
        .project
        .as_ref()
        .map(|path| portable_relative(&paths.root, path))
        .transpose()?;
    let manifest = SourceBundleManifestA0 {
        project_path: project_relative,
        root_schematic_path: root_relative,
        schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
        sources: source_rows,
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    };
    let source_snapshot_sha256 = source_snapshot_sha256(&manifest, &buffers)?;
    let bundle = SourceBundle::from_manifest(manifest, buffers, limits)
        .map_err(|error| DesignError::context("source bundle is invalid", error))?;
    performance.finish_detail(
        "load_design_sources",
        "assemble_and_hash_source_bundle",
        bundle_started,
    );
    Ok(LoadedDesignSources {
        input_path: paths.input,
        bundle_root: paths.root,
        bundle,
        source_snapshot_sha256,
        pcb_path: paths.pcb,
        pcb_source,
    })
}

fn source_snapshot_sha256(
    manifest: &SourceBundleManifestA0,
    buffers: &[Vec<u8>],
) -> Result<String, DesignError> {
    let mut sources = manifest.sources.iter().collect::<Vec<_>>();
    sources.sort_by_key(|source| *source.slot);
    if sources.len() != buffers.len() {
        return Err(DesignError::new(
            "source snapshot manifest and loaded slots have different lengths",
        ));
    }
    let mut digest = Sha256::new();
    digest.update(b"kicad_monkey.native.source_snapshot.a1\0");
    digest_length_prefixed(&mut digest, manifest.root_schematic_path.as_bytes())?;
    if let Some(project_path) = manifest.project_path.as_deref() {
        digest.update([1]);
        digest_length_prefixed(&mut digest, project_path.as_bytes())?;
    } else {
        digest.update([0]);
    }
    digest_usize(&mut digest, sources.len(), "source snapshot count")?;
    for source in sources {
        let slot = *source.slot;
        let bytes = buffers
            .get(slot as usize)
            .ok_or_else(|| DesignError::new("source snapshot slot is out of range"))?;
        digest.update(slot.to_be_bytes());
        digest_length_prefixed(&mut digest, source.path.as_bytes())?;
        digest_length_prefixed(&mut digest, source.kind.to_string().as_bytes())?;
        digest_usize(&mut digest, bytes.len(), "source snapshot byte length")?;
        digest.update(bytes);
    }
    Ok(hex_encoded(&digest.finalize()))
}

fn digest_length_prefixed(digest: &mut Sha256, bytes: &[u8]) -> Result<(), DesignError> {
    digest_usize(digest, bytes.len(), "source snapshot string length")?;
    digest.update(bytes);
    Ok(())
}

fn digest_usize(digest: &mut Sha256, value: usize, label: &str) -> Result<(), DesignError> {
    let value = u64::try_from(value)
        .map_err(|_| DesignError::new(format!("{label} does not fit unsigned 64-bit")))?;
    digest.update(value.to_be_bytes());
    Ok(())
}

pub fn build_structured_design_facts(
    loaded: &LoadedDesignSources,
) -> Result<StructuredDesignFacts, DesignError> {
    build_structured_design_facts_with_options(loaded, true)
}

pub fn build_structured_design_facts_with_options(
    loaded: &LoadedDesignSources,
    include_indexes: bool,
) -> Result<StructuredDesignFacts, DesignError> {
    build_structured_design_facts_internal(
        loaded,
        include_indexes,
        &mut PerformanceRecorder::new(false),
    )
}

pub(crate) fn build_structured_design_facts_profiled(
    loaded: &LoadedDesignSources,
    include_indexes: bool,
    performance: &mut PerformanceRecorder,
) -> Result<StructuredDesignFacts, DesignError> {
    build_structured_design_facts_internal(loaded, include_indexes, performance)
}

#[allow(
    clippy::too_many_lines,
    reason = "facts construction records each bounded native derivation sub-stage"
)]
fn build_structured_design_facts_internal(
    loaded: &LoadedDesignSources,
    include_indexes: bool,
    performance: &mut PerformanceRecorder,
) -> Result<StructuredDesignFacts, DesignError> {
    let (index, index_profile) = if performance.is_enabled() {
        SchematicBundleIndex::build_profiled(&loaded.bundle, SchematicBundleLimits::default())
    } else {
        SchematicBundleIndex::build(&loaded.bundle, SchematicBundleLimits::default())
            .map(|index| (index, Default::default()))
    }
    .map_err(|error| DesignError::context("could not index schematic hierarchy", error))?;
    performance.record_detail(
        "build_structured_design_facts",
        "parse_schematic_index_definitions",
        std::time::Duration::from_nanos(index_profile.parse_definitions_ns),
    );
    performance.record_detail(
        "build_structured_design_facts",
        "realize_schematic_occurrences",
        std::time::Duration::from_nanos(index_profile.realize_occurrences_ns),
    );
    performance.record_detail(
        "build_structured_design_facts",
        "assemble_schematic_indexes",
        std::time::Duration::from_nanos(index_profile.assemble_indexes_ns),
    );
    let (design_facts, facts_profile) = if performance.is_enabled() {
        build_kicad_design_facts_profiled(
            &index,
            &loaded.bundle,
            ProjectLimits::default(),
            Default::default(),
            KiCadNetlistLimits::default(),
        )
    } else {
        build_kicad_design_facts(
            &index,
            &loaded.bundle,
            ProjectLimits::default(),
            Default::default(),
            KiCadNetlistLimits::default(),
        )
        .map(|facts| (facts, Default::default()))
    }
    .map_err(|error| DesignError::context("could not build structured design facts", error))?;
    performance.record_detail(
        "build_structured_design_facts",
        "parse_project_document",
        std::time::Duration::from_nanos(facts_profile.project_parse_ns),
    );
    performance.record_detail(
        "build_structured_design_facts",
        "build_compiled_schematic_graph",
        std::time::Duration::from_nanos(facts_profile.compiled_graph_ns),
    );
    performance.record_detail(
        "build_structured_design_facts",
        "build_kicad_netlist",
        std::time::Duration::from_nanos(facts_profile.netlist_ns),
    );
    let graph_validation_started = performance.start();
    validate_compiled_schematic_graph(design_facts.graph())
        .map_err(|error| DesignError::context("compiled schematic graph is invalid", error))?;
    performance.finish_detail(
        "build_structured_design_facts",
        "validate_compiled_schematic_graph",
        graph_validation_started,
    );
    let netlist = design_facts.netlist();
    let netlist_source =
        display_source_path(&loaded.bundle_root.join(loaded.bundle.root_schematic_path()));
    let netlist_emit_started = performance.start();
    let kicad_netlist = emit_kicad_netlist(
        netlist,
        &netlist_source,
        "",
        GRAPH_TOOL,
        KiCadNetlistLimits::default().max_output_bytes,
    )
    .map_err(|error| DesignError::context("could not emit KiCad netlist", error))?;
    performance.finish_detail(
        "build_structured_design_facts",
        "emit_kicad_netlist",
        netlist_emit_started,
    );
    let netlist_json_started = performance.start();
    let netlist_json = build_kicad_netlist_json(
        netlist,
        KiCadNetlistJsonMetadata {
            source: "",
            date: "",
            tool: "kicad_monkey",
        },
    );
    performance.finish_detail(
        "build_structured_design_facts",
        "build_kicad_netlist_json",
        netlist_json_started,
    );
    let pcb_parse_started = performance.start();
    let pcb_view = loaded
        .pcb_source
        .as_deref()
        .map(|source| PcbView::parse(source, PcbLimits::default()))
        .transpose()
        .map_err(|error| DesignError::context("could not parse PCB", error))?;
    performance.finish_detail(
        "build_structured_design_facts",
        "parse_pcb_view",
        pcb_parse_started,
    );
    let pcb_filename = loaded
        .pcb_path
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let pcb = pcb_view
        .as_ref()
        .zip(pcb_filename)
        .map(|(view, filename)| KiCadDesignPcb {
            source_filename: filename,
            view,
        });
    let json_paths = design_json_paths(loaded)?;
    let (design_json, design_json_profile) = if performance.is_enabled() {
        build_kicad_design_json_profiled(&index, &design_facts, &json_paths, pcb, include_indexes)
    } else {
        build_kicad_design_json(&index, &design_facts, &json_paths, pcb, include_indexes)
            .map(|value| (value, Default::default()))
    }
    .map_err(|error| DesignError::context("could not build KiCad design JSON", error))?;
    for (name, elapsed_ns) in [
        (
            "design_json_binding_and_preflight",
            design_json_profile.binding_and_preflight_ns,
        ),
        (
            "design_json_netlist_json",
            design_json_profile.netlist_json_ns,
        ),
        (
            "design_json_project_variants_options",
            design_json_profile.project_variants_options_ns,
        ),
        ("design_json_sheets", design_json_profile.sheets_ns),
        ("design_json_components", design_json_profile.components_ns),
        (
            "design_json_schematic_hierarchy_and_nets",
            design_json_profile.hierarchy_and_nets_ns,
        ),
        (
            "design_json_compiled_graph_value",
            design_json_profile.compiled_graph_value_ns,
        ),
        ("design_json_pnp", design_json_profile.pnp_ns),
        (
            "design_json_classes_and_indexes",
            design_json_profile.classes_and_indexes_ns,
        ),
        (
            "design_json_output_limit_serialization",
            design_json_profile.output_limit_serialization_ns,
        ),
    ] {
        performance.record_detail(
            "build_structured_design_facts",
            name,
            std::time::Duration::from_nanos(elapsed_ns),
        );
    }
    let instances_started = performance.start();
    let schematic_instances = design_facts
        .schematic_instances()
        .map_err(|error| DesignError::context("could not enumerate schematic instances", error))?;
    performance.finish_detail(
        "build_structured_design_facts",
        "enumerate_schematic_instances",
        instances_started,
    );
    let (compiled_schematic_graph, netlist) = design_facts.into_parts();
    Ok(StructuredDesignFacts {
        compiled_schematic_graph,
        netlist,
        netlist_json,
        design_json,
        kicad_netlist,
        schematic_instances,
    })
}

pub fn build_board_plot_document(
    loaded: &LoadedDesignSources,
) -> Result<Option<BoardPlotDocument>, DesignError> {
    build_board_plot_document_with_limits(loaded, BoardPlotDocumentLimits::default())
}

pub fn build_board_plot_document_with_limits(
    loaded: &LoadedDesignSources,
    limits: BoardPlotDocumentLimits,
) -> Result<Option<BoardPlotDocument>, DesignError> {
    build_board_plot_document_internal(loaded, limits, false).map(|(document, _profile)| document)
}

pub(crate) fn build_prepared_board_plot_document_profiled(
    prepared: PreparedBoardPlotSources,
    profile_enabled: bool,
) -> Result<(Option<BoardPlotDocument>, BoardPlotDocumentBuildProfile), DesignError> {
    let profile = BoardPlotDocumentBuildProfile {
        project_sidecars_ns: prepared.project_sidecars_ns,
        ..BoardPlotDocumentBuildProfile::default()
    };
    build_board_plot_document_with_resolved_sidecars(
        &prepared.source,
        &prepared.source_path,
        prepared.net_classes,
        prepared.text_variables,
        BoardPlotDocumentLimits::default(),
        profile_enabled,
        profile,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one source-bound board build keeps bounded projection and opt-in timings aligned"
)]
fn build_board_plot_document_internal(
    loaded: &LoadedDesignSources,
    limits: BoardPlotDocumentLimits,
    profile_enabled: bool,
) -> Result<(Option<BoardPlotDocument>, BoardPlotDocumentBuildProfile), DesignError> {
    build_board_plot_document_from_sources(
        loaded.pcb_source.as_deref(),
        loaded.pcb_path.as_deref(),
        loaded.bundle.project().map(|source| source.bytes()),
        limits,
        profile_enabled,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one source-bound board build keeps bounded projection and opt-in timings aligned"
)]
fn build_board_plot_document_from_sources(
    source: Option<&str>,
    source_path: Option<&Path>,
    project_source: Option<&[u8]>,
    limits: BoardPlotDocumentLimits,
    profile_enabled: bool,
) -> Result<(Option<BoardPlotDocument>, BoardPlotDocumentBuildProfile), DesignError> {
    let mut profile = BoardPlotDocumentBuildProfile::default();
    let Some(source) = source else {
        return Ok((None, profile));
    };
    let source_path = source_path.ok_or_else(|| DesignError::new("PCB source path is missing"))?;
    let project_started = profile_enabled.then(std::time::Instant::now);
    let (net_classes, text_variables) = board_project_sidecars(project_source)?;
    profile.project_sidecars_ns = profile_elapsed_ns(project_started);
    build_board_plot_document_with_resolved_sidecars(
        source,
        source_path,
        net_classes,
        text_variables,
        limits,
        profile_enabled,
        profile,
    )
}

struct BoardFontInputs {
    faces: BTreeSet<String>,
    embedded_files: Vec<SchematicEmbeddedFile>,
    face_scan_ns: u64,
    embedded_extraction_ns: u64,
}

fn board_font_inputs(source: &str, profile_enabled: bool) -> Result<BoardFontInputs, DesignError> {
    let faces_started = profile_enabled.then(std::time::Instant::now);
    let mut faces = BTreeSet::from(["Arial".to_owned()]);
    collect_font_faces(source, &mut faces)?;
    let face_scan_ns = profile_elapsed_ns(faces_started);
    let embedded_started = profile_enabled.then(std::time::Instant::now);
    let embedded_files = schematic_embedded_files(source, SchematicEmbeddedLimits::default())
        .map_err(|error| DesignError::context("could not extract board font sidecars", error))?;
    Ok(BoardFontInputs {
        faces,
        embedded_files,
        face_scan_ns,
        embedded_extraction_ns: profile_elapsed_ns(embedded_started),
    })
}

fn account_overlapped_pair(first: u64, second: u64, blocking: u64) -> (u64, u64) {
    let total = first.saturating_add(second);
    if total == 0 || blocking == 0 {
        return (0, 0);
    }
    let accounted = blocking.min(total);
    let first = u128::from(accounted)
        .saturating_mul(u128::from(first))
        .checked_div(u128::from(total))
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(accounted);
    (first, accounted.saturating_sub(first))
}

fn preflight_parallel_board_fonts(
    source: &str,
    limits: BoardPlotLimits,
) -> Result<(), DesignError> {
    if source.len() > limits.max_source_bytes {
        return Err(DesignError::new(
            "could not build board plot facts: board source exceeds max_source_bytes",
        ));
    }
    Ok(())
}

fn resolve_parallel_board_inputs<T, U>(
    facts: Result<T, DesignError>,
    fonts: std::thread::Result<Result<U, DesignError>>,
) -> Result<(T, U), DesignError> {
    let facts = facts?;
    let fonts = fonts.map_err(|_| DesignError::new("board font worker panicked"))??;
    Ok((facts, fonts))
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one source-bound board build keeps bounded projection and opt-in timings aligned"
)]
fn build_board_plot_document_with_resolved_sidecars(
    source: &str,
    source_path: &Path,
    net_classes: BoardNetClassAssignments,
    text_variables: BoardTextVariables,
    limits: BoardPlotDocumentLimits,
    profile_enabled: bool,
    mut profile: BoardPlotDocumentBuildProfile,
) -> Result<(Option<BoardPlotDocument>, BoardPlotDocumentBuildProfile), DesignError> {
    preflight_parallel_board_fonts(source, limits.plot)?;
    let ((facts, facts_profile), font_inputs, font_blocking_ns) = std::thread::scope(|scope| {
        let font_worker = scope.spawn(|| board_font_inputs(source, profile_enabled));
        let facts = if profile_enabled {
            board_plot_facts_with_sidecars_profiled(
                source,
                limits.plot,
                PcbLimits::default(),
                &net_classes,
                &text_variables,
            )
        } else {
            board_plot_facts_with_sidecars(
                source,
                limits.plot,
                PcbLimits::default(),
                &net_classes,
                &text_variables,
            )
            .map(|facts| (facts, BoardPlotFactsBuildProfile::default()))
        };
        let join_started = profile_enabled.then(std::time::Instant::now);
        let font_inputs = font_worker.join();
        let font_blocking_ns = profile_elapsed_ns(join_started);
        let facts =
            facts.map_err(|error| DesignError::context("could not build board plot facts", error));
        let (facts, font_inputs) = resolve_parallel_board_inputs(facts, font_inputs)?;
        Ok::<_, DesignError>((facts, font_inputs, font_blocking_ns))
    })?;
    profile.facts = facts_profile;
    let layers_started = profile_enabled.then(std::time::Instant::now);
    let mut copper_layers = Vec::new();
    for layer in facts.view().layers() {
        let layer =
            layer.map_err(|error| DesignError::context("could not enumerate PCB layers", error))?;
        if layer.name.ends_with(".Cu") {
            if copper_layers.len() == limits.max_copper_layers {
                return Err(DesignError::new("PCB copper layer count exceeds its limit"));
            }
            copper_layers.push(layer.name);
        }
    }
    profile.copper_layer_enumeration_ns = profile_elapsed_ns(layers_started);
    let (font_face_scan_accounted_ns, embedded_font_extraction_accounted_ns) =
        account_overlapped_pair(
            font_inputs.face_scan_ns,
            font_inputs.embedded_extraction_ns,
            font_blocking_ns,
        );
    profile.font_face_scan_ns = font_inputs.face_scan_ns;
    profile.font_face_scan_accounted_ns = font_face_scan_accounted_ns;
    profile.embedded_font_extraction_ns = font_inputs.embedded_extraction_ns;
    profile.embedded_font_extraction_accounted_ns = embedded_font_extraction_accounted_ns;
    let font_selection_started = profile_enabled.then(std::time::Instant::now);
    let font_styles = schematic_font_styles(&font_inputs.faces, &font_inputs.embedded_files)?;
    profile.font_index_and_selection_ns = profile_elapsed_ns(font_selection_started);
    let font_resources_started = profile_enabled.then(std::time::Instant::now);
    let fonts = font_styles
        .iter()
        .map(|style| {
            Ok(PlotterTextFont {
                face: &style.face,
                bold: style.bold,
                italic: style.italic,
                font_bytes: style.bytes.as_ref(),
                shaping: shaping_template(
                    &format!("board_{}_{}", style.name, style.sha256),
                    &style.sha256,
                )?,
                fake_bold: style.fake_bold,
                fake_italic: style.fake_italic,
            })
        })
        .collect::<Result<Vec<_>, DesignError>>()?;
    let text_resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    profile.font_resource_setup_ns = profile_elapsed_ns(font_resources_started);
    let bounds_started = profile_enabled.then(std::time::Instant::now);
    let bounds = facts
        .bounds(Some(&text_resources), limits.bounds)
        .map_err(|error| DesignError::context("could not bound board geometry", error))?;
    profile.bounds_ns = profile_elapsed_ns(bounds_started);
    let document = facts.into_document();
    let identity_started = profile_enabled.then(std::time::Instant::now);
    let source_sha256 = sha256_hex(source.as_bytes());
    let document_id = format!("pcb-sha256:{source_sha256}");
    profile.source_identity_ns = profile_elapsed_ns(identity_started);
    let projection_started = profile_enabled.then(std::time::Instant::now);
    let contract = project_board_plot_document_a0(
        document,
        Some(display_source_path(source_path)),
        document_id,
        limits.contract,
    )
    .map_err(|error| DesignError::context("could not project board plot document", error))?;
    profile.contract_projection_ns = profile_elapsed_ns(projection_started);
    let serialization_started = profile_enabled.then(std::time::Instant::now);
    let mut writer = LimitedSerializedSize {
        written: 0,
        limit: limits.max_contract_bytes,
    };
    serde_json::to_writer(&mut writer, &contract)
        .map_err(|error| DesignError::context("board plot contract exceeds its limit", error))?;
    profile.contract_serialization_ns = profile_elapsed_ns(serialization_started);
    let materialization_started = profile_enabled.then(std::time::Instant::now);
    let value = serde_json::to_value(contract).map_err(|error| {
        DesignError::context("could not materialize board plot document", error)
    })?;
    profile.contract_materialization_ns = profile_elapsed_ns(materialization_started);
    Ok((
        Some(BoardPlotDocument {
            value,
            source_path: display_source_path(source_path),
            source_sha256,
            copper_layers,
            bounds,
        }),
        profile,
    ))
}

fn board_project_sidecars(
    project_source: Option<&[u8]>,
) -> Result<(BoardNetClassAssignments, BoardTextVariables), DesignError> {
    let project = project_source
        .map(|source| ProjectDocument::from_reader(source, ProjectLimits::default()))
        .transpose()
        .map_err(|error| DesignError::context("could not parse board project settings", error))?;
    let Some(project) = project.as_ref().map(ProjectDocument::view) else {
        return Ok((
            BoardNetClassAssignments::default(),
            BoardTextVariables::default(),
        ));
    };
    let net_settings = project
        .net_settings()
        .map_err(|error| DesignError::context("could not read board net classes", error))?;
    let variables = project
        .text_variables()
        .map_err(|error| DesignError::context("could not read board text variables", error))?;
    Ok((
        BoardNetClassAssignments::from_entries(net_settings.assignments),
        BoardTextVariables::from_entries(variables),
    ))
}

pub fn build_schematic_plot_documents(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
) -> Result<Vec<serde_json::Value>, DesignError> {
    build_schematic_plot_documents_with_limits(
        loaded,
        instances,
        SchematicPlotDocumentsLimits::default(),
    )
}

pub fn build_schematic_plot_documents_with_limits(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
    limits: SchematicPlotDocumentsLimits,
) -> Result<Vec<serde_json::Value>, DesignError> {
    build_schematic_plot_document_artifacts_with_limits(loaded, instances, limits).map(
        |artifacts| {
            artifacts
                .into_iter()
                .map(|artifact| artifact.value)
                .collect()
        },
    )
}

pub fn build_schematic_plot_document_artifacts(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
) -> Result<Vec<SchematicPlotDocument>, DesignError> {
    build_schematic_plot_document_artifacts_with_limits(
        loaded,
        instances,
        SchematicPlotDocumentsLimits::default(),
    )
}

pub(crate) fn build_schematic_plot_document_artifacts_profiled(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
) -> Result<
    (
        Vec<SchematicPlotDocument>,
        SchematicPlotDocumentsBuildProfile,
    ),
    DesignError,
> {
    build_schematic_plot_document_artifacts_internal(
        loaded,
        instances,
        SchematicPlotDocumentsLimits::default(),
        true,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass keeps sidecar budgets and each occurrence-bound document together"
)]
pub fn build_schematic_plot_document_artifacts_with_limits(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
    limits: SchematicPlotDocumentsLimits,
) -> Result<Vec<SchematicPlotDocument>, DesignError> {
    build_schematic_plot_document_artifacts_internal(loaded, instances, limits, false)
        .map(|(documents, _profile)| documents)
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass keeps sidecar budgets and each occurrence-bound document together"
)]
fn build_schematic_plot_document_artifacts_internal(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
    limits: SchematicPlotDocumentsLimits,
    profile_enabled: bool,
) -> Result<
    (
        Vec<SchematicPlotDocument>,
        SchematicPlotDocumentsBuildProfile,
    ),
    DesignError,
> {
    if instances.len() > limits.max_documents {
        return Err(DesignError::new(format!(
            "schematic plot document count exceeds its limit: {} > {}",
            instances.len(),
            limits.max_documents
        )));
    }
    if limits.max_total_output_bytes < 2 {
        return Err(DesignError::new(
            "schematic plot aggregate output limit is smaller than an empty JSON array",
        ));
    }
    let mut profile = SchematicPlotDocumentsBuildProfile::default();
    let embedded_started = profile_enabled.then(std::time::Instant::now);
    let embedded_files = design_embedded_files(loaded)?;
    profile.embedded_sidecars_ns = profile_elapsed_ns(embedded_started);
    let project_started = profile_enabled.then(std::time::Instant::now);
    let (variables, drawing_settings, worksheet_source) =
        project_plot_sidecars(loaded, &embedded_files)?;
    profile.project_sidecars_ns = profile_elapsed_ns(project_started);
    let faces_started = profile_enabled.then(std::time::Instant::now);
    let font_faces = requested_font_faces(loaded, worksheet_source.as_deref())?;
    profile.requested_font_faces_ns = profile_elapsed_ns(faces_started);
    let font_selection_started = profile_enabled.then(std::time::Instant::now);
    let font_styles = schematic_font_styles(&font_faces, &embedded_files)?;
    profile.font_index_and_selection_ns = profile_elapsed_ns(font_selection_started);
    let font_resources_started = profile_enabled.then(std::time::Instant::now);
    let mut fonts = Vec::with_capacity(font_styles.len());
    for style in &font_styles {
        fonts.push(PlotterTextFont {
            face: &style.face,
            bold: style.bold,
            italic: style.italic,
            font_bytes: style.bytes.as_ref(),
            shaping: shaping_template(
                &format!("schematic_{}_{}", style.name, style.sha256),
                &style.sha256,
            )?,
            fake_bold: style.fake_bold,
            fake_italic: style.fake_italic,
        });
    }
    fonts.sort_by(|left, right| {
        (left.face, left.bold, left.italic).cmp(&(right.face, right.bold, right.italic))
    });
    let text_resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    profile.font_resource_setup_ns = profile_elapsed_ns(font_resources_started);
    let mut documents = Vec::with_capacity(instances.len());
    let mut total_output_bytes = 2_usize;
    let mut batch_budget = SchematicPlotContractBudget {
        derived_items: 0,
        materialized_bytes: 0,
    };
    for (document_index, instance) in instances.iter().enumerate() {
        if document_index != 0 {
            total_output_bytes = total_output_bytes.checked_add(1).ok_or_else(|| {
                DesignError::new("schematic plot aggregate output byte count overflowed")
            })?;
        }
        let source = loaded
            .bundle
            .source(&instance.source_path)
            .map_err(|error| DesignError::context("could not resolve schematic source", error))?
            .ok_or_else(|| {
                DesignError::new(format!(
                    "schematic instance source is missing: {}",
                    instance.source_path
                ))
            })?;
        let source_path = loaded.bundle_root.join(source.path());
        let context = SchematicPlotContext {
            source_path: Some(display_source_path(&source_path)),
            document_id: Some(instance.document_id.clone()),
            sheet_index: instance.sheet_number,
            sheet_count: instance.sheet_count,
            sheet_path: instance.sheet_path.clone(),
            sheet_instance_path: instance.sheet_instance_path.clone(),
            sheet_name: instance.sheet_name.clone(),
            project_variables: variables.clone(),
            worksheet_source: worksheet_source.clone(),
        };
        let source_text = source
            .text()
            .map_err(|error| DesignError::context("schematic source is not UTF-8", error))?;
        let (document, document_profile) = if profile_enabled {
            schematic_plot_document_with_sheets_profiled(
                source_text,
                SchematicPlotLimits::default(),
                &context,
                drawing_settings,
                Some(&text_resources),
            )
        } else {
            schematic_plot_document_with_sheets(
                source_text,
                SchematicPlotLimits::default(),
                &context,
                drawing_settings,
                Some(&text_resources),
            )
            .map(|document| (document, SchematicPlotBuildProfile::default()))
        }
        .map_err(|error| DesignError::context("could not build schematic plot document", error))?;
        profile.add_plot_ir(document_profile);
        let budget_started = profile_enabled.then(std::time::Instant::now);
        let document_budget = schematic_plot_document_budget(&document).map_err(|error| {
            DesignError::context("could not budget schematic plot document", error)
        })?;
        charge_plot_batch_budget(&mut batch_budget, document_budget, limits)?;
        profile.plot_contract_budget_ns = profile
            .plot_contract_budget_ns
            .saturating_add(profile_elapsed_ns(budget_started));
        let projection_started = profile_enabled.then(std::time::Instant::now);
        let value =
            schematic_plot_document_json(&document, limits.per_document).map_err(|error| {
                DesignError::context("could not project schematic plot document", error)
            })?;
        profile.plot_json_projection_ns = profile
            .plot_json_projection_ns
            .saturating_add(profile_elapsed_ns(projection_started));
        let serialization_started = profile_enabled.then(std::time::Instant::now);
        let mut writer =
            AggregateLimitedWriter::new(&mut total_output_bytes, limits.max_total_output_bytes);
        serde_json::to_writer(&mut writer, &value).map_err(|error| {
            DesignError::context("schematic plot aggregate output limit exceeded", error)
        })?;
        profile.aggregate_output_serialization_ns = profile
            .aggregate_output_serialization_ns
            .saturating_add(profile_elapsed_ns(serialization_started));
        documents.push(SchematicPlotDocument {
            value,
            instance: instance.clone(),
        });
    }
    Ok((documents, profile))
}

fn profile_elapsed_ns(started: Option<std::time::Instant>) -> u64 {
    started.map_or(0, |started| {
        u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
    })
}

pub fn build_schematic_base_svgs(
    documents: &[serde_json::Value],
) -> Result<Vec<SchematicBaseSvg>, DesignError> {
    build_schematic_base_svgs_bound(documents, SchematicSvgDocumentsLimits::default())
}

pub fn build_schematic_base_svgs_for_plot_documents(
    documents: &[SchematicPlotDocument],
) -> Result<Vec<SchematicBaseSvg>, DesignError> {
    build_schematic_base_svgs_for_plot_documents_with_limits(
        documents,
        SchematicSvgDocumentsLimits::default(),
    )
}

pub fn build_schematic_base_svgs_for_plot_documents_with_limits(
    documents: &[SchematicPlotDocument],
    limits: SchematicSvgDocumentsLimits,
) -> Result<Vec<SchematicBaseSvg>, DesignError> {
    if documents.len() > limits.max_documents {
        return Err(DesignError::new(
            "schematic SVG document count exceeds its limit",
        ));
    }
    let values = documents
        .iter()
        .map(|document| &document.value)
        .collect::<Vec<_>>();
    build_schematic_base_svg_values(&values, limits, false).map(|(artifacts, _profile)| artifacts)
}

pub(crate) fn build_schematic_base_svgs_for_plot_documents_profiled(
    documents: &[SchematicPlotDocument],
    limits: SchematicSvgDocumentsLimits,
) -> Result<(Vec<SchematicBaseSvg>, SchematicBaseSvgBuildProfile), DesignError> {
    if documents.len() > limits.max_documents {
        return Err(DesignError::new(
            "schematic SVG document count exceeds its limit",
        ));
    }
    let values = documents
        .iter()
        .map(|document| &document.value)
        .collect::<Vec<_>>();
    build_schematic_base_svg_values(&values, limits, true)
}

pub fn build_schematic_base_svgs_with_limits(
    documents: &[serde_json::Value],
    limits: SchematicSvgDocumentsLimits,
) -> Result<Vec<SchematicBaseSvg>, DesignError> {
    build_schematic_base_svgs_bound(documents, limits)
}

fn build_schematic_base_svgs_bound(
    documents: &[serde_json::Value],
    limits: SchematicSvgDocumentsLimits,
) -> Result<Vec<SchematicBaseSvg>, DesignError> {
    let values = documents.iter().collect::<Vec<_>>();
    build_schematic_base_svg_values(&values, limits, false).map(|(artifacts, _profile)| artifacts)
}

fn build_schematic_base_svg_values(
    documents: &[&serde_json::Value],
    limits: SchematicSvgDocumentsLimits,
    profile_enabled: bool,
) -> Result<(Vec<SchematicBaseSvg>, SchematicBaseSvgBuildProfile), DesignError> {
    if documents.len() > limits.max_documents {
        return Err(DesignError::new(format!(
            "schematic SVG document count exceeds its limit: {} > {}",
            documents.len(),
            limits.max_documents
        )));
    }
    let mut artifacts = Vec::with_capacity(documents.len());
    let mut total_svg_bytes = 0_usize;
    let mut profile = SchematicBaseSvgBuildProfile::default();
    for document in documents {
        let source_path = document["source_path"]
            .as_str()
            .ok_or_else(|| DesignError::new("schematic plot source path is missing"))?;
        let remaining = limits
            .max_total_svg_bytes
            .checked_sub(total_svg_bytes)
            .ok_or_else(|| DesignError::new("schematic SVG aggregate byte limit exceeded"))?;
        let request_started = profile_enabled.then(std::time::Instant::now);
        let request = schematic_svg_request((*document).clone(), &limits.per_document, remaining)?;
        profile.request_projection_ns = profile
            .request_projection_ns
            .saturating_add(profile_elapsed_ns(request_started));
        let render_started = profile_enabled.then(std::time::Instant::now);
        let artifact = render_svg(&request)
            .map_err(|error| DesignError::context("could not render schematic base SVG", error))?;
        profile.native_render_ns = profile
            .native_render_ns
            .saturating_add(profile_elapsed_ns(render_started));
        total_svg_bytes = total_svg_bytes
            .checked_add(artifact.metrics.svg_bytes)
            .ok_or_else(|| DesignError::new("schematic SVG aggregate byte count overflowed"))?;
        let identity_started = profile_enabled.then(std::time::Instant::now);
        artifacts.push(SchematicBaseSvg {
            document_id: artifact.document_id,
            source_path: source_path.to_owned(),
            plot_document_sha256: json_sha256(document)?,
            svg: artifact.svg,
            metrics: artifact.metrics,
        });
        profile.artifact_identity_ns = profile
            .artifact_identity_ns
            .saturating_add(profile_elapsed_ns(identity_started));
    }
    Ok((artifacts, profile))
}

fn json_sha256(value: &serde_json::Value) -> Result<String, DesignError> {
    let mut hasher = Sha256::new();
    serde_json::to_writer(&mut DigestWriter(&mut hasher), value)
        .map_err(|error| DesignError::context("could not fingerprint plot document", error))?;
    Ok(hex_bytes(&hasher.finalize()))
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

struct DigestWriter<'a>(&'a mut Sha256);

impl io::Write for DigestWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn schematic_svg_request(
    document: serde_json::Value,
    configured: &SchematicSvgRenderLimits,
    remaining_svg_bytes: usize,
) -> Result<NativeSvgRenderRequestA0, DesignError> {
    let canvas = document
        .get("canvas")
        .ok_or_else(|| DesignError::new("schematic plot document canvas is missing"))?;
    let width_nm = positive_svg_dimension(canvas, "width_nm")?;
    let height_nm = positive_svg_dimension(canvas, "height_nm")?;
    let max_svg_bytes = remaining_svg_bytes.min(configured.max_svg_bytes);
    let per_document = NativeSvgRenderLimits {
        max_block_depth: configured.max_block_depth,
        max_image_encoded_bytes: svg_uint(configured.max_image_encoded_bytes),
        max_operations: configured.max_operations,
        max_points: svg_uint(configured.max_points),
        max_records: configured.max_records,
        max_render_work: svg_uint(configured.max_render_work),
        max_result_bytes: svg_uint(max_svg_bytes),
        max_svg_bytes: svg_uint(max_svg_bytes),
        max_svg_elements: svg_uint(configured.max_svg_elements),
        max_text_bytes: svg_uint(configured.max_text_bytes),
    };
    Ok(NativeSvgRenderRequestA0 {
        document: NativeSvgPlotDocument::SchematicSvgDocument(NativeSchematicSvgDocument {
            kind: "schematic".to_owned(),
            value: document,
        }),
        limits: per_document,
        profile: "plotter-base-a0".to_owned(),
        type_: "kicad_monkey.native.svg.request".to_owned(),
        version: "a0".to_owned(),
        viewport: NativeSvgViewport {
            height_nm,
            min_x_nm: kicad_monkey_contracts::JavaScriptSafeInteger::try_from(0_i64)
                .expect("zero is a JavaScript-safe integer"),
            min_y_nm: kicad_monkey_contracts::JavaScriptSafeInteger::try_from(0_i64)
                .expect("zero is a JavaScript-safe integer"),
            width_nm,
        },
    })
}

fn positive_svg_dimension(
    canvas: &serde_json::Value,
    name: &str,
) -> Result<NativeSvgPositiveSafeInteger, DesignError> {
    let value = canvas
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| DesignError::new(format!("schematic plot canvas {name} is missing")))?;
    let value = std::num::NonZeroU64::new(value).ok_or_else(|| {
        DesignError::new(format!("schematic plot canvas {name} must be positive"))
    })?;
    Ok(NativeSvgPositiveSafeInteger(value))
}

fn svg_uint(value: usize) -> SvgUint64 {
    SvgUint64(value.to_string())
}

fn charge_plot_batch_budget(
    total: &mut SchematicPlotContractBudget,
    document: SchematicPlotContractBudget,
    limits: SchematicPlotDocumentsLimits,
) -> Result<(), DesignError> {
    let derived_items = total
        .derived_items
        .checked_add(document.derived_items)
        .ok_or_else(|| DesignError::new("schematic plot aggregate item count overflowed"))?;
    let materialized_bytes = total
        .materialized_bytes
        .checked_add(document.materialized_bytes)
        .ok_or_else(|| DesignError::new("schematic plot aggregate byte count overflowed"))?;
    if derived_items > limits.max_total_derived_items {
        return Err(DesignError::new(format!(
            "schematic plot aggregate derived item limit exceeded: {derived_items} > {}",
            limits.max_total_derived_items
        )));
    }
    if materialized_bytes > limits.max_total_materialized_bytes {
        return Err(DesignError::new(format!(
            "schematic plot aggregate materialized byte limit exceeded: {materialized_bytes} > {}",
            limits.max_total_materialized_bytes
        )));
    }
    total.derived_items = derived_items;
    total.materialized_bytes = materialized_bytes;
    Ok(())
}

fn project_plot_sidecars(
    loaded: &LoadedDesignSources,
    embedded_files: &[SchematicEmbeddedFile],
) -> Result<
    (
        SchematicPlotVariables,
        SchematicDrawingSettings,
        Option<Vec<u8>>,
    ),
    DesignError,
> {
    let project = loaded
        .bundle
        .project()
        .map(|source| ProjectDocument::from_reader(source.bytes(), ProjectLimits::default()))
        .transpose()
        .map_err(|error| {
            DesignError::context("could not parse schematic project settings", error)
        })?;
    let project = project.as_ref().map(ProjectDocument::view);
    let variables = project
        .map(|view| view.text_variables())
        .transpose()
        .map_err(|error| DesignError::context("could not read schematic text variables", error))?
        .unwrap_or_default();
    let drawing_settings = project.map_or_else(SchematicDrawingSettings::default, |view| {
        view.schematic_drawing_settings()
    });
    let worksheet_source = project
        .and_then(|view| view.get_path("schematic.page_layout_descr_file"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| load_project_worksheet(loaded, embedded_files, value))
        .transpose()?
        .flatten();
    Ok((
        SchematicPlotVariables::from_entries(variables),
        drawing_settings,
        worksheet_source,
    ))
}

fn design_embedded_files(
    loaded: &LoadedDesignSources,
) -> Result<Vec<SchematicEmbeddedFile>, DesignError> {
    let limits = SchematicEmbeddedLimits::default();
    let mut files = Vec::new();
    let mut decoded_bytes = 0_usize;
    for source in loaded.bundle.sources() {
        if source.kind() != SourceKind::Schematic {
            continue;
        }
        let extracted = schematic_embedded_files(
            source
                .text()
                .map_err(|error| DesignError::context("schematic source is not UTF-8", error))?,
            limits,
        )
        .map_err(|error| DesignError::context("could not extract schematic sidecars", error))?;
        for file in extracted {
            decoded_bytes = decoded_bytes
                .checked_add(file.bytes.len())
                .ok_or_else(|| DesignError::new("design embedded sidecar byte count overflowed"))?;
            if files.len() >= limits.max_files || decoded_bytes > limits.max_decoded_bytes {
                return Err(DesignError::new(
                    "design embedded sidecars exceed their aggregate limit",
                ));
            }
            files.push(file);
        }
    }
    Ok(files)
}

struct AggregateLimitedWriter<'a> {
    written: &'a mut usize,
    limit: usize,
}

impl<'a> AggregateLimitedWriter<'a> {
    const fn new(written: &'a mut usize, limit: usize) -> Self {
        Self { written, limit }
    }
}

impl io::Write for AggregateLimitedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(buffer.len())
            .filter(|next| *next <= self.limit)
            .ok_or_else(|| io::Error::other(format!("output exceeds {} bytes", self.limit)))?;
        *self.written = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn load_project_worksheet(
    loaded: &LoadedDesignSources,
    embedded_files: &[SchematicEmbeddedFile],
    worksheet: &str,
) -> Result<Option<Vec<u8>>, DesignError> {
    if let Some(name) = worksheet.strip_prefix("kicad-embed://") {
        return Ok(embedded_files
            .iter()
            .find(|file| file.file_type == "worksheet" && file.name == name)
            .map(|file| file.bytes.clone()));
    }
    let relative = Path::new(worksheet);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, Component::Prefix(_)))
    {
        return Err(DesignError::new(
            "project worksheet path must remain inside the design bundle",
        ));
    }
    let unresolved = loaded.bundle_root.join(relative);
    if !unresolved.exists() {
        return Ok(None);
    }
    let path = unresolved.canonicalize().map_err(|error| {
        DesignError::context(
            &format!("could not resolve project worksheet {worksheet}"),
            error,
        )
    })?;
    ensure_contained(&loaded.bundle_root, &path)?;
    read_bounded(&path, SchematicPlotLimits::default().max_worksheet_bytes).map(Some)
}

fn requested_font_faces(
    loaded: &LoadedDesignSources,
    worksheet_source: Option<&[u8]>,
) -> Result<BTreeSet<String>, DesignError> {
    let mut faces = BTreeSet::from([String::new(), "Arial".to_owned()]);
    for source in loaded.bundle.sources() {
        if source.kind() == SourceKind::Schematic {
            collect_font_faces(
                source.text().map_err(|error| {
                    DesignError::context("schematic source is not UTF-8", error)
                })?,
                &mut faces,
            )?;
        }
    }
    if let Some(source) = worksheet_source {
        let source = std::str::from_utf8(source)
            .map_err(|error| DesignError::context("worksheet source is not UTF-8", error))?;
        collect_font_faces(source, &mut faces)?;
    }
    Ok(faces)
}

fn collect_font_faces(source: &str, faces: &mut BTreeSet<String>) -> Result<(), DesignError> {
    const MAX_FACES: usize = 64;
    const MAX_FACE_BYTES: usize = 64 * 1024;
    let mut lexer = kicad_monkey_core::Lexer::new(source);
    let mut expecting_head = false;
    let mut expecting_face = false;
    while let Some(token) = lexer
        .next()
        .transpose()
        .map_err(|error| DesignError::context("could not scan plot font faces", error))?
    {
        match token.kind {
            TokenKind::Left => {
                expecting_head = true;
                expecting_face = false;
            }
            TokenKind::Right => {
                expecting_head = false;
                expecting_face = false;
            }
            _ if expecting_head => {
                expecting_face = token.lexeme == "face";
                expecting_head = false;
            }
            TokenKind::QuotedString if expecting_face => {
                expecting_face = false;
                let face = serde_json::from_str::<String>(token.lexeme).map_err(|error| {
                    DesignError::context("could not decode plot font face", error)
                })?;
                if face.len() > MAX_FACE_BYTES {
                    return Err(DesignError::new("plot font face exceeds its byte limit"));
                }
                faces.insert(face);
                if faces.len() > MAX_FACES {
                    return Err(DesignError::new("plot font face count exceeds its limit"));
                }
                if faces.iter().map(String::len).sum::<usize>() > MAX_FACE_BYTES {
                    return Err(DesignError::new(
                        "plot font face names exceed their aggregate byte limit",
                    ));
                }
            }
            _ => expecting_face = false,
        }
    }
    Ok(())
}

fn schematic_font_styles<'a>(
    faces: &BTreeSet<String>,
    embedded_files: &'a [SchematicEmbeddedFile],
) -> Result<Vec<SchematicFontStyle<'a>>, DesignError> {
    schematic_font_styles_with_limits(faces, embedded_files, PlotterTextCacheLimits::default())
}

fn schematic_font_styles_with_limits<'a>(
    faces: &BTreeSet<String>,
    embedded_files: &'a [SchematicEmbeddedFile],
    font_limits: PlotterTextCacheLimits,
) -> Result<Vec<SchematicFontStyle<'a>>, DesignError> {
    let embedded_fonts = embedded_font_index(embedded_files)?;
    let mut styles = Vec::with_capacity(faces.len().saturating_mul(4));
    let mut retained_font_bytes = 0_usize;
    for face in faces {
        for (bold, italic, name) in [
            (false, false, "regular"),
            (false, true, "italic"),
            (true, false, "bold"),
            (true, true, "bold_italic"),
        ] {
            let style = resolve_schematic_font(face, bold, italic, name, &embedded_fonts)?;
            retained_font_bytes = retained_font_bytes
                .checked_add(style.bytes.len())
                .filter(|bytes| *bytes <= font_limits.max_font_bytes)
                .ok_or_else(|| DesignError::new("plot font bytes exceed their aggregate limit"))?;
            styles.push(style);
        }
    }
    Ok(styles)
}

fn resolve_schematic_font<'a>(
    face: &str,
    bold: bool,
    italic: bool,
    name: &str,
    embedded_fonts: &[EmbeddedFontCandidate<'a>],
) -> Result<SchematicFontStyle<'a>, DesignError> {
    if let Some((candidate, selected_bold, selected_italic)) =
        embedded_font_selection(face, bold, italic, embedded_fonts)
    {
        let file = candidate.file;
        return Ok(SchematicFontStyle {
            face: face.to_owned(),
            bold,
            italic,
            name: name.to_owned(),
            bytes: Cow::Borrowed(&file.bytes),
            sha256: candidate.sha256.clone(),
            fake_bold: bold && !selected_bold,
            fake_italic: italic && !selected_italic,
        });
    }
    let system_root = std::env::var_os("SystemRoot").map(PathBuf::from);
    if let Some(font_dir) = system_root.map(|root| root.join("Fonts")) {
        let (filename, selected_bold, selected_italic) =
            windows_font_selection(face, bold, italic, &font_dir);
        let path = font_dir.join(filename);
        if path.is_file() {
            let bytes = read_bounded(&path, PlotterTextCacheLimits::default().max_face_bytes)?;
            let sha256 = sha256_hex(&bytes);
            return Ok(SchematicFontStyle {
                face: face.to_owned(),
                bold,
                italic,
                name: name.to_owned(),
                bytes: Cow::Owned(bytes),
                sha256,
                fake_bold: bold && !selected_bold,
                fake_italic: italic && !selected_italic,
            });
        }
    }
    let source = match (bold, italic) {
        (false, false) => KICAD_STROKE_REGULAR,
        (false, true) => KICAD_STROKE_ITALIC,
        (true, false) => KICAD_STROKE_BOLD,
        (true, true) => KICAD_STROKE_BOLD_ITALIC,
    };
    let sha256 = sha256_hex(source);
    Ok(SchematicFontStyle {
        face: face.to_owned(),
        bold,
        italic,
        name: name.to_owned(),
        bytes: Cow::Borrowed(source),
        sha256,
        fake_bold: false,
        fake_italic: false,
    })
}

fn embedded_font_selection<'index, 'files>(
    requested_face: &str,
    bold: bool,
    italic: bool,
    embedded_fonts: &'index [EmbeddedFontCandidate<'files>],
) -> Option<(&'index EmbeddedFontCandidate<'files>, bool, bool)> {
    let requested = normalized_font_name(requested_face);
    if requested.is_empty() {
        return None;
    }
    for (candidate_bold, candidate_italic) in font_style_lookup_order(bold, italic) {
        if let Some(candidate) = embedded_fonts.iter().rev().find(|candidate| {
            candidate.bold == candidate_bold
                && candidate.italic == candidate_italic
                && candidate.families.iter().any(|family| family == &requested)
        }) {
            return Some((candidate, candidate_bold, candidate_italic));
        }
    }
    None
}

fn embedded_font_index(
    embedded_files: &[SchematicEmbeddedFile],
) -> Result<Vec<EmbeddedFontCandidate<'_>>, DesignError> {
    const MAX_NAME_RECORDS: usize = 65_536;
    const MAX_FAMILY_ALIASES: usize = 4_096;
    const MAX_FAMILY_BYTES: usize = 256 * 1024;
    let mut name_records = 0_usize;
    let mut family_aliases = 0_usize;
    let mut family_bytes = 0_usize;
    let mut candidates = Vec::new();
    for file in embedded_files
        .iter()
        .filter(|file| file.file_type.eq_ignore_ascii_case("font"))
    {
        if file.bytes.len() > PlotterTextCacheLimits::default().max_face_bytes {
            return Err(DesignError::new(
                "embedded font face exceeds its byte limit",
            ));
        }
        let Ok(face) = ttf_parser::Face::parse(&file.bytes, 0) else {
            continue;
        };
        let mut families = BTreeSet::new();
        for name in face.names() {
            name_records = name_records
                .checked_add(1)
                .filter(|count| *count <= MAX_NAME_RECORDS)
                .ok_or_else(|| DesignError::new("embedded font name records exceed their limit"))?;
            if !matches!(
                name.name_id,
                ttf_parser::name_id::FAMILY | ttf_parser::name_id::TYPOGRAPHIC_FAMILY
            ) {
                continue;
            }
            if let Some(name) = name.to_string() {
                push_font_family(
                    &mut families,
                    &name,
                    &mut family_aliases,
                    &mut family_bytes,
                    MAX_FAMILY_ALIASES,
                    MAX_FAMILY_BYTES,
                )?;
            }
        }
        if let Some(stem) = Path::new(&file.name)
            .file_stem()
            .and_then(|stem| stem.to_str())
        {
            push_font_family(
                &mut families,
                strip_font_style_suffix(stem),
                &mut family_aliases,
                &mut family_bytes,
                MAX_FAMILY_ALIASES,
                MAX_FAMILY_BYTES,
            )?;
        }
        if !families.is_empty() {
            candidates.push(EmbeddedFontCandidate {
                file,
                families: families.into_iter().collect(),
                bold: face.is_bold(),
                italic: face.is_italic(),
                sha256: sha256_hex(&file.bytes),
            });
        }
    }
    Ok(candidates)
}

fn push_font_family(
    families: &mut BTreeSet<String>,
    value: &str,
    aliases: &mut usize,
    bytes: &mut usize,
    max_aliases: usize,
    max_bytes: usize,
) -> Result<(), DesignError> {
    let normalized = normalized_font_name(value);
    if normalized.is_empty() || families.contains(&normalized) {
        return Ok(());
    }
    *aliases = aliases
        .checked_add(1)
        .filter(|count| *count <= max_aliases)
        .ok_or_else(|| DesignError::new("embedded font family aliases exceed their limit"))?;
    *bytes = bytes
        .checked_add(normalized.len())
        .filter(|count| *count <= max_bytes)
        .ok_or_else(|| DesignError::new("embedded font family names exceed their byte limit"))?;
    families.insert(normalized);
    Ok(())
}

fn font_style_lookup_order(bold: bool, italic: bool) -> Vec<(bool, bool)> {
    let mut order = vec![(bold, italic)];
    if bold && italic {
        order.extend([(true, false), (false, true)]);
    }
    order.push((false, false));
    order.dedup();
    order
}

fn normalized_font_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn strip_font_style_suffix(value: &str) -> &str {
    const SUFFIXES: [&str; 7] = [
        "regular", "medium", "bold", "italic", "semibold", "demibold", "black",
    ];
    let lower = value.to_ascii_lowercase();
    for suffix in SUFFIXES {
        if let Some(prefix) = lower.strip_suffix(suffix) {
            let cut = prefix.trim_end_matches(['-', '_', ' ']).len();
            return &value[..cut];
        }
    }
    value
}

fn windows_font_selection(
    face: &str,
    bold: bool,
    italic: bool,
    font_dir: &Path,
) -> (&'static str, bool, bool) {
    let normalized = face.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.eq_ignore_ascii_case("Fragment Mono") {
        return match (bold, italic) {
            (false, false) if font_dir.join("CascadiaCode.ttf").is_file() => {
                ("CascadiaCode.ttf", false, false)
            }
            (true, false | true) if font_dir.join("BOOKOSB.TTF").is_file() => {
                ("BOOKOSB.TTF", true, false)
            }
            _ => ("arial.ttf", false, false),
        };
    }
    if normalized.eq_ignore_ascii_case("Berkeley Mono") {
        let candidate = if bold {
            "BerkeleyMono-Bold.ttf"
        } else {
            "BerkeleyMono-Regular.ttf"
        };
        if font_dir.join(candidate).is_file() {
            return (candidate, bold, false);
        }
    }
    if bold {
        ("arialbd.ttf", true, false)
    } else {
        ("arial.ttf", false, false)
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

pub(crate) fn hex_encoded(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

fn shaping_template(font_id: &str, sha256: &str) -> Result<ShapingInput, DesignError> {
    serde_json::from_value(serde_json::json!({
        "font_id": font_id,
        "font_sha256": sha256,
        "face_index": 0,
        "variations": [],
        "text": "",
        "text_index_unit": "utf8_byte_offset",
        "scale_x": 1000,
        "scale_y": 1000,
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "features": [],
        "buffer_properties": {
            "cluster_level": "monotone_graphemes",
            "beginning_of_text": true,
            "end_of_text": true,
            "default_ignorables": "normal",
            "do_not_insert_dotted_circle": false,
            "produce_unsafe_to_concat": false,
            "produce_safe_to_insert_tatweel": false
        }
    }))
    .map_err(|error| DesignError::context("could not build default shaping template", error))
}

fn design_json_paths(loaded: &LoadedDesignSources) -> Result<KiCadDesignJsonPaths, DesignError> {
    let mut paths = KiCadDesignJsonPaths::default();
    for source in loaded.bundle.sources() {
        let absolute = loaded.bundle_root.join(source.path());
        let absolute = absolute.canonicalize().map_err(|error| {
            DesignError::context("could not resolve design JSON source path", error)
        })?;
        ensure_contained(&loaded.bundle_root, &absolute)?;
        let filename = absolute
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| DesignError::new("design source filename is not valid Unicode"))?
            .to_owned();
        let path = display_source_path(&absolute);
        match source.kind() {
            SourceKind::Project => {
                paths.project_name = absolute
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned);
                paths.project_filename = Some(filename);
                paths.project_path = Some(path);
            }
            SourceKind::Schematic => {
                paths.schematic_paths.insert(
                    source.path().to_owned(),
                    KiCadDesignSourcePath { filename, path },
                );
            }
            _ => {}
        }
    }
    Ok(paths)
}

pub(crate) fn display_source_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(value) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{value}")
    } else if let Some(value) = value.strip_prefix(r"\\?\") {
        value.to_owned()
    } else {
        value.into_owned()
    }
}

fn resolve_child_schematic(
    bundle_root: &Path,
    source_parent: &Path,
    sheet_file: &str,
) -> Result<PathBuf, DesignError> {
    if sheet_file.is_empty() {
        return Err(DesignError::new("schematic sheet has an empty file path"));
    }
    let raw = Path::new(sheet_file);
    if raw.is_absolute()
        || raw
            .components()
            .any(|part| matches!(part, Component::Prefix(_)))
    {
        return Err(DesignError::new(format!(
            "schematic sheet path is not relative: {sheet_file}"
        )));
    }
    let child = source_parent.join(raw).canonicalize().map_err(|error| {
        DesignError::context(
            &format!("could not resolve child schematic {sheet_file}"),
            error,
        )
    })?;
    ensure_contained(bundle_root, &child)?;
    if !child.is_file() {
        return Err(DesignError::new(format!(
            "child schematic is not a regular file: {}",
            child.display()
        )));
    }
    Ok(child)
}

fn ensure_contained(root: &Path, path: &Path) -> Result<(), DesignError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(DesignError::new(format!(
            "design source escapes bundle root: {}",
            path.display()
        )))
    }
}

fn portable_relative(root: &Path, path: &Path) -> Result<String, DesignError> {
    ensure_contained(root, path)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|error| DesignError::context("could not make source path relative", error))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or_else(|| DesignError::new("source path is not valid Unicode"))?,
            ),
            _ => return Err(DesignError::new("source path is not portable")),
        }
    }
    if parts.is_empty() {
        return Err(DesignError::new(
            "source path is empty relative to bundle root",
        ));
    }
    Ok(parts.join("/"))
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, DesignError> {
    let mut file = File::open(path)
        .map_err(|error| DesignError::context("could not open design source", error))?;
    let read_limit = maximum
        .checked_add(1)
        .ok_or_else(|| DesignError::new("source byte limit overflowed"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(read_limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| DesignError::context("could not read design source", error))?;
    if bytes.len() > maximum {
        return Err(DesignError::new(format!(
            "design source exceeds byte limit: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one manifest row and byte slot are appended atomically"
)]
fn push_source(
    rows: &mut Vec<SourceBundleSource>,
    buffers: &mut Vec<Vec<u8>>,
    total_bytes: &mut usize,
    path: String,
    kind: SourceKind,
    bytes: Vec<u8>,
    limits: SourceBundleLimits,
) -> Result<(), DesignError> {
    if rows.len() >= limits.max_sources {
        return Err(DesignError::new("design source count exceeds bundle limit"));
    }
    *total_bytes = total_bytes
        .checked_add(bytes.len())
        .filter(|total| *total <= limits.max_total_bytes)
        .ok_or_else(|| DesignError::new("design source bytes exceed bundle limit"))?;
    let slot = u32::try_from(rows.len())
        .map_err(|_| DesignError::new("design source slot does not fit u32"))?;
    rows.push(SourceBundleSource {
        kind,
        path,
        slot: SourceSlot(slot),
        source_bytes: CanonicalUint64Decimal(bytes.len().to_string()),
    });
    buffers.push(bytes);
    Ok(())
}

#[cfg(test)]
mod plot_batch_budget_tests {
    use super::*;

    #[test]
    fn source_snapshot_binds_exact_loaded_carrier_bytes() {
        let manifest = SourceBundleManifestA0 {
            project_path: None,
            root_schematic_path: "root.kicad_sch".to_owned(),
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources: vec![SourceBundleSource {
                kind: SourceKind::Schematic,
                path: "root.kicad_sch".to_owned(),
                slot: SourceSlot(0),
                source_bytes: CanonicalUint64Decimal("3".to_owned()),
            }],
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        };
        let first = source_snapshot_sha256(&manifest, &[b"abc".to_vec()]).unwrap();
        let changed = source_snapshot_sha256(&manifest, &[b"abd".to_vec()]).unwrap();
        assert_ne!(first, changed);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn aggregate_budget_accepts_exact_and_rejects_one_under() {
        let document = SchematicPlotContractBudget {
            derived_items: 41,
            materialized_bytes: 73,
        };
        let exact = SchematicPlotDocumentsLimits {
            max_total_derived_items: document.derived_items,
            max_total_materialized_bytes: document.materialized_bytes,
            ..SchematicPlotDocumentsLimits::default()
        };
        let mut total = SchematicPlotContractBudget {
            derived_items: 0,
            materialized_bytes: 0,
        };
        charge_plot_batch_budget(&mut total, document, exact).expect("exact batch budget");
        assert_eq!(total, document);

        for limits in [
            SchematicPlotDocumentsLimits {
                max_total_derived_items: document.derived_items - 1,
                ..exact
            },
            SchematicPlotDocumentsLimits {
                max_total_materialized_bytes: document.materialized_bytes - 1,
                ..exact
            },
        ] {
            let mut total = SchematicPlotContractBudget {
                derived_items: 0,
                materialized_bytes: 0,
            };
            assert!(charge_plot_batch_budget(&mut total, document, limits).is_err());
        }
    }

    #[test]
    fn overlapped_profile_pair_charges_only_blocking_time() {
        assert_eq!(account_overlapped_pair(30, 70, 10), (3, 7));
        assert_eq!(account_overlapped_pair(30, 70, 100), (30, 70));
        assert_eq!(account_overlapped_pair(30, 70, 150), (30, 70));
        assert_eq!(account_overlapped_pair(0, 0, 10), (0, 0));
    }

    #[test]
    fn board_facts_error_precedes_joined_font_worker_panic() {
        let facts = Err::<(), _>(DesignError::new("primary board facts error"));
        let font_panic: std::thread::Result<Result<(), DesignError>> =
            Err(Box::new("injected font panic"));
        let error = resolve_parallel_board_inputs(facts, font_panic).unwrap_err();
        assert_eq!(error.to_string(), "primary board facts error");
    }

    #[test]
    fn board_source_limit_rejects_before_parallel_font_work_is_eligible() {
        let limits = BoardPlotLimits {
            max_source_bytes: 2,
            ..BoardPlotLimits::default()
        };
        assert!(preflight_parallel_board_fonts("abc", limits).is_err());
        assert!(preflight_parallel_board_fonts("ab", limits).is_ok());
    }

    #[test]
    fn embedded_font_selection_uses_family_style_and_regular_fallback() {
        let files = vec![
            SchematicEmbeddedFile {
                name: "renamed-bold.ttf".to_owned(),
                file_type: "font".to_owned(),
                bytes: KICAD_STROKE_BOLD.to_vec(),
            },
            SchematicEmbeddedFile {
                name: "renamed-regular.ttf".to_owned(),
                file_type: "font".to_owned(),
                bytes: KICAD_STROKE_REGULAR.to_vec(),
            },
        ];
        let indexed = embedded_font_index(&files).expect("bounded embedded font index");
        assert_eq!(indexed.len(), 2);
        let (bold, selected_bold, selected_italic) =
            embedded_font_selection("renamed", true, false, &indexed).expect("bold face");
        assert_eq!(bold.file.name, "renamed-bold.ttf");
        assert!(selected_bold);
        assert!(!selected_italic);

        let regular_files = &files[1..];
        let regular_only =
            embedded_font_index(regular_files).expect("bounded regular embedded font index");
        let (fallback, selected_bold, selected_italic) =
            embedded_font_selection("renamed", true, true, &regular_only)
                .expect("regular fallback");
        assert_eq!(fallback.file.name, "renamed-regular.ttf");
        assert!(!selected_bold);
        assert!(!selected_italic);

        let faces = BTreeSet::from(["renamed".to_owned()]);
        let exact_bytes = KICAD_STROKE_REGULAR.len() * 4;
        let exact_limits = PlotterTextCacheLimits {
            max_font_bytes: exact_bytes,
            ..PlotterTextCacheLimits::default()
        };
        let styles = schematic_font_styles_with_limits(&faces, regular_files, exact_limits)
            .expect("exact reused embedded font budget");
        assert_eq!(styles.len(), 4);
        assert!(
            styles
                .iter()
                .all(|style| matches!(style.bytes, Cow::Borrowed(_)))
        );
        assert!(
            schematic_font_styles_with_limits(
                &faces,
                regular_files,
                PlotterTextCacheLimits {
                    max_font_bytes: exact_bytes - 1,
                    ..exact_limits
                },
            )
            .err()
            .expect("one-under reused embedded font budget")
            .to_string()
            .contains("aggregate limit")
        );
    }
}

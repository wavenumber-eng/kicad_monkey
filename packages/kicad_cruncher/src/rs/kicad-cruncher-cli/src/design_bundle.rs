//! Transactional publication of the pure-Rust design-review bundle.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use kicad_monkey_core::{ENGINE_VERSION, KiCadSchematicInstance};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::DesignOptions;
use crate::design::{
    DesignError, SchematicSvgDocumentsLimits, build_board_plot_document,
    build_schematic_base_svgs_for_plot_documents_with_limits,
    build_schematic_plot_document_artifacts, build_structured_design_facts_profiled,
    display_source_path, hex_encoded, load_design_sources_profiled, sha256_hex,
};
use crate::pcb_review_svg::{PcbReviewSvgLimits, build_pcb_review_svgs_with_limits};
use crate::performance::{PerformanceProfile, PerformanceRecorder};
use crate::schematic_review_svg::{
    SchematicReviewSvgLimits, build_schematic_review_svgs_with_limits,
};

const MANIFEST_SCHEMA: &str = "kicad_cruncher.design_review_manifest.a0";
const GRAPH_LINKAGE_CONTRACT: &str = "kicad_monkey.schematic.svg.compiled_graph_linkage.a0";
const SCHEMATIC_REVIEW_THEME: &str = "kicad_cruncher.design_review.schematic_svg.a0";
const PAD_COLOR: &str = "#000000";
const TRACE_COLOR: &str = "#B8B8B8";
const EDGE_COLOR: &str = "#000000";
const PTH_DRILL_COLOR: &str = "#2563EB";
const PTH_SLOT_COLOR: &str = "#0891B2";
const NPTH_DRILL_COLOR: &str = "#DC2626";
const NPTH_SLOT_COLOR: &str = "#F97316";
const UNKNOWN_HOLE_COLOR: &str = "#6B7280";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const PERFORMANCE_PROFILE_ENV: &str = "KICAD_CRUNCHER_PERFORMANCE_PROFILE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesignReviewBundleLimits {
    pub max_artifacts: usize,
    pub max_artifact_path_bytes: usize,
    pub max_artifact_bytes: usize,
    pub max_total_artifact_bytes: usize,
}

impl Default for DesignReviewBundleLimits {
    fn default() -> Self {
        Self {
            max_artifacts: 8_192,
            max_artifact_path_bytes: 32 * 1024,
            max_artifact_bytes: 1024 * 1024 * 1024,
            max_total_artifact_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct DesignReviewBundle {
    pub output_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub component_count: usize,
    pub net_count: usize,
    pub schematic_svg_count: usize,
    pub pcb_svg_count: usize,
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema: &'static str,
    input: String,
    design_json: &'a str,
    compiled_schematic_graph: GraphArtifact<'a>,
    netlist_json: &'a str,
    netlist_kicad_sexpr: &'a str,
    schematic_svgs: &'a [SchematicArtifact],
    pcb_svgs: &'a [PcbArtifact],
    readme: &'a str,
    design_facts: DesignFactsArtifact<'a>,
}

#[derive(Serialize)]
struct GraphArtifact<'a> {
    file: &'a str,
    schema: &'a str,
    #[serde(rename = "type")]
    type_: &'a str,
    identity_namespace: &'a str,
    counts: GraphCounts,
    linkage_contract: &'static str,
}

#[derive(Serialize)]
struct GraphCounts {
    unit_definitions: usize,
    page_definitions: usize,
    unit_occurrences: usize,
    page_occurrences: usize,
    hierarchy_occurrences: usize,
    component_occurrences: usize,
    local_net_occurrences: usize,
    terminal_occurrences: usize,
    hierarchy_terminal_bindings: usize,
    graphical_artifact_links: usize,
}

#[derive(Serialize)]
struct DesignFactsArtifact<'a> {
    backend: &'static str,
    engine_version: &'static str,
    resource_profile: &'static str,
    source_snapshot_sha256: &'a str,
    compiled_schematic_graph_sha256: &'a str,
    kicad_netlist_bytes: usize,
    kicad_netlist_sha256: &'a str,
}

#[derive(Serialize)]
struct SchematicArtifact {
    file: String,
    sheet_number: usize,
    sheet_count: usize,
    sheet_name: String,
    sheet_path: String,
    sheet_path_uuids: String,
    sheet_instance_path: String,
    source: String,
    page_occurrence_ref: String,
    artifact_key: &'static str,
    graph_link_count: usize,
    resolved_svg_identity_count: usize,
}

#[derive(Serialize)]
struct PcbArtifact {
    file: String,
    layer: String,
    included_layers: Vec<String>,
    drill_slot_record_count: usize,
}

pub fn run_design(options: &DesignOptions) -> Result<DesignReviewBundle, DesignError> {
    let input = resolve_design_input(options.input.as_deref())?;
    let output = options
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from("output").join("design"));
    let limits = DesignReviewBundleLimits::default();
    if std::env::var_os(PERFORMANCE_PROFILE_ENV).as_deref() == Some(std::ffi::OsStr::new("1")) {
        let (bundle, profile) =
            write_design_review_bundle_profiled(&input, &output, options.include_indexes, limits)?;
        if let Ok(payload) = serde_json::to_string(&profile) {
            eprintln!("KICAD_CRUNCHER_PERFORMANCE_PROFILE={payload}");
        }
        Ok(bundle)
    } else {
        write_design_review_bundle(&input, &output, options.include_indexes, limits)
    }
}

pub fn resolve_design_input(input: Option<&Path>) -> Result<PathBuf, DesignError> {
    if let Some(input) = input {
        return Ok(input.to_path_buf());
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(".")
        .map_err(|error| DesignError::context("could not inspect current directory", error))?
    {
        let path = entry
            .map_err(|error| DesignError::context("could not inspect current directory", error))?
            .path();
        if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("kicad_pro"))
        {
            if candidates.len() == 4_096 {
                return Err(DesignError::new(
                    "current directory exceeds project auto-detection limit",
                ));
            }
            candidates.push(path);
        }
    }
    candidates.sort();
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(DesignError::new(
            "no .kicad_pro file was found in the current directory",
        )),
        count => Err(DesignError::new(format!(
            "found {count} .kicad_pro files; specify the design input explicitly"
        ))),
    }
}

pub fn write_design_review_bundle(
    input: &Path,
    output: &Path,
    include_indexes: bool,
    limits: DesignReviewBundleLimits,
) -> Result<DesignReviewBundle, DesignError> {
    write_design_review_bundle_internal(
        input,
        output,
        include_indexes,
        limits,
        PerformanceRecorder::new(false),
    )
    .map(|(bundle, _profile)| bundle)
}

pub(crate) fn write_design_review_bundle_profiled(
    input: &Path,
    output: &Path,
    include_indexes: bool,
    limits: DesignReviewBundleLimits,
) -> Result<(DesignReviewBundle, PerformanceProfile), DesignError> {
    write_design_review_bundle_internal(
        input,
        output,
        include_indexes,
        limits,
        PerformanceRecorder::new(true),
    )
}

fn write_design_review_bundle_internal(
    input: &Path,
    output: &Path,
    include_indexes: bool,
    limits: DesignReviewBundleLimits,
    mut performance: PerformanceRecorder,
) -> Result<(DesignReviewBundle, PerformanceProfile), DesignError> {
    let resolve_started = performance.start();
    validate_limits(limits)?;
    let (destination, parent) = resolve_destination(output)?;
    performance.finish("resolve_and_validate_output", resolve_started);
    let staging_started = performance.start();
    let temporary = create_temporary_directory(&parent)?;
    let staging = temporary.join("output");
    fs::create_dir(&staging).map_err(|error| {
        DesignError::context("could not create bundle staging directory", error)
    })?;
    performance.finish("create_staging_directory", staging_started);
    let staged = write_staged_bundle(
        input,
        &staging,
        &destination,
        include_indexes,
        limits,
        &mut performance,
    );
    let (result, preserve_transaction) = match staged {
        Ok(result) => {
            let publish_started = performance.start();
            let published = publish_staged_tree(&staging, &destination, &temporary);
            performance.finish("publish_staged_tree", publish_started);
            match published {
                Ok(()) => (Ok(result), false),
                Err(failure) => (Err(failure.error), failure.preserve_transaction),
            }
        }
        Err(error) => (Err(error), false),
    };
    let cleanup_started = performance.start();
    cleanup_transaction(&temporary, preserve_transaction);
    performance.finish("cleanup_transaction", cleanup_started);
    match result {
        Ok(staged) => {
            let bundle = DesignReviewBundle {
                output_dir: destination.clone(),
                manifest_path: destination.join("design_review_manifest.json"),
                component_count: staged.component_count,
                net_count: staged.net_count,
                schematic_svg_count: staged.schematic_svg_count,
                pcb_svg_count: staged.pcb_svg_count,
            };
            let profile = performance.complete(staged.artifact_count, staged.artifact_bytes);
            Ok((bundle, profile))
        }
        Err(error) => Err(error),
    }
}

fn cleanup_transaction(temporary: &Path, preserve: bool) {
    if !preserve {
        let _cleanup = fs::remove_dir_all(temporary);
    }
}

fn validate_limits(limits: DesignReviewBundleLimits) -> Result<(), DesignError> {
    if limits.max_artifacts < 6
        || limits.max_artifact_path_bytes == 0
        || limits.max_artifact_bytes == 0
        || limits.max_total_artifact_bytes == 0
    {
        return Err(DesignError::new("design-review bundle limits are invalid"));
    }
    Ok(())
}

fn resolve_destination(output: &Path) -> Result<(PathBuf, PathBuf), DesignError> {
    let name = output
        .file_name()
        .ok_or_else(|| DesignError::new("design-review output must name a directory"))?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| DesignError::context("could not create output parent", error))?;
    let parent = parent
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve output parent", error))?;
    let destination = parent.join(name);
    let destination = if destination.exists() {
        destination
            .canonicalize()
            .map_err(|error| DesignError::context("could not resolve existing output", error))?
    } else {
        destination
    };
    Ok((destination, parent))
}

fn create_temporary_directory(parent: &Path) -> Result<PathBuf, DesignError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| DesignError::context("system clock is before Unix epoch", error))?
        .as_nanos();
    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".kicad-cruncher-design-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(DesignError::context(
                    "could not create transaction directory",
                    error,
                ));
            }
        }
    }
    Err(DesignError::new(
        "could not allocate a unique bundle transaction directory",
    ))
}

struct StagedSummary {
    component_count: usize,
    net_count: usize,
    schematic_svg_count: usize,
    pcb_svg_count: usize,
    artifact_count: usize,
    artifact_bytes: usize,
}

#[allow(
    clippy::too_many_lines,
    reason = "one transaction assembles and validates one manifest"
)]
fn write_staged_bundle(
    input: &Path,
    staging: &Path,
    destination: &Path,
    include_indexes: bool,
    limits: DesignReviewBundleLimits,
    performance: &mut PerformanceRecorder,
) -> Result<StagedSummary, DesignError> {
    let load_started = performance.start();
    let loaded = load_design_sources_profiled(input, performance)?;
    performance.finish("load_design_sources", load_started);
    validate_destination_source_separation(&loaded, destination)?;
    let facts_started = performance.start();
    let facts = build_structured_design_facts_profiled(&loaded, include_indexes, performance)?;
    performance.finish("build_structured_design_facts", facts_started);
    let stem = loaded
        .input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| DesignError::new("design input filename is not valid Unicode"))?;
    let design_path = format!("{stem}_design.json");
    let graph_path = format!("{stem}_compiled_schematic_graph.json");
    let netlist_json_path = format!("{stem}_netlist.json");
    let netlist_sexpr_path = format!("{stem}_netlist.net");
    let readme_path = "README.md";
    let manifest_path = "design_review_manifest.json";
    let mut budget = ArtifactBudget::new(limits);

    let structured_write_started = performance.start();
    write_json(staging, &design_path, &facts.design_json, &mut budget)?;
    write_json(
        staging,
        &graph_path,
        &facts.compiled_schematic_graph,
        &mut budget,
    )?;
    write_json(
        staging,
        &netlist_json_path,
        &facts.netlist_json,
        &mut budget,
    )?;
    write_text(
        staging,
        &netlist_sexpr_path,
        facts.kicad_netlist.as_bytes(),
        &mut budget,
    )?;
    performance.finish("write_structured_artifacts", structured_write_started);

    budget.ensure_future_artifacts(facts.schematic_instances.len().saturating_add(2))?;
    let schematic_documents_started = performance.start();
    let documents = build_schematic_plot_document_artifacts(&loaded, &facts.schematic_instances)?;
    performance.finish(
        "build_schematic_plot_documents",
        schematic_documents_started,
    );
    let mut base_limits = SchematicSvgDocumentsLimits::default();
    base_limits.max_total_svg_bytes = base_limits
        .max_total_svg_bytes
        .min(budget.remaining_bytes());
    let schematic_base_started = performance.start();
    let base_svgs =
        build_schematic_base_svgs_for_plot_documents_with_limits(&documents, base_limits)?;
    performance.finish("render_schematic_base_svgs", schematic_base_started);
    let mut review_limits = SchematicReviewSvgLimits::default();
    review_limits.max_total_output_bytes = review_limits
        .max_total_output_bytes
        .min(budget.remaining_bytes());
    review_limits.max_output_bytes_per_document = review_limits
        .max_output_bytes_per_document
        .min(budget.remaining_bytes())
        .min(limits.max_artifact_bytes);
    let schematic_review_started = performance.start();
    let reviews = build_schematic_review_svgs_with_limits(
        &documents,
        &base_svgs,
        &facts.compiled_schematic_graph,
        &facts.design_json,
        &format!("../{graph_path}"),
        review_limits,
    )?;
    performance.finish("enrich_schematic_review_svgs", schematic_review_started);
    if reviews.len() != facts.schematic_instances.len() {
        return Err(DesignError::new(
            "schematic review output count does not match design instances",
        ));
    }
    let mut used_names = HashSet::new();
    let mut schematic_artifacts = Vec::with_capacity(reviews.len());
    let schematic_write_started = performance.start();
    for (instance, review) in facts.schematic_instances.iter().zip(reviews) {
        let filename = schematic_filename(instance, &mut used_names);
        let path = format!("schematics/{filename}");
        write_text(staging, &path, review.svg.as_bytes(), &mut budget)?;
        schematic_artifacts.push(SchematicArtifact {
            file: path,
            sheet_number: instance.sheet_number,
            sheet_count: instance.sheet_count,
            sheet_name: instance.sheet_name.clone(),
            sheet_path: instance.sheet_path.clone(),
            sheet_path_uuids: instance.sheet_path_uuids.clone(),
            sheet_instance_path: instance.sheet_instance_path.clone(),
            source: display_source_path(&loaded.bundle_root.join(&instance.source_path)),
            page_occurrence_ref: review.page_occurrence_ref,
            artifact_key: review.artifact_key,
            graph_link_count: review.graph_link_count,
            resolved_svg_identity_count: review.resolved_svg_identity_count,
        });
    }
    performance.finish("write_schematic_svgs", schematic_write_started);

    let mut pcb_artifacts = Vec::new();
    let board_document_started = performance.start();
    if let Some(document) = build_board_plot_document(&loaded)? {
        performance.finish("build_board_plot_document", board_document_started);
        budget.ensure_future_artifacts(document.copper_layer_count().saturating_add(2))?;
        let board_name = safe_filename(
            loaded
                .pcb_path
                .as_deref()
                .and_then(Path::file_stem)
                .and_then(|value| value.to_str())
                .unwrap_or("board"),
            "board",
        );
        let mut pcb_limits = PcbReviewSvgLimits::default();
        pcb_limits.max_total_svg_bytes =
            pcb_limits.max_total_svg_bytes.min(budget.remaining_bytes());
        pcb_limits.max_svg_bytes_per_layer = pcb_limits
            .max_svg_bytes_per_layer
            .min(budget.remaining_bytes())
            .min(limits.max_artifact_bytes);
        let pcb_reviews_started = performance.start();
        let pcb_reviews = build_pcb_review_svgs_with_limits(&loaded, &document, pcb_limits)?;
        performance.finish("build_pcb_review_svgs", pcb_reviews_started);
        let pcb_write_started = performance.start();
        for review in pcb_reviews {
            let layer_name = safe_filename(&review.layer, "layer");
            let path = format!("pcb/copper_layers/{board_name}__{layer_name}__review.svg");
            write_text(staging, &path, review.svg.as_bytes(), &mut budget)?;
            pcb_artifacts.push(PcbArtifact {
                file: path,
                layer: review.layer,
                included_layers: review.included_layers,
                drill_slot_record_count: review.drill_slot_record_count,
            });
        }
        performance.finish("write_pcb_svgs", pcb_write_started);
    } else {
        performance.finish("build_board_plot_document", board_document_started);
    }

    let metadata_started = performance.start();
    let graph_sha256 = canonical_json_sha256(&facts.design_json["compiled_schematic_graph"])?;
    let netlist_sha256 = sha256_hex(facts.kicad_netlist.as_bytes());
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        input: display_source_path(&loaded.input_path),
        design_json: &design_path,
        compiled_schematic_graph: GraphArtifact {
            file: &graph_path,
            schema: &facts.compiled_schematic_graph.schema,
            type_: &facts.compiled_schematic_graph.type_,
            identity_namespace: &facts.compiled_schematic_graph.identity_namespace,
            counts: graph_counts(&facts.compiled_schematic_graph),
            linkage_contract: GRAPH_LINKAGE_CONTRACT,
        },
        netlist_json: &netlist_json_path,
        netlist_kicad_sexpr: &netlist_sexpr_path,
        schematic_svgs: &schematic_artifacts,
        pcb_svgs: &pcb_artifacts,
        readme: readme_path,
        design_facts: DesignFactsArtifact {
            backend: "kicad-monkey-native",
            engine_version: ENGINE_VERSION,
            resource_profile: "design-facts-bounded-a1",
            source_snapshot_sha256: &loaded.source_snapshot_sha256,
            compiled_schematic_graph_sha256: &graph_sha256,
            kicad_netlist_bytes: facts.kicad_netlist.len(),
            kicad_netlist_sha256: &netlist_sha256,
        },
    };
    write_readme(
        staging,
        readme_path,
        &loaded.input_path,
        &design_path,
        &graph_path,
        &netlist_json_path,
        &netlist_sexpr_path,
        &schematic_artifacts,
        &pcb_artifacts,
        manifest_path,
        &mut budget,
    )?;
    validate_manifest_artifacts(&manifest, staging, limits)?;
    write_json(staging, manifest_path, &manifest, &mut budget)?;
    performance.finish("build_and_write_bundle_metadata", metadata_started);
    Ok(StagedSummary {
        component_count: facts.design_json["components"]
            .as_array()
            .map_or(0, Vec::len),
        net_count: facts.design_json["nets"].as_array().map_or(0, Vec::len),
        schematic_svg_count: schematic_artifacts.len(),
        pcb_svg_count: pcb_artifacts.len(),
        artifact_count: budget.artifacts,
        artifact_bytes: budget.bytes,
    })
}

fn validate_destination_source_separation(
    loaded: &crate::design::LoadedDesignSources,
    destination: &Path,
) -> Result<(), DesignError> {
    if destination.is_file() || loaded.bundle_root.starts_with(destination) {
        return Err(DesignError::new(
            "design-review output overlaps the loaded design source tree",
        ));
    }
    for source in loaded.bundle.sources() {
        if loaded
            .bundle_root
            .join(source.path())
            .starts_with(destination)
        {
            return Err(DesignError::new(
                "design-review output contains a loaded design source",
            ));
        }
    }
    if loaded
        .pcb_path
        .as_deref()
        .is_some_and(|path| path.starts_with(destination))
    {
        return Err(DesignError::new(
            "design-review output contains the loaded PCB source",
        ));
    }
    Ok(())
}

fn graph_counts(graph: &CompiledSchematicGraphA0) -> GraphCounts {
    GraphCounts {
        unit_definitions: graph.unit_definitions.len(),
        page_definitions: graph.page_definitions.len(),
        unit_occurrences: graph.unit_occurrences.len(),
        page_occurrences: graph.page_occurrences.len(),
        hierarchy_occurrences: graph.hierarchy_occurrences.len(),
        component_occurrences: graph.component_occurrences.len(),
        local_net_occurrences: graph.local_net_occurrences.len(),
        terminal_occurrences: graph.terminal_occurrences.len(),
        hierarchy_terminal_bindings: graph.hierarchy_terminal_bindings.len(),
        graphical_artifact_links: graph.graphical_artifact_links.len(),
    }
}

fn canonical_json_sha256(value: &Value) -> Result<String, DesignError> {
    let mut writer = DigestWriter(Sha256::new());
    write_canonical_json(&mut writer, value)?;
    Ok(hex_encoded(&writer.0.finalize()))
}

fn write_canonical_json(writer: &mut impl Write, value: &Value) -> Result<(), DesignError> {
    match value {
        Value::Array(values) => {
            writer.write_all(b"[")?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    writer.write_all(b",")?;
                }
                write_canonical_json(writer, value)?;
            }
            writer.write_all(b"]")?;
        }
        Value::Object(values) => {
            writer.write_all(b"{")?;
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    writer.write_all(b",")?;
                }
                serde_json::to_writer(&mut *writer, key).map_err(|error| {
                    DesignError::context("could not hash canonical graph key", error)
                })?;
                writer.write_all(b":")?;
                write_canonical_json(writer, &values[key])?;
            }
            writer.write_all(b"}")?;
        }
        _ => serde_json::to_writer(writer, value)
            .map_err(|error| DesignError::context("could not hash canonical graph value", error))?,
    }
    Ok(())
}

struct DigestWriter(Sha256);

impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn schematic_filename(instance: &KiCadSchematicInstance, used: &mut HashSet<String>) -> String {
    let source_stem = Path::new(&instance.source_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("sheet");
    let sheet_name = if instance.sheet_name.is_empty() {
        source_stem
    } else {
        &instance.sheet_name
    };
    let base = format!(
        "{:02}_{}",
        instance.sheet_number,
        safe_filename(sheet_name, "sheet")
    );
    let mut filename = format!("{base}.svg");
    if !used.insert(filename.clone()) {
        filename = format!("{base}_{:02}.svg", instance.instance_index);
        used.insert(filename.clone());
    }
    filename
}

fn safe_filename(value: &str, fallback: &str) -> String {
    let mut output = String::new();
    let mut underscore = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '-') {
            output.push(character);
            underscore = false;
        } else if !underscore {
            output.push('_');
            underscore = true;
        }
    }
    let trimmed = output.trim_matches(['_', '.', '-']);
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

struct ArtifactBudget {
    limits: DesignReviewBundleLimits,
    artifacts: usize,
    bytes: usize,
}

impl ArtifactBudget {
    fn new(limits: DesignReviewBundleLimits) -> Self {
        Self {
            limits,
            artifacts: 0,
            bytes: 0,
        }
    }

    fn begin(&mut self, path: &str) -> Result<(), DesignError> {
        validate_relative_path(path, self.limits.max_artifact_path_bytes)?;
        self.artifacts = self
            .artifacts
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_artifacts)
            .ok_or_else(|| DesignError::new("design-review artifact count exceeds its limit"))?;
        Ok(())
    }

    fn ensure_future_artifacts(&self, additional: usize) -> Result<(), DesignError> {
        self.artifacts
            .checked_add(additional)
            .filter(|count| *count <= self.limits.max_artifacts)
            .ok_or_else(|| DesignError::new("design-review artifact count exceeds its limit"))?;
        Ok(())
    }

    fn remaining_bytes(&self) -> usize {
        self.limits
            .max_total_artifact_bytes
            .saturating_sub(self.bytes)
    }
}

struct BoundedArtifactWriter<'a> {
    file: File,
    budget: &'a mut ArtifactBudget,
    artifact_bytes: usize,
}

impl Write for BoundedArtifactWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let artifact_bytes = self
            .artifact_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("design-review artifact byte count overflowed"))?;
        let total_bytes = self
            .budget
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("design-review total byte count overflowed"))?;
        if artifact_bytes > self.budget.limits.max_artifact_bytes {
            return Err(io::Error::other(
                "design-review artifact exceeds its byte limit",
            ));
        }
        if total_bytes > self.budget.limits.max_total_artifact_bytes {
            return Err(io::Error::other(
                "design-review bundle exceeds its total byte limit",
            ));
        }
        let written = self.file.write(bytes)?;
        self.artifact_bytes += written;
        self.budget.bytes += written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn artifact_writer<'a>(
    root: &Path,
    path: &str,
    budget: &'a mut ArtifactBudget,
) -> Result<BoundedArtifactWriter<'a>, DesignError> {
    budget.begin(path)?;
    let target = root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| DesignError::context("could not create artifact directory", error))?;
    }
    let file = File::create(&target)
        .map_err(|error| DesignError::context("could not create design-review artifact", error))?;
    Ok(BoundedArtifactWriter {
        file,
        budget,
        artifact_bytes: 0,
    })
}

fn write_json(
    root: &Path,
    path: &str,
    value: &impl Serialize,
    budget: &mut ArtifactBudget,
) -> Result<(), DesignError> {
    let mut writer = artifact_writer(root, path, budget)?;
    serde_json::to_writer_pretty(&mut writer, value)
        .map_err(|error| DesignError::context("could not write JSON artifact", error))?;
    writer
        .write_all(b"\n")
        .map_err(|error| DesignError::context("could not finish JSON artifact", error))?;
    writer
        .flush()
        .map_err(|error| DesignError::context("could not flush JSON artifact", error))
}

fn write_text(
    root: &Path,
    path: &str,
    bytes: &[u8],
    budget: &mut ArtifactBudget,
) -> Result<(), DesignError> {
    let mut writer = artifact_writer(root, path, budget)?;
    writer
        .write_all(bytes)
        .and_then(|()| writer.flush())
        .map_err(|error| DesignError::context("could not write design-review artifact", error))
}

fn manifest_paths<'a>(manifest: &'a Manifest<'_>) -> Vec<&'a str> {
    let mut paths = vec![
        manifest.design_json,
        manifest.compiled_schematic_graph.file,
        manifest.netlist_json,
        manifest.netlist_kicad_sexpr,
        manifest.readme,
    ];
    paths.extend(
        manifest
            .schematic_svgs
            .iter()
            .map(|item| item.file.as_str()),
    );
    paths.extend(manifest.pcb_svgs.iter().map(|item| item.file.as_str()));
    paths
}

fn validate_manifest_artifacts(
    manifest: &Manifest<'_>,
    root: &Path,
    limits: DesignReviewBundleLimits,
) -> Result<(), DesignError> {
    let root = root
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve staged bundle", error))?;
    let paths = manifest_paths(manifest);
    if paths.len() > limits.max_artifacts {
        return Err(DesignError::new(
            "manifest artifact count exceeds its limit",
        ));
    }
    for path in paths {
        validate_relative_path(path, limits.max_artifact_path_bytes)?;
        let candidate = root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let candidate = candidate.canonicalize().map_err(|error| {
            DesignError::context("manifest artifact does not exist before publication", error)
        })?;
        if !candidate.starts_with(&root) || !candidate.is_file() {
            return Err(DesignError::new(
                "manifest artifact is not a contained regular file",
            ));
        }
    }
    Ok(())
}

fn validate_relative_path(path: &str, maximum_bytes: usize) -> Result<(), DesignError> {
    if path.is_empty() || path.len() > maximum_bytes || path.contains('\\') {
        return Err(DesignError::new(
            "artifact path is not safe bundle-relative",
        ));
    }
    if path.as_bytes().get(1) == Some(&b':') || Path::new(path).is_absolute() {
        return Err(DesignError::new(
            "artifact path is not safe bundle-relative",
        ));
    }
    for component in Path::new(path).components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(DesignError::new(
                "artifact path is not safe bundle-relative",
            ));
        }
    }
    if path.split('/').any(|segment| segment.is_empty()) {
        return Err(DesignError::new(
            "artifact path is not safe bundle-relative",
        ));
    }
    Ok(())
}

struct PublishFailure {
    error: DesignError,
    preserve_transaction: bool,
}

fn publish_staged_tree(
    staging: &Path,
    destination: &Path,
    temporary: &Path,
) -> Result<(), PublishFailure> {
    publish_staged_tree_with(staging, destination, temporary, |from, to| {
        fs::rename(from, to)
    })
}

fn publish_staged_tree_with(
    staging: &Path,
    destination: &Path,
    temporary: &Path,
    mut rename: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<(), PublishFailure> {
    let backup = temporary.join("previous");
    let had_previous = destination.exists();
    if had_previous {
        rename(destination, &backup).map_err(|error| PublishFailure {
            error: DesignError::context("could not stage previous bundle", error),
            preserve_transaction: false,
        })?;
    }
    if let Err(error) = rename(staging, destination) {
        if had_previous && let Err(restore_error) = rename(&backup, destination) {
            return Err(PublishFailure {
                error: DesignError::new(format!(
                    "could not publish bundle ({error}); restoring the previous bundle also failed ({restore_error}); recovery backup retained at {}",
                    backup.display()
                )),
                preserve_transaction: true,
            });
        }
        return Err(PublishFailure {
            error: DesignError::context("could not publish staged design-review bundle", error),
            preserve_transaction: false,
        });
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "README names every bundle artifact family"
)]
fn write_readme(
    root: &Path,
    path: &str,
    input: &Path,
    design_json: &str,
    graph: &str,
    netlist_json: &str,
    netlist_sexpr: &str,
    schematics: &[SchematicArtifact],
    pcbs: &[PcbArtifact],
    manifest: &str,
    budget: &mut ArtifactBudget,
) -> Result<(), DesignError> {
    let mut writer = artifact_writer(root, path, budget)?;
    write!(
        writer,
        "# KiCad Design Review Bundle\n\nInput: `{}`\n\nThis folder is generated by `kicad-cruncher design` / `design-review` / `dr`.\nIt is intended for design review agents that need a machine-readable design\nmodel plus visual context.\n\n## Files\n\n- `{design_json}`: KiCad-native design JSON from `kicad-monkey`.\n- `{graph}`: exact compiled schematic connectivity graph from\n  the Design JSON, with occurrence-scoped identities and drawing links.\n- `{netlist_json}`: KiCad-native netlist JSON from `kicad-monkey`.\n- `{netlist_sexpr}`: kicad-cli-style S-expression netlist.\n- `{manifest}`: artifact index for this review bundle.\n- `schematics/`: enriched black-and-white schematic SVGs, one file per\n  concrete hierarchy instance.\n- `pcb/copper_layers/`: enriched PCB SVGs, one file per copper layer.\n\n## Design JSON Relationships\n\nThe design JSON schema is `kicad_monkey.design.a0`. It includes project text\nvariables, schematic hierarchy, components, nets, optional PnP data, and lookup\nindexes unless `--no-indexes` was used.\n\n## Compiled Schematic Graph Navigation\n\nThe standalone graph uses schema `kicad_monkey.compiled_schematic_graph.a0`.\nEach schematic SVG embeds a `compiled_schematic_graph_view` that identifies its\ncanonical page occurrence and maps SVG element ids to graph link ids and graph\ntargets back to SVG element ids. The authoritative join is\n`page_occurrence_ref + artifact_key + element_id`; do not infer connectivity\nfrom displayed names, text, geometry, or DOM order. Follow terminal\n`local_net_occurrence_ref` values to discover electrically related terminals\nand components, and follow hierarchy bindings for parent/child sheet ports.\n\nComponent and net entries carry SVG link fields where available. Schematic SVG\ngroups use `data-uuid`, `data-ref`, `data-component`, `data-pin-*`, and net\nrelationship attributes so an agent can map graphics back to symbols, pins,\nports, sheets, and nets. PCB SVG groups use `data-component`, `data-pad-*`,\n`data-net`, `data-layer-*`, `data-hole-*`, and IPC-4761 via metadata when the\nsource board provides it.\n\n## Schematic SVGs\n\n",
        display_source_path(input)
    )?;
    if schematics.is_empty() {
        write!(writer, "- No schematic SVGs were generated.")?;
    } else {
        for (index, item) in schematics.iter().enumerate() {
            if index != 0 {
                writer.write_all(b"\n")?;
            }
            write!(
                writer,
                "- `{}`: sheet {}/{} `{}` instance `{}`",
                item.file,
                item.sheet_number,
                item.sheet_count,
                item.sheet_path,
                item.sheet_instance_path
            )?;
        }
    }
    write!(
        writer,
        "\n\nRepeated hierarchical sheets produce separate SVGs. Use `sheet_path` for the\nhuman hierarchy path and `sheet_instance_path` for the KiCad UUID instance path.\nSchematic SVGs use the `{SCHEMATIC_REVIEW_THEME}` role theme from\n`kicad-monkey`: enriched source-object groups and net metadata are preserved,\nwhile schematic graphics are rendered as black on a white page for review.\n\n## PCB Review SVGs\n\n"
    )?;
    if pcbs.is_empty() {
        write!(writer, "- No PCB copper-layer SVGs were generated.")?;
    } else {
        for (index, item) in pcbs.iter().enumerate() {
            if index != 0 {
                writer.write_all(b"\n")?;
            }
            write!(
                writer,
                "- `{}`: copper layer `{}` plus `Edge.Cuts` and {} enriched drill/slot records",
                item.file, item.layer, item.drill_slot_record_count
            )?;
        }
    }
    write!(
        writer,
        "\n\nPCB review SVGs preserve the enriched `kicad-monkey` metadata and apply this\nreview theme:\n\n- pads belonging to footprints: black (`{PAD_COLOR}`);\n- tracks, arcs, vias, and zones/polygons: light gray (`{TRACE_COLOR}`);\n- board outline / `Edge.Cuts`: black (`{EDGE_COLOR}`);\n- plated drills: blue (`{PTH_DRILL_COLOR}`);\n- plated slots: cyan (`{PTH_SLOT_COLOR}`);\n- non-plated drills: red (`{NPTH_DRILL_COLOR}`);\n- non-plated slots: orange (`{NPTH_SLOT_COLOR}`);\n- unknown-plating holes: neutral gray (`{UNKNOWN_HOLE_COLOR}`).\n\nDrill and slot cutouts come from the enriched `kicad-monkey` PCB SVG records.\nApplications should use `data-hole-plating` and `data-hole-kind` to distinguish\nplated through-hole pads/vias from KiCad `np_thru_hole` mechanical pads. Valid\nplating values are `plated`, `non_plated`, and `unknown`. The design-review\ntheme colors those existing records in place; it does not create a second\ndrill/slot overlay, add duplicate boolean plating fields, or change the\n`kicad-monkey` spelling of `non_plated`.\n\nDraw order is tracks/arcs first, polygons/zones above those, edge cuts, pads,\nthen the `kicad-monkey` drill/slot records last.\n"
    )?;
    writer
        .flush()
        .map_err(|error| DesignError::context("could not finish README", error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::load_design_sources;

    #[test]
    fn filenames_match_the_python_contract() {
        assert_eq!(safe_filename(" F.Cu ", "layer"), "F.Cu");
        assert_eq!(safe_filename("a / b", "sheet"), "a_b");
        assert_eq!(safe_filename("...", "artifact"), "artifact");
    }

    #[test]
    fn bundle_paths_are_strictly_relative() {
        for invalid in ["", "/x", "C:/x", "a\\b", "a/../b", "a//b", "./a"] {
            assert!(validate_relative_path(invalid, 100).is_err(), "{invalid}");
        }
        assert!(validate_relative_path("schematics/01_root.svg", 100).is_ok());
    }

    #[test]
    fn canonical_json_hash_sorts_every_object_key() {
        let value = serde_json::json!({"z": {"b": 1, "a": 2}, "a": [3, 4]});
        assert_eq!(
            canonical_json_sha256(&value).unwrap(),
            sha256_hex(br#"{"a":[3,4],"z":{"a":2,"b":1}}"#)
        );
    }

    #[test]
    fn writer_accepts_exact_and_rejects_one_under() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-bundle-budget-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let exact = DesignReviewBundleLimits {
            max_artifacts: 6,
            max_artifact_path_bytes: 16,
            max_artifact_bytes: 3,
            max_total_artifact_bytes: 3,
        };
        let mut budget = ArtifactBudget::new(exact);
        write_text(&root, "a", b"abc", &mut budget).unwrap();
        let artifact_under = DesignReviewBundleLimits {
            max_artifact_bytes: 2,
            ..exact
        };
        let mut budget = ArtifactBudget::new(artifact_under);
        assert!(write_text(&root, "b", b"abc", &mut budget).is_err());
        let total_under = DesignReviewBundleLimits {
            max_total_artifact_bytes: 2,
            ..exact
        };
        let mut budget = ArtifactBudget::new(total_under);
        assert!(write_text(&root, "c", b"abc", &mut budget).is_err());
        let count_one = DesignReviewBundleLimits {
            max_artifacts: 1,
            ..exact
        };
        let mut budget = ArtifactBudget::new(count_one);
        write_text(&root, "d", b"a", &mut budget).unwrap();
        assert!(write_text(&root, "e", b"a", &mut budget).is_err());
        assert!(validate_relative_path("abc", 3).is_ok());
        assert!(validate_relative_path("abc", 2).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one contract test lists the exact stage and detail inventories"
    )]
    fn profiled_bundle_reports_each_whole_pipeline_stage() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-profile-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let output = root.join("review");
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pro");
        let (_bundle, profile) = write_design_review_bundle_profiled(
            &project,
            &output,
            true,
            DesignReviewBundleLimits::default(),
        )
        .unwrap();

        assert_eq!(profile.schema, crate::performance::PROFILE_SCHEMA);
        assert!(profile.total_elapsed_ns > 0);
        assert!(profile.accounted_elapsed_ns > 0);
        assert_eq!(
            profile.accounted_elapsed_ns + profile.unattributed_elapsed_ns,
            profile.total_elapsed_ns
        );
        assert_eq!(profile.artifact_count, 9);
        assert!(profile.artifact_bytes > 0);
        assert_eq!(
            profile
                .stages
                .iter()
                .map(|stage| stage.name)
                .collect::<Vec<_>>(),
            [
                "resolve_and_validate_output",
                "create_staging_directory",
                "load_design_sources",
                "build_structured_design_facts",
                "write_structured_artifacts",
                "build_schematic_plot_documents",
                "render_schematic_base_svgs",
                "enrich_schematic_review_svgs",
                "write_schematic_svgs",
                "build_board_plot_document",
                "build_pcb_review_svgs",
                "write_pcb_svgs",
                "build_and_write_bundle_metadata",
                "publish_staged_tree",
                "cleanup_transaction",
            ]
        );
        assert_eq!(
            profile
                .details
                .iter()
                .map(|detail| (detail.parent, detail.name))
                .collect::<Vec<_>>(),
            [
                ("load_design_sources", "resolve_design_paths"),
                ("load_design_sources", "read_project_source"),
                ("load_design_sources", "read_schematic_sources"),
                ("load_design_sources", "parse_schematic_documents"),
                ("load_design_sources", "extract_schematic_definitions",),
                ("load_design_sources", "discover_schematic_hierarchy"),
                ("load_design_sources", "insert_schematic_source_carriers",),
                ("load_design_sources", "assemble_and_hash_source_bundle",),
                ("load_design_sources", "read_pcb_source"),
                (
                    "build_structured_design_facts",
                    "parse_schematic_index_definitions",
                ),
                (
                    "build_structured_design_facts",
                    "realize_schematic_occurrences",
                ),
                (
                    "build_structured_design_facts",
                    "assemble_schematic_indexes",
                ),
                ("build_structured_design_facts", "parse_project_document",),
                (
                    "build_structured_design_facts",
                    "build_compiled_schematic_graph",
                ),
                ("build_structured_design_facts", "build_kicad_netlist"),
                (
                    "build_structured_design_facts",
                    "validate_compiled_schematic_graph",
                ),
                ("build_structured_design_facts", "emit_kicad_netlist"),
                ("build_structured_design_facts", "build_kicad_netlist_json",),
                ("build_structured_design_facts", "parse_pcb_view"),
                (
                    "build_structured_design_facts",
                    "design_json_binding_and_preflight",
                ),
                ("build_structured_design_facts", "design_json_netlist_json",),
                (
                    "build_structured_design_facts",
                    "design_json_project_variants_options",
                ),
                ("build_structured_design_facts", "design_json_sheets",),
                ("build_structured_design_facts", "design_json_components",),
                (
                    "build_structured_design_facts",
                    "design_json_schematic_hierarchy_and_nets",
                ),
                (
                    "build_structured_design_facts",
                    "design_json_compiled_graph_value",
                ),
                ("build_structured_design_facts", "design_json_pnp"),
                (
                    "build_structured_design_facts",
                    "design_json_classes_and_indexes",
                ),
                (
                    "build_structured_design_facts",
                    "design_json_output_limit_serialization",
                ),
                (
                    "build_structured_design_facts",
                    "enumerate_schematic_instances",
                ),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staged_artifact_failure_preserves_existing_destination() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-bundle-transaction-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let output = root.join("review");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("previous.txt"), b"keep me").unwrap();
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pro");
        let limits = DesignReviewBundleLimits {
            max_artifacts: 6,
            max_artifact_path_bytes: 32 * 1024,
            max_artifact_bytes: 1,
            max_total_artifact_bytes: 1,
        };

        assert!(write_design_review_bundle(&project, &output, true, limits).is_err());
        assert_eq!(fs::read(output.join("previous.txt")).unwrap(), b"keep me");
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".kicad-cruncher-design-")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_publish_and_restore_retains_a_reported_recovery_backup() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-bundle-restore-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = root.join(".kicad-cruncher-design-test");
        let staging = temporary.join("output");
        let destination = root.join("review");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("previous.txt"), b"recover me").unwrap();
        let mut calls = 0_usize;

        let failure = publish_staged_tree_with(&staging, &destination, &temporary, |from, to| {
            calls += 1;
            if calls == 1 {
                fs::rename(from, to)
            } else {
                Err(io::Error::other("injected rename failure"))
            }
        })
        .unwrap_err();
        cleanup_transaction(&temporary, failure.preserve_transaction);

        let backup = temporary.join("previous");
        assert!(failure.preserve_transaction);
        assert!(
            failure
                .error
                .to_string()
                .contains(&backup.display().to_string())
        );
        assert_eq!(
            fs::read(backup.join("previous.txt")).unwrap(),
            b"recover me"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_publish_restores_the_previous_destination() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-bundle-publish-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = root.join(".kicad-cruncher-design-test");
        let staging = temporary.join("output");
        let destination = root.join("review");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("previous.txt"), b"restore me").unwrap();
        let mut calls = 0_usize;

        let failure = publish_staged_tree_with(&staging, &destination, &temporary, |from, to| {
            calls += 1;
            if calls == 2 {
                Err(io::Error::other("injected publish failure"))
            } else {
                fs::rename(from, to)
            }
        })
        .unwrap_err();
        cleanup_transaction(&temporary, failure.preserve_transaction);

        assert!(!failure.preserve_transaction);
        assert_eq!(
            fs::read(destination.join("previous.txt")).unwrap(),
            b"restore me"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn output_ancestor_of_sources_is_rejected_without_source_loss() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-bundle-overlap-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/corpus/kicad/projects/hlr_test");
        for filename in [
            "hlr_test.kicad_pro",
            "hlr_test.kicad_sch",
            "hlr_test.kicad_pcb",
        ] {
            fs::copy(corpus.join(filename), source.join(filename)).unwrap();
        }
        let project = source.join("hlr_test.kicad_pro");
        let project_before = fs::read(&project).unwrap();

        let error = write_design_review_bundle(
            &project,
            &source,
            true,
            DesignReviewBundleLimits::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("overlaps"));
        assert_eq!(fs::read(&project).unwrap(), project_before);
        assert!(source.join("hlr_test.kicad_sch").is_file());
        assert!(source.join("hlr_test.kicad_pcb").is_file());
        #[cfg(windows)]
        {
            let case_alias = PathBuf::from(source.to_string_lossy().to_ascii_uppercase());
            let error = write_design_review_bundle(
                &project,
                &case_alias,
                true,
                DesignReviewBundleLimits::default(),
            )
            .unwrap_err();
            assert!(error.to_string().contains("overlaps"));
            assert_eq!(fs::read(&project).unwrap(), project_before);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_design_extensions_are_ascii_case_insensitive() {
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-bundle-suffix-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/corpus/kicad/projects/hlr_test");
        let project_case = root.join("project");
        fs::create_dir_all(&project_case).unwrap();
        fs::copy(
            corpus.join("hlr_test.kicad_pro"),
            project_case.join("hlr_test.KICAD_PRO"),
        )
        .unwrap();
        fs::copy(
            corpus.join("hlr_test.kicad_sch"),
            project_case.join("hlr_test.kicad_sch"),
        )
        .unwrap();
        assert!(load_design_sources(&project_case.join("hlr_test.KICAD_PRO")).is_ok());

        let schematic_case = root.join("schematic");
        fs::create_dir_all(&schematic_case).unwrap();
        fs::copy(
            corpus.join("hlr_test.kicad_pro"),
            schematic_case.join("hlr_test.kicad_pro"),
        )
        .unwrap();
        fs::copy(
            corpus.join("hlr_test.kicad_sch"),
            schematic_case.join("hlr_test.KICAD_SCH"),
        )
        .unwrap();
        assert!(load_design_sources(&schematic_case.join("hlr_test.KICAD_SCH")).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
}

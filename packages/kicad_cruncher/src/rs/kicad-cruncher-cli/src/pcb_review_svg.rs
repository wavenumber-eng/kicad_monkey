//! Cruncher-owned native PCB copper-layer review SVG composition.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::num::NonZeroU64;

use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::native_svg_render_request::{
    CanonicalUint64Decimal, NativeBoardSvgDocument, NativeSvgPlotDocument,
    NativeSvgPositiveSafeInteger, NativeSvgRenderLimits, NativeSvgRenderRequestA0,
    NativeSvgViewport,
};
use kicad_monkey_core::{PcbLimits, PcbView, ProjectDocument, ProjectLimits};
use kicad_monkey_svg::{SvgMetrics, render_svg};
use serde_json::{Value, json};

use crate::design::{
    BoardPlotDocument, DesignError, LoadedDesignSources, SchematicSvgRenderLimits, sha256_hex,
};

const ENRICHMENT_SCHEMA: &str = "kicad_monkey.pcb.svg.enrichment.a0";
const REVIEW_THEME: &str = "kicad_cruncher.design_review.pcb_svg.a0";
const EDGE_COLOR: &str = "#000000";
const TRACE_COLOR: &str = "#B8B8B8";
const PAD_COLOR: &str = "#000000";
const PTH_DRILL_COLOR: &str = "#2563EB";
const PTH_SLOT_COLOR: &str = "#0891B2";
const NPTH_DRILL_COLOR: &str = "#DC2626";
const NPTH_SLOT_COLOR: &str = "#F97316";
const UNKNOWN_HOLE_COLOR: &str = "#6B7280";
// The enrichment shape retains at most twelve copies of any PCB-authored
// string (layer names are the widest fan-out). Sixteen leaves four copies of
// headroom while remaining a structural upper bound, not a sampled estimate.
const METADATA_PCB_STRING_COPIES: usize = 16;
// Project variables/classes fan out into at most two retained maps.
const METADATA_PROJECT_STRING_COPIES: usize = 4;
// One source item expands into fewer than 32 Value/map/vector entries. This
// charge covers node structs plus BTree/Vec spare capacity on 64-bit targets.
const METADATA_NODE_BYTES_PER_ITEM: usize = 4096;
const METADATA_FIXED_NODE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcbReviewSvgLimits {
    pub max_layers: usize,
    pub max_records_per_layer: usize,
    pub max_operations_per_layer: usize,
    pub max_total_filter_work: usize,
    pub max_metadata_items: usize,
    pub max_metadata_materialized_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_total_materialized_bytes: usize,
    pub max_total_composition_work: usize,
    pub max_svg_bytes_per_layer: usize,
    pub max_total_svg_bytes: usize,
    pub native: SchematicSvgRenderLimits,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PcbReviewSvgBuildProfile {
    pub source_binding_ns: u64,
    pub pcb_view_parse_ns: u64,
    pub project_parse_ns: u64,
    pub metadata_preflight_ns: u64,
    pub metadata_materialization_ns: u64,
    pub contract_usage_preflight_ns: u64,
    pub contract_serialized_size_ns: u64,
    pub metadata_serialization_ns: u64,
    pub layer_filter_ns: u64,
    pub render_request_ns: u64,
    pub native_render_ns: u64,
    pub composition_preflight_ns: u64,
    pub compose_review_svg_ns: u64,
    pub artifact_finalize_ns: u64,
}

impl Default for PcbReviewSvgLimits {
    fn default() -> Self {
        Self {
            max_layers: 64,
            max_records_per_layer: 1_000_000,
            max_operations_per_layer: 4_000_000,
            max_total_filter_work: 256_000_000,
            max_metadata_items: 4_000_000,
            max_metadata_materialized_bytes: 512 * 1024 * 1024,
            max_metadata_bytes: 256 * 1024 * 1024,
            max_total_materialized_bytes: 4 * 1024 * 1024 * 1024,
            max_total_composition_work: 8 * 1024 * 1024 * 1024,
            max_svg_bytes_per_layer: 512 * 1024 * 1024,
            max_total_svg_bytes: 1024 * 1024 * 1024,
            native: crate::design::SchematicSvgDocumentsLimits::default().per_document,
        }
    }
}

#[derive(Debug)]
pub struct PcbReviewSvg {
    pub layer: String,
    pub included_layers: Vec<String>,
    pub drill_slot_record_count: usize,
    pub svg: String,
    pub metrics: SvgMetrics,
    pub viewport_bounds_nm: [i64; 4],
}

pub fn build_pcb_review_svgs(
    loaded: &LoadedDesignSources,
    document: &BoardPlotDocument,
) -> Result<Vec<PcbReviewSvg>, DesignError> {
    build_pcb_review_svgs_with_limits(loaded, document, PcbReviewSvgLimits::default())
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded pass keeps each filtered document, render, and enrichment together"
)]
pub fn build_pcb_review_svgs_with_limits(
    loaded: &LoadedDesignSources,
    document: &BoardPlotDocument,
    limits: PcbReviewSvgLimits,
) -> Result<Vec<PcbReviewSvg>, DesignError> {
    build_pcb_review_svgs_internal(loaded, document, limits, false)
        .map(|(artifacts, _profile)| artifacts)
}

pub(crate) fn build_pcb_review_svgs_profiled(
    loaded: &LoadedDesignSources,
    document: &BoardPlotDocument,
    limits: PcbReviewSvgLimits,
) -> Result<(Vec<PcbReviewSvg>, PcbReviewSvgBuildProfile), DesignError> {
    build_pcb_review_svgs_internal(loaded, document, limits, true)
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded pass keeps each filtered document, render, enrichment, and timing together"
)]
fn build_pcb_review_svgs_internal(
    loaded: &LoadedDesignSources,
    document: &BoardPlotDocument,
    limits: PcbReviewSvgLimits,
    profile_enabled: bool,
) -> Result<(Vec<PcbReviewSvg>, PcbReviewSvgBuildProfile), DesignError> {
    let mut profile = PcbReviewSvgBuildProfile::default();
    let binding_started = profile_enabled.then(std::time::Instant::now);
    if document.copper_layers.len() > limits.max_layers {
        return Err(DesignError::new("PCB review layer count exceeds its limit"));
    }
    let source = loaded
        .pcb_source
        .as_deref()
        .ok_or_else(|| DesignError::new("PCB source is missing"))?;
    if sha256_hex(source.as_bytes()) != document.source_sha256 {
        return Err(DesignError::new(
            "PCB review plot document does not belong to the loaded PCB source",
        ));
    }
    profile.source_binding_ns = profile_elapsed_ns(binding_started);
    let view_started = profile_enabled.then(std::time::Instant::now);
    let view = PcbView::parse(source, PcbLimits::default())
        .map_err(|error| DesignError::context("could not parse PCB enrichment facts", error))?;
    profile.pcb_view_parse_ns = profile_elapsed_ns(view_started);
    let project_started = profile_enabled.then(std::time::Instant::now);
    let project = loaded
        .bundle
        .project()
        .map(|source| ProjectDocument::from_reader(source.bytes(), ProjectLimits::default()))
        .transpose()
        .map_err(|error| DesignError::context("could not parse PCB project", error))?;
    profile.project_parse_ns = profile_elapsed_ns(project_started);
    let metadata_preflight_started = profile_enabled.then(std::time::Instant::now);
    let metadata_materialized_bytes = preflight_enrichment_metadata(
        loaded,
        &view,
        project.as_ref(),
        &document.source_path,
        limits,
    )?;
    profile.metadata_preflight_ns = profile_elapsed_ns(metadata_preflight_started);
    let [min_x, min_y, max_x, max_y] = document
        .bounds
        .ok_or_else(|| DesignError::new("PCB plot document has no bounded geometry"))?;
    let bounds = Bounds {
        min_x,
        min_y,
        max_x,
        max_y,
    };
    let mut composition = CompositionBudget::new(limits);
    composition.reserve(metadata_materialized_bytes, metadata_materialized_bytes)?;
    let metadata_started = profile_enabled.then(std::time::Instant::now);
    let mut metadata = enrichment_metadata(
        loaded,
        &view,
        project.as_ref(),
        bounds,
        &[],
        &document.source_path,
    )?;
    profile.metadata_materialization_ns = profile_elapsed_ns(metadata_started);
    let usage_started = profile_enabled.then(std::time::Instant::now);
    let (contract_materialized_bytes, contract_preflight_work) =
        value_projection_usage(&document.value, composition.remaining_work())?;
    composition.reserve(0, contract_preflight_work)?;
    profile.contract_usage_preflight_ns = profile_elapsed_ns(usage_started);
    let size_started = profile_enabled.then(std::time::Instant::now);
    let contract_bytes = serialized_size(&document.value, composition.remaining_work())?;
    composition.reserve(0, contract_bytes)?;
    profile.contract_serialized_size_ns = profile_elapsed_ns(size_started);
    let mut total_svg_bytes = 0_usize;
    let mut filter_work = FilterWork::new(limits.max_total_filter_work);
    let mut artifacts = Vec::with_capacity(document.copper_layers.len());
    for layer in &document.copper_layers {
        // A filtered Value can retain the complete contract plus Value/map
        // allocation overhead. Reserve its structural upper bound before the
        // first clone in `filter_document`.
        composition.begin_temporary(contract_materialized_bytes, contract_bytes)?;
        let included_layers = vec![layer.clone(), "Edge.Cuts".to_owned()];
        metadata["view"]["included_layers"] = json!(&included_layers);
        metadata["view"]["includes_board_outline"] = json!(true);
        let metadata_limit = limits
            .max_metadata_bytes
            .min(composition.remaining_materialized())
            .min(composition.remaining_work());
        let metadata_serialization_started = profile_enabled.then(std::time::Instant::now);
        let mut metadata_writer = LimitedVecWriter::new(metadata_limit);
        serde_json::to_writer_pretty(&mut metadata_writer, &metadata).map_err(|error| {
            DesignError::context("PCB enrichment metadata exceeds its limit", error)
        })?;
        let metadata_text = metadata_writer.into_string()?;
        composition.reserve(metadata_text.len(), metadata_text.len())?;
        profile.metadata_serialization_ns = profile
            .metadata_serialization_ns
            .saturating_add(profile_elapsed_ns(metadata_serialization_started));
        let filter_started = profile_enabled.then(std::time::Instant::now);
        let filtered =
            filter_document(&document.value, &included_layers, limits, &mut filter_work)?;
        profile.layer_filter_ns = profile
            .layer_filter_ns
            .saturating_add(profile_elapsed_ns(filter_started));
        let remaining = limits
            .max_total_svg_bytes
            .checked_sub(total_svg_bytes)
            .ok_or_else(|| DesignError::new("PCB review aggregate SVG limit exceeded"))?;
        let render_limit = remaining
            .min(limits.max_svg_bytes_per_layer)
            .min(composition.remaining_materialized())
            .min(composition.remaining_work());
        let request_started = profile_enabled.then(std::time::Instant::now);
        let request = render_request(filtered, bounds, limits.native, render_limit)?;
        profile.render_request_ns = profile
            .render_request_ns
            .saturating_add(profile_elapsed_ns(request_started));
        let render_started = profile_enabled.then(std::time::Instant::now);
        let rendered = render_svg(&request)
            .map_err(|error| DesignError::context("could not render PCB base SVG", error))?;
        profile.native_render_ns = profile
            .native_render_ns
            .saturating_add(profile_elapsed_ns(render_started));
        composition.reserve(rendered.svg.len(), rendered.svg.len())?;
        let filtered = match &request.document {
            NativeSvgPlotDocument::BoardSvgDocument(document) => &document.value,
            _ => return Err(DesignError::new("PCB SVG request lost its board document")),
        };
        let composition_preflight_started = profile_enabled.then(std::time::Instant::now);
        let composition_upper = composition_upper_bound(
            &rendered.svg,
            contract_bytes,
            &metadata_text,
            &document.source_path,
            &included_layers,
        )?;
        composition.reserve(composition_upper, composition_upper)?;
        profile.composition_preflight_ns = profile
            .composition_preflight_ns
            .saturating_add(profile_elapsed_ns(composition_preflight_started));
        let compose_started = profile_enabled.then(std::time::Instant::now);
        let svg = compose_review_svg(
            &rendered.svg,
            filtered,
            layer,
            &included_layers,
            &document.source_path,
            &metadata_text,
            bounds,
            limits.max_svg_bytes_per_layer.min(remaining),
        )?;
        profile.compose_review_svg_ns = profile
            .compose_review_svg_ns
            .saturating_add(profile_elapsed_ns(compose_started));
        let finalize_started = profile_enabled.then(std::time::Instant::now);
        total_svg_bytes = total_svg_bytes
            .checked_add(svg.len())
            .ok_or_else(|| DesignError::new("PCB review aggregate SVG byte count overflowed"))?;
        if total_svg_bytes > limits.max_total_svg_bytes {
            return Err(DesignError::new("PCB review aggregate SVG limit exceeded"));
        }
        let drill_slot_record_count = svg
            .lines()
            .filter(|line| {
                line.contains("data-hole-kind=")
                    && (line.contains("data-primitive=\"pad-hole\"")
                        || line.contains("data-primitive=\"via-hole\""))
            })
            .count();
        artifacts.push(PcbReviewSvg {
            layer: layer.clone(),
            included_layers,
            drill_slot_record_count,
            svg,
            metrics: rendered.metrics,
            viewport_bounds_nm: [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y],
        });
        composition.end_temporary(contract_materialized_bytes)?;
        profile.artifact_finalize_ns = profile
            .artifact_finalize_ns
            .saturating_add(profile_elapsed_ns(finalize_started));
    }
    Ok((artifacts, profile))
}

fn profile_elapsed_ns(started: Option<std::time::Instant>) -> u64 {
    started.map_or(0, |started| {
        u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
    })
}

fn value_projection_usage(
    value: &Value,
    maximum_work: usize,
) -> Result<(usize, usize), DesignError> {
    fn add(total: &mut usize, value: usize) -> Result<(), DesignError> {
        *total = total.checked_add(value).ok_or_else(|| {
            DesignError::new("PCB review Value materialization estimate overflowed")
        })?;
        Ok(())
    }

    fn work(total: &mut usize, value: usize, maximum: usize) -> Result<(), DesignError> {
        *total = total
            .checked_add(value)
            .filter(|value| *value <= maximum)
            .ok_or_else(|| {
                DesignError::new("PCB review aggregate composition work exceeds its limit")
            })?;
        Ok(())
    }

    fn visit(
        value: &Value,
        total: &mut usize,
        visited: &mut usize,
        maximum_work: usize,
    ) -> Result<(), DesignError> {
        // Covers the Value enum/vector slot or one BTreeMap entry, including
        // allocator metadata and alignment on supported 64-bit targets.
        add(total, 128)?;
        work(visited, 1, maximum_work)?;
        match value {
            Value::String(text) => {
                add(total, text.len().saturating_mul(2))?;
                work(visited, text.len(), maximum_work)
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, total, visited, maximum_work)?;
                }
                Ok(())
            }
            Value::Object(values) => {
                for (key, value) in values {
                    add(total, 128)?;
                    add(total, key.len().saturating_mul(2))?;
                    work(visited, key.len().saturating_add(1), maximum_work)?;
                    visit(value, total, visited, maximum_work)?;
                }
                Ok(())
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        }
    }

    let mut total = 4096_usize;
    let mut work = 0_usize;
    visit(value, &mut total, &mut work, maximum_work)?;
    Ok((total, work))
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

impl Bounds {
    fn width(self) -> u64 {
        self.max_x.abs_diff(self.min_x).max(1)
    }

    fn height(self) -> u64 {
        self.max_y.abs_diff(self.min_y).max(1)
    }
}

struct CompositionBudget {
    materialized: usize,
    temporary_materialized: usize,
    maximum_materialized: usize,
    work: usize,
    maximum_work: usize,
}

impl CompositionBudget {
    const fn new(limits: PcbReviewSvgLimits) -> Self {
        Self {
            materialized: 0,
            temporary_materialized: 0,
            maximum_materialized: limits.max_total_materialized_bytes,
            work: 0,
            maximum_work: limits.max_total_composition_work,
        }
    }

    fn reserve(&mut self, materialized: usize, work: usize) -> Result<(), DesignError> {
        let retained = self.materialized.checked_add(materialized).ok_or_else(|| {
            DesignError::new("PCB review aggregate materialization exceeds its limit")
        })?;
        retained
            .checked_add(self.temporary_materialized)
            .filter(|value| *value <= self.maximum_materialized)
            .ok_or_else(|| {
                DesignError::new("PCB review aggregate materialization exceeds its limit")
            })?;
        self.materialized = retained;
        self.work = self
            .work
            .checked_add(work)
            .filter(|value| *value <= self.maximum_work)
            .ok_or_else(|| {
                DesignError::new("PCB review aggregate composition work exceeds its limit")
            })?;
        Ok(())
    }

    fn begin_temporary(&mut self, materialized: usize, work: usize) -> Result<(), DesignError> {
        if self.temporary_materialized != 0 {
            return Err(DesignError::new(
                "PCB review temporary materialization scopes overlap",
            ));
        }
        self.materialized
            .checked_add(materialized)
            .filter(|value| *value <= self.maximum_materialized)
            .ok_or_else(|| {
                DesignError::new("PCB review aggregate materialization exceeds its limit")
            })?;
        self.work = self
            .work
            .checked_add(work)
            .filter(|value| *value <= self.maximum_work)
            .ok_or_else(|| {
                DesignError::new("PCB review aggregate composition work exceeds its limit")
            })?;
        self.temporary_materialized = materialized;
        Ok(())
    }

    fn end_temporary(&mut self, materialized: usize) -> Result<(), DesignError> {
        if self.temporary_materialized != materialized {
            return Err(DesignError::new(
                "PCB review temporary materialization scope is inconsistent",
            ));
        }
        self.temporary_materialized = 0;
        Ok(())
    }

    const fn remaining_materialized(&self) -> usize {
        self.maximum_materialized
            .saturating_sub(self.materialized)
            .saturating_sub(self.temporary_materialized)
    }

    const fn remaining_work(&self) -> usize {
        self.maximum_work.saturating_sub(self.work)
    }
}

fn serialized_size(value: &Value, maximum: usize) -> Result<usize, DesignError> {
    let mut writer = CountWriter {
        written: 0,
        maximum,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| DesignError::context("could not size PCB plot contract", error))?;
    Ok(writer.written)
}

fn composition_upper_bound(
    base: &str,
    contract_bytes: usize,
    metadata: &str,
    source_path: &str,
    included_layers: &[String],
) -> Result<usize, DesignError> {
    // During composition the base, split/categorized group strings, and final
    // output can coexist. XML escaping can expand authored text by at most 6x;
    // record attributes are bounded by the already-sized contract.
    let included_bytes = included_layers.iter().try_fold(0_usize, |total, layer| {
        total
            .checked_add(layer.len())
            .ok_or_else(|| DesignError::new("PCB review composition layer byte count overflowed"))
    })?;
    base.len()
        .checked_mul(4)
        .and_then(|value| {
            contract_bytes
                .checked_mul(7)
                .and_then(|part| value.checked_add(part))
        })
        .and_then(|value| {
            metadata
                .len()
                .checked_mul(7)
                .and_then(|part| value.checked_add(part))
        })
        .and_then(|value| {
            source_path
                .len()
                .checked_mul(6)
                .and_then(|part| value.checked_add(part))
        })
        .and_then(|value| {
            included_bytes
                .checked_mul(6)
                .and_then(|part| value.checked_add(part))
        })
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or_else(|| DesignError::new("PCB review composition upper bound overflowed"))
}

fn filter_document(
    document: &Value,
    included_layers: &[String],
    limits: PcbReviewSvgLimits,
    work: &mut FilterWork,
) -> Result<Value, DesignError> {
    let wanted = included_layers
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let source = document
        .as_object()
        .ok_or_else(|| DesignError::new("board plot document is not an object"))?;
    let mut output = source
        .iter()
        .filter(|(name, _)| name.as_str() != "records")
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    let mut records = Vec::new();
    let mut total_operations = 0_usize;
    for record in array(document, "records")? {
        work.charge(1)?;
        let layers = declared_layers(record, work)?;
        let is_via = record["kind"].as_str() == Some("via");
        if !record_has_npth(record, work)?
            && !layers.is_empty()
            && !layer_set_matches(&layers, &wanted, is_via)
        {
            continue;
        }
        if records.len() == limits.max_records_per_layer {
            return Err(DesignError::new(
                "PCB review record count exceeds its limit",
            ));
        }
        let operations = filter_operations(
            record,
            &wanted,
            included_layers,
            limits.max_operations_per_layer - total_operations,
            work,
        )?;
        total_operations = total_operations
            .checked_add(operations.len())
            .ok_or_else(|| DesignError::new("PCB review operation count overflowed"))?;
        if total_operations > limits.max_operations_per_layer {
            return Err(DesignError::new(
                "PCB review operation count exceeds its limit",
            ));
        }
        let retained_object = record
            .as_object()
            .ok_or_else(|| DesignError::new("board record is not an object"))?;
        let mut retained_object = retained_object
            .iter()
            .filter(|(name, _)| !matches!(name.as_str(), "operations" | "operation_count"))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();
        retained_object.insert("operation_count".to_owned(), json!(operations.len()));
        retained_object.insert("operations".to_owned(), Value::Array(operations));
        records.push(Value::Object(retained_object));
    }
    output.insert("records".to_owned(), Value::Array(records));
    output.insert("total_operations".to_owned(), json!(total_operations));
    Ok(Value::Object(output))
}

fn filter_operations(
    record: &Value,
    wanted: &BTreeSet<&str>,
    included_layers: &[String],
    maximum: usize,
    work: &mut FilterWork,
) -> Result<Vec<Value>, DesignError> {
    let source = array(record, "operations")?;
    let mut output = Vec::new();
    let mut index = 0;
    while index < source.len() {
        work.charge(1)?;
        if source[index]["kind"].as_str() == Some("StartBlock")
            && index + 2 < source.len()
            && source[index + 2]["kind"].as_str() == Some("EndBlock")
        {
            let inner = &source[index + 1];
            let layers = declared_layers(inner, work)?;
            let role = inner["role"].as_str().unwrap_or_default();
            if role == "npth_hole"
                || layers.is_empty()
                || layer_set_matches(
                    &layers,
                    wanted,
                    matches!(role, "via_aperture" | "via_drill"),
                )
            {
                if layers.is_empty() && record["kind"].as_str() == Some("footprint") {
                    let bound =
                        bind_unlayered_footprint_block(&source[index..index + 3], included_layers)?;
                    extend_operations(&mut output, &bound, maximum)?;
                } else {
                    extend_operations(&mut output, &source[index..index + 3], maximum)?;
                }
            }
            index += 3;
            continue;
        }
        let operation = &source[index];
        let layers = declared_layers(operation, work)?;
        let role = operation["role"].as_str().unwrap_or_default();
        if matches!(operation["kind"].as_str(), Some("StartBlock" | "EndBlock"))
            || role == "npth_hole"
            || layers.is_empty()
            || layer_set_matches(
                &layers,
                wanted,
                matches!(role, "via_aperture" | "via_drill"),
            )
        {
            extend_operations(&mut output, std::slice::from_ref(operation), maximum)?;
        }
        index += 1;
    }
    for (index, operation) in output.iter_mut().enumerate() {
        if let Some(object) = operation.as_object_mut() {
            object.insert("index".to_owned(), json!(index));
        }
    }
    Ok(output)
}

fn bind_unlayered_footprint_block(
    operations: &[Value],
    layers: &[String],
) -> Result<[Value; 3], DesignError> {
    let [start, inner, end] = operations else {
        return Err(DesignError::new("unlayered footprint block is malformed"));
    };
    let mut start = start.clone();
    let mut inner = inner.clone();
    let start_object = start
        .as_object_mut()
        .ok_or_else(|| DesignError::new("footprint block start is not an object"))?;
    start_object.insert("layers".to_owned(), json!(layers));
    let attrs = start_object
        .entry("extra_attrs")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| DesignError::new("footprint block attributes are not an object"))?;
    attrs.insert("layer_names".to_owned(), json!(layers.join(",")));
    inner
        .as_object_mut()
        .ok_or_else(|| DesignError::new("footprint block operation is not an object"))?
        .insert("layers".to_owned(), json!(layers));
    Ok([start, inner, end.clone()])
}

fn extend_operations(
    output: &mut Vec<Value>,
    operations: &[Value],
    maximum: usize,
) -> Result<(), DesignError> {
    if output.len().saturating_add(operations.len()) > maximum {
        return Err(DesignError::new(
            "PCB review operation count exceeds its limit",
        ));
    }
    output.extend(operations.iter().cloned());
    Ok(())
}

struct FilterWork {
    used: usize,
    maximum: usize,
}

impl FilterWork {
    const fn new(maximum: usize) -> Self {
        Self { used: 0, maximum }
    }

    fn charge(&mut self, amount: usize) -> Result<(), DesignError> {
        self.used = self
            .used
            .checked_add(amount)
            .filter(|used| *used <= self.maximum)
            .ok_or_else(|| DesignError::new("PCB review filtering work exceeds its limit"))?;
        Ok(())
    }
}

fn declared_layers(value: &Value, work: &mut FilterWork) -> Result<BTreeSet<String>, DesignError> {
    work.charge(1)?;
    let mut layers = BTreeSet::new();
    if let Some(layer) = value["layer"].as_str().filter(|layer| !layer.is_empty()) {
        layers.insert(layer.to_owned());
    }
    for key in ["layers", "fill_layers"] {
        for layer in value[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !layer.is_empty() {
                layers.insert(layer.to_owned());
            }
        }
    }
    if let Some(operations) = value["operations"].as_array() {
        for operation in operations {
            layers.extend(declared_layers(operation, work)?);
        }
    }
    Ok(layers)
}

fn record_has_npth(record: &Value, work: &mut FilterWork) -> Result<bool, DesignError> {
    for operation in record["operations"].as_array().into_iter().flatten() {
        work.charge(1)?;
        if operation["role"].as_str() == Some("npth_hole") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn layer_set_matches(
    declared: &BTreeSet<String>,
    wanted: &BTreeSet<&str>,
    allow_copper_span: bool,
) -> bool {
    if declared.iter().any(|layer| layer_matches(layer, wanted)) {
        return true;
    }
    if !allow_copper_span {
        return false;
    }
    let mut indices = declared.iter().filter_map(|layer| copper_index(layer));
    let Some(first) = indices.next() else {
        return false;
    };
    let (mut low, mut high) = (first, first);
    for index in indices {
        low = low.min(index);
        high = high.max(index);
    }
    low != high
        && wanted
            .iter()
            .filter_map(|layer| copper_index(layer))
            .any(|index| low <= index && index <= high)
}

fn layer_matches(layer: &str, wanted: &BTreeSet<&str>) -> bool {
    wanted.contains(layer)
        || (layer == "*.Cu" && wanted.iter().any(|value| value.ends_with(".Cu")))
        || (layer == "*.Mask" && wanted.iter().any(|value| value.ends_with(".Mask")))
        || (layer == "F&B.Cu" && (wanted.contains("F.Cu") || wanted.contains("B.Cu")))
}

fn copper_index(layer: &str) -> Option<usize> {
    match layer {
        "F.Cu" => Some(0),
        "B.Cu" => Some(10_000),
        value if value.starts_with("In") && value.ends_with(".Cu") => {
            value[2..value.len() - 3].parse().ok()
        }
        _ => None,
    }
}

fn render_request(
    document: Value,
    bounds: Bounds,
    limits: SchematicSvgRenderLimits,
    max_svg_bytes: usize,
) -> Result<NativeSvgRenderRequestA0, DesignError> {
    Ok(NativeSvgRenderRequestA0 {
        document: NativeSvgPlotDocument::BoardSvgDocument(NativeBoardSvgDocument {
            kind: "board".to_owned(),
            value: document,
        }),
        limits: NativeSvgRenderLimits {
            max_block_depth: limits.max_block_depth,
            max_image_encoded_bytes: decimal(limits.max_image_encoded_bytes),
            max_operations: limits.max_operations,
            max_points: decimal(limits.max_points),
            max_records: limits.max_records,
            max_render_work: decimal(limits.max_render_work),
            max_result_bytes: decimal(max_svg_bytes),
            max_svg_bytes: decimal(max_svg_bytes),
            max_svg_elements: decimal(limits.max_svg_elements),
            max_text_bytes: decimal(limits.max_text_bytes),
        },
        profile: "plotter-base-a0".to_owned(),
        type_: "kicad_monkey.native.svg.request".to_owned(),
        version: "a0".to_owned(),
        viewport: NativeSvgViewport {
            height_nm: positive(bounds.height())?,
            min_x_nm: JavaScriptSafeInteger::try_from(bounds.min_x)
                .map_err(|error| DesignError::context("PCB viewport X is unsafe", error))?,
            min_y_nm: JavaScriptSafeInteger::try_from(bounds.min_y)
                .map_err(|error| DesignError::context("PCB viewport Y is unsafe", error))?,
            width_nm: positive(bounds.width())?,
        },
    })
}

fn decimal(value: usize) -> CanonicalUint64Decimal {
    CanonicalUint64Decimal(value.to_string())
}

fn positive(value: u64) -> Result<NativeSvgPositiveSafeInteger, DesignError> {
    NonZeroU64::new(value)
        .map(NativeSvgPositiveSafeInteger)
        .ok_or_else(|| DesignError::new("PCB viewport is empty"))
}

fn preflight_enrichment_metadata(
    loaded: &LoadedDesignSources,
    view: &PcbView<'_>,
    project: Option<&ProjectDocument>,
    source_path: &str,
    limits: PcbReviewSvgLimits,
) -> Result<usize, DesignError> {
    let mut items = 0_usize;
    for result in view.layers() {
        result.map_err(|error| DesignError::context("could not read PCB layer", error))?;
        items = checked_metadata_item(items, limits)?;
    }
    for result in view.nets() {
        result.map_err(|error| DesignError::context("could not read PCB net", error))?;
        items = checked_metadata_item(items, limits)?;
    }
    for result in view.footprints() {
        result.map_err(|error| DesignError::context("could not read PCB footprint", error))?;
        items = checked_metadata_item(items, limits)?;
    }
    for result in view.footprint_properties() {
        result.map_err(|error| DesignError::context("could not read PCB property", error))?;
        items = checked_metadata_item(items, limits)?;
    }
    if let Some(stackup) = view
        .setup()
        .map_err(|error| DesignError::context("could not read PCB setup", error))?
        .and_then(|setup| setup.stackup)
    {
        for _ in stackup.layers {
            items = checked_metadata_item(items, limits)?;
        }
    }
    if let Some(project) = project {
        let variables = project
            .view()
            .text_variables()
            .map_err(|error| DesignError::context("could not read PCB project variables", error))?;
        for _ in variables {
            items = checked_metadata_item(items, limits)?;
        }
        let settings = project
            .view()
            .net_settings()
            .map_err(|error| DesignError::context("could not read PCB net classes", error))?;
        for _ in settings.assignments {
            items = checked_metadata_item(items, limits)?;
        }
    }
    let source_bytes = loaded.pcb_source.as_deref().map_or(0, str::len);
    let project_bytes = loaded
        .bundle
        .project()
        .map_or(0, |source| source.bytes().len());
    let materialized_bytes =
        metadata_materialized_upper(source_bytes, project_bytes, source_path.len(), items)?;
    enforce_metadata_materialized_limit(
        materialized_bytes,
        limits.max_metadata_materialized_bytes,
    )?;
    Ok(materialized_bytes)
}

fn enforce_metadata_materialized_limit(bytes: usize, maximum: usize) -> Result<(), DesignError> {
    if bytes > maximum {
        return Err(DesignError::new(
            "PCB enrichment metadata materialized-byte upper bound exceeds its limit",
        ));
    }
    Ok(())
}

fn metadata_materialized_upper(
    source_bytes: usize,
    project_bytes: usize,
    source_path_bytes: usize,
    items: usize,
) -> Result<usize, DesignError> {
    source_bytes
        .checked_mul(METADATA_PCB_STRING_COPIES)
        .and_then(|bytes| {
            project_bytes
                .checked_mul(METADATA_PROJECT_STRING_COPIES)
                .and_then(|project| bytes.checked_add(project))
        })
        .and_then(|bytes| {
            source_path_bytes
                .checked_mul(4)
                .and_then(|path| bytes.checked_add(path))
        })
        .and_then(|bytes| {
            items
                .checked_mul(METADATA_NODE_BYTES_PER_ITEM)
                .and_then(|nodes| bytes.checked_add(nodes))
        })
        .and_then(|bytes| bytes.checked_add(METADATA_FIXED_NODE_BYTES))
        .ok_or_else(|| DesignError::new("PCB enrichment metadata budget overflowed"))
}

fn checked_metadata_item(items: usize, limits: PcbReviewSvgLimits) -> Result<usize, DesignError> {
    items
        .checked_add(1)
        .filter(|items| *items <= limits.max_metadata_items)
        .ok_or_else(|| DesignError::new("PCB enrichment metadata item count exceeds its limit"))
}

#[allow(
    clippy::too_many_lines,
    reason = "the established enrichment contract is assembled together for exact field parity"
)]
fn enrichment_metadata(
    loaded: &LoadedDesignSources,
    view: &PcbView<'_>,
    project: Option<&ProjectDocument>,
    bounds: Bounds,
    included_layers: &[String],
    source_path: &str,
) -> Result<Value, DesignError> {
    let layers = view
        .layers()
        .map(|layer| layer.map_err(|error| DesignError::context("could not read PCB layer", error)))
        .collect::<Result<Vec<_>, _>>()?;
    let nets = view
        .nets()
        .map(|net| net.map_err(|error| DesignError::context("could not read PCB net", error)))
        .collect::<Result<Vec<_>, _>>()?;
    let footprints = view
        .footprints()
        .map(|footprint| {
            footprint.map_err(|error| DesignError::context("could not read PCB footprint", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut parameters = vec![BTreeMap::new(); footprints.len()];
    for property in view.footprint_properties() {
        let property =
            property.map_err(|error| DesignError::context("could not read PCB property", error))?;
        if let Some(values) = parameters.get_mut(property.footprint_index) {
            values.insert(property.name, property.value);
        }
    }
    let project_variables = project
        .map(|project| project.view().text_variables())
        .transpose()
        .map_err(|error| DesignError::context("could not read PCB project variables", error))?
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let net_name_to_classes = project
        .map(|project| project.view().net_settings())
        .transpose()
        .map_err(|error| DesignError::context("could not read PCB net classes", error))?
        .map(|settings| settings.assignments.into_iter().collect::<BTreeMap<_, _>>())
        .unwrap_or_default();
    let setup = view
        .setup()
        .map_err(|error| DesignError::context("could not read PCB setup", error))?;
    let aux_axis_origin = setup.as_ref().map_or([0.0, 0.0], |setup| {
        [setup.aux_axis_origin.x, setup.aux_axis_origin.y]
    });
    let stackup = setup.as_ref().and_then(|setup| setup.stackup.as_ref());
    let stackup_layers = stackup
        .map(|stackup| {
            stackup.layers.iter().enumerate().map(|(index, layer)| json!({
                "index": index,
                "name": layer.name,
                "display_name": layers.iter().find(|item| item.name == layer.name).and_then(|item| item.user_name.clone()).filter(|name| !name.is_empty()).unwrap_or_else(|| layer.name.clone()),
                "type": layer.type_name,
                "role": stackup_role(&layer.name, &layer.type_name),
                "thickness_mm": layer.thickness,
                "thickness_locked": layer.thickness_locked,
                "material": layer.material,
                "epsilon_r": layer.epsilon_r.filter(|value| *value != 0.0),
                "loss_tangent": layer.loss_tangent.filter(|value| *value != 0.0),
                "color": layer.color,
                "sublayers": [],
            })).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let computed_thickness = stackup
        .map(|stackup| {
            stackup
                .layers
                .iter()
                .map(|layer| layer.thickness)
                .sum::<f64>()
        })
        .unwrap_or_default();
    Ok(json!({
        "schema": ENRICHMENT_SCHEMA,
        "source": {"kicad_pcb_file": source_path},
        "project": {"text_variables": project_variables},
        "board": {
            "bbox_mm": [nm_mm(bounds.min_x), nm_mm(bounds.min_y), nm_mm(bounds.max_x), nm_mm(bounds.max_y)],
            "aux_axis_origin_mm": aux_axis_origin,
            "thickness_mm": document_number(&loaded.pcb_source, "thickness").unwrap_or(1.6),
            "stackup": {
                "present": stackup.is_some(),
                "computed_thickness_mm": computed_thickness,
                "copper_finish": stackup.map(|value| value.copper_finish.as_str()).unwrap_or_default(),
                "dielectric_constraints": stackup.is_some_and(|value| value.dielectric_constraints),
                "edge_connector": stackup.map(|value| value.edge_connector.as_str()).unwrap_or("none"),
                "edge_plating": stackup.is_some_and(|value| value.edge_plating),
                "layers": stackup_layers,
            },
        },
        "view": {
            "kind": "layer_set",
            "included_layers": included_layers,
            "profile": "enriched",
            "includes_board_outline": included_layers.iter().any(|layer| layer == "Edge.Cuts"),
        },
        "layers": {
            "all_layer_names": layers.iter().map(|layer| layer.name.clone()).collect::<Vec<_>>(),
            "layer_ordinal_to_name": layers.iter().map(|layer| (layer.ordinal.to_string(), layer.name.clone())).collect::<BTreeMap<_, _>>(),
            "layer_name_to_role": layers.iter().map(|layer| (layer.name.clone(), layer_role(&layer.name))).collect::<BTreeMap<_, _>>(),
            "layer_name_to_display_name": layers.iter().map(|layer| (layer.name.clone(), layer.user_name.clone().filter(|value| !value.is_empty()).unwrap_or_else(|| layer.name.clone()))).collect::<BTreeMap<_, _>>(),
            "layer_name_to_user_name": layers.iter().map(|layer| (layer.name.clone(), layer.user_name.clone().unwrap_or_default())).collect::<BTreeMap<_, _>>(),
            "layers": layers.iter().map(|layer| json!({
                "ordinal": layer.ordinal,
                "name": layer.name,
                "type": layer.kind,
                "role": layer_role(&layer.name),
                "user_name": layer.user_name.clone().unwrap_or_default(),
                "display_name": layer.user_name.clone().unwrap_or_else(|| layer.name.clone()),
            })).collect::<Vec<_>>(),
        },
        "lookup": {
            "net_index_to_name": nets.iter().map(|net| (net.code.to_string(), net.name.clone())).collect::<BTreeMap<_, _>>(),
            "net_name_to_classes": net_name_to_classes,
            "component_index_to_designator": footprints.iter().enumerate().filter_map(|(index, footprint)| footprint.reference.clone().filter(|value| !value.is_empty()).map(|value| (index.to_string(), value))).collect::<BTreeMap<_, _>>(),
            "component_index_to_uid": footprints.iter().enumerate().filter_map(|(index, footprint)| footprint.uuid.clone().filter(|value| !value.is_empty()).map(|value| (index.to_string(), value))).collect::<BTreeMap<_, _>>(),
        },
        "components": footprints.iter().enumerate().map(|(index, footprint)| json!({
            "index": index,
            "designator": footprint.reference.clone().unwrap_or_default(),
            "unique_id": footprint.uuid.clone().unwrap_or_default(),
            "footprint": footprint.library_link,
            "value": footprint.value.clone().unwrap_or_default(),
            "description": footprint.description,
            "layer": footprint.layer.clone().unwrap_or_default(),
            "x_mm": footprint.at_x.unwrap_or_default(),
            "y_mm": footprint.at_y.unwrap_or_default(),
            "rotation_deg": footprint.angle.unwrap_or_default(),
            "parameters": parameters[index],
        })).collect::<Vec<_>>(),
    }))
}

fn document_number(source: &Option<String>, name: &str) -> Option<f64> {
    let source = source.as_deref()?;
    let marker = format!("({name} ");
    let value = source.split_once(&marker)?.1.split_once(')')?.0.trim();
    value.parse().ok()
}

fn nm_mm(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

fn layer_role(layer: &str) -> &'static str {
    if layer.ends_with(".Cu") || matches!(layer, "*.Cu" | "F&B.Cu") {
        "copper"
    } else if layer == "Edge.Cuts" {
        "board-outline"
    } else if layer.ends_with(".Mask") || layer == "*.Mask" {
        "soldermask"
    } else if layer.ends_with(".SilkS") {
        "silkscreen"
    } else if layer.ends_with(".Paste") {
        "paste"
    } else if layer.ends_with(".Fab") {
        "fab"
    } else if layer.ends_with(".Courtyard") {
        "courtyard"
    } else if layer.ends_with(".User") || layer.starts_with("User.") {
        "user"
    } else {
        "other"
    }
}

fn stackup_role(name: &str, type_name: &str) -> &'static str {
    match type_name {
        "copper" => "copper",
        "core" | "prepreg" => "dielectric",
        "soldermask" => "soldermask",
        "silkscreen" => "silkscreen",
        _ if name.ends_with(".SilkS") => "silkscreen",
        _ if name.ends_with(".Mask") => "soldermask",
        _ if name.ends_with(".Paste") => "solderpaste",
        _ => "other",
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "all binding inputs and the bounded composition pass stay explicit at this boundary"
)]
fn compose_review_svg(
    base: &str,
    document: &Value,
    layer: &str,
    included_layers: &[String],
    source_path: &str,
    metadata: &str,
    bounds: Bounds,
    max_bytes: usize,
) -> Result<String, DesignError> {
    let records = array(document, "records")?;
    let mut lines = base.lines();
    let declaration = lines
        .next()
        .ok_or_else(|| DesignError::new("PCB SVG is empty"))?;
    let root = lines
        .next()
        .ok_or_else(|| DesignError::new("PCB SVG root is missing"))?;
    let background = lines
        .next()
        .ok_or_else(|| DesignError::new("PCB SVG background is missing"))?;
    let viewport = lines
        .next()
        .ok_or_else(|| DesignError::new("PCB SVG viewport is missing"))?;
    if !root.starts_with("<svg ") || !viewport.starts_with("<g transform=") {
        return Err(DesignError::new("PCB SVG base topology is invalid"));
    }
    let escaped_metadata_bytes = xml_text_escaped_len(metadata)?;
    let fixed_minimum = declaration
        .len()
        .checked_add(root.len())
        .and_then(|value| value.checked_add(background.len()))
        .and_then(|value| value.checked_add(escaped_metadata_bytes))
        .and_then(|value| value.checked_add(128))
        .ok_or_else(|| DesignError::new("PCB review SVG minimum size overflowed"))?;
    if fixed_minimum > max_bytes {
        return Err(DesignError::new("PCB review SVG exceeds its limit"));
    }
    let body = lines.collect::<Vec<_>>();
    if body.len() < 2 || body[body.len() - 2] != "</g>" || body[body.len() - 1] != "</svg>" {
        return Err(DesignError::new("PCB SVG base terminator is invalid"));
    }
    let groups = top_level_groups(&body[..body.len() - 2])?;
    if groups.len() != records.len() {
        return Err(DesignError::new(
            "PCB SVG records do not match the plot document",
        ));
    }
    let mut categorized = Vec::with_capacity(groups.len());
    let mut categorized_bytes = 0_usize;
    for (record, group) in records.iter().zip(groups) {
        let attrs = record_attrs(record);
        let category = review_category(&attrs);
        if let Some((normal, drill)) = split_via_group(record, group) {
            push_styled_group(
                &mut categorized,
                &mut categorized_bytes,
                max_bytes,
                &normal,
                &attrs,
                category,
                bounds,
            )?;
            let drill_attrs = via_hole_attrs(record);
            push_styled_group(
                &mut categorized,
                &mut categorized_bytes,
                max_bytes,
                &drill,
                &drill_attrs,
                "drill",
                bounds,
            )?;
        } else if let Some((normal, drill)) = split_footprint_holes(record, group) {
            push_styled_group(
                &mut categorized,
                &mut categorized_bytes,
                max_bytes,
                &normal,
                &attrs,
                category,
                bounds,
            )?;
            let drill_attrs = footprint_hole_attrs(record);
            push_styled_group(
                &mut categorized,
                &mut categorized_bytes,
                max_bytes,
                &drill,
                &drill_attrs,
                "drill",
                bounds,
            )?;
        } else {
            push_styled_group(
                &mut categorized,
                &mut categorized_bytes,
                max_bytes,
                group,
                &attrs,
                category,
                bounds,
            )?;
        }
    }
    categorized.sort_by_key(|(order, _)| *order);
    let width = nm_text(bounds.width());
    let height = nm_text(bounds.height());
    let root = root
        .replace(&format!("viewBox=\"0 0 {} {}\"", bounds.width(), bounds.height()), &format!("viewBox=\"0 0 {width} {height}\""))
        .replacen(">", &format!(
            " data-stage=\"enriched\" data-group-mode=\"source-record\" data-enrichment-schema=\"{ENRICHMENT_SCHEMA}\" data-view-kind=\"layer_set\" data-profile=\"enriched\" data-mirror-x=\"false\" data-source=\"{}\" data-included-layers=\"{}\" data-review-theme=\"{REVIEW_THEME}\" data-review-layer=\"{}\" data-review-draw-order=\"tracks,polygons-zones,edge-cuts,pads,drills-slots\">",
            xml_attr(source_path.rsplit(['/', '\\']).next().unwrap_or(source_path)),
            xml_attr(&included_layers.join(",")),
            xml_attr(layer),
        ), 1);
    let mut output = String::new();
    push_bounded(&mut output, declaration, max_bytes)?;
    push_bounded(&mut output, "\n", max_bytes)?;
    push_bounded(&mut output, &root, max_bytes)?;
    push_bounded(&mut output, "\n", max_bytes)?;
    push_bounded(
        &mut output,
        "<metadata id=\"pcb-enrichment-a0\" data-schema=\"kicad_monkey.pcb.svg.enrichment.a0\">\n",
        max_bytes,
    )?;
    push_xml_text_bounded(&mut output, metadata, max_bytes)?;
    push_bounded(&mut output, "\n</metadata>\n", max_bytes)?;
    push_bounded(
        &mut output,
        &background.replace("/>", " transform=\"scale(0.000001)\"/>"),
        max_bytes,
    )?;
    push_bounded(&mut output, "\n", max_bytes)?;
    for (_, group) in categorized {
        push_bounded(&mut output, &group, max_bytes)?;
    }
    push_bounded(&mut output, "</svg>\n", max_bytes)?;
    Ok(output)
}

fn top_level_groups<'a>(lines: &'a [&'a str]) -> Result<Vec<&'a [&'a str]>, DesignError> {
    let mut groups = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("<g ") {
            if depth == 0 {
                start = index;
            }
            depth += 1;
        }
        if *line == "</g>" {
            depth -= 1;
            if depth == 0 {
                groups.push(&lines[start..=index]);
            }
        }
        if depth < 0 {
            return Err(DesignError::new("PCB SVG group topology is invalid"));
        }
    }
    if depth != 0 {
        return Err(DesignError::new("PCB SVG group topology is unbalanced"));
    }
    Ok(groups)
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one deterministic mapping preserves the established record metadata vocabulary"
)]
fn record_attrs(record: &Value) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let kind = record["kind"].as_str().unwrap_or("record");
    attrs.insert(
        "data-primitive".to_owned(),
        match kind {
            "segment" => "track",
            "track_arc" => "arc",
            "zone_fill" => "zone",
            "footprint" => "footprint",
            "via" => "via",
            "gr_text" => "text",
            "gr_text_box" => "text-box",
            "dimension" => "dimension",
            "table" => "table",
            value if value.starts_with("gr_") => "graphic",
            value => value,
        }
        .to_owned(),
    );
    if let Some(uuid) = record["uuid"].as_str().filter(|value| !value.is_empty()) {
        attrs.insert("data-element-key".to_owned(), uuid.to_owned());
    }
    let layers = if kind == "footprint" {
        footprint_normal_layers(record)
    } else {
        ordered_layers(record)
    };
    set_layer_attrs(&mut attrs, &layers);
    for (source, target) in [
        ("net_id", "data-net-id"),
        ("net_id", "data-net-index"),
        ("net_name", "data-net"),
        ("net_class", "data-net-class"),
    ] {
        if let Some(value) = scalar_text(&record[source]) {
            attrs.insert(target.to_owned(), value);
        }
    }
    if let Some(classes) = record["net_classes"].as_array() {
        let value = classes
            .iter()
            .filter_map(scalar_text)
            .collect::<Vec<_>>()
            .join(",");
        if !value.is_empty() {
            attrs.insert("data-net-classes".to_owned(), value);
        }
    }
    if kind == "footprint" {
        for (source, target) in [
            ("reference", "data-component"),
            ("library_link", "data-footprint"),
        ] {
            if let Some(value) = scalar_text(&record[source]) {
                attrs.insert(target.to_owned(), value);
            }
        }
        if let Some(uuid) = record["uuid"].as_str().filter(|value| !value.is_empty()) {
            attrs.insert("data-component-uid".to_owned(), uuid.to_owned());
            attrs.insert("data-component-uuid".to_owned(), uuid.to_owned());
        }
    }
    if kind == "via" {
        let uuid = record["uuid"].as_str().unwrap_or_default();
        attrs.insert("data-hole-owner".to_owned(), uuid.to_owned());
        attrs.insert("data-hole-kind".to_owned(), hole_kind(record).to_owned());
        attrs.insert(
            "data-hole-plating".to_owned(),
            record["hole_plating"]
                .as_str()
                .unwrap_or("plated")
                .to_owned(),
        );
        attrs.insert(
            "data-via-type".to_owned(),
            record["via_type"].as_str().unwrap_or("through").to_owned(),
        );
        if let Some(drill) = scalar_text(&record["drill"]) {
            attrs.insert("data-hole-diameter-mm".to_owned(), drill.clone());
            attrs.insert("data-via-drill-mm".to_owned(), drill);
        }
        if let Some(size) = scalar_text(&record["size"]) {
            attrs.insert("data-via-size-mm".to_owned(), size);
        }
        for name in [
            "ipc4761_metadata",
            "ipc4761_tenting_front",
            "ipc4761_tenting_back",
            "ipc4761_covering_front",
            "ipc4761_covering_back",
            "ipc4761_plugging_front",
            "ipc4761_plugging_back",
            "ipc4761_capping",
            "ipc4761_filling",
        ] {
            if let Some(value) = scalar_text(&record[name]) {
                attrs.insert(format!("data-{}", name.replace('_', "-")), value);
            }
        }
    }
    attrs
}

fn ordered_layers(value: &Value) -> Vec<String> {
    let mut output = Vec::new();
    let mut add = |layer: &str| {
        if !layer.is_empty() && !output.iter().any(|value| value == layer) {
            output.push(layer.to_owned());
        }
    };
    if let Some(layer) = value["layer"].as_str() {
        add(layer);
    }
    for key in ["layers", "fill_layers"] {
        for layer in value[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            add(layer);
        }
    }
    if let Some(operations) = value["operations"].as_array() {
        for operation in operations {
            for layer in ordered_layers(operation) {
                add(&layer);
            }
        }
    }
    output
}

fn footprint_normal_layers(record: &Value) -> Vec<String> {
    let mut retained = record.clone();
    let Some(object) = retained.as_object_mut() else {
        return Vec::new();
    };
    let Some(operations) = object.get("operations").and_then(Value::as_array) else {
        return ordered_layers(record);
    };
    let mut normal = Vec::new();
    let mut index = 0;
    while index < operations.len() {
        if operations[index]["kind"].as_str() == Some("StartBlock")
            && index + 2 < operations.len()
            && operations[index + 2]["kind"].as_str() == Some("EndBlock")
        {
            if !matches!(
                operations[index + 1]["role"].as_str(),
                Some("pad_drill" | "npth_hole")
            ) {
                normal.extend(operations[index..index + 3].iter().cloned());
            }
            index += 3;
        } else {
            normal.push(operations[index].clone());
            index += 1;
        }
    }
    object.insert("operations".to_owned(), Value::Array(normal));
    ordered_layers(&retained)
}

fn set_layer_attrs(attrs: &mut BTreeMap<String, String>, layers: &[String]) {
    if layers.len() == 1 {
        attrs.insert("data-layer-name".to_owned(), layers[0].clone());
        attrs.insert(
            "data-layer-role".to_owned(),
            layer_role(&layers[0]).to_owned(),
        );
    } else if !layers.is_empty() {
        attrs.insert("data-layer-names".to_owned(), layers.join(","));
        attrs.insert(
            "data-layer-roles".to_owned(),
            layers
                .iter()
                .map(|value| layer_role(value))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(","),
        );
    }
}

fn split_via_group<'a>(
    record: &Value,
    group: &'a [&'a str],
) -> Option<(Vec<&'a str>, Vec<&'a str>)> {
    if record["kind"].as_str() != Some("via") {
        return None;
    }
    let operations = record["operations"].as_array()?;
    if group.len() != operations.len().checked_add(2)? {
        return None;
    }
    let mut normal = vec![group[0]];
    let mut drill = vec![group[0]];
    for (operation, line) in operations.iter().zip(&group[1..group.len() - 1]) {
        if matches!(
            operation["role"].as_str(),
            Some("via_drill" | "via_mask_drill" | "npth_hole")
        ) {
            drill.push(line);
        } else {
            normal.push(line);
        }
    }
    if drill.len() == 1 {
        return None;
    }
    normal.push(group[group.len() - 1]);
    drill.push(group[group.len() - 1]);
    Some((normal, drill))
}

fn split_footprint_holes<'a>(
    record: &Value,
    group: &'a [&'a str],
) -> Option<(Vec<&'a str>, Vec<&'a str>)> {
    if record["kind"].as_str() != Some("footprint") || group.len() < 3 {
        return None;
    }
    let mut normal = vec![group[0]];
    let mut drill = vec![group[0]];
    let mut cursor = 1;
    while cursor < group.len() - 1 {
        if !group[cursor].starts_with("<g ") {
            normal.push(group[cursor]);
            cursor += 1;
            continue;
        }
        let start = cursor;
        let mut depth = 0_i32;
        while cursor < group.len() - 1 {
            if group[cursor].starts_with("<g ") {
                depth += 1;
            } else if group[cursor] == "</g>" {
                depth -= 1;
            }
            cursor += 1;
            if depth == 0 {
                break;
            }
        }
        let target = if group[start].contains("data-hole-render=\"drill\"") {
            &mut drill
        } else {
            &mut normal
        };
        target.extend_from_slice(&group[start..cursor]);
    }
    if drill.len() == 1 {
        return None;
    }
    normal.push(group[group.len() - 1]);
    drill.push(group[group.len() - 1]);
    Some((normal, drill))
}

fn footprint_hole_attrs(record: &Value) -> BTreeMap<String, String> {
    let mut attrs = record_attrs(record);
    attrs.remove("data-layer-name");
    attrs.remove("data-layer-role");
    attrs.remove("data-layer-names");
    attrs.remove("data-layer-roles");
    set_layer_attrs(&mut attrs, &footprint_hole_layers(record));
    let uuid = record["uuid"].as_str().unwrap_or_default();
    attrs.insert("data-primitive".to_owned(), "hole".to_owned());
    attrs.insert("id".to_owned(), format!("{uuid}:drill_overlay"));
    attrs.insert("data-uuid".to_owned(), format!("{uuid}:drill_overlay"));
    attrs.insert("data-ref".to_owned(), "drill_overlay".to_owned());
    attrs.insert(
        "data-object-id".to_owned(),
        record["object_id"].as_str().unwrap_or_default().to_owned(),
    );
    attrs
}

fn footprint_hole_layers(record: &Value) -> Vec<String> {
    let mut layers = record["layer"]
        .as_str()
        .filter(|layer| !layer.is_empty())
        .map_or_else(Vec::new, |layer| vec![layer.to_owned()]);
    let Some(operations) = record["operations"].as_array() else {
        return layers;
    };
    let mut index = 0;
    while index + 2 < operations.len() {
        if operations[index]["kind"].as_str() == Some("StartBlock")
            && operations[index + 2]["kind"].as_str() == Some("EndBlock")
            && matches!(
                operations[index + 1]["role"].as_str(),
                Some("pad_drill" | "npth_hole")
            )
        {
            for layer in ordered_layers(&operations[index])
                .into_iter()
                .chain(ordered_layers(&operations[index + 1]))
            {
                if !layers.contains(&layer) {
                    layers.push(layer);
                }
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    layers
}

fn via_hole_attrs(record: &Value) -> BTreeMap<String, String> {
    let mut attrs = record_attrs(record);
    let uuid = record["uuid"].as_str().unwrap_or_default();
    attrs.insert("data-primitive".to_owned(), "via-hole".to_owned());
    attrs.insert("id".to_owned(), format!("{uuid}:drill_overlay"));
    attrs.insert("data-uuid".to_owned(), format!("{uuid}:drill_overlay"));
    attrs.insert("data-ref".to_owned(), "drill_overlay".to_owned());
    attrs.insert("data-object-id".to_owned(), "via".to_owned());
    attrs.insert("data-hole-owner".to_owned(), uuid.to_owned());
    attrs.insert("data-hole-kind".to_owned(), hole_kind(record).to_owned());
    attrs.insert(
        "data-hole-plating".to_owned(),
        record["hole_plating"]
            .as_str()
            .unwrap_or("plated")
            .to_owned(),
    );
    attrs.insert(
        "data-via-type".to_owned(),
        record["via_type"].as_str().unwrap_or("through").to_owned(),
    );
    if let Some(drill) = scalar_text(&record["drill"]) {
        attrs.insert("data-hole-diameter-mm".to_owned(), drill.clone());
        attrs.insert("data-via-drill-mm".to_owned(), drill);
    }
    if let Some(size) = scalar_text(&record["size"]) {
        attrs.insert("data-via-size-mm".to_owned(), size);
    }
    attrs.insert("data-hole-render".to_owned(), "drill".to_owned());
    for name in [
        "ipc4761_metadata",
        "ipc4761_tenting_front",
        "ipc4761_tenting_back",
        "ipc4761_covering_front",
        "ipc4761_covering_back",
        "ipc4761_plugging_front",
        "ipc4761_plugging_back",
        "ipc4761_capping",
        "ipc4761_filling",
    ] {
        if let Some(value) = scalar_text(&record[name]) {
            attrs.insert(format!("data-{}", name.replace('_', "-")), value);
        }
    }
    attrs
}

fn hole_kind(record: &Value) -> &str {
    record["operations"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|operation| {
            matches!(
                operation["role"].as_str(),
                Some("via_drill" | "via_mask_drill" | "npth_hole")
            )
            .then(|| {
                if operation["kind"].as_str() == Some("ThickSegment") {
                    "slot"
                } else {
                    "round"
                }
            })
        })
        .unwrap_or("round")
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn style_group(
    lines: &[&str],
    attrs: &BTreeMap<String, String>,
    category: &str,
    bounds: Bounds,
    max_bytes: usize,
) -> Result<String, DesignError> {
    let mut output = String::new();
    let mut categories = vec![category.to_owned()];
    let mut hole_colors = vec![(category == "drill").then(|| hole_color(attrs))];
    for (index, line) in lines.iter().enumerate() {
        let raw_next = output
            .len()
            .checked_add(line.len())
            .and_then(|value| value.checked_add(1))
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| DesignError::new("PCB review SVG exceeds its limit"))?;
        debug_assert!(raw_next <= max_bytes);
        let mut line = (*line).to_owned();
        if index == 0 {
            if attrs.contains_key("id") {
                line = attribute_value(&line, "transform").map_or_else(
                    || "<g>".to_owned(),
                    |transform| format!("<g transform=\"{}\">", xml_attr(transform)),
                );
            }
            let fragment = attrs
                .iter()
                .map(|(name, value)| format!(" {name}=\"{}\"", xml_attr(value)))
                .collect::<String>();
            line = line.replacen('>', &format!("{fragment}>"), 1);
            line = prepend_transform(
                &line,
                &format!(
                    "scale(0.000001) translate({} {})",
                    bounds.min_x.saturating_neg(),
                    bounds.min_y.saturating_neg()
                ),
            )?;
        }
        if line.starts_with("<g ") && index != 0 {
            let nested = line_category(&line)
                .unwrap_or_else(|| categories.last().cloned().unwrap_or_default());
            let nested_hole_color = if nested == "drill" {
                Some(category_color("drill", &line))
            } else {
                hole_colors.last().copied().flatten()
            };
            categories.push(nested);
            hole_colors.push(nested_hole_color);
        }
        let current = categories.last().map(String::as_str).unwrap_or(category);
        if !line.starts_with("<g ") && line != "</g>" {
            let color = if current == "drill" {
                hole_colors
                    .last()
                    .copied()
                    .flatten()
                    .unwrap_or(UNKNOWN_HOLE_COLOR)
            } else {
                category_color(current, &line)
            };
            line = recolor(&line, color);
        }
        push_bounded(&mut output, &line, max_bytes)?;
        push_bounded(&mut output, "\n", max_bytes)?;
        if line == "</g>" && categories.len() > 1 {
            categories.pop();
            hole_colors.pop();
        }
    }
    Ok(output)
}

fn push_styled_group(
    categorized: &mut Vec<(usize, String)>,
    materialized_bytes: &mut usize,
    max_bytes: usize,
    lines: &[&str],
    attrs: &BTreeMap<String, String>,
    category: &str,
    bounds: Bounds,
) -> Result<(), DesignError> {
    let remaining = max_bytes
        .checked_sub(*materialized_bytes)
        .ok_or_else(|| DesignError::new("PCB review SVG exceeds its limit"))?;
    let group = style_group(lines, attrs, category, bounds, remaining)?;
    *materialized_bytes = materialized_bytes
        .checked_add(group.len())
        .filter(|value| *value <= max_bytes)
        .ok_or_else(|| DesignError::new("PCB review SVG exceeds its limit"))?;
    categorized.push((usize::from(category_order(category)), group));
    Ok(())
}

fn attribute_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = line.find(&marker)? + marker.len();
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

fn hole_color(attrs: &BTreeMap<String, String>) -> &'static str {
    match (
        attrs.get("data-hole-plating").map(String::as_str),
        attrs.get("data-hole-kind").map(String::as_str),
    ) {
        (Some("non_plated"), Some("slot")) => NPTH_SLOT_COLOR,
        (Some("non_plated"), _) => NPTH_DRILL_COLOR,
        (Some("plated"), Some("slot")) => PTH_SLOT_COLOR,
        (Some("plated"), _) => PTH_DRILL_COLOR,
        _ => UNKNOWN_HOLE_COLOR,
    }
}

fn prepend_transform(line: &str, prefix: &str) -> Result<String, DesignError> {
    let marker = " transform=\"";
    if let Some(start) = line.find(marker) {
        let value_start = start + marker.len();
        let end = line[value_start..]
            .find('"')
            .map(|offset| value_start + offset)
            .ok_or_else(|| DesignError::new("PCB SVG record transform is malformed"))?;
        let mut output = line.to_owned();
        output.insert_str(value_start, &format!("{prefix} "));
        debug_assert!(end <= output.len());
        Ok(output)
    } else {
        Ok(line.replacen('>', &format!(" transform=\"{prefix}\">"), 1))
    }
}

fn line_category(line: &str) -> Option<String> {
    if line.contains("data-hole-render=\"drill\"")
        || line.contains("data-primitive=\"pad-hole\"")
        || line.contains("data-primitive=\"via-hole\"")
    {
        Some("drill".to_owned())
    } else if line.contains("data-layer-name=\"Edge.Cuts\"")
        || line.contains("data-layer-role=\"board-outline\"")
    {
        Some("edge".to_owned())
    } else if line.contains("data-primitive=\"pad\"") {
        Some("pad".to_owned())
    } else {
        None
    }
}

fn review_category(attrs: &BTreeMap<String, String>) -> &'static str {
    if attrs
        .get("data-layer-name")
        .is_some_and(|value| value == "Edge.Cuts")
        || attrs
            .get("data-layer-names")
            .is_some_and(|value| value.split(',').any(|layer| layer == "Edge.Cuts"))
    {
        "edge"
    } else {
        match attrs.get("data-primitive").map(String::as_str) {
            Some("track" | "arc" | "via") => "track",
            Some("zone") => "zone",
            Some("footprint" | "pad") => "pad",
            _ => "other",
        }
    }
}

fn category_order(category: &str) -> u8 {
    match category {
        "track" => 10,
        "zone" => 20,
        "edge" => 30,
        "pad" => 40,
        "drill" => 50,
        _ => 25,
    }
}

fn category_color(category: &str, line: &str) -> &'static str {
    match category {
        "track" | "zone" => TRACE_COLOR,
        "edge" => EDGE_COLOR,
        "pad" => PAD_COLOR,
        "drill"
            if line.contains("data-hole-plating=\"non_plated\"")
                && line.contains("data-hole-kind=\"slot\"") =>
        {
            NPTH_SLOT_COLOR
        }
        "drill" if line.contains("data-hole-plating=\"non_plated\"") => NPTH_DRILL_COLOR,
        "drill"
            if line.contains("data-hole-plating=\"plated\"")
                && line.contains("data-hole-kind=\"slot\"") =>
        {
            PTH_SLOT_COLOR
        }
        "drill" if line.contains("data-hole-plating=\"plated\"") => PTH_DRILL_COLOR,
        "drill" => UNKNOWN_HOLE_COLOR,
        _ => "",
    }
}

fn recolor(line: &str, color: &str) -> String {
    if color.is_empty() {
        return line.to_owned();
    }
    let mut output = line.to_owned();
    for name in ["fill", "stroke"] {
        let marker = format!("{name}=\"");
        let mut offset = 0;
        while let Some(found) = output[offset..].find(&marker) {
            let start = offset + found + marker.len();
            let Some(end) = output[start..].find('"').map(|value| start + value) else {
                break;
            };
            if &output[start..end] != "none" {
                output.replace_range(start..end, color);
            }
            offset = end + 1;
        }
    }
    output
}

fn nm_text(value: u64) -> String {
    let whole = value / 1_000_000;
    let fraction = value % 1_000_000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:06}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn push_xml_text_bounded(
    output: &mut String,
    value: &str,
    limit: usize,
) -> Result<(), DesignError> {
    let mut start = 0;
    for (index, character) in value.char_indices() {
        let escaped = match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            _ => continue,
        };
        push_bounded(output, &value[start..index], limit)?;
        push_bounded(output, escaped, limit)?;
        start = index + character.len_utf8();
    }
    push_bounded(output, &value[start..], limit)
}

fn xml_text_escaped_len(value: &str) -> Result<usize, DesignError> {
    value.chars().try_fold(0_usize, |total, character| {
        let bytes = match character {
            '&' => 5,
            '<' | '>' => 4,
            _ => character.len_utf8(),
        };
        total
            .checked_add(bytes)
            .ok_or_else(|| DesignError::new("PCB metadata XML size overflowed"))
    })
}

struct LimitedVecWriter {
    bytes: Vec<u8>,
    limit: usize,
}

struct CountWriter {
    written: usize,
    maximum: usize,
}

impl Write for CountWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.written = self
            .written
            .checked_add(buffer.len())
            .filter(|written| *written <= self.maximum)
            .ok_or_else(|| io::Error::other("PCB serialized size overflowed"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl LimitedVecWriter {
    const fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn into_string(self) -> Result<String, DesignError> {
        String::from_utf8(self.bytes)
            .map_err(|error| DesignError::context("PCB enrichment is not UTF-8", error))
    }
}

impl Write for LimitedVecWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes
            .len()
            .checked_add(buffer.len())
            .filter(|value| *value <= self.limit)
            .ok_or_else(|| io::Error::other("PCB enrichment metadata exceeds its limit"))?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn push_bounded(output: &mut String, value: &str, limit: usize) -> Result<(), DesignError> {
    let next = output
        .len()
        .checked_add(value.len())
        .ok_or_else(|| DesignError::new("PCB review SVG byte count overflowed"))?;
    if next > limit {
        return Err(DesignError::new("PCB review SVG exceeds its limit"));
    }
    output.push_str(value);
    Ok(())
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], DesignError> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| DesignError::new(format!("board plot {key} is not an array")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprint_hole_layers_preserve_authored_or_bound_drill_scope() {
        let mut record = json!({
            "layer": "F.Cu",
            "operations": [
                {"kind": "StartBlock", "layers": ["*.Cu", "*.Mask"]},
                {"kind": "Circle", "role": "npth_hole", "layers": ["*.Cu", "*.Mask"]},
                {"kind": "EndBlock"}
            ]
        });
        assert_eq!(footprint_hole_layers(&record), ["F.Cu", "*.Cu", "*.Mask"]);
        record["operations"][0]["layers"] = json!(["F.Cu", "Edge.Cuts"]);
        record["operations"][1]["layers"] = json!(["F.Cu", "Edge.Cuts"]);
        assert_eq!(footprint_hole_layers(&record), ["F.Cu", "Edge.Cuts"]);
    }

    #[test]
    fn filter_ceilings_accept_exact_and_reject_one_under() {
        let document = json!({
            "schema": "kicad.plotter_ir.a0",
            "records": [{
                "kind": "segment",
                "layer": "F.Cu",
                "operation_count": 1,
                "operations": [{"kind": "ThickSegment", "layer": null}]
            }],
            "total_operations": 1
        });
        let included = ["F.Cu".to_owned(), "Edge.Cuts".to_owned()];
        let exact_counts = PcbReviewSvgLimits {
            max_records_per_layer: 1,
            max_operations_per_layer: 1,
            ..PcbReviewSvgLimits::default()
        };
        let mut measured = FilterWork::new(usize::MAX);
        filter_document(&document, &included, exact_counts, &mut measured)
            .expect("measure bounded filtering work");
        let exact = PcbReviewSvgLimits {
            max_total_filter_work: measured.used,
            ..exact_counts
        };
        filter_document(
            &document,
            &included,
            exact,
            &mut FilterWork::new(measured.used),
        )
        .expect("inclusive filter ceilings");
        for limits in [
            PcbReviewSvgLimits {
                max_records_per_layer: 0,
                ..exact
            },
            PcbReviewSvgLimits {
                max_operations_per_layer: 0,
                ..exact
            },
            PcbReviewSvgLimits {
                max_total_filter_work: measured.used - 1,
                ..exact
            },
        ] {
            assert!(
                filter_document(
                    &document,
                    &included,
                    limits,
                    &mut FilterWork::new(limits.max_total_filter_work)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn metadata_and_composition_ceilings_are_inclusive() {
        let item_limits = PcbReviewSvgLimits {
            max_metadata_items: 1,
            ..PcbReviewSvgLimits::default()
        };
        assert_eq!(checked_metadata_item(0, item_limits).unwrap(), 1);
        assert!(checked_metadata_item(1, item_limits).is_err());
        let materialized = metadata_materialized_upper(100, 50, 10, 2).unwrap();
        enforce_metadata_materialized_limit(materialized, materialized)
            .expect("inclusive metadata materialization boundary");
        assert!(enforce_metadata_materialized_limit(materialized, materialized - 1).is_err());

        let mut exact_writer = LimitedVecWriter::new(3);
        exact_writer
            .write_all(b"abc")
            .expect("exact metadata bytes");
        let mut under_writer = LimitedVecWriter::new(2);
        assert!(under_writer.write_all(b"abc").is_err());

        let exact_limits = PcbReviewSvgLimits {
            max_total_materialized_bytes: 10,
            max_total_composition_work: 20,
            ..PcbReviewSvgLimits::default()
        };
        let mut budget = CompositionBudget::new(exact_limits);
        budget.reserve(10, 20).expect("inclusive aggregate budget");
        assert!(budget.reserve(1, 0).is_err());
        let mut work_under = CompositionBudget::new(PcbReviewSvgLimits {
            max_total_materialized_bytes: 10,
            max_total_composition_work: 19,
            ..exact_limits
        });
        assert!(work_under.reserve(10, 20).is_err());
        let mut temporary = CompositionBudget::new(exact_limits);
        temporary
            .begin_temporary(10, 20)
            .expect("inclusive peak temporary budget");
        assert_eq!(temporary.remaining_materialized(), 0);
        temporary.end_temporary(10).expect("temporary release");
        assert_eq!(temporary.remaining_materialized(), 10);
        let mut temporary_under = CompositionBudget::new(PcbReviewSvgLimits {
            max_total_materialized_bytes: 9,
            ..exact_limits
        });
        assert!(temporary_under.begin_temporary(10, 0).is_err());

        let short_strings = json!({"layers": vec!["x"; 1024]});
        let serialized = serialized_size(&short_strings, usize::MAX).expect("serialized size");
        let (materialized, traversal_work) =
            value_projection_usage(&short_strings, usize::MAX).expect("Value usage");
        assert!(materialized > serialized * 4);
        assert_eq!(
            value_projection_usage(&short_strings, traversal_work)
                .expect("inclusive recursive work")
                .0,
            materialized
        );
        assert!(value_projection_usage(&short_strings, traversal_work - 1).is_err());
        assert_eq!(
            serialized_size(&short_strings, serialized).expect("inclusive serialization work"),
            serialized
        );
        assert!(serialized_size(&short_strings, serialized - 1).is_err());
    }
}

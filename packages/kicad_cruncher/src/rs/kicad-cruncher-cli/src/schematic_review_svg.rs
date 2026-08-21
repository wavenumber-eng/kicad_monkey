//! Cruncher-owned enrichment of presentation-neutral Monkey schematic SVGs.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

use kicad_monkey_contracts::generated::compiled_schematic_graph::{
    CompiledSchematicGraphA0, GraphicalArtifactLink, PageOccurrence,
};
use kicad_monkey_core::{KiCadSchematicInstance, validate_compiled_schematic_graph};
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::design::{DesignError, SchematicBaseSvg, SchematicPlotDocument};

const ENRICHMENT_SCHEMA: &str = "kicad_monkey.schematic.svg.enrichment.a0";
const ENRICHMENT_METADATA_ID: &str = "schematic-enrichment-a0";
const GRAPH_VIEW_SCHEMA: &str = "kicad_monkey.schematic.svg.compiled_graph_view.a0";
const GRAPH_LINKAGE_CONTRACT: &str = "kicad_monkey.schematic.svg.compiled_graph_linkage.a0";
const GRAPH_ARTIFACT_KEY: &str = "sch.dwg_scene";
const REVIEW_THEME: &str = "kicad_cruncher.design_review.schematic_svg.a0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicReviewSvgLimits {
    pub max_documents: usize,
    pub max_total_output_bytes: usize,
    pub max_output_bytes_per_document: usize,
    pub max_graph_links_per_document: usize,
    pub max_graph_index_items: usize,
    pub max_graph_index_materialized_bytes: usize,
    pub max_graph_artifact_bytes: usize,
    pub max_graph_view_materialized_bytes: usize,
    pub max_graph_view_serialized_bytes: usize,
    pub max_record_attributes_per_document: usize,
    pub max_record_attribute_bytes_per_document: usize,
    pub max_svg_selector_ids_per_document: usize,
    pub max_svg_selector_bytes_per_document: usize,
    pub max_view_index_items_per_document: usize,
    pub max_view_index_materialized_bytes_per_document: usize,
    pub max_view_index_serialized_bytes_per_document: usize,
    pub max_view_authority_items: usize,
    pub max_view_authority_materialized_bytes: usize,
    pub max_total_view_index_work: usize,
    pub max_cached_design_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SchematicReviewSvgBuildProfile {
    pub validate_and_bind_ns: u64,
    pub graph_index_ns: u64,
    pub view_authority_ns: u64,
    pub document_alignment_ns: u64,
    pub graph_page_view_ns: u64,
    pub record_attributes_ns: u64,
    pub selector_index_and_validation_ns: u64,
    pub view_indexes_ns: u64,
    pub composition_root_ns: u64,
    pub metadata_serialization_ns: u64,
    pub base_svg_transform_ns: u64,
    pub output_finish_ns: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SvgCompositionProfile {
    root_ns: u64,
    metadata_serialization_ns: u64,
    base_svg_transform_ns: u64,
    output_finish_ns: u64,
}

impl Default for SchematicReviewSvgLimits {
    fn default() -> Self {
        Self {
            max_documents: 4_096,
            max_total_output_bytes: 1024 * 1024 * 1024,
            max_output_bytes_per_document: 768 * 1024 * 1024,
            max_graph_links_per_document: 4_000_000,
            max_graph_index_items: 64_000_000,
            max_graph_index_materialized_bytes: 1024 * 1024 * 1024,
            max_graph_artifact_bytes: 16 * 1024,
            max_graph_view_materialized_bytes: 512 * 1024 * 1024,
            max_graph_view_serialized_bytes: 512 * 1024 * 1024,
            max_record_attributes_per_document: 16_000_000,
            max_record_attribute_bytes_per_document: 256 * 1024 * 1024,
            max_svg_selector_ids_per_document: 8_000_000,
            max_svg_selector_bytes_per_document: 256 * 1024 * 1024,
            max_view_index_items_per_document: 16_000_000,
            max_view_index_materialized_bytes_per_document: 512 * 1024 * 1024,
            max_view_index_serialized_bytes_per_document: 512 * 1024 * 1024,
            max_view_authority_items: 16_000_000,
            max_view_authority_materialized_bytes: 512 * 1024 * 1024,
            max_total_view_index_work: 64_000_000,
            max_cached_design_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct SchematicReviewSvg {
    pub document_id: String,
    pub svg: String,
    pub page_occurrence_ref: String,
    pub artifact_key: &'static str,
    pub graph_link_count: usize,
    pub resolved_svg_identity_count: usize,
}

pub fn build_schematic_review_svgs(
    documents: &[SchematicPlotDocument],
    base_svgs: &[SchematicBaseSvg],
    graph: &CompiledSchematicGraphA0,
    design: &Value,
    graph_artifact: &str,
) -> Result<Vec<SchematicReviewSvg>, DesignError> {
    build_schematic_review_svgs_with_limits(
        documents,
        base_svgs,
        graph,
        design,
        graph_artifact,
        SchematicReviewSvgLimits::default(),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the bounded variant exposes every independently owned review input"
)]
pub fn build_schematic_review_svgs_with_limits(
    documents: &[SchematicPlotDocument],
    base_svgs: &[SchematicBaseSvg],
    graph: &CompiledSchematicGraphA0,
    design: &Value,
    graph_artifact: &str,
    limits: SchematicReviewSvgLimits,
) -> Result<Vec<SchematicReviewSvg>, DesignError> {
    build_schematic_review_svgs_internal(
        documents,
        base_svgs,
        graph,
        design,
        graph_artifact,
        limits,
        false,
    )
    .map(|(reviews, _profile)| reviews)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the profiled path mirrors the bounded review input surface"
)]
pub(crate) fn build_schematic_review_svgs_profiled(
    documents: &[SchematicPlotDocument],
    base_svgs: &[SchematicBaseSvg],
    graph: &CompiledSchematicGraphA0,
    design: &Value,
    graph_artifact: &str,
    limits: SchematicReviewSvgLimits,
) -> Result<(Vec<SchematicReviewSvg>, SchematicReviewSvgBuildProfile), DesignError> {
    build_schematic_review_svgs_internal(
        documents,
        base_svgs,
        graph,
        design,
        graph_artifact,
        limits,
        true,
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one ordered pass keeps bounded review derivation and its opt-in timings aligned"
)]
fn build_schematic_review_svgs_internal(
    documents: &[SchematicPlotDocument],
    base_svgs: &[SchematicBaseSvg],
    graph: &CompiledSchematicGraphA0,
    design: &Value,
    graph_artifact: &str,
    limits: SchematicReviewSvgLimits,
    profile_enabled: bool,
) -> Result<(Vec<SchematicReviewSvg>, SchematicReviewSvgBuildProfile), DesignError> {
    let mut profile = SchematicReviewSvgBuildProfile::default();
    let validation_started = profile_enabled.then(std::time::Instant::now);
    validate_compiled_schematic_graph(graph)
        .map_err(|error| DesignError::context("invalid compiled schematic graph", error))?;
    validate_graph_design_binding(graph, design)?;
    profile.validate_and_bind_ns = profile_elapsed_ns(validation_started);
    let graph_index_started = profile_enabled.then(std::time::Instant::now);
    let graph_index = GraphIndex::build(graph, limits)?;
    profile.graph_index_ns = profile_elapsed_ns(graph_index_started);
    let authority_started = profile_enabled.then(std::time::Instant::now);
    let view_authority = ViewIndexAuthority::build(design, limits)?;
    profile.view_authority_ns = profile_elapsed_ns(authority_started);
    let count = documents.len();
    if base_svgs.len() != count {
        return Err(DesignError::new(
            "schematic review SVG inputs must have identical document counts",
        ));
    }
    if count > limits.max_documents {
        return Err(DesignError::new(format!(
            "schematic review SVG document count exceeds its limit: {count} > {}",
            limits.max_documents
        )));
    }

    let mut output = Vec::with_capacity(count);
    let mut total_bytes = 0_usize;
    let mut total_view_work = 0_usize;
    let metadata_cache_started = profile_enabled.then(std::time::Instant::now);
    let cached_design = nested_pretty_raw_value(design, limits.max_cached_design_bytes)?;
    profile.metadata_serialization_ns = profile_elapsed_ns(metadata_cache_started);
    for (document, base) in documents.iter().zip(base_svgs) {
        let alignment_started = profile_enabled.then(std::time::Instant::now);
        let instance = &document.instance;
        let document = &document.value;
        if document["document_id"].as_str() != Some(&base.document_id)
            || instance.document_id != base.document_id
            || document["source_path"].as_str() != Some(&base.source_path)
            || !path_has_suffix(&base.source_path, &instance.source_path)
            || value_sha256(document)? != base.plot_document_sha256
        {
            return Err(DesignError::new(
                "schematic review SVG document identities are not aligned",
            ));
        }
        profile.document_alignment_ns = profile
            .document_alignment_ns
            .saturating_add(profile_elapsed_ns(alignment_started));
        let graph_view_started = profile_enabled.then(std::time::Instant::now);
        let graph_view = graph_page_view(graph, &graph_index, instance, graph_artifact, limits)?;
        profile.graph_page_view_ns = profile
            .graph_page_view_ns
            .saturating_add(profile_elapsed_ns(graph_view_started));
        let attributes_started = profile_enabled.then(std::time::Instant::now);
        let record_attrs = record_attribute_fragments(document, limits)?;
        profile.record_attributes_ns = profile
            .record_attributes_ns
            .saturating_add(profile_elapsed_ns(attributes_started));
        let selectors_started = profile_enabled.then(std::time::Instant::now);
        let selectors = svg_id_counts(&base.svg, limits)?;
        validate_record_selectors(&selectors, record_attrs.keys())?;
        validate_graph_selectors(&selectors, &graph_view)?;
        profile.selector_index_and_validation_ns = profile
            .selector_index_and_validation_ns
            .saturating_add(profile_elapsed_ns(selectors_started));
        let remaining_view_work = limits
            .max_total_view_index_work
            .checked_sub(total_view_work)
            .ok_or_else(|| DesignError::new("schematic view index aggregate work exceeded"))?;
        let view_indexes_started = profile_enabled.then(std::time::Instant::now);
        let (view_indexes, view_work) =
            schematic_view_indexes(&view_authority, instance, limits, remaining_view_work)?;
        profile.view_indexes_ns = profile
            .view_indexes_ns
            .saturating_add(profile_elapsed_ns(view_indexes_started));
        total_view_work = total_view_work
            .checked_add(view_work)
            .ok_or_else(|| DesignError::new("schematic view index work overflowed"))?;
        if total_view_work > limits.max_total_view_index_work {
            return Err(DesignError::new(
                "schematic view index aggregate work limit exceeded",
            ));
        }
        let remaining = limits
            .max_total_output_bytes
            .checked_sub(total_bytes)
            .ok_or_else(|| DesignError::new("schematic review SVG aggregate limit exceeded"))?;
        let output_limit = remaining.min(limits.max_output_bytes_per_document);
        let (svg, composition_profile) = enrich_svg(
            &base.svg,
            &record_attrs,
            &cached_design,
            instance,
            &base.source_path,
            &view_indexes,
            &graph_view.value,
            output_limit,
            profile_enabled,
        )?;
        profile.composition_root_ns = profile
            .composition_root_ns
            .saturating_add(composition_profile.root_ns);
        profile.metadata_serialization_ns = profile
            .metadata_serialization_ns
            .saturating_add(composition_profile.metadata_serialization_ns);
        profile.base_svg_transform_ns = profile
            .base_svg_transform_ns
            .saturating_add(composition_profile.base_svg_transform_ns);
        profile.output_finish_ns = profile
            .output_finish_ns
            .saturating_add(composition_profile.output_finish_ns);
        total_bytes = total_bytes
            .checked_add(svg.len())
            .ok_or_else(|| DesignError::new("schematic review SVG byte count overflowed"))?;
        output.push(SchematicReviewSvg {
            document_id: base.document_id.clone(),
            svg,
            page_occurrence_ref: graph_view.page_ref,
            artifact_key: GRAPH_ARTIFACT_KEY,
            graph_link_count: graph_view.link_count,
            resolved_svg_identity_count: graph_view.element_count,
        });
    }
    Ok((output, profile))
}

fn profile_elapsed_ns(started: Option<std::time::Instant>) -> u64 {
    started.map_or(0, |started| {
        u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
    })
}

fn path_has_suffix(path: &str, suffix: &str) -> bool {
    let path = path.replace('\\', "/");
    let suffix = suffix.replace('\\', "/");
    path == suffix || path.ends_with(&format!("/{suffix}"))
}

fn value_sha256(value: &Value) -> Result<String, DesignError> {
    let mut writer = HashWriter::default();
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| DesignError::context("could not fingerprint plot document", error))?;
    Ok(writer.finish_hex())
}

struct GraphView {
    value: Value,
    page_ref: String,
    link_count: usize,
    element_count: usize,
    element_ids: Vec<String>,
}

struct GraphIndex<'a> {
    page_by_source_record: BTreeMap<&'a str, &'a PageOccurrence>,
    links_by_page: BTreeMap<&'a str, Vec<&'a GraphicalArtifactLink>>,
}

impl<'a> GraphIndex<'a> {
    fn build(
        graph: &'a CompiledSchematicGraphA0,
        limits: SchematicReviewSvgLimits,
    ) -> Result<Self, DesignError> {
        let item_count = graph
            .page_occurrences
            .len()
            .checked_add(graph.graphical_artifact_links.len())
            .ok_or_else(|| DesignError::new("schematic graph index item count overflowed"))?;
        if item_count > limits.max_graph_index_items {
            return Err(DesignError::new(
                "schematic graph index item limit exceeded",
            ));
        }
        let bytes = graph
            .page_occurrences
            .iter()
            .try_fold(0_usize, |total, page| {
                total.checked_add(
                    page.source_identity
                        .sch_source_key_source_record
                        .as_deref()
                        .map_or(0, str::len)
                        .saturating_add(page.id.len())
                        .saturating_add(96),
                )
            })
            .and_then(|total| {
                graph
                    .graphical_artifact_links
                    .iter()
                    .try_fold(total, |total, link| {
                        total.checked_add(
                            link.page_occurrence_ref
                                .len()
                                .saturating_add(link.id.len())
                                .saturating_add(64),
                        )
                    })
            });
        if bytes.is_none_or(|bytes| bytes > limits.max_graph_index_materialized_bytes) {
            return Err(DesignError::new(
                "schematic graph index byte limit exceeded",
            ));
        }
        let mut page_by_source_record = BTreeMap::new();
        for page in &graph.page_occurrences {
            if let Some(record) = page.source_identity.sch_source_key_source_record.as_deref()
                && page_by_source_record.insert(record, page).is_some()
            {
                return Err(DesignError::new(
                    "compiled graph has duplicate page source records",
                ));
            }
        }
        let mut links_by_page = BTreeMap::<&str, Vec<&GraphicalArtifactLink>>::new();
        for link in &graph.graphical_artifact_links {
            if link.artifact_key == GRAPH_ARTIFACT_KEY {
                links_by_page
                    .entry(&link.page_occurrence_ref)
                    .or_default()
                    .push(link);
            }
        }
        Ok(Self {
            page_by_source_record,
            links_by_page,
        })
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one bounded pass resolves, budgets, indexes, and serializes one graph page view"
)]
fn graph_page_view(
    graph: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
    instance: &KiCadSchematicInstance,
    graph_artifact: &str,
    limits: SchematicReviewSvgLimits,
) -> Result<GraphView, DesignError> {
    if graph_artifact.len() > limits.max_graph_artifact_bytes {
        return Err(DesignError::new(
            "schematic graph artifact path byte limit exceeded",
        ));
    }
    let source_record = format!("instance-path:{}", instance.sheet_instance_path);
    let page = index
        .page_by_source_record
        .get(source_record.as_str())
        .copied();
    if page.is_none_or(|value| value.id != instance.page_occurrence_ref) {
        return Err(DesignError::new(format!(
            "compiled graph page resolution requires one exact match for {source_record}"
        )));
    }
    let page_ref = page.expect("one page").id.clone();
    let matching_links = index
        .links_by_page
        .get(page_ref.as_str())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let link_count = matching_links.len();
    if link_count > limits.max_graph_links_per_document {
        return Err(DesignError::new(
            "schematic review graph link limit exceeded",
        ));
    }
    let header_bytes = graph_artifact
        .len()
        .checked_add(graph.schema.len())
        .and_then(|total| total.checked_add(graph.identity_namespace.len()))
        .and_then(|total| total.checked_add(page_ref.len()))
        .and_then(|total| total.checked_add(4_096));
    let graph_view_bytes = header_bytes.and_then(|header| {
        matching_links.iter().try_fold(header, |total, link| {
            let strings = link
                .id
                .len()
                .saturating_mul(2)
                .saturating_add(link.element_id.len().saturating_mul(3))
                .saturating_add(link.target_ref.len().saturating_mul(3))
                .saturating_add(link.target_type.to_string().len())
                .saturating_add(256);
            total.checked_add(strings)
        })
    });
    if graph_view_bytes.is_none_or(|bytes| bytes > limits.max_graph_view_materialized_bytes) {
        return Err(DesignError::new(
            "schematic review graph view byte limit exceeded",
        ));
    }
    let mut links = matching_links.to_vec();
    links.sort_by(|left, right| left.id.cmp(&right.id));
    let mut element_to_links = BTreeMap::<String, Vec<String>>::new();
    let mut target_to_elements = BTreeMap::<String, BTreeSet<String>>::new();
    let mut target_types = BTreeMap::<String, String>::new();
    for link in &links {
        element_to_links
            .entry(link.element_id.clone())
            .or_default()
            .push(link.id.clone());
        target_to_elements
            .entry(link.target_ref.clone())
            .or_default()
            .insert(link.element_id.clone());
        target_types.insert(link.target_ref.clone(), link.target_type.to_string());
    }
    let element_ids = element_to_links.keys().cloned().collect::<Vec<_>>();
    let value = json!({
        "schema": GRAPH_VIEW_SCHEMA,
        "graph_schema": graph.schema,
        "identity_namespace": graph.identity_namespace,
        "graph_artifact": graph_artifact,
        "linkage_contract": GRAPH_LINKAGE_CONTRACT,
        "page_occurrence_ref": page_ref,
        "artifact_key": GRAPH_ARTIFACT_KEY,
        "graphical_artifact_link_refs": links.iter().map(|link| &link.id).collect::<Vec<_>>(),
        "element_to_graphical_artifact_link_refs": element_to_links,
        "target_to_element_ids": target_to_elements,
        "target_type_by_ref": target_types,
    });
    let mut serialized = CountingWriter::new(limits.max_graph_view_serialized_bytes);
    serde_json::to_writer(&mut serialized, &value).map_err(|error| {
        DesignError::context("schematic graph view serialized limit exceeded", error)
    })?;
    Ok(GraphView {
        value,
        page_ref,
        link_count: links.len(),
        element_count: element_ids.len(),
        element_ids,
    })
}

fn validate_graph_selectors(
    selectors: &BTreeMap<String, usize>,
    graph_view: &GraphView,
) -> Result<(), DesignError> {
    for id in &graph_view.element_ids {
        let count = selectors.get(&xml_attribute(id)).copied().unwrap_or(0);
        if count != 1 {
            return Err(DesignError::new(format!(
                "compiled graph SVG selector {id:?} resolved {count} times"
            )));
        }
    }
    Ok(())
}

fn validate_record_selectors<'a>(
    selectors: &BTreeMap<String, usize>,
    ids: impl Iterator<Item = &'a String>,
) -> Result<(), DesignError> {
    for id in ids {
        let count = selectors.get(&xml_attribute(id)).copied().unwrap_or(0);
        if count != 1 {
            return Err(DesignError::new(format!(
                "schematic record SVG selector {id:?} resolved {count} times"
            )));
        }
    }
    Ok(())
}

fn svg_id_counts(
    svg: &str,
    limits: SchematicReviewSvgLimits,
) -> Result<BTreeMap<String, usize>, DesignError> {
    let mut output = BTreeMap::<String, usize>::new();
    let mut selector_bytes = 0_usize;
    let mut remaining = svg;
    while let Some(start) = remaining.find(" id=\"") {
        let value = &remaining[start + 5..];
        let end = value
            .find('"')
            .ok_or_else(|| DesignError::new("schematic SVG contains an unterminated ID"))?;
        let id = &value[..end];
        selector_bytes = selector_bytes
            .checked_add(id.len())
            .ok_or_else(|| DesignError::new("schematic SVG selector bytes overflowed"))?;
        if selector_bytes > limits.max_svg_selector_bytes_per_document {
            return Err(DesignError::new(
                "schematic SVG selector byte limit exceeded",
            ));
        }
        if !output.contains_key(id) && output.len() == limits.max_svg_selector_ids_per_document {
            return Err(DesignError::new(
                "schematic SVG selector count limit exceeded",
            ));
        }
        *output.entry(id.to_owned()).or_default() += 1;
        remaining = &value[end + 1..];
    }
    Ok(output)
}

fn validate_graph_design_binding(
    graph: &CompiledSchematicGraphA0,
    design: &Value,
) -> Result<(), DesignError> {
    let design_graph = design
        .get("compiled_schematic_graph")
        .ok_or_else(|| DesignError::new("design JSON compiled schematic graph is missing"))?;
    let mut typed_hash = HashWriter::default();
    serde_json::to_writer(&mut typed_hash, graph)
        .map_err(|error| DesignError::context("could not fingerprint typed graph", error))?;
    let mut design_hash = HashWriter::default();
    serde_json::to_writer(&mut design_hash, design_graph)
        .map_err(|error| DesignError::context("could not fingerprint design graph", error))?;
    if typed_hash.finish() != design_hash.finish() {
        return Err(DesignError::new(
            "design JSON and compiled schematic graph are not the same facts",
        ));
    }
    Ok(())
}

fn record_attribute_fragments(
    document: &Value,
    limits: SchematicReviewSvgLimits,
) -> Result<BTreeMap<String, String>, DesignError> {
    let records = document["records"]
        .as_array()
        .ok_or_else(|| DesignError::new("schematic plot records are missing"))?;
    let mut attribute_count = 0_usize;
    let mut attribute_bytes = 0_usize;
    let mut output = BTreeMap::new();
    for record in records {
        let id = text_field(record, "uuid")?;
        let kind = text_field(record, "kind")?;
        let mut attrs = BTreeMap::<String, String>::new();
        attrs.insert("data-primitive".to_owned(), record_primitive(record, kind));
        attrs.insert("data-source-kind".to_owned(), "schematic".to_owned());
        if !id.is_empty() {
            attrs.insert("data-element-key".to_owned(), id.to_owned());
        }
        add_record_layer_attrs(record, &mut attrs);
        add_record_specific_attrs(record, kind, &mut attrs);
        attribute_count = attribute_count
            .checked_add(attrs.len())
            .ok_or_else(|| DesignError::new("schematic record attribute count overflowed"))?;
        if attribute_count > limits.max_record_attributes_per_document {
            return Err(DesignError::new(
                "schematic record attribute limit exceeded",
            ));
        }
        let fragment_bytes = attrs.iter().try_fold(0_usize, |total, (name, value)| {
            total
                .checked_add(4)
                .and_then(|total| total.checked_add(name.len()))
                .and_then(|total| total.checked_add(xml_attribute_len(value)))
        });
        let next_attribute_bytes = fragment_bytes
            .and_then(|bytes| {
                attribute_bytes
                    .checked_add(id.len())
                    .and_then(|total| total.checked_add(bytes))
            })
            .ok_or_else(|| DesignError::new("schematic record attribute bytes overflowed"))?;
        if next_attribute_bytes > limits.max_record_attribute_bytes_per_document {
            return Err(DesignError::new(
                "schematic record attribute byte limit exceeded",
            ));
        }
        let mut fragment = String::with_capacity(fragment_bytes.expect("checked above"));
        for (name, value) in attrs {
            fragment.push(' ');
            fragment.push_str(&name);
            fragment.push_str("=\"");
            fragment.push_str(&xml_attribute(&value));
            fragment.push('"');
        }
        attribute_bytes = next_attribute_bytes;
        if output.insert(id.to_owned(), fragment).is_some() {
            return Err(DesignError::new(
                "schematic plot document contains duplicate record UUIDs",
            ));
        }
    }
    Ok(output)
}

fn add_record_layer_attrs(record: &Value, attrs: &mut BTreeMap<String, String>) {
    let mut layers = Vec::<String>::new();
    for layer in record["operations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|operation| operation["layer"].as_str())
        .filter(|layer| !layer.is_empty())
    {
        if !layers.iter().any(|known| known == layer) {
            layers.push(layer.to_owned());
        }
    }
    if layers.len() == 1 {
        attrs.insert(
            "data-layer-name".to_owned(),
            layers.into_iter().next().expect("one layer"),
        );
    } else if !layers.is_empty() {
        attrs.insert(
            "data-layer-names".to_owned(),
            layers.into_iter().collect::<Vec<_>>().join(","),
        );
    }
}

fn add_record_specific_attrs(record: &Value, kind: &str, attrs: &mut BTreeMap<String, String>) {
    match kind {
        "label" | "global_label" | "hierarchical_label" | "text" | "text_box" => {
            add_value_attr(record, "text", "data-text", attrs);
            add_value_attr(record, "shape", "data-shape", attrs);
        }
        "symbol_instance" | "symbol_overplot" => add_symbol_attrs(record, attrs),
        "sheet" => {
            add_value_attr(record, "sheet_name", "data-sheet-name", attrs);
            add_value_attr(record, "sheet_file", "data-sheet-file", attrs);
            for field in ["at_x_nm", "at_y_nm", "size_x_nm", "size_y_nm"] {
                add_value_attr(
                    record,
                    field,
                    &format!("data-{}", field.replace('_', "-")),
                    attrs,
                );
            }
        }
        "netclass_flag" => add_value_attr(record, "shape", "data-shape", attrs),
        _ => {}
    }
}

fn add_symbol_attrs(record: &Value, attrs: &mut BTreeMap<String, String>) {
    let reference = scalar_text(&record["reference"]);
    let id = scalar_text(&record["uuid"]);
    if !reference.is_empty() {
        attrs.insert("data-component".to_owned(), reference.clone());
        attrs.insert("data-designator".to_owned(), reference.clone());
    }
    if !id.is_empty() {
        attrs.insert("data-component-uid".to_owned(), id.clone());
        attrs.insert("data-component-uuid".to_owned(), id);
    }
    for (field, attribute) in [
        ("lib_id", "data-symbol-library-ref"),
        ("lib_name", "data-symbol-library-name"),
        ("unit", "data-symbol-unit"),
        ("convert", "data-symbol-convert"),
    ] {
        add_value_attr(record, field, attribute, attrs);
    }
    let power = reference.starts_with('#')
        || scalar_text(&record["lib_id"])
            .to_lowercase()
            .starts_with("power:");
    attrs.insert(
        "data-symbol-role".to_owned(),
        if power { "power" } else { "component" }.to_owned(),
    );
    for field in [
        "in_bom",
        "on_board",
        "dnp",
        "exclude_from_sim",
        "in_pos_files",
    ] {
        if let Some(value) = record.get(field).and_then(Value::as_bool) {
            attrs.insert(
                format!("data-{}", field.replace('_', "-")),
                value.to_string(),
            );
        }
    }
}

fn add_value_attr(
    record: &Value,
    field: &str,
    attribute: &str,
    attrs: &mut BTreeMap<String, String>,
) {
    let value = scalar_text(&record[field]);
    if !value.is_empty() {
        attrs.insert(attribute.to_owned(), value);
    }
}

fn record_primitive(record: &Value, kind: &str) -> String {
    match kind {
        "symbol_instance" => {
            let reference = scalar_text(&record["reference"]);
            let lib_id = scalar_text(&record["lib_id"]).to_lowercase();
            if reference.starts_with('#') || lib_id.starts_with("power:") {
                "power-symbol"
            } else {
                "symbol"
            }
            .to_owned()
        }
        "symbol_overplot" => "symbol-overplot".to_owned(),
        "sheet" => "sheet-symbol".to_owned(),
        "hierarchical_label" => "port".to_owned(),
        "global_label" => "global-label".to_owned(),
        "bus_entry" => "bus-entry".to_owned(),
        "no_connect" => "no-connect".to_owned(),
        "netclass_flag" => "netclass-flag".to_owned(),
        "sheet_header_background" => "drawing-sheet-background".to_owned(),
        "sheet_header" => "drawing-sheet".to_owned(),
        "text_box" => "text-box".to_owned(),
        value if value.starts_with("graphic_") => "graphic".to_owned(),
        _ => scalar_text(&record["primitive"])
            .chars()
            .collect::<String>()
            .trim()
            .to_owned()
            .pipe_or(kind),
    }
}

trait PipeOr {
    fn pipe_or(self, fallback: &str) -> String;
}

impl PipeOr for String {
    fn pipe_or(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_owned()
        } else {
            self
        }
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        _ => String::new(),
    }
}

fn text_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, DesignError> {
    value[field]
        .as_str()
        .ok_or_else(|| DesignError::new(format!("schematic record {field} is missing")))
}

fn nested_pretty_raw_value(value: &Value, limit: usize) -> Result<Box<RawValue>, DesignError> {
    let mut preflight = PrettyCachePreflight::new(limit);
    serde_json::to_writer_pretty(&mut preflight, value).map_err(|error| {
        DesignError::context("schematic design metadata cache exceeds its limit", error)
    })?;
    let indentation_bytes = preflight
        .newlines
        .checked_mul(2)
        .ok_or_else(|| DesignError::new("schematic metadata indentation count overflowed"))?;
    let nested_bytes = preflight
        .written
        .checked_add(indentation_bytes)
        .ok_or_else(|| DesignError::new("schematic metadata cache size overflowed"))?;
    preflight
        .written
        .checked_add(nested_bytes)
        .filter(|bytes| *bytes <= limit)
        .ok_or_else(|| DesignError::new("schematic design metadata cache exceeds its limit"))?;

    let mut writer = ExactBytesWriter::new(preflight.written);
    serde_json::to_writer_pretty(&mut writer, value).map_err(|error| {
        DesignError::context("schematic design metadata cache exceeds its limit", error)
    })?;
    let serialized = writer.finish()?;
    let mut nested = vec![0_u8; nested_bytes];
    let mut output = 0_usize;
    for byte in serialized.bytes() {
        nested[output] = byte;
        output += 1;
        if byte == b'\n' {
            nested[output..output + 2].copy_from_slice(b"  ");
            output += 2;
        }
    }
    debug_assert_eq!(output, nested_bytes);
    let nested = String::from_utf8(nested)
        .map_err(|error| DesignError::context("schematic metadata cache is not UTF-8", error))?;
    RawValue::from_string(nested)
        .map_err(|error| DesignError::context("schematic metadata cache is invalid", error))
}

#[derive(Serialize)]
struct EnrichmentPayload<'a> {
    schema: &'static str,
    source: SourceMetadata<'a>,
    view: ViewMetadata<'a>,
    view_indexes: &'a Value,
    design: &'a RawValue,
    compiled_schematic_graph_view: &'a Value,
}

#[derive(Serialize)]
struct SourceMetadata<'a> {
    kicad_sch_file: &'a str,
}

#[derive(Serialize)]
struct ViewMetadata<'a> {
    kind: &'static str,
    profile: &'static str,
    sheet_name: &'a str,
    sheet_path: &'a str,
    sheet_instance_path: &'a str,
}

#[allow(
    clippy::too_many_arguments,
    reason = "bounded XML streaming keeps borrowed review inputs separate without cloning"
)]
fn enrich_svg(
    base: &str,
    record_attrs: &BTreeMap<String, String>,
    design: &RawValue,
    instance: &KiCadSchematicInstance,
    source_path: &str,
    view_indexes: &Value,
    graph_view: &Value,
    limit: usize,
    profile_enabled: bool,
) -> Result<(String, SvgCompositionProfile), DesignError> {
    let mut profile = SvgCompositionProfile::default();
    let root_started = profile_enabled.then(std::time::Instant::now);
    let root_start = base
        .find("<svg ")
        .ok_or_else(|| DesignError::new("base SVG root is missing"))?;
    let root_end = base[root_start..]
        .find(">\n")
        .map(|offset| root_start + offset)
        .ok_or_else(|| DesignError::new("base SVG root is malformed"))?;
    let mut writer = LimitedBytes::new(limit);
    writer
        .write_all(&base.as_bytes()[..root_start])
        .map_err(output_error)?;
    write_transformed(&mut writer, &base[root_start..root_end], record_attrs)?;
    for (name, value) in root_attributes(instance, source_path, graph_view) {
        write!(&mut writer, " {name}=\"{}\"", xml_attribute(&value)).map_err(output_error)?;
    }
    writer.write_all(b">\n").map_err(output_error)?;
    writeln!(
        &mut writer,
        "<metadata id=\"{ENRICHMENT_METADATA_ID}\" data-schema=\"{ENRICHMENT_SCHEMA}\">"
    )
    .map_err(output_error)?;
    profile.root_ns = profile_elapsed_ns(root_started);
    let payload = EnrichmentPayload {
        schema: ENRICHMENT_SCHEMA,
        source: SourceMetadata {
            kicad_sch_file: source_path,
        },
        view: ViewMetadata {
            kind: "schematic_sheet",
            profile: "enriched",
            sheet_name: &instance.sheet_name,
            sheet_path: &instance.sheet_path,
            sheet_instance_path: &instance.sheet_instance_path,
        },
        view_indexes,
        design,
        compiled_schematic_graph_view: graph_view,
    };
    let metadata_started = profile_enabled.then(std::time::Instant::now);
    {
        let mut escaped = XmlTextWriter(&mut writer);
        serde_json::to_writer_pretty(&mut escaped, &payload)
            .map_err(|error| DesignError::context("schematic enrichment limit exceeded", error))?;
    }
    writer.write_all(b"\n</metadata>\n").map_err(output_error)?;
    profile.metadata_serialization_ns = profile_elapsed_ns(metadata_started);
    let transform_started = profile_enabled.then(std::time::Instant::now);
    write_transformed(&mut writer, &base[root_end + 2..], record_attrs)?;
    profile.base_svg_transform_ns = profile_elapsed_ns(transform_started);
    let finish_started = profile_enabled.then(std::time::Instant::now);
    let output = writer.finish()?;
    profile.output_finish_ns = profile_elapsed_ns(finish_started);
    Ok((output, profile))
}

fn root_attributes(
    instance: &KiCadSchematicInstance,
    source_path: &str,
    graph_view: &Value,
) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        (
            "data-compiled-graph-artifact-key",
            GRAPH_ARTIFACT_KEY.to_owned(),
        ),
        (
            "data-compiled-graph-linkage-contract",
            GRAPH_LINKAGE_CONTRACT.to_owned(),
        ),
        (
            "data-compiled-graph-page-occurrence-ref",
            scalar_text(&graph_view["page_occurrence_ref"]),
        ),
        (
            "data-compiled-graph-schema",
            scalar_text(&graph_view["graph_schema"]),
        ),
        (
            "data-compiled-graph-view-schema",
            GRAPH_VIEW_SCHEMA.to_owned(),
        ),
        ("data-enrichment-schema", ENRICHMENT_SCHEMA.to_owned()),
        ("data-profile", "enriched".to_owned()),
        ("data-review-theme", REVIEW_THEME.to_owned()),
        ("data-sheet-name", instance.sheet_name.clone()),
        ("data-sheet-path", instance.sheet_path.clone()),
        ("data-source", source_path.to_owned()),
        ("data-view-kind", "schematic_sheet".to_owned()),
    ])
}

struct ViewIndexAuthority<'a> {
    sheet_map: &'a Value,
    net_by_name: BTreeMap<&'a str, &'a Value>,
}

impl<'a> ViewIndexAuthority<'a> {
    fn build(design: &'a Value, limits: SchematicReviewSvgLimits) -> Result<Self, DesignError> {
        let nets = design["nets"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        if nets.len() > limits.max_view_authority_items {
            return Err(DesignError::new(
                "schematic view authority item limit exceeded",
            ));
        }
        let bytes = nets.iter().try_fold(0_usize, |total, net| {
            total.checked_add(
                net["name"]
                    .as_str()
                    .map_or(0, str::len)
                    .saturating_add(net["uid"].as_str().map_or(0, str::len))
                    .saturating_add(net["net_class"].as_str().map_or(0, str::len))
                    .saturating_add(64),
            )
        });
        if bytes.is_none_or(|bytes| bytes > limits.max_view_authority_materialized_bytes) {
            return Err(DesignError::new(
                "schematic view authority byte limit exceeded",
            ));
        }
        let mut net_by_name = BTreeMap::new();
        for net in nets {
            if let Some(name) = net["name"].as_str() {
                net_by_name.entry(name).or_insert(net);
            }
        }
        Ok(Self {
            sheet_map: &design["indexes"]["sheet_svg_to_nets"],
            net_by_name,
        })
    }
}

fn schematic_view_indexes(
    authority: &ViewIndexAuthority<'_>,
    instance: &KiCadSchematicInstance,
    limits: SchematicReviewSvgLimits,
    max_work: usize,
) -> Result<(Value, usize), DesignError> {
    let keys = sheet_lookup_keys(&instance.sheet_path, &instance.sheet_instance_path);
    let mut svg_names = BTreeMap::<String, BTreeSet<String>>::new();
    let mut budget = ViewIndexBudget::new(limits, max_work);
    for key in &keys {
        if let Some(rows) = authority.sheet_map.get(key).and_then(Value::as_object) {
            for (id, names) in rows {
                if let Some(name) = names.as_str() {
                    add_svg_net_name(&mut svg_names, id, name, &mut budget)?;
                } else if let Some(names) = names.as_array() {
                    for name in names.iter().filter_map(Value::as_str) {
                        add_svg_net_name(&mut svg_names, id, name, &mut budget)?;
                    }
                }
            }
        }
    }
    let mut svg_to_nets = BTreeMap::<String, Vec<Value>>::new();
    let mut svg_to_net = BTreeMap::<String, Value>::new();
    let mut net_to_svg = BTreeMap::<String, BTreeSet<String>>::new();
    let mut uid_to_svg = BTreeMap::<String, BTreeSet<String>>::new();
    for (id, names) in svg_names {
        for name in &names {
            let net = authority.net_by_name.get(name.as_str()).copied();
            let summary_bytes = name
                .len()
                .saturating_add(
                    net.and_then(|value| value["uid"].as_str())
                        .map_or(0, str::len),
                )
                .saturating_add(
                    net.and_then(|value| value["net_class"].as_str())
                        .map_or(0, str::len),
                )
                .saturating_add(id.len().saturating_mul(4))
                .saturating_add(256);
            budget.attempt(summary_bytes.saturating_add(4))?;
            budget.retain(4, summary_bytes)?;
        }
        let summaries = names
            .iter()
            .map(|name| net_summary(name, authority.net_by_name.get(name.as_str()).copied()))
            .collect::<Vec<_>>();
        if summaries.len() == 1 {
            svg_to_net.insert(id.clone(), summaries[0].clone());
        }
        for summary in &summaries {
            let name = scalar_text(&summary["name"]);
            let uid = scalar_text(&summary["uid"]);
            if !name.is_empty() {
                net_to_svg.entry(name).or_default().insert(id.clone());
            }
            if !uid.is_empty() {
                uid_to_svg.entry(uid).or_default().insert(id.clone());
            }
        }
        svg_to_nets.insert(id, summaries);
    }
    let value = json!({ "sheet_lookup_keys": keys, "svg_to_net": svg_to_net, "svg_to_nets": svg_to_nets, "net_to_svg": net_to_svg, "net_uid_to_svg": uid_to_svg });
    let mut serialized = CountingWriter::new(limits.max_view_index_serialized_bytes_per_document);
    serde_json::to_writer(&mut serialized, &value).map_err(|error| {
        DesignError::context("schematic view index serialized limit exceeded", error)
    })?;
    Ok((value, budget.work))
}

fn add_svg_net_name(
    map: &mut BTreeMap<String, BTreeSet<String>>,
    id: &str,
    name: &str,
    budget: &mut ViewIndexBudget,
) -> Result<(), DesignError> {
    budget.attempt(id.len().saturating_add(name.len()).saturating_add(1))?;
    if map.get(id).is_some_and(|names| names.contains(name)) {
        return Ok(());
    }
    budget.retain(2, id.len().saturating_add(name.len()).saturating_add(128))?;
    map.entry(id.to_owned())
        .or_default()
        .insert(name.to_owned());
    Ok(())
}

struct ViewIndexBudget {
    items: usize,
    bytes: usize,
    work: usize,
    max_work: usize,
    limits: SchematicReviewSvgLimits,
}

impl ViewIndexBudget {
    fn new(limits: SchematicReviewSvgLimits, max_work: usize) -> Self {
        Self {
            items: 0,
            bytes: 0,
            work: 0,
            max_work,
            limits,
        }
    }

    fn attempt(&mut self, work: usize) -> Result<(), DesignError> {
        self.work = self
            .work
            .checked_add(work)
            .ok_or_else(|| DesignError::new("schematic view index work overflowed"))?;
        if self.work > self.max_work {
            return Err(DesignError::new(
                "schematic view index aggregate work limit exceeded",
            ));
        }
        Ok(())
    }

    fn retain(&mut self, items: usize, bytes: usize) -> Result<(), DesignError> {
        self.items = self
            .items
            .checked_add(items)
            .ok_or_else(|| DesignError::new("schematic view index item count overflowed"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| DesignError::new("schematic view index byte count overflowed"))?;
        if self.items > self.limits.max_view_index_items_per_document {
            return Err(DesignError::new("schematic view index item limit exceeded"));
        }
        if self.bytes > self.limits.max_view_index_materialized_bytes_per_document {
            return Err(DesignError::new("schematic view index byte limit exceeded"));
        }
        Ok(())
    }
}

fn net_summary(name: &str, net: Option<&Value>) -> Value {
    let Some(net) = net else {
        return json!({"uid": "", "name": name});
    };
    let mut row = serde_json::Map::new();
    row.insert("uid".to_owned(), json!(scalar_text(&net["uid"])));
    row.insert("name".to_owned(), json!(name));
    if let Some(value) = net.get("auto_named").and_then(Value::as_bool) {
        row.insert("auto_named".to_owned(), json!(value));
    }
    if let Some(value) = net
        .get("net_class")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        row.insert("net_class".to_owned(), json!(value));
    }
    Value::Object(row)
}

fn sheet_lookup_keys(sheet_path: &str, instance_path: &str) -> Vec<String> {
    let mut output = Vec::new();
    add_sheet_key(&mut output, instance_path);
    let parts = instance_path
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() > 1 {
        add_sheet_key(&mut output, &format!("/{}/", parts[1..].join("/")));
    } else if parts.len() == 1 {
        add_sheet_key(&mut output, "/");
    }
    add_sheet_key(&mut output, sheet_path);
    output
}

fn add_sheet_key(output: &mut Vec<String>, value: &str) {
    let text = value.trim();
    if text.is_empty() {
        return;
    }
    if !output.iter().any(|known| known == text) {
        output.push(text.to_owned());
    }
    let normalized = if text.starts_with('/') {
        text.to_owned()
    } else {
        let value = format!("/{text}");
        if !output.contains(&value) {
            output.push(value.clone());
        }
        value
    };
    if normalized != "/" && !normalized.ends_with('/') {
        let slash = format!("{normalized}/");
        if !output.contains(&slash) {
            output.push(slash);
        }
    }
}

fn write_transformed(
    writer: &mut LimitedBytes,
    source: &str,
    attrs: &BTreeMap<String, String>,
) -> Result<(), DesignError> {
    for line in source.split_inclusive('\n') {
        let fragment = line
            .strip_prefix("<g id=\"")
            .and_then(|rest| rest.find('"').map(|end| (rest, end)))
            .and_then(|(rest, end)| attrs.get(&rest[..end]));
        let output_len = themed_len(line)?
            .checked_add(fragment.map_or(0, String::len))
            .ok_or_else(|| DesignError::new("schematic review line length overflowed"))?;
        if output_len > writer.remaining() {
            return Err(DesignError::new(
                "schematic review SVG output limit exceeded",
            ));
        }
        if fragment.is_none()
            && !line.contains(" fill=\"#")
            && !line.contains(" stroke=\"#")
            && !line.contains("-opacity=\"")
        {
            writer.write_all(line.as_bytes()).map_err(output_error)?;
            continue;
        }
        let mut owned;
        let transformed_source = if let Some(fragment) = fragment {
            owned = String::with_capacity(line.len() + fragment.len());
            let end = line
                .rfind('>')
                .ok_or_else(|| DesignError::new("schematic record group is malformed"))?;
            owned.push_str(&line[..end]);
            owned.push_str(fragment);
            owned.push_str(&line[end..]);
            owned.as_str()
        } else {
            line
        };
        let transformed = black_and_white(transformed_source);
        writer
            .write_all(transformed.as_bytes())
            .map_err(output_error)?;
    }
    Ok(())
}

fn themed_len(source: &str) -> Result<usize, DesignError> {
    let mut removed = 0_usize;
    for attribute in [" fill-opacity=\"", " stroke-opacity=\""] {
        let mut remaining = source;
        while let Some(start) = remaining.find(attribute) {
            let value = &remaining[start + attribute.len()..];
            let end = value
                .find('"')
                .ok_or_else(|| DesignError::new("schematic SVG opacity is unterminated"))?;
            removed = removed
                .checked_add(attribute.len() + end + 1)
                .ok_or_else(|| DesignError::new("schematic SVG opacity length overflowed"))?;
            remaining = &value[end + 1..];
        }
    }
    source
        .len()
        .checked_sub(removed)
        .ok_or_else(|| DesignError::new("schematic SVG transformed length underflowed"))
}

fn black_and_white(source: &str) -> String {
    let mut output = source.to_owned();
    for (attribute, opacity_attribute) in [
        (" fill=\"#", "\" fill-opacity=\""),
        (" stroke=\"#", "\" stroke-opacity=\""),
    ] {
        let mut search_from = 0_usize;
        while let Some(relative) = output[search_from..].find(attribute) {
            let color_start = search_from + relative + attribute.len() - 1;
            let color_end = color_start + 7;
            let Some(color) = output.get(color_start..color_end) else {
                break;
            };
            if color[1..].bytes().all(|byte| byte.is_ascii_hexdigit()) {
                let opacity = output[color_end..]
                    .strip_prefix(opacity_attribute)
                    .and_then(|rest| rest.split_once('"').map(|(value, _)| value));
                let white = match color {
                    "#F5F4EF" | "#FFFFC2" => opacity.is_none(),
                    "#FFFFFF" => opacity.is_none() || opacity == Some("0"),
                    _ => false,
                };
                output.replace_range(
                    color_start..color_end,
                    if white { "#FFFFFF" } else { "#000000" },
                );
            }
            search_from = color_end;
        }
    }
    for attribute in [" fill-opacity=\"", " stroke-opacity=\""] {
        while let Some(start) = output.find(attribute) {
            let value_start = start + attribute.len();
            let Some(end) = output[value_start..].find('"') else {
                break;
            };
            output.replace_range(start..value_start + end + 1, "");
        }
    }
    output
}

fn xml_attribute(value: &str) -> String {
    let mut output = String::with_capacity(xml_attribute_len(value));
    for character in value.chars() {
        output.push_str(match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            _ => {
                output.push(character);
                continue;
            }
        });
    }
    output
}

fn xml_attribute_len(value: &str) -> usize {
    value.chars().fold(0_usize, |total, character| {
        total.saturating_add(match character {
            '&' => 5,
            '<' | '>' => 4,
            '"' => 6,
            _ => character.len_utf8(),
        })
    })
}

fn output_error(error: io::Error) -> DesignError {
    DesignError::context("schematic review SVG output limit exceeded", error)
}

struct LimitedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl LimitedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    fn finish(self) -> Result<String, DesignError> {
        String::from_utf8(self.bytes)
            .map_err(|error| DesignError::context("schematic review SVG is not UTF-8", error))
    }

    fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.bytes.len())
    }
}
impl Write for LimitedBytes {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let total = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("byte count overflowed"))?;
        if total > self.limit {
            return Err(io::Error::other("output byte limit exceeded"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct XmlTextWriter<'a>(&'a mut LimitedBytes);
impl Write for XmlTextWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut span_start = 0_usize;
        for (index, byte) in buf.iter().enumerate() {
            let replacement: &[u8] = match byte {
                b'&' => b"&amp;",
                b'<' => b"&lt;",
                b'>' => b"&gt;",
                _ => continue,
            };
            if span_start < index {
                self.0.write_all(&buf[span_start..index])?;
            }
            self.0.write_all(replacement)?;
            span_start = index + 1;
        }
        if span_start < buf.len() {
            self.0.write_all(&buf[span_start..])?;
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[derive(Default)]
struct HashWriter(Sha256);

impl HashWriter {
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    fn finish_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let bytes = self.finish();
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }
}

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct CountingWriter {
    written: usize,
    limit: usize,
}

struct PrettyCachePreflight {
    written: usize,
    newlines: usize,
    limit: usize,
}

impl PrettyCachePreflight {
    const fn new(limit: usize) -> Self {
        Self {
            written: 0,
            newlines: 0,
            limit,
        }
    }
}

impl Write for PrettyCachePreflight {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.written = self
            .written
            .checked_add(bytes.len())
            .filter(|written| *written <= self.limit)
            .ok_or_else(|| io::Error::other("byte limit exceeded"))?;
        self.newlines = self
            .newlines
            .checked_add(bytes.iter().filter(|byte| **byte == b'\n').count())
            .ok_or_else(|| io::Error::other("newline count overflowed"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ExactBytesWriter {
    bytes: Vec<u8>,
    written: usize,
}

impl ExactBytesWriter {
    fn new(size: usize) -> Self {
        Self {
            bytes: vec![0; size],
            written: 0,
        }
    }

    fn finish(self) -> Result<String, DesignError> {
        if self.written != self.bytes.len() {
            return Err(DesignError::new(
                "schematic metadata cache serialization size changed",
            ));
        }
        String::from_utf8(self.bytes)
            .map_err(|error| DesignError::context("schematic metadata cache is not UTF-8", error))
    }
}

impl Write for ExactBytesWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self
            .written
            .checked_add(bytes.len())
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| io::Error::other("exact buffer length exceeded"))?;
        self.bytes[self.written..end].copy_from_slice(bytes);
        self.written = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl CountingWriter {
    fn new(limit: usize) -> Self {
        Self { written: 0, limit }
    }
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let total = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("byte count overflowed"))?;
        if total > self.limit {
            return Err(io::Error::other("byte limit exceeded"));
        }
        self.written = total;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde::Serialize;
    use serde_json::Value;

    use super::{LimitedBytes, XmlTextWriter, black_and_white, nested_pretty_raw_value};

    #[test]
    fn cached_design_preserves_nested_pretty_json_exactly() {
        #[derive(Serialize)]
        struct Original<'a> {
            before: &'static str,
            design: &'a Value,
            after: u8,
        }
        #[derive(Serialize)]
        struct Cached<'a> {
            before: &'static str,
            design: &'a serde_json::value::RawValue,
            after: u8,
        }
        let design = serde_json::json!({"nested": {"items": [1, 2]}, "text": "exact"});
        let raw = nested_pretty_raw_value(&design, 4096).unwrap();
        let expected = serde_json::to_string_pretty(&Original {
            before: "value",
            design: &design,
            after: 3,
        })
        .unwrap();
        let actual = serde_json::to_string_pretty(&Cached {
            before: "value",
            design: &raw,
            after: 3,
        })
        .unwrap();
        assert_eq!(actual, expected);
        let exact_cache_bytes =
            serde_json::to_string_pretty(&design).unwrap().len() + raw.get().len();
        nested_pretty_raw_value(&design, exact_cache_bytes).expect("exact cache ceiling");
        assert!(nested_pretty_raw_value(&design, exact_cache_bytes - 1).is_err());
    }

    #[test]
    fn xml_text_writer_preserves_utf8_and_escapes_only_xml_text_bytes() {
        let input = "plain & <tag> \"quoted\" café";
        let expected = "plain &amp; &lt;tag&gt; \"quoted\" café";
        let mut output = LimitedBytes::new(expected.len());
        XmlTextWriter(&mut output)
            .write_all(input.as_bytes())
            .unwrap();
        assert_eq!(output.finish().unwrap(), expected);

        let mut under = LimitedBytes::new(expected.len() - 1);
        assert!(
            XmlTextWriter(&mut under)
                .write_all(input.as_bytes())
                .is_err()
        );
    }

    #[test]
    fn review_theme_changes_color_attributes_but_not_literal_hex_text() {
        let source = concat!(
            "<text fill=\"#123456\" stroke=\"#F5F4EF\">#123456</text>",
            "<path fill=\"#FFFFFF\" fill-opacity=\"0.5019607843137255\"/>",
            "<path fill=\"#F5F4EF\" fill-opacity=\"0.5019607843137255\"/>",
            "<path fill=\"#FFFFFF\" fill-opacity=\"0\"/>",
            "<path fill=\"#F5F4EF\"/>"
        );
        assert_eq!(
            black_and_white(source),
            concat!(
                "<text fill=\"#000000\" stroke=\"#FFFFFF\">#123456</text>",
                "<path fill=\"#000000\"/>",
                "<path fill=\"#000000\"/>",
                "<path fill=\"#FFFFFF\"/>",
                "<path fill=\"#FFFFFF\"/>"
            )
        );
    }
}

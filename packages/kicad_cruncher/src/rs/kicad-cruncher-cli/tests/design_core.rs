use std::path::PathBuf;

use kicad_cruncher_cli::design::{
    SchematicPlotDocumentsLimits, SchematicSvgDocumentsLimits, build_schematic_base_svgs,
    build_schematic_base_svgs_for_plot_documents, build_schematic_base_svgs_with_limits,
    build_schematic_plot_document_artifacts, build_schematic_plot_documents,
    build_schematic_plot_documents_with_limits, build_structured_design_facts, load_design_sources,
};
use kicad_cruncher_cli::schematic_review_svg::{
    SchematicReviewSvgLimits, build_schematic_review_svgs, build_schematic_review_svgs_with_limits,
};
use kicad_monkey_core::{KiCadNetlist, validate_compiled_schematic_graph};
use serde_json::Value;

fn hlr_test_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pro")
}

#[test]
fn project_sources_feed_graph_and_netlist_without_a_sidecar() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    assert_eq!(loaded.bundle.project_path(), Some("hlr_test.kicad_pro"));
    assert_eq!(loaded.bundle.root_schematic_path(), "hlr_test.kicad_sch");
    assert_eq!(loaded.bundle.sources().len(), 2);

    let facts = build_structured_design_facts(&loaded).unwrap();
    validate_compiled_schematic_graph(&facts.compiled_schematic_graph).unwrap();
    assert!(!facts.netlist.components.is_empty());
    assert!(!facts.netlist.nets.is_empty());
    assert_hlr_netlist_json(&facts.netlist_json, &facts.netlist);
    assert!(facts.kicad_netlist.starts_with("(export"));
    assert!(facts.kicad_netlist.contains("(version \"E\""));
    assert!(facts.kicad_netlist.contains("(design"));
}

fn assert_hlr_netlist_json(payload: &Value, netlist: &KiCadNetlist) {
    assert_eq!(payload["schema"], "kicad_monkey.netlist.a0");
    assert_eq!(payload["generator"], "kicad_monkey");
    assert_eq!(
        payload["components"]
            .as_array()
            .expect("component rows")
            .len(),
        netlist.components.len()
    );
    assert_eq!(
        payload["nets"].as_array().expect("net rows").len(),
        netlist.nets.len()
    );
    assert_eq!(payload["net_classes"][0]["name"], "Default");
    assert_eq!(payload["design"]["tool"], "kicad_monkey");
    assert_eq!(payload["design"]["sheets"][0]["name"], "/");
    let first_component = &payload["components"][0];
    assert_eq!(first_component["designator"], "U1");
    assert_eq!(first_component["parameters"]["_source_cad"], "kicad");
    assert_eq!(
        first_component["parameters"]["kicad_instance_uuid"],
        "6c953fb9-8db6-4f7c-b301-f9d89614ea74"
    );
}

#[test]
fn direct_schematic_input_discovers_the_adjacent_project() {
    let loaded = load_design_sources(&hlr_test_project().with_extension("kicad_sch")).unwrap();
    assert_eq!(loaded.bundle.project_path(), Some("hlr_test.kicad_pro"));
    assert_eq!(loaded.bundle.sources().len(), 2);
}

#[test]
fn schematic_plot_batches_enforce_exact_document_and_output_limits() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let facts = build_structured_design_facts(&loaded).unwrap();
    let baseline = build_schematic_plot_documents(&loaded, &facts.schematic_instances).unwrap();
    let output_bytes = serde_json::to_vec(&baseline).unwrap().len();
    let exact = SchematicPlotDocumentsLimits {
        max_documents: baseline.len(),
        max_total_output_bytes: output_bytes,
        ..SchematicPlotDocumentsLimits::default()
    };
    build_schematic_plot_documents_with_limits(&loaded, &facts.schematic_instances, exact)
        .expect("exact batch limits");

    for limits in [
        SchematicPlotDocumentsLimits {
            max_documents: baseline.len() - 1,
            ..exact
        },
        SchematicPlotDocumentsLimits {
            max_total_output_bytes: output_bytes - 1,
            ..exact
        },
        SchematicPlotDocumentsLimits {
            max_total_derived_items: 0,
            ..exact
        },
        SchematicPlotDocumentsLimits {
            max_total_materialized_bytes: 0,
            ..exact
        },
    ] {
        assert!(
            build_schematic_plot_documents_with_limits(
                &loaded,
                &facts.schematic_instances,
                limits,
            )
            .is_err()
        );
    }
}

#[test]
fn schematic_base_svg_preserves_canvas_and_record_identity() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let facts = build_structured_design_facts(&loaded).unwrap();
    let documents = build_schematic_plot_documents(&loaded, &facts.schematic_instances).unwrap();
    let artifacts = build_schematic_base_svgs(&documents).unwrap();
    assert_eq!(artifacts.len(), documents.len());
    for (document, artifact) in documents.iter().zip(&artifacts) {
        assert_eq!(artifact.document_id, document["document_id"]);
        assert!(artifact.svg.starts_with("<?xml version=\"1.0\""));
        assert!(artifact.svg.contains(&format!(
            "viewBox=\"0 0 {} {}\"",
            document["canvas"]["width_nm"].as_u64().unwrap(),
            document["canvas"]["height_nm"].as_u64().unwrap()
        )));
        let records = document["records"].as_array().unwrap();
        assert_eq!(artifact.metrics.records, records.len());
        for record in records {
            let uuid = record["uuid"].as_str().unwrap();
            assert!(artifact.svg.contains(&format!("id=\"{uuid}\"")));
        }
    }
}

#[test]
fn schematic_base_svg_batches_enforce_exact_document_and_byte_limits() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let facts = build_structured_design_facts(&loaded).unwrap();
    let documents = build_schematic_plot_documents(&loaded, &facts.schematic_instances).unwrap();
    let baseline = build_schematic_base_svgs(&documents).unwrap();
    let total_bytes = baseline.iter().map(|artifact| artifact.svg.len()).sum();
    let exact = SchematicSvgDocumentsLimits {
        max_documents: baseline.len(),
        max_total_svg_bytes: total_bytes,
        ..SchematicSvgDocumentsLimits::default()
    };
    build_schematic_base_svgs_with_limits(&documents, exact.clone()).expect("exact SVG limits");

    let document_error = build_schematic_base_svgs_with_limits(
        &documents,
        SchematicSvgDocumentsLimits {
            max_documents: baseline.len() - 1,
            ..exact.clone()
        },
    )
    .unwrap_err();
    assert!(document_error.to_string().contains("document count"));
    let byte_error = build_schematic_base_svgs_with_limits(
        &documents,
        SchematicSvgDocumentsLimits {
            max_total_svg_bytes: total_bytes - 1,
            ..exact
        },
    )
    .unwrap_err();
    assert!(byte_error.to_string().contains("base SVG"));

    let first_bytes = baseline[0].svg.len();
    let mut per_document = SchematicSvgDocumentsLimits::default().per_document;
    per_document.max_svg_bytes = first_bytes;
    build_schematic_base_svgs_with_limits(
        &documents[..1],
        SchematicSvgDocumentsLimits {
            max_documents: 1,
            max_total_svg_bytes: first_bytes,
            per_document,
        },
    )
    .expect("exact per-document SVG limit");
    per_document.max_svg_bytes = first_bytes - 1;
    let per_document_error = build_schematic_base_svgs_with_limits(
        &documents[..1],
        SchematicSvgDocumentsLimits {
            max_documents: 1,
            max_total_svg_bytes: first_bytes,
            per_document,
        },
    )
    .unwrap_err();
    assert!(per_document_error.to_string().contains("base SVG"));
}

#[test]
fn schematic_review_svg_binds_graph_enrichment_and_black_white_theme() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let facts = build_structured_design_facts(&loaded).unwrap();
    let documents =
        build_schematic_plot_document_artifacts(&loaded, &facts.schematic_instances).unwrap();
    let base = build_schematic_base_svgs_for_plot_documents(&documents).unwrap();
    let review = build_schematic_review_svgs(
        &documents,
        &base,
        &facts.compiled_schematic_graph,
        &facts.design_json,
        "../compiled_schematic_graph.json",
    )
    .unwrap();
    assert_eq!(review.len(), facts.schematic_instances.len());
    for (artifact, instance) in review.iter().zip(&facts.schematic_instances) {
        assert_review_svg(artifact, instance, &facts.compiled_schematic_graph);
    }
}

fn assert_review_svg(
    artifact: &kicad_cruncher_cli::schematic_review_svg::SchematicReviewSvg,
    instance: &kicad_monkey_core::KiCadSchematicInstance,
    graph: &kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0,
) {
    assert_eq!(artifact.page_occurrence_ref, instance.page_occurrence_ref);
    assert_eq!(artifact.artifact_key, "sch.dwg_scene");
    for marker in [
        "data-review-theme=\"kicad_cruncher.design_review.schematic_svg.a0\"",
        "id=\"schematic-enrichment-a0\"",
        "data-primitive=\"symbol\"",
        "compiled_schematic_graph_view",
        "../compiled_schematic_graph.json",
    ] {
        assert!(artifact.svg.contains(marker));
    }
    assert!(
        svg_colors(&artifact.svg)
            .iter()
            .all(|color| matches!(color.as_str(), "#000000" | "#FFFFFF"))
    );
    assert!(artifact.svg.contains("#000000"));
    assert_eq!(
        artifact.graph_link_count,
        graph
            .graphical_artifact_links
            .iter()
            .filter(|link| link.page_occurrence_ref == instance.page_occurrence_ref)
            .count()
    );
    assert_eq!(artifact.document_id, instance.document_id);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table-driven test covers every public enrichment ceiling and stale binding"
)]
fn schematic_review_svg_enforces_exact_and_one_under_output_limits() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let facts = build_structured_design_facts(&loaded).unwrap();
    let documents =
        build_schematic_plot_document_artifacts(&loaded, &facts.schematic_instances[..1]).unwrap();
    let mut base = build_schematic_base_svgs_for_plot_documents(&documents).unwrap();
    let baseline = build_schematic_review_svgs(
        &documents,
        &base,
        &facts.compiled_schematic_graph,
        &facts.design_json,
        "../compiled_schematic_graph.json",
    )
    .unwrap();
    let bytes = baseline[0].svg.len();
    let exact = SchematicReviewSvgLimits {
        max_documents: 1,
        max_total_output_bytes: bytes,
        max_output_bytes_per_document: bytes,
        ..SchematicReviewSvgLimits::default()
    };
    build_schematic_review_svgs_with_limits(
        &documents,
        &base,
        &facts.compiled_schematic_graph,
        &facts.design_json,
        "../compiled_schematic_graph.json",
        exact,
    )
    .expect("exact review SVG limit");
    for limits in [
        SchematicReviewSvgLimits {
            max_documents: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_total_output_bytes: bytes - 1,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_output_bytes_per_document: bytes - 1,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_graph_links_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_graph_index_items: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_graph_index_materialized_bytes: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_graph_artifact_bytes: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_graph_view_materialized_bytes: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_graph_view_serialized_bytes: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_record_attributes_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_record_attribute_bytes_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_svg_selector_ids_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_svg_selector_bytes_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_view_index_items_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_view_index_materialized_bytes_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_view_index_serialized_bytes_per_document: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_view_authority_items: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_view_authority_materialized_bytes: 0,
            ..exact
        },
        SchematicReviewSvgLimits {
            max_total_view_index_work: 0,
            ..exact
        },
    ] {
        assert!(
            build_schematic_review_svgs_with_limits(
                &documents,
                &base,
                &facts.compiled_schematic_graph,
                &facts.design_json,
                "../compiled_schematic_graph.json",
                limits,
            )
            .is_err()
        );
    }

    let original_hash = std::mem::replace(&mut base[0].plot_document_sha256, "stale".to_owned());
    assert!(
        build_schematic_review_svgs(
            &documents,
            &base,
            &facts.compiled_schematic_graph,
            &facts.design_json,
            "../compiled_schematic_graph.json",
        )
        .is_err()
    );
    base[0].plot_document_sha256 = original_hash;

    let mut stale_design = facts.design_json.clone();
    stale_design["compiled_schematic_graph"]["schema"] = Value::String("stale".to_owned());
    assert!(
        build_schematic_review_svgs(
            &documents,
            &base,
            &facts.compiled_schematic_graph,
            &stale_design,
            "../compiled_schematic_graph.json",
        )
        .is_err()
    );
}

fn svg_colors(svg: &str) -> Vec<String> {
    svg.match_indices('#')
        .filter_map(|(index, _)| svg.get(index..index + 7))
        .filter(|value| value[1..].bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_owned)
        .collect()
}

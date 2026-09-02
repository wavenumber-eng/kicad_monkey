use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use kicad_cruncher_cli::design::{
    BoardPlotDocumentLimits, SchematicPlotDocumentsLimits, SchematicSvgDocumentsLimits,
    build_board_plot_document, build_board_plot_document_with_limits, build_schematic_base_svgs,
    build_schematic_base_svgs_for_plot_documents, build_schematic_base_svgs_with_limits,
    build_schematic_plot_document_artifacts, build_schematic_plot_documents,
    build_schematic_plot_documents_with_limits, build_structured_design_facts, load_design_sources,
};
use kicad_cruncher_cli::pcb_review_svg::{
    PcbReviewSvgLimits, build_pcb_review_svgs, build_pcb_review_svgs_with_limits,
};
use kicad_cruncher_cli::schematic_review_svg::{
    SchematicReviewSvgLimits, build_schematic_review_svgs, build_schematic_review_svgs_with_limits,
};
use kicad_monkey_core::{
    BoardPlotLimits, BoardTextVariables, KiCadNetlist, PcbLimits, board_plot_facts_with_sidecars,
    validate_compiled_schematic_graph,
};
use serde_json::Value;

fn hlr_test_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pro")
}

fn taillight_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../tests/corpus/kicad/projects/taillight/input/11-10045__taillight__C.kicad_pro",
    )
}

struct TemporaryDesign {
    root: PathBuf,
}

impl Drop for TemporaryDesign {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn hlr_with_board_image() -> (TemporaryDesign, PathBuf) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "kicad-cruncher-board-image-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("temporary design directory");
    let hlr_project = hlr_test_project();
    let source = hlr_project.parent().expect("HLR directory");
    for name in ["hlr_test.kicad_pro", "hlr_test.kicad_sch"] {
        std::fs::copy(source.join(name), root.join(name)).expect("copy HLR source");
    }
    let mut board =
        std::fs::read_to_string(source.join("hlr_test.kicad_pcb")).expect("read HLR board");
    let closing = board.rfind(')').expect("board root close");
    board.insert_str(
        closing,
        r#"(net 1 "VIEW_REUSE_NET")
        (property "view_reuse_property" "present")
        (image (at 1000 2000) (layer "F.SilkS") (scale 0)
          (data "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB"))
"#,
    );
    std::fs::write(root.join("hlr_test.kicad_pcb"), board).expect("write image board");
    let project = root.join("hlr_test.kicad_pro");
    (TemporaryDesign { root }, project)
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

#[test]
fn validated_schematic_definition_preserves_cruncher_source_and_facts() {
    let loaded = load_design_sources(&hlr_test_project()).expect("single-pass schematic load");
    let source = loaded
        .bundle
        .root_schematic()
        .text()
        .expect("exact root schematic source");
    assert!(source.starts_with("(kicad_sch"));
    let facts = build_structured_design_facts(&loaded).expect("facts from validated definition");
    assert_eq!(facts.schematic_instances.len(), 1);
    assert_eq!(facts.netlist.components.len(), 1);
    assert!(!facts.netlist.nets.is_empty());
    validate_compiled_schematic_graph(&facts.compiled_schematic_graph)
        .expect("compiled graph from reused validation definition");
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
fn pcb_review_svg_uses_native_board_projection_and_enforces_output_limits() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let document = build_board_plot_document(&loaded).unwrap().unwrap();
    let baseline = build_pcb_review_svgs(&loaded, &document).unwrap();
    assert!(!baseline.is_empty());
    let other = load_design_sources(&taillight_project()).unwrap();
    assert!(build_pcb_review_svgs(&other, &document).is_err());
    assert!(baseline.iter().all(|artifact| {
        artifact.svg.contains("id=\"pcb-enrichment-a0\"")
            && artifact.svg.contains("data-review-layer=")
            && artifact.included_layers.contains(&"Edge.Cuts".to_owned())
    }));
    let total_bytes = baseline.iter().map(|artifact| artifact.svg.len()).sum();
    let largest = baseline
        .iter()
        .map(|artifact| artifact.svg.len())
        .max()
        .unwrap();
    let exact = PcbReviewSvgLimits {
        max_layers: baseline.len(),
        max_svg_bytes_per_layer: largest,
        max_total_svg_bytes: total_bytes,
        ..PcbReviewSvgLimits::default()
    };
    build_pcb_review_svgs_with_limits(&loaded, &document, exact)
        .expect("exact PCB review SVG limits");
    for limits in [
        PcbReviewSvgLimits {
            max_layers: baseline.len() - 1,
            ..exact
        },
        PcbReviewSvgLimits {
            max_svg_bytes_per_layer: largest - 1,
            ..exact
        },
        PcbReviewSvgLimits {
            max_total_svg_bytes: total_bytes - 1,
            ..exact
        },
        PcbReviewSvgLimits {
            max_metadata_bytes: 0,
            ..exact
        },
        PcbReviewSvgLimits {
            max_total_filter_work: 0,
            ..exact
        },
        PcbReviewSvgLimits {
            max_metadata_items: 0,
            ..exact
        },
        PcbReviewSvgLimits {
            max_metadata_materialized_bytes: 0,
            ..exact
        },
        PcbReviewSvgLimits {
            max_total_materialized_bytes: 0,
            ..exact
        },
        PcbReviewSvgLimits {
            max_total_composition_work: 0,
            ..exact
        },
    ] {
        assert!(build_pcb_review_svgs_with_limits(&loaded, &document, limits).is_err());
    }
}

#[test]
fn board_projection_limits_accept_exact_and_reject_one_under() {
    let loaded = load_design_sources(&hlr_test_project()).unwrap();
    let document = build_board_plot_document(&loaded).unwrap().unwrap();
    let copper_layers = document.copper_layer_count();
    let exact = BoardPlotDocumentLimits {
        max_copper_layers: copper_layers,
        max_contract_bytes: document.serialized_bytes().unwrap(),
        contract: kicad_monkey_core::BoardPlotContractLimits {
            max_records: document.record_count(),
            max_operations: document.operation_count(),
            ..kicad_monkey_core::BoardPlotContractLimits::default()
        },
        ..BoardPlotDocumentLimits::default()
    };
    build_board_plot_document_with_limits(&loaded, exact)
        .expect("exact board projection limits")
        .expect("PCB document");
    for limits in [
        BoardPlotDocumentLimits {
            max_copper_layers: copper_layers - 1,
            ..exact
        },
        BoardPlotDocumentLimits {
            max_contract_bytes: exact.max_contract_bytes - 1,
            ..exact
        },
        BoardPlotDocumentLimits {
            contract: kicad_monkey_core::BoardPlotContractLimits {
                max_records: document.record_count() - 1,
                ..exact.contract
            },
            ..exact
        },
        BoardPlotDocumentLimits {
            contract: kicad_monkey_core::BoardPlotContractLimits {
                max_operations: document.operation_count() - 1,
                ..exact.contract
            },
            ..exact
        },
    ] {
        assert!(build_board_plot_document_with_limits(&loaded, limits).is_err());
    }
}

#[test]
fn board_image_viewport_crosses_the_monkey_cruncher_boundary() {
    let (_temporary, project) = hlr_with_board_image();
    let loaded = load_design_sources(&project).expect("image design sources");
    let document = build_board_plot_document(&loaded)
        .expect("image board plot")
        .expect("PCB document");
    let artifacts = build_pcb_review_svgs(&loaded, &document).expect("image review SVGs");
    assert!(!artifacts.is_empty());
    for artifact in artifacts {
        assert_eq!(artifact.viewport_bounds_nm[2], 1_000_050_000);
        assert_eq!(artifact.viewport_bounds_nm[3], 2_000_050_000);
    }
}

#[test]
fn reused_board_view_preserves_full_monkey_facts_and_caller_limits() {
    let (_temporary, project) = hlr_with_board_image();
    let loaded = load_design_sources(&project).expect("image design sources");
    let source = loaded.pcb_source.as_deref().expect("PCB source");
    let facts = board_plot_facts_with_sidecars(
        source,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
    )
    .expect("source-bound board facts");
    assert!(facts.view().layers().next().is_some());
    assert!(facts.view().nets().next().is_some());
    assert!(facts.view().properties().next().is_some());
    assert!(facts.view().images().next().is_some());
    assert!(facts.view().setup().expect("board setup").is_some());
    assert!(
        facts
            .bounds(None, Default::default())
            .expect("image-aware bounds")
            .is_some()
    );

    assert!(
        board_plot_facts_with_sidecars(
            source,
            BoardPlotLimits::default(),
            PcbLimits {
                max_images: 0,
                ..PcbLimits::default()
            },
            &Default::default(),
            &BoardTextVariables::default(),
        )
        .is_err()
    );
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
            nm_text(document["canvas"]["width_nm"].as_u64().unwrap()),
            nm_text(document["canvas"]["height_nm"].as_u64().unwrap())
        )));
        assert!(!artifact.svg.contains("font-size=\"1270000\""));
        let records = document["records"].as_array().unwrap();
        assert_eq!(artifact.metrics.records, records.len());
        for record in records {
            let uuid = record["uuid"].as_str().unwrap();
            assert!(artifact.svg.contains(&format!("id=\"{uuid}\"")));
        }
    }
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
    let pretty_design = serde_json::to_string_pretty(&facts.design_json).unwrap();
    let nested_design_bytes =
        pretty_design.len() + pretty_design.bytes().filter(|byte| *byte == b'\n').count() * 2;
    let cache_bytes = pretty_design.len() + nested_design_bytes;
    let exact = SchematicReviewSvgLimits {
        max_documents: 1,
        max_total_output_bytes: bytes,
        max_output_bytes_per_document: bytes,
        max_cached_design_bytes: cache_bytes,
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
        SchematicReviewSvgLimits {
            max_cached_design_bytes: cache_bytes - 1,
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

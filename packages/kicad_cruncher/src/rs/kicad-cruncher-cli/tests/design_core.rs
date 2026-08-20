use std::path::PathBuf;

use kicad_cruncher_cli::design::{
    SchematicPlotDocumentsLimits, SchematicSvgDocumentsLimits, build_schematic_base_svgs,
    build_schematic_base_svgs_with_limits, build_schematic_plot_documents,
    build_schematic_plot_documents_with_limits, build_structured_design_facts, load_design_sources,
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

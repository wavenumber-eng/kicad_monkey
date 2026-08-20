use std::path::PathBuf;

use kicad_cruncher_cli::design::{build_structured_design_facts, load_design_sources};
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

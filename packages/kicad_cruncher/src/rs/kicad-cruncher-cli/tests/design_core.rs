use std::path::PathBuf;

use kicad_cruncher_cli::design::{build_structured_design_facts, load_design_sources};
use kicad_monkey_core::validate_compiled_schematic_graph;

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
    assert!(facts.kicad_netlist.starts_with("(export"));
    assert!(facts.kicad_netlist.contains("(version \"E\""));
    assert!(facts.kicad_netlist.contains("(design"));
}

#[test]
fn direct_schematic_input_omits_project_source() {
    let loaded = load_design_sources(&hlr_test_project().with_extension("kicad_sch")).unwrap();
    assert_eq!(loaded.bundle.project_path(), None);
    assert_eq!(loaded.bundle.sources().len(), 1);
}

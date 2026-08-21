use kicad_monkey_contracts::generated::compiled_schematic_graph::{
    GraphicalTargetType, TerminalRole,
};
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    CompiledSchematicGraphLimits, SchematicBundleIndex, SchematicBundleLimits, SourceBundle,
    SourceBundleErrorKind, SourceBundleLimits, build_compiled_schematic_graph,
    validate_compiled_schematic_graph,
};

#[test]
fn structural_rows_are_deterministic_owned_and_semantically_valid() {
    let index = structural_index();
    assert_eq!(index.project_file(), "demo.kicad_pro");
    assert_eq!(
        index.portable_source_path("nested/root.kicad_sch"),
        "root.kicad_sch"
    );
    let first =
        build_compiled_schematic_graph(&index, Default::default()).expect("first structural graph");
    let second = build_compiled_schematic_graph(&index, Default::default())
        .expect("second structural graph");
    validate_compiled_schematic_graph(&first).expect("semantic structural graph");
    assert_eq!(
        serde_json::to_value(&first).expect("first JSON"),
        serde_json::to_value(&second).expect("second JSON")
    );
    assert_graph_shape(&first);
    assert_source_ownership(&first);
}

fn assert_graph_shape(
    graph: &kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0,
) {
    assert_eq!(graph.unit_definitions.len(), 2);
    assert_eq!(graph.page_definitions.len(), 2);
    assert_eq!(graph.unit_occurrences.len(), 2);
    assert_eq!(graph.page_occurrences.len(), 2);
    assert_eq!(graph.hierarchy_occurrences.len(), 1);
    assert_eq!(graph.component_occurrences.len(), 2);
    assert_eq!(graph.local_net_occurrences.len(), 8);
    assert_eq!(graph.terminal_occurrences.len(), 8);
    assert_eq!(graph.hierarchy_terminal_bindings.len(), 3);
    assert_eq!(graph.graphical_artifact_links.len(), 13);
    let sheet_entry_ids = graph
        .terminal_occurrences
        .iter()
        .filter(|row| row.role == TerminalRole::SheetEntry)
        .map(|row| row.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(sheet_entry_ids.len(), 3);
}

fn assert_source_ownership(
    graph: &kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0,
) {
    assert_eq!(
        graph.unit_definitions[0]
            .source_identity
            .sch_source_key_source_path
            .as_deref(),
        Some("root.kicad_sch")
    );
    assert_eq!(
        graph.unit_definitions[1]
            .source_identity
            .sch_source_key_source_path
            .as_deref(),
        Some("child.kicad_sch")
    );
    assert_eq!(
        graph.unit_occurrences[1]
            .parent_hierarchy_occurrence_ref
            .as_deref(),
        Some(graph.hierarchy_occurrences[0].id.as_str())
    );
    assert_eq!(
        graph
            .component_occurrences
            .iter()
            .map(|row| row.physical_designator.as_str())
            .collect::<Vec<_>>(),
        ["R1", "C1"]
    );
}

#[test]
fn every_structural_row_family_fails_before_one_over_publication() {
    let index = structural_index();
    let retained_string_bytes = minimum_retained_string_limit(&index);
    assert!(retained_string_bytes > 0);
    let exact = CompiledSchematicGraphLimits {
        design: Default::default(),
        max_unit_definitions: 2,
        max_page_definitions: 2,
        max_unit_occurrences: 2,
        max_page_occurrences: 2,
        max_hierarchy_occurrences: 1,
        max_component_occurrences: 2,
        max_local_net_occurrences: 8,
        max_terminal_occurrences: 8,
        max_hierarchy_terminal_bindings: 3,
        max_graphical_artifact_links: 13,
        max_retained_string_bytes: retained_string_bytes,
    };
    build_compiled_schematic_graph(&index, exact).expect("exact structural row limits");
    for limits in [
        CompiledSchematicGraphLimits {
            max_unit_definitions: 1,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_page_definitions: 1,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_unit_occurrences: 1,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_page_occurrences: 1,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_hierarchy_occurrences: 0,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_component_occurrences: 1,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_local_net_occurrences: 7,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_terminal_occurrences: 7,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_hierarchy_terminal_bindings: 2,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_graphical_artifact_links: 12,
            ..exact
        },
        CompiledSchematicGraphLimits {
            max_retained_string_bytes: retained_string_bytes - 1,
            ..exact
        },
    ] {
        let error = build_compiled_schematic_graph(&index, limits)
            .expect_err("one-under structural graph limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    }
}

#[test]
fn power_ports_link_parent_symbols_while_component_pins_link_rendered_pins() {
    let index = single_index(
        br##"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "power:+3V3" (power local)
              (pin_numbers (hide yes))
              (pin_names (hide yes))
              (symbol "power:+3V3_1_1"
                (pin power_in line (at 0 0 90) (length 0)
                  (name "~") (number "1"))))
            (symbol "Demo:Visible"
              (symbol "Demo:Visible_1_1"
                (pin passive line (at 0 0 0) (length 2.54)
                  (name "P") (number "1")))))
          (symbol (lib_id "power:+3V3") (lib_name "power:+3V3")
            (at 0 0 0) (uuid power)
            (property "Reference" "#PWR01")
            (property "Value" "+3V3")
            (pin "1" (uuid power-pin)))
          (symbol (lib_id "Demo:Visible") (lib_name "Demo:Visible")
            (at 20 0 0) (uuid component)
            (property "Reference" "U1")
            (property "Value" "Visible")
            (pin "1" (uuid component-pin))))"##,
    );
    let graph = build_compiled_schematic_graph(&index, Default::default())
        .expect("compiled graph pin selectors");
    validate_compiled_schematic_graph(&graph).expect("valid compiled graph pin selectors");

    for (role, expected_element_id) in [
        (TerminalRole::PowerPort, "power"),
        (TerminalRole::ComponentPin, "component-pin"),
    ] {
        let terminal = graph
            .terminal_occurrences
            .iter()
            .find(|terminal| terminal.role == role)
            .expect("terminal role");
        let link = graph
            .graphical_artifact_links
            .iter()
            .find(|link| {
                link.target_type == GraphicalTargetType::SchTerminalOccurrence
                    && link.target_ref == terminal.id
            })
            .expect("terminal graphical link");
        assert_eq!(link.element_id, expected_element_id);
    }
    assert!(
        graph
            .graphical_artifact_links
            .iter()
            .all(|link| link.element_id != "power-pin")
    );
}

fn minimum_retained_string_limit(index: &SchematicBundleIndex) -> usize {
    let mut high = 1_usize;
    while build_compiled_schematic_graph(
        index,
        CompiledSchematicGraphLimits {
            max_retained_string_bytes: high,
            ..Default::default()
        },
    )
    .is_err()
    {
        high = high.checked_mul(2).expect("finite retained-string limit");
    }
    let mut low = 0_usize;
    while low < high {
        let middle = low + (high - low) / 2;
        if build_compiled_schematic_graph(
            index,
            CompiledSchematicGraphLimits {
                max_retained_string_bytes: middle,
                ..Default::default()
            },
        )
        .is_ok()
        {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn structural_index() -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (uuid structural-root)
      (lib_symbols
        (symbol "Demo:One"
          (symbol "Demo:One_1_1"
            (pin passive line (at 0 0 0) (name "P") (number "1")))))
      (bus (pts (xy 0 20) (xy 10 20)) (uuid root-bus))
      (bus_entry (at 2 20) (size 0 -5) (uuid root-entry))
      (sheet (uuid child-sheet)
        (property "Sheetname" "Child")
        (property "Sheetfile" "child.kicad_sch")
        (pin "SIG" input (at 20 0 180) (uuid sheet-pin))
        (pin "LEFT" input (at 30 0 180))
        (pin "RIGHT" output (at 40 0 180)))
      (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid root-symbol)
        (property "Reference" "R1") (property "Value" "One")))"#
        .to_vec();
    let child = br#"(kicad_sch
      (uuid structural-child)
      (lib_symbols
        (symbol "Demo:One"
          (symbol "Demo:One_1_1"
            (pin passive line (at 0 0 0) (name "P") (number "1")))))
      (hierarchical_label "SIG" (shape input) (at 20 0 0) (uuid child-port))
      (hierarchical_label "LEFT" (shape output) (at 30 0 0) (uuid child-left))
      (hierarchical_label "RIGHT" (shape input) (at 40 0 0) (uuid child-right))
      (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid child-symbol)
        (property "Reference" "C1") (property "Value" "One")))"#
        .to_vec();
    let buffers = vec![project, root, child];
    let manifest = SourceBundleManifestA0 {
        project_path: Some("nested/demo.kicad_pro".to_owned()),
        root_schematic_path: "nested/root.kicad_sch".to_owned(),
        schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
        sources: vec![
            descriptor("nested/demo.kicad_pro", SourceKind::Project, 0, &buffers[0]),
            descriptor(
                "nested/root.kicad_sch",
                SourceKind::Schematic,
                1,
                &buffers[1],
            ),
            descriptor(
                "nested/child.kicad_sch",
                SourceKind::Schematic,
                2,
                &buffers[2],
            ),
        ],
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    };
    let bundle = SourceBundle::from_manifest(manifest, buffers, SourceBundleLimits::default())
        .expect("structural source bundle");
    SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("structural schematic index")
}

fn single_index(root: &[u8]) -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let root = root.to_vec();
    let buffers = vec![project, root];
    let manifest = SourceBundleManifestA0 {
        project_path: Some("demo.kicad_pro".to_owned()),
        root_schematic_path: "root.kicad_sch".to_owned(),
        schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
        sources: vec![
            descriptor("demo.kicad_pro", SourceKind::Project, 0, &buffers[0]),
            descriptor("root.kicad_sch", SourceKind::Schematic, 1, &buffers[1]),
        ],
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    };
    let bundle = SourceBundle::from_manifest(manifest, buffers, SourceBundleLimits::default())
        .expect("single schematic source bundle");
    SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("single schematic index")
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

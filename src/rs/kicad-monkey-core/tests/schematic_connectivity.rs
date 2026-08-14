use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicDriverPriority,
    SchematicOccurrenceConnectivityLimits, SchematicPoint, SchematicWireDriverKind, SourceBundle,
    SourceBundleErrorKind, SourceBundleLimits, build_schematic_occurrence_subgraphs,
};

#[test]
fn occurrence_subgraphs_attach_exact_carriers_without_attaching_pins_mid_segment() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Part"
              (symbol "Demo:Part_1_1"
                (pin passive line (at 0 0 0) (name "P1") (number "1"))
                (pin passive line (at 7 0 0) (name "MID") (number "2")))))
          (wire (pts (xy 0 0) (xy 10 0)) (uuid horizontal))
          (wire (pts (xy 5 5) (xy 5 0)) (uuid vertical))
          (junction (at 5 0) (uuid junction))
          (label "SIG" (at 8 0 0) (uuid label))
          (no_connect (at 0 0) (uuid nc))
          (symbol (lib_id "Demo:Part") (lib_name "Demo:Part")
            (at 0 0 0) (uuid placed)
            (property "Reference" "U1") (property "Value" "Part")))"#,
    );
    let subgraphs = build_schematic_occurrence_subgraphs(
        &index,
        1,
        SchematicOccurrenceConnectivityLimits::default(),
    )
    .expect("occurrence connectivity");

    let signal = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "SIG")
        .expect("labelled wire subgraph");
    assert_eq!(signal.chosen_priority, SchematicDriverPriority::LocalLabel);
    assert_eq!(
        signal.chosen_kind,
        Some(SchematicWireDriverKind::LocalLabel)
    );
    assert_eq!(
        points(&signal.coords),
        [
            [0, 0],
            [50_000, 0],
            [50_000, 50_000],
            [80_000, 0],
            [100_000, 0]
        ]
    );
    assert!(signal.no_connect);
    assert_eq!(signal.pin_drivers.len(), 1);
    assert_eq!(signal.pin_drivers[0].pin_number, "1");

    let middle_pin = subgraphs
        .iter()
        .find(|subgraph| {
            subgraph
                .pin_drivers
                .iter()
                .any(|driver| driver.pin_number == "2")
        })
        .expect("middle pin singleton");
    assert_eq!(points(&middle_pin.coords), [[70_000, 0]]);
    assert!(middle_pin.label_drivers.is_empty());
}

#[test]
fn canonical_bus_members_merge_distinct_wire_label_spellings() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (bus_alias "BUS" (members "A/B"))
          (bus (pts (xy 0 20) (xy 10 20)) (uuid bus))
          (bus_entry (at 2 20) (size 0 -5) (uuid tap-a))
          (bus_entry (at 8 20) (size 0 -5) (uuid tap-b))
          (wire (pts (xy 2 15) (xy 2 10)) (uuid wire-a))
          (wire (pts (xy 8 15) (xy 8 10)) (uuid wire-b))
          (label "A{slash}B" (at 2 10 0) (uuid label-a))
          (label "A/B" (at 8 10 0) (uuid label-b))
          (global_label "BUS" (shape output) (at 0 20 0) (uuid bus-name)))"#,
    );
    let subgraphs = build_schematic_occurrence_subgraphs(
        &index,
        1,
        SchematicOccurrenceConnectivityLimits::default(),
    )
    .expect("bus member connectivity");
    let member = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "A/B")
        .expect("canonical member subgraph");
    assert_eq!(member.label_drivers.len(), 2);
    assert_eq!(
        points(&member.coords),
        [
            [20_000, 100_000],
            [20_000, 150_000],
            [80_000, 100_000],
            [80_000, 150_000]
        ]
    );
}

#[test]
fn stacked_pins_power_priority_and_hidden_no_connects_are_materialized() {
    let index = index(
        br##"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Stacked"
              (symbol "Demo:Stacked_1_1"
                (pin passive line (at 0 0 0) (name "PAD") (number "[A1-A4, B7]"))))
            (symbol "power:LOCAL" (power local)
              (symbol "power:LOCAL_1_1"
                (pin power_in line (at 0 0 0) (name "PWR") (number "1"))))
            (symbol "Demo:NC"
              (symbol "Demo:NC_1_1"
                (pin no_connect line (at 0 0 0) (name "NC1") (number "1") (hide yes))
                (pin no_connect line (at 0 0 0) (name "NC2") (number "2") (hide yes)))))
          (symbol (lib_id "Demo:Stacked") (lib_name "Demo:Stacked")
            (at 0 0 0) (uuid stacked)
            (property "Reference" "J1") (property "Value" "Stacked"))
          (symbol (lib_id "power:LOCAL") (lib_name "power:LOCAL")
            (at 20 0 0) (uuid power)
            (property "Reference" "#PWR01") (property "Value" "+3V3_LOCAL"))
          (symbol (lib_id "Demo:NC") (lib_name "Demo:NC")
            (at 40 0 0) (uuid nc)
            (property "Reference" "U1") (property "Value" "NC")))"##,
    );
    let subgraphs = build_schematic_occurrence_subgraphs(
        &index,
        1,
        SchematicOccurrenceConnectivityLimits::default(),
    )
    .expect("pin semantics");

    let stacked = subgraphs
        .iter()
        .find(|subgraph| subgraph.pin_drivers.iter().any(|pin| pin.reference == "J1"))
        .expect("stacked pin subgraph");
    assert_eq!(
        stacked
            .pin_drivers
            .iter()
            .map(|pin| (pin.pin_number.as_str(), pin.pin_name.as_str()))
            .collect::<Vec<_>>(),
        [
            ("A1", "PAD_A1"),
            ("A2", "PAD_A2"),
            ("A3", "PAD_A3"),
            ("A4", "PAD_A4"),
            ("B7", "PAD_B7"),
        ]
    );

    let power = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "+3V3_LOCAL")
        .expect("local power subgraph");
    assert_eq!(
        power.chosen_priority,
        SchematicDriverPriority::LocalPowerPin
    );
    assert_eq!(
        power.chosen_kind,
        Some(SchematicWireDriverKind::LocalPowerPin)
    );

    let hidden = subgraphs
        .iter()
        .filter(|subgraph| subgraph.pin_drivers.iter().any(|pin| pin.reference == "U1"))
        .collect::<Vec<_>>();
    assert_eq!(hidden.len(), 2);
    assert_ne!(hidden[0].coords, hidden[1].coords);
}

#[test]
fn occurrence_connectivity_limits_fail_closed_independently() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (wire (pts (xy 0 0) (xy 10 0)) (uuid wire))
          (bus_entry (at 20 20) (size 1 1) (uuid entry))
          (label "A" (at 0 0 0) (uuid a))
          (label "B" (at 30 30 0) (uuid b)))"#,
    );
    let exact = SchematicOccurrenceConnectivityLimits {
        max_entry_segments: 1,
        max_entry_index_nodes: 1,
        max_graph_points: 5,
        max_label_drivers: 2,
        max_subgraphs: 4,
        max_retained_points: 5,
        max_retained_string_bytes: 6,
        max_attachment_query_work: 5,
        ..SchematicOccurrenceConnectivityLimits::default()
    };
    build_schematic_occurrence_subgraphs(&index, 1, exact).expect("simultaneous exact limits");
    for (limits, message) in [
        (
            SchematicOccurrenceConnectivityLimits {
                max_attachment_query_work: 4,
                ..exact
            },
            "query work",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_entry_segments: 0,
                ..exact
            },
            "bus-entry segment count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_entry_index_nodes: 0,
                ..exact
            },
            "index node count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_graph_points: 4,
                ..exact
            },
            "graph point count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_label_drivers: 1,
                ..exact
            },
            "label driver count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_subgraphs: 3,
                ..exact
            },
            "subgraph count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_retained_points: 4,
                ..exact
            },
            "retained subgraph points",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_retained_string_bytes: 5,
                ..exact
            },
            "retained string bytes",
        ),
    ] {
        let error = build_schematic_occurrence_subgraphs(&index, 1, limits)
            .expect_err("independent occurrence connectivity limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(message), "{}", error.message);
    }
}

#[test]
fn pin_expansion_and_jumper_work_limits_are_independent() {
    let stacked_index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Stacked"
              (symbol "Demo:Stacked_1_1"
                (pin passive line (at 0 0 0) (name "PAD") (number "[1-3]")))))
          (symbol (lib_id "Demo:Stacked") (lib_name "Demo:Stacked")
            (at 0 0 0) (uuid placed)
            (property "Reference" "J1") (property "Value" "Stacked")))"#,
    );
    let exact = SchematicOccurrenceConnectivityLimits {
        max_graph_points: 1,
        max_pin_drivers: 3,
        max_subgraphs: 1,
        max_retained_points: 1,
        max_retained_string_bytes: 67,
        max_expanded_pins: 3,
        max_expanded_pin_bytes: 3,
        ..SchematicOccurrenceConnectivityLimits::default()
    };
    build_schematic_occurrence_subgraphs(&stacked_index, 1, exact)
        .expect("simultaneous exact pin limits");
    for (limits, message) in [
        (
            SchematicOccurrenceConnectivityLimits {
                max_pin_drivers: 2,
                ..exact
            },
            "pin driver count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_expanded_pins: 2,
                ..exact
            },
            "expanded pin count",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_expanded_pin_bytes: 2,
                ..exact
            },
            "expanded pin bytes",
        ),
        (
            SchematicOccurrenceConnectivityLimits {
                max_retained_string_bytes: 66,
                ..exact
            },
            "retained string bytes",
        ),
    ] {
        let error = build_schematic_occurrence_subgraphs(&stacked_index, 1, limits)
            .expect_err("independent pin connectivity limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(message), "{}", error.message);
    }

    let jumper_index = index(jumper_source());
    let jumper_library = &jumper_index
        .definition("design/root.kicad_sch")
        .expect("jumper definition")
        .library_symbols[0];
    assert!(jumper_library.duplicate_pin_numbers_are_jumpers);
    assert_eq!(jumper_library.jumper_pin_groups, [["2", "3"]]);
    let exact_jumper = SchematicOccurrenceConnectivityLimits {
        max_jumper_union_work: 6,
        ..SchematicOccurrenceConnectivityLimits::default()
    };
    let subgraphs = build_schematic_occurrence_subgraphs(&jumper_index, 1, exact_jumper)
        .expect("exact jumper union work");
    let jumper_groups = subgraphs
        .iter()
        .filter(|subgraph| !subgraph.pin_drivers.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(jumper_groups.len(), 2);
    assert_eq!(jumper_groups[0].pin_drivers.len(), 2);
    assert_eq!(jumper_groups[1].pin_drivers.len(), 2);
    let error = build_schematic_occurrence_subgraphs(
        &jumper_index,
        1,
        SchematicOccurrenceConnectivityLimits {
            max_jumper_union_work: 5,
            ..exact_jumper
        },
    )
    .expect_err("one-under jumper union work");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("jumper union work"));
}

#[test]
fn jumper_metadata_limits_fail_before_publication() {
    let source = source_bundle(jumper_source());
    for (limits, message) in [
        (
            SchematicBundleLimits {
                max_jumper_groups_per_source: 0,
                ..SchematicBundleLimits::default()
            },
            "jumper group count",
        ),
        (
            SchematicBundleLimits {
                max_jumper_members_per_source: 1,
                ..SchematicBundleLimits::default()
            },
            "jumper member count",
        ),
        (
            SchematicBundleLimits {
                max_jumper_member_bytes_per_source: 1,
                ..SchematicBundleLimits::default()
            },
            "jumper member bytes",
        ),
    ] {
        let error = SchematicBundleIndex::build(&source, limits)
            .expect_err("independent jumper metadata limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(message), "{}", error.message);
    }
}

fn jumper_source() -> &'static [u8] {
    br#"(kicad_sch
      (uuid root)
      (lib_symbols
        (symbol "Demo:Jumper" (duplicate_pin_numbers_are_jumpers yes)
          (jumper_pin_groups ("2" "3"))
          (symbol "Demo:Jumper_1_1"
            (pin passive line (at 0 0 0) (name "A") (number "1"))
            (pin passive line (at 10 0 0) (name "B") (number "1"))
            (pin passive line (at 20 0 0) (name "C") (number "2"))
            (pin passive line (at 30 0 0) (name "D") (number "3")))))
      (symbol (lib_id "Demo:Jumper") (lib_name "Demo:Jumper")
        (at 0 0 0) (uuid placed)
        (property "Reference" "J1") (property "Value" "Jumper")))"#
}

fn index(root: &[u8]) -> SchematicBundleIndex {
    SchematicBundleIndex::build(&source_bundle(root), SchematicBundleLimits::default())
        .expect("schematic bundle index")
}

fn source_bundle(root: &[u8]) -> SourceBundle {
    let project = b"{}".to_vec();
    let root = root.to_vec();
    let sources = vec![
        descriptor("design/demo.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
    ];
    SourceBundle::from_manifest(
        SourceBundleManifestA0 {
            project_path: Some("design/demo.kicad_pro".to_owned()),
            root_schematic_path: "design/root.kicad_sch".to_owned(),
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources,
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        },
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("source bundle")
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

fn points(values: &[SchematicPoint]) -> Vec<[i64; 2]> {
    values
        .iter()
        .map(|point| [point.x_iu, point.y_iu])
        .collect()
}

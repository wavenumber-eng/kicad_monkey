use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicBusConnectivityLimits,
    SchematicBusDriverKind, SchematicDriverPriority, SourceBundle, SourceBundleErrorKind,
    SourceBundleLimits, build_schematic_bus_subgraphs,
};

#[test]
fn bus_subgraphs_classify_taps_choose_drivers_and_expand_members() {
    let index = index();
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");
    let subgraphs =
        build_schematic_bus_subgraphs(definition, SchematicBusConnectivityLimits::default())
            .expect("bus subgraphs");

    let physical = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "ZZ[0..1]")
        .expect("physical named bus");
    assert_eq!(physical.chosen_priority, SchematicDriverPriority::Global);
    assert_eq!(
        physical.chosen_kind,
        Some(SchematicBusDriverKind::GlobalLabel)
    );
    assert_eq!(physical.members, ["ZZ0", "ZZ1"]);
    assert_eq!(
        points(&physical.coords),
        [[0, 0], [20_000, 0], [80_000, 0], [100_000, 0]]
    );
    assert_eq!(
        points(&physical.tap_wire_coords),
        [[20_000, 20_000], [80_000, 20_000]]
    );
    assert_eq!(
        physical
            .drivers
            .iter()
            .map(|driver| (driver.text.as_str(), driver.kind))
            .collect::<Vec<_>>(),
        [
            ("LOCAL{D0,D1}", SchematicBusDriverKind::LocalLabel),
            ("ZZ[0..1]", SchematicBusDriverKind::GlobalLabel),
        ]
    );

    let alias = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "ALIAS")
        .expect("orphan alias");
    assert_eq!(alias.members, ["A", "B"]);
    assert_eq!(points(&alias.coords), [[200_000, 200_000]]);

    let sheet_pin = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "QQ[0..1]")
        .expect("orphan sheet pin");
    assert_eq!(sheet_pin.members, ["QQ0", "QQ1"]);
    assert_eq!(sheet_pin.chosen_priority, SchematicDriverPriority::SheetPin);
}

#[test]
fn duplicate_aliases_are_last_writer_visible_once_per_build() {
    let index = index();
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");
    let subgraphs =
        build_schematic_bus_subgraphs(definition, SchematicBusConnectivityLimits::default())
            .expect("bus subgraphs");
    let alias = subgraphs
        .iter()
        .find(|subgraph| subgraph.chosen_name == "ALIAS")
        .expect("alias subgraph");
    assert_eq!(alias.members, ["A", "B"]);
}

#[test]
fn bus_compiler_structural_limits_fail_closed_before_growth() {
    let index = index();
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");
    let exact = exact_limits();
    build_schematic_bus_subgraphs(definition, exact).expect("simultaneous exact limits");
    assert_limit_failures(
        definition,
        [
            (
                SchematicBusConnectivityLimits {
                    max_segments: 2,
                    ..exact
                },
                "segment count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_segment_index_nodes: 1,
                    ..exact
                },
                "index node count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_segment_query_work: 15,
                    ..exact
                },
                "query work",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_taps: 1,
                    ..exact
                },
                "tap count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_aliases: 1,
                    ..exact
                },
                "alias index count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_graph_points: 3,
                    ..exact
                },
                "graph point count",
            ),
        ],
    );
}

#[test]
fn bus_compiler_output_limits_fail_closed_before_growth() {
    let index = index();
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");
    let exact = exact_limits();
    assert_limit_failures(
        definition,
        [
            (
                SchematicBusConnectivityLimits {
                    max_drivers: 5,
                    ..exact
                },
                "driver count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_subgraphs: 2,
                    ..exact
                },
                "subgraph count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_retained_points: 5,
                    ..exact
                },
                "retained points",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_retained_string_bytes: 127,
                    ..exact
                },
                "retained string bytes",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_expanded_members: 5,
                    ..exact
                },
                "member count",
            ),
            (
                SchematicBusConnectivityLimits {
                    max_expanded_member_bytes: 13,
                    ..exact
                },
                "member bytes",
            ),
        ],
    );
}

fn exact_limits() -> SchematicBusConnectivityLimits {
    SchematicBusConnectivityLimits {
        max_segments: 3,
        max_segment_index_nodes: 2,
        max_segment_query_work: 16,
        max_subgraphs: 3,
        max_drivers: 6,
        max_taps: 2,
        max_aliases: 2,
        max_graph_points: 4,
        max_retained_points: 6,
        max_retained_string_bytes: 128,
        max_expanded_members: 6,
        max_expanded_member_bytes: 14,
        ..SchematicBusConnectivityLimits::default()
    }
}

fn assert_limit_failures<const N: usize>(
    definition: &kicad_monkey_core::SchematicDefinition,
    cases: [(SchematicBusConnectivityLimits, &str); N],
) {
    for (limits, expected) in cases {
        let error = build_schematic_bus_subgraphs(definition, limits)
            .expect_err("independent bus compiler limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

fn index() -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (version 20250114)
      (uuid root)
      (bus_alias "ALIAS" (members "OLD"))
      (bus_alias "ALIAS" (members "A" "B"))
      (bus (pts (xy 0 0) (xy 10 0)) (uuid bus-a))
      (wire (pts (xy 2 2) (xy 2 0)) (uuid wire-a))
      (wire (pts (xy 8 2) (xy 8 0)) (uuid wire-b))
      (bus_entry (at 2 0) (size 0 2) (uuid tap-a))
      (bus_entry (at 8 0) (size 0 2) (uuid tap-b))
      (label "D0" (at 2 2 0) (uuid label-d0))
      (label "D0" (at 8 2 0) (uuid label-d0b))
      (label "LOCAL{D0,D1}" (at 5 0 0) (uuid label-bus))
      (global_label "ZZ[0..1]" (shape output) (at 0 0 0) (uuid global-bus))
      (hierarchical_label "ALIAS" (shape bidirectional) (at 20 20 0) (uuid orphan-alias))
      (sheet
        (uuid child-sheet)
        (property "Sheetname" "Child")
        (property "Sheetfile" "child.kicad_sch")
        (pin "QQ[0..1]" input (at 30 30 0) (uuid sheet-bus))))"#
        .to_vec();
    let child = b"(kicad_sch (version 20250114) (uuid child))".to_vec();
    let sources = vec![
        descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        descriptor("design/child.kicad_sch", SourceKind::Schematic, 2, &child),
    ];
    let bundle = SourceBundle::from_manifest(
        SourceBundleManifestA0 {
            project_path: Some("design/root.kicad_pro".to_owned()),
            root_schematic_path: "design/root.kicad_sch".to_owned(),
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources,
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        },
        vec![project, root, child],
        SourceBundleLimits::default(),
    )
    .expect("source bundle");
    SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default()).expect("schematic index")
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

fn points(values: &[kicad_monkey_core::SchematicPoint]) -> Vec<[i64; 2]> {
    values
        .iter()
        .map(|point| [point.x_iu, point.y_iu])
        .collect()
}

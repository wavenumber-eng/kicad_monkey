use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicLocalNetLimits, SchematicSubpartSettings,
    SourceBundle, SourceBundleErrorKind, SourceBundleLimits, build_schematic_occurrence_nets,
    build_schematic_occurrence_nets_with_settings,
};

#[test]
fn local_names_escape_scope_and_materialize_sorted_terminals() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Pair"
              (symbol "Demo:Pair_1_1"
                (pin passive line (at 0 0 0) (name "~") (number "2"))
                (pin passive line (at 10 0 0) (name "~") (number "1")))))
          (wire (pts (xy 0 0) (xy 20 0)) (uuid wire))
          (label "A/B
C" (at 5 0 0) (uuid local))
          (symbol (lib_id "Demo:Pair") (lib_name "Demo:Pair")
            (at 0 0 0) (uuid z-symbol)
            (property "Reference" "Z1") (property "Value" "Pair"))
          (symbol (lib_id "Demo:Pair") (lib_name "Demo:Pair")
            (at 10 0 0) (uuid a-symbol)
            (property "Reference" "A1") (property "Value" "Pair")))"#,
    );
    let nets = build_schematic_occurrence_nets(&index, 1, 7, Default::default())
        .expect("local net materialization");
    let signal = nets
        .iter()
        .find(|net| net.name == "/A{slash}BC")
        .expect("escaped local-label net");
    assert_eq!(signal.code, 7);
    assert!(!signal.auto_named);
    assert_eq!(
        signal
            .terminals
            .iter()
            .map(|terminal| (terminal.designator.as_str(), terminal.pin.as_str()))
            .collect::<Vec<_>>(),
        [("A1", "1"), ("Z1", "2")]
    );
    assert!(
        signal
            .terminals
            .iter()
            .all(|terminal| terminal.sheet_path == "/")
    );
}

#[test]
fn auto_names_use_duplicate_pin_names_units_and_source_pin_identity() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Multi"
              (symbol "Demo:Multi_1_1"
                (pin bidirectional line (at 0 0 0) (name "IO") (number "1"))
                (pin bidirectional line (at 10 0 0) (name "IO") (number "2")))
              (symbol "Demo:Multi_2_1"
                (pin bidirectional line (at 0 0 0) (name "ALT") (number "3")))))
          (symbol (lib_id "Demo:Multi") (lib_name "Demo:Multi")
            (at 0 0 0) (unit 1) (uuid placed)
            (property "Reference" "U1") (property "Value" "Multi")
            (pin "1" (uuid pin-one))))"#,
    );
    let limits = SchematicLocalNetLimits::default();
    let nets = build_schematic_occurrence_nets_with_settings(
        &index,
        1,
        1,
        SchematicSubpartSettings {
            first_id: u32::from(b'A'),
            separator: u32::from(b'-'),
        },
        limits,
    )
    .expect("unit-aware local nets");
    let by_name = nets
        .iter()
        .map(|net| (net.name.as_str(), net))
        .collect::<std::collections::HashMap<_, _>>();
    let first = by_name
        .get("Net-(U1-A-IO-Pad1)")
        .expect("first duplicate-name pin");
    let second = by_name
        .get("Net-(U1-A-IO-Pad2)")
        .expect("second duplicate-name pin");
    assert_eq!(first.terminals[0].source_pin_id, "pin-one");
    assert_eq!(first.terminals[0].svg_id, "pin-one");
    assert!(second.terminals[0].source_pin_id.is_empty());
    assert_eq!(second.terminals[0].svg_id, "placed__pin__2");
}

#[test]
fn isolated_and_named_multi_pin_auto_names_follow_kicad_quality_rules() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Named"
              (symbol "Demo:Named_1_1"
                (pin bidirectional line (at 0 0 0) (name "VREF") (number "1"))
                (pin bidirectional line (at 10 0 0) (name "IO") (number "2"))))
            (symbol "Demo:Passive"
              (symbol "Demo:Passive_1_1"
                (pin passive line (at 0 0 0) (name "~") (number "1")))))
          (symbol (lib_id "Demo:Named") (lib_name "Demo:Named")
            (at 0 0 0) (uuid named)
            (property "Reference" "U1") (property "Value" "Named"))
          (symbol (lib_id "Demo:Passive") (lib_name "Demo:Passive")
            (at 20 0 0) (uuid passive)
            (property "Reference" "R1") (property "Value" "Passive")))"#,
    );
    let names = build_schematic_occurrence_nets(&index, 1, 1, Default::default())
        .expect("auto names")
        .into_iter()
        .map(|net| net.name)
        .collect::<std::collections::HashSet<_>>();
    assert!(names.contains("Net-(U1-VREF)"));
    assert!(names.contains("Net-(U1-IO)"));
    assert!(names.contains("unconnected-(R1-Pad1)"));

    let error = build_schematic_occurrence_nets(&index, 1, u64::MAX, Default::default())
        .expect_err("a later local net code must not wrap");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("net code"));
}

#[test]
fn local_net_limits_accept_exact_output_and_fail_independently() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:One"
              (symbol "Demo:One_1_1"
                (pin passive line (at 0 0 0) (name "P") (number "1")))))
          (global_label "SIG" (shape input) (at 0 0 0) (uuid label))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One")
            (at 0 0 0) (uuid placed)
            (property "Reference" "U1") (property "Value" "One")
            (pin "1" (uuid pin-one))))"#,
    );
    let baseline = build_schematic_occurrence_nets(&index, 1, 1, Default::default())
        .expect("baseline local net");
    assert_eq!(baseline.len(), 1);
    assert_eq!(baseline[0].terminals.len(), 1);
    let name_bytes = baseline[0].name.len();
    let retained_bytes = name_bytes
        + baseline[0]
            .terminals
            .iter()
            .map(|terminal| {
                terminal.designator.len()
                    + terminal.pin.len()
                    + terminal.pin_name.len()
                    + terminal.pin_type.len()
                    + terminal.sheet_path.len()
                    + terminal.source_pin_id.len()
                    + terminal.svg_id.len()
            })
            .sum::<usize>();
    let exact = SchematicLocalNetLimits {
        max_nets: 1,
        max_terminals: 1,
        max_name_bytes: name_bytes,
        max_retained_string_bytes: retained_bytes,
        ..SchematicLocalNetLimits::default()
    };
    build_schematic_occurrence_nets(&index, 1, 1, exact)
        .expect("simultaneous exact local net limits");
    for (limits, message) in [
        (
            SchematicLocalNetLimits {
                max_nets: 0,
                ..exact
            },
            "net count",
        ),
        (
            SchematicLocalNetLimits {
                max_terminals: 0,
                ..exact
            },
            "terminal count",
        ),
        (
            SchematicLocalNetLimits {
                max_name_bytes: name_bytes - 1,
                ..exact
            },
            "name bytes",
        ),
        (
            SchematicLocalNetLimits {
                max_retained_string_bytes: retained_bytes - 1,
                ..exact
            },
            "retained string bytes",
        ),
    ] {
        let error = build_schematic_occurrence_nets(&index, 1, 1, limits)
            .expect_err("independent local net limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(message), "{}", error.message);
    }
}

#[test]
fn duplicate_sheet_pin_names_receive_stable_kicad_suffixes() {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (uuid root)
      (sheet (uuid first) (property "Sheetfile" "child.kicad_sch")
        (pin "OUT" output (at 0 0 0) (uuid first-pin)))
      (sheet (uuid second) (property "Sheetfile" "child.kicad_sch")
        (pin "OUT" output (at 10 0 0) (uuid second-pin))))"#
        .to_vec();
    let child = b"(kicad_sch (uuid child))".to_vec();
    let sources = vec![
        descriptor("design/demo.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        descriptor("design/child.kicad_sch", SourceKind::Schematic, 2, &child),
    ];
    let bundle = SourceBundle::from_manifest(
        SourceBundleManifestA0 {
            project_path: Some("design/demo.kicad_pro".to_owned()),
            root_schematic_path: "design/root.kicad_sch".to_owned(),
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources,
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        },
        vec![project, root, child],
        SourceBundleLimits::default(),
    )
    .expect("sheet-pin source bundle");
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("sheet-pin index");
    let exact = SchematicLocalNetLimits {
        max_nets: 2,
        max_terminals: 0,
        max_name_bytes: 6,
        max_retained_string_bytes: 10,
        ..SchematicLocalNetLimits::default()
    };
    let names = build_schematic_occurrence_nets(&index, 1, 1, exact)
        .expect("sheet-pin nets")
        .into_iter()
        .map(|net| net.name)
        .collect::<Vec<_>>();
    assert_eq!(names, ["/OUT", "/OUT_1"]);
    let error = build_schematic_occurrence_nets(
        &index,
        1,
        1,
        SchematicLocalNetLimits {
            max_retained_string_bytes: 9,
            ..exact
        },
    )
    .expect_err("one byte below the retained suffixed names must fail");
    assert!(error.message.contains("retained string bytes"));
}

#[test]
fn losing_auto_name_candidates_are_transient_and_code_ranges_preflight() {
    let index = index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Pair"
              (symbol "Demo:Pair_1_1"
                (pin bidirectional line (at 0 0 0) (name "A") (number "1"))
                (pin bidirectional line (at 10 0 0) (name "ZZZZZZZZZZZZZZZZ") (number "2")))))
          (wire (pts (xy 0 0) (xy 10 0)) (uuid joined))
          (global_label "OTHER" (shape input) (at 20 0 0) (uuid other))
          (symbol (lib_id "Demo:Pair") (lib_name "Demo:Pair")
            (at 0 0 0) (uuid placed)
            (property "Reference" "U1") (property "Value" "Pair")))"#,
    );
    let baseline = build_schematic_occurrence_nets(&index, 1, 1, Default::default())
        .expect("baseline candidate selection");
    assert_eq!(baseline.len(), 2);
    assert!(baseline.iter().any(|net| net.name == "Net-(U1-A)"));
    let retained_bytes = baseline
        .iter()
        .map(|net| {
            net.name.len()
                + net
                    .terminals
                    .iter()
                    .map(|terminal| {
                        terminal.designator.len()
                            + terminal.pin.len()
                            + terminal.pin_name.len()
                            + terminal.pin_type.len()
                            + terminal.sheet_path.len()
                            + terminal.source_pin_id.len()
                            + terminal.svg_id.len()
                    })
                    .sum::<usize>()
        })
        .sum::<usize>();
    build_schematic_occurrence_nets(
        &index,
        1,
        1,
        SchematicLocalNetLimits {
            max_name_bytes: 64,
            max_retained_string_bytes: retained_bytes,
            ..SchematicLocalNetLimits::default()
        },
    )
    .expect("losing candidate bytes are not retained output");

    let overflow = build_schematic_occurrence_nets(
        &index,
        1,
        u64::MAX,
        SchematicLocalNetLimits {
            max_name_bytes: 0,
            ..SchematicLocalNetLimits::default()
        },
    )
    .expect_err("code range must fail before name materialization");
    assert!(
        overflow.message.contains("net code"),
        "{}",
        overflow.message
    );
    let count_first = build_schematic_occurrence_nets(
        &index,
        1,
        u64::MAX,
        SchematicLocalNetLimits {
            max_nets: 1,
            max_name_bytes: 0,
            ..SchematicLocalNetLimits::default()
        },
    )
    .expect_err("shape limits must precede code and name validation");
    assert!(
        count_first.message.contains("net count"),
        "{}",
        count_first.message
    );
}

fn index(root: &[u8]) -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let root = root.to_vec();
    let sources = vec![
        descriptor("design/demo.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
    ];
    let bundle = SourceBundle::from_manifest(
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
    .expect("source bundle");
    SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("schematic bundle index")
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

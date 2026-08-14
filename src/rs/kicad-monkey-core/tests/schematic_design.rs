use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicDesignNetLimits, SourceBundle,
    SourceBundleErrorKind, SourceBundleLimits, build_schematic_scalar_design_nets,
};

#[test]
fn scalar_design_merge_binds_hierarchy_globals_and_global_power_only() {
    let index = hierarchy_index();
    let design = build_schematic_scalar_design_nets(&index, 1, Default::default())
        .expect("scalar design nets");
    assert_hierarchy_bindings(&design);
    assert_scalar_nets(&design);
}

fn assert_hierarchy_bindings(design: &kicad_monkey_core::SchematicScalarDesignNetlist) {
    assert_eq!(design.hierarchy_bindings.len(), 2);
    let resolved = design
        .hierarchy_bindings
        .iter()
        .find(|binding| binding.sheet_pin_name == "SIG")
        .expect("resolved hierarchy binding");
    assert!(resolved.is_resolved());
    assert_eq!(resolved.parent_occurrence_index, 1);
    assert_eq!(resolved.child_occurrence_index, 2);
    assert_eq!(resolved.sheet_pin_uuid, "sheet-sig");
    assert_eq!(
        resolved.hierarchical_label_uuid.as_deref(),
        Some("child-sig")
    );
    assert!(
        !design
            .hierarchy_bindings
            .iter()
            .find(|binding| binding.sheet_pin_name == "MISS")
            .expect("unresolved hierarchy binding")
            .is_resolved()
    );
}

fn assert_scalar_nets(design: &kicad_monkey_core::SchematicScalarDesignNetlist) {
    let by_name = design.nets.iter().fold(
        std::collections::HashMap::<&str, Vec<_>>::new(),
        |mut out, net| {
            out.entry(&net.name).or_default().push(net);
            out
        },
    );
    assert_eq!(terminal_refs(by_name["/Child/SIG"][0]), ["C0.1", "R0.1"]);
    assert_eq!(terminal_refs(by_name["G"][0]), ["C1.1", "R1.1"]);
    assert_eq!(terminal_refs(by_name["VCC"][0]), ["C2.1", "R2.1"]);
    assert_eq!(
        by_name["LOCAL"].len(),
        2,
        "local power stays per occurrence"
    );
    assert_eq!(terminal_refs(by_name["/MISS"][0]), ["R4.1"]);
    assert_eq!(terminal_refs(by_name["/Child/ORPHAN"][0]), ["C4.1"]);
    assert_eq!(
        design.nets.iter().map(|net| net.code).collect::<Vec<_>>(),
        (1..=design.nets.len() as u64).collect::<Vec<_>>()
    );
    assert!(
        design
            .nets
            .iter()
            .flat_map(|net| &net.terminals)
            .filter(|terminal| terminal.designator.starts_with('C'))
            .all(|terminal| {
                terminal.occurrence_index == 2 && terminal.sheet_path == "/child-sheet/"
            })
    );
}

#[test]
fn scalar_design_limits_fail_before_codes_names_and_output_growth() {
    let index = hierarchy_index();
    let baseline = build_schematic_scalar_design_nets(&index, 1, Default::default())
        .expect("baseline scalar design");
    let exact = exact_design_limits(&baseline);
    build_schematic_scalar_design_nets(&index, 1, exact).expect("simultaneous exact design limits");
    for (limits, message) in design_limit_failures(exact) {
        let error = build_schematic_scalar_design_nets(&index, 1, limits)
            .expect_err("one-under design limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(message), "{}", error.message);
    }
    let code_error = build_schematic_scalar_design_nets(
        &index,
        u64::MAX,
        SchematicDesignNetLimits {
            max_name_bytes: 0,
            ..SchematicDesignNetLimits::default()
        },
    )
    .expect_err("code range is checked before naming");
    assert!(code_error.message.contains("net code"));
}

fn design_limit_failures(
    exact: SchematicDesignNetLimits,
) -> Vec<(SchematicDesignNetLimits, &'static str)> {
    vec![
        (
            SchematicDesignNetLimits {
                max_subgraphs: 0,
                ..exact
            },
            "subgraph count",
        ),
        (
            SchematicDesignNetLimits {
                max_indexed_coords: 0,
                ..exact
            },
            "coordinate count",
        ),
        (
            SchematicDesignNetLimits {
                max_hierarchy_bindings: 0,
                ..exact
            },
            "binding count",
        ),
        (
            SchematicDesignNetLimits {
                max_nets: exact.max_nets - 1,
                ..exact
            },
            "net count",
        ),
        (
            SchematicDesignNetLimits {
                max_net_members: exact.max_net_members - 1,
                ..exact
            },
            "member count",
        ),
        (
            SchematicDesignNetLimits {
                max_terminals: exact.max_terminals - 1,
                ..exact
            },
            "terminal count",
        ),
        (
            SchematicDesignNetLimits {
                max_union_work: 0,
                ..exact
            },
            "union work",
        ),
        (
            SchematicDesignNetLimits {
                max_merge_keys: 0,
                ..exact
            },
            "merge-key count",
        ),
        (
            SchematicDesignNetLimits {
                max_drivers_per_net: 1,
                ..exact
            },
            "driver count",
        ),
        (
            SchematicDesignNetLimits {
                max_work_string_bytes: 0,
                ..exact
            },
            "work string bytes",
        ),
        (
            SchematicDesignNetLimits {
                max_name_bytes: 0,
                ..exact
            },
            "name bytes",
        ),
        (
            SchematicDesignNetLimits {
                max_retained_string_bytes: exact.max_retained_string_bytes - 1,
                ..exact
            },
            "retained string bytes",
        ),
    ]
}

fn exact_design_limits(
    baseline: &kicad_monkey_core::SchematicScalarDesignNetlist,
) -> SchematicDesignNetLimits {
    let terminal_count = baseline
        .nets
        .iter()
        .map(|net| net.terminals.len())
        .sum::<usize>();
    let member_count = baseline
        .nets
        .iter()
        .map(|net| net.members.len())
        .sum::<usize>();
    let retained = baseline
        .hierarchy_bindings
        .iter()
        .map(|binding| {
            binding.sheet_pin_name.len()
                + binding.sheet_pin_uuid.len()
                + binding
                    .hierarchical_label_uuid
                    .as_deref()
                    .map_or(0, str::len)
        })
        .sum::<usize>()
        + baseline
            .nets
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
    SchematicDesignNetLimits {
        max_nets: baseline.nets.len(),
        max_net_members: member_count,
        max_terminals: terminal_count,
        max_retained_string_bytes: retained,
        ..SchematicDesignNetLimits::default()
    }
}

fn terminal_refs(net: &kicad_monkey_core::SchematicDesignNet) -> Vec<String> {
    net.terminals
        .iter()
        .map(|terminal| format!("{}.{}", terminal.designator, terminal.pin))
        .collect()
}

fn hierarchy_index() -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let root = sheet_source("R", true);
    let child = sheet_source("C", false);
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
    .expect("hierarchy source bundle");
    SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default()).expect("hierarchy index")
}

fn sheet_source(prefix: &str, root: bool) -> Vec<u8> {
    let interfaces = if root {
        r#"(sheet (uuid child-sheet)
              (property "Sheetname" "Child")
              (property "Sheetfile" "child.kicad_sch")
              (pin "SIG" input (at 0 0 0) (uuid sheet-sig))
              (pin "MISS" input (at 40 0 0) (uuid sheet-miss)))"#
    } else {
        r#"(hierarchical_label "SIG" (shape output) (at 0 0 0) (uuid child-sig))
            (hierarchical_label "ORPHAN" (shape output) (at 40 0 0) (uuid child-orphan))"#
    };
    format!(
        r##"(kicad_sch
          (uuid {prefix}-root)
          (lib_symbols
            (symbol "Demo:One"
              (symbol "Demo:One_1_1"
                (pin bidirectional line (at 0 0 0) (name "P") (number "1"))))
            (symbol "power:Global" (power)
              (symbol "power:Global_1_1"
                (pin power_in line (at 0 0 0) (name "VCC") (number "1"))))
            (symbol "power:Local" (power local)
              (symbol "power:Local_1_1"
                (pin power_in line (at 0 0 0) (name "LOCAL") (number "1")))))
          {interfaces}
          (global_label "G" (shape input) (at 10 0 0) (uuid {prefix}-global))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid {prefix}0)
            (property "Reference" "{prefix}0") (property "Value" "One"))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 10 0 0) (uuid {prefix}1)
            (property "Reference" "{prefix}1") (property "Value" "One"))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 20 0 0) (uuid {prefix}2)
            (property "Reference" "{prefix}2") (property "Value" "One"))
          (symbol (lib_id "power:Global") (lib_name "power:Global") (at 20 0 0) (uuid {prefix}-pg)
            (property "Reference" "#PWR-{prefix}-G") (property "Value" "VCC"))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 30 0 0) (uuid {prefix}3)
            (property "Reference" "{prefix}3") (property "Value" "One"))
          (symbol (lib_id "power:Local") (lib_name "power:Local") (at 30 0 0) (uuid {prefix}-pl)
            (property "Reference" "#PWR-{prefix}-L") (property "Value" "LOCAL"))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 40 0 0) (uuid {prefix}4)
            (property "Reference" "{prefix}4") (property "Value" "One")))"##
    )
    .into_bytes()
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

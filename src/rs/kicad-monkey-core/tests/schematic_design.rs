use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicDesignNetLimits, SchematicLabelDriver,
    SchematicPinDriver, SourceBundle, SourceBundleErrorKind, SourceBundleLimits,
    build_schematic_occurrence_subgraphs, build_schematic_scalar_design_nets,
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

#[test]
fn wide_hierarchy_target_index_has_exact_count_and_string_limits() {
    let sheet_count = 32;
    let index = wide_hierarchy_index(sheet_count);
    let (target_count, target_bytes) = exact_target_index_shape(&index);
    assert_eq!(target_count, sheet_count);

    let exact = SchematicDesignNetLimits {
        max_sheet_pin_targets: target_count,
        max_target_index_bytes: target_bytes,
        ..SchematicDesignNetLimits::default()
    };
    let design = build_schematic_scalar_design_nets(&index, 1, exact)
        .expect("wide hierarchy at exact target limits");
    assert_eq!(design.nets.len(), sheet_count);
    for child in 0..sheet_count {
        assert!(
            design
                .nets
                .iter()
                .any(|net| net.name == format!("/Child{child}/PIN")),
            "missing indexed off-board target for Child{child}"
        );
    }

    for (limits, message) in [
        (
            SchematicDesignNetLimits {
                max_sheet_pin_targets: target_count - 1,
                ..exact
            },
            "target count",
        ),
        (
            SchematicDesignNetLimits {
                max_target_index_bytes: target_bytes - 1,
                ..exact
            },
            "target string bytes",
        ),
    ] {
        let error = build_schematic_scalar_design_nets(&index, 1, limits)
            .expect_err("one-under target-index limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains(message), "{}", error.message);
    }
}

#[test]
fn merged_driver_clone_bytes_are_preflighted_for_long_fan_in() {
    let symbol_count = 24;
    let index = fan_in_index(symbol_count, 256);
    let subgraphs = build_schematic_occurrence_subgraphs(&index, 1, Default::default())
        .expect("fan-in occurrence subgraphs");
    let merged = subgraphs
        .iter()
        .find(|subgraph| subgraph.pin_drivers.len() == symbol_count)
        .expect("fan-in merged subgraph");
    assert_eq!(merged.label_drivers.len(), 1);
    let exact_bytes = exact_merged_driver_bytes(merged);

    let exact = SchematicDesignNetLimits {
        max_merged_driver_bytes: exact_bytes,
        ..SchematicDesignNetLimits::default()
    };
    let design = build_schematic_scalar_design_nets(&index, 1, exact)
        .expect("fan-in at exact merged-driver byte limit");
    assert_eq!(design.nets.len(), 1);
    assert_eq!(design.nets[0].name, "/NET");
    assert_eq!(design.nets[0].terminals.len(), symbol_count);

    let error = build_schematic_scalar_design_nets(
        &index,
        1,
        SchematicDesignNetLimits {
            max_merged_driver_bytes: exact_bytes - 1,
            ..exact
        },
    )
    .expect_err("one-under merged-driver byte limit");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("merged design driver bytes"));
}

fn exact_target_index_shape(index: &SchematicBundleIndex) -> (usize, usize) {
    index
        .occurrences()
        .filter_map(|child| {
            let parent_index = child.parent_index?;
            let sheet_index = child.parent_sheet_index?;
            let parent = index.occurrence(parent_index)?;
            let sheet = index
                .definition(&parent.source_path)?
                .sheets
                .get(sheet_index)?;
            (!sheet.on_board).then_some(
                sheet
                    .pins
                    .iter()
                    .map(|pin| {
                        let key_bytes = if pin.uuid.is_empty() {
                            pin.name.len()
                        } else {
                            pin.uuid.len()
                        };
                        (1_usize, key_bytes + child.human_address.len())
                    })
                    .fold((0_usize, 0_usize), |left, right| {
                        (left.0 + right.0, left.1 + right.1)
                    }),
            )
        })
        .fold((0_usize, 0_usize), |left, right| {
            (left.0 + right.0, left.1 + right.1)
        })
}

fn exact_merged_driver_bytes(subgraph: &kicad_monkey_core::SchematicWireSubgraph) -> usize {
    let pin_strings = subgraph
        .pin_drivers
        .iter()
        .map(|pin| {
            pin.symbol_uuid.len()
                + pin.reference.len()
                + pin.pin_number.len()
                + pin.pin_name.len()
                + pin.electrical_type.len()
                + pin.power_value.len()
                + pin.designator_with_unit.len()
                + pin.source_pin_uuid.len()
                + pin.pin_svg_id.len()
        })
        .sum::<usize>();
    let label_strings = subgraph
        .label_drivers
        .iter()
        .map(|label| label.text.len() + label.shape.len() + label.source_uuid.len())
        .sum::<usize>();
    let selected_choice_strings = "/NET".len() + "/".len() + "NET".len();
    subgraph.pin_drivers.len() * std::mem::size_of::<SchematicPinDriver>()
        + subgraph.label_drivers.len() * std::mem::size_of::<SchematicLabelDriver>()
        + pin_strings
        + label_strings
        + selected_choice_strings
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

fn wide_hierarchy_index(sheet_count: usize) -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let child = b"(kicad_sch (uuid repeated-child))".to_vec();
    let mut sheets = String::new();
    let mut symbols = String::new();
    for index in 0..sheet_count {
        let x = index * 10;
        let pin_uuid = if index == 0 {
            String::new()
        } else {
            format!(" (uuid pin-{index})")
        };
        sheets.push_str(&format!(
            r#"(sheet (uuid sheet-{index})
                 (property "Sheetname" "Child{index}")
                 (property "Sheetfile" "child.kicad_sch")
                 (on_board no)
                 (pin "PIN" input (at {x} 0 0){pin_uuid}))"#
        ));
        symbols.push_str(&format!(
            r#"(symbol (lib_id "Demo:One") (lib_name "Demo:One")
                 (at {x} 0 0) (uuid symbol-{index})
                 (property "Reference" "R{index}")
                 (property "Value" "One"))"#
        ));
    }
    let root = format!(
        r#"(kicad_sch
              (uuid wide-root)
              (lib_symbols
                (symbol "Demo:One"
                  (symbol "Demo:One_1_1"
                    (pin bidirectional line (at 0 0 0) (name "P") (number "1")))))
              {sheets}
              {symbols})"#
    )
    .into_bytes();
    bundle_index(
        "wide",
        vec![
            descriptor("wide/demo.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("wide/root.kicad_sch", SourceKind::Schematic, 1, &root),
            descriptor("wide/child.kicad_sch", SourceKind::Schematic, 2, &child),
        ],
        vec![project, root, child],
    )
}

fn fan_in_index(symbol_count: usize, reference_bytes: usize) -> SchematicBundleIndex {
    let project = b"{}".to_vec();
    let mut symbols = String::new();
    for index in 0..symbol_count {
        let suffix = index.to_string();
        let reference = format!("{}{}", "R".repeat(reference_bytes - suffix.len()), suffix);
        symbols.push_str(&format!(
            r#"(symbol (lib_id "Demo:One") (lib_name "Demo:One")
                 (at 0 0 0) (uuid symbol-{index})
                 (property "Reference" "{reference}")
                 (property "Value" "One"))"#
        ));
    }
    let root = format!(
        r#"(kicad_sch
              (uuid fan-in-root)
              (lib_symbols
                (symbol "Demo:One"
                  (symbol "Demo:One_1_1"
                    (pin bidirectional line (at 0 0 0) (name "LONG_PIN_NAME") (number "1")))))
              (label "NET" (at 0 0 0) (uuid fan-in-label))
              {symbols})"#
    )
    .into_bytes();
    bundle_index(
        "fan-in",
        vec![
            descriptor("fan-in/demo.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("fan-in/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ],
        vec![project, root],
    )
}

fn bundle_index(
    project_name: &str,
    sources: Vec<SourceBundleSource>,
    buffers: Vec<Vec<u8>>,
) -> SchematicBundleIndex {
    let bundle = SourceBundle::from_manifest(
        SourceBundleManifestA0 {
            project_path: Some(format!("{project_name}/demo.kicad_pro")),
            root_schematic_path: format!("{project_name}/root.kicad_sch"),
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources,
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        },
        buffers,
        SourceBundleLimits::default(),
    )
    .expect("synthetic source bundle");
    SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("synthetic schematic index")
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

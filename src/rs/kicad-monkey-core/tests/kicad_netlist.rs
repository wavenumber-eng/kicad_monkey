use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    KiCadNetlist, KiCadNetlistLimits, ProjectDocument, ProjectLimits, SchematicBundleIndex,
    SchematicBundleLimits, SourceBundle, SourceBundleErrorKind, SourceBundleLimits,
    build_kicad_netlist, emit_kicad_netlist,
};

#[test]
fn empty_version_e_emit_matches_python_exactly_and_fails_closed() {
    let netlist = KiCadNetlist {
        nets: Vec::new(),
        components: Vec::new(),
        libparts: Vec::new(),
        libraries: Vec::new(),
        net_classes: Vec::new(),
        sheets: Vec::new(),
    };
    let expected = "(export\n  (version \"E\"\n  )\n  (design\n    (source \"\"\n    )\n    (date \"\"\n    )\n    (tool \"kicad_monkey\"\n    )\n  )\n  (components\n  )\n  (libparts\n  )\n  (libraries\n  )\n  (nets\n  )\n)\n";
    let output = emit_kicad_netlist(&netlist, "", "", "kicad_monkey", expected.len())
        .expect("exact output limit");
    assert_eq!(output, expected);
    assert_eq!(
        emit_kicad_netlist(&netlist, "", "", "kicad_monkey", expected.len() - 1)
            .expect_err("one under")
            .kind,
        SourceBundleErrorKind::ResourceLimit
    );
}

#[test]
fn native_materializer_emits_components_libparts_sheets_and_resolved_nets() {
    let index = structural_index();
    let netlist =
        build_kicad_netlist(&index, None, KiCadNetlistLimits::default()).expect("native netlist");
    assert_structural_netlist(&netlist);
    assert!(netlist.net_classes.is_empty());
    assert!(netlist.nets.iter().all(|net| net.net_class.is_empty()));
    let output = emit_kicad_netlist(&netlist, "root.kicad_sch", "", "kicad_monkey", 1_000_000)
        .expect("version-E output");
    let reparsed = kicad_monkey_core::sexpr::parse(&output).expect("valid S-expression");
    assert!(matches!(reparsed, kicad_monkey_core::sexpr::Sexp::List(_)));
}

#[test]
fn multi_unit_tstamps_preserve_nonprimary_source_order_then_lowest_uuid_primary() {
    let index = single_index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:Multi"
              (symbol "Demo:Multi_1_1")
              (symbol "Demo:Multi_2_1")
              (symbol "Demo:Multi_3_1")
              (symbol "Demo:Multi_4_1")))
          (symbol (lib_id "Demo:Multi") (lib_name "Demo:Multi")
            (at 0 0 0) (unit 1) (uuid 7332c9d0-2326-43ef-8057-b98c9b8e133c)
            (property "Reference" "U1") (property "Value" "Multi"))
          (symbol (lib_id "Demo:Multi") (lib_name "Demo:Multi")
            (at 10 0 0) (unit 3) (uuid 82dbe4d7-1c62-492f-b41d-8a0e7e250760)
            (property "Reference" "U1") (property "Value" "Multi"))
          (symbol (lib_id "Demo:Multi") (lib_name "Demo:Multi")
            (at 20 0 0) (unit 4) (uuid cae74693-d5c5-481a-9faa-97917b358de7)
            (property "Reference" "U1") (property "Value" "Multi"))
          (symbol (lib_id "Demo:Multi") (lib_name "Demo:Multi")
            (at 30 0 0) (unit 2) (uuid d31054c5-0fcb-49eb-903b-e65f43072f2d)
            (property "Reference" "U1") (property "Value" "Multi")))"#,
    );
    let netlist =
        build_kicad_netlist(&index, None, KiCadNetlistLimits::default()).expect("native netlist");
    assert_eq!(netlist.components.len(), 1);
    assert_eq!(
        netlist.components[0].instance_uuids,
        [
            "82dbe4d7-1c62-492f-b41d-8a0e7e250760",
            "cae74693-d5c5-481a-9faa-97917b358de7",
            "d31054c5-0fcb-49eb-903b-e65f43072f2d",
            "7332c9d0-2326-43ef-8057-b98c9b8e133c",
        ]
    );
}

fn assert_structural_netlist(netlist: &KiCadNetlist) {
    assert_eq!(netlist.sheets.len(), 2);
    assert_eq!(
        netlist
            .components
            .iter()
            .map(|component| component.reference.as_str())
            .collect::<Vec<_>>(),
        ["R1", "C1"]
    );
    assert_eq!(netlist.libparts.len(), 1);
    assert_eq!(netlist.libparts[0].lib, "Demo");
    assert_eq!(netlist.libparts[0].part, "One");
    assert_eq!(netlist.libparts[0].description, "Synthetic library part");
    assert_eq!(netlist.components[0].value, "first-duplicate-uuid-wins");
    assert_eq!(netlist.sheets[0].title, "Structural demo");
    assert_eq!(netlist.sheets[0].company, "Wavenumber");
    assert_eq!(netlist.sheets[0].revision, "A");
    assert_eq!(netlist.sheets[0].date, "2026-08-14");
    assert!(netlist.nets.iter().all(|net| net.code > 0));
    assert!(
        netlist
            .nets
            .iter()
            .flat_map(|net| &net.terminals)
            .any(|terminal| terminal.designator == "R1")
    );
}

#[test]
fn project_patterns_assign_the_first_declared_valid_net_class() {
    let project = r#"{
      "net_settings": {
        "classes": [{"name":""}, {"name":"Power"}, {"name":"Fallback"}],
        "netclass_patterns": [
          {"pattern":"*", "netclass":""},
          {"pattern":"*", "netclass":"missing"},
          {"pattern":"*", "netclass":"Power"},
          {"pattern":"*", "netclass":"Fallback"}
        ]
      }
    }"#;
    let document = ProjectDocument::parse(project.to_owned(), ProjectLimits::default())
        .expect("project settings");
    let netlist = build_kicad_netlist(
        &structural_index(),
        Some(document.view()),
        KiCadNetlistLimits::default(),
    )
    .expect("native netlist");
    assert!(!netlist.nets.is_empty());
    assert!(netlist.nets.iter().all(|net| net.net_class == "Power"));
}

#[test]
fn aggregate_wildcard_work_accepts_exact_and_rejects_one_under() {
    let index = single_index(
        br#"(kicad_sch
          (uuid root)
          (lib_symbols
            (symbol "Demo:One"
              (symbol "Demo:One_1_1"
                (pin passive line (at 0 0 0) (name "P") (number "1")))))
          (global_label "ALPHA" (shape input) (at 0 0 0) (uuid alpha))
          (global_label "BETA" (shape input) (at 10 0 0) (uuid beta))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One")
            (at 0 0 0) (uuid alpha-symbol)
            (property "Reference" "U1") (property "Value" "One"))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One")
            (at 10 0 0) (uuid beta-symbol)
            (property "Reference" "U2") (property "Value" "One")))"#,
    );
    let project = ProjectDocument::parse(
        r#"{
          "net_settings": {
            "classes": [{"name":"Default"}, {"name":"Signals"}],
            "netclass_patterns": [
              {"pattern":"NO*", "netclass":"Signals"},
              {"pattern":"A*", "netclass":"Signals"}
            ]
          }
        }"#
        .to_owned(),
        ProjectLimits::default(),
    )
    .expect("project patterns");
    // ALPHA charges 26 + 19 units; BETA charges 22 + 16 units.
    let exact_work = 83;
    build_kicad_netlist(
        &index,
        Some(project.view()),
        KiCadNetlistLimits {
            max_wildcard_match_work: exact_work,
            ..KiCadNetlistLimits::default()
        },
    )
    .expect("exact aggregate wildcard work");
    let error = build_kicad_netlist(
        &index,
        Some(project.view()),
        KiCadNetlistLimits {
            max_wildcard_match_work: exact_work - 1,
            ..KiCadNetlistLimits::default()
        },
    )
    .expect_err("one-under aggregate wildcard work");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("wildcard match work"));
}

#[test]
fn schematic_title_block_is_bounded_before_model_publication() {
    let error = structural_index_with_limits(SchematicBundleLimits {
        max_title_block_children_per_source: 3,
        ..SchematicBundleLimits::default()
    })
    .expect_err("four title-block children exceed the limit");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);

    let error = structural_index_with_limits(SchematicBundleLimits {
        max_library_properties_per_symbol: 0,
        ..SchematicBundleLimits::default()
    })
    .expect_err("library properties have an independent limit");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
}

#[test]
fn independent_materialization_limits_fail_before_publication() {
    let index = structural_index();
    for limits in [
        KiCadNetlistLimits {
            max_nets: 0,
            ..KiCadNetlistLimits::default()
        },
        KiCadNetlistLimits {
            max_components: 1,
            ..KiCadNetlistLimits::default()
        },
        KiCadNetlistLimits {
            max_component_candidates: 1,
            ..KiCadNetlistLimits::default()
        },
        KiCadNetlistLimits {
            max_libparts: 0,
            ..KiCadNetlistLimits::default()
        },
        KiCadNetlistLimits {
            max_sheets: 1,
            ..KiCadNetlistLimits::default()
        },
        KiCadNetlistLimits {
            max_retained_string_bytes: 1,
            ..KiCadNetlistLimits::default()
        },
    ] {
        assert_eq!(
            build_kicad_netlist(&index, None, limits)
                .expect_err("resource limit")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }
}

fn structural_index() -> SchematicBundleIndex {
    structural_index_with_limits(SchematicBundleLimits::default()).expect("index")
}

fn structural_index_with_limits(
    limits: SchematicBundleLimits,
) -> Result<SchematicBundleIndex, kicad_monkey_core::SourceBundleError> {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (uuid structural-root)
      (title_block
        (title "Structural demo")
        (date "2026-08-14")
        (rev "A")
        (company "Wavenumber"))
      (lib_symbols
        (symbol "Demo:One"
          (property "Description" "Synthetic library part")
          (symbol "Demo:One_1_1"
            (pin passive line (at 0 0 0) (name "P") (number "1")))))
      (sheet (uuid child-sheet)
        (property "Sheetname" "Child")
        (property "Sheetfile" "child.kicad_sch")
        (pin "SIG" input (at 20 0 180) (uuid sheet-pin)))
      (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid root-symbol)
        (property "Reference" "R1") (property "Value" "first-duplicate-uuid-wins"))
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
      (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid child-symbol)
        (property "Reference" "C1") (property "Value" "One")))"#
        .to_vec();
    let unused = br#"(kicad_sch
      (uuid unused)
      (lib_symbols
        (symbol "Demo:Unused"
          (symbol "Demo:Unused_1_1"
            (pin passive line (at 0 0 0) (name "P") (number "1"))))))"#
        .to_vec();
    let buffers = vec![project, root, child, unused];
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
            descriptor(
                "nested/unused.kicad_sch",
                SourceKind::Schematic,
                3,
                &buffers[3],
            ),
        ],
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    };
    let bundle = SourceBundle::from_manifest(manifest, buffers, SourceBundleLimits::default())?;
    SchematicBundleIndex::build(&bundle, limits)
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

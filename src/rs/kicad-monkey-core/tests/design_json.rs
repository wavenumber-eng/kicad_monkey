use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    KiCadDesignJsonLimits, KiCadDesignJsonPaths, KiCadNetlistLimits, ProjectLimits,
    SchematicBundleIndex, SchematicBundleLimits, SourceBundle, SourceBundleLimits,
    build_kicad_design_facts, build_kicad_design_json, build_kicad_design_json_with_limits,
};

#[test]
fn public_builder_validates_binding_and_enforces_exact_limits() {
    let (bundle, index) = design_input("One");
    let facts = build_kicad_design_facts(
        &index,
        &bundle,
        ProjectLimits::default(),
        Default::default(),
        KiCadNetlistLimits::default(),
    )
    .expect("structured design facts");
    let instances = facts.schematic_instances().expect("schematic instances");
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].sheet_name, "demo");
    assert_eq!(instances[0].sheet_path, "/");
    assert_eq!(instances[0].sheet_instance_path, "/root");
    assert_eq!(instances[0].sheet_number, instances[0].instance_index);
    assert_eq!(instances[0].document_id, "root");
    assert!(instances[0].is_top_level);
    let paths = KiCadDesignJsonPaths::default();
    let baseline =
        build_kicad_design_json(&index, &facts, &paths, None, true).expect("baseline design JSON");
    let output_bytes = serde_json::to_vec(&baseline).unwrap().len();

    build_kicad_design_json_with_limits(
        &index,
        &facts,
        &paths,
        None,
        true,
        KiCadDesignJsonLimits {
            max_output_bytes: output_bytes,
            ..KiCadDesignJsonLimits::default()
        },
    )
    .expect("exact limits");
    for limits in [
        KiCadDesignJsonLimits {
            max_derived_items: 0,
            max_output_bytes: output_bytes,
            ..KiCadDesignJsonLimits::default()
        },
        KiCadDesignJsonLimits {
            max_output_bytes: output_bytes - 1,
            ..KiCadDesignJsonLimits::default()
        },
    ] {
        assert!(
            build_kicad_design_json_with_limits(&index, &facts, &paths, None, true, limits)
                .is_err()
        );
    }

    let (stale_bundle, stale_index) = design_input("Stale");
    let error = build_kicad_design_json(&stale_index, &facts, &paths, None, true)
        .expect_err("same-path stale graph");
    assert!(error.to_string().contains("does not belong"));

    let error = build_kicad_design_facts(
        &index,
        &stale_bundle,
        ProjectLimits::default(),
        Default::default(),
        KiCadNetlistLimits::default(),
    )
    .expect_err("mismatched project and schematic bundle");
    assert!(error.to_string().contains("does not belong"));
}

fn design_input(value: &str) -> (SourceBundle, SchematicBundleIndex) {
    let project = format!(r#"{{"text_variables": {{"VALUE": "{value}"}}}}"#).into_bytes();
    let schematic = format!(
        r#"(kicad_sch
      (uuid root)
      (sheet_instances
        (path "/unmatched" (page "7"))
        (path "/root" (page "0")))
      (lib_symbols
        (symbol "Demo:One"
          (symbol "Demo:One_1_1"
            (pin passive line (at 0 0 0) (name "P") (number "1")))))
      (symbol (lib_id "Demo:One") (lib_name "Demo:One")
        (at 0 0 0) (uuid symbol)
        (property "Reference" "U1") (property "Value" "{value}")))"#
    )
    .into_bytes();
    let buffers = vec![project, schematic];
    let manifest = SourceBundleManifestA0 {
        project_path: Some("demo.kicad_pro".to_owned()),
        root_schematic_path: "demo.kicad_sch".to_owned(),
        schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
        sources: vec![
            descriptor("demo.kicad_pro", SourceKind::Project, 0, &buffers[0]),
            descriptor("demo.kicad_sch", SourceKind::Schematic, 1, &buffers[1]),
        ],
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    };
    let bundle = SourceBundle::from_manifest(manifest, buffers, SourceBundleLimits::default())
        .expect("source bundle");
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("schematic index");
    (bundle, index)
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

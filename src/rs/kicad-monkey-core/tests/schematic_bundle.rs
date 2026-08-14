use kicad_monkey_contracts::decode_source_bundle_manifest_a0;
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SourceBundle, SourceBundleErrorKind,
    SourceBundleLimits,
};

fn manifest(sources: Vec<SourceBundleSource>) -> SourceBundleManifestA0 {
    SourceBundleManifestA0 {
        project_path: Some("design/root.kicad_pro".to_owned()),
        root_schematic_path: "design/root.kicad_sch".to_owned(),
        schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
        sources,
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    }
}

fn descriptor(path: &str, kind: SourceKind, slot: u32, bytes: &[u8]) -> SourceBundleSource {
    SourceBundleSource {
        kind,
        path: path.to_owned(),
        slot: slot.into(),
        source_bytes: bytes.len().to_string().into(),
    }
}

fn hierarchy_bundle() -> SourceBundle {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (version 20250114)
      (generator eeschema)
      (generator_version "9.0")
      (uuid root-uuid)
      (sheet
        (uuid sheet-a)
        (property "Sheetname" "First")
        (property "Sheetfile" "sub/child.kicad_sch")
        (on_board no)
        (dnp yes))
      (sheet
        (uuid sheet-b)
        (property "Sheet name" "Second")
        (property "Sheet file" "sub/child.kicad_sch")))"#
        .to_vec();
    let child = br#"(kicad_sch
      (version 20250114)
      (uuid child-source)
      (sheet
        (uuid leaf-placement)
        (property "Sheetname" "Leaf")
        (property "Sheetfile" "../leaf.kicad_sch")
        (in_bom no)
        (exclude_from_sim yes)))"#
        .to_vec();
    let leaf = b"(kicad_sch (version 20250114) (uuid leaf-source))".to_vec();
    let sources = vec![
        descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        descriptor(
            "design/sub/child.kicad_sch",
            SourceKind::Schematic,
            2,
            &child,
        ),
        descriptor("design/leaf.kicad_sch", SourceKind::Schematic, 3, &leaf),
    ];
    SourceBundle::from_manifest(
        manifest(sources),
        vec![project, root, child, leaf],
        SourceBundleLimits::default(),
    )
    .expect("hierarchy bundle")
}

#[test]
fn shared_manifest_decodes_strictly_and_preserves_exact_byte_slots() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/source_bundle_a0_vectors.json"
    ))
    .expect("vectors");
    let manifest = decode_source_bundle_manifest_a0(
        &serde_json::to_vec(&vectors["manifest"]).expect("manifest JSON"),
    )
    .expect("strict manifest");
    let buffers = vectors["buffers_utf8"]
        .as_array()
        .expect("buffers")
        .iter()
        .map(|value| value.as_str().expect("buffer").as_bytes().to_vec())
        .collect::<Vec<_>>();
    let bundle = SourceBundle::from_manifest(manifest, buffers, SourceBundleLimits::default())
        .expect("bundle");
    assert_eq!(bundle.sources().len(), 2);
    assert_eq!(bundle.total_bytes(), 47);
    assert_eq!(bundle.project().expect("project").bytes(), b"{}");
    assert_eq!(
        bundle.root_schematic().bytes(),
        b"(kicad_sch (version 20250114) (uuid root-id))"
    );
}

#[test]
fn manifest_slots_paths_sizes_kinds_and_project_json_fail_closed() {
    let project = b"{}".to_vec();
    let root = b"(kicad_sch)".to_vec();
    let base = vec![
        descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
    ];

    let mut duplicate_slot = base.clone();
    duplicate_slot[1].slot = 0.into();
    assert_eq!(
        SourceBundle::from_manifest(
            manifest(duplicate_slot),
            vec![project.clone(), root.clone()],
            SourceBundleLimits::default(),
        )
        .expect_err("duplicate slot")
        .kind,
        SourceBundleErrorKind::Slot
    );

    let mut wrong_size = base.clone();
    wrong_size[1].source_bytes = "999".to_owned().into();
    assert_eq!(
        SourceBundle::from_manifest(
            manifest(wrong_size),
            vec![project.clone(), root.clone()],
            SourceBundleLimits::default(),
        )
        .expect_err("wrong byte count")
        .kind,
        SourceBundleErrorKind::Contract
    );

    let mut escaping = base.clone();
    escaping[1].path = "../root.kicad_sch".to_owned();
    assert_eq!(
        SourceBundle::from_manifest(
            manifest(escaping),
            vec![project.clone(), root.clone()],
            SourceBundleLimits::default(),
        )
        .expect_err("escaping path")
        .kind,
        SourceBundleErrorKind::Path
    );

    assert_eq!(
        SourceBundle::from_manifest(
            manifest(base),
            vec![b"[]".to_vec(), root],
            SourceBundleLimits::default(),
        )
        .expect_err("non-object project")
        .kind,
        SourceBundleErrorKind::Project
    );
}

#[test]
fn schematics_are_scanned_once_and_repeated_pages_realize_distinct_occurrences() {
    let bundle = hierarchy_bundle();
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("schematic hierarchy");
    assert_eq!(index.definitions().len(), 3);
    assert_eq!(index.occurrences().len(), 5);
    let occurrences = index.occurrences().collect::<Vec<_>>();
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.source_path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "design/root.kicad_sch",
            "design/sub/child.kicad_sch",
            "design/leaf.kicad_sch",
            "design/sub/child.kicad_sch",
            "design/leaf.kicad_sch",
        ]
    );
    assert_eq!(occurrences[1].parent_index, Some(1));
    assert_eq!(occurrences[2].parent_index, Some(2));
    assert_eq!(occurrences[3].parent_index, Some(1));
    assert_eq!(occurrences[4].parent_index, Some(4));
    assert_eq!(occurrences[1].occurrence_address, "/root-uuid/sheet-a");
    assert_eq!(occurrences[3].occurrence_address, "/root-uuid/sheet-b");
}

#[test]
fn hierarchy_occurrences_fold_parent_policy_without_mutating_definitions() {
    let bundle = hierarchy_bundle();
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("schematic hierarchy");
    let occurrences = index.occurrences().collect::<Vec<_>>();
    assert!(occurrences[1].effective_dnp);
    assert!(!occurrences[1].effective_on_board);
    assert!(!occurrences[2].effective_in_bom);
    assert!(occurrences[2].effective_exclude_from_sim);
    assert!(occurrences[2].effective_dnp);
    assert!(!occurrences[2].effective_on_board);
    assert!(!occurrences[4].effective_in_bom);
    assert!(occurrences[4].effective_exclude_from_sim);
    assert!(!occurrences[4].effective_dnp);
    assert!(occurrences[4].effective_on_board);
}

#[test]
fn hierarchy_missing_sources_cycles_and_occurrence_limits_fail_closed() {
    let bundle = hierarchy_bundle();
    let limits = SchematicBundleLimits {
        max_occurrences: 4,
        ..SchematicBundleLimits::default()
    };
    assert_eq!(
        SchematicBundleIndex::build(&bundle, limits)
            .expect_err("occurrence limit")
            .kind,
        SourceBundleErrorKind::ResourceLimit
    );

    let project = b"{}".to_vec();
    let missing_root = br#"(kicad_sch
      (sheet (uuid missing) (property "Sheetfile" "missing.kicad_sch")))"#
        .to_vec();
    let missing_bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor(
                "design/root.kicad_sch",
                SourceKind::Schematic,
                1,
                &missing_root,
            ),
        ]),
        vec![project.clone(), missing_root],
        SourceBundleLimits::default(),
    )
    .expect("missing-reference bundle boundary");
    assert_eq!(
        SchematicBundleIndex::build(&missing_bundle, SchematicBundleLimits::default())
            .expect_err("missing source")
            .kind,
        SourceBundleErrorKind::MissingSource
    );

    let cycle_root = br#"(kicad_sch
      (sheet (uuid child) (property "Sheetfile" "sub/child.kicad_sch")))"#
        .to_vec();
    let cycle_child = br#"(kicad_sch
      (sheet (uuid root) (property "Sheetfile" "../root.kicad_sch")))"#
        .to_vec();
    let cycle_bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor(
                "design/root.kicad_sch",
                SourceKind::Schematic,
                1,
                &cycle_root,
            ),
            descriptor(
                "design/sub/child.kicad_sch",
                SourceKind::Schematic,
                2,
                &cycle_child,
            ),
        ]),
        vec![project, cycle_root, cycle_child],
        SourceBundleLimits::default(),
    )
    .expect("cycle bundle boundary");
    assert_eq!(
        SchematicBundleIndex::build(&cycle_bundle, SchematicBundleLimits::default())
            .expect_err("hierarchy cycle")
            .kind,
        SourceBundleErrorKind::HierarchyCycle
    );
}

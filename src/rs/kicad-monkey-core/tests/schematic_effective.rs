use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SourceBundle, SourceBundleLimits,
};

#[test]
fn top_level_legacy_records_resolve_repeated_child_occurrences() {
    let root = br#"(kicad_sch
      (uuid top-source)
      (sheet (uuid sheet-a)
        (property "Sheetname" "First")
        (property "Sheetfile" "child.kicad_sch"))
      (sheet (uuid sheet-b)
        (property "Sheetname" "Second")
        (property "Sheetfile" "child.kicad_sch"))
      (symbol_instances
        (path "/sheet-a/child-symbol" (reference "U1") (unit 1))
        (path "/sheet-b/child-symbol" (reference "U2") (unit 2))))"#
        .to_vec();
    let child = br#"(kicad_sch
      (uuid child-source)
      (symbol (lib_id "Demo:Part") (uuid child-symbol) (unit 9)
        (property "Reference" "U") (property "Value" "part")))"#
        .to_vec();
    let index = SchematicBundleIndex::build(&bundle(root, child), SchematicBundleLimits::default())
        .expect("repeated child index");

    let first = index.effective_symbols(2, None).expect("first child");
    let second = index.effective_symbols(3, None).expect("second child");
    assert_eq!((first[0].reference.as_str(), first[0].unit), ("U1", 1));
    assert_eq!((second[0].reference.as_str(), second[0].unit), ("U2", 2));
}

#[test]
fn repeated_occurrence_resolution_uses_indexed_suffix_and_fallback_paths() {
    const OCCURRENCES: usize = 128;
    const INSTANCES: usize = 2_048;

    let mut sheets = String::new();
    for index in 0..OCCURRENCES {
        sheets.push_str(&format!(
            "(sheet (uuid sheet-{index}) (property \"Sheetname\" \"Page {index}\") \
             (property \"Sheetfile\" \"child.kicad_sch\"))"
        ));
    }
    let root = format!("(kicad_sch (uuid top-source) {sheets})").into_bytes();

    let mut paths = String::new();
    paths.push_str("(path \"/unrelated/first\" (reference \"FIRST\") (unit 1))");
    for index in 1..INSTANCES {
        paths.push_str(&format!(
            "(path \"/unrelated/{index}\" (reference \"R{index}\") (unit 1))"
        ));
    }
    paths.push_str(&format!(
        "(path \"/prefix/sheet-{}\" (reference \"LATE\") (unit 7))",
        OCCURRENCES - 1
    ));
    let child = format!(
        "(kicad_sch (uuid child-source) \
         (symbol (lib_id \"Demo:Part\") (uuid child-symbol) \
           (property \"Reference\" \"U\") \
           (instances (project \"root\" {paths}))))"
    )
    .into_bytes();
    let index = SchematicBundleIndex::build(&bundle(root, child), SchematicBundleLimits::default())
        .expect("indexed suffix bundle");

    for occurrence_index in 2..=OCCURRENCES {
        let symbols = index
            .effective_symbols(occurrence_index, None)
            .expect("fallback symbol");
        assert_eq!(symbols[0].reference, "FIRST");
    }
    let late = index
        .effective_symbols(OCCURRENCES + 1, None)
        .expect("late suffix symbol");
    assert_eq!((late[0].reference.as_str(), late[0].unit), ("LATE", 7));
}

#[test]
fn indexed_suffix_resolution_rejects_ambiguous_compatible_paths() {
    let root = br#"(kicad_sch
      (uuid top-source)
      (sheet (uuid sheet-a)
        (property "Sheetname" "Page")
        (property "Sheetfile" "child.kicad_sch")))"#
        .to_vec();
    let child = br#"(kicad_sch
      (uuid child-source)
      (symbol (lib_id "Demo:Part") (uuid child-symbol)
        (instances (project "root"
          (path "/first/sheet-a" (reference "U1"))
          (path "/second/sheet-a" (reference "U2"))))))"#
        .to_vec();
    let index = SchematicBundleIndex::build(&bundle(root, child), SchematicBundleLimits::default())
        .expect("ambiguous suffix index");

    let error = index
        .effective_symbols(2, None)
        .expect_err("compatible suffix paths must be ambiguous");
    assert!(error.message.contains("ambiguous across 2 records"));
}

fn bundle(root: Vec<u8>, child: Vec<u8>) -> SourceBundle {
    let project = b"{}".to_vec();
    let sources = vec![
        descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
        descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        descriptor("design/child.kicad_sch", SourceKind::Schematic, 2, &child),
    ];
    SourceBundle::from_manifest(
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

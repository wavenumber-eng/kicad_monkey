use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SourceBundle, SourceBundleErrorKind,
    SourceBundleLimits,
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

#[test]
fn compact_suffix_index_preserves_shorter_path_and_project_scoping() {
    let root = br#"(kicad_sch
      (uuid top-source)
      (sheet (uuid sheet-a)
        (property "Sheetname" "Page")
        (property "Sheetfile" "child.kicad_sch")))"#
        .to_vec();
    let child = br#"(kicad_sch
      (uuid child-source)
      (symbol (lib_id "Demo:Part") (uuid child-symbol)
        (instances
          (project "other"
            (path "/wrong/sheet-a" (reference "WRONG") (unit 8)))
          (project "root"
            (path "sheet-a" (reference "RIGHT") (unit 4))))))"#
        .to_vec();
    let index = SchematicBundleIndex::build(&bundle(root, child), SchematicBundleLimits::default())
        .expect("short suffix index");

    let symbol = index.effective_symbols(2, None).expect("child symbol");
    assert_eq!((symbol[0].reference.as_str(), symbol[0].unit), ("RIGHT", 4));
}

#[test]
fn empty_path_still_establishes_project_scope_before_global_suffixes() {
    let root = br#"(kicad_sch
      (uuid top-source)
      (sheet (uuid sheet-a)
        (property "Sheetname" "Page")
        (property "Sheetfile" "child.kicad_sch")))"#
        .to_vec();
    let child = br#"(kicad_sch
      (uuid child-source)
      (symbol (lib_id "Demo:Part") (uuid child-symbol)
        (instances
          (project "root"
            (path "" (reference "ROOT") (unit 3)))
          (project "other"
            (path "/prefix/sheet-a" (reference "WRONG") (unit 8))))))"#
        .to_vec();
    let index = SchematicBundleIndex::build(&bundle(root, child), SchematicBundleLimits::default())
        .expect("empty requested-project path index");

    let symbol = index.effective_symbols(2, None).expect("child symbol");
    assert_eq!((symbol[0].reference.as_str(), symbol[0].unit), ("ROOT", 3));
}

#[test]
fn compact_instance_indexes_enforce_the_aggregate_source_byte_budget() {
    let long_path = format!("/{}", "x".repeat(4_096));
    let short_path = "/sheet/second";
    let root = format!(
        "(kicad_sch \
         (symbol (uuid symbol-a) (instances \
         (project \"root\" \
           (path \"{long_path}\" (reference \"U1\"))))) \
         (symbol (uuid symbol-b) (instances \
         (project \"root\" \
           (path \"{short_path}\" (reference \"U2\"))))))"
    )
    .into_bytes();
    let source = bundle(root, b"(kicad_sch)".to_vec());
    let index_bytes = [long_path.as_str(), short_path]
        .into_iter()
        .map(logical_instance_index_bytes)
        .sum();

    SchematicBundleIndex::build(
        &source,
        SchematicBundleLimits {
            max_symbol_instance_index_bytes_per_source: index_bytes,
            ..SchematicBundleLimits::default()
        },
    )
    .expect("exact compact-index byte limit");
    let error = SchematicBundleIndex::build(
        &source,
        SchematicBundleLimits {
            max_symbol_instance_index_bytes_per_source: index_bytes - 1,
            ..SchematicBundleLimits::default()
        },
    )
    .expect_err("one byte over compact-index limit");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("instance index bytes"));
}

#[test]
fn embedded_library_pins_select_effective_unit_and_transform_exactly() {
    let root = br#"(kicad_sch
      (uuid root-source)
      (lib_symbols
        (symbol "Namespace:Base" (power local)
          (symbol "Namespace:Base_0_0"
            (pin passive line (at 1 2 0) (name "COMMON") (number "0")))
          (symbol "Namespace:Base_1_1"
            (pin input line (at 9 9 0) (name "UNIT1") (number "1")))
          (symbol "Namespace:Base_2_1"
            (pin output inverted (at 0 3 90) (name "ACTIVE") (number "2")
              (hide yes) (uuid pin-2)))
          (symbol "Namespace:Base_2_2"
            (pin passive line (at 8 8 0) (name "STYLE2") (number "3"))))
        (symbol "Alias" (extends "Namespace:Base")))
      (symbol (lib_id "Device:Alias") (lib_name "Alias")
        (at 10 20 90) (mirror x) (unit 1) (convert 1) (uuid placed-a)
        (property "Reference" "U")
        (instances
          (project "root"
            (path "/root-source" (reference "U7") (unit 2))))))"#
        .to_vec();
    let index = SchematicBundleIndex::build(
        &bundle(root, b"(kicad_sch)".to_vec()),
        SchematicBundleLimits::default(),
    )
    .expect("embedded library pin index");
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");
    assert_eq!(definition.library_symbols.len(), 2);
    assert!(definition.library_symbols[0].power);
    assert_eq!(
        definition.library_symbols[0].power_kind.as_deref(),
        Some("local")
    );

    let terminals = index.symbol_terminals(1).expect("placed terminals");
    assert_eq!(terminals.len(), 2);
    assert_eq!(
        (
            terminals[0].reference.as_str(),
            terminals[0].pin_number.as_str(),
            terminals[0].pin_name.as_str(),
            terminals[0].at.x_iu,
            terminals[0].at.y_iu,
        ),
        ("U7", "0", "COMMON", 80_000, 210_000)
    );
    assert_eq!(
        (
            terminals[1].pin_number.as_str(),
            terminals[1].electrical_type.as_str(),
            terminals[1].graphic_style.as_str(),
            terminals[1].hidden,
            terminals[1].at.x_iu,
            terminals[1].at.y_iu,
        ),
        ("2", "output", "inverted", true, 70_000, 200_000)
    );
}

#[test]
fn embedded_library_and_terminal_limits_fail_independently() {
    let root = br#"(kicad_sch
      (uuid root-source)
      (lib_symbols
        (symbol "Demo:Part"
          (symbol "Demo:Part_1_1"
            (pin passive line (at 0 0 0) (name "P") (number "1")))))
      (symbol (lib_id "Demo:Part") (uuid placed-a)))"#
        .to_vec();
    let source = bundle(root, b"(kicad_sch)".to_vec());
    for limits in [
        SchematicBundleLimits {
            max_library_symbols_per_source: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_library_subsymbols_per_source: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_library_pins_per_source: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_library_lookup_key_bytes_per_source: 0,
            ..SchematicBundleLimits::default()
        },
    ] {
        assert_eq!(
            SchematicBundleIndex::build(&source, limits)
                .expect_err("embedded library limit")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }
    let limited = SchematicBundleIndex::build(
        &source,
        SchematicBundleLimits {
            max_symbol_terminals_per_occurrence: 0,
            ..SchematicBundleLimits::default()
        },
    )
    .expect("terminal-limited index");
    assert_eq!(
        limited
            .symbol_terminals(1)
            .expect_err("terminal count limit")
            .kind,
        SourceBundleErrorKind::ResourceLimit
    );
}

#[test]
fn inherited_pin_owners_are_resolved_once_for_long_chains_and_many_placements() {
    const CHAIN_LENGTH: usize = 1_024;
    const PLACEMENT_COUNT: usize = 1_024;

    let mut libraries = String::new();
    for index in 0..CHAIN_LENGTH {
        libraries.push_str(&format!(
            "(symbol \"Alias{index}\" (extends \"Alias{}\"))",
            index + 1
        ));
    }
    libraries.push_str(&format!(
        "(symbol \"Alias{CHAIN_LENGTH}\" \
         (symbol \"Alias{CHAIN_LENGTH}_1_1\" \
           (pin passive line (at 0 0 0) (name \"P\") (number \"1\"))))"
    ));
    let mut placements = String::new();
    for index in 0..PLACEMENT_COUNT {
        placements.push_str(&format!(
            "(symbol (lib_id \"Alias0\") (uuid \"placed-{index}\"))"
        ));
    }
    let root = format!("(kicad_sch (uuid root-source) (lib_symbols {libraries}) {placements})")
        .into_bytes();
    let index = SchematicBundleIndex::build(
        &bundle(root, b"(kicad_sch)".to_vec()),
        SchematicBundleLimits::default(),
    )
    .expect("long inheritance chain");
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");

    for placed in &definition.symbols {
        assert_eq!(
            definition
                .library_pin_symbol_for_placement(placed)
                .map(|symbol| symbol.name.as_str()),
            Some("Alias1024")
        );
    }
}

#[test]
fn missing_and_cyclic_library_inheritance_have_no_pin_owner() {
    let root = br#"(kicad_sch
      (uuid root-source)
      (lib_symbols
        (symbol "CycleA" (extends "CycleB"))
        (symbol "CycleB" (extends "CycleA"))
        (symbol "Missing" (extends "Absent")))
      (symbol (lib_id "CycleA") (uuid cycle-a))
      (symbol (lib_id "CycleB") (uuid cycle-b))
      (symbol (lib_id "Missing") (uuid missing)))"#
        .to_vec();
    let index = SchematicBundleIndex::build(
        &bundle(root, b"(kicad_sch)".to_vec()),
        SchematicBundleLimits::default(),
    )
    .expect("invalid inheritance remains representable");
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");

    for placed in &definition.symbols {
        assert!(
            definition
                .library_pin_symbol_for_placement(placed)
                .is_none()
        );
    }
    assert!(
        index
            .symbol_terminals(1)
            .expect("empty terminals")
            .is_empty()
    );
}

#[test]
fn terminal_retained_byte_limit_counts_repeated_symbol_fields() {
    let root = br#"(kicad_sch
      (uuid root-source)
      (lib_symbols
        (symbol "Demo:Part"
          (symbol "Demo:Part_1_1"
            (pin passive line (at 0 0 0) (name "FIRST") (number "1"))
            (pin passive line (at 1 0 0) (name "SECOND") (number "2")))))
      (symbol (lib_id "Demo:Part") (uuid long-placed-uuid)
        (property "Reference" "LONG-REFERENCE")))"#
        .to_vec();
    let source = bundle(root, b"(kicad_sch)".to_vec());
    let baseline = SchematicBundleIndex::build(&source, SchematicBundleLimits::default())
        .expect("baseline terminal bytes");
    let terminals = baseline.symbol_terminals(1).expect("baseline terminals");
    assert_eq!(terminals.len(), 2);
    let retained_bytes = terminals
        .iter()
        .map(|terminal| {
            terminal.symbol_uuid.len()
                + terminal.reference.len()
                + terminal.pin_number.len()
                + terminal.pin_name.len()
                + terminal.electrical_type.len()
                + terminal.graphic_style.len()
        })
        .sum();

    SchematicBundleIndex::build(
        &source,
        SchematicBundleLimits {
            max_symbol_terminal_retained_bytes_per_occurrence: retained_bytes,
            ..SchematicBundleLimits::default()
        },
    )
    .expect("exact retained-byte limit")
    .symbol_terminals(1)
    .expect("exact retained-byte output");
    let limited = SchematicBundleIndex::build(
        &source,
        SchematicBundleLimits {
            max_symbol_terminal_retained_bytes_per_occurrence: retained_bytes - 1,
            ..SchematicBundleLimits::default()
        },
    )
    .expect("one-over retained-byte index");
    let error = limited
        .symbol_terminals(1)
        .expect_err("one-over retained-byte output");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("terminal retained bytes"));
}

#[test]
fn arbitrary_negative_and_mirrored_terminal_transforms_are_deterministic() {
    let root = br#"(kicad_sch
      (uuid root-source)
      (lib_symbols
        (symbol "Demo:Part"
          (symbol "Demo:Part_1_1"
            (pin passive line (at 1 0 0) (name "P") (number "1")))))
      (symbol (lib_id "Demo:Part") (at 0 0 45) (uuid arbitrary))
      (symbol (lib_id "Demo:Part") (at 0 0 -90) (uuid negative))
      (symbol (lib_id "Demo:Part") (at 0 0 90) (mirror x) (uuid mirrored)))"#
        .to_vec();
    let index = SchematicBundleIndex::build(
        &bundle(root, b"(kicad_sch)".to_vec()),
        SchematicBundleLimits::default(),
    )
    .expect("transform vectors");
    let terminals = index.symbol_terminals(1).expect("transformed terminals");
    let point = |uuid: &str| {
        terminals
            .iter()
            .find(|terminal| terminal.symbol_uuid == uuid)
            .map(|terminal| terminal.at)
            .expect("terminal by UUID")
    };

    assert_eq!(
        (point("arbitrary").x_iu, point("arbitrary").y_iu),
        (7_071, -7_071)
    );
    assert_eq!(
        (point("negative").x_iu, point("negative").y_iu),
        (0, 10_000)
    );
    assert_eq!(
        (point("mirrored").x_iu, point("mirrored").y_iu),
        (0, 10_000)
    );
}

#[test]
fn terminal_transform_and_translation_overflow_fail_closed() {
    let transform_overflow = br#"(kicad_sch
      (lib_symbols
        (symbol "Demo:Part"
          (symbol "Demo:Part_1_1"
            (pin passive line (at 0 -922337203685477.5808 0)
              (name "P") (number "1")))))
      (symbol (lib_id "Demo:Part") (uuid placed)))"#
        .to_vec();
    let translation_overflow = br#"(kicad_sch
      (lib_symbols
        (symbol "Demo:Part"
          (symbol "Demo:Part_1_1"
            (pin passive line (at 0.0001 0 0) (name "P") (number "1")))))
      (symbol (lib_id "Demo:Part") (at 922337203685477.5807 0 0)
        (uuid placed)))"#
        .to_vec();

    for (source, expected) in [
        (transform_overflow, "transform overflows"),
        (translation_overflow, "translation overflows"),
    ] {
        let index = SchematicBundleIndex::build(
            &bundle(source, b"(kicad_sch)".to_vec()),
            SchematicBundleLimits::default(),
        )
        .expect("overflow vector parses");
        let error = index
            .symbol_terminals(1)
            .expect_err("overflow must fail closed");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

fn logical_instance_index_bytes(path: &str) -> usize {
    14 * size_of::<usize>() + 2 * path.trim_end_matches('/').len()
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

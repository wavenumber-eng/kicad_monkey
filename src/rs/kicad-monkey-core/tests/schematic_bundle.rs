use kicad_monkey_contracts::decode_source_bundle_manifest_a0;
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicDefinition, SchematicLabelScope,
    SchematicPinShape, SchematicPoint, SourceBundle, SourceBundleErrorKind, SourceBundleLimits,
};
use std::sync::Arc;

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

#[test]
fn typed_connectivity_carriers_preserve_scope_and_wire_topology() {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (version 20250114)
      (uuid root-uuid)
      (wire (pts (xy 0 0) (xy 10 0)) (uuid wire-a))
      (wire (pts (xy 10 0) (xy 20 0)) (uuid wire-b))
      (bus (pts (xy 0 10) (xy 20 10)) (uuid bus-a))
      (bus_entry (at 5 10) (size 0 -10) (uuid entry-a))
      (junction (at 10 0) (uuid junction-a))
      (no_connect (at 30 0) (uuid nc-a))
      (label "LOCAL" (at 0 0 0) (uuid label-a))
      (global_label "GLOBAL" (shape output) (at 20 0 0) (uuid label-b))
      (hierarchical_label "PORT" (shape bidirectional) (at 5 0 0) (uuid label-c)))"#
        .to_vec();
    let bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ]),
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("typed schematic bundle");
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("typed schematic index");
    let definition = index
        .definition("design/root.kicad_sch")
        .expect("root definition");

    assert_carrier_inventory(definition);
    assert_wire_topology(definition);
}

fn assert_carrier_inventory(definition: &SchematicDefinition) {
    assert_eq!(definition.wires.len(), 2);
    assert_eq!(definition.buses.len(), 1);
    assert_eq!(definition.bus_entries.len(), 1);
    assert_eq!(definition.junctions.len(), 1);
    assert_eq!(definition.no_connects.len(), 1);
    assert_eq!(definition.labels.len(), 3);
    assert_eq!(definition.labels[0].scope, SchematicLabelScope::Local);
    assert_eq!(definition.labels[1].scope, SchematicLabelScope::Global);
    assert_eq!(definition.labels[1].shape, "output");
    assert_eq!(
        definition.labels[2].scope,
        SchematicLabelScope::Hierarchical
    );
    assert_eq!(definition.labels[2].shape, "bidirectional");
}

fn assert_wire_topology(definition: &SchematicDefinition) {
    let origin = SchematicPoint { x_iu: 0, y_iu: 0 };
    let far_wire = SchematicPoint {
        x_iu: 200_000,
        y_iu: 0,
    };
    assert!(definition.connectivity.connected(origin, far_wire));
    assert_eq!(
        definition
            .connectivity
            .component(origin)
            .expect("wire component"),
        &[
            SchematicPoint { x_iu: 0, y_iu: 0 },
            SchematicPoint {
                x_iu: 100_000,
                y_iu: 0,
            },
            SchematicPoint {
                x_iu: 200_000,
                y_iu: 0,
            },
        ]
    );
    assert!(!definition.connectivity.connected(
        SchematicPoint {
            x_iu: 0,
            y_iu: 100_000
        },
        SchematicPoint {
            x_iu: 200_000,
            y_iu: 100_000,
        }
    ));
    assert!(
        definition
            .connectivity
            .component(SchematicPoint {
                x_iu: 300_000,
                y_iu: 0
            })
            .is_none()
    );
}

#[test]
fn connectivity_object_and_point_limits_fail_before_unbounded_realization() {
    let project = b"{}".to_vec();
    let root = b"(kicad_sch (wire (pts (xy 0 0) (xy 1 0))))".to_vec();
    let bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ]),
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("bounded schematic bundle");

    let point_limited = SchematicBundleLimits {
        max_points_per_connectivity_object: 1,
        ..SchematicBundleLimits::default()
    };
    assert_eq!(
        SchematicBundleIndex::build(&bundle, point_limited)
            .expect_err("polyline point limit")
            .kind,
        SourceBundleErrorKind::ResourceLimit
    );

    let source_point_limited = SchematicBundleLimits {
        max_connectivity_points_per_source: 1,
        ..SchematicBundleLimits::default()
    };
    assert_eq!(
        SchematicBundleIndex::build(&bundle, source_point_limited)
            .expect_err("source connectivity point limit")
            .kind,
        SourceBundleErrorKind::ResourceLimit
    );

    let object_limited = SchematicBundleLimits {
        max_connectivity_objects_per_source: 0,
        ..SchematicBundleLimits::default()
    };
    assert_eq!(
        SchematicBundleIndex::build(&bundle, object_limited)
            .expect_err("connectivity object limit")
            .kind,
        SourceBundleErrorKind::ResourceLimit
    );
}

#[test]
fn every_retained_connectivity_family_has_an_independent_pre_push_limit() {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (wire (pts (xy 0 0) (xy 1 0)))
      (bus (pts (xy 0 1) (xy 1 1)))
      (bus_entry (at 0 1) (size 0 -1))
      (junction (at 0 0))
      (no_connect (at 2 0))
      (label "N" (at 0 0 0)))"#
        .to_vec();
    let bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ]),
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("all connectivity families");
    let cases = [
        (
            "wire",
            SchematicBundleLimits {
                max_wires_per_source: 0,
                ..SchematicBundleLimits::default()
            },
        ),
        (
            "bus",
            SchematicBundleLimits {
                max_buses_per_source: 0,
                ..SchematicBundleLimits::default()
            },
        ),
        (
            "bus-entry",
            SchematicBundleLimits {
                max_bus_entries_per_source: 0,
                ..SchematicBundleLimits::default()
            },
        ),
        (
            "junction",
            SchematicBundleLimits {
                max_junctions_per_source: 0,
                ..SchematicBundleLimits::default()
            },
        ),
        (
            "no-connect",
            SchematicBundleLimits {
                max_no_connects_per_source: 0,
                ..SchematicBundleLimits::default()
            },
        ),
        (
            "label",
            SchematicBundleLimits {
                max_labels_per_source: 0,
                ..SchematicBundleLimits::default()
            },
        ),
    ];
    for (family, limits) in cases {
        let error = SchematicBundleIndex::build(&bundle, limits).expect_err(family);
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit, "{family}");
        assert!(
            error.message.contains(family),
            "{family}: {}",
            error.message
        );
    }
}

#[test]
fn hierarchical_sheet_pins_are_typed_and_independently_bounded() {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (sheet
        (uuid child)
        (property "Sheetfile" "child.kicad_sch")
        (pin "DATA" bidirectional (at 12.7 25.4 180) (uuid pin-a))
        (pin "READY" output (at 0 0 0))
        (pin "MISSING" (at 0 0 0))
        (pin "UNKNOWN" bogus (at 0 0 0))))"#
        .to_vec();
    let child = b"(kicad_sch)".to_vec();
    let bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
            descriptor("design/child.kicad_sch", SourceKind::Schematic, 2, &child),
        ]),
        vec![project, root, child],
        SourceBundleLimits::default(),
    )
    .expect("sheet pin bundle");
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("sheet pin index");
    let pin = &index
        .definition("design/root.kicad_sch")
        .expect("root")
        .sheets[0]
        .pins[0];
    assert_eq!(pin.name, "DATA");
    assert_eq!(pin.shape, SchematicPinShape::Bidirectional);
    assert_eq!(pin.uuid, "pin-a");
    assert_eq!(
        pin.at,
        SchematicPoint {
            x_iu: 127_000,
            y_iu: 254_000
        }
    );
    let pins = &index
        .definition("design/root.kicad_sch")
        .expect("root")
        .sheets[0]
        .pins;
    assert_eq!(pins[1].shape, SchematicPinShape::Output);
    assert_eq!(pins[2].shape, SchematicPinShape::Input);
    assert_eq!(pins[3].shape, SchematicPinShape::Input);

    let limits = SchematicBundleLimits {
        max_sheet_pins_per_sheet: 0,
        ..SchematicBundleLimits::default()
    };
    let error = SchematicBundleIndex::build(&bundle, limits).expect_err("sheet pin limit");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("sheet pin"));
}

#[test]
fn placed_symbols_preserve_source_fields_and_children() {
    let bundle = placed_symbol_bundle();
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("placed symbol index");
    let symbol = &index
        .definition("design/root.kicad_sch")
        .expect("root")
        .symbols[0];
    assert_eq!(
        (symbol.lib_id.as_str(), symbol.lib_name.as_str()),
        ("Device:R", "R")
    );
    assert_eq!(
        symbol.at,
        SchematicPoint {
            x_iu: 127_000,
            y_iu: 254_000
        }
    );
    assert_eq!(
        (symbol.angle_degrees, symbol.unit, symbol.convert),
        (90.0, 2, 2)
    );
    assert_eq!(symbol.mirror.as_deref(), Some("x"));
    assert!(symbol.exclude_from_sim && symbol.dnp && symbol.fields_autoplaced);
    assert!(!symbol.in_bom && !symbol.on_board && !symbol.in_pos_files);
    assert_eq!(symbol.properties[0].key, "Reference");
    assert_eq!(symbol.properties[1].value, "10k");
    assert_eq!(symbol.pins[0].number, "1");
    assert_eq!(symbol.pins[0].alternate.as_deref(), Some("ALT"));
    assert_eq!(symbol.pins[1].uuid, "pin-2");
}

#[test]
fn modern_and_legacy_symbol_instance_overlays_are_typed_and_indexed() {
    let bundle = placed_symbol_bundle();
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("placed symbol index");
    let definition = index.definition("design/root.kicad_sch").expect("root");
    let symbol = &definition.symbols[0];
    let instance = symbol
        .unique_instance_for_path("/root/")
        .expect("unique modern path")
        .expect("normalized modern path");
    assert_eq!(
        (
            instance.project.as_ref(),
            instance.reference.as_str(),
            instance.unit
        ),
        ("demo", "R1", 2)
    );
    let variant = instance.variant("DNP").expect("variant overlay");
    assert_eq!((variant.dnp, variant.in_bom), (Some(true), Some(false)));
    assert_eq!(variant.exclude_from_sim, None);
    assert_eq!(
        (
            variant.fields[0].name.as_str(),
            variant.fields[0].value.as_str()
        ),
        ("MPN", "variant-part")
    );
    let legacy = definition
        .legacy_symbol_instance("/legacy/symbol-a/")
        .expect("normalized legacy path");
    assert_eq!(
        (
            legacy.reference.as_str(),
            legacy.unit,
            legacy.value.as_str(),
            legacy.footprint.as_str()
        ),
        ("R9", 3, "legacy-value", "Legacy:Part")
    );
}

#[test]
fn symbol_and_instance_limits_fail_independently_before_retention() {
    let bundle = placed_symbol_bundle();
    for limits in [
        SchematicBundleLimits {
            max_symbols_per_source: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_symbol_properties_per_symbol: 1,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_symbol_pins_per_symbol: 1,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_symbol_instance_projects_per_symbol: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_symbol_instances_per_symbol: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_symbol_variants_per_instance: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_symbol_variant_fields_per_variant: 0,
            ..SchematicBundleLimits::default()
        },
        SchematicBundleLimits {
            max_legacy_symbol_instances_per_source: 0,
            ..SchematicBundleLimits::default()
        },
    ] {
        assert_eq!(
            SchematicBundleIndex::build(&bundle, limits)
                .expect_err("symbol child limit")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }
}

fn placed_symbol_bundle() -> SourceBundle {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (symbol
        (lib_id "Device:R") (lib_name "R") (at 12.7 25.4 90) (mirror x)
        (unit 2) (convert 2) (exclude_from_sim yes) (in_bom no)
        (on_board no) (in_pos_files no) (dnp yes) (fields_autoplaced)
        (uuid symbol-a)
        (property "Reference" "R1") (property "Value" "10k")
        (pin "1" (uuid pin-1) (alternate "ALT")) (pin "2" (uuid pin-2))
        (instances
          (project "demo"
            (path "/root" (reference "R1") (unit 2)
              (variant (name "DNP") (dnp yes) (in_bom no)
                (field (name "MPN") (value "variant-part")))))))
      (symbol_instances
        (path "/legacy/symbol-a" (reference "R9") (unit 3)
          (value "legacy-value") (footprint "Legacy:Part"))))"#
        .to_vec();
    SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ]),
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("placed symbol bundle")
}

#[test]
fn modern_symbol_instance_paths_require_exact_project_ownership() {
    let project = b"{}".to_vec();
    let root = br#"(kicad_sch
      (symbol (uuid symbol-a)
        (instances
          (project "demo")
          (foreign (path "/not-owned" (reference "R1") (unit 1))))))"#
        .to_vec();
    let bundle = SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ]),
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("foreign instance bundle");
    let error = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect_err("foreign path must not inherit a preceding project");
    assert_eq!(error.kind, SourceBundleErrorKind::Schematic);
    assert!(error.message.contains("outside its owning project"));
}

#[test]
fn symbol_instance_lookup_is_project_aware_and_rejects_ambiguity() {
    let bundle = schematic_with_root(
        br#"(kicad_sch
          (symbol (uuid symbol-a)
            (instances
              (project "alpha"
                (path "/same" (reference "A1"))
                (path "/same/" (reference "A2")))
              (project "beta"
                (path "/same" (reference "B1"))
                (path "/other" (reference "B2"))))))"#
            .to_vec(),
    );
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("duplicate path index");
    let symbol = &index
        .definition("design/root.kicad_sch")
        .expect("root")
        .symbols[0];
    assert_eq!(
        symbol
            .instance_for_project("alpha", "/same")
            .expect_err("same-project duplicate must be ambiguous")
            .matches,
        2
    );
    assert_eq!(
        symbol
            .instance_for_project("beta", "/same/")
            .expect("unique beta path")
            .expect("beta path")
            .reference,
        "B1"
    );
    assert_eq!(
        symbol
            .unique_instance_for_path("/same")
            .expect_err("cross-project path must be ambiguous")
            .matches,
        3
    );
    assert_eq!(
        symbol
            .unique_instance_for_path("/other/")
            .expect("unique path")
            .expect("other path")
            .reference,
        "B2"
    );
    assert!(
        symbol
            .instance_for_project("missing", "/same")
            .expect("missing project is not ambiguous")
            .is_none()
    );
}

#[test]
fn repeated_instance_paths_share_one_project_name_allocation() {
    let project_name = "project-".to_owned() + &"x".repeat(64 * 1024);
    let mut paths = String::new();
    for index in 0..512 {
        paths.push_str(&format!(
            "(path \"/sheet/{index}\" (reference \"R{index}\") (unit 1))"
        ));
    }
    let root = format!(
        "(kicad_sch (symbol (uuid symbol-a) (instances (project \"{project_name}\" {paths}))))"
    );
    let bundle = schematic_with_root(root.into_bytes());
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())
        .expect("shared project allocation index");
    let instances = &index
        .definition("design/root.kicad_sch")
        .expect("root")
        .symbols[0]
        .instances;
    assert_eq!(instances.len(), 512);
    assert!(
        instances
            .iter()
            .all(|instance| Arc::ptr_eq(&instances[0].project, &instance.project))
    );
}

fn schematic_with_root(root: Vec<u8>) -> SourceBundle {
    let project = b"{}".to_vec();
    SourceBundle::from_manifest(
        manifest(vec![
            descriptor("design/root.kicad_pro", SourceKind::Project, 0, &project),
            descriptor("design/root.kicad_sch", SourceKind::Schematic, 1, &root),
        ]),
        vec![project, root],
        SourceBundleLimits::default(),
    )
    .expect("single schematic bundle")
}

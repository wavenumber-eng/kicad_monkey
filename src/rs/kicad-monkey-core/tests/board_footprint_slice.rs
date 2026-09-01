use kicad_monkey_core::board_plotter_ir::{
    BoardFootprintBlock, BoardFootprintChildAttributes, BoardFootprintChildMetadata,
    BoardFootprintOperation, BoardFootprintRecord, BoardTextRenderCacheCoordinateSpace,
};
use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardPlotRecord, ErrorKind, PlotDocumentMetadata,
    PlotDocumentProjectionLimits, PlotterOperation, board_plot_document,
    board_plot_document_with_net_classes, project_board_plot_document_with_metadata_a0,
};

const CANONICAL_SOURCE: &str = r#"(kicad_pcb
  (version 20260817)
  (zone (net 0) (layer "F.Cu")
    (filled_polygon (layer "F.Cu") (pts (xy 0 0) (xy 1 0) (xy 0 1))))
  (footprint "Demo:Canonical" locked
    (layer "B.Cu") (at 10 20 45) (uuid "fp-id")
    (descr "canonical") (tags "demo") (attr smd dnp)
    (property "Reference" "R1" (at 0 0) (layer "F.SilkS") (uuid "ref-id")
      (effects (font (size 1 1))))
    (property "Value" "10k" (at 0 1) (layer "F.Fab") (uuid "value-id")
      (effects (font (size 1 1))))
    (property "Note" "note" (at 0 2) (layer "F.Fab") (uuid "note-id")
      (effects (font (size 1 1))))
    (fp_text user "${Reference}-${Value}" (at 1 1) (layer "B.SilkS") (uuid "text-id")
      (effects (font (size 1 1)) (justify right top mirror)))
    (fp_text_box "box" (start -1 -1) (end 2 2) (margins 0 0 0 0)
      (layer "F.Fab") (border yes) (uuid "box-id")
      (stroke (width 0.2) (type solid)) (effects (font (size 1 1))))
    (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid))
      (layer "F.SilkS") (uuid "line-id"))
    (fp_arc (start 1 0) (mid 0 1) (end -1 0) (stroke (width 0.1) (type solid))
      (layer "F.Fab") (uuid "arc-id"))
    (fp_circle (center 0 0) (end 1 0) (stroke (width 0.1) (type solid))
      (fill none) (layer "F.CrtYd") (uuid "circle-id"))
    (fp_rect (start -1 -1) (end 1 1) (stroke (width 0.1) (type solid))
      (fill none) (layer "F.Fab") (uuid "rect-id"))
    (fp_poly (pts (xy 0 0) (xy 1 0) (xy 0 1))
      (stroke (width 0.1) (type solid)) (fill solid)
      (layer "B.Cu") (uuid "poly-id")))
)"#;

const PAD_SOURCE: &str = r#"(kicad_pcb
  (net 1 "GND")
  (footprint "Demo:Pads" (at 10 20 45) (uuid "fp-pads")
    (property "Reference" "U1")
    (solder_mask_margin 0.2)
    (pad "1" thru_hole oval (at 1 2 75) (size 2 1)
      (drill oval 0.6 1.0 (offset 0.1 0.2))
      (layers "F.Cu" "B.Cu") (net 1) (uuid "pad-1"))
    (pad "" np_thru_hole circle (at -1 -2) (size 1 1)
      (layers "*.Cu" "*.Mask") (solder_mask_margin -1) (uuid "pad-2")))
)"#;

const CACHE_SOURCE: &str = r#"(kicad_pcb
  (footprint "Demo:Cache" (at 10 20 90) (uuid "fp-cache")
    (property "Label" "cached" (at 0 0) (layer "F.SilkS") (uuid "cache-property")
      (effects (font (size 1 1)))
      (render_cache "cached" 123
        (polygon (pts (xy 10 20) (xy 10 21) (xy 9 20))))))
)"#;

fn footprint_record(document: &kicad_monkey_core::BoardPlotDocument) -> &BoardFootprintRecord {
    document
        .records
        .iter()
        .find_map(|record| match record {
            BoardPlotRecord::Footprint(value) => Some(value),
            _ => None,
        })
        .expect("embedded footprint record")
}

fn child_metadata(operation: &BoardFootprintOperation) -> Option<&BoardFootprintChildMetadata> {
    match operation {
        BoardFootprintOperation::Geometry { metadata, .. }
        | BoardFootprintOperation::Text { metadata, .. } => Some(metadata),
        _ => None,
    }
}

fn start_block(operation: &BoardFootprintOperation) -> &BoardFootprintBlock {
    let BoardFootprintOperation::StartBlock(block) = operation else {
        panic!("expected StartBlock, got {operation:?}");
    };
    block
}

fn assert_canonical_record_fields(record: &BoardFootprintRecord) {
    assert_eq!(record.uuid, "fp-id");
    assert_eq!(record.library_link, "Demo:Canonical");
    assert_eq!(
        (record.reference.as_str(), record.value.as_str()),
        ("R1", "10k")
    );
    assert_eq!(record.layer, "B.Cu");
    assert!(record.locked);
    assert_eq!(
        (record.descr.as_str(), record.tags.as_str()),
        ("canonical", "demo")
    );
    assert_eq!(record.attr, ["smd", "dnp"]);
    assert_eq!(
        (record.placement.x_nm, record.placement.y_nm),
        (10_000_000, 20_000_000)
    );
    assert_eq!(record.placement.angle_deg, 45.0);
}

fn assert_child_metadata_entry(
    actual: &BoardFootprintChildMetadata,
    expected: (&str, usize, Option<usize>, &str),
) {
    let (data_ref, object_index, sub_index, label) = expected;
    assert_eq!(actual.data_ref, data_ref);
    assert_eq!(actual.extra_attrs.footprint_primitive, data_ref);
    assert_eq!(actual.extra_attrs.footprint_object_index, object_index);
    assert_eq!(actual.extra_attrs.footprint_subop_index, sub_index);
    assert_eq!(actual.label, label);
    assert_eq!(actual.data_uuid, label.split(':').next().unwrap());
    assert_eq!(actual.extra_attrs.component, "R1");
    assert_eq!(actual.extra_attrs.component_uid, "fp-id");
    assert_eq!(actual.extra_attrs.footprint, "Demo:Canonical");
}

fn assert_canonical_child_metadata(record: &BoardFootprintRecord) {
    let metadata = record
        .operations
        .iter()
        .filter_map(child_metadata)
        .collect::<Vec<_>>();
    assert_eq!(metadata.len(), 11);
    let expected = [
        ("property", 0, None, "ref-id:footprint-text:0"),
        ("property", 1, None, "value-id:footprint-text:1"),
        ("property", 2, None, "note-id:footprint-text:2"),
        ("fp_text", 0, None, "text-id:footprint-text:0"),
        ("fp_text_box", 0, Some(0), "box-id:footprint-graphic:0:0"),
        ("fp_text_box", 0, Some(1), "box-id:footprint-text:0:1"),
        ("fp_line", 0, Some(0), "line-id:footprint-graphic:0:0"),
        ("fp_arc", 0, Some(0), "arc-id:footprint-graphic:0:0"),
        ("fp_circle", 0, None, "circle-id:footprint-graphic:0"),
        ("fp_rect", 0, None, "rect-id:footprint-graphic:0"),
        ("fp_poly", 0, None, "poly-id:footprint-graphic:0"),
    ];
    for (actual, expected) in metadata.iter().zip(expected) {
        assert_child_metadata_entry(actual, expected);
    }
    assert_eq!(
        metadata[0].extra_attrs.footprint_text_role.as_deref(),
        Some("designator")
    );
    assert_eq!(
        metadata[1].extra_attrs.footprint_text_role.as_deref(),
        Some("value")
    );
    assert_eq!(
        metadata[2].extra_attrs.property_name.as_deref(),
        Some("Note")
    );
    assert_eq!(
        metadata[8].extra_attrs.footprint_graphic_kind.as_deref(),
        Some("circle")
    );
}

fn assert_canonical_user_text(record: &BoardFootprintRecord) {
    let BoardFootprintOperation::Text {
        operation: user_text,
        ..
    } = &record.operations[3]
    else {
        panic!("expected substituted user text");
    };
    assert_eq!(user_text.text, "R1-10k");
    assert_eq!(
        (user_text.h_align, user_text.v_align),
        (
            kicad_monkey_core::board_plotter_ir::BoardTextHAlign::Right,
            kicad_monkey_core::board_plotter_ir::BoardTextVAlign::Top,
        )
    );
    assert!(
        !user_text.mirror,
        "the Python A0 operation omits a mirror field"
    );
}

#[test]
fn embedded_footprint_records_follow_python_order_placement_and_child_metadata() {
    let document = board_plot_document(CANONICAL_SOURCE, BoardPlotLimits::default())
        .expect("canonical embedded footprint");
    assert!(matches!(document.records[0], BoardPlotRecord::Zone(_)));
    assert!(matches!(document.records[1], BoardPlotRecord::Footprint(_)));

    let record = footprint_record(&document);
    assert_canonical_record_fields(record);
    assert_canonical_child_metadata(record);
    assert_canonical_user_text(record);
}

#[test]
fn typed_board_projection_matches_the_full_embedded_footprint_python_vector() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board parity vectors");
    let vector = &vectors["vectors"][12];
    assert_eq!(
        vector["id"],
        "embedded-footprints-follow-zones-and-keep-local-ownership"
    );
    let source = vector["source"].as_str().expect("source");
    let classes = BoardNetClassAssignments::from_entries([("GND", vec!["Power", "HighCurrent"])]);
    let document =
        board_plot_document_with_net_classes(source, BoardPlotLimits::default(), &classes)
            .expect("embedded footprint document");
    let projected = project_board_plot_document_with_metadata_a0(
        document,
        PlotDocumentMetadata {
            source_path: Some(
                vector["source_path"]
                    .as_str()
                    .expect("source path")
                    .to_owned(),
            ),
            document_id: vector["document_id"]
                .as_str()
                .expect("document id")
                .to_owned(),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .expect("typed board projection");
    assert_eq!(
        serde_json::to_value(projected).expect("projected JSON"),
        vector["expected"]
    );
}

fn assert_duplicate_reference_record(record: &BoardFootprintRecord) {
    assert_eq!(record.layer, "F.Cu");
    assert_eq!((record.placement.x_nm, record.placement.y_nm), (0, 0));
    assert_eq!(record.placement.angle_deg, 0.0);
    assert!(!record.locked);
    assert_eq!((record.descr.as_str(), record.tags.as_str()), ("", ""));
    assert_eq!(record.reference, "FIRST");
    assert_eq!(
        record.operations.len(),
        2,
        "only the first duplicate field is graphical"
    );
}

fn assert_duplicate_reference_operations(record: &BoardFootprintRecord) {
    let BoardFootprintOperation::Text {
        operation: property,
        metadata,
    } = &record.operations[0]
    else {
        panic!("expected Reference property");
    };
    assert_eq!(property.text, "FIRST");
    assert_eq!(metadata.label, "Bare:footprint-text:0");
    assert_eq!(metadata.data_uuid, metadata.label);

    let BoardFootprintOperation::Text {
        operation: reference,
        metadata,
    } = &record.operations[1]
    else {
        panic!("expected reference fp_text");
    };
    assert_eq!(
        reference.text, "SECOND",
        "the last duplicate remains the text variable"
    );
    assert_eq!(metadata.object_id, "reference:0");
}

fn assert_empty_footprint(record: &BoardPlotRecord) {
    let BoardPlotRecord::Footprint(empty) = record else {
        panic!("expected empty footprint record");
    };
    assert_eq!(empty.library_link, "Empty");
    assert_eq!((empty.reference.as_str(), empty.value.as_str()), ("", ""));
    assert_eq!(empty.layer, "F.Cu");
    assert!(empty.operations.is_empty());
}

#[test]
fn duplicate_reference_defaults_and_fallback_labels_match_python() {
    let source = r#"(kicad_pcb
      (footprint "Bare"
        (property "Reference" "FIRST" (at 0 0) (layer "F.SilkS"))
        (property "Reference" "SECOND" (at 1 0) (layer "F.SilkS"))
        (fp_text reference "RAW" (at 0 0) (layer "F.SilkS")
          (effects (font (size 1 1)))))
      (footprint "Empty"))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("defaults");
    let record = footprint_record(&document);
    assert_duplicate_reference_record(record);
    assert_duplicate_reference_operations(record);
    assert_empty_footprint(&document.records[1]);
}

#[test]
fn authored_cache_points_are_localized_and_charge_retained_output_points() {
    let document = board_plot_document(CACHE_SOURCE, BoardPlotLimits::default())
        .expect("localized authored cache");
    let record = footprint_record(&document);
    let BoardFootprintOperation::Text { operation, .. } = &record.operations[0] else {
        panic!("expected cached property text");
    };
    assert_eq!(
        operation.render_cache_polygons,
        [vec![[0, 0], [-1_000_000, 0], [0, -1_000_000]]]
    );
    let cache = operation
        .render_cache
        .as_ref()
        .expect("typed authored cache");
    assert_eq!(
        cache.coordinate_space,
        BoardTextRenderCacheCoordinateSpace::FootprintLocal
    );
    assert_eq!(cache.text, "cached");
    assert_eq!(cache.angle, 123.0);
    assert!(
        !cache.exact,
        "footprint requests intentionally omit angle matching"
    );
    assert_eq!(
        cache.polygons,
        [vec![vec![[0, 0], [-1_000_000, 0], [0, -1_000_000]]]]
    );

    let exact = BoardPlotLimits {
        max_points: 6,
        ..BoardPlotLimits::default()
    };
    board_plot_document(CACHE_SOURCE, exact).expect("three retained plus three exterior points");
    let one_under = BoardPlotLimits {
        max_points: 5,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(CACHE_SOURCE, one_under)
            .expect_err("cache exterior duplication consumes the point budget")
            .kind,
        ErrorKind::ResourceLimit
    );
}

fn assert_plated_pad(record: &BoardFootprintRecord) {
    let plated = start_block(&record.operations[0]);
    assert_eq!(
        (plated.data_ref.as_str(), plated.label.as_str()),
        ("pad", "pad-1")
    );
    assert_eq!(plated.layers, ["F.Cu", "B.Cu"]);
    assert_eq!(plated.extra_attrs.component.as_deref(), Some("U1"));
    assert_eq!(plated.extra_attrs.pad_designator.as_deref(), Some("U1-1"));
    assert_eq!(plated.extra_attrs.net_id.as_deref(), Some("1"));
    assert_eq!(plated.extra_attrs.net.as_deref(), Some("GND"));
    assert_eq!(plated.extra_attrs.net_class.as_deref(), Some("Power"));
    assert_eq!(
        plated.extra_attrs.net_classes.as_deref(),
        Some("Power,Default")
    );
    let BoardFootprintOperation::Pad(PlotterOperation::FlashPadOval(flash)) = &record.operations[1]
    else {
        panic!("expected plated oval flash");
    };
    assert_eq!(flash.orient_deg, 30.0);
    assert_eq!(flash.mask_margin_nm, 200_000);
    assert!(matches!(
        record.operations[2],
        BoardFootprintOperation::EndBlock
    ));
}

fn assert_plated_hole(record: &BoardFootprintRecord) {
    let plated_hole = start_block(&record.operations[3]);
    assert_eq!(plated_hole.data_ref, "pad_hole");
    assert_eq!(plated_hole.label, "pad-1:hole");
    assert_eq!(plated_hole.layers, ["F.Cu", "B.Cu"]);
    assert_eq!(plated_hole.extra_attrs.hole_owner.as_deref(), Some("pad-1"));
    assert_eq!(plated_hole.extra_attrs.hole_kind.as_deref(), Some("slot"));
    assert_eq!(
        plated_hole.extra_attrs.hole_plating.as_deref(),
        Some("plated")
    );
    assert_eq!(
        plated_hole.extra_attrs.hole_width_mm.as_deref(),
        Some("0.6")
    );
    assert_eq!(
        plated_hole.extra_attrs.hole_height_mm.as_deref(),
        Some("1.0")
    );
    let BoardFootprintOperation::Pad(PlotterOperation::ThickSegment(slot)) = &record.operations[4]
    else {
        panic!("expected plated slot drill");
    };
    assert_eq!(slot.width_nm, 600_000);
    assert_eq!(slot.role.as_deref(), Some("pad_drill"));
}

fn assert_npth_pad_and_hole(record: &BoardFootprintRecord) {
    let npth = start_block(&record.operations[6]);
    assert_eq!(npth.label, "pad-2");
    assert_eq!(npth.extra_attrs.pad_designator, None);
    let BoardFootprintOperation::Pad(PlotterOperation::FlashPadCircle(npth_flash)) =
        &record.operations[7]
    else {
        panic!("expected NPTH source flash");
    };
    assert_eq!(
        npth_flash.mask_margin_nm, -500_000,
        "negative margin clamps to half size"
    );

    let npth_hole = start_block(&record.operations[9]);
    assert!(npth_hole.layers.is_empty(), "NPTH hole block omits layers");
    assert_eq!(
        npth_hole.extra_attrs.hole_plating.as_deref(),
        Some("non_plated")
    );
    assert_eq!(
        npth_hole.extra_attrs.hole_diameter_mm.as_deref(),
        Some("1.0")
    );
    let BoardFootprintOperation::Pad(PlotterOperation::Circle(hole)) = &record.operations[10]
    else {
        panic!("expected fallback NPTH drill");
    };
    assert_eq!(hole.role.as_deref(), Some("npth_hole"));
    assert_eq!(hole.mask_margin_nm, Some(-500_000));
    assert_eq!(
        (hole.pad_size_x_nm, hole.pad_size_y_nm),
        (Some(1_000_000), Some(1_000_000))
    );
}

#[test]
fn pad_and_hole_blocks_preserve_orientation_mask_net_and_npth_metadata() {
    let classes = BoardNetClassAssignments::from_entries([("GND", vec!["Power", "Default"])]);
    let document =
        board_plot_document_with_net_classes(PAD_SOURCE, BoardPlotLimits::default(), &classes)
            .expect("pad and hole blocks");
    let record = footprint_record(&document);
    assert_eq!(record.operations.len(), 12);
    assert_plated_pad(record);
    assert_plated_hole(record);
    assert_npth_pad_and_hole(record);
}

#[test]
fn legacy_unlayered_npth_pad_keeps_its_source_shape_and_drill_blocks() {
    let source = r#"(kicad_pcb
      (footprint "Demo:Unlayered" (uuid "fp-unlayered")
        (pad "" np_thru_hole roundrect (size 1.75 1.75) (drill 1.77)
          (layers) (roundrect_rratio 0.15) (uuid "unlayered-pad"))))"#;
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("legacy unlayered NPTH pad");
    let record = footprint_record(&document);
    assert_eq!(record.operations.len(), 6);
    assert_eq!(start_block(&record.operations[0]).label, "unlayered-pad");
    assert_eq!(
        start_block(&record.operations[3]).label,
        "unlayered-pad:hole"
    );
}

#[test]
fn operation_text_and_cache_text_limits_are_exact_and_fail_without_a_document() {
    let pad_only = r#"(kicad_pcb
      (footprint "P" (pad "1" smd circle (size 1 1) (layers "F.Cu"))))"#;
    board_plot_document(
        pad_only,
        BoardPlotLimits {
            max_operations: 3,
            ..BoardPlotLimits::default()
        },
    )
    .expect("StartBlock, flash, EndBlock fit exactly");
    assert_eq!(
        board_plot_document(
            pad_only,
            BoardPlotLimits {
                max_operations: 2,
                ..BoardPlotLimits::default()
            },
        )
        .expect_err("block wrappers consume the operation budget")
        .kind,
        ErrorKind::ResourceLimit
    );

    let text_only = r#"(kicad_pcb
      (footprint "T" (fp_text user "abcd" (at 0 0) (layer "F.SilkS")
        (effects (font (size 1 1))))))"#;
    board_plot_document(
        text_only,
        BoardPlotLimits {
            max_text_bytes: 4,
            ..BoardPlotLimits::default()
        },
    )
    .expect("four retained text bytes");
    assert_eq!(
        board_plot_document(
            text_only,
            BoardPlotLimits {
                max_text_bytes: 3,
                ..BoardPlotLimits::default()
            },
        )
        .expect_err("one-under text budget")
        .kind,
        ErrorKind::ResourceLimit
    );

    board_plot_document(
        CACHE_SOURCE,
        BoardPlotLimits {
            max_text_bytes: 12,
            ..BoardPlotLimits::default()
        },
    )
    .expect("operation and cache retain two six-byte strings");
    assert_eq!(
        board_plot_document(
            CACHE_SOURCE,
            BoardPlotLimits {
                max_text_bytes: 11,
                ..BoardPlotLimits::default()
            },
        )
        .expect_err("cache text duplication is retained")
        .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn input_point_and_polygon_limits_cover_footprint_graphics_and_custom_pads() {
    let poly = r#"(kicad_pcb
      (footprint "Poly"
        (fp_poly (pts (xy 0 0) (xy 1 0) (xy 0 1))
          (stroke (width 0.1) (type solid)) (fill solid) (layer "F.Cu"))))"#;
    board_plot_document(
        poly,
        BoardPlotLimits {
            max_input_points: 3,
            max_input_polygons: 1,
            ..BoardPlotLimits::default()
        },
    )
    .expect("one three-point footprint polygon");
    for limits in [
        BoardPlotLimits {
            max_input_points: 2,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_input_polygons: 0,
            ..BoardPlotLimits::default()
        },
    ] {
        assert_eq!(
            board_plot_document(poly, limits)
                .expect_err("one-under footprint polygon input limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let custom = r#"(kicad_pcb
      (footprint "Custom"
        (pad "1" smd custom (size 1 1) (layers "F.Cu")
          (primitives (gr_poly (pts (xy 0 0) (xy 1 0) (xy 0 1))
            (width 0.1) (fill yes))))))"#;
    board_plot_document(
        custom,
        BoardPlotLimits {
            max_input_points: 3,
            max_input_polygons: 1,
            ..BoardPlotLimits::default()
        },
    )
    .expect("one custom-pad polygon");
    assert_eq!(
        board_plot_document(
            custom,
            BoardPlotLimits {
                max_input_polygons: 0,
                ..BoardPlotLimits::default()
            },
        )
        .expect_err("custom pad polygon count")
        .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn metadata_limit_matches_the_retained_record_child_and_block_strings() {
    let source = r#"(kicad_pcb
      (footprint "F" (uuid "U")
        (fp_circle (center 0 0) (end 1 0) (stroke (width 0.1) (type solid))
          (layer "F.SilkS") (uuid "C"))
        (pad "1" smd circle (size 1 1) (layers "F.Cu") (uuid "P"))))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("metadata probe");
    let expected = retained_metadata_bytes(footprint_record(&document));
    assert!(expected > 0);
    board_plot_document(
        source,
        BoardPlotLimits {
            max_metadata_bytes: expected,
            ..BoardPlotLimits::default()
        },
    )
    .expect("exact retained metadata bytes");
    assert_eq!(
        board_plot_document(
            source,
            BoardPlotLimits {
                max_metadata_bytes: expected - 1,
                ..BoardPlotLimits::default()
            },
        )
        .expect_err("one-under retained metadata bytes")
        .kind,
        ErrorKind::ResourceLimit
    );
}

fn retained_metadata_bytes(record: &BoardFootprintRecord) -> usize {
    let record_bytes = [
        record.uuid.len(),
        record.library_link.len(),
        record.reference.len(),
        record.value.len(),
        record.layer.len(),
        record.descr.len(),
        record.tags.len(),
    ]
    .into_iter()
    .chain(record.attr.iter().map(String::len))
    .sum::<usize>();
    record
        .operations
        .iter()
        .fold(record_bytes, |total, operation| {
            total
                + match operation {
                    BoardFootprintOperation::Geometry { metadata, .. }
                    | BoardFootprintOperation::Text { metadata, .. } => {
                        child_metadata_bytes(metadata)
                    }
                    BoardFootprintOperation::StartBlock(block) => block_metadata_bytes(block),
                    BoardFootprintOperation::Pad(_) | BoardFootprintOperation::EndBlock => 0,
                }
        })
}

fn child_metadata_bytes(metadata: &BoardFootprintChildMetadata) -> usize {
    let attrs = &metadata.extra_attrs;
    [
        metadata.label.len(),
        metadata.data_uuid.len(),
        metadata.data_ref.len(),
        metadata.object_id.len(),
        attrs.component.len(),
        attrs.component_uid.len(),
        attrs.component_uuid.len(),
        attrs.footprint.len(),
        attrs.primitive.len(),
        attrs.footprint_primitive.len(),
    ]
    .into_iter()
    .sum::<usize>()
        + child_attribute_options(attrs)
            .map(String::len)
            .sum::<usize>()
}

fn child_attribute_options(attrs: &BoardFootprintChildAttributes) -> impl Iterator<Item = &String> {
    [
        attrs.layer_name.as_ref(),
        attrs.layer_role.as_ref(),
        attrs.footprint_text_role.as_ref(),
        attrs.property_name.as_ref(),
        attrs.fp_text_type.as_ref(),
        attrs.footprint_graphic_kind.as_ref(),
    ]
    .into_iter()
    .flatten()
}

fn block_metadata_bytes(block: &BoardFootprintBlock) -> usize {
    block.label.len()
        + block.data_uuid.len()
        + block.data_ref.len()
        + block.object_id.len()
        + block.layers.iter().map(String::len).sum::<usize>()
        + block.extra_attrs.primitive.len()
        + block_attribute_options(&block.extra_attrs)
            .map(String::len)
            .sum::<usize>()
}

fn block_attribute_options(
    attrs: &kicad_monkey_core::board_plotter_ir::BoardFootprintBlockAttributes,
) -> impl Iterator<Item = &String> {
    [
        attrs.component.as_ref(),
        attrs.component_uid.as_ref(),
        attrs.component_uuid.as_ref(),
        attrs.footprint.as_ref(),
        attrs.pad_number.as_ref(),
        attrs.pad_designator.as_ref(),
        attrs.pad_type.as_ref(),
        attrs.pad_shape.as_ref(),
        attrs.layer_names.as_ref(),
        attrs.net_index.as_ref(),
        attrs.net_id.as_ref(),
        attrs.net.as_ref(),
        attrs.net_class.as_ref(),
        attrs.net_classes.as_ref(),
        attrs.hole_owner.as_ref(),
        attrs.hole_kind.as_ref(),
        attrs.hole_plating.as_ref(),
        attrs.hole_render.as_ref(),
        attrs.hole_diameter_mm.as_ref(),
        attrs.hole_width_mm.as_ref(),
        attrs.hole_height_mm.as_ref(),
    ]
    .into_iter()
    .flatten()
}

use kicad_monkey_core::{ErrorKind, PcbLimits, PcbView, parse};

const SOURCE: &str = r#"# board comment survives
(kicad_pcb
  (version 20240108)
  (generator pcbnew)
  (general (thickness 1.6))
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (36 "B.SilkS" user "b.silkscreen"))
  (setup (future_setup "preserve"))
  (net 0 "")
  (net 1 "GND")
  (property "Owner" "old value")
  (footprint "Demo:Part"
    (layer "F.Cu")
    (at 10.5 20.25 90)
    (property "Reference" "U1")
    (property "Value" "Demo")
    (uuid 11111111-1111-1111-1111-111111111111)
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu" "F.Paste"))
    (model "${KICAD9_3DMODEL_DIR}/Demo.wrl"
      (offset (xyz 1 2 3))
      (scale (xyz 1.5 2.5 3.5))
      (rotate (xyz 10 20 30)))
    (future_footprint_data (nested "preserve")))
  (segment (start 1 2) (end 3 4) (width 0.25) (layer "F.Cu") (net 1)
    (uuid 22222222-2222-2222-2222-222222222222))
  (via (at 5 6) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net 1)
    (uuid 33333333-3333-3333-3333-333333333333))
  (zone (net 1) (net_name "GND") (layers "F.Cu" "B.Cu")
    (uuid 44444444-4444-4444-4444-444444444444)
    (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))
  (gr_line (start 0 0) (end 1 1) (stroke (width 0.1) (type default)) (layer "Edge.Cuts"))
  (image (at 0 0) (data "known image"))
  (barcode "known barcode")
  (table (cells))
  (future_board_data (nested "must survive"))
)
"#;

const CARRIERS: &str = r#"(kicad_pcb
  (net 1 "GND")
  (gr_text "hello" (at 1 2) (layer "F.SilkS") (uuid text-id))
  (gr_line (start 1 2) (end 3 4) (stroke (width 0.2) (type dash)) (layer "Edge.Cuts") (uuid line-id))
  (gr_rect (start 5 6) (end 7 8) (stroke (width 0.3) (type default)) (fill solid) (layer "F.Cu"))
  (gr_arc (start 1 0) (mid 0 1) (end -1 0) (stroke (width 0.4) (type default)) (layer "B.Cu"))
  (gr_circle (center 10 10) (end 12 10) (stroke (width 0.5) (type default)) (fill no) (layer "Dwgs.User"))
  (gr_poly (pts (xy 0 0) (xy 1 0) (xy 1 1)) (stroke (width 0.1) (type default)) (fill solid) (layer "F.Cu"))
  (gr_curve (pts (xy 0 0) (xy 1 0) (xy 1 1) (xy 2 1)) (stroke (width 0.1) (type default)) (layer "F.SilkS"))
  (gr_text_box "boxed" (start 0 0) (end 5 5) (layer "F.SilkS"))
  (arc (start 0 0) (mid 1 1) (end 2 0) (width 0.25) (layer "F.Cu") (net "GND") (uuid arc-id))
  (dimension (type aligned) (locked yes) (layer "Cmts.User") (uuid dimension-id)
    (pts (xy 0 0) (xy 5 0)) (height 2.5) (orientation 1))
  (group "Review" (id group-id) locked (members line-id arc-id))
  (generated (id generated-id) (type tuned_delay) (name "Tune") (layer "F.Cu")
    (locked yes) (corner_radius 1.5) (members arc-id))
  (embedded_files
    (file (name "asset.step") (type model) (data |YWJj| |ZA==|) (checksum "abcd")))
)"#;

#[test]
fn board_view_indexes_major_families_and_exact_unknown_source() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board view");
    let counts = view.counts();
    assert_eq!(
        (
            counts.layers,
            counts.nets,
            counts.properties,
            counts.footprints,
            counts.pads,
            counts.models,
            counts.segments,
            counts.vias,
            counts.zones,
            counts.graphics,
            counts.unknown_top_level,
        ),
        (3, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1)
    );
    assert_eq!(
        view.root_span().text(SOURCE).expect("root"),
        &SOURCE[25..SOURCE.len() - 1]
    );
    let unknown = view
        .unknown_top_level_forms()
        .next()
        .expect("unknown span")
        .text(SOURCE)
        .expect("unknown text");
    assert_eq!(unknown, "(future_board_data (nested \"must survive\"))");
    assert_eq!(
        view.unknown_top_level_forms().count(),
        counts.unknown_top_level
    );
}

#[test]
fn typed_iterators_decode_layers_nets_and_footprints() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board view");
    let layers = view
        .layers()
        .collect::<Result<Vec<_>, _>>()
        .expect("layers");
    assert_eq!(layers[2].ordinal, 36);
    assert_eq!(layers[2].name, "B.SilkS");
    assert_eq!(layers[2].user_name.as_deref(), Some("b.silkscreen"));

    let nets = view.nets().collect::<Result<Vec<_>, _>>().expect("nets");
    assert_eq!((nets[1].code, nets[1].name.as_str()), (1, "GND"));

    let footprint = view
        .footprints()
        .next()
        .expect("footprint")
        .expect("typed footprint");
    assert_eq!(footprint.library_link, "Demo:Part");
    assert_eq!(footprint.reference.as_deref(), Some("U1"));
    assert_eq!(footprint.value.as_deref(), Some("Demo"));
    assert_eq!(footprint.layer.as_deref(), Some("F.Cu"));
    assert_eq!(
        (footprint.at_x, footprint.at_y, footprint.angle),
        (Some(10.5), Some(20.25), Some(90.0))
    );
    assert_eq!((footprint.pad_count, footprint.model_count), (1, 1));
    assert!(view.source()[footprint.source_range].starts_with("(footprint"));
}

#[test]
fn typed_iterators_decode_nested_pads_and_models() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board view");
    let pad = view.pads().next().expect("pad").expect("typed pad");
    assert_eq!(
        (
            pad.footprint_index,
            pad.number.as_str(),
            pad.kind.as_str(),
            pad.shape.as_str(),
            pad.net.ordinal,
            pad.net.name.as_deref(),
        ),
        (0, "1", "smd", "rect", None, None)
    );
    assert_eq!(
        (pad.at_x, pad.at_y, pad.size_x, pad.size_y),
        (0.0, 0.0, 1.0, 1.0)
    );
    assert_eq!(pad.layers, ["F.Cu", "F.Paste"]);

    let model = view.models().next().expect("model").expect("typed model");
    assert_eq!(model.footprint_index, 0);
    assert_eq!(model.path, "${KICAD9_3DMODEL_DIR}/Demo.wrl");
    assert_eq!(model.offset, [1.0, 2.0, 3.0]);
    assert_eq!(model.scale, [1.5, 2.5, 3.5]);
    assert_eq!(model.rotate, [10.0, 20.0, 30.0]);
}

#[test]
fn typed_iterators_decode_routing_and_zones() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board view");
    let segment = view
        .segments()
        .next()
        .expect("segment")
        .expect("typed segment");
    assert_eq!(
        (
            segment.start_x,
            segment.start_y,
            segment.end_x,
            segment.end_y
        ),
        (1.0, 2.0, 3.0, 4.0)
    );
    assert_eq!(
        (
            segment.width,
            segment.layer.as_deref(),
            segment.net.ordinal,
            segment.net.name.as_deref()
        ),
        (Some(0.25), Some("F.Cu"), Some(1), Some("GND"))
    );

    let via = view.vias().next().expect("via").expect("typed via");
    assert_eq!(
        (via.at_x, via.at_y, via.size, via.drill),
        (5.0, 6.0, Some(0.8), Some(0.4))
    );
    assert_eq!(via.layers, ["F.Cu", "B.Cu"]);

    let zone = view.zones().next().expect("zone").expect("typed zone");
    assert_eq!(
        (zone.net.ordinal, zone.net_name.as_deref()),
        (Some(1), Some("GND"))
    );
    assert_eq!(zone.layers, ["F.Cu", "B.Cu"]);
}

#[test]
fn focused_board_edit_preserves_unknown_bytes_and_is_stable_after_reparse() {
    let limits = PcbLimits::default();
    let view = PcbView::parse(SOURCE, limits).expect("board view");
    let edit = view.set_property("Owner", "new \"owner\"").expect("edit");
    assert!(edit.changed);
    assert!(edit.source.contains("(future_setup \"preserve\")"));
    assert!(
        edit.source
            .contains("(future_footprint_data (nested \"preserve\"))")
    );
    assert!(
        edit.source
            .contains("(future_board_data (nested \"must survive\"))")
    );
    assert!(
        edit.source
            .contains("(property \"Owner\" \"new \\\"owner\\\"\")")
    );
    parse(&edit.source).expect("semantic reparse");

    let second = PcbView::parse(&edit.source, limits)
        .expect("typed reparse")
        .set_property("Owner", "new \"owner\"")
        .expect("stable write");
    assert!(!second.changed);
    assert_eq!(second.source, edit.source);
}

#[test]
fn root_family_nested_and_output_limits_fail_closed() {
    let extra_root = format!("(metadata)\n{SOURCE}");
    assert_eq!(
        PcbView::parse(&extra_root, PcbLimits::default())
            .expect_err("single root")
            .kind,
        ErrorKind::UnexpectedToken
    );
    for limits in [
        PcbLimits {
            max_layers: 2,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_pads: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_segments: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_vias: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_zones: 0,
            ..PcbLimits::default()
        },
    ] {
        assert_eq!(
            PcbView::parse(SOURCE, limits)
                .expect_err("family limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }
    let view = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_output_bytes: 1,
            ..PcbLimits::default()
        },
    )
    .expect("read remains allowed");
    assert_eq!(
        view.set_property("Owner", "new")
            .expect_err("output limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let view = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_pad_children: 2,
            ..PcbLimits::default()
        },
    )
    .expect("pad child limit remains lazy");
    assert_eq!(
        view.pads()
            .next()
            .expect("pad")
            .expect_err("pad child limit")
            .kind,
        ErrorKind::ResourceLimit
    );
    let view = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_model_children: 2,
            ..PcbLimits::default()
        },
    )
    .expect("model child limit remains lazy");
    assert_eq!(
        view.models()
            .next()
            .expect("model")
            .expect_err("model child limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn malformed_net_table_records_report_absolute_positions() {
    let source = "# prefix\n(kicad_pcb\n  (net wrong \"GND\")\n)\n";
    let error = PcbView::parse(source, PcbLimits::default()).expect_err("bad net code");
    let position = error.position.expect("absolute position");
    assert_eq!(position.offset, source.find("wrong").expect("offset"));
    assert_eq!(position.line, 3);
    assert_eq!(position.column, 8);
}

#[test]
fn board_net_table_resolves_ordinal_name_unknown_and_empty_references_once() {
    let source = r#"(kicad_pcb
  (net 1 "GND")
  (net 2 "SIG")
  (footprint "Demo"
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1))
    (pad "2" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net "SIG"))
    (pad "3" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 99))
    (pad "4" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net "MISSING"))
    (pad "5" smd rect (at 0 0) (size 1 1) (layers "F.Cu")))
)"#;
    let view = PcbView::parse(source, PcbLimits::default()).expect("board view");
    let pads = view.pads().collect::<Result<Vec<_>, _>>().expect("pads");
    assert_eq!(
        pads.iter().map(|pad| pad.net.clone()).collect::<Vec<_>>(),
        [
            kicad_monkey_core::PcbNetRef {
                ordinal: Some(1),
                name: Some("GND".to_owned()),
            },
            kicad_monkey_core::PcbNetRef {
                ordinal: Some(2),
                name: Some("SIG".to_owned()),
            },
            kicad_monkey_core::PcbNetRef {
                ordinal: Some(99),
                name: None,
            },
            kicad_monkey_core::PcbNetRef {
                ordinal: None,
                name: Some("MISSING".to_owned()),
            },
            kicad_monkey_core::PcbNetRef::default(),
        ]
    );
}

#[test]
fn remaining_board_carriers_are_typed_in_source_order() {
    let view = PcbView::parse(CARRIERS, PcbLimits::default()).expect("board view");
    let counts = view.counts();
    assert_eq!(
        (
            counts.graphics,
            counts.gr_texts,
            counts.gr_lines,
            counts.gr_rects,
            counts.gr_arcs,
            counts.gr_circles,
            counts.gr_polys,
            counts.gr_curves,
            counts.gr_text_boxes,
        ),
        (8, 1, 1, 1, 1, 1, 1, 1, 1)
    );
    assert_eq!(
        (
            counts.arcs,
            counts.dimensions,
            counts.groups,
            counts.generated_items,
            counts.embedded_files,
        ),
        (1, 1, 1, 1, 1)
    );
    let graphics = view
        .graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("graphics");
    assert_eq!(graphics[0].text.as_deref(), Some("hello"));
    assert_eq!(graphics[1].start.expect("start").x, 1.0);
    assert_eq!(graphics[1].stroke_width, Some(0.2));
    assert_eq!(graphics[1].stroke_kind.as_deref(), Some("dash"));
    assert_eq!(graphics[4].center.expect("center").x, 10.0);
    assert_eq!(graphics[5].points.len(), 3);
    assert_eq!(graphics[6].points.len(), 4);
}

#[test]
fn routing_arcs_and_dimensions_are_typed() {
    let view = PcbView::parse(CARRIERS, PcbLimits::default()).expect("board view");
    let arc = view.arcs().next().expect("arc").expect("typed arc");
    assert_eq!((arc.start.x, arc.mid.y, arc.end.x), (0.0, 1.0, 2.0));
    assert_eq!(
        (arc.net.ordinal, arc.net.name.as_deref()),
        (Some(1), Some("GND"))
    );

    let dimension = view
        .dimensions()
        .next()
        .expect("dimension")
        .expect("typed dimension");
    assert_eq!(dimension.kind, "aligned");
    assert_eq!(dimension.points.len(), 2);
    assert!(dimension.locked);
    assert_eq!(dimension.orientation, Some(1));
}

#[test]
fn groups_generated_items_and_embedded_files_are_typed() {
    let view = PcbView::parse(CARRIERS, PcbLimits::default()).expect("board view");
    let group = view.groups().next().expect("group").expect("typed group");
    assert_eq!(group.name, "Review");
    assert_eq!(group.uuid.as_deref(), Some("group-id"));
    assert!(group.locked);
    assert_eq!(group.members, ["line-id", "arc-id"]);

    let generated = view
        .generated_items()
        .next()
        .expect("generated")
        .expect("typed generated");
    assert_eq!(generated.kind.as_deref(), Some("tuned_delay"));
    assert_eq!(generated.name.as_deref(), Some("Tune"));
    assert_eq!(generated.property_heads, ["corner_radius"]);

    let file = view
        .embedded_files()
        .next()
        .expect("file")
        .expect("typed file");
    assert_eq!(file.name, "asset.step");
    assert_eq!(file.file_type, "model");
    assert_eq!(file.checksum.as_deref(), Some("abcd"));
    assert_eq!(file.encoded_data_bytes, 8);
}

#[test]
fn identified_top_level_removal_is_source_preserving_and_idempotent() {
    let limits = PcbLimits::default();
    let view = PcbView::parse(CARRIERS, limits).expect("board view");
    let removed = view
        .remove_top_level_by_id("group-id")
        .expect("remove group");
    assert!(removed.changed);
    assert!(!removed.source.contains("(group \"Review\""));
    assert!(removed.source.contains("(generated (id generated-id)"));
    assert!(removed.source.contains("(file (name \"asset.step\")"));
    parse(&removed.source).expect("removed source remains valid");

    let second = PcbView::parse(&removed.source, limits)
        .expect("reparse")
        .remove_top_level_by_id("group-id")
        .expect("stable absent removal");
    assert!(!second.changed);
    assert_eq!(second.source, removed.source);

    let strict = PcbView::parse(
        &removed.source,
        PcbLimits {
            max_output_bytes: removed.source.len() - 1,
            ..limits
        },
    )
    .expect("strict reparse");
    assert_eq!(
        strict
            .remove_top_level_by_id("group-id")
            .expect_err("idempotent output must remain bounded")
            .kind,
        ErrorKind::ResourceLimit
    );

    let duplicate = CARRIERS.replace("(uuid line-id)", "(uuid group-id)");
    let view = PcbView::parse(&duplicate, limits).expect("duplicate view");
    assert_eq!(
        view.remove_top_level_by_id("group-id")
            .expect_err("ambiguous identifier")
            .kind,
        ErrorKind::UnexpectedToken
    );

    let prefix = CARRIERS
        .strip_suffix(')')
        .expect("synthetic board terminator");
    let unknown_duplicate = format!("{prefix}  (future_item (uuid group-id) (payload keep))\n)\n");
    let view = PcbView::parse(&unknown_duplicate, limits).expect("future-form view");
    assert_eq!(
        view.remove_top_level_by_id("group-id")
            .expect_err("future duplicate must be ambiguous")
            .kind,
        ErrorKind::UnexpectedToken
    );

    let unknown = "(kicad_pcb\n  (future_item (uuid future-id) (payload keep))\n)\n";
    let view = PcbView::parse(unknown, limits).expect("unknown view");
    let edit = view
        .remove_top_level_by_id("future-id")
        .expect("remove identified future form");
    assert!(edit.changed);
    assert!(!edit.source.contains("future_item"));
    parse(&edit.source).expect("future removal reparses");
}

#[test]
fn expanded_carrier_and_nested_collection_limits_fail_closed() {
    for limits in [
        PcbLimits {
            max_graphics: 7,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_arcs: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_dimensions: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_groups: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_generated_items: 0,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_embedded_files: 0,
            ..PcbLimits::default()
        },
    ] {
        assert_eq!(
            PcbView::parse(CARRIERS, limits)
                .expect_err("carrier limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let view = PcbView::parse(
        CARRIERS,
        PcbLimits {
            max_graphic_points: 2,
            ..PcbLimits::default()
        },
    )
    .expect("point limit remains lazy");
    assert_eq!(
        view.graphics()
            .nth(5)
            .expect("polygon")
            .expect_err("point limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let view = PcbView::parse(
        CARRIERS,
        PcbLimits {
            max_members: 1,
            ..PcbLimits::default()
        },
    )
    .expect("member limit remains lazy");
    assert_eq!(
        view.groups()
            .next()
            .expect("group")
            .expect_err("member limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[path = "support/source_authoring/footprint_parity.rs"]
mod parity;

use kicad_monkey_core::{
    ErrorKind, FootprintLimits, FootprintView, PcbFootprintMemberOwner, PcbLimits, PcbView, parse,
};

const SOURCE: &str = r#"# header retained
(footprint "Demo:Part"
  (version 20240108)
  (generator pcbnew)
  (property "Reference" "REF**" (at 0 0 0) (layer "F.SilkS"))
  (property "Value" "old value" (at 0 1 0) (layer "F.Fab"))
  (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu" "F.Paste"))
  (future_extension (nested "must survive"))
)
"#;

#[test]
fn typed_view_reads_name_properties_and_pads_without_a_generic_tree() {
    let view = FootprintView::parse(SOURCE, FootprintLimits::default()).expect("typed view");
    assert_eq!(view.name().expect("name"), "Demo:Part");
    assert_eq!(view.pad_count(), 1);
    let properties = view
        .properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("properties");
    assert_eq!(properties.len(), 2);
    assert_eq!(properties[1].name, "Value");
    assert_eq!(properties[1].value, "old value");
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "one source fixture verifies all three standalone text carrier families"
)]
fn typed_view_decodes_standalone_text_carriers_without_board_ownership() {
    let source = r#"(footprint "Text"
      (property "Reference" "REF**" (at 1 2 30) (layer "F.SilkS")
        (effects (font (size 1 2) bold)) (uuid property-id)
        (render_cache "REF**" 0))
      (fp_text user "hello" (at 3 4 45) (layer "B.SilkS") hide
        (tstamp text-id) (render_cache "hello" 45))
      (fp_text_box "boxed" (pts (xy -1 -2) (xy 9) (xy 3 4))
        (margins 0.1 0.2 0.3 0.4) (layer "F.Fab")
        (stroke (width 0.2) (type dash)) (border yes) (uuid box-id)
        (render_cache "boxed" 0)))"#;
    let view = FootprintView::parse(source, FootprintLimits::default()).expect("typed text view");
    let properties = view
        .graphical_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("graphical properties");
    assert_eq!((properties[0].at_x, properties[0].angle), (1.0, 30.0));
    assert_eq!(properties[0].effects.font.size_x, 2.0);
    assert!(properties[0].effects.font.bold);
    assert_eq!(properties[0].uuid.as_deref(), Some("property-id"));
    assert!(properties[0].render_cache_range.is_some());

    let texts = view.texts().collect::<Result<Vec<_>, _>>().expect("texts");
    assert_eq!(texts[0].layer, "B.SilkS");
    assert!(texts[0].hidden);
    assert_eq!(texts[0].uuid.as_deref(), Some("text-id"));
    assert!(texts[0].render_cache_range.is_some());

    let boxes = view
        .text_boxes()
        .collect::<Result<Vec<_>, _>>()
        .expect("text boxes");
    assert_eq!(
        (
            boxes[0].start_x,
            boxes[0].start_y,
            boxes[0].end_x,
            boxes[0].end_y
        ),
        (-1.0, -2.0, 3.0, 4.0)
    );
    assert_eq!(boxes[0].border, Some(true));
    assert_eq!(boxes[0].stroke_kind.as_deref(), Some("dash"));
    assert_eq!(boxes[0].uuid.as_deref(), Some("box-id"));
    assert!(boxes[0].render_cache_range.is_some());
    assert_eq!(boxes[0].polygon_points, [[-1.0, -2.0], [3.0, 4.0]]);

    let error = FootprintView::parse(
        source,
        FootprintLimits {
            max_text_carriers: 2,
            ..FootprintLimits::default()
        },
    )
    .expect_err("aggregate standalone text-carrier limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn standalone_text_decoder_rejects_nonfinite_numbers() {
    for source in [
        r#"(footprint "Bad" (fp_text user "x" (at 0 0 NaN)))"#,
        r#"(footprint "Bad" (property "P" "x" (at 0 0) (layer "F.Fab")
          (effects (font (color 0 0 0 inf)))))"#,
    ] {
        let view = FootprintView::parse(source, FootprintLimits::default()).expect("selected view");
        let error = if source.contains("fp_text") {
            view.texts()
                .next()
                .expect("text")
                .expect_err("finite angle")
        } else {
            view.graphical_properties()
                .next()
                .expect("property")
                .expect_err("finite color")
        };
        assert_eq!(error.kind, ErrorKind::UnexpectedToken);
        assert!(error.message.contains("finite"));
    }
}

#[test]
fn focused_edit_preserves_unknown_bytes_and_has_a_stable_second_write() {
    let limits = FootprintLimits::default();
    let view = FootprintView::parse(SOURCE, limits).expect("typed view");
    let edit = view
        .set_property("Value", "new \"value\"", limits.max_output_bytes)
        .expect("focused edit");
    assert!(edit.changed);
    assert!(
        edit.source
            .contains("(future_extension (nested \"must survive\"))")
    );
    assert!(
        edit.source
            .contains("(property \"Value\" \"new \\\"value\\\"\"")
    );
    parse(&edit.source).expect("edited source remains semantically parseable");

    let second = FootprintView::parse(&edit.source, limits)
        .expect("reparse")
        .set_property("Value", "new \"value\"", limits.max_output_bytes)
        .expect("stable edit");
    assert!(!second.changed);
    assert_eq!(second.source, edit.source);
}

#[test]
fn typed_view_and_writer_fail_closed_on_limits_and_ambiguous_properties() {
    let limits = FootprintLimits {
        max_pads: 0,
        ..FootprintLimits::default()
    };
    assert_eq!(
        FootprintView::parse(SOURCE, limits)
            .expect_err("pad limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let duplicate = SOURCE.replace(
        "  (property \"Value\" \"old value\" (at 0 1 0) (layer \"F.Fab\"))",
        "  (property \"Value\" \"old value\" (at 0 1 0) (layer \"F.Fab\"))\n  (property \"Value\" \"duplicate\")",
    );
    let view = FootprintView::parse(&duplicate, FootprintLimits::default()).expect("typed view");
    assert_eq!(
        view.set_property("Value", "new", usize::MAX)
            .expect_err("duplicate must fail")
            .kind,
        ErrorKind::UnexpectedToken
    );

    let view = FootprintView::parse(SOURCE, FootprintLimits::default()).expect("typed view");
    assert_eq!(
        view.set_property("Value", "expanded", 1)
            .expect_err("output limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn typed_view_requires_one_footprint_root_and_ignores_nested_foreign_properties() {
    let extra_root = format!("(metadata (property \"Value\" \"wrong\"))\n{SOURCE}");
    assert_eq!(
        FootprintView::parse(&extra_root, FootprintLimits::default())
            .expect_err("extra top-level form")
            .kind,
        ErrorKind::UnexpectedToken
    );

    let foreign_child = SOURCE.replace(
        "  (future_extension (nested \"must survive\"))",
        "  (metadata (property \"Value\" \"wrong\"))\n  (future_extension (nested \"must survive\"))",
    );
    let view =
        FootprintView::parse(&foreign_child, FootprintLimits::default()).expect("typed view");
    let properties = view
        .properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("properties");
    assert_eq!(properties.len(), 2);
    let edit = view
        .set_property("Value", "right", usize::MAX)
        .expect("focused edit");
    assert!(
        edit.source
            .contains("(metadata (property \"Value\" \"wrong\"))")
    );
    assert!(edit.source.contains("(property \"Value\" \"right\""));
}

#[test]
fn lazy_property_errors_report_absolute_source_positions() {
    let source = "# prefix\n(footprint \"Demo\"\n  (property \"Value\")\n)\n";
    let view = FootprintView::parse(source, FootprintLimits::default()).expect("typed view");
    let error = view
        .properties()
        .next()
        .expect("property")
        .expect_err("missing value");
    let property_start = source.find("(property").expect("property offset");
    let closing_offset = property_start
        + source[property_start..]
            .find(')')
            .expect("property closing offset");
    let position = error.position.expect("absolute position");
    assert_eq!(position.offset, closing_offset);
    assert_eq!(position.line, 3);
    assert_eq!(position.column, 20);
}

#[test]
fn standalone_members_match_the_same_definition_embedded_on_a_board() {
    let members = r#"
  (descr "Typed parity")
  (tags "standalone demo")
  (attr through_hole)
  (component_classes (class "Power"))
  (private_layers "F.Cu" "In1.Cu")
  (net_tie_pad_groups "1, 2")
  (duplicate_pad_numbers_are_jumpers yes)
  (jumper_pad_groups ("1" "2"))
  (solder_mask_margin 0.08)
  (thermal_width 0.31)
  (thermal_gap 0.21)
  (property "Reference" "J1" (at 0 -2 0) (layer "F.SilkS")
    (effects (font (size 1 1) (thickness 0.15)) (justify left))
    (uuid reference-id) (render_cache "J1" 0))
  (property "Value" "Connector" (at 0 2 0) (layer "F.Fab"))
  (fp_text user "LOCAL" (at 1 -1 15) (layer "B.SilkS" knockout)
    (effects (font (size 1.2 1.1) italic) (justify right mirror))
    (uuid text-id) (render_cache "LOCAL" 15))
  (fp_text_box "BOX" (start -2 -2) (end 2 2) (margins 0.1 0.2 0.3 0.4)
    (angle 20) (layer "F.Fab") (effects (font (size 0.8 0.9)))
    (stroke (width 0.12) (type dash)) (border yes) (uuid box-id)
    (render_cache "BOX" 20))
  (fp_line (start -2 -1) (end 2 -1)
    (stroke (width 0.15) (type solid)) (layer "F.SilkS") (uuid line-id))
  (fp_curve (pts (xy 0 0) (xy 1 1) (xy 2 1) (xy 3 0))
    (stroke (width 0.1) (type solid)) (layer "F.Fab") (uuid curve-id))
  (pad "1" thru_hole oval (at 1 2 30) (size 2 3)
    (drill oval 0.8 1.4 (offset 0.1 0.2))
    (layers "*.Cu" "*.Mask") (net 1 "GND")
    (remove_unused_layers yes) (uuid pad-1))
  (pad "" np_thru_hole circle (at -1 0) (size 1.2 1.2)
    (drill 1) (layers "*.Cu" "*.Mask") (uuid npth-1))
  (pad "2" smd custom (at 3 0) (size 1 1) (layers "F.Cu" "F.Mask")
    (options (clearance outline) (anchor rect))
    (primitives (gr_poly (pts (xy 0 0) (xy 1 0) (xy 0 1)) (width 0) (fill yes)))
    (uuid custom-2))
  (model "${KICAD9_3DMODEL_DIR}/Connector.step"
    (offset (xyz 1 2 3)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 90)))
"#;
    let standalone_source = format!("(footprint \"Parity\"{members})");
    let board_source = format!("(kicad_pcb (footprint \"Parity\"{members}))");
    let standalone =
        FootprintView::parse(&standalone_source, FootprintLimits::default()).expect("standalone");
    let board = PcbView::parse(&board_source, PcbLimits::default()).expect("board");

    parity::assert_metadata(&standalone, &board, &standalone_source);
    parity::assert_pads(&standalone, &board);
    parity::assert_models_and_graphics(&standalone, &board);
    parity::assert_properties_and_text(&standalone, &board);
    parity::assert_text_boxes(&standalone, &board);
}

#[test]
fn standalone_member_limits_bound_nested_pad_and_model_work() {
    let source = r#"(footprint "Limited"
      (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu"))
      (model "asset.step" (offset (xyz 0 0 0)) (scale (xyz 1 1 1))
        (rotate (xyz 0 0 0))))"#;
    let exact = FootprintView::parse(
        source,
        FootprintLimits {
            max_object_nodes: 3,
            ..FootprintLimits::default()
        },
    )
    .expect("exact nested limit");
    exact.pads().next().expect("pad").expect("exact pad");
    exact.models().next().expect("model").expect("exact model");

    let one_under = FootprintView::parse(
        source,
        FootprintLimits {
            max_object_nodes: 2,
            ..FootprintLimits::default()
        },
    )
    .expect("lazy view");
    assert_eq!(
        one_under
            .pads()
            .next()
            .expect("pad")
            .expect_err("pad child limit")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(
        one_under
            .models()
            .next()
            .expect("model")
            .expect_err("model child limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one matrix proves the common standalone object ceiling reaches each nested decoder"
)]
fn standalone_object_limit_bounds_scalar_and_policy_fanout() {
    let exact_metadata = FootprintView::parse(
        r#"(footprint "Limited" locked extra evidence)"#,
        FootprintLimits {
            max_object_nodes: 4,
            ..FootprintLimits::default()
        },
    )
    .expect("metadata view");
    exact_metadata.metadata().expect("exact footprint header");
    let limited_metadata = FootprintView::parse(
        r#"(footprint "Limited" locked extra evidence)"#,
        FootprintLimits {
            max_object_nodes: 3,
            ..FootprintLimits::default()
        },
    )
    .expect("lazy metadata view");
    assert_eq!(
        limited_metadata
            .metadata()
            .expect_err("footprint header scalar limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let property_source = r#"(footprint "Limited" (property "Reference" "R1" extra evidence))"#;
    FootprintView::parse(
        property_source,
        FootprintLimits {
            max_object_nodes: 4,
            ..FootprintLimits::default()
        },
    )
    .expect("exact property metadata view")
    .metadata()
    .expect("exact property header");
    let error = FootprintView::parse(
        property_source,
        FootprintLimits {
            max_object_nodes: 3,
            ..FootprintLimits::default()
        },
    )
    .expect("one-under property metadata view")
    .metadata()
    .expect_err("metadata property header scalar limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    for source in [
        r#"(footprint "Limited" (pad "1" smd rect (layers A B C D)))"#,
        r#"(footprint "Limited" (pad "1" smd rect (net 1 GND extra evidence)))"#,
        r#"(footprint "Limited" (pad "1" smd rect
          (teardrops (best_length_ratio 0.5) (max_length 1)
            (best_width_ratio 0.5) (max_width 2))))"#,
        r#"(footprint "Limited" (pad "1" smd rect
          (zone_layer_connections A B C D)))"#,
    ] {
        let exact = FootprintView::parse(
            source,
            FootprintLimits {
                max_object_nodes: 4,
                ..FootprintLimits::default()
            },
        )
        .expect("exact pad view");
        exact.pads().next().expect("pad").expect("exact pad limit");

        let one_under = FootprintView::parse(
            source,
            FootprintLimits {
                max_object_nodes: 3,
                ..FootprintLimits::default()
            },
        )
        .expect("one-under pad view");
        assert_eq!(
            one_under
                .pads()
                .next()
                .expect("pad")
                .expect_err("nested pad scalar or policy limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let model_source = r#"(footprint "Limited"
      (model "asset.step" extra evidence tokens)
      (model "xyz.step" (offset (xyz 1 2 3 4))))"#;
    let exact_models = FootprintView::parse(
        model_source,
        FootprintLimits {
            max_object_nodes: 4,
            ..FootprintLimits::default()
        },
    )
    .expect("exact model view")
    .models()
    .collect::<Result<Vec<_>, _>>()
    .expect("exact model scalar limits");
    assert_eq!(exact_models.len(), 2);
    let limited_models = FootprintView::parse(
        model_source,
        FootprintLimits {
            max_object_nodes: 3,
            ..FootprintLimits::default()
        },
    )
    .expect("lazy model view")
    .models()
    .collect::<Vec<_>>();
    assert!(limited_models.iter().all(|model| {
        model
            .as_ref()
            .is_err_and(|error| error.kind == ErrorKind::ResourceLimit)
    }));

    let graphic_source = r#"(footprint "Limited" (fp_line border extra authored tokens))"#;
    FootprintView::parse(
        graphic_source,
        FootprintLimits {
            max_object_nodes: 4,
            ..FootprintLimits::default()
        },
    )
    .expect("exact graphic view")
    .graphics()
    .next()
    .expect("graphic")
    .expect("exact graphic header");
    let error = FootprintView::parse(
        graphic_source,
        FootprintLimits {
            max_object_nodes: 3,
            ..FootprintLimits::default()
        },
    )
    .expect("one-under graphic view")
    .graphics()
    .next()
    .expect("graphic")
    .expect_err("graphic header scalar limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn standalone_local_coordinates_remain_separate_from_board_occurrence_transforms() {
    let standalone_source =
        r#"(footprint "Local" (pad "1" smd rect (at 1 2 30) (size 1 1) (layers "F.Cu")))"#;
    let board_source = r#"(kicad_pcb
      (footprint "Local" (layer "F.Cu") (at 10 20 45) (uuid top-id)
        (pad "1" smd rect (at 1 2 30) (size 1 1) (layers "F.Cu")))
      (footprint "Local" (layer "B.Cu") (at 30 40 90) (uuid bottom-id)
        (pad "1" smd rect (at 1 2 30) (size 1 1) (layers "F.Cu"))))"#;
    let standalone =
        FootprintView::parse(standalone_source, FootprintLimits::default()).expect("standalone");
    let standalone_pad = standalone.pads().next().expect("pad").expect("typed pad");
    assert_eq!(
        standalone_pad.owner,
        PcbFootprintMemberOwner::StandaloneFootprint
    );
    assert_eq!((standalone_pad.at_x, standalone_pad.at_y), (1.0, 2.0));

    let board = PcbView::parse(board_source, PcbLimits::default()).expect("board");
    let pads = board.pads().collect::<Result<Vec<_>, _>>().expect("pads");
    assert_eq!((pads[0].at_x, pads[0].at_y), (1.0, 2.0));
    assert_eq!((pads[1].at_x, pads[1].at_y), (1.0, 2.0));
    assert_eq!(
        pads[1].owner,
        PcbFootprintMemberOwner::EmbeddedFootprint { footprint_index: 1 }
    );
    let transforms = board
        .footprint_transforms()
        .collect::<Result<Vec<_>, _>>()
        .expect("occurrence transforms");
    assert_eq!(
        (transforms[0].x, transforms[0].y, transforms[0].angle),
        (10.0, 20.0, 45.0)
    );
    assert_eq!(transforms[1].layer, "B.Cu");
    assert_eq!(
        (transforms[1].x, transforms[1].y, transforms[1].angle),
        (30.0, 40.0, 90.0)
    );
}

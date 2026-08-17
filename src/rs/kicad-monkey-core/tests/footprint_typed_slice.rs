use kicad_monkey_core::{ErrorKind, FootprintLimits, FootprintView, parse};

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
fn typed_view_decodes_standalone_text_carriers_without_board_ownership() {
    let source = r#"(footprint "Text"
      (property "Reference" "REF**" (at 1 2 30) (layer "F.SilkS")
        (effects (font (size 1 2) bold)))
      (fp_text user "hello" (at 3 4 45) (layer "B.SilkS") hide)
      (fp_text_box "boxed" (pts (xy -1 -2) (xy 9) (xy 3 4))
        (margins 0.1 0.2 0.3 0.4) (layer "F.Fab") (border yes)))"#;
    let view = FootprintView::parse(source, FootprintLimits::default()).expect("typed text view");
    let properties = view
        .graphical_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("graphical properties");
    assert_eq!((properties[0].at_x, properties[0].angle), (1.0, 30.0));
    assert_eq!(properties[0].effects.font.size_x, 2.0);
    assert!(properties[0].effects.font.bold);

    let texts = view.texts().collect::<Result<Vec<_>, _>>().expect("texts");
    assert_eq!(texts[0].layer, "B.SilkS");
    assert!(texts[0].hidden);

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

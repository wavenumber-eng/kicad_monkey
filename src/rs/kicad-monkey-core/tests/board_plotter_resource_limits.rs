use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, ErrorKind, board_plot_document,
    board_plot_document_with_net_classes,
};

#[test]
fn graphic_output_budget_does_not_limit_board_properties() {
    let source = r#"(kicad_pcb (property "Revision" "A"))"#;
    let document = board_plot_document(
        source,
        BoardPlotLimits {
            max_graphics: 0,
            max_operations: 0,
            max_points: 0,
            ..BoardPlotLimits::default()
        },
    )
    .expect("properties use their own fixed structural ceiling");
    assert!(document.records.is_empty());
}

#[test]
fn structural_input_points_are_independent_from_emitted_points() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (zone (net 0) (layers "*.Cu") (uuid "keepout") (hatch edge 0.5)
        (keepout (tracks not_allowed))
        (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))))"#;
    let zero_output = BoardPlotLimits {
        max_operations: 0,
        max_points: 0,
        ..BoardPlotLimits::default()
    };
    let document = board_plot_document(source, zero_output)
        .expect("non-emitted outline points do not consume output budgets");
    assert_eq!(document.records[0].operation_count(), 0);

    let limited_input = BoardPlotLimits {
        max_operations: 0,
        max_points: 0,
        max_input_points: 2,
        ..BoardPlotLimits::default()
    };
    let error = board_plot_document(source, limited_input)
        .expect_err("decoded outline points use the structural input ceiling");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("max_input_points"));

    let limited_polygons = BoardPlotLimits {
        max_operations: 0,
        max_points: 0,
        max_input_polygons: 0,
        ..BoardPlotLimits::default()
    };
    let error = board_plot_document(source, limited_polygons)
        .expect_err("decoded outlines use the structural polygon ceiling");
    assert!(error.message.contains("max_input_polygons"));
}

#[test]
fn cache_specific_wrapping_is_explicitly_deferred() {
    let source = r#"(kicad_pcb
      (gr_text_box "A A" (start 0 0) (end 5.19 2) (angle 90)
        (effects (font (size 1 2.1)))
        (render_cache "A
A" 90
          (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))))"#;
    let error = board_plot_document(source, BoardPlotLimits::default())
        .expect_err("outline-font cache wrapping is outside this slice");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("outline-font bridge"));

    let stale_source = r#"(kicad_pcb
      (gr_text_box "A A" (start 0 0) (end 4 2)
        (effects (font (size 1 2.1)))
        (render_cache "A A" 0
          (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))))"#;
    let error = board_plot_document(stale_source, BoardPlotLimits::default())
        .expect_err("potentially wrapping cache text is deferred even when source text matches");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
}

#[test]
fn retained_net_class_strings_are_preflighted_before_repeated_clones() {
    let source = r#"(kicad_pcb
      (net 1 "GND")
      (segment (start 0 0) (end 1 0) (width 0.1) (layer "F.Cu") (net 1))
      (segment (start 0 1) (end 1 1) (width 0.1) (layer "F.Cu") (net 1)))"#;
    let classes = BoardNetClassAssignments::from_entries([("GND", vec!["abc", "de"])]);
    let per_record = 8 + 3 * std::mem::size_of::<String>();
    board_plot_document_with_net_classes(
        source,
        BoardPlotLimits {
            max_net_class_bytes: 2 * per_record,
            ..BoardPlotLimits::default()
        },
        &classes,
    )
    .expect("exact retained class budget is inclusive");
    let error = board_plot_document_with_net_classes(
        source,
        BoardPlotLimits {
            max_net_class_bytes: 2 * per_record - 1,
            ..BoardPlotLimits::default()
        },
        &classes,
    )
    .expect_err("second record is rejected before cloning class strings");
    assert!(error.message.contains("max_net_class_bytes"));
}

#[test]
fn emitted_geometry_budget_is_checked_before_coordinate_conversion() {
    let source = r#"(kicad_pcb
      (gr_poly (pts (xy 100000000000000000000 0) (xy 1 0) (xy 1 1))
        (stroke (width 0.1) (type solid)) (fill none)))"#;
    let error = board_plot_document(
        source,
        BoardPlotLimits {
            max_points: 0,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("known point count is rejected before converting coordinates");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("max_points"));
}

#[test]
fn text_box_retained_budget_uses_the_wrapped_payload() {
    let source = r#"(kicad_pcb
      (gr_text_box "A  A " (start 0 0) (end 1.8 2)
        (effects (font (size 1 1.27)))))"#;
    board_plot_document(
        source,
        BoardPlotLimits {
            max_text_bytes: 8,
            ..BoardPlotLimits::default()
        },
    )
    .expect("two wrapped four-byte copies fit exactly");
    let error = board_plot_document(
        source,
        BoardPlotLimits {
            max_text_bytes: 7,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("wrapped retained text fails one byte under");
    assert!(error.message.contains("max_text_bytes"));
}

#[test]
fn knockout_final_point_shape_is_preflighted_from_the_raw_cache() {
    let source = r#"(kicad_pcb
      (gr_text "KO" (at 0 0) (layer "F.SilkS" knockout)
        (effects (font (size 1 1) (thickness 0.2)))
        (render_cache "KO" 0
          (polygon
            (pts (xy 0 0) (xy 1 0) (xy 1 1))
            (pts (xy 0.2 0.2) (xy 0.8 0.2) (xy 0.8 0.8))))))"#;
    board_plot_document(
        source,
        BoardPlotLimits {
            max_points: 14,
            ..BoardPlotLimits::default()
        },
    )
    .expect("glyph contours plus both background copies fit exactly");
    let error = board_plot_document(
        source,
        BoardPlotLimits {
            max_points: 13,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("knockout final shape is rejected before typed conversion");
    assert!(error.message.contains("max_points"));

    let many_exteriors = r#"(kicad_pcb
      (gr_text "KO" (at 0 0) (layer "F.SilkS" knockout)
        (effects (font (size 1 1) (thickness 0.2)))
        (render_cache "KO" 0
          (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
          (polygon (pts (xy 2 0) (xy 3 0) (xy 3 1)))
          (polygon (pts (xy 4 0) (xy 5 0) (xy 5 1))))))"#;
    board_plot_document(
        many_exteriors,
        BoardPlotLimits {
            max_points: 17,
            ..BoardPlotLimits::default()
        },
    )
    .expect("knockout skips transient authored exterior mirrors");
}

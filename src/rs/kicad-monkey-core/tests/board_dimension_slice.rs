use kicad_monkey_core::{
    BoardDimensionOperation, BoardPlotLimits, BoardPlotRecord, ErrorKind, PcbLimits, PcbView,
    board_plot_document,
};

fn dimension_records(source: &str) -> Vec<kicad_monkey_core::BoardDimensionRecord> {
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("dimension plot document");
    document
        .records
        .into_iter()
        .filter_map(|record| match record {
            BoardPlotRecord::Dimension(record) => Some(record),
            _ => None,
        })
        .collect()
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "one source carrier asserts every independently parsed dimension format/style field"
)]
fn dimension_parser_preserves_format_style_text_and_python_defaults() {
    let source = r#"(kicad_pcb
      (dimension (pts (xy 0 0) (xy 9) (xy 5 0))
        (format (prefix "[") (suffix "]") (units 0) (units_format 2)
          (precision 8) (override_value "OVERRIDE") (suppress_zeroes YES))
        (style (thickness 0.3) (arrow_length 1.1) (text_position_mode 2)
          (arrow_direction inward) (extension_height 0.4) (extension_offset 0.2)
          (keep_text_aligned TrUe) (text_frame 1))
        (gr_text "authored" (at) (uuid text-id))))"#;
    let view = PcbView::parse(source, PcbLimits::default()).expect("board view");
    let dimension = view
        .dimensions()
        .next()
        .expect("dimension")
        .expect("typed dimension");
    assert_eq!(dimension.kind, "aligned");
    assert_eq!(dimension.layer, "Cmts.User");
    assert_eq!(dimension.points.len(), 2, "incomplete xy is ignored");
    assert_eq!(dimension.format.prefix, "[");
    assert_eq!(dimension.format.suffix, "]");
    assert_eq!(dimension.format.units, 0);
    assert_eq!(dimension.format.units_format, 2);
    assert_eq!(dimension.format.precision, 8);
    assert_eq!(dimension.format.override_value.as_deref(), Some("OVERRIDE"));
    assert!(dimension.format.suppress_zeroes);
    assert_eq!(dimension.style.thickness, 0.3);
    assert_eq!(dimension.style.arrow_length, 1.1);
    assert_eq!(dimension.style.text_position_mode, 2);
    assert_eq!(dimension.style.arrow_direction, "inward");
    assert_eq!(dimension.style.extension_height, 0.4);
    assert_eq!(dimension.style.extension_offset, 0.2);
    assert!(dimension.style.keep_text_aligned);
    assert_eq!(dimension.style.text_frame, Some(1));
    let text = dimension.text.expect("nested text");
    assert_eq!(text.text.as_deref(), Some("authored"));
    assert_eq!(
        text.at.expect("default at"),
        kicad_monkey_core::pcb::PcbPoint { x: 0.0, y: 0.0 }
    );
    assert_eq!(text.layer.as_deref(), Some("F.SilkS"));
    assert_eq!(text.uuid.as_deref(), Some("text-id"));

    let defaults = PcbView::parse(
        "(kicad_pcb (dimension (style) (pts)))",
        PcbLimits::default(),
    )
    .expect("default board")
    .dimensions()
    .next()
    .expect("default dimension")
    .expect("typed default dimension");
    assert!(
        !defaults.style.keep_text_aligned,
        "present empty style differs from absent style"
    );
}

#[test]
fn dimension_value_formatting_matches_units_override_and_precision_rules() {
    let source = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 25.4 0))
        (format (prefix "[") (suffix "]") (units 0) (units_format 1)
          (precision 4) (suppress_zeroes yes))
        (gr_text "old" (effects (font (size 1 1)))))
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (prefix "<") (suffix ">") (units 2) (units_format 2)
          (precision 9) (override_value "custom"))
        (gr_text "old" (effects (font (size 1 1)))))
      (dimension (type center) (pts (xy 0 0) (xy 10 0))
        (format (units_format 0) (precision 0) (suppress_zeroes yes))
        (gr_text "old" (effects (font (size 1 1)))))
      (dimension (type center) (pts (xy 0 0) (xy 0 0))
        (format (units_format 0) (precision 0) (suppress_zeroes yes))
        (gr_text "old" (effects (font (size 1 1))))))"#;
    let records = dimension_records(source);
    assert_eq!(records.len(), 4);
    assert_eq!(records[0].text.as_deref(), Some("[1 in]"));
    assert_eq!(records[1].text.as_deref(), Some("<custom (mm)>"));
    assert_eq!(records[2].text.as_deref(), Some("1"));
    assert_eq!(records[3].text.as_deref(), Some(""));
}

#[test]
fn dimension_record_operation_input_and_text_limits_are_fail_closed() {
    let center = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))))"#;
    assert_eq!(dimension_records(center)[0].operations.len(), 2);

    for limits in [
        BoardPlotLimits {
            max_graphics: 0,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_operations: 1,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_input_points: 1,
            ..BoardPlotLimits::default()
        },
    ] {
        let error = board_plot_document(center, limits).expect_err("dimension limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }

    let text = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (override_value "abc") (units_format 0) (precision 0))
        (gr_text "old" (effects (font (size 1 1))))))"#;
    board_plot_document(
        text,
        BoardPlotLimits {
            max_text_bytes: 3,
            ..BoardPlotLimits::default()
        },
    )
    .expect("stroke text is retained once at the exact byte limit");
    let error = board_plot_document(
        text,
        BoardPlotLimits {
            max_text_bytes: 2,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("resolved dimension text observes max_text_bytes");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    let unused_precision = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (precision 1000000))))"#;
    let document = board_plot_document(
        unused_precision,
        BoardPlotLimits {
            max_text_bytes: 0,
            ..BoardPlotLimits::default()
        },
    )
    .expect("dimensions without nested text never format a value");
    assert_eq!(document.records[0].operation_count(), 2);
}

#[test]
fn malformed_and_extreme_dimensions_do_not_emit_partial_geometry() {
    let incomplete = r#"(kicad_pcb
      (dimension (type aligned) (pts (xy 0) (xy 1))))"#;
    let records = dimension_records(incomplete);
    assert_eq!(records.len(), 1);
    assert!(records[0].operations.is_empty());
    board_plot_document(
        incomplete,
        BoardPlotLimits {
            max_operations: 0,
            ..BoardPlotLimits::default()
        },
    )
    .expect("zero-operation dimension records fit a zero operation budget");

    let unknown = r#"(kicad_pcb
      (dimension (type future) (pts (xy 0 0) (xy 1 0))))"#;
    let error = board_plot_document(unknown, BoardPlotLimits::default())
        .expect_err("unknown dimension types fail closed");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);

    for malformed in [
        "(kicad_pcb (dimension (height nope)))",
        "(kicad_pcb (dimension (pts (xy nope 0))))",
        "(kicad_pcb (dimension (format (precision nope))))",
        "(kicad_pcb (dimension (style (arrow_length nope))))",
    ] {
        assert!(
            board_plot_document(malformed, BoardPlotLimits::default()).is_err(),
            "complete malformed dimension scalars are strict"
        );
    }

    let huge_precision = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (precision 1000000)) (gr_text "old")))"#;
    let error = board_plot_document(
        huge_precision,
        BoardPlotLimits {
            max_text_bytes: 32,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("precision is preflighted before formatting");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    let unsafe_derived = r#"(kicad_pcb
      (dimension (type center)
        (pts (xy 9007199254.740991 0) (xy 9007199254.740991 1))))"#;
    let error = board_plot_document(unsafe_derived, BoardPlotLimits::default())
        .expect_err("unsafe derived dimension coordinates fail closed");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
}

#[test]
fn faced_dimension_text_cache_and_markup_limits_are_inclusive() {
    let faced = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (override_value "abc") (units_format 0) (precision 0))
        (gr_text "old" (effects (font (face "Arial") (size 1 1))))))"#;
    board_plot_document(
        faced,
        BoardPlotLimits {
            max_operations: 3,
            max_text_bytes: 6,
            ..BoardPlotLimits::default()
        },
    )
    .expect("record and faced operation fit exact limits without a cache");
    for limits in [
        BoardPlotLimits {
            max_operations: 2,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_text_bytes: 5,
            ..BoardPlotLimits::default()
        },
    ] {
        let error = board_plot_document(faced, limits).expect_err("faced dimension limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }

    let cached = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (override_value "abc") (units_format 0) (precision 0))
        (gr_text "old" (effects (font (face "Arial") (size 1 1)))
          (render_cache "abc" 0
            (polygon (pts (xy 0 0) (xy 1 0) (xy 0 1)))))))"#;
    board_plot_document(
        cached,
        BoardPlotLimits {
            max_text_bytes: 9,
            ..BoardPlotLimits::default()
        },
    )
    .expect("record, operation, and cache text fit the exact byte limit");
    for limits in [
        BoardPlotLimits {
            max_text_bytes: 8,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_cache_polygons: 0,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_cache_contours: 0,
            ..BoardPlotLimits::default()
        },
    ] {
        let error = board_plot_document(cached, limits).expect_err("cache limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }

    let markup = r#"(kicad_pcb
      (dimension (type center) (pts (xy 0 0) (xy 1 0))
        (format (override_value "A_{B}^{C}~{D}_{E}^{F}") (units_format 0) (precision 0))
        (gr_text "old" (effects (font (size 1 1))))))"#;
    let error = board_plot_document(
        markup,
        BoardPlotLimits {
            max_parse_nodes: 8,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("dimension markup observes the parse-node ceiling");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn dimension_shape_branches_and_text_position_modes_cover_python_edges() {
    let source = r#"(kicad_pcb
      (dimension (type aligned) (pts (xy 0 0) (xy 10 0)) (height -2))
      (dimension (type aligned) (pts (xy 0 0) (xy 10 0)) (height 0))
      (dimension (type aligned) (pts (xy 1 1) (xy 1 1)) (height 2))
      (dimension (type orthogonal) (pts (xy 0 0) (xy 2 5)) (height 2) (orientation 1))
      (dimension (type radial) (pts (xy 0 0) (xy 2 0)))
      (dimension (type leader) (pts (xy 0 0) (xy 2 1)))
      (dimension (type center) (pts (xy 0 0) (xy 1 0)))
      (dimension (type aligned) (pts (xy 0 0) (xy 10 0)) (height 3)
        (format (override_value "A") (units_format 0) (precision 0))
        (style (text_position_mode 0) (keep_text_aligned yes))
        (gr_text "old" (at 9 9 45) (effects (font (face "Arial") (size 1 1)))))
      (dimension (type aligned) (pts (xy 0 0) (xy 10 0)) (height 3)
        (format (override_value "A") (units_format 0) (precision 0))
        (style (text_position_mode 1) (keep_text_aligned yes))
        (gr_text "old" (at 9 9 45) (effects (font (face "Arial") (size 1 1)))))
      (dimension (type aligned) (pts (xy 0 0) (xy 10 0)) (height 3)
        (format (override_value "A") (units_format 0) (precision 0))
        (style (text_position_mode 2) (keep_text_aligned no))
        (gr_text "old" (at 9 9 45) (effects (font (face "Arial") (size 1 1))))))"#;
    let records = dimension_records(source);
    assert_eq!(records.len(), 10);
    assert_eq!(
        records[..7]
            .iter()
            .map(|record| record.operations.len())
            .collect::<Vec<_>>(),
        [7, 7, 0, 7, 5, 3, 2]
    );

    let text_operation = |record: &kicad_monkey_core::BoardDimensionRecord| {
        let BoardDimensionOperation::Text(operation) = &record.operations[0] else {
            panic!("leading faced dimension Text")
        };
        (operation.x, operation.y, operation.orient_deg)
    };
    let automatic = text_operation(&records[7]);
    assert_eq!(automatic, (5_000_000, 1_875_000, 0.0));
    let midpoint = text_operation(&records[8]);
    assert_eq!(midpoint, (5_000_000, 3_000_000, 0.0));
    let authored = text_operation(&records[9]);
    assert_eq!(authored, (9_000_000, 9_000_000, 45.0));
}

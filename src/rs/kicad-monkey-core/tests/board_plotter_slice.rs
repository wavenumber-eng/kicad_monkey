use kicad_monkey_core::{
    BoardPlotLimits, BoardPlotRecordKind, ErrorKind, PlotterFill, PlotterOperation,
    board_plot_document,
};

const LINE_BOARD: &str = r#"(kicad_pcb
  (version 20240108)
  (generator pcbnew)
  (generator_version "8.0")
  (general (thickness 2.4))
  (paper "A3")
  (gr_line
    (start 0 0)
    (end 1.5 -2)
    (stroke (width 0.2) (type solid))
    (layer "Edge.Cuts")
    (uuid "line-uuid"))
)"#;

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies the complete promoted record"
)]
fn board_plotter_reads_metadata_and_solid_lines_with_record_level_layers() {
    let document =
        board_plot_document(LINE_BOARD, BoardPlotLimits::default()).expect("plotter document");
    assert_eq!(document.version, 20_240_108);
    assert_eq!(document.generator, "pcbnew");
    assert_eq!(document.generator_version, "8.0");
    assert_eq!(document.thickness_mm, 2.4);
    assert_eq!(document.paper, "A3");
    assert_eq!(document.records.len(), 1);
    let record = &document.records[0];
    assert_eq!(record.kind, BoardPlotRecordKind::GrLine);
    assert_eq!(record.uuid, "line-uuid");
    assert_eq!(record.layer, "Edge.Cuts");
    assert_eq!(record.operations.len(), 1);
    let PlotterOperation::ThickSegment(line) = &record.operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.start_x, 0);
    assert_eq!(line.start_y, 0);
    assert_eq!(line.end_x, 1_500_000);
    assert_eq!(line.end_y, -2_000_000);
    assert_eq!(line.width_nm, 200_000);
    assert_eq!(line.layer, None, "board graphic operations are layerless");
    assert_eq!(line.role, None);
    assert!(line.layers.is_empty());
}

#[test]
fn board_plotter_defaults_match_python_without_footprint_pen_clamps() {
    let document = board_plot_document("(kicad_pcb)", BoardPlotLimits::default())
        .expect("default board document");
    assert_eq!(document.version, 20_260_206);
    assert_eq!(document.generator, "pcbnew");
    assert_eq!(document.generator_version, "10.0");
    assert_eq!(document.thickness_mm, 1.6);
    assert_eq!(document.paper, "A4");
    assert!(document.records.is_empty());

    // PCB stroke widths are not clamped to the footprint plot pen minimums:
    // sub-minimum widths survive exactly and zero/absent widths stay zero.
    let thin = r#"(kicad_pcb
      (gr_line (start 0 0) (end 1 0) (stroke (width 0.05) (type solid))))"#;
    let document = board_plot_document(thin, BoardPlotLimits::default()).expect("thin stroke");
    let PlotterOperation::ThickSegment(line) = &document.records[0].operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 50_000);

    let zero = r#"(kicad_pcb (gr_line (start 0 0) (end 1 0) (stroke (type solid))))"#;
    let document = board_plot_document(zero, BoardPlotLimits::default()).expect("zero stroke");
    let PlotterOperation::ThickSegment(line) = &document.records[0].operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 0);
}

#[test]
fn legacy_width_applies_only_without_a_stroke_form() {
    let legacy = r#"(kicad_pcb (gr_line (start 0 0) (end 1 0) (width 0.3)))"#;
    let document = board_plot_document(legacy, BoardPlotLimits::default()).expect("legacy width");
    let PlotterOperation::ThickSegment(line) = &document.records[0].operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 300_000);

    // A stroke form wins entirely, even when it omits its own width.
    let shadowed =
        r#"(kicad_pcb (gr_line (start 0 0) (end 1 0) (width 0.3) (stroke (type solid))))"#;
    let document =
        board_plot_document(shadowed, BoardPlotLimits::default()).expect("shadowed legacy width");
    let PlotterOperation::ThickSegment(line) = &document.records[0].operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 0);
}

#[test]
fn dashed_board_lines_expand_inside_one_record_and_fail_closed_on_unknown_styles() {
    let source = LINE_BOARD.replace("(type solid)", "(type dash)");
    let document =
        board_plot_document(&source, BoardPlotLimits::default()).expect("dashed decomposition");
    assert_eq!(document.records.len(), 1);
    let record = &document.records[0];
    assert_eq!(record.operations.len(), 1);
    let PlotterOperation::ThickSegment(segment) = &record.operations[0] else {
        panic!("dash decomposition emits thick segments");
    };
    assert_eq!((segment.end_x, segment.end_y), (1_320_000, -1_760_000));
    assert_eq!(segment.layer, None);

    for style in ["dot", "dash_dot", "dash_dot_dot"] {
        let patterned = LINE_BOARD.replace("(type solid)", &format!("(type {style})"));
        let document = board_plot_document(&patterned, BoardPlotLimits::default())
            .expect("supported patterned stroke");
        assert!(!document.records[0].operations.is_empty(), "{style}");
        assert!(
            document.records[0]
                .operations
                .iter()
                .all(|operation| matches!(operation, PlotterOperation::ThickSegment(_))),
            "{style} decomposes to thick segments"
        );
    }

    let limited = BoardPlotLimits {
        max_operations: 0,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(&source, limited)
            .expect_err("decomposition observes operation limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let unsupported = LINE_BOARD.replace("(type solid)", "(type custom)");
    let error = board_plot_document(&unsupported, BoardPlotLimits::default())
        .expect_err("unknown stroke style must not become solid");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(
        error
            .message
            .contains("Unsupported board graphic stroke type")
    );
}

#[test]
fn patterned_decomposition_never_returns_truncated_or_zero_progress_geometry() {
    let long_segment = r#"(kicad_pcb
      (gr_line (start 0 0) (end 7000000 0)
        (stroke (width 0.1) (type dash))))"#;
    let error = board_plot_document(long_segment, BoardPlotLimits::default())
        .expect_err("long segment must not return the first 10,000 pattern steps");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("safety step limit"));

    let long_arc = r#"(kicad_pcb
      (gr_arc (start 1000000 0) (mid 0 1000000) (end -1000000 0)
        (stroke (width 0.1) (type dot))))"#;
    let error = board_plot_document(long_arc, BoardPlotLimits::default())
        .expect_err("long arc must not return the first 10,000 pattern steps");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("safety step limit"));

    let zero_progress = r#"(kicad_pcb
      (gr_line (start 0 0) (end 1 0)
        (stroke (width -1) (type dot))))"#;
    let error = board_plot_document(zero_progress, BoardPlotLimits::default())
        .expect_err("zero-width pattern cannot progress");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("forward progress"));
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies every promoted category"
)]
fn records_follow_python_category_order_with_python_layer_defaults() {
    let source = r#"(kicad_pcb
      (gr_curve (pts (xy 0 0) (xy 1 0) (xy 1 1) (xy 2 1)) (stroke (width 0.1)))
      (gr_poly (pts (xy 0 0) (xy 2 0) (xy 1 1.5)) (stroke (width 0.1)) (fill solid))
      (gr_rect (start -2 -1) (end -1 1) (stroke (width 0.05)) (fill no))
      (gr_text "skipped" (at 0 0) (layer "F.SilkS"))
      (gr_circle (center 2 2) (end 2 3.5) (stroke (width 0.12)) (fill yes))
      (gr_arc (start 1 0) (mid 0 1) (end -1 0) (stroke (width 0.15)))
      (gr_line (start 0 1) (end 3 1) (stroke (width 0.1) (type solid))))"#;
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("category grouping");
    let kinds: Vec<_> = document.records.iter().map(|record| record.kind).collect();
    assert_eq!(
        kinds,
        [
            BoardPlotRecordKind::GrLine,
            BoardPlotRecordKind::GrArc,
            BoardPlotRecordKind::GrCircle,
            BoardPlotRecordKind::GrRect,
            BoardPlotRecordKind::GrPoly,
            BoardPlotRecordKind::GrCurve,
        ],
        "gr_text is deferred to the board-text slice and records group by category"
    );
    for record in &document.records {
        let expected_layer = if record.kind == BoardPlotRecordKind::GrCurve {
            "F.SilkS"
        } else {
            "Edge.Cuts"
        };
        assert_eq!(record.layer, expected_layer, "{:?}", record.kind);
        assert_eq!(record.uuid, "");
    }

    let PlotterOperation::ArcThreePoint(arc) = &document.records[1].operations[0] else {
        panic!("expected solid arc three-point operation");
    };
    assert_eq!(arc.fill, PlotterFill::NoFill);
    let PlotterOperation::Circle(circle) = &document.records[2].operations[0] else {
        panic!("expected circle operation");
    };
    assert_eq!((circle.cx, circle.cy), (2_000_000, 2_000_000));
    assert_eq!(circle.diameter_nm, 3_000_000);
    assert_eq!(circle.fill, PlotterFill::FilledShape);
    let PlotterOperation::Rect(rect) = &document.records[3].operations[0] else {
        panic!("expected rect operation");
    };
    assert_eq!(rect.fill, PlotterFill::NoFill);
    assert_eq!(rect.corner_radius_nm, 0);
    let PlotterOperation::PlotPoly(poly) = &document.records[4].operations[0] else {
        panic!("expected polygon operation");
    };
    assert_eq!(
        poly.points,
        [[0, 0], [2_000_000, 0], [1_000_000, 1_500_000]]
    );
    assert_eq!(poly.fill, PlotterFill::FilledShape);
    let PlotterOperation::BezierCurve(curve) = &document.records[5].operations[0] else {
        panic!("expected bezier operation");
    };
    assert_eq!(curve.tolerance_nm, 0);
    assert_eq!((curve.end_x, curve.end_y), (2_000_000, 1_000_000));
}

#[test]
fn unknown_fills_stay_unfilled_like_the_lenient_python_parser() {
    let source = r#"(kicad_pcb
      (gr_circle (center 0 0) (end 1 0) (stroke (width 0.1)) (fill hatch)))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("lenient fill");
    let PlotterOperation::Circle(circle) = &document.records[0].operations[0] else {
        panic!("expected circle operation");
    };
    assert_eq!(circle.fill, PlotterFill::NoFill);
}

#[test]
fn malformed_curves_produce_empty_records_but_still_validate_strokes() {
    let source = r#"(kicad_pcb
      (gr_curve (pts (xy 0 0) (xy 1 0) (xy 1 1)) (stroke (width 0.1)) (uuid "short")))"#;
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("malformed curve");
    assert_eq!(document.records.len(), 1);
    assert_eq!(document.records[0].kind, BoardPlotRecordKind::GrCurve);
    assert_eq!(document.records[0].uuid, "short");
    assert!(document.records[0].operations.is_empty());

    let invalid = source.replace("(stroke (width 0.1))", "(stroke (width 0.1) (type wavy))");
    let error = board_plot_document(&invalid, BoardPlotLimits::default())
        .expect_err("stroke validation happens before the point-count fallback");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
}

#[test]
fn polygon_points_observe_an_aggregate_limit() {
    let source = r#"(kicad_pcb
      (gr_poly (pts (xy 0 0) (xy 2 0) (xy 1 1.5)) (stroke (width 0.1)) (fill solid)))"#;
    let exact = BoardPlotLimits {
        max_points: 3,
        ..BoardPlotLimits::default()
    };
    assert!(board_plot_document(source, exact).is_ok());
    let limited = BoardPlotLimits {
        max_points: 2,
        ..BoardPlotLimits::default()
    };
    let error =
        board_plot_document(source, limited).expect_err("polygon points observe max_points");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("max_points"));
}

#[test]
fn plotter_outputs_stay_inside_the_javascript_safe_integer_range() {
    const SAFE_MAX: i64 = 9_007_199_254_740_991;
    const SAFE_MIN: i64 = -SAFE_MAX;

    for version in [SAFE_MIN, SAFE_MAX] {
        let source = format!("(kicad_pcb (version {version}))");
        let document = board_plot_document(&source, BoardPlotLimits::default())
            .expect("safe boundary version");
        assert_eq!(document.version, version);
    }

    for version in [SAFE_MIN - 1, SAFE_MAX + 1] {
        let source = format!("(kicad_pcb (version {version}))");
        let error =
            board_plot_document(&source, BoardPlotLimits::default()).expect_err("unsafe version");
        assert_eq!(error.kind, ErrorKind::UnexpectedToken);
        assert!(error.message.contains("safe-integer"));
    }

    let outside = r#"(kicad_pcb
      (gr_line (start 9007199255 0) (end 0 0)
        (stroke (width 0.1) (type solid))))"#;
    let error =
        board_plot_document(outside, BoardPlotLimits::default()).expect_err("unsafe coordinate");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(error.message.contains("safe-integer"));
}

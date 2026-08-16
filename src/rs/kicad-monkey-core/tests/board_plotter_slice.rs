use kicad_monkey_core::{
    BoardGraphicRecord, BoardGraphicRecordKind, BoardNetClassAssignments, BoardPlotLimits,
    BoardPlotRecord, BoardSegmentRecord, BoardTextVariables, BoardTrackArcRecord,
    BoardViaOperationKind, BoardViaRecord, BoardViaType, BoardZoneRecord, ErrorKind, PlotterFill,
    PlotterOperation, board_plot_document, board_plot_document_with_net_classes,
    board_plot_document_with_sidecars,
};

fn graphic(record: &BoardPlotRecord) -> &BoardGraphicRecord {
    match record {
        BoardPlotRecord::Graphic(record) => record,
        other => panic!("expected graphic record, got {other:?}"),
    }
}

fn segment(record: &BoardPlotRecord) -> &BoardSegmentRecord {
    match record {
        BoardPlotRecord::Segment(record) => record,
        other => panic!("expected segment record, got {other:?}"),
    }
}

fn track_arc(record: &BoardPlotRecord) -> &BoardTrackArcRecord {
    match record {
        BoardPlotRecord::TrackArc(record) => record,
        other => panic!("expected track arc record, got {other:?}"),
    }
}

fn via(record: &BoardPlotRecord) -> &BoardViaRecord {
    match record {
        BoardPlotRecord::Via(record) => record,
        other => panic!("expected via record, got {other:?}"),
    }
}

fn zone(record: &BoardPlotRecord) -> &BoardZoneRecord {
    match record {
        BoardPlotRecord::Zone(record) => record,
        other => panic!("expected zone record, got {other:?}"),
    }
}

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
    let record = graphic(&document.records[0]);
    assert_eq!(record.kind, BoardGraphicRecordKind::GrLine);
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
    let PlotterOperation::ThickSegment(line) = &graphic(&document.records[0]).operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 50_000);

    let zero = r#"(kicad_pcb (gr_line (start 0 0) (end 1 0) (stroke (type solid))))"#;
    let document = board_plot_document(zero, BoardPlotLimits::default()).expect("zero stroke");
    let PlotterOperation::ThickSegment(line) = &graphic(&document.records[0]).operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 0);
}

#[test]
fn legacy_width_applies_only_without_a_stroke_form() {
    let legacy = r#"(kicad_pcb (gr_line (start 0 0) (end 1 0) (width 0.3)))"#;
    let document = board_plot_document(legacy, BoardPlotLimits::default()).expect("legacy width");
    let PlotterOperation::ThickSegment(line) = &graphic(&document.records[0]).operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 300_000);

    // A stroke form wins entirely, even when it omits its own width.
    let shadowed =
        r#"(kicad_pcb (gr_line (start 0 0) (end 1 0) (width 0.3) (stroke (type solid))))"#;
    let document =
        board_plot_document(shadowed, BoardPlotLimits::default()).expect("shadowed legacy width");
    let PlotterOperation::ThickSegment(line) = &graphic(&document.records[0]).operations[0] else {
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
    let record = graphic(&document.records[0]);
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
        assert!(
            !graphic(&document.records[0]).operations.is_empty(),
            "{style}"
        );
        assert!(
            graphic(&document.records[0])
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
    let kinds: Vec<_> = document.records[..6]
        .iter()
        .map(|record| graphic(record).kind)
        .collect();
    assert_eq!(
        kinds,
        [
            BoardGraphicRecordKind::GrLine,
            BoardGraphicRecordKind::GrArc,
            BoardGraphicRecordKind::GrCircle,
            BoardGraphicRecordKind::GrRect,
            BoardGraphicRecordKind::GrPoly,
            BoardGraphicRecordKind::GrCurve,
        ],
        "records group by category with gr_text emitted after the curves"
    );
    assert_eq!(document.records.len(), 7);
    let BoardPlotRecord::Text(text) = &document.records[6] else {
        panic!("expected the gr_text record after the graphic categories");
    };
    assert_eq!(text.text, "skipped");
    for record in &document.records[..6] {
        let record = graphic(record);
        let expected_layer = if record.kind == BoardGraphicRecordKind::GrCurve {
            "F.SilkS"
        } else {
            "Edge.Cuts"
        };
        assert_eq!(record.layer, expected_layer, "{:?}", record.kind);
        assert_eq!(record.uuid, "");
    }

    let PlotterOperation::ArcThreePoint(arc) = &graphic(&document.records[1]).operations[0] else {
        panic!("expected solid arc three-point operation");
    };
    assert_eq!(arc.fill, PlotterFill::NoFill);
    let PlotterOperation::Circle(circle) = &graphic(&document.records[2]).operations[0] else {
        panic!("expected circle operation");
    };
    assert_eq!((circle.cx, circle.cy), (2_000_000, 2_000_000));
    assert_eq!(circle.diameter_nm, 3_000_000);
    assert_eq!(circle.fill, PlotterFill::FilledShape);
    let PlotterOperation::Rect(rect) = &graphic(&document.records[3]).operations[0] else {
        panic!("expected rect operation");
    };
    assert_eq!(rect.fill, PlotterFill::NoFill);
    assert_eq!(rect.corner_radius_nm, 0);
    let PlotterOperation::PlotPoly(poly) = &graphic(&document.records[4]).operations[0] else {
        panic!("expected polygon operation");
    };
    assert_eq!(
        poly.points,
        [[0, 0], [2_000_000, 0], [1_000_000, 1_500_000]]
    );
    assert_eq!(poly.fill, PlotterFill::FilledShape);
    let PlotterOperation::BezierCurve(curve) = &graphic(&document.records[5]).operations[0] else {
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
    let PlotterOperation::Circle(circle) = &graphic(&document.records[0]).operations[0] else {
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
    assert_eq!(
        graphic(&document.records[0]).kind,
        BoardGraphicRecordKind::GrCurve
    );
    assert_eq!(graphic(&document.records[0]).uuid, "short");
    assert!(graphic(&document.records[0]).operations.is_empty());

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

#[test]
fn tracks_and_vias_follow_graphics_in_python_family_order() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (via (at 1 1) (size 0.5) (drill 0.2) (net 0) (uuid "via-first"))
      (arc (start 5 0) (mid 5.5 0.5) (end 6 0) (net 0) (uuid "arc-early"))
      (segment (start 0 0) (end 1 0) (net 0) (uuid "segment-early"))
      (gr_line (start 0 0) (end 1 0) (stroke (width 0.2) (type solid))))"#;
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("family ordering");
    assert!(matches!(document.records[0], BoardPlotRecord::Graphic(_)));
    assert!(matches!(document.records[1], BoardPlotRecord::Segment(_)));
    assert!(matches!(document.records[2], BoardPlotRecord::TrackArc(_)));
    assert!(matches!(document.records[3], BoardPlotRecord::Via(_)));
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies both segment states"
)]
fn segment_records_pin_bare_locked_flags_and_verbatim_widths() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (net 7 "GND")
      (segment (start 1 2) (end 3 4) (width -0.25) (layer "F.Cu") locked (net 7) (uuid "s1"))
      (segment (start 0 0) (end 1 0) (locked yes) (net 0) (uuid "s2")))"#;
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("segment records");

    let locked = segment(&document.records[0]);
    assert!(locked.locked, "bare locked atoms set the Python has_flag");
    assert_eq!(locked.layer, "F.Cu");
    assert_eq!(locked.net_id, Some(7));
    assert_eq!(locked.net_name.as_deref(), Some("GND"));
    let PlotterOperation::ThickSegment(line) = &locked.operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(
        line.width_nm, -250_000,
        "track widths are emitted verbatim, negatives included"
    );
    assert_eq!((line.start_x, line.start_y), (1_000_000, 2_000_000));
    assert_eq!((line.end_x, line.end_y), (3_000_000, 4_000_000));
    assert_eq!(line.layer, None, "track operations are layerless");
    assert!(line.layers.is_empty());

    let bare = segment(&document.records[1]);
    assert!(
        !bare.locked,
        "the (locked yes) child form is not the bare flag Python honors"
    );
    assert_eq!(bare.layer, "", "missing layers default to the empty string");
    assert_eq!(bare.net_id, Some(0));
    assert_eq!(bare.net_name, None, "empty net names are omitted");
    let PlotterOperation::ThickSegment(line) = &bare.operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.width_nm, 0, "missing widths stay zero");
}

#[test]
fn track_arc_records_reverse_endpoints_and_carry_no_lock_state() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (arc (start 5 0) (mid 5.5 0.5) (end 6 0) (width 0.3) (layer "F.Cu") (net 0) (uuid "a1")))"#;
    let document =
        board_plot_document(source, BoardPlotLimits::default()).expect("track arc record");
    let record = track_arc(&document.records[0]);
    assert_eq!(record.uuid, "a1");
    assert_eq!(record.layer, "F.Cu");
    let PlotterOperation::ArcThreePoint(arc) = &record.operations[0] else {
        panic!("expected arc three-point operation");
    };
    assert_eq!(
        (arc.start_x, arc.start_y),
        (6_000_000, 0),
        "the serializer plots from the file end point"
    );
    assert_eq!((arc.mid_x, arc.mid_y), (5_500_000, 500_000));
    assert_eq!(
        (arc.end_x, arc.end_y),
        (5_000_000, 0),
        "back to the file start point"
    );
    assert_eq!(arc.fill, PlotterFill::NoFill);
    assert_eq!(arc.width_nm, 300_000);
    assert_eq!(arc.layer, None, "track operations are layerless");
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies the full via sequence"
)]
fn via_records_emit_aperture_drill_then_exposed_mask_pairs() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (setup (pad_to_mask_clearance 0.051))
      (via (at 7 8) (size 0.5) (drill 0.3) (layers "F.Cu" "B.Cu") (net 0) (uuid "v1")
        (tenting (front no) (back yes))))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("via record");
    let record = via(&document.records[0]);
    assert_eq!(record.via_type, BoardViaType::Through);
    assert_eq!(record.drill, 0.3);
    assert_eq!(record.size, 0.5);
    assert_eq!(record.fabrication.tenting_front, Some(false));
    assert_eq!(record.fabrication.tenting_back, Some(true));
    assert!(record.fabrication.any());

    let kinds: Vec<_> = record
        .operations
        .iter()
        .map(|operation| operation.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            BoardViaOperationKind::Aperture,
            BoardViaOperationKind::Drill,
            BoardViaOperationKind::MaskOpening,
            BoardViaOperationKind::MaskDrill,
        ],
        "back tenting yes suppresses the B.Mask pair"
    );
    assert_eq!(record.operations[0].diameter_nm, 500_000);
    assert_eq!(record.operations[0].layers, ["F.Cu", "B.Cu"]);
    assert_eq!(record.operations[1].diameter_nm, 300_000);
    assert_eq!(record.operations[1].layers, ["F.Cu", "B.Cu"]);
    assert_eq!(
        record.operations[2].diameter_nm, 602_000,
        "mask openings widen by twice the setup pad_to_mask_clearance"
    );
    assert_eq!(record.operations[2].layers, ["F.Mask"]);
    assert_eq!(record.operations[3].diameter_nm, 300_000);
    assert_eq!(record.operations[3].layers, ["F.Mask"]);
    for operation in &record.operations {
        assert_eq!((operation.x, operation.y), (7_000_000, 8_000_000));
    }
}

#[test]
fn via_drills_fall_back_to_half_size_only_when_non_positive() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (via (at 1 1) (size 0.5) (drill -0.3) (layers "F.Cu" "B.Cu") (net 0) (uuid "negative"))
      (via (at 2 2) (size 0.6) (layers "F.Cu" "B.Cu") (net 0) (uuid "missing")))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("via drills");

    let negative = via(&document.records[0]);
    assert_eq!(negative.drill, -0.3, "the raw drill extra is preserved");
    assert_eq!(negative.operations[1].diameter_nm, 250_000);

    let missing = via(&document.records[1]);
    assert_eq!(missing.drill, 0.0);
    assert_eq!(missing.operations[1].diameter_nm, 300_000);
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies every mask gate"
)]
fn via_mask_exposure_requires_untented_sides_reaching_outer_copper() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (via (at 1 1) (size 0.5) (drill 0.2) (layers "*.Cu") (net 0) (uuid "wildcard")
        (tenting (front no) (back no)))
      (via (at 2 2) (size 0.5) (drill 0.2) (layers "In1.Cu" "In2.Cu") (net 0) (uuid "inner")
        (tenting (front no)))
      (via (at 3 3) (size 0.5) (drill 0.2) (layers "F.Cu" "B.Cu") (net 0) (uuid "tented"))
      (via (at 4 4) (size 0.5) (drill 0.2) (net 0) (uuid "unrouted")))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("mask gating");

    let wildcard = via(&document.records[0]);
    assert_eq!(
        wildcard.operations.len(),
        6,
        "*.Cu reaches both outer sides"
    );
    assert_eq!(wildcard.operations[2].layers, ["F.Mask"]);
    assert_eq!(wildcard.operations[4].layers, ["B.Mask"]);

    let inner = via(&document.records[1]);
    assert_eq!(
        inner.operations.len(),
        2,
        "untented sides expose no mask without outer copper"
    );
    assert!(inner.fabrication.any(), "the ipc extras still serialize");

    let tented = via(&document.records[2]);
    assert_eq!(
        tented.operations.len(),
        2,
        "absent tenting never exposes mask openings"
    );
    assert!(!tented.fabrication.any());

    let unrouted = via(&document.records[3]);
    assert!(unrouted.layers.is_empty());
    for operation in &unrouted.operations {
        assert!(
            operation.layers.is_empty(),
            "unrouted vias keep their empty layer list verbatim"
        );
    }
}

#[test]
fn via_types_and_fabrication_mirror_header_flags() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (via micro (at 1 1) (size 0.4) (drill 0.1) (layers "F.Cu" "In1.Cu") (net 0) (uuid "m")
        (covering (front yes)) (plugging (back no)) (capping yes) (filling no))
      (via blind (at 2 2) (size 0.6) (layers "In1.Cu" "B.Cu") (net 0) (uuid "b"))
      (via buried (at 3 3) (size 0.7) (drill 0.35) (layers "In1.Cu" "In2.Cu") (net 0) (uuid "u")))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("via types");

    let micro = via(&document.records[0]);
    assert_eq!(micro.via_type, BoardViaType::Micro);
    assert_eq!(micro.fabrication.covering_front, Some(true));
    assert_eq!(micro.fabrication.plugging_back, Some(false));
    assert_eq!(micro.fabrication.capping, Some(true));
    assert_eq!(micro.fabrication.filling, Some(false));
    assert_eq!(micro.fabrication.tenting_front, None);

    assert_eq!(via(&document.records[1]).via_type, BoardViaType::Blind);
    assert_eq!(via(&document.records[2]).via_type, BoardViaType::Buried);
}

#[test]
fn track_and_via_records_observe_request_budgets() {
    let tracks = r#"(kicad_pcb
      (net 0 "")
      (gr_line (start 0 0) (end 1 0) (stroke (width 0.2) (type solid)))
      (segment (start 0 0) (end 1 0) (net 0)))"#;
    let limited = BoardPlotLimits {
        max_operations: 1,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(tracks, limited)
            .expect_err("segment operations charge the shared budget")
            .kind,
        ErrorKind::ResourceLimit
    );

    let vias = r#"(kicad_pcb
      (net 0 "")
      (via (at 1 1) (size 0.5) (drill 0.2) (net 0)))"#;
    let limited = BoardPlotLimits {
        max_operations: 1,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(vias, limited)
            .expect_err("via operations charge the shared budget")
            .kind,
        ErrorKind::ResourceLimit
    );

    let two_segments = r#"(kicad_pcb
      (net 0 "")
      (segment (start 0 0) (end 1 0) (net 0))
      (segment (start 0 1) (end 1 1) (net 0)))"#;
    let limited = BoardPlotLimits {
        max_graphics: 1,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(two_segments, limited)
            .expect_err("the record budget bounds every promoted family")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies the full zone record"
)]
fn zone_records_serialize_fill_rings_with_parallel_annotations() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (net 1 "GND")
      (zone (net 1) (net_name "GND") (layer "F.Cu") (uuid "z1") (hatch edge 0.5)
        (polygon (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10)))
        (filled_polygon (layer "F.Cu") (pts (xy 0.1 0.1) (xy 9.9 0.1) (xy 9.9 9.9)))
        (filled_polygon (layer "F.Cu") (island) (pts (xy 1 1) (xy 2 1) (xy 2 2))))
      (zone (net 0) (layers "F.Cu" "B.Cu") (uuid "z2") (hatch edge 0.5)
        (filled_polygon (layer "B.Cu") (pts))))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("zone records");
    assert_eq!(document.records.len(), 2);

    let filled = zone(&document.records[0]);
    assert_eq!(filled.uuid, "z1");
    assert_eq!(
        filled.layers,
        ["F.Cu"],
        "singular layer folds into the list"
    );
    assert_eq!(filled.fill_layers, ["F.Cu", "F.Cu"]);
    assert_eq!(filled.fill_island, [false, true]);
    assert_eq!(filled.net_id, Some(1));
    assert_eq!(filled.net_name.as_deref(), Some("GND"));
    assert_eq!(
        filled.operations.len(),
        2,
        "the zone (polygon ...) outline is never serialized"
    );
    for operation in &filled.operations {
        let PlotterOperation::PlotPoly(poly) = operation else {
            panic!("zone fills emit polygon operations only");
        };
        assert_eq!(poly.fill, PlotterFill::FilledShape);
        assert_eq!(poly.width_nm, 0);
        assert_eq!(poly.layer, None, "zone fill operations are layerless");
    }
    let PlotterOperation::PlotPoly(ring) = &filled.operations[0] else {
        panic!("expected polygon operation");
    };
    assert_eq!(
        ring.points,
        [
            [100_000, 100_000],
            [9_900_000, 100_000],
            [9_900_000, 9_900_000]
        ]
    );

    let empty = zone(&document.records[1]);
    assert_eq!(empty.layers, ["F.Cu", "B.Cu"]);
    assert_eq!(empty.net_id, Some(0));
    assert_eq!(empty.net_name, None, "empty net names are omitted");
    assert_eq!(empty.fill_layers, ["B.Cu"]);
    assert_eq!(empty.fill_island, [false]);
    let PlotterOperation::PlotPoly(poly) = &empty.operations[0] else {
        panic!("expected polygon operation");
    };
    assert!(poly.points.is_empty(), "empty pts lists survive verbatim");
}

#[test]
fn keepout_zones_and_empty_net_names_follow_the_python_serializer() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (net 1 "GND")
      (zone (net 0) (layers "*.Cu") (uuid "keepout") (hatch edge 0.5)
        (keepout (tracks not_allowed) (vias not_allowed) (pads allowed)
          (copperpour not_allowed) (footprints allowed))
        (polygon (pts (xy 5 5) (xy 6 5) (xy 6 6))))
      (zone (net 1) (net_name "") (uuid "blank") (hatch edge 0.5)
        (filled_polygon (layer "In1.Cu") (pts))))"#;
    let document = board_plot_document(source, BoardPlotLimits::default()).expect("zone edges");

    let keepout = zone(&document.records[0]);
    assert_eq!(keepout.uuid, "keepout");
    assert!(
        keepout.operations.is_empty(),
        "keepout zones still emit records with zero fill operations"
    );
    assert!(keepout.fill_layers.is_empty());
    assert!(keepout.fill_island.is_empty());

    let blank = zone(&document.records[1]);
    assert_eq!(
        blank.net_name.as_deref(),
        Some("GND"),
        "present-but-empty net_name still resolves from the net table"
    );
    assert!(blank.layers.is_empty(), "missing layers stay empty");
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single parity assertion test intentionally verifies every assignment rule"
)]
fn net_class_extras_follow_normalized_project_assignments() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (net 1 "GND")
      (net 2 "/rail/+3V3")
      (gr_line (start 0 0) (end 1 0) (stroke (width 0.2) (type solid)))
      (segment (start 1 2) (end 3 4) (net 1) (uuid "seg-gnd"))
      (arc (start 5 0) (mid 5.5 0.5) (end 6 0) (net 2) (uuid "arc-rail"))
      (via (at 7 8) (size 0.5) (drill 0.3) (net 1) (uuid "via-gnd"))
      (zone (net 1) (net_name "CUSTOM") (layer "B.Cu") (uuid "zone-custom") (hatch edge 0.5))
      (zone (net 0) (layer "B.Cu") (uuid "zone-anon") (hatch edge 0.5)))"#;
    let net_classes = BoardNetClassAssignments::from_entries([
        ("GND", vec!["Power", "Default"]),
        ("/rail/+3V3", vec!["Rail"]),
        ("", vec!["ShouldNotAppear"]),
        ("CUSTOM", vec!["CustomClass", ""]),
        ("Unused", vec![]),
    ]);
    let document =
        board_plot_document_with_net_classes(source, BoardPlotLimits::default(), &net_classes)
            .expect("net class extras");

    assert!(matches!(document.records[0], BoardPlotRecord::Graphic(_)));
    let seg = segment(&document.records[1]);
    assert_eq!(
        seg.net_classes.net_class.as_deref(),
        Some("Power"),
        "the first class wins the singular extra"
    );
    assert_eq!(seg.net_classes.net_classes, ["Power", "Default"]);
    let arc = track_arc(&document.records[2]);
    assert_eq!(arc.net_classes.net_class.as_deref(), Some("Rail"));
    assert_eq!(arc.net_classes.net_classes, ["Rail"]);
    let gnd_via = via(&document.records[3]);
    assert_eq!(gnd_via.net_classes.net_classes, ["Power", "Default"]);

    let custom = zone(&document.records[4]);
    assert_eq!(
        custom.net_name.as_deref(),
        Some("CUSTOM"),
        "explicit net_name wins over the board table"
    );
    assert_eq!(custom.net_classes.net_class.as_deref(), Some("CustomClass"));
    assert_eq!(
        custom.net_classes.net_classes,
        ["CustomClass"],
        "empty class strings are filtered out"
    );

    let anon = zone(&document.records[5]);
    assert_eq!(
        anon.net_classes.net_class, None,
        "records without truthy net names never gain extras"
    );
    assert!(anon.net_classes.net_classes.is_empty());
}

#[test]
fn net_class_assignments_normalize_like_the_python_project_reader() {
    let net_classes = BoardNetClassAssignments::from_entries([
        ("GND", vec!["First"]),
        ("GND", vec!["Second"]),
        ("Empty", vec!["", ""]),
    ]);
    let extras = net_classes.extras_for(Some("GND"));
    assert_eq!(
        extras.net_class.as_deref(),
        Some("Second"),
        "later duplicate assignments overwrite earlier ones"
    );
    let empty = net_classes.extras_for(Some("Empty"));
    assert_eq!(
        empty.net_class, None,
        "all-empty class lists produce no extras"
    );
    assert!(empty.net_classes.is_empty());
    let unknown = net_classes.extras_for(Some("Missing"));
    assert_eq!(unknown.net_class, None, "lookups are exact-match only");
    assert_eq!(net_classes.extras_for(None).net_class, None);
}

#[test]
fn zone_records_observe_request_budgets() {
    let source = r#"(kicad_pcb
      (net 0 "")
      (zone (net 0) (layer "F.Cu") (uuid "z") (hatch edge 0.5)
        (filled_polygon (layer "F.Cu") (pts (xy 0 0) (xy 1 0) (xy 1 1)))))"#;

    let limited = BoardPlotLimits {
        max_operations: 0,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(source, limited)
            .expect_err("zone rings charge the shared operation budget")
            .kind,
        ErrorKind::ResourceLimit
    );

    let limited = BoardPlotLimits {
        max_points: 2,
        ..BoardPlotLimits::default()
    };
    let error =
        board_plot_document(source, limited).expect_err("zone ring points observe max_points");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("max_points"));

    let two_zones = r#"(kicad_pcb
      (net 0 "")
      (zone (net 0) (layer "F.Cu") (uuid "z1") (hatch edge 0.5))
      (zone (net 0) (layer "F.Cu") (uuid "z2") (hatch edge 0.5)))"#;
    let limited = BoardPlotLimits {
        max_graphics: 1,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(two_zones, limited)
            .expect_err("the record budget bounds zone records")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn board_text_resolves_sidecars_and_board_properties_in_one_linear_pass() {
    let source = r#"(kicad_pcb
      (property "Revision" "board-rev")
      (gr_text "${PROJECT}:${Revision}:${missing}" (at 1 2) (uuid "text")))"#;
    let variables =
        BoardTextVariables::from_entries([("project", "monkey"), ("Revision", "project-rev")]);
    let document = board_plot_document_with_sidecars(
        source,
        BoardPlotLimits::default(),
        &BoardNetClassAssignments::default(),
        &variables,
    )
    .expect("board text sidecars");
    let BoardPlotRecord::Text(record) = &document.records[0] else {
        panic!("expected board text record");
    };
    assert_eq!(record.text, "monkey:board-rev:${missing}");
    assert_eq!(record.operations[0].text, record.text);
}

#[test]
fn board_text_render_cache_points_observe_the_aggregate_limit() {
    let source = r#"(kicad_pcb
      (gr_poly (pts (xy 0 0) (xy 1 0) (xy 1 1))
        (stroke (width 0.1) (type solid)) (fill none))
      (gr_text "cached" (at 0 0)
        (render_cache "cached" 0
          (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))))"#;

    let exact = BoardPlotLimits {
        max_points: 9,
        ..BoardPlotLimits::default()
    };
    board_plot_document(source, exact).expect("the exact aggregate point limit is inclusive");

    let limited = BoardPlotLimits {
        max_points: 8,
        ..BoardPlotLimits::default()
    };
    let error = board_plot_document(source, limited)
        .expect_err("authored cache parsing observes remaining aggregate points");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("max_points"));

    let no_operations = BoardPlotLimits {
        max_operations: 1,
        ..BoardPlotLimits::default()
    };
    assert_eq!(
        board_plot_document(source, no_operations)
            .expect_err("graphic and text operations share one budget")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn board_text_expansion_and_cache_structure_observe_independent_limits() {
    let expanded = r#"(kicad_pcb
      (property "A" "1234567890")
      (gr_text "${A}${A}" (at 0 0)))"#;
    board_plot_document(
        expanded,
        BoardPlotLimits {
            max_text_bytes: 40,
            ..BoardPlotLimits::default()
        },
    )
    .expect("exact resolved text budget is inclusive");
    let error = board_plot_document(
        expanded,
        BoardPlotLimits {
            max_text_bytes: 39,
            ..BoardPlotLimits::default()
        },
    )
    .expect_err("substitution must stop before retaining oversized text");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert!(error.message.contains("max_text_bytes"));

    let cache = r#"(kicad_pcb
      (gr_text "cached" (at 0 0)
        (render_cache "cached" 0 (polygon (pts)))))"#;
    for limits in [
        BoardPlotLimits {
            max_cache_polygons: 0,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_cache_contours: 0,
            ..BoardPlotLimits::default()
        },
        BoardPlotLimits {
            max_parse_nodes: 1,
            ..BoardPlotLimits::default()
        },
    ] {
        assert_eq!(
            board_plot_document(cache, limits)
                .expect_err("independent cache/tree limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let operation_first = BoardPlotLimits {
        max_operations: 0,
        max_cache_polygons: 0,
        ..BoardPlotLimits::default()
    };
    let error = board_plot_document(cache, operation_first)
        .expect_err("operation budget is preflighted before cache construction");
    assert!(error.message.contains("operation"));
}

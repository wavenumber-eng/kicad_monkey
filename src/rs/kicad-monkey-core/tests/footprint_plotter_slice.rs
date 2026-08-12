use kicad_monkey_core::{
    ErrorKind, FootprintPlotLimits, PlotterOperation, footprint_plot_document,
};

const LINE_FOOTPRINT: &str = r#"(footprint "Demo"
  (version 20240108)
  (generator pcbnew)
  (generator_version "8.0")
  locked
  (layer "F.Cu")
  (uuid "abc")
  (descr "Example")
  (tags "demo test")
  (attr smd through_hole)
  (fp_line
    (start 0 0)
    (end 1.5 -2)
    (stroke (width 0.2) (type solid))
    (layer "F.SilkS"))
)"#;

#[test]
fn footprint_plotter_reads_metadata_and_solid_lines_without_a_full_tree() {
    let document = footprint_plot_document(LINE_FOOTPRINT, FootprintPlotLimits::default())
        .expect("plotter document");
    assert_eq!(document.name, "Demo");
    assert_eq!(document.version, 20_240_108);
    assert_eq!(document.generator, "pcbnew");
    assert_eq!(document.generator_version, "8.0");
    assert_eq!(document.layer, "F.Cu");
    assert_eq!(document.uuid, "abc");
    assert_eq!(document.descr, "Example");
    assert_eq!(document.tags, "demo test");
    assert_eq!(document.attr, ["smd", "through_hole"]);
    assert!(document.locked);
    assert!(!document.placed);
    assert_eq!(document.operations.len(), 1);
    let PlotterOperation::ThickSegment(line) = &document.operations[0] else {
        panic!("expected thick segment");
    };
    assert_eq!(line.start_x, 0);
    assert_eq!(line.start_y, 0);
    assert_eq!(line.end_x, 1_500_000);
    assert_eq!(line.end_y, -2_000_000);
    assert_eq!(line.width_nm, 200_000);
    assert_eq!(line.layer.as_deref(), Some("F.SilkS"));
}

#[test]
fn footprint_plotter_defaults_match_python_and_operation_limits_fail_closed() {
    let source = "(footprint \"Empty\")";
    let document =
        footprint_plot_document(source, FootprintPlotLimits::default()).expect("default document");
    assert_eq!(document.version, 20_260_206);
    assert_eq!(document.generator, "pcbnew");
    assert_eq!(document.generator_version, "10.0");
    assert_eq!(document.layer, "F.Cu");
    assert!(document.operations.is_empty());

    let limited = FootprintPlotLimits {
        max_operations: 0,
        ..FootprintPlotLimits::default()
    };
    assert_eq!(
        footprint_plot_document(LINE_FOOTPRINT, limited)
            .expect_err("operation limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn dashed_lines_expand_to_bounded_thick_segments() {
    let source = LINE_FOOTPRINT.replace("(type solid)", "(type dash)");
    let document = footprint_plot_document(&source, FootprintPlotLimits::default())
        .expect("dashed line decomposition");
    assert_eq!(document.operations.len(), 1);
    let PlotterOperation::ThickSegment(segment) = &document.operations[0] else {
        panic!("dash decomposition emits thick segments");
    };
    assert_eq!((segment.end_x, segment.end_y), (1_320_000, -1_760_000));

    for style in ["dot", "dash_dot", "dash_dot_dot"] {
        let patterned = LINE_FOOTPRINT.replace("(type solid)", &format!("(type {style})"));
        let operations = footprint_plot_document(&patterned, FootprintPlotLimits::default())
            .expect("supported patterned stroke")
            .operations;
        assert!(!operations.is_empty(), "{style}");
        assert!(
            operations
                .iter()
                .all(|operation| matches!(operation, PlotterOperation::ThickSegment(_))),
            "{style} decomposes to thick segments"
        );
    }

    let limited = FootprintPlotLimits {
        max_operations: 0,
        ..FootprintPlotLimits::default()
    };
    assert_eq!(
        footprint_plot_document(&source, limited)
            .expect_err("decomposition observes operation limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let unsupported = LINE_FOOTPRINT.replace("(type solid)", "(type custom)");
    let error = footprint_plot_document(&unsupported, FootprintPlotLimits::default())
        .expect_err("unknown stroke style must not become solid");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(error.message.contains("Unsupported footprint stroke type"));
}

#[test]
fn promoted_graphics_follow_python_grouping_and_bound_emitted_operations() {
    let source = r#"(footprint "Graphics"
      (fp_circle (center 2 2) (end 2 3.5) (stroke (width 0.12)) (fill yes))
      (fp_line (start 0 0) (end 2 0) (stroke (width 0.1) (type solid)))
      (fp_rect (start -2 -1) (end -1 1) (stroke (width 0.05)) (fill no))
      (fp_arc (start 1 0) (mid 0 1) (end -1 0) (stroke (width 0.15)))
      (fp_poly (pts (xy 0 0) (xy 2 0) (xy 1 1.5)) (stroke (width 0.1)) (fill solid))
      (fp_line (start 0 1) (end 3 1) (stroke (width 0.1) (type dash)))
      (fp_arc (start 1 0) (mid 0 1) (end -1 0) (stroke (width 0.2) (type dot))))"#;
    let document =
        footprint_plot_document(source, FootprintPlotLimits::default()).expect("promoted graphics");
    assert_eq!(document.operations.len(), 11);
    assert!(matches!(
        document.operations[0],
        PlotterOperation::ThickSegment(_)
    ));
    assert!(matches!(
        document.operations[3],
        PlotterOperation::ArcThreePoint(_)
    ));
    assert!(matches!(
        document.operations[8],
        PlotterOperation::Circle(_)
    ));
    assert!(matches!(document.operations[9], PlotterOperation::Rect(_)));
    let PlotterOperation::PlotPoly(poly) = &document.operations[10] else {
        panic!("expected canonical polygon group last");
    };
    assert_eq!(
        poly.points,
        [[0, 0], [2_000_000, 0], [1_000_000, 1_500_000]]
    );

    let limited = FootprintPlotLimits {
        max_operations: 10,
        ..FootprintPlotLimits::default()
    };
    assert_eq!(
        footprint_plot_document(source, limited)
            .expect_err("decomposed output observes operation limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn standard_pad_flashes_and_drills_share_the_plotter_operation_vocabulary() {
    let source = r#"(footprint "Pads"
      (solder_mask_margin 0.05)
      (pad "1" smd roundrect (at 1 2 45) (size 2 1)
        (layers "F.Cu" "F.Mask") (roundrect_rratio 0.2))
      (pad "2" thru_hole circle (at 5 5) (size 1.8 1.8)
        (drill 0.8) (layers "*.Cu" "*.Mask")))"#;
    let document = footprint_plot_document(source, FootprintPlotLimits::default())
        .expect("pad plotter operations");
    assert_eq!(document.operations.len(), 3);
    let PlotterOperation::FlashPadRoundRect(roundrect) = &document.operations[0] else {
        panic!("expected roundrect flash");
    };
    assert_eq!(roundrect.corner_radius_nm, 200_000);
    assert_eq!(roundrect.mask_margin_nm, 50_000);
    let PlotterOperation::FlashPadCircle(circle) = &document.operations[1] else {
        panic!("expected through-hole copper flash");
    };
    assert_eq!(circle.layers, ["*.Cu", "*.Mask"]);
    let PlotterOperation::Circle(drill) = &document.operations[2] else {
        panic!("expected shared circle drill operation");
    };
    assert_eq!(drill.role.as_deref(), Some("pad_drill"));
    assert_eq!(drill.layer, None);

    let limited = FootprintPlotLimits {
        max_operations: 2,
        ..FootprintPlotLimits::default()
    };
    assert_eq!(
        footprint_plot_document(source, limited)
            .expect_err("pad flashes and drills share the emitted-operation limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn duplicate_metadata_keeps_the_first_value_like_the_python_model() {
    let duplicates = (0..24)
        .map(|index| format!("(generator value-{index})"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("(footprint \"Demo\" {duplicates} (layer \"F.Cu\") (layer \"B.Cu\"))");
    let limits = FootprintPlotLimits {
        max_metadata_forms: 32,
        max_operations: 0,
        ..FootprintPlotLimits::default()
    };
    let document =
        footprint_plot_document(&source, limits).expect("duplicate metadata remains deterministic");
    assert_eq!(document.generator, "value-0");
    assert_eq!(document.layer, "F.Cu");

    let metadata_limited = FootprintPlotLimits {
        max_metadata_forms: 8,
        ..limits
    };
    let error = footprint_plot_document(&source, metadata_limited)
        .expect_err("metadata has an independent limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn plotter_outputs_stay_inside_the_javascript_safe_integer_range() {
    const SAFE_MAX: i64 = 9_007_199_254_740_991;
    const SAFE_MIN: i64 = -SAFE_MAX;

    for version in [SAFE_MIN, SAFE_MAX] {
        let source = format!("(footprint \"Demo\" (version {version}))");
        let document = footprint_plot_document(&source, FootprintPlotLimits::default())
            .expect("safe boundary version");
        assert_eq!(document.version, version);
    }

    for version in [SAFE_MIN - 1, SAFE_MAX + 1] {
        let source = format!("(footprint \"Demo\" (version {version}))");
        let error = footprint_plot_document(&source, FootprintPlotLimits::default())
            .expect_err("unsafe version");
        assert_eq!(error.kind, ErrorKind::UnexpectedToken);
        assert!(error.message.contains("safe-integer"));
    }

    let inside = r#"(footprint "Demo"
      (fp_line (start 9007199254.74099 0) (end 0 0)
        (stroke (width 0.1) (type solid))))"#;
    let document = footprint_plot_document(inside, FootprintPlotLimits::default())
        .expect("largest representable near-boundary coordinate");
    let PlotterOperation::ThickSegment(line) = &document.operations[0] else {
        panic!("expected thick segment");
    };
    assert!(line.start_x <= SAFE_MAX);

    let outside = r#"(footprint "Demo"
      (fp_line (start 9007199255 0) (end 0 0)
        (stroke (width 0.1) (type solid))))"#;
    let error = footprint_plot_document(outside, FootprintPlotLimits::default())
        .expect_err("unsafe coordinate");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(error.message.contains("safe-integer"));
}

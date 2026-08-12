use kicad_monkey_core::{ErrorKind, FootprintPlotLimits, footprint_plot_document};

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
    let line = &document.operations[0];
    assert_eq!(line.start_x, 0);
    assert_eq!(line.start_y, 0);
    assert_eq!(line.end_x, 1_500_000);
    assert_eq!(line.end_y, -2_000_000);
    assert_eq!(line.width_nm, 200_000);
    assert_eq!(line.layer, "F.SilkS");
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
fn unsupported_dashed_lines_are_explicit() {
    let source = LINE_FOOTPRINT.replace("(type solid)", "(type dash)");
    let error = footprint_plot_document(&source, FootprintPlotLimits::default())
        .expect_err("dash is not yet promoted");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(error.message.contains("only solid"));
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
    assert!(document.operations[0].start_x <= SAFE_MAX);

    let outside = r#"(footprint "Demo"
      (fp_line (start 9007199255 0) (end 0 0)
        (stroke (width 0.1) (type solid))))"#;
    let error = footprint_plot_document(outside, FootprintPlotLimits::default())
        .expect_err("unsafe coordinate");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(error.message.contains("safe-integer"));
}

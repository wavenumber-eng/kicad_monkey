use kicad_monkey_core::{ErrorKind, PcbLimits, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (footprint "Demo:Manufacturing"
    (layer "F.Cu")
    (pad "1" thru_hole circle (at 1 2) (size 2 2) (layers "*.Cu" "*.Mask")
      (teardrops
        (best_length_ratio 0.5) (max_length 1)
        (best_width_ratio 1) (max_width 2)
        (curved_edges no)filter_ratio 0.9)
        (enabled yes) (allow_two_segments yes) (prefer_zone_connections no)
      )
      (backdrill (size 0.5) (layers "F.Cu" "In1.Cu"))
      (tertiary_drill (size 0.4) (layers "B.Cu" "In2.Cu"))
      (front_post_machining counterbore (size 1.1) (depth 0.2) (angle 90))
      (back_post_machining countersink (size 1.2) (depth 0.25) (angle 75))
      (zone_layer_connections "In1.Cu" "In2.Cu")
      (uuid pad-id)))
  (via (at 4 5) (size 1) (drill 0.4) (layers "F.Cu" "B.Cu")
    (backdrill (size 0.6) (layers "F.Cu" "In2.Cu"))
    (tertiary_drill (size 0.45) (layers "B.Cu" "In3.Cu"))
    (front_post_machining counterbore (size 1.3) (depth 0.3) (angle 80))
    (back_post_machining countersink (size 1.4) (depth 0.35) (angle 70))
    (zone_layer_connections "In3.Cu" "In4.Cu")
    (uuid via-id)))"#;

#[test]
fn pad_manufacturing_records_match_python_including_legacy_teardrops() {
    let pad = PcbView::parse(SOURCE, PcbLimits::default())
        .expect("board")
        .pads()
        .next()
        .expect("pad")
        .expect("typed pad");
    let teardrops = pad.teardrops.expect("teardrops");
    assert_eq!(teardrops.best_length_ratio, Some(0.5));
    assert_eq!(teardrops.max_length, Some(1.0));
    assert_eq!(teardrops.best_width_ratio, Some(1.0));
    assert_eq!(teardrops.max_width, Some(2.0));
    assert_eq!(teardrops.curved_edges, Some(false));
    assert_eq!(teardrops.filter_ratio, Some(0.9));
    assert_eq!(teardrops.enabled, Some(true));
    assert_eq!(teardrops.allow_two_segments, Some(true));
    assert_eq!(teardrops.prefer_zone_connections, Some(false));
    assert!(SOURCE[teardrops.source_range].starts_with("(teardrops"));
    assert_manufacturing(
        pad.backdrill.as_ref().expect("backdrill"),
        pad.tertiary_drill.as_ref().expect("tertiary drill"),
        pad.front_post_machining.as_ref().expect("front machining"),
        pad.back_post_machining.as_ref().expect("back machining"),
        &pad.zone_layer_connections
            .as_ref()
            .expect("zone connections")
            .forced_layers,
        (0.5, 0.4, 1.1, 1.2),
        &["In1.Cu", "In2.Cu"],
    );
}

#[test]
fn via_reuses_the_same_manufacturing_semantics() {
    let via = PcbView::parse(SOURCE, PcbLimits::default())
        .expect("board")
        .vias()
        .next()
        .expect("via")
        .expect("typed via");
    assert_manufacturing(
        via.backdrill.as_ref().expect("backdrill"),
        via.tertiary_drill.as_ref().expect("tertiary drill"),
        via.front_post_machining.as_ref().expect("front machining"),
        via.back_post_machining.as_ref().expect("back machining"),
        &via.zone_layer_connections
            .as_ref()
            .expect("zone connections")
            .forced_layers,
        (0.6, 0.45, 1.3, 1.4),
        &["In3.Cu", "In4.Cu"],
    );
}

fn assert_manufacturing(
    backdrill: &kicad_monkey_core::PcbDrillProperties,
    tertiary: &kicad_monkey_core::PcbDrillProperties,
    front: &kicad_monkey_core::PcbPostMachiningProperties,
    back: &kicad_monkey_core::PcbPostMachiningProperties,
    zone_layers: &[String],
    sizes: (f64, f64, f64, f64),
    expected_zone_layers: &[&str],
) {
    assert_eq!(backdrill.size, Some(sizes.0));
    assert_eq!(backdrill.layers.start, "F.Cu");
    assert_eq!(tertiary.size, Some(sizes.1));
    assert!(tertiary.layers.end.starts_with("In"));
    assert_eq!(front.mode, "counterbore");
    assert_eq!(front.size, Some(sizes.2));
    assert!(front.depth.is_some());
    assert!(front.angle.is_some());
    assert_eq!(back.mode, "countersink");
    assert_eq!(back.size, Some(sizes.3));
    assert_eq!(zone_layers, expected_zone_layers);
}

#[test]
fn sparse_manufacturing_records_follow_python_truthiness() {
    let source = r#"(kicad_pcb
      (footprint "Sparse" (layer "F.Cu")
        (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu")
          (teardrops) (backdrill (layers "F.Cu"))
          (front_post_machining (size 1)) (zone_layer_connections))))"#;
    let pad = PcbView::parse(source, PcbLimits::default())
        .expect("board")
        .pads()
        .next()
        .expect("pad")
        .expect("typed pad");
    assert!(
        pad.teardrops.is_some(),
        "present empty block remains present"
    );
    assert!(pad.backdrill.is_none(), "incomplete layer span is falsey");
    assert!(pad.front_post_machining.is_none(), "missing mode is falsey");
    assert_eq!(
        pad.zone_layer_connections
            .expect("present empty zone override")
            .forced_layers,
        Vec::<String>::new()
    );
}

#[test]
fn manufacturing_limits_accept_exact_boundaries_and_fail_closed_above_them() {
    let exact = PcbLimits {
        max_manufacturing_children: 8,
        max_teardrop_scalars: 2,
        max_zone_layer_connections: 2,
        ..PcbLimits::default()
    };
    PcbView::parse(SOURCE, exact)
        .expect("board")
        .pads()
        .collect::<Result<Vec<_>, _>>()
        .expect("exact pad limits");
    PcbView::parse(SOURCE, exact)
        .expect("board")
        .vias()
        .collect::<Result<Vec<_>, _>>()
        .expect("exact via limits");

    for limits in [
        PcbLimits {
            max_manufacturing_children: 7,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_teardrop_scalars: 1,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_zone_layer_connections: 1,
            ..PcbLimits::default()
        },
    ] {
        let view = PcbView::parse(SOURCE, limits).expect("lazy manufacturing limit");
        let error = view
            .pads()
            .collect::<Result<Vec<_>, _>>()
            .expect_err("resource limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }
}

#[test]
fn malformed_bare_teardrop_value_reports_an_absolute_source_position() {
    let source = r#"(kicad_pcb
      (footprint "Bad" (layer "F.Cu")
        (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu")
          (teardrops (curved_edges no)filter_ratio bad)
            (enabled yes))
          (uuid pad-id))))"#;
    let error = PcbView::parse(source, PcbLimits::default())
        .expect("lazy board")
        .pads()
        .next()
        .expect("pad")
        .expect_err("invalid filter ratio");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert_eq!(
        error.position.expect("source position").offset,
        source.find("bad").expect("bad token")
    );
}

#[test]
fn teardrop_booleans_accept_only_the_parser_dialect_and_report_absolute_errors() {
    let valid = board_with_teardrops(
        "(curved_edges yes) (enabled true) (allow_two_segments no) (prefer_zone_connections false)",
    );
    let parameters = PcbView::parse(&valid, PcbLimits::default())
        .expect("valid board")
        .pads()
        .next()
        .expect("pad")
        .expect("typed pad")
        .teardrops
        .expect("teardrops");
    assert_eq!(parameters.curved_edges, Some(true));
    assert_eq!(parameters.enabled, Some(true));
    assert_eq!(parameters.allow_two_segments, Some(false));
    assert_eq!(parameters.prefer_zone_connections, Some(false));

    let empty = board_with_teardrops("(enabled)");
    assert_eq!(
        PcbView::parse(&empty, PcbLimits::default())
            .expect("empty boolean board")
            .pads()
            .next()
            .expect("pad")
            .expect("typed pad")
            .teardrops
            .expect("teardrops")
            .enabled,
        None
    );

    for invalid in ["maybe", "1"] {
        let source = board_with_teardrops(&format!("(enabled {invalid})"));
        let error = PcbView::parse(&source, PcbLimits::default())
            .expect("lazy invalid board")
            .pads()
            .next()
            .expect("pad")
            .expect_err("invalid boolean");
        assert_eq!(error.kind, ErrorKind::UnexpectedToken);
        let field = format!("(enabled {invalid}");
        assert_eq!(
            error.position.expect("absolute position").offset,
            source.find(&field).expect("field") + "(enabled ".len()
        );
    }
}

fn board_with_teardrops(body: &str) -> String {
    format!(
        "(kicad_pcb (footprint \"Bool\" (layer \"F.Cu\") \
         (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") \
         (teardrops {body}))))"
    )
}

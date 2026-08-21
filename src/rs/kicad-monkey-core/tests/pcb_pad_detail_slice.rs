use kicad_monkey_core::{ErrorKind, PcbLimits, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (net 7 "POWER")
  (footprint "Demo:Pads"
    (layer "F.Cu")
    (pad "1" smd roundrect (at 1 2 30) (size 3 4) (layers "F.Cu" "F.Mask")
      (net 7 "POWER") (uuid round-id) (pinfunction "VCC") (pintype "power_in")
      (die_length 0.8) (rect_delta 0.2 0.4 ignored) (roundrect_rratio 0.25)
      (chamfer_ratio 0.1) (chamfer top_left bottom_right)
      (solder_mask_margin 0.05) (solder_paste_margin -0.01)
      (solder_paste_margin_ratio -0.1) (clearance invalid)
      (thermal_bridge_width 0.3) (thermal_bridge_angle 45) (thermal_gap 0.2)
      (zone_connect 2) (remove_unused_layers) (keep_end_layers no))
    (pad "2" smd custom (at 5 6) (size 2 2) (layers "B.Cu")
      (options (clearance outline) (anchor rect))
      (primitives
        (gr_poly (pts (xy 0 0) (xy 1 0) (xy 1 1) (xy 999))
          (width 0.1) (fill solid))
        (gr_line (start 0 0) (end 1 1))))))"#;

#[test]
fn pad_shape_fabrication_and_connection_fields_match_python_semantics() {
    let pad = PcbView::parse(SOURCE, PcbLimits::default())
        .expect("board")
        .pads()
        .next()
        .expect("pad")
        .expect("typed pad");
    assert_pad_shape_fields(&pad);
    assert_pad_fabrication_fields(&pad);
    assert_pad_connection_fields(&pad);
}

fn assert_pad_shape_fields(pad: &kicad_monkey_core::PcbPad) {
    assert_eq!(
        (pad.number.as_str(), pad.kind.as_str(), pad.shape.as_str()),
        ("1", "smd", "roundrect")
    );
    assert_eq!(pad.pin_function.as_deref(), Some("VCC"));
    assert_eq!(pad.pin_type.as_deref(), Some("power_in"));
    assert_eq!(pad.die_length, Some(0.8));
    assert_eq!((pad.rect_delta_x, pad.rect_delta_y), (Some(0.2), Some(0.4)));
    assert_eq!(pad.roundrect_rratio, Some(0.25));
    assert_eq!(pad.chamfer_ratio, Some(0.1));
    assert_eq!(pad.chamfer_corners, ["top_left", "bottom_right"]);
}

fn assert_pad_fabrication_fields(pad: &kicad_monkey_core::PcbPad) {
    assert_eq!(pad.solder_mask_margin, Some(0.05));
    assert_eq!(pad.solder_paste_margin, Some(-0.01));
    assert_eq!(pad.solder_paste_margin_ratio, Some(-0.1));
    assert_eq!(pad.clearance, None, "Python tolerates malformed clearance");
    assert_eq!(pad.thermal_bridge_width, Some(0.3));
    assert_eq!(pad.thermal_bridge_angle, Some(45.0));
    assert_eq!(pad.thermal_gap, Some(0.2));
}

fn assert_pad_connection_fields(pad: &kicad_monkey_core::PcbPad) {
    assert_eq!(pad.zone_connect, Some(2));
    assert_eq!(pad.remove_unused_layers, Some(true));
    assert_eq!(pad.keep_end_layers, Some(false));
}

#[test]
fn custom_pad_options_and_primitives_are_typed_and_source_evidenced() {
    let pad = PcbView::parse(SOURCE, PcbLimits::default())
        .expect("board")
        .pads()
        .nth(1)
        .expect("custom pad")
        .expect("typed custom pad");
    let options = pad.custom_options.expect("custom options");
    assert_eq!(options.clearance.as_deref(), Some("outline"));
    assert_eq!(options.anchor.as_deref(), Some("rect"));
    assert!(SOURCE[options.source_range].starts_with("(options"));
    assert_eq!(pad.custom_primitives.len(), 2);
    let polygon = &pad.custom_primitives[0];
    assert_eq!(polygon.kind, "gr_poly");
    assert_eq!(
        polygon
            .points
            .iter()
            .map(|point| (point.x, point.y))
            .collect::<Vec<_>>(),
        [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]
    );
    assert_eq!(polygon.width, Some(0.1));
    assert_eq!(polygon.fill.as_deref(), Some("solid"));
    assert!(SOURCE[polygon.source_range.clone()].starts_with("(gr_poly"));
    assert_eq!(pad.custom_primitives[1].kind, "gr_line");
    assert!(pad.custom_primitives[1].points.is_empty());
}

#[test]
fn pad_detail_limits_accept_exact_boundaries_and_fail_closed_above_them() {
    let exact = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_pad_chamfer_corners: 2,
            max_pad_header_scalars: 3,
            max_pad_custom_primitives: 2,
            max_pad_custom_point_forms: 4,
            max_pad_custom_points: 3,
            ..PcbLimits::default()
        },
    )
    .expect("board")
    .pads()
    .collect::<Result<Vec<_>, _>>()
    .expect("exact limits");
    assert_eq!(exact.len(), 2);

    for limits in [
        PcbLimits {
            max_pad_header_scalars: 2,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_pad_chamfer_corners: 1,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_pad_custom_primitives: 1,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_pad_custom_point_forms: 3,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_pad_custom_points: 2,
            ..PcbLimits::default()
        },
    ] {
        let view = PcbView::parse(SOURCE, limits).expect("lazy pad detail limit");
        let error = view
            .pads()
            .collect::<Result<Vec<_>, _>>()
            .expect_err("resource limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }
}

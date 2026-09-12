use kicad_monkey_core::{
    ErrorKind, FootprintLimits, FootprintView, PcbLimits, PcbPadPolygonPoint,
    PcbPadPrimitiveGeometry, PcbView,
};

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
    assert!(pad.has_layers);
    assert_eq!(pad.layers, ["F.Cu", "F.Mask"]);

    // Speedy's explicit empty-layer NPTH must remain distinguishable from an
    // omitted declaration in both document scopes. Neither synthesizes copper.
    for (declaration, present, expected_layers) in [
        ("", false, vec![]),
        ("(layers)", true, vec![]),
        ("(layers \"*.Cu\" \"*.Mask\")", true, vec!["*.Cu", "*.Mask"]),
    ] {
        let footprint = format!(
            "(footprint \"Presence\" (pad \"\" np_thru_hole roundrect (at 0 0) \
             (size 1.75 1.75) (drill 1.77) (roundrect_rratio 0.25) {declaration} \
             (uuid dff75feb-201e-433a-aab4-4d5d154d4948)))"
        );
        let board = format!("(kicad_pcb {footprint})");
        let embedded = PcbView::parse(&board, PcbLimits::default())
            .expect("board")
            .pads()
            .next()
            .expect("pad")
            .expect("embedded pad");
        let standalone = FootprintView::parse(&footprint, FootprintLimits::default())
            .expect("footprint")
            .pads()
            .next()
            .expect("pad")
            .expect("standalone pad");
        for pad in [embedded, standalone] {
            assert_eq!(pad.has_layers, present);
            assert_eq!(pad.layers, expected_layers);
            assert_eq!(pad.kind, "np_thru_hole");
            assert_eq!(pad.plated, Some(false));
            assert_eq!(
                pad.uuid.as_deref(),
                Some("dff75feb-201e-433a-aab4-4d5d154d4948")
            );
        }
    }
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
    assert!(matches!(
        pad.custom_primitives[1].geometry,
        Some(PcbPadPrimitiveGeometry::Line { .. })
    ));
    // The deliberately incomplete legacy polygon retains its XY projection,
    // but cannot masquerade as complete typed geometry.
    assert!(polygon.geometry.is_none());
    assert_custom_style_precedence();
    assert_all_custom_geometry();
    assert_padstack_source_records();
}

fn assert_custom_style_precedence() {
    for (style, width, fill) in [
        ("", None, None),
        ("(width 0) (fill none)", Some(0.0), Some("none")),
        (
            "(stroke (width 0.2) (type solid)) (fill no)",
            Some(0.2),
            Some("no"),
        ),
        (
            "(width 0.1) (stroke (width 0.2)) (fill yes)",
            Some(0.2),
            Some("yes"),
        ),
        ("(stroke (width 0.2)) (width 0.1)", Some(0.1), None),
        ("(width 0.1) (stroke (type solid))", Some(0.1), None),
        ("(stroke (width 0.1) (width 0.2))", Some(0.2), None),
    ] {
        let source = SOURCE.replace("(width 0.1) (fill solid)", style);
        let view = PcbView::parse(&source, PcbLimits::default()).unwrap();
        let pad = view.pads().nth(1).unwrap().unwrap();
        assert_eq!(pad.custom_primitives[0].width, width);
        assert_eq!(pad.custom_primitives[0].fill.as_deref(), fill);
    }
}

fn assert_padstack_source_records() {
    // The source-bearing subset of all seven Speedy front/inner/back pads.
    let footprint = r#"(footprint "Stack" (pad "1" thru_hole circle
      (size 0.635 0.635) (drill 0.3) (layers "*.Cu" "*.Mask")
      (padstack (mode front_inner_back)
        (layer "Inner" (shape circle) (size 0.635 0.635) (zone_connect -1))
        (layer "B.Cu" (shape circle) (size 0.635 0.635)))))"#;
    let board = format!("(kicad_pcb {footprint})");
    let embedded = PcbView::parse(&board, Default::default())
        .unwrap()
        .pads()
        .next()
        .unwrap()
        .unwrap();
    let local = FootprintView::parse(footprint, Default::default())
        .unwrap()
        .pads()
        .next()
        .unwrap()
        .unwrap();
    for pad in [embedded, local] {
        let stack = pad.padstack.unwrap();
        assert_eq!(stack.mode.as_deref(), Some("front_inner_back"));
        assert_eq!(
            stack
                .layers
                .iter()
                .map(|row| row.layer.as_str())
                .collect::<Vec<_>>(),
            ["Inner", "B.Cu"]
        );
        assert_eq!(stack.layers[0].zone_connect, Some(-1));
        assert_eq!(stack.layers[1].zone_connect, None);
        assert_eq!(stack.layers[0].size.unwrap().x, 0.635);
        assert!(stack.layers[0].offset.is_none());
    }
    let source = r#"(kicad_pcb (footprint "Budgets" (pad "1" smd custom (size 1 1) (layers "F.Cu")
      (primitives (gr_line (start 0 0) (end 1 0) (width 0.1)))
      (padstack (mode custom) (layer "B.Cu" (shape custom) (size 1 1)
        (thermal_bridge_angle 23) (options (anchor circle) (clearance convexhull))
        (primitives (gr_circle (center 0 0) (end 1 0) (width 0) (fill yes))))))))"#;
    let exact = PcbLimits {
        max_pad_custom_points: 4,
        max_pad_custom_primitives: 2,
        ..Default::default()
    };
    let pad = PcbView::parse(source, exact)
        .unwrap()
        .pads()
        .next()
        .unwrap()
        .unwrap();
    let row = &pad.padstack.as_ref().unwrap().layers[0];
    assert_eq!(row.thermal_bridge_angle, Some(23.0));
    assert_eq!(
        row.custom_options.as_ref().unwrap().clearance.as_deref(),
        Some("convexhull")
    );
    assert!(matches!(
        row.custom_primitives[0].geometry,
        Some(PcbPadPrimitiveGeometry::Circle { .. })
    ));
    for limits in [
        PcbLimits {
            max_pad_custom_points: 3,
            ..exact
        },
        PcbLimits {
            max_pad_custom_primitives: 1,
            ..exact
        },
    ] {
        assert_eq!(
            PcbView::parse(source, limits)
                .unwrap()
                .pads()
                .next()
                .unwrap()
                .unwrap_err()
                .kind,
            ErrorKind::ResourceLimit
        );
    }
}

fn assert_all_custom_geometry() {
    let footprint = r#"(footprint "Custom"
      (pad "1" smd custom (size 1 1) (layers "F.Cu")
        (primitives
          (gr_arc (start 1 0) (mid 0 1) (end -1 0) (width 0.1))
          (gr_circle (center 0 0) (end 2 0) (width 0) (fill solid))
          (gr_rect (start -1 -2) (end 1 2) (radius 0.2) (width 0.1) (fill no))
          (gr_curve (pts (xy 0 0) (xy 1 2) (xy 2 1) (xy 3 0)) (width 0.2))
          (gr_poly (pts (xy 1 0) (arc (start 1 0) (mid 0 1) (end -1 0)) (xy 0 -1)) (width 0) (fill yes))
          (gr_bbox (start -1 -1) (end 1 1))
          (gr_vector (start 0 0) (end 1 0)))))"#;
    let board = format!("(kicad_pcb {footprint})");
    let board_pad = PcbView::parse(&board, PcbLimits::default())
        .unwrap()
        .pads()
        .next()
        .unwrap()
        .unwrap();
    let local_pad = FootprintView::parse(footprint, FootprintLimits::default())
        .unwrap()
        .pads()
        .next()
        .unwrap()
        .unwrap();
    let expected = &board_pad.custom_primitives;
    for (a, b) in expected.iter().zip(&local_pad.custom_primitives) {
        assert_eq!(a.geometry, b.geometry);
        assert_eq!((a.width, &a.fill), (b.width, &b.fill));
    }
    assert_custom_arc_circle_rect(expected);
    assert_custom_curve_polygon_proxies(expected);
}

fn assert_custom_arc_circle_rect(expected: &[kicad_monkey_core::PcbPadCustomPrimitive]) {
    assert!(
        matches!(expected[0].geometry, Some(PcbPadPrimitiveGeometry::Arc { start, mid, end }) if start.x == 1.0 && mid.y == 1.0 && end.x == -1.0)
    );
    assert!(
        matches!(expected[1].geometry, Some(PcbPadPrimitiveGeometry::Circle { center, end }) if center.x == 0.0 && end.x == 2.0)
    );
    assert!(matches!(
        expected[2].geometry,
        Some(PcbPadPrimitiveGeometry::Rect {
            radius: Some(0.2),
            ..
        })
    ));
}

fn assert_custom_curve_polygon_proxies(expected: &[kicad_monkey_core::PcbPadCustomPrimitive]) {
    assert!(
        matches!(expected[3].geometry, Some(PcbPadPrimitiveGeometry::Curve { points }) if points[1].y == 2.0 && points[2].x == 2.0)
    );
    assert!(
        matches!(&expected[4].geometry, Some(PcbPadPrimitiveGeometry::Polygon { points }) if matches!(points[1], PcbPadPolygonPoint::Arc { mid, .. } if mid.y == 1.0))
    );
    assert!(matches!(
        expected[5].geometry,
        Some(PcbPadPrimitiveGeometry::BoundingBoxProxy { .. })
    ));
    assert!(matches!(
        expected[6].geometry,
        Some(PcbPadPrimitiveGeometry::VectorProxy { .. })
    ));
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
            max_pad_custom_points: 5,
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
            max_pad_custom_points: 4,
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

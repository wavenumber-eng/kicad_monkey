use kicad_monkey_core::{
    AuthoredGraphic, AuthoredGraphicGeometry, AuthoredLayer, AuthoredNet, AuthoredNetRef,
    AuthoredPcb, AuthoredPoint, AuthoredRoutingArc, AuthoredSegment, AuthoredZone,
    AuthoredZoneFill, AuthoredZoneFilledPolygon, AuthoredZoneHatch, AuthoredZonePadConnection,
    AuthoredZonePolygon, ErrorKind, PcbAuthoringLimits,
};

fn point(x_mm: f64, y_mm: f64) -> AuthoredPoint {
    AuthoredPoint { x_mm, y_mm }
}

fn uuid(value: u64) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

fn polygon(points: &[(f64, f64)]) -> AuthoredZonePolygon {
    AuthoredZonePolygon {
        points: points.iter().map(|&(x, y)| point(x, y)).collect(),
    }
}

fn filled(layer: &str, island: bool, points: &[(f64, f64)]) -> AuthoredZoneFilledPolygon {
    AuthoredZoneFilledPolygon {
        layer: layer.to_owned(),
        island,
        points: points.iter().map(|&(x, y)| point(x, y)).collect(),
    }
}

fn arc_semantics(
    start: (f64, f64),
    mid: (f64, f64),
    end: (f64, f64),
) -> (&'static str, &'static str) {
    let denominator =
        2.0 * (start.0 * (mid.1 - end.1) + mid.0 * (end.1 - start.1) + end.0 * (start.1 - mid.1));
    let squared = |point: (f64, f64)| point.0 * point.0 + point.1 * point.1;
    let center_x = (squared(start) * (mid.1 - end.1)
        + squared(mid) * (end.1 - start.1)
        + squared(end) * (start.1 - mid.1))
        / denominator;
    let center_y = (squared(start) * (end.0 - mid.0)
        + squared(mid) * (start.0 - end.0)
        + squared(end) * (mid.0 - start.0))
        / denominator;
    let angle = |point: (f64, f64)| (point.1 - center_y).atan2(point.0 - center_x);
    let start_angle = angle(start);
    let clockwise_sweep = (angle(end) - start_angle).rem_euclid(std::f64::consts::TAU);
    let clockwise_mid = (angle(mid) - start_angle).rem_euclid(std::f64::consts::TAU);
    let (direction, sweep) = if clockwise_mid <= clockwise_sweep + 1e-9 {
        ("cw", clockwise_sweep)
    } else {
        ("ccw", std::f64::consts::TAU - clockwise_sweep)
    };
    (
        direction,
        if sweep > std::f64::consts::PI {
            "major"
        } else {
            "minor"
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one complete zone fixture keeps source-ring relationships visible"
)]
fn board() -> AuthoredPcb {
    let layers = [
        (0, "F.Cu", "signal"),
        (1, "In1.Cu", "power"),
        (2, "In2.Cu", "power"),
        (31, "B.Cu", "signal"),
        (44, "Edge.Cuts", "user"),
    ]
    .into_iter()
    .map(|(ordinal, name, kind)| AuthoredLayer {
        ordinal,
        name: name.to_owned(),
        kind: kind.to_owned(),
        user_name: None,
    })
    .collect();
    let profile = [
        ((0.0, 0.0), (30.0, 0.0)),
        ((30.0, 0.0), (30.0, 30.0)),
        ((30.0, 30.0), (0.0, 30.0)),
        ((0.0, 30.0), (0.0, 0.0)),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (start, end))| AuthoredGraphic {
        geometry: AuthoredGraphicGeometry::Line {
            start: point(start.0, start.1),
            end: point(end.0, end.1),
        },
        layer: "Edge.Cuts".to_owned(),
        stroke_width_mm: 0.05,
        stroke_kind: "default".to_owned(),
        fill: None,
        uuid: uuid(10 + index as u64),
    })
    .collect();
    let zone = AuthoredZone {
        net: AuthoredNetRef {
            code: 1,
            name: "GND".to_owned(),
        },
        layers: vec!["F.Cu".to_owned(), "In1.Cu".to_owned()],
        locked: true,
        uuid: uuid(100),
        name: Some("GROUND POUR".to_owned()),
        hatch: AuthoredZoneHatch::Edge,
        hatch_pitch_mm: 0.5,
        priority: 3,
        pad_connection: AuthoredZonePadConnection::ThermalRelief,
        connect_pads_clearance_mm: 0.2,
        min_thickness_mm: 0.25,
        filled_areas_thickness: Some(false),
        fill: Some(AuthoredZoneFill {
            thermal_gap_mm: 0.3,
            thermal_bridge_width_mm: 0.4,
            island_removal_mode: Some(2),
            island_area_min_mm2: Some(1.0),
        }),
        outlines: vec![polygon(&[
            (1.0, 1.0),
            (29.0, 1.0),
            (29.0, 29.0),
            (1.0, 29.0),
        ])],
        filled_polygons: vec![
            // One authoritative bridge/fracture-encoded point chain. The
            // repeated (1,10)/(10,10) bridge preserves the hole; a second
            // opposite-winding filled_polygon would instead be another outline.
            filled(
                "F.Cu",
                false,
                &[
                    (29.0, 29.0),
                    (1.0, 29.0),
                    (1.0, 10.0),
                    (10.0, 10.0),
                    (10.0, 20.0),
                    (20.0, 20.0),
                    (20.0, 10.0),
                    (10.0, 10.0),
                    (1.0, 10.0),
                    (1.0, 1.0),
                    (29.0, 1.0),
                ],
            ),
            filled("F.Cu", true, &[(3.0, 3.0), (4.0, 3.0), (3.5, 4.0)]),
            filled(
                "In1.Cu",
                false,
                &[(2.0, 2.0), (28.0, 2.0), (28.0, 28.0), (2.0, 28.0)],
            ),
        ],
    };
    AuthoredPcb {
        layers,
        nets: vec![AuthoredNet {
            code: 1,
            name: "GND".to_owned(),
        }],
        profile,
        segments: vec![AuthoredSegment {
            start: point(2.0, 2.0),
            end: point(3.0, 2.0),
            width_mm: 0.2,
            layer: "F.Cu".to_owned(),
            net_code: 0,
            uuid: uuid(200),
        }],
        arcs: [
            ((6.0, 4.0), (5.414_213_562, 5.414_213_562), (4.0, 6.0)),
            ((12.0, 4.0), (8.585_786_438, 5.414_213_562), (10.0, 2.0)),
            ((18.0, 4.0), (17.414_213_562, 2.585_786_438), (16.0, 2.0)),
            ((24.0, 4.0), (20.585_786_438, 2.585_786_438), (22.0, 6.0)),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (start, mid, end))| AuthoredRoutingArc {
            start: point(start.0, start.1),
            mid: point(mid.0, mid.1),
            end: point(end.0, end.1),
            width_mm: 0.2,
            layer: "In1.Cu".to_owned(),
            net_code: 0,
            uuid: uuid(201 + index as u64),
        })
        .collect(),
        zones: vec![zone],
        ..AuthoredPcb::default()
    }
}

fn publish(name: &str, source: &str) {
    let Some(directory) = std::env::var_os("KM_ZONE_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create zone oracle directory");
    std::fs::write(directory.join(name), source).expect("write zone source");
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "one semantic proof intentionally checks every authored zone/cache field"
)]
fn authored_zone_intent_and_fill_cache_chains_round_trip_exactly() {
    let document = board()
        .to_document(PcbAuthoringLimits::default())
        .expect("authored zone board");
    publish("native-zone.kicad_pcb", document.source());
    let view = document.view().expect("board view");
    let zone = view.zones().next().expect("zone").expect("typed zone");
    assert_eq!(zone.net.ordinal, Some(1));
    assert_eq!(zone.net.name.as_deref(), Some("GND"));
    assert_eq!(zone.layers, ["F.Cu", "In1.Cu"]);
    assert!(zone.layers_plural);
    assert!(zone.locked);
    assert_eq!(zone.uuid.as_deref(), Some(uuid(100).as_str()));
    assert_eq!(zone.name.as_deref(), Some("GROUND POUR"));
    assert_eq!((zone.hatch_style.as_str(), zone.hatch_pitch), ("edge", 0.5));
    assert_eq!(zone.priority, 3);
    assert_eq!(zone.connect_pads_mode, None);
    assert_eq!(zone.connect_pads_clearance, 0.2);
    assert_eq!(zone.min_thickness, 0.25);
    assert!(zone.fill_enabled);
    assert_eq!((zone.thermal_gap, zone.thermal_bridge_width), (0.3, 0.4));
    assert_eq!(zone.island_removal_mode, Some(2));
    assert_eq!(zone.island_area_min, Some(1.0));
    assert_eq!(zone.polygons.len(), 1);
    assert_eq!(zone.polygons[0].points.len(), 4);
    assert_eq!(zone.filled_polygons.len(), 3);
    assert_eq!(
        zone.filled_polygons
            .iter()
            .map(|ring| (ring.layer.as_str(), ring.island, ring.points.len()))
            .collect::<Vec<_>>(),
        [("F.Cu", false, 11), ("F.Cu", true, 3), ("In1.Cu", false, 4),]
    );
    assert_eq!(
        zone.filled_polygons[0]
            .points
            .iter()
            .map(|point| (point.x, point.y))
            .collect::<Vec<_>>(),
        [
            (29.0, 29.0),
            (1.0, 29.0),
            (1.0, 10.0),
            (10.0, 10.0),
            (10.0, 20.0),
            (20.0, 20.0),
            (20.0, 10.0),
            (10.0, 10.0),
            (1.0, 10.0),
            (1.0, 1.0),
            (29.0, 1.0),
        ]
    );
    let segments = view
        .segments()
        .collect::<Result<Vec<_>, _>>()
        .expect("segments");
    let arcs = view.arcs().collect::<Result<Vec<_>, _>>().expect("arcs");
    assert_eq!(arcs.len(), 4);
    assert_eq!(segments[0].net.ordinal, Some(0));
    assert_eq!(arcs[0].net.ordinal, Some(0));
    assert_eq!(segments[0].layer.as_deref(), Some("F.Cu"));
    assert_eq!(arcs[0].layer.as_deref(), Some("In1.Cu"));
    assert_eq!(
        arcs.iter()
            .map(|arc| {
                arc_semantics(
                    (arc.start.x, arc.start.y),
                    (arc.mid.x, arc.mid.y),
                    (arc.end.x, arc.end.y),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("cw", "minor"),
            ("cw", "major"),
            ("ccw", "minor"),
            ("ccw", "major"),
        ]
    );
}

#[test]
fn zone_pad_connection_tokens_match_kicad_semantics() {
    for (mode, expected) in [
        (AuthoredZonePadConnection::ThermalRelief, None),
        (AuthoredZonePadConnection::NoConnection, Some("no")),
        (AuthoredZonePadConnection::Solid, Some("yes")),
        (
            AuthoredZonePadConnection::ThroughHoleOnly,
            Some("thru_hole_only"),
        ),
    ] {
        let mut input = board();
        input.zones[0].pad_connection = mode;
        let document = input
            .to_document(PcbAuthoringLimits::default())
            .expect("zone connection document");
        let actual = document
            .view()
            .expect("view")
            .zones()
            .next()
            .expect("zone")
            .expect("zone record")
            .connect_pads_mode;
        assert_eq!(actual.as_deref(), expected);
        if mode == AuthoredZonePadConnection::Solid {
            publish("native-zone-solid.kicad_pcb", document.source());
        }
    }
}

#[test]
fn zone_traversal_limits_accept_exact_counts_and_reject_one_under() {
    let exact = PcbAuthoringLimits {
        max_objects: 25,
        max_points: 46,
        ..PcbAuthoringLimits::default()
    };
    board().canonical_text(exact).expect("exact zone limits");

    for limits in [
        PcbAuthoringLimits {
            max_objects: 24,
            max_points: 46,
            ..PcbAuthoringLimits::default()
        },
        PcbAuthoringLimits {
            max_objects: 25,
            max_points: 45,
            ..PcbAuthoringLimits::default()
        },
    ] {
        let error = board()
            .canonical_text(limits)
            .expect_err("one-under zone limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }
}

#[test]
fn invalid_zone_and_unconnected_route_values_fail_before_emission() {
    let limits = PcbAuthoringLimits::default();
    let mut cases = Vec::new();

    let mut named_zero = board();
    named_zero.zones[0].net.code = 0;
    cases.push(named_zero);

    let mut unknown_net = board();
    unknown_net.zones[0].net.code = 2;
    cases.push(unknown_net);

    let mut no_layers = board();
    no_layers.zones[0].layers.clear();
    cases.push(no_layers);

    let mut odd_copper_count = board();
    odd_copper_count
        .layers
        .retain(|layer| layer.name != "In2.Cu");
    cases.push(odd_copper_count);

    let mut noncontiguous_copper = board();
    let layer = noncontiguous_copper
        .layers
        .iter_mut()
        .find(|layer| layer.name == "In2.Cu")
        .expect("In2.Cu");
    layer.ordinal = 3;
    layer.name = "In3.Cu".to_owned();
    cases.push(noncontiguous_copper);

    let mut duplicate_layers = board();
    duplicate_layers.zones[0].layers.push("F.Cu".to_owned());
    cases.push(duplicate_layers);

    let mut no_outlines = board();
    no_outlines.zones[0].outlines.clear();
    cases.push(no_outlines);

    let mut short_outline = board();
    short_outline.zones[0].outlines[0].points.truncate(2);
    cases.push(short_outline);

    let mut cache_without_fill = board();
    cache_without_fill.zones[0].fill = None;
    cases.push(cache_without_fill);

    let mut cache_outside_zone = board();
    cache_outside_zone.zones[0].filled_polygons[0].layer = "B.Cu".to_owned();
    cases.push(cache_outside_zone);

    let mut bad_island_mode = board();
    bad_island_mode.zones[0]
        .fill
        .as_mut()
        .expect("fill")
        .island_removal_mode = Some(3);
    cases.push(bad_island_mode);

    let mut orphan_island_area = board();
    orphan_island_area.zones[0]
        .fill
        .as_mut()
        .expect("fill")
        .island_removal_mode = Some(1);
    cases.push(orphan_island_area);

    let mut unsupported_fill_thickness_yes = board();
    unsupported_fill_thickness_yes.zones[0].filled_areas_thickness = Some(true);
    cases.push(unsupported_fill_thickness_yes);

    let mut unknown_segment_net = board();
    unknown_segment_net.segments[0].net_code = 2;
    cases.push(unknown_segment_net);

    let mut negative_arc_net = board();
    negative_arc_net.arcs[0].net_code = -1;
    cases.push(negative_arc_net);

    let mut noncopper_segment = board();
    noncopper_segment.segments[0].layer = "Edge.Cuts".to_owned();
    cases.push(noncopper_segment);

    let mut noncopper_arc = board();
    noncopper_arc.arcs[0].layer = "Edge.Cuts".to_owned();
    cases.push(noncopper_arc);

    for case in cases {
        let error = case
            .canonical_text(limits)
            .expect_err("invalid zone source");
        assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    }
}

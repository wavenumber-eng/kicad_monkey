#[path = "support/source_authoring/zones.rs"]
mod fixtures;
use fixtures::board;

use kicad_monkey_core::{
    AuthoredGraphic, AuthoredGraphicGeometry, AuthoredKeepout, AuthoredLayer, AuthoredNet,
    AuthoredNetRef, AuthoredPcb, AuthoredPlacementConstraint, AuthoredPlacementSource,
    AuthoredPoint, AuthoredRestriction, AuthoredRoutingArc, AuthoredRuleArea, AuthoredSegment,
    AuthoredZone, AuthoredZoneFill, AuthoredZoneFilledPolygon, AuthoredZoneHatch,
    AuthoredZonePadConnection, AuthoredZonePolygon, ErrorKind, PcbAuthoringLimits,
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

fn publish(name: &str, source: &str) {
    let Some(directory) = std::env::var_os("KM_ZONE_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create zone oracle directory");
    std::fs::write(directory.join(name), source).expect("write zone source");
}

#[test]
fn authored_rule_areas_preserve_restrictions_placement_and_outline_order() {
    let mut input = rule_area_board();
    let document = input
        .to_document(PcbAuthoringLimits::default())
        .expect("rule areas");
    publish("native-rule-areas.kicad_pcb", document.source());
    let view = document.view().expect("view");
    let actual = view.zones().collect::<Result<Vec<_>, _>>().expect("zones");
    assert_eq!(actual.len(), input.rule_areas.len());
    for (area, expected) in actual.iter().zip(&input.rule_areas) {
        assert_rule_area(area, expected);
    }
    let mut duplicate = input.clone();
    duplicate.rule_areas[1].uuid = duplicate.rule_areas[0].uuid.clone();
    assert!(
        duplicate
            .canonical_text(PcbAuthoringLimits::default())
            .is_err()
    );
    input.rule_areas[0].layers = vec!["Unknown".to_owned()];
    assert!(input.canonical_text(PcbAuthoringLimits::default()).is_err());
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

fn rule_area_board() -> AuthoredPcb {
    let mut input = board();
    input.zones.clear();
    let placements = [
        None,
        Some((
            false,
            AuthoredPlacementSource::SheetName("/Power stage".to_owned()),
        )),
        Some((
            true,
            AuthoredPlacementSource::ComponentClass("Fast \"IO\"".to_owned()),
        )),
        Some((
            true,
            AuthoredPlacementSource::Group("Local group".to_owned()),
        )),
    ];
    for (index, placement) in placements.into_iter().enumerate() {
        input.rule_areas.push(AuthoredRuleArea {
            layers: vec!["F.Cu".to_owned(), "B.Cu".to_owned()],
            locked: true,
            uuid: uuid(300 + index as u64),
            name: Some(format!("Rule {index}")),
            hatch: AuthoredZoneHatch::Edge,
            hatch_pitch_mm: 0.5,
            keepout: AuthoredKeepout {
                tracks: AuthoredRestriction::NotAllowed,
                vias: AuthoredRestriction::Allowed,
                pads: AuthoredRestriction::Allowed,
                copperpour: AuthoredRestriction::NotAllowed,
                footprints: AuthoredRestriction::Allowed,
            },
            placement: placement
                .map(|(enabled, source)| AuthoredPlacementConstraint { enabled, source }),
            outlines: vec![
                polygon(&[(1.0, 1.0), (9.0, 1.0), (9.0, 9.0), (1.0, 9.0)]),
                polygon(&[(3.0, 3.0), (3.0, 5.0), (5.0, 5.0), (5.0, 3.0)]),
            ],
        });
    }
    input
}

fn assert_rule_area(area: &kicad_monkey_core::PcbZone, expected: &AuthoredRuleArea) {
    assert_eq!(area.uuid.as_ref(), Some(&expected.uuid));
    assert_eq!(area.layers, expected.layers);
    assert!(area.locked);
    assert_eq!(area.net.ordinal, Some(0));
    assert!(!area.fill_enabled);
    assert!(area.filled_polygons.is_empty());
    assert_keepout(area);
    assert_eq!(
        area.placement
            .as_ref()
            .map(|p| (p.enabled, p.source_type.as_str(), p.source.as_str())),
        expected.placement.as_ref().map(|p| {
            let (kind, source) = p.source.source_pair();
            (p.enabled, kind, source)
        })
    );
    assert_eq!(
        area.polygons
            .iter()
            .map(|p| p.points.iter().map(|v| (v.x, v.y)).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        expected
            .outlines
            .iter()
            .map(|p| p
                .points
                .iter()
                .map(|v| (v.x_mm, v.y_mm))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
}

fn assert_keepout(area: &kicad_monkey_core::PcbZone) {
    let keepout = area.keepout.as_ref().expect("keepout");
    assert_eq!(
        (
            &*keepout.tracks,
            &*keepout.vias,
            &*keepout.pads,
            &*keepout.copperpour,
            &*keepout.footprints
        ),
        (
            "not_allowed",
            "allowed",
            "allowed",
            "not_allowed",
            "allowed"
        )
    );
    assert!(
        keepout.has_tracks
            && keepout.has_vias
            && keepout.has_pads
            && keepout.has_copperpour
            && keepout.has_footprints
    );
}

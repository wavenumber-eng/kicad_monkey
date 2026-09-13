use super::*;

fn zone() -> AuthoredZone {
    AuthoredZone {
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
    }
}

pub(super) fn board() -> AuthoredPcb {
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
        locked: false,
        stroke_width_mm: 0.05,
        stroke_kind: "default".to_owned(),
        fill: None,
        uuid: uuid(10 + index as u64),
    })
    .collect();
    let zone = zone();
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

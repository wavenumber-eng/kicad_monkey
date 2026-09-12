use kicad_monkey_core::{
    AuthoredChamferCorner, AuthoredDrill, AuthoredFootprint, AuthoredFootprintOccurrence,
    AuthoredFrontBackPolicy, AuthoredLayer, AuthoredNet, AuthoredNetRef, AuthoredPad,
    AuthoredPadKind, AuthoredPadShape, AuthoredPcb, AuthoredPoint, AuthoredVia, AuthoredViaKind,
    AuthoredZoneConnection, ErrorKind, PcbAuthoringLimits,
};

fn point(x_mm: f64, y_mm: f64) -> AuthoredPoint {
    AuthoredPoint { x_mm, y_mm }
}

fn uuid(value: u64) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

fn layers() -> Vec<AuthoredLayer> {
    [
        (0, "F.Cu", "signal"),
        (1, "In1.Cu", "power"),
        (2, "In2.Cu", "mixed"),
        (31, "B.Cu", "signal"),
        (34, "B.Paste", "user"),
        (35, "F.Paste", "user"),
        (38, "B.Mask", "user"),
        (39, "F.Mask", "user"),
    ]
    .into_iter()
    .map(|(ordinal, name, kind)| AuthoredLayer {
        ordinal,
        name: name.to_owned(),
        kind: kind.to_owned(),
        user_name: None,
    })
    .collect()
}

fn pad(number: &str, shape: AuthoredPadShape, uuid_value: u64) -> AuthoredPad {
    AuthoredPad {
        number: number.to_owned(),
        kind: AuthoredPadKind::Smd,
        shape,
        at: point(0.0, 0.0),
        angle_degrees: 0.0,
        size_x_mm: 2.0,
        size_y_mm: 1.0,
        drill: None,
        layers: vec!["F.Cu".to_owned(), "F.Paste".to_owned(), "F.Mask".to_owned()],
        net: Some(AuthoredNetRef {
            code: 1,
            name: "SIGNAL".to_owned(),
        }),
        uuid: uuid(uuid_value),
        solder_mask_margin_mm: None,
        solder_paste_margin_mm: None,
        solder_paste_margin_ratio: None,
        clearance_mm: None,
        thermal_bridge_width_mm: None,
        thermal_bridge_angle_degrees: None,
        thermal_gap_mm: None,
        zone_connect: None,
        zone_layer_connections: Vec::new(),
        remove_unused_layers: None,
        keep_end_layers: None,
    }
}

fn via(
    kind: AuthoredViaKind,
    start_layer: &str,
    end_layer: &str,
    x_mm: f64,
    uuid_value: u64,
) -> AuthoredVia {
    AuthoredVia {
        kind,
        at: point(x_mm, 10.0),
        size_mm: 0.8,
        drill_mm: 0.4,
        start_layer: start_layer.to_owned(),
        end_layer: end_layer.to_owned(),
        free: false,
        net_code: 1,
        uuid: uuid(uuid_value),
        tenting: None,
        covering: None,
        plugging: None,
        capping: None,
        filling: None,
        zone_layer_connections: Vec::new(),
        remove_unused_layers: None,
        keep_end_layers: None,
        start_end_only: None,
    }
}

fn authored_board() -> AuthoredPcb {
    let mut footprint = AuthoredFootprint::new("Demo:PadVia");

    let mut trapezoid = pad(
        "1",
        AuthoredPadShape::Trapezoid {
            delta_x_mm: 0.2,
            delta_y_mm: -0.1,
        },
        101,
    );
    trapezoid.at = point(-2.0, 0.0);
    trapezoid.solder_mask_margin_mm = Some(0.05);
    trapezoid.solder_paste_margin_mm = Some(-0.02);
    trapezoid.solder_paste_margin_ratio = Some(-0.1);
    trapezoid.clearance_mm = Some(0.08);
    trapezoid.thermal_bridge_width_mm = Some(0.3);
    trapezoid.thermal_bridge_angle_degrees = Some(45.0);
    trapezoid.thermal_gap_mm = Some(0.2);
    trapezoid.zone_connect = Some(AuthoredZoneConnection::Solid);

    let mut chamfered = pad(
        "2",
        AuthoredPadShape::ChamferedRoundRect {
            radius_ratio: 0.15,
            chamfer_ratio: 0.2,
            corners: vec![
                AuthoredChamferCorner::TopLeft,
                AuthoredChamferCorner::BottomRight,
            ],
        },
        102,
    );
    chamfered.at = point(2.0, 0.0);

    let mut through = pad("3", AuthoredPadShape::Oval, 103);
    through.kind = AuthoredPadKind::ThroughHole;
    through.at = point(0.0, 3.0);
    through.size_x_mm = 2.0;
    through.size_y_mm = 3.0;
    through.drill = Some(AuthoredDrill {
        width_mm: 0.8,
        height_mm: Some(1.6),
        offset: point(0.1, -0.1),
    });
    through.layers = vec!["*.Cu".to_owned(), "*.Mask".to_owned()];
    through.zone_layer_connections = vec!["In1.Cu".to_owned(), "In2.Cu".to_owned()];
    through.zone_connect = Some(AuthoredZoneConnection::ThermalReliefForThroughHole);
    through.remove_unused_layers = Some(true);
    through.keep_end_layers = Some(false);

    footprint.pads = vec![trapezoid, chamfered, through];
    let occurrence = AuthoredFootprintOccurrence {
        footprint,
        layer: "F.Cu".to_owned(),
        at: point(20.0, 20.0),
        angle_degrees: 0.0,
        uuid: uuid(100),
    };

    let mut through_via = via(AuthoredViaKind::Through, "F.Cu", "B.Cu", 10.0, 201);
    through_via.free = true;
    through_via.tenting = Some(AuthoredFrontBackPolicy {
        front: Some(false),
        back: Some(true),
    });
    through_via.zone_layer_connections = vec!["In2.Cu".to_owned()];
    through_via.remove_unused_layers = Some(true);
    through_via.keep_end_layers = Some(true);

    let mut blind = via(AuthoredViaKind::BlindBuried, "F.Cu", "In1.Cu", 12.0, 202);
    blind.covering = Some(AuthoredFrontBackPolicy {
        front: Some(true),
        back: None,
    });
    blind.capping = Some(true);

    let mut buried = via(AuthoredViaKind::BlindBuried, "In1.Cu", "In2.Cu", 14.0, 203);
    buried.plugging = Some(AuthoredFrontBackPolicy {
        front: None,
        back: Some(false),
    });
    buried.filling = Some(false);

    let mut micro = via(AuthoredViaKind::Micro, "In2.Cu", "B.Cu", 16.0, 204);
    micro.size_mm = 0.3;
    micro.drill_mm = 0.1;

    let mut start_end_only = via(AuthoredViaKind::Through, "F.Cu", "B.Cu", 18.0, 205);
    start_end_only.start_end_only = Some(true);

    let mut unconnected = via(AuthoredViaKind::Through, "F.Cu", "B.Cu", 20.0, 206);
    unconnected.net_code = 0;

    AuthoredPcb {
        layers: layers(),
        nets: vec![AuthoredNet {
            code: 1,
            name: "SIGNAL".to_owned(),
        }],
        footprints: vec![occurrence],
        vias: vec![
            through_via,
            blind,
            buried,
            micro,
            start_end_only,
            unconnected,
        ],
        ..AuthoredPcb::default()
    }
}

fn publish_for_independent_oracle(source: &str) {
    let Some(directory) = std::env::var_os("KM_PAD_VIA_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create independent-oracle output directory");
    std::fs::write(directory.join("native-pad-via.kicad_pcb"), source)
        .expect("write independent-oracle source");
}

#[test]
fn typed_zone_connection_codes_cover_the_complete_kicad_domain() {
    assert_eq!(
        [
            AuthoredZoneConnection::NoConnection,
            AuthoredZoneConnection::ThermalRelief,
            AuthoredZoneConnection::Solid,
            AuthoredZoneConnection::ThermalReliefForThroughHole,
        ]
        .map(AuthoredZoneConnection::source_code),
        [0, 1, 2, 3]
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "one semantic proof intentionally checks every typed pad/via field"
)]
fn typed_pad_via_source_round_trips_every_supported_policy() {
    let document = authored_board()
        .to_document(PcbAuthoringLimits::default())
        .expect("fresh pad/via board");
    publish_for_independent_oracle(document.source());
    let view = document.view().expect("board view");

    let pads = view.pads().collect::<Result<Vec<_>, _>>().expect("pads");
    assert_eq!(pads.len(), 3);
    assert_eq!(pads[0].shape, "trapezoid");
    assert_eq!(
        (pads[0].rect_delta_x, pads[0].rect_delta_y),
        (Some(0.2), Some(-0.1))
    );
    assert_eq!(pads[0].solder_mask_margin, Some(0.05));
    assert_eq!(pads[0].solder_paste_margin, Some(-0.02));
    assert_eq!(pads[0].solder_paste_margin_ratio, Some(-0.1));
    assert_eq!(pads[0].clearance, Some(0.08));
    assert_eq!(pads[0].thermal_bridge_width, Some(0.3));
    assert_eq!(pads[0].thermal_bridge_angle, Some(45.0));
    assert_eq!(pads[0].thermal_gap, Some(0.2));
    assert_eq!(pads[0].zone_connect, Some(2));
    assert_eq!(pads[1].shape, "roundrect");
    assert_eq!(pads[1].roundrect_rratio, Some(0.15));
    assert_eq!(pads[1].chamfer_ratio, Some(0.2));
    assert_eq!(pads[1].chamfer_corners, ["top_left", "bottom_right"]);
    let drill = pads[2].drill.as_ref().expect("plated slot");
    assert_eq!((drill.width, drill.height), (0.8, Some(1.6)));
    assert_eq!((drill.offset.x, drill.offset.y), (0.1, -0.1));
    assert_eq!(pads[2].remove_unused_layers, Some(true));
    assert_eq!(pads[2].keep_end_layers, Some(false));
    assert_eq!(pads[2].zone_connect, Some(3));
    assert_eq!(
        pads[2]
            .zone_layer_connections
            .as_ref()
            .expect("pad forced layers")
            .forced_layers,
        ["In1.Cu", "In2.Cu"]
    );

    let vias = view.vias().collect::<Result<Vec<_>, _>>().expect("vias");
    assert_eq!(vias.len(), 6);
    assert_eq!(vias[0].via_type, None);
    assert_eq!(vias[0].layers, ["F.Cu", "B.Cu"]);
    assert!(vias[0].free);
    assert_eq!(
        (
            vias[0].tenting.as_ref().and_then(|value| value.front),
            vias[0].tenting.as_ref().and_then(|value| value.back)
        ),
        (Some(false), Some(true))
    );
    assert_eq!(vias[0].remove_unused_layers, Some(true));
    assert_eq!(vias[0].keep_end_layers, Some(true));
    assert_eq!(vias[0].start_end_only, None);
    assert_eq!(vias[1].via_type.as_deref(), Some("blind"));
    assert_eq!(vias[1].layers, ["F.Cu", "In1.Cu"]);
    assert_eq!(
        vias[1].covering.as_ref().and_then(|value| value.front),
        Some(true)
    );
    assert_eq!(vias[1].capping, Some(true));
    assert_eq!(vias[2].via_type.as_deref(), Some("blind"));
    assert_eq!(vias[2].layers, ["In1.Cu", "In2.Cu"]);
    assert_eq!(
        vias[2].plugging.as_ref().and_then(|value| value.back),
        Some(false)
    );
    assert_eq!(vias[2].filling, Some(false));
    assert_eq!(vias[3].via_type.as_deref(), Some("micro"));
    assert_eq!(vias[3].layers, ["In2.Cu", "B.Cu"]);
    assert_eq!((vias[3].size, vias[3].drill), (0.3, 0.1));
    assert_eq!(vias[4].layers, ["F.Cu", "B.Cu"]);
    assert_eq!(vias[4].start_end_only, Some(true));
    assert!(vias[..5].iter().all(|value| value.net.ordinal == Some(1)));
    assert_eq!(vias[5].layers, ["F.Cu", "B.Cu"]);
    assert_eq!(vias[5].net.ordinal, Some(0));
    assert_eq!(vias[5].net.name, None);
}

#[test]
fn pad_via_invalid_or_lossy_combinations_fail_before_emission() {
    let limits = PcbAuthoringLimits::default();
    let mut cases = Vec::new();

    let mut through_span = authored_board();
    through_span.vias[0].end_layer = "In2.Cu".to_owned();
    cases.push(through_span);

    let mut complete_blind = authored_board();
    complete_blind.vias[1].end_layer = "B.Cu".to_owned();
    cases.push(complete_blind);

    let mut nonadjacent_micro = authored_board();
    nonadjacent_micro.vias[3].start_layer = "F.Cu".to_owned();
    cases.push(nonadjacent_micro);

    let mut reversed = authored_board();
    reversed.vias[2].start_layer = "In2.Cu".to_owned();
    reversed.vias[2].end_layer = "In1.Cu".to_owned();
    cases.push(reversed);

    let mut oversized_drill = authored_board();
    oversized_drill.vias[0].drill_mm = 0.9;
    cases.push(oversized_drill);

    let mut negative_net = authored_board();
    negative_net.vias[0].net_code = -1;
    cases.push(negative_net);

    let mut empty_policy = authored_board();
    empty_policy.vias[0].tenting = Some(AuthoredFrontBackPolicy::default());
    cases.push(empty_policy);

    let mut orphan_land_policy = authored_board();
    orphan_land_policy.vias[0].remove_unused_layers = None;
    cases.push(orphan_land_policy);

    let mut lossy_false_start_end = authored_board();
    lossy_false_start_end.vias[0].start_end_only = Some(false);
    cases.push(lossy_false_start_end);

    let mut conflicting_land_modes = authored_board();
    conflicting_land_modes.vias[0].start_end_only = Some(true);
    cases.push(conflicting_land_modes);

    let mut lossy_false_remove = authored_board();
    lossy_false_remove.vias[0].remove_unused_layers = Some(false);
    lossy_false_remove.vias[0].keep_end_layers = None;
    cases.push(lossy_false_remove);

    let mut duplicate_chamfer = authored_board();
    duplicate_chamfer.footprints[0].footprint.pads[1].shape =
        AuthoredPadShape::ChamferedRoundRect {
            radius_ratio: 0.1,
            chamfer_ratio: 0.2,
            corners: vec![
                AuthoredChamferCorner::TopLeft,
                AuthoredChamferCorner::TopLeft,
            ],
        };
    cases.push(duplicate_chamfer);

    let mut orphan_pad_land_policy = authored_board();
    orphan_pad_land_policy.footprints[0].footprint.pads[2].remove_unused_layers = None;
    cases.push(orphan_pad_land_policy);

    let mut smd_forced_layers = authored_board();
    smd_forced_layers.footprints[0].footprint.pads[0].zone_layer_connections =
        vec!["In1.Cu".to_owned()];
    cases.push(smd_forced_layers);

    let mut orphan_forced_layers = authored_board();
    orphan_forced_layers.footprints[0].footprint.pads[2].remove_unused_layers = None;
    orphan_forced_layers.footprints[0].footprint.pads[2].keep_end_layers = None;
    cases.push(orphan_forced_layers);

    let mut orphan_via_forced_layers = authored_board();
    orphan_via_forced_layers.vias[5].zone_layer_connections = vec!["In1.Cu".to_owned()];
    cases.push(orphan_via_forced_layers);

    let mut duplicate_pad_layers = authored_board();
    duplicate_pad_layers.footprints[0].footprint.pads[0]
        .layers
        .push("F.Cu".to_owned());
    cases.push(duplicate_pad_layers);

    let mut duplicate_pad_forced_layers = authored_board();
    duplicate_pad_forced_layers.footprints[0].footprint.pads[2]
        .zone_layer_connections
        .push("In1.Cu".to_owned());
    cases.push(duplicate_pad_forced_layers);

    let mut duplicate_via_forced_layers = authored_board();
    duplicate_via_forced_layers.vias[0]
        .zone_layer_connections
        .push("In2.Cu".to_owned());
    cases.push(duplicate_via_forced_layers);

    for case in cases {
        let error = case
            .canonical_text(limits)
            .expect_err("invalid authored source");
        assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    }
}

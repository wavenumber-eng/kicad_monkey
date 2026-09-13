use super::*;

pub(super) fn point(x_mm: f64, y_mm: f64) -> AuthoredPoint {
    AuthoredPoint { x_mm, y_mm }
}

pub(super) fn uuid(value: u64) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

pub(super) fn layers() -> Vec<AuthoredLayer> {
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

pub(super) fn pad(number: &str, shape: AuthoredPadShape, uuid_value: u64) -> AuthoredPad {
    AuthoredPad {
        number: number.to_owned(),
        padstack: None,
        pin_function: None,
        pin_type: None,
        die_length_mm: None,
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

pub(super) fn via(
    kind: AuthoredViaKind,
    start_layer: &str,
    end_layer: &str,
    x_mm: f64,
    uuid_value: u64,
) -> AuthoredVia {
    AuthoredVia {
        kind,
        padstack: None,
        at: point(x_mm, 10.0),
        size_mm: 0.8,
        drill_mm: 0.4,
        backdrill: None,
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

pub(super) fn authored_board() -> AuthoredPcb {
    let occurrence = pad_occurrence();
    let vias = policy_vias();
    AuthoredPcb {
        layers: layers(),
        nets: vec![AuthoredNet {
            code: 1,
            name: "SIGNAL".to_owned(),
        }],
        footprints: vec![occurrence],
        vias,
        ..AuthoredPcb::default()
    }
}

pub(super) fn publish_for_independent_oracle(source: &str) {
    let Some(directory) = std::env::var_os("KM_PAD_VIA_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create independent-oracle output directory");
    std::fs::write(directory.join("native-pad-via.kicad_pcb"), source)
        .expect("write independent-oracle source");
}

pub(super) fn stack_layer(
    layer: AuthoredPadstackLayerSelector,
    shape: AuthoredPadShape,
    size_x_mm: f64,
    size_y_mm: f64,
) -> AuthoredPadstackLayer {
    AuthoredPadstackLayer::Land {
        layer,
        shape,
        size_x_mm,
        size_y_mm,
        offset: point(0.2, -0.3),
        clearance_mm: Some(0.0),
        thermal_bridge_width_mm: Some(0.25),
        thermal_gap_mm: Some(0.2),
        thermal_bridge_angle_degrees: None,
        zone_connect: Some(AuthoredPadstackZoneConnection::Inherited),
    }
}

pub(super) fn slotted_through_pad() -> AuthoredPad {
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
    through.padstack = Some(AuthoredPadstack {
        mode: AuthoredPadstackMode::FrontInnerBack,
        layers: vec![
            stack_layer(
                AuthoredPadstackLayerSelector::Inner,
                AuthoredPadShape::Circle,
                1.2,
                1.2,
            ),
            stack_layer(
                AuthoredPadstackLayerSelector::CopperLayer("B.Cu".into()),
                AuthoredPadShape::Rect,
                1.4,
                1.8,
            ),
        ],
    });

    through
}

pub(super) fn pad_occurrence() -> AuthoredFootprintOccurrence {
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

    let through = slotted_through_pad();
    let mut cut_only = pad("", AuthoredPadShape::Circle, 104);
    cut_only.kind = AuthoredPadKind::NonPlatedThroughHole;
    cut_only.at = point(4.0, 3.0);
    cut_only.drill = Some(AuthoredDrill {
        width_mm: 0.8,
        height_mm: None,
        offset: point(0.1, -0.1),
    });
    cut_only.layers.clear();
    cut_only.net = None;
    footprint.pads = vec![trapezoid, chamfered, through, cut_only];
    AuthoredFootprintOccurrence {
        footprint,
        layer: "F.Cu".to_owned(),
        at: point(20.0, 20.0),
        angle_degrees: 0.0,
        uuid: uuid(100),
        locked: false,
        placement_path: None,
        placement_sheet_name: None,
        placement_sheet_file: None,
    }
}

pub(super) fn policy_vias() -> Vec<AuthoredVia> {
    let mut through_via = via(AuthoredViaKind::Through, "F.Cu", "B.Cu", 10.0, 201);
    through_via.backdrill = Some(AuthoredBackdrill {
        size_mm: 0.5,
        start_layer: "B.Cu".to_owned(),
        end_layer: "In2.Cu".to_owned(),
    });
    through_via.free = true;
    through_via.tenting = Some(AuthoredFrontBackPolicy {
        front: Some(false),
        back: Some(true),
    });
    through_via.zone_layer_connections = vec!["In2.Cu".to_owned()];
    through_via.remove_unused_layers = Some(true);
    through_via.keep_end_layers = Some(true);
    through_via.padstack = Some(AuthoredViaStack {
        mode: AuthoredPadstackMode::Custom,
        layers: vec![
            AuthoredViaStackLayer {
                layer: AuthoredPadstackLayerSelector::CopperLayer("In1.Cu".into()),
                size_mm: 0.6,
            },
            AuthoredViaStackLayer {
                layer: AuthoredPadstackLayerSelector::CopperLayer("B.Cu".into()),
                size_mm: 0.7,
            },
        ],
    });

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

    vec![
        through_via,
        blind,
        buried,
        micro,
        start_end_only,
        unconnected,
    ]
}

pub(super) fn sparse_footprint() -> AuthoredStandaloneFootprint {
    use kicad_monkey_core::{
        AuthoredPadAnchor, AuthoredPadPrimitive, AuthoredPadPrimitiveGeometry,
    };
    let mut local = AuthoredStandaloneFootprint::new("SparseStack", "F.Cu");
    let mut pad = pad("1", AuthoredPadShape::Circle, 300);
    pad.net = None;
    pad.size_y_mm = 2.0;
    pad.layers = vec!["*.Cu".into(), "*.Mask".into()];
    pad.kind = AuthoredPadKind::ThroughHole;
    pad.drill = Some(AuthoredDrill {
        width_mm: 0.4,
        height_mm: None,
        offset: point(0.0, 0.0),
    });
    pad.padstack = Some(AuthoredPadstack {
        mode: AuthoredPadstackMode::Custom,
        layers: vec![stack_layer(
            AuthoredPadstackLayerSelector::CopperLayer("B.Cu".into()),
            AuthoredPadShape::Custom {
                anchor: Some(AuthoredPadAnchor::Circle),
                clearance: None,
                primitives: vec![AuthoredPadPrimitive {
                    geometry: AuthoredPadPrimitiveGeometry::Line {
                        start: point(-0.4, 0.0),
                        end: point(0.4, 0.2),
                    },
                    width_mm: 0.1,
                    fill: None,
                }],
            },
            1.4,
            1.8,
        )],
    });
    local.footprint.pads.push(pad);
    local
}

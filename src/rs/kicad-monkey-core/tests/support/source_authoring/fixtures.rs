use super::*;

pub(super) fn point(x_mm: f64, y_mm: f64) -> AuthoredPoint {
    AuthoredPoint { x_mm, y_mm }
}

pub(super) fn uuid(value: u64) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

pub(super) fn effective_surface_policy(
    local: Option<f64>,
    footprint: Option<f64>,
    board: f64,
) -> f64 {
    local.or(footprint).unwrap_or(board)
}

pub(super) fn publish_for_independent_oracle(name: &str, source: &str) {
    let Some(directory) = std::env::var_os("KM_AUTHORING_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create independent-oracle output directory");
    std::fs::write(directory.join(name), source).expect("write independent-oracle source");
}

pub(super) fn layers() -> Vec<AuthoredLayer> {
    [
        (0, "F.Cu", "signal"),
        (31, "B.Cu", "signal"),
        (32, "B.Adhes", "user"),
        (33, "F.Adhes", "user"),
        (34, "B.Paste", "user"),
        (35, "F.Paste", "user"),
        (36, "B.SilkS", "user"),
        (37, "F.SilkS", "user"),
        (38, "B.Mask", "user"),
        (39, "F.Mask", "user"),
        (44, "Edge.Cuts", "user"),
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

pub(super) fn text_effects() -> AuthoredTextEffects {
    AuthoredTextEffects {
        size_x_mm: 1.25,
        size_y_mm: 0.75,
        thickness_mm: Some(0.15),
        ..AuthoredTextEffects::default()
    }
}

fn board_smd_pad(base: u64, layer: &str) -> AuthoredPad {
    AuthoredPad {
        number: "2".to_owned(),
        padstack: None,
        pin_function: None,
        pin_type: None,
        die_length_mm: None,
        kind: AuthoredPadKind::Smd,
        shape: AuthoredPadShape::Rect,
        at: point(1.5, 0.0),
        angle_degrees: 0.0,
        size_x_mm: 1.0,
        size_y_mm: 1.5,
        drill: None,
        layers: if layer == "B.Cu" {
            vec!["B.Cu".to_owned(), "B.Paste".to_owned(), "B.Mask".to_owned()]
        } else {
            vec!["F.Cu".to_owned(), "F.Paste".to_owned(), "F.Mask".to_owned()]
        },
        net: Some(AuthoredNetRef {
            code: 1,
            name: "GND".to_owned(),
        }),
        uuid: uuid(base + 5),
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

pub(super) fn board_footprint(
    base: u64,
    reference: &str,
    layer: &str,
) -> AuthoredFootprintOccurrence {
    let mut footprint = AuthoredFootprint::new("Demo:Native");
    footprint.attributes = vec!["through_hole".to_owned(), "dnp".to_owned()];
    footprint.properties.push(AuthoredFootprintProperty {
        name: "Reference".to_owned(),
        value: reference.to_owned(),
        at: point(0.0, -2.0),
        angle_degrees: 0.0,
        layer: if layer == "B.Cu" {
            "B.SilkS".to_owned()
        } else {
            "F.SilkS".to_owned()
        },
        hidden: false,
        unlocked: false,
        effects: text_effects(),
        render_cache: None,
        uuid: uuid(base + 1),
    });
    footprint.pads.push(board_through_pad(base));
    footprint.pads.push(board_npth_pad(base));
    footprint.pads.push(board_smd_pad(base, layer));
    footprint.graphics.push(AuthoredGraphic {
        geometry: AuthoredGraphicGeometry::Line {
            start: point(-1.0, 1.0),
            end: point(1.0, 1.0),
        },
        layer: if layer == "B.Cu" {
            "B.SilkS".to_owned()
        } else {
            "F.SilkS".to_owned()
        },
        stroke_width_mm: 0.15,
        stroke_kind: "default".to_owned(),
        fill: None,
        uuid: uuid(base + 4),
    });
    AuthoredFootprintOccurrence {
        footprint,
        layer: layer.to_owned(),
        at: point(0.0, 0.0),
        angle_degrees: 0.0,
        uuid: uuid(base),
        locked: false,
        placement_path: None,
        placement_sheet_name: None,
        placement_sheet_file: None,
    }
}

fn profile_line(base: u64, start: (f64, f64), end: (f64, f64)) -> AuthoredGraphic {
    AuthoredGraphic {
        geometry: AuthoredGraphicGeometry::Line {
            start: point(start.0, start.1),
            end: point(end.0, end.1),
        },
        layer: "Edge.Cuts".to_owned(),
        stroke_width_mm: 0.05,
        stroke_kind: "default".to_owned(),
        fill: None,
        uuid: uuid(base),
    }
}

fn add_embedded_model(occurrence: &mut AuthoredFootprintOccurrence) {
    occurrence.footprint.models.push(AuthoredModel {
        path: "kicad-embed://footprint.step".to_owned(),
        offset_mm: [0.5, 1.0, 1.5],
        scale: [1.0, 2.0, 1.0],
        rotate_degrees: [0.0, 45.0, 90.0],
    });
    occurrence
        .footprint
        .embedded_files
        .push(AuthoredEmbeddedFile {
            name: "footprint.step".to_owned(),
            file_type: "model".to_owned(),
            data: AuthoredResourceData::Bytes(b"footprint-owned shared model".to_vec()),
        });
}

fn authored_stackup() -> AuthoredStackup {
    AuthoredStackup {
        layers: vec![
            AuthoredStackupLayer {
                name: "F.Cu".to_owned(),
                type_name: "copper".to_owned(),
                thickness_mm: Some(0.035),
                thickness_locked: false,
                material: None,
                epsilon_r: None,
                loss_tangent: None,
                color: None,
            },
            AuthoredStackupLayer {
                name: "dielectric 1".to_owned(),
                type_name: "core".to_owned(),
                thickness_mm: Some(1.53),
                thickness_locked: true,
                material: Some("FR4".to_owned()),
                epsilon_r: Some(4.5),
                loss_tangent: Some(0.02),
                color: None,
            },
            AuthoredStackupLayer {
                name: "B.Cu".to_owned(),
                type_name: "copper".to_owned(),
                thickness_mm: Some(0.035),
                thickness_locked: false,
                material: None,
                epsilon_r: None,
                loss_tangent: None,
                color: None,
            },
        ],
        copper_finish: Some("ENIG".to_owned()),
        dielectric_constraints: true,
        edge_connector: None,
        edge_plating: false,
    }
}

pub(super) fn authored_board() -> AuthoredPcb {
    let mut front = board_footprint(100, "U1", "F.Cu");
    front.at = point(10.0, 20.0);
    front.locked = true;
    front.placement_path = Some(format!("/{}/{}", uuid(700), uuid(701)));
    front.placement_sheet_name = Some("Power".to_owned());
    front.placement_sheet_file = Some("power.kicad_sch".to_owned());
    front.footprint.clearance_mm = Some(0.0);
    front.footprint.zone_connect = Some(AuthoredZoneConnection::Solid);
    front.footprint.pads[0].pin_function = Some("VDD".to_owned());
    front.footprint.pads[0].pin_type = Some("power_in".to_owned());
    front.footprint.pads[0].die_length_mm = Some(0.75);
    front.footprint.solder_mask_margin_mm = Some(0.03);
    front.footprint.solder_paste_margin_mm = Some(-0.01);
    front.footprint.solder_paste_margin_ratio = Some(-0.05);
    front.footprint.pads[2].solder_mask_margin_mm = Some(0.05);
    front.footprint.pads[2].solder_paste_margin_mm = Some(-0.02);
    front.footprint.pads[2].solder_paste_margin_ratio = Some(-0.1);
    add_embedded_model(&mut front);
    let mut bottom = board_footprint(200, "U1", "B.Cu");
    bottom.at = point(40.0, 30.0);
    bottom.angle_degrees = 90.0;
    bottom.footprint.pads[0].solder_mask_margin_mm = None;
    let outer = [
        ((0.0, 0.0), (50.0, 0.0)),
        ((50.0, 0.0), (50.0, 40.0)),
        ((50.0, 40.0), (0.0, 40.0)),
        ((0.0, 40.0), (0.0, 0.0)),
    ];
    let cutout = [
        ((20.0, 15.0), (30.0, 15.0)),
        ((30.0, 15.0), (30.0, 25.0)),
        ((30.0, 25.0), (20.0, 25.0)),
        ((20.0, 25.0), (20.0, 15.0)),
    ];
    AuthoredPcb {
        layers: layers(),
        properties: vec![
            kicad_monkey_core::AuthoredProperty {
                name: "REVISION".to_owned(),
                value: "A \"prototype\"".to_owned(),
            },
            kicad_monkey_core::AuthoredProperty {
                name: "EMPTY".to_owned(),
                value: String::new(),
            },
        ],
        setup: AuthoredSetup {
            aux_axis_origin: point(5.0, 6.0),
            grid_origin: point(1.0, 2.0),
            pad_to_mask_clearance_mm: 0.01,
            pad_to_paste_clearance_mm: -0.005,
            pad_to_paste_clearance_ratio: -0.02,
            allow_soldermask_bridges_in_footprints: true,
            tenting_front: true,
            tenting_back: false,
            stackup: Some(authored_stackup()),
        },
        nets: vec![AuthoredNet {
            code: 1,
            name: "GND".to_owned(),
        }],
        profile: outer
            .into_iter()
            .chain(cutout)
            .enumerate()
            .map(|(index, (start, end))| profile_line(10 + index as u64, start, end))
            .collect(),
        footprints: vec![front, bottom],
        segments: vec![AuthoredSegment {
            start: point(10.0, 20.0),
            end: point(20.0, 20.0),
            width_mm: 0.25,
            layer: "F.Cu".to_owned(),
            net_code: 1,
            uuid: uuid(300),
        }],
        arcs: vec![AuthoredRoutingArc {
            start: point(20.0, 20.0),
            mid: point(25.0, 25.0),
            end: point(30.0, 20.0),
            width_mm: 0.25,
            layer: "F.Cu".to_owned(),
            net_code: 1,
            uuid: uuid(301),
        }],
        embedded_files: vec![AuthoredEmbeddedFile {
            name: "board.step".to_owned(),
            file_type: "model".to_owned(),
            data: AuthoredResourceData::Bytes(b"board-owned shared model".to_vec()),
        }],
        ..AuthoredPcb::default()
    }
}

pub(super) fn standalone_footprint() -> AuthoredStandaloneFootprint {
    let mut standalone = AuthoredStandaloneFootprint::new("Demo_Standalone", "F.Cu");
    let footprint = &mut standalone.footprint;
    footprint.attributes.push("dnp".to_owned());
    footprint.description = Some("Fresh typed footprint".to_owned());
    footprint.solder_mask_margin_mm = Some(0.02);
    footprint.solder_paste_margin_mm = Some(-0.01);
    footprint.solder_paste_margin_ratio = Some(-0.05);
    footprint.clearance_mm = Some(0.12);
    footprint.zone_connect = Some(AuthoredZoneConnection::NoConnection);
    footprint.properties = vec![
        AuthoredFootprintProperty {
            name: "Reference".to_owned(),
            value: "REF**".to_owned(),
            at: point(0.0, -2.0),
            angle_degrees: 0.0,
            layer: "F.SilkS".to_owned(),
            hidden: false,
            unlocked: false,
            effects: text_effects(),
            render_cache: None,
            uuid: uuid(405),
        },
        AuthoredFootprintProperty {
            name: "Value".to_owned(),
            value: "Demo_Standalone".to_owned(),
            at: point(0.0, 2.0),
            angle_degrees: 0.0,
            layer: "F.Fab".to_owned(),
            hidden: true,
            unlocked: false,
            effects: text_effects(),
            render_cache: None,
            uuid: uuid(406),
        },
    ];
    footprint.texts = vec![AuthoredFootprintText {
        text: "ASSEMBLY".to_owned(),
        at: point(0.0, 0.0),
        angle_degrees: 30.0,
        layer: "F.Fab".to_owned(),
        hidden: false,
        unlocked: false,
        knockout: false,
        effects: text_effects(),
        render_cache: None,
        uuid: uuid(401),
    }];
    footprint.graphics.push(AuthoredGraphic {
        geometry: AuthoredGraphicGeometry::Rect {
            start: point(-2.0, -1.0),
            end: point(2.0, 1.0),
        },
        layer: "F.SilkS".to_owned(),
        stroke_width_mm: 0.15,
        stroke_kind: "default".to_owned(),
        fill: Some("none".to_owned()),
        uuid: uuid(403),
    });
    footprint.pads.push(standalone_custom_pad());
    let mut cut_only = board_footprint(500, "REF**", "F.Cu")
        .footprint
        .pads
        .remove(1);
    cut_only.layers.clear();
    footprint.pads.push(cut_only);
    footprint.models.push(AuthoredModel {
        path: "kicad-embed://native.step".to_owned(),
        offset_mm: [1.0, 2.0, 3.0],
        scale: [1.0, 1.0, 1.0],
        rotate_degrees: [0.0, 0.0, 90.0],
    });
    footprint.embedded_files.push(AuthoredEmbeddedFile {
        name: "native.step".to_owned(),
        file_type: "model".to_owned(),
        data: AuthoredResourceData::Bytes(b"tiny synthetic model bytes".to_vec()),
    });
    standalone
}

pub(super) fn composite_custom_shape() -> AuthoredPadShape {
    let solid = Some(AuthoredPadPrimitiveFill::Solid);
    AuthoredPadShape::Custom {
        anchor: Some(AuthoredPadAnchor::Circle),
        clearance: Some(AuthoredCustomPadClearance::ConvexHull),
        primitives: vec![
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Polygon {
                    points: vec![point(-1.0, -0.5), point(1.0, -0.5), point(0.0, 1.0)]
                        .into_iter()
                        .map(AuthoredPadPolygonPoint::Xy)
                        .collect(),
                },
                width_mm: 0.01,
                fill: solid,
            },
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Line {
                    start: point(-1.0, 0.0),
                    end: point(1.0, 0.0),
                },
                width_mm: 0.2,
                fill: None,
            },
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Arc {
                    start: point(1.0, 0.0),
                    mid: point(0.0, 1.0),
                    end: point(-1.0, 0.0),
                },
                width_mm: 0.1,
                fill: None,
            },
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Circle {
                    center: point(0.0, 0.0),
                    end: point(0.5, 0.0),
                },
                width_mm: 0.0,
                fill: solid,
            },
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Rect {
                    start: point(-0.5, -0.5),
                    end: point(0.5, 0.5),
                    radius_mm: Some(0.1),
                },
                width_mm: 0.1,
                fill: Some(AuthoredPadPrimitiveFill::Unfilled),
            },
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Curve {
                    points: [
                        point(-1.0, 0.0),
                        point(-0.5, 1.0),
                        point(0.5, 1.0),
                        point(1.0, 0.0),
                    ],
                },
                width_mm: 0.1,
                fill: None,
            },
            AuthoredPadPrimitive {
                geometry: AuthoredPadPrimitiveGeometry::Polygon {
                    points: vec![
                        AuthoredPadPolygonPoint::Xy(point(1.0, 0.0)),
                        AuthoredPadPolygonPoint::Arc {
                            start: point(1.0, 0.0),
                            mid: point(0.0, 1.0),
                            end: point(-1.0, 0.0),
                        },
                        AuthoredPadPolygonPoint::Xy(point(0.0, -1.0)),
                    ],
                },
                width_mm: 0.0,
                fill: solid,
            },
        ],
    }
}

fn board_through_pad(base: u64) -> AuthoredPad {
    AuthoredPad {
        number: "1".to_owned(),
        kind: AuthoredPadKind::ThroughHole,
        padstack: None,
        pin_function: None,
        pin_type: None,
        die_length_mm: None,
        shape: AuthoredPadShape::Circle,
        at: point(0.0, 0.0),
        angle_degrees: 0.0,
        size_x_mm: 2.0,
        size_y_mm: 2.0,
        drill: Some(AuthoredDrill {
            width_mm: 0.8,
            height_mm: None,
            offset: point(0.0, 0.0),
        }),
        layers: vec!["*.Cu".to_owned(), "*.Mask".to_owned()],
        net: Some(AuthoredNetRef {
            code: 1,
            name: "GND".to_owned(),
        }),
        uuid: uuid(base + 2),
        solder_mask_margin_mm: Some(0.05),
        solder_paste_margin_mm: None,
        solder_paste_margin_ratio: None,
        clearance_mm: None,
        thermal_bridge_width_mm: None,
        thermal_bridge_angle_degrees: None,
        thermal_gap_mm: None,
        zone_connect: None,
        zone_layer_connections: Vec::new(),
        remove_unused_layers: Some(true),
        keep_end_layers: Some(true),
    }
}

fn board_npth_pad(base: u64) -> AuthoredPad {
    AuthoredPad {
        number: String::new(),
        padstack: None,
        pin_function: None,
        pin_type: None,
        die_length_mm: None,
        kind: AuthoredPadKind::NonPlatedThroughHole,
        shape: AuthoredPadShape::Oval,
        at: point(3.0, 0.0),
        angle_degrees: 90.0,
        size_x_mm: 2.0,
        size_y_mm: 3.0,
        drill: Some(AuthoredDrill {
            width_mm: 0.8,
            height_mm: Some(1.6),
            offset: point(0.1, -0.1),
        }),
        layers: vec!["*.Cu".to_owned(), "*.Mask".to_owned()],
        net: None,
        uuid: uuid(base + 3),
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

fn standalone_custom_pad() -> AuthoredPad {
    AuthoredPad {
        number: "1".to_owned(),
        kind: AuthoredPadKind::Smd,
        padstack: None,
        pin_function: None,
        pin_type: None,
        die_length_mm: Some(-0.25),
        shape: composite_custom_shape(),
        at: point(0.0, 0.0),
        angle_degrees: 0.0,
        size_x_mm: 2.0,
        size_y_mm: 2.0,
        drill: None,
        layers: vec!["F.Cu".to_owned(), "F.Paste".to_owned(), "F.Mask".to_owned()],
        net: None,
        uuid: uuid(404),
        solder_mask_margin_mm: Some(0.04),
        solder_paste_margin_mm: Some(-0.02),
        solder_paste_margin_ratio: Some(-0.1),
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

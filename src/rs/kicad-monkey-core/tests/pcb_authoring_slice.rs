use kicad_monkey_core::{
    AuthoredDrill, AuthoredEmbeddedFile, AuthoredFootprint, AuthoredFootprintOccurrence,
    AuthoredFootprintProperty, AuthoredFootprintText, AuthoredGraphic, AuthoredGraphicGeometry,
    AuthoredLayer, AuthoredModel, AuthoredNet, AuthoredNetRef, AuthoredPad, AuthoredPadKind,
    AuthoredPadShape, AuthoredPcb, AuthoredPoint, AuthoredResourceData, AuthoredRoutingArc,
    AuthoredSegment, AuthoredSetup, AuthoredStackup, AuthoredStackupLayer,
    AuthoredStandaloneFootprint, AuthoredTextEffects, EmbeddedDecodeLimits, EmbeddedFileOwner,
    ErrorKind, FootprintDocument, FootprintLimits, PcbAuthoringLimits, PcbFootprintMemberOwner,
    PcbProfileOwner,
};
use std::io::{Cursor, Write};

fn point(x_mm: f64, y_mm: f64) -> AuthoredPoint {
    AuthoredPoint { x_mm, y_mm }
}

fn uuid(value: u64) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

fn effective_surface_policy(local: Option<f64>, footprint: Option<f64>, board: f64) -> f64 {
    local.or(footprint).unwrap_or(board)
}

fn publish_for_independent_oracle(name: &str, source: &str) {
    let Some(directory) = std::env::var_os("KM_AUTHORING_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create independent-oracle output directory");
    std::fs::write(directory.join(name), source).expect("write independent-oracle source");
}

fn layers() -> Vec<AuthoredLayer> {
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

fn text_effects() -> AuthoredTextEffects {
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

fn board_footprint(base: u64, reference: &str, layer: &str) -> AuthoredFootprintOccurrence {
    let mut footprint = AuthoredFootprint::new("Demo:Native");
    footprint.attributes = vec!["through_hole".to_owned()];
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
    footprint.pads.push(AuthoredPad {
        number: "1".to_owned(),
        kind: AuthoredPadKind::ThroughHole,
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
    });
    footprint.pads.push(AuthoredPad {
        number: String::new(),
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
    });
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

fn authored_board() -> AuthoredPcb {
    let mut front = board_footprint(100, "U1", "F.Cu");
    front.at = point(10.0, 20.0);
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
        setup: AuthoredSetup {
            aux_axis_origin: point(5.0, 6.0),
            grid_origin: point(1.0, 2.0),
            pad_to_mask_clearance_mm: 0.01,
            pad_to_paste_clearance_mm: -0.005,
            pad_to_paste_clearance_ratio: -0.02,
            allow_soldermask_bridges_in_footprints: true,
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

fn standalone_footprint() -> AuthoredStandaloneFootprint {
    let mut standalone = AuthoredStandaloneFootprint::new("Demo_Standalone", "F.Cu");
    let footprint = &mut standalone.footprint;
    footprint.description = Some("Fresh typed footprint".to_owned());
    footprint.solder_mask_margin_mm = Some(0.02);
    footprint.solder_paste_margin_mm = Some(-0.01);
    footprint.solder_paste_margin_ratio = Some(-0.05);
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
    footprint.pads.push(AuthoredPad {
        number: "1".to_owned(),
        kind: AuthoredPadKind::Smd,
        shape: AuthoredPadShape::CustomPolygon {
            points: vec![point(-1.0, -0.5), point(1.0, -0.5), point(0.0, 1.0)],
        },
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
    });
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

#[test]
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one first-spike semantic proof intentionally checks every required source family"
)]
fn fresh_typed_board_emits_and_reads_required_first_spike_semantics() {
    let document = authored_board()
        .to_document(PcbAuthoringLimits::default())
        .expect("fresh board document");
    publish_for_independent_oracle("native-authored.kicad_pcb", document.source());
    let view = document.view().expect("fresh board view");
    assert_eq!(view.layers().count(), 11);
    assert_eq!(
        view.nets().next().expect("net").expect("net decode").name,
        "GND"
    );
    let setup = view.setup().expect("setup decode").expect("setup");
    assert_eq!(setup.aux_axis_origin.x, 5.0);
    assert_eq!(setup.grid_origin.y, 2.0);
    let stackup = setup.stackup.expect("stackup");
    assert_eq!(stackup.layers.len(), 3);
    assert_eq!(stackup.layers[1].material, "FR4");
    assert_eq!(stackup.layers[1].thickness, 1.53);
    let profiles = view
        .profile_primitives()
        .collect::<Result<Vec<_>, _>>()
        .expect("profile primitives");
    let expected_profile = [
        ((0.0, 0.0), (50.0, 0.0)),
        ((50.0, 0.0), (50.0, 40.0)),
        ((50.0, 40.0), (0.0, 40.0)),
        ((0.0, 40.0), (0.0, 0.0)),
        ((20.0, 15.0), (30.0, 15.0)),
        ((30.0, 15.0), (30.0, 25.0)),
        ((30.0, 25.0), (20.0, 25.0)),
        ((20.0, 25.0), (20.0, 15.0)),
    ];
    assert_eq!(profiles.len(), expected_profile.len());
    for (index, (profile, (expected_start, expected_end))) in
        profiles.iter().zip(expected_profile).enumerate()
    {
        assert_eq!(profile.owner, PcbProfileOwner::Board);
        let start = profile.graphic.start.expect("profile start");
        let end = profile.graphic.end.expect("profile end");
        assert_eq!((start.x, start.y), expected_start);
        assert_eq!((end.x, end.y), expected_end);
        assert_eq!(profile.graphic.layer.as_deref(), Some("Edge.Cuts"));
        assert_eq!(profile.graphic.stroke_width, Some(0.05));
        assert_eq!(
            profile.graphic.uuid.as_deref(),
            Some(uuid(10 + index as u64).as_str())
        );
    }

    let footprints = view
        .footprints()
        .collect::<Result<Vec<_>, _>>()
        .expect("footprints");
    assert_eq!(footprints.len(), 2);
    assert_eq!(
        (footprints[0].at_x, footprints[0].at_y),
        (Some(10.0), Some(20.0))
    );
    assert_eq!(footprints[1].layer.as_deref(), Some("B.Cu"));
    assert_eq!(footprints[1].angle, Some(90.0));
    assert_ne!(footprints[0].uuid, footprints[1].uuid);
    assert_eq!(footprints[0].reference.as_deref(), Some("U1"));
    assert_eq!(footprints[1].reference.as_deref(), Some("U1"));
    assert_eq!(footprints[0].solder_mask_margin, Some(0.03));
    assert_eq!(footprints[0].solder_paste_margin, Some(-0.01));
    assert_eq!(footprints[0].solder_paste_margin_ratio, Some(-0.05));
    assert_eq!(footprints[1].solder_mask_margin, None);
    let metadata = view.metadata().expect("board metadata");
    assert_eq!(metadata.pad_to_mask_clearance, 0.01);
    assert_eq!(metadata.pad_to_paste_clearance, -0.005);
    assert_eq!(metadata.pad_to_paste_clearance_ratio, -0.02);
    let pads = view.pads().collect::<Result<Vec<_>, _>>().expect("pads");
    assert_eq!(pads.len(), 6);
    assert_eq!(pads[0].net.name.as_deref(), Some("GND"));
    assert_eq!(
        pads[0].owner,
        PcbFootprintMemberOwner::EmbeddedFootprint { footprint_index: 0 }
    );
    assert_eq!(pads[0].layers, ["*.Cu", "*.Mask"]);
    assert_eq!(pads[0].solder_mask_margin, Some(0.05));
    assert_eq!(pads[0].remove_unused_layers, Some(true));
    assert_eq!(pads[0].keep_end_layers, Some(true));
    assert_eq!(pads[1].kind, "np_thru_hole");
    assert_eq!(pads[1].drill.as_ref().expect("slot").height, Some(1.6));
    assert_eq!(
        (pads[1].at_x, pads[1].at_y, pads[1].angle),
        (3.0, 0.0, 90.0)
    );
    assert_eq!(pads[1].drill.as_ref().expect("slot").offset.x, 0.1);
    assert_eq!(pads[1].drill.as_ref().expect("slot").offset.y, -0.1);
    assert_eq!(
        pads[3].owner,
        PcbFootprintMemberOwner::EmbeddedFootprint { footprint_index: 1 }
    );
    assert_eq!((pads[3].at_x, pads[3].at_y), (0.0, 0.0));
    assert_eq!(pads[3].solder_mask_margin, None);
    assert_ne!(pads[0].uuid, pads[3].uuid);
    assert_eq!(pads[2].number, "2");
    assert_eq!(pads[2].solder_mask_margin, Some(0.05));
    assert_eq!(pads[2].solder_paste_margin, Some(-0.02));
    assert_eq!(pads[2].solder_paste_margin_ratio, Some(-0.1));
    assert_eq!(pads[5].number, "2");
    assert_eq!(pads[5].solder_mask_margin, None);
    assert_eq!(pads[5].solder_paste_margin, None);
    assert_eq!(pads[5].solder_paste_margin_ratio, None);
    assert_eq!(
        (
            effective_surface_policy(
                pads[2].solder_mask_margin,
                footprints[0].solder_mask_margin,
                metadata.pad_to_mask_clearance,
            ),
            effective_surface_policy(
                pads[2].solder_paste_margin,
                footprints[0].solder_paste_margin,
                metadata.pad_to_paste_clearance,
            ),
            effective_surface_policy(
                pads[2].solder_paste_margin_ratio,
                footprints[0].solder_paste_margin_ratio,
                metadata.pad_to_paste_clearance_ratio,
            ),
        ),
        (0.05, -0.02, -0.1)
    );
    assert_eq!(
        (
            effective_surface_policy(
                pads[1].solder_mask_margin,
                footprints[0].solder_mask_margin,
                metadata.pad_to_mask_clearance,
            ),
            effective_surface_policy(
                pads[1].solder_paste_margin,
                footprints[0].solder_paste_margin,
                metadata.pad_to_paste_clearance,
            ),
            effective_surface_policy(
                pads[1].solder_paste_margin_ratio,
                footprints[0].solder_paste_margin_ratio,
                metadata.pad_to_paste_clearance_ratio,
            ),
        ),
        (0.03, -0.01, -0.05)
    );
    assert_eq!(
        (
            effective_surface_policy(
                pads[5].solder_mask_margin,
                footprints[1].solder_mask_margin,
                metadata.pad_to_mask_clearance,
            ),
            effective_surface_policy(
                pads[5].solder_paste_margin,
                footprints[1].solder_paste_margin,
                metadata.pad_to_paste_clearance,
            ),
            effective_surface_policy(
                pads[5].solder_paste_margin_ratio,
                footprints[1].solder_paste_margin_ratio,
                metadata.pad_to_paste_clearance_ratio,
            ),
        ),
        (0.01, -0.005, -0.02)
    );
    let properties = view
        .footprint_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("footprint properties");
    assert_eq!(properties[1].footprint_index, 1);
    assert_eq!((properties[1].at.x, properties[1].at.y), (0.0, -2.0));
    assert_eq!(properties[1].layer, "B.SilkS");
    assert_eq!(properties[1].effects.font.size_x, 1.25);
    assert_eq!(properties[1].effects.font.size_y, 0.75);
    let graphics = view
        .footprint_graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("footprint graphics");
    assert_eq!(graphics[1].footprint_index, 1);
    assert_eq!(graphics[1].graphic.layer.as_deref(), Some("B.SilkS"));
    assert_eq!(
        (
            graphics[1].graphic.start.expect("bottom graphic start").x,
            graphics[1].graphic.end.expect("bottom graphic end").x,
        ),
        (-1.0, 1.0)
    );
    let model = view.models().next().expect("model").expect("model decode");
    assert_eq!(model.path, "kicad-embed://footprint.step");
    assert_eq!(model.offset, [0.5, 1.0, 1.5]);
    assert_eq!(model.scale, [1.0, 2.0, 1.0]);
    assert_eq!(model.rotate, [0.0, 45.0, 90.0]);
    let resources = view
        .all_embedded_files()
        .collect::<Result<Vec<_>, _>>()
        .expect("authored board resources");
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].name, "board.step");
    assert_eq!(resources[0].owner, EmbeddedFileOwner::Board);
    assert_eq!(
        view.decode_embedded_file(&resources[0], EmbeddedDecodeLimits::default())
            .expect("board resource decode")
            .expect("board resource bytes"),
        b"board-owned shared model"
    );
    assert_eq!(
        resources[1].owner,
        EmbeddedFileOwner::EmbeddedFootprint { footprint_index: 0 }
    );
    assert_eq!(resources[1].name, "footprint.step");
    assert_eq!(
        view.decode_embedded_file(&resources[1], EmbeddedDecodeLimits::default())
            .expect("footprint resource decode")
            .expect("footprint resource bytes"),
        b"footprint-owned shared model"
    );

    let segments = view
        .segments()
        .collect::<Result<Vec<_>, _>>()
        .expect("segments");
    assert_eq!(segments.len(), 1);
    assert_eq!(
        (
            segments[0].start_x,
            segments[0].start_y,
            segments[0].end_x,
            segments[0].end_y,
        ),
        (10.0, 20.0, 20.0, 20.0)
    );
    assert_eq!(segments[0].width, Some(0.25));
    assert_eq!(segments[0].layer.as_deref(), Some("F.Cu"));
    assert_eq!(segments[0].net.ordinal, Some(1));
    assert_eq!(segments[0].net.name.as_deref(), Some("GND"));
    assert_eq!(segments[0].uuid.as_deref(), Some(uuid(300).as_str()));
    let arcs = view.arcs().collect::<Result<Vec<_>, _>>().expect("arcs");
    assert_eq!(arcs.len(), 1);
    assert_eq!((arcs[0].start.x, arcs[0].start.y), (20.0, 20.0));
    assert_eq!((arcs[0].mid.x, arcs[0].mid.y), (25.0, 25.0));
    assert_eq!((arcs[0].end.x, arcs[0].end.y), (30.0, 20.0));
    assert_eq!(arcs[0].width, Some(0.25));
    assert_eq!(arcs[0].layer.as_deref(), Some("F.Cu"));
    assert_eq!(arcs[0].net.ordinal, Some(1));
    assert_eq!(arcs[0].net.name.as_deref(), Some("GND"));
    assert_eq!(arcs[0].uuid.as_deref(), Some(uuid(301).as_str()));

    let mut bytes = Vec::new();
    document.write_to(&mut bytes).expect("board write");
    assert_eq!(bytes, document.source().as_bytes());
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "the standalone proof follows one source owner through all typed member associations"
)]
fn fresh_typed_standalone_footprint_round_trips_members_model_and_resource_bytes() {
    let document = standalone_footprint()
        .to_document(PcbAuthoringLimits::default())
        .expect("fresh footprint document");
    publish_for_independent_oracle("Demo_Standalone.kicad_mod", document.source());
    let view = document.view().expect("footprint view");
    assert_eq!(view.name().expect("name"), "Demo_Standalone");
    let metadata = view.metadata().expect("standalone metadata");
    assert_eq!(metadata.solder_mask_margin, Some(0.02));
    assert_eq!(metadata.solder_paste_margin, Some(-0.01));
    assert_eq!(metadata.solder_paste_margin_ratio, Some(-0.05));
    let texts = view
        .texts()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone texts");
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].kind, "user");
    assert_eq!(texts[0].text, "ASSEMBLY");
    assert_eq!(texts[0].effects.font.size_x, 1.25);
    assert_eq!(texts[0].effects.font.size_y, 0.75);
    assert_eq!(view.graphics().count(), 1);
    let pad = view.pads().next().expect("pad").expect("pad decode");
    assert_eq!(pad.shape, "custom");
    assert_eq!(pad.custom_primitives[0].points.len(), 3);
    assert_eq!(pad.solder_mask_margin, Some(0.04));
    let model = view.models().next().expect("model").expect("model decode");
    assert_eq!(model.path, "kicad-embed://native.step");
    assert_eq!(model.offset, [1.0, 2.0, 3.0]);
    assert_eq!(model.rotate, [0.0, 0.0, 90.0]);
    let resource = view
        .embedded_files()
        .next()
        .expect("resource")
        .expect("resource metadata");
    assert_eq!(resource.name, "native.step");
    assert_eq!(
        model.path.strip_prefix("kicad-embed://"),
        Some(resource.name.as_str())
    );
    assert_eq!(
        view.decode_embedded_file(&resource, Default::default())
            .expect("resource decode")
            .expect("resource bytes"),
        b"tiny synthetic model bytes"
    );
}

#[test]
fn authored_writer_rejects_unsupported_loss_identity_and_resource_limits() {
    let board = authored_board();
    let text = board
        .canonical_text(PcbAuthoringLimits::default())
        .expect("baseline");
    let exact = PcbAuthoringLimits {
        max_output_bytes: text.len(),
        ..PcbAuthoringLimits::default()
    };
    assert_eq!(
        board.canonical_text(exact).expect("exact output").len(),
        text.len()
    );
    assert_eq!(
        board
            .canonical_text(PcbAuthoringLimits {
                max_output_bytes: text.len() - 1,
                ..exact
            })
            .expect_err("one-under output")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut unsupported = board.clone();
    unsupported.version = 20_260_101;
    let error = unsupported
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unsupported version");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("supported"));

    let mut duplicate = board.clone();
    duplicate.footprints[1].uuid = duplicate.footprints[0].uuid.clone();
    let error = duplicate
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("duplicate source identity");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("duplicate KiCad UUID"));

    let mut unknown_net = board;
    unknown_net.segments[0].net_code = 99;
    let error = unknown_net
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unknown net");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("unknown authored net"));

    let error = standalone_footprint()
        .canonical_text(PcbAuthoringLimits {
            max_resource_encoded_bytes: 0,
            ..PcbAuthoringLimits::default()
        })
        .expect_err("encoded resource limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn board_resource_namespace_and_embedded_model_links_are_loss_aware() {
    let board = authored_board();
    let mut shadowed_payload = board.clone();
    shadowed_payload.footprints[0].footprint.embedded_files[0].name = "board.step".to_owned();
    shadowed_payload.footprints[0].footprint.models[0].path = "kicad-embed://board.step".to_owned();
    let error = shadowed_payload
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("footprint payload shadowed by board resource");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("conflicting payloads"));
    assert_eq!(
        shadowed_payload
            .canonical_text(PcbAuthoringLimits {
                max_objects: 0,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("bounded traversal precedes namespace allocation")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut board_owned_reference = board.clone();
    board_owned_reference.footprints[0].footprint.embedded_files[0].name = "board.step".to_owned();
    board_owned_reference.footprints[0].footprint.embedded_files[0].data =
        AuthoredResourceData::DeclarationOnly;
    board_owned_reference.footprints[0].footprint.models[0].path =
        "kicad-embed://board.step".to_owned();
    board_owned_reference
        .canonical_text(PcbAuthoringLimits::default())
        .expect("footprint declaration references board-owned payload");

    let mut direct_board_reference = board.clone();
    direct_board_reference.footprints[0]
        .footprint
        .embedded_files
        .clear();
    direct_board_reference.footprints[0].footprint.models[0].path =
        "kicad-embed://board.step".to_owned();
    direct_board_reference
        .canonical_text(PcbAuthoringLimits::default())
        .expect("occurrence model directly references board declaration");

    let mut incompatible_reference = board_owned_reference;
    incompatible_reference.footprints[0]
        .footprint
        .embedded_files[0]
        .file_type = "font".to_owned();
    let error = incompatible_reference
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("incompatible resource declaration type");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("conflicting types"));

    let mut conflicting_occurrence_payloads = board.clone();
    conflicting_occurrence_payloads.footprints[1]
        .footprint
        .embedded_files
        .push(AuthoredEmbeddedFile {
            name: "footprint.step".to_owned(),
            file_type: "model".to_owned(),
            data: AuthoredResourceData::Bytes(b"different payload".to_vec()),
        });
    let error = conflicting_occurrence_payloads
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("conflicting occurrence payloads");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("conflicting payloads"));

    let mut repeated_identical_payload = board.clone();
    repeated_identical_payload.footprints[1]
        .footprint
        .embedded_files
        .push(board.footprints[0].footprint.embedded_files[0].clone());
    repeated_identical_payload
        .canonical_text(PcbAuthoringLimits::default())
        .expect("identical occurrence payloads can be hoisted once");

    let mut missing_local = board.clone();
    missing_local.footprints[0].footprint.embedded_files.clear();
    let error = missing_local
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("missing local embedded model declaration");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert_eq!(
        missing_local
            .canonical_text(PcbAuthoringLimits {
                max_resource_input_bytes: 0,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("resource traversal is bounded before model lookup")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut empty_name = board;
    empty_name.footprints[0].footprint.models[0].path = "kicad-embed://".to_owned();
    let error = empty_name
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("empty embedded model resource name");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
}

#[test]
fn standalone_embedded_models_require_local_model_declarations() {
    let mut missing = standalone_footprint();
    missing.footprint.embedded_files.clear();
    assert_eq!(
        missing
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("missing standalone embedded model resource")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut deferred_missing = standalone_footprint();
    deferred_missing.footprint.models[0].path = "kicad-embed://missing.step".to_owned();
    assert_eq!(
        deferred_missing
            .canonical_text(PcbAuthoringLimits {
                max_resource_input_bytes: 0,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("standalone resource traversal is bounded before model lookup")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut wrong_type = standalone_footprint();
    wrong_type.footprint.embedded_files[0].file_type = "other".to_owned();
    assert_eq!(
        wrong_type
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("wrong standalone embedded model resource type")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut declaration_only = standalone_footprint();
    declaration_only.footprint.embedded_files[0].data = AuthoredResourceData::DeclarationOnly;
    declaration_only
        .canonical_text(PcbAuthoringLimits::default())
        .expect("host-supplied standalone embedded model declaration");
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one boundary matrix keeps every public authoring limit and loss rejection together"
)]
fn authored_validation_limits_and_loss_boundaries_are_explicit() {
    let minimal = AuthoredStandaloneFootprint::new("A", "F.Cu");
    let string_bytes = minimal.generator.len()
        + minimal.generator_version.len()
        + minimal.layer.len()
        + minimal.footprint.name.len();
    minimal
        .canonical_text(PcbAuthoringLimits {
            max_objects: 1,
            max_string_bytes: string_bytes,
            ..PcbAuthoringLimits::default()
        })
        .expect("exact object and string limits");
    for limits in [
        PcbAuthoringLimits {
            max_objects: 0,
            ..PcbAuthoringLimits::default()
        },
        PcbAuthoringLimits {
            max_string_bytes: string_bytes - 1,
            ..PcbAuthoringLimits::default()
        },
    ] {
        assert_eq!(
            minimal
                .canonical_text(limits)
                .expect_err("one-under authoring limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let mut with_points = minimal.clone();
    with_points.footprint.graphics.push(AuthoredGraphic {
        geometry: AuthoredGraphicGeometry::Line {
            start: point(0.0, 0.0),
            end: point(1.0, 0.0),
        },
        layer: "F.SilkS".to_owned(),
        stroke_width_mm: 0.1,
        stroke_kind: "solid".to_owned(),
        fill: None,
        uuid: uuid(900),
    });
    with_points
        .canonical_text(PcbAuthoringLimits {
            max_points: 2,
            ..PcbAuthoringLimits::default()
        })
        .expect("exact point limit");
    assert_eq!(
        with_points
            .canonical_text(PcbAuthoringLimits {
                max_points: 1,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("one-under point limit")
            .kind,
        ErrorKind::ResourceLimit
    );

    let resource = standalone_footprint();
    let input_bytes = match &resource.footprint.embedded_files[0].data {
        AuthoredResourceData::Bytes(bytes) => bytes.len(),
        _ => panic!("fixture resource bytes"),
    };
    let compressed_bound = zstd::zstd_safe::compress_bound(input_bytes);
    let work_bytes = compressed_bound + compressed_bound.div_ceil(3) * 4;
    resource
        .canonical_text(PcbAuthoringLimits {
            max_resource_input_bytes: input_bytes,
            max_resource_work_bytes: work_bytes,
            ..PcbAuthoringLimits::default()
        })
        .expect("exact resource input and work limits");
    for limits in [
        PcbAuthoringLimits {
            max_resource_input_bytes: input_bytes - 1,
            ..PcbAuthoringLimits::default()
        },
        PcbAuthoringLimits {
            max_resource_work_bytes: work_bytes - 1,
            ..PcbAuthoringLimits::default()
        },
    ] {
        assert_eq!(
            resource
                .canonical_text(limits)
                .expect_err("bounded resource work")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let mut locked_without_thickness = authored_board();
    let stackup = locked_without_thickness
        .setup
        .stackup
        .as_mut()
        .expect("stackup");
    stackup.layers[0].thickness_mm = None;
    stackup.layers[0].thickness_locked = true;
    assert_eq!(
        locked_without_thickness
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("lock without thickness")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut invalid_board_surface_policy = authored_board();
    invalid_board_surface_policy
        .setup
        .pad_to_paste_clearance_ratio = f64::NAN;
    assert_eq!(
        invalid_board_surface_policy
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("nonfinite board surface policy")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut invalid_footprint_surface_policy = standalone_footprint();
    invalid_footprint_surface_policy
        .footprint
        .solder_mask_margin_mm = Some(f64::INFINITY);
    assert_eq!(
        invalid_footprint_surface_policy
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("nonfinite footprint surface policy")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut unknown_layer = authored_board();
    unknown_layer.segments[0].layer = "No.Such.Layer".to_owned();
    let error = unknown_layer
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unknown board layer");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("absent from the board layer table"));

    let mut unsupported_token = minimal.clone();
    unsupported_token.footprint.graphics = with_points.footprint.graphics;
    unsupported_token.footprint.graphics[0].stroke_kind = "wavy".to_owned();
    assert_eq!(
        unsupported_token
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("unsupported source token")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut standalone_net = standalone_footprint();
    standalone_net.footprint.pads[0].net = Some(AuthoredNetRef {
        code: 1,
        name: "GND".to_owned(),
    });
    let error = standalone_net
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("standalone pad net would be discarded by KiCad");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(
        error
            .message
            .contains("cannot author board net associations")
    );

    let mut npth_net = authored_board();
    npth_net.footprints[0].footprint.pads[1].net = Some(AuthoredNetRef {
        code: 1,
        name: "GND".to_owned(),
    });
    let error = npth_net
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("KiCad discards NPTH net associations");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("non-plated through-hole"));

    let mut smd_inner_land_policy = standalone_footprint();
    smd_inner_land_policy.footprint.pads[0].remove_unused_layers = Some(true);
    let error = smd_inner_land_policy
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("KiCad discards SMD inner-land policy");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("plated through-hole pads"));

    let mut filled_line = authored_board();
    filled_line.profile[0].fill = Some("solid".to_owned());
    let error = filled_line
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("KiCad discards line fill");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("line and arc graphics"));

    let mut duplicate_property = standalone_footprint();
    duplicate_property.footprint.properties[1].name = "Reference".to_owned();
    let error = duplicate_property
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("KiCad keeps only one property with a given name");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("duplicate footprint property name"));

    let mut unknown_stackup_copper = authored_board();
    unknown_stackup_copper
        .setup
        .stackup
        .as_mut()
        .expect("stackup")
        .layers[0]
        .name = "X.Cu".to_owned();
    let error = unknown_stackup_copper
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("KiCad reinterprets undeclared stackup copper");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("copper stackup layer"));

    let mut invalid_paper = authored_board();
    invalid_paper.paper = "FOO".to_owned();
    let error = invalid_paper
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unsupported paper token");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("unsupported paper size"));

    let mut invalid_edge_connector = authored_board();
    invalid_edge_connector
        .setup
        .stackup
        .as_mut()
        .expect("stackup")
        .edge_connector = Some("none".to_owned());
    let error = invalid_edge_connector
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("absence represents no edge connector");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("stackup edge connector"));

    let mut invalid_layer_kind = authored_board();
    invalid_layer_kind.layers[0].kind = "user".to_owned();
    let error = invalid_layer_kind
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("copper layer cannot be user kind");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("requires a copper kind"));

    let mut invalid_standalone_root_layer = standalone_footprint();
    invalid_standalone_root_layer.layer = "No.Such.Layer".to_owned();
    let error = invalid_standalone_root_layer
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unknown standalone root layer");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(
        error
            .message
            .contains("unsupported standalone footprint layer")
    );

    let mut noncopper_standalone_root = standalone_footprint();
    noncopper_standalone_root.layer = "F.SilkS".to_owned();
    let error = noncopper_standalone_root
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("footprint roots require a copper side");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("root layer must be F.Cu or B.Cu"));

    let invalid_standalone_name = AuthoredStandaloneFootprint::new("Library:Name", "F.Cu");
    let error = invalid_standalone_name
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("standalone roots cannot carry library links");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("library-link colon"));

    let mut invalid_standalone_member_layer = standalone_footprint();
    invalid_standalone_member_layer.footprint.graphics[0].layer = "No.Such.Layer".to_owned();
    let error = invalid_standalone_member_layer
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unknown standalone member layer");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(
        error
            .message
            .contains("unsupported standalone footprint layer")
    );
}

#[test]
fn owned_footprint_document_io_obeys_exact_limits_and_reports_errors() {
    let source = standalone_footprint()
        .canonical_text(PcbAuthoringLimits::default())
        .expect("standalone source");
    let exact = FootprintLimits {
        max_source_bytes: source.len(),
        max_output_bytes: source.len(),
        ..FootprintLimits::default()
    };
    let document = FootprintDocument::from_reader(Cursor::new(source.as_bytes()), exact)
        .expect("exact footprint read");
    let mut output = Vec::new();
    document
        .write_to(&mut output)
        .expect("exact footprint write");
    assert_eq!(output, source.as_bytes());

    assert_eq!(
        FootprintDocument::from_reader(
            Cursor::new(source.as_bytes()),
            FootprintLimits {
                max_source_bytes: source.len() - 1,
                ..FootprintLimits::default()
            },
        )
        .expect_err("source ceiling")
        .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(
        FootprintDocument::from_reader(
            Cursor::new([b'(', 0xff, b')']),
            FootprintLimits::default(),
        )
        .expect_err("invalid UTF-8")
        .kind,
        ErrorKind::InvalidUtf8
    );

    let strict = FootprintDocument::parse(
        source,
        FootprintLimits {
            max_output_bytes: output.len() - 1,
            ..FootprintLimits::default()
        },
    )
    .expect("parse with strict output limit");
    let mut untouched = Vec::new();
    assert_eq!(
        strict
            .write_to(&mut untouched)
            .expect_err("output ceiling")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert!(untouched.is_empty());
    assert_eq!(
        FootprintDocument::from_reader(FailingReader, FootprintLimits::default())
            .expect_err("read error")
            .kind,
        ErrorKind::Io
    );
    assert_eq!(
        document
            .write_to(FailingWriter)
            .expect_err("write error")
            .kind,
        ErrorKind::Io
    );
}

struct FailingReader;

impl std::io::Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("read failure"))
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("write failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

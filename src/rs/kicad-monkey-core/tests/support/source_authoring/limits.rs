use super::*;

pub(super) fn assert_metadata_loss_rejections() {
    assert_group_loss_rejections();
    let mut board = authored_board();
    board.properties.push(board.properties[0].clone());
    assert_eq!(
        board.canonical_text(Default::default()).unwrap_err().kind,
        ErrorKind::InvalidBuildValue
    );
    for (function, kind, length) in [
        (Some("pin"), None, None),
        (None, Some("passive"), None),
        (None, None, Some(0.0)),
        (None, None, Some(f64::NAN)),
    ] {
        let mut footprint = standalone_footprint();
        let pad = &mut footprint.footprint.pads[0];
        pad.pin_function = function.map(str::to_owned);
        pad.pin_type = kind.map(str::to_owned);
        pad.die_length_mm = length;
        assert_eq!(
            footprint
                .canonical_text(Default::default())
                .unwrap_err()
                .kind,
            ErrorKind::InvalidBuildValue
        );
    }
    let mut board = authored_board();
    board.footprints[0].footprint.pads[0].pin_function = Some(String::new());
    assert_eq!(
        board.canonical_text(Default::default()).unwrap_err().kind,
        ErrorKind::InvalidBuildValue
    );
}

fn assert_group_loss_rejections() {
    for members in [
        vec![],
        vec![uuid(999)],
        vec![uuid(102)],
        vec![uuid(801)],
        vec![uuid(300), uuid(300)],
        vec![uuid(100)],
    ] {
        let mut board = authored_board();
        board.groups[0].members = members;
        assert_eq!(
            board.canonical_text(Default::default()).unwrap_err().kind,
            ErrorKind::InvalidBuildValue
        );
    }
    let mut board = authored_board();
    board.groups[0].uuid = uuid(300);
    assert!(board.canonical_text(Default::default()).is_err());
    let mut board = authored_board();
    board.groups[0].members[0] = uuid(300).to_ascii_uppercase();
    assert!(board.canonical_text(Default::default()).is_ok());
    let limits = PcbAuthoringLimits {
        pcb_limits: kicad_monkey_core::PcbLimits {
            max_members: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        board.canonical_text(limits).unwrap_err().kind,
        ErrorKind::ResourceLimit
    );
}
pub(super) fn assert_custom_compatibility_and_rejections() {
    let points = vec![point(-1.0, -0.5), point(1.0, -0.5), point(0.0, 1.0)];
    let mut legacy = standalone_footprint();
    legacy.footprint.pads[0].shape = AuthoredPadShape::CustomPolygon {
        points: points.clone(),
    };
    let mut explicit = legacy.clone();
    explicit.footprint.pads[0].shape = AuthoredPadShape::Custom {
        anchor: Some(AuthoredPadAnchor::Rect),
        clearance: Some(AuthoredCustomPadClearance::Outline),
        primitives: vec![AuthoredPadPrimitive {
            geometry: AuthoredPadPrimitiveGeometry::Polygon {
                points: points
                    .into_iter()
                    .map(AuthoredPadPolygonPoint::Xy)
                    .collect(),
            },
            width_mm: 0.0,
            fill: Some(AuthoredPadPrimitiveFill::Solid),
        }],
    };
    assert_eq!(
        legacy.canonical_text(Default::default()).unwrap(),
        explicit.canonical_text(Default::default()).unwrap()
    );
    for (index, width, fill) in [
        (1, 0.1, Some(AuthoredPadPrimitiveFill::Solid)),
        (4, 0.0, Some(AuthoredPadPrimitiveFill::Unfilled)),
        (0, -0.1, Some(AuthoredPadPrimitiveFill::Solid)),
        (0, f64::NAN, Some(AuthoredPadPrimitiveFill::Solid)),
    ] {
        let mut source = standalone_footprint();
        let AuthoredPadShape::Custom { primitives, .. } = &mut source.footprint.pads[0].shape
        else {
            unreachable!()
        };
        primitives[index].width_mm = width;
        primitives[index].fill = fill;
        assert_eq!(
            source.canonical_text(Default::default()).unwrap_err().kind,
            ErrorKind::InvalidBuildValue
        );
    }
}
pub(super) fn assert_object_string_and_point_limits(
    minimal: &AuthoredStandaloneFootprint,
) -> AuthoredStandaloneFootprint {
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
        net: None,
        locked: false,
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

    with_points
}

pub(super) fn assert_resource_work_limits() {
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
}

pub(super) fn assert_setup_policy_losses() {
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
}

pub(super) fn assert_member_policy_losses(
    minimal: &AuthoredStandaloneFootprint,
    with_points: AuthoredStandaloneFootprint,
) {
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
    assert!(error.message.contains("line, arc and curve graphics"));

    let mut duplicate_property = standalone_footprint();
    duplicate_property.footprint.properties[1].name = "Reference".to_owned();
    let error = duplicate_property
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("KiCad keeps only one property with a given name");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("duplicate footprint property name"));
}

pub(super) fn assert_document_scope_losses() {
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

    assert_standalone_scope_losses();
}

fn assert_standalone_scope_losses() {
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

pub(super) fn assert_occurrence_metadata_losses(board: &AuthoredPcb) {
    for path in ["", "not-a-uuid", "/not-a-uuid", "/"] {
        let mut invalid = board.clone();
        invalid.footprints[0].placement_path = Some(path.to_owned());
        assert_eq!(
            invalid
                .canonical_text(PcbAuthoringLimits::default())
                .expect_err("invalid or discarded placement path")
                .kind,
            ErrorKind::InvalidBuildValue
        );
    }
    let mut empty_sheet = board.clone();
    empty_sheet.footprints[0].placement_sheet_name = Some(String::new());
    assert!(
        empty_sheet
            .canonical_text(PcbAuthoringLimits::default())
            .is_err()
    );
    let mut nonfinite_clearance = board.clone();
    nonfinite_clearance.footprints[0].footprint.clearance_mm = Some(f64::NAN);
    assert!(
        nonfinite_clearance
            .canonical_text(PcbAuthoringLimits::default())
            .is_err()
    );
}

pub(super) fn assert_case_insensitive_identity(board: &AuthoredPcb) {
    let mut mixed_case = board.clone();
    let uppercase_uuid = "ABCDEF01-2345-6789-ABCD-EF0123456789";
    mixed_case.footprints[0].uuid = uppercase_uuid.to_owned();
    mixed_case.groups[1].members[1] = uppercase_uuid.to_owned();
    assert!(
        mixed_case
            .canonical_text(PcbAuthoringLimits::default())
            .expect("unique uppercase identity retains its authored spelling")
            .contains(uppercase_uuid)
    );
    mixed_case.footprints[1].uuid = uppercase_uuid.to_ascii_lowercase();
    let error = mixed_case
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("case variants are the same occurrence identity");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("duplicate KiCad UUID"));
    mixed_case.footprints[1].uuid = board.footprints[1].uuid.clone();
    mixed_case.footprints[1].footprint.pads[0].uuid = uppercase_uuid.to_ascii_lowercase();
    let error = mixed_case
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("member and occurrence identities share the same UUID domain");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("duplicate KiCad UUID"));
}

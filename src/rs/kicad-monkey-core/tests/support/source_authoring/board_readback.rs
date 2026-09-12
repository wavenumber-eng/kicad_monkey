use super::*;

pub(super) fn assert_board_metadata_and_profile(view: &kicad_monkey_core::PcbView<'_>) {
    let properties = view.properties().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        properties
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str()))
            .collect::<Vec<_>>(),
        vec![("REVISION", "A \"prototype\""), ("EMPTY", "")]
    );
    let metadata_pad = view
        .pads()
        .map(Result::unwrap)
        .find(|pad| pad.uuid.as_deref() == Some(uuid(102).as_str()))
        .unwrap();
    assert_eq!(
        (
            metadata_pad.pin_function.as_deref(),
            metadata_pad.pin_type.as_deref(),
            metadata_pad.die_length
        ),
        (Some("VDD"), Some("power_in"), Some(0.75))
    );
    assert_eq!(view.layers().count(), 11);
    assert_eq!(
        view.nets().next().expect("net").expect("net decode").name,
        "GND"
    );
    let setup = view.setup().expect("setup decode").expect("setup");
    assert_eq!(setup.aux_axis_origin.x, 5.0);
    assert_eq!(setup.grid_origin.y, 2.0);
    assert!(setup.tenting_front);
    assert!(!setup.tenting_back);
    let stackup = setup.stackup.expect("stackup");
    assert_eq!(stackup.layers.len(), 3);
    assert_eq!(stackup.layers[1].material, "FR4");
    assert_eq!(stackup.layers[1].thickness, 1.53);
    assert_board_profile(view);
}

fn assert_board_profile(view: &kicad_monkey_core::PcbView<'_>) {
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
}

pub(super) fn assert_board_occurrences(view: &kicad_monkey_core::PcbView<'_>) {
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
    assert!(footprints[0].locked);
    assert_eq!(
        footprints[0].placement_path,
        Some(format!("/{}/{}", uuid(700), uuid(701)))
    );
    assert_eq!(footprints[0].placement_sheet_name.as_deref(), Some("Power"));
    assert_eq!(
        footprints[0].placement_sheet_file.as_deref(),
        Some("power.kicad_sch")
    );
    assert_occurrence_policies(&footprints);
}

fn assert_occurrence_policies(footprints: &[kicad_monkey_core::PcbFootprint]) {
    assert_eq!(footprints[0].clearance, Some(0.0));
    assert_eq!(footprints[0].zone_connect, Some(2));
    assert!(!footprints[1].locked);
    assert_eq!(footprints[1].placement_path, None);
    assert_eq!(footprints[1].placement_sheet_name, None);
    assert_eq!(footprints[1].placement_sheet_file, None);
    assert_eq!(footprints[1].clearance, None);
    assert_eq!(footprints[1].zone_connect, None);
    assert_eq!(footprints[0].reference.as_deref(), Some("U1"));
    assert_eq!(footprints[1].reference.as_deref(), Some("U1"));
    assert_eq!(footprints[0].solder_mask_margin, Some(0.03));
    assert_eq!(footprints[0].solder_paste_margin, Some(-0.01));
    assert_eq!(footprints[0].solder_paste_margin_ratio, Some(-0.05));
    assert_eq!(footprints[1].solder_mask_margin, None);
}

pub(super) fn assert_board_local_artwork(view: &kicad_monkey_core::PcbView<'_>) {
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
}

pub(super) fn assert_board_resources(view: &kicad_monkey_core::PcbView<'_>) {
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
}

pub(super) fn assert_board_routes(view: &kicad_monkey_core::PcbView<'_>) {
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
    assert_board_arcs(view);
}

fn assert_board_arcs(view: &kicad_monkey_core::PcbView<'_>) {
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
}

pub(super) fn assert_board_pad_members(pads: &[kicad_monkey_core::PcbPad]) {
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
    assert_repeated_pad_policies(pads);
}

fn assert_repeated_pad_policies(pads: &[kicad_monkey_core::PcbPad]) {
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
}

pub(super) fn assert_board_surface_policy(view: &kicad_monkey_core::PcbView<'_>) {
    let metadata = view.metadata().expect("board metadata");
    assert_eq!(metadata.pad_to_mask_clearance, 0.01);
    assert_eq!(metadata.pad_to_paste_clearance, -0.005);
    assert_eq!(metadata.pad_to_paste_clearance_ratio, -0.02);
    let pads = view.pads().collect::<Result<Vec<_>, _>>().expect("pads");
    assert_board_pad_members(&pads);
    let footprints = view
        .footprints()
        .collect::<Result<Vec<_>, _>>()
        .expect("footprints");
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
}

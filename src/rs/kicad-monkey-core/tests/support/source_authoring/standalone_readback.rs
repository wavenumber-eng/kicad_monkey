use super::*;

pub(super) fn assert_metadata_and_artwork(view: &kicad_monkey_core::FootprintView<'_>) {
    assert_eq!(view.name().expect("name"), "Demo_Standalone");
    let metadata = view.metadata().expect("standalone metadata");
    assert_eq!(metadata.attributes, ["dnp"]);
    assert_eq!(metadata.solder_mask_margin, Some(0.02));
    assert_eq!(metadata.solder_paste_margin, Some(-0.01));
    assert_eq!(metadata.solder_paste_margin_ratio, Some(-0.05));
    assert_eq!(metadata.clearance, Some(0.12));
    assert_eq!(metadata.zone_connect, Some(0));
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
}

pub(super) fn assert_placed_custom_pad(pad: &kicad_monkey_core::PcbPad) {
    let mut board = authored_board();
    let mut occurrence = board_footprint(900, "U9", "B.Cu");
    occurrence.footprint = standalone_footprint().footprint;
    occurrence.at = point(13.0, 27.0);
    occurrence.angle_degrees = 90.0;
    board.footprints = vec![occurrence];
    board.groups.clear();
    board.layers.push(AuthoredLayer {
        ordinal: 49,
        name: "F.Fab".to_owned(),
        kind: "user".to_owned(),
        user_name: None,
    });
    let embedded = board
        .to_document(Default::default())
        .expect("placed composite");
    let embedded_pad = embedded.view().unwrap().pads().next().unwrap().unwrap();
    let embedded_options = embedded_pad.custom_options.as_ref().unwrap();
    let standalone_options = pad.custom_options.as_ref().unwrap();
    assert_eq!(
        (&embedded_options.anchor, &embedded_options.clearance),
        (&standalone_options.anchor, &standalone_options.clearance)
    );
    assert_eq!(
        embedded_pad
            .custom_primitives
            .iter()
            .map(|p| (&p.geometry, p.width, &p.fill))
            .collect::<Vec<_>>(),
        pad.custom_primitives
            .iter()
            .map(|p| (&p.geometry, p.width, &p.fill))
            .collect::<Vec<_>>()
    );
}

pub(super) fn assert_custom_pad(
    view: &kicad_monkey_core::FootprintView<'_>,
) -> kicad_monkey_core::PcbPad {
    let pad = view
        .pads()
        .map(Result::unwrap)
        .find(|pad| pad.uuid.as_deref() == Some(uuid(404).as_str()))
        .unwrap();
    assert_eq!(pad.die_length, Some(-0.25));
    assert_eq!(pad.shape, "custom");
    assert_eq!(pad.custom_primitives[0].points.len(), 3);
    assert_eq!(
        pad.custom_options.as_ref().unwrap().anchor.as_deref(),
        Some("circle")
    );
    assert_eq!(
        pad.custom_options.as_ref().unwrap().clearance.as_deref(),
        Some("convexhull")
    );
    assert_eq!(pad.custom_primitives.len(), 7);
    assert_eq!(pad.custom_primitives[0].width, Some(0.01));
    assert_curved_custom_primitives(&pad);
    assert_eq!(pad.solder_mask_margin, Some(0.04));
    pad
}

fn assert_curved_custom_primitives(pad: &kicad_monkey_core::PcbPad) {
    assert!(
        matches!(pad.custom_primitives[5].geometry, Some(PcbPadPrimitiveGeometry::Curve { points }) if points[1].x == -0.5 && points[2].y == 1.0)
    );
    assert!(
        matches!(&pad.custom_primitives[6].geometry, Some(PcbPadPrimitiveGeometry::Polygon { points }) if matches!(points[1], PcbPadPolygonPoint::Arc { mid, .. } if mid.y == 1.0))
    );
}

pub(super) fn assert_cut_model_and_resource(view: &kicad_monkey_core::FootprintView<'_>) {
    let cut = view.pads().nth(1).expect("NPTH").expect("NPTH decode");
    assert!(cut.has_layers && cut.layers.is_empty());
    assert_eq!(cut.plated, Some(false));
    assert_eq!(cut.drill.expect("NPTH drill").height, Some(1.6));
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

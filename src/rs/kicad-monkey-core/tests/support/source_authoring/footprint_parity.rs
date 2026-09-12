use super::*;

pub(super) fn assert_metadata(
    standalone: &FootprintView<'_>,
    board: &PcbView<'_>,
    standalone_source: &str,
) {
    let standalone_metadata = standalone.metadata().expect("standalone metadata");
    let board_metadata = board
        .footprints()
        .next()
        .expect("embedded footprint")
        .expect("embedded metadata");
    assert_eq!(
        standalone_metadata.library_link,
        board_metadata.library_link
    );
    assert_eq!(standalone_metadata.reference, board_metadata.reference);
    assert_eq!(standalone_metadata.value, board_metadata.value);
    assert_eq!(standalone_metadata.description, board_metadata.description);
    assert_eq!(standalone_metadata.tags, board_metadata.tags);
    assert_eq!(standalone_metadata.attributes, board_metadata.attributes);
    assert_metadata_details(&standalone_metadata, standalone_source);
}

fn assert_metadata_details(
    standalone_metadata: &kicad_monkey_core::PcbFootprint,
    standalone_source: &str,
) {
    assert_eq!(standalone_metadata.component_classes.len(), 1);
    assert_eq!(standalone_metadata.component_classes[0].name, "Power");
    assert_eq!(standalone_metadata.private_layers, ["F.Cu", "In1.Cu"]);
    assert_eq!(
        standalone_metadata.net_tie_pad_groups[0].pad_names,
        ["1", "2"]
    );
    assert_eq!(
        standalone_metadata.net_tie_pad_groups[0]
            .raw_token
            .as_deref(),
        Some("\"1, 2\"")
    );
    let net_tie_range = &standalone_metadata.net_tie_pad_groups[0].source_range;
    assert_eq!(&standalone_source[net_tie_range.clone()], "\"1, 2\"");
    assert_eq!(
        standalone_metadata.jumper_pad_groups[0].pad_names,
        ["1", "2"]
    );
    assert_eq!(standalone_metadata.thermal_width, Some(0.31));
    assert_eq!(standalone_metadata.thermal_gap, Some(0.21));
    assert_eq!(standalone_metadata.solder_mask_margin, Some(0.08));
}

pub(super) fn assert_pads(standalone: &FootprintView<'_>, board: &PcbView<'_>) {
    let standalone_pads = standalone
        .pads()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone pads");
    let board_pads = board
        .pads()
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded pads");
    assert_eq!(standalone_pads.len(), 3);
    for (standalone_pad, board_pad) in standalone_pads.iter().zip(&board_pads) {
        assert_pad_parity(standalone_pad, board_pad);
    }
    assert_pad_details(&standalone_pads);
}

fn assert_pad_details(standalone_pads: &[kicad_monkey_core::PcbPad]) {
    assert_eq!(standalone_pads[0].angle, 30.0);
    assert_eq!(standalone_pads[0].plated, Some(true));
    assert_eq!(standalone_pads[0].net.ordinal, Some(1));
    assert_eq!(standalone_pads[0].net.name.as_deref(), Some("GND"));
    assert_eq!(
        standalone_pads[0]
            .drill
            .as_ref()
            .expect("slot")
            .shape
            .as_str(),
        "oval"
    );
    let slot = standalone_pads[0].drill.as_ref().expect("slot details");
    assert_eq!((slot.width, slot.height), (0.8, Some(1.4)));
    assert_eq!((slot.offset.x, slot.offset.y), (0.1, 0.2));
    assert_eq!(standalone_pads[0].remove_unused_layers, Some(true));
    assert_eq!(standalone_pads[1].kind, "np_thru_hole");
    assert_eq!(standalone_pads[1].plated, Some(false));
    assert_eq!(standalone_pads[2].custom_primitives[0].points.len(), 3);
    assert_eq!(standalone_pads[2].plated, None);
}

pub(super) fn assert_models_and_graphics(standalone: &FootprintView<'_>, board: &PcbView<'_>) {
    let standalone_models = standalone
        .models()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone models");
    let board_models = board
        .models()
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded models");
    assert_eq!(standalone_models[0].path, board_models[0].path);
    assert_eq!(
        standalone_models[0].owner,
        PcbFootprintMemberOwner::StandaloneFootprint
    );
    assert_eq!(standalone_models[0].offset, [1.0, 2.0, 3.0]);
    assert_eq!(standalone_models[0].rotate, [0.0, 0.0, 90.0]);

    let standalone_graphics = standalone
        .graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone graphics");
    let board_graphics = board
        .footprint_graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded graphics");
    assert_eq!(standalone_graphics.len(), 2);
    assert_eq!(standalone_graphics[0].kind, board_graphics[0].graphic.kind);
    assert_eq!(
        standalone_graphics[0].start,
        board_graphics[0].graphic.start
    );
    assert_eq!(standalone_graphics[0].end, board_graphics[0].graphic.end);
    assert_eq!(standalone_graphics[0].uuid.as_deref(), Some("line-id"));
    assert_eq!(standalone_graphics[1].points.len(), 4);
    assert_eq!(standalone_graphics[1].uuid.as_deref(), Some("curve-id"));
}

pub(super) fn assert_properties_and_text(standalone: &FootprintView<'_>, board: &PcbView<'_>) {
    let standalone_properties = standalone
        .graphical_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone properties");
    let board_properties = board
        .footprint_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded properties");
    assert_eq!(standalone_properties.len(), board_properties.len());
    for (standalone_property, board_property) in standalone_properties.iter().zip(&board_properties)
    {
        assert_eq!(standalone_property.name, board_property.name);
        assert_eq!(standalone_property.value, board_property.value);
        assert_eq!(standalone_property.layer, board_property.layer);
        assert_eq!(
            standalone_property.effects.font,
            board_property.effects.font
        );
        assert_eq!(
            standalone_property.effects.justify,
            board_property.effects.justify
        );
    }
    assert_eq!(standalone_properties[0].uuid, board_properties[0].uuid);
    assert!(standalone_properties[0].effects.source_range.is_some());
    assert!(standalone_properties[0].render_cache_range.is_some());
    assert!(board_properties[0].render_cache_range.is_some());

    assert_text_parity(standalone, board);
}

fn assert_text_parity(standalone: &FootprintView<'_>, board: &PcbView<'_>) {
    let standalone_texts = standalone
        .texts()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone texts");
    let board_texts = board
        .footprint_texts()
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded texts");
    assert_eq!(standalone_texts.len(), 1);
    assert_eq!(standalone_texts[0].text, board_texts[0].text);
    assert_eq!(standalone_texts[0].angle, board_texts[0].angle);
    assert_eq!(
        standalone_texts[0].effects.font,
        board_texts[0].effects.font
    );
    assert_eq!(
        standalone_texts[0].effects.justify,
        board_texts[0].effects.justify
    );
    assert_eq!(standalone_texts[0].uuid, board_texts[0].uuid);
    assert!(standalone_texts[0].effects.source_range.is_some());
    assert!(standalone_texts[0].render_cache_range.is_some());
    assert!(board_texts[0].render_cache_range.is_some());
}

pub(super) fn assert_text_boxes(standalone: &FootprintView<'_>, board: &PcbView<'_>) {
    let standalone_boxes = standalone
        .text_boxes()
        .collect::<Result<Vec<_>, _>>()
        .expect("standalone text boxes");
    let board_boxes = board
        .footprint_text_boxes()
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded text boxes");
    assert_eq!(standalone_boxes.len(), 1);
    assert_eq!(standalone_boxes[0].text, board_boxes[0].text);
    assert_eq!(standalone_boxes[0].margins, board_boxes[0].margins);
    assert_eq!(standalone_boxes[0].stroke_kind, board_boxes[0].stroke_kind);
    assert_eq!(standalone_boxes[0].uuid, board_boxes[0].uuid);
    assert!(standalone_boxes[0].render_cache_range.is_some());
    assert!(board_boxes[0].render_cache_range.is_some());
}

fn assert_pad_parity(
    standalone_pad: &kicad_monkey_core::PcbPad,
    board_pad: &kicad_monkey_core::PcbPad,
) {
    assert_eq!(standalone_pad.number, board_pad.number);
    assert_eq!(standalone_pad.kind, board_pad.kind);
    assert_eq!(standalone_pad.shape, board_pad.shape);
    assert_eq!(standalone_pad.uuid, board_pad.uuid);
    assert_eq!(standalone_pad.layers, board_pad.layers);
    assert_eq!(standalone_pad.drill, board_pad.drill);
    assert_eq!(
        standalone_pad
            .custom_options
            .as_ref()
            .map(|options| (&options.clearance, &options.anchor)),
        board_pad
            .custom_options
            .as_ref()
            .map(|options| (&options.clearance, &options.anchor))
    );
    assert_eq!(
        standalone_pad
            .custom_primitives
            .iter()
            .map(|primitive| (
                &primitive.kind,
                &primitive.points,
                &primitive.geometry,
                primitive.width,
                &primitive.fill
            ))
            .collect::<Vec<_>>(),
        board_pad
            .custom_primitives
            .iter()
            .map(|primitive| (
                &primitive.kind,
                &primitive.points,
                &primitive.geometry,
                primitive.width,
                &primitive.fill
            ))
            .collect::<Vec<_>>()
    );
}

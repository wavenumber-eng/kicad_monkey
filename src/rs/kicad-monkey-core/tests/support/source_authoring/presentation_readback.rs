use super::*;

pub(super) fn assert_board_graphics(board_document: &kicad_monkey_core::PcbDocument) {
    let board_view = board_document.view().expect("board view");
    let board_graphics = board_view
        .graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("board graphics");
    assert_eq!(
        board_graphics
            .iter()
            .map(|graphic| graphic.kind)
            .collect::<Vec<_>>(),
        [
            PcbGraphicKind::Circle,
            PcbGraphicKind::Text,
            PcbGraphicKind::Text,
            PcbGraphicKind::TextBox,
        ]
    );
    assert_eq!(board_graphics[0].layer.as_deref(), Some("B.SilkS"));
    assert_eq!(board_graphics[0].stroke_width, Some(0.0));
    assert_eq!(board_graphics[0].fill.as_deref(), Some("solid"));
    let footprint_graphics = board_view
        .footprint_graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("footprint graphics");
    assert_eq!(footprint_graphics[1].graphic.stroke_width, Some(0.0));
    assert_eq!(footprint_graphics[1].graphic.fill.as_deref(), Some("solid"));
    assert_eq!(board_graphics[1].text.as_deref(), Some("BOARD-TTF"));
    assert_eq!(board_graphics[2].at.expect("native text position").x, 15.0);
    assert_eq!(board_graphics[3].border, Some(false));
}

pub(super) fn assert_placed_presentation(board_document: &kicad_monkey_core::PcbDocument) {
    let board_view = board_document.view().expect("board view");
    let board_properties = board_view
        .footprint_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("board footprint properties");
    assert_eq!(board_properties[0].name, "Reference");
    assert_eq!(board_properties[0].layer, "B.SilkS");
    assert!(board_properties[0].unlocked);
    assert_eq!(
        board_properties[0].effects.font.face.as_deref(),
        Some("Arial")
    );
    assert_eq!(board_properties[0].effects.font.line_spacing, Some(1.1));
    assert_eq!(
        board_properties[0].effects.justify,
        ["right", "top", "mirror"]
    );
    let occurrence_cache = cache_from_range(
        board_document.source(),
        board_properties[0]
            .render_cache_range
            .clone()
            .expect("occurrence cache"),
    );
    assert_eq!(
        (
            occurrence_cache.text.as_str(),
            occurrence_cache.angle_degrees
        ),
        ("R1", 15.0)
    );
    let expected_board_points = [
        (30.707_106_781_186_55, 15.878_679_656_440_358),
        (31.957_106_781_186_553, 18.878_679_656_440_358),
        (33.707_106_781_186_55, 21.378_679_656_440_358),
        (31.207_106_781_186_553, 16.878_679_656_440_358),
        (31.457_106_781_186_553, 17.378_679_656_440_358),
        (31.707_106_781_186_553, 16.878_679_656_440_358),
    ];
    for (actual, expected) in occurrence_cache
        .polygons
        .iter()
        .flat_map(|polygon| &polygon.contours)
        .flat_map(|contour| &contour.points)
        .zip(expected_board_points)
    {
        assert_close(actual.x, expected.0);
        assert_close(actual.y, expected.1);
    }
    assert_placed_text_and_boxes(&board_view);
}

fn assert_placed_text_and_boxes(board_view: &kicad_monkey_core::PcbView<'_>) {
    let board_texts = board_view
        .footprint_texts()
        .collect::<Result<Vec<_>, _>>()
        .expect("board footprint texts");
    assert!(board_texts[0].knockout);
    assert!(board_texts[0].unlocked);
    assert_eq!(board_texts[0].effects.font.face, None);
    let board_boxes = board_view
        .footprint_text_boxes()
        .collect::<Result<Vec<_>, _>>()
        .expect("board footprint boxes");
    assert_eq!(board_boxes[0].margins, [0.1, 0.2, 0.3, 0.4]);
    assert_eq!(
        board_boxes[0]
            .polygon_points
            .iter()
            .map(|point| (point.x, point.y))
            .collect::<Vec<_>>(),
        [(-3.0, -2.0), (3.0, -2.0), (3.0, 2.0), (-3.0, 2.0)]
    );
    assert_eq!(board_boxes[0].stroke_kind.as_deref(), Some("dash"));
    assert_eq!(board_boxes[0].border, Some(true));
}

pub(super) fn assert_standalone_presentation(
    footprint_document: &kicad_monkey_core::FootprintDocument,
) {
    let footprint_view = footprint_document.view().expect("footprint view");
    let properties = footprint_view
        .graphical_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("properties");
    let reference = &properties[0];
    assert_eq!(
        (reference.at_x, reference.at_y, reference.angle),
        (1.0, 2.0, 15.0)
    );
    assert_eq!(reference.effects.font.size_x, 1.2);
    assert_eq!(reference.effects.font.size_y, 0.8);
    assert_eq!(reference.effects.font.color, None);
    assert_eq!(reference.effects.href, None);
    let reference_cache = cache_from_range(
        footprint_document.source(),
        reference
            .render_cache_range
            .clone()
            .expect("reference cache"),
    );
    assert_eq!(reference_cache, cache("R1", 15.0));

    assert_standalone_text_and_boxes(&footprint_view);
}

fn assert_standalone_text_and_boxes(footprint_view: &kicad_monkey_core::FootprintView<'_>) {
    let texts = footprint_view
        .texts()
        .collect::<Result<Vec<_>, _>>()
        .expect("texts");
    assert_eq!(texts[0].text, "NATIVE~{A}");
    assert_eq!(texts[0].angle, -30.0);
    assert_eq!(texts[0].effects.font.face, None);
    assert_eq!(texts[0].effects.justify, ["left", "bottom", "mirror"]);
    assert!(texts[0].knockout);
    assert!(texts[0].render_cache_range.is_none());

    let boxes = footprint_view
        .text_boxes()
        .collect::<Result<Vec<_>, _>>()
        .expect("text boxes");
    assert_eq!((boxes[0].start_x, boxes[0].end_x), (-3.0, 3.0));
    assert!(!boxes[0].locked);
    assert_eq!(boxes[0].knockout, Some(false));
    assert!(boxes[0].render_cache_range.is_none());
}

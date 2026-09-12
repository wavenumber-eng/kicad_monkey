use kicad_monkey_core::{
    AuthoredBoardText, AuthoredColor, AuthoredFootprint, AuthoredFootprintOccurrence,
    AuthoredFootprintProperty, AuthoredFootprintText, AuthoredGraphic, AuthoredGraphicGeometry,
    AuthoredLayer, AuthoredPcb, AuthoredPoint, AuthoredStandaloneFootprint, AuthoredTextBox,
    AuthoredTextBoxGeometry, AuthoredTextEffects, AuthoredTextHorizontalJustification,
    AuthoredTextVerticalJustification, ErrorKind, PcbAuthoringLimits, PcbGraphicKind, TextContour,
    TextPoint, TextRenderCache, TextRenderCacheLimits, TextRenderCachePolygon,
    read_text_render_cache_a0,
};

fn point(x_mm: f64, y_mm: f64) -> AuthoredPoint {
    AuthoredPoint { x_mm, y_mm }
}

fn uuid(value: u64) -> String {
    format!("00000000-0000-0000-0000-{value:012x}")
}

fn cache(text: &str, angle_degrees: f64) -> TextRenderCache {
    TextRenderCache {
        text: text.to_owned(),
        angle_degrees,
        polygons: vec![TextRenderCachePolygon {
            contours: vec![
                TextContour {
                    points: vec![
                        TextPoint { x: 1.0, y: 0.0 },
                        TextPoint { x: 2.25, y: 3.0 },
                        TextPoint { x: 4.0, y: 5.5 },
                    ],
                },
                TextContour {
                    points: vec![
                        TextPoint { x: 1.5, y: 1.0 },
                        TextPoint { x: 1.75, y: 1.5 },
                        TextPoint { x: 2.0, y: 1.0 },
                    ],
                },
            ],
        }],
    }
}

fn ttf_effects() -> AuthoredTextEffects {
    AuthoredTextEffects {
        face: Some("Arial".to_owned()),
        size_x_mm: 1.2,
        size_y_mm: 0.8,
        thickness_mm: Some(0.12),
        line_spacing: Some(1.1),
        bold: true,
        italic: true,
        color: None,
        horizontal_justify: AuthoredTextHorizontalJustification::Right,
        vertical_justify: AuthoredTextVerticalJustification::Top,
        mirrored: true,
        href: None,
    }
}

fn newstroke_effects() -> AuthoredTextEffects {
    AuthoredTextEffects {
        size_x_mm: 1.5,
        size_y_mm: 1.0,
        thickness_mm: Some(0.2),
        horizontal_justify: AuthoredTextHorizontalJustification::Left,
        vertical_justify: AuthoredTextVerticalJustification::Bottom,
        mirrored: true,
        ..AuthoredTextEffects::default()
    }
}

fn footprint() -> AuthoredFootprint {
    let mut footprint = AuthoredFootprint::new("Demo:Presentation");
    footprint.properties = vec![
        AuthoredFootprintProperty {
            name: "Reference".to_owned(),
            value: "R1".to_owned(),
            at: point(1.0, 2.0),
            angle_degrees: 15.0,
            layer: "B.SilkS".to_owned(),
            hidden: false,
            unlocked: true,
            effects: ttf_effects(),
            render_cache: Some(cache("R1", 15.0)),
            uuid: uuid(101),
        },
        AuthoredFootprintProperty {
            name: "Value".to_owned(),
            value: "Demo:Presentation".to_owned(),
            at: point(1.0, 4.0),
            angle_degrees: -20.0,
            layer: "B.Fab".to_owned(),
            hidden: true,
            unlocked: false,
            effects: newstroke_effects(),
            render_cache: None,
            uuid: uuid(102),
        },
    ];
    footprint.texts.push(AuthoredFootprintText {
        text: "NATIVE~{A}".to_owned(),
        at: point(-2.0, 3.0),
        angle_degrees: -30.0,
        layer: "B.SilkS".to_owned(),
        hidden: false,
        unlocked: true,
        knockout: true,
        effects: newstroke_effects(),
        render_cache: None,
        uuid: uuid(103),
    });
    footprint.text_boxes.push(AuthoredTextBox {
        text: "BOX".to_owned(),
        geometry: AuthoredTextBoxGeometry::Polygon {
            points: vec![
                point(-3.0, -2.0),
                point(3.0, -2.0),
                point(3.0, 2.0),
                point(-3.0, 2.0),
            ],
        },
        margins_mm: [0.1, 0.2, 0.3, 0.4],
        angle_degrees: 45.0,
        layer: "B.SilkS".to_owned(),
        locked: true,
        border: true,
        knockout: false,
        stroke_width_mm: 0.1,
        stroke_kind: "dash".to_owned(),
        effects: newstroke_effects(),
        render_cache: None,
        uuid: uuid(104),
    });
    footprint.graphics.push(AuthoredGraphic {
        geometry: AuthoredGraphicGeometry::Line {
            start: point(-4.0, 0.0),
            end: point(4.0, 0.0),
        },
        layer: "B.SilkS".to_owned(),
        stroke_width_mm: 0.15,
        stroke_kind: "solid".to_owned(),
        fill: None,
        uuid: uuid(105),
    });
    footprint
}

fn layers() -> Vec<AuthoredLayer> {
    [
        (0, "F.Cu", "signal"),
        (31, "B.Cu", "signal"),
        (36, "B.SilkS", "user"),
        (37, "F.SilkS", "user"),
        (48, "B.Fab", "user"),
        (49, "F.Fab", "user"),
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

fn board() -> AuthoredPcb {
    let mut rotated_footprint = footprint();
    rotated_footprint.text_boxes.clear();
    let mut boxed_footprint = footprint();
    boxed_footprint.properties.clear();
    boxed_footprint.texts.clear();
    boxed_footprint.graphics.clear();
    boxed_footprint.text_boxes[0].uuid = uuid(304);
    AuthoredPcb {
        layers: layers(),
        graphics: vec![AuthoredGraphic {
            geometry: AuthoredGraphicGeometry::Circle {
                center: point(10.0, 10.0),
                end: point(12.0, 10.0),
            },
            layer: "B.SilkS".to_owned(),
            stroke_width_mm: 0.2,
            stroke_kind: "dash_dot".to_owned(),
            fill: Some("none".to_owned()),
            uuid: uuid(201),
        }],
        texts: vec![
            AuthoredBoardText {
                text: "BOARD-TTF".to_owned(),
                at: point(15.0, 10.0),
                angle_degrees: 90.0,
                layer: "F.SilkS".to_owned(),
                locked: true,
                knockout: false,
                effects: ttf_effects(),
                render_cache: Some(cache("BOARD-TTF", 90.0)),
                uuid: uuid(202),
            },
            AuthoredBoardText {
                text: "BOARD-NATIVE".to_owned(),
                at: point(15.0, 15.0),
                angle_degrees: -45.0,
                layer: "B.SilkS".to_owned(),
                locked: false,
                knockout: true,
                effects: newstroke_effects(),
                render_cache: None,
                uuid: uuid(203),
            },
        ],
        text_boxes: vec![AuthoredTextBox {
            text: "BOARD-BOX".to_owned(),
            geometry: AuthoredTextBoxGeometry::Rectangle {
                start: point(20.0, 10.0),
                end: point(28.0, 14.0),
            },
            margins_mm: [0.1; 4],
            angle_degrees: 30.0,
            layer: "B.SilkS".to_owned(),
            locked: true,
            border: false,
            knockout: true,
            stroke_width_mm: 0.12,
            stroke_kind: "solid".to_owned(),
            effects: newstroke_effects(),
            render_cache: None,
            uuid: uuid(204),
        }],
        footprints: vec![
            AuthoredFootprintOccurrence {
                footprint: rotated_footprint,
                layer: "B.Cu".to_owned(),
                at: point(30.0, 20.0),
                angle_degrees: 135.0,
                uuid: uuid(200),
            },
            AuthoredFootprintOccurrence {
                footprint: boxed_footprint,
                layer: "F.Cu".to_owned(),
                at: point(50.0, 20.0),
                angle_degrees: 0.0,
                uuid: uuid(300),
            },
        ],
        ..AuthoredPcb::default()
    }
}

fn standalone() -> AuthoredStandaloneFootprint {
    let mut value = AuthoredStandaloneFootprint::new("Demo_Presentation", "F.Cu");
    value.footprint = footprint();
    value.footprint.name = "Demo_Presentation".to_owned();
    value.footprint.text_boxes[0].locked = false;
    value.footprint.text_boxes[0].geometry = AuthoredTextBoxGeometry::Rectangle {
        start: point(-3.0, -2.0),
        end: point(3.0, 2.0),
    };
    value
}

fn publish(name: &str, source: &str) {
    let Some(directory) = std::env::var_os("KM_PRESENTATION_OUTPUT_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create presentation oracle directory");
    std::fs::write(directory.join(name), source).expect("write presentation source");
}

fn cache_from_range(source: &str, range: std::ops::Range<usize>) -> TextRenderCache {
    read_text_render_cache_a0(&source.as_bytes()[range], TextRenderCacheLimits::default())
        .expect("typed cache readback")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "{actual} != {expected}"
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one proof follows source presentation facts through every carrier"
)]
fn authored_board_and_footprint_presentation_round_trip_exactly() {
    let board_document = board()
        .to_document(PcbAuthoringLimits::default())
        .expect("presentation board");
    publish("native-presentation.kicad_pcb", board_document.source());
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
    assert_eq!(board_graphics[1].text.as_deref(), Some("BOARD-TTF"));
    assert_eq!(board_graphics[2].at.expect("native text position").x, 15.0);
    assert_eq!(board_graphics[3].border, Some(false));

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

    let footprint_document = standalone()
        .to_document(PcbAuthoringLimits::default())
        .expect("presentation footprint");
    publish("Demo_Presentation.kicad_mod", footprint_document.source());
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

#[test]
fn stale_or_newstroke_render_caches_and_invalid_effects_fail_before_emission() {
    let mut stale_text = standalone();
    stale_text.footprint.properties[0]
        .render_cache
        .as_mut()
        .expect("cache")
        .text = "stale".to_owned();

    let mut stale_angle = standalone();
    stale_angle.footprint.properties[0]
        .render_cache
        .as_mut()
        .expect("cache")
        .angle_degrees = 0.0;

    let mut newstroke_cache = standalone();
    newstroke_cache.footprint.texts[0].render_cache = Some(cache("NATIVE~{A}", -30.0));

    let mut invalid_color = standalone();
    invalid_color.footprint.properties[0].effects.color = Some(AuthoredColor {
        red: 10,
        green: 20,
        blue: 30,
        alpha: 0.5,
    });

    let mut unsupported_href = standalone();
    unsupported_href.footprint.properties[0].effects.href =
        Some("https://example.invalid/native".to_owned());

    let mut discarded_standalone_lock = standalone();
    discarded_standalone_lock.footprint.text_boxes[0].locked = true;

    let mut rotated_text_box = board();
    rotated_text_box.footprints[1].angle_degrees = 45.0;

    let mut transformed_overflow = board();
    transformed_overflow.footprints[0].at.x_mm = f64::MAX;
    transformed_overflow.footprints[0].angle_degrees = 0.0;
    transformed_overflow.footprints[0].footprint.properties[0]
        .render_cache
        .as_mut()
        .expect("cache")
        .polygons[0]
        .contours[0]
        .points[0]
        .x = f64::MAX;

    let mut hidden_standalone_text = standalone();
    hidden_standalone_text.footprint.texts[0].hidden = true;

    let mut hidden_embedded_text = board();
    hidden_embedded_text.footprints[0].footprint.texts[0].hidden = true;

    for value in [
        stale_text,
        stale_angle,
        newstroke_cache,
        invalid_color,
        unsupported_href,
        discarded_standalone_lock,
        hidden_standalone_text,
    ] {
        let error = value
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("invalid presentation source");
        assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    }
    let error = transformed_overflow
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("overflowing occurrence cache transform");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    let error = hidden_embedded_text
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("hidden embedded user text rewrite");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    let error = rotated_text_box
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("rotated embedded text-box loss");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
}

#[test]
fn footprint_cache_realization_uses_resolved_text_keep_upright_and_carrier_frames() {
    let mut value = board();
    let reference = &mut value.footprints[0].footprint.properties[0];
    reference.value = "${Value}".to_owned();
    reference.angle_degrees = 180.0;
    reference.unlocked = false;
    reference.render_cache = Some(cache("Demo:Presentation", 0.0));
    let text_box = &mut value.footprints[1].footprint.text_boxes[0];
    text_box.effects = ttf_effects();
    text_box.render_cache = Some(cache("BOX", 45.0));

    let document = value
        .to_document(PcbAuthoringLimits::default())
        .expect("resolved and carrier-aware caches");
    let view = document.view().expect("authored cache board");
    let property = view
        .footprint_properties()
        .next()
        .expect("reference property")
        .expect("typed reference property");
    let property_cache = cache_from_range(
        document.source(),
        property.render_cache_range.expect("property cache range"),
    );
    assert_eq!(property_cache.text, "Demo:Presentation");
    assert_eq!(property_cache.angle_degrees, 0.0);
    assert_close(
        property_cache.polygons[0].contours[0].points[0].x,
        30.70710678118655,
    );
    assert_close(
        property_cache.polygons[0].contours[0].points[0].y,
        15.878679656440358,
    );

    let text_box = view
        .footprint_text_boxes()
        .next()
        .expect("footprint text box")
        .expect("typed footprint text box");
    let text_box_cache = cache_from_range(
        document.source(),
        text_box.render_cache_range.expect("text-box cache range"),
    );
    assert_eq!(text_box_cache.text, "BOX");
    assert_eq!(text_box_cache.angle_degrees, 45.0);
    assert_close(text_box_cache.polygons[0].contours[0].points[0].x, 51.0);
    assert_close(text_box_cache.polygons[0].contours[0].points[0].y, 20.0);
}

#[test]
fn footprint_cache_validation_rejects_unresolved_or_wrong_realization_context() {
    let mut unresolved = board();
    unresolved.footprints[0].footprint.properties[0].value = "${MISSING}".to_owned();
    unresolved.footprints[0].footprint.properties[0]
        .render_cache
        .as_mut()
        .expect("cache")
        .text = "${MISSING}".to_owned();
    assert_eq!(
        unresolved
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("unresolved cached footprint variable")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut locked_raw_angle = board();
    let reference = &mut locked_raw_angle.footprints[0].footprint.properties[0];
    reference.angle_degrees = 180.0;
    reference.unlocked = false;
    reference
        .render_cache
        .as_mut()
        .expect("cache")
        .angle_degrees = 180.0;
    assert_eq!(
        locked_raw_angle
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("locked raw cache angle")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut standalone_locked = standalone();
    let reference = &mut standalone_locked.footprint.properties[0];
    reference.angle_degrees = 180.0;
    reference.unlocked = false;
    reference
        .render_cache
        .as_mut()
        .expect("cache")
        .angle_degrees = 0.0;
    standalone_locked
        .canonical_text(PcbAuthoringLimits::default())
        .expect("standalone locked cache folds 180 degrees to zero");

    let mut standalone_raw_angle = standalone_locked;
    standalone_raw_angle.footprint.properties[0]
        .render_cache
        .as_mut()
        .expect("cache")
        .angle_degrees = 180.0;
    assert_eq!(
        standalone_raw_angle
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("standalone locked raw cache angle")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut unresolved_board = board();
    unresolved_board.texts[0].text = "${PROJECT}".to_owned();
    unresolved_board.texts[0]
        .render_cache
        .as_mut()
        .expect("cache")
        .text = "resolved".to_owned();
    assert_eq!(
        unresolved_board
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("board cache requires companion variables")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    for (angle, unlocked, cache_angle) in [
        (180.0, false, 0.0),
        (91.0, false, -89.0),
        (180.0, true, 180.0),
    ] {
        let mut angle_case = board();
        let property = &mut angle_case.footprints[0].footprint.properties[0];
        property.angle_degrees = angle;
        property.unlocked = unlocked;
        property
            .render_cache
            .as_mut()
            .expect("property cache")
            .angle_degrees = cache_angle;
        angle_case
            .canonical_text(PcbAuthoringLimits::default())
            .expect("property keep-upright cache angle");

        let mut text_case = board();
        let text = &mut text_case.footprints[0].footprint.texts[0];
        text.angle_degrees = angle;
        text.unlocked = unlocked;
        text.effects = ttf_effects();
        text.render_cache = Some(cache("NATIVE~{A}", cache_angle));
        text_case
            .canonical_text(PcbAuthoringLimits::default())
            .expect("user-text keep-upright cache angle");
    }
}

#[test]
fn presentation_points_obey_an_exact_aggregate_limit() {
    let exact = PcbAuthoringLimits {
        max_points: 13,
        ..PcbAuthoringLimits::default()
    };
    standalone()
        .canonical_text(exact)
        .expect("exact presentation point limit");
    let error = standalone()
        .canonical_text(PcbAuthoringLimits {
            max_points: 12,
            ..PcbAuthoringLimits::default()
        })
        .expect_err("one-under presentation point limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

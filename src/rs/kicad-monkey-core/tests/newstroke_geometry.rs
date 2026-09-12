use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, NewstrokeErrorKind,
    NewstrokeLimits, NewstrokeRequest, PcbLimits, TextHorizontalAlignment, TextVerticalAlignment,
    board_plot_facts_with_sidecars, realize_newstroke_a0,
};

fn request(text: &str) -> NewstrokeRequest<'_> {
    NewstrokeRequest {
        text,
        position_x_mm: 1.0,
        position_y_mm: 2.0,
        size_x_mm: 1.0,
        size_y_mm: 1.0,
        angle_degrees: 0.0,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Bottom,
        mirrored: false,
        italic: false,
        bold: false,
        stroke_width_mm: None,
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-12, "{actual} != {expected}");
}

#[test]
fn public_polylines_match_the_existing_python_newstroke_oracle() {
    let output = realize_newstroke_a0(request("A"), NewstrokeLimits::default()).expect("A");
    assert_eq!(output.polylines.len(), 2, "A has one pen-up break");
    assert_eq!(output.polylines[0].points.len(), 2);
    assert_eq!(output.polylines[1].points.len(), 3);
    let expected = [
        (1.190_476_190_476_190_5, 1.716_685_714_285_714_4),
        (1.666_666_666_666_666_5, 1.716_685_714_285_714_4),
        (1.095_238_095_238_095_3, 2.0024),
        (1.428_571_428_571_428_6, 1.002_400_000_000_000_2),
        (1.761_904_761_904_761_9, 2.0024),
    ];
    for (point, (x, y)) in output
        .polylines
        .iter()
        .flat_map(|polyline| &polyline.points)
        .zip(expected)
    {
        assert_close(point.x_mm, x);
        assert_close(point.y_mm, y);
    }
    assert_eq!(output.point_count, 5);
    assert_close(output.advance_width_mm, 18.0 / 21.0);
    assert_close(output.alignment_width_mm, 18.0 / 21.0);
    assert_close(output.effective_stroke_width_mm, 0.125);
    let bounds = output.bounds_mm.expect("centerline bounds");
    for (actual, expected) in bounds.into_iter().zip([
        1.095_238_095_238_095_3,
        1.002_400_000_000_000_2,
        1.761_904_761_904_761_9,
        2.0024,
    ]) {
        assert_close(actual, expected);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one compact fixed-oracle matrix keeps each placement vector auditable"
)]
fn isolated_alignment_rotation_mirror_and_markup_match_python_oracles() {
    struct Case {
        name: &'static str,
        request: NewstrokeRequest<'static>,
        first: (f64, f64),
        last: (f64, f64),
        bounds: [f64; 4],
    }

    let mut center_center = request("A");
    center_center.horizontal_alignment = TextHorizontalAlignment::Center;
    center_center.vertical_alignment = TextVerticalAlignment::Center;
    let mut right_top = request("A");
    right_top.horizontal_alignment = TextHorizontalAlignment::Right;
    right_top.vertical_alignment = TextVerticalAlignment::Top;
    let mut rotated = request("A");
    rotated.position_x_mm = 0.0;
    rotated.position_y_mm = 0.0;
    rotated.angle_degrees = 90.0;
    let mut mirrored = request("A");
    mirrored.position_x_mm = 0.0;
    mirrored.position_y_mm = 0.0;
    mirrored.mirrored = true;
    let mut subscript = request("_{A}");
    subscript.position_x_mm = 0.0;
    subscript.position_y_mm = 0.0;
    let mut superscript = request("^{A}");
    superscript.position_x_mm = 0.0;
    superscript.position_y_mm = 0.0;
    let mut nested = request("_{^{A}}");
    nested.position_x_mm = 0.0;
    nested.position_y_mm = 0.0;

    let cases = [
        Case {
            name: "center/center",
            request: center_center,
            first: (0.761_904_761_904_761_9, 2.169_066_666_666_666_7),
            last: (1.333_333_333_333_333_3, 2.454_780_952_380_952_3),
            bounds: [
                0.666_666_666_666_666_7,
                1.454_780_952_380_952_3,
                1.333_333_333_333_333_3,
                2.454_780_952_380_952_3,
            ],
        },
        Case {
            name: "right/top",
            request: right_top,
            first: (0.333_333_333_333_333_37, 2.716_685_714_285_714),
            last: (0.904_761_904_761_904_8, 3.0024),
            bounds: [
                0.238_095_238_095_238_14,
                2.0024,
                0.904_761_904_761_904_8,
                3.0024,
            ],
        },
        Case {
            name: "+90 maps +X to -Y",
            request: rotated,
            first: (-0.283_314_285_714_285_7, -0.190_476_190_476_190_5),
            last: (0.002_400_000_000_000_046, -0.761_904_761_904_761_9),
            bounds: [
                -0.997_599_999_999_999_9,
                -0.761_904_761_904_761_9,
                0.002_400_000_000_000_046,
                -0.095_238_095_238_095_23,
            ],
        },
        Case {
            name: "mirror",
            request: mirrored,
            first: (-0.190_476_190_476_190_47, -0.283_314_285_714_285_7),
            last: (-0.761_904_761_904_761_9, 0.0024),
            bounds: [
                -0.761_904_761_904_761_9,
                -0.997_599_999_999_999_9,
                -0.095_238_095_238_095_23,
                0.0024,
            ],
        },
        Case {
            name: "subscript",
            request: subscript,
            first: (0.152_380_952_380_952_4, -0.115_695_238_095_238_07),
            last: (0.609_523_809_523_809_6, 0.112_876_190_476_190_48),
            bounds: [
                0.076_190_476_190_476_2,
                -0.687_123_809_523_809_5,
                0.609_523_809_523_809_6,
                0.112_876_190_476_190_48,
            ],
        },
        Case {
            name: "superscript",
            request: superscript,
            first: (0.152_380_952_380_952_4, -0.515_695_238_095_238),
            last: (0.609_523_809_523_809_6, -0.287_123_809_523_809_5),
            bounds: [
                0.076_190_476_190_476_2,
                -1.087_123_809_523_809_4,
                0.609_523_809_523_809_6,
                -0.287_123_809_523_809_5,
            ],
        },
        Case {
            name: "nested subscript precedence",
            request: nested,
            first: (0.152_380_952_380_952_4, -0.115_695_238_095_238_07),
            last: (0.609_523_809_523_809_6, 0.112_876_190_476_190_48),
            bounds: [
                0.076_190_476_190_476_2,
                -0.687_123_809_523_809_5,
                0.609_523_809_523_809_6,
                0.112_876_190_476_190_48,
            ],
        },
    ];

    for case in cases {
        let output = realize_newstroke_a0(case.request, NewstrokeLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let first = output.polylines.first().expect(case.name).points[0];
        let last = *output
            .polylines
            .last()
            .expect(case.name)
            .points
            .last()
            .expect(case.name);
        assert_close(first.x_mm, case.first.0);
        assert_close(first.y_mm, case.first.1);
        assert_close(last.x_mm, case.last.0);
        assert_close(last.y_mm, case.last.1);
        for (actual, expected) in output
            .bounds_mm
            .expect(case.name)
            .into_iter()
            .zip(case.bounds)
        {
            assert_close(actual, expected);
        }
    }
}

#[test]
fn placement_rotation_mirroring_alignment_and_width_are_source_semantic() {
    let mut transformed = request("A");
    transformed.size_y_mm = 2.0;
    transformed.angle_degrees = 90.0;
    transformed.horizontal_alignment = TextHorizontalAlignment::Center;
    transformed.vertical_alignment = TextVerticalAlignment::Top;
    transformed.mirrored = true;
    transformed.italic = true;
    transformed.bold = true;
    let output = realize_newstroke_a0(transformed, NewstrokeLimits::default()).expect("placed A");
    let first = output.polylines[0].points[0];
    assert_close(first.x_mm, 2.433_371_428_571_428_3);
    assert_close(first.y_mm, 1.941_076_190_476_190_6);
    let last = output.polylines[1].points[2];
    assert_close(last.x_mm, 3.0048);
    assert_close(last.y_mm, 2.583_933_333_333_333_4);
    assert_close(output.effective_stroke_width_mm, 0.2);

    let mut explicit = request("Cu");
    explicit.stroke_width_mm = Some(0.05);
    assert_close(
        realize_newstroke_a0(explicit, NewstrokeLimits::default())
            .expect("explicit copper width")
            .effective_stroke_width_mm,
        0.05,
    );
}

#[test]
fn markup_pen_ups_and_missing_glyph_behavior_are_explicit() {
    let mut markup = request("A_{2}~{B}");
    markup.position_x_mm = 0.0;
    markup.position_y_mm = 0.0;
    let output = realize_newstroke_a0(markup, NewstrokeLimits::default()).expect("markup");
    assert_eq!(output.polylines.len(), 5);
    assert_eq!(output.point_count, 37);
    assert_eq!(output.polylines.last().expect("overbar").points.len(), 2);
    let overbar = &output.polylines.last().expect("overbar").points;
    assert_close(overbar[0].x_mm, 1.719_047_619_047_619_2);
    assert_close(overbar[0].y_mm, -1.275_219_047_619_047_7);
    assert_close(overbar[1].x_mm, 2.519_047_619_047_619);
    assert_close(overbar[1].y_mm, -1.275_219_047_619_047_7);

    let newline = realize_newstroke_a0(request("A\nB"), NewstrokeLimits::default())
        .expect("documented single-line missing-glyph fallback");
    assert_eq!(newline.missing_glyphs, 1);
    let question = realize_newstroke_a0(request("A?B"), NewstrokeLimits::default()).expect("?");
    let plain = realize_newstroke_a0(request("AB"), NewstrokeLimits::default()).expect("plain");
    assert_eq!(newline.polylines, question.polylines);
    assert_close(newline.advance_width_mm, question.advance_width_mm);
    assert_close(newline.alignment_width_mm, plain.alignment_width_mm);
    let unsupported = realize_newstroke_a0(request("\u{10ffff}"), NewstrokeLimits::default())
        .expect("fallback glyph");
    assert_eq!(unsupported.missing_glyphs, 1);
    let question = realize_newstroke_a0(request("?"), NewstrokeLimits::default()).expect("?");
    assert_eq!(unsupported.polylines, question.polylines);
    assert_close(unsupported.advance_width_mm, 18.0 / 21.0);
    assert_close(unsupported.alignment_width_mm, 0.0);
}

#[test]
fn board_bounds_reuse_pins_exact_newstroke_centerlines() {
    let source = r#"(kicad_pcb
      (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (37 "F.SilkS" user "f.silkscreen"))
      (footprint "Test:A" (layer "F.Cu") (at 0 0)
        (property "Reference" "A" (at 1 2 0) (layer "F.SilkS")
          (effects (font (size 1 1) (thickness 0.1)) (justify left bottom)))))"#;
    let facts = board_plot_facts_with_sidecars(
        source,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .expect("board facts");
    assert_eq!(
        facts.bounds(None, Default::default()).expect("bounds"),
        Some([1_095_238, 1_002_400, 1_761_905, 2_002_400])
    );
}

#[test]
fn every_public_limit_is_exact_and_fails_one_under() {
    let baseline =
        realize_newstroke_a0(request("A_{2}"), NewstrokeLimits::default()).expect("baseline");
    let exact = NewstrokeLimits {
        max_text_bytes: "A_{2}".len(),
        max_markup_nodes: baseline.markup_nodes,
        max_polylines: baseline.polylines.len(),
        max_points: baseline.point_count,
    };
    realize_newstroke_a0(request("A_{2}"), exact).expect("exact limits");
    for limits in [
        NewstrokeLimits {
            max_text_bytes: exact.max_text_bytes - 1,
            ..exact
        },
        NewstrokeLimits {
            max_markup_nodes: exact.max_markup_nodes - 1,
            ..exact
        },
        NewstrokeLimits {
            max_polylines: exact.max_polylines - 1,
            ..exact
        },
        NewstrokeLimits {
            max_points: exact.max_points - 1,
            ..exact
        },
    ] {
        assert_eq!(
            realize_newstroke_a0(request("A_{2}"), limits)
                .expect_err("one-under limit")
                .kind,
            NewstrokeErrorKind::ResourceLimit
        );
    }
}

#[test]
fn invalid_geometry_and_stroke_width_fail_before_realization() {
    let mut invalid = request("A");
    invalid.angle_degrees = f64::NAN;
    assert_eq!(
        realize_newstroke_a0(invalid, NewstrokeLimits::default())
            .expect_err("nonfinite angle")
            .kind,
        NewstrokeErrorKind::InvalidInput
    );
    invalid = request("A");
    invalid.stroke_width_mm = Some(-0.1);
    assert_eq!(
        realize_newstroke_a0(invalid, NewstrokeLimits::default())
            .expect_err("negative width")
            .kind,
        NewstrokeErrorKind::InvalidInput
    );
    invalid = request("A");
    invalid.position_x_mm = f64::MAX;
    invalid.size_x_mm = f64::MAX;
    assert_eq!(
        realize_newstroke_a0(invalid, NewstrokeLimits::default())
            .expect_err("derived coordinate overflow")
            .kind,
        NewstrokeErrorKind::InvalidInput
    );
}

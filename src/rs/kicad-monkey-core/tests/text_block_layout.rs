use kicad_monkey_contracts::generated::shaping_record::{
    OpenTypeTag, ShapingFeature, ShapingInput,
};
use kicad_monkey_core::{
    KICAD_OUTLINE_FACE_SCALER, KICAD_OUTLINE_SIZE_COMPENSATION, KICAD_SUBSCRIPT_FACE_SCALER,
    KICAD_SUBSCRIPT_VERTICAL_OFFSET_RATIO, KICAD_SUPERSCRIPT_VERTICAL_OFFSET_RATIO,
    KICAD_TEXT_INTERLINE_FACTOR, KICAD_TEXT_OVERBAR_HEIGHT_RATIO, KICAD_TEXT_TAB_WIDTH_FACTOR,
    TextBlockLayoutLimits, TextBlockLayoutRequest, TextContourErrorKind, TextContourLimits,
    TextHorizontalAlignment, TextRenderCacheErrorKind, TextRenderCacheLimits,
    TextVerticalAlignment, generate_text_render_cache_block_hinted_a0, layout_text_block_hinted_a0,
};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));
const CFF_FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-cff-fixture.otf"
));

#[derive(Deserialize)]
struct Vectors {
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Record {
    shaping: ShapingInput,
}

fn base_shaping(text: &str) -> ShapingInput {
    let vectors: Vectors = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_layout_vectors.json"
    )))
    .unwrap();
    let mut shaping = vectors.records.into_iter().next().unwrap().shaping;
    shaping.text = text.to_owned();
    shaping.features.clear();
    shaping
}

fn cff_shaping(text: &str) -> ShapingInput {
    let mut shaping = base_shaping(text);
    shaping.font_id = "outline_cff_fixture".parse().unwrap();
    shaping.font_sha256.0 =
        "83e67e1e0332393f5bb0c0352e8f92d705b31672e9e992589464b9426583de49".to_owned();
    shaping.scale_x = 1000;
    shaping.scale_y = 1000;
    shaping
}

fn request(shaping: &ShapingInput) -> TextBlockLayoutRequest<'_> {
    TextBlockLayoutRequest {
        shaping,
        size_x: 2.0,
        size_y: 2.0,
        position_x: 10.0,
        position_y: 20.0,
        angle_degrees: 0.0,
        mirrored: false,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        line_spacing: 1.0,
        stroke_width: 0.0,
        max_error: 2.0,
        fake_bold: false,
        fake_italic: false,
    }
}

#[test]
fn multiline_and_tabs_follow_kicad_block_metrics() {
    let single_shaping = base_shaping("S");
    let single = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&single_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();

    let multiline_shaping = base_shaping("S\nS");
    let multiline = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&multiline_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    assert_eq!(multiline.line_widths, vec![single.width, single.width]);
    assert_eq!(multiline.glyphs, single.glyphs * 2);
    assert_eq!(multiline.contours.len(), single.contours.len() * 2);
    assert_close(
        multiline.height,
        2.0 * 1.17 + 2.0 * KICAD_TEXT_INTERLINE_FACTOR,
    );

    let tabbed_shaping = base_shaping("S\tS");
    let tabbed = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&tabbed_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let tab_width = 2.0 * KICAD_TEXT_TAB_WIDTH_FACTOR;
    let expected_width =
        single.width + (tab_width - single.width.rem_euclid(tab_width)) + single.width;
    assert_close(tabbed.width, expected_width);
    assert_eq!(tabbed.glyphs, single.glyphs * 2);
}

#[test]
fn line_spacing_alignment_and_authored_origin_transform_compose() {
    let shaping = base_shaping("S\nSS");
    let baseline = layout_text_block_hinted_a0(
        FONT_BYTES,
        TextBlockLayoutRequest {
            line_spacing: 2.0,
            horizontal_alignment: TextHorizontalAlignment::Center,
            vertical_alignment: TextVerticalAlignment::Center,
            angle_degrees: 90.0,
            mirrored: true,
            ..request(&shaping)
        },
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    assert_eq!(baseline.line_widths.len(), 2);
    assert!(baseline.line_widths[1] > baseline.line_widths[0]);
    assert_close(
        baseline.height,
        2.0 * 1.17 + 2.0 * KICAD_TEXT_INTERLINE_FACTOR * 2.0,
    );
    assert!(
        baseline
            .contours
            .iter()
            .flat_map(|contour| &contour.points)
            .all(|point| point.x.is_finite() && point.y.is_finite())
    );
}

#[test]
fn aggregate_line_run_and_geometry_limits_are_inclusive_and_fail_closed() {
    let shaping = cff_shaping("C\nC");
    let baseline = layout_text_block_hinted_a0(
        CFF_FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let points = baseline
        .contours
        .iter()
        .map(|contour| contour.points.len())
        .sum::<usize>();
    let run_metadata_bytes = shaping.font_id.len()
        + shaping.font_sha256.0.len()
        + shaping.text_index_unit.len()
        + shaping.language.as_deref().map_or(0, str::len)
        + shaping.script.as_ref().map_or(0, |script| script.0.len())
        + shaping
            .features
            .iter()
            .map(|feature| feature.tag.0.len())
            .sum::<usize>()
        + shaping
            .variations
            .iter()
            .map(|variation| variation.axis.0.len())
            .sum::<usize>();
    let exact = TextBlockLayoutLimits {
        contours: TextContourLimits {
            shaping: kicad_monkey_core::TextShapingLimits {
                max_glyphs: baseline.glyphs,
                ..kicad_monkey_core::TextShapingLimits::default()
            },
            max_outline_commands: baseline.outline_commands,
            max_bezier_work_items: baseline.bezier_work_items,
            max_contours: baseline.contours.len(),
            max_points: points,
            ..TextContourLimits::default()
        },
        max_lines: 2,
        max_runs: 2,
        max_run_metadata_bytes: run_metadata_bytes * 2,
        max_feature_inspections: TextBlockLayoutLimits::default().max_feature_inspections,
        max_feature_applications: TextBlockLayoutLimits::default().max_feature_applications,
        max_markup_nodes: TextBlockLayoutLimits::default().max_markup_nodes,
    };
    layout_text_block_hinted_a0(CFF_FONT_BYTES, request(&shaping), exact)
        .expect("aggregate ceilings are inclusive");

    for limits in [
        TextBlockLayoutLimits {
            max_lines: 1,
            ..exact
        },
        TextBlockLayoutLimits {
            max_runs: 1,
            ..exact
        },
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                shaping: kicad_monkey_core::TextShapingLimits {
                    max_glyphs: baseline.glyphs - 1,
                    ..exact.contours.shaping
                },
                ..exact.contours
            },
            ..exact
        },
        TextBlockLayoutLimits {
            max_run_metadata_bytes: run_metadata_bytes * 2 - 1,
            ..exact
        },
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                max_points: points - 1,
                ..exact.contours
            },
            ..exact
        },
    ] {
        let error =
            layout_text_block_hinted_a0(CFF_FONT_BYTES, request(&shaping), limits).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    }
}

#[test]
fn aggregate_command_bezier_and_contour_limits_fail_one_under() {
    let shaping = cff_shaping("C\nC");
    let baseline = layout_text_block_hinted_a0(
        CFF_FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let exact = TextBlockLayoutLimits {
        contours: TextContourLimits {
            max_outline_commands: baseline.outline_commands,
            max_bezier_work_items: baseline.bezier_work_items,
            max_contours: baseline.contours.len(),
            ..TextContourLimits::default()
        },
        ..TextBlockLayoutLimits::default()
    };
    layout_text_block_hinted_a0(CFF_FONT_BYTES, request(&shaping), exact)
        .expect("aggregate command, Bezier, and contour ceilings are inclusive");

    for limits in [
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                max_outline_commands: baseline.outline_commands - 1,
                ..exact.contours
            },
            ..exact
        },
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                max_bezier_work_items: baseline.bezier_work_items - 1,
                ..exact.contours
            },
            ..exact
        },
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                max_contours: baseline.contours.len() - 1,
                ..exact.contours
            },
            ..exact
        },
    ] {
        let error =
            layout_text_block_hinted_a0(CFF_FONT_BYTES, request(&shaping), limits).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    }
}

#[test]
fn delimiter_limits_reject_before_processing_unbounded_tail() {
    let shaping = base_shaping(&format!("S{}", "\n".repeat(100_000)));
    let error = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits {
            max_lines: 2,
            ..TextBlockLayoutLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    assert_eq!(error.path, "$.lines");
}

#[test]
fn aggregate_feature_work_includes_metadata_and_each_utf8_run() {
    let mut shaping = base_shaping("é\tS");
    shaping.features = vec![
        ShapingFeature {
            end: u32::MAX,
            start: 0,
            tag: OpenTypeTag("kern".to_owned()),
            value: 1,
        },
        ShapingFeature {
            end: 2,
            start: 0,
            tag: OpenTypeTag("liga".to_owned()),
            value: 0,
        },
        ShapingFeature {
            end: 4,
            start: 3,
            tag: OpenTypeTag("calt".to_owned()),
            value: 0,
        },
    ];
    let exact = TextBlockLayoutLimits {
        max_feature_inspections: 9,
        max_feature_applications: 4,
        ..TextBlockLayoutLimits::default()
    };
    layout_text_block_hinted_a0(FONT_BYTES, request(&shaping), exact)
        .expect("metadata plus two three-feature run scans are exact");

    for limits in [
        TextBlockLayoutLimits {
            max_feature_inspections: 8,
            ..exact
        },
        TextBlockLayoutLimits {
            max_feature_applications: 3,
            ..exact
        },
    ] {
        let error = layout_text_block_hinted_a0(FONT_BYTES, request(&shaping), limits).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
        assert!(error.path.starts_with("$.features."));
    }
}

#[test]
fn block_cache_composition_preserves_text_and_composes_output_limits() {
    let shaping = base_shaping("S\nS");
    let cache = generate_text_render_cache_block_hinted_a0(
        FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits::default(),
        TextRenderCacheLimits {
            max_text_bytes: shaping.text.len(),
            ..TextRenderCacheLimits::default()
        },
    )
    .unwrap();
    assert_eq!(cache.text, shaping.text);
    assert!(!cache.polygons.is_empty());

    let error = generate_text_render_cache_block_hinted_a0(
        FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits::default(),
        TextRenderCacheLimits {
            max_text_bytes: shaping.text.len() - 1,
            ..TextRenderCacheLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextRenderCacheErrorKind::ResourceLimit);
    assert_eq!(error.path, "$.text");
}

#[test]
fn markup_markers_are_consumed_and_styled_runs_shrink() {
    let plain_shaping = base_shaping("VIN");
    let plain = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&plain_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();

    let subscript_shaping = base_shaping("V_{IN}");
    let subscript = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&subscript_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    // The marker and braces shape no glyphs of their own.
    assert_eq!(subscript.glyphs, plain.glyphs);
    // The styled run advances on the 917-unit face, so the line narrows.
    assert!(subscript.width < plain.width);
    assert!(subscript.width > plain.width * 0.5);
}

#[test]
fn subscript_and_superscript_offsets_match_reference_ratios_exactly() {
    let subscript_shaping = base_shaping("_{S}");
    let subscript = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&subscript_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let superscript_shaping = base_shaping("^{S}");
    let superscript = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&superscript_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    // Both styles reuse the same styled outline, so every point differs by
    // exactly the two baseline offsets on the shared 917-unit face.
    let internal_to_y = 2.0 / KICAD_OUTLINE_FACE_SCALER * KICAD_OUTLINE_SIZE_COMPENSATION;
    let expected_delta = (KICAD_SUPERSCRIPT_VERTICAL_OFFSET_RATIO
        - KICAD_SUBSCRIPT_VERTICAL_OFFSET_RATIO)
        * KICAD_SUBSCRIPT_FACE_SCALER
        * internal_to_y;
    assert_eq!(subscript.contours.len(), superscript.contours.len());
    assert!(!subscript.contours.is_empty());
    for (subscript_contour, superscript_contour) in
        subscript.contours.iter().zip(&superscript.contours)
    {
        assert_eq!(
            subscript_contour.points.len(),
            superscript_contour.points.len()
        );
        for (subscript_point, superscript_point) in subscript_contour
            .points
            .iter()
            .zip(&superscript_contour.points)
        {
            assert_close(subscript_point.x, superscript_point.x);
            assert_close(subscript_point.y - superscript_point.y, expected_delta);
        }
    }
}

#[test]
fn nested_superscript_markers_keep_subscript_precedence() {
    // `_{^{S}}` keeps the subscript flag set, so the nested superscript marker
    // must not lift the run back up.
    let nested_shaping = base_shaping("_{^{S}}");
    let nested = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&nested_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let subscript_shaping = base_shaping("_{S}");
    let subscript = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&subscript_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    assert_eq!(nested.contours.len(), subscript.contours.len());
    for (nested_contour, subscript_contour) in nested.contours.iter().zip(&subscript.contours) {
        for (nested_point, subscript_point) in
            nested_contour.points.iter().zip(&subscript_contour.points)
        {
            assert_close(nested_point.x, subscript_point.x);
            assert_close(nested_point.y, subscript_point.y);
        }
    }
}

#[test]
fn overbar_appends_reference_oval_polygon_after_children() {
    let overbar_shaping = base_shaping("~{S}");
    let stroked_request = TextBlockLayoutRequest {
        stroke_width: 0.3,
        ..request(&overbar_shaping)
    };
    let with_bar = layout_text_block_hinted_a0(
        FONT_BYTES,
        stroked_request,
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let plain_shaping = base_shaping("S");
    let plain = layout_text_block_hinted_a0(
        FONT_BYTES,
        TextBlockLayoutRequest {
            stroke_width: 0.3,
            ..request(&plain_shaping)
        },
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    // The bar advances nothing and is appended after the group's children.
    assert_close(with_bar.width, plain.width);
    assert_eq!(with_bar.contours.len(), plain.contours.len() + 1);
    let bar = with_bar.contours.last().unwrap();
    // strokeWidth / 180 error yields KiCad's constant 24-segment oval.
    assert_eq!(bar.points.len(), 28);
    let radius = 0.3 / 2.0;
    let trim = 2.0 * 0.1;
    let bar_y = 20.0 + 2.0 - 2.0 * KICAD_TEXT_OVERBAR_HEIGHT_RATIO;
    let min_x = bar
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::MAX, f64::min);
    let max_x = bar
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::MIN, f64::max);
    let min_y = bar
        .points
        .iter()
        .map(|point| point.y)
        .fold(f64::MAX, f64::min);
    let max_y = bar
        .points
        .iter()
        .map(|point| point.y)
        .fold(f64::MIN, f64::max);
    // With 24 segments the cap vertices closest to the axis sit at 82.5°.
    let cap_reach = radius * 82.5_f64.to_radians().sin();
    assert_close(min_x, 10.0 + trim - cap_reach);
    assert_close(max_x, 10.0 + plain.width - trim + cap_reach);
    assert_close(min_y, bar_y - radius);
    assert_close(max_y, bar_y + radius);

    // A non-positive stroke width suppresses the bar entirely.
    let without_bar = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&overbar_shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    assert_eq!(without_bar.contours.len(), plain.contours.len());
}

#[test]
fn fake_italic_applies_the_truncated_freetype_shear_to_glyph_points() {
    let shaping = base_shaping("S");
    let plain = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let italic = layout_text_block_hinted_a0(
        FONT_BYTES,
        TextBlockLayoutRequest {
            fake_italic: true,
            ..request(&shaping)
        },
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    // The shear runs after shaping, so advances and block metrics are equal.
    assert_eq!(italic.width, plain.width);
    assert_eq!(italic.height, plain.height);
    assert_eq!(italic.contours.len(), plain.contours.len());

    // `FT_Set_Transform` with `yx = 0`, `yy = 0x10000` leaves y untouched, so
    // curve flattening subdivides identically and points pair up one-to-one.
    let plain_points: Vec<_> = plain
        .contours
        .iter()
        .flat_map(|contour| &contour.points)
        .collect();
    let italic_points: Vec<_> = italic
        .contours
        .iter()
        .flat_map(|contour| &contour.points)
        .collect();
    assert_eq!(plain_points.len(), italic_points.len());
    for (plain_point, italic_point) in plain_points.iter().zip(&italic_points) {
        assert_eq!(italic_point.y, plain_point.y);
    }

    // Between any two points of the same glyph the shear displacement obeys
    // `x' = x cos 12 + y sin 12` up to per-point 26.6 rounding: final y grows
    // downward, so higher points (smaller y) lean further right.
    let angle = 12.0_f64.to_radians();
    let (sin_tilt, cos_tilt) = angle.sin_cos();
    let base_plain = plain_points[0];
    let base_shift = italic_points[0].x - base_plain.x;
    // One 26.6 step per rounded term, in millimetres.
    let rounding = 2.0 / 1433.0 * 1.4 / 16.0 * 2.0;
    for (plain_point, italic_point) in plain_points.iter().zip(&italic_points) {
        let shift = italic_point.x - plain_point.x;
        let expected = base_shift + (cos_tilt - 1.0) * (plain_point.x - base_plain.x)
            - sin_tilt * (plain_point.y - base_plain.y);
        assert!(
            (shift - expected).abs() <= rounding,
            "shear delta {shift} != {expected} at ({}, {})",
            plain_point.x,
            plain_point.y
        );
    }
    assert!(
        plain_points
            .iter()
            .zip(&italic_points)
            .any(|(plain_point, italic_point)| italic_point.x != plain_point.x),
        "the shear must move at least one point"
    );
}

#[test]
fn fake_bold_grows_the_glyph_by_the_freetype_embolden_strength() {
    let shaping = base_shaping("S");
    let plain = layout_text_block_hinted_a0(
        FONT_BYTES,
        request(&shaping),
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    let bold = layout_text_block_hinted_a0(
        FONT_BYTES,
        TextBlockLayoutRequest {
            fake_bold: true,
            ..request(&shaping)
        },
        TextBlockLayoutLimits::default(),
    )
    .unwrap();
    // Embolden does not touch advances, so block metrics stay equal.
    assert_eq!(bold.width, plain.width);
    assert_eq!(bold.contours.len(), plain.contours.len());

    let bounds = |contours: &[kicad_monkey_core::TextContour]| {
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for point in contours.iter().flat_map(|contour| &contour.points) {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
        (min_x, max_x, min_y, max_y)
    };
    let (plain_min_x, plain_max_x, plain_min_y, plain_max_y) = bounds(&plain.contours);
    let (bold_min_x, bold_max_x, bold_min_y, bold_max_y) = bounds(&bold.contours);
    // `FT_Outline_Embolden(outline, 1 << 6)` grows the outline by about one
    // hinted pixel (4 internal units) toward +x and glyph-up (-y here).
    let pixel = 4.0 * 2.0 / 1433.0 * 1.4;
    let growth_x = (bold_max_x - bold_min_x) - (plain_max_x - plain_min_x);
    let growth_y = (bold_max_y - bold_min_y) - (plain_max_y - plain_min_y);
    assert!(
        growth_x > 0.25 * pixel && growth_x < 3.0 * pixel,
        "x growth {growth_x} outside the embolden band around {pixel}"
    );
    assert!(
        growth_y > 0.25 * pixel && growth_y < 3.0 * pixel,
        "y growth {growth_y} outside the embolden band around {pixel}"
    );
    assert!(bold_min_y < plain_min_y, "bold must grow glyph-up");
    assert!(bold_max_x > plain_max_x, "bold must grow to the right");
}

#[test]
fn markup_node_and_overbar_geometry_ceilings_are_inclusive_and_fail_closed() {
    let shaping = base_shaping("~{S}\n_{S}");
    let stroked = |limits| {
        layout_text_block_hinted_a0(
            FONT_BYTES,
            TextBlockLayoutRequest {
                stroke_width: 0.3,
                ..request(&shaping)
            },
            limits,
        )
    };
    let baseline = stroked(TextBlockLayoutLimits::default()).unwrap();
    let points = baseline
        .contours
        .iter()
        .map(|contour| contour.points.len())
        .sum::<usize>();
    // Each line retains one group node plus one inner text node.
    let exact = TextBlockLayoutLimits {
        contours: TextContourLimits {
            max_contours: baseline.contours.len(),
            max_points: points,
            ..TextContourLimits::default()
        },
        max_markup_nodes: 4,
        ..TextBlockLayoutLimits::default()
    };
    stroked(exact).expect("markup and overbar geometry ceilings are inclusive");

    for limits in [
        TextBlockLayoutLimits {
            max_markup_nodes: 3,
            ..exact
        },
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                max_contours: baseline.contours.len() - 1,
                ..exact.contours
            },
            ..exact
        },
        TextBlockLayoutLimits {
            contours: TextContourLimits {
                max_points: points - 1,
                ..exact.contours
            },
            ..exact
        },
    ] {
        let error = stroked(limits).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-9, "{actual} != {expected}");
}

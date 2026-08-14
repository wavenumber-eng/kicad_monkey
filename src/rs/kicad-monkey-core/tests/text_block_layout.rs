use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    KICAD_TEXT_INTERLINE_FACTOR, KICAD_TEXT_TAB_WIDTH_FACTOR, TextBlockLayoutLimits,
    TextBlockLayoutRequest, TextContourErrorKind, TextContourLimits, TextHorizontalAlignment,
    TextRenderCacheErrorKind, TextRenderCacheLimits, TextVerticalAlignment,
    generate_text_render_cache_block_hinted_a0, layout_text_block_hinted_a0,
};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
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
        max_error: 2.0,
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
    let shaping = base_shaping("S\nS");
    let baseline = layout_text_block_hinted_a0(
        FONT_BYTES,
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
    };
    layout_text_block_hinted_a0(FONT_BYTES, request(&shaping), exact)
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
        let error = layout_text_block_hinted_a0(FONT_BYTES, request(&shaping), limits).unwrap_err();
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

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-9, "{actual} != {expected}");
}

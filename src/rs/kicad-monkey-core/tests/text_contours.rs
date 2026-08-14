use kicad_monkey_contracts::FiniteFloat;
use kicad_monkey_contracts::generated::shaping_record::{
    FontVariationCoordinate, OpenTypeTag, ShapingInput,
};
use kicad_monkey_core::{
    FontOutlineFace, FontOutlineFaceRequest, FontOutlineLimits, TextContourErrorKind,
    TextContourLimits, TextContourOutput, TextContourRequest, shape_text_contours_a0,
};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));
const CURVE_FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-variable-fixture.ttf"
));
const CURVE_FONT_SHA256: &str = "03d6e00f5026b063e4e06e315ae97ed635f59c60fd405f805b9194016a310fb3";

#[derive(Deserialize)]
struct Vectors {
    oracle: Oracle,
    font: FontRecord,
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Oracle {
    shaping_engine: String,
    outline_engine: String,
    curve_engine: String,
    coordinate_space: String,
    hinting: String,
}

#[derive(Deserialize)]
struct FontRecord {
    font_id: String,
    font_sha256: String,
    face_index: u32,
    units_per_em: u16,
}

#[derive(Clone, Deserialize)]
struct Record {
    case_id: String,
    shaping: ShapingInput,
    size_x: f64,
    size_y: f64,
    origin_x: f64,
    origin_y: f64,
    max_error: f64,
    comparison: Comparison,
    contours: Vec<Vec<[f64; 2]>>,
    advance_x: f64,
    advance_y: f64,
}

#[derive(Clone, Deserialize)]
struct Comparison {
    absolute_tolerance: f64,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_contour_vectors.json"
    )))
    .unwrap()
}

fn render(record: &Record, limits: TextContourLimits) -> Result<TextContourOutput, String> {
    render_with_bytes(FONT_BYTES, record, limits)
}

fn render_with_bytes(
    font_bytes: &[u8],
    record: &Record,
    limits: TextContourLimits,
) -> Result<TextContourOutput, String> {
    shape_text_contours_a0(
        font_bytes,
        TextContourRequest {
            shaping: &record.shaping,
            size_x: record.size_x,
            size_y: record.size_y,
            origin_x: record.origin_x,
            origin_y: record.origin_y,
            max_error: record.max_error,
        },
        limits,
    )
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn native_contours_match_fixed_uharfbuzz_fonttools_kicad_records() {
    let vectors = vectors();
    assert_eq!(vectors.oracle.shaping_engine, "uharfbuzz");
    assert_eq!(vectors.oracle.outline_engine, "fontTools.BasePen");
    assert_eq!(
        vectors.oracle.curve_engine,
        "kicad_monkey.KiCadTextRenderer._bezier_get_poly"
    );
    assert_eq!(
        vectors.oracle.coordinate_space,
        "positioned_text_run_units_y_down"
    );
    assert_eq!(vectors.oracle.hinting, "none");
    assert_eq!(vectors.font.units_per_em, 1000);

    for record in vectors.records {
        let actual = render(&record, TextContourLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", record.case_id));
        assert_eq!(actual.units_per_em, vectors.font.units_per_em);
        assert_close(
            actual.advance_x,
            record.advance_x,
            record.comparison.absolute_tolerance,
            &format!("{}.advance_x", record.case_id),
        );
        assert_close(
            actual.advance_y,
            record.advance_y,
            record.comparison.absolute_tolerance,
            &format!("{}.advance_y", record.case_id),
        );
        assert_eq!(
            actual.contours.len(),
            record.contours.len(),
            "{} contour count",
            record.case_id
        );
        for (contour_index, (actual_contour, expected_contour)) in
            actual.contours.iter().zip(&record.contours).enumerate()
        {
            assert_eq!(
                actual_contour.points.len(),
                expected_contour.len(),
                "{} contour {contour_index} point count",
                record.case_id
            );
            for (point_index, (actual_point, expected_point)) in actual_contour
                .points
                .iter()
                .zip(expected_contour)
                .enumerate()
            {
                assert_close(
                    actual_point.x,
                    expected_point[0],
                    record.comparison.absolute_tolerance,
                    &format!(
                        "{}.contours[{contour_index}][{point_index}].x",
                        record.case_id
                    ),
                );
                assert_close(
                    actual_point.y,
                    expected_point[1],
                    record.comparison.absolute_tolerance,
                    &format!(
                        "{}.contours[{contour_index}][{point_index}].y",
                        record.case_id
                    ),
                );
            }
        }
    }
}

#[test]
fn reusable_face_extracts_multiple_glyphs_without_revalidating_the_buffer() {
    let vectors = vectors();
    let face = FontOutlineFace::new(
        FONT_BYTES,
        FontOutlineFaceRequest {
            font_id: &vectors.font.font_id,
            font_sha256: &vectors.font.font_sha256,
            face_index: vectors.font.face_index,
            variations: &[],
        },
        FontOutlineLimits::default(),
    )
    .unwrap();
    let glyph_a = face.extract_glyph(34).unwrap();
    let glyph_o = face.extract_glyph(48).unwrap();
    assert_eq!(face.units_per_em(), vectors.font.units_per_em);
    assert!(!glyph_a.commands.is_empty());
    assert!(!glyph_o.commands.is_empty());
}

#[test]
fn aggregate_contour_limits_are_inclusive_and_fail_closed_one_under() {
    let vectors = vectors();
    let record = vectors
        .records
        .iter()
        .find(|record| record.case_id == "kerning_pair_anisotropic_offset")
        .unwrap();
    let baseline = render(record, TextContourLimits::default()).unwrap();
    assert!(baseline.outline_commands > 0);
    let point_count = baseline
        .contours
        .iter()
        .map(|contour| contour.points.len())
        .sum::<usize>();
    let exact = TextContourLimits {
        max_outline_commands: baseline.outline_commands,
        max_contours: baseline.contours.len(),
        max_points: point_count,
        ..TextContourLimits::default()
    };
    let repeated = render(record, exact).unwrap();
    assert_eq!(repeated.outline_commands, baseline.outline_commands);
    assert_eq!(repeated.bezier_work_items, baseline.bezier_work_items);

    for one_under in [
        TextContourLimits {
            max_outline_commands: baseline.outline_commands - 1,
            ..TextContourLimits::default()
        },
        TextContourLimits {
            max_contours: baseline.contours.len() - 1,
            ..TextContourLimits::default()
        },
        TextContourLimits {
            max_points: point_count - 1,
            ..TextContourLimits::default()
        },
    ] {
        let error = shape_text_contours_a0(
            FONT_BYTES,
            TextContourRequest {
                shaping: &record.shaping,
                size_x: record.size_x,
                size_y: record.size_y,
                origin_x: record.origin_x,
                origin_y: record.origin_y,
                max_error: record.max_error,
            },
            one_under,
        )
        .unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    }
}

#[test]
fn curve_work_and_variation_preflight_are_independently_bounded() {
    let vectors = vectors();
    let mut curve_record = vectors.records[0].clone();
    curve_record.shaping.font_id = "outline_variable_fixture".parse().unwrap();
    curve_record.shaping.font_sha256.0 = CURVE_FONT_SHA256.to_owned();
    curve_record.shaping.variations = vec![FontVariationCoordinate {
        axis: OpenTypeTag("wght".to_owned()),
        value: FiniteFloat::try_from(700.0).unwrap(),
    }];
    let curved = render_with_bytes(
        CURVE_FONT_BYTES,
        &curve_record,
        TextContourLimits::default(),
    )
    .unwrap();
    assert!(curved.bezier_work_items > 0);
    assert!(curved.peak_temporary_bezier_points > 0);
    let exact_curve = render_with_bytes(
        CURVE_FONT_BYTES,
        &curve_record,
        TextContourLimits {
            max_bezier_work_items: curved.bezier_work_items,
            max_temporary_bezier_points: curved.peak_temporary_bezier_points,
            ..TextContourLimits::default()
        },
    )
    .unwrap();
    assert_eq!(exact_curve.bezier_work_items, curved.bezier_work_items);
    assert_eq!(
        exact_curve.peak_temporary_bezier_points,
        curved.peak_temporary_bezier_points
    );
    let error = shape_text_contours_a0(
        CURVE_FONT_BYTES,
        TextContourRequest {
            shaping: &curve_record.shaping,
            size_x: curve_record.size_x,
            size_y: curve_record.size_y,
            origin_x: curve_record.origin_x,
            origin_y: curve_record.origin_y,
            max_error: curve_record.max_error,
        },
        TextContourLimits {
            max_bezier_work_items: curved.bezier_work_items - 1,
            ..TextContourLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    let error = shape_text_contours_a0(
        CURVE_FONT_BYTES,
        TextContourRequest {
            shaping: &curve_record.shaping,
            size_x: curve_record.size_x,
            size_y: curve_record.size_y,
            origin_x: curve_record.origin_x,
            origin_y: curve_record.origin_y,
            max_error: curve_record.max_error,
        },
        TextContourLimits {
            max_temporary_bezier_points: curved.peak_temporary_bezier_points - 1,
            ..TextContourLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    let error = shape_text_contours_a0(
        CURVE_FONT_BYTES,
        TextContourRequest {
            shaping: &curve_record.shaping,
            size_x: curve_record.size_x,
            size_y: curve_record.size_y,
            origin_x: curve_record.origin_x,
            origin_y: curve_record.origin_y,
            max_error: curve_record.max_error,
        },
        TextContourLimits {
            outline: FontOutlineLimits {
                max_variations: 0,
                ..FontOutlineLimits::default()
            },
            ..TextContourLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
}

#[test]
fn invalid_geometry_inputs_fail_before_font_work() {
    let vectors = vectors();
    let record = &vectors.records[0];
    let error = shape_text_contours_a0(
        FONT_BYTES,
        TextContourRequest {
            shaping: &record.shaping,
            size_x: f64::INFINITY,
            size_y: record.size_y,
            origin_x: record.origin_x,
            origin_y: record.origin_y,
            max_error: record.max_error,
        },
        TextContourLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::InvalidInput);
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, path: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{path}: {actual} != {expected} (tolerance {tolerance})"
    );
}

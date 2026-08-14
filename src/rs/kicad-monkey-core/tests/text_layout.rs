use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    TextContourErrorKind, TextContourLimits, TextHorizontalAlignment, TextLayoutRequest, TextPoint,
    TextVerticalAlignment, layout_single_line_text_a0,
};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));

#[derive(Deserialize)]
struct Vectors {
    oracle: Oracle,
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    transform_order: Vec<String>,
    rotation_origin: String,
    height_fudge_factor: f64,
}

#[derive(Deserialize)]
struct Record {
    case_id: String,
    shaping: ShapingInput,
    size_x: f64,
    size_y: f64,
    position_x: f64,
    position_y: f64,
    angle_degrees: f64,
    mirrored: bool,
    horizontal_alignment: String,
    vertical_alignment: String,
    max_error: f64,
    comparison: Comparison,
    contours: Vec<Vec<[f64; 2]>>,
    advance_x: f64,
    advance_y: f64,
}

#[derive(Deserialize)]
struct Comparison {
    absolute_tolerance: f64,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_layout_vectors.json"
    )))
    .unwrap()
}

fn horizontal(value: &str) -> TextHorizontalAlignment {
    match value {
        "left" => TextHorizontalAlignment::Left,
        "center" => TextHorizontalAlignment::Center,
        "right" => TextHorizontalAlignment::Right,
        other => panic!("unknown horizontal alignment: {other}"),
    }
}

fn vertical(value: &str) -> TextVerticalAlignment {
    match value {
        "top" => TextVerticalAlignment::Top,
        "center" => TextVerticalAlignment::Center,
        "bottom" => TextVerticalAlignment::Bottom,
        other => panic!("unknown vertical alignment: {other}"),
    }
}

fn request(record: &Record) -> TextLayoutRequest<'_> {
    TextLayoutRequest {
        shaping: &record.shaping,
        size_x: record.size_x,
        size_y: record.size_y,
        position_x: record.position_x,
        position_y: record.position_y,
        angle_degrees: record.angle_degrees,
        mirrored: record.mirrored,
        horizontal_alignment: horizontal(&record.horizontal_alignment),
        vertical_alignment: vertical(&record.vertical_alignment),
        max_error: record.max_error,
    }
}

#[test]
fn native_single_line_layout_matches_python_alignment_and_transform_records() {
    let vectors = vectors();
    assert_eq!(
        vectors.oracle.implementation,
        "kicad_monkey.KiCadTextRenderer alignment helpers"
    );
    assert_eq!(
        vectors.oracle.transform_order,
        ["alignment", "mirror_x", "clockwise_rotation"]
    );
    assert_eq!(vectors.oracle.rotation_origin, "authored_text_position");
    assert_eq!(vectors.oracle.height_fudge_factor, 1.17);
    for record in vectors.records {
        let output =
            layout_single_line_text_a0(FONT_BYTES, request(&record), TextContourLimits::default())
                .unwrap_or_else(|error| panic!("{}: {error:?}", record.case_id));
        assert_close(
            output.advance_x,
            record.advance_x,
            record.comparison.absolute_tolerance,
            &format!("{}.advance_x", record.case_id),
        );
        assert_close(
            output.advance_y,
            record.advance_y,
            record.comparison.absolute_tolerance,
            &format!("{}.advance_y", record.case_id),
        );
        assert_contours(
            &output
                .contours
                .iter()
                .map(|contour| contour.points.clone())
                .collect::<Vec<_>>(),
            &record.contours,
            record.comparison.absolute_tolerance,
            &record.case_id,
        );
    }
}

#[test]
fn layout_reuses_contour_limits_and_rejects_nonfinite_transforms() {
    let vectors = vectors();
    let record = &vectors.records[1];
    let baseline =
        layout_single_line_text_a0(FONT_BYTES, request(record), TextContourLimits::default())
            .unwrap();
    let point_count = baseline
        .contours
        .iter()
        .map(|contour| contour.points.len())
        .sum::<usize>();
    layout_single_line_text_a0(
        FONT_BYTES,
        request(record),
        TextContourLimits {
            max_points: point_count,
            ..TextContourLimits::default()
        },
    )
    .expect("retained point ceiling is inclusive through transforms");
    let error = layout_single_line_text_a0(
        FONT_BYTES,
        TextLayoutRequest {
            angle_degrees: f64::NAN,
            ..request(record)
        },
        TextContourLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::InvalidInput);
    assert_eq!(error.path, "$.transform");
}

fn assert_contours(
    actual: &[Vec<TextPoint>],
    expected: &[Vec<[f64; 2]>],
    tolerance: f64,
    case_id: &str,
) {
    assert_eq!(actual.len(), expected.len(), "{case_id} contour count");
    for (contour_index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual.len(),
            expected.len(),
            "{case_id} contour {contour_index} point count"
        );
        for (point_index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_close(
                actual.x,
                expected[0],
                tolerance,
                &format!("{case_id}.contours[{contour_index}][{point_index}].x"),
            );
            assert_close(
                actual.y,
                expected[1],
                tolerance,
                &format!("{case_id}.contours[{contour_index}][{point_index}].y"),
            );
        }
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, path: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{path}: {actual} != {expected} (tolerance {tolerance})"
    );
}

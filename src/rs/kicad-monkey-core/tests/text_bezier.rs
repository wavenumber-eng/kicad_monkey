use kicad_monkey_core::{
    TextBezierErrorKind, TextBezierLimits, TextBezierOutput, TextPoint, flatten_cubic_bezier,
    flatten_quadratic_bezier,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Vectors {
    oracle: Oracle,
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Oracle {
    implementation: String,
    kicad_revision: String,
    kicad_source_algorithm: String,
    kicad_text_integration: String,
    coordinate_space: String,
}

#[derive(Clone, Deserialize)]
struct Record {
    case_id: String,
    kind: String,
    control: Vec<[f64; 2]>,
    max_error: f64,
    comparison: Comparison,
    points: Vec<[f64; 2]>,
}

#[derive(Clone, Copy, Deserialize)]
struct Comparison {
    absolute_tolerance: f64,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_bezier_vectors.json"
    )))
    .unwrap()
}

fn flatten(record: &Record, limits: TextBezierLimits) -> Result<TextBezierOutput, String> {
    let points = record
        .control
        .iter()
        .map(|point| TextPoint {
            x: point[0],
            y: point[1],
        })
        .collect::<Vec<_>>();
    let output = match record.kind.as_str() {
        "quadratic" => flatten_quadratic_bezier(
            points.try_into().map_err(|_| "quadratic control count")?,
            record.max_error,
            limits,
        ),
        "cubic" => flatten_cubic_bezier(
            points.try_into().map_err(|_| "cubic control count")?,
            record.max_error,
            limits,
        ),
        other => return Err(format!("unknown curve kind: {other}")),
    };
    output.map_err(|error| format!("{error:?}"))
}

#[test]
fn native_decomposition_matches_fixed_python_kicad_records() {
    let vectors = vectors();
    assert_eq!(
        vectors.oracle.implementation,
        "kicad_monkey.KiCadTextRenderer._bezier_get_poly"
    );
    assert_eq!(
        vectors.oracle.kicad_source_algorithm,
        "libs/kimath/src/bezier_curves.cpp"
    );
    assert_eq!(
        vectors.oracle.kicad_text_integration,
        "common/font/outline_decomposer.cpp"
    );
    assert_eq!(
        vectors.oracle.kicad_revision,
        "5f555f4d63b970e410d567d1f79e05e8ce41b9d8"
    );
    assert_eq!(vectors.oracle.coordinate_space, "caller_units_f64");
    for record in vectors.records {
        let actual = flatten(&record, TextBezierLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", record.case_id));
        assert_eq!(
            actual.points.len(),
            record.points.len(),
            "{}",
            record.case_id
        );
        for (index, (actual, expected)) in actual.points.iter().zip(&record.points).enumerate() {
            assert_close(
                actual.x,
                expected[0],
                record.comparison.absolute_tolerance,
                &format!("{}[{index}].x", record.case_id),
            );
            assert_close(
                actual.y,
                expected[1],
                record.comparison.absolute_tolerance,
                &format!("{}[{index}].y", record.case_id),
            );
        }
    }
}

#[test]
fn output_and_work_limits_are_independently_inclusive() {
    for record in vectors().records {
        let baseline = flatten(&record, TextBezierLimits::default()).unwrap();
        let exact = TextBezierLimits {
            max_points: baseline.points.len(),
            max_work_items: baseline.work_items,
        };
        let repeated = flatten(&record, exact)
            .unwrap_or_else(|error| panic!("{} exact limits: {error}", record.case_id));
        assert_eq!(repeated.points.len(), baseline.points.len());
        assert_eq!(repeated.work_items, baseline.work_items);

        if !baseline.points.is_empty() {
            let one_under_points = TextBezierLimits {
                max_points: baseline.points.len() - 1,
                ..TextBezierLimits::default()
            };
            assert!(
                flatten(&record, one_under_points).is_err(),
                "{} points",
                record.case_id
            );
        }
        if baseline.work_items > 0 {
            let one_under_work = TextBezierLimits {
                max_work_items: baseline.work_items - 1,
                ..TextBezierLimits::default()
            };
            assert!(
                flatten(&record, one_under_work).is_err(),
                "{} work",
                record.case_id
            );
        }
    }
}

#[test]
fn nonfinite_inputs_and_adversarial_curve_work_fail_closed() {
    let invalid = flatten_quadratic_bezier(
        [
            TextPoint { x: 0.0, y: 0.0 },
            TextPoint {
                x: f64::NAN,
                y: 1.0,
            },
            TextPoint { x: 2.0, y: 0.0 },
        ],
        2.0,
        TextBezierLimits::default(),
    )
    .unwrap_err();
    assert_eq!(invalid.kind, TextBezierErrorKind::InvalidInput);

    let invalid_tolerance = flatten_cubic_bezier(
        [TextPoint { x: 0.0, y: 0.0 }; 4],
        f64::INFINITY,
        TextBezierLimits::default(),
    )
    .unwrap_err();
    assert_eq!(invalid_tolerance.kind, TextBezierErrorKind::InvalidInput);

    let limited = flatten_quadratic_bezier(
        [
            TextPoint { x: 0.0, y: 0.0 },
            TextPoint {
                x: 1.0e12,
                y: 1.0e12,
            },
            TextPoint { x: 1.0, y: 0.0 },
        ],
        1.0e-12,
        TextBezierLimits {
            max_points: 8,
            max_work_items: 8,
        },
    )
    .unwrap_err();
    assert_eq!(limited.kind, TextBezierErrorKind::ResourceLimit);

    let cubic_limited = flatten_cubic_bezier(
        [
            TextPoint { x: 0.0, y: 0.0 },
            TextPoint {
                x: 1.0e9,
                y: -1.0e12,
            },
            TextPoint {
                x: -1.0e9,
                y: 1.0e12,
            },
            TextPoint { x: 1.0, y: 0.0 },
        ],
        1.0e-12,
        TextBezierLimits {
            max_points: 64,
            max_work_items: 8,
        },
    )
    .unwrap_err();
    assert_eq!(cubic_limited.kind, TextBezierErrorKind::ResourceLimit);
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, path: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{path}: {actual} != {expected} (tolerance {tolerance})"
    );
}

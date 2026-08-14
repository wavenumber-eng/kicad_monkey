use kicad_monkey_core::{
    TextContour, TextContourErrorKind, TextPoint, TextTopologyLimits, fracture_text_contours_a0,
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
    outline_source: String,
    semantics: Vec<String>,
}

#[derive(Deserialize)]
struct Record {
    case_id: String,
    contours: Vec<Vec<[f64; 2]>>,
    fractured: Vec<Vec<[f64; 2]>>,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_topology_vectors.json"
    )))
    .unwrap()
}

fn contours(record: &Record) -> Vec<TextContour> {
    record
        .contours
        .iter()
        .map(|contour| TextContour {
            points: contour
                .iter()
                .map(|point| TextPoint {
                    x: point[0],
                    y: point[1],
                })
                .collect(),
        })
        .collect()
}

#[test]
fn native_topology_matches_fixed_python_fracture_records() {
    let vectors = vectors();
    assert_eq!(
        vectors.oracle.implementation,
        "kicad_monkey.kicad_render_cache private topology helpers"
    );
    assert_eq!(
        vectors.oracle.outline_source,
        "tests/parity/text_contour_vectors.json"
    );
    assert_eq!(
        vectors.oracle.semantics,
        [
            "trim_one_duplicate_closure",
            "first_containing_exterior",
            "leftmost_hole_bridge",
        ]
    );
    for record in vectors.records {
        let output = fracture_text_contours_a0(&contours(&record), TextTopologyLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error:?}", record.case_id));
        let actual = output
            .contours
            .iter()
            .map(|contour| {
                contour
                    .points
                    .iter()
                    .map(|point| [point.x, point.y])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, record.fractured, "{}", record.case_id);
    }
}

#[test]
fn every_topology_limit_is_inclusive_and_fails_closed_one_under() {
    let vectors = vectors();
    let record = vectors
        .records
        .iter()
        .find(|record| record.case_id == "disjoint_exteriors_with_holes")
        .unwrap();
    let input = contours(record);
    let baseline = fracture_text_contours_a0(&input, TextTopologyLimits::default()).unwrap();
    let exact = TextTopologyLimits {
        max_input_contours: input.len(),
        max_input_points: baseline.input_points,
        max_output_polygons: baseline.contours.len(),
        max_edge_slots: baseline.edge_slots,
        max_output_points: baseline.output_points,
        max_containment_edge_checks: baseline.containment_edge_checks,
        max_bridge_edge_checks: baseline.bridge_edge_checks,
        max_link_steps: baseline.link_steps,
    };
    fracture_text_contours_a0(&input, exact).expect("all ceilings are inclusive");

    let cases = [
        (
            TextTopologyLimits {
                max_input_contours: exact.max_input_contours - 1,
                ..exact
            },
            "$.contours",
        ),
        (
            TextTopologyLimits {
                max_input_points: exact.max_input_points - 1,
                ..exact
            },
            "$.contours[*].points",
        ),
        (
            TextTopologyLimits {
                max_output_polygons: exact.max_output_polygons - 1,
                ..exact
            },
            "$.polygons",
        ),
        (
            TextTopologyLimits {
                max_edge_slots: exact.max_edge_slots - 1,
                ..exact
            },
            "$.edges",
        ),
        (
            TextTopologyLimits {
                max_output_points: exact.max_output_points - 1,
                ..exact
            },
            "$.output.points",
        ),
        (
            TextTopologyLimits {
                max_containment_edge_checks: exact.max_containment_edge_checks - 1,
                ..exact
            },
            "$.work.containment_edge_checks",
        ),
        (
            TextTopologyLimits {
                max_bridge_edge_checks: exact.max_bridge_edge_checks - 1,
                ..exact
            },
            "$.work.bridge_edge_checks",
        ),
        (
            TextTopologyLimits {
                max_link_steps: exact.max_link_steps - 1,
                ..exact
            },
            "$.work.link_steps",
        ),
    ];
    for (limits, path) in cases {
        let error = fracture_text_contours_a0(&input, limits).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit, "{path}");
        assert_eq!(error.path, path);
    }
}

#[test]
fn simplify_seam_picks_smallest_x_min_y_run_and_scans_hole_bridge_from_seam() {
    // The exterior carries TWO min-y runs at y=0; the seam sits one past the
    // exit of the run holding the smallest-x min-y point, so the fractured
    // ring starts at (1.5, 1) and not after the (2,0)-(3,0) run. The hole's
    // stored order reaches its min-x UPPER vertex (1,2) first when scanned
    // from its own Simplify seam (one past (1.5,1.9)), pinning the
    // Wavenumber-style bridge that the old largest-y tie-break missed.
    // Expected output fixed against the Python reference fracture helpers.
    let input = [
        TextContour {
            points: [
                (0.0, 0.0),
                (1.0, 0.0),
                (1.5, 1.0),
                (2.0, 0.0),
                (3.0, 0.0),
                (3.0, 5.0),
                (0.0, 5.0),
            ]
            .map(|(x, y)| TextPoint { x, y })
            .to_vec(),
        },
        TextContour {
            points: [(1.0, 2.0), (1.0, 3.0), (2.0, 3.0), (2.0, 2.0), (1.5, 1.9)]
                .map(|(x, y)| TextPoint { x, y })
                .to_vec(),
        },
    ];
    let output = fracture_text_contours_a0(&input, TextTopologyLimits::default()).unwrap();
    let actual: Vec<(f64, f64)> = output.contours[0]
        .points
        .iter()
        .map(|point| (point.x, point.y))
        .collect();
    assert_eq!(output.contours.len(), 1);
    assert_eq!(
        actual,
        [
            (1.5, 1.0),
            (2.0, 0.0),
            (3.0, 0.0),
            (3.0, 5.0),
            (0.0, 5.0),
            (0.0, 2.0),
            (1.0, 2.0),
            (1.0, 3.0),
            (2.0, 3.0),
            (2.0, 2.0),
            (1.5, 1.9),
            (1.0, 2.0),
            (0.0, 2.0),
            (0.0, 0.0),
            (1.0, 0.0),
        ]
    );
}

#[test]
fn nonfinite_input_is_rejected_before_topology_publication() {
    let input = [TextContour {
        points: vec![
            TextPoint { x: 0.0, y: 0.0 },
            TextPoint {
                x: f64::NAN,
                y: 1.0,
            },
            TextPoint { x: 1.0, y: 0.0 },
        ],
    }];
    let error = fracture_text_contours_a0(&input, TextTopologyLimits::default()).unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::InvalidInput);
    assert_eq!(error.path, "$.contours[0].points");
}

#[test]
fn nonfinite_derived_intersections_fail_closed() {
    let input = [
        TextContour {
            points: vec![
                TextPoint {
                    x: -1.0e308,
                    y: -1.0,
                },
                TextPoint { x: 1.0e308, y: 1.0 },
                TextPoint { x: 1.0e308, y: 2.0 },
                TextPoint {
                    x: -1.0e308,
                    y: 2.0,
                },
            ],
        },
        TextContour {
            points: vec![
                TextPoint { x: 0.0, y: 0.0 },
                TextPoint { x: 0.5, y: 0.5 },
                TextPoint { x: -0.5, y: 0.5 },
            ],
        },
    ];
    let error = fracture_text_contours_a0(&input, TextTopologyLimits::default()).unwrap_err();
    assert_eq!(error.kind, TextContourErrorKind::InvalidInput);
    assert_eq!(error.path, "$.work.derived_coordinate");
}

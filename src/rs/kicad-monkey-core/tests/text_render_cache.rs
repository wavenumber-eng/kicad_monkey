use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    TextContour, TextHorizontalAlignment, TextLayoutRequest, TextPoint, TextRenderCache,
    TextRenderCacheErrorKind, TextRenderCacheLimits, TextRenderCachePolygon, TextVerticalAlignment,
    generate_text_render_cache_a0, read_text_render_cache_a0, write_text_render_cache_a0,
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
    serialization_probe: Probe,
}

#[derive(Deserialize)]
struct Oracle {
    kicad_revision: String,
    kicad_writer: String,
}

#[derive(Deserialize)]
struct Record {
    case_id: String,
    layout: LayoutRecord,
    polygons: Vec<Vec<[f64; 2]>>,
    sexpr: String,
}

#[derive(Deserialize)]
struct LayoutRecord {
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
}

#[derive(Deserialize)]
struct Comparison {
    absolute_tolerance: f64,
}

#[derive(Deserialize)]
struct Probe {
    text: String,
    angle_degrees: f64,
    polygons: Vec<Vec<Vec<[f64; 2]>>>,
    sexpr: String,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_render_cache_vectors.json"
    )))
    .unwrap()
}

fn request(record: &LayoutRecord) -> TextLayoutRequest<'_> {
    TextLayoutRequest {
        shaping: &record.shaping,
        size_x: record.size_x,
        size_y: record.size_y,
        position_x: record.position_x,
        position_y: record.position_y,
        angle_degrees: record.angle_degrees,
        mirrored: record.mirrored,
        horizontal_alignment: match record.horizontal_alignment.as_str() {
            "left" => TextHorizontalAlignment::Left,
            "center" => TextHorizontalAlignment::Center,
            "right" => TextHorizontalAlignment::Right,
            other => panic!("unknown horizontal alignment: {other}"),
        },
        vertical_alignment: match record.vertical_alignment.as_str() {
            "top" => TextVerticalAlignment::Top,
            "center" => TextVerticalAlignment::Center,
            "bottom" => TextVerticalAlignment::Bottom,
            other => panic!("unknown vertical alignment: {other}"),
        },
        max_error: record.max_error,
    }
}

fn probe_cache(probe: &Probe) -> TextRenderCache {
    TextRenderCache {
        text: probe.text.clone(),
        angle_degrees: probe.angle_degrees,
        polygons: probe
            .polygons
            .iter()
            .map(|polygon| TextRenderCachePolygon {
                contours: polygon
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
                    .collect(),
            })
            .collect(),
    }
}

#[test]
fn native_cache_composition_matches_fixed_python_records() {
    let vectors = vectors();
    assert_eq!(
        vectors.oracle.kicad_revision,
        "d6ff4c23641ee5236b7c9fac19eb6af1849294f5"
    );
    assert!(vectors.oracle.kicad_writer.ends_with("::formatRenderCache"));
    for record in vectors.records {
        let cache = generate_text_render_cache_a0(
            FONT_BYTES,
            request(&record.layout),
            TextRenderCacheLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error:?}", record.case_id));
        assert_eq!(cache.text, record.layout.shaping.text);
        assert_close(
            cache.angle_degrees,
            record.layout.angle_degrees,
            record.layout.comparison.absolute_tolerance,
        );
        assert_polygons(
            &cache,
            &record.polygons,
            record.layout.comparison.absolute_tolerance,
        );

        let parsed =
            read_text_render_cache_a0(record.sexpr.as_bytes(), TextRenderCacheLimits::default())
                .unwrap();
        assert_polygons(
            &parsed,
            &record.polygons,
            record.layout.comparison.absolute_tolerance,
        );
    }
}

#[test]
fn streaming_reader_and_bounded_writer_round_trip_python_probe_exactly() {
    let probe = vectors().serialization_probe;
    let expected = probe_cache(&probe);
    let parsed =
        read_text_render_cache_a0(probe.sexpr.as_bytes(), TextRenderCacheLimits::default())
            .unwrap();
    assert_eq!(parsed, expected);
    let written = write_text_render_cache_a0(&parsed, TextRenderCacheLimits::default()).unwrap();
    assert_eq!(written, probe.sexpr.as_bytes());
    assert_eq!(
        read_text_render_cache_a0(&written, TextRenderCacheLimits::default()).unwrap(),
        expected
    );
}

#[test]
fn cache_io_limits_are_inclusive_and_fail_closed_one_under() {
    let probe = vectors().serialization_probe;
    let cache = probe_cache(&probe);
    let contours = cache
        .polygons
        .iter()
        .map(|polygon| polygon.contours.len())
        .sum::<usize>();
    let points = cache
        .polygons
        .iter()
        .flat_map(|polygon| &polygon.contours)
        .map(|contour| contour.points.len())
        .sum::<usize>();
    let written = write_text_render_cache_a0(&cache, TextRenderCacheLimits::default()).unwrap();
    let exact = TextRenderCacheLimits {
        max_source_bytes: written.len(),
        max_text_bytes: cache.text.len(),
        max_polygons: cache.polygons.len(),
        max_contours: contours,
        max_points: points,
        max_output_bytes: written.len(),
        ..TextRenderCacheLimits::default()
    };
    assert_eq!(read_text_render_cache_a0(&written, exact).unwrap(), cache);
    assert_eq!(write_text_render_cache_a0(&cache, exact).unwrap(), written);

    let read_cases = [
        TextRenderCacheLimits {
            max_source_bytes: exact.max_source_bytes - 1,
            ..exact
        },
        TextRenderCacheLimits {
            max_text_bytes: exact.max_text_bytes - 1,
            ..exact
        },
        TextRenderCacheLimits {
            max_polygons: exact.max_polygons - 1,
            ..exact
        },
        TextRenderCacheLimits {
            max_contours: exact.max_contours - 1,
            ..exact
        },
        TextRenderCacheLimits {
            max_points: exact.max_points - 1,
            ..exact
        },
    ];
    for limits in read_cases {
        assert_eq!(
            read_text_render_cache_a0(&written, limits)
                .unwrap_err()
                .kind,
            TextRenderCacheErrorKind::ResourceLimit
        );
    }
    let error = write_text_render_cache_a0(
        &cache,
        TextRenderCacheLimits {
            max_output_bytes: exact.max_output_bytes - 1,
            ..exact
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextRenderCacheErrorKind::ResourceLimit);
}

#[test]
fn malformed_or_nonfinite_cache_input_is_terminal() {
    for source in [
        b"(wrong \"A\" 0)".as_slice(),
        b"(render_cache \"A\" NaN)".as_slice(),
        b"(render_cache \"A\" 0 (polygon (pts (xy 1))))".as_slice(),
        b"(render_cache \"A\" 0) trailing".as_slice(),
    ] {
        assert!(read_text_render_cache_a0(source, TextRenderCacheLimits::default()).is_err());
    }

    let vectors = vectors();
    let record = &vectors.records[0];
    let error = generate_text_render_cache_a0(
        FONT_BYTES,
        request(&record.layout),
        TextRenderCacheLimits {
            max_text_bytes: record.layout.shaping.text.len() - 1,
            ..TextRenderCacheLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, TextRenderCacheErrorKind::ResourceLimit);

    let mut cache = probe_cache(&vectors.serialization_probe);
    cache.polygons[0].contours[0].points[0].x = f64::INFINITY;
    let error = write_text_render_cache_a0(&cache, TextRenderCacheLimits::default()).unwrap_err();
    assert_eq!(error.kind, TextRenderCacheErrorKind::InvalidInput);
}

fn assert_polygons(cache: &TextRenderCache, expected: &[Vec<[f64; 2]>], tolerance: f64) {
    assert_eq!(cache.polygons.len(), expected.len());
    for (polygon, expected) in cache.polygons.iter().zip(expected) {
        assert_eq!(polygon.contours.len(), 1);
        assert_eq!(polygon.contours[0].points.len(), expected.len());
        for (point, expected) in polygon.contours[0].points.iter().zip(expected) {
            assert_close(point.x, expected[0], tolerance);
            assert_close(point.y, expected[1], tolerance);
        }
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected} (tolerance {tolerance})"
    );
}

use kicad_monkey_contracts::generated::outline_vector::{
    CoordinateComparisonPolicy, FontVariationCoordinate, OutlineCommand, OutlineVectorA0,
};
use kicad_monkey_contracts::validate_outline_vector_contract;
use kicad_monkey_core::{
    FONT_OUTLINE_ENGINE, FontOutlineErrorKind, FontOutlineLimits, FontOutlineRequest,
    extract_font_outline_a0,
};
use serde_json::Value;

const STROKE_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));
const VARIABLE_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-variable-fixture.ttf"
));
const CFF_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-cff-fixture.otf"
));
const CFF2_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-cff2-fixture.otf"
));
const COMPOSITE_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-composite-fixture.ttf"
));
const COLLECTION_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/outline-collection-fixture.ttc"
));

fn vectors() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/font_outline_a0_vectors.json"
    )))
    .expect("font outline vectors")
}

fn records() -> Vec<OutlineVectorA0> {
    vectors()["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|value| serde_json::from_value(value.clone()).expect("outline record"))
        .collect()
}

fn font_bytes(font_id: &str) -> &'static [u8] {
    match font_id {
        "kicad_stroke_regular" => STROKE_FONT,
        "outline_variable_fixture" => VARIABLE_FONT,
        "outline_cff_fixture" => CFF_FONT,
        "outline_cff2_fixture" => CFF2_FONT,
        "outline_composite_fixture" => COMPOSITE_FONT,
        "outline_collection_second_face" => COLLECTION_FONT,
        other => panic!("unknown fixture font: {other}"),
    }
}

fn request(record: &OutlineVectorA0) -> FontOutlineRequest<'_> {
    FontOutlineRequest {
        font_id: &record.font_id,
        font_sha256: &record.font_sha256,
        face_index: record.face_index,
        variations: &record.variations,
        glyph_id: record.glyph_id,
    }
}

#[test]
fn ttf_parser_matches_fixed_fonttools_records() {
    let vectors = vectors();
    assert_eq!(FONT_OUTLINE_ENGINE, "ttf-parser-0.25.1");
    assert_eq!(
        vectors["oracle"]["coordinate_space"],
        "unscaled_font_design_units"
    );
    for record in records() {
        validate_outline_vector_contract(&record).expect("valid oracle record");
        let output = extract_font_outline_a0(
            font_bytes(&record.font_id),
            request(&record),
            FontOutlineLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error:?}", record.case_id));
        assert_eq!(u32::from(output.units_per_em), record.units_per_em.get());
        compare_commands(
            &output.commands,
            &record.commands,
            &record.coordinate_comparison,
        );
    }
}

#[test]
fn corpus_covers_line_quadratic_variable_and_cubic_paths() {
    let records = records();
    let stroke = find(&records, "kicad_stroke_line_outline");
    assert!(has_kind(&stroke.commands, "line_to"));
    let default = find(&records, "variable_quadratic_default");
    let weighted = find(&records, "variable_quadratic_weight_700");
    assert!(has_kind(&default.commands, "quad_to"));
    assert_ne!(
        serde_json::to_value(&default.commands).unwrap(),
        serde_json::to_value(&weighted.commands).unwrap(),
        "the wght coordinate must alter the generated gvar outline"
    );
    assert!(has_kind(
        &find(&records, "cff_cubic_outline").commands,
        "curve_to"
    ));
    assert!(has_kind(
        &find(&records, "cff2_cubic_outline").commands,
        "curve_to"
    ));
    let composite = find(&records, "transformed_composite_glyf");
    assert!(has_kind(&composite.commands, "quad_to"));
    assert_ne!(
        coordinate(&composite.commands[0], "x"),
        50.0,
        "the composite transform must change its base glyph origin"
    );
    let collection = find(&records, "collection_second_face");
    assert_eq!(collection.face_index, 1);
    assert_eq!(
        collection.font_id.as_str(),
        "outline_collection_second_face"
    );
}

#[test]
fn resource_limits_are_inclusive_and_fail_closed() {
    let record = records().remove(0);
    let bytes = font_bytes(&record.font_id);
    let exact = FontOutlineLimits {
        max_font_bytes: bytes.len(),
        max_commands: record.commands.len(),
        ..FontOutlineLimits::default()
    };
    extract_font_outline_a0(bytes, request(&record), exact).expect("inclusive limits");
    let one_under_font = FontOutlineLimits {
        max_font_bytes: bytes.len() - 1,
        ..FontOutlineLimits::default()
    };
    assert_eq!(
        extract_font_outline_a0(bytes, request(&record), one_under_font)
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::ResourceLimit
    );
    let one_under_commands = FontOutlineLimits {
        max_commands: record.commands.len() - 1,
        ..FontOutlineLimits::default()
    };
    assert_eq!(
        extract_font_outline_a0(bytes, request(&record), one_under_commands)
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::ResourceLimit
    );
    let no_metadata = FontOutlineLimits {
        max_metadata_bytes: 0,
        ..FontOutlineLimits::default()
    };
    assert_eq!(
        extract_font_outline_a0(bytes, request(&record), no_metadata)
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::ResourceLimit
    );

    let variable = records()
        .into_iter()
        .find(|record| record.case_id.as_str() == "variable_quadratic_weight_700")
        .unwrap();
    let metadata_bytes = variable.font_id.len()
        + variable.font_sha256.len()
        + variable
            .variations
            .iter()
            .map(|variation| variation.axis.0.len())
            .sum::<usize>();
    let exact_metadata_and_variations = FontOutlineLimits {
        max_metadata_bytes: metadata_bytes,
        max_variations: variable.variations.len(),
        ..FontOutlineLimits::default()
    };
    extract_font_outline_a0(
        VARIABLE_FONT,
        request(&variable),
        exact_metadata_and_variations,
    )
    .expect("metadata and variation limits are inclusive");
    let one_under_metadata = FontOutlineLimits {
        max_metadata_bytes: metadata_bytes - 1,
        ..FontOutlineLimits::default()
    };
    assert_eq!(
        extract_font_outline_a0(VARIABLE_FONT, request(&variable), one_under_metadata)
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::ResourceLimit
    );
    let one_under_variations = FontOutlineLimits {
        max_variations: variable.variations.len() - 1,
        ..FontOutlineLimits::default()
    };
    assert_eq!(
        extract_font_outline_a0(VARIABLE_FONT, request(&variable), one_under_variations,)
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::ResourceLimit
    );
}

#[test]
fn hash_face_glyph_and_variation_failures_are_structured() {
    let records = records();
    let record = find(&records, "variable_quadratic_weight_700");
    let mut wrong_hash = request(record);
    wrong_hash.font_sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
    assert_eq!(
        extract_font_outline_a0(VARIABLE_FONT, wrong_hash, FontOutlineLimits::default())
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::HashMismatch
    );
    let mut wrong_face = request(record);
    wrong_face.face_index = 1;
    assert_eq!(
        extract_font_outline_a0(VARIABLE_FONT, wrong_face, FontOutlineLimits::default())
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::InvalidFont
    );
    let mut huge_glyph = request(record);
    huge_glyph.glyph_id = u32::MAX;
    assert_eq!(
        extract_font_outline_a0(VARIABLE_FONT, huge_glyph, FontOutlineLimits::default())
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::InvalidInput
    );
    let mut absent_glyph = request(record);
    absent_glyph.glyph_id = 0;
    assert_eq!(
        extract_font_outline_a0(VARIABLE_FONT, absent_glyph, FontOutlineLimits::default())
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::MissingOutline
    );

    let unknown_axis = [variation("wdth", 700.0)];
    let mut unknown = request(record);
    unknown.variations = &unknown_axis;
    let error =
        extract_font_outline_a0(VARIABLE_FONT, unknown, FontOutlineLimits::default()).unwrap_err();
    assert_eq!(error.kind, FontOutlineErrorKind::InvalidInput);
    assert_eq!(error.path, "$.variations[0].axis");

    let duplicate_axes = [variation("wght", 400.0), variation("wght", 700.0)];
    let mut duplicate = request(record);
    duplicate.variations = &duplicate_axes;
    let error = extract_font_outline_a0(VARIABLE_FONT, duplicate, FontOutlineLimits::default())
        .unwrap_err();
    assert_eq!(error.kind, FontOutlineErrorKind::InvalidContract);
    assert_eq!(error.path, "$.variations[1].axis");

    let static_record = find(&records, "kicad_stroke_line_outline");
    let static_variation = [variation("wght", 700.0)];
    let mut static_request = request(static_record);
    static_request.variations = &static_variation;
    assert_eq!(
        extract_font_outline_a0(STROKE_FONT, static_request, FontOutlineLimits::default())
            .unwrap_err()
            .kind,
        FontOutlineErrorKind::InvalidInput
    );
}

fn variation(axis: &str, value: f64) -> FontVariationCoordinate {
    FontVariationCoordinate {
        axis: axis.to_owned().into(),
        value: value.try_into().unwrap(),
    }
}

fn find<'a>(records: &'a [OutlineVectorA0], case_id: &str) -> &'a OutlineVectorA0 {
    records
        .iter()
        .find(|record| record.case_id.as_str() == case_id)
        .unwrap()
}

fn has_kind(commands: &[OutlineCommand], kind: &str) -> bool {
    serde_json::to_value(commands)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["kind"] == kind)
}

fn coordinate(command: &OutlineCommand, field: &str) -> f64 {
    serde_json::to_value(command).unwrap()[field]
        .as_f64()
        .unwrap()
}

fn compare_commands(
    actual: &[OutlineCommand],
    expected: &[OutlineCommand],
    policy: &CoordinateComparisonPolicy,
) {
    let actual = serde_json::to_value(actual).unwrap();
    let expected = serde_json::to_value(expected).unwrap();
    let tolerance = match policy {
        CoordinateComparisonPolicy::ExactComparisonPolicy(_) => 0.0,
        CoordinateComparisonPolicy::AbsoluteToleranceComparisonPolicy(value) => {
            value.absolute_tolerance.get()
        }
    };
    compare_json(&actual, &expected, tolerance, "$.commands");
}

fn compare_json(actual: &Value, expected: &Value, tolerance: f64, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let delta = (actual.as_f64().unwrap() - expected.as_f64().unwrap()).abs();
            assert!(
                delta <= tolerance,
                "{path}: {actual} != {expected} (tolerance {tolerance})"
            );
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare_json(actual, expected, tolerance, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} field count");
            for (key, expected) in expected {
                compare_json(&actual[key], expected, tolerance, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

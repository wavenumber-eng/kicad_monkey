use kicad_monkey_contracts::generated::shaping_record::{
    ShapedGlyph, ShapingInput, ShapingRecordA0,
};
use kicad_monkey_contracts::validate_shaping_record_contract;
use kicad_monkey_core::{
    TEXT_SHAPING_ENGINE, TextShapingErrorKind, TextShapingLimits, shape_text_a0,
};

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));
const VARIABLE_FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/shaping-variable-fixture.ttf"
));

fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/font_shaping_a0_vectors.json"
    )))
    .expect("font shaping vectors")
}

fn records() -> Vec<ShapingRecordA0> {
    vectors()["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|value| serde_json::from_value(value.clone()).expect("shaping record"))
        .collect()
}

fn font_bytes(font_id: &str) -> &'static [u8] {
    match font_id {
        "kicad_stroke_regular" => FONT_BYTES,
        "shaping_variable_fixture" => VARIABLE_FONT_BYTES,
        other => panic!("unknown fixture font: {other}"),
    }
}

#[test]
fn harfrust_matches_fixed_uharfbuzz_records_exactly() {
    let vectors = vectors();
    assert_eq!(TEXT_SHAPING_ENGINE, "harfrust-0.13.0-harfbuzz-14.3.0");
    assert_eq!(vectors["oracle"]["harfbuzz_version"], "14.2.0");
    assert_eq!(vectors["oracle"]["text_input_api"], "hb_buffer_add_utf8");
    for (expected, record) in vectors["records"].as_array().unwrap().iter().zip(records()) {
        validate_shaping_record_contract(&record).expect("valid oracle record");
        let actual = shape_text_a0(
            font_bytes(&record.input.font_id),
            &record.input,
            TextShapingLimits::default(),
        )
        .expect("shape fixed-font record");
        let expected_font = vectors["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|font| font["font_id"].as_str() == Some(record.input.font_id.as_str()))
            .unwrap();
        assert_eq!(
            u64::from(actual.units_per_em),
            expected_font["units_per_em"]
        );
        assert_eq!(
            serde_json::to_value(actual.glyphs).unwrap(),
            expected["glyphs"],
            "{}",
            record.case_id
        );
    }
}

#[test]
fn unicode_clusters_scaled_rtl_and_ignorables_are_structurally_visible() {
    let records = records();
    let combining = records
        .iter()
        .find(|record| record.case_id.as_str() == "kicad_stroke_unicode_combining")
        .unwrap();
    assert_eq!(combining.input.text.chars().count(), 4);
    assert_eq!(combining.input.text.len(), 6);
    assert_eq!(
        combining
            .glyphs
            .iter()
            .map(|glyph| glyph.cluster)
            .collect::<Vec<_>>(),
        [0, 1, 1, 5]
    );

    let rtl = records
        .iter()
        .find(|record| record.case_id.as_str() == "kicad_stroke_scaled_rtl")
        .unwrap();
    assert_eq!(
        rtl.glyphs
            .iter()
            .map(|glyph| glyph.cluster)
            .collect::<Vec<_>>(),
        [3, 2, 1, 0]
    );
    assert_eq!(rtl.input.scale_x, 1_375);
    assert_eq!(rtl.input.scale_y, 625);

    let removed = records
        .iter()
        .find(|record| record.case_id.as_str().contains("remove_default"))
        .unwrap();
    let preserved = records
        .iter()
        .find(|record| record.case_id.as_str().contains("preserve_default"))
        .unwrap();
    assert!(removed.glyphs.len() < preserved.glyphs.len());

    let ranged = records
        .iter()
        .find(|record| record.case_id.as_str() == "fixture_non_global_utf8_feature")
        .unwrap();
    assert_eq!(ranged.input.text.len(), 4);
    assert_eq!(
        (ranged.input.features[0].start, ranged.input.features[0].end),
        (2, 4)
    );
    assert_eq!(ranged.glyphs.len(), 2, "the ranged liga joins only A and B");
    let mut without_feature = ranged.input.clone();
    without_feature.features.clear();
    assert_eq!(
        shape_text_a0(
            VARIABLE_FONT_BYTES,
            &without_feature,
            TextShapingLimits::default()
        )
        .unwrap()
        .glyphs
        .len(),
        3,
        "the generated font proves the ranged feature changes shaping"
    );

    let arabic = records
        .iter()
        .find(|record| record.case_id.as_str() == "kicad_stroke_arabic_unsafe_concat")
        .unwrap();
    assert!(arabic.glyphs.iter().any(|glyph| glyph.unsafe_to_concat));
}

#[test]
fn optional_arabic_flags_are_mapped_with_versioned_evidence() {
    let evidence = &vectors()["versioned_optional_flag_evidence"];
    assert_eq!(evidence["comparison"], "geometry_exact_flag_presence");
    let input: ShapingInput = serde_json::from_value(evidence["input"].clone()).unwrap();
    let c_glyphs: Vec<ShapedGlyph> =
        serde_json::from_value(evidence["uharfbuzz_glyphs"].clone()).unwrap();
    let rust_glyphs = shape_text_a0(FONT_BYTES, &input, TextShapingLimits::default())
        .unwrap()
        .glyphs;
    assert_eq!(geometry(&rust_glyphs), geometry(&c_glyphs));
    assert!(c_glyphs.iter().any(|glyph| glyph.safe_to_insert_tatweel));
    assert!(rust_glyphs.iter().any(|glyph| glyph.safe_to_insert_tatweel));
}

fn geometry(glyphs: &[ShapedGlyph]) -> Vec<(u32, u32, i64, i64, i64, i64)> {
    glyphs
        .iter()
        .map(|glyph| {
            (
                glyph.glyph_id,
                glyph.cluster,
                glyph.x_advance.get(),
                glyph.y_advance.get(),
                glyph.x_offset.get(),
                glyph.y_offset.get(),
            )
        })
        .collect()
}

#[test]
fn resource_preflight_hash_and_face_fail_closed() {
    let record = records().remove(0);
    let exact_input = TextShapingLimits {
        max_font_bytes: FONT_BYTES.len(),
        max_text_bytes: record.input.text.len(),
        max_glyphs: record.glyphs.len(),
        ..TextShapingLimits::default()
    };
    shape_text_a0(FONT_BYTES, &record.input, exact_input).expect("inclusive byte and glyph limits");
    let one_under_font = TextShapingLimits {
        max_font_bytes: FONT_BYTES.len() - 1,
        ..TextShapingLimits::default()
    };
    assert_eq!(
        shape_text_a0(FONT_BYTES, &record.input, one_under_font)
            .unwrap_err()
            .kind,
        TextShapingErrorKind::ResourceLimit
    );
    let one_under_text = TextShapingLimits {
        max_text_bytes: record.input.text.len() - 1,
        ..TextShapingLimits::default()
    };
    assert_eq!(
        shape_text_a0(FONT_BYTES, &record.input, one_under_text)
            .unwrap_err()
            .kind,
        TextShapingErrorKind::ResourceLimit
    );
    let no_metadata = TextShapingLimits {
        max_metadata_bytes: 0,
        ..TextShapingLimits::default()
    };
    assert_eq!(
        shape_text_a0(FONT_BYTES, &record.input, no_metadata)
            .unwrap_err()
            .kind,
        TextShapingErrorKind::ResourceLimit
    );

    let mut wrong_hash = record.input.clone();
    wrong_hash.font_sha256.0 = "0".repeat(64);
    assert_eq!(
        shape_text_a0(FONT_BYTES, &wrong_hash, TextShapingLimits::default())
            .unwrap_err()
            .kind,
        TextShapingErrorKind::HashMismatch
    );
    let mut wrong_face = record.input.clone();
    wrong_face.face_index = 1;
    assert_eq!(
        shape_text_a0(FONT_BYTES, &wrong_face, TextShapingLimits::default())
            .unwrap_err()
            .kind,
        TextShapingErrorKind::InvalidFont
    );

    let feature_record = records()
        .into_iter()
        .find(|value| !value.input.features.is_empty())
        .unwrap();
    let no_features = TextShapingLimits {
        max_features: 0,
        ..TextShapingLimits::default()
    };
    assert_eq!(
        shape_text_a0(FONT_BYTES, &feature_record.input, no_features)
            .unwrap_err()
            .kind,
        TextShapingErrorKind::ResourceLimit
    );
}

#[test]
fn output_and_adapter_specific_inputs_are_bounded() {
    let record = records().remove(0);
    let one_under_glyph = TextShapingLimits {
        max_glyphs: record.glyphs.len() - 1,
        ..TextShapingLimits::default()
    };
    assert_eq!(
        shape_text_a0(FONT_BYTES, &record.input, one_under_glyph)
            .unwrap_err()
            .kind,
        TextShapingErrorKind::ResourceLimit
    );

    let mut empty_language = record.input.clone();
    empty_language.language = Some(String::new());
    assert_eq!(
        shape_text_a0(FONT_BYTES, &empty_language, TextShapingLimits::default())
            .unwrap_err()
            .kind,
        TextShapingErrorKind::InvalidContract
    );

    let mut huge_variation = vectors()["records"][0]["input"].clone();
    huge_variation["variations"] = serde_json::json!([{"axis": "wght", "value": 1.0e100}]);
    let huge_variation = serde_json::from_value(huge_variation).unwrap();
    assert_eq!(
        shape_text_a0(FONT_BYTES, &huge_variation, TextShapingLimits::default())
            .unwrap_err()
            .kind,
        TextShapingErrorKind::InvalidInput
    );
}

#[test]
fn variation_axes_are_supported_indexed_and_fail_closed() {
    let variable = records()
        .into_iter()
        .find(|record| record.case_id.as_str() == "fixture_supported_variation_axis")
        .unwrap();
    shape_text_a0(
        VARIABLE_FONT_BYTES,
        &variable.input,
        TextShapingLimits::default(),
    )
    .expect("declared wght axis is accepted");

    let mut unknown_axis = variable.input.clone();
    unknown_axis.variations[0].axis.0 = "wdth".to_owned();
    let error = shape_text_a0(
        VARIABLE_FONT_BYTES,
        &unknown_axis,
        TextShapingLimits::default(),
    )
    .expect_err("unknown variable-font axis must not be ignored");
    assert_eq!(error.kind, TextShapingErrorKind::InvalidInput);
    assert_eq!(error.path, "$.variations[0].axis");

    let mut non_variable = variable.input;
    non_variable.font_id = "kicad_stroke_regular".parse().unwrap();
    non_variable.font_sha256.0 =
        "e12a1ae527c6089914db479f4a30d2b5ff2745953e27b5709d6b933f4be3b487".to_owned();
    let error = shape_text_a0(FONT_BYTES, &non_variable, TextShapingLimits::default())
        .expect_err("variations on a static face must not be ignored");
    assert_eq!(error.kind, TextShapingErrorKind::InvalidInput);
    assert_eq!(error.path, "$.variations");
}

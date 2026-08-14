use kicad_monkey_contracts::generated::font_bundle_manifest::FontBundleManifestA0;
use kicad_monkey_contracts::generated::font_resolution_request::FontResolutionRequestA0;
use kicad_monkey_contracts::generated::outline_vector::OutlineVectorA0;
use kicad_monkey_contracts::generated::shaping_record::ShapingRecordA0;
use kicad_monkey_contracts::{
    FiniteFloat, FontBundleLimits, FontResolutionLimits, PositiveU32, StableTextId,
    resolve_font_selection_contract, validate_font_bundle_contract,
    validate_outline_vector_contract, validate_shaping_record_contract,
};

fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/font_text_a0_vectors.json"
    )))
    .expect("font/text vectors")
}

fn manifest() -> FontBundleManifestA0 {
    serde_json::from_value(vectors()["manifest"].clone()).expect("font manifest")
}

fn request(name: &str) -> FontResolutionRequestA0 {
    serde_json::from_value(vectors()["resolution_requests"][name].clone())
        .expect("font resolution request")
}

fn metadata_string_bytes(manifest: &FontBundleManifestA0) -> usize {
    manifest
        .fonts
        .iter()
        .map(|font| {
            font.id.len()
                + font.sha256.0.len()
                + font.aliases.iter().map(String::len).sum::<usize>()
                + font
                    .variations
                    .iter()
                    .map(|variation| variation.axis.0.len())
                    .sum::<usize>()
                + font.family.as_deref().map_or(0, str::len)
                + font.style.as_deref().map_or(0, str::len)
                + font.postscript_name.as_deref().map_or(0, str::len)
        })
        .sum()
}

#[test]
fn generated_font_text_roots_are_strict_and_round_trip() {
    let vectors = vectors();
    let manifest: FontBundleManifestA0 =
        serde_json::from_value(vectors["manifest"].clone()).expect("manifest");
    let shaping: ShapingRecordA0 =
        serde_json::from_value(vectors["shaping_record"].clone()).expect("shaping record");
    let shaping_index: ShapingRecordA0 =
        serde_json::from_value(vectors["shaping_index_record"].clone())
            .expect("shaping index record");
    let outline: OutlineVectorA0 =
        serde_json::from_value(vectors["outline_vector"].clone()).expect("outline vector");
    assert_eq!(manifest.fonts.len(), 2);
    assert_eq!(shaping.glyphs.len(), 1);
    assert_eq!(shaping_index.input.text.chars().count(), 4);
    assert_eq!(shaping_index.input.text.len(), 6);
    assert_eq!(shaping_index.input.features[0].start, 1);
    assert_eq!(shaping_index.input.features[0].end, 5);
    assert_eq!(
        shaping_index
            .glyphs
            .iter()
            .map(|glyph| glyph.cluster)
            .collect::<Vec<_>>(),
        [0, 1, 5]
    );
    assert_eq!(outline.commands.len(), 5);
    validate_shaping_record_contract(&shaping).expect("valid shaping record");
    validate_shaping_record_contract(&shaping_index).expect("valid UTF-8 index record");
    validate_outline_vector_contract(&outline).expect("valid outline vector");
    assert_eq!(serde_json::to_value(manifest).unwrap(), vectors["manifest"]);
    assert_eq!(
        serde_json::to_value(shaping).unwrap(),
        vectors["shaping_record"]
    );
    let encoded_outline = serde_json::to_value(outline).unwrap();
    assert_eq!(
        encoded_outline["case_id"],
        vectors["outline_vector"]["case_id"]
    );
    assert_eq!(
        encoded_outline["commands"],
        vectors["outline_vector"]["commands"]
    );
    assert_eq!(
        encoded_outline["coordinate_comparison"]["absolute_tolerance"]
            .as_f64()
            .unwrap(),
        0.000_001
    );
}

#[test]
fn generated_scalar_invariants_reject_invalid_values() {
    let vectors = vectors();
    let mut negative_tolerance = vectors["outline_vector"].clone();
    negative_tolerance["coordinate_comparison"]["absolute_tolerance"] = (-0.001).into();
    assert!(serde_json::from_value::<OutlineVectorA0>(negative_tolerance).is_err());

    let mut zero_units = vectors["outline_vector"].clone();
    zero_units["units_per_em"] = 0.into();
    assert!(serde_json::from_value::<OutlineVectorA0>(zero_units).is_err());

    let mut invalid_id = vectors["shaping_record"].clone();
    invalid_id["case_id"] = "".into();
    assert!(serde_json::from_value::<ShapingRecordA0>(invalid_id).is_err());

    assert!(FiniteFloat::try_from(f64::INFINITY).is_err());
    assert!(PositiveU32::try_from(0).is_err());
    assert!("bad id".parse::<StableTextId>().is_err());

    let mut extra = vectors["manifest"].clone();
    extra["buffers"] = serde_json::json!(["embedded bytes are forbidden"]);
    assert!(serde_json::from_value::<FontBundleManifestA0>(extra).is_err());
}

#[test]
fn manifest_validation_enforces_exact_slot_hash_and_metadata_semantics() {
    let buffers: [&[u8]; 2] = [b"font-a", b"font-b"];
    validate_font_bundle_contract(&manifest(), &buffers, FontBundleLimits::default())
        .expect("valid bundle");

    for (pointer, replacement, code) in [
        (
            "/fonts/1/id",
            serde_json::json!("primary"),
            "duplicate_font_id",
        ),
        ("/fonts/1/slot", serde_json::json!(0), "duplicate_font_slot"),
        ("/fonts/1/slot", serde_json::json!(2), "invalid_slot"),
        (
            "/fonts/0/sha256",
            serde_json::json!("A".repeat(64)),
            "invalid_hash",
        ),
        (
            "/fonts/0/aliases/1",
            serde_json::json!("Primary Sans"),
            "invalid_alias",
        ),
        (
            "/fonts/0/variations/0/axis",
            serde_json::json!("bad"),
            "invalid_variation",
        ),
    ] {
        let mut candidate = vectors()["manifest"].clone();
        *candidate.pointer_mut(pointer).expect("registered pointer") = replacement;
        let candidate: FontBundleManifestA0 =
            serde_json::from_value(candidate).expect("structurally valid candidate");
        let error =
            validate_font_bundle_contract(&candidate, &buffers, FontBundleLimits::default())
                .expect_err(pointer);
        assert_eq!(error.code, code, "{pointer}");
    }

    let error = validate_font_bundle_contract(
        &manifest(),
        &[b"font-x", b"font-b"],
        FontBundleLimits::default(),
    )
    .expect_err("hash mismatch");
    assert_eq!(error.code, "hash_mismatch");
    let error =
        validate_font_bundle_contract(&manifest(), &[b"font-a"], FontBundleLimits::default())
            .expect_err("unreferenced slot mismatch");
    assert_eq!(error.code, "buffer_count_mismatch");
}

#[test]
fn manifest_resource_boundaries_are_inclusive_and_checked_before_hashing() {
    let buffers: [&[u8]; 2] = [b"font-a", b"font-b"];
    let manifest = manifest();
    let metadata_bytes = metadata_string_bytes(&manifest);
    let exact = FontBundleLimits {
        max_fonts: 2,
        max_font_bytes: 6,
        max_total_font_bytes: 12,
        max_aliases_per_font: 2,
        max_variations_per_font: 1,
        max_metadata_string_bytes: metadata_bytes,
    };
    validate_font_bundle_contract(&manifest, &buffers, exact).expect("inclusive ceilings");
    for limits in [
        FontBundleLimits {
            max_fonts: 1,
            ..exact
        },
        FontBundleLimits {
            max_font_bytes: 5,
            ..exact
        },
        FontBundleLimits {
            max_total_font_bytes: 11,
            ..exact
        },
        FontBundleLimits {
            max_aliases_per_font: 1,
            ..exact
        },
        FontBundleLimits {
            max_variations_per_font: 0,
            ..exact
        },
        FontBundleLimits {
            max_metadata_string_bytes: metadata_bytes - 1,
            ..exact
        },
    ] {
        let error = validate_font_bundle_contract(&manifest, &buffers, limits)
            .expect_err("one-under ceiling");
        assert_eq!(error.code, "resource_limit");
    }
}

#[test]
fn deterministic_resolution_prefers_id_and_rejects_ambiguous_aliases() {
    let manifest = manifest();
    let buffers: [&[u8]; 2] = [b"font-a", b"font-b"];
    let bundle = validate_font_bundle_contract(&manifest, &buffers, FontBundleLimits::default())
        .expect("validated bundle");
    assert_eq!(
        resolve_font_selection_contract(
            &bundle,
            &request("explicit"),
            FontResolutionLimits::default(),
        )
        .unwrap()
        .id
        .as_str(),
        "primary"
    );
    assert_eq!(
        resolve_font_selection_contract(
            &bundle,
            &request("unique_alias"),
            FontResolutionLimits::default(),
        )
        .unwrap()
        .id
        .as_str(),
        "secondary"
    );
    assert_eq!(
        resolve_font_selection_contract(
            &bundle,
            &request("ambiguous_alias"),
            FontResolutionLimits::default(),
        )
        .unwrap_err()
        .code,
        "ambiguous_font"
    );
    assert_eq!(
        resolve_font_selection_contract(
            &bundle,
            &request("missing"),
            FontResolutionLimits::default(),
        )
        .unwrap_err()
        .code,
        "missing_font"
    );
}

#[test]
fn shaping_indices_require_utf8_code_point_boundaries() {
    let vectors = vectors();
    let valid: ShapingRecordA0 =
        serde_json::from_value(vectors["shaping_index_record"].clone()).unwrap();
    validate_shaping_record_contract(&valid).expect("durable UTF-8 byte-offset vector");

    let mut inside_multibyte_scalar = vectors["shaping_index_record"].clone();
    inside_multibyte_scalar["input"]["features"][0]["start"] = 2.into();
    let invalid: ShapingRecordA0 = serde_json::from_value(inside_multibyte_scalar).unwrap();
    assert_eq!(
        validate_shaping_record_contract(&invalid).unwrap_err().code,
        "invalid_text_index"
    );

    let mut inside_combining_scalar = vectors["shaping_index_record"].clone();
    inside_combining_scalar["glyphs"][2]["cluster"] = 4.into();
    let invalid: ShapingRecordA0 = serde_json::from_value(inside_combining_scalar).unwrap();
    assert_eq!(
        validate_shaping_record_contract(&invalid).unwrap_err().code,
        "invalid_text_index"
    );

    let mut terminal_cluster = vectors["shaping_index_record"].clone();
    terminal_cluster["glyphs"][2]["cluster"] = 6.into();
    let invalid: ShapingRecordA0 = serde_json::from_value(terminal_cluster).unwrap();
    assert_eq!(
        validate_shaping_record_contract(&invalid).unwrap_err().code,
        "invalid_text_index"
    );

    let mut empty_text_glyph = vectors["shaping_record"].clone();
    empty_text_glyph["input"]["text"] = "".into();
    empty_text_glyph["input"]["features"] = serde_json::json!([]);
    empty_text_glyph["glyphs"][0]["cluster"] = 0.into();
    let invalid: ShapingRecordA0 = serde_json::from_value(empty_text_glyph).unwrap();
    assert_eq!(
        validate_shaping_record_contract(&invalid).unwrap_err().code,
        "invalid_text_index"
    );

    let mut duplicate_feature = vectors["shaping_index_record"].clone();
    let repeated = duplicate_feature["input"]["features"][0].clone();
    duplicate_feature["input"]["features"]
        .as_array_mut()
        .unwrap()
        .push(repeated);
    let invalid: ShapingRecordA0 = serde_json::from_value(duplicate_feature).unwrap();
    let error = validate_shaping_record_contract(&invalid).unwrap_err();
    assert_eq!(error.code, "duplicate_feature_tag");
    assert_eq!(error.path, "$.input.features[1].tag");
}

#[test]
fn aggregate_resource_preflight_wins_before_any_hashing() {
    let buffers: [&[u8]; 2] = [b"font-x", b"font-b"];
    let limits = FontBundleLimits {
        max_total_font_bytes: 11,
        ..FontBundleLimits::default()
    };
    let error = validate_font_bundle_contract(&manifest(), &buffers, limits)
        .expect_err("aggregate bytes must fail before the bad first hash");
    assert_eq!(error.code, "resource_limit");
}

#[test]
fn indexed_resolution_has_bounded_request_work_and_requires_a_valid_manifest() {
    let manifest = manifest();
    let buffers: [&[u8]; 2] = [b"font-a", b"font-b"];
    let bundle =
        validate_font_bundle_contract(&manifest, &buffers, FontBundleLimits::default()).unwrap();
    let error = resolve_font_selection_contract(
        &bundle,
        &request("unique_alias"),
        FontResolutionLimits {
            max_request_aliases: 0,
            max_request_string_bytes: usize::MAX,
        },
    )
    .expect_err("alias count limit");
    assert_eq!(error.code, "resource_limit");
    let error = resolve_font_selection_contract(
        &bundle,
        &request("unique_alias"),
        FontResolutionLimits {
            max_request_aliases: usize::MAX,
            max_request_string_bytes: "Secondary Sans".len() - 1,
        },
    )
    .expect_err("request byte limit");
    assert_eq!(error.code, "resource_limit");

    let mut invalid = vectors()["manifest"].clone();
    invalid["fonts"][1]["id"] = "primary".into();
    let invalid: FontBundleManifestA0 = serde_json::from_value(invalid).unwrap();
    assert_eq!(
        validate_font_bundle_contract(&invalid, &buffers, FontBundleLimits::default())
            .expect_err("invalid manifests cannot produce an indexed handle")
            .code,
        "duplicate_font_id"
    );
}

#[test]
fn text_safe_integer_vectors_match_generated_rust_boundaries() {
    let vectors = vectors();
    for case in vectors["safe_integer_cases"]
        .as_array()
        .expect("safe integer cases")
    {
        let root = case["root"].as_str().expect("root");
        let mut candidate = vectors[root].clone();
        *candidate
            .pointer_mut(case["pointer"].as_str().expect("pointer"))
            .expect("registered pointer") = case["value"].clone();
        let valid = match root {
            "shaping_record" => serde_json::from_value::<ShapingRecordA0>(candidate).is_ok(),
            "outline_vector" => serde_json::from_value::<OutlineVectorA0>(candidate).is_ok(),
            _ => panic!("unknown root"),
        };
        assert_eq!(valid, case["valid"].as_bool().unwrap(), "{}", case["id"]);
    }

    for case in vectors["scale_integer_cases"].as_array().unwrap() {
        let mut candidate = vectors["shaping_record"].clone();
        candidate["input"]["scale_x"] = case["value"].clone();
        assert_eq!(
            serde_json::from_value::<ShapingRecordA0>(candidate).is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}",
            case["id"]
        );
    }
}

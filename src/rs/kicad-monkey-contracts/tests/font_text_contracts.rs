use kicad_monkey_contracts::generated::font_bundle_manifest::FontBundleManifestA0;
use kicad_monkey_contracts::generated::font_resolution_request::FontResolutionRequestA0;
use kicad_monkey_contracts::generated::outline_vector::OutlineVectorA0;
use kicad_monkey_contracts::generated::shaping_record::ShapingRecordA0;
use kicad_monkey_contracts::{
    FontBundleLimits, resolve_font_selection_contract, validate_font_bundle_contract,
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

#[test]
fn generated_font_text_roots_are_strict_and_round_trip() {
    let vectors = vectors();
    let manifest: FontBundleManifestA0 =
        serde_json::from_value(vectors["manifest"].clone()).expect("manifest");
    let shaping: ShapingRecordA0 =
        serde_json::from_value(vectors["shaping_record"].clone()).expect("shaping record");
    let outline: OutlineVectorA0 =
        serde_json::from_value(vectors["outline_vector"].clone()).expect("outline vector");
    assert_eq!(manifest.fonts.len(), 2);
    assert_eq!(shaping.glyphs.len(), 1);
    assert_eq!(outline.commands.len(), 5);
    assert_eq!(serde_json::to_value(manifest).unwrap(), vectors["manifest"]);
    assert_eq!(
        serde_json::to_value(shaping).unwrap(),
        vectors["shaping_record"]
    );
    assert_eq!(
        serde_json::to_value(outline).unwrap(),
        vectors["outline_vector"]
    );

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
    let exact = FontBundleLimits {
        max_fonts: 2,
        max_font_bytes: 6,
        max_total_font_bytes: 12,
        max_aliases_per_font: 2,
        max_variations_per_font: 1,
    };
    validate_font_bundle_contract(&manifest(), &buffers, exact).expect("inclusive ceilings");
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
    ] {
        let error = validate_font_bundle_contract(&manifest(), &buffers, limits)
            .expect_err("one-under ceiling");
        assert_eq!(error.code, "resource_limit");
    }
}

#[test]
fn deterministic_resolution_prefers_id_and_rejects_ambiguous_aliases() {
    let manifest = manifest();
    assert_eq!(
        resolve_font_selection_contract(&manifest, &request("explicit"))
            .unwrap()
            .id,
        "primary"
    );
    assert_eq!(
        resolve_font_selection_contract(&manifest, &request("unique_alias"))
            .unwrap()
            .id,
        "secondary"
    );
    assert_eq!(
        resolve_font_selection_contract(&manifest, &request("ambiguous_alias"))
            .unwrap_err()
            .code,
        "ambiguous_font"
    );
    assert_eq!(
        resolve_font_selection_contract(&manifest, &request("missing"))
            .unwrap_err()
            .code,
        "missing_font"
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
}

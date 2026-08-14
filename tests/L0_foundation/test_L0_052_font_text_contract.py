"""Font bundle, shaping-record, and outline-vector contract gate."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import subprocess
from typing import Any

import msgspec
import pytest
from jsonschema import Draft202012Validator

from kicad_monkey.contracts.generated import (
    FontBundleManifestA0,
    OutlineVectorA0,
    ShapingRecordA0,
    decode_font_bundle_manifest_a0,
    decode_font_resolution_request_a0,
    decode_outline_vector_a0,
    decode_shaping_record_a0,
    resolve_font_selection_a0,
    validate_font_bundle_manifest_a0,
    validate_outline_vector_a0,
    validate_shaping_record_a0,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/font_text_a0_vectors.json"
SCHEMA_ROOT = PACKAGE_ROOT / "contracts/generated/schema"


def _vectors() -> dict[str, Any]:
    return json.loads(VECTORS.read_text(encoding="utf-8"))


def _decode_manifest(vectors: dict[str, object]) -> FontBundleManifestA0:
    return decode_font_bundle_manifest_a0(json.dumps(vectors["manifest"]).encode())


def test_all_four_generated_roots_are_strict_and_keep_font_bytes_out_of_band() -> None:
    vectors = _vectors()
    roots = [
        ("FontBundleManifest.json", vectors["manifest"], decode_font_bundle_manifest_a0),
        (
            "FontResolutionRequest.json",
            vectors["resolution_requests"]["explicit"],
            decode_font_resolution_request_a0,
        ),
        ("ShapingRecord.json", vectors["shaping_record"], decode_shaping_record_a0),
        (
            "ShapingRecord.json",
            vectors["shaping_index_record"],
            decode_shaping_record_a0,
        ),
        ("OutlineVector.json", vectors["outline_vector"], decode_outline_vector_a0),
    ]
    for schema_name, value, decoder in roots:
        schema = json.loads((SCHEMA_ROOT / schema_name).read_text(encoding="utf-8"))
        Draft202012Validator(schema).validate(value)
        decoder(json.dumps(value).encode())
        with_unknown = {**value, "unknown": True}
        assert list(Draft202012Validator(schema).iter_errors(with_unknown))
        with pytest.raises(msgspec.ValidationError):
            decoder(json.dumps(with_unknown).encode())

    manifest = vectors["manifest"]
    assert "buffers" not in manifest
    assert "buffers_utf8" not in manifest


def test_python_manifest_semantics_enforce_slots_hashes_and_inclusive_limits() -> None:
    vectors = _vectors()
    manifest = _decode_manifest(vectors)
    buffers = tuple(value.encode() for value in vectors["buffers_utf8"])
    metadata_strings: list[str] = []
    for font in vectors["manifest"]["fonts"]:
        metadata_strings.extend([font["id"], font["sha256"], *font["aliases"]])
        metadata_strings.extend(variation["axis"] for variation in font["variations"])
        metadata_strings.extend(
            font[field]
            for field in ("family", "style", "postscript_name")
            if field in font
        )
    metadata_bytes = sum(len(value.encode()) for value in metadata_strings)
    validate_font_bundle_manifest_a0(
        manifest,
        buffers,
        max_fonts=2,
        max_font_bytes=6,
        max_total_font_bytes=12,
        max_aliases_per_font=2,
        max_variations_per_font=1,
        max_metadata_string_bytes=metadata_bytes,
    )

    limit_cases = [
        {"max_fonts": 1},
        {"max_font_bytes": 5},
        {"max_total_font_bytes": 11},
        {"max_aliases_per_font": 1},
        {"max_variations_per_font": 0},
    ]
    for limits in limit_cases:
        with pytest.raises(msgspec.ValidationError, match="resource_limit"):
            validate_font_bundle_manifest_a0(manifest, buffers, **limits)
    with pytest.raises(msgspec.ValidationError, match="resource_limit"):
        validate_font_bundle_manifest_a0(
            manifest,
            buffers,
            max_metadata_string_bytes=metadata_bytes - 1,
        )

    with pytest.raises(msgspec.ValidationError, match="buffer_count_mismatch"):
        validate_font_bundle_manifest_a0(manifest, buffers[:1])
    with pytest.raises(msgspec.ValidationError, match="hash_mismatch"):
        validate_font_bundle_manifest_a0(manifest, (b"font-x", buffers[1]))
    with pytest.raises(msgspec.ValidationError, match="resource_limit"):
        validate_font_bundle_manifest_a0(
            manifest,
            (b"font-x", buffers[1]),
            max_total_font_bytes=11,
        )

    semantic_cases = [
        ("id", 1, "primary", "duplicate_font_id"),
        ("slot", 1, 0, "duplicate_font_slot"),
    ]
    for field, font_index, value, code in semantic_cases:
        candidate = deepcopy(vectors["manifest"])
        candidate["fonts"][font_index][field] = value
        with pytest.raises(msgspec.ValidationError, match=code):
            validate_font_bundle_manifest_a0(
                decode_font_bundle_manifest_a0(json.dumps(candidate).encode()), buffers
            )

    invalid_hash = deepcopy(vectors["manifest"])
    invalid_hash["fonts"][0]["sha256"] = "A" * 64
    with pytest.raises(msgspec.ValidationError, match="matching regex"):
        decode_font_bundle_manifest_a0(json.dumps(invalid_hash).encode())

    duplicate_alias = deepcopy(vectors["manifest"])
    duplicate_alias["fonts"][0]["aliases"][1] = "Primary Sans"
    with pytest.raises(msgspec.ValidationError, match="invalid_alias"):
        validate_font_bundle_manifest_a0(
            decode_font_bundle_manifest_a0(json.dumps(duplicate_alias).encode()), buffers
        )

    invalid_axis = deepcopy(vectors["manifest"])
    invalid_axis["fonts"][0]["variations"].append(
        deepcopy(invalid_axis["fonts"][0]["variations"][0])
    )
    with pytest.raises(msgspec.ValidationError, match="invalid_variation"):
        validate_font_bundle_manifest_a0(
            decode_font_bundle_manifest_a0(json.dumps(invalid_axis).encode()), buffers
        )

    variation = msgspec.structs.replace(
        manifest.fonts[0].variations[0], value=float("inf")
    )
    font = msgspec.structs.replace(manifest.fonts[0], variations=[variation])
    nonfinite = msgspec.structs.replace(manifest, fonts=[font, manifest.fonts[1]])
    with pytest.raises(msgspec.ValidationError, match="invalid_variation"):
        validate_font_bundle_manifest_a0(nonfinite, buffers)


def test_python_font_resolution_is_deterministic_and_fail_closed() -> None:
    vectors = _vectors()
    manifest = _decode_manifest(vectors)
    buffers = tuple(value.encode() for value in vectors["buffers_utf8"])
    bundle = validate_font_bundle_manifest_a0(manifest, buffers)
    requests = vectors["resolution_requests"]
    explicit = decode_font_resolution_request_a0(json.dumps(requests["explicit"]).encode())
    unique = decode_font_resolution_request_a0(json.dumps(requests["unique_alias"]).encode())
    ambiguous = decode_font_resolution_request_a0(
        json.dumps(requests["ambiguous_alias"]).encode()
    )
    missing = decode_font_resolution_request_a0(json.dumps(requests["missing"]).encode())
    assert resolve_font_selection_a0(bundle, explicit).id == "primary"
    assert resolve_font_selection_a0(bundle, unique).id == "secondary"
    with pytest.raises(msgspec.ValidationError, match="ambiguous_font"):
        resolve_font_selection_a0(bundle, ambiguous)
    with pytest.raises(msgspec.ValidationError, match="missing_font"):
        resolve_font_selection_a0(bundle, missing)
    with pytest.raises(msgspec.ValidationError, match="resource_limit"):
        resolve_font_selection_a0(bundle, unique, max_request_aliases=0)
    with pytest.raises(msgspec.ValidationError, match="resource_limit"):
        resolve_font_selection_a0(
            bundle,
            unique,
            max_request_string_bytes=len("Secondary Sans".encode()) - 1,
        )


def test_intermediate_records_preserve_shaping_and_outline_separation() -> None:
    vectors = _vectors()
    shaping = decode_shaping_record_a0(json.dumps(vectors["shaping_record"]).encode())
    shaping_index = decode_shaping_record_a0(
        json.dumps(vectors["shaping_index_record"]).encode()
    )
    outline = decode_outline_vector_a0(json.dumps(vectors["outline_vector"]).encode())
    assert isinstance(shaping, ShapingRecordA0)
    assert isinstance(outline, OutlineVectorA0)
    assert shaping.input.text == "ffi"
    assert shaping.case_id == "stroke_regular_latin_ltr_ffi"
    assert shaping.input.buffer_properties.cluster_level == "monotone_graphemes"
    assert shaping.input.buffer_properties.produce_unsafe_to_concat is True
    assert len(shaping_index.input.text) == 4
    assert len(shaping_index.input.text.encode()) == 6
    assert shaping_index.input.text_index_unit == "utf8_byte_offset"
    assert (shaping_index.input.features[0].start, shaping_index.input.features[0].end) == (
        1,
        5,
    )
    assert [glyph.cluster for glyph in shaping_index.glyphs] == [0, 1, 5]
    validate_shaping_record_a0(shaping_index)
    validate_outline_vector_a0(outline)
    assert shaping.glyphs[0].glyph_id == outline.glyph_id
    encoded_outline = json.loads(msgspec.json.encode(outline))
    assert encoded_outline["case_id"] == "fractional_outline_glyph_5044"
    assert encoded_outline["coordinate_format"] == "font_design_units_f64"
    assert encoded_outline["coordinate_comparison"] == {
        "mode": "absolute_tolerance",
        "absolute_tolerance": 0.000001,
    }
    assert encoded_outline["commands"][3]["control1_x"] == 75.125
    assert [command["kind"] for command in encoded_outline["commands"]] == [
        "move_to",
        "line_to",
        "quad_to",
        "curve_to",
        "close",
    ]

    negative_tolerance = deepcopy(vectors["outline_vector"])
    negative_tolerance["coordinate_comparison"]["absolute_tolerance"] = -0.001
    outline_schema = json.loads(
        (SCHEMA_ROOT / "OutlineVector.json").read_text(encoding="utf-8")
    )
    assert list(Draft202012Validator(outline_schema).iter_errors(negative_tolerance))
    with pytest.raises(msgspec.ValidationError):
        decode_outline_vector_a0(json.dumps(negative_tolerance).encode())


def test_text_semantics_reject_invalid_indices_ids_units_and_nonfinite_programmatic_values() -> None:
    vectors = _vectors()

    inside_scalar = deepcopy(vectors["shaping_index_record"])
    inside_scalar["input"]["features"][0]["start"] = 2
    with pytest.raises(msgspec.ValidationError, match="invalid_text_index"):
        decode_shaping_record_a0(json.dumps(inside_scalar).encode())

    inside_combining = deepcopy(vectors["shaping_index_record"])
    inside_combining["glyphs"][2]["cluster"] = 4
    with pytest.raises(msgspec.ValidationError, match="invalid_text_index"):
        decode_shaping_record_a0(json.dumps(inside_combining).encode())

    terminal_cluster = deepcopy(vectors["shaping_index_record"])
    terminal_cluster["glyphs"][2]["cluster"] = 6
    with pytest.raises(msgspec.ValidationError, match="invalid_text_index"):
        decode_shaping_record_a0(json.dumps(terminal_cluster).encode())

    empty_text_glyph = deepcopy(vectors["shaping_record"])
    empty_text_glyph["input"]["text"] = ""
    empty_text_glyph["input"]["features"] = []
    empty_text_glyph["glyphs"][0]["cluster"] = 0
    with pytest.raises(msgspec.ValidationError, match="invalid_text_index"):
        decode_shaping_record_a0(json.dumps(empty_text_glyph).encode())

    empty_language = deepcopy(vectors["shaping_record"])
    empty_language["input"]["language"] = ""
    with pytest.raises(msgspec.ValidationError):
        decode_shaping_record_a0(json.dumps(empty_language).encode())

    for root, field in (("shaping_record", "case_id"), ("outline_vector", "font_id")):
        invalid_id = deepcopy(vectors[root])
        invalid_id[field] = ""
        decoder = (
            decode_shaping_record_a0 if root == "shaping_record" else decode_outline_vector_a0
        )
        with pytest.raises(msgspec.ValidationError):
            decoder(json.dumps(invalid_id).encode())

    zero_units = deepcopy(vectors["outline_vector"])
    zero_units["units_per_em"] = 0
    with pytest.raises(msgspec.ValidationError):
        decode_outline_vector_a0(json.dumps(zero_units).encode())

    outline = decode_outline_vector_a0(json.dumps(vectors["outline_vector"]).encode())
    first_command = msgspec.structs.replace(outline.commands[0], x=float("inf"))
    nonfinite_outline = msgspec.structs.replace(
        outline, commands=[first_command, *outline.commands[1:]]
    )
    with pytest.raises(msgspec.ValidationError, match="invalid_coordinate"):
        validate_outline_vector_a0(nonfinite_outline)

    shaping = decode_shaping_record_a0(json.dumps(vectors["shaping_record"]).encode())
    variation = msgspec.structs.replace(
        shaping.input.variations[0], value=float("inf")
    )
    shaping_input = msgspec.structs.replace(shaping.input, variations=[variation])
    nonfinite_shaping = msgspec.structs.replace(shaping, input=shaping_input)
    with pytest.raises(msgspec.ValidationError, match="invalid_variation"):
        validate_shaping_record_a0(nonfinite_shaping)


def test_text_safe_integer_vectors_match_schema_and_python() -> None:
    vectors = _vectors()
    decoders = {
        "shaping_record": (
            Draft202012Validator(
                json.loads((SCHEMA_ROOT / "ShapingRecord.json").read_text(encoding="utf-8"))
            ),
            decode_shaping_record_a0,
        ),
        "outline_vector": (
            Draft202012Validator(
                json.loads((SCHEMA_ROOT / "OutlineVector.json").read_text(encoding="utf-8"))
            ),
            decode_outline_vector_a0,
        ),
    }
    for case in vectors["safe_integer_cases"]:
        candidate = deepcopy(vectors[case["root"]])
        current = candidate
        parts = case["pointer"].strip("/").split("/")
        for part in parts[:-1]:
            current = current[int(part)] if isinstance(current, list) else current[part]
        key = parts[-1]
        if isinstance(current, list):
            current[int(key)] = case["value"]
        else:
            current[key] = case["value"]
        validator, decoder = decoders[case["root"]]
        schema_valid = not list(validator.iter_errors(candidate))
        assert schema_valid is case["valid"], case["id"]
        if case["valid"]:
            decoder(json.dumps(candidate).encode())
        else:
            with pytest.raises(msgspec.ValidationError):
                decoder(json.dumps(candidate).encode())

    shaping_schema = Draft202012Validator(
        json.loads((SCHEMA_ROOT / "ShapingRecord.json").read_text(encoding="utf-8"))
    )
    for case in vectors["scale_integer_cases"]:
        candidate = deepcopy(vectors["shaping_record"])
        candidate["input"]["scale_x"] = case["value"]
        schema_valid = not list(shaping_schema.iter_errors(candidate))
        assert schema_valid is case["valid"], case["id"]
        if case["valid"]:
            decode_shaping_record_a0(json.dumps(candidate).encode())
        else:
            with pytest.raises(msgspec.ValidationError):
                decode_shaping_record_a0(json.dumps(candidate).encode())


def test_focused_native_contract_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-contracts",
            "--test",
            "font_text_contracts",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

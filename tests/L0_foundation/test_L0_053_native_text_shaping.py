"""Fixed-font Python/HarfRust shaping parity gate."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

from jsonschema import Draft202012Validator
import pytest

from kicad_monkey.contracts.generated import decode_shaping_record_a0
from scripts.generate_font_shaping_vectors import _ordered_ranged_features

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/font_shaping_a0_vectors.json"
SCHEMA = PACKAGE_ROOT / "contracts/generated/schema/ShapingRecord.json"


def test_fixed_uharfbuzz_records_are_current_strict_and_use_utf8_input() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "engine": "uharfbuzz",
        "harfbuzz_version": "14.2.0",
        "text_input_api": "hb_buffer_add_utf8",
    }
    fonts = {font["font_id"]: font for font in vectors["fonts"]}
    assert fonts["kicad_stroke_regular"] == {
        "font_id": "kicad_stroke_regular",
        "font_path": "assets/fonts/kicad-stroke.ttf",
        "font_sha256": "e12a1ae527c6089914db479f4a30d2b5ff2745953e27b5709d6b933f4be3b487",
        "face_index": 0,
        "units_per_em": 1000,
    }
    variable_font = fonts["shaping_variable_fixture"]
    fixture_path = PACKAGE_ROOT / variable_font["font_path"]
    assert fixture_path.is_file()
    assert hashlib.sha256(fixture_path.read_bytes()).hexdigest() == variable_font["font_sha256"]
    validator = Draft202012Validator(json.loads(SCHEMA.read_text(encoding="utf-8")))
    for record in vectors["records"]:
        validator.validate(record)
        decode_shaping_record_a0(json.dumps(record).encode())
    combining = next(
        record
        for record in vectors["records"]
        if record["case_id"] == "kicad_stroke_unicode_combining"
    )
    assert len(combining["input"]["text"]) == 4
    assert len(combining["input"]["text"].encode()) == 6
    assert [glyph["cluster"] for glyph in combining["glyphs"]] == [0, 1, 1, 5]

    ranged = next(
        record
        for record in vectors["records"]
        if record["case_id"] == "fixture_non_global_utf8_feature"
    )
    assert len(ranged["input"]["text"]) == 3
    assert len(ranged["input"]["text"].encode()) == 4
    assert ranged["input"]["features"] == [
        {"tag": "dlig", "value": 1, "start": 2, "end": 4}
    ]
    assert len(ranged["glyphs"]) == 2

    arabic = next(
        record
        for record in vectors["records"]
        if record["case_id"] == "kicad_stroke_arabic_unsafe_concat"
    )
    assert any(glyph["unsafe_to_concat"] for glyph in arabic["glyphs"])
    optional_flags = vectors["versioned_optional_flag_evidence"]
    assert optional_flags["comparison"] == "geometry_exact_flag_presence"
    assert any(
        glyph["safe_to_insert_tatweel"]
        for glyph in optional_flags["uharfbuzz_glyphs"]
    )

    completed = subprocess.run(
        [
            sys.executable,
            "scripts/generate_font_shaping_vectors.py",
            "--check",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_shaping_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_shaping",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_oracle_feature_adapter_preserves_ranges_and_rejects_duplicate_tags() -> None:
    assert _ordered_ranged_features(
        [{"tag": "dlig", "value": 1, "start": 2, "end": 4}]
    ) == {"dlig": [(2, 4, 1)]}
    with pytest.raises(ValueError, match="duplicate feature tag"):
        _ordered_ranged_features(
            [
                {"tag": "dlig", "value": 1, "start": 0, "end": 1},
                {"tag": "dlig", "value": 0, "start": 1, "end": 2},
            ]
        )

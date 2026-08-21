"""Fixed-font FontTools/ttf-parser outline parity gate."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

from jsonschema import Draft202012Validator

from kicad_monkey.contracts.generated import decode_outline_vector_a0

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/font_outline_a0_vectors.json"
SCHEMA = PACKAGE_ROOT / "contracts/generated/schema/OutlineVector.json"


def test_fonttools_outline_records_are_current_strict_and_out_of_band() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "engine": "fontTools",
        "coordinate_space": "unscaled_font_design_units",
        "quadratic_api": "BasePen._qCurveToOne",
        "cubic_api": "BasePen._curveToOne",
    }
    fonts = {font["font_id"]: font for font in vectors["fonts"]}
    assert set(fonts) == {
        "kicad_stroke_regular",
        "outline_variable_fixture",
        "outline_cff_fixture",
        "outline_cff2_fixture",
        "outline_composite_fixture",
        "outline_collection_second_face",
    }
    for font in fonts.values():
        path = PACKAGE_ROOT / font["font_path"]
        assert path.is_file()
        assert hashlib.sha256(path.read_bytes()).hexdigest() == font["font_sha256"]

    validator = Draft202012Validator(json.loads(SCHEMA.read_text(encoding="utf-8")))
    for record in vectors["records"]:
        validator.validate(record)
        decode_outline_vector_a0(json.dumps(record).encode())
        assert "font_bytes" not in record

    by_case = {record["case_id"]: record for record in vectors["records"]}
    assert any(
        command["kind"] == "line_to"
        for command in by_case["kicad_stroke_line_outline"]["commands"]
    )
    default = by_case["variable_quadratic_default"]
    weighted = by_case["variable_quadratic_weight_700"]
    assert any(command["kind"] == "quad_to" for command in default["commands"])
    assert default["commands"] != weighted["commands"]
    assert weighted["variations"] == [{"axis": "wght", "value": 700.0}]
    assert any(
        command["kind"] == "curve_to"
        for command in by_case["cff_cubic_outline"]["commands"]
    )
    cff2 = by_case["cff2_cubic_outline"]
    assert cff2["variations"] == [{"axis": "wght", "value": 700.0}]
    assert cff2["commands"][0]["x"] == 810
    assert any(
        command["kind"] == "curve_to"
        for command in cff2["commands"]
    )
    composite = by_case["transformed_composite_glyf"]
    assert any(command["kind"] == "quad_to" for command in composite["commands"])
    assert composite["commands"][0]["x"] != 50
    collection = by_case["collection_second_face"]
    assert collection["face_index"] == 1
    assert collection["font_id"] == "outline_collection_second_face"

    completed = subprocess.run(
        [sys.executable, "scripts/generate_font_outline_vectors.py", "--check"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_outline_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "font_outline",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

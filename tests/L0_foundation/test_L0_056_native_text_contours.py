"""Native shaping-to-outline contour placement parity gate."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/text_contour_vectors.json"


def test_text_contour_records_are_current_bounded_and_out_of_band() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "shaping_engine": "uharfbuzz",
        "outline_engine": "fontTools.BasePen",
        "curve_engine": "kicad_monkey.KiCadTextRenderer._bezier_get_poly",
        "coordinate_space": "positioned_text_run_units_y_down",
        "hinting": "none",
    }
    font = vectors["font"]
    font_path = PACKAGE_ROOT / font["font_path"]
    assert hashlib.sha256(font_path.read_bytes()).hexdigest() == font["font_sha256"]
    records = {record["case_id"]: record for record in vectors["records"]}
    assert set(records) == {
        "single_glyph_origin",
        "kerning_pair_anisotropic_offset",
        "multiple_contours_hole",
        "missing_outline_space_advances",
    }
    assert len(records["multiple_contours_hole"]["contours"]) > 1
    assert records["missing_outline_space_advances"]["advance_x"] > records[
        "single_glyph_origin"
    ]["advance_x"]
    assert all("font_bytes" not in record for record in records.values())

    completed = subprocess.run(
        [sys.executable, "scripts/generate_text_contour_vectors.py", "--check"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_text_contour_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_contours",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

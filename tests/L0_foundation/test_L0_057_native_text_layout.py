"""Native single-line KiCad text alignment and transform parity gate."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/text_layout_vectors.json"


def test_text_layout_records_are_current_and_cover_transform_order() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "implementation": "kicad_monkey.KiCadTextRenderer alignment helpers",
        "transform_order": ["alignment", "mirror_x", "clockwise_rotation"],
        "rotation_origin": "authored_text_position",
        "height_fudge_factor": 1.17,
    }
    records = {record["case_id"]: record for record in vectors["records"]}
    assert set(records) == {
        "left_top_identity",
        "center_center_rotated",
        "right_bottom_mirrored_quarter_turn",
        "right_top_mirrored_negative_rotation",
        "right_to_left_centered",
    }
    assert {record["horizontal_alignment"] for record in records.values()} == {
        "left",
        "center",
        "right",
    }
    assert {record["vertical_alignment"] for record in records.values()} == {
        "top",
        "center",
        "bottom",
    }
    assert any(record["mirrored"] for record in records.values())
    assert any(record["angle_degrees"] < 0 for record in records.values())
    assert (
        records["right_to_left_centered"]["shaping"]["direction"]
        == "right_to_left"
    )

    completed = subprocess.run(
        [sys.executable, "scripts/generate_text_layout_vectors.py", "--check"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_text_layout_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_layout",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

"""KiCad BEZIER_POLY parity gate for native text geometry."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/text_bezier_vectors.json"


def test_python_kicad_decomposition_records_are_current_and_cover_curve_classes() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "implementation": "kicad_monkey.KiCadTextRenderer._bezier_get_poly",
        "kicad_revision": "5f555f4d63b970e410d567d1f79e05e8ce41b9d8",
        "kicad_source_algorithm": "libs/kimath/src/bezier_curves.cpp",
        "kicad_text_integration": "common/font/outline_decomposer.cpp",
        "coordinate_space": "caller_units_f64",
    }
    records = {record["case_id"]: record for record in vectors["records"]}
    assert {
        "quadratic_arch_default_error",
        "quadratic_arch_fine_error",
        "quadratic_collinear",
        "cubic_arch_default_error",
        "cubic_single_inflection",
        "cubic_double_inflection",
        "cubic_fractional_cff_shape",
        "cubic_nonpositive_uses_kicad_default",
    } == set(records)
    assert len(records["quadratic_arch_fine_error"]["points"]) > len(
        records["quadratic_arch_default_error"]["points"]
    )
    assert len(records["quadratic_collinear"]["points"]) == 2
    assert all(
        record["comparison"]
        == {"mode": "absolute_tolerance", "absolute_tolerance": 1.0e-10}
        for record in records.values()
    )

    completed = subprocess.run(
        [sys.executable, "scripts/generate_text_bezier_vectors.py", "--check"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_text_bezier_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_bezier",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

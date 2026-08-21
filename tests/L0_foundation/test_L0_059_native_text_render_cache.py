"""Native typed render-cache composition and S-expression I/O gate."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/text_render_cache_vectors.json"


def test_render_cache_records_are_current_and_pin_upstream_writer() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "composition": (
            "accepted Python layout contours plus kicad_render_cache fracture"
        ),
        "serialization": "RenderCache.to_sexp plus kicad_sexpr.build_sexp",
        "kicad_revision": "d6ff4c23641ee5236b7c9fac19eb6af1849294f5",
        "kicad_writer": (
            "pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp::formatRenderCache"
        ),
    }
    records = {record["case_id"]: record for record in vectors["records"]}
    assert set(records) == {
        "single_glyph_cache",
        "rotated_holed_glyph_cache",
    }
    assert len(records["single_glyph_cache"]["polygons"]) >= 1
    assert len(records["rotated_holed_glyph_cache"]["polygons"]) == 1
    assert "\\n" in vectors["serialization_probe"]["sexpr"]

    completed = subprocess.run(
        [sys.executable, "scripts/generate_text_render_cache_vectors.py", "--check"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_text_render_cache_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_render_cache",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

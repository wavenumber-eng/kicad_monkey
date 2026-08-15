"""Native render-cache contour topology and hole-fracture parity gate."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTORS = PACKAGE_ROOT / "tests/parity/text_topology_vectors.json"


def test_text_topology_records_are_current_and_cover_fracture_order() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    assert vectors["oracle"] == {
        "implementation": "kicad_monkey.kicad_render_cache private topology helpers",
        "outline_source": "tests/parity/text_contour_vectors.json",
        "semantics": [
            "trim_one_duplicate_closure",
            "containment_depth_parity",
            "leftmost_hole_bridge",
            "degenerate_rings_standalone_unless_holed",
        ],
    }
    records = {record["case_id"]: record for record in vectors["records"]}
    assert set(records) == {
        "duplicate_closure_and_short_contour",
        "degenerate_in_holed_group",
        "single_square_hole",
        "hole_before_exterior",
        "signed_zero_top_tie",
        "two_holes_sorted_by_leftmost_point",
        "disjoint_exteriors_with_holes",
        "kicad_stroke_o_outline",
    }
    assert len(records["single_square_hole"]["fractured"]) == 1
    # KiCad keeps collapsed rings as standalone outlines in hole-free sets and
    # loses them to Clipper2 `Simplify()` when the set fractures.
    assert records["duplicate_closure_and_short_contour"]["fractured"][1] == (
        [[9.0, 9.0], [10.0, 10.0]]
    )
    assert records["degenerate_in_holed_group"]["fractured"] == (
        records["single_square_hole"]["fractured"]
    )
    # Storage order must not change classification: the hole-first layout of
    # Noto faces fractures identically to the exterior-first layout.
    assert records["hole_before_exterior"]["fractured"] == (
        records["single_square_hole"]["fractured"]
    )
    assert len(records["two_holes_sorted_by_leftmost_point"]["fractured"][0]) > 12
    assert len(records["disjoint_exteriors_with_holes"]["fractured"]) == 2
    assert records["kicad_stroke_o_outline"]["source_case_id"] == (
        "multiple_contours_hole"
    )
    assert len(records["kicad_stroke_o_outline"]["fractured"]) == 1

    completed = subprocess.run(
        [sys.executable, "scripts/generate_text_topology_vectors.py", "--check"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_focused_native_text_topology_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_topology",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

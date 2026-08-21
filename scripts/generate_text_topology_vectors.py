"""Generate fixed render-cache contour topology and fracture records."""

from __future__ import annotations

import argparse
from collections.abc import Callable
import json
from pathlib import Path
from typing import Any, cast

from kicad_monkey import kicad_render_cache

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = ROOT / "tests/parity/text_topology_vectors.json"
CONTOUR_PATH = ROOT / "tests/parity/text_contour_vectors.json"

CASES: tuple[dict[str, Any], ...] = (
    {
        # KiCad keeps rings collapsed below three points as standalone
        # outlines at their stored position when the set is hole-free.
        "case_id": "duplicate_closure_and_short_contour",
        "contours": [
            [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]],
            [[9.0, 9.0], [10.0, 10.0]],
        ],
    },
    {
        # `Fracture()`'s Clipper2 `Simplify()` drops degenerate rings, so
        # they only survive in hole-free sets that skip `Fracture()`.
        "case_id": "degenerate_in_holed_group",
        "contours": [
            [[0.0, 0.0], [8.0, 0.0], [8.0, 8.0], [0.0, 8.0]],
            [[2.0, 2.0], [2.0, 6.0], [6.0, 6.0], [6.0, 2.0]],
            [[9.0, 9.0]],
        ],
    },
    {
        # Two horizontal min-y runs at equal y: Clipper2's DoTopOfScanbeam
        # defers horizontal maxima onto a LIFO stack processed right-to-left,
        # so the LEFTMOST run closes last and stores the Simplify seam
        # (Wavenumber 'W').
        "case_id": "two_horizontal_min_y_runs_seam_tie",
        "contours": [
            [
                [0.0, 0.0],
                [2.0, 0.0],
                [2.0, 4.0],
                [6.0, 4.0],
                [6.0, 0.0],
                [8.0, 0.0],
                [8.0, 10.0],
                [0.0, 10.0],
            ],
            [[3.0, 6.0], [5.0, 6.0], [5.0, 8.0], [3.0, 8.0]],
        ],
    },
    {
        # Two single-point min-y feet at equal y: non-horizontal maxima are
        # processed inline left-to-right, so the RIGHTMOST (largest-x) foot
        # closes last and stores the seam (tiny_tapeout FreeMono 'P').
        "case_id": "two_point_min_y_feet_seam_tie",
        "contours": [
            [
                [1.0, 0.0],
                [2.0, 4.0],
                [6.0, 4.0],
                [7.0, 0.0],
                [8.0, 10.0],
                [0.0, 10.0],
            ],
            [[3.0, 6.0], [5.0, 6.0], [5.0, 8.0], [3.0, 8.0]],
        ],
    },
    {
        "case_id": "single_square_hole",
        "contours": [
            [[0.0, 0.0], [8.0, 0.0], [8.0, 8.0], [0.0, 8.0]],
            [[2.0, 2.0], [2.0, 6.0], [6.0, 6.0], [6.0, 2.0]],
        ],
    },
    {
        # Noto faces store a glyph's holes before the owning exterior; the
        # classifier must not depend on storage order.
        "case_id": "hole_before_exterior",
        "contours": [
            [[2.0, 2.0], [2.0, 6.0], [6.0, 6.0], [6.0, 2.0]],
            [[0.0, 0.0], [8.0, 0.0], [8.0, 8.0], [0.0, 8.0]],
        ],
    },
    {
        "case_id": "signed_zero_top_tie",
        "contours": [
            [[0.0, -0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            [[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]],
        ],
    },
    {
        "case_id": "two_holes_sorted_by_leftmost_point",
        "contours": [
            [[0.0, 0.0], [12.0, 0.0], [12.0, 10.0], [0.0, 10.0]],
            [[7.0, 2.0], [7.0, 5.0], [10.0, 5.0], [10.0, 2.0]],
            [[2.0, 3.0], [2.0, 7.0], [5.0, 7.0], [5.0, 3.0]],
        ],
    },
    {
        "case_id": "disjoint_exteriors_with_holes",
        "contours": [
            [[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]],
            [[2.0, 2.0], [2.0, 4.0], [4.0, 4.0], [4.0, 2.0]],
            [[10.0, 0.0], [16.0, 0.0], [16.0, 6.0], [10.0, 6.0]],
            [[12.0, 2.0], [12.0, 4.0], [14.0, 4.0], [14.0, 2.0]],
        ],
    },
)


def generate_vectors() -> dict[str, Any]:
    fracture = cast(
        Callable[[list[list[tuple[float, float]]]], list[list[tuple[float, float]]]],
        getattr(kicad_render_cache, "_fracture_render_cache_contours"),
    )
    trim = cast(
        Callable[[list[tuple[float, float]]], list[tuple[float, float]]],
        getattr(kicad_render_cache, "_trim_repeated_closing_point"),
    )
    contour_vectors = json.loads(CONTOUR_PATH.read_text(encoding="utf-8"))
    outline_case = next(
        record
        for record in contour_vectors["records"]
        if record["case_id"] == "multiple_contours_hole"
    )
    cases = [
        *CASES,
        {
            "case_id": "kicad_stroke_o_outline",
            "source_case_id": "multiple_contours_hole",
            "contours": outline_case["contours"],
        },
    ]
    records = []
    for case in cases:
        contours = [
            [(float(point[0]), float(point[1])) for point in contour]
            for contour in case["contours"]
        ]
        normalized = [trim(contour) for contour in contours]
        records.append(
            {
                **case,
                "fractured": fracture(normalized),
            }
        )
    return {
        "oracle": {
            "implementation": "kicad_monkey.kicad_render_cache private topology helpers",
            "outline_source": "tests/parity/text_contour_vectors.json",
            "semantics": [
                "trim_one_duplicate_closure",
                "containment_depth_parity",
                "leftmost_hole_bridge",
                "degenerate_rings_standalone_unless_holed",
            ],
        },
        "records": records,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = (json.dumps(generate_vectors(), indent=2, sort_keys=True) + "\n").encode()
    if args.check:
        if OUTPUT_PATH.read_bytes() != encoded:
            raise SystemExit(f"stale text topology vectors: {OUTPUT_PATH}")
        return
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_bytes(encoded)


if __name__ == "__main__":
    main()

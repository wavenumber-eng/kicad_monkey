"""Generate fixed KiCad text Bézier decomposition records."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from kicad_monkey.kicad_text import KiCadTextRenderer

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = ROOT / "tests/parity/text_bezier_vectors.json"

CASES: tuple[dict[str, Any], ...] = (
    {
        "case_id": "quadratic_arch_default_error",
        "kind": "quadratic",
        "control": [[0.0, 0.0], [50.0, 100.0], [100.0, 0.0]],
        "max_error": 2.0,
    },
    {
        "case_id": "quadratic_arch_fine_error",
        "kind": "quadratic",
        "control": [[-25.5, 10.25], [80.75, 240.5], [175.125, -30.0]],
        "max_error": 0.25,
    },
    {
        "case_id": "quadratic_collinear",
        "kind": "quadratic",
        "control": [[0.0, 0.0], [50.0, 50.0], [100.0, 100.0]],
        "max_error": 2.0,
    },
    {
        "case_id": "cubic_arch_default_error",
        "kind": "cubic",
        "control": [[0.0, 0.0], [0.0, 100.0], [100.0, 100.0], [100.0, 0.0]],
        "max_error": 2.0,
    },
    {
        "case_id": "cubic_single_inflection",
        "kind": "cubic",
        "control": [[0.0, 0.0], [98.0, -169.0], [95.0, 99.0], [100.0, 0.0]],
        "max_error": 1.0,
    },
    {
        "case_id": "cubic_double_inflection",
        "kind": "cubic",
        "control": [[0.0, 0.0], [14.0, -165.0], [-77.0, -154.0], [100.0, 0.0]],
        "max_error": 1.0,
    },
    {
        "case_id": "cubic_fractional_cff_shape",
        "kind": "cubic",
        "control": [[850.25, 700.5], [700.125, 850.75], [300.5, 850.25], [150.0, 500.125]],
        "max_error": 2.0,
    },
    {
        "case_id": "cubic_nonpositive_uses_kicad_default",
        "kind": "cubic",
        "control": [[0.0, 0.0], [0.0, 100.0], [100.0, 100.0], [100.0, 0.0]],
        "max_error": 0.0,
    },
)


def generate_vectors() -> dict[str, Any]:
    records = []
    for case in CASES:
        control = [tuple(point) for point in case["control"]]
        points = KiCadTextRenderer._bezier_get_poly(control, case["max_error"])
        records.append(
            {
                **case,
                "comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 1.0e-10},
                "points": [[point[0], point[1]] for point in points],
            }
        )
    return {
        "oracle": {
            "implementation": "kicad_monkey.KiCadTextRenderer._bezier_get_poly",
            "kicad_source_algorithm": "common/font/outline_decomposer.cpp",
            "coordinate_space": "caller_units_f64",
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
            raise SystemExit(f"stale text Bézier vectors: {OUTPUT_PATH}")
        return
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_bytes(encoded)


if __name__ == "__main__":
    main()

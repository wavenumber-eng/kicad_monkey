"""Generate fixed typed render-cache composition and S-expression records."""

from __future__ import annotations

import argparse
from collections.abc import Callable
import json
from pathlib import Path
from typing import Any, cast

from kicad_monkey import kicad_render_cache
from kicad_monkey.kicad_primitives import RenderCache, RenderCachePolygon
from kicad_monkey.kicad_sexpr import build_sexp

ROOT = Path(__file__).resolve().parents[1]
LAYOUT_PATH = ROOT / "tests/parity/text_layout_vectors.json"
OUTPUT_PATH = ROOT / "tests/parity/text_render_cache_vectors.json"
CASES = (
    ("single_glyph_cache", "left_top_identity"),
    ("rotated_holed_glyph_cache", "right_bottom_mirrored_quarter_turn"),
)


def generate_vectors() -> dict[str, Any]:
    layout_vectors = json.loads(LAYOUT_PATH.read_text(encoding="utf-8"))
    layouts = {record["case_id"]: record for record in layout_vectors["records"]}
    fracture = cast(
        Callable[[list[list[tuple[float, float]]]], list[list[tuple[float, float]]]],
        getattr(kicad_render_cache, "_fracture_render_cache_contours"),
    )
    trim = cast(
        Callable[[list[tuple[float, float]]], list[tuple[float, float]]],
        getattr(kicad_render_cache, "_trim_repeated_closing_point"),
    )
    records = []
    for case_id, layout_case_id in CASES:
        layout = layouts[layout_case_id]
        contours = [
            [(float(point[0]), float(point[1])) for point in contour]
            for contour in layout["contours"]
        ]
        fractured = fracture([trim(contour) for contour in contours])
        cache = RenderCache(
            text=str(layout["shaping"]["text"]),
            angle=float(layout["angle_degrees"]),
            polygons=[RenderCachePolygon(points=points) for points in fractured],
        )
        records.append(
            {
                "case_id": case_id,
                "layout_case_id": layout_case_id,
                "layout": layout,
                "polygons": fractured,
                "sexpr": build_sexp(cache.to_sexp()),
            }
        )

    probe = RenderCache(
        text='A"\\\n',
        angle=37.5,
        polygons=[
            RenderCachePolygon(
                contours=[
                    [(1.0, -0.0), (2.25, 3.0), (4.0, 5.5)],
                    [(1.5, 1.0), (1.75, 1.5), (2.0, 1.0)],
                ]
            )
        ],
    )
    return {
        "oracle": {
            "composition": "accepted Python layout contours plus kicad_render_cache fracture",
            "serialization": "RenderCache.to_sexp plus kicad_sexpr.build_sexp",
            "kicad_revision": "d6ff4c23641ee5236b7c9fac19eb6af1849294f5",
            "kicad_writer": "pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp::formatRenderCache",
        },
        "records": records,
        "serialization_probe": {
            "text": probe.text,
            "angle_degrees": probe.angle,
            "polygons": [
                [contour.points for contour in polygon.contours]
                for polygon in probe.polygons
            ],
            "sexpr": build_sexp(probe.to_sexp()),
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = (json.dumps(generate_vectors(), indent=2, sort_keys=True) + "\n").encode()
    if args.check:
        if OUTPUT_PATH.read_bytes() != encoded:
            raise SystemExit(f"stale text render-cache vectors: {OUTPUT_PATH}")
        return
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_bytes(encoded)


if __name__ == "__main__":
    main()

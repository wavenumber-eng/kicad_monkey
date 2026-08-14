"""Generate fixed single-line KiCad text alignment and transform records."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

from kicad_monkey.kicad_geometry import HAlign, TextParams, VAlign
from kicad_monkey.kicad_text import KiCadTextRenderer

ROOT = Path(__file__).resolve().parents[1]
CONTOUR_PATH = ROOT / "tests/parity/text_contour_vectors.json"
OUTPUT_PATH = ROOT / "tests/parity/text_layout_vectors.json"

CASES: tuple[dict[str, Any], ...] = (
    {
        "case_id": "left_top_identity",
        "base_case_id": "single_glyph_origin",
        "position_x": 0.0,
        "position_y": 0.0,
        "horizontal_alignment": "left",
        "vertical_alignment": "top",
        "angle_degrees": 0.0,
        "mirrored": False,
    },
    {
        "case_id": "center_center_rotated",
        "base_case_id": "kerning_pair_anisotropic_offset",
        "position_x": 2.5,
        "position_y": -1.25,
        "horizontal_alignment": "center",
        "vertical_alignment": "center",
        "angle_degrees": 37.0,
        "mirrored": False,
    },
    {
        "case_id": "right_bottom_mirrored_quarter_turn",
        "base_case_id": "multiple_contours_hole",
        "position_x": -3.0,
        "position_y": 4.0,
        "horizontal_alignment": "right",
        "vertical_alignment": "bottom",
        "angle_degrees": 90.0,
        "mirrored": True,
    },
    {
        "case_id": "right_top_mirrored_negative_rotation",
        "base_case_id": "missing_outline_space_advances",
        "position_x": 1.0,
        "position_y": 2.0,
        "horizontal_alignment": "right",
        "vertical_alignment": "top",
        "angle_degrees": -405.0,
        "mirrored": True,
    },
)


def generate_vectors() -> dict[str, Any]:
    contour_vectors = json.loads(CONTOUR_PATH.read_text(encoding="utf-8"))
    contours_by_id = {
        record["case_id"]: record for record in contour_vectors["records"]
    }
    return {
        "oracle": {
            "implementation": "kicad_monkey.KiCadTextRenderer alignment helpers",
            "transform_order": ["alignment", "mirror_x", "clockwise_rotation"],
            "rotation_origin": "authored_text_position",
            "height_fudge_factor": 1.17,
        },
        "font": contour_vectors["font"],
        "records": [_case(contours_by_id, case) for case in CASES],
    }


def _case(
    contours_by_id: dict[str, dict[str, Any]], case: dict[str, Any]
) -> dict[str, Any]:
    base = contours_by_id[str(case["base_case_id"])]
    h_align = {
        "left": HAlign.LEFT,
        "center": HAlign.CENTER,
        "right": HAlign.RIGHT,
    }[str(case["horizontal_alignment"])]
    v_align = {
        "top": VAlign.TOP,
        "center": VAlign.CENTER,
        "bottom": VAlign.BOTTOM,
    }[str(case["vertical_alignment"])]
    params = TextParams(
        text=str(base["shaping"]["text"]),
        size_x=float(base["size_x"]),
        size_y=float(base["size_y"]),
        position_x=float(case["position_x"]),
        position_y=float(case["position_y"]),
        angle=float(case["angle_degrees"]),
        mirrored=bool(case["mirrored"]),
        h_align=h_align,
        v_align=v_align,
    )
    horizontal_offset = KiCadTextRenderer._horizontal_line_offset(
        params, float(base["advance_x"])
    )
    vertical_offset = KiCadTextRenderer._vertical_line_offset(
        params, line_count=1, interline=0.0
    )
    radians = math.radians(params.angle)
    cos_angle = math.cos(radians)
    sin_angle = math.sin(radians)
    transformed: list[list[list[float]]] = []
    for contour in base["contours"]:
        points: list[list[float]] = []
        for point in contour:
            x = (
                float(point[0])
                - float(base["origin_x"])
                + params.position_x
                + horizontal_offset
            )
            y = (
                float(point[1])
                - float(base["origin_y"])
                + params.position_y
                + vertical_offset
            )
            x, y = KiCadTextRenderer._transform_rendered_text_point(
                params,
                cos_a=cos_angle,
                sin_a=sin_angle,
                x=x,
                y=y,
            )
            points.append([x, y])
        transformed.append(points)
    return {
        **case,
        "size_x": base["size_x"],
        "size_y": base["size_y"],
        "max_error": base["max_error"],
        "shaping": base["shaping"],
        "comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 1.0e-9},
        "advance_x": base["advance_x"],
        "advance_y": base["advance_y"],
        "contours": transformed,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = (json.dumps(generate_vectors(), indent=2, sort_keys=True) + "\n").encode()
    if args.check:
        if OUTPUT_PATH.read_bytes() != encoded:
            raise SystemExit(f"stale text layout vectors: {OUTPUT_PATH}")
        return
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_bytes(encoded)


if __name__ == "__main__":
    main()

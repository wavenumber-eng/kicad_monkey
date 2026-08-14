"""Generate fixed shaping-to-contour records for the native Rust text path."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path
from typing import Any, cast

from fontTools.pens.basePen import BasePen
from fontTools.ttLib import TTFont
import uharfbuzz as hb

from kicad_monkey.kicad_text import KiCadTextRenderer

ROOT = Path(__file__).resolve().parents[1]
FONT_PATH = ROOT / "assets/fonts/kicad-stroke.ttf"
OUTPUT_PATH = ROOT / "tests/parity/text_contour_vectors.json"
FACE_SCALER = 1433.0
SIZE_COMPENSATION = 1.4
HB = cast(Any, hb)


class _ContourPen(BasePen):  # type: ignore[type-arg]
    def __init__(self, glyph_set: Any, *, upem: int) -> None:
        super().__init__(glyph_set)
        self.upem = upem
        self.contours: list[list[tuple[float, float]]] = []
        self.current: list[tuple[float, float]] = []
        self.last = (0.0, 0.0)
        self.start: tuple[float, float] | None = None
        self.command_count = 0

    def _internal(self, point: tuple[float, float]) -> tuple[float, float]:
        return (
            float(point[0]) * FACE_SCALER / self.upem,
            float(point[1]) * FACE_SCALER / self.upem,
        )

    def _push(self, point: tuple[float, float]) -> None:
        if not self.current or self.current[-1] != point:
            self.current.append(point)

    def _flush(self) -> None:
        if self.current:
            self.contours.append(self.current)
            self.current = []

    def _moveTo(self, pt: tuple[float, float]) -> None:
        self.command_count += 1
        self._flush()
        self.last = self._internal(pt)
        self.start = self.last
        self._push(self.last)

    def _lineTo(self, pt: tuple[float, float]) -> None:
        self.command_count += 1
        self.last = self._internal(pt)
        self._push(self.last)

    def _qCurveToOne(
        self, pt1: tuple[float, float], pt2: tuple[float, float]
    ) -> None:
        self.command_count += 1
        control_internal = self._internal(pt1)
        end_internal = self._internal(pt2)
        for point in KiCadTextRenderer._bezier_get_poly(
            [self.last, control_internal, end_internal], 2.0
        ):
            self._push(point)
        self.last = end_internal

    def _curveToOne(
        self,
        pt1: tuple[float, float],
        pt2: tuple[float, float],
        pt3: tuple[float, float],
    ) -> None:
        self.command_count += 1
        control1_internal = self._internal(pt1)
        control2_internal = self._internal(pt2)
        end_internal = self._internal(pt3)
        for point in KiCadTextRenderer._bezier_get_poly(
            [self.last, control1_internal, control2_internal, end_internal], 2.0
        ):
            self._push(point)
        self.last = end_internal

    def _closePath(self) -> None:
        self.command_count += 1
        if self.start is not None and self.last != self.start:
            self._push(self.start)
        self._flush()
        self.start = None

    def _endPath(self) -> None:
        self.command_count += 1
        self._flush()


CASES: tuple[dict[str, Any], ...] = (
    {
        "case_id": "single_glyph_origin",
        "text": "A",
        "size_x": 1.0,
        "size_y": 1.0,
        "origin_x": 0.0,
        "origin_y": 0.0,
    },
    {
        "case_id": "kerning_pair_anisotropic_offset",
        "text": "AV",
        "size_x": 1.3,
        "size_y": 0.8,
        "origin_x": 2.5,
        "origin_y": -1.25,
    },
    {
        "case_id": "multiple_contours_hole",
        "text": "O",
        "size_x": 2.0,
        "size_y": 1.5,
        "origin_x": -3.0,
        "origin_y": 4.0,
    },
    {
        "case_id": "missing_outline_space_advances",
        "text": "A A",
        "size_x": 0.75,
        "size_y": 1.25,
        "origin_x": 1.0,
        "origin_y": 2.0,
    },
)


def generate_vectors() -> dict[str, Any]:
    font_bytes = FONT_PATH.read_bytes()
    digest = hashlib.sha256(font_bytes).hexdigest()
    font = TTFont(BytesIO(font_bytes), lazy=False)
    upem = int(cast(Any, font["head"]).unitsPerEm)
    hb_face = HB.Face(font_bytes)
    records = [_case(font, hb_face, digest, upem, case) for case in CASES]
    return {
        "oracle": {
            "shaping_engine": "uharfbuzz",
            "outline_engine": "fontTools.BasePen",
            "curve_engine": "kicad_monkey.KiCadTextRenderer._bezier_get_poly",
            "coordinate_space": "positioned_text_run_units_y_down",
            "hinting": "none",
        },
        "font": {
            "font_id": "kicad_stroke_regular",
            "font_path": "assets/fonts/kicad-stroke.ttf",
            "font_sha256": digest,
            "face_index": 0,
            "units_per_em": upem,
        },
        "records": records,
    }


def _case(
    font: TTFont,
    hb_face: Any,
    digest: str,
    upem: int,
    case: dict[str, Any],
) -> dict[str, Any]:
    hb_font = HB.Font(hb_face)
    hb_font.scale = (upem, upem)
    buffer = HB.Buffer()
    buffer.add_utf8(str(case["text"]).encode())
    buffer.direction = "ltr"
    buffer.script = "Latn"
    buffer.language = "en"
    buffer.cluster_level = HB.BufferClusterLevel.MONOTONE_GRAPHEMES
    buffer.flags = HB.BufferFlags.BOT | HB.BufferFlags.EOT
    HB.shape(hb_font, buffer)

    final_contours: list[list[list[float]]] = []
    cursor_x = 0.0
    cursor_y = 0.0
    internal_to_x = float(case["size_x"]) / FACE_SCALER * SIZE_COMPENSATION
    internal_to_y = float(case["size_y"]) / FACE_SCALER * SIZE_COMPENSATION
    glyph_set = font.getGlyphSet()
    for info, position in zip(
        buffer.glyph_infos, buffer.glyph_positions, strict=True
    ):
        glyph_name = font.getGlyphName(info.codepoint)
        pen = _ContourPen(glyph_set, upem=upem)
        font["glyf"][glyph_name].draw(pen, font["glyf"])
        pen._flush()
        positioned_x = cursor_x + float(position.x_offset)
        positioned_y = cursor_y + float(position.y_offset)
        for contour in pen.contours:
            final_contours.append(
                [
                    [
                        float(case["origin_x"])
                        + (point[0] + positioned_x * FACE_SCALER / upem)
                        * internal_to_x,
                        float(case["origin_y"])
                        - (point[1] + positioned_y * FACE_SCALER / upem)
                        * internal_to_y,
                    ]
                    for point in contour
                ]
            )
        cursor_x += float(position.x_advance)
        cursor_y += float(position.y_advance)

    shaping = {
        "font_id": "kicad_stroke_regular",
        "font_sha256": digest,
        "face_index": 0,
        "variations": [],
        "text": case["text"],
        "text_index_unit": "utf8_byte_offset",
        "scale_x": upem,
        "scale_y": upem,
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "features": [],
        "buffer_properties": {
            "cluster_level": "monotone_graphemes",
            "beginning_of_text": True,
            "end_of_text": True,
            "default_ignorables": "normal",
            "do_not_insert_dotted_circle": False,
            "produce_unsafe_to_concat": False,
            "produce_safe_to_insert_tatweel": False,
        },
    }
    return {
        **case,
        "max_error": 2.0,
        "comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 1.0e-9},
        "shaping": shaping,
        "advance_x": cursor_x / upem * float(case["size_x"]) * SIZE_COMPENSATION,
        "advance_y": -cursor_y / upem * float(case["size_y"]) * SIZE_COMPENSATION,
        "contours": final_contours,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = (json.dumps(generate_vectors(), indent=2, sort_keys=True) + "\n").encode()
    if args.check:
        if OUTPUT_PATH.read_bytes() != encoded:
            raise SystemExit(f"stale text contour vectors: {OUTPUT_PATH}")
        return
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_bytes(encoded)


if __name__ == "__main__":
    main()

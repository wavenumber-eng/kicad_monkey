"""Generate fixed-font HarfBuzz shaping records for native Rust parity."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any, cast

import uharfbuzz as _hb

hb = cast(Any, _hb)

ROOT = Path(__file__).resolve().parents[1]
FONT_PATH = ROOT / "assets/fonts/kicad-stroke.ttf"
OUTPUT_PATH = ROOT / "tests/parity/font_shaping_a0_vectors.json"


CASES: tuple[dict[str, Any], ...] = (
    {
        "case_id": "kicad_stroke_latin_ffi",
        "text": "ffi",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "cluster_level": "monotone_graphemes",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
    {
        "case_id": "kicad_stroke_unicode_combining",
        "text": "Aé\u0301B",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "cluster_level": "monotone_graphemes",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": True,
        "produce_safe_to_insert_tatweel": True,
    },
    {
        "case_id": "kicad_stroke_scaled_rtl",
        "text": "ABCD",
        "direction": "right_to_left",
        "script": "Latn",
        "language": "en",
        "scale": (1375, 625),
        "features": [{"tag": "kern", "value": 0, "start": 0, "end": 4_294_967_295}],
        "cluster_level": "monotone_characters",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": True,
        "produce_safe_to_insert_tatweel": False,
    },
    {
        "case_id": "kicad_stroke_remove_default_ignorable",
        "text": "A\u200dB",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "cluster_level": "characters",
        "default_ignorables": "remove",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
    {
        "case_id": "kicad_stroke_preserve_default_ignorable",
        "text": "A\u200dB",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "cluster_level": "characters",
        "default_ignorables": "preserve",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
)


def generate_vectors() -> dict[str, Any]:
    font_bytes = FONT_PATH.read_bytes()
    digest = hashlib.sha256(font_bytes).hexdigest()
    face = hb.Face(font_bytes, 0)
    records = [_shape_case(face, digest, case) for case in CASES]
    return {
        "oracle": {
            "engine": "uharfbuzz",
            "harfbuzz_version": hb.version_string(),
            "font_path": "assets/fonts/kicad-stroke.ttf",
            "font_sha256": digest,
            "face_index": 0,
            "units_per_em": face.upem,
            "text_input_api": "hb_buffer_add_utf8",
        },
        "records": records,
    }


def _shape_case(face: Any, digest: str, case: dict[str, Any]) -> dict[str, Any]:
    font = hb.Font(face)
    font.scale = case["scale"]
    buffer = hb.Buffer()
    buffer.add_utf8(case["text"].encode())
    buffer.direction = {
        "left_to_right": "ltr",
        "right_to_left": "rtl",
        "top_to_bottom": "ttb",
        "bottom_to_top": "btt",
    }[case["direction"]]
    buffer.script = case["script"]
    buffer.language = case["language"]
    buffer.cluster_level = {
        "monotone_graphemes": hb.BufferClusterLevel.MONOTONE_GRAPHEMES,
        "monotone_characters": hb.BufferClusterLevel.MONOTONE_CHARACTERS,
        "characters": hb.BufferClusterLevel.CHARACTERS,
    }[case["cluster_level"]]
    buffer.flags = _buffer_flags(case)
    features = {
        feature["tag"]: feature["value"] for feature in case["features"]
    }
    hb.shape(font, buffer, features or None)
    glyphs = [
        {
            "glyph_id": info.codepoint,
            "cluster": info.cluster,
            "x_advance": position.x_advance,
            "y_advance": position.y_advance,
            "x_offset": position.x_offset,
            "y_offset": position.y_offset,
            "unsafe_to_break": bool(info.flags & hb.GlyphFlags.UNSAFE_TO_BREAK),
            "safe_to_insert_tatweel": bool(
                info.flags & hb.GlyphFlags.SAFE_TO_INSERT_TATWEEL
            ),
            "unsafe_to_concat": bool(info.flags & hb.GlyphFlags.UNSAFE_TO_CONCAT),
        }
        for info, position in zip(buffer.glyph_infos, buffer.glyph_positions, strict=True)
    ]
    properties = {
        "cluster_level": case["cluster_level"],
        "beginning_of_text": True,
        "end_of_text": True,
        "default_ignorables": case["default_ignorables"],
        "do_not_insert_dotted_circle": False,
        "produce_unsafe_to_concat": case["produce_unsafe_to_concat"],
        "produce_safe_to_insert_tatweel": case["produce_safe_to_insert_tatweel"],
    }
    return {
        "schema": "kicad_monkey.shaping_record.a0",
        "type": "kicad_monkey.shaping_record",
        "version": "a0",
        "case_id": case["case_id"],
        "comparison": {"mode": "exact"},
        "input": {
            "font_id": "kicad_stroke_regular",
            "font_sha256": digest,
            "face_index": 0,
            "variations": [],
            "text": case["text"],
            "text_index_unit": "utf8_byte_offset",
            "scale_x": case["scale"][0],
            "scale_y": case["scale"][1],
            "direction": case["direction"],
            "script": case["script"],
            "language": case["language"],
            "features": case["features"],
            "buffer_properties": properties,
        },
        "glyphs": glyphs,
    }


def _buffer_flags(case: dict[str, Any]) -> Any:
    flags = hb.BufferFlags.BOT | hb.BufferFlags.EOT
    if case["default_ignorables"] == "preserve":
        flags |= hb.BufferFlags.PRESERVE_DEFAULT_IGNORABLES
    elif case["default_ignorables"] == "remove":
        flags |= hb.BufferFlags.REMOVE_DEFAULT_IGNORABLES
    if case["produce_unsafe_to_concat"]:
        flags |= hb.BufferFlags.PRODUCE_UNSAFE_TO_CONCAT
    if case["produce_safe_to_insert_tatweel"]:
        flags |= hb.BufferFlags.PRODUCE_SAFE_TO_INSERT_TATWEEL
    return flags


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    rendered = json.dumps(generate_vectors(), ensure_ascii=False, indent=2) + "\n"
    if args.check:
        return 0 if OUTPUT_PATH.read_text(encoding="utf-8") == rendered else 1
    OUTPUT_PATH.write_text(rendered, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

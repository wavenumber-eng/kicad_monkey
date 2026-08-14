"""Generate fixed HarfBuzz shaping evidence for native Rust parity."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path
from typing import Any, cast

from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.TupleVariation import TupleVariation
import uharfbuzz as _hb

hb = cast(Any, _hb)

ROOT = Path(__file__).resolve().parents[1]
FONT_PATH = ROOT / "assets/fonts/kicad-stroke.ttf"
OUTPUT_PATH = ROOT / "tests/parity/font_shaping_a0_vectors.json"
VARIABLE_FONT_PATH = ROOT / "tests/parity/fonts/shaping-variable-fixture.ttf"


CASES: tuple[dict[str, Any], ...] = (
    {
        "case_id": "kicad_stroke_latin_ffi",
        "text": "ffi",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "variations": [],
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
        "variations": [],
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
        "variations": [],
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
        "variations": [],
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
        "variations": [],
        "cluster_level": "characters",
        "default_ignorables": "preserve",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
    {
        "case_id": "kicad_stroke_arabic_unsafe_concat",
        "text": "\u0627\u0628",
        "direction": "right_to_left",
        "script": "Arab",
        "language": "ar",
        "scale": (1000, 1000),
        "features": [],
        "variations": [],
        "cluster_level": "monotone_graphemes",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": True,
        "produce_safe_to_insert_tatweel": True,
    },
    {
        "case_id": "fixture_non_global_utf8_feature",
        "font_id": "shaping_variable_fixture",
        "font_path": "tests/parity/fonts/shaping-variable-fixture.ttf",
        "text": "\u00e9AB",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [{"tag": "dlig", "value": 1, "start": 2, "end": 4}],
        "variations": [],
        "cluster_level": "monotone_characters",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
    {
        "case_id": "fixture_default_variation_axis",
        "font_id": "shaping_variable_fixture",
        "font_path": "tests/parity/fonts/shaping-variable-fixture.ttf",
        "text": "A",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "variations": [],
        "cluster_level": "monotone_graphemes",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
    {
        "case_id": "fixture_supported_variation_axis",
        "font_id": "shaping_variable_fixture",
        "font_path": "tests/parity/fonts/shaping-variable-fixture.ttf",
        "text": "A",
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "scale": (1000, 1000),
        "features": [],
        "variations": [{"axis": "wght", "value": 700.0}],
        "cluster_level": "monotone_graphemes",
        "default_ignorables": "normal",
        "produce_unsafe_to_concat": False,
        "produce_safe_to_insert_tatweel": False,
    },
)

OPTIONAL_FLAG_CASE: dict[str, Any] = {
    "case_id": "kicad_stroke_arabic_safe_tatweel",
    "text": "\u0644\u0627",
    "direction": "right_to_left",
    "script": "Arab",
    "language": "ar",
    "scale": (1000, 1000),
    "features": [],
    "variations": [],
    "cluster_level": "monotone_graphemes",
    "default_ignorables": "normal",
    "produce_unsafe_to_concat": True,
    "produce_safe_to_insert_tatweel": True,
}


def generate_vectors() -> dict[str, Any]:
    font_buffers = {
        "assets/fonts/kicad-stroke.ttf": FONT_PATH.read_bytes(),
        "tests/parity/fonts/shaping-variable-fixture.ttf": _variable_font_fixture(),
    }
    faces = {path: hb.Face(data, 0) for path, data in font_buffers.items()}
    records = [
        _shape_case(
            faces[case.get("font_path", "assets/fonts/kicad-stroke.ttf")],
            hashlib.sha256(
                font_buffers[case.get("font_path", "assets/fonts/kicad-stroke.ttf")]
            ).hexdigest(),
            case,
        )
        for case in CASES
    ]
    stroke_bytes = font_buffers["assets/fonts/kicad-stroke.ttf"]
    optional_flag_record = _shape_case(
        faces["assets/fonts/kicad-stroke.ttf"],
        hashlib.sha256(stroke_bytes).hexdigest(),
        OPTIONAL_FLAG_CASE,
    )
    return {
        "oracle": {
            "engine": "uharfbuzz",
            "harfbuzz_version": hb.version_string(),
            "text_input_api": "hb_buffer_add_utf8",
        },
        "fonts": [
            {
                "font_id": "kicad_stroke_regular",
                "font_path": "assets/fonts/kicad-stroke.ttf",
                "font_sha256": hashlib.sha256(font_buffers["assets/fonts/kicad-stroke.ttf"]).hexdigest(),
                "face_index": 0,
                "units_per_em": faces["assets/fonts/kicad-stroke.ttf"].upem,
            },
            {
                "font_id": "shaping_variable_fixture",
                "font_path": "tests/parity/fonts/shaping-variable-fixture.ttf",
                "font_sha256": hashlib.sha256(font_buffers["tests/parity/fonts/shaping-variable-fixture.ttf"]).hexdigest(),
                "face_index": 0,
                "units_per_em": faces["tests/parity/fonts/shaping-variable-fixture.ttf"].upem,
            },
        ],
        "records": records,
        "versioned_optional_flag_evidence": {
            "case_id": optional_flag_record["case_id"],
            "comparison": "geometry_exact_flag_presence",
            "input": optional_flag_record["input"],
            "uharfbuzz_glyphs": optional_flag_record["glyphs"],
        },
    }


def _shape_case(face: Any, digest: str, case: dict[str, Any]) -> dict[str, Any]:
    font = hb.Font(face)
    font.scale = case["scale"]
    if case["variations"]:
        font.set_variations(
            {variation["axis"]: variation["value"] for variation in case["variations"]}
        )
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
    features = _ordered_ranged_features(case["features"])
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
            "font_id": case.get("font_id", "kicad_stroke_regular"),
            "font_sha256": digest,
            "face_index": 0,
            "variations": case["variations"],
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


def _ordered_ranged_features(
    features: list[dict[str, int | str]],
) -> dict[str, list[tuple[int, int, int]]]:
    """Preserve each feature range in insertion order; duplicate tags fail closed."""
    shaped: dict[str, list[tuple[int, int, int]]] = {}
    for feature in features:
        feature_tag = str(feature["tag"])
        if feature_tag in shaped:
            raise ValueError(f"duplicate feature tag cannot preserve C-array order: {feature_tag}")
        shaped[feature_tag] = [
            (int(feature["start"]), int(feature["end"]), int(feature["value"]))
        ]
    return shaped


def _variable_font_fixture() -> bytes:
    builder = FontBuilder(1000, isTTF=True)
    glyph_order = [".notdef", "A", "B", "AB", "eacute"]
    builder.setupGlyphOrder(glyph_order)
    glyphs = {name: _fixture_glyph(index) for index, name in enumerate(glyph_order)}
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics({name: (500, 0) for name in glyph_order})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupCharacterMap({0x0041: "A", 0x0042: "B", 0x00E9: "eacute"})
    builder.setupNameTable(
        {
            "familyName": "KiCad Monkey Shaping Fixture",
            "styleName": "Regular",
            "uniqueFontIdentifier": "KiCadMonkeyShapingFixture-Regular",
            "fullName": "KiCad Monkey Shaping Fixture Regular",
            "psName": "KiCadMonkeyShapingFixture-Regular",
            "version": "Version 1.000",
        }
    )
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupMaxp()
    builder.setupFvar([("wght", 100, 400, 900, "Weight")], [])
    builder.setupGvar(
        {
            "A": [
                TupleVariation(
                    {"wght": (0.0, 1.0, 1.0)},
                    [(0, 0)] * 3
                    + [(0, 0), (200, 0), (0, 0), (0, 0)],
                )
            ]
        }
    )
    addOpenTypeFeaturesFromString(
        builder.font,
        "feature dlig { sub A B by AB; } dlig;",
    )
    builder.font.recalcTimestamp = False
    head = cast(Any, builder.font["head"])
    head.created = 0
    head.modified = 0
    output = BytesIO()
    builder.save(output)
    return output.getvalue()


def _fixture_glyph(index: int) -> Any:
    pen = TTGlyphPen(None)
    if index:
        left = 50 + index * 10
        pen.moveTo((left, 0))
        pen.lineTo((450, 0))
        pen.lineTo((250, 700 - index * 10))
        pen.closePath()
    return pen.glyph()


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
    fixture = _variable_font_fixture()
    if args.check:
        current_fixture = VARIABLE_FONT_PATH.read_bytes() if VARIABLE_FONT_PATH.exists() else b""
        return 0 if OUTPUT_PATH.read_text(encoding="utf-8") == rendered and current_fixture == fixture else 1
    VARIABLE_FONT_PATH.parent.mkdir(parents=True, exist_ok=True)
    VARIABLE_FONT_PATH.write_bytes(fixture)
    OUTPUT_PATH.write_text(rendered, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

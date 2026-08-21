"""Generate deterministic glyph-outline evidence for native Rust parity."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path
from typing import Any, cast

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.basePen import BasePen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTCollection, TTFont
from fontTools.ttLib.tables.TupleVariation import TupleVariation
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[1]
STROKE_FONT_PATH = ROOT / "assets/fonts/kicad-stroke.ttf"
VARIABLE_FONT_PATH = ROOT / "tests/parity/fonts/outline-variable-fixture.ttf"
CFF_FONT_PATH = ROOT / "tests/parity/fonts/outline-cff-fixture.otf"
CFF2_FONT_PATH = ROOT / "tests/parity/fonts/outline-cff2-fixture.otf"
COMPOSITE_FONT_PATH = ROOT / "tests/parity/fonts/outline-composite-fixture.ttf"
COLLECTION_FONT_PATH = ROOT / "tests/parity/fonts/outline-collection-fixture.ttc"
OUTPUT_PATH = ROOT / "tests/parity/font_outline_a0_vectors.json"


class _CommandPen(BasePen):  # type: ignore[type-arg]
    def __init__(self, glyph_set: Any, *, expand_close: bool) -> None:
        super().__init__(glyph_set)
        self.commands: list[dict[str, Any]] = []
        self._expand_close = expand_close
        self._contour_start: tuple[float, float] | None = None
        self._current: tuple[float, float] | None = None

    def _moveTo(self, pt: tuple[float, float]) -> None:
        self.commands.append(_point_command("move_to", pt))
        self._contour_start = pt
        self._current = pt

    def _lineTo(self, pt: tuple[float, float]) -> None:
        self.commands.append(_point_command("line_to", pt))
        self._current = pt

    def _qCurveToOne(
        self, pt1: tuple[float, float], pt2: tuple[float, float]
    ) -> None:
        self.commands.append(
            {
                "kind": "quad_to",
                "control_x": _coordinate(pt1[0]),
                "control_y": _coordinate(pt1[1]),
                "x": _coordinate(pt2[0]),
                "y": _coordinate(pt2[1]),
            }
        )
        self._current = pt2

    def _curveToOne(
        self,
        pt1: tuple[float, float],
        pt2: tuple[float, float],
        pt3: tuple[float, float],
    ) -> None:
        self.commands.append(
            {
                "kind": "curve_to",
                "control1_x": _coordinate(pt1[0]),
                "control1_y": _coordinate(pt1[1]),
                "control2_x": _coordinate(pt2[0]),
                "control2_y": _coordinate(pt2[1]),
                "x": _coordinate(pt3[0]),
                "y": _coordinate(pt3[1]),
            }
        )
        self._current = pt3

    def _closePath(self) -> None:
        # ttf-parser expands the implicit closing edge before `close`; make
        # that semantic segment explicit in the language-neutral oracle too.
        if (
            self._expand_close
            and self._contour_start is not None
            and self._current != self._contour_start
        ):
            self.commands.append(_point_command("line_to", self._contour_start))
        self.commands.append({"kind": "close"})
        self._contour_start = None
        self._current = None

    def _endPath(self) -> None:
        raise ValueError("open contours are not valid promoted outline evidence")


def generate_vectors() -> dict[str, Any]:
    font_buffers = {
        "assets/fonts/kicad-stroke.ttf": STROKE_FONT_PATH.read_bytes(),
        "tests/parity/fonts/outline-variable-fixture.ttf": _variable_font_fixture(),
        "tests/parity/fonts/outline-cff-fixture.otf": _cff_font_fixture(),
        "tests/parity/fonts/outline-cff2-fixture.otf": _cff2_font_fixture(),
        "tests/parity/fonts/outline-composite-fixture.ttf": _composite_font_fixture(),
        "tests/parity/fonts/outline-collection-fixture.ttc": _collection_font_fixture(),
    }
    cases = (
        {
            "case_id": "kicad_stroke_line_outline",
            "font_id": "kicad_stroke_regular",
            "font_path": "assets/fonts/kicad-stroke.ttf",
            "glyph_name": "uni0041",
            "variations": [],
            "coordinate_comparison": {"mode": "exact"},
        },
        {
            "case_id": "variable_quadratic_default",
            "font_id": "outline_variable_fixture",
            "font_path": "tests/parity/fonts/outline-variable-fixture.ttf",
            "glyph_name": "A",
            "variations": [],
            "coordinate_comparison": {"mode": "exact"},
        },
        {
            "case_id": "variable_quadratic_weight_700",
            "font_id": "outline_variable_fixture",
            "font_path": "tests/parity/fonts/outline-variable-fixture.ttf",
            "glyph_name": "A",
            "variations": [{"axis": "wght", "value": 700.0}],
            "coordinate_comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 0.0001},
        },
        {
            "case_id": "cff_cubic_outline",
            "font_id": "outline_cff_fixture",
            "font_path": "tests/parity/fonts/outline-cff-fixture.otf",
            "glyph_name": "C",
            "variations": [],
            "coordinate_comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 0.0001},
        },
        {
            "case_id": "cff2_cubic_outline",
            "font_id": "outline_cff2_fixture",
            "font_path": "tests/parity/fonts/outline-cff2-fixture.otf",
            "glyph_name": "S",
            "variations": [{"axis": "wght", "value": 700.0}],
            "coordinate_comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 0.003},
        },
        {
            "case_id": "transformed_composite_glyf",
            "font_id": "outline_composite_fixture",
            "font_path": "tests/parity/fonts/outline-composite-fixture.ttf",
            "glyph_name": "X",
            "variations": [],
            "coordinate_comparison": {"mode": "absolute_tolerance", "absolute_tolerance": 0.0001},
        },
        {
            "case_id": "collection_second_face",
            "font_id": "outline_collection_second_face",
            "font_path": "tests/parity/fonts/outline-collection-fixture.ttc",
            "face_index": 1,
            "glyph_name": "C",
            "variations": [],
            "coordinate_comparison": {"mode": "exact"},
        },
    )
    records = [
        _outline_case(font_buffers[str(case["font_path"])], case) for case in cases
    ]
    fonts_by_id: dict[str, dict[str, Any]] = {}
    for case, record in zip(cases, records, strict=True):
        fonts_by_id.setdefault(
            str(record["font_id"]),
            {
                "font_id": record["font_id"],
                "font_path": case["font_path"],
                "font_sha256": record["font_sha256"],
                "face_index": record["face_index"],
                "units_per_em": record["units_per_em"],
            },
        )
    return {
        "oracle": {
            "engine": "fontTools",
            "coordinate_space": "unscaled_font_design_units",
            "quadratic_api": "BasePen._qCurveToOne",
            "cubic_api": "BasePen._curveToOne",
        },
        "fonts": list(fonts_by_id.values()),
        "records": records,
    }


def _outline_case(font_bytes: bytes, case: dict[str, Any]) -> dict[str, Any]:
    face_index = int(case.get("face_index", 0))
    font = TTFont(BytesIO(font_bytes), fontNumber=face_index, lazy=False)
    location = {
        str(variation["axis"]): float(variation["value"])
        for variation in case["variations"]
    }
    glyph_name = str(case["glyph_name"])
    glyph_id = font.getGlyphID(glyph_name)
    if location:
        font = instantiateVariableFont(font, location, inplace=False)
    glyph_set = font.getGlyphSet()
    pen = _CommandPen(glyph_set, expand_close="glyf" in font)
    if "glyf" in font:
        # Draw raw glyf coordinates. getGlyphSet intentionally reconciles the
        # phantom left side bearing, while ttf-parser exposes design points.
        font["glyf"][glyph_name].draw(pen, font["glyf"])
    else:
        glyph_set[glyph_name].draw(pen)
    return {
        "schema": "kicad_monkey.outline_vector.a0",
        "type": "kicad_monkey.outline_vector",
        "version": "a0",
        "case_id": case["case_id"],
        "coordinate_format": "font_design_units_f64",
        "coordinate_comparison": case["coordinate_comparison"],
        "font_id": case["font_id"],
        "font_sha256": hashlib.sha256(font_bytes).hexdigest(),
        "face_index": face_index,
        "variations": case["variations"],
        "glyph_id": glyph_id,
        "units_per_em": cast(Any, font["head"]).unitsPerEm,
        "commands": pen.commands,
    }


def _variable_font_fixture() -> bytes:
    builder = FontBuilder(1000, isTTF=True)
    glyph_order = [".notdef", "A"]
    builder.setupGlyphOrder(glyph_order)
    notdef = TTGlyphPen(None).glyph()
    pen = TTGlyphPen(None)
    pen.moveTo((100, 0))
    pen.qCurveTo((500, 800), (900, 0))
    pen.lineTo((700, 0))
    pen.qCurveTo((500, 500), (300, 0))
    pen.closePath()
    glyph = pen.glyph()
    builder.setupGlyf({".notdef": notdef, "A": glyph})
    builder.setupHorizontalMetrics({".notdef": (500, 0), "A": (1000, 0)})
    builder.setupHorizontalHeader(ascent=900, descent=-200)
    builder.setupCharacterMap({0x0041: "A"})
    _setup_names(builder, "KiCad Monkey Outline Variable Fixture")
    builder.setupOS2(
        sTypoAscender=900,
        sTypoDescender=-200,
        usWinAscent=900,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupMaxp()
    builder.setupFvar([("wght", 100, 400, 900, "Weight")], [])
    point_count = len(glyph.coordinates)
    deltas = [(0, 0)] * (point_count + 4)
    deltas[1] = (0, 200)
    builder.setupGvar(
        {"A": [TupleVariation({"wght": (0.0, 1.0, 1.0)}, deltas)]}
    )
    return _save_deterministic(builder)


def _cff_font_fixture() -> bytes:
    builder = FontBuilder(1000, isTTF=False)
    glyph_order = [".notdef", "C"]
    builder.setupGlyphOrder(glyph_order)
    char_strings: dict[str, Any] = {}
    for name in glyph_order:
        pen = T2CharStringPen(500, None)
        if name == "C":
            pen.moveTo((850, 700))
            pen.curveTo((700, 850), (300, 850), (150, 500))
            pen.curveTo((100, 250), (300, 100), (700, 150))
            pen.lineTo((650, 300))
            pen.curveTo((400, 250), (300, 350), (350, 500))
            pen.curveTo((400, 650), (600, 650), (750, 550))
            pen.closePath()
        char_strings[name] = pen.getCharString()
    builder.setupCharacterMap({0x0043: "C"})
    builder.setupCFF(
        "KiCadMonkeyOutlineCFFFixture-Regular",
        {"FullName": "KiCad Monkey Outline CFF Fixture Regular"},
        char_strings,
        {},
    )
    builder.setupHorizontalMetrics({name: (500, 0) for name in glyph_order})
    builder.setupHorizontalHeader(ascent=900, descent=-200)
    _setup_names(builder, "KiCad Monkey Outline CFF Fixture")
    builder.setupOS2(
        sTypoAscender=900,
        sTypoDescender=-200,
        usWinAscent=900,
        usWinDescent=200,
    )
    builder.setupPost()
    return _save_deterministic(builder)


def _cff2_font_fixture() -> bytes:
    builder = FontBuilder(1000, isTTF=False)
    glyph_order = [".notdef", "S"]
    builder.setupGlyphOrder(glyph_order)
    char_strings: dict[str, Any] = {}
    for name in glyph_order:
        pen = T2CharStringPen(None, None, CFF2=True)
        if name == "S":
            pen.moveTo((750, 750))
            pen.curveTo((550, 900), (200, 850), (180, 600))
            pen.curveTo((160, 420), (650, 500), (620, 250))
            pen.curveTo((600, 80), (260, 70), (120, 220))
            pen.closePath()
        char_string = pen.getCharString(private=None, globalSubrs=None)
        if name == "S":
            program = cast(list[Any], char_string.program)
            if program[:2] != [750, 750]:
                raise ValueError("unexpected generated CFF2 fixture program")
            # One region: vary the first rmoveto operand by +100 at wght=900.
            char_string.program = [750, 100, 1, "blend", *program[1:]]
        char_strings[name] = char_string
    builder.setupCharacterMap({0x0053: "S"})
    _setup_names(builder, "KiCad Monkey Outline CFF2 Fixture")
    builder.setupFvar([("wght", 100, 400, 900, "Weight")], [])
    builder.setupCFF2(char_strings, regions=[{"wght": (0.0, 1.0, 1.0)}])
    builder.setupHorizontalMetrics({name: (600, 0) for name in glyph_order})
    builder.setupHorizontalHeader(ascent=900, descent=-200)
    builder.setupOS2(
        sTypoAscender=900,
        sTypoDescender=-200,
        usWinAscent=900,
        usWinDescent=200,
    )
    builder.setupPost()
    return _save_deterministic(builder)


def _composite_font_fixture() -> bytes:
    builder = FontBuilder(1000, isTTF=True)
    glyph_order = [".notdef", "base", "X"]
    builder.setupGlyphOrder(glyph_order)
    base_pen = TTGlyphPen(None)
    base_pen.moveTo((50, 50))
    base_pen.qCurveTo((300, 700), (550, 50))
    base_pen.lineTo((400, 50))
    base_pen.qCurveTo((300, 400), (200, 50))
    base_pen.closePath()
    base_glyph = base_pen.glyph()
    component_pen = TTGlyphPen({"base": base_glyph})
    # Use a symmetric shear with no component offset so both engines exercise
    # the affine transform without entering legacy scaled-offset ambiguity.
    component_pen.addComponent("base", (0.75, 0.125, 0.125, 1.125, 0, 0))
    builder.setupGlyf(
        {
            ".notdef": TTGlyphPen(None).glyph(),
            "base": base_glyph,
            "X": component_pen.glyph(),
        }
    )
    builder.setupHorizontalMetrics({".notdef": (800, 0), "base": (800, 50), "X": (800, 44)})
    builder.setupHorizontalHeader(ascent=900, descent=-200)
    builder.setupCharacterMap({0x0058: "X"})
    _setup_names(builder, "KiCad Monkey Outline Composite Fixture")
    builder.setupOS2(
        sTypoAscender=900,
        sTypoDescender=-200,
        usWinAscent=900,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupMaxp()
    return _save_deterministic(builder)


def _collection_font_fixture() -> bytes:
    collection = TTCollection()
    collection.fonts = [
        TTFont(BytesIO(_collection_face_fixture("First", "Z", 0x005A, 40))),
        TTFont(BytesIO(_collection_face_fixture("Second", "C", 0x0043, 160))),
    ]
    for font in collection.fonts:
        font.recalcTimestamp = False
    output = BytesIO()
    collection.save(output)
    return output.getvalue()


def _collection_face_fixture(
    suffix: str, glyph_name: str, codepoint: int, inset: int
) -> bytes:
    builder = FontBuilder(1000, isTTF=True)
    glyph_order = [".notdef", glyph_name]
    builder.setupGlyphOrder(glyph_order)
    pen = TTGlyphPen(None)
    pen.moveTo((inset, 100))
    pen.lineTo((900 - inset, 100))
    pen.lineTo((800 - inset, 700))
    pen.lineTo((200 + inset, 700))
    pen.closePath()
    builder.setupGlyf({".notdef": TTGlyphPen(None).glyph(), glyph_name: pen.glyph()})
    builder.setupHorizontalMetrics({name: (1000, 0) for name in glyph_order})
    builder.setupHorizontalHeader(ascent=900, descent=-200)
    builder.setupCharacterMap({codepoint: glyph_name})
    _setup_names(builder, f"KiCad Monkey Outline Collection {suffix}")
    builder.setupOS2(
        sTypoAscender=900,
        sTypoDescender=-200,
        usWinAscent=900,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupMaxp()
    return _save_deterministic(builder)


def _setup_names(builder: FontBuilder, family: str) -> None:
    compact = family.replace(" ", "")
    builder.setupNameTable(
        {
            "familyName": family,
            "styleName": "Regular",
            "uniqueFontIdentifier": f"{compact}-Regular",
            "fullName": f"{family} Regular",
            "psName": f"{compact}-Regular",
            "version": "Version 1.000",
        }
    )


def _save_deterministic(builder: FontBuilder) -> bytes:
    builder.font.recalcTimestamp = False
    head = cast(Any, builder.font["head"])
    # OpenType seconds since 1904-01-01 for the Unix epoch; deterministic and
    # old enough to avoid FontTools' suspicious-low-timestamp warning.
    head.created = 2_082_844_800
    head.modified = 2_082_844_800
    output = BytesIO()
    builder.save(output)
    return output.getvalue()


def _point_command(kind: str, point: tuple[float, float]) -> dict[str, Any]:
    return {"kind": kind, "x": _coordinate(point[0]), "y": _coordinate(point[1])}


def _coordinate(value: float) -> int | float:
    numeric = float(value)
    return int(numeric) if numeric.is_integer() else numeric


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    payload = generate_vectors()
    encoded = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    font_payloads = {
        VARIABLE_FONT_PATH: _variable_font_fixture(),
        CFF_FONT_PATH: _cff_font_fixture(),
        CFF2_FONT_PATH: _cff2_font_fixture(),
        COMPOSITE_FONT_PATH: _composite_font_fixture(),
        COLLECTION_FONT_PATH: _collection_font_fixture(),
    }
    if args.check:
        if OUTPUT_PATH.read_bytes() != encoded:
            raise SystemExit(f"stale outline vectors: {OUTPUT_PATH}")
        for path, font_bytes in font_payloads.items():
            if path.read_bytes() != font_bytes:
                raise SystemExit(f"stale outline fixture: {path}")
        return
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    VARIABLE_FONT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_bytes(encoded)
    for path, font_bytes in font_payloads.items():
        path.write_bytes(font_bytes)


if __name__ == "__main__":
    main()

"""Generate installable webfonts from KiCad's bundled Newstroke stroke font.

Builds the "KiCad Stroke" family (Light/Regular/Bold, each upright and
italic) as TTF/OTF/WOFF/WOFF2 plus a ready-to-use CSS file. Glyph outlines
are produced by buffering the vendored Newstroke Hershey polylines
(``kicad_monkey.kicad_stroke_font``) with round caps and joins, mirroring how
KiCad strokes text on screen.

Weight pen widths follow KiCad's own automatic pen rules relative to the
text size (regular = size/8, bold = size/5, with size/12 as the light
companion), and the italic variants apply KiCad's 1/8 shear.

Usage:

    uv run python tools/generate_kicad_stroke_webfont.py --output-dir assets/fonts \
        --formats ttf,otf,woff,woff2 --light --bold --italic
"""

from __future__ import annotations

import argparse
import math
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence

PROJECT_ROOT = Path(__file__).resolve().parents[1]
PY_SRC = PROJECT_ROOT / "src" / "py"
if str(PY_SRC) not in sys.path:
    sys.path.insert(0, str(PY_SRC))

from kicad_monkey.kicad_stroke_font import (  # noqa: E402
    ITALIC_TILT,
    StrokeGlyph,
    _load_glyph_data,
    parse_hershey_glyph,
)
from fontTools.fontBuilder import FontBuilder  # noqa: E402
from fontTools.pens.t2CharStringPen import T2CharStringPen  # noqa: E402
from fontTools.pens.ttGlyphPen import TTGlyphPen  # noqa: E402
from fontTools.ttLib import TTFont  # noqa: E402
from shapely import BufferCapStyle, BufferJoinStyle, GeometryCollection  # noqa: E402
from shapely.geometry import LineString, MultiPolygon, Point, Polygon  # noqa: E402
from shapely.geometry.base import BaseGeometry  # noqa: E402
from shapely.ops import unary_union  # noqa: E402

FontFormat = str

FIRST_CODEPOINT = 0x20
FAMILY_NAME = "KiCad Stroke"
FAMILY_SLUG = "kicad-stroke"
FONT_VERSION = "1.0"
FONT_REVISION = 1.0
# OpenType head timestamps count seconds from 1904-01-01. Pinning that epoch
# keeps generated files deterministic without emitting invalid-date warnings.
OPENTYPE_EPOCH = 2_082_844_800
DEFAULT_UNITS_PER_EM = 1000
DEFAULT_QUAD_SEGS = 8
DEFAULT_OUTPUT_DIR = PROJECT_ROOT / "temp" / "kicad_stroke_fonts"
DEFAULT_FORMATS = ("ttf", "otf", "woff")
SUPPORTED_FORMATS = ("ttf", "otf", "woff", "woff2")
CSS_FILE_NAME = "kicad-monkey-stroke-fonts.css"

# Newstroke normalizes the capital-letter span to 1.0 with the baseline
# sitting 1/21 above the capital bottom overshoot (FONT_OFFSET handling in
# kicad_stroke_font). CAP_BOTTOM re-bases glyphs onto a y-up baseline.
CAP_BOTTOM = 1.0 / 21.0
CAP_SPAN = 1.0
# Lowercase x top in the Newstroke tables ('x' reaches -13/21 over a +1/21
# baseline offset), used for the OS/2 x-height field.
X_HEIGHT = 0.667
# Fraction of the em occupied by capital-letter ink (centerline span plus one
# stroke width from the round caps); matches common Latin fonts.
CAP_HEIGHT_EM = 0.72
# KiCad's stroke-font interline pitch is 1.62x the text size.
DEFAULT_LINE_SPACING_FACTOR = 1.62
DEFAULT_ASCENT_FACTOR = 1.22

# KiCad automatic pen widths relative to text size: size/8 regular and
# size/5 bold; size/12 extends the same progression down for light.
WEIGHT_STROKE_RATIOS: dict[str, float] = {
    "light": 1.0 / 12.0,
    "regular": 1.0 / 8.0,
    "bold": 1.0 / 5.0,
}
WEIGHT_LIGHT = 300
WEIGHT_REGULAR = 400
WEIGHT_BOLD = 700
WEIGHT_VALUES = {"light": WEIGHT_LIGHT, "regular": WEIGHT_REGULAR, "bold": WEIGHT_BOLD}
ITALIC_ANGLE_DEGREES = -math.degrees(math.atan(ITALIC_TILT))

# Advance-only whitespace codepoints kept despite having no strokes.
WHITESPACE_CODEPOINTS = (0x20, 0xA0)

EPSILON = 1e-9
OS2_FS_SELECTION_ITALIC = 1 << 0
OS2_FS_SELECTION_BOLD = 1 << 5
OS2_FS_SELECTION_REGULAR = 1 << 6
OS2_FS_SELECTION_USE_TYPO_METRICS = 1 << 7
HEAD_MAC_STYLE_BOLD = 1 << 0
HEAD_MAC_STYLE_ITALIC = 1 << 1
OS2_UNICODE_RANGE_BASIC_LATIN = 1 << 0
OS2_UNICODE_RANGE_LATIN_1_SUPPLEMENT = 1 << 1
OS2_UNICODE_RANGE_LATIN_EXTENDED_A = 1 << 2
OS2_UNICODE_RANGE_LATIN_EXTENDED_B = 1 << 3
OS2_UNICODE_RANGE_GREEK = 1 << 7
OS2_UNICODE_RANGE_CYRILLIC = 1 << 9
# Letterlike Symbols bit 35, Arrows bit 37, Mathematical Operators bit 38,
# Superscripts And Subscripts bit 40 -> ulUnicodeRange2.
OS2_UNICODE_RANGE2_LETTERLIKE = 1 << (35 - 32)
OS2_UNICODE_RANGE2_ARROWS = 1 << (37 - 32)
OS2_UNICODE_RANGE2_MATH_OPERATORS = 1 << (38 - 32)
OS2_UNICODE_RANGE2_SUPER_SUBSCRIPTS = 1 << (40 - 32)
OS2_CODE_PAGE_LATIN_1 = 1 << 0


@dataclass(frozen=True)
class FontBuildPlan:
    weight_name: str
    stroke_ratio: float
    italic: bool
    units_per_em: int = DEFAULT_UNITS_PER_EM
    quad_segs: int = DEFAULT_QUAD_SEGS
    line_spacing_factor: float = DEFAULT_LINE_SPACING_FACTOR
    ascent_factor: float = DEFAULT_ASCENT_FACTOR
    cap_height_em: float = CAP_HEIGHT_EM

    @property
    def em_scale(self) -> float:
        """Glyph-space cap units (Newstroke cap span = 1.0) to em fraction."""

        return self.cap_height_em / (CAP_SPAN + self.stroke_ratio)


@dataclass(frozen=True)
class BuiltFontData:
    family_name: str
    style_name: str
    full_name: str
    postscript_name: str
    file_stem: str
    units_per_em: int
    glyph_order: tuple[str, ...]
    cmap: dict[int, str]
    contours: dict[str, list[list[tuple[int, int]]]]
    advances: dict[str, int]
    lsbs: dict[str, int]
    ascent: int
    descent: int
    line_gap: int
    win_ascent: int
    win_descent: int
    cap_height: int
    x_height: int
    weight: int
    italic: bool


@dataclass(frozen=True)
class GeneratedFontFile:
    path: Path
    weight_name: str
    stroke_ratio: float
    format: str
    family_name: str
    full_name: str
    weight: int
    italic: bool


@dataclass(frozen=True)
class GenerationResult:
    font_files: tuple[GeneratedFontFile, ...]
    css_path: Path | None


_GLYPH_MAP_CACHE: dict[int, StrokeGlyph] | None = None


def newstroke_glyph_map() -> dict[int, StrokeGlyph]:
    """Codepoint to parsed glyph for every drawable Newstroke entry."""

    global _GLYPH_MAP_CACHE
    if _GLYPH_MAP_CACHE is None:
        glyph_map: dict[int, StrokeGlyph] = {}
        for index, encoded in enumerate(_load_glyph_data()):
            code = FIRST_CODEPOINT + index
            glyph = parse_hershey_glyph(encoded)
            if glyph.strokes or code in WHITESPACE_CODEPOINTS:
                glyph_map[code] = glyph
        _GLYPH_MAP_CACHE = glyph_map
    return _GLYPH_MAP_CACHE


def default_codepoints() -> tuple[int, ...]:
    return tuple(sorted(newstroke_glyph_map()))


def _font_space_strokes(
    glyph: StrokeGlyph,
    *,
    italic: bool,
) -> list[list[tuple[float, float]]]:
    """Re-base a y-down Newstroke glyph onto a y-up baseline, with shear."""

    strokes: list[list[tuple[float, float]]] = []
    for stroke in glyph.strokes:
        points: list[tuple[float, float]] = []
        for x, y in stroke:
            font_y = CAP_BOTTOM - y
            font_x = x + font_y * ITALIC_TILT if italic else x
            points.append((font_x, font_y))
        strokes.append(points)
    return strokes


def _buffer_stroke(
    points: Sequence[tuple[float, float]],
    radius: float,
    quad_segs: int,
) -> BaseGeometry | None:
    if not points:
        return None
    x0, y0 = points[0]
    is_dot = all(abs(x - x0) <= EPSILON and abs(y - y0) <= EPSILON for x, y in points)
    if is_dot:
        return Point(x0, y0).buffer(radius, quad_segs=quad_segs)
    return LineString(points).buffer(
        radius,
        quad_segs=quad_segs,
        cap_style=BufferCapStyle.round,
        join_style=BufferJoinStyle.round,
    )


def _stroke_geometry(
    strokes: Sequence[Sequence[tuple[float, float]]],
    radius: float,
    quad_segs: int,
) -> BaseGeometry:
    pieces = [
        piece
        for stroke in strokes
        if (piece := _buffer_stroke(stroke, radius, quad_segs)) is not None
    ]
    if not pieces:
        return GeometryCollection()
    return unary_union(pieces)


def _polygons_from_geometry(geometry: BaseGeometry) -> list[Polygon]:
    if geometry.is_empty:
        return []
    if isinstance(geometry, Polygon):
        return [geometry]
    if isinstance(geometry, MultiPolygon):
        return _sort_polygons(tuple(geometry.geoms))
    if isinstance(geometry, GeometryCollection):
        polygons: list[Polygon] = []
        for child in geometry.geoms:
            polygons.extend(_polygons_from_geometry(child))
        return _sort_polygons(polygons)
    return []


def _sort_polygons(polygons: Sequence[Polygon]) -> list[Polygon]:
    return sorted(
        polygons,
        key=lambda polygon: tuple(round(value, 9) for value in polygon.bounds),
    )


def _contour_units(
    coords: Sequence[tuple[float, float]],
    glyph_units: float,
) -> list[tuple[int, int]]:
    rounded = [
        (int(round(x * glyph_units)), int(round(y * glyph_units))) for x, y in coords
    ]
    points: list[tuple[int, int]] = []
    for point in rounded:
        if not points or points[-1] != point:
            points.append(point)
    if len(points) > 1 and points[0] == points[-1]:
        points.pop()
    if len(set(points)) < 3:
        return []
    return points


def _signed_area(contour: Sequence[tuple[int, int]]) -> float:
    area = 0.0
    for index, (x1, y1) in enumerate(contour):
        x2, y2 = contour[(index + 1) % len(contour)]
        area += x1 * y2 - x2 * y1
    return area / 2.0


def _orient_contour(
    contour: Sequence[tuple[int, int]],
    *,
    clockwise: bool,
) -> list[tuple[int, int]]:
    is_clockwise = _signed_area(contour) < 0.0
    if is_clockwise == clockwise:
        return list(contour)
    return list(reversed(contour))


def _polygon_contours(
    polygon: Polygon,
    glyph_units: float,
) -> list[list[tuple[int, int]]]:
    contours: list[list[tuple[int, int]]] = []
    exterior = _contour_units(polygon.exterior.coords, glyph_units)
    if exterior:
        contours.append(_orient_contour(exterior, clockwise=True))
    for interior in polygon.interiors:
        hole = _contour_units(interior.coords, glyph_units)
        if hole:
            contours.append(_orient_contour(hole, clockwise=False))
    return contours


def _glyph_contours(
    glyph: StrokeGlyph,
    plan: FontBuildPlan,
    glyph_units: float,
) -> list[list[tuple[int, int]]]:
    strokes = _font_space_strokes(glyph, italic=plan.italic)
    geometry = _stroke_geometry(strokes, plan.stroke_ratio / 2.0, plan.quad_segs)
    contours: list[list[tuple[int, int]]] = []
    for polygon in _polygons_from_geometry(geometry):
        contours.extend(_polygon_contours(polygon, glyph_units))
    return contours


def _font_identity(plan: FontBuildPlan) -> tuple[str, str, str, str, int]:
    weight_word = plan.weight_name.capitalize()
    if plan.weight_name == "regular":
        subfamily = "Italic" if plan.italic else "Regular"
        slug = f"{FAMILY_SLUG}-italic" if plan.italic else FAMILY_SLUG
    else:
        subfamily = f"{weight_word} Italic" if plan.italic else weight_word
        slug = f"{FAMILY_SLUG}-{plan.weight_name}"
        if plan.italic:
            slug = f"{slug}-italic"
    full_name = FAMILY_NAME if subfamily == "Regular" else f"{FAMILY_NAME} {subfamily}"
    weight = WEIGHT_VALUES[plan.weight_name]
    return FAMILY_NAME, subfamily, full_name, slug, weight


def _postscript_name(full_name: str) -> str:
    return full_name.replace(" ", "-")


def _glyph_name(code: int) -> str:
    return f"uni{code:04X}"


def _ink_extremes(
    contours: Mapping[str, Sequence[Sequence[tuple[int, int]]]],
) -> tuple[int, int]:
    ys = [
        y
        for glyph_contours in contours.values()
        for contour in glyph_contours
        for _, y in contour
    ]
    if not ys:
        return 0, 0
    return max(ys), min(ys)


def build_font_data(
    plan: FontBuildPlan,
    codepoints: Sequence[int] | None = None,
) -> BuiltFontData:
    glyph_map = newstroke_glyph_map()
    codes = (
        tuple(sorted(codepoints)) if codepoints is not None else default_codepoints()
    )
    missing = [code for code in codes if code not in glyph_map]
    if missing:
        raise ValueError(f"codepoint U+{missing[0]:04X} has no Newstroke glyph")

    family_name, style_name, full_name, file_stem, weight = _font_identity(plan)
    glyph_units = plan.em_scale * plan.units_per_em
    cmap = {code: _glyph_name(code) for code in codes}
    glyph_order = (".notdef", *tuple(cmap[code] for code in codes))
    contours: dict[str, list[list[tuple[int, int]]]] = {".notdef": []}
    advances = {".notdef": int(round(glyph_units))}
    lsbs = {".notdef": 0}
    for code in codes:
        glyph = glyph_map[code]
        glyph_name = cmap[code]
        glyph_contours = _glyph_contours(glyph, plan, glyph_units)
        contours[glyph_name] = glyph_contours
        advances[glyph_name] = max(0, int(round(glyph.width * glyph_units)))
        lsbs[glyph_name] = min(
            (x for contour in glyph_contours for x, _ in contour), default=0
        )

    line_total = int(round(plan.line_spacing_factor * glyph_units))
    ascent = int(round(plan.ascent_factor * glyph_units))
    descent = line_total - ascent
    ink_top, ink_bottom = _ink_extremes(contours)
    return BuiltFontData(
        family_name=family_name,
        style_name=style_name,
        full_name=full_name,
        postscript_name=_postscript_name(full_name),
        file_stem=file_stem,
        units_per_em=plan.units_per_em,
        glyph_order=glyph_order,
        cmap=cmap,
        contours=contours,
        advances=advances,
        lsbs=lsbs,
        ascent=ascent,
        descent=descent,
        line_gap=0,
        win_ascent=max(ascent, ink_top),
        win_descent=max(descent, -ink_bottom),
        cap_height=int(round(plan.cap_height_em * plan.units_per_em)),
        x_height=int(round((X_HEIGHT + plan.stroke_ratio) * glyph_units)),
        weight=weight,
        italic=plan.italic,
    )


def _draw_contours(
    pen: TTGlyphPen | T2CharStringPen,
    contours: Sequence[Sequence[tuple[int, int]]],
) -> None:
    for contour in contours:
        pen.moveTo(contour[0])
        for point in contour[1:]:
            pen.lineTo(point)
        pen.closePath()


def _setup_common_font(fb: FontBuilder, font_data: BuiltFontData) -> None:
    fb.font.recalcTimestamp = False
    fb.updateHead(
        created=OPENTYPE_EPOCH,
        modified=OPENTYPE_EPOCH,
        fontRevision=FONT_REVISION,
    )
    fb.setupGlyphOrder(list(font_data.glyph_order))
    fb.setupCharacterMap(font_data.cmap)


def _finish_font(
    fb: FontBuilder,
    font_data: BuiltFontData,
    metrics: Mapping[str, tuple[int, int]],
) -> None:
    fb.setupHorizontalMetrics(dict(metrics))
    fb.setupHorizontalHeader(
        ascent=font_data.ascent,
        descent=-font_data.descent,
        lineGap=font_data.line_gap,
    )
    fb.setupNameTable(
        {
            "familyName": font_data.family_name,
            "styleName": font_data.style_name,
            "uniqueFontIdentifier": f"{font_data.postscript_name}:{FONT_VERSION}",
            "fullName": font_data.full_name,
            "psName": font_data.postscript_name,
            "typographicFamily": font_data.family_name,
            "typographicSubfamily": font_data.style_name,
            "wwsFamilyName": font_data.family_name,
            "wwsSubfamilyName": font_data.style_name,
            "version": f"Version {FONT_VERSION}",
            "manufacturer": "Wavenumber",
            "designer": "Wavenumber",
            "description": (
                "Generated from the KiCad Newstroke stroke font "
                "(Newstroke by Vladimir Uryvaev, CC0-1.0)."
            ),
        }
    )
    is_bold = font_data.weight >= WEIGHT_BOLD
    fs_selection = OS2_FS_SELECTION_USE_TYPO_METRICS
    if font_data.italic:
        fs_selection |= OS2_FS_SELECTION_ITALIC
    elif not is_bold:
        fs_selection |= OS2_FS_SELECTION_REGULAR
    if is_bold:
        fs_selection |= OS2_FS_SELECTION_BOLD
    fb.setupOS2(
        version=4,
        usWeightClass=font_data.weight,
        usWidthClass=5,
        sTypoAscender=font_data.ascent,
        sTypoDescender=-font_data.descent,
        sTypoLineGap=font_data.line_gap,
        usWinAscent=font_data.win_ascent,
        usWinDescent=font_data.win_descent,
        sCapHeight=font_data.cap_height,
        sxHeight=font_data.x_height,
        achVendID="WNUM",
        fsSelection=fs_selection,
        fsType=0,
        ulUnicodeRange1=(
            OS2_UNICODE_RANGE_BASIC_LATIN
            | OS2_UNICODE_RANGE_LATIN_1_SUPPLEMENT
            | OS2_UNICODE_RANGE_LATIN_EXTENDED_A
            | OS2_UNICODE_RANGE_LATIN_EXTENDED_B
            | OS2_UNICODE_RANGE_GREEK
            | OS2_UNICODE_RANGE_CYRILLIC
        ),
        ulUnicodeRange2=(
            OS2_UNICODE_RANGE2_LETTERLIKE
            | OS2_UNICODE_RANGE2_ARROWS
            | OS2_UNICODE_RANGE2_MATH_OPERATORS
            | OS2_UNICODE_RANGE2_SUPER_SUBSCRIPTS
        ),
        ulUnicodeRange3=0,
        ulUnicodeRange4=0,
        ulCodePageRange1=OS2_CODE_PAGE_LATIN_1,
        ulCodePageRange2=0,
        usFirstCharIndex=min(font_data.cmap),
        usLastCharIndex=min(max(font_data.cmap), 0xFFFF),
    )
    mac_style = HEAD_MAC_STYLE_BOLD if is_bold else 0
    if font_data.italic:
        mac_style |= HEAD_MAC_STYLE_ITALIC
    fb.font["head"].macStyle = mac_style
    italic_angle = ITALIC_ANGLE_DEGREES if font_data.italic else 0.0
    fb.setupPost(italicAngle=italic_angle)


def _save_ttf(font_data: BuiltFontData, output_path: Path) -> None:
    glyphs = {}
    for glyph_name in font_data.glyph_order:
        pen = TTGlyphPen(None)
        _draw_contours(pen, font_data.contours[glyph_name])
        glyphs[glyph_name] = pen.glyph()

    fb = FontBuilder(font_data.units_per_em, isTTF=True)
    _setup_common_font(fb, font_data)
    fb.setupGlyf(glyphs)
    metrics = {
        glyph_name: (font_data.advances[glyph_name], font_data.lsbs[glyph_name])
        for glyph_name in font_data.glyph_order
    }
    _finish_font(fb, font_data, metrics)
    fb.save(output_path)


def _save_otf(font_data: BuiltFontData, output_path: Path) -> None:
    char_strings = {}
    for glyph_name in font_data.glyph_order:
        pen = T2CharStringPen(font_data.advances[glyph_name], None)
        _draw_contours(pen, font_data.contours[glyph_name])
        char_strings[glyph_name] = pen.getCharString()

    fb = FontBuilder(font_data.units_per_em, isTTF=False)
    _setup_common_font(fb, font_data)
    fb.setupCFF(
        font_data.postscript_name,
        {"FullName": font_data.full_name, "FamilyName": font_data.family_name},
        char_strings,
        {},
    )
    metrics = {
        glyph_name: (font_data.advances[glyph_name], font_data.lsbs[glyph_name])
        for glyph_name in font_data.glyph_order
    }
    _finish_font(fb, font_data, metrics)
    fb.save(output_path)


def _save_webfont(source_ttf_path: Path, output_path: Path, flavor: str) -> None:
    font = TTFont(source_ttf_path)
    font.recalcTimestamp = False
    font.flavor = flavor
    try:
        font.save(output_path)
    except ImportError as exc:
        if flavor == "woff2":
            raise RuntimeError(
                "WOFF2 generation requires the optional brotli package"
            ) from exc
        raise


def _font_file(
    path: Path,
    plan: FontBuildPlan,
    output_format: str,
    font_data: BuiltFontData,
) -> GeneratedFontFile:
    return GeneratedFontFile(
        path=path,
        weight_name=plan.weight_name,
        stroke_ratio=plan.stroke_ratio,
        format=output_format,
        family_name=font_data.family_name,
        full_name=font_data.full_name,
        weight=font_data.weight,
        italic=font_data.italic,
    )


def _generate_plan_fonts(
    plan: FontBuildPlan,
    formats: Sequence[FontFormat],
    output_dir: Path,
    codepoints: Sequence[int] | None,
) -> tuple[GeneratedFontFile, ...]:
    font_data = build_font_data(plan, codepoints)
    generated: list[GeneratedFontFile] = []
    ttf_path = output_dir / f"{font_data.file_stem}.ttf"
    needs_ttf = "ttf" in formats or "woff" in formats or "woff2" in formats

    if needs_ttf:
        _save_ttf(font_data, ttf_path)
    if "ttf" in formats:
        generated.append(_font_file(ttf_path, plan, "ttf", font_data))
    if "otf" in formats:
        otf_path = output_dir / f"{font_data.file_stem}.otf"
        _save_otf(font_data, otf_path)
        generated.append(_font_file(otf_path, plan, "otf", font_data))
    for font_format in ("woff", "woff2"):
        if font_format not in formats:
            continue
        webfont_path = output_dir / f"{font_data.file_stem}.{font_format}"
        _save_webfont(ttf_path, webfont_path, font_format)
        generated.append(_font_file(webfont_path, plan, font_format, font_data))
    if "ttf" not in formats and ttf_path.exists():
        ttf_path.unlink()
    return tuple(generated)


def _build_plans(
    weights: Sequence[str],
    *,
    italic: bool,
    units_per_em: int,
    quad_segs: int,
) -> tuple[FontBuildPlan, ...]:
    unknown = [name for name in weights if name not in WEIGHT_STROKE_RATIOS]
    if unknown:
        valid = ", ".join(WEIGHT_STROKE_RATIOS)
        raise ValueError(f"unknown weight {unknown[0]!r}; expected {valid}")
    plans: list[FontBuildPlan] = []
    for weight_name in weights:
        for is_italic in (False, True) if italic else (False,):
            plans.append(
                FontBuildPlan(
                    weight_name=weight_name,
                    stroke_ratio=WEIGHT_STROKE_RATIOS[weight_name],
                    italic=is_italic,
                    units_per_em=units_per_em,
                    quad_segs=quad_segs,
                )
            )
    return tuple(plans)


def generate_fonts(
    *,
    output_dir: Path,
    weights: Sequence[str] = ("regular",),
    italic: bool = False,
    formats: Sequence[FontFormat] = DEFAULT_FORMATS,
    units_per_em: int = DEFAULT_UNITS_PER_EM,
    quad_segs: int = DEFAULT_QUAD_SEGS,
    codepoints: Sequence[int] | None = None,
    write_css: bool = True,
) -> GenerationResult:
    if units_per_em <= 0:
        raise ValueError("units_per_em must be positive")
    if quad_segs <= 0:
        raise ValueError("quad_segs must be positive")
    invalid = [name for name in formats if name not in SUPPORTED_FORMATS]
    if invalid:
        valid = ", ".join(SUPPORTED_FORMATS)
        raise ValueError(f"unknown output format {invalid[0]!r}; expected {valid}")

    output_dir.mkdir(parents=True, exist_ok=True)
    generated: list[GeneratedFontFile] = []
    for plan in _build_plans(
        weights, italic=italic, units_per_em=units_per_em, quad_segs=quad_segs
    ):
        started = time.monotonic()
        generated.extend(_generate_plan_fonts(plan, formats, output_dir, codepoints))
        _, style_name, _, _, _ = _font_identity(plan)
        print(f"built {FAMILY_NAME} {style_name} in {time.monotonic() - started:.1f}s")

    css_path: Path | None = None
    if write_css:
        css_path = output_dir / CSS_FILE_NAME
        css_path.write_text(_build_css(generated, output_dir), encoding="utf-8")
    return GenerationResult(font_files=tuple(generated), css_path=css_path)


def _css_sources(
    font_files: Sequence[GeneratedFontFile],
    output_dir: Path,
) -> list[str]:
    format_labels = {
        "woff2": "woff2",
        "woff": "woff",
        "ttf": "truetype",
        "otf": "opentype",
    }
    by_format = {font_file.format: font_file for font_file in font_files}
    sources: list[str] = []
    for font_format in ("woff2", "woff", "ttf", "otf"):
        font_file = by_format.get(font_format)
        if font_file is None:
            continue
        rel_path = font_file.path.relative_to(output_dir).as_posix()
        sources.append(f"url('{rel_path}') format('{format_labels[font_format]}')")
    return sources


def _build_css(font_files: Sequence[GeneratedFontFile], output_dir: Path) -> str:
    by_face: dict[tuple[str, int, bool], list[GeneratedFontFile]] = {}
    for font_file in font_files:
        key = (font_file.family_name, font_file.weight, font_file.italic)
        by_face.setdefault(key, []).append(font_file)

    blocks: list[str] = []
    for family_name, weight, italic in sorted(by_face):
        sources = _css_sources(by_face[(family_name, weight, italic)], output_dir)
        if not sources:
            continue
        font_style = "italic" if italic else "normal"
        blocks.append(
            "@font-face {\n"
            f"  font-family: '{family_name}';\n"
            f"  src: {', '.join(sources)};\n"
            f"  font-weight: {weight};\n"
            f"  font-style: {font_style};\n"
            "  font-display: swap;\n"
            "}"
        )
    return "\n\n".join(blocks) + ("\n" if blocks else "")


# --- Demo page -------------------------------------------------------------

# The monkey marks are always written lowercase.
DEMO_TITLE = "kicad monkey"
DEMO_TAGLINE = "kicad newstroke \u00b7 modern webfont"
DEMO_FILE_NAME = "demo.html"
# (label, formula) pairs typeset entirely with glyphs the font provides.
# A four-space run separates related equations; the demo stacks each on its
# own line so multi-part rows read top to bottom.
DEMO_REFERENCE_EQUATIONS: tuple[tuple[str, str], ...] = (
    ("Ohm's law", "V = I\u00d7R"),
    ("Power", "P = V\u00d7I = I\u00b2R = V\u00b2/R"),
    ("Capacitance", "Q = C\u00d7V    i = C \u2202v/\u2202t"),
    ("Inductance", "v = L \u2202i/\u2202t    E = LI\u00b2/2"),
    ("Reactance", "XC = 1/(2\u03c0fC)    XL = 2\u03c0fL"),
    ("Resonance", "f\u2080 = 1/(2\u03c0\u221a(LC))"),
    ("Time constant", "\u03c4 = RC    \u03c4 = L/R"),
    ("Angular frequency", "\u03c9 = 2\u03c0f"),
    ("Wavelength", "\u03bb = c/f    k = 2\u03c0/\u03bb"),
    ("Impedance", "Z\u00b2 = R\u00b2 + (XL - XC)\u00b2"),
    ("Thermal rise", "\u0394T = P \u00d7 Rth"),
)
DEMO_FIELD_EQUATIONS: tuple[tuple[str, str], ...] = (
    ("Acoustic wave", "\u2207\u00b2p = (1/c\u00b2) \u2202\u00b2p/\u2202t\u00b2"),
    (
        "Maxwell's equations",
        "\u2207\u00b7D = \u03c1\tGauss's Law    "
        "\u2207\u00b7B = 0\tGauss's Law for Magnetism    "
        "\u2207\u00d7E = -\u2202B/\u2202t\tFaraday's Law    "
        "\u2207\u00d7H = J + \u2202D/\u2202t\tAmp\u00e8re-Maxwell Law",
    ),
    (
        "Telegrapher",
        "\u2202V/\u2202x = -L \u2202I/\u2202t - R\u00b7I    "
        "\u2202I/\u2202x = -C \u2202V/\u2202t - G\u00b7V",
    ),
    ("Diffusion", "\u2202T/\u2202t = k\u2207\u00b2T"),
    (
        "Navier-Stokes",
        "\u03c1(\u2202v/\u2202t + v\u00b7\u2207v) = -\u2207p + \u00b5\u2207\u00b2v",
    ),
    (
        "Einstein's field equations",
        "G\u03bc\u03bd + \u039bg\u03bc\u03bd = (8\u03c0G/c\u2074)T\u03bc\u03bd",
    ),
)
# (ref, part number, description) rows with realistic value formatting.
DEMO_BOM_ROWS: tuple[tuple[str, str, str], ...] = (
    ("R1", "RC0603FR-0710KL", "RES 10.0k\u03a9 \u00b11% 100mW 0603"),
    ("R2", "ERJ-3EKF4992V", "RES 49.9k\u03a9 \u00b11% 100mW 0603"),
    ("R3", "WSL2512R0100FEA", "RES SHUNT 10m\u03a9 \u00b11% 1W 2512"),
    ("C1", "GRM188R71C104KA01D", "CAP CER 0.1\u00b5F 16V X7R 0603"),
    ("C2", "C0805C475K8PACTU", "CAP CER 4.7\u00b5F 10V X5R 0805"),
    ("C3", "UWT1V101MCL1GB", "CAP ALUM 100\u00b5F 35V \u00b120% SMD"),
    ("L1", "LQW18AN10NJ00D", "IND 10nH \u00b15% Q=27 @ 250MHz 0603"),
    ("L2", "SRP6540-4R7M", "IND PWR 4.7\u00b5H \u00b120% 8.5A SHIELDED"),
    ("FB1", "BLM18AG601SN1D", "FERRITE 600\u03a9 @ 100MHz 500mA 0603"),
    ("Y1", "ABM8-8.000MHZ-B2-T", "XTAL 8.000MHz \u00b120ppm 18pF SMD"),
    ("D1", "SS34-E3/57T", "DIODE SCHOTTKY 40V 3A DO-214AB"),
    ("U1", "LMR33630ADDAR", "BUCK REG 3.6-36V 3A 400kHz SO-8"),
)
# Fictitious fabrication notes in the all-caps fab-drawing idiom. Newstroke
# carries real comparison glyphs, so tolerances can use them directly.
DEMO_FAB_NOTES: tuple[str, ...] = (
    "OVERALL PCB DIMENSIONS: 47.0mm \u00d7 12.0mm. BOARD OUTLINE DEFINED ON "
    "LAYER Edge.Cuts. ALL ROUTED DIMENSIONS \u00b10.2mm UNLESS NOTED AS "
    "CRITICAL.",
    "BOARD THICKNESS: 1.6mm \u00b110%, 4-LAYER STACKUP, SINGLE LAMINATION.",
    "LAMINATE: FR-4 PER IPC-4101/126, TG \u2265 170\u00b0C, TD \u2265 340\u00b0C.",
    "COPPER WEIGHT: 1 OZ/FT\u00b2 (35\u00b5m) OUTER LAYERS, 0.5 OZ/FT\u00b2 "
    "(17\u00b5m) INNER LAYERS.",
    "SURFACE FINISH: ENIG PER IPC-4552, 2-4 \u00b5IN (0.05-0.10 \u00b5m) AU "
    "OVER 120-240 \u00b5IN (3-6 \u00b5m) NI.",
    "MINIMUM DRILL SIZE: 0.25mm (10 MIL). MINIMUM TRACE/SPACE: 0.127mm (5 MIL).",
    "CONTROLLED IMPEDANCE: 50\u03a9 \u00b110% SINGLE-ENDED, 90\u03a9 \u00b110% "
    "DIFFERENTIAL, REFERENCED TO L2.",
    "SOLDER MASK: BLACK LPI BOTH SIDES PER IPC-SM-840 CLASS T.",
    "SILKSCREEN: WHITE LPI INK, COMPONENT SIDE ONLY.",
    "NC DRILL FILE PROVIDED WITH .TXT EXTENSION.",
    "MOUSEBITES PERMITTED ON SHORT EDGES ONLY. LONG EDGES SHALL BE FREE OF "
    "PROTRUSIONS. FAB MAY USE ROUTING OR V-SCORE.",
    "PCB SHALL BE 100% ELECTRICALLY TESTED PER IPC-9252.",
)
# One cell per sample so the weight rows line up column-for-column.
DEMO_SPECIMEN_SEGMENTS = (
    "R7 4.7k\u03a9 \u00b11%",
    "C2 0.1\u00b5F 25V",
    "L1 10\u00b5H",
)
# U+03A2 is the reserved gap in the Greek capital range.
GREEK_UPPER_CODEPOINTS = tuple(code for code in range(0x0391, 0x03AA) if code != 0x03A2)
GREEK_LOWER_CODEPOINTS = tuple(range(0x03B1, 0x03CA))
DEMO_GREEK_UPPER_LINE = " ".join(chr(code) for code in GREEK_UPPER_CODEPOINTS)
DEMO_GREEK_LOWER_LINE = " ".join(chr(code) for code in GREEK_LOWER_CODEPOINTS)
DEMO_SYMBOL_LINE = (
    "\u00b0 \u00b1 \u00b2 \u00b3 \u2074 \u2080 \u2081 \u2082 "
    "\u00d7 \u00f7 \u00b7 \u00b5 \u03a9 \u2202 \u2206 \u2207 \u221a "
    "\u2211 \u222b \u221e \u2248 \u2260 \u2264 \u2265 "
    "\u2190 \u2192 \u2191 \u2193 \u2022 \u212b \u23da"
)

# Demo tab metadata: (tab element id, tab label).
_DEMO_TABS = (
    ("tab-regular", "Regular"),
    ("tab-italic", "Italic"),
)


def _escape_html(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def _demo_equation_part(part: str) -> str:
    """A tab splits an equation from its law name; names align in a column."""

    equation, sep, law = part.partition("\t")
    if not sep:
        return _escape_html(part)
    return (
        f'<span class="eq-lhs">{_escape_html(equation)}</span>'
        f'<span class="eq-law">({_escape_html(law)})</span>'
    )


def _demo_equation_rows(equations: Sequence[tuple[str, str]]) -> str:
    rows: list[str] = []
    for name, formula in equations:
        stacked = "<br>".join(
            _demo_equation_part(part) for part in formula.split("    ")
        )
        rows.append(
            f'      <tr><td class="eq-name">{_escape_html(name)}</td>'
            f'<td class="eq-formula">{stacked}</td></tr>'
        )
    return "\n".join(rows)


def _demo_weight_rows() -> str:
    rows: list[str] = []
    for weight_name, label in (
        ("light", "Light"),
        ("regular", "Regular"),
        ("bold", "Bold"),
    ):
        cells = "".join(
            f"<td>{_escape_html(segment)}</td>" for segment in DEMO_SPECIMEN_SEGMENTS
        )
        rows.append(
            f'      <tr class="specimen {weight_name}">'
            f'<td class="label">{label} <span class="ratio-{weight_name}"></span></td>'
            f"{cells}</tr>"
        )
    return "\n".join(rows)


def _demo_bom_rows() -> str:
    return "\n".join(
        f'      <tr><td class="bom-ref">{_escape_html(ref)}</td>'
        f'<td class="bom-mpn">{_escape_html(mpn)}</td>'
        f"<td>{_escape_html(description)}</td></tr>"
        for ref, mpn, description in DEMO_BOM_ROWS
    )


def _demo_fab_note_items() -> str:
    return "\n".join(f"      <li>{_escape_html(note)}</li>" for note in DEMO_FAB_NOTES)


_DEMO_STYLE = """\
  body {
    margin: 0;
    padding: 2.5em 1.5em 4em;
    background: var(--mk-bg);
    color: var(--mk-text);
    font-family: 'KiCad Stroke', 'Consolas', monospace;
    font-size: 12pt;
  }
  main { max-width: 62em; margin: 0 auto; }
  header.mark { text-align: center; margin-bottom: 2.5em; }
  .monkey-art {
    display: inline-block;
    margin: 0 auto;
    font-family: 'Cascadia Mono', 'Consolas', monospace;
    font-size: 10pt;
    line-height: 1.05;
    color: var(--mk-accent);
    text-shadow: var(--mk-glow);
    text-align: left;
    font-style: normal;
  }
  h1 {
    margin: 0.4em 0 0.1em;
    font-size: 26pt;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--mk-accent);
    text-shadow: var(--mk-glow);
  }
  .tagline { margin: 0; color: var(--mk-text-dim); }
  h2 {
    font-size: 14pt;
    font-weight: 700;
    color: var(--mk-accent-bright);
    text-shadow: var(--mk-glow);
    border-bottom: 1px solid var(--mk-border);
    padding-bottom: 0.25em;
    margin: 2em 0 0.8em;
  }
  section.panel {
    background: var(--mk-panel);
    border: 1px solid var(--mk-border);
    padding: 0.4em 1.4em 1.2em;
    margin-bottom: 1.6em;
    overflow-x: auto;
  }
  table { border-collapse: collapse; width: 100%; }
  td, th { padding: 0.3em 0.9em 0.3em 0; vertical-align: top; }
  th {
    text-align: left;
    color: var(--mk-text-dim);
    font-weight: 700;
    border-bottom: 1px solid var(--mk-grid);
  }
  .eq-name { color: var(--mk-text-dim); white-space: nowrap; width: 12em; }
  .eq-formula { color: var(--mk-accent-bright); }
  .eq-formula .eq-lhs { display: inline-block; min-width: 11em; }
  .eq-formula .eq-law { color: var(--mk-text-dim); font-weight: 300; }
  .bom-ref { color: var(--mk-accent); white-space: nowrap; }
  .bom-mpn { color: var(--mk-accent-bright); white-space: nowrap; }
  .specimen { margin: 0.35em 0; }
  .specimen.light { font-weight: 300; }
  .specimen.bold { font-weight: 700; }
  .specimen .label {
    display: inline-block;
    width: 9em;
    color: var(--mk-text-dim);
    font-weight: 400;
  }
  table.weights td { white-space: nowrap; }
  table.weights td.label { display: table-cell; }
  ol.fab-notes { margin: 0.6em 0 0; padding-left: 2.4em; }
  ol.fab-notes li { margin: 0.4em 0; }
  input.style-tab { display: none; }
  nav.tabs {
    max-width: 62em;
    margin: 0 auto 1.6em;
    display: flex;
    justify-content: center;
    gap: 0.6em;
    font-family: 'Consolas', monospace;
  }
  nav.tabs label {
    padding: 0.35em 1.3em;
    border: 1px solid var(--mk-border);
    background: var(--mk-panel);
    color: var(--mk-text-dim);
    cursor: pointer;
  }
  #tab-regular:checked ~ nav.tabs label[for="tab-regular"],
  #tab-italic:checked ~ nav.tabs label[for="tab-italic"] {
    color: var(--mk-accent);
    border-color: var(--mk-accent);
    text-shadow: var(--mk-glow);
  }
  #tab-italic:checked ~ main { font-style: italic; }
"""


def _demo_ratio_css() -> str:
    """Weight-ratio captions (KiCad pen rules) for the Weights panel labels."""

    rules: list[str] = []
    for weight_name, stroke_ratio in WEIGHT_STROKE_RATIOS.items():
        denominator = 1.0 / stroke_ratio
        rules.append(
            f'  .ratio-{weight_name}::after {{ content: "size/{denominator:g}"; }}'
        )
    return "\n".join(rules) + "\n"


def _demo_tab_controls() -> str:
    inputs = "\n".join(
        f'<input type="radio" name="style-tab" id="{tab_id}" class="style-tab"'
        f"{' checked' if index == 0 else ''}>"
        for index, (tab_id, _tab_label) in enumerate(_DEMO_TABS)
    )
    labels = "\n".join(
        f'<label for="{tab_id}">{label}</label>' for tab_id, label in _DEMO_TABS
    )
    return f'{inputs}\n<nav class="tabs">\n{labels}\n</nav>\n'


def build_demo_html(art_text: str, theme_css: str, css_href: str) -> str:
    """Self-contained phosphor-themed showcase page for the webfont."""

    theme_match = re.search(r'data-theme="([a-z][a-z0-9_-]*)"', theme_css)
    theme_name = theme_match.group(1) if theme_match else "blue"
    reference_rows = _demo_equation_rows(DEMO_REFERENCE_EQUATIONS)
    field_rows = _demo_equation_rows(DEMO_FIELD_EQUATIONS)
    return (
        "<!DOCTYPE html>\n"
        f'<html lang="en" data-theme="{theme_name}">\n'
        "<head>\n"
        '<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        '<meta name="color-scheme" content="dark">\n'
        '<link rel="icon" href="data:,">\n'
        f"<title>{DEMO_TITLE} \u2014 stroke font demo</title>\n"
        f'<link rel="stylesheet" href="{css_href}">\n'
        "<style>\n"
        f"{theme_css.rstrip()}\n"
        f"{_DEMO_STYLE}"
        f"{_demo_ratio_css()}"
        "</style>\n"
        "</head>\n"
        "<body>\n"
        f"{_demo_tab_controls()}"
        "<main>\n"
        '<header class="mark">\n'
        f'<pre class="monkey-art" role="img" aria-label="kicad monkey logo">'
        f"{_escape_html(art_text.rstrip())}</pre>\n"
        f"<h1>{DEMO_TITLE}</h1>\n"
        f'<p class="tagline">{DEMO_TAGLINE}</p>\n'
        "</header>\n"
        '<section class="panel">\n'
        "<h2>ELECTRONICS REFERENCE</h2>\n"
        f"<table>\n{reference_rows}\n</table>\n"
        "</section>\n"
        '<section class="panel">\n'
        "<h2>FIELD EQUATIONS</h2>\n"
        f"<table>\n{field_rows}\n</table>\n"
        "</section>\n"
        '<section class="panel">\n'
        "<h2>BILL OF MATERIALS</h2>\n"
        "<table>\n"
        "      <tr><th>Ref</th><th>Part number</th>"
        "<th>Description</th></tr>\n"
        f"{_demo_bom_rows()}\n"
        "</table>\n"
        "</section>\n"
        '<section class="panel">\n'
        "<h2>FABRICATION NOTES</h2>\n"
        '<ol class="fab-notes">\n'
        f"{_demo_fab_note_items()}\n"
        "</ol>\n"
        "</section>\n"
        '<section class="panel">\n'
        "<h2>GREEK AND SYMBOLS</h2>\n"
        f'<div class="specimen"><span class="label">Capitals</span>'
        f"{_escape_html(DEMO_GREEK_UPPER_LINE)}</div>\n"
        f'<div class="specimen"><span class="label">Lowercase</span>'
        f"{_escape_html(DEMO_GREEK_LOWER_LINE)}</div>\n"
        f'<div class="specimen"><span class="label">Symbols</span>'
        f"{_escape_html(DEMO_SYMBOL_LINE)}</div>\n"
        "</section>\n"
        '<section class="panel">\n'
        "<h2>WEIGHTS</h2>\n"
        '<table class="weights">\n'
        f"{_demo_weight_rows()}\n"
        "</table>\n"
        "</section>\n"
        "</main>\n"
        "</body>\n"
        "</html>\n"
    )


def _split_cli_list(value: str) -> tuple[str, ...]:
    return tuple(part.strip().lower() for part in value.split(",") if part.strip())


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument(
        "--formats",
        default=",".join(DEFAULT_FORMATS),
        help=f"Comma-separated formats from: {', '.join(SUPPORTED_FORMATS)}",
    )
    parser.add_argument(
        "--light",
        action="store_true",
        help="Also build the Light weight (pen = size/12)",
    )
    parser.add_argument(
        "--bold",
        action="store_true",
        help="Also build the Bold weight (KiCad bold pen = size/5)",
    )
    parser.add_argument(
        "--italic",
        action="store_true",
        help="Also build italic variants (KiCad 1/8 shear)",
    )
    parser.add_argument("--quad-segs", type=int, default=DEFAULT_QUAD_SEGS)
    parser.add_argument("--units-per-em", type=int, default=DEFAULT_UNITS_PER_EM)
    parser.add_argument(
        "--max-codepoint",
        type=lambda value: int(value, 0),
        default=None,
        help="Optional cap on included codepoints (e.g. 0x2300) for fast builds",
    )
    parser.add_argument("--no-css", action="store_true", help="Skip writing the CSS")
    parser.add_argument(
        "--demo-art",
        type=Path,
        default=None,
        help="Monkey mark text file; with --demo-theme-css writes demo.html",
    )
    parser.add_argument(
        "--demo-theme-css",
        type=Path,
        default=None,
        help="Phosphor theme CSS file for the demo page",
    )
    return parser.parse_args(argv)


def _cli_weights(args: argparse.Namespace) -> tuple[str, ...]:
    weights: list[str] = []
    if args.light:
        weights.append("light")
    weights.append("regular")
    if args.bold:
        weights.append("bold")
    return tuple(weights)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if (args.demo_art is None) != (args.demo_theme_css is None):
        raise SystemExit("--demo-art and --demo-theme-css must be used together")
    if args.demo_art is not None and args.no_css:
        raise SystemExit("--demo-art requires CSS output; remove --no-css")
    codepoints: tuple[int, ...] | None = None
    if args.max_codepoint is not None:
        codepoints = tuple(
            code for code in default_codepoints() if code <= args.max_codepoint
        )
    result = generate_fonts(
        output_dir=args.output_dir,
        weights=_cli_weights(args),
        italic=args.italic,
        formats=_split_cli_list(args.formats),
        units_per_em=args.units_per_em,
        quad_segs=args.quad_segs,
        codepoints=codepoints,
        write_css=not args.no_css,
    )
    for font_file in result.font_files:
        print(f"wrote {font_file.path}")
    if result.css_path is not None:
        print(f"wrote {result.css_path}")
    if args.demo_art is not None and args.demo_theme_css is not None:
        demo_path = args.output_dir / DEMO_FILE_NAME
        demo_path.write_text(
            build_demo_html(
                args.demo_art.read_text(encoding="utf-8"),
                args.demo_theme_css.read_text(encoding="utf-8"),
                result.css_path.name if result.css_path is not None else CSS_FILE_NAME,
            ),
            encoding="utf-8",
            newline="\n",
        )
        print(f"wrote {demo_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

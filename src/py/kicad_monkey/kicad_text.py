"""KiCad-compatible text rendering to 2D polygons.

This module provides text rendering that matches KiCad's outline font implementation.
It uses FreeType + HarfBuzz for text shaping and glyph outline extraction.

KiCad Source Reference:
    Version: 9.0.0-rc3-4364-g5f555f4d63
    Commit: 5f555f4d63b970e410d567d1f79e05e8ce41b9d8
    Date: 2025-11-27
    Source: https://gitlab.com/kicad/code/kicad
    Key files referenced:
    - common/font/outline_font.cpp - TrueType glyph generation with FreeType/HarfBuzz
    - common/font/font.cpp - Font base class, alignment calculations
    - libs/kimath/src/bezier_curves.cpp - Bezier curve flattening algorithm
    - common/font/outline_decomposer.cpp - Text outline integration path
    - include/font/outline_font.h - Constants (OUTLINE_FONT_SIZE_COMPENSATION=1.4)
    - include/font/text_attributes.h - TEXT_ATTRIBUTES structure
    - pcbnew/pcb_text.cpp - TransformTextToPolySet(), knockout rendering
    - include/gr_text.h - GetKnockoutTextMargin() formula

Architecture:
    TextParams (input) -> KiCadTextRenderer.render() -> RenderedGeometry (output)
    RenderedGeometry -> SVGSerializer/OpenGLSerializer -> Output format

Usage:
    from kicad_text import KiCadTextRenderer
    from geometry import TextParams
    from serializers import geometry_to_svg

    renderer = KiCadTextRenderer()
    params = TextParams.from_kicad_expression('(gr_text "TEST" (at 5 2.5) ...)')
    geometry = renderer.render(params)
    svg = geometry_to_svg([geometry])

Design Notes for C++ Porting:
    - Explicit type hints on all methods and variables
    - Avoid Python-specific idioms
    - Use simple data structures that map to C++ equivalents

================================================================================
THE MAGIC: How to Match KiCad's Text Rendering
================================================================================

Getting text rendering to match KiCad exactly requires understanding several
non-obvious scale factors and their interactions.

KEY INSIGHT: The 1.4x compensation factor
-----------------------------------------
KiCad's outline fonts appear smaller than stroke fonts because stroke fonts are
measured by cap height while outline fonts use full height (ascenders+descenders).
KiCad compensates by scaling outline fonts by 1.4x (OUTLINE_FONT_SIZE_COMPENSATION).

This 1.4x is applied to the FreeType char_size, NOT to the final scale factor.
If you apply it twice (once to FT, once to scale), text will be wrong size.

SCALE FACTOR MATH:
-----------------
1. FreeType setup:
   scaler = 16 * 64 * 1.4 = 1433.6  (base_size * 26.6_factor * compensation)
   face.set_char_size(0, scaler, 1152, 0)  # Use high-res DPI

2. This produces glyphs at:
   (scaler/64) * (1152/72) = 22.4 * 16 = 358.4 device units per em

3. Final scale (WITHOUT 1.4x again!):
   coord_scale = 16 * 1152 / 72 = 256
   scale = target_size / coord_scale

4. KiCad truncates faceSize() to an integer before the final scale:
   final_scale = target_size / 1433 * 1.4

TRANSFORM ORDER (matching KiCad's pipeline):
-------------------------------------------
1. Load glyph outline from FreeType
2. Divide by 64 (26.6 fixed-point to float)
3. Add cursor position (cumulative advance)
4. Add HarfBuzz offset (kerning adjustment)
5. Scale to target size (with Y flip: vy = -gy * scale_y)
6. Apply alignment offset
7. Mirror if needed (pt.x = 2*origin - pt.x)
8. Rotate if needed
9. Translate to final position

KiCad Source References:
------------------------
- kicad/common/font/outline_font.cpp - Main glyph rendering
- kicad/common/font/font.cpp - Alignment calculation (HEIGHT_FUDGE_FACTOR)
- kicad/include/font/outline_font.h - OUTLINE_FONT_SIZE_COMPENSATION constant
- kicad/libs/kimath/src/bezier_curves.cpp - Bezier curve flattening algorithm
- kicad/common/font/outline_decomposer.cpp - Text outline integration path

See ARCHITECTURE.md for full documentation of the text rendering algorithm.
"""

from __future__ import annotations

import ctypes
import math
import os
import re
import subprocess
from dataclasses import dataclass, field
from functools import lru_cache
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple, cast

import freetype
import uharfbuzz as hb

from .kicad_geometry import (
    HAlign,
    Point,
    RenderedGeometry,
    TextParams,
    VAlign,
)

freetype = cast(Any, freetype)
hb = cast(Any, hb)


# =============================================================================
# KiCad Constants (from KiCad source)
# =============================================================================

# FreeType default DPI
GLYPH_DEFAULT_DPI: int = 72

# KiCad uses higher resolution for better outline quality
GLYPH_RESOLUTION: int = 1152

# Scale factor for converting FreeType coords
GLYPH_SIZE_SCALER: float = float(GLYPH_DEFAULT_DPI) / float(GLYPH_RESOLUTION)

# Outline font size compensation (outline_font.h line 173)
# Outline fonts are scaled on full-height (ascenders+descenders), so they appear
# smaller than stroke fonts. This compensates.
OUTLINE_FONT_SIZE_COMPENSATION: float = 1.4

# Character size scaler for FT_Set_Char_Size (1/64th of a point)
CHAR_SIZE_SCALER: int = 64

# Height fudge factor for matching KiCad 6.0 positioning (font.cpp line 203)
HEIGHT_FUDGE_FACTOR: float = 1.17

# Bezier curve flattening tolerance.  KiCad passes ADVANCED_CFG::m_FontErrorSize
# to BEZIER_POLY for outline-font decomposition; the default is 2.
BEZIER_ERROR_TOLERANCE: float = 2.0

# Outline superscript/subscript constants from include/font/outline_font.h.
SUBSCRIPT_SUPERSCRIPT_SIZE_RATIO: float = 0.64
SUBSCRIPT_VERTICAL_OFFSET: float = -0.25
SUPERSCRIPT_VERTICAL_OFFSET: float = 0.45

# Text decoration metrics from include/font/font_metrics.h.  PCB cache output
# only reaches overbar through serialized markup; underline is an internal text
# attribute in KiCad's PCB path.
OVERBAR_HEIGHT_RATIO: float = 1.23
UNDERLINE_OFFSET_RATIO: float = -0.16


# =============================================================================
# Windows font paths
# =============================================================================

WINDOWS_FONT_PATHS: List[str] = [
    "C:/Windows/Fonts",
]

FONT_FILES: Dict[str, str] = {
    "arial": "arial.ttf",
    "arial bold": "arialbd.ttf",
    "arial italic": "ariali.ttf",
    "arial bold italic": "arialbi.ttf",
    "times new roman": "times.ttf",
    "times new roman bold": "timesbd.ttf",
    "courier new": "cour.ttf",
    "courier new bold": "courbd.ttf",
    "verdana": "verdana.ttf",
    "tahoma": "tahoma.ttf",
    "calibri": "calibri.ttf",
    "calibri bold": "calibrib.ttf",
}


def _decode_font_name(value: Any) -> str:
    if isinstance(value, bytes):
        try:
            return value.decode("utf-8", errors="ignore")
        except Exception:
            return ""
    return str(value or "")


def _font_lookup_keys(value: object) -> Tuple[str, ...]:
    text = re.sub(r"\s+", " ", str(value or "").strip()).casefold()
    if not text:
        return ()

    keys = [text]
    simplified = re.sub(
        r"[-_ ]+(regular|medium|bold|italic|semibold|demibold|black)$",
        "",
        text,
        flags=re.I,
    )
    if simplified and simplified not in keys:
        keys.append(simplified)
    if "-" in text:
        prefix = text.split("-", 1)[0].strip()
        if prefix and prefix not in keys:
            keys.append(prefix)
    return tuple(keys)


def _font_style_flags(family: str, style: str) -> Tuple[bool, bool]:
    text = f"{family} {style}".casefold()
    bold = any(
        token in text
        for token in (
            "bold",
            "heavy",
            "black",
            "thick",
            "dark",
            "semibold",
            "demibold",
        )
    )
    italic = any(token in text for token in ("italic", "oblique", "slant"))
    return bold, italic


def _font_style_lookup_order(
    key: str,
    *,
    bold: bool = False,
    italic: bool = False,
) -> Tuple[Tuple[str, bool, bool], ...]:
    order = [(key, bool(bold), bool(italic))]
    if bold and italic:
        order.extend([(key, True, False), (key, False, True)])
    order.append((key, False, False))

    seen: set[Tuple[str, bool, bool]] = set()
    out: List[Tuple[str, bool, bool]] = []
    for style_key in order:
        if style_key in seen:
            continue
        seen.add(style_key)
        out.append(style_key)
    return tuple(out)


@lru_cache(maxsize=1)
def _system_font_files() -> Tuple[Path, ...]:
    search_dirs = [
        Path("C:/Windows/Fonts"),
        Path.home() / "AppData/Local/Microsoft/Windows/Fonts",
        Path.home() / ".fonts",
        Path.home() / ".local/share/fonts",
        Path("/usr/share/fonts"),
        Path("/usr/local/share/fonts"),
    ]
    if local_appdata := os.environ.get("LOCALAPPDATA"):
        search_dirs.append(Path(local_appdata) / "fonts")

    paths: List[Path] = []
    for directory in search_dirs:
        if not directory.exists():
            continue
        try:
            children = directory.rglob("*")
        except OSError:
            continue
        for font_path in children:
            if font_path.suffix.casefold() in {".ttf", ".otf", ".ttc"}:
                paths.append(font_path)

    seen: set[str] = set()
    unique: List[Path] = []
    for path in paths:
        key = str(path).casefold()
        if key in seen or not path.exists():
            continue
        seen.add(key)
        unique.append(path)
    return tuple(unique)


@lru_cache(maxsize=1)
def _system_font_paths() -> Dict[Tuple[str, bool, bool], Tuple[str, ...]]:
    out: Dict[Tuple[str, bool, bool], List[str]] = {}
    for font_path in _system_font_files():
        try:
            face = freetype.Face(str(font_path))
        except Exception:
            continue

        family = _decode_font_name(getattr(face, "family_name", ""))
        style = _decode_font_name(getattr(face, "style_name", ""))
        if not family:
            continue

        bold, italic = _font_style_flags(family, style)
        keys = set(_font_lookup_keys(family))
        if style:
            keys.update(_font_lookup_keys(f"{family} {style}"))
        for key in keys:
            out.setdefault((key, bold, italic), []).append(str(font_path))

    return {key: tuple(value) for key, value in out.items()}


@lru_cache(maxsize=128)
def _fontconfig_match_path(font_name: str, bold: bool = False, italic: bool = False) -> Optional[str]:
    query = font_name.strip() or "sans-serif"
    styles: List[str] = []
    if bold:
        styles.append("Bold")
    if italic:
        styles.append("Italic")
    if styles:
        query = f"{query}:style={','.join(styles)}"

    try:
        completed = subprocess.run(
            ["fc-match", "-f", "%{file}\n", query],
            capture_output=True,
            check=False,
            text=True,
            timeout=2.0,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode != 0:
        return None

    path_text = completed.stdout.splitlines()[0].strip() if completed.stdout.splitlines() else ""
    if not path_text:
        return None
    path = Path(path_text)
    if path.exists() and path.suffix.casefold() in {".ttf", ".otf", ".ttc"}:
        return str(path)
    return None


# =============================================================================
# Backward Compatibility Aliases
# =============================================================================

# For backward compatibility with existing code
KiCadTextParams = TextParams


# =============================================================================
# Type Aliases
# =============================================================================

FontCacheKey = Tuple[str, bool, bool]  # (font_name, bold, italic)
HBFontCacheKey = Tuple[str, int]  # (font_path, upem)


@dataclass(slots=True)
class _GlyphInfo:
    codepoint: int


@dataclass(slots=True)
class _GlyphPosition:
    x_advance: float
    y_advance: float
    x_offset: float
    y_offset: float


@dataclass(slots=True)
class _ShapedText:
    infos: List[Any]
    positions: List[Any]
    position_scale: float
    integer_cursor: bool
    apply_offsets: bool


@dataclass(slots=True)
class _TextStyle:
    subscript: bool = False
    superscript: bool = False


@dataclass(slots=True)
class _MarkupPart:
    text: str = ""
    marker: str = ""
    children: List["_MarkupPart"] = field(default_factory=list)


class _HarfBuzzFtApi:
    class _HbGlyphInfo(ctypes.Structure):
        _fields_ = [
            ("codepoint", ctypes.c_uint32),
            ("mask", ctypes.c_uint32),
            ("cluster", ctypes.c_uint32),
            ("var1", ctypes.c_uint32),
            ("var2", ctypes.c_uint32),
        ]

    class _HbGlyphPosition(ctypes.Structure):
        _fields_ = [
            ("x_advance", ctypes.c_int32),
            ("y_advance", ctypes.c_int32),
            ("x_offset", ctypes.c_int32),
            ("y_offset", ctypes.c_int32),
            ("var", ctypes.c_uint32),
        ]

    def __init__(self, dll_path: Path) -> None:
        self._dll_directory_handle = None
        add_dll_directory = cast(
            "Callable[[str], object] | None",
            getattr(os, "add_dll_directory", None),
        )
        if add_dll_directory is not None:
            self._dll_directory_handle = add_dll_directory(str(dll_path.parent))
        self.lib = ctypes.CDLL(str(dll_path))
        self.lib.hb_buffer_create.restype = ctypes.c_void_p
        self.lib.hb_buffer_add_utf8.argtypes = [
            ctypes.c_void_p,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_uint,
            ctypes.c_int,
        ]
        self.lib.hb_buffer_guess_segment_properties.argtypes = [ctypes.c_void_p]
        self.lib.hb_ft_font_create_referenced.argtypes = [ctypes.c_void_p]
        self.lib.hb_ft_font_create_referenced.restype = ctypes.c_void_p
        self.lib.hb_shape.argtypes = [
            ctypes.c_void_p,
            ctypes.c_void_p,
            ctypes.c_void_p,
            ctypes.c_uint,
        ]
        self.lib.hb_buffer_get_glyph_infos.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_uint),
        ]
        self.lib.hb_buffer_get_glyph_infos.restype = ctypes.POINTER(self._HbGlyphInfo)
        self.lib.hb_buffer_get_glyph_positions.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_uint),
        ]
        self.lib.hb_buffer_get_glyph_positions.restype = ctypes.POINTER(self._HbGlyphPosition)
        self.lib.hb_buffer_destroy.argtypes = [ctypes.c_void_p]
        self.lib.hb_font_destroy.argtypes = [ctypes.c_void_p]

    def shape(self, face: Any, text: str) -> Tuple[List[_GlyphInfo], List[_GlyphPosition]]:
        buffer = self.lib.hb_buffer_create()
        font = None
        try:
            encoded = text.encode("utf-8")
            self.lib.hb_buffer_add_utf8(buffer, encoded, -1, 0, -1)
            self.lib.hb_buffer_guess_segment_properties(buffer)
            ft_face = face._FT_Face
            if ft_face is None:
                return [], []
            font = self.lib.hb_ft_font_create_referenced(
                ctypes.cast(ft_face, ctypes.c_void_p)
            )
            self.lib.hb_shape(font, buffer, None, 0)
            count = ctypes.c_uint(0)
            infos_ptr = self.lib.hb_buffer_get_glyph_infos(buffer, ctypes.byref(count))
            positions_ptr = self.lib.hb_buffer_get_glyph_positions(buffer, ctypes.byref(count))
            infos = [
                _GlyphInfo(codepoint=int(infos_ptr[index].codepoint))
                for index in range(count.value)
            ]
            positions = [
                _GlyphPosition(
                    x_advance=float(positions_ptr[index].x_advance),
                    y_advance=float(positions_ptr[index].y_advance),
                    x_offset=float(positions_ptr[index].x_offset),
                    y_offset=float(positions_ptr[index].y_offset),
                )
                for index in range(count.value)
            ]
            return infos, positions
        finally:
            if font:
                self.lib.hb_font_destroy(font)
            self.lib.hb_buffer_destroy(buffer)


_HBFT_API: Optional[_HarfBuzzFtApi] = None
_HBFT_API_MISSING = False


def _find_harfbuzz_dll() -> Optional[Path]:
    env_path = os.environ.get("KICAD_HARFBUZZ_DLL")
    if env_path and Path(env_path).is_file():
        return Path(env_path)

    return None


def _get_hbft_api() -> Optional[_HarfBuzzFtApi]:
    global _HBFT_API, _HBFT_API_MISSING
    if _HBFT_API_MISSING:
        return None
    if _HBFT_API is not None:
        return _HBFT_API

    dll_path = _find_harfbuzz_dll()
    if dll_path is None:
        _HBFT_API_MISSING = True
        return None

    try:
        _HBFT_API = _HarfBuzzFtApi(dll_path)
    except OSError:
        _HBFT_API_MISSING = True
        return None

    return _HBFT_API


# =============================================================================
# Text Renderer
# =============================================================================

class KiCadTextRenderer:
    """Renders text to 2D polygons matching KiCad's implementation.

    The renderer produces RenderedGeometry objects that contain polygon contours.
    These can then be serialized to various output formats using the serializers.

    Example:
        renderer = KiCadTextRenderer()
        params = TextParams(text="TEST", font_name="Arial", size_x=1.0, size_y=1.0)
        geometry = renderer.render(params)

        # Convert to SVG
        from serializers import geometry_to_svg
        svg = geometry_to_svg([geometry])

        # Or convert to OpenGL mesh
        from serializers import geometry_to_opengl
        vertices, indices = geometry_to_opengl([geometry])
    """

    def __init__(self) -> None:
        self._font_cache: Dict[FontCacheKey, Any] = {}
        self._font_data_cache: Dict[str, bytes] = {}
        self._hb_font_cache: Dict[HBFontCacheKey, Any] = {}
        # Embedded fonts: font_name -> (font_data_bytes, virtual_path)
        self._embedded_fonts: Dict[str, Tuple[bytes, str]] = {}

    def register_embedded_font(self, font_name: str, font_data: bytes) -> str:
        """Register an embedded font from raw TTF data.

        Args:
            font_name: The font name (e.g., "Wavenumber")
            font_data: Raw TTF/OTF font file bytes

        Returns:
            Virtual path that can be used to reference the font
        """
        # Create a virtual path for the embedded font
        virtual_path: str = f"embedded://{font_name}"

        aliases = set(_font_lookup_keys(font_name))
        aliases.update(_font_lookup_keys(Path(font_name).stem))
        try:
            face = freetype.Face.from_bytes(font_data)
            aliases.update(_font_lookup_keys(_decode_font_name(face.family_name)))
        except Exception:
            pass

        # Store the font data under filename and FreeType family aliases.
        for alias in aliases:
            self._embedded_fonts[alias] = (font_data, virtual_path)

        # Pre-cache the font data so HarfBuzz can use it
        self._font_data_cache[virtual_path] = font_data

        return virtual_path

    # =========================================================================
    # Font Loading
    # =========================================================================

    def _find_font_file(
        self,
        font_name: str,
        bold: bool = False,
        italic: bool = False
    ) -> Optional[str]:
        """Find the font file path for a given font name.

        First checks embedded fonts, then known local files, system font
        indexes, fontconfig, and finally the Windows Arial fallback.
        """
        # Check embedded fonts first
        base_name: str = font_name.lower()
        if base_name in self._embedded_fonts:
            return self._embedded_fonts[base_name][1]  # Return virtual path

        # Also check with -regular suffix stripped (e.g., "brand-regular" -> "brand")
        if '-' in base_name:
            stripped = base_name.split('-')[0]
            if stripped in self._embedded_fonts:
                return self._embedded_fonts[stripped][1]

        base_key: str = font_name.lower()
        style_keys: List[str] = []
        if bold and italic:
            # KiCad asks fontconfig for separate "Bold" and "Italic" style
            # values.  On Windows/Arial this resolves to the bold face and
            # KiCad fakes the missing italic slant, rather than selecting
            # arialbi.ttf.
            style_keys.extend([
                base_key + " bold",
                base_key + " italic",
                base_key + " bold italic",
            ])
        elif bold:
            style_keys.append(base_key + " bold")
        elif italic:
            style_keys.append(base_key + " italic")

        for key in style_keys:
            if key in FONT_FILES:
                for path in WINDOWS_FONT_PATHS:
                    full_path: str = path + "/" + FONT_FILES[key]
                    try:
                        with open(full_path, 'rb'):
                            return full_path
                    except FileNotFoundError:
                        continue

        # Try base font without style
        if base_key in FONT_FILES:
            for path in WINDOWS_FONT_PATHS:
                full_path = path + "/" + FONT_FILES[base_key]
                try:
                    with open(full_path, 'rb'):
                        return full_path
                except FileNotFoundError:
                    continue

        system_fonts = _system_font_paths()
        for key in _font_lookup_keys(font_name):
            for style_key in _font_style_lookup_order(key, bold=bold, italic=italic):
                for candidate in system_fonts.get(style_key, ()):
                    if Path(candidate).exists():
                        return candidate

        fontconfig_path = _fontconfig_match_path(font_name, bold=bold, italic=italic)
        if fontconfig_path is not None:
            return fontconfig_path

        # Fallback to Arial
        for path in WINDOWS_FONT_PATHS:
            full_path = path + "/arial.ttf"
            try:
                with open(full_path, 'rb'):
                    return full_path
            except FileNotFoundError:
                continue

        return None

    def _get_font(
        self,
        font_name: str,
        bold: bool = False,
        italic: bool = False
    ) -> Any | None:
        """Get or load a FreeType font face."""
        cache_key: FontCacheKey = (font_name, bold, italic)
        if cache_key in self._font_cache:
            return self._font_cache[cache_key]

        font_path: Optional[str] = self._find_font_file(font_name, bold, italic)
        if font_path is None:
            return None

        try:
            # Handle embedded fonts (virtual paths)
            if font_path.startswith("embedded://"):
                # Load from cached data
                font_data: bytes = self._font_data_cache[font_path]
                face: Any = freetype.Face.from_bytes(font_data)
            else:
                # Load from file
                face = freetype.Face(font_path)
                with open(font_path, 'rb') as f:
                    self._font_data_cache[font_path] = f.read()

            self._font_cache[cache_key] = face
            return face
        except Exception as e:
            print(f"Error loading font {font_name}: {e}")
            return None

    def _get_hb_font(self, font_path: str, upem: int) -> Any:
        """Get or create a HarfBuzz font for text shaping."""
        cache_key: HBFontCacheKey = (font_path, upem)
        if cache_key in self._hb_font_cache:
            return self._hb_font_cache[cache_key]

        if font_path not in self._font_data_cache:
            with open(font_path, 'rb') as f:
                self._font_data_cache[font_path] = f.read()

        blob: Any = getattr(hb, "Blob")(self._font_data_cache[font_path])
        hb_face: Any = getattr(hb, "Face")(blob)
        font: Any = getattr(hb, "Font")(hb_face)
        font.scale = (upem, upem)

        self._hb_font_cache[cache_key] = font
        return font

    def _shape_text(self, hb_font: Any, text: str) -> Tuple[List[Any], List[Any]]:
        """Use HarfBuzz to shape text and get glyph positions."""
        buf: Any = getattr(hb, "Buffer")()
        buf.add_str(text)
        buf.guess_segment_properties()
        getattr(hb, "shape")(hb_font, buf)
        return buf.glyph_infos, buf.glyph_positions

    def _shape_text_for_face(
        self,
        face: Any,
        hb_font: Any,
        text: str,
        fallback_scaler: Optional[int] = None,
    ) -> _ShapedText:
        hbft_api = _get_hbft_api()
        if hbft_api is not None:
            infos, positions = hbft_api.shape(face, text)
            return _ShapedText(
                infos=infos,
                positions=positions,
                position_scale=GLYPH_SIZE_SCALER,
                integer_cursor=True,
                apply_offsets=False,
            )

        upem: int = face.units_per_EM if face.units_per_EM is not None else 2048
        infos, positions = self._shape_text(hb_font, text)
        scaler = fallback_scaler if fallback_scaler is not None else self._compute_face_size(16)
        return _ShapedText(
            infos=list(infos),
            positions=list(positions),
            position_scale=float(scaler) / float(upem),
            integer_cursor=False,
            apply_offsets=True,
        )

    @staticmethod
    def _advance_cursor(cursor: float, delta: float, integer_cursor: bool) -> float:
        next_cursor = cursor + delta
        if integer_cursor:
            return float(math.trunc(next_cursor))
        return next_cursor

    @staticmethod
    def _round_to_int(value: float) -> int:
        return int(math.floor(value + 0.5))

    @classmethod
    def _arc_segment_count(cls, radius: float, error: float, arc_angle_deg: float = 360.0) -> int:
        radius = max(radius, 1e-9)
        error = max(error, 1e-9)
        rel_error = error / radius
        arc_increment = 180.0 / math.pi * math.acos(max(-1.0, 1.0 - rel_error)) * 2.0
        arc_increment = min(360.0 / 8.0, arc_increment)
        seg_count = cls._round_to_int(abs(arc_angle_deg) / arc_increment)
        return max(seg_count, 2)

    @classmethod
    def _stroke_segment_to_polygon(
        cls,
        start: Point,
        end: Point,
        width: float,
    ) -> List[Point]:
        """Port KiCad's ERROR_INSIDE `TransformOvalToPolygon()` point order.

        `CALLBACK_GAL::DrawGlyph()` turns stroke glyphs, including overbar
        markup, into oval polygons with error `strokeWidth / 180`.
        """

        if width <= 0.0:
            return []

        start_x, start_y = start
        end_x, end_y = end
        delta_x = end_x - start_x
        delta_y = end_y - start_y

        if delta_x < 0.0:
            start_x, start_y, end_x, end_y = end_x, end_y, start_x, start_y
            delta_x = end_x - start_x
            delta_y = end_y - start_y

        seg_len = math.hypot(delta_x, delta_y)
        radius = width / 2.0
        error = width / 180.0
        num_segs = cls._arc_segment_count(radius, error, 360.0)
        num_segs = max(8, num_segs)
        num_segs = ((num_segs + 7) // 8) * 8
        delta = math.pi * 2.0 / float(num_segs)

        if seg_len > 1e-12:
            ux = delta_x / seg_len
            uy = delta_y / seg_len
        else:
            ux = 1.0
            uy = 0.0

        def place(local_x: float, local_y: float) -> Point:
            return (
                start_x + local_x * ux - local_y * uy,
                start_y + local_x * uy + local_y * ux,
            )

        points: List[Point] = []

        for index in range(num_segs // 2):
            angle = delta / 2.0 + float(index) * delta
            points.append(
                place(
                    seg_len + radius * math.sin(angle),
                    -radius * math.cos(angle),
                )
            )

        points.append(place(seg_len, radius))
        points.append(place(0.0, radius))

        for index in range(num_segs // 2):
            angle = delta / 2.0 + float(index) * delta
            points.append(
                place(
                    -radius * math.sin(angle),
                    radius * math.cos(angle),
                )
            )

        points.append(place(0.0, -radius))
        points.append(place(seg_len, -radius))
        return points

    @classmethod
    def _subscript_scaler(cls, scaler: int) -> int:
        return cls._round_to_int(float(scaler) * SUBSCRIPT_SUPERSCRIPT_SIZE_RATIO)

    @classmethod
    def _parse_markup(cls, text: str) -> List[_MarkupPart]:
        """Parse KiCad's lightweight `^{}`, `_{}`, and `~{}` markup syntax."""

        def parse_parts(index: int, stop_on_brace: bool = False) -> Tuple[List[_MarkupPart], int]:
            parts: List[_MarkupPart] = []
            buffer: List[str] = []

            def flush_buffer() -> None:
                if buffer:
                    parts.append(_MarkupPart(text="".join(buffer)))
                    buffer.clear()

            while index < len(text):
                char = text[index]
                if stop_on_brace and char == "}":
                    flush_buffer()
                    return parts, index + 1

                if index + 1 < len(text) and char in "^_~" and text[index + 1] == "{":
                    flush_buffer()
                    children, index = parse_parts(index + 2, stop_on_brace=True)
                    parts.append(_MarkupPart(marker=char, children=children))
                    continue

                buffer.append(char)
                index += 1

            flush_buffer()
            return parts, index

        parts, _index = parse_parts(0)
        return parts

    @staticmethod
    def _child_text_style(style: _TextStyle, marker: str) -> _TextStyle:
        if marker == "_":
            return _TextStyle(subscript=True, superscript=style.superscript)
        if marker == "^":
            return _TextStyle(subscript=style.subscript, superscript=True)
        return _TextStyle(subscript=style.subscript, superscript=style.superscript)

    @staticmethod
    def _vertical_style_offset(style: _TextStyle, scaler: int) -> float:
        if style.subscript:
            return SUBSCRIPT_VERTICAL_OFFSET * float(scaler)
        if style.superscript:
            return SUPERSCRIPT_VERTICAL_OFFSET * float(scaler)
        return 0.0

    def measure_run_width(self, params: TextParams, text: str) -> float:
        """Measure a single shaped run in millimeters.

        KiCad's text-box linebreaker asks the font for unrotated run widths.
        This mirrors the same HarfBuzz advance path used by outline rendering.
        """

        if not text:
            return 0.0

        face: Any | None = self._get_font(
            params.font_name, params.bold, params.italic
        )
        if face is None:
            return 0.0

        font_path: Optional[str] = self._find_font_file(
            params.font_name, params.bold, params.italic
        )
        if font_path is None:
            return 0.0

        scaler: int = self._compute_face_size(16)
        face.set_char_size(0, scaler, GLYPH_RESOLUTION, 0)
        upem: int = face.units_per_EM if face.units_per_EM is not None else 2048
        hb_font: Any = self._get_hb_font(font_path, upem)
        shaped = self._shape_text_for_face(face, hb_font, text, fallback_scaler=scaler)
        cursor_x = 0.0
        for pos in shaped.positions:
            cursor_x = self._advance_cursor(
                cursor_x,
                float(pos.x_advance) * shaped.position_scale,
                shaped.integer_cursor,
            )
        scale_x = params.size_x / float(scaler) * OUTLINE_FONT_SIZE_COMPENSATION
        return cursor_x * scale_x

    def measure_markup_width(self, params: TextParams, text: str) -> float:
        """Measure a text run with KiCad superscript/subscript markup."""

        if not text:
            return 0.0

        face: Any | None = self._get_font(
            params.font_name, params.bold, params.italic
        )
        if face is None:
            return 0.0

        font_path: Optional[str] = self._find_font_file(
            params.font_name, params.bold, params.italic
        )
        if font_path is None:
            return 0.0

        base_scaler: int = self._compute_face_size(16)
        scale_x: float = params.size_x / float(base_scaler) * OUTLINE_FONT_SIZE_COMPENSATION
        upem: int = face.units_per_EM if face.units_per_EM is not None else 2048
        hb_font: Any = self._get_hb_font(font_path, upem)

        def styled_scaler(style: _TextStyle) -> int:
            if style.subscript or style.superscript:
                return self._subscript_scaler(base_scaler)
            return base_scaler

        def measure_plain(text_run: str, style: _TextStyle) -> float:
            if not text_run:
                return 0.0
            run_scaler = styled_scaler(style)
            face.set_char_size(0, run_scaler, GLYPH_RESOLUTION, 0)
            shaped = self._shape_text_for_face(
                face,
                hb_font,
                text_run,
                fallback_scaler=run_scaler,
            )
            cursor_x = 0.0
            for pos in shaped.positions:
                cursor_x = self._advance_cursor(
                    cursor_x,
                    float(pos.x_advance) * shaped.position_scale,
                    shaped.integer_cursor,
                )
            return cursor_x * scale_x

        def advance_tabs(text_run: str, style: _TextStyle, position_x: float) -> float:
            runs = text_run.split("\t")
            for index, run in enumerate(runs):
                position_x += measure_plain(run, style)
                if index < len(runs) - 1:
                    tab_width = params.size_x * 4.0 * 0.6
                    if tab_width > 0:
                        current_intrusion = position_x % tab_width
                        position_x += tab_width - current_intrusion
            return position_x

        def measure_parts(parts: List[_MarkupPart], style: _TextStyle, position_x: float) -> float:
            for part in parts:
                if part.marker:
                    position_x = measure_parts(
                        part.children,
                        self._child_text_style(style, part.marker),
                        position_x,
                    )
                else:
                    position_x = advance_tabs(part.text, style, position_x)
            return position_x

        return measure_parts(self._parse_markup(text), _TextStyle(), 0.0)

    @classmethod
    def _markup_part_source(cls, part: _MarkupPart) -> str:
        if not part.marker:
            return part.text
        return (
            f"{part.marker}{{"
            + "".join(cls._markup_part_source(child) for child in part.children)
            + "}"
        )

    def _wordbreak_markup(self, params: TextParams, text_line: str) -> List[Tuple[str, float]]:
        """Break a single line into KiCad markup-aware words."""

        markup_runs: List[Tuple[str, float]] = []
        for part in self._parse_markup(text_line):
            if part.marker:
                token = self._markup_part_source(part)
                markup_runs.append((token, self.measure_markup_width(params, token)))
                continue

            for token in re.findall(r" +|[^ ]+", part.text):
                measured = token.strip() or token
                markup_runs.append((token, self.measure_markup_width(params, measured)))

        words: List[Tuple[str, float]] = []
        for run, run_width in markup_runs:
            if words and not words[-1][0].endswith(" "):
                previous_run, previous_width = words[-1]
                words[-1] = (previous_run + run, previous_width + run_width)
            else:
                words.append((run, run_width))

        return words

    def linebreak_text(self, params: TextParams, column_width: float) -> str:
        """Insert KiCad-style text-box line breaks.

        This follows `KIFONT::FONT::LinebreakText()`: split existing lines,
        tokenize normal text on spaces, keep marked-up runs as single words,
        and wrap only when pending spaces would overflow the column.
        """

        if column_width <= 0.0 or not params.text:
            return params.text

        space_width = self.measure_run_width(params, " ")
        output: List[str] = []
        text_lines = params.text.split("\n")

        for line_index, text_line in enumerate(text_lines):
            bury_mode = False
            line_width = 0.0
            pending_spaces = ""

            for word, word_width in self._wordbreak_markup(params, text_line):
                pending_space_width = len(pending_spaces) * space_width
                overflow = (
                    line_width + pending_space_width + word_width
                    > column_width - params.stroke_width
                )

                if overflow and pending_spaces:
                    output.append("\n")
                    line_width = 0.0
                    pending_spaces = ""
                    pending_space_width = 0.0
                    bury_mode = True

                if word == " ":
                    pending_spaces += word
                else:
                    if bury_mode:
                        bury_mode = False
                    else:
                        output.append(pending_spaces)
                        line_width += pending_space_width

                    if word.endswith(" "):
                        output.append(word[:-1])
                        pending_spaces = " "
                    else:
                        output.append(word)
                        pending_spaces = ""

                    line_width += word_width

            if line_index != len(text_lines) - 1:
                output.append("\n")

        return "".join(output)

    @staticmethod
    def _face_has_bold(face: Any) -> bool:
        return bool(face.style_flags & getattr(freetype, "FT_STYLE_FLAG_BOLD"))

    @staticmethod
    def _face_has_italic(face: Any) -> bool:
        return bool(face.style_flags & getattr(freetype, "FT_STYLE_FLAG_ITALIC"))

    @staticmethod
    def _set_fake_italic_transform(face: Any, enabled: bool) -> None:
        matrix = freetype.Matrix()
        if enabled:
            angle = float(-math.pi * 12.0) / 180.0
            matrix.xx = int(math.cos(angle) * 0x10000)
            matrix.xy = int(-math.sin(angle) * 0x10000)
            matrix.yx = 0
            matrix.yy = 0x10000
        else:
            matrix.xx = 0x10000
            matrix.xy = 0
            matrix.yx = 0
            matrix.yy = 0x10000

        delta = freetype.Vector()
        delta.x = 0
        delta.y = 0
        face.set_transform(matrix, delta)

    # =========================================================================
    # Bezier Curve Flattening
    # =========================================================================

    @staticmethod
    def _vector_add(a: Point, b: Point) -> Point:
        return (a[0] + b[0], a[1] + b[1])

    @staticmethod
    def _vector_sub(a: Point, b: Point) -> Point:
        return (a[0] - b[0], a[1] - b[1])

    @staticmethod
    def _vector_mul(scale: float, point: Point) -> Point:
        return (scale * point[0], scale * point[1])

    @staticmethod
    def _dot(a: Point, b: Point) -> float:
        return a[0] * b[0] + a[1] * b[1]

    @staticmethod
    def _cross(a: Point, b: Point) -> float:
        return a[0] * b[1] - a[1] * b[0]

    @classmethod
    def _squared_norm(cls, point: Point) -> float:
        return cls._dot(point, point)

    @classmethod
    def _euclidean_norm(cls, point: Point) -> float:
        return math.sqrt(cls._squared_norm(point))

    @staticmethod
    def _approx_int(value: float) -> float:
        d: float = 0.6744897501960817
        d4: float = d * d * d * d
        return value / (1.0 - d + math.pow(d4 + value * value * 0.25, 0.25))

    @staticmethod
    def _approx_inv_int(value: float) -> float:
        p: float = 0.39538816
        return value * (1.0 - p + math.sqrt(p * p + 0.25 * value * value))

    @classmethod
    def _bezier_eval(cls, control_points: List[Point], t: float) -> Point:
        omt: float = 1.0 - t
        omt2: float = omt * omt

        if len(control_points) == 3:
            return cls._vector_add(
                cls._vector_add(
                    cls._vector_mul(omt2, control_points[0]),
                    cls._vector_mul(2.0 * omt * t, control_points[1]),
                ),
                cls._vector_mul(t * t, control_points[2]),
            )

        if len(control_points) == 4:
            omt3: float = omt * omt2
            t2: float = t * t
            t3: float = t * t2
            return cls._vector_add(
                cls._vector_add(
                    cls._vector_mul(omt3, control_points[0]),
                    cls._vector_mul(3.0 * t * omt2, control_points[1]),
                ),
                cls._vector_add(
                    cls._vector_mul(3.0 * t2 * omt, control_points[2]),
                    cls._vector_mul(t3, control_points[3]),
                ),
            )

        raise ValueError("Bezier control point count must be 3 or 4")

    @classmethod
    def _bezier_is_flat(cls, control_points: List[Point], max_error: float) -> bool:
        if len(control_points) == 3:
            d21 = cls._vector_sub(control_points[1], control_points[0])
            d31 = cls._vector_sub(control_points[2], control_points[0])
            d31_norm = cls._squared_norm(d31)
            if d31_norm == 0.0:
                return True
            t: float = cls._dot(d21, d31) / d31_norm
            u: float = min(max(t, 0.0), 1.0)
            projected = cls._vector_add(control_points[0], cls._vector_mul(u, d31))
            delta = cls._vector_sub(control_points[1], projected)
            return 0.5 * cls._euclidean_norm(delta) <= max_error

        if len(control_points) == 4:
            delta = cls._vector_sub(control_points[3], control_points[0])
            delta_norm = cls._squared_norm(delta)
            if delta_norm == 0.0:
                return True

            d21 = cls._vector_sub(control_points[1], control_points[0])
            d31 = cls._vector_sub(control_points[2], control_points[0])
            cross1: float = cls._cross(delta, d21)
            cross2: float = cls._cross(delta, d31)

            inv_delta_sq: float = 1.0 / delta_norm
            d1: float = (cross1 * cross1) * inv_delta_sq
            d2: float = (cross2 * cross2) * inv_delta_sq

            factor: float = 3.0 / 4.0 if cross1 * cross2 > 0.0 else 4.0 / 9.0
            f2: float = factor * factor
            tolerance: float = max_error * max_error
            return d1 * f2 <= tolerance and d2 * f2 <= tolerance

        raise ValueError("Bezier control point count must be 3 or 4")

    @classmethod
    def _bezier_subdivide(cls, control_points: List[Point], t: float) -> Tuple[List[Point], List[Point]]:
        if len(control_points) == 3:
            left = [
                control_points[0],
                cls._vector_add(
                    control_points[0],
                    cls._vector_mul(t, cls._vector_sub(control_points[1], control_points[0])),
                ),
                cls._bezier_eval(control_points, t),
            ]
            right = [
                left[2],
                cls._vector_add(
                    control_points[1],
                    cls._vector_mul(t, cls._vector_sub(control_points[2], control_points[1])),
                ),
                control_points[2],
            ]
            return left, right

        if len(control_points) == 4:
            left_ctrl1 = cls._vector_add(
                control_points[0],
                cls._vector_mul(t, cls._vector_sub(control_points[1], control_points[0])),
            )
            tmp = cls._vector_add(
                control_points[1],
                cls._vector_mul(t, cls._vector_sub(control_points[2], control_points[1])),
            )
            left_ctrl2 = cls._vector_add(left_ctrl1, cls._vector_mul(t, cls._vector_sub(tmp, left_ctrl1)))
            right_ctrl2 = cls._vector_add(
                control_points[2],
                cls._vector_mul(t, cls._vector_sub(control_points[3], control_points[2])),
            )
            right_ctrl1 = cls._vector_add(tmp, cls._vector_mul(t, cls._vector_sub(right_ctrl2, tmp)))
            shared = cls._vector_add(left_ctrl2, cls._vector_mul(t, cls._vector_sub(right_ctrl1, left_ctrl2)))

            return (
                [control_points[0], left_ctrl1, left_ctrl2, shared],
                [shared, right_ctrl1, right_ctrl2, control_points[3]],
            )

        raise ValueError("Bezier control point count must be 3 or 4")

    @classmethod
    def _bezier_get_quad_poly(cls, control_points: List[Point], max_error: float) -> List[Point]:
        ddx: float = 2.0 * control_points[1][0] - control_points[0][0] - control_points[2][0]
        ddy: float = 2.0 * control_points[1][1] - control_points[0][1] - control_points[2][1]
        u0: float = (control_points[1][0] - control_points[0][0]) * ddx + (
            control_points[1][1] - control_points[0][1]
        ) * ddy
        u2: float = (control_points[2][0] - control_points[1][0]) * ddx + (
            control_points[2][1] - control_points[1][1]
        ) * ddy
        cross: float = (control_points[2][0] - control_points[0][0]) * ddy - (
            control_points[2][1] - control_points[0][1]
        ) * ddx
        denom: float = math.hypot(ddx, ddy)
        if cross == 0.0 or denom == 0.0:
            return [control_points[0], control_points[2]]

        x0: float = u0 / cross
        x2: float = u2 / cross
        if x2 == x0:
            return [control_points[0], control_points[2]]

        scale: float = abs(cross) / (denom * abs(x2 - x0))
        if max_error <= 0.0 or scale <= 0.0:
            return [control_points[0], control_points[2]]

        a0: float = cls._approx_int(x0)
        a2: float = cls._approx_int(x2)
        segment_count: int = math.ceil(0.5 * abs(a2 - a0) * math.sqrt(scale / max_error))

        v0: float = cls._approx_inv_int(a0)
        v2: float = cls._approx_inv_int(a2)
        output: List[Point] = [control_points[0]]

        for index in range(segment_count):
            if v2 == v0:
                break
            u: float = cls._approx_inv_int(a0 + (a2 - a0) * index / segment_count)
            t: float = (u - v0) / (v2 - v0)
            output.append(cls._bezier_eval(control_points, t))

        output.append(control_points[2])
        return output

    @classmethod
    def _bezier_number_of_inflection_points(cls, control_points: List[Point]) -> int:
        d21 = cls._vector_sub(control_points[1], control_points[0])
        d32 = cls._vector_sub(control_points[2], control_points[1])
        d43 = cls._vector_sub(control_points[3], control_points[2])

        cross1: float = cls._cross(d21, d32) * cls._cross(d32, d43)
        cross2: float = cls._cross(d21, d32) * cls._cross(d21, d43)

        if cross1 < 0.0:
            return 1
        if cross2 > 0.0:
            return 0

        b1: bool = cls._dot(d21, d32) > 0.0
        b2: bool = cls._dot(d32, d43) > 0.0

        if b1 ^ b2:
            return 0

        return -1

    @classmethod
    def _bezier_third_control_point_deviation(cls, control_points: List[Point]) -> float:
        delta = cls._vector_sub(control_points[1], control_points[0])
        len_sq: float = cls._squared_norm(delta)
        if len_sq < 1e-6:
            return 0.0

        length: float = math.sqrt(len_sq)
        r: float = (control_points[1][1] - control_points[0][1]) / length
        s: float = (control_points[0][0] - control_points[1][0]) / length
        u: float = (control_points[1][0] * control_points[0][1] - control_points[0][0] * control_points[1][1]) / length
        return abs(r * control_points[2][0] + s * control_points[2][1] + u)

    @classmethod
    def _bezier_recursive_segmentation(cls, control_points: List[Point], max_error: float) -> List[Point]:
        output: List[Point] = []
        stack: List[List[Point]] = [list(control_points)]

        while stack:
            bezier = stack.pop()
            if bezier[3] == bezier[0]:
                continue
            if cls._bezier_is_flat(bezier, max_error):
                output.append(bezier[3])
                continue

            left, right = cls._bezier_subdivide(bezier, 0.5)
            stack.append(right)
            stack.append(left)

        return output

    @classmethod
    def _bezier_find_inflection_points(cls, control_points: List[Point]) -> Tuple[int, float, float]:
        a = (
            -control_points[0][0] + 3.0 * control_points[1][0] - 3.0 * control_points[2][0] + control_points[3][0],
            -control_points[0][1] + 3.0 * control_points[1][1] - 3.0 * control_points[2][1] + control_points[3][1],
        )
        b = (
            3.0 * control_points[0][0] - 6.0 * control_points[1][0] + 3.0 * control_points[2][0],
            3.0 * control_points[0][1] - 6.0 * control_points[1][1] + 3.0 * control_points[2][1],
        )
        c = (
            -3.0 * control_points[0][0] + 3.0 * control_points[1][0],
            -3.0 * control_points[0][1] + 3.0 * control_points[1][1],
        )

        qa: float = 3.0 * cls._cross(a, b)
        qb: float = 3.0 * cls._cross(a, c)
        qc: float = cls._cross(b, c)
        root_term: float = qb * qb - 4.0 * qa * qc

        if root_term >= 0.0 and qa != 0.0:
            root: float = math.sqrt(root_term)
            t1: float = (-qb + root) / (2.0 * qa)
            t2: float = (-qb - root) / (2.0 * qa)

            if 0.0 < t1 < 1.0 and 0.0 < t2 < 1.0:
                if t1 > t2:
                    t1, t2 = t2, t1
                if t2 - t1 > 0.00001:
                    return 2, t1, t2
                return 1, t1, t2
            if 0.0 < t1 < 1.0:
                return 1, t1, 0.0
            if 0.0 < t2 < 1.0:
                return 1, t2, 0.0

        return 0, 0.0, 0.0

    @classmethod
    def _bezier_cubic_parabolic_approx(cls, control_points: List[Point], max_error: float) -> List[Point]:
        output: List[Point] = []
        current = list(control_points)

        while True:
            if any(math.isnan(coord) for point in current for coord in point):
                break

            if cls._bezier_is_flat(current, max_error):
                output.append(current[3])
                break

            deviation: float = cls._bezier_third_control_point_deviation(current)
            if deviation <= 0.0:
                output.extend(cls._bezier_recursive_segmentation(current, max_error))
                break

            t: float = 2.0 * math.sqrt(max_error / (3.0 * deviation))
            if not math.isfinite(t) or t > 1.0:
                output.extend(cls._bezier_recursive_segmentation(current, max_error))
                break

            first, second = cls._bezier_subdivide(current, t)
            if cls._bezier_is_flat(first, max_error):
                output.append(first[3])
            else:
                output.extend(cls._bezier_recursive_segmentation(first, max_error))

            current = second

        return output

    @classmethod
    def _bezier_get_cubic_poly(cls, control_points: List[Point], max_error: float) -> List[Point]:
        output: List[Point] = [control_points[0]]

        if cls._bezier_number_of_inflection_points(control_points) == 0:
            output.extend(cls._bezier_cubic_parabolic_approx(control_points, max_error))
            return output

        inflection_count, t1, _t2 = cls._bezier_find_inflection_points(control_points)

        if inflection_count == 2:
            sub1, tmp1 = cls._bezier_subdivide(control_points, t1)
            output.extend(cls._bezier_cubic_parabolic_approx(sub1, max_error))

            second_count, second_t1, _second_t2 = cls._bezier_find_inflection_points(tmp1)
            if second_count in (1, 2):
                sub2, sub3 = cls._bezier_subdivide(tmp1, second_t1)
            else:
                output.append(tmp1[3])
                return output

            output.extend(cls._bezier_recursive_segmentation(sub2, max_error))
            output.extend(cls._bezier_cubic_parabolic_approx(sub3, max_error))
        elif inflection_count == 1:
            sub1, sub2 = cls._bezier_subdivide(control_points, t1)
            output.extend(cls._bezier_cubic_parabolic_approx(sub1, max_error))
            output.extend(cls._bezier_cubic_parabolic_approx(sub2, max_error))
        else:
            output.extend(cls._bezier_cubic_parabolic_approx(control_points, max_error))

        return output

    @classmethod
    def _bezier_get_poly(cls, control_points: List[Point], max_error: float) -> List[Point]:
        if max_error <= 0.0:
            max_error = 10.0
        if len(control_points) == 3:
            return cls._bezier_get_quad_poly(control_points, max_error)
        if len(control_points) == 4:
            return cls._bezier_get_cubic_poly(control_points, max_error)
        raise ValueError("Bezier control point count must be 3 or 4")

    # =========================================================================
    # Outline Extraction
    # =========================================================================

    def _get_point(self, outline: Any, idx: int) -> Point:
        """Get outline point, converting from 26.6 fixed-point.

        FreeType returns outline coordinates in 26.6 fixed-point format (value * 64).
        We convert to float by dividing by 64.
        """
        p: Tuple[int, int] = outline.points[idx]
        return (float(p[0]) / 64.0, float(p[1]) / 64.0)

    @staticmethod
    def _outline_vector_to_kicad_point(vector: Any) -> Point:
        return (float(vector.x) * GLYPH_SIZE_SCALER, float(vector.y) * GLYPH_SIZE_SCALER)

    def _outline_to_contours(
        self,
        outline: Any,
        error_tolerance: float = BEZIER_ERROR_TOLERANCE
    ) -> List[List[Point]]:
        """Convert FreeType outline to KiCad-style polygon contours."""
        contours: List[List[Point]] = []
        current: List[Point] = []
        last_point: Point = (0.0, 0.0)

        def add_contour_point(point: Point) -> None:
            if not current or current[-1] != point:
                current.append(point)

        def move_to(end_point: Any, _context: Any) -> int:
            nonlocal current, last_point
            if current:
                contours.append(current)
                current = []
            last_point = self._outline_vector_to_kicad_point(end_point)
            add_contour_point(last_point)
            return 0

        def line_to(end_point: Any, _context: Any) -> int:
            nonlocal last_point
            last_point = self._outline_vector_to_kicad_point(end_point)
            add_contour_point(last_point)
            return 0

        def conic_to(control_point: Any, end_point: Any, context: Any) -> int:
            return cubic_to(control_point, None, end_point, context)

        def cubic_to(
            first_control_point: Any,
            second_control_point: Optional[Any],
            end_point: Any,
            _context: Any,
        ) -> int:
            nonlocal last_point
            control_points: List[Point] = [
                last_point,
                self._outline_vector_to_kicad_point(first_control_point),
            ]
            if second_control_point is not None:
                control_points.append(self._outline_vector_to_kicad_point(second_control_point))
            control_points.append(self._outline_vector_to_kicad_point(end_point))

            for point in self._bezier_get_poly(control_points, error_tolerance):
                add_contour_point(point)

            last_point = self._outline_vector_to_kicad_point(end_point)
            return 0

        outline.decompose(
            move_to=move_to,
            line_to=line_to,
            conic_to=conic_to,
            cubic_to=cubic_to,
            shift=0,
            delta=0,
        )

        if current:
            contours.append(current)

        return contours

    def _compute_face_size(self, base_size: int = 16) -> int:
        """Compute FreeType face size with KiCad's compensation.

        KiCad: faceSize = size * 64 * 1.4
        """
        return int(float(base_size) * float(CHAR_SIZE_SCALER) * OUTLINE_FONT_SIZE_COMPENSATION)

    # =========================================================================
    # Main Rendering Methods
    # =========================================================================

    def render(self, params: TextParams) -> RenderedGeometry:
        """Render text to neutral geometry form.

        This is the primary rendering method. It produces a RenderedGeometry
        object that can be serialized to various output formats.

        Args:
            params: TextParams with all text properties

        Returns:
            RenderedGeometry containing polygon contours
        """
        contours: List[List[Point]] = self._render_contours(params)

        geometry = RenderedGeometry(
            source_text=params.text,
            layer=params.layer,
            is_knockout=params.knockout,
        )

        for contour in contours:
            geometry.add_contour(contour)

        return geometry

    def _styled_scaler_for_style(self, style: _TextStyle, scaler: int) -> int:
        if style.subscript or style.superscript:
            return self._subscript_scaler(scaler)
        return scaler

    def _measure_plain_run_width_for_face(
        self,
        face: Any,
        hb_font: Any,
        text: str,
        style: _TextStyle,
        *,
        scaler: int,
        scale_x: float,
    ) -> float:
        if not text:
            return 0.0
        run_scaler = self._styled_scaler_for_style(style, scaler)
        face.set_char_size(0, run_scaler, GLYPH_RESOLUTION, 0)
        shaped = self._shape_text_for_face(
            face,
            hb_font,
            text,
            fallback_scaler=run_scaler,
        )
        cursor_x = 0.0
        for pos in shaped.positions:
            cursor_x = self._advance_cursor(
                cursor_x,
                float(pos.x_advance) * shaped.position_scale,
                shaped.integer_cursor,
            )
        return cursor_x * scale_x

    def _advance_plain_width_for_face(
        self,
        face: Any,
        hb_font: Any,
        text: str,
        style: _TextStyle,
        *,
        params: TextParams,
        scaler: int,
        scale_x: float,
        position_x: float,
    ) -> float:
        runs = text.split("\t")
        for index, run in enumerate(runs):
            position_x += self._measure_plain_run_width_for_face(
                face,
                hb_font,
                run,
                style,
                scaler=scaler,
                scale_x=scale_x,
            )
            if index < len(runs) - 1:
                tab_width = params.size_x * 4.0 * 0.6
                if tab_width > 0:
                    position_x += tab_width - position_x % tab_width
        return position_x

    def _measure_markup_parts_for_face(
        self,
        face: Any,
        hb_font: Any,
        parts: List[_MarkupPart],
        style: _TextStyle,
        *,
        params: TextParams,
        scaler: int,
        scale_x: float,
        position_x: float,
    ) -> float:
        for part in parts:
            if part.marker:
                position_x = self._measure_markup_parts_for_face(
                    face,
                    hb_font,
                    part.children,
                    self._child_text_style(style, part.marker),
                    params=params,
                    scaler=scaler,
                    scale_x=scale_x,
                    position_x=position_x,
                )
            else:
                position_x = self._advance_plain_width_for_face(
                    face,
                    hb_font,
                    part.text,
                    style,
                    params=params,
                    scaler=scaler,
                    scale_x=scale_x,
                    position_x=position_x,
                )
        return position_x

    def _measure_line_width_for_face(
        self,
        face: Any,
        hb_font: Any,
        line: str,
        *,
        params: TextParams,
        scaler: int,
        scale_x: float,
    ) -> float:
        return self._measure_markup_parts_for_face(
            face,
            hb_font,
            self._parse_markup(line),
            _TextStyle(),
            params=params,
            scaler=scaler,
            scale_x=scale_x,
            position_x=0.0,
        )

    @staticmethod
    def _transform_rendered_text_point(
        params: TextParams,
        *,
        cos_a: float,
        sin_a: float,
        x: float,
        y: float,
    ) -> Point:
        origin_x = params.position_x
        origin_y = params.position_y
        if params.mirrored:
            x = origin_x - (x - origin_x)

        if params.angle != 0.0:
            dx = x - origin_x
            dy = y - origin_y
            x = origin_x + dx * cos_a + dy * sin_a
            y = origin_y + dy * cos_a - dx * sin_a

        return (x, y)

    def _render_text_run_contours(
        self,
        face: Any,
        hb_font: Any,
        text: str,
        run_x: float,
        run_y: float,
        style: _TextStyle,
        *,
        params: TextParams,
        scaler: int,
        scale_x: float,
        scale_y: float,
        fake_bold: bool,
        fake_italic: bool,
        cos_a: float,
        sin_a: float,
    ) -> Tuple[List[List[Point]], float]:
        if not text:
            return [], run_x

        run_scaler = self._styled_scaler_for_style(style, scaler)
        face.set_char_size(0, run_scaler, GLYPH_RESOLUTION, 0)
        shaped = self._shape_text_for_face(
            face,
            hb_font,
            text,
            fallback_scaler=run_scaler,
        )
        contours_out: List[List[Point]] = []
        cursor_x = 0.0
        cursor_y = 0.0
        vertical_offset = self._vertical_style_offset(style, run_scaler)

        for index, info in enumerate(shaped.infos):
            pos: Any = shaped.positions[index]
            self._set_fake_italic_transform(face, fake_italic)
            face.load_glyph(info.codepoint, getattr(freetype, "FT_LOAD_NO_BITMAP"))
            if fake_bold:
                freetype.FT_Outline_Embolden(face.glyph.outline._FT_Outline, 1 << 6)

            if face.glyph.outline.n_points > 0:
                self._append_glyph_contours(
                    contours_out,
                    face.glyph.outline,
                    cursor_x=cursor_x,
                    cursor_y=cursor_y,
                    position=pos,
                    shaped=shaped,
                    vertical_offset=vertical_offset,
                    run_x=run_x,
                    run_y=run_y,
                    params=params,
                    scale_x=scale_x,
                    scale_y=scale_y,
                    cos_a=cos_a,
                    sin_a=sin_a,
                )

            cursor_x = self._advance_cursor(
                cursor_x,
                float(pos.x_advance) * shaped.position_scale,
                shaped.integer_cursor,
            )
            cursor_y = self._advance_cursor(
                cursor_y,
                float(pos.y_advance) * shaped.position_scale,
                shaped.integer_cursor,
            )

        return contours_out, run_x + cursor_x * scale_x

    def _append_glyph_contours(
        self,
        contours_out: List[List[Point]],
        outline: Any,
        *,
        cursor_x: float,
        cursor_y: float,
        position: Any,
        shaped: Any,
        vertical_offset: float,
        run_x: float,
        run_y: float,
        params: TextParams,
        scale_x: float,
        scale_y: float,
        cos_a: float,
        sin_a: float,
    ) -> None:
        hb_x_offset = 0.0
        hb_y_offset = 0.0
        if shaped.apply_offsets:
            hb_x_offset = float(position.x_offset) * shaped.position_scale
            hb_y_offset = float(position.y_offset) * shaped.position_scale

        for contour in self._outline_to_contours(outline):
            transformed: List[Point] = []
            for pt in contour:
                gx = pt[0] + cursor_x + hb_x_offset
                gy = pt[1] + cursor_y + hb_y_offset + vertical_offset
                vx = run_x + gx * scale_x
                vy = run_y - gy * scale_y
                transformed.append(
                    self._transform_rendered_text_point(
                        params,
                        cos_a=cos_a,
                        sin_a=sin_a,
                        x=vx,
                        y=vy,
                    )
                )
            if transformed:
                contours_out.append(transformed)

    def _render_plain_text_contours(
        self,
        face: Any,
        hb_font: Any,
        text: str,
        run_x: float,
        run_y: float,
        style: _TextStyle,
        *,
        params: TextParams,
        scaler: int,
        scale_x: float,
        scale_y: float,
        fake_bold: bool,
        fake_italic: bool,
        cos_a: float,
        sin_a: float,
    ) -> Tuple[List[List[Point]], float]:
        contours_out: List[List[Point]] = []
        runs = text.split("\t")
        for run_index, run in enumerate(runs):
            run_contours, run_x = self._render_text_run_contours(
                face,
                hb_font,
                run,
                run_x,
                run_y,
                style,
                params=params,
                scaler=scaler,
                scale_x=scale_x,
                scale_y=scale_y,
                fake_bold=fake_bold,
                fake_italic=fake_italic,
                cos_a=cos_a,
                sin_a=sin_a,
            )
            contours_out.extend(run_contours)

            if run_index < len(runs) - 1:
                tab_width = params.size_x * 4.0 * 0.6
                if tab_width > 0:
                    current_intrusion = (run_x - params.position_x) % tab_width
                    run_x += tab_width - current_intrusion

        return contours_out, run_x

    def _render_markup_part_contours(
        self,
        face: Any,
        hb_font: Any,
        parts: List[_MarkupPart],
        run_x: float,
        run_y: float,
        style: _TextStyle,
        *,
        params: TextParams,
        scaler: int,
        scale_x: float,
        scale_y: float,
        fake_bold: bool,
        fake_italic: bool,
        cos_a: float,
        sin_a: float,
    ) -> Tuple[List[List[Point]], float]:
        contours_out: List[List[Point]] = []
        for part in parts:
            if part.marker:
                marker_start_x = run_x
                child_contours, run_x = self._render_markup_part_contours(
                    face,
                    hb_font,
                    part.children,
                    run_x,
                    run_y,
                    self._child_text_style(style, part.marker),
                    params=params,
                    scaler=scaler,
                    scale_x=scale_x,
                    scale_y=scale_y,
                    fake_bold=fake_bold,
                    fake_italic=fake_italic,
                    cos_a=cos_a,
                    sin_a=sin_a,
                )
                contours_out.extend(child_contours)
                self._append_overbar_contour(
                    contours_out,
                    part.marker,
                    marker_start_x,
                    run_x,
                    run_y,
                    params,
                    cos_a=cos_a,
                    sin_a=sin_a,
                )
            else:
                text_contours, run_x = self._render_plain_text_contours(
                    face,
                    hb_font,
                    part.text,
                    run_x,
                    run_y,
                    style,
                    params=params,
                    scaler=scaler,
                    scale_x=scale_x,
                    scale_y=scale_y,
                    fake_bold=fake_bold,
                    fake_italic=fake_italic,
                    cos_a=cos_a,
                    sin_a=sin_a,
                )
                contours_out.extend(text_contours)

        return contours_out, run_x

    def _append_overbar_contour(
        self,
        contours_out: List[List[Point]],
        marker: str | None,
        marker_start_x: float,
        run_x: float,
        run_y: float,
        params: TextParams,
        *,
        cos_a: float,
        sin_a: float,
    ) -> None:
        if marker != "~":
            return
        bar_trim = params.size_x * 0.1
        bar_offset = params.size_y * OVERBAR_HEIGHT_RATIO
        bar_start = self._transform_rendered_text_point(
            params,
            cos_a=cos_a,
            sin_a=sin_a,
            x=marker_start_x + bar_trim,
            y=run_y - bar_offset,
        )
        bar_end = self._transform_rendered_text_point(
            params,
            cos_a=cos_a,
            sin_a=sin_a,
            x=run_x - bar_trim,
            y=run_y - bar_offset,
        )
        bar_contour = self._stroke_segment_to_polygon(
            bar_start,
            bar_end,
            params.stroke_width,
        )
        if bar_contour:
            contours_out.append(bar_contour)

    @staticmethod
    def _vertical_line_offset(params: TextParams, *, line_count: int, interline: float) -> float:
        height = params.size_y * HEIGHT_FUDGE_FACTOR
        if line_count > 1:
            height += interline * float(line_count - 1)

        if params.v_align == VAlign.TOP:
            return params.size_y
        if params.v_align == VAlign.CENTER:
            return params.size_y - height / 2.0
        return params.size_y - height

    @staticmethod
    def _horizontal_line_offset(params: TextParams, line_width: float) -> float:
        if params.h_align == HAlign.CENTER:
            return -line_width / 2.0
        if params.h_align == HAlign.RIGHT:
            return -line_width
        return 0.0

    def _render_contours(self, params: TextParams) -> List[List[Point]]:
        """Internal method to render text to contour lists."""
        face: Any | None = self._get_font(params.font_name, params.bold, params.italic)
        if face is None:
            return []

        font_path: Optional[str] = self._find_font_file(params.font_name, params.bold, params.italic)
        if font_path is None:
            return []

        scaler: int = self._compute_face_size(16)
        face.set_char_size(0, scaler, GLYPH_RESOLUTION, 0)
        upem: int = face.units_per_EM if face.units_per_EM is not None else 2048
        hb_font: Any = self._get_hb_font(font_path, upem)
        scale_x: float = params.size_x / float(scaler) * OUTLINE_FONT_SIZE_COMPENSATION
        scale_y: float = params.size_y / float(scaler) * OUTLINE_FONT_SIZE_COMPENSATION
        fake_bold: bool = params.bold and not self._face_has_bold(face)
        fake_italic: bool = params.italic and not self._face_has_italic(face)

        rad: float = math.radians(params.angle)
        cos_a: float = math.cos(rad)
        sin_a: float = math.sin(rad)

        lines = params.text.split("\n")
        line_widths = [
            self._measure_line_width_for_face(
                face,
                hb_font,
                line,
                params=params,
                scaler=scaler,
                scale_x=scale_x,
            )
            for line in lines
        ]
        line_spacing = float(getattr(params, "line_spacing", 1.0))
        interline = params.size_y * 1.68 * line_spacing
        v_offset = self._vertical_line_offset(params, line_count=len(lines), interline=interline)

        final_contours: List[List[Point]] = []
        for line_index, line in enumerate(lines):
            h_offset = self._horizontal_line_offset(params, line_widths[line_index])
            run_x = params.position_x + h_offset
            run_y = params.position_y + v_offset + float(line_index) * interline
            run_contours, _run_x = self._render_markup_part_contours(
                face,
                hb_font,
                self._parse_markup(line),
                run_x,
                run_y,
                _TextStyle(),
                params=params,
                scaler=scaler,
                scale_x=scale_x,
                scale_y=scale_y,
                fake_bold=fake_bold,
                fake_italic=fake_italic,
                cos_a=cos_a,
                sin_a=sin_a,
            )
            final_contours.extend(run_contours)

        return final_contours

    # =========================================================================
    # Backward Compatibility Methods
    # =========================================================================

    def get_text_polygons(self, params: TextParams) -> List[List[Point]]:
        """Render text to 2D polygon contours.

        DEPRECATED: Use render() instead for the new architecture.

        Args:
            params: TextParams with all text properties

        Returns:
            List of polygon contours, each contour is a list of (x, y) points in mm
        """
        return self._render_contours(params)

    def to_svg(
        self,
        params: TextParams,
        board_width: Optional[float] = None,
        board_height: Optional[float] = None,
        board_origin_x: float = 0.0,
        board_origin_y: float = 0.0,
        fill_color: str = '#F2EDA1',
        edge_color: str = '#D0D2CD'
    ) -> str:
        """Generate SVG output.

        DEPRECATED: Use render() instead and serialize manually.

        Args:
            params: Text parameters
            board_width: Board width in mm
            board_height: Board height in mm
            board_origin_x: Board origin X
            board_origin_y: Board origin Y
            fill_color: Fill color
            edge_color: Edge color

        Returns:
            SVG string
        """
        raise NotImplementedError(
            "to_svg() is deprecated. Use render() to get RenderedGeometry, "
            "then serialize the contours manually."
        )


# =============================================================================
# Comparison utilities
# =============================================================================

def compare_with_kicad_svg(our_svg_path: str, kicad_svg_path: str) -> Dict[str, Any]:
    """Compare our SVG with KiCad's reference SVG.

    Returns dict with comparison metrics.
    """
    def extract_paths(svg_content: str) -> List[List[Point]]:
        """Extract path coordinates from SVG (filled paths only)."""
        paths: List[List[Point]] = []
        path_pattern: str = r'<path[^>]*style="([^"]*)"[^>]*d="([^"]+)"'
        for match in re.finditer(path_pattern, svg_content):
            style: str = match.group(1)
            d: str = match.group(2)

            if 'fill:none' in style or 'fill: none' in style:
                continue

            coords: List[Point] = []
            parts: List[str] = d.replace('\n', ' ').split()
            i: int = 0
            while i < len(parts):
                part: str = parts[i]
                if part in ('M', 'L'):
                    i += 1
                    if i < len(parts):
                        xy: List[str] = parts[i].split(',')
                        if len(xy) == 2:
                            coords.append((float(xy[0]), float(xy[1])))
                    i += 1
                elif part == 'Z':
                    i += 1
                elif ',' in part:
                    xy = part.split(',')
                    if len(xy) == 2:
                        try:
                            coords.append((float(xy[0]), float(xy[1])))
                        except ValueError:
                            pass
                    i += 1
                else:
                    i += 1
            if len(coords) > 0:
                paths.append(coords)
        return paths

    with open(our_svg_path, 'r') as f:
        our_content: str = f.read()
    with open(kicad_svg_path, 'r') as f:
        kicad_content: str = f.read()

    our_paths: List[List[Point]] = extract_paths(our_content)
    kicad_paths: List[List[Point]] = extract_paths(kicad_content)

    result: Dict[str, Any] = {
        'our_path_count': len(our_paths),
        'kicad_path_count': len(kicad_paths),
        'match': len(our_paths) == len(kicad_paths),
    }

    if len(our_paths) > 0 and len(kicad_paths) > 0:
        our_xs: List[float] = [p[0] for path in our_paths for p in path]
        our_ys: List[float] = [p[1] for path in our_paths for p in path]
        kicad_xs: List[float] = [p[0] for path in kicad_paths for p in path]
        kicad_ys: List[float] = [p[1] for path in kicad_paths for p in path]

        result['our_bounds'] = {
            'min_x': min(our_xs), 'max_x': max(our_xs),
            'min_y': min(our_ys), 'max_y': max(our_ys),
        }
        result['kicad_bounds'] = {
            'min_x': min(kicad_xs), 'max_x': max(kicad_xs),
            'min_y': min(kicad_ys), 'max_y': max(kicad_ys),
        }

        our_cx: float = (result['our_bounds']['min_x'] + result['our_bounds']['max_x']) / 2.0
        our_cy: float = (result['our_bounds']['min_y'] + result['our_bounds']['max_y']) / 2.0
        kicad_cx: float = (result['kicad_bounds']['min_x'] + result['kicad_bounds']['max_x']) / 2.0
        kicad_cy: float = (result['kicad_bounds']['min_y'] + result['kicad_bounds']['max_y']) / 2.0

        result['center_offset'] = {
            'x': our_cx - kicad_cx,
            'y': our_cy - kicad_cy,
        }

        our_w: float = result['our_bounds']['max_x'] - result['our_bounds']['min_x']
        our_h: float = result['our_bounds']['max_y'] - result['our_bounds']['min_y']
        kicad_w: float = result['kicad_bounds']['max_x'] - result['kicad_bounds']['min_x']
        kicad_h: float = result['kicad_bounds']['max_y'] - result['kicad_bounds']['min_y']

        result['size_ratio'] = {
            'width': our_w / kicad_w if kicad_w > 0.0 else 0.0,
            'height': our_h / kicad_h if kicad_h > 0.0 else 0.0,
        }

        if len(our_paths) >= 1 and len(kicad_paths) >= 1:
            our_first: Point = our_paths[0][0]
            kicad_first: Point = kicad_paths[0][0]
            result['first_glyph_offset'] = {
                'x': our_first[0] - kicad_first[0],
                'y': our_first[1] - kicad_first[1],
            }

    return result


# =============================================================================
# Main test
# =============================================================================

if __name__ == '__main__':
    renderer = KiCadTextRenderer()

    # Match the test case: "TEST" at center, Arial 1mm, center/center alignment
    params = TextParams(
        text="TEST",
        font_name="Arial",
        size_x=1.0,
        size_y=1.0,
        position_x=5.0,
        position_y=2.5,
        angle=0.0,
        h_align=HAlign.CENTER,
        v_align=VAlign.CENTER,
    )

    # Render to geometry
    geometry: RenderedGeometry = renderer.render(params)
    print(f"Rendered: {geometry.get_point_count()} points in {geometry.get_contour_count()} contours")
    print(f"Bounds: {geometry.get_bounds()}")

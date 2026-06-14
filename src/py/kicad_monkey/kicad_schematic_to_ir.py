"""
KiCadSchematic to KiCadPlotterDocument converter.

Walks a parsed :class:`KiCadSchematic` and emits a
:class:`KiCadPlotterDocument` whose records contain
:class:`KiCadPlotterOp` instances drawn from the PLOTTER vocabulary.

Mirrors the natural emit order in
``SCH_IO_KICAD_SEXPR::saveSchematicFile``:
  sheet_header → wires → buses → bus_entries → junctions →
  no_connects → labels (local/global/hierarchical) → texts →
  symbol instances → hierarchical sheets.

Coordinate convention: ``.kicad_sch`` files store positions in mm
already in screen-Y (Y-down) convention — the same convention used
by KiCad's PLOTTER. Unlike :mod:`kicad_lib_symbol_to_ir` (which
flips Y-up library coords), the schematic boundary applies *no* Y
flip; we just multiply by 1_000_000 to land in nm.

The converter emits:
  * Sheet header w/ paper size + title block.
  * Wires / buses / bus entries / junctions / no_connects:
    full geometry.
  * Labels (local / global / hierarchical): text body only.
  * Top-level (text ...) annotations: full geometry.
  * Hierarchical sheets: header records only (sheet name & file +
    rectangle).

``symbol_instance`` records include composed symbol bodies when the
placement's ``lib_id`` resolves against the schematic's embedded
``lib_symbols`` block. The body is composed via :func:`lib_symbol_to_ir` and
re-anchored at the placement via :class:`KiCadPlotterTransform2D`.
Placements whose library symbol is not in ``lib_symbols`` fall back to
empty-ops header-only records.
"""

from __future__ import annotations

import base64
import binascii
import importlib
from functools import lru_cache
import hashlib
import json
import math
import os
from pathlib import Path
import re
import subprocess
import tempfile
from typing import TYPE_CHECKING, Any, Callable, List, Optional, Tuple, cast

from .kicad_lib_symbol_to_ir import (
    _effects_to_text_kwargs,
    _pin_direction_flags,
    _pin_direction_local_nm,
    _pin_number_text_kwargs,
    _pin_text_clearance_nm,
    _select_subsymbols,
    lib_symbol_to_ir,
    mm_to_nm,
    pin_graphic_style_to_ops,
    rgba_to_hex,
    stroke_type_to_line_style,
    sym_fill_to_kicad_fill,
    y_to_ir,
)
from .kicad_plotter_ir import (
    KiCadFillType,
    KiCadHorizAlign,
    KiCadLineStyle,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadVertAlign,
    styled_plotter_op,
)
from .kicad_plotter_transform import (
    KiCadPlotterTransform2D,
    apply_transform_to_op,
    transform_point,
)
from .kicad_schematic_style import (
    DEFAULT_KICAD_DRAWING_SHEET_VERSION_TEXT,
    DEFAULT_MIN_PLOT_PEN_WIDTH_NM,
    DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
    DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM,
    LAYER_BUS,
    LAYER_DNP_MARKER,
    LAYER_GLOBLABEL,
    LAYER_HIERLABEL,
    LAYER_JUNCTION,
    LAYER_LOCLABEL,
    LAYER_NOCONNECT,
    LAYER_NOTES,
    LAYER_PINNAM,
    LAYER_SCHEMATIC_BACKGROUND,
    LAYER_SHEET,
    LAYER_SHEETLABEL,
    LAYER_WIRE,
    apply_default_text_style,
    sheet_property_layer_color,
    symbol_property_layer_color,
)
from .kicad_base import find_all_elements, find_element, unquote_string
from .kicad_schematic_ids import schematic_pin_group_id, schematic_sheet_pin_group_id

if TYPE_CHECKING:
    from .kicad_lib_symbol import LibSymbol
    from .kicad_schematic import KiCadSchematic
    from .kicad_sch_junction import SchJunction
    from .kicad_sch_label import (
        SchGlobalLabel,
        SchHierarchicalLabel,
        SchLabel,
        SchNetclassFlag,
    )
    from .kicad_sch_no_connect import SchNoConnect
    from .kicad_sch_sheet import SchSheet
    from .kicad_sch_shapes import (
        SchArc,
        SchBezier,
        SchCircle,
        SchPolyline,
        SchRectangle,
    )
    from .kicad_sch_image import SchImage
    from .kicad_sch_symbol import SchSymbol
    from .kicad_sch_text import SchText
    from .kicad_sch_text_box import SchTextBox
    from .kicad_sch_title_block import PaperSize
    from .kicad_sch_wire import SchBus, SchBusEntry, SchWire


# ---------------------------------------------------------------------------
# Paper / page sizing
# ---------------------------------------------------------------------------


def _kicad_standard_page_mm(width_mm: float, height_mm: float) -> Tuple[float, float]:
    """Mirror KiCad PAGE_INFO's standard-page mm -> integer-mil conversion."""
    return (
        int(width_mm * 1000.0 / 25.4 + 0.5) * 0.0254,
        int(height_mm * 1000.0 / 25.4 + 0.5) * 0.0254,
    )


# Page dimensions in mm for KiCad's standard paper sizes.
# Values mirror ``common/page_info.cpp``: ISO A* sizes are first rounded
# to integer mils by ``EDA_UNIT_UTILS::Mm2mils`` and the SVG plotter then
# converts those mils back to mm. Width is the long side; landscape is the
# default — ``portrait=True`` swaps W/H at conversion time.
_PAPER_DIMENSIONS_MM: dict[str, Tuple[float, float]] = {
    "A0": _kicad_standard_page_mm(1189.0, 841.0),
    "A1": _kicad_standard_page_mm(841.0, 594.0),
    "A2": _kicad_standard_page_mm(594.0, 420.0),
    "A3": _kicad_standard_page_mm(420.0, 297.0),
    "A4": _kicad_standard_page_mm(297.0, 210.0),
    "A5": _kicad_standard_page_mm(210.0, 148.0),
    # ANSI sizes (mils → mm via ×0.0254).
    "A": (279.4, 215.9),    # 11000 × 8500 mils
    "B": (431.8, 279.4),    # 17000 × 11000
    "C": (558.8, 431.8),    # 22000 × 17000
    "D": (863.6, 558.8),    # 34000 × 22000
    "E": (1117.6, 863.6),   # 44000 × 34000
    "USLetter": (279.4, 215.9),
    "USLegal":  (355.6, 215.9),
    "USLedger": (431.8, 279.4),
}


def paper_size_to_nm(paper: "PaperSize") -> Tuple[int, int]:
    """Return ``(width_nm, height_nm)`` for a :class:`PaperSize`.

    Honours ``paper.portrait`` (swaps width/height). For ``size="User"``
    with both ``width`` and ``height`` set, uses those mm values
    directly. Unknown sizes fall back to A4 so the converter never
    raises on a malformed page.
    """
    if (
        paper.size == "User"
        and paper.width is not None
        and paper.height is not None
    ):
        w_mm, h_mm = float(paper.width), float(paper.height)
    else:
        w_mm, h_mm = _PAPER_DIMENSIONS_MM.get(
            paper.size, _PAPER_DIMENSIONS_MM["A4"]
        )
    if paper.portrait:
        w_mm, h_mm = h_mm, w_mm
    return mm_to_nm(w_mm), mm_to_nm(h_mm)


# ---------------------------------------------------------------------------
# Default eeschema dimensions
# ---------------------------------------------------------------------------


_MIL_TO_MM = 0.0254
# Default wire/bus widths track ``DEFAULT_WIRE_WIDTH_MILS = 6`` and
# ``DEFAULT_BUS_WIDTH_MILS = 12`` from ``schematic_settings.h``.
DEFAULT_WIRE_WIDTH_MM = 6.0 * _MIL_TO_MM
DEFAULT_BUS_WIDTH_MM = 12.0 * _MIL_TO_MM
# eeschema renders junctions as a filled disc with diameter 0.9144mm
# (= 36 mils) by default. ``junction.diameter == 0`` means "use default".
DEFAULT_JUNCTION_DIAMETER_MM = 0.9144
# eeschema's no-connect "X" extends ±0.635 mm (= 25 mils) from the pin.
DEFAULT_NO_CONNECT_HALF_MM = 0.6096
# SCH_SYMBOL::PlotDNP uses 3x DEFAULT_LINE_WIDTH_MILS.
DEFAULT_DNP_MARKER_STROKE_WIDTH_NM = mm_to_nm(DEFAULT_WIRE_WIDTH_MM * 3.0)
# Label / text default font size (1.27mm = 50 mils) when the parsed
# ``Effects`` block is absent or carries 0.
DEFAULT_TEXT_SIZE_MM = 1.27
# Global-label box-margin / text-height ratio (``DEFAULT_LABEL_SIZE_RATIO``
# in eeschema's ``default_values.h``, surfaced via
# ``SCH_LABEL_BASE::GetLabelBoxExpansion``). Used to compute the symmetric
# margin around the global-label arrow-box outline.
DEFAULT_LABEL_SIZE_RATIO = 0.375
# ``DEFAULT_TEXT_OFFSET_RATIO`` from eeschema/default_values.h. KiCad applies
# this to label text before plotting so the glyphs sit clear of the wire.
DEFAULT_TEXT_OFFSET_RATIO = 0.15
# KiCad outline-font constants from include/font/outline_font.h and
# include/font/outline_decomposer.h.
_OUTLINE_FONT_FACE_SIZE = int(16 * 64 * 1.4)
_OUTLINE_FONT_SIZE_COMPENSATION = 1.4
_OUTLINE_FONT_SUBSCRIPT_SUPERSCRIPT_SIZE = 0.64
_GLYPH_RESOLUTION = 1152
_GLYPH_SIZE_SCALER = 72.0 / _GLYPH_RESOLUTION
_FONT_METRICS_INTERLINE_PITCH = 1.68
_EDA_TEXT_BBOX_FUDGE_RATIO = 0.17
_EDA_TEXT_OVERBAR_HEIGHT_RATIO = 1.0 / 6.0
_SCH_TEXT_FIELD_MATCH_ADJUST_RATIO = 0.4
# SCH_TEXT::GetSchematicTextOffset() returns KiCad's historical 0.25 mm
# plotting nudge. SCH_TEXT::GetOffsetToMatchSCH_FIELD() then offsets
# outline-font text by 40% of the difference between KiCad's first-line
# outline-font text-box height and the schematic text size.
_SCH_TEXT_PLOT_OFFSET_NM = 250_000
_EMBEDDED_OUTLINE_FONT_PATHS: dict[tuple[str, bool, bool], str] = {}
_REGISTERED_EMBEDDED_FONT_SOURCES: set[str] = set()
_FONTCONFIG_MISSING_FONT_SUBSTITUTES: dict[str, tuple[str, bool, bool]] = {
    # These are not metric calibration constants.  They mirror the font names
    # reported by KiCad's FONTCONFIG::FindFont reporter when the current
    # staged Windows CLI is missing these requested faces.
    "avenir black": ("Bookman Old Style", True, False),
    "berkeley mono trial": ("Cascadia Code", False, False),
    "fira code": ("Cascadia Code", False, False),
    "fira sans medium": ("Cascadia Code", False, False),
    "fira sans semibold": ("Bookman Old Style", True, False),
}
_FONTCONFIG_MISSING_FONT_STYLE_SUBSTITUTES: dict[tuple[str, bool, bool], tuple[str, bool, bool]] = {
    # Observed from the same fontconfig DLL used by the staged KiCad CLI oracle.
    # Fragment Mono resolves to different fallback families depending on the
    # requested style, so a family-only substitute gives wrong SVG textLength.
    ("fragment mono", False, False): ("Cascadia Code", False, False),
    ("fragment mono", False, True): ("Montserrat", False, True),
    ("fragment mono", True, False): ("Bookman Old Style", True, False),
    ("fragment mono", True, True): ("Bookman Old Style", True, False),
}


def _symbol_body_stroke_width_for_schematic(sch: "KiCadSchematic") -> int:
    return DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM


def _symbol_polyline_stroke_width_for_schematic(sch: "KiCadSchematic") -> int:
    return DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM


@lru_cache(maxsize=64)
def _project_file_for_schematic_path(path_text: str) -> str:
    path = Path(path_text)
    if not path.exists():
        return ""

    candidates = [path.with_suffix(".kicad_pro")]
    candidates.extend(sorted(path.parent.glob("*.kicad_pro")))
    for parent in path.parent.parents:
        candidates.extend(sorted(parent.glob("*.kicad_pro")))
    seen: set[Path] = set()
    for candidate in candidates:
        try:
            candidate = candidate.resolve()
        except (OSError, ValueError):
            continue
        if candidate in seen or not candidate.exists():
            continue
        seen.add(candidate)
        try:
            data = json.loads(candidate.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            return str(candidate)
    return ""


@lru_cache(maxsize=64)
def _project_raw_for_schematic_path(path_text: str) -> dict:
    project_path = _project_file_for_schematic_path(path_text)
    if not project_path:
        return {}
    try:
        data = json.loads(Path(project_path).read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError, json.JSONDecodeError):
        return {}
    if isinstance(data, dict):
        return dict(data)
    return {}


def _project_root_schematic_path_for_schematic_path(path_text: str) -> Path | None:
    project_path = _project_file_for_schematic_path(path_text)
    if not project_path:
        return None
    candidate = Path(project_path).with_suffix(".kicad_sch")
    return candidate if candidate.exists() else None


def _project_drawing_settings_for_schematic_path(path_text: str) -> dict:
    data = _project_raw_for_schematic_path(path_text)
    if data:
        drawing = data.get("schematic", {}).get("drawing", {})
        if isinstance(drawing, dict):
            return dict(drawing)
    return {}


def _schematic_project_drawing_settings(
    schematic: "KiCadSchematic",
    source_path: Optional[str],
) -> dict:
    path = getattr(schematic, "source_path", None)
    if path is None and source_path:
        path = source_path
    if path is None:
        return {}
    return _project_drawing_settings_for_schematic_path(str(path))


def _project_text_variables_for_schematic_path(path_text: str) -> dict[str, str]:
    data = _project_raw_for_schematic_path(path_text)
    variables = data.get("text_variables", {}) if data else {}
    if not isinstance(variables, dict):
        return {}
    return {str(name): str(value) for name, value in variables.items()}


def _schematic_project_text_variables(
    schematic: "KiCadSchematic",
    source_path: Optional[str],
) -> dict[str, str]:
    path = getattr(schematic, "source_path", None)
    if path is None and source_path:
        path = source_path
    if path is None:
        return {}
    return _project_text_variables_for_schematic_path(str(path))


_PROJECT_TEXT_VARIABLE_RE = re.compile(r"\$\{([^}]*)\}")


def _expand_project_text_variables(text: str, project_vars: Optional[dict]) -> str:
    if not project_vars:
        return text
    variables = {str(name): str(value) for name, value in project_vars.items()}

    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        if name == "":
            return ""
        return variables.get(name, match.group(0))

    return _PROJECT_TEXT_VARIABLE_RE.sub(replace, text)


def _schematic_builtin_text_variables(
    schematic: "KiCadSchematic",
    *,
    sheet_index: int,
    sheet_count: int,
    project_vars: Optional[dict],
) -> dict[str, str]:
    variables: dict[str, str] = {
        "#": str(sheet_index),
        "##": str(sheet_count),
        "VARIANT": "",
    }
    tb = getattr(schematic, "title_block", None)
    if tb is not None:
        source = {
            "TITLE": getattr(tb, "title", "") or "",
            "ISSUE_DATE": getattr(tb, "date", "") or "",
            "REVISION": getattr(tb, "rev", "") or "",
            "COMPANY": getattr(tb, "company", "") or "",
        }
        comments = getattr(tb, "comments", None) or {}
        for index, value in comments.items():
            source[f"COMMENT{index}"] = value or ""
        for name, value in source.items():
            variables[name] = _expand_project_text_variables(str(value), project_vars)
    return variables


def _project_sheet_count_for_schematic_path(path_text: str) -> int | None:
    data = _project_raw_for_schematic_path(path_text)
    sheets = data.get("sheets", []) if data else []
    if not isinstance(sheets, list):
        return None
    return len(sheets) or None


def _schematic_project_sheet_count(
    schematic: "KiCadSchematic",
    source_path: Optional[str],
) -> int | None:
    path = getattr(schematic, "source_path", None)
    if path is None and source_path:
        path = source_path
    if path is None:
        return None
    return _project_sheet_count_for_schematic_path(str(path))


def _project_page_layout_file_for_schematic_path(path_text: str) -> str:
    data = _project_raw_for_schematic_path(path_text)
    schematic = data.get("schematic", {}) if data else {}
    if not isinstance(schematic, dict):
        return ""
    value = schematic.get("page_layout_descr_file", "") or ""
    return str(value)


def _schematic_project_page_layout_file(
    schematic: "KiCadSchematic",
    source_path: Optional[str],
) -> str:
    path = getattr(schematic, "source_path", None)
    if path is None and source_path:
        path = source_path
    if path is None:
        return ""
    return _project_page_layout_file_for_schematic_path(str(path))


def _decompress_embedded_payload(data: bytes) -> bytes:
    try:
        import zstandard as _zstandard
    except ImportError as exc:  # pragma: no cover - dependency guard
        raise RuntimeError("zstd support is unavailable; install 'zstandard'") from exc
    return _zstandard.ZstdDecompressor().decompress(data)


def _embedded_file_bytes_from_raw(
    raw: list,
    *,
    name: str | None = None,
    file_type: str | None = None,
) -> list[tuple[str, str, bytes]]:
    embedded = find_element(raw, "embedded_files")
    if embedded is None:
        return []

    out: list[tuple[str, str, bytes]] = []
    for file_elem in find_all_elements(embedded, "file"):
        found_name = ""
        found_type = ""
        data_parts: list[str] = []
        for item in file_elem[1:]:
            if not isinstance(item, list) or not item:
                continue
            if item[0] == "name" and len(item) > 1:
                found_name = unquote_string(item[1])
            elif item[0] == "type" and len(item) > 1:
                found_type = str(item[1])
            elif item[0] == "data":
                data_parts = [str(part) for part in item[1:]]
        if name is not None and found_name != name:
            continue
        if file_type is not None and found_type != file_type:
            continue
        if not data_parts:
            continue

        encoded = "".join(data_parts).replace("\n", "").replace("\r", "").strip("|")
        try:
            compressed = base64.b64decode(encoded)
            out.append((found_name, found_type, _decompress_embedded_payload(compressed)))
        except (binascii.Error, RuntimeError, ValueError):
            continue
    return out


def _embedded_file_text(
    schematic: "KiCadSchematic",
    *,
    name: str,
    file_type: str,
) -> str | None:
    raw = getattr(schematic, "_raw_sexp", None)
    if raw is None:
        return None
    return _embedded_file_text_from_raw(raw, name=name, file_type=file_type)


def _embedded_file_text_from_raw(
    raw: list,
    *,
    name: str,
    file_type: str,
) -> str | None:
    for _found_name, _found_type, payload in _embedded_file_bytes_from_raw(
        raw,
        name=name,
        file_type=file_type,
    ):
        return payload.decode("utf-8-sig")
    return None


@lru_cache(maxsize=64)
def _embedded_file_text_from_schematic_path(
    path_text: str,
    *,
    name: str,
    file_type: str,
) -> str | None:
    try:
        from .kicad_sexpr import parse_sexp

        raw = parse_sexp(Path(path_text).read_text(encoding="utf-8-sig"))
    except (OSError, ValueError, TypeError):
        return None
    return _embedded_file_text_from_raw(raw, name=name, file_type=file_type)


def _resolve_project_layout_file_near_schematic(
    layout_name: str,
    schematic: "KiCadSchematic",
    source_path: Optional[str],
) -> Path | None:
    candidate = Path(layout_name)
    if candidate.is_absolute():
        return candidate if candidate.exists() else None

    base = getattr(schematic, "source_path", None)
    if base is None and source_path:
        base = source_path
    if base is None:
        return None

    raw_name = layout_name.replace("\\", "/")
    start_dir = Path(base).parent
    for parent in (start_dir, *start_dir.parents):
        candidate = parent / raw_name
        if candidate.exists():
            return candidate
    return None


def _project_worksheet_for_schematic(
    schematic: "KiCadSchematic",
    source_path: Optional[str],
):
    from .kicad_drawing_sheet import load_default_drawing_sheet
    from .kicad_worksheet import KiCadWorksheet

    layout = _schematic_project_page_layout_file(schematic, source_path)
    if not layout:
        return load_default_drawing_sheet()

    if layout.startswith("kicad-embed://"):
        embedded_name = layout[len("kicad-embed://") :]
        text = _embedded_file_text(
            schematic,
            name=embedded_name,
            file_type="worksheet",
        )
        if text:
            return KiCadWorksheet.from_text(text)
        root_schematic = None
        path = getattr(schematic, "source_path", None)
        if path is None and source_path:
            path = source_path
        if path is not None:
            root_schematic = _project_root_schematic_path_for_schematic_path(str(path))
        if root_schematic is not None:
            text = _embedded_file_text_from_schematic_path(
                str(root_schematic),
                name=embedded_name,
                file_type="worksheet",
            )
            if text:
                return KiCadWorksheet.from_text(text)
        candidate = _resolve_project_layout_file_near_schematic(
            embedded_name,
            schematic,
            source_path,
        )
        if candidate is not None:
            return KiCadWorksheet.from_file(candidate)
        return load_default_drawing_sheet()

    candidate = _resolve_project_layout_file_near_schematic(
        layout,
        schematic,
        source_path,
    )
    if candidate is not None:
        return KiCadWorksheet.from_file(candidate)
    return load_default_drawing_sheet()


def _drawing_setting_float(settings: dict, key: str, default: float) -> float:
    try:
        return float(settings.get(key, default))
    except (TypeError, ValueError):
        return default


def _drawing_default_line_width_nm(settings: dict) -> int:
    mils = _drawing_setting_float(
        settings,
        "default_line_thickness",
        DEFAULT_WIRE_WIDTH_MM / _MIL_TO_MM,
    )
    return max(mm_to_nm(mils * _MIL_TO_MM), DEFAULT_MIN_PLOT_PEN_WIDTH_NM)


def _resolve_stroke(
    stroke,
    default_width_mm: float,
    default_color: str | None = None,
) -> Tuple[KiCadLineStyle, int, Optional[str]]:
    """Return ``(line_style, width_nm, color_hex)`` from a Stroke."""
    raw_width = getattr(stroke, "width", 0.0) if stroke else 0.0
    if raw_width < 0:
        width_nm = 0
    elif raw_width == 0:
        width_nm = max(mm_to_nm(default_width_mm), DEFAULT_MIN_PLOT_PEN_WIDTH_NM)
    else:
        width_nm = max(mm_to_nm(raw_width), DEFAULT_MIN_PLOT_PEN_WIDTH_NM)
    if stroke and stroke.type:
        line_style = stroke_type_to_line_style(stroke.type)
    else:
        line_style = KiCadLineStyle.DEFAULT
    color = (
        rgba_to_hex(stroke.color) if (stroke and stroke.color) else None
    ) or default_color
    return line_style, width_nm, color


# ---------------------------------------------------------------------------
# Per-element op emitters
# ---------------------------------------------------------------------------


def wire_to_op(wire: "SchWire") -> Optional[KiCadPlotterOp]:
    """Convert a :class:`SchWire` to a ``PlotPoly`` op."""
    if not wire.points:
        return None
    line_style, width_nm, color = _resolve_stroke(
        wire.stroke, DEFAULT_WIRE_WIDTH_MM, LAYER_WIRE
    )
    pts = [(mm_to_nm(x), mm_to_nm(y)) for x, y in wire.points]
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=pts, fill=KiCadFillType.NO_FILL, width_nm=width_nm
        ),
        stroke_color=color,
        line_style=line_style,
    )


def bus_to_op(bus: "SchBus") -> Optional[KiCadPlotterOp]:
    """Convert a :class:`SchBus` to a ``PlotPoly`` op (thicker default)."""
    if not bus.points:
        return None
    line_style, width_nm, color = _resolve_stroke(
        bus.stroke, DEFAULT_BUS_WIDTH_MM, LAYER_BUS
    )
    pts = [(mm_to_nm(x), mm_to_nm(y)) for x, y in bus.points]
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=pts, fill=KiCadFillType.NO_FILL, width_nm=width_nm
        ),
        stroke_color=color,
        line_style=line_style,
    )


def bus_entry_to_op(entry: "SchBusEntry") -> KiCadPlotterOp:
    """Convert a :class:`SchBusEntry` to a 2-point ``PlotPoly`` op.

    A bus entry is a short diagonal stub from ``(at_x, at_y)`` to
    ``(at_x + size_x, at_y + size_y)``.
    """
    line_style, width_nm, color = _resolve_stroke(
        entry.stroke, DEFAULT_WIRE_WIDTH_MM, LAYER_WIRE
    )
    p0 = (mm_to_nm(entry.at_x), mm_to_nm(entry.at_y))
    p1 = (mm_to_nm(entry.at_x + entry.size_x), mm_to_nm(entry.at_y + entry.size_y))
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=[p0, p1], fill=KiCadFillType.NO_FILL, width_nm=width_nm
        ),
        stroke_color=color,
        line_style=line_style,
    )


def junction_to_op(junction: "SchJunction") -> KiCadPlotterOp:
    """Convert a :class:`SchJunction` to a filled ``Circle`` op."""
    diameter_mm = (
        junction.diameter
        if junction.diameter and junction.diameter > 0
        else DEFAULT_JUNCTION_DIAMETER_MM
    )
    color = (rgba_to_hex(junction.color) if junction.color else None) or LAYER_JUNCTION
    return styled_plotter_op(
        KiCadPlotterOp.circle(
            cx=mm_to_nm(junction.at_x),
            cy=mm_to_nm(junction.at_y),
            diameter_nm=mm_to_nm(diameter_mm),
            fill=KiCadFillType.FILLED_SHAPE,
            width_nm=0,
        ),
        stroke_color=color,
        fill_color=color,
    )


def no_connect_to_ops(
    no_connect: "SchNoConnect",
    *,
    default_line_width_nm: int | None = None,
) -> List[KiCadPlotterOp]:
    """Convert a :class:`SchNoConnect` to two crossing ``PlotPoly`` ops.

    eeschema draws a no-connect as a small "X" centred at ``(at_x, at_y)``.
    """
    cx, cy = no_connect.at_x, no_connect.at_y
    h = DEFAULT_NO_CONNECT_HALF_MM
    width_nm = (
        int(default_line_width_nm)
        if default_line_width_nm is not None
        else mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    )
    seg_a = [
        (mm_to_nm(cx - h), mm_to_nm(cy - h)),
        (mm_to_nm(cx + h), mm_to_nm(cy + h)),
    ]
    seg_b = [
        (mm_to_nm(cx - h), mm_to_nm(cy + h)),
        (mm_to_nm(cx + h), mm_to_nm(cy - h)),
    ]
    return [
        styled_plotter_op(
            KiCadPlotterOp.plot_poly(
                points=seg_a, fill=KiCadFillType.NO_FILL, width_nm=width_nm
            ),
            stroke_color=LAYER_NOCONNECT,
        ),
        styled_plotter_op(
            KiCadPlotterOp.plot_poly(
                points=seg_b, fill=KiCadFillType.NO_FILL, width_nm=width_nm
            ),
            stroke_color=LAYER_NOCONNECT,
        ),
    ]


def _label_to_op(
    label,
    default_size_mm: float = DEFAULT_TEXT_SIZE_MM,
    layer_color: str = LAYER_LOCLABEL,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
    default_line_width_nm: int | None = None,
) -> KiCadPlotterOp:
    """Shared text-op emit for labels (local / global / hierarchical).

    No Y-flip — schematic coords are already Y-down. Uses the same
    ``Effects`` lifter as :mod:`kicad_lib_symbol_to_ir` so font/face/
    italic/bold/justify mappings stay consistent.
    """
    text_layer_color = (
        LAYER_BUS
        if _looks_like_bus_label_text(getattr(label, "text", ""))
        else layer_color
    )
    kwargs = _text_kwargs_with_plot_defaults(
        label.effects,
        text_layer_color,
        default_line_width_nm=default_line_width_nm,
    )
    if "size_x_nm" not in kwargs or kwargs.get("size_x_nm", 0) == 0:
        kwargs["size_x_nm"] = mm_to_nm(default_size_mm)
        kwargs["size_y_nm"] = mm_to_nm(default_size_mm)
    if label.__class__.__name__ in ("SchGlobalLabel", "SchHierarchicalLabel"):
        kwargs["v_align"] = KiCadVertAlign.CENTER
    x_nm = mm_to_nm(label.at_x)
    y_nm = mm_to_nm(label.at_y)
    dx_nm, dy_nm = _label_plot_text_offset_nm(
        label,
        kwargs,
        text_offset_ratio=text_offset_ratio,
    )
    return KiCadPlotterOp.text(
        x=x_nm + dx_nm,
        y=y_nm + dy_nm,
        text=_plot_display_text(label.text),
        orient_deg=_label_plot_orient_deg(label),
        **kwargs,
    )


def label_to_op(
    label: "SchLabel",
    *,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
    default_line_width_nm: int | None = None,
) -> KiCadPlotterOp:
    """Convert a :class:`SchLabel` to a ``Text`` op (body only)."""
    return _label_to_op(
        label,
        layer_color=LAYER_LOCLABEL,
        text_offset_ratio=text_offset_ratio,
        default_line_width_nm=default_line_width_nm,
    )


def global_label_to_op(
    label: "SchGlobalLabel",
    *,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
    default_line_width_nm: int | None = None,
) -> KiCadPlotterOp:
    """Convert a :class:`SchGlobalLabel` to a ``Text`` op (body only).

    The arrow-shaped border around the text is emitted separately once
    global-label shape geometry from ``SCH_GLOBAL_LABEL::CreateGraphicShape``
    is available.
    """
    return _label_to_op(
        label,
        layer_color=LAYER_GLOBLABEL,
        text_offset_ratio=text_offset_ratio,
        default_line_width_nm=default_line_width_nm,
    )


def hierarchical_label_to_op(
    label: "SchHierarchicalLabel",
    *,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
    default_line_width_nm: int | None = None,
) -> KiCadPlotterOp:
    """Convert a :class:`SchHierarchicalLabel` to a ``Text`` op (body only).

    The triangular shape decoration is emitted separately by
    :func:`hierarchical_label_decoration_to_op`.
    """
    return _label_to_op(
        label,
        layer_color=LAYER_HIERLABEL,
        text_offset_ratio=text_offset_ratio,
        default_line_width_nm=default_line_width_nm,
    )


# ---------------------------------------------------------------------------
# Label / sheet-pin triangle decorations
# ---------------------------------------------------------------------------
#
# Verbatim port of KiCad's ``TemplateShape[5][4]`` table from
# ``eeschema/sch_label.cpp:67-96``. First-axis index = LABEL_FLAG_SHAPE
# (INPUT / OUTPUT / BIDI / TRI_STATE / UNSPECIFIED). Second-axis index
# = SPIN_STYLE enum (LEFT=0=HN, UP=1, RIGHT=2=HI, BOTTOM=3). Each entry
# is a list of ``(dx, dy)`` reduced-unit corners; the actual coords are
# ``halfSize * (dx, dy) + aPos`` where ``halfSize = TextHeight // 2``.
# Templates already wrap (last point == first), so they are emitted as
# closed ``PlotPoly`` polygons without an extra closing vertex.

_LABEL_TEMPLATE_INPUT = [
    [(0, 0), (-1, -1), (-2, -1), (-2, 1), (-1, 1), (0, 0)],   # HN/LEFT
    [(0, 0), (1, -1), (1, -2), (-1, -2), (-1, -1), (0, 0)],   # UP
    [(0, 0), (1, 1), (2, 1), (2, -1), (1, -1), (0, 0)],       # HI/RIGHT
    [(0, 0), (1, 1), (1, 2), (-1, 2), (-1, 1), (0, 0)],       # BOTTOM
]

_LABEL_TEMPLATE_OUTPUT = [
    [(-2, 0), (-1, 1), (0, 1), (0, -1), (-1, -1), (-2, 0)],   # HN/LEFT
    [(0, -2), (1, -1), (1, 0), (-1, 0), (-1, -1), (0, -2)],   # UP
    [(2, 0), (1, -1), (0, -1), (0, 1), (1, 1), (2, 0)],       # HI/RIGHT
    [(0, 2), (1, 1), (1, 0), (-1, 0), (-1, 1), (0, 2)],       # BOTTOM
]

_LABEL_TEMPLATE_BIDI = [
    [(0, 0), (-1, -1), (-2, 0), (-1, 1), (0, 0)],             # HN/LEFT
    [(0, 0), (-1, -1), (0, -2), (1, -1), (0, 0)],             # UP
    [(0, 0), (1, -1), (2, 0), (1, 1), (0, 0)],                # HI/RIGHT
    [(0, 0), (-1, 1), (0, 2), (1, 1), (0, 0)],                # BOTTOM
]

# L_TRISTATE shares the BIDI geometry verbatim (sch_label.cpp:87-90).
_LABEL_TEMPLATE_TRISTATE = _LABEL_TEMPLATE_BIDI

_LABEL_TEMPLATE_UNSPECIFIED = [
    [(0, -1), (-2, -1), (-2, 1), (0, 1), (0, -1)],            # HN/LEFT
    [(1, 0), (1, -2), (-1, -2), (-1, 0), (1, 0)],             # UP
    [(0, -1), (2, -1), (2, 1), (0, 1), (0, -1)],              # HI/RIGHT
    [(1, 0), (1, 2), (-1, 2), (-1, 0), (1, 0)],               # BOTTOM
]

_LABEL_DECORATION_TEMPLATES = {
    "input": _LABEL_TEMPLATE_INPUT,
    "output": _LABEL_TEMPLATE_OUTPUT,
    "bidirectional": _LABEL_TEMPLATE_BIDI,
    "tri_state": _LABEL_TEMPLATE_TRISTATE,
    "passive": _LABEL_TEMPLATE_UNSPECIFIED,
}


def _at_angle_to_spin_idx(at_angle: float) -> int:
    """Map a label's ``at_angle`` (deg) to KiCad's SPIN_STYLE enum index.

    SPIN_STYLE values: LEFT=0 (HN), UP=1, RIGHT=2 (HI), BOTTOM=3
    (``sch_label.h:46-49``). Eeschema rotates labels CCW by 90° via
    ``LEFT→BOTTOM→RIGHT→UP→LEFT`` (``sch_label.cpp:119-121``), giving
    the angle→spin mapping below. Unknown angles fall back to RIGHT
    (text reads left→right, anchor on left), the eeschema default.
    """
    a = int(round(float(at_angle))) % 360
    return {0: 2, 90: 1, 180: 0, 270: 3}.get(a, 2)


def _sheet_pin_at_angle_to_spin_idx(at_angle: float) -> int:
    """Map ``SCH_SHEET_PIN`` side angles to KiCad's SPIN_STYLE enum index."""
    a = int(round(float(at_angle))) % 360
    return {0: 0, 90: 3, 180: 2, 270: 1}.get(a, 2)


def _looks_like_bus_label_text(text: object) -> bool:
    value = str(text or "").replace("{slash}", "")
    for idx, ch in enumerate(value):
        if ch == "{" and (idx == 0 or value[idx - 1] != "~") and "}" in value[idx + 1:]:
            return True
    return False


def _plot_display_text(text: object) -> str:
    return str(text or "").replace("{slash}", "/")


def _plot_metric_text(text: object) -> str:
    value = _plot_display_text(text)
    out: list[str] = []
    i = 0
    while i < len(value):
        ch = value[i]
        if ch in "~^_" and i + 1 < len(value) and value[i + 1] == "{":
            i += 2
            depth = 1
            while i < len(value) and depth:
                inner = value[i]
                if inner == "{":
                    depth += 1
                    out.append(inner)
                elif inner == "}":
                    depth -= 1
                    if depth:
                        out.append(inner)
                else:
                    out.append(inner)
                i += 1
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def _wx_string_split_plot_text(text: str) -> str:
    """Mirror wxStringSplit(..., '\\n') for plotted EDA_TEXT content."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    return "\n".join(lines)


def _ki_round(value: float) -> int:
    """Mirror KiROUND for the positive schematic lengths used here."""
    if value >= 0:
        return int(math.floor(value + 0.5))
    return int(math.ceil(value - 0.5))


def _freetype_harfbuzz_scale(units_per_em: int, ft_scale: int) -> int:
    """Mirror FT_MulFix for the scale hb-ft derives from a sized face."""
    value = int(units_per_em) * int(ft_scale)
    if value >= 0:
        return (value + 0x8000) >> 16
    return -(((-value) + 0x8000) >> 16)


def _nm_to_schematic_iu(value_nm: int | float) -> int:
    """Convert nm to eeschema IU (100 nm) using KiCad rounding."""
    return _ki_round(float(value_nm) / 100.0)


def _schematic_iu_to_nm(value_iu: int) -> int:
    return int(value_iu) * 100


def _schematic_half_nm(value_nm: int) -> int:
    """Return KiCad integer-IU half of a schematic length."""
    return _schematic_iu_to_nm(_nm_to_schematic_iu(value_nm) // 2)


def _parse_color_hex_rgba(color: object) -> tuple[float, float, float, float] | None:
    text = str(color).strip()
    if not text.startswith("#"):
        return None
    body = text[1:]
    if len(body) == 3:
        body = "".join(ch * 2 for ch in body) + "FF"
    elif len(body) == 4:
        body = "".join(ch * 2 for ch in body)
    elif len(body) == 6:
        body += "FF"
    elif len(body) != 8:
        return None
    try:
        r = int(body[0:2], 16) / 255.0
        g = int(body[2:4], 16) / 255.0
        b = int(body[4:6], 16) / 255.0
        a = int(body[6:8], 16) / 255.0
    except ValueError:
        return None
    return r, g, b, a


def _rgba_to_plot_hex(r: float, g: float, b: float, a: float) -> str:
    vals = []
    for value in (r, g, b, a):
        vals.append(max(0, min(255, _ki_round(value * 255.0))))
    return f"#{vals[0]:02X}{vals[1]:02X}{vals[2]:02X}{vals[3]:02X}"


def _dnp_dimmed_color(color: object) -> object:
    rgba = _parse_color_hex_rgba(color)
    bg = _parse_color_hex_rgba(LAYER_SCHEMATIC_BACKGROUND)
    if rgba is None or bg is None:
        return color
    r, g, b, a = rgba
    bg_r, bg_g, bg_b, _bg_a = bg
    lightness = (max(r, g, b) + min(r, g, b)) / 2.0
    return _rgba_to_plot_hex(
        (bg_r + lightness) / 2.0,
        (bg_g + lightness) / 2.0,
        (bg_b + lightness) / 2.0,
        a,
    )


def _label_horiz_justify(label) -> str | None:
    effects = getattr(label, "effects", None)
    justify = getattr(effects, "justify", None) if effects is not None else None
    if not justify:
        return None
    for tok in justify:
        if tok == "right":
            return "right"
        if tok == "left":
            return "left"
        if tok == "center":
            return "center"
    return None


def _label_vert_justify(label) -> str | None:
    effects = getattr(label, "effects", None)
    justify = getattr(effects, "justify", None) if effects is not None else None
    if not justify:
        return None
    for tok in justify:
        if tok == "top":
            return "top"
        if tok == "bottom":
            return "bottom"
        if tok == "center":
            return "center"
    return None


def _label_spin_idx(label) -> int:
    """Return KiCad SCH_LABEL_BASE::GetSpinStyle() as a 0..3 index."""
    h_justify = _label_horiz_justify(label)
    if h_justify is None:
        return _at_angle_to_spin_idx(getattr(label, "at_angle", 0.0))

    h_right = h_justify == "right"
    angle = int(round(float(getattr(label, "at_angle", 0.0)))) % 180
    if angle == 90:
        return 3 if h_right else 1
    return 0 if h_right else 2


def _label_plot_orient_deg(label) -> float:
    """Return the plotted text angle for KiCad label-family items."""
    return 90.0 if _label_spin_idx(label) in (1, 3) else 0.0


def _label_text_pen_width_nm(label, kwargs: dict) -> int:
    effects = getattr(label, "effects", None)
    font = getattr(effects, "font", None) if effects is not None else None
    explicit_mm = getattr(font, "thickness", None) if font is not None else None
    if explicit_mm is not None and explicit_mm > 0:
        return mm_to_nm(explicit_mm)

    size_x_nm = int(kwargs.get("size_x_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    size_y_nm = int(kwargs.get("size_y_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    if kwargs.get("bold"):
        pen_width_nm = _ki_round(size_x_nm / 5.0)
    else:
        pen_width_nm = _ki_round(size_x_nm / 8.0)
    return min(pen_width_nm, _ki_round(min(abs(size_x_nm), abs(size_y_nm)) * 0.25))


def _label_plot_text_offset_nm(
    label,
    kwargs: dict,
    *,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
) -> Tuple[int, int]:
    """Mirror the label-family GetSchematicTextOffset() implementations."""
    label_kind = label.__class__.__name__
    text_height_nm = int(kwargs.get("size_y_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    spin_idx = _label_spin_idx(label)

    if label_kind == "SchGlobalLabel":
        horiz_nm = _ki_round(DEFAULT_LABEL_SIZE_RATIO * text_height_nm)
        shape = getattr(getattr(label, "shape", None), "value", getattr(label, "shape", None))
        if shape in ("input", "bidirectional", "tri_state"):
            horiz_nm += (text_height_nm * 3) // 4
        vert_nm = int(text_height_nm * 0.0715)

        if spin_idx == 0:      # LEFT
            return -horiz_nm, vert_nm
        if spin_idx == 1:      # UP
            return vert_nm, -horiz_nm
        if spin_idx == 3:      # BOTTOM
            return vert_nm, horiz_nm
        return horiz_nm, vert_nm

    text_offset_nm = _ki_round(text_offset_ratio * text_height_nm)

    if label_kind == "SchHierarchicalLabel":
        text_width_nm = int(kwargs.get("size_x_nm") or text_height_nm)
        dist_nm = text_offset_nm + text_width_nm
        if spin_idx == 0:      # LEFT
            return -dist_nm, 0
        if spin_idx == 1:      # UP
            return 0, -dist_nm
        if spin_idx == 3:      # BOTTOM
            return 0, dist_nm
        return dist_nm, 0

    dist_nm = text_offset_nm + _label_text_pen_width_nm(label, kwargs)
    if spin_idx in (1, 3):     # UP/BOTTOM vertical text
        return -dist_nm, 0
    return 0, -dist_nm


def _shape_decoration_key(shape) -> Optional[str]:
    """Return the ``_LABEL_DECORATION_TEMPLATES`` key for ``shape`` (or ``None``).

    Accepts a raw string or a :class:`LabelShape` enum member. Returns
    ``None`` for label shapes without a triangle/box decoration (e.g.
    DOT/ROUND/DIAMOND/RECTANGLE used by SCH_DIRECTIVE_LABEL).
    """
    if shape is None:
        return None
    val = getattr(shape, "value", shape)
    if not isinstance(val, str):
        return None
    return val if val in _LABEL_DECORATION_TEMPLATES else None


def _emit_template_decoration(
    *,
    shape_key: str,
    spin_idx: int,
    half_size_nm: int,
    anchor_x_nm: int,
    anchor_y_nm: int,
    pen_width_nm: int,
    stroke_color: str,
) -> KiCadPlotterOp:
    """Build a closed ``PlotPoly(NO_FILL)`` op for a triangle/arrow decoration."""
    template = _LABEL_DECORATION_TEMPLATES[shape_key][spin_idx]
    points = [
        (half_size_nm * dx + anchor_x_nm, half_size_nm * dy + anchor_y_nm)
        for dx, dy in template
    ]
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=points, fill=KiCadFillType.NO_FILL, width_nm=pen_width_nm
        ),
        stroke_color=stroke_color,
    )


def _label_text_height_nm(
    label, default_size_mm: float = DEFAULT_TEXT_SIZE_MM
) -> int:
    """Return the effective text height in nm for a label-like object.

    Mirrors ``GR_TEXT_INFOBASE::GetTextHeight`` (= ``size.y``); falls
    back to ``default_size_mm`` when the parsed Effects block is
    missing or carries 0.
    """
    eff = getattr(label, "effects", None)
    if eff is not None and eff.font is not None and eff.font.size_y > 0:
        return mm_to_nm(eff.font.size_y)
    return mm_to_nm(default_size_mm)


def hierarchical_label_decoration_to_op(
    label: "SchHierarchicalLabel",
) -> Optional[KiCadPlotterOp]:
    """Triangle/arrow decoration polygon for a :class:`SchHierarchicalLabel`.

    Mirrors ``SCH_HIERLABEL::CreateGraphicShape`` (``sch_label.cpp:2425``).
    Returns ``None`` for label shapes without a triangle decoration
    (e.g. directive-label DOT/ROUND/DIAMOND/RECTANGLE shapes).
    """
    shape_key = _shape_decoration_key(getattr(label, "shape", None))
    if shape_key is None:
        return None
    half_size_nm = _label_text_height_nm(label) // 2
    spin_idx = _at_angle_to_spin_idx(label.at_angle)
    return _emit_template_decoration(
        shape_key=shape_key,
        spin_idx=spin_idx,
        half_size_nm=half_size_nm,
        anchor_x_nm=mm_to_nm(label.at_x),
        anchor_y_nm=mm_to_nm(label.at_y),
        pen_width_nm=mm_to_nm(DEFAULT_WIRE_WIDTH_MM),
        stroke_color=LAYER_HIERLABEL,
    )


# Sheet pins reuse the hier-label TemplateShape table but with INPUT
# and OUTPUT shapes swapped, per ``SCH_SHEET_PIN::CreateGraphicShape``
# (``sch_sheet_pin.cpp:355-374``).
_SHEET_PIN_SHAPE_SWAP = {"input": "output", "output": "input"}


def _font_has_explicit_thickness(effects) -> bool:
    font = getattr(effects, "font", None) if effects is not None else None
    return getattr(font, "thickness", None) is not None


def _text_kwargs_with_plot_defaults(
    effects,
    layer_color: str,
    *,
    default_line_width_nm: int | None = None,
) -> dict:
    kwargs = apply_default_text_style(_effects_to_text_kwargs(effects), layer_color)
    if (
        default_line_width_nm is not None
        and not _font_has_explicit_thickness(effects)
        and not bool(kwargs.get("bold"))
    ):
        pen_width_nm = max(
            int(default_line_width_nm),
            DEFAULT_MIN_PLOT_PEN_WIDTH_NM,
        )
        size_x = int(kwargs.get("size_x_nm") or 0)
        size_y = int(kwargs.get("size_y_nm") or 0)
        text_size = min(abs(size_x), abs(size_y))
        if text_size > 0:
            pen_width_nm = min(pen_width_nm, int((text_size * 0.25) + 0.5))
        kwargs["pen_width_nm"] = pen_width_nm
    return kwargs


def _sheet_pin_text_kwargs(
    pin,
    *,
    default_line_width_nm: int | None = None,
) -> dict:
    layer_color = (
        LAYER_BUS
        if _looks_like_bus_label_text(getattr(pin, "name", ""))
        else LAYER_SHEETLABEL
    )
    return _text_kwargs_with_plot_defaults(
        pin.effects,
        layer_color,
        default_line_width_nm=default_line_width_nm,
    )


def sheet_pin_decoration_to_op(
    pin,
    *,
    default_line_width_nm: int | None = None,
) -> Optional[KiCadPlotterOp]:
    """Triangle/arrow decoration polygon for a :class:`SchSheetPin`.

    Reuses ``SCH_HIERLABEL::CreateGraphicShape`` with INPUT↔OUTPUT
    swapped, per ``SCH_SHEET_PIN::CreateGraphicShape``
    (``sch_sheet_pin.cpp:355-374``).
    """
    raw_shape = getattr(pin, "shape", None)
    shape_key = _shape_decoration_key(raw_shape)
    if shape_key is None:
        return None
    shape_key = _SHEET_PIN_SHAPE_SWAP.get(shape_key, shape_key)
    kwargs = _sheet_pin_text_kwargs(
        pin,
        default_line_width_nm=default_line_width_nm,
    )
    half_size_nm = _label_text_height_nm(pin) // 2
    spin_idx = _sheet_pin_at_angle_to_spin_idx(pin.at_angle)
    return _emit_template_decoration(
        shape_key=shape_key,
        spin_idx=spin_idx,
        half_size_nm=half_size_nm,
        anchor_x_nm=mm_to_nm(pin.at_x),
        anchor_y_nm=mm_to_nm(pin.at_y),
        pen_width_nm=int(kwargs["pen_width_nm"]),
        stroke_color=LAYER_SHEETLABEL,
    )


# ---------------------------------------------------------------------------
# Global-label arrow box
# ---------------------------------------------------------------------------


def _label_text_size_x_nm(label) -> int:
    """Return the effective text width-axis size in nm for a label-like object.

    Mirrors ``GR_TEXT_INFOBASE::GetTextWidth`` (= ``size.x``); falls
    back to the height (``size.y``) when ``size.x`` is missing/zero,
    and to ``DEFAULT_TEXT_SIZE_MM`` when ``effects`` is absent.
    """
    eff = getattr(label, "effects", None)
    if eff is not None and eff.font is not None:
        if eff.font.size_x > 0:
            return mm_to_nm(eff.font.size_x)
        if eff.font.size_y > 0:
            return mm_to_nm(eff.font.size_y)
    return mm_to_nm(DEFAULT_TEXT_SIZE_MM)


def _stroke_font_text_width_nm(text: str, size_x_nm: int) -> int:
    """Estimate text width in nm using KiCad's newstroke-font glyph metrics.

    Mirrors KiCad's ``KIFONT::FONT::StringBoundaryLimits`` for the
    stroke font: glyph widths are stored in normalised stroke-font
    units; the rendered width in real-world units is
    ``sum(glyph.width) * size.x``. Spaces use the per-font space-width
    constant. Bold/italic/font-face overrides are ignored in v1 — this
    is the same heuristic the existing
    :class:`KiCadStrokeFontRenderer` uses for h-align offset (see
    ``kicad_stroke_font.py:render_text_polylines:407``).
    """
    if not text or size_x_nm <= 0:
        return 0
    from .kicad_stroke_font import get_renderer
    renderer = get_renderer()
    width_units = renderer._calculate_text_width(text)
    return int(round(width_units * size_x_nm))


def _font_lookup_keys(value: object) -> tuple[str, ...]:
    key = re.sub(r"[^0-9a-z]+", " ", str(value or "").casefold()).strip()
    if not key:
        return ()
    compact = key.replace(" ", "")
    return (key,) if compact == key else (key, compact)


def _decode_font_name(value: object) -> str:
    if isinstance(value, bytes):
        try:
            return value.decode("utf-8", errors="ignore")
        except Exception:
            return ""
    return str(value or "")


def _font_style_flags(family: str, style: str) -> tuple[bool, bool]:
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


def _fontconfig_missing_font_substitute(
    font_face: str,
    *,
    bold: bool = False,
    italic: bool = False,
) -> Optional[tuple[str, bool, bool]]:
    for key in _font_lookup_keys(font_face):
        style_substitute = _FONTCONFIG_MISSING_FONT_STYLE_SUBSTITUTES.get(
            (key, bool(bold), bool(italic))
        )
        if style_substitute is not None:
            return style_substitute
        substitute = _FONTCONFIG_MISSING_FONT_SUBSTITUTES.get(key)
        if substitute is not None:
            return substitute
    return None


def _safe_embedded_font_filename(name: str, digest: str) -> str:
    filename = Path(name or "embedded_font.ttf").name
    filename = re.sub(r"[^0-9A-Za-z_.-]+", "_", filename).strip("._") or "embedded_font.ttf"
    return f"{digest}_{filename}"


def _cache_clear_outline_fonts() -> None:
    _outline_font_path.cache_clear()
    _outline_font_text_width_nm.cache_clear()
    _outline_font_line_height_nm.cache_clear()


def _register_embedded_font_payload(name: str, data: bytes) -> bool:
    if not data:
        return False

    digest = hashlib.sha1(data).hexdigest()
    font_dir = Path(tempfile.gettempdir()) / "kicad_monkey_embedded_fonts"
    try:
        font_dir.mkdir(parents=True, exist_ok=True)
        font_path = font_dir / _safe_embedded_font_filename(name, digest)
        if not font_path.exists():
            font_path.write_bytes(data)
    except OSError:
        return False

    try:
        import freetype

        face = freetype.Face(str(font_path))
    except Exception:
        return False

    family = _decode_font_name(getattr(face, "family_name", ""))
    style = _decode_font_name(getattr(face, "style_name", ""))
    if not family:
        family = Path(name).stem
    bold, italic = _font_style_flags(family, style)
    registered = False

    keys: set[str] = set(_font_lookup_keys(family))
    stem = Path(name).stem
    stem = re.sub(r"[-_ ]+(regular|medium|bold|italic|semibold|demibold|black)$", "", stem, flags=re.I)
    keys.update(_font_lookup_keys(stem))

    for key in keys:
        style_key = (key, bold, italic)
        if _EMBEDDED_OUTLINE_FONT_PATHS.get(style_key) != str(font_path):
            _EMBEDDED_OUTLINE_FONT_PATHS[style_key] = str(font_path)
            registered = True
        if not bold and not italic and _EMBEDDED_OUTLINE_FONT_PATHS.get((key, False, False)) != str(font_path):
            _EMBEDDED_OUTLINE_FONT_PATHS[(key, False, False)] = str(font_path)
            registered = True
    return registered


def _register_embedded_fonts_from_raw(raw: list | None, source_key: str) -> None:
    if raw is None or source_key in _REGISTERED_EMBEDDED_FONT_SOURCES:
        return
    _REGISTERED_EMBEDDED_FONT_SOURCES.add(source_key)
    changed = False
    for name, _file_type, payload in _embedded_file_bytes_from_raw(raw, file_type="font"):
        changed = _register_embedded_font_payload(name, payload) or changed
    if changed:
        _cache_clear_outline_fonts()


@lru_cache(maxsize=32)
def _register_embedded_fonts_from_schematic_path(path_text: str) -> None:
    try:
        from .kicad_sexpr import parse_sexp

        raw = parse_sexp(Path(path_text).read_text(encoding="utf-8-sig"))
    except (OSError, ValueError, TypeError):
        return
    _register_embedded_fonts_from_raw(raw, str(Path(path_text).resolve()))


def _register_embedded_fonts_for_schematic(
    schematic: "KiCadSchematic",
    source_path: Optional[str],
) -> None:
    raw = getattr(schematic, "_raw_sexp", None)
    path = getattr(schematic, "source_path", None)
    if path is None and source_path:
        path = source_path
    source_key = str(path) if path is not None else f"schematic:{id(schematic)}"
    _register_embedded_fonts_from_raw(raw, source_key)

    if path is None:
        return
    root_schematic = _project_root_schematic_path_for_schematic_path(str(path))
    if root_schematic is not None:
        _register_embedded_fonts_from_schematic_path(str(root_schematic))


@lru_cache(maxsize=4)
def _arial_metric_font(bold: bool):
    try:
        ImageFont = cast(Any, importlib.import_module("PIL.ImageFont"))
    except Exception:
        return None

    font_path = _arial_outline_font_path(bool(bold))
    if font_path is None:
        return None
    try:
        return ImageFont.truetype(str(font_path), 1000)
    except Exception:
        return None


@lru_cache(maxsize=4)
def _arial_outline_font_path(bold: bool) -> Optional[str]:
    font_name = "arialbd.ttf" if bold else "arial.ttf"
    font_path = Path("C:/Windows/Fonts") / font_name
    if not font_path.exists():
        return None
    return str(font_path)


@lru_cache(maxsize=1)
def _windows_registry_font_paths() -> tuple[Path, ...]:
    if os.name != "nt":
        return ()
    try:
        import winreg
    except Exception:
        return ()

    paths: list[Path] = []
    roots = (
        (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"),
        (winreg.HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"),
    )

    for root, subkey in roots:
        try:
            with winreg.OpenKey(root, subkey) as key:
                count = winreg.QueryInfoKey(key)[1]
                for index in range(count):
                    try:
                        _name, value, _kind = winreg.EnumValue(key, index)
                    except OSError:
                        continue
                    if not isinstance(value, str) or not value:
                        continue
                    font_path = Path(os.path.expandvars(value))
                    if not font_path.is_absolute():
                        font_path = Path("C:/Windows/Fonts") / font_path
                    paths.append(font_path)
        except OSError:
            continue

    seen: set[str] = set()
    unique: list[Path] = []
    for path in paths:
        key = str(path).casefold()
        if key in seen:
            continue
        seen.add(key)
        unique.append(path)
    return tuple(unique)


@lru_cache(maxsize=1)
def _system_outline_font_files() -> tuple[Path, ...]:
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
    font_paths: list[Path] = []
    for directory in search_dirs:
        if not directory.exists():
            continue
        try:
            children = directory.rglob("*")
        except OSError:
            continue
        for font_path in children:
            if font_path.suffix.casefold() in {".ttf", ".otf", ".ttc"}:
                font_paths.append(font_path)
    font_paths.extend(_windows_registry_font_paths())

    seen: set[str] = set()
    unique: list[Path] = []
    for path in font_paths:
        key = str(path).casefold()
        if key in seen or not path.exists():
            continue
        seen.add(key)
        unique.append(path)
    return tuple(unique)


@lru_cache(maxsize=1)
def _system_outline_font_paths() -> dict[tuple[str, bool, bool], list[str]]:
    try:
        import freetype
    except Exception:
        return {}

    out: dict[tuple[str, bool, bool], list[str]] = {}
    for font_path in _system_outline_font_files():
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
            out.setdefault((key, False, False), []).append(str(font_path))
    return out


@lru_cache(maxsize=128)
def _fontconfig_outline_font_path(
    font_face: str,
    *,
    bold: bool = False,
    italic: bool = False,
) -> Optional[str]:
    query = (font_face or "").strip() or "sans-serif"
    styles: list[str] = []
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


def _font_style_lookup_order(
    key: str,
    *,
    bold: bool = False,
    italic: bool = False,
) -> tuple[tuple[str, bool, bool], ...]:
    """Return the style fallback order KiCad uses for outline font lookup."""
    order = [(key, bool(bold), bool(italic))]
    if bold and italic:
        order.extend([(key, True, False), (key, False, True)])
    order.append((key, False, False))

    seen: set[tuple[str, bool, bool]] = set()
    out: list[tuple[str, bool, bool]] = []
    for style_key in order:
        if style_key in seen:
            continue
        seen.add(style_key)
        out.append(style_key)
    return tuple(out)


@lru_cache(maxsize=64)
def _outline_font_path(
    font_face: str = "",
    *,
    bold: bool = False,
    italic: bool = False,
    allow_substitute: bool = True,
) -> Optional[str]:
    face = re.sub(r"\s+", " ", (font_face or "").strip()).casefold()
    keys = _font_lookup_keys(face)
    embedded_candidates: list[str] = []
    for key in keys:
        embedded_candidates.extend(
            path
            for style_key in _font_style_lookup_order(key, bold=bold, italic=italic)
            if (path := _EMBEDDED_OUTLINE_FONT_PATHS.get(style_key))
        )
    for candidate in embedded_candidates:
        if Path(candidate).exists():
            return candidate

    if not face or face == "arial":
        arial_path = _arial_outline_font_path(bool(bold))
        if arial_path is not None:
            return arial_path

    windows = Path("C:/Windows/Fonts")
    user_fonts = Path.home() / "AppData/Local/Microsoft/Windows/Fonts"
    candidates: dict[tuple[str, bool, bool], list[Path]] = {
        ("arial", False, False): [windows / "arial.ttf"],
        ("arial", True, False): [windows / "arialbd.ttf"],
        ("arial", False, True): [windows / "ariali.ttf"],
        ("arial", True, True): [windows / "arialbi.ttf"],
        ("consolas", False, False): [windows / "consola.ttf"],
        ("consolas", True, False): [windows / "consolab.ttf"],
        ("consolas", False, True): [windows / "consolai.ttf"],
        ("consolas", True, True): [windows / "consolaz.ttf"],
        ("times new roman", False, False): [windows / "times.ttf"],
        ("times new roman", True, False): [windows / "timesbd.ttf"],
        ("times new roman", False, True): [windows / "timesi.ttf"],
        ("times new roman", True, True): [windows / "timesbi.ttf"],
        ("montserrat", False, False): [windows / "Montserrat-Regular.ttf"],
        ("montserrat", True, False): [windows / "Montserrat-Bold.ttf"],
        ("montserrat", False, True): [windows / "Montserrat-Italic.ttf"],
        ("montserrat", True, True): [windows / "Montserrat-BoldItalic.ttf"],
        ("berkeley mono", False, False): [user_fonts / "BerkeleyMono-Regular.ttf"],
        ("berkeley mono", True, False): [user_fonts / "BerkeleyMono-Bold.ttf"],
    }
    for style_key in _font_style_lookup_order(face, bold=bold, italic=italic):
        for candidate in candidates.get(style_key, ()):
            if candidate.exists():
                return str(candidate)

    system_fonts = _system_outline_font_paths()
    for key in keys:
        for style_key in _font_style_lookup_order(key, bold=bold, italic=italic):
            for candidate in system_fonts.get(style_key, ()):
                if Path(candidate).exists():
                    return candidate

    if allow_substitute:
        substitute = _fontconfig_missing_font_substitute(
            face,
            bold=bool(bold),
            italic=bool(italic),
        )
        if substitute is not None:
            sub_face, sub_bold, sub_italic = substitute
            candidate = _outline_font_path(
                sub_face,
                bold=sub_bold,
                italic=sub_italic,
                allow_substitute=False,
            )
            if candidate is not None:
                return candidate

        fontconfig_path = _fontconfig_outline_font_path(
            font_face or "Arial",
            bold=bool(bold),
            italic=bool(italic),
        )
        if fontconfig_path is not None:
            return fontconfig_path

    return _arial_outline_font_path(bool(bold))


@lru_cache(maxsize=64)
def _outline_font_data(font_path: str) -> Optional[bytes]:
    try:
        return Path(font_path).read_bytes()
    except OSError:
        return None


@lru_cache(maxsize=1024)
def _outline_font_text_width_nm(
    text: str,
    size_x_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
    supersub: bool = False,
) -> Optional[int]:
    """Return KiCad outline-font text width when FreeType/HarfBuzz are available."""
    font_path = _outline_font_path(font_face, bold=bool(bold), italic=bool(italic))
    font_data = _outline_font_data(font_path) if font_path is not None else None
    if font_path is None or font_data is None:
        return None

    if "\t" in text:
        size_x_iu = _nm_to_schematic_iu(size_x_nm)
        tab_width_iu = _ki_round(size_x_iu * 4.0 * 0.6)
        if tab_width_iu <= 0:
            return 0
        cursor_iu = 0
        run: list[str] = []

        def flush_run() -> None:
            nonlocal cursor_iu, run
            if not run:
                return
            run_width_nm = _outline_font_text_width_nm(
                "".join(run),
                size_x_nm,
                bold=bold,
                italic=italic,
                font_face=font_face,
                supersub=supersub,
            )
            if run_width_nm is not None:
                cursor_iu += _nm_to_schematic_iu(run_width_nm)
            run = []

        for char in text:
            if char == "\t":
                flush_run()
                current_intrusion = cursor_iu % tab_width_iu
                cursor_iu += tab_width_iu - current_intrusion
            else:
                run.append(char)
        flush_run()
        return _schematic_iu_to_nm(cursor_iu)

    try:
        import freetype
        import uharfbuzz as hb
    except Exception:
        return None
    hb = cast(Any, hb)

    try:
        face_size = (
            _ki_round(_OUTLINE_FONT_FACE_SIZE * _OUTLINE_FONT_SUBSCRIPT_SUPERSCRIPT_SIZE)
            if supersub
            else _OUTLINE_FONT_FACE_SIZE
        )
        face = freetype.Face(font_path)
        face.set_char_size(0, face_size, _GLYPH_RESOLUTION, 0)
        hb_face = hb.Face(font_data)
        hb_font = hb.Font(hb_face)
        scale_x = _freetype_harfbuzz_scale(face.units_per_EM, face.size.x_scale)
        scale_y = _freetype_harfbuzz_scale(face.units_per_EM, face.size.y_scale)
        hb_font.scale = (
            scale_x or (face.size.x_ppem << 6),
            scale_y or (face.size.y_ppem << 6),
        )
        try:
            hb_font.ppem = (face.size.x_ppem, face.size.y_ppem)
        except Exception:
            pass
        buf = hb.Buffer()
        buf.add_str(text)
        buf.guess_segment_properties()
        hb.shape(hb_font, buf)
        glyph_cursor = sum(
            int(pos.x_advance * _GLYPH_SIZE_SCALER)
            for pos in buf.glyph_positions
        )
        size_x_iu = _nm_to_schematic_iu(size_x_nm)
        width_iu = int(
            glyph_cursor
            * (float(size_x_iu) / _OUTLINE_FONT_FACE_SIZE)
            * _OUTLINE_FONT_SIZE_COMPENSATION
        )
    except Exception:
        return None
    return _schematic_iu_to_nm(width_iu)


def _schematic_plain_text_width_nm(
    text: str,
    size_x_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
    supersub: bool = False,
) -> int:
    if not text or size_x_nm <= 0:
        return 0
    outline_width = _outline_font_text_width_nm(
        text,
        size_x_nm,
        bold=bold,
        italic=italic,
        font_face=font_face,
        supersub=supersub,
    )
    if outline_width is not None:
        return outline_width
    font = _arial_metric_font(bool(bold))
    if font is not None:
        try:
            bbox = font.getbbox(text)
            width_px = max(0, int(bbox[2]) - int(bbox[0]))
            # KiCad's font-size convention maps Arial's 1000px bbox width to
            # roughly 1.4x the schematic text size.
            scale = _OUTLINE_FONT_SUBSCRIPT_SUPERSCRIPT_SIZE if supersub else 1.0
            return _ki_round(width_px * float(size_x_nm) * scale * 1.4 / 1000.0)
        except Exception:
            pass
    scale = _OUTLINE_FONT_SUBSCRIPT_SUPERSCRIPT_SIZE if supersub else 1.0
    return _ki_round(_stroke_font_text_width_nm(text, size_x_nm) * scale * 0.88)


def _markup_outline_text_width_nm(
    text: str,
    size_x_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
) -> Optional[int]:
    if not any(marker in text for marker in ("_{", "^{", "~{")):
        return None

    def visible_markup_text(index: int, *, stop_at_brace: bool) -> tuple[str, int]:
        chars: list[str] = []
        while index < len(text):
            ch = text[index]
            if stop_at_brace and ch == "}":
                break
            if ch in "_^~" and index + 1 < len(text) and text[index + 1] == "{":
                child, child_end = visible_markup_text(index + 2, stop_at_brace=True)
                chars.append(child)
                index = child_end + 1 if child_end < len(text) and text[child_end] == "}" else child_end
                continue
            chars.append(ch)
            index += 1
        return "".join(chars), index

    if "_{" not in text and "^{" not in text:
        visible_text, _end = visible_markup_text(0, stop_at_brace=False)
        return _schematic_plain_text_width_nm(
            visible_text,
            size_x_nm,
            bold=bold,
            italic=italic,
            font_face=font_face,
        )

    def measure_run(run: list[str], *, supersub: bool) -> int:
        if not run:
            return 0
        return _schematic_plain_text_width_nm(
            "".join(run),
            size_x_nm,
            bold=bold,
            italic=italic,
            font_face=font_face,
            supersub=supersub,
        )

    def parse(index: int, *, supersub: bool, stop_at_brace: bool) -> tuple[int, int, bool]:
        width_nm = 0
        run: list[str] = []
        saw_markup = False
        while index < len(text):
            ch = text[index]
            if stop_at_brace and ch == "}":
                break
            if ch in "_^~" and index + 1 < len(text) and text[index + 1] == "{":
                width_nm += measure_run(run, supersub=supersub)
                run = []
                child_width, child_end, child_markup = parse(
                    index + 2,
                    supersub=supersub or ch in "_^",
                    stop_at_brace=True,
                )
                width_nm += child_width
                saw_markup = True
                index = child_end + 1 if child_end < len(text) and text[child_end] == "}" else child_end
                continue
            run.append(ch)
            index += 1
        width_nm += measure_run(run, supersub=supersub)
        return width_nm, index, saw_markup

    width_nm, _end, saw_markup = parse(0, supersub=False, stop_at_brace=False)
    return width_nm if saw_markup else None


def _schematic_outline_text_width_nm(
    text: str,
    size_x_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
) -> int:
    """Return KiCad outline-font text width for SVG textLength metrics."""
    if not text or size_x_nm <= 0:
        return 0
    display_text = _plot_display_text(text)
    if not display_text:
        return 0
    markup_width = _markup_outline_text_width_nm(
        display_text,
        size_x_nm,
        bold=bold,
        italic=italic,
        font_face=font_face,
    )
    if markup_width is not None:
        return markup_width
    metric_text = _plot_metric_text(display_text)
    if not metric_text:
        return 0
    return _schematic_plain_text_width_nm(
        metric_text,
        size_x_nm,
        bold=bold,
        italic=italic,
        font_face=font_face,
    )


@lru_cache(maxsize=16)
def _outline_font_line_height_nm(
    size_y_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
) -> Optional[int]:
    """Return KiCad outline-font line bbox height for overbar field centering."""
    if size_y_nm <= 0:
        return 0
    font_path = _outline_font_path(
        font_face,
        bold=bool(bold),
        italic=bool(italic),
    )
    if font_path is None:
        return None
    try:
        import freetype
    except Exception:
        return None

    try:
        face = freetype.Face(font_path)
        face.set_char_size(0, _OUTLINE_FONT_FACE_SIZE, _GLYPH_RESOLUTION, 0)
        ascender = int(abs(face.size.ascender * _GLYPH_SIZE_SCALER))
        descender = int(abs(face.size.descender * _GLYPH_SIZE_SCALER))
        size_y_iu = _nm_to_schematic_iu(size_y_nm)
        scale = (
            float(size_y_iu)
            / _OUTLINE_FONT_FACE_SIZE
            * _OUTLINE_FONT_SIZE_COMPENSATION
        )
        height_iu = int(ascender * scale) + int(descender * scale)
    except Exception:
        return None
    return _schematic_iu_to_nm(height_iu)


def _symbol_field_outline_center_voffset_nm(
    size_y_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
) -> int:
    """Distance from a field's justified text position to its bbox center.

    ``SCH_FIELD::Plot`` centers symbol-owned fields on
    ``SCH_FIELD::GetBoundingBox().Centre()`` and plots them with center
    alignment.  ``EDA_TEXT::GetTextBox`` builds outline-font bboxes from the
    face ascender/descender and shifts top/bottom text by the 17% fudge factor.
    """
    line_height_nm = _outline_font_line_height_nm(
        size_y_nm,
        bold=bool(bold),
        italic=bool(italic),
        font_face=font_face,
    )
    if line_height_nm is None:
        return size_y_nm // 2

    line_height_iu = _nm_to_schematic_iu(line_height_nm)
    fudge_iu = _ki_round(line_height_iu * _EDA_TEXT_BBOX_FUDGE_RATIO)
    return _schematic_iu_to_nm((line_height_iu // 2) - fudge_iu)


def _symbol_field_overbar_vjust_extra_nm(
    text: str,
    size_y_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
) -> int:
    """Mirror the extra ``EDA_TEXT::GetTextBox`` height for overbar markup."""
    if "~{" not in str(text) or size_y_nm <= 0:
        return 0

    line_height_nm = _outline_font_line_height_nm(
        size_y_nm,
        bold=bool(bold),
        italic=bool(italic),
        font_face=font_face,
    )
    if line_height_nm is None:
        return 0

    line_height_iu = _nm_to_schematic_iu(line_height_nm)
    return _schematic_iu_to_nm(
        int(line_height_iu * _EDA_TEXT_OVERBAR_HEIGHT_RATIO) // 2
    )


def _sch_text_outline_adjust_nm(
    text: str,
    size_y_nm: int,
    *,
    bold: bool = False,
    italic: bool = False,
    font_face: str = "",
) -> int:
    size_y_iu = _nm_to_schematic_iu(size_y_nm)
    first_line = text.split("\n", 1)[0] if text else ""
    if first_line == "":
        return _schematic_iu_to_nm(
            _ki_round(-size_y_iu * _SCH_TEXT_FIELD_MATCH_ADJUST_RATIO)
        )

    line_height_nm = _outline_font_line_height_nm(
        size_y_nm,
        bold=bold,
        italic=italic,
        font_face=font_face,
    )
    if line_height_nm is None:
        return 0
    line_height_iu = _nm_to_schematic_iu(line_height_nm)
    size_diff_iu = line_height_iu - size_y_iu
    return _schematic_iu_to_nm(
        _ki_round(size_diff_iu * _SCH_TEXT_FIELD_MATCH_ADJUST_RATIO)
    )


# Per-shape rotation around the anchor for SPIN_STYLE values
# 0=LEFT, 1=UP, 2=RIGHT, 3=BOTTOM. Matches the
# ``RotatePoint(aPoint, ±ANGLE_90 / ANGLE_180)`` cases in
# ``SCH_GLOBALLABEL::CreateGraphicShape`` (``sch_label.cpp:2353-2360``).
def _rotate_for_spin(px: int, py: int, spin_idx: int) -> Tuple[int, int]:
    if spin_idx == 0:    # LEFT  → identity
        return px, py
    if spin_idx == 1:    # UP    → RotatePoint(p, -90°): (x,y) → (-y, x)
        return -py, px
    if spin_idx == 2:    # RIGHT → RotatePoint(p, 180°): (x,y) → (-x,-y)
        return -px, -py
    if spin_idx == 3:    # BOTTOM → RotatePoint(p, +90°): (x,y) → (y, -x)
        return py, -px
    return px, py


def global_label_decoration_to_op(
    label: "SchGlobalLabel",
) -> Optional[KiCadPlotterOp]:
    """Arrow-box decoration polygon for a :class:`SchGlobalLabel`.

    Mirrors ``SCH_GLOBALLABEL::CreateGraphicShape``
    (``sch_label.cpp:2305-2366``). Builds a 6-point box around the
    label text with one side notched into a triangle whose apex
    direction depends on ``label.shape``:

      * ``input``           — apex on the right (``aPoints[0]``)
      * ``output``          — apex on the left (``aPoints[3]``)
      * ``bidirectional``,
        ``tri_state``       — apex on both ends
      * ``passive`` (UNSPECIFIED) — flat box, no apex

    Returns ``None`` for SCH_DIRECTIVE_LABEL shapes
    (``dot/round/diamond/rectangle``) which use a different geometry
    path.

    Text-width estimate follows KiCad's outline-font text box metrics
    used by ``EDA_TEXT::GetTextBox``.
    """
    shape_key = _shape_decoration_key(getattr(label, "shape", None))
    if shape_key is None:
        return None

    text_height_nm = _label_text_height_nm(label)
    margin_nm = int(round(DEFAULT_LABEL_SIZE_RATIO * text_height_nm))
    half_size_nm = (text_height_nm // 2) + margin_nm
    line_width_nm = mm_to_nm(DEFAULT_WIRE_WIDTH_MM)

    text = _plot_display_text(getattr(label, "text", "") or "")
    size_x_nm = _label_text_size_x_nm(label)
    eff = getattr(label, "effects", None)
    bold = bool(
        eff is not None
        and getattr(eff, "font", None) is not None
        and getattr(eff.font, "bold", False)
    )
    font_face = ""
    if eff is not None and getattr(eff, "font", None) is not None:
        font_face = str(getattr(eff.font, "face", "") or "")
    text_width_nm = _schematic_outline_text_width_nm(
        text,
        size_x_nm,
        bold=bold,
        italic=bool(
            eff is not None
            and getattr(eff, "font", None) is not None
            and getattr(eff.font, "italic", False)
        ),
        font_face=font_face,
    )

    symb_len = text_width_nm + 2 * margin_nm
    x = symb_len + line_width_nm + 3
    y = half_size_nm + line_width_nm + 3

    # Six outline points (relative to anchor) — matches sch_label.cpp:2319-2324.
    pts = [
        [0, 0],
        [0, -y],
        [-x, -y],
        [-x, 0],
        [-x, y],
        [0, y],
    ]

    # Per-shape apex offsets (sch_label.cpp:2328-2346).
    x_offset = 0
    if shape_key == "input":
        x_offset = -half_size_nm
        pts[0][0] += half_size_nm
    elif shape_key == "output":
        pts[3][0] -= half_size_nm
    elif shape_key in ("bidirectional", "tri_state"):
        x_offset = -half_size_nm
        pts[0][0] += half_size_nm
        pts[3][0] -= half_size_nm
    # "passive" (UNSPECIFIED): no offset, flat box.

    spin_idx = _label_spin_idx(label)
    anchor_x_nm = mm_to_nm(label.at_x)
    anchor_y_nm = mm_to_nm(label.at_y)

    rotated: List[Tuple[int, int]] = []
    for px, py in pts:
        px += x_offset
        rx, ry = _rotate_for_spin(px, py, spin_idx)
        rotated.append((rx + anchor_x_nm, ry + anchor_y_nm))

    # Closing point (sch_label.cpp:2365: aPoints.push_back(aPoints[0])).
    rotated.append(rotated[0])

    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=rotated,
            fill=KiCadFillType.NO_FILL,
            width_nm=line_width_nm,
        ),
        stroke_color=LAYER_GLOBLABEL,
    )


def sch_text_to_op(
    txt: "SchText",
    *,
    default_line_width_nm: int | None = None,
    project_vars: Optional[dict] = None,
) -> KiCadPlotterOp:
    """Convert a :class:`SchText` (top-level annotation) to a ``Text`` op."""
    kwargs = _text_kwargs_with_plot_defaults(
        txt.effects,
        LAYER_NOTES,
        default_line_width_nm=default_line_width_nm,
    )
    if "size_x_nm" not in kwargs or kwargs.get("size_x_nm", 0) == 0:
        kwargs["size_x_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    if "size_y_nm" not in kwargs or kwargs.get("size_y_nm", 0) == 0:
        kwargs["size_y_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
    kwargs.setdefault("v_align", KiCadVertAlign.CENTER)
    text = _wx_string_split_plot_text(
        _expand_project_text_variables(txt.text or "", project_vars)
    )
    outline_adjust_nm = _sch_text_outline_adjust_nm(
        text,
        int(kwargs["size_y_nm"]),
        bold=bool(kwargs.get("bold", False)),
        italic=bool(kwargs.get("italic", False)),
        font_face=str(kwargs.get("font_face", "") or ""),
    )
    adjust_x, adjust_y = _rotate_xy(0, -outline_adjust_nm, -float(txt.at_angle))
    return KiCadPlotterOp.text(
        x=mm_to_nm(txt.at_x) + _ki_round(adjust_x),
        y=mm_to_nm(txt.at_y) - _SCH_TEXT_PLOT_OFFSET_NM + _ki_round(adjust_y),
        text=text,
        orient_deg=float(txt.at_angle),
        multiline="\n" in text,
        **kwargs,
    )


_TEXTBOX_INTERLINE_FACTOR = _FONT_METRICS_INTERLINE_PITCH


def _text_kwargs_with_default_size(effects) -> dict:
    kwargs = _effects_to_text_kwargs(effects)
    if "size_x_nm" not in kwargs or kwargs.get("size_x_nm", 0) == 0:
        kwargs["size_x_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    if "size_y_nm" not in kwargs or kwargs.get("size_y_nm", 0) == 0:
        kwargs["size_y_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    return kwargs


def _coerce_h_align(value) -> KiCadHorizAlign:
    if isinstance(value, KiCadHorizAlign):
        return value
    try:
        return KiCadHorizAlign(str(value))
    except ValueError:
        return KiCadHorizAlign.LEFT


def _coerce_v_align(value) -> KiCadVertAlign:
    if isinstance(value, KiCadVertAlign):
        return value
    try:
        return KiCadVertAlign(str(value))
    except ValueError:
        return KiCadVertAlign.TOP


def _rotate_xy(x: float, y: float, angle_deg: float) -> tuple[float, float]:
    a = float(angle_deg) % 360.0
    if a == 0.0:
        return x, y
    if a == 90.0:
        return -y, x
    if a == 180.0:
        return -x, -y
    if a == 270.0:
        return y, -x
    rad = math.radians(a)
    c, s = math.cos(rad), math.sin(rad)
    return x * c - y * s, x * s + y * c


def _text_box_margins_nm(tb: "SchTextBox", kwargs: dict) -> tuple[int, int, int, int]:
    if tb.margins is not None:
        left, top, right, bottom = tb.margins
        return mm_to_nm(left), mm_to_nm(top), mm_to_nm(right), mm_to_nm(bottom)

    # SCH_TEXTBOX::GetLegacyTextMargin(): schematic text boxes use
    # stroke / 2 + text_height * 0.75 when the source omits margins.
    stroke_mm = tb.stroke.width if tb.stroke and tb.stroke.width > 0 else 0.0
    margin_mm = (stroke_mm / 2.0) + (int(kwargs["size_y_nm"]) / 1_000_000.0 * 0.75)
    margin_nm = mm_to_nm(margin_mm)
    return margin_nm, margin_nm, margin_nm, margin_nm


def _text_box_wrap_width_nm(
    tb: "SchTextBox",
    margins: tuple[int, int, int, int],
) -> int:
    margin_left, margin_top, margin_right, margin_bottom = margins
    if (float(tb.at_angle) % 180.0) == 90.0:
        return max(0, mm_to_nm(abs(tb.size_y)) - margin_top - margin_bottom)
    return max(0, mm_to_nm(abs(tb.size_x)) - margin_left - margin_right)


def _wrap_text_box_line(line: str, max_width_nm: int, kwargs: dict) -> list[str]:
    if not line or max_width_nm <= 0:
        return [line]
    size_x_nm = int(kwargs.get("size_x_nm") or kwargs.get("size_y_nm") or 0)
    bold = bool(kwargs.get("bold", False))
    font_face = str(kwargs.get("font_face", "") or "")
    italic = bool(kwargs.get("italic", False))
    if (
        _schematic_outline_text_width_nm(
            line,
            size_x_nm,
            bold=bold,
            italic=italic,
            font_face=font_face,
        )
        <= max_width_nm
    ):
        return [line]

    words = line.split(" ")
    out: list[str] = []
    current = ""
    for word in words:
        candidate = word if current == "" else f"{current} {word}"
        if current and _schematic_outline_text_width_nm(
            candidate,
            size_x_nm,
            bold=bold,
            italic=italic,
            font_face=font_face,
        ) > max_width_nm:
            out.append(current.rstrip())
            current = word
        else:
            current = candidate
    if current or not out:
        out.append(current.rstrip())
    return out


def _wrap_text_box_lines(
    tb: "SchTextBox",
    kwargs: dict,
    margins: tuple[int, int, int, int],
    text: str,
) -> list[str]:
    # KiCad's FONT::LinebreakText tests against column width minus stroke
    # thickness, so text that exactly fits the content box still wraps.
    max_width_nm = max(
        0,
        _text_box_wrap_width_nm(tb, margins) - int(kwargs.get("pen_width_nm") or 0),
    )
    out: list[str] = []
    raw_lines = text.split("\n")
    while raw_lines and raw_lines[-1] == "":
        raw_lines.pop()
    for line in raw_lines:
        out.extend(_wrap_text_box_line(line.rstrip(), max_width_nm, kwargs))
    return out


def _text_box_draw_pos_nm(
    tb: "SchTextBox",
    h_align: KiCadHorizAlign,
    v_align: KiCadVertAlign,
    margins: tuple[int, int, int, int],
) -> tuple[int, int]:
    x1 = mm_to_nm(tb.at_x)
    y1 = mm_to_nm(tb.at_y)
    x2 = mm_to_nm(tb.at_x + tb.size_x)
    y2 = mm_to_nm(tb.at_y + tb.size_y)
    left, right = min(x1, x2), max(x1, x2)
    top, bottom = min(y1, y2), max(y1, y2)
    margin_left, margin_top, margin_right, margin_bottom = margins

    vertical = (float(tb.at_angle) % 180.0) == 90.0
    if vertical:
        if h_align == KiCadHorizAlign.CENTER:
            y = (top + bottom) // 2
        elif h_align == KiCadHorizAlign.RIGHT:
            y = top + margin_top
        else:
            y = bottom - margin_bottom

        if v_align == KiCadVertAlign.CENTER:
            x = (left + right) // 2
        elif v_align == KiCadVertAlign.BOTTOM:
            x = right - margin_right
        else:
            x = left + margin_left
    else:
        if h_align == KiCadHorizAlign.CENTER:
            x = (left + right) // 2
        elif h_align == KiCadHorizAlign.RIGHT:
            x = right - margin_right
        else:
            x = left + margin_left

        if v_align == KiCadVertAlign.CENTER:
            y = (top + bottom) // 2
        elif v_align == KiCadVertAlign.BOTTOM:
            y = bottom - margin_bottom
        else:
            y = top + margin_top

    return x, y


def _text_box_line_positions_nm(
    tb: "SchTextBox",
    line_count: int,
    kwargs: dict,
    h_align: KiCadHorizAlign,
    v_align: KiCadVertAlign,
    margins: tuple[int, int, int, int],
) -> list[tuple[int, int]]:
    draw_x, draw_y = _text_box_draw_pos_nm(tb, h_align, v_align, margins)
    line_step = int(round(int(kwargs["size_y_nm"]) * _TEXTBOX_INTERLINE_FACTOR))
    pos_x, pos_y = draw_x, draw_y

    if line_count > 1:
        if v_align == KiCadVertAlign.CENTER:
            pos_y -= (line_count - 1) * line_step // 2
        elif v_align == KiCadVertAlign.BOTTOM:
            pos_y -= (line_count - 1) * line_step

    rel_x, rel_y = _rotate_xy(pos_x - draw_x, pos_y - draw_y, -float(tb.at_angle))
    step_x, step_y = _rotate_xy(0, line_step, -float(tb.at_angle))
    pos_x = int(round(draw_x + rel_x))
    pos_y = int(round(draw_y + rel_y))
    step_x_i = int(round(step_x))
    step_y_i = int(round(step_y))

    out: list[tuple[int, int]] = []
    for _idx in range(line_count):
        out.append((pos_x, pos_y))
        pos_x += step_x_i
        pos_y += step_y_i
    return out


def text_box_outline_to_op(tb: "SchTextBox") -> KiCadPlotterOp:
    """Convert a schematic ``text_box`` outline into a ``Rect`` op."""
    line_style, width_nm, color = _resolve_stroke(
        tb.stroke, DEFAULT_WIRE_WIDTH_MM, LAYER_NOTES
    )
    fill_color = rgba_to_hex(tb.fill.color) if getattr(tb.fill, "color", None) else None
    return styled_plotter_op(
        KiCadPlotterOp.rect(
            x1=mm_to_nm(tb.at_x),
            y1=mm_to_nm(tb.at_y),
            x2=mm_to_nm(tb.at_x + tb.size_x),
            y2=mm_to_nm(tb.at_y + tb.size_y),
            fill=sym_fill_to_kicad_fill(tb.fill.type),
            width_nm=width_nm,
            corner_radius_nm=0,
        ),
        stroke_color=color,
        fill_color=fill_color,
        line_style=line_style,
    )


def text_box_to_ops(
    tb: "SchTextBox",
    *,
    project_vars: Optional[dict] = None,
) -> List[KiCadPlotterOp]:
    """Convert a schematic ``text_box`` into outline and per-line text ops."""
    outline = text_box_outline_to_op(tb)
    fill = KiCadFillType(outline.payload["fill"])
    ops: List[KiCadPlotterOp] = []

    if fill not in (KiCadFillType.NO_FILL, KiCadFillType.FILLED_SHAPE):
        fill_payload = dict(outline.payload)
        fill_payload["width_nm"] = 0
        fill_color = fill_payload.get("fill_color") or fill_payload.get("stroke_color")
        if fill_color:
            fill_payload["stroke_color"] = fill_color
            fill_payload["fill_color"] = fill_color
        ops.append(KiCadPlotterOp(kind=outline.kind, payload=fill_payload))

        outline_payload = dict(outline.payload)
        outline_payload["fill"] = KiCadFillType.NO_FILL.value
        outline_payload.pop("fill_color", None)
        ops.append(KiCadPlotterOp(kind=outline.kind, payload=outline_payload))
    else:
        ops.append(outline)

    kwargs = apply_default_text_style(
        _text_kwargs_with_default_size(tb.effects), LAYER_NOTES
    )
    h_align = _coerce_h_align(kwargs.get("h_align", KiCadHorizAlign.CENTER))
    v_align = _coerce_v_align(kwargs.get("v_align", KiCadVertAlign.CENTER))
    kwargs["h_align"] = h_align
    kwargs["v_align"] = v_align

    text = _expand_project_text_variables(tb.text or "", project_vars)
    margins = _text_box_margins_nm(tb, kwargs)
    lines = _wrap_text_box_lines(tb, kwargs, margins, text)
    positions = _text_box_line_positions_nm(
        tb,
        len(lines),
        kwargs,
        h_align,
        v_align,
        margins,
    )

    text_kwargs = dict(kwargs)
    text_kwargs["multiline"] = False
    for line, (x, y) in zip(lines, positions):
        if line == "":
            continue
        ops.append(
            KiCadPlotterOp.text(
                x=x,
                y=y,
                text=line,
                orient_deg=float(tb.at_angle),
                **text_kwargs,
            )
        )
    return ops


_GRAPHIC_FILLED_OUTLINE_KINDS = {
    KiCadPlotterOpKind.RECT,
    KiCadPlotterOpKind.CIRCLE,
    KiCadPlotterOpKind.ARC_THREE_POINT,
    KiCadPlotterOpKind.PLOT_POLY,
}

_SCHEMATIC_IMAGE_DPI = 300.0


def _graphic_fill_color(fill, stroke_color: str | None) -> str | None:
    explicit = rgba_to_hex(fill.color) if getattr(fill, "color", None) else None
    if explicit:
        return explicit
    fill_type = getattr(fill, "type", None)
    fill_value = getattr(fill_type, "value", str(fill_type) if fill_type else "")
    if fill_value == "background":
        return LAYER_SCHEMATIC_BACKGROUND
    if fill_value == "outline":
        return stroke_color or LAYER_NOTES
    if fill_value in {"color", "hatch", "reverse_hatch", "cross_hatch"}:
        return LAYER_NOTES
    return None


def _split_graphic_fill_outline_op(op: KiCadPlotterOp) -> List[KiCadPlotterOp]:
    if op.kind not in _GRAPHIC_FILLED_OUTLINE_KINDS:
        return [op]
    fill = str(op.payload.get("fill") or "")
    if fill in (KiCadFillType.NO_FILL.value, KiCadFillType.FILLED_SHAPE.value):
        return [op]

    fill_payload = dict(op.payload)
    fill_payload["width_nm"] = 0
    fill_color = fill_payload.get("fill_color") or fill_payload.get("stroke_color")
    if fill_color:
        fill_payload["stroke_color"] = fill_color
        fill_payload["fill_color"] = fill_color

    outline_payload = dict(op.payload)
    outline_payload["fill"] = KiCadFillType.NO_FILL.value
    outline_payload.pop("fill_color", None)
    return [
        KiCadPlotterOp(kind=op.kind, payload=fill_payload),
        KiCadPlotterOp(kind=op.kind, payload=outline_payload),
    ]


def _graphic_shape_style(stroke, fill) -> tuple[KiCadLineStyle, int, str | None, str | None]:
    line_style, width_nm, stroke_color = _resolve_stroke(
        stroke, DEFAULT_WIRE_WIDTH_MM, LAYER_NOTES
    )
    fill_color = _graphic_fill_color(fill, stroke_color)
    return line_style, width_nm, stroke_color, fill_color


def schematic_polyline_to_ops(poly: "SchPolyline") -> List[KiCadPlotterOp]:
    if len(poly.points) < 2:
        return []
    line_style, width_nm, stroke_color, fill_color = _graphic_shape_style(
        poly.stroke, poly.fill
    )
    op = styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=[(mm_to_nm(x), mm_to_nm(y)) for x, y in poly.points],
            fill=sym_fill_to_kicad_fill(poly.fill.type),
            width_nm=width_nm,
        ),
        stroke_color=stroke_color,
        fill_color=fill_color,
        line_style=line_style,
    )
    return _split_graphic_fill_outline_op(op)


def schematic_rectangle_to_ops(rect: "SchRectangle") -> List[KiCadPlotterOp]:
    line_style, width_nm, stroke_color, fill_color = _graphic_shape_style(
        rect.stroke, rect.fill
    )
    op = styled_plotter_op(
        KiCadPlotterOp.rect(
            x1=mm_to_nm(rect.start_x),
            y1=mm_to_nm(rect.start_y),
            x2=mm_to_nm(rect.end_x),
            y2=mm_to_nm(rect.end_y),
            fill=sym_fill_to_kicad_fill(rect.fill.type),
            width_nm=width_nm,
            corner_radius_nm=mm_to_nm(rect.radius or 0.0),
        ),
        stroke_color=stroke_color,
        fill_color=fill_color,
        line_style=line_style,
    )
    return _split_graphic_fill_outline_op(op)


def schematic_arc_to_ops(arc: "SchArc") -> List[KiCadPlotterOp]:
    line_style, width_nm, stroke_color, fill_color = _graphic_shape_style(
        arc.stroke, arc.fill
    )
    op = styled_plotter_op(
        KiCadPlotterOp.arc_three_point(
            start_x=mm_to_nm(arc.start_x),
            start_y=mm_to_nm(arc.start_y),
            mid_x=mm_to_nm(arc.mid_x),
            mid_y=mm_to_nm(arc.mid_y),
            end_x=mm_to_nm(arc.end_x),
            end_y=mm_to_nm(arc.end_y),
            fill=sym_fill_to_kicad_fill(arc.fill.type),
            width_nm=width_nm,
        ),
        stroke_color=stroke_color,
        fill_color=fill_color,
        line_style=line_style,
    )
    return _split_graphic_fill_outline_op(op)


def schematic_circle_to_ops(circle: "SchCircle") -> List[KiCadPlotterOp]:
    line_style, width_nm, stroke_color, fill_color = _graphic_shape_style(
        circle.stroke, circle.fill
    )
    op = styled_plotter_op(
        KiCadPlotterOp.circle(
            cx=mm_to_nm(circle.center_x),
            cy=mm_to_nm(circle.center_y),
            diameter_nm=mm_to_nm(circle.radius * 2.0),
            fill=sym_fill_to_kicad_fill(circle.fill.type),
            width_nm=width_nm,
        ),
        stroke_color=stroke_color,
        fill_color=fill_color,
        line_style=line_style,
    )
    return _split_graphic_fill_outline_op(op)


def schematic_bezier_to_ops(bez: "SchBezier") -> List[KiCadPlotterOp]:
    if len(bez.points) < 2:
        return []
    line_style, width_nm, stroke_color, fill_color = _graphic_shape_style(
        bez.stroke, bez.fill
    )
    if len(bez.points) == 4:
        sx, sy = bez.points[0]
        c1x, c1y = bez.points[1]
        c2x, c2y = bez.points[2]
        ex, ey = bez.points[3]
        return [
            styled_plotter_op(
                KiCadPlotterOp.bezier_curve(
                    start_x=mm_to_nm(sx),
                    start_y=mm_to_nm(sy),
                    ctrl1_x=mm_to_nm(c1x),
                    ctrl1_y=mm_to_nm(c1y),
                    ctrl2_x=mm_to_nm(c2x),
                    ctrl2_y=mm_to_nm(c2y),
                    end_x=mm_to_nm(ex),
                    end_y=mm_to_nm(ey),
                    width_nm=width_nm,
                ),
                stroke_color=stroke_color,
                line_style=line_style,
            )
        ]

    op = styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=[(mm_to_nm(x), mm_to_nm(y)) for x, y in bez.points],
            fill=sym_fill_to_kicad_fill(bez.fill.type),
            width_nm=width_nm,
        ),
        stroke_color=stroke_color,
        fill_color=fill_color,
        line_style=line_style,
    )
    return _split_graphic_fill_outline_op(op)


def _image_data_b64(img: "SchImage") -> str:
    return "".join(str(chunk).strip() for chunk in (img.data or ()))


def _image_bytes_from_b64(data_b64: str) -> bytes:
    if not data_b64:
        return b""
    try:
        return base64.b64decode(data_b64, validate=False)
    except (binascii.Error, ValueError):
        return b""


def _kicad_ppi_from_ppm(pixels_per_meter: int | float | None) -> int | None:
    if pixels_per_meter is None or pixels_per_meter <= 0:
        return None
    return int(round(float(pixels_per_meter) * 0.0254)) or None


def _kicad_bmp_ppi_from_ppm(pixels_per_meter: int | None) -> int | None:
    if pixels_per_meter is None or pixels_per_meter <= 0:
        return None
    # wx's BMP loader exposes resolution as an integer pixels-per-cm option.
    # KiCad then multiplies that truncated value by 2.54 and rounds.
    pixels_per_cm = int(pixels_per_meter // 100)
    return int(round(float(pixels_per_cm) * 2.54)) or None


def _jpeg_metadata(data: bytes) -> tuple[int, int, int | None, int | None] | None:
    pos = 2
    size = len(data)
    ppi_x: int | None = None
    ppi_y: int | None = None
    while pos + 9 <= size:
        if data[pos] != 0xFF:
            pos += 1
            continue
        marker = data[pos + 1]
        pos += 2
        if marker in {0xD8, 0xD9}:
            continue
        if pos + 2 > size:
            return None
        segment_len = int.from_bytes(data[pos:pos + 2], "big")
        if segment_len < 2 or pos + segment_len > size:
            return None
        segment = data[pos + 2:pos + segment_len]
        if marker == 0xE0 and segment.startswith(b"JFIF\x00") and len(segment) >= 12:
            units = segment[7]
            density_x = int.from_bytes(segment[8:10], "big")
            density_y = int.from_bytes(segment[10:12], "big")
            if density_x > 0 and density_y > 0:
                if units == 1:  # pixels per inch
                    ppi_x = int(round(float(density_x))) or None
                    ppi_y = int(round(float(density_y))) or None
                elif units == 2:  # pixels per centimeter
                    ppi_x = int(round(float(density_x) * 2.54)) or None
                    ppi_y = int(round(float(density_y) * 2.54)) or None
        if marker in {
            0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7,
            0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
        }:
            if segment_len < 7:
                return None
            height = int.from_bytes(data[pos + 3:pos + 5], "big")
            width = int.from_bytes(data[pos + 5:pos + 7], "big")
            return width, height, ppi_x, ppi_y
        pos += segment_len
    return None


def _png_metadata(data: bytes) -> tuple[int, int, int | None, int | None]:
    width = 0
    height = 0
    ppm_x: int | None = None
    ppm_y: int | None = None
    pos = 8
    size = len(data)
    while pos + 8 <= size:
        chunk_len = int.from_bytes(data[pos:pos + 4], "big")
        chunk_type = data[pos + 4:pos + 8]
        chunk_start = pos + 8
        chunk_end = chunk_start + chunk_len
        if chunk_end + 4 > size:
            break
        chunk = data[chunk_start:chunk_end]
        if chunk_type == b"IHDR" and chunk_len >= 8:
            width = int.from_bytes(chunk[0:4], "big")
            height = int.from_bytes(chunk[4:8], "big")
        elif chunk_type == b"pHYs" and chunk_len >= 9 and chunk[8] == 1:
            ppm_x = int.from_bytes(chunk[0:4], "big") or None
            ppm_y = int.from_bytes(chunk[4:8], "big") or None
        pos = chunk_end + 4
    return width, height, ppm_x, ppm_y


def _bmp_metadata(data: bytes) -> tuple[int, int, int | None, int | None] | None:
    if len(data) < 26 or not data.startswith(b"BM"):
        return None
    dib_header_size = int.from_bytes(data[14:18], "little")
    if dib_header_size == 12 and len(data) >= 26:
        width = int.from_bytes(data[18:20], "little")
        height = int.from_bytes(data[20:22], "little")
        return width, height, None, None
    if dib_header_size < 40 or len(data) < 54:
        return None
    width = int.from_bytes(data[18:22], "little", signed=True)
    height = int.from_bytes(data[22:26], "little", signed=True)
    ppm_x = int.from_bytes(data[38:42], "little", signed=True)
    ppm_y = int.from_bytes(data[42:46], "little", signed=True)
    return (
        abs(width),
        abs(height),
        _kicad_bmp_ppi_from_ppm(ppm_x),
        _kicad_bmp_ppi_from_ppm(ppm_y),
    )


def _image_metadata(data: bytes) -> tuple[str, int, int, int | None, int | None]:
    if len(data) >= 24 and data.startswith(b"\x89PNG\r\n\x1a\n"):
        width, height, ppm_x, ppm_y = _png_metadata(data)
        return (
            "png",
            width,
            height,
            _kicad_ppi_from_ppm(ppm_x),
            _kicad_ppi_from_ppm(ppm_y),
        )
    if len(data) >= 4 and data.startswith(b"\xFF\xD8"):
        meta = _jpeg_metadata(data)
        if meta is not None:
            return "jpeg", meta[0], meta[1], meta[2], meta[3]
    if len(data) >= 26 and data.startswith(b"BM"):
        meta = _bmp_metadata(data)
        if meta is not None:
            return "bmp", meta[0], meta[1], meta[2], meta[3]
    return "png", 0, 0, None, None


def _image_extent_nm(
    size_px: int,
    scale: float,
    ppi: int | None = None,
) -> int:
    if ppi and ppi > 0:
        size_mm = float(size_px) * float(scale) * 25.4 / float(ppi)
    else:
        size_mm = float(size_px) * float(scale) * 25.4 / _SCHEMATIC_IMAGE_DPI
    return mm_to_nm(size_mm)


def schematic_image_to_op(img: "SchImage") -> KiCadPlotterOp:
    data_b64 = _image_data_b64(img)
    image_format, width_px, height_px, ppi_x, ppi_y = _image_metadata(
        _image_bytes_from_b64(data_b64)
    )
    scale = float(img.scale) if img.scale is not None else 1.0
    return styled_plotter_op(
        KiCadPlotterOp.plot_image(
            x=mm_to_nm(img.at_x),
            y=mm_to_nm(img.at_y),
            width_nm=(
                _image_extent_nm(width_px, scale, ppi_x) if width_px > 0 else 0
            ),
            height_nm=(
                _image_extent_nm(height_px, scale, ppi_y) if height_px > 0 else 0
            ),
            scale=scale,
            image_data_b64=data_b64,
            image_format=image_format,
        ),
        stroke_color=LAYER_NOTES,
    )


# ---------------------------------------------------------------------------
# Per-element record emitters
# ---------------------------------------------------------------------------


def _wire_record(wire: "SchWire") -> Optional[KiCadPlotterRecord]:
    op = wire_to_op(wire)
    if op is None:
        return None
    return KiCadPlotterRecord(
        uuid=wire.uuid or "",
        kind="wire",
        object_id=wire.uuid or "",
        bounds=None,
        operations=[op],
        extras={},
    )


def _bus_record(bus: "SchBus") -> Optional[KiCadPlotterRecord]:
    op = bus_to_op(bus)
    if op is None:
        return None
    return KiCadPlotterRecord(
        uuid=bus.uuid or "",
        kind="bus",
        object_id=bus.uuid or "",
        bounds=None,
        operations=[op],
        extras={},
    )


def _bus_entry_record(entry: "SchBusEntry") -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=entry.uuid or "",
        kind="bus_entry",
        object_id=entry.uuid or "",
        bounds=None,
        operations=[bus_entry_to_op(entry)],
        extras={},
    )


def _junction_record(junction: "SchJunction") -> KiCadPlotterRecord:
    extras: dict = {}
    if junction.color:
        extras["color"] = rgba_to_hex(junction.color)
    return KiCadPlotterRecord(
        uuid=junction.uuid or "",
        kind="junction",
        object_id=junction.uuid or "",
        bounds=None,
        operations=[junction_to_op(junction)],
        extras=extras,
    )


def _no_connect_record(
    nc: "SchNoConnect",
    *,
    default_line_width_nm: int | None = None,
) -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=nc.uuid or "",
        kind="no_connect",
        object_id=nc.uuid or "",
        bounds=None,
        operations=no_connect_to_ops(
            nc,
            default_line_width_nm=default_line_width_nm,
        ),
        extras={},
    )


def _label_record(
    label,
    kind: str,
    op_fn,
    decoration_fn=None,
) -> KiCadPlotterRecord:
    extras: dict = {"text": label.text}
    shape_obj = getattr(label, "shape", None)
    if shape_obj is not None:
        shape_val = getattr(shape_obj, "value", shape_obj)
        if shape_val is not None:
            extras["shape"] = shape_val
    operations: List[KiCadPlotterOp] = [op_fn(label)]
    if decoration_fn is not None:
        deco = decoration_fn(label)
        if deco is not None:
            operations.append(deco)
    return KiCadPlotterRecord(
        uuid=getattr(label, "uuid", "") or "",
        kind=kind,
        object_id=label.text,
        bounds=None,
        operations=operations,
        extras=extras,
    )


def _text_record(
    txt: "SchText",
    *,
    default_line_width_nm: int | None = None,
    project_vars: Optional[dict] = None,
) -> KiCadPlotterRecord:
    text = _expand_project_text_variables(txt.text or "", project_vars)
    return KiCadPlotterRecord(
        uuid=txt.uuid or "",
        kind="text",
        object_id=txt.uuid or "",
        bounds=None,
        operations=[
            sch_text_to_op(
                txt,
                default_line_width_nm=default_line_width_nm,
                project_vars=project_vars,
            )
        ],
        extras={"text": text},
    )


def _text_box_record(
    tb: "SchTextBox",
    *,
    project_vars: Optional[dict] = None,
) -> KiCadPlotterRecord:
    text = _expand_project_text_variables(tb.text or "", project_vars)
    return KiCadPlotterRecord(
        uuid=tb.uuid or "",
        kind="text_box",
        object_id=tb.uuid or "",
        bounds=None,
        operations=text_box_to_ops(tb, project_vars=project_vars),
        extras={"text": text},
    )


def _graphic_record(
    *,
    uuid: str,
    kind: str,
    operations: List[KiCadPlotterOp],
    extras: Optional[dict] = None,
) -> Optional[KiCadPlotterRecord]:
    if not operations:
        return None
    return KiCadPlotterRecord(
        uuid=uuid or "",
        kind=kind,
        object_id=uuid or "",
        bounds=None,
        operations=operations,
        extras=extras or {},
    )


def _graphic_polyline_record(poly: "SchPolyline") -> Optional[KiCadPlotterRecord]:
    return _graphic_record(
        uuid=poly.uuid,
        kind="graphic_polyline",
        operations=schematic_polyline_to_ops(poly),
    )


def _graphic_rectangle_record(rect: "SchRectangle") -> Optional[KiCadPlotterRecord]:
    return _graphic_record(
        uuid=rect.uuid,
        kind="graphic_rectangle",
        operations=schematic_rectangle_to_ops(rect),
    )


def _graphic_arc_record(arc: "SchArc") -> Optional[KiCadPlotterRecord]:
    return _graphic_record(
        uuid=arc.uuid,
        kind="graphic_arc",
        operations=schematic_arc_to_ops(arc),
    )


def _graphic_circle_record(circle: "SchCircle") -> Optional[KiCadPlotterRecord]:
    return _graphic_record(
        uuid=circle.uuid,
        kind="graphic_circle",
        operations=schematic_circle_to_ops(circle),
    )


def _graphic_bezier_record(bez: "SchBezier") -> Optional[KiCadPlotterRecord]:
    return _graphic_record(
        uuid=bez.uuid,
        kind="graphic_bezier",
        operations=schematic_bezier_to_ops(bez),
    )


def _image_record(img: "SchImage") -> KiCadPlotterRecord:
    op = schematic_image_to_op(img)
    return KiCadPlotterRecord(
        uuid=img.uuid or "",
        kind="image",
        object_id=img.uuid or "",
        bounds=None,
        operations=[op],
        extras={
            "scale": op.payload.get("scale"),
            "image_format": op.payload.get("image_format"),
            "width_nm": op.payload.get("width_nm"),
            "height_nm": op.payload.get("height_nm"),
        },
    )


def _placement_transform(sym: "SchSymbol") -> KiCadPlotterTransform2D:
    """Build the SCH placement transform for a placed symbol.

    KiCad SCH placement composes: rotation by ``at_angle`` (around the
    symbol-local origin), then mirror per ``sym.mirror`` ("x" → flip Y
    axis = our ``mirror_x``; "y" → flip X axis = our ``mirror_y``),
    then translation to ``(at_x, at_y)``. Library coords have already
    been Y-flipped by :func:`lib_symbol_to_ir` so the transform here
    just rotates / mirrors / translates in the schematic's screen-Y
    frame.
    """
    return KiCadPlotterTransform2D(
        offset_x_nm=mm_to_nm(sym.at_x),
        offset_y_nm=mm_to_nm(sym.at_y),
        rotation_deg=-float(sym.at_angle),
        mirror_x=(sym.mirror == "x"),
        mirror_y=(sym.mirror == "y"),
    )


def _is_text_angle_horizontal(angle_deg: float) -> bool:
    return int(round(float(angle_deg))) % 180 == 0


def _flip_h_align(value: str) -> str:
    if value == KiCadHorizAlign.LEFT.value:
        return KiCadHorizAlign.RIGHT.value
    if value == KiCadHorizAlign.RIGHT.value:
        return KiCadHorizAlign.LEFT.value
    return value


def _flip_v_align(value: str) -> str:
    if value == KiCadVertAlign.TOP.value:
        return KiCadVertAlign.BOTTOM.value
    if value == KiCadVertAlign.BOTTOM.value:
        return KiCadVertAlign.TOP.value
    return value


def _apply_symbol_device_text_plot_attrs(
    payload: dict,
    *,
    sym: "SchSymbol",
    local_orient_deg: float,
) -> None:
    """Mirror KiCad's special SCH_TEXT::Plot branch for LAYER_DEVICE text."""
    x1, y1, x2, y2 = _symbol_transform_matrix(sym)
    orig_horiz = _is_text_angle_horizontal(local_orient_deg)
    screen_horiz = (x1 != 0) ^ (not orig_horiz)
    payload["orient_deg"] = 0.0 if screen_horiz else 90.0

    if orig_horiz:
        flip_h = (x1 < 0) if screen_horiz else (x2 > 0)
    else:
        flip_h = (y1 > 0) if screen_horiz else (y2 < 0)
    if flip_h:
        payload["h_align"] = _flip_h_align(
            str(payload.get("h_align", KiCadHorizAlign.LEFT.value))
        )

    det = x1 * y2 - x2 * y1
    if det < 0 and (orig_horiz == (x1 > 0)):
        payload["v_align"] = _flip_v_align(
            str(payload.get("v_align", KiCadVertAlign.BOTTOM.value))
        )


def _apply_symbol_body_transform_to_ops(
    ops: list[KiCadPlotterOp],
    sym: "SchSymbol",
) -> list[KiCadPlotterOp]:
    transform = _placement_transform(sym)
    out: list[KiCadPlotterOp] = []
    for op in ops:
        transformed = apply_transform_to_op(op, transform)
        kind = str(getattr(transformed.kind, "value", transformed.kind))
        if kind == KiCadPlotterOpKind.TEXT.value:
            _apply_symbol_device_text_plot_attrs(
                transformed.payload,
                sym=sym,
                local_orient_deg=float(op.payload.get("orient_deg", 0.0)),
            )
        out.append(transformed)
    return out


def _pin_root_local_nm(pin) -> tuple[int, int]:
    angle = int(round(float(pin.at_angle))) % 360
    pos_x = mm_to_nm(pin.at_x)
    pos_y = y_to_ir(pin.at_y)
    length_nm = mm_to_nm(pin.length)
    if angle == 0:
        return pos_x + length_nm, pos_y
    if angle == 180:
        return pos_x - length_nm, pos_y
    if angle == 90:
        return pos_x, pos_y - length_nm
    if angle == 270:
        return pos_x, pos_y + length_nm

    rad = math.radians(float(pin.at_angle))
    return (
        mm_to_nm(pin.at_x + pin.length * math.cos(rad)),
        y_to_ir(pin.at_y + pin.length * math.sin(rad)),
    )


def _text_kwargs_have_visible_size(kwargs: dict) -> bool:
    return min(
        abs(int(kwargs.get("size_x_nm") or 0)),
        abs(int(kwargs.get("size_y_nm") or 0)),
    ) > 0


def _selected_pin_name_and_style(pin, alternate_name: Optional[str]) -> tuple[str, object]:
    name = str(getattr(pin, "name", "") or "")
    graphic_style = getattr(pin, "graphic_style", "line")
    if not alternate_name:
        return name, graphic_style

    for alt in getattr(pin, "alternates", ()) or ():
        if str(getattr(alt, "name", "") or "") == alternate_name:
            return alternate_name, getattr(alt, "graphic_style", graphic_style)

    return alternate_name, graphic_style


def _placed_pin_to_ops_kicad_plot(
    pin,
    *,
    transform: KiCadPlotterTransform2D,
    pin_names_offset: float,
    pin_names_hide: bool,
    pin_numbers_hide: bool,
    default_line_width_nm: int | None = None,
    alternate_name: Optional[str] = None,
) -> list[KiCadPlotterOp]:
    if getattr(pin, "hide", False):
        return []

    pos_x, pos_y = transform_point(mm_to_nm(pin.at_x), y_to_ir(pin.at_y), transform)
    root_x, root_y = transform_point(*_pin_root_local_nm(pin), transform)

    ops: list[KiCadPlotterOp] = []
    name, graphic_style = _selected_pin_name_and_style(pin, alternate_name)
    ops.extend(
        pin_graphic_style_to_ops(
            start_x=root_x,
            start_y=root_y,
            end_x=pos_x,
            end_y=pos_y,
            graphic_style=graphic_style,
        )
    )

    local_direction_x, local_direction_y = _pin_direction_local_nm(pin)
    dir_x, dir_y = transform_point(
        mm_to_nm(pin.at_x) + local_direction_x,
        y_to_ir(pin.at_y) + local_direction_y,
        transform,
    )
    horizontal, pin_right, pin_down = _pin_direction_flags(
        dir_x - pos_x,
        dir_y - pos_y,
    )
    text_orient = 0.0 if horizontal else 90.0
    midpoint_x = (root_x + pos_x) // 2
    midpoint_y = (root_y + pos_y) // 2
    draws_name = bool(name and name != "~" and not pin_names_hide)

    if getattr(pin, "number", "") and not pin_numbers_hide:
        kwargs = _pin_number_text_kwargs(
            pin.number_effects,
            default_line_width_nm=default_line_width_nm,
        )
        kwargs["h_align"] = KiCadHorizAlign.CENTER
        kwargs["v_align"] = KiCadVertAlign.BOTTOM
        if _text_kwargs_have_visible_size(kwargs):
            text_clearance_nm = _pin_text_clearance_nm(kwargs)

            if pin_names_offset > 0 or not draws_name:
                num_x = midpoint_x if horizontal else root_x - text_clearance_nm
                num_y = root_y - text_clearance_nm if horizontal else midpoint_y
            elif horizontal:
                num_x = midpoint_x
                num_y = root_y + text_clearance_nm
                kwargs["v_align"] = KiCadVertAlign.TOP
            else:
                num_x = root_x + text_clearance_nm
                num_y = midpoint_y
                kwargs["v_align"] = KiCadVertAlign.TOP

            ops.append(
                KiCadPlotterOp.text(
                    x=num_x,
                    y=num_y,
                    text=pin.number,
                    orient_deg=text_orient,
                    **kwargs,
                )
            )

    if draws_name:
        kwargs = apply_default_text_style(
            _effects_to_text_kwargs(pin.name_effects),
            LAYER_PINNAM,
            clamp_pen_width=False,
        )
        if not _text_kwargs_have_visible_size(kwargs):
            return ops
        text_clearance_nm = _pin_text_clearance_nm(kwargs)

        if pin_names_offset > 0:
            offset_nm = mm_to_nm(pin_names_offset)
            if horizontal:
                name_x = root_x + offset_nm if pin_right else root_x - offset_nm
                name_y = root_y
                kwargs["h_align"] = (
                    KiCadHorizAlign.LEFT if pin_right else KiCadHorizAlign.RIGHT
                )
            else:
                name_x = root_x
                name_y = root_y + offset_nm if pin_down else root_y - offset_nm
                kwargs["h_align"] = (
                    KiCadHorizAlign.RIGHT if pin_down else KiCadHorizAlign.LEFT
                )
            kwargs["v_align"] = KiCadVertAlign.CENTER
        elif horizontal:
            name_x = midpoint_x
            name_y = root_y - text_clearance_nm
            kwargs["h_align"] = KiCadHorizAlign.CENTER
            kwargs["v_align"] = KiCadVertAlign.BOTTOM
        else:
            name_x = root_x - text_clearance_nm
            name_y = midpoint_y
            kwargs["h_align"] = KiCadHorizAlign.CENTER
            kwargs["v_align"] = KiCadVertAlign.BOTTOM

        ops.append(
            KiCadPlotterOp.text(
                x=name_x,
                y=name_y,
                text=name,
                orient_deg=text_orient,
                **kwargs,
            )
        )

    return ops


def _placed_symbol_pin_ops(
    sym: "SchSymbol",
    lib_sym: "LibSymbol",
    *,
    pin_block_factory: Optional[Callable[[Any], dict | None]] = None,
    default_line_width_nm: int | None = None,
) -> list[KiCadPlotterOp]:
    transform = _placement_transform(sym)
    operations: list[KiCadPlotterOp] = []
    selected_alternate_by_number = {
        str(getattr(pin, "number", "") or ""): str(getattr(pin, "alternate", "") or "")
        for pin in getattr(sym, "pins", ()) or ()
        if getattr(pin, "alternate", None)
    }
    for sub in _select_subsymbols(
        lib_sym.subsymbols,
        unit=int(sym.unit),
        style=int(sym.convert),
    ):
        for pin in sub.pins:
            pin_ops = _placed_pin_to_ops_kicad_plot(
                pin,
                transform=transform,
                pin_names_offset=lib_sym.pin_names_offset,
                pin_names_hide=lib_sym.pin_names_hide,
                pin_numbers_hide=lib_sym.pin_numbers_hide,
                default_line_width_nm=default_line_width_nm,
                alternate_name=selected_alternate_by_number.get(
                    str(getattr(pin, "number", "") or "")
                ),
            )
            if not pin_ops:
                continue
            block_kwargs = (
                pin_block_factory(pin) if pin_block_factory is not None else None
            )
            if block_kwargs:
                operations.append(KiCadPlotterOp.start_block(**block_kwargs))
                operations.extend(pin_ops)
                operations.append(KiCadPlotterOp.end_block())
            else:
                operations.extend(pin_ops)
    return operations


def _without_pin_blocks(ops: list[KiCadPlotterOp]) -> list[KiCadPlotterOp]:
    out: list[KiCadPlotterOp] = []
    skipping_pin_block = False
    for op in ops:
        kind = str(getattr(op.kind, "value", op.kind))
        if kind == "StartBlock" and (op.payload or {}).get("data_ref") == "symbol_pin":
            skipping_pin_block = True
            continue
        if skipping_pin_block:
            if kind == "EndBlock":
                skipping_pin_block = False
            continue
        out.append(op)
    return out


def _compose_symbol_body_and_pin_ops(
    sym: "SchSymbol",
    lib_sym: "LibSymbol",
    *,
    default_stroke_width_nm: int,
    default_polyline_stroke_width_nm: int,
    default_line_width_nm: int | None = None,
    project_vars: Optional[dict] = None,
) -> tuple[List[KiCadPlotterOp], List[KiCadPlotterOp]]:
    """
    Compose a placed symbol's body ops by feeding the library symbol
    through :func:`lib_symbol_to_ir` and re-anchoring the result via
    :func:`apply_transform_to_ops` at the SCH placement.

    Only ``lib_subsymbol`` records' ops are pulled — the leading
    ``lib_symbol`` header record carries metadata (no draw ops) and
    is dropped here.
    """
    placed_pin_uuid_by_number = {
        str(getattr(pin, "number", "") or ""): str(getattr(pin, "uuid", "") or "")
        for pin in getattr(sym, "pins", ()) or ()
    }
    seen_group_ids: dict[str, int] = {}

    def _pin_block_factory(pin) -> dict | None:
        pin_number = str(getattr(pin, "number", "") or "")
        source_pin_uuid = placed_pin_uuid_by_number.get(pin_number, "")
        group_id = schematic_pin_group_id(
            symbol_uuid=getattr(sym, "uuid", "") or "",
            pin_number=pin_number,
            source_pin_uuid=source_pin_uuid,
        )
        if not group_id:
            return None
        seen_count = seen_group_ids.get(group_id, 0)
        seen_group_ids[group_id] = seen_count + 1
        svg_group_id = (
            group_id if seen_count == 0 else f"{group_id}__{seen_count + 1}"
        )
        extra_attrs = {
            "primitive": "pin",
            "object-type": "pin",
            "pin": pin_number,
            "symbol-uuid": getattr(sym, "uuid", "") or "",
            "designator": getattr(sym, "reference", "") or "",
            "lib-pin-uuid": getattr(pin, "uuid", "") or "",
        }
        return {
            "label": svg_group_id,
            "data_uuid": svg_group_id,
            "data_ref": "symbol_pin",
            "object_id": source_pin_uuid or group_id,
            "extra_attrs": extra_attrs,
        }

    lib_doc = lib_symbol_to_ir(
        lib_sym,
        unit=int(sym.unit),
        style=int(sym.convert),
        default_stroke_width_nm=default_stroke_width_nm,
        default_polyline_stroke_width_nm=default_polyline_stroke_width_nm,
        pin_block_factory=_pin_block_factory,
        project_vars=project_vars,
    )
    body_ops: List[KiCadPlotterOp] = []
    for r in lib_doc.records:
        if r.kind == "lib_subsymbol":
            body_ops.extend(_without_pin_blocks(r.operations))
    placed_body_ops = _apply_symbol_body_transform_to_ops(body_ops, sym)
    seen_group_ids.clear()
    placed_pin_ops = _placed_symbol_pin_ops(
        sym,
        lib_sym,
        pin_block_factory=_pin_block_factory,
        default_line_width_nm=default_line_width_nm,
    )
    return placed_body_ops, placed_pin_ops


def _compose_symbol_body_ops(
    sym: "SchSymbol",
    lib_sym: "LibSymbol",
    *,
    default_stroke_width_nm: int,
    default_polyline_stroke_width_nm: int,
    default_line_width_nm: int | None = None,
    project_vars: Optional[dict] = None,
) -> List[KiCadPlotterOp]:
    body_ops, pin_ops = _compose_symbol_body_and_pin_ops(
        sym,
        lib_sym,
        default_stroke_width_nm=default_stroke_width_nm,
        default_polyline_stroke_width_nm=default_polyline_stroke_width_nm,
        default_line_width_nm=default_line_width_nm,
        project_vars=project_vars,
    )
    return body_ops + pin_ops


def _symbol_field_plot_orient_deg(prop, parent_symbol: "SchSymbol") -> float:
    orient = float(prop.at_angle)
    if int(round(float(parent_symbol.at_angle))) % 180 == 90:
        return 90.0 if int(round(orient)) % 180 == 0 else 0.0
    return orient


def _ki_rotate_xy_nm(x_nm: int, y_nm: int, angle_deg: float) -> tuple[int, int]:
    """Mirror KiCad's ``RotatePoint`` screen-coordinate rotation."""
    angle = int(round(float(angle_deg))) % 360
    if angle == 0:
        return x_nm, y_nm
    if angle == 90:
        return y_nm, -x_nm
    if angle == 180:
        return -x_nm, -y_nm
    if angle == 270:
        return -y_nm, x_nm

    rad = math.radians(float(angle_deg))
    sin_a = math.sin(rad)
    cos_a = math.cos(rad)
    return (
        _ki_round(y_nm * sin_a + x_nm * cos_a),
        _ki_round(y_nm * cos_a - x_nm * sin_a),
    )


def _symbol_transform_matrix(sym: "SchSymbol") -> tuple[int, int, int, int]:
    """Return KiCad ``TRANSFORM`` coefficients for a placed symbol."""
    angle = int(round(float(sym.at_angle))) % 360
    matrix = {
        0: (1, 0, 0, 1),
        90: (0, 1, -1, 0),
        180: (-1, 0, 0, -1),
        270: (0, -1, 1, 0),
    }.get(angle, (1, 0, 0, 1))

    mirror = getattr(sym, "mirror", None)
    if mirror == "x":
        matrix = _compose_symbol_transform(matrix, (1, 0, 0, -1))
    elif mirror == "y":
        matrix = _compose_symbol_transform(matrix, (-1, 0, 0, 1))
    return matrix


def _compose_symbol_transform(
    base: tuple[int, int, int, int],
    delta: tuple[int, int, int, int],
) -> tuple[int, int, int, int]:
    """Mirror ``SCH_SYMBOL::SetOrientation`` transform composition."""
    x1, y1, x2, y2 = base
    tx1, ty1, tx2, ty2 = delta
    return (
        x1 * tx1 + x2 * ty1,
        y1 * tx1 + y2 * ty1,
        x1 * tx2 + x2 * ty2,
        y1 * tx2 + y2 * ty2,
    )


def _apply_symbol_transform_nm(
    x_nm: int,
    y_nm: int,
    matrix: tuple[int, int, int, int],
) -> tuple[int, int]:
    x1, y1, x2, y2 = matrix
    return (x1 * x_nm + y1 * y_nm, x2 * x_nm + y2 * y_nm)


def _symbol_field_center_xy_nm(
    prop,
    parent_symbol: "SchSymbol",
    kwargs: dict,
    text: str,
) -> tuple[int, int]:
    x_nm = mm_to_nm(prop.at_x)
    y_nm = mm_to_nm(prop.at_y)

    h_justify = _label_horiz_justify(prop) or "center"
    v_justify = _label_vert_justify(prop) or "center"
    if h_justify == "center" and v_justify == "center":
        return x_nm, y_nm

    size_x_nm = int(kwargs.get("size_x_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    size_y_nm = int(kwargs.get("size_y_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    width_nm = _schematic_outline_text_width_nm(
        text,
        size_x_nm,
        bold=bool(kwargs.get("bold")),
        italic=bool(kwargs.get("italic", False)),
        font_face=str(kwargs.get("font_face", "") or ""),
    )
    if width_nm <= 0 and v_justify == "center":
        return x_nm, y_nm

    bold = bool(kwargs.get("bold"))
    italic = bool(kwargs.get("italic", False))
    font_face = str(kwargs.get("font_face", "") or "")
    center_voffset_nm = _symbol_field_outline_center_voffset_nm(
        size_y_nm,
        bold=bold,
        italic=italic,
        font_face=font_face,
    )
    v_extra_nm = _symbol_field_overbar_vjust_extra_nm(
        text,
        size_y_nm,
        bold=bold,
        italic=italic,
        font_face=font_face,
    )
    local_dx = 0
    if h_justify == "left":
        local_dx = _schematic_half_nm(width_nm)
    elif h_justify == "right":
        local_dx = -_schematic_half_nm(width_nm)

    local_dy = 0
    if v_justify == "top":
        local_dy = center_voffset_nm + v_extra_nm
    elif v_justify == "bottom":
        local_dy = -(center_voffset_nm + v_extra_nm)

    rotated_dx, rotated_dy = _ki_rotate_xy_nm(local_dx, local_dy, prop.at_angle)
    dx_nm, dy_nm = _apply_symbol_transform_nm(
        rotated_dx,
        rotated_dy,
        _symbol_transform_matrix(parent_symbol),
    )
    return x_nm + dx_nm, y_nm + dy_nm


def _resolve_symbol_property_value(prop, parent_symbol: Optional["SchSymbol"]) -> str:
    value = str(getattr(prop, "value", "") or "")
    if parent_symbol is None:
        return value
    if not (value.startswith("${") and value.endswith("}")):
        return value

    token = value[2:-1].strip().lower()
    for candidate in getattr(parent_symbol, "properties", ()) or ():
        key = str(getattr(candidate, "key", "") or "").strip().lower()
        if key == token:
            return str(getattr(candidate, "value", "") or "")
    return value


def _symbol_instance_reference(
    sym: "SchSymbol",
    sheet_instance_path: Optional[str],
) -> str:
    instances = getattr(sym, "instances", ()) or ()
    if sheet_instance_path:
        target = str(sheet_instance_path).rstrip("/")
        for inst in instances:
            inst_path = str(getattr(inst, "path", "") or "").rstrip("/")
            inst_ref = str(getattr(inst, "reference", "") or "")
            if inst_path == target and inst_ref:
                return inst_ref
    for inst in instances:
        inst_ref = str(getattr(inst, "reference", "") or "")
        if inst_ref:
            return inst_ref
    return str(getattr(sym, "reference", "") or "")


def _unit_letter_suffix(unit: int) -> str:
    if unit <= 0:
        return ""
    letters: list[str] = []
    value = unit
    while value > 0:
        value -= 1
        letters.append(chr(ord("A") + (value % 26)))
        value //= 26
    return "".join(reversed(letters))


def symbol_property_to_op(
    prop,
    parent_symbol: Optional["SchSymbol"] = None,
    *,
    default_line_width_nm: int | None = None,
    reference_unit_suffix: str = "",
    sheet_instance_path: Optional[str] = None,
) -> Optional[KiCadPlotterOp]:
    """Convert a visible :class:`SymProperty` to a ``Text`` op.

    Returns ``None`` for hidden properties (``hide=True``) or empty-
    valued properties (KiCad's ``SCH_FIELD::Plot`` skips fields with
    ``IsVisible() == false`` and treats blank values as nothing-to-
    draw). Coords are absolute schematic mm (already Y-down) — no
    flip, just ``mm_to_nm``.
    """
    if prop.hide:
        return None
    key = str(getattr(prop, "key", "") or "")
    value = _resolve_symbol_property_value(prop, parent_symbol)
    if parent_symbol is not None and key == "Reference":
        value = _symbol_instance_reference(parent_symbol, sheet_instance_path) or value
    value = "" if value is None else str(value)
    if (
        value
        and reference_unit_suffix
        and key == "Reference"
        and not value.endswith(reference_unit_suffix)
    ):
        value = f"{value}{reference_unit_suffix}"
    if value == "~":
        return None
    if getattr(prop, "show_name", False):
        value = f"{key}: {value or ''}"
    if not value:
        return None
    kwargs = _text_kwargs_with_plot_defaults(
        prop.effects,
        symbol_property_layer_color(key),
        default_line_width_nm=default_line_width_nm,
    )
    if "size_x_nm" not in kwargs or kwargs.get("size_x_nm", 0) == 0:
        kwargs["size_x_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
        kwargs["size_y_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    orient_deg = float(prop.at_angle)
    x_nm = mm_to_nm(prop.at_x)
    y_nm = mm_to_nm(prop.at_y)
    if parent_symbol is not None:
        # SCH_FIELD::Plot centers symbol-owned fields on their transformed
        # bounding box and flips horizontal/vertical angle when the parent
        # symbol transform swaps axes.
        orient_deg = _symbol_field_plot_orient_deg(prop, parent_symbol)
        kwargs["h_align"] = KiCadHorizAlign.CENTER
        kwargs["v_align"] = KiCadVertAlign.CENTER
        x_nm, y_nm = _symbol_field_center_xy_nm(prop, parent_symbol, kwargs, value)
    else:
        kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
        kwargs.setdefault("v_align", KiCadVertAlign.CENTER)
    return KiCadPlotterOp.text(
        x=x_nm,
        y=y_nm,
        text=value,
        orient_deg=orient_deg,
        **kwargs,
    )


def _symbol_instance_record(
    sym: "SchSymbol",
    lib_sym: Optional["LibSymbol"] = None,
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
    default_polyline_stroke_width_nm: int = DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM,
    default_line_width_nm: int | None = None,
    sheet_instance_path: Optional[str] = None,
    project_vars: Optional[dict] = None,
) -> KiCadPlotterRecord:
    """Header + composed body ops for a placed symbol.

    When ``lib_sym`` is provided, the symbol body is composed via
    :func:`_compose_symbol_body_ops`. Placements whose ``lib_id`` can't
    be resolved against the schematic's ``lib_symbols`` (rare in
    well-formed schematics) fall back to header-only (empty ops).
    """
    extras = {
        "lib_id": sym.lib_id,
        "lib_name": sym.lib_name,
        "reference": _symbol_instance_reference(sym, sheet_instance_path),
        "at_x_nm": mm_to_nm(sym.at_x),
        "at_y_nm": mm_to_nm(sym.at_y),
        "at_angle_deg": float(sym.at_angle),
        "mirror": sym.mirror,
        "unit": int(sym.unit),
        "convert": int(sym.convert),
        "in_bom": bool(sym.in_bom),
        "on_board": bool(sym.on_board),
        "dnp": bool(sym.dnp),
        "exclude_from_sim": bool(sym.exclude_from_sim),
        "in_pos_files": bool(sym.in_pos_files),
    }
    body_ops: List[KiCadPlotterOp] = []
    pin_ops: List[KiCadPlotterOp] = []
    if lib_sym is not None:
        body_ops, pin_ops = _compose_symbol_body_and_pin_ops(
            sym,
            lib_sym,
            default_stroke_width_nm=default_stroke_width_nm,
            default_polyline_stroke_width_nm=default_polyline_stroke_width_nm,
            default_line_width_nm=default_line_width_nm,
            project_vars=project_vars,
        )
    operations: List[KiCadPlotterOp] = body_ops + pin_ops
    reference_unit_suffix = (
        _unit_letter_suffix(int(sym.unit))
        if lib_sym is not None and getattr(lib_sym, "unit_count", 1) > 1
        else ""
    )
    # Visible symbol property fields (Reference / Value / Footprint /
    # user fields) — emitted in property declaration order, after the
    # body ops so callers see the body underneath the field labels.
    for prop in sym.properties:
        op = symbol_property_to_op(
            prop,
            parent_symbol=sym,
            default_line_width_nm=default_line_width_nm,
            reference_unit_suffix=reference_unit_suffix,
            sheet_instance_path=sheet_instance_path,
        )
        if op is not None:
            operations.append(op)
    if sym.dnp:
        operations = _dnp_dimmed_ops(operations)
        operations.extend(_symbol_dnp_marker_ops(sym, lib_sym, body_ops, pin_ops))
    return KiCadPlotterRecord(
        uuid=sym.uuid or "",
        kind="symbol_instance",
        object_id=sym.lib_id or sym.uuid or "",
        bounds=None,
        operations=operations,
        extras=extras,
    )


def _dnp_dimmed_ops(ops: list[KiCadPlotterOp]) -> list[KiCadPlotterOp]:
    dimmed: list[KiCadPlotterOp] = []
    for op in ops:
        payload = dict(op.payload or {})
        for key in ("stroke_color", "fill_color", "color"):
            if key in payload:
                payload[key] = _dnp_dimmed_color(payload[key])
        dimmed.append(KiCadPlotterOp(kind=op.kind, payload=payload))
    return dimmed


def _bbox_union(
    a: tuple[int, int, int, int] | None,
    b: tuple[int, int, int, int] | None,
) -> tuple[int, int, int, int] | None:
    if a is None:
        return b
    if b is None:
        return a
    return min(a[0], b[0]), min(a[1], b[1]), max(a[2], b[2]), max(a[3], b[3])


def _bbox_from_ops(ops: list[KiCadPlotterOp]) -> tuple[int, int, int, int] | None:
    bbox: tuple[int, int, int, int] | None = None
    for op in ops:
        bbox = _bbox_union(bbox, _op_bbox_nm(op))
    return bbox


def _placed_pin_root_bboxes(
    sym: "SchSymbol",
    lib_sym: Optional["LibSymbol"],
) -> list[tuple[int, int, int, int]]:
    if lib_sym is None:
        return []
    transform = _placement_transform(sym)
    out: list[tuple[int, int, int, int]] = []
    for sub in _select_subsymbols(
        lib_sym.subsymbols,
        unit=int(sym.unit),
        style=int(sym.convert),
    ):
        for pin in sub.pins:
            if getattr(pin, "hide", False):
                continue
            x_nm, y_nm = transform_point(*_pin_root_local_nm(pin), transform)
            out.append((x_nm, y_nm, x_nm, y_nm))
    return out


def _symbol_dnp_marker_ops(
    sym: "SchSymbol",
    lib_sym: Optional["LibSymbol"],
    body_ops: list[KiCadPlotterOp],
    pin_ops: list[KiCadPlotterOp],
) -> list[KiCadPlotterOp]:
    body_bbox = _bbox_from_ops(body_ops)
    for root_bbox in _placed_pin_root_bboxes(sym, lib_sym):
        body_bbox = _bbox_union(body_bbox, root_bbox)
    if body_bbox is None:
        return []

    pin_graphic_ops = [
        op
        for op in pin_ops
        if _kind_name(op) not in {"Text", "StartBlock", "EndBlock"}
    ]
    pins_bbox = _bbox_union(body_bbox, _bbox_from_ops(pin_graphic_ops))
    if pins_bbox is None:
        pins_bbox = body_bbox

    margin_x = max(body_bbox[0] - pins_bbox[0], pins_bbox[2] - body_bbox[2])
    margin_y = max(body_bbox[1] - pins_bbox[1], pins_bbox[3] - body_bbox[3])
    margin_x = max(margin_x * 0.6, margin_y * 0.3)
    margin_y = max(margin_y * 0.6, margin_x * 0.3)

    left = body_bbox[0] - _ki_round(margin_x)
    top = body_bbox[1] - _ki_round(margin_y)
    right = body_bbox[2] + _ki_round(margin_x)
    bottom = body_bbox[3] + _ki_round(margin_y)

    return [
        styled_plotter_op(
            KiCadPlotterOp.thick_segment(
                start_x=left,
                start_y=top,
                end_x=right,
                end_y=bottom,
                width_nm=DEFAULT_DNP_MARKER_STROKE_WIDTH_NM,
            ),
            stroke_color=LAYER_DNP_MARKER,
        ),
        styled_plotter_op(
            KiCadPlotterOp.thick_segment(
                start_x=right,
                start_y=top,
                end_x=left,
                end_y=bottom,
                width_nm=DEFAULT_DNP_MARKER_STROKE_WIDTH_NM,
            ),
            stroke_color=LAYER_DNP_MARKER,
        ),
    ]


def _kind_name(op: KiCadPlotterOp) -> str:
    return str(getattr(op.kind, "value", op.kind))


def _bbox_from_points(points: list[tuple[int, int]]) -> tuple[int, int, int, int] | None:
    if not points:
        return None
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    return min(xs), min(ys), max(xs), max(ys)


def _inflate_bbox(
    bbox: tuple[int, int, int, int] | None,
    amount_nm: int,
) -> tuple[int, int, int, int] | None:
    if bbox is None or amount_nm <= 0:
        return bbox
    return (
        bbox[0] - amount_nm,
        bbox[1] - amount_nm,
        bbox[2] + amount_nm,
        bbox[3] + amount_nm,
    )


def _op_bbox_nm(op: KiCadPlotterOp) -> tuple[int, int, int, int] | None:
    payload = op.payload or {}
    kind = _kind_name(op)
    bbox: tuple[int, int, int, int] | None = None

    if kind == "PlotPoly":
        bbox = _bbox_from_points(
            [(int(pt[0]), int(pt[1])) for pt in payload.get("points", [])]
        )
    elif kind == "Rect":
        x1 = int(payload.get("x1", 0) or 0)
        y1 = int(payload.get("y1", 0) or 0)
        x2 = int(payload.get("x2", 0) or 0)
        y2 = int(payload.get("y2", 0) or 0)
        bbox = (min(x1, x2), min(y1, y2), max(x1, x2), max(y1, y2))
    elif kind == "Circle":
        cx = int(payload.get("cx", 0) or 0)
        cy = int(payload.get("cy", 0) or 0)
        radius = int(payload.get("diameter_nm", 0) or 0) // 2
        bbox = (cx - radius, cy - radius, cx + radius, cy + radius)
    elif kind == "ArcThreePoint":
        bbox = _bbox_from_points(
            [
                (int(payload.get("start_x", 0) or 0), int(payload.get("start_y", 0) or 0)),
                (int(payload.get("mid_x", 0) or 0), int(payload.get("mid_y", 0) or 0)),
                (int(payload.get("end_x", 0) or 0), int(payload.get("end_y", 0) or 0)),
            ]
        )
    elif kind == "BezierCurve":
        bbox = _bbox_from_points(
            [
                (int(payload.get("start_x", 0) or 0), int(payload.get("start_y", 0) or 0)),
                (int(payload.get("ctrl1_x", 0) or 0), int(payload.get("ctrl1_y", 0) or 0)),
                (int(payload.get("ctrl2_x", 0) or 0), int(payload.get("ctrl2_y", 0) or 0)),
                (int(payload.get("end_x", 0) or 0), int(payload.get("end_y", 0) or 0)),
            ]
        )
    elif kind == "ThickSegment":
        bbox = _bbox_from_points(
            [
                (int(payload.get("start_x", 0) or 0), int(payload.get("start_y", 0) or 0)),
                (int(payload.get("end_x", 0) or 0), int(payload.get("end_y", 0) or 0)),
            ]
        )
    elif kind == "Text":
        x = int(payload.get("x", 0) or 0)
        y = int(payload.get("y", 0) or 0)
        size_x_nm = int(payload.get("size_x_nm", 0) or 0)
        size_y_nm = int(payload.get("size_y_nm", 0) or 0)
        width_nm = _schematic_outline_text_width_nm(
            str(payload.get("text", "") or ""),
            size_x_nm,
            bold=bool(payload.get("bold")),
            italic=bool(payload.get("italic", False)),
            font_face=str(payload.get("font_face", "") or ""),
        )
        orient = int(round(float(payload.get("orient_deg", 0.0) or 0.0))) % 180
        if orient == 90:
            bbox = (
                x - size_y_nm // 2,
                y - width_nm // 2,
                x + size_y_nm // 2,
                y + width_nm // 2,
            )
        else:
            bbox = (
                x - width_nm // 2,
                y - size_y_nm // 2,
                x + width_nm // 2,
                y + size_y_nm // 2,
            )

    width_nm = int(payload.get("width_nm", 0) or 0)
    return _inflate_bbox(bbox, width_nm // 2)


def _record_bbox_nm(record: KiCadPlotterRecord) -> tuple[int, int, int, int] | None:
    bboxes = [bbox for op in record.operations if (bbox := _op_bbox_nm(op)) is not None]
    if not bboxes:
        return None
    return (
        min(b[0] for b in bboxes),
        min(b[1] for b in bboxes),
        max(b[2] for b in bboxes),
        max(b[3] for b in bboxes),
    )


def _symbol_overlap_bbox_nm(
    record: KiCadPlotterRecord,
) -> tuple[int, int, int, int] | None:
    """Bounding box used for KiCad's symbol-overlap overplot decision.

    ``SCH_SCREEN::Plot`` queries ``SCH_SYMBOL::GetBoundingBox()``, which
    merges the library body, pin graphics, and visible fields.  It does not
    include pin name/number text, so skip text inside our ``symbol_pin``
    blocks while keeping the pin graphic ops in those same blocks.
    """
    bbox: tuple[int, int, int, int] | None = None
    in_pin_block = False
    for op in record.operations:
        kind = _kind_name(op)
        payload = op.payload or {}
        if kind == "StartBlock" and payload.get("data_ref") == "symbol_pin":
            in_pin_block = True
            continue
        if kind == "EndBlock" and in_pin_block:
            in_pin_block = False
            continue
        if in_pin_block and kind == "Text":
            continue
        bbox = _bbox_union(bbox, _op_bbox_nm(op))
    return bbox


def _bboxes_overlap(
    a: tuple[int, int, int, int] | None,
    b: tuple[int, int, int, int] | None,
) -> bool:
    if a is None or b is None:
        return False
    return not (a[2] < b[0] or b[2] < a[0] or a[3] < b[1] or b[3] < a[1])


def _overlapping_symbol_indices(records: list[KiCadPlotterRecord]) -> set[int]:
    bboxes = [_symbol_overlap_bbox_nm(record) for record in records]
    out: set[int] = set()
    for i, bbox in enumerate(bboxes):
        for j, other in enumerate(bboxes):
            if i == j:
                continue
            if _bboxes_overlap(bbox, other):
                out.add(i)
                break
    return out


def _symbol_overplot_record(
    sym: "SchSymbol",
    lib_sym: Optional["LibSymbol"],
    *,
    default_stroke_width_nm: int,
    default_polyline_stroke_width_nm: int,
    default_line_width_nm: int | None = None,
    sheet_instance_path: Optional[str] = None,
    project_vars: Optional[dict] = None,
) -> Optional[KiCadPlotterRecord]:
    if lib_sym is None:
        return None

    body_ops, pin_ops = _compose_symbol_body_and_pin_ops(
        sym,
        lib_sym,
        default_stroke_width_nm=default_stroke_width_nm,
        default_polyline_stroke_width_nm=default_polyline_stroke_width_nm,
        default_line_width_nm=default_line_width_nm,
        project_vars=project_vars,
    )
    operations: List[KiCadPlotterOp] = []
    reference_unit_suffix = (
        _unit_letter_suffix(int(sym.unit))
        if getattr(lib_sym, "unit_count", 1) > 1
        else ""
    )
    for prop in sym.properties:
        op = symbol_property_to_op(
            prop,
            parent_symbol=sym,
            default_line_width_nm=default_line_width_nm,
            reference_unit_suffix=reference_unit_suffix,
            sheet_instance_path=sheet_instance_path,
        )
        if op is not None:
            operations.append(op)

    operations.extend(pin_ops)

    if sym.dnp:
        operations = _dnp_dimmed_ops(operations)
        operations.extend(_symbol_dnp_marker_ops(sym, lib_sym, body_ops, pin_ops))

    if not operations:
        return None
    return KiCadPlotterRecord(
        uuid=(sym.uuid or "") + ":overplot",
        kind="symbol_overplot",
        object_id=sym.lib_id or sym.uuid or "",
        bounds=None,
        operations=operations,
        extras={"source_symbol_uuid": sym.uuid or "", "lib_id": sym.lib_id},
    )


def sheet_outline_to_op(sheet: "SchSheet") -> KiCadPlotterOp:
    """Convert a :class:`SchSheet`'s rectangle to a ``Rect`` op (border).

    Mirrors ``SCH_SHEET::Plot`` (eeschema/sch_sheet.cpp:1560): NO_FILL
    rect from ``m_pos`` to ``m_pos + m_size`` with the sheet's
    effective pen width. Stroke width falls back to the wire-default
    when the parsed ``Stroke`` is missing or zero-width (matches
    ``GetEffectivePenWidth`` ⟶ ``DEFAULT_LINE_WIDTH_MILS``).
    """
    line_style, width_nm, color = _resolve_stroke(
        sheet.stroke, DEFAULT_WIRE_WIDTH_MM, LAYER_SHEET
    )
    return styled_plotter_op(
        KiCadPlotterOp.rect(
            x1=mm_to_nm(sheet.at_x),
            y1=mm_to_nm(sheet.at_y),
            x2=mm_to_nm(sheet.at_x + sheet.size_x),
            y2=mm_to_nm(sheet.at_y + sheet.size_y),
            fill=KiCadFillType.NO_FILL,
            width_nm=width_nm,
        ),
        stroke_color=color,
        line_style=line_style,
    )


def sheet_background_to_op(sheet: "SchSheet") -> Optional[KiCadPlotterOp]:
    """Optional ``Rect`` op for a sheet's background fill.

    Mirrors the background pass in ``SCH_SHEET::Plot`` (sch_sheet.cpp:
    1550-1554): when ``backgroundColor.a > 0`` the plotter emits a
    ``FILLED_SHAPE`` rect over ``m_pos..m_pos+m_size`` before the
    border. Returns ``None`` when the parsed sheet has no
    ``fill_color`` or its alpha is zero. The emitted op is meant to
    precede :func:`sheet_outline_to_op` in the record's op list so
    downstream renderers paint background → border in the right
    z-order.
    """
    fill = sheet.fill_color
    if fill is None:
        return None
    # ``fill_color`` is ``(R, G, B, A)`` with A in [0, 1] (see
    # SchSheet.from_sexp). Treat A==0 as "no background".
    try:
        alpha = float(fill[3])
    except (IndexError, TypeError, ValueError):
        alpha = 1.0
    if alpha <= 0.0:
        return None
    fill_color = rgba_to_hex(fill)
    return styled_plotter_op(
        KiCadPlotterOp.rect(
            x1=mm_to_nm(sheet.at_x),
            y1=mm_to_nm(sheet.at_y),
            x2=mm_to_nm(sheet.at_x + sheet.size_x),
            y2=mm_to_nm(sheet.at_y + sheet.size_y),
            fill=KiCadFillType.FILLED_SHAPE,
            width_nm=0,
        ),
        stroke_color=fill_color,
        fill_color=fill_color,
    )


def sheet_property_to_op(prop) -> Optional[KiCadPlotterOp]:
    """Convert a visible :class:`SchSheetProperty` to a ``Text`` op.

    Skip rules mirror :func:`symbol_property_to_op` (and KiCad's
    ``SCH_FIELD::Plot``): ``hide=True`` → drop, empty ``value`` →
    drop. Used to emit ``Sheetname`` / ``Sheetfile`` / custom sheet
    fields. Coords are absolute schematic mm (no Y-flip).
    """
    if getattr(prop, "hide", False):
        return None
    key = str(getattr(prop, "key", "") or "")
    value = str(getattr(prop, "value", "") or "")
    if not value and not getattr(prop, "show_name", False):
        return None
    kwargs = apply_default_text_style(
        _effects_to_text_kwargs(prop.effects),
        sheet_property_layer_color(key),
    )
    if "size_x_nm" not in kwargs or kwargs.get("size_x_nm", 0) == 0:
        kwargs["size_x_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
        kwargs["size_y_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
    kwargs.setdefault("v_align", KiCadVertAlign.CENTER)
    text = value
    if getattr(prop, "show_name", False):
        text = f"{key}: {text}"
    elif key in ("Sheetfile", "Sheet file"):
        text = f"File: {text}"
    if not text:
        return None
    return KiCadPlotterOp.text(
        x=mm_to_nm(prop.at_x),
        y=mm_to_nm(prop.at_y),
        text=text,
        orient_deg=float(prop.at_angle),
        **kwargs,
    )


def sheet_pin_to_op(
    pin,
    *,
    default_line_width_nm: int | None = None,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
) -> KiCadPlotterOp:
    """Convert a :class:`SchSheetPin` to a ``Text`` op (body only).

    Sheet pins carry their label content under ``.name`` (not
    ``.text`` like the schematic label classes). KiCad stores sheet
    pins at the sheet-edge anchor but plots the readable text outward
    from that anchor by ``GetTextOffset() + GetTextWidth()``.
    """
    kwargs = _sheet_pin_text_kwargs(
        pin,
        default_line_width_nm=default_line_width_nm,
    )
    if "size_x_nm" not in kwargs or kwargs.get("size_x_nm", 0) == 0:
        kwargs["size_x_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
        kwargs["size_y_nm"] = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    kwargs["v_align"] = KiCadVertAlign.CENTER

    x_nm = mm_to_nm(pin.at_x)
    y_nm = mm_to_nm(pin.at_y)
    size_x_nm = int(kwargs.get("size_x_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    size_y_nm = int(kwargs.get("size_y_nm") or mm_to_nm(DEFAULT_TEXT_SIZE_MM))
    dist_nm = _ki_round(float(text_offset_ratio) * size_y_nm) + size_x_nm
    spin_idx = _sheet_pin_at_angle_to_spin_idx(pin.at_angle)
    if spin_idx == 0:      # LEFT
        x_nm -= dist_nm
    elif spin_idx == 1:    # UP
        y_nm -= dist_nm
    elif spin_idx == 3:    # BOTTOM
        y_nm += dist_nm
    else:                  # RIGHT
        x_nm += dist_nm

    return KiCadPlotterOp.text(
        x=x_nm,
        y=y_nm,
        text=_plot_display_text(pin.name),
        orient_deg=90.0 if spin_idx in (1, 3) else 0.0,
        **kwargs,
    )


def _sheet_record(
    sheet: "SchSheet",
    *,
    default_line_width_nm: int | None = None,
    text_offset_ratio: float = DEFAULT_TEXT_OFFSET_RATIO,
) -> KiCadPlotterRecord:
    """Composed record for a hierarchical sheet.

    Op order mirrors ``SCH_SHEET::Plot``:
    optional background fill → border rect → sheet pins → property
    fields (Sheetname / Sheetfile / custom). ``extras`` keeps the
    rectangle metadata so callers can still inspect the sheet box
    without walking the ops.
    """
    extras = {
        "sheet_name": sheet.sheet_name,
        "sheet_file": sheet.sheet_file,
        "at_x_nm": mm_to_nm(sheet.at_x),
        "at_y_nm": mm_to_nm(sheet.at_y),
        "size_x_nm": mm_to_nm(sheet.size_x),
        "size_y_nm": mm_to_nm(sheet.size_y),
    }

    operations: List[KiCadPlotterOp] = []
    bg_op = sheet_background_to_op(sheet)
    if bg_op is not None:
        operations.append(bg_op)
    outline_op = sheet_outline_to_op(sheet)
    operations.append(outline_op)
    if bg_op is None:
        # KiCad plots sheets in two passes. Transparent sheet fills skip
        # only the background fill; the border is still emitted in both
        # the background and foreground passes.
        operations.append(outline_op)
    for sp in sheet.pins:
        pin_ops: List[KiCadPlotterOp] = [
            sheet_pin_to_op(
                sp,
                default_line_width_nm=default_line_width_nm,
                text_offset_ratio=text_offset_ratio,
            )
        ]
        deco = sheet_pin_decoration_to_op(
            sp,
            default_line_width_nm=default_line_width_nm,
        )
        if deco is not None:
            pin_ops.append(deco)
        group_id = schematic_sheet_pin_group_id(
            sheet_uuid=getattr(sheet, "uuid", "") or "",
            pin_name=getattr(sp, "name", "") or "",
            source_pin_uuid=getattr(sp, "uuid", "") or "",
        )
        if group_id:
            shape_obj = getattr(sp, "shape", None)
            shape = getattr(shape_obj, "value", shape_obj)
            operations.append(
                KiCadPlotterOp.start_block(
                    label=group_id,
                    data_uuid=group_id,
                    data_ref="sheet_pin",
                    object_id=getattr(sp, "uuid", "") or group_id,
                    extra_attrs={
                        "primitive": "sheet-entry",
                        "object-type": "sheet-pin",
                        "sheet-uuid": getattr(sheet, "uuid", "") or "",
                        "sheet-name": getattr(sheet, "sheet_name", "") or "",
                        "sheet-file": getattr(sheet, "sheet_file", "") or "",
                        "pin": getattr(sp, "name", "") or "",
                        "pin-name": getattr(sp, "name", "") or "",
                        "shape": shape,
                    },
                )
            )
            operations.extend(pin_ops)
            operations.append(KiCadPlotterOp.end_block())
        else:
            operations.extend(pin_ops)
    for prop in sheet.properties:
        op = sheet_property_to_op(prop)
        if op is not None:
            operations.append(op)

    return KiCadPlotterRecord(
        uuid=getattr(sheet, "uuid", "") or "",
        kind="sheet",
        object_id=sheet.sheet_name or "",
        bounds=None,
        operations=operations,
        extras=extras,
    )


def _netclass_flag_record(
    flag: "SchNetclassFlag",
    *,
    default_line_width_nm: int | None = None,
) -> KiCadPlotterRecord:
    """Composed record for directive/netclass flag property fields."""
    operations: List[KiCadPlotterOp] = []
    for prop in getattr(flag, "properties", ()) or ():
        op = symbol_property_to_op(
            prop,
            default_line_width_nm=default_line_width_nm,
        )
        if op is not None:
            operations.append(op)

    return KiCadPlotterRecord(
        uuid=getattr(flag, "uuid", "") or "",
        kind="netclass_flag",
        object_id=getattr(flag, "text", "") or getattr(flag, "uuid", "") or "",
        bounds=None,
        operations=operations,
        extras={
            "at_x_nm": mm_to_nm(getattr(flag, "at_x", 0.0)),
            "at_y_nm": mm_to_nm(getattr(flag, "at_y", 0.0)),
            "shape": str(getattr(flag, "shape", "") or ""),
            "length_nm": mm_to_nm(getattr(flag, "length", 0.0)),
        },
    )


def _table_record(
    table,
    *,
    project_vars: Optional[dict] = None,
) -> KiCadPlotterRecord:
    """Composed record for schematic tables."""
    operations: List[KiCadPlotterOp] = []
    for cell in getattr(table, "cells", ()) or ():
        operations.extend(text_box_to_ops(cell, project_vars=project_vars))

    return KiCadPlotterRecord(
        uuid=getattr(table, "uuid", "") or "",
        kind="table",
        object_id=getattr(table, "uuid", "") or "",
        bounds=None,
        operations=operations,
        extras={"cell_count": len(getattr(table, "cells", ()) or ())},
    )


def _sheet_header_record(
    sch: "KiCadSchematic",
    *,
    source_path: Optional[str] = None,
    sheet_index: int = 1,
    sheet_count: int = 1,
    sheet_path: str = "/",
    sheet_name: str = "",
    project_vars: Optional[dict] = None,
) -> KiCadPlotterRecord:
    """One leading record per document — paper + title block + identity.

    ``operations`` carries the drawing-sheet ops (border rects, tick marks,
    tick labels, title-block fields) emitted from KiCad's default page-layout
    template by :func:`drawing_sheet_to_ops`. The default template is loaded
    once and shared.
    """
    width_nm, height_nm = paper_size_to_nm(sch.paper)
    extras: dict = {
        "paper_size": sch.paper.size,
        "paper_width_mm": sch.paper.width,
        "paper_height_mm": sch.paper.height,
        "paper_portrait": bool(sch.paper.portrait),
        "sheet_width_nm": width_nm,
        "sheet_height_nm": height_nm,
        "version": int(sch.version),
        "generator": sch.generator,
        "generator_version": sch.generator_version,
    }
    title_block_dict: Optional[dict] = None
    if sch.title_block is not None:
        tb = sch.title_block
        title_block_dict = {
            "title": tb.title or "",
            "date": tb.date or "",
            "rev": tb.rev or "",
            "company": tb.company or "",
            "comments": dict(tb.comments) if tb.comments else {},
        }
        extras["title_block"] = title_block_dict

    # Compose the drawing-sheet ops from the default template. Lazy
    # imports keep the per-element record emitters free of the wks
    # parser dep when callers don't go through schematic_to_ir.
    from .kicad_drawing_sheet import (
        drawing_sheet_to_ops,
    )

    filename = ""
    if source_path:
        from pathlib import Path
        filename = Path(source_path).name

    wks = _project_worksheet_for_schematic(sch, source_path)
    operations = [
        styled_plotter_op(
            KiCadPlotterOp.rect(
                x1=0,
                y1=0,
                x2=width_nm,
                y2=height_nm,
                fill=KiCadFillType.FILLED_SHAPE,
                width_nm=100,
            ),
            stroke_color=LAYER_SCHEMATIC_BACKGROUND,
            fill_color=LAYER_SCHEMATIC_BACKGROUND,
        )
    ]
    # KiCad leaves ${SHEETNAME} empty on the virtual root sheet; named
    # hierarchical sheets resolve from the parent sheet placement.
    drawing_sheet_name = "" if sheet_path == "/" else sheet_name
    operations.extend(drawing_sheet_to_ops(
        wks,
        paper_width_nm=width_nm,
        paper_height_nm=height_nm,
        title_block=title_block_dict,
        sheet_index=sheet_index,
        sheet_count=sheet_count,
        paper_name=sch.paper.size or "",
        filename=filename,
        sheet_path=sheet_path,
        sheet_name=drawing_sheet_name,
        kicad_version=DEFAULT_KICAD_DRAWING_SHEET_VERSION_TEXT,
        project_vars=project_vars,
    ))

    return KiCadPlotterRecord(
        uuid=sch.uuid or "",
        kind="sheet_header",
        object_id=sch.uuid or "",
        bounds=None,
        operations=operations,
        extras=extras,
    )


def _parse_positive_page_number(page: object) -> Optional[int]:
    try:
        value = int(str(page).strip())
    except (TypeError, ValueError):
        return None
    if value <= 0:
        return None
    return value


def _infer_sheet_count(schematic: "KiCadSchematic") -> int:
    """Infer total hierarchy pages from parsed KiCad instance metadata."""
    pages = [
        parsed for inst in getattr(schematic, "sheet_instances", ()) or ()
        if (parsed := _parse_positive_page_number(getattr(inst, "page", "")))
        is not None
    ]
    for sheet in getattr(schematic, "sheets", ()) or ():
        for inst in getattr(sheet, "instances", ()) or ():
            parsed = _parse_positive_page_number(getattr(inst, "page", ""))
            if parsed is not None:
                pages.append(parsed)
    return max(pages, default=1)


# ---------------------------------------------------------------------------
# Top-level converter
# ---------------------------------------------------------------------------


def _schematic_ir_context(
    schematic: "KiCadSchematic",
    *,
    source_path: Optional[str],
    sheet_index: int,
    sheet_count: int,
    sheet_instance_path: Optional[str],
    project_vars: Optional[dict],
) -> tuple[float, int, int, Optional[str], dict[str, str]]:
    drawing_settings = _schematic_project_drawing_settings(schematic, source_path)
    text_offset_ratio = _drawing_setting_float(
        drawing_settings,
        "text_offset_ratio",
        DEFAULT_TEXT_OFFSET_RATIO,
    )
    default_line_width_nm = _drawing_default_line_width_nm(drawing_settings)
    inferred_project_vars = _schematic_project_text_variables(schematic, source_path)
    explicit_project_vars = (
        {str(k): str(v) for k, v in project_vars.items()} if project_vars else {}
    )
    project_sheet_count = _schematic_project_sheet_count(schematic, source_path)
    if sheet_instance_path is None and getattr(schematic, "uuid", ""):
        sheet_instance_path = "/" + str(schematic.uuid)
    if project_sheet_count is not None:
        sheet_count = project_sheet_count
    else:
        sheet_count = max(sheet_count, _infer_sheet_count(schematic))
    expansion_project_vars = dict(inferred_project_vars)
    expansion_project_vars.update(explicit_project_vars)
    builtin_project_vars = _schematic_builtin_text_variables(
        schematic,
        sheet_index=sheet_index,
        sheet_count=sheet_count,
        project_vars=expansion_project_vars,
    )
    effective_project_vars = dict(expansion_project_vars)
    # Built-in sheet/title variables are resolved by KiCad before falling
    # back to project text variables, so names such as VARIANT/TITLE win here.
    effective_project_vars.update(builtin_project_vars)
    return (
        text_offset_ratio,
        default_line_width_nm,
        sheet_count,
        sheet_instance_path,
        effective_project_vars,
    )


def _append_schematic_connectivity_records(
    records: List[KiCadPlotterRecord],
    schematic: "KiCadSchematic",
    *,
    default_line_width_nm: int,
) -> None:
    for w in schematic.wires:
        rec = _wire_record(w)
        if rec is not None:
            records.append(rec)
    for b in schematic.buses:
        rec = _bus_record(b)
        if rec is not None:
            records.append(rec)
    for be in schematic.bus_entries:
        records.append(_bus_entry_record(be))
    for j in schematic.junctions:
        records.append(_junction_record(j))
    for nc in schematic.no_connects:
        records.append(
            _no_connect_record(nc, default_line_width_nm=default_line_width_nm)
        )


def _append_schematic_label_records(
    records: List[KiCadPlotterRecord],
    schematic: "KiCadSchematic",
    *,
    default_line_width_nm: int,
    text_offset_ratio: float,
) -> None:
    for lbl in schematic.labels:
        records.append(
            _label_record(
                lbl,
                "label",
                lambda item: label_to_op(
                    item,
                    text_offset_ratio=text_offset_ratio,
                    default_line_width_nm=default_line_width_nm,
                ),
            )
        )
    for lbl in schematic.global_labels:
        records.append(
            _label_record(
                lbl,
                "global_label",
                lambda item: global_label_to_op(
                    item,
                    text_offset_ratio=text_offset_ratio,
                    default_line_width_nm=default_line_width_nm,
                ),
                decoration_fn=global_label_decoration_to_op,
            )
        )
    for lbl in schematic.hierarchical_labels:
        records.append(
            _label_record(
                lbl,
                "hierarchical_label",
                lambda item: hierarchical_label_to_op(
                    item,
                    text_offset_ratio=text_offset_ratio,
                    default_line_width_nm=default_line_width_nm,
                ),
                decoration_fn=hierarchical_label_decoration_to_op,
            )
        )


def _append_schematic_text_records(
    records: List[KiCadPlotterRecord],
    schematic: "KiCadSchematic",
    *,
    default_line_width_nm: int,
    project_vars: dict[str, str],
) -> None:
    for flag in getattr(schematic, "netclass_flags", ()) or ():
        records.append(
            _netclass_flag_record(flag, default_line_width_nm=default_line_width_nm)
        )
    for t in schematic.texts:
        records.append(
            _text_record(
                t,
                default_line_width_nm=default_line_width_nm,
                project_vars=project_vars,
            )
        )
    for tb in getattr(schematic, "text_boxes", ()) or ():
        records.append(_text_box_record(tb, project_vars=project_vars))


def _append_schematic_graphics_records(
    records: List[KiCadPlotterRecord],
    schematic: "KiCadSchematic",
    *,
    project_vars: dict[str, str],
) -> None:
    for poly in getattr(schematic, "polylines", ()) or ():
        rec = _graphic_polyline_record(poly)
        if rec is not None:
            records.append(rec)
    for arc in getattr(schematic, "arcs", ()) or ():
        rec = _graphic_arc_record(arc)
        if rec is not None:
            records.append(rec)
    for circle in getattr(schematic, "circles", ()) or ():
        rec = _graphic_circle_record(circle)
        if rec is not None:
            records.append(rec)
    for rect in getattr(schematic, "rectangles", ()) or ():
        rec = _graphic_rectangle_record(rect)
        if rec is not None:
            records.append(rec)
    for bez in getattr(schematic, "beziers", ()) or ():
        rec = _graphic_bezier_record(bez)
        if rec is not None:
            records.append(rec)
    for img in getattr(schematic, "images", ()) or ():
        records.append(_image_record(img))
    for table in getattr(schematic, "tables", ()) or ():
        records.append(_table_record(table, project_vars=project_vars))


def _append_schematic_symbol_records(
    records: List[KiCadPlotterRecord],
    schematic: "KiCadSchematic",
    *,
    default_line_width_nm: int,
    sheet_instance_path: Optional[str],
    project_vars: dict[str, str],
) -> None:
    symbol_default_stroke_width_nm = _symbol_body_stroke_width_for_schematic(schematic)
    symbol_default_polyline_stroke_width_nm = (
        _symbol_polyline_stroke_width_for_schematic(schematic)
    )
    symbol_entries: list[tuple["SchSymbol", Optional["LibSymbol"], KiCadPlotterRecord]] = []
    for sym in schematic.symbols:
        if hasattr(schematic, "get_lib_symbol_for_symbol"):
            lib_sym = schematic.get_lib_symbol_for_symbol(sym)
        else:
            lib_sym = schematic.get_lib_symbol(sym.lib_id) if sym.lib_id else None
        record = _symbol_instance_record(
            sym,
            lib_sym,
            default_stroke_width_nm=symbol_default_stroke_width_nm,
            default_polyline_stroke_width_nm=symbol_default_polyline_stroke_width_nm,
            default_line_width_nm=default_line_width_nm,
            sheet_instance_path=sheet_instance_path,
            project_vars=project_vars,
        )
        symbol_entries.append((sym, lib_sym, record))
        records.append(record)

    symbol_records = [entry[2] for entry in symbol_entries]
    for idx in sorted(_overlapping_symbol_indices(symbol_records)):
        sym, lib_sym, _record = symbol_entries[idx]
        overplot = _symbol_overplot_record(
            sym,
            lib_sym,
            default_stroke_width_nm=symbol_default_stroke_width_nm,
            default_polyline_stroke_width_nm=symbol_default_polyline_stroke_width_nm,
            default_line_width_nm=default_line_width_nm,
            sheet_instance_path=sheet_instance_path,
            project_vars=project_vars,
        )
        if overplot is not None:
            records.append(overplot)


def _append_schematic_sheet_records(
    records: List[KiCadPlotterRecord],
    schematic: "KiCadSchematic",
    *,
    default_line_width_nm: int,
    text_offset_ratio: float,
) -> None:
    for sh in schematic.sheets:
        records.append(
            _sheet_record(
                sh,
                default_line_width_nm=default_line_width_nm,
                text_offset_ratio=text_offset_ratio,
            )
        )


def schematic_to_ir(
    schematic: "KiCadSchematic",
    *,
    source_path: Optional[str] = None,
    document_id: Optional[str] = None,
    sheet_index: int = 1,
    sheet_count: int = 1,
    sheet_path: str = "/",
    sheet_instance_path: Optional[str] = None,
    sheet_name: str = "",
    project_vars: Optional[dict] = None,
) -> KiCadPlotterDocument:
    """
    Convert a :class:`KiCadSchematic` to a :class:`KiCadPlotterDocument`.

    Emits one ``sheet_header`` record (paper + title block) followed
    by per-element records in KiCad's natural emit order:
      wires → buses → bus_entries → junctions → no_connects →
      labels (local / global / hierarchical) → texts →
      symbol instances → hierarchical sheets.

    Coordinates are KiCad internal-unit nm; the Y axis is "down" (no
    flip — schematic file coords are already screen-Y).
    """
    _register_embedded_fonts_for_schematic(schematic, source_path)
    (
        text_offset_ratio,
        default_line_width_nm,
        sheet_count,
        sheet_instance_path,
        effective_project_vars,
    ) = _schematic_ir_context(
        schematic,
        source_path=source_path,
        sheet_index=sheet_index,
        sheet_count=sheet_count,
        sheet_instance_path=sheet_instance_path,
        project_vars=project_vars,
    )
    records: List[KiCadPlotterRecord] = [_sheet_header_record(
        schematic,
        source_path=source_path,
        sheet_index=sheet_index,
        sheet_count=sheet_count,
        sheet_path=sheet_path,
        sheet_name=sheet_name,
        project_vars=effective_project_vars,
    )]

    _append_schematic_connectivity_records(
        records,
        schematic,
        default_line_width_nm=default_line_width_nm,
    )
    _append_schematic_label_records(
        records,
        schematic,
        default_line_width_nm=default_line_width_nm,
        text_offset_ratio=text_offset_ratio,
    )
    _append_schematic_text_records(
        records,
        schematic,
        default_line_width_nm=default_line_width_nm,
        project_vars=effective_project_vars,
    )
    _append_schematic_graphics_records(
        records,
        schematic,
        project_vars=effective_project_vars,
    )
    _append_schematic_symbol_records(
        records,
        schematic,
        default_line_width_nm=default_line_width_nm,
        sheet_instance_path=sheet_instance_path,
        project_vars=effective_project_vars,
    )
    _append_schematic_sheet_records(
        records,
        schematic,
        default_line_width_nm=default_line_width_nm,
        text_offset_ratio=text_offset_ratio,
    )

    width_nm, height_nm = paper_size_to_nm(schematic.paper)

    return KiCadPlotterDocument(
        records=records,
        source_path=source_path,
        source_kind="SCH",
        document_id=document_id or schematic.uuid or None,
        canvas={"width_nm": width_nm, "height_nm": height_nm},
        coordinate_space={"unit": "nm", "y_axis": "down"},
        background_color=None,
        render_hints=None,
        extras={},
    )


__all__ = [
    "DEFAULT_BUS_WIDTH_MM",
    "DEFAULT_JUNCTION_DIAMETER_MM",
    "DEFAULT_LABEL_SIZE_RATIO",
    "DEFAULT_NO_CONNECT_HALF_MM",
    "DEFAULT_TEXT_SIZE_MM",
    "DEFAULT_WIRE_WIDTH_MM",
    "bus_entry_to_op",
    "bus_to_op",
    "global_label_decoration_to_op",
    "global_label_to_op",
    "hierarchical_label_decoration_to_op",
    "hierarchical_label_to_op",
    "junction_to_op",
    "label_to_op",
    "no_connect_to_ops",
    "paper_size_to_nm",
    "sch_text_to_op",
    "schematic_arc_to_ops",
    "schematic_bezier_to_ops",
    "schematic_circle_to_ops",
    "schematic_image_to_op",
    "schematic_polyline_to_ops",
    "schematic_rectangle_to_ops",
    "schematic_to_ir",
    "symbol_property_to_op",
    "text_box_outline_to_op",
    "text_box_to_ops",
    "wire_to_op",
]

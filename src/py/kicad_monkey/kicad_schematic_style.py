"""KiCad schematic plot-style defaults shared by IR emitters."""

from __future__ import annotations

from collections.abc import Mapping
import re
from typing import Any

DEFAULT_SCHEMATIC_FONT_FACE = "Arial"
DEFAULT_MIN_PLOT_PEN_WIDTH_NM = 84_700
DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM = 152400
DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM = DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM
DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM = DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM
DEFAULT_KICAD_DRAWING_SHEET_VERSION_TEXT = "KiCad E.D.A. 10.0.0-912-gf11d3da677-dirty"
DEFAULT_PIN_SYMBOL_RADIUS_NM = 635000

# KiCad default schematic color theme values from
# common/settings/builtin_color_themes.h. Store alpha-bearing hex so recorder
# dumps and declarative IR compare without losing opacity.
LAYER_BUS = "#000084FF"
LAYER_BUS_JUNCTION = "#000084FF"
LAYER_DEVICE = "#840000FF"
LAYER_DEVICE_BACKGROUND = "#FFFFC2FF"
LAYER_DNP_MARKER = "#DC090DD9"
LAYER_FIELDS = "#840084FF"
LAYER_GLOBLABEL = "#840000FF"
LAYER_HIERLABEL = "#725600FF"
LAYER_JUNCTION = "#009600FF"
LAYER_LOCLABEL = "#0F0F0FFF"
LAYER_NETCLASS_REFS = "#484848FF"
LAYER_NOCONNECT = "#000084FF"
LAYER_NOTES = "#0000C2FF"
LAYER_PIN = "#840000FF"
LAYER_PINNAM = "#006464FF"
LAYER_PINNUM = "#A90000FF"
LAYER_REFERENCEPART = "#006464FF"
LAYER_SCHEMATIC_DRAWINGSHEET = "#840000FF"
LAYER_SCHEMATIC_BACKGROUND = "#F5F4EFFF"
LAYER_SHEET = "#840000FF"
LAYER_SHEET_BACKGROUND = "#FFFFFF00"
LAYER_SHEETFIELDS = "#840084FF"
LAYER_SHEETFILENAME = "#725600FF"
LAYER_SHEETLABEL = "#006464FF"
LAYER_SHEETNAME = "#006464FF"
LAYER_VALUEPART = "#006464FF"
LAYER_WIRE = "#009600FF"

SCHEMATIC_SVG_ROLE_COLORS = {
    "background": LAYER_SCHEMATIC_BACKGROUND,
    "bus": LAYER_BUS,
    "bus_junction": LAYER_BUS_JUNCTION,
    "component_body": LAYER_DEVICE_BACKGROUND,
    "component_outline": LAYER_DEVICE,
    "dnp_marker": LAYER_DNP_MARKER,
    "fields": LAYER_FIELDS,
    "junction": LAYER_JUNCTION,
    "label_global": LAYER_GLOBLABEL,
    "label_hier": LAYER_HIERLABEL,
    "label_local": LAYER_LOCLABEL,
    "netclass_ref": LAYER_NETCLASS_REFS,
    "no_connect": LAYER_NOCONNECT,
    "note": LAYER_NOTES,
    "pin": LAYER_PIN,
    "pin_name": LAYER_PINNAM,
    "pin_number": LAYER_PINNUM,
    "reference": LAYER_REFERENCEPART,
    "sheet": LAYER_SHEET,
    "sheet_background": LAYER_SHEET_BACKGROUND,
    "sheet_fields": LAYER_SHEETFIELDS,
    "sheet_filename": LAYER_SHEETFILENAME,
    "sheet_label": LAYER_SHEETLABEL,
    "sheet_name": LAYER_SHEETNAME,
    "value": LAYER_VALUEPART,
    "wire": LAYER_WIRE,
    "worksheet": LAYER_SCHEMATIC_DRAWINGSHEET,
}
SCHEMATIC_SVG_FALLBACK_COLOR_ROLES = frozenset({"foreground"})
SCHEMATIC_SVG_COLOR_ROLES = tuple(
    sorted(set(SCHEMATIC_SVG_ROLE_COLORS) | SCHEMATIC_SVG_FALLBACK_COLOR_ROLES)
)
SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS = {
    role: "#FFFFFF"
    if role in {"background", "component_body", "sheet_background"}
    else "#000000"
    for role in SCHEMATIC_SVG_COLOR_ROLES
}
_SCHEMATIC_SVG_COLOR_ROLE_ALIASES = {
    "component": "component_outline",
    "component_fill": "component_body",
    "default": "foreground",
    "default_foreground": "foreground",
    "drawing_sheet": "worksheet",
    "fallback": "foreground",
    "fallback_foreground": "foreground",
    "global_label": "label_global",
    "hier_label": "label_hier",
    "hierarchical_label": "label_hier",
    "local_label": "label_local",
    "sheet_file": "sheet_filename",
    "symbol": "component_outline",
    "symbol_body": "component_body",
    "symbol_fill": "component_body",
    "symbol_outline": "component_outline",
}


def _round_pen_width_nm(value: float) -> int:
    return int(((value / 100.0) + 0.5)) * 100


def _color_override_key(value: object) -> str:
    return re.sub(r"\s+", "", str(value or "")).upper()


def _add_color_override(overrides: dict[str, str], source: str, target: str) -> None:
    if not source or not target:
        return
    source_key = _color_override_key(source)
    overrides[source_key] = target
    if source_key.startswith("#") and len(source_key) == 9 and source_key.endswith("FF"):
        overrides[source_key[:7]] = target


def normalize_schematic_svg_color_role(role: object) -> str:
    """Return the canonical schematic SVG colour role name."""

    name = re.sub(r"[^a-z0-9]+", "_", str(role or "").strip().lower()).strip("_")
    return _SCHEMATIC_SVG_COLOR_ROLE_ALIASES.get(name, name)


def schematic_svg_role_color_overrides(
    role_colors: Mapping[str, str] | None,
    *,
    base: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Return source-colour overrides for semantic schematic SVG roles.

    Role names use KiCad colour-theme keys such as ``wire``,
    ``component_outline``, ``component_body``, ``pin``, ``pin_name``,
    ``label_global``, and ``worksheet``. Unknown roles raise ``ValueError`` so
    misspelled review/theme configs fail early.
    """

    overrides = dict(base or {})
    if not role_colors:
        return overrides

    for raw_role, target in role_colors.items():
        role = normalize_schematic_svg_color_role(raw_role)
        if role in SCHEMATIC_SVG_FALLBACK_COLOR_ROLES:
            continue
        source = SCHEMATIC_SVG_ROLE_COLORS.get(role)
        if source is None:
            allowed = ", ".join(SCHEMATIC_SVG_COLOR_ROLES)
            raise ValueError(
                f"unknown schematic SVG colour role {raw_role!r}; "
                f"expected one of: {allowed}"
            )
        _add_color_override(overrides, source, str(target))
    return overrides


def schematic_svg_role_color(
    role_colors: Mapping[str, str] | None,
    role: str,
    *,
    default: str | None = None,
) -> str | None:
    """Return a configured colour for one semantic schematic SVG role."""

    if not role_colors:
        return default
    canonical = normalize_schematic_svg_color_role(role)
    for raw_role, target in role_colors.items():
        if normalize_schematic_svg_color_role(raw_role) == canonical:
            return str(target)
    return default


def apply_default_text_style(
    kwargs: dict[str, Any],
    layer_color: str,
    *,
    clamp_pen_width: bool = True,
) -> dict[str, Any]:
    """Return text kwargs with KiCad CLI default face/color/auto-width filled in."""
    out = dict(kwargs)
    out.setdefault("color", layer_color)
    if not out.get("font_face"):
        out["font_face"] = DEFAULT_SCHEMATIC_FONT_FACE
    if int(out.get("pen_width_nm") or 0) <= 0:
        size_x = int(out.get("size_x_nm") or 0)
        size_y = int(out.get("size_y_nm") or 0)
        if bool(out.get("bold")):
            text_size = min(abs(size_x), abs(size_y))
            if text_size > 0:
                out["pen_width_nm"] = _round_pen_width_nm(text_size / 5.0)
            else:
                out["pen_width_nm"] = DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM
        else:
            out["pen_width_nm"] = DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM
    size_x = int(out.get("size_x_nm") or 0)
    size_y = int(out.get("size_y_nm") or 0)
    text_size = min(abs(size_x), abs(size_y))
    if clamp_pen_width and text_size > 0:
        out["pen_width_nm"] = min(
            int(out.get("pen_width_nm") or 0),
            int((text_size * 0.25) + 0.5),
        )
    return out


def symbol_property_layer_color(key: str) -> str:
    if key == "Reference":
        return LAYER_REFERENCEPART
    if key == "Value":
        return LAYER_VALUEPART
    return LAYER_FIELDS


def sheet_property_layer_color(key: str) -> str:
    if key in {"Sheetname", "Sheet name"}:
        return LAYER_SHEETNAME
    if key in {"Sheetfile", "Sheet file"}:
        return LAYER_SHEETFILENAME
    return LAYER_SHEETFIELDS


__all__ = [
    "DEFAULT_SCHEMATIC_FONT_FACE",
    "DEFAULT_MIN_PLOT_PEN_WIDTH_NM",
    "DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM",
    "DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM",
    "DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM",
    "DEFAULT_KICAD_DRAWING_SHEET_VERSION_TEXT",
    "DEFAULT_PIN_SYMBOL_RADIUS_NM",
    "SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS",
    "SCHEMATIC_SVG_COLOR_ROLES",
    "SCHEMATIC_SVG_FALLBACK_COLOR_ROLES",
    "SCHEMATIC_SVG_ROLE_COLORS",
    "LAYER_BUS",
    "LAYER_BUS_JUNCTION",
    "LAYER_DEVICE",
    "LAYER_DEVICE_BACKGROUND",
    "LAYER_DNP_MARKER",
    "LAYER_FIELDS",
    "LAYER_GLOBLABEL",
    "LAYER_HIERLABEL",
    "LAYER_JUNCTION",
    "LAYER_LOCLABEL",
    "LAYER_NOCONNECT",
    "LAYER_NOTES",
    "LAYER_PIN",
    "LAYER_PINNAM",
    "LAYER_PINNUM",
    "LAYER_REFERENCEPART",
    "LAYER_SCHEMATIC_BACKGROUND",
    "LAYER_SCHEMATIC_DRAWINGSHEET",
    "LAYER_SHEET",
    "LAYER_SHEET_BACKGROUND",
    "LAYER_SHEETFIELDS",
    "LAYER_SHEETFILENAME",
    "LAYER_SHEETLABEL",
    "LAYER_SHEETNAME",
    "LAYER_VALUEPART",
    "LAYER_WIRE",
    "apply_default_text_style",
    "normalize_schematic_svg_color_role",
    "sheet_property_layer_color",
    "schematic_svg_role_color",
    "schematic_svg_role_color_overrides",
    "symbol_property_layer_color",
]

"""KiCad preference-file adapters for SVG rendering options."""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
from pathlib import Path
import re
from typing import Any

from .kicad_schematic_style import SCHEMATIC_SVG_COLOR_ROLES
from .kicad_sch_svg_renderer import KiCadSvgRenderOptions
from .kicad_symbol_svg import SymbolTheme


@dataclass(frozen=True)
class KiCadSvgPreferenceTheme:
    """Resolved KiCad preference colour theme used by SVG renderers."""

    name: str
    default_font: str | None
    schematic: dict[str, str]
    board: dict[str, str]


_CSS_RGB_RE = re.compile(
    r"rgba?\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*,\s*([0-9.]+)"
    r"(?:\s*,\s*([0-9.]+)\s*)?\)",
    re.IGNORECASE,
)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError, TypeError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def _clamp_byte(value: str) -> int:
    return max(0, min(255, int(round(float(value)))))


def _alpha_byte(value: str | None) -> int | None:
    if value is None:
        return None
    number = float(value)
    if number <= 1.0:
        number *= 255.0
    return max(0, min(255, int(round(number))))


def _normalise_css_color(value: object) -> str:
    text = str(value or "").strip()
    if not text:
        return ""
    if text.startswith("#"):
        out = text.upper()
        if len(out) in {4, 5}:
            chars = out[1:]
            out = "#" + "".join(ch * 2 for ch in chars)
        return out
    match = _CSS_RGB_RE.fullmatch(text)
    if not match:
        return text
    r = _clamp_byte(match.group(1))
    g = _clamp_byte(match.group(2))
    b = _clamp_byte(match.group(3))
    a = _alpha_byte(match.group(4))
    if a is None or a == 255:
        return f"#{r:02X}{g:02X}{b:02X}"
    return f"#{r:02X}{g:02X}{b:02X}{a:02X}"


def load_kicad_svg_preference_theme(
    preferences_dir: Path | str,
    *,
    theme_name: str | None = None,
) -> KiCadSvgPreferenceTheme:
    """Load KiCad colour/font preferences from a KiCad config directory."""

    pref_dir = Path(preferences_dir)
    eeschema = _read_json(pref_dir / "eeschema.json")
    appearance_raw = eeschema.get("appearance")
    appearance: dict[str, Any] = appearance_raw if isinstance(appearance_raw, dict) else {}
    selected_name = theme_name or str(appearance.get("color_theme") or "").strip()
    theme_raw = _read_json(pref_dir / "colors" / f"{selected_name}.json") if selected_name else {}
    schematic_raw_value = theme_raw.get("schematic")
    board_raw_value = theme_raw.get("board")
    schematic_raw: dict[str, Any] = schematic_raw_value if isinstance(schematic_raw_value, dict) else {}
    board_raw: dict[str, Any] = board_raw_value if isinstance(board_raw_value, dict) else {}
    schematic = {
        str(key): _normalise_css_color(value)
        for key, value in schematic_raw.items()
        if _normalise_css_color(value)
    }
    board = {
        str(key): _normalise_css_color(value)
        for key, value in board_raw.items()
        if _normalise_css_color(value)
    }
    default_font = str(appearance.get("default_font") or "").strip() or None
    return KiCadSvgPreferenceTheme(
        name=selected_name,
        default_font=default_font,
        schematic=schematic,
        board=board,
    )


def schematic_svg_options_from_preferences(
    preferences_dir: Path | str,
    *,
    theme_name: str | None = None,
    base: KiCadSvgRenderOptions | None = None,
) -> KiCadSvgRenderOptions:
    """Return schematic SVG options using a KiCad colour theme and default font."""

    pref = load_kicad_svg_preference_theme(preferences_dir, theme_name=theme_name)
    opts = replace(base) if base is not None else KiCadSvgRenderOptions.enriched_default()
    role_colors: dict[str, str] = {}
    for role in SCHEMATIC_SVG_COLOR_ROLES:
        target = pref.schematic.get(role)
        if target:
            role_colors[role] = target
    if role_colors:
        opts = opts.with_schematic_role_colors(role_colors)
    if pref.default_font:
        opts.font_face_override = pref.default_font
    return opts


def symbol_theme_from_preferences(
    preferences_dir: Path | str,
    *,
    theme_name: str | None = None,
    base: SymbolTheme | None = None,
) -> SymbolTheme:
    """Return a symbol SVG theme using KiCad schematic colour preferences."""

    pref = load_kicad_svg_preference_theme(preferences_dir, theme_name=theme_name)
    theme = base or SymbolTheme()
    schematic = pref.schematic
    theme.body_outline = schematic.get("component_outline", theme.body_outline)
    theme.body_fill = schematic.get("component_body", theme.body_fill)
    theme.pin_color = schematic.get("pin", theme.pin_color)
    theme.text_color = schematic.get("fields", theme.text_color)
    theme.pin_name_color = schematic.get("pin_name", theme.pin_name_color)
    theme.pin_number_color = schematic.get("pin_number", theme.pin_number_color)
    theme.reference_color = schematic.get("reference", theme.reference_color)
    theme.value_color = schematic.get("value", theme.value_color)
    theme.field_color = schematic.get("fields", theme.field_color)
    theme.background_color = schematic.get("background", theme.background_color)
    return theme


__all__ = [
    "KiCadSvgPreferenceTheme",
    "load_kicad_svg_preference_theme",
    "schematic_svg_options_from_preferences",
    "symbol_theme_from_preferences",
]

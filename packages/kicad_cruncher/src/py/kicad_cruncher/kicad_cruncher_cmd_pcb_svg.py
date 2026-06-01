"""pcb-svg command for explicit A0 KiCad PCB SVG views."""

from __future__ import annotations

import argparse
import base64
import hashlib
import html
import json
import logging
import math
import os
from pathlib import Path
from typing import TYPE_CHECKING, Literal, cast

from kicad_cruncher.config_json import load_json_config
from kicad_cruncher.kicad_cruncher_common import resolve_output_dir
from kicad_cruncher.kicad_cruncher_pcb_svg_config import (
    PCB_DEFAULT_SVG_SCALE,
    PCB_SVG_CONFIG_FILENAME,
    PCB_SVG_CONFIG_SCHEMA,
    _PcbSvgConfig,
    _PcbSvgViewConfig,
    normalize_layer_token,
    parse_pcb_layer_selector,
    physical_layer_from_token,
    resolve_config_output_path,
)
from kicad_cruncher.kicad_cruncher_pcb_svg_projection import (
    _AssemblyProjectedArc,
    _AssemblyProjectedGeometry,
    _AssemblyProjectionOptions,
    _get_assembly_projection_cache,
)

log = logging.getLogger(__name__)

if TYPE_CHECKING:
    from kicad_monkey.kicad_geometry import BoundingBox
    from kicad_monkey.kicad_model import EmbeddedFile, Model
    from kicad_monkey.kicad_pcb import KiCadPcb
    from kicad_monkey.kicad_pcb_footprint import Footprint

_VIEW_ALIASES = {
    "top": "top_view",
    "top-view": "top_view",
    "bottom": "bottom_view",
    "bottom-view": "bottom_view",
    "top-pin1": "top_pin1_view",
    "top-pin-1": "top_pin1_view",
    "pin1-top": "top_pin1_view",
    "pin-1-top": "top_pin1_view",
    "bottom-pin1": "bottom_pin1_view",
    "bottom-pin-1": "bottom_pin1_view",
    "pin1-bottom": "bottom_pin1_view",
    "pin-1-bottom": "bottom_pin1_view",
    "assembly-top": "assembly_top_view",
    "assembly-bottom": "assembly_bottom_view",
}

_HLR_TOKENS = {"ASSEMBLY_HLR_TOP", "ASSEMBLY_HLR_BOTTOM"}
_SYNTHETIC_NO_PHYSICAL = {
    "BOARD_CUTOUTS",
    "DRILLS",
    "SLOTS",
    "ASSEMBLY_DESIGNATORS_TOP",
    "ASSEMBLY_DESIGNATORS_BOTTOM",
    "PIN1_TOP",
    "PIN1_BOTTOM",
}
_ASSEMBLY_HLR_EDGE_FLAG_KEYS = {
    "edge_v_sharp",
    "edge_v_outline",
    "edge_v_smooth",
    "edge_v_sewn",
    "edge_v_iso",
    "edge_h_sharp",
    "edge_h_outline",
    "edge_h_smooth",
    "edge_h_sewn",
    "edge_h_iso",
}
_PCB_SVG_ASSEMBLY_HLR_TOP_LAYER_ID = 9004
_PCB_SVG_ASSEMBLY_HLR_BOTTOM_LAYER_ID = 9005


def _safe_svg_id(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "._-" else "-" for ch in value).strip("-")


def _fmt(value: float) -> str:
    text = f"{float(value):.4f}".rstrip("0").rstrip(".")
    return text or "0"


def _comment_safe(value: object) -> str:
    return " ".join(str(value).replace("\r", " ").replace("\n", " ").split())


def _default_pcb_svg_config_text() -> str:
    """Return the JSONC text used for auto-created PCB SVG configs."""
    payload = json.dumps(_PcbSvgConfig.default().to_dict(), indent=2)
    return (
        "// kicad-cruncher pcb-svg configuration\n"
        "// This file is JSONC: // comments, /* block comments */, and trailing\n"
        "//   commas are accepted.\n"
        "\n"
        f"// Schema: {PCB_SVG_CONFIG_SCHEMA}\n"
        "\n"
        "// Common physical layer tokens: TOP, BOTTOM, TOPOVERLAY, BOTTOMOVERLAY,\n"
        "//   TOPPASTE, BOTTOMPASTE, TOPSOLDER, BOTTOMSOLDER, F.Cu, B.Cu,\n"
        "//   F.SilkS, B.SilkS, F.Fab, B.Fab, Edge.Cuts.\n"
        "\n"
        "// Synthetic layer tokens: BOARD_OUTLINE, BOARD_CUTOUTS, DRILLS, SLOTS,\n"
        "//   ASSEMBLY_HLR_TOP, ASSEMBLY_HLR_BOTTOM,\n"
        "//   ASSEMBLY_DESIGNATORS_TOP, ASSEMBLY_DESIGNATORS_BOTTOM, PIN1_TOP, PIN1_BOTTOM.\n"
        "\n"
        "// In each view, the layers array is the draw order. KiCad physical layers\n"
        "//   are rendered by kicad-monkey; ASSEMBLY_HLR_* adds geometer STEP HLR\n"
        "//   overlays from embedded or resolvable STEP models.\n"
        "\n"
        "/*\n"
        "HLR modes:\n"
        "  bounding_box - footprint bounds rectangle; no Geometer/STEP projection.\n"
        "  simple       - Geometer simple outline, with bounds fallback when no\n"
        "                 STEP model is available.\n"
        "  detail       - Geometer detailed visible projection, with bounds fallback\n"
        "                 when no STEP model is available.\n"
        "  none         - suppress HLR projection.\n"
        "\n"
        "Assembly HLR style override example, globally, inside any view.styles,\n"
        "or inside components.<designator>.assembly_hlr for a single part:\n"
        "  \"assembly_hlr\": {\n"
        "    \"enabled\": true,\n"
        "    \"color\": \"#F59E0B\",\n"
        "    \"line_width_mm\": 0.12,\n"
        "    \"projection_algorithm\": \"exact\",\n"
        "    \"curve_mode\": \"native_arcs\",\n"
        "    \"samples_per_curve\": 24,\n"
        "    \"round_digits\": 3,\n"
        "    \"include_visible\": true,\n"
        "    \"include_outline\": true,\n"
        "    \"union_polygons\": true,\n"
        "    \"mesh_linear_deflection\": 0.01,\n"
        "    \"mesh_angular_deflection\": 0.5,\n"
        "    \"mesh_relative\": false,\n"
        "    \"hlr_angle_tolerance\": 0.0174533\n"
        "  }\n"
        "\n"
        "Component override examples:\n"
        "  \"components\": {\n"
        "    \"J1\": {\"projection\": \"none\"},\n"
        "    \"U5\": {\"projection\": \"detail\", \"assembly_hlr\": {\"color\": \"#2563EB\"}}\n"
        "  }\n"
        "*/\n"
        "\n"
        "// Set layer_outputs.enabled=false if you only want composed views.\n"
        f"{payload}\n"
    )


def _write_default_pcb_svg_config(config_path: Path) -> None:
    """Write an editable A0 pcb-svg config template."""
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(_default_pcb_svg_config_text(), encoding="utf-8")


def _load_pcb_svg_config(config_path: Path) -> _PcbSvgConfig:
    try:
        raw_data = load_json_config(config_path)
    except Exception as exc:
        raise ValueError(f"Failed to parse pcb-svg config '{config_path}': {exc}") from exc
    return _PcbSvgConfig.from_dict(raw_data)


def _parse_pcb_views(raw_views: str | None) -> set[str] | None:
    if raw_views is None:
        return None
    values = [token.strip().lower() for token in raw_views.split(",") if token.strip()]
    if not values:
        return None
    selected: set[str] = set()
    for value in values:
        normalized = value.replace("_", "-")
        if normalized == "all":
            return {"all"}
        if normalized == "none":
            selected.add("none")
            continue
        if normalized in {"layers", "layer", "layer-outputs"}:
            selected.add("layers")
            continue
        selected.add(_VIEW_ALIASES.get(normalized, value.replace("-", "_")))
    return selected


def _apply_pcb_view_selection(config: _PcbSvgConfig, raw_views: str | None) -> None:
    """Apply CLI view filtering to an A0 config."""
    selected = _parse_pcb_views(raw_views)
    if selected is None or "all" in selected:
        return
    if "none" in selected and len(selected) == 1:
        config.layer_outputs["enabled"] = False
        for view in config.views:
            view.enabled = False
        return

    config.layer_outputs["enabled"] = "layers" in selected
    known_names = {view.name for view in config.views}
    requested_views = selected - {"layers", "none"}
    unknown = requested_views - known_names
    if unknown:
        raise ValueError(
            "Unknown --views token(s): "
            + ", ".join(sorted(unknown))
            + ". Use a configured view name, layers, all, or none."
        )
    for view in config.views:
        view.enabled = view.name in requested_views


def _apply_pcb_layer_selection(config: _PcbSvgConfig, raw_layers: str | None) -> None:
    selected_layers = parse_pcb_layer_selector(raw_layers)
    if selected_layers is not None:
        config.layer_outputs["layers"] = selected_layers


def _apply_cli_overrides(config: _PcbSvgConfig, args: object) -> None:
    pcbdoc = getattr(args, "pcbdoc", None)
    if pcbdoc:
        config.global_options.pcbdoc = str(pcbdoc)
    _apply_pcb_view_selection(config, getattr(args, "pcb_views", None))
    _apply_pcb_layer_selection(config, getattr(args, "pcb_layers", None))
    svg_scale = getattr(args, "pcb_svg_scale", None)
    if svg_scale is not None:
        config.global_options.svg_scale = float(svg_scale)
    svg_size_unit = getattr(args, "pcb_svg_size_unit", None)
    if svg_size_unit is not None:
        config.global_options.svg_size_unit = str(svg_size_unit)
    if getattr(args, "pcb_clean_output", False):
        config.global_options.clean_output = True


def _resolve_pcb_svg_configs(
    args: object,
    input_files: list[Path],
) -> tuple[dict[Path, _PcbSvgConfig], list[Path]]:
    """Resolve one A0 pcb-svg config per input file."""
    resolved_input_files = [path.resolve() for path in input_files]
    created_paths: list[Path] = []
    config_by_input: dict[Path, _PcbSvgConfig] = {}
    config_cache: dict[Path, _PcbSvgConfig] = {}

    raw_config = getattr(args, "config", None)
    if raw_config:
        explicit_config_path = Path(raw_config).resolve()
        if not explicit_config_path.exists():
            _write_default_pcb_svg_config(explicit_config_path)
            created_paths.append(explicit_config_path)
        loaded_config = _load_pcb_svg_config(explicit_config_path)
        _apply_cli_overrides(loaded_config, args)
        for input_file in resolved_input_files:
            config_by_input[input_file] = loaded_config
        return config_by_input, created_paths

    for input_file in resolved_input_files:
        auto_config_path = input_file.parent / PCB_SVG_CONFIG_FILENAME
        if not auto_config_path.exists():
            _write_default_pcb_svg_config(auto_config_path)
            created_paths.append(auto_config_path)

    for input_file in resolved_input_files:
        auto_config_path = input_file.parent / PCB_SVG_CONFIG_FILENAME
        loaded = config_cache.get(auto_config_path)
        if loaded is None:
            loaded = _load_pcb_svg_config(auto_config_path)
            _apply_cli_overrides(loaded, args)
            config_cache[auto_config_path] = loaded
        config_by_input[input_file] = loaded

    return config_by_input, sorted(set(created_paths))


def _resolve_explicit_input(raw_file: str | None) -> list[Path] | None:
    """Resolve explicit input or auto-detect one KiCad project/PCB in CWD."""
    if raw_file:
        input_file = Path(raw_file).resolve()
        if input_file.exists():
            return [input_file]
        log.error("File not found: %s", input_file)
        return None

    projects = sorted(path for path in Path.cwd().glob("*.kicad_pro") if path.is_file())
    pcbs = sorted(path for path in Path.cwd().glob("*.kicad_pcb") if path.is_file())
    candidates = projects or pcbs
    if len(candidates) != 1:
        log.error(
            "No file specified and no single .kicad_pro/.kicad_pcb found "
            "in current directory"
        )
        log.info("Usage: kicad-cruncher pcb-svg [project.kicad_pro | board.kicad_pcb]")
        return None
    log.info("Auto-detected PCB SVG input: %s", candidates[0].name)
    return [candidates[0].resolve()]


def _validate_input_file(input_file: Path) -> bool:
    suffix = input_file.suffix.lower()
    if suffix in {".kicad_pcb", ".kicad_pro"}:
        return True
    log.error("Unsupported file type: %s", suffix)
    log.info("Supported PCB SVG types: .kicad_pcb, .kicad_pro")
    return False


def _resolve_project_pcb(project_path: Path, selector: str | None) -> Path:
    if selector:
        selector_path = Path(selector)
        candidates = []
        if selector_path.is_absolute():
            candidates.append(selector_path)
        else:
            candidates.append(project_path.parent / selector_path)
            candidates.append(project_path.parent / f"{selector}.kicad_pcb")
        for candidate in candidates:
            if candidate.exists() and candidate.suffix.lower() == ".kicad_pcb":
                return candidate.resolve()
        raise FileNotFoundError(f"Could not resolve PCB selector {selector!r} for {project_path}")

    same_stem = project_path.with_suffix(".kicad_pcb")
    if same_stem.exists():
        return same_stem.resolve()
    pcbs = sorted(path for path in project_path.parent.glob("*.kicad_pcb") if path.is_file())
    if len(pcbs) == 1:
        return pcbs[0].resolve()
    raise FileNotFoundError(f"Could not find a single .kicad_pcb next to {project_path.name}")


def _load_kicad_pcb(input_file: Path, config: _PcbSvgConfig) -> tuple[KiCadPcb, Path]:
    from kicad_monkey.kicad_pcb import KiCadPcb

    pcb_path = (
        _resolve_project_pcb(input_file, config.global_options.pcbdoc)
        if input_file.suffix.lower() == ".kicad_pro"
        else input_file
    )
    return KiCadPcb.from_file(pcb_path), pcb_path.resolve()


def _style_enabled(styles: dict[str, dict[str, object]], name: str) -> bool:
    return bool(styles.get(name, {}).get("enabled", True))


def _style_color(styles: dict[str, dict[str, object]], name: str, default: str) -> str:
    return str(styles.get(name, {}).get("color") or default)


def _style_float(
    styles: dict[str, dict[str, object]],
    name: str,
    key: str,
    default: float,
) -> float:
    value = styles.get(name, {}).get(key, default)
    if not isinstance(value, int | float | str):
        raise ValueError(f"Invalid pcb-svg style value {name}.{key}")
    try:
        return float(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid pcb-svg style value {name}.{key}") from exc


def _style_int(
    styles: dict[str, dict[str, object]],
    name: str,
    key: str,
    default: int,
) -> int:
    value = styles.get(name, {}).get(key, default)
    if not isinstance(value, int | float | str):
        raise ValueError(f"Invalid pcb-svg style value {name}.{key}")
    try:
        return int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid pcb-svg style value {name}.{key}") from exc


def _style_bool(
    styles: dict[str, dict[str, object]],
    name: str,
    key: str,
    default: bool,
) -> bool:
    value = styles.get(name, {}).get(key, default)
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        normalized = value.lower().strip()
        if normalized in {"1", "true", "yes", "on"}:
            return True
        if normalized in {"0", "false", "no", "off"}:
            return False
    return bool(value)


def _physical_layers_for_tokens(tokens: list[str]) -> list[str]:
    layers: list[str] = []
    for raw_token in tokens:
        token = normalize_layer_token(raw_token)
        if token in _HLR_TOKENS or token in _SYNTHETIC_NO_PHYSICAL:
            continue
        physical = physical_layer_from_token(token)
        if physical is not None and physical not in layers:
            layers.append(physical)
    return layers


def _layer_output_tokens(config: _PcbSvgConfig, pcb: KiCadPcb) -> list[str]:
    configured = config.layer_outputs.get("layers", "auto")
    if configured == "auto":
        layers = [str(getattr(layer, "name", "") or "") for layer in getattr(pcb, "layers", [])]
        return [layer for layer in layers if layer]
    if not isinstance(configured, list):
        return []
    return [normalize_layer_token(str(token)) for token in configured]


def _layer_output_special_tokens(config: _PcbSvgConfig) -> list[str]:
    raw_include_special = config.layer_outputs.get("include_special_layers", [])
    if not isinstance(raw_include_special, list):
        return []
    return [normalize_layer_token(str(token)) for token in raw_include_special]


def _render_kicad_svg(pcb: KiCadPcb, layers: list[str]) -> str:
    return str(pcb.to_svg(layers=layers or None))


def _view_mirror(config: _PcbSvgConfig, view: _PcbSvgViewConfig) -> bool:
    if view.mirror is not None:
        return bool(view.mirror)
    normalized_layers = [normalize_layer_token(token) for token in view.layers]
    return bool(
        config.global_options.mirror_bottom_view
        and any("BOTTOM" in token or token.startswith("B.") for token in normalized_layers)
    )


def _render_a0_layer_outputs(
    config: _PcbSvgConfig,
    pcb: KiCadPcb,
    *,
    output_dir: Path,
    board_name: str,
    layer_manifest: dict[str, object],
) -> int:
    if not bool(config.layer_outputs.get("enabled", True)):
        return 0
    layer_dir = output_dir / str(config.layer_outputs.get("output_dir") or "layers")
    include_special = _layer_output_special_tokens(config)
    written = 0
    for layer_token in _layer_output_tokens(config, pcb):
        group_id = f"pcb-svg-layer-{_safe_svg_id(layer_token.lower())}"
        view_layers = [layer_token, *include_special]
        physical_layers = _physical_layers_for_tokens(view_layers)
        if not physical_layers and not bool(config.global_options.show_empty_layers):
            continue
        svg_text = _render_kicad_svg(pcb, physical_layers)
        layer_path = layer_dir / f"{board_name}__{_safe_svg_id(layer_token)}.svg"
        layer_path.parent.mkdir(parents=True, exist_ok=True)
        layer_path.write_text(svg_text, encoding="utf-8")
        layer_manifest[layer_token] = {
            "file": str(layer_path.relative_to(output_dir)).replace("\\", "/"),
            "layers": view_layers,
            "physical_layers": physical_layers,
            "group_id": group_id,
        }
        written += 1
    return written


def _render_a0_configured_views(
    config: _PcbSvgConfig,
    pcb: KiCadPcb,
    pcb_path: Path,
    *,
    output_dir: Path,
    board_name: str,
    view_manifest: dict[str, object],
) -> int:
    written = 0
    for view in config.enabled_views():
        styles = config.resolved_styles_for_view(view)
        mirror = _view_mirror(config, view)
        group_id = view.resolved_group_id()
        svg_text = _render_view_svg(
            pcb,
            pcb_path,
            view,
            group_id=group_id,
            mirror=mirror,
            styles=styles,
        )
        view_path = resolve_config_output_path(
            output_dir,
            view.resolved_output_svg(),
            board=board_name,
            view=view.name,
        )
        view_path.parent.mkdir(parents=True, exist_ok=True)
        view_path.write_text(svg_text, encoding="utf-8")
        view_manifest[view.name] = {
            "file": str(view_path.relative_to(output_dir)).replace("\\", "/"),
            "group_id": group_id,
            "layers": view.layers,
            "mirrored": mirror,
            "assembly_hlr_mode": view.assembly_hlr_mode,
        }
        written += 1
    return written


def _render_view_svg(
    pcb: KiCadPcb,
    pcb_path: Path,
    view: _PcbSvgViewConfig,
    *,
    group_id: str,
    mirror: bool,
    styles: dict[str, dict[str, object]],
) -> str:
    physical_layers = _physical_layers_for_tokens(view.layers)
    svg_text = _render_kicad_svg(pcb, physical_layers)
    overlay = _render_assembly_hlr_overlay(
        pcb,
        pcb_path,
        view,
        styles=styles,
        mirror=mirror,
    )
    if not overlay:
        return svg_text
    insert_at = svg_text.rfind("</svg>")
    if insert_at < 0:
        return svg_text + "\n" + overlay
    return svg_text[:insert_at] + overlay + "\n" + svg_text[insert_at:]


def _render_assembly_hlr_overlay(
    pcb: KiCadPcb,
    pcb_path: Path,
    view: _PcbSvgViewConfig,
    *,
    styles: dict[str, dict[str, object]],
    mirror: bool,
) -> str:
    del mirror
    hlr_tokens = _hlr_tokens_for_view(view)
    if not _should_render_assembly_hlr(view, styles, hlr_tokens):
        return ""

    bbox = _compute_pcb_svg_bbox(pcb)
    if bbox.is_empty:
        return ""
    color = _style_color(styles, "assembly_hlr", "#F59E0B")
    line_width = _style_float(styles, "assembly_hlr", "line_width_mm", 0.12)
    pieces = [
        _render_assembly_hlr_token_group(
            pcb,
            pcb_path,
            token,
            view=view,
            styles=styles,
            bbox=bbox,
            color=color,
            line_width=line_width,
        )
        for token in hlr_tokens
    ]
    return "\n" + "\n".join(pieces)


def _hlr_tokens_for_view(view: _PcbSvgViewConfig) -> list[str]:
    return [
        normalized
        for token in view.layers
        if (normalized := normalize_layer_token(token)) in _HLR_TOKENS
    ]


def _should_render_assembly_hlr(
    view: _PcbSvgViewConfig,
    styles: dict[str, dict[str, object]],
    hlr_tokens: list[str],
) -> bool:
    return bool(
        hlr_tokens
        and view.assembly_hlr_mode != "none"
        and _style_enabled(styles, "assembly_hlr")
    )


def _compute_pcb_svg_bbox(pcb: KiCadPcb) -> BoundingBox:
    from kicad_monkey.kicad_pcb_bounds import compute_pcb_svg_bounding_box

    return compute_pcb_svg_bounding_box(pcb, None)


def _pcb_footprints(pcb: KiCadPcb) -> list[Footprint]:
    return cast("list[Footprint]", pcb.footprints)


def _assembly_hlr_layer_id(side: str) -> int:
    if side == "top":
        return _PCB_SVG_ASSEMBLY_HLR_TOP_LAYER_ID
    return _PCB_SVG_ASSEMBLY_HLR_BOTTOM_LAYER_ID


def _render_assembly_hlr_token_group(
    pcb: KiCadPcb,
    pcb_path: Path,
    token: str,
    *,
    view: _PcbSvgViewConfig,
    styles: dict[str, dict[str, object]],
    bbox: BoundingBox,
    color: str,
    line_width: float,
) -> str:
    side = "top" if token == "ASSEMBLY_HLR_TOP" else "bottom"
    group_lines = [
        (
            f'<g id="assembly-overlay" data-layer-id="{_assembly_hlr_layer_id(side)}" '
            f'data-layer-token="{html.escape(token)}" '
            f'data-assembly-symbol="{html.escape(view.assembly_hlr_mode)}" '
            f'stroke="{html.escape(color)}" stroke-width="{_fmt(line_width)}" '
            'fill="none" stroke-linecap="round" stroke-linejoin="round">'
        )
    ]
    for footprint in _pcb_footprints(pcb):
        if _footprint_side(footprint) != side:
            continue
        group_lines.extend(
            _render_footprint_hlr(
                pcb,
                pcb_path,
                footprint,
                side=side,
                mode=view.assembly_hlr_mode,
                styles=styles,
                bbox=bbox,
            )
        )
    group_lines.append("</g>")
    return "\n".join(group_lines)


def _footprint_side(footprint: Footprint) -> str:
    layer = str(getattr(footprint, "layer", "") or "")
    return "bottom" if layer.startswith("B.") else "top"


def _footprint_designator(footprint: Footprint) -> str:
    getter = getattr(footprint, "get_property_value", None)
    if callable(getter):
        try:
            designator = str(getter("Reference", "") or "").strip()
            if designator:
                return designator
        except (TypeError, ValueError):
            pass
    return str(getattr(footprint, "library_link", "") or "component").strip() or "component"


def _render_footprint_hlr(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    side: str,
    mode: str,
    styles: dict[str, dict[str, object]],
    bbox: BoundingBox,
) -> list[str]:
    designator = _footprint_designator(footprint)
    component_style = styles.get("assembly_hlr", {})
    projection_mode = mode
    if projection_mode in {"simple", "detail"}:
        rendered = _render_footprint_geometer_hlr(
            pcb,
            pcb_path,
            footprint,
            side=side,
            mode=projection_mode,
            styles=styles,
            bbox=bbox,
        )
        if rendered:
            group_start = (
                f'<g data-component="{html.escape(designator)}" '
                f'data-projection="{projection_mode}">'
            )
            return [
                group_start,
                *rendered,
                "</g>",
            ]
    if projection_mode == "none":
        return []
    return [
        f'<g data-component="{html.escape(designator)}" data-projection="bounding_box">',
        _render_footprint_bounds_rect(
            footprint,
            bbox=bbox,
            color=str(component_style.get("color") or "#F59E0B"),
        ),
        "</g>",
    ]


def _render_footprint_geometer_hlr(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    side: str,
    mode: str,
    styles: dict[str, dict[str, object]],
    bbox: BoundingBox,
) -> list[str]:
    model = _first_step_model(footprint)
    if model is None:
        return []
    step_bytes = _resolve_model_step_bytes(pcb, footprint, model, pcb_path)
    if step_bytes is None:
        return []
    model_hash = hashlib.sha256(step_bytes).hexdigest()
    transform = _model_transform_matrix(model)
    options = _assembly_projection_options(side=side, styles=styles)
    try:
        _, projected = _get_assembly_projection_cache().project(
            model_hash=model_hash,
            step_bytes=step_bytes,
            pose_signature=_model_pose_signature(model),
            transform_matrix=transform,
            options=options,
            model_label=str(getattr(model, "path", "")),
        )
    except Exception as exc:
        log.warning("Geometer HLR failed for %s: %s", _footprint_designator(footprint), exc)
        return []
    return _projected_geometry_to_svg(projected, footprint, model, mode=mode, bbox=bbox)


def _assembly_projection_options(
    *,
    side: str,
    styles: dict[str, dict[str, object]],
) -> _AssemblyProjectionOptions:
    style = styles.get("assembly_hlr", {})
    edge_flags = {
        key: bool(style[key])
        for key in _ASSEMBLY_HLR_EDGE_FLAG_KEYS
        if key in style
    }
    curve_mode: Literal["native_arcs", "polyline"] = (
        "polyline" if str(style.get("curve_mode", "native_arcs")) == "polyline" else "native_arcs"
    )
    return _AssemblyProjectionOptions(
        side="bottom" if side == "bottom" else "top",
        projection_algorithm=(
            None
            if style.get("projection_algorithm") is None
            else str(style.get("projection_algorithm"))
        ),
        curve_mode=curve_mode,
        samples_per_curve=_style_int(styles, "assembly_hlr", "samples_per_curve", 24),
        round_digits=_style_int(styles, "assembly_hlr", "round_digits", 3),
        include_visible=_style_bool(styles, "assembly_hlr", "include_visible", True),
        include_outline=_style_bool(styles, "assembly_hlr", "include_outline", True),
        union_polygons=_style_bool(styles, "assembly_hlr", "union_polygons", True),
        mesh_linear_deflection=_optional_style_float(style, "mesh_linear_deflection"),
        mesh_angular_deflection=_optional_style_float(style, "mesh_angular_deflection"),
        mesh_relative=_optional_style_bool(style, "mesh_relative"),
        hlr_angle_tolerance=_optional_style_float(style, "hlr_angle_tolerance"),
        edge_flags=edge_flags or None,
    )


def _optional_style_float(style: dict[str, object], key: str) -> float | None:
    value = style.get(key)
    if value is None:
        return None
    if not isinstance(value, int | float | str):
        raise ValueError(f"Invalid pcb-svg style value assembly_hlr.{key}")
    return float(value)


def _optional_style_bool(style: dict[str, object], key: str) -> bool | None:
    value = style.get(key)
    if value is None:
        return None
    if isinstance(value, bool):
        return value
    return str(value).strip().lower() in {"1", "true", "yes", "on"}


def _first_step_model(footprint: Footprint) -> Model | None:
    for model in getattr(footprint, "models", []) or []:
        path = str(getattr(model, "path", "") or "").lower()
        if path.endswith((".step", ".stp")):
            return model
    return None


def _resolve_model_step_bytes(
    pcb: KiCadPcb,
    footprint: Footprint,
    model: Model,
    pcb_path: Path,
) -> bytes | None:
    raw_path = str(getattr(model, "path", "") or "")
    if raw_path.startswith("kicad-embed://"):
        embedded_name = raw_path[len("kicad-embed://") :]
        embedded = _find_embedded_file(pcb, footprint, embedded_name)
        if embedded is None:
            return None
        return _decode_embedded_file_data(str(getattr(embedded, "data", "") or ""))
    model_path = _resolve_external_model_path(raw_path, pcb_path.parent)
    if model_path is None or not model_path.exists():
        return None
    return model_path.read_bytes()


def _find_embedded_file(pcb: KiCadPcb, footprint: Footprint, name: str) -> EmbeddedFile | None:
    wanted = name.lower()
    for owner in (footprint, pcb):
        for embedded in getattr(owner, "embedded_files", []) or []:
            if (
                str(getattr(embedded, "name", "") or "").lower() == wanted
                and str(getattr(embedded, "data", "") or "")
            ):
                return embedded
    return None


def _decode_embedded_file_data(data: str) -> bytes | None:
    if not data:
        return None
    try:
        compressed = base64.b64decode(data, validate=False)
    except ValueError:
        return None
    try:
        import zstandard as zstd
    except Exception as exc:  # pragma: no cover - dependency failure path
        raise RuntimeError("zstd support is unavailable; install 'zstandard'") from exc
    return zstd.ZstdDecompressor().decompress(compressed)


def _resolve_external_model_path(raw_path: str, base_dir: Path) -> Path | None:
    if not raw_path:
        return None
    expanded = os.path.expandvars(raw_path)
    if expanded.startswith("${") and "}" in expanded:
        env_name, rest = expanded[2:].split("}", 1)
        expanded = os.environ.get(env_name, "") + rest
    path = Path(expanded)
    if path.is_absolute():
        return path
    return base_dir / path


def _model_pose_signature(model: Model) -> tuple[float, ...]:
    values: list[float] = []
    for attr in ("offset", "scale", "rotate"):
        values.extend(float(value) for value in getattr(model, attr, ()))
    return tuple(values)


def _model_transform_matrix(model: Model) -> list[list[float]]:
    raw_scale = tuple(float(value) for value in getattr(model, "scale", (1.0, 1.0, 1.0)))
    scale = (
        raw_scale[0] if len(raw_scale) > 0 else 1.0,
        raw_scale[1] if len(raw_scale) > 1 else 1.0,
        raw_scale[2] if len(raw_scale) > 2 else 1.0,
    )
    return _scale_matrix(scale)


def _scale_matrix(scale: tuple[float, float, float]) -> list[list[float]]:
    return [
        [scale[0], 0.0, 0.0, 0.0],
        [0.0, scale[1], 0.0, 0.0],
        [0.0, 0.0, scale[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _rotation_x_matrix(angle: float) -> list[list[float]]:
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cosine, -sine, 0.0],
        [0.0, sine, cosine, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _rotation_y_matrix(angle: float) -> list[list[float]]:
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return [
        [cosine, 0.0, sine, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sine, 0.0, cosine, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _rotation_z_matrix(angle: float) -> list[list[float]]:
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return [
        [cosine, -sine, 0.0, 0.0],
        [sine, cosine, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _matrix_multiply(left: list[list[float]], right: list[list[float]]) -> list[list[float]]:
    return [
        [
            sum(left[row][idx] * right[idx][col] for idx in range(4))
            for col in range(4)
        ]
        for row in range(4)
    ]


def _projected_geometry_to_svg(
    projected: _AssemblyProjectedGeometry,
    footprint: Footprint,
    model: Model,
    *,
    mode: str,
    bbox: BoundingBox,
) -> list[str]:
    if mode == "detail":
        segments = projected.detail_line_segments or projected.simple_line_segments
        arcs = projected.detail_arcs or projected.simple_arcs
    else:
        segments = projected.simple_line_segments or projected.detail_line_segments
        arcs = projected.simple_arcs or projected.detail_arcs
    lines: list[str] = []
    for start, end in segments:
        x1, y1 = _model_point_to_svg(start, footprint, model, bbox)
        x2, y2 = _model_point_to_svg(end, footprint, model, bbox)
        lines.append(f'<line x1="{_fmt(x1)}" y1="{_fmt(y1)}" x2="{_fmt(x2)}" y2="{_fmt(y2)}"/>')
    for arc in arcs:
        rendered = _projected_arc_to_svg(arc, footprint, model, bbox)
        if rendered:
            lines.append(rendered)
    return lines


def _projected_arc_to_svg(
    arc: _AssemblyProjectedArc,
    footprint: Footprint,
    model: Model,
    bbox: BoundingBox,
) -> str:
    center = _model_point_to_svg(arc.center, footprint, model, bbox)
    start = _model_point_to_svg(arc.start, footprint, model, bbox)
    end = _model_point_to_svg(arc.end, footprint, model, bbox)
    if arc.full_circle:
        return f'<circle cx="{_fmt(center[0])}" cy="{_fmt(center[1])}" r="{_fmt(arc.radius)}"/>'
    large_arc = 1 if abs(float(arc.extent_rad)) > math.pi else 0
    sweep = 0 if arc.ccw else 1
    return (
        f'<path d="M {_fmt(start[0])} {_fmt(start[1])} '
        f'A {_fmt(arc.radius)} {_fmt(arc.radius)} 0 {large_arc} {sweep} '
        f'{_fmt(end[0])} {_fmt(end[1])}"/>'
    )


def _model_point_to_svg(
    point: tuple[float, float],
    footprint: Footprint,
    model: Model,
    bbox: BoundingBox,
) -> tuple[float, float]:
    offset = tuple(float(value) for value in getattr(model, "offset", (0.0, 0.0, 0.0)))
    local_x = float(point[0]) + offset[0]
    local_y = float(point[1]) + offset[1]
    angle = math.radians(-float(getattr(footprint, "at_angle", 0.0) or 0.0))
    cosine = math.cos(angle)
    sine = math.sin(angle)
    board_x = float(getattr(footprint, "at_x", 0.0) or 0.0) + local_x * cosine - local_y * sine
    board_y = float(getattr(footprint, "at_y", 0.0) or 0.0) + local_x * sine + local_y * cosine
    return board_x - float(bbox.min_x), board_y - float(bbox.min_y)


def _render_footprint_bounds_rect(
    footprint: Footprint,
    *,
    bbox: BoundingBox,
    color: str,
) -> str:
    bounds = footprint.get_bounds()
    x = float(bounds.min_x) - float(bbox.min_x)
    y = float(bounds.min_y) - float(bbox.min_y)
    width = float(bounds.max_x) - float(bounds.min_x)
    height = float(bounds.max_y) - float(bounds.min_y)
    return (
        f'<rect x="{_fmt(x)}" y="{_fmt(y)}" width="{_fmt(width)}" height="{_fmt(height)}" '
        f'stroke="{html.escape(color)}" fill="none"/>'
    )


def _render_a0_board_outputs(
    config: _PcbSvgConfig,
    input_file: Path,
    *,
    output_dir: Path,
) -> int:
    pcb, pcb_path = _load_kicad_pcb(input_file, config)
    board_name = pcb_path.stem
    layer_manifest: dict[str, object] = {}
    view_manifest: dict[str, object] = {}
    manifest: dict[str, object] = {
        "schema": "pcb.svg.manifest.a0",
        "board": board_name,
        "source_input": input_file.name,
        "source_pcb": pcb_path.name,
        "layer_outputs": layer_manifest,
        "views": view_manifest,
    }
    written = _render_a0_layer_outputs(
        config,
        pcb,
        output_dir=output_dir,
        board_name=board_name,
        layer_manifest=layer_manifest,
    )
    written += _render_a0_configured_views(
        config,
        pcb,
        pcb_path,
        output_dir=output_dir,
        board_name=board_name,
        view_manifest=view_manifest,
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = output_dir / f"{board_name}__views.json"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    log.info("Rendered PCB SVG A0 outputs for %s into %s", board_name, output_dir)
    return written + 1


def _render_pcb_svg_to_output(
    input_files: list[Path],
    output_dir: Path,
    config_by_input: dict[Path, _PcbSvgConfig],
) -> int:
    total_written = 0
    for input_file in input_files:
        resolved_input = input_file.resolve()
        config = config_by_input.get(resolved_input)
        if config is None:
            log.error("No pcb-svg config resolved for input: %s", resolved_input)
            return 1
        try:
            total_written += _render_a0_board_outputs(
                config,
                resolved_input,
                output_dir=output_dir,
            )
        except Exception as exc:
            log.error("PCB SVG generation failed for %s: %s", input_file.name, exc)
            return 1
    log.info("Successfully generated %s PCB SVG artifact file(s)", total_written)
    return 0


def cmd_pcb_svg(args: argparse.Namespace) -> int:
    """Handle pcb-svg subcommand."""
    input_files = _resolve_explicit_input(str(args.file) if args.file else None)
    if input_files is None:
        return 1
    if any(not _validate_input_file(input_file) for input_file in input_files):
        return 1
    try:
        config_by_input, created_configs = _resolve_pcb_svg_configs(args, input_files)
    except ValueError as exc:
        log.error(str(exc))
        return 1
    if created_configs:
        for config_path in created_configs:
            log.info("Created pcb-svg config template: %s", config_path)
        log.info("pcb-svg config template created and defaulted for this invocation.")
    output_dir = resolve_output_dir(args.output, "pcb-svg")
    return _render_pcb_svg_to_output(input_files, output_dir, config_by_input)


def add_pcb_svg_option_arguments(parser: argparse.ArgumentParser) -> None:
    """Add shared PCB SVG flags to the pcb-svg command."""
    parser.add_argument(
        "--config",
        type=Path,
        help=(
            f"path to {PCB_SVG_CONFIG_SCHEMA} JSON/JSONC config. If omitted, pcb-svg "
            f"uses {PCB_SVG_CONFIG_FILENAME} next to each input file, creating "
            "a template when missing."
        ),
    )
    parser.add_argument(
        "--doc",
        "--pcbdoc",
        dest="pcbdoc",
        type=str,
        help="with .kicad_pro input, select a specific .kicad_pcb by filename, stem, or path",
    )
    parser.add_argument(
        "--views",
        "--pcb-views",
        dest="pcb_views",
        type=str,
        help="comma-separated view names plus layers, all, or none",
    )
    parser.add_argument(
        "--layers",
        "--pcb-layers",
        dest="pcb_layers",
        type=str,
        help="comma-separated physical PCB layers for layer outputs",
    )
    parser.add_argument(
        "--scale",
        "--pcb-svg-scale",
        dest="pcb_svg_scale",
        type=float,
        default=None,
        help=f"SVG display scale for A0 outputs (default: config or {PCB_DEFAULT_SVG_SCALE})",
    )
    parser.add_argument(
        "--size-unit",
        "--pcb-svg-size-unit",
        dest="pcb_svg_size_unit",
        type=str,
        default=None,
        help="SVG width/height unit suffix for A0 outputs",
    )
    parser.add_argument(
        "--clean-output",
        dest="pcb_clean_output",
        action="store_true",
        help="request clean output behavior in the resolved config",
    )


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the pcb-svg command parser."""
    parser = subparsers.add_parser(
        "pcb-svg",
        help="generate PCB SVG layer outputs and configured design views",
        description=(
            "Generate KiCad PCB SVG layer outputs and configured A0 design views "
            "from .kicad_pcb or .kicad_pro files. The command uses pcb.svg.config "
            "JSON/JSONC configs and supports geometer-backed assembly HLR overlays."
        ),
        epilog=(
            "Examples:\n"
            "  kicad-cruncher pcb-svg board.kicad_pcb\n"
            "  kicad-cruncher pcb-svg project.kicad_pro --views assembly-top\n"
            "  kicad-cruncher pcb-svg board.kicad_pcb --config pcb.svg.config -o output/pcb-svg\n"
            "  kicad-cruncher pcb-svg board.kicad_pcb --layers F.Cu,Edge.Cuts --views layers"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "file",
        nargs="?",
        help="KiCad PCB or project file; optional when one .kicad_pro/.kicad_pcb is in CWD",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="output directory (default: ./output/pcb-svg)",
    )
    add_pcb_svg_option_arguments(parser)
    parser.set_defaults(handler=cmd_pcb_svg)
    return parser


__all__ = [
    "add_pcb_svg_option_arguments",
    "cmd_pcb_svg",
    "register_parser",
]

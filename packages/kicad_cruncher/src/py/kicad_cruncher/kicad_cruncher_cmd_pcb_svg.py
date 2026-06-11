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
import re
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING, Literal, cast

from kicad_cruncher.config_json import load_json_config
from kicad_cruncher.kicad_cruncher_common import resolve_output_dir
from kicad_cruncher.kicad_cruncher_pcb_model_pose import (
    Matrix4,
    board_world_to_svg,
    kicad_model_pose,
    model_bounds_to_svg_rect,
    transform_footprint_local_to_board,
)
from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
    PcbSvgComposition,
    render_pcb_svg_composition,
)
from kicad_cruncher.kicad_cruncher_pcb_svg_config import (
    PCB_DEFAULT_SVG_SCALE,
    PCB_SVG_ASSEMBLY_VIRTUAL_LAYERS,
    PCB_SVG_CONFIG_FILENAME,
    PCB_SVG_CONFIG_SCHEMA,
    _PcbSvgConfig,
    _PcbSvgViewConfig,
    is_synthetic_layer_token,
    normalize_layer_token,
    parse_pcb_layer_selector,
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
    "cutout": "board_cutouts",
    "cutouts": "board_cutouts",
    "board-cutouts": "board_cutouts",
    "board_cutouts": "board_cutouts",
    "top-pin1": "top_pin1_view",
    "top-pin-1": "top_pin1_view",
    "pin1-top": "top_pin1_view",
    "pin-1-top": "top_pin1_view",
    "bottom-pin1": "bottom_pin1_view",
    "bottom-pin-1": "bottom_pin1_view",
    "pin1-bottom": "bottom_pin1_view",
    "pin-1-bottom": "bottom_pin1_view",
    "top-hlr-bounds": "top_hlr_bounding_boxes",
    "top-hlr-bounding-boxes": "top_hlr_bounding_boxes",
    "hlr-bounds-top": "top_hlr_bounding_boxes",
    "bottom-hlr-bounds": "bottom_hlr_bounding_boxes",
    "bottom-hlr-bounding-boxes": "bottom_hlr_bounding_boxes",
    "hlr-bounds-bottom": "bottom_hlr_bounding_boxes",
    "assembly-top": "assembly_top_view",
    "assembly-bottom": "assembly_bottom_view",
}
_MERGED_DEFAULT_VIEW_ALIASES = {
    "top_view": ("assembly_top_view",),
    "bottom_view": ("assembly_bottom_view",),
    "top_pin1_view": ("assembly_top_view",),
    "bottom_pin1_view": ("assembly_bottom_view",),
    "board_cutouts": ("assembly_top_view", "assembly_bottom_view"),
}

_HLR_TOKENS = set(PCB_SVG_ASSEMBLY_VIRTUAL_LAYERS)
_DESIGNATOR_TOKENS = {"ASSEMBLY_DESIGNATORS_TOP", "ASSEMBLY_DESIGNATORS_BOTTOM"}
_DESIGNATOR_RANGE_RE = re.compile(r"^([A-Za-z]+)(\d+)-([A-Za-z]*)(\d+)$")
_DESIGNATOR_NUMBER_RE = re.compile(r"^([A-Za-z]+)(\d+)$")
_ASSEMBLY_TOKEN_MODE_BY_TOKEN = {
    "ASSEMBLY_HLR_TOP_OUTLINE": ("top", "outline"),
    "ASSEMBLY_HLR_TOP_DETAIL": ("top", "detail"),
    "ASSEMBLY_HLR_BOTTOM_OUTLINE": ("bottom", "outline"),
    "ASSEMBLY_HLR_BOTTOM_DETAIL": ("bottom", "detail"),
    "ASSEMBLY_BOUNDS_TOP_MODEL": ("top", "model_bounds"),
    "ASSEMBLY_BOUNDS_BOTTOM_MODEL": ("bottom", "model_bounds"),
    "ASSEMBLY_BOUNDS_TOP_PADS": ("top", "pad_bounds"),
    "ASSEMBLY_BOUNDS_BOTTOM_PADS": ("bottom", "pad_bounds"),
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
_PCB_SVG_ASSEMBLY_DESIGNATORS_TOP_LAYER_ID = 9006
_PCB_SVG_ASSEMBLY_DESIGNATORS_BOTTOM_LAYER_ID = 9007
_MODEL_BOUNDS_CACHE: dict[tuple[str, tuple[float, ...]], dict[str, object]] = {}


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
    header = (
        "/*\n"
        "kicad-cruncher pcb-svg configuration\n"
        "\n"
        "This file is JSONC: block comments, line comments, and trailing commas\n"
        "are accepted. The generated template keeps all explanatory comments in\n"
        "this top block so the configuration body stays easy to scan.\n"
        "\n"
        f"Schema: {PCB_SVG_CONFIG_SCHEMA}\n"
        "\n"
        "Common physical layer tokens:\n"
        "  TOP, BOTTOM, TOPOVERLAY, BOTTOMOVERLAY, TOPPASTE, BOTTOMPASTE,\n"
        "  TOPSOLDER, BOTTOMSOLDER, F.Cu, B.Cu, F.SilkS, B.SilkS, F.Fab,\n"
        "  B.Fab, Edge.Cuts.\n"
        "\n"
        "Virtual layer tokens:\n"
        "  BOARD_OUTLINE - board perimeter derived from Edge.Cuts.\n"
        "  BOARD_CUTOUTS - internal cutout regions derived from Edge.Cuts.\n"
        "  DRILLS - through-hole and non-plated circular drill overlays.\n"
        "  SLOTS - slotted through-hole and non-plated slot overlays.\n"
        "  PIN1_TOP - top-side pin-1 marker overlay.\n"
        "  PIN1_BOTTOM - bottom-side pin-1 marker overlay.\n"
        "  ASSEMBLY_HLR_TOP - top-side assembly overlay using the view HLR mode.\n"
        "  ASSEMBLY_HLR_BOTTOM - bottom-side assembly overlay using the view HLR mode.\n"
        "  ASSEMBLY_HLR_TOP_OUTLINE - top-side Geometer STEP outline output.\n"
        "  ASSEMBLY_HLR_TOP_DETAIL - top-side Geometer detailed STEP projection output.\n"
        "  ASSEMBLY_HLR_BOTTOM_OUTLINE - bottom-side Geometer STEP outline output.\n"
        "  ASSEMBLY_HLR_BOTTOM_DETAIL - bottom-side Geometer detailed STEP projection output.\n"
        "  ASSEMBLY_BOUNDS_TOP_MODEL - top-side transformed STEP model bounds.\n"
        "  ASSEMBLY_BOUNDS_BOTTOM_MODEL - bottom-side transformed STEP model bounds.\n"
        "  ASSEMBLY_BOUNDS_TOP_PADS - top-side pad/hole bounds.\n"
        "  ASSEMBLY_BOUNDS_BOTTOM_PADS - bottom-side pad/hole bounds.\n"
        "  ASSEMBLY_DESIGNATORS_TOP - top-side assembly reference designator overlay.\n"
        "  ASSEMBLY_DESIGNATORS_BOTTOM - bottom-side assembly reference designator overlay.\n"
        "\n"
        "Views and draw order:\n"
        "  Each view has its own layers array, assembly_hlr_mode, styles, and pin1\n"
        "  settings. The layers array is the draw order. KiCad physical layers are\n"
        "  rendered by kicad-monkey; ASSEMBLY_HLR_* layers add Geometer STEP HLR\n"
        "  overlays from embedded or resolvable STEP models.\n"
        "  styles.board_outline.max_arc_segment_mm, max_curve_segment_mm, and\n"
        "  max_circle_segment_mm control smoothing of derived BOARD_OUTLINE and\n"
        "  BOARD_CUTOUTS paths; smaller values create smoother, larger SVG paths.\n"
        "\n"
        "Per-view override resolution:\n"
        "  Global styles/config are merged with each view.styles and view.pin1 while\n"
        "  rendering that view. components.<designator>.projection,\n"
        "  components.<designator>.assembly_hlr, and\n"
        "  components.<designator>.assembly_designators are evaluated per view over\n"
        "  that view's resolved mode/styles. This means one component override can\n"
        "  suppress or restyle HLR in a view that renders HLR while another view\n"
        "  continues to use its own resolved settings. For designators,\n"
        "  styles.assembly_designators.selector_overrides apply after global/view\n"
        "  style resolution and before exact components.<designator> overrides.\n"
        "\n"
        "HLR modes:\n"
        "  bounding_box - transformed STEP model bounds, falling back to pad bounds.\n"
        "  model_bounds - transformed STEP model bounds rectangle only.\n"
        "  pad_bounds   - footprint pad/hole bounds rectangle only; no STEP projection.\n"
        "  outline      - Geometer assembly outline, with hole-first bounds fallback\n"
        "                 when no STEP model is available.\n"
        "  detail       - Geometer detailed visible projection, with hole-first\n"
        "                 bounds fallback when no STEP model is available.\n"
        "  none         - suppress HLR projection. Legacy simple values are accepted\n"
        "                 as aliases for outline.\n"
        "\n"
        "Assembly HLR style override example, globally, inside any view.styles,\n"
        "or inside components.<designator>.assembly_hlr for a single part:\n"
        '  "assembly_hlr": {\n'
        '    "enabled": true,\n'
        '    "color": "#F59E0B",\n'
        '    "line_width_mm": 0.12,\n'
        '    "projection_algorithm": "exact",\n'
        '    "curve_mode": "native_arcs",\n'
        '    "samples_per_curve": 24,\n'
        '    "round_digits": 3,\n'
        '    "include_visible": true,\n'
        '    "include_outline": true,\n'
        '    "outline_algorithm": "mesh-shadow",\n'
        '    "union_polygons": true,\n'
        '    "mesh_linear_deflection": 0.01,\n'
        '    "mesh_angular_deflection": 0.5,\n'
        '    "mesh_relative": false,\n'
        '    "hlr_angle_tolerance": 0.0174533\n'
        "  }\n"
        "\n"
        "Component override examples:\n"
        '  "components": {\n'
        '    "J1": {"projection": "none"},\n'
        '    "U5": {"projection": "detail", "assembly_hlr": {"color": "#2563EB"}},\n'
        '    "R12": {"assembly_designators": {\n'
        '      "opacity": 0.65,\n'
        '      "rotation_aspect_threshold": 1.2,\n'
        '      "rotation_direction": "cw"\n'
        "    }}\n"
        "  }\n"
        "\n"
        "Pin-1 exclusion examples, globally or inside a view.pin1 object:\n"
        '  "pin1": {\n'
        '    "exclude_single_pin": true,\n'
        '    "exclude_designators": ["R", "C", "U1", "U5-U15", "J*"]\n'
        "  }\n"
        "\n"
        "Assembly designator views use assembly_designators style controls and fit\n"
        "text inside pad/model bounds according to the view projection mode. Text\n"
        "rotates 90 degrees when bounds height/width exceeds\n"
        "assembly_designators.rotation_aspect_threshold, default 1.5.\n"
        "assembly_designators.rotation_direction selects cw or ccw, and\n"
        "assembly_designators.selector_overrides can target ranges/groups.\n"
        "\n"
        "Layer outputs:\n"
        "  Set layer_outputs.enabled=false if you only want composed views.\n"
        "  layer_outputs.add_*_to_physical_layers controls per-layer context;\n"
        "  raw Edge.Cuts plus computed DRILLS/SLOTS overlays are included by default.\n"
        "  layer_outputs.write_virtual_layers controls standalone __virtual__ debug\n"
        "  outputs; include_special_layers selects which virtual layer files are\n"
        "  written.\n"
        "*/\n"
    )
    return f"{header}{payload}\n"


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
    requested_views: set[str] = set()
    for view_name in selected - {"layers", "none"}:
        if view_name in known_names:
            requested_views.add(view_name)
        else:
            requested_views.update(_MERGED_DEFAULT_VIEW_ALIASES.get(view_name, (view_name,)))
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
            "No file specified and no single .kicad_pro/.kicad_pcb found in current directory"
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


def _layer_output_tokens(config: _PcbSvgConfig, pcb: KiCadPcb) -> list[str]:
    configured = config.layer_outputs.get("layers", "auto")
    if configured == "auto":
        layers = [_pcb_layer_name(layer) for layer in getattr(pcb, "layers", [])]
        return [layer for layer in layers if layer]
    if not isinstance(configured, list):
        return []
    return [normalize_layer_token(str(token)) for token in configured]


def _pcb_layer_name(layer: object) -> str:
    return str(getattr(layer, "canonical_name", None) or getattr(layer, "name", None) or "")


def _layer_output_bool(config: _PcbSvgConfig, key: str, default: bool) -> bool:
    value = config.layer_outputs.get(key, default)
    if value is None:
        return default
    if isinstance(value, bool):
        return value
    if isinstance(value, int | float):
        return bool(value)
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in {"1", "true", "yes", "on"}:
            return True
        if normalized in {"0", "false", "no", "off"}:
            return False
    raise ValueError(f"Invalid pcb-svg layer_outputs.{key} boolean value: {value!r}")


def _layer_output_special_tokens(config: _PcbSvgConfig) -> list[str]:
    if not _layer_output_bool(config, "write_virtual_layers", True):
        return []
    raw_include_special = config.layer_outputs.get("include_special_layers", [])
    if not isinstance(raw_include_special, list):
        return []
    return [normalize_layer_token(str(token)) for token in raw_include_special]


def _physical_layer_context_tokens(config: _PcbSvgConfig, layer_token: str) -> list[str]:
    if layer_token == "Edge.Cuts":
        return []
    tokens: list[str] = []
    if _layer_output_bool(config, "add_edge_cuts_to_physical_layers", True):
        tokens.append("Edge.Cuts")
    if _layer_output_bool(config, "add_drills_to_physical_layers", True):
        tokens.append("DRILLS")
    if _layer_output_bool(config, "add_slots_to_physical_layers", True):
        tokens.append("SLOTS")
    return tokens


def _append_unique_token(tokens: list[str], token: str) -> None:
    if token not in tokens:
        tokens.append(token)


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
    pcb_path: Path,
    *,
    output_dir: Path,
    board_name: str,
    layer_manifest: dict[str, object],
) -> int:
    if not bool(config.layer_outputs.get("enabled", True)):
        return 0
    layer_dir = output_dir / str(config.layer_outputs.get("output_dir") or "layers")
    physical_tokens, virtual_tokens = _layer_output_token_groups(config, pcb)

    written = 0
    for layer_token in physical_tokens:
        written += _write_physical_layer_output(
            pcb,
            config=config,
            layer_token=layer_token,
            layer_dir=layer_dir,
            output_dir=output_dir,
            board_name=board_name,
            layer_manifest=layer_manifest,
        )
    for layer_token in virtual_tokens:
        written += _write_virtual_layer_output(
            pcb,
            pcb_path,
            config=config,
            layer_token=layer_token,
            layer_dir=layer_dir,
            output_dir=output_dir,
            board_name=board_name,
            layer_manifest=layer_manifest,
        )
    return written


def _layer_output_token_groups(config: _PcbSvgConfig, pcb: KiCadPcb) -> tuple[list[str], list[str]]:
    physical_tokens: list[str] = []
    virtual_tokens: list[str] = []
    write_virtual_layers = _layer_output_bool(config, "write_virtual_layers", True)
    for layer_token in _layer_output_tokens(config, pcb):
        if is_synthetic_layer_token(layer_token):
            if write_virtual_layers:
                _append_unique_token(virtual_tokens, layer_token)
            continue
        _append_unique_token(physical_tokens, layer_token)
    for layer_token in _layer_output_special_tokens(config):
        _append_unique_token(virtual_tokens, layer_token)
    return physical_tokens, virtual_tokens


def _write_physical_layer_output(
    pcb: KiCadPcb,
    *,
    config: _PcbSvgConfig,
    layer_token: str,
    layer_dir: Path,
    output_dir: Path,
    board_name: str,
    layer_manifest: dict[str, object],
) -> int:
    group_id = f"pcb-svg-layer-{_safe_svg_id(layer_token.lower())}"
    view_layers = [layer_token, *_physical_layer_context_tokens(config, layer_token)]
    composition = render_pcb_svg_composition(
        pcb,
        view_layers,
        styles=config.global_options.styles,
        group_id=group_id,
        config=config,
    )
    if not composition.physical_layers and not bool(config.global_options.show_empty_layers):
        return 0
    layer_path = layer_dir / f"{board_name}__{_safe_svg_id(layer_token)}.svg"
    layer_path.parent.mkdir(parents=True, exist_ok=True)
    layer_path.write_text(composition.svg_text, encoding="utf-8")
    layer_manifest[layer_token] = {
        "file": str(layer_path.relative_to(output_dir)).replace("\\", "/"),
        "layers": view_layers,
        "context_layers": view_layers[1:],
        "physical_layers": composition.physical_layers,
        "group_id": group_id,
    }
    return 1


def _write_virtual_layer_output(
    pcb: KiCadPcb,
    pcb_path: Path,
    *,
    config: _PcbSvgConfig,
    layer_token: str,
    layer_dir: Path,
    output_dir: Path,
    board_name: str,
    layer_manifest: dict[str, object],
) -> int:
    group_id = f"pcb-svg-layer-virtual-{_safe_svg_id(layer_token.lower())}"
    view_layers = [layer_token]
    composition = _render_virtual_layer_composition(
        pcb,
        pcb_path,
        layer_token,
        group_id=group_id,
        config=config,
    )
    layer_path = layer_dir / f"{board_name}__virtual__{_safe_svg_id(layer_token.lower())}.svg"
    layer_path.parent.mkdir(parents=True, exist_ok=True)
    layer_path.write_text(composition.svg_text, encoding="utf-8")
    layer_manifest[layer_token] = {
        "file": str(layer_path.relative_to(output_dir)).replace("\\", "/"),
        "layers": view_layers,
        "physical_layers": composition.physical_layers,
        "group_id": group_id,
        "virtual": True,
    }
    return 1


def _render_virtual_layer_composition(
    pcb: KiCadPcb,
    pcb_path: Path,
    layer_token: str,
    *,
    group_id: str,
    config: _PcbSvgConfig,
) -> PcbSvgComposition:
    token = normalize_layer_token(layer_token)
    if token in _DESIGNATOR_TOKENS:
        return _render_assembly_designator_virtual_layer_output(
            pcb,
            pcb_path,
            token,
            group_id=group_id,
            styles=config.global_options.styles,
            config=config,
        )
    if _is_assembly_virtual_layer_token(token):
        return _render_assembly_virtual_layer_output(
            pcb,
            pcb_path,
            token,
            group_id=group_id,
            styles=config.global_options.styles,
            config=config,
        )
    return render_pcb_svg_composition(
        pcb,
        [layer_token],
        styles=config.global_options.styles,
        group_id=group_id,
        config=config,
    )


def _is_assembly_virtual_layer_token(token: str) -> bool:
    return normalize_layer_token(token) in _HLR_TOKENS


def _render_assembly_virtual_layer_output(
    pcb: KiCadPcb,
    pcb_path: Path,
    layer_token: str,
    *,
    group_id: str,
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
) -> PcbSvgComposition:
    token = normalize_layer_token(layer_token)
    _, mode = _assembly_token_projection(token, config.assembly.default_projection)
    composition = render_pcb_svg_composition(
        pcb,
        ["BOARD_OUTLINE"],
        styles=styles,
        group_id=group_id,
        config=config,
    )
    view = _PcbSvgViewConfig(
        name=f"virtual_{token.lower()}",
        group_id=group_id,
        layers=[token],
        assembly_hlr_mode=mode,
    )
    overlay = _render_assembly_hlr_overlay(
        pcb,
        pcb_path,
        view,
        styles=styles,
        config=config,
        mirror=False,
    )
    svg_text = _insert_svg_overlay(composition.svg_text, overlay)
    return PcbSvgComposition(svg_text=svg_text, physical_layers=composition.physical_layers)


def _render_assembly_designator_virtual_layer_output(
    pcb: KiCadPcb,
    pcb_path: Path,
    layer_token: str,
    *,
    group_id: str,
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
) -> PcbSvgComposition:
    token = normalize_layer_token(layer_token)
    composition = render_pcb_svg_composition(
        pcb,
        ["BOARD_OUTLINE"],
        styles=styles,
        group_id=group_id,
        config=config,
    )
    view = _PcbSvgViewConfig(
        name=f"virtual_{token.lower()}",
        group_id=group_id,
        layers=[token],
        assembly_hlr_mode=config.assembly.default_projection,
    )
    overlay = _render_assembly_designator_overlay(
        pcb,
        pcb_path,
        view,
        styles=styles,
        config=config,
    )
    svg_text = _insert_svg_overlay(composition.svg_text, overlay)
    return PcbSvgComposition(svg_text=svg_text, physical_layers=composition.physical_layers)


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
            config=config,
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
    config: _PcbSvgConfig,
) -> str:
    composition = render_pcb_svg_composition(
        pcb,
        view.layers,
        styles=styles,
        group_id=group_id,
        config=config,
        pin1_config=config.resolved_pin1_for_view(view),
    )
    svg_text = composition.svg_text
    overlays = [
        _render_assembly_hlr_overlay(
            pcb,
            pcb_path,
            view,
            styles=styles,
            config=config,
            mirror=mirror,
        ),
        _render_assembly_designator_overlay(
            pcb,
            pcb_path,
            view,
            styles=styles,
            config=config,
        ),
    ]
    overlay = "\n".join(piece for piece in overlays if piece)
    if not overlay:
        return svg_text
    return _insert_svg_overlay(svg_text, overlay)


def _render_assembly_hlr_overlay(
    pcb: KiCadPcb,
    pcb_path: Path,
    view: _PcbSvgViewConfig,
    *,
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
    mirror: bool,
) -> str:
    del mirror
    hlr_tokens = _hlr_tokens_for_view(view)
    if not _should_render_assembly_hlr(view, styles, hlr_tokens, config=config):
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
            config=config,
            bbox=bbox,
            color=color,
            line_width=line_width,
        )
        for token in hlr_tokens
    ]
    return "\n" + "\n".join(pieces)


def _render_assembly_designator_overlay(
    pcb: KiCadPcb,
    pcb_path: Path,
    view: _PcbSvgViewConfig,
    *,
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
) -> str:
    tokens = _designator_tokens_for_view(view)
    if not tokens or not _style_enabled(styles, "assembly_designators"):
        return ""
    bbox = _compute_pcb_svg_bbox(pcb)
    if bbox.is_empty:
        return ""
    pieces = [
        _render_assembly_designator_token_group(
            pcb,
            pcb_path,
            token,
            view=view,
            styles=styles,
            config=config,
            bbox=bbox,
        )
        for token in tokens
    ]
    return "\n" + "\n".join(pieces)


def _render_assembly_designator_token_group(
    pcb: KiCadPcb,
    pcb_path: Path,
    token: str,
    *,
    view: _PcbSvgViewConfig,
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
    bbox: BoundingBox,
) -> str:
    side = "bottom" if normalize_layer_token(token) == "ASSEMBLY_DESIGNATORS_BOTTOM" else "top"
    layer_id = (
        _PCB_SVG_ASSEMBLY_DESIGNATORS_BOTTOM_LAYER_ID
        if side == "bottom"
        else _PCB_SVG_ASSEMBLY_DESIGNATORS_TOP_LAYER_ID
    )
    group_lines = [
        (
            f'<g id="assembly-designators-{side}" data-layer-id="{layer_id}" '
            f'data-layer-token="{html.escape(token)}" '
            f'data-feature="assembly-designators">'
        )
    ]
    for footprint in _pcb_footprints(pcb):
        if _footprint_side(footprint) != side:
            continue
        designator = _footprint_designator(footprint)
        override = config.components.get(designator)
        if override and override.show_designator is False:
            continue
        component_styles = _component_designator_styles(
            styles,
            config=config,
            designator=designator,
        )
        if not _style_enabled(component_styles, "assembly_designators"):
            continue
        projection = _component_projection_mode(
            view.assembly_hlr_mode,
            config=config,
            designator=designator,
        )
        rect, bounds_kind = _designator_bounds_rect(
            pcb,
            pcb_path,
            footprint,
            projection_mode=projection,
            bbox=bbox,
        )
        if rect is None:
            continue
        group_lines.append(
            _svg_assembly_designator_text(
                designator,
                rect,
                bounds_kind=bounds_kind,
                projection_mode=projection,
                token=token,
                styles=component_styles,
            )
        )
    group_lines.append("</g>")
    return "\n".join(group_lines)


def _designator_bounds_rect(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    projection_mode: str,
    bbox: BoundingBox,
) -> tuple[tuple[float, float, float, float] | None, str]:
    if projection_mode == "pad_bounds":
        return _footprint_pad_bounds_rect_values(footprint, bbox=bbox), "pads"
    if projection_mode == "none":
        rect = _footprint_pad_bounds_rect_values(footprint, bbox=bbox)
        return rect, "pads"
    model_rect = _footprint_model_bounds_rect_values(pcb, pcb_path, footprint, bbox=bbox)
    if model_rect is not None:
        return model_rect, "model"
    hole_rect = _footprint_hole_bounds_rect_values(footprint, bbox=bbox)
    if hole_rect is not None:
        return hole_rect, "holes"
    return _footprint_pad_bounds_rect_values(footprint, bbox=bbox), "pads"


def _svg_assembly_designator_text(
    designator: str,
    rect: tuple[float, float, float, float],
    *,
    bounds_kind: str,
    projection_mode: str,
    token: str,
    styles: dict[str, dict[str, object]],
) -> str:
    metrics = _assembly_designator_text_metrics(designator, rect, styles)
    if metrics is None:
        return ""
    cx, cy, font_size, rotation = metrics
    color = _style_color(styles, "assembly_designators", "#2563EB")
    font_family = _assembly_designator_font_family(styles)
    font_weight = _assembly_designator_font_weight(styles)
    opacity = _style_float(styles, "assembly_designators", "opacity", 1.0)
    transform = f' transform="rotate({_fmt(rotation)} {_fmt(cx)} {_fmt(cy)})"' if rotation else ""
    weight_attr = f' font-weight="{html.escape(font_weight)}"' if font_weight else ""
    return (
        f'<text x="{_fmt(cx)}" y="{_fmt(cy)}"{transform} '
        f'font-size="{_fmt(font_size)}" text-anchor="middle" '
        f'dominant-baseline="central" fill="{html.escape(color)}" '
        f'font-family="{html.escape(font_family)}"{weight_attr} '
        f'opacity="{_fmt(opacity)}" '
        f'data-layer-token="{html.escape(token)}" '
        f'data-primitive="assembly-designator" '
        f'data-component="{html.escape(designator)}" '
        f'data-bounds-kind="{html.escape(bounds_kind)}" '
        f'data-projection="{html.escape(projection_mode)}">'
        f"{html.escape(designator)}</text>"
    )


def _assembly_designator_text_metrics(
    designator: str,
    rect: tuple[float, float, float, float],
    styles: dict[str, dict[str, object]],
) -> tuple[float, float, float, int] | None:
    x, y, width, height = rect
    if width <= 0.0 or height <= 0.0:
        return None
    rotation = _assembly_designator_rotation(rect, styles)
    available_width, available_height = _assembly_designator_available_text_box(
        width,
        height,
        rotation,
        styles,
    )
    font_size = _assembly_designator_font_size(
        designator,
        available_width,
        available_height,
        styles,
    )
    if font_size <= 0.0 or available_width <= 0.0:
        return None
    return x + width / 2.0, y + height / 2.0, font_size, rotation


def _assembly_designator_available_text_box(
    width: float,
    height: float,
    rotation: int,
    styles: dict[str, dict[str, object]],
) -> tuple[float, float]:
    fill_ratio = min(
        1.0,
        max(0.05, _style_float(styles, "assembly_designators", "box_fill_ratio", 0.80)),
    )
    text_width = height if rotation else width
    text_height = width if rotation else height
    return text_width * fill_ratio, text_height * fill_ratio


def _assembly_designator_font_size(
    designator: str,
    available_width: float,
    available_height: float,
    styles: dict[str, dict[str, object]],
) -> float:
    min_font = _style_float(styles, "assembly_designators", "min_font_size_mm", 0.35)
    max_font = _style_float(styles, "assembly_designators", "max_font_size_mm", 2.5)
    estimated_font = available_width / max(len(designator) * 0.62, 1.0)
    font_size = min(max_font, available_height, estimated_font)
    if font_size < min_font:
        return min(min_font, available_height, estimated_font)
    return font_size


def _assembly_designator_font_family(styles: dict[str, dict[str, object]]) -> str:
    return str(
        styles.get("assembly_designators", {}).get("font_family") or "Arial, sans-serif"
    )


def _assembly_designator_font_weight(styles: dict[str, dict[str, object]]) -> str:
    return str(styles.get("assembly_designators", {}).get("font_weight") or "")


def _assembly_designator_rotation(
    rect: tuple[float, float, float, float],
    styles: dict[str, dict[str, object]],
) -> int:
    _x, _y, width, height = rect
    if width <= 0.0:
        return 0
    threshold = _style_float(
        styles,
        "assembly_designators",
        "rotation_aspect_threshold",
        1.5,
    )
    return _assembly_designator_rotation_direction(styles) if height / width > threshold else 0


def _assembly_designator_rotation_direction(
    styles: dict[str, dict[str, object]],
) -> int:
    raw = styles.get("assembly_designators", {}).get("rotation_direction", "ccw")
    direction = str(raw).strip().lower()
    if direction in {"cw", "clockwise", "right", "+90", "90"}:
        return 90
    if direction in {"ccw", "counterclockwise", "counter-clockwise", "left", "-90"}:
        return -90
    return -90


def _designator_tokens_for_view(view: _PcbSvgViewConfig) -> list[str]:
    return [
        normalized
        for token in view.layers
        if (normalized := normalize_layer_token(token)) in _DESIGNATOR_TOKENS
    ]


def _insert_svg_overlay(svg_text: str, overlay: str) -> str:
    if not overlay:
        return svg_text
    insert_at = svg_text.rfind("</svg>")
    if insert_at < 0:
        return svg_text + "\n" + overlay
    return svg_text[:insert_at] + overlay + "\n" + svg_text[insert_at:]


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
    *,
    config: _PcbSvgConfig,
) -> bool:
    if not hlr_tokens or not _style_enabled(styles, "assembly_hlr"):
        return False
    return any(
        _assembly_token_projection(token, view.assembly_hlr_mode)[1] != "none"
        for token in hlr_tokens
    ) or any(
        override.projection and override.projection != "none"
        for override in config.components.values()
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
    config: _PcbSvgConfig,
    bbox: BoundingBox,
    color: str,
    line_width: float,
) -> str:
    side, mode = _assembly_token_projection(token, view.assembly_hlr_mode)
    group_lines = [
        (
            f'<g id="assembly-overlay" data-layer-id="{_assembly_hlr_layer_id(side)}" '
            f'data-layer-token="{html.escape(token)}" '
            f'data-assembly-symbol="{html.escape(mode)}" '
            f'stroke="{html.escape(color)}" stroke-width="{_fmt(line_width)}" '
            'fill="none" stroke-linecap="round" stroke-linejoin="round">'
        )
    ]
    for footprint in _pcb_footprints(pcb):
        if _footprint_side(footprint) != side:
            continue
        designator = _footprint_designator(footprint)
        component_styles = _component_hlr_styles(styles, config=config, designator=designator)
        if not _style_enabled(component_styles, "assembly_hlr"):
            continue
        component_color = _style_color(component_styles, "assembly_hlr", color)
        component_line_width = _style_float(
            component_styles,
            "assembly_hlr",
            "line_width_mm",
            line_width,
        )
        component_opacity = _style_float(
            component_styles,
            "assembly_hlr",
            "opacity",
            0.75,
        )
        component_mode = _component_projection_mode(
            mode,
            config=config,
            designator=designator,
        )
        group_lines.extend(
            _render_footprint_hlr(
                pcb,
                pcb_path,
                footprint,
                designator=designator,
                side=side,
                mode=component_mode,
                styles=component_styles,
                color=component_color,
                line_width=component_line_width,
                opacity=component_opacity,
                bbox=bbox,
            )
        )
    group_lines.append("</g>")
    return "\n".join(group_lines)


def _assembly_token_projection(token: str, fallback_mode: str) -> tuple[str, str]:
    normalized = normalize_layer_token(token)
    if normalized == "ASSEMBLY_HLR_TOP":
        return "top", fallback_mode
    if normalized == "ASSEMBLY_HLR_BOTTOM":
        return "bottom", fallback_mode
    return _ASSEMBLY_TOKEN_MODE_BY_TOKEN.get(normalized, ("top", fallback_mode))


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


def _component_projection_mode(
    default_mode: str,
    *,
    config: _PcbSvgConfig,
    designator: str,
) -> str:
    override = config.components.get(designator)
    return override.projection if override and override.projection else default_mode


def _component_hlr_styles(
    styles: dict[str, dict[str, object]],
    *,
    config: _PcbSvgConfig,
    designator: str,
) -> dict[str, dict[str, object]]:
    override = config.components.get(designator)
    if not override or not override.assembly_hlr:
        return styles
    merged = {name: dict(style) for name, style in styles.items()}
    assembly_hlr = dict(merged.get("assembly_hlr", {}))
    assembly_hlr.update(override.assembly_hlr)
    merged["assembly_hlr"] = assembly_hlr
    return merged


def _component_designator_styles(
    styles: dict[str, dict[str, object]],
    *,
    config: _PcbSvgConfig,
    designator: str,
) -> dict[str, dict[str, object]]:
    override = config.components.get(designator)
    selector_overrides = _matching_assembly_designator_selector_overrides(
        designator,
        styles,
    )
    if not selector_overrides and (not override or not override.assembly_designators):
        return styles
    merged = {name: dict(style) for name, style in styles.items()}
    assembly_designators = dict(merged.get("assembly_designators", {}))
    for selector_override in selector_overrides:
        assembly_designators.update(selector_override)
    if override and override.assembly_designators:
        assembly_designators.update(override.assembly_designators)
    merged["assembly_designators"] = assembly_designators
    return merged


def _matching_assembly_designator_selector_overrides(
    designator: str,
    styles: dict[str, dict[str, object]],
) -> list[dict[str, object]]:
    raw_overrides = styles.get("assembly_designators", {}).get("selector_overrides")
    if not isinstance(raw_overrides, Mapping):
        return []
    matches: list[dict[str, object]] = []
    for selector, raw_style in raw_overrides.items():
        if not isinstance(raw_style, Mapping):
            continue
        if _designator_selector_matches(designator, str(selector)):
            matches.append(dict(raw_style))
    return matches


def _designator_selector_matches(designator: str, selector: str) -> bool:
    upper = designator.strip().upper()
    raw = selector.strip().upper()
    if not raw:
        return False
    if raw.endswith("*"):
        return upper.startswith(raw[:-1])
    if raw.isalpha():
        return upper.startswith(raw)
    range_match = _DESIGNATOR_RANGE_RE.match(raw)
    if range_match:
        prefix_a, start, prefix_b, end = range_match.groups()
        prefix_b = prefix_b or prefix_a
        if prefix_a != prefix_b:
            return False
        designator_match = _DESIGNATOR_NUMBER_RE.match(upper)
        if not designator_match:
            return False
        designator_prefix, designator_number = designator_match.groups()
        if designator_prefix != prefix_a:
            return False
        low = min(int(start), int(end))
        high = max(int(start), int(end))
        return low <= int(designator_number) <= high
    return upper == raw


def _render_footprint_hlr(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    designator: str,
    side: str,
    mode: str,
    styles: dict[str, dict[str, object]],
    color: str,
    line_width: float,
    opacity: float,
    bbox: BoundingBox,
) -> list[str]:
    projection_mode = mode
    if projection_mode == "none":
        return []
    if projection_mode in {"outline", "detail"}:
        return _render_hlr_projection_group(
            pcb,
            pcb_path,
            footprint,
            designator=designator,
            side=side,
            mode=projection_mode,
            styles=styles,
            color=color,
            line_width=line_width,
            opacity=opacity,
            bbox=bbox,
        )
    return _render_footprint_bounds_group(
        pcb,
        pcb_path,
        footprint,
        designator=designator,
        projection_mode=projection_mode,
        color=color,
        line_width=line_width,
        opacity=opacity,
        bbox=bbox,
    )


def _render_hlr_projection_group(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    designator: str,
    side: str,
    mode: str,
    styles: dict[str, dict[str, object]],
    color: str,
    line_width: float,
    opacity: float,
    bbox: BoundingBox,
) -> list[str]:
    rendered = _render_footprint_geometer_hlr(
        pcb,
        pcb_path,
        footprint,
        side=side,
        mode=mode,
        styles=styles,
        bbox=bbox,
    )
    if not rendered:
        fallback = _render_footprint_hole_bounds_rect(
            footprint,
            bbox=bbox,
            color=color,
        ) or _render_footprint_pad_bounds_rect(footprint, bbox=bbox, color=color)
        if not fallback:
            return []
        rendered = [fallback]
    return _svg_component_projection_group(
        designator,
        mode,
        rendered,
        color=color,
        line_width=line_width,
        opacity=opacity,
    )


def _render_footprint_bounds_group(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    designator: str,
    projection_mode: str,
    color: str,
    line_width: float,
    opacity: float,
    bbox: BoundingBox,
) -> list[str]:
    rect = _footprint_bounds_rect(
        pcb,
        pcb_path,
        footprint,
        projection_mode=projection_mode,
        color=color,
        bbox=bbox,
    )
    if not rect:
        return []
    group_mode = (
        projection_mode if projection_mode in {"model_bounds", "pad_bounds"} else "bounding_box"
    )
    return _svg_component_projection_group(
        designator,
        group_mode,
        [rect],
        color=color,
        line_width=line_width,
        opacity=opacity,
    )


def _footprint_bounds_rect(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    projection_mode: str,
    color: str,
    bbox: BoundingBox,
) -> str | None:
    if projection_mode == "model_bounds":
        return _render_footprint_model_bounds_rect(
            pcb,
            pcb_path,
            footprint,
            bbox=bbox,
            color=color,
        )
    if projection_mode == "pad_bounds":
        return _render_footprint_pad_bounds_rect(footprint, bbox=bbox, color=color)
    return _render_footprint_model_bounds_rect(
        pcb,
        pcb_path,
        footprint,
        bbox=bbox,
        color=color,
    ) or _render_footprint_pad_bounds_rect(footprint, bbox=bbox, color=color)


def _svg_component_projection_group(
    designator: str,
    projection: str,
    lines: list[str],
    *,
    color: str,
    line_width: float,
    opacity: float,
) -> list[str]:
    opacity_attr = f' opacity="{_fmt(opacity)}"' if opacity < 1.0 else ""
    return [
        (
            f'<g data-component="{html.escape(designator)}" '
            f'data-projection="{html.escape(projection)}" '
            f'stroke="{html.escape(color)}" stroke-width="{_fmt(line_width)}"'
            f"{opacity_attr}>"
        ),
        *lines,
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
    pose = kicad_model_pose(pcb, footprint, model)
    options = _assembly_projection_options(side=side, styles=styles)
    try:
        _, projected = _get_assembly_projection_cache().project(
            model_hash=model_hash,
            step_bytes=step_bytes,
            pose_signature=pose.signature,
            transform_matrix=pose.matrix,
            options=options,
            model_label=str(getattr(model, "path", "")),
        )
    except Exception as exc:
        log.warning("Geometer HLR failed for %s: %s", _footprint_designator(footprint), exc)
        return []
    return _projected_geometry_to_svg(projected, mode=mode, bbox=bbox)


def _assembly_projection_options(
    *,
    side: str,
    styles: dict[str, dict[str, object]],
) -> _AssemblyProjectionOptions:
    style = styles.get("assembly_hlr", {})
    edge_flags = {key: bool(style[key]) for key in _ASSEMBLY_HLR_EDGE_FLAG_KEYS if key in style}
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
        outline_algorithm=str(style.get("outline_algorithm", "mesh-shadow") or "mesh-shadow"),
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
            if str(getattr(embedded, "name", "") or "").lower() == wanted and str(
                getattr(embedded, "data", "") or ""
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


def _projected_geometry_to_svg(
    projected: _AssemblyProjectedGeometry,
    *,
    mode: str,
    bbox: BoundingBox,
) -> list[str]:
    if mode == "detail":
        segments = projected.detail_line_segments or projected.outline_line_segments
        arcs = projected.detail_arcs or projected.outline_arcs
    else:
        segments = projected.outline_line_segments or projected.detail_line_segments
        arcs = projected.outline_arcs or projected.detail_arcs
    lines: list[str] = []
    for start, end in segments:
        x1, y1 = board_world_to_svg(start, bbox=bbox)
        x2, y2 = board_world_to_svg(end, bbox=bbox)
        lines.append(f'<line x1="{_fmt(x1)}" y1="{_fmt(y1)}" x2="{_fmt(x2)}" y2="{_fmt(y2)}"/>')
    for arc in arcs:
        rendered = _projected_arc_to_svg(arc, bbox)
        if rendered:
            lines.append(rendered)
    return lines


def _projected_arc_to_svg(
    arc: _AssemblyProjectedArc,
    bbox: BoundingBox,
) -> str:
    center = board_world_to_svg(arc.center, bbox=bbox)
    start = board_world_to_svg(arc.start, bbox=bbox)
    end = board_world_to_svg(arc.end, bbox=bbox)
    if arc.full_circle:
        return f'<circle cx="{_fmt(center[0])}" cy="{_fmt(center[1])}" r="{_fmt(arc.radius)}"/>'
    large_arc = 1 if abs(float(arc.extent_rad)) > math.pi else 0
    sweep = 0 if arc.ccw else 1
    return (
        f'<path d="M {_fmt(start[0])} {_fmt(start[1])} '
        f"A {_fmt(arc.radius)} {_fmt(arc.radius)} 0 {large_arc} {sweep} "
        f'{_fmt(end[0])} {_fmt(end[1])}"/>'
    )


def _render_footprint_model_bounds_rect(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    bbox: BoundingBox,
    color: str,
) -> str:
    rect = _footprint_model_bounds_rect_values(pcb, pcb_path, footprint, bbox=bbox)
    if rect is None:
        return ""
    model = _first_step_model(footprint)
    return _svg_rect_from_values(
        rect,
        color=color,
        data_attrs={
            "data-bounds-kind": "model",
            "data-model-path": str(getattr(model, "path", "") if model else ""),
        },
    )


def _footprint_model_bounds_rect_values(
    pcb: KiCadPcb,
    pcb_path: Path,
    footprint: Footprint,
    *,
    bbox: BoundingBox,
) -> tuple[float, float, float, float] | None:
    model = _first_step_model(footprint)
    if model is None:
        return None
    step_bytes = _resolve_model_step_bytes(pcb, footprint, model, pcb_path)
    if step_bytes is None:
        return None
    pose = kicad_model_pose(pcb, footprint, model)
    model_hash = hashlib.sha256(step_bytes).hexdigest()
    try:
        bounds = _geometer_model_bounds(
            step_bytes,
            model_hash=model_hash,
            pose_signature=pose.signature,
            transform_matrix=pose.matrix,
            model_label=str(getattr(model, "path", "")),
        )
    except Exception as exc:
        log.warning(
            "Geometer model bounds failed for %s: %s",
            _footprint_designator(footprint),
            exc,
        )
        return None
    return model_bounds_to_svg_rect(bounds, bbox=bbox)


def _geometer_model_bounds(
    step_bytes: bytes,
    *,
    model_hash: str,
    pose_signature: tuple[float, ...],
    transform_matrix: Matrix4,
    model_label: str,
) -> dict[str, object]:
    cache_key = (model_hash, pose_signature)
    cached = _MODEL_BOUNDS_CACHE.get(cache_key)
    if cached is not None:
        return cached
    try:
        import geometer
    except Exception as exc:  # pragma: no cover - dependency failure path
        raise RuntimeError(
            "The geometer Python package is required for KiCad Cruncher model bounds."
        ) from exc
    label = model_label.strip() or f"hash:{model_hash[:12]}"
    log.info("Computing Geometer model bounds: %s (hash=%s)", label, model_hash[:12])
    result = geometer.model_bounds(step_bytes, model_transform=transform_matrix)
    bounds = dict(result.bounds)
    _MODEL_BOUNDS_CACHE[cache_key] = bounds
    return bounds


def _render_footprint_hole_bounds_rect(
    footprint: Footprint,
    *,
    bbox: BoundingBox,
    color: str,
) -> str:
    rect = _footprint_hole_bounds_rect_values(footprint, bbox=bbox)
    if rect is None:
        return ""
    return _svg_rect_from_values(
        rect,
        color=color,
        data_attrs={"data-bounds-kind": "holes"},
    )


def _footprint_hole_bounds_rect_values(
    footprint: Footprint,
    *,
    bbox: BoundingBox,
) -> tuple[float, float, float, float] | None:
    bounds = _footprint_hole_bounds(footprint)
    if bounds is None or not bounds.is_valid():
        return None
    x = float(bounds.min_x) - float(bbox.min_x)
    y = float(bounds.min_y) - float(bbox.min_y)
    width = float(bounds.max_x) - float(bounds.min_x)
    height = float(bounds.max_y) - float(bounds.min_y)
    return x, y, width, height


def _footprint_hole_bounds(footprint: Footprint) -> BoundingBox | None:
    from kicad_monkey.kicad_geometry import BoundingBox

    bounds = BoundingBox()
    for pad in getattr(footprint, "pads", []) or []:
        for point in _pad_hole_local_corners(pad):
            bounds.expand(transform_footprint_local_to_board(footprint, point))
    return bounds if bounds.is_valid() else None


def _pad_hole_local_corners(pad: object) -> list[tuple[float, float]]:
    diameter = _optional_number(getattr(pad, "drill", None))
    width = _optional_number(getattr(pad, "drill_width", None)) or diameter
    height = _optional_number(getattr(pad, "drill_height", None)) or diameter
    if width is None or height is None or width <= 0.0 or height <= 0.0:
        return []
    center_x = _number_or_zero(getattr(pad, "at_x", None)) + _number_or_zero(
        getattr(pad, "drill_offset_x", None)
    )
    center_y = _number_or_zero(getattr(pad, "at_y", None)) + _number_or_zero(
        getattr(pad, "drill_offset_y", None)
    )
    half_width = width / 2.0
    half_height = height / 2.0
    corners = (
        (-half_width, -half_height),
        (half_width, -half_height),
        (half_width, half_height),
        (-half_width, half_height),
    )
    angle_rad = math.radians(_number_or_zero(getattr(pad, "at_angle", None)))
    if abs(angle_rad) <= 1.0e-12:
        return [(center_x + x, center_y + y) for x, y in corners]
    cos_angle = math.cos(angle_rad)
    sin_angle = math.sin(angle_rad)
    return [
        (
            center_x + x * cos_angle - y * sin_angle,
            center_y + x * sin_angle + y * cos_angle,
        )
        for x, y in corners
    ]


def _optional_number(value: object) -> float | None:
    if value is None:
        return None
    if not isinstance(value, (int, float, str)):
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(result):
        return None
    return result


def _number_or_zero(value: object) -> float:
    return _optional_number(value) or 0.0


def _render_footprint_pad_bounds_rect(
    footprint: Footprint,
    *,
    bbox: BoundingBox,
    color: str,
) -> str:
    rect = _footprint_pad_bounds_rect_values(footprint, bbox=bbox)
    if rect is None:
        return ""
    return _svg_rect_from_values(
        rect,
        color=color,
        data_attrs={"data-bounds-kind": "pads"},
    )


def _footprint_pad_bounds_rect_values(
    footprint: Footprint,
    *,
    bbox: BoundingBox,
) -> tuple[float, float, float, float] | None:
    bounds = _footprint_pad_bounds(footprint)
    if bounds is None or not bounds.is_valid():
        return None
    x = float(bounds.min_x) - float(bbox.min_x)
    y = float(bounds.min_y) - float(bbox.min_y)
    width = float(bounds.max_x) - float(bounds.min_x)
    height = float(bounds.max_y) - float(bounds.min_y)
    return x, y, width, height


def _footprint_pad_bounds(footprint: Footprint) -> BoundingBox | None:
    from kicad_monkey.kicad_geometry import BoundingBox

    bounds = BoundingBox()
    for pad in getattr(footprint, "pads", []) or []:
        local_bounds = pad.get_bounds()
        if not local_bounds.is_valid():
            continue
        for point in (
            (local_bounds.min_x, local_bounds.min_y),
            (local_bounds.max_x, local_bounds.min_y),
            (local_bounds.max_x, local_bounds.max_y),
            (local_bounds.min_x, local_bounds.max_y),
        ):
            bounds.expand(transform_footprint_local_to_board(footprint, point))
    return bounds if bounds.is_valid() else None


def _svg_rect_from_values(
    rect: tuple[float, float, float, float],
    *,
    color: str,
    data_attrs: Mapping[str, str],
) -> str:
    x, y, width, height = rect
    attrs = " ".join(
        f'{html.escape(str(key))}="{html.escape(str(value))}"'
        for key, value in sorted(data_attrs.items())
        if value
    )
    return (
        f'<rect x="{_fmt(x)}" y="{_fmt(y)}" width="{_fmt(width)}" height="{_fmt(height)}" '
        f'stroke="{html.escape(color)}" fill="none" {attrs}/>'
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
        pcb_path,
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

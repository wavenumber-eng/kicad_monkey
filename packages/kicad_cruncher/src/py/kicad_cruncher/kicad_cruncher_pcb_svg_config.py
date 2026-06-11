"""A0 config model for KiCad PCB SVG layer/view rendering."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path

from kicad_cruncher.config_json import enum_help, render_commented_jsonc

PCB_SVG_CONFIG_FILENAME = "pcb.svg.config"
PCB_SVG_CONFIG_SCHEMA = "pcb.svg.config.a0"
PCB_DEFAULT_SVG_SCALE = 10.0
PCB_SVG_CANVAS_BOUNDS_MODES = frozenset({"board_outline", "all_geometry"})
PCB_SVG_COMPONENT_PROJECTION_MODES = frozenset(
    {"detail", "outline", "bounding_box", "model_bounds", "pad_bounds", "none"}
)
PCB_SVG_COMPONENT_SIDES = frozenset({"top", "bottom"})
PCB_SVG_ASSEMBLY_VIRTUAL_LAYERS = frozenset(
    {
        "ASSEMBLY_HLR_TOP",
        "ASSEMBLY_HLR_BOTTOM",
        "ASSEMBLY_HLR_TOP_OUTLINE",
        "ASSEMBLY_HLR_TOP_DETAIL",
        "ASSEMBLY_HLR_BOTTOM_OUTLINE",
        "ASSEMBLY_HLR_BOTTOM_DETAIL",
        "ASSEMBLY_BOUNDS_TOP_MODEL",
        "ASSEMBLY_BOUNDS_BOTTOM_MODEL",
        "ASSEMBLY_BOUNDS_TOP_PADS",
        "ASSEMBLY_BOUNDS_BOTTOM_PADS",
    }
)
PCB_SVG_SPECIAL_LAYERS = frozenset(
    {
        "BOARD_OUTLINE",
        "BOARD_CUTOUTS",
        "DRILLS",
        "SLOTS",
        *PCB_SVG_ASSEMBLY_VIRTUAL_LAYERS,
        "ASSEMBLY_DESIGNATORS_TOP",
        "ASSEMBLY_DESIGNATORS_BOTTOM",
        "PIN1_TOP",
        "PIN1_BOTTOM",
    }
)
_PCB_SVG_PROJECTION_MODE_OPTIONS = (
    "detail",
    "outline",
    "bounding_box",
    "model_bounds",
    "pad_bounds",
    "none",
)
_PCB_SVG_SYNTHETIC_LAYER_OPTIONS = tuple(sorted(PCB_SVG_SPECIAL_LAYERS))
_PCB_SVG_CONFIG_HEADER = (
    "kicad-cruncher pcb-svg configuration.",
    "",
    "This file is JSONC. Comments and trailing commas are accepted.",
    "Configured views render physical and virtual layers in the listed draw order.",
    "The generated default emits assembly_top_view and assembly_bottom_view.",
    "Default assembly views include cutouts, drills, slots, pin-1 markers,",
    "Geometer outline HLR, and bold monospace assembly designators.",
    "Physical layer tokens include F.Cu, B.Cu, F.SilkS, B.SilkS, F.Fab, B.Fab,",
    "F.Paste, B.Paste, F.Mask, B.Mask, Edge.Cuts, TOP, and BOTTOM.",
    "Virtual layer tokens include BOARD_OUTLINE, BOARD_CUTOUTS, DRILLS, SLOTS,",
    "PIN1_TOP, PIN1_BOTTOM, assembly HLR/detail/bounds tokens, and designators.",
    "Component overrides live under components.<designator>.",
    "Exact component settings merge over global and per-view styles.",
)
_PCB_SVG_CONFIG_COMMENTS = {
    ("schema",): "Required config contract id.",
    ("global",): "Global rendering options and default style table.",
    (
        "global",
        "pcbdoc",
    ): "Project-relative PCB file for .kicad_pro inputs; null auto-resolves.",
    ("global", "canvas"): "SVG canvas bounds and margin policy.",
    ("global", "canvas", "bounds"): enum_help(
        "Canvas bounds mode",
        ("board_outline", "all_geometry"),
    ),
    ("global", "canvas", "margin_mm"): "Extra margin around the selected canvas bounds.",
    ("global", "include_metadata"): "Emit data-* metadata in generated SVG elements.",
    ("global", "show_empty_layers"): "Write physical layer SVGs even when no geometry is present.",
    (
        "global",
        "clip_to_outline",
    ): "Clip rendered copper/layers to the board outline when possible.",
    (
        "global",
        "clip_holes_from_copper",
    ): "Clip drill and slot holes out of rendered copper layers.",
    (
        "global",
        "mirror_bottom_view",
    ): "Mirror bottom-side composed views into a board-front reading orientation.",
    ("global", "svg_scale"): "SVG user-unit scale factor per mm.",
    (
        "global",
        "svg_size_unit",
    ): "Optional SVG width/height unit suffix; empty string keeps raw user units.",
    ("global", "clean_output"): "Remove prior generated outputs before writing the new render set.",
    ("global", "styles"): (
        "Global style table.",
        "Per-view styles and component overrides merge over these defaults.",
    ),
    ("global", "styles", "board_outline"): "Board perimeter style derived from Edge.Cuts geometry.",
    (
        "global",
        "styles",
        "board_outline",
        "max_arc_segment_mm",
    ): (
        "Maximum sampled chord length for board-outline arcs.",
        "Smaller values are smoother and larger.",
    ),
    (
        "global",
        "styles",
        "board_outline",
        "max_curve_segment_mm",
    ): "Maximum sampled segment length for board-outline curves.",
    (
        "global",
        "styles",
        "board_outline",
        "max_circle_segment_mm",
    ): "Maximum sampled chord length for board-outline circles.",
    (
        "global",
        "styles",
        "board_cutouts",
    ): "Internal board cutout style derived from smaller closed Edge.Cuts regions.",
    ("global", "styles", "board_cutouts", "outline_style"): enum_help(
        "Cutout outline style",
        ("solid", "dashed"),
    ),
    ("global", "styles", "drills"): "Circular drill overlay style.",
    ("global", "styles", "slots"): "Slotted drill overlay style.",
    (
        "global",
        "styles",
        "pin1_marker",
    ): "Pin-1 marker style for synthesized PIN1_TOP/PIN1_BOTTOM overlays.",
    ("global", "styles", "assembly_designators"): "Assembly reference designator text style.",
    (
        "global",
        "styles",
        "assembly_designators",
        "font_family",
    ): "CSS font-family for assembly designator text.",
    (
        "global",
        "styles",
        "assembly_designators",
        "font_weight",
    ): "CSS font weight for assembly designator text.",
    (
        "global",
        "styles",
        "assembly_designators",
        "rotation_direction",
    ): enum_help(
        "Designator rotation direction when bounds are tall",
        (
            "cw",
            "clockwise",
            "right",
            "+90",
            "90",
            "ccw",
            "counterclockwise",
            "counter-clockwise",
            "left",
            "-90",
        ),
    ),
    ("global", "styles", "assembly_hlr"): "Geometer-backed assembly projection style.",
    ("global", "styles", "assembly_hlr", "curve_mode"): enum_help(
        "Projection curve output mode",
        ("native_arcs", "segments"),
    ),
    ("global", "styles", "assembly_hlr", "outline_algorithm"): enum_help(
        "Outline algorithm",
        ("mesh-shadow", "hlr"),
    ),
    ("assembly",): "Default assembly projection and designator color policy.",
    ("assembly", "default_projection"): enum_help(
        "Default component projection mode",
        _PCB_SVG_PROJECTION_MODE_OPTIONS,
    ),
    ("assembly", "dnp_projection"): enum_help(
        "Projection mode used for DNP components",
        _PCB_SVG_PROJECTION_MODE_OPTIONS,
    ),
    ("assembly", "designator_color"): "Fallback color for fitted assembly designators.",
    ("assembly", "dnp_designator_color"): "Fallback color for DNP assembly designators.",
    ("dnp",): "DNP hatch/marker style for DNP review overlays.",
    ("diodes",): "Diode/cathode marker detection and rendering options.",
    (
        "diodes",
        "numeric_cathode_pad",
    ): "Numeric cathode pad name used when no named cathode pad is present.",
    ("diodes", "cathode_pad_names"): "Named pads treated as cathode markers.",
    ("diodes", "designator_prefixes"): "Designator prefixes treated as diode-like parts.",
    (
        "diodes",
        "parameter_terms",
    ): "Case-insensitive parameter text terms treated as diode-like parts.",
    ("pin1",): (
        "Global pin-1 marker selection policy.",
        "Per-view pin1 overrides merge over this object.",
    ),
    (
        "pin1",
        "exclude_designators",
    ): "Exact, prefix, wildcard, or range selectors excluded from pin-1 markers.",
    (
        "pin1",
        "exclude_designator_prefixes",
    ): "Additional prefix selectors excluded from pin-1 marker generation.",
    ("pin1", "exclude_single_pin"): "Suppress pin-1 markers for one-pin footprints.",
    ("components",): "Per-component overrides keyed by reference designator, for example J1 or U5.",
    ("components", "side"): enum_help("Forced component side", ("top", "bottom")),
    ("components", "projection"): enum_help(
        "Per-component projection mode",
        _PCB_SVG_PROJECTION_MODE_OPTIONS,
    ),
    ("components", "assembly_hlr"): "Per-component assembly HLR style overrides.",
    ("components", "assembly_designators"): "Per-component assembly designator style overrides.",
    ("components", "pin1_enabled"): "Force pin-1 marker on or off for this component.",
    ("components", "pin1_pad"): "Specific pad name to use for this component's pin-1 marker.",
    ("components", "cathode_pad"): "Specific pad name to use for this component's cathode marker.",
    ("components", "diode"): "Force diode/cathode marker detection on or off for this component.",
    (
        "components",
        "diode_line_art",
    ): "Force diode line-art rendering on or off for this component.",
    (
        "components",
        "show_designator",
    ): "Force assembly designator rendering on or off for this component.",
    ("layer_outputs",): "Standalone per-layer and virtual-layer SVG output settings.",
    ("layer_outputs", "layers"): "Physical layers to write, or auto for all detected board layers.",
    ("layer_outputs", "include_special_layers"): enum_help(
        "Virtual layers to write when write_virtual_layers is true",
        _PCB_SVG_SYNTHETIC_LAYER_OPTIONS,
    ),
    (
        "layer_outputs",
        "add_edge_cuts_to_physical_layers",
    ): "Include raw Edge.Cuts context in each non-Edge.Cuts physical layer output.",
    (
        "layer_outputs",
        "add_drills_to_physical_layers",
    ): "Include computed round drill overlays as context in physical layer outputs.",
    (
        "layer_outputs",
        "add_slots_to_physical_layers",
    ): "Include computed slot overlays as context in physical layer outputs.",
    (
        "layer_outputs",
        "write_virtual_layers",
    ): "Write standalone __virtual__ SVG files for selected virtual layers.",
    (
        "layer_outputs",
        "output_dir",
    ): "Directory for per-layer SVG outputs relative to the selected output root.",
    ("views",): "Composed SVG views. Each view's layers array is the draw order.",
    ("views", "name"): "Stable view name used by --views filtering and default output paths.",
    ("views", "group_id"): "SVG group id for this composed view.",
    ("views", "output_svg"): (
        "Output path template relative to the selected output root.",
        "Supports {board} and {view}.",
    ),
    ("views", "layers"): "Ordered physical and virtual layer tokens rendered into this view.",
    ("views", "mirror"): "Override bottom-view mirroring for this view.",
    ("views", "assembly_hlr_mode"): enum_help(
        "View projection mode for ASSEMBLY_HLR_TOP/BOTTOM tokens",
        _PCB_SVG_PROJECTION_MODE_OPTIONS,
    ),
    ("views", "styles"): "Per-view style overrides merged over global styles.",
    ("views", "pin1"): "Per-view pin-1 selection overrides.",
    ("views", "description"): "Optional human-readable view description.",
}
_PCB_SVG_COMMENTS_BY_KEY = {
    "enabled": "Enable this feature or output block.",
    "color": "CSS color used for this rendered feature.",
    "line_width_mm": "Stroke width in mm.",
    "opacity": "Opacity from 0 to 1.",
    "plated_color": "CSS color for plated drill/slot geometry.",
    "non_plated_color": "CSS color for non-plated drill/slot geometry.",
    "hatch": "Draw a hatch fill when true.",
    "hatch_spacing_mm": "Hatch line spacing in mm.",
    "hatch_angle_deg": "Hatch angle in degrees.",
    "hatch_line_width_mm": "Hatch stroke width in mm.",
    "outline_dash_mm": "Dash length for dashed outlines in mm.",
    "outline_width_mm": "Cutout outline stroke width in mm.",
    "min_arc_segments": "Minimum segment count when sampling arcs.",
    "min_curve_segments": "Minimum segment count when sampling curves.",
    "min_circle_segments": "Minimum segment count when sampling circles.",
    "max_arc_segments": "Maximum segment count when sampling arcs.",
    "max_curve_segments": "Maximum segment count when sampling curves.",
    "max_circle_segments": "Maximum segment count when sampling circles.",
    "dot_diameter_mm": "Fixed pin-1 dot diameter in mm.",
    "pad_diameter_ratio": "Pin-1 dot diameter as a ratio of selected pad size.",
    "min_dot_diameter_mm": "Minimum pin-1 dot diameter in mm.",
    "max_dot_diameter_mm": "Maximum pin-1 dot diameter in mm.",
    "box_fill_ratio": "Fraction of component bounds used by fitted designator text.",
    "min_font_size_mm": "Minimum fitted designator font size in mm.",
    "max_font_size_mm": "Maximum fitted designator font size in mm.",
    "rotation_aspect_threshold": (
        "Rotate designator text when fitted bounds exceed this height/width ratio."
    ),
    "include_visible": "Include visible HLR/detail geometry.",
    "include_outline": "Include outline/silhouette geometry.",
    "samples_per_curve": "Segment samples per curve when curve output is segmented.",
    "round_digits": "Decimal digits retained in generated SVG geometry.",
    "union_polygons": "Union outline polygons before rendering when supported by Geometer.",
}

_STYLE_ORDER = (
    "board_outline",
    "board_cutouts",
    "drills",
    "slots",
    "copper_traces",
    "vias",
    "copper_polygons",
    "smd_pads",
    "through_hole_pads",
    "silkscreen_component_graphics",
    "silkscreen_designators",
    "silkscreen_board_graphics",
    "pin1_marker",
    "assembly_designators",
    "keepout",
    "assembly_hlr",
)

_KICAD_PHYSICAL_LAYER_ALIASES = {
    "TOP": "F.Cu",
    "BOTTOM": "B.Cu",
    "TOP_COPPER": "F.Cu",
    "BOTTOM_COPPER": "B.Cu",
    "TOPOVERLAY": "F.SilkS",
    "BOTTOMOVERLAY": "B.SilkS",
    "TOP_SILK": "F.SilkS",
    "BOTTOM_SILK": "B.SilkS",
    "TOP_SILKSCREEN": "F.SilkS",
    "BOTTOM_SILKSCREEN": "B.SilkS",
    "TOPPASTE": "F.Paste",
    "BOTTOMPASTE": "B.Paste",
    "TOP_PASTE": "F.Paste",
    "BOTTOM_PASTE": "B.Paste",
    "TOPSOLDER": "F.Mask",
    "BOTTOMSOLDER": "B.Mask",
    "TOP_MASK": "F.Mask",
    "BOTTOM_MASK": "B.Mask",
    "BOARD_OUTLINE": "Edge.Cuts",
    "OUTLINE": "Edge.Cuts",
    "BOARD_PROFILE": "Edge.Cuts",
    "COMMENTS": "Cmts.User",
    "DRAWINGS": "Dwgs.User",
    "USER_COMMENTS": "Cmts.User",
    "USER_DRAWINGS": "Dwgs.User",
    "USER_ECO1": "Eco1.User",
    "USER_ECO2": "Eco2.User",
    "ECO1_USER": "Eco1.User",
    "ECO2_USER": "Eco2.User",
}

_KICAD_CANONICAL_LAYER_BY_UPPER = {
    "F.CU": "F.Cu",
    "B.CU": "B.Cu",
    "F.SILKS": "F.SilkS",
    "B.SILKS": "B.SilkS",
    "F.FAB": "F.Fab",
    "B.FAB": "B.Fab",
    "F.PASTE": "F.Paste",
    "B.PASTE": "B.Paste",
    "F.MASK": "F.Mask",
    "B.MASK": "B.Mask",
    "EDGE.CUTS": "Edge.Cuts",
    "CMTS.USER": "Cmts.User",
    "DWGS.USER": "Dwgs.User",
    "ECO1.USER": "Eco1.User",
    "ECO2.USER": "Eco2.User",
}


def _coerce_bool(value: object, default: bool) -> bool:
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
    raise ValueError(f"Invalid boolean value in pcb-svg config: {value!r}")


def _coerce_float(value: object, default: float) -> float:
    if value is None:
        return float(default)
    if not isinstance(value, int | float | str):
        raise ValueError(f"Invalid numeric value in pcb-svg config: {value!r}")
    try:
        return float(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid numeric value in pcb-svg config: {value!r}") from exc


def _coerce_nonnegative_float(value: object, default: float, *, field_name: str) -> float:
    result = _coerce_float(value, default)
    if result < 0.0:
        raise ValueError(f"pcb-svg config field '{field_name}' must be non-negative")
    return result


def _coerce_optional_str(value: object) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _coerce_object_mapping(value: object, *, field_name: str) -> dict[str, object] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise ValueError(f"pcb-svg config field '{field_name}' must be an object")
    return {str(key): item for key, item in value.items()}


def _coerce_str_list(value: object, *, field_name: str) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list):
        raise ValueError(f"pcb-svg config field '{field_name}' must be an array")
    result: list[str] = []
    for item in value:
        text = str(item).strip()
        if text:
            result.append(normalize_layer_token(text))
    return result


def _coerce_raw_str_list(
    value: object,
    default: list[str],
    *,
    field_name: str,
) -> list[str]:
    if value is None:
        return list(default)
    if not isinstance(value, list):
        raise ValueError(f"pcb-svg config field '{field_name}' must be an array")
    return [str(item).strip() for item in value if str(item).strip()]


def _coerce_selector_list(
    value: object,
    default: list[str],
    *,
    field_name: str,
) -> list[str]:
    if value is None:
        return list(default)
    raw_items: list[object]
    if isinstance(value, str):
        raw_items = [value]
    elif isinstance(value, list):
        raw_items = value
    else:
        raise ValueError(f"pcb-svg config field '{field_name}' must be a string or an array")
    selectors: list[str] = []
    for item in raw_items:
        for token in str(item).split(","):
            text = token.strip()
            if text:
                selectors.append(text.upper())
    return selectors


def _coerce_projection_mode(value: object, default: str, *, field_name: str) -> str:
    raw = str(value or default).strip().lower().replace("-", "_")
    aliases = {
        "bbox": "bounding_box",
        "box": "bounding_box",
        "bounds": "bounding_box",
        "model-bounds": "model_bounds",
        "model_bbox": "model_bounds",
        "model-bbox": "model_bounds",
        "pad_bounds": "pad_bounds",
        "pad-bounds": "pad_bounds",
        "pad-bbox": "pad_bounds",
        "off": "none",
        "disabled": "none",
    }
    mode = aliases.get(raw, raw)
    if mode not in PCB_SVG_COMPONENT_PROJECTION_MODES:
        raise ValueError(
            f"pcb-svg config field '{field_name}' must be one of: "
            + ", ".join(sorted(PCB_SVG_COMPONENT_PROJECTION_MODES))
        )
    return mode


def _coerce_component_side(value: object, *, field_name: str) -> str | None:
    raw = str(value or "").strip().lower()
    if not raw:
        return None
    aliases = {"toplayer": "top", "bottomlayer": "bottom"}
    side = aliases.get(raw.replace("_", "").replace("-", ""), raw)
    if side not in PCB_SVG_COMPONENT_SIDES:
        raise ValueError(f"pcb-svg config field '{field_name}' must be 'top' or 'bottom'")
    return side


def normalize_layer_token(value: str) -> str:
    """Normalize a config layer token while preserving KiCad layer spelling."""
    token = value.strip()
    if not token:
        raise ValueError("Empty layer token in pcb-svg config")
    upper = token.upper().replace(" ", "_").replace("-", "_")
    synthetic_aliases = {
        "CUTOUTS": "BOARD_CUTOUTS",
        "CUTOUT": "BOARD_CUTOUTS",
        "HLR_TOP": "ASSEMBLY_HLR_TOP",
        "HLR_BOTTOM": "ASSEMBLY_HLR_BOTTOM",
        "HLR_TOP_OUTLINE": "ASSEMBLY_HLR_TOP_OUTLINE",
        "HLR_TOP_DETAIL": "ASSEMBLY_HLR_TOP_DETAIL",
        "HLR_BOTTOM_OUTLINE": "ASSEMBLY_HLR_BOTTOM_OUTLINE",
        "HLR_BOTTOM_DETAIL": "ASSEMBLY_HLR_BOTTOM_DETAIL",
        "MODEL_BOUNDS_TOP": "ASSEMBLY_BOUNDS_TOP_MODEL",
        "MODEL_BOUNDS_BOTTOM": "ASSEMBLY_BOUNDS_BOTTOM_MODEL",
        "PAD_BOUNDS_TOP": "ASSEMBLY_BOUNDS_TOP_PADS",
        "PAD_BOUNDS_BOTTOM": "ASSEMBLY_BOUNDS_BOTTOM_PADS",
        "DESIGNATORS_TOP": "ASSEMBLY_DESIGNATORS_TOP",
        "DESIGNATORS_BOTTOM": "ASSEMBLY_DESIGNATORS_BOTTOM",
        "PIN_1_TOP": "PIN1_TOP",
        "PIN_1_BOTTOM": "PIN1_BOTTOM",
    }
    if upper in synthetic_aliases:
        return synthetic_aliases[upper]
    if upper in PCB_SVG_SPECIAL_LAYERS:
        return upper
    if upper in _KICAD_PHYSICAL_LAYER_ALIASES:
        return _KICAD_PHYSICAL_LAYER_ALIASES[upper]
    dotted_upper = token.upper()
    if dotted_upper in _KICAD_CANONICAL_LAYER_BY_UPPER:
        return _KICAD_CANONICAL_LAYER_BY_UPPER[dotted_upper]
    if upper.startswith("IN") and upper.endswith("_CU"):
        inner = upper[2:-3]
        if inner.isdigit():
            return f"In{inner}.Cu"
    return token


def is_synthetic_layer_token(token: str) -> bool:
    """Return whether a normalized layer token is synthetic."""
    return normalize_layer_token(token) in PCB_SVG_SPECIAL_LAYERS


def physical_layer_from_token(token: str) -> str | None:
    """Return a KiCad physical layer name, or None for synthetic-only layers."""
    normalized = normalize_layer_token(token)
    if normalized == "BOARD_OUTLINE":
        return "Edge.Cuts"
    if normalized in PCB_SVG_SPECIAL_LAYERS:
        return None
    return normalized


def parse_pcb_layer_selector(raw_layers: str | None) -> list[str] | None:
    """Parse comma-separated CLI layer selectors into normalized tokens."""
    if raw_layers is None:
        return None
    tokens = [token.strip() for token in raw_layers.split(",") if token.strip()]
    if not tokens:
        raise ValueError("--layers was provided but no valid layer tokens were found")
    resolved: list[str] = []
    for token in tokens:
        normalized = normalize_layer_token(token)
        if normalized not in resolved:
            resolved.append(normalized)
    return resolved


def default_pcb_svg_styles() -> dict[str, dict[str, object]]:
    """Return the default A0 style table for configured PCB SVG views."""
    return {
        "board_outline": {
            "enabled": True,
            "color": "#000000",
            "line_width_mm": 0.10,
            "max_arc_segment_mm": 1.0,
            "max_curve_segment_mm": 0.5,
            "max_circle_segment_mm": 1.0,
            "min_arc_segments": 6,
            "min_curve_segments": 8,
            "min_circle_segments": 64,
            "max_arc_segments": 2048,
            "max_curve_segments": 2048,
            "max_circle_segments": 2048,
        },
        "board_cutouts": {
            "enabled": True,
            "color": "#FF0000",
            "hatch": True,
            "hatch_spacing_mm": 0.5,
            "hatch_angle_deg": 45.0,
            "hatch_line_width_mm": 0.08,
            "outline_style": "solid",
            "outline_dash_mm": 1.5,
            "outline_width_mm": 0.15,
        },
        "drills": {
            "enabled": True,
            "plated_color": "#90EE90",
            "non_plated_color": "#ADD8E6",
            "opacity": 1.0,
        },
        "slots": {
            "enabled": True,
            "plated_color": "#90EE90",
            "non_plated_color": "#ADD8E6",
            "opacity": 1.0,
        },
        "copper_traces": {"enabled": True, "color": "#000000"},
        "vias": {"enabled": True, "color": "#000000"},
        "copper_polygons": {"enabled": True, "color": "#888888"},
        "smd_pads": {"enabled": True, "color": "#000000"},
        "through_hole_pads": {"enabled": True, "color": "#000000"},
        "silkscreen_component_graphics": {"enabled": True, "color": "#000000"},
        "silkscreen_designators": {"enabled": True, "color": "#000000"},
        "silkscreen_board_graphics": {"enabled": True, "color": "#000000"},
        "pin1_marker": {
            "enabled": True,
            "color": "#2563EB",
            "dot_diameter_mm": 0.55,
            "pad_diameter_ratio": 0.60,
            "min_dot_diameter_mm": 0.25,
            "max_dot_diameter_mm": 1.0,
        },
        "assembly_designators": {
            "enabled": True,
            "color": "#2563EB",
            "font_family": "Arial, sans-serif",
            "box_fill_ratio": 0.80,
            "min_font_size_mm": 0.35,
            "max_font_size_mm": 2.5,
            "rotation_aspect_threshold": 1.5,
            "rotation_direction": "ccw",
            "opacity": 1.0,
        },
        "keepout": {"enabled": True, "color": "#CC00CC"},
        "assembly_hlr": {
            "enabled": True,
            "color": "#F59E0B",
            "line_width_mm": 0.12,
            "opacity": 0.75,
            "curve_mode": "native_arcs",
            "samples_per_curve": 24,
            "round_digits": 3,
            "include_visible": True,
            "include_outline": True,
            "outline_algorithm": "mesh-shadow",
            "union_polygons": True,
        },
    }


def default_pcb_svg_assembly_view_styles() -> dict[str, dict[str, object]]:
    """Return muted copper overrides for default assembly review views."""
    return {
        "drills": {"plated_color": "#F7F7F7", "non_plated_color": "#F7F7F7"},
        "slots": {"plated_color": "#F7F7F7", "non_plated_color": "#F7F7F7"},
        "copper_traces": {"color": "#BBBBBB"},
        "vias": {"color": "#BBBBBB"},
        "copper_polygons": {"color": "#DDDDDD"},
        "smd_pads": {"color": "#AAAAAA"},
        "through_hole_pads": {"color": "#AAAAAA"},
        "assembly_designators": {
            "font_family": "Consolas, 'Liberation Mono', 'Courier New', monospace",
            "font_weight": "700",
        },
    }


def merge_pcb_svg_styles(
    base: dict[str, dict[str, object]],
    override: Mapping[str, object] | None,
) -> dict[str, dict[str, object]]:
    """Merge an A0 style table while preserving default style keys."""
    merged = {name: dict(base.get(name, {})) for name in _STYLE_ORDER}
    if override:
        for name, raw_style in override.items():
            if not isinstance(raw_style, dict):
                raise ValueError(f"pcb-svg style '{name}' must be an object")
            target = merged.setdefault(str(name), {})
            target.update(raw_style)
    return merged


@dataclass(slots=True)
class _PcbSvgCanvasConfig:
    bounds: str = "board_outline"
    margin_mm: float = 1.0

    @classmethod
    def from_dict(cls, data: dict[str, object] | None) -> _PcbSvgCanvasConfig:
        if data is None:
            return cls()
        bounds = str(data.get("bounds", "board_outline") or "board_outline").strip().lower()
        aliases = {
            "board": "board_outline",
            "outline": "board_outline",
            "board_profile": "board_outline",
            "all": "all_geometry",
            "rendered_view": "all_geometry",
            "rendered_geometry": "all_geometry",
        }
        bounds = aliases.get(bounds, bounds)
        if bounds not in PCB_SVG_CANVAS_BOUNDS_MODES:
            raise ValueError(
                "pcb-svg config field 'global.canvas.bounds' must be "
                "'board_outline' or 'all_geometry'"
            )
        return cls(
            bounds=bounds,
            margin_mm=_coerce_nonnegative_float(
                data.get("margin_mm"),
                1.0,
                field_name="global.canvas.margin_mm",
            ),
        )

    def to_dict(self) -> dict[str, object]:
        return {"bounds": self.bounds, "margin_mm": self.margin_mm}


@dataclass(slots=True)
class _PcbSvgGlobalConfig:
    pcbdoc: str | None = None
    canvas: _PcbSvgCanvasConfig = field(default_factory=_PcbSvgCanvasConfig)
    include_metadata: bool = True
    show_empty_layers: bool = False
    clip_to_outline: bool = True
    clip_holes_from_copper: bool = True
    mirror_bottom_view: bool = True
    svg_scale: float = PCB_DEFAULT_SVG_SCALE
    svg_size_unit: str = ""
    clean_output: bool = False
    styles: dict[str, dict[str, object]] = field(default_factory=default_pcb_svg_styles)

    @classmethod
    def from_dict(cls, data: dict[str, object] | None) -> _PcbSvgGlobalConfig:
        if data is None:
            return cls()
        default = cls()
        return cls(
            pcbdoc=_coerce_optional_str(data.get("pcbdoc")),
            canvas=_PcbSvgCanvasConfig.from_dict(
                _coerce_object_mapping(data.get("canvas"), field_name="global.canvas")
            ),
            include_metadata=_coerce_bool(data.get("include_metadata"), True),
            show_empty_layers=_coerce_bool(data.get("show_empty_layers"), False),
            clip_to_outline=_coerce_bool(data.get("clip_to_outline"), True),
            clip_holes_from_copper=_coerce_bool(data.get("clip_holes_from_copper"), True),
            mirror_bottom_view=_coerce_bool(data.get("mirror_bottom_view"), True),
            svg_scale=_coerce_float(data.get("svg_scale"), default.svg_scale),
            svg_size_unit=str(data.get("svg_size_unit", default.svg_size_unit) or ""),
            clean_output=_coerce_bool(data.get("clean_output"), False),
            styles=merge_pcb_svg_styles(
                default.styles,
                _coerce_object_mapping(data.get("styles"), field_name="global.styles"),
            ),
        )

    def to_dict(self) -> dict[str, object]:
        result: dict[str, object] = {
            "canvas": self.canvas.to_dict(),
            "include_metadata": self.include_metadata,
            "show_empty_layers": self.show_empty_layers,
            "clip_to_outline": self.clip_to_outline,
            "clip_holes_from_copper": self.clip_holes_from_copper,
            "mirror_bottom_view": self.mirror_bottom_view,
            "svg_scale": self.svg_scale,
            "svg_size_unit": self.svg_size_unit,
            "clean_output": self.clean_output,
            "styles": self.styles,
        }
        if self.pcbdoc is not None:
            result["pcbdoc"] = self.pcbdoc
        return result


@dataclass(slots=True)
class _PcbSvgViewConfig:
    name: str
    enabled: bool = True
    group_id: str | None = None
    output_svg: str | None = None
    layers: list[str] = field(default_factory=list)
    mirror: bool | None = None
    assembly_hlr_mode: str = "detail"
    styles: dict[str, dict[str, object]] = field(default_factory=dict)
    pin1: _PcbSvgPin1Config | None = None
    description: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, object]) -> _PcbSvgViewConfig:
        if not isinstance(data, dict):
            raise ValueError("Each item in pcb-svg config 'views' must be an object")
        name = _coerce_optional_str(data.get("name"))
        if not name:
            raise ValueError("Each pcb-svg view must include a non-empty 'name'")
        mode = str(data.get("assembly_hlr_mode", "detail") or "detail").lower()
        aliases = {
            "bounding-box": "bounding_box",
            "bbox": "bounding_box",
            "box": "bounding_box",
            "model-bounds": "model_bounds",
            "model-bbox": "model_bounds",
            "pad-bounds": "pad_bounds",
            "pad-bbox": "pad_bounds",
            "off": "none",
        }
        mode = aliases.get(mode, mode)
        if mode not in PCB_SVG_COMPONENT_PROJECTION_MODES:
            raise ValueError(f"Unsupported assembly_hlr_mode {mode!r} for pcb-svg view {name!r}")
        styles = (
            _coerce_object_mapping(
                data.get("styles"),
                field_name=f"views.{name}.styles",
            )
            or {}
        )
        pin1_config = (
            _PcbSvgPin1Config.from_dict(
                _coerce_object_mapping(data.get("pin1"), field_name=f"views.{name}.pin1")
            )
            if data.get("pin1") is not None
            else None
        )
        return cls(
            name=name,
            enabled=_coerce_bool(data.get("enabled"), True),
            group_id=_coerce_optional_str(data.get("group_id")),
            output_svg=_coerce_optional_str(data.get("output_svg")),
            layers=_coerce_str_list(data.get("layers"), field_name=f"views.{name}.layers"),
            mirror=None if data.get("mirror") is None else _coerce_bool(data.get("mirror"), False),
            assembly_hlr_mode=mode,
            styles=merge_pcb_svg_styles({}, styles),
            pin1=pin1_config,
            description=_coerce_optional_str(data.get("description")),
        )

    def resolved_group_id(self) -> str:
        return self.group_id or f"pcb-svg-view-{self.name.replace('_', '-')}"

    def resolved_output_svg(self) -> str:
        return self.output_svg or f"views/{{board}}__{self.name}.svg"

    def to_dict(self) -> dict[str, object]:
        result: dict[str, object] = {
            "name": self.name,
            "enabled": self.enabled,
            "group_id": self.resolved_group_id(),
            "output_svg": self.resolved_output_svg(),
            "layers": list(self.layers),
            "assembly_hlr_mode": self.assembly_hlr_mode,
        }
        if self.mirror is not None:
            result["mirror"] = self.mirror
        if self.styles:
            result["styles"] = self.styles
        if self.pin1 is not None:
            result["pin1"] = self.pin1.to_dict()
        if self.description:
            result["description"] = self.description
        return result


@dataclass(slots=True)
class _PcbSvgAssemblyConfig:
    default_projection: str = "pad_bounds"
    dnp_projection: str = "bounding_box"
    designator_color: str = "#111111"
    dnp_designator_color: str = "#FF0000"

    @classmethod
    def from_dict(cls, data: dict[str, object] | None) -> _PcbSvgAssemblyConfig:
        if data is None:
            return cls()
        return cls(
            default_projection=_coerce_projection_mode(
                data.get("default_projection"),
                "detail",
                field_name="assembly.default_projection",
            ),
            dnp_projection=_coerce_projection_mode(
                data.get("dnp_projection"),
                "bounding_box",
                field_name="assembly.dnp_projection",
            ),
            designator_color=str(data.get("designator_color", "#111111") or "#111111"),
            dnp_designator_color=str(data.get("dnp_designator_color", "#FF0000") or "#FF0000"),
        )

    def to_dict(self) -> dict[str, object]:
        return {
            "default_projection": self.default_projection,
            "dnp_projection": self.dnp_projection,
            "designator_color": self.designator_color,
            "dnp_designator_color": self.dnp_designator_color,
        }


@dataclass(slots=True)
class _PcbSvgDnpConfig:
    color: str = "#FF0000"
    hatch: bool = True
    hatch_spacing_mm: float = 1.5
    hatch_angle_deg: float = 45.0
    hatch_line_width_mm: float = 0.08

    @classmethod
    def from_dict(cls, data: dict[str, object] | None) -> _PcbSvgDnpConfig:
        if data is None:
            return cls()
        return cls(
            color=str(data.get("color", "#FF0000") or "#FF0000"),
            hatch=_coerce_bool(data.get("hatch"), True),
            hatch_spacing_mm=_coerce_float(data.get("hatch_spacing_mm"), 1.5),
            hatch_angle_deg=_coerce_float(data.get("hatch_angle_deg"), 45.0),
            hatch_line_width_mm=_coerce_float(data.get("hatch_line_width_mm"), 0.08),
        )

    def to_dict(self) -> dict[str, object]:
        return {
            "color": self.color,
            "hatch": self.hatch,
            "hatch_spacing_mm": self.hatch_spacing_mm,
            "hatch_angle_deg": self.hatch_angle_deg,
            "hatch_line_width_mm": self.hatch_line_width_mm,
        }


@dataclass(slots=True)
class _PcbSvgDiodeConfig:
    enabled: bool = True
    line_art: bool = True
    marker_color: str = "#FF0000"
    numeric_cathode_pad: str = "2"
    cathode_pad_names: list[str] = field(default_factory=lambda: ["K", "C"])
    designator_prefixes: list[str] = field(default_factory=lambda: ["D", "LED"])
    parameter_terms: list[str] = field(
        default_factory=lambda: ["diode", "schottky", "zener", "tvs", "led"]
    )

    @classmethod
    def from_dict(cls, data: dict[str, object] | None) -> _PcbSvgDiodeConfig:
        default = cls()
        if data is None:
            return default
        return cls(
            enabled=_coerce_bool(data.get("enabled"), default.enabled),
            line_art=_coerce_bool(data.get("line_art"), default.line_art),
            marker_color=str(
                data.get("marker_color", default.marker_color) or default.marker_color
            ),
            numeric_cathode_pad=str(
                data.get("numeric_cathode_pad", default.numeric_cathode_pad)
                or default.numeric_cathode_pad
            ),
            cathode_pad_names=_coerce_raw_str_list(
                data.get("cathode_pad_names"),
                default.cathode_pad_names,
                field_name="diodes.cathode_pad_names",
            ),
            designator_prefixes=_coerce_raw_str_list(
                data.get("designator_prefixes"),
                default.designator_prefixes,
                field_name="diodes.designator_prefixes",
            ),
            parameter_terms=_coerce_raw_str_list(
                data.get("parameter_terms"),
                default.parameter_terms,
                field_name="diodes.parameter_terms",
            ),
        )

    def to_dict(self) -> dict[str, object]:
        return {
            "enabled": self.enabled,
            "line_art": self.line_art,
            "marker_color": self.marker_color,
            "numeric_cathode_pad": self.numeric_cathode_pad,
            "cathode_pad_names": list(self.cathode_pad_names),
            "designator_prefixes": list(self.designator_prefixes),
            "parameter_terms": list(self.parameter_terms),
        }


@dataclass(slots=True)
class _PcbSvgPin1Config:
    exclude_designators: list[str] = field(default_factory=lambda: ["R", "C"])
    exclude_designator_prefixes: list[str] = field(default_factory=list)
    exclude_single_pin: bool = True

    @classmethod
    def from_dict(
        cls,
        data: dict[str, object] | None,
        default: _PcbSvgPin1Config | None = None,
    ) -> _PcbSvgPin1Config:
        default = default or cls()
        if data is None:
            return default
        selectors = _coerce_selector_list(
            data.get("exclude_designators"),
            default.exclude_designators,
            field_name="pin1.exclude_designators",
        )
        prefixes = _coerce_selector_list(
            data.get("exclude_designator_prefixes"),
            default.exclude_designator_prefixes,
            field_name="pin1.exclude_designator_prefixes",
        )
        return cls(
            exclude_designators=selectors,
            exclude_designator_prefixes=prefixes,
            exclude_single_pin=_coerce_bool(
                data.get("exclude_single_pin"),
                default.exclude_single_pin,
            ),
        )

    def to_dict(self) -> dict[str, object]:
        result: dict[str, object] = {
            "exclude_designators": list(self.exclude_designators),
            "exclude_single_pin": self.exclude_single_pin,
        }
        if self.exclude_designator_prefixes:
            result["exclude_designator_prefixes"] = list(self.exclude_designator_prefixes)
        return result


@dataclass(slots=True)
class _PcbSvgComponentOverride:
    side: str | None = None
    projection: str | None = None
    assembly_hlr: dict[str, object] = field(default_factory=dict)
    assembly_designators: dict[str, object] = field(default_factory=dict)
    pin1_enabled: bool | None = None
    pin1_pad: str | None = None
    cathode_pad: str | None = None
    diode: bool | None = None
    diode_line_art: bool | None = None
    show_designator: bool | None = None

    @classmethod
    def from_dict(cls, designator: str, data: dict[str, object]) -> _PcbSvgComponentOverride:
        return cls(
            side=_coerce_component_side(
                data.get("side"),
                field_name=f"components.{designator}.side",
            ),
            projection=(
                None
                if data.get("projection") is None
                else _coerce_projection_mode(
                    data.get("projection"),
                    "detail",
                    field_name=f"components.{designator}.projection",
                )
            ),
            assembly_hlr=_coerce_object_mapping(
                data.get("assembly_hlr"),
                field_name=f"components.{designator}.assembly_hlr",
            )
            or {},
            assembly_designators=_coerce_object_mapping(
                data.get("assembly_designators"),
                field_name=f"components.{designator}.assembly_designators",
            )
            or {},
            pin1_enabled=(
                None
                if data.get("pin1_enabled") is None
                else _coerce_bool(data.get("pin1_enabled"), False)
            ),
            pin1_pad=_coerce_optional_str(data.get("pin1_pad")),
            cathode_pad=_coerce_optional_str(data.get("cathode_pad")),
            diode=None if data.get("diode") is None else _coerce_bool(data.get("diode"), False),
            diode_line_art=(
                None
                if data.get("diode_line_art") is None
                else _coerce_bool(data.get("diode_line_art"), False)
            ),
            show_designator=(
                None
                if data.get("show_designator") is None
                else _coerce_bool(data.get("show_designator"), False)
            ),
        )

    def to_dict(self) -> dict[str, object]:
        result: dict[str, object] = {}
        for key in (
            "side",
            "projection",
            "pin1_enabled",
            "pin1_pad",
            "cathode_pad",
            "diode",
            "diode_line_art",
            "show_designator",
        ):
            value = getattr(self, key)
            if value is not None:
                result[key] = value
        if self.assembly_hlr:
            result["assembly_hlr"] = dict(self.assembly_hlr)
        if self.assembly_designators:
            result["assembly_designators"] = dict(self.assembly_designators)
        return result


def _coerce_layer_outputs_config(
    data: dict[str, object],
    default: _PcbSvgConfig,
) -> dict[str, object]:
    raw_layer_outputs = data.get("layer_outputs", default.layer_outputs)
    if not isinstance(raw_layer_outputs, dict):
        raise ValueError("pcb-svg config field 'layer_outputs' must be an object")
    layer_outputs = dict(raw_layer_outputs)
    if "layers" in layer_outputs and isinstance(layer_outputs["layers"], list):
        layer_outputs["layers"] = [
            normalize_layer_token(str(token)) for token in layer_outputs["layers"]
        ]
    if "include_special_layers" in layer_outputs:
        layer_outputs["include_special_layers"] = _coerce_str_list(
            layer_outputs.get("include_special_layers"),
            field_name="layer_outputs.include_special_layers",
        )
    _coerce_layer_output_booleans(layer_outputs)
    return layer_outputs


def _coerce_layer_output_booleans(layer_outputs: dict[str, object]) -> None:
    for key in (
        "add_edge_cuts_to_physical_layers",
        "add_drills_to_physical_layers",
        "add_slots_to_physical_layers",
        "write_virtual_layers",
    ):
        if key in layer_outputs:
            layer_outputs[key] = _coerce_bool(layer_outputs.get(key), True)


def _coerce_view_configs(
    data: dict[str, object],
    default: _PcbSvgConfig,
) -> list[_PcbSvgViewConfig]:
    raw_views = data.get("views", [view.to_dict() for view in default.views])
    if not isinstance(raw_views, list):
        raise ValueError("pcb-svg config field 'views' must be an array")
    return [_PcbSvgViewConfig.from_dict(view) for view in raw_views]


def _coerce_component_overrides(
    data: dict[str, object],
) -> dict[str, _PcbSvgComponentOverride]:
    raw_components = data.get("components", {})
    if raw_components is None:
        raw_components = {}
    if not isinstance(raw_components, dict):
        raise ValueError("pcb-svg config field 'components' must be an object")
    return {
        str(designator): _PcbSvgComponentOverride.from_dict(str(designator), raw)
        for designator, raw in raw_components.items()
        if isinstance(raw, dict)
    }


@dataclass(slots=True)
class _PcbSvgConfig:
    global_options: _PcbSvgGlobalConfig = field(default_factory=_PcbSvgGlobalConfig)
    assembly: _PcbSvgAssemblyConfig = field(default_factory=_PcbSvgAssemblyConfig)
    dnp: _PcbSvgDnpConfig = field(default_factory=_PcbSvgDnpConfig)
    diodes: _PcbSvgDiodeConfig = field(default_factory=_PcbSvgDiodeConfig)
    pin1: _PcbSvgPin1Config = field(default_factory=_PcbSvgPin1Config)
    components: dict[str, _PcbSvgComponentOverride] = field(default_factory=dict)
    layer_outputs: dict[str, object] = field(default_factory=dict)
    views: list[_PcbSvgViewConfig] = field(default_factory=list)

    @classmethod
    def default(cls) -> _PcbSvgConfig:
        return cls(
            layer_outputs={
                "enabled": True,
                "layers": "auto",
                "add_edge_cuts_to_physical_layers": True,
                "add_drills_to_physical_layers": True,
                "add_slots_to_physical_layers": True,
                "write_virtual_layers": True,
                "include_special_layers": [
                    "BOARD_OUTLINE",
                    "BOARD_CUTOUTS",
                    "DRILLS",
                    "SLOTS",
                    "ASSEMBLY_HLR_TOP_OUTLINE",
                    "ASSEMBLY_HLR_TOP_DETAIL",
                    "ASSEMBLY_HLR_BOTTOM_OUTLINE",
                    "ASSEMBLY_HLR_BOTTOM_DETAIL",
                    "ASSEMBLY_BOUNDS_TOP_MODEL",
                    "ASSEMBLY_BOUNDS_BOTTOM_MODEL",
                    "ASSEMBLY_BOUNDS_TOP_PADS",
                    "ASSEMBLY_BOUNDS_BOTTOM_PADS",
                    "ASSEMBLY_DESIGNATORS_TOP",
                    "ASSEMBLY_DESIGNATORS_BOTTOM",
                ],
                "output_dir": "layers",
            },
            views=[
                _PcbSvgViewConfig(
                    name="assembly_top_view",
                    output_svg="views/{board}__assembly_top_view.svg",
                    layers=[
                        "BOARD_OUTLINE",
                        "BOARD_CUTOUTS",
                        "F.Cu",
                        "DRILLS",
                        "SLOTS",
                        "PIN1_TOP",
                        "ASSEMBLY_HLR_TOP",
                        "ASSEMBLY_DESIGNATORS_TOP",
                    ],
                    assembly_hlr_mode="outline",
                    styles=default_pcb_svg_assembly_view_styles(),
                ),
                _PcbSvgViewConfig(
                    name="assembly_bottom_view",
                    output_svg="views/{board}__assembly_bottom_view.svg",
                    layers=[
                        "BOARD_OUTLINE",
                        "BOARD_CUTOUTS",
                        "B.Cu",
                        "DRILLS",
                        "SLOTS",
                        "PIN1_BOTTOM",
                        "ASSEMBLY_HLR_BOTTOM",
                        "ASSEMBLY_DESIGNATORS_BOTTOM",
                    ],
                    assembly_hlr_mode="outline",
                    styles=default_pcb_svg_assembly_view_styles(),
                ),
            ],
        )

    @classmethod
    def from_dict(cls, data: dict[str, object]) -> _PcbSvgConfig:
        schema = data.get("schema")
        if schema != PCB_SVG_CONFIG_SCHEMA:
            raise ValueError(
                f"pcb-svg config schema must be {PCB_SVG_CONFIG_SCHEMA!r}; got {schema!r}"
            )
        default = cls.default()

        return cls(
            global_options=_PcbSvgGlobalConfig.from_dict(
                _coerce_object_mapping(data.get("global"), field_name="global")
            ),
            assembly=_PcbSvgAssemblyConfig.from_dict(
                _coerce_object_mapping(data.get("assembly"), field_name="assembly")
            ),
            dnp=_PcbSvgDnpConfig.from_dict(
                _coerce_object_mapping(data.get("dnp"), field_name="dnp")
            ),
            diodes=_PcbSvgDiodeConfig.from_dict(
                _coerce_object_mapping(data.get("diodes"), field_name="diodes")
            ),
            pin1=_PcbSvgPin1Config.from_dict(
                _coerce_object_mapping(data.get("pin1"), field_name="pin1")
            ),
            components=_coerce_component_overrides(data),
            layer_outputs=_coerce_layer_outputs_config(data, default),
            views=_coerce_view_configs(data, default),
        )

    def enabled_views(self) -> list[_PcbSvgViewConfig]:
        return [view for view in self.views if view.enabled]

    def resolved_styles_for_view(self, view: _PcbSvgViewConfig) -> dict[str, dict[str, object]]:
        return merge_pcb_svg_styles(self.global_options.styles, view.styles)

    def resolved_pin1_for_view(self, view: _PcbSvgViewConfig) -> _PcbSvgPin1Config:
        return view.pin1 or self.pin1

    def to_dict(self) -> dict[str, object]:
        return {
            "schema": PCB_SVG_CONFIG_SCHEMA,
            "global": self.global_options.to_dict(),
            "assembly": self.assembly.to_dict(),
            "dnp": self.dnp.to_dict(),
            "diodes": self.diodes.to_dict(),
            "pin1": self.pin1.to_dict(),
            "components": {
                designator: override.to_dict()
                for designator, override in sorted(self.components.items())
            },
            "layer_outputs": self.layer_outputs,
            "views": [view.to_dict() for view in self.views],
        }


def pcb_svg_default_config_text(config: _PcbSvgConfig | None = None) -> str:
    """Render the editable pcb-svg default config."""
    return render_commented_jsonc(
        (config or _PcbSvgConfig.default()).to_dict(),
        comments_by_path=_PCB_SVG_CONFIG_COMMENTS,
        comments_by_key=_PCB_SVG_COMMENTS_BY_KEY,
        header_lines=_PCB_SVG_CONFIG_HEADER,
    )


def resolve_config_output_path(
    output_dir: Path,
    template: str,
    *,
    board: str,
    view: str,
) -> Path:
    """Resolve an output path template under an output directory."""
    rendered = template.format(board=board, view=view)
    path = Path(rendered)
    if path.is_absolute():
        return path
    return output_dir / path


__all__ = [
    "PCB_DEFAULT_SVG_SCALE",
    "PCB_SVG_CONFIG_FILENAME",
    "PCB_SVG_CONFIG_SCHEMA",
    "PCB_SVG_SPECIAL_LAYERS",
    "_PcbSvgConfig",
    "_PcbSvgViewConfig",
    "is_synthetic_layer_token",
    "normalize_layer_token",
    "parse_pcb_layer_selector",
    "physical_layer_from_token",
    "pcb_svg_default_config_text",
    "resolve_config_output_path",
]

"""Config helpers for PCB layer STEP fixture-alignment output."""

from __future__ import annotations

import re
from copy import deepcopy

from kicad_cruncher.config_json import JsoncCommentMap, enum_help, render_commented_jsonc

PCB_LAYER_STEP_CONFIG_SCHEMA_V2 = "wn.kicad_cruncher.pcb_layer_step.config.v2"

_PCB_LAYER_STEP_DEFAULT_CONFIG_PAYLOAD: dict[str, object] = {
    "schema": PCB_LAYER_STEP_CONFIG_SCHEMA_V2,
    "defaults": {
        "pcbdoc": None,
        "layer": "bottom",
        "z_mm": 0.0,
        "thickness_mm": 0.035,
        "include_board_outline": True,
        "board_outline": {
            "color": "#FFFF00",
            "cutout_color": "#FFFF00",
            "cutouts": True,
            "width_mm": 0.2,
            "fuse": True,
        },
    },
    "outputs": [
        {
            "name": "fixture_alignment",
            "output_step": "{board}__fixture_alignment.step",
            "features": {
                "defaults": {"color": "#B87333"},
                "component_pads": {
                    "mode": "matching_designators",
                    "include_designators": ["TP*", "M*"],
                    "color": "#B87333",
                    "step_body_name": "component_pads",
                    "thickness_bias_mm": 0.010,
                    "highlight_rules": [
                        {
                            "designators": ["TP*"],
                            "color": "red",
                            "step_body_name": "test_points",
                        }
                    ],
                },
                "free_pads": {
                    "enabled": False,
                    "color": "#B87333",
                    "step_body_name": "free_pads",
                    "thickness_bias_mm": 0.010,
                },
                "tracks": {
                    "enabled": False,
                    "color": "#B87333",
                    "step_body_name": "tracks",
                    "thickness_bias_mm": 0.0,
                },
                "arcs": {
                    "enabled": False,
                    "color": "#B87333",
                    "step_body_name": "arcs",
                    "thickness_bias_mm": 0.0,
                },
                "fills": {
                    "enabled": False,
                    "color": "#B87333",
                    "step_body_name": "fills",
                    "thickness_bias_mm": 0.0,
                },
                "polygons": {
                    "enabled": False,
                    "color": "#7A8F2A",
                    "step_body_name": "polygons",
                    "thickness_bias_mm": 0.003,
                },
                "regions": {
                    "enabled": False,
                    "color": "#7A8F2A",
                    "step_body_name": "regions",
                    "thickness_bias_mm": 0.003,
                },
                "vias": {
                    "enabled": False,
                    "color": "#C08540",
                    "step_body_name": "vias",
                    "thickness_bias_mm": 0.006,
                },
            },
            "drills": {
                "mode": "overlay",
                "selected_component_mode": "cut",
                "other_component_mode": "none",
                "free_pad_mode": "overlay",
                "via_mode": "inherit",
                "minimum_diameter_mm": 0.85,
                "shape": "ring",
                "color": "#666666",
                "plated_color": "#666666",
                "non_plated_color": "#00AEEF",
                "ring_width_mm": 0.12,
                "plated_ring_shape": "pad",
                "overlay_thickness_mm": 0.001,
            },
            "fuse_copper": False,
        }
    ],
}

_PCB_LAYER_STEP_HEADER_LINES = (
    "kicad-cruncher pcb-layer-step configuration",
    "",
    "pcb-layer-step creates compact fixture-alignment models, not full fabrication STEP exports.",
    "This file is JSONC, not strict JSON. Comments and trailing commas are accepted.",
    "Generated configs use the pcb-layer-step.jsonc file name.",
    "Auto-discovery no longer probes old .json names.",
    "Feature bodies are extruded by Geometer from 2D regions.",
    "thickness_mm controls nominal layer thickness.",
    "thickness_bias_mm is symmetric and prevents z-fighting.",
    "",
    "CONFIG SHAPE",
    "defaults: shared settings copied into every output.",
    "outputs: one or more output definitions; each output overrides only what it needs.",
    "",
    "COORDINATES",
    "Geometer receives XY geometry relative to the KiCad aux axis origin",
    "from setup/aux_axis_origin.",
    "Boards without an aux axis origin use absolute KiCad PCB coordinates in millimeters.",
    "",
    "COMMON OUTPUT FIELDS",
    "name, output_step, pcbdoc, layer, z_mm, thickness_mm",
    "copper_color, include_copper, include_board_outline, include_board_cutouts",
    "include_poured_polygons, cut_holes, drill_hole_mode, max_boolean_drill_cuts",
    "drill_hole_color, drill_plated_hole_color, drill_non_plated_hole_color",
    "drill_overlay_thickness_mm, drill_minimum_diameter_mm, drill_hole_shape",
    "drill_ring_width_mm, drill_plated_ring_shape, drill_selected_component_mode",
    "drill_other_component_mode, drill_free_pad_mode, drill_via_mode",
    "fuse_copper, fuse_board_outline, arc_segments",
    "include_tracks, include_arcs, include_fills, include_regions, include_vias",
    "include_component_pads, include_free_pads, include_designators",
    "",
    "DESIGNATOR PATTERNS",
    "Patterns are case-insensitive shell-style matches.",
    'Examples: ["TP*"], ["TP*", "J*", "U1"], or ["M*"].',
    "",
    "COLOR VALUES",
    "Colors may be #RRGGBB values or names.",
    "Names: black, blue, brown, copper, gray, green, grey, orange, purple, red, white, yellow.",
    "",
    "DRILL MODES",
    "Options: none, cut, overlay, auto. Scoped modes add inherit.",
    "",
    "DRILL SHAPES",
    "Options: solid, ring. plated_ring_shape options: annulus, pad.",
    "",
    "CLI OVERRIDES",
    "Run: kicad-cruncher pcb-layer-step --help",
)

_PCB_LAYER_STEP_COMMENTS: JsoncCommentMap = {
    ("schema",): "pcb-layer-step config contract id.",
    ("defaults",): "Shared settings merged into each output.",
    ("defaults", "pcbdoc"): (
        "Optional .kicad_pcb selector when a .kicad_pro contains more",
        "than one board.",
    ),
    ("defaults", "layer"): enum_help(
        "PCB layer selector.",
        ("bottom", "top", "B.Cu", "F.Cu", "native KiCad layer name", "layer id"),
    ),
    ("defaults", "z_mm"): "Bottom Z plane for generated bodies.",
    ("defaults", "thickness_mm"): "Nominal extrusion thickness before feature bias.",
    ("defaults", "include_board_outline"): "Include separate board-outline body.",
    ("defaults", "board_outline"): "Board outline and cutout body styling.",
    ("defaults", "board_outline", "color"): "STEP color for the outer board outline body.",
    (
        "defaults",
        "board_outline",
        "cutout_color",
    ): "STEP color for interior board cutout outline bodies.",
    (
        "defaults",
        "board_outline",
        "cutouts",
    ): "Include separate bodies around interior board cutouts.",
    ("defaults", "board_outline", "width_mm"): "Outline stroke body width in millimeters.",
    ("defaults", "board_outline", "fuse"): "Request Geometer planar fusion for outline bodies.",
    ("outputs",): "Output definitions. Each item inherits defaults and overrides selected fields.",
    ("outputs", "name"): "Output id for filename tokens and logs.",
    (
        "outputs",
        "output_step",
    ): (
        "Output STEP path template.",
        "Supports {board}, {Board}, {layer}, {Layer}, {output}, and {Output}.",
    ),
    (
        "outputs",
        "features",
    ): "Copper feature selection, color, body-name, and thickness-bias policy.",
    (
        "outputs",
        "features",
        "defaults",
    ): "Fallback feature styling. color is the fallback copper color.",
    ("outputs", "features", "component_pads"): "Component-owned pad selection and styling.",
    ("outputs", "features", "component_pads", "mode"): enum_help(
        "Component pad mode.",
        ("none", "all", "matching_designators"),
    ),
    (
        "outputs",
        "features",
        "component_pads",
        "include_designators",
    ): "Case-insensitive shell patterns for component designators.",
    (
        "outputs",
        "features",
        "component_pads",
        "highlight_rules",
    ): "Rules that split matching component pads into separately colored STEP bodies.",
    (
        "outputs",
        "features",
        "component_pads",
        "highlight_rules",
        "designators",
    ): "Case-insensitive shell patterns for this highlight rule.",
    (
        "outputs",
        "features",
        "free_pads",
    ): "Pads not owned by a component; KiCad pads are normally footprint-owned.",
    ("outputs", "features", "tracks"): "Copper track primitives.",
    ("outputs", "features", "arcs"): "Copper arc primitives.",
    ("outputs", "features", "fills"): "Reserved for fill-style feature support.",
    ("outputs", "features", "polygons"): "Poured polygon copper primitives.",
    (
        "outputs",
        "features",
        "regions",
    ): "Filled copper graphics such as gr_poly, gr_rect, and gr_circle.",
    ("outputs", "features", "vias"): "Via copper pad primitives.",
    ("outputs", "drills"): "Drill and slot visualization policy.",
    ("outputs", "drills", "mode"): enum_help(
        "Global drill mode.",
        ("auto", "cut", "overlay", "none"),
    ),
    ("outputs", "drills", "selected_component_mode"): enum_help(
        "Drill mode for pads selected by component_pads.",
        ("inherit", "cut", "overlay", "none"),
    ),
    ("outputs", "drills", "other_component_mode"): enum_help(
        "Drill mode for unselected component-owned pads.",
        ("inherit", "cut", "overlay", "none"),
    ),
    ("outputs", "drills", "free_pad_mode"): enum_help(
        "Drill mode for pads not owned by a component.",
        ("inherit", "cut", "overlay", "none"),
    ),
    ("outputs", "drills", "via_mode"): enum_help(
        "Drill mode for via holes.",
        ("inherit", "cut", "overlay", "none"),
    ),
    ("outputs", "drills", "minimum_diameter_mm"): "Omit drill overlays/cuts below this diameter.",
    ("outputs", "drills", "shape"): enum_help(
        "Overlay shape.",
        ("solid", "ring"),
    ),
    ("outputs", "drills", "color"): "Default STEP color for drill overlay bodies.",
    ("outputs", "drills", "plated_color"): "STEP color for plated drill overlay bodies.",
    ("outputs", "drills", "non_plated_color"): "STEP color for non-plated drill overlay bodies.",
    ("outputs", "drills", "ring_width_mm"): "Fixed annulus width when drill shape is ring.",
    ("outputs", "drills", "plated_ring_shape"): enum_help(
        "Plated drill ring policy.",
        ("annulus", "pad"),
    ),
    ("outputs", "drills", "overlay_thickness_mm"): "Z thickness for separate drill overlay bodies.",
    ("outputs", "fuse_copper"): "Request Geometer planar fusion for copper bodies.",
}

_PCB_LAYER_STEP_KEY_COMMENTS: dict[str, str] = {
    "enabled": "Enable this feature body.",
    "color": "STEP color for this body or rule.",
    "step_body_name": "Stable Geometer STEP body id/name.",
    "thickness_bias_mm": (
        "Symmetric Z bias that prevents overlapping colored bodies from z-fighting."
    ),
    "designators": "Case-insensitive shell-style designator patterns.",
}


def pcb_layer_step_default_config_text() -> str:
    """Render the editable pcb-layer-step default config."""
    return render_commented_jsonc(
        default_pcb_layer_step_config_payload(),
        comments_by_path=_PCB_LAYER_STEP_COMMENTS,
        comments_by_key=_PCB_LAYER_STEP_KEY_COMMENTS,
        header_lines=_PCB_LAYER_STEP_HEADER_LINES,
    )


def default_pcb_layer_step_config_payload() -> dict[str, object]:
    """Return the generated pcb-layer-step default config payload."""
    return deepcopy(_PCB_LAYER_STEP_DEFAULT_CONFIG_PAYLOAD)


PCB_LAYER_STEP_DEFAULT_CONFIG_TEXT = pcb_layer_step_default_config_text()


def resolve_pcb_layer_selector(selector: str | int | None) -> str:
    """Resolve CLI/user layer selectors to a KiCad canonical layer token."""
    if selector is None:
        return "B.Cu"
    if isinstance(selector, int):
        return _layer_by_numeric_selector(selector)

    text = str(selector).strip()
    if not text:
        return "B.Cu"
    if text.isdigit():
        return _layer_by_numeric_selector(int(text))

    normalized = _normalize_layer_selector(text)
    layer = _layer_aliases().get(normalized)
    if layer is not None:
        return layer
    return _canonical_layer_spelling(text)


def _normalize_layer_selector(value: str) -> str:
    return re.sub(r"[\s_\-]+", "", value).upper()


def _layer_aliases() -> dict[str, str]:
    return {
        "TOP": "F.Cu",
        "TOPLAYER": "F.Cu",
        "FRONT": "F.Cu",
        "FCU": "F.Cu",
        "F.CU": "F.Cu",
        "BOTTOM": "B.Cu",
        "BOTTOMLAYER": "B.Cu",
        "BOT": "B.Cu",
        "BACK": "B.Cu",
        "BCU": "B.Cu",
        "B.CU": "B.Cu",
        "EDGE.CUTS": "Edge.Cuts",
        "EDGECUTS": "Edge.Cuts",
    }


def _layer_by_numeric_selector(value: int) -> str:
    if value == 0:
        return "F.Cu"
    if value == 31:
        return "B.Cu"
    if 1 <= value <= 30:
        return f"In{value}.Cu"
    raise ValueError(f"Unknown KiCad layer ordinal: {value!r}")


def _canonical_layer_spelling(value: str) -> str:
    lower = value.casefold()
    common = {
        "f.cu": "F.Cu",
        "b.cu": "B.Cu",
        "edge.cuts": "Edge.Cuts",
        "f.silks": "F.SilkS",
        "b.silks": "B.SilkS",
        "f.fab": "F.Fab",
        "b.fab": "B.Fab",
        "f.mask": "F.Mask",
        "b.mask": "B.Mask",
        "f.paste": "F.Paste",
        "b.paste": "B.Paste",
    }
    return common.get(lower, value)

"""Generate a STEP alignment model for one KiCad PCB layer."""

from __future__ import annotations

import fnmatch
import json
import logging
import math
import re
from collections.abc import Callable, Iterable, Mapping, MutableMapping
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import TYPE_CHECKING, Protocol, cast

from kicad_monkey.kicad_base import PadShape, PadType
from kicad_monkey.kicad_geometry import rotate_point
from kicad_monkey.kicad_pcb_pad_svg import pad_on_layer
from kicad_monkey.kicad_pcb_polygon_ops import PolygonSet

from kicad_cruncher.config_json import load_json_config
from kicad_cruncher.kicad_cruncher_pcb_layer_step_config import (
    PCB_LAYER_STEP_CONFIG_SCHEMA_V2,
    pcb_layer_step_default_config_text,
    resolve_pcb_layer_selector,
)
from kicad_cruncher.kicad_cruncher_pcb_model_pose import transform_footprint_local_to_board
from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
    _BoardRegion,
    _classify_edge_cut_regions,
    _interior_board_regions,
    _outer_board_region,
)

if TYPE_CHECKING:
    from kicad_monkey.kicad_pcb import KiCadPcb
    from kicad_monkey.kicad_pcb_footprint import Footprint

log = logging.getLogger(__name__)

DEFAULT_COPPER_COLOR = "#B87333"
DEFAULT_OUTLINE_COLOR = "#FFFF00"
DEFAULT_BOARD_CUTOUT_COLOR = "#FFFF00"
DEFAULT_DRILL_HOLE_COLOR = "#FFFFFF"
DEFAULT_MAX_BOOLEAN_DRILL_CUTS = 128
PCB_LAYER_STEP_CONFIG_FILENAME = "pcb-layer-step.jsonc"
DEFAULT_PAD_THICKNESS_BIAS_MM = 0.010
DEFAULT_VIA_THICKNESS_BIAS_MM = 0.006
DEFAULT_POLYGON_THICKNESS_BIAS_MM = 0.003
DEFAULT_TRACE_THICKNESS_BIAS_MM = 0.0
DRILL_HOLE_MODE_AUTO = "auto"
DRILL_HOLE_MODE_CUT = "cut"
DRILL_HOLE_MODE_OVERLAY = "overlay"
DRILL_HOLE_MODE_NONE = "none"
DRILL_SCOPE_MODE_INHERIT = "inherit"
DRILL_HOLE_SHAPE_SOLID = "solid"
DRILL_HOLE_SHAPE_RING = "ring"
DRILL_HOLE_SHAPES = frozenset({DRILL_HOLE_SHAPE_SOLID, DRILL_HOLE_SHAPE_RING})
DRILL_PLATED_RING_SHAPE_ANNULUS = "annulus"
DRILL_PLATED_RING_SHAPE_PAD = "pad"
DRILL_PLATED_RING_SHAPES = frozenset(("annulus", "pad"))
DRILL_HOLE_MODES = frozenset(
    {
        DRILL_HOLE_MODE_AUTO,
        DRILL_HOLE_MODE_CUT,
        DRILL_HOLE_MODE_OVERLAY,
        DRILL_HOLE_MODE_NONE,
    }
)
DRILL_SCOPE_MODES = frozenset(
    {
        DRILL_SCOPE_MODE_INHERIT,
        DRILL_HOLE_MODE_CUT,
        DRILL_HOLE_MODE_OVERLAY,
        DRILL_HOLE_MODE_NONE,
    }
)
_NON_COPPER_BODY_IDS = frozenset(
    {
        "board_outline",
        "board_cutouts",
        "drill_holes",
        "plated_drill_holes",
        "non_plated_drill_holes",
    }
)
_COLOR_NAMES = {
    "black": "#000000",
    "blue": "#0000FF",
    "brown": "#A52A2A",
    "copper": DEFAULT_COPPER_COLOR,
    "gray": "#808080",
    "green": "#008000",
    "grey": "#808080",
    "orange": "#FFA500",
    "purple": "#800080",
    "red": "#FF0000",
    "white": "#FFFFFF",
    "yellow": "#FFFF00",
}


class _GeometerPlanarStepModule(Protocol):
    def write_planar_step(self, request: dict[str, object], output_path: Path) -> None:
        """Write a planar STEP file from a Geometer request."""


_OvalSegmentMethod = Callable[
    [float, float],
    tuple[tuple[float, float], tuple[float, float], float],
]
_PadPolygonMethod = Callable[[float, float], list[tuple[float, float]]]
_RoundRectPolygonMethod = Callable[[float, float, float], list[tuple[float, float]]]


@dataclass(frozen=True, slots=True)
class _PadColorRule:
    designators: tuple[str, ...]
    color: str
    step_body_name: str = "matched_pads"


@dataclass(frozen=True, slots=True)
class PcbLayerStepOptions:
    """Options for one-layer PCB STEP export."""

    layer: str = "B.Cu"
    thickness_mm: float = 0.035
    z_mm: float = 0.0
    copper_color: str = DEFAULT_COPPER_COLOR
    outline_width_mm: float = 0.2
    outline_color: str = DEFAULT_OUTLINE_COLOR
    board_cutout_color: str = DEFAULT_BOARD_CUTOUT_COLOR
    include_board_cutouts: bool = True
    include_copper: bool = True
    include_board_outline: bool = True
    include_poured_polygons: bool = True
    cut_holes: bool = True
    drill_hole_mode: str = DRILL_HOLE_MODE_AUTO
    max_boolean_drill_cuts: int = DEFAULT_MAX_BOOLEAN_DRILL_CUTS
    drill_hole_color: str = DEFAULT_DRILL_HOLE_COLOR
    drill_plated_hole_color: str = DEFAULT_DRILL_HOLE_COLOR
    drill_non_plated_hole_color: str = DEFAULT_DRILL_HOLE_COLOR
    drill_overlay_thickness_mm: float = 0.001
    drill_minimum_diameter_mm: float = 0.0
    drill_hole_shape: str = DRILL_HOLE_SHAPE_SOLID
    drill_ring_width_mm: float = 0.12
    drill_plated_ring_shape: str = DRILL_PLATED_RING_SHAPE_ANNULUS
    drill_selected_component_mode: str = DRILL_SCOPE_MODE_INHERIT
    drill_other_component_mode: str = DRILL_SCOPE_MODE_INHERIT
    drill_free_pad_mode: str = DRILL_SCOPE_MODE_INHERIT
    drill_via_mode: str = DRILL_SCOPE_MODE_INHERIT
    fuse_copper: bool = True
    fuse_board_outline: bool = True
    arc_segments: int = 32
    include_tracks: bool = True
    include_arcs: bool = True
    include_fills: bool = True
    include_regions: bool = True
    include_vias: bool = True
    include_component_pads: bool = True
    include_free_pads: bool = True
    include_designators: tuple[str, ...] = ()
    pad_color_rules: tuple[_PadColorRule, ...] = ()
    track_color: str | None = None
    track_body: str = "tracks"
    arc_color: str | None = None
    arc_body: str = "arcs"
    fill_color: str | None = None
    fill_body: str = "fills"
    polygon_color: str | None = None
    polygon_body: str = "polygons"
    region_color: str | None = None
    region_body: str = "regions"
    via_color: str | None = None
    via_body: str = "vias"
    component_pad_color: str | None = None
    component_pad_body: str = "component_pads"
    free_pad_color: str | None = None
    free_pad_body: str = "free_pads"
    track_thickness_bias_mm: float = DEFAULT_TRACE_THICKNESS_BIAS_MM
    arc_thickness_bias_mm: float = DEFAULT_TRACE_THICKNESS_BIAS_MM
    fill_thickness_bias_mm: float = DEFAULT_TRACE_THICKNESS_BIAS_MM
    polygon_thickness_bias_mm: float = DEFAULT_POLYGON_THICKNESS_BIAS_MM
    region_thickness_bias_mm: float = DEFAULT_POLYGON_THICKNESS_BIAS_MM
    via_thickness_bias_mm: float = DEFAULT_VIA_THICKNESS_BIAS_MM
    component_pad_thickness_bias_mm: float = DEFAULT_PAD_THICKNESS_BIAS_MM
    free_pad_thickness_bias_mm: float = DEFAULT_PAD_THICKNESS_BIAS_MM


@dataclass(frozen=True, slots=True)
class PcbLayerStepResult:
    """Summary of a generated one-layer PCB STEP export."""

    output_path: Path
    manifest_path: Path
    board_name: str
    layer: str
    copper_body_count: int
    outline_body_count: int
    drill_cut_count: int
    source_input: str | None


def _coerce_optional_str(value: object) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _coerce_str(value: object, default: str) -> str:
    if value is None:
        return default
    return str(value)


def _coerce_color(value: object, default: str) -> str:
    text = _coerce_str(value, default).strip()
    named = _COLOR_NAMES.get(text.casefold())
    return named or text


def _coerce_optional_color(value: object) -> str | None:
    if value is None:
        return None
    return _coerce_color(value, DEFAULT_COPPER_COLOR)


def _coerce_str_tuple(value: object) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        text = value.strip()
        return (text,) if text else ()
    if isinstance(value, Iterable):
        return tuple(text for item in value if (text := str(item).strip()))
    raise ValueError(f"Invalid string list in pcb-layer-step config: {value!r}")


def _coerce_float(value: object, default: float) -> float:
    if value is None:
        return default
    if not isinstance(value, str | int | float):
        raise ValueError(f"Invalid numeric value in pcb-layer-step config: {value!r}")
    try:
        return float(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid numeric value in pcb-layer-step config: {value!r}") from exc


def _coerce_bool(value: object, default: bool) -> bool:
    if value is None:
        return default
    if isinstance(value, bool):
        return value
    if isinstance(value, int | float):
        return bool(value)
    if isinstance(value, str):
        normalized = value.strip().lower()
        if normalized in {"1", "true", "yes", "y", "on"}:
            return True
        if normalized in {"0", "false", "no", "n", "off"}:
            return False
    raise ValueError(f"Invalid boolean value in pcb-layer-step config: {value!r}")


def _coerce_drill_hole_mode(value: object, *, cut_holes: bool) -> str:
    if value is None:
        return DRILL_HOLE_MODE_AUTO if cut_holes else DRILL_HOLE_MODE_NONE
    normalized = str(value).strip().casefold().replace("-", "_")
    aliases = {
        "boolean": DRILL_HOLE_MODE_CUT,
        "boolean_cut": DRILL_HOLE_MODE_CUT,
        "cutout": DRILL_HOLE_MODE_CUT,
        "cutouts": DRILL_HOLE_MODE_CUT,
        "cuts": DRILL_HOLE_MODE_CUT,
        "off": DRILL_HOLE_MODE_NONE,
        "omit": DRILL_HOLE_MODE_NONE,
    }
    mode = aliases.get(normalized, normalized)
    if mode not in DRILL_HOLE_MODES:
        raise ValueError(f"Invalid drill_hole_mode in pcb-layer-step config: {value!r}")
    return mode


def _coerce_drill_hole_shape(value: object, default: str) -> str:
    if value is None:
        return default
    shape = str(value).strip().casefold().replace("-", "_")
    if shape not in DRILL_HOLE_SHAPES:
        raise ValueError(f"Invalid drill_hole_shape in pcb-layer-step config: {value!r}")
    return shape


def _coerce_drill_plated_ring_shape(value: object, default: str) -> str:
    if value is None:
        return default
    normalized = str(value).strip().casefold().replace("-", "_")
    aliases = {
        "hole": DRILL_PLATED_RING_SHAPE_ANNULUS,
        "drill": DRILL_PLATED_RING_SHAPE_ANNULUS,
        "full_pad": DRILL_PLATED_RING_SHAPE_PAD,
        "pad_shape": DRILL_PLATED_RING_SHAPE_PAD,
    }
    shape = aliases.get(normalized, normalized)
    if shape not in DRILL_PLATED_RING_SHAPES:
        raise ValueError(f"Invalid drill plated ring shape in pcb-layer-step config: {value!r}")
    return shape


def _coerce_drill_scope_mode(value: object, default: str) -> str:
    if value is None:
        return default
    normalized = str(value).strip().casefold().replace("-", "_")
    aliases = {
        "default": DRILL_SCOPE_MODE_INHERIT,
        "global": DRILL_SCOPE_MODE_INHERIT,
        "off": DRILL_HOLE_MODE_NONE,
        "omit": DRILL_HOLE_MODE_NONE,
        "boolean": DRILL_HOLE_MODE_CUT,
        "boolean_cut": DRILL_HOLE_MODE_CUT,
        "cutout": DRILL_HOLE_MODE_CUT,
        "cutouts": DRILL_HOLE_MODE_CUT,
        "cuts": DRILL_HOLE_MODE_CUT,
    }
    mode = aliases.get(normalized, normalized)
    if mode not in DRILL_SCOPE_MODES:
        raise ValueError(f"Invalid scoped drill mode in pcb-layer-step config: {value!r}")
    return mode


def _coerce_pad_highlight_rules(value: object) -> tuple[_PadColorRule, ...]:
    if value is None:
        return ()
    if not isinstance(value, list):
        raise ValueError(
            "pcb-layer-step config field "
            "'features.component_pads.highlight_rules' must be a list"
        )
    rules: list[_PadColorRule] = []
    for index, raw_rule in enumerate(value):
        if not isinstance(raw_rule, dict):
            raise ValueError(
                "pcb-layer-step features.component_pads.highlight_rules"
                f"[{index}] must be an object"
            )
        designators = _coerce_str_tuple(raw_rule.get("designators"))
        if not designators:
            raise ValueError(
                "pcb-layer-step features.component_pads.highlight_rules"
                f"[{index}] requires designators"
            )
        rules.append(
            _PadColorRule(
                designators=designators,
                color=_coerce_color(raw_rule.get("color"), DEFAULT_COPPER_COLOR),
                step_body_name=str(raw_rule.get("step_body_name") or "matched_pads"),
            )
        )
    return tuple(rules)


def _config_mapping(value: object, field_name: str) -> Mapping[str, object]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ValueError(f"pcb-layer-step config field '{field_name}' must be an object")
    return value


def _feature_value(features: Mapping[str, object], name: str, *aliases: str) -> object:
    for key in (name, *aliases):
        if key in features:
            return features[key]
    return None


def _feature_enabled(
    *,
    features: Mapping[str, object],
    merged: Mapping[str, object],
    name: str,
    default: bool,
    legacy_key: str,
    aliases: tuple[str, ...] = (),
) -> bool:
    value = _feature_value(features, name, *aliases)
    if isinstance(value, Mapping):
        return _coerce_bool(value.get("enabled"), default)
    if value is not None:
        return _coerce_bool(value, default)
    return _coerce_bool(merged.get(legacy_key), default)


def _feature_color_and_body(
    *,
    features: Mapping[str, object],
    merged: Mapping[str, object],
    name: str,
    body_default: str,
    color_key: str,
    body_key: str,
    aliases: tuple[str, ...] = (),
) -> tuple[str | None, str]:
    del color_key, body_key
    color_value = None
    body_value = None
    candidate = _feature_value(features, name, *aliases)
    if isinstance(candidate, Mapping):
        color_value = candidate.get("color", color_value)
        body_value = candidate.get("step_body_name", body_value)
    elif candidate is not None and not isinstance(candidate, bool):
        color_value = candidate
    return (
        _coerce_optional_color(color_value),
        str(body_value or body_default),
    )


def _feature_thickness_bias(
    *,
    features: Mapping[str, object],
    merged: Mapping[str, object],
    name: str,
    default: float,
    legacy_key: str,
    aliases: tuple[str, ...] = (),
) -> float:
    del merged, legacy_key
    value = None
    candidate = _feature_value(features, name, *aliases)
    if isinstance(candidate, Mapping):
        value = candidate.get("thickness_bias_mm", value)
    return _coerce_float(value, default)


def _reject_removed_config_fields(
    *,
    merged: Mapping[str, object],
    features: Mapping[str, object],
) -> None:
    _reject_removed_root_fields(merged)
    _reject_removed_feature_body_fields(features)


def _reject_removed_root_fields(merged: Mapping[str, object]) -> None:
    removed_root_fields = {
        "colors": "use features.defaults, feature color fields, and "
        "features.component_pads.highlight_rules",
        "pad_color_rules": "use features.component_pads.highlight_rules",
        "pad_rules": "use features.component_pads.highlight_rules",
        "thickness_bias": "put thickness_bias_mm on each feature entry",
        "thickness_bias_mm": "put thickness_bias_mm on each feature entry",
    }
    for field_name, replacement in removed_root_fields.items():
        if field_name in merged:
            raise ValueError(
                f"pcb-layer-step config field '{field_name}' was removed; {replacement}"
            )


def _reject_removed_feature_body_fields(features: Mapping[str, object]) -> None:
    for feature_name, raw_feature in features.items():
        if isinstance(raw_feature, Mapping) and "body" in raw_feature:
            raise ValueError(
                f"pcb-layer-step config field 'features.{feature_name}.body' "
                "was removed; use step_body_name"
            )
        if feature_name == "component_pads" and isinstance(raw_feature, Mapping):
            _reject_removed_highlight_rule_body_fields(raw_feature)


def _reject_removed_highlight_rule_body_fields(
    component_pads: Mapping[str, object],
) -> None:
    highlight_rules = component_pads.get("highlight_rules")
    if not isinstance(highlight_rules, list):
        return
    for index, rule in enumerate(highlight_rules):
        if isinstance(rule, Mapping) and "body" in rule:
            raise ValueError(
                "pcb-layer-step config field "
                f"'features.component_pads.highlight_rules[{index}].body' "
                "was removed; use step_body_name"
            )


def _merge_options(data: Mapping[str, object]) -> dict[str, object]:
    options = data.get("options")
    if options is None:
        return dict(data)
    if not isinstance(options, Mapping):
        raise ValueError("pcb-layer-step config field 'options' must be an object")
    return {**dict(data), **dict(options)}


def _root_config_defaults(data: Mapping[str, object]) -> dict[str, object]:
    return {key: value for key, value in data.items() if key not in {"defaults", "outputs"}}


def _output_config_dicts(data: Mapping[str, object]) -> tuple[Mapping[str, object], ...]:
    raw_outputs = data.get("outputs")
    if not isinstance(raw_outputs, list) or not raw_outputs:
        raise ValueError("pcb-layer-step config field 'outputs' must be a non-empty list")
    outputs: list[Mapping[str, object]] = []
    for index, raw_output in enumerate(raw_outputs):
        if not isinstance(raw_output, Mapping):
            raise ValueError(f"pcb-layer-step config outputs[{index}] must be an object")
        outputs.append(raw_output)
    return tuple(outputs)


def _component_pad_settings(
    *,
    features: Mapping[str, object],
    merged: Mapping[str, object],
    default: PcbLayerStepConfig,
) -> tuple[bool, object]:
    component_pads = features.get("component_pads")
    component_pad_designators = merged.get("include_designators")
    include_component_pads = default.include_component_pads
    if isinstance(component_pads, Mapping):
        mode = str(component_pads.get("mode") or "all").strip().casefold()
        include_component_pads = mode != "none"
        if "enabled" in component_pads:
            include_component_pads = _coerce_bool(
                component_pads.get("enabled"),
                include_component_pads,
            )
        component_pad_designators = component_pads.get(
            "include_designators",
            component_pad_designators,
        )
    elif component_pads is not None:
        include_component_pads = _coerce_bool(component_pads, default.include_component_pads)
    else:
        include_component_pads = _coerce_bool(
            merged.get("include_component_pads"),
            default.include_component_pads,
        )
    return include_component_pads, component_pad_designators


def _component_pad_highlight_rules(
    features: Mapping[str, object],
) -> tuple[_PadColorRule, ...]:
    component_pads = features.get("component_pads")
    if not isinstance(component_pads, Mapping):
        return ()
    return _coerce_pad_highlight_rules(component_pads.get("highlight_rules"))


def _drill_color_source(*, drills: Mapping[str, object], merged: Mapping[str, object]) -> object:
    return drills.get("color", merged.get("drill_hole_color"))


def _drill_plated_color_source(
    *,
    drills: Mapping[str, object],
    merged: Mapping[str, object],
    drill_color: object,
) -> object:
    return drills.get("plated_color", merged.get("drill_plated_hole_color", drill_color))


def _drill_non_plated_color_source(
    *,
    drills: Mapping[str, object],
    merged: Mapping[str, object],
    drill_color: object,
) -> object:
    return drills.get("non_plated_color", merged.get("drill_non_plated_hole_color", drill_color))


@dataclass(frozen=True, slots=True)
class PcbLayerStepConfig:
    """JSON config for one-layer PCB STEP export."""

    schema: str = PCB_LAYER_STEP_CONFIG_SCHEMA_V2
    name: str | None = None
    output_step: str | None = None
    pcbdoc: str | None = None
    layer: str = "bottom"
    thickness_mm: float = 0.035
    z_mm: float = 0.0
    copper_color: str = DEFAULT_COPPER_COLOR
    outline_width_mm: float = 0.2
    outline_color: str = DEFAULT_OUTLINE_COLOR
    board_cutout_color: str = DEFAULT_BOARD_CUTOUT_COLOR
    include_board_cutouts: bool = True
    include_copper: bool = True
    include_board_outline: bool = True
    include_poured_polygons: bool = True
    cut_holes: bool = True
    drill_hole_mode: str = DRILL_HOLE_MODE_AUTO
    max_boolean_drill_cuts: int = DEFAULT_MAX_BOOLEAN_DRILL_CUTS
    drill_hole_color: str = DEFAULT_DRILL_HOLE_COLOR
    drill_plated_hole_color: str = DEFAULT_DRILL_HOLE_COLOR
    drill_non_plated_hole_color: str = DEFAULT_DRILL_HOLE_COLOR
    drill_overlay_thickness_mm: float = 0.001
    drill_minimum_diameter_mm: float = 0.0
    drill_hole_shape: str = DRILL_HOLE_SHAPE_SOLID
    drill_ring_width_mm: float = 0.12
    drill_plated_ring_shape: str = DRILL_PLATED_RING_SHAPE_ANNULUS
    drill_selected_component_mode: str = DRILL_SCOPE_MODE_INHERIT
    drill_other_component_mode: str = DRILL_SCOPE_MODE_INHERIT
    drill_free_pad_mode: str = DRILL_SCOPE_MODE_INHERIT
    drill_via_mode: str = DRILL_SCOPE_MODE_INHERIT
    fuse_copper: bool = True
    fuse_board_outline: bool = True
    arc_segments: int = 32
    include_tracks: bool = True
    include_arcs: bool = True
    include_fills: bool = True
    include_regions: bool = True
    include_vias: bool = True
    include_component_pads: bool = True
    include_free_pads: bool = True
    include_designators: tuple[str, ...] = ()
    pad_color_rules: tuple[_PadColorRule, ...] = ()
    track_color: str | None = None
    track_body: str = "tracks"
    arc_color: str | None = None
    arc_body: str = "arcs"
    fill_color: str | None = None
    fill_body: str = "fills"
    polygon_color: str | None = None
    polygon_body: str = "polygons"
    region_color: str | None = None
    region_body: str = "regions"
    via_color: str | None = None
    via_body: str = "vias"
    component_pad_color: str | None = None
    component_pad_body: str = "component_pads"
    free_pad_color: str | None = None
    free_pad_body: str = "free_pads"
    track_thickness_bias_mm: float = DEFAULT_TRACE_THICKNESS_BIAS_MM
    arc_thickness_bias_mm: float = DEFAULT_TRACE_THICKNESS_BIAS_MM
    fill_thickness_bias_mm: float = DEFAULT_TRACE_THICKNESS_BIAS_MM
    polygon_thickness_bias_mm: float = DEFAULT_POLYGON_THICKNESS_BIAS_MM
    region_thickness_bias_mm: float = DEFAULT_POLYGON_THICKNESS_BIAS_MM
    via_thickness_bias_mm: float = DEFAULT_VIA_THICKNESS_BIAS_MM
    component_pad_thickness_bias_mm: float = DEFAULT_PAD_THICKNESS_BIAS_MM
    free_pad_thickness_bias_mm: float = DEFAULT_PAD_THICKNESS_BIAS_MM
    outputs: tuple[PcbLayerStepConfig, ...] = ()

    @classmethod
    def default(cls) -> PcbLayerStepConfig:
        return cls()

    @classmethod
    def from_dict(cls, data: object) -> PcbLayerStepConfig:
        if not isinstance(data, Mapping):
            raise ValueError("pcb-layer-step config root must be a JSON object")
        if data.get("schema") != PCB_LAYER_STEP_CONFIG_SCHEMA_V2:
            raise ValueError(
                "pcb-layer-step config schema must be "
                f"{PCB_LAYER_STEP_CONFIG_SCHEMA_V2!r}; got {data.get('schema')!r}"
            )
        if "outputs" in data:
            return cls._from_outputs_dict(data)
        return cls._from_merged_dict(data)

    @classmethod
    def _from_outputs_dict(cls, data: Mapping[str, object]) -> PcbLayerStepConfig:
        defaults = _config_mapping(data.get("defaults"), "defaults")
        merged_defaults = {**_root_config_defaults(data), **dict(defaults)}
        schema = str(data.get("schema"))
        outputs = tuple(
            cls._from_merged_dict({**merged_defaults, **dict(raw_output)}, schema=schema)
            for raw_output in _output_config_dicts(data)
        )
        defaults_config = cls._from_merged_dict(merged_defaults, schema=schema)
        return replace(defaults_config, outputs=outputs)

    @classmethod
    def _from_merged_dict(
        cls,
        data: Mapping[str, object],
        *,
        schema: str | None = None,
    ) -> PcbLayerStepConfig:
        merged = _merge_options(data)
        default = cls()
        board_outline = _config_mapping(merged.get("board_outline"), "board_outline")
        features = _config_mapping(merged.get("features"), "features")
        _reject_removed_config_fields(merged=merged, features=features)
        feature_defaults = _config_mapping(features.get("defaults"), "features.defaults")
        drills = _config_mapping(merged.get("drills"), "drills")
        include_component_pads, component_pad_designators = _component_pad_settings(
            features=features,
            merged=merged,
            default=default,
        )
        track_color, track_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="tracks",
            aliases=("traces",),
            body_default="tracks",
            color_key="track_color",
            body_key="track_body",
        )
        arc_color, arc_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="arcs",
            body_default="arcs",
            color_key="arc_color",
            body_key="arc_body",
        )
        fill_color, fill_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="fills",
            body_default="fills",
            color_key="fill_color",
            body_key="fill_body",
        )
        polygon_color, polygon_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="polygons",
            aliases=("poured_polygons",),
            body_default="polygons",
            color_key="polygon_color",
            body_key="polygon_body",
        )
        region_color, region_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="regions",
            aliases=("shapebased_regions",),
            body_default="regions",
            color_key="region_color",
            body_key="region_body",
        )
        via_color, via_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="vias",
            body_default="vias",
            color_key="via_color",
            body_key="via_body",
        )
        component_pad_color, component_pad_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="component_pads",
            aliases=("pads",),
            body_default="component_pads",
            color_key="component_pad_color",
            body_key="component_pad_body",
        )
        free_pad_color, free_pad_body = _feature_color_and_body(
            features=features,
            merged=merged,
            name="free_pads",
            body_default="free_pads",
            color_key="free_pad_color",
            body_key="free_pad_body",
        )
        cut_holes = _coerce_bool(merged.get("cut_holes"), default.cut_holes)
        drill_color = _drill_color_source(drills=drills, merged=merged)
        return cls(
            schema=str(schema or merged.get("schema") or default.schema),
            name=_coerce_optional_str(merged.get("name")),
            output_step=_coerce_optional_str(merged.get("output_step")),
            pcbdoc=_coerce_optional_str(merged.get("pcbdoc")),
            layer=_coerce_str(merged.get("layer"), default.layer),
            thickness_mm=_coerce_float(merged.get("thickness_mm"), default.thickness_mm),
            z_mm=_coerce_float(merged.get("z_mm"), default.z_mm),
            copper_color=_coerce_color(
                feature_defaults.get("color", merged.get("copper_color")),
                default.copper_color,
            ),
            outline_width_mm=_coerce_float(
                board_outline.get("width_mm", merged.get("outline_width_mm")),
                default.outline_width_mm,
            ),
            outline_color=_coerce_color(
                board_outline.get("color", merged.get("outline_color")),
                default.outline_color,
            ),
            board_cutout_color=_coerce_color(
                board_outline.get(
                    "cutout_color",
                    board_outline.get("cutouts_color", merged.get("board_cutout_color")),
                ),
                default.board_cutout_color,
            ),
            include_board_cutouts=_coerce_bool(
                board_outline.get("cutouts", merged.get("include_board_cutouts")),
                default.include_board_cutouts,
            ),
            include_copper=_coerce_bool(merged.get("include_copper"), default.include_copper),
            include_board_outline=_coerce_bool(
                merged.get("include_board_outline"),
                default.include_board_outline,
            ),
            include_poured_polygons=_feature_enabled(
                features=features,
                merged=merged,
                name="polygons",
                aliases=("poured_polygons",),
                legacy_key="include_poured_polygons",
                default=default.include_poured_polygons,
            ),
            cut_holes=cut_holes,
            drill_hole_mode=_coerce_drill_hole_mode(
                drills.get("mode", merged.get("drill_hole_mode")),
                cut_holes=cut_holes,
            ),
            max_boolean_drill_cuts=int(
                _coerce_float(
                    merged.get("max_boolean_drill_cuts"),
                    default.max_boolean_drill_cuts,
                )
            ),
            drill_hole_color=_coerce_color(drill_color, default.drill_hole_color),
            drill_plated_hole_color=_coerce_color(
                _drill_plated_color_source(
                    drills=drills,
                    merged=merged,
                    drill_color=drill_color,
                ),
                default.drill_plated_hole_color,
            ),
            drill_non_plated_hole_color=_coerce_color(
                _drill_non_plated_color_source(
                    drills=drills,
                    merged=merged,
                    drill_color=drill_color,
                ),
                default.drill_non_plated_hole_color,
            ),
            drill_overlay_thickness_mm=_coerce_float(
                drills.get("overlay_thickness_mm", merged.get("drill_overlay_thickness_mm")),
                default.drill_overlay_thickness_mm,
            ),
            drill_minimum_diameter_mm=_coerce_float(
                drills.get("minimum_diameter_mm", merged.get("drill_minimum_diameter_mm")),
                default.drill_minimum_diameter_mm,
            ),
            drill_hole_shape=_coerce_drill_hole_shape(
                drills.get("shape", merged.get("drill_hole_shape")),
                default.drill_hole_shape,
            ),
            drill_ring_width_mm=_coerce_float(
                drills.get("ring_width_mm", merged.get("drill_ring_width_mm")),
                default.drill_ring_width_mm,
            ),
            drill_plated_ring_shape=_coerce_drill_plated_ring_shape(
                drills.get("plated_ring_shape", merged.get("drill_plated_ring_shape")),
                default.drill_plated_ring_shape,
            ),
            drill_selected_component_mode=_coerce_drill_scope_mode(
                drills.get(
                    "selected_component_mode",
                    merged.get("drill_selected_component_mode"),
                ),
                default.drill_selected_component_mode,
            ),
            drill_other_component_mode=_coerce_drill_scope_mode(
                drills.get(
                    "other_component_mode",
                    merged.get("drill_other_component_mode"),
                ),
                default.drill_other_component_mode,
            ),
            drill_free_pad_mode=_coerce_drill_scope_mode(
                drills.get("free_pad_mode", merged.get("drill_free_pad_mode")),
                default.drill_free_pad_mode,
            ),
            drill_via_mode=_coerce_drill_scope_mode(
                drills.get("via_mode", merged.get("drill_via_mode")),
                default.drill_via_mode,
            ),
            fuse_copper=_coerce_bool(merged.get("fuse_copper"), default.fuse_copper),
            fuse_board_outline=_coerce_bool(
                board_outline.get("fuse", merged.get("fuse_board_outline")),
                default.fuse_board_outline,
            ),
            arc_segments=int(_coerce_float(merged.get("arc_segments"), default.arc_segments)),
            include_tracks=_feature_enabled(
                features=features,
                merged=merged,
                name="tracks",
                aliases=("traces",),
                legacy_key="include_tracks",
                default=default.include_tracks,
            ),
            include_arcs=_feature_enabled(
                features=features,
                merged=merged,
                name="arcs",
                legacy_key="include_arcs",
                default=default.include_arcs,
            ),
            include_fills=_feature_enabled(
                features=features,
                merged=merged,
                name="fills",
                legacy_key="include_fills",
                default=default.include_fills,
            ),
            include_regions=_feature_enabled(
                features=features,
                merged=merged,
                name="regions",
                aliases=("shapebased_regions",),
                legacy_key="include_regions",
                default=default.include_regions,
            ),
            include_vias=_feature_enabled(
                features=features,
                merged=merged,
                name="vias",
                legacy_key="include_vias",
                default=default.include_vias,
            ),
            include_component_pads=include_component_pads,
            include_free_pads=_feature_enabled(
                features=features,
                merged=merged,
                name="free_pads",
                legacy_key="include_free_pads",
                default=default.include_free_pads,
            ),
            include_designators=_coerce_str_tuple(component_pad_designators),
            pad_color_rules=_component_pad_highlight_rules(features),
            track_color=track_color,
            track_body=track_body,
            arc_color=arc_color,
            arc_body=arc_body,
            fill_color=fill_color,
            fill_body=fill_body,
            polygon_color=polygon_color,
            polygon_body=polygon_body,
            region_color=region_color,
            region_body=region_body,
            via_color=via_color,
            via_body=via_body,
            component_pad_color=component_pad_color,
            component_pad_body=component_pad_body,
            free_pad_color=free_pad_color,
            free_pad_body=free_pad_body,
            track_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="tracks",
                aliases=("traces",),
                legacy_key="track_thickness_bias_mm",
                default=default.track_thickness_bias_mm,
            ),
            arc_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="arcs",
                legacy_key="arc_thickness_bias_mm",
                default=default.arc_thickness_bias_mm,
            ),
            fill_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="fills",
                legacy_key="fill_thickness_bias_mm",
                default=default.fill_thickness_bias_mm,
            ),
            polygon_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="polygons",
                aliases=("poured_polygons",),
                legacy_key="polygon_thickness_bias_mm",
                default=default.polygon_thickness_bias_mm,
            ),
            region_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="regions",
                aliases=("shapebased_regions",),
                legacy_key="region_thickness_bias_mm",
                default=default.region_thickness_bias_mm,
            ),
            via_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="vias",
                legacy_key="via_thickness_bias_mm",
                default=default.via_thickness_bias_mm,
            ),
            component_pad_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="component_pads",
                aliases=("pads",),
                legacy_key="component_pad_thickness_bias_mm",
                default=default.component_pad_thickness_bias_mm,
            ),
            free_pad_thickness_bias_mm=_feature_thickness_bias(
                features=features,
                merged=merged,
                name="free_pads",
                legacy_key="free_pad_thickness_bias_mm",
                default=default.free_pad_thickness_bias_mm,
            ),
        )

    def to_options(self) -> PcbLayerStepOptions:
        return PcbLayerStepOptions(
            layer=resolve_pcb_layer_selector(self.layer),
            thickness_mm=self.thickness_mm,
            z_mm=self.z_mm,
            copper_color=self.copper_color,
            outline_width_mm=self.outline_width_mm,
            outline_color=self.outline_color,
            board_cutout_color=self.board_cutout_color,
            include_copper=self.include_copper,
            include_board_outline=self.include_board_outline,
            include_board_cutouts=self.include_board_cutouts,
            include_poured_polygons=self.include_poured_polygons,
            cut_holes=self.cut_holes,
            drill_hole_mode=self.drill_hole_mode,
            max_boolean_drill_cuts=self.max_boolean_drill_cuts,
            drill_hole_color=self.drill_hole_color,
            drill_plated_hole_color=self.drill_plated_hole_color,
            drill_non_plated_hole_color=self.drill_non_plated_hole_color,
            drill_overlay_thickness_mm=self.drill_overlay_thickness_mm,
            drill_minimum_diameter_mm=self.drill_minimum_diameter_mm,
            drill_hole_shape=self.drill_hole_shape,
            drill_ring_width_mm=self.drill_ring_width_mm,
            drill_plated_ring_shape=self.drill_plated_ring_shape,
            drill_selected_component_mode=self.drill_selected_component_mode,
            drill_other_component_mode=self.drill_other_component_mode,
            drill_free_pad_mode=self.drill_free_pad_mode,
            drill_via_mode=self.drill_via_mode,
            fuse_copper=self.fuse_copper,
            fuse_board_outline=self.fuse_board_outline,
            arc_segments=self.arc_segments,
            include_tracks=self.include_tracks,
            include_arcs=self.include_arcs,
            include_fills=self.include_fills,
            include_regions=self.include_regions,
            include_vias=self.include_vias,
            include_component_pads=self.include_component_pads,
            include_free_pads=self.include_free_pads,
            include_designators=self.include_designators,
            pad_color_rules=self.pad_color_rules,
            track_color=self.track_color,
            track_body=self.track_body,
            arc_color=self.arc_color,
            arc_body=self.arc_body,
            fill_color=self.fill_color,
            fill_body=self.fill_body,
            polygon_color=self.polygon_color,
            polygon_body=self.polygon_body,
            region_color=self.region_color,
            region_body=self.region_body,
            via_color=self.via_color,
            via_body=self.via_body,
            component_pad_color=self.component_pad_color,
            component_pad_body=self.component_pad_body,
            free_pad_color=self.free_pad_color,
            free_pad_body=self.free_pad_body,
            track_thickness_bias_mm=self.track_thickness_bias_mm,
            arc_thickness_bias_mm=self.arc_thickness_bias_mm,
            fill_thickness_bias_mm=self.fill_thickness_bias_mm,
            polygon_thickness_bias_mm=self.polygon_thickness_bias_mm,
            region_thickness_bias_mm=self.region_thickness_bias_mm,
            via_thickness_bias_mm=self.via_thickness_bias_mm,
            component_pad_thickness_bias_mm=self.component_pad_thickness_bias_mm,
            free_pad_thickness_bias_mm=self.free_pad_thickness_bias_mm,
        )


@dataclass(slots=True)
class _Segment:
    kind: str = "line"
    center: tuple[float, float] | None = None
    sweep: str | None = None

    def to_json(self) -> dict[str, object]:
        data: dict[str, object] = {"kind": self.kind}
        if self.center is not None:
            data["center"] = [self.center[0], self.center[1]]
        if self.sweep is not None:
            data["sweep"] = self.sweep
        return data


@dataclass(slots=True)
class _Ring:
    points: list[tuple[float, float]]
    segments: list[_Segment] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.points = _dedupe_closed_points(self.points)
        if not self.segments:
            self.segments = [_Segment() for _ in self.points]
        if len(self.segments) != len(self.points):
            raise ValueError("ring segments must match ring points")

    def to_json(self) -> dict[str, object]:
        return {
            "points": [[x, y] for x, y in self.points],
            "segments": [segment.to_json() for segment in self.segments],
        }


@dataclass(slots=True)
class _Region:
    outer: _Ring
    holes: list[_Ring] = field(default_factory=list)

    def to_json(self) -> dict[str, object]:
        data: dict[str, object] = {"outer": self.outer.to_json()}
        if self.holes:
            data["holes"] = [hole.to_json() for hole in self.holes]
        return data


@dataclass(frozen=True, slots=True)
class _SourceFeature:
    kind: str
    region: _Region
    component_designator: str | None = None
    pad_designator: str | None = None


@dataclass(frozen=True, slots=True)
class _DrillFeature:
    region: _Region
    center: tuple[float, float]
    diameter_mm: float
    slot_length_mm: float | None = None
    rotation_degrees: float = 0.0
    plated: bool = True
    pad_region: _Region | None = None
    source_kind: str = "pad"
    component_designator: str | None = None
    pad_designator: str | None = None


def write_default_pcb_layer_step_config(config_path: Path) -> None:
    """Write a default editable pcb-layer-step JSON config."""
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(pcb_layer_step_default_config_text(), encoding="utf-8")


def load_pcb_layer_step_config(config_path: Path) -> PcbLayerStepConfig:
    """Load a pcb-layer-step JSON or JSONC config."""
    try:
        raw_data = load_json_config(config_path)
    except Exception as exc:
        raise ValueError(f"Failed to parse pcb-layer-step config '{config_path}': {exc}") from exc
    return PcbLayerStepConfig.from_dict(raw_data)


def export_pcb_layer_step(
    pcb: object,
    output_path: Path,
    *,
    options: PcbLayerStepOptions | None = None,
    board_name: str | None = None,
    source_input: str | None = None,
) -> PcbLayerStepResult:
    """Export a selected PCB layer as a colored STEP alignment model."""
    opts = options or PcbLayerStepOptions()
    _validate_options(opts)
    geometer = _load_geometer()
    output_path = Path(output_path).resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    layer = resolve_pcb_layer_selector(opts.layer)
    resolved_board_name = board_name or _board_name_from_pcb(pcb)

    log.info("Collecting %s layer geometry for %s", layer, resolved_board_name)
    features = _collect_layer_features(pcb, layer, opts)
    drill_features = _collect_drill_features(pcb, layer, opts)
    drill_hole_mode = _effective_drill_hole_mode(opts, len(drill_features))
    log.info(
        "Collected features: layer=%d drill=%d mode=%s",
        len(features),
        len(drill_features),
        drill_hole_mode,
    )
    bodies, counts = _build_step_bodies(
        pcb=pcb,
        opts=replace(opts, layer=layer),
        features=features,
        drill_features=drill_features,
        drill_hole_mode=drill_hole_mode,
    )
    if not bodies:
        raise ValueError(f"No geometry found for layer {layer}")

    origin_mm = _board_origin_mm(pcb)
    _apply_origin_relative_geometry(bodies, origin_mm)
    request = {
        "schema": "geometry.planar_step.request.a0",
        "units": "mm",
        "name": _step_name(resolved_board_name),
        "bodies": bodies,
    }
    log.info("Writing STEP with %d bodies: %s", len(bodies), output_path.name)
    geometer.write_planar_step(request, output_path)

    manifest_path = output_path.with_suffix(".json")
    manifest = _build_manifest(
        pcb=pcb,
        opts=replace(opts, layer=layer),
        output_path=output_path,
        board_name=resolved_board_name,
        source_input=source_input,
        drill_hole_mode=drill_hole_mode,
        counts=counts,
        coordinate_origin=_coordinate_origin_payload(origin_mm),
    )
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    return PcbLayerStepResult(
        output_path=output_path,
        manifest_path=manifest_path,
        board_name=resolved_board_name,
        layer=layer,
        copper_body_count=counts["copper_bodies"],
        outline_body_count=counts["outline_bodies"],
        drill_cut_count=counts["drill_cut_geometries"],
        source_input=source_input,
    )


def _validate_options(opts: PcbLayerStepOptions) -> None:
    if opts.thickness_mm <= 0.0:
        raise ValueError("STEP layer thickness must be positive")
    if opts.outline_width_mm < 0.0:
        raise ValueError("Board outline width must be non-negative")
    if opts.drill_ring_width_mm < 0.0:
        raise ValueError("Drill ring width must be non-negative")
    if opts.drill_plated_ring_shape not in DRILL_PLATED_RING_SHAPES:
        raise ValueError("Drill plated ring shape must be 'annulus' or 'pad'")
    scoped_modes = {
        "drill_selected_component_mode": opts.drill_selected_component_mode,
        "drill_other_component_mode": opts.drill_other_component_mode,
        "drill_free_pad_mode": opts.drill_free_pad_mode,
        "drill_via_mode": opts.drill_via_mode,
    }
    invalid_scoped_modes = [
        f"{name}={value!r}"
        for name, value in scoped_modes.items()
        if value not in DRILL_SCOPE_MODES
    ]
    if invalid_scoped_modes:
        raise ValueError("Invalid scoped drill mode(s): " + ", ".join(invalid_scoped_modes))


def _load_geometer() -> _GeometerPlanarStepModule:
    try:
        import geometer
    except Exception as exc:
        raise RuntimeError(
            "PCB layer STEP export requires wn-geometer planar_step support"
        ) from exc
    if not hasattr(geometer, "write_planar_step"):
        raise RuntimeError("PCB layer STEP export requires wn-geometer write_planar_step support")
    return cast(_GeometerPlanarStepModule, geometer)


def _build_manifest(
    *,
    pcb: object,
    opts: PcbLayerStepOptions,
    output_path: Path,
    board_name: str,
    source_input: str | None,
    drill_hole_mode: str,
    counts: dict[str, int],
    coordinate_origin: dict[str, object],
) -> dict[str, object]:
    return {
        "schema": "wn.kicad_cruncher.pcb_layer_step.v1",
        "backend": "geometer.planar_step",
        "board": board_name,
        "source_input": source_input,
        "step_file": output_path.name,
        "coordinate_origin": coordinate_origin,
        "layer": {
            "id": _pcb_layer_ordinal(pcb, opts.layer),
            "json_name": opts.layer,
            "display_name": opts.layer,
        },
        "options": {
            "thickness_mm": float(opts.thickness_mm),
            "z_mm": float(opts.z_mm),
            "copper_color": opts.copper_color,
            "outline_width_mm": float(opts.outline_width_mm),
            "outline_color": opts.outline_color,
            "board_cutout_color": opts.board_cutout_color,
            "include_copper": bool(opts.include_copper),
            "include_board_outline": bool(opts.include_board_outline),
            "include_board_cutouts": bool(opts.include_board_cutouts),
            "include_poured_polygons": bool(opts.include_poured_polygons),
            "cut_holes": bool(opts.cut_holes),
            "drill_hole_mode": opts.drill_hole_mode,
            "effective_drill_hole_mode": drill_hole_mode,
            "max_boolean_drill_cuts": int(opts.max_boolean_drill_cuts),
            "drill_hole_color": opts.drill_hole_color,
            "drill_plated_hole_color": opts.drill_plated_hole_color,
            "drill_non_plated_hole_color": opts.drill_non_plated_hole_color,
            "drill_overlay_thickness_mm": float(opts.drill_overlay_thickness_mm),
            "drill_minimum_diameter_mm": float(opts.drill_minimum_diameter_mm),
            "drill_hole_shape": opts.drill_hole_shape,
            "drill_ring_width_mm": float(opts.drill_ring_width_mm),
            "drill_plated_ring_shape": opts.drill_plated_ring_shape,
            "drill_selected_component_mode": opts.drill_selected_component_mode,
            "drill_other_component_mode": opts.drill_other_component_mode,
            "drill_free_pad_mode": opts.drill_free_pad_mode,
            "drill_via_mode": opts.drill_via_mode,
            "fuse_copper": bool(opts.fuse_copper),
            "fuse_board_outline": bool(opts.fuse_board_outline),
            "arc_segments": int(opts.arc_segments),
            "features": {
                "tracks": bool(opts.include_tracks),
                "arcs": bool(opts.include_arcs),
                "fills": bool(opts.include_fills),
                "polygons": bool(opts.include_poured_polygons),
                "regions": bool(opts.include_regions),
                "vias": bool(opts.include_vias),
                "component_pads": bool(opts.include_component_pads),
                "free_pads": bool(opts.include_free_pads),
                "include_designators": list(opts.include_designators),
            },
            "pad_color_rules": [
                {
                    "designators": list(rule.designators),
                    "color": rule.color,
                    "step_body_name": rule.step_body_name,
                }
                for rule in opts.pad_color_rules
            ],
            "feature_color_rules": {
                "tracks": {"color": opts.track_color, "step_body_name": opts.track_body},
                "arcs": {"color": opts.arc_color, "step_body_name": opts.arc_body},
                "fills": {"color": opts.fill_color, "step_body_name": opts.fill_body},
                "polygons": {"color": opts.polygon_color, "step_body_name": opts.polygon_body},
                "regions": {"color": opts.region_color, "step_body_name": opts.region_body},
                "vias": {"color": opts.via_color, "step_body_name": opts.via_body},
                "component_pads": {
                    "color": opts.component_pad_color,
                    "step_body_name": opts.component_pad_body,
                },
                "free_pads": {"color": opts.free_pad_color, "step_body_name": opts.free_pad_body},
            },
            "thickness_bias_mm": {
                "tracks": float(opts.track_thickness_bias_mm),
                "arcs": float(opts.arc_thickness_bias_mm),
                "fills": float(opts.fill_thickness_bias_mm),
                "polygons": float(opts.polygon_thickness_bias_mm),
                "regions": float(opts.region_thickness_bias_mm),
                "vias": float(opts.via_thickness_bias_mm),
                "component_pads": float(opts.component_pad_thickness_bias_mm),
                "free_pads": float(opts.free_pad_thickness_bias_mm),
            },
        },
        "counts": counts,
        "bytes": output_path.stat().st_size,
    }


def _build_step_bodies(
    *,
    pcb: object,
    opts: PcbLayerStepOptions,
    features: list[_SourceFeature],
    drill_features: list[_DrillFeature],
    drill_hole_mode: str,
) -> tuple[list[dict[str, object]], dict[str, int]]:
    board_cutouts = _collect_board_cutout_regions(pcb)
    drill_mode_by_feature = _drill_modes_for_features(
        drill_features,
        opts,
        drill_hole_mode,
    )
    boolean_drill_cutouts = [
        feature.region for feature, mode in drill_mode_by_feature if mode == DRILL_HOLE_MODE_CUT
    ]
    drill_copper_cutouts = [
        feature.region
        for feature, mode in drill_mode_by_feature
        if mode in {DRILL_HOLE_MODE_CUT, DRILL_HOLE_MODE_OVERLAY}
    ]
    overlay_drill_features = [
        feature for feature, mode in drill_mode_by_feature if mode == DRILL_HOLE_MODE_OVERLAY
    ]
    pad_clip_regions = _pad_clip_regions(features)
    shared_cutouts = [*drill_copper_cutouts, *board_cutouts]
    bodies = [
        *_copper_bodies_from_features(features, opts, shared_cutouts, pad_clip_regions),
        *_drill_overlay_bodies(overlay_drill_features, DRILL_HOLE_MODE_OVERLAY, opts),
        *_outline_bodies(pcb, opts),
    ]
    counts = _build_counts(
        features=features,
        drill_features=drill_features,
        overlay_drill_features=overlay_drill_features,
        boolean_drill_cutouts=boolean_drill_cutouts,
        drill_copper_cutouts=drill_copper_cutouts,
        board_cutouts=board_cutouts,
        bodies=bodies,
    )
    return bodies, counts


def _copper_bodies_from_features(
    features: list[_SourceFeature],
    opts: PcbLayerStepOptions,
    cutouts: list[_Region],
    pad_clip_regions: list[_Region],
) -> list[dict[str, object]]:
    if not opts.include_copper:
        return []
    grouped: dict[tuple[str, str, float, bool], list[_SourceFeature]] = {}
    for feature in features:
        body_id, color, thickness_bias_mm = _body_style_for_feature(feature, opts)
        clip_to_pads = _feature_clips_to_pad_shapes(feature)
        grouped.setdefault((body_id, color, thickness_bias_mm, clip_to_pads), []).append(feature)
    return [
        _body_from_regions(
            body_id=body_id,
            color=color,
            regions=[feature.region for feature in body_features],
            z_mm=_biased_z_mm(opts.z_mm, thickness_bias_mm),
            thickness_mm=_biased_thickness_mm(opts.thickness_mm, thickness_bias_mm),
            fuse_regions=opts.fuse_copper,
            cutouts=[
                *_copper_body_cutouts(features, body_features, opts, cutouts),
                *(pad_clip_regions if clip_to_pads else []),
            ],
        )
        for (body_id, color, thickness_bias_mm, clip_to_pads), body_features in grouped.items()
        if body_features
    ]


def _copper_body_cutouts(
    all_features: list[_SourceFeature],
    body_features: list[_SourceFeature],
    opts: PcbLayerStepOptions,
    shared_cutouts: list[_Region],
) -> list[_Region]:
    del all_features, body_features, opts
    return shared_cutouts


def _is_trace_only_body(features: list[_SourceFeature]) -> bool:
    return bool(features) and all(feature.kind in {"track", "arc"} for feature in features)


def _body_style_for_feature(
    feature: _SourceFeature, opts: PcbLayerStepOptions
) -> tuple[str, str, float]:
    thickness_bias_mm = _thickness_bias_for_feature(feature, opts)
    body_id, color = _configured_body_style(feature, opts)
    body_id, color = _apply_pad_color_rules(feature, opts, body_id, color)
    if body_id == "copper" and thickness_bias_mm > 0.0:
        body_id = f"copper_{_feature_body_suffix(feature.kind)}"
    return body_id, color, thickness_bias_mm


def _configured_body_style(
    feature: _SourceFeature,
    opts: PcbLayerStepOptions,
) -> tuple[str, str]:
    style = _feature_style_options(opts).get(feature.kind)
    if style is None:
        return "copper", opts.copper_color
    body_template, default_body_id, color = style
    if color is None and body_template == default_body_id:
        return "copper", opts.copper_color
    return (
        _format_step_body_name(body_template, feature, default_body_id),
        color or opts.copper_color,
    )


def _feature_style_options(
    opts: PcbLayerStepOptions,
) -> dict[str, tuple[str, str, str | None]]:
    region_style = (opts.region_body, "regions", opts.region_color)
    return {
        "track": (opts.track_body, "tracks", opts.track_color),
        "arc": (opts.arc_body, "arcs", opts.arc_color),
        "fill": (opts.fill_body, "fills", opts.fill_color),
        "polygon": (opts.polygon_body, "polygons", opts.polygon_color),
        "region": region_style,
        "via": (opts.via_body, "vias", opts.via_color),
        "component_pad": (
            opts.component_pad_body,
            "component_pads",
            opts.component_pad_color,
        ),
        "free_pad": (opts.free_pad_body, "free_pads", opts.free_pad_color),
    }


def _apply_pad_color_rules(
    feature: _SourceFeature,
    opts: PcbLayerStepOptions,
    body_id: str,
    color: str,
) -> tuple[str, str]:
    if feature.kind not in {"component_pad", "free_pad"}:
        return body_id, color
    designator = feature.component_designator or feature.pad_designator or ""
    for rule in opts.pad_color_rules:
        if _matches_any_pattern(designator, rule.designators):
            return (
                _format_step_body_name(rule.step_body_name, feature, "matched_pads"),
                rule.color,
            )
    return body_id, color


def _format_step_body_name(
    template: str,
    feature: _SourceFeature,
    fallback: str,
) -> str:
    values = {
        "component": feature.component_designator or "",
        "pad": feature.pad_designator or "",
        "feature": _feature_body_suffix(feature.kind),
    }
    text = str(template)
    for key, value in values.items():
        text = text.replace(f"{{{key}}}", value)
    name = _step_name(text)
    return name if name != "board" or text.strip() else _step_name(fallback)


def _biased_z_mm(z_mm: float, thickness_bias_mm: float) -> float:
    return z_mm - max(0.0, thickness_bias_mm)


def _biased_thickness_mm(thickness_mm: float, thickness_bias_mm: float) -> float:
    bias = max(0.0, thickness_bias_mm)
    return thickness_mm + (2.0 * bias)


def _feature_body_suffix(kind: str) -> str:
    return {
        "track": "tracks",
        "arc": "arcs",
        "fill": "fills",
        "polygon": "polygons",
        "region": "regions",
        "via": "vias",
        "component_pad": "pads",
        "free_pad": "pads",
    }.get(kind, kind)


def _pad_clip_regions(features: list[_SourceFeature]) -> list[_Region]:
    return [
        feature.region
        for feature in features
        if feature.kind in {"component_pad", "free_pad", "via"}
    ]


def _feature_clips_to_pad_shapes(feature: _SourceFeature) -> bool:
    return feature.kind in {"track", "arc", "fill", "polygon", "region"}


def _thickness_bias_for_feature(
    feature: _SourceFeature,
    opts: PcbLayerStepOptions,
) -> float:
    return {
        "track": opts.track_thickness_bias_mm,
        "arc": opts.arc_thickness_bias_mm,
        "fill": opts.fill_thickness_bias_mm,
        "polygon": opts.polygon_thickness_bias_mm,
        "region": opts.region_thickness_bias_mm,
        "via": opts.via_thickness_bias_mm,
        "component_pad": opts.component_pad_thickness_bias_mm,
        "free_pad": opts.free_pad_thickness_bias_mm,
    }.get(feature.kind, 0.0)


def _drill_overlay_bodies(
    drill_features: list[_DrillFeature],
    drill_hole_mode: str,
    opts: PcbLayerStepOptions,
) -> list[dict[str, object]]:
    if drill_hole_mode != DRILL_HOLE_MODE_OVERLAY or not drill_features:
        return []
    grouped: dict[tuple[str, str], list[_DrillFeature]] = {}
    for feature in drill_features:
        body_id, color = _drill_body_style(feature, opts)
        grouped.setdefault((body_id, color), []).append(feature)
    return [
        _body_from_regions(
            body_id=body_id,
            color=color,
            regions=[_drill_overlay_region(feature, opts) for feature in features],
            z_mm=opts.z_mm + opts.thickness_mm,
            thickness_mm=max(0.0001, opts.drill_overlay_thickness_mm),
            fuse_regions=False,
            cutouts=[],
        )
        for (body_id, color), features in grouped.items()
    ]


def _drill_body_style(feature: _DrillFeature, opts: PcbLayerStepOptions) -> tuple[str, str]:
    if _drill_overlay_uses_single_color(opts):
        return "drill_holes", opts.drill_hole_color
    if feature.plated:
        return "plated_drill_holes", opts.drill_plated_hole_color
    return "non_plated_drill_holes", opts.drill_non_plated_hole_color


def _drill_overlay_uses_single_color(opts: PcbLayerStepOptions) -> bool:
    return (
        opts.drill_plated_hole_color == opts.drill_hole_color
        and opts.drill_non_plated_hole_color == opts.drill_hole_color
    )


def _outline_bodies(pcb: object, opts: PcbLayerStepOptions) -> list[dict[str, object]]:
    if not opts.include_board_outline or opts.outline_width_mm <= 0.0:
        return []
    bodies: list[dict[str, object]] = []
    outline_regions = _collect_board_outline_regions(pcb, opts)
    if outline_regions:
        bodies.append(
            _body_from_regions(
                body_id="board_outline",
                color=opts.outline_color,
                regions=outline_regions,
                z_mm=opts.z_mm,
                thickness_mm=opts.thickness_mm,
                fuse_regions=opts.fuse_board_outline,
                cutouts=[],
            )
        )
    cutout_regions = _collect_board_cutout_outline_regions(pcb, opts)
    if opts.include_board_cutouts and cutout_regions:
        bodies.append(
            _body_from_regions(
                body_id="board_cutouts",
                color=opts.board_cutout_color,
                regions=cutout_regions,
                z_mm=opts.z_mm,
                thickness_mm=opts.thickness_mm,
                fuse_regions=opts.fuse_board_outline,
                cutouts=[],
            )
        )
    return bodies


def _body_from_regions(
    *,
    body_id: str,
    color: str,
    regions: list[_Region],
    z_mm: float,
    thickness_mm: float,
    fuse_regions: bool,
    cutouts: list[_Region],
) -> dict[str, object]:
    body: dict[str, object] = {
        "id": body_id,
        "name": body_id,
        "color": color,
        "z_mm": z_mm,
        "thickness_mm": thickness_mm,
        "regions": [region.to_json() for region in regions],
    }
    if fuse_regions:
        body["fuse_regions"] = True
    if cutouts:
        body["cutouts"] = [cutout.to_json() for cutout in cutouts]
    return body


def _build_counts(
    *,
    features: list[_SourceFeature],
    drill_features: list[_DrillFeature],
    overlay_drill_features: list[_DrillFeature],
    boolean_drill_cutouts: list[_Region],
    drill_copper_cutouts: list[_Region],
    board_cutouts: list[_Region],
    bodies: list[dict[str, object]],
) -> dict[str, int]:
    drill_overlay_count, plated_overlay_count, non_plated_overlay_count = (
        _drill_overlay_counts(overlay_drill_features)
    )
    return {
        "source_layer_geometries": len(features),
        "drill_cut_geometries": len(drill_features),
        "drill_boolean_cut_geometries": len(boolean_drill_cutouts),
        "drill_copper_cutout_geometries": len(drill_copper_cutouts),
        "drill_overlay_geometries": drill_overlay_count,
        "drill_plated_overlay_geometries": plated_overlay_count,
        "drill_non_plated_overlay_geometries": non_plated_overlay_count,
        "board_cutout_geometries": len(board_cutouts),
        "board_cutout_outline_geometries": sum(
            _body_region_count(body) for body in bodies if str(body.get("id")) == "board_cutouts"
        ),
        "copper_bodies": sum(1 for body in bodies if _is_step_copper_body(body)),
        "outline_bodies": sum(1 for body in bodies if str(body.get("id")) == "board_outline"),
        "board_cutout_outline_bodies": sum(
            1 for body in bodies if str(body.get("id")) == "board_cutouts"
        ),
        "body_count": len(bodies),
    }


def _drill_overlay_counts(
    drill_features: list[_DrillFeature],
) -> tuple[int, int, int]:
    plated_count = sum(1 for feature in drill_features if feature.plated)
    return (len(drill_features), plated_count, len(drill_features) - plated_count)


def _is_step_copper_body(body: Mapping[str, object]) -> bool:
    return str(body.get("id")) not in _NON_COPPER_BODY_IDS


def _body_region_count(body: Mapping[str, object]) -> int:
    regions = body.get("regions")
    return len(regions) if isinstance(regions, list) else 0


def _effective_drill_hole_mode(opts: PcbLayerStepOptions, drill_count: int) -> str:
    if not opts.cut_holes:
        return DRILL_HOLE_MODE_NONE
    requested = _coerce_drill_hole_mode(opts.drill_hole_mode, cut_holes=True)
    if requested != DRILL_HOLE_MODE_AUTO:
        return requested
    if drill_count <= max(0, int(opts.max_boolean_drill_cuts)):
        return DRILL_HOLE_MODE_CUT
    log.info(
        "Using drill overlay instead of boolean drill cuts for %d holes (threshold: %d)",
        drill_count,
        int(opts.max_boolean_drill_cuts),
    )
    return DRILL_HOLE_MODE_OVERLAY


def _drill_modes_for_features(
    drill_features: list[_DrillFeature],
    opts: PcbLayerStepOptions,
    global_mode: str,
) -> list[tuple[_DrillFeature, str]]:
    if global_mode == DRILL_HOLE_MODE_NONE:
        return [(feature, DRILL_HOLE_MODE_NONE) for feature in drill_features]
    return [
        (feature, _effective_drill_feature_mode(feature, opts, global_mode))
        for feature in drill_features
    ]


def _effective_drill_feature_mode(
    feature: _DrillFeature,
    opts: PcbLayerStepOptions,
    global_mode: str,
) -> str:
    scoped_mode = _configured_drill_scope_mode(feature, opts)
    return global_mode if scoped_mode == DRILL_SCOPE_MODE_INHERIT else scoped_mode


def _configured_drill_scope_mode(
    feature: _DrillFeature,
    opts: PcbLayerStepOptions,
) -> str:
    if feature.source_kind == "via":
        return opts.drill_via_mode
    if feature.component_designator:
        if _include_pad_feature(feature.component_designator, opts):
            return opts.drill_selected_component_mode
        return opts.drill_other_component_mode
    return opts.drill_free_pad_mode


def _collect_layer_features(
    pcb: object, layer: str, opts: PcbLayerStepOptions
) -> list[_SourceFeature]:
    features: list[_SourceFeature] = []
    if opts.include_tracks or opts.include_poured_polygons:
        features.extend(_track_features(pcb, layer, opts))
    if opts.include_arcs or opts.include_poured_polygons:
        features.extend(_arc_features(pcb, layer, opts))
    if _is_copper_layer(layer):
        if opts.include_component_pads:
            features.extend(_pad_features(pcb, layer, opts))
        if opts.include_vias:
            features.extend(_via_features(pcb, layer))
    if opts.include_poured_polygons:
        features.extend(_zone_features(pcb, layer))
    if opts.include_regions:
        features.extend(_graphic_region_features(pcb, layer))
    return features


def _track_features(pcb: object, layer: str, opts: PcbLayerStepOptions) -> list[_SourceFeature]:
    if not opts.include_tracks:
        return []
    return [
        _SourceFeature("track", region)
        for segment in getattr(pcb, "segments", []) or []
        if str(getattr(segment, "layer", "")) == layer
        for region in _regions_from_polygon_set(segment._to_poly(_arc_error(opts)))
    ]


def _arc_features(pcb: object, layer: str, opts: PcbLayerStepOptions) -> list[_SourceFeature]:
    if not opts.include_arcs:
        return []
    return [
        _SourceFeature("arc", region)
        for arc in getattr(pcb, "arcs", []) or []
        if str(getattr(arc, "layer", "")) == layer
        for region in _regions_from_polygon_set(arc._to_poly(_arc_error(opts)))
    ]


def _pad_features(pcb: object, layer: str, opts: PcbLayerStepOptions) -> list[_SourceFeature]:
    features: list[_SourceFeature] = []
    for footprint in getattr(pcb, "footprints", []) or []:
        designator = _footprint_designator(footprint)
        if not _matches_designator_filter(designator, opts.include_designators):
            continue
        for pad in getattr(footprint, "pads", []) or []:
            if not pad_on_layer(pad, layer):
                continue
            region = _pad_region(footprint, pad, layer, opts)
            if region is None:
                continue
            features.append(
                _SourceFeature(
                    "component_pad",
                    region,
                    component_designator=designator,
                    pad_designator=str(getattr(pad, "number", "") or "").strip() or None,
                )
            )
    return features


def _via_features(pcb: object, layer: str) -> list[_SourceFeature]:
    return [
        _SourceFeature("via", region)
        for via in getattr(pcb, "vias", []) or []
        if _via_spans_layer(via, layer)
        for region in _via_copper_regions(via)
    ]


def _zone_features(pcb: object, layer: str) -> list[_SourceFeature]:
    features: list[_SourceFeature] = []
    for zone in getattr(pcb, "zones", []) or []:
        features.extend(_zone_filled_polygon_features(zone, layer))
        features.extend(_zone_outline_polygon_features(zone, layer))
    return features


def _zone_filled_polygon_features(zone: object, layer: str) -> list[_SourceFeature]:
    if not _zone_is_copper_pour(zone):
        return []
    features: list[_SourceFeature] = []
    for filled in getattr(zone, "filled_polygons", []) or []:
        if str(getattr(filled, "layer", "")) != layer:
            continue
        region = _region_from_points(getattr(filled, "points", []) or [])
        if region is not None:
            features.append(_SourceFeature("polygon", region))
    return features


def _zone_outline_polygon_features(zone: object, layer: str) -> list[_SourceFeature]:
    if not _zone_is_copper_pour(zone):
        return []
    if getattr(zone, "filled_polygons", []):
        return []
    if not bool(getattr(zone, "fill_enabled", False)):
        return []
    if not _layer_in_collection(layer, getattr(zone, "layers", [])):
        return []
    features: list[_SourceFeature] = []
    for polygon in getattr(zone, "polygons", []) or []:
        region = _region_from_points(getattr(polygon, "points", []) or [])
        if region is not None:
            features.append(_SourceFeature("polygon", region))
    return features


def _zone_is_copper_pour(zone: object) -> bool:
    return getattr(zone, "keepout", None) is None and getattr(zone, "placement", None) is None


def _graphic_region_features(pcb: object, layer: str) -> list[_SourceFeature]:
    return [
        *_graphic_poly_features(pcb, layer),
        *_filled_graphic_shape_features(pcb, layer),
    ]


def _graphic_poly_features(pcb: object, layer: str) -> list[_SourceFeature]:
    features: list[_SourceFeature] = []
    for poly in getattr(pcb, "gr_polys", []) or []:
        if str(getattr(poly, "layer", "")) != layer:
            continue
        region = _region_from_points(getattr(poly, "points", []) or [])
        if region is not None:
            features.append(_SourceFeature("region", region))
    return features


def _filled_graphic_shape_features(pcb: object, layer: str) -> list[_SourceFeature]:
    features: list[_SourceFeature] = []
    for item in [
        *(getattr(pcb, "gr_rects", []) or []),
        *(getattr(pcb, "gr_circles", []) or []),
    ]:
        if str(getattr(item, "layer", "")) != layer:
            continue
        if not _graphic_fill_enabled(item):
            continue
        for region in _regions_from_polygon_set(item._to_poly()):
            features.append(_SourceFeature("region", region))
    return features


def _graphic_fill_enabled(item: object) -> bool:
    fill = getattr(item, "fill", None)
    return str(getattr(fill, "value", fill)).casefold() in {"solid", "yes", "true", "1"}


def _collect_drill_features(
    pcb: object,
    layer: str,
    opts: PcbLayerStepOptions,
) -> list[_DrillFeature]:
    if not _is_copper_layer(layer):
        return []
    drills = [
        *_pad_drill_features(pcb, layer, opts),
        *_via_drill_features(pcb, layer, opts),
    ]
    return [
        drill for drill in drills if drill.diameter_mm > max(0.0, opts.drill_minimum_diameter_mm)
    ]


def _pad_drill_features(
    pcb: object,
    layer: str,
    opts: PcbLayerStepOptions,
) -> list[_DrillFeature]:
    features: list[_DrillFeature] = []
    for footprint in getattr(pcb, "footprints", []) or []:
        for pad in getattr(footprint, "pads", []) or []:
            if not pad_on_layer(pad, layer):
                continue
            feature = _pad_hole_feature(footprint, pad, layer, opts)
            if feature is not None:
                features.append(feature)
    return features


def _via_drill_features(
    pcb: object,
    layer: str,
    opts: PcbLayerStepOptions,
) -> list[_DrillFeature]:
    del opts
    features: list[_DrillFeature] = []
    for via in getattr(pcb, "vias", []) or []:
        if not _via_spans_layer(via, layer):
            continue
        feature = _via_hole_feature(via)
        if feature is not None:
            features.append(feature)
    return features


def _collect_board_outline_regions(pcb: object, opts: PcbLayerStepOptions) -> list[_Region]:
    outer = _outer_board_region(_edge_cut_regions(pcb))
    if outer is None:
        return []
    return _outline_stroke_regions_from_points(outer.points, opts.outline_width_mm)


def _collect_board_cutout_outline_regions(pcb: object, opts: PcbLayerStepOptions) -> list[_Region]:
    regions: list[_Region] = []
    for cutout in _interior_board_regions(_edge_cut_regions(pcb)):
        regions.extend(_outline_stroke_regions_from_points(cutout.points, opts.outline_width_mm))
    return regions


def _collect_board_cutout_regions(pcb: object) -> list[_Region]:
    return [
        region
        for cutout in _interior_board_regions(_edge_cut_regions(pcb))
        if (region := _region_from_points(cutout.points)) is not None
    ]


def _edge_cut_regions(pcb: object) -> list[_BoardRegion]:
    styles = {
        "board_outline": {
            "max_arc_segment_mm": 0.25,
            "max_curve_segment_mm": 0.25,
            "max_circle_segment_mm": 0.25,
            "min_arc_segments": 8,
            "min_curve_segments": 12,
            "min_circle_segments": 96,
            "max_arc_segments": 4096,
            "max_curve_segments": 4096,
            "max_circle_segments": 4096,
        }
    }
    return _classify_edge_cut_regions(cast("KiCadPcb", pcb), styles=styles)


def _pad_region(
    footprint: object,
    pad: object,
    layer: str,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    if not pad_on_layer(pad, layer):
        return None
    center = (float(getattr(pad, "at_x", 0.0) or 0.0), float(getattr(pad, "at_y", 0.0) or 0.0))
    size_x = float(getattr(pad, "size_x", 0.0) or 0.0)
    size_y = float(getattr(pad, "size_y", 0.0) or 0.0)
    if size_x <= 0.0 or size_y <= 0.0:
        return None
    region = _local_pad_region(pad, center, size_x, size_y, opts)
    if region is None:
        return None
    region = _apply_footprint_pad_orientation_offset(footprint, center, region)
    return _transform_region_to_board(footprint, region)


def _local_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    handlers = {
        "circle": _circle_pad_region,
        "oval": _oval_pad_region,
        "roundrect": _roundrect_pad_region,
        "trapezoid": _trapezoid_pad_region,
        "custom": _custom_pad_region,
    }
    handler = handlers.get(_pad_shape_name(pad), _rect_pad_region)
    return handler(pad, center, size_x, size_y, opts)


def _circle_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    del pad
    if math.isclose(size_x, size_y, rel_tol=1e-9, abs_tol=1e-9):
        return _circle_region(center, size_x / 2.0)
    return _ellipse_region(center, size_x / 2.0, size_y / 2.0, 0.0, opts.arc_segments)


def _oval_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    del size_x, size_y, opts
    method = getattr(pad, "_to_oval_segment", None)
    if not callable(method):
        return None
    start, end, width = cast(_OvalSegmentMethod, method)(center[0], center[1])
    return _line_capsule_region(start, end, width)


def _roundrect_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    del size_x, size_y
    method = getattr(pad, "_to_roundrect_polygon", None)
    if not callable(method):
        return None
    return _region_from_points(
        cast(_RoundRectPolygonMethod, method)(center[0], center[1], _arc_error(opts))
    )


def _trapezoid_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    del size_x, size_y, opts
    method = getattr(pad, "_to_trapezoid_polygon", None)
    if not callable(method):
        return None
    return _region_from_points(cast(_PadPolygonMethod, method)(center[0], center[1]))


def _rect_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    del size_x, size_y, opts
    method = getattr(pad, "_to_rect_polygon", None)
    if not callable(method):
        return None
    return _region_from_points(cast(_PadPolygonMethod, method)(center[0], center[1]))


def _custom_pad_region(
    pad: object,
    center: tuple[float, float],
    size_x: float,
    size_y: float,
    opts: PcbLayerStepOptions,
) -> _Region | None:
    del opts
    for primitive in getattr(pad, "custom_primitives", []) or []:
        region = _custom_primitive_region(pad, primitive, center)
        if region is not None:
            return region
    return _rectangle_region(center=center, width_mm=size_x, height_mm=size_y)


def _custom_primitive_region(
    pad: object,
    primitive: object,
    center: tuple[float, float],
) -> _Region | None:
    points = list(getattr(primitive, "points", []) or [])
    if getattr(primitive, "primitive_type", "") != "gr_poly" or len(points) < 3:
        return None
    translated = _custom_primitive_points(pad, points, center)
    if bool(getattr(primitive, "is_filled", False)):
        return _region_from_points(translated)
    width = float(getattr(primitive, "width", 0.0) or 0.0)
    if width <= 0.0:
        return None
    strokes = _outline_stroke_regions_from_points(translated, width)
    return strokes[0] if strokes else None


def _custom_primitive_points(
    pad: object,
    points: list[object],
    center: tuple[float, float],
) -> list[tuple[float, float]]:
    angle = -float(getattr(pad, "at_angle", 0.0) or 0.0)
    rotated = [
        rotate_point(x, y, angle)
        for point in points
        if (xy := _point_tuple(point)) is not None
        for x, y in (xy,)
    ]
    return [(x + center[0], y + center[1]) for x, y in rotated]


def _point_tuple(value: object) -> tuple[float, float] | None:
    if isinstance(value, tuple | list) and len(value) >= 2:
        return (float(value[0]), float(value[1]))
    return None


def _pad_hole_feature(
    footprint: object,
    pad: object,
    layer: str,
    opts: PcbLayerStepOptions,
) -> _DrillFeature | None:
    drill = getattr(pad, "drill", None)
    if drill is None:
        return None
    diameter_mm = float(drill or 0.0)
    if diameter_mm <= 0.0:
        return None
    width = float(getattr(pad, "drill_width", None) or diameter_mm)
    height = float(getattr(pad, "drill_height", None) or diameter_mm)
    center_local = _pad_drill_center_local(pad)
    if bool(getattr(pad, "drill_oval", False)) and max(width, height) > min(width, height):
        return _pad_slot_hole_feature(
            footprint,
            pad,
            layer,
            opts,
            center_local=center_local,
            width=width,
            height=height,
        )
    return _pad_round_hole_feature(
        footprint,
        pad,
        layer,
        opts,
        center_local=center_local,
        diameter_mm=diameter_mm,
    )


def _pad_round_hole_feature(
    footprint: object,
    pad: object,
    layer: str,
    opts: PcbLayerStepOptions,
    *,
    center_local: tuple[float, float],
    diameter_mm: float,
) -> _DrillFeature:
    local_region = _circle_region(center_local, diameter_mm / 2.0)
    designator = _footprint_designator(footprint)
    return _DrillFeature(
        region=_transform_region_to_board(footprint, local_region),
        center=_footprint_local_to_board(footprint, center_local),
        diameter_mm=diameter_mm,
        plated=_pad_is_plated(pad),
        pad_region=_pad_region(footprint, pad, layer, opts),
        source_kind="pad",
        component_designator=designator or None,
        pad_designator=str(getattr(pad, "number", "") or "").strip() or None,
    )


def _pad_slot_hole_feature(
    footprint: object,
    pad: object,
    layer: str,
    opts: PcbLayerStepOptions,
    *,
    center_local: tuple[float, float],
    width: float,
    height: float,
) -> _DrillFeature | None:
    slot_length = max(width, height)
    slot_diameter = min(width, height)
    rotation = _pad_slot_rotation_degrees(pad, width, height)
    local_region = _capsule_region(
        center_local,
        slot_length,
        slot_diameter,
        rotation,
        opts.arc_segments,
    )
    if local_region is None:
        return None
    local_region = _apply_footprint_pad_orientation_offset(
        footprint,
        center_local,
        local_region,
    )
    designator = _footprint_designator(footprint)
    return _DrillFeature(
        region=_transform_region_to_board(footprint, local_region),
        center=_footprint_local_to_board(footprint, center_local),
        diameter_mm=slot_diameter,
        slot_length_mm=slot_length,
        rotation_degrees=rotation - float(getattr(footprint, "at_angle", 0.0) or 0.0),
        plated=_pad_is_plated(pad),
        pad_region=_pad_region(footprint, pad, layer, opts),
        source_kind="pad",
        component_designator=designator or None,
        pad_designator=str(getattr(pad, "number", "") or "").strip() or None,
    )


def _pad_slot_rotation_degrees(pad: object, width: float, height: float) -> float:
    pad_angle = -float(getattr(pad, "at_angle", 0.0) or 0.0)
    return pad_angle + (0.0 if width > height else 90.0)


def _pad_drill_center_local(pad: object) -> tuple[float, float]:
    offset_x = float(getattr(pad, "drill_offset_x", 0.0) or 0.0)
    offset_y = float(getattr(pad, "drill_offset_y", 0.0) or 0.0)
    if not math.isclose(offset_x, 0.0, abs_tol=1e-12) or not math.isclose(
        offset_y, 0.0, abs_tol=1e-12
    ):
        offset_x, offset_y = rotate_point(
            offset_x,
            offset_y,
            -float(getattr(pad, "at_angle", 0.0) or 0.0),
        )
    return (
        float(getattr(pad, "at_x", 0.0) or 0.0) + offset_x,
        float(getattr(pad, "at_y", 0.0) or 0.0) + offset_y,
    )


def _via_hole_feature(via: object) -> _DrillFeature | None:
    diameter_mm = float(getattr(via, "drill", 0.0) or 0.0)
    if diameter_mm <= 0.0:
        return None
    center = (
        float(getattr(via, "at_x", 0.0) or 0.0),
        float(getattr(via, "at_y", 0.0) or 0.0),
    )
    return _DrillFeature(
        region=_circle_region(center, diameter_mm / 2.0),
        center=center,
        diameter_mm=diameter_mm,
        plated=True,
        source_kind="via",
    )


def _drill_overlay_region(feature: _DrillFeature, opts: PcbLayerStepOptions) -> _Region:
    if opts.drill_hole_shape != DRILL_HOLE_SHAPE_RING:
        return feature.region
    if (
        feature.plated
        and feature.pad_region is not None
        and opts.drill_plated_ring_shape == DRILL_PLATED_RING_SHAPE_PAD
    ):
        return _Region(feature.pad_region.outer, [*feature.pad_region.holes, feature.region.outer])
    if opts.drill_ring_width_mm <= 0.0:
        return feature.region
    outer_diameter = feature.diameter_mm + (2.0 * opts.drill_ring_width_mm)
    if feature.slot_length_mm is not None:
        outer = _capsule_region(
            feature.center,
            feature.slot_length_mm + (2.0 * opts.drill_ring_width_mm),
            outer_diameter,
            feature.rotation_degrees,
            opts.arc_segments,
        )
        if outer is None:
            return feature.region
        return _Region(outer.outer, [feature.region.outer])
    outer = _circle_region(feature.center, outer_diameter / 2.0)
    return _Region(outer.outer, [feature.region.outer])


def _via_copper_regions(via: object) -> list[_Region]:
    size = float(getattr(via, "size", 0.0) or 0.0)
    if size <= 0.0:
        return []
    center = (
        float(getattr(via, "at_x", 0.0) or 0.0),
        float(getattr(via, "at_y", 0.0) or 0.0),
    )
    return [_circle_region(center, size / 2.0)]


def _regions_from_polygon_set(polyset: PolygonSet) -> list[_Region]:
    outlines = getattr(polyset, "outlines", []) or []
    holes = getattr(polyset, "holes", []) or []
    regions: list[_Region] = []
    for outline in outlines:
        region = _region_from_points(
            outline,
            holes=holes if len(outlines) == 1 else [],
        )
        if region is not None:
            regions.append(region)
    return regions


def _region_from_points(
    points: Iterable[tuple[float, float]],
    *,
    holes: Iterable[Iterable[tuple[float, float]]] = (),
) -> _Region | None:
    outer = _dedupe_closed_points([(float(x), float(y)) for x, y in points])
    if len(outer) < 3 or abs(_polygon_signed_area(outer)) <= 1e-9:
        return None
    hole_rings = [
        _Ring(hole_points)
        for hole in holes
        if len(hole_points := _dedupe_closed_points([(float(x), float(y)) for x, y in hole])) >= 3
    ]
    return _Region(_Ring(outer), hole_rings)


def _outline_stroke_regions_from_points(
    points: list[tuple[float, float]],
    width_mm: float,
) -> list[_Region]:
    if len(points) < 2 or width_mm <= 0.0:
        return []
    regions: list[_Region] = []
    for index, start in enumerate(points):
        end = points[(index + 1) % len(points)]
        region = _line_capsule_region(start, end, width_mm)
        if region is not None:
            regions.append(region)
    return regions


def _line_capsule_region(
    start: tuple[float, float],
    end: tuple[float, float],
    width_mm: float,
) -> _Region | None:
    radius = width_mm / 2.0
    if radius <= 0.0:
        return None
    if _points_close(start, end):
        return _circle_region(start, radius)
    sx, sy = start
    ex, ey = end
    dx = ex - sx
    dy = ey - sy
    length = math.hypot(dx, dy)
    if length <= 1e-12:
        return _circle_region(start, radius)
    nx = -dy / length
    ny = dx / length
    points = [
        (sx + nx * radius, sy + ny * radius),
        (ex + nx * radius, ey + ny * radius),
        (ex - nx * radius, ey - ny * radius),
        (sx - nx * radius, sy - ny * radius),
    ]
    segments = [
        _Segment("line"),
        _Segment("arc", center=end, sweep="cw"),
        _Segment("line"),
        _Segment("arc", center=start, sweep="cw"),
    ]
    return _Region(_Ring(points, segments))


def _capsule_region(
    center: tuple[float, float],
    length_mm: float,
    diameter_mm: float,
    rotation_degrees: float,
    arc_segments: int,
) -> _Region | None:
    del arc_segments
    straight = max(0.0, length_mm - diameter_mm)
    dx = (straight / 2.0) * math.cos(math.radians(rotation_degrees))
    dy = (straight / 2.0) * math.sin(math.radians(rotation_degrees))
    start = (center[0] - dx, center[1] - dy)
    end = (center[0] + dx, center[1] + dy)
    return _line_capsule_region(start, end, diameter_mm)


def _circle_region(center: tuple[float, float], radius_mm: float) -> _Region:
    cx, cy = center
    points = [
        (cx + radius_mm, cy),
        (cx, cy + radius_mm),
        (cx - radius_mm, cy),
        (cx, cy - radius_mm),
    ]
    segments = [_Segment("arc", center=center, sweep="ccw") for _ in range(4)]
    return _Region(_Ring(points, segments))


def _ellipse_region(
    center: tuple[float, float],
    radius_x_mm: float,
    radius_y_mm: float,
    rotation_degrees: float,
    samples: int,
) -> _Region:
    count = max(16, int(samples))
    points = [
        _rotate_point(
            (
                center[0] + radius_x_mm * math.cos(2.0 * math.pi * idx / count),
                center[1] + radius_y_mm * math.sin(2.0 * math.pi * idx / count),
            ),
            center,
            rotation_degrees,
        )
        for idx in range(count)
    ]
    return _Region(_Ring(points))


def _rectangle_region(
    *,
    center: tuple[float, float],
    width_mm: float,
    height_mm: float,
) -> _Region:
    cx, cy = center
    half_w = width_mm / 2.0
    half_h = height_mm / 2.0
    return _Region(
        _Ring(
            [
                (cx - half_w, cy - half_h),
                (cx + half_w, cy - half_h),
                (cx + half_w, cy + half_h),
                (cx - half_w, cy + half_h),
            ]
        )
    )


def _transform_region_to_board(footprint: object, region: _Region) -> _Region:
    return _Region(
        _transform_ring_to_board(footprint, region.outer),
        [_transform_ring_to_board(footprint, hole) for hole in region.holes],
    )


def _apply_footprint_pad_orientation_offset(
    footprint: object,
    center: tuple[float, float],
    region: _Region,
) -> _Region:
    angle = float(getattr(footprint, "at_angle", 0.0) or 0.0)
    if math.isclose(angle, 0.0, abs_tol=1e-12):
        return region
    return _Region(
        _rotate_ring(region.outer, center, angle),
        [_rotate_ring(hole, center, angle) for hole in region.holes],
    )


def _rotate_ring(
    ring: _Ring,
    center: tuple[float, float],
    angle_degrees: float,
) -> _Ring:
    segments = [
        _Segment(
            kind=segment.kind,
            center=_rotate_point(segment.center, center, angle_degrees)
            if segment.center is not None
            else None,
            sweep=segment.sweep,
        )
        for segment in ring.segments
    ]
    return _Ring(
        [_rotate_point(point, center, angle_degrees) for point in ring.points],
        segments,
    )


def _transform_ring_to_board(footprint: object, ring: _Ring) -> _Ring:
    segments = [
        _Segment(
            kind=segment.kind,
            center=_footprint_local_to_board(footprint, segment.center)
            if segment.center is not None
            else None,
            sweep=segment.sweep,
        )
        for segment in ring.segments
    ]
    return _Ring(
        [_footprint_local_to_board(footprint, point) for point in ring.points],
        segments,
    )


def _footprint_local_to_board(
    footprint: object,
    point: tuple[float, float],
) -> tuple[float, float]:
    return transform_footprint_local_to_board(cast("Footprint", footprint), point)


def _rotate_point(
    point: tuple[float, float],
    origin: tuple[float, float],
    rotation_degrees: float,
) -> tuple[float, float]:
    if math.isclose(rotation_degrees, 0.0, abs_tol=1e-12):
        return point
    angle = math.radians(rotation_degrees)
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    px, py = point
    ox, oy = origin
    dx = px - ox
    dy = py - oy
    return (ox + dx * cos_a - dy * sin_a, oy + dx * sin_a + dy * cos_a)


def _pad_shape_name(pad: object) -> str:
    shape = getattr(pad, "shape", "")
    if isinstance(shape, PadShape):
        return shape.value
    return str(shape).split(".")[-1].casefold()


def _pad_is_plated(pad: object) -> bool:
    pad_type = getattr(pad, "pad_type", None)
    if isinstance(pad_type, PadType):
        return pad_type != PadType.NP_THRU_HOLE
    return str(pad_type) != PadType.NP_THRU_HOLE.value


def _is_copper_layer(layer: str) -> bool:
    return layer.endswith(".Cu")


def _layer_in_collection(layer: str, layers: object) -> bool:
    if isinstance(layers, str):
        values = [layers]
    elif isinstance(layers, Iterable):
        values = [str(item) for item in layers]
    else:
        values = []
    if layer in values:
        return True
    if layer in {"F.Cu", "B.Cu"} and "F&B.Cu" in values:
        return True
    return layer.endswith(".Cu") and "*.Cu" in values


def _via_spans_layer(via: object, layer: str) -> bool:
    layers = [str(item) for item in getattr(via, "layers", []) or []]
    if _layer_in_collection(layer, layers):
        return True
    if not _is_copper_layer(layer) or len(layers) < 2:
        return False
    ordinals: list[int] = []
    for item in layers:
        ordinal = _layer_ordinal_from_name(item)
        if ordinal is not None:
            ordinals.append(ordinal)
    target = _layer_ordinal_from_name(layer)
    if target is None or not ordinals:
        return False
    return min(ordinals) <= target <= max(ordinals)


def _layer_ordinal_from_name(layer: str) -> int | None:
    if layer == "F.Cu":
        return 0
    if layer == "B.Cu":
        return 31
    match = re.fullmatch(r"In(\d+)\.Cu", layer)
    if match:
        return int(match.group(1))
    return None


def _pcb_layer_ordinal(pcb: object, layer: str) -> int | None:
    for board_layer in getattr(pcb, "layers", []) or []:
        if str(getattr(board_layer, "canonical_name", "")) == layer:
            return int(getattr(board_layer, "ordinal", 0))
    return _layer_ordinal_from_name(layer)


def _matches_designator_filter(value: str, patterns: tuple[str, ...]) -> bool:
    return not patterns or _matches_any_pattern(value, patterns)


def _include_pad_feature(designator: str, opts: PcbLayerStepOptions) -> bool:
    return _matches_designator_filter(designator, opts.include_designators)


def _matches_any_pattern(value: str, patterns: Iterable[str]) -> bool:
    normalized = value.casefold()
    return any(fnmatch.fnmatchcase(normalized, pattern.casefold()) for pattern in patterns)


def _footprint_designator(footprint: object) -> str:
    getter = getattr(footprint, "get_property_value", None)
    if callable(getter):
        try:
            return str(getter("Reference", "") or "").strip()
        except (TypeError, ValueError):
            pass
    return str(getattr(footprint, "reference", "") or "").strip()


def _board_origin_mm(pcb: object) -> tuple[float, float]:
    aux_axis_origin = getattr(pcb, "aux_axis_origin_mm", None)
    if callable(aux_axis_origin):
        try:
            origin = aux_axis_origin()
            if isinstance(origin, tuple | list) and len(origin) >= 2:
                return (float(origin[0]), float(origin[1]))
        except (TypeError, ValueError):
            pass
    return (0.0, 0.0)


def _apply_origin_relative_geometry(
    bodies: list[dict[str, object]],
    origin_mm: tuple[float, float],
) -> None:
    dx_mm = -origin_mm[0]
    dy_mm = -origin_mm[1]
    if dx_mm == 0.0 and dy_mm == 0.0:
        return
    for body in bodies:
        _translate_regions(body.get("regions"), dx_mm, dy_mm)
        _translate_regions(body.get("cutouts"), dx_mm, dy_mm)


def _coordinate_origin_payload(origin_mm: tuple[float, float]) -> dict[str, object]:
    return {
        "mode": "kicad_aux_axis_origin",
        "origin_mm": [origin_mm[0], origin_mm[1]],
        "geometry": (
            "x_step_mm=x_kicad_mm-aux_axis_origin_x_mm; y_step_mm=y_kicad_mm-aux_axis_origin_y_mm"
        ),
    }


def _translate_regions(value: object, dx_mm: float, dy_mm: float) -> None:
    if not isinstance(value, list):
        return
    for region in value:
        if isinstance(region, MutableMapping):
            _translate_region(region, dx_mm, dy_mm)


def _translate_region(region: MutableMapping[str, object], dx_mm: float, dy_mm: float) -> None:
    outer = region.get("outer")
    if isinstance(outer, MutableMapping):
        _translate_ring(outer, dx_mm, dy_mm)
    holes = region.get("holes")
    if isinstance(holes, list):
        for hole in holes:
            if isinstance(hole, MutableMapping):
                _translate_ring(hole, dx_mm, dy_mm)


def _translate_ring(ring: MutableMapping[str, object], dx_mm: float, dy_mm: float) -> None:
    points = ring.get("points")
    if isinstance(points, list):
        ring["points"] = [
            [float(point[0]) + dx_mm, float(point[1]) + dy_mm]
            for point in points
            if isinstance(point, list | tuple) and len(point) >= 2
        ]
    segments = ring.get("segments")
    if isinstance(segments, list):
        for segment in segments:
            if isinstance(segment, MutableMapping):
                _translate_segment(segment, dx_mm, dy_mm)


def _translate_segment(segment: MutableMapping[str, object], dx_mm: float, dy_mm: float) -> None:
    center = segment.get("center")
    if isinstance(center, list | tuple) and len(center) >= 2:
        segment["center"] = [float(center[0]) + dx_mm, float(center[1]) + dy_mm]


def _arc_error(opts: PcbLayerStepOptions) -> float:
    return max(0.001, min(0.01, 1.0 / max(16, int(opts.arc_segments))))


def _points_close(
    a: tuple[float, float],
    b: tuple[float, float],
    tol: float = 1e-9,
) -> bool:
    return math.isclose(a[0], b[0], abs_tol=tol) and math.isclose(a[1], b[1], abs_tol=tol)


def _dedupe_closed_points(points: list[tuple[float, float]]) -> list[tuple[float, float]]:
    deduped: list[tuple[float, float]] = []
    for x, y in points:
        point = (float(x), float(y))
        if deduped and _points_close(deduped[-1], point):
            continue
        deduped.append(point)
    if len(deduped) > 1 and _points_close(deduped[0], deduped[-1]):
        deduped.pop()
    return deduped


def _polygon_signed_area(points: list[tuple[float, float]]) -> float:
    if len(points) < 3:
        return 0.0
    area = 0.0
    for index, (x1, y1) in enumerate(points):
        x2, y2 = points[(index + 1) % len(points)]
        area += x1 * y2 - x2 * y1
    return area / 2.0


def _board_name_from_pcb(pcb: object) -> str:
    filepath = getattr(pcb, "filepath", None)
    if filepath:
        return Path(filepath).stem
    return "board"


def _step_name(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_]+", "_", value).strip("_") or "board"


def layer_step_output_name(board_name: str, layer: str) -> str:
    """Return the conventional filename for one generated layer STEP artifact."""
    layer_name = re.sub(r"[^A-Za-z0-9]+", "_", layer.lower()).strip("_")
    return f"{_step_name(board_name)}__{layer_name}_layer.step"

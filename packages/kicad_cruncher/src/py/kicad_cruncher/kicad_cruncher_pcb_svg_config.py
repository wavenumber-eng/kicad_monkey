"""A0 config model for KiCad PCB SVG layer/view rendering."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path

PCB_SVG_CONFIG_FILENAME = "pcb.svg.config"
PCB_SVG_CONFIG_SCHEMA = "pcb.svg.config.a0"
PCB_DEFAULT_SVG_SCALE = 10.0
PCB_SVG_CANVAS_BOUNDS_MODES = frozenset({"board_outline", "all_geometry"})
PCB_SVG_COMPONENT_PROJECTION_MODES = frozenset(
    {"detail", "simple", "bounding_box", "none"}
)
PCB_SVG_COMPONENT_SIDES = frozenset({"top", "bottom"})
PCB_SVG_SPECIAL_LAYERS = frozenset(
    {
        "BOARD_OUTLINE",
        "BOARD_CUTOUTS",
        "DRILLS",
        "SLOTS",
        "ASSEMBLY_HLR_TOP",
        "ASSEMBLY_HLR_BOTTOM",
        "ASSEMBLY_DESIGNATORS_TOP",
        "ASSEMBLY_DESIGNATORS_BOTTOM",
        "PIN1_TOP",
        "PIN1_BOTTOM",
    }
)

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


def _coerce_projection_mode(value: object, default: str, *, field_name: str) -> str:
    raw = str(value or default).strip().lower().replace("-", "_")
    aliases = {
        "bbox": "bounding_box",
        "box": "bounding_box",
        "bounds": "bounding_box",
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
    """Return the default A0 style table shared with Altium-style configs."""
    return {
        "board_outline": {"enabled": True, "color": "#000000", "line_width_mm": 0.10},
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
            "min_dot_diameter_mm": 0.25,
        },
        "keepout": {"enabled": True, "color": "#CC00CC"},
        "assembly_hlr": {
            "enabled": True,
            "color": "#F59E0B",
            "line_width_mm": 0.12,
            "curve_mode": "native_arcs",
            "samples_per_curve": 24,
            "round_digits": 3,
            "include_visible": True,
            "include_outline": True,
            "union_polygons": True,
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
            "legacy": "all_geometry",
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
            "detailed": "detail",
            "bounding-box": "bounding_box",
            "bbox": "bounding_box",
            "box": "bounding_box",
            "off": "none",
        }
        mode = aliases.get(mode, mode)
        if mode not in {"simple", "detail", "bounding_box", "none"}:
            raise ValueError(
                f"Unsupported assembly_hlr_mode {mode!r} for pcb-svg view {name!r}"
            )
        styles = _coerce_object_mapping(
            data.get("styles"),
            field_name=f"views.{name}.styles",
        ) or {}
        return cls(
            name=name,
            enabled=_coerce_bool(data.get("enabled"), True),
            group_id=_coerce_optional_str(data.get("group_id")),
            output_svg=_coerce_optional_str(data.get("output_svg")),
            layers=_coerce_str_list(data.get("layers"), field_name=f"views.{name}.layers"),
            mirror=None if data.get("mirror") is None else _coerce_bool(data.get("mirror"), False),
            assembly_hlr_mode=mode,
            styles=merge_pcb_svg_styles({}, styles),
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
        if self.description:
            result["description"] = self.description
        return result


@dataclass(slots=True)
class _PcbSvgAssemblyConfig:
    default_projection: str = "detail"
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
            dnp_designator_color=str(
                data.get("dnp_designator_color", "#FF0000") or "#FF0000"
            ),
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
    exclude_designator_prefixes: list[str] = field(default_factory=lambda: ["R", "C", "L"])

    @classmethod
    def from_dict(cls, data: dict[str, object] | None) -> _PcbSvgPin1Config:
        default = cls()
        if data is None:
            return default
        prefixes = _coerce_raw_str_list(
            data.get("exclude_designator_prefixes"),
            default.exclude_designator_prefixes,
            field_name="pin1.exclude_designator_prefixes",
        )
        return cls([prefix.upper() for prefix in prefixes if prefix.strip()])

    def to_dict(self) -> dict[str, object]:
        return {"exclude_designator_prefixes": list(self.exclude_designator_prefixes)}


@dataclass(slots=True)
class _PcbSvgComponentOverride:
    side: str | None = None
    projection: str | None = None
    assembly_hlr: dict[str, object] = field(default_factory=dict)
    pin1_enabled: bool | None = None
    pin1_pad: str | None = None
    cathode_pad: str | None = None
    diode: bool | None = None
    diode_line_art: bool | None = None
    show_designator: bool | None = None

    @classmethod
    def from_dict(
        cls, designator: str, data: dict[str, object]
    ) -> _PcbSvgComponentOverride:
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
        return result


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
                "include_special_layers": [
                    "BOARD_OUTLINE",
                    "BOARD_CUTOUTS",
                    "DRILLS",
                    "SLOTS",
                ],
                "output_dir": "layers",
            },
            views=[
                _PcbSvgViewConfig(
                    name="top_view",
                    group_id="pcb-svg-view-top",
                    output_svg="views/{board}__top_view.svg",
                    layers=[
                        "BOARD_OUTLINE",
                        "F.Cu",
                        "F.SilkS",
                        "DRILLS",
                        "SLOTS",
                        "ASSEMBLY_HLR_TOP",
                    ],
                    assembly_hlr_mode="detail",
                ),
                _PcbSvgViewConfig(
                    name="bottom_view",
                    group_id="pcb-svg-view-bottom",
                    output_svg="views/{board}__bottom_view.svg",
                    layers=[
                        "BOARD_OUTLINE",
                        "B.Cu",
                        "B.SilkS",
                        "DRILLS",
                        "SLOTS",
                        "ASSEMBLY_HLR_BOTTOM",
                    ],
                    assembly_hlr_mode="detail",
                ),
                _PcbSvgViewConfig(
                    name="board_cutouts",
                    group_id="pcb-svg-view-board-cutouts",
                    output_svg="views/{board}__board_cutouts.svg",
                    layers=["BOARD_OUTLINE", "BOARD_CUTOUTS"],
                    assembly_hlr_mode="none",
                    description="Board cutouts",
                ),
                _PcbSvgViewConfig(
                    name="top_hlr_bounding_boxes",
                    group_id="pcb-svg-view-top-hlr-bounding-boxes",
                    output_svg="views/{board}__top_hlr_bounding_boxes.svg",
                    layers=["BOARD_OUTLINE", "F.Cu", "ASSEMBLY_HLR_TOP"],
                    assembly_hlr_mode="bounding_box",
                ),
                _PcbSvgViewConfig(
                    name="bottom_hlr_bounding_boxes",
                    group_id="pcb-svg-view-bottom-hlr-bounding-boxes",
                    output_svg="views/{board}__bottom_hlr_bounding_boxes.svg",
                    layers=["BOARD_OUTLINE", "B.Cu", "ASSEMBLY_HLR_BOTTOM"],
                    assembly_hlr_mode="bounding_box",
                ),
                _PcbSvgViewConfig(
                    name="top_pin1_view",
                    group_id="pcb-svg-view-top-pin1",
                    output_svg="views/{board}__top_pin1_view.svg",
                    layers=[
                        "BOARD_OUTLINE",
                        "TOP",
                        "DRILLS",
                        "SLOTS",
                        "PIN1_TOP",
                        "ASSEMBLY_HLR_TOP",
                    ],
                    assembly_hlr_mode="simple",
                ),
                _PcbSvgViewConfig(
                    name="bottom_pin1_view",
                    group_id="pcb-svg-view-bottom-pin1",
                    output_svg="views/{board}__bottom_pin1_view.svg",
                    layers=[
                        "BOARD_OUTLINE",
                        "BOTTOM",
                        "DRILLS",
                        "SLOTS",
                        "PIN1_BOTTOM",
                        "ASSEMBLY_HLR_BOTTOM",
                    ],
                    assembly_hlr_mode="simple",
                ),
                _PcbSvgViewConfig(
                    name="assembly_top_view",
                    output_svg="views/{board}__assembly_top_view.svg",
                    layers=["BOARD_OUTLINE", "F.Cu", "ASSEMBLY_HLR_TOP"],
                    assembly_hlr_mode="simple",
                ),
                _PcbSvgViewConfig(
                    name="assembly_bottom_view",
                    output_svg="views/{board}__assembly_bottom_view.svg",
                    layers=["BOARD_OUTLINE", "B.Cu", "ASSEMBLY_HLR_BOTTOM"],
                    assembly_hlr_mode="simple",
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
        for key in (
            "add_edge_cuts_to_physical_layers",
            "add_drills_to_physical_layers",
            "add_slots_to_physical_layers",
        ):
            if key in layer_outputs:
                layer_outputs[key] = _coerce_bool(layer_outputs.get(key), True)

        raw_views = data.get("views", [view.to_dict() for view in default.views])
        if not isinstance(raw_views, list):
            raise ValueError("pcb-svg config field 'views' must be an array")

        raw_components = data.get("components", {})
        if raw_components is None:
            raw_components = {}
        if not isinstance(raw_components, dict):
            raise ValueError("pcb-svg config field 'components' must be an object")

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
            components={
                str(designator): _PcbSvgComponentOverride.from_dict(str(designator), raw)
                for designator, raw in raw_components.items()
                if isinstance(raw, dict)
            },
            layer_outputs=layer_outputs,
            views=[_PcbSvgViewConfig.from_dict(view) for view in raw_views],
        )

    def enabled_views(self) -> list[_PcbSvgViewConfig]:
        return [view for view in self.views if view.enabled]

    def resolved_styles_for_view(self, view: _PcbSvgViewConfig) -> dict[str, dict[str, object]]:
        return merge_pcb_svg_styles(self.global_options.styles, view.styles)

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
    "resolve_config_output_path",
]

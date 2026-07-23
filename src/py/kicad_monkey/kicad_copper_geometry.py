"""Projection-first PCB copper geometry extraction.

This module deliberately sits beside Plotter IR.  It emits board-space copper
polygons and drill metadata for geometry consumers without constructing a full
``KiCadPcb`` or translating renderer operations back into polygons.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from types import MappingProxyType
from typing import Any, Iterable, Mapping, Sequence

from ._api_markers import public_api
from .kicad_base import PadShape, PadType, unquote_string
from .kicad_geometry import rotate_point
from .kicad_pad import Pad
from .kicad_pcb import KiCadPcb
from .kicad_pcb_footprint import Footprint
from .kicad_pcb_other import Layer, Net, NetRef, Stackup
from .kicad_pcb_polygon_ops import (
    DEFAULT_ERROR_MM,
    PolygonSet,
    circle_to_polygon,
    oval_to_polygon,
)
from .kicad_pcb_projection import KiCadPcbProjection
from .kicad_pcb_routing import Arc, Segment, Via
from .kicad_property import Property
from .kicad_sexpr import SexpFormSpan, SexpSelector, iter_sexp_form_spans


KICAD_COPPER_GEOMETRY_SCHEMA = "kicad.copper_geometry.a0"
KICAD_COPPER_GEOMETRY_ACCEPTED_SCHEMAS = frozenset(
    {
        KICAD_COPPER_GEOMETRY_SCHEMA,
    }
)
NM_PER_MM = 1_000_000

# Contract vocabulary for feature and drill kinds.
COPPER_FEATURE_KINDS = frozenset(
    {"track", "track_arc", "via", "pad", "zone_fill"}
)
COPPER_DRILL_KINDS = frozenset({"via", "plated_pad", "npth_pad"})

NmPoint = tuple[int, int]
NmRing = tuple[NmPoint, ...]


def _enum_value(value: object) -> str:
    if isinstance(value, Enum):
        return str(value.value)
    return str(value or "")


def _mm_to_nm(value: float) -> int:
    return int(round(float(value) * NM_PER_MM))


def _ring_to_nm(points: Iterable[tuple[float, float]]) -> NmRing:
    output: list[NmPoint] = []
    for x, y in points:
        point = (_mm_to_nm(x), _mm_to_nm(y))
        if not output or output[-1] != point:
            output.append(point)
    if len(output) > 1 and output[0] == output[-1]:
        output.pop()
    return tuple(output)


def _ring_to_json(ring: NmRing) -> list[list[int]]:
    return [[x, y] for x, y in ring]


@public_api
@dataclass(frozen=True, slots=True)
class KiCadCopperLayer:
    """One document-local copper layer."""

    index: int
    name: str
    source_ordinal: int | None = None
    layer_type: str = ""
    user_name: str | None = None
    thickness_mm: float | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "name": self.name,
            "source_ordinal": self.source_ordinal,
            "type": self.layer_type,
            "user_name": self.user_name,
            "thickness_mm": self.thickness_mm,
        }


@public_api
@dataclass(frozen=True, slots=True)
class KiCadCopperNet:
    """One document-local net; the name is authoritative."""

    index: int
    name: str
    source_ordinal: int | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "name": self.name,
            "source_ordinal": self.source_ordinal,
        }


@public_api
@dataclass(frozen=True, slots=True)
class KiCadCopperFeature:
    """One board-space copper polygon."""

    source_order: int
    kind: str
    source_uid: str
    net_index: int | None
    layer_indexes: tuple[int, ...]
    outer_nm: NmRing
    holes_nm: tuple[NmRing, ...] = ()
    footprint_uid: str | None = None
    component_ref: str | None = None
    pad_number: str | None = None
    island: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_order": self.source_order,
            "kind": self.kind,
            "source_uid": self.source_uid,
            "net_index": self.net_index,
            "layer_indexes": list(self.layer_indexes),
            "outer_nm": _ring_to_json(self.outer_nm),
            "holes_nm": [_ring_to_json(ring) for ring in self.holes_nm],
            "footprint_uid": self.footprint_uid,
            "component_ref": self.component_ref,
            "pad_number": self.pad_number,
            "island": self.island,
        }


@public_api
@dataclass(frozen=True, slots=True)
class KiCadCopperDrill:
    """One via or pad drill used to derive holes and plated barrels."""

    source_uid: str
    kind: str
    center_nm: NmPoint
    width_nm: int
    height_nm: int
    oval: bool
    plated: bool
    layer_indexes: tuple[int, ...]
    footprint_uid: str | None = None
    component_ref: str | None = None
    pad_number: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_uid": self.source_uid,
            "kind": self.kind,
            "center_nm": list(self.center_nm),
            "width_nm": self.width_nm,
            "height_nm": self.height_nm,
            "oval": self.oval,
            "plated": self.plated,
            "layer_indexes": list(self.layer_indexes),
            "footprint_uid": self.footprint_uid,
            "component_ref": self.component_ref,
            "pad_number": self.pad_number,
        }


@public_api
@dataclass(frozen=True, slots=True)
class KiCadCopperGeometryDocument:
    """Versioned, renderer-neutral copper geometry document."""

    source_path: str | None
    curve_tolerance_mm: float
    bounds_nm: tuple[int, int, int, int] | None
    layers: tuple[KiCadCopperLayer, ...]
    nets: tuple[KiCadCopperNet, ...]
    features: tuple[KiCadCopperFeature, ...]
    drills: tuple[KiCadCopperDrill, ...]
    stats: Mapping[str, int]
    schema: str = KICAD_COPPER_GEOMETRY_SCHEMA

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-compatible representation."""
        return {
            "schema": self.schema,
            "source": {"path": self.source_path},
            "coordinate_space": {
                "unit": "nm",
                "nm_per_mm": NM_PER_MM,
                "x_axis": "board-right",
                "y_axis": "board-down",
                "rings_closed": False,
                "ring_roles_explicit": True,
            },
            "curve_tolerance_mm": self.curve_tolerance_mm,
            "bounds_nm": list(self.bounds_nm) if self.bounds_nm is not None else None,
            "layers": [layer.to_dict() for layer in self.layers],
            "nets": [net.to_dict() for net in self.nets],
            "features": [feature.to_dict() for feature in self.features],
            "drills": [drill.to_dict() for drill in self.drills],
            "stats": dict(self.stats),
        }


@dataclass(slots=True)
class _RawFeature:
    kind: str
    source_uid: str
    net_name: str
    net_ordinal: int | None
    layer_names: tuple[str, ...]
    outer_nm: NmRing
    holes_nm: tuple[NmRing, ...]
    footprint_uid: str | None = None
    component_ref: str | None = None
    pad_number: str | None = None
    island: bool = False


@dataclass(slots=True)
class _RawDrill:
    source_uid: str
    kind: str
    center_nm: NmPoint
    width_nm: int
    height_nm: int
    oval: bool
    plated: bool
    layer_names: tuple[str, ...]
    footprint_uid: str | None = None
    component_ref: str | None = None
    pad_number: str | None = None


@dataclass(slots=True)
class _SlimProjectionSource:
    source_path: Path | None
    layers: list[Layer]
    nets: list[Net]
    segments: list[Segment]
    arcs: list[Arc]
    vias: list[Via]
    zones: list["_SlimZone"]
    footprints: list[Footprint]
    stackup: Stackup | None


@dataclass(slots=True)
class _SlimFilledPolygon:
    layer: str
    island: bool
    outer_nm: NmRing


@dataclass(slots=True)
class _SlimZone:
    net: NetRef
    layers: list[str]
    uuid: str | None
    filled_polygons: list[_SlimFilledPolygon]


_NUMBER_PATTERN = r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"
_XY_PATTERN = re.compile(
    rf"\(\s*xy\s+({_NUMBER_PATTERN})\s+({_NUMBER_PATTERN})\s*\)"
)
_FILLED_LAYER_PATTERN = re.compile(
    r'\(\s*layer\s+(?:"((?:\\.|[^"])*)"|([^\s()]+))\s*\)'
)
_ISLAND_PATTERN = re.compile(r"\(\s*island(?:\s|\))")
_AT_PATTERN = re.compile(
    rf"\(\s*at\s+({_NUMBER_PATTERN})\s+({_NUMBER_PATTERN})"
    rf"(?:\s+({_NUMBER_PATTERN}))?"
)
_UUID_PATTERN = re.compile(
    r'\(\s*uuid\s+(?:"((?:\\.|[^"])*)"|([^\s()]+))\s*\)'
)
_PROPERTY_PATTERN = re.compile(
    r'^\(\s*property\s+"((?:\\.|[^"])*)"\s+"((?:\\.|[^"])*)"'
)


def _parse_filled_polygon_span(span: SexpFormSpan) -> _SlimFilledPolygon:
    text = span.text()
    layer_match = _FILLED_LAYER_PATTERN.search(text)
    quoted_layer = layer_match.group(1) if layer_match is not None else None
    bare_layer = layer_match.group(2) if layer_match is not None else None
    layer = (
        quoted_layer
        if quoted_layer is not None
        else unquote_string(bare_layer or "")
    )
    outer_nm = _ring_to_nm(
        (float(match.group(1)), float(match.group(2)))
        for match in _XY_PATTERN.finditer(text)
    )
    return _SlimFilledPolygon(
        layer=layer,
        island=_ISLAND_PATTERN.search(text) is not None,
        outer_nm=outer_nm,
    )


def _resolve_net_ref(
    net_ref: NetRef,
    *,
    name_by_ordinal: dict[int, str],
    ordinal_by_name: dict[str, int],
) -> NetRef:
    return net_ref.resolve_name(name_by_ordinal).resolve_ordinal(ordinal_by_name)


def _parse_slim_footprint(
    _parent: SexpFormSpan,
    children: Sequence[SexpFormSpan],
    *,
    name_by_ordinal: dict[int, str],
    ordinal_by_name: dict[str, int],
) -> Footprint:
    at_x = 0.0
    at_y = 0.0
    at_angle = 0.0
    uuid: str | None = None
    properties: list[Property] = []
    pads: list[Pad] = []
    for child in children:
        if child.head == "at":
            match = _AT_PATTERN.match(child.text())
            if match is not None:
                at_x = float(match.group(1))
                at_y = float(match.group(2))
                if match.group(3) is not None:
                    at_angle = float(match.group(3))
        elif child.head == "uuid":
            match = _UUID_PATTERN.match(child.text())
            if match is not None:
                uuid = match.group(1) or match.group(2)
        elif child.head == "property":
            match = _PROPERTY_PATTERN.match(child.text())
            if match is not None and match.group(1) == "Reference":
                properties.append(
                    Property(name=match.group(1), value=match.group(2))
                )
        elif child.head == "pad":
            parsed = child.parse()
            if not isinstance(parsed, list) or not parsed:
                continue
            pad = Pad.from_sexp(parsed)
            pad.net = _resolve_net_ref(
                pad.net,
                name_by_ordinal=name_by_ordinal,
                ordinal_by_name=ordinal_by_name,
            )
            pads.append(pad)
    return Footprint(
        library_link="",
        at_x=at_x,
        at_y=at_y,
        at_angle=at_angle,
        uuid=uuid,
        properties=properties,
        pads=pads,
    )


def _parse_slim_zone(
    _parent: SexpFormSpan,
    children: Sequence[SexpFormSpan],
    *,
    name_by_ordinal: dict[int, str],
    ordinal_by_name: dict[str, int],
) -> _SlimZone:
    raw_net: object = 0
    explicit_name = ""
    layers: list[str] = []
    uuid: str | None = None
    filled_polygons: list[_SlimFilledPolygon] = []
    for child in children:
        if child.head == "filled_polygon":
            filled_polygons.append(_parse_filled_polygon_span(child))
            continue
        parsed = child.parse()
        if not isinstance(parsed, list) or not parsed:
            continue
        if child.head == "net" and len(parsed) > 1:
            raw_net = parsed[1]
        elif child.head == "net_name":
            explicit_name = unquote_string(parsed[1]) if len(parsed) > 1 else ""
        elif child.head == "layers":
            layers = [unquote_string(value) for value in parsed[1:]]
        elif child.head == "layer" and len(parsed) > 1:
            layers = [unquote_string(parsed[1])]
        elif child.head == "uuid" and len(parsed) > 1:
            uuid = unquote_string(parsed[1])
    net = _resolve_net_ref(
        NetRef.from_raw_token(raw_net, explicit_name=explicit_name),
        name_by_ordinal=name_by_ordinal,
        ordinal_by_name=ordinal_by_name,
    )
    return _SlimZone(
        net=net,
        layers=layers,
        uuid=uuid,
        filled_polygons=filled_polygons,
    )


def _slim_projection_source(
    projection: KiCadPcbProjection,
) -> _SlimProjectionSource | None:
    source_text = getattr(projection, "_source_text", None)
    if source_text is None or getattr(projection, "_board", None) is not None:
        return None
    root = "kicad_pcb"
    top_level_heads = {
        "layers",
        "net",
        "segment",
        "arc",
        "via",
        "footprint",
        "module",
        "zone",
    }
    paths: set[tuple[str, ...]] = {
        (root, head) for head in top_level_heads
    }
    paths.add((root, "setup", "stackup"))
    for footprint_head in ("footprint", "module"):
        paths.update(
            {
                (root, footprint_head, "at"),
                (root, footprint_head, "uuid"),
                (root, footprint_head, "property"),
                (root, footprint_head, "pad"),
            }
        )
    paths.update(
        {
            (root, "zone", "net"),
            (root, "zone", "net_name"),
            (root, "zone", "layer"),
            (root, "zone", "layers"),
            (root, "zone", "uuid"),
            (root, "zone", "filled_polygon"),
        }
    )
    spans = iter_sexp_form_spans(
        source_text,
        SexpSelector(paths=paths),
        source_path=projection.source_path,
    )
    top_level: dict[str, list[SexpFormSpan]] = {}
    children_by_parent: dict[int, list[SexpFormSpan]] = {}
    stackup_span: SexpFormSpan | None = None
    current_parent: SexpFormSpan | None = None
    for span in spans:
        if span.path == (root, "setup", "stackup"):
            stackup_span = span
            continue
        if span.depth == 1:
            top_level.setdefault(str(span.head or ""), []).append(span)
            current_parent = (
                span
                if span.head in {"footprint", "module", "zone"}
                else None
            )
            if current_parent is not None:
                children_by_parent[current_parent.start_offset] = []
            continue
        if (
            current_parent is not None
            and span.depth == 2
            and span.start_offset < current_parent.end_offset
        ):
            children_by_parent[current_parent.start_offset].append(span)

    layers: list[Layer] = []
    layer_spans = top_level.get("layers", [])
    if layer_spans:
        parsed_layers = layer_spans[0].parse()
        if isinstance(parsed_layers, list):
            layers = [
                Layer.from_sexp(item)
                for item in parsed_layers[1:]
                if isinstance(item, list) and item
            ]
    stackup = None
    if stackup_span is not None:
        parsed_stackup = stackup_span.parse()
        if isinstance(parsed_stackup, list):
            stackup = Stackup.from_sexp(parsed_stackup)
    nets = [
        Net.from_sexp(parsed)
        for span in top_level.get("net", [])
        if isinstance((parsed := span.parse()), list)
    ]
    name_by_ordinal = {net.ordinal: net.name for net in nets}
    ordinal_by_name = {net.name: net.ordinal for net in nets}

    def parse_net_bound(
        spans_to_parse: Sequence[SexpFormSpan],
        factory: Any,
    ) -> list[Any]:
        output: list[Any] = []
        for span in spans_to_parse:
            parsed = span.parse()
            if not isinstance(parsed, list):
                continue
            item = factory(parsed)
            item.net = _resolve_net_ref(
                item.net,
                name_by_ordinal=name_by_ordinal,
                ordinal_by_name=ordinal_by_name,
            )
            output.append(item)
        return output

    footprint_spans = [
        *top_level.get("footprint", []),
        *top_level.get("module", []),
    ]
    footprint_spans.sort(key=lambda span: span.start_offset)
    zones = [
        _parse_slim_zone(
            span,
            children_by_parent.get(span.start_offset, ()),
            name_by_ordinal=name_by_ordinal,
            ordinal_by_name=ordinal_by_name,
        )
        for span in top_level.get("zone", [])
    ]
    footprints = [
        _parse_slim_footprint(
            span,
            children_by_parent.get(span.start_offset, ()),
            name_by_ordinal=name_by_ordinal,
            ordinal_by_name=ordinal_by_name,
        )
        for span in footprint_spans
    ]
    return _SlimProjectionSource(
        source_path=projection.source_path,
        layers=layers,
        nets=nets,
        segments=parse_net_bound(
            top_level.get("segment", ()),
            Segment.from_sexp,
        ),
        arcs=parse_net_bound(top_level.get("arc", ()), Arc.from_sexp),
        vias=parse_net_bound(top_level.get("via", ()), Via.from_sexp),
        zones=zones,
        footprints=footprints,
        stackup=stackup,
    )


class _BoardSource:
    def __init__(
        self,
        source: KiCadPcb | KiCadPcbProjection | _SlimProjectionSource,
    ) -> None:
        self.source = source
        self.is_projection = isinstance(source, KiCadPcbProjection)

    @property
    def source_path(self) -> str | None:
        value = getattr(self.source, "source_path", None)
        return str(value) if value is not None else None

    def collection(self, name: str) -> list[Any]:
        value = getattr(self.source, name)
        return list(value() if self.is_projection else value)

    def stackup(self) -> Any:
        value = getattr(self.source, "stackup", None)
        return value() if self.is_projection and callable(value) else value


def _is_copper_layer(name: str) -> bool:
    return str(name).endswith(".Cu")


def _expand_layer_names(
    requested: Sequence[str],
    copper_layer_names: Sequence[str],
) -> tuple[str, ...]:
    requested_names = [str(name) for name in requested if str(name)]
    if "*.Cu" in requested_names:
        return tuple(copper_layer_names)
    if "F&B.Cu" in requested_names:
        return tuple(
            name for name in copper_layer_names if name in {"F.Cu", "B.Cu"}
        )
    if len(requested_names) == 2 and all(
        name in copper_layer_names for name in requested_names
    ):
        first = copper_layer_names.index(requested_names[0])
        last = copper_layer_names.index(requested_names[1])
        low, high = sorted((first, last))
        return tuple(copper_layer_names[low : high + 1])
    return tuple(name for name in requested_names if name in copper_layer_names)


def _net_parts(obj: object) -> tuple[str, int | None]:
    net = getattr(obj, "net", None)
    return (
        str(getattr(net, "name", "") or ""),
        getattr(net, "ordinal", None),
    )


def _component_reference(footprint: object) -> str:
    getter = getattr(footprint, "get_property_value", None)
    if callable(getter):
        return str(getter("Reference") or "")
    return ""


def _transform_footprint_point(
    point: tuple[float, float],
    footprint: object,
) -> tuple[float, float]:
    x, y = rotate_point(point[0], point[1], -float(getattr(footprint, "at_angle", 0.0)))
    return (
        x + float(getattr(footprint, "at_x", 0.0)),
        y + float(getattr(footprint, "at_y", 0.0)),
    )


def _pad_local_rings(pad: Pad, error: float) -> list[list[tuple[float, float]]]:
    shape = getattr(pad, "shape", None)
    cx = float(getattr(pad, "at_x", 0.0))
    cy = float(getattr(pad, "at_y", 0.0))
    size_x = float(getattr(pad, "size_x", 0.0))
    size_y = float(getattr(pad, "size_y", 0.0))
    drill = float(getattr(pad, "drill", 0.0) or 0.0)
    pad_type = getattr(pad, "pad_type", None)
    if (
        pad_type == PadType.NP_THRU_HOLE
        and shape == PadShape.CIRCLE
        and max(size_x, size_y) <= drill
    ):
        return []
    if shape == PadShape.CIRCLE:
        return [circle_to_polygon((cx, cy), size_x / 2.0, error)]
    if shape == PadShape.OVAL:
        start, end, width = pad._to_oval_segment(cx, cy)
        return [oval_to_polygon(start, end, width, error)]
    if shape == PadShape.ROUNDRECT:
        return [pad._to_roundrect_polygon(cx, cy, error)]
    if shape == PadShape.TRAPEZOID:
        return [pad._to_trapezoid_polygon(cx, cy)]
    if shape == PadShape.RECT:
        return [pad._to_rect_polygon(cx, cy)]
    if shape == PadShape.CUSTOM:
        output: list[list[tuple[float, float]]] = []
        angle = -float(pad.at_angle)
        for primitive in pad.custom_primitives:
            if primitive.primitive_type != "gr_poly":
                continue
            points = primitive.points
            if not points:
                continue
            ring: list[tuple[float, float]] = []
            for x, y in points:
                rx, ry = rotate_point(float(x), float(y), angle)
                ring.append((rx + cx, ry + cy))
            output.append(ring)
        return output
    return []


def _pad_drill_local_ring(
    pad: Pad,
    error: float,
) -> tuple[list[tuple[float, float]], tuple[float, float], float, float] | None:
    drill = float(getattr(pad, "drill", 0.0) or 0.0)
    width = float(getattr(pad, "drill_width", 0.0) or drill)
    height = float(getattr(pad, "drill_height", 0.0) or drill)
    if width <= 0 or height <= 0:
        return None
    offset_x = float(getattr(pad, "drill_offset_x", 0.0) or 0.0)
    offset_y = float(getattr(pad, "drill_offset_y", 0.0) or 0.0)
    angle = -float(getattr(pad, "at_angle", 0.0))
    offset_x, offset_y = rotate_point(offset_x, offset_y, angle)
    center = (
        float(getattr(pad, "at_x", 0.0)) + offset_x,
        float(getattr(pad, "at_y", 0.0)) + offset_y,
    )
    if abs(width - height) <= 1e-12:
        return circle_to_polygon(center, width / 2.0, error), center, width, height
    if width > height:
        delta = width - height
        start = (center[0] - delta / 2.0, center[1])
        end = (center[0] + delta / 2.0, center[1])
        ring = oval_to_polygon(start, end, height, error)
    else:
        delta = height - width
        start = (center[0], center[1] - delta / 2.0)
        end = (center[0], center[1] + delta / 2.0)
        ring = oval_to_polygon(start, end, width, error)
    if angle:
        ring = [
            rotate_point(x, y, angle, center[0], center[1])
            for x, y in ring
        ]
    return ring, center, width, height


def _polygon_set_features(
    *,
    kind: str,
    source_uid: str,
    net_name: str,
    net_ordinal: int | None,
    layer_names: tuple[str, ...],
    polygon_set: PolygonSet,
) -> list[_RawFeature]:
    holes_nm = tuple(
        ring_nm
        for hole in polygon_set.holes
        if len((ring_nm := _ring_to_nm(hole))) >= 3
    )
    return [
        _RawFeature(
            kind=kind,
            source_uid=source_uid,
            net_name=net_name,
            net_ordinal=net_ordinal,
            layer_names=layer_names,
            outer_nm=_ring_to_nm(outline),
            holes_nm=holes_nm,
        )
        for outline in polygon_set.outlines
        if len(outline) >= 3
    ]


def _collect_raw_geometry(
    board: _BoardSource,
    *,
    curve_tolerance_mm: float,
    copper_layer_names: Sequence[str],
) -> tuple[list[_RawFeature], list[_RawDrill], dict[str, int]]:
    features: list[_RawFeature] = []
    drills: list[_RawDrill] = []
    stats = {
        "tracks": 0,
        "track_arcs": 0,
        "vias": 0,
        "pads": 0,
        "zone_fills": 0,
        "features": 0,
        "drills": 0,
    }

    for segment in board.collection("segments"):
        layer_names = _expand_layer_names([segment.layer], copper_layer_names)
        if not layer_names:
            continue
        net_name, net_ordinal = _net_parts(segment)
        features.extend(
            _polygon_set_features(
                kind="track",
                source_uid=str(segment.uuid or ""),
                net_name=net_name,
                net_ordinal=net_ordinal,
                layer_names=layer_names,
                polygon_set=segment._to_poly(curve_tolerance_mm),
            )
        )
        stats["tracks"] += 1

    for arc in board.collection("arcs"):
        layer_names = _expand_layer_names([arc.layer], copper_layer_names)
        if not layer_names:
            continue
        net_name, net_ordinal = _net_parts(arc)
        features.extend(
            _polygon_set_features(
                kind="track_arc",
                source_uid=str(arc.uuid or ""),
                net_name=net_name,
                net_ordinal=net_ordinal,
                layer_names=layer_names,
                polygon_set=arc._to_poly(curve_tolerance_mm),
            )
        )
        stats["track_arcs"] += 1

    for via in board.collection("vias"):
        layer_names = _expand_layer_names(via.layers, copper_layer_names)
        if not layer_names or float(via.size) <= 0:
            continue
        net_name, net_ordinal = _net_parts(via)
        outer_nm = _ring_to_nm(
            circle_to_polygon(
                (float(via.at_x), float(via.at_y)),
                float(via.size) / 2.0,
                curve_tolerance_mm,
            )
        )
        holes_nm: tuple[NmRing, ...] = ()
        if float(via.drill) > 0:
            holes_nm = (
                _ring_to_nm(
                    circle_to_polygon(
                        (float(via.at_x), float(via.at_y)),
                        float(via.drill) / 2.0,
                        curve_tolerance_mm,
                    )
                ),
            )
        features.append(
            _RawFeature(
                kind="via",
                source_uid=str(via.uuid or ""),
                net_name=net_name,
                net_ordinal=net_ordinal,
                layer_names=layer_names,
                outer_nm=outer_nm,
                holes_nm=holes_nm,
            )
        )
        if float(via.drill) > 0:
            drills.append(
                _RawDrill(
                    source_uid=str(via.uuid or ""),
                    kind="via",
                    center_nm=(
                        _mm_to_nm(float(via.at_x)),
                        _mm_to_nm(float(via.at_y)),
                    ),
                    width_nm=_mm_to_nm(float(via.drill)),
                    height_nm=_mm_to_nm(float(via.drill)),
                    oval=False,
                    plated=True,
                    layer_names=layer_names,
                )
            )
        stats["vias"] += 1

    for zone in board.collection("zones"):
        net_name, net_ordinal = _net_parts(zone)
        for filled in zone.filled_polygons:
            layer_name = str(filled.layer or "")
            if not layer_name and len(zone.layers) == 1:
                layer_name = str(zone.layers[0])
            layer_names = _expand_layer_names([layer_name], copper_layer_names)
            outer_nm = getattr(filled, "outer_nm", None)
            if outer_nm is None:
                outer_nm = _ring_to_nm(filled.points)
            if not layer_names or len(outer_nm) < 3:
                continue
            features.append(
                _RawFeature(
                    kind="zone_fill",
                    source_uid=str(zone.uuid or ""),
                    net_name=net_name,
                    net_ordinal=net_ordinal,
                    layer_names=layer_names,
                    outer_nm=outer_nm,
                    holes_nm=(),
                    island=bool(filled.island),
                )
            )
            stats["zone_fills"] += 1

    for footprint in board.collection("footprints"):
        footprint_uid = str(getattr(footprint, "uuid", "") or "")
        component_ref = _component_reference(footprint)
        for pad in footprint.pads:
            layer_names = _expand_layer_names(pad.layers, copper_layer_names)
            if not layer_names:
                continue
            net_name, net_ordinal = _net_parts(pad)
            # Board-embedded pads store absolute orientation (same convention as
            # pcb_footprint_to_record). Geometry helpers expect footprint-local
            # angle, so subtract the footprint placement angle first.
            absolute_angle = float(getattr(pad, "at_angle", 0.0) or 0.0)
            footprint_angle = float(getattr(footprint, "at_angle", 0.0) or 0.0)
            relative_angle = absolute_angle - footprint_angle
            saved_angle = pad.at_angle
            pad.at_angle = relative_angle
            try:
                local_rings = _pad_local_rings(pad, curve_tolerance_mm)
                drill_info = _pad_drill_local_ring(pad, curve_tolerance_mm)
            finally:
                pad.at_angle = saved_angle
            world_holes_nm: tuple[NmRing, ...] = ()
            if drill_info is not None:
                drill_ring, drill_center, drill_width, drill_height = drill_info
                world_holes_nm = (
                    _ring_to_nm(
                        _transform_footprint_point(point, footprint)
                        for point in drill_ring
                    ),
                )
                world_center = _transform_footprint_point(drill_center, footprint)
                plated = getattr(pad, "pad_type", None) != PadType.NP_THRU_HOLE
                drills.append(
                    _RawDrill(
                        source_uid=str(pad.uuid or ""),
                        kind="plated_pad" if plated else "npth_pad",
                        center_nm=(
                            _mm_to_nm(world_center[0]),
                            _mm_to_nm(world_center[1]),
                        ),
                        width_nm=_mm_to_nm(drill_width),
                        height_nm=_mm_to_nm(drill_height),
                        oval=abs(drill_width - drill_height) > 1e-12,
                        plated=plated,
                        layer_names=layer_names,
                        footprint_uid=footprint_uid,
                        component_ref=component_ref,
                        pad_number=str(pad.number),
                    )
                )
            for local_ring in local_rings:
                if len(local_ring) < 3:
                    continue
                features.append(
                    _RawFeature(
                        kind="pad",
                        source_uid=str(pad.uuid or ""),
                        net_name=net_name,
                        net_ordinal=net_ordinal,
                        layer_names=layer_names,
                        outer_nm=_ring_to_nm(
                            _transform_footprint_point(point, footprint)
                            for point in local_ring
                        ),
                        holes_nm=world_holes_nm,
                        footprint_uid=footprint_uid,
                        component_ref=component_ref,
                        pad_number=str(pad.number),
                    )
                )
            stats["pads"] += 1

    stats["features"] = len(features)
    stats["drills"] = len(drills)
    return features, drills, stats


def _build_layers(
    board: _BoardSource,
) -> tuple[tuple[KiCadCopperLayer, ...], tuple[str, ...]]:
    source_layers = [
        layer
        for layer in board.collection("layers")
        if _is_copper_layer(str(layer.canonical_name))
    ]
    stackup = board.stackup()
    thickness_by_name = {
        str(layer.name): float(layer.thickness)
        for layer in getattr(stackup, "layers", ()) or ()
        if str(getattr(layer, "type_name", "")).lower() == "copper"
    }
    layers = tuple(
        KiCadCopperLayer(
            index=index,
            name=str(layer.canonical_name),
            source_ordinal=int(layer.ordinal),
            layer_type=_enum_value(layer.layer_type),
            user_name=str(layer.user_name) if layer.user_name else None,
            thickness_mm=thickness_by_name.get(str(layer.canonical_name)),
        )
        for index, layer in enumerate(source_layers)
    )
    return layers, tuple(layer.name for layer in layers)


def _build_nets(
    raw_features: Sequence[_RawFeature],
    source_nets: Sequence[object],
) -> tuple[tuple[KiCadCopperNet, ...], dict[tuple[str, int | None], int]]:
    keys = {
        (feature.net_name, feature.net_ordinal)
        for feature in raw_features
        if feature.net_name or feature.net_ordinal is not None
    }
    keys.update(
        (
            str(getattr(net, "name", "") or ""),
            int(getattr(net, "ordinal")),
        )
        for net in source_nets
        if str(getattr(net, "name", "") or "")
        and getattr(net, "ordinal", None) is not None
    )
    ordered = sorted(
        keys,
        key=lambda item: (
            item[0],
            item[1] if item[1] is not None else -1,
        ),
    )
    nets = tuple(
        KiCadCopperNet(index=index, name=name, source_ordinal=ordinal)
        for index, (name, ordinal) in enumerate(ordered)
    )
    return nets, {key: index for index, key in enumerate(ordered)}


def _bounds_nm(features: Sequence[KiCadCopperFeature]) -> tuple[int, int, int, int] | None:
    min_x: int | None = None
    min_y: int | None = None
    max_x: int | None = None
    max_y: int | None = None
    for feature in features:
        for x, y in feature.outer_nm:
            min_x = x if min_x is None else min(min_x, x)
            min_y = y if min_y is None else min(min_y, y)
            max_x = x if max_x is None else max(max_x, x)
            max_y = y if max_y is None else max(max_y, y)
    if min_x is None or min_y is None or max_x is None or max_y is None:
        return None
    return (min_x, min_y, max_x, max_y)


def _coerce_source(
    source: str | Path | KiCadPcb | KiCadPcbProjection,
) -> _BoardSource:
    if isinstance(source, KiCadPcbProjection):
        return _BoardSource(_slim_projection_source(source) or source)
    if isinstance(source, KiCadPcb):
        return _BoardSource(source)
    projection = KiCadPcbProjection.from_file(Path(source))
    return _BoardSource(_slim_projection_source(projection) or projection)


@public_api
def emit_pcb_copper_geometry(
    source: str | Path | KiCadPcb | KiCadPcbProjection,
    *,
    curve_tolerance_mm: float = DEFAULT_ERROR_MM,
) -> KiCadCopperGeometryDocument:
    """Emit a renderer-neutral copper geometry document.

    Path inputs open a :class:`KiCadPcbProjection` and extract only copper
    families needed for tracks, arcs, vias, pads, filled zones, and drills.
    Callers that already own a :class:`KiCadPcb` or
    :class:`KiCadPcbProjection` may pass those objects directly.

    The returned document uses integer nanometres, unclosed rings, dense
    document-local layer/net indexes, and authoritative net names. It does
    not construct Plotter IR and does not include silk, fab, text, mask,
    paste, or 3D component models.
    """
    if curve_tolerance_mm <= 0:
        raise ValueError("curve_tolerance_mm must be positive")
    board = _coerce_source(source)
    layers, copper_layer_names = _build_layers(board)
    raw_features, raw_drills, stats = _collect_raw_geometry(
        board,
        curve_tolerance_mm=float(curve_tolerance_mm),
        copper_layer_names=copper_layer_names,
    )
    nets, net_index_by_key = _build_nets(
        raw_features,
        board.collection("nets"),
    )
    layer_index_by_name = {layer.name: layer.index for layer in layers}
    features: list[KiCadCopperFeature] = []
    for source_order, raw in enumerate(raw_features):
        if len(raw.outer_nm) < 3:
            continue
        layer_indexes = tuple(
            layer_index_by_name[name]
            for name in raw.layer_names
            if name in layer_index_by_name
        )
        if not layer_indexes:
            continue
        features.append(
            KiCadCopperFeature(
                source_order=source_order,
                kind=raw.kind,
                source_uid=raw.source_uid,
                net_index=net_index_by_key.get((raw.net_name, raw.net_ordinal)),
                layer_indexes=layer_indexes,
                outer_nm=raw.outer_nm,
                holes_nm=raw.holes_nm,
                footprint_uid=raw.footprint_uid,
                component_ref=raw.component_ref,
                pad_number=raw.pad_number,
                island=raw.island,
            )
        )
    drills = tuple(
        KiCadCopperDrill(
            source_uid=raw.source_uid,
            kind=raw.kind,
            center_nm=raw.center_nm,
            width_nm=raw.width_nm,
            height_nm=raw.height_nm,
            oval=raw.oval,
            plated=raw.plated,
            layer_indexes=tuple(
                layer_index_by_name[name]
                for name in raw.layer_names
                if name in layer_index_by_name
            ),
            footprint_uid=raw.footprint_uid,
            component_ref=raw.component_ref,
            pad_number=raw.pad_number,
        )
        for raw in raw_drills
    )
    stats = {**stats, "features": len(features), "drills": len(drills)}
    feature_tuple = tuple(features)
    return KiCadCopperGeometryDocument(
        source_path=board.source_path,
        curve_tolerance_mm=float(curve_tolerance_mm),
        bounds_nm=_bounds_nm(feature_tuple),
        layers=layers,
        nets=nets,
        features=feature_tuple,
        drills=drills,
        stats=MappingProxyType(stats),
    )


__all__ = [
    "COPPER_DRILL_KINDS",
    "COPPER_FEATURE_KINDS",
    "KICAD_COPPER_GEOMETRY_ACCEPTED_SCHEMAS",
    "KICAD_COPPER_GEOMETRY_SCHEMA",
    "KiCadCopperDrill",
    "KiCadCopperFeature",
    "KiCadCopperGeometryDocument",
    "KiCadCopperLayer",
    "KiCadCopperNet",
    "emit_pcb_copper_geometry",
]

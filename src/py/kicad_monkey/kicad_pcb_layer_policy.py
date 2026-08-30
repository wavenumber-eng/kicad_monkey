"""Resolve KiCad PTH pad/via copper-flash policy against a board.

KiCad keeps an item's authored layer span separate from the copper layers on
which its annular land is actually flashed.  Consumers that model physical
drills must use the former; copper renderers and exporters must use the latter.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass
import math
from typing import Any, Iterable, Sequence

from shapely import affinity
from shapely.geometry import LineString, Point, Polygon, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union


_GEOMETRY_TOLERANCE_MM = 1e-6
_GeometryBounds = tuple[float, float, float, float]


def copper_layer_stack(board: Any) -> tuple[str, ...]:
    """Return enabled copper layer names in physical stack order."""

    return tuple(
        name
        for layer in getattr(board, "layers", ()) or ()
        if (name := str(getattr(layer, "canonical_name", "") or "")).endswith(".Cu")
    )


def copper_span_layers(
    authored_layers: Sequence[str], copper_stack: Sequence[str]
) -> tuple[str, ...]:
    """Expand KiCad wildcard/end-point layer syntax to a physical Cu span."""

    stack = tuple(str(layer) for layer in copper_stack if str(layer).endswith(".Cu"))
    authored = tuple(str(layer) for layer in authored_layers)
    if "*.Cu" in authored:
        return stack

    endpoints = [layer for layer in authored if layer in stack]
    if len(endpoints) >= 2:
        first = stack.index(endpoints[0])
        last = stack.index(endpoints[-1])
        low, high = sorted((first, last))
        return stack[low : high + 1]
    return tuple(layer for layer in stack if layer in endpoints)


def pad_copper_layers(
    authored_layers: Sequence[str], copper_stack: Sequence[str]
) -> tuple[str, ...]:
    """Expand a pad's exact copper membership without inventing via spans."""

    stack = tuple(str(layer) for layer in copper_stack if str(layer).endswith(".Cu"))
    authored = {str(layer) for layer in authored_layers}
    if "*.Cu" in authored:
        return stack
    selected = {layer for layer in authored if layer in stack}
    if "F&B.Cu" in authored:
        selected.update(layer for layer in ("F.Cu", "B.Cu") if layer in stack)
    return tuple(layer for layer in stack if layer in selected)


def _is_via(item: Any) -> bool:
    return hasattr(item, "start_end_only")


def _pad_type_value(pad: Any) -> str:
    value = getattr(pad, "pad_type", "")
    return str(getattr(value, "value", value))


def _pad_policy_applies(pad: Any) -> bool:
    return _pad_type_value(pad) == "thru_hole"


def effective_copper_flash_layers(
    item: Any,
    copper_stack: Sequence[str],
    *,
    connected_layers: Iterable[str] = (),
) -> tuple[str, ...]:
    """Resolve one pad/via to the copper layers KiCad permits it to flash on."""

    authored = tuple(str(layer) for layer in getattr(item, "layers", ()) or ())
    is_via = _is_via(item)
    span = (
        copper_span_layers(authored, copper_stack)
        if is_via
        else pad_copper_layers(authored, copper_stack)
    )
    if not span:
        return ()

    if is_via and bool(getattr(item, "start_end_only", False)):
        return tuple(layer for layer in span if layer in {span[0], span[-1]})

    if not bool(getattr(item, "remove_unused_layers", False)) or (
        not is_via and not _pad_policy_applies(item)
    ):
        return span

    retained = {str(layer) for layer in connected_layers}
    zone_connections = getattr(item, "zone_layer_connections", None)
    retained.update(getattr(zone_connections, "forced_layers", ()) or ())
    if bool(getattr(item, "keep_end_layers", False)):
        if is_via:
            retained.update((span[0], span[-1]))
        else:
            retained.update(layer for layer in ("F.Cu", "B.Cu") if layer in span)
    return tuple(layer for layer in span if layer in retained)


def effective_item_layers(
    item: Any,
    copper_stack: Sequence[str],
    *,
    connected_layers: Iterable[str] = (),
) -> tuple[str, ...]:
    """Return authored non-Cu layers plus resolved copper-flash layers."""

    authored = tuple(str(layer) for layer in getattr(item, "layers", ()) or ())
    flashed = set(
        effective_copper_flash_layers(
            item,
            copper_stack,
            connected_layers=connected_layers,
        )
    )
    result: list[str] = []
    for layer in authored:
        if layer == "*.Cu":
            result.extend(name for name in copper_stack if name in flashed)
        elif layer == "F&B.Cu":
            result.extend(
                name
                for name in ("F.Cu", "B.Cu")
                if name in flashed and name not in result
            )
        elif layer.endswith(".Cu"):
            # Via endpoints describe a span; pad copper membership is exact.
            continue
        elif layer not in result:
            result.append(layer)
    result.extend(
        layer for layer in copper_stack if layer in flashed and layer not in result
    )
    return tuple(result)


def _net_key(item: Any) -> tuple[str, object] | None:
    net = getattr(item, "net", None)
    ordinal = getattr(net, "ordinal", None)
    if ordinal is not None:
        return ("ordinal", int(ordinal))
    name = str(getattr(net, "name", "") or "")
    return ("name", name) if name else None


def _footprint_pad_position(footprint: Any, pad: Any) -> tuple[float, float]:
    angle = math.radians(float(getattr(footprint, "at_angle", 0.0) or 0.0))
    local_x = float(getattr(pad, "at_x", 0.0) or 0.0)
    local_y = float(getattr(pad, "at_y", 0.0) or 0.0)
    # KiCad board coordinates are Y-down; its positive footprint angle uses
    # the same transform as the existing board bounds/IR code.
    rotated_x = local_x * math.cos(angle) + local_y * math.sin(angle)
    rotated_y = -local_x * math.sin(angle) + local_y * math.cos(angle)
    return (
        rotated_x + float(getattr(footprint, "at_x", 0.0) or 0.0),
        rotated_y + float(getattr(footprint, "at_y", 0.0) or 0.0),
    )


def _polygon(points: Sequence[tuple[float, float]]) -> BaseGeometry:
    if len(points) < 3:
        return Polygon()
    geometry = Polygon(points)
    return geometry if geometry.is_valid else geometry.buffer(0)


def _transform_footprint_geometry(
    geometry: BaseGeometry, footprint: Any
) -> BaseGeometry:
    geometry = affinity.rotate(
        geometry,
        -float(getattr(footprint, "at_angle", 0.0) or 0.0),
        origin=(0.0, 0.0),
    )
    return affinity.translate(
        geometry,
        xoff=float(getattr(footprint, "at_x", 0.0) or 0.0),
        yoff=float(getattr(footprint, "at_y", 0.0) or 0.0),
    )


def _custom_pad_geometry(
    pad: Any,
    cx: float,
    cy: float,
    size_x: float,
    size_y: float,
) -> BaseGeometry:
    parts: list[BaseGeometry] = []
    anchor = str(getattr(getattr(pad, "custom_options", None), "anchor", "") or "")
    if anchor == "circle":
        parts.append(Point(cx, cy).buffer(min(size_x, size_y) / 2.0, quad_segs=64))
    else:
        parts.append(_polygon(pad._to_rect_polygon(cx, cy)))  # noqa: SLF001
    for primitive in getattr(pad, "custom_primitives", ()) or ():
        if getattr(primitive, "primitive_type", "") != "gr_poly":
            continue
        points = _custom_pad_primitive_points(pad, primitive, cx, cy)
        part = _polygon(points)
        width = abs(float(getattr(primitive, "width", 0.0) or 0.0))
        if width:
            part = part.buffer(width / 2.0, quad_segs=16)
        if not part.is_empty:
            parts.append(part)
    return unary_union(parts)


def _custom_pad_primitive_points(
    pad: Any,
    primitive: Any,
    cx: float,
    cy: float,
) -> list[tuple[float, float]]:
    from .kicad_geometry import rotate_point

    angle = -float(getattr(pad, "at_angle", 0.0) or 0.0)
    points: list[tuple[float, float]] = []
    for x, y in getattr(primitive, "points", ()) or ():
        if angle:
            x, y = rotate_point(x, y, angle)
        points.append((x + cx, y + cy))
    return points


def _default_pad_geometry(
    pad: Any,
    cx: float,
    cy: float,
    size_x: float,
    size_y: float,
) -> BaseGeometry:
    points = (
        pad._to_rect_polygon(cx, cy)  # noqa: SLF001
        if hasattr(pad, "_to_rect_polygon")
        else box(
            cx - size_x / 2.0,
            cy - size_y / 2.0,
            cx + size_x / 2.0,
            cy + size_y / 2.0,
        )
    )
    return points if isinstance(points, BaseGeometry) else _polygon(points)


def _place_pad_geometry(
    geometry: BaseGeometry,
    footprint: Any,
    pad: Any,
    cx: float,
    cy: float,
) -> BaseGeometry:
    # Board files retain a pad's absolute orientation even though its position
    # is footprint-local. Undo the footprint rotation around the local pad
    # center before placing the whole geometry on the board.
    footprint_angle = float(getattr(footprint, "at_angle", 0.0) or 0.0)
    if footprint_angle:
        geometry = affinity.rotate(geometry, footprint_angle, origin=(cx, cy))
    geometry = _transform_footprint_geometry(geometry, footprint)

    # KiCad's drill offset moves the copper shape; the physical hole remains
    # at the pad anchor.
    offset_x = float(getattr(pad, "drill_offset_x", 0.0) or 0.0)
    offset_y = float(getattr(pad, "drill_offset_y", 0.0) or 0.0)
    if offset_x or offset_y:
        from .kicad_geometry import rotate_point

        offset_x, offset_y = rotate_point(
            offset_x,
            offset_y,
            -float(getattr(pad, "at_angle", 0.0) or 0.0),
        )
        geometry = affinity.translate(geometry, xoff=offset_x, yoff=offset_y)
    return geometry


def _pad_geometry(footprint: Any, pad: Any) -> BaseGeometry:
    cx = float(getattr(pad, "at_x", 0.0) or 0.0)
    cy = float(getattr(pad, "at_y", 0.0) or 0.0)
    size_x = abs(float(getattr(pad, "size_x", 0.0) or 0.0))
    size_y = abs(float(getattr(pad, "size_y", 0.0) or 0.0))
    shape = str(getattr(getattr(pad, "shape", ""), "value", getattr(pad, "shape", "")))

    if shape == "circle":
        geometry = Point(cx, cy).buffer(size_x / 2.0, quad_segs=64)
    elif shape == "oval":
        start, end, width = pad._to_oval_segment(cx, cy)  # noqa: SLF001
        geometry = LineString((start, end)).buffer(abs(width) / 2.0, quad_segs=64)
    elif shape == "trapezoid":
        geometry = _polygon(pad._to_trapezoid_polygon(cx, cy))  # noqa: SLF001
    elif shape == "roundrect":
        geometry = _polygon(pad._to_roundrect_polygon(cx, cy))  # noqa: SLF001
    elif shape == "custom":
        geometry = _custom_pad_geometry(pad, cx, cy, size_x, size_y)
    else:
        geometry = _default_pad_geometry(pad, cx, cy, size_x, size_y)
    return _place_pad_geometry(geometry, footprint, pad, cx, cy)


def _pad_hole_geometry(footprint: Any, pad: Any) -> BaseGeometry:
    center = _footprint_pad_position(footprint, pad)
    angle = -math.radians(float(getattr(pad, "at_angle", 0.0) or 0.0))
    if bool(getattr(pad, "drill_oval", False)):
        width = abs(float(getattr(pad, "drill_width", 0.0) or 0.0))
        height = abs(float(getattr(pad, "drill_height", 0.0) or 0.0))
        if width <= 0.0 or height <= 0.0:
            return Polygon()
        if width >= height:
            half = (width - height) / 2.0
            start, end, diameter = (-half, 0.0), (half, 0.0), height
        else:
            half = (height - width) / 2.0
            start, end, diameter = (0.0, -half), (0.0, half), width
        cos_a, sin_a = math.cos(angle), math.sin(angle)
        placed = [
            (
                center[0] + x * cos_a - y * sin_a,
                center[1] + x * sin_a + y * cos_a,
            )
            for x, y in (start, end)
        ]
        return LineString(placed).buffer(diameter / 2.0, quad_segs=64)
    diameter = abs(float(getattr(pad, "drill", 0.0) or 0.0))
    return Point(center).buffer(diameter / 2.0, quad_segs=64) if diameter else Polygon()


def _via_geometry(via: Any) -> BaseGeometry:
    return Point(float(via.at_x), float(via.at_y)).buffer(
        abs(float(getattr(via, "size", 0.0) or 0.0)) / 2.0,
        quad_segs=64,
    )


def _via_hole_geometry(via: Any) -> BaseGeometry:
    diameter = abs(float(getattr(via, "drill", 0.0) or 0.0))
    return Point(float(via.at_x), float(via.at_y)).buffer(
        diameter / 2.0,
        quad_segs=64,
    )


def _routing_geometry(item: Any) -> BaseGeometry:
    if hasattr(item, "mid_x"):
        polygons = item._to_poly().outlines  # noqa: SLF001
        return unary_union([_polygon(points) for points in polygons])
    width = abs(float(getattr(item, "width", 0.0) or 0.0))
    return LineString(((item.start_x, item.start_y), (item.end_x, item.end_y))).buffer(
        width / 2.0,
        quad_segs=32,
    )


def _physical_copper_layers(item: Any, copper_stack: Sequence[str]) -> tuple[str, ...]:
    layers = tuple(str(layer) for layer in getattr(item, "layers", ()) or ())
    return (
        copper_span_layers(layers, copper_stack)
        if _is_via(item)
        else pad_copper_layers(layers, copper_stack)
    )


@dataclass(frozen=True)
class _GeometryCandidate:
    geometry: BaseGeometry
    bounds: _GeometryBounds | None


@dataclass(frozen=True)
class _ConnectedItem:
    source: Any
    full_geometry: BaseGeometry
    full_bounds: _GeometryBounds | None
    hole_geometry: BaseGeometry
    hole_bounds: _GeometryBounds | None
    copper_layers: tuple[str, ...]
    conditional_layers: frozenset[str]
    footprint_owner: int | None
    is_pad: bool
    conditional_pad: bool


def _geometry_bounds(geometry: BaseGeometry) -> _GeometryBounds | None:
    if geometry.is_empty:
        return None
    minimum_x, minimum_y, maximum_x, maximum_y = geometry.bounds
    return (minimum_x, minimum_y, maximum_x, maximum_y)


def _bounds_overlap(
    left: _GeometryBounds | None,
    right: _GeometryBounds | None,
) -> bool:
    if left is None or right is None:
        return False
    return not (
        left[2] + _GEOMETRY_TOLERANCE_MM < right[0]
        or right[2] + _GEOMETRY_TOLERANCE_MM < left[0]
        or left[3] + _GEOMETRY_TOLERANCE_MM < right[1]
        or right[3] + _GEOMETRY_TOLERANCE_MM < left[1]
    )


def _conditional_layers(item: Any, physical_layers: Sequence[str]) -> frozenset[str]:
    if not _is_via(item) and not _pad_policy_applies(item):
        return frozenset()
    if _is_via(item) and bool(getattr(item, "start_end_only", False)):
        ends = {physical_layers[0], physical_layers[-1]} if physical_layers else set()
        return frozenset(layer for layer in physical_layers if layer not in ends)
    if not bool(getattr(item, "remove_unused_layers", False)):
        return frozenset()
    retained_ends = (
        {physical_layers[0], physical_layers[-1]}
        if _is_via(item)
        and physical_layers
        and bool(getattr(item, "keep_end_layers", False))
        else {
            layer
            for layer in ("F.Cu", "B.Cu")
            if layer in physical_layers
            and bool(getattr(item, "keep_end_layers", False))
        }
    )
    return frozenset(layer for layer in physical_layers if layer not in retained_ends)


@dataclass(frozen=True)
class PcbLayerFlashResolver:
    """Board-scoped direct-connectivity evidence for PTH flash resolution.

    KiCad's connectivity query considers traces, arcs, vias, and pads, while
    zone connections are supplied by ``zone_layer_connections``.  The source
    model supplies the copper shapes needed for local collision checks without
    conflating an entire net with a direct connection. KiCad's same-footprint
    and conditional-pad exclusions are applied to coincident pad connectivity.
    """

    copper_stack: tuple[str, ...]
    _routing_shapes: dict[
        tuple[tuple[str, object], str], tuple[_GeometryCandidate, ...]
    ]
    _items_by_net: dict[tuple[str, object], tuple[_ConnectedItem, ...]]

    @classmethod
    def from_board(cls, board: Any) -> "PcbLayerFlashResolver":
        stack = copper_layer_stack(board)
        routing: dict[tuple[tuple[str, object], str], list[_GeometryCandidate]] = (
            defaultdict(list)
        )
        for item in tuple(getattr(board, "segments", ()) or ()) + tuple(
            getattr(board, "arcs", ()) or ()
        ):
            net = _net_key(item)
            layer = str(getattr(item, "layer", "") or "")
            if net is None or not layer.endswith(".Cu"):
                continue
            geometry = _routing_geometry(item)
            if not geometry.is_empty:
                routing[(net, layer)].append(
                    _GeometryCandidate(geometry, _geometry_bounds(geometry))
                )

        items_by_net: dict[tuple[str, object], list[_ConnectedItem]] = defaultdict(list)
        for via in getattr(board, "vias", ()) or ():
            if (net := _net_key(via)) is not None:
                physical_layers = copper_span_layers(via.layers, stack)
                full_geometry = _via_geometry(via)
                hole_geometry = _via_hole_geometry(via)
                items_by_net[net].append(
                    _ConnectedItem(
                        source=via,
                        full_geometry=full_geometry,
                        full_bounds=_geometry_bounds(full_geometry),
                        hole_geometry=hole_geometry,
                        hole_bounds=_geometry_bounds(hole_geometry),
                        copper_layers=physical_layers,
                        conditional_layers=_conditional_layers(via, physical_layers),
                        footprint_owner=None,
                        is_pad=False,
                        conditional_pad=False,
                    )
                )
        for footprint in getattr(board, "footprints", ()) or ():
            for pad in getattr(footprint, "pads", ()) or ():
                if (net := _net_key(pad)) is None:
                    continue
                physical_layers = pad_copper_layers(pad.layers, stack)
                full_geometry = _pad_geometry(footprint, pad)
                hole_geometry = _pad_hole_geometry(footprint, pad)
                items_by_net[net].append(
                    _ConnectedItem(
                        source=pad,
                        full_geometry=full_geometry,
                        full_bounds=_geometry_bounds(full_geometry),
                        hole_geometry=hole_geometry,
                        hole_bounds=_geometry_bounds(hole_geometry),
                        copper_layers=physical_layers,
                        conditional_layers=_conditional_layers(pad, physical_layers),
                        footprint_owner=id(footprint),
                        is_pad=True,
                        conditional_pad=_pad_policy_applies(pad)
                        and bool(getattr(pad, "remove_unused_layers", False)),
                    )
                )

        return cls(
            copper_stack=stack,
            _routing_shapes={key: tuple(value) for key, value in routing.items()},
            _items_by_net={key: tuple(value) for key, value in items_by_net.items()},
        )

    def _connected_layers(
        self,
        item: Any,
        geometry: BaseGeometry,
        *,
        footprint_owner: int | None = None,
    ) -> set[str]:
        net = _net_key(item)
        if net is None:
            return set()
        physical_layers = _physical_copper_layers(item, self.copper_stack)
        target_bounds = _geometry_bounds(geometry)
        connected = {
            layer
            for layer in physical_layers
            if any(
                _bounds_overlap(target_bounds, route.bounds)
                and geometry.distance(route.geometry) <= _GEOMETRY_TOLERANCE_MM
                for route in self._routing_shapes.get((net, layer), ())
            )
        }
        conditional_pad = (
            not _is_via(item)
            and _pad_policy_applies(item)
            and bool(getattr(item, "remove_unused_layers", False))
        )
        for other in self._items_by_net.get(net, ()):
            if other.source is item:
                continue
            if footprint_owner is not None and other.footprint_owner == footprint_owner:
                continue
            if conditional_pad and other.is_pad and other.conditional_pad:
                continue
            for layer in physical_layers:
                if layer not in other.copper_layers:
                    continue
                other_geometry = (
                    other.hole_geometry
                    if layer in other.conditional_layers
                    else other.full_geometry
                )
                other_bounds = (
                    other.hole_bounds
                    if layer in other.conditional_layers
                    else other.full_bounds
                )
                if _bounds_overlap(target_bounds, other_bounds) and (
                    geometry.distance(other_geometry) <= _GEOMETRY_TOLERANCE_MM
                ):
                    connected.add(layer)
        return connected

    def via_flash_layers(self, via: Any) -> tuple[str, ...]:
        if not self.copper_stack:
            return tuple(str(layer) for layer in getattr(via, "layers", ()) or ())
        connected = (
            self._connected_layers(via, _via_hole_geometry(via))
            if bool(getattr(via, "remove_unused_layers", False))
            and not bool(getattr(via, "start_end_only", False))
            else ()
        )
        return effective_copper_flash_layers(
            via,
            self.copper_stack,
            connected_layers=connected,
        )

    def via_item_layers(self, via: Any) -> tuple[str, ...]:
        if not self.copper_stack:
            return tuple(str(layer) for layer in getattr(via, "layers", ()) or ())
        connected = (
            self._connected_layers(via, _via_hole_geometry(via))
            if bool(getattr(via, "remove_unused_layers", False))
            and not bool(getattr(via, "start_end_only", False))
            else ()
        )
        return effective_item_layers(
            via,
            self.copper_stack,
            connected_layers=connected,
        )

    def pad_item_layers(self, pad: Any, footprint: Any) -> tuple[str, ...]:
        if not self.copper_stack:
            return tuple(str(layer) for layer in getattr(pad, "layers", ()) or ())
        connected = (
            self._connected_layers(
                pad,
                _pad_hole_geometry(footprint, pad),
                footprint_owner=id(footprint),
            )
            if _pad_policy_applies(pad)
            and bool(getattr(pad, "remove_unused_layers", False))
            else ()
        )
        return effective_item_layers(
            pad,
            self.copper_stack,
            connected_layers=connected,
        )


__all__ = [
    "PcbLayerFlashResolver",
    "copper_layer_stack",
    "copper_span_layers",
    "pad_copper_layers",
    "effective_copper_flash_layers",
    "effective_item_layers",
]

"""KiCad-style source geometry bounds for PCB graphical shapes."""

from __future__ import annotations

import math
from collections.abc import Sequence

from .kicad_geometry import BoundingBox
from .kicad_pcb_polygon_ops import (
    DEFAULT_ERROR_MM,
    _calc_arc_center,
    _normalize_angle,
    bezier_to_polyline,
)

Point = tuple[float, float]

_ANGLE_EPS = 1e-10


def _stroke_margin(width: float) -> float:
    return max(0.0, width) / 2.0


def _bounds_from_points(points: Sequence[Point], width: float = 0.0) -> BoundingBox:
    bbox = BoundingBox()
    for point in points:
        bbox.expand(point)

    if not bbox.is_valid():
        return bbox

    margin = _stroke_margin(width)
    if margin == 0.0:
        return bbox
    return bbox.expand_by(margin)


def segment_bounds(start: Point, end: Point, width: float = 0.0) -> BoundingBox:
    """Return KiCad ``SHAPE_T::SEGMENT`` bounds."""
    return _bounds_from_points([start, end], width)


def rect_bounds(start: Point, end: Point, width: float = 0.0) -> BoundingBox:
    """Return KiCad ``SHAPE_T::RECTANGLE`` bounds."""
    min_x = min(start[0], end[0])
    min_y = min(start[1], end[1])
    max_x = max(start[0], end[0])
    max_y = max(start[1], end[1])
    return _bounds_from_points(
        [
            (min_x, min_y),
            (max_x, min_y),
            (max_x, max_y),
            (min_x, max_y),
        ],
        width,
    )


def circle_bounds(center: Point, end: Point, width: float = 0.0) -> BoundingBox:
    """Return KiCad ``SHAPE_T::CIRCLE`` bounds."""
    radius = math.hypot(end[0] - center[0], end[1] - center[1])
    radius += _stroke_margin(width)
    return BoundingBox(
        min_x=center[0] - radius,
        min_y=center[1] - radius,
        max_x=center[0] + radius,
        max_y=center[1] + radius,
    )


def _arc_sweep(start_angle: float, mid_angle: float, end_angle: float) -> float:
    sweep = _normalize_angle(end_angle - start_angle)
    mid_offset = _normalize_angle(mid_angle - start_angle)

    if sweep >= 0:
        if mid_offset < 0 or mid_offset > sweep:
            sweep -= 2 * math.pi
    elif mid_offset > 0 or mid_offset < sweep:
        sweep += 2 * math.pi

    return sweep


def _angle_on_sweep(angle: float, start_angle: float, sweep: float) -> bool:
    offset = _normalize_angle(angle - start_angle)

    if sweep >= 0:
        if offset < -_ANGLE_EPS:
            offset += 2 * math.pi
        return -_ANGLE_EPS <= offset <= sweep + _ANGLE_EPS

    if offset > _ANGLE_EPS:
        offset -= 2 * math.pi
    return sweep - _ANGLE_EPS <= offset <= _ANGLE_EPS


def arc_bounds(
    start: Point,
    mid: Point,
    end: Point,
    width: float = 0.0,
) -> BoundingBox:
    """Return KiCad ``SHAPE_T::ARC`` bounds for an unfilled arc."""
    center, radius = _calc_arc_center(start, mid, end)
    if center is None or radius <= 0:
        return segment_bounds(start, end, width)

    start_angle = math.atan2(start[1] - center[1], start[0] - center[0])
    mid_angle = math.atan2(mid[1] - center[1], mid[0] - center[0])
    end_angle = math.atan2(end[1] - center[1], end[0] - center[0])
    sweep = _arc_sweep(start_angle, mid_angle, end_angle)

    points: list[Point] = [start, end]
    for angle in (0.0, math.pi / 2, math.pi, 3 * math.pi / 2):
        if _angle_on_sweep(angle, start_angle, sweep):
            points.append(
                (
                    center[0] + radius * math.cos(angle),
                    center[1] + radius * math.sin(angle),
                )
            )

    return _bounds_from_points(points, width)


def poly_bounds(points: Sequence[Point], width: float = 0.0) -> BoundingBox:
    """Return KiCad ``SHAPE_T::POLY`` bounds for xy-only polygon points."""
    return _bounds_from_points(points, width)


def bezier_bounds(
    points: Sequence[Point],
    width: float = 0.0,
    error: float = DEFAULT_ERROR_MM,
) -> BoundingBox:
    """Return KiCad-style ``SHAPE_T::BEZIER`` approximated-point bounds."""
    if len(points) != 4:
        return _bounds_from_points(points, width)

    polyline = bezier_to_polyline(points[0], points[1], points[2], points[3], error)
    if not polyline:
        polyline = list(points)
    return _bounds_from_points(polyline, width)

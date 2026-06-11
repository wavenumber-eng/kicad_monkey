"""Geometer-backed STEP projection helpers for KiCad assembly overlays."""

from __future__ import annotations

import logging
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Literal

log = logging.getLogger(__name__)

_ProjectionSide = Literal["top", "bottom"]
_CurveMode = Literal["native_arcs", "polyline"]
_CacheKey = tuple[object, ...]


@dataclass(frozen=True)
class _AssemblyProjectionOptions:
    side: _ProjectionSide
    projection_algorithm: str | None = None
    outline_algorithm: str = "mesh-shadow"
    curve_mode: _CurveMode = "native_arcs"
    samples_per_curve: int = 24
    round_digits: int = 3
    include_visible: bool = True
    include_outline: bool = True
    union_polygons: bool = True
    mesh_linear_deflection: float | None = None
    mesh_angular_deflection: float | None = None
    mesh_relative: bool | None = None
    hlr_angle_tolerance: float | None = None
    edge_flags: Mapping[str, bool] | None = None


@dataclass(frozen=True)
class _AssemblyProjectedArc:
    start: tuple[float, float]
    end: tuple[float, float]
    center: tuple[float, float]
    radius: float
    extent_rad: float
    ccw: bool
    full_circle: bool


@dataclass(frozen=True)
class _AssemblyProjectedGeometry:
    outline_line_segments: tuple[tuple[tuple[float, float], tuple[float, float]], ...]
    outline_arcs: tuple[_AssemblyProjectedArc, ...]
    detail_line_segments: tuple[tuple[tuple[float, float], tuple[float, float]], ...]
    detail_arcs: tuple[_AssemblyProjectedArc, ...]

    @property
    def is_empty(self) -> bool:
        return (
            not self.outline_line_segments
            and not self.outline_arcs
            and not self.detail_line_segments
            and not self.detail_arcs
        )


class _AssemblyProjectionCache:
    """Caches Geometer HLR results for repeated component model projections."""

    def __init__(self) -> None:
        self._projection_by_key: dict[_CacheKey, _AssemblyProjectedGeometry] = {}

    def build_cache_key(
        self,
        *,
        model_hash: str,
        pose_signature: tuple[float, ...],
        options: _AssemblyProjectionOptions,
    ) -> _CacheKey:
        return (
            str(model_hash),
            str(options.side),
            str(options.projection_algorithm or ""),
            str(options.outline_algorithm or ""),
            str(options.curve_mode),
            int(max(2, options.samples_per_curve)),
            int(max(0, options.round_digits)),
            bool(options.include_visible),
            bool(options.include_outline),
            bool(options.union_polygons),
            None
            if options.mesh_linear_deflection is None
            else float(options.mesh_linear_deflection),
            None
            if options.mesh_angular_deflection is None
            else float(options.mesh_angular_deflection),
            None if options.mesh_relative is None else bool(options.mesh_relative),
            None if options.hlr_angle_tolerance is None else float(options.hlr_angle_tolerance),
            tuple(sorted((str(k), bool(v)) for k, v in (options.edge_flags or {}).items())),
            tuple(float(value) for value in pose_signature),
        )

    def project(
        self,
        *,
        model_hash: str,
        step_bytes: bytes,
        pose_signature: tuple[float, ...],
        transform_matrix: object,
        options: _AssemblyProjectionOptions,
        model_label: str | None = None,
    ) -> tuple[_CacheKey, _AssemblyProjectedGeometry]:
        cache_key = self.build_cache_key(
            model_hash=model_hash,
            pose_signature=pose_signature,
            options=options,
        )
        cached = self._projection_by_key.get(cache_key)
        if cached is not None:
            return cache_key, cached

        label = str(model_label or "").strip() or f"hash:{str(model_hash)[:12]}"
        log.info(
            "Computing Geometer HLR STEP projection: %s (hash=%s, side=%s)",
            label,
            str(model_hash)[:12],
            str(options.side),
        )
        projected = self._project_with_geometer(
            step_bytes=bytes(step_bytes),
            transform_matrix=transform_matrix,
            options=options,
        )
        self._projection_by_key[cache_key] = projected
        return cache_key, projected

    def _project_with_geometer(
        self,
        *,
        step_bytes: bytes,
        transform_matrix: object,
        options: _AssemblyProjectionOptions,
    ) -> _AssemblyProjectedGeometry:
        try:
            import geometer
        except Exception as exc:  # pragma: no cover - dependency failure path
            raise RuntimeError(
                "The geometer Python package is required for KiCad Cruncher "
                "assembly STEP projection."
            ) from exc

        side = str(options.side).strip().lower()
        if side == "bottom":
            view_id = "bottom"
            direction = [0.0, 0.0, -1.0]
            projection_y_direction = [0.0, 1.0, 0.0]
        else:
            view_id = "top"
            direction = [0.0, 0.0, 1.0]
            projection_y_direction = [0.0, 1.0, 0.0]

        round_digits = int(max(0, options.round_digits))
        curve_mode = str(options.curve_mode).strip().lower()
        if curve_mode not in {"native_arcs", "polyline"}:
            curve_mode = "native_arcs"

        result = geometer.project_step_hlr(
            step_bytes,
            views=[
                {
                    "id": view_id,
                    "direction": direction,
                    "up": projection_y_direction,
                }
            ],
            model_transform=_matrix4_for_geometer(transform_matrix),
            options=self._hlr_options_for_geometer(
                options,
                curve_mode=curve_mode,
                round_digits=round_digits,
            ),
        )
        outline = _mapping_from_object(result.geometry(view_id, "outline"))
        detail = _mapping_from_object(result.geometry(view_id, "detail"))
        projected = _AssemblyProjectedGeometry(
            outline_line_segments=self._dedupe_segments(
                list(_segments_from_mode(outline)),
                round_digits=round_digits,
            ),
            outline_arcs=self._dedupe_arcs(
                list(_arcs_from_mode(outline)),
                round_digits=round_digits,
            ),
            detail_line_segments=self._dedupe_segments(
                list(_segments_from_mode(detail)),
                round_digits=round_digits,
            ),
            detail_arcs=self._dedupe_arcs(list(_arcs_from_mode(detail)), round_digits=round_digits),
        )
        return _normalize_projected_geometry(projected, flip_x=side == "bottom")

    def _hlr_options_for_geometer(
        self,
        options: _AssemblyProjectionOptions,
        *,
        curve_mode: str,
        round_digits: int,
    ) -> dict[str, object]:
        hlr_options: dict[str, object] = {
            "curve_mode": curve_mode,
            "samples_per_curve": int(max(2, options.samples_per_curve)),
            "round_digits": round_digits,
            "include_visible": bool(options.include_visible),
            "include_outline": bool(options.include_outline),
            "union_outline_polygons": bool(options.union_polygons),
            "outline_algorithm": str(options.outline_algorithm or "mesh-shadow"),
        }
        if options.projection_algorithm:
            hlr_options["projection_algorithm"] = str(options.projection_algorithm)
        if options.mesh_linear_deflection is not None:
            hlr_options["mesh_linear_deflection"] = float(options.mesh_linear_deflection)
        if options.mesh_angular_deflection is not None:
            hlr_options["mesh_angular_deflection"] = float(options.mesh_angular_deflection)
        if options.mesh_relative is not None:
            hlr_options["mesh_relative"] = bool(options.mesh_relative)
        if options.hlr_angle_tolerance is not None:
            hlr_options["hlr_angle_tolerance"] = float(options.hlr_angle_tolerance)
        hlr_options.update({key: bool(value) for key, value in (options.edge_flags or {}).items()})
        return hlr_options

    def _dedupe_segments(
        self,
        segments: list[tuple[tuple[float, float], tuple[float, float]]],
        *,
        round_digits: int,
    ) -> tuple[tuple[tuple[float, float], tuple[float, float]], ...]:
        deduped: list[tuple[tuple[float, float], tuple[float, float]]] = []
        seen: set[tuple[float, float, float, float]] = set()
        for (x1, y1), (x2, y2) in segments:
            rx1 = round(float(x1), round_digits)
            ry1 = round(float(y1), round_digits)
            rx2 = round(float(x2), round_digits)
            ry2 = round(float(y2), round_digits)
            if rx1 == rx2 and ry1 == ry2:
                continue
            if rx1 > rx2 or (rx1 == rx2 and ry1 > ry2):
                rx1, ry1, rx2, ry2 = rx2, ry2, rx1, ry1
            key = (rx1, ry1, rx2, ry2)
            if key in seen:
                continue
            seen.add(key)
            deduped.append(((rx1, ry1), (rx2, ry2)))
        return tuple(deduped)

    def _dedupe_arcs(
        self,
        arcs: list[_AssemblyProjectedArc],
        *,
        round_digits: int,
    ) -> tuple[_AssemblyProjectedArc, ...]:
        deduped: list[_AssemblyProjectedArc] = []
        seen: set[_CacheKey] = set()
        for arc in arcs:
            start = (
                round(float(arc.start[0]), round_digits),
                round(float(arc.start[1]), round_digits),
            )
            end = (
                round(float(arc.end[0]), round_digits),
                round(float(arc.end[1]), round_digits),
            )
            center = (
                round(float(arc.center[0]), round_digits),
                round(float(arc.center[1]), round_digits),
            )
            radius = round(float(arc.radius), round_digits)
            if arc.full_circle:
                key = ("full", center[0], center[1], radius)
            else:
                key = (
                    "arc",
                    start[0],
                    start[1],
                    end[0],
                    end[1],
                    center[0],
                    center[1],
                    radius,
                    round(float(arc.extent_rad), max(round_digits, 3)),
                    bool(arc.ccw),
                )
            if key in seen:
                continue
            seen.add(key)
            deduped.append(
                _AssemblyProjectedArc(
                    start=start,
                    end=end,
                    center=center,
                    radius=radius,
                    extent_rad=float(arc.extent_rad),
                    ccw=bool(arc.ccw),
                    full_circle=bool(arc.full_circle),
                )
            )
        return tuple(deduped)


def _matrix4_for_geometer(matrix: object) -> list[list[float]]:
    to_list = getattr(matrix, "tolist", None)
    if callable(to_list):
        matrix = to_list()
    values = _sequence_values(matrix)
    if values is None:
        raise ValueError("transform_matrix must be a 4x4 matrix or flat 16-value sequence")
    if len(values) == 16 and not _is_nested_sequence(values):
        flat = [_float_value(value) for value in values]
        return [flat[idx : idx + 4] for idx in range(0, 16, 4)]
    if len(values) != 4:
        raise ValueError("transform_matrix must be a 4x4 matrix or flat 16-value sequence")
    rows: list[list[float]] = []
    for row in values:
        row_values = _sequence_values(row)
        if row_values is None or len(row_values) != 4:
            raise ValueError("transform_matrix rows must contain 4 values")
        rows.append([_float_value(value) for value in row_values])
    return rows


def _float_value(raw: object) -> float:
    if not isinstance(raw, int | float | str):
        raise TypeError(f"Expected numeric value, got {type(raw).__name__}")
    return float(raw)


def _sequence_values(raw: object) -> list[object] | None:
    if not isinstance(raw, Sequence) or isinstance(raw, str | bytes | bytearray):
        return None
    return list(raw)


def _is_nested_sequence(values: Sequence[object]) -> bool:
    return any(
        isinstance(value, Sequence) and not isinstance(value, str | bytes | bytearray)
        for value in values
    )


def _mapping_from_object(raw: object) -> Mapping[str, object]:
    if isinstance(raw, Mapping):
        return raw
    return {}


def _segments_from_mode(
    mode: Mapping[str, object],
) -> tuple[tuple[tuple[float, float], tuple[float, float]], ...]:
    segments: list[tuple[tuple[float, float], tuple[float, float]]] = []
    for raw in _sequence_values(mode.get("segments")) or []:
        parsed = _segment_from_json(raw)
        if parsed is not None:
            segments.append(parsed)
    return tuple(segments)


def _segment_from_json(
    raw: object,
) -> tuple[tuple[float, float], tuple[float, float]] | None:
    if isinstance(raw, Mapping):
        start = _point2(raw.get("start"))
        end = _point2(raw.get("end"))
        if start is None or end is None:
            return None
        return start, end
    values = _sequence_values(raw)
    if values is None:
        return None
    try:
        if len(values) == 4:
            return (
                (_float_value(values[0]), _float_value(values[1])),
                (_float_value(values[2]), _float_value(values[3])),
            )
        if len(values) == 2:
            start = _point2(values[0])
            end = _point2(values[1])
            if start is None or end is None:
                return None
            return start, end
    except (TypeError, ValueError):
        return None
    return None


def _arcs_from_mode(mode: Mapping[str, object]) -> tuple[_AssemblyProjectedArc, ...]:
    arcs: list[_AssemblyProjectedArc] = []
    for raw in _sequence_values(mode.get("arcs")) or []:
        parsed = _arc_from_json(raw)
        if parsed is not None:
            arcs.append(parsed)
    return tuple(arcs)


def _normalize_projected_geometry(
    geometry: _AssemblyProjectedGeometry,
    *,
    flip_x: bool,
) -> _AssemblyProjectedGeometry:
    if not flip_x:
        return geometry
    return _AssemblyProjectedGeometry(
        outline_line_segments=tuple(
            (_flip_point_x(start), _flip_point_x(end))
            for start, end in geometry.outline_line_segments
        ),
        outline_arcs=tuple(_flip_arc_x(arc) for arc in geometry.outline_arcs),
        detail_line_segments=tuple(
            (_flip_point_x(start), _flip_point_x(end))
            for start, end in geometry.detail_line_segments
        ),
        detail_arcs=tuple(_flip_arc_x(arc) for arc in geometry.detail_arcs),
    )


def _flip_point_x(point: tuple[float, float]) -> tuple[float, float]:
    return -float(point[0]), float(point[1])


def _flip_arc_x(arc: _AssemblyProjectedArc) -> _AssemblyProjectedArc:
    return _AssemblyProjectedArc(
        start=_flip_point_x(arc.start),
        end=_flip_point_x(arc.end),
        center=_flip_point_x(arc.center),
        radius=arc.radius,
        extent_rad=arc.extent_rad,
        ccw=bool(arc.ccw) if arc.full_circle else not bool(arc.ccw),
        full_circle=arc.full_circle,
    )


def _arc_from_json(raw: object) -> _AssemblyProjectedArc | None:
    if not isinstance(raw, Mapping):
        return None
    start = _point2(raw.get("start"))
    end = _point2(raw.get("end"))
    center = _point2(raw.get("center"))
    if start is None or end is None or center is None:
        return None
    try:
        raw_radius = raw.get("radius")
        raw_extent_rad = raw.get("extent_rad")
        if raw_radius is None or raw_extent_rad is None:
            return None
        radius = float(raw_radius)
        extent_rad = float(raw_extent_rad)
    except (TypeError, ValueError):
        return None
    return _AssemblyProjectedArc(
        start=start,
        end=end,
        center=center,
        radius=radius,
        extent_rad=extent_rad,
        ccw=bool(raw.get("ccw", True)),
        full_circle=bool(raw.get("full_circle", False)),
    )


def _point2(raw: object) -> tuple[float, float] | None:
    values = _sequence_values(raw)
    if values is None:
        return None
    if len(values) < 2:
        return None
    try:
        return _float_value(values[0]), _float_value(values[1])
    except (TypeError, ValueError):
        return None


_GLOBAL_ASSEMBLY_PROJECTION_CACHE = _AssemblyProjectionCache()


def _get_assembly_projection_cache() -> _AssemblyProjectionCache:
    return _GLOBAL_ASSEMBLY_PROJECTION_CACHE


__all__ = [
    "_AssemblyProjectedArc",
    "_AssemblyProjectedGeometry",
    "_AssemblyProjectionOptions",
    "_get_assembly_projection_cache",
]

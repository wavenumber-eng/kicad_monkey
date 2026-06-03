"""KiCad PCB 3D model pose helpers for assembly SVG projection."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from kicad_monkey.kicad_model import Model
    from kicad_monkey.kicad_pcb import KiCadPcb
    from kicad_monkey.kicad_pcb_footprint import Footprint

Matrix4 = list[list[float]]

_BOARD_MODEL_Z_OFFSET_MM = 0.05


class _BoundsLike(Protocol):
    min_x: float
    min_y: float


@dataclass(frozen=True, slots=True)
class KiCadModelPose:
    """Resolved model-to-KiCad-3D-world transform for one footprint model."""

    matrix: Matrix4
    side: str
    board_thickness_mm: float

    @property
    def signature(self) -> tuple[float, ...]:
        return tuple(round(value, 9) for row in self.matrix for value in row)


def kicad_model_pose(
    pcb: KiCadPcb,
    footprint: Footprint,
    model: Model,
) -> KiCadModelPose:
    """Return the KiCad STEP-exporter style model transform matrix.

    The target coordinate system is KiCad's 3D/STEP world in millimeters:
    board X is preserved, board Y is negated, and model rotations are applied
    in KiCad's negative Z, negative Y, negative X order after the model offset.
    """

    side = "bottom" if str(getattr(footprint, "layer", "") or "").startswith("B.") else "top"
    board_thickness = _board_thickness_mm(pcb)
    offset = _vec3(getattr(model, "offset", (0.0, 0.0, 0.0)), default=0.0)
    scale = _vec3(getattr(model, "scale", (1.0, 1.0, 1.0)), default=1.0)
    rotation = _vec3(getattr(model, "rotate", (0.0, 0.0, 0.0)), default=0.0)

    offset_z = offset[2] + _BOARD_MODEL_Z_OFFSET_MM
    if side == "bottom":
        offset_z += board_thickness / 2.0
    else:
        offset_z += board_thickness / 2.0

    matrix = _translation_matrix(
        float(getattr(footprint, "at_x", 0.0) or 0.0),
        -float(getattr(footprint, "at_y", 0.0) or 0.0),
        0.0,
    )
    matrix = _matrix_multiply(
        matrix,
        _rotation_z_matrix(math.radians(float(getattr(footprint, "at_angle", 0.0) or 0.0))),
    )
    if side == "bottom":
        matrix = _matrix_multiply(matrix, _rotation_x_matrix(math.pi))
    matrix = _matrix_multiply(matrix, _translation_matrix(offset[0], offset[1], offset_z))
    matrix = _matrix_multiply(matrix, _rotation_z_matrix(math.radians(-rotation[2])))
    matrix = _matrix_multiply(matrix, _rotation_y_matrix(math.radians(-rotation[1])))
    matrix = _matrix_multiply(matrix, _rotation_x_matrix(math.radians(-rotation[0])))
    matrix = _matrix_multiply(matrix, _scale_matrix(scale))

    return KiCadModelPose(
        matrix=matrix,
        side=side,
        board_thickness_mm=board_thickness,
    )


def board_world_to_svg(
    point: tuple[float, float],
    *,
    bbox: _BoundsLike,
) -> tuple[float, float]:
    """Map KiCad 3D world XY back onto the KiCad Monkey PCB SVG canvas."""

    world_x, world_y = float(point[0]), float(point[1])
    board_x = world_x
    board_y = -world_y
    return board_x - float(bbox.min_x), board_y - float(bbox.min_y)


def model_bounds_to_svg_rect(
    bounds: object,
    *,
    bbox: _BoundsLike,
) -> tuple[float, float, float, float] | None:
    """Project transformed Geometer model bounds onto the PCB SVG canvas."""

    min_values = _bounds_vec3(bounds, "min")
    max_values = _bounds_vec3(bounds, "max")
    if min_values is None or max_values is None:
        return None

    points = [
        board_world_to_svg((min_values[0], min_values[1]), bbox=bbox),
        board_world_to_svg((max_values[0], min_values[1]), bbox=bbox),
        board_world_to_svg((max_values[0], max_values[1]), bbox=bbox),
        board_world_to_svg((min_values[0], max_values[1]), bbox=bbox),
    ]
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    min_x = min(xs)
    min_y = min(ys)
    return min_x, min_y, max(xs) - min_x, max(ys) - min_y


def transform_footprint_local_to_board(
    footprint: Footprint,
    point: tuple[float, float],
) -> tuple[float, float]:
    """Transform a 2D footprint-local point to board coordinates."""

    angle = math.radians(-float(getattr(footprint, "at_angle", 0.0) or 0.0))
    cosine = math.cos(angle)
    sine = math.sin(angle)
    local_x, local_y = float(point[0]), float(point[1])
    return (
        float(getattr(footprint, "at_x", 0.0) or 0.0) + local_x * cosine - local_y * sine,
        float(getattr(footprint, "at_y", 0.0) or 0.0) + local_x * sine + local_y * cosine,
    )


def _board_thickness_mm(pcb: KiCadPcb) -> float:
    stackup = getattr(pcb, "stackup", None)
    get_board_thickness = getattr(stackup, "get_board_thickness", None)
    if callable(get_board_thickness):
        try:
            raw_value = get_board_thickness()
        except (TypeError, ValueError):
            pass
        else:
            if isinstance(raw_value, int | float | str):
                value = float(raw_value)
                if value > 0.0:
                    return value
    try:
        return float(getattr(pcb, "thickness", 1.6) or 1.6)
    except (TypeError, ValueError):
        return 1.6


def _vec3(raw: object, *, default: float) -> tuple[float, float, float]:
    try:
        values = tuple(float(value) for value in raw)  # type: ignore[operator]
    except (TypeError, ValueError):
        values = ()
    return (
        values[0] if len(values) > 0 else default,
        values[1] if len(values) > 1 else default,
        values[2] if len(values) > 2 else default,
    )


def _bounds_vec3(bounds: object, key: str) -> tuple[float, float, float] | None:
    values = bounds.get(key) if hasattr(bounds, "get") else None  # type: ignore[attr-defined]
    if not isinstance(values, list | tuple) or len(values) < 3:
        return None
    try:
        return float(values[0]), float(values[1]), float(values[2])
    except (TypeError, ValueError):
        return None


def _translation_matrix(x: float, y: float, z: float) -> Matrix4:
    return [
        [1.0, 0.0, 0.0, float(x)],
        [0.0, 1.0, 0.0, float(y)],
        [0.0, 0.0, 1.0, float(z)],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _scale_matrix(scale: tuple[float, float, float]) -> Matrix4:
    return [
        [scale[0], 0.0, 0.0, 0.0],
        [0.0, scale[1], 0.0, 0.0],
        [0.0, 0.0, scale[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _rotation_x_matrix(angle: float) -> Matrix4:
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cosine, -sine, 0.0],
        [0.0, sine, cosine, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _rotation_y_matrix(angle: float) -> Matrix4:
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return [
        [cosine, 0.0, sine, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sine, 0.0, cosine, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _rotation_z_matrix(angle: float) -> Matrix4:
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return [
        [cosine, -sine, 0.0, 0.0],
        [sine, cosine, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _matrix_multiply(left: Matrix4, right: Matrix4) -> Matrix4:
    return [
        [
            sum(left[row][idx] * right[idx][col] for idx in range(4))
            for col in range(4)
        ]
        for row in range(4)
    ]


__all__ = [
    "KiCadModelPose",
    "Matrix4",
    "board_world_to_svg",
    "kicad_model_pose",
    "model_bounds_to_svg_rect",
    "transform_footprint_local_to_board",
]

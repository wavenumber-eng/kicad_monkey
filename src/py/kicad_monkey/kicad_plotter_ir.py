"""
KiCad Plotter-call IR.

This module owns the canonical, JSON-serializable intermediate
representation that mirrors KiCad's own ``PLOTTER`` virtual-method
vocabulary (see ``include/plotters/plotter.h``). Every KiCad rendering
call -- ``Circle``, ``Arc``, ``BezierCurve``, ``Rect``, ``PlotPoly``,
``Text``, ``PenTo``, ``Flash*Pad`` family, plus state and lifecycle --
maps to a single :class:`KiCadPlotterOp` whose payload is a flat
JSON-safe dict.

Coordinates are KiCad internal units (nanometres, ``int``). Colours
are hex strings (``"#RRGGBB"`` or ``"#RRGGBBAA"``). Enums (``FILL_T``,
``LINE_STYLE``, ``GR_TEXT_*_ALIGN``) are stored by their KiCad name
strings so a future C++-side ``RECORDER_PLOTTER`` can serialise into
this schema by simple ``wxString::FromUTF8(EnumName)``.

This module is pure data: no rendering, no parser dependencies, no
external libraries. The flat-function SVG renderer that consumes it
lives in :mod:`kicad_monkey.kicad_sch_svg_renderer`.
"""

from __future__ import annotations

import copy
import json
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any


# =============================================================================
# Schema constant
# =============================================================================

KICAD_PLOTTER_IR_SCHEMA = "kicad.plotter_ir.a0"
KICAD_PLOTTER_IR_ACCEPTED_SCHEMAS = frozenset(
    {
        KICAD_PLOTTER_IR_SCHEMA,
        "kicad.plotter_ir.v1",
    }
)


# =============================================================================
# KiCad enum mirrors (string values match the C++ enum names)
# =============================================================================


class KiCadFillType(str, Enum):
    """Mirror of ``FILL_T`` in ``include/eda_shape.h``."""

    NO_FILL = "NO_FILL"
    FILLED_SHAPE = "FILLED_SHAPE"
    FILLED_WITH_BG_BODYCOLOR = "FILLED_WITH_BG_BODYCOLOR"
    FILLED_WITH_COLOR = "FILLED_WITH_COLOR"
    HATCH = "HATCH"
    REVERSE_HATCH = "REVERSE_HATCH"
    CROSS_HATCH = "CROSS_HATCH"


class KiCadLineStyle(str, Enum):
    """Mirror of ``LINE_STYLE`` in ``include/stroke_params.h``."""

    DEFAULT = "DEFAULT"
    SOLID = "SOLID"
    DASH = "DASH"
    DOT = "DOT"
    DASH_DOT = "DASH_DOT"
    DASH_DOT_DOT = "DASH_DOT_DOT"


class KiCadHorizAlign(str, Enum):
    """Mirror of ``GR_TEXT_H_ALIGN_T`` in ``include/eda_text.h``."""

    LEFT = "GR_TEXT_H_ALIGN_LEFT"
    CENTER = "GR_TEXT_H_ALIGN_CENTER"
    RIGHT = "GR_TEXT_H_ALIGN_RIGHT"
    INDETERMINATE = "GR_TEXT_H_ALIGN_INDETERMINATE"


class KiCadVertAlign(str, Enum):
    """Mirror of ``GR_TEXT_V_ALIGN_T`` in ``include/eda_text.h``."""

    TOP = "GR_TEXT_V_ALIGN_TOP"
    CENTER = "GR_TEXT_V_ALIGN_CENTER"
    BOTTOM = "GR_TEXT_V_ALIGN_BOTTOM"
    INDETERMINATE = "GR_TEXT_V_ALIGN_INDETERMINATE"


class KiCadPenAction(str, Enum):
    """Plume kind for :meth:`KiCadPlotterOp.pen_to` (``PenTo`` in C++)."""

    UP = "U"        # only moves the pen
    DOWN = "D"      # draw a line from current to target
    ZERO = "Z"      # finish a path (no movement)


# =============================================================================
# Op kinds
# =============================================================================


class KiCadPlotterOpKind(str, Enum):
    """
    Every PLOTTER virtual method exposed in ``include/plotters/plotter.h``
    has a corresponding op kind. The ``str`` values match the C++ method
    name verbatim so a future ``RECORDER_PLOTTER`` patch can dump JSON
    by introspecting the call site.
    """

    # ---- path verbs ----
    PEN_TO = "PenTo"

    # ---- primitives ----
    CIRCLE = "Circle"
    ARC_THREE_POINT = "ArcThreePoint"
    ARC_CENTER_ANGLE = "ArcCenterAngle"
    BEZIER_CURVE = "BezierCurve"
    RECT = "Rect"
    PLOT_POLY = "PlotPoly"
    TEXT = "Text"
    PLOT_IMAGE = "PlotImage"

    # ---- thick-segment / thick-arc convenience ----
    THICK_SEGMENT = "ThickSegment"
    THICK_ARC = "ThickArc"

    # ---- pad flashes (PCB) ----
    FLASH_PAD_CIRCLE = "FlashPadCircle"
    FLASH_PAD_OVAL = "FlashPadOval"
    FLASH_PAD_RECT = "FlashPadRect"
    FLASH_PAD_ROUNDRECT = "FlashPadRoundRect"
    FLASH_PAD_CUSTOM = "FlashPadCustom"
    FLASH_PAD_TRAPEZ = "FlashPadTrapez"
    FLASH_REG_POLYGON = "FlashRegularPolygon"

    # ---- state ----
    SET_CURRENT_LINE_WIDTH = "SetCurrentLineWidth"
    SET_COLOR = "SetColor"
    SET_DASH = "SetDash"
    SET_VIEWPORT = "SetViewport"

    # ---- lifecycle ----
    START_PLOT = "StartPlot"
    END_PLOT = "EndPlot"
    SET_PAGE_SETTINGS = "SetPageSettings"

    # ---- grouping (SVG / Gerber blocks) ----
    START_BLOCK = "StartBlock"
    END_BLOCK = "EndBlock"


_OP_KIND_VALUES = {kind.value for kind in KiCadPlotterOpKind}


def _coerce_kind(kind: KiCadPlotterOpKind | str) -> KiCadPlotterOpKind | str:
    """
    Return the canonical :class:`KiCadPlotterOpKind` if the value is
    known; otherwise return the raw string so a future RECORDER op
    kind that we don't yet have an enum for still round-trips. This
    keeps the IR forward-compatible with KiCad upstream changes.
    """
    if isinstance(kind, KiCadPlotterOpKind):
        return kind
    text = str(kind)
    if text in _OP_KIND_VALUES:
        return KiCadPlotterOpKind(text)
    return text


# =============================================================================
# Bounds
# =============================================================================


@dataclass(frozen=True)
class KiCadPlotterBounds:
    """
    Axis-aligned bounding box in KiCad internal units (nm).

    Field names match KiCad's ``BOX2I`` mental model: ``left/top/right/bottom``
    with Y-down (matches the schematic plotter's flipped viewport).
    """

    left: int
    top: int
    right: int
    bottom: int

    @classmethod
    def from_dict(cls, data: dict[str, Any] | None) -> KiCadPlotterBounds | None:
        if not isinstance(data, dict):
            return None
        return cls(
            left=int(data.get("left", 0)),
            top=int(data.get("top", 0)),
            right=int(data.get("right", 0)),
            bottom=int(data.get("bottom", 0)),
        )

    def to_dict(self) -> dict[str, int]:
        return {
            "left": int(self.left),
            "top": int(self.top),
            "right": int(self.right),
            "bottom": int(self.bottom),
        }


# =============================================================================
# Pen / brush / font payload helpers
# =============================================================================


def _normalize_color_hex(color: str) -> str:
    """
    Normalize a hex colour to ``"#RRGGBB"`` or ``"#RRGGBBAA"`` (uppercase).
    Accepts ``"#RGB"``, ``"#RGBA"``, ``"#RRGGBB"``, ``"#RRGGBBAA"`` and
    the leading ``#`` is required.
    """
    text = str(color).strip()
    if not text.startswith("#"):
        raise ValueError(f"Color must start with '#': {color!r}")
    body = text[1:].upper()
    if len(body) == 3:
        body = "".join(c * 2 for c in body)
    elif len(body) == 4:
        body = "".join(c * 2 for c in body)
    if len(body) not in (6, 8):
        raise ValueError(f"Color must be #RGB / #RGBA / #RRGGBB / #RRGGBBAA: {color!r}")
    if any(c not in "0123456789ABCDEF" for c in body):
        raise ValueError(f"Color contains non-hex chars: {color!r}")
    return "#" + body


def make_pen(
    *,
    color: str = "#000000",
    width_nm: int = 0,
    line_style: KiCadLineStyle | str = KiCadLineStyle.SOLID,
    dash_values: list[float] | None = None,
) -> dict[str, Any]:
    """
    Build a JSON-safe pen descriptor.
    """
    return {
        "color": _normalize_color_hex(color),
        "width_nm": int(width_nm),
        "line_style": KiCadLineStyle(str(line_style)).value
        if not isinstance(line_style, KiCadLineStyle)
        else line_style.value,
        "dash_values": [float(v) for v in (dash_values or [])],
    }


def make_brush(*, color: str = "#000000", alpha: int = 255) -> dict[str, Any]:
    """
    Build a JSON-safe brush descriptor (solid fill).
    """
    alpha_byte = max(0, min(255, int(alpha)))
    return {
        "color": _normalize_color_hex(color),
        "alpha": alpha_byte,
    }


def make_font(
    *,
    face: str = "",
    size_nm: int = 0,
    italic: bool = False,
    bold: bool = False,
    rotation_deg: float = 0.0,
) -> dict[str, Any]:
    """
    Build a JSON-safe font descriptor.

    ``face=""`` means KiCad's stroke font (KiFont newstroke). Non-empty
    values resolve through the document's font catalog.
    """
    return {
        "face": str(face),
        "size_nm": int(size_nm),
        "italic": bool(italic),
        "bold": bool(bold),
        "rotation_deg": float(rotation_deg),
    }


def styled_plotter_op(
    op: "KiCadPlotterOp",
    *,
    stroke_color: str | None = None,
    fill_color: str | None = None,
    line_style: KiCadLineStyle | str | None = None,
) -> "KiCadPlotterOp":
    """
    Return ``op`` with declarative style metadata attached.

    KiCad's recorder stream carries stroke/fill styling mostly as plotter
    state (`SetColor`, `SetDash`, `SetCurrentLineWidth`). The monkey IR keeps
    geometry declarative, so parser-side emitters attach the effective style
    to each primitive payload for oracle normalization and SVG rendering.
    """
    payload = copy.deepcopy(op.payload)
    if stroke_color:
        payload["stroke_color"] = _normalize_color_hex(stroke_color)
    if fill_color:
        payload["fill_color"] = _normalize_color_hex(fill_color)
    if line_style is not None:
        payload["line_style"] = (
            KiCadLineStyle(str(line_style)).value
            if not isinstance(line_style, KiCadLineStyle)
            else line_style.value
        )
    return KiCadPlotterOp(kind=op.kind, payload=payload)


# =============================================================================
# Plotter operation
# =============================================================================


@dataclass(frozen=True)
class KiCadPlotterOp:
    """
    One PLOTTER call recorded in JSON-safe form.

    ``kind`` is a :class:`KiCadPlotterOpKind` for the well-known set,
    or a raw ``str`` for forward-compat with future PLOTTER virtuals.

    ``payload`` contains only JSON-serializable primitives
    (``int``/``float``/``str``/``bool``/``None``/``list``/``dict``).
    Coordinates are in KiCad internal units (nm). Angles are degrees
    (float).
    """

    kind: KiCadPlotterOpKind | str
    payload: dict[str, Any] = field(default_factory=dict)

    # ---- serialisation ----

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> KiCadPlotterOp:
        payload = copy.deepcopy(data)
        raw_kind = payload.pop("kind", "")
        payload.pop("index", None)
        return cls(kind=_coerce_kind(raw_kind), payload=payload)

    def to_dict(self, *, index: int | None = None) -> dict[str, Any]:
        kind_value = (
            self.kind.value if isinstance(self.kind, KiCadPlotterOpKind) else str(self.kind)
        )
        out: dict[str, Any] = {"kind": kind_value}
        if index is not None:
            out["index"] = int(index)
        out.update(copy.deepcopy(self.payload))
        return out

    # ---- path verbs ----

    @classmethod
    def pen_to(cls, *, x: int, y: int, action: KiCadPenAction | str = KiCadPenAction.DOWN) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.PEN_TO,
            payload={"x": int(x), "y": int(y), "action": KiCadPenAction(str(action)).value
                     if not isinstance(action, KiCadPenAction) else action.value},
        )

    # ---- primitives ----

    @classmethod
    def circle(
        cls,
        *,
        cx: int,
        cy: int,
        diameter_nm: int,
        fill: KiCadFillType | str = KiCadFillType.NO_FILL,
        width_nm: int = 0,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.CIRCLE,
            payload={
                "cx": int(cx),
                "cy": int(cy),
                "diameter_nm": int(diameter_nm),
                "fill": KiCadFillType(str(fill)).value
                if not isinstance(fill, KiCadFillType)
                else fill.value,
                "width_nm": int(width_nm),
            },
        )

    @classmethod
    def arc_three_point(
        cls,
        *,
        start_x: float,
        start_y: float,
        mid_x: float,
        mid_y: float,
        end_x: float,
        end_y: float,
        fill: KiCadFillType | str = KiCadFillType.NO_FILL,
        width_nm: int = 0,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.ARC_THREE_POINT,
            payload={
                "start_x": float(start_x),
                "start_y": float(start_y),
                "mid_x": float(mid_x),
                "mid_y": float(mid_y),
                "end_x": float(end_x),
                "end_y": float(end_y),
                "fill": KiCadFillType(str(fill)).value
                if not isinstance(fill, KiCadFillType)
                else fill.value,
                "width_nm": int(width_nm),
            },
        )

    @classmethod
    def arc_center_angle(
        cls,
        *,
        cx: float,
        cy: float,
        start_angle_deg: float,
        sweep_deg: float,
        radius_nm: float,
        fill: KiCadFillType | str = KiCadFillType.NO_FILL,
        width_nm: int = 0,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.ARC_CENTER_ANGLE,
            payload={
                "cx": float(cx),
                "cy": float(cy),
                "start_angle_deg": float(start_angle_deg),
                "sweep_deg": float(sweep_deg),
                "radius_nm": float(radius_nm),
                "fill": KiCadFillType(str(fill)).value
                if not isinstance(fill, KiCadFillType)
                else fill.value,
                "width_nm": int(width_nm),
            },
        )

    @classmethod
    def bezier_curve(
        cls,
        *,
        start_x: int,
        start_y: int,
        ctrl1_x: int,
        ctrl1_y: int,
        ctrl2_x: int,
        ctrl2_y: int,
        end_x: int,
        end_y: int,
        tolerance_nm: int = 0,
        width_nm: int = 0,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.BEZIER_CURVE,
            payload={
                "start_x": int(start_x),
                "start_y": int(start_y),
                "ctrl1_x": int(ctrl1_x),
                "ctrl1_y": int(ctrl1_y),
                "ctrl2_x": int(ctrl2_x),
                "ctrl2_y": int(ctrl2_y),
                "end_x": int(end_x),
                "end_y": int(end_y),
                "tolerance_nm": int(tolerance_nm),
                "width_nm": int(width_nm),
            },
        )

    @classmethod
    def rect(
        cls,
        *,
        x1: int,
        y1: int,
        x2: int,
        y2: int,
        fill: KiCadFillType | str = KiCadFillType.NO_FILL,
        width_nm: int = 0,
        corner_radius_nm: int = 0,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.RECT,
            payload={
                "x1": int(x1),
                "y1": int(y1),
                "x2": int(x2),
                "y2": int(y2),
                "fill": KiCadFillType(str(fill)).value
                if not isinstance(fill, KiCadFillType)
                else fill.value,
                "width_nm": int(width_nm),
                "corner_radius_nm": int(corner_radius_nm),
            },
        )

    @classmethod
    def plot_poly(
        cls,
        *,
        points: list[tuple[int, int]] | list[list[int]],
        fill: KiCadFillType | str = KiCadFillType.NO_FILL,
        width_nm: int = 0,
    ) -> KiCadPlotterOp:
        normalized = [[int(p[0]), int(p[1])] for p in points]
        return cls(
            kind=KiCadPlotterOpKind.PLOT_POLY,
            payload={
                "points": normalized,
                "fill": KiCadFillType(str(fill)).value
                if not isinstance(fill, KiCadFillType)
                else fill.value,
                "width_nm": int(width_nm),
            },
        )

    @classmethod
    def text(
        cls,
        *,
        x: int,
        y: int,
        text: str,
        color: str = "#000000",
        orient_deg: float = 0.0,
        size_x_nm: int,
        size_y_nm: int,
        h_align: KiCadHorizAlign | str = KiCadHorizAlign.LEFT,
        v_align: KiCadVertAlign | str = KiCadVertAlign.BOTTOM,
        pen_width_nm: int = 0,
        italic: bool = False,
        bold: bool = False,
        multiline: bool = False,
        font_face: str = "",
        render_cache_polygons: list[list[list[int]]] | None = None,
        render_cache: dict[str, Any] | None = None,
    ) -> KiCadPlotterOp:
        payload = {
            "x": int(x),
            "y": int(y),
            "text": str(text),
            "color": _normalize_color_hex(color),
            "orient_deg": float(orient_deg),
            "size_x_nm": int(size_x_nm),
            "size_y_nm": int(size_y_nm),
            "h_align": KiCadHorizAlign(str(h_align)).value
            if not isinstance(h_align, KiCadHorizAlign)
            else h_align.value,
            "v_align": KiCadVertAlign(str(v_align)).value
            if not isinstance(v_align, KiCadVertAlign)
            else v_align.value,
            "pen_width_nm": int(pen_width_nm),
            "italic": bool(italic),
            "bold": bool(bold),
            "multiline": bool(multiline),
            "font_face": str(font_face),
        }
        if render_cache_polygons:
            payload["render_cache_polygons"] = [
                [[int(point[0]), int(point[1])] for point in polygon]
                for polygon in render_cache_polygons
                if polygon
            ]
        if render_cache:
            payload["render_cache"] = copy.deepcopy(render_cache)
        return cls(kind=KiCadPlotterOpKind.TEXT, payload=payload)

    @classmethod
    def plot_image(
        cls,
        *,
        x: int,
        y: int,
        width_nm: int,
        height_nm: int,
        scale: float = 1.0,
        image_data_b64: str = "",
        image_format: str = "png",
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.PLOT_IMAGE,
            payload={
                "x": int(x),
                "y": int(y),
                "width_nm": int(width_nm),
                "height_nm": int(height_nm),
                "scale": float(scale),
                "image_data_b64": str(image_data_b64),
                "image_format": str(image_format),
            },
        )

    @classmethod
    def thick_segment(
        cls,
        *,
        start_x: int,
        start_y: int,
        end_x: int,
        end_y: int,
        width_nm: int,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.THICK_SEGMENT,
            payload={
                "start_x": int(start_x),
                "start_y": int(start_y),
                "end_x": int(end_x),
                "end_y": int(end_y),
                "width_nm": int(width_nm),
            },
        )

    @classmethod
    def thick_arc(
        cls,
        *,
        cx: float,
        cy: float,
        start_angle_deg: float,
        sweep_deg: float,
        radius_nm: float,
        width_nm: int,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.THICK_ARC,
            payload={
                "cx": float(cx),
                "cy": float(cy),
                "start_angle_deg": float(start_angle_deg),
                "sweep_deg": float(sweep_deg),
                "radius_nm": float(radius_nm),
                "width_nm": int(width_nm),
            },
        )

    # ---- pad flashes ----

    @classmethod
    def flash_pad_circle(
        cls, *, x: int, y: int, diameter_nm: int
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.FLASH_PAD_CIRCLE,
            payload={"x": int(x), "y": int(y), "diameter_nm": int(diameter_nm)},
        )

    @classmethod
    def flash_pad_oval(
        cls, *, x: int, y: int, size_x_nm: int, size_y_nm: int, orient_deg: float
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.FLASH_PAD_OVAL,
            payload={
                "x": int(x),
                "y": int(y),
                "size_x_nm": int(size_x_nm),
                "size_y_nm": int(size_y_nm),
                "orient_deg": float(orient_deg),
            },
        )

    @classmethod
    def flash_pad_rect(
        cls, *, x: int, y: int, size_x_nm: int, size_y_nm: int, orient_deg: float
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.FLASH_PAD_RECT,
            payload={
                "x": int(x),
                "y": int(y),
                "size_x_nm": int(size_x_nm),
                "size_y_nm": int(size_y_nm),
                "orient_deg": float(orient_deg),
            },
        )

    @classmethod
    def flash_pad_roundrect(
        cls,
        *,
        x: int,
        y: int,
        size_x_nm: int,
        size_y_nm: int,
        corner_radius_nm: int,
        orient_deg: float,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.FLASH_PAD_ROUNDRECT,
            payload={
                "x": int(x),
                "y": int(y),
                "size_x_nm": int(size_x_nm),
                "size_y_nm": int(size_y_nm),
                "corner_radius_nm": int(corner_radius_nm),
                "orient_deg": float(orient_deg),
            },
        )

    @classmethod
    def flash_pad_custom(
        cls,
        *,
        x: int,
        y: int,
        size_x_nm: int,
        size_y_nm: int,
        orient_deg: float,
        polygons: list[list[list[int]]],
        polygon_widths_nm: list[int] | None = None,
        anchor_shape: str | None = None,
    ) -> KiCadPlotterOp:
        normalized = [[[int(p[0]), int(p[1])] for p in ring] for ring in polygons]
        payload: dict[str, Any] = {
            "x": int(x),
            "y": int(y),
            "size_x_nm": int(size_x_nm),
            "size_y_nm": int(size_y_nm),
            "orient_deg": float(orient_deg),
            "polygons": normalized,
        }
        if polygon_widths_nm is not None:
            payload["polygon_widths_nm"] = [int(width) for width in polygon_widths_nm]
        if anchor_shape:
            payload["anchor_shape"] = str(anchor_shape)
        return cls(kind=KiCadPlotterOpKind.FLASH_PAD_CUSTOM, payload=payload)

    @classmethod
    def flash_pad_trapez(
        cls,
        *,
        x: int,
        y: int,
        corners: list[tuple[int, int]] | list[list[int]],
        orient_deg: float,
    ) -> KiCadPlotterOp:
        if len(corners) != 4:
            raise ValueError(f"trapezoid requires 4 corners, got {len(corners)}")
        normalized = [[int(c[0]), int(c[1])] for c in corners]
        return cls(
            kind=KiCadPlotterOpKind.FLASH_PAD_TRAPEZ,
            payload={
                "x": int(x),
                "y": int(y),
                "corners": normalized,
                "orient_deg": float(orient_deg),
            },
        )

    @classmethod
    def flash_reg_polygon(
        cls,
        *,
        x: int,
        y: int,
        diameter_nm: int,
        corner_count: int,
        orient_deg: float,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.FLASH_REG_POLYGON,
            payload={
                "x": int(x),
                "y": int(y),
                "diameter_nm": int(diameter_nm),
                "corner_count": int(corner_count),
                "orient_deg": float(orient_deg),
            },
        )

    # ---- state ----

    @classmethod
    def set_current_line_width(cls, *, width_nm: int) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.SET_CURRENT_LINE_WIDTH,
            payload={"width_nm": int(width_nm)},
        )

    @classmethod
    def set_color(cls, *, color: str) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.SET_COLOR,
            payload={"color": _normalize_color_hex(color)},
        )

    @classmethod
    def set_dash(
        cls,
        *,
        line_width_nm: int,
        line_style: KiCadLineStyle | str,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.SET_DASH,
            payload={
                "line_width_nm": int(line_width_nm),
                "line_style": KiCadLineStyle(str(line_style)).value
                if not isinstance(line_style, KiCadLineStyle)
                else line_style.value,
            },
        )

    @classmethod
    def set_viewport(
        cls,
        *,
        offset_x_nm: int,
        offset_y_nm: int,
        ius_per_decimil: float,
        scale: float,
        mirror: bool,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.SET_VIEWPORT,
            payload={
                "offset_x_nm": int(offset_x_nm),
                "offset_y_nm": int(offset_y_nm),
                "ius_per_decimil": float(ius_per_decimil),
                "scale": float(scale),
                "mirror": bool(mirror),
            },
        )

    # ---- lifecycle ----

    @classmethod
    def start_plot(cls, *, page_name: str = "") -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.START_PLOT,
            payload={"page_name": str(page_name)},
        )

    @classmethod
    def end_plot(cls) -> KiCadPlotterOp:
        return cls(kind=KiCadPlotterOpKind.END_PLOT)

    @classmethod
    def set_page_settings(
        cls,
        *,
        page_type: str,
        width_nm: int,
        height_nm: int,
        portrait: bool = False,
    ) -> KiCadPlotterOp:
        return cls(
            kind=KiCadPlotterOpKind.SET_PAGE_SETTINGS,
            payload={
                "page_type": str(page_type),
                "width_nm": int(width_nm),
                "height_nm": int(height_nm),
                "portrait": bool(portrait),
            },
        )

    # ---- grouping ----

    @classmethod
    def start_block(
        cls,
        *,
        label: str = "",
        data_uuid: str = "",
        data_ref: str = "",
        object_id: str = "",
        extra_attrs: dict[str, Any] | None = None,
        layers: list[str] | tuple[str, ...] | None = None,
    ) -> KiCadPlotterOp:
        payload: dict[str, Any] = {"label": str(label)}
        if data_uuid:
            payload["data_uuid"] = str(data_uuid)
        if data_ref:
            payload["data_ref"] = str(data_ref)
        if object_id:
            payload["object_id"] = str(object_id)
        if layers:
            payload["layers"] = [str(layer) for layer in layers if str(layer)]
        if extra_attrs:
            payload["extra_attrs"] = {
                str(key): str(value)
                for key, value in extra_attrs.items()
                if value is not None and str(value) != ""
            }
        return cls(
            kind=KiCadPlotterOpKind.START_BLOCK,
            payload=payload,
        )

    @classmethod
    def end_block(cls) -> KiCadPlotterOp:
        return cls(kind=KiCadPlotterOpKind.END_BLOCK)


# =============================================================================
# Records and document
# =============================================================================


@dataclass(frozen=True)
class KiCadPlotterRecord:
    """
    One source-item-keyed sub-stream of plotter ops.

    Every record corresponds to one schematic or PCB item (symbol, wire,
    label, footprint, pad, track, ...) and groups all of its plotter calls
    for downstream diff and scene tooling.
    """

    uuid: str
    kind: str
    object_id: str
    bounds: KiCadPlotterBounds | None = None
    operations: list[KiCadPlotterOp] = field(default_factory=list)
    extras: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> KiCadPlotterRecord:
        extras = copy.deepcopy(data)
        operations_data = extras.pop("operations", []) or []
        bounds = KiCadPlotterBounds.from_dict(extras.pop("bounds", None))
        extras.pop("operation_count", None)
        return cls(
            uuid=str(extras.pop("uuid", "")),
            kind=str(extras.pop("kind", "")),
            object_id=str(extras.pop("object_id", "")),
            bounds=bounds,
            operations=[
                KiCadPlotterOp.from_dict(op)
                for op in operations_data
                if isinstance(op, dict)
            ],
            extras=extras,
        )

    def to_dict(self) -> dict[str, Any]:
        data: dict[str, Any] = {
            "uuid": self.uuid,
            "kind": self.kind,
            "object_id": self.object_id,
        }
        if self.bounds is not None:
            data["bounds"] = self.bounds.to_dict()
        data["operation_count"] = len(self.operations)
        data["operations"] = [
            op.to_dict(index=index) for index, op in enumerate(self.operations)
        ]
        data.update(copy.deepcopy(self.extras))
        return data


@dataclass(frozen=True)
class KiCadPlotterDocument:
    """
    Top-level IR document. JSON-serializable; round-trippable via
    :meth:`from_dict` / :meth:`to_dict`.
    """

    records: list[KiCadPlotterRecord] = field(default_factory=list)
    source_path: str | None = None
    source_kind: str = "SCH"  # "SCH" | "SYM" | "PCB" | "FP"
    generated_utc: str | None = None
    document_id: str | None = None
    canvas: dict[str, Any] | None = None
    coordinate_space: dict[str, Any] | None = None
    background_color: str | None = None
    render_hints: dict[str, Any] | None = None
    extras: dict[str, Any] = field(default_factory=dict)

    # ---- I/O ----

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> KiCadPlotterDocument:
        schema = str(data.get("schema", "")).strip()
        if schema not in KICAD_PLOTTER_IR_ACCEPTED_SCHEMAS:
            raise ValueError(
                f"Unexpected plotter IR schema: {schema!r} "
                f"(expected {KICAD_PLOTTER_IR_SCHEMA!r})"
            )
        extras = copy.deepcopy(data)
        for key in (
            "schema",
            "source_path",
            "source_kind",
            "generated_utc",
            "document_id",
            "canvas",
            "coordinate_space",
            "background_color",
            "render_hints",
            "records",
            "total_operations",
        ):
            extras.pop(key, None)

        records = [
            KiCadPlotterRecord.from_dict(rec)
            for rec in (data.get("records") or [])
            if isinstance(rec, dict)
        ]

        return cls(
            records=records,
            source_path=str(data["source_path"]) if data.get("source_path") is not None else None,
            source_kind=str(data.get("source_kind", "SCH")),
            generated_utc=str(data["generated_utc"]) if data.get("generated_utc") is not None else None,
            document_id=str(data["document_id"]) if data.get("document_id") is not None else None,
            canvas=copy.deepcopy(data["canvas"]) if isinstance(data.get("canvas"), dict) else None,
            coordinate_space=copy.deepcopy(data["coordinate_space"])
            if isinstance(data.get("coordinate_space"), dict)
            else None,
            background_color=str(data["background_color"])
            if data.get("background_color") is not None
            else None,
            render_hints=copy.deepcopy(data["render_hints"])
            if isinstance(data.get("render_hints"), dict)
            else None,
            extras=extras,
        )

    @classmethod
    def from_file(cls, path: str | Path) -> KiCadPlotterDocument:
        ir_path = Path(path)
        data = json.loads(ir_path.read_text(encoding="utf-8-sig"))
        if not isinstance(data, dict):
            raise ValueError(f"Plotter IR payload must be a JSON object: {ir_path}")
        return cls.from_dict(data)

    def to_dict(self) -> dict[str, Any]:
        data: dict[str, Any] = {
            "schema": KICAD_PLOTTER_IR_SCHEMA,
            "source_kind": self.source_kind,
            "total_operations": sum(len(rec.operations) for rec in self.records),
            "records": [rec.to_dict() for rec in self.records],
        }
        if self.source_path is not None:
            data["source_path"] = self.source_path
        if self.generated_utc is not None:
            data["generated_utc"] = self.generated_utc
        if self.document_id is not None:
            data["document_id"] = self.document_id
        if self.canvas is not None:
            data["canvas"] = copy.deepcopy(self.canvas)
        if self.coordinate_space is not None:
            data["coordinate_space"] = copy.deepcopy(self.coordinate_space)
        if self.background_color is not None:
            data["background_color"] = self.background_color
        if self.render_hints is not None:
            data["render_hints"] = copy.deepcopy(self.render_hints)
        data.update(copy.deepcopy(self.extras))
        return data

    def to_normalized_dict(self, *, source_path: str | None = None) -> dict[str, Any]:
        """
        Drop non-deterministic fields (``generated_utc``) and normalise
        path separators so two IR docs from the same source compare
        cleanly. Used by oracle-diff workflows.
        """
        data = self.to_dict()
        data.pop("generated_utc", None)
        if source_path is None:
            data.pop("source_path", None)
        else:
            data["source_path"] = source_path.replace("\\", "/")
        records = data.get("records")
        if isinstance(records, list):
            for rec in records:
                if isinstance(rec, dict) and isinstance(rec.get("operations"), list):
                    rec["operation_count"] = len(rec["operations"])
            data["total_operations"] = sum(
                len(rec.get("operations") or [])
                for rec in records
                if isinstance(rec, dict)
            )
        return data

    def write_json(self, path: str | Path) -> Path:
        out_path = Path(path)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(
            json.dumps(self.to_dict(), indent=2) + "\n",
            encoding="utf-8",
        )
        return out_path

    def write_normalized_json(
        self,
        path: str | Path,
        *,
        source_path: str | None = None,
    ) -> Path:
        out_path = Path(path)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        payload = self.to_normalized_dict(source_path=source_path)
        out_path.write_text(
            json.dumps(payload, indent=2) + "\n",
            encoding="utf-8",
        )
        return out_path


__all__ = [
    "KICAD_PLOTTER_IR_ACCEPTED_SCHEMAS",
    "KICAD_PLOTTER_IR_SCHEMA",
    "KiCadFillType",
    "KiCadHorizAlign",
    "KiCadLineStyle",
    "KiCadPenAction",
    "KiCadPlotterBounds",
    "KiCadPlotterDocument",
    "KiCadPlotterOp",
    "KiCadPlotterOpKind",
    "KiCadPlotterRecord",
    "KiCadVertAlign",
    "make_brush",
    "make_font",
    "make_pen",
    "styled_plotter_op",
]

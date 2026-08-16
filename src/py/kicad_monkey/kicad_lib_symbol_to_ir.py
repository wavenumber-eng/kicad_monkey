"""
LibSymbol to KiCadPlotterDocument converter.

Walks a parsed :class:`LibSymbol` (or :class:`LibSubSymbol`) and emits
a :class:`KiCadPlotterDocument` whose records contain
:class:`KiCadPlotterOp` instances drawn from the PLOTTER vocabulary.
This is the "parser to IR" leg of the plotter pipeline; the ``svg_*``
primitive layer (or a future RECORDER_PLOTTER-side oracle)
consumes the IR.

Mirrors KiCad's ``LIB_SYMBOL::Plot()`` traversal:
  rectangles → circles → arcs → polylines → beziers → texts → pins.

Coordinate convention: ``.kicad_sym`` files store positions in mm with
**Y-up**. The IR carries KiCad internal-unit nm with **Y-down** to
match KiCad's PLOTTER (post-flip) convention. We multiply by 1_000_000
and negate Y at the boundary so a future RECORDER_PLOTTER dump and our
parser-side dump can be diffed directly.

Stroke widths translate to KiCad plot-time widths: zero uses the
schematic symbol default, positive hairlines are clamped to the plotter
minimum, and negative imported sentinel widths remain literal zero.
"""

from __future__ import annotations

import math
import re
from typing import TYPE_CHECKING, Any, Callable, Iterable

from .kicad_base import StrokeType
from .kicad_defaults import (
    KICAD_DEFAULT_PIN_NAME_OFFSET_MM,
    KICAD_DEFAULT_TEXT_SIZE_MM,
)
from .kicad_plotter_ir import (
    KICAD_PLOTTER_IR_SCHEMA,  # noqa: F401 - re-exported for callers
    KiCadFillType,
    KiCadHorizAlign,
    KiCadLineStyle,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadVertAlign,
    styled_plotter_op,
)
from .kicad_schematic_style import (
    DEFAULT_MIN_PLOT_PEN_WIDTH_NM,
    DEFAULT_PIN_SYMBOL_RADIUS_NM,
    DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM,
    DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
    DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM,
    LAYER_DEVICE,
    LAYER_DEVICE_BACKGROUND,
    LAYER_PIN,
    LAYER_PINNAM,
    LAYER_PINNUM,
    apply_default_text_style,
)
from .kicad_sym_rectangle import SymFillType

if TYPE_CHECKING:
    from .kicad_lib_subsymbol import LibSubSymbol
    from .kicad_lib_symbol import LibSymbol
    from .kicad_primitives import Effects, Stroke
    from .kicad_sym_arc import SymArc
    from .kicad_sym_bezier import SymBezier
    from .kicad_sym_circle import SymCircle
    from .kicad_sym_pin import SymPin
    from .kicad_sym_polyline import SymPolyline
    from .kicad_sym_rectangle import SymFill, SymRectangle
    from .kicad_sym_text import SymText


# ---------------------------------------------------------------------------
# Unit conversion + enum mapping
# ---------------------------------------------------------------------------


_MM_PER_NM = 1_000_000  # 1 mm = 1_000_000 nm


def mm_to_nm(value_mm: float) -> int:
    """Convert a mm float (as parsed from .kicad_sym) to int nm."""
    return int(round(value_mm * _MM_PER_NM))


def y_to_ir(y_mm: float) -> int:
    """Convert a Y-up mm coord to Y-down nm coord (KiCad PLOTTER convention)."""
    return -mm_to_nm(y_mm)


_STROKE_TO_LINE_STYLE: dict[StrokeType, KiCadLineStyle] = {
    StrokeType.SOLID: KiCadLineStyle.SOLID,
    StrokeType.DASH: KiCadLineStyle.DASH,
    StrokeType.DOT: KiCadLineStyle.DOT,
    StrokeType.DASH_DOT: KiCadLineStyle.DASH_DOT,
    StrokeType.DASH_DOT_DOT: KiCadLineStyle.DASH_DOT_DOT,
    StrokeType.DEFAULT: KiCadLineStyle.DEFAULT,
}


_SYM_FILL_TO_KICAD_FILL: dict[SymFillType, KiCadFillType] = {
    SymFillType.NONE: KiCadFillType.NO_FILL,
    SymFillType.OUTLINE: KiCadFillType.FILLED_SHAPE,
    SymFillType.BACKGROUND: KiCadFillType.FILLED_WITH_BG_BODYCOLOR,
    SymFillType.COLOR: KiCadFillType.FILLED_WITH_COLOR,
    SymFillType.HATCH: KiCadFillType.HATCH,
    SymFillType.REVERSE_HATCH: KiCadFillType.REVERSE_HATCH,
    SymFillType.CROSS_HATCH: KiCadFillType.CROSS_HATCH,
}


def stroke_type_to_line_style(stroke_type: StrokeType) -> KiCadLineStyle:
    """Map a parser StrokeType to the IR LINE_STYLE mirror."""
    return _STROKE_TO_LINE_STYLE.get(stroke_type, KiCadLineStyle.DEFAULT)


def sym_fill_to_kicad_fill(sym_fill_type: SymFillType) -> KiCadFillType:
    """Map a parser SymFillType to the IR FILL_T mirror."""
    return _SYM_FILL_TO_KICAD_FILL.get(sym_fill_type, KiCadFillType.NO_FILL)


def rgba_to_hex(rgba: tuple[int, int, int, float] | None) -> str | None:
    """Convert KiCad's (R, G, B, A) tuple (0-255 ints + 0-1 float) to ``#RRGGBBAA``."""
    if rgba is None:
        return None
    r, g, b, a = rgba
    if float(a) <= 0.0:
        return None
    r_i = max(0, min(255, int(r)))
    g_i = max(0, min(255, int(g)))
    b_i = max(0, min(255, int(b)))
    # Alpha stored 0-1 in the parser; some legacy fixtures use 0-255 as
    # a float — coerce conservatively.
    a_clamped = float(a)
    a_i = (
        max(0, min(255, int(round(a_clamped * 255))))
        if 0.0 <= a_clamped <= 1.0
        else max(0, min(255, int(round(a_clamped))))
    )
    return f"#{r_i:02X}{g_i:02X}{b_i:02X}{a_i:02X}"


def stroke_width_nm(
    stroke: Any,
    *,
    default_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
) -> int:
    """Return the IR stroke width in nm.

    KiCad uses zero-width symbol strokes as "use the schematic default".
    Some imported symbols carry tiny negative widths as a no-outline
    sentinel; KiCad plots those as literal zero width.
    """
    width = getattr(stroke, "width", 0.0) or 0.0
    if width < 0:
        return 0
    if width == 0:
        return max(int(default_width_nm), DEFAULT_MIN_PLOT_PEN_WIDTH_NM)
    return max(mm_to_nm(width), DEFAULT_MIN_PLOT_PEN_WIDTH_NM)


def _stroke_line_style(stroke: "Stroke") -> KiCadLineStyle:
    return stroke_type_to_line_style(stroke.type)


def _stroke_color(stroke: "Stroke") -> str:
    return rgba_to_hex(stroke.color) or LAYER_DEVICE if getattr(stroke, "color", None) else LAYER_DEVICE


def _fill_color(fill: "SymFill", stroke: "Stroke | None" = None) -> str | None:
    explicit = rgba_to_hex(fill.color) if getattr(fill, "color", None) else None
    if explicit:
        return explicit
    if fill.type == SymFillType.BACKGROUND:
        return LAYER_DEVICE_BACKGROUND
    if fill.type == SymFillType.OUTLINE:
        return _stroke_color(stroke) if stroke is not None else LAYER_DEVICE
    if fill.type == SymFillType.COLOR:
        return LAYER_DEVICE
    return None


# ---------------------------------------------------------------------------
# Per-shape op emitters
# ---------------------------------------------------------------------------


def rectangle_to_op(
    rect: "SymRectangle",
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
) -> KiCadPlotterOp:
    """Convert a :class:`SymRectangle` into a ``Rect`` op."""
    return styled_plotter_op(
        KiCadPlotterOp.rect(
            x1=mm_to_nm(rect.start_x),
            y1=y_to_ir(rect.start_y),
            x2=mm_to_nm(rect.end_x),
            y2=y_to_ir(rect.end_y),
            fill=sym_fill_to_kicad_fill(rect.fill.type),
            width_nm=stroke_width_nm(
                rect.stroke,
                default_width_nm=default_stroke_width_nm,
            ),
        ),
        stroke_color=_stroke_color(rect.stroke),
        fill_color=_fill_color(rect.fill, rect.stroke),
        line_style=_stroke_line_style(rect.stroke),
    )


def circle_to_op(
    circle: "SymCircle",
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
) -> KiCadPlotterOp:
    """Convert a :class:`SymCircle` into a ``Circle`` op."""
    return styled_plotter_op(
        KiCadPlotterOp.circle(
            cx=mm_to_nm(circle.center_x),
            cy=y_to_ir(circle.center_y),
            diameter_nm=mm_to_nm(circle.radius * 2.0),
            fill=sym_fill_to_kicad_fill(circle.fill.type),
            width_nm=stroke_width_nm(
                circle.stroke,
                default_width_nm=default_stroke_width_nm,
            ),
        ),
        stroke_color=_stroke_color(circle.stroke),
        fill_color=_fill_color(circle.fill, circle.stroke),
        line_style=_stroke_line_style(circle.stroke),
    )


def arc_to_op(
    arc: "SymArc",
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
) -> KiCadPlotterOp:
    """Convert a :class:`SymArc` into an ``ArcThreePoint`` op."""
    return styled_plotter_op(
        KiCadPlotterOp.arc_three_point(
            start_x=mm_to_nm(arc.start_x),
            start_y=y_to_ir(arc.start_y),
            mid_x=mm_to_nm(arc.mid_x),
            mid_y=y_to_ir(arc.mid_y),
            end_x=mm_to_nm(arc.end_x),
            end_y=y_to_ir(arc.end_y),
            fill=sym_fill_to_kicad_fill(arc.fill.type),
            width_nm=stroke_width_nm(
                arc.stroke,
                default_width_nm=default_stroke_width_nm,
            ),
        ),
        stroke_color=_stroke_color(arc.stroke),
        fill_color=_fill_color(arc.fill, arc.stroke),
        line_style=_stroke_line_style(arc.stroke),
    )


def polyline_to_op(
    poly: "SymPolyline",
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
) -> KiCadPlotterOp:
    """Convert a :class:`SymPolyline` into a ``PlotPoly`` op."""
    points = [(mm_to_nm(x), y_to_ir(y)) for x, y in poly.points]
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=points,
            fill=sym_fill_to_kicad_fill(poly.fill.type),
            width_nm=stroke_width_nm(
                poly.stroke,
                default_width_nm=default_stroke_width_nm,
            ),
        ),
        stroke_color=_stroke_color(poly.stroke),
        fill_color=_fill_color(poly.fill, poly.stroke),
        line_style=_stroke_line_style(poly.stroke),
    )


def bezier_to_op(
    bez: "SymBezier",
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
) -> KiCadPlotterOp | None:
    """
    Convert a :class:`SymBezier` into a ``BezierCurve`` op.

    KiCad symbol beziers are cubic (4 control points). For
    less-than-4-point shapes, falls back to a polyline op so callers
    don't need to special-case malformed/quadratic curves.
    """
    if len(bez.points) == 4:
        sx, sy = bez.points[0]
        c1x, c1y = bez.points[1]
        c2x, c2y = bez.points[2]
        ex, ey = bez.points[3]
        return styled_plotter_op(
            KiCadPlotterOp.bezier_curve(
                start_x=mm_to_nm(sx),
                start_y=y_to_ir(sy),
                ctrl1_x=mm_to_nm(c1x),
                ctrl1_y=y_to_ir(c1y),
                ctrl2_x=mm_to_nm(c2x),
                ctrl2_y=y_to_ir(c2y),
                end_x=mm_to_nm(ex),
                end_y=y_to_ir(ey),
                width_nm=stroke_width_nm(
                    bez.stroke,
                    default_width_nm=default_stroke_width_nm,
                ),
            ),
            stroke_color=_stroke_color(bez.stroke),
            line_style=_stroke_line_style(bez.stroke),
        )
    if len(bez.points) >= 2:
        # Degenerate case — emit as polyline so geometry isn't lost.
        return polyline_to_op_from_points(
            [(mm_to_nm(x), y_to_ir(y)) for x, y in bez.points],
            width_nm=stroke_width_nm(
                bez.stroke,
                default_width_nm=default_stroke_width_nm,
            ),
            fill=sym_fill_to_kicad_fill(bez.fill.type),
            stroke_color=_stroke_color(bez.stroke),
            fill_color=_fill_color(bez.fill, bez.stroke),
            line_style=_stroke_line_style(bez.stroke),
        )
    return None


def polyline_to_op_from_points(
    points: list[tuple[int, int]],
    *,
    width_nm: int = 0,
    fill: KiCadFillType = KiCadFillType.NO_FILL,
    stroke_color: str | None = None,
    fill_color: str | None = None,
    line_style: KiCadLineStyle | str | None = None,
) -> KiCadPlotterOp:
    """Direct ``PlotPoly`` emit from already-converted nm points."""
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(points=points, fill=fill, width_nm=width_nm),
        stroke_color=stroke_color,
        fill_color=fill_color,
        line_style=line_style,
    )


_FILLED_OUTLINE_KINDS = {
    KiCadPlotterOpKind.RECT,
    KiCadPlotterOpKind.CIRCLE,
    KiCadPlotterOpKind.ARC_THREE_POINT,
    KiCadPlotterOpKind.ARC_CENTER_ANGLE,
    KiCadPlotterOpKind.PLOT_POLY,
}


def _split_filled_outline_op(
    op: KiCadPlotterOp,
) -> tuple[KiCadPlotterOp, KiCadPlotterOp | None]:
    """
    Split KiCad symbol body primitives into fill-first and outline-later ops.

    KiCad plots filled library graphics as a fill pass, then emits pin/text
    graphics, then draws the stroke outline. Keeping that order in the IR
    avoids SVG outlines being hidden under pin graphics or text.
    """
    if op.kind not in _FILLED_OUTLINE_KINDS:
        return op, None
    fill = str(op.payload.get("fill") or "")
    if fill in (KiCadFillType.NO_FILL.value, KiCadFillType.FILLED_SHAPE.value):
        return op, None

    fill_payload = dict(op.payload)
    fill_payload["width_nm"] = 0
    fill_color = fill_payload.get("fill_color") or fill_payload.get("stroke_color")
    if fill_color:
        fill_payload["stroke_color"] = fill_color
        fill_payload["fill_color"] = fill_color
    outline_payload = dict(op.payload)
    outline_payload["fill"] = KiCadFillType.NO_FILL.value
    outline_payload.pop("fill_color", None)

    return (
        KiCadPlotterOp(kind=op.kind, payload=fill_payload),
        KiCadPlotterOp(kind=op.kind, payload=outline_payload),
    )


def _effects_to_text_kwargs(effects: Any | None) -> dict:
    """Pull font + alignment kwargs out of a parsed ``Effects`` block."""
    if effects is None:
        return {
            "size_x_nm": mm_to_nm(1.27),
            "size_y_nm": mm_to_nm(1.27),
        }
    font = effects.font
    out: dict = {
        "size_x_nm": mm_to_nm(font.size_x),
        "size_y_nm": mm_to_nm(font.size_y),
        "italic": bool(font.italic),
        "bold": bool(font.bold),
        "font_face": font.face or "",
    }
    if font.thickness is not None:
        out["pen_width_nm"] = mm_to_nm(font.thickness)
    font_color = getattr(font, "color", None)
    if font_color is not None:
        explicit_color = rgba_to_hex(font_color)
        if explicit_color is not None:
            out["color"] = explicit_color
    if effects.justify:
        h_map = {
            "left": KiCadHorizAlign.LEFT,
            "right": KiCadHorizAlign.RIGHT,
            "center": KiCadHorizAlign.CENTER,
        }
        v_map = {
            "top": KiCadVertAlign.TOP,
            "bottom": KiCadVertAlign.BOTTOM,
            "center": KiCadVertAlign.CENTER,
        }
        for tok in effects.justify:
            if tok in h_map:
                out["h_align"] = h_map[tok]
            elif tok in v_map:
                out["v_align"] = v_map[tok]
    return out


_PROJECT_TEXT_VARIABLE_RE = re.compile(r"\$\{([^}]+)\}")


def _expand_project_text_variables(text: str, project_vars: dict | None) -> str:
    if not project_vars:
        return text
    variables = {str(name): str(value) for name, value in project_vars.items()}

    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        return variables.get(name, match.group(0))

    return _PROJECT_TEXT_VARIABLE_RE.sub(replace, text)


def _wx_string_split_plot_text(text: str) -> str:
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    return "\n".join(lines)


def text_to_op(
    text: "SymText",
    *,
    project_vars: dict | None = None,
) -> KiCadPlotterOp | None:
    """Convert a :class:`SymText` into a ``Text`` op."""
    if bool(getattr(text, "hide", False)) or bool(
        getattr(getattr(text, "effects", None), "hide", False)
    ):
        return None
    kwargs = apply_default_text_style(_effects_to_text_kwargs(text.effects), LAYER_DEVICE)
    kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
    kwargs.setdefault("v_align", KiCadVertAlign.CENTER)
    resolved_text = _wx_string_split_plot_text(
        _expand_project_text_variables(text.text or "", project_vars)
    )
    return KiCadPlotterOp.text(
        x=mm_to_nm(text.at_x),
        y=y_to_ir(text.at_y),
        text=resolved_text,
        orient_deg=float(text.at_angle),
        multiline="\n" in resolved_text,
        **kwargs,
    )


# Pin-text default font size (KiCad default 50 mil = 1.27 mm).
_PIN_TEXT_DEFAULT_MM = KICAD_DEFAULT_TEXT_SIZE_MM
# SCH_PIN::PlotPinTexts offsets text by PIN_TEXT_MARGIN (4 mil) plus the
# plotted text stroke width. Pin numbers auto-plot at size/5, which is 10 mil
# for the default 50 mil pin-number font.
_PIN_TEXT_MARGIN_MM = 0.1016
_PIN_NUMBER_OFFSET_MM = _PIN_TEXT_MARGIN_MM


def _auto_pin_number_pen_width_nm(kwargs: dict) -> int:
    size_x = int(kwargs.get("size_x_nm") or 0)
    size_y = int(kwargs.get("size_y_nm") or 0)
    text_size = min(abs(size_x), abs(size_y))
    if text_size <= 0:
        return DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM
    return int(text_size / 5.0 + 0.5)


def _pin_number_text_kwargs(
    effects: "Effects | None",
    *,
    default_line_width_nm: int | None = None,
) -> dict:
    raw = _effects_to_text_kwargs(effects)
    explicit_width = int(raw.get("pen_width_nm") or 0) > 0
    kwargs = apply_default_text_style(
        raw,
        LAYER_PINNUM,
        clamp_pen_width=False,
    )
    if not explicit_width:
        if default_line_width_nm is not None:
            kwargs["pen_width_nm"] = int(default_line_width_nm)
        else:
            kwargs["pen_width_nm"] = _auto_pin_number_pen_width_nm(kwargs)
    return kwargs


def _pin_text_clearance_nm(kwargs: dict) -> int:
    return mm_to_nm(_PIN_TEXT_MARGIN_MM) + int(
        kwargs.get("pen_width_nm") or DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM
    )


def _pin_effects_text_size_positive(effects: "Effects | None") -> bool:
    if effects is None:
        return True
    font = getattr(effects, "font", None)
    if font is None:
        return True
    return (
        abs(float(getattr(font, "size_x", 0.0) or 0.0)) > 0.0
        and abs(float(getattr(font, "size_y", 0.0) or 0.0)) > 0.0
    )


_PIN_DIRECTION_STEP_NM = 1_000_000


def _pin_direction_local_nm(pin: "SymPin") -> tuple[int, int]:
    """Return the pin's orientation vector in local IR coordinates."""
    angle = int(round(float(pin.at_angle))) % 360
    if angle == 0:
        return _PIN_DIRECTION_STEP_NM, 0
    if angle == 180:
        return -_PIN_DIRECTION_STEP_NM, 0
    if angle == 90:
        return 0, -_PIN_DIRECTION_STEP_NM
    if angle == 270:
        return 0, _PIN_DIRECTION_STEP_NM

    rad = math.radians(float(pin.at_angle))
    return (
        int(round(math.cos(rad) * _PIN_DIRECTION_STEP_NM)),
        int(round(-math.sin(rad) * _PIN_DIRECTION_STEP_NM)),
    )


def _pin_direction_flags(direction_x: int, direction_y: int) -> tuple[bool, bool, bool]:
    """Return ``(horizontal, pin_right, pin_down)`` from a draw vector."""
    horizontal = abs(direction_x) >= abs(direction_y)
    pin_right = horizontal and direction_x > 0
    pin_down = (not horizontal) and direction_y > 0
    return horizontal, pin_right, pin_down


def _pin_style_value(graphic_style: object) -> str:
    value = getattr(graphic_style, "value", graphic_style)
    return str(value or "line")


def _pin_axis_step(start_x: int, start_y: int, end_x: int, end_y: int) -> tuple[int, int]:
    dx = 0 if end_x == start_x else (1 if end_x > start_x else -1)
    dy = 0 if end_y == start_y else (1 if end_y > start_y else -1)
    return dx, dy


def _pin_poly_op(points: list[tuple[int, int]]) -> KiCadPlotterOp:
    return styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=points,
            fill=KiCadFillType.NO_FILL,
            width_nm=DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM,
        ),
        stroke_color=LAYER_PIN,
    )


def _pin_circle_op(cx: int, cy: int) -> KiCadPlotterOp:
    return styled_plotter_op(
        KiCadPlotterOp.circle(
            cx=cx,
            cy=cy,
            diameter_nm=DEFAULT_PIN_SYMBOL_RADIUS_NM * 2,
            fill=KiCadFillType.NO_FILL,
            width_nm=DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM,
        ),
        stroke_color=LAYER_PIN,
    )


def pin_graphic_style_to_ops(
    *,
    start_x: int,
    start_y: int,
    end_x: int,
    end_y: int,
    graphic_style: object,
) -> list[KiCadPlotterOp]:
    """Emit ``SCH_PIN::PlotPinType`` geometry for one resolved pin shaft."""
    shape = _pin_style_value(graphic_style)
    mx, my = _pin_axis_step(start_x, start_y, end_x, end_y)
    radius = DEFAULT_PIN_SYMBOL_RADIUS_NM
    ops: list[KiCadPlotterOp] = []

    if shape in {"inverted", "inverted_clock"}:
        ops.append(_pin_circle_op(start_x + mx * radius, start_y + my * radius))
        line_start = (start_x + mx * radius * 2, start_y + my * radius * 2)
        if line_start != (end_x, end_y):
            ops.append(_pin_poly_op([line_start, (end_x, end_y)]))
    elif shape == "edge_clock_high":
        if my == 0:
            ops.append(
                _pin_poly_op([
                    (start_x, start_y + radius),
                    (start_x + mx * radius * 2, start_y),
                    (start_x, start_y - radius),
                ])
            )
        else:
            ops.append(
                _pin_poly_op([
                    (start_x + radius, start_y),
                    (start_x, start_y + my * radius * 2),
                    (start_x - radius, start_y),
                ])
            )
        line_start = (start_x + mx * radius * 2, start_y + my * radius * 2)
        if line_start != (end_x, end_y):
            ops.append(_pin_poly_op([line_start, (end_x, end_y)]))
    elif (start_x, start_y) != (end_x, end_y):
        ops.append(_pin_poly_op([(start_x, start_y), (end_x, end_y)]))

    if shape in {"clock", "inverted_clock", "clock_low"}:
        if my == 0:
            ops.append(
                _pin_poly_op([
                    (start_x, start_y + radius),
                    (start_x - mx * radius * 2, start_y),
                    (start_x, start_y - radius),
                ])
            )
        else:
            ops.append(
                _pin_poly_op([
                    (start_x + radius, start_y),
                    (start_x, start_y - my * radius * 2),
                    (start_x - radius, start_y),
                ])
            )

    if shape in {"input_low", "clock_low"}:
        if my == 0:
            ops.append(
                _pin_poly_op([
                    (start_x + mx * radius * 2, start_y),
                    (start_x + mx * radius * 2, start_y - radius * 2),
                    (start_x, start_y),
                ])
            )
        else:
            ops.append(
                _pin_poly_op([
                    (start_x, start_y + my * radius * 2),
                    (start_x - radius * 2, start_y + my * radius * 2),
                    (start_x, start_y),
                ])
            )

    if shape == "output_low":
        if my == 0:
            ops.append(
                _pin_poly_op([
                    (start_x, start_y - radius * 2),
                    (start_x + mx * radius * 2, start_y),
                ])
            )
        else:
            ops.append(
                _pin_poly_op([
                    (start_x - radius * 2, start_y),
                    (start_x, start_y + my * radius * 2),
                ])
            )
    elif shape == "non_logic":
        ops.append(
            _pin_poly_op([
                (start_x - (mx + my) * radius, start_y - (my - mx) * radius),
                (start_x + (mx + my) * radius, start_y + (my - mx) * radius),
            ])
        )
        ops.append(
            _pin_poly_op([
                (start_x - (mx - my) * radius, start_y - (my + mx) * radius),
                (start_x + (mx - my) * radius, start_y + (my + mx) * radius),
            ])
        )

    return ops


def _pin_to_ops_kicad_plot(
    pin: "SymPin",
    *,
    pin_names_offset: float,
    pin_names_hide: bool,
    pin_numbers_hide: bool,
) -> list[KiCadPlotterOp]:
    """Pin line/text placement matching SCH_PIN::PlotPinTexts."""
    angle = int(round(float(pin.at_angle))) % 360
    pos_x = mm_to_nm(pin.at_x)
    pos_y = y_to_ir(pin.at_y)
    length_nm = mm_to_nm(pin.length)

    if angle == 0:
        root_x, root_y = pos_x + length_nm, pos_y
    elif angle == 180:
        root_x, root_y = pos_x - length_nm, pos_y
    elif angle == 90:
        root_x, root_y = pos_x, pos_y - length_nm
    elif angle == 270:
        root_x, root_y = pos_x, pos_y + length_nm
    else:
        import math

        rad = math.radians(float(pin.at_angle))
        root_x = mm_to_nm(pin.at_x + pin.length * math.cos(rad))
        root_y = y_to_ir(pin.at_y + pin.length * math.sin(rad))

    ops: list[KiCadPlotterOp] = []
    ops.extend(
        pin_graphic_style_to_ops(
            start_x=root_x,
            start_y=root_y,
            end_x=pos_x,
            end_y=pos_y,
            graphic_style=pin.graphic_style,
        )
    )

    direction_x, direction_y = _pin_direction_local_nm(pin)
    horizontal, pin_right, pin_down = _pin_direction_flags(direction_x, direction_y)
    text_orient = 0.0 if horizontal else 90.0
    midpoint_x = (root_x + pos_x) // 2
    midpoint_y = (root_y + pos_y) // 2
    name = pin.name
    draws_name = bool(
        name
        and name != "~"
        and not pin_names_hide
        and _pin_effects_text_size_positive(pin.name_effects)
    )

    if (
        pin.number
        and not pin_numbers_hide
        and _pin_effects_text_size_positive(pin.number_effects)
    ):
        kwargs = _pin_number_text_kwargs(pin.number_effects)
        kwargs["h_align"] = KiCadHorizAlign.CENTER
        kwargs["v_align"] = KiCadVertAlign.BOTTOM
        text_clearance_nm = _pin_text_clearance_nm(kwargs)

        if pin_names_offset > 0 or not draws_name:
            num_x = midpoint_x if horizontal else root_x - text_clearance_nm
            num_y = root_y - text_clearance_nm if horizontal else midpoint_y
        elif horizontal:
            num_x = midpoint_x
            num_y = root_y + text_clearance_nm
            kwargs["v_align"] = KiCadVertAlign.TOP
        else:
            num_x = root_x + text_clearance_nm
            num_y = midpoint_y
            kwargs["v_align"] = KiCadVertAlign.TOP

        ops.append(
            KiCadPlotterOp.text(
                x=num_x,
                y=num_y,
                text=pin.number,
                orient_deg=text_orient,
                **kwargs,
            )
        )

    if draws_name:
        kwargs = apply_default_text_style(
            _effects_to_text_kwargs(pin.name_effects),
            LAYER_PINNAM,
            clamp_pen_width=False,
        )
        text_clearance_nm = _pin_text_clearance_nm(kwargs)

        if pin_names_offset > 0:
            offset_nm = mm_to_nm(pin_names_offset)
            if horizontal:
                name_x = root_x + offset_nm if pin_right else root_x - offset_nm
                name_y = root_y
                kwargs["h_align"] = (
                    KiCadHorizAlign.LEFT if pin_right else KiCadHorizAlign.RIGHT
                )
            else:
                name_x = root_x
                name_y = root_y + offset_nm if pin_down else root_y - offset_nm
                kwargs["h_align"] = (
                    KiCadHorizAlign.RIGHT if pin_down else KiCadHorizAlign.LEFT
                )
            kwargs["v_align"] = KiCadVertAlign.CENTER
        elif horizontal:
            name_x = midpoint_x
            name_y = root_y - text_clearance_nm
            kwargs["h_align"] = KiCadHorizAlign.CENTER
            kwargs["v_align"] = KiCadVertAlign.BOTTOM
        else:
            name_x = root_x - text_clearance_nm
            name_y = midpoint_y
            kwargs["h_align"] = KiCadHorizAlign.CENTER
            kwargs["v_align"] = KiCadVertAlign.BOTTOM

        ops.append(
            KiCadPlotterOp.text(
                x=name_x,
                y=name_y,
                text=name,
                orient_deg=text_orient,
                **kwargs,
            )
        )

    return ops


def pin_to_ops(
    pin: "SymPin",
    *,
    pin_names_offset: float = KICAD_DEFAULT_PIN_NAME_OFFSET_MM,
    pin_names_hide: bool = False,
    pin_numbers_hide: bool = False,
) -> list[KiCadPlotterOp]:
    """
    Convert a :class:`SymPin` into the ops needed to plot it.

    Emits the pin **wire** (line from external connection point to
    the body root) plus pin **number** and pin **name** Text ops at
    KiCad's canonical positions:

      * Number: midway along the shaft, offset perpendicularly by
        ``_PIN_NUMBER_OFFSET_MM``, rotated to read along the pin axis,
        centred. Suppressed when ``pin_numbers_hide`` is set or when
        the pin's ``number`` is empty.
      * Name (when ``pin_names_offset > 0``): inside the symbol body,
        starting at ``body_end + pin_names_offset`` along the pin
        direction, justified so the text reads outward toward the
        body interior. Suppressed when ``pin_names_hide`` is set, or
        the pin's ``name`` is empty / ``"~"``.
      * Name (when ``pin_names_offset == 0``): just past the pin tip
        on the outside, perpendicular offset, centred — matching
        KiCad's "draw outside" convention.

    Pin coords are .kicad_sym Y-up mm; the IR is Y-down nm — this
    helper applies the conversion via :func:`y_to_ir`.

    Pin graphic-style decorations (inverted bubble, clock triangle,
    etc.) are not emitted by this converter yet.
    """
    if pin.hide:
        return []
    return _pin_to_ops_kicad_plot(
        pin,
        pin_names_offset=pin_names_offset,
        pin_names_hide=pin_names_hide,
        pin_numbers_hide=pin_numbers_hide,
    )
    import math

    rad = math.radians(pin.at_angle)
    cos_t = math.cos(rad)
    sin_t = math.sin(rad)
    body_x_mm = pin.at_x + pin.length * cos_t
    body_y_mm = pin.at_y + pin.length * sin_t  # Y-up symbol coords

    ops: list[KiCadPlotterOp] = [
        styled_plotter_op(
            KiCadPlotterOp.plot_poly(
                points=[
                    (mm_to_nm(pin.at_x), y_to_ir(pin.at_y)),
                    (mm_to_nm(body_x_mm), y_to_ir(body_y_mm)),
                ],
                fill=KiCadFillType.NO_FILL,
                width_nm=DEFAULT_SCHEMATIC_TEXT_PEN_WIDTH_NM,
            ),
            stroke_color=LAYER_PIN,
        )
    ]

    # Pin orientation determines how text rotates so it reads along the pin.
    # Horizontal pins (at_angle 0 or 180): text orient = 0 (reads left→right).
    # Vertical pins (at_angle 90 or 270): text orient = 90 (reads bottom→top).
    text_orient = 90.0 if int(round(pin.at_angle)) % 180 == 90 else 0.0

    # ---- Pin number ------------------------------------------------------
    if pin.number and not pin_numbers_hide:
        # Shaft midpoint, perpendicular offset above the pin axis (Y-up).
        mid_x_mm = pin.at_x + (pin.length / 2.0) * cos_t
        mid_y_mm = pin.at_y + (pin.length / 2.0) * sin_t
        # Perpendicular = (-sin θ, cos θ) in Y-up.
        num_x_mm = mid_x_mm + _PIN_NUMBER_OFFSET_MM * -sin_t
        num_y_mm = mid_y_mm + _PIN_NUMBER_OFFSET_MM * cos_t

        kwargs = apply_default_text_style(
            _effects_to_text_kwargs(pin.number_effects), LAYER_PINNUM
        )
        kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
        kwargs.setdefault("v_align", KiCadVertAlign.BOTTOM)
        ops.append(
            KiCadPlotterOp.text(
                x=mm_to_nm(num_x_mm),
                y=y_to_ir(num_y_mm),
                text=pin.number,
                orient_deg=text_orient,
                **kwargs,
            )
        )

    # ---- Pin name --------------------------------------------------------
    name = pin.name
    if name and name != "~" and not pin_names_hide:
        kwargs = apply_default_text_style(
            _effects_to_text_kwargs(pin.name_effects), LAYER_PINNAM
        )
        if pin_names_offset > 0:
            # Name is inside the body, offset along the pin direction
            # past body_end. Justify so text reads toward symbol interior.
            name_x_mm = body_x_mm + pin_names_offset * cos_t
            name_y_mm = body_y_mm + pin_names_offset * sin_t
            # For horizontal pins: text reads horizontally; left-aligned for
            # right-pointing pins (θ=0), right-aligned for left-pointing (θ=180).
            # For vertical pins (orient=90): same logic on rotated axis —
            # angle 90 (Y-up "up", body above): text starts at name_pos and
            # reads "up" along the rotated axis → left-aligned in rotated
            # frame. angle 270: right-aligned in rotated frame.
            if text_orient == 0.0:
                if cos_t >= 0.0:
                    kwargs.setdefault("h_align", KiCadHorizAlign.LEFT)
                else:
                    kwargs.setdefault("h_align", KiCadHorizAlign.RIGHT)
            else:
                if sin_t >= 0.0:
                    kwargs.setdefault("h_align", KiCadHorizAlign.LEFT)
                else:
                    kwargs.setdefault("h_align", KiCadHorizAlign.RIGHT)
            kwargs.setdefault("v_align", KiCadVertAlign.CENTER)
        else:
            # Name is outside the body, perpendicular to pin axis at tip.
            name_x_mm = body_x_mm + _PIN_NUMBER_OFFSET_MM * -sin_t
            name_y_mm = body_y_mm + _PIN_NUMBER_OFFSET_MM * cos_t
            kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
            kwargs.setdefault("v_align", KiCadVertAlign.BOTTOM)

        ops.append(
            KiCadPlotterOp.text(
                x=mm_to_nm(name_x_mm),
                y=y_to_ir(name_y_mm),
                text=name,
                orient_deg=text_orient,
                **kwargs,
            )
        )

    return ops


# ---------------------------------------------------------------------------
# Top-level converters
# ---------------------------------------------------------------------------


def subsymbol_to_record(
    subsym: "LibSubSymbol",
    *,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
    default_polyline_stroke_width_nm: int = DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM,
    pin_names_offset: float = KICAD_DEFAULT_PIN_NAME_OFFSET_MM,
    pin_names_hide: bool = False,
    pin_numbers_hide: bool = False,
    pin_block_factory: Callable[["SymPin"], dict[str, Any] | None] | None = None,
    project_vars: dict | None = None,
) -> KiCadPlotterRecord:
    """
    Convert a :class:`LibSubSymbol` to a :class:`KiCadPlotterRecord`.

    Op ordering matches ``LIB_SYMBOL::Plot``: body fills, texts, pins, then
    deferred body outlines for filled primitives.

    Pin-text behaviour is controlled by the parent :class:`LibSymbol`'s
    ``pin_names_offset`` / ``pin_names_hide`` / ``pin_numbers_hide``
    flags (forwarded by :func:`lib_symbol_to_ir`); callers using
    ``subsymbol_to_record`` directly may pass them explicitly.
    """
    ops: list[KiCadPlotterOp] = []
    outline_ops: list[KiCadPlotterOp] = []

    def append_body_op(op: KiCadPlotterOp) -> None:
        fill_op, outline_op = _split_filled_outline_op(op)
        ops.append(fill_op)
        if outline_op is not None:
            outline_ops.append(outline_op)

    for r in subsym.rectangles:
        append_body_op(
            rectangle_to_op(r, default_stroke_width_nm=default_stroke_width_nm)
        )
    for c in subsym.circles:
        append_body_op(
            circle_to_op(c, default_stroke_width_nm=default_stroke_width_nm)
        )
    for a in subsym.arcs:
        append_body_op(
            arc_to_op(a, default_stroke_width_nm=default_stroke_width_nm)
        )
    for p in subsym.polylines:
        append_body_op(
            polyline_to_op(
                p,
                default_stroke_width_nm=default_polyline_stroke_width_nm,
            )
        )
    for b in subsym.beziers:
        op = bezier_to_op(b, default_stroke_width_nm=default_stroke_width_nm)
        if op is not None:
            append_body_op(op)
    for t in subsym.texts:
        op = text_to_op(t, project_vars=project_vars)
        if op is not None:
            ops.append(op)
    for pin in subsym.pins:
        pin_ops = pin_to_ops(
            pin,
            pin_names_offset=pin_names_offset,
            pin_names_hide=pin_names_hide,
            pin_numbers_hide=pin_numbers_hide,
        )
        if not pin_ops:
            continue
        block_kwargs = (
            pin_block_factory(pin) if pin_block_factory is not None else None
        )
        if block_kwargs:
            ops.append(KiCadPlotterOp.start_block(**block_kwargs))
            ops.extend(pin_ops)
            ops.append(KiCadPlotterOp.end_block())
        else:
            ops.extend(pin_ops)
    ops.extend(outline_ops)

    return KiCadPlotterRecord(
        uuid="",
        kind="lib_subsymbol",
        object_id=subsym.name,
        bounds=None,
        operations=ops,
        extras={
            "unit": int(subsym.unit),
            "style": int(subsym.style),
        },
    )


def lib_symbol_to_ir(
    symbol: "LibSymbol",
    *,
    unit: int | None = None,
    style: int = 0,
    source_path: str | None = None,
    document_id: str | None = None,
    default_stroke_width_nm: int = DEFAULT_SYMBOL_BODY_STROKE_WIDTH_NM,
    default_polyline_stroke_width_nm: int = DEFAULT_SYMBOL_POLYLINE_STROKE_WIDTH_NM,
    pin_block_factory: Callable[["SymPin"], dict[str, Any] | None] | None = None,
    project_vars: dict | None = None,
) -> KiCadPlotterDocument:
    """
    Render a :class:`LibSymbol` to a :class:`KiCadPlotterDocument`.

    ``unit`` selects which unit's subsymbols to include (plus the
    common ``unit=0`` subsymbol). When ``unit`` is ``None``, every
    matching-``style`` subsymbol is included.
    ``style`` follows KiCad's body-style convention (0 = normal, 1 =
    De Morgan).

    Each kept :class:`LibSubSymbol` becomes one :class:`KiCadPlotterRecord`
    with ``kind="lib_subsymbol"``; a leading record with
    ``kind="lib_symbol"`` carries top-level metadata so consumers can
    identify the source symbol without re-walking.
    """
    selected = _select_subsymbols(symbol.subsymbols, unit=unit, style=style)
    records: list[KiCadPlotterRecord] = [
        _symbol_header_record(symbol, unit=unit, style=style)
    ]
    for sub in selected:
        records.append(
            subsymbol_to_record(
                sub,
                default_stroke_width_nm=default_stroke_width_nm,
                default_polyline_stroke_width_nm=default_polyline_stroke_width_nm,
                pin_names_offset=symbol.pin_names_offset,
                pin_names_hide=symbol.pin_names_hide,
                pin_numbers_hide=symbol.pin_numbers_hide,
                pin_block_factory=pin_block_factory,
                project_vars=project_vars,
            )
        )

    return KiCadPlotterDocument(
        records=records,
        source_path=source_path,
        source_kind="SYM",
        document_id=document_id or symbol.name,
        canvas=None,
        coordinate_space={"unit": "nm", "y_axis": "down"},
        background_color=None,
        render_hints=None,
        extras={
            "selection": {
                "unit": unit,
                "style": int(style),
            },
        },
    )


def _select_subsymbols(
    subs: Iterable["LibSubSymbol"],
    *,
    unit: int | None,
    style: int,
) -> list["LibSubSymbol"]:
    """
    Filter subsymbols by unit/style, preserving source order.

    Matches KiCad's ``LIB_SYMBOL`` body-style rule
    (``bodyStyle == 0 || bodyStyle == requested``): subsymbols with
    ``style == 0`` are common to all body styles and are always
    included; subsymbols with ``style != 0`` are included only when
    they match the requested ``style`` exactly. The unit dimension
    follows the same wildcard rule (``unit == 0`` is common; otherwise
    must equal the requested unit, or unit-filter disabled when
    ``unit is None``).
    """
    requested_styles = {0, int(style)}
    if int(style) == 0:
        # KiCad library subsymbol names commonly encode normal body style as
        # ``_..._1``. Treat style=0 as the default-normal selector used by the
        # SVG API rather than as "common style records only".
        requested_styles.add(1)

    out: list["LibSubSymbol"] = []
    for s in subs:
        if s.style not in requested_styles:
            continue
        if unit is None or s.unit == 0 or s.unit == unit:
            out.append(s)
    return out


def _symbol_header_record(
    symbol: "LibSymbol",
    *,
    unit: int | None,
    style: int,
) -> KiCadPlotterRecord:
    """One leading record describing the source symbol (no draw ops)."""
    extras: dict = {
        "name": symbol.name,
        "extends": symbol.extends,
        "unit": unit,
        "style": int(style),
        "in_bom": symbol.in_bom,
        "on_board": symbol.on_board,
        "power": symbol.power,
    }
    return KiCadPlotterRecord(
        uuid="",
        kind="lib_symbol",
        object_id=symbol.name,
        bounds=None,
        operations=[],
        extras=extras,
    )


__all__ = [
    "arc_to_op",
    "bezier_to_op",
    "circle_to_op",
    "lib_symbol_to_ir",
    "mm_to_nm",
    "pin_graphic_style_to_ops",
    "pin_to_ops",
    "polyline_to_op",
    "polyline_to_op_from_points",
    "rectangle_to_op",
    "rgba_to_hex",
    "stroke_type_to_line_style",
    "stroke_width_nm",
    "subsymbol_to_record",
    "sym_fill_to_kicad_fill",
    "text_to_op",
    "y_to_ir",
]

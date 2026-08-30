"""
KiCadFootprint to KiCadPlotterDocument converter.

Walks a parsed :class:`KiCadFootprint` (`.kicad_mod` standalone footprint
file or PCB-embedded footprint) and emits a :class:`KiCadPlotterDocument`
whose record carries :class:`KiCadPlotterOp` instances drawn from the
PLOTTER vocabulary. This is the parser to IR boundary for
footprints; downstream rendering (`render_ir_to_svg`) consumes the IR.

Mirrors KiCad's footprint emit order (``pcb_io_kicad_sexpr.cpp`` lines
1130-1448 / ``KiCadFootprint.to_sexp``):

    properties → fp_texts → fp_lines → fp_arcs → fp_circles →
    fp_rects → fp_polys → pads

Coordinate convention: unlike `.kicad_sym` (Y-up), `.kicad_mod` /
`.kicad_pcb` files store positions in mm with **Y-down** already
matching KiCad's PLOTTER convention. We multiply by 1_000_000 at the
boundary but do NOT negate Y. This keeps the IR's
``coordinate_space={"unit":"nm", "y_axis":"down"}`` invariant.

Stroke widths translate to nm with KiCad's minimum plot pen width floor.
Zero-width strokes use the schematic/body default before that floor is
applied.

Pad shapes dispatch onto the ``FlashPad*`` op family:
    CIRCLE     → FlashPadCircle
    OVAL       → FlashPadOval
    RECT       → FlashPadRect
    ROUNDRECT  → FlashPadRoundRect (corner_radius from roundrect_rratio)
    TRAPEZOID  → FlashPadTrapez (rect_delta converted to local corners)
    CUSTOM     → FlashPadCustom (gr_poly primitives translated; non-poly
                                  primitives currently dropped)

Hidden ``Property`` and ``FpText`` items (``hide=True``) are skipped,
mirroring ``PCB_FIELD::Plot`` / ``PCB_TEXT::Plot``. Empty-value
properties are also skipped.
"""

from __future__ import annotations

import copy
import math
from enum import Enum
from typing import TYPE_CHECKING, Any, List, cast

from .kicad_base import FillType, PadShape, PadType, StrokeType
from .kicad_lib_symbol_to_ir import (
    _effects_to_text_kwargs,
    mm_to_nm,
    stroke_width_nm,
)
from .kicad_plotter_ir import (
    KiCadFillType,
    KiCadHorizAlign,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterRecord,
    KiCadVertAlign,
)
from .kicad_stroke_decompose import (
    decompose_arc,
    decompose_segment,
    is_decomposable_style,
)
from .kicad_primitives import Stroke
from .kicad_text_variables import (
    object_property_text_variables,
    substitute_text_variables,
)

if TYPE_CHECKING:
    from .kicad_footprint import KiCadFootprint
    from .kicad_fp_arc import FpArc
    from .kicad_fp_circle import FpCircle
    from .kicad_fp_line import FpLine
    from .kicad_fp_poly import FpPoly
    from .kicad_fp_rect import FpRect
    from .kicad_fp_text import FpText
    from .kicad_pad import Pad
    from .kicad_property import Property


# ---------------------------------------------------------------------------
# Enum mapping
# ---------------------------------------------------------------------------


_FP_FILL_TO_KICAD_FILL: dict[FillType, KiCadFillType] = {
    FillType.NONE: KiCadFillType.NO_FILL,
    FillType.NO: KiCadFillType.NO_FILL,
    FillType.YES: KiCadFillType.FILLED_SHAPE,
    FillType.SOLID: KiCadFillType.FILLED_SHAPE,
}


def fp_fill_to_kicad_fill(fill: FillType) -> KiCadFillType:
    """Map a parser :class:`FillType` (PCB-side) to the IR FILL_T mirror."""
    return _FP_FILL_TO_KICAD_FILL.get(fill, KiCadFillType.NO_FILL)


# ---------------------------------------------------------------------------
# Per-shape op emitters
# ---------------------------------------------------------------------------


def fp_line_to_op(line: "FpLine") -> KiCadPlotterOp:
    """Convert an :class:`FpLine` into a ``ThickSegment`` op (solid only)."""
    return KiCadPlotterOp.thick_segment(
        start_x=mm_to_nm(line.start_x),
        start_y=mm_to_nm(line.start_y),
        end_x=mm_to_nm(line.end_x),
        end_y=mm_to_nm(line.end_y),
        width_nm=stroke_width_nm(line.stroke),
    )


def fp_line_to_ops(line: "FpLine") -> list[KiCadPlotterOp]:
    """Convert an :class:`FpLine` to one or more ``ThickSegment`` ops.

    Mirrors :func:`gr_line_to_ops` for footprint-local fp_line records:
    SOLID/DEFAULT → single op; DASH/DOT/DASH_DOT/DASH_DOT_DOT → per-dash
    sub-segments.
    """
    style = line.stroke.type if line.stroke else None
    width_nm = stroke_width_nm(line.stroke)
    if not is_decomposable_style(style):
        return [fp_line_to_op(line)]
    decomposable_style = cast(StrokeType, style)
    pieces = decompose_segment(
        mm_to_nm(line.start_x), mm_to_nm(line.start_y),
        mm_to_nm(line.end_x), mm_to_nm(line.end_y),
        width_nm,
        decomposable_style,
    )
    if not pieces:
        return [fp_line_to_op(line)]
    return [
        KiCadPlotterOp.thick_segment(
            start_x=sx, start_y=sy, end_x=ex, end_y=ey, width_nm=width_nm,
        )
        for sx, sy, ex, ey in pieces
    ]


def fp_arc_to_op(arc: "FpArc") -> KiCadPlotterOp:
    """Convert an :class:`FpArc` into an ``ArcThreePoint`` op (solid only)."""
    return KiCadPlotterOp.arc_three_point(
        start_x=mm_to_nm(arc.start_x),
        start_y=mm_to_nm(arc.start_y),
        mid_x=mm_to_nm(arc.mid_x),
        mid_y=mm_to_nm(arc.mid_y),
        end_x=mm_to_nm(arc.end_x),
        end_y=mm_to_nm(arc.end_y),
        fill=KiCadFillType.NO_FILL,  # fp_arc has no fill semantics
        width_nm=stroke_width_nm(arc.stroke),
    )


def fp_arc_to_ops(arc: "FpArc") -> list[KiCadPlotterOp]:
    """Convert an :class:`FpArc` to one or more ops.

    SOLID/DEFAULT → single ``ArcThreePoint`` op; non-solid styles → chord
    sub-segments via :func:`decompose_arc`.
    """
    style = arc.stroke.type if arc.stroke else None
    width_nm = stroke_width_nm(arc.stroke)
    if not is_decomposable_style(style):
        return [fp_arc_to_op(arc)]
    decomposable_style = cast(StrokeType, style)
    pieces = decompose_arc(
        mm_to_nm(arc.start_x), mm_to_nm(arc.start_y),
        mm_to_nm(arc.mid_x), mm_to_nm(arc.mid_y),
        mm_to_nm(arc.end_x), mm_to_nm(arc.end_y),
        width_nm,
        decomposable_style,
    )
    if not pieces:
        return [fp_arc_to_op(arc)]
    return [
        KiCadPlotterOp.thick_segment(
            start_x=sx, start_y=sy, end_x=ex, end_y=ey, width_nm=width_nm,
        )
        for sx, sy, ex, ey in pieces
    ]


def fp_circle_to_op(circle: "FpCircle") -> KiCadPlotterOp:
    """
    Convert an :class:`FpCircle` into a ``Circle`` op.

    Radius is recovered from the (center, end) pair via Euclidean
    distance, then doubled for the IR's diameter convention.
    """
    diameter_nm = mm_to_nm(circle.radius * 2.0)
    return KiCadPlotterOp.circle(
        cx=mm_to_nm(circle.center_x),
        cy=mm_to_nm(circle.center_y),
        diameter_nm=diameter_nm,
        fill=fp_fill_to_kicad_fill(circle.fill),
        width_nm=stroke_width_nm(circle.stroke),
    )


def fp_rect_to_op(rect: "FpRect") -> KiCadPlotterOp:
    """Convert an :class:`FpRect` into a ``Rect`` op."""
    return KiCadPlotterOp.rect(
        x1=mm_to_nm(rect.start_x),
        y1=mm_to_nm(rect.start_y),
        x2=mm_to_nm(rect.end_x),
        y2=mm_to_nm(rect.end_y),
        fill=fp_fill_to_kicad_fill(rect.fill),
        width_nm=stroke_width_nm(rect.stroke),
    )


def fp_poly_to_op(poly: "FpPoly") -> KiCadPlotterOp:
    """Convert an :class:`FpPoly` into a ``PlotPoly`` op."""
    points = [(mm_to_nm(x), mm_to_nm(y)) for x, y in poly.points]
    return KiCadPlotterOp.plot_poly(
        points=points,
        fill=fp_fill_to_kicad_fill(poly.fill),
        width_nm=stroke_width_nm(poly.stroke),
    )


def _resolved_fp_text_value(
    text_type: object,
    raw_text: object,
    variables: dict[str, str] | None,
) -> str:
    raw = str(raw_text or "")
    lookup = variables or {}
    kind = str(text_type or "")
    if kind == "reference":
        raw = lookup.get("Reference", lookup.get("REFERENCE", raw))
    elif kind == "value":
        raw = lookup.get("Value", lookup.get("VALUE", raw))
    return substitute_text_variables(raw, lookup)


def fp_text_to_op(
    text: "FpText",
    *,
    variables: dict[str, str] | None = None,
) -> KiCadPlotterOp | None:
    """
    Convert an :class:`FpText` into a ``Text`` op.

    Returns ``None`` when ``text.hide`` is True or ``text.text`` is empty,
    mirroring KiCad's ``PCB_TEXT::Plot`` skip rule.
    """
    resolved_text = _resolved_fp_text_value(text.text_type, text.text, variables)
    if text.hide or not resolved_text:
        return None
    kwargs = _effects_to_text_kwargs(text.effects)
    return KiCadPlotterOp.text(
        x=mm_to_nm(text.at_x),
        y=mm_to_nm(text.at_y),
        text=resolved_text,
        orient_deg=float(text.at_angle),
        **kwargs,
    )


def fp_text_box_to_ops(
    text_box: Any,
    *,
    variables: dict[str, str] | None = None,
    default_h_align: KiCadHorizAlign = KiCadHorizAlign.LEFT,
    default_v_align: KiCadVertAlign = KiCadVertAlign.TOP,
) -> List[KiCadPlotterOp]:
    """Convert an ``fp_text_box`` into optional border and text ops."""
    ops: List[KiCadPlotterOp] = []
    if text_box.border:
        ops.append(
            KiCadPlotterOp.rect(
                x1=mm_to_nm(text_box.start_x),
                y1=mm_to_nm(text_box.start_y),
                x2=mm_to_nm(text_box.end_x),
                y2=mm_to_nm(text_box.end_y),
                fill=KiCadFillType.NO_FILL,
                width_nm=stroke_width_nm(
                    text_box.stroke if text_box.stroke is not None else Stroke(),
                    default_width_nm=mm_to_nm(0.2),
                ),
            )
        )

    if not text_box.text:
        return ops

    kwargs = _effects_to_text_kwargs(text_box.effects)
    h_align = kwargs.get("h_align", default_h_align)
    v_align = kwargs.get("v_align", default_v_align)
    kwargs["h_align"] = h_align
    kwargs["v_align"] = v_align

    x1 = min(text_box.start_x, text_box.end_x)
    y1 = min(text_box.start_y, text_box.end_y)
    x2 = max(text_box.start_x, text_box.end_x)
    y2 = max(text_box.start_y, text_box.end_y)
    margin_left, margin_top, margin_right, margin_bottom = text_box.margins

    if h_align == KiCadHorizAlign.RIGHT:
        x = x2 - margin_right
    elif h_align == KiCadHorizAlign.CENTER:
        x = (x1 + x2) / 2.0
    else:
        x = x1 + margin_left

    if v_align == KiCadVertAlign.BOTTOM:
        y = y2 - margin_bottom
    elif v_align == KiCadVertAlign.CENTER:
        y = (y1 + y2) / 2.0
    else:
        y = y1 + margin_top

    resolved_text = substitute_text_variables(text_box.text, variables or {})
    text_lines = _wrap_text_box_lines(
        resolved_text,
        max_width_mm=max(0.0, (x2 - x1) - margin_left - margin_right),
        size_x_mm=float((kwargs.get("size_x_nm") or mm_to_nm(1.27)) / 1_000_000.0),
    )
    ops.append(
        KiCadPlotterOp.text(
            x=mm_to_nm(x),
            y=mm_to_nm(y),
            text="\n".join(text_lines),
            orient_deg=float(getattr(text_box, "angle", 0.0)),
            multiline=len(text_lines) > 1 or "\n" in resolved_text,
            **kwargs,
        )
    )
    return ops


def _wrap_text_box_lines(
    text: str,
    *,
    max_width_mm: float,
    size_x_mm: float,
) -> list[str]:
    if max_width_mm <= 0.0 or " " not in text:
        return text.split("\n")

    from .kicad_stroke_font import get_renderer

    renderer = get_renderer()

    def _width_mm(value: str) -> float:
        return float(renderer._calculate_text_width(value)) * float(size_x_mm)

    lines: list[str] = []
    for paragraph in text.split("\n"):
        words = paragraph.split(" ")
        current = ""
        for word in words:
            candidate = word if not current else current + " " + word
            if current and _width_mm(candidate) > max_width_mm:
                lines.append(current)
                current = word
            else:
                current = candidate
        lines.append(current)
    return lines


def property_to_op(prop: "Property") -> KiCadPlotterOp | None:
    """
    Convert a footprint :class:`Property` into a ``Text`` op.

    Returns ``None`` when ``prop.hide`` is True or ``prop.value`` is empty,
    mirroring KiCad's ``PCB_FIELD::Plot`` skip rule.
    """
    if prop.hide or not prop.value or not getattr(prop, "graphical", True):
        return None
    kwargs = _effects_to_text_kwargs(prop.effects)
    return KiCadPlotterOp.text(
        x=mm_to_nm(prop.at_x),
        y=mm_to_nm(prop.at_y),
        text=prop.value,
        orient_deg=float(prop.at_angle),
        **kwargs,
    )


def _footprint_text_variables(properties: list["Property"]) -> dict[str, str]:
    return object_property_text_variables(properties)


def _op_with_layer(op: KiCadPlotterOp, layer: str) -> KiCadPlotterOp:
    payload = copy.deepcopy(op.payload)
    payload["layer"] = str(layer)
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _op_with_layers(op: KiCadPlotterOp, layers: list[str]) -> KiCadPlotterOp:
    payload = copy.deepcopy(op.payload)
    payload["layers"] = [str(layer) for layer in layers]
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _resolved_pad_mask_margin_nm(
    pad: "Pad",
    footprint: "KiCadFootprint",
) -> int:
    margin = getattr(pad, "solder_mask_margin", None)
    if margin is None:
        margin = getattr(footprint, "solder_mask_margin", None)
    if margin is None:
        margin = 0.0

    min_size = min(
        float(getattr(pad, "size_x", 0.0)),
        float(getattr(pad, "size_y", 0.0)),
    )
    margin = max(float(margin), -min_size / 2.0)
    return mm_to_nm(margin)


def _op_with_pad_mask_hints(
    op: KiCadPlotterOp,
    pad: "Pad",
    footprint: "KiCadFootprint",
) -> KiCadPlotterOp:
    kind = op.kind.value if isinstance(op.kind, Enum) else str(op.kind)
    role = str(op.payload.get("role", ""))
    if not kind.startswith("FlashPad") and role != "npth_hole":
        return op
    payload = copy.deepcopy(op.payload)
    payload["mask_margin_nm"] = _resolved_pad_mask_margin_nm(pad, footprint)
    if role == "npth_hole":
        payload["pad_size_x_nm"] = mm_to_nm(float(getattr(pad, "size_x", 0.0)))
        payload["pad_size_y_nm"] = mm_to_nm(float(getattr(pad, "size_y", 0.0)))
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def pad_to_ops(
    pad: "Pad",
    *,
    orient_deg_offset: float = 0.0,
) -> List[KiCadPlotterOp]:
    """
    Convert a :class:`Pad` into one or more ``FlashPad*`` ops.

    Dispatches on :attr:`Pad.shape`:

    * CIRCLE     → ``FlashPadCircle`` (uses ``size_x`` as diameter)
    * OVAL       → ``FlashPadOval``
    * RECT       → ``FlashPadRect``
    * ROUNDRECT  → ``FlashPadRoundRect`` with
      ``corner_radius_nm = min(size_x, size_y) * roundrect_rratio``
      (defaults rratio to 0.25 when missing)
    * TRAPEZOID  → ``FlashPadTrapez`` with KiCad's ``rect_delta``
      converted to pad-local corners
    * CUSTOM     → ``FlashPadCustom`` with each ``gr_poly`` primitive's
      points expressed in pad-local nm. Non-``gr_poly`` primitives are
      dropped.

    Pad center is at (``at_x``, ``at_y``); ``at_angle`` plus
    ``orient_deg_offset`` becomes ``orient_deg`` on the flash op so consumers
    can re-derive the rotated geometry. Returns an empty list for unhandled
    shapes (forward-compat).
    """
    drill = getattr(pad, "drill", None)
    if (
        _pad_type_value(pad) == PadType.NP_THRU_HOLE.value
        and pad.shape == PadShape.CIRCLE
        and drill is not None
        and max(float(pad.size_x), float(pad.size_y)) <= float(drill)
    ):
        return []

    x = mm_to_nm(pad.at_x)
    y = mm_to_nm(pad.at_y)
    size_x_nm = mm_to_nm(pad.size_x)
    size_y_nm = mm_to_nm(pad.size_y)
    orient_deg = float(pad.at_angle) + float(orient_deg_offset)
    offset_x_nm, offset_y_nm = _rotate_pad_local_nm(
        mm_to_nm(pad.drill_offset_x or 0.0),
        mm_to_nm(pad.drill_offset_y or 0.0),
        orient_deg,
    )
    x += offset_x_nm
    y += offset_y_nm

    shape = pad.shape

    if shape == PadShape.CIRCLE:
        # KiCad uses size_x as the circle diameter (size_y mirrors it).
        return [
            KiCadPlotterOp.flash_pad_circle(
                x=x, y=y, diameter_nm=size_x_nm,
            )
        ]
    if shape == PadShape.OVAL:
        return [
            KiCadPlotterOp.flash_pad_oval(
                x=x, y=y,
                size_x_nm=size_x_nm, size_y_nm=size_y_nm,
                orient_deg=orient_deg,
            )
        ]
    if shape == PadShape.RECT:
        return [
            KiCadPlotterOp.flash_pad_rect(
                x=x, y=y,
                size_x_nm=size_x_nm, size_y_nm=size_y_nm,
                orient_deg=orient_deg,
            )
        ]
    if shape == PadShape.TRAPEZOID:
        half_x = int(size_x_nm / 2)
        half_y = int(size_y_nm / 2)
        delta_x = int(mm_to_nm(pad.rect_delta_x or 0.0) / 2)
        delta_y = int(mm_to_nm(pad.rect_delta_y or 0.0) / 2)
        corners = [
            [-half_x - delta_y, half_y + delta_x],
            [half_x + delta_y, half_y - delta_x],
            [half_x - delta_y, -half_y + delta_x],
            [-half_x + delta_y, -half_y - delta_x],
        ]
        return [
            KiCadPlotterOp.flash_pad_trapez(
                x=x,
                y=y,
                corners=corners,
                orient_deg=orient_deg,
            )
        ]
    if shape == PadShape.ROUNDRECT:
        rratio = pad.roundrect_rratio if pad.roundrect_rratio is not None else 0.25
        shorter_nm = min(size_x_nm, size_y_nm)
        corner_radius_nm = int(round(shorter_nm * rratio))

        # KiCad chamfered roundrect pads (typically rratio≈0 with non-empty
        # chamfer_corners + chamfer_ratio>0) are emitted by kicad-cli's
        # PCB_PLOTTER::PlotPad as a single 5..8-vertex polygon. Mirror that
        # structurally by emitting flash_pad_custom in pad-local nm.
        chamfer_ratio = getattr(pad, "chamfer_ratio", None)
        chamfer_corners = getattr(pad, "chamfer_corners", None) or []
        if (
            chamfer_corners
            and chamfer_ratio is not None
            and chamfer_ratio > 0
            and corner_radius_nm < 1
        ):
            chamfer_polygon = _chamfered_pad_local_polygon_nm(
                size_x_nm=size_x_nm,
                size_y_nm=size_y_nm,
                chamfer_ratio=float(chamfer_ratio),
                chamfer_corners=chamfer_corners,
            )
            return [
                KiCadPlotterOp.flash_pad_custom(
                    x=x, y=y,
                    size_x_nm=size_x_nm, size_y_nm=size_y_nm,
                    orient_deg=orient_deg,
                    polygons=[chamfer_polygon],
                )
            ]
        return [
            KiCadPlotterOp.flash_pad_roundrect(
                x=x, y=y,
                size_x_nm=size_x_nm, size_y_nm=size_y_nm,
                corner_radius_nm=corner_radius_nm,
                orient_deg=orient_deg,
            )
        ]
    if shape == PadShape.CUSTOM:
        polygons: list[list[list[int]]] = []
        polygon_widths_nm: list[int] = []
        for prim in pad.custom_primitives:
            if prim.primitive_type != "gr_poly" or not prim.points:
                continue
            polygons.append([[mm_to_nm(px), mm_to_nm(py)] for px, py in prim.points])
            polygon_widths_nm.append(mm_to_nm(float(prim.width or 0.0)))
        anchor_shape = None
        if pad.custom_options is not None:
            anchor_shape = pad.custom_options.anchor
        return [
            KiCadPlotterOp.flash_pad_custom(
                x=x, y=y,
                size_x_nm=size_x_nm, size_y_nm=size_y_nm,
                orient_deg=orient_deg,
                polygons=polygons,
                polygon_widths_nm=polygon_widths_nm,
                anchor_shape=anchor_shape,
            )
        ]
    # Unknown shape — be forward-compatible.
    return []


def _chamfered_pad_local_polygon_nm(
    *,
    size_x_nm: int,
    size_y_nm: int,
    chamfer_ratio: float,
    chamfer_corners: List[str],
) -> List[List[int]]:
    """Build a chamfered rectangle polygon in pad-local nm coordinates.

    Mirrors KiCad's ``TransformRoundChamferedRectToPolygon`` corner mutation
    for the rratio≈0 chamfer case. ``chamfer_corners`` is the s-expression
    list (top_left / top_right / bottom_left / bottom_right). The output is
    centered at origin; the caller folds in pad center + orient via the
    flash op payload.
    """
    half_w = size_x_nm / 2.0
    half_h = size_y_nm / 2.0
    shorter = min(size_x_nm, size_y_nm)
    chamfer = max(0.0, chamfer_ratio * shorter)
    if chamfer <= 0:
        return [
            [int(round(-half_w)), int(round(-half_h))],
            [int(round(half_w)), int(round(-half_h))],
            [int(round(half_w)), int(round(half_h))],
            [int(round(-half_w)), int(round(half_h))],
        ]

    # Start CW from top-left to mirror legacy pad_to_chamfered_rect_polygon.
    corners: list[dict[str, float]] = [
        {"x": -half_w, "y": -half_h},
        {"x": half_w, "y": -half_h},
        {"x": half_w, "y": half_h},
        {"x": -half_w, "y": half_h},
    ]
    chamfer_set = set(chamfer_corners)
    corner_names = ["top_left", "top_right", "bottom_right", "bottom_left"]
    sign = [0, 1, -1, 0, 0, -1, 1, 0]

    chamfer_count = sum(1 for name in corner_names if name in chamfer_set)
    pos = 0
    for cc, name in enumerate(corner_names):
        if name not in chamfer_set:
            pos += 1
            continue
        corners.insert(pos + 1, dict(corners[pos]))
        corners[pos]["x"] += sign[(2 * cc) & 7] * chamfer
        corners[pos]["y"] += sign[(2 * cc - 2) & 7] * chamfer
        corners[pos + 1]["x"] += sign[(2 * cc + 1) & 7] * chamfer
        corners[pos + 1]["y"] += sign[(2 * cc - 1) & 7] * chamfer
        pos += 2

    if chamfer_count > 1 and 2 * chamfer >= shorter:
        dedup: list[dict[str, float]] = []
        for pt in corners:
            if not dedup:
                dedup.append(pt)
                continue
            if abs(pt["x"] - dedup[-1]["x"]) > 1e-9 or abs(pt["y"] - dedup[-1]["y"]) > 1e-9:
                dedup.append(pt)
        if (
            len(dedup) > 1
            and abs(dedup[0]["x"] - dedup[-1]["x"]) < 1e-9
            and abs(dedup[0]["y"] - dedup[-1]["y"]) < 1e-9
        ):
            dedup.pop()
        corners = dedup

    return [
        [int(round(pt["x"])), int(round(pt["y"]))]
        for pt in corners
    ]


def _pad_type_value(pad: "Pad") -> str:
    pad_type = pad.pad_type
    return pad_type.value if isinstance(pad_type, PadType) else str(pad_type)


def _rotate_pad_local_nm(x_nm: int, y_nm: int, angle_deg: float) -> tuple[int, int]:
    if angle_deg == 0.0:
        return x_nm, y_nm
    theta = math.radians(-angle_deg)
    cos_a = math.cos(theta)
    sin_a = math.sin(theta)
    return (
        int(round(x_nm * cos_a - y_nm * sin_a)),
        int(round(x_nm * sin_a + y_nm * cos_a)),
    )


def _drill_circle_op(
    *,
    cx: int,
    cy: int,
    diameter_nm: int,
    role: str,
) -> KiCadPlotterOp:
    op = KiCadPlotterOp.circle(
        cx=cx,
        cy=cy,
        diameter_nm=diameter_nm,
        fill=KiCadFillType.FILLED_SHAPE,
        width_nm=0,
    )
    return KiCadPlotterOp(kind=op.kind, payload={**op.payload, "role": role})


def _drill_slot_op(
    *,
    cx: int,
    cy: int,
    orient_deg: float,
    drill_width_nm: int,
    drill_height_nm: int,
    role: str,
) -> KiCadPlotterOp | None:
    major = max(drill_width_nm, drill_height_nm)
    minor = min(drill_width_nm, drill_height_nm)
    if major <= 0 or minor <= 0:
        return None

    theta = math.radians(-orient_deg)
    if drill_height_nm > drill_width_nm:
        theta += math.pi / 2.0

    half_length = max(0.0, (major - minor) / 2.0)
    dx = int(round(math.cos(theta) * half_length))
    dy = int(round(math.sin(theta) * half_length))
    if dx == 0 and dy == 0:
        return _drill_circle_op(cx=cx, cy=cy, diameter_nm=minor, role=role)

    op = KiCadPlotterOp.thick_segment(
        start_x=cx - dx,
        start_y=cy - dy,
        end_x=cx + dx,
        end_y=cy + dy,
        width_nm=minor,
    )
    return KiCadPlotterOp(kind=op.kind, payload={**op.payload, "role": role})


def pad_drill_to_ops(
    pad: "Pad",
    *,
    orient_deg_offset: float = 0.0,
) -> List[KiCadPlotterOp]:
    """Emit synthetic drill/hole overlay ops for through-hole pads."""
    pad_type = _pad_type_value(pad)
    if pad_type not in (PadType.THRU_HOLE.value, PadType.NP_THRU_HOLE.value):
        return []

    role = "npth_hole" if pad_type == PadType.NP_THRU_HOLE.value else "pad_drill"
    cx = mm_to_nm(pad.at_x)
    cy = mm_to_nm(pad.at_y)
    orient_deg = float(pad.at_angle) + float(orient_deg_offset)

    drill_width = getattr(pad, "drill_width", None)
    drill_height = getattr(pad, "drill_height", None)
    if (
        bool(getattr(pad, "drill_oval", False))
        and drill_width is not None
        and drill_height is not None
        and drill_width > 0
        and drill_height > 0
    ):
        op = _drill_slot_op(
            cx=cx,
            cy=cy,
            orient_deg=orient_deg,
            drill_width_nm=mm_to_nm(float(drill_width)),
            drill_height_nm=mm_to_nm(float(drill_height)),
            role=role,
        )
        return [op] if op is not None else []

    drill = getattr(pad, "drill", None)
    if drill and drill > 0:
        return [
            _drill_circle_op(
                cx=cx,
                cy=cy,
                diameter_nm=mm_to_nm(drill),
                role=role,
            )
        ]

    if pad_type == PadType.NP_THRU_HOLE.value:
        fallback = min(getattr(pad, "size_x", 0.0), getattr(pad, "size_y", 0.0))
        if fallback > 0:
            return [
                _drill_circle_op(
                    cx=cx,
                    cy=cy,
                    diameter_nm=mm_to_nm(fallback),
                    role=role,
                )
            ]

    return []


# ---------------------------------------------------------------------------
# Top-level converters
# ---------------------------------------------------------------------------


def footprint_to_record(footprint: "KiCadFootprint") -> KiCadPlotterRecord:
    """
    Convert a :class:`KiCadFootprint` to a :class:`KiCadPlotterRecord`.

    Op ordering matches ``KiCadFootprint.to_sexp``:

        properties → fp_texts → fp_lines → fp_arcs → fp_circles →
        fp_rects → fp_polys → pads
    """
    ops: list[KiCadPlotterOp] = []
    variables = _footprint_text_variables(footprint.properties)

    # Reference + Value first (matches to_sexp ordering), then others.
    ref_prop = next((p for p in footprint.properties if p.name == "Reference"), None)
    val_prop = next((p for p in footprint.properties if p.name == "Value"), None)
    other_props = [
        p for p in footprint.properties if p.name not in ("Reference", "Value")
    ]
    for prop in [p for p in (ref_prop, val_prop) if p is not None] + other_props:
        op = property_to_op(prop)
        if op is not None:
            ops.append(_op_with_layer(op, prop.layer))

    for txt in footprint.fp_texts:
        op = fp_text_to_op(txt, variables=variables)
        if op is not None:
            ops.append(_op_with_layer(op, txt.layer))

    for text_box in getattr(footprint, "fp_text_boxes", []) or []:
        ops.extend(
            _op_with_layer(op, text_box.layer)
            for op in fp_text_box_to_ops(text_box, variables=variables)
        )
    for line in footprint.fp_lines:
        for op in fp_line_to_ops(line):
            ops.append(_op_with_layer(op, line.layer))
    for arc in footprint.fp_arcs:
        for op in fp_arc_to_ops(arc):
            ops.append(_op_with_layer(op, arc.layer))
    for circle in footprint.fp_circles:
        ops.append(_op_with_layer(fp_circle_to_op(circle), circle.layer))
    for rect in footprint.fp_rects:
        ops.append(_op_with_layer(fp_rect_to_op(rect), rect.layer))
    for poly in footprint.fp_polys:
        ops.append(_op_with_layer(fp_poly_to_op(poly), poly.layer))
    for pad in footprint.pads:
        ops.extend(
            _op_with_pad_mask_hints(
                _op_with_layers(op, list(pad.layers)),
                pad,
                footprint,
            )
            for op in [*pad_to_ops(pad), *pad_drill_to_ops(pad)]
        )

    extras: dict = {
        "name": footprint.name,
        "layer": footprint.layer,
        "locked": bool(footprint.locked),
        "placed": bool(footprint.placed),
        "descr": footprint.descr,
        "tags": footprint.tags,
        "attr": list(footprint.attr) if footprint.attr else [],
    }

    return KiCadPlotterRecord(
        uuid=footprint.uuid or "",
        kind="footprint",
        object_id=footprint.name,
        bounds=None,
        operations=ops,
        extras=extras,
    )


def footprint_to_ir(
    footprint: "KiCadFootprint",
    *,
    source_path: str | None = None,
    document_id: str | None = None,
) -> KiCadPlotterDocument:
    """
    Render a :class:`KiCadFootprint` to a :class:`KiCadPlotterDocument`.

    Emits a single record (``kind="footprint"``) carrying every
    geometry op for the footprint, in KiCad's canonical emit order.
    A footprint has no unit/style axis, so a single record is sufficient.
    """
    record = footprint_to_record(footprint)
    return KiCadPlotterDocument(
        records=[record],
        source_path=source_path,
        source_kind="MOD",
        document_id=document_id or footprint.name,
        canvas=None,
        coordinate_space={"unit": "nm", "y_axis": "down"},
        background_color=None,
        render_hints=None,
        extras={
            "version": int(footprint.version),
            "generator": str(footprint.generator),
            "generator_version": str(footprint.generator_version),
        },
    )


__all__ = [
    "footprint_to_ir",
    "footprint_to_record",
    "fp_arc_to_op",
    "fp_circle_to_op",
    "fp_fill_to_kicad_fill",
    "fp_line_to_op",
    "fp_poly_to_op",
    "fp_rect_to_op",
    "fp_text_box_to_ops",
    "fp_text_to_op",
    "pad_drill_to_ops",
    "pad_to_ops",
    "property_to_op",
]

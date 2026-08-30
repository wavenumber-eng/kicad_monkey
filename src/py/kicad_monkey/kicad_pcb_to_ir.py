"""
KiCadPcb to KiCadPlotterDocument converter.

Walks a parsed :class:`KiCadPcb` (`.kicad_pcb` board file) and emits a
:class:`KiCadPlotterDocument` whose records carry the PLOTTER
op vocabulary. This is the parser to IR boundary for full boards;
downstream rendering (`render_ir_to_svg`) consumes the IR.

Record layout (one record per source item, for traceability):

    * board-level graphics
        - ``gr_line``    → 1 ``ThickSegment`` op
        - ``gr_arc``     → 1 ``ArcThreePoint`` op (with stroke width)
        - ``gr_circle``  → 1 ``Circle`` op (radius from (centre, end))
        - ``gr_rect``    → 1 ``Rect`` op
        - ``gr_poly``    → 1 ``PlotPoly`` op
        - ``gr_curve``   → 1 ``BezierCurve`` op (4 control points)
        - ``gr_text``    → 1 ``Text`` op (skipped when hide=True / "")
    * routing
        - ``segment``    → 1 ``ThickSegment`` op
        - ``track_arc``  → 1 ``ArcThreePoint`` op (with track width)
        - ``via``        → an optional effective-copper ``FlashPadCircle``
          followed by a physical-span drill ``Circle``. A via whose copper is
          removed from every layer therefore emits a drill-only record.
    * zones
        - ``zone_fill``  → N ``PlotPoly`` ops (one per filled_polygon
                            ring; FILLED_SHAPE)
    * footprints (PCB-embedded; reuses footprint per-element op emitters
      but carries placement (``at_x``/``at_y``/``at_angle``) in extras)
        - ``footprint``  → all fp graphics + pad ops

Coordinate convention: `.kicad_pcb` stores positions in mm with
**Y-down** already matching KiCad's PLOTTER convention. We multiply
by 1_000_000 at the boundary but do NOT negate Y. This keeps the IR's
``coordinate_space={"unit":"nm","y_axis":"down"}`` invariant.

Layer is captured in each record's ``extras["layer"]`` for downstream
filtering. Net info (``extras["net_id"]`` / ``extras["net_name"]``)
flows through tracks / vias / zones so consumers can colour-key by
net later.
"""

from __future__ import annotations

import copy
from collections.abc import Mapping, Sequence
from dataclasses import replace
import math
from types import MappingProxyType
from typing import TYPE_CHECKING, Any, cast

from .kicad_footprint_to_ir import (
    _footprint_text_variables,
    fp_arc_to_ops,
    fp_circle_to_op,
    fp_fill_to_kicad_fill,
    fp_line_to_ops,
    fp_poly_to_op,
    fp_rect_to_op,
    fp_text_box_to_ops,
    fp_text_to_op,
    pad_drill_to_ops,
    pad_to_ops,
    property_to_op,
)
from .kicad_lib_symbol_to_ir import (
    _effects_to_text_kwargs,
    mm_to_nm,
)
from .kicad_plotter_ir import (
    KiCadFillType,
    KiCadHorizAlign,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterRecord,
    KiCadVertAlign,
)
from .kicad_pcb_svg_enrichment import pcb_layer_role, project_net_name_to_classes
from .kicad_pcb_layer_policy import PcbLayerFlashResolver
from .kicad_stroke_decompose import (
    decompose_arc,
    decompose_segment,
    is_decomposable_style,
)
from .kicad_render_cache import (
    RenderCacheRequest,
    RenderCacheResolver,
    board_text_variables,
    render_cache_request_for_board_text,
    render_cache_request_for_footprint_property,
    render_cache_request_for_footprint_text,
    render_cache_request_for_footprint_text_box,
    render_cache_request_for_dimension_text,
    render_cache_request_for_table_cell,
    substitute_text_variables,
)

if TYPE_CHECKING:
    from .kicad_pcb import KiCadPcb
    from .kicad_pcb_footprint import Footprint
    from .kicad_pcb_gr_arc import GrArc
    from .kicad_pcb_gr_circle import GrCircle
    from .kicad_pcb_gr_curve import GrCurve
    from .kicad_pcb_gr_line import GrLine
    from .kicad_pcb_gr_poly import GrPoly
    from .kicad_pcb_gr_rect import GrRect
    from .kicad_pcb_gr_text import GrText
    from .kicad_pcb_graphics import GrTextBox
    from .kicad_pcb_other import NetTable
    from .kicad_pcb_routing import Arc as TrackArc
    from .kicad_pcb_routing import Segment, Via
    from .kicad_pcb_zone import FilledPolygon, Zone
    from .kicad_pcb_other import Dimension, Table, TableCell


# ---------------------------------------------------------------------------
# Per-item op emitters
# ---------------------------------------------------------------------------


def _pcb_stroke_width_nm(stroke: Any) -> int:
    """Return PCB graphic stroke width without schematic pen-width clamping."""

    width = float(getattr(stroke, "width", 0.0) or 0.0)
    if width <= 0.0:
        return 0
    return mm_to_nm(width)


def gr_line_to_op(line: "GrLine") -> KiCadPlotterOp:
    """Convert a ``gr_line`` into a ``ThickSegment`` op (solid only)."""
    return KiCadPlotterOp.thick_segment(
        start_x=mm_to_nm(line.start_x),
        start_y=mm_to_nm(line.start_y),
        end_x=mm_to_nm(line.end_x),
        end_y=mm_to_nm(line.end_y),
        width_nm=_pcb_stroke_width_nm(line.stroke),
    )


def gr_line_to_ops(line: "GrLine") -> list[KiCadPlotterOp]:
    """Convert a ``gr_line`` to one or more ``ThickSegment`` ops.

    For SOLID/DEFAULT styles emits a single op. For DASH/DOT/DASH_DOT/
    DASH_DOT_DOT decomposes into per-dash sub-segments (matching kicad-cli
    ``STROKE_PARAMS::Stroke`` output: one segment per dash).
    """
    style = getattr(line.stroke, "type", None) if line.stroke else None
    width_nm = _pcb_stroke_width_nm(line.stroke)
    if not is_decomposable_style(style):
        return [gr_line_to_op(line)]
    pieces = decompose_segment(
        mm_to_nm(line.start_x), mm_to_nm(line.start_y),
        mm_to_nm(line.end_x), mm_to_nm(line.end_y),
        width_nm,
        cast(Any, style),
    )
    if not pieces:
        return [gr_line_to_op(line)]
    return [
        KiCadPlotterOp.thick_segment(
            start_x=sx, start_y=sy, end_x=ex, end_y=ey, width_nm=width_nm,
        )
        for sx, sy, ex, ey in pieces
    ]


def gr_arc_to_op(arc: "GrArc") -> KiCadPlotterOp:
    """Convert a ``gr_arc`` (start/mid/end) into an ``ArcThreePoint`` op (solid only)."""
    return KiCadPlotterOp.arc_three_point(
        start_x=mm_to_nm(arc.start_x),
        start_y=mm_to_nm(arc.start_y),
        mid_x=mm_to_nm(arc.mid_x),
        mid_y=mm_to_nm(arc.mid_y),
        end_x=mm_to_nm(arc.end_x),
        end_y=mm_to_nm(arc.end_y),
        fill=KiCadFillType.NO_FILL,
        width_nm=_pcb_stroke_width_nm(arc.stroke),
    )


def gr_arc_to_ops(arc: "GrArc") -> list[KiCadPlotterOp]:
    """Convert a ``gr_arc`` to one or more ops.

    For SOLID/DEFAULT styles emits the single ``ArcThreePoint`` op. For
    non-solid styles decomposes into chord sub-segments per kicad-cli
    (0.5-degree steps within each dash, single chord per dot interval).
    """
    style = getattr(arc.stroke, "type", None) if arc.stroke else None
    width_nm = _pcb_stroke_width_nm(arc.stroke)
    if not is_decomposable_style(style):
        return [gr_arc_to_op(arc)]
    pieces = decompose_arc(
        mm_to_nm(arc.start_x), mm_to_nm(arc.start_y),
        mm_to_nm(arc.mid_x), mm_to_nm(arc.mid_y),
        mm_to_nm(arc.end_x), mm_to_nm(arc.end_y),
        width_nm,
        cast(Any, style),
    )
    if not pieces:
        return [gr_arc_to_op(arc)]
    return [
        KiCadPlotterOp.thick_segment(
            start_x=sx, start_y=sy, end_x=ex, end_y=ey, width_nm=width_nm,
        )
        for sx, sy, ex, ey in pieces
    ]


def gr_circle_to_op(circle: "GrCircle") -> KiCadPlotterOp:
    """
    Convert a ``gr_circle`` into a ``Circle`` op.

    KiCad stores a circle as ``(centre, end)`` — the radius is the
    Euclidean distance between the two points, doubled to a diameter
    for the IR.
    """
    dx = circle.end_x - circle.center_x
    dy = circle.end_y - circle.center_y
    radius_mm = (dx * dx + dy * dy) ** 0.5
    return KiCadPlotterOp.circle(
        cx=mm_to_nm(circle.center_x),
        cy=mm_to_nm(circle.center_y),
        diameter_nm=mm_to_nm(radius_mm * 2.0),
        fill=fp_fill_to_kicad_fill(circle.fill),
        width_nm=_pcb_stroke_width_nm(circle.stroke),
    )


def gr_rect_to_op(rect: "GrRect") -> KiCadPlotterOp:
    """Convert a ``gr_rect`` into a ``Rect`` op."""
    return KiCadPlotterOp.rect(
        x1=mm_to_nm(rect.start_x),
        y1=mm_to_nm(rect.start_y),
        x2=mm_to_nm(rect.end_x),
        y2=mm_to_nm(rect.end_y),
        fill=fp_fill_to_kicad_fill(rect.fill),
        width_nm=_pcb_stroke_width_nm(rect.stroke),
    )


def gr_poly_to_op(poly: "GrPoly") -> KiCadPlotterOp:
    """Convert a ``gr_poly`` into a ``PlotPoly`` op."""
    points = [(mm_to_nm(x), mm_to_nm(y)) for x, y in poly.points]
    return KiCadPlotterOp.plot_poly(
        points=points,
        fill=fp_fill_to_kicad_fill(poly.fill),
        width_nm=_pcb_stroke_width_nm(poly.stroke),
    )


def gr_curve_to_op(curve: "GrCurve") -> KiCadPlotterOp | None:
    """
    Convert a ``gr_curve`` (4 control points) into a ``BezierCurve`` op.

    Returns ``None`` when fewer than 4 control points are present
    (parser tolerates malformed curves; the IR requires the full set).
    """
    pts = curve.points
    if len(pts) < 4:
        return None
    return KiCadPlotterOp.bezier_curve(
        start_x=mm_to_nm(pts[0][0]), start_y=mm_to_nm(pts[0][1]),
        ctrl1_x=mm_to_nm(pts[1][0]), ctrl1_y=mm_to_nm(pts[1][1]),
        ctrl2_x=mm_to_nm(pts[2][0]), ctrl2_y=mm_to_nm(pts[2][1]),
        end_x=mm_to_nm(pts[3][0]), end_y=mm_to_nm(pts[3][1]),
        width_nm=_pcb_stroke_width_nm(curve.stroke),
    )


def _board_point_to_footprint_local(
    x: float,
    y: float,
    footprint: "Footprint | None",
) -> tuple[float, float]:
    if footprint is None:
        return x, y
    dx = x - float(getattr(footprint, "at_x", 0.0) or 0.0)
    dy = y - float(getattr(footprint, "at_y", 0.0) or 0.0)
    angle = math.radians(float(getattr(footprint, "at_angle", 0.0) or 0.0))
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    return (dx * cos_a - dy * sin_a, dx * sin_a + dy * cos_a)


def _render_cache_polygons_nm(
    request: RenderCacheRequest,
    *,
    footprint: "Footprint | None" = None,
) -> tuple[list[list[list[int]]], dict[str, Any] | None, str, bool]:
    result = RenderCacheResolver().ensure_cache(request)
    if not result.usable or result.cache is None:
        return [], None, result.source.value, result.exact

    exterior_polygons: list[list[list[int]]] = []
    typed_polygons: list[dict[str, list[list[list[int]]]]] = []
    for polygon in result.cache.polygons:
        contours: list[list[list[int]]] = []
        for contour in polygon.contours:
            if len(contour.points) < 3:
                continue
            points = []
            for x, y in contour.points:
                point_x, point_y = _board_point_to_footprint_local(x, y, footprint)
                points.append([mm_to_nm(point_x), mm_to_nm(point_y)])
            contours.append(points)
        if not contours:
            continue
        exterior_polygons.append(contours[0])
        typed_polygons.append({"contours": contours})

    if not typed_polygons:
        return [], None, result.source.value, result.exact

    payload = {
        "schema": "kicad.render_cache.v1",
        "unit": "nm",
        "coordinate_space": "footprint_local" if footprint is not None else "board",
        "text": result.cache.text,
        "angle": float(result.cache.angle),
        "source": result.source.value,
        "exact": bool(result.exact),
        "polygons": typed_polygons,
    }
    return exterior_polygons, payload, result.source.value, result.exact


def _op_with_render_cache_payload(
    op: KiCadPlotterOp,
    request: RenderCacheRequest | None,
    *,
    footprint: "Footprint | None" = None,
) -> KiCadPlotterOp:
    if request is None:
        return op
    polygons, cache_payload, source, exact = _render_cache_polygons_nm(
        request,
        footprint=footprint,
    )
    if not polygons:
        return op
    payload = copy.deepcopy(op.payload)
    payload["render_cache_polygons"] = polygons
    if cache_payload is not None:
        payload["render_cache"] = cache_payload
    payload["render_cache_source"] = source
    payload["render_cache_exact"] = bool(exact)
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _is_text_op(op: KiCadPlotterOp) -> bool:
    return op.kind == "Text" or getattr(op.kind, "value", None) == "Text"


def _text_op_from_render_cache_request(
    request: RenderCacheRequest,
    effects: Any,
    *,
    footprint: "Footprint | None" = None,
) -> KiCadPlotterOp | None:
    params = request.text_params
    if params is None:
        return None

    kwargs = _effects_to_text_kwargs(effects)
    op = KiCadPlotterOp.text(
        x=mm_to_nm(float(getattr(params, "position_x", 0.0))),
        y=mm_to_nm(float(getattr(params, "position_y", 0.0))),
        text=request.text,
        orient_deg=float(getattr(params, "angle", 0.0)),
        multiline="\n" in request.text,
        **kwargs,
    )
    return _op_with_render_cache_payload(op, request, footprint=footprint)


def _apply_knockout_to_text_op(
    op: KiCadPlotterOp, *, knockout_margin_nm: int
) -> KiCadPlotterOp:
    """Restructure a text op's render_cache for knockout text rendering.

    Knockout text on PCB silkscreen renders as a single compound polygon:
    a filled background rectangle (text bbox inflated by knockout margin)
    with each glyph contour subtracted via ``fill-rule="evenodd"``. The
    plotter-IR's existing typed render_cache pipeline already emits one
    ``<path>`` per polygon entry and applies evenodd when a polygon has
    multiple contours, so the only transformation needed here is to
    coalesce all glyph contours under a single polygon entry whose first
    contour is the background rectangle.

    Returns ``op`` unchanged if there's no usable render cache.
    """
    payload = op.payload
    cache = payload.get("render_cache")
    if not isinstance(cache, dict):
        return op
    polygons = cache.get("polygons")
    if not isinstance(polygons, list) or not polygons:
        return op

    glyph_contours: list[list[list[int]]] = []
    min_x = min_y = None
    max_x = max_y = None
    for polygon in polygons:
        if not isinstance(polygon, dict):
            continue
        contours = polygon.get("contours")
        if not isinstance(contours, list):
            continue
        for contour in contours:
            if not isinstance(contour, list) or len(contour) < 3:
                continue
            glyph_contours.append(contour)
            for point in contour:
                px, py = int(point[0]), int(point[1])
                if min_x is None or px < min_x:
                    min_x = px
                if max_x is None or px > max_x:
                    max_x = px
                if min_y is None or py < min_y:
                    min_y = py
                if max_y is None or py > max_y:
                    max_y = py

    if not glyph_contours or min_x is None or max_x is None or min_y is None or max_y is None:
        return op

    margin = int(knockout_margin_nm)
    bx0, by0 = min_x - margin, min_y - margin
    bx1, by1 = max_x + margin, max_y + margin
    bg_rect_contour: list[list[int]] = [
        [bx0, by0],
        [bx1, by0],
        [bx1, by1],
        [bx0, by1],
    ]

    new_polygon = {"contours": [bg_rect_contour, *glyph_contours]}
    new_cache = copy.deepcopy(cache)
    new_cache["polygons"] = [new_polygon]
    new_cache["knockout"] = True

    new_payload = copy.deepcopy(payload)
    new_payload["render_cache"] = new_cache
    # Refresh the simple ``render_cache_polygons`` list so the legacy
    # (non-typed) renderer fallback would also see the knockout outer
    # rectangle as a single polygon (matching the typed pipeline's
    # single-path emit).
    new_payload["render_cache_polygons"] = [bg_rect_contour]
    new_payload["knockout"] = True
    return KiCadPlotterOp(kind=op.kind, payload=new_payload)


def _stroke_alignment_token(value: object, *, axis: str) -> str:
    text = str(value)
    if axis == "h":
        if text.endswith("_RIGHT"):
            return "right"
        if text.endswith("_CENTER"):
            return "center"
        return "left"
    if text.endswith("_TOP"):
        return "top"
    if text.endswith("_CENTER"):
        return "center"
    return "bottom"


def _polygon_contours_from_geometry(geometry: object) -> list[list[list[int]]]:
    if getattr(geometry, "is_empty", False):
        return []
    geoms = list(getattr(geometry, "geoms", []) or [geometry])
    contours: list[list[list[int]]] = []
    for geom in geoms:
        exterior = getattr(geom, "exterior", None)
        if exterior is None:
            continue
        ext_points = [
            [mm_to_nm(float(x)), mm_to_nm(float(y))]
            for x, y in list(exterior.coords)[:-1]
        ]
        if len(ext_points) >= 3:
            contours.append(ext_points)
        for interior in getattr(geom, "interiors", []) or []:
            int_points = [
                [mm_to_nm(float(x)), mm_to_nm(float(y))]
                for x, y in list(interior.coords)[:-1]
            ]
            if len(int_points) >= 3:
                contours.append(int_points)
    return contours


def _apply_synthetic_knockout_to_text_box_op(
    op: KiCadPlotterOp,
    text_box: "GrTextBox",
) -> KiCadPlotterOp:
    """Build a fallback filled knockout contour for text boxes without cache."""
    if op.payload.get("render_cache"):
        return op
    try:
        from shapely.geometry import Polygon, box as shapely_box
        from shapely.ops import unary_union

        from .kicad_text import KiCadTextRenderer
    except Exception:
        return op

    text = str(op.payload.get("text", "") or "")
    if not text:
        return op

    effects = getattr(text_box, "effects", None)
    font = effects.font if effects else None
    if font is None:
        return op

    from .kicad_stroke_font import get_renderer

    x_mm = float(op.payload.get("x", 0) or 0) / 1_000_000.0
    y_mm = float(op.payload.get("y", 0) or 0) / 1_000_000.0
    size_x_mm = float(op.payload.get("size_x_nm", mm_to_nm(1.27))) / 1_000_000.0
    size_y_mm = float(op.payload.get("size_y_nm", mm_to_nm(1.27))) / 1_000_000.0
    orient_deg = float(op.payload.get("orient_deg", 0.0) or 0.0)
    h_align = _stroke_alignment_token(op.payload.get("h_align", ""), axis="h")
    v_align = _stroke_alignment_token(op.payload.get("v_align", ""), axis="v")
    text_lines = text.split("\n")

    line_positions = [(x_mm, y_mm)]
    if len(text_lines) > 1:
        line_step = size_y_mm * 1.68
        start_y = y_mm
        if v_align == "center":
            start_y -= (len(text_lines) - 1) * line_step / 2.0
        elif v_align == "bottom":
            start_y -= (len(text_lines) - 1) * line_step
        line_positions = [(x_mm, start_y + idx * line_step) for idx in range(len(text_lines))]

    renderer = get_renderer()
    stroke_polygons = []
    for line, (line_x, line_y) in zip(text_lines, line_positions):
        if not line:
            continue
        for polyline in renderer.render_text_polylines(
            text=line,
            pos_x=line_x,
            pos_y=line_y,
            size_x=size_x_mm,
            size_y=size_y_mm,
            angle=orient_deg,
            h_align=h_align,
            v_align=v_align,
            mirror=bool(op.payload.get("mirror", False)),
            italic=bool(op.payload.get("italic", False)),
        ):
            for start, end in zip(polyline, polyline[1:]):
                contour = KiCadTextRenderer._stroke_segment_to_polygon(
                    start,
                    end,
                    float(font.effective_thickness),
                )
                if len(contour) < 3:
                    continue
                polygon = Polygon(contour)
                if not polygon.is_valid:
                    polygon = polygon.buffer(0)
                if not polygon.is_empty:
                    stroke_polygons.append(polygon)

    if not stroke_polygons:
        return op

    text_geometry = unary_union(stroke_polygons).simplify(
        0.0005,
        preserve_topology=True,
    )
    x1 = min(float(text_box.start_x), float(text_box.end_x))
    y1 = min(float(text_box.start_y), float(text_box.end_y))
    x2 = max(float(text_box.start_x), float(text_box.end_x))
    y2 = max(float(text_box.start_y), float(text_box.end_y))
    stroke = getattr(text_box, "stroke", None)
    border_width = float(getattr(stroke, "width", font.effective_thickness))
    rect_geometry = shapely_box(x1, y1, x2, y2).buffer(
        border_width / 2.0,
        quad_segs=4,
        join_style="round",
    )

    contours = _polygon_contours_from_geometry(rect_geometry)
    contours.extend(_polygon_contours_from_geometry(text_geometry))
    if not contours:
        return op

    payload = copy.deepcopy(op.payload)
    payload["render_cache"] = {
        "polygons": [{"contours": contours}],
        "knockout": True,
    }
    payload["knockout"] = True
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def gr_text_to_op(text: "GrText", board: "KiCadPcb | None" = None) -> KiCadPlotterOp | None:
    """
    Convert a ``gr_text`` into a ``Text`` op.

    Returns ``None`` when ``text.hide`` is True or ``text.text`` is
    empty (mirroring KiCad's ``PCB_TEXT::Plot`` skip rule).
    """
    if getattr(text, "hide", False) or not getattr(text, "text", ""):
        return None
    kwargs = _effects_to_text_kwargs(text.effects)
    font = text.effects.font if text.effects else None
    cache_request = None
    resolved_text = (
        substitute_text_variables(text.text, board_text_variables(board))
        if board is not None
        else text.text
    )
    if board is not None and (text.render_cache or (font and font.face)):
        cache_request = render_cache_request_for_board_text(
            text,
            board,
            object_type="gr_text",
            include_text_params=bool(font and font.face),
        )
        resolved_text = cache_request.text
    kwargs.setdefault("h_align", KiCadHorizAlign.CENTER)
    kwargs.setdefault("v_align", KiCadVertAlign.CENTER)
    op = KiCadPlotterOp.text(
        x=mm_to_nm(text.at_x),
        y=mm_to_nm(text.at_y),
        text=resolved_text,
        orient_deg=float(getattr(text, "at_angle", 0.0)),
        **kwargs,
    )
    justify = getattr(text.effects, "justify", None) if text.effects else None
    if justify and "mirror" in justify:
        op = KiCadPlotterOp(
            kind=op.kind,
            payload={**op.payload, "mirror": True},
        )
    if not (font and font.face):
        op = KiCadPlotterOp(
            kind=op.kind,
            payload={
                **op.payload,
                "text_as_polygons": True,
                # kicad-cli plots stroke-font board text one segment per
                # path (PCB plotter records each MoveTo/LineTo pair), so
                # request per-segment polyline granularity for parity.
                "polyline_per_segment": True,
            },
        )
    op = _op_with_render_cache_payload(op, cache_request)
    if getattr(text, "knockout", False):
        margin_mm = text.get_knockout_margin() if hasattr(text, "get_knockout_margin") else 0.0
        op = _apply_knockout_to_text_op(op, knockout_margin_nm=mm_to_nm(margin_mm))
    return op


def track_segment_to_op(seg: "Segment") -> KiCadPlotterOp:
    """Convert a track ``segment`` into a ``ThickSegment`` op."""
    return KiCadPlotterOp.thick_segment(
        start_x=mm_to_nm(seg.start_x),
        start_y=mm_to_nm(seg.start_y),
        end_x=mm_to_nm(seg.end_x),
        end_y=mm_to_nm(seg.end_y),
        width_nm=mm_to_nm(seg.width),
    )


def track_arc_to_op(arc: "TrackArc") -> KiCadPlotterOp:
    """Convert a track ``arc`` into an ``ArcThreePoint`` op with width."""
    # KiCad's track-arc plotter emits the arc from the file's end point back
    # to its start point. Keep that plot direction in the IR so SVG sweep
    # flags and path endpoints match kicad-cli.
    return KiCadPlotterOp.arc_three_point(
        start_x=mm_to_nm(arc.end_x),
        start_y=mm_to_nm(arc.end_y),
        mid_x=mm_to_nm(arc.mid_x),
        mid_y=mm_to_nm(arc.mid_y),
        end_x=mm_to_nm(arc.start_x),
        end_y=mm_to_nm(arc.start_y),
        fill=KiCadFillType.NO_FILL,
        width_nm=mm_to_nm(arc.width),
    )


def via_to_op(
    via: "Via", *, flashed_layers: Sequence[str] | None = None
) -> KiCadPlotterOp:
    """Convert a ``via`` into an effective-copper ``FlashPadCircle`` op.

    Carries explicitly supplied effective copper layers in its payload so the per-op
    layer filter restricts the aperture to copper layers only —
    kicad-cli does not emit a via aperture on mask / silk / etc.
    """
    op = KiCadPlotterOp.flash_pad_circle(
        x=mm_to_nm(via.at_x),
        y=mm_to_nm(via.at_y),
        diameter_nm=mm_to_nm(via.size),
    )
    layers = list(via.layers) if flashed_layers is None else list(flashed_layers)
    return KiCadPlotterOp(
        kind=op.kind,
        payload={**op.payload, "role": "via_aperture", "layers": layers},
    )


def via_drill_to_op(via: "Via") -> KiCadPlotterOp:
    """Convert a ``via`` drill into a synthetic filled ``Circle`` op."""
    drill_size = via.drill if via.drill and via.drill > 0 else via.size * 0.5
    op = KiCadPlotterOp.circle(
        cx=mm_to_nm(via.at_x),
        cy=mm_to_nm(via.at_y),
        diameter_nm=mm_to_nm(drill_size),
        fill=KiCadFillType.FILLED_SHAPE,
        width_nm=0,
    )
    return KiCadPlotterOp(
        kind=op.kind,
        payload={**op.payload, "role": "via_drill", "layers": list(via.layers)},
    )


def via_mask_opening_to_op(via: "Via", *, mask_layer: str, clearance_mm: float) -> KiCadPlotterOp:
    """Synthesize a via mask opening (outer circle) on the given mask layer.

    Mirrors kicad-cli, which emits a filled circle of radius
    ``via.size/2 + board_mask_clearance`` on F.Mask / B.Mask whenever the
    via is tenting-exposed on the corresponding side.
    """
    diameter_mm = float(via.size) + 2.0 * float(clearance_mm)
    op = KiCadPlotterOp.flash_pad_circle(
        x=mm_to_nm(via.at_x),
        y=mm_to_nm(via.at_y),
        diameter_nm=mm_to_nm(diameter_mm),
    )
    return KiCadPlotterOp(
        kind=op.kind,
        payload={
            **op.payload,
            "role": "via_mask_opening",
            "layers": [mask_layer],
        },
    )


def via_mask_drill_to_op(via: "Via", *, mask_layer: str) -> KiCadPlotterOp:
    """Synthesize a via mask drill knockout on the given mask layer.

    Same geometry as :func:`via_drill_to_op` but pinned to a single mask
    layer (so it knocks the drill out of the mask opening).
    """
    drill_size = via.drill if via.drill and via.drill > 0 else via.size * 0.5
    op = KiCadPlotterOp.circle(
        cx=mm_to_nm(via.at_x),
        cy=mm_to_nm(via.at_y),
        diameter_nm=mm_to_nm(drill_size),
        fill=KiCadFillType.FILLED_SHAPE,
        width_nm=0,
    )
    return KiCadPlotterOp(
        kind=op.kind,
        payload={
            **op.payload,
            "role": "via_mask_drill",
            "layers": [mask_layer],
        },
    )


def zone_filled_polygon_to_op(fpoly: "FilledPolygon") -> KiCadPlotterOp:
    """Convert one ``filled_polygon`` ring into a filled ``PlotPoly`` op."""
    points = [(mm_to_nm(x), mm_to_nm(y)) for x, y in fpoly.points]
    return KiCadPlotterOp.plot_poly(
        points=points,
        fill=KiCadFillType.FILLED_SHAPE,
        width_nm=0,
    )


# ---------------------------------------------------------------------------
# Per-item record builders
# ---------------------------------------------------------------------------


def _net_extras(net: Any) -> dict[str, Any]:
    """
    Pull (net_id, net_name) out of a :class:`NetRef`.

    Reads ``NetRef.ordinal`` (the integer net code, e.g. ``2``) and
    ``NetRef.name`` (e.g. ``"+3V3"``). Returns an empty dict when both
    are absent so callers can ``extras.update(_net_extras(...))``
    without bloating the record.
    """
    if net is None:
        return {}
    extras: dict[str, Any] = {}
    nid = getattr(net, "ordinal", None)
    nname = getattr(net, "name", None)
    if nid is not None:
        extras["net_id"] = int(nid)
    if nname:
        extras["net_name"] = str(nname)
    return extras


def _enum_text(value: object) -> str:
    raw = getattr(value, "value", value)
    return str(raw or "")


def _net_classes_for_name(
    net_name_to_classes: Mapping[str, Sequence[str]],
    net_name: str,
) -> Sequence[str]:
    if not net_name:
        return ()
    return net_name_to_classes.get(net_name, ())


def _net_class_snapshot(board: "KiCadPcb") -> Mapping[str, Sequence[str]]:
    """Build one detached immutable project-netclass lookup."""
    return MappingProxyType(
        {
            name: tuple(classes)
            for name, classes in project_net_name_to_classes(board).items()
        }
    )


def _with_net_class_extras(
    extras: dict[str, Any],
    net_name_to_classes: Mapping[str, Sequence[str]],
) -> dict[str, Any]:
    out = dict(extras)
    net_name = str(out.get("net_name", "") or "")
    classes = _net_classes_for_name(net_name_to_classes, net_name)
    if classes:
        out["net_class"] = classes[0]
        out["net_classes"] = list(classes)
    return out


def _records_with_net_class_extras(
    records: list[KiCadPlotterRecord],
    net_name_to_classes: Mapping[str, Sequence[str]],
) -> list[KiCadPlotterRecord]:
    return [
        replace(
            record,
            extras=_with_net_class_extras(record.extras or {}, net_name_to_classes),
        )
        for record in records
    ]


def _pad_resolved_net(
    pad: Any,
    board: "KiCadPcb | None",
    net_table: "NetTable | None",
) -> Any:
    net = getattr(pad, "net", None)
    if net_table is not None:
        return net_table.resolve(net)
    resolver = getattr(board, "resolve_net_ref", None)
    if callable(resolver):
        return resolver(net)
    return net


def _pad_block_label(
    pad: Any,
    footprint: "Footprint",
    *,
    index: int,
    suffix: str = "",
) -> str:
    base = str(getattr(pad, "uuid", "") or "")
    if not base:
        owner = str(getattr(footprint, "uuid", "") or footprint.library_link or "footprint")
        number = str(getattr(pad, "number", "") or index)
        base = f"{owner}:pad:{index}:{number}"
    return f"{base}:{suffix}" if suffix else base


def _pad_hole_kind(pad: Any) -> str:
    return (
        "slot"
        if bool(getattr(pad, "drill_oval", False))
        and getattr(pad, "drill_width", None) is not None
        and getattr(pad, "drill_height", None) is not None
        else "round"
    )


def _pad_hole_dimension_attrs(pad: Any) -> dict[str, Any]:
    if _pad_hole_kind(pad) == "slot":
        attrs: dict[str, Any] = {}
        width = getattr(pad, "drill_width", None)
        height = getattr(pad, "drill_height", None)
        if width is not None:
            attrs["hole_width_mm"] = float(width)
        if height is not None:
            attrs["hole_height_mm"] = float(height)
        return attrs

    drill = getattr(pad, "drill", None)
    if drill is None and _enum_text(getattr(pad, "pad_type", "")) == "np_thru_hole":
        fallback = min(
            float(getattr(pad, "size_x", 0.0)),
            float(getattr(pad, "size_y", 0.0)),
        )
        drill = fallback if fallback > 0 else None
    return {"hole_diameter_mm": float(drill)} if drill else {}


def _pad_block_extra_attrs(
    pad: Any,
    footprint: "Footprint",
    board: "KiCadPcb | None",
    net_table: "NetTable | None",
    net_name_to_classes: Mapping[str, Sequence[str]],
    *,
    primitive: str,
    pad_index: int,
    operation_layers: Sequence[str] | None = None,
) -> dict[str, Any]:
    get_property = getattr(footprint, "get_property_value", None)
    component = get_property("Reference", "") if callable(get_property) else ""
    pad_number = str(getattr(pad, "number", "") or "")
    attrs: dict[str, Any] = {
        "primitive": primitive,
        "component": component,
        "component_uid": getattr(footprint, "uuid", "") or "",
        "component_uuid": getattr(footprint, "uuid", "") or "",
        "footprint": footprint.library_link,
        "pad_number": pad_number,
        "pad_designator": f"{component}-{pad_number}" if component and pad_number else pad_number,
        "pad_type": _enum_text(getattr(pad, "pad_type", "")),
        "pad_shape": _enum_text(getattr(pad, "shape", "")),
        "layer_names": ",".join(
            str(layer)
            for layer in (
                operation_layers
                if operation_layers is not None
                else getattr(pad, "layers", []) or []
            )
        ),
    }

    net = _pad_resolved_net(pad, board, net_table)
    net_id = getattr(net, "ordinal", None)
    net_name = str(getattr(net, "name", "") or "")
    if net_id is not None:
        attrs["net_index"] = net_id
        attrs["net_id"] = net_id
    if net_name:
        attrs["net"] = net_name
    classes = _net_classes_for_name(net_name_to_classes, net_name)
    if classes:
        attrs["net_class"] = classes[0]
        attrs["net_classes"] = ",".join(classes)

    if primitive == "pad-hole":
        pad_type = _enum_text(getattr(pad, "pad_type", ""))
        attrs.update(
            {
                "hole_owner": _pad_block_label(pad, footprint, index=pad_index),
                "hole_kind": _pad_hole_kind(pad),
                "hole_plating": "non_plated" if pad_type == "np_thru_hole" else "plated",
                "hole_render": "drill",
            }
        )
        attrs.update(_pad_hole_dimension_attrs(pad))
    return attrs


def _pad_block_ops(
    pad: Any,
    footprint: "Footprint",
    board: "KiCadPcb | None",
    net_table: "NetTable | None",
    net_name_to_classes: Mapping[str, Sequence[str]],
    *,
    pad_index: int,
    pad_orient_offset: float,
    flashed_layers: Sequence[str] | None = None,
    drill_layers: Sequence[str] | None = None,
) -> list[KiCadPlotterOp]:
    layers = [str(layer) for layer in getattr(pad, "layers", []) or []]
    flash_layers = layers if flashed_layers is None else [str(layer) for layer in flashed_layers]
    physical_drill_layers = (
        layers if drill_layers is None else [str(layer) for layer in drill_layers]
    )
    pad_type = _enum_text(getattr(pad, "pad_type", ""))
    drill_operation_layers = (
        layers if pad_type == "np_thru_hole" else physical_drill_layers
    )

    def prepare(op: KiCadPlotterOp, operation_layers: list[str]) -> KiCadPlotterOp:
        return _op_with_pad_mask_hints(
            _op_with_pcb_layers(op, operation_layers),
            pad,
            footprint,
            board,
        )

    legacy_unlayered_flash = not layers and not bool(
        getattr(pad, "remove_unused_layers", False)
    )
    pad_ops = (
        [
            prepare(op, flash_layers)
            for op in pad_to_ops(pad, orient_deg_offset=pad_orient_offset)
        ]
        if flash_layers or legacy_unlayered_flash
        else []
    )
    drill_ops = [
        prepare(op, drill_operation_layers)
        for op in pad_drill_to_ops(pad, orient_deg_offset=pad_orient_offset)
    ]

    ops: list[KiCadPlotterOp] = []
    if pad_ops:
        label = _pad_block_label(pad, footprint, index=pad_index)
        ops.append(
            KiCadPlotterOp.start_block(
                label=label,
                data_uuid=label,
                data_ref="pad",
                object_id=str(getattr(pad, "number", "") or "pad"),
                extra_attrs=_pad_block_extra_attrs(
                    pad,
                    footprint,
                    board,
                    net_table,
                    net_name_to_classes,
                    primitive="pad",
                    pad_index=pad_index,
                    operation_layers=flash_layers,
                ),
                layers=flash_layers,
            )
        )
        ops.extend(pad_ops)
        ops.append(KiCadPlotterOp.end_block())

    if drill_ops:
        label = _pad_block_label(pad, footprint, index=pad_index, suffix="hole")
        hole_layers = None if pad_type == "np_thru_hole" else physical_drill_layers
        ops.append(
            KiCadPlotterOp.start_block(
                label=label,
                data_uuid=label,
                data_ref="pad_hole",
                object_id=str(getattr(pad, "number", "") or "pad"),
                extra_attrs=_pad_block_extra_attrs(
                    pad,
                    footprint,
                    board,
                    net_table,
                    net_name_to_classes,
                    primitive="pad-hole",
                    pad_index=pad_index,
                    operation_layers=drill_operation_layers,
                ),
                layers=hole_layers,
            )
        )
        ops.extend(drill_ops)
        ops.append(KiCadPlotterOp.end_block())

    return ops


def gr_line_to_record(line: "GrLine") -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=line.uuid or "",
        kind="gr_line",
        object_id="gr_line",
        operations=gr_line_to_ops(line),
        extras={"layer": line.layer},
    )


def gr_arc_to_record(arc: "GrArc") -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=arc.uuid or "",
        kind="gr_arc",
        object_id="gr_arc",
        operations=gr_arc_to_ops(arc),
        extras={"layer": arc.layer},
    )


def gr_circle_to_record(circle: "GrCircle") -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=circle.uuid or "",
        kind="gr_circle",
        object_id="gr_circle",
        operations=[gr_circle_to_op(circle)],
        extras={"layer": circle.layer},
    )


def gr_rect_to_record(rect: "GrRect") -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=rect.uuid or "",
        kind="gr_rect",
        object_id="gr_rect",
        operations=[gr_rect_to_op(rect)],
        extras={"layer": rect.layer},
    )


def gr_poly_to_record(poly: "GrPoly") -> KiCadPlotterRecord:
    return KiCadPlotterRecord(
        uuid=poly.uuid or "",
        kind="gr_poly",
        object_id="gr_poly",
        operations=[gr_poly_to_op(poly)],
        extras={"layer": poly.layer},
    )


def gr_curve_to_record(curve: "GrCurve") -> KiCadPlotterRecord:
    op = gr_curve_to_op(curve)
    return KiCadPlotterRecord(
        uuid=curve.uuid or "",
        kind="gr_curve",
        object_id="gr_curve",
        operations=[op] if op is not None else [],
        extras={"layer": curve.layer},
    )


def gr_text_to_record(text: "GrText", board: "KiCadPcb | None" = None) -> KiCadPlotterRecord:
    op = gr_text_to_op(text, board=board)
    return KiCadPlotterRecord(
        uuid=text.uuid or "",
        kind="gr_text",
        object_id="gr_text",
        operations=[op] if op is not None else [],
        extras={
            "layer": text.layer,
            "text": op.payload.get("text", text.text) if op is not None else text.text,
            "hide": bool(getattr(text, "hide", False)),
        },
    )


def gr_text_box_to_ops(
    text_box: "GrTextBox",
    board: "KiCadPcb | None" = None,
) -> list[KiCadPlotterOp]:
    """Convert a board-level ``gr_text_box`` into optional border and text ops."""
    variables = board_text_variables(board) if board is not None else None
    ops = fp_text_box_to_ops(
        text_box,
        variables=variables,
        default_h_align=KiCadHorizAlign.CENTER,
        default_v_align=KiCadVertAlign.CENTER,
    )
    font = text_box.effects.font if text_box.effects else None
    if not (font and font.face):
        ops = [
            KiCadPlotterOp(
                kind=op.kind,
                payload={**op.payload, "text_as_polygons": True},
            )
            if _is_text_op(op)
            else op
            for op in ops
        ]
    if getattr(text_box, "knockout", False):
        ops = [
            _apply_synthetic_knockout_to_text_box_op(op, text_box)
            if _is_text_op(op)
            else op
            for op in ops
        ]
    if board is None or not (text_box.render_cache or (font and font.face)):
        return ops

    request = render_cache_request_for_board_text(
        text_box,
        board,
        object_type="gr_text_box",
        include_text_params=bool(font and font.face),
    )
    for index, op in enumerate(ops):
        if _is_text_op(op):
            payload = copy.deepcopy(op.payload)
            payload["text"] = request.text
            ops[index] = _op_with_render_cache_payload(
                KiCadPlotterOp(kind=op.kind, payload=payload),
                request,
            )
            break
    return ops


def gr_text_box_to_record(
    text_box: "GrTextBox",
    board: "KiCadPcb | None" = None,
) -> KiCadPlotterRecord:
    operations = gr_text_box_to_ops(text_box, board=board)
    text_payload = next(
        (op.payload.get("text") for op in operations if _is_text_op(op)),
        text_box.text,
    )
    return KiCadPlotterRecord(
        uuid=text_box.uuid or "",
        kind="gr_text_box",
        object_id="gr_text_box",
        operations=operations,
        extras={
            "layer": text_box.layer,
            "text": text_payload,
            "border": bool(text_box.border),
        },
    )


def track_segment_to_record(seg: "Segment") -> KiCadPlotterRecord:
    extras: dict[str, Any] = {"layer": seg.layer, "locked": bool(seg.locked)}
    extras.update(_net_extras(seg.net))
    return KiCadPlotterRecord(
        uuid=seg.uuid or "",
        kind="segment",
        object_id="segment",
        operations=[track_segment_to_op(seg)],
        extras=extras,
    )


def track_arc_to_record(arc: "TrackArc") -> KiCadPlotterRecord:
    extras: dict[str, Any] = {"layer": arc.layer}
    extras.update(_net_extras(arc.net))
    return KiCadPlotterRecord(
        uuid=arc.uuid or "",
        kind="track_arc",
        object_id="track_arc",
        operations=[track_arc_to_op(arc)],
        extras=extras,
    )


def _via_exposed_mask_layers(via: "Via") -> list[str]:
    """Return the mask layers a via is tenting-exposed on.

    A via emits mask openings only when ``via.tenting`` explicitly disables
    tenting on that side. ``None`` means "use board default" — which
    kicad-cli treats as tented (no mask opening).
    """
    tenting = getattr(via, "tenting", None)
    if tenting is None:
        return []
    layers = list(getattr(via, "layers", None) or [])
    out: list[str] = []
    if getattr(tenting, "front", None) is False and any(
        layer == "F.Cu" or layer == "*.Cu" for layer in layers
    ):
        out.append("F.Mask")
    if getattr(tenting, "back", None) is False and any(
        layer == "B.Cu" or layer == "*.Cu" for layer in layers
    ):
        out.append("B.Mask")
    return out


def _bool_metadata(value: object) -> str | None:
    if value is True:
        return "true"
    if value is False:
        return "false"
    return None


def _front_back_metadata(prefix: str, value: object) -> dict[str, str]:
    if value is None:
        return {}
    attrs: dict[str, str] = {}
    front = _bool_metadata(getattr(value, "front", None))
    back = _bool_metadata(getattr(value, "back", None))
    if front is not None:
        attrs[f"{prefix}_front"] = front
    if back is not None:
        attrs[f"{prefix}_back"] = back
    return attrs


def _via_fabrication_extras(via: "Via") -> dict[str, str]:
    extras: dict[str, str] = {}
    extras.update(
        _front_back_metadata("ipc4761_tenting", getattr(via, "tenting", None))
    )
    extras.update(
        _front_back_metadata("ipc4761_covering", getattr(via, "covering", None))
    )
    extras.update(
        _front_back_metadata("ipc4761_plugging", getattr(via, "plugging", None))
    )

    capping = _bool_metadata(getattr(via, "capping", None))
    filling = _bool_metadata(getattr(via, "filling", None))
    if capping is not None:
        extras["ipc4761_capping"] = capping
    if filling is not None:
        extras["ipc4761_filling"] = filling
    if extras:
        extras["ipc4761_metadata"] = "true"
    return extras


def via_to_record(
    via: "Via",
    *,
    mask_clearance_mm: float = 0.0,
    flashed_layers: Sequence[str] | None = None,
) -> KiCadPlotterRecord:
    resolved_flash_layers = (
        list(via.layers) if flashed_layers is None else list(flashed_layers)
    )
    operations: list[KiCadPlotterOp] = []
    legacy_unlayered_aperture = not via.layers and not any(
        bool(getattr(via, field, False))
        for field in ("remove_unused_layers", "start_end_only")
    )
    if resolved_flash_layers or legacy_unlayered_aperture:
        operations.append(via_to_op(via, flashed_layers=resolved_flash_layers))
    operations.append(via_drill_to_op(via))
    for mask_layer in _via_exposed_mask_layers(via):
        operations.append(
            via_mask_opening_to_op(
                via, mask_layer=mask_layer, clearance_mm=mask_clearance_mm
            )
        )
        operations.append(via_mask_drill_to_op(via, mask_layer=mask_layer))

    extras: dict[str, Any] = {
        "layers": list(via.layers),
        "drill": float(via.drill),
        "size": float(via.size),
        "via_type": via.via_type or "through",
        "hole_kind": "round",
        "hole_plating": "plated",
        "hole_render": "drill",
    }
    extras.update(_via_fabrication_extras(via))
    extras.update(_net_extras(via.net))
    return KiCadPlotterRecord(
        uuid=via.uuid or "",
        kind="via",
        object_id="via",
        operations=operations,
        extras=extras,
    )


def _op_with_pcb_layer(op: KiCadPlotterOp, layer: str) -> KiCadPlotterOp:
    payload = copy.deepcopy(op.payload)
    payload["layer"] = str(layer)
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _op_with_pcb_layers(op: KiCadPlotterOp, layers: list[str]) -> KiCadPlotterOp:
    payload = copy.deepcopy(op.payload)
    payload["layers"] = [str(layer) for layer in layers]
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _resolved_pad_mask_margin_nm(
    pad: Any,
    footprint: "Footprint",
    board: "KiCadPcb | None",
) -> int:
    margin = getattr(pad, "solder_mask_margin", None)
    if margin is None:
        margin = getattr(footprint, "solder_mask_margin", None)
    if margin is None and board is not None:
        margin = getattr(board, "pad_to_mask_clearance", 0.0)
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
    pad: Any,
    footprint: "Footprint",
    board: "KiCadPcb | None",
) -> KiCadPlotterOp:
    kind = str(getattr(op.kind, "value", op.kind))
    role = str(op.payload.get("role", ""))
    if not kind.startswith("FlashPad") and role != "npth_hole":
        return op
    payload = copy.deepcopy(op.payload)
    payload["mask_margin_nm"] = _resolved_pad_mask_margin_nm(pad, footprint, board)
    if role == "npth_hole":
        payload["pad_size_x_nm"] = mm_to_nm(float(getattr(pad, "size_x", 0.0)))
        payload["pad_size_y_nm"] = mm_to_nm(float(getattr(pad, "size_y", 0.0)))
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _segment_op_mm(
    start: tuple[float, float],
    end: tuple[float, float],
    *,
    width_mm: float,
) -> KiCadPlotterOp:
    return KiCadPlotterOp.thick_segment(
        start_x=mm_to_nm(start[0]),
        start_y=mm_to_nm(start[1]),
        end_x=mm_to_nm(end[0]),
        end_y=mm_to_nm(end[1]),
        width_nm=mm_to_nm(width_mm),
    )


def _layered_segment_op(
    start: tuple[float, float],
    end: tuple[float, float],
    *,
    width_mm: float,
    layer: str,
) -> KiCadPlotterOp:
    return _op_with_pcb_layer(
        _segment_op_mm(start, end, width_mm=width_mm),
        layer,
    )


def _vec_add(
    a: tuple[float, float],
    b: tuple[float, float],
) -> tuple[float, float]:
    return (a[0] + b[0], a[1] + b[1])


def _vec_sub(
    a: tuple[float, float],
    b: tuple[float, float],
) -> tuple[float, float]:
    return (a[0] - b[0], a[1] - b[1])


def _vec_len(vector: tuple[float, float]) -> float:
    return math.hypot(vector[0], vector[1])


def _vec_resize(vector: tuple[float, float], length: float) -> tuple[float, float]:
    norm = _vec_len(vector)
    if norm == 0.0:
        return (0.0, 0.0)
    return (vector[0] * length / norm, vector[1] * length / norm)


def _vec_angle(vector: tuple[float, float]) -> float:
    if vector == (0.0, 0.0):
        return 0.0
    return math.degrees(math.atan2(vector[1], vector[0]))


def _rotate_vector_kicad(
    vector: tuple[float, float],
    angle_deg: float,
) -> tuple[float, float]:
    rad = math.radians(angle_deg)
    sin_a = math.sin(rad)
    cos_a = math.cos(rad)
    x, y = vector
    return (y * sin_a + x * cos_a, y * cos_a - x * sin_a)


_DIM_ARROW_ANGLE_DEG = 27.5
_DIM_INWARD_ARROW_TAIL_RATIO = 2.0
_DIM_TEXT_MARGIN_RATIO = 0.625


def _dimension_text_alignment(text_object: "GrText") -> tuple[str, str]:
    effects = getattr(text_object, "effects", None)
    justify = getattr(effects, "justify", None) or []
    h_align = "center"
    v_align = "center"
    for tok in justify:
        if tok in ("left", "right", "center"):
            h_align = tok
        elif tok in ("top", "bottom"):
            v_align = tok
    return h_align, v_align


def _stroke_text_width(text: str) -> float:
    from .kicad_stroke_font import get_glyph, get_space_width

    width = 0.0
    for char in text:
        if char == " ":
            width += get_space_width()
            continue
        glyph = get_glyph(char)
        if glyph is not None:
            width += glyph.width
    return width


def _dimension_text_box(text_object: "GrText") -> tuple[float, float, float, float] | None:
    """Return KiCad's logical dimension-text box, including text margin."""

    text_str = getattr(text_object, "text", "") or ""
    effects = getattr(text_object, "effects", None)
    font = effects.font if effects else None
    if not text_str or font is None:
        return None

    text_width = _stroke_text_width(text_str) * float(font.size_x)
    text_height = (22.0 / 21.0) * float(font.size_y)
    margin = _DIM_TEXT_MARGIN_RATIO * float(font.size_y)

    h_align, v_align = _dimension_text_alignment(text_object)
    if h_align == "left":
        min_x = 0.0
        max_x = text_width
    elif h_align == "right":
        min_x = -text_width
        max_x = 0.0
    else:
        min_x = -text_width / 2.0
        max_x = text_width / 2.0

    if v_align == "top":
        min_y = 0.0
        max_y = text_height
    elif v_align == "bottom":
        min_y = -text_height
        max_y = 0.0
    else:
        min_y = -text_height / 2.0
        max_y = text_height / 2.0

    min_x -= margin
    max_x += margin
    min_y -= margin
    max_y += margin

    angle = float(getattr(text_object, "at_angle", 0.0) or 0.0)
    corners = (
        (min_x, min_y),
        (max_x, min_y),
        (max_x, max_y),
        (min_x, max_y),
    )
    transformed = [
        _vec_add(
            _rotate_vector_kicad((x, y), angle),
            (float(text_object.at_x), float(text_object.at_y)),
        )
        for x, y in corners
    ]
    xs = [x for x, _y in transformed]
    ys = [y for _x, y in transformed]
    return (min(xs), min(ys), max(xs), max(ys))


def _segment_box_intersection(
    start: tuple[float, float],
    target: tuple[float, float],
    box: tuple[float, float, float, float],
) -> tuple[float, float] | None:
    min_x, min_y, max_x, max_y = box
    dx = target[0] - start[0]
    dy = target[1] - start[1]
    candidates: list[tuple[float, float, float]] = []
    eps = 1e-12

    if abs(dx) > eps:
        for x in (min_x, max_x):
            t = (x - start[0]) / dx
            y = start[1] + t * dy
            if -eps <= t <= 1.0 + eps and min_y - eps <= y <= max_y + eps:
                candidates.append((max(0.0, min(1.0, t)), x, y))
    if abs(dy) > eps:
        for y in (min_y, max_y):
            t = (y - start[1]) / dy
            x = start[0] + t * dx
            if -eps <= t <= 1.0 + eps and min_x - eps <= x <= max_x + eps:
                candidates.append((max(0.0, min(1.0, t)), x, y))

    if not candidates:
        return None
    _t, x, y = min(candidates, key=lambda item: item[0])
    return (x, y)


def _dimension_text_connector_end(
    start: tuple[float, float],
    text_object: "GrText",
) -> tuple[float, float]:
    target = (float(text_object.at_x), float(text_object.at_y))
    box = _dimension_text_box(text_object)
    if box is None:
        return target
    return _segment_box_intersection(start, target, box) or target


def _dimension_text_frame_ops(
    text_object: "GrText",
    *,
    width_mm: float,
    layer: str,
) -> list[KiCadPlotterOp]:
    box = _dimension_text_box(text_object)
    if box is None:
        return []
    min_x, min_y, max_x, max_y = box
    left_top = (min_x, min_y)
    left_bottom = (min_x, max_y)
    right_bottom = (max_x, max_y)
    right_top = (max_x, min_y)
    return [
        _layered_segment_op(left_top, left_bottom, width_mm=width_mm, layer=layer),
        _layered_segment_op(left_bottom, right_bottom, width_mm=width_mm, layer=layer),
        _layered_segment_op(right_bottom, right_top, width_mm=width_mm, layer=layer),
        _layered_segment_op(right_top, left_top, width_mm=width_mm, layer=layer),
    ]


def _dimension_arrow_ops(
    start: tuple[float, float],
    angle_deg: float,
    *,
    arrow_length: float,
    tail_length: float = 0.0,
    width_mm: float,
    layer: str,
) -> list[KiCadPlotterOp]:
    ops: list[KiCadPlotterOp] = []
    if tail_length:
        tail = _rotate_vector_kicad((tail_length, 0.0), -angle_deg)
        ops.append(
            _layered_segment_op(
                start,
                _vec_add(start, tail),
                width_mm=width_mm,
                layer=layer,
            )
        )

    for delta in (_DIM_ARROW_ANGLE_DEG, -_DIM_ARROW_ANGLE_DEG):
        end = _vec_add(
            start,
            _rotate_vector_kicad((arrow_length, 0.0), -angle_deg + delta),
        )
        ops.append(_layered_segment_op(start, end, width_mm=width_mm, layer=layer))
    return ops


def _dimension_aligned_shape_ops(dimension: "Dimension") -> list[KiCadPlotterOp]:
    if len(dimension.points) < 2:
        return []
    start, end = dimension.points[0], dimension.points[1]
    dim_vec = _vec_sub(end, start)
    if _vec_len(dim_vec) == 0.0:
        return []

    style = dimension.style
    width = float(style.thickness)
    layer = dimension.layer
    extension = (-dim_vec[1], dim_vec[0]) if dimension.height > 0.0 else (dim_vec[1], -dim_vec[0])
    extension_height = abs(dimension.height) - style.extension_offset + style.extension_height

    ops: list[KiCadPlotterOp] = []
    for feature_point in (start, end):
        ext_start = _vec_add(feature_point, _vec_resize(extension, style.extension_offset))
        ext_end = _vec_add(ext_start, _vec_resize(extension, extension_height))
        ops.append(_layered_segment_op(ext_start, ext_end, width_mm=width, layer=layer))

    crossbar = dimension._aligned_crossbar()
    if crossbar is None:
        return ops
    crossbar_start, crossbar_end = crossbar
    ops.append(_layered_segment_op(crossbar_start, crossbar_end, width_mm=width, layer=layer))

    dim_angle = _vec_angle(dim_vec)
    if style.arrow_direction == "inward":
        tail = style.arrow_length * _DIM_INWARD_ARROW_TAIL_RATIO
        ops.extend(
            _dimension_arrow_ops(
                crossbar_start,
                dim_angle + 180.0,
                arrow_length=style.arrow_length,
                tail_length=tail,
                width_mm=width,
                layer=layer,
            )
        )
        ops.extend(
            _dimension_arrow_ops(
                crossbar_end,
                dim_angle,
                arrow_length=style.arrow_length,
                tail_length=tail,
                width_mm=width,
                layer=layer,
            )
        )
    else:
        ops.extend(
            _dimension_arrow_ops(
                crossbar_start,
                dim_angle,
                arrow_length=style.arrow_length,
                width_mm=width,
                layer=layer,
            )
        )
        ops.extend(
            _dimension_arrow_ops(
                crossbar_end,
                dim_angle + 180.0,
                arrow_length=style.arrow_length,
                width_mm=width,
                layer=layer,
            )
        )
    return ops


def _dimension_orthogonal_shape_ops(dimension: "Dimension") -> list[KiCadPlotterOp]:
    if len(dimension.points) < 2:
        return []
    start, end = dimension.points[0], dimension.points[1]
    crossbar = dimension._orthogonal_crossbar()
    if crossbar is None:
        return []
    crossbar_start, crossbar_end = crossbar
    style = dimension.style
    width = float(style.thickness)
    layer = dimension.layer
    extension = (dimension.height, 0.0) if dimension.orientation == 1 else (0.0, dimension.height)
    extension_height = abs(dimension.height) - style.extension_offset + style.extension_height

    ops: list[KiCadPlotterOp] = []
    ext_start = _vec_add(start, _vec_resize(extension, style.extension_offset))
    ext_end = _vec_add(ext_start, _vec_resize(extension, extension_height))
    ops.append(_layered_segment_op(ext_start, ext_end, width_mm=width, layer=layer))

    end_extension = _vec_sub(end, crossbar_end)
    end_extension_len = _vec_len(end_extension)
    if end_extension_len:
        end_extension_height = end_extension_len - style.extension_offset + style.extension_height
        ext_start = _vec_sub(crossbar_end, _vec_resize(end_extension, style.extension_height))
        ext_end = _vec_add(ext_start, _vec_resize(end_extension, end_extension_height))
        ops.append(_layered_segment_op(ext_start, ext_end, width_mm=width, layer=layer))
    else:
        # CLI emits a 0.1 mm filled marker dot at the second reference
        # point when no extension line is needed (the crossbar passes
        # through the reference point exactly).
        ops.append(
            _op_with_pcb_layer(
                KiCadPlotterOp.circle(
                    cx=mm_to_nm(end[0]),
                    cy=mm_to_nm(end[1]),
                    diameter_nm=mm_to_nm(0.2),
                    fill=KiCadFillType.FILLED_SHAPE,
                    width_nm=0,
                ),
                layer,
            )
        )

    ops.append(_layered_segment_op(crossbar_start, crossbar_end, width_mm=width, layer=layer))

    crossbar_angle = _vec_angle(_vec_sub(crossbar_end, crossbar_start))
    if style.arrow_direction == "inward":
        tail = style.arrow_length * _DIM_INWARD_ARROW_TAIL_RATIO
        ops.extend(
            _dimension_arrow_ops(
                crossbar_start,
                crossbar_angle + 180.0,
                arrow_length=style.arrow_length,
                tail_length=tail,
                width_mm=width,
                layer=layer,
            )
        )
        ops.extend(
            _dimension_arrow_ops(
                crossbar_end,
                crossbar_angle,
                arrow_length=style.arrow_length,
                tail_length=tail,
                width_mm=width,
                layer=layer,
            )
        )
    else:
        ops.extend(
            _dimension_arrow_ops(
                crossbar_start,
                crossbar_angle,
                arrow_length=style.arrow_length,
                width_mm=width,
                layer=layer,
            )
        )
        ops.extend(
            _dimension_arrow_ops(
                crossbar_end,
                crossbar_angle + 180.0,
                arrow_length=style.arrow_length,
                width_mm=width,
                layer=layer,
            )
        )
    return ops


def _dimension_radial_shape_ops(dimension: "Dimension") -> list[KiCadPlotterOp]:
    if len(dimension.points) < 2:
        return []
    center, radius_point = dimension.points[0], dimension.points[1]
    style = dimension.style
    width = float(style.thickness)
    layer = dimension.layer
    arm = (0.0, style.arrow_length)
    radial = _vec_sub(radius_point, center)
    if _vec_len(radial) == 0.0:
        return []

    ops = [
        _layered_segment_op(_vec_sub(center, arm), _vec_add(center, arm), width_mm=width, layer=layer),
    ]
    arm = _rotate_vector_kicad(arm, -90.0)
    ops.append(
        _layered_segment_op(_vec_sub(center, arm), _vec_add(center, arm), width_mm=width, layer=layer)
    )
    text_object = dimension.resolved_gr_text()
    if text_object is not None and text_object.text:
        leader_end = _dimension_text_connector_end(radius_point, text_object)
    else:
        leader_length = (
            float(dimension.leader_length)
            if dimension.leader_length is not None
            else style.arrow_length * 3.0
        )
        leader_end = _vec_add(radius_point, _vec_resize(radial, leader_length))
    ops.append(_layered_segment_op(radius_point, leader_end, width_mm=width, layer=layer))

    # KiCad stops the radial connector at the logical text box edge rather
    # than drawing through to the value text anchor.

    ops.extend(
        _dimension_arrow_ops(
            radius_point,
            _vec_angle(radial),
            arrow_length=style.arrow_length,
            width_mm=width,
            layer=layer,
        )
    )
    return ops


def _dimension_leader_shape_ops(dimension: "Dimension") -> list[KiCadPlotterOp]:
    if len(dimension.points) < 2:
        return []
    start, end = dimension.points[0], dimension.points[1]
    first_line = _vec_sub(end, start)
    if _vec_len(first_line) == 0.0:
        return []
    style = dimension.style
    width = float(style.thickness)
    layer = dimension.layer
    arrow_start = _vec_add(start, _vec_resize(first_line, style.extension_offset))
    ops = [
        _layered_segment_op(arrow_start, end, width_mm=width, layer=layer),
    ]
    ops.extend(
        _dimension_arrow_ops(
            arrow_start,
            _vec_angle(first_line),
            arrow_length=style.arrow_length,
            width_mm=width,
            layer=layer,
        )
    )
    text_object = dimension.resolved_gr_text()
    if text_object is not None and text_object.text:
        # text_frame == 1 draws the logical text box before the connector.
        if style.text_frame == 1:
            ops.extend(_dimension_text_frame_ops(text_object, width_mm=width, layer=layer))
        text_pos = _dimension_text_connector_end(end, text_object)
        if _vec_len(_vec_sub(text_pos, end)) > 0.0:
            ops.append(_layered_segment_op(end, text_pos, width_mm=width, layer=layer))
    return ops


def _dimension_center_shape_ops(dimension: "Dimension") -> list[KiCadPlotterOp]:
    if len(dimension.points) < 2:
        return []
    center, end = dimension.points[0], dimension.points[1]
    arm = _vec_sub(end, center)
    if _vec_len(arm) == 0.0:
        return []
    width = float(dimension.style.thickness)
    layer = dimension.layer
    ops = [
        _layered_segment_op(_vec_sub(center, arm), _vec_add(center, arm), width_mm=width, layer=layer)
    ]
    arm = _rotate_vector_kicad(arm, -90.0)
    ops.append(
        _layered_segment_op(_vec_sub(center, arm), _vec_add(center, arm), width_mm=width, layer=layer)
    )
    return ops


def dimension_shape_ops(dimension: "Dimension") -> list[KiCadPlotterOp]:
    if dimension.dimension_type == "orthogonal":
        return _dimension_orthogonal_shape_ops(dimension)
    if dimension.dimension_type == "radial":
        return _dimension_radial_shape_ops(dimension)
    if dimension.dimension_type == "leader":
        return _dimension_leader_shape_ops(dimension)
    if dimension.dimension_type == "center":
        return _dimension_center_shape_ops(dimension)
    return _dimension_aligned_shape_ops(dimension)


def zone_to_record(zone: "Zone") -> KiCadPlotterRecord:
    """
    Bundle every ``filled_polygon`` of a zone into one record.

    The record's ``extras["fill_layers"]`` enumerates the per-ring
    layer names so consumers can split / colour-key without re-walking
    the source ``Zone``.
    """
    ops: list[KiCadPlotterOp] = []
    fill_layers: list[str] = []
    fill_island: list[bool] = []
    for fpoly in zone.filled_polygons:
        ops.append(zone_filled_polygon_to_op(fpoly))
        fill_layers.append(fpoly.layer)
        fill_island.append(bool(fpoly.island))
    extras: dict[str, Any] = {
        "layers": list(zone.layers),
        "fill_layers": fill_layers,
        "fill_island": fill_island,
    }
    extras.update(_net_extras(zone.net))
    return KiCadPlotterRecord(
        uuid=zone.uuid or "",
        kind="zone_fill",
        object_id="zone",
        operations=ops,
        extras=extras,
    )


def table_cell_text_to_op(
    cell: "TableCell",
    table: "Table",
    board: "KiCadPcb",
    *,
    object_path: str = "",
) -> KiCadPlotterOp | None:
    if not cell.text or cell.effects is None:
        return None
    font = cell.effects.font if cell.effects else None
    if not (cell.render_cache or (font and font.face)):
        return None
    request = render_cache_request_for_table_cell(
        cell,
        table,
        board,
        object_path=object_path,
        include_text_params=bool(font and font.face),
    )
    op = _text_op_from_render_cache_request(request, cell.effects)
    if op is None:
        return None
    return _op_with_pcb_layer(op, cell.layer)


def table_to_record(table: "Table", board: "KiCadPcb") -> KiCadPlotterRecord:
    ops: list[KiCadPlotterOp] = []
    if table.cells:
        min_x = min(min(cell.start_x, cell.end_x) for cell in table.cells)
        max_x = max(max(cell.start_x, cell.end_x) for cell in table.cells)
        min_y = min(min(cell.start_y, cell.end_y) for cell in table.cells)
        max_y = max(max(cell.start_y, cell.end_y) for cell in table.cells)
        x_coords = sorted({cell.start_x for cell in table.cells} | {cell.end_x for cell in table.cells})
        y_coords = sorted({cell.start_y for cell in table.cells} | {cell.end_y for cell in table.cells})

        sep_width = float(table.separators_stroke.width) if table.separators_stroke else 0.2
        if table.separators_cols:
            for x in (coord for coord in x_coords if coord != min_x and coord != max_x):
                for index in range(len(y_coords) - 1):
                    ops.append(
                        _layered_segment_op(
                            (x, y_coords[index]),
                            (x, y_coords[index + 1]),
                            width_mm=sep_width,
                            layer=table.layer,
                        )
                    )

        if table.separators_rows:
            for y in (coord for coord in y_coords if coord != min_y and coord != max_y):
                for index in range(len(x_coords) - 1):
                    ops.append(
                        _layered_segment_op(
                            (x_coords[index + 1], y),
                            (x_coords[index], y),
                            width_mm=sep_width,
                            layer=table.layer,
                        )
                    )

        if table.border_external:
            border_width = float(table.border_stroke.width) if table.border_stroke else 0.2
            for start, end in (
                ((min_x, min_y), (max_x, min_y)),
                ((max_x, min_y), (max_x, max_y)),
                ((max_x, max_y), (min_x, max_y)),
                ((min_x, max_y), (min_x, min_y)),
            ):
                ops.append(
                    _layered_segment_op(
                        start,
                        end,
                        width_mm=border_width,
                        layer=table.layer,
                    )
                )

    for cell_index, cell in enumerate(table.cells):
        op = table_cell_text_to_op(
            cell,
            table,
            board,
            object_path=f"table[{getattr(table, 'uuid', '')}]/cell[{cell_index}]",
        )
        if op is not None:
            ops.append(op)
    return KiCadPlotterRecord(
        uuid=table.uuid or "",
        kind="table",
        object_id="table",
        operations=ops,
        extras={
            "layers": sorted({table.layer, *(cell.layer for cell in table.cells)}),
            "cell_count": len(table.cells),
        },
    )


def _dimension_stroke_text_ops(
    text_object: "GrText", *, layer: str
) -> list[KiCadPlotterOp]:
    """Tessellate stroke-font dimension text to per-segment ``thick_segment`` ops.

    KiCad's plotter records dimension value text one ``MoveTo``/``LineTo``
    pair at a time, so ``kicad-cli pcb export svg`` emits one ``<path>``
    per individual line segment of each Newstroke glyph stroke. To reach
    SVG-element parity for the L3_007 oracle, this helper renders the
    text via :class:`KiCadStrokeFontRenderer` and emits one
    ``KiCadPlotterOp.thick_segment`` per consecutive pair of polyline
    points — each becomes one ``<polyline>`` in the IR renderer's
    output, which counts identically to a CLI ``<path>`` under the
    oracle's renderer-agnostic ``total_strokes`` metric (``<path> +
    <polyline> + <line> + <rect> + <polygon>``).
    """
    from .kicad_stroke_font import get_renderer

    text_str = getattr(text_object, "text", "") or ""
    if not text_str or bool(getattr(text_object, "hide", False)):
        return []
    effects = getattr(text_object, "effects", None)
    font = effects.font if effects else None
    if font is None:
        return []

    h_align = "center"
    v_align = "center"
    mirror = False
    justify = getattr(effects, "justify", None) or []
    for tok in justify:
        if tok in ("left", "right", "center"):
            h_align = tok
        elif tok in ("top", "bottom"):
            v_align = tok
        elif tok == "mirror":
            mirror = True

    polylines = get_renderer().render_text_polylines(
        text=text_str,
        pos_x=float(text_object.at_x),
        pos_y=float(text_object.at_y),
        size_x=float(font.size_x),
        size_y=float(font.size_y),
        angle=float(getattr(text_object, "at_angle", 0.0)),
        h_align=h_align,
        v_align=v_align,
        mirror=mirror,
        italic=bool(font.italic),
    )
    thickness_nm = mm_to_nm(font.effective_thickness)
    ops: list[KiCadPlotterOp] = []
    for polyline in polylines:
        if len(polyline) < 2:
            continue
        for i in range(len(polyline) - 1):
            x0, y0 = polyline[i]
            x1, y1 = polyline[i + 1]
            seg = KiCadPlotterOp.thick_segment(
                start_x=mm_to_nm(x0),
                start_y=mm_to_nm(y0),
                end_x=mm_to_nm(x1),
                end_y=mm_to_nm(y1),
                width_nm=thickness_nm,
            )
            ops.append(_op_with_pcb_layer(seg, layer))
    return ops


def dimension_text_to_record(
    dimension: "Dimension",
    board: "KiCadPcb",
    *,
    index: int = 0,
) -> KiCadPlotterRecord:
    text_object = (
        dimension.resolved_gr_text()
        if hasattr(dimension, "resolved_gr_text")
        else getattr(dimension, "gr_text", None)
    )
    if text_object is None:
        return KiCadPlotterRecord(
            uuid=dimension.uuid or "",
            kind="dimension",
            object_id="dimension",
            operations=dimension_shape_ops(dimension),
            extras={
                "layers": [dimension.layer],
                "dimension_type": dimension.dimension_type,
            },
        )

    text_layer = text_object.layer or dimension.layer
    font = text_object.effects.font if text_object.effects else None
    ops: list[KiCadPlotterOp] = []

    if font is not None and font.face:
        # TTF-faced dimension text → use the typed render-cache pipeline
        # so glyphs emit as ``<path>`` polygons matching ``kicad-cli``.
        request = render_cache_request_for_dimension_text(
            dimension, board, include_text_params=True
        )
        text_op = _text_op_from_render_cache_request(request, text_object.effects)
        if text_op is not None:
            ops.append(_op_with_pcb_layer(text_op, text_layer))
    else:
        # Stroke-font dimension text → emit one ``thick_segment`` per
        # Newstroke line segment for per-segment parity with the CLI
        # plotter recording granularity (one SVG element per segment).
        ops.extend(_dimension_stroke_text_ops(text_object, layer=text_layer))

    ops.extend(dimension_shape_ops(dimension))
    resolved_text = str(getattr(text_object, "text", ""))
    return KiCadPlotterRecord(
        uuid=dimension.uuid or text_object.uuid or "",
        kind="dimension",
        object_id="dimension",
        operations=ops,
        extras={
            "layers": sorted({dimension.layer, text_layer}),
            "text": resolved_text,
            "dimension_type": dimension.dimension_type,
        },
    )


def _footprint_component_attrs(footprint: "Footprint") -> dict[str, Any]:
    get_property = getattr(footprint, "get_property_value", None)
    component = get_property("Reference", "") if callable(get_property) else ""
    return {
        "component": component,
        "component_uid": getattr(footprint, "uuid", "") or "",
        "component_uuid": getattr(footprint, "uuid", "") or "",
        "footprint": footprint.library_link,
    }


def _footprint_child_label(
    footprint: "Footprint",
    primitive: str,
    source: Any,
    *,
    index: int,
    sub_index: int | None = None,
) -> tuple[str, str]:
    source_uuid = str(getattr(source, "uuid", "") or "")
    owner = str(getattr(footprint, "uuid", "") or footprint.library_link or "footprint")
    suffix = f":{sub_index}" if sub_index is not None else ""
    label = f"{source_uuid or owner}:{primitive}:{index}{suffix}"
    return label, source_uuid or label


def _footprint_layer_attrs(layer: object) -> dict[str, Any]:
    layer_name = str(layer or "")
    if not layer_name:
        return {}
    return {
        "layer_name": layer_name,
        "layer_role": pcb_layer_role(layer_name),
    }


def _footprint_text_role(kind: object, *, property_name: object = "") -> str:
    source = str(property_name or kind or "").lower()
    if source == "reference":
        return "designator"
    if source == "value":
        return "value"
    if property_name:
        return "property"
    return "user"


def _op_with_footprint_child_metadata(
    op: KiCadPlotterOp,
    footprint: "Footprint",
    source: Any,
    *,
    data_ref: str,
    primitive: str,
    object_id: str,
    index: int,
    layer: str,
    sub_index: int | None = None,
    extra_attrs: dict[str, Any] | None = None,
) -> KiCadPlotterOp:
    label, data_uuid = _footprint_child_label(
        footprint, primitive, source, index=index, sub_index=sub_index
    )
    attrs = {
        **_footprint_component_attrs(footprint),
        **_footprint_layer_attrs(layer),
        "primitive": primitive,
        "footprint_primitive": data_ref,
        "footprint_object_index": index,
    }
    if sub_index is not None:
        attrs["footprint_subop_index"] = sub_index
    if extra_attrs:
        attrs.update(extra_attrs)

    payload = copy.deepcopy(op.payload)
    payload["label"] = label
    payload["data_uuid"] = data_uuid
    payload["data_ref"] = data_ref
    payload["object_id"] = object_id
    merged_extra_attrs = dict(payload.get("extra_attrs") or {})
    merged_extra_attrs.update(attrs)
    payload["extra_attrs"] = merged_extra_attrs
    return KiCadPlotterOp(kind=op.kind, payload=payload)


def _op_with_footprint_text_metadata(
    op: KiCadPlotterOp,
    footprint: "Footprint",
    source: Any,
    *,
    data_ref: str,
    object_id: str,
    index: int,
    layer: str,
    text_role: str,
    sub_index: int | None = None,
    extra_attrs: dict[str, Any] | None = None,
) -> KiCadPlotterOp:
    attrs = {
        "footprint_text_role": text_role,
        **(extra_attrs or {}),
    }
    return _op_with_footprint_child_metadata(
        op,
        footprint,
        source,
        data_ref=data_ref,
        primitive="footprint-text",
        object_id=object_id,
        index=index,
        sub_index=sub_index,
        layer=layer,
        extra_attrs=attrs,
    )


def _op_with_footprint_graphic_metadata(
    op: KiCadPlotterOp,
    footprint: "Footprint",
    source: Any,
    *,
    data_ref: str,
    graphic_kind: str,
    object_id: str,
    index: int,
    layer: str,
    sub_index: int | None = None,
) -> KiCadPlotterOp:
    return _op_with_footprint_child_metadata(
        op,
        footprint,
        source,
        data_ref=data_ref,
        primitive="footprint-graphic",
        object_id=object_id,
        index=index,
        sub_index=sub_index,
        layer=layer,
        extra_attrs={"footprint_graphic_kind": graphic_kind},
    )


def pcb_footprint_to_record(
    footprint: "Footprint",
    *,
    board: "KiCadPcb | None" = None,
    net_table: "NetTable | None" = None,
    net_name_to_classes: Mapping[str, Sequence[str]] | None = None,
    layer_flash_resolver: PcbLayerFlashResolver | None = None,
) -> KiCadPlotterRecord:
    """
    Convert a PCB-embedded :class:`Footprint` to a :class:`KiCadPlotterRecord`.

    Distinct from :func:`footprint_to_record`, which targets the
    standalone :class:`KiCadFootprint` (`.kicad_mod`). The PCB-embedded
    variant uses ``library_link`` instead of ``name`` and carries a
    placement transform (``at_x``, ``at_y``, ``at_angle``).

    Geometry ops are emitted in footprint-local coordinates; the placement
    transform is stored in
    ``extras["placement"] = {"x_nm","y_nm","angle_deg"}`` so downstream
    renderers can position it on the board. Op order matches the standalone
    footprint converter:

        properties → fp_texts → fp_lines → fp_arcs → fp_circles →
        fp_rects → fp_polys → pads
    """
    if board is not None:
        if net_table is None:
            net_table = board.net_table()
        if net_name_to_classes is None:
            net_name_to_classes = _net_class_snapshot(board)
    resolved_net_name_to_classes = net_name_to_classes or {}

    ops: list[KiCadPlotterOp] = []

    # Reference + Value first (matches to_sexp ordering), then others.
    ref_prop = next((p for p in footprint.properties if p.name == "Reference"), None)
    val_prop = next((p for p in footprint.properties if p.name == "Value"), None)
    other_props = [
        p for p in footprint.properties if p.name not in ("Reference", "Value")
    ]
    ordered_props = [p for p in (ref_prop, val_prop) if p is not None] + other_props
    for prop_index, prop in enumerate(ordered_props):
        op = property_to_op(prop)
        if op is not None:
            font = prop.effects.font if prop.effects else None
            if prop.render_cache or (font and font.face):
                request = render_cache_request_for_footprint_property(
                    prop,
                    footprint,
                    include_text_params=bool(font and font.face),
                )
                payload = copy.deepcopy(op.payload)
                payload["text"] = request.text
                op = _op_with_render_cache_payload(
                    KiCadPlotterOp(kind=op.kind, payload=payload),
                    request,
                    footprint=footprint,
                )
            op = _op_with_footprint_text_metadata(
                op,
                footprint,
                prop,
                data_ref="property",
                object_id=prop.name,
                index=prop_index,
                layer=prop.layer,
                text_role=_footprint_text_role(prop.name, property_name=prop.name),
                extra_attrs={"property_name": prop.name},
            )
            ops.append(_op_with_pcb_layer(op, prop.layer))

    variables = _footprint_text_variables(footprint.properties)
    for text_index, txt in enumerate(footprint.fp_texts):
        op = fp_text_to_op(txt, variables=variables)
        if op is not None:
            font = txt.effects.font if txt.effects else None
            if txt.render_cache or (font and font.face):
                request = render_cache_request_for_footprint_text(
                    txt,
                    footprint,
                    include_text_params=bool(font and font.face),
                )
                payload = copy.deepcopy(op.payload)
                payload["text"] = request.text
                op = _op_with_render_cache_payload(
                    KiCadPlotterOp(kind=op.kind, payload=payload),
                    request,
                    footprint=footprint,
                )
            if getattr(txt, "knockout", False) and font is not None:
                margin_mm = max(
                    font.effective_thickness / 2.0,
                    font.size_y / 9.0,
                )
                op = _apply_knockout_to_text_op(
                    op, knockout_margin_nm=mm_to_nm(margin_mm)
                )
            text_type = str(getattr(txt, "text_type", "") or "")
            op = _op_with_footprint_text_metadata(
                op,
                footprint,
                txt,
                data_ref="fp_text",
                object_id=f"{text_type}:{text_index}" if text_type else str(text_index),
                index=text_index,
                layer=txt.layer,
                text_role=_footprint_text_role(text_type),
                extra_attrs={"fp_text_type": text_type},
            )
            ops.append(_op_with_pcb_layer(op, txt.layer))
    for text_box_index, text_box in enumerate(getattr(footprint, "fp_text_boxes", []) or []):
        text_box_ops = fp_text_box_to_ops(text_box, variables=variables)
        font = text_box.effects.font if text_box.effects else None
        if text_box.render_cache or (font and font.face):
            request = render_cache_request_for_footprint_text_box(
                text_box,
                footprint,
                include_text_params=bool(font and font.face),
            )
            for index, op in enumerate(text_box_ops):
                if not _is_text_op(op):
                    continue
                payload = copy.deepcopy(op.payload)
                payload["text"] = request.text
                text_box_ops[index] = _op_with_render_cache_payload(
                    KiCadPlotterOp(kind=op.kind, payload=payload),
                    request,
                    footprint=footprint,
                )
                break
        for op_index, op in enumerate(text_box_ops):
            if _is_text_op(op):
                op = _op_with_footprint_text_metadata(
                    op,
                    footprint,
                    text_box,
                    data_ref="fp_text_box",
                    object_id=f"text_box:{text_box_index}",
                    index=text_box_index,
                    sub_index=op_index,
                    layer=text_box.layer,
                    text_role="user",
                )
            else:
                op = _op_with_footprint_graphic_metadata(
                    op,
                    footprint,
                    text_box,
                    data_ref="fp_text_box",
                    graphic_kind="text-box-border",
                    object_id=f"text_box:{text_box_index}",
                    index=text_box_index,
                    sub_index=op_index,
                    layer=text_box.layer,
                )
            ops.append(_op_with_pcb_layer(op, text_box.layer))
    for graphic_index, line in enumerate(footprint.fp_lines):
        for op_index, op in enumerate(fp_line_to_ops(line)):
            op = _op_with_footprint_graphic_metadata(
                op,
                footprint,
                line,
                data_ref="fp_line",
                graphic_kind="line",
                object_id=f"line:{graphic_index}",
                index=graphic_index,
                sub_index=op_index,
                layer=line.layer,
            )
            ops.append(_op_with_pcb_layer(op, line.layer))
    for graphic_index, arc in enumerate(footprint.fp_arcs):
        for op_index, op in enumerate(fp_arc_to_ops(arc)):
            op = _op_with_footprint_graphic_metadata(
                op,
                footprint,
                arc,
                data_ref="fp_arc",
                graphic_kind="arc",
                object_id=f"arc:{graphic_index}",
                index=graphic_index,
                sub_index=op_index,
                layer=arc.layer,
            )
            ops.append(_op_with_pcb_layer(op, arc.layer))
    for graphic_index, circle in enumerate(footprint.fp_circles):
        op = _op_with_footprint_graphic_metadata(
            fp_circle_to_op(circle),
            footprint,
            circle,
            data_ref="fp_circle",
            graphic_kind="circle",
            object_id=f"circle:{graphic_index}",
            index=graphic_index,
            layer=circle.layer,
        )
        ops.append(_op_with_pcb_layer(op, circle.layer))
    for graphic_index, rect in enumerate(footprint.fp_rects):
        op = _op_with_footprint_graphic_metadata(
            fp_rect_to_op(rect),
            footprint,
            rect,
            data_ref="fp_rect",
            graphic_kind="rect",
            object_id=f"rect:{graphic_index}",
            index=graphic_index,
            layer=rect.layer,
        )
        ops.append(_op_with_pcb_layer(op, rect.layer))
    for graphic_index, poly in enumerate(footprint.fp_polys):
        op = _op_with_footprint_graphic_metadata(
            fp_poly_to_op(poly),
            footprint,
            poly,
            data_ref="fp_poly",
            graphic_kind="poly",
            object_id=f"poly:{graphic_index}",
            index=graphic_index,
            layer=poly.layer,
        )
        ops.append(_op_with_pcb_layer(op, poly.layer))
    for pad_index, pad in enumerate(footprint.pads):
        pad_orient_offset = -float(getattr(footprint, "at_angle", 0.0) or 0.0)
        flashed_layers = (
            layer_flash_resolver.pad_item_layers(pad, footprint)
            if layer_flash_resolver is not None
            else None
        )
        ops.extend(
            _pad_block_ops(
                pad,
                footprint,
                board,
                net_table,
                resolved_net_name_to_classes,
                pad_index=pad_index,
                pad_orient_offset=pad_orient_offset,
                flashed_layers=flashed_layers,
                drill_layers=(
                    layer_flash_resolver.copper_stack
                    if layer_flash_resolver is not None
                    and layer_flash_resolver.copper_stack
                    else None
                ),
            )
        )

    get_property = getattr(footprint, "get_property_value", None)
    extras: dict[str, Any] = {
        "library_link": footprint.library_link,
        "reference": get_property("Reference", "") if callable(get_property) else "",
        "value": get_property("Value", "") if callable(get_property) else "",
        "layer": footprint.layer,
        "locked": bool(footprint.locked),
        "descr": footprint.descr,
        "tags": footprint.tags,
        "attr": list(footprint.attr) if footprint.attr else [],
        "placement": {
            "x_nm": mm_to_nm(footprint.at_x),
            "y_nm": mm_to_nm(footprint.at_y),
            "angle_deg": float(footprint.at_angle),
        },
    }

    return KiCadPlotterRecord(
        uuid=footprint.uuid or "",
        kind="footprint",
        object_id=footprint.library_link,
        bounds=None,
        operations=ops,
        extras=extras,
    )


# ---------------------------------------------------------------------------
# Top-level converter
# ---------------------------------------------------------------------------


def pcb_to_ir(
    pcb: "KiCadPcb",
    *,
    source_path: str | None = None,
    document_id: str | None = None,
) -> KiCadPlotterDocument:
    """
    Render a :class:`KiCadPcb` to a :class:`KiCadPlotterDocument`.

    Records are emitted in a stable per-category order:

        gr_lines → gr_arcs → gr_circles → gr_rects → gr_polys →
        gr_curves → gr_texts → segments → track_arcs → vias →
        zones → footprints

    Footprint records are built by :func:`pcb_footprint_to_record`
    (which reuses footprint per-element op emitters and carries the
    footprint's placement transform in ``extras["placement"]``); each
    footprint contributes one record with all of its properties /
    fp_texts / graphics / pads in canonical footprint order.
    """
    records: list[KiCadPlotterRecord] = []
    net_table = pcb.net_table()
    net_name_to_classes = _net_class_snapshot(pcb)
    layer_flash_resolver = PcbLayerFlashResolver.from_board(pcb)

    for line in pcb.gr_lines:
        records.append(gr_line_to_record(line))
    for arc in pcb.gr_arcs:
        records.append(gr_arc_to_record(arc))
    for circle in pcb.gr_circles:
        records.append(gr_circle_to_record(circle))
    for rect in pcb.gr_rects:
        records.append(gr_rect_to_record(rect))
    for poly in pcb.gr_polys:
        records.append(gr_poly_to_record(poly))
    for curve in pcb.gr_curves:
        records.append(gr_curve_to_record(curve))
    for text in pcb.gr_texts:
        records.append(gr_text_to_record(text, board=pcb))
    for text_box in getattr(pcb, "gr_text_boxes", []) or []:
        records.append(gr_text_box_to_record(text_box, board=pcb))

    for seg in pcb.segments:
        records.append(track_segment_to_record(seg))
    for arc in pcb.arcs:
        records.append(track_arc_to_record(arc))
    mask_clearance_mm = float(getattr(pcb, "pad_to_mask_clearance", 0.0) or 0.0)
    for via in pcb.vias:
        records.append(
            via_to_record(
                via,
                mask_clearance_mm=mask_clearance_mm,
                flashed_layers=layer_flash_resolver.via_flash_layers(via),
            )
        )

    for table in getattr(pcb, "tables", []) or []:
        records.append(table_to_record(table, pcb))

    for dimension_index, dimension in enumerate(getattr(pcb, "dimensions", []) or []):
        records.append(dimension_text_to_record(dimension, pcb, index=dimension_index))

    for zone in pcb.zones:
        records.append(zone_to_record(zone))

    for footprint in pcb.footprints:
        records.append(
            pcb_footprint_to_record(
                footprint,
                board=pcb,
                net_table=net_table,
                net_name_to_classes=net_name_to_classes,
                layer_flash_resolver=layer_flash_resolver,
            )
        )

    return KiCadPlotterDocument(
        records=_records_with_net_class_extras(records, net_name_to_classes),
        source_path=source_path,
        source_kind="PCB",
        document_id=document_id,
        canvas=None,
        coordinate_space={"unit": "nm", "y_axis": "down"},
        background_color=None,
        render_hints=None,
        extras={
            "version": int(pcb.version),
            "generator": str(pcb.generator),
            "generator_version": str(pcb.generator_version),
            "thickness_mm": float(pcb.thickness),
            "paper": str(pcb.paper),
        },
    )


__all__ = [
    "gr_arc_to_op",
    "gr_arc_to_record",
    "gr_circle_to_op",
    "gr_circle_to_record",
    "gr_curve_to_op",
    "gr_curve_to_record",
    "gr_line_to_op",
    "gr_line_to_record",
    "gr_poly_to_op",
    "gr_poly_to_record",
    "gr_rect_to_op",
    "gr_rect_to_record",
    "gr_text_to_op",
    "gr_text_to_record",
    "gr_text_box_to_ops",
    "gr_text_box_to_record",
    "pcb_footprint_to_record",
    "pcb_to_ir",
    "table_cell_text_to_op",
    "table_to_record",
    "dimension_shape_ops",
    "dimension_text_to_record",
    "track_arc_to_op",
    "track_arc_to_record",
    "track_segment_to_op",
    "track_segment_to_record",
    "via_drill_to_op",
    "via_to_op",
    "via_to_record",
    "zone_filled_polygon_to_op",
    "zone_to_record",
]

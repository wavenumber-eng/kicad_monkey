"""
KiCad Plotter-IR to SVG renderer.

Consumes a :class:`KiCadPlotterDocument` (the JSON-safe plotter-call IR
produced by the symbol, schematic, footprint, and PCB converters) and emits
an SVG string by dispatching
each :class:`KiCadPlotterOp` to the matching ``svg_*`` primitive in
:mod:`kicad_monkey.kicad_sch_svg_renderer`.

Cross-validation contract (formal):
    parser to SVG-direct equals parser to IR to SVG-from-IR

The renderer covers the schematic op kinds emitted by the symbol and
schematic converters:

  * ``Circle``         → ``svg_circle``
  * ``ArcThreePoint``  → ``svg_arc``
  * ``BezierCurve``    → ``svg_bezier``
  * ``Rect``           → ``svg_rect``
  * ``PlotPoly``       → ``svg_polygon`` (filled) / ``svg_polyline`` (open)
  * ``Text``           → ``svg_text_or_poly`` (multiline → per-line stack)

The dispatcher also supports PCB and footprint ops:

  * ``ThickSegment``        → ``svg_polyline`` with stroke width
  * ``ThickArc``            → centre+angle → 3-point ``svg_arc``
  * ``FlashPadCircle``      → filled ``svg_circle``
  * ``FlashPadRect``        → rotated 4-corner ``svg_polygon``
  * ``FlashPadOval``        → stadium polygon (rotated, ~32 verts)
  * ``FlashPadRoundRect``   → rotated rounded-rect polygon
  * ``FlashPadTrapez``      → rotated 4-corner ``svg_polygon``
  * ``FlashPadCustom``      → one ``svg_polygon`` per primitive ring
  * ``FlashRegularPolygon`` → rotated regular-polygon ``svg_polygon``

State / lifecycle ops (``SetColor``, ``SetCurrentLineWidth``,
``SetDash``, ``StartPlot``, ``EndPlot``, ``PenTo``,
``SetViewport``, ``SetPageSettings``) are silently skipped by the
primitive dispatcher. ``PlotImage`` embeds schematic and worksheet
bitmap payloads. The ``StartBlock`` / ``EndBlock`` pair renders
nested SVG groups for semantic sub-objects such as placed symbol pins.

Records with no draw ops (``sheet_header``, header-only
``symbol_instance`` / ``sheet``) emit an empty ``<g>`` group keyed by
the record's UUID so downstream tooling can hook on identity even
when the geometry is deferred.
"""

from __future__ import annotations

import html
import math
import re
from collections.abc import Iterable as IterableABC
from typing import Any, Callable, Iterable

from .kicad_plotter_ir import (
    KiCadFillType,
    KiCadLineStyle,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
)
from .kicad_pcb_svg_enrichment import (
    pcb_record_has_svg_data_attrs,
    pcb_record_svg_data_attrs,
    svg_attrs_to_string,
)
from .kicad_schematic_svg_enrichment import (
    schematic_record_has_svg_data_attrs,
    schematic_record_svg_data_attrs,
)
from .kicad_sch_svg_renderer import (
    KiCadSvgRenderContext,
    KiCadSvgRenderOptions,
    KiCadSvgRenderProfile,
    KiCadVariantDimMode,
    fmt_user_number,
    svg_arc,
    svg_bezier,
    svg_circle,
    svg_document,
    svg_group,
    svg_polygon,
    svg_polyline,
    svg_path,
    svg_rect,
    svg_text_poly,
    svg_text_or_poly,
)
from .kicad_variant_overlay import VARIANT_STATE_DIMMED, VARIANT_STATE_KEY


# =============================================================================
# Op dispatch
# =============================================================================


_TEXT_INTERLINE_FACTOR = 1.68


def _ki_round(value: float) -> int:
    if value >= 0:
        return int(math.floor(value + 0.5))
    return int(math.ceil(value - 0.5))


def _nm_to_schematic_iu(value_nm: int | float) -> int:
    return _ki_round(float(value_nm) / 100.0)


def _schematic_iu_to_nm(value_iu: int) -> int:
    return int(value_iu) * 100


def _is_filled(fill: str) -> bool:
    """Return True if the IR fill enum represents any solid/patterned fill."""
    return fill not in ("", KiCadFillType.NO_FILL.value)


def _rotate_text_offset(x: float, y: float, angle_deg: float) -> tuple[float, float]:
    a = float(angle_deg) % 360.0
    if a == 0.0:
        return x, y
    if a == 90.0:
        return -y, x
    if a == 180.0:
        return -x, -y
    if a == 270.0:
        return y, -x
    rad = math.radians(a)
    c, s = math.cos(rad), math.sin(rad)
    return x * c - y * s, x * s + y * c


def _multiline_text_positions(
    *,
    x: int,
    y: int,
    line_count: int,
    size_y_nm: int,
    orient_deg: float,
    v_align: str,
) -> list[tuple[int, int]]:
    line_step_iu = _ki_round(_nm_to_schematic_iu(size_y_nm) * _TEXT_INTERLINE_FACTOR)
    line_step = _schematic_iu_to_nm(line_step_iu)
    pos_y = y
    if line_count > 1:
        if v_align == "GR_TEXT_V_ALIGN_CENTER":
            pos_y -= _schematic_iu_to_nm((line_count - 1) * line_step_iu // 2)
        elif v_align == "GR_TEXT_V_ALIGN_BOTTOM":
            pos_y -= (line_count - 1) * line_step

    rel_x, rel_y = _rotate_text_offset(0, pos_y - y, -orient_deg)
    step_x, step_y = _rotate_text_offset(0, line_step, -orient_deg)
    pos_x = int(round(x + rel_x))
    pos_y = int(round(y + rel_y))
    step_x_i = int(round(step_x))
    step_y_i = int(round(step_y))

    out: list[tuple[int, int]] = []
    for _idx in range(line_count):
        out.append((pos_x, pos_y))
        pos_x += step_x_i
        pos_y += step_y_i
    return out


def _render_cache_path_d(
    contours: Iterable,
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    parts: list[str] = []
    for contour in contours:
        points = [
            (int(point[0]), int(point[1]))
            for point in contour
            if isinstance(point, (list, tuple)) and len(point) >= 2
        ]
        if len(points) < 3:
            continue
        first_x, first_y = points[0]
        commands = [
            f"M {fmt_user_number(ctx.to_user_x(first_x))},{fmt_user_number(ctx.to_user_y(first_y))}"
        ]
        commands.extend(
            f"L {fmt_user_number(ctx.to_user_x(x))},{fmt_user_number(ctx.to_user_y(y))}"
            for x, y in points[1:]
        )
        commands.append("Z")
        parts.append(" ".join(commands))
    return " ".join(parts)


def _render_typed_cache_polygons(
    render_cache: object,
    *,
    color: str,
    ctx: KiCadSvgRenderContext,
) -> str:
    if not isinstance(render_cache, dict):
        return ""
    polygons = render_cache.get("polygons")
    if not isinstance(polygons, list):
        return ""

    fragments: list[str] = []
    for polygon in polygons:
        if not isinstance(polygon, dict):
            continue
        contours = polygon.get("contours")
        if not isinstance(contours, list):
            continue
        d = _render_cache_path_d(contours, ctx=ctx)
        if not d:
            continue
        fragment = svg_path(
            d,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            fill_color=color,
            stroke_color=color,
            width_nm=0,
        )
        if len(contours) > 1 or _profile_is_oracle(ctx.options):
            fragment = fragment.replace(" />", ' fill-rule="evenodd" clip-rule="evenodd" />')
        fragments.append(fragment)
    return "\n".join(fragments)


def _render_text_op(op: KiCadPlotterOp, *, ctx: KiCadSvgRenderContext) -> str:
    """Render a single ``Text`` op, splitting multiline text into stacked lines."""
    p = op.payload
    text = str(p.get("text", ""))
    color = str(p.get("color", "#000000"))
    typed_cache_svg = _render_typed_cache_polygons(
        p.get("render_cache"),
        color=color,
        ctx=ctx,
    )
    if typed_cache_svg:
        return typed_cache_svg

    render_cache_polygons = p.get("render_cache_polygons", []) or []
    if render_cache_polygons:
        fragments: list[str] = []
        for polygon in render_cache_polygons:
            points = [(int(point[0]), int(point[1])) for point in polygon]
            if len(points) < 3:
                continue
            fragments.append(
                svg_polygon(
                    points,
                    ctx=ctx,
                    fill=KiCadFillType.FILLED_SHAPE.value,
                    fill_color=color,
                    stroke_color=color,
                    width_nm=0,
                )
            )
        return "\n".join(fragments)

    multiline = bool(p.get("multiline", False))
    size_y_nm = int(p.get("size_y_nm", 1_270_000))
    base_kwargs: dict[str, Any] = dict(
        ctx=ctx,
        color=color,
        size_x_nm=int(p.get("size_x_nm", size_y_nm)),
        size_y_nm=size_y_nm,
        orient_deg=float(p.get("orient_deg", 0.0)),
        h_align=str(p.get("h_align", "GR_TEXT_H_ALIGN_LEFT")),
        v_align=str(p.get("v_align", "GR_TEXT_V_ALIGN_BOTTOM")),
        italic=bool(p.get("italic", False)),
        bold=bool(p.get("bold", False)),
        font_face=str(p.get("font_face", "")),
        pen_width_nm=int(p.get("pen_width_nm", 0)),
        mirror=bool(p.get("mirror", False)),
    )
    x = int(p.get("x", 0))
    y = int(p.get("y", 0))
    text_renderer: Callable[..., str] = (
        svg_text_poly if bool(p.get("text_as_polygons", False)) else svg_text_or_poly
    )
    if text_renderer is svg_text_poly and p.get("polyline_per_segment") is not None:
        # Per-op granularity override (gr_text stroke font): kicad-cli's PCB
        # plotter records one path per stroke segment, not per polyline.
        base_kwargs["per_segment"] = bool(p.get("polyline_per_segment"))
    if not multiline or "\n" not in text:
        return text_renderer(x, y, text, **base_kwargs)
    lines = text.split("\n")
    positions = _multiline_text_positions(
        x=x,
        y=y,
        line_count=len(lines),
        size_y_nm=size_y_nm,
        orient_deg=base_kwargs["orient_deg"],
        v_align=base_kwargs["v_align"],
    )
    fragments: list[str] = []
    for line, (line_x, line_y) in zip(lines, positions):
        if line == "":
            continue
        fragments.append(text_renderer(line_x, line_y, line, **base_kwargs))
    return "\n".join(fragments)


def _render_plot_image_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    data_b64 = str(p.get("image_data_b64", "") or "")
    width_nm = int(round(float(p.get("width_nm", 0) or 0)))
    height_nm = int(round(float(p.get("height_nm", 0) or 0)))
    if not data_b64 or width_nm <= 0 or height_nm <= 0:
        return ""

    image_format = str(p.get("image_format", "png") or "png").lower()
    if image_format in {"jpg", "jpeg"}:
        mime = "image/jpeg"
    elif image_format == "bmp":
        mime = "image/bmp"
    elif image_format == "svg":
        mime = "image/svg+xml"
    else:
        mime = "image/png"

    center_x_nm = float(p.get("x", 0) or 0)
    center_y_nm = float(p.get("y", 0) or 0)
    x_nm = center_x_nm - width_nm / 2.0
    y_nm = center_y_nm - height_nm / 2.0

    attrs = [
        f'x="{fmt_user_number(ctx.to_user_x(x_nm))}"',
        f'y="{fmt_user_number(ctx.to_user_y(y_nm))}"',
        f'width="{fmt_user_number(ctx.to_user_length(width_nm))}"',
        f'height="{fmt_user_number(ctx.to_user_length(height_nm))}"',
        'preserveAspectRatio="none"',
        f'href="data:{mime};base64,{html.escape(data_b64, quote=True)}"',
    ]
    return "<image " + " ".join(attrs) + " />"


# =============================================================================
# Geometry helpers (PCB ops)
# =============================================================================


def _rotate_local_point(x: float, y: float, angle_deg: float) -> tuple[float, float]:
    """
    KiCad's ``RotatePoint`` (see ``libs/kimath/src/trigo.cpp``):

        rot(x, y, theta) = (x*cos(theta) + y*sin(theta),
                           -x*sin(theta) + y*cos(theta))

    Mathematically CCW; visually CW in KiCad's Y-down coordinate frame.
    """
    a = math.radians(angle_deg)
    s = math.sin(a)
    c = math.cos(a)
    return (x * c + y * s, -x * s + y * c)


def _absolutize(
    local_pts: Iterable[tuple[float, float]],
    *,
    cx: int,
    cy: int,
    orient_deg: float,
) -> list[tuple[int, int]]:
    """Rotate local points around the origin then translate by ``(cx, cy)``."""
    out: list[tuple[int, int]] = []
    for lx, ly in local_pts:
        rx, ry = _rotate_local_point(lx, ly, orient_deg)
        out.append((int(round(rx + cx)), int(round(ry + cy))))
    return out


def _rect_local_corners(size_x: int, size_y: int) -> list[tuple[float, float]]:
    """Four corners of an axis-aligned rectangle centred at (0, 0)."""
    hx = size_x / 2.0
    hy = size_y / 2.0
    return [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]


def _stadium_local_corners(
    size_x: int, size_y: int, *, segments_per_arc: int = 16
) -> list[tuple[float, float]]:
    """
    Stadium (oval pad) outline approximated as a polygon. The long axis
    follows the larger of ``size_x`` / ``size_y``.
    """
    pts: list[tuple[float, float]] = []
    if size_x >= size_y:
        r = size_y / 2.0
        half_straight = max(0.0, (size_x - size_y) / 2.0)
        # Right semicircle (-pi/2 -> +pi/2)
        for i in range(segments_per_arc + 1):
            t = -math.pi / 2 + math.pi * i / segments_per_arc
            pts.append((half_straight + r * math.cos(t), r * math.sin(t)))
        # Left semicircle (+pi/2 -> +3pi/2)
        for i in range(segments_per_arc + 1):
            t = math.pi / 2 + math.pi * i / segments_per_arc
            pts.append((-half_straight + r * math.cos(t), r * math.sin(t)))
    else:
        r = size_x / 2.0
        half_straight = max(0.0, (size_y - size_x) / 2.0)
        # Top semicircle (0 -> +pi)
        for i in range(segments_per_arc + 1):
            t = math.pi * i / segments_per_arc
            pts.append((r * math.cos(t), half_straight + r * math.sin(t)))
        # Bottom semicircle (+pi -> +2pi)
        for i in range(segments_per_arc + 1):
            t = math.pi + math.pi * i / segments_per_arc
            pts.append((r * math.cos(t), -half_straight + r * math.sin(t)))
    return pts


def _roundrect_local_corners(
    size_x: int,
    size_y: int,
    corner_radius: int,
    *,
    error_nm: int = 5_000,
) -> list[tuple[float, float]]:
    """
    Rounded-rectangle outline approximated like KiCad's CornerListToPolygon.

    KiCad chooses the arc segment count from a maximum chord error rather
    than a fixed per-corner segment count. Matching that keeps path-coordinate
    comparisons stable against kicad-cli SVG output.
    """
    hx = size_x / 2.0
    hy = size_y / 2.0
    r = max(0.0, min(float(corner_radius), min(hx, hy)))
    if r <= 0.0:
        return _rect_local_corners(size_x, size_y)

    radius = max(1_000.0, r)
    error = max(1_000.0, float(error_nm))
    rel_error = min(error / radius, 1.0)
    arc_increment = 180.0 / math.pi * math.acos(1.0 - rel_error) * 2.0
    arc_increment = min(360.0 / 8.0, arc_increment)
    full_circle_segments = max(round(360.0 / arc_increment), 2)
    num_segs = max(16, full_circle_segments)
    ang_delta = 360.0 / num_segs

    last_seg = 90.0
    while last_seg > ang_delta:
        last_seg -= ang_delta
    ang_pos_start = ang_delta if abs(last_seg) < 0.001 else (ang_delta + last_seg) / 2.0

    corner_centers = [
        (-hx + r, -hy + r),
        (hx - r, -hy + r),
        (hx - r, hy - r),
        (-hx + r, hy - r),
    ]
    arc_start_angles = [180.0, 270.0, 0.0, 90.0]

    pts: list[tuple[float, float]] = []
    for (qcx, qcy), start_angle in zip(corner_centers, arc_start_angles):
        t = math.radians(start_angle)
        pts.append((qcx + r * math.cos(t), qcy + r * math.sin(t)))

        ang_pos = ang_pos_start
        while ang_pos < 90.0 - 0.001:
            t = math.radians(start_angle + ang_pos)
            pts.append((qcx + r * math.cos(t), qcy + r * math.sin(t)))
            ang_pos += ang_delta

        t = math.radians(start_angle + 90.0)
        pts.append((qcx + r * math.cos(t), qcy + r * math.sin(t)))
    return pts


def _regular_polygon_local(
    diameter: int, corner_count: int
) -> list[tuple[float, float]]:
    """Vertices of a regular polygon centred at origin, on a circle of d/2."""
    r = diameter / 2.0
    n = max(3, int(corner_count))
    return [
        (r * math.cos(2 * math.pi * i / n), r * math.sin(2 * math.pi * i / n))
        for i in range(n)
    ]


def _primitive_width_for_svg(p: dict) -> int | None:
    """
    Interpret PLOTTER primitive width for SVG output.

    KiCad emits filled symbol primitives as a fill-only pass with
    ``width_nm=0`` and then a later no-fill outline pass. For unfilled
    primitives, zero still means "use the current/default plotter pen".
    """
    width_nm = int(p.get("width_nm", 0) or 0)
    fill = str(p.get("fill", KiCadFillType.NO_FILL.value) or "")
    if width_nm == 0 and fill != KiCadFillType.NO_FILL.value:
        return 0
    return width_nm or None


def _stroke_color(p: dict) -> str | None:
    return p.get("stroke_color") or p.get("color")


def _fill_color(p: dict) -> str | None:
    return p.get("fill_color")


def _line_style(p: dict) -> str | None:
    return p.get("line_style")


_DRILL_ROLES = {"pad_drill", "via_drill", "npth_hole", "via_mask_drill"}


def _is_copper_layer_name(layer: str) -> bool:
    return layer.endswith(".Cu")


def _is_mask_layer_name(layer: str) -> bool:
    return layer.endswith(".Mask")


def _visible_layers_have_non_mask(visible_layers: tuple[str, ...]) -> bool:
    return any(not _is_mask_layer_name(layer) for layer in visible_layers)


def _pad_op_layer_visibility(p: dict, ctx: KiCadSvgRenderContext) -> tuple[bool, bool]:
    """Return ``(mask_visible, non_mask_pad_visible)`` for a pad op."""
    visible_layers = _visible_pcb_layers(ctx)
    if visible_layers is None:
        return (False, True)

    raw_layers = p.get("layers")
    if not isinstance(raw_layers, IterableABC) or isinstance(raw_layers, (str, bytes)):
        return (False, True)

    mask_visible = False
    non_mask_visible = False
    for raw_layer in raw_layers:
        layer = str(raw_layer)
        if not _layers_visible([layer], visible_layers):
            continue
        if _is_mask_layer_name(layer) or layer == "*.Mask":
            mask_visible = True
        else:
            non_mask_visible = True
    return mask_visible, non_mask_visible


def _mask_margin_nm(p: dict) -> int:
    return int(p.get("mask_margin_nm", 0) or 0)


def _with_mask_expanded_sizes(p: dict) -> dict:
    margin = _mask_margin_nm(p)
    if margin == 0:
        return p
    out = dict(p)
    for key in ("diameter_nm", "size_x_nm", "size_y_nm"):
        if key in out:
            out[key] = int(out.get(key, 0) or 0) + 2 * margin
    if "corner_radius_nm" in out:
        out["corner_radius_nm"] = int(out.get("corner_radius_nm", 0) or 0) + margin
    return out


def _render_pad_with_mask_variant(
    p: dict,
    *,
    ctx: KiCadSvgRenderContext,
    render_nominal,
    render_expanded,
) -> str:
    mask_visible, non_mask_visible = _pad_op_layer_visibility(p, ctx)
    margin = _mask_margin_nm(p)
    if not mask_visible:
        return render_nominal(p)
    if margin == 0:
        return render_nominal(p)

    expanded = render_expanded(_with_mask_expanded_sizes(p))
    if non_mask_visible:
        nominal = render_nominal(p)
        if nominal:
            return "\n".join(fragment for fragment in (nominal, expanded) if fragment)
    return expanded


def _path_d_from_polygon_points(
    points: Iterable[tuple[int, int]] | Iterable[tuple[float, float]],
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    pts = list(points)
    if len(pts) < 3:
        return ""
    first_x, first_y = pts[0]
    commands = [
        f"M {fmt_user_number(ctx.to_user_x(first_x))},{fmt_user_number(ctx.to_user_y(first_y))}"
    ]
    commands.extend(
        f"L {fmt_user_number(ctx.to_user_x(x))},{fmt_user_number(ctx.to_user_y(y))}"
        for x, y in pts[1:]
    )
    commands.append("Z")
    return " ".join(commands)


def _filled_path_color(
    fill: KiCadFillType | str | None,
    fill_color: str | None,
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    if fill == KiCadFillType.FILLED_WITH_BG_BODYCOLOR.value:
        return ctx.sheet_area_color
    return ctx.resolve_color(
        fill_color or ctx.options.default_fill_color or ctx.current_color
    )


def _render_filled_polygon_like_cli(
    points: Iterable[tuple[int, int]] | Iterable[tuple[float, float]],
    *,
    ctx: KiCadSvgRenderContext,
    fill: KiCadFillType | str | None = KiCadFillType.FILLED_SHAPE.value,
    fill_color: str | None = None,
    stroke_color: str | None = None,
    width_nm: int | float | None = 0,
    line_style: KiCadLineStyle | str | None = None,
) -> str:
    pts = list(points)
    fill_value = fill.value if isinstance(fill, KiCadFillType) else str(fill or "")
    if _profile_is_oracle(ctx.options) and _is_filled(fill_value) and width_nm == 0:
        d = _path_d_from_polygon_points(pts, ctx=ctx)
        if not d:
            return ""
        color = _filled_path_color(fill_value, fill_color, ctx=ctx)
        return (
            f'<path d="{d}" fill="{color}" stroke="none" '
            f'fill-rule="evenodd" clip-rule="evenodd" />'
        )
    return svg_polygon(
        pts,
        ctx=ctx,
        fill=fill,
        fill_color=fill_color,
        stroke_color=stroke_color,
        width_nm=width_nm,
        line_style=line_style,
    )


def _path_d_from_polyline_points(
    points: Iterable[tuple[int, int]] | Iterable[tuple[float, float]],
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    pts = list(points)
    if len(pts) < 2:
        return ""
    first_x, first_y = pts[0]
    commands = [
        f"M {fmt_user_number(ctx.to_user_x(first_x))},{fmt_user_number(ctx.to_user_y(first_y))}"
    ]
    commands.extend(
        f"L {fmt_user_number(ctx.to_user_x(x))},{fmt_user_number(ctx.to_user_y(y))}"
        for x, y in pts[1:]
    )
    return " ".join(commands)


def _render_stroked_polyline_like_cli(
    points: Iterable[tuple[int, int]] | Iterable[tuple[float, float]],
    *,
    ctx: KiCadSvgRenderContext,
    stroke_color: str | None = None,
    width_nm: int | float | None = None,
    line_style: KiCadLineStyle | str | None = None,
) -> str:
    pts = list(points)
    if _profile_is_oracle(ctx.options):
        d = _path_d_from_polyline_points(pts, ctx=ctx)
        if not d:
            return ""
        return svg_path(
            d,
            ctx=ctx,
            fill=KiCadFillType.NO_FILL.value,
            stroke_color=stroke_color,
            width_nm=width_nm,
            line_style=line_style,
        )
    return svg_polyline(
        pts,
        ctx=ctx,
        stroke_color=stroke_color,
        width_nm=width_nm,
        line_style=line_style,
    )


def _render_rect_like_cli(
    x1: int,
    y1: int,
    x2: int,
    y2: int,
    *,
    ctx: KiCadSvgRenderContext,
    fill: KiCadFillType | str | None = KiCadFillType.NO_FILL.value,
    fill_color: str | None = None,
    stroke_color: str | None = None,
    width_nm: int | float | None = None,
    corner_radius_nm: int = 0,
    line_style: KiCadLineStyle | str | None = None,
) -> str:
    if _profile_is_oracle(ctx.options) and corner_radius_nm <= 0:
        d = _path_d_from_polygon_points(
            [(x1, y1), (x2, y1), (x2, y2), (x1, y2)],
            ctx=ctx,
        )
        return svg_path(
            d,
            ctx=ctx,
            fill=fill,
            fill_color=fill_color,
            stroke_color=stroke_color,
            width_nm=width_nm,
            line_style=line_style,
        )
    return svg_rect(
        x1,
        y1,
        x2,
        y2,
        ctx=ctx,
        fill=fill,
        fill_color=fill_color,
        stroke_color=stroke_color,
        width_nm=width_nm,
        corner_radius_nm=corner_radius_nm,
        line_style=line_style,
    )


def _drill_render_mode(role: str, ctx: KiCadSvgRenderContext) -> str:
    # NPTH holes are rendered identically to PTH drills by kicad-cli:
    # white knockout on copper/mask, outline on silk/fab/edge.
    visible_layers = _visible_pcb_layers(ctx)
    has_copper = visible_layers is None or any(
        _is_copper_layer_name(layer) for layer in visible_layers
    )
    has_mask_only = (
        visible_layers is not None
        and not has_copper
        and any(_is_mask_layer_name(layer) for layer in visible_layers)
    )
    if has_copper or has_mask_only:
        return "white"
    return "outline"


def _npth_mask_aperture_circle(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    if str(p.get("role", "")) != "npth_hole":
        return ""
    margin = _mask_margin_nm(p)
    if margin <= 0:
        return ""
    mask_visible, _ = _pad_op_layer_visibility(p, ctx)
    if not mask_visible:
        return ""
    diameter = int(p.get("diameter_nm", 0))
    pad_size_x = int(p.get("pad_size_x_nm", 0) or 0)
    pad_size_y = int(p.get("pad_size_y_nm", 0) or 0)
    pad_min = min(size for size in (pad_size_x, pad_size_y) if size > 0) if (
        pad_size_x > 0 or pad_size_y > 0
    ) else 0
    if diameter <= 0 or pad_min <= 0 or pad_min > diameter:
        return ""
    return svg_circle(
        int(p.get("cx", 0)),
        int(p.get("cy", 0)),
        (diameter + 2 * margin) // 2,
        ctx=ctx,
        fill=KiCadFillType.FILLED_SHAPE.value,
        fill_color="#000000",
        stroke_color="#000000",
        width_nm=0,
    )


def _render_drill_circle_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    role = str(p.get("role", ""))
    diameter = int(p.get("diameter_nm", 0))
    radius_nm = diameter // 2
    mode = _drill_render_mode(role, ctx)
    mask_aperture = _npth_mask_aperture_circle(p, ctx=ctx)
    if mode == "black":
        nominal = svg_circle(
            int(p.get("cx", 0)),
            int(p.get("cy", 0)),
            radius_nm,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            fill_color="#000000",
            stroke_color="#000000",
            width_nm=0,
        )
        return "\n".join(part for part in (mask_aperture, nominal) if part)
    if mode == "white":
        nominal = svg_circle(
            int(p.get("cx", 0)),
            int(p.get("cy", 0)),
            radius_nm,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            fill_color="#FFFFFF",
            stroke_color="#FFFFFF",
            width_nm=0,
        )
        return "\n".join(part for part in (mask_aperture, nominal) if part)
    nominal = svg_circle(
        int(p.get("cx", 0)),
        int(p.get("cy", 0)),
        radius_nm,
        ctx=ctx,
        fill=KiCadFillType.NO_FILL.value,
        stroke_color=_stroke_color(p),
        width_nm=100_000,
    )
    return "\n".join(part for part in (mask_aperture, nominal) if part)


def _render_drill_slot_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    role = str(p.get("role", ""))
    mode = _drill_render_mode(role, ctx)
    stroke_color = None
    if mode == "black":
        stroke_color = "#000000"
    elif mode == "white":
        stroke_color = "#FFFFFF"
    return _render_stroked_polyline_like_cli(
        [
            (int(p.get("start_x", 0)), int(p.get("start_y", 0))),
            (int(p.get("end_x", 0)), int(p.get("end_y", 0))),
        ],
        ctx=ctx,
        stroke_color=stroke_color,
        width_nm=int(p.get("width_nm", 0)) or None,
        line_style=_line_style(p),
    )


def _render_thick_segment_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    """Two-point polyline with a draw width — round caps come from ctx defaults."""
    width_nm = int(p.get("width_nm", 0)) or None
    return _render_stroked_polyline_like_cli(
        [
            (int(p.get("start_x", 0)), int(p.get("start_y", 0))),
            (int(p.get("end_x", 0)), int(p.get("end_y", 0))),
        ],
        ctx=ctx,
        stroke_color=_stroke_color(p),
        width_nm=width_nm,
        line_style=_line_style(p),
    )


def _render_thick_arc_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    """Convert center+angle form to 3-point and dispatch to svg_arc."""
    cx = float(p.get("cx", 0.0))
    cy = float(p.get("cy", 0.0))
    radius = float(p.get("radius_nm", 0.0))
    a0 = math.radians(float(p.get("start_angle_deg", 0.0)))
    sweep = math.radians(float(p.get("sweep_deg", 0.0)))
    width_nm = int(p.get("width_nm", 0)) or None
    am = a0 + sweep / 2.0
    ae = a0 + sweep
    return svg_arc(
        cx + radius * math.cos(a0),
        cy + radius * math.sin(a0),
        cx + radius * math.cos(am),
        cy + radius * math.sin(am),
        cx + radius * math.cos(ae),
        cy + radius * math.sin(ae),
        ctx=ctx,
        fill=KiCadFillType.NO_FILL.value,
        stroke_color=_stroke_color(p),
        width_nm=width_nm,
        line_style=_line_style(p),
    )


def _render_arc_center_angle_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    """Convert center+angle form to 3-point and dispatch to svg_arc."""
    cx = float(p.get("cx", 0.0))
    cy = float(p.get("cy", 0.0))
    radius = float(p.get("radius_nm", 0.0))
    a0 = math.radians(float(p.get("start_angle_deg", 0.0)))
    sweep = math.radians(float(p.get("sweep_deg", 0.0)))
    am = a0 + sweep / 2.0
    ae = a0 + sweep
    return svg_arc(
        cx + radius * math.cos(a0),
        cy + radius * math.sin(a0),
        cx + radius * math.cos(am),
        cy + radius * math.sin(am),
        cx + radius * math.cos(ae),
        cy + radius * math.sin(ae),
        ctx=ctx,
        fill=p.get("fill", KiCadFillType.NO_FILL.value),
        fill_color=_fill_color(p),
        stroke_color=_stroke_color(p),
        width_nm=_primitive_width_for_svg(p),
        line_style=_line_style(p),
    )


def _render_flash_pad_circle_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    def _render(payload: dict) -> str:
        diameter = int(payload.get("diameter_nm", 0))
        return svg_circle(
            int(payload.get("x", 0)),
            int(payload.get("y", 0)),
            diameter // 2,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            fill_color=_fill_color(payload),
            stroke_color=_stroke_color(payload),
            width_nm=0,
            line_style=_line_style(payload),
        )

    return _render_pad_with_mask_variant(
        p,
        ctx=ctx,
        render_nominal=_render,
        render_expanded=_render,
    )


def _render_flash_pad_rect_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    def _render_rect(payload: dict) -> str:
        cx = int(payload.get("x", 0))
        cy = int(payload.get("y", 0))
        pts = _absolutize(
            _rect_local_corners(
                int(payload.get("size_x_nm", 0)),
                int(payload.get("size_y_nm", 0)),
            ),
            cx=cx,
            cy=cy,
            orient_deg=float(payload.get("orient_deg", 0.0)),
        )
        return _render_filled_polygon_like_cli(
            pts,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            width_nm=0,
        )

    def _render_mask(payload: dict) -> str:
        margin = _mask_margin_nm(payload)
        if margin <= 0:
            return _render_rect(payload)
        cx = int(payload.get("x", 0))
        cy = int(payload.get("y", 0))
        pts = _absolutize(
            _roundrect_local_corners(
                int(payload.get("size_x_nm", 0)),
                int(payload.get("size_y_nm", 0)),
                margin,
            ),
            cx=cx,
            cy=cy,
            orient_deg=float(payload.get("orient_deg", 0.0)),
        )
        return _render_filled_polygon_like_cli(
            pts,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            width_nm=0,
        )

    return _render_pad_with_mask_variant(
        p,
        ctx=ctx,
        render_nominal=_render_rect,
        render_expanded=_render_mask,
    )


def _render_flash_pad_oval_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    """Render an oval pad as a thick segment with round caps.

    Mirrors kicad-cli's ``PCB_PLOTTER::PlotPad_Oval`` which emits an
    oval pad as a single ``ThickSegment`` call (centerline + width +
    ``stroke-linecap=round``) rather than a tessellated polygon. The
    long axis follows the larger of ``size_x`` / ``size_y``; if both
    are equal the pad degenerates to a circle of diameter ``size_x``.
    """
    def _render(payload: dict) -> str:
        cx = int(payload.get("x", 0))
        cy = int(payload.get("y", 0))
        size_x = int(payload.get("size_x_nm", 0))
        size_y = int(payload.get("size_y_nm", 0))
        orient_deg = float(payload.get("orient_deg", 0.0))

        if size_x == size_y:
            # Degenerate: oval collapses to a filled circle.
            return svg_circle(
                cx,
                cy,
                size_x // 2,
                ctx=ctx,
                fill=KiCadFillType.FILLED_SHAPE.value,
                fill_color=_fill_color(payload),
                stroke_color=_stroke_color(payload),
                width_nm=0,
                line_style=_line_style(payload),
            )

        if size_x >= size_y:
            half_straight = (size_x - size_y) / 2.0
            local = [(-half_straight, 0.0), (half_straight, 0.0)]
            width_nm = size_y
        else:
            half_straight = (size_y - size_x) / 2.0
            local = [(0.0, -half_straight), (0.0, half_straight)]
            width_nm = size_x

        pts = _absolutize(local, cx=cx, cy=cy, orient_deg=orient_deg)
        return _render_stroked_polyline_like_cli(
            pts,
            ctx=ctx,
            stroke_color=_stroke_color(payload),
            width_nm=width_nm,
            line_style=_line_style(payload),
        )

    return _render_pad_with_mask_variant(
        p,
        ctx=ctx,
        render_nominal=_render,
        render_expanded=_render,
    )


def _render_flash_pad_roundrect_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    def _render(payload: dict) -> str:
        cx = int(payload.get("x", 0))
        cy = int(payload.get("y", 0))
        pts = _absolutize(
            _roundrect_local_corners(
                int(payload.get("size_x_nm", 0)),
                int(payload.get("size_y_nm", 0)),
                int(payload.get("corner_radius_nm", 0)),
            ),
            cx=cx,
            cy=cy,
            orient_deg=float(payload.get("orient_deg", 0.0)),
        )
        return _render_filled_polygon_like_cli(
            pts,
            ctx=ctx,
            fill=KiCadFillType.FILLED_SHAPE.value,
            width_nm=0,
        )

    return _render_pad_with_mask_variant(
        p,
        ctx=ctx,
        render_nominal=_render,
        render_expanded=_render,
    )


def _render_flash_pad_trapez_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    cx = int(p.get("x", 0))
    cy = int(p.get("y", 0))
    raw_corners = p.get("corners", []) or []
    locals_ = [(float(c[0]), float(c[1])) for c in raw_corners]
    if not locals_:
        return ""
    pts = _absolutize(locals_, cx=cx, cy=cy, orient_deg=float(p.get("orient_deg", 0.0)))
    return _render_filled_polygon_like_cli(
        pts,
        ctx=ctx,
        fill=KiCadFillType.FILLED_SHAPE.value,
        width_nm=0,
    )


def _custom_pad_rings(
    payload: dict,
    *,
    expand_for_mask: bool = False,
) -> list[list[tuple[float, float]]]:
    polygons = payload.get("polygons", []) or []
    rings = [
        [(float(pt[0]), float(pt[1])) for pt in ring]
        for ring in polygons
        if ring
    ]
    margin = _mask_margin_nm(payload) if expand_for_mask else 0
    if margin <= 0:
        return rings

    try:
        from shapely.geometry import MultiPolygon, Polygon
    except Exception:
        return rings

    out: list[list[tuple[float, float]]] = []
    for ring in rings:
        if len(ring) < 3:
            continue
        try:
            geom = Polygon(ring)
            if geom.is_empty or not geom.is_valid:
                continue
            buffered = geom.buffer(float(margin), quad_segs=1, join_style="round")
        except Exception:
            continue
        out.extend(
            _polygon_like_exterior_rings(
                buffered,
                multi_polygon_type=MultiPolygon,
                polygon_type=Polygon,
            )
        )
    return out or rings


def _custom_pad_polygon_widths(payload: dict) -> list[float]:
    widths = payload.get("polygon_widths_nm", []) or []
    return [float(width or 0.0) for width in widths]


def _custom_pad_anchor_geometry(payload: dict):
    anchor_shape = str(payload.get("anchor_shape", "") or "").lower()
    if not anchor_shape:
        return None
    size_x = float(payload.get("size_x_nm", 0) or 0)
    size_y = float(payload.get("size_y_nm", 0) or 0)
    if size_x <= 0.0 or size_y <= 0.0:
        return None
    try:
        from shapely.affinity import scale
        from shapely.geometry import Point, box
    except Exception:
        return None
    if anchor_shape == "rect":
        return box(-size_x / 2.0, -size_y / 2.0, size_x / 2.0, size_y / 2.0)
    if anchor_shape == "circle":
        return scale(
            Point(0.0, 0.0).buffer(0.5, quad_segs=16),
            xfact=size_x,
            yfact=size_y,
            origin=(0.0, 0.0),
        )
    return None


def _polygon_like_exterior_rings(
    geom,
    *,
    multi_polygon_type,
    polygon_type,
) -> list[list[tuple[float, float]]]:
    if isinstance(geom, multi_polygon_type):
        geoms_iter = geom.geoms
    elif isinstance(geom, polygon_type):
        geoms_iter = [geom]
    else:
        geoms_iter = []

    out: list[list[tuple[float, float]]] = []
    for item in geoms_iter:
        if item.is_empty:
            continue
        coords = list(item.exterior.coords)
        if len(coords) > 1 and coords[0] == coords[-1]:
            coords = coords[:-1]
        if len(coords) >= 3:
            out.append([(float(x), float(y)) for x, y in coords])
    return out


def _custom_pad_cli_rings(
    payload: dict,
    *,
    expand_for_mask: bool = False,
) -> list[list[tuple[float, float]]]:
    rings = _custom_pad_rings(payload, expand_for_mask=False)
    try:
        from shapely.geometry import MultiPolygon, Polygon
        from shapely.ops import unary_union
    except Exception:
        return _custom_pad_rings(payload, expand_for_mask=expand_for_mask)

    geoms = []
    widths = _custom_pad_polygon_widths(payload)
    for index, ring in enumerate(rings):
        if len(ring) < 3:
            continue
        try:
            geom = Polygon(ring)
            if geom.is_empty or not geom.is_valid:
                continue
            width = widths[index] if index < len(widths) else 0.0
            if width > 0.0:
                geom = geom.buffer(width / 2.0, quad_segs=1, join_style="round")
            geoms.append(geom)
        except Exception:
            continue

    anchor_geom = _custom_pad_anchor_geometry(payload)
    if anchor_geom is not None and not anchor_geom.is_empty:
        geoms.append(anchor_geom)
    if not geoms:
        return rings

    try:
        geom = unary_union(geoms)
        margin = _mask_margin_nm(payload) if expand_for_mask else 0
        if margin > 0:
            geom = geom.buffer(float(margin), quad_segs=1, join_style="round")
    except Exception:
        return _custom_pad_rings(payload, expand_for_mask=expand_for_mask)

    return _polygon_like_exterior_rings(
        geom,
        multi_polygon_type=MultiPolygon,
        polygon_type=Polygon,
    ) or rings


def _render_custom_pad_payload(
    payload: dict,
    *,
    ctx: KiCadSvgRenderContext,
    expand_for_mask: bool = False,
) -> str:
    cx = int(payload.get("x", 0))
    cy = int(payload.get("y", 0))
    orient_deg = float(payload.get("orient_deg", 0.0))
    polygon_source = (
        _custom_pad_cli_rings if _profile_is_oracle(ctx.options) else _custom_pad_rings
    )
    fragments: list[str] = []
    for locals_ in polygon_source(payload, expand_for_mask=expand_for_mask):
        fragments.append(
            _render_filled_polygon_like_cli(
                _absolutize(locals_, cx=cx, cy=cy, orient_deg=orient_deg),
                ctx=ctx,
                fill=KiCadFillType.FILLED_SHAPE.value,
                width_nm=0,
            )
        )
    return "\n".join(fragments)


def _render_flash_pad_custom_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    return _render_pad_with_mask_variant(
        p,
        ctx=ctx,
        render_nominal=lambda payload: _render_custom_pad_payload(
            payload,
            ctx=ctx,
            expand_for_mask=False,
        ),
        render_expanded=lambda payload: _render_custom_pad_payload(
            payload,
            ctx=ctx,
            expand_for_mask=True,
        ),
    )


def _render_flash_reg_polygon_op(p: dict, *, ctx: KiCadSvgRenderContext) -> str:
    cx = int(p.get("x", 0))
    cy = int(p.get("y", 0))
    pts = _absolutize(
        _regular_polygon_local(
            int(p.get("diameter_nm", 0)), int(p.get("corner_count", 0))
        ),
        cx=cx,
        cy=cy,
        orient_deg=float(p.get("orient_deg", 0.0)),
    )
    return _render_filled_polygon_like_cli(
        pts,
        ctx=ctx,
        fill=KiCadFillType.FILLED_SHAPE.value,
        width_nm=0,
    )


def render_op(op: KiCadPlotterOp, *, ctx: KiCadSvgRenderContext) -> str:
    """
    Translate one :class:`KiCadPlotterOp` into an SVG fragment.

    Returns an empty string for unsupported / state-only ops so the
    caller can simply concatenate without filtering.
    """
    kind = op.kind.value if isinstance(op.kind, KiCadPlotterOpKind) else str(op.kind)
    p = op.payload

    if kind == KiCadPlotterOpKind.CIRCLE.value:
        if str(p.get("role", "")) in _DRILL_ROLES:
            return _render_drill_circle_op(p, ctx=ctx)
        diameter = int(p.get("diameter_nm", 0))
        # Integer division mirrors KiCad's own integer-radius math.
        radius_nm = diameter // 2
        return svg_circle(
            int(p.get("cx", 0)),
            int(p.get("cy", 0)),
            radius_nm,
            ctx=ctx,
            fill=p.get("fill", KiCadFillType.NO_FILL.value),
            fill_color=_fill_color(p),
            stroke_color=_stroke_color(p),
            width_nm=_primitive_width_for_svg(p),
            line_style=_line_style(p),
        )

    if kind == KiCadPlotterOpKind.RECT.value:
        return _render_rect_like_cli(
            int(p.get("x1", 0)),
            int(p.get("y1", 0)),
            int(p.get("x2", 0)),
            int(p.get("y2", 0)),
            ctx=ctx,
            fill=p.get("fill", KiCadFillType.NO_FILL.value),
            fill_color=_fill_color(p),
            stroke_color=_stroke_color(p),
            width_nm=_primitive_width_for_svg(p),
            corner_radius_nm=int(p.get("corner_radius_nm", 0)),
            line_style=_line_style(p),
        )

    if kind == KiCadPlotterOpKind.ARC_THREE_POINT.value:
        return svg_arc(
            float(p.get("start_x", 0.0)),
            float(p.get("start_y", 0.0)),
            float(p.get("mid_x", 0.0)),
            float(p.get("mid_y", 0.0)),
            float(p.get("end_x", 0.0)),
            float(p.get("end_y", 0.0)),
            ctx=ctx,
            fill=p.get("fill", KiCadFillType.NO_FILL.value),
            fill_color=_fill_color(p),
            stroke_color=_stroke_color(p),
            width_nm=_primitive_width_for_svg(p),
            line_style=_line_style(p),
        )

    if kind == KiCadPlotterOpKind.ARC_CENTER_ANGLE.value:
        return _render_arc_center_angle_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.BEZIER_CURVE.value:
        return svg_bezier(
            int(p.get("start_x", 0)),
            int(p.get("start_y", 0)),
            int(p.get("ctrl1_x", 0)),
            int(p.get("ctrl1_y", 0)),
            int(p.get("ctrl2_x", 0)),
            int(p.get("ctrl2_y", 0)),
            int(p.get("end_x", 0)),
            int(p.get("end_y", 0)),
            ctx=ctx,
            width_nm=int(p.get("width_nm", 0)) or None,
            stroke_color=_stroke_color(p),
            line_style=_line_style(p),
        )

    if kind == KiCadPlotterOpKind.PLOT_POLY.value:
        points = p.get("points", []) or []
        # Normalise to a list of (x,y) tuples; tolerate dict or
        # tuple-of-tuples shapes from the JSON deserializer.
        normalised = [(int(pt[0]), int(pt[1])) for pt in points]
        fill = str(p.get("fill", KiCadFillType.NO_FILL.value))
        width_nm = _primitive_width_for_svg(p)
        if _is_filled(fill):
            return _render_filled_polygon_like_cli(
                normalised,
                ctx=ctx,
                fill=fill,
                fill_color=_fill_color(p),
                stroke_color=_stroke_color(p),
                width_nm=width_nm,
                line_style=_line_style(p),
            )
        return _render_stroked_polyline_like_cli(
            normalised,
            ctx=ctx,
            stroke_color=_stroke_color(p),
            width_nm=width_nm,
            line_style=_line_style(p),
        )

    if kind == KiCadPlotterOpKind.TEXT.value:
        return _render_text_op(op, ctx=ctx)

    if kind == KiCadPlotterOpKind.PLOT_IMAGE.value:
        return _render_plot_image_op(p, ctx=ctx)

    # ---- PCB ops (footprint / pcb pipelines) ----

    if kind == KiCadPlotterOpKind.THICK_SEGMENT.value:
        if str(p.get("role", "")) in _DRILL_ROLES:
            return _render_drill_slot_op(p, ctx=ctx)
        return _render_thick_segment_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.THICK_ARC.value:
        return _render_thick_arc_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_PAD_CIRCLE.value:
        return _render_flash_pad_circle_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_PAD_OVAL.value:
        return _render_flash_pad_oval_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_PAD_RECT.value:
        return _render_flash_pad_rect_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_PAD_ROUNDRECT.value:
        return _render_flash_pad_roundrect_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_PAD_TRAPEZ.value:
        return _render_flash_pad_trapez_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_PAD_CUSTOM.value:
        return _render_flash_pad_custom_op(p, ctx=ctx)

    if kind == KiCadPlotterOpKind.FLASH_REG_POLYGON.value:
        return _render_flash_reg_polygon_op(p, ctx=ctx)

    # State / lifecycle ops are intentionally skipped here —
    # they are consumed by the renderer's context plumbing or land
    # in a later phase (SetPageSettings, etc.).
    return ""


# =============================================================================
# Record + document rendering
# =============================================================================


def _variant_overlay_attrs(
    record: KiCadPlotterRecord,
    *,
    options: KiCadSvgRenderOptions,
) -> str | None:
    """
    Translate the ``extras["variant_state"]`` annotation into group
    attributes. Returns ``None`` when no overlay applies (record
    active, mode NONE, or extras missing).
    """
    state = (record.extras or {}).get(VARIANT_STATE_KEY)
    if state != VARIANT_STATE_DIMMED:
        return None
    mode = options.variant_dim_mode
    if mode == KiCadVariantDimMode.DIM_OVERLAY:
        opacity = max(0.0, min(1.0, float(options.variant_dim_opacity)))
        return (
            f'opacity="{opacity:.3f}" '
            f'data-variant-state="{VARIANT_STATE_DIMMED}"'
        )
    if mode == KiCadVariantDimMode.GREYSCALE:
        return (
            f'style="filter:grayscale(100%);" '
            f'data-variant-state="{VARIANT_STATE_DIMMED}"'
        )
    return None


def _join_svg_attr_strings(*parts: str | None) -> str | None:
    return " ".join(part for part in parts if part) or None


def _record_placement_transform(
    record: KiCadPlotterRecord,
    *,
    ctx: KiCadSvgRenderContext,
) -> str | None:
    """
    Return an SVG transform for PCB-embedded footprint records.

    ``pcb_to_ir`` intentionally stores footprint geometry in footprint-local
    coordinates and carries the PCB placement under ``extras["placement"]``.
    Applying this as an SVG group transform keeps custom pads/trapezoids local
    and avoids double-transforming flash-pad primitive payloads.
    """
    placement = (record.extras or {}).get("placement")
    if not isinstance(placement, dict):
        return None

    x_nm = int(placement.get("x_nm", 0) or 0)
    y_nm = int(placement.get("y_nm", 0) or 0)
    angle_deg = float(placement.get("angle_deg", 0.0) or 0.0)

    parts: list[str] = []
    if x_nm or y_nm:
        parts.append(
            "translate("
            f"{fmt_user_number(ctx.to_user_x(x_nm))} "
            f"{fmt_user_number(ctx.to_user_y(y_nm))}"
            ")"
        )
    if angle_deg:
        parts.append(f"rotate({fmt_user_number(-angle_deg)})")
    return " ".join(parts) or None


def _record_operation_context(
    record: KiCadPlotterRecord,
    *,
    ctx: KiCadSvgRenderContext,
) -> KiCadSvgRenderContext:
    """
    Return the context used to render a record's child operations.

    PCB-embedded footprint records store geometry in footprint-local
    coordinates and carry board placement in ``extras["placement"]``. The
    placement group applies the board-space offset, so child ops must not also
    receive the document bbox offset.
    """
    placement = (record.extras or {}).get("placement")
    if not isinstance(placement, dict):
        return ctx

    from copy import copy as _copy

    local_ctx = _copy(ctx)
    local_ctx.offset_x_nm = 0
    local_ctx.offset_y_nm = 0
    return local_ctx


def _svg_attr(name: str, value: object) -> str:
    return f'{name}="{html.escape(str(value), quote=True)}"'


def _svg_profile(options: KiCadSvgRenderOptions) -> str:
    profile = getattr(options, "profile", KiCadSvgRenderProfile.ENRICHED)
    if isinstance(profile, KiCadSvgRenderProfile):
        return profile.value
    return str(profile)


def _profile_is_oracle(options: KiCadSvgRenderOptions) -> bool:
    return _svg_profile(options) == KiCadSvgRenderProfile.ORACLE.value


def _profile_is_enriched(options: KiCadSvgRenderOptions) -> bool:
    return _svg_profile(options) == KiCadSvgRenderProfile.ENRICHED.value


def _emit_metadata(options: KiCadSvgRenderOptions) -> bool:
    if _profile_is_oracle(options):
        return False
    return _profile_is_enriched(options) or bool(getattr(options, "include_metadata", False))


def _emit_ids(options: KiCadSvgRenderOptions) -> bool:
    if _profile_is_oracle(options):
        return False
    return _profile_is_enriched(options) or bool(getattr(options, "include_ids", False))


def _block_extra_attrs(payload: dict) -> str | None:
    attrs: list[str] = []
    object_id = payload.get("object_id")
    if object_id:
        attrs.append(_svg_attr("data-object-id", object_id))
    extra = payload.get("extra_attrs") or {}
    if isinstance(extra, dict):
        for key, value in extra.items():
            if value is None or str(value) == "":
                continue
            attr = str(key).strip().replace("_", "-")
            if not attr:
                continue
            if not attr.startswith("data-"):
                attr = f"data-{attr}"
            if not all(ch.isalnum() or ch in "-_:" for ch in attr):
                continue
            attrs.append(_svg_attr(attr, value))
    return " ".join(attrs) or None


def _render_block_group(
    payload: dict,
    body: str,
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    label = str(payload.get("label", "") or "")
    data_uuid = str(payload.get("data_uuid", "") or "")
    data_ref = str(payload.get("data_ref", "") or "")
    emit_metadata = _emit_metadata(ctx.options)
    emit_ids = _emit_ids(ctx.options)
    extra_attrs = _block_extra_attrs(payload) if emit_metadata else None
    if not emit_ids and not emit_metadata and not extra_attrs:
        return body
    return svg_group(
        body,
        label=(label or None) if emit_ids else None,
        data_uuid=(data_uuid or None) if emit_metadata else None,
        data_ref=(data_ref or None) if emit_metadata else None,
        extra_attrs=extra_attrs,
    )


def _render_op_metadata_group(
    op: KiCadPlotterOp,
    body: str,
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    payload = op.payload or {}
    emit_metadata = _emit_metadata(ctx.options)
    emit_ids = _emit_ids(ctx.options)
    extra_attrs = _block_extra_attrs(payload) if emit_metadata else None
    label = str(payload.get("label", "") or "")
    data_uuid = str(payload.get("data_uuid", "") or "")
    data_ref = str(payload.get("data_ref", "") or "")
    if not emit_ids and not emit_metadata and not extra_attrs:
        return body
    if not any((label, data_uuid, data_ref, extra_attrs)):
        return body
    return svg_group(
        body,
        label=(label or None) if emit_ids else None,
        data_uuid=(data_uuid or None) if emit_metadata else None,
        data_ref=(data_ref or None) if emit_metadata else None,
        extra_attrs=extra_attrs,
    )


# Style-bucket grouping: each rendered op fragment is
# wrapped in a ``<g style="...">`` whose CSS mirrors the element-local
# attributes. This mirrors ``kicad-cli pcb export svg`` output structure
# (one CLI ``<g>`` per fill/stroke combination) so style-keyed metrics
# (white drills, white strokes, stroke-width buckets) match without
# disturbing the per-record ``<g data-ref data-uuid>`` wrapper that
# existing per-element identity tests rely on.
_FIRST_ELEMENT_ATTRS_RE = re.compile(
    r'<(?:circle|rect|polyline|polygon|path|line|ellipse)\b([^>]*)',
    re.IGNORECASE,
)
_FILL_ATTR_RE = re.compile(r'\bfill="([^"]*)"')
_STROKE_ATTR_RE = re.compile(r'\bstroke="([^"]*)"')
_STROKE_WIDTH_ATTR_RE = re.compile(r'\bstroke-width="([^"]*)"')


def _wrap_with_style_bucket(fragment: str) -> str:
    """Wrap ``fragment`` in a ``<g style="...">`` mirroring its first
    element's fill / stroke / stroke-width.

    A zero stroke-width is canonicalised to ``stroke:none`` (and the
    stroke-width drops out) to match the CLI convention used by the
    ``_count_white_drill_circles`` oracle helper. Fragments that don't
    contain a recognised primitive are returned unchanged.
    """
    if not fragment or not fragment.strip():
        return fragment
    match = _FIRST_ELEMENT_ATTRS_RE.search(fragment)
    if not match:
        return fragment
    attrs = match.group(1)
    fill_match = _FILL_ATTR_RE.search(attrs)
    stroke_match = _STROKE_ATTR_RE.search(attrs)
    width_match = _STROKE_WIDTH_ATTR_RE.search(attrs)

    parts: list[str] = []
    if fill_match is not None:
        parts.append(f"fill:{fill_match.group(1)}")

    width_value = width_match.group(1) if width_match else None
    width_is_zero = False
    if width_value is not None:
        try:
            width_is_zero = float(width_value) == 0.0
        except ValueError:
            width_is_zero = False

    if width_is_zero:
        parts.append("stroke:none")
    else:
        if stroke_match is not None:
            parts.append(f"stroke:{stroke_match.group(1)}")
        if width_value is not None:
            parts.append(f"stroke-width:{width_value}")

    if not parts:
        return fragment
    parts.append("stroke-linecap:round")
    parts.append("stroke-linejoin:round")
    style = "; ".join(parts)
    return f'<g style="{style}">{fragment}</g>'


def _render_ops_with_blocks(
    operations: Iterable[KiCadPlotterOp],
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    root: list[str] = []
    stack: list[tuple[dict, list[str]]] = []

    def _append(fragment: str) -> None:
        if not fragment:
            return
        if stack:
            stack[-1][1].append(fragment)
        else:
            root.append(fragment)

    for op in operations:
        kind = (
            op.kind.value
            if isinstance(op.kind, KiCadPlotterOpKind)
            else str(op.kind)
        )
        if kind == KiCadPlotterOpKind.START_BLOCK.value:
            stack.append((dict(op.payload), []))
            continue
        if kind == KiCadPlotterOpKind.END_BLOCK.value:
            if not stack:
                continue
            payload, fragments = stack.pop()
            _append(_render_block_group(payload, "\n".join(fragments), ctx=ctx))
            continue
        fragment = _wrap_with_style_bucket(render_op(op, ctx=ctx))
        _append(_render_op_metadata_group(op, fragment, ctx=ctx))

    # Close malformed unbalanced blocks conservatively so a partial
    # recorder stream still produces valid SVG.
    while stack:
        payload, fragments = stack.pop()
        group = _render_block_group(payload, "\n".join(fragments), ctx=ctx)
        if stack:
            stack[-1][1].append(group)
        else:
            root.append(group)

    return "\n".join(root)


def _visible_pcb_layers(ctx: KiCadSvgRenderContext) -> tuple[str, ...] | None:
    raw_layers = getattr(ctx.options, "visible_layers", None)
    if raw_layers is None:
        return None
    return tuple(str(layer) for layer in raw_layers)


def _copper_layer_index(layer: str) -> int | None:
    if layer == "F.Cu":
        return 0
    if layer == "B.Cu":
        return 10_000
    if layer.startswith("In") and layer.endswith(".Cu"):
        inner = layer[2:-3]
        if inner.isdigit():
            return int(inner)
    return None


_PCB_LAYER_ALIASES: dict[str, tuple[str, ...]] = {
    "Cmts.User": ("User.Comments",),
    "User.Comments": ("Cmts.User",),
    "Dwgs.User": ("User.Drawings",),
    "User.Drawings": ("Dwgs.User",),
    "Eco1.User": ("User.Eco1",),
    "User.Eco1": ("Eco1.User",),
    "Eco2.User": ("User.Eco2",),
    "User.Eco2": ("Eco2.User",),
}


def _layer_name_visible(layer: str, visible_layers: tuple[str, ...]) -> bool:
    for visible_layer in visible_layers:
        if layer == visible_layer:
            return True
        if visible_layer in _PCB_LAYER_ALIASES.get(layer, ()):
            return True
        if layer == "*.Cu" and visible_layer.endswith(".Cu"):
            return True
        if layer == "*.Mask" and visible_layer.endswith(".Mask"):
            return True
        if layer == "*.Paste" and visible_layer.endswith(".Paste"):
            return True
        if layer == "F&B.Cu" and visible_layer in ("F.Cu", "B.Cu"):
            return True
    return False


def _layers_visible(
    layers: Iterable[object],
    visible_layers: tuple[str, ...],
    *,
    allow_copper_span: bool = False,
) -> bool:
    layer_names = [str(layer) for layer in layers if layer]
    if any(_layer_name_visible(layer, visible_layers) for layer in layer_names):
        return True

    if not allow_copper_span:
        return False

    copper_indices = [
        index for layer in layer_names
        if (index := _copper_layer_index(layer)) is not None
    ]
    if len(copper_indices) < 2:
        return False
    low = min(copper_indices)
    high = max(copper_indices)
    return any(
        (visible_index := _copper_layer_index(layer)) is not None
        and low <= visible_index <= high
        for layer in visible_layers
    )


def _op_visible_on_layers(
    op: KiCadPlotterOp,
    visible_layers: tuple[str, ...],
) -> bool:
    role = str(op.payload.get("role", ""))
    if role == "npth_hole":
        return bool(visible_layers)

    layers = op.payload.get("layers")
    if role == "pad_drill":
        if _visible_layers_have_non_mask(visible_layers):
            return True
        if isinstance(layers, IterableABC) and not isinstance(layers, (str, bytes)):
            return _layers_visible(layers, visible_layers)
        return bool(visible_layers)

    if role in {"via_aperture", "via_drill"}:
        if isinstance(layers, IterableABC) and not isinstance(layers, (str, bytes)):
            return _layers_visible(layers, visible_layers, allow_copper_span=True)
        return False

    layer = op.payload.get("layer")
    if layer:
        return _layer_name_visible(str(layer), visible_layers)
    if isinstance(layers, IterableABC) and not isinstance(layers, (str, bytes)):
        return _layers_visible(layers, visible_layers)
    return True


def _record_ops_for_visible_layers(
    record: KiCadPlotterRecord,
    visible_layers: tuple[str, ...],
) -> list[KiCadPlotterOp] | None:
    extras = record.extras or {}
    fill_layers = extras.get("fill_layers")
    if isinstance(fill_layers, list):
        ops: list[KiCadPlotterOp] = []
        for index, op in enumerate(record.operations):
            if index < len(fill_layers):
                if _layer_name_visible(str(fill_layers[index]), visible_layers):
                    ops.append(op)
            elif _op_visible_on_layers(op, visible_layers):
                ops.append(op)
        return ops or None

    if record.kind == "footprint":
        ops = [
            op for op in record.operations
            if _op_visible_on_layers(op, visible_layers)
        ]
        return ops or None

    layer = extras.get("layer")
    if layer and not _layer_name_visible(str(layer), visible_layers):
        return None

    layers = extras.get("layers")
    if isinstance(layers, IterableABC) and not isinstance(layers, (str, bytes)):
        if not _layers_visible(
            layers,
            visible_layers,
            allow_copper_span=(record.kind == "via"),
        ):
            ops = [
                op for op in record.operations
                if _op_visible_on_layers(op, visible_layers)
            ]
            return ops or None

    ops = [
        op for op in record.operations
        if _op_visible_on_layers(op, visible_layers)
    ]
    if record.operations and not ops:
        return None
    return ops


def _record_visible_operations(
    record: KiCadPlotterRecord,
    *,
    ctx: KiCadSvgRenderContext,
) -> list[KiCadPlotterOp] | None:
    operations: Iterable[KiCadPlotterOp] = record.operations
    visible_layers = _visible_pcb_layers(ctx)
    if visible_layers is not None:
        visible_ops = _record_ops_for_visible_layers(record, visible_layers)
        if visible_ops is None:
            return None
        operations = visible_ops
    return list(operations)


def _is_drill_overlay_op(op: KiCadPlotterOp) -> bool:
    return str((op.payload or {}).get("role", "")) in _DRILL_ROLES


def _op_kind_value(op: KiCadPlotterOp) -> str:
    return op.kind.value if isinstance(op.kind, KiCadPlotterOpKind) else str(op.kind)


def _is_block_start_op(op: KiCadPlotterOp) -> bool:
    return _op_kind_value(op) == KiCadPlotterOpKind.START_BLOCK.value


def _is_block_end_op(op: KiCadPlotterOp) -> bool:
    return _op_kind_value(op) == KiCadPlotterOpKind.END_BLOCK.value


def _split_drill_overlay_ops(
    operations: Iterable[KiCadPlotterOp],
) -> tuple[list[KiCadPlotterOp], list[KiCadPlotterOp]]:
    normal: list[KiCadPlotterOp] = []
    drill_overlay: list[KiCadPlotterOp] = []
    ops = list(operations)
    index = 0
    while index < len(ops):
        op = ops[index]
        if _is_block_start_op(op):
            block = [op]
            depth = 1
            cursor = index + 1
            while cursor < len(ops):
                child = ops[cursor]
                block.append(child)
                if _is_block_start_op(child):
                    depth += 1
                elif _is_block_end_op(child):
                    depth -= 1
                    if depth == 0:
                        break
                cursor += 1

            if depth == 0:
                draw_ops = [
                    child for child in block
                    if not (_is_block_start_op(child) or _is_block_end_op(child))
                ]
                has_drill = any(_is_drill_overlay_op(child) for child in draw_ops)
                has_non_drill = any(
                    not _is_drill_overlay_op(child) for child in draw_ops
                )
                if has_drill and not has_non_drill:
                    drill_overlay.extend(block)
                else:
                    normal.extend(block)
                index = cursor + 1
                continue

        if _is_drill_overlay_op(op):
            drill_overlay.append(op)
        else:
            normal.append(op)
        index += 1
    return normal, drill_overlay


def _render_record_operations(
    record: KiCadPlotterRecord,
    operations: Iterable[KiCadPlotterOp],
    *,
    ctx: KiCadSvgRenderContext,
    include_group: bool = True,
    label_suffix: str = "",
    data_ref: str | None = None,
) -> str:
    op_ctx = _record_operation_context(record, ctx=ctx)
    operations = list(operations)
    body = _render_ops_with_blocks(operations, ctx=op_ctx)
    if not include_group:
        return body
    transform = _record_placement_transform(record, ctx=ctx)
    label = f"{record.uuid}{label_suffix}" if record.uuid else None
    emit_metadata = _emit_metadata(ctx.options)
    emit_ids = _emit_ids(ctx.options)
    record_attrs = None
    if emit_metadata:
        if pcb_record_has_svg_data_attrs(record):
            record_attrs = svg_attrs_to_string(
                pcb_record_svg_data_attrs(record, operations, data_ref=data_ref)
            )
        elif schematic_record_has_svg_data_attrs(record):
            record_attrs = svg_attrs_to_string(
                schematic_record_svg_data_attrs(record, operations)
            )
    extra_attrs = _join_svg_attr_strings(
        _variant_overlay_attrs(record, options=ctx.options),
        record_attrs,
    )
    data_uuid = label if emit_metadata else None
    data_ref_value = (data_ref if data_ref is not None else record.kind) if emit_metadata else None
    label_value = label if emit_ids else None
    if not any((label_value, transform, data_uuid, data_ref_value, extra_attrs)):
        return body
    return svg_group(
        body,
        label=label_value,
        transform=transform,
        data_uuid=data_uuid,
        data_ref=data_ref_value,
        extra_attrs=extra_attrs,
    )


def render_record(
    record: KiCadPlotterRecord,
    *,
    ctx: KiCadSvgRenderContext,
    include_group: bool = True,
) -> str:
    """
    Render every op in ``record`` and (by default) wrap the result in
    a ``<g>`` keyed by the record's UUID with ``data-ref`` carrying
    the record kind. Empty-op records still emit a placeholder group
    so downstream tooling can hook on identity.

    Records carrying ``extras["variant_state"] == "dimmed"`` get an opacity
    or filter overlay on the wrapper ``<g>`` per ``ctx.options.variant_dim_*``.
    """
    operations = _record_visible_operations(record, ctx=ctx)
    if operations is None:
        return ""
    return _render_record_operations(
        record,
        operations,
        ctx=ctx,
        include_group=include_group,
    )


def _resolve_canvas_dims(doc: KiCadPlotterDocument) -> tuple[int, int]:
    """Pull (width_nm, height_nm) from doc.canvas or fall back to A4."""
    canvas = doc.canvas or {}
    width = int(canvas.get("width_nm", 0) or 0)
    height = int(canvas.get("height_nm", 0) or 0)
    if width <= 0 or height <= 0:
        # Default to A4 landscape (matches paper_size_to_nm default).
        width = 297_000_000
        height = 210_000_000
    return width, height


def _op_kind_str(op: KiCadPlotterOp) -> str:
    return op.kind.value if isinstance(op.kind, KiCadPlotterOpKind) else str(op.kind)


def _is_full_sheet_background_op(
    op: KiCadPlotterOp,
    *,
    ctx: KiCadSvgRenderContext,
) -> bool:
    """Return True for a worksheet page-background fill.

    Worksheet border/title-block geometry renders after the schematic so it
    stays visible, but a full-page background fill must render before the
    schematic or it hides the entire drawing.
    """
    if _op_kind_str(op) != KiCadPlotterOpKind.RECT.value:
        return False
    payload = op.payload or {}
    if not _is_filled(str(payload.get("fill", KiCadFillType.NO_FILL.value) or "")):
        return False

    try:
        x1 = int(payload.get("x1", 0) or 0)
        y1 = int(payload.get("y1", 0) or 0)
        x2 = int(payload.get("x2", 0) or 0)
        y2 = int(payload.get("y2", 0) or 0)
    except (TypeError, ValueError):
        return False

    left, right = sorted((x1, x2))
    top, bottom = sorted((y1, y2))
    return (
        left == 0
        and top == 0
        and right == int(ctx.sheet_width_nm)
        and bottom == int(ctx.sheet_height_nm)
    )


def _split_sheet_header_record(
    record: KiCadPlotterRecord,
    *,
    ctx: KiCadSvgRenderContext,
) -> tuple[KiCadPlotterRecord | None, KiCadPlotterRecord | None]:
    background_ops: list[KiCadPlotterOp] = []
    overlay_ops: list[KiCadPlotterOp] = []
    for op in record.operations:
        if _is_full_sheet_background_op(op, ctx=ctx):
            background_ops.append(op)
        else:
            overlay_ops.append(op)

    background = None
    if background_ops:
        background = KiCadPlotterRecord(
            uuid=f"{record.uuid}:background",
            kind="sheet_header_background",
            object_id=record.object_id,
            bounds=record.bounds,
            operations=background_ops,
            extras=dict(record.extras),
        )

    overlay = None
    if overlay_ops or not background_ops:
        overlay = KiCadPlotterRecord(
            uuid=record.uuid,
            kind=record.kind,
            object_id=record.object_id,
            bounds=record.bounds,
            operations=overlay_ops,
            extras=record.extras,
        )

    return background, overlay


def _svg_document_records(
    records: Iterable[KiCadPlotterRecord],
    *,
    ctx: KiCadSvgRenderContext,
) -> list[KiCadPlotterRecord]:
    """Return records in visual plot order for full-document SVG output."""
    sheet_backgrounds: list[KiCadPlotterRecord] = []
    regular: list[KiCadPlotterRecord] = []
    sheet_headers: list[KiCadPlotterRecord] = []
    for record in records:
        if record.kind == "sheet_header":
            background, overlay = _split_sheet_header_record(record, ctx=ctx)
            if background is not None:
                sheet_backgrounds.append(background)
            if overlay is not None:
                sheet_headers.append(overlay)
        else:
            regular.append(record)
    return sheet_backgrounds + regular + sheet_headers


def render_ir_to_svg(
    doc: KiCadPlotterDocument,
    *,
    options: KiCadSvgRenderOptions | None = None,
    ctx: KiCadSvgRenderContext | None = None,
    root_extra_attrs: dict[str, object] | None = None,
    metadata_elements: Iterable[str] | None = None,
) -> str:
    """
    Render a :class:`KiCadPlotterDocument` as a complete SVG document.

    Either pass a fully-prepared ``ctx`` (canvas dims already set) or
    let the renderer build a default context from ``options`` and the
    document's ``canvas`` field. Full-page worksheet background fills
    render first, while the ``sheet_header`` border and title block
    render last without changing the document's stored IR record order.
    """
    width_nm, height_nm = _resolve_canvas_dims(doc)
    if ctx is None:
        opts = options if options is not None else KiCadSvgRenderOptions()
        ctx = KiCadSvgRenderContext(
            sheet_width_nm=width_nm,
            sheet_height_nm=height_nm,
            options=opts,
        )
    else:
        # Honour any ctx the caller built but fill in sheet dims if
        # they were left at zero.
        if ctx.sheet_width_nm <= 0:
            ctx.sheet_width_nm = width_nm
        if ctx.sheet_height_nm <= 0:
            ctx.sheet_height_nm = height_nm

    body_parts: list[str] = []
    drill_overlay_parts: list[str] = []
    for rec in _svg_document_records(doc.records, ctx=ctx):
        operations = _record_visible_operations(rec, ctx=ctx)
        if operations is None:
            continue
        normal_ops, drill_overlay_ops = _split_drill_overlay_ops(operations)
        if normal_ops or not rec.operations:
            body_parts.append(
                _render_record_operations(rec, normal_ops, ctx=ctx)
            )
        if drill_overlay_ops:
            drill_overlay_parts.append(
                _render_record_operations(
                    rec,
                    drill_overlay_ops,
                    ctx=ctx,
                    label_suffix=":drill_overlay",
                    data_ref="drill_overlay",
                )
            )
    body_parts.extend(drill_overlay_parts)
    body = "\n".join(part for part in body_parts if part)
    return svg_document(
        body,
        ctx=ctx,
        root_extra_attrs=root_extra_attrs,
        metadata_elements=metadata_elements,
    )


def render_records(
    records: Iterable[KiCadPlotterRecord],
    *,
    ctx: KiCadSvgRenderContext,
) -> str:
    """
    Render an iterable of records back-to-back without a document
    envelope. Useful for embedding IR fragments inside a larger SVG.
    """
    parts = [render_record(rec, ctx=ctx) for rec in records]
    return "\n".join(part for part in parts if part)


__all__ = [
    "render_ir_to_svg",
    "render_op",
    "render_record",
    "render_records",
]

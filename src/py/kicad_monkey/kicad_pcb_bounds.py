"""PCB SVG viewBox helpers for the plotter-IR renderer."""

from __future__ import annotations

import base64
import math
import struct
from typing import TYPE_CHECKING

from .kicad_geometry import BoundingBox, HAlign, TextParams, VAlign, rotate_point
from .kicad_pcb_pad_svg import pad_on_layer
from .kicad_render_cache import (
    RenderCacheRequest,
    RenderCacheResolver,
    render_cache_request_for_dimension_text,
)
from .kicad_stroke_font import get_renderer as get_stroke_font_renderer
from .kicad_text import KiCadTextRenderer

if TYPE_CHECKING:
    from .kicad_pcb import KiCadPcb


_text_renderer = None


def _get_text_renderer() -> KiCadTextRenderer:
    global _text_renderer
    if _text_renderer is None:
        _text_renderer = KiCadTextRenderer()
    return _text_renderer


def _render_cache_polygons_for_request(
    request: RenderCacheRequest,
) -> list[list[tuple[float, float]]]:
    result = RenderCacheResolver().ensure_cache(request)
    if not result.usable or result.cache is None:
        return []

    polygons: list[list[tuple[float, float]]] = []
    for polygon in result.cache.polygons:
        if len(polygon.points) >= 3:
            polygons.append(list(polygon.points))
    return polygons


def _expand_bbox_with_polygons(
    bbox: BoundingBox,
    polygons: list[list[tuple[float, float]]],
) -> None:
    for polygon in polygons:
        for x, y in polygon:
            bbox.expand((x, y))


def _compute_property_text_bbox(
    prop,
    fp_x: float,
    fp_y: float,
    fp_angle: float,
) -> BoundingBox:
    bbox = BoundingBox()

    if prop.hide or prop.name.startswith("ki_"):
        return bbox

    try:
        effects = prop.effects
        font = effects.font if effects else None

        h_align_str = "center"
        v_align_str = "center"
        if effects and effects.justify:
            if "left" in effects.justify:
                h_align_str = "left"
            elif "right" in effects.justify:
                h_align_str = "right"
            if "top" in effects.justify:
                v_align_str = "top"
            elif "bottom" in effects.justify:
                v_align_str = "bottom"

        size_x = font.size_x if font else 1.27
        size_y = font.size_y if font else 1.27
        text_angle = prop.at_angle
        is_italic = font.italic if font else False
        is_mirrored = effects.justify and "mirror" in effects.justify if effects else False

        if font and font.face:
            h_align_map = {"left": HAlign.LEFT, "center": HAlign.CENTER, "right": HAlign.RIGHT}
            v_align_map = {"top": VAlign.TOP, "center": VAlign.CENTER, "bottom": VAlign.BOTTOM}

            params = TextParams(
                text=prop.value,
                font_name=font.face,
                size_x=size_x,
                size_y=size_y,
                position_x=prop.at_x,
                position_y=prop.at_y,
                angle=text_angle,
                bold=font.bold if font else False,
                italic=is_italic,
                stroke_width=font.effective_thickness if font else 0.127,
                h_align=h_align_map.get(h_align_str, HAlign.CENTER),
                v_align=v_align_map.get(v_align_str, VAlign.CENTER),
                mirrored=is_mirrored,
            )

            geometry = _get_text_renderer().render(params)
            text_bounds = geometry.get_bounds()

            if text_bounds.is_valid():
                corners = [
                    (text_bounds.min_x, text_bounds.min_y),
                    (text_bounds.max_x, text_bounds.min_y),
                    (text_bounds.max_x, text_bounds.max_y),
                    (text_bounds.min_x, text_bounds.max_y),
                ]
                for lx, ly in corners:
                    if fp_angle != 0:
                        lx, ly = rotate_point(lx, ly, -fp_angle)
                    bbox.expand((lx + fp_x, ly + fp_y))
        else:
            stroke_renderer = get_stroke_font_renderer()
            polylines = stroke_renderer.render_text_polylines(
                text=prop.value,
                pos_x=prop.at_x,
                pos_y=prop.at_y,
                size_x=size_x,
                size_y=size_y,
                angle=text_angle,
                h_align=h_align_str,
                v_align=v_align_str,
                mirror=is_mirrored,
                italic=is_italic,
            )

            for polyline in polylines:
                for lx, ly in polyline:
                    if fp_angle != 0:
                        lx, ly = rotate_point(lx, ly, -fp_angle)
                    bbox.expand((lx + fp_x, ly + fp_y))
    except Exception:
        pass

    return bbox


def _get_image_dimensions(data: str) -> tuple[int | None, int | None]:
    try:
        image_data = base64.b64decode(data)
    except Exception:
        return None, None

    if image_data[:2] == b"\xff\xd8":
        i = 2
        while i < len(image_data) - 10:
            if image_data[i] != 0xFF:
                i += 1
                continue
            marker = image_data[i + 1]
            if marker == 0xD8:
                i += 2
                continue
            if marker == 0xD9:
                break
            if marker in (0x00, 0xFF):
                i += 1
                continue
            if i + 4 > len(image_data):
                break
            length = struct.unpack(">H", image_data[i + 2 : i + 4])[0]
            if marker in (
                0xC0,
                0xC1,
                0xC2,
                0xC3,
                0xC5,
                0xC6,
                0xC7,
                0xC9,
                0xCA,
                0xCB,
                0xCD,
                0xCE,
                0xCF,
            ):
                if i + 9 <= len(image_data):
                    height = struct.unpack(">H", image_data[i + 5 : i + 7])[0]
                    width = struct.unpack(">H", image_data[i + 7 : i + 9])[0]
                    return width, height
            i += 2 + length
        return None, None

    if image_data[:8] == b"\x89PNG\r\n\x1a\n":
        if len(image_data) >= 24:
            width = struct.unpack(">I", image_data[16:20])[0]
            height = struct.unpack(">I", image_data[20:24])[0]
            return width, height
        return None, None

    return None, None


def compute_cubic_bezier_bounds(
    p0: tuple[float, float],
    p1: tuple[float, float],
    p2: tuple[float, float],
    p3: tuple[float, float],
) -> tuple[float, float, float, float]:
    min_x = min(p0[0], p3[0])
    max_x = max(p0[0], p3[0])
    min_y = min(p0[1], p3[1])
    max_y = max(p0[1], p3[1])

    for axis in [0, 1]:
        v0, v1, v2, v3 = p0[axis], p1[axis], p2[axis], p3[axis]
        a = 3 * (-v0 + 3 * v1 - 3 * v2 + v3)
        b = 6 * (v0 - 2 * v1 + v2)
        c = 3 * (v1 - v0)

        if abs(a) < 1e-10:
            if abs(b) > 1e-10:
                candidates = [-c / b]
            else:
                candidates = []
        else:
            discriminant = b * b - 4 * a * c
            if discriminant < 0:
                candidates = []
            else:
                sqrt_disc = math.sqrt(discriminant)
                candidates = [
                    (-b + sqrt_disc) / (2 * a),
                    (-b - sqrt_disc) / (2 * a),
                ]

        for t in candidates:
            if 0 < t < 1:
                val = (
                    (1 - t) ** 3 * v0
                    + 3 * (1 - t) ** 2 * t * v1
                    + 3 * (1 - t) * t**2 * v2
                    + t**3 * v3
                )
                if axis == 0:
                    min_x = min(min_x, val)
                    max_x = max(max_x, val)
                else:
                    min_y = min(min_y, val)
                    max_y = max(max_y, val)

    return (min_x, min_y, max_x, max_y)


def _is_copper_layer(layer: str) -> bool:
    return layer.endswith(".Cu") or layer in ("F.Cu", "B.Cu")


def compute_footprint_svg_bounding_box_on_layers(
    fp,
    layers: list[str] | None = None,
) -> BoundingBox:
    bbox = BoundingBox()
    fp_x = fp.at_x
    fp_y = fp.at_y
    fp_angle = fp.at_angle

    for pad in fp.pads:
        render_pad = layers is None or any(pad_on_layer(pad, layer) for layer in layers)
        if render_pad:
            pad_x, pad_y = pad.at_x, pad.at_y
            if fp_angle != 0:
                pad_x, pad_y = rotate_point(pad_x, pad_y, -fp_angle)
            pad_x += fp_x
            pad_y += fp_y

            half_w = pad.size_x / 2
            half_h = pad.size_y / 2
            r = max(half_w, half_h)
            bbox.expand((pad_x - r, pad_y - r))
            bbox.expand((pad_x + r, pad_y + r))

    for line in fp.fp_lines:
        if layers is None or line.layer in layers:
            sx, sy = line.start_x, line.start_y
            ex, ey = line.end_x, line.end_y
            if fp_angle != 0:
                sx, sy = rotate_point(sx, sy, -fp_angle)
                ex, ey = rotate_point(ex, ey, -fp_angle)
            bbox.expand((sx + fp_x, sy + fp_y))
            bbox.expand((ex + fp_x, ey + fp_y))

    for arc in fp.fp_arcs:
        if layers is None or arc.layer in layers:
            for x, y in [(arc.start_x, arc.start_y), (arc.mid_x, arc.mid_y), (arc.end_x, arc.end_y)]:
                if fp_angle != 0:
                    x, y = rotate_point(x, y, -fp_angle)
                bbox.expand((x + fp_x, y + fp_y))

    for circle in fp.fp_circles:
        if layers is None or circle.layer in layers:
            cx, cy = circle.center_x, circle.center_y
            ex, ey = circle.end_x, circle.end_y
            if fp_angle != 0:
                cx, cy = rotate_point(cx, cy, -fp_angle)
                ex, ey = rotate_point(ex, ey, -fp_angle)
            cx += fp_x
            cy += fp_y
            ex += fp_x
            ey += fp_y
            radius = math.hypot(ex - cx, ey - cy)
            bbox.expand((cx - radius, cy - radius))
            bbox.expand((cx + radius, cy + radius))

    for rect in fp.fp_rects:
        if layers is None or rect.layer in layers:
            sx, sy = rect.start_x, rect.start_y
            ex, ey = rect.end_x, rect.end_y
            if fp_angle != 0:
                sx, sy = rotate_point(sx, sy, -fp_angle)
                ex, ey = rotate_point(ex, ey, -fp_angle)
            sx += fp_x
            sy += fp_y
            ex += fp_x
            ey += fp_y
            bbox.expand((min(sx, ex), min(sy, ey)))
            bbox.expand((max(sx, ex), max(sy, ey)))

    for poly in fp.fp_polys:
        if layers is None or poly.layer in layers:
            for x, y in poly.points:
                if fp_angle != 0:
                    x, y = rotate_point(x, y, -fp_angle)
                bbox.expand((x + fp_x, y + fp_y))

    for prop in fp.properties:
        if layers is None or prop.layer in layers:
            bbox.merge(_compute_property_text_bbox(prop, fp_x, fp_y, fp_angle))

    return bbox


def _layer_visible(layer: str, layers: list[str] | None) -> bool:
    return layers is None or layer in layers


def _expand_routing_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    for seg in pcb.segments:
        if _layer_visible(seg.layer, layers):
            bbox.expand((seg.start_x, seg.start_y))
            bbox.expand((seg.end_x, seg.end_y))

    for arc in pcb.arcs:
        if _layer_visible(arc.layer, layers):
            bbox.expand((arc.start_x, arc.start_y))
            bbox.expand((arc.mid_x, arc.mid_y))
            bbox.expand((arc.end_x, arc.end_y))

    copper_layers = layers is None or any(_is_copper_layer(layer_name) for layer_name in layers)
    if not copper_layers:
        return
    for via in pcb.vias:
        if layers is None or any(layer_name in via.layers for layer_name in layers):
            r = via.size / 2
            bbox.expand((via.at_x - r, via.at_y - r))
            bbox.expand((via.at_x + r, via.at_y + r))


def _expand_zone_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    standard_copper_layers = {"F.Cu", "B.Cu", "In1.Cu"}
    for zone in pcb.zones:
        if zone.filled_polygons and zone.layer in standard_copper_layers:
            for poly in zone.polygons:
                if _layer_visible(zone.layer, layers):
                    for x, y in poly.points:
                        bbox.expand((x, y))

        for filled in zone.filled_polygons:
            if not _layer_visible(filled.layer, layers):
                continue
            for x, y in filled.points:
                bbox.expand((x, y))


def _expand_board_graphics_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    for line in pcb.gr_lines:
        if _layer_visible(line.layer, layers):
            bbox.expand((line.start_x, line.start_y))
            bbox.expand((line.end_x, line.end_y))

    for rect in pcb.gr_rects:
        if _layer_visible(rect.layer, layers):
            bbox.expand((min(rect.start_x, rect.end_x), min(rect.start_y, rect.end_y)))
            bbox.expand((max(rect.start_x, rect.end_x), max(rect.start_y, rect.end_y)))

    for circle in pcb.gr_circles:
        if _layer_visible(circle.layer, layers):
            radius = math.hypot(circle.end_x - circle.center_x, circle.end_y - circle.center_y)
            bbox.expand((circle.center_x - radius, circle.center_y - radius))
            bbox.expand((circle.center_x + radius, circle.center_y + radius))

    for arc in pcb.gr_arcs:
        if _layer_visible(arc.layer, layers):
            for x, y in [(arc.start_x, arc.start_y), (arc.mid_x, arc.mid_y), (arc.end_x, arc.end_y)]:
                bbox.expand((x, y))

    for poly in pcb.gr_polys:
        if _layer_visible(poly.layer, layers):
            for x, y in poly.points:
                bbox.expand((x, y))

    for curve in getattr(pcb, "gr_curves", []):
        if _layer_visible(curve.layer, layers) and len(curve.points) >= 4:
            p0, p1, p2, p3 = curve.points[:4]
            min_x, min_y, max_x, max_y = compute_cubic_bezier_bounds(p0, p1, p2, p3)
            bbox.expand((min_x, min_y))
            bbox.expand((max_x, max_y))


def _expand_board_text_box_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    for text_box in getattr(pcb, "gr_text_boxes", []):
        if _layer_visible(text_box.layer, layers):
            bbox.expand((min(text_box.start_x, text_box.end_x), min(text_box.start_y, text_box.end_y)))
            bbox.expand((max(text_box.start_x, text_box.end_x), max(text_box.start_y, text_box.end_y)))

    for table in getattr(pcb, "tables", []):
        for cell in table.cells:
            if layers is not None and table.layer not in layers and cell.layer not in layers:
                continue
            bbox.expand((min(cell.start_x, cell.end_x), min(cell.start_y, cell.end_y)))
            bbox.expand((max(cell.start_x, cell.end_x), max(cell.start_y, cell.end_y)))


def _expand_text_outline_bbox(bbox: BoundingBox, text_object) -> None:
    if not hasattr(text_object, "_to_poly"):
        return
    try:
        poly_set = text_object._to_poly()
    except Exception:
        return
    for outline in poly_set.outlines:
        for x, y in outline:
            bbox.expand((x, y))


def _expand_board_text_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    for text in getattr(pcb, "gr_texts", []):
        if _layer_visible(text.layer, layers):
            _expand_text_outline_bbox(bbox, text)


def _expand_dimension_text_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    for dimension_index, dimension in enumerate(getattr(pcb, "dimensions", [])):
        text_object = (
            dimension.resolved_gr_text()
            if hasattr(dimension, "resolved_gr_text")
            else getattr(dimension, "gr_text", None)
        )
        if text_object is None or not text_object.text:
            continue
        text_layer = text_object.layer or dimension.layer
        if not _layer_visible(text_layer, layers):
            continue

        font = text_object.effects.font if text_object.effects else None
        outline_polygons: list[list[tuple[float, float]]] = []
        if text_object.render_cache or (font and font.face):
            request = render_cache_request_for_dimension_text(
                dimension,
                pcb,
                object_path=f"dimension[{dimension_index}]/gr_text",
                include_text_params=bool(font and font.face),
            )
            outline_polygons = _render_cache_polygons_for_request(request)
        if outline_polygons:
            _expand_bbox_with_polygons(bbox, outline_polygons)
        else:
            _expand_text_outline_bbox(bbox, text_object)


def _expand_drill_bbox(bbox: BoundingBox, pcb: "KiCadPcb") -> None:
    for fp in pcb.footprints:
        fp_x = fp.at_x
        fp_y = fp.at_y
        fp_angle = fp.at_angle
        for pad in fp.pads:
            drill_size = None
            if hasattr(pad, "drill") and pad.drill and pad.drill > 0:
                drill_size = pad.drill
            elif hasattr(pad, "drill_width") and pad.drill_width and pad.drill_width > 0:
                drill_size = pad.drill_width
            if drill_size and drill_size > 0:
                px, py = pad.at_x, pad.at_y
                if fp_angle != 0:
                    px, py = rotate_point(px, py, -fp_angle)
                px += fp_x
                py += fp_y
                r = drill_size / 2
                bbox.expand((px - r, py - r))
                bbox.expand((px + r, py + r))

    for via in pcb.vias:
        drill_size = via.drill if via.drill and via.drill > 0 else via.size * 0.5
        if drill_size > 0:
            r = drill_size / 2
            bbox.expand((via.at_x - r, via.at_y - r))
            bbox.expand((via.at_x + r, via.at_y + r))


def _expand_image_bbox(bbox: BoundingBox, pcb: "KiCadPcb", layers: list[str] | None) -> None:
    for img in getattr(pcb, "images", []):
        if not _layer_visible(img.layer, layers):
            continue
        img_width_px, img_height_px = _get_image_dimensions(img.data)
        if img_width_px and img_height_px:
            scale = img.scale if hasattr(img, "scale") and img.scale else 1.0
            img_width_mm = img_width_px * 0.1 * scale
            img_height_mm = img_height_px * 0.1 * scale
            bbox.expand((img.at_x - img_width_mm / 2, img.at_y - img_height_mm / 2))
            bbox.expand((img.at_x + img_width_mm / 2, img.at_y + img_height_mm / 2))


def compute_pcb_svg_bounding_box(
    pcb: "KiCadPcb",
    layers: list[str] | None = None,
) -> BoundingBox:
    bbox = BoundingBox()

    for fp in pcb.footprints:
        bbox.merge(compute_footprint_svg_bounding_box_on_layers(fp, layers))

    _expand_routing_bbox(bbox, pcb, layers)
    _expand_zone_bbox(bbox, pcb, layers)
    _expand_board_graphics_bbox(bbox, pcb, layers)
    _expand_board_text_box_bbox(bbox, pcb, layers)
    _expand_board_text_bbox(bbox, pcb, layers)
    _expand_dimension_text_bbox(bbox, pcb, layers)
    _expand_drill_bbox(bbox, pcb)
    _expand_image_bbox(bbox, pcb, layers)

    return bbox


def empty_pcb_svg() -> str:
    return """<?xml version="1.0" standalone="no"?>
 <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN"
 "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg
  xmlns:svg="http://www.w3.org/2000/svg"
  xmlns="http://www.w3.org/2000/svg"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  version="1.1"
  width="0mm" height="0mm" viewBox="0 0 0 0">
</svg>
"""


__all__ = [
    "compute_cubic_bezier_bounds",
    "compute_footprint_svg_bounding_box_on_layers",
    "compute_pcb_svg_bounding_box",
    "empty_pcb_svg",
]

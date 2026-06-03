"""Composed A0 PCB SVG views for physical and virtual KiCad layers."""

from __future__ import annotations

import copy
import html
import math
import re
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

from kicad_cruncher.kicad_cruncher_pcb_svg_config import (
    PCB_SVG_ASSEMBLY_VIRTUAL_LAYERS,
    _PcbSvgConfig,
    normalize_layer_token,
    physical_layer_from_token,
)

if TYPE_CHECKING:
    from kicad_monkey.kicad_pcb import KiCadPcb

_SVG_NS = "http://www.w3.org/2000/svg"
_XLINK_NS = "http://www.w3.org/1999/xlink"
_EDGE_CUTS_LAYER = "Edge.Cuts"
_HLR_TOKENS = set(PCB_SVG_ASSEMBLY_VIRTUAL_LAYERS)
_HOLE_TOKENS = {"DRILLS", "SLOTS"}
_PIN1_TOKENS = {"PIN1_TOP", "PIN1_BOTTOM"}
_VIRTUAL_TOKENS = {
    "BOARD_OUTLINE",
    "BOARD_CUTOUTS",
    "DRILLS",
    "SLOTS",
    *_HLR_TOKENS,
    "ASSEMBLY_DESIGNATORS_TOP",
    "ASSEMBLY_DESIGNATORS_BOTTOM",
    "PIN1_TOP",
    "PIN1_BOTTOM",
}
_DRAWABLE_TAGS = {"circle", "ellipse", "line", "path", "polygon", "polyline", "rect", "text"}
_GRID_PAD_RE = re.compile(r"^([A-Za-z]+)(\d+)$")
_POINT_PRECISION = 4
_POINT_KEY_EPSILON = 1.0e-6
_MIN_REGION_AREA_MM2 = 1.0e-4

ET.register_namespace("", _SVG_NS)
ET.register_namespace("xlink", _XLINK_NS)


@dataclass(slots=True)
class PcbSvgComposition:
    """Rendered SVG text plus physical layer dependencies used to create it."""

    svg_text: str
    physical_layers: list[str]


@dataclass(slots=True)
class _BoardRegion:
    """Closed Edge.Cuts region sampled in board coordinates."""

    points: list[tuple[float, float]]
    source_kind: str
    source_ids: list[str] = field(default_factory=list)
    center: tuple[float, float] | None = None
    radius: float | None = None

    @property
    def area(self) -> float:
        return abs(_polygon_signed_area(self.points))

    @property
    def centroid(self) -> tuple[float, float]:
        return _polygon_centroid(self.points)

    @property
    def bounds(self) -> tuple[float, float, float, float]:
        xs = [point[0] for point in self.points]
        ys = [point[1] for point in self.points]
        return min(xs), min(ys), max(xs), max(ys)


@dataclass(slots=True)
class _EdgeSegment:
    """Open Edge.Cuts primitive prepared for loop assembly."""

    points: list[tuple[float, float]]
    source_kind: str
    source_id: str

    @property
    def start_key(self) -> tuple[int, int]:
        return _point_key(self.points[0])

    @property
    def end_key(self) -> tuple[int, int]:
        return _point_key(self.points[-1])


def render_pcb_svg_composition(
    pcb: KiCadPcb,
    layer_tokens: list[str],
    *,
    styles: dict[str, dict[str, object]],
    group_id: str,
    config: _PcbSvgConfig,
) -> PcbSvgComposition:
    """Render an A0 composed SVG using KiCad Monkey plus virtual layer renderers."""
    tokens = [normalize_layer_token(token) for token in layer_tokens]
    root_layers = _root_render_layers(tokens, pcb)
    root = _render_root_svg(pcb, root_layers)
    origin = _root_origin_from_bbox(pcb, root_layers)
    root.set("id", group_id)
    root.set("data-compositor-schema", "kicad_cruncher.pcb_svg.compositor.a0")
    root.set("data-layer-tokens", ",".join(tokens))

    source_children = [copy.deepcopy(child) for child in list(root)]
    root[:] = [
        child
        for child in source_children
        if _svg_local_name(child.tag) in {"metadata", "defs"}
    ]

    physical_layers = _physical_layers_for_tokens(tokens)
    copied_physical = False
    board_regions = _classify_edge_cut_regions(pcb)

    for token in tokens:
        copied_physical = _append_composition_token(
            root,
            source_children,
            token,
            pcb=pcb,
            tokens=tokens,
            board_regions=board_regions,
            origin=origin,
            styles=styles,
            config=config,
            copied_physical=copied_physical,
        )

    if not copied_physical and not any(token in _VIRTUAL_TOKENS for token in tokens):
        _append_physical_layer(
            root,
            source_children,
            _EDGE_CUTS_LAYER,
            tokens=tokens,
            styles=styles,
        )

    _reorder_top_level_groups(root)
    return PcbSvgComposition(
        svg_text=_svg_to_text(root),
        physical_layers=physical_layers,
    )


def _append_composition_token(
    root: ET.Element,
    source_children: list[ET.Element],
    token: str,
    *,
    pcb: KiCadPcb,
    tokens: list[str],
    board_regions: list[_BoardRegion],
    origin: tuple[float, float],
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
    copied_physical: bool,
) -> bool:
    if token in _HLR_TOKENS:
        return copied_physical
    if token == "BOARD_OUTLINE":
        _append_board_outline(root, board_regions, origin=origin, styles=styles)
        return copied_physical
    if token == "BOARD_CUTOUTS":
        _append_board_cutouts(root, board_regions, origin=origin, styles=styles)
        return copied_physical
    if token in _HOLE_TOKENS:
        _append_holes(root, source_children, token, styles=styles)
        return True
    if token in _PIN1_TOKENS:
        _append_pin1_markers(root, pcb, token, origin=origin, styles=styles, config=config)
        return copied_physical
    physical = physical_layer_from_token(token)
    if physical is None:
        return copied_physical
    _append_physical_layer(root, source_children, physical, tokens=tokens, styles=styles)
    return True


def _root_render_layers(tokens: list[str], pcb: KiCadPcb) -> list[str]:
    layers = _physical_layers_for_tokens(tokens)
    if _EDGE_CUTS_LAYER not in layers:
        layers.append(_EDGE_CUTS_LAYER)
    if _HOLE_TOKENS & set(tokens) and not any(layer.endswith(".Cu") for layer in layers):
        layers.extend(layer for layer in _pcb_copper_layers(pcb) if layer not in layers)
    return layers


def _physical_layers_for_tokens(tokens: list[str]) -> list[str]:
    layers: list[str] = []
    for token in tokens:
        if token == "BOARD_OUTLINE":
            continue
        if token in _HLR_TOKENS or token in _VIRTUAL_TOKENS:
            continue
        physical = physical_layer_from_token(token)
        if physical is not None and physical not in layers:
            layers.append(physical)
    return layers


def _pcb_copper_layers(pcb: KiCadPcb) -> list[str]:
    layers = [
        str(getattr(layer, "canonical_name", None) or getattr(layer, "name", None) or "")
        for layer in getattr(pcb, "layers", [])
    ]
    return [layer for layer in layers if layer.endswith(".Cu")]


def _render_root_svg(pcb: KiCadPcb, layers: list[str]) -> ET.Element:
    from kicad_monkey import KiCadSvgRenderOptions

    svg_text = str(
        pcb.to_svg(
            layers=layers or None,
            black_and_white=False,
            options=KiCadSvgRenderOptions.enriched_default(),
        )
    )
    return ET.fromstring(svg_text)


def _root_origin_from_bbox(
    pcb: KiCadPcb,
    layers: list[str],
) -> tuple[float, float]:
    from kicad_monkey.kicad_pcb_bounds import compute_pcb_svg_bounding_box

    del layers
    bbox = compute_pcb_svg_bounding_box(pcb, None)
    if not bbox.is_valid():
        return (0.0, 0.0)
    return (float(bbox.min_x), float(bbox.min_y))


def _append_physical_layer(
    root: ET.Element,
    source_children: list[ET.Element],
    layer: str,
    *,
    tokens: list[str],
    styles: dict[str, dict[str, object]],
) -> None:
    prune_holes = bool(_HOLE_TOKENS & set(tokens))
    for child in source_children:
        if _svg_local_name(child.tag) in {"metadata", "defs"}:
            continue
        candidate = copy.deepcopy(child)
        if _prune_for_layers(candidate, {layer}, prune_holes=prune_holes):
            _apply_a0_theme(candidate, styles, set(tokens))
            root.append(candidate)


def _append_holes(
    root: ET.Element,
    source_children: list[ET.Element],
    token: str,
    *,
    styles: dict[str, dict[str, object]],
) -> None:
    wanted_kind = "slot" if token == "SLOTS" else "round"
    group = ET.Element(
        _svg_tag("g"),
        {
            "id": f"pcb-svg-{token.lower()}",
            "data-layer-token": token,
            "data-ref": "hole-overlay",
        },
    )
    for child in source_children:
        if _svg_local_name(child.tag) in {"metadata", "defs"}:
            continue
        candidate = copy.deepcopy(child)
        if _prune_for_holes(candidate, wanted_kind):
            _apply_a0_theme(candidate, styles, {token})
            group.append(candidate)
    if len(group):
        root.append(group)


def _append_board_outline(
    root: ET.Element,
    regions: list[_BoardRegion],
    *,
    origin: tuple[float, float],
    styles: dict[str, dict[str, object]],
) -> None:
    outline = _outer_board_region(regions)
    if outline is None or not _style_enabled(styles, "board_outline"):
        return
    color = _style_color(styles, "board_outline", "#000000")
    width = _style_float(styles, "board_outline", "line_width_mm", 0.10)
    group = ET.Element(
        _svg_tag("g"),
        {
            "id": "pcb-svg-board-outline",
            "data-layer-token": "BOARD_OUTLINE",
            "data-feature": "board-outline",
            "data-source-kinds": outline.source_kind,
            "data-source-uuids": ",".join(outline.source_ids),
        },
    )
    group.append(
        _region_to_svg_element(
            outline,
            origin=origin,
            stroke=color,
            stroke_width=width,
            fill="none",
            extra_attrs={"data-feature": "board-outline"},
        )
    )
    root.append(group)


def _append_board_cutouts(
    root: ET.Element,
    regions: list[_BoardRegion],
    *,
    origin: tuple[float, float],
    styles: dict[str, dict[str, object]],
) -> None:
    cutouts = _interior_board_regions(regions)
    if not cutouts or not _style_enabled(styles, "board_cutouts"):
        return

    _ensure_cutout_hatch_defs(root, styles)
    color = _style_color(styles, "board_cutouts", "#FF0000")
    width = _style_float(styles, "board_cutouts", "outline_width_mm", 0.15)
    use_hatch = _style_bool(styles, "board_cutouts", "hatch", True)
    fill = "url(#board-cutout-hatch)" if use_hatch else "none"
    group = ET.Element(
        _svg_tag("g"),
        {
            "id": "pcb-svg-board-cutouts",
            "data-layer-token": "BOARD_CUTOUTS",
            "data-layer-key": "BOARD_CUTOUTS",
            "data-feature": "board-cutouts",
            "data-cutout-count": str(len(cutouts)),
        },
    )
    for index, cutout in enumerate(cutouts, start=1):
        group.append(
            _region_to_svg_element(
                cutout,
                origin=origin,
                stroke=color,
                stroke_width=width,
                fill=fill,
                extra_attrs={
                    "data-layer-key": "BOARD_CUTOUTS",
                    "data-feature": "board-cutout",
                    "data-cutout-index": str(index),
                    "data-source-kinds": cutout.source_kind,
                    "data-source-uuids": ",".join(cutout.source_ids),
                },
            )
        )
    root.append(group)


def _ensure_cutout_hatch_defs(
    root: ET.Element,
    styles: dict[str, dict[str, object]],
) -> None:
    defs = next((child for child in root if _svg_local_name(child.tag) == "defs"), None)
    if defs is None:
        defs = ET.Element(_svg_tag("defs"))
        root.insert(0, defs)
    if any(child.get("id") == "board-cutout-hatch" for child in defs):
        return
    color = _style_color(styles, "board_cutouts", "#FF0000")
    spacing = _style_float(styles, "board_cutouts", "hatch_spacing_mm", 0.5)
    angle = _style_float(styles, "board_cutouts", "hatch_angle_deg", 45.0)
    width = _style_float(styles, "board_cutouts", "hatch_line_width_mm", 0.08)
    pattern = ET.SubElement(
        defs,
        _svg_tag("pattern"),
        {
            "id": "board-cutout-hatch",
            "patternUnits": "userSpaceOnUse",
            "width": _fmt(spacing),
            "height": _fmt(spacing),
            "patternTransform": f"rotate({_fmt(angle)})",
        },
    )
    ET.SubElement(
        pattern,
        _svg_tag("line"),
        {
            "x1": "0",
            "y1": "0",
            "x2": "0",
            "y2": _fmt(spacing),
            "stroke": color,
            "stroke-width": _fmt(width),
        },
    )


def _append_pin1_markers(
    root: ET.Element,
    pcb: KiCadPcb,
    token: str,
    *,
    origin: tuple[float, float],
    styles: dict[str, dict[str, object]],
    config: _PcbSvgConfig,
) -> None:
    if not _style_enabled(styles, "pin1_marker"):
        return
    side = "bottom" if token == "PIN1_BOTTOM" else "top"
    color = _style_color(styles, "pin1_marker", "#2563EB")
    diameter = max(
        _style_float(styles, "pin1_marker", "dot_diameter_mm", 0.55),
        _style_float(styles, "pin1_marker", "min_dot_diameter_mm", 0.25),
    )
    group = ET.Element(
        _svg_tag("g"),
        {
            "id": f"pcb-svg-{token.lower()}",
            "data-layer-token": token,
            "data-feature": "pin1-markers",
        },
    )
    for footprint in getattr(pcb, "footprints", []) or []:
        marker = _pin1_marker_target(footprint, side=side, config=config)
        if marker is None:
            continue
        designator, pad = marker
        ET.SubElement(group, _svg_tag("circle"), _pin1_marker_attrs(
            footprint,
            pad,
            designator=designator,
            token=token,
            origin=origin,
            diameter=diameter,
            color=color,
        ))
    if len(group):
        root.append(group)


def _pin1_marker_target(
    footprint: object,
    *,
    side: str,
    config: _PcbSvgConfig,
) -> tuple[str, object] | None:
    if not _footprint_is_side(footprint, side, config=config):
        return None
    designator = _footprint_designator(footprint)
    override = config.components.get(designator)
    if override and override.pin1_enabled is False:
        return None
    if not (override and override.pin1_enabled is True) and _excluded_pin1_designator(
        designator, config
    ):
        return None
    pad = _select_pin1_pad(footprint, override_pin=(override.pin1_pad if override else None))
    if pad is None:
        return None
    return designator, pad


def _pin1_marker_attrs(
    footprint: object,
    pad: object,
    *,
    designator: str,
    token: str,
    origin: tuple[float, float],
    diameter: float,
    color: str,
) -> dict[str, str]:
    board_x, board_y = _pad_board_position(footprint, pad)
    return {
        "cx": _fmt(board_x - origin[0]),
        "cy": _fmt(board_y - origin[1]),
        "r": _fmt(diameter / 2.0),
        "fill": color,
        "stroke": color,
        "stroke-width": "0",
        "data-layer-token": token,
        "data-primitive": "pin1-marker",
        "data-component": designator,
        "data-component-uuid": str(getattr(footprint, "uuid", "") or ""),
        "data-footprint": str(getattr(footprint, "library_link", "") or ""),
        "data-pad-number": str(getattr(pad, "number", "") or ""),
        "data-pad-uuid": str(getattr(pad, "uuid", "") or ""),
    }


def _classify_edge_cut_regions(pcb: KiCadPcb) -> list[_BoardRegion]:
    regions: list[_BoardRegion] = []
    regions.extend(_closed_regions_from_open_segment_loops(pcb))
    regions.extend(_closed_regions_from_rects(pcb))
    regions.extend(_closed_regions_from_circles(pcb))
    regions.extend(_closed_regions_from_polys(pcb))
    return [region for region in regions if region.area > _MIN_REGION_AREA_MM2]


def _closed_regions_from_open_segment_loops(pcb: KiCadPcb) -> list[_BoardRegion]:
    segments = [
        *_edge_cut_line_segments(pcb),
        *_edge_cut_arc_segments(pcb),
        *_edge_cut_curve_segments(pcb),
    ]
    return _assemble_closed_segment_regions(segments)


def _edge_cut_line_segments(pcb: KiCadPcb) -> list[_EdgeSegment]:
    segments: list[_EdgeSegment] = []
    for line in getattr(pcb, "gr_lines", []) or []:
        if str(getattr(line, "layer", "")) != _EDGE_CUTS_LAYER:
            continue
        segments.append(
            _EdgeSegment(
                points=[
                    (float(getattr(line, "start_x", 0.0)), float(getattr(line, "start_y", 0.0))),
                    (float(getattr(line, "end_x", 0.0)), float(getattr(line, "end_y", 0.0))),
                ],
                source_kind="gr_line",
                source_id=str(getattr(line, "uuid", "") or ""),
            )
        )
    return segments


def _edge_cut_arc_segments(pcb: KiCadPcb) -> list[_EdgeSegment]:
    segments: list[_EdgeSegment] = []
    for arc in getattr(pcb, "gr_arcs", []) or []:
        if str(getattr(arc, "layer", "")) != _EDGE_CUTS_LAYER:
            continue
        segments.append(
            _EdgeSegment(
                points=_sample_arc_points(arc),
                source_kind="gr_arc",
                source_id=str(getattr(arc, "uuid", "") or ""),
            )
        )
    return segments


def _edge_cut_curve_segments(pcb: KiCadPcb) -> list[_EdgeSegment]:
    segments: list[_EdgeSegment] = []
    for curve in getattr(pcb, "gr_curves", []) or []:
        if str(getattr(curve, "layer", "")) != _EDGE_CUTS_LAYER:
            continue
        points = _sample_curve_points(curve)
        if len(points) < 2:
            continue
        segments.append(
            _EdgeSegment(
                points=points,
                source_kind="gr_curve",
                source_id=str(getattr(curve, "uuid", "") or ""),
            )
        )
    return segments


def _closed_regions_from_rects(pcb: KiCadPcb) -> list[_BoardRegion]:
    regions: list[_BoardRegion] = []
    for rect in getattr(pcb, "gr_rects", []) or []:
        if str(getattr(rect, "layer", "")) != _EDGE_CUTS_LAYER:
            continue
        x1 = float(min(getattr(rect, "start_x", 0.0), getattr(rect, "end_x", 0.0)))
        y1 = float(min(getattr(rect, "start_y", 0.0), getattr(rect, "end_y", 0.0)))
        x2 = float(max(getattr(rect, "start_x", 0.0), getattr(rect, "end_x", 0.0)))
        y2 = float(max(getattr(rect, "start_y", 0.0), getattr(rect, "end_y", 0.0)))
        regions.append(
            _BoardRegion(
                points=[(x1, y1), (x2, y1), (x2, y2), (x1, y2)],
                source_kind="gr_rect",
                source_ids=[str(getattr(rect, "uuid", "") or "")],
            )
        )
    return regions


def _closed_regions_from_circles(pcb: KiCadPcb) -> list[_BoardRegion]:
    regions: list[_BoardRegion] = []
    for circle in getattr(pcb, "gr_circles", []) or []:
        if str(getattr(circle, "layer", "")) != _EDGE_CUTS_LAYER:
            continue
        center = (
            float(getattr(circle, "center_x", 0.0)),
            float(getattr(circle, "center_y", 0.0)),
        )
        radius = math.hypot(
            float(getattr(circle, "end_x", 0.0)) - center[0],
            float(getattr(circle, "end_y", 0.0)) - center[1],
        )
        if radius <= 0.0:
            continue
        points = [
            (
                center[0] + math.cos(2.0 * math.pi * index / 64.0) * radius,
                center[1] + math.sin(2.0 * math.pi * index / 64.0) * radius,
            )
            for index in range(64)
        ]
        regions.append(
            _BoardRegion(
                points=points,
                source_kind="gr_circle",
                source_ids=[str(getattr(circle, "uuid", "") or "")],
                center=center,
                radius=radius,
            )
        )
    return regions


def _closed_regions_from_polys(pcb: KiCadPcb) -> list[_BoardRegion]:
    regions: list[_BoardRegion] = []
    for poly in getattr(pcb, "gr_polys", []) or []:
        if str(getattr(poly, "layer", "")) != _EDGE_CUTS_LAYER:
            continue
        points = [(float(x), float(y)) for x, y in getattr(poly, "points", []) or []]
        if len(points) >= 3:
            regions.append(
                _BoardRegion(
                    points=points,
                    source_kind="gr_poly",
                    source_ids=[str(getattr(poly, "uuid", "") or "")],
                )
            )
    return regions


def _assemble_closed_segment_regions(segments: list[_EdgeSegment]) -> list[_BoardRegion]:
    adjacency = _edge_segment_adjacency(segments)
    regions: list[_BoardRegion] = []
    visited: set[int] = set()
    for start_index, segment in enumerate(segments):
        if start_index in visited:
            continue
        region, local_seen = _closed_region_from_segment_loop(
            start_index,
            segment,
            segments,
            adjacency,
        )
        visited.update(local_seen)
        if region is not None:
            regions.append(region)
    return regions


def _edge_segment_adjacency(segments: list[_EdgeSegment]) -> dict[tuple[int, int], list[int]]:
    adjacency: dict[tuple[int, int], list[int]] = {}
    for index, segment in enumerate(segments):
        adjacency.setdefault(segment.start_key, []).append(index)
        adjacency.setdefault(segment.end_key, []).append(index)
    return adjacency


def _closed_region_from_segment_loop(
    start_index: int,
    segment: _EdgeSegment,
    segments: list[_EdgeSegment],
    adjacency: dict[tuple[int, int], list[int]],
) -> tuple[_BoardRegion | None, set[int]]:
    points, source_ids, source_kinds, current_key, local_seen = _walk_segment_loop(
        start_index,
        segment,
        segments,
        adjacency,
    )
    start_key = segment.start_key
    if current_key != start_key or len(points) < 3:
        return None, local_seen
    if _point_key(points[-1]) == start_key:
        points = points[:-1]
    if abs(_polygon_signed_area(points)) <= _MIN_REGION_AREA_MM2:
        return None, local_seen
    return (
        _BoardRegion(
            points=points,
            source_kind="+".join(sorted(source_kinds)),
            source_ids=source_ids,
        ),
        local_seen,
    )


def _walk_segment_loop(
    start_index: int,
    segment: _EdgeSegment,
    segments: list[_EdgeSegment],
    adjacency: dict[tuple[int, int], list[int]],
) -> tuple[list[tuple[float, float]], list[str], set[str], tuple[int, int], set[int]]:
    local_seen = {start_index}
    points = list(segment.points)
    source_ids = [segment.source_id] if segment.source_id else []
    source_kinds = {segment.source_kind}
    start_key = segment.start_key
    current_key = segment.end_key
    while current_key != start_key:
        next_index = _next_segment_index(adjacency, current_key, local_seen)
        if next_index is None:
            break
        next_segment = segments[next_index]
        local_seen.add(next_index)
        next_points, current_key = _oriented_segment_points(next_segment, current_key)
        points.extend(next_points[1:])
        if next_segment.source_id:
            source_ids.append(next_segment.source_id)
        source_kinds.add(next_segment.source_kind)
    return points, source_ids, source_kinds, current_key, local_seen


def _next_segment_index(
    adjacency: dict[tuple[int, int], list[int]],
    current_key: tuple[int, int],
    local_seen: set[int],
) -> int | None:
    return next(
        (
            candidate
            for candidate in adjacency.get(current_key, [])
            if candidate not in local_seen
        ),
        None,
    )


def _oriented_segment_points(
    segment: _EdgeSegment,
    current_key: tuple[int, int],
) -> tuple[list[tuple[float, float]], tuple[int, int]]:
    if segment.start_key == current_key:
        return segment.points, segment.end_key
    return list(reversed(segment.points)), segment.start_key


def _outer_board_region(regions: list[_BoardRegion]) -> _BoardRegion | None:
    if not regions:
        return None
    return max(regions, key=lambda region: region.area)


def _interior_board_regions(regions: list[_BoardRegion]) -> list[_BoardRegion]:
    outer = _outer_board_region(regions)
    if outer is None:
        return []
    result: list[_BoardRegion] = []
    for region in regions:
        if region is outer:
            continue
        if region.area >= outer.area * 0.95:
            continue
        if _region_inside_outer(region, outer):
            result.append(region)
    return sorted(result, key=lambda region: (region.centroid[1], region.centroid[0]))


def _region_inside_outer(region: _BoardRegion, outer: _BoardRegion) -> bool:
    min_x, min_y, max_x, max_y = region.bounds
    outer_min_x, outer_min_y, outer_max_x, outer_max_y = outer.bounds
    if min_x < outer_min_x or min_y < outer_min_y or max_x > outer_max_x or max_y > outer_max_y:
        return False
    return _point_in_polygon(region.centroid, outer.points)


def _sample_arc_points(arc: object) -> list[tuple[float, float]]:
    start = (float(getattr(arc, "start_x", 0.0)), float(getattr(arc, "start_y", 0.0)))
    mid = (float(getattr(arc, "mid_x", 0.0)), float(getattr(arc, "mid_y", 0.0)))
    end = (float(getattr(arc, "end_x", 0.0)), float(getattr(arc, "end_y", 0.0)))
    circle = _circle_from_three_points(start, mid, end)
    if circle is None:
        return [start, mid, end]
    center_x, center_y, radius = circle
    start_angle = math.atan2(start[1] - center_y, start[0] - center_x)
    mid_angle = math.atan2(mid[1] - center_y, mid[0] - center_x)
    end_angle = math.atan2(end[1] - center_y, end[0] - center_x)
    ccw_delta = _positive_angle_delta(start_angle, end_angle)
    mid_delta = _positive_angle_delta(start_angle, mid_angle)
    sweep = ccw_delta if mid_delta <= ccw_delta else -(2.0 * math.pi - ccw_delta)
    samples = max(6, min(96, int(abs(sweep) / (math.pi / 24.0)) + 2))
    return [
        (
            center_x + math.cos(start_angle + sweep * index / (samples - 1)) * radius,
            center_y + math.sin(start_angle + sweep * index / (samples - 1)) * radius,
        )
        for index in range(samples)
    ]


def _sample_curve_points(curve: object) -> list[tuple[float, float]]:
    raw_points = getattr(curve, "points", []) or []
    points = [(float(x), float(y)) for x, y in raw_points[:4]]
    if len(points) != 4:
        return points

    p0, p1, p2, p3 = points
    control_length = (
        math.dist(p0, p1)
        + math.dist(p1, p2)
        + math.dist(p2, p3)
    )
    samples = max(8, min(128, int(control_length / 0.5) + 2))
    result: list[tuple[float, float]] = []
    for index in range(samples):
        t = index / (samples - 1)
        omt = 1.0 - t
        x = (
            omt**3 * p0[0]
            + 3.0 * omt**2 * t * p1[0]
            + 3.0 * omt * t**2 * p2[0]
            + t**3 * p3[0]
        )
        y = (
            omt**3 * p0[1]
            + 3.0 * omt**2 * t * p1[1]
            + 3.0 * omt * t**2 * p2[1]
            + t**3 * p3[1]
        )
        result.append((x, y))
    return result


def _circle_from_three_points(
    p1: tuple[float, float],
    p2: tuple[float, float],
    p3: tuple[float, float],
) -> tuple[float, float, float] | None:
    ax, ay = p1
    bx, by = p2
    cx, cy = p3
    determinant = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by))
    if abs(determinant) < 1.0e-9:
        return None
    ax2ay2 = ax * ax + ay * ay
    bx2by2 = bx * bx + by * by
    cx2cy2 = cx * cx + cy * cy
    center_x = (
        ax2ay2 * (by - cy) + bx2by2 * (cy - ay) + cx2cy2 * (ay - by)
    ) / determinant
    center_y = (
        ax2ay2 * (cx - bx) + bx2by2 * (ax - cx) + cx2cy2 * (bx - ax)
    ) / determinant
    return center_x, center_y, math.hypot(ax - center_x, ay - center_y)


def _positive_angle_delta(start: float, end: float) -> float:
    return (end - start) % (2.0 * math.pi)


def _region_to_svg_element(
    region: _BoardRegion,
    *,
    origin: tuple[float, float],
    stroke: str,
    stroke_width: float,
    fill: str,
    extra_attrs: dict[str, str],
) -> ET.Element:
    attrs = {
        "fill": fill,
        "stroke": stroke,
        "stroke-width": _fmt(stroke_width),
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
        **extra_attrs,
    }
    if region.center is not None and region.radius is not None:
        return ET.Element(
            _svg_tag("circle"),
            {
                "cx": _fmt(region.center[0] - origin[0]),
                "cy": _fmt(region.center[1] - origin[1]),
                "r": _fmt(region.radius),
                **attrs,
            },
        )
    points = [(x - origin[0], y - origin[1]) for x, y in region.points]
    path_data = " ".join(
        [f"M {_fmt(points[0][0])} {_fmt(points[0][1])}"]
        + [f"L {_fmt(x)} {_fmt(y)}" for x, y in points[1:]]
        + ["Z"]
    )
    return ET.Element(_svg_tag("path"), {"d": path_data, **attrs})


def _prune_for_layers(
    element: ET.Element,
    layers: set[str],
    *,
    prune_holes: bool,
    inherited: dict[str, str] | None = None,
) -> bool:
    attrs = _merged_attrs(inherited, element)
    if prune_holes and _is_hole_category(attrs):
        return False
    keep_self = _attrs_match_layers(attrs, layers)
    child_kept = _prune_layer_children(
        element,
        layers,
        prune_holes=prune_holes,
        inherited=_next_inherited_attrs(inherited, element),
    )
    return _keep_pruned_layer_element(element, attrs, keep_self, child_kept)


def _prune_layer_children(
    element: ET.Element,
    layers: set[str],
    *,
    prune_holes: bool,
    inherited: dict[str, str],
) -> bool:
    child_kept = False
    for child in list(element):
        if _prune_for_layers(child, layers, prune_holes=prune_holes, inherited=inherited):
            child_kept = True
            continue
        element.remove(child)
    return child_kept


def _keep_pruned_layer_element(
    element: ET.Element,
    attrs: dict[str, str],
    keep_self: bool,
    child_kept: bool,
) -> bool:
    local_name = _svg_local_name(element.tag)
    if local_name in {"metadata", "defs"}:
        return False
    if local_name == "g":
        return keep_self or child_kept
    if local_name in _DRAWABLE_TAGS:
        return keep_self or (not _has_layer_attrs(attrs) and child_kept)
    return child_kept


def _prune_for_holes(
    element: ET.Element,
    wanted_kind: str,
    inherited: dict[str, str] | None = None,
) -> bool:
    attrs = _merged_attrs(inherited, element)
    is_hole = _is_hole_category(attrs)
    keep_self = is_hole and attrs.get("data-hole-kind", "round") == wanted_kind
    own_data = {key: value for key, value in element.attrib.items() if key.startswith("data-")}
    next_inherited = {**(inherited or {}), **own_data}
    child_kept = False
    for child in list(element):
        if not _prune_for_holes(child, wanted_kind, next_inherited):
            element.remove(child)
        else:
            child_kept = True
    local_name = _svg_local_name(element.tag)
    if local_name in {"metadata", "defs"}:
        return False
    if local_name == "g":
        return keep_self or child_kept
    return keep_self


def _merged_attrs(inherited: dict[str, str] | None, element: ET.Element) -> dict[str, str]:
    attrs = dict(inherited or {})
    attrs.update({key: value for key, value in element.attrib.items() if key.startswith("data-")})
    return attrs


def _next_inherited_attrs(
    inherited: dict[str, str] | None,
    element: ET.Element,
) -> dict[str, str]:
    own_data = {key: value for key, value in element.attrib.items() if key.startswith("data-")}
    return {**(inherited or {}), **own_data}


def _has_layer_attrs(attrs: dict[str, str]) -> bool:
    return any(
        key in attrs
        for key in (
            "data-layer-name",
            "data-layer-names",
            "data-layer-role",
        )
    )


def _attrs_match_layers(attrs: dict[str, str], layers: set[str]) -> bool:
    layer_values = _layer_values_from_attrs(attrs)
    if not layer_values:
        return False
    return any(_layer_value_matches(value, layers) for value in layer_values)


def _layer_values_from_attrs(attrs: dict[str, str]) -> list[str]:
    values: list[str] = []
    if layer_name := attrs.get("data-layer-name"):
        values.append(layer_name)
    if layer_names := attrs.get("data-layer-names"):
        values.extend(part.strip() for part in layer_names.split(",") if part.strip())
    if attrs.get("data-layer-role") == "board-outline":
        values.append(_EDGE_CUTS_LAYER)
    return values


def _layer_value_matches(value: str, layers: set[str]) -> bool:
    if value in layers:
        return True
    if value == "*.Cu":
        return any(layer.endswith(".Cu") for layer in layers)
    if value == "*.Mask":
        return any(layer.endswith(".Mask") for layer in layers)
    if value == "*.Paste":
        return any(layer.endswith(".Paste") for layer in layers)
    return False


def _apply_a0_theme(
    element: ET.Element,
    styles: dict[str, dict[str, object]],
    tokens: set[str],
    inherited: dict[str, str] | None = None,
) -> None:
    attrs = _merged_attrs(inherited, element)
    category = _svg_category(attrs)
    _apply_a0_theme_to_element(element, attrs, category, styles)
    next_inherited = _next_inherited_attrs(inherited, element)
    for child in list(element):
        _apply_a0_theme(child, styles, tokens, next_inherited)


def _apply_a0_theme_to_element(
    element: ET.Element,
    attrs: dict[str, str],
    category: str,
    styles: dict[str, dict[str, object]],
) -> None:
    if category == "hole":
        _apply_hole_theme(element, attrs, styles)
        return
    color_style = _a0_theme_color_style(attrs, category)
    if color_style is not None:
        _set_svg_element_color(element, _style_color(styles, color_style, "#000000"))


def _apply_hole_theme(
    element: ET.Element,
    attrs: dict[str, str],
    styles: dict[str, dict[str, object]],
) -> None:
    _set_svg_element_color(element, _hole_color(attrs, styles))
    style_name = "slots" if attrs.get("data-hole-kind", "") == "slot" else "drills"
    opacity = _style_float(styles, style_name, "opacity", 1.0)
    if _svg_local_name(element.tag) in _DRAWABLE_TAGS:
        element.set("fill-opacity", _fmt(opacity))
        element.set("stroke-opacity", _fmt(opacity))


def _a0_theme_color_style(attrs: dict[str, str], category: str) -> str | None:
    if category == "edge":
        return "board_outline"
    if category == "zone":
        return "copper_polygons"
    if category == "track":
        return "vias" if attrs.get("data-primitive", "") == "via" else "copper_traces"
    if category == "pad":
        return _pad_style_name(attrs)
    if category == "silk":
        return (
            "silkscreen_designators"
            if attrs.get("data-footprint-text-role") == "designator"
            else "silkscreen_component_graphics"
        )
    return None


def _svg_category(attrs: dict[str, str]) -> str:
    for category, predicate in _SVG_CATEGORY_RULES:
        if predicate(attrs):
            return category
    return "other"


def _is_edge_category(attrs: dict[str, str]) -> bool:
    return (
        attrs.get("data-layer-name", "") == _EDGE_CUTS_LAYER
        or attrs.get("data-layer-role", "") == "board-outline"
        or _EDGE_CUTS_LAYER in attrs.get("data-layer-names", "")
    )


def _is_zone_category(attrs: dict[str, str]) -> bool:
    return attrs.get("data-ref", "") == "zone_fill" or attrs.get("data-primitive", "") == "zone"


def _is_track_category(attrs: dict[str, str]) -> bool:
    return attrs.get("data-ref", "") in {"segment", "track_arc", "via"} or attrs.get(
        "data-primitive", ""
    ) in {"track", "arc", "via"}


def _is_pad_category(attrs: dict[str, str]) -> bool:
    return attrs.get("data-ref", "") in {"pad", "footprint"} or attrs.get(
        "data-primitive", ""
    ) == "pad"


def _is_silk_category(attrs: dict[str, str]) -> bool:
    return "SilkS" in attrs.get("data-layer-name", "") or "SilkS" in attrs.get(
        "data-layer-names", ""
    )


def _is_hole_category(attrs: dict[str, str]) -> bool:
    return (
        attrs.get("data-ref", "") in {"drill_overlay", "pad_hole"}
        or attrs.get("data-primitive", "") in {"pad-hole", "via-hole"}
        or attrs.get("data-hole-render", "") in {"drill", "slot"}
    )


_SVG_CATEGORY_RULES = (
    ("hole", _is_hole_category),
    ("edge", _is_edge_category),
    ("zone", _is_zone_category),
    ("track", _is_track_category),
    ("pad", _is_pad_category),
    ("silk", _is_silk_category),
)


def _hole_color(attrs: dict[str, str], styles: dict[str, dict[str, object]]) -> str:
    style_name = "slots" if attrs.get("data-hole-kind", "") == "slot" else "drills"
    style = styles.get(style_name, {})
    plated = attrs.get("data-hole-plating", "") != "non_plated"
    key = "plated_color" if plated else "non_plated_color"
    return str(style.get(key) or ("#90EE90" if plated else "#ADD8E6"))


def _pad_style_name(attrs: dict[str, str]) -> str:
    return "smd_pads" if attrs.get("data-pad-type", "") == "smd" else "through_hole_pads"


def _reorder_top_level_groups(root: ET.Element) -> None:
    order = {
        "track": 10,
        "zone": 20,
        "other": 25,
        "edge": 30,
        "pad": 40,
        "hole": 50,
        "pin1": 60,
        "defs": 0,
        "metadata": 1,
    }

    def key(item: tuple[int, ET.Element]) -> tuple[int, int]:
        index, element = item
        local_name = _svg_local_name(element.tag)
        if local_name in {"defs", "metadata"}:
            return (order[local_name], index)
        if element.get("data-layer-token", "") in _PIN1_TOKENS:
            return (order["pin1"], index)
        attrs = {name: value for name, value in element.attrib.items() if name.startswith("data-")}
        return (order.get(_svg_category(attrs), order["other"]), index)

    root[:] = [element for _index, element in sorted(enumerate(list(root)), key=key)]


def _footprint_designator(footprint: object) -> str:
    get_property = getattr(footprint, "get_property_value", None)
    return str(get_property("Reference", "") if callable(get_property) else "")


def _footprint_is_side(footprint: object, side: str, *, config: _PcbSvgConfig) -> bool:
    designator = _footprint_designator(footprint)
    override = config.components.get(designator)
    if override and override.side:
        return override.side == side
    layer = str(getattr(footprint, "layer", "") or "")
    return (side == "bottom" and layer.startswith("B.")) or (
        side == "top" and not layer.startswith("B.")
    )


def _excluded_pin1_designator(designator: str, config: _PcbSvgConfig) -> bool:
    upper = designator.upper()
    return any(
        upper.startswith(prefix.upper())
        for prefix in config.pin1.exclude_designator_prefixes
    )


def _select_pin1_pad(footprint: object, *, override_pin: str | None) -> object | None:
    pads = list(getattr(footprint, "pads", []) or [])
    if not pads:
        return None
    if override_pin:
        selected = _pad_by_name(pads, override_pin)
        if selected is not None:
            return selected
    selected = _pad_by_candidates(pads, ("1", "A1"))
    if selected is not None:
        return selected
    grid_pad = _grid_pin1_pad(pads)
    if grid_pad is not None:
        return grid_pad
    return pads[0]


def _pad_by_candidates(pads: list[object], candidates: tuple[str, ...]) -> object | None:
    for candidate in candidates:
        selected = _pad_by_name(pads, candidate)
        if selected is not None:
            return selected
    return None


def _grid_pin1_pad(pads: list[object]) -> object | None:
    grid_pads = [
        (match.group(1).upper(), int(match.group(2)), pad)
        for pad in pads
        if (match := _GRID_PAD_RE.match(str(getattr(pad, "number", "") or "")))
    ]
    if not grid_pads:
        return None
    return sorted(grid_pads, key=lambda item: (item[1], item[0]))[0][2]


def _pad_by_name(pads: list[object], name: str) -> object | None:
    wanted = name.strip().upper()
    for pad in pads:
        if str(getattr(pad, "number", "") or "").strip().upper() == wanted:
            return pad
    return None


def _pad_board_position(footprint: object, pad: object) -> tuple[float, float]:
    x = float(getattr(pad, "at_x", 0.0) or 0.0)
    y = float(getattr(pad, "at_y", 0.0) or 0.0)
    angle = math.radians(-float(getattr(footprint, "at_angle", 0.0) or 0.0))
    cosine = math.cos(angle)
    sine = math.sin(angle)
    return (
        float(getattr(footprint, "at_x", 0.0) or 0.0) + x * cosine - y * sine,
        float(getattr(footprint, "at_y", 0.0) or 0.0) + x * sine + y * cosine,
    )


def _set_svg_element_color(element: ET.Element, color: str) -> None:
    style = element.get("style")
    if style:
        element.set("style", _style_with_color(style, color))
    if _svg_local_name(element.tag) in _DRAWABLE_TAGS:
        fill = element.get("fill")
        if fill is not None and fill.lower() != "none":
            element.set("fill", color)
        stroke = element.get("stroke")
        if stroke is not None and stroke.lower() != "none":
            element.set("stroke", color)


def _style_with_color(style: str, color: str) -> str:
    parts: list[str] = []
    seen_fill = False
    seen_stroke = False
    for item in style.split(";"):
        item = item.strip()
        if not item:
            continue
        name, sep, value = item.partition(":")
        if not sep:
            parts.append(item)
            continue
        key = name.strip().lower()
        raw_value = value.strip()
        if key == "fill":
            seen_fill = True
            parts.append(f"{name.strip()}:{raw_value if raw_value.lower() == 'none' else color}")
        elif key == "stroke":
            seen_stroke = True
            parts.append(f"{name.strip()}:{raw_value if raw_value.lower() == 'none' else color}")
        else:
            parts.append(f"{name.strip()}:{raw_value}")
    if not seen_fill:
        parts.append(f"fill:{color}")
    if not seen_stroke:
        parts.append(f"stroke:{color}")
    return "; ".join(parts)


def _style_enabled(styles: dict[str, dict[str, object]], name: str) -> bool:
    return bool(styles.get(name, {}).get("enabled", True))


def _style_color(styles: dict[str, dict[str, object]], name: str, default: str) -> str:
    return str(styles.get(name, {}).get("color") or default)


def _style_float(
    styles: dict[str, dict[str, object]],
    name: str,
    key: str,
    default: float,
) -> float:
    value = styles.get(name, {}).get(key, default)
    if not isinstance(value, int | float | str):
        raise ValueError(f"Invalid pcb-svg style value {name}.{key}")
    return float(value)


def _style_bool(
    styles: dict[str, dict[str, object]],
    name: str,
    key: str,
    default: bool,
) -> bool:
    value = styles.get(name, {}).get(key, default)
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        normalized = value.lower().strip()
        if normalized in {"1", "true", "yes", "on"}:
            return True
        if normalized in {"0", "false", "no", "off"}:
            return False
    return bool(value)


def _polygon_signed_area(points: list[tuple[float, float]]) -> float:
    if len(points) < 3:
        return 0.0
    return 0.5 * sum(
        points[index][0] * points[(index + 1) % len(points)][1]
        - points[(index + 1) % len(points)][0] * points[index][1]
        for index in range(len(points))
    )


def _polygon_centroid(points: list[tuple[float, float]]) -> tuple[float, float]:
    area = _polygon_signed_area(points)
    if abs(area) < 1.0e-12:
        x_values = [point[0] for point in points]
        y_values = [point[1] for point in points]
        return (sum(x_values) / len(x_values), sum(y_values) / len(y_values))
    factor = 1.0 / (6.0 * area)
    cx = 0.0
    cy = 0.0
    for index, point in enumerate(points):
        next_point = points[(index + 1) % len(points)]
        cross = point[0] * next_point[1] - next_point[0] * point[1]
        cx += (point[0] + next_point[0]) * cross
        cy += (point[1] + next_point[1]) * cross
    return (cx * factor, cy * factor)


def _point_in_polygon(point: tuple[float, float], polygon: list[tuple[float, float]]) -> bool:
    x, y = point
    inside = False
    j = len(polygon) - 1
    for i, pi in enumerate(polygon):
        pj = polygon[j]
        if ((pi[1] > y) != (pj[1] > y)) and (
            x < (pj[0] - pi[0]) * (y - pi[1]) / (pj[1] - pi[1]) + pi[0]
        ):
            inside = not inside
        j = i
    return inside


def _point_key(point: tuple[float, float]) -> tuple[int, int]:
    scale = 10**_POINT_PRECISION
    return (_quantize_point_coord(point[0], scale), _quantize_point_coord(point[1], scale))


def _quantize_point_coord(value: float, scale: int) -> int:
    scaled = value * scale
    if scaled >= 0.0:
        return math.floor(scaled + 0.5 + _POINT_KEY_EPSILON)
    return math.ceil(scaled - 0.5 - _POINT_KEY_EPSILON)


def _svg_tag(local_name: str) -> str:
    return f"{{{_SVG_NS}}}{local_name}"


def _svg_local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1] if "}" in tag else tag


def _fmt(value: float) -> str:
    text = f"{float(value):.4f}".rstrip("0").rstrip(".")
    return text or "0"


def _svg_to_text(root: ET.Element) -> str:
    ET.indent(root, space="  ")
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(
        root,
        encoding="unicode",
        short_empty_elements=True,
    )


def _escaped(value: object) -> str:
    return html.escape(str(value), quote=True)

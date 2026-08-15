"""
Typed render-cache validation and resolution helpers.

This module is the shared boundary for KiCad outline-font cache use.  It does
not generate glyphs yet; it validates whether an existing file cache can be used
for a semantic text object and gives renderers a single place to consume cache
geometry.
"""

from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass, field
from enum import Enum
from math import isclose
from typing import Any, Mapping, Optional, Tuple

from .kicad_primitives import RenderCache, RenderCacheContour, RenderCachePolygon
from .kicad_text_variables import (
    board_text_variables,
    footprint_text_variables,
    substitute_text_variables,
    table_cell_text_variables,
)


class RenderCacheSource(str, Enum):
    """Where a resolved cache came from."""

    EXISTING_FILE_CACHE = "existing_file_cache"
    MISSING = "missing_cache"
    INVALID_EXISTING_CACHE = "invalid_existing_cache"
    PYTHON_GENERATED_CACHE = "python_generated_cache"
    KICAD_ORACLE_CACHE = "kicad_oracle_cache"
    NATIVE_GENERATED_CACHE = "native_generated_cache"


@dataclass(frozen=True)
class RenderCacheRequest:
    """Semantic text request used to validate or resolve a render cache."""

    text: str
    angle: Optional[float] = None  # None means the caller cannot validate angle.
    render_cache: Optional[RenderCache] = None
    object_type: str = ""
    object_path: str = ""
    font_face: Optional[str] = None
    mirrored: Optional[bool] = None
    offset: Optional[Tuple[float, float]] = None
    text_params: Optional[Any] = None
    embedded_fonts: Tuple[Tuple[str, bytes], ...] = ()
    angle_tolerance: float = 1e-9


@dataclass
class RenderCacheValidation:
    """Validation result for an existing render cache."""

    valid: bool
    reasons: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return self.valid


@dataclass
class RenderCacheResult:
    """Resolved cache payload plus provenance."""

    cache: Optional[RenderCache]
    source: RenderCacheSource
    validation: RenderCacheValidation
    exact: bool = False

    @property
    def usable(self) -> bool:
        return self.cache is not None and self.validation.valid


class RenderCacheResolver:
    """Validate and resolve typed KiCad render caches."""

    def validate_cache(self, request: RenderCacheRequest) -> RenderCacheValidation:
        cache = request.render_cache
        reasons: list[str] = []
        warnings: list[str] = []

        if cache is None:
            return RenderCacheValidation(valid=False, reasons=["missing_cache"])

        if cache.text != request.text:
            reasons.append("resolved_text_mismatch")

        if request.angle is None:
            warnings.append("angle_context_not_provided")
        elif not isclose(cache.angle, request.angle, abs_tol=request.angle_tolerance):
            reasons.append("angle_mismatch")

        if not cache.polygons:
            reasons.append("empty_cache")

        for index, polygon in enumerate(cache.polygons):
            if len(polygon.points) < 3:
                reasons.append(f"polygon_{index}_empty_exterior")
            for contour_index, contour in enumerate(polygon.contours[1:], start=1):
                if len(contour.points) < 3:
                    reasons.append(f"polygon_{index}_hole_{contour_index}_empty")

        if request.font_face is not None:
            warnings.append("font_context_not_serialized_in_kicad_cache")
        if request.mirrored is not None:
            warnings.append("mirror_state_not_serialized_in_kicad_cache")
        if request.offset is not None:
            warnings.append("offset_not_serialized_in_kicad_cache")

        return RenderCacheValidation(
            valid=not reasons,
            reasons=reasons,
            warnings=warnings,
        )

    def ensure_cache(self, request: RenderCacheRequest) -> RenderCacheResult:
        validation = self.validate_cache(request)
        if request.render_cache is None:
            if request.text_params is not None:
                return self.generate_cache(request)
            return RenderCacheResult(
                cache=None,
                source=RenderCacheSource.MISSING,
                validation=validation,
                exact=False,
            )

        if validation.valid:
            return RenderCacheResult(
                cache=request.render_cache,
                source=RenderCacheSource.EXISTING_FILE_CACHE,
                validation=validation,
                exact=not validation.warnings,
            )

        if request.text_params is not None:
            return self.generate_cache(request)

        return RenderCacheResult(
            cache=None,
            source=RenderCacheSource.INVALID_EXISTING_CACHE,
            validation=validation,
            exact=False,
        )

    def generate_cache(self, request: RenderCacheRequest) -> RenderCacheResult:
        """Generate a Python render cache when text parameters are available."""

        if request.text_params is None:
            return RenderCacheResult(
                cache=None,
                source=RenderCacheSource.MISSING,
                validation=RenderCacheValidation(
                    valid=False,
                    reasons=["missing_text_params"],
                ),
                exact=False,
            )

        cache = generate_render_cache_from_text_params(
            request.text_params,
            embedded_fonts=request.embedded_fonts,
        )
        reasons: list[str] = []
        warnings = ["python_generated_cache_not_kicad_exact"]
        if not cache.polygons:
            reasons.append("empty_cache")
        if cache.text != request.text:
            reasons.append("resolved_text_mismatch")
        if request.angle is not None and not isclose(
            cache.angle,
            request.angle,
            abs_tol=request.angle_tolerance,
        ):
            reasons.append("angle_mismatch")

        return RenderCacheResult(
            cache=cache if not reasons else None,
            source=RenderCacheSource.PYTHON_GENERATED_CACHE,
            validation=RenderCacheValidation(
                valid=not reasons,
                reasons=reasons,
                warnings=warnings,
            ),
            exact=False,
        )


def _trim_repeated_closing_point(points: list[tuple[float, float]]) -> list[tuple[float, float]]:
    if len(points) > 1 and points[0] == points[-1]:
        return points[:-1]
    return points


def _point_in_polygon(point: tuple[float, float], polygon: list[tuple[float, float]]) -> bool:
    x, y = point
    inside = False
    count = len(polygon)
    for index in range(count):
        x1, y1 = polygon[index]
        x2, y2 = polygon[(index + 1) % count]
        if (y1 > y) != (y2 > y):
            x_intersect = (x2 - x1) * (y - y1) / (y2 - y1) + x1
            if x < x_intersect:
                inside = not inside
    return inside


def _signed_area(points: list[tuple[float, float]]) -> float:
    total = 0.0
    count = len(points)
    for index in range(count):
        x1, y1 = points[index]
        x2, y2 = points[(index + 1) % count]
        total += x1 * y2 - x2 * y1
    return total / 2.0


def _fracture_seam_index(points: list[tuple[float, float]]) -> int:
    """Approximate the ring seam `Fracture()`'s Clipper2 `Simplify()` stores.

    Simplified rings start just past the exit of a min-y run: the maximal
    stretch of consecutive stored points at the ring's minimum y.  The seam
    lands at the min-y top that closes the ring LAST in Clipper2's sweep:
    `DoTopOfScanbeam` handles single-point maxima left-to-right but defers
    horizontal maxima onto a LIFO stack processed right-to-left afterwards.
    So a multi-point (horizontal) run always closes after every single-point
    run, and among horizontal runs the leftmost closes last (Wavenumber 'W'),
    while among single-point runs the rightmost closes last (tiny_tapeout
    FreeMono 'P').
    """

    count = len(points)
    min_y = min(point[1] for point in points)
    runs = _min_y_runs(points, min_y)
    if not runs:
        # Every stored point sits at min y; keep the smallest-x anchor.
        return min(range(count), key=lambda index: points[index][0])
    def run_min_x(run: tuple[int, int]) -> float:
        index, x = run[0], points[run[0]][0]
        while index != run[1]:
            index = (index + 1) % count
            x = min(x, points[index][0])
        return x

    horizontal = [run for run in runs if run[1] != run[0]]
    if horizontal:
        chosen = min(horizontal, key=run_min_x)
    else:
        chosen = max(runs, key=lambda run: points[run[0]][0])
    return (chosen[1] + 1) % count


def _min_y_runs(
    points: list[tuple[float, float]],
    min_y: float,
) -> list[tuple[int, int]]:
    """Maximal cyclic (start, end) stretches of consecutive min-y points."""

    count = len(points)
    runs: list[tuple[int, int]] = []
    for index in range(count):
        if points[index][1] != min_y or points[(index - 1) % count][1] == min_y:
            continue
        end = index
        while points[(end + 1) % count][1] == min_y:
            end = (end + 1) % count
        runs.append((index, end))
    return runs


def _rotate_exterior_for_fracture(
    points: list[tuple[float, float]],
) -> list[tuple[float, float]]:
    """Approximate KiCad's pre-fracture simplify vertex ordering.

    `SHAPE_POLY_SET::Fracture()` simplifies holed glyph polygons before
    bridging holes, restarting every ring at its Simplify seam.
    """

    if not points:
        return points
    start = _fracture_seam_index(points)
    return points[start:] + points[:start]


def _leftmost_index(points: list[tuple[float, float]]) -> int:
    """Approximate KiCad's hole bridge-vertex selection.

    `fractureSingleCacheFriendly()` bridges each hole at the first stored
    point whose x equals the hole's minimum (strict `<` scan, no tie-break).
    The stored order comes from `Fracture()`'s Clipper2 `Simplify()`, so the
    scan starts at the ring's Simplify seam and returns the first min-x
    point it reaches in stored traversal order.
    """

    count = len(points)
    seam = _fracture_seam_index(points)
    x_min = min(point[0] for point in points)
    for offset in range(count):
        index = (seam + offset) % count
        if points[index][0] == x_min:
            return index
    raise AssertionError("hole ring has no min-x point")


def _edge_matches_y(
    p1: tuple[float, float],
    p2: tuple[float, float],
    y: float,
) -> bool:
    return (y >= p1[1] or y >= p2[1]) and (y <= p1[1] or y <= p2[1])


def _edge_x_intersect(
    p1: tuple[float, float],
    p2: tuple[float, float],
    y: float,
) -> float:
    if p1[1] == p2[1]:
        return max(p1[0], p2[0])
    return p1[0] + (p2[0] - p1[0]) * (y - p1[1]) / (p2[1] - p1[1])


def _fracture_exterior_with_holes(
    exterior: list[tuple[float, float]],
    holes: list[list[tuple[float, float]]],
) -> list[tuple[float, float]]:
    """Port the cache-friendly KiCad hole-fracture path for glyph outlines."""

    if not holes:
        return exterior

    paths = [_rotate_exterior_for_fracture(exterior)] + holes
    path_infos: list[dict[str, int | float]] = []
    edge_count = 0
    edges: list[dict[str, Any]] = []

    for path_index, path in enumerate(paths):
        leftmost = _leftmost_index(path)
        x_min = min(point[0] for point in path)
        y_min = min(point[1] for point in path)
        path_infos.append(
            {
                "path_index": path_index,
                "leftmost": leftmost,
                "x": x_min,
                "y_or_bridge": y_min,
                "provoking": edge_count,
            }
        )
        edge_count += len(path)
        if path_index > 0:
            edge_count += 3

    path_infos[1:] = sorted(path_infos[1:], key=lambda item: (item["x"], item["y_or_bridge"]))

    edge_index = 0
    for info in path_infos:
        path = paths[int(info["path_index"])]
        provoking_edge = edge_index
        info["provoking"] = provoking_edge

        for point_index, point in enumerate(path):
            edges.append(
                {
                    "p1": point,
                    "p2": path[(point_index + 1) % len(path)],
                    "next": edge_index + 1 if point_index < len(path) - 1 else provoking_edge,
                }
            )
            edge_index += 1

        if provoking_edge != 0:
            info["y_or_bridge"] = edge_index
            edges.extend([{}, {}, {}])
            edge_index += 3

    for info in path_infos[1:]:
        provoking = int(info["provoking"])
        edge_index = provoking + int(info["leftmost"])
        bridge_index = int(info["y_or_bridge"])
        edge = edges[edge_index]
        x, y = edge["p1"]
        nearest_index: Optional[int] = None
        nearest_x = 0.0
        min_dist = float("inf")

        for candidate_index in range(provoking):
            candidate = edges[candidate_index]
            if "p1" not in candidate or not _edge_matches_y(candidate["p1"], candidate["p2"], y):
                continue

            x_intersect = _edge_x_intersect(candidate["p1"], candidate["p2"], y)
            dist = x - x_intersect
            if dist >= 0.0 and dist < min_dist:
                min_dist = dist
                nearest_x = x_intersect
                nearest_index = candidate_index

        if nearest_index is None:
            continue

        split_point = (nearest_x, y)
        nearest = edges[nearest_index]
        edges[bridge_index] = {
            "p1": split_point,
            "p2": edge["p1"],
            "next": edge_index,
        }
        edges[bridge_index + 1] = {
            "p1": edge["p1"],
            "p2": split_point,
            "next": bridge_index + 2,
        }
        edges[bridge_index + 2] = {
            "p1": split_point,
            "p2": nearest["p2"],
            "next": nearest["next"],
        }
        nearest["p2"] = split_point
        nearest["next"] = bridge_index

        last_index = edge_index
        while edges[last_index]["next"] != edge_index:
            last_index = int(edges[last_index]["next"])
        edges[last_index]["next"] = bridge_index + 1

    fractured: list[tuple[float, float]] = []
    current_index = 0
    while True:
        edge = edges[current_index]
        # KiCad assembles the fractured ring through SHAPE_LINE_CHAIN::Append,
        # which drops points equal to the last one; a bridge split point can
        # coincide with the hole's bridge vertex at small glyph sizes.
        if not fractured or fractured[-1] != edge["p1"]:
            fractured.append(edge["p1"])
        next_index = int(edge["next"])
        if next_index == 0:
            break
        current_index = next_index

    return fractured


def _clipper_simplify_order_key(
    points: list[tuple[float, float]],
) -> tuple[float, float]:
    """Approximate Clipper2's `LocMinSorter` outer output order.

    KiCad's `Fracture()` runs `Simplify()` per glyph `SHAPE_POLY_SET`, and
    Clipper2's sweep emits each disjoint outer at its bottom-most local
    minimum: bottom-most first (largest y in the y-down cache frame), ties by
    smallest x, otherwise stable in input order.
    """

    max_y = max(point[1] for point in points)
    min_x = min(point[0] for point in points if point[1] == max_y)
    return (-max_y, min_x)


def _fracture_contour_group(
    contours: list[list[tuple[float, float]]],
) -> list[list[tuple[float, float]]]:
    """Fracture one glyph's contours with KiCad's holed-glyph normalization.

    KiCad only calls `Fracture()` when the glyph polygon set has holes, and
    `Fracture()` runs Clipper2 `Simplify()` over the whole set first.  A holed
    group therefore gets every ring's seam rewritten (including hole-free
    outers) and its outers reordered; a hole-free group keeps font order and
    ring starts untouched.
    """

    retained = [contour for contour in contours if contour]

    # KiCad's `Fracture()` classifies rings by containment, not storage order:
    # Noto faces store a glyph's hole contours before the exterior that owns
    # them.  Containment-depth parity reproduces that order-independently:
    # even depth = exterior, odd depth = hole, and a hole's parent is its
    # first stored containing exterior one level up.  Degenerate (<3 point)
    # rings — contours collapsed by cache-frame quantization — are standalone:
    # they never contain, never count as holes, and stay at their stored
    # position among the exteriors.
    containers: list[list[int]] = []
    for index, contour in enumerate(retained):
        containers.append(
            [
                other_index
                for other_index, other in enumerate(retained)
                if other_index != index
                and len(other) >= 3
                and _point_in_polygon(contour[0], other)
            ]
        )

    polygons: list[dict[str, Any]] = []
    polygon_by_contour: dict[int, dict[str, Any]] = {}
    for index, contour in enumerate(retained):
        if len(contour) < 3 or len(containers[index]) % 2 == 0:
            polygon: dict[str, Any] = {"exterior": contour, "holes": []}
            polygons.append(polygon)
            polygon_by_contour[index] = polygon

    for index, contour in enumerate(retained):
        depth = len(containers[index])
        if len(contour) < 3 or depth % 2 == 0:
            continue
        parent = next(
            (
                polygon_by_contour[other_index]
                for other_index in containers[index]
                if len(containers[other_index]) == depth - 1
            ),
            None,
        )
        if parent is None:
            polygons.append({"exterior": contour, "holes": []})
        else:
            parent["holes"].append(contour)

    group_has_holes = any(polygon["holes"] for polygon in polygons)
    if group_has_holes:
        # `Fracture()`'s Clipper2 `Simplify()` emits rings in a fixed
        # orientation (exteriors positive shoelace area in the cache frame,
        # holes negative) regardless of input winding.  Mirrored text flips
        # every ring's winding, so normalize before the seam/bridge rules;
        # hole-free groups skip Simplify and keep their stored winding.
        for polygon in polygons:
            if _signed_area(polygon["exterior"]) < 0.0:
                polygon["exterior"] = list(reversed(polygon["exterior"]))
            polygon["holes"] = [
                list(reversed(hole)) if _signed_area(hole) > 0.0 else hole
                for hole in polygon["holes"]
            ]
    fractured: list[list[tuple[float, float]]] = []
    for polygon in polygons:
        if polygon["holes"]:
            fractured.append(
                _fracture_exterior_with_holes(
                    polygon["exterior"],
                    polygon["holes"],
                )
            )
        elif group_has_holes:
            # `Fracture()`'s Clipper2 `Simplify()` drops degenerate rings, so
            # they only survive in hole-free groups that skip `Fracture()`.
            if len(polygon["exterior"]) >= 3:
                fractured.append(_rotate_exterior_for_fracture(polygon["exterior"]))
        else:
            fractured.append(polygon["exterior"])

    if group_has_holes:
        fractured.sort(key=_clipper_simplify_order_key)
    return fractured


def _fracture_render_cache_contours(
    contours: list[list[tuple[float, float]]],
    group_sizes: Optional[list[int]] = None,
) -> list[list[tuple[float, float]]]:
    """Fracture cache contours, scoped per glyph group when sizes are known.

    KiCad's saved-cache writer fractures each drawn glyph's polygon set
    independently, so hole containment, hole bridging, and the Simplify seam
    and outer reordering are all scoped to one glyph group while glyph draw
    order is preserved.  Without group sizes the whole input is one group.
    """

    if group_sizes is None:
        return _fracture_contour_group(contours)
    if sum(group_sizes) != len(contours):
        raise ValueError("contour group sizes do not partition the contours")

    fractured: list[list[tuple[float, float]]] = []
    start = 0
    for size in group_sizes:
        fractured.extend(_fracture_contour_group(contours[start:start + size]))
        start += size
    return fractured


def generate_render_cache_from_text_params(
    text_params: Any,
    *,
    renderer: Optional[Any] = None,
    embedded_fonts: Tuple[Tuple[str, bytes], ...] = (),
) -> RenderCache:
    """Generate a typed render cache from existing FreeType/HarfBuzz text params.

    This is the first Python generation backend.  It uses the shared
    FreeType/HarfBuzz renderer and returns non-exact provenance until the
    remaining KiCad font lookup, style, multiline, and text-box cases are
    oracle-covered.
    """

    if renderer is None:
        from .kicad_text import KiCadTextRenderer

        renderer = KiCadTextRenderer()

    for font_name, font_data in embedded_fonts:
        if hasattr(renderer, "register_embedded_font"):
            renderer.register_embedded_font(font_name, font_data)

    geometry = renderer.render(text_params)
    polygons: list[RenderCachePolygon] = []
    contours = [
        _trim_repeated_closing_point(list(contour.points))
        for contour in geometry.contours
    ]
    group_sizes = list(getattr(geometry, "contour_group_sizes", []) or []) or None
    for points in _fracture_render_cache_contours(contours, group_sizes):
        # KiCad's saved-cache writer publishes degenerate (collapsed) rings
        # as their own polygons, so keep every non-empty fractured contour.
        if points:
            polygons.append(
                RenderCachePolygon(
                    contours=[RenderCacheContour(points=points)],
                )
            )

    return RenderCache(
        text=str(getattr(text_params, "text", "")),
        angle=float(getattr(text_params, "angle", 0.0)),
        polygons=polygons,
    )


def _decompress_embedded_payload(data: bytes) -> bytes:
    try:
        import zstandard as _zstandard
    except ImportError:
        return data
    try:
        return _zstandard.ZstdDecompressor().decompress(data)
    except _zstandard.ZstdError:
        return data


def _embedded_font_payloads(*containers: Any) -> Tuple[Tuple[str, bytes], ...]:
    fonts: list[Tuple[str, bytes]] = []
    seen: set[Tuple[str, int]] = set()
    for container in containers:
        for embedded_file in getattr(container, "embedded_files", []) or []:
            if str(getattr(embedded_file, "file_type", "")).lower() != "font":
                continue
            name = str(getattr(embedded_file, "name", "") or "")
            data_text = str(getattr(embedded_file, "data", "") or "")
            if not name or not data_text:
                continue
            encoded = data_text.replace("\n", "").replace("\r", "").strip("|")
            try:
                compressed = base64.b64decode(encoded)
            except (binascii.Error, ValueError):
                continue
            payload = _decompress_embedded_payload(compressed)
            key = (name.casefold(), hash(payload))
            if key in seen:
                continue
            seen.add(key)
            fonts.append((name, payload))
    return tuple(fonts)


def _font_face_for(text_object: Any) -> Optional[str]:
    effects = getattr(text_object, "effects", None)
    font = getattr(effects, "font", None)
    return getattr(font, "face", None)


def render_cache_request_for_board_text(
    text_object: Any,
    board: Any = None,
    *,
    object_type: str = "",
    object_path: str = "",
    include_text_params: bool = False,
) -> RenderCacheRequest:
    """Build a render-cache request for board-level text or text boxes."""

    raw_text = str(getattr(text_object, "text", ""))
    resolved_text = substitute_text_variables(raw_text, board_text_variables(board))
    if hasattr(text_object, "render_cache_text"):
        resolved_text = text_object.render_cache_text(resolved_text)
    angle = getattr(text_object, "at_angle", getattr(text_object, "angle", None))
    text_params = None
    if include_text_params and hasattr(text_object, "to_text_params"):
        try:
            text_params = text_object.to_text_params(text=resolved_text)
        except TypeError:
            text_params = text_object.to_text_params()
            text_params.text = resolved_text
    return RenderCacheRequest(
        text=resolved_text,
        angle=angle,
        render_cache=getattr(text_object, "render_cache", None),
        object_type=object_type or type(text_object).__name__,
        object_path=object_path,
        font_face=_font_face_for(text_object),
        text_params=text_params,
        embedded_fonts=_embedded_font_payloads(board),
    )


def _resolve_fp_text_value(fp_text: Any, variables: Mapping[str, str]) -> str:
    raw_text = str(getattr(fp_text, "text", ""))
    text_type = getattr(fp_text, "text_type", "")
    if text_type == "reference":
        return variables.get("Reference", raw_text)
    if text_type == "value":
        return variables.get("Value", raw_text)
    return raw_text


def render_cache_request_for_footprint_text(
    fp_text: Any,
    footprint: Any = None,
    *,
    object_path: str = "",
    include_text_params: bool = False,
) -> RenderCacheRequest:
    """Build a render-cache request for footprint `fp_text`.

    Locked (keep-upright) text folds its absolute angle into (-90, 90] inside
    `to_text_params` via `keep_upright_draw_angle`; `(unlocked yes)` passes
    the angle through unchanged.
    """

    variables = footprint_text_variables(footprint)
    resolved_text = substitute_text_variables(
        _resolve_fp_text_value(fp_text, variables),
        variables,
    )
    text_params = None
    if include_text_params and hasattr(fp_text, "to_text_params"):
        try:
            text_params = fp_text.to_text_params(footprint=footprint)
        except TypeError:
            text_params = fp_text.to_text_params()
        text_params.text = resolved_text
    return RenderCacheRequest(
        text=resolved_text,
        angle=None,
        render_cache=getattr(fp_text, "render_cache", None),
        object_type="fp_text",
        object_path=object_path,
        font_face=_font_face_for(fp_text),
        text_params=text_params,
        embedded_fonts=_embedded_font_payloads(footprint),
    )


def render_cache_request_for_footprint_property(
    prop: Any,
    footprint: Any = None,
    *,
    object_path: str = "",
    include_text_params: bool = False,
) -> RenderCacheRequest:
    """Build a render-cache request for footprint property text."""

    variables = footprint_text_variables(footprint)
    resolved_text = substitute_text_variables(str(getattr(prop, "value", "")), variables)
    text_params = None
    if include_text_params and hasattr(prop, "to_text_params"):
        try:
            text_params = prop.to_text_params(footprint=footprint)
        except TypeError:
            text_params = prop.to_text_params()
        text_params.text = resolved_text
    return RenderCacheRequest(
        text=resolved_text,
        angle=None,
        render_cache=getattr(prop, "render_cache", None),
        object_type="property",
        object_path=object_path,
        font_face=_font_face_for(prop),
        text_params=text_params,
        embedded_fonts=_embedded_font_payloads(footprint),
    )


def render_cache_request_for_footprint_text_box(
    text_box: Any,
    footprint: Any = None,
    *,
    object_path: str = "",
    include_text_params: bool = False,
) -> RenderCacheRequest:
    """Build a render-cache request for footprint-local text boxes."""

    variables = footprint_text_variables(footprint)
    resolved_text = substitute_text_variables(str(getattr(text_box, "text", "")), variables)
    if hasattr(text_box, "render_cache_text"):
        try:
            resolved_text = text_box.render_cache_text(resolved_text, footprint=footprint)
        except TypeError:
            resolved_text = text_box.render_cache_text(resolved_text)
    text_params = None
    if include_text_params and hasattr(text_box, "to_text_params"):
        try:
            text_params = text_box.to_text_params(text=resolved_text, footprint=footprint)
        except TypeError:
            text_params = text_box.to_text_params()
            text_params.text = resolved_text
    return RenderCacheRequest(
        text=resolved_text,
        angle=None,
        render_cache=getattr(text_box, "render_cache", None),
        object_type="fp_text_box",
        object_path=object_path,
        font_face=_font_face_for(text_box),
        text_params=text_params,
        embedded_fonts=_embedded_font_payloads(footprint),
    )


def render_cache_request_for_table_cell(
    cell: Any,
    table: Any = None,
    board: Any = None,
    footprint: Any = None,
    *,
    object_path: str = "",
    include_text_params: bool = False,
) -> RenderCacheRequest:
    """Build a render-cache request for a PCB `table_cell`."""

    variables = {}
    variables.update(board_text_variables(board))
    variables.update(footprint_text_variables(footprint))
    variables.update(table_cell_text_variables(cell, table))
    resolved_text = substitute_text_variables(str(getattr(cell, "text", "")), variables)
    if hasattr(cell, "render_cache_text"):
        try:
            resolved_text = cell.render_cache_text(resolved_text, footprint=footprint)
        except TypeError:
            resolved_text = cell.render_cache_text(resolved_text)
    text_params = None
    if include_text_params and hasattr(cell, "to_text_params"):
        try:
            text_params = cell.to_text_params(text=resolved_text, footprint=footprint)
        except TypeError:
            text_params = cell.to_text_params()
            text_params.text = resolved_text
    return RenderCacheRequest(
        text=resolved_text,
        angle=None,
        render_cache=getattr(cell, "render_cache", None),
        object_type="table_cell",
        object_path=object_path,
        font_face=_font_face_for(cell),
        text_params=text_params,
        embedded_fonts=_embedded_font_payloads(board, footprint),
    )


def render_cache_request_for_dimension_text(
    dimension: Any,
    board: Any = None,
    *,
    object_path: str = "",
    include_text_params: bool = False,
) -> RenderCacheRequest:
    """Build a render-cache request for a dimension's nested `gr_text`."""

    resolver = getattr(dimension, "resolved_gr_text", None)
    text_object = resolver() if callable(resolver) else getattr(dimension, "gr_text", None)
    if text_object is None:
        return RenderCacheRequest(
            text="",
            angle=None,
            render_cache=None,
            object_type="dimension",
            object_path=object_path,
        )

    return render_cache_request_for_board_text(
        text_object,
        board,
        object_type="dimension",
        object_path=object_path,
        include_text_params=include_text_params,
    )


def ensure_render_cache(request: RenderCacheRequest) -> RenderCacheResult:
    """Validate an existing cache through the default resolver."""

    return RenderCacheResolver().ensure_cache(request)


def render_cache_exterior_polygons(
    text: str,
    render_cache: Optional[RenderCache],
    *,
    angle: Optional[float] = None,
) -> list[list[tuple[float, float]]]:
    """Return exterior contours from a usable existing render cache.

    Current direct PCB SVG fill batching is flat-polygon based, so this helper
    returns exterior contours only.  The typed cache still preserves hole
    contours for future hole-aware consumers.
    """

    result = ensure_render_cache(
        RenderCacheRequest(text=text, angle=angle, render_cache=render_cache)
    )
    if not result.usable or result.cache is None:
        return []

    polygons: list[list[tuple[float, float]]] = []
    for polygon in result.cache.polygons:
        if len(polygon.points) >= 3:
            polygons.append(list(polygon.points))
    return polygons


__all__ = [
    "RenderCacheRequest",
    "RenderCacheResolver",
    "RenderCacheResult",
    "RenderCacheSource",
    "RenderCacheValidation",
    "board_text_variables",
    "ensure_render_cache",
    "footprint_text_variables",
    "generate_render_cache_from_text_params",
    "render_cache_exterior_polygons",
    "render_cache_request_for_board_text",
    "render_cache_request_for_dimension_text",
    "render_cache_request_for_footprint_property",
    "render_cache_request_for_footprint_text",
    "render_cache_request_for_footprint_text_box",
    "render_cache_request_for_table_cell",
    "substitute_text_variables",
    "table_cell_text_variables",
]

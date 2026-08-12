"""
Footprint filters for KiCad .kicad_mod files.

These filters operate on parsed s-expressions and modify footprint data.
"""
import base64
import copy
import io
import logging
import math
from typing import Any, cast

import numpy as np
import trimesh
import trimesh.transformations as tf
import zstandard as zstd
from numpy import sign
from shapely.geometry import MultiPolygon, Polygon
from shapely.ops import unary_union

from .kicad_base import find_element
from .kicad_footprint_normalize import normalize_unsafe_footprint_pad_sizes
from .kicad_sexpr import QuotedString

log = logging.getLogger(__name__)


def get_footprint_side(s_expression: list) -> str:
    """
    Detects which side of the PCB a footprint is on based on its layer attribute.

    In KiCad PCB files, embedded footprints have a (layer "F.Cu") or (layer "B.Cu")
    attribute indicating which side of the board they're placed on.

    Args:
        s_expression: The footprint s-expression list

    Returns:
        "front" if on F.Cu (top side) or no layer found
        "back" if on B.Cu (bottom side)
    """
    for item in s_expression:
        if isinstance(item, list) and len(item) >= 2 and item[0] == 'layer':
            layer_name = str(item[1]).strip('"')
            if layer_name == 'B.Cu':
                return "back"
            elif layer_name == 'F.Cu':
                return "front"
    # Default to front side if no layer attribute found (standalone .kicad_mod files)
    return "front"


def get_fab_layer_for_side(side: str) -> str:
    """
    Returns the appropriate fab layer name based on footprint side.

    Args:
        side: "front" or "back"

    Returns:
        "F.Fab" for front side, "B.Fab" for back side
    """
    return "B.Fab" if side == "back" else "F.Fab"


def add_reference_text_to_fab(s_expression: list, center_position: list, hull_shortest_side: float, fab_layer: str | None = None) -> list:
    """
    Common helper function to add a reference text string to the fab layer.

    Args:
        s_expression: The s-expression list to modify
        center_position: [x, y] coordinates for the reference text center
        hull_shortest_side: The shortest side of the bounding box (used for font sizing)
        fab_layer: Layer to add the reference text to (default: auto-detect from footprint side)

    Returns:
        Modified s-expression with reference text added
    """
    # Auto-detect fab layer if not specified
    if fab_layer is None:
        side = get_footprint_side(s_expression)
        fab_layer = get_fab_layer_for_side(side)
    # Find the reference value from the s-expression
    reference = None
    part_center = None

    for p in s_expression:
        if isinstance(p, list) and len(p) > 0:
            # Directly check for property Reference
            if p[0] == 'property' and len(p) >= 3 and p[1] == QuotedString('Reference'):
                reference = p[2]
                # Try to get the center from the 'at' field in this property
                for item in p:
                    if isinstance(item, list) and item[0] == 'at' and len(item) >= 3:
                        part_center = [float(item[1]), float(item[2])]
            # Fallback: global at
            if part_center is None and p[0] == 'at' and len(p) >= 3:
                part_center = [float(p[1]), float(p[2])]

    if reference is not None:
        # Calculate appropriate font size based on bounding box size
        size = min(0.25 * hull_shortest_side, 1.0)
        thickness = min(hull_shortest_side / 10, 0.5)

        ref_string = [
            'fp_text', 'reference', reference,
            ['at', center_position[0], center_position[1]],
            ['layer', QuotedString(fab_layer)],
            ['effects', ['font', ['size', size, size], ['thickness', thickness]]],
        ]

        log.info(f"- Adding reference string (fp_text reference \"{reference}\") at [{center_position[0]:.4f}, {center_position[1]:.4f}] on {fab_layer}")
        log.info("Success: Added reference string.")

        # Add the reference text
        s_expression.append(ref_string)

        # Move the reference text to just after the first pad (if any pads exist)
        for i, p in enumerate(s_expression):
            if isinstance(p, list) and len(p) > 0 and p[0] == 'pad':
                # Move the fp_text reference to the position after the first pad
                s_expression.insert(i, s_expression.pop())
                break
    else:
        log.error("Error: Could not find reference. Skipping reference string addition.")

    return s_expression


def fp_filter__clean_layers(unfiltered_s_expression: Any, layers: list[str] | None = None) -> Any:
    """
    Removes all objects on specified layers from a footprint s-expression.

    Args:
        unfiltered_s_expression: The parsed s-expression list
        layers: List of layer names to clean. Supports exact matches and prefix matches.
                Default: ["F.Fab", "B.Fab", "User."]

    Layer matching:
        - Exact match: "F.Fab" matches only "F.Fab"
        - Prefix match (ends with .): "User." matches "User.1", "User.Drawings", etc.
    """
    if layers is None:
        layers = ["F.Fab", "B.Fab", "User.", "Eco1.User", "Eco2.User"]

    log.info(f"\nRunning fp_filter__clean_layers(layers={layers})...\n")

    def layer_matches(layer_name, patterns):
        """Check if layer_name matches any pattern in the list."""
        if layer_name is None:
            return False
        # Normalize: strip quotes if present
        layer_str = str(layer_name).strip('"')
        for pattern in patterns:
            if pattern.endswith('.'):
                # Prefix match
                if layer_str.startswith(pattern[:-1]):
                    return True
            else:
                # Exact match
                if layer_str == pattern:
                    return True
        return False

    layers_removed = 0
    # Walk in reverse so removals don't disturb iteration.
    for i in range(len(unfiltered_s_expression) - 1, -1, -1):
        p = unfiltered_s_expression[i]
        if not isinstance(p, list):
            continue
        layer_elem = find_element(p, 'layer')
        layer_name = layer_elem[1] if (layer_elem is not None and len(layer_elem) >= 2) else None
        if layer_matches(layer_name, layers):
            log.info(f"- Removing object ({p[0]}) on layer {layer_name}")
            unfiltered_s_expression.pop(i)
            layers_removed += 1
            continue
        # Clear property "Value" to empty string (in place).
        if (len(p) >= 3 and p[0] == 'property' and p[1] == QuotedString('Value')):
            log.info(f"- Setting property \"Value\" to empty string for {p[2]}")
            log.info("Success: Cleared property \"Value\".")
            p[2] = QuotedString(' ')

    if layers_removed > 0:
        log.info(f"Success: {layers_removed} objects removed from layers matching {layers}.")
    else:
        log.warning(f"Warning: No objects found on layers matching {layers}.")


    log.info("\nDone! S-expression has been filtered...")
    return unfiltered_s_expression


def fp_filter__clean_fab(unfiltered_s_expression: Any) -> Any:
    """Backward compatible wrapper - cleans F.Fab, B.Fab, User.*, Eco layers."""
    return fp_filter__clean_layers(unfiltered_s_expression)


def _orthogonal_convex_quadrants(points: Any) -> list[list[Any]]:
    center = np.mean(points, axis=0)
    center_x, center_y = center
    quadrants: list[list[Any]] = [[], [], [], []]
    for point in points:
        x, y = point
        if x >= center_x and y >= center_y:
            quadrants[0].append(point)
        elif x < center_x and y >= center_y:
            quadrants[1].append(point)
        elif x < center_x and y < center_y:
            quadrants[2].append(point)
        else:
            quadrants[3].append(point)
    return quadrants


def _filter_orthogonal_convex_quadrants(
    quadrants: list[list[Any]],
    points: Any,
) -> list[Any]:
    q1, q2, q3, q4 = quadrants
    q1 = [
        p for p in q1
        if not any(p_check[0] > p[0] and p_check[1] > p[1] for p_check in points)
    ]
    q2 = [
        p for p in q2
        if not any(p_check[0] < p[0] and p_check[1] > p[1] for p_check in points)
    ]
    q3 = [
        p for p in q3
        if not any(p_check[0] < p[0] and p_check[1] < p[1] for p_check in points)
    ]
    q4 = [
        p for p in q4
        if not any(p_check[0] > p[0] and p_check[1] < p[1] for p_check in points)
    ]
    return q1 + q2 + q3 + q4


def _orthogonal_midpoint(start: Any, end: Any, center: Any) -> Any:
    intersection_p1 = [end[1], start[0]]
    intersection_p2 = [start[1], end[0]]
    distance_p1 = math.sqrt(
        (intersection_p1[0] - start[0]) ** 2
        + (intersection_p1[1] - start[1]) ** 2
    )
    distance_p2 = math.sqrt(
        (intersection_p2[0] - end[0]) ** 2
        + (intersection_p2[1] - end[1]) ** 2
    )
    diagonal_center_match = (
        abs(start[0]) < abs(start[1])
        and (
            (start[0] > center[0] and start[1] > center[1])
            or (start[0] < center[0] and start[1] < center[1])
        )
    )
    if diagonal_center_match:
        horizontal_first = False
    elif sign(start[0]) == sign(start[1]):
        horizontal_first = distance_p1 >= distance_p2
    else:
        horizontal_first = distance_p1 > distance_p2
    return np.array([end[0], start[1]]) if horizontal_first else np.array([start[0], end[1]])


def _orthogonal_convex_hull(points: Any) -> Any:
    hull_points = np.array(_filter_orthogonal_convex_quadrants(
        _orthogonal_convex_quadrants(points),
        points,
    ))
    center = np.mean(hull_points, axis=0)
    sorted_hull = np.array(
        sorted(
            hull_points,
            key=lambda p: np.arctan2(p[1] - center[1], p[0] - center[0]),
            reverse=True,
        )
    )
    ortho_path = []
    for i, start in enumerate(sorted_hull):
        end = sorted_hull[(i + 1) % len(sorted_hull)]
        mid = _orthogonal_midpoint(start, end, center)
        ortho_path.append(start)
        if not np.array_equal(mid, start):
            ortho_path.append(mid)
        if not np.array_equal(end, mid):
            ortho_path.append(end)
    return np.array(ortho_path)


def _sexpr_float_pair(item: list[Any], *, start_index: int = 1) -> list[float]:
    return [float(item[start_index]), float(item[start_index + 1])]


def _rotate_fab_bbox_point(
    point: list[float],
    *,
    center: list[float],
    rotation_deg: float,
) -> list[float]:
    rotation_rad = np.radians(rotation_deg + 90)
    cos_theta = np.cos(rotation_rad)
    sin_theta = np.sin(rotation_rad)
    x, y = point
    x_new = cos_theta * (x - center[0]) - sin_theta * (y - center[1]) + center[0]
    y_new = sin_theta * (x - center[0]) + cos_theta * (y - center[1]) + center[1]
    return [float(f"{x_new:.4f}"), float(f"{y_new:.4f}")]


def _pad_outline_points_for_fab_bbox(pad: Any, bb_line_width: float) -> list[list[float]]:
    if not isinstance(pad, list) or len(pad) == 0 or pad[0] != 'pad':
        return []
    pad_size: list[float] | None = None
    pad_center: list[float] | None = None
    pad_rotation = 0.0
    for item in pad:
        if isinstance(item, list) and len(item) >= 3 and item[0] == 'size':
            pad_size = _sexpr_float_pair(item)
        if isinstance(item, list) and len(item) >= 3 and item[0] == 'at':
            pad_center = _sexpr_float_pair(item)
            if len(item) > 3 and isinstance(item[3], (int, float)):
                pad_rotation = float(item[3])
    if pad_size is None or pad_center is None:
        return []

    new_size = [pad_size[1] + (3 * bb_line_width), pad_size[0] + (3 * bb_line_width)]
    left = float(f"{(pad_center[0] - (new_size[0]) / 2):.4f}")
    right = float(f"{(pad_center[0] + (new_size[0]) / 2):.4f}")
    top = float(f"{(pad_center[1] - (new_size[1]) / 2):.4f}")
    bottom = float(f"{(pad_center[1] + (new_size[1]) / 2):.4f}")
    corners = [[left, top], [right, top], [right, bottom], [left, bottom]]
    if pad_rotation == 0:
        return corners
    return [
        _rotate_fab_bbox_point(corner, center=pad_center, rotation_deg=pad_rotation)
        for corner in corners
    ]


def _silkscreen_line_points_for_fab_bbox(item: Any) -> list[list[float]]:
    if not isinstance(item, list) or len(item) == 0 or item[0] != 'fp_line':
        return []
    if not any(
        isinstance(sub_item, list)
        and len(sub_item) >= 2
        and sub_item[0] == 'layer'
        and sub_item[1] == 'F.SilkS'
        for sub_item in item
    ):
        return []

    start_point = None
    end_point = None
    scaler = 1.1
    for sub_item in item:
        if not isinstance(sub_item, list) or len(sub_item) < 3:
            continue
        if sub_item[0] == 'start':
            start_point = [float(sub_item[1]) * scaler, float(sub_item[2]) * scaler]
        elif sub_item[0] == 'end':
            end_point = [float(sub_item[1]) * scaler, float(sub_item[2]) * scaler]
    if start_point is None or end_point is None:
        return []
    return [start_point, end_point]


def _collect_fab_bbox_points(
    unfiltered_s_expression: Any,
    *,
    bb_line_width: float,
) -> list[list[float]]:
    points: list[list[float]] = []
    for item in unfiltered_s_expression:
        points.extend(_pad_outline_points_for_fab_bbox(item, bb_line_width))
        points.extend(_silkscreen_line_points_for_fab_bbox(item))
    return points


def _append_fab_bbox_lines(
    unfiltered_s_expression: Any,
    *,
    hull_points: Any,
    fab_layer: str,
    bb_line_width: float,
) -> None:
    for index, start_point in enumerate(hull_points):
        end_point = hull_points[(index + 1) % len(hull_points)]
        unfiltered_s_expression.append([
            'fp_line',
            ['start', start_point[0], start_point[1]],
            ['end', end_point[0], end_point[1]],
            ['stroke', ['width', bb_line_width], ['type', 'solid'], ['color', 0, 0, 0, 1]],
            ['layer', QuotedString(fab_layer)],
            ['uuid', QuotedString('')],
        ])


def _fab_bbox_center_and_shortest_side(hull_points: Any) -> tuple[list[float], float]:
    center_x = np.mean(hull_points[:, 0])
    center_y = np.mean(hull_points[:, 1])
    hull_height = np.max(hull_points[:, 1]) - np.min(hull_points[:, 1])
    hull_width = np.max(hull_points[:, 0]) - np.min(hull_points[:, 0])
    return [center_x, center_y], min(hull_height, hull_width)


def fp_filter__add_fab_bounding_orthogonal_convex(unfiltered_s_expression: Any) -> Any:
    """
    - Auto generates a new convex hull bounding box that is 75% larger than the pads on the fab layer.
    - Adds a "REF**" string to the center of the part on the fab layer.
    - Auto-detects if footprint is on front or back side and uses appropriate fab layer (F.Fab or B.Fab).
    """

    log.info("\nRunning fp_filter__add_fab_bounding_orthogonal_convex()...\n")

    # Detect footprint side and determine appropriate fab layer
    side = get_footprint_side(unfiltered_s_expression)
    fab_layer = get_fab_layer_for_side(side)
    log.info(f"- Footprint is on {side} side, using {fab_layer} layer")

    bb_line_width = .127

    point_collection = _collect_fab_bbox_points(
        unfiltered_s_expression,
        bb_line_width=bb_line_width,
    )

    # Using the convex hull to create a bounding box around all pads
    if len(point_collection) > 1:
        points_array = np.array(point_collection)
        hull_points = _orthogonal_convex_hull(points_array)
        bounding_box_center, hull_shortest_side = _fab_bbox_center_and_shortest_side(
            hull_points
        )
        _append_fab_bbox_lines(
            unfiltered_s_expression,
            hull_points=hull_points,
            fab_layer=fab_layer,
            bb_line_width=bb_line_width,
        )
    else:
        # There are too few points so defaults will be defined.
        hull_shortest_side = 5
        bounding_box_center = [0, 0]
        log.warning("Warning: Footprint does not have enough points to define a convex hull.\n Placing the reference string in the center.")


    log.info(f"- Adding bounding box around pads with {len(point_collection)} points on {fab_layer}.")
    log.info("Success: Added bounding box around pads.")

    # Use the common function to add reference text to the appropriate fab layer
    add_reference_text_to_fab(unfiltered_s_expression, bounding_box_center, hull_shortest_side, fab_layer)
    log.info("\nDone! S-expression has been filtered...")

    return unfiltered_s_expression


def fp_filter__normalized_embedded_model_naming(unfiltered_s_expression: Any) -> Any:
    """Keep one real embedded STEP model and give it a footprint-scoped name.

    Altium conversions can emit generic embedded names that collide when
    footprints are collected into a library.  They can also emit several
    component bodies even though the library contract permits one canonical
    STEP model.  Select a real STEP payload (preferring a name that already
    matches the footprint), rename that payload and its model reference, and
    discard the other embedded model bodies.

    A filename extension is not evidence that the payload is STEP.  Genuine
    SolidWorks, Parasolid, or unknown payloads are dropped with a warning; this
    filter does not pretend to convert proprietary model formats.
    """
    log.info("\nRunning fp_filter__normalized_embedded_model_naming()...\n")

    footprint_name = str(unfiltered_s_expression[1])

    # For PCB-embedded footprints, the name is "library:footprint"
    # Extract just the footprint name (after the colon)
    if ':' in footprint_name:
        footprint_name_only = footprint_name.split(':')[-1]
        log.info(f"  - Detected PCB footprint format, using name: {footprint_name_only}")
        footprint_name = footprint_name_only

    embedded_files_section = find_element(unfiltered_s_expression, 'embedded_files')
    if embedded_files_section is None:
        log.info("Info: No embedded_files section found (normal for PCB-embedded footprints).")
        return unfiltered_s_expression

    embedded_models = [
        item
        for item in embedded_files_section[1:]
        if _embedded_file_is_model(item)
    ]
    model_sections = [
        item
        for item in unfiltered_s_expression
        if (
            isinstance(item, list)
            and len(item) > 1
            and item[0] == 'model'
            and str(item[1]).startswith('kicad-embed://')
        )
    ]
    files_by_name: dict[str, list[list[Any]]] = {}
    for file_node in embedded_models:
        file_name = _embedded_file_name(file_node)
        if file_name:
            files_by_name.setdefault(file_name.casefold(), []).append(file_node)

    candidates: list[tuple[list[Any], list[Any], str]] = []
    paired_file_ids: set[int] = set()
    for model_node in model_sections:
        model_name = str(model_node[1])[len('kicad-embed://'):]
        matches = files_by_name.get(model_name.casefold(), [])
        if not matches:
            log.warning("Dropping embedded model reference with no payload: %s", model_name)
            continue
        file_node = matches.pop(0)
        paired_file_ids.add(id(file_node))
        model_format = _embedded_file_model_format(file_node)
        if model_format != 'step':
            log.warning(
                "Dropping unsupported embedded model %s (detected: %s); "
                "no conversion to STEP is available",
                model_name,
                model_format,
            )
            continue
        candidates.append((model_node, file_node, model_name))

    for file_node in embedded_models:
        if id(file_node) not in paired_file_ids:
            log.warning(
                "Dropping unreferenced embedded model payload %s (detected: %s)",
                _embedded_file_name(file_node) or "<unnamed>",
                _embedded_file_model_format(file_node),
            )

    selected: tuple[list[Any], list[Any], str] | None = None
    matching = [
        candidate
        for candidate in candidates
        if _embedded_model_stem(candidate[2]).casefold() == footprint_name.casefold()
    ]
    if matching:
        selected = matching[0]
    elif candidates:
        selected = candidates[0]

    if len(candidates) > 1:
        kept = selected[2] if selected is not None else "none"
        log.warning(
            "Footprint %s contains %d embedded STEP models; keeping %s and dropping the rest",
            footprint_name,
            len(candidates),
            kept,
        )

    selected_model = selected[0] if selected is not None else None
    selected_file = selected[1] if selected is not None else None
    canonical_name = f"{footprint_name}.STEP"
    if selected_model is not None and selected_file is not None:
        name_node = _direct_child(selected_file, 'name')
        if name_node is not None:
            name_node[1] = QuotedString(canonical_name)
        selected_model[1] = QuotedString(f"kicad-embed://{canonical_name}")

    embedded_files_section[:] = [
        embedded_files_section[0],
        *[
            item
            for item in embedded_files_section[1:]
            if not _embedded_file_is_model(item) or item is selected_file
        ],
    ]
    model_section_ids = {id(item) for item in model_sections}
    unfiltered_s_expression[:] = [
        item
        for item in unfiltered_s_expression
        if id(item) not in model_section_ids or item is selected_model
    ]
    if len(embedded_files_section) == 1:
        unfiltered_s_expression.remove(embedded_files_section)

    log.info("\nDone! S-expression has been filtered...")
    return unfiltered_s_expression


def _direct_child(node: list[Any], tag: str) -> list[Any] | None:
    return next(
        (
            item
            for item in node[1:]
            if isinstance(item, list) and item and item[0] == tag
        ),
        None,
    )


def _embedded_file_name(file_node: list[Any]) -> str:
    name_node = _direct_child(file_node, 'name')
    return str(name_node[1]) if name_node is not None and len(name_node) > 1 else ""


def _embedded_file_is_model(file_node: Any) -> bool:
    if not isinstance(file_node, list) or not file_node or file_node[0] != 'file':
        return False
    type_node = _direct_child(file_node, 'type')
    return (
        type_node is not None
        and len(type_node) > 1
        and str(type_node[1]).casefold() == 'model'
    )


def _embedded_file_model_format(file_node: list[Any]) -> str:
    data_node = _direct_child(file_node, 'data')
    if data_node is None:
        return 'unknown'
    encoded = ''.join(str(item) for item in data_node[1:])
    encoded = encoded.replace('\n', '').replace('\r', '').strip('|')
    if not encoded:
        return 'unknown'
    try:
        compressed = base64.b64decode(encoded)
        try:
            payload = zstd.ZstdDecompressor().decompress(compressed)
        except zstd.ZstdError:
            with zstd.ZstdDecompressor().stream_reader(compressed) as reader:
                payload = reader.read()
    except Exception:
        return 'unknown'

    head = payload.lstrip()[:64]
    if head.startswith(b'ISO-10303-21;'):
        return 'step'
    if head.startswith(b'\xd0\xcf\x11\xe0'):
        return 'solidworks'
    if head.startswith(b'**'):
        return 'parasolid'
    return 'unknown'


def _embedded_model_stem(name: str) -> str:
    leaf = name.replace('\\', '/').rsplit('/', 1)[-1]
    return leaf.rsplit('.', 1)[0] if '.' in leaf else leaf


def fp_filter__fix_zero_sized_pads(unfiltered_s_expression: Any) -> Any:
    """
    Compatibility wrapper for KiCad-parity direct pad-size normalization.

    If either direct/default size axis is nonpositive, both axes are pinned to
    1 um. Nested per-layer padstack size forms are left unchanged.
    """
    log.info("\nRunning fp_filter__fix_zero_sized_pads()...\n")

    result = normalize_unsafe_footprint_pad_sizes(unfiltered_s_expression)
    for change in result.changes:
        log.warning(
            "- Fixing unsafe pad %r size %s: setting size to 0.001mm",
            change.pad_name,
            change.original_size,
        )

    if result.count > 0:
        log.info("Success: Fixed %s unsafe pad size(s).", result.count)

    log.info("\nDone! S-expression has been filtered...")
    return result.expression


def fp_filter__fix_fp_text_font_to_arial(unfiltered_s_expression: Any) -> Any:
    """
    This is a filter for an s-expression file that does the following:
    - Finds all fp_text objects
    - Ensures they have (face "Arial") in their effects/font section
    - If face doesn't exist, adds it
    - If face exists but is not Arial, changes it to Arial
    """
    log.info("\nRunning fp_filter__fix_fp_text_font_to_arial()...\n")

    texts_fixed = [0]

    def ensure_arial(elem: list, object_desc: str) -> None:
        effects = find_element(elem, 'effects')
        if effects is None:
            log.info(f"- Adding effects section with Arial font to {object_desc}")
            elem.append(['effects', ['font', ['face', QuotedString('Arial')]]])
            texts_fixed[0] += 1
            return
        font = find_element(effects, 'font')
        if font is None:
            log.info(f"- Adding font section with Arial face to {object_desc}")
            effects.append(['font', ['face', QuotedString('Arial')]])
            texts_fixed[0] += 1
            return
        for k, font_item in enumerate(font):
            if isinstance(font_item, list) and len(font_item) >= 2 and font_item[0] == 'face':
                if font_item[1] != QuotedString('Arial'):
                    log.info(f"- Changing font face from {font_item[1]} to Arial for {object_desc}")
                    font[k] = ['face', QuotedString('Arial')]
                    texts_fixed[0] += 1
                return
        log.info(f"- Adding Arial font face to {object_desc}")
        font.append(['face', QuotedString('Arial')])
        texts_fixed[0] += 1

    for elem in unfiltered_s_expression:
        if not (isinstance(elem, list) and len(elem) > 0 and elem[0] in ('fp_text', 'property')):
            continue
        label = elem[1] if len(elem) > 1 else "unknown"
        ensure_arial(elem, f"{elem[0]} {label}")

    if texts_fixed[0] > 0:
        log.info(f"Success: Fixed {texts_fixed[0]} text/property font face(s) to Arial.")
    else:
        log.warning("Warning: No text/property fonts needed fixing.")

    log.info("\nDone! S-expression has been filtered...")
    return unfiltered_s_expression


def _model_transform_from_sexp(s_expr: list) -> np.ndarray:
    """Extract KiCad 3D model transform as a 4x4 matrix."""
    for item in s_expr:
        if not (isinstance(item, list) and item and item[0] == 'model'):
            continue

        offset = [0, 0, 0]
        scale = [1, 1, 1]
        rotate = [0, 0, 0]
        for sub in item[1:]:
            if not (isinstance(sub, list) and sub):
                continue
            for xyz in sub[1:]:
                if isinstance(xyz, list) and xyz[0] == 'xyz':
                    if sub[0] == 'offset':
                        offset = [float(x) for x in xyz[1:]]
                    elif sub[0] == 'scale':
                        scale = [float(x) for x in xyz[1:]]
                    elif sub[0] == 'rotate':
                        rotate = [float(x) for x in xyz[1:]]

        m_scale = tf.scale_matrix(scale[0], [0, 0, 0])
        m_scale[1, 1] = scale[1]
        m_scale[2, 2] = scale[2]
        m_rot_x = tf.rotation_matrix(np.deg2rad(-rotate[0]), [1, 0, 0])
        m_rot_y = tf.rotation_matrix(np.deg2rad(-rotate[1]), [0, 1, 0])
        m_rot_z = tf.rotation_matrix(np.deg2rad(-rotate[2]), [0, 0, 1])
        m_trans = tf.translation_matrix([offset[0], offset[1], offset[2]])
        log.info(f"[........   ] Created transformation matrix (offset={offset}, rotate={rotate}).")
        return tf.concatenate_matrices(m_trans, m_rot_z, m_rot_y, m_rot_x, m_scale)

    log.error("[.......x   ] Error: Could not create transformation matrix.")
    return np.eye(4)


def _polygon_to_fp_lines(polygon, layer="Eco1.User", width=0.12):
    """Convert a shapely Polygon or MultiPolygon to KiCad fp_line records."""
    fp_lines = []

    def add_ring(ring):
        coords = list(ring.coords)
        for i in range(len(coords) - 1):
            start = coords[i]
            end = coords[i + 1]
            fp_lines.append([
                'fp_line',
                ['start', float(start[0]), float(start[1])],
                ['end', float(end[0]), float(end[1])],
                ['stroke', ['width', width], ['type', 'default']],
                ['layer', QuotedString(layer)]
            ])

    if isinstance(polygon, Polygon):
        add_ring(polygon.exterior)
        for interior in polygon.interiors:
            add_ring(interior)
    elif isinstance(polygon, MultiPolygon):
        for poly in polygon.geoms:
            add_ring(poly.exterior)
            for interior in poly.interiors:
                add_ring(interior)
    log.info("[...........] Generated KiCad S-Expression fp_lines from projected polygon.")
    return fp_lines


def _embedded_step_data_from_sexp(sexp, step_exts=(".stp", ".step")):
    """Find embedded STEP/STP base64 payload data in a footprint s-expression."""
    if not isinstance(sexp, list):
        return None
    if sexp and sexp[0] == 'embedded_files':
        for file_node in sexp[1:]:
            if not (isinstance(file_node, list) and file_node and file_node[0] == 'file'):
                continue
            name = next((item[1] for item in file_node[1:] if isinstance(item, list) and item and item[0] == 'name'), None)
            data_items = [
                item
                for item in file_node[1:]
                if isinstance(item, list) and item and item[0] == 'data'
            ]
            data = ''.join(data_items[0][1:]).replace('\n', '').replace('\r', '').strip('|') if data_items else None
            if name and any(str(name).lower().endswith(ext) for ext in step_exts) and data:
                log.info(f"[..         ] Success: Found step data for {str(name)}")
                return data
    for child in sexp:
        result = _embedded_step_data_from_sexp(child, step_exts)
        if result:
            return result
    return None


def _embedded_model_name_from_sexp(s_expr: list) -> str | None:
    """Return the kicad-embed model filename from the footprint model record."""
    for item in s_expr:
        if isinstance(item, list) and item and item[0] == 'model':
            if len(item) > 1 and isinstance(item[1], str) and item[1].startswith("kicad-embed://"):
                return item[1][len("kicad-embed://"):]
    return None


def _load_step_mesh_dict(step_data: bytes, file_name: str | None):
    """Load STEP data through trimesh's cascade importer."""
    step_io = io.BytesIO(step_data)
    try:
        return cast(Any, trimesh).exchange.cascade.load_step(
            step_io, file_type="step", merge_primitives=False
        )
    except Exception as e:
        log.warning(f"STEP loading failed for {file_name}: {e}")
        log.warning("Falling back to convex hull from pads...")
        return None


def _step_node_map(mesh_dict: dict[str, Any]) -> dict[str, np.ndarray]:
    """Build geometry-name to full transform map from a trimesh STEP graph."""
    frame_transforms = {}
    geometry_frames = {}
    for node in mesh_dict['graph']:
        frame_to = node.get('frame_to')
        frame_from = node.get('frame_from')
        matrix = node.get('matrix', np.eye(4))
        if frame_to:
            frame_transforms[frame_to] = (frame_from, np.array(matrix).reshape(4, 4))
        if 'geometry' in node and frame_to:
            geometry_frames[node['geometry']] = frame_to

    def get_full_transform(frame):
        if frame == 'world' or frame not in frame_transforms:
            return np.eye(4)
        parent_frame, local_matrix = frame_transforms[frame]
        return get_full_transform(parent_frame) @ local_matrix

    node_map = {geom_name: get_full_transform(frame) for geom_name, frame in geometry_frames.items()}
    for node in mesh_dict['graph']:
        if 'geometry' in node and node['geometry'] not in node_map:
            node_map[node['geometry']] = get_full_transform(node.get('frame_to', 'world'))
    return node_map


def _meshes_from_step_geometry(mesh_dict: dict[str, Any]) -> list:
    """Construct transformed trimesh meshes from STEP geometry payloads."""
    node_map = _step_node_map(mesh_dict)
    meshes = []
    for name, part in mesh_dict['geometry'].items():
        if 'faces' not in part:
            continue
        verts = part['vertices']
        if verts.shape[1] == 2:
            verts = np.hstack([verts, np.zeros((verts.shape[0], 1))])
        elif verts.shape[1] != 3:
            log.warning(f"Skipping geometry '{name}' with unexpected vertex shape: {verts.shape}")
            continue
        verts_hom = np.hstack([verts, np.ones((verts.shape[0], 1))])
        verts_trans = (node_map.get(name, np.eye(4)) @ verts_hom.T).T[:, :3]
        meshes.append(trimesh.Trimesh(vertices=verts_trans, faces=part['faces']))
    return meshes


def _project_mesh_shadow(mesh) -> Any:
    """Project a mesh along Z into a shapely 2D shadow polygon."""
    polys = []
    for face in mesh.faces:
        pts_2d = mesh.vertices[face][:, :2]
        pts_2d[:, 1] *= -1
        poly = Polygon(pts_2d)
        if poly.is_valid and poly.area > 1e-12:
            polys.append(poly)
    return unary_union(polys)


def _insert_fp_lines_after_drawings(s_expr: list, fp_lines: list) -> None:
    """Insert generated fab lines after existing drawing or embedded records."""
    draw_primitives = {'fp_line', 'fp_arc', 'fp_circle', 'fp_poly', 'fp_text', 'fp_rect'}
    last_draw_idx = -1
    last_embedded_idx = -1
    for idx, item in enumerate(s_expr):
        if isinstance(item, list) and item:
            if item[0] in draw_primitives:
                last_draw_idx = idx
            elif item[0] in ('embedded_files', 'model'):
                last_embedded_idx = idx

    insert_idx = last_draw_idx + 1 if last_draw_idx != -1 else last_embedded_idx + 1
    if insert_idx == 0 and last_embedded_idx == -1:
        insert_idx = len(s_expr)
    for fp_line in fp_lines:
        s_expr.insert(insert_idx, fp_line)
        insert_idx += 1


def fp_filter__orthographic_projection_outline(unfiltered_s_expression: Any) -> Any:
    """
    This is a filter for an s-expression file that does the following:
    - Extracts the embedded STEP file data.
    - Decodes the BASE64 and decompresses with ZSTD.
    - Assembles the STEP file and applies STEP file node graph requests and KiCad requested transformations.
    - Flattens the STEP model using Trimesh along the Z-Axis and finds an outline.
    - Applies the outline as an assortment of fp_lines on the appropriate fab layer to the s-expression file.
    - Adds a "REF**" string to the center of the part on the appropriate fab layer.
    - Auto-detects if footprint is on front or back side and uses appropriate fab layer (F.Fab or B.Fab).
    - If no embedded STEP model is found, falls back to fp_filter__add_fab_bounding_orthogonal_convex.
    """

    log.info("\nRunning fp_filter__orthographic_projection_outline()...\n")

    # Detect footprint side and determine appropriate fab layer
    side = get_footprint_side(unfiltered_s_expression)
    fab_layer = get_fab_layer_for_side(side)
    log.info(f"- Footprint is on {side} side, using {fab_layer} layer")

    unfiltered_s_expr_list = unfiltered_s_expression

    file_name = _embedded_model_name_from_sexp(unfiltered_s_expr_list)
    b64_data = _embedded_step_data_from_sexp(unfiltered_s_expr_list)
    if not b64_data:
        log.warning(f"Warning: No embedded STEP data found in {file_name}.")
        log.info("Falling back to fp_filter__add_fab_bounding_orthogonal_convex()...")
        return fp_filter__add_fab_bounding_orthogonal_convex(unfiltered_s_expr_list)

    compressed_data = base64.b64decode(b64_data)
    log.info("[...        ] Base64 decoded.")

    try:
        data = zstd.decompress(compressed_data)
        log.info("[....       ] Success: ZSTD decompressed successfully.")
    except Exception as e:
        log.error(f"[...x       ] Error: ZSTD decompression failed: {e}")
        return fp_filter__add_fab_bounding_orthogonal_convex(unfiltered_s_expr_list)

    mesh_dict = _load_step_mesh_dict(data, file_name)
    if mesh_dict is None:
        return fp_filter__add_fab_bounding_orthogonal_convex(unfiltered_s_expr_list)

    log.info("[.....      ] STEP model data set up for Trimesh.")
    meshes = _meshes_from_step_geometry(mesh_dict)
    log.info("[......     ] Constructed STEP model from node map.")

    for _i, part in enumerate(meshes):
        if not part.is_winding_consistent:
            part.invert()

    mesh = trimesh.util.concatenate(meshes)
    log.info("[.......    ] Mesh concatenated for global transformations.")

    # Apply scale: STEP is in meters, KiCad footprints are in mm
    mesh.apply_scale(1000)

    # Get and apply KiCad model transform (offset, scale, rotate)
    model_transform = _model_transform_from_sexp(unfiltered_s_expr_list)
    mesh.apply_transform(model_transform)
    log.info("[.........  ] Applied KiCad model transformations.")

    shadow_2d = _project_mesh_shadow(mesh)
    log.info("[.......... ] Created 2D projection.")

    fab_fp_lines = _polygon_to_fp_lines(shadow_2d, layer=fab_layer, width=0.12)
    filtered_s_expr = copy.deepcopy(unfiltered_s_expr_list)

    # Calculate bounding box dimensions and center from the 2D projection
    bounds = shadow_2d.bounds  # (minx, miny, maxx, maxy)
    projection_width = bounds[2] - bounds[0]
    projection_height = bounds[3] - bounds[1]
    projection_center = [(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2]
    hull_shortest_side = min(projection_width, projection_height)

    _insert_fp_lines_after_drawings(filtered_s_expr, fab_fp_lines)

    # Add reference text to the appropriate fab layer at the center of the projection
    log.info(f"[........... ] Adding reference text to {fab_layer} layer.")
    filtered_s_expr = add_reference_text_to_fab(filtered_s_expr, projection_center, hull_shortest_side, fab_layer)

    log.info("[ooooooooooo] Done!")
    return filtered_s_expr

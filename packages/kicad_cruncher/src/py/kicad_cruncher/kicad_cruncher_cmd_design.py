"""Design JSON command for kicad_cruncher."""

from __future__ import annotations

import argparse
import json
import logging
import math
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import TYPE_CHECKING

from kicad_cruncher.kicad_cruncher_common import (
    find_kicad_project_in_cwd,
    resolve_output_dir,
    supported_design_input_suffixes,
)

log = logging.getLogger(__name__)

if TYPE_CHECKING:
    from kicad_monkey import KiCadDesign, KiCadPcb

JsonObject = dict[str, object]
Artifact = dict[str, object]

_SVG_NS = "http://www.w3.org/2000/svg"
_XLINK_NS = "http://www.w3.org/1999/xlink"
_DRAWABLE_TAGS = {"circle", "ellipse", "line", "path", "polygon", "polyline", "rect"}
_PCB_EDGE_COLOR = "#D0D0D0"
_PCB_TRACE_COLOR = "#B8B8B8"
_PCB_PAD_COLOR = "#000000"
_PCB_PTH_DRILL_COLOR = "#2563EB"
_PCB_PTH_SLOT_COLOR = "#0891B2"
_PCB_NPTH_DRILL_COLOR = "#DC2626"
_PCB_NPTH_SLOT_COLOR = "#F97316"

ET.register_namespace("", _SVG_NS)
ET.register_namespace("xlink", _XLINK_NS)


def _resolve_input_file(raw_file: str | None) -> Path | None:
    """Resolve an explicit file or auto-detect a project in the current directory."""
    if raw_file:
        input_file = Path(raw_file).resolve()
        if input_file.exists():
            return input_file
        log.error("File not found: %s", input_file)
        return None

    input_file = find_kicad_project_in_cwd()
    if input_file is None:
        log.error("No file specified and no single .kicad_pro found in current directory")
        log.info("Usage: kicad-cruncher design [project.kicad_pro | schematic.kicad_sch]")
        return None
    log.info("Auto-detected project: %s", input_file.name)
    return input_file.resolve()


def _validate_input_suffix(input_file: Path) -> bool:
    """Return whether the input file suffix is supported for design JSON."""
    suffix = input_file.suffix.lower()
    if suffix in supported_design_input_suffixes():
        return True
    log.error("Unsupported file type: %s", suffix)
    log.info("Supported types: .kicad_pro, .kicad_sch")
    return False


def _safe_filename(value: str, *, fallback: str = "artifact") -> str:
    """Return a filesystem-safe filename stem."""
    text = re.sub(r"[^A-Za-z0-9_.-]+", "_", value.strip())
    text = re.sub(r"_+", "_", text).strip("_.-")
    return text or fallback


def _relpath(path: Path, root: Path) -> str:
    return str(path.relative_to(root)).replace("\\", "/")


def _write_json(path: Path, payload: JsonObject) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def _render_schematic_svgs(design: KiCadDesign, output_dir: Path) -> list[Artifact]:
    """Write one enriched black-and-white SVG per concrete schematic instance."""
    from kicad_monkey import KiCadSvgRenderOptions, render_ir_to_svg

    schematic_dir = output_dir / "schematics"
    artifacts: list[Artifact] = []
    used_names: set[str] = set()

    options = KiCadSvgRenderOptions.enriched_default()
    options.black_and_white = True
    options.background_color = "#FFFFFF"
    options.default_fill_color = "#000000"
    options.default_stroke_color = "#000000"

    for instance in design.schematic_instances():
        sheet_name = str(getattr(instance, "sheet_name", "") or "")
        source_path = getattr(instance, "source_path", None)
        if not sheet_name and source_path is not None:
            sheet_name = Path(source_path).stem
        safe_sheet_name = _safe_filename(sheet_name, fallback="sheet")
        base_name = f"{int(instance.sheet_number):02d}_{safe_sheet_name}"
        filename = f"{base_name}.svg"
        if filename in used_names:
            filename = f"{base_name}_{int(instance.instance_index):02d}.svg"
        used_names.add(filename)

        svg_path = schematic_dir / filename
        ir = design.to_schematic_instance_ir(instance)
        svg_text = render_ir_to_svg(ir, options=options)
        svg_path.parent.mkdir(parents=True, exist_ok=True)
        svg_path.write_text(svg_text, encoding="utf-8")

        artifacts.append(
            {
                "file": _relpath(svg_path, output_dir),
                "sheet_number": int(instance.sheet_number),
                "sheet_count": int(instance.sheet_count),
                "sheet_name": instance.sheet_name,
                "sheet_path": instance.sheet_path,
                "sheet_path_uuids": instance.sheet_path_uuids,
                "sheet_instance_path": instance.sheet_instance_path,
                "source": str(instance.source_path) if instance.source_path is not None else "",
            }
        )
    return artifacts


def _pcb_layer_name(layer: object) -> str:
    return str(getattr(layer, "canonical_name", None) or getattr(layer, "name", None) or "")


def _pcb_copper_layers(pcb: KiCadPcb) -> list[str]:
    return [
        name
        for layer in getattr(pcb, "layers", []) or []
        if (name := _pcb_layer_name(layer)).endswith(".Cu")
    ]


def _svg_local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1] if "}" in tag else tag


def _style_with_color(style: str, color: str) -> str:
    parts: list[str] = []
    seen: set[str] = set()
    for raw_part in style.split(";"):
        part = raw_part.strip()
        if not part or ":" not in part:
            continue
        key, value = [item.strip() for item in part.split(":", 1)]
        if key in {"fill", "stroke"} and value.lower() != "none":
            value = color
        seen.add(key)
        parts.append(f"{key}:{value}")
    if "stroke-linecap" not in seen:
        parts.append("stroke-linecap:round")
    if "stroke-linejoin" not in seen:
        parts.append("stroke-linejoin:round")
    return "; ".join(parts)


def _set_svg_element_color(element: ET.Element, color: str) -> None:
    local_name = _svg_local_name(element.tag)
    style = element.get("style")
    if style:
        element.set("style", _style_with_color(style, color))
    if local_name in _DRAWABLE_TAGS:
        fill = element.get("fill")
        if fill is not None and fill.lower() != "none":
            element.set("fill", color)
        stroke = element.get("stroke")
        if stroke is not None and stroke.lower() != "none":
            element.set("stroke", color)


def _is_drill_review_category(attrs: dict[str, str]) -> bool:
    return (
        "drill" in attrs.get("data-ref", "")
        or "drill" in attrs.get("data-primitive", "")
        or attrs.get("data-hole-render", "") == "drill"
    )


def _is_edge_review_category(attrs: dict[str, str]) -> bool:
    return (
        attrs.get("data-layer-name", "") == "Edge.Cuts"
        or attrs.get("data-layer-role", "") == "board-outline"
        or "Edge.Cuts" in attrs.get("data-layer-names", "")
    )


def _is_zone_review_category(attrs: dict[str, str]) -> bool:
    return attrs.get("data-ref", "") == "zone_fill" or attrs.get("data-primitive", "") == "zone"


def _is_track_review_category(attrs: dict[str, str]) -> bool:
    return attrs.get("data-ref", "") in {"segment", "track_arc", "via"} or attrs.get(
        "data-primitive", ""
    ) in {"track", "via"}


def _is_pad_review_category(attrs: dict[str, str]) -> bool:
    return attrs.get("data-ref", "") in {"pad", "footprint"} or attrs.get(
        "data-primitive", ""
    ) == "pad"


def _svg_review_category(attrs: dict[str, str]) -> str:
    for category, predicate in (
        ("drill", _is_drill_review_category),
        ("edge", _is_edge_review_category),
        ("zone", _is_zone_review_category),
        ("track", _is_track_review_category),
        ("pad", _is_pad_review_category),
    ):
        if predicate(attrs):
            return category
    return "other"


def _merged_data_attrs(chain: list[dict[str, str]], element: ET.Element) -> dict[str, str]:
    attrs: dict[str, str] = {}
    for item in chain:
        attrs.update(item)
    attrs.update({key: value for key, value in element.attrib.items() if key.startswith("data-")})
    return attrs


def _apply_pcb_review_theme(element: ET.Element, chain: list[dict[str, str]] | None = None) -> None:
    """Apply review colours based on enriched SVG data attributes."""
    inherited = [] if chain is None else chain
    attrs = _merged_data_attrs(inherited, element)
    category = _svg_review_category(attrs)
    if category == "edge":
        _set_svg_element_color(element, _PCB_EDGE_COLOR)
    elif category in {"track", "zone"}:
        _set_svg_element_color(element, _PCB_TRACE_COLOR)
    elif category == "pad":
        _set_svg_element_color(element, _PCB_PAD_COLOR)

    next_chain = inherited
    own_data = {key: value for key, value in element.attrib.items() if key.startswith("data-")}
    if own_data:
        next_chain = [*inherited, own_data]
    for child in list(element):
        _apply_pcb_review_theme(child, next_chain)


def _top_level_svg_group_category(element: ET.Element) -> str:
    attrs = {key: value for key, value in element.attrib.items() if key.startswith("data-")}
    return _svg_review_category(attrs)


def _reorder_pcb_review_groups(root: ET.Element) -> None:
    """Sort top-level drawing groups into the requested review draw order."""
    order = {"track": 10, "zone": 20, "edge": 30, "pad": 40, "drill": 50, "other": 25}
    children = list(root)
    indexed = list(enumerate(children))

    def sort_key(item: tuple[int, ET.Element]) -> tuple[int, int]:
        index, child = item
        if _svg_local_name(child.tag) != "g":
            return (0, index)
        return (order.get(_top_level_svg_group_category(child), order["other"]), index)

    root[:] = [child for _index, child in sorted(indexed, key=sort_key)]


def _fmt_svg(value: float) -> str:
    text = f"{float(value):.6f}".rstrip("0").rstrip(".")
    return text or "0"


def _layer_matches(layer: str, wanted: str) -> bool:
    if layer == wanted:
        return True
    if layer == "*.Cu":
        return wanted.endswith(".Cu")
    if layer == "F&B.Cu":
        return wanted in {"F.Cu", "B.Cu"}
    return False


def _pad_reaches_layer(pad: object, layer: str) -> bool:
    return any(
        _layer_matches(str(raw_layer), layer) for raw_layer in getattr(pad, "layers", []) or []
    )


def _copper_layer_index_map(pcb: KiCadPcb) -> dict[str, int]:
    return {layer: index for index, layer in enumerate(_pcb_copper_layers(pcb))}


def _via_reaches_layer(via: object, layer: str, layer_indexes: dict[str, int]) -> bool:
    layers = [str(item) for item in getattr(via, "layers", []) or []]
    if any(_layer_matches(item, layer) for item in layers):
        return True
    endpoints = [layer_indexes[item] for item in layers if item in layer_indexes]
    if len(endpoints) < 2 or layer not in layer_indexes:
        return False
    return min(endpoints) <= layer_indexes[layer] <= max(endpoints)


def _pad_type_value(pad: object) -> str:
    pad_type = getattr(pad, "pad_type", "")
    return str(getattr(pad_type, "value", pad_type))


def _rotated_point(x: float, y: float, angle_deg: float) -> tuple[float, float]:
    radians = math.radians(angle_deg)
    return (
        x * math.cos(radians) - y * math.sin(radians),
        x * math.sin(radians) + y * math.cos(radians),
    )


def _float_attr(obj: object, attr: str) -> float:
    return float(getattr(obj, attr, 0.0) or 0.0)


def _pad_drill_offset_mm(pad: object) -> tuple[float, float]:
    offset_x = _float_attr(pad, "drill_offset_x")
    offset_y = _float_attr(pad, "drill_offset_y")
    if not offset_x and not offset_y:
        return (0.0, 0.0)
    return _rotated_point(offset_x, offset_y, _float_attr(pad, "at_angle"))


def _pad_drill_center_mm(footprint: object, pad: object) -> tuple[float, float]:
    offset_x, offset_y = _pad_drill_offset_mm(pad)
    rel_x, rel_y = _rotated_point(
        _float_attr(pad, "at_x") + offset_x,
        _float_attr(pad, "at_y") + offset_y,
        _float_attr(footprint, "at_angle"),
    )
    return (_float_attr(footprint, "at_x") + rel_x, _float_attr(footprint, "at_y") + rel_y)


def _component_reference(footprint: object) -> str:
    getter = getattr(footprint, "get_property_value", None)
    if callable(getter):
        return str(getter("Reference", "") or "")
    return ""


def _append_round_hole(
    group: ET.Element,
    *,
    cx: float,
    cy: float,
    diameter: float,
    color: str,
    attrs: dict[str, str],
) -> None:
    element = ET.SubElement(group, f"{{{_SVG_NS}}}circle")
    element.set("cx", _fmt_svg(cx))
    element.set("cy", _fmt_svg(cy))
    element.set("r", _fmt_svg(diameter / 2.0))
    element.set("fill", color)
    element.set("fill-opacity", "0.82")
    element.set("stroke", color)
    element.set("stroke-width", "0.05")
    for key, value in attrs.items():
        if value:
            element.set(key, value)


def _append_slot(
    group: ET.Element,
    *,
    cx: float,
    cy: float,
    width: float,
    height: float,
    angle_deg: float,
    color: str,
    attrs: dict[str, str],
) -> None:
    major = max(width, height)
    minor = min(width, height)
    if major <= 0 or minor <= 0:
        return
    slot_angle = math.radians(-angle_deg)
    if height > width:
        slot_angle += math.pi / 2.0
    dx = math.cos(slot_angle) * (major - minor) / 2.0
    dy = math.sin(slot_angle) * (major - minor) / 2.0
    if abs(dx) < 1e-9 and abs(dy) < 1e-9:
        _append_round_hole(group, cx=cx, cy=cy, diameter=minor, color=color, attrs=attrs)
        return
    element = ET.SubElement(group, f"{{{_SVG_NS}}}line")
    element.set("x1", _fmt_svg(cx - dx))
    element.set("y1", _fmt_svg(cy - dy))
    element.set("x2", _fmt_svg(cx + dx))
    element.set("y2", _fmt_svg(cy + dy))
    element.set("stroke", color)
    element.set("stroke-width", _fmt_svg(minor))
    element.set("stroke-opacity", "0.82")
    element.set("stroke-linecap", "round")
    for key, value in attrs.items():
        if value:
            element.set(key, value)


def _bbox_svg_point(bbox: object, x_mm: float, y_mm: float) -> tuple[float, float]:
    return (x_mm - _float_attr(bbox, "min_x"), y_mm - _float_attr(bbox, "min_y"))


def _pad_hole_attrs(pad: object, *, component: str, plating: str) -> dict[str, str]:
    return {
        "data-review-object": "pad-hole",
        "data-component": component,
        "data-pad-number": str(getattr(pad, "number", "") or ""),
        "data-hole-plating": plating,
        "data-source-uuid": str(getattr(pad, "uuid", "") or ""),
    }


def _append_pad_slot_overlay(
    group: ET.Element,
    *,
    footprint: object,
    pad: object,
    cx: float,
    cy: float,
    plating: str,
    attrs: dict[str, str],
) -> bool:
    width = _float_attr(pad, "drill_width")
    height = _float_attr(pad, "drill_height")
    if width <= 0 or height <= 0:
        return False
    attrs["data-hole-kind"] = "slot"
    color = _PCB_NPTH_SLOT_COLOR if plating == "non-plated" else _PCB_PTH_SLOT_COLOR
    _append_slot(
        group,
        cx=cx,
        cy=cy,
        width=width,
        height=height,
        angle_deg=_float_attr(footprint, "at_angle") + _float_attr(pad, "at_angle"),
        color=color,
        attrs=attrs,
    )
    return True


def _append_pad_round_overlay(
    group: ET.Element,
    *,
    pad: object,
    cx: float,
    cy: float,
    plating: str,
    attrs: dict[str, str],
) -> bool:
    drill = _float_attr(pad, "drill")
    if drill <= 0:
        return False
    attrs["data-hole-kind"] = "round"
    color = _PCB_NPTH_DRILL_COLOR if plating == "non-plated" else _PCB_PTH_DRILL_COLOR
    _append_round_hole(group, cx=cx, cy=cy, diameter=drill, color=color, attrs=attrs)
    return True


def _append_pad_hole_overlay(
    group: ET.Element,
    *,
    bbox: object,
    footprint: object,
    pad: object,
    component: str,
    pad_type: str,
) -> bool:
    plating = "non-plated" if pad_type == "np_thru_hole" else "plated"
    cx_mm, cy_mm = _pad_drill_center_mm(footprint, pad)
    cx, cy = _bbox_svg_point(bbox, cx_mm, cy_mm)
    attrs = _pad_hole_attrs(pad, component=component, plating=plating)
    if bool(getattr(pad, "drill_oval", False)):
        return _append_pad_slot_overlay(
            group,
            footprint=footprint,
            pad=pad,
            cx=cx,
            cy=cy,
            plating=plating,
            attrs=attrs,
        )
    return _append_pad_round_overlay(
        group,
        pad=pad,
        cx=cx,
        cy=cy,
        plating=plating,
        attrs=attrs,
    )


def _append_pad_hole_overlays(
    group: ET.Element,
    *,
    pcb: KiCadPcb,
    bbox: object,
    layer: str,
) -> int:
    count = 0
    for footprint in getattr(pcb, "footprints", []) or []:
        component = _component_reference(footprint)
        for pad in getattr(footprint, "pads", []) or []:
            if not _pad_reaches_layer(pad, layer):
                continue
            pad_type = _pad_type_value(pad)
            if pad_type not in {"thru_hole", "np_thru_hole"}:
                continue
            count += int(
                _append_pad_hole_overlay(
                    group,
                    bbox=bbox,
                    footprint=footprint,
                    pad=pad,
                    component=component,
                    pad_type=pad_type,
                )
            )
    return count


def _append_via_hole_overlay(group: ET.Element, *, bbox: object, via: object) -> bool:
    drill = _float_attr(via, "drill")
    if drill <= 0:
        return False
    cx, cy = _bbox_svg_point(bbox, _float_attr(via, "at_x"), _float_attr(via, "at_y"))
    _append_round_hole(
        group,
        cx=cx,
        cy=cy,
        diameter=drill,
        color=_PCB_PTH_DRILL_COLOR,
        attrs={
            "data-review-object": "via-hole",
            "data-hole-kind": "round",
            "data-hole-plating": "plated",
            "data-via-type": str(getattr(via, "via_type", "") or "through"),
            "data-source-uuid": str(getattr(via, "uuid", "") or ""),
        },
    )
    return True


def _append_via_hole_overlays(
    group: ET.Element,
    *,
    pcb: KiCadPcb,
    bbox: object,
    layer: str,
) -> int:
    count = 0
    layer_indexes = _copper_layer_index_map(pcb)
    for via in getattr(pcb, "vias", []) or []:
        if _via_reaches_layer(via, layer, layer_indexes):
            count += int(_append_via_hole_overlay(group, bbox=bbox, via=via))
    return count


def _append_pcb_drill_slot_overlay(root: ET.Element, pcb: KiCadPcb, layer: str) -> int:
    """Append coloured drill/slot overlay geometry for one copper layer."""
    from kicad_monkey.kicad_pcb_bounds import compute_pcb_svg_bounding_box

    bbox = compute_pcb_svg_bounding_box(pcb, None)
    if getattr(bbox, "is_empty", False):
        return 0

    group = ET.Element(f"{{{_SVG_NS}}}g")
    group.set("id", "design-review-drills-slots")
    group.set("data-review-role", "drills-slots")
    group.set("data-layer-name", layer)
    count = _append_pad_hole_overlays(group, pcb=pcb, bbox=bbox, layer=layer)
    count += _append_via_hole_overlays(group, pcb=pcb, bbox=bbox, layer=layer)

    if count:
        root.append(group)
    return count


def _style_pcb_review_svg(svg_text: str, pcb: KiCadPcb, layer: str) -> tuple[str, int]:
    """Apply the design-review PCB visual contract to a rendered SVG string."""
    root = ET.fromstring(svg_text)
    root.set("data-review-theme", "kicad_cruncher.design_review.pcb_svg.a0")
    root.set("data-review-layer", layer)
    root.set("data-review-draw-order", "tracks,polygons-zones,edge-cuts,pads,drills-slots")
    _apply_pcb_review_theme(root)
    _reorder_pcb_review_groups(root)
    overlay_count = _append_pcb_drill_slot_overlay(root, pcb, layer)
    ET.indent(root, space="  ")
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(
        root,
        encoding="unicode",
        short_empty_elements=True,
    ), overlay_count


def _render_pcb_review_svgs(design: KiCadDesign, output_dir: Path) -> list[Artifact]:
    """Write one review SVG per copper layer, including edge cuts and drill overlays."""
    pcb = design.pcb
    if pcb is None:
        return []

    pcb_dir = output_dir / "pcb" / "copper_layers"
    board_name = _safe_filename(
        Path(str(design.pcb_path)).stem if getattr(design, "pcb_path", None) else "board"
    )
    artifacts: list[Artifact] = []
    for copper_layer in _pcb_copper_layers(pcb):
        svg_text = pcb.to_svg(
            layers=[copper_layer, "Edge.Cuts"],
            fill=_PCB_TRACE_COLOR,
            stroke=_PCB_TRACE_COLOR,
            black_and_white=False,
            profile="enriched",
        )
        styled_svg, overlay_count = _style_pcb_review_svg(svg_text, pcb, copper_layer)
        layer_file = pcb_dir / f"{board_name}__{_safe_filename(copper_layer)}__review.svg"
        layer_file.parent.mkdir(parents=True, exist_ok=True)
        layer_file.write_text(styled_svg, encoding="utf-8")
        artifacts.append(
            {
                "file": _relpath(layer_file, output_dir),
                "layer": copper_layer,
                "included_layers": [copper_layer, "Edge.Cuts"],
                "drill_slot_overlay_count": overlay_count,
            }
        )
    return artifacts


def _readme_text(
    *,
    input_file: Path,
    design_json: str,
    schematic_svgs: list[Artifact],
    pcb_svgs: list[Artifact],
    manifest_file: str,
) -> str:
    schematic_lines = "\n".join(
        f"- `{item['file']}`: sheet {item['sheet_number']}/{item['sheet_count']} "
        f"`{item['sheet_path']}` instance `{item.get('sheet_instance_path') or ''}`"
        for item in schematic_svgs
    ) or "- No schematic SVGs were generated."
    pcb_lines = "\n".join(
        f"- `{item['file']}`: copper layer `{item['layer']}` plus `Edge.Cuts` and "
        f"{item['drill_slot_overlay_count']} drill/slot overlays"
        for item in pcb_svgs
    ) or "- No PCB copper-layer SVGs were generated."
    return f"""# KiCad Design Review Bundle

Input: `{input_file}`

This folder is generated by `kicad-cruncher design` / `design-review` / `dr`.
It is intended for design review agents that need a machine-readable design
model plus visual context.

## Files

- `{design_json}`: KiCad-native design JSON from `kicad-monkey`.
- `{manifest_file}`: artifact index for this review bundle.
- `schematics/`: enriched black-and-white schematic SVGs, one file per
  concrete hierarchy instance.
- `pcb/copper_layers/`: enriched PCB SVGs, one file per copper layer.

## Design JSON Relationships

The design JSON schema is `kicad_monkey.design.a0`. It includes project text
variables, schematic hierarchy, components, nets, optional PnP data, and lookup
indexes unless `--no-indexes` was used.

Component and net entries carry SVG link fields where available. Schematic SVG
groups use `data-uuid`, `data-ref`, `data-component`, `data-pin-*`, and net
relationship attributes so an agent can map graphics back to symbols, pins,
ports, sheets, and nets. PCB SVG groups use `data-component`, `data-pad-*`,
`data-net`, `data-layer-*`, `data-hole-*`, and IPC-4761 via metadata when the
source board provides it.

## Schematic SVGs

{schematic_lines}

Repeated hierarchical sheets produce separate SVGs. Use `sheet_path` for the
human hierarchy path and `sheet_instance_path` for the KiCad UUID instance path.

## PCB Review SVGs

{pcb_lines}

PCB review SVGs preserve the enriched `kicad-monkey` metadata and apply this
review theme:

- pads belonging to footprints: black (`{_PCB_PAD_COLOR}`);
- tracks, arcs, vias, and zones/polygons: light gray (`{_PCB_TRACE_COLOR}`);
- board outline / `Edge.Cuts`: light gray (`{_PCB_EDGE_COLOR}`);
- plated drills: blue (`{_PCB_PTH_DRILL_COLOR}`);
- plated slots: cyan (`{_PCB_PTH_SLOT_COLOR}`);
- non-plated drills: red (`{_PCB_NPTH_DRILL_COLOR}`);
- non-plated slots: orange (`{_PCB_NPTH_SLOT_COLOR}`).

Draw order is tracks/arcs first, polygons/zones above those, edge cuts, pads,
then the coloured drill/slot overlay last.
"""


def _write_review_readme(
    output_dir: Path,
    *,
    input_file: Path,
    design_json_path: Path,
    schematic_svgs: list[Artifact],
    pcb_svgs: list[Artifact],
    manifest_path: Path,
) -> Path:
    readme_path = output_dir / "README.md"
    readme_path.write_text(
        _readme_text(
            input_file=input_file,
            design_json=_relpath(design_json_path, output_dir),
            schematic_svgs=schematic_svgs,
            pcb_svgs=pcb_svgs,
            manifest_file=_relpath(manifest_path, output_dir),
        ),
        encoding="utf-8",
    )
    return readme_path


def cmd_design(args: argparse.Namespace) -> int:
    """Generate a KiCad design review bundle from a project or schematic."""
    from kicad_monkey import KiCadDesign

    input_file = _resolve_input_file(str(args.file) if args.file else None)
    if input_file is None:
        return 1
    if not _validate_input_suffix(input_file):
        return 1

    output_dir = resolve_output_dir(args.output, "design")
    output_file = output_dir / f"{input_file.stem}_design.json"
    include_indexes = not bool(args.no_indexes)

    try:
        design = KiCadDesign.from_file(input_file)
        payload = design.to_json(include_indexes=include_indexes)
        _write_json(output_file, payload)
        schematic_svgs = _render_schematic_svgs(design, output_dir)
        pcb_svgs = _render_pcb_review_svgs(design, output_dir)
        manifest_path = output_dir / "design_review_manifest.json"
        manifest = {
            "schema": "kicad_cruncher.design_review_manifest.a0",
            "input": str(input_file),
            "design_json": _relpath(output_file, output_dir),
            "schematic_svgs": schematic_svgs,
            "pcb_svgs": pcb_svgs,
            "readme": "README.md",
        }
        _write_json(manifest_path, manifest)
        readme_path = _write_review_readme(
            output_dir,
            input_file=input_file,
            design_json_path=output_file,
            schematic_svgs=schematic_svgs,
            pcb_svgs=pcb_svgs,
            manifest_path=manifest_path,
        )
    except Exception as exc:
        log.error("Design review generation failed: %s", exc)
        return 1

    log.info(
        "Design review: %d components, %d nets, %d schematic SVGs, %d PCB SVGs -> %s",
        len(payload.get("components", [])),
        len(payload.get("nets", [])),
        len(schematic_svgs),
        len(pcb_svgs),
        readme_path,
    )
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the design command parser."""
    design_parser = subparsers.add_parser(
        "design",
        aliases=["design-review", "dr"],
        help="generate KiCad design review artifacts",
        description=(
            "Generate a KiCad design review bundle from .kicad_pro or .kicad_sch files. "
            "The output includes KiCad-native design JSON, enriched schematic SVGs, "
            "enriched PCB copper-layer SVGs, a manifest, and a README for review agents. "
            "The design JSON includes project metadata, schematic hierarchy, components, "
            "nets, variants, and optional lookup indexes."
        ),
        epilog=(
            "Examples:\n"
            "  kicad-cruncher design project.kicad_pro\n"
            "  kicad-cruncher design-review project.kicad_pro\n"
            "  kicad-cruncher dr project.kicad_pro\n"
            "  kicad-cruncher design schematic.kicad_sch\n"
            "  kicad-cruncher design                    # Auto-detect one .kicad_pro in CWD\n"
            "  kicad-cruncher design project.kicad_pro --no-indexes\n"
            "  kicad-cruncher design project.kicad_pro -o output_dir/"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    design_parser.add_argument(
        "file",
        nargs="?",
        help="KiCad project or schematic file; optional when one .kicad_pro is in CWD",
    )
    design_parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="output directory (default: ./output/design)",
    )
    design_parser.add_argument(
        "--no-indexes",
        action="store_true",
        help="exclude lookup indexes from JSON",
    )
    design_parser.set_defaults(handler=cmd_design)
    return design_parser

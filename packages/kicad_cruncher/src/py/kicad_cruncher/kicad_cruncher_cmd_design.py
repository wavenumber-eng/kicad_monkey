"""Design JSON command for kicad_cruncher."""

from __future__ import annotations

import argparse
import json
import logging
import re
import time
import xml.etree.ElementTree as ET
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Protocol

from kicad_cruncher.kicad_cruncher_common import (
    find_kicad_project_in_cwd,
    resolve_output_dir,
    supported_design_input_suffixes,
)
from kicad_cruncher.logging_utils import stage_done_text, stage_progress_text, stage_start_text

log = logging.getLogger(__name__)

if TYPE_CHECKING:
    from kicad_monkey import KiCadDesign, KiCadPcb

JsonObject = dict[str, object]
Artifact = dict[str, object]
ProgressCallback = Callable[[str], None]


class _SchematicInstanceLike(Protocol):
    """Typed subset of KiCad schematic instance fields used by this command."""

    sheet_name: str
    source_path: Path | None
    sheet_path: str
    sheet_number: int
    sheet_count: int
    instance_index: int
    sheet_path_uuids: list[str]
    sheet_instance_path: str


class _PcbReviewRenderCacheLike(Protocol):
    """Typed subset of the PCB SVG render cache used by design review."""

    def root_svg(self, pcb: KiCadPcb, layers: list[str] | None) -> ET.Element: ...

_SVG_NS = "http://www.w3.org/2000/svg"
_XLINK_NS = "http://www.w3.org/1999/xlink"
_DRAWABLE_TAGS = {"circle", "ellipse", "line", "path", "polygon", "polyline", "rect"}
_PCB_EDGE_COLOR = "#000000"
_PCB_TRACE_COLOR = "#B8B8B8"
_PCB_PAD_COLOR = "#000000"
_PCB_PTH_DRILL_COLOR = "#2563EB"
_PCB_PTH_SLOT_COLOR = "#0891B2"
_PCB_NPTH_DRILL_COLOR = "#DC2626"
_PCB_NPTH_SLOT_COLOR = "#F97316"
_PCB_UNKNOWN_HOLE_COLOR = "#6B7280"
_SCHEMATIC_REVIEW_THEME = "kicad_cruncher.design_review.schematic_svg.a0"

ET.register_namespace("", _SVG_NS)
ET.register_namespace("xlink", _XLINK_NS)


@dataclass(frozen=True, slots=True)
class DesignReviewBundle:
    """Files and counts emitted for one design-review bundle."""

    output_dir: Path
    design_json_path: Path
    netlist_json_path: Path
    netlist_kicad_sexpr_path: Path
    manifest_path: Path
    readme_path: Path
    manifest: JsonObject
    component_count: int
    net_count: int
    schematic_svgs: list[Artifact]
    pcb_svgs: list[Artifact]


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


def _emit_progress(progress: ProgressCallback | None, message: str) -> None:
    if progress is not None:
        progress(message)


def _schematic_instance_names(instance: _SchematicInstanceLike) -> tuple[str, Path | None, str]:
    """Return display names derived from a concrete schematic instance."""
    sheet_name = str(instance.sheet_name or "")
    source_path = instance.source_path
    if not sheet_name and source_path is not None:
        sheet_name = Path(str(source_path)).stem
    sheet_label = str(instance.sheet_path or "") or sheet_name or str(source_path or "sheet")
    return sheet_name, source_path, sheet_label


def _schematic_instance_filename(
    instance: _SchematicInstanceLike,
    sheet_name: str,
    used_names: set[str],
) -> str:
    """Return a stable schematic SVG filename for one instance."""
    safe_sheet_name = _safe_filename(sheet_name, fallback="sheet")
    base_name = f"{int(instance.sheet_number):02d}_{safe_sheet_name}"
    filename = f"{base_name}.svg"
    if filename in used_names:
        filename = f"{base_name}_{int(instance.instance_index):02d}.svg"
    used_names.add(filename)
    return filename


def _schematic_svg_artifact(
    instance: _SchematicInstanceLike,
    svg_path: Path,
    output_dir: Path,
) -> Artifact:
    """Return the manifest entry for one schematic SVG."""
    source_path = instance.source_path
    return {
        "file": _relpath(svg_path, output_dir),
        "sheet_number": int(instance.sheet_number),
        "sheet_count": int(instance.sheet_count),
        "sheet_name": instance.sheet_name,
        "sheet_path": instance.sheet_path,
        "sheet_path_uuids": instance.sheet_path_uuids,
        "sheet_instance_path": instance.sheet_instance_path,
        "source": str(source_path) if source_path is not None else "",
    }


def _render_schematic_svgs(
    design: KiCadDesign,
    output_dir: Path,
    *,
    design_payload: JsonObject,
    progress: ProgressCallback | None = None,
) -> list[Artifact]:
    """Write one enriched black-and-white SVG per concrete schematic instance."""
    from kicad_monkey import (
        SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS,
        KiCadSvgRenderOptions,
        render_ir_to_svg,
    )
    from kicad_monkey.kicad_schematic_svg_enrichment import (
        schematic_root_svg_attrs,
        schematic_svg_enrichment_metadata_element,
        schematic_svg_enrichment_payload,
    )

    schematic_dir = output_dir / "schematics"
    artifacts: list[Artifact] = []
    used_names: set[str] = set()

    options = KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
        SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS
    )
    profile_obj = options.profile
    profile_value = str(getattr(profile_obj, "value", profile_obj))

    instances = list(design.schematic_instances())
    _emit_progress(progress, f"rendering {len(instances)} schematic SVG(s)")
    for index, instance in enumerate(instances, start=1):
        sheet_name, source_path, sheet_label = _schematic_instance_names(instance)
        _emit_progress(
            progress,
            f"rendering schematic SVG {index}/{len(instances)}: {sheet_label}",
        )
        filename = _schematic_instance_filename(instance, sheet_name, used_names)
        svg_path = schematic_dir / filename
        ir = design.to_schematic_instance_ir(instance)
        metadata_payload = schematic_svg_enrichment_payload(
            design_payload,
            source_path=source_path or "",
            sheet_name=instance.sheet_name,
            sheet_path=instance.sheet_path,
            sheet_instance_path=instance.sheet_instance_path,
            profile=profile_value,
        )
        root_attrs = schematic_root_svg_attrs(
            source_path=source_path or "",
            sheet_name=instance.sheet_name,
            sheet_path=instance.sheet_path,
            profile=profile_value,
        )
        root_attrs["data-review-theme"] = _SCHEMATIC_REVIEW_THEME
        svg_text = render_ir_to_svg(
            ir,
            options=options,
            root_extra_attrs=root_attrs,
            metadata_elements=[schematic_svg_enrichment_metadata_element(metadata_payload)],
        )
        svg_path.parent.mkdir(parents=True, exist_ok=True)
        svg_path.write_text(svg_text, encoding="utf-8")
        _emit_progress(
            progress,
            f"wrote schematic SVG {index}/{len(instances)}: {_relpath(svg_path, output_dir)}",
        )

        artifacts.append(_schematic_svg_artifact(instance, svg_path, output_dir))
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
        attrs.get("data-ref", "") in {"drill_overlay", "pad_hole"}
        or attrs.get("data-primitive", "") in {"pad-hole", "via-hole"}
        or attrs.get("data-hole-render", "") in {"drill", "slot"}
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


def _pcb_hole_review_color(attrs: dict[str, str]) -> str:
    plating = attrs.get("data-hole-plating", "")
    hole_kind = attrs.get("data-hole-kind", "")
    if plating not in {"plated", "non_plated"}:
        return _PCB_UNKNOWN_HOLE_COLOR
    is_non_plated = plating == "non_plated"
    if hole_kind == "slot":
        return _PCB_NPTH_SLOT_COLOR if is_non_plated else _PCB_PTH_SLOT_COLOR
    return _PCB_NPTH_DRILL_COLOR if is_non_plated else _PCB_PTH_DRILL_COLOR


def _apply_pcb_review_theme(element: ET.Element, chain: list[dict[str, str]] | None = None) -> None:
    """Apply review colours based on enriched SVG data attributes."""
    inherited = [] if chain is None else chain
    attrs = _merged_data_attrs(inherited, element)
    category = _svg_review_category(attrs)
    if category == "drill":
        _set_svg_element_color(element, _pcb_hole_review_color(attrs))
    elif category == "edge":
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


def _count_pcb_hole_records(root: ET.Element) -> int:
    """Count KiCad Monkey enriched drill/slot records in a styled SVG."""
    return sum(
        1
        for element in root.iter()
        if element.attrib.get("data-primitive") in {"pad-hole", "via-hole"}
        and element.attrib.get("data-hole-kind")
        and element.attrib.get("data-hole-plating")
    )


def _style_pcb_review_svg(svg_text: str, layer: str) -> tuple[str, int]:
    """Apply the design-review PCB visual contract to a rendered SVG string."""
    root = ET.fromstring(svg_text)
    root.set("data-review-theme", "kicad_cruncher.design_review.pcb_svg.a0")
    root.set("data-review-layer", layer)
    root.set("data-review-draw-order", "tracks,polygons-zones,edge-cuts,pads,drills-slots")
    _apply_pcb_review_theme(root)
    _reorder_pcb_review_groups(root)
    drill_slot_record_count = _count_pcb_hole_records(root)
    ET.indent(root, space="  ")
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(
        root,
        encoding="unicode",
        short_empty_elements=True,
    ), drill_slot_record_count


def _cached_pcb_review_svg_text(
    pcb: KiCadPcb,
    render_cache: _PcbReviewRenderCacheLike,
    layer: str,
) -> str:
    """Render one PCB review layer from a command-scoped cached board IR."""
    root_svg = render_cache.root_svg(pcb, [layer, "Edge.Cuts"])
    return ET.tostring(root_svg, encoding="unicode", short_empty_elements=True)


def _render_pcb_review_svgs(
    design: KiCadDesign,
    output_dir: Path,
    *,
    progress: ProgressCallback | None = None,
) -> list[Artifact]:
    """Write one review SVG per copper layer, including edge cuts and hole records."""
    pcb = design.pcb
    if pcb is None:
        _emit_progress(progress, "skipping PCB review SVGs: no board is attached")
        return []

    pcb_dir = output_dir / "pcb" / "copper_layers"
    board_name = _safe_filename(
        Path(str(design.pcb_path)).stem if getattr(design, "pcb_path", None) else "board"
    )
    artifacts: list[Artifact] = []
    copper_layers = _pcb_copper_layers(pcb)
    from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
        PcbSvgCompositionRenderCache,
    )

    render_cache = PcbSvgCompositionRenderCache(pcb)
    _emit_progress(progress, f"rendering {len(copper_layers)} PCB copper-layer review SVG(s)")
    for index, copper_layer in enumerate(copper_layers, start=1):
        _emit_progress(
            progress,
            f"rendering PCB review SVG {index}/{len(copper_layers)}: {copper_layer}",
        )
        svg_text = _cached_pcb_review_svg_text(pcb, render_cache, copper_layer)
        styled_svg, drill_slot_record_count = _style_pcb_review_svg(svg_text, copper_layer)
        layer_file = pcb_dir / f"{board_name}__{_safe_filename(copper_layer)}__review.svg"
        layer_file.parent.mkdir(parents=True, exist_ok=True)
        layer_file.write_text(styled_svg, encoding="utf-8")
        _emit_progress(
            progress,
            f"wrote PCB review SVG {index}/{len(copper_layers)}: "
            f"{_relpath(layer_file, output_dir)}",
        )
        artifacts.append(
            {
                "file": _relpath(layer_file, output_dir),
                "layer": copper_layer,
                "included_layers": [copper_layer, "Edge.Cuts"],
                "drill_slot_record_count": drill_slot_record_count,
            }
        )
    return artifacts


def _readme_text(
    *,
    input_file: Path,
    design_json: str,
    netlist_json: str,
    netlist_kicad_sexpr: str,
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
        f"{item['drill_slot_record_count']} enriched drill/slot records"
        for item in pcb_svgs
    ) or "- No PCB copper-layer SVGs were generated."
    return f"""# KiCad Design Review Bundle

Input: `{input_file}`

This folder is generated by `kicad-cruncher design` / `design-review` / `dr`.
It is intended for design review agents that need a machine-readable design
model plus visual context.

## Files

- `{design_json}`: KiCad-native design JSON from `kicad-monkey`.
- `{netlist_json}`: KiCad-native netlist JSON from `kicad-monkey`.
- `{netlist_kicad_sexpr}`: kicad-cli-style S-expression netlist.
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
Schematic SVGs use the `{_SCHEMATIC_REVIEW_THEME}` role theme from
`kicad-monkey`: enriched source-object groups and net metadata are preserved,
while schematic graphics are rendered as black on a white page for review.

## PCB Review SVGs

{pcb_lines}

PCB review SVGs preserve the enriched `kicad-monkey` metadata and apply this
review theme:

- pads belonging to footprints: black (`{_PCB_PAD_COLOR}`);
- tracks, arcs, vias, and zones/polygons: light gray (`{_PCB_TRACE_COLOR}`);
- board outline / `Edge.Cuts`: black (`{_PCB_EDGE_COLOR}`);
- plated drills: blue (`{_PCB_PTH_DRILL_COLOR}`);
- plated slots: cyan (`{_PCB_PTH_SLOT_COLOR}`);
- non-plated drills: red (`{_PCB_NPTH_DRILL_COLOR}`);
- non-plated slots: orange (`{_PCB_NPTH_SLOT_COLOR}`);
- unknown-plating holes: neutral gray (`{_PCB_UNKNOWN_HOLE_COLOR}`).

Drill and slot cutouts come from the enriched `kicad-monkey` PCB SVG records.
Applications should use `data-hole-plating` and `data-hole-kind` to distinguish
plated through-hole pads/vias from KiCad `np_thru_hole` mechanical pads. Valid
plating values are `plated`, `non_plated`, and `unknown`. The design-review
theme colors those existing records in place; it does not create a second
drill/slot overlay, add duplicate boolean plating fields, or change the
`kicad-monkey` spelling of `non_plated`.

Draw order is tracks/arcs first, polygons/zones above those, edge cuts, pads,
then the `kicad-monkey` drill/slot records last.
"""


def _write_review_readme(
    output_dir: Path,
    *,
    input_file: Path,
    design_json_path: Path,
    netlist_json_path: Path,
    netlist_kicad_sexpr_path: Path,
    schematic_svgs: list[Artifact],
    pcb_svgs: list[Artifact],
    manifest_path: Path,
) -> Path:
    readme_path = output_dir / "README.md"
    readme_path.write_text(
        _readme_text(
            input_file=input_file,
            design_json=_relpath(design_json_path, output_dir),
            netlist_json=_relpath(netlist_json_path, output_dir),
            netlist_kicad_sexpr=_relpath(netlist_kicad_sexpr_path, output_dir),
            schematic_svgs=schematic_svgs,
            pcb_svgs=pcb_svgs,
            manifest_file=_relpath(manifest_path, output_dir),
        ),
        encoding="utf-8",
    )
    return readme_path


def write_design_review_bundle(
    input_file: Path,
    output_dir: Path,
    *,
    include_indexes: bool = True,
    progress: ProgressCallback | None = None,
) -> DesignReviewBundle:
    """Write the shared design-review bundle used by design and megamaid."""
    from kicad_monkey import KiCadDesign

    output_dir.mkdir(parents=True, exist_ok=True)
    _emit_progress(progress, f"loading design from {input_file}")
    design = KiCadDesign.from_file(input_file)
    _emit_progress(progress, "building design JSON")
    design_payload = design.to_json(include_indexes=include_indexes)

    design_json_path = output_dir / f"{input_file.stem}_design.json"
    netlist_json_path = output_dir / f"{input_file.stem}_netlist.json"
    netlist_kicad_sexpr_path = output_dir / f"{input_file.stem}_netlist.net"
    _emit_progress(progress, f"writing design JSON: {_relpath(design_json_path, output_dir)}")
    _write_json(design_json_path, design_payload)
    _emit_progress(progress, "building netlist JSON")
    netlist_payload = design.to_netlist_json()
    _emit_progress(progress, f"writing netlist JSON: {_relpath(netlist_json_path, output_dir)}")
    _write_json(netlist_json_path, netlist_payload)
    _emit_progress(progress, "building KiCad S-expression netlist")
    netlist_kicad_sexpr_path.write_text(
        design.to_kicad_netlist_sexpr(tool="kicad_cruncher", date=""),
        encoding="utf-8",
    )

    schematic_svgs = _render_schematic_svgs(
        design,
        output_dir,
        design_payload=design_payload,
        progress=progress,
    )
    pcb_svgs = _render_pcb_review_svgs(design, output_dir, progress=progress)
    manifest_path = output_dir / "design_review_manifest.json"
    manifest: JsonObject = {
        "schema": "kicad_cruncher.design_review_manifest.a0",
        "input": str(input_file),
        "design_json": _relpath(design_json_path, output_dir),
        "netlist_json": _relpath(netlist_json_path, output_dir),
        "netlist_kicad_sexpr": _relpath(netlist_kicad_sexpr_path, output_dir),
        "schematic_svgs": schematic_svgs,
        "pcb_svgs": pcb_svgs,
        "readme": "README.md",
    }
    _emit_progress(progress, "writing design-review manifest and README")
    _write_json(manifest_path, manifest)
    readme_path = _write_review_readme(
        output_dir,
        input_file=input_file,
        design_json_path=design_json_path,
        netlist_json_path=netlist_json_path,
        netlist_kicad_sexpr_path=netlist_kicad_sexpr_path,
        schematic_svgs=schematic_svgs,
        pcb_svgs=pcb_svgs,
        manifest_path=manifest_path,
    )
    return DesignReviewBundle(
        output_dir=output_dir,
        design_json_path=design_json_path,
        netlist_json_path=netlist_json_path,
        netlist_kicad_sexpr_path=netlist_kicad_sexpr_path,
        manifest_path=manifest_path,
        readme_path=readme_path,
        manifest=manifest,
        component_count=len(design_payload.get("components", [])),
        net_count=len(design_payload.get("nets", [])),
        schematic_svgs=schematic_svgs,
        pcb_svgs=pcb_svgs,
    )


def cmd_design(args: argparse.Namespace) -> int:
    """Generate a KiCad design review bundle from a project or schematic."""
    input_file = _resolve_input_file(str(args.file) if args.file else None)
    if input_file is None:
        return 1
    if not _validate_input_suffix(input_file):
        return 1

    output_dir = resolve_output_dir(args.output, "design")
    include_indexes = not bool(args.no_indexes)
    started = time.perf_counter()

    def progress(message: str) -> None:
        log.info("%s", stage_progress_text(f"Design review: {message}"))

    try:
        log.info("%s", stage_start_text(f"Design review: starting bundle for {input_file}"))
        bundle = write_design_review_bundle(
            input_file,
            output_dir,
            include_indexes=include_indexes,
            progress=progress,
        )
    except Exception as exc:
        log.error("Design review generation failed: %s", exc)
        return 1

    log.info(
        "%s",
        stage_done_text(
            "Design review: "
            f"{bundle.component_count} components, {bundle.net_count} nets, "
            f"{len(bundle.schematic_svgs)} schematic SVGs, {len(bundle.pcb_svgs)} PCB SVGs "
            f"-> {bundle.readme_path} in {time.perf_counter() - started:.2f}s"
        ),
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
            "The output includes KiCad-native design JSON, enriched black-and-white "
            "schematic SVGs, enriched PCB copper-layer SVGs, KiCad-native netlist JSON, "
            "a KiCad S-expression netlist, a manifest, and a README for review agents. "
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

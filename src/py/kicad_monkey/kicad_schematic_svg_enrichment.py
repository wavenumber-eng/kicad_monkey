"""Schematic SVG enrichment helpers.

The strict oracle profile suppresses these attributes. The enriched profile
uses them as a DOM lookup surface that lines up with design/netlist JSON.
"""

from __future__ import annotations

from collections.abc import Iterable
from collections import Counter
import html
import json
from typing import Any
import xml.etree.ElementTree as ET

from .kicad_compiled_schematic_graph import validate_compiled_schematic_graph
from .kicad_plotter_ir import KiCadPlotterOp, KiCadPlotterRecord


KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA = "kicad_monkey.schematic.svg.enrichment.a0"
KICAD_SCHEMATIC_SVG_ENRICHMENT_METADATA_ID = "schematic-enrichment-a0"
KICAD_SCHEMATIC_COMPILED_GRAPH_VIEW_SCHEMA = (
    "kicad_monkey.schematic.svg.compiled_graph_view.a0"
)
KICAD_SCHEMATIC_GRAPH_LINKAGE_CONTRACT = (
    "kicad_monkey.schematic.svg.compiled_graph_linkage.a0"
)
KICAD_SCHEMATIC_GRAPH_ARTIFACT_KEY = "sch.dwg_scene"

_SCHEMATIC_RECORD_KINDS = {
    "sheet_header",
    "sheet_header_background",
    "wire",
    "bus",
    "bus_entry",
    "junction",
    "no_connect",
    "label",
    "global_label",
    "hierarchical_label",
    "text",
    "text_box",
    "graphic_polyline",
    "graphic_rectangle",
    "graphic_arc",
    "graphic_circle",
    "graphic_bezier",
    "image",
    "symbol_instance",
    "symbol_overplot",
    "sheet",
    "netclass_flag",
    "table",
}


def _clean_string(value: object) -> str:
    return str(value or "").strip()


def _record_primitive(record: KiCadPlotterRecord) -> str:
    extras = record.extras or {}
    if record.kind == "symbol_instance":
        if _is_power_symbol(record):
            return "power-symbol"
        return "symbol"
    if record.kind == "symbol_overplot":
        return "symbol-overplot"
    if record.kind == "sheet":
        return "sheet-symbol"
    if record.kind == "hierarchical_label":
        return "port"
    if record.kind == "global_label":
        return "global-label"
    if record.kind == "label":
        return "label"
    if record.kind == "bus_entry":
        return "bus-entry"
    if record.kind == "no_connect":
        return "no-connect"
    if record.kind == "netclass_flag":
        return "netclass-flag"
    if record.kind.startswith("graphic_"):
        return "graphic"
    if record.kind == "sheet_header_background":
        return "drawing-sheet-background"
    if record.kind == "sheet_header":
        return "drawing-sheet"
    if record.kind == "text_box":
        return "text-box"
    return _clean_string(extras.get("primitive")) or record.kind or "record"


def _is_power_symbol(record: KiCadPlotterRecord) -> bool:
    extras = record.extras or {}
    reference = _clean_string(extras.get("reference"))
    lib_id = _clean_string(extras.get("lib_id")).lower()
    return reference.startswith("#") or lib_id.startswith("power:")


def _shape_attrs(attrs: dict[str, object], extras: dict[str, Any]) -> None:
    shape = extras.get("shape")
    if shape is not None and str(shape) != "":
        attrs["data-shape"] = shape


def _text_attrs(attrs: dict[str, object], extras: dict[str, Any]) -> None:
    text = extras.get("text")
    if text is not None and str(text) != "":
        attrs["data-text"] = text


def _symbol_attrs(attrs: dict[str, object], record: KiCadPlotterRecord) -> None:
    extras = record.extras or {}
    reference = _clean_string(extras.get("reference"))
    if reference:
        attrs["data-component"] = reference
        attrs["data-designator"] = reference
    if record.uuid:
        attrs["data-component-uid"] = record.uuid
        attrs["data-component-uuid"] = record.uuid
    if extras.get("lib_id"):
        attrs["data-symbol-library-ref"] = extras["lib_id"]
    if extras.get("lib_name"):
        attrs["data-symbol-library-name"] = extras["lib_name"]
    if extras.get("unit") is not None:
        attrs["data-symbol-unit"] = extras["unit"]
    if extras.get("convert") is not None:
        attrs["data-symbol-convert"] = extras["convert"]
    attrs["data-symbol-role"] = "power" if _is_power_symbol(record) else "component"
    for key in (
        "in_bom",
        "on_board",
        "dnp",
        "exclude_from_sim",
        "in_pos_files",
    ):
        if key in extras:
            attrs[f"data-{key.replace('_', '-')}"] = str(bool(extras[key])).lower()


def _sheet_attrs(attrs: dict[str, object], extras: dict[str, Any]) -> None:
    if extras.get("sheet_name"):
        attrs["data-sheet-name"] = extras["sheet_name"]
    if extras.get("sheet_file"):
        attrs["data-sheet-file"] = extras["sheet_file"]
    for key in ("at_x_nm", "at_y_nm", "size_x_nm", "size_y_nm"):
        if extras.get(key) is not None:
            attrs[f"data-{key.replace('_', '-')}"] = extras[key]


def _record_layer_attrs(
    attrs: dict[str, object],
    operations: Iterable[KiCadPlotterOp],
) -> None:
    layers: list[str] = []
    for op in operations:
        payload = op.payload or {}
        layer = payload.get("layer")
        if isinstance(layer, str) and layer and layer not in layers:
            layers.append(layer)
    if len(layers) == 1:
        attrs["data-layer-name"] = layers[0]
    elif layers:
        attrs["data-layer-names"] = ",".join(layers)


def schematic_record_svg_data_attrs(
    record: KiCadPlotterRecord,
    operations: Iterable[KiCadPlotterOp],
) -> dict[str, object]:
    """Return public SVG `data-*` attrs for a schematic record group."""

    ops = list(operations)
    extras = record.extras or {}
    primitive = _record_primitive(record)
    attrs: dict[str, object] = {
        "data-primitive": primitive,
        "data-source-kind": "schematic",
    }
    if record.uuid:
        attrs["data-element-key"] = record.uuid

    _record_layer_attrs(attrs, ops)

    if record.kind in {"label", "global_label", "hierarchical_label", "text", "text_box"}:
        _text_attrs(attrs, extras)
        _shape_attrs(attrs, extras)
    elif record.kind in {"symbol_instance", "symbol_overplot"}:
        _symbol_attrs(attrs, record)
    elif record.kind == "sheet":
        _sheet_attrs(attrs, extras)
    elif record.kind == "netclass_flag":
        _shape_attrs(attrs, extras)

    return attrs


def schematic_record_has_svg_data_attrs(record: KiCadPlotterRecord) -> bool:
    """Return True when `record` is covered by the schematic SVG contract."""

    return record.kind in _SCHEMATIC_RECORD_KINDS


def _sheet_key_variants(value: object) -> list[str]:
    text = str(value or "").strip()
    if not text:
        return []
    variants = [text]
    if not text.startswith("/"):
        text = f"/{text}"
        variants.append(text)
    if text != "/" and not text.endswith("/"):
        variants.append(f"{text}/")
    return variants


def _sheet_lookup_keys(*, sheet_path: object, sheet_instance_path: object) -> list[str]:
    keys: list[str] = []

    def add(value: object) -> None:
        for key in _sheet_key_variants(value):
            if key not in keys:
                keys.append(key)

    add(sheet_instance_path)
    instance_parts = [
        part for part in str(sheet_instance_path or "").strip("/").split("/") if part
    ]
    if len(instance_parts) > 1:
        add(f"/{'/'.join(instance_parts[1:])}/")
    elif len(instance_parts) == 1:
        add("/")
    add(sheet_path)
    return keys


def _net_summary_by_name(design_payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for net in design_payload.get("nets", []) or []:
        if not isinstance(net, dict):
            continue
        name = str(net.get("name", "") or "")
        if not name:
            continue
        row: dict[str, Any] = {
            "uid": str(net.get("uid", "") or ""),
            "name": name,
        }
        if "auto_named" in net:
            row["auto_named"] = bool(net.get("auto_named"))
        if net.get("net_class"):
            row["net_class"] = str(net.get("net_class", ""))
        out.setdefault(name, row)
    return out


def _net_summary(name: str, net_by_name: dict[str, dict[str, Any]]) -> dict[str, Any]:
    return dict(net_by_name.get(name) or {"uid": "", "name": name})


def schematic_svg_view_indexes(
    design_payload: dict[str, Any],
    *,
    sheet_path: object = "",
    sheet_instance_path: object = "",
) -> dict[str, Any]:
    """Return current-view net lookup indexes for enriched schematic SVG."""

    indexes = design_payload.get("indexes", {}) if isinstance(design_payload, dict) else {}
    if not isinstance(indexes, dict):
        indexes = {}
    sheet_map = indexes.get("sheet_svg_to_nets", {})
    if not isinstance(sheet_map, dict):
        sheet_map = {}

    sheet_keys = _sheet_lookup_keys(
        sheet_path=sheet_path,
        sheet_instance_path=sheet_instance_path,
    )
    svg_to_net_names: dict[str, set[str]] = {}
    for sheet_key in sheet_keys:
        row = sheet_map.get(sheet_key, {})
        if not isinstance(row, dict):
            continue
        for svg_id, net_names in row.items():
            svg_key = str(svg_id or "")
            if not svg_key:
                continue
            if isinstance(net_names, str):
                candidates = [net_names]
            else:
                candidates = list(net_names or [])
            for name in candidates:
                net_name = str(name or "")
                if net_name:
                    svg_to_net_names.setdefault(svg_key, set()).add(net_name)

    net_by_name = _net_summary_by_name(design_payload)
    svg_to_nets: dict[str, list[dict[str, Any]]] = {}
    svg_to_net: dict[str, dict[str, Any]] = {}
    net_to_svg: dict[str, list[str]] = {}
    net_uid_to_svg: dict[str, list[str]] = {}
    for svg_id, net_names in sorted(svg_to_net_names.items()):
        summaries = [_net_summary(name, net_by_name) for name in sorted(net_names)]
        svg_to_nets[svg_id] = summaries
        if len(summaries) == 1:
            svg_to_net[svg_id] = summaries[0]
        for summary in summaries:
            name = str(summary.get("name", "") or "")
            uid = str(summary.get("uid", "") or "")
            if name:
                net_to_svg.setdefault(name, []).append(svg_id)
            if uid:
                net_uid_to_svg.setdefault(uid, []).append(svg_id)

    return {
        "sheet_lookup_keys": sheet_keys,
        "svg_to_net": svg_to_net,
        "svg_to_nets": svg_to_nets,
        "net_to_svg": {
            name: sorted(set(svg_ids)) for name, svg_ids in sorted(net_to_svg.items())
        },
        "net_uid_to_svg": {
            uid: sorted(set(svg_ids)) for uid, svg_ids in sorted(net_uid_to_svg.items())
        },
    }


def resolve_compiled_schematic_graph_page_occurrence(
    compiled_schematic_graph: dict[str, Any],
    schematic_instance: object,
) -> dict[str, Any]:
    """Resolve exactly one graph page for a concrete schematic occurrence."""
    validate_compiled_schematic_graph(compiled_schematic_graph)
    instance_path = _clean_string(
        getattr(schematic_instance, "sheet_instance_path", "")
    )
    if not instance_path:
        raise ValueError("schematic instance has no sheet_instance_path")
    source_record = f"instance-path:{instance_path}"
    matches = [
        row
        for row in compiled_schematic_graph["page_occurrences"]
        if row.get("source_identity", {}).get("sch.source_key.source_record")
        == source_record
    ]
    if len(matches) != 1:
        raise ValueError(
            "compiled schematic graph page resolution requires exactly one "
            f"match for {source_record!r}; found {len(matches)}"
        )
    return matches[0]


def _compiled_schematic_graph_page_view_for_ref(
    compiled_schematic_graph: dict[str, Any],
    *,
    page_occurrence_ref: str,
    graph_artifact: str,
    artifact_key: str,
) -> dict[str, Any]:
    links = sorted(
        (
            row
            for row in compiled_schematic_graph["graphical_artifact_links"]
            if row.get("page_occurrence_ref") == page_occurrence_ref
            and row.get("artifact_key") == artifact_key
        ),
        key=lambda row: str(row["id"]),
    )
    element_to_links: dict[str, list[str]] = {}
    target_to_elements: dict[str, list[str]] = {}
    target_type_by_ref: dict[str, str] = {}
    for link in links:
        element_id = str(link["element_id"])
        link_ref = str(link["id"])
        target_ref = str(link["target_ref"])
        element_to_links.setdefault(element_id, []).append(link_ref)
        target_to_elements.setdefault(target_ref, []).append(element_id)
        target_type_by_ref[target_ref] = str(link["target_type"])

    return {
        "schema": KICAD_SCHEMATIC_COMPILED_GRAPH_VIEW_SCHEMA,
        "graph_schema": str(compiled_schematic_graph["schema"]),
        "identity_namespace": str(compiled_schematic_graph["identity_namespace"]),
        "graph_artifact": str(graph_artifact),
        "linkage_contract": KICAD_SCHEMATIC_GRAPH_LINKAGE_CONTRACT,
        "page_occurrence_ref": page_occurrence_ref,
        "artifact_key": artifact_key,
        "graphical_artifact_link_refs": [str(row["id"]) for row in links],
        "element_to_graphical_artifact_link_refs": {
            element_id: sorted(link_refs)
            for element_id, link_refs in sorted(element_to_links.items())
        },
        "target_to_element_ids": {
            target_ref: sorted(set(element_ids))
            for target_ref, element_ids in sorted(target_to_elements.items())
        },
        "target_type_by_ref": dict(sorted(target_type_by_ref.items())),
    }


def compiled_schematic_graph_page_view(
    compiled_schematic_graph: dict[str, Any],
    schematic_instance: object,
    *,
    graph_artifact: str,
    artifact_key: str = KICAD_SCHEMATIC_GRAPH_ARTIFACT_KEY,
) -> dict[str, Any]:
    """Project deterministic graph/SVG joins for one schematic occurrence."""
    page = resolve_compiled_schematic_graph_page_occurrence(
        compiled_schematic_graph,
        schematic_instance,
    )
    return _compiled_schematic_graph_page_view_for_ref(
        compiled_schematic_graph,
        page_occurrence_ref=str(page["id"]),
        graph_artifact=graph_artifact,
        artifact_key=artifact_key,
    )


def validate_schematic_svg_compiled_graph_view(
    svg_text: str,
    compiled_schematic_graph: dict[str, Any],
    graph_view: dict[str, Any],
) -> dict[str, int]:
    """Validate page ownership and exact SVG identity resolution for a view."""
    validate_compiled_schematic_graph(compiled_schematic_graph)
    page_ref = _clean_string(graph_view.get("page_occurrence_ref"))
    page_refs = {
        str(row["id"]) for row in compiled_schematic_graph["page_occurrences"]
    }
    if page_ref not in page_refs:
        raise ValueError(f"compiled graph view references unknown page {page_ref!r}")
    expected = _compiled_schematic_graph_page_view_for_ref(
        compiled_schematic_graph,
        page_occurrence_ref=page_ref,
        graph_artifact=_clean_string(graph_view.get("graph_artifact")),
        artifact_key=_clean_string(graph_view.get("artifact_key")),
    )
    if graph_view != expected:
        raise ValueError("compiled graph SVG view does not match its owning graph page")

    root = ET.fromstring(svg_text)
    id_counts = Counter(
        element.attrib["id"]
        for element in root.iter()
        if element.attrib.get("id")
    )
    element_ids = list(expected["element_to_graphical_artifact_link_refs"])
    missing = sorted(element_id for element_id in element_ids if id_counts[element_id] == 0)
    ambiguous = sorted(
        element_id for element_id in element_ids if id_counts[element_id] > 1
    )
    if missing or ambiguous:
        raise ValueError(
            "compiled graph SVG selectors must resolve exactly once; "
            f"missing={missing!r}, ambiguous={ambiguous!r}"
        )
    return {
        "graph_link_count": len(expected["graphical_artifact_link_refs"]),
        "resolved_svg_identity_count": len(element_ids),
    }


def schematic_svg_enrichment_payload(
    design_payload: dict[str, Any],
    *,
    source_path: object = "",
    sheet_name: object = "",
    sheet_path: object = "",
    sheet_instance_path: object = "",
    profile: object = "enriched",
    compiled_schematic_graph: dict[str, Any] | None = None,
    schematic_instance: object | None = None,
    compiled_graph_artifact: str = "",
) -> dict[str, Any]:
    """Return document-level metadata embedded in enriched schematic SVG."""

    payload = {
        "schema": KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA,
        "source": {
            "kicad_sch_file": str(source_path or ""),
        },
        "view": {
            "kind": "schematic_sheet",
            "profile": str(profile),
            "sheet_name": str(sheet_name or ""),
            "sheet_path": str(sheet_path or ""),
            "sheet_instance_path": str(sheet_instance_path or ""),
        },
        "view_indexes": schematic_svg_view_indexes(
            design_payload,
            sheet_path=sheet_path,
            sheet_instance_path=sheet_instance_path,
        ),
        "design": design_payload,
    }
    if compiled_schematic_graph is not None or schematic_instance is not None:
        if compiled_schematic_graph is None or schematic_instance is None:
            raise ValueError(
                "compiled_schematic_graph and schematic_instance must be provided together"
            )
        payload["compiled_schematic_graph_view"] = compiled_schematic_graph_page_view(
            compiled_schematic_graph,
            schematic_instance,
            graph_artifact=compiled_graph_artifact,
        )
    return payload


def schematic_root_svg_attrs(
    *,
    source_path: object = "",
    sheet_name: object = "",
    sheet_path: object = "",
    profile: object = "enriched",
    compiled_graph_view: dict[str, Any] | None = None,
) -> dict[str, object]:
    """Return root SVG attributes for enriched schematic output."""

    attrs: dict[str, object] = {
        "data-enrichment-schema": KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA,
        "data-view-kind": "schematic_sheet",
        "data-profile": str(profile),
        "data-source": str(source_path or ""),
        "data-sheet-name": str(sheet_name or ""),
        "data-sheet-path": str(sheet_path or ""),
    }
    if compiled_graph_view is not None:
        attrs.update(
            {
                "data-compiled-graph-schema": compiled_graph_view["graph_schema"],
                "data-compiled-graph-view-schema": compiled_graph_view["schema"],
                "data-compiled-graph-page-occurrence-ref": compiled_graph_view[
                    "page_occurrence_ref"
                ],
                "data-compiled-graph-artifact-key": compiled_graph_view["artifact_key"],
                "data-compiled-graph-linkage-contract": compiled_graph_view[
                    "linkage_contract"
                ],
            }
        )
    return attrs


def schematic_svg_enrichment_metadata_element(payload: dict[str, Any]) -> str:
    body = html.escape(json.dumps(payload, indent=2, sort_keys=True), quote=False)
    return (
        f'<metadata id="{KICAD_SCHEMATIC_SVG_ENRICHMENT_METADATA_ID}" '
        f'data-schema="{KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA}">\n'
        f"{body}\n"
        "</metadata>"
    )

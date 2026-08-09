"""KiCad-native JSON payload builders for :class:`KiCadDesign`.

The payloads in this module describe KiCad projects, hierarchy, components,
nets, variants, and optional lookup indexes using KiCad-owned schema IDs. The
schemas are owned by kicad_monkey and do not depend on external model packages.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import TYPE_CHECKING, Any, Iterable

from .kicad_compiled_schematic_graph import build_compiled_schematic_graph
from .kicad_netlist_model import (
    KiCadNet,
    KiCadNetEndpoint,
    KiCadNetlist,
    KiCadNetlistComponent,
    KiCadNetlistTerminal,
)
from .kicad_schematic_connectivity import SCH_IU_PER_MM
from .kicad_schematic_occurrence import walk_schematic_occurrences

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .kicad_design import KiCadDesign
    from .kicad_sch_sheet import SchSheet
    from .kicad_schematic import KiCadSchematic


KICAD_DESIGN_JSON_SCHEMA = "kicad_monkey.design.a0"
KICAD_NETLIST_JSON_SCHEMA = "kicad_monkey.netlist.a0"
KICAD_SCHEMATIC_HIERARCHY_SCHEMA = "kicad_monkey.schematic_hierarchy.a0"
KICAD_DESIGN_JSON_GENERATOR = "kicad_monkey"


_PREFIX_TO_TYPE = {
    "R": "passive_2pin",
    "C": "passive_2pin",
    "L": "passive_2pin",
    "D": "passive_2pin",
    "LED": "passive_2pin",
    "U": "ic",
    "IC": "ic",
    "J": "connector",
    "P": "connector",
    "CON": "connector",
    "Q": "transistor",
    "T": "transformer",
    "TR": "transformer",
    "Y": "crystal",
    "X": "crystal",
    "F": "fuse",
    "S": "switch",
    "SW": "switch",
    "K": "relay",
    "RY": "relay",
    "TP": "test_point",
    "FID": "fiducial",
    "MH": "mounting_hole",
}

_GRAPHICAL_ID_KEYS = (
    "wires",
    "junctions",
    "labels",
    "power_ports",
    "ports",
    "sheet_entries",
)


def kicad_design_to_json(design: "KiCadDesign", *, include_indexes: bool = True) -> dict:
    """Build a KiCad-native design payload."""
    netlist = design.to_netlist()
    component_svg_ids = _component_svg_ids(netlist)
    component_pin_counts = _component_pin_counts(netlist)
    components = [
        _component_json(
            comp,
            pin_count=component_pin_counts.get(comp.reference, 0),
            svg_id=component_svg_ids.get(comp.reference, ""),
        )
        for comp in netlist.components
    ]

    result: dict[str, Any] = {
        "schema": KICAD_DESIGN_JSON_SCHEMA,
        "generator": KICAD_DESIGN_JSON_GENERATOR,
        "project": _project_json(design),
        "variants": _variants_json(design),
        "options": _options_json(design),
        "sheets": _sheets_json(design, netlist),
        "components": components,
        "schematic_hierarchy": _schematic_hierarchy_json(design),
        "nets": _nets_json(netlist, component_svg_ids=component_svg_ids),
        "compiled_schematic_graph": build_compiled_schematic_graph(design).to_json(),
    }

    pnp = _pnp_json(design, netlist)
    if pnp is not None:
        result["pnp"] = pnp

    net_classes = _net_classes_json(netlist)
    if net_classes:
        result["net_classes"] = net_classes
        result["net_name_to_classes"] = _net_name_to_classes(netlist)

    if include_indexes:
        result["indexes"] = _indexes_json(netlist, components)

    return result


def kicad_netlist_to_json(netlist: KiCadNetlist) -> dict:
    """Build a KiCad-native raw netlist payload."""
    component_svg_ids = _component_svg_ids(netlist)
    return {
        "schema": KICAD_NETLIST_JSON_SCHEMA,
        "generator": KICAD_DESIGN_JSON_GENERATOR,
        "components": [_raw_component_json(comp) for comp in netlist.components],
        "nets": _nets_json(netlist, component_svg_ids=component_svg_ids),
        "net_classes": _net_classes_json(netlist),
        "design": _netlist_design_metadata_json(netlist),
    }


def _project_json(design: "KiCadDesign") -> dict[str, Any]:
    project = design.project
    project_path = design.project_path
    if project is None:
        return {
            "name": project_path.stem if project_path else None,
            "filename": project_path.name if project_path else None,
            "path": str(project_path) if project_path else None,
            "text_variables": {},
        }
    return {
        "name": project_path.stem if project_path else None,
        "filename": project_path.name if project_path else None,
        "path": str(project_path) if project_path else None,
        "text_variables": dict(sorted(project.text_variables.items())),
    }


def _options_json(design: "KiCadDesign") -> dict[str, Any]:
    project = design.project
    return {
        "net_identifier_scope": "KICAD_PROJECT",
        "allow_ports_to_name_nets": True,
        "allow_sheet_entries_to_name_nets": True,
        "allow_single_pin_nets": True,
        "append_sheet_numbers_to_local_nets": False,
        "power_port_names_take_priority": True,
        "higher_level_names_take_priority": True,
        "auto_sheet_numbering": True,
        "kicad_schematic_format": "sexpr",
        "kicad_supported_oracle_versions": ["9", "10"],
        "kicad_subpart_first_id": (
            project.get_path("schematic.subpart_first_id") if project else None
        ),
        "kicad_subpart_id_separator": (
            project.get_path("schematic.subpart_id_separator") if project else None
        ),
    }


def _variants_json(design: "KiCadDesign") -> list[dict[str, Any]]:
    project = design.project
    if project is None or not project.variants:
        return []

    top = design.top_schematic
    variants: list[dict[str, Any]] = []
    for variant in project.variants:
        entry: dict[str, Any] = {
            "name": variant.name,
            "dnp": [],
            "parameter_overrides": {},
            "kicad_project_variant": {
                "name": variant.name,
                "description": variant.description,
            },
        }
        if variant.description is not None:
            entry["description"] = variant.description
        if top is not None:
            dnp, overrides = _schematic_variant_effects(top, variant.name)
            entry["dnp"] = dnp
            if overrides:
                entry["parameter_overrides"] = overrides
            else:
                entry.pop("parameter_overrides", None)
        variants.append(entry)
    return variants


def _schematic_variant_effects(
    top: "KiCadSchematic",
    variant_name: str,
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    from .kicad_variants import resolve_symbol

    dnp: list[str] = []
    overrides: dict[str, dict[str, Any]] = {}
    for sym, sheet_path, _owner in top.walk_symbols():
        base = resolve_symbol(sym, None, sheet_path=sheet_path)
        effective = resolve_symbol(sym, variant_name, sheet_path=sheet_path)
        ref = effective.reference or base.reference
        if not ref:
            continue
        if effective.dnp:
            dnp.append(ref)
        changed = {
            str(key): value
            for key, value in effective.fields.items()
            if base.fields.get(key) != value
        }
        if effective.value != base.value:
            changed.setdefault("Value", effective.value)
        if changed:
            overrides[ref] = changed
    return sorted(set(dnp)), dict(sorted(overrides.items()))


def _pnp_json(design: "KiCadDesign", netlist: KiCadNetlist) -> dict[str, Any] | None:
    pcb = design.pcb
    if pcb is None or not getattr(pcb, "footprints", None):
        return None

    component_by_ref = {comp.reference: comp for comp in netlist.components}
    placements: list[dict[str, Any]] = []
    for footprint in pcb.footprints:
        properties = _footprint_properties(footprint)
        designator = properties.get("Reference", "")
        if not designator:
            continue
        comp = component_by_ref.get(designator)
        parameters = _component_parameters_json(comp) if comp is not None else properties
        value = comp.value if comp is not None else properties.get("Value", "")
        description = comp.libsource_description if comp is not None else (footprint.descr or "")
        placements.append(
            {
                "designator": designator,
                "comment": value,
                "layer": _pnp_layer(footprint.layer),
                "footprint": footprint.library_link,
                "center_x": round(float(footprint.at_x), 4),
                "center_y": round(float(footprint.at_y), 4),
                "rotation": round(float(footprint.at_angle), 4),
                "description": description,
                "parameters": dict(sorted(parameters.items())),
                "kicad_uuid": footprint.uuid or "",
            }
        )

    if not placements:
        return None
    return {
        "units": "mm",
        "source_pcb": design.pcb_path.name if design.pcb_path else "",
        "placements": sorted(placements, key=lambda row: row["designator"]),
    }


def _footprint_properties(footprint) -> dict[str, str]:
    out: dict[str, str] = {}
    for prop in getattr(footprint, "properties", []) or []:
        name = str(getattr(prop, "name", "") or "")
        if not name:
            continue
        out[name] = str(getattr(prop, "value", "") or "")
    return dict(sorted(out.items()))


def _pnp_layer(layer: str) -> str:
    text = str(layer or "")
    if text.startswith("B."):
        return "bottom"
    if text.startswith("F."):
        return "top"
    return text


def _sheets_json(design: "KiCadDesign", netlist: KiCadNetlist) -> list[dict[str, Any]]:
    source_by_path = _sheet_source_path_by_human_path(design.top_schematic)
    out: list[dict[str, Any]] = []
    for sheet in netlist.design_metadata.sheets:
        source_path = source_by_path.get(sheet.name)
        out.append(
            {
                "filename": source_path.name if source_path else "",
                "path": str(source_path) if source_path else "",
                "sheet_number": int(sheet.number),
                "sheet_path": sheet.name,
                "sheet_path_uuids": sheet.tstamps,
                "title": sheet.title,
                "company": sheet.company,
                "revision": sheet.revision,
                "date": sheet.date,
            }
        )
    return out


def _schematic_hierarchy_json(design: "KiCadDesign") -> dict[str, Any]:
    top = design.top_schematic
    documents: list[dict[str, Any]] = []
    sheet_symbols: list[dict[str, Any]] = []
    links: list[dict[str, Any]] = []
    unresolved: list[dict[str, Any]] = []

    occurrences = list(walk_schematic_occurrences(top)) if top is not None else []
    children = {
        (id(occurrence.parent), id(occurrence.sheet_symbol)): occurrence
        for occurrence in occurrences
        if occurrence.parent is not None and occurrence.sheet_symbol is not None
    }

    for occurrence in occurrences:
        sch = occurrence.schematic
        source_path = getattr(sch, "source_path", None)
        documents.append(
            {
                "sheet_index": len(documents) + 1,
                "filename": source_path.name if isinstance(source_path, Path) else "",
                "path": str(source_path) if isinstance(source_path, Path) else "",
                "is_top_level": occurrence.parent is None,
                "sheet_path": occurrence.sheet_path,
                "sheet_path_uuids": occurrence.sheet_path_uuids,
                "metadata": {
                    "uuid": getattr(sch, "uuid", "") or "",
                    "version": int(getattr(sch, "version", 0) or 0),
                    "generator": getattr(sch, "generator", "") or "",
                    "generator_version": getattr(sch, "generator_version", "") or "",
                },
            }
        )
        for sheet in getattr(sch, "sheets", []) or []:
            child = children.get((id(occurrence), id(sheet)))
            child_path = (
                child.sheet_path
                if child is not None
                else _join_sheet_path(
                    occurrence.sheet_path,
                    sheet.sheet_name or sheet.sheet_file,
                )
            )
            child_uuid_path = (
                child.sheet_path_uuids
                if child is not None
                else _join_sheet_path(
                    occurrence.sheet_path_uuids,
                    sheet.uuid or sheet.sheet_file,
                )
            )
            row = _sheet_symbol_json(
                sheet,
                source_sheet_path=occurrence.sheet_path,
                child_sheet_path=child_path,
                child_sheet_path_uuids=child_uuid_path,
            )
            sheet_symbols.append(row)
            if child is None:
                unresolved.append(row)
                continue
            links.append(
                {
                    "parent_sheet_path": occurrence.sheet_path,
                    "sheet_symbol_uid": sheet.uuid or "",
                    "child_sheet_path": child_path,
                    "child_filename": sheet.sheet_file,
                }
            )

    return {
        "schema": KICAD_SCHEMATIC_HIERARCHY_SCHEMA,
        "requested_scope": "KICAD_PROJECT",
        "effective_scope": "HIERARCHICAL" if sheet_symbols else "GLOBAL",
        "documents": documents,
        "sheet_symbols": sheet_symbols,
        "hierarchy_paths": [],
        "channels": [],
        "links": links,
        "unresolved": unresolved,
    }


def _sheet_symbol_json(
    sheet: "SchSheet",
    *,
    source_sheet_path: str,
    child_sheet_path: str,
    child_sheet_path_uuids: str,
) -> dict[str, Any]:
    return {
        "uid": sheet.uuid or "",
        "name": sheet.sheet_name,
        "child_filename": sheet.sheet_file,
        "source_sheet_path": source_sheet_path,
        "child_sheet_path": child_sheet_path,
        "child_sheet_path_uuids": child_sheet_path_uuids,
        "entries": [
            {
                "name": pin.name,
                "uid": pin.uuid,
                "shape": getattr(getattr(pin, "shape", None), "value", "")
                or str(getattr(pin, "shape", "") or ""),
            }
            for pin in getattr(sheet, "pins", []) or []
        ],
    }


def _sheet_source_path_by_human_path(
    top: "KiCadSchematic | None",
) -> dict[str, Path]:
    out: dict[str, Path] = {}
    if top is not None:
        for occurrence in walk_schematic_occurrences(top):
            source_path = getattr(occurrence.schematic, "source_path", None)
            if isinstance(source_path, Path):
                out.setdefault(occurrence.sheet_path, source_path)
    return out


def _join_sheet_path(parent: str, child: str) -> str:
    parent = parent or "/"
    if not parent.endswith("/"):
        parent += "/"
    child = str(child or "").strip("/")
    return parent if not child else f"{parent}{child}/"


def _component_svg_ids(netlist: KiCadNetlist) -> dict[str, str]:
    return {
        comp.reference: comp.instance_uuid
        for comp in netlist.components
        if comp.reference and comp.instance_uuid
    }


def _component_json(
    comp: KiCadNetlistComponent,
    *,
    pin_count: int,
    svg_id: str,
) -> dict[str, Any]:
    return {
        "designator": comp.reference,
        "svg_id": svg_id,
        "value": comp.value,
        "footprint": comp.footprint,
        "library_ref": _library_ref(comp),
        "description": comp.libsource_description,
        "hierarchy": _component_hierarchy_json(comp),
        "classification": _component_classification_json(
            comp.reference,
            pin_count=pin_count,
        ),
        "parameters": _component_parameters_json(comp),
    }


def _raw_component_json(comp: KiCadNetlistComponent) -> dict[str, Any]:
    return {
        "designator": comp.reference,
        "value": comp.value,
        "footprint": comp.footprint,
        "library_ref": _library_ref(comp),
        "description": comp.libsource_description,
        "parameters": _component_parameters_json(comp),
    }


def _library_ref(comp: KiCadNetlistComponent) -> str:
    if comp.libsource_lib and comp.libsource_part:
        return f"{comp.libsource_lib}:{comp.libsource_part}"
    return comp.libsource_part or comp.libsource_lib


def _component_parameters_json(comp: KiCadNetlistComponent) -> dict[str, str]:
    out: dict[str, str] = dict(sorted((comp.properties or {}).items()))
    out.setdefault("_source_cad", "kicad")
    if comp.libsource_lib:
        out["kicad_libsource_lib"] = comp.libsource_lib
    if comp.libsource_part:
        out["kicad_libsource_part"] = comp.libsource_part
    if comp.sheet_path_names:
        out["kicad_sheet_path_names"] = comp.sheet_path_names
    if comp.sheet_path_uuids:
        out["kicad_sheet_path_uuids"] = comp.sheet_path_uuids
    if comp.instance_uuid:
        out["kicad_instance_uuid"] = comp.instance_uuid
    out["kicad_in_bom"] = "true" if comp.in_bom else "false"
    out["kicad_on_board"] = "true" if comp.on_board else "false"
    out["kicad_dnp"] = "true" if comp.dnp else "false"
    return dict(sorted(out.items()))


def _component_hierarchy_json(comp: KiCadNetlistComponent) -> dict[str, Any]:
    return {
        "base_designator": comp.reference,
        "channel": None,
        "channel_index": None,
        "sheet": comp.sheet_path_names,
        "sheet_path": comp.sheet_path_names,
        "sheet_path_uuids": comp.sheet_path_uuids,
    }


def _component_classification_json(designator: str, *, pin_count: int) -> dict[str, Any]:
    match = re.match(r"^([A-Za-z]+)", designator or "")
    prefix = match.group(1).upper() if match else ""
    return {
        "prefix": prefix,
        "type": _PREFIX_TO_TYPE.get(prefix, "unknown"),
        "pin_count": int(pin_count),
    }


def _component_pin_counts(netlist: KiCadNetlist) -> dict[str, int]:
    pins_by_ref: dict[str, set[str]] = {}
    for net in netlist.nets:
        for term in net.terminals:
            pins_by_ref.setdefault(term.designator, set()).add(term.pin)
    return {
        reference: len(pins)
        for reference, pins in pins_by_ref.items()
    }


def _nets_json(
    netlist: KiCadNetlist,
    *,
    component_svg_ids: dict[str, str],
) -> list[dict[str, Any]]:
    return [
        _net_json(net, index=index, component_svg_ids=component_svg_ids)
        for index, net in enumerate(netlist.nets, start=1)
    ]


def _net_json(
    net: KiCadNet,
    *,
    index: int,
    component_svg_ids: dict[str, str],
) -> dict[str, Any]:
    return {
        "uid": f"{index:012x}",
        "name": net.name,
        "auto_named": bool(net.auto_named),
        "source_sheets": _sorted_unique(term.sheet_path for term in net.terminals),
        "terminals": [_terminal_json(term) for term in _sorted_terminals(net.terminals)],
        "graphical": _net_graphical_json(net, component_svg_ids),
        "aliases": sorted(set(net.aliases or [])),
        "endpoints": _net_endpoints_json(net, component_svg_ids),
        "driver_priority": int(net.driver_priority),
        "driver_kind": str(net.driver_kind),
        "net_class": net.net_class,
    }


def _terminal_json(term: KiCadNetlistTerminal) -> dict[str, str]:
    return {
        "designator": term.designator,
        "pin": term.pin,
        "pin_name": term.pin_name,
        "pin_type": _pin_type_json(term.pin_type),
    }


def _net_graphical_json(
    net: KiCadNet,
    component_svg_ids: dict[str, str],
) -> dict[str, Any]:
    graphical = {
        key: _sorted_unique((net.graphical or {}).get(key, ()))
        for key in _GRAPHICAL_ID_KEYS
    }
    pins = []
    seen: set[tuple[str, str, str]] = set()
    for term in _sorted_terminals(net.terminals):
        svg_id = term.svg_id or component_svg_ids.get(term.designator, "")
        if not svg_id:
            continue
        key = (term.designator, term.pin, svg_id)
        if key in seen:
            continue
        seen.add(key)
        row = {"designator": term.designator, "pin": term.pin, "svg_id": svg_id}
        if term.source_pin_id and term.source_pin_id != svg_id:
            row["source_pin_id"] = term.source_pin_id
        pins.append(row)
    graphical["pins"] = pins
    return graphical


def _net_endpoints_json(
    net: KiCadNet,
    component_svg_ids: dict[str, str],
) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = [
        _semantic_endpoint_json(endpoint) for endpoint in net.endpoints
    ]
    for term in _sorted_terminals(net.terminals):
        svg_id = term.svg_id or component_svg_ids.get(term.designator, "")
        source_pin_id = term.source_pin_id or ""
        endpoint_id = f"pin:{term.designator}:{term.pin}"
        row: dict[str, Any] = {
            "endpoint_id": endpoint_id,
            "role": "pin",
            "element_id": svg_id,
            "object_id": source_pin_id or svg_id,
            "name": term.pin_name or endpoint_id,
            "source_sheet": term.sheet_path,
            "designator": term.designator,
            "pin": term.pin,
            "pin_type": _pin_type_json(term.pin_type),
        }
        if term.pin_name:
            row["pin_name"] = term.pin_name
        out.append(row)
    return _unique_sorted_dicts(out, _endpoint_sort_key)


def _semantic_endpoint_json(endpoint: KiCadNetEndpoint) -> dict[str, Any]:
    row: dict[str, Any] = {
        "endpoint_id": endpoint.endpoint_id,
        "role": endpoint.role,
        "element_id": endpoint.element_id,
        "object_id": endpoint.object_id,
        "name": endpoint.name,
        "source_sheet": endpoint.source_sheet,
    }
    if endpoint.connection_point is not None:
        row["connection_point"] = {
            "x": _schematic_iu_to_mm(endpoint.connection_point[0]),
            "y": _schematic_iu_to_mm(endpoint.connection_point[1]),
            "units": "mm",
        }
    return row


def _schematic_iu_to_mm(value: int) -> float:
    """Convert snapped schematic internal units to KiCad schematic millimetres."""
    return round(int(value) / SCH_IU_PER_MM, 4)


def _endpoint_sort_key(endpoint: dict[str, Any]) -> tuple[object, ...]:
    point = endpoint.get("connection_point")
    point_data = point if isinstance(point, dict) else {}
    return (
        str(endpoint.get("endpoint_id", "")),
        str(endpoint.get("role", "")),
        str(endpoint.get("element_id", "")),
        str(endpoint.get("object_id", "")),
        str(endpoint.get("name", "")),
        str(endpoint.get("source_sheet", "")),
        str(endpoint.get("designator", "")),
        str(endpoint.get("pin", "")),
        str(endpoint.get("pin_name", "")),
        str(endpoint.get("pin_type", "")),
        int(point_data.get("x", -1)) if isinstance(point_data.get("x"), int) else -1,
        int(point_data.get("y", -1)) if isinstance(point_data.get("y"), int) else -1,
    )


def _unique_sorted_dicts(
    values: Iterable[dict[str, Any]],
    key_func,
) -> list[dict[str, Any]]:
    by_key: dict[tuple[object, ...], dict[str, Any]] = {}
    for value in values:
        by_key.setdefault(key_func(value), value)
    return [by_key[key] for key in sorted(by_key)]


def _pin_type_json(value: str) -> str:
    text = str(value or "passive").strip()
    return text.upper() if text else "PASSIVE"


def _sorted_terminals(
    terminals: Iterable[KiCadNetlistTerminal],
) -> list[KiCadNetlistTerminal]:
    return sorted(terminals, key=lambda t: (t.designator, t.pin, t.pin_name, t.pin_type))


def _sorted_unique(values: Iterable[str]) -> list[str]:
    return sorted({str(value) for value in values if str(value)})


def _indexes_json(netlist: KiCadNetlist, components: list[dict[str, Any]]) -> dict:
    svg_to_component = {
        str(comp.get("svg_id")): str(comp.get("designator"))
        for comp in components
        if comp.get("svg_id") and comp.get("designator")
    }

    component_to_nets: dict[str, list[str]] = {}
    net_to_components: dict[str, list[str]] = {}
    svg_to_net_candidates: dict[str, set[str]] = {}
    sheet_svg_to_net_candidates: dict[str, dict[str, set[str]]] = {}
    net_to_graphics: dict[str, list[str]] = {}

    def add_svg_net(svg_id: object, net_name: str) -> None:
        text = str(svg_id or "")
        if text:
            svg_to_net_candidates.setdefault(text, set()).add(net_name)

    def add_sheet_svg_net(sheet_path: object, svg_id: object, net_name: str) -> None:
        sheet = str(sheet_path or "")
        text = str(svg_id or "")
        if sheet and text:
            sheet_svg_to_net_candidates.setdefault(sheet, {}).setdefault(text, set()).add(
                net_name
            )

    for net in netlist.nets:
        designators = sorted({term.designator for term in net.terminals if term.designator})
        net_to_components[net.name] = designators
        for designator in designators:
            component_to_nets.setdefault(designator, []).append(net.name)
        graphics: list[str] = []
        for key in _GRAPHICAL_ID_KEYS:
            for svg_id in (net.graphical or {}).get(key, ()):
                svg_id = str(svg_id or "")
                if not svg_id:
                    continue
                graphics.append(svg_id)
                add_svg_net(svg_id, net.name)
        for term in net.terminals:
            for svg_id in (term.svg_id, term.source_pin_id):
                svg_id = str(svg_id or "")
                if not svg_id:
                    continue
                graphics.append(svg_id)
                add_svg_net(svg_id, net.name)
                add_sheet_svg_net(term.sheet_path, svg_id, net.name)
        for endpoint in net.endpoints:
            add_sheet_svg_net(endpoint.source_sheet, endpoint.element_id, net.name)
            add_sheet_svg_net(endpoint.source_sheet, endpoint.object_id, net.name)
        if graphics:
            net_to_graphics[net.name] = sorted(set(graphics))

    svg_to_nets = {
        svg_id: sorted(net_names)
        for svg_id, net_names in sorted(svg_to_net_candidates.items())
    }
    sheet_svg_to_nets = {
        sheet: {
            svg_id: sorted(net_names)
            for svg_id, net_names in sorted(sheet_map.items())
        }
        for sheet, sheet_map in sorted(sheet_svg_to_net_candidates.items())
    }

    return {
        "svg_to_component": svg_to_component,
        "component_to_nets": {
            key: sorted(set(values)) for key, values in sorted(component_to_nets.items())
        },
        "net_to_components": dict(sorted(net_to_components.items())),
        "svg_to_net": {
            svg_id: net_names[0]
            for svg_id, net_names in svg_to_nets.items()
            if len(net_names) == 1
        },
        "svg_to_nets": svg_to_nets,
        "sheet_svg_to_nets": sheet_svg_to_nets,
        "net_to_graphics": dict(sorted(net_to_graphics.items())),
    }


def _net_classes_json(netlist: KiCadNetlist) -> list[dict[str, Any]]:
    assigned: dict[str, list[str]] = {cls.name: [] for cls in netlist.net_classes}
    for net in netlist.nets:
        if net.net_class:
            assigned.setdefault(net.net_class, []).append(net.name)
    return [
        {
            "name": cls.name,
            "description": cls.description,
            "nets": sorted(set(assigned.get(cls.name, []))),
        }
        for cls in netlist.net_classes
    ]


def _net_name_to_classes(netlist: KiCadNetlist) -> dict[str, list[str]]:
    return {
        net.name: [net.net_class]
        for net in netlist.nets
        if net.net_class
    }


def _netlist_design_metadata_json(netlist: KiCadNetlist) -> dict[str, Any]:
    return {
        "source": netlist.design_metadata.source,
        "date": netlist.design_metadata.date,
        "tool": netlist.design_metadata.tool,
        "sheets": [
            {
                "number": sheet.number,
                "name": sheet.name,
                "tstamps": sheet.tstamps,
                "title": sheet.title,
                "company": sheet.company,
                "revision": sheet.revision,
                "date": sheet.date,
            }
            for sheet in netlist.design_metadata.sheets
        ],
    }


__all__ = [
    "KICAD_DESIGN_JSON_GENERATOR",
    "KICAD_DESIGN_JSON_SCHEMA",
    "KICAD_NETLIST_JSON_SCHEMA",
    "KICAD_SCHEMATIC_HIERARCHY_SCHEMA",
    "kicad_design_to_json",
    "kicad_netlist_to_json",
]

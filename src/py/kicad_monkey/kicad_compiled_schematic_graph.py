"""KiCad-owned producer for the CAD-neutral compiled schematic graph."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Iterable
from uuid import UUID

from .kicad_netlist_compiler import (
    _is_power_symbol,
    _resolve_instance_reference,
)
from .kicad_netlist_design import (
    _build_legacy_instance_lookup,
    _build_legacy_unit_lookup,
    _canonical_instance_path,
    _resolve_instance_unit,
    compile_design_subgraphs,
    merge_design_nets,
)
from .kicad_netlist_model import KiCadDriverKind

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .kicad_design import KiCadDesign


KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA = "kicad_monkey.compiled_schematic_graph.a0"
KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE = "sch.compiled_schematic_graph"
KICAD_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE = "sch.compiled_schematic_graph.a0"
_IDENTITY_EPOCH_MS = 1_786_060_800_000


def _canonical_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def _stable_uuidv7(scope: dict[str, str], object_type: str, identity: object) -> str:
    address = _canonical_json(
        {
            "namespace": KICAD_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE,
            "design_scope": scope,
            "object_type": object_type,
            "identity": identity,
        }
    )
    digest = hashlib.sha256(address.encode("utf-8")).digest()
    value = bytearray(16)
    value[:6] = _IDENTITY_EPOCH_MS.to_bytes(6, "big")
    value[6] = 0x70 | (digest[0] & 0x0F)
    value[7] = digest[1]
    value[8] = 0x80 | (digest[2] & 0x3F)
    value[9:] = digest[3:10]
    return str(UUID(bytes=bytes(value)))


def _row(
    scope: dict[str, str],
    object_type: str,
    identity: object,
    **fields: object,
) -> dict[str, object]:
    return {
        "type": object_type,
        "id": _stable_uuidv7(scope, object_type, identity),
        **fields,
    }


def _source_path(design: "KiCadDesign", schematic: object) -> str:
    value = getattr(schematic, "source_path", None)
    if value is None:
        return ""
    path = Path(str(value))
    project_path = getattr(design, "project_path", None)
    top_source = getattr(getattr(design, "top_schematic", None), "source_path", None)
    anchor = (
        Path(project_path).resolve().parent
        if project_path is not None
        else (Path(str(top_source)).resolve().parent if top_source is not None else None)
    )
    if anchor is not None:
        try:
            path = path.resolve().relative_to(anchor)
        except (OSError, ValueError):
            pass
    return path.as_posix()


def _source_identity(**values: object) -> dict[str, object]:
    return {
        key: value
        for key, value in sorted(values.items())
        if value not in (None, "", [], {})
    }


def _source_designator(symbol: object) -> str:
    for prop in getattr(symbol, "properties", ()) or ():
        if getattr(prop, "key", "") == "Reference":
            return str(getattr(prop, "value", "") or "")
    return ""


def _local_net_key(page_address: str, subgraph: object) -> str:
    tokens: list[str] = []
    for driver in getattr(subgraph, "pin_drivers", ()) or ():
        tokens.append(
            str(getattr(driver, "source_uuid", "") or "")
            or (
                f"symbol:{getattr(driver, 'svg_uuid', '')}:"
                f"pin:{getattr(driver, 'pin_number', '')}"
            )
        )
    for driver in getattr(subgraph, "label_drivers", ()) or ():
        tokens.append(
            str(getattr(driver, "source_uuid", "") or "")
            or f"label:{getattr(driver, 'kind', '')}:{getattr(driver, 'name', '')}"
        )
    for values in (getattr(subgraph, "graphical", {}) or {}).values():
        tokens.extend(str(value) for value in values if value)
    if not tokens:
        tokens.extend(f"coord:{x}:{y}" for x, y in sorted(subgraph.coords))
    digest = hashlib.sha256("\n".join(sorted(set(tokens))).encode()).hexdigest()[:24]
    return f"{page_address}#local-net:{digest}"


@dataclass
class KiCadCompiledSchematicGraph:
    """Versioned embedded graph payload with Appz-compatible row cardinalities."""

    unit_definitions: list[dict[str, object]] = field(default_factory=list)
    page_definitions: list[dict[str, object]] = field(default_factory=list)
    unit_occurrences: list[dict[str, object]] = field(default_factory=list)
    page_occurrences: list[dict[str, object]] = field(default_factory=list)
    hierarchy_occurrences: list[dict[str, object]] = field(default_factory=list)
    component_occurrences: list[dict[str, object]] = field(default_factory=list)
    local_net_occurrences: list[dict[str, object]] = field(default_factory=list)
    terminal_occurrences: list[dict[str, object]] = field(default_factory=list)
    hierarchy_terminal_bindings: list[dict[str, object]] = field(default_factory=list)
    graphical_artifact_links: list[dict[str, object]] = field(default_factory=list)

    def to_json(self) -> dict[str, object]:
        return {
            "schema": KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA,
            "type": KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE,
            "identity_namespace": KICAD_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE,
            **{
                name: list(getattr(self, name))
                for name in (
                    "unit_definitions",
                    "page_definitions",
                    "unit_occurrences",
                    "page_occurrences",
                    "hierarchy_occurrences",
                    "component_occurrences",
                    "local_net_occurrences",
                    "terminal_occurrences",
                    "hierarchy_terminal_bindings",
                    "graphical_artifact_links",
                )
            },
        }

    @classmethod
    def from_json(cls, payload: dict[str, object]) -> "KiCadCompiledSchematicGraph":
        validate_compiled_schematic_graph(payload)
        names = (
            "unit_definitions",
            "page_definitions",
            "unit_occurrences",
            "page_occurrences",
            "hierarchy_occurrences",
            "component_occurrences",
            "local_net_occurrences",
            "terminal_occurrences",
            "hierarchy_terminal_bindings",
            "graphical_artifact_links",
        )
        return cls(
            **{name: [dict(row) for row in payload.get(name, [])] for name in names}
        )


def build_compiled_schematic_graph(
    design: "KiCadDesign",
) -> KiCadCompiledSchematicGraph:
    """Compile the complete, variant-neutral schematic graph for ``design``."""
    top = design.top_schematic
    if top is None:
        return KiCadCompiledSchematicGraph()
    project = getattr(design, "project_path", None)
    source = getattr(top, "source_path", None)
    project_file = (
        Path(project).name
        if project is not None
        else (Path(str(source)).name if source is not None else "schematic.kicad_sch")
    )
    scope = {"source_cad": "kicad", "project_file": project_file.casefold()}
    graph = KiCadCompiledSchematicGraph()
    compiled = compile_design_subgraphs(top, include_off_board=True)
    merge_design_nets(compiled)
    legacy_refs = _build_legacy_instance_lookup(compiled)
    legacy_units = _build_legacy_unit_lookup(compiled)

    definition_by_key: dict[str, tuple[str, str]] = {}
    occurrence_rows: dict[int, tuple[dict[str, object], dict[str, object]]] = {}
    component_by_symbol: dict[tuple[str, str], str] = {}
    terminal_by_source: dict[tuple[str, str, str], str] = {}

    for cs in compiled:
        occurrence = cs.occurrence
        assert occurrence is not None and cs.schematic is not None
        source_path = _source_path(design, cs.schematic)
        source_uuid = str(getattr(cs.schematic, "uuid", "") or "")
        definition_key = source_path or source_uuid or f"object:{id(cs.schematic)}"
        definition = definition_by_key.get(definition_key)
        if definition is None:
            unit_source = _source_identity(
                **{
                    "sch.source_key.source_path": source_path,
                    "sch.source_key.source_uuid": source_uuid,
                }
            )
            unit = _row(
                scope,
                "sch.unit_definition",
                unit_source,
                display_name=Path(source_path).stem
                if source_path
                else occurrence.sheet_name,
                page_definition_refs=[],
                source_identity=unit_source,
            )
            page = _row(
                scope,
                "sch.page_definition",
                {"source": unit_source, "page": 1},
                unit_definition_ref=unit["id"],
                display_name=unit["display_name"],
                source_identity=unit_source,
            )
            unit["page_definition_refs"] = [page["id"]]
            graph.unit_definitions.append(unit)
            graph.page_definitions.append(page)
            definition = (str(unit["id"]), str(page["id"]))
            definition_by_key[definition_key] = definition

        address = occurrence.occurrence_address
        source_occurrence_path = occurrence.sheet_path_uuids
        occurrence_source = _source_identity(
            **{
                "sch.source_key.source_path": source_occurrence_path,
                "sch.source_key.source_record": f"instance-path:{address}",
                "sch.source_key.source_uuid": (
                    occurrence.sheet_symbol.uuid
                    if occurrence.sheet_symbol is not None
                    else source_uuid
                ),
            }
        )
        unit_occurrence = _row(
            scope,
            "sch.unit_occurrence",
            occurrence_source,
            unit_definition_ref=definition[0],
            display_name=occurrence.sheet_name,
            page_occurrence_refs=[],
            source_identity=occurrence_source,
        )
        page_occurrence = _row(
            scope,
            "sch.page_occurrence",
            {"occurrence": occurrence_source, "page_definition_ref": definition[1]},
            page_definition_ref=definition[1],
            unit_occurrence_ref=unit_occurrence["id"],
            display_name=occurrence.sheet_name,
            address_key=f"sheet{occurrence.index}",
            sheet_number=str(occurrence.index),
            instance_order=occurrence.index - 1,
            source_identity=occurrence_source,
        )
        unit_occurrence["page_occurrence_refs"] = [page_occurrence["id"]]
        graph.unit_occurrences.append(unit_occurrence)
        graph.page_occurrences.append(page_occurrence)
        occurrence_rows[id(occurrence)] = (unit_occurrence, page_occurrence)

        if occurrence.parent is not None:
            parent_unit, parent_page = occurrence_rows[id(occurrence.parent)]
            hierarchy_source = _source_identity(
                **{
                    "sch.source_key.source_uuid": occurrence.sheet_symbol.uuid,
                    "sch.source_key.source_path": source_occurrence_path,
                    "sch.source_key.source_record": f"instance-path:{address}",
                }
            )
            hierarchy = _row(
                scope,
                "sch.hierarchy_occurrence",
                hierarchy_source,
                parent_unit_occurrence_ref=parent_unit["id"],
                parent_page_occurrence_ref=parent_page["id"],
                child_unit_occurrence_ref=unit_occurrence["id"],
                source_identity=hierarchy_source,
            )
            graph.hierarchy_occurrences.append(hierarchy)
            unit_occurrence["parent_hierarchy_occurrence_ref"] = hierarchy["id"]
            graph.graphical_artifact_links.append(
                _row(
                    scope,
                    "sch.graphical_artifact_link",
                    {
                        "page": parent_page["id"],
                        "target": hierarchy["id"],
                        "element": occurrence.sheet_symbol.uuid,
                    },
                    page_occurrence_ref=parent_page["id"],
                    target_type="sch.hierarchy_occurrence",
                    target_ref=hierarchy["id"],
                    artifact_key="sch.dwg_scene",
                    element_id=occurrence.sheet_symbol.uuid,
                    source_identity=_source_identity(
                        **{
                            "sch.source_key.artifact_element": (
                                occurrence.sheet_symbol.uuid
                            )
                        }
                    ),
                )
            )

        canonical = _canonical_instance_path(top, cs.sheet_path)
        for symbol in getattr(cs.schematic, "symbols", ()) or ():
            lib_symbol = cs.schematic.get_lib_symbol_for_symbol(symbol)
            if _is_power_symbol(symbol, lib_symbol):
                continue
            physical = _resolve_instance_reference(
                symbol,
                cs.sheet_path,
                legacy_refs,
                canonical,
            )
            if not physical or physical.startswith("#"):
                continue
            unit_number = _resolve_instance_unit(
                symbol,
                cs.sheet_path,
                legacy_unit_lookup=legacy_units,
                canonical_path=canonical,
            )
            symbol_uuid = str(getattr(symbol, "uuid", "") or "")
            component_source = _source_identity(
                **{
                    "sch.source_key.source_uuid": symbol_uuid,
                    "sch.source_key.source_path": source_occurrence_path,
                }
            )
            component = _row(
                scope,
                "sch.component_occurrence",
                {"source": component_source, "unit": unit_number},
                page_occurrence_ref=page_occurrence["id"],
                source_designator=_source_designator(symbol),
                physical_designator=physical,
                display_designator=physical,
                unit=max(1, unit_number),
                body_style=int(getattr(symbol, "convert", 0) or 0),
                source_identity=component_source,
            )
            graph.component_occurrences.append(component)
            component_by_symbol[(cs.sheet_path, symbol_uuid)] = str(component["id"])
            if symbol_uuid:
                graph.graphical_artifact_links.append(
                    _row(
                        scope,
                        "sch.graphical_artifact_link",
                        {
                            "page": page_occurrence["id"],
                            "target": component["id"],
                            "element": symbol_uuid,
                        },
                        page_occurrence_ref=page_occurrence["id"],
                        target_type="sch.component_occurrence",
                        target_ref=component["id"],
                        artifact_key="sch.dwg_scene",
                        element_id=symbol_uuid,
                        source_identity=_source_identity(
                            **{"sch.source_key.artifact_element": symbol_uuid}
                        ),
                    )
                )

        pin_element_counts: dict[str, int] = {}
        for candidate_subgraph in cs.subgraphs:
            for candidate_driver in candidate_subgraph.pin_drivers:
                element_id = str(candidate_driver.pin_svg_uuid or "")
                if element_id:
                    pin_element_counts[element_id] = (
                        pin_element_counts.get(element_id, 0) + 1
                    )
        terminal_by_semantic_source: dict[
            tuple[str, str, str, str], dict[str, object]
        ] = {}

        for subgraph_index, subgraph in enumerate(cs.subgraphs):
            local_key = _local_net_key(address, subgraph)
            compiled_net_code = cs.subgraph_net_codes.get(subgraph_index)
            display_name = cs.subgraph_net_names.get(
                subgraph_index, subgraph.chosen_name
            )
            aliases = sorted(
                {
                    str(driver.name)
                    for driver in subgraph.label_drivers
                    if getattr(driver, "name", "")
                }
            )
            local_source = _source_identity(
                **{
                    "sch.source_key.compiled_net": local_key,
                    "sch.source_key.source_record": (
                        f"net-uid:{compiled_net_code:012x}"
                        if compiled_net_code is not None
                        else ""
                    ),
                    "sch.source_key.source_path": source_occurrence_path,
                }
            )
            local = _row(
                scope,
                "sch.local_net_occurrence",
                {"compiled_net": local_key, "page": page_occurrence["id"]},
                page_occurrence_ref=page_occurrence["id"],
                display_name=display_name,
                qualified_name=display_name,
                aliases=aliases,
                source_identity=local_source,
            )
            graph.local_net_occurrences.append(local)

            def add_terminal(
                *,
                role: str,
                source_uuid: str,
                source_subobject: str,
                name: str,
                pin_designator: str,
                component_ref: str | None = None,
                element_id: str = "",
            ) -> None:
                semantic_source_key = (
                    role,
                    component_ref or "",
                    source_uuid,
                    source_subobject,
                )
                existing = terminal_by_semantic_source.get(semantic_source_key)
                if existing is not None:
                    if existing["local_net_occurrence_ref"] != local["id"]:
                        raise ValueError(
                            "one semantic terminal source resolves to multiple local nets"
                        )
                    existing_name = str(existing.get("name", "") or "")
                    if name and (not existing_name or name < existing_name):
                        existing["name"] = name
                    terminal_by_source[(cs.sheet_path, role, source_uuid)] = str(
                        existing["id"]
                    )
                    return
                terminal_source = _source_identity(
                    **{
                        "sch.source_key.source_uuid": source_uuid,
                        "sch.source_key.source_subobject": source_subobject,
                        "sch.source_key.source_path": source_occurrence_path,
                    }
                )
                terminal = _row(
                    scope,
                    "sch.terminal_occurrence",
                    {
                        "source": terminal_source,
                        "role": role,
                        "component": component_ref or "",
                    },
                    page_occurrence_ref=page_occurrence["id"],
                    role=role,
                    local_net_occurrence_ref=local["id"],
                    component_occurrence_ref=component_ref,
                    name=name,
                    pin_designator=pin_designator,
                    resolution_diagnostics=(
                        ["logical_pin_unresolved"] if role == "component_pin" else []
                    ),
                    source_identity=terminal_source,
                )
                if component_ref is None:
                    terminal.pop("component_occurrence_ref")
                graph.terminal_occurrences.append(terminal)
                terminal_by_semantic_source[semantic_source_key] = terminal
                terminal_id = str(terminal["id"])
                terminal_by_source[(cs.sheet_path, role, source_uuid)] = terminal_id
                if element_id:
                    graph.graphical_artifact_links.append(
                        _row(
                            scope,
                            "sch.graphical_artifact_link",
                            {
                                "page": page_occurrence["id"],
                                "target": terminal_id,
                                "element": element_id,
                            },
                            page_occurrence_ref=page_occurrence["id"],
                            target_type="sch.terminal_occurrence",
                            target_ref=terminal_id,
                            artifact_key="sch.dwg_scene",
                            element_id=element_id,
                            source_identity=_source_identity(
                                **{"sch.source_key.artifact_element": element_id}
                            ),
                        )
                    )

            for driver in subgraph.pin_drivers:
                symbol_uuid = str(getattr(driver, "svg_uuid", "") or "")
                if driver.designator.startswith("#"):
                    if driver.is_power:
                        add_terminal(
                            role="power_port",
                            source_uuid=symbol_uuid,
                            source_subobject=str(driver.pin_number),
                            name=driver.power_value or driver.pin_name,
                            pin_designator=str(driver.pin_number),
                            element_id=(
                                driver.pin_svg_uuid
                                if pin_element_counts.get(driver.pin_svg_uuid) == 1
                                else ""
                            ),
                        )
                    continue
                component_ref = component_by_symbol.get((cs.sheet_path, symbol_uuid))
                source_uuid = str(driver.source_uuid or symbol_uuid)
                add_terminal(
                    role="component_pin",
                    source_uuid=source_uuid,
                    source_subobject=str(driver.pin_number),
                    name=str(driver.pin_name or ""),
                    pin_designator=str(driver.pin_number),
                    component_ref=component_ref,
                    element_id=(
                        driver.pin_svg_uuid
                        if pin_element_counts.get(driver.pin_svg_uuid) == 1
                        else ""
                    ),
                )
            for driver in subgraph.label_drivers:
                if driver.kind == KiCadDriverKind.HIER_LABEL:
                    role = "port"
                elif driver.kind == KiCadDriverKind.SHEET_PIN:
                    role = "sheet_entry"
                else:
                    continue
                add_terminal(
                    role=role,
                    source_uuid=str(driver.source_uuid or driver.svg_uuid),
                    source_subobject=str(driver.name),
                    name=str(driver.name),
                    pin_designator="",
                    element_id=driver.svg_uuid or driver.source_uuid,
                )

            for bucket, values in (subgraph.graphical or {}).items():
                if bucket in {"power_ports", "ports", "sheet_entries"}:
                    continue
                for element_id in values:
                    graph.graphical_artifact_links.append(
                        _row(
                            scope,
                            "sch.graphical_artifact_link",
                            {
                                "page": page_occurrence["id"],
                                "target": local["id"],
                                "element": element_id,
                            },
                            page_occurrence_ref=page_occurrence["id"],
                            target_type="sch.local_net_occurrence",
                            target_ref=local["id"],
                            artifact_key="sch.dwg_scene",
                            element_id=element_id,
                            source_identity=_source_identity(
                                **{"sch.source_key.artifact_element": element_id}
                            ),
                        )
                    )

    _add_hierarchy_bindings(graph, compiled, occurrence_rows, terminal_by_source, scope)
    validate_compiled_schematic_graph(graph.to_json())
    return graph


def _add_hierarchy_bindings(
    graph: KiCadCompiledSchematicGraph,
    compiled: Iterable[object],
    occurrence_rows: dict[int, tuple[dict[str, object], dict[str, object]]],
    terminal_by_source: dict[tuple[str, str, str], str],
    scope: dict[str, str],
) -> None:
    hierarchy_by_child = {
        row["child_unit_occurrence_ref"]: row for row in graph.hierarchy_occurrences
    }
    for child in compiled:
        occurrence = child.occurrence
        if occurrence is None or occurrence.parent is None or child.parent is None:
            continue
        child_unit, _child_page = occurrence_rows[id(occurrence)]
        hierarchy = hierarchy_by_child.get(child_unit["id"])
        if hierarchy is None or occurrence.sheet_symbol is None:
            continue
        child_ports: dict[str, str] = {}
        for subgraph in child.subgraphs:
            for driver in subgraph.label_drivers:
                if driver.kind == KiCadDriverKind.HIER_LABEL:
                    ref = terminal_by_source.get(
                        (
                            child.sheet_path,
                            "port",
                            str(driver.source_uuid or driver.svg_uuid),
                        )
                    )
                    if ref:
                        child_ports.setdefault(str(driver.name), ref)
        for pin in occurrence.sheet_symbol.pins:
            parent_ref = terminal_by_source.get(
                (child.parent.sheet_path, "sheet_entry", str(pin.uuid or ""))
            )
            child_ref = child_ports.get(str(pin.name))
            if not parent_ref or not child_ref:
                continue
            graph.hierarchy_terminal_bindings.append(
                _row(
                    scope,
                    "sch.hierarchy_terminal_binding",
                    {
                        "hierarchy": hierarchy["id"],
                        "parent": parent_ref,
                        "child": child_ref,
                    },
                    hierarchy_occurrence_ref=hierarchy["id"],
                    parent_terminal_occurrence_ref=parent_ref,
                    child_terminal_occurrence_ref=child_ref,
                    source_identity=_source_identity(
                        **{
                            "sch.source_key.source_uuid": pin.uuid,
                            "sch.source_key.source_subobject": pin.name,
                        }
                    ),
                )
            )


def validate_compiled_schematic_graph(payload: dict[str, object]) -> None:
    """Strictly validate embedded row identity, ownership, and references."""
    if payload.get("schema") != KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA:
        raise ValueError("unsupported compiled schematic graph schema")
    if payload.get("type") != KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE:
        raise ValueError("invalid compiled schematic graph type")
    collections = [
        value
        for key, value in payload.items()
        if key.endswith("s") and isinstance(value, list)
    ]
    rows = [row for values in collections for row in values if isinstance(row, dict)]
    ids = [str(row.get("id", "")) for row in rows]
    if not all(ids) or len(ids) != len(set(ids)):
        raise ValueError(
            "compiled schematic graph row ids must be unique and non-empty"
        )
    known = set(ids)
    row_by_id = {str(row["id"]): row for row in rows}
    for row in rows:
        for key, value in row.items():
            if key.endswith("_ref") and value is not None and str(value) not in known:
                if key in {
                    "design_component_ref",
                    "design_component_pin_ref",
                    "design_net_ref",
                }:
                    continue
                raise ValueError(
                    f"unresolved compiled schematic graph ref {key}={value}"
                )
            if key.endswith("_refs") and isinstance(value, list):
                missing = [ref for ref in value if str(ref) not in known]
                if missing:
                    raise ValueError(f"unresolved compiled schematic graph refs {key}")

    page_by_id = {str(row["id"]): row for row in payload.get("page_occurrences", [])}
    component_by_id = {
        str(row["id"]): row for row in payload.get("component_occurrences", [])
    }
    local_by_id = {
        str(row["id"]): row for row in payload.get("local_net_occurrences", [])
    }
    for terminal in payload.get("terminal_occurrences", []):
        page_ref = str(terminal.get("page_occurrence_ref", ""))
        if page_ref not in page_by_id:
            raise ValueError("terminal occurrence has invalid page owner")
        component_ref = terminal.get("component_occurrence_ref")
        if component_ref is not None:
            component = component_by_id[str(component_ref)]
            if component.get("page_occurrence_ref") != page_ref:
                raise ValueError("terminal and component occurrence owners differ")
        local_ref = terminal.get("local_net_occurrence_ref")
        if local_ref is not None:
            local = local_by_id[str(local_ref)]
            if local.get("page_occurrence_ref") != page_ref:
                raise ValueError("terminal and local-net occurrence owners differ")

    selectors: dict[tuple[str, str, str], tuple[str, str]] = {}
    for link in payload.get("graphical_artifact_links", []):
        selector = (
            str(link.get("page_occurrence_ref", "")),
            str(link.get("artifact_key", "")),
            str(link.get("element_id", "")),
        )
        target = (str(link.get("target_type", "")), str(link.get("target_ref", "")))
        previous = selectors.setdefault(selector, target)
        if previous != target:
            raise ValueError("graphical artifact selector resolves to multiple targets")
        target_row = row_by_id[target[1]]
        if target_row.get("type") != target[0]:
            raise ValueError("graphical artifact target type does not match target row")


__all__ = [
    "KICAD_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE",
    "KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA",
    "KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE",
    "KiCadCompiledSchematicGraph",
    "build_compiled_schematic_graph",
    "validate_compiled_schematic_graph",
]

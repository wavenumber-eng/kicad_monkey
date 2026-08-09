"""KiCad-owned producer for the CAD-neutral compiled schematic graph."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Iterable, cast

from .kicad_compiled_schematic_graph_identity import (
    SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE,
    SchCompiledSchematicGraphIdentityAllocator,
    compiled_schematic_graph_design_scope,
)

from .kicad_netlist_compiler import (
    Subgraph,
    _is_power_symbol,
    _resolve_instance_reference,
)
from .kicad_netlist_design import (
    CompiledSheet,
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
    from .kicad_schematic import KiCadSchematic


KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA = "kicad_monkey.compiled_schematic_graph.a0"
KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE = "sch.compiled_schematic_graph"
KICAD_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE = (
    SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE
)
_COLLECTION_NAMES = (
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
_COLLECTION_TYPES = {
    name: f"sch.{name.removesuffix('s')}" for name in _COLLECTION_NAMES
}
# Collection names whose singular is not a simple trailing-s removal.
_COLLECTION_TYPES.update(
    {
        "hierarchy_occurrences": "sch.hierarchy_occurrence",
        "hierarchy_terminal_bindings": "sch.hierarchy_terminal_binding",
        "graphical_artifact_links": "sch.graphical_artifact_link",
    }
)
_TERMINAL_ROLES = {"component_pin", "sheet_entry", "port", "power_port"}
_RESOLUTION_DIAGNOSTICS = {
    "logical_pin_unresolved",
    "component_occurrence_unresolved",
    "hierarchy_terminal_binding_unresolved",
    "design_net_unresolved",
}


def _source_row(
    allocator: SchCompiledSchematicGraphIdentityAllocator,
    object_type: str,
    identity_source: dict[str, object],
    *,
    owner_refs: tuple[str, ...] = (),
    **fields: object,
) -> dict[str, object]:
    return {
        "type": object_type,
        "id": allocator.allocate_source(
            object_type=object_type,
            source_identity=identity_source,
            owner_refs=owner_refs,
        ),
        **fields,
    }


def _derived_row(
    allocator: SchCompiledSchematicGraphIdentityAllocator,
    object_type: str,
    identity: dict[str, object],
    **fields: object,
) -> dict[str, object]:
    return {
        "type": object_type,
        "id": allocator.allocate_derived(object_type=object_type, identity=identity),
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
        else (
            Path(str(top_source)).resolve().parent if top_source is not None else None
        )
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


def _payload_collection(
    payload: dict[str, object], name: str
) -> list[dict[str, object]]:
    value = payload.get(name)
    if not isinstance(value, list) or not all(isinstance(row, dict) for row in value):
        raise ValueError(f"compiled schematic graph {name} must be a list of objects")
    return cast(list[dict[str, object]], value)


@dataclass
class KiCadCompiledSchematicGraph:
    """Versioned embedded graph payload with CAD-neutral row cardinalities."""

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
            **{name: list(getattr(self, name)) for name in _COLLECTION_NAMES},
        }

    @classmethod
    def from_json(cls, payload: dict[str, object]) -> "KiCadCompiledSchematicGraph":
        validate_compiled_schematic_graph(payload)
        return cls(
            **{
                name: [dict(row) for row in _payload_collection(payload, name)]
                for name in _COLLECTION_NAMES
            }
        )


def _append_component_occurrences(
    *,
    graph: KiCadCompiledSchematicGraph,
    compiled_sheet: CompiledSheet,
    top: "KiCadSchematic",
    page_occurrence: dict[str, object],
    source_occurrence_path: str,
    identity_allocator: SchCompiledSchematicGraphIdentityAllocator,
    legacy_refs: dict[str, str],
    legacy_units: dict[str, int],
    component_by_symbol: dict[tuple[str, str], str],
) -> None:
    canonical = _canonical_instance_path(top, compiled_sheet.sheet_path)
    assert compiled_sheet.schematic is not None
    for symbol in compiled_sheet.schematic.symbols:
        lib_symbol = compiled_sheet.schematic.get_lib_symbol_for_symbol(symbol)
        if _is_power_symbol(symbol, lib_symbol):
            continue
        physical = _resolve_instance_reference(
            symbol,
            compiled_sheet.sheet_path,
            legacy_refs,
            canonical,
        )
        if not physical or physical.startswith("#"):
            continue
        unit_number = _resolve_instance_unit(
            symbol,
            compiled_sheet.sheet_path,
            legacy_unit_lookup=legacy_units,
            canonical_path=canonical,
        )
        symbol_uuid = str(symbol.uuid or "")
        component_source = _source_identity(
            **{
                "sch.source_key.source_uuid": symbol_uuid,
                "sch.source_key.source_path": source_occurrence_path,
            }
        )
        component = _source_row(
            identity_allocator,
            "sch.component_occurrence",
            component_source,
            owner_refs=(str(page_occurrence["id"]),),
            page_occurrence_ref=page_occurrence["id"],
            source_designator=_source_designator(symbol),
            physical_designator=physical,
            display_designator=physical,
            unit=max(1, unit_number),
            body_style=int(symbol.convert or 0),
            source_identity=component_source,
        )
        graph.component_occurrences.append(component)
        component_by_symbol[(compiled_sheet.sheet_path, symbol_uuid)] = str(
            component["id"]
        )
        if not symbol_uuid:
            continue
        graph.graphical_artifact_links.append(
            _derived_row(
                identity_allocator,
                "sch.graphical_artifact_link",
                {
                    "page_occurrence_ref": page_occurrence["id"],
                    "target_type": "sch.component_occurrence",
                    "target_ref": component["id"],
                    "artifact_key": "sch.dwg_scene",
                    "element_id": symbol_uuid,
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


@dataclass
class _TerminalBuildContext:
    graph: KiCadCompiledSchematicGraph
    identity_allocator: SchCompiledSchematicGraphIdentityAllocator
    compiled_sheet: CompiledSheet
    page_occurrence: dict[str, object]
    source_occurrence_path: str
    component_by_symbol: dict[tuple[str, str], str]
    terminal_by_source: dict[tuple[str, str, str], str]
    terminal_by_semantic_source: dict[tuple[str, str, str, str], dict[str, object]]
    pin_element_counts: dict[str, int]
    subgraph_terminals: list[dict[str, object]] = field(default_factory=list)
    terminal_element_ids: set[str] = field(default_factory=set)

    def add_terminal(
        self,
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
        existing = self.terminal_by_semantic_source.get(semantic_source_key)
        if existing is not None:
            self._reuse_terminal(
                existing,
                role=role,
                source_uuid=source_uuid,
                name=name,
                element_id=element_id,
            )
            return
        terminal_source = _source_identity(
            **{
                "sch.source_key.source_uuid": source_uuid,
                "sch.source_key.source_subobject": source_subobject,
                "sch.source_key.source_path": self.source_occurrence_path,
            }
        )
        terminal = _source_row(
            self.identity_allocator,
            "sch.terminal_occurrence",
            terminal_source,
            owner_refs=(str(self.page_occurrence["id"]), str(component_ref or "")),
            page_occurrence_ref=self.page_occurrence["id"],
            role=role,
            component_occurrence_ref=component_ref,
            name=name,
            pin_designator=pin_designator,
            resolution_diagnostics=_terminal_diagnostics(role, component_ref),
            source_identity=terminal_source,
        )
        if component_ref is None:
            terminal.pop("component_occurrence_ref")
        self.graph.terminal_occurrences.append(terminal)
        self.terminal_by_semantic_source[semantic_source_key] = terminal
        self.subgraph_terminals.append(terminal)
        terminal_id = str(terminal["id"])
        self.terminal_by_source[(self.compiled_sheet.sheet_path, role, source_uuid)] = (
            terminal_id
        )
        if element_id:
            self.terminal_element_ids.add(element_id)
            self._append_terminal_link(terminal_id, element_id)

    def _reuse_terminal(
        self,
        terminal: dict[str, object],
        *,
        role: str,
        source_uuid: str,
        name: str,
        element_id: str,
    ) -> None:
        if terminal in self.subgraph_terminals:
            return
        if terminal.get("local_net_occurrence_ref"):
            raise ValueError(
                "one semantic terminal source resolves to multiple local nets"
            )
        existing_name = str(terminal.get("name", "") or "")
        if name and (not existing_name or name < existing_name):
            terminal["name"] = name
        self.terminal_by_source[(self.compiled_sheet.sheet_path, role, source_uuid)] = (
            str(terminal["id"])
        )
        self.subgraph_terminals.append(terminal)
        if element_id:
            self.terminal_element_ids.add(element_id)

    def _append_terminal_link(self, terminal_id: str, element_id: str) -> None:
        page_ref = self.page_occurrence["id"]
        self.graph.graphical_artifact_links.append(
            _derived_row(
                self.identity_allocator,
                "sch.graphical_artifact_link",
                {
                    "page_occurrence_ref": page_ref,
                    "target_type": "sch.terminal_occurrence",
                    "target_ref": terminal_id,
                    "artifact_key": "sch.dwg_scene",
                    "element_id": element_id,
                },
                page_occurrence_ref=page_ref,
                target_type="sch.terminal_occurrence",
                target_ref=terminal_id,
                artifact_key="sch.dwg_scene",
                element_id=element_id,
                source_identity=_source_identity(
                    **{"sch.source_key.artifact_element": element_id}
                ),
            )
        )


def _terminal_diagnostics(role: str, component_ref: str | None) -> list[str]:
    diagnostics: list[str] = []
    if role == "component_pin":
        if component_ref is None:
            diagnostics.append("component_occurrence_unresolved")
        diagnostics.append("logical_pin_unresolved")
    diagnostics.append("design_net_unresolved")
    return diagnostics


def _append_bus_drawing_links(
    *,
    graph: KiCadCompiledSchematicGraph,
    compiled_sheet: CompiledSheet,
    page_occurrence: dict[str, object],
    identity_allocator: SchCompiledSchematicGraphIdentityAllocator,
) -> None:
    assert compiled_sheet.schematic is not None
    page_ref = page_occurrence["id"]
    for source_objects in (
        getattr(compiled_sheet.schematic, "buses", ()) or (),
        getattr(compiled_sheet.schematic, "bus_entries", ()) or (),
    ):
        for source_object in source_objects:
            element_id = str(getattr(source_object, "uuid", "") or "")
            if not element_id:
                continue
            graph.graphical_artifact_links.append(
                _derived_row(
                    identity_allocator,
                    "sch.graphical_artifact_link",
                    {
                        "page_occurrence_ref": page_ref,
                        "target_type": "sch.page_occurrence",
                        "target_ref": page_ref,
                        "artifact_key": "sch.dwg_scene",
                        "element_id": element_id,
                    },
                    page_occurrence_ref=page_ref,
                    target_type="sch.page_occurrence",
                    target_ref=page_ref,
                    artifact_key="sch.dwg_scene",
                    element_id=element_id,
                    source_identity=_source_identity(
                        **{"sch.source_key.artifact_element": element_id}
                    ),
                )
            )


def _pin_element_counts(compiled_sheet: CompiledSheet) -> dict[str, int]:
    counts: dict[str, int] = {}
    for subgraph in compiled_sheet.subgraphs:
        for driver in subgraph.pin_drivers:
            element_id = str(driver.pin_svg_uuid or "")
            if element_id:
                counts[element_id] = counts.get(element_id, 0) + 1
    return counts


def _append_pin_terminals(context: _TerminalBuildContext, subgraph: Subgraph) -> None:
    for driver in subgraph.pin_drivers:
        symbol_uuid = str(getattr(driver, "svg_uuid", "") or "")
        element_id = (
            driver.pin_svg_uuid
            if context.pin_element_counts.get(driver.pin_svg_uuid) == 1
            else ""
        )
        if driver.designator.startswith("#"):
            if driver.is_power:
                context.add_terminal(
                    role="power_port",
                    source_uuid=symbol_uuid,
                    source_subobject=str(driver.pin_number),
                    name=driver.power_value or driver.pin_name,
                    pin_designator=str(driver.pin_number),
                    element_id=element_id,
                )
            continue
        context.add_terminal(
            role="component_pin",
            source_uuid=str(driver.source_uuid or symbol_uuid),
            source_subobject=str(driver.pin_number),
            name=str(driver.pin_name or ""),
            pin_designator=str(driver.pin_number),
            component_ref=context.component_by_symbol.get(
                (context.compiled_sheet.sheet_path, symbol_uuid)
            ),
            element_id=element_id,
        )


def _label_terminal_role(kind: KiCadDriverKind) -> str:
    if kind in {KiCadDriverKind.HIER_LABEL, KiCadDriverKind.GLOBAL_LABEL}:
        return "port"
    if kind == KiCadDriverKind.SHEET_PIN:
        return "sheet_entry"
    return ""


def _append_label_terminals(context: _TerminalBuildContext, subgraph: Subgraph) -> None:
    for driver in subgraph.label_drivers:
        role = _label_terminal_role(driver.kind)
        if not role:
            continue
        context.add_terminal(
            role=role,
            source_uuid=str(driver.source_uuid or driver.svg_uuid),
            source_subobject=str(driver.name),
            name=str(driver.name),
            pin_designator="",
            element_id=driver.svg_uuid or driver.source_uuid,
        )


def _local_graphical_elements(
    subgraph: Subgraph, terminal_element_ids: set[str]
) -> list[str]:
    return sorted(
        {
            str(element_id)
            for bucket, values in (subgraph.graphical or {}).items()
            if bucket not in {"power_ports", "ports", "sheet_entries"}
            for element_id in values
            if element_id and str(element_id) not in terminal_element_ids
        }
    )


def _local_topology_evidence(
    terminals: list[dict[str, object]], graphical_element_ids: list[str]
) -> dict[str, object]:
    terminal_refs = sorted({str(terminal["id"]) for terminal in terminals})
    if terminal_refs:
        return {"terminal_occurrence_refs": terminal_refs}
    if graphical_element_ids:
        return {
            "graphical_selectors": [
                f"sch.dwg_scene\x1f{element_id}" for element_id in graphical_element_ids
            ]
        }
    return {}


def _append_local_drawing_links(
    *,
    context: _TerminalBuildContext,
    local_ref: object,
    graphical_element_ids: list[str],
) -> None:
    page_ref = context.page_occurrence["id"]
    for element_id in graphical_element_ids:
        context.graph.graphical_artifact_links.append(
            _derived_row(
                context.identity_allocator,
                "sch.graphical_artifact_link",
                {
                    "page_occurrence_ref": page_ref,
                    "target_type": "sch.local_net_occurrence",
                    "target_ref": local_ref,
                    "artifact_key": "sch.dwg_scene",
                    "element_id": element_id,
                },
                page_occurrence_ref=page_ref,
                target_type="sch.local_net_occurrence",
                target_ref=local_ref,
                artifact_key="sch.dwg_scene",
                element_id=element_id,
                source_identity=_source_identity(
                    **{"sch.source_key.artifact_element": element_id}
                ),
            )
        )


def _append_subgraph(
    *,
    context: _TerminalBuildContext,
    subgraph: Subgraph,
    compiled_net_code: int | None,
    display_name: str,
) -> None:
    _append_pin_terminals(context, subgraph)
    _append_label_terminals(context, subgraph)
    graphical_element_ids = _local_graphical_elements(
        subgraph, context.terminal_element_ids
    )
    topology_evidence = _local_topology_evidence(
        context.subgraph_terminals, graphical_element_ids
    )
    if not topology_evidence:
        return
    aliases = sorted(
        {
            str(driver.name)
            for driver in subgraph.label_drivers
            if getattr(driver, "name", "")
        }
    )
    local_source = _source_identity(
        **{
            "sch.source_key.source_record": (
                f"net-uid:{compiled_net_code:012x}"
                if compiled_net_code is not None
                else ""
            ),
            "sch.source_key.source_path": context.source_occurrence_path,
        }
    )
    local = _derived_row(
        context.identity_allocator,
        "sch.local_net_occurrence",
        {
            "page_occurrence_ref": context.page_occurrence["id"],
            **topology_evidence,
        },
        page_occurrence_ref=context.page_occurrence["id"],
        display_name=display_name,
        qualified_name=display_name,
        aliases=aliases,
        source_identity=local_source,
    )
    context.graph.local_net_occurrences.append(local)
    for terminal in context.subgraph_terminals:
        terminal["local_net_occurrence_ref"] = local["id"]
    _append_local_drawing_links(
        context=context,
        local_ref=local["id"],
        graphical_element_ids=graphical_element_ids,
    )


def _append_sheet_connectivity(
    *,
    graph: KiCadCompiledSchematicGraph,
    compiled_sheet: CompiledSheet,
    page_occurrence: dict[str, object],
    source_occurrence_path: str,
    identity_allocator: SchCompiledSchematicGraphIdentityAllocator,
    component_by_symbol: dict[tuple[str, str], str],
    terminal_by_source: dict[tuple[str, str, str], str],
) -> None:
    terminal_by_semantic_source: dict[tuple[str, str, str, str], dict[str, object]] = {}
    pin_counts = _pin_element_counts(compiled_sheet)
    for subgraph_index, subgraph in enumerate(compiled_sheet.subgraphs):
        context = _TerminalBuildContext(
            graph=graph,
            identity_allocator=identity_allocator,
            compiled_sheet=compiled_sheet,
            page_occurrence=page_occurrence,
            source_occurrence_path=source_occurrence_path,
            component_by_symbol=component_by_symbol,
            terminal_by_source=terminal_by_source,
            terminal_by_semantic_source=terminal_by_semantic_source,
            pin_element_counts=pin_counts,
        )
        _append_subgraph(
            context=context,
            subgraph=subgraph,
            compiled_net_code=compiled_sheet.subgraph_net_codes.get(subgraph_index),
            display_name=compiled_sheet.subgraph_net_names.get(
                subgraph_index, subgraph.chosen_name
            ),
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
    scope = compiled_schematic_graph_design_scope(
        source_cad="kicad", project={"filename": project_file}
    )
    identity_allocator = SchCompiledSchematicGraphIdentityAllocator(design_scope=scope)
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
            unit = _source_row(
                identity_allocator,
                "sch.unit_definition",
                unit_source,
                display_name=Path(source_path).stem
                if source_path
                else occurrence.sheet_name,
                page_definition_refs=[],
                source_identity=unit_source,
            )
            page = _source_row(
                identity_allocator,
                "sch.page_definition",
                unit_source,
                owner_refs=(str(unit["id"]),),
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
        unit_occurrence = _source_row(
            identity_allocator,
            "sch.unit_occurrence",
            occurrence_source,
            unit_definition_ref=definition[0],
            display_name=occurrence.sheet_name,
            page_occurrence_refs=[],
            source_identity=occurrence_source,
        )
        page_occurrence = _source_row(
            identity_allocator,
            "sch.page_occurrence",
            occurrence_source,
            owner_refs=(str(unit_occurrence["id"]),),
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

        sheet_symbol = occurrence.sheet_symbol
        if occurrence.parent is not None and sheet_symbol is not None:
            parent_unit, parent_page = occurrence_rows[id(occurrence.parent)]
            hierarchy_source = _source_identity(
                **{
                    "sch.source_key.source_uuid": sheet_symbol.uuid,
                    "sch.source_key.source_path": source_occurrence_path,
                    "sch.source_key.source_record": f"instance-path:{address}",
                }
            )
            hierarchy = _source_row(
                identity_allocator,
                "sch.hierarchy_occurrence",
                hierarchy_source,
                owner_refs=(str(parent_page["id"]), str(unit_occurrence["id"])),
                parent_unit_occurrence_ref=parent_unit["id"],
                parent_page_occurrence_ref=parent_page["id"],
                child_unit_occurrence_ref=unit_occurrence["id"],
                source_identity=hierarchy_source,
            )
            graph.hierarchy_occurrences.append(hierarchy)
            unit_occurrence["parent_hierarchy_occurrence_ref"] = hierarchy["id"]
            graph.graphical_artifact_links.append(
                _derived_row(
                    identity_allocator,
                    "sch.graphical_artifact_link",
                    {
                        "page_occurrence_ref": parent_page["id"],
                        "target_type": "sch.hierarchy_occurrence",
                        "target_ref": hierarchy["id"],
                        "artifact_key": "sch.dwg_scene",
                        "element_id": sheet_symbol.uuid,
                    },
                    page_occurrence_ref=parent_page["id"],
                    target_type="sch.hierarchy_occurrence",
                    target_ref=hierarchy["id"],
                    artifact_key="sch.dwg_scene",
                    element_id=sheet_symbol.uuid,
                    source_identity=_source_identity(
                        **{"sch.source_key.artifact_element": sheet_symbol.uuid}
                    ),
                )
            )

        _append_component_occurrences(
            graph=graph,
            compiled_sheet=cs,
            top=top,
            page_occurrence=page_occurrence,
            source_occurrence_path=source_occurrence_path,
            identity_allocator=identity_allocator,
            legacy_refs=legacy_refs,
            legacy_units=legacy_units,
            component_by_symbol=component_by_symbol,
        )

        _append_bus_drawing_links(
            graph=graph,
            compiled_sheet=cs,
            page_occurrence=page_occurrence,
            identity_allocator=identity_allocator,
        )
        _append_sheet_connectivity(
            graph=graph,
            compiled_sheet=cs,
            page_occurrence=page_occurrence,
            source_occurrence_path=source_occurrence_path,
            identity_allocator=identity_allocator,
            component_by_symbol=component_by_symbol,
            terminal_by_source=terminal_by_source,
        )

    _add_hierarchy_bindings(
        graph,
        compiled,
        occurrence_rows,
        terminal_by_source,
        identity_allocator,
    )
    validate_compiled_schematic_graph(graph.to_json())
    return graph


def _add_hierarchy_bindings(
    graph: KiCadCompiledSchematicGraph,
    compiled: Iterable[CompiledSheet],
    occurrence_rows: dict[int, tuple[dict[str, object], dict[str, object]]],
    terminal_by_source: dict[tuple[str, str, str], str],
    identity_allocator: SchCompiledSchematicGraphIdentityAllocator,
) -> None:
    terminal_rows = {str(row["id"]): row for row in graph.terminal_occurrences}

    def mark_unresolved(terminal_ref: str) -> None:
        terminal = terminal_rows.get(terminal_ref)
        if terminal is None:
            return
        diagnostics = terminal.setdefault("resolution_diagnostics", [])
        assert isinstance(diagnostics, list)
        if "hierarchy_terminal_binding_unresolved" not in diagnostics:
            diagnostics.append("hierarchy_terminal_binding_unresolved")

    hierarchy_by_child = {
        row["child_unit_occurrence_ref"]: row for row in graph.hierarchy_occurrences
    }
    for child in compiled:
        occurrence = child.occurrence
        parent = child.parent
        if occurrence is None or occurrence.parent is None or parent is None:
            continue
        child_unit, _child_page = occurrence_rows[id(occurrence)]
        hierarchy = hierarchy_by_child.get(child_unit["id"])
        sheet_symbol = occurrence.sheet_symbol
        if hierarchy is None or sheet_symbol is None:
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
        bound_child_refs: set[str] = set()
        for pin in sheet_symbol.pins:
            parent_ref = terminal_by_source.get(
                (parent.sheet_path, "sheet_entry", str(pin.uuid or ""))
            )
            child_ref = child_ports.get(str(pin.name))
            if not parent_ref or not child_ref:
                if parent_ref:
                    mark_unresolved(parent_ref)
                continue
            bound_child_refs.add(child_ref)
            graph.hierarchy_terminal_bindings.append(
                _derived_row(
                    identity_allocator,
                    "sch.hierarchy_terminal_binding",
                    {
                        "hierarchy_occurrence_ref": hierarchy["id"],
                        "parent_terminal_occurrence_ref": parent_ref,
                        "child_terminal_occurrence_ref": child_ref,
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
        for child_ref in set(child_ports.values()) - bound_child_refs:
            mark_unresolved(child_ref)


def _validate_payload_header(payload: dict[str, object]) -> None:
    if payload.get("schema") != KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA:
        raise ValueError("unsupported compiled schematic graph schema")
    if payload.get("type") != KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE:
        raise ValueError("invalid compiled schematic graph type")


def _validate_graph_rows(
    payload: dict[str, object],
) -> dict[str, dict[str, object]]:
    _validate_payload_header(payload)
    collections_by_name = {
        name: _payload_collection(payload, name) for name in _COLLECTION_NAMES
    }
    collections = list(collections_by_name.values())
    rows = [row for values in collections for row in values]
    for name, values in collections_by_name.items():
        expected_type = _COLLECTION_TYPES[name]
        if any(row.get("type") != expected_type for row in values):
            raise ValueError(
                f"compiled schematic graph {name} rows must use {expected_type}"
            )
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

    return row_by_id


def _validate_terminal_rows(payload: dict[str, object]) -> None:
    page_by_id = {
        str(row["id"]): row for row in _payload_collection(payload, "page_occurrences")
    }
    component_by_id = {
        str(row["id"]): row
        for row in _payload_collection(payload, "component_occurrences")
    }
    local_by_id = {
        str(row["id"]): row
        for row in _payload_collection(payload, "local_net_occurrences")
    }
    for terminal in _payload_collection(payload, "terminal_occurrences"):
        page_ref = str(terminal.get("page_occurrence_ref", ""))
        if page_ref not in page_by_id:
            raise ValueError("terminal occurrence has invalid page owner")
        role = str(terminal.get("role", ""))
        if role not in _TERMINAL_ROLES:
            raise ValueError(f"terminal occurrence has invalid role {role!r}")
        diagnostics_value = terminal.get("resolution_diagnostics", [])
        if not isinstance(diagnostics_value, list):
            raise ValueError("terminal resolution_diagnostics must be a list")
        diagnostics = {str(value) for value in diagnostics_value}
        if not diagnostics <= _RESOLUTION_DIAGNOSTICS:
            raise ValueError("terminal occurrence has unregistered diagnostics")
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
        if role == "component_pin":
            if (
                component_ref is None
                and "component_occurrence_unresolved" not in diagnostics
            ):
                raise ValueError(
                    "component-pin terminal needs component ownership or a diagnostic"
                )
            if (
                terminal.get("design_component_pin_ref") is None
                and "logical_pin_unresolved" not in diagnostics
            ):
                raise ValueError(
                    "component-pin terminal needs logical-pin ownership or a diagnostic"
                )


def _validated_hierarchy_binding_parent_refs(
    payload: dict[str, object],
) -> tuple[set[str], dict[str, dict[str, object]], dict[str, dict[str, object]]]:
    page_by_id = {
        str(row["id"]): row for row in _payload_collection(payload, "page_occurrences")
    }
    hierarchy_by_id = {
        str(row["id"]): row
        for row in _payload_collection(payload, "hierarchy_occurrences")
    }
    terminal_by_id = {
        str(row["id"]): row
        for row in _payload_collection(payload, "terminal_occurrences")
    }
    page_refs_by_unit: dict[str, set[str]] = {}
    for page_ref, page in page_by_id.items():
        page_refs_by_unit.setdefault(
            str(page.get("unit_occurrence_ref", "")), set()
        ).add(page_ref)
    binding_parent_refs: set[str] = set()
    for binding in _payload_collection(payload, "hierarchy_terminal_bindings"):
        hierarchy = hierarchy_by_id[str(binding["hierarchy_occurrence_ref"])]
        parent = terminal_by_id[str(binding["parent_terminal_occurrence_ref"])]
        child = terminal_by_id[str(binding["child_terminal_occurrence_ref"])]
        if parent.get("role") != "sheet_entry" or child.get("role") != "port":
            raise ValueError("hierarchy binding must connect a sheet_entry to a port")
        if parent.get("page_occurrence_ref") != hierarchy.get(
            "parent_page_occurrence_ref"
        ):
            raise ValueError("hierarchy binding parent terminal has wrong page owner")
        child_unit_ref = str(hierarchy.get("child_unit_occurrence_ref", ""))
        if str(child.get("page_occurrence_ref", "")) not in page_refs_by_unit.get(
            child_unit_ref, set()
        ):
            raise ValueError("hierarchy binding child terminal has wrong unit owner")
        parent_net = parent.get("design_net_ref")
        child_net = child.get("design_net_ref")
        binding_net = binding.get("design_net_ref")
        resolved_nets = {
            str(value) for value in (parent_net, child_net, binding_net) if value
        }
        if len(resolved_nets) > 1:
            raise ValueError("hierarchy binding resolves to different design nets")
        binding_parent_refs.add(str(parent["id"]))
    return binding_parent_refs, hierarchy_by_id, terminal_by_id


def _validate_hierarchy_binding_completeness(payload: dict[str, object]) -> None:
    (
        binding_parent_refs,
        hierarchy_by_id,
        terminal_by_id,
    ) = _validated_hierarchy_binding_parent_refs(payload)
    hierarchy_parent_pages = {
        str(row.get("parent_page_occurrence_ref", ""))
        for row in hierarchy_by_id.values()
    }
    for terminal_ref, terminal in terminal_by_id.items():
        if (
            terminal.get("role") != "sheet_entry"
            or str(terminal.get("page_occurrence_ref", ""))
            not in hierarchy_parent_pages
        ):
            continue
        terminal_diagnostics = terminal.get("resolution_diagnostics", [])
        diagnostics = (
            {str(value) for value in terminal_diagnostics}
            if isinstance(terminal_diagnostics, list)
            else set()
        )
        if (
            not terminal.get("design_net_ref")
            and "design_net_unresolved" not in diagnostics
        ):
            raise ValueError(
                "hierarchy sheet-entry terminal needs a design net or diagnostic"
            )
        if (
            terminal_ref not in binding_parent_refs
            and "hierarchy_terminal_binding_unresolved" not in diagnostics
        ):
            raise ValueError(
                "hierarchy sheet-entry terminal needs a binding or diagnostic"
            )


def _graphical_target_page(target_type: str, target_row: dict[str, object]) -> object:
    if target_type == "sch.hierarchy_occurrence":
        return target_row.get("parent_page_occurrence_ref")
    if target_type == "sch.page_occurrence":
        return target_row.get("id")
    return target_row.get("page_occurrence_ref")


def _validate_graphical_links(
    payload: dict[str, object], row_by_id: dict[str, dict[str, object]]
) -> None:
    selectors: dict[tuple[str, str, str], tuple[str, str]] = {}
    for link in _payload_collection(payload, "graphical_artifact_links"):
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
        target_page = _graphical_target_page(target[0], target_row)
        if str(target_page or "") != selector[0]:
            raise ValueError("graphical artifact target has wrong page owner")


def validate_compiled_schematic_graph(payload: dict[str, object]) -> None:
    """Strictly validate embedded row identity, ownership, and references."""

    row_by_id = _validate_graph_rows(payload)
    _validate_terminal_rows(payload)
    _validate_hierarchy_binding_completeness(payload)
    _validate_graphical_links(payload, row_by_id)


__all__ = [
    "KICAD_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE",
    "KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA",
    "KICAD_COMPILED_SCHEMATIC_GRAPH_TYPE",
    "KiCadCompiledSchematicGraph",
    "build_compiled_schematic_graph",
    "validate_compiled_schematic_graph",
]

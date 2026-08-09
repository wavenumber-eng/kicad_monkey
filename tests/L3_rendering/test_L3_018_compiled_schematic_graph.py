"""Corpus acceptance tests for the embedded compiled schematic graph."""

from __future__ import annotations

import copy
from functools import lru_cache

import pytest

from kicad_monkey import KiCadDesign, validate_compiled_schematic_graph
from kicad_monkey.kicad_compiled_schematic_graph import build_compiled_schematic_graph
from kicad_monkey.kicad_compiled_schematic_graph_identity import (
    SchCompiledSchematicGraphIdentityAllocator,
    compiled_schematic_graph_design_scope,
)
from kicad_monkey.kicad_schematic_occurrence import walk_schematic_occurrences
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    resolve_kicad_manifest_path,
)


COLLECTIONS = (
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


@lru_cache(maxsize=None)
def _compiled_graph(case_id: str) -> dict[str, object]:
    case = get_kicad_corpus_case(case_id)
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None
    return KiCadDesign.from_project_file(project_path).to_json()[
        "compiled_schematic_graph"
    ]


def _design(case_id: str) -> KiCadDesign:
    case = get_kicad_corpus_case(case_id)
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None
    return KiCadDesign.from_project_file(project_path)


@pytest.mark.parametrize(
    ("case_id", "expected_counts"),
    (
        (
            "real_world/yoshi_mainboard",
            (1, 1, 1, 1, 0, 48, 58, 222, 0, 547),
        ),
        (
            "real_world/taillight",
            (2, 2, 6, 6, 5, 97, 110, 367, 25, 881),
        ),
        (
            "real_world/speedy_processing_module",
            (14, 14, 19, 19, 18, 536, 838, 3089, 295, 7417),
        ),
        (
            "real_world/jumperless_v5r7",
            (2, 2, 2, 2, 1, 704, 1174, 3850, 0, 7585),
        ),
    ),
)
def test_reference_projects_emit_complete_valid_graphs(
    case_id: str,
    expected_counts: tuple[int, ...],
) -> None:
    graph = _compiled_graph(case_id)

    assert tuple(len(graph[name]) for name in COLLECTIONS) == expected_counts
    validate_compiled_schematic_graph(graph)
    row_ids = [row["id"] for name in COLLECTIONS for row in graph[name]]
    assert len(row_ids) == len(set(row_ids))
    selectors = [
        (row["page_occurrence_ref"], row["artifact_key"], row["element_id"])
        for row in graph["graphical_artifact_links"]
    ]
    assert len(selectors) == len(set(selectors))
    assert not any(
        policy_key in row
        for name in COLLECTIONS
        for row in graph[name]
        for policy_key in ("dnp", "in_bom", "on_board", "variant")
    )


def test_speedy_preserves_reuse_multipart_and_scalar_hierarchy_bindings() -> None:
    graph = _compiled_graph("real_world/speedy_processing_module")

    assert len(graph["unit_definitions"]) < len(graph["unit_occurrences"])
    ic1 = [
        row
        for row in graph["component_occurrences"]
        if row["physical_designator"] == "IC1"
    ]
    assert {row["unit"] for row in ic1} == set(range(1, 10))
    assert len({row["page_occurrence_ref"] for row in ic1}) == 4
    assert graph["hierarchy_terminal_bindings"]
    assert all(
        row["parent_terminal_occurrence_ref"] and row["child_terminal_occurrence_ref"]
        for row in graph["hierarchy_terminal_bindings"]
    )


def test_identity_vectors_match_the_governed_generic_allocator() -> None:
    scope = compiled_schematic_graph_design_scope(
        source_cad="KiCad", project={"filename": r"Board\Main.kicad_pro"}
    )
    allocator = SchCompiledSchematicGraphIdentityAllocator(design_scope=scope)
    source = {
        "sch.source_key.source_path": "sheet.kicad_sch",
        "sch.source_key.source_uuid": "11111111-1111-1111-1111-111111111111",
    }
    unit_ref = allocator.allocate_source(
        object_type="sch.unit_definition", source_identity=source
    )
    page_ref = allocator.allocate_source(
        object_type="sch.page_definition",
        source_identity=source,
        owner_refs=(unit_ref,),
    )
    terminal_ref = allocator.allocate_source(
        object_type="sch.terminal_occurrence",
        source_identity={
            "sch.source_key.source_uuid": "22222222-2222-2222-2222-222222222222",
            "sch.source_key.source_subobject": "1",
            "sch.source_key.source_path": "ignored",
        },
        owner_refs=(
            "33333333-3333-7333-8333-333333333333",
            "44444444-4444-7444-8444-444444444444",
        ),
    )
    local_ref = allocator.allocate_derived(
        object_type="sch.local_net_occurrence",
        identity={
            "page_occurrence_ref": "33333333-3333-7333-8333-333333333333",
            "terminal_occurrence_refs": [terminal_ref],
        },
    )

    assert scope == {
        "source_cad": "kicad",
        "project_file": "board/main.kicad_pro",
    }
    assert unit_ref == "019fd985-0000-70bd-a222-1ececf294168"
    assert page_ref == "019fd985-0000-775f-b855-85c6a18a16b0"
    assert terminal_ref == "019fd985-0000-7add-b1e5-e963dc94ae89"
    assert local_ref == "019fd985-0000-72f1-ae66-5eb49119d483"


def test_wire_uuid_edit_does_not_change_terminal_topology_local_net_identity() -> None:
    design = _design("real_world/yoshi_mainboard")
    assert design.top_schematic is not None
    wire = next(
        wire for wire in design.top_schematic.wires if getattr(wire, "uuid", "")
    )
    old_uuid = str(wire.uuid)
    before = build_compiled_schematic_graph(design).to_json()
    old_link = next(
        row
        for row in before["graphical_artifact_links"]
        if row["element_id"] == old_uuid
        and row["target_type"] == "sch.local_net_occurrence"
    )

    wire.uuid = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    after = build_compiled_schematic_graph(design).to_json()
    new_link = next(
        row
        for row in after["graphical_artifact_links"]
        if row["element_id"] == wire.uuid
        and row["target_type"] == "sch.local_net_occurrence"
    )

    assert new_link["target_ref"] == old_link["target_ref"]


def test_jumperless_global_ports_and_deferred_bus_drawing_evidence_are_complete() -> (
    None
):
    design = _design("real_world/jumperless_v5r7")
    assert design.top_schematic is not None
    graph = build_compiled_schematic_graph(design).to_json()
    selectors = {
        (row["page_occurrence_ref"], row["element_id"]): row
        for row in graph["graphical_artifact_links"]
    }
    pages_by_path = {
        str(row["source_identity"].get("sch.source_key.source_path", "")): row["id"]
        for row in graph["page_occurrences"]
    }
    terminal_by_selector = {
        (
            row["page_occurrence_ref"],
            row["source_identity"].get("sch.source_key.source_uuid", ""),
        ): row
        for row in graph["terminal_occurrences"]
    }

    bus_count = 0
    global_label_count = 0
    for occurrence in walk_schematic_occurrences(design.top_schematic):
        page_ref = pages_by_path[occurrence.sheet_path_uuids]
        for source_object in (
            *(occurrence.schematic.buses or ()),
            *(occurrence.schematic.bus_entries or ()),
        ):
            bus_count += 1
            assert (page_ref, source_object.uuid) in selectors
        for label in occurrence.schematic.global_labels or ():
            global_label_count += 1
            terminal = terminal_by_selector[(page_ref, label.uuid)]
            assert terminal["role"] == "port"

    assert bus_count == 236
    assert global_label_count > 0


def test_unmatched_hierarchy_boundary_is_diagnosed_and_valid() -> None:
    design = _design("real_world/speedy_processing_module")
    assert design.top_schematic is not None
    occurrence = next(
        item
        for item in walk_schematic_occurrences(design.top_schematic)
        if item.sheet_symbol is not None
        and item.sheet_symbol.pins
        and item.schematic.hierarchical_labels
        and any(
            pin.name == label.text
            for pin in item.sheet_symbol.pins
            for label in item.schematic.hierarchical_labels
        )
    )
    pin = next(
        pin
        for pin in occurrence.sheet_symbol.pins
        if any(
            pin.name == label.text for label in occurrence.schematic.hierarchical_labels
        )
    )
    label = next(
        label
        for label in occurrence.schematic.hierarchical_labels
        if label.text == pin.name
    )
    label.text = f"{label.text}_UNMATCHED"

    graph = build_compiled_schematic_graph(design).to_json()
    terminals = {
        row["source_identity"].get("sch.source_key.source_uuid", ""): row
        for row in graph["terminal_occurrences"]
    }
    parent = terminals[pin.uuid]
    child = terminals[label.uuid]
    bindings = graph["hierarchy_terminal_bindings"]

    assert not any(
        row["parent_terminal_occurrence_ref"] == parent["id"]
        or row["child_terminal_occurrence_ref"] == child["id"]
        for row in bindings
    )
    assert "hierarchy_terminal_binding_unresolved" in parent["resolution_diagnostics"]
    assert "hierarchy_terminal_binding_unresolved" in child["resolution_diagnostics"]
    validate_compiled_schematic_graph(graph)

    invalid = copy.deepcopy(graph)
    invalid_parent = next(
        row for row in invalid["terminal_occurrences"] if row["id"] == parent["id"]
    )
    invalid_parent["resolution_diagnostics"].remove(
        "hierarchy_terminal_binding_unresolved"
    )
    with pytest.raises(ValueError, match="needs a binding or diagnostic"):
        validate_compiled_schematic_graph(invalid)

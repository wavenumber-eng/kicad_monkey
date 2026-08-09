"""Corpus acceptance tests for the embedded compiled schematic graph."""

from __future__ import annotations

from functools import lru_cache

import pytest

from kicad_monkey import KiCadDesign, validate_compiled_schematic_graph
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
            (2, 2, 2, 2, 1, 704, 1342, 3269, 0, 7349),
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

"""Generated compiled-graph transport and deterministic identity vectors."""

from __future__ import annotations

import json
import re
import uuid
from pathlib import Path
from typing import Any

import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    CompiledSchematicGraphA0,
    decode_compiled_schematic_graph_a0,
)
from kicad_monkey.kicad_compiled_schematic_graph_identity import (
    SchCompiledSchematicGraphIdentityAllocator,
    compiled_schematic_graph_design_scope,
)

VECTORS = Path(__file__).resolve().parents[1] / "parity/compiled_schematic_graph_a0_vectors.json"


def _vectors() -> dict[str, Any]:
    return json.loads(VECTORS.read_text(encoding="utf-8"))


def _allocate(
    allocator: SchCompiledSchematicGraphIdentityAllocator,
    allocation: dict[str, Any],
) -> str:
    if allocation["mode"] == "source":
        return allocator.allocate_source(
            object_type=allocation["object_type"],
            source_identity=allocation["source_identity"],
            owner_refs=allocation["owner_refs"],
        )
    return allocator.allocate_derived(
        object_type=allocation["object_type"],
        identity=allocation["identity"],
    )


def test_generated_compiled_graph_projection_is_strict_and_complete() -> None:
    graph = _vectors()["graph"]
    decoded = decode_compiled_schematic_graph_a0(json.dumps(graph).encode())
    assert isinstance(decoded, CompiledSchematicGraphA0)
    assert decoded.identity_namespace == "sch.compiled_schematic_graph.a0"
    assert len(decoded.graphical_artifact_links) == 1
    assert (
        decoded.unit_definitions[0].source_identity.sch_source_key_source_path
        == "sheet.kicad_sch"
    )
    assert json.loads(msgspec.json.encode(decoded)) == graph

    invalid = dict(graph)
    invalid["unknown_field"] = True
    with pytest.raises(msgspec.ValidationError, match="unknown field"):
        decode_compiled_schematic_graph_a0(json.dumps(invalid).encode())

    invalid_role = json.loads(json.dumps(graph))
    invalid_role["terminal_occurrences"][0]["role"] = "invented_role"
    with pytest.raises(msgspec.ValidationError):
        decode_compiled_schematic_graph_a0(json.dumps(invalid_role).encode())


def test_identity_vectors_match_the_python_producer_allocator() -> None:
    vectors = _vectors()
    identity = vectors["identity"]
    scope_input = identity["scope_input"]
    scope = compiled_schematic_graph_design_scope(**scope_input)
    assert scope == identity["normalized_scope"]
    allocator = SchCompiledSchematicGraphIdentityAllocator(design_scope=scope)
    allocations = [*identity["allocations"], *identity["supporting_allocations"]]
    for allocation in allocations:
        actual = _allocate(allocator, allocation)
        assert actual == allocation["expected"]
        parsed = uuid.UUID(actual)
        assert parsed.version == 7
        assert parsed.variant == uuid.RFC_4122
        collection = vectors["graph"][allocation["graph_collection"]]
        assert collection[allocation.get("graph_index", 0)]["id"] == actual

    for allocation in identity["canonical_allocations"]:
        actual = _allocate(allocator, allocation)
        assert actual == allocation["expected"]
        assert uuid.UUID(actual).version == 7

    assert {row["object_type"] for row in identity["allocations"]} == {
        "sch.unit_definition",
        "sch.page_definition",
        "sch.unit_occurrence",
        "sch.page_occurrence",
        "sch.hierarchy_occurrence",
        "sch.component_occurrence",
        "sch.local_net_occurrence",
        "sch.terminal_occurrence",
        "sch.hierarchy_terminal_binding",
        "sch.graphical_artifact_link",
    }


def test_identity_selector_precedence_and_scope_normalization_are_portable() -> None:
    identity = _vectors()["identity"]
    scope = identity["normalized_scope"]
    for case in identity["selector_equivalence"]:
        left_allocator = SchCompiledSchematicGraphIdentityAllocator(design_scope=scope)
        right_allocator = SchCompiledSchematicGraphIdentityAllocator(design_scope=scope)
        left = {"mode": "source", **case["left"]}
        right = {"mode": "source", **case["right"]}
        assert _allocate(left_allocator, left) == case["expected"]
        assert _allocate(right_allocator, right) == case["expected"]

    for case in identity["scope_cases"]:
        assert compiled_schematic_graph_design_scope(
            source_cad=case["source_cad"], project=case["project"]
        ) == case["expected"]


def test_identity_invalid_and_duplicate_addresses_fail_closed() -> None:
    identity = _vectors()["identity"]
    for case in identity["failures"]:
        with pytest.raises(ValueError, match=re.escape(case["error_match"])):
            if case["mode"] == "scope":
                compiled_schematic_graph_design_scope(
                    source_cad=case["source_cad"], project=case["project"]
                )
                continue
            allocator = SchCompiledSchematicGraphIdentityAllocator(
                design_scope=identity["normalized_scope"]
            )
            allocation = dict(case)
            if case["mode"] == "duplicate_source":
                allocation["mode"] = "source"
                _allocate(allocator, allocation)
            _allocate(allocator, allocation)

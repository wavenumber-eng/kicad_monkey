"""Generated compiled-graph transport and deterministic identity vectors."""

from __future__ import annotations

import json
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


def test_generated_compiled_graph_projection_is_strict_and_complete() -> None:
    graph = _vectors()["graph"]
    decoded = decode_compiled_schematic_graph_a0(json.dumps(graph).encode())
    assert isinstance(decoded, CompiledSchematicGraphA0)
    assert decoded.identity_namespace == "sch.compiled_schematic_graph.a0"
    assert len(decoded.graphical_artifact_links) == 1
    assert (
        decoded.unit_definitions[0].source_identity.sch_source_key_source_path
        == "root.kicad_sch"
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
    identity = _vectors()["identity"]
    scope_input = identity["scope_input"]
    scope = compiled_schematic_graph_design_scope(**scope_input)
    assert scope == identity["normalized_scope"]
    allocator = SchCompiledSchematicGraphIdentityAllocator(design_scope=scope)
    for allocation in identity["allocations"]:
        if allocation["mode"] == "source":
            actual = allocator.allocate_source(
                object_type=allocation["object_type"],
                source_identity=allocation["source_identity"],
                owner_refs=allocation["owner_refs"],
            )
        else:
            actual = allocator.allocate_derived(
                object_type=allocation["object_type"],
                identity=allocation["identity"],
            )
        assert actual == allocation["expected"]

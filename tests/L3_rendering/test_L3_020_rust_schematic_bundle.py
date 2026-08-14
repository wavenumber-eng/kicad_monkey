"""Native source-bundle and hierarchy parity over compiled-graph projects."""

from __future__ import annotations

import json
import shutil
import subprocess
from decimal import Decimal, InvalidOperation, ROUND_HALF_EVEN
from pathlib import Path

from kicad_monkey import KiCadDesign
from kicad_monkey.kicad_schematic_connectivity import ConnectivityGraph, snap_mm_to_iu
from kicad_monkey.kicad_schematic_occurrence import walk_schematic_occurrences
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    resolve_kicad_manifest_path,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
REFERENCE_CASES = (
    "real_world/yoshi_mainboard",
    "real_world/taillight",
    "real_world/speedy_processing_module",
    "real_world/jumperless_v5r7",
)
COORDINATE_VECTORS = PACKAGE_ROOT / "tests" / "parity" / "schematic_coordinate_iu_vectors.json"
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1


def _point(x_mm: float, y_mm: float) -> list[int]:
    return list(snap_mm_to_iu(x_mm, y_mm))


def _reference_decimal_iu(value: str) -> int | None:
    try:
        decimal = Decimal(value)
    except InvalidOperation:
        return None
    if not decimal.is_finite():
        return None
    rounded = int((decimal * 10_000).to_integral_value(rounding=ROUND_HALF_EVEN))
    return rounded if I64_MIN <= rounded <= I64_MAX else None


def test_shared_coordinate_vectors_encode_exact_python_ties_even_policy() -> None:
    payload = json.loads(COORDINATE_VECTORS.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.schematic_coordinate_iu_vectors.a0"
    for case in payload["cases"]:
        expected = case["expected_iu"]
        actual = _reference_decimal_iu(case["millimetres"])
        assert actual == (None if expected is None else int(expected)), case["name"]


def _polyline(value: object) -> dict[str, object]:
    return {
        "uuid": str(getattr(value, "uuid", "") or ""),
        "points": [_point(x, y) for x, y in getattr(value, "points", ())],
    }


def _marker(value: object) -> dict[str, object]:
    return {
        "uuid": str(getattr(value, "uuid", "") or ""),
        "at": _point(float(getattr(value, "at_x", 0.0)), float(getattr(value, "at_y", 0.0))),
    }


def _label(value: object, scope: str) -> dict[str, object]:
    shape = getattr(value, "shape", "")
    return {
        "scope": scope,
        "text": str(getattr(value, "text", "") or ""),
        "shape": str(getattr(shape, "value", shape) or ""),
        "uuid": str(getattr(value, "uuid", "") or ""),
        "at": _point(float(getattr(value, "at_x", 0.0)), float(getattr(value, "at_y", 0.0))),
    }


def _definition_summary(schematic: object, bundle_root: Path) -> dict[str, object]:
    graph = ConnectivityGraph()
    for wire in getattr(schematic, "wires", ()):
        graph.add_wire(wire)
    for bus in getattr(schematic, "buses", ()):
        graph.add_bus(bus)
    for entry in getattr(schematic, "bus_entries", ()):
        graph.add_bus_entry(entry)
    graph.add_junctions(getattr(schematic, "junctions", ()))
    components = sorted(
        [sorted([list(point) for point in component]) for component in graph.components()]
    )
    source_path = Path(str(getattr(schematic, "source_path"))).resolve()
    return {
        "source_path": source_path.relative_to(bundle_root).as_posix(),
        "sheets": [
            {
                "uuid": str(getattr(sheet, "uuid", "") or ""),
                "pins": [
                    {
                        "name": str(getattr(pin, "name", "") or ""),
                        "shape": str(
                            getattr(getattr(pin, "shape", ""), "value", getattr(pin, "shape", ""))
                            or ""
                        ),
                        "uuid": str(getattr(pin, "uuid", "") or ""),
                        "at": _point(pin.at_x, pin.at_y),
                    }
                    for pin in getattr(sheet, "pins", ())
                ],
            }
            for sheet in getattr(schematic, "sheets", ())
        ],
        "wires": [_polyline(value) for value in getattr(schematic, "wires", ())],
        "buses": [_polyline(value) for value in getattr(schematic, "buses", ())],
        "bus_entries": [
            {
                "uuid": str(getattr(value, "uuid", "") or ""),
                "at": _point(value.at_x, value.at_y),
                "size": _point(value.size_x, value.size_y),
            }
            for value in getattr(schematic, "bus_entries", ())
        ],
        "junctions": [_marker(value) for value in getattr(schematic, "junctions", ())],
        "no_connects": [_marker(value) for value in getattr(schematic, "no_connects", ())],
        "labels": [
            *[_label(value, "local") for value in getattr(schematic, "labels", ())],
            *[_label(value, "global") for value in getattr(schematic, "global_labels", ())],
            *[
                _label(value, "hierarchical")
                for value in getattr(schematic, "hierarchical_labels", ())
            ],
        ],
        "connectivity_components": components,
    }


def _request(
    case_id: str,
) -> tuple[
    dict[str, object],
    set[str],
    list[dict[str, object]],
    dict[str, dict[str, object]],
]:
    case = get_kicad_corpus_case(case_id)
    assert case is not None
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None
    design = KiCadDesign.from_project_file(project_path)
    top = design.top_schematic
    assert top is not None and top.source_path is not None
    occurrences = list(walk_schematic_occurrences(top))
    schematic_paths = sorted(
        {
            str(Path(occurrence.schematic.source_path).resolve())
            for occurrence in occurrences
            if occurrence.schematic.source_path is not None
        }
    )
    bundle_root = project_path.parent.resolve()
    request = {
        "bundle_root": str(bundle_root),
        "project_path": str(project_path.resolve()),
        "root_schematic_path": str(Path(top.source_path).resolve()),
        "schematic_paths": schematic_paths,
    }
    expected_occurrences = []
    for occurrence in occurrences:
        occurrence_source = occurrence.schematic.source_path
        assert occurrence_source is not None
        expected_occurrences.append(
            {
            "source_path": Path(occurrence_source).resolve().relative_to(bundle_root).as_posix(),
            "parent_index": occurrence.parent.index if occurrence.parent else None,
            "occurrence_address": occurrence.occurrence_address,
            "effective_in_bom": occurrence.effective_in_bom,
            "effective_on_board": occurrence.effective_on_board,
            "effective_dnp": occurrence.effective_dnp,
            "effective_exclude_from_sim": occurrence.effective_exclude_from_sim,
            }
        )
    expected_definitions = {
        Path(path).relative_to(bundle_root).as_posix() for path in map(Path, schematic_paths)
    }
    schematic_by_path = {
        str(Path(occurrence.schematic.source_path).resolve()): occurrence.schematic
        for occurrence in occurrences
        if occurrence.schematic.source_path is not None
    }
    expected_source_models: dict[str, dict[str, object]] = {}
    for schematic in schematic_by_path.values():
        summary = _definition_summary(schematic, bundle_root)
        source_key = summary["source_path"]
        assert isinstance(source_key, str)
        expected_source_models[source_key] = summary
    return request, expected_definitions, expected_occurrences, expected_source_models


def test_native_source_bundle_matches_python_hierarchy_inventory() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for native source-bundle validation"
    requests_and_counts = [_request(case_id) for case_id in REFERENCE_CASES]
    completed = subprocess.run(
        [
            cargo,
            "run",
            "--locked",
            "--quiet",
            "--package",
            "kicad-monkey-core",
            "--example",
            "schematic_bundle_gate",
        ],
        cwd=PACKAGE_ROOT,
        input="".join(
            f"{json.dumps(request, separators=(',', ':'))}\n"
            for request, _definitions, _occurrences, _source_models in requests_and_counts
        ),
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    results = [json.loads(line) for line in completed.stdout.splitlines()]
    assert len(results) == len(REFERENCE_CASES)
    for result, (_request_payload, definitions, occurrences, source_models) in zip(
        results, requests_and_counts, strict=True
    ):
        assert set(result["definition_paths"]) == definitions
        assert result["occurrences"] == occurrences
        assert {
            definition["source_path"]: definition for definition in result["definitions"]
        } == source_models
        assert result["total_bytes"] > 0

"""Native source-bundle and hierarchy parity over compiled-graph projects."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

from kicad_monkey import KiCadDesign
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


def _request(case_id: str) -> tuple[dict[str, object], set[str], list[dict[str, object]]]:
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
    return request, expected_definitions, expected_occurrences


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
            for request, _definitions, _occurrences in requests_and_counts
        ),
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    results = [json.loads(line) for line in completed.stdout.splitlines()]
    assert len(results) == len(REFERENCE_CASES)
    for result, (_request_payload, definitions, occurrences) in zip(
        results, requests_and_counts, strict=True
    ):
        assert set(result["definition_paths"]) == definitions
        assert result["occurrences"] == occurrences
        assert result["total_bytes"] > 0

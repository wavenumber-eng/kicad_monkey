"""Rack-owned native project JSON parity and mutation gate."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

from _suite_paths import KICAD_PACKAGE_ROOT, TEST_CORPUS_ROOT
from kicad_monkey import KiCadProject

PACKAGE_ROOT = KICAD_PACKAGE_ROOT
PROJECT_ROOT = TEST_CORPUS_ROOT / "kicad"
EXPECTED_PROJECT_COUNT = 217
EXPECTED_PROJECT_BYTES = 3_048_297
BATCH_SIZE = 24


def _run(command: list[str], *, timeout: int = 900) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )
    return completed


def _example(name: str) -> Path:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust project gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-core",
            "--example",
            name,
        ]
    )
    return (
        PACKAGE_ROOT
        / "target/debug/examples"
        / (f"{name}.exe" if os.name == "nt" else name)
    )


def _project_files() -> list[Path]:
    paths = sorted(PROJECT_ROOT.rglob("*.kicad_pro"))
    assert len(paths) == EXPECTED_PROJECT_COUNT, (
        f"expected {EXPECTED_PROJECT_COUNT} restored project files under {PROJECT_ROOT}; "
        "verify tests/corpus/kicad.zip and its extraction"
    )
    assert sum(path.stat().st_size for path in paths) == EXPECTED_PROJECT_BYTES
    return paths


def test_native_project_model_matches_python_across_restored_corpus() -> None:
    executable = _example("project_gate")
    paths = _project_files()
    actual: list[dict[str, Any]] = []
    for start in range(0, len(paths), BATCH_SIZE):
        batch = paths[start : start + BATCH_SIZE]
        evidence = json.loads(
            _run([str(executable), *(str(path) for path in batch)]).stdout
        )
        assert evidence["schema"] == "kicad_monkey.project_gate_evidence.a0"
        assert evidence["file_count"] == len(batch)
        actual.extend(evidence["files"])
    assert len(actual) == EXPECTED_PROJECT_COUNT
    for path, native in zip(paths, actual):
        assert native == _python_projection(path)


def test_native_project_mutation_matches_python(tmp_path: Path) -> None:
    source = {
        "meta": {"filename": "mutation.kicad_pro"},
        "text_variables": {"TITLE": "Before"},
        "schematic": {"variants": [{"name": "Production"}]},
        "future": {"preserved": [1, 2, 3]},
    }
    input_path = tmp_path / "input.kicad_pro"
    output_path = tmp_path / "output.kicad_pro"
    input_path.write_text(
        json.dumps(source, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    evidence = json.loads(
        _run(
            [str(_example("project_mutation_gate")), str(input_path), str(output_path)]
        ).stdout
    )
    assert evidence == {
        "schema": "kicad_monkey.project_mutation_gate.a0",
        "changed_text": True,
        "renamed": True,
        "changed_path": True,
    }

    python_project = KiCadProject.from_file(input_path)
    python_project.set_text_variable("RUST_GATE", "enabled")
    python_project.add_variant("Rust Gate", description="native parity")
    assert python_project.rename_variant("Rust Gate", "Rust Gate Renamed")
    python_project.set_path("meta.rust_gate", True)
    assert output_path.read_text(encoding="utf-8") == python_project.to_text()
    reloaded = KiCadProject.from_file(output_path)
    assert reloaded.get_text_variable("RUST_GATE") == "enabled"
    assert reloaded.get_variant("Rust Gate Renamed") is not None
    assert reloaded.get_path("future.preserved") == [1, 2, 3]


def test_native_project_resource_and_io_oracles_are_rack_owned() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust project gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-core",
            "--test",
            "project",
            "--",
            "--test-threads",
            "2",
        ]
    )


def _python_projection(path: Path) -> dict[str, Any]:
    source = path.read_bytes()
    project = KiCadProject.from_file(path)
    canonical = project.to_text().encode()
    net_settings = project.net_settings
    board_settings = project.board_design_settings
    assert net_settings is not None
    assert board_settings is not None
    tuning = board_settings.tuning_pattern_settings
    assert tuning is not None
    return {
        "path": str(path),
        "source_bytes": len(source),
        "source_sha256": hashlib.sha256(source).hexdigest(),
        "canonical_sha256": hashlib.sha256(canonical).hexdigest(),
        "exact_write": True,
        "stable_canonical_write": True,
        "text_variables": [list(item) for item in project.text_variables.items()],
        "variants": [
            {"name": variant.name, "description": variant.description}
            for variant in project.variants
        ],
        "net_settings": {
            "classes": [_net_class(value) for value in net_settings.classes],
            "assignments": [
                [name, values]
                for name, values in net_settings.netclass_assignments.items()
            ],
            "patterns": [
                {"pattern": value.pattern, "netclass_name": value.netclass_name}
                for value in net_settings.netclass_patterns
            ],
            "colors": [list(item) for item in net_settings.net_colors.items()],
        },
        "board_design_settings": {
            "diff_pair_dimensions": [
                {"width": value.width, "gap": value.gap, "via_gap": value.via_gap}
                for value in board_settings.diff_pair_dimensions
            ],
            "tuning": {
                "diff_pair_defaults": _tuning(tuning.diff_pair_defaults),
                "diff_pair_skew_defaults": _tuning(tuning.diff_pair_skew_defaults),
                "single_track_defaults": _tuning(tuning.single_track_defaults),
            },
        },
    }


def _net_class(value: Any) -> dict[str, Any]:
    fields = (
        "name",
        "track_width",
        "clearance",
        "diff_pair_gap",
        "diff_pair_width",
        "diff_pair_via_gap",
        "via_diameter",
        "via_drill",
        "microvia_diameter",
        "microvia_drill",
        "bus_width",
        "wire_width",
        "pcb_color",
        "schematic_color",
        "line_style",
        "priority",
        "tuning_profile",
    )
    return {field: getattr(value, field) for field in fields}


def _tuning(value: Any) -> dict[str, Any]:
    fields = (
        "spacing",
        "min_amplitude",
        "max_amplitude",
        "corner_style",
        "corner_radius_percentage",
        "single_sided",
    )
    return {field: getattr(value, field) for field in fields}

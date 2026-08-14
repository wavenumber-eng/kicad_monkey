"""Rack-owned native schematic semantic round-trip and writer gate."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

from _suite_paths import KICAD_PACKAGE_ROOT

PACKAGE_ROOT = KICAD_PACKAGE_ROOT
SCHEMATIC_INPUTS = PACKAGE_ROOT / "tests/L1_parsing/cases/schematics/input"


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


def _roundtrip_executable() -> Path:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust schematic writer gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "schematic_roundtrip_gate",
        ]
    )
    return PACKAGE_ROOT / "target/debug/examples" / (
        "schematic_roundtrip_gate.exe"
        if os.name == "nt"
        else "schematic_roundtrip_gate"
    )


def _schematic_inputs() -> list[Path]:
    paths = sorted(SCHEMATIC_INPUTS.glob("*.kicad_sch"))
    assert len(paths) == 8, (
        "the durable native schematic round-trip set must contain all eight "
        f"package-local reference inputs: {SCHEMATIC_INPUTS}"
    )
    return paths


def test_native_owned_schematic_roundtrip_is_exact_and_semantically_stable() -> None:
    paths = _schematic_inputs()
    evidence = json.loads(
        _run([str(_roundtrip_executable()), *(str(path) for path in paths)]).stdout
    )
    assert evidence["schema"] == "kicad_monkey.schematic_roundtrip_evidence.a0"
    assert evidence["file_count"] == len(paths)
    assert evidence["source_bytes"] == sum(path.stat().st_size for path in paths)
    assert evidence["semantic_decode_passes_per_file"] == 2
    assert evidence["exact_first_writes"] == len(paths)
    assert evidence["stable_second_writes"] == len(paths)
    assert [Path(item["path"]).resolve() for item in evidence["files"]] == [
        path.resolve() for path in paths
    ]
    assert sum(item["symbols"] for item in evidence["files"]) > 0
    assert sum(item["connectivity_objects"] for item in evidence["files"]) > 0


def test_native_schematic_mutation_and_resource_oracles_are_rack_owned() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust schematic writer gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "schematic_document",
        ]
    )


def test_native_schematic_roundtrip_failures_name_the_file_and_stage(
    tmp_path: Path,
) -> None:
    malformed = tmp_path / "malformed.kicad_sch"
    malformed.write_text("(kicad_sch (symbol", encoding="utf-8")
    completed = subprocess.run(
        [str(_roundtrip_executable()), str(malformed)],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=30,
        check=False,
    )
    assert completed.returncode != 0
    assert malformed.name in completed.stderr
    assert malformed.parent.name in completed.stderr
    assert "owned read" in completed.stderr

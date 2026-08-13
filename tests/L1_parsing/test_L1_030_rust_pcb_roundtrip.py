"""Rack-owned native PCB semantic round-trip and stable-write corpus gate."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

from conftest import get_all_pcb_files
from _suite_paths import KICAD_PACKAGE_ROOT

PACKAGE_ROOT = KICAD_PACKAGE_ROOT


def _run(command: list[str], *, timeout: int = 240) -> subprocess.CompletedProcess[str]:
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
    assert cargo is not None, "cargo is required for the Rust PCB round-trip gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "pcb_roundtrip_gate",
        ]
    )
    return PACKAGE_ROOT / "target/debug/examples" / (
        "pcb_roundtrip_gate.exe" if os.name == "nt" else "pcb_roundtrip_gate"
    )


def test_native_owned_pcb_roundtrip_is_exact_and_semantically_stable_on_corpus() -> None:
    boards = get_all_pcb_files()
    assert boards, "required PCB corpus contains no .kicad_pcb files"
    evidence = json.loads(
        _run([str(_roundtrip_executable()), *(str(path) for path in boards)]).stdout
    )
    assert evidence["schema"] == "kicad_monkey.pcb_roundtrip_evidence.a0"
    assert evidence["file_count"] == len(boards)
    assert evidence["source_bytes"] == sum(path.stat().st_size for path in boards)
    assert evidence["semantic_decode_passes_per_file"] == 2
    assert evidence["exact_first_writes"] == len(boards)
    assert evidence["stable_second_writes"] == len(boards)
    assert len(evidence["files"]) == len(boards)
    assert [Path(item["path"]).resolve() for item in evidence["files"]] == [
        path.resolve() for path in boards
    ]


def test_native_owned_pcb_mutation_and_resource_oracles_are_rack_owned() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust PCB writer gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "pcb_document_slice",
        ]
    )

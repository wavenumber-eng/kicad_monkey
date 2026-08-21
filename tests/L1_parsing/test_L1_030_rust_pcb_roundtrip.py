"""Rack-owned native PCB semantic round-trip and stable-write corpus gate."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

from _suite_paths import KICAD_PACKAGE_ROOT
from kicad_monkey.testing.corpus import get_kicad_corpus_root

PACKAGE_ROOT = KICAD_PACKAGE_ROOT


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
    boards = _authoritative_pcb_inputs()
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


def _authoritative_pcb_inputs() -> list[Path]:
    """Return every PCB below input/, excluding derived fixture trees."""
    root = get_kicad_corpus_root()
    boards = []
    for path in root.rglob("*.kicad_pcb"):
        parts = path.relative_to(root).parts
        if "input" in parts and not {"output", "reference_output"}.intersection(parts):
            boards.append(path)
    boards.sort()
    assert boards, f"required authoritative PCB input corpus is empty: {root}"
    return boards


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


def test_native_roundtrip_failures_name_the_file_and_stage(tmp_path: Path) -> None:
    malformed = tmp_path / "malformed.kicad_pcb"
    malformed.write_text("(kicad_pcb (footprint", encoding="utf-8")
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

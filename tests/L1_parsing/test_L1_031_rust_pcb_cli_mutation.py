"""Rack-owned KiCad CLI acceptance gate for native Rust PCB mutations."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import TypedDict, cast

import pytest

from _suite_paths import KICAD_PACKAGE_ROOT
from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli

PACKAGE_ROOT = KICAD_PACKAGE_ROOT
FIXTURE = PACKAGE_ROOT / "tests/parity/pcb_native_mutation_a0.kicad_pcb"


class CaseEvidence(TypedDict):
    operation: str
    output: str
    changed: bool
    reparsed: bool
    stable_second_write: bool
    unrelated_semantics_preserved: bool


class GateEvidence(TypedDict):
    schema: str
    cases: list[CaseEvidence]


def _run(command: list[str], *, timeout: int = 120) -> subprocess.CompletedProcess[str]:
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


def _mutation_executable() -> Path:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the native PCB mutation gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "pcb_mutation_gate",
        ]
    )
    return PACKAGE_ROOT / "target/debug/examples" / (
        "pcb_mutation_gate.exe" if os.name == "nt" else "pcb_mutation_gate"
    )


@pytest.fixture(scope="module")
def mutation_evidence(tmp_path_factory: pytest.TempPathFactory) -> GateEvidence:
    output_dir = tmp_path_factory.mktemp("rust-pcb-mutations")
    return cast(
        GateEvidence,
        json.loads(
            _run([str(_mutation_executable()), str(FIXTURE), str(output_dir)]).stdout
        ),
    )


def test_native_rust_pcb_mutations_preserve_unrelated_semantics_and_stabilize(
    mutation_evidence: GateEvidence,
) -> None:
    evidence = mutation_evidence
    assert evidence["schema"] == "kicad_monkey.pcb_mutation_cli_evidence.a0"
    assert [case["operation"] for case in evidence["cases"]] == [
        "property_update",
        "property_insert",
        "property_remove",
        "stable_layer_edit",
        "top_level_remove",
    ]
    assert all(
        case["changed"]
        and case["reparsed"]
        and case["stable_second_write"]
        and case["unrelated_semantics_preserved"]
        for case in evidence["cases"]
    )


def test_native_rust_pcb_mutations_are_accepted_by_kicad_cli(
    mutation_evidence: GateEvidence,
) -> None:
    cli = resolve_kicad_cli(required_capability="pcb_svg")
    if cli is None or not Path(cli).exists():
        pytest.skip("no PCB-capable kicad-cli resolvable on this machine")

    cli_env = os.environ.copy()
    cli_env.update(kicad_cli_subprocess_env(Path(cli)) or {})
    for case in mutation_evidence["cases"]:
        output = Path(case["output"])
        completed = subprocess.run(
            [str(cli), "pcb", "upgrade", "--force", str(output)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=120,
            check=False,
            env=cli_env,
        )
        diagnostic = (completed.stdout or "") + (completed.stderr or "")
        assert completed.returncode == 0, (
            f"kicad-cli rejected Rust {case['operation']} output {output.name}\n"
            f"--- stdout/stderr (first 800 chars) ---\n{diagnostic[:800]}"
        )

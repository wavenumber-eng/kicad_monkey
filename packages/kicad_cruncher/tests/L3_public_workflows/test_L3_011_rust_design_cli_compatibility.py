"""Behavioral oracle checks shared by the Python and Rust design CLIs."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest
from kicad_monkey import KiCadDesign

_PACKAGE_ROOT = Path(__file__).resolve().parents[2]
_WORKSPACE = Path(__file__).resolve().parents[4]
_RUST_EXE = _WORKSPACE / "target" / "debug" / (
    "kicad-cruncher.exe" if os.name == "nt" else "kicad-cruncher"
)
_RUST_NETLIST_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "design_netlist_json_oracle.exe"
    if os.name == "nt"
    else "design_netlist_json_oracle"
)
_PROJECT = (
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "hlr_test"
    / "hlr_test.kicad_pro"
)
_DESIGN_HELP_MARKERS = (
    "design review bundle",
    "enriched black-and-white schematic SVGs",
    "enriched PCB copper-layer SVGs",
    "KiCad-native netlist JSON",
    "KiCad S-expression netlist",
    "project metadata",
    "schematic hierarchy",
    "components",
    "nets",
    "default: ./output/design",
)


@pytest.fixture(scope="module", autouse=True)
def _build_rust_cli() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "kicad-cruncher-cli",
            "--bins",
            "--example",
            "design_netlist_json_oracle",
        ],
        cwd=_WORKSPACE,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert _RUST_EXE.is_file()
    assert _RUST_NETLIST_ORACLE.is_file()


def _run(command: list[str]) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env.update({"NO_COLOR": "1", "PYTHONUTF8": "1"})
    return subprocess.run(
        command,
        cwd=_PACKAGE_ROOT,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=30,
        check=False,
    )


@pytest.mark.parametrize("alias", ("design", "design-review", "dr"))
def test_rust_design_help_matches_the_python_cli_contract(alias: str) -> None:
    python = _run([sys.executable, "-m", "kicad_cruncher", alias, "--help"])
    rust = _run([str(_RUST_EXE), alias, "--help"])

    assert python.returncode == rust.returncode == 0
    assert python.stderr == rust.stderr == ""
    for marker in _DESIGN_HELP_MARKERS:
        assert marker in python.stdout
        assert marker in rust.stdout


@pytest.mark.parametrize("alias", ("design", "design-review", "dr"))
def test_rust_usage_errors_match_the_python_cli_contract(alias: str) -> None:
    arguments = [alias, "--not-a-real-option"]
    python = _run([sys.executable, "-m", "kicad_cruncher", *arguments])
    rust = _run([str(_RUST_EXE), *arguments])

    assert python.returncode == rust.returncode == 2
    assert python.stdout == rust.stdout == ""
    for completed in (python, rust):
        assert "usage: kicad-cruncher" in completed.stderr
        assert "unrecognized arguments: --not-a-real-option" in completed.stderr
        assert "Traceback" not in completed.stderr


@pytest.mark.parametrize("source", (_PROJECT, _PROJECT.with_suffix(".kicad_sch")))
def test_rust_netlist_json_matches_the_python_design_oracle_exactly(
    source: Path,
) -> None:
    completed = _run([str(_RUST_NETLIST_ORACLE), str(source)])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    assert json.loads(completed.stdout) == KiCadDesign.from_file(source).to_netlist_json()

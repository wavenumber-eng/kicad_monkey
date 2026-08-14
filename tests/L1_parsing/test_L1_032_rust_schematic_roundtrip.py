"""Rack-owned native schematic semantic round-trip and writer gate."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

import pytest

from _suite_paths import KICAD_PACKAGE_ROOT
from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadSchematic
from kicad_monkey.kicad_sexpr import parse_sexp

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


def _mutation_executable() -> Path:
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
            "schematic_mutation_gate",
        ]
    )
    return PACKAGE_ROOT / "target/debug/examples" / (
        "schematic_mutation_gate.exe"
        if os.name == "nt"
        else "schematic_mutation_gate"
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


@pytest.fixture(scope="module")
def mutation_evidence(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Any]:
    output = tmp_path_factory.mktemp("rust-schematic-mutation") / "mutated.kicad_sch"
    source = SCHEMATIC_INPUTS / "led_component.kicad_sch"
    return json.loads(
        _run([str(_mutation_executable()), str(source), str(output)]).stdout
    )


def test_inserted_property_has_kicad_placement_and_python_semantics(
    mutation_evidence: dict[str, Any],
) -> None:
    assert mutation_evidence["schema"] == (
        "kicad_monkey.schematic_mutation_cli_evidence.a0"
    )
    assert mutation_evidence["changed"]
    assert mutation_evidence["stable_second_write"]
    assert mutation_evidence["unrelated_semantics_preserved"]
    assert mutation_evidence["inserted_property_has_complete_placement"]

    output = Path(mutation_evidence["output"])
    tree = parse_sexp(output.read_text(encoding="utf-8"))
    matches = _find_property_forms(tree, "Rust Native Property")
    assert len(matches) == 1
    at_forms = [child for child in matches[0] if _form_head(child) == "at"]
    assert at_forms == [["at", 0, 0, 0]]

    schematic = KiCadSchematic.from_file(output)
    symbol = next(
        item
        for item in schematic.symbols
        if item.uuid == mutation_evidence["symbol_uuid"]
    )
    inserted = [prop for prop in symbol.properties if prop.key == "Rust Native Property"]
    assert len(inserted) == 1
    assert inserted[0].value == "source-preserving"
    assert (inserted[0].at_x, inserted[0].at_y, inserted[0].at_angle) == (0, 0, 0)


def test_inserted_property_is_accepted_by_schematic_capable_kicad_cli(
    mutation_evidence: dict[str, Any], tmp_path: Path
) -> None:
    cli = resolve_kicad_cli()
    if cli is None or not Path(cli).exists():
        pytest.skip("no kicad-cli resolvable on this machine")
    environment = os.environ.copy()
    environment.update(kicad_cli_subprocess_env(Path(cli)) or {})
    probe = subprocess.run(
        [str(cli), "sch", "export", "netlist", "--help"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=30,
        check=False,
        env=environment,
    )
    if probe.returncode != 0 or "export netlist" not in probe.stdout + probe.stderr:
        pytest.skip("resolved kicad-cli has no schematic netlist exporter")

    netlist = tmp_path / "mutated.net"
    output = Path(mutation_evidence["output"])
    completed = subprocess.run(
        [
            str(cli),
            "sch",
            "export",
            "netlist",
            "--output",
            str(netlist),
            str(output),
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=120,
        check=False,
        env=environment,
    )
    diagnostic = (completed.stdout or "") + (completed.stderr or "")
    assert completed.returncode == 0, (
        f"kicad-cli rejected Rust schematic mutation {output.name}\n"
        f"--- stdout/stderr (first 800 chars) ---\n{diagnostic[:800]}"
    )
    assert netlist.exists() and netlist.stat().st_size > 0


def _find_property_forms(value: object, name: str) -> list[list[Any]]:
    if not isinstance(value, list):
        return []
    matches: list[list[Any]] = []
    if len(value) >= 2 and value[0] == "property" and value[1] == name:
        matches.append(value)
    for child in value:
        matches.extend(_find_property_forms(child, name))
    return matches


def _form_head(value: object) -> object:
    return value[0] if isinstance(value, list) and value else None

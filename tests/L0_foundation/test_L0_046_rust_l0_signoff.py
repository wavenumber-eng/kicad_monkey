"""Rack-owned signoff for Rust L0, TypeSpec generation, and WASM."""

from __future__ import annotations

import hashlib
from pathlib import Path
import shutil
import subprocess
import tomllib


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = PACKAGE_ROOT / "contracts" / "generated" / "schema"
SCOPE_PATH = PACKAGE_ROOT / "tests" / "parity" / "scope.toml"


def _run(command: list[str], *, timeout: int = 300) -> None:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )


def _schema_hashes() -> dict[str, str]:
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(SCHEMA_ROOT.glob("*.json"))
    }


def test_l0_parity_registry_has_no_required_implementation_gap() -> None:
    payload = tomllib.loads(SCOPE_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.parity_scope.a0"
    assert payload["review_state"] == "accepted"
    surfaces = payload["surfaces"]
    assert len({surface["id"] for surface in surfaces}) == len(surfaces)
    assert all(
        surface["status"] in {"closed", "review_ready"}
        for surface in surfaces
        if surface["disposition"] in {"required", "replaced_by_contract"}
    )
    for surface in surfaces:
        for evidence in surface["evidence"]:
            assert (PACKAGE_ROOT / evidence).exists(), f"missing {surface['id']} evidence: {evidence}"


def test_typespec_and_generated_rust_contracts_are_clean() -> None:
    npm = shutil.which("npm")
    cargo = shutil.which("cargo")
    assert npm is not None, "npm is required for TypeSpec generation checks"
    assert cargo is not None, "cargo is required for Rust contract generation checks"
    assert (PACKAGE_ROOT / "node_modules" / ".bin" / "tsp.cmd").exists(), (
        "TypeSpec dependencies are missing; run `npm ci`"
    )

    before = _schema_hashes()
    _run([npm, "run", "check:typespec"])
    _run([npm, "run", "generate:contracts"])
    assert _schema_hashes() == before, "TypeSpec-generated JSON Schemas were stale"
    _run(
        [
            cargo,
            "run",
            "--package",
            "kicad-monkey-codegen",
            "--locked",
            "--",
            "--check",
        ]
    )


def test_rust_l0_quality_and_real_wasm_smoke() -> None:
    cargo = shutil.which("cargo")
    runner = shutil.which("wasm-bindgen-test-runner")
    assert cargo is not None, "cargo is required for Rust L0 signoff"
    assert runner is not None, (
        "install the lock-compatible WASM runner with "
        "`cargo install wasm-bindgen-cli --version 0.2.127 --locked`"
    )

    _run([cargo, "fmt", "--all", "--", "--check"])
    _run([cargo, "check", "--workspace", "--all-targets", "--locked"])
    _run(
        [
            cargo,
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]
    )
    _run(
        [
            cargo,
            "test",
            "--package",
            "kicad-monkey-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--locked",
        ]
    )

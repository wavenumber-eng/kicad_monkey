"""Rack-owned signoff for Rust L0, TypeSpec generation, and WASM."""

from __future__ import annotations

import hashlib
from pathlib import Path
import shutil
import subprocess
import tomllib

from wn_dev_std.rust_policy import check_rust_policy


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


def test_l0_parity_registry_governs_required_implementation_gaps() -> None:
    payload = tomllib.loads(SCOPE_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.parity_scope.a0"
    assert payload["review_state"] in {"in_progress", "accepted"}
    surfaces = payload["surfaces"]
    assert len({surface["id"] for surface in surfaces}) == len(surfaces)
    rack_case_paths: dict[str, str] = {}
    for stratum_path in (PACKAGE_ROOT / "tests").rglob("STRATUM.toml"):
        stratum = tomllib.loads(stratum_path.read_text(encoding="utf-8"))
        for subtest in stratum.get("subtests", []):
            rack_case = subtest.get("id")
            if rack_case is None:
                stem_parts = (
                    Path(subtest["file"]).stem.removeprefix("test_").split("_")
                )
                if len(stem_parts) < 2:
                    continue
                rack_case = "_".join(stem_parts[:2])
            test_path = (stratum_path.parent / subtest["file"]).relative_to(
                PACKAGE_ROOT
            )
            normalized_path = test_path.as_posix()
            assert rack_case not in rack_case_paths, f"duplicate Rack id: {rack_case}"
            rack_case_paths[rack_case] = normalized_path
    for surface in surfaces:
        disposition = surface["disposition"]
        status = surface["status"]
        assert disposition in {
            "required",
            "replaced_by_contract",
            "required_bounded_slice",
            "deferred_geometer_service",
        }, surface["id"]
        assert status in {"planned", "review_ready", "closed", "deferred"}, (
            surface["id"]
        )
        if disposition in {"required", "replaced_by_contract"}:
            assert status in {"closed", "review_ready"}, surface["id"]
        elif disposition == "required_bounded_slice":
            assert status in {"planned", "review_ready", "closed"}, surface["id"]
        else:
            assert disposition == "deferred_geometer_service", surface["id"]
            assert status == "deferred", surface["id"]
        if status == "planned":
            assert payload["review_state"] == "in_progress", surface["id"]
            assert not surface["rack_cases"], surface["id"]
        elif disposition == "required_bounded_slice":
            assert surface["rack_cases"], surface["id"]
        if disposition == "required_bounded_slice":
            promotion_fields = (
                "vector_evidence",
                "implementation_evidence",
                "semantic_resource_evidence",
            )
            for field in promotion_fields:
                assert field in surface, f"missing {surface['id']} field: {field}"
                if status == "planned":
                    assert not surface[field], f"premature {surface['id']} {field}"
                else:
                    assert surface[field], f"missing {surface['id']} {field}"
                for evidence in surface[field]:
                    assert (PACKAGE_ROOT / evidence).exists(), (
                        f"missing {surface['id']} {field}: {evidence}"
                    )
        if surface["id"] == "plotter_ir.phase5_exit" and status != "planned":
            assert surface["rack_cases"] == ["L3_022"]
            assert (
                rack_case_paths["L3_022"]
                == "tests/L3_rendering/test_L3_022_rust_phase5_exit.py"
            )
            assert (
                "tests/L3_rendering/test_L3_022_rust_phase5_exit.py"
                in surface["semantic_resource_evidence"]
            )
        for rack_case in surface["rack_cases"]:
            assert rack_case in rack_case_paths, (
                f"unknown {surface['id']} Rack case: {rack_case}"
            )
        for evidence in surface["evidence"]:
            assert (PACKAGE_ROOT / evidence).exists(), (
                f"missing {surface['id']} evidence: {evidence}"
            )
    if payload["review_state"] == "accepted":
        assert all(
            surface["status"] == "closed"
            for surface in surfaces
            if surface["disposition"] == "required_bounded_slice"
        )


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


def test_wn_dev_std_rust_hygiene_profile_passes() -> None:
    config = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))[
        "tool"
    ]["wn_dev_std"]
    checks = check_rust_policy(PACKAGE_ROOT, config, "rust-app")
    failures = [f"{check.name}: {check.detail}" for check in checks if not check.passed]
    assert not failures, "\n".join(failures)


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


def test_wasm_operation_families_build_independently() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for WASM feature checks"
    for feature in ("sexpr", "footprint", "symbol"):
        _run(
            [
                cargo,
                "check",
                "--package",
                "kicad-monkey-wasm",
                "--target",
                "wasm32-unknown-unknown",
                "--no-default-features",
                "--features",
                feature,
                "--locked",
            ]
        )

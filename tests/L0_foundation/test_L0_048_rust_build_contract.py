"""Rack ownership for semantic build-node validation and WASM exposure."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = PACKAGE_ROOT / "contracts" / "generated" / "schema" / "BuildRequest.json"


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


def test_build_contract_has_semantic_limits_and_native_wasm_evidence() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust build-operation gate"
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    assert {"root", "max_output_bytes", "max_depth", "max_nodes"} <= set(
        schema["required"]
    )

    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-contracts",
        ]
    )
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-wasm",
            "--target",
            "wasm32-unknown-unknown",
        ]
    )

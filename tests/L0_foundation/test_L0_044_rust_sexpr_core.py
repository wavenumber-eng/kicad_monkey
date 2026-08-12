"""Rack bridge for the first Rust S-expression foundation slice."""

from __future__ import annotations

from pathlib import Path
import shutil
import subprocess


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def test_rust_sexpr_core_passes_locked_cargo_tests() -> None:
    """Run the dependency-free Rust parser tests through package-local Rack."""
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the promoted Rust foundation lane"

    completed = subprocess.run(
        [
            cargo,
            "test",
            "--workspace",
            "--locked",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )

    assert completed.returncode == 0, (
        "Rust L0 workspace tests failed.\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )

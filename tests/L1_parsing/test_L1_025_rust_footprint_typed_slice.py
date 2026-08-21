"""Rack ownership for the first Rust typed footprint reader/writer slice."""

from __future__ import annotations

from pathlib import Path
import shutil
import subprocess


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def test_rust_footprint_view_and_focused_writer_pass() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust typed footprint gate"
    completed = subprocess.run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "footprint_typed_slice",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    assert completed.returncode == 0, (
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )

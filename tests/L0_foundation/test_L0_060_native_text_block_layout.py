"""Native bounded KiCad multiline and tabbed outline-text gate."""

from __future__ import annotations

from pathlib import Path
import subprocess

PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def test_focused_native_text_block_layout_suite_passes() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "text_block_layout",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

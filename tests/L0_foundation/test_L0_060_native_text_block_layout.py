"""Native bounded KiCad multiline and tabbed outline-text gate."""

from __future__ import annotations

from pathlib import Path
import subprocess

PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def test_focused_native_text_block_layout_suite_passes() -> None:
    unit = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--lib",
            "text_block_layout::tests",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert unit.returncode == 0, unit.stdout + unit.stderr

    integration = subprocess.run(
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
    assert integration.returncode == 0, integration.stdout + integration.stderr

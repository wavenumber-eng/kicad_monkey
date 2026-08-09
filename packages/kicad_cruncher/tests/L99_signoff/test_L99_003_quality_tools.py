"""Rack-owned source quality tool checks."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


PACKAGE_ROOT = _project_root()
QUALITY_STATUS_DOC = PACKAGE_ROOT / "docs" / "design" / "quality-signoff-status.md"


def _run_module(module: str, *args: str) -> subprocess.CompletedProcess[str]:
    """Run a Python module from the repository root and capture output."""
    return subprocess.run(
        [sys.executable, "-m", module, *args],
        cwd=PACKAGE_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def test_package_source_ruff_check_passes() -> None:
    """Verify source-wide linting is part of Rack signoff."""
    completed = _run_module("ruff", "check", ".")

    assert completed.returncode == 0, completed.stderr + completed.stdout


def test_package_pyright_check_passes() -> None:
    """Verify package-wide type checking is part of Rack signoff."""
    completed = _run_module("pyright")

    assert completed.returncode == 0, completed.stderr + completed.stdout


def test_quality_status_documents_broader_ratchet_state() -> None:
    """Verify the quality gate documents the active ratchet strategy."""
    text = QUALITY_STATUS_DOC.read_text(encoding="utf-8")

    assert "py_signoff" in text
    assert "package-wide ruff" in text
    assert "package-wide pyright" in text
    assert "design" in text


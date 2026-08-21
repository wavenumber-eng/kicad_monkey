"""Installed-artifact coverage for the promoted pure-Rust design CLI."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

_PACKAGE_ROOT = Path(__file__).resolve().parents[2]
_WORKSPACE = _PACKAGE_ROOT.parents[1]
_INSTALL_TEST = _PACKAGE_ROOT / "tests" / "support_scripts" / "rust_cli_install_test.py"
_VERIFY_CANDIDATE = _WORKSPACE / "scripts" / "verify_phase7_rust_cli.py"


@pytest.mark.skipif(os.name != "nt", reason="the promoted artifact targets Windows x64")
def test_installed_rust_cli_runs_design_without_python(tmp_path: Path) -> None:
    completed = subprocess.run(
        [
            sys.executable,
            str(_INSTALL_TEST),
            "--workspace",
            str(_WORKSPACE),
            "--artifact-dir",
            str(tmp_path / "artifact"),
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
        timeout=600,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert len(list((tmp_path / "artifact").glob("*.zip"))) == 1
    assert len(list((tmp_path / "artifact").glob("*.json"))) == 1
    valid = subprocess.run(
        [sys.executable, str(_VERIFY_CANDIDATE), str(tmp_path / "artifact")],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=True,
        timeout=30,
    )
    assert valid.stderr == ""
    mismatch = subprocess.run(
        [
            sys.executable,
            str(_VERIFY_CANDIDATE),
            str(tmp_path / "artifact"),
            "--expected-version",
            "0.0.0",
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    assert mismatch.returncode != 0
    assert "does not match" in mismatch.stderr
    wrong_commit = subprocess.run(
        [
            sys.executable,
            str(_VERIFY_CANDIDATE),
            str(tmp_path / "artifact"),
            "--git-sha",
            "not-the-reviewed-commit",
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    assert wrong_commit.returncode != 0
    assert "commit does not match" in wrong_commit.stderr
    archive = next((tmp_path / "artifact").glob("*.zip"))
    with archive.open("ab") as stream:
        stream.write(b"tampered")
    tampered = subprocess.run(
        [sys.executable, str(_VERIFY_CANDIDATE), str(tmp_path / "artifact")],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    assert tampered.returncode != 0
    assert "archive record does not match" in tampered.stderr

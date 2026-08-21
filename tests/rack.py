#!/usr/bin/env python
"""Delegating rack wrapper for the KiCad monkey test suite."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from _suite_paths import (
    KICAD_PACKAGE_ROOT,
    TEST_CORPUS_ARCHIVE,
    TEST_CORPUS_ROOT,
    TEST_GENERATED_CORPUS_ROOT,
    TESTS_DIR,
    TESTS_REPO_ROOT,
)


def _prepend_pythonpath(env: dict[str, str], *paths: Path | None) -> None:
    existing = env.get("PYTHONPATH")
    entries = [str(path) for path in paths if path is not None]
    if existing:
        entries.append(existing)
    env["PYTHONPATH"] = os.pathsep.join(entries)


def _find_rack_executable(name: str) -> Path | None:
    # Keep the venv path intact; resolving it follows Linux python symlinks to /usr/bin.
    sibling = Path(sys.executable).with_name(name)
    if sibling.exists():
        return sibling
    located = shutil.which(name)
    if located:
        return Path(located)
    return None


def main() -> int:
    env = os.environ.copy()
    env["RACK_TESTS_DIR"] = str(TESTS_DIR)
    env["WN_RACK_TESTS_DIR"] = str(TESTS_DIR)
    env.setdefault("WN_TEST_SUITES_ROOT", str(TESTS_REPO_ROOT))
    if TEST_CORPUS_ARCHIVE.is_file() and not env.get("KM_CORPUS"):
        env["KM_CORPUS"] = str(TEST_CORPUS_ARCHIVE)
    env["KM_CORPUS_ROOT"] = str(TEST_CORPUS_ROOT)
    env["KM_CORPUS_OUTPUT_ROOT"] = str(TEST_GENERATED_CORPUS_ROOT)
    env["KM_CORPUS_RESOLVED_FROM"] = str(
        Path(env["KM_CORPUS"]).expanduser().resolve()
    )
    env.pop("WN_TEST_CORPUS", None)
    _prepend_pythonpath(env, KICAD_PACKAGE_ROOT / "src" / "py")

    rack_exe_name = "rack.exe" if os.name == "nt" else "rack"
    rack_exe = _find_rack_executable(rack_exe_name)
    if rack_exe is None:
        raise SystemExit(
            f"Rack executable not found near '{sys.executable}' or on PATH. "
            "Run 'uv sync --group dev' from this package to install wn-rack."
        )

    completed = subprocess.run([str(rack_exe), *sys.argv[1:]], env=env)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())

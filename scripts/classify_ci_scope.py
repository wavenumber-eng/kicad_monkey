#!/usr/bin/env python
"""Classify changed paths into the smallest safe CI scope."""

from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath


SCOPES = ("fast", "python", "full")
_FAST_ROOT_FILES = {
    "AGENTS.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "LICENSE",
    "README.md",
}
_PYTHON_ROOT_FILES = {
    "hatch_build.py",
    "pyproject.toml",
    "uv.lock",
}
_FULL_ROOT_FILES = {
    "Cargo.lock",
    "Cargo.toml",
    "package-lock.json",
    "package.json",
}


def classify(paths: list[str], *, event_name: str) -> str:
    """Return fast, python, or full, escalating unknown paths safely."""

    if event_name == "workflow_dispatch":
        return "full"
    normalized = [PurePosixPath(path.replace("\\", "/")) for path in paths if path]
    if not normalized:
        return "full"
    path_scope = max((_classify_path(path) for path in normalized), key=SCOPES.index)
    if event_name == "push" and path_scope == "python":
        # Main owns release candidates. Any merged implementation or dependency
        # change gets the complete gates and exact publishable artifacts once.
        return "full"
    return path_scope


def _classify_path(path: PurePosixPath) -> str:
    value = path.as_posix()
    if value in _FAST_ROOT_FILES or value.startswith((".github/", "docs/")):
        return "fast"
    if value in _FULL_ROOT_FILES or value.startswith(
        ("src/rs/", "tests/corpus/", "tests/contracts/", "packaging/")
    ):
        return "full"
    if value in _PYTHON_ROOT_FILES or value.endswith((".py", ".pyi")):
        return "python"
    if value.startswith("packages/kicad_cruncher/"):
        if "/src/rs/" in value or value.endswith(("Cargo.toml", "Cargo.lock")):
            return "full"
        return "python"
    return "full"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--paths-file", type=Path, required=True)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()
    scope = classify(
        args.paths_file.read_text(encoding="utf-8").splitlines(),
        event_name=args.event,
    )
    print(scope)
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as output:
            output.write(f"scope={scope}\n")


if __name__ == "__main__":
    main()

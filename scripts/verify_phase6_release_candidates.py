#!/usr/bin/env python
"""Verify the hash-bound Phase 6 release candidate directory."""

from __future__ import annotations

import argparse
from collections.abc import Collection
import hashlib
import json
from pathlib import Path
import re
from typing import TypedDict, cast


_ROLES = {
    "monkey_sdist": re.compile(r"^kicad_monkey-.+\.tar\.gz$"),
    "monkey_windows_x64_wheel": re.compile(
        r"^kicad_monkey-.+-py3-none-win_amd64\.whl$"
    ),
    "cruncher_sdist": re.compile(r"^kicad_cruncher-.+\.tar\.gz$"),
    "cruncher_universal_wheel": re.compile(r"^kicad_cruncher-.+-py3-none-any\.whl$"),
}
_MANIFEST_FILENAME = "phase6-release-candidate-a0.json"
_VERSION = re.compile(r"^\d{4}\.\d+\.\d+(?:\.\d+)?$")


class _ArtifactEntry(TypedDict):
    role: str
    filename: str
    bytes: object
    sha256: object


def _validate_artifact_entries(value: object) -> list[_ArtifactEntry]:
    """Require one uniquely named manifest entry for every candidate role."""

    if not isinstance(value, list) or len(value) != len(_ROLES):
        raise SystemExit("Phase 6 candidate must contain exactly four artifact entries")
    if not all(isinstance(entry, dict) for entry in value):
        raise SystemExit("Phase 6 candidate artifact entries must be objects")
    entries = cast(list[_ArtifactEntry], value)
    roles = [entry.get("role") for entry in entries]
    if len(set(roles)) != len(roles) or set(roles) != set(_ROLES):
        raise SystemExit("Phase 6 candidate artifact roles must be unique and complete")
    filenames = [entry.get("filename") for entry in entries]
    if not all(isinstance(filename, str) for filename in filenames) or len(
        set(filenames)
    ) != len(filenames):
        raise SystemExit("Phase 6 candidate artifact filenames must be unique strings")
    return entries


def _validate_candidate_member_names(
    member_names: Collection[str],
    entries: list[_ArtifactEntry],
) -> None:
    """Reject every directory member not bound by the candidate manifest."""

    expected = {_MANIFEST_FILENAME, *(entry["filename"] for entry in entries)}
    if len(member_names) != len(expected) or set(member_names) != expected:
        raise SystemExit(
            "Phase 6 candidate directory must contain only its manifest and four artifacts"
        )


def _validate_candidate_directory(root: Path, entries: list[_ArtifactEntry]) -> None:
    members = list(root.iterdir())
    _validate_candidate_member_names([member.name for member in members], entries)
    if any(member.is_symlink() or not member.is_file() for member in members):
        raise SystemExit("Phase 6 candidate directory members must be regular files")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify(
    directory: Path,
    *,
    git_sha: str | None = None,
    run_id: str | None = None,
    monkey_version: str | None = None,
    cruncher_version: str | None = None,
) -> dict[str, Path]:
    root = directory.resolve()
    manifest_path = root / _MANIFEST_FILENAME
    payload = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if payload.get("schema") != "kicad_monkey.phase6_release_candidate.a0":
        raise SystemExit("unexpected Phase 6 candidate manifest schema")
    if payload.get("platform") != "windows-x64":
        raise SystemExit("Phase 6 candidate is not the Windows x64 artifact set")
    if git_sha is not None and payload.get("git_sha") != git_sha:
        raise SystemExit("Phase 6 candidate commit does not match this workflow")
    source = payload.get("source")
    if (
        not isinstance(source, dict)
        or source.get("workflow") != "CI"
        or not isinstance(source.get("run_id"), str)
        or not source["run_id"].isdigit()
    ):
        raise SystemExit("Phase 6 candidate source workflow/run identity is invalid")
    if run_id is not None and source.get("run_id") != run_id:
        raise SystemExit("Phase 6 candidate run does not match")
    versions = payload.get("versions")
    if not isinstance(versions, dict):
        raise SystemExit("Phase 6 candidate versions must be an object")
    if any(
        not isinstance(versions.get(package), str)
        or _VERSION.fullmatch(versions[package]) is None
        for package in ("monkey", "cruncher")
    ):
        raise SystemExit("Phase 6 candidate package versions are invalid")
    if monkey_version is not None and versions.get("monkey") != monkey_version:
        raise SystemExit("Phase 6 Monkey version does not match")
    if cruncher_version is not None and versions.get("cruncher") != cruncher_version:
        raise SystemExit("Phase 6 Cruncher version does not match")
    entries = _validate_artifact_entries(payload.get("artifacts"))
    _validate_candidate_directory(root, entries)
    resolved: dict[str, Path] = {}
    for entry in entries:
        role = entry["role"]
        filename = entry.get("filename")
        if not isinstance(filename, str) or _ROLES[role].fullmatch(filename) is None:
            raise SystemExit(f"invalid {role} filename: {filename!r}")
        package = "monkey" if role.startswith("monkey_") else "cruncher"
        if not filename.startswith(f"kicad_{package}-{versions[package]}"):
            raise SystemExit(f"{role} filename does not match its package version")
        path = (root / filename).resolve()
        try:
            path.relative_to(root)
        except ValueError as error:
            raise SystemExit(
                f"candidate path escaped its directory: {filename}"
            ) from error
        if not path.is_file():
            raise SystemExit(f"candidate file is missing: {filename}")
        if path.stat().st_size != entry.get("bytes") or _sha256(path) != entry.get(
            "sha256"
        ):
            raise SystemExit(f"candidate hash or byte count changed: {filename}")
        resolved[role] = path
    return resolved


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--git-sha")
    parser.add_argument("--run-id")
    parser.add_argument("--monkey-version")
    parser.add_argument("--cruncher-version")
    args = parser.parse_args()
    verify(
        args.directory,
        git_sha=args.git_sha,
        run_id=args.run_id,
        monkey_version=args.monkey_version,
        cruncher_version=args.cruncher_version,
    )


if __name__ == "__main__":
    main()

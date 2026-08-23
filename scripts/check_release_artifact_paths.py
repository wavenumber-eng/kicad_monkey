#!/usr/bin/env python3
"""Reject local filesystem paths embedded in release artifacts."""

from __future__ import annotations

import argparse
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from pathlib import Path
import re
import tarfile
import zipfile


@dataclass(frozen=True)
class ArtifactEntry:
    artifact: Path
    member: str
    payload: bytes


def _archive_entries(path: Path) -> Iterator[ArtifactEntry]:
    if path.suffix.lower() in {".whl", ".zip"}:
        with zipfile.ZipFile(path) as archive:
            for item in archive.infolist():
                if not item.is_dir():
                    yield ArtifactEntry(path, item.filename, archive.read(item))
    elif path.name.lower().endswith((".tar.gz", ".tgz")):
        with tarfile.open(path, "r:gz") as archive:
            for item in archive.getmembers():
                if item.isfile():
                    stream = archive.extractfile(item)
                    yield ArtifactEntry(path, item.name, stream.read() if stream else b"")


def iter_entries(root: Path) -> Iterator[ArtifactEntry]:
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if path.suffix.lower() in {".whl", ".zip"} or path.name.lower().endswith(
            (".tar.gz", ".tgz")
        ):
            yield from _archive_entries(path)
        else:
            yield ArtifactEntry(path, path.relative_to(root).as_posix(), path.read_bytes())


def _path_spellings(path: str) -> set[bytes]:
    normalized = path.strip().rstrip("/\\")
    if not normalized:
        return set()
    spellings = {
        normalized,
        normalized.replace("\\", "/"),
        normalized.replace("/", "\\"),
    }
    return {value.casefold().encode("utf-8") for value in spellings if value}


def _search_views(payload: bytes) -> Iterator[bytes]:
    """Expose byte strings and both alignments of Windows wide strings."""
    yield payload.lower()
    for encoding in ("utf-16-le", "utf-16-be"):
        for offset in (0, 1):
            decoded = payload[offset:].decode(encoding, errors="ignore").casefold()
            yield decoded.encode("utf-8", errors="ignore")


def find_path_leaks(
    entries: Iterable[ArtifactEntry], forbidden_paths: Iterable[str]
) -> list[tuple[str, str, str]]:
    explicit = {
        spelling
        for path in forbidden_paths
        for spelling in _path_spellings(path)
    }
    wsl_mount = re.compile(rb"/" + rb"mnt/[a-z]/", re.IGNORECASE)
    file_url = rb"file:" + rb"///"
    wsl_unc = rb"\\\\" + rb"wsl"
    findings: list[tuple[str, str, str]] = []
    for entry in entries:
        name = entry.member.casefold().encode("utf-8")
        targets = (name, *_search_views(entry.payload))
        labels: list[str] = []
        if any(wsl_mount.search(target) for target in targets):
            labels.append("WSL mount path")
        if any(file_url in target for target in targets):
            labels.append("local file URL")
        if any(wsl_unc in target for target in targets):
            labels.append("WSL UNC path")
        if any(spelling in target for spelling in explicit for target in targets):
            labels.append("explicit local root")
        findings.extend(
            (entry.artifact.name, entry.member, label) for label in sorted(set(labels))
        )
    return findings


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact_directory", type=Path)
    parser.add_argument(
        "--forbid",
        action="append",
        default=[],
        help="Local root that must not appear (may be repeated)",
    )
    args = parser.parse_args()
    if not args.artifact_directory.is_dir():
        raise SystemExit(f"Artifact directory does not exist: {args.artifact_directory}")
    entries = list(iter_entries(args.artifact_directory))
    if not entries:
        raise SystemExit(f"No artifact content found in {args.artifact_directory}")
    findings = find_path_leaks(entries, args.forbid)
    if findings:
        details = "\n".join(
            f"- {artifact}: {member} ({label})"
            for artifact, member, label in findings
        )
        raise SystemExit(f"Release artifacts contain local paths:\n{details}")
    print(f"Checked {len(entries)} artifact entries: no local paths found")


if __name__ == "__main__":
    main()

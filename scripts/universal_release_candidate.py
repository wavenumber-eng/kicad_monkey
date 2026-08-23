#!/usr/bin/env python
"""Create or verify the hash-bound universal Monkey wheel candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


MANIFEST = "universal-release-candidate-a0.json"
SCHEMA = "kicad_monkey.universal_release_candidate.a0"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write(
    directory: Path,
    git_sha: str,
    *,
    run_id: str,
    workflow: str,
    version: str,
) -> Path:
    wheels = list(directory.glob("kicad_monkey-*-py3-none-any.whl"))
    if len(wheels) != 1:
        raise SystemExit("universal candidate requires exactly one Monkey wheel")
    wheel = wheels[0]
    manifest = directory / MANIFEST
    manifest.write_text(
        json.dumps(
            {
                "schema": SCHEMA,
                "git_sha": git_sha,
                "source": {"workflow": workflow, "run_id": run_id},
                "version": version,
                "artifact": {
                    "filename": wheel.name,
                    "bytes": wheel.stat().st_size,
                    "sha256": _sha256(wheel),
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return manifest


def verify(
    directory: Path,
    git_sha: str | None = None,
    *,
    run_id: str | None = None,
    expected_version: str | None = None,
) -> Path:
    root = directory.resolve()
    members = list(root.iterdir())
    manifest = root / MANIFEST
    payload = json.loads(manifest.read_text(encoding="utf-8"))
    if payload.get("schema") != SCHEMA:
        raise SystemExit("unexpected universal candidate schema")
    if git_sha is not None and payload.get("git_sha") != git_sha:
        raise SystemExit("universal candidate commit does not match")
    source = payload.get("source")
    if (
        not isinstance(source, dict)
        or source.get("workflow") != "CI"
        or not isinstance(source.get("run_id"), str)
        or not source["run_id"].isdigit()
    ):
        raise SystemExit("universal candidate source workflow/run identity is invalid")
    if run_id is not None and source.get("run_id") != run_id:
        raise SystemExit("universal candidate run does not match")
    if expected_version is not None and payload.get("version") != expected_version:
        raise SystemExit("universal candidate version does not match")
    version = payload.get("version")
    if not isinstance(version, str):
        raise SystemExit("universal candidate version is invalid")
    artifact = payload.get("artifact")
    if not isinstance(artifact, dict):
        raise SystemExit("universal candidate artifact must be an object")
    filename = artifact.get("filename")
    if not isinstance(filename, str):
        raise SystemExit("universal candidate filename must be a string")
    wheel = (root / filename).resolve()
    if (
        len(members) != 2
        or set(members) != {manifest, wheel}
        or wheel.is_symlink()
        or not wheel.is_file()
        or not wheel.name.startswith(f"kicad_monkey-{version}-")
        or not wheel.name.endswith("-py3-none-any.whl")
        or wheel.stat().st_size != artifact.get("bytes")
        or _sha256(wheel) != artifact.get("sha256")
    ):
        raise SystemExit("universal candidate files do not match the manifest")
    return wheel


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    write_parser = subparsers.add_parser("write")
    write_parser.add_argument("directory", type=Path)
    write_parser.add_argument("--git-sha", required=True)
    write_parser.add_argument("--run-id", required=True)
    write_parser.add_argument("--workflow", required=True)
    write_parser.add_argument("--version", required=True)
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("directory", type=Path)
    verify_parser.add_argument("--git-sha")
    verify_parser.add_argument("--run-id")
    verify_parser.add_argument("--expected-version")
    args = parser.parse_args()
    if args.command == "write":
        write(
            args.directory,
            args.git_sha,
            run_id=args.run_id,
            workflow=args.workflow,
            version=args.version,
        )
    else:
        verify(
            args.directory,
            args.git_sha,
            run_id=args.run_id,
            expected_version=args.expected_version,
        )


if __name__ == "__main__":
    main()

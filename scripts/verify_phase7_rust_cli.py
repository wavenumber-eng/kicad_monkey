#!/usr/bin/env python
"""Verify a hash-bound Phase 7 Rust CLI release candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import zipfile
from pathlib import Path
from typing import cast

_SCHEMA = "kicad_cruncher.rust_cli_release.a1"
_VERSION = re.compile(r"^\d{4}\.\d+\.\d+(?:\.\d+)?$")
_EXECUTABLES = {"kicad-cruncher.exe", "kcr.exe", "geometer.exe"}
_GEOMETER_LICENSES = {
    "CLIPPER2_LICENSE.txt",
    "OCCT_LGPL_EXCEPTION.txt",
    "OCCT_LICENSE_LGPL_21.txt",
    "RAPIDJSON_LICENSE.txt",
    "THIRD_PARTY_NOTICES.md",
    "WN_GEOMETER_LICENSE.txt",
}
_ZIP_MEMBERS = {
    *_EXECUTABLES,
    "geometer.build-attestation.json",
    "README.md",
    "LICENSE",
    *(f"licenses/geometer/{name}" for name in _GEOMETER_LICENSES),
}


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify(
    directory: Path,
    *,
    git_sha: str | None = None,
    run_id: str | None = None,
    expected_version: str | None = None,
) -> tuple[Path, Path]:
    root = directory.resolve()
    members = list(root.iterdir())
    if any(member.is_symlink() or not member.is_file() for member in members):
        raise SystemExit("Phase 7 candidate members must be regular files")
    manifests = list(root.glob("kicad-cruncher-*-windows-x64.json"))
    archives = list(root.glob("kicad-cruncher-*-windows-x64.zip"))
    if len(members) != 2 or len(manifests) != 1 or len(archives) != 1:
        raise SystemExit("Phase 7 candidate must contain one manifest and one archive")

    manifest_path = manifests[0]
    archive = archives[0]
    payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    version = payload.get("version")
    if payload.get("schema") != _SCHEMA:
        raise SystemExit("unexpected Phase 7 candidate schema")
    if payload.get("platform") != "windows-x64":
        raise SystemExit("Phase 7 candidate is not for Windows x64")
    if not isinstance(version, str) or _VERSION.fullmatch(version) is None:
        raise SystemExit("invalid Phase 7 candidate version")
    if expected_version is not None and version != expected_version:
        raise SystemExit(
            f"Phase 7 candidate version {version} does not match {expected_version}"
        )
    if git_sha is not None and payload.get("git_sha") != git_sha:
        raise SystemExit("Phase 7 candidate commit does not match this workflow")
    source = payload.get("source")
    if (
        not isinstance(source, dict)
        or not isinstance(source.get("workflow"), str)
        or not source["workflow"]
        or not isinstance(source.get("run_id"), str)
        or not source["run_id"].isdigit()
    ):
        raise SystemExit("Phase 7 candidate source workflow/run identity is invalid")
    if run_id is not None and source != {"workflow": "CI", "run_id": run_id}:
        raise SystemExit("Phase 7 candidate run does not match")

    expected_stem = f"kicad-cruncher-{version}-windows-x64"
    if (
        archive.name != f"{expected_stem}.zip"
        or manifest_path.name != f"{expected_stem}.json"
    ):
        raise SystemExit("Phase 7 candidate filenames do not match its version")
    archive_record = payload.get("archive")
    if not isinstance(archive_record, dict):
        raise SystemExit("Phase 7 archive record must be an object")
    if archive_record != {
        "filename": archive.name,
        "bytes": archive.stat().st_size,
        "sha256": _sha256(archive),
    }:
        raise SystemExit("Phase 7 archive record does not match its bytes")

    executable_records = payload.get("executables")
    if not isinstance(executable_records, list) or len(executable_records) != 3:
        raise SystemExit("Phase 7 manifest must contain exactly three executables")
    records_by_name: dict[str, dict[str, object]] = {}
    for record_value in executable_records:
        if not isinstance(record_value, dict):
            continue
        record = cast(dict[str, object], record_value)
        name = record.get("filename")
        if not isinstance(name, str) or name in records_by_name:
            continue
        records_by_name[name] = record
    if set(records_by_name) != _EXECUTABLES:
        raise SystemExit("Phase 7 executable records are incomplete or duplicated")

    with zipfile.ZipFile(archive) as bundle:
        if set(bundle.namelist()) != _ZIP_MEMBERS or len(bundle.infolist()) != len(
            _ZIP_MEMBERS
        ):
            raise SystemExit("Phase 7 archive members are not exact")
        for name, record in records_by_name.items():
            executable = bundle.read(name)
            if record != {
                "filename": name,
                "bytes": len(executable),
                "sha256": _sha256_bytes(executable),
            }:
                raise SystemExit(f"Phase 7 executable record does not match {name}")
        geometer = payload.get("geometer")
        if not isinstance(geometer, dict):
            raise SystemExit("Phase 7 manifest omits Geometer provenance")
        attestation_bytes = bundle.read("geometer.build-attestation.json")
        attestation_record = geometer.get("attestation")
        if attestation_record != {
            "filename": "geometer.build-attestation.json",
            "bytes": len(attestation_bytes),
            "sha256": _sha256_bytes(attestation_bytes),
        }:
            raise SystemExit("Phase 7 Geometer attestation record does not match")
        attestation = json.loads(attestation_bytes)
        if not isinstance(attestation, dict):
            raise SystemExit("Phase 7 Geometer attestation has an invalid shape")
        build = attestation.get("build", {})
        artifact = attestation.get("artifact", {})
        if not isinstance(build, dict) or not isinstance(artifact, dict):
            raise SystemExit("Phase 7 Geometer attestation has an invalid shape")
        source = build.get("source", {})
        if not isinstance(source, dict):
            raise SystemExit("Phase 7 Geometer attestation has invalid provenance")
        if (
            geometer
            != {
                "release": "2026.9.13",
                "c_abi_generation": 20260913,
                "source_revision": "703e60029a248801947150f913f77a1cecce2662",
                "attestation": attestation_record,
            }
            or build.get("geometer_version") != "2026.9.13"
            or build.get("c_abi_version") != 20260913
            or source.get("revision") != "703e60029a248801947150f913f77a1cecce2662"
            or source.get("tree_state") != "clean"
            or artifact.get("name") != "geometer.exe"
            or artifact.get("sha256") != _sha256_bytes(bundle.read("geometer.exe"))
        ):
            raise SystemExit("Phase 7 Geometer provenance is incompatible")
    return archive, manifest_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--git-sha", default=None)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-version", default=None)
    args = parser.parse_args()
    verify(
        args.directory,
        git_sha=args.git_sha,
        run_id=args.run_id,
        expected_version=args.expected_version,
    )


if __name__ == "__main__":
    main()

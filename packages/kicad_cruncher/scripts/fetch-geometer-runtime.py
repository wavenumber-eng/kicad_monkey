#!/usr/bin/env python3
"""Fetch and verify the exact Geometer runtime paired with the Rust client."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import stat
import tempfile
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath

_PACKAGE_ROOT = Path(__file__).resolve().parents[1]
_SOURCE = _PACKAGE_ROOT / "runtimes" / "geometer-2026.9.13.json"
_REPOSITORY = "wavenumber-eng/geometer"


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _host_platform() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    architecture = {"amd64": "x64", "x86_64": "x64", "arm64": "arm64", "aarch64": "arm64"}.get(
        machine
    )
    operating_system = {"windows": "windows", "linux": "linux", "darwin": "macos"}.get(system)
    if operating_system is None or architecture is None:
        raise SystemExit(f"unsupported Geometer runtime host: {system}-{machine}")
    return f"{operating_system}-{architecture}"


def _safe_member(info: zipfile.ZipInfo) -> PurePosixPath:
    path = PurePosixPath(info.filename)
    if path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        raise SystemExit(f"unsafe Geometer archive member: {info.filename}")
    mode = info.external_attr >> 16
    if stat.S_ISLNK(mode):
        raise SystemExit(f"Geometer archive contains a symbolic link: {info.filename}")
    return path


def _read_archive(
    archive: Path, executable: str
) -> tuple[bytes, bytes, dict[str, object], dict[str, bytes]]:
    with zipfile.ZipFile(archive) as bundle:
        entries: dict[str, bytes] = {}
        for info in bundle.infolist():
            path = _safe_member(info)
            if info.is_dir():
                continue
            name = path.as_posix()
            if name in entries:
                raise SystemExit(f"duplicate Geometer archive member: {name}")
            entries[name] = bundle.read(info)
    required = {executable, "geometer.build-attestation.json"}
    if not required <= entries.keys():
        raise SystemExit("Geometer archive omits its runtime or build attestation")
    licenses = {name: payload for name, payload in entries.items() if name.startswith("licenses/")}
    if not licenses:
        raise SystemExit("Geometer archive omits third-party licenses")
    attestation_bytes = entries["geometer.build-attestation.json"]
    return entries[executable], attestation_bytes, json.loads(attestation_bytes), licenses


def _validate_attestation(
    manifest: dict[str, object],
    attestation: dict[str, object],
    executable_name: str,
    executable: bytes,
) -> None:
    build = attestation.get("build")
    artifact = attestation.get("artifact")
    if not isinstance(build, dict) or not isinstance(artifact, dict):
        raise SystemExit("Geometer build attestation has an invalid shape")
    source = build.get("source")
    if not isinstance(source, dict):
        raise SystemExit("Geometer build attestation omits source provenance")
    expected = (
        build.get("geometer_version") == manifest["release"]
        and build.get("c_abi_version") == manifest["c_abi_generation"]
        and source.get("revision") == manifest["source_revision"]
        and source.get("tree_state") == "clean"
        and artifact.get("name") == executable_name
        and artifact.get("sha256") == _sha256_bytes(executable)
    )
    if not expected:
        raise SystemExit("Geometer build attestation does not match the pinned runtime")


def fetch_runtime(destination: Path, platform_name: str, archive: Path | None = None) -> Path:
    manifest = json.loads(_SOURCE.read_text(encoding="utf-8"))
    assets = manifest["assets"]
    if platform_name not in assets:
        raise SystemExit(f"unsupported Geometer release platform: {platform_name}")
    asset = assets[platform_name]
    with tempfile.TemporaryDirectory(prefix="kicad_cruncher_geometer_") as temporary:
        downloaded = Path(temporary) / asset["archive"]
        if archive is None:
            url = f"https://github.com/{_REPOSITORY}/releases/download/{manifest['tag']}/{asset['archive']}"
            request = urllib.request.Request(url, headers={"User-Agent": "kicad-cruncher-build"})
            with (
                urllib.request.urlopen(request, timeout=120) as response,
                downloaded.open("wb") as output,
            ):
                shutil.copyfileobj(response, output)
        else:
            shutil.copyfile(archive, downloaded)
        if _sha256_file(downloaded) != asset["sha256"]:
            raise SystemExit("Geometer release archive SHA-256 does not match the pin")
        executable, attestation_bytes, attestation, licenses = _read_archive(
            downloaded, asset["executable"]
        )
        _validate_attestation(manifest, attestation, asset["executable"], executable)
        destination.mkdir(parents=True, exist_ok=True)
        executable_path = destination / asset["executable"]
        executable_path.write_bytes(executable)
        if platform_name != "windows-x64":
            executable_path.chmod(0o755)
        (destination / "geometer.build-attestation.json").write_bytes(attestation_bytes)
        license_dir = destination / "licenses" / "geometer"
        license_dir.mkdir(parents=True, exist_ok=True)
        for name, payload in licenses.items():
            (license_dir / PurePosixPath(name).name).write_bytes(payload)
    return executable_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", default=None)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--archive", type=Path, default=None)
    args = parser.parse_args()
    executable = fetch_runtime(
        args.destination.resolve(), args.platform or _host_platform(), args.archive
    )
    os.sys.stdout.write(str(executable) + "\n")


if __name__ == "__main__":
    main()

"""Build, install, and exercise the pure-Rust Cruncher design CLI."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path, PurePosixPath


def _run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        check=False,
        timeout=600,
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"Command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _native_runtime_env(bin_dir: Path) -> dict[str, str]:
    env = os.environ.copy()
    for name in ("PYTHONHOME", "PYTHONPATH", "VIRTUAL_ENV", "UV_PROJECT_ENVIRONMENT"):
        env.pop(name, None)
    env["PATH"] = str(bin_dir)
    return env


def _assert_no_workspace_path(executable: Path, workspace: Path) -> None:
    payload = executable.read_bytes().lower()
    spellings = {
        str(workspace.resolve()),
        str(workspace.resolve()).replace("\\", "/"),
    }
    for spelling in spellings:
        for encoded in (spelling.encode(), spelling.encode("utf-16-le")):
            if encoded.lower() in payload:
                raise AssertionError(
                    f"installed executable embeds the workspace path: {executable}"
                )


def _assert_windows_x64_pe(executable: Path) -> None:
    payload = executable.read_bytes()
    if len(payload) < 64 or payload[:2] != b"MZ":
        raise AssertionError(f"installed artifact is not a Windows PE: {executable}")
    pe_offset = int.from_bytes(payload[60:64], "little")
    header = payload[pe_offset : pe_offset + 6]
    if header[:4] != b"PE\0\0" or header[4:6] != b"\x64\x86":
        raise AssertionError(f"installed artifact is not Windows x64: {executable}")


def _copy_fixture(workspace: Path, destination: Path) -> Path:
    configured = os.environ.get("KM_CORPUS", "").strip()
    archive_path = (
        Path(configured) if configured else workspace / "tests" / "corpus" / "kicad.zip"
    ).expanduser().resolve()
    if archive_path.suffix.lower() != ".zip" or not archive_path.is_file():
        raise AssertionError(f"KM_CORPUS must name the reviewed kicad.zip: {archive_path}")
    prefix = PurePosixPath("kicad/projects/led_component/input")
    copied = destination / "source"
    with zipfile.ZipFile(archive_path) as archive:
        selected = [
            info
            for info in archive.infolist()
            if PurePosixPath(info.filename).is_relative_to(prefix)
        ]
        if not selected:
            raise AssertionError(f"reviewed corpus omits fixture {prefix}")
        for info in selected:
            relative = PurePosixPath(info.filename).relative_to(prefix)
            if relative == PurePosixPath(".") or info.is_dir():
                continue
            if any(part in ("", ".", "..") for part in relative.parts):
                raise AssertionError(f"unsafe reviewed corpus member: {info.filename}")
            target = copied.joinpath(*relative.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(info) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
    project = copied / "led_component.kicad_pro"
    if not project.is_file():
        raise AssertionError(f"reviewed corpus fixture omits {project.name}")
    return project


def _assert_bundle(output: Path) -> None:
    manifest_path = output / "design_review_manifest.json"
    if not manifest_path.is_file():
        raise AssertionError(f"expected review manifest at {manifest_path}")
    manifest = json.loads(manifest_path.read_text("utf-8"))
    if manifest.get("schema") != "kicad_cruncher.design_review_manifest.a0":
        raise AssertionError("installed Rust CLI emitted the wrong manifest schema")
    if manifest.get("design_facts", {}).get("backend") != "kicad-monkey-native":
        raise AssertionError("installed Rust CLI did not report its native facts backend")
    required = [
        manifest["design_json"],
        manifest["compiled_schematic_graph"]["file"],
        manifest["netlist_json"],
        manifest["netlist_kicad_sexpr"],
        manifest["readme"],
        *(record["file"] for record in manifest["schematic_svgs"]),
        *(record["file"] for record in manifest["pcb_svgs"]),
    ]
    missing = [relative for relative in required if not (output / relative).is_file()]
    if missing:
        raise AssertionError(f"installed Rust CLI omitted bundle artifacts: {missing}")


def _write_release_artifact(
    artifact_dir: Path,
    *,
    bin_dir: Path,
    package_root: Path,
    workspace: Path,
    version: str,
    git_sha: str,
) -> tuple[Path, Path]:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    stem = f"kicad-cruncher-{version}-windows-x64"
    archive = artifact_dir / f"{stem}.zip"
    manifest_path = artifact_dir / f"{stem}.json"
    executable_names = ("kicad-cruncher.exe", "kcr.exe")
    readme = package_root / "docs" / "contracts" / "rust_cli_windows_x64.md"
    license_path = workspace / "LICENSE"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for name in executable_names:
            bundle.write(bin_dir / name, name)
        bundle.write(readme, "README.md")
        bundle.write(license_path, "LICENSE")
    manifest = {
        "schema": "kicad_cruncher.rust_cli_release.a0",
        "version": version,
        "platform": "windows-x64",
        "git_sha": git_sha,
        "archive": {
            "filename": archive.name,
            "bytes": archive.stat().st_size,
            "sha256": _sha256(archive),
        },
        "executables": [
            {
                "filename": name,
                "bytes": (bin_dir / name).stat().st_size,
                "sha256": _sha256(bin_dir / name),
            }
            for name in executable_names
        ],
    }
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return archive, manifest_path


def run_install_test(
    workspace: Path,
    *,
    artifact_dir: Path | None = None,
    git_sha: str | None = None,
) -> None:
    if os.name != "nt":
        raise AssertionError("the promoted Rust CLI artifact currently targets Windows x64")
    workspace = workspace.resolve()
    package_root = workspace / "packages" / "kicad_cruncher"
    crate = package_root / "src" / "rs" / "kicad-cruncher-cli"
    metadata = json.loads(
        _run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=workspace,
        ).stdout
    )
    version = next(
        package["version"]
        for package in metadata["packages"]
        if package["name"] == "kicad-cruncher-cli"
    )
    with tempfile.TemporaryDirectory(prefix="kicad_cruncher_rust_install_") as temp:
        temp_dir = Path(temp)
        install_root = temp_dir / "install"
        _run(
            [
                "cargo",
                "install",
                "--locked",
                "--path",
                str(crate),
                "--root",
                str(install_root),
                "--force",
                "--bins",
            ],
            cwd=workspace,
        )
        bin_dir = install_root / "bin"
        executables = [bin_dir / "kicad-cruncher.exe", bin_dir / "kcr.exe"]
        for executable in executables:
            if not executable.is_file():
                raise AssertionError(f"cargo install omitted {executable.name}")
            _assert_windows_x64_pe(executable)
            _assert_no_workspace_path(executable, workspace)

        runtime = temp_dir / "runtime"
        runtime.mkdir()
        env = _native_runtime_env(bin_dir)
        for executable in executables:
            version_result = _run([str(executable), "--version"], cwd=runtime, env=env)
            if version not in version_result.stdout:
                raise AssertionError(f"unexpected version output from {executable.name}")

        project = _copy_fixture(workspace, runtime)
        output = runtime / "review"
        _run(
            [str(executables[1]), "design-review", str(project), "--output", str(output)],
            cwd=runtime,
            env=env,
        )
        _assert_bundle(output)

        if artifact_dir is not None:
            resolved_sha = git_sha or _run(
                ["git", "rev-parse", "HEAD"], cwd=workspace
            ).stdout.strip()
            _write_release_artifact(
                artifact_dir.resolve(),
                bin_dir=bin_dir,
                package_root=package_root,
                workspace=workspace,
                version=version,
                git_sha=resolved_sha,
            )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=None)
    parser.add_argument("--artifact-dir", type=Path, default=None)
    parser.add_argument("--git-sha", default=None)
    args = parser.parse_args()
    package_root = Path(__file__).resolve().parents[2]
    workspace = args.workspace or package_root.parents[1]
    run_install_test(
        workspace,
        artifact_dir=args.artifact_dir,
        git_sha=args.git_sha,
    )
    sys.stdout.write("Installed pure-Rust CLI smoke test passed.\n")


if __name__ == "__main__":
    main()

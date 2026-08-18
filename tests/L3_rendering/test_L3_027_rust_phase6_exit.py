"""Mandatory Windows x64 Phase 6 release-candidate exit gate."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

from scripts.verify_phase6_release_candidates import (
    _validate_artifact_entries,
    _validate_candidate_member_names,
)


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
SCOPE_PATH = PACKAGE_ROOT / "tests" / "parity" / "scope.toml"
ARCHIVE_METADATA = PACKAGE_ROOT / "tests" / "corpus" / "kicad.archive.toml"
EXPECTED_ROLES = {
    "monkey_sdist": ("kicad_monkey-", ".tar.gz"),
    "monkey_windows_x64_wheel": ("kicad_monkey-", "-py3-none-win_amd64.whl"),
    "cruncher_sdist": ("kicad_cruncher-", ".tar.gz"),
    "cruncher_universal_wheel": ("kicad_cruncher-", "-py3-none-any.whl"),
}
PREREQUISITE_SURFACES = (
    "cruncher.native_operation_transport_package",
    "cruncher.native_svg",
    "cruncher.no_fallback_physical_provider",
    "cruncher.native_design_facts",
    "cruncher.native_full_cli",
)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _required_file_env(name: str) -> Path:
    raw = os.environ.get(name, "").strip()
    assert raw, f"{name} must name a Phase 6 release-candidate file"
    path = Path(raw).expanduser().resolve()
    assert path.is_file(), f"{name} is not a file: {path}"
    return path


def _run(command: list[str], *, env: dict[str, str], timeout: int = 1800) -> None:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )


def test_windows_x64_phase6_release_candidate_is_exact_and_native() -> None:
    assert os.name == "nt", "P6_050 is a mandatory Windows x64 release gate"
    corpus = _required_file_env("KM_CORPUS")
    assert corpus.suffix.lower() == ".zip", "KM_CORPUS must be the reviewed ZIP"
    archive = tomllib.loads(ARCHIVE_METADATA.read_text(encoding="utf-8"))
    assert corpus.stat().st_size == archive["size"]
    assert _sha256(corpus) == archive["sha256"]

    manifest_path = _required_file_env("KM_PHASE6_ARTIFACT_MANIFEST")
    artifact_root = manifest_path.parent.resolve()
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    assert manifest["schema"] == "kicad_monkey.phase6_release_candidate.a0"
    assert manifest["platform"] == "windows-x64"
    assert re.fullmatch(r"[0-9a-f]{40}", manifest["git_sha"])
    local_git_sha = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=True,
    ).stdout.strip()
    assert manifest["git_sha"] == local_git_sha
    github_sha = os.environ.get("GITHUB_SHA")
    if github_sha is not None:
        assert manifest["git_sha"] == github_sha
    assert isinstance(manifest["source_date_epoch"], int)
    assert manifest["source_date_epoch"] > 0
    assert manifest["corpus"] == {
        "filename": corpus.name,
        "bytes": archive["size"],
        "sha256": archive["sha256"],
    }

    entries = _validate_artifact_entries(manifest["artifacts"])
    assert len(entries) == len(EXPECTED_ROLES)
    assert {entry["role"] for entry in entries} == set(EXPECTED_ROLES)
    assert len({entry["filename"] for entry in entries}) == len(entries)

    duplicate_role = [dict(entry) for entry in entries]
    duplicate_role[-1]["role"] = duplicate_role[0]["role"]
    try:
        _validate_artifact_entries(duplicate_role)
    except SystemExit:
        pass
    else:
        raise AssertionError("duplicate candidate artifact roles must fail closed")

    duplicate_filename = [dict(entry) for entry in entries]
    duplicate_filename[-1]["filename"] = duplicate_filename[0]["filename"]
    try:
        _validate_artifact_entries(duplicate_filename)
    except SystemExit:
        pass
    else:
        raise AssertionError("duplicate candidate artifact filenames must fail closed")

    exact_member_names = {
        "phase6-release-candidate-a0.json",
        *(entry["filename"] for entry in entries),
    }
    _validate_candidate_member_names(exact_member_names, entries)
    try:
        _validate_candidate_member_names(
            {*exact_member_names, "unmanifested-kicad-monkey.whl"},
            entries,
        )
    except SystemExit:
        pass
    else:
        raise AssertionError(
            "unmanifested candidate directory members must fail closed"
        )

    artifacts: dict[str, Path] = {}
    for entry in entries:
        role = entry["role"]
        prefix, suffix = EXPECTED_ROLES[role]
        filename = entry["filename"]
        assert filename.startswith(prefix) and filename.endswith(suffix), (
            role,
            filename,
        )
        path = (artifact_root / filename).resolve()
        path.relative_to(artifact_root)
        assert path.is_file(), path
        assert path.stat().st_size == entry["bytes"]
        assert _sha256(path) == entry["sha256"]
        artifacts[role] = path

    scope = tomllib.loads(SCOPE_PATH.read_text(encoding="utf-8"))
    surfaces = {surface["id"]: surface for surface in scope["surfaces"]}
    assert all(
        surfaces[surface_id]["status"] == "closed"
        for surface_id in PREREQUISITE_SURFACES
    )
    assert surfaces["cruncher.phase6_exit"]["status"] in {"review_ready", "closed"}

    env = dict(os.environ)
    for name in (
        "KICAD_MONKEY_NATIVE",
        "KICAD_CRUNCHER_NATIVE_DESIGN_FACTS",
        "KICAD_CRUNCHER_NATIVE_PHYSICAL",
        "PYTHONHOME",
        "PYTHONPATH",
    ):
        env.pop(name, None)
    _run(
        [
            sys.executable,
            "-m",
            "pytest",
            "tests/L0_foundation/test_L0_046_rust_l0_signoff.py::"
            "test_phase6_native_contract_and_cli_freeze_manifest_is_exact",
            "-q",
        ],
        env=env,
    )
    _run(
        [
            sys.executable,
            "tests/support_scripts/native_wheel_test.py",
            str(artifacts["monkey_windows_x64_wheel"]),
        ],
        env=env,
    )
    _run(
        [
            sys.executable,
            "tests/support_scripts/toolchain_install_test.py",
            "--monkey-wheel",
            str(artifacts["monkey_windows_x64_wheel"]),
            "--cruncher-wheel",
            str(artifacts["cruncher_universal_wheel"]),
            "--monkey-sdist",
            str(artifacts["monkey_sdist"]),
            "--cruncher-sdist",
            str(artifacts["cruncher_sdist"]),
        ],
        env=env,
    )

"""Shared provenance capture for advisory performance evidence."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import platform
import subprocess
from typing import Any


def sha256_file(path: Path) -> str:
    """Return the SHA-256 of one immutable input or executable."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _output(command: list[str], *, root: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"Command failed: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed.stdout


def collect_performance_provenance(
    *,
    package_root: Path,
    executables: dict[str, Path],
    feature_sets: dict[str, list[str]],
    archive: Path | None,
) -> dict[str, Any]:
    """Capture exact source, build, host, toolchain, and input identities."""
    git_sha = _output(["git", "rev-parse", "HEAD"], root=package_root).strip()
    base_sha = _output(["git", "rev-parse", "origin/main"], root=package_root).strip()
    diff = subprocess.run(
        ["git", "diff", f"{base_sha}..{git_sha}", "--binary"],
        cwd=package_root,
        capture_output=True,
        timeout=60,
        check=True,
    ).stdout
    patch_id_result = (
        subprocess.run(
            ["git", "patch-id", "--stable"],
            cwd=package_root,
            input=diff,
            capture_output=True,
            timeout=60,
            check=True,
        )
        .stdout.decode("ascii")
        .split()
    )
    return {
        "git_sha": git_sha,
        "base_sha": base_sha,
        "git_status_porcelain": _output(
            ["git", "status", "--porcelain", "--untracked-files=no"],
            root=package_root,
        ).splitlines(),
        "stable_patch_id": patch_id_result[0] if patch_id_result else None,
        "profile": "release",
        "feature_sets": feature_sets,
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "logical_cpus": os.cpu_count(),
        },
        "toolchain": {
            "rustc": _output(
                ["rustc", "--version", "--verbose"], root=package_root
            ).strip(),
            "cargo": _output(["cargo", "--version"], root=package_root).strip(),
        },
        "locks": {
            "cargo_sha256": sha256_file(package_root / "Cargo.lock"),
            "uv_sha256": sha256_file(package_root / "uv.lock"),
        },
        "archive": (
            {"path": str(archive), "sha256": sha256_file(archive)}
            if archive is not None and archive.is_file()
            else None
        ),
        "executables": {
            name: {"path": str(path), "sha256": sha256_file(path)}
            for name, path in executables.items()
        },
    }

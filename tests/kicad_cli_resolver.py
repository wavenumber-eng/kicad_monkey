"""Shared KiCad CLI resolver for corpus-backed oracle tests."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Literal


def _corpus_roots() -> list[Path]:
    roots: list[Path] = []
    env_corpus = os.environ.get("WN_TEST_CORPUS")
    if env_corpus:
        roots.append(Path(env_corpus))
    roots.append(Path(__file__).resolve().parent / "corpus")

    out: list[Path] = []
    seen: set[Path] = set()
    for root in roots:
        key = root.resolve() if root.exists() else root
        if key in seen:
            continue
        seen.add(key)
        out.append(root)
    return out


def _manifest_short_hashes() -> list[str]:
    """Return staged CLI hashes in manifest order.

    The manifest is authoritative because the corpus can contain stale
    experimental builds that should not be used just because their mtime is
    newer.
    """
    repo_root = Path(__file__).resolve().parents[1]
    manifest = repo_root / "tools" / "kicad-cli" / "MANIFEST.toml"
    if not manifest.exists():
        return []

    hashes: list[str] = []
    for match in re.finditer(
        r'^\s*short_hash\s*=\s*"([^"]+)"\s*$',
        manifest.read_text(encoding="utf-8"),
        flags=re.MULTILINE,
    ):
        hashes.append(match.group(1))
    return hashes


KiCadCliCapability = Literal["any", "pcb_svg", "pcb_bbox"]


def _build_tree_dir(candidate: Path) -> Path | None:
    """Return the KiCad build directory for a build-tree ``kicad-cli``."""
    build_dir = candidate.parent.parent
    if candidate.parent.name != "kicad":
        return None
    if not (build_dir / "pcbnew" / "_pcbnew.dll").exists():
        return None
    return build_dir


def kicad_cli_subprocess_env(candidate: Path) -> dict[str, str] | None:
    """Return subprocess env overrides needed by a build-tree KiCad CLI.

    Staged corpus and installed KiCad CLIs are self-contained enough for the
    default environment. A local debug build, however, needs the build DLL
    directories and vcpkg Python runtime on ``PATH``.
    """
    build_dir = _build_tree_dir(candidate)
    if build_dir is None:
        return None

    vcpkg = build_dir / "vcpkg_installed" / "x64-windows"
    python_home = vcpkg / "tools" / "python3"
    if not python_home.exists():
        return None

    path_entries = [
        build_dir / "kicad",
        build_dir / "pcbnew",
        build_dir / "eeschema",
        build_dir / "common",
        build_dir / "common" / "gal",
        build_dir / "libs" / "kimath",
        build_dir / "libs" / "core",
        build_dir / "libs" / "kiplatform",
        build_dir / "3d-viewer",
        vcpkg / "debug" / "bin",
        vcpkg / "bin",
        python_home,
        python_home / "DLLs",
    ]

    env = os.environ.copy()
    env["KICAD_RUN_FROM_BUILD_DIR"] = "1"
    env["KICAD_USE_EXTERNAL_PYTHONHOME"] = "1"
    env["PYTHONHOME"] = str(python_home)
    env["PATH"] = os.pathsep.join(str(path) for path in path_entries) + os.pathsep + env.get("PATH", "")
    return env


def _iter_kicad_cli_candidates() -> list[Path]:
    """Return kicad-cli candidates in the shared oracle resolution order."""
    candidates: list[Path] = []
    seen: set[Path] = set()

    def add(candidate: Path) -> None:
        key = candidate.resolve() if candidate.exists() else candidate
        if key in seen:
            return
        seen.add(key)
        candidates.append(candidate)

    env_cli = os.environ.get("KICAD_CLI")
    if env_cli:
        add(Path(env_cli))

    manifest_hashes = _manifest_short_hashes()
    for corpus_root in _corpus_roots():
        corpus_tools = corpus_root / "tools" / "kicad-cli"
        for short_hash in manifest_hashes:
            add(corpus_tools / short_hash / "bin" / "kicad-cli.exe")

    for corpus_root in _corpus_roots():
        corpus_tools = corpus_root / "tools" / "kicad-cli"
        if not corpus_tools.exists():
            continue
        staged = sorted(
            (d for d in corpus_tools.iterdir() if d.is_dir()),
            key=lambda d: d.stat().st_mtime,
            reverse=True,
        )
        for staged_dir in staged:
            add(staged_dir / "bin" / "kicad-cli.exe")

    cli = shutil.which("kicad-cli")
    if cli:
        add(Path(cli))

    add(Path(r"C:\Program Files\KiCad\10.0\bin\kicad-cli.exe"))
    add(Path(r"C:\Program Files\KiCad\9.0\bin\kicad-cli.exe"))
    return candidates


def _has_pcb_runtime(candidate: Path) -> bool:
    """Return whether a Windows candidate has access to pcbnew runtime DLLs."""
    if os.name != "nt":
        return True
    return (candidate.parent / "_pcbnew.dll").exists() or _build_tree_dir(candidate) is not None


def _supports_pcb_export_command(candidate: Path, export_command: str) -> bool:
    """Probe whether a kicad-cli executable can load a PCB export command."""
    if not candidate.exists():
        return False
    if not _has_pcb_runtime(candidate):
        return False

    try:
        result = subprocess.run(
            [str(candidate), "pcb", "export", export_command, "--help"],
            capture_output=True,
            text=True,
            env=kicad_cli_subprocess_env(candidate),
            timeout=15,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False

    combined_output = f"{result.stdout}\n{result.stderr}"
    return (
        result.returncode == 0
        and "Failed to parse" not in combined_output
        and f"export {export_command}" in combined_output
    )


def _supports_pcb_svg(candidate: Path) -> bool:
    """Probe whether a kicad-cli executable can load the PCB SVG exporter."""
    return _supports_pcb_export_command(candidate, "svg")


def _supports_pcb_bbox(candidate: Path) -> bool:
    """Probe whether a kicad-cli executable can load the PCB bbox exporter."""
    return _supports_pcb_export_command(candidate, "bbox")


def resolve_kicad_cli(
    *, required_capability: KiCadCliCapability = "any"
) -> Path | None:
    """Find the KiCad 9/10 oracle binary.

    Resolution policy:
    1. explicit ``$KICAD_CLI``;
    2. manifest-listed staged corpus builds, in manifest order;
    3. any other staged corpus build as a fallback;
    4. ``PATH``;
    5. installed KiCad 10/9.
    """
    for candidate in _iter_kicad_cli_candidates():
        if not candidate.exists():
            continue
        if required_capability == "any":
            return candidate
        if required_capability == "pcb_svg":
            if _supports_pcb_svg(candidate):
                return candidate
            continue
        if required_capability == "pcb_bbox":
            if _supports_pcb_bbox(candidate):
                return candidate
            continue
    return None

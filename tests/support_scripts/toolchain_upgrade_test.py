"""Rehearse standalone-era Cruncher upgrade and rollback in a clean venv."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
OLD_CRUNCHER_VERSION = "2026.8.11"
NEW_CRUNCHER_VERSION = "2026.8.11.1"
OLD_MONKEY_VERSION = "2026.8.11.1"
NEW_MONKEY_VERSION = "2026.8.17"


def _latest_wheel(dist_dir: Path, prefix: str) -> Path:
    wheels = sorted(dist_dir.glob(f"{prefix}-*.whl"), key=lambda path: path.stat().st_mtime)
    if not wheels:
        raise SystemExit(f"No {prefix} wheel found in {dist_dir}")
    return wheels[-1].resolve()


def _python(venv: Path) -> Path:
    return venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")


def _run(command: list[str], *, cwd: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode:
        raise SystemExit(
            f"Command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed.stdout


def _assert_versions(python: Path, *, cruncher: str, monkey: str, cwd: Path) -> None:
    output = _run(
        [
            str(python),
            "-I",
            "-c",
            (
                "from importlib.metadata import version; "
                "print(version('kicad-cruncher')); print(version('kicad-monkey'))"
            ),
        ],
        cwd=cwd,
    ).splitlines()
    if output != [cruncher, monkey]:
        raise SystemExit(f"Unexpected installed versions: {output}")
    report = _run([str(python), "-I", "-m", "kicad_cruncher", "version"], cwd=cwd)
    if f"kicad-cruncher {cruncher}" not in report or f"kicad-monkey {monkey}" not in report:
        raise SystemExit(f"Unexpected CLI version report:\n{report}")


def main() -> None:
    monkey_wheel = _latest_wheel(REPOSITORY_ROOT / "dist", "kicad_monkey")
    cruncher_wheel = _latest_wheel(
        REPOSITORY_ROOT / "packages" / "kicad_cruncher" / "dist",
        "kicad_cruncher",
    )

    with tempfile.TemporaryDirectory(prefix="kicad_toolchain_upgrade_test_") as temp:
        root = Path(temp).resolve()
        venv = root / "venv"
        _run([sys.executable, "-m", "venv", str(venv)], cwd=root)
        python = _python(venv)
        pip = [str(python), "-m", "pip", "install", "--disable-pip-version-check", "--no-cache-dir"]

        _run(
            [
                *pip,
                f"kicad-monkey=={OLD_MONKEY_VERSION}",
                f"kicad-cruncher=={OLD_CRUNCHER_VERSION}",
            ],
            cwd=root,
        )
        _assert_versions(
            python,
            cruncher=OLD_CRUNCHER_VERSION,
            monkey=OLD_MONKEY_VERSION,
            cwd=root,
        )

        _run([*pip, "--upgrade", "--force-reinstall", str(monkey_wheel), str(cruncher_wheel)], cwd=root)
        _assert_versions(
            python,
            cruncher=NEW_CRUNCHER_VERSION,
            monkey=NEW_MONKEY_VERSION,
            cwd=root,
        )

        _run(
            [
                *pip,
                "--force-reinstall",
                f"kicad-monkey=={OLD_MONKEY_VERSION}",
                f"kicad-cruncher=={OLD_CRUNCHER_VERSION}",
            ],
            cwd=root,
        )
        _assert_versions(
            python,
            cruncher=OLD_CRUNCHER_VERSION,
            monkey=OLD_MONKEY_VERSION,
            cwd=root,
        )

    print(
        "Toolchain upgrade rehearsal passed: "
        f"Cruncher {OLD_CRUNCHER_VERSION} -> {NEW_CRUNCHER_VERSION} -> {OLD_CRUNCHER_VERSION}"
    )


if __name__ == "__main__":
    main()

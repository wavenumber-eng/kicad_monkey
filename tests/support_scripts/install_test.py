"""Run a clean installed-package test for a built KiCad Monkey wheel."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def _run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> None:
    """Run a subprocess and raise with captured context on failure."""
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        cmd_text = " ".join(command)
        raise SystemExit(
            f"Command failed ({completed.returncode}): {cmd_text}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )


def _latest_wheel(dist_dir: Path) -> Path:
    """Return the newest KiCad Monkey wheel in a dist directory."""
    wheels = sorted(
        dist_dir.glob("kicad_monkey-*.whl"),
        key=lambda path: path.stat().st_mtime,
    )
    if not wheels:
        raise SystemExit(f"No kicad_monkey wheel found in {dist_dir}")
    return wheels[-1]


def _venv_python(venv_dir: Path) -> Path:
    """Return the Python executable path for a venv."""
    script_dir = "Scripts" if os.name == "nt" else "bin"
    executable = "python.exe" if os.name == "nt" else "python"
    return venv_dir / script_dir / executable


def _clean_env(venv_dir: Path) -> dict[str, str]:
    """Build an environment that prefers the test venv and avoids source leakage."""
    venv_dir = venv_dir.resolve()
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    env.pop("PYTHONHOME", None)
    env.pop("__PYVENV_LAUNCHER__", None)
    script_dir = venv_dir / ("Scripts" if os.name == "nt" else "bin")
    env["VIRTUAL_ENV"] = str(venv_dir)
    env["PATH"] = str(script_dir) + os.pathsep + env.get("PATH", "")
    return env


def run_install_test(wheel: Path) -> None:
    """Install a wheel into a temporary venv and verify import behavior."""
    wheel = wheel.resolve()
    if not wheel.exists():
        raise SystemExit(f"Wheel does not exist: {wheel}")

    with tempfile.TemporaryDirectory(prefix="kicad_monkey_install_test_") as temp:
        temp_dir = Path(temp).resolve()
        venv_dir = temp_dir / "venv"
        sys.stdout.write(f"Creating test venv: {venv_dir}\n")
        _run([sys.executable, "-m", "venv", str(venv_dir)], cwd=temp_dir)

        python = _venv_python(venv_dir)
        env = _clean_env(venv_dir)
        _run([str(python), "-m", "pip", "install", str(wheel)], cwd=temp_dir, env=env)
        _run(
            [
                str(python),
                "-c",
                (
                    "import kicad_monkey; "
                    "from kicad_monkey import parse_sexp; "
                    "assert kicad_monkey.__version__; "
                    "assert parse_sexp('(kicad_pcb)')[0] == 'kicad_pcb'; "
                    "print(kicad_monkey.__version__)"
                ),
            ],
            cwd=temp_dir,
            env=env,
        )
        if os.name == "nt":
            _run(
                [
                    str(python),
                    "-I",
                    "-c",
                    (
                        "from kicad_monkey import kicad_native_handshake; "
                        "h=kicad_native_handshake(); "
                        "assert h['version']=='a0'; "
                        "assert h['operations']==['design-facts']"
                    ),
                ],
                cwd=temp_dir,
                env=env,
            )
        sys.stdout.write("Installed-package test passed.\n")


def main() -> None:
    """Parse arguments and run the install test."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--wheel",
        type=Path,
        default=None,
        help="Wheel to install. Defaults to the newest kicad_monkey wheel in dist/.",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[2]
    wheel = args.wheel or _latest_wheel(repo_root / "dist")
    run_install_test(wheel)


if __name__ == "__main__":
    main()

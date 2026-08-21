"""Windows no-fallback Cruncher physical-provider exit gate."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def test_cruncher_uses_the_real_native_physical_provider_without_fallback() -> None:
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = "4"
    env["RUST_TEST_THREADS"] = "2"
    env["KICAD_CRUNCHER_NATIVE_PHYSICAL"] = "1"
    native = PACKAGE_ROOT / "target" / "debug" / (
        "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
    )
    env["KICAD_MONKEY_NATIVE"] = str(native)
    commands = [
        [
            "cargo",
            "build",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-native",
        ],
        [
            "uv",
            "run",
            "--package",
            "kicad-cruncher",
            "pytest",
            "packages/kicad_cruncher/tests/L3_public_workflows/test_L3_008_native_physical_provider.py",
            "-q",
        ],
    ]
    for command in commands:
        completed = subprocess.run(
            command,
            cwd=PACKAGE_ROOT,
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=300,
            check=False,
        )
        assert completed.returncode == 0, completed.stdout + completed.stderr

"""Native base-SVG parity and resource exit gate."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def test_all_frozen_documents_match_native_svg_snapshots_and_resource_gates() -> None:
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = "4"
    env["RUST_TEST_THREADS"] = "2"
    npm = "npm.cmd" if os.name == "nt" else "npm"
    commands = [
        [npm, "run", "generate:contracts"],
        [npm, "run", "check:typespec"],
        [npm, "run", "check:python-generation"],
        [npm, "run", "check:typescript-generation"],
        [
            "uv",
            "run",
            "pytest",
            "tests/L0_foundation/test_L0_046_rust_l0_signoff.py::test_phase5_plotter_contract_freeze_manifest_is_exact",
            "-q",
        ],
        [
            "cargo",
            "run",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-codegen",
            "--",
            "--check",
        ],
        [
            "cargo",
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-contracts",
            "--test",
            "native_transport_contracts",
            "--",
            "--test-threads",
            "2",
        ],
        [
            "cargo",
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-svg",
            "--",
            "--test-threads",
            "2",
        ],
        [
            "cargo",
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-native",
            "--test",
            "render_svg",
            "--",
            "--test-threads",
            "2",
        ],
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
            "python",
            "scripts/generate_native_svg_vectors.py",
            "--check",
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

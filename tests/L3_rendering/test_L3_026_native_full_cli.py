"""Installed native-backed Cruncher CLI compatibility exit gate."""

from __future__ import annotations

import os
import subprocess
import tempfile
from pathlib import Path

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
CRUNCHER_ROOT = PACKAGE_ROOT / "packages" / "kicad_cruncher"


def _run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    timeout: int = 900,
) -> None:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def _only(directory: Path, pattern: str) -> Path:
    matches = list(directory.glob(pattern))
    assert len(matches) == 1, f"expected one {pattern} in {directory}, got {matches}"
    return matches[0]


def _build_release_artifacts(root: Path, env: dict[str, str]) -> tuple[Path, ...]:
    monkey_sdist_dir = root / "monkey-sdist"
    monkey_wheel_dir = root / "monkey-wheel"
    cruncher_sdist_dir = root / "cruncher-sdist"
    cruncher_wheel_dir = root / "cruncher-wheel"
    _run(
        ["uv", "build", "--sdist", "--out-dir", str(monkey_sdist_dir)],
        cwd=PACKAGE_ROOT,
        env=env,
    )
    monkey_sdist = _only(monkey_sdist_dir, "kicad_monkey-*.tar.gz")
    _run(
        [
            "uv",
            "build",
            str(monkey_sdist),
            "--wheel",
            "--out-dir",
            str(monkey_wheel_dir),
        ],
        cwd=PACKAGE_ROOT,
        env=env,
    )
    _run(
        ["uv", "build", "--sdist", "--out-dir", str(cruncher_sdist_dir)],
        cwd=CRUNCHER_ROOT,
        env=env,
    )
    cruncher_sdist = _only(cruncher_sdist_dir, "kicad_cruncher-*.tar.gz")
    _run(
        [
            "uv",
            "build",
            str(cruncher_sdist),
            "--wheel",
            "--out-dir",
            str(cruncher_wheel_dir),
        ],
        cwd=CRUNCHER_ROOT,
        env=env,
    )
    return (
        _only(monkey_wheel_dir, "kicad_monkey-*.whl"),
        _only(cruncher_wheel_dir, "kicad_cruncher-*.whl"),
        monkey_sdist,
        cruncher_sdist,
    )


def test_installed_cruncher_cli_is_native_backed_and_compatible() -> None:
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = "4"
    env["RUST_TEST_THREADS"] = "2"
    env["KICAD_CRUNCHER_NATIVE_PHYSICAL"] = "1"
    env["KICAD_CRUNCHER_NATIVE_DESIGN_FACTS"] = "1"
    native = (
        PACKAGE_ROOT
        / "target"
        / "debug"
        / ("kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native")
    )
    env["KICAD_MONKEY_NATIVE"] = str(native)
    _run(
        [
            "cargo",
            "build",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-native",
        ],
        cwd=PACKAGE_ROOT,
        env=env,
    )
    _run(
        [
            "uv",
            "run",
            "--package",
            "kicad-cruncher",
            "--extra",
            "test",
            "pytest",
            "tests/L0_foundation/test_L0_062_native_process_client.py",
            "tests/L0_foundation/test_L0_063_native_svg_transport.py",
            "packages/kicad_cruncher/tests/L0_public_cli/test_L0_001_cli_entrypoint.py",
            "packages/kicad_cruncher/tests/L0_public_cli/"
            "test_L0_004_design_review_manifest.py",
            "packages/kicad_cruncher/tests/L3_public_workflows/"
            "test_L3_008_native_physical_provider.py",
            "packages/kicad_cruncher/tests/L3_public_workflows/"
            "test_L3_009_native_design_facts_provider.py",
            "packages/kicad_cruncher/tests/L3_public_workflows/"
            "test_L3_010_design_cli_compatibility.py",
            "-q",
        ],
        cwd=PACKAGE_ROOT,
        env=env,
    )
    with tempfile.TemporaryDirectory(prefix="kicad_phase6_cli_") as temporary:
        monkey_wheel, cruncher_wheel, monkey_sdist, cruncher_sdist = (
            _build_release_artifacts(Path(temporary), env)
        )
        _run(
            [
                "uv",
                "run",
                "--extra",
                "test",
                "python",
                "tests/support_scripts/toolchain_install_test.py",
                "--monkey-wheel",
                str(monkey_wheel),
                "--cruncher-wheel",
                str(cruncher_wheel),
                "--monkey-sdist",
                str(monkey_sdist),
                "--cruncher-sdist",
                str(cruncher_sdist),
            ],
            cwd=PACKAGE_ROOT,
            env=env,
        )

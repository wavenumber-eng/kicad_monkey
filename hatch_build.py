"""Hatch build hook for the Windows native operation process."""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
from pathlib import Path
from typing import Any

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


class CustomBuildHook(BuildHookInterface):
    """Build and include the Monkey-owned native executable on Windows."""

    def initialize(self, version: str, build_data: dict[str, Any]) -> None:
        if self.target_name == "sdist":
            standalone_manifest = Path(self.root) / "packaging" / "Cargo.sdist.toml"
            if not standalone_manifest.is_file():
                raise RuntimeError(
                    f"standalone sdist workspace manifest is missing: {standalone_manifest}"
                )
            build_data["force_include"][str(standalone_manifest)] = "Cargo.toml"
            return
        if self.target_name != "wheel" or version == "editable" or os.name != "nt":
            return
        cargo = shutil.which("cargo")
        if cargo is None:
            raise RuntimeError("cargo is required to build the Windows native wheel")
        env = dict(os.environ)
        env["CARGO_BUILD_JOBS"] = "4"
        env["RUST_TEST_THREADS"] = "2"
        subprocess.run(
            [
                cargo,
                "build",
                "--locked",
                "--release",
                "--jobs",
                "4",
                "--package",
                "kicad-monkey-native",
            ],
            cwd=self.root,
            env=env,
            check=True,
        )
        binary = Path(self.root) / "target" / "release" / "kicad-monkey-native.exe"
        if not binary.is_file():
            raise RuntimeError(f"native build did not produce {binary}")
        build_data["force_include"][str(binary)] = (
            "kicad_monkey/_native/kicad-monkey-native.exe"
        )
        build_data["pure_python"] = False
        build_data["tag"] = f"py3-none-{_windows_platform_tag()}"


def _windows_platform_tag() -> str:
    machine = platform.machine().casefold()
    if machine in {"amd64", "x86_64"}:
        return "win_amd64"
    if machine in {"arm64", "aarch64"}:
        return "win_arm64"
    raise RuntimeError(f"unsupported Windows wheel architecture: {platform.machine()}")

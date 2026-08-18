"""Verify the Windows wheel contains and runs the native operation process."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from zipfile import ZipFile


_SCHEMATIC = """(kicad_sch
  (version 20260306)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid root)
  (paper "A4")
)
"""


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: native_wheel_test.py PATH_TO_WHEEL")
    wheel = Path(sys.argv[1]).resolve()
    if not wheel.is_file() or "-py3-none-win_" not in wheel.name:
        raise SystemExit(f"not a Windows py3 wheel: {wheel}")
    member = "kicad_monkey/_native/kicad-monkey-native.exe"
    with ZipFile(wheel) as archive:
        names = archive.namelist()
        if names.count(member) != 1:
            raise SystemExit(f"wheel must contain exactly one {member}")
        if any("src/rs/" in name or "target/" in name for name in names):
            raise SystemExit("wheel contains Rust workspace/build paths")
        binary = archive.read(member)
    workspace = str(Path(__file__).resolve().parents[2]).encode("utf-8")
    if workspace in binary:
        raise SystemExit("native executable embeds the build workspace path")

    uv = shutil.which("uv")
    if uv is None:
        raise SystemExit("uv is required for the isolated wheel check")
    with tempfile.TemporaryDirectory(prefix="kicad-monkey-native-wheel-") as temporary:
        root = Path(temporary)
        environment = root / "venv"
        subprocess.run([uv, "venv", str(environment)], check=True, cwd=root)
        python = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        subprocess.run(
            [uv, "pip", "install", "--python", str(python), str(wheel)],
            check=True,
            cwd=root,
        )
        (root / "demo.kicad_pro").write_text("{}\n", encoding="utf-8")
        (root / "demo.kicad_sch").write_text(_SCHEMATIC, encoding="utf-8")
        completed = subprocess.run(
            [
                str(python),
                "-I",
                "-c",
                (
                    "import json,sys; "
                    "from pathlib import Path; "
                    "from kicad_monkey import KiCadDesign,kicad_native_handshake,"
                    "native_design_facts_for_design; "
                    "root=Path(sys.argv[1]); "
                    "handshake=kicad_native_handshake(); "
                    "facts=native_design_facts_for_design("
                    "KiCadDesign.from_project_file(root/'demo.kicad_pro')); "
                    "assert facts.engine_version==handshake['engine_version']; "
                    "assert '(version \"E\"' in facts.kicad_netlist; "
                    "print(json.dumps(handshake, sort_keys=True))"
                ),
                str(root),
            ],
            check=True,
            cwd=root,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        payload = json.loads(completed.stdout)
    if payload != {
        "engine_version": "0.1.0",
        "operations": ["design-facts"],
        "type": "kicad_monkey.native.handshake",
        "version": "a0",
    }:
        raise SystemExit(f"unexpected native handshake: {payload!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

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

_FOOTPRINT_DOCUMENT = {
    "schema": "kicad.plotter_ir.a0",
    "source_kind": "MOD",
    "total_operations": 1,
    "records": [
        {
            "uuid": "line",
            "kind": "footprint",
            "object_id": "Demo",
            "operation_count": 1,
            "operations": [
                {
                    "kind": "ThickSegment",
                    "index": 0,
                    "start_x": 0,
                    "start_y": 0,
                    "end_x": 1_000_000,
                    "end_y": 0,
                    "width_nm": 100_000,
                    "layer": "F.SilkS",
                }
            ],
            "name": "Demo",
            "layer": "F.Cu",
            "locked": False,
            "placed": False,
            "descr": "",
            "tags": "",
            "attr": [],
        }
    ],
    "source_path": "demo.kicad_mod",
    "document_id": "wheel-svg",
    "coordinate_space": {"unit": "nm", "y_axis": "down"},
    "version": 20260101,
    "generator": "pcbnew",
    "generator_version": "10.0",
}


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
                    "kicad_native_handshake_a1,kicad_native_handshake_a2,"
                    "native_design_facts_for_design,native_render_svg; "
                    "root=Path(sys.argv[1]); "
                    "document=json.loads(sys.argv[2]); "
                    "handshake=kicad_native_handshake(); "
                    "handshake_a1=kicad_native_handshake_a1(); "
                    "handshake_a2=kicad_native_handshake_a2(); "
                    "facts=native_design_facts_for_design("
                    "KiCadDesign.from_project_file(root/'demo.kicad_pro')); "
                    "svg=native_render_svg(document,document_kind='footprint',"
                    "viewport={'min_x_nm':0,'min_y_nm':0,'width_nm':1000000,'height_nm':1000000}); "
                    "assert facts.engine_version==handshake['engine_version']; "
                    "assert svg.engine_version==handshake_a1['engine_version']; "
                    "assert svg.document_id=='wheel-svg' and '<line' in svg.svg_utf8; "
                    "assert '(version \"E\"' in facts.kicad_netlist; "
                    "assert facts.resource_profile=='design-facts-bounded-a1'; "
                    "assert facts.source_snapshot_sha256==facts.design_fingerprint; "
                    "print(json.dumps({'a0':handshake,'a1':handshake_a1,'a2':handshake_a2}, sort_keys=True))"
                ),
                str(root),
                json.dumps(_FOOTPRINT_DOCUMENT, separators=(",", ":")),
            ],
            check=True,
            cwd=root,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        payload = json.loads(completed.stdout)
    if payload["a0"] != {
        "engine_version": "0.1.0",
        "operations": ["design-facts"],
        "type": "kicad_monkey.native.handshake",
        "version": "a0",
    }:
        raise SystemExit(f"unexpected native handshake: {payload!r}")
    if payload["a1"]["operations"] != ["design-facts", "render-svg"]:
        raise SystemExit(f"unexpected expanded native handshake: {payload!r}")
    if payload["a2"]["operations"] != [
        "design-facts",
        "render-svg",
        "design-facts-a1",
    ]:
        raise SystemExit(f"unexpected source-bound native handshake: {payload!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

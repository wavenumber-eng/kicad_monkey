"""Real Python-to-Rust gate for the native design-facts transport."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from kicad_monkey import KiCadDesign
from kicad_monkey.kicad_compiled_schematic_graph import (
    build_compiled_schematic_graph,
)
from kicad_monkey.kicad_native import (
    kicad_native_handshake,
    native_design_facts_for_design,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
NATIVE_BINARY = (
    PACKAGE_ROOT
    / "target"
    / "debug"
    / ("kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native")
)

_SCHEMATIC = """(kicad_sch
  (version 20260306)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid root)
  (paper "A4")
  (lib_symbols
    (symbol "Demo:One"
      (symbol "Demo:One_1_1"
        (pin passive line (at 0 0 0) (name "P") (number "1")))))
  (symbol
    (lib_id "Demo:One")
    (lib_name "Demo:One")
    (at 0 0 0)
    (uuid root-symbol)
    (property "Reference" "R1")
    (property "Value" "One"))
)
"""


def _build_native_binary() -> None:
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = "4"
    env["RUST_TEST_THREADS"] = "2"
    completed = subprocess.run(
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
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=300,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert NATIVE_BINARY.is_file()


def test_python_client_consumes_real_native_design_facts(tmp_path: Path) -> None:
    _build_native_binary()
    project = tmp_path / "demo.kicad_pro"
    schematic = tmp_path / "demo.kicad_sch"
    project.write_text("{}\n", encoding="utf-8")
    schematic.write_text(_SCHEMATIC, encoding="utf-8")
    design = KiCadDesign.from_project_file(project)

    handshake = kicad_native_handshake(executable=NATIVE_BINARY)
    facts = native_design_facts_for_design(design, executable=NATIVE_BINARY)

    assert handshake["operations"] == ["design-facts"]
    assert facts.engine_version == handshake["engine_version"]
    assert facts.compiled_schematic_graph == build_compiled_schematic_graph(design).to_json()
    assert '(version "E"' in facts.kicad_netlist
    assert design.to_json(compiled_schematic_graph=facts)[
        "compiled_schematic_graph"
    ] == facts.compiled_schematic_graph

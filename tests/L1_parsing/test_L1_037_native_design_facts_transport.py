"""Real Python-to-Rust gate for the native design-facts transport."""

from __future__ import annotations

import hashlib
import os
import subprocess
from pathlib import Path

from kicad_monkey import (
    KiCadDesign,
    get_value,
    kicad_native_handshake,
    kicad_native_handshake_a2,
    native_design_facts,
    native_design_facts_for_design,
    parse_sexp,
)
from kicad_monkey.kicad_compiled_schematic_graph import (
    build_compiled_schematic_graph,
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

    project_bytes = project.read_bytes()
    schematic_bytes = schematic.read_bytes()
    manifest = {
        "schema": "kicad_monkey.source_bundle_manifest.a0",
        "type": "kicad_monkey.source_bundle_manifest",
        "version": "a0",
        "root_schematic_path": schematic.name,
        "project_path": project.name,
        "sources": [
            {
                "path": project.name,
                "kind": "project",
                "slot": 0,
                "source_bytes": str(len(project_bytes)),
            },
            {
                "path": schematic.name,
                "kind": "schematic",
                "slot": 1,
                "source_bytes": str(len(schematic_bytes)),
            },
        ],
    }
    limits = {
        "max_sources": 2,
        "max_source_bytes": str(max(len(project_bytes), len(schematic_bytes))),
        "max_total_source_bytes": str(len(project_bytes) + len(schematic_bytes)),
        "max_path_bytes": 4096,
        "max_output_bytes": str(64 * 1024 * 1024),
    }
    source_path = str(schematic.resolve())
    handshake_a0 = kicad_native_handshake(executable=NATIVE_BINARY)
    legacy_facts = native_design_facts(
        bundle_root=tmp_path,
        manifest=manifest,
        file_slots=[
            {"slot": 0, "path": project.name},
            {"slot": 1, "path": schematic.name},
        ],
        limits=limits,
        source_path=source_path,
        date="2026-08-17",
        tool="l1-037",
        executable=NATIVE_BINARY,
    )
    handshake_a2 = kicad_native_handshake_a2(executable=NATIVE_BINARY)
    facts = native_design_facts_for_design(
        design,
        source_path=source_path,
        date="2026-08-17",
        tool="l1-037",
        executable=NATIVE_BINARY,
    )

    assert handshake_a0["operations"] == ["design-facts"]
    assert handshake_a2["operations"] == [
        "design-facts",
        "render-svg",
        "design-facts-a1",
    ]
    assert legacy_facts.engine_version == handshake_a0["engine_version"]
    assert facts.engine_version == handshake_a2["engine_version"]
    expected_graph = build_compiled_schematic_graph(design).to_json()
    assert legacy_facts.compiled_schematic_graph == expected_graph
    assert facts.compiled_schematic_graph == expected_graph
    assert legacy_facts.resource_profile is None
    assert legacy_facts.source_snapshot_sha256 is None
    assert legacy_facts.kicad_netlist_bytes is None
    assert legacy_facts.kicad_netlist_sha256 is None
    assert facts.resource_profile == "design-facts-bounded-a1"
    assert facts.source_snapshot_sha256 == facts.design_fingerprint
    assert facts.source_snapshot_sha256 is not None
    assert len(facts.source_snapshot_sha256) == 64
    assert set(facts.source_snapshot_sha256) <= set("0123456789abcdef")
    netlist_bytes = facts.kicad_netlist.encode("utf-8")
    assert facts.kicad_netlist_bytes == len(netlist_bytes)
    assert facts.kicad_netlist_sha256 == hashlib.sha256(netlist_bytes).hexdigest()
    assert '(version "E"' in facts.kicad_netlist
    netlist_root = parse_sexp(facts.kicad_netlist)
    design_block = next(
        child
        for child in netlist_root[1:]
        if isinstance(child, list) and child and child[0] == "design"
    )
    assert get_value(design_block, "source") == source_path
    assert get_value(design_block, "date") == "2026-08-17"
    assert get_value(design_block, "tool") == "l1-037"
    assert design.to_json(compiled_schematic_graph=facts)[
        "compiled_schematic_graph"
    ] == facts.compiled_schematic_graph

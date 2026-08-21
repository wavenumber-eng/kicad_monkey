"""Source-bound native design-facts provider exit gate."""

from __future__ import annotations

import os
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

from kicad_monkey import (
    KiCadDesign,
    KiCadNativeError,
    KiCadSvgRenderOptions,
    native_design_facts_for_design,
    parse_sexp,
    render_ir_to_svg,
)
from kicad_monkey.kicad_compiled_schematic_graph import (
    build_compiled_schematic_graph,
)
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    resolve_kicad_manifest_path,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
REFERENCE_CASES = (
    "real_world/yoshi_mainboard",
    "real_world/taillight",
    "real_world/speedy_processing_module",
    "real_world/jumperless_v5r7",
)
YOSHI_POWER_SYMBOL_ID = "0712f77e-8c3e-4d5a-85be-9f5a0ee70922"
YOSHI_UNRENDERED_PIN_ID = "dc9808a5-00d5-4a27-a92d-73f6a1ccbd03"


def _unordered_sexpr(value: object) -> object:
    """Canonicalize version-E blocks whose child-form order is non-semantic."""
    if not isinstance(value, list):
        return str(value)
    scalars = tuple(str(item) for item in value if not isinstance(item, list))
    children = sorted(
        (_unordered_sexpr(item) for item in value if isinstance(item, list)),
        key=repr,
    )
    return scalars, tuple(children)


def _graph_element_ids(value: object) -> set[str]:
    if isinstance(value, dict):
        ids = {
            element_id
            for key, element_id in value.items()
            if key == "element_id" and isinstance(element_id, str) and element_id
        }
        for child in value.values():
            ids.update(_graph_element_ids(child))
        return ids
    if isinstance(value, list):
        ids: set[str] = set()
        for child in value:
            ids.update(_graph_element_ids(child))
        return ids
    return set()


def _rendered_schematic_ids(design: KiCadDesign) -> set[str]:
    ids: set[str] = set()
    options = KiCadSvgRenderOptions.enriched_default()
    for instance in design.schematic_instances():
        svg = render_ir_to_svg(design.to_schematic_instance_ir(instance), options=options)
        for element in ET.fromstring(svg).iter():
            element_id = element.attrib.get("id", "")
            if element_id:
                ids.add(element_id)
    return ids


def _assert_reference_project_parity(native: Path) -> None:
    yoshi_graph: dict[str, object] | None = None
    yoshi_design: KiCadDesign | None = None
    for case_id in REFERENCE_CASES:
        case = get_kicad_corpus_case(case_id)
        assert case is not None, case_id
        project_path = resolve_kicad_manifest_path(case, "project_file")
        assert project_path is not None, case_id
        design = KiCadDesign.from_project_file(project_path)
        top = design.top_schematic
        assert top is not None and top.source_path is not None
        source_path = str(top.source_path)
        try:
            first = native_design_facts_for_design(
                design,
                source_path=source_path,
                date="",
                tool="kicad_cruncher",
                executable=native,
            )
            second = native_design_facts_for_design(
                design,
                source_path=source_path,
                date="",
                tool="kicad_cruncher",
                executable=native,
            )
        except KiCadNativeError as error:
            raise AssertionError(f"native design facts failed for {case_id}: {error}") from error
        expected_graph = build_compiled_schematic_graph(design).to_json()
        assert first.compiled_schematic_graph == expected_graph, case_id
        assert second.compiled_schematic_graph == first.compiled_schematic_graph, case_id
        assert second.source_snapshot_sha256 == first.source_snapshot_sha256, case_id
        assert second.kicad_netlist == first.kicad_netlist, case_id
        expected_netlist = design.to_kicad_netlist_sexpr(
            tool="kicad_cruncher", date=""
        )
        assert _unordered_sexpr(parse_sexp(first.kicad_netlist)) == _unordered_sexpr(
            parse_sexp(expected_netlist)
        ), case_id
        if case_id == "real_world/yoshi_mainboard":
            yoshi_graph = first.compiled_schematic_graph
            yoshi_design = design

    assert yoshi_graph is not None and yoshi_design is not None
    graph_ids = _graph_element_ids(yoshi_graph)
    rendered_ids = _rendered_schematic_ids(yoshi_design)
    assert YOSHI_POWER_SYMBOL_ID in graph_ids
    assert YOSHI_POWER_SYMBOL_ID in rendered_ids
    assert YOSHI_UNRENDERED_PIN_ID not in graph_ids
    assert YOSHI_UNRENDERED_PIN_ID not in rendered_ids
    assert graph_ids <= rendered_ids


def test_source_bound_native_design_facts_drive_cruncher_without_fallback() -> None:
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = "4"
    env["RUST_TEST_THREADS"] = "2"
    env["KICAD_CRUNCHER_NATIVE_DESIGN_FACTS"] = "1"
    native = PACKAGE_ROOT / "target" / "debug" / (
        "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
    )
    env["KICAD_MONKEY_NATIVE"] = str(native)
    npm = "npm.cmd" if os.name == "nt" else "npm"
    commands = [
        [npm, "run", "generate:contracts"],
        [npm, "run", "check:typespec"],
        [npm, "run", "check:python-generation"],
        [npm, "run", "check:typescript-generation"],
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
            "kicad-monkey-native",
            "--test",
            "design_facts",
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
            "--all-packages",
            "--all-extras",
            "pytest",
            "tests/L0_foundation/test_L0_049_generated_contract_projections.py",
            "tests/L0_foundation/test_L0_062_native_process_client.py",
            "tests/L1_parsing/test_L1_037_native_design_facts_transport.py",
            "packages/kicad_cruncher/tests/L3_public_workflows/test_L3_009_native_design_facts_provider.py",
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
            timeout=600,
            check=False,
        )
        assert completed.returncode == 0, completed.stdout + completed.stderr
    _assert_reference_project_parity(native)

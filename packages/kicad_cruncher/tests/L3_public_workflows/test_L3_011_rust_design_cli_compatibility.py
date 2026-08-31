"""Behavioral oracle checks shared by the Python and Rust design CLIs."""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from kicad_cruncher import kicad_cruncher_cmd_design as design_cmd
from kicad_monkey import (
    KiCadDesign,
    find_all_elements,
    find_element,
    get_value,
    parse_sexp,
)

_PACKAGE_ROOT = Path(__file__).resolve().parents[2]
_WORKSPACE = Path(__file__).resolve().parents[4]
_RUST_EXE = _WORKSPACE / "target" / "debug" / (
    "kicad-cruncher.exe" if os.name == "nt" else "kicad-cruncher"
)
_RUST_NETLIST_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "design_netlist_json_oracle.exe"
    if os.name == "nt"
    else "design_netlist_json_oracle"
)
_RUST_DESIGN_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "design_json_oracle.exe" if os.name == "nt" else "design_json_oracle"
)
_RUST_SCHEMATIC_INSTANCES_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "schematic_instances_oracle.exe"
    if os.name == "nt"
    else "schematic_instances_oracle"
)
_RUST_SCHEMATIC_PLOT_DOCUMENTS_ORACLE = (
    _WORKSPACE
    / "target"
    / "debug"
    / "examples"
    / (
        "schematic_plot_documents_oracle.exe"
        if os.name == "nt"
        else "schematic_plot_documents_oracle"
    )
)
_RUST_SCHEMATIC_BASE_SVGS_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "schematic_base_svgs_oracle.exe"
    if os.name == "nt"
    else "schematic_base_svgs_oracle"
)
_RUST_SCHEMATIC_REVIEW_SVGS_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "schematic_review_svgs_oracle.exe"
    if os.name == "nt"
    else "schematic_review_svgs_oracle"
)
_RUST_PCB_REVIEW_SVGS_ORACLE = _WORKSPACE / "target" / "debug" / "examples" / (
    "pcb_review_svgs_oracle.exe" if os.name == "nt" else "pcb_review_svgs_oracle"
)
_PROJECT = (
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "hlr_test"
    / "hlr_test.kicad_pro"
)
_REPRESENTATIVE_PROJECTS = (
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "taillight"
    / "input"
    / "11-10045__taillight__C.kicad_pro",
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "yoshi_mainboard"
    / "input"
    / "11-10080__yoshi-mainboard__A.kicad_pro",
)
_LARGE_HIERARCHY_PROJECT = (
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "4-ch-backplane"
    / "input"
    / "4-ch-backplane.kicad_pro"
)
_EMBEDDED_WORKSHEET_PROJECT = (
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "charge_indicator"
    / "input"
    / "11-10043__charge_indicator__C.kicad_pro"
)
_EMBEDDED_BERKELEY_PROJECT = (
    _PACKAGE_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "speedy_processing_module"
    / "input"
    / "11-10084__speedy_processing_module__B.kicad_pro"
)
_MANIFEST_SCHEMA = (
    _PACKAGE_ROOT / "docs" / "contracts" / "design_review_manifest.a0.schema.json"
)
_PROJECT_BUS_ALIAS_PROJECT = (
    _WORKSPACE
    / "tests"
    / "cases"
    / "project_bus_alias_hierarchy"
    / "input"
    / "project_bus_alias_hierarchy.kicad_pro"
)
_PROJECT_BUS_ALIAS_EXPECTED = {
    "/CTRL_A": {("TP1", "1"), ("TP101", "1")},
    "/CTRL_B": {("TP2", "1"), ("TP102", "1")},
}


def _first_json_difference(actual: object, expected: object, path: str = "$") -> str:
    if type(actual) is not type(expected):
        return f"{path}: type {type(actual).__name__} != {type(expected).__name__}"
    if isinstance(actual, dict):
        if actual.keys() != expected.keys():
            return f"{path}: keys {actual.keys() ^ expected.keys()}"
        for key in sorted(actual, key=lambda value: value == "total_operations"):
            if actual[key] != expected[key]:
                return _first_json_difference(actual[key], expected[key], f"{path}.{key}")
    elif isinstance(actual, list):
        for index, value in enumerate(actual[: len(expected)]):
            if value != expected[index]:
                if path.endswith(".records"):
                    actual_window = [
                        (item.get("kind"), item.get("uuid"))
                        for item in actual[max(0, index - 2) : index + 3]
                    ]
                    expected_window = [
                        (item.get("kind"), item.get("uuid"))
                        for item in expected[max(0, index - 2) : index + 3]
                    ]
                    return (
                        f"{path}[{index}]: "
                        f"{value.get('kind')} {value.get('uuid')} != "
                        f"{expected[index].get('kind')} {expected[index].get('uuid')}; "
                        f"windows {actual_window!r} != {expected_window!r}"
                    )
                return _first_json_difference(value, expected[index], f"{path}[{index}]")
        if len(actual) != len(expected):
            return f"{path}: length {len(actual)} != {len(expected)}"
    return f"{path}: {actual!r} != {expected!r}"


def _svg_record_geometry(element: ET.Element) -> tuple[int, tuple[float, ...]]:
    """Tag-independent unique-coordinate count and envelope for one record."""
    drawable = {"path", "polygon", "polyline", "line", "rect", "circle", "ellipse"}
    points: list[tuple[float, float]] = []
    for child in element.iter():
        tag = child.tag.rsplit("}", 1)[-1]
        if tag not in drawable:
            continue
        attrs = child.attrib
        if tag in {"polygon", "polyline"}:
            numbers = [
                float(value)
                for value in re.findall(
                    r"[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?", attrs.get("points", "")
                )
            ]
            points.extend(zip(numbers[::2], numbers[1::2], strict=False))
        elif tag == "path":
            numbers = [
                float(value)
                for value in re.findall(
                    r"[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?", attrs.get("d", "")
                )
            ]
            points.extend(zip(numbers[::2], numbers[1::2], strict=False))
        elif tag == "line":
            points.extend(
                [(float(attrs["x1"]), float(attrs["y1"])), (float(attrs["x2"]), float(attrs["y2"]))]
            )
        elif tag == "rect":
            x, y = float(attrs.get("x", 0)), float(attrs.get("y", 0))
            width, height = float(attrs.get("width", 0)), float(attrs.get("height", 0))
            points.extend([(x, y), (x + width, y + height)])
        else:
            cx, cy = float(attrs.get("cx", 0)), float(attrs.get("cy", 0))
            rx = float(attrs.get("r", attrs.get("rx", 0)))
            ry = float(attrs.get("r", attrs.get("ry", 0)))
            points.extend([(cx - rx, cy - ry), (cx + rx, cy + ry)])
    if not points:
        return 0, ()
    normalized = {(round(x, 6), round(y, 6)) for x, y in points}
    xs, ys = zip(*normalized, strict=True)
    envelope = tuple(round(value, 6) for value in (min(xs), min(ys), max(xs), max(ys)))
    assert all(math.isfinite(value) for value in envelope)
    return len(normalized), envelope
_DESIGN_HELP_MARKERS = (
    "design review bundle",
    "enriched black-and-white schematic SVGs",
    "enriched PCB copper-layer SVGs",
    "KiCad-native netlist JSON",
    "KiCad S-expression netlist",
    "project metadata",
    "schematic hierarchy",
    "components",
    "nets",
    "default: ./output/design",
)


@pytest.fixture(scope="module", autouse=True)
def _build_rust_cli() -> None:
    completed = subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "kicad-cruncher-cli",
            "--bins",
            "--examples",
        ],
        cwd=_WORKSPACE,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert _RUST_EXE.is_file()
    assert _RUST_NETLIST_ORACLE.is_file()
    assert _RUST_DESIGN_ORACLE.is_file()
    assert _RUST_SCHEMATIC_INSTANCES_ORACLE.is_file()
    assert _RUST_SCHEMATIC_PLOT_DOCUMENTS_ORACLE.is_file()
    assert _RUST_SCHEMATIC_BASE_SVGS_ORACLE.is_file()
    assert _RUST_SCHEMATIC_REVIEW_SVGS_ORACLE.is_file()
    assert _RUST_PCB_REVIEW_SVGS_ORACLE.is_file()


def _run(
    command: list[str], *, timeout: int = 60, cwd: Path = _PACKAGE_ROOT
) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env.update({"NO_COLOR": "1", "PYTHONUTF8": "1"})
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
    )


_WINDOWS_EXACT_RENDER_ORACLE = pytest.mark.skipif(
    os.name != "nt",
    reason=(
        "exact plot/font geometry is governed against the Windows x64 release "
        "target; Linux intentionally resolves a different system-font authority"
    ),
)


@pytest.mark.parametrize("alias", ("design", "design-review", "dr"))
def test_rust_design_help_matches_the_python_cli_contract(alias: str) -> None:
    python = _run([sys.executable, "-m", "kicad_cruncher", alias, "--help"])
    rust = _run([str(_RUST_EXE), alias, "--help"])

    assert python.returncode == rust.returncode == 0
    assert python.stderr == rust.stderr == ""
    for marker in _DESIGN_HELP_MARKERS:
        assert marker in python.stdout
        assert marker in rust.stdout


@pytest.mark.parametrize("alias", ("design", "design-review", "dr"))
def test_rust_usage_errors_match_the_python_cli_contract(alias: str) -> None:
    arguments = [alias, "--not-a-real-option"]
    python = _run([sys.executable, "-m", "kicad_cruncher", *arguments])
    rust = _run([str(_RUST_EXE), *arguments])

    assert python.returncode == rust.returncode == 2
    assert python.stdout == rust.stdout == ""
    for completed in (python, rust):
        assert "usage: kicad-cruncher" in completed.stderr
        assert "unrecognized arguments: --not-a-real-option" in completed.stderr
        assert "Traceback" not in completed.stderr


def _bundle_digest(root: Path) -> dict[str, str]:
    return {
        path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def _json_net_terminals(payload: dict) -> dict[str, set[tuple[str, str]]]:
    return {
        str(net["name"]): {
            (str(terminal["designator"]), str(terminal["pin"]))
            for terminal in net["terminals"]
        }
        for net in payload["nets"]
    }


def _sexpr_net_terminals(text: str) -> dict[str, set[tuple[str, str]]]:
    nets = find_element(parse_sexp(text), "nets")
    assert nets is not None
    return {
        str(get_value(net, "name") or ""): {
            (
                str(get_value(node, "ref") or ""),
                str(get_value(node, "pin") or ""),
            )
            for node in find_all_elements(net, "node")
        }
        for net in find_all_elements(nets, "net")
    }


def _assert_rust_bundle(output: Path, project: Path = _PROJECT) -> None:
    manifest = json.loads((output / "design_review_manifest.json").read_text("utf-8"))
    Draft202012Validator(json.loads(_MANIFEST_SCHEMA.read_text("utf-8"))).validate(
        manifest
    )
    graph_record = manifest["compiled_schematic_graph"]
    schematic = manifest["schematic_svgs"]
    pcbs = manifest["pcb_svgs"]
    expected_files = {
        "README.md",
        "design_review_manifest.json",
        manifest["design_json"],
        graph_record["file"],
        manifest["netlist_json"],
        manifest["netlist_kicad_sexpr"],
        *(item["file"] for item in schematic),
        *(item["file"] for item in pcbs),
    }
    assert {
        path.relative_to(output).as_posix()
        for path in output.rglob("*")
        if path.is_file()
    } == expected_files
    design = json.loads((output / manifest["design_json"]).read_text("utf-8"))
    graph = json.loads((output / graph_record["file"]).read_text("utf-8"))
    assert graph == design["compiled_schematic_graph"]
    canonical_graph = json.dumps(
        graph, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    facts = manifest["design_facts"]
    assert facts["backend"] == "kicad-monkey-native"
    assert facts["resource_profile"] == "design-facts-bounded-a1"
    assert re.fullmatch(r"[0-9a-f]{64}", facts["source_snapshot_sha256"])
    assert hashlib.sha256(canonical_graph).hexdigest() == facts[
        "compiled_schematic_graph_sha256"
    ]
    netlist_bytes = (output / manifest["netlist_kicad_sexpr"]).read_bytes()
    assert len(netlist_bytes) == facts["kicad_netlist_bytes"]
    assert hashlib.sha256(netlist_bytes).hexdigest() == facts["kicad_netlist_sha256"]
    netlist = parse_sexp(netlist_bytes.decode("utf-8"))
    design_block = next(
        row
        for row in netlist[1:]
        if isinstance(row, list) and row and row[0] == "design"
    )
    assert get_value(design_block, "source") == str(project.with_suffix(".kicad_sch"))
    assert get_value(design_block, "date") == ""
    assert get_value(design_block, "tool") == "kicad_cruncher"
    assert len(schematic) == 1
    assert {item["layer"] for item in pcbs} == {"F.Cu", "B.Cu"}
    assert all(
        "kicad_monkey.pcb.svg.enrichment.a0"
        in (output / item["file"]).read_text("utf-8")
        for item in pcbs
    )
    assert (output / manifest["readme"]).read_text("utf-8") == design_cmd._readme_text(
        input_file=Path(manifest["input"]),
        design_json=manifest["design_json"],
        compiled_schematic_graph=graph_record["file"],
        netlist_json=manifest["netlist_json"],
        netlist_kicad_sexpr=manifest["netlist_kicad_sexpr"],
        schematic_svgs=schematic,
        pcb_svgs=pcbs,
        manifest_file="design_review_manifest.json",
    )


def test_rust_design_aliases_publish_the_same_complete_transactional_bundle(
    tmp_path: Path,
) -> None:
    digests: list[dict[str, str]] = []
    for alias in ("design", "design-review", "dr"):
        output = tmp_path / alias
        completed = _run(
            [str(_RUST_EXE), alias, str(_PROJECT.resolve()), "-o", str(output)],
            timeout=120,
        )
        assert completed.returncode == 0, completed.stdout + completed.stderr
        assert completed.stderr == ""
        assert "Design review:" in completed.stdout
        _assert_rust_bundle(output)
        digests.append(_bundle_digest(output))
    assert digests[0] == digests[1] == digests[2]


@pytest.mark.parametrize(
    "source_suffix",
    (".kicad_pro", ".kicad_sch"),
    ids=("project-entrypoint", "adjacent-schematic-entrypoint"),
)
def test_rust_design_workflow_publishes_project_bus_alias_members_consistently(
    tmp_path: Path, source_suffix: str
) -> None:
    output = tmp_path / "project-bus-alias-review"
    source = _PROJECT_BUS_ALIAS_PROJECT.resolve().with_suffix(source_suffix)

    completed = _run(
        [str(_RUST_EXE), "design", str(source), "-o", str(output)],
        timeout=120,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert completed.stderr == ""
    manifest = json.loads((output / "design_review_manifest.json").read_text("utf-8"))
    Draft202012Validator(json.loads(_MANIFEST_SCHEMA.read_text("utf-8"))).validate(
        manifest
    )
    assert manifest["design_facts"]["backend"] == "kicad-monkey-native"
    design_json = json.loads((output / manifest["design_json"]).read_text("utf-8"))
    netlist_json = json.loads((output / manifest["netlist_json"]).read_text("utf-8"))
    graph = json.loads(
        (output / manifest["compiled_schematic_graph"]["file"]).read_text("utf-8")
    )
    sexpr = (output / manifest["netlist_kicad_sexpr"]).read_text("utf-8")

    assert graph == design_json["compiled_schematic_graph"]
    assert _json_net_terminals(netlist_json) == _PROJECT_BUS_ALIAS_EXPECTED
    assert _json_net_terminals(design_json) == _PROJECT_BUS_ALIAS_EXPECTED
    assert _sexpr_net_terminals(sexpr) == _PROJECT_BUS_ALIAS_EXPECTED
    assert netlist_json == KiCadDesign.from_file(source).to_netlist_json()
    assert len(manifest["schematic_svgs"]) == 2
    assert manifest["pcb_svgs"] == []


def test_rust_design_failure_preserves_the_previous_bundle(tmp_path: Path) -> None:
    output = tmp_path / "review"
    output.mkdir()
    marker = output / "previous.txt"
    marker.write_text("keep me", encoding="utf-8")
    missing = tmp_path / "missing.kicad_pro"

    completed = _run([str(_RUST_EXE), "design", str(missing), "-o", str(output)])

    assert completed.returncode == 1
    assert completed.stdout == ""
    assert "could not resolve design input" in completed.stderr
    assert marker.read_text("utf-8") == "keep me"
    assert list(tmp_path.glob(".kicad-cruncher-design-*")) == []


def test_rust_design_auto_detects_one_project_and_honors_no_indexes(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    source.mkdir()
    for suffix in (".kicad_pro", ".kicad_sch", ".kicad_pcb"):
        shutil.copy2(_PROJECT.with_suffix(suffix), source / f"hlr_test{suffix}")
    output = tmp_path / "review"

    completed = _run(
        [str(_RUST_EXE), "design", "--no-indexes", "-o", str(output)],
        timeout=120,
        cwd=source,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    _assert_rust_bundle(output, source / "hlr_test.kicad_pro")
    manifest = json.loads((output / "design_review_manifest.json").read_text("utf-8"))
    design = json.loads((output / manifest["design_json"]).read_text("utf-8"))
    assert "indexes" not in design


@pytest.mark.parametrize("source", (_PROJECT, _PROJECT.with_suffix(".kicad_sch")))
def test_rust_netlist_json_matches_the_python_design_oracle_exactly(
    source: Path,
) -> None:
    completed = _run([str(_RUST_NETLIST_ORACLE), str(source)])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    assert json.loads(completed.stdout) == KiCadDesign.from_file(source).to_netlist_json()


@pytest.mark.parametrize(
    "source", (_PROJECT.resolve(), _PROJECT.with_suffix(".kicad_sch").resolve())
)
@pytest.mark.parametrize("include_indexes", (True, False))
def test_rust_design_json_matches_the_python_oracle_exactly(
    source: Path,
    include_indexes: bool,
) -> None:
    arguments = [str(_RUST_DESIGN_ORACLE), str(source)]
    if not include_indexes:
        arguments.append("--no-indexes")
    completed = _run(arguments)

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    assert json.loads(completed.stdout) == KiCadDesign.from_file(source).to_json(
        include_indexes=include_indexes
    )


@pytest.mark.parametrize("source", _REPRESENTATIVE_PROJECTS)
def test_rust_design_json_matches_hierarchy_graphics_multi_unit_and_pnp_oracles(
    source: Path,
) -> None:
    source = source.resolve()
    completed = _run([str(_RUST_DESIGN_ORACLE), str(source)])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    assert json.loads(completed.stdout) == KiCadDesign.from_file(source).to_json()


def test_rust_design_json_matches_large_hierarchical_net_naming_oracle() -> None:
    source = _LARGE_HIERARCHY_PROJECT.resolve()
    completed = _run([str(_RUST_DESIGN_ORACLE), str(source), "--no-indexes"])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    assert json.loads(completed.stdout) == KiCadDesign.from_file(source).to_json(
        include_indexes=False
    )


@pytest.mark.parametrize(
    "source", (_PROJECT, _REPRESENTATIVE_PROJECTS[0], _LARGE_HIERARCHY_PROJECT)
)
def test_rust_schematic_instances_match_the_python_hierarchy_oracle(source: Path) -> None:
    source = source.resolve()
    completed = _run([str(_RUST_SCHEMATIC_INSTANCES_ORACLE), str(source)])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    design = KiCadDesign.from_file(source)
    graph = design.to_json(include_indexes=False)["compiled_schematic_graph"]
    pages = {
        row["source_identity"]["sch.source_key.source_record"]: row
        for row in graph["page_occurrences"]
    }
    expected: list[dict[str, object]] = []
    for instance in design.schematic_instances():
        instance_path = str(instance.sheet_instance_path or "")
        page = pages[f"instance-path:{instance_path}"]
        expected.append(
            {
                "instance_index": instance.instance_index,
                "sheet_number": instance.sheet_number,
                "sheet_count": instance.sheet_count,
                "source_path": Path(instance.source_path)
                .resolve()
                .relative_to(source.parent)
                .as_posix(),
                "sheet_name": instance.sheet_name,
                "sheet_path": instance.sheet_path,
                "sheet_path_uuids": instance.sheet_path_uuids,
                "sheet_instance_path": instance_path,
                "sheet_symbol_uid": instance.sheet_symbol_uid,
                "sheet_file": instance.sheet_file,
                "parent_sheet_path": instance.parent_sheet_path,
                "parent_sheet_path_uuids": instance.parent_sheet_path_uuids,
                "parent_sheet_instance_path": instance.parent_sheet_instance_path,
                "is_top_level": instance.is_top_level,
                "document_id": instance.ir_kwargs()["document_id"],
                "page_occurrence_ref": page["id"],
            }
        )
    actual = json.loads(completed.stdout)
    assert actual == expected, _first_json_difference(actual, expected)


@pytest.mark.parametrize(
    "source",
    (
        _PROJECT,
        _REPRESENTATIVE_PROJECTS[0],
        _REPRESENTATIVE_PROJECTS[1],
        _EMBEDDED_WORKSHEET_PROJECT,
    ),
)
@_WINDOWS_EXACT_RENDER_ORACLE
def test_rust_schematic_plot_documents_match_the_python_oracle_exactly(
    source: Path,
) -> None:
    source = source.resolve()
    completed = _run([str(_RUST_SCHEMATIC_PLOT_DOCUMENTS_ORACLE), str(source)])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    design = KiCadDesign.from_file(source)
    expected = [
        design.to_schematic_instance_ir(instance).to_dict()
        for instance in design.schematic_instances()
    ]
    actual = json.loads(completed.stdout)
    assert actual == expected, _first_json_difference(actual, expected)


@_WINDOWS_EXACT_RENDER_ORACLE
def test_rust_plot_document_uses_generic_embedded_font_family_and_style() -> None:
    source = _EMBEDDED_BERKELEY_PROJECT.resolve()
    completed = _run(
        [str(_RUST_SCHEMATIC_PLOT_DOCUMENTS_ORACLE), str(source), "--first"]
    )

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    design = KiCadDesign.from_file(source)
    expected = [
        design.to_schematic_instance_ir(design.schematic_instances()[0]).to_dict()
    ]
    actual = json.loads(completed.stdout)
    assert actual == expected, _first_json_difference(actual, expected)


def test_rust_plot_documents_load_custom_worksheet_and_missing_italic_face(
    tmp_path: Path,
) -> None:
    project = tmp_path / "styled.kicad_pro"
    project.write_text(
        json.dumps(
            {
                "schematic": {"page_layout_descr_file": "styled.kicad_wks"},
                "text_variables": {"LABEL": "Resolved"},
            }
        ),
        encoding="utf-8",
    )
    (tmp_path / "styled.kicad_sch").write_text(
        """(kicad_sch (version 20240101) (generator eeschema)
  (generator_version "10.0") (uuid "styled-root") (paper "A4")
  (lib_symbols)
  (sheet_instances (path "/styled-root" (page "1"))))
""",
        encoding="utf-8",
    )
    (tmp_path / "styled.kicad_wks").write_text(
        """(kicad_wks (version 20210606) (generator pl_editor)
  (setup (textsize 1.5 1.5) (linewidth 0.15) (textlinewidth 0.15)
    (left_margin 10) (right_margin 10) (top_margin 10) (bottom_margin 10))
  (tbtext "${LABEL}" (name "") (pos 20 20 ltcorner)
    (font (face "Definitely Missing Face") (size 1.5 1.5) italic)))
""",
        encoding="utf-8",
    )

    completed = _run([str(_RUST_SCHEMATIC_PLOT_DOCUMENTS_ORACLE), str(project)])

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    design = KiCadDesign.from_file(project)
    expected = [
        design.to_schematic_instance_ir(instance).to_dict()
        for instance in design.schematic_instances()
    ]
    assert json.loads(completed.stdout) == expected


@pytest.mark.parametrize("source", (_PROJECT, _REPRESENTATIVE_PROJECTS[0]))
def test_rust_schematic_base_svg_preserves_python_plot_identity(source: Path) -> None:
    source = source.resolve()
    completed = _run(
        [str(_RUST_SCHEMATIC_BASE_SVGS_ORACLE), str(source), "--first"]
    )

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    design = KiCadDesign.from_file(source)
    expected = design.to_schematic_instance_ir(design.schematic_instances()[0]).to_dict()
    [actual] = json.loads(completed.stdout)
    assert actual["document_id"] == expected["document_id"]
    assert actual["metrics"]["records"] == len(expected["records"])
    assert actual["metrics"]["operations"] == expected["total_operations"]
    assert actual["metrics"]["svg_bytes"] == len(actual["svg"].encode("utf-8"))

    root = ET.fromstring(actual["svg"])
    assert root.attrib["viewBox"] == (
        f'0 0 {expected["canvas"]["width_nm"]} '
        f'{expected["canvas"]["height_nm"]}'
    )
    rendered_records = {
        element.attrib["id"]: (
            element.attrib.get("data-ref"),
            element.attrib.get("data-object-id"),
        )
        for element in root.iter()
        if "id" in element.attrib
    }
    for record in expected["records"]:
        assert rendered_records[record["uuid"]] == (
            record["kind"],
            record["object_id"],
        )


@pytest.mark.parametrize(
    ("source", "first_only"),
    ((_PROJECT, True), (_REPRESENTATIVE_PROJECTS[0], False)),
)
@_WINDOWS_EXACT_RENDER_ORACLE
def test_rust_schematic_review_svg_matches_python_enrichment_contract(
    source: Path,
    first_only: bool,
) -> None:
    from kicad_monkey.kicad_schematic_svg_enrichment import (
        schematic_record_svg_data_attrs,
        schematic_root_svg_attrs,
        schematic_svg_enrichment_payload,
    )

    source = source.resolve()
    command = [str(_RUST_SCHEMATIC_REVIEW_SVGS_ORACLE), str(source)]
    if first_only:
        command.append("--first")
    completed = _run(command)
    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    actual_artifacts = json.loads(completed.stdout)
    design = KiCadDesign.from_file(source)
    design_payload = design.to_json()
    graph = design_payload["compiled_schematic_graph"]
    instances = design.schematic_instances()
    if first_only:
        instances = instances[:1]
    assert len(actual_artifacts) == len(instances)
    for actual, instance in zip(actual_artifacts, instances, strict=True):
        ir = design.to_schematic_instance_ir(instance)
        expected_payload = schematic_svg_enrichment_payload(
            design_payload,
            source_path=instance.source_path or "",
            sheet_name=instance.sheet_name,
            sheet_path=instance.sheet_path,
            sheet_instance_path=instance.sheet_instance_path,
            profile="enriched",
            compiled_schematic_graph=graph,
            schematic_instance=instance,
            compiled_graph_artifact="../compiled_schematic_graph.json",
        )
        _assert_schematic_review_svg(
            actual,
            instance,
            ir,
            expected_payload,
            schematic_record_svg_data_attrs,
            schematic_root_svg_attrs,
        )


def _assert_schematic_review_svg(
    actual: dict[str, object],
    instance: object,
    ir: object,
    expected_payload: dict[str, object],
    record_attrs: object,
    root_attrs: object,
) -> None:

    root = ET.fromstring(actual["svg"])
    metadata = next(
        element
        for element in root.iter()
        if element.tag.endswith("metadata")
        and element.attrib.get("id") == "schematic-enrichment-a0"
    )
    assert json.loads(metadata.text or "") == expected_payload
    graph_view = expected_payload["compiled_schematic_graph_view"]
    expected_root_attrs = root_attrs(
        source_path=instance.source_path or "",
        sheet_name=instance.sheet_name,
        sheet_path=instance.sheet_path,
        profile="enriched",
        compiled_graph_view=graph_view,
    )
    expected_root_attrs["data-review-theme"] = (
        "kicad_cruncher.design_review.schematic_svg.a0"
    )
    for name, value in expected_root_attrs.items():
        assert root.attrib[name] == str(value)

    by_id = {
        element.attrib["id"]: element
        for element in root.iter()
        if element.attrib.get("id")
    }
    for record in ir.records:
        expected_attrs = record_attrs(record, record.operations)
        for name, value in expected_attrs.items():
            assert by_id[record.uuid].attrib[name] == str(value)
    colors = {
        value.upper()
        for element in root.iter()
        for name, value in element.attrib.items()
        if name in {"fill", "stroke"} and re.fullmatch(r"#[0-9A-Fa-f]{6}", value)
    }
    assert colors <= {"#000000", "#FFFFFF"}
    assert actual["page_occurrence_ref"] == graph_view["page_occurrence_ref"]
    assert actual["graph_link_count"] == len(graph_view["graphical_artifact_link_refs"])
    assert actual["resolved_svg_identity_count"] == len(
        graph_view["element_to_graphical_artifact_link_refs"]
    )


@pytest.mark.parametrize(
    "source", (_PROJECT, _REPRESENTATIVE_PROJECTS[0], _EMBEDDED_BERKELEY_PROJECT)
)
@_WINDOWS_EXACT_RENDER_ORACLE
def test_rust_pcb_review_svg_matches_python_enrichment_contract(source: Path) -> None:
    from kicad_cruncher.kicad_cruncher_cmd_design import (
        _cached_pcb_review_svg_text,
        _pcb_copper_layers,
        _style_pcb_review_svg,
    )
    from kicad_cruncher.kicad_cruncher_native_physical import NativePhysicalProvider
    from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
        PcbSvgCompositionRenderCache,
    )

    source = source.resolve()
    completed = _run(
        [str(_RUST_PCB_REVIEW_SVGS_ORACLE), str(source)], timeout=120
    )
    assert completed.returncode == 0, completed.stderr
    assert completed.stderr == ""
    actual_artifacts = json.loads(completed.stdout)
    design = KiCadDesign.from_file(source)
    native = _WORKSPACE / "target" / "debug" / (
        "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
    )
    cache = PcbSvgCompositionRenderCache(
        design.pcb,
        physical_provider=NativePhysicalProvider(executable=native),
    )
    layers = _pcb_copper_layers(design.pcb)
    assert [artifact["layer"] for artifact in actual_artifacts] == layers
    for actual, layer in zip(actual_artifacts, layers, strict=True):
        expected_base = _cached_pcb_review_svg_text(design.pcb, cache, layer)
        expected_svg, expected_holes = _style_pcb_review_svg(expected_base, layer)
        expected = ET.fromstring(expected_svg)
        rendered = ET.fromstring(actual["svg"])
        assert actual["included_layers"] == [layer, "Edge.Cuts"]
        assert actual["drill_slot_record_count"] == expected_holes
        actual_root_attrs = dict(rendered.attrib)
        expected_root_attrs = dict(expected.attrib)
        actual_view_box = [float(value) for value in actual_root_attrs.pop("viewBox").split()]
        expected_view_box = [
            float(value) for value in expected_root_attrs.pop("viewBox").split()
        ]
        actual_width = float(actual_root_attrs.pop("width").removesuffix("mm"))
        expected_width = float(expected_root_attrs.pop("width").removesuffix("mm"))
        assert actual_root_attrs == expected_root_attrs
        assert actual_view_box == pytest.approx(expected_view_box, abs=0.001)
        assert actual_width == pytest.approx(expected_width, abs=0.001)
        expected_metadata = next(
            element for element in expected if element.tag.endswith("metadata")
        )
        actual_metadata = next(
            element for element in rendered if element.tag.endswith("metadata")
        )
        actual_payload = json.loads(actual_metadata.text or "")
        expected_payload = json.loads(expected_metadata.text or "")
        actual_bbox = actual_payload["board"].pop("bbox_mm")
        expected_bbox = expected_payload["board"].pop("bbox_mm")
        assert actual_bbox == pytest.approx(expected_bbox, abs=0.001)
        assert [value / 1_000_000 for value in actual["viewport_bounds_nm"]] == pytest.approx(
            expected_bbox, abs=0.001
        )
        assert actual_payload == expected_payload, _first_json_difference(
            actual_payload, expected_payload
        )
        expected_ids = [
            element.attrib["id"] for element in expected.iter() if element.attrib.get("id")
        ]
        actual_ids = [
            element.attrib["id"] for element in rendered.iter() if element.attrib.get("id")
        ]
        assert not [name for name, count in Counter(expected_ids).items() if count != 1]
        assert not [name for name, count in Counter(actual_ids).items() if count != 1]
        expected_by_id = {
            element.attrib["id"]: element
            for element in expected.iter()
            if element.attrib.get("id")
        }
        actual_by_id = {
            element.attrib["id"]: element
            for element in rendered.iter()
            if element.attrib.get("id")
        }
        assert actual_by_id.keys() == expected_by_id.keys(), (
            actual_by_id.keys() ^ expected_by_id.keys()
        )
        for element_id, expected_element in expected_by_id.items():
            assert actual_by_id[element_id].attrib == expected_element.attrib, (
                element_id,
                actual_by_id[element_id].attrib,
                expected_element.attrib,
            )
            # Native Rust and Python may encode the same filled geometry as
            # polygons or paths. Compare the themed colors within every
            # exact-bound source record instead of renderer-specific tags.
            expected_colors = {
                value
                for child in expected_element.iter()
                for name, value in child.attrib.items()
                if name in {"fill", "stroke"} and value != "none"
            }
            actual_colors = {
                value
                for child in actual_by_id[element_id].iter()
                for name, value in child.attrib.items()
                if name in {"fill", "stroke"} and value != "none"
            }
            assert actual_colors == expected_colors, element_id
            actual_geometry = _svg_record_geometry(actual_by_id[element_id])
            expected_geometry = _svg_record_geometry(expected_element)
            # Equivalent curve/path tessellation can retain a slightly
            # different number of unique samples. A 1% per-record ceiling
            # still detects missing, moved, or broad topology drift without
            # coupling the oracle to renderer-specific contour segmentation.
            assert actual_geometry[0] == pytest.approx(
                expected_geometry[0], rel=0.01, abs=2
            ), element_id
            assert actual_geometry[1] == pytest.approx(expected_geometry[1], abs=0.001), element_id

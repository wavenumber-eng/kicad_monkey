"""Behavioral oracle checks shared by the Python and Rust design CLIs."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

import pytest
from kicad_monkey import KiCadDesign

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


def _run(command: list[str]) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env.update({"NO_COLOR": "1", "PYTHONUTF8": "1"})
    return subprocess.run(
        command,
        cwd=_PACKAGE_ROOT,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
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

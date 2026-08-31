"""KiCad 10 project-level bus alias regression fixture.

The authored fixture follows KiCad's issue24220 QA topology at a smaller
scale: two terminal-bearing bus members cross a hierarchical sheet boundary.
Unlike legacy KiCad schematics, neither ``.kicad_sch`` contains a
``(bus_alias ...)`` form; ``CTRL`` exists only in the adjacent project JSON.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

import pytest

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import (
    KiCadDesign,
    KiCadSchematic,
    compile_design_netlist,
    find_all_elements,
    find_element,
    get_value,
    parse_sexp,
)


CASE_ROOT = Path(__file__).parents[1] / "cases" / "project_bus_alias_hierarchy"
INPUT_ROOT = CASE_ROOT / "input"
REFERENCE_ROOT = CASE_ROOT / "reference_output"
PROJECT = INPUT_ROOT / "project_bus_alias_hierarchy.kicad_pro"
SCHEMATIC = INPUT_ROOT / "project_bus_alias_hierarchy.kicad_sch"
CHILD = INPUT_ROOT / "member_sheet.kicad_sch"
ORACLE = REFERENCE_ROOT / "project_bus_alias_hierarchy.xml"


def _xml_terminal_map(path: Path) -> dict[str, set[tuple[str, str]]]:
    root = ET.parse(path).getroot()
    return {
        str(net.attrib["name"]): {
            (str(node.attrib["ref"]), str(node.attrib["pin"]))
            for node in net.findall("node")
        }
        for net in root.findall("./nets/net")
    }


def _json_terminal_map(payload: dict) -> dict[str, set[tuple[str, str]]]:
    return {
        str(net["name"]): {
            (str(terminal["designator"]), str(terminal["pin"]))
            for terminal in net["terminals"]
        }
        for net in payload["nets"]
    }


def _sexpr_terminal_map(text: str) -> dict[str, set[tuple[str, str]]]:
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


EXPECTED = {
    "/CTRL_A": {("TP1", "1"), ("TP101", "1")},
    "/CTRL_B": {("TP2", "1"), ("TP102", "1")},
}


def test_fixture_defines_alias_only_in_project_json() -> None:
    design = KiCadDesign.from_project_file(PROJECT)

    assert design.project is not None
    assert design.project.bus_aliases == {"CTRL": ["CTRL_A", "CTRL_B"]}
    assert KiCadSchematic(SCHEMATIC).bus_aliases == []
    assert KiCadSchematic(CHILD).bus_aliases == []


def test_reviewed_kicad_oracle_connects_both_members_across_hierarchy() -> None:
    assert _xml_terminal_map(ORACLE) == EXPECTED


def test_python_design_matches_kicad_project_alias_oracle() -> None:
    design = KiCadDesign.from_project_file(PROJECT)

    assert _json_terminal_map(design.to_netlist_json()) == EXPECTED
    payload = design.to_json()
    assert _json_terminal_map(payload) == EXPECTED
    assert _sexpr_terminal_map(design.to_kicad_netlist_sexpr(date="")) == EXPECTED
    assert payload["indexes"]["net_to_components"] == {
        "/CTRL_A": ["TP1", "TP101"],
        "/CTRL_B": ["TP102", "TP2"],
    }
    assert payload["indexes"]["component_to_nets"] == {
        "TP1": ["/CTRL_A"],
        "TP101": ["/CTRL_A"],
        "TP102": ["/CTRL_B"],
        "TP2": ["/CTRL_B"],
    }
    assert payload["indexes"]["sheet_svg_to_nets"] == {
        "/": {
            "4309841b-e755-56a6-9071-cbad92612b5d": ["/CTRL_A"],
            "941c7699-b393-57fa-a869-c44588932d6c": ["/CTRL_B"],
        },
        "/200c9cfe-1b08-56b8-850b-f77c1e9e7f43/": {
            "c0c6117b-8678-53b5-aa78-2fb1701da785": ["/CTRL_B"],
            "ed8d566b-e932-55f2-b143-6ec464e2da7e": ["/CTRL_A"],
        },
    }
    local_nets = payload["compiled_schematic_graph"]["local_net_occurrences"]
    assert {
        (
            row["source_identity"]["sch.source_key.source_path"],
            row["qualified_name"],
        )
        for row in local_nets
        if row["qualified_name"] in EXPECTED
    } == {
        ("/", "/CTRL_A"),
        ("/", "/CTRL_B"),
        ("/200c9cfe-1b08-56b8-850b-f77c1e9e7f43/", "/CTRL_A"),
        ("/200c9cfe-1b08-56b8-850b-f77c1e9e7f43/", "/CTRL_B"),
    }


def test_project_alias_view_and_raw_mutation_remain_one_authoritative_model() -> None:
    design = KiCadDesign.from_project_file(PROJECT)
    assert design.project is not None

    detached = design.project.bus_aliases
    detached["CTRL"] = []
    design.refresh_netlist()
    assert _json_terminal_map(design.to_netlist_json()) == EXPECTED

    design.project.set_path("schematic.bus_aliases", {"CTRL": []})
    design.refresh_netlist()
    assert all(
        len(terminals) == 1
        for terminals in _json_terminal_map(design.to_netlist_json()).values()
    )
    assert '"bus_aliases": {' in design.project.to_text()


def test_project_origin_alias_cycle_fails_closed() -> None:
    design = KiCadDesign.from_project_file(PROJECT)
    assert design.project is not None
    design.project.set_path(
        "schematic.bus_aliases",
        {"CTRL": ["SECOND"], "SECOND": ["CTRL"]},
    )

    with pytest.raises(ValueError, match="bus alias cycle"):
        design.refresh_netlist()


def test_schematic_entrypoint_discovers_adjacent_project_aliases() -> None:
    design = KiCadDesign.from_file(SCHEMATIC)

    assert design.project is not None
    assert design.project.bus_aliases == {"CTRL": ["CTRL_A", "CTRL_B"]}
    assert _json_terminal_map(design.to_netlist_json()) == EXPECTED


def test_direct_compiler_accepts_project_alias_context() -> None:
    top = KiCadSchematic(SCHEMATIC)
    payload = {
        "nets": [
            {
                "name": net.name,
                "terminals": [
                    {"designator": terminal.designator, "pin": terminal.pin}
                    for terminal in net.terminals
                ],
            }
            for net in compile_design_netlist(
                top,
                bus_aliases={"CTRL": ["CTRL_A", "CTRL_B"]},
            ).nets
        ]
    }

    assert _json_terminal_map(payload) == EXPECTED


@pytest.mark.skipif(
    resolve_kicad_cli(required_capability="any") is None,
    reason="kicad-cli not available for live project bus-alias oracle",
)
def test_live_kicad_cli_preserves_reviewed_oracle(tmp_path: Path) -> None:
    cli = resolve_kicad_cli(required_capability="any")
    assert cli is not None
    version = subprocess.run(
        [str(cli), "--version"],
        capture_output=True,
        text=True,
        check=False,
        env=kicad_cli_subprocess_env(cli),
        timeout=15,
    )
    match = re.search(r"\b(\d+)\.", version.stdout + version.stderr)
    if version.returncode != 0 or match is None or int(match.group(1)) < 10:
        pytest.skip("live project bus-alias oracle requires KiCad 10 or newer")
    live_input = tmp_path / "input"
    live_input.mkdir()
    for source in (PROJECT, SCHEMATIC, CHILD):
        shutil.copy2(source, live_input / source.name)
    live_schematic = live_input / SCHEMATIC.name
    output = tmp_path / "project_bus_alias_hierarchy.xml"
    completed = subprocess.run(
        [
            str(cli),
            "sch",
            "export",
            "netlist",
            "--format",
            "kicadxml",
            "--output",
            str(output),
            str(live_schematic),
        ],
        capture_output=True,
        text=True,
        check=False,
        env=kicad_cli_subprocess_env(cli),
        timeout=60,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    assert _xml_terminal_map(output) == EXPECTED

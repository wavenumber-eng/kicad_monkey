"""L3 contract checks for KiCad-native design/netlist JSON.

The native payloads should be KiCad-owned but close enough to
the established downstream design payload conventions that callers can make a
small source-CAD switch instead of a bespoke KiCad data shape.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from _suite_paths import TEST_CORPUS_ROOT
from kicad_monkey import KiCadDesign


_PUBLIC_CONTRACT_PROJECT = (
    Path("kicad")
    / "projects"
    / "canbob"
    / "input"
    / "CANBOB (MAGE-CANBOB-003).kicad_pro"
)


def _resolve_public_contract_project() -> Path | None:
    project_file = TEST_CORPUS_ROOT / _PUBLIC_CONTRACT_PROJECT
    return project_file if project_file.is_file() else None


_CANBOB_PRO = _resolve_public_contract_project()


@pytest.mark.skipif(_CANBOB_PRO is None, reason="canbob corpus project not present")
def test_public_design_json_uses_altium_shaped_kicad_contract():
    assert _CANBOB_PRO is not None
    payload = KiCadDesign.from_project_file(_CANBOB_PRO).to_json(include_indexes=True)

    assert payload["schema"] == "kicad_monkey.design.a0"
    assert payload["generator"] == "kicad_monkey"
    assert payload["project"]["filename"] == "CANBOB (MAGE-CANBOB-003).kicad_pro"
    assert "parameters" not in payload["project"]
    assert isinstance(payload["project"]["text_variables"], dict)
    assert payload["components"]
    assert payload["nets"]
    assert payload["sheets"]
    assert payload["pnp"]["units"] == "mm"
    assert payload["pnp"]["source_pcb"] == "CANBOB (MAGE-CANBOB-003).kicad_pcb"
    assert payload["pnp"]["placements"]

    first_component = payload["components"][0]
    assert {
        "designator",
        "svg_id",
        "value",
        "footprint",
        "library_ref",
        "description",
        "hierarchy",
        "classification",
        "parameters",
    }.issubset(first_component)

    first_net = payload["nets"][0]
    assert {
        "uid",
        "name",
        "auto_named",
        "source_sheets",
        "terminals",
        "graphical",
        "aliases",
        "endpoints",
    }.issubset(first_net)
    assert len(first_net["uid"]) == 12
    assert set(first_net["graphical"]) == {
        "wires",
        "junctions",
        "labels",
        "power_ports",
        "ports",
        "sheet_entries",
        "pins",
    }

    indexes = payload["indexes"]
    assert {
        "svg_to_component",
        "component_to_nets",
        "net_to_components",
    }.issubset(indexes)
    assert indexes["component_to_nets"]
    assert indexes["net_to_components"]
    assert indexes["svg_to_component"]
    assert indexes["svg_to_net"]
    assert indexes["svg_to_nets"]
    assert indexes["sheet_svg_to_nets"]
    assert indexes["net_to_graphics"]

    populated_graphics = {
        key
        for net in payload["nets"]
        for key, values in net["graphical"].items()
        if key != "pins" and values
    }
    assert {
        "wires",
        "junctions",
        "labels",
        "power_ports",
        "ports",
        "sheet_entries",
    }.issubset(populated_graphics)
    assert any(net["graphical"]["pins"] for net in payload["nets"])

    hierarchy = payload["schematic_hierarchy"]
    assert hierarchy["schema"] == "kicad_monkey.schematic_hierarchy.a0"
    assert hierarchy["documents"]
    assert hierarchy["sheet_symbols"]


@pytest.mark.skipif(_CANBOB_PRO is None, reason="canbob corpus project not present")
def test_public_kicad_netlist_json_uses_raw_netlist_contract():
    assert _CANBOB_PRO is not None
    design = KiCadDesign.from_project_file(_CANBOB_PRO)
    payload = design.to_kicad_netlist_json()

    assert payload["schema"] == "kicad_monkey.netlist.a0"
    assert payload["generator"] == "kicad_monkey"
    assert payload["components"]
    assert payload["nets"]
    assert payload["design"]["sheets"]
    assert payload["nets"][0]["terminals"]

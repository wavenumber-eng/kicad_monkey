"""L0 foundation tests for the KiCadDesign netlist API.

Covers ``KiCadDesign.to_netlist`` / ``to_kicad_netlist_sexpr`` /
``to_netlist_json`` / ``get_net`` / ``get_component`` / ``refresh_netlist``.

Tests are pure-unit and decoupled from full schematic compilation: the
underlying ``compile_design_netlist`` walk is already covered by L0_024-
028. Here we focus on the routing surface — caching, top-schematic
guard, KiCad-native JSON shape, and source-path threading into the kicadsexpr
emit.
"""

from __future__ import annotations

import re
from uuid import UUID

import pytest

from kicad_monkey import (
    KICAD_NETLIST_VERSION,
    KiCadDesign,
    KiCadDesignMetadata,
    KiCadDesignSheet,
    KiCadLibPart,
    KiCadLibPartPin,
    KiCadNet,
    KiCadNetEndpoint,
    KiCadNetlist,
    KiCadNetlistComponent,
    KiCadNetlistTerminal,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterRecord,
    render_ir_to_svg,
)
from kicad_monkey.kicad_design_json import kicad_netlist_to_json
from kicad_monkey.kicad_compiled_schematic_graph import (
    KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA,
    KiCadCompiledSchematicGraph,
    build_compiled_schematic_graph,
    validate_compiled_schematic_graph,
)
from kicad_monkey.kicad_compiled_schematic_graph_identity import (
    SchCompiledSchematicGraphIdentityAllocator,
    compiled_schematic_graph_design_scope,
)
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_netlist_model import KiCadNetClass
from kicad_monkey.kicad_sch_enums import PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_sheet import SchSheet, SchSheetProperty
from kicad_monkey.kicad_sch_symbol import SchSymbol
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_sym_pin import SymPin
from kicad_monkey.kicad_sym_property import SymProperty


_MIN_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-3333-4444-555555555555")
  (paper "A4")
  (title_block
    (title "DemoBoard")
    (date "2026-05-10")
    (rev "A")
    (company "ACME")
  )
)
"""


def _write_min_sch(path):
    path.write_text(_MIN_SCH_TEXT, encoding="utf-8")


def _svg_ids(svg: str) -> set[str]:
    return set(re.findall(r'\bid="([^"]+)"', svg))


def _make_synthetic_netlist() -> KiCadNetlist:
    """Build a small KiCadNetlist for routing tests."""
    return KiCadNetlist(
        components=[
            KiCadNetlistComponent(
                reference="R1",
                value="10k",
                footprint="Resistor_SMD:R_0402_1005Metric",
                libsource_lib="Device",
                libsource_part="R",
                libsource_description="Resistor",
                instance_uuid="r1-uuid",
            ),
            KiCadNetlistComponent(
                reference="C1",
                value="100n",
                libsource_lib="Device",
                libsource_part="C",
                libsource_description="Capacitor",
                instance_uuid="c1-uuid",
            ),
        ],
        libparts=[
            KiCadLibPart(
                lib="Device",
                part="R",
                pins=[
                    KiCadLibPartPin(number="1", name="~", pin_type="passive"),
                    KiCadLibPartPin(number="2", name="~", pin_type="passive"),
                ],
            ),
        ],
        nets=[
            KiCadNet(
                name="VCC",
                code=1,
                terminals=[
                    KiCadNetlistTerminal(designator="R1", pin="1"),
                    KiCadNetlistTerminal(designator="C1", pin="1"),
                ],
            ),
            KiCadNet(
                name="GND",
                code=2,
                terminals=[
                    KiCadNetlistTerminal(designator="R1", pin="2"),
                    KiCadNetlistTerminal(designator="C1", pin="2"),
                ],
            ),
        ],
        design_metadata=KiCadDesignMetadata(
            sheets=[KiCadDesignSheet(number=1, name="/", tstamps="/abc/")],
        ),
    )


# ---------------------------------------------------------------------------
# Top-schematic guard
# ---------------------------------------------------------------------------


def test_to_netlist_raises_when_no_top_schematic():
    design = KiCadDesign(project=None, schematics=[])
    with pytest.raises(ValueError, match="no top schematic"):
        design.to_netlist()


# ---------------------------------------------------------------------------
# Caching
# ---------------------------------------------------------------------------


def test_to_netlist_caches_result(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)

    n1 = design.to_netlist()
    n2 = design.to_netlist()
    assert n1 is n2  # same cached instance


def test_refresh_netlist_recomputes(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)

    n1 = design.to_netlist()
    n2 = design.refresh_netlist()
    assert n1 is not n2  # fresh instance after refresh
    # But the new instance is cached on subsequent calls.
    assert design.to_netlist() is n2


# ---------------------------------------------------------------------------
# get_net / get_component (routing)
# ---------------------------------------------------------------------------


def test_get_net_returns_named_net(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    vcc = design.get_net("VCC")
    assert vcc is not None
    assert vcc.name == "VCC"
    assert len(vcc.terminals) == 2


def test_get_net_returns_none_for_missing(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    assert design.get_net("DOES_NOT_EXIST") is None


def test_get_component_returns_by_reference(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    r1 = design.get_component("R1")
    assert r1 is not None
    assert r1.value == "10k"
    assert r1.footprint == "Resistor_SMD:R_0402_1005Metric"


def test_get_component_returns_none_for_missing(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    assert design.get_component("U99") is None


# ---------------------------------------------------------------------------
# to_kicad_netlist_sexpr
# ---------------------------------------------------------------------------


def test_to_kicad_netlist_sexpr_emits_versioned_envelope(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    text = design.to_kicad_netlist_sexpr(date="")
    assert text.startswith("(export")
    # Version is the locked constant — format_sexp puts each list on its
    # own line so the closing paren is on the next line.
    assert f'(version "{KICAD_NETLIST_VERSION}"' in text
    # Components and nets visible in the rendered text.
    assert '(ref "R1"' in text
    assert '(name "VCC"' in text


def test_to_kicad_netlist_sexpr_threads_source_path(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    text = design.to_kicad_netlist_sexpr(date="")
    # The schematic's filename should land inside (source "...").
    # Path separators are backslash-escaped by QuotedString on Windows
    # so we match the stem rather than the full literal path.
    assert "demo.kicad_sch" in text
    assert "(source " in text


def test_to_kicad_netlist_sexpr_respects_tool_and_date(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    text = design.to_kicad_netlist_sexpr(tool="custom-cli", date="2026-01-01")
    assert '(tool "custom-cli"' in text
    assert '(date "2026-01-01"' in text


# ---------------------------------------------------------------------------
# to_netlist_json
# ---------------------------------------------------------------------------


def test_to_netlist_json_returns_kicad_native_dict(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()

    payload = design.to_netlist_json()
    assert payload["schema"] == "kicad_monkey.netlist.a0"
    assert payload["generator"] == "kicad_monkey"
    assert payload["design"]["tool"] == "kicad_monkey"

    # Components carry through.
    refs = [c["designator"] for c in payload["components"]]
    assert refs == ["R1", "C1"]

    # Nets carry through.
    net_names = [n["name"] for n in payload["nets"]]
    assert net_names == ["VCC", "GND"]


def test_to_netlist_json_includes_kicad_net_classes(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = _make_synthetic_netlist()
    design._netlist.net_classes = [
        KiCadNetClass(name="Default"),
        KiCadNetClass(name="Power"),
    ]
    design._netlist.nets[0].net_class = "Power"
    design._netlist.nets[1].net_class = "Default"

    payload = design.to_netlist_json()
    by_name = {row["name"]: row for row in payload["net_classes"]}
    assert set(by_name) == {"Default", "Power"}
    assert by_name["Power"]["nets"] == ["VCC"]
    assert by_name["Default"]["nets"] == ["GND"]


def test_design_json_pin_count_counts_unique_pins_once(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = KiCadNetlist(
        components=[
            KiCadNetlistComponent(reference="U1"),
            KiCadNetlistComponent(reference="U2"),
        ],
        nets=[
            KiCadNet(
                name="SIG_A",
                terminals=[
                    KiCadNetlistTerminal(designator="U1", pin="1"),
                    KiCadNetlistTerminal(designator="U1", pin="1"),
                    KiCadNetlistTerminal(designator="U1", pin="2"),
                ],
            ),
            KiCadNet(
                name="SIG_B",
                terminals=[
                    KiCadNetlistTerminal(designator="U1", pin="2"),
                    KiCadNetlistTerminal(designator="U1", pin=""),
                    KiCadNetlistTerminal(designator="u1", pin="3"),
                ],
            ),
        ],
    )

    payload = design.to_json(include_indexes=False)
    by_ref = {component["designator"]: component for component in payload["components"]}

    assert by_ref["U1"]["classification"]["pin_count"] == 3
    assert by_ref["U2"]["classification"]["pin_count"] == 0


def test_design_json_pin_count_is_case_sensitive(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = KiCadNetlist(
        components=[
            KiCadNetlistComponent(reference="U1"),
            KiCadNetlistComponent(reference="u1"),
        ],
        nets=[
            KiCadNet(
                name="SIG",
                terminals=[
                    KiCadNetlistTerminal(designator="U1", pin="1"),
                    KiCadNetlistTerminal(designator="u1", pin="1"),
                    KiCadNetlistTerminal(designator="u1", pin="2"),
                ],
            ),
        ],
    )

    payload = design.to_json(include_indexes=False)
    by_ref = {component["designator"]: component for component in payload["components"]}

    assert by_ref["U1"]["classification"]["pin_count"] == 1
    assert by_ref["u1"]["classification"]["pin_count"] == 2


def test_design_json_pin_count_preserves_empty_designator_exact_match(tmp_path):
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = KiCadNetlist(
        components=[
            KiCadNetlistComponent(reference=""),
        ],
        nets=[
            KiCadNet(
                name="SIG",
                terminals=[
                    KiCadNetlistTerminal(designator="", pin="1"),
                    KiCadNetlistTerminal(designator="", pin="1"),
                    KiCadNetlistTerminal(designator="", pin="2"),
                ],
            ),
        ],
    )

    payload = design.to_json(include_indexes=False)

    assert payload["components"][0]["designator"] == ""
    assert payload["components"][0]["classification"]["pin_count"] == 2


def test_kicad_netlist_json_pin_endpoints_keep_source_pin_identity():
    netlist = KiCadNetlist(
        components=[
            KiCadNetlistComponent(reference="U1", instance_uuid="symbol-uuid"),
        ],
        nets=[
            KiCadNet(
                name="SIG",
                endpoints=[
                    KiCadNetEndpoint(
                        endpoint_id="port:hier-uuid",
                        role="port",
                        element_id="hier-uuid",
                        object_id="hier-uuid",
                        name="SIG",
                        source_sheet="/",
                        connection_point=(10000, 20000),
                    )
                ],
                terminals=[
                    KiCadNetlistTerminal(
                        designator="U1",
                        pin="5",
                        pin_name="GPIO",
                        pin_type="bidirectional",
                        sheet_path="/",
                        source_pin_id="pin-uuid",
                        svg_id="pin-uuid",
                    )
                ],
            )
        ],
    )

    payload = kicad_netlist_to_json(netlist)

    pin_ref = payload["nets"][0]["graphical"]["pins"][0]
    assert pin_ref == {
        "designator": "U1",
        "pin": "5",
        "svg_id": "pin-uuid",
    }
    endpoints = {
        endpoint["endpoint_id"]: endpoint
        for endpoint in payload["nets"][0]["endpoints"]
    }
    semantic_endpoint = endpoints["port:hier-uuid"]
    assert semantic_endpoint["role"] == "port"
    assert semantic_endpoint["element_id"] == "hier-uuid"
    assert semantic_endpoint["object_id"] == "hier-uuid"
    assert semantic_endpoint["name"] == "SIG"
    assert semantic_endpoint["connection_point"] == {
        "x": 1.0,
        "y": 2.0,
        "units": "mm",
    }
    endpoint = endpoints["pin:U1:5"]
    assert endpoint["endpoint_id"] == "pin:U1:5"
    assert endpoint["element_id"] == "pin-uuid"
    assert endpoint["object_id"] == "pin-uuid"
    assert endpoint["name"] == "GPIO"
    assert endpoint["pin_type"] == "BIDIRECTIONAL"


def test_schematic_json_svg_ids_resolve_to_rendered_svg_groups(tmp_path):
    netlist = KiCadNetlist(
        components=[
            KiCadNetlistComponent(reference="U1", instance_uuid="symbol-uuid"),
        ],
        nets=[
            KiCadNet(
                name="SIG",
                graphical={
                    "wires": ["wire-uuid"],
                    "labels": ["label-uuid"],
                },
                endpoints=[
                    KiCadNetEndpoint(
                        endpoint_id="label:label-uuid",
                        role="label",
                        element_id="label-uuid",
                        object_id="label-uuid",
                        name="SIG",
                        source_sheet="/",
                    )
                ],
                terminals=[
                    KiCadNetlistTerminal(
                        designator="U1",
                        pin="1",
                        pin_name="IN",
                        pin_type="input",
                        sheet_path="/",
                        source_pin_id="pin-uuid",
                        svg_id="pin-uuid",
                    )
                ],
            )
        ],
    )
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)
    design._netlist = netlist
    payload = design.to_json(include_indexes=True)
    doc = KiCadPlotterDocument(
        source_kind="SCH",
        canvas={"width_nm": 100_000_000, "height_nm": 100_000_000},
        records=[
            KiCadPlotterRecord(
                uuid="symbol-uuid",
                kind="symbol_instance",
                object_id="Device:U",
                operations=[
                    KiCadPlotterOp.start_block(
                        label="pin-uuid",
                        data_uuid="pin-uuid",
                        data_ref="symbol_pin",
                        object_id="pin-uuid",
                    ),
                    KiCadPlotterOp.circle(
                        cx=10_000_000,
                        cy=10_000_000,
                        diameter_nm=1_000_000,
                    ),
                    KiCadPlotterOp.end_block(),
                ],
            ),
            KiCadPlotterRecord(
                uuid="wire-uuid",
                kind="wire",
                object_id="wire-uuid",
                operations=[
                    KiCadPlotterOp.thick_segment(
                        start_x=0,
                        start_y=10_000_000,
                        end_x=20_000_000,
                        end_y=10_000_000,
                        width_nm=100_000,
                    )
                ],
            ),
            KiCadPlotterRecord(
                uuid="label-uuid",
                kind="label",
                object_id="label-uuid",
                operations=[
                    KiCadPlotterOp.text(
                        x=20_000_000,
                        y=10_000_000,
                        text="SIG",
                        size_x_nm=1_270_000,
                        size_y_nm=1_270_000,
                    )
                ],
            ),
        ],
    )
    ids = _svg_ids(render_ir_to_svg(doc))

    component_id = payload["components"][0]["svg_id"]
    assert component_id == "symbol-uuid"
    assert component_id in ids

    net = payload["nets"][0]
    assert net["graphical"]["wires"] == ["wire-uuid"]
    assert net["graphical"]["labels"] == ["label-uuid"]
    assert net["graphical"]["pins"] == [
        {"designator": "U1", "pin": "1", "svg_id": "pin-uuid"}
    ]

    linked_ids = {
        *net["graphical"]["wires"],
        *net["graphical"]["labels"],
        *(pin["svg_id"] for pin in net["graphical"]["pins"]),
        *(endpoint["element_id"] for endpoint in net["endpoints"]),
    }
    assert linked_ids <= ids


# ---------------------------------------------------------------------------
# Empty schematic integration smoke test
# ---------------------------------------------------------------------------


def test_empty_schematic_produces_empty_netlist(tmp_path):
    """An empty schematic should compile cleanly with no nets/components."""
    sch = tmp_path / "demo.kicad_sch"
    _write_min_sch(sch)
    design = KiCadDesign.from_schematic_file(sch)

    netlist = design.to_netlist()
    assert isinstance(netlist, KiCadNetlist)
    assert netlist.nets == []
    assert netlist.components == []
    assert netlist.libparts == []


def test_compiled_graph_keeps_reused_off_board_occurrences_and_stable_ids():
    pin = SymPin(
        electrical_type=PinElectricalType.PASSIVE,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        number="1",
        name="IN",
    )
    library = LibSymbol(
        name="Device:R",
        subsymbols=[LibSubSymbol(name="Device:R_1_0", unit=1, pins=[pin])],
    )
    symbol = SchSymbol(lib_id="Device:R", uuid="symbol-source", on_board=False)
    symbol.properties = [
        SymProperty(key="Reference", value="R1"),
        SymProperty(key="Value", value="10k"),
    ]
    child = KiCadSchematic()
    child.uuid = "child-source"
    child.source_path = "C:/portable/child.kicad_sch"
    child.lib_symbols.append(library)
    child.symbols.append(symbol)

    root = KiCadSchematic()
    root.uuid = "root-source"
    root.source_path = "C:/portable/demo.kicad_sch"
    for name, uuid in (("A", "placement-a"), ("B", "placement-b")):
        sheet = SchSheet(uuid=uuid, on_board=False)
        sheet.properties = [
            SchSheetProperty(key="Sheetname", value=name),
            SchSheetProperty(key="Sheetfile", value="child.kicad_sch"),
        ]
        root.sheets.append(sheet)
    root.sub_schematics["child.kicad_sch"] = child
    design = KiCadDesign(schematics=[root])

    first = build_compiled_schematic_graph(design).to_json()
    second = build_compiled_schematic_graph(design).to_json()

    assert first == second
    assert first["schema"] == KICAD_COMPILED_SCHEMATIC_GRAPH_SCHEMA
    assert len(first["unit_definitions"]) == 2
    assert len(first["page_occurrences"]) == 3
    assert len(first["hierarchy_occurrences"]) == 2
    assert len(first["component_occurrences"]) == 2
    assert len(first["local_net_occurrences"]) == 2
    assert {
        row["source_identity"]["sch.source_key.source_path"]
        for row in first["unit_definitions"]
    } == {"demo.kicad_sch", "child.kicad_sch"}
    component_ids = {row["id"] for row in first["component_occurrences"]}
    assert len(component_ids) == 2
    assert all(UUID(row_id).version == 7 for row_id in component_ids)
    component_links = [
        row
        for row in first["graphical_artifact_links"]
        if row["target_type"] == "sch.component_occurrence"
    ]
    terminal_links = [
        row
        for row in first["graphical_artifact_links"]
        if row["target_type"] == "sch.terminal_occurrence"
    ]
    assert len(component_links) == 2
    assert len(terminal_links) == 2
    assert {row["element_id"] for row in component_links} == {"symbol-source"}
    assert {row["page_occurrence_ref"] for row in component_links} == {
        row["page_occurrence_ref"] for row in first["component_occurrences"]
    }
    validate_compiled_schematic_graph(first)
    assert KiCadCompiledSchematicGraph.from_json(first).to_json() == first


def test_compiled_graph_disambiguates_nested_hierarchy_inside_reused_parent(
    tmp_path,
):
    leaf = KiCadSchematic()
    leaf.uuid = "leaf-source"
    leaf.source_path = tmp_path / "leaf.kicad_sch"

    parent = KiCadSchematic()
    parent.uuid = "parent-source"
    parent.source_path = tmp_path / "parent.kicad_sch"
    nested = SchSheet(uuid="leaf-placement")
    nested.properties = [
        SchSheetProperty(key="Sheetname", value="leaf"),
        SchSheetProperty(key="Sheetfile", value="leaf.kicad_sch"),
    ]
    parent.sheets.append(nested)
    parent.sub_schematics["leaf.kicad_sch"] = leaf

    root = KiCadSchematic()
    root.uuid = "root-source"
    root.source_path = tmp_path / "root.kicad_sch"
    parent_a_placement = SchSheet(uuid="parent-a")
    parent_a_placement.properties = [
        SchSheetProperty(key="Sheetname", value="A"),
        SchSheetProperty(key="Sheetfile", value="parent.kicad_sch"),
    ]
    root.sheets.append(parent_a_placement)
    root.sub_schematics["parent.kicad_sch"] = parent

    design = KiCadDesign(schematics=[root])
    before = build_compiled_schematic_graph(design).to_json()
    before_nested = next(
        row
        for row in before["unit_occurrences"]
        if row["source_identity"].get("sch.source_key.source_uuid") == "leaf-placement"
    )

    parent_b_placement = SchSheet(uuid="parent-b")
    parent_b_placement.properties = [
        SchSheetProperty(key="Sheetname", value="B"),
        SchSheetProperty(key="Sheetfile", value="parent.kicad_sch"),
    ]
    root.sheets.append(parent_b_placement)

    graph = build_compiled_schematic_graph(design).to_json()
    nested_occurrences = [
        row
        for row in graph["unit_occurrences"]
        if row["source_identity"].get("sch.source_key.source_uuid") == "leaf-placement"
    ]

    assert len(graph["unit_occurrences"]) == 5
    assert len(nested_occurrences) == 2
    assert len({row["id"] for row in nested_occurrences}) == 2
    surviving_nested = next(
        row
        for row in nested_occurrences
        if row["source_identity"]["sch.source_key.source_path"]
        == before_nested["source_identity"]["sch.source_key.source_path"]
    )
    assert surviving_nested["id"] == before_nested["id"]

    # Occurrence paths retain the released a0 unowned address without depending
    # on how many siblings currently share a placement UUID.
    parent_a = next(
        row
        for row in graph["unit_occurrences"]
        if row["source_identity"].get("sch.source_key.source_uuid") == "parent-a"
    )
    parent_a_page = next(
        row
        for row in graph["page_occurrences"]
        if row["unit_occurrence_ref"] == parent_a["id"]
    )
    legacy_allocator = SchCompiledSchematicGraphIdentityAllocator(
        design_scope=compiled_schematic_graph_design_scope(
            source_cad="kicad",
            project={"filename": "root.kicad_sch"},
        )
    )
    legacy_unit_ref = legacy_allocator.allocate_source(
        object_type="sch.unit_occurrence",
        source_identity=parent_a["source_identity"],
    )
    legacy_page_ref = legacy_allocator.allocate_source(
        object_type="sch.page_occurrence",
        source_identity=parent_a_page["source_identity"],
        owner_refs=(legacy_unit_ref,),
    )
    assert parent_a["id"] == legacy_unit_ref
    assert parent_a_page["id"] == legacy_page_ref
    validate_compiled_schematic_graph(graph)


def test_compiled_graph_omits_hidden_and_ambiguous_stacked_pin_selectors():
    hidden = SymPin(
        electrical_type=PinElectricalType.POWER_IN,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        number="1",
        name="VCC",
        hide=True,
    )
    stacked = SymPin(
        electrical_type=PinElectricalType.PASSIVE,
        graphic_style=PinGraphicStyle.LINE,
        at_x=2.54,
        at_y=0.0,
        number="[2,4]",
        name="OSC1",
    )
    library = LibSymbol(
        name="Device:Mixed",
        subsymbols=[
            LibSubSymbol(name="Device:Mixed_1_0", unit=1, pins=[hidden, stacked])
        ],
    )
    symbol = SchSymbol(lib_id="Device:Mixed", uuid="mixed-symbol")
    symbol.properties = [
        SymProperty(key="Reference", value="U1"),
        SymProperty(key="Value", value="Mixed"),
    ]
    schematic = KiCadSchematic()
    schematic.uuid = "mixed-root"
    schematic.source_path = "C:/portable/mixed.kicad_sch"
    schematic.lib_symbols.append(library)
    schematic.symbols.append(symbol)

    graph = build_compiled_schematic_graph(
        KiCadDesign(schematics=[schematic])
    ).to_json()

    assert len(graph["terminal_occurrences"]) == 3
    assert {
        (row["name"], row["pin_designator"]) for row in graph["terminal_occurrences"]
    } == {("VCC", "1"), ("OSC1_2", "2"), ("OSC1_4", "4")}
    assert [
        row
        for row in graph["graphical_artifact_links"]
        if row["target_type"] == "sch.terminal_occurrence"
    ] == []
    assert [
        row["element_id"]
        for row in graph["graphical_artifact_links"]
        if row["target_type"] == "sch.component_occurrence"
    ] == ["mixed-symbol"]
    validate_compiled_schematic_graph(graph)


def test_compiled_graph_omits_zero_length_power_pin_without_visible_text():
    pin = SymPin(
        electrical_type=PinElectricalType.POWER_IN,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        length=0.0,
        number="1",
        name="~",
    )
    library = LibSymbol(
        name="power:+3V3",
        power=True,
        pin_names_hide=True,
        pin_numbers_hide=True,
        subsymbols=[
            LibSubSymbol(name="power:+3V3_1_0", unit=1, pins=[pin])
        ],
    )
    symbol = SchSymbol(lib_id="power:+3V3", uuid="power-symbol")
    symbol.properties = [
        SymProperty(key="Reference", value="#PWR01"),
        SymProperty(key="Value", value="+3V3"),
    ]
    schematic = KiCadSchematic()
    schematic.uuid = "power-root"
    schematic.source_path = "C:/portable/power.kicad_sch"
    schematic.lib_symbols.append(library)
    schematic.symbols.append(symbol)

    graph = build_compiled_schematic_graph(
        KiCadDesign(schematics=[schematic])
    ).to_json()

    assert len(graph["terminal_occurrences"]) == 1
    assert [
        row
        for row in graph["graphical_artifact_links"]
        if row["target_type"] == "sch.terminal_occurrence"
    ] == []
    validate_compiled_schematic_graph(graph)

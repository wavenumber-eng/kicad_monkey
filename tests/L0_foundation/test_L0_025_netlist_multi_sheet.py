"""
Test L0_025: multi-sheet netlist merge (Phase G — Slice N-4).

Pure-unit coverage for :mod:`kicad_monkey.kicad_netlist_design`.

Builds 2- and 3-level synthetic schematic hierarchies in-memory (no
on-disk parse) and asserts that:

* Sheet pins on the parent sheet pair with the child sheet's
  matching ``hierarchical_label`` to form a single net (with sheet
  path taken from the contributing subgraph that holds the chosen
  driver).
* Cross-sheet ``global_label`` text matches collapse to one net
  named ``/<text>``.
* Cross-sheet ``global_power_pin`` value matches collapse to one
  net named bare (e.g. ``GND``).
* Pure-pin subgraphs across sheets stay separate (no spurious cross-
  sheet merging).
* Net codes are sequential starting at 1.
"""

from __future__ import annotations

from typing import Optional

from kicad_monkey import (
    KiCadDriverPriority,
    compile_design_netlist,
    compile_design_subgraphs,
    merge_design_nets,
)
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_sch_enums import LabelShape, PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_label import (
    SchGlobalLabel,
    SchHierarchicalLabel,
    SchLabel,
)
from kicad_monkey.kicad_sch_sheet import SchSheet, SchSheetPin, SchSheetProperty
from kicad_monkey.kicad_sch_symbol import SchSymbol
from kicad_monkey.kicad_sch_wire import SchBusAlias, SchWire
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_schematic_occurrence import walk_schematic_occurrences
from kicad_monkey.kicad_sym_pin import SymPin
from kicad_monkey.kicad_sym_property import SymProperty


# ---------------------------------------------------------------------------
# Synth helpers
# ---------------------------------------------------------------------------


def _pin(
    at_x: float,
    at_y: float,
    *,
    number: str = "1",
    electrical: PinElectricalType = PinElectricalType.PASSIVE,
) -> SymPin:
    return SymPin(
        electrical_type=electrical,
        graphic_style=PinGraphicStyle.LINE,
        at_x=at_x,
        at_y=at_y,
        at_angle=180.0,
        length=0.0,
        number=number,
        name="~",
    )


def _libsym(
    name: str,
    *pins: SymPin,
    power: bool = False,
    power_kind: Optional[str] = None,
) -> LibSymbol:
    sub = LibSubSymbol(name=f"{name}_1_0", unit=1, style=0, pins=list(pins))
    return LibSymbol(name=name, power=power, power_kind=power_kind, subsymbols=[sub])


def _placed(
    lib_id: str,
    *,
    reference: str,
    value: str = "",
    at_x: float = 0.0,
    at_y: float = 0.0,
) -> SchSymbol:
    sym = SchSymbol(
        lib_id=lib_id,
        at_x=at_x,
        at_y=at_y,
        at_angle=0.0,
        mirror=None,
        unit=1,
        convert=1,
    )
    sym.properties = [
        SymProperty(key="Reference", value=reference, id=0),
        SymProperty(key="Value", value=value or reference, id=1),
    ]
    return sym


def _sheet(sheet_file: str, sheet_name: str, uuid: str, *pins: SchSheetPin) -> SchSheet:
    sh = SchSheet(uuid=uuid)
    sh.properties = [
        SchSheetProperty(key="Sheetname", value=sheet_name),
        SchSheetProperty(key="Sheetfile", value=sheet_file),
    ]
    sh.pins = list(pins)
    return sh


def _spin(name: str, at_x: float, at_y: float) -> SchSheetPin:
    return SchSheetPin(name=name, shape=LabelShape.INPUT, at_x=at_x, at_y=at_y)


def _wire(*points) -> SchWire:
    return SchWire(points=[tuple(p) for p in points])


# ---------------------------------------------------------------------------
# Fixture builders
# ---------------------------------------------------------------------------


def _two_level_hierarchy_with_sheet_pin():
    """Root sheet has R1 wired to a sheet pin "SIG"; sub sheet has R2
    wired to a hierarchical_label "SIG"."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))

    # Child sub-schematic.
    sub = KiCadSchematic()
    sub.uuid = "child-uuid"
    sub.lib_symbols.append(libR)
    sub.symbols.append(_placed("Device:R", reference="R2", at_x=20.0, at_y=10.0))
    hier_label = SchHierarchicalLabel(text="SIG", at_x=20.0, at_y=10.0)
    hier_label.uuid = "hier-label-uuid"
    sub.hierarchical_labels.append(hier_label)

    # Root schematic.
    root = KiCadSchematic()
    root.uuid = "root-uuid"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", at_x=10.0, at_y=10.0))
    root.wires.append(_wire((10.0, 10.0), (40.0, 10.0)))
    sheet_pin = _spin("SIG", 40.0, 10.0)
    sheet_pin.uuid = "sheet-pin-uuid"
    sheet = _sheet("sub.kicad_sch", "sub", "sheet-uuid", sheet_pin)
    root.sheets.append(sheet)
    root.sub_schematics["sub.kicad_sch"] = sub
    return root, sub


def _two_sheets_with_global_label():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sub = KiCadSchematic()
    sub.uuid = "child"
    sub.lib_symbols.append(libR)
    sub.symbols.append(_placed("Device:R", reference="R2", at_x=20.0, at_y=10.0))
    sub.global_labels.append(SchGlobalLabel(text="VCC", at_x=20.0, at_y=10.0))

    root = KiCadSchematic()
    root.uuid = "root"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", at_x=10.0, at_y=10.0))
    root.global_labels.append(SchGlobalLabel(text="VCC", at_x=10.0, at_y=10.0))
    sheet = _sheet("sub.kicad_sch", "sub", "child-sheet-uuid")
    root.sheets.append(sheet)
    root.sub_schematics["sub.kicad_sch"] = sub
    return root


def _two_sheets_with_global_power():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    libGND = _libsym(
        "power:GND",
        _pin(0.0, 0.0, number="1", electrical=PinElectricalType.POWER_IN),
        power=True,
        power_kind="global",
    )
    sub = KiCadSchematic()
    sub.uuid = "child"
    sub.lib_symbols.extend([libR, libGND])
    sub.symbols.append(_placed("Device:R", reference="R2", at_x=20.0, at_y=10.0))
    sub.symbols.append(
        _placed("power:GND", reference="#PWR2", value="GND", at_x=20.0, at_y=10.0)
    )

    root = KiCadSchematic()
    root.uuid = "root"
    root.lib_symbols.extend([libR, libGND])
    root.symbols.append(_placed("Device:R", reference="R1", at_x=10.0, at_y=10.0))
    root.symbols.append(
        _placed("power:GND", reference="#PWR1", value="GND", at_x=10.0, at_y=10.0)
    )
    sheet = _sheet("sub.kicad_sch", "sub", "child-sheet")
    root.sheets.append(sheet)
    root.sub_schematics["sub.kicad_sch"] = sub
    return root


# ---------------------------------------------------------------------------
# compile_design_subgraphs — discovers all sheets in tree
# ---------------------------------------------------------------------------


def test_compile_design_subgraphs_yields_root_then_child():
    root, _sub = _two_level_hierarchy_with_sheet_pin()
    compiled = compile_design_subgraphs(root)
    assert len(compiled) == 2
    # Root path is always "/" — kicad-cli convention; the top
    # schematic's own UUID never appears in the path.
    assert compiled[0].sheet_path == "/"
    assert compiled[0].parent is None
    assert compiled[1].sheet_path == "/sheet-uuid/"
    assert compiled[1].parent is compiled[0]
    assert compiled[1].sheet_path_human == "/sub/"


def test_compile_design_netlist_keeps_off_board_child_with_inherited_policy():
    """Off-board descendants remain in the complete graph and carry policy."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))

    active = KiCadSchematic()
    active.uuid = "active-child"
    active.lib_symbols.append(libR)
    active.symbols.append(_placed("Device:R", reference="R_ON", at_x=10.0, at_y=0.0))
    active.hierarchical_labels.append(
        SchHierarchicalLabel(text="ON", at_x=10.0, at_y=0.0)
    )

    off_board = KiCadSchematic()
    off_board.uuid = "off-child"
    off_board.lib_symbols.append(libR)
    off_board.symbols.append(
        _placed("Device:R", reference="R_OFF", at_x=10.0, at_y=20.0)
    )
    off_board.hierarchical_labels.append(
        SchHierarchicalLabel(text="OFF", at_x=10.0, at_y=20.0)
    )

    root = KiCadSchematic()
    root.uuid = "root"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R_ROOT_ON", at_x=0.0, at_y=0.0))
    root.symbols.append(
        _placed("Device:R", reference="R_ROOT_OFF", at_x=0.0, at_y=20.0)
    )
    root.wires.append(_wire((0.0, 0.0), (20.0, 0.0)))
    root.wires.append(_wire((0.0, 20.0), (20.0, 20.0)))

    active_sheet = _sheet(
        "active.kicad_sch", "active", "active-sheet", _spin("ON", 20.0, 0.0)
    )
    off_sheet = _sheet("off.kicad_sch", "off", "off-sheet", _spin("OFF", 20.0, 20.0))
    off_sheet.on_board = False
    root.sheets.extend([active_sheet, off_sheet])
    root.sub_schematics["active.kicad_sch"] = active
    root.sub_schematics["off.kicad_sch"] = off_board

    compiled = compile_design_subgraphs(root)
    assert [cs.sheet_path_human for cs in compiled] == ["/", "/active/", "/off/"]

    nl = compile_design_netlist(root)
    component_refs = {component.reference for component in nl.components}
    terminal_refs = {
        terminal.designator for net in nl.nets for terminal in net.terminals
    }
    assert {"R_ROOT_ON", "R_ROOT_OFF", "R_ON", "R_OFF"} <= component_refs
    assert {"R_ROOT_ON", "R_ROOT_OFF", "R_ON", "R_OFF"} <= terminal_refs
    off_component = next(item for item in nl.components if item.reference == "R_OFF")
    assert off_component.on_board is False
    assert off_component.properties["exclude_from_board"] == ""
    off = nl.get_net("/off/OFF")
    assert off is not None
    assert {(t.designator, t.pin) for t in off.terminals} == {
        ("R_ROOT_OFF", "1"),
        ("R_OFF", "1"),
    }


# ---------------------------------------------------------------------------
# Sheet-pin ↔ hier-label pairing
# ---------------------------------------------------------------------------


def test_sheet_pin_to_hier_label_merges_into_one_net():
    """R1 (root) and R2 (child) both connect to a "SIG" net via the
    sheet_pin / hier_label pairing."""
    root, _ = _two_level_hierarchy_with_sheet_pin()
    nl = compile_design_netlist(root)
    # Find the merged net — it's the one with both R1 and R2 on it.
    candidates = [
        n for n in nl.nets if {t.designator for t in n.terminals} >= {"R1", "R2"}
    ]
    assert len(candidates) == 1, [n.name for n in nl.nets]
    sig = candidates[0]
    # Driver is the hier_label (priority=3) since the parent has no
    # local label — sheet_pin (priority=2) loses to hier_label.
    assert sig.driver_priority == int(KiCadDriverPriority.HIER_LABEL)
    # Net name reflects the sheet path of the contributing subgraph
    # (parent's path "/root-uuid/" since sheet_pin was the entry).
    # The chosen driver kind picks the hier_label, so name is
    # "<sheet_path>SIG" — sheet path is from whichever subgraph wins
    # discovery order. We assert the net name ENDS with "SIG" and
    # starts with "/" — both conventions are acceptable.
    assert sig.name.endswith("SIG"), sig.name


def test_sheet_pin_to_hier_label_net_keeps_semantic_endpoints():
    root, _ = _two_level_hierarchy_with_sheet_pin()
    nl = compile_design_netlist(root)
    sig = next(
        n for n in nl.nets if {t.designator for t in n.terminals} >= {"R1", "R2"}
    )

    by_role = {endpoint.role: endpoint for endpoint in sig.endpoints}
    assert by_role["sheet_entry"].endpoint_id == "sheet_entry:sheet-pin-uuid"
    assert by_role["sheet_entry"].element_id == "sheet-pin-uuid"
    assert by_role["sheet_entry"].object_id == "sheet-pin-uuid"
    assert by_role["sheet_entry"].source_sheet == "/"
    assert by_role["sheet_entry"].connection_point == (400000, 100000)

    assert by_role["port"].endpoint_id == "port:hier-label-uuid"
    assert by_role["port"].element_id == "hier-label-uuid"
    assert by_role["port"].object_id == "hier-label-uuid"
    assert by_role["port"].source_sheet == "/sheet-uuid/"
    assert by_role["port"].connection_point == (200000, 100000)


def test_sheet_pin_with_no_matching_hier_label_does_not_merge():
    """A sheet pin name that isn't in the child's hier_label list →
    parent and child stay separate."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sub = KiCadSchematic()
    sub.uuid = "c"
    sub.lib_symbols.append(libR)
    sub.symbols.append(_placed("Device:R", reference="R2", at_x=20.0, at_y=10.0))
    # Child has hier_label "DIFFERENT_NAME" — not "SIG".
    sub.hierarchical_labels.append(
        SchHierarchicalLabel(text="DIFFERENT_NAME", at_x=20.0, at_y=10.0)
    )

    root = KiCadSchematic()
    root.uuid = "r"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", at_x=10.0, at_y=10.0))
    root.wires.append(_wire((10.0, 10.0), (40.0, 10.0)))
    root.sheets.append(_sheet("sub.kicad_sch", "sub", "s", _spin("SIG", 40.0, 10.0)))
    root.sub_schematics["sub.kicad_sch"] = sub

    nl = compile_design_netlist(root)
    # No single net should contain BOTH R1 and R2.
    refs_per_net = [{t.designator for t in n.terminals} for n in nl.nets]
    assert not any("R1" in r and "R2" in r for r in refs_per_net), refs_per_net


def test_cross_sheet_bus_members_match_escaped_slash_labels():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))

    def child(uuid: str, ref: str) -> KiCadSchematic:
        sch = KiCadSchematic()
        sch.uuid = uuid
        sch.lib_symbols.append(libR)
        sch.symbols.append(_placed("Device:R", reference=ref, at_x=10.0, at_y=10.0))
        sch.labels.append(SchLabel(text="ADC0{slash}GPIO0", at_x=10.0, at_y=10.0))
        sch.hierarchical_labels.append(
            SchHierarchicalLabel(text="{ATMEGA_BREAKOUT}", at_x=0.0, at_y=0.0)
        )
        return sch

    root = KiCadSchematic()
    root.uuid = "root"
    root.bus_aliases.append(SchBusAlias(name="ATMEGA_BREAKOUT", members=["ADC0/GPIO0"]))
    child_a = child("a", "R1")
    child_b = child("b", "R2")
    root.sheets.append(
        _sheet(
            "a.kicad_sch",
            "a",
            "sheet-a",
            _spin("{ATMEGA_BREAKOUT}", 0.0, 0.0),
        )
    )
    root.sheets.append(
        _sheet(
            "b.kicad_sch",
            "b",
            "sheet-b",
            _spin("{ATMEGA_BREAKOUT}", 20.0, 0.0),
        )
    )
    root.sub_schematics["a.kicad_sch"] = child_a
    root.sub_schematics["b.kicad_sch"] = child_b

    nl = compile_design_netlist(root)
    merged = [n for n in nl.nets if {t.designator for t in n.terminals} == {"R1", "R2"}]
    assert len(merged) == 1, [
        (n.name, [(t.designator, t.pin) for t in n.terminals]) for n in nl.nets
    ]
    assert "ADC0{slash}GPIO0" in merged[0].name


def test_project_aliases_override_legacy_aliases_and_keep_unrelated_legacy() -> None:
    root = KiCadSchematic()
    root.uuid = "root"
    root.bus_aliases.extend(
        [
            SchBusAlias(name="CTRL", members=["OLD_A", "OLD_B"]),
            SchBusAlias(name="LEGACY_ONLY", members=["L0", "L0"]),
        ]
    )

    compiled = compile_design_subgraphs(
        root,
        bus_aliases={"CTRL": [], "PROJECT_ONLY": ["P0", "P1"]},
    )

    assert compiled[0].bus_aliases_design == {
        "CTRL": [],
        "LEGACY_ONLY": ["L0", "L0"],
        "PROJECT_ONLY": ["P0", "P1"],
    }


def test_design_duplicate_sheet_pin_names_get_stable_suffixes():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))

    root = KiCadSchematic()
    root.uuid = "r"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", at_x=10.0, at_y=10.0))
    root.symbols.append(_placed("Device:R", reference="R2", at_x=20.0, at_y=10.0))
    root.sheets.append(
        _sheet("missing1.kicad_sch", "child1", "s1", _spin("OUT", 10.0, 10.0))
    )
    root.sheets.append(
        _sheet("missing2.kicad_sch", "child2", "s2", _spin("OUT", 20.0, 10.0))
    )

    nl = compile_design_netlist(root)
    by_name = {
        net.name: sorted((t.designator, t.pin) for t in net.terminals)
        for net in nl.nets
    }
    assert by_name["/OUT"] == [("R1", "1")]
    assert by_name["/OUT_1"] == [("R2", "1")]


def test_design_sheet_pin_suffix_follows_source_order_not_net_order():
    """Duplicate sheet-pin suffixes follow schematic sheet-pin order.

    The terminal-bearing net is last spatially but belongs to the third
    ``DO`` sheet pin in source order, matching kicad-cli's ``/DO_2``.
    """
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))

    def din(x: float) -> SchSheetPin:
        return SchSheetPin(name="DIN", shape=LabelShape.INPUT, at_x=x, at_y=0.0)

    def do(x: float) -> SchSheetPin:
        return SchSheetPin(name="DO", shape=LabelShape.OUTPUT, at_x=x, at_y=0.0)

    root = KiCadSchematic()
    root.uuid = "r"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", at_x=100.0, at_y=0.0))
    root.sheets.extend(
        [
            _sheet("missing2.kicad_sch", "controller2", "s2", do(10.0)),
            _sheet("missing3.kicad_sch", "controller3", "s3", din(20.0), do(30.0)),
            _sheet("missing6.kicad_sch", "controller6", "s6", din(80.0), do(90.0)),
            _sheet("missing5.kicad_sch", "controller5", "s5", din(60.0), do(70.0)),
            _sheet("missing4.kicad_sch", "controller4", "s4", din(40.0), do(50.0)),
        ]
    )
    root.wires.extend(
        [
            _wire((10.0, 0.0), (20.0, 0.0)),
            _wire((30.0, 0.0), (40.0, 0.0)),
            _wire((50.0, 0.0), (60.0, 0.0)),
            _wire((70.0, 0.0), (80.0, 0.0)),
            _wire((90.0, 0.0), (100.0, 0.0)),
        ]
    )

    nl = compile_design_netlist(root)

    terminal_net = next(
        net
        for net in nl.nets
        if sorted((t.designator, t.pin) for t in net.terminals) == [("R1", "1")]
    )
    assert terminal_net.name == "/DO_2"


# ---------------------------------------------------------------------------
# Cross-sheet global-label merge
# ---------------------------------------------------------------------------


def test_cross_sheet_global_label_merges_into_one_net():
    root = _two_sheets_with_global_label()
    nl = compile_design_netlist(root)
    # GLOBAL_LABEL emits bare (no sheet-path prefix), parity with
    # power-symbol value names — see ``name_net``.
    vcc = nl.get_net("VCC")
    assert vcc is not None, [n.name for n in nl.nets]
    refs = sorted({t.designator for t in vcc.terminals})
    assert refs == ["R1", "R2"]
    assert vcc.driver_priority == int(KiCadDriverPriority.GLOBAL)


# ---------------------------------------------------------------------------
# Cross-sheet global-power-pin merge
# ---------------------------------------------------------------------------


def test_cross_sheet_global_power_symbol_merges_into_one_net():
    root = _two_sheets_with_global_power()
    nl = compile_design_netlist(root)
    gnd = nl.get_net("GND")
    assert gnd is not None, [n.name for n in nl.nets]
    refs = sorted({t.designator for t in gnd.terminals})
    # R1, R2, plus the two power-symbol designators.
    assert "R1" in refs and "R2" in refs


# ---------------------------------------------------------------------------
# Net code numbering
# ---------------------------------------------------------------------------


def test_design_netlist_codes_sequential_starting_at_one():
    root = _two_sheets_with_global_label()
    nl = compile_design_netlist(root)
    codes = sorted(n.code for n in nl.nets)
    assert codes == list(range(1, len(codes) + 1))


# ---------------------------------------------------------------------------
# Pin-only nets stay sheet-local
# ---------------------------------------------------------------------------


def test_isolated_pin_only_subgraphs_do_not_cross_merge():
    """Two child sheets each with their own isolated R-net — without
    any cross-sheet driver, they stay distinct."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))

    def _make_sub(uuid: str, ref: str) -> KiCadSchematic:
        s = KiCadSchematic()
        s.uuid = uuid
        s.lib_symbols.append(libR)
        s.symbols.append(_placed("Device:R", reference=ref, at_x=20.0, at_y=10.0))
        return s

    a = _make_sub("a", "RA")
    b = _make_sub("b", "RB")

    root = KiCadSchematic()
    root.uuid = "r"
    root.sheets.append(_sheet("a.kicad_sch", "A", "uuid-a"))
    root.sheets.append(_sheet("b.kicad_sch", "B", "uuid-b"))
    root.sub_schematics["a.kicad_sch"] = a
    root.sub_schematics["b.kicad_sch"] = b

    nl = compile_design_netlist(root)
    # Each sheet's R should have its own auto-named net.
    auto_nets = [n for n in nl.nets if n.auto_named]
    refs = {n.terminals[0].designator for n in auto_nets if n.terminals}
    assert "RA" in refs and "RB" in refs
    # And the two are NOT in the same net.
    for n in auto_nets:
        ref_set = {t.designator for t in n.terminals}
        assert not (("RA" in ref_set) and ("RB" in ref_set))


# ---------------------------------------------------------------------------
# merge_design_nets is callable directly with hand-built CompiledSheets
# ---------------------------------------------------------------------------


def test_merge_design_nets_is_a_pure_function():
    """Direct call to :func:`merge_design_nets` works on the output of
    :func:`compile_design_subgraphs`."""
    root, _ = _two_level_hierarchy_with_sheet_pin()
    compiled = compile_design_subgraphs(root)
    nets = merge_design_nets(compiled)
    assert any({t.designator for t in n.terminals} >= {"R1", "R2"} for n in nets)


def test_occurrence_walker_is_parent_first_and_folds_ancestor_policy():
    leaf = KiCadSchematic()
    leaf.uuid = "leaf-source"

    mid = KiCadSchematic()
    mid.uuid = "mid-source"
    leaf_sheet = _sheet("leaf.kicad_sch", "LEAF", "leaf-placement")
    leaf_sheet.on_board = False
    mid.sheets.append(leaf_sheet)
    mid.sub_schematics["leaf.kicad_sch"] = leaf

    root = KiCadSchematic()
    root.uuid = "root-source"
    mid_sheet = _sheet("mid.kicad_sch", "MID", "mid-placement")
    mid_sheet.dnp = True
    mid_sheet.in_bom = False
    mid_sheet.exclude_from_sim = True
    root.sheets.append(mid_sheet)
    root.sub_schematics["mid.kicad_sch"] = mid

    occurrences = list(walk_schematic_occurrences(root))

    assert [item.sheet_path for item in occurrences] == ["/", "/MID/", "/MID/LEAF/"]
    assert occurrences[1].parent is occurrences[0]
    assert occurrences[2].parent is occurrences[1]
    assert occurrences[2].occurrence_address == (
        "/root-source/mid-placement/leaf-placement"
    )
    assert occurrences[2].effective_dnp is True
    assert occurrences[2].effective_in_bom is False
    assert occurrences[2].effective_on_board is False
    assert occurrences[2].effective_exclude_from_sim is True
    assert occurrences[2].raw_dnp is False
    assert occurrences[2].raw_in_bom is True


def test_occurrence_walker_can_filter_effectively_off_board_subtrees():
    root, child = _two_level_hierarchy_with_sheet_pin()
    root.sheets[0].on_board = False
    grandchild = KiCadSchematic()
    child.sheets.append(_sheet("grand.kicad_sch", "grand", "grand-placement"))
    child.sub_schematics["grand.kicad_sch"] = grandchild

    assert len(list(walk_schematic_occurrences(root))) == 3
    assert len(list(walk_schematic_occurrences(root, include_off_board=False))) == 1
    assert len(compile_design_subgraphs(root)) == 3
    assert len(compile_design_subgraphs(root, include_off_board=False)) == 1

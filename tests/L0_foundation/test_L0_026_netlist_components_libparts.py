"""
Test L0_026: component + libpart collation (Phase G — Slice N-4b).

Pure-unit coverage for the component / libpart walk that the kicadsexpr
emit (slice N-5) consumes. Tests build synthetic schematic hierarchies
in-memory and assert that ``compile_design_netlist`` populates
``components`` / ``libparts`` / ``design_metadata.sheets`` correctly.
"""

from __future__ import annotations

from typing import Optional

from kicad_monkey import (
    collect_design_components,
    collect_design_libparts,
    compile_design_netlist,
    compile_design_subgraphs,
)
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_sch_enums import LabelShape, PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_sheet import SchSheet, SchSheetPin, SchSheetProperty
from kicad_monkey.kicad_sch_symbol import SchSymbol
from kicad_monkey.kicad_sch_title_block import TitleBlock
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_sym_pin import SymPin
from kicad_monkey.kicad_sym_property import SymProperty


# ---------------------------------------------------------------------------
# Synth helpers
# ---------------------------------------------------------------------------


def _pin(at_x: float, at_y: float, *, number: str = "1", name: str = "~",
         electrical: PinElectricalType = PinElectricalType.PASSIVE) -> SymPin:
    return SymPin(
        electrical_type=electrical,
        graphic_style=PinGraphicStyle.LINE,
        at_x=at_x, at_y=at_y, at_angle=180.0, length=0.0,
        number=number, name=name,
    )


def _libsym(
    name: str, *pins: SymPin,
    power: bool = False, power_kind: Optional[str] = None,
    description: str = "", datasheet: str = "",
    keywords: str = "", fp_filters: str = "",
) -> LibSymbol:
    sub = LibSubSymbol(name=f"{name}_1_0", unit=1, style=0, pins=list(pins))
    props = []
    if description:
        props.append(SymProperty(key="Description", value=description, id=5))
    if datasheet:
        props.append(SymProperty(key="Datasheet", value=datasheet, id=3))
    if keywords:
        props.append(SymProperty(key="ki_keywords", value=keywords, id=5))
    if fp_filters:
        props.append(SymProperty(key="ki_fp_filters", value=fp_filters, id=6))
    # Add a Reference / Value property so get_property_value() works.
    # Value uses the bare part name (post-colon) — matches the KiCad
    # convention where a symbol's `Value` field holds e.g. "R", not
    # "Device:R".
    bare_part = name.split(":", 1)[1] if ":" in name else name
    props.append(SymProperty(key="Reference", value="R", id=0))
    props.append(SymProperty(key="Value", value=bare_part, id=1))
    return LibSymbol(name=name, power=power, power_kind=power_kind,
                     subsymbols=[sub], properties=props)


def _placed(lib_id: str, *, reference: str, value: str = "",
            footprint: str = "", uuid: str = "",
            lib_name: str = "",
            properties_extra: Optional[dict] = None,
            in_bom: bool = True, on_board: bool = True,
            dnp: bool = False,
            at_x: float = 0.0, at_y: float = 0.0,
            unit: int = 1) -> SchSymbol:
    sym = SchSymbol(lib_id=lib_id, at_x=at_x, at_y=at_y, at_angle=0.0,
                    mirror=None, unit=unit, convert=1,
                    in_bom=in_bom, on_board=on_board, dnp=dnp,
                    uuid=uuid, lib_name=lib_name)
    sym.properties = [
        SymProperty(key="Reference", value=reference, id=0),
        SymProperty(key="Value", value=value or reference, id=1),
    ]
    if footprint:
        sym.properties.append(SymProperty(key="Footprint", value=footprint, id=2))
    if properties_extra:
        idx = 4
        for k, v in properties_extra.items():
            sym.properties.append(SymProperty(key=k, value=v, id=idx))
            idx += 1
    return sym


def _sheet(sheet_file: str, sheet_name: str, uuid: str,
           *pins: SchSheetPin) -> SchSheet:
    sh = SchSheet(uuid=uuid)
    sh.properties = [
        SchSheetProperty(key="Sheetname", value=sheet_name),
        SchSheetProperty(key="Sheetfile", value=sheet_file),
    ]
    sh.pins = list(pins)
    return sh


def _spin(name: str, at_x: float, at_y: float) -> SchSheetPin:
    return SchSheetPin(name=name, shape=LabelShape.INPUT,
                       at_x=at_x, at_y=at_y)


# ---------------------------------------------------------------------------
# collect_design_components
# ---------------------------------------------------------------------------


def test_collect_components_root_only_simple():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", value="10k",
                               uuid="uid-r1"))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert len(comps) == 1
    c = comps[0]
    assert c.reference == "R1"
    assert c.value == "10k"
    assert c.libsource_lib == "Device"
    assert c.libsource_part == "R"
    assert c.instance_uuid == "uid-r1"
    # Root sheet path is always "/" — kicad-cli convention; the top
    # schematic's own UUID never appears in the path.
    assert c.sheet_path_uuids == "/"
    assert c.sheet_path_names == "/"


def test_collect_components_root_properties_use_schematic_file_name():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.source_path = "C:/projects/power-supply.kicad_sch"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="uid-r1"))

    [component] = collect_design_components(compile_design_subgraphs(sch))

    assert component.properties["Sheetname"] == "power-supply"
    assert component.properties["Sheetfile"] == "power-supply.kicad_sch"


def test_collect_components_multi_sheet_carries_sheet_path():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sub = KiCadSchematic()
    sub.uuid = "child"
    sub.lib_symbols.append(libR)
    sub.symbols.append(_placed("Device:R", reference="R2", uuid="r2-uid",
                               at_x=20.0, at_y=10.0))

    root = KiCadSchematic()
    root.uuid = "root"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", uuid="r1-uid"))
    root.sheets.append(_sheet("sub.kicad_sch", "sub", "sheetuuid"))
    root.sub_schematics["sub.kicad_sch"] = sub

    compiled = compile_design_subgraphs(root)
    comps = collect_design_components(compiled)
    by_ref = {c.reference: c for c in comps}
    assert set(by_ref.keys()) == {"R1", "R2"}
    assert by_ref["R1"].sheet_path_uuids == "/"
    assert by_ref["R1"].sheet_path_names == "/"
    assert by_ref["R2"].sheet_path_uuids == "/sheetuuid/"
    assert by_ref["R2"].sheet_path_names == "/sub/"


def test_collect_components_dedupes_by_uuid():
    """Same SchSymbol exposed via two compiled sheets (e.g. shared
    schematic file) should appear once when its uuid matches."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sym = _placed("Device:R", reference="R1", uuid="dup-uuid")
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    # Same symbol object twice (uuid matches).
    sch.symbols.append(sym)
    sch.symbols.append(sym)
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert len(comps) == 1


def test_collect_components_collapses_multi_unit_reference():
    """KiCad emits one component row per reference, not one per unit."""
    libU = _libsym("Device:U", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libU)
    sch.symbols.append(_placed(
        "Device:U", reference="U1", uuid="u1-unit2",
        footprint="Package:WrongUnit", unit=2, at_x=10.0,
    ))
    sch.symbols.append(_placed(
        "Device:U", reference="U1", uuid="u1-unit1",
        footprint="Package:Unit1", unit=1,
    ))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert len(comps) == 1
    assert comps[0].reference == "U1"
    assert comps[0].footprint == "Package:Unit1"
    assert comps[0].instance_uuid == "u1-unit1"
    assert comps[0].instance_uuids == ["u1-unit2", "u1-unit1"]


def test_collect_components_reverses_non_primary_multi_unit_tstamps():
    libU = _libsym("Device:U", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libU)
    # The lexical-lowest UUID is KiCad's primary and remains last. Other
    # unit UUIDs are prepended as encountered, giving unit 2 then unit 1.
    sch.symbols.extend([
        _placed("Device:U", reference="U1", uuid="1-unit3", unit=3),
        _placed("Device:U", reference="U1", uuid="2-unit1", unit=1),
        _placed("Device:U", reference="U1", uuid="3-unit2", unit=2),
    ])

    [component] = collect_design_components(compile_design_subgraphs(sch))

    assert component.instance_uuid == "1-unit3"
    assert component.instance_uuids == ["3-unit2", "2-unit1", "1-unit3"]


def test_collect_components_suppresses_later_cross_sheet_multi_unit_rows():
    libU = LibSymbol(
        name="Device:U",
        subsymbols=[
            LibSubSymbol(
                name="Device:U_1_0",
                unit=1,
                style=0,
                pins=[_pin(0.0, 0.0, number="1")],
            ),
            LibSubSymbol(
                name="Device:U_2_0",
                unit=2,
                style=0,
                pins=[_pin(0.0, 0.0, number="2")],
            ),
        ],
    )
    child_a = KiCadSchematic()
    child_a.uuid = "child-a"
    child_a.lib_symbols.append(libU)
    child_a.symbols.append(_placed(
        "Device:U", reference="U1", uuid="u1-a", unit=1,
    ))
    child_b = KiCadSchematic()
    child_b.uuid = "child-b"
    child_b.lib_symbols.append(libU)
    child_b.symbols.append(_placed(
        "Device:U", reference="U1", uuid="u1-b", unit=2,
    ))
    root = KiCadSchematic()
    root.uuid = "root"
    root.sheets.append(_sheet("a.kicad_sch", "A", "sheet-a"))
    root.sheets.append(_sheet("b.kicad_sch", "B", "sheet-b"))
    root.sub_schematics["a.kicad_sch"] = child_a
    root.sub_schematics["b.kicad_sch"] = child_b

    comps = collect_design_components(compile_design_subgraphs(root))
    assert len(comps) == 1
    assert comps[0].reference == "U1"
    assert comps[0].sheet_path_names == "/A/"
    assert comps[0].instance_uuids == ["u1-a"]


def test_collect_components_carries_extra_properties():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed(
        "Device:R", reference="R1", uuid="r1",
        footprint="Resistor_SMD:R_0603",
        properties_extra={"MPN": "ERJ-3EKF1002V", "Manufacturer": "Panasonic"},
    ))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert len(comps) == 1
    c = comps[0]
    assert c.footprint == "Resistor_SMD:R_0603"
    # Standard fields excluded from `properties` dict; extras kept.
    assert "Reference" not in c.properties
    assert "Value" not in c.properties
    assert "Footprint" not in c.properties
    assert c.properties == {"MPN": "ERJ-3EKF1002V", "Manufacturer": "Panasonic"}


def test_collect_components_carries_kicad_component_metadata():
    libU = _libsym(
        "VendorPart_1",
        _pin(0.0, 0.0, number="1"),
        _pin(2.54, 0.0, number="2"),
        description="Library description",
        keywords="mixed signal",
        fp_filters="Package:QFN*",
    )
    sub = KiCadSchematic()
    sub.uuid = "child"
    sub.lib_symbols.append(libU)
    sub.symbols.append(_placed(
        "Vendor:VendorPart",
        reference="U1",
        value="VendorPart",
        footprint="Package:QFN",
        uuid="u1",
        lib_name="VendorPart_1",
        properties_extra={
            "Datasheet": "https://example.test/ds.pdf",
            "Description": "Placed description",
            "MPN": "VP-123",
        },
    ))

    root = KiCadSchematic()
    root.uuid = "root"
    root.sheets.append(_sheet("sub.kicad_sch", "Analog", "sheetuuid"))
    root.sub_schematics["sub.kicad_sch"] = sub

    comps = collect_design_components(compile_design_subgraphs(root))
    assert len(comps) == 1
    comp = comps[0]
    assert comp.libsource_lib == ""
    assert comp.libsource_part == "VendorPart_1"
    assert comp.libsource_description == "Library description"
    assert comp.datasheet == "https://example.test/ds.pdf"
    assert comp.description == "Placed description"
    assert comp.fields == {
        "MPN": "VP-123",
        "Footprint": "Package:QFN",
        "Datasheet": "https://example.test/ds.pdf",
        "Description": "Placed description",
    }
    assert comp.properties == {
        "MPN": "VP-123",
        "Sheetname": "Analog",
        "Sheetfile": "sub.kicad_sch",
        "ki_keywords": "mixed signal",
        "ki_fp_filters": "Package:QFN*",
    }
    assert [(unit.name, unit.pins) for unit in comp.units] == [("A", ["1", "2"])]


def test_collect_components_units_match_kicad_position_ordering():
    libU = LibSymbol(
        name="Device:U",
        subsymbols=[
            LibSubSymbol(
                name="Device:U_0_0",
                unit=0,
                style=0,
                pins=[_pin(1.0, -1.0, number="[7-8]")],
            ),
            LibSubSymbol(
                name="Device:U_1_0",
                unit=1,
                style=0,
                unit_name="LEFT",
                pins=[
                    _pin(10.0, 0.0, number="3"),
                    _pin(-5.0, 1.0, number="2"),
                    _pin(-5.0, -1.0, number="1"),
                ],
            ),
            LibSubSymbol(
                name="Device:U_2_0",
                unit=2,
                style=0,
                pins=[
                    _pin(0.0, 2.0, number="B"),
                    _pin(0.0, 1.0, number="A"),
                ],
            ),
        ],
    )
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libU)
    sch.symbols.append(_placed("Device:U", reference="U1", uuid="u1"))

    comps = collect_design_components(compile_design_subgraphs(sch))
    assert len(comps) == 1
    assert [(unit.name, unit.pins) for unit in comps[0].units] == [
        ("LEFT", ["2", "1", "7", "8", "3"]),
        ("B", ["B", "A", "7", "8"]),
    ]


def test_collect_components_carries_in_bom_dnp_flags():
    """Symbols round-trip ``in_bom`` and ``dnp`` flags.

    Only ``on_board=no`` filters a symbol from the components block
    (kicad-cli parity); see the sibling test.
    """
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1",
                               in_bom=False, on_board=True, dnp=True))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert comps[0].in_bom is False
    assert comps[0].on_board is True
    assert comps[0].dnp is True
    assert list(comps[0].properties.items()) == [
        ("exclude_from_bom", ""),
        ("dnp", ""),
    ]


def test_collect_components_folds_policy_from_every_sheet_ancestor():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    leaf = KiCadSchematic()
    leaf.uuid = "leaf"
    leaf.lib_symbols.append(libR)
    leaf.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))

    mid = KiCadSchematic()
    mid.uuid = "mid"
    mid.sheets.append(_sheet("leaf.kicad_sch", "Leaf", "leaf-placement"))
    mid.sub_schematics["leaf.kicad_sch"] = leaf

    root = KiCadSchematic()
    root.uuid = "root"
    mid_sheet = _sheet("mid.kicad_sch", "Mid", "mid-placement")
    mid_sheet.dnp = True
    mid_sheet.in_bom = False
    root.sheets.append(mid_sheet)
    root.sub_schematics["mid.kicad_sch"] = mid

    [component] = collect_design_components(compile_design_subgraphs(root))

    assert component.dnp is True
    assert component.in_bom is False
    assert component.on_board is True
    assert component.properties["dnp"] == ""
    assert component.properties["exclude_from_bom"] == ""


def test_collect_components_expands_value_var_from_symbol_property():
    """``${VAR}`` in the Value field resolves against the symbol's
    own properties (case-insensitive). Mirrors kicad-cli's
    ``ResolveTextVar`` precedence — sallen_key / top_level rely on
    this to surface ALTIUM_VALUE / Sim.Params into the comp row.
    """
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed(
        "Device:R", reference="R1", uuid="r1",
        value="${ALTIUM_VALUE}",
        properties_extra={"ALTIUM_VALUE": "10kOhm"},
    ))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert comps[0].value == "10kOhm"
    # User-defined property values use KiCad's shown-text path too.
    assert comps[0].properties["ALTIUM_VALUE"] == "10kOhm"


def test_collect_components_user_fields_expand_value_var_and_blank_tilde():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed(
        "Device:R",
        reference="R1",
        uuid="r1",
        value="10k",
        properties_extra={
            "ALTIUM_VALUE": "${VALUE}",
            "BlankMeta": "~",
            "Datasheet": "~",
        },
    ))
    comps = collect_design_components(compile_design_subgraphs(sch))
    assert comps[0].datasheet == ""
    assert comps[0].fields["ALTIUM_VALUE"] == "10k"
    assert comps[0].fields["BlankMeta"] == ""
    assert comps[0].fields["Datasheet"] == ""
    assert comps[0].properties["ALTIUM_VALUE"] == "10k"
    assert comps[0].properties["BlankMeta"] == ""


def test_collect_components_value_var_falls_back_to_project_text_vars():
    """When no matching symbol property exists, ``${VAR}`` resolves
    against the project's ``text_variables`` dict."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed(
        "Device:R", reference="R1", uuid="r1", value="${BOARD_REV}",
    ))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled, {"BOARD_REV": "v3"})
    assert comps[0].value == "v3"


def test_collect_components_value_var_unknown_token_passes_through():
    """Unknown ``${VAR}`` tokens are left in place (KiCad parity)."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed(
        "Device:R", reference="R1", uuid="r1", value="${MISSING}",
    ))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert comps[0].value == "${MISSING}"


def test_collect_components_filters_on_board_no():
    """``(on_board no)`` symbols are filtered from the components
    block to match kicad-cli's netlist export. ``(dnp yes)`` alone
    is not enough — dual-population placements stay in the netlist."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    # dnp=yes but on_board=yes → kept.
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1",
                               in_bom=True, on_board=True, dnp=True))
    # on_board=no → filtered.
    sch.symbols.append(_placed("Device:R", reference="R2", uuid="r2",
                               at_x=10.0, in_bom=True, on_board=False, dnp=False))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    refs = [c.reference for c in comps]
    assert refs == ["R1"]
    assert comps[0].properties["dnp"] == ""


def test_collect_components_filters_hash_prefixed_references():
    """Custom-library power symbols with ``#`` refs do not emit comp rows."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    libVPP = _libsym("flat_hierarchy:VPP", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.extend([libR, libVPP])
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    sch.symbols.append(_placed("flat_hierarchy:VPP", reference="#PWR01", uuid="pwr"))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert [c.reference for c in comps] == ["R1"]


def test_collect_components_libsource_description_from_lib_symbol():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"),
                   description="Resistor")
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    compiled = compile_design_subgraphs(sch)
    comps = collect_design_components(compiled)
    assert comps[0].libsource_description == "Resistor"


# ---------------------------------------------------------------------------
# collect_design_libparts
# ---------------------------------------------------------------------------


def test_collect_libparts_simple_resistor():
    libR = _libsym(
        "Device:R",
        _pin(0.0, 0.0, number="1", name="~"),
        _pin(0.0, -2.54, number="2", name="~"),
        description="Resistor",
        datasheet="~",
    )
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    compiled = compile_design_subgraphs(sch)
    libparts = collect_design_libparts(compiled)
    assert len(libparts) == 1
    lp = libparts[0]
    assert lp.lib == "Device"
    assert lp.part == "R"
    assert lp.description == "Resistor"
    assert {p.number for p in lp.pins} == {"1", "2"}
    # Standard fields surface in `fields` dict.
    assert lp.fields.get("Reference") == "R"
    assert lp.fields.get("Value") == "R"


def test_collect_libparts_skips_blank_pin_number_sentinel():
    """KiCad omits libpart pins whose library pin number is ``"~"``."""
    libHole = _libsym("flat_hierarchy:MOUNTING_HOLE", _pin(0.0, 0.0, number="~", name="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libHole)
    sch.symbols.append(_placed("flat_hierarchy:MOUNTING_HOLE", reference="HOLE1", uuid="h1"))
    compiled = compile_design_subgraphs(sch)
    libparts = collect_design_libparts(compiled)
    assert len(libparts) == 1
    assert libparts[0].part == "MOUNTING_HOLE"
    assert libparts[0].pins == []


def test_collect_libparts_dedupes_across_sheets():
    """Same lib_id present in root + child schematic → single libpart."""
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sub = KiCadSchematic()
    sub.uuid = "c"
    sub.lib_symbols.append(libR)
    sub.symbols.append(_placed("Device:R", reference="R2", uuid="r2",
                               at_x=20.0, at_y=10.0))

    root = KiCadSchematic()
    root.uuid = "r"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    root.sheets.append(_sheet("sub.kicad_sch", "sub", "sh"))
    root.sub_schematics["sub.kicad_sch"] = sub

    compiled = compile_design_subgraphs(root)
    libparts = collect_design_libparts(compiled)
    assert len(libparts) == 1
    assert libparts[0].part == "R"


def test_collect_libparts_sorts_pins_naturally():
    """Pins emit sorted by natural-numeric order — 2 before 10."""
    libU = _libsym(
        "Amp:OPAMP",
        _pin(0.0, 0.0, number="10"),
        _pin(0.0, -2.54, number="1"),
        _pin(0.0, -5.08, number="2"),
    )
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libU)
    sch.symbols.append(_placed("Amp:OPAMP", reference="U1", uuid="u1"))
    compiled = compile_design_subgraphs(sch)
    libparts = collect_design_libparts(compiled)
    assert [p.number for p in libparts[0].pins] == ["1", "2", "10"]


def test_collect_libparts_pin_type_mirrors_electrical_type():
    libR = _libsym(
        "Device:R",
        _pin(0.0, 0.0, number="1", electrical=PinElectricalType.INPUT),
        _pin(0.0, -2.54, number="2", electrical=PinElectricalType.OUTPUT),
    )
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    compiled = compile_design_subgraphs(sch)
    libparts = collect_design_libparts(compiled)
    by_num = {p.number: p for p in libparts[0].pins}
    assert by_num["1"].pin_type == "input"
    assert by_num["2"].pin_type == "output"


# ---------------------------------------------------------------------------
# Sheet records
# ---------------------------------------------------------------------------


def test_design_metadata_has_one_sheet_per_compiled_sheet():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sub = KiCadSchematic()
    sub.uuid = "c"
    sub.lib_symbols.append(libR)
    sub.symbols.append(_placed("Device:R", reference="R2", uuid="r2",
                               at_x=20.0, at_y=10.0))

    root = KiCadSchematic()
    root.uuid = "r"
    root.lib_symbols.append(libR)
    root.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    root.sheets.append(_sheet("sub.kicad_sch", "sub", "sh"))
    root.sub_schematics["sub.kicad_sch"] = sub

    nl = compile_design_netlist(root)
    assert len(nl.design_metadata.sheets) == 2
    sheets = nl.design_metadata.sheets
    assert sheets[0].number == 1
    assert sheets[0].name == "/"
    assert sheets[0].tstamps == "/"
    assert sheets[1].number == 2
    assert sheets[1].name == "/sub/"
    assert sheets[1].tstamps == "/sh/"


def test_design_metadata_picks_up_title_block_fields():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    sch.title_block = TitleBlock(
        title="My Project", company="ACME", rev="1.0", date="2026-05-10",
    )
    nl = compile_design_netlist(sch)
    s = nl.design_metadata.sheets[0]
    assert s.title == "My Project"
    assert s.company == "ACME"
    assert s.revision == "1.0"
    assert s.date == "2026-05-10"


# ---------------------------------------------------------------------------
# compile_design_netlist populates everything
# ---------------------------------------------------------------------------


def test_compile_design_netlist_populates_components_and_libparts():
    libR = _libsym("Device:R", _pin(0.0, 0.0, number="1"))
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1"))
    nl = compile_design_netlist(sch)
    assert len(nl.components) == 1
    assert len(nl.libparts) == 1
    assert nl.components[0].reference == "R1"
    assert nl.libparts[0].part == "R"

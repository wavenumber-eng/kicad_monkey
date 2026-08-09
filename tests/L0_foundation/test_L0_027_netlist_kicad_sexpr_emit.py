"""
Test L0_027: KiCad-format netlist emit (Phase G — Slice N-5).

Renders synthesized :class:`KiCadNetlist` payloads through
:func:`to_kicad_sexpr` and asserts the resulting sexpr text round-trips
through :func:`parse_sexp` into the expected structure. Comparisons are
structural — we walk the parsed tree and assert specific elements
exist with expected values rather than byte-comparing the formatted
output (the date / tool / formatter spacing aren't part of the
contract).
"""

from __future__ import annotations

from typing import Any, List, Optional

from kicad_monkey import (
    KICAD_NETLIST_VERSION,
    KiCadDesignSheet,
    KiCadLibPart,
    KiCadLibPartPin,
    KiCadNet,
    KiCadNetlist,
    KiCadNetlistComponent,
    KiCadNetlistTerminal,
    compile_design_netlist,
    parse_sexp,
    to_kicad_sexpr,
)
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_netlist_model import KiCadNetlistComponentUnit
from kicad_monkey.kicad_sch_enums import PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_symbol import SchSymbol
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_sym_pin import SymPin
from kicad_monkey.kicad_sym_property import SymProperty


# ---------------------------------------------------------------------------
# S-expression tree-walk helpers
# ---------------------------------------------------------------------------


def _find(tree: list, tag: str) -> Optional[list]:
    """Return the first child list whose head token equals ``tag``."""
    for child in tree[1:]:
        if isinstance(child, list) and child and child[0] == tag:
            return child
    return None


def _find_all(tree: list, tag: str) -> List[list]:
    return [c for c in tree[1:] if isinstance(c, list) and c and c[0] == tag]


def _value(tree: list, tag: str) -> Optional[str]:
    """Return the first quoted/bare value of a single-arg child."""
    child = _find(tree, tag)
    if child is None or len(child) < 2:
        return None
    return _strip(child[1])


def _strip(tok: Any) -> str:
    """Strip surrounding quotes if any."""
    s = str(tok)
    if len(s) >= 2 and s[0] == '"' and s[-1] == '"':
        return s[1:-1]
    return s


# ---------------------------------------------------------------------------
# Fixture builders
# ---------------------------------------------------------------------------


def _simple_netlist() -> KiCadNetlist:
    nl = KiCadNetlist()
    nl.design_metadata.sheets = [
        KiCadDesignSheet(number=1, name="/", tstamps="/"),
    ]
    nl.components = [
        KiCadNetlistComponent(
            reference="R1",
            value="10k",
            footprint="Resistor_SMD:R_0603",
            libsource_lib="Device",
            libsource_part="R",
            libsource_description="Resistor",
            sheet_path_names="/",
            sheet_path_uuids="/",
            instance_uuid="r1-uuid",
            properties={"MPN": "ERJ-3EKF1002V"},
        ),
    ]
    nl.libparts = [
        KiCadLibPart(
            lib="Device",
            part="R",
            description="Resistor",
            docs="~",
            footprints_filter=["R_*"],
            fields={"Reference": "R", "Value": "R", "Datasheet": "~"},
            pins=[
                KiCadLibPartPin(number="1", name="~", pin_type="passive"),
                KiCadLibPartPin(number="2", name="~", pin_type="passive"),
            ],
        ),
    ]
    nl.libraries = []
    nl.nets = [
        KiCadNet(
            name="/VCC",
            code=1,
            terminals=[
                KiCadNetlistTerminal(
                    designator="R1",
                    pin="1",
                    pin_name="~",
                    pin_type="passive",
                    sheet_path="/",
                ),
            ],
        ),
        KiCadNet(
            name="Net-(R1-2)",
            code=2,
            auto_named=True,
            terminals=[
                KiCadNetlistTerminal(
                    designator="R1",
                    pin="2",
                    pin_name="~",
                    pin_type="passive",
                    sheet_path="/",
                ),
            ],
        ),
    ]
    return nl


def test_kicadsexpr_omits_effectively_off_board_components_and_nodes() -> None:
    """The internal sidecar retains policy; KiCad's board netlist does not."""

    netlist = _simple_netlist()
    netlist.components.append(
        KiCadNetlistComponent(reference="R2", value="DNI", on_board=False)
    )
    netlist.nets[0].terminals.append(KiCadNetlistTerminal(designator="R2", pin="1"))

    tree = parse_sexp(to_kicad_sexpr(netlist, date=""))
    components = _find(tree, "components")
    nets = _find(tree, "nets")
    assert components is not None and nets is not None
    assert [_value(row, "ref") for row in _find_all(components, "comp")] == ["R1"]
    assert "R2" not in {
        _value(node, "ref")
        for net in _find_all(nets, "net")
        for node in _find_all(net, "node")
    }


def _placed(
    lib_id: str,
    *,
    reference: str,
    value: str = "",
    uuid: str = "",
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
        uuid=uuid,
    )
    sym.properties = [
        SymProperty(key="Reference", value=reference, id=0),
        SymProperty(key="Value", value=value or reference, id=1),
    ]
    return sym


def _libR_full() -> LibSymbol:
    sub = LibSubSymbol(
        name="Device:R_1_0",
        unit=1,
        style=0,
        pins=[
            SymPin(
                electrical_type=PinElectricalType.PASSIVE,
                graphic_style=PinGraphicStyle.LINE,
                at_x=0.0,
                at_y=0.0,
                at_angle=180.0,
                length=0.0,
                number="1",
                name="~",
            ),
            SymPin(
                electrical_type=PinElectricalType.PASSIVE,
                graphic_style=PinGraphicStyle.LINE,
                at_x=0.0,
                at_y=-2.54,
                at_angle=0.0,
                length=0.0,
                number="2",
                name="~",
            ),
        ],
    )
    return LibSymbol(
        name="Device:R",
        subsymbols=[sub],
        properties=[
            SymProperty(key="Reference", value="R", id=0),
            SymProperty(key="Value", value="R", id=1),
            SymProperty(key="Description", value="Resistor", id=5),
        ],
    )


# ---------------------------------------------------------------------------
# Top-level envelope shape
# ---------------------------------------------------------------------------


def test_emit_starts_with_export_and_version_E():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    assert tree[0] == "export"
    version = _find(tree, "version")
    assert version is not None
    assert _strip(version[1]) == KICAD_NETLIST_VERSION


def test_emit_has_all_top_level_blocks():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    for tag in ("design", "components", "libparts", "libraries", "nets"):
        assert _find(tree, tag) is not None, f"missing {tag} block"


def test_emit_ends_with_newline():
    text = to_kicad_sexpr(_simple_netlist())
    assert text.endswith("\n")


# ---------------------------------------------------------------------------
# (design ...) block
# ---------------------------------------------------------------------------


def test_emit_design_carries_source_date_tool():
    text = to_kicad_sexpr(
        _simple_netlist(),
        source_path="/tmp/foo.kicad_sch",
        tool="kicad_monkey",
        date="2026-05-10",
    )
    tree = parse_sexp(text)
    design = _find(tree, "design")
    assert _value(design, "source") == "/tmp/foo.kicad_sch"
    assert _value(design, "date") == "2026-05-10"
    assert _value(design, "tool") == "kicad_monkey"


def test_emit_design_emits_one_sheet_per_metadata_record():
    nl = _simple_netlist()
    nl.design_metadata.sheets = [
        KiCadDesignSheet(number=1, name="/", tstamps="/"),
        KiCadDesignSheet(number=2, name="/sub/", tstamps="/abc/"),
    ]
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    design = _find(tree, "design")
    sheets = _find_all(design, "sheet")
    assert len(sheets) == 2
    assert _value(sheets[0], "number") == "1"
    assert _value(sheets[0], "name") == "/"
    assert _value(sheets[0], "tstamps") == "/"
    assert _value(sheets[1], "number") == "2"
    assert _value(sheets[1], "name") == "/sub/"
    assert _value(sheets[1], "tstamps") == "/abc/"


def test_emit_design_sheet_has_title_block_with_9_comments():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    design = _find(tree, "design")
    sheet = _find(design, "sheet")
    tb = _find(sheet, "title_block")
    assert tb is not None
    comments = _find_all(tb, "comment")
    assert len(comments) == 9


def test_emit_title_block_uses_set_metadata():
    nl = _simple_netlist()
    nl.design_metadata.sheets = [
        KiCadDesignSheet(
            number=1,
            name="/",
            tstamps="/",
            title="Project",
            company="ACME",
            revision="1.0",
            date="2026-05-10",
        ),
    ]
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    sheet = _find(_find(tree, "design"), "sheet")
    tb = _find(sheet, "title_block")
    assert _value(tb, "title") == "Project"
    assert _value(tb, "company") == "ACME"
    assert _value(tb, "rev") == "1.0"
    assert _value(tb, "date") == "2026-05-10"


# ---------------------------------------------------------------------------
# (components ...) block
# ---------------------------------------------------------------------------


def test_emit_components_one_comp_per_record():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    comps = _find_all(_find(tree, "components"), "comp")
    assert len(comps) == 1
    c = comps[0]
    assert _value(c, "ref") == "R1"
    assert _value(c, "value") == "10k"
    assert _value(c, "footprint") == "Resistor_SMD:R_0603"


def test_emit_components_carry_libsource_block():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    comp = _find(_find(tree, "components"), "comp")
    libsource = _find(comp, "libsource")
    assert _value(libsource, "lib") == "Device"
    assert _value(libsource, "part") == "R"
    assert _value(libsource, "description") == "Resistor"


def test_emit_components_carry_extra_properties():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    comp = _find(_find(tree, "components"), "comp")
    props = _find_all(comp, "property")
    assert len(props) == 1
    p = props[0]
    assert _value(p, "name") == "MPN"
    assert _value(p, "value") == "ERJ-3EKF1002V"


def test_emit_components_carry_kicad_metadata_blocks():
    nl = _simple_netlist()
    comp_model = nl.components[0]
    comp_model.datasheet = "https://example.test/r1.pdf"
    comp_model.description = "Resistor row"
    comp_model.fields = {
        "MPN": "ERJ-3EKF1002V",
        "Footprint": "Resistor_SMD:R_0603",
        "Datasheet": "https://example.test/r1.pdf",
        "Description": "",
    }
    comp_model.units = [KiCadNetlistComponentUnit(name="A", pins=["1", "2"])]
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    comp = _find(_find(tree, "components"), "comp")
    assert _value(comp, "datasheet") == "https://example.test/r1.pdf"
    assert _value(comp, "description") == "Resistor row"

    fields = _find_all(_find(comp, "fields"), "field")
    assert [
        (_value(field, "name"), _strip(field[2]) if len(field) > 2 else "")
        for field in fields
    ] == [
        ("MPN", "ERJ-3EKF1002V"),
        ("Footprint", "Resistor_SMD:R_0603"),
        ("Datasheet", "https://example.test/r1.pdf"),
        ("Description", ""),
    ]

    units = _find(comp, "units")
    unit = _find(units, "unit")
    assert _value(unit, "name") == "A"
    pins = _find_all(_find(unit, "pins"), "pin")
    assert [_value(pin, "num") for pin in pins] == ["1", "2"]


def test_emit_components_carry_sheetpath_and_tstamps():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    comp = _find(_find(tree, "components"), "comp")
    sp = _find(comp, "sheetpath")
    assert _value(sp, "names") == "/"
    assert _value(sp, "tstamps") == "/"
    assert _value(comp, "tstamps") == "r1-uuid"


def test_emit_components_carry_multi_unit_tstamps():
    nl = _simple_netlist()
    nl.components[0].instance_uuid = "u1"
    nl.components[0].instance_uuids = ["u1", "u2", "u3"]
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    comp = _find(_find(tree, "components"), "comp")
    tstamps = _find(comp, "tstamps")
    assert [_strip(value) for value in tstamps[1:]] == ["u1", "u2", "u3"]


def test_emit_components_omit_footprint_when_empty():
    nl = _simple_netlist()
    nl.components[0].footprint = ""
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    comp = _find(_find(tree, "components"), "comp")
    assert _find(comp, "footprint") is None


# ---------------------------------------------------------------------------
# (libparts ...) block
# ---------------------------------------------------------------------------


def test_emit_libparts_carry_lib_and_part():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    libparts = _find_all(_find(tree, "libparts"), "libpart")
    assert len(libparts) == 1
    lp = libparts[0]
    assert _value(lp, "lib") == "Device"
    assert _value(lp, "part") == "R"
    assert _value(lp, "description") == "Resistor"
    assert _value(lp, "docs") == "~"


def test_emit_libparts_emit_pins_with_num_name_type():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    libpart = _find(_find(tree, "libparts"), "libpart")
    pins = _find(libpart, "pins")
    pin_entries = _find_all(pins, "pin")
    assert len(pin_entries) == 2
    nums = [_value(p, "num") for p in pin_entries]
    assert nums == ["1", "2"]
    types = [_value(p, "type") for p in pin_entries]
    assert types == ["passive", "passive"]


def test_emit_libparts_emit_footprints_when_present():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    libpart = _find(_find(tree, "libparts"), "libpart")
    fps = _find(libpart, "footprints")
    assert fps is not None
    fp_entries = _find_all(fps, "fp")
    assert len(fp_entries) == 1
    assert _strip(fp_entries[0][1]) == "R_*"


def test_emit_libparts_omit_footprints_when_empty():
    nl = _simple_netlist()
    nl.libparts[0].footprints_filter = []
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    libpart = _find(_find(tree, "libparts"), "libpart")
    assert _find(libpart, "footprints") is None


def test_emit_libparts_emit_fields_block():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    libpart = _find(_find(tree, "libparts"), "libpart")
    fields = _find(libpart, "fields")
    assert fields is not None
    field_entries = _find_all(fields, "field")
    # 3 standard fields: Datasheet, Reference, Value (sorted alphabetically).
    field_names = [_value(f, "name") for f in field_entries]
    assert field_names == ["Datasheet", "Reference", "Value"]


# ---------------------------------------------------------------------------
# (libraries ...) block
# ---------------------------------------------------------------------------


def test_emit_libraries_empty_when_none_set():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    libraries = _find(tree, "libraries")
    assert libraries is not None
    assert len(libraries) == 1  # just the head 'libraries' token


def test_emit_libraries_emits_one_library_block_per_entry():
    nl = _simple_netlist()
    nl.libraries = ["Device", "Connector"]
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    libs = _find_all(_find(tree, "libraries"), "library")
    assert len(libs) == 2
    logicals = [_value(library, "logical") for library in libs]
    assert logicals == ["Device", "Connector"]


# ---------------------------------------------------------------------------
# (nets ...) block
# ---------------------------------------------------------------------------


def test_emit_nets_emit_code_name_and_nodes():
    text = to_kicad_sexpr(_simple_netlist())
    tree = parse_sexp(text)
    nets = _find_all(_find(tree, "nets"), "net")
    assert len(nets) == 2
    n1 = nets[0]
    assert _value(n1, "code") == "1"
    assert _value(n1, "name") == "/VCC"
    nodes = _find_all(n1, "node")
    assert len(nodes) == 1
    assert _value(nodes[0], "ref") == "R1"
    assert _value(nodes[0], "pin") == "1"
    assert _value(nodes[0], "pinfunction") == "~"
    assert _value(nodes[0], "pintype") == "passive"


def test_emit_nodes_omit_pinfunction_when_empty():
    nl = _simple_netlist()
    nl.nets[0].terminals[0].pin_name = ""
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    net = _find(_find(tree, "nets"), "net")
    node = _find(net, "node")
    assert _find(node, "pinfunction") is None


def test_emit_nodes_omit_pintype_when_empty():
    nl = _simple_netlist()
    nl.nets[0].terminals[0].pin_type = ""
    text = to_kicad_sexpr(nl)
    tree = parse_sexp(text)
    net = _find(_find(tree, "nets"), "net")
    node = _find(net, "node")
    assert _find(node, "pintype") is None


# ---------------------------------------------------------------------------
# End-to-end — KiCadDesign-like flow on a synthesized 1-sheet schematic
# ---------------------------------------------------------------------------


def test_end_to_end_schematic_to_sexpr_round_trip():
    """Build a tiny schematic, compile it, emit, parse — verify the
    round trip exposes the right (ref, pin) tuples per net."""
    libR = _libR_full()
    sch = KiCadSchematic()
    sch.uuid = "root"
    sch.lib_symbols.append(libR)
    sch.symbols.append(_placed("Device:R", reference="R1", uuid="r1-uid"))

    netlist = compile_design_netlist(sch)
    text = to_kicad_sexpr(
        netlist, source_path="/tmp/r1.kicad_sch", date="X", tool="kicad_monkey"
    )
    tree = parse_sexp(text)

    # Components
    comps = _find_all(_find(tree, "components"), "comp")
    assert len(comps) == 1
    assert _value(comps[0], "ref") == "R1"

    # Libparts
    libparts = _find_all(_find(tree, "libparts"), "libpart")
    assert len(libparts) == 1
    assert _value(libparts[0], "lib") == "Device"
    assert _value(libparts[0], "part") == "R"

    # Nets — auto-named since R1 has no labels.
    nets = _find_all(_find(tree, "nets"), "net")
    refs_per_net = []
    for n in nets:
        nodes = _find_all(n, "node")
        refs_per_net.append({(_value(nd, "ref"), _value(nd, "pin")) for nd in nodes})
    assert {("R1", "1")} in refs_per_net
    assert {("R1", "2")} in refs_per_net

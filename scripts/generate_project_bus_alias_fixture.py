"""Generate the compact KiCad 10 project-bus-alias regression fixture.

The fixture deliberately keeps ``CTRL`` out of both schematic files.  KiCad
10 stores the alias in ``project_bus_alias_hierarchy.kicad_pro`` and applies it
design-wide while connecting the root and child buses through a hierarchical
sheet pin.

Reference outputs are generated separately with KiCad 10 so this script never
turns KiCad Monkey's own output into its oracle.
"""

from __future__ import annotations

import json
import shutil
import uuid
from pathlib import Path

from kicad_monkey import KiCadSchematic
from kicad_monkey.kicad_schematic import SheetInstancePath
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_sch_enums import LabelShape, PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_label import SchHierarchicalLabel, SchLabel
from kicad_monkey.kicad_sch_sheet import (
    SchSheet,
    SchSheetInstance,
    SchSheetPin,
    SchSheetProperty,
)
from kicad_monkey.kicad_sch_symbol import SchSymbol, SchSymbolInstance, SchSymbolPin
from kicad_monkey.kicad_sch_wire import SchBus, SchBusEntry, SchWire
from kicad_monkey.kicad_sym_pin import SymPin
from kicad_monkey.kicad_sym_property import SymProperty


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = PACKAGE_ROOT / "tests" / "cases" / "project_bus_alias_hierarchy"
INPUT_ROOT = FIXTURE_ROOT / "input"
PROJECT_NAME = "project_bus_alias_hierarchy"
CHILD_FILE = "member_sheet.kicad_sch"
NAMESPACE = uuid.UUID("c0782db8-e533-44dc-bda7-c88624f292f6")


def uid(name: str) -> str:
    return str(uuid.uuid5(NAMESPACE, name))


ROOT_UUID = uid("schematic:root")
SHEET_UUID = uid("sheet:member-sheet")
ROOT_PATH = f"/{ROOT_UUID}"
CHILD_PATH = f"{ROOT_PATH}/{SHEET_UUID}"


def terminal_symbol_definition() -> LibSymbol:
    pin = SymPin(
        electrical_type=PinElectricalType.PASSIVE,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        at_angle=180.0,
        length=2.54,
        number="1",
        name="~",
    )
    return LibSymbol(
        name="SYNTH_TERMINAL",
        properties=[
            SymProperty(key="Reference", value="TP", id=0),
            SymProperty(key="Value", value="SYNTH_TERMINAL", id=1),
        ],
        subsymbols=[
            LibSubSymbol(name="SYNTH_TERMINAL_1_0", unit=1, style=0, pins=[pin])
        ],
    )


def terminal(
    reference: str,
    x: float,
    y: float,
    path: str,
    *,
    reference_at: tuple[float, float] | None = None,
    value_at: tuple[float, float] | None = None,
) -> SchSymbol:
    reference_x, reference_y = reference_at or (x, y - 1.27)
    value_x, value_y = value_at or (x, y + 1.27)
    symbol = SchSymbol(
        lib_id="SYNTH_TERMINAL",
        at_x=x,
        at_y=y,
        at_angle=0.0,
        unit=1,
        convert=1,
        uuid=uid(f"symbol:{reference}"),
    )
    symbol.properties = [
        SymProperty(
            key="Reference",
            value=reference,
            id=0,
            at_x=reference_x,
            at_y=reference_y,
        ),
        SymProperty(
            key="Value",
            value="SYNTH_TERMINAL",
            id=1,
            at_x=value_x,
            at_y=value_y,
        ),
    ]
    symbol.pins = [SchSymbolPin(number="1", uuid=uid(f"pin:{reference}:1"))]
    symbol.instances = [
        SchSymbolInstance(project=PROJECT_NAME, path=path, reference=reference, unit=1)
    ]
    return symbol


def wire(name: str, start: tuple[float, float], end: tuple[float, float]) -> SchWire:
    return SchWire(points=[start, end], uuid=uid(f"wire:{name}"))


def member_bus(
    prefix: str,
    *,
    hierarchy: bool,
    offset_x: float = 0.0,
    offset_y: float = 0.0,
) -> tuple[
    list[SchWire],
    list[SchBusEntry],
    list[SchBus],
    list[SchLabel],
    list[SchHierarchicalLabel],
]:
    def shifted(value: float, offset: float) -> float:
        return round(value + offset, 6)

    terminal_x = shifted(20.0, offset_x)
    upper_y = shifted(32.46, offset_y)
    lower_y = shifted(37.54, offset_y)
    bus_y = shifted(35.0, offset_y)
    wire_end_x = shifted(40.0, offset_x)
    bus_start_x = shifted(42.54, offset_x)
    bus_end_x = shifted(60.0, offset_x)

    wires = [
        wire(f"{prefix}:CTRL_A", (terminal_x, upper_y), (wire_end_x, upper_y)),
        wire(f"{prefix}:CTRL_B", (terminal_x, lower_y), (wire_end_x, lower_y)),
    ]
    entries = [
        SchBusEntry(
            at_x=wire_end_x,
            at_y=upper_y,
            size_x=2.54,
            size_y=2.54,
            uuid=uid(f"entry:{prefix}:CTRL_A"),
        ),
        SchBusEntry(
            at_x=wire_end_x,
            at_y=lower_y,
            size_x=2.54,
            size_y=-2.54,
            uuid=uid(f"entry:{prefix}:CTRL_B"),
        ),
    ]
    buses = [
        SchBus(
            points=[(bus_start_x, bus_y), (bus_end_x, bus_y)],
            uuid=uid(f"bus:{prefix}:CTRL"),
        )
    ]
    labels = [
        SchLabel(
            text="CTRL_A",
            at_x=shifted(25.0, offset_x),
            at_y=upper_y,
            uuid=uid(f"label:{prefix}:CTRL_A"),
        ),
        SchLabel(
            text="CTRL_B",
            at_x=shifted(25.0, offset_x),
            at_y=lower_y,
            uuid=uid(f"label:{prefix}:CTRL_B"),
        ),
        SchLabel(
            text="{CTRL}",
            at_x=shifted(50.0, offset_x),
            at_y=bus_y,
            uuid=uid(f"label:{prefix}:CTRL"),
        ),
    ]
    hierarchical_labels = (
        [
            SchHierarchicalLabel(
                text="{CTRL}",
                shape=LabelShape.INPUT,
                at_x=bus_end_x,
                at_y=bus_y,
                uuid=uid("hier-label:child:CTRL"),
            )
        ]
        if hierarchy
        else []
    )
    return wires, entries, buses, labels, hierarchical_labels


def new_schematic(name: str) -> KiCadSchematic:
    schematic = KiCadSchematic()
    schematic.uuid = ROOT_UUID if name == "root" else uid(f"schematic:{name}")
    schematic.lib_symbols.append(terminal_symbol_definition())
    return schematic


def build_root() -> KiCadSchematic:
    schematic = new_schematic("root")
    schematic.symbols.extend(
        [
            terminal(
                "TP1",
                79.38,
                58.42,
                ROOT_PATH,
                reference_at=(77.216, 52.832),
                value_at=(84.074, 54.864),
            ),
            terminal(
                "TP2",
                79.38,
                63.5,
                ROOT_PATH,
                reference_at=(78.74, 64.77),
                value_at=(85.344, 66.802),
            ),
        ]
    )
    wires, entries, buses, labels, hierarchical_labels = member_bus(
        "root", hierarchy=False, offset_x=59.38, offset_y=25.96
    )
    schematic.wires.extend(wires)
    schematic.bus_entries.extend(entries)
    schematic.buses.extend(buses)
    schematic.labels.extend(labels)
    schematic.hierarchical_labels.extend(hierarchical_labels)

    sheet = SchSheet(
        at_x=119.38,
        at_y=53.34,
        size_x=25.4,
        size_y=15.24,
        uuid=SHEET_UUID,
    )
    sheet.properties = [
        SchSheetProperty(
            key="Sheetname",
            value="MEMBER_SHEET",
            at_x=126.746,
            at_y=52.324,
        ),
        SchSheetProperty(
            key="Sheetfile",
            value=CHILD_FILE,
            at_x=131.064,
            at_y=70.104,
        ),
    ]
    sheet.pins = [
        SchSheetPin(
            name="{CTRL}",
            shape=LabelShape.INPUT,
            at_x=119.38,
            at_y=60.96,
            at_angle=180.0,
            uuid=uid("sheet-pin:CTRL"),
        )
    ]
    sheet.instances = [
        SchSheetInstance(
            project=PROJECT_NAME,
            path=ROOT_PATH,
            page="2",
        )
    ]
    schematic.sheets.append(sheet)
    schematic.sheet_instances = [SheetInstancePath(path="/", page="1")]
    return schematic


def build_child() -> KiCadSchematic:
    schematic = new_schematic("child")
    schematic.symbols.extend(
        [
            terminal(
                "TP101",
                131.45,
                100.33,
                CHILD_PATH,
                reference_at=(130.048, 94.996),
                value_at=(135.382, 97.282),
            ),
            terminal(
                "TP102",
                131.45,
                105.41,
                CHILD_PATH,
                reference_at=(129.54, 106.934),
                value_at=(135.128, 108.966),
            ),
        ]
    )
    wires, entries, buses, labels, hierarchical_labels = member_bus(
        "child", hierarchy=True, offset_x=111.45, offset_y=67.87
    )
    schematic.wires.extend(wires)
    schematic.bus_entries.extend(entries)
    schematic.buses.extend(buses)
    schematic.labels.extend(labels)
    schematic.hierarchical_labels.extend(hierarchical_labels)
    return schematic


def schematic_text(schematic: KiCadSchematic) -> str:
    return schematic.to_text().replace(
        "  (version 20250114\n  )", "  (version 20250114)"
    )


def write_fixture() -> None:
    if INPUT_ROOT.exists():
        shutil.rmtree(INPUT_ROOT)
    FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
    INPUT_ROOT.mkdir(parents=True)
    (FIXTURE_ROOT / "reference_output").mkdir(exist_ok=True)

    (INPUT_ROOT / f"{PROJECT_NAME}.kicad_sch").write_text(
        schematic_text(build_root()), encoding="utf-8"
    )
    (INPUT_ROOT / CHILD_FILE).write_text(
        schematic_text(build_child()), encoding="utf-8"
    )

    project = {
        "meta": {"filename": f"{PROJECT_NAME}.kicad_pro", "version": 1},
        "schematic": {
            "bus_aliases": {"CTRL": ["CTRL_A", "CTRL_B"]},
            "subpart_first_id": 65,
            "subpart_id_separator": 0,
        },
        "text_variables": {},
    }
    (INPUT_ROOT / f"{PROJECT_NAME}.kicad_pro").write_text(
        json.dumps(project, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )

    metadata = {
        "id": "project_bus_alias_hierarchy",
        "origin": "generated_synthetic_project",
        "generator": "scripts/generate_project_bus_alias_fixture.py",
        "kicad_format": 10,
        "purpose": (
            "Project-level bus alias expansion across a hierarchical sheet "
            "using real buses, entries, member labels, and terminal-bearing nets."
        ),
        "oracle": "KiCad 10 CLI kicadxml netlist export",
        "oracle_version": "10.0.5",
        "reference_outputs": ["project_bus_alias_hierarchy.xml"],
        "license": "Project-authored test fixture",
    }
    (FIXTURE_ROOT / "case_metadata.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )

    (FIXTURE_ROOT / "README.md").write_text(
        """# Project-level bus alias hierarchy fixture

This original KiCad 10 fixture mirrors the useful topology of KiCad's
`issue24220` QA project without copying its files. `CTRL` is declared only in
the `.kicad_pro` file and expands to `CTRL_A` and `CTRL_B`. Both members are
terminal-bearing nets connected across the `MEMBER_SHEET` hierarchy boundary.

The XML netlist in `reference_output/` was generated with KiCad CLI 10.0.5.
Its timestamp and absolute source path are canonicalized. `output/` is
transient, and local editor-state files such as `.kicad_prl` must not be
committed.
""",
        encoding="utf-8",
    )


def main() -> int:
    write_fixture()
    print(f"Wrote {FIXTURE_ROOT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

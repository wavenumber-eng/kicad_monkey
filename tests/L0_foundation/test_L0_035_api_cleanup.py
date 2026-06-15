"""Focused public API cleanup coverage."""

from __future__ import annotations

from enum import IntEnum

import pytest

from kicad_monkey import (
    KiCadObjectCollection,
    KiCadSchematic,
    KiCadSymbolLib,
    LibSymbol,
    PropertyId,
    SchSheet,
    SchSymbol,
    StandardPropertyKey,
    StandardSheetPropertyKey,
    SymProperty,
)
from kicad_monkey._api_markers import public_api
from kicad_monkey.kicad_base import get_at
from kicad_monkey.kicad_sch_sheet import SchSheetProperty


def test_public_api_marks_classes_and_descriptors():
    @public_api
    class Marked:
        @public_api
        @property
        def value(self) -> int:
            return 1

        @public_api
        @classmethod
        def build(cls):
            return cls()

    assert Marked.__public_api__ is True
    assert Marked.value.fget.__public_api__ is True
    assert Marked.__dict__["build"].__func__.__public_api__ is True


def test_property_ids_are_named_int_enum_values():
    assert issubclass(PropertyId, IntEnum)
    assert PropertyId.REFERENCE == 0
    assert int(PropertyId.USER_START) == 5


def test_get_at_accepts_kicad_unlocked_flag_after_coordinates():
    assert get_at(["fp_text", ["at", "0", "0", "unlocked"]]) == (0.0, 0.0, 0.0)
    assert get_at(["fp_text", ["at", "1.5", "-2.5", "90", "unlocked"]]) == (1.5, -2.5, 90.0)


def test_symbol_library_constructor_and_ir_alias():
    symbol = LibSymbol("Device:R")
    lib = KiCadSymbolLib(
        version=20241209,
        generator="kicad_symbol_editor",
        generator_version="9.0",
        symbols=[symbol],
    )

    assert lib.get_symbol("Device:R") is symbol
    doc = lib.to_ir("Device:R")
    assert doc.source_kind == "SYM"
    assert doc.document_id == "Device:R"
    assert doc.records[0].kind == "lib_symbol"


def test_lib_symbol_property_mutation_and_ir_entrypoint():
    symbol = LibSymbol("Device:R")

    symbol.upsert_property(StandardPropertyKey.REFERENCE, "R")
    symbol.upsert_property(StandardPropertyKey.VALUE, "10k")
    custom = symbol.upsert_property("Manufacturer", "Wavenumber")

    assert symbol.reference == "R"
    assert symbol.value == "10k"
    assert custom.id == int(PropertyId.USER_START)
    assert symbol.set_property_value("Manufacturer", "WN") is True
    assert symbol.get_property_value("Manufacturer") == "WN"
    assert symbol.remove_property("Manufacturer") is True

    doc = symbol.to_ir()
    assert doc.source_kind == "SYM"
    assert doc.document_id == "Device:R"


def test_schematic_object_query_and_property_mutation_facade():
    schematic = KiCadSchematic()
    symbol = SchSymbol(lib_id="Device:R")
    symbol.upsert_property(StandardPropertyKey.REFERENCE, "R1")
    symbol.upsert_property(StandardPropertyKey.VALUE, "10k")
    sheet = SchSheet()
    sheet.upsert_property(StandardSheetPropertyKey.SHEET_NAME, "Power")
    sheet.upsert_property(StandardSheetPropertyKey.SHEET_FILE, "power.kicad_sch")

    schematic.add_object(symbol)
    schematic.add_object(sheet)

    assert isinstance(schematic.objects, KiCadObjectCollection)
    assert schematic.objects.first(SchSymbol, lib_id="Device:R") is symbol
    assert schematic.objects.of_type("SchSheet").first() is sheet
    assert schematic.properties.first(SymProperty, key="Reference").value == "R1"
    assert schematic.properties.first(SchSheetProperty, key="Sheetname").value == "Power"
    assert sheet.sheet_file == "power.kicad_sch"

    assert symbol.get_property(StandardPropertyKey.REFERENCE) == "R1"
    assert symbol.get_property_object(StandardPropertyKey.VALUE).value == "10k"
    assert symbol.set_property_value(StandardPropertyKey.FOOTPRINT, "Resistor_SMD:R_0402", create=True)
    assert symbol.footprint == "Resistor_SMD:R_0402"
    assert symbol.remove_property(StandardPropertyKey.FOOTPRINT)

    with pytest.raises(TypeError):
        schematic.objects.append(symbol)

    assert schematic.remove_object(symbol) is True
    assert schematic.get_symbol_by_reference("R1") is None


def test_schematic_to_ir_entrypoint():
    schematic = KiCadSchematic()

    doc = schematic.to_ir(document_id="empty")

    assert doc.source_kind == "SCH"
    assert doc.document_id == "empty"

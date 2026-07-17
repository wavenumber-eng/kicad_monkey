"""Focused PCB and footprint public API cleanup coverage."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from kicad_monkey import KiCadFootprint, KiCadObjectCollection, KiCadPcb
from kicad_monkey.kicad_fp_line import FpLine
from kicad_monkey.kicad_pcb_footprint import Footprint
from kicad_monkey.kicad_pcb_gr_line import GrLine
from kicad_monkey.kicad_pcb_other import BoardProperty
from kicad_monkey.kicad_property import Property


PLOTTER_IR_SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "docs"
    / "contracts"
    / "kicad_plotter_ir_a0.schema.json"
)


def _plotter_ir_contract_validator() -> Draft202012Validator:
    schema = json.loads(PLOTTER_IR_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_pcb_object_query_properties_and_ir_entrypoint():
    pcb = KiCadPcb()
    prop = pcb.upsert_property("Lifecycle", "Prototype")
    line = GrLine(0.0, 0.0, 1.0, 0.0, layer="Edge.Cuts")

    pcb.add_object(line)

    assert isinstance(pcb.objects, KiCadObjectCollection)
    assert pcb.objects.first(GrLine, layer="Edge.Cuts") is line
    assert pcb.objects.first(BoardProperty, key="Lifecycle") is prop
    assert pcb.get_property_value("Lifecycle") == "Prototype"
    assert pcb.set_property_value("Lifecycle", "Release") is True
    assert pcb.get_property("Lifecycle") == "Release"

    doc = pcb.to_ir(document_id="board")
    assert doc.source_kind == "PCB"
    assert doc.document_id == "board"
    assert doc.records[0].kind == "gr_line"

    with pytest.raises(TypeError):
        pcb.objects.append(line)

    assert pcb.remove_object(line) is True
    assert pcb.objects.first(GrLine) is None
    assert pcb.remove_property("Lifecycle") is True


def test_standalone_footprint_object_query_properties_and_ir_entrypoint():
    footprint = KiCadFootprint()
    footprint.name = "R_0402"
    prop = footprint.upsert_property("Reference", "R")
    line = FpLine(0.0, 0.0, 1.0, 0.0)

    footprint.add_object(line)

    assert footprint.reference == "R"
    assert footprint.objects.first(FpLine) is line
    assert footprint.objects.first(Property, name="Reference") is prop
    assert footprint.set_property_value("Value", "10k", create=True) is True
    assert footprint.value == "10k"

    doc = footprint.to_ir()
    assert doc.source_kind == "MOD"
    assert doc.document_id == "R_0402"
    assert doc.records[0].kind == "footprint"

    assert footprint.remove_object(line) is True
    assert footprint.remove_property("Value") is True


def test_embedded_footprint_object_query_properties_and_ir_entrypoint():
    footprint = Footprint("Device:R", at_x=5.0, at_y=6.0)
    prop = footprint.upsert_property("Reference", "R1")
    line = FpLine(0.0, 0.0, 1.0, 0.0)

    footprint.add_object(line)

    assert footprint.get_property_value("Reference") == "R1"
    assert footprint.objects.first(FpLine) is line
    assert footprint.objects.first(Property, name="Reference") is prop

    doc = footprint.to_ir(document_id="R1")
    assert doc.source_kind == "PCB_FOOTPRINT"
    assert doc.document_id == "R1"
    assert doc.records[0].object_id == "Device:R"
    _plotter_ir_contract_validator().validate(doc.to_dict())

    assert footprint.remove_object(line) is True
    assert footprint.remove_property("Reference") is True

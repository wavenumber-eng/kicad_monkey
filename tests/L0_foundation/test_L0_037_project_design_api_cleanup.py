"""Focused project/design public API cleanup coverage."""

from __future__ import annotations

import json

import pytest

from kicad_monkey import (
    KiCadDesign,
    KiCadObjectCollection,
    KiCadPcb,
    KiCadSchematic,
    KiCadSchematicInstance,
)
from kicad_monkey.kicad_design_json import KICAD_DESIGN_JSON_SCHEMA
from kicad_monkey.kicad_project import KiCadProject, ProjectVariant


_MIN_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-3333-4444-555555555555")
  (paper "A4")
)
"""

_MIN_PCB_TEXT = """(kicad_pcb
  (version 20241229)
  (generator "pcbnew")
  (generator_version "9.0")
  (general (thickness 1.6) (legacy_teardrops no))
  (paper "A4")
  (layers)
  (embedded_fonts no)
)
"""

_HIER_TOP_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
  (paper "A4")
  (sheet
    (at 10 10)
    (size 20 20)
    (uuid "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
    (property "Sheetname" "POWER_A"
      (at 10 10 0)
      (effects (font (size 1.27 1.27)))
    )
    (property "Sheetfile" "child.kicad_sch"
      (at 10 12 0)
      (effects (font (size 1.27 1.27)))
    )
    (instances
      (project "demo"
        (path "/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb" (page "2"))
      )
    )
  )
  (sheet
    (at 40 10)
    (size 20 20)
    (uuid "cccccccc-cccc-cccc-cccc-cccccccccccc")
    (property "Sheetname" "POWER_B"
      (at 40 10 0)
      (effects (font (size 1.27 1.27)))
    )
    (property "Sheetfile" "child.kicad_sch"
      (at 40 12 0)
      (effects (font (size 1.27 1.27)))
    )
    (instances
      (project "demo"
        (path "/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/cccccccc-cccc-cccc-cccc-cccccccccccc" (page "3"))
      )
    )
  )
  (sheet_instances
    (path "/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" (page "1"))
  )
)
"""

_HIER_CHILD_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "dddddddd-dddd-dddd-dddd-dddddddddddd")
  (paper "A4")
  (text "linked child note"
    (exclude_from_sim no)
    (at 1 2 0)
    (effects (font (size 1.27 1.27)) (href "https://example.test/child"))
    (uuid "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee")
  )
)
"""


def _write_project(path, text_variables=None):
    path.write_text(
        json.dumps(
            {
                "text_variables": dict(text_variables or {}),
                "schematic": {
                    "variants": [
                        {"name": "Default"},
                        {"name": "Alt", "description": "alternate assembly"},
                    ]
                },
            },
            indent=2,
        ),
        encoding="utf-8",
    )


def _write_schematic(path):
    path.write_text(_MIN_SCH_TEXT, encoding="utf-8")


def _write_pcb(path):
    path.write_text(_MIN_PCB_TEXT, encoding="utf-8")


def test_project_json_text_variable_and_variant_helpers(tmp_path):
    project = KiCadProject.from_json_dict(
        {
            "text_variables": {"TITLE": "Demo"},
            "schematic": {
                "variants": [{"name": "Default"}],
                "bus_aliases": {
                    "I2C": ["SCL", "SDA"],
                    "USB": ["D+", "D-"],
                    "FILTERED": ["VALID", 7, None, "", {"not": "a member"}],
                    "BROKEN": "not-a-list",
                },
            },
        }
    )

    assert project.get_text_variable("TITLE") == "Demo"
    assert project.get_variant("Default") == ProjectVariant("Default")
    assert list(project.iter_variants()) == [ProjectVariant("Default")]
    assert project.bus_aliases == {
        "I2C": ["SCL", "SDA"],
        "USB": ["D+", "D-"],
        "FILTERED": ["VALID", ""],
    }

    project.set_text_variable("REV", "A")
    assert project.text_variables["REV"] == "A"
    assert project.raw["text_variables"]["REV"] == "A"
    assert project.remove_text_variable("REV") is True
    assert "REV" not in project.raw["text_variables"]

    out = tmp_path / "demo.kicad_pro"
    project.to_file(out)
    loaded = KiCadProject.from_file(out)
    assert loaded.to_json()["text_variables"] == {"TITLE": "Demo"}
    assert loaded.bus_aliases == project.bus_aliases


def test_design_from_file_dispatches_by_suffix(tmp_path):
    pro = tmp_path / "demo.kicad_pro"
    sch = tmp_path / "demo.kicad_sch"
    pcb = tmp_path / "demo.kicad_pcb"
    _write_project(pro, {"TITLE": "Demo"})
    _write_schematic(sch)
    _write_pcb(pcb)

    assert KiCadDesign.from_file(pro).project_path == pro
    assert isinstance(KiCadDesign.from_file(sch).top_schematic, KiCadSchematic)
    assert KiCadDesign.from_file(pcb).pcb_path == pcb

    with pytest.raises(ValueError, match="unsupported"):
        KiCadDesign.from_file(tmp_path / "demo.txt")


def test_design_document_query_and_pcb_ir(tmp_path):
    pro = tmp_path / "demo.kicad_pro"
    sch = tmp_path / "demo.kicad_sch"
    pcb = tmp_path / "demo.kicad_pcb"
    _write_project(pro, {"TITLE": "Demo"})
    _write_schematic(sch)
    _write_pcb(pcb)

    design = KiCadDesign.from_project_file(pro)

    assert isinstance(design.objects, KiCadObjectCollection)
    assert isinstance(design.objects.first(KiCadProject), KiCadProject)
    assert isinstance(design.objects.first(KiCadSchematic), KiCadSchematic)
    assert isinstance(design.objects.first(KiCadPcb), KiCadPcb)

    doc = design.to_pcb_ir(document_id="board")
    assert doc.source_kind == "PCB"
    assert doc.document_id == "board"


def test_design_schematic_mutators_and_json_text(tmp_path):
    pro = tmp_path / "demo.kicad_pro"
    sch = tmp_path / "demo.kicad_sch"
    _write_project(pro, {})
    _write_schematic(sch)

    design = KiCadDesign.from_project_file(pro)
    extra = KiCadSchematic.from_text(_MIN_SCH_TEXT)

    assert design.add_schematic(extra) is extra
    assert list(design.iter_schematics())[-1] is extra
    assert design.remove_schematic(extra) is True

    text = design.to_json_text(include_indexes=False)
    payload = json.loads(text)
    assert payload["schema"] == KICAD_DESIGN_JSON_SCHEMA


def test_design_schematic_instance_navigation(tmp_path):
    pro = tmp_path / "demo.kicad_pro"
    top = tmp_path / "demo.kicad_sch"
    child = tmp_path / "child.kicad_sch"
    _write_project(pro, {})
    top.write_text(_HIER_TOP_SCH_TEXT, encoding="utf-8")
    child.write_text(_HIER_CHILD_SCH_TEXT, encoding="utf-8")

    design = KiCadDesign.from_project_file(pro)
    instances = list(design.iter_schematic_instances())

    assert all(isinstance(instance, KiCadSchematicInstance) for instance in instances)
    assert [instance.sheet_name for instance in instances] == [
        "demo",
        "POWER_A",
        "POWER_B",
    ]
    assert [instance.sheet_path for instance in instances] == [
        "/",
        "/POWER_A/",
        "/POWER_B/",
    ]
    assert [instance.sheet_number for instance in instances] == [1, 2, 3]
    assert [instance.sheet_count for instance in instances] == [3, 3, 3]

    child_by_object = design.schematic_instances_for(instances[1].schematic)
    child_by_path = design.schematic_instances_for(child)
    assert [instance.sheet_path for instance in child_by_object] == [
        "/POWER_A/",
        "/POWER_B/",
    ]
    assert [instance.sheet_path for instance in child_by_path] == [
        "/POWER_A/",
        "/POWER_B/",
    ]

    assert design.find_schematic_instances(sheet_name="POWER_B")[0].sheet_path == "/POWER_B/"
    assert design.find_schematic_instances(sheet_instance_path=instances[2].sheet_instance_path)[
        0
    ].sheet_path == "/POWER_B/"

    top_instance = instances[0]
    assert [instance.sheet_path for instance in design.child_schematic_instances(top_instance)] == [
        "/POWER_A/",
        "/POWER_B/",
    ]
    assert design.parent_schematic_instance(instances[1]) == top_instance
    assert design.parent_schematic_instance("/POWER_B/") == top_instance
    assert design.parent_schematic_instance(top_instance) is None

    power_b = instances[2]
    assert power_b.sheet_path_uuids == "/cccccccc-cccc-cccc-cccc-cccccccccccc/"
    assert (
        power_b.sheet_instance_path
        == "/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/cccccccc-cccc-cccc-cccc-cccccccccccc"
    )
    assert power_b.sheet_symbol_uid == "cccccccc-cccc-cccc-cccc-cccccccccccc"
    assert power_b.sheet_file == "child.kicad_sch"
    assert power_b.parent_sheet_path == "/"

    kwargs = power_b.ir_kwargs(document_id="power-b")
    assert kwargs == {
        "sheet_index": 3,
        "sheet_count": 3,
        "sheet_path": "/POWER_B/",
        "sheet_instance_path": power_b.sheet_instance_path,
        "sheet_name": "POWER_B",
        "document_id": "power-b",
    }
    doc = design.to_schematic_instance_ir(power_b, document_id="power-b")
    assert doc.document_id == "power-b"
    assert doc.source_kind == "SCH"
    assert doc.records[0].kind == "sheet_header"
    text_op = next(
        op
        for record in doc.records
        for op in record.operations
        if op.payload.get("text") == "linked child note"
    )
    assert (
        text_op.payload["context"]["hyperlink"]["href"]
        == "https://example.test/child"
    )

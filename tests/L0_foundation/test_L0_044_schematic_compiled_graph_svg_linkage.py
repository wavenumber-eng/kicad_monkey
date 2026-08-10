"""Compiled schematic graph linkage for enriched schematic SVG output."""

from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace

import pytest
from jsonschema import Draft202012Validator

from kicad_monkey import (
    KiCadDesign,
    KiCadSvgRenderOptions,
    render_ir_to_svg,
    validate_schematic_svg_compiled_graph_view,
)
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_sch_enums import PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_sheet import SchSheet, SchSheetProperty
from kicad_monkey.kicad_sch_symbol import SchSymbol
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_schematic_svg_enrichment import (
    KICAD_SCHEMATIC_COMPILED_GRAPH_VIEW_SCHEMA,
    KICAD_SCHEMATIC_GRAPH_ARTIFACT_KEY,
    KICAD_SCHEMATIC_GRAPH_LINKAGE_CONTRACT,
    compiled_schematic_graph_page_view,
    resolve_compiled_schematic_graph_page_occurrence,
    schematic_root_svg_attrs,
    schematic_svg_enrichment_metadata_element,
    schematic_svg_enrichment_payload,
)
from kicad_monkey.kicad_sym_pin import SymPin
from kicad_monkey.kicad_sym_property import SymProperty


def _linked_design(tmp_path: Path) -> KiCadDesign:
    pin = SymPin(
        electrical_type=PinElectricalType.PASSIVE,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        length=2.54,
        number="1",
        name="IN",
    )
    library = LibSymbol(
        name="Device:R",
        subsymbols=[LibSubSymbol(name="Device:R_1_0", unit=1, pins=[pin])],
    )
    symbol = SchSymbol(lib_id="Device:R", uuid="symbol-source")
    symbol.properties = [
        SymProperty(key="Reference", value="R1"),
        SymProperty(key="Value", value="10k"),
    ]
    schematic = KiCadSchematic()
    schematic.uuid = "root-source"
    schematic.source_path = tmp_path / "demo.kicad_sch"
    schematic.lib_symbols.append(library)
    schematic.symbols.append(symbol)
    return KiCadDesign(schematics=[schematic])


def _linked_svg(tmp_path: Path):
    design = _linked_design(tmp_path)
    instance = design.schematic_instances()[0]
    design_payload = design.to_json()
    graph = design_payload["compiled_schematic_graph"]
    graph_artifact = "../demo_compiled_schematic_graph.json"
    payload = schematic_svg_enrichment_payload(
        design_payload,
        source_path=instance.source_path,
        sheet_name=instance.sheet_name,
        sheet_path=instance.sheet_path,
        sheet_instance_path=instance.sheet_instance_path,
        compiled_schematic_graph=graph,
        schematic_instance=instance,
        compiled_graph_artifact=graph_artifact,
    )
    view = payload["compiled_schematic_graph_view"]
    svg = render_ir_to_svg(
        design.to_schematic_instance_ir(instance),
        options=KiCadSvgRenderOptions.enriched_default(),
        root_extra_attrs=schematic_root_svg_attrs(
            source_path=instance.source_path,
            sheet_name=instance.sheet_name,
            sheet_path=instance.sheet_path,
            compiled_graph_view=view,
        ),
        metadata_elements=[schematic_svg_enrichment_metadata_element(payload)],
    )
    return design, instance, payload, graph, view, svg


def test_compiled_graph_page_view_is_deterministic_and_schema_valid(tmp_path):
    _design, instance, payload, graph, view, svg = _linked_svg(tmp_path)

    assert resolve_compiled_schematic_graph_page_occurrence(graph, instance)["id"] == view[
        "page_occurrence_ref"
    ]
    assert view == compiled_schematic_graph_page_view(
        graph,
        instance,
        graph_artifact="../demo_compiled_schematic_graph.json",
    )
    assert view["schema"] == KICAD_SCHEMATIC_COMPILED_GRAPH_VIEW_SCHEMA
    assert view["artifact_key"] == KICAD_SCHEMATIC_GRAPH_ARTIFACT_KEY
    assert view["linkage_contract"] == KICAD_SCHEMATIC_GRAPH_LINKAGE_CONTRACT
    assert view["graphical_artifact_link_refs"]
    assert view["element_to_graphical_artifact_link_refs"]
    assert view["target_to_element_ids"]
    assert (
        f'data-compiled-graph-page-occurrence-ref="{view["page_occurrence_ref"]}"'
        in svg
    )

    schema_path = (
        Path(__file__).parents[2]
        / "docs"
        / "contracts"
        / "schematic_svg_enrichment_a0.schema.json"
    )
    Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8"))).validate(
        payload
    )


def test_compiled_graph_svg_view_rejects_wrong_page_and_bad_selectors(tmp_path):
    _design, _instance, _payload, graph, view, svg = _linked_svg(tmp_path)

    counts = validate_schematic_svg_compiled_graph_view(svg, graph, view)
    assert counts["graph_link_count"] == len(view["graphical_artifact_link_refs"])
    assert counts["resolved_svg_identity_count"] == len(
        view["element_to_graphical_artifact_link_refs"]
    )

    with pytest.raises(ValueError, match="exactly one match"):
        resolve_compiled_schematic_graph_page_occurrence(
            graph,
            SimpleNamespace(sheet_instance_path="/not-a-real-page"),
        )

    element_id = next(iter(view["element_to_graphical_artifact_link_refs"]))
    missing = svg.replace(f'id="{element_id}"', 'id="missing-selector"', 1)
    with pytest.raises(ValueError, match="missing"):
        validate_schematic_svg_compiled_graph_view(missing, graph, view)

    duplicate = svg.replace("</svg>", f'<g id="{element_id}" />\n</svg>')
    with pytest.raises(ValueError, match="ambiguous"):
        validate_schematic_svg_compiled_graph_view(duplicate, graph, view)


def test_reused_sheet_views_scope_shared_source_ids_by_page_occurrence(tmp_path):
    child = _linked_design(tmp_path).top_schematic
    assert child is not None
    child.uuid = "child-source"
    child.source_path = tmp_path / "child.kicad_sch"

    root = KiCadSchematic()
    root.uuid = "root-source"
    root.source_path = tmp_path / "root.kicad_sch"
    for name, uuid in (("A", "placement-a"), ("B", "placement-b")):
        sheet = SchSheet(uuid=uuid)
        sheet.properties = [
            SchSheetProperty(key="Sheetname", value=name),
            SchSheetProperty(key="Sheetfile", value="child.kicad_sch"),
        ]
        root.sheets.append(sheet)
    root.sub_schematics["child.kicad_sch"] = child
    design = KiCadDesign(schematics=[root])
    graph = design.to_json()["compiled_schematic_graph"]
    child_instances = design.schematic_instances()[1:]

    views = [
        compiled_schematic_graph_page_view(
            graph,
            instance,
            graph_artifact="../root_compiled_schematic_graph.json",
        )
        for instance in child_instances
    ]

    assert len(views) == 2
    assert views[0]["page_occurrence_ref"] != views[1]["page_occurrence_ref"]
    assert "symbol-source" in views[0]["element_to_graphical_artifact_link_refs"]
    assert "symbol-source" in views[1]["element_to_graphical_artifact_link_refs"]
    assert views[0]["graphical_artifact_link_refs"] != views[1][
        "graphical_artifact_link_refs"
    ]

"""Real-project acceptance for directive and schematic rule-area plotting."""

from __future__ import annotations

from kicad_monkey import KiCadDesign, KiCadPlotterOpKind
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    resolve_kicad_manifest_path,
)


def test_speedy_ethernet_directive_and_rule_areas_reach_plotter_ir() -> None:
    case = get_kicad_corpus_case("real_world/speedy_processing_module")
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None
    design = KiCadDesign.from_project_file(project_path)
    ethernet = next(
        instance
        for instance in design.schematic_instances()
        if instance.sheet_name == "Ethernet"
    )
    records = design.to_schematic_instance_ir(ethernet).records

    directive = next(
        record
        for record in records
        if record.uuid == "eb0a9f9e-3abb-4197-9c92-14dfcbc39f4b"
    )
    assert directive.kind == "netclass_flag"
    assert directive.extras == {
        "at_x_nm": 567_690_000,
        "at_y_nm": 262_890_000,
        "shape": "round",
        "length_nm": 2_540_000,
    }
    assert [operation.kind for operation in directive.operations[:2]] == [
        KiCadPlotterOpKind.THICK_SEGMENT,
        KiCadPlotterOpKind.CIRCLE,
    ]
    assert {
        operation.payload["stroke_color"] for operation in directive.operations[:2]
    } == {"#484848FF"}
    assert [
        operation.payload["text"]
        for operation in directive.operations
        if operation.kind == KiCadPlotterOpKind.TEXT
    ] == ["100Z_DIFF", "ETH_PHY"]

    rule_areas = {
        record.uuid: record
        for record in records
        if record.uuid
        in {
            "778d9d7d-c436-4233-86bd-4972856324b9",
            "e95d894e-3ee0-4593-8b16-1cb9ba386a97",
        }
    }
    assert set(rule_areas) == {
        "778d9d7d-c436-4233-86bd-4972856324b9",
        "e95d894e-3ee0-4593-8b16-1cb9ba386a97",
    }
    for record in rule_areas.values():
        assert record.kind == "rule_area"
        assert len(record.operations) == 1
        operation = record.operations[0]
        assert operation.kind == KiCadPlotterOpKind.PLOT_POLY
        assert operation.payload["points"][0] == operation.payload["points"][-1]
        assert operation.payload["fill"] == "NO_FILL"
        assert operation.payload["line_style"] == "DASH"
        assert operation.payload["stroke_color"] == "#C20000FF"
        assert operation.payload["width_nm"] == 152_400

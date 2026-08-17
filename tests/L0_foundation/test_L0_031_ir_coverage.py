"""L0 coverage-report tests for KiCad plotter IR histograms."""

from __future__ import annotations

import json

from kicad_monkey.kicad_ir_coverage import (
    SCHEMA,
    build_ir_coverage_report,
    render_ir_coverage_markdown,
)


def test_build_ir_coverage_report_counts_ir_and_recorder_outputs(tmp_path):
    root = tmp_path / "kicad"
    generated_root = tmp_path / "generated"
    root.mkdir()
    (root / "manifest.json").write_text(
        json.dumps(
            {
                "cases": [
                    {
                        "id": "real_world_recorder/demo.1",
                        "name": "demo.1",
                        "status": "active",
                        "origin": "real_world_recorder",
                        "recorder_file": "projects/demo/reference_output/recorder_dumps/demo.1.json",
                        "output_root": "projects/demo/output",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )

    ir_dir = generated_root / "projects" / "demo" / "output" / "schematic_ir"
    ir_dir.mkdir(parents=True)
    (ir_dir / "demo.json").write_text(
        json.dumps(
            {
                "schema": "kicad.plotter_ir.a0",
                "records": [
                    {
                        "uuid": "sheet",
                        "kind": "sheet_header",
                        "object_id": "sheet",
                        "operations": [
                            {
                                "kind": "Rect",
                                "index": 0,
                                "x1": 0,
                                "y1": 0,
                                "x2": 10,
                                "y2": 10,
                                "fill": "NO_FILL",
                                "stroke_color": "#840000FF",
                            }
                        ],
                    },
                    {
                        "uuid": "wire",
                        "kind": "wire",
                        "object_id": "wire",
                        "operations": [
                            {
                                "kind": "PlotPoly",
                                "index": 0,
                                "points": [[0, 0], [10, 10]],
                                "stroke_color": "#009600FF",
                                "width_nm": 152400,
                            }
                        ],
                    },
                ],
            }
        ),
        encoding="utf-8",
    )

    recorder_dir = root / "projects" / "demo" / "reference_output" / "recorder_dumps"
    recorder_dir.mkdir(parents=True)
    (recorder_dir / "demo.1.json").write_text(
        json.dumps(
            {
                "schema": "kicad.plotter_recorder.v1",
                "ops": [
                    {"kind": "SetColor", "color": "#009600FF"},
                    {
                        "kind": "PlotPoly",
                        "points": [[0, 0], [10, 10]],
                        "fill": "NO_FILL",
                        "width_nm": 152400,
                    },
                ],
            }
        ),
        encoding="utf-8",
    )

    drift_dir = generated_root / "projects" / "demo" / "output" / "recorder_drift"
    drift_dir.mkdir(parents=True)
    (drift_dir / "demo.1.json").write_text(
        json.dumps(
            {
                "op_hist": {
                    "delta": {
                        "PlotPoly": 1,
                        "Rect": -1,
                        "SetColor": 1,
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    op_dir = generated_root / "projects" / "demo" / "output" / "op_equivalence"
    op_dir.mkdir(parents=True)
    (op_dir / "demo.1.json").write_text(
        json.dumps(
            {
                "equivalent": False,
                "stream_sizes": {"recorder_total": 2, "monkey_total": 1},
                "pair_outcomes": {"matched": 1, "style_mismatches": 0},
                "length_divergence": {"monkey_short": 1, "monkey_long": 0},
                "first_divergence": {"kind": "monkey_short", "details": "demo"},
            }
        ),
        encoding="utf-8",
    )

    report = build_ir_coverage_report(root, generated_root=generated_root)

    assert report["schema"] == SCHEMA
    assert report["summary"]["ir_files"] == 1
    assert report["summary"]["recorder_files"] == 1
    assert report["histograms"]["monkey_op_kind"]["Rect"] == 1
    assert report["histograms"]["monkey_op_kind"]["PlotPoly"] == 1
    assert report["histograms"]["recorder_op_kind"]["SetColor"] == 1
    assert report["histograms"]["recorder_delta_positive_geometry_by_op_kind"] == {
        "PlotPoly": 1
    }
    assert report["histograms"]["monkey_delta_positive_geometry_by_op_kind"] == {
        "Rect": 1
    }
    gap_kinds = {row["kind"] for row in report["coverage"]["synthetic_gap_plan"]}
    assert "FlashPadCustom" in gap_kinds
    assert report["equivalence"]["non_equivalent_ranked"][0]["name"] == "demo.1"


def test_render_ir_coverage_markdown_includes_gap_sections(tmp_path):
    root = tmp_path / "kicad"
    root.mkdir()
    (root / "manifest.json").write_text('{"cases": []}', encoding="utf-8")
    report = build_ir_coverage_report(root)

    markdown = render_ir_coverage_markdown(report)

    assert "# KiCad Monkey IR Coverage" in markdown
    assert "Known Plotter Op Coverage" in markdown
    assert "Synthetic Gap Plan" in markdown
    assert "Non-Equivalent Recorder Cases" in markdown

"""
Test L0_010: Recorder JSON loader (Phase F-6)

Pure-unit coverage for ``kicad_recorder_loader``: translates a
``kicad.plotter_recorder.v1`` JSON dump (produced by the KiCad-side
``RECORDER_PLOTTER`` patch) into a canonical ``kicad.plotter_ir.a0``
:class:`KiCadPlotterDocument`.

Exercises:
- Schema validation
- Per-op field-name translations (PenTo plume->action, SetDash
  width_nm->line_width_nm, StartPlot page_number->page_name,
  SetPageSettings type->page_type)
- LINE_STYLE value translation (DASHDOT->DASH_DOT,
  DASHDOTDOT->DASH_DOT_DOT)
- Canvas translation (type->page_type)
- Pass-through for ops/fields that match field-for-field
- Document wrapping (single ``recorder_dump`` record holding all ops)
- Forward-compat for unknown op kinds (raw string preserved)
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from kicad_monkey import (
    KICAD_PLOTTER_RECORDER_SCHEMA,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    load_recorder_dict,
    load_recorder_file,
    normalise_recorder_op_units,
    translate_recorder_canvas,
    translate_recorder_op,
)


# ---------------------------------------------------------------------------
# Schema constant
# ---------------------------------------------------------------------------


def test_schema_constant_value():
    assert KICAD_PLOTTER_RECORDER_SCHEMA == "kicad.plotter_recorder.v1"


# ---------------------------------------------------------------------------
# Per-op field-name translation
# ---------------------------------------------------------------------------


def test_translate_pen_to_renames_plume_to_action():
    op = translate_recorder_op(
        {"kind": "PenTo", "payload": {"x": 1, "y": 2, "plume": "U"}}
    )
    assert op.kind == KiCadPlotterOpKind.PEN_TO
    assert op.payload == {"x": 1, "y": 2, "action": "U"}


def test_translate_pen_to_already_action_is_passthrough():
    op = translate_recorder_op(
        {"kind": "PenTo", "payload": {"x": 1, "y": 2, "action": "D"}}
    )
    assert op.payload == {"x": 1, "y": 2, "action": "D"}


def test_translate_set_dash_renames_width_to_line_width():
    op = translate_recorder_op(
        {"kind": "SetDash", "payload": {"width_nm": 0, "line_style": "SOLID"}}
    )
    assert op.kind == KiCadPlotterOpKind.SET_DASH
    assert op.payload == {"line_width_nm": 0, "line_style": "SOLID"}


def test_translate_set_dash_dashdot_value_normalised():
    op = translate_recorder_op(
        {"kind": "SetDash", "payload": {"width_nm": 5, "line_style": "DASHDOT"}}
    )
    assert op.payload == {"line_width_nm": 5, "line_style": "DASH_DOT"}


def test_translate_set_dash_dashdotdot_value_normalised():
    op = translate_recorder_op(
        {"kind": "SetDash", "payload": {"width_nm": 5, "line_style": "DASHDOTDOT"}}
    )
    assert op.payload == {"line_width_nm": 5, "line_style": "DASH_DOT_DOT"}


def test_translate_start_plot_renames_page_number():
    op = translate_recorder_op(
        {"kind": "StartPlot", "payload": {"page_number": "3"}}
    )
    assert op.kind == KiCadPlotterOpKind.START_PLOT
    assert op.payload == {"page_name": "3"}


def test_translate_set_page_settings_renames_type():
    op = translate_recorder_op(
        {
            "kind": "SetPageSettings",
            "payload": {
                "type": "A4",
                "width_nm": 297000000,
                "height_nm": 210000000,
                "portrait": False,
            },
        }
    )
    assert op.kind == KiCadPlotterOpKind.SET_PAGE_SETTINGS
    assert op.payload == {
        "page_type": "A4",
        "width_nm": 297000000,
        "height_nm": 210000000,
        "portrait": False,
    }


@pytest.mark.parametrize(
    "kind,payload",
    [
        ("Circle", {"cx": 1, "cy": 2, "diameter_nm": 100, "fill": "NO_FILL", "width_nm": 5}),
        (
            "Rect",
            {"x1": 0, "y1": 0, "x2": 10, "y2": 20, "fill": "NO_FILL", "width_nm": 5, "corner_radius_nm": 0},
        ),
        ("SetColor", {"color": "#FF0000FF"}),
        ("SetCurrentLineWidth", {"width_nm": 1524}),
        (
            "SetViewport",
            {"offset_x_nm": 0, "offset_y_nm": 0, "ius_per_decimil": 25.4, "scale": 1.0, "mirror": False},
        ),
        ("StartBlock", {}),
        ("EndBlock", {}),
        ("EndPlot", {}),
        (
            "PlotPoly",
            {"points": [[0, 0], [10, 0], [10, 10]], "fill": "FILLED_SHAPE", "width_nm": 0},
        ),
        (
            "ArcThreePoint",
            {
                "start_x": 0.0,
                "start_y": 0.0,
                "mid_x": 5.0,
                "mid_y": 5.0,
                "end_x": 10.0,
                "end_y": 0.0,
                "fill": "NO_FILL",
                "width_nm": 1524,
            },
        ),
    ],
)
def test_translate_passthrough_ops(kind, payload):
    """Ops whose recorder field names already match the IR are pass-through."""
    op = translate_recorder_op({"kind": kind, "payload": dict(payload)})
    assert op.payload == payload


def test_translate_unknown_op_kind_preserves_raw_string():
    op = translate_recorder_op(
        {"kind": "FutureOp", "payload": {"x": 1}}
    )
    # Forward-compat: unknown kind round-trips as raw string
    assert op.kind == "FutureOp"
    assert op.payload == {"x": 1}


def test_translate_plot_image_derives_internal_extents_from_pixel_payload():
    op = translate_recorder_op(
        {
            "kind": "PlotImage",
            "payload": {
                "x": 100,
                "y": 200,
                "width_px": 10,
                "height_px": 5,
                "scale_factor": 1270.0,
            },
        }
    )
    assert op.payload["width_nm"] == 12_700.0
    assert op.payload["height_nm"] == 6_350.0


def test_translate_op_missing_payload_is_empty_dict():
    op = translate_recorder_op({"kind": "EndPlot"})
    assert op.payload == {}


def test_translate_op_rejects_non_dict():
    with pytest.raises(TypeError):
        translate_recorder_op([1, 2, 3])


def test_normalise_recorder_op_units_scales_from_viewport():
    ops = [
        KiCadPlotterOp(
            kind=KiCadPlotterOpKind.SET_VIEWPORT,
            payload={
                "offset_x_nm": 0,
                "offset_y_nm": 0,
                "ius_per_decimil": 25.4,
                "scale": 1.0,
                "mirror": False,
            },
        ),
        KiCadPlotterOp(kind=KiCadPlotterOpKind.PEN_TO, payload={"x": 1, "y": 2}),
        KiCadPlotterOp(
            kind=KiCadPlotterOpKind.SET_DASH,
            payload={"line_width_nm": 1524, "line_style": "SOLID"},
        ),
        KiCadPlotterOp(
            kind=KiCadPlotterOpKind.RECT,
            payload={
                "x1": 10,
                "y1": 20,
                "x2": 30,
                "y2": 40,
                "width_nm": 5,
                "fill": "NO_FILL",
            },
        ),
        KiCadPlotterOp(
            kind=KiCadPlotterOpKind.PLOT_POLY,
            payload={"points": [[1, 2], [3, 4]], "width_nm": 0},
        ),
        KiCadPlotterOp(
            kind=KiCadPlotterOpKind.SET_PAGE_SETTINGS,
            payload={
                "page_type": "A4",
                "width_nm": 297000000,
                "height_nm": 210000000,
                "portrait": False,
            },
        ),
    ]

    out, nm_per_unit = normalise_recorder_op_units(ops)

    assert nm_per_unit == 100.0
    assert out[1].payload == {"x": 100, "y": 200}
    assert out[2].payload == {"line_width_nm": 152400, "line_style": "SOLID"}
    assert out[3].payload["x1"] == 1000
    assert out[3].payload["y2"] == 4000
    assert out[3].payload["width_nm"] == 500
    assert out[4].payload["points"] == [[100, 200], [300, 400]]
    assert out[5].payload["width_nm"] == 297000000


def test_normalise_recorder_op_units_without_viewport_is_passthrough():
    ops = [KiCadPlotterOp(kind=KiCadPlotterOpKind.PEN_TO, payload={"x": 1, "y": 2})]
    out, nm_per_unit = normalise_recorder_op_units(ops)
    assert out is ops
    assert nm_per_unit is None


# ---------------------------------------------------------------------------
# Canvas translation
# ---------------------------------------------------------------------------


def test_translate_canvas_renames_type():
    out = translate_recorder_canvas(
        {"type": "A4", "width_nm": 1, "height_nm": 2, "portrait": False}
    )
    assert out == {"page_type": "A4", "width_nm": 1, "height_nm": 2, "portrait": False}


def test_translate_canvas_already_page_type_passthrough():
    out = translate_recorder_canvas(
        {"page_type": "User", "width_nm": 1, "height_nm": 2, "portrait": True}
    )
    assert out == {"page_type": "User", "width_nm": 1, "height_nm": 2, "portrait": True}


def test_translate_canvas_none_returns_none():
    assert translate_recorder_canvas(None) is None


def test_translate_canvas_non_dict_returns_none():
    assert translate_recorder_canvas("A4") is None


# ---------------------------------------------------------------------------
# Document loading
# ---------------------------------------------------------------------------


def _minimal_recorder_dict() -> dict:
    return {
        "schema": KICAD_PLOTTER_RECORDER_SCHEMA,
        "canvas": {
            "type": "A4",
            "width_nm": 297000000,
            "height_nm": 210000000,
            "portrait": False,
        },
        "ops": [
            {"kind": "SetPageSettings", "payload": {"type": "A4", "width_nm": 297000000,
                                                     "height_nm": 210000000, "portrait": False}},
            {"kind": "StartPlot", "payload": {"page_number": "1"}},
            {"kind": "PenTo", "payload": {"x": 0, "y": 0, "plume": "U"}},
            {"kind": "SetDash", "payload": {"width_nm": 1524, "line_style": "SOLID"}},
            {"kind": "EndPlot", "payload": {}},
        ],
    }


def test_load_recorder_dict_returns_document():
    doc = load_recorder_dict(_minimal_recorder_dict())
    assert isinstance(doc, KiCadPlotterDocument)
    assert len(doc.records) == 1
    assert doc.records[0].kind == "recorder_dump"
    assert len(doc.records[0].operations) == 5
    assert doc.canvas == {
        "page_type": "A4",
        "width_nm": 297000000,
        "height_nm": 210000000,
        "portrait": False,
    }
    assert doc.coordinate_space == {"unit": "nm", "y_axis": "down"}
    assert doc.source_kind == "SCH"


def test_load_recorder_dict_translates_each_op():
    doc = load_recorder_dict(_minimal_recorder_dict())
    ops = doc.records[0].operations

    # Op 1: SetPageSettings - type -> page_type
    assert ops[0].payload["page_type"] == "A4"
    assert "type" not in ops[0].payload

    # Op 2: StartPlot - page_number -> page_name
    assert ops[1].payload == {"page_name": "1"}

    # Op 3: PenTo - plume -> action
    assert ops[2].payload == {"x": 0, "y": 0, "action": "U"}

    # Op 4: SetDash - width_nm -> line_width_nm
    assert ops[3].payload == {"line_width_nm": 1524, "line_style": "SOLID"}

    # Op 5: EndPlot - empty
    assert ops[4].payload == {}


def test_load_recorder_dict_preserves_op_order():
    data = _minimal_recorder_dict()
    doc = load_recorder_dict(data)
    expected_kinds = ["SetPageSettings", "StartPlot", "PenTo", "SetDash", "EndPlot"]
    actual_kinds = [
        op.kind.value if hasattr(op.kind, "value") else str(op.kind)
        for op in doc.records[0].operations
    ]
    assert actual_kinds == expected_kinds


def test_load_recorder_dict_rejects_wrong_schema():
    bad = {"schema": "wrong.schema.v1", "ops": []}
    with pytest.raises(ValueError, match="recorder schema"):
        load_recorder_dict(bad)


def test_load_recorder_dict_rejects_missing_schema():
    with pytest.raises(ValueError):
        load_recorder_dict({"ops": []})


def test_load_recorder_dict_rejects_non_dict_payload():
    with pytest.raises(TypeError):
        load_recorder_dict([1, 2, 3])


def test_load_recorder_dict_rejects_non_list_ops():
    bad = {"schema": KICAD_PLOTTER_RECORDER_SCHEMA, "ops": "not a list"}
    with pytest.raises(ValueError, match="must be a list"):
        load_recorder_dict(bad)


def test_load_recorder_dict_skips_non_dict_ops_in_list():
    """Forward-compat: malformed entries in ops list are skipped."""
    data = {
        "schema": KICAD_PLOTTER_RECORDER_SCHEMA,
        "ops": [
            {"kind": "EndPlot", "payload": {}},
            "garbage",
            {"kind": "EndBlock", "payload": {}},
        ],
    }
    doc = load_recorder_dict(data)
    assert len(doc.records[0].operations) == 2


def test_load_recorder_dict_empty_ops_yields_empty_record():
    data = {"schema": KICAD_PLOTTER_RECORDER_SCHEMA, "ops": []}
    doc = load_recorder_dict(data)
    assert len(doc.records) == 1
    assert doc.records[0].operations == []


def test_load_recorder_dict_no_canvas_yields_none():
    data = {"schema": KICAD_PLOTTER_RECORDER_SCHEMA, "ops": []}
    doc = load_recorder_dict(data)
    assert doc.canvas is None


def test_load_recorder_dict_object_id_from_document_id():
    doc = load_recorder_dict(_minimal_recorder_dict(), document_id="my_sheet")
    assert doc.document_id == "my_sheet"
    assert doc.records[0].object_id == "my_sheet"


def test_load_recorder_dict_object_id_from_source_path_stem():
    doc = load_recorder_dict(_minimal_recorder_dict(), source_path="/x/y/foo.1.json")
    # No document_id given - falls back to source path stem
    assert doc.records[0].object_id == "foo.1"


def test_load_recorder_dict_default_object_id():
    doc = load_recorder_dict(_minimal_recorder_dict())
    assert doc.records[0].object_id == "recorder_dump"


# ---------------------------------------------------------------------------
# File loading
# ---------------------------------------------------------------------------


def test_load_recorder_file_round_trip(tmp_path: Path):
    payload = _minimal_recorder_dict()
    file_path = tmp_path / "sample.1.json"
    file_path.write_text(json.dumps(payload), encoding="utf-8")

    doc = load_recorder_file(file_path)
    assert isinstance(doc, KiCadPlotterDocument)
    assert doc.source_path == str(file_path)
    assert doc.document_id == "sample.1"
    assert doc.records[0].object_id == "sample.1"
    assert len(doc.records[0].operations) == 5


def test_load_recorder_file_strips_utf8_bom(tmp_path: Path):
    """Loader uses utf-8-sig encoding, so a UTF-8 BOM is tolerated."""
    payload = _minimal_recorder_dict()
    text = "\ufeff" + json.dumps(payload)
    file_path = tmp_path / "bom.json"
    file_path.write_bytes(text.encode("utf-8"))

    doc = load_recorder_file(file_path)
    assert len(doc.records[0].operations) == 5

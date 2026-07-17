"""
Test L0_005: Plotter IR contract (Phase F-1)

Pure-unit coverage for the JSON-serializable plotter-call IR that
mirrors KiCad's PLOTTER virtual-method vocabulary. No parser deps,
no rendering - exercises only the IR layer (op kinds, dataclasses,
JSON I/O, helpers, normalisation).
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from kicad_monkey import (
    KICAD_PLOTTER_IR_SCHEMA,
    KiCadFillType,
    KiCadHorizAlign,
    KiCadLineStyle,
    KiCadPenAction,
    KiCadPlotterBounds,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadVertAlign,
    make_brush,
    make_font,
    make_pen,
)


# ---------------------------------------------------------------------------
# Schema constant
# ---------------------------------------------------------------------------


PLOTTER_IR_SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "docs"
    / "contracts"
    / "kicad_plotter_ir_a0.schema.json"
)


def test_schema_constant_value():
    assert KICAD_PLOTTER_IR_SCHEMA == "kicad.plotter_ir.a0"


# ---------------------------------------------------------------------------
# Op-kind enum sanity
# ---------------------------------------------------------------------------


def test_op_kinds_match_plotter_virtuals():
    """
    Every PLOTTER virtual we care about has a member, and the .value
    string matches the C++ method name verbatim (so a future
    RECORDER_PLOTTER patch can dump JSON by name).
    """
    expected = {
        # path verbs
        "PenTo",
        # primitives
        "Circle", "ArcThreePoint", "ArcCenterAngle", "BezierCurve",
        "Rect", "PlotPoly", "Text", "PlotImage",
        # thick variants
        "ThickSegment", "ThickArc",
        # pad flashes
        "FlashPadCircle", "FlashPadOval", "FlashPadRect",
        "FlashPadRoundRect", "FlashPadCustom", "FlashPadTrapez",
        "FlashRegularPolygon",
        # state
        "SetCurrentLineWidth", "SetColor", "SetDash", "SetViewport",
        # lifecycle
        "StartPlot", "EndPlot", "SetPageSettings",
        # grouping
        "StartBlock", "EndBlock",
    }
    actual = {kind.value for kind in KiCadPlotterOpKind}
    assert expected.issubset(actual), (
        f"missing op kinds: {expected - actual}"
    )


# ---------------------------------------------------------------------------
# Op constructors -> payload shape
# ---------------------------------------------------------------------------


def test_circle_constructor_payload():
    op = KiCadPlotterOp.circle(
        cx=100, cy=200, diameter_nm=50, fill="FILLED_SHAPE", width_nm=10
    )
    assert op.kind == KiCadPlotterOpKind.CIRCLE
    assert op.payload == {
        "cx": 100, "cy": 200, "diameter_nm": 50,
        "fill": "FILLED_SHAPE", "width_nm": 10,
    }


def test_rect_constructor_default_no_fill():
    op = KiCadPlotterOp.rect(x1=0, y1=0, x2=100, y2=200)
    assert op.payload["fill"] == "NO_FILL"
    assert op.payload["width_nm"] == 0
    assert op.payload["corner_radius_nm"] == 0


def test_plot_poly_normalises_tuple_and_list_points():
    op_tup = KiCadPlotterOp.plot_poly(points=[(0, 0), (10, 20)], width_nm=1)
    op_list = KiCadPlotterOp.plot_poly(points=[[0, 0], [10, 20]], width_nm=1)
    assert op_tup.payload["points"] == [[0, 0], [10, 20]]
    assert op_list.payload["points"] == [[0, 0], [10, 20]]


def test_pen_to_action_enum_round_trip():
    op_str = KiCadPlotterOp.pen_to(x=1, y=2, action="U")
    op_enum = KiCadPlotterOp.pen_to(x=1, y=2, action=KiCadPenAction.UP)
    assert op_str.payload["action"] == "U"
    assert op_enum.payload["action"] == "U"


def test_fill_enum_accepts_string_or_enum():
    op_str = KiCadPlotterOp.circle(
        cx=0, cy=0, diameter_nm=10, fill="FILLED_WITH_COLOR"
    )
    op_enum = KiCadPlotterOp.circle(
        cx=0, cy=0, diameter_nm=10, fill=KiCadFillType.FILLED_WITH_COLOR
    )
    assert op_str.payload["fill"] == op_enum.payload["fill"] == "FILLED_WITH_COLOR"


def test_text_op_carries_alignment_pen_and_font_face():
    op = KiCadPlotterOp.text(
        x=0, y=0, text="VCC", color="#0080FF", orient_deg=90.0,
        size_x_nm=2_000_000, size_y_nm=2_000_000,
        h_align=KiCadHorizAlign.CENTER, v_align=KiCadVertAlign.TOP,
        pen_width_nm=200_000, italic=True, bold=False,
        multiline=False, font_face="KiCad Default",
    )
    p = op.payload
    assert p["text"] == "VCC"
    assert p["color"] == "#0080FF"
    assert p["h_align"] == "GR_TEXT_H_ALIGN_CENTER"
    assert p["v_align"] == "GR_TEXT_V_ALIGN_TOP"
    assert p["italic"] is True
    assert p["bold"] is False
    assert p["font_face"] == "KiCad Default"


def test_flash_pad_trapez_requires_4_corners():
    with pytest.raises(ValueError, match="trapezoid requires 4 corners"):
        KiCadPlotterOp.flash_pad_trapez(
            x=0, y=0, corners=[(0, 0), (1, 0), (1, 1)], orient_deg=0.0
        )


def test_set_dash_emits_line_style_string():
    op = KiCadPlotterOp.set_dash(line_width_nm=100, line_style=KiCadLineStyle.DASH_DOT)
    assert op.payload == {"line_width_nm": 100, "line_style": "DASH_DOT"}


# ---------------------------------------------------------------------------
# Color normalisation
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "raw,expected",
    [
        ("#ff0000", "#FF0000"),
        ("#FF0000", "#FF0000"),
        ("#f00", "#FF0000"),
        ("#abcdef", "#ABCDEF"),
        ("#abcdef80", "#ABCDEF80"),
    ],
)
def test_color_normalisation(raw: str, expected: str):
    op = KiCadPlotterOp.set_color(color=raw)
    assert op.payload["color"] == expected


@pytest.mark.parametrize(
    "bad",
    ["red", "#GGG", "#12", "#12345", "0000FF", ""],
)
def test_color_rejection(bad: str):
    with pytest.raises(ValueError):
        KiCadPlotterOp.set_color(color=bad)


def test_rgba_shorthand_expands():
    # #1234 -> #11223344 (each nibble doubled, alpha included)
    op = KiCadPlotterOp.set_color(color="#1234")
    assert op.payload["color"] == "#11223344"


# ---------------------------------------------------------------------------
# Op round-trip (op -> dict -> op)
# ---------------------------------------------------------------------------


def _sample_ops() -> list[KiCadPlotterOp]:
    return [
        KiCadPlotterOp.start_plot(page_name="A4"),
        KiCadPlotterOp.set_page_settings(
            page_type="A4", width_nm=297_000_000, height_nm=210_000_000
        ),
        KiCadPlotterOp.set_viewport(
            offset_x_nm=0, offset_y_nm=0,
            ius_per_decimil=2540.0, scale=1.0, mirror=False,
        ),
        KiCadPlotterOp.set_color(color="#101010"),
        KiCadPlotterOp.set_current_line_width(width_nm=152400),
        KiCadPlotterOp.pen_to(x=0, y=0, action=KiCadPenAction.UP),
        KiCadPlotterOp.pen_to(x=10_000, y=20_000, action="D"),
        KiCadPlotterOp.circle(cx=100, cy=200, diameter_nm=50, fill="NO_FILL", width_nm=10),
        KiCadPlotterOp.arc_three_point(
            start_x=0, start_y=0, mid_x=50, mid_y=50, end_x=100, end_y=0,
            fill="NO_FILL", width_nm=5,
        ),
        KiCadPlotterOp.arc_center_angle(
            cx=0, cy=0, start_angle_deg=0.0, sweep_deg=90.0,
            radius_nm=1_000_000, fill="FILLED_SHAPE", width_nm=0,
        ),
        KiCadPlotterOp.bezier_curve(
            start_x=0, start_y=0, ctrl1_x=10, ctrl1_y=20,
            ctrl2_x=30, ctrl2_y=40, end_x=50, end_y=0,
            tolerance_nm=100, width_nm=10,
        ),
        KiCadPlotterOp.rect(
            x1=0, y1=0, x2=1_000, y2=2_000, fill="FILLED_WITH_BG_BODYCOLOR",
            width_nm=10, corner_radius_nm=50,
        ),
        KiCadPlotterOp.plot_poly(
            points=[(0, 0), (10, 0), (10, 10), (0, 10)],
            fill="FILLED_SHAPE", width_nm=0,
        ),
        KiCadPlotterOp.text(
            x=10, y=20, text="Hello", color="#000000", orient_deg=0.0,
            size_x_nm=1_500_000, size_y_nm=1_500_000,
        ),
        KiCadPlotterOp.thick_segment(
            start_x=0, start_y=0, end_x=100_000, end_y=0, width_nm=200_000,
        ),
        KiCadPlotterOp.flash_pad_circle(x=0, y=0, diameter_nm=1_000_000),
        KiCadPlotterOp.flash_pad_oval(
            x=0, y=0, size_x_nm=1_000_000, size_y_nm=2_000_000, orient_deg=0.0,
        ),
        KiCadPlotterOp.flash_pad_rect(
            x=0, y=0, size_x_nm=1_000_000, size_y_nm=2_000_000, orient_deg=90.0,
        ),
        KiCadPlotterOp.flash_pad_roundrect(
            x=0, y=0, size_x_nm=1_000_000, size_y_nm=2_000_000,
            corner_radius_nm=100_000, orient_deg=0.0,
        ),
        KiCadPlotterOp.flash_pad_trapez(
            x=0, y=0,
            corners=[(0, 0), (10, 0), (8, 5), (2, 5)],
            orient_deg=0.0,
        ),
        KiCadPlotterOp.flash_pad_custom(
            x=0, y=0, size_x_nm=1_000_000, size_y_nm=1_000_000, orient_deg=0.0,
            polygons=[[[0, 0], [10, 0], [10, 10], [0, 10]]],
        ),
        KiCadPlotterOp.flash_reg_polygon(
            x=0, y=0, diameter_nm=1_000_000, corner_count=6, orient_deg=0.0,
        ),
        KiCadPlotterOp.set_dash(line_width_nm=10, line_style="DASH"),
        KiCadPlotterOp.start_block(label="symbol:R1"),
        KiCadPlotterOp.end_block(),
        KiCadPlotterOp.end_plot(),
    ]


def test_each_op_round_trips_through_dict():
    for op in _sample_ops():
        d = op.to_dict()
        op2 = KiCadPlotterOp.from_dict(d)
        assert op2.kind == op.kind, f"kind drift: {op.kind} -> {op2.kind}"
        assert op2.payload == op.payload, f"payload drift on {op.kind}"


def test_op_to_dict_includes_kind_and_optional_index():
    op = KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=10)
    d = op.to_dict()
    assert d["kind"] == "Circle"
    assert "index" not in d

    d2 = op.to_dict(index=7)
    assert d2["index"] == 7


def test_start_block_carries_svg_group_metadata():
    op = KiCadPlotterOp.start_block(
        label="pin-uuid",
        data_uuid="pin-uuid",
        data_ref="symbol_pin",
        object_id="source-pin",
        extra_attrs={"pin": "1", "symbol_uuid": "symbol-uuid", "empty": ""},
    )
    assert op.payload == {
        "label": "pin-uuid",
        "data_uuid": "pin-uuid",
        "data_ref": "symbol_pin",
        "object_id": "source-pin",
        "extra_attrs": {
            "pin": "1",
            "symbol_uuid": "symbol-uuid",
        },
    }


def test_unknown_op_kind_round_trips_as_raw_string():
    """Forward-compat: a future RECORDER op our enum doesn't know about
    should still survive the JSON round-trip."""
    raw = {"kind": "FuturePlotterCall", "x": 1, "y": 2}
    op = KiCadPlotterOp.from_dict(raw)
    assert op.kind == "FuturePlotterCall"
    d = op.to_dict()
    assert d["kind"] == "FuturePlotterCall"
    assert d["x"] == 1


# ---------------------------------------------------------------------------
# Bounds
# ---------------------------------------------------------------------------


def test_bounds_round_trip():
    b = KiCadPlotterBounds(left=0, top=10, right=100, bottom=200)
    d = b.to_dict()
    assert d == {"left": 0, "top": 10, "right": 100, "bottom": 200}
    b2 = KiCadPlotterBounds.from_dict(d)
    assert b2 == b


def test_bounds_from_none_returns_none():
    assert KiCadPlotterBounds.from_dict(None) is None
    assert KiCadPlotterBounds.from_dict("not a dict") is None


# ---------------------------------------------------------------------------
# Record + Document round-trip
# ---------------------------------------------------------------------------


def _sample_doc() -> KiCadPlotterDocument:
    return KiCadPlotterDocument(
        records=[
            KiCadPlotterRecord(
                uuid="aaaa-bbbb",
                kind="lib_symbol",
                object_id="R1",
                bounds=KiCadPlotterBounds(0, 0, 1_000_000, 2_000_000),
                operations=_sample_ops(),
            ),
            KiCadPlotterRecord(
                uuid="cccc-dddd",
                kind="wire",
                object_id="W1",
                operations=[
                    KiCadPlotterOp.thick_segment(
                        start_x=0, start_y=0, end_x=10_000, end_y=0, width_nm=152400,
                    ),
                ],
            ),
        ],
        source_path="C:/foo/bar.kicad_sch",
        source_kind="SCH",
        generated_utc="2026-05-09T12:00:00Z",
        document_id="design-001",
        canvas={"width_nm": 297_000_000, "height_nm": 210_000_000},
        coordinate_space={"unit": "nm", "y_axis": "down"},
        background_color="#FFFFFF",
        render_hints={"variant": None},
    )


def test_record_round_trip_preserves_operations_and_bounds():
    doc = _sample_doc()
    rec = doc.records[0]
    d = rec.to_dict()
    assert d["uuid"] == "aaaa-bbbb"
    assert d["kind"] == "lib_symbol"
    assert d["operation_count"] == len(rec.operations)
    assert d["bounds"] == {"left": 0, "top": 0, "right": 1_000_000, "bottom": 2_000_000}
    rec2 = KiCadPlotterRecord.from_dict(d)
    assert rec2.uuid == rec.uuid
    assert rec2.kind == rec.kind
    assert len(rec2.operations) == len(rec.operations)
    assert rec2.bounds == rec.bounds


def test_document_round_trip_through_dict():
    doc = _sample_doc()
    d = doc.to_dict()
    assert d["schema"] == KICAD_PLOTTER_IR_SCHEMA
    assert d["source_kind"] == "SCH"
    assert d["total_operations"] == sum(len(r.operations) for r in doc.records)
    assert d["canvas"] == {"width_nm": 297_000_000, "height_nm": 210_000_000}

    doc2 = KiCadPlotterDocument.from_dict(d)
    assert len(doc2.records) == len(doc.records)
    assert doc2.source_path == doc.source_path
    assert doc2.source_kind == doc.source_kind
    assert doc2.generated_utc == doc.generated_utc
    assert doc2.canvas == doc.canvas
    assert doc2.coordinate_space == doc.coordinate_space
    assert doc2.background_color == doc.background_color
    assert doc2.render_hints == doc.render_hints


def test_normalized_dict_drops_generated_utc_and_path():
    doc = _sample_doc()
    nd = doc.to_normalized_dict()
    assert "generated_utc" not in nd
    assert "source_path" not in nd
    assert nd["total_operations"] == sum(len(r.operations) for r in doc.records)


def test_normalized_dict_keeps_explicit_source_path_and_normalises_separators():
    doc = _sample_doc()
    nd = doc.to_normalized_dict(source_path=r"C:\foo\bar.kicad_sch")
    assert nd["source_path"] == "C:/foo/bar.kicad_sch"


def test_document_file_round_trip(tmp_path: Path):
    doc = _sample_doc()
    out = tmp_path / "test.ir.json"
    doc.write_json(out)
    assert out.exists()
    doc2 = KiCadPlotterDocument.from_file(out)
    assert len(doc2.records) == len(doc.records)
    assert doc2.records[0].operations[7].payload == doc.records[0].operations[7].payload


def test_normalized_file_round_trip(tmp_path: Path):
    doc = _sample_doc()
    out = tmp_path / "test.normalized.ir.json"
    doc.write_normalized_json(out, source_path="fixtures/foo.kicad_sch")
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert "generated_utc" not in payload
    assert payload["source_path"] == "fixtures/foo.kicad_sch"


# ---------------------------------------------------------------------------
# Schema validation
# ---------------------------------------------------------------------------


def _plotter_ir_contract_validator() -> Draft202012Validator:
    schema = json.loads(PLOTTER_IR_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_plotter_ir_json_schema_validates_sample_document():
    validator = _plotter_ir_contract_validator()
    validator.validate(_sample_doc().to_dict())


def test_plotter_ir_json_schema_validates_context_extensions():
    payload = _sample_doc().to_dict()
    payload["context"] = {"producer": "unit-test", "capabilities": ["scene"]}
    payload["records"][0]["context"] = {"source": {"uuid": "aaaa-bbbb"}}
    payload["records"][0]["operations"][0]["context"] = {
        "hyperlink": {"href": "https://example.test/reference"}
    }

    validator = _plotter_ir_contract_validator()
    validator.validate(payload)

    doc = KiCadPlotterDocument.from_dict(payload)
    assert doc.extras["context"]["producer"] == "unit-test"
    assert doc.records[0].extras["context"]["source"]["uuid"] == "aaaa-bbbb"
    assert (
        doc.records[0].operations[0].payload["context"]["hyperlink"]["href"]
        == "https://example.test/reference"
    )


def test_plotter_ir_json_schema_rejects_known_op_missing_required_payload():
    payload = _sample_doc().to_dict()
    circle = payload["records"][0]["operations"][7]
    assert circle["kind"] == "Circle"
    del circle["diameter_nm"]

    validator = _plotter_ir_contract_validator()
    with pytest.raises(ValidationError, match="diameter_nm"):
        validator.validate(payload)


def test_plotter_ir_schema_known_op_enum_matches_runtime_enum():
    schema = json.loads(PLOTTER_IR_SCHEMA_PATH.read_text(encoding="utf-8"))
    schema_kinds = set(schema["$defs"]["knownOpKind"]["enum"])
    runtime_kinds = {kind.value for kind in KiCadPlotterOpKind}
    assert schema_kinds == runtime_kinds


def test_plotter_ir_schema_source_kind_enum_matches_emitters():
    schema = json.loads(PLOTTER_IR_SCHEMA_PATH.read_text(encoding="utf-8"))
    assert set(schema["properties"]["source_kind"]["enum"]) == {
        "SCH",
        "SYM",
        "PCB",
        "MOD",
        "PCB_FOOTPRINT",
    }


def test_from_dict_accepts_legacy_v1_schema_but_emits_a0():
    payload = _sample_doc().to_dict()
    payload["schema"] = "kicad.plotter_ir.v1"

    doc = KiCadPlotterDocument.from_dict(payload)

    assert doc.to_dict()["schema"] == KICAD_PLOTTER_IR_SCHEMA


def test_from_dict_rejects_missing_or_wrong_schema():
    with pytest.raises(ValueError, match="Unexpected plotter IR schema"):
        KiCadPlotterDocument.from_dict({"records": []})

    with pytest.raises(ValueError, match="Unexpected plotter IR schema"):
        KiCadPlotterDocument.from_dict({"schema": "altium.got.v1", "records": []})


def test_from_file_rejects_non_object_payload(tmp_path: Path):
    bad = tmp_path / "bad.json"
    bad.write_text("[]", encoding="utf-8")
    with pytest.raises(ValueError, match="must be a JSON object"):
        KiCadPlotterDocument.from_file(bad)


# ---------------------------------------------------------------------------
# Helpers (make_pen / make_brush / make_font)
# ---------------------------------------------------------------------------


def test_make_pen_defaults_and_normalisation():
    p = make_pen(color="#abc", width_nm=100, line_style="DASH")
    assert p == {
        "color": "#AABBCC",
        "width_nm": 100,
        "line_style": "DASH",
        "dash_values": [],
    }


def test_make_pen_carries_dash_values_as_floats():
    p = make_pen(color="#000", width_nm=0, dash_values=[1, 2, 3])
    assert p["dash_values"] == [1.0, 2.0, 3.0]


def test_make_brush_clamps_alpha():
    assert make_brush(color="#FFFFFF", alpha=999)["alpha"] == 255
    assert make_brush(color="#FFFFFF", alpha=-5)["alpha"] == 0
    assert make_brush(color="#FFFFFF", alpha=128)["alpha"] == 128


def test_make_font_shape():
    f = make_font(face="KiCad Default", size_nm=2_000_000, italic=True, bold=False, rotation_deg=90.0)
    assert f == {
        "face": "KiCad Default",
        "size_nm": 2_000_000,
        "italic": True,
        "bold": False,
        "rotation_deg": 90.0,
    }

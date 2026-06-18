"""
Test L0_007: LibSymbol → IR converter (Phase F-3)

Pure-unit coverage for the parser → IR boundary that turns a parsed
``LibSymbol`` (mm, Y-up) into a ``KiCadPlotterDocument`` (nm, Y-down)
mirroring KiCad's ``LIB_SYMBOL::Plot()`` traversal. No oracle, no
rendering — just the unit conversion + enum mapping + per-shape op
emission.
"""

from __future__ import annotations

import pytest

from kicad_monkey import (
    KiCadFillType,
    KiCadHorizAlign,
    KiCadLineStyle,
    KiCadPlotterDocument,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadSymbolLib,
    KiCadVertAlign,
    LibSubSymbol,
    LibSymbol,
    SymArc,
    SymBezier,
    SymCircle,
    SymPin,
    SymPolyline,
    SymRectangle,
    SymText,
    arc_to_op,
    bezier_to_op,
    circle_to_op,
    lib_symbol_to_ir,
    mm_to_nm,
    pin_to_ops,
    polyline_to_op,
    rectangle_to_op,
    rgba_to_hex,
    stroke_type_to_line_style,
    stroke_width_nm,
    subsymbol_to_record,
    sym_fill_to_kicad_fill,
    text_to_op,
    y_to_ir,
)
from kicad_monkey.kicad_base import StrokeType
from kicad_monkey.kicad_primitives import Effects, Font, Stroke
from kicad_monkey.kicad_sch_enums import PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sym_rectangle import SymFill, SymFillType


# ---------------------------------------------------------------------------
# Unit conversion
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "value_mm, expected_nm",
    [
        (0.0, 0),
        (1.0, 1_000_000),
        (2.54, 2_540_000),
        (-3.81, -3_810_000),
        (0.0254, 25_400),
    ],
)
def test_mm_to_nm(value_mm, expected_nm):
    assert mm_to_nm(value_mm) == expected_nm


def test_y_to_ir_negates_y_axis():
    """Y-up mm → Y-down nm."""
    assert y_to_ir(0.0) == 0
    assert y_to_ir(1.0) == -1_000_000
    assert y_to_ir(-2.5) == 2_500_000


def test_stroke_width_nm_zero_uses_symbol_default():
    s = Stroke(width=0.0)
    assert stroke_width_nm(s) == 152_400


def test_stroke_width_nm_negative_is_literal_zero():
    s = Stroke(width=-0.0001)
    assert stroke_width_nm(s) == 0


def test_stroke_width_nm_hairline_clamps_to_plot_minimum():
    s = Stroke(width=0.01)
    assert stroke_width_nm(s) == 84_700


def test_stroke_width_nm_nonzero_converts():
    s = Stroke(width=0.254)  # 10 mil
    assert stroke_width_nm(s) == 254_000


# ---------------------------------------------------------------------------
# Enum mapping
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "stroke_type, expected",
    [
        (StrokeType.SOLID, KiCadLineStyle.SOLID),
        (StrokeType.DASH, KiCadLineStyle.DASH),
        (StrokeType.DOT, KiCadLineStyle.DOT),
        (StrokeType.DASH_DOT, KiCadLineStyle.DASH_DOT),
        (StrokeType.DASH_DOT_DOT, KiCadLineStyle.DASH_DOT_DOT),
        (StrokeType.DEFAULT, KiCadLineStyle.DEFAULT),
    ],
)
def test_stroke_type_to_line_style(stroke_type, expected):
    assert stroke_type_to_line_style(stroke_type) == expected


@pytest.mark.parametrize(
    "sym_fill, expected",
    [
        (SymFillType.NONE, KiCadFillType.NO_FILL),
        (SymFillType.OUTLINE, KiCadFillType.FILLED_SHAPE),
        (SymFillType.BACKGROUND, KiCadFillType.FILLED_WITH_BG_BODYCOLOR),
        (SymFillType.COLOR, KiCadFillType.FILLED_WITH_COLOR),
        (SymFillType.HATCH, KiCadFillType.HATCH),
        (SymFillType.REVERSE_HATCH, KiCadFillType.REVERSE_HATCH),
        (SymFillType.CROSS_HATCH, KiCadFillType.CROSS_HATCH),
    ],
)
def test_sym_fill_to_kicad_fill(sym_fill, expected):
    assert sym_fill_to_kicad_fill(sym_fill) == expected


def test_rgba_to_hex_basic():
    assert rgba_to_hex((255, 0, 0, 1.0)) == "#FF0000FF"
    assert rgba_to_hex((0, 128, 255, 0.5)) == "#0080FF80"


def test_rgba_to_hex_none():
    assert rgba_to_hex(None) is None


def test_rgba_to_hex_clamps_channels():
    assert rgba_to_hex((-1, 999, 128, 1.0)) == "#00FF80FF"


# ---------------------------------------------------------------------------
# Per-shape op emitters
# ---------------------------------------------------------------------------


def test_rectangle_to_op_basic():
    rect = SymRectangle(
        start_x=0.0, start_y=0.0,
        end_x=10.0, end_y=5.0,
        stroke=Stroke(width=0.254),
        fill=SymFill(type=SymFillType.OUTLINE),
    )
    op = rectangle_to_op(rect)
    assert op.kind == KiCadPlotterOpKind.RECT
    assert op.payload["x1"] == 0
    assert op.payload["y1"] == 0
    assert op.payload["x2"] == 10_000_000
    assert op.payload["y2"] == -5_000_000  # Y flipped
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 254_000


def test_circle_to_op_radius_to_diameter():
    c = SymCircle(
        center_x=2.0, center_y=3.0, radius=1.5,
        stroke=Stroke(width=0.1),
        fill=SymFill(type=SymFillType.NONE),
    )
    op = circle_to_op(c)
    assert op.kind == KiCadPlotterOpKind.CIRCLE
    assert op.payload["cx"] == 2_000_000
    assert op.payload["cy"] == -3_000_000
    assert op.payload["diameter_nm"] == 3_000_000  # 2 * 1.5 mm
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value
    assert op.payload["width_nm"] == 100_000


def test_arc_to_op_three_points():
    a = SymArc(
        start_x=0.0, start_y=0.0,
        mid_x=1.0, mid_y=1.0,
        end_x=2.0, end_y=0.0,
        stroke=Stroke(width=0.1),
    )
    op = arc_to_op(a)
    assert op.kind == KiCadPlotterOpKind.ARC_THREE_POINT
    assert op.payload["start_x"] == 0.0
    assert op.payload["start_y"] == 0.0
    assert op.payload["mid_x"] == 1_000_000.0
    assert op.payload["mid_y"] == -1_000_000.0
    assert op.payload["end_x"] == 2_000_000.0
    assert op.payload["end_y"] == 0.0


def test_polyline_to_op_yields_plot_poly():
    p = SymPolyline(
        points=[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
        stroke=Stroke(width=0.15),
        fill=SymFill(type=SymFillType.OUTLINE),
    )
    op = polyline_to_op(p)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["points"] == [
        [0, 0],
        [1_000_000, 0],
        [1_000_000, -1_000_000],
    ]
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 150_000


def test_bezier_to_op_cubic_4_points():
    b = SymBezier(
        points=[(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)],
        stroke=Stroke(width=0.1),
    )
    op = bezier_to_op(b)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.BEZIER_CURVE
    assert op.payload["start_x"] == 0
    assert op.payload["ctrl1_x"] == 1_000_000
    assert op.payload["ctrl1_y"] == -2_000_000
    assert op.payload["end_x"] == 4_000_000
    assert op.payload["end_y"] == 0


def test_bezier_to_op_2_or_3_points_falls_back_to_polyline():
    b = SymBezier(
        points=[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)],  # quadratic / 3 pts
        stroke=Stroke(width=0.1),
    )
    op = bezier_to_op(b)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY


def test_bezier_to_op_empty_returns_none():
    b = SymBezier(points=[])
    assert bezier_to_op(b) is None


def test_text_to_op_with_default_effects():
    t = SymText(text="Hi", at_x=2.0, at_y=3.0, at_angle=0.0, effects=None)
    op = text_to_op(t)
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["x"] == 2_000_000
    assert op.payload["y"] == -3_000_000
    assert op.payload["text"] == "Hi"
    assert op.payload["size_x_nm"] == 1_270_000  # default 1.27 mm
    assert op.payload["size_y_nm"] == 1_270_000
    assert op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value


def test_text_to_op_uses_effects_font():
    eff = Effects(
        font=Font(face="Arial", size_x=2.0, size_y=2.5, bold=True, italic=True, thickness=0.3),
        justify=["right", "top"],
    )
    t = SymText(text="X", at_x=0.0, at_y=0.0, at_angle=90.0, effects=eff)
    op = text_to_op(t)
    assert op.payload["size_x_nm"] == 2_000_000
    assert op.payload["size_y_nm"] == 2_500_000
    assert op.payload["bold"] is True
    assert op.payload["italic"] is True
    assert op.payload["font_face"] == "Arial"
    assert op.payload["pen_width_nm"] == 300_000
    assert op.payload["orient_deg"] == pytest.approx(90.0)
    assert op.payload["h_align"] == KiCadHorizAlign.RIGHT.value
    assert op.payload["v_align"] == KiCadVertAlign.TOP.value


def test_text_to_op_expands_project_vars_and_marks_multiline():
    t = SymText(text="Hello ${NAME}\n\nDone\n", at_x=0.0, at_y=0.0)
    op = text_to_op(t, project_vars={"NAME": "KiCad"})

    assert op is not None
    assert op.payload["text"] == "Hello KiCad\n\nDone"
    assert op.payload["multiline"] is True


def test_text_to_op_returns_none_for_hidden_symbol_text():
    nested = SymText(text="BANK 64", effects=Effects(hide=True))
    sibling = SymText.from_sexp([
        "text",
        "BANK 65",
        ["at", 0.0, 0.0, 0],
        ["hide", "yes"],
        ["effects", ["font", ["size", 1.778, 1.778]]],
    ])

    assert sibling.hide is True
    assert text_to_op(nested) is None
    assert text_to_op(sibling) is None
    emitted = sibling.to_sexp()
    assert ["hide", "yes"] not in emitted
    effects = next(item for item in emitted if isinstance(item, list) and item[0] == "effects")
    assert "hide" in effects or ["hide", "yes"] in effects


def test_pin_to_ops_emits_wire_segment():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=0.0,
        length=2.54,
        name="IN", number="1",
    )
    ops = pin_to_ops(pin)
    # Wire + number + name (in that order).
    assert len(ops) == 3
    wire = ops[0]
    assert wire.kind == KiCadPlotterOpKind.PLOT_POLY
    assert wire.payload["points"] == [[2_540_000, 0], [0, 0]]


def test_pin_to_ops_inverted_pin_emits_circle_and_shortened_wire():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.INVERTED,
        at_x=0.0, at_y=0.0, at_angle=0.0,
        length=2.54,
        name="~", number="",
    )
    ops = pin_to_ops(pin)
    assert [o.kind for o in ops] == [
        KiCadPlotterOpKind.CIRCLE,
        KiCadPlotterOpKind.PLOT_POLY,
    ]
    assert ops[0].payload["cx"] == 1_905_000
    assert ops[0].payload["cy"] == 0
    assert ops[0].payload["diameter_nm"] == 1_270_000
    assert ops[1].payload["points"] == [[1_270_000, 0], [0, 0]]


def test_pin_to_ops_emits_number_at_shaft_midpoint():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=0.0,
        length=2.54,
        name="~", number="7",  # name="~" → name suppressed
    )
    ops = pin_to_ops(pin)
    # Wire + number only (no name because "~" is the suppress sentinel).
    assert [o.kind for o in ops] == [
        KiCadPlotterOpKind.PLOT_POLY,
        KiCadPlotterOpKind.TEXT,
    ]
    num_op = ops[1]
    # Shaft midpoint at (1.27 mm, 0) Y-up; pin-number auto stroke is
    # 10 mil, so KiCad offsets by 4 mil margin + 10 mil stroke.
    assert num_op.payload["text"] == "7"
    assert num_op.payload["x"] == 1_270_000
    assert num_op.payload["y"] == -355_600
    assert num_op.payload["pen_width_nm"] == 254_000
    assert num_op.payload["orient_deg"] == pytest.approx(0.0)
    assert num_op.payload["h_align"] == KiCadHorizAlign.CENTER.value


def test_pin_to_ops_name_inside_body_when_offset_positive():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=0.0,
        length=2.54,
        name="IN", number="",  # number="" → number suppressed
    )
    ops = pin_to_ops(pin, pin_names_offset=0.508)
    # Wire + name (no number).
    assert [o.kind for o in ops] == [
        KiCadPlotterOpKind.PLOT_POLY,
        KiCadPlotterOpKind.TEXT,
    ]
    name_op = ops[1]
    # body_end = (2.54, 0); name pos = body_end + 0.508 along pin dir = (3.048, 0).
    assert name_op.payload["text"] == "IN"
    assert name_op.payload["x"] == 3_048_000
    assert name_op.payload["y"] == 0
    assert name_op.payload["h_align"] == KiCadHorizAlign.LEFT.value


def test_pin_to_ops_name_outside_when_offset_zero():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=180.0,
        length=2.54,
        name="OUT", number="",
    )
    ops = pin_to_ops(pin, pin_names_offset=0.0)
    # Pin pointing left → body_end = (-2.54, 0). With offset=0, name is
    # placed perpendicular at body_end. cos(180)=-1, sin(180)=0,
    # perpendicular (-sin, cos) = (0, -1) in Y-up → IR Y direction is +508_000.
    name_op = ops[1]
    assert name_op.payload["text"] == "OUT"
    assert name_op.payload["x"] == -1_270_000
    # Y-up perpendicular: 0.508 mm in (-sin, cos) direction → (0, -0.508)
    # y_to_ir(-0.508) = +508_000.
    assert name_op.payload["y"] == -254_000
    assert name_op.payload["h_align"] == KiCadHorizAlign.CENTER.value


def test_pin_to_ops_respects_hide_flags():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=0.0,
        length=2.54, name="IN", number="1",
    )
    # pin_numbers_hide → only wire + name
    ops = pin_to_ops(pin, pin_numbers_hide=True)
    assert [o.kind for o in ops] == [
        KiCadPlotterOpKind.PLOT_POLY,
        KiCadPlotterOpKind.TEXT,
    ]
    assert ops[1].payload["text"] == "IN"
    # pin_names_hide → only wire + number
    ops = pin_to_ops(pin, pin_names_hide=True)
    assert [o.kind for o in ops] == [
        KiCadPlotterOpKind.PLOT_POLY,
        KiCadPlotterOpKind.TEXT,
    ]
    assert ops[1].payload["text"] == "1"
    # Both → only wire
    ops = pin_to_ops(pin, pin_numbers_hide=True, pin_names_hide=True)
    assert [o.kind for o in ops] == [KiCadPlotterOpKind.PLOT_POLY]


def test_pin_to_ops_suppresses_zero_size_pin_texts():
    zero_text = Effects(font=Font(size_x=0.0, size_y=0.0))
    pin = SymPin(
        electrical_type=PinElectricalType.PASSIVE,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        at_angle=0.0,
        length=2.54,
        name="G",
        name_effects=zero_text,
        number="1",
        number_effects=zero_text,
    )

    ops = pin_to_ops(pin)

    assert [o.kind for o in ops] == [KiCadPlotterOpKind.PLOT_POLY]


def test_pin_to_ops_vertical_pin_has_orient_90():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=90.0,
        length=2.54,
        name="IN", number="1",
    )
    ops = pin_to_ops(pin)
    # ops = [wire, number, name]
    assert ops[1].payload["orient_deg"] == pytest.approx(90.0)
    assert ops[2].payload["orient_deg"] == pytest.approx(90.0)


def test_pin_to_ops_zero_length_vertical_uses_orientation_for_text():
    pin = SymPin(
        electrical_type=PinElectricalType.POWER_IN,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=90.0,
        length=0.0,
        name="VCC", number="5",
    )
    ops = pin_to_ops(pin)
    number_op = ops[0]
    name_op = ops[1]

    assert number_op.payload["text"] == "5"
    assert number_op.payload["x"] == -355_600
    assert number_op.payload["y"] == 0
    assert number_op.payload["orient_deg"] == pytest.approx(90.0)

    assert name_op.payload["text"] == "VCC"
    assert name_op.payload["x"] == 0
    assert name_op.payload["y"] == -508_000
    assert name_op.payload["orient_deg"] == pytest.approx(90.0)
    assert name_op.payload["h_align"] == KiCadHorizAlign.LEFT.value


def test_pin_to_ops_handles_orientation():
    pin = SymPin(
        electrical_type=PinElectricalType.OUTPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=90.0,
        length=2.54,
    )
    ops = pin_to_ops(pin)
    op = ops[0]
    # 90° in Y-up coords → body straight UP from external point. Y-up
    # pre-flip; IR Y-down → body should be at -2.54 mm in IR.
    pts = op.payload["points"]
    assert pts[0] == [0, -2_540_000]
    # cos(90)=0, sin(90)=1 → body_y_mm = +2.54; y_to_ir → -2_540_000
    assert pts[1] == [0, 0]


def test_pin_hidden_emits_no_ops():
    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0, at_y=0.0, at_angle=0.0,
        length=2.54, hide=True,
    )
    assert pin_to_ops(pin) == []


# ---------------------------------------------------------------------------
# Subsymbol / symbol-level converters
# ---------------------------------------------------------------------------


def _make_subsymbol() -> "LibSubSymbol":
    return LibSubSymbol(
        name="MySym_1_0",
        unit=1, style=0,
        rectangles=[
            SymRectangle(start_x=-5.0, start_y=-5.0, end_x=5.0, end_y=5.0,
                         stroke=Stroke(width=0.254),
                         fill=SymFill(type=SymFillType.BACKGROUND))
        ],
        circles=[
            SymCircle(center_x=0.0, center_y=0.0, radius=1.0,
                      stroke=Stroke(width=0.1))
        ],
        polylines=[SymPolyline(points=[(-1.0, 0.0), (1.0, 0.0)],
                               stroke=Stroke(width=0.1))],
        pins=[
            SymPin(
                electrical_type=PinElectricalType.INPUT,
                graphic_style=PinGraphicStyle.LINE,
                at_x=-7.62, at_y=0.0, at_angle=0.0,
                length=2.54, name="IN", number="1",
            )
        ],
    )


def test_subsymbol_to_record_emits_all_shapes_in_order():
    sub = _make_subsymbol()
    rec = subsymbol_to_record(sub)
    assert isinstance(rec, KiCadPlotterRecord)
    assert rec.kind == "lib_subsymbol"
    assert rec.object_id == "MySym_1_0"
    kinds = [op.kind for op in rec.operations]
    # Filled body rect is split into a fill pass before pins and an outline
    # pass after pins, matching KiCad's symbol plot order.
    assert kinds == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.CIRCLE,
        KiCadPlotterOpKind.PLOT_POLY,   # polyline
        KiCadPlotterOpKind.PLOT_POLY,   # pin wire
        KiCadPlotterOpKind.TEXT,        # pin number
        KiCadPlotterOpKind.TEXT,        # pin name
        KiCadPlotterOpKind.RECT,        # deferred rect outline
    ]
    fill_rect = rec.operations[0]
    outline_rect = rec.operations[-1]
    assert fill_rect.payload["fill"] == KiCadFillType.FILLED_WITH_BG_BODYCOLOR.value
    assert fill_rect.payload["width_nm"] == 0
    assert outline_rect.payload["fill"] == KiCadFillType.NO_FILL.value
    assert outline_rect.payload["width_nm"] == 254_000
    assert rec.extras["unit"] == 1
    assert rec.extras["style"] == 0


def test_subsymbol_to_record_splits_default_width_filled_body_outline():
    sub = LibSubSymbol(
        name="DefaultStroke_1_0",
        unit=1,
        style=0,
        rectangles=[
            SymRectangle(
                start_x=-1.0,
                start_y=-1.0,
                end_x=1.0,
                end_y=1.0,
                stroke=Stroke(width=0.0),
                fill=SymFill(type=SymFillType.BACKGROUND),
            )
        ],
    )
    rec = subsymbol_to_record(sub)
    assert [op.kind for op in rec.operations] == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.RECT,
    ]
    fill_rect, outline_rect = rec.operations
    assert fill_rect.payload["fill"] == KiCadFillType.FILLED_WITH_BG_BODYCOLOR.value
    assert fill_rect.payload["width_nm"] == 0
    assert outline_rect.payload["fill"] == KiCadFillType.NO_FILL.value
    assert outline_rect.payload["width_nm"] == 152_400


def test_lib_symbol_to_ir_returns_document_with_header_and_record():
    sym = LibSymbol(
        name="MySym",
        in_bom=True, on_board=True,
        subsymbols=[_make_subsymbol()],
    )
    doc = lib_symbol_to_ir(sym, unit=1, style=0, source_path="C:/path/test.kicad_sym")
    assert isinstance(doc, KiCadPlotterDocument)
    assert doc.source_kind == "SYM"
    assert doc.coordinate_space == {"unit": "nm", "y_axis": "down"}
    assert len(doc.records) == 2  # header + 1 subsymbol
    header, subrec = doc.records
    assert header.kind == "lib_symbol"
    assert header.object_id == "MySym"
    assert header.extras["name"] == "MySym"
    assert header.extras["unit"] == 1
    assert subrec.kind == "lib_subsymbol"
    assert doc.document_id == "MySym"
    assert doc.source_path == "C:/path/test.kicad_sym"


def test_lib_symbol_to_ir_filters_by_unit():
    sym = LibSymbol(
        name="MultiUnit",
        subsymbols=[
            LibSubSymbol(name="MultiUnit_0_0", unit=0, style=0),  # common
            LibSubSymbol(name="MultiUnit_1_0", unit=1, style=0),
            LibSubSymbol(name="MultiUnit_2_0", unit=2, style=0),
        ],
    )
    doc = lib_symbol_to_ir(sym, unit=1, style=0)
    # header + (unit=0 common) + (unit=1) -> NOT unit=2
    assert len(doc.records) == 3
    sub_ids = [r.object_id for r in doc.records[1:]]
    assert sub_ids == ["MultiUnit_0_0", "MultiUnit_1_0"]


def test_lib_symbol_to_ir_filters_by_style():
    # KiCad body-style rule in files: style==0 is common, style==1 is the
    # normal/default body, and style==2 is De Morgan.
    sym = LibSymbol(
        name="DM",
        subsymbols=[
            LibSubSymbol(name="DM_1_0", unit=1, style=0),  # common
            LibSubSymbol(name="DM_1_1", unit=1, style=1),  # normal-only
            LibSubSymbol(name="DM_1_2", unit=1, style=2),  # demorgan-only
        ],
    )
    common_only = lib_symbol_to_ir(sym, unit=1, style=0)
    normal = lib_symbol_to_ir(sym, unit=1, style=1)
    demorgan = lib_symbol_to_ir(sym, unit=1, style=2)
    # style=0 request: default rendering includes common + normal subsymbols.
    assert [r.object_id for r in common_only.records[1:]] == ["DM_1_0", "DM_1_1"]
    # style=1 request: common + normal-only
    assert [r.object_id for r in normal.records[1:]] == ["DM_1_0", "DM_1_1"]
    # style=2 request: common + demorgan-only
    assert [r.object_id for r in demorgan.records[1:]] == ["DM_1_0", "DM_1_2"]


def test_lib_symbol_to_ir_unit_none_emits_all_matching_style():
    sym = LibSymbol(
        name="All",
        subsymbols=[
            LibSubSymbol(name="All_0_0", unit=0, style=0),
            LibSubSymbol(name="All_1_0", unit=1, style=0),
            LibSubSymbol(name="All_2_0", unit=2, style=0),
            LibSubSymbol(name="All_2_1", unit=2, style=1),
        ],
    )
    doc = lib_symbol_to_ir(sym, unit=None, style=0)
    sub_ids = [r.object_id for r in doc.records[1:]]
    assert sub_ids == ["All_0_0", "All_1_0", "All_2_0", "All_2_1"]


def test_symbol_library_ir_resolves_extends_to_base_geometry():
    base = LibSymbol(name="Base", subsymbols=[_make_subsymbol()])
    child = LibSymbol(name="Child", extends="Base")
    lib = KiCadSymbolLib()
    lib.symbols = [base, child]

    doc = lib.symbol_to_ir("Child", part_id=1)

    assert doc.document_id == "Child"
    assert doc.records[0].object_id == "Child"
    subrecords = [record for record in doc.records if record.kind == "lib_subsymbol"]
    assert subrecords
    assert sum(len(record.operations) for record in subrecords) > 0


def test_lib_symbol_to_ir_round_trip_via_dict():
    """Document survives JSON round-trip without loss."""
    sym = LibSymbol(name="RT", subsymbols=[_make_subsymbol()])
    doc = lib_symbol_to_ir(sym, unit=1, style=0)
    doc_dict = doc.to_dict()
    rebuilt = KiCadPlotterDocument.from_dict(doc_dict)
    assert rebuilt.source_kind == doc.source_kind
    assert len(rebuilt.records) == len(doc.records)
    # First subsymbol record's first op (rect) preserved
    rect_op_orig = doc.records[1].operations[0]
    rect_op_new = rebuilt.records[1].operations[0]
    assert rect_op_new.kind == rect_op_orig.kind
    assert rect_op_new.payload == rect_op_orig.payload

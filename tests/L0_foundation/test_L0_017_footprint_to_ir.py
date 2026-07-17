"""
Test L0_017: KiCadFootprint → IR converter (Phase F-7)

Pure-unit coverage for the parser → IR boundary that turns a parsed
``KiCadFootprint`` (mm, Y-down) into a ``KiCadPlotterDocument`` (nm,
Y-down) mirroring KiCad's footprint emit order. No oracle, no
rendering — just the unit conversion + enum mapping + per-shape op
emission + pad-shape dispatch.

Distinct from F-3 (`test_L0_007_lib_symbol_to_ir.py`):
* PCB coords are Y-down already → no Y-flip
* Pads dispatch onto the FlashPad* op family
* Properties/FpTexts have hide+empty-value skip rules
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from kicad_monkey import (
    KiCadFillType,
    KiCadPlotterDocument,
    KiCadPlotterOpKind,
    footprint_to_ir,
    footprint_to_record,
    fp_arc_to_op,
    fp_circle_to_op,
    fp_fill_to_kicad_fill,
    fp_line_to_op,
    fp_poly_to_op,
    fp_rect_to_op,
    fp_text_box_to_ops,
    fp_text_to_op,
    pad_drill_to_ops,
    pad_to_ops,
    property_to_op,
)
from kicad_monkey.kicad_base import FillType, PadShape, PadType
from kicad_monkey.kicad_footprint import KiCadFootprint
from kicad_monkey.kicad_fp_arc import FpArc
from kicad_monkey.kicad_fp_circle import FpCircle
from kicad_monkey.kicad_fp_line import FpLine
from kicad_monkey.kicad_fp_poly import FpPoly
from kicad_monkey.kicad_fp_rect import FpRect
from kicad_monkey.kicad_fp_text import FpText
from kicad_monkey.kicad_fp_text_box import FpTextBox
from kicad_monkey.kicad_pad import Pad, PadCustomOptions, PadCustomPrimitive
from kicad_monkey.kicad_primitives import Effects, Font, Stroke
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


# ---------------------------------------------------------------------------
# Enum mapping
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "fp_fill, expected",
    [
        (FillType.NONE, KiCadFillType.NO_FILL),
        (FillType.NO, KiCadFillType.NO_FILL),
        (FillType.YES, KiCadFillType.FILLED_SHAPE),
        (FillType.SOLID, KiCadFillType.FILLED_SHAPE),
    ],
)
def test_fp_fill_to_kicad_fill(fp_fill, expected):
    assert fp_fill_to_kicad_fill(fp_fill) == expected


# ---------------------------------------------------------------------------
# Per-shape emitters: fp_line / fp_arc / fp_circle / fp_rect / fp_poly
# ---------------------------------------------------------------------------


def test_fp_line_to_op_emits_thick_segment():
    line = FpLine(
        start_x=1.0, start_y=2.0, end_x=3.0, end_y=4.0,
        layer="F.SilkS", stroke=Stroke(width=0.15),
    )
    op = fp_line_to_op(line)
    assert op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    assert op.payload == {
        "start_x": 1_000_000,
        "start_y": 2_000_000,  # NO Y-flip (PCB Y-down)
        "end_x": 3_000_000,
        "end_y": 4_000_000,
        "width_nm": 150_000,
    }


def test_fp_line_to_op_no_y_flip():
    """PCB coords are Y-down already; +Y in input → +Y in IR."""
    line = FpLine(start_x=0.0, start_y=5.0, end_x=0.0, end_y=10.0)
    op = fp_line_to_op(line)
    assert op.payload["start_y"] == 5_000_000
    assert op.payload["end_y"] == 10_000_000


def test_fp_arc_to_op_emits_arc_three_point():
    arc = FpArc(
        start_x=0.0, start_y=0.0,
        mid_x=1.0, mid_y=1.0,
        end_x=2.0, end_y=0.0,
        layer="F.SilkS", stroke=Stroke(width=0.1),
    )
    op = fp_arc_to_op(arc)
    assert op.kind == KiCadPlotterOpKind.ARC_THREE_POINT
    assert op.payload["start_x"] == 0
    assert op.payload["mid_x"] == 1_000_000
    assert op.payload["end_x"] == 2_000_000
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value
    assert op.payload["width_nm"] == 100_000


def test_fp_circle_to_op_recovers_radius_from_endpoint():
    # Center at (5,5), end at (8,9) → radius = sqrt(3^2+4^2) = 5
    circle = FpCircle(
        center_x=5.0, center_y=5.0,
        end_x=8.0, end_y=9.0,
        stroke=Stroke(width=0.2),
        fill=FillType.SOLID,
    )
    op = fp_circle_to_op(circle)
    assert op.kind == KiCadPlotterOpKind.CIRCLE
    assert op.payload["cx"] == 5_000_000
    assert op.payload["cy"] == 5_000_000
    assert op.payload["diameter_nm"] == 10_000_000  # 2*radius=10
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 200_000


def test_fp_circle_to_op_no_fill():
    circle = FpCircle(
        center_x=0.0, center_y=0.0,
        end_x=1.0, end_y=0.0,
        stroke=Stroke(width=0.1),
        fill=FillType.NO,
    )
    op = fp_circle_to_op(circle)
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value


def test_fp_rect_to_op():
    rect = FpRect(
        start_x=-1.0, start_y=-2.0,
        end_x=3.0, end_y=4.0,
        stroke=Stroke(width=0.12),
        fill=FillType.YES,
    )
    op = fp_rect_to_op(rect)
    assert op.kind == KiCadPlotterOpKind.RECT
    assert op.payload["x1"] == -1_000_000
    assert op.payload["y1"] == -2_000_000
    assert op.payload["x2"] == 3_000_000
    assert op.payload["y2"] == 4_000_000
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 120_000


def test_fp_poly_to_op_filled():
    poly = FpPoly(
        points=[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        stroke=Stroke(width=0.05),
        fill=FillType.SOLID,
    )
    op = fp_poly_to_op(poly)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["points"] == [
        [0, 0],
        [1_000_000, 0],
        [1_000_000, 1_000_000],
        [0, 1_000_000],
    ]
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 84_700


def test_fp_poly_to_op_unfilled():
    poly = FpPoly(
        points=[(0.0, 0.0), (1.0, 0.0)],
        stroke=Stroke(width=0.1),
        fill=FillType.NO,
    )
    op = fp_poly_to_op(poly)
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value


# ---------------------------------------------------------------------------
# Per-shape emitters: fp_text / property
# ---------------------------------------------------------------------------


def test_fp_text_to_op_basic():
    text = FpText(
        text_type="user", text="HELLO",
        at_x=1.0, at_y=2.0, at_angle=90.0,
        layer="F.SilkS",
    )
    op = fp_text_to_op(text)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["x"] == 1_000_000
    assert op.payload["y"] == 2_000_000
    assert op.payload["text"] == "HELLO"
    assert op.payload["orient_deg"] == 90.0


def test_fp_text_to_op_returns_none_when_hidden():
    text = FpText(text_type="user", text="X", at_x=0.0, at_y=0.0, hide=True)
    assert fp_text_to_op(text) is None


def test_fp_text_from_sexp_parses_hide_yes():
    text = FpText.from_sexp([
        "fp_text",
        "user",
        "HIDDEN",
        ["at", 0.0, 0.0, 0.0],
        ["layer", "F.SilkS"],
        ["hide", "yes"],
    ])

    assert text.hide is True
    assert fp_text_to_op(text) is None


def test_fp_text_to_op_returns_none_when_empty():
    text = FpText(text_type="user", text="", at_x=0.0, at_y=0.0)
    assert fp_text_to_op(text) is None


def test_fp_text_to_op_resolves_reference_variable():
    text = FpText(text_type="user", text="${REFERENCE}", at_x=0.0, at_y=0.0)

    op = fp_text_to_op(text, variables={"Reference": "J1"})

    assert op is not None
    assert op.payload["text"] == "J1"


def test_fp_text_to_op_lifts_effects_font_size():
    effects = Effects(font=Font(size_x=2.0, size_y=2.0))
    text = FpText(
        text_type="user", text="X",
        at_x=0.0, at_y=0.0,
        effects=effects,
    )
    op = fp_text_to_op(text)
    assert op is not None
    assert op.payload["size_x_nm"] == 2_000_000
    assert op.payload["size_y_nm"] == 2_000_000


def test_property_to_op_basic():
    prop = Property(
        name="Reference", value="R1",
        at_x=0.5, at_y=-1.0, at_angle=0.0,
    )
    op = property_to_op(prop)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "R1"
    assert op.payload["x"] == 500_000
    assert op.payload["y"] == -1_000_000


def test_property_to_op_returns_none_when_hidden():
    prop = Property(name="Reference", value="R1", hide=True)
    assert property_to_op(prop) is None


def test_property_from_sexp_metadata_only_is_not_graphical():
    prop = Property.from_sexp(["property", "ki_fp_filters", "wavenumber:R0402"])

    assert prop.graphical is False
    assert property_to_op(prop) is None


def test_property_to_op_returns_none_when_value_empty():
    prop = Property(name="Footprint", value="")
    assert property_to_op(prop) is None


def test_fp_text_box_from_sexp_preserves_border_no():
    box = FpTextBox.from_sexp([
        "fp_text_box",
        "${REFERENCE}",
        ["start", -2.0, -1.0],
        ["end", 2.0, 1.0],
        ["margins", 0.1, 0.2, 0.3, 0.4],
        ["layer", "User.4"],
        ["effects", ["font", ["size", 0.5, 0.5]], ["justify", "left", "top"]],
        ["border", "no"],
        ["stroke", ["width", 0.1], ["type", "default"]],
    ])

    assert box.text == "${REFERENCE}"
    assert box.layer == "User.4"
    assert box.border is False
    assert ["margins", 0.1, 0.2, 0.3, 0.4] in box.to_sexp()
    assert ["border", "no"] in box.to_sexp()


def test_fp_text_box_to_ops_emits_border_and_expanded_text():
    box = FpTextBox(
        text="${REFERENCE}",
        start_x=0.0,
        start_y=0.0,
        end_x=4.0,
        end_y=2.0,
        layer="User.4",
        effects=Effects(font=Font(size_x=0.5, size_y=0.5), justify=["left", "top"]),
        stroke=Stroke(width=0.1),
        border=True,
    )

    ops = fp_text_box_to_ops(box, variables={"REFERENCE": "J1"})

    assert [op.kind for op in ops] == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.TEXT,
    ]
    assert ops[0].payload["x1"] == 0
    assert ops[0].payload["x2"] == 4_000_000
    assert ops[0].payload["width_nm"] == 100_000
    assert ops[1].payload["text"] == "J1"
    assert ops[1].payload["x"] == 0
    assert ops[1].payload["y"] == 0
    assert ops[1].payload["h_align"] == "GR_TEXT_H_ALIGN_LEFT"
    assert ops[1].payload["v_align"] == "GR_TEXT_V_ALIGN_TOP"


# ---------------------------------------------------------------------------
# Pad dispatch: CIRCLE / OVAL / RECT / ROUNDRECT / TRAPEZOID / CUSTOM
# ---------------------------------------------------------------------------


def _make_pad(shape: PadShape, **overrides) -> Pad:
    defaults = dict(
        number="1", pad_type=PadType.SMD, shape=shape,
        at_x=1.0, at_y=2.0, at_angle=45.0,
        size_x=1.5, size_y=0.8,
        layers=["F.Cu"],
    )
    defaults.update(overrides)
    return Pad(**defaults)


def test_pad_to_ops_circle_uses_size_x_as_diameter():
    pad = _make_pad(PadShape.CIRCLE, size_x=1.0, size_y=1.0)
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    op = ops[0]
    assert op.kind == KiCadPlotterOpKind.FLASH_PAD_CIRCLE
    assert op.payload == {
        "x": 1_000_000,
        "y": 2_000_000,  # no Y-flip
        "diameter_nm": 1_000_000,
    }


def test_pad_to_ops_skips_npth_copper_aperture():
    pad = _make_pad(
        PadShape.CIRCLE,
        pad_type=PadType.NP_THRU_HOLE,
        size_x=1.0,
        size_y=1.0,
        drill=1.0,
        layers=["*.Cu", "*.Mask"],
    )

    assert pad_to_ops(pad) == []
    assert pad_drill_to_ops(pad)


def test_pad_to_ops_keeps_oversized_npth_circle_marker():
    pad = _make_pad(
        PadShape.CIRCLE,
        pad_type=PadType.NP_THRU_HOLE,
        size_x=2.5,
        size_y=2.5,
        drill=1.4,
        layers=["*.Cu", "F.Mask"],
    )

    assert len(pad_to_ops(pad)) == 1
    assert pad_drill_to_ops(pad)


def test_pad_to_ops_keeps_npth_marker_when_pad_size_differs_from_drill():
    pad = _make_pad(
        PadShape.RECT,
        pad_type=PadType.NP_THRU_HOLE,
        size_x=0.001,
        size_y=0.001,
        drill=1.0,
        layers=["*.Cu", "*.Mask"],
    )

    assert len(pad_to_ops(pad)) == 1
    assert pad_drill_to_ops(pad)


def test_pad_to_ops_oval():
    pad = _make_pad(PadShape.OVAL)
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    op = ops[0]
    assert op.kind == KiCadPlotterOpKind.FLASH_PAD_OVAL
    assert op.payload["x"] == 1_000_000
    assert op.payload["y"] == 2_000_000
    assert op.payload["size_x_nm"] == 1_500_000
    assert op.payload["size_y_nm"] == 800_000
    assert op.payload["orient_deg"] == 45.0


def test_pad_to_ops_rect():
    pad = _make_pad(PadShape.RECT)
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    assert ops[0].kind == KiCadPlotterOpKind.FLASH_PAD_RECT
    assert ops[0].payload["size_x_nm"] == 1_500_000
    assert ops[0].payload["size_y_nm"] == 800_000


def test_pad_to_ops_trapezoid_uses_rect_delta_corners():
    pad = _make_pad(
        PadShape.TRAPEZOID,
        size_x=2.0,
        size_y=1.0,
        rect_delta_x=0.2,
        rect_delta_y=0.4,
    )
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    assert ops[0].kind == KiCadPlotterOpKind.FLASH_PAD_TRAPEZ
    assert ops[0].payload["corners"] == [
        [-1_200_000, 600_000],
        [1_200_000, 400_000],
        [800_000, -400_000],
        [-800_000, -600_000],
    ]


def test_pad_from_sexp_preserves_trapezoid_rect_delta():
    pad = Pad.from_sexp(
        [
            "pad",
            "1",
            "smd",
            "trapezoid",
            ["at", 1.0, 2.0, 30.0],
            ["size", 2.0, 1.0],
            ["rect_delta", 0.2, 0.4],
            ["layers", "F.Cu", "F.Mask"],
        ]
    )

    assert pad.shape == PadShape.TRAPEZOID
    assert pad.rect_delta_x == 0.2
    assert pad.rect_delta_y == 0.4
    assert ["rect_delta", 0.2, 0.4] in pad.to_sexp()


def test_pad_to_ops_roundrect_uses_explicit_rratio():
    # corner_radius_nm = min(size_x_nm, size_y_nm) * rratio
    # min = 800_000, rratio = 0.25 → 200_000
    pad = _make_pad(PadShape.ROUNDRECT, roundrect_rratio=0.25)
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    op = ops[0]
    assert op.kind == KiCadPlotterOpKind.FLASH_PAD_ROUNDRECT
    assert op.payload["corner_radius_nm"] == 200_000


def test_pad_to_ops_roundrect_defaults_rratio_to_quarter():
    pad = _make_pad(PadShape.ROUNDRECT, size_x=1.0, size_y=1.0)
    # rratio defaults to 0.25 → corner = 250_000
    ops = pad_to_ops(pad)
    assert ops[0].payload["corner_radius_nm"] == 250_000


def test_pad_to_ops_custom_translates_gr_poly_primitives():
    pad = _make_pad(
        PadShape.CUSTOM,
        size_x=0.0, size_y=0.0,
        custom_primitives=[
            PadCustomPrimitive(
                primitive_type="gr_poly",
                points=[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
                width=0.1,
            ),
        ],
    )
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    op = ops[0]
    assert op.kind == KiCadPlotterOpKind.FLASH_PAD_CUSTOM
    assert op.payload["polygons"] == [
        [[0, 0], [1_000_000, 0], [1_000_000, 1_000_000]],
    ]
    assert op.payload["polygon_widths_nm"] == [100_000]


def test_pad_to_ops_custom_preserves_anchor_shape():
    pad = _make_pad(
        PadShape.CUSTOM,
        size_x=2.0, size_y=1.0,
        custom_options=PadCustomOptions(anchor="rect"),
        custom_primitives=[
            PadCustomPrimitive(
                primitive_type="gr_poly",
                points=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
            ),
        ],
    )
    ops = pad_to_ops(pad)

    assert len(ops) == 1
    assert ops[0].payload["anchor_shape"] == "rect"


def test_pad_to_ops_custom_drops_non_gr_poly_primitives():
    pad = _make_pad(
        PadShape.CUSTOM,
        custom_primitives=[
            PadCustomPrimitive(primitive_type="gr_arc", points=[(0.0, 0.0)]),
            PadCustomPrimitive(
                primitive_type="gr_poly",
                points=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
            ),
        ],
    )
    ops = pad_to_ops(pad)
    assert len(ops) == 1
    assert len(ops[0].payload["polygons"]) == 1


def test_pad_drill_to_ops_round_through_hole():
    pad = _make_pad(
        PadShape.CIRCLE,
        pad_type=PadType.THRU_HOLE,
        drill=0.4,
    )

    ops = pad_drill_to_ops(pad)

    assert len(ops) == 1
    assert ops[0].kind == KiCadPlotterOpKind.CIRCLE
    assert ops[0].payload["role"] == "pad_drill"
    assert ops[0].payload["cx"] == 1_000_000
    assert ops[0].payload["cy"] == 2_000_000
    assert ops[0].payload["diameter_nm"] == 400_000


def test_pad_drill_to_ops_oval_slot():
    pad = _make_pad(
        PadShape.OVAL,
        pad_type=PadType.THRU_HOLE,
        at_angle=0.0,
        drill_oval=True,
        drill_width=0.4,
        drill_height=1.0,
    )

    ops = pad_drill_to_ops(pad)

    assert len(ops) == 1
    assert ops[0].kind == KiCadPlotterOpKind.THICK_SEGMENT
    assert ops[0].payload["role"] == "pad_drill"
    assert ops[0].payload["start_x"] == 1_000_000
    assert ops[0].payload["start_y"] == 1_700_000
    assert ops[0].payload["end_x"] == 1_000_000
    assert ops[0].payload["end_y"] == 2_300_000
    assert ops[0].payload["width_nm"] == 400_000


def test_pad_drill_to_ops_npth_falls_back_to_pad_size():
    pad = _make_pad(
        PadShape.CIRCLE,
        pad_type=PadType.NP_THRU_HOLE,
        drill=None,
        size_x=1.2,
        size_y=0.8,
    )

    ops = pad_drill_to_ops(pad)

    assert len(ops) == 1
    assert ops[0].kind == KiCadPlotterOpKind.CIRCLE
    assert ops[0].payload["role"] == "npth_hole"
    assert ops[0].payload["diameter_nm"] == 800_000


# ---------------------------------------------------------------------------
# Top-level: footprint_to_record op order + footprint_to_ir document shape
# ---------------------------------------------------------------------------


def _basic_footprint() -> KiCadFootprint:
    fp = KiCadFootprint()
    fp.name = "TestFP"
    fp.layer = "F.Cu"
    fp.descr = "Test footprint"
    fp.tags = "test"
    fp.attr = ["smd"]
    return fp


def test_footprint_to_record_op_order():
    """
    Mirrors KiCadFootprint.to_sexp emit order:
        properties → fp_texts → fp_lines → fp_arcs → fp_circles →
        fp_rects → fp_polys → pads
    """
    fp = _basic_footprint()
    fp.properties = [Property(name="Reference", value="R1", at_x=0.0, at_y=0.0)]
    fp.fp_texts = [FpText(text_type="user", text="MARK", at_x=0.0, at_y=1.0)]
    fp.fp_lines = [FpLine(start_x=0.0, start_y=0.0, end_x=1.0, end_y=0.0)]
    fp.fp_arcs = [FpArc(
        start_x=0.0, start_y=0.0, mid_x=1.0, mid_y=1.0, end_x=2.0, end_y=0.0,
    )]
    fp.fp_circles = [FpCircle(center_x=0.0, center_y=0.0, end_x=1.0, end_y=0.0)]
    fp.fp_rects = [FpRect(start_x=0.0, start_y=0.0, end_x=1.0, end_y=1.0)]
    fp.fp_polys = [FpPoly(
        points=[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
    )]
    fp.pads = [_make_pad(PadShape.CIRCLE, size_x=0.5, size_y=0.5)]

    record = footprint_to_record(fp)
    kinds = [op.kind for op in record.operations]
    assert kinds == [
        KiCadPlotterOpKind.TEXT,            # property (Reference)
        KiCadPlotterOpKind.TEXT,            # fp_text
        KiCadPlotterOpKind.THICK_SEGMENT,   # fp_line
        KiCadPlotterOpKind.ARC_THREE_POINT, # fp_arc
        KiCadPlotterOpKind.CIRCLE,          # fp_circle
        KiCadPlotterOpKind.RECT,            # fp_rect
        KiCadPlotterOpKind.PLOT_POLY,       # fp_poly
        KiCadPlotterOpKind.FLASH_PAD_CIRCLE,  # pad
    ]


def test_footprint_to_record_property_order_reference_value_first():
    fp = _basic_footprint()
    fp.properties = [
        Property(name="Datasheet", value="ds.pdf", at_x=0.0, at_y=0.0),
        Property(name="Value", value="V1", at_x=0.0, at_y=0.0),
        Property(name="Reference", value="R1", at_x=0.0, at_y=0.0),
    ]
    record = footprint_to_record(fp)
    texts = [op.payload["text"] for op in record.operations]
    # Reference first, then Value, then Datasheet (others in source order)
    assert texts == ["R1", "V1", "ds.pdf"]


def test_footprint_to_record_tags_standalone_ops_with_layers():
    fp = _basic_footprint()
    fp.properties = [
        Property(name="Reference", value="R1", layer="F.Fab"),
    ]
    fp.fp_lines = [
        FpLine(start_x=0.0, start_y=0.0, end_x=1.0, end_y=0.0, layer="F.SilkS"),
    ]
    fp.pads = [
        _make_pad(PadShape.CIRCLE, layers=["F.Cu", "F.Mask"], size_x=1.0, size_y=1.0),
    ]

    record = footprint_to_record(fp)

    assert record.operations[0].payload["layer"] == "F.Fab"
    assert record.operations[1].payload["layer"] == "F.SilkS"
    assert record.operations[2].payload["layers"] == ["F.Cu", "F.Mask"]
    assert record.operations[2].payload["mask_margin_nm"] == 0


def test_footprint_to_record_skips_hidden_properties():
    fp = _basic_footprint()
    fp.properties = [
        Property(name="Reference", value="R1"),
        Property(name="Value", value="V1", hide=True),
        Property(name="Footprint", value="", hide=False),  # empty value
    ]
    record = footprint_to_record(fp)
    texts = [op.payload["text"] for op in record.operations]
    assert texts == ["R1"]  # Value hidden + Footprint empty both dropped


def test_footprint_to_record_skips_hidden_fp_texts():
    fp = _basic_footprint()
    fp.fp_texts = [
        FpText(text_type="user", text="A", at_x=0.0, at_y=0.0),
        FpText(text_type="user", text="B", at_x=0.0, at_y=0.0, hide=True),
        FpText(text_type="user", text="", at_x=0.0, at_y=0.0),
    ]
    record = footprint_to_record(fp)
    texts = [op.payload["text"] for op in record.operations]
    assert texts == ["A"]


def test_footprint_to_record_extras_carry_metadata():
    fp = _basic_footprint()
    fp.locked = True
    fp.placed = False
    fp.uuid = "abcd-1234"
    record = footprint_to_record(fp)
    assert record.uuid == "abcd-1234"
    assert record.kind == "footprint"
    assert record.object_id == "TestFP"
    assert record.extras == {
        "name": "TestFP",
        "layer": "F.Cu",
        "locked": True,
        "placed": False,
        "descr": "Test footprint",
        "tags": "test",
        "attr": ["smd"],
    }


def test_footprint_to_ir_document_shape():
    fp = _basic_footprint()
    fp.fp_lines = [FpLine(start_x=0.0, start_y=0.0, end_x=1.0, end_y=0.0)]
    doc = footprint_to_ir(
        fp, source_path="/path/to/test.kicad_mod", document_id="custom-id",
    )
    assert isinstance(doc, KiCadPlotterDocument)
    assert doc.source_kind == "MOD"
    assert doc.source_path == "/path/to/test.kicad_mod"
    assert doc.document_id == "custom-id"
    assert doc.coordinate_space == {"unit": "nm", "y_axis": "down"}
    assert doc.canvas is None
    assert len(doc.records) == 1
    assert doc.records[0].kind == "footprint"
    assert doc.extras["version"] == fp.version
    assert doc.extras["generator"] == fp.generator
    assert doc.extras["generator_version"] == fp.generator_version
    _plotter_ir_contract_validator().validate(doc.to_dict())


def test_footprint_to_ir_default_document_id_is_name():
    fp = _basic_footprint()
    fp.name = "AutoIdFP"
    doc = footprint_to_ir(fp)
    assert doc.document_id == "AutoIdFP"


def test_footprint_to_ir_empty_footprint_emits_empty_record():
    fp = _basic_footprint()
    doc = footprint_to_ir(fp)
    assert len(doc.records) == 1
    assert doc.records[0].operations == []


def test_footprint_to_svg_uses_ir_layer_filtering():
    fp = _basic_footprint()
    fp.fp_lines = [
        FpLine(
            start_x=-1.0,
            start_y=0.0,
            end_x=1.0,
            end_y=0.0,
            layer="F.SilkS",
            stroke=Stroke(width=0.2),
        ),
    ]
    fp.pads = [
        _make_pad(
            PadShape.CIRCLE,
            at_x=0.0,
            at_y=0.0,
            at_angle=0.0,
            size_x=1.0,
            size_y=1.0,
            layers=["F.Cu"],
        ),
    ]

    copper_svg = fp.to_svg(layers=["F.Cu"])
    silk_svg = fp.to_svg(layers=["F.SilkS"])

    assert "<?xml version=\"1.0\" encoding=\"UTF-8\"?>" in copper_svg
    assert "<circle" in copper_svg
    assert "<polyline" not in copper_svg
    assert "<polyline" in silk_svg
    assert "<circle" not in silk_svg


# ---------------------------------------------------------------------------
# JSON round-trip
# ---------------------------------------------------------------------------


def test_footprint_to_ir_json_round_trip_preserves_pad_geometry():
    fp = _basic_footprint()
    fp.pads = [
        _make_pad(PadShape.RECT, at_x=2.5, at_y=-3.0, size_x=1.6, size_y=0.8),
        _make_pad(PadShape.ROUNDRECT, at_x=5.0, at_y=0.0, roundrect_rratio=0.25),
    ]
    doc = footprint_to_ir(fp)
    blob = doc.to_dict()
    rebuilt = KiCadPlotterDocument.from_dict(blob)
    assert rebuilt.records[0].operations[0].payload["x"] == 2_500_000
    assert rebuilt.records[0].operations[0].payload["y"] == -3_000_000
    assert (
        rebuilt.records[0].operations[1].kind
        == KiCadPlotterOpKind.FLASH_PAD_ROUNDRECT
    )

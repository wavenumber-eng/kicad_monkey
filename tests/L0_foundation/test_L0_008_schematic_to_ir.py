"""
Test L0_008: KiCadSchematic → IR converter (Phase F-4)

Pure-unit coverage for the parser → IR boundary that turns a parsed
``KiCadSchematic`` (mm, Y-down) into a ``KiCadPlotterDocument`` (nm,
Y-down — no flip applied at this boundary). No oracle, no rendering —
just the unit conversion + per-element op emission + record ordering.
"""

from __future__ import annotations

import base64
from pathlib import Path

import pytest

from kicad_monkey import (
    DEFAULT_BUS_WIDTH_MM,
    DEFAULT_JUNCTION_DIAMETER_MM,
    DEFAULT_LABEL_SIZE_RATIO,
    DEFAULT_NO_CONNECT_HALF_MM,
    DEFAULT_TEXT_SIZE_MM,
    DEFAULT_WIRE_WIDTH_MM,
    KiCadFillType,
    KiCadHorizAlign,
    KiCadLineStyle,
    KiCadPlotterDocument,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadSchematic,
    KiCadVertAlign,
    SchBus,
    SchBusEntry,
    SchGlobalLabel,
    SchHierarchicalLabel,
    SchJunction,
    SchLabel,
    SchNoConnect,
    SchSheet,
    SchSymbol,
    SchTextBox,
    SchWire,
    bus_entry_to_op,
    bus_to_op,
    global_label_decoration_to_op,
    global_label_to_op,
    hierarchical_label_decoration_to_op,
    hierarchical_label_to_op,
    junction_to_op,
    label_to_op,
    mm_to_nm,
    no_connect_to_ops,
    paper_size_to_nm,
    sch_text_to_op,
    schematic_image_to_op,
    schematic_rectangle_to_ops,
    schematic_to_ir,
    sheet_background_to_op,
    sheet_outline_to_op,
    sheet_pin_decoration_to_op,
    sheet_pin_to_op,
    sheet_property_to_op,
    wire_to_op,
    text_box_outline_to_op,
    text_box_to_ops,
)
from kicad_monkey.kicad_base import StrokeType
from kicad_monkey.kicad_sch_sheet import SchSheetPin, SchSheetProperty
from kicad_monkey.kicad_primitives import Effects, Font, Stroke
from kicad_monkey.kicad_sch_enums import LabelShape, PinElectricalType, PinGraphicStyle
from kicad_monkey.kicad_sch_image import SchImage
from kicad_monkey.kicad_sch_shapes import (
    SchArc,
    SchBezier,
    SchCircle,
    SchPolyline,
    SchRectangle,
)
from kicad_monkey.kicad_schematic_style import (
    LAYER_BUS,
    LAYER_DNP_MARKER,
    LAYER_GLOBLABEL,
    LAYER_LOCLABEL,
    LAYER_NETCLASS_REFS,
    LAYER_NOTES,
    LAYER_WIRE,
)
from kicad_monkey.kicad_sch_text import SchText
from kicad_monkey.kicad_sch_title_block import PaperSize, TitleBlock
from kicad_monkey.kicad_sym_rectangle import SymFill, SymFillType


def _require_outline_font(
    font_face: str = "Arial",
    *,
    bold: bool = False,
    italic: bool = False,
    allow_substitute: bool = False,
    expected_stems: tuple[str, ...] = (),
) -> str:
    from kicad_monkey.kicad_schematic_to_ir import _outline_font_path

    path = _outline_font_path(
        font_face,
        bold=bold,
        italic=italic,
        allow_substitute=allow_substitute,
    )
    if path is None:
        pytest.skip(f"outline font is not available: {font_face!r}")

    if expected_stems:
        stem = Path(path).stem.casefold()
        if not any(expected.casefold() in stem for expected in expected_stems):
            pytest.skip(f"resolved outline font is not this test's calibrated face: {path}")
    return path


def test_outline_font_path_does_not_substitute_when_disabled(monkeypatch):
    from kicad_monkey import kicad_schematic_to_ir as schematic_ir

    schematic_ir._outline_font_path.cache_clear()
    monkeypatch.setattr(schematic_ir, "_system_outline_font_paths", lambda: {})
    monkeypatch.setattr(
        schematic_ir,
        "_arial_outline_font_path",
        lambda _bold: "fallback-arial.ttf",
    )
    try:
        assert (
            schematic_ir._outline_font_path(
                "Definitely Missing Exact Face",
                allow_substitute=False,
            )
            is None
        )
    finally:
        schematic_ir._outline_font_path.cache_clear()


# ---------------------------------------------------------------------------
# Paper size
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "size, expected_w_mm, expected_h_mm",
    [
        ("A4", 297.0022, 210.0072),
        ("A3", 419.9890, 297.0022),
        ("A2", 594.0044, 419.9890),
        ("A0", 1188.9994, 840.9940),
        ("USLetter", 279.4, 215.9),
        ("USLegal", 355.6, 215.9),
    ],
)
def test_paper_size_to_nm_standard(size, expected_w_mm, expected_h_mm):
    paper = PaperSize(size=size)
    w_nm, h_nm = paper_size_to_nm(paper)
    assert w_nm == mm_to_nm(expected_w_mm)
    assert h_nm == mm_to_nm(expected_h_mm)


def test_paper_size_to_nm_portrait_swaps():
    landscape = paper_size_to_nm(PaperSize(size="A4", portrait=False))
    portrait = paper_size_to_nm(PaperSize(size="A4", portrait=True))
    assert landscape == (mm_to_nm(297.0022), mm_to_nm(210.0072))
    assert portrait == (mm_to_nm(210.0072), mm_to_nm(297.0022))


def test_paper_size_to_nm_user_uses_explicit_dimensions():
    paper = PaperSize(size="User", width=400.0, height=300.0)
    assert paper_size_to_nm(paper) == (mm_to_nm(400.0), mm_to_nm(300.0))


def test_paper_size_to_nm_unknown_falls_back_to_a4():
    paper = PaperSize(size="WeirdoFormat")
    assert paper_size_to_nm(paper) == (mm_to_nm(297.0022), mm_to_nm(210.0072))


# ---------------------------------------------------------------------------
# Wires / buses / bus entries
# ---------------------------------------------------------------------------


def test_wire_to_op_emits_2point_polyline_no_y_flip():
    """Schematic Y is already screen-Y; mm_to_nm only, no negation."""
    wire = SchWire(points=[(10.0, 20.0), (40.0, 20.0)],
                   stroke=Stroke(width=0.0))
    op = wire_to_op(wire)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    pts = op.payload["points"]
    assert pts == [[10_000_000, 20_000_000], [40_000_000, 20_000_000]]
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value
    # 0-width fallback uses the eeschema default (6 mils = 0.1524 mm).
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)


def test_wire_to_op_with_explicit_stroke_width():
    wire = SchWire(points=[(0.0, 0.0), (10.0, 0.0)],
                   stroke=Stroke(width=0.5))
    op = wire_to_op(wire)
    assert op.payload["width_nm"] == mm_to_nm(0.5)


def test_wire_to_op_returns_none_when_empty():
    wire = SchWire(points=[])
    assert wire_to_op(wire) is None


def test_bus_to_op_uses_default_bus_width():
    bus = SchBus(points=[(0.0, 0.0), (10.0, 0.0)],
                 stroke=Stroke(width=0.0))
    op = bus_to_op(bus)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    # Default bus width is 12 mils = 0.3048mm — DOUBLE the wire default.
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_BUS_WIDTH_MM)
    assert op.payload["width_nm"] == 2 * mm_to_nm(DEFAULT_WIRE_WIDTH_MM)


def test_bus_to_op_returns_none_when_empty():
    assert bus_to_op(SchBus(points=[])) is None


def test_bus_entry_to_op_emits_diagonal_segment():
    entry = SchBusEntry(at_x=10.0, at_y=20.0, size_x=2.54, size_y=2.54,
                        stroke=Stroke(width=0.0))
    op = bus_entry_to_op(entry)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    assert op.payload["stroke_color"] == LAYER_WIRE
    pts = op.payload["points"]
    assert pts == [[10_000_000, 20_000_000],
                   [12_540_000, 22_540_000]]


def test_bus_entry_to_op_uses_explicit_stroke_width():
    entry = SchBusEntry(
        at_x=10.0,
        at_y=20.0,
        size_x=2.54,
        size_y=2.54,
        stroke=Stroke(width=0.4),
    )
    op = bus_entry_to_op(entry)
    assert op.payload["width_nm"] == 400_000


# ---------------------------------------------------------------------------
# Junctions
# ---------------------------------------------------------------------------


def test_junction_to_op_default_diameter_filled_circle():
    j = SchJunction(at_x=10.0, at_y=20.0, diameter=0.0)
    op = junction_to_op(j)
    assert op.kind == KiCadPlotterOpKind.CIRCLE
    assert op.payload["cx"] == 10_000_000
    assert op.payload["cy"] == 20_000_000
    assert op.payload["diameter_nm"] == mm_to_nm(DEFAULT_JUNCTION_DIAMETER_MM)
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 0


def test_junction_to_op_explicit_diameter():
    j = SchJunction(at_x=0.0, at_y=0.0, diameter=2.0)
    op = junction_to_op(j)
    assert op.payload["diameter_nm"] == 2_000_000


# ---------------------------------------------------------------------------
# No-connect
# ---------------------------------------------------------------------------


def test_no_connect_emits_two_crossing_segments():
    nc = SchNoConnect(at_x=5.0, at_y=10.0)
    ops = no_connect_to_ops(nc)
    assert len(ops) == 2
    for op in ops:
        assert op.kind == KiCadPlotterOpKind.PLOT_POLY
        assert op.payload["fill"] == KiCadFillType.NO_FILL.value
        assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
        assert len(op.payload["points"]) == 2

    h = mm_to_nm(DEFAULT_NO_CONNECT_HALF_MM)
    cx, cy = mm_to_nm(5.0), mm_to_nm(10.0)
    # First diagonal: top-left → bottom-right.
    assert ops[0].payload["points"] == [[cx - h, cy - h], [cx + h, cy + h]]
    # Second diagonal: bottom-left → top-right.
    assert ops[1].payload["points"] == [[cx - h, cy + h], [cx + h, cy - h]]


def test_no_connect_uses_project_default_line_width_when_provided():
    nc = SchNoConnect(at_x=5.0, at_y=10.0)
    ops = no_connect_to_ops(nc, default_line_width_nm=254_000)
    assert [op.payload["width_nm"] for op in ops] == [254_000, 254_000]


# ---------------------------------------------------------------------------
# Labels
# ---------------------------------------------------------------------------


def _label_with_effects(cls, **kwargs):
    eff = Effects(font=Font(size_x=1.27, size_y=1.27))
    return cls(at_x=0.0, at_y=0.0, at_angle=0.0, effects=eff, **kwargs)


def test_label_to_op_text_body():
    lbl = _label_with_effects(SchLabel, text="NET_A", uuid="lbl-1")
    op = label_to_op(lbl)
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "NET_A"
    assert op.payload["x"] == 0
    assert op.payload["y"] == -349_250
    # No Y-flip — schematic coords are already Y-down.


def test_label_default_text_size_when_effects_missing():
    lbl = SchLabel(text="X", at_x=0.0, at_y=0.0)
    op = label_to_op(lbl)
    assert op.payload["size_x_nm"] == mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    assert op.payload["size_y_nm"] == mm_to_nm(DEFAULT_TEXT_SIZE_MM)


@pytest.mark.parametrize(
    "angle, expected",
    [(0.0, 0.0), (90.0, 90.0), (180.0, 0.0), (270.0, 90.0)],
)
def test_label_to_op_uses_plotted_spin_orientation(angle, expected):
    lbl = SchLabel(
        text="GPIO",
        at_x=0.0,
        at_y=0.0,
        at_angle=angle,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    op = label_to_op(lbl)
    assert op.payload["orient_deg"] == expected


def test_label_to_op_uses_project_default_line_width_for_auto_thickness():
    lbl = SchLabel(
        text="DATA-IN",
        at_x=0.0,
        at_y=0.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    op = label_to_op(lbl, default_line_width_nm=254000)
    assert op.payload["pen_width_nm"] == 254000


def test_label_to_op_uses_bus_color_for_bus_text():
    lbl = SchLabel(
        text="USB_C{USB}",
        at_x=0.0,
        at_y=0.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    op = label_to_op(lbl)
    assert op.payload["color"] == LAYER_BUS


def test_label_to_op_treats_overbar_markup_as_local_label_not_bus():
    lbl = SchLabel(
        text="~{USB_BOOT}",
        at_x=0.0,
        at_y=0.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    op = label_to_op(lbl)
    assert op.payload["color"] == LAYER_LOCLABEL


def test_global_label_to_op_displays_slash_escape_without_bus_color():
    lbl = _label_with_effects(
        SchGlobalLabel,
        text="FP{slash}Boost",
        shape=LabelShape.OUTPUT,
    )
    op = global_label_to_op(lbl)
    assert op.payload["text"] == "FP/Boost"
    assert op.payload["color"] == LAYER_GLOBLABEL


def test_global_label_to_op_treats_overbar_markup_as_global_label_not_bus():
    lbl = _label_with_effects(
        SchGlobalLabel,
        text="NANO_~{RESET}_0",
        shape=LabelShape.BIDIRECTIONAL,
    )
    op = global_label_to_op(lbl)
    assert op.payload["color"] == LAYER_GLOBLABEL


def test_global_label_to_op_emits_text():
    lbl = _label_with_effects(SchGlobalLabel, text="VCC",
                              shape=LabelShape.OUTPUT)
    op = global_label_to_op(lbl)
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "VCC"
    assert (op.payload["x"], op.payload["y"]) == (476_250, 90_804)


def test_label_to_op_preserves_href_context():
    lbl = SchLabel(
        text="DOCS",
        at_x=0.0,
        at_y=0.0,
        at_angle=0.0,
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27),
            href="https://example.test/label",
        ),
    )

    op = label_to_op(lbl)

    assert op.payload["context"]["hyperlink"]["href"] == "https://example.test/label"


def test_hierarchical_label_to_op_emits_text():
    lbl = _label_with_effects(SchHierarchicalLabel, text="DATA",
                              shape=LabelShape.BIDIRECTIONAL)
    op = hierarchical_label_to_op(lbl)
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "DATA"
    assert (op.payload["x"], op.payload["y"]) == (1_460_500, 0)
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value


# ---------------------------------------------------------------------------
# Top-level text
# ---------------------------------------------------------------------------


def test_sch_text_to_op_uses_plotted_single_text_anchor():
    txt = SchText(text="hello\nworld", at_x=1.0, at_y=2.0, at_angle=0.0)
    op = sch_text_to_op(txt)
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["multiline"] is True
    assert op.payload["x"] == 1_000_000
    assert op.payload["y"] == 1_462_000
    assert op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value
    assert "context" not in op.payload


def test_sch_text_to_op_preserves_href_context():
    txt = SchText(
        text="linked note",
        at_x=1.0,
        at_y=2.0,
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27),
            href="https://example.test/note",
        ),
    )

    op = sch_text_to_op(txt)

    assert op.payload["context"]["hyperlink"]["href"] == "https://example.test/note"


def test_sch_text_to_op_rotates_outline_adjust_separately_from_plot_offset():
    txt = SchText(
        text="+12v Batt",
        at_x=40.64,
        at_y=78.486,
        at_angle=90.0,
        effects=Effects(font=Font(size_x=2.286, size_y=2.286)),
    )
    op = sch_text_to_op(txt)
    assert op.payload["x"] == 40_121_500
    assert op.payload["y"] == 78_236_000


def test_sch_text_to_op_uses_berkeley_mono_trial_outline_adjust():
    _require_outline_font(
        "Berkeley Mono Trial",
        allow_substitute=True,
        expected_stems=("cascadia",),
    )
    txt = SchText(
        text="1    2    3",
        at_x=90.424,
        at_y=340.36,
        effects=Effects(font=Font(face="Berkeley Mono Trial", size_x=8.509, size_y=8.509)),
    )

    op = sch_text_to_op(txt)

    assert op.payload["x"] == 90_424_000
    assert op.payload["y"] == 337_967_200


def test_sch_text_to_op_uses_times_new_romans_outline_adjust():
    _require_outline_font("Times New Roman", bold=True, allow_substitute=False)
    txt = SchText(
        text="${PROJECT_NAME}",
        at_x=25.4,
        at_y=40.64,
        effects=Effects(
            font=Font(
                face="Times New Roman",
                size_x=11.0,
                size_y=11.0,
                thickness=1.0,
                bold=True,
            ),
            justify=["left", "bottom"],
        ),
    )

    op = sch_text_to_op(txt, project_vars={"PROJECT_NAME": "Example_Public_Project"})

    assert op.payload["text"] == "Example_Public_Project"
    assert op.payload["y"] == 37_946_600


def test_sch_text_to_op_uses_avenir_black_outline_adjust():
    _require_outline_font(
        "Avenir Black",
        bold=True,
        allow_substitute=True,
        expected_stems=("book",),
    )
    txt = SchText(
        text="[CM5]",
        at_x=27.94,
        at_y=22.352,
        effects=Effects(
            font=Font(face="Avenir Black", size_x=5.27, size_y=5.27, bold=True),
        ),
    )

    op = sch_text_to_op(txt)

    assert op.payload["y"] == 20_733_700


def test_sch_text_to_op_uses_berkeley_mono_outline_adjust():
    _require_outline_font("Berkeley Mono", bold=True, allow_substitute=False)
    txt = SchText(
        text="DC Path",
        at_x=330.2,
        at_y=327.66,
        effects=Effects(
            font=Font(
                face="Berkeley Mono",
                size_x=3.81,
                size_y=3.81,
                thickness=0.254,
                bold=True,
            ),
        ),
    )

    op = sch_text_to_op(txt)

    assert op.payload["y"] == 326_367_200


def test_sch_text_to_op_preserves_one_trailing_blank_line_like_wx_split():
    txt = SchText(text="one\n\n", at_x=1.0, at_y=2.0)

    op = sch_text_to_op(txt)

    assert op.payload["text"] == "one\n"
    assert op.payload["multiline"] is True


def test_sch_text_to_op_uses_empty_first_line_for_outline_adjust():
    txt = SchText(
        text="\nHPZ1608D102-R60TF",
        at_x=429.514,
        at_y=107.95,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )

    op = sch_text_to_op(txt)

    assert op.payload["text"] == "\nHPZ1608D102-R60TF"
    assert op.payload["y"] == 108_208_000


def test_sch_text_to_op_uses_project_default_line_width_for_auto_thickness():
    txt = SchText(
        text="Fiducials",
        at_x=1.0,
        at_y=2.0,
        effects=Effects(font=Font(size_x=2.286, size_y=2.286)),
    )
    op = sch_text_to_op(txt, default_line_width_nm=254000)
    assert op.payload["pen_width_nm"] == 254000


def test_sch_text_to_op_expands_project_text_variables():
    txt = SchText(
        text="Revision: ${REVISION}\nProject: ${PROJECT_NAME}\n${UNKNOWN}",
        at_x=1.0,
        at_y=2.0,
    )

    op = sch_text_to_op(
        txt,
        project_vars={
            "REVISION": "RevA",
            "PROJECT_NAME": "Example_Public_Project",
        },
    )

    assert op.payload["text"] == (
        "Revision: RevA\nProject: Example_Public_Project\n${UNKNOWN}"
    )


def test_text_box_outline_to_op_uses_default_shape_stroke():
    tb = SchTextBox(
        text=".control\nversion\n.endc",
        at_x=109.22,
        at_y=130.81,
        size_x=14.986,
        size_y=8.7796,
        stroke=Stroke(width=0.0),
        effects=Effects(font=Font(size_x=1.524, size_y=1.524),
                        justify=["left", "top"]),
    )

    op = text_box_outline_to_op(tb)

    assert op.kind == KiCadPlotterOpKind.RECT
    assert op.payload["x1"] == 109_220_000
    assert op.payload["y1"] == 130_810_000
    assert op.payload["x2"] == 124_206_000
    assert op.payload["y2"] == 139_589_600
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)


def test_text_box_to_ops_emits_outline_and_per_line_text_positions():
    tb = SchTextBox(
        text=".control\nversion\n.endc",
        at_x=109.22,
        at_y=130.81,
        size_x=14.986,
        size_y=8.7796,
        stroke=Stroke(width=0.0),
        effects=Effects(font=Font(size_x=1.524, size_y=1.524),
                        justify=["left", "top"]),
    )

    ops = text_box_to_ops(tb)

    assert [op.kind for op in ops] == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.TEXT,
        KiCadPlotterOpKind.TEXT,
        KiCadPlotterOpKind.TEXT,
    ]
    assert [op.payload["text"] for op in ops[1:]] == [
        ".control", "version", ".endc"
    ]
    # SCH_TEXTBOX::GetLegacyTextMargin(): stroke/2 + text_height*0.75.
    assert (ops[1].payload["x"], ops[1].payload["y"]) == (
        110_363_000, 131_953_000
    )
    line_step_nm = int(round(1_524_000 * 1.68))
    assert ops[2].payload["y"] - ops[1].payload["y"] == line_step_nm
    assert ops[1].payload["multiline"] is False
    assert ops[1].payload["h_align"] == KiCadHorizAlign.LEFT.value
    assert ops[1].payload["v_align"] == KiCadVertAlign.TOP.value


def test_text_box_to_ops_defaults_vertical_alignment_to_center():
    tb = SchTextBox(
        text="one\ntwo",
        at_x=10.0,
        at_y=20.0,
        size_x=30.0,
        size_y=10.0,
        margins=(1.0, 1.0, 1.0, 1.0),
        effects=Effects(font=Font(size_x=1.0, size_y=1.0), justify=["left"]),
    )

    ops = text_box_to_ops(tb)
    text_ops = [op for op in ops if op.kind == KiCadPlotterOpKind.TEXT]

    assert [op.payload["v_align"] for op in text_ops] == [
        KiCadVertAlign.CENTER.value,
        KiCadVertAlign.CENTER.value,
    ]
    assert text_ops[0].payload["y"] == 24_160_000
    assert text_ops[1].payload["y"] == 25_840_000


def test_text_box_to_ops_defaults_horizontal_alignment_to_center():
    tb = SchTextBox(
        text="centered",
        at_x=10.0,
        at_y=20.0,
        size_x=30.0,
        size_y=10.0,
        margins=(1.0, 1.0, 1.0, 1.0),
        effects=Effects(font=Font(size_x=1.0, size_y=1.0)),
    )

    ops = text_box_to_ops(tb)
    text_op = next(op for op in ops if op.kind == KiCadPlotterOpKind.TEXT)

    assert text_op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert text_op.payload["x"] == 25_000_000
    assert text_op.payload["y"] == 25_000_000


def test_text_box_to_ops_ignores_trailing_blank_line_for_centered_block():
    tb = SchTextBox(
        text="one\ntwo\n",
        at_x=0.0,
        at_y=0.0,
        size_x=20.0,
        size_y=20.0,
        margins=(0.0, 0.0, 0.0, 0.0),
        effects=Effects(font=Font(size_x=1.0, size_y=1.0), justify=["left"]),
    )

    ops = text_box_to_ops(tb)
    text_ops = [op for op in ops if op.kind == KiCadPlotterOpKind.TEXT]

    assert [op.payload["text"] for op in text_ops] == ["one", "two"]
    assert [op.payload["y"] for op in text_ops] == [9_160_000, 10_840_000]


def test_text_box_to_ops_expands_project_text_variables():
    tb = SchTextBox(
        text="${FOO}\n${BAR}",
        at_x=0.0,
        at_y=0.0,
        size_x=50.0,
        size_y=20.0,
        margins=(0.0, 0.0, 0.0, 0.0),
        effects=Effects(font=Font(size_x=1.0, size_y=1.0), justify=["left"]),
    )

    ops = text_box_to_ops(tb, project_vars={"FOO": "alpha", "BAR": "beta"})
    text_ops = [op for op in ops if op.kind == KiCadPlotterOpKind.TEXT]

    assert [op.payload["text"] for op in text_ops] == ["alpha", "beta"]


def test_text_box_to_ops_preserves_href_context_on_text_lines():
    tb = SchTextBox(
        text="alpha\nbeta",
        at_x=0.0,
        at_y=0.0,
        size_x=50.0,
        size_y=20.0,
        margins=(0.0, 0.0, 0.0, 0.0),
        effects=Effects(
            font=Font(size_x=1.0, size_y=1.0),
            justify=["left"],
            href="https://example.test/box",
        ),
    )

    text_ops = [
        op for op in text_box_to_ops(tb) if op.kind == KiCadPlotterOpKind.TEXT
    ]

    assert [op.payload["context"]["hyperlink"]["href"] for op in text_ops] == [
        "https://example.test/box",
        "https://example.test/box",
    ]


def test_text_box_to_ops_wraps_long_lines_to_content_width():
    _require_outline_font("Arial", bold=True, allow_substitute=False)
    tb = SchTextBox(
        text=(
            "2) TOC useful for large projects, nice overview of what sheet "
            "does what, title, date, revision, variant version, legends, "
            "notes, Top/Bottom views"
        ),
        at_x=10.16,
        at_y=320.04,
        size_x=161.29,
        size_y=50.8,
        margins=(1.9049, 1.9049, 1.9049, 1.9049),
        effects=Effects(
            font=Font(size_x=2.54, size_y=2.54, thickness=0.381, bold=True),
            justify=["left", "top"],
        ),
    )

    ops = text_box_to_ops(tb)

    assert [op.payload["text"] for op in ops[1:]] == [
        (
            "2) TOC useful for large projects, nice overview of what sheet "
            "does what, title, date, revision,"
        ),
        "variant version, legends, notes, Top/Bottom views",
    ]


def test_text_box_to_ops_wraps_against_content_width_minus_pen():
    tb = SchTextBox(
        text="QSPI32 boot mode support (UG1085)",
        at_x=124.46,
        at_y=71.12,
        size_x=-31.75,
        size_y=12.7,
        margins=(0.9525, 0.9525, 0.9525, 0.9525),
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27),
            justify=["left", "top"],
        ),
    )

    ops = text_box_to_ops(tb)
    text_ops = [op for op in ops if op.kind == KiCadPlotterOpKind.TEXT]

    assert [op.payload["text"] for op in text_ops] == [
        "QSPI32 boot mode support",
        "(UG1085)",
    ]


def test_text_box_to_ops_fill_pass_uses_fill_color_and_zero_width():
    tb = SchTextBox(
        text="filled",
        at_x=10.0,
        at_y=20.0,
        size_x=30.0,
        size_y=40.0,
        stroke=Stroke(width=-0.0001),
        fill=SymFill(type=SymFillType.COLOR, color=(0, 255, 255, 1.0)),
    )

    ops = text_box_to_ops(tb)

    assert [op.kind for op in ops[:2]] == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.RECT,
    ]
    assert ops[0].payload["fill"] == KiCadFillType.FILLED_WITH_COLOR.value
    assert ops[0].payload["width_nm"] == 0
    assert ops[0].payload["stroke_color"] == "#00FFFFFF"
    assert ops[0].payload["fill_color"] == "#00FFFFFF"
    assert ops[1].payload["fill"] == KiCadFillType.NO_FILL.value
    assert ops[1].payload["width_nm"] == 0
    assert "fill_color" not in ops[1].payload


# ---------------------------------------------------------------------------
# Top-level graphic shapes and images
# ---------------------------------------------------------------------------


def _png_b64(width: int, height: int, pixels_per_meter: int | None = None) -> str:
    data = (
        b"\x89PNG\r\n\x1a\n"
        + (13).to_bytes(4, "big")
        + b"IHDR"
        + int(width).to_bytes(4, "big")
        + int(height).to_bytes(4, "big")
        + b"\x08\x02\x00\x00\x00"
        + b"\x00\x00\x00\x00"
    )
    if pixels_per_meter is not None:
        phys = (
            int(pixels_per_meter).to_bytes(4, "big")
            + int(pixels_per_meter).to_bytes(4, "big")
            + b"\x01"
        )
        data += (9).to_bytes(4, "big") + b"pHYs" + phys + b"\x00\x00\x00\x00"
    return base64.b64encode(data).decode("ascii")


def _jpeg_b64(
    width: int,
    height: int,
    *,
    density: int = 1,
    density_units: int = 0,
) -> str:
    app0 = (
        b"JFIF\x00"
        + b"\x01\x01"
        + bytes([density_units])
        + int(density).to_bytes(2, "big")
        + int(density).to_bytes(2, "big")
        + b"\x00\x00"
    )
    sof0 = (
        b"\x08"
        + int(height).to_bytes(2, "big")
        + int(width).to_bytes(2, "big")
        + b"\x03\x01\x11\x00\x02\x11\x00\x03\x11\x00"
    )
    data = (
        b"\xff\xd8"
        + b"\xff\xe0"
        + (len(app0) + 2).to_bytes(2, "big")
        + app0
        + b"\xff\xc0"
        + (len(sof0) + 2).to_bytes(2, "big")
        + sof0
        + b"\xff\xd9"
    )
    return base64.b64encode(data).decode("ascii")


def _bmp_b64(
    width: int,
    height: int,
    *,
    pixels_per_meter: int = 0,
) -> str:
    row_size = ((int(width) * 3 + 3) // 4) * 4
    image_size = row_size * int(height)
    data = (
        b"BM"
        + (54 + image_size).to_bytes(4, "little")
        + b"\x00\x00\x00\x00"
        + (54).to_bytes(4, "little")
        + (40).to_bytes(4, "little")
        + int(width).to_bytes(4, "little", signed=True)
        + int(height).to_bytes(4, "little", signed=True)
        + (1).to_bytes(2, "little")
        + (24).to_bytes(2, "little")
        + (0).to_bytes(4, "little")
        + image_size.to_bytes(4, "little")
        + int(pixels_per_meter).to_bytes(4, "little", signed=True)
        + int(pixels_per_meter).to_bytes(4, "little", signed=True)
        + (0).to_bytes(4, "little")
        + (0).to_bytes(4, "little")
        + (b"\x00" * image_size)
    )
    return base64.b64encode(data).decode("ascii")


def test_schematic_to_ir_emits_top_level_graphic_shapes_no_y_flip():
    sch = KiCadSchematic()
    sch.paper = PaperSize(size="A4")
    stroke = Stroke(width=0.2, type=StrokeType.SOLID)
    sch.polylines = [
        SchPolyline(points=[(1.0, 2.0), (3.0, 4.0)], stroke=stroke, uuid="p1")
    ]
    sch.arcs = [
        SchArc(
            start_x=10.0, start_y=11.0,
            mid_x=12.0, mid_y=13.0,
            end_x=14.0, end_y=15.0,
            stroke=stroke,
            uuid="a1",
        )
    ]
    sch.circles = [
        SchCircle(center_x=20.0, center_y=21.0, radius=2.5, stroke=stroke, uuid="c1")
    ]
    sch.rectangles = [
        SchRectangle(start_x=30.0, start_y=31.0, end_x=40.0, end_y=41.0,
                     stroke=stroke, uuid="r1")
    ]
    sch.beziers = [
        SchBezier(
            points=[(50.0, 51.0), (52.0, 53.0), (54.0, 55.0), (56.0, 57.0)],
            stroke=stroke,
            uuid="b1",
        )
    ]

    records = schematic_to_ir(sch).records[1:]

    assert [rec.kind for rec in records] == [
        "graphic_polyline",
        "graphic_arc",
        "graphic_circle",
        "graphic_rectangle",
        "graphic_bezier",
    ]
    assert records[0].operations[0].payload["points"] == [
        [1_000_000, 2_000_000],
        [3_000_000, 4_000_000],
    ]
    assert records[0].operations[0].payload["stroke_color"] == LAYER_NOTES
    assert records[1].operations[0].payload["start_y"] == 11_000_000
    assert records[2].operations[0].payload["cy"] == 21_000_000
    assert records[2].operations[0].payload["diameter_nm"] == 5_000_000
    assert records[3].operations[0].payload["y2"] == 41_000_000
    assert records[4].operations[0].kind == KiCadPlotterOpKind.BEZIER_CURVE


def test_schematic_graphic_filled_rectangle_splits_fill_then_outline():
    rect = SchRectangle(
        start_x=10.0,
        start_y=20.0,
        end_x=30.0,
        end_y=40.0,
        stroke=Stroke(width=0.254, type=StrokeType.SOLID),
        fill=SymFill(type=SymFillType.COLOR, color=(0, 255, 255, 1.0)),
    )

    ops = schematic_rectangle_to_ops(rect)

    assert [op.kind for op in ops] == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.RECT,
    ]
    assert ops[0].payload["fill"] == KiCadFillType.FILLED_WITH_COLOR.value
    assert ops[0].payload["width_nm"] == 0
    assert ops[0].payload["stroke_color"] == "#00FFFFFF"
    assert ops[1].payload["fill"] == KiCadFillType.NO_FILL.value
    assert ops[1].payload["width_nm"] == 254_000
    assert "fill_color" not in ops[1].payload


def test_schematic_image_to_op_extracts_png_dimensions_and_scale():
    data_b64 = _png_b64(10, 5)
    img = SchImage(
        at_x=144.78,
        at_y=111.76,
        scale=1.5,
        uuid="img1",
        data=[data_b64[:20], data_b64[20:]],
    )

    op = schematic_image_to_op(img)

    assert op.kind == KiCadPlotterOpKind.PLOT_IMAGE
    assert op.payload["x"] == 144_780_000
    assert op.payload["y"] == 111_760_000
    assert op.payload["width_nm"] == mm_to_nm(10 * 1.5 * 25.4 / 300.0)
    assert op.payload["height_nm"] == mm_to_nm(5 * 1.5 * 25.4 / 300.0)
    assert op.payload["image_data_b64"] == data_b64
    assert op.payload["image_format"] == "png"
    assert op.payload["stroke_color"] == LAYER_NOTES


def test_schematic_image_to_op_uses_png_physical_density_when_present():
    data_b64 = _png_b64(407, 407, pixels_per_meter=3780)
    img = SchImage(
        at_x=277.114,
        at_y=173.736,
        scale=0.123469,
        data=[data_b64],
    )

    op = schematic_image_to_op(img)

    assert op.payload["width_nm"] == mm_to_nm(407 * 0.123469 * 25.4 / 96.0)
    assert op.payload["height_nm"] == mm_to_nm(407 * 0.123469 * 25.4 / 96.0)


def test_schematic_image_to_op_uses_jpeg_jfif_density_when_present():
    data_b64 = _jpeg_b64(699, 443, density=96, density_units=1)
    img = SchImage(
        at_x=220.98,
        at_y=30.48,
        scale=0.0733675,
        data=[data_b64],
    )

    op = schematic_image_to_op(img)

    assert op.payload["image_format"] == "jpeg"
    assert op.payload["width_nm"] == mm_to_nm(699 * 0.0733675 * 25.4 / 96.0)
    assert op.payload["height_nm"] == mm_to_nm(443 * 0.0733675 * 25.4 / 96.0)


def test_schematic_image_to_op_uses_bmp_density_like_kicad_wx_loader():
    data_b64 = _bmp_b64(512, 512, pixels_per_meter=5669)
    img = SchImage(
        at_x=334.01,
        at_y=264.16,
        scale=0.110937,
        data=[data_b64],
    )

    op = schematic_image_to_op(img)

    assert op.payload["image_format"] == "bmp"
    assert op.payload["width_nm"] == mm_to_nm(512 * 0.110937 * 25.4 / 142.0)
    assert op.payload["height_nm"] == mm_to_nm(512 * 0.110937 * 25.4 / 142.0)


# ---------------------------------------------------------------------------
# Top-level converter
# ---------------------------------------------------------------------------


def _empty_schematic() -> KiCadSchematic:
    sch = KiCadSchematic()
    sch.uuid = "doc-uuid"
    sch.paper = PaperSize(size="A4")
    sch.title_block = TitleBlock(
        title="Demo", date="2026-05-09", rev="A",
        company="Wavenumber", comments={1: "first"}
    )
    return sch


def test_schematic_to_ir_empty_sheet_emits_only_header():
    sch = _empty_schematic()
    doc = schematic_to_ir(sch, source_path="design.kicad_sch")
    assert isinstance(doc, KiCadPlotterDocument)
    assert doc.source_kind == "SCH"
    assert doc.coordinate_space == {"unit": "nm", "y_axis": "down"}
    assert doc.canvas == {
        "width_nm": mm_to_nm(297.0022),
        "height_nm": mm_to_nm(210.0072),
    }
    assert len(doc.records) == 1
    header = doc.records[0]
    assert header.kind == "sheet_header"
    assert header.object_id == "doc-uuid"
    assert header.extras["paper_size"] == "A4"
    assert header.extras["sheet_width_nm"] == mm_to_nm(297.0022)
    assert header.extras["sheet_height_nm"] == mm_to_nm(210.0072)
    assert header.extras["title_block"]["title"] == "Demo"
    assert header.extras["title_block"]["comments"] == {1: "first"}


def test_schematic_to_ir_emits_records_in_kicad_order():
    sch = _empty_schematic()
    sch.wires = [SchWire(points=[(0.0, 0.0), (10.0, 0.0)], uuid="w1")]
    sch.buses = [SchBus(points=[(0.0, 5.0), (10.0, 5.0)], uuid="bus1")]
    sch.bus_entries = [SchBusEntry(at_x=10.0, at_y=5.0, size_x=2.54,
                                   size_y=2.54, uuid="be1")]
    sch.junctions = [SchJunction(at_x=5.0, at_y=0.0, uuid="j1")]
    sch.no_connects = [SchNoConnect(at_x=20.0, at_y=0.0, uuid="nc1")]
    sch.labels = [_label_with_effects(SchLabel, text="L1", uuid="lab1")]
    sch.global_labels = [_label_with_effects(SchGlobalLabel, text="VCC",
                                              uuid="gl1",
                                              shape=LabelShape.INPUT)]
    sch.hierarchical_labels = [
        _label_with_effects(SchHierarchicalLabel, text="HL",
                            uuid="hl1", shape=LabelShape.OUTPUT)
    ]
    sch.texts = [SchText(text="note", at_x=1.0, at_y=2.0, uuid="t1")]
    sch.text_boxes = [SchTextBox(text="box", at_x=2.0, at_y=3.0, uuid="tb1")]
    sch.symbols = [SchSymbol(lib_id="Device:R", at_x=15.0, at_y=15.0,
                              uuid="s1")]
    sch.sheets = [SchSheet(at_x=100.0, at_y=100.0, size_x=50.0, size_y=40.0)]

    doc = schematic_to_ir(sch)
    kinds = [r.kind for r in doc.records]
    assert kinds == [
        "sheet_header",
        "wire",
        "bus",
        "bus_entry",
        "junction",
        "no_connect",
        "label",
        "global_label",
        "hierarchical_label",
        "text",
        "text_box",
        "symbol_instance",
        "sheet",
    ]


def test_schematic_to_ir_expands_project_variables_in_text_records():
    sch = _empty_schematic()
    sch.texts = [
        SchText(
            text="Revision: ${CUSTOM_REVISION}",
            at_x=1.0,
            at_y=2.0,
            uuid="t1",
        )
    ]

    doc = schematic_to_ir(sch, project_vars={"CUSTOM_REVISION": "RevA"})
    rec = next(record for record in doc.records if record.kind == "text")

    assert rec.extras["text"] == "Revision: RevA"
    assert rec.operations[0].payload["text"] == "Revision: RevA"


def test_schematic_to_ir_expands_builtin_text_variables_in_text_records():
    sch = _empty_schematic()
    sch.texts = [
        SchText(
            text="[${#}/${##}] ${TITLE} ${REVISION} ${COMPANY} ${COMMENT1} ${VARIANT}",
            at_x=1.0,
            at_y=2.0,
            uuid="t1",
        )
    ]

    doc = schematic_to_ir(
        sch,
        sheet_index=2,
        sheet_count=4,
        project_vars={"VARIANT": "ProjectVariant"},
    )
    rec = next(record for record in doc.records if record.kind == "text")

    assert rec.operations[0].payload["text"] == "[2/4] Demo A Wavenumber first "


def test_symbol_instance_record_unresolved_lib_id_is_header_only():
    # When the placement's lib_id is not in schematic.lib_symbols, the
    # F-6.4 composer falls back to header-only (operations=[]).
    sch = _empty_schematic()
    sym = SchSymbol(
        lib_id="Device:R", at_x=10.0, at_y=20.0, at_angle=90.0,
        unit=1, dnp=True, mirror="x", uuid="sym1",
    )
    sch.symbols = [sym]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    assert rec.kind == "symbol_instance"
    assert rec.operations == []  # Lib symbol unresolved → empty body.
    assert rec.extras["lib_id"] == "Device:R"
    assert rec.extras["at_x_nm"] == mm_to_nm(10.0)
    assert rec.extras["at_y_nm"] == mm_to_nm(20.0)
    assert rec.extras["at_angle_deg"] == 90.0
    assert rec.extras["unit"] == 1
    assert rec.extras["mirror"] == "x"
    assert rec.extras["dnp"] is True


def test_symbol_instance_record_composes_body_when_lib_sym_resolves():
    # Phase F-6.4 — when schematic.lib_symbols contains a matching
    # entry for sym.lib_id, the placement's body is composed via
    # lib_symbol_to_ir + the placement transform.
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_primitives import Stroke
    from kicad_monkey.kicad_sym_rectangle import SymRectangle

    rect = SymRectangle(
        start_x=-1.0, start_y=-1.0, end_x=1.0, end_y=1.0,
        stroke=Stroke(width=0.0, type="default"),
    )
    sub = LibSubSymbol(
        name="R_1_0", unit=1, style=0, rectangles=[rect]
    )
    lib_sym = LibSymbol(name="Device:R", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="Device:R", at_x=100.0, at_y=200.0, at_angle=0.0,
            unit=1, convert=1, mirror=None, uuid="r1",
        )
    ]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    assert rec.kind == "symbol_instance"
    assert len(rec.operations) >= 1
    # First op is the rect → translated to placement.
    rect_ops = [op for op in rec.operations if op.kind == KiCadPlotterOpKind.RECT]
    assert len(rect_ops) == 1
    p = rect_ops[0].payload
    # Lib coords ([-1,1] mm) → flipped Y in lib → translated by (100,200) mm.
    # mm_to_nm: 1 mm = 1_000_000 nm.
    # Lib (-1,-1) → after F-3 Y-flip: (-1_000_000, +1_000_000). Translate by
    # (100*1e6, 200*1e6) → (99_000_000, 201_000_000). Lib ( 1, 1) → after
    # Y-flip: (1_000_000, -1_000_000). Translate → (101_000_000, 199_000_000).
    assert (p["x1"], p["y1"]) == (99_000_000, 201_000_000)
    assert (p["x2"], p["y2"]) == (101_000_000, 199_000_000)


def test_symbol_instance_record_dims_and_marks_dnp_symbols():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_primitives import Stroke
    from kicad_monkey.kicad_sym_property import SymProperty
    from kicad_monkey.kicad_sym_rectangle import SymRectangle

    rect = SymRectangle(
        start_x=-1.0,
        start_y=-1.0,
        end_x=1.0,
        end_y=1.0,
        stroke=Stroke(width=0.0, type="default"),
    )
    sub = LibSubSymbol(name="Device:R_1_0", unit=1, style=0, rectangles=[rect])
    lib_sym = LibSymbol(name="Device:R", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="Device:R",
            at_x=100.0,
            at_y=200.0,
            unit=1,
            convert=1,
            dnp=True,
            properties=[
                SymProperty(
                    key="Reference",
                    value="R1",
                    id=0,
                    at_x=100.0,
                    at_y=197.0,
                    hide=False,
                )
            ],
        )
    ]

    rec = schematic_to_ir(sch).records[-1]
    rect_op = next(op for op in rec.operations if op.kind == KiCadPlotterOpKind.RECT)
    text_op = next(op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT)
    marker_ops = [
        op for op in rec.operations if op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    ]

    assert rect_op.payload["stroke_color"] == "#9C9B99FF"
    assert text_op.payload["color"] != "#006464FF"
    assert len(marker_ops) == 2
    assert {op.payload["stroke_color"] for op in marker_ops} == {"#DC090DD9"}
    assert {op.payload["width_nm"] for op in marker_ops} == {457_200}


def test_schematic_to_ir_overplots_dnp_marker_for_overlapping_dnp_symbol():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_primitives import Stroke
    from kicad_monkey.kicad_sym_rectangle import SymRectangle

    rect = SymRectangle(
        start_x=-1.0,
        start_y=-1.0,
        end_x=1.0,
        end_y=1.0,
        stroke=Stroke(width=0.0, type="default"),
    )
    lib_sym = LibSymbol(
        name="Device:R",
        subsymbols=[LibSubSymbol(name="Device:R_1_0", unit=1, style=0, rectangles=[rect])],
    )
    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(lib_id="Device:R", at_x=100.0, at_y=200.0, dnp=True, uuid="r1"),
        SchSymbol(lib_id="Device:R", at_x=100.0, at_y=200.0, dnp=False, uuid="r2"),
    ]

    doc = schematic_to_ir(sch)
    overplot = next(record for record in doc.records if record.uuid == "r1:overplot")
    marker_ops = [
        op for op in overplot.operations if op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    ]

    assert len(marker_ops) == 2
    assert {op.payload["stroke_color"] for op in marker_ops} == {"#DC090DD9"}


def test_symbol_overlap_detection_ignores_pin_text_boxes():
    from kicad_monkey import KiCadPlotterOp
    from kicad_monkey.kicad_schematic_to_ir import _overlapping_symbol_indices

    pin_text_only = KiCadPlotterRecord(
        uuid="pin-text",
        kind="symbol_instance",
        object_id="pin-text",
        operations=[
            KiCadPlotterOp.start_block(data_ref="symbol_pin"),
            KiCadPlotterOp.text(
                x=0,
                y=0,
                text="PIN",
                size_x_nm=1_270_000,
                size_y_nm=1_270_000,
                h_align=KiCadHorizAlign.CENTER,
                v_align=KiCadVertAlign.CENTER,
            ),
            KiCadPlotterOp.end_block(),
        ],
    )
    body = KiCadPlotterRecord(
        uuid="body",
        kind="symbol_instance",
        object_id="body",
        operations=[
            KiCadPlotterOp.rect(
                x1=-100_000,
                y1=-100_000,
                x2=100_000,
                y2=100_000,
                fill=KiCadFillType.NO_FILL,
                width_nm=0,
            )
        ],
    )

    assert _overlapping_symbol_indices([pin_text_only, body]) == set()


def test_symbol_instance_record_wraps_visible_pin_ops_in_group():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sch_symbol import SchSymbolPin
    from kicad_monkey.kicad_sym_pin import SymPin

    pin = SymPin(
        electrical_type=PinElectricalType.INPUT,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        at_angle=0.0,
        length=2.54,
        name="IN",
        number="1",
        uuid="lib-pin-uuid",
    )
    sub = LibSubSymbol(name="Device:R_1_0", unit=1, style=0, pins=[pin])
    lib_sym = LibSymbol(name="Device:R", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="Device:R",
            at_x=10.0,
            at_y=20.0,
            unit=1,
            convert=1,
            uuid="symbol-uuid",
            pins=[SchSymbolPin(number="1", uuid="pin-uuid")],
        )
    ]

    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    kinds = [op.kind for op in rec.operations]
    assert KiCadPlotterOpKind.START_BLOCK in kinds
    assert KiCadPlotterOpKind.END_BLOCK in kinds

    start_index = kinds.index(KiCadPlotterOpKind.START_BLOCK)
    end_index = kinds.index(KiCadPlotterOpKind.END_BLOCK)
    assert start_index < end_index
    assert KiCadPlotterOpKind.PLOT_POLY in kinds[start_index:end_index]

    start = rec.operations[start_index]
    assert start.payload["label"] == "pin-uuid"
    assert start.payload["data_uuid"] == "pin-uuid"
    assert start.payload["data_ref"] == "symbol_pin"
    assert start.payload["object_id"] == "pin-uuid"
    assert start.payload["extra_attrs"]["primitive"] == "pin"
    assert start.payload["extra_attrs"]["object-type"] == "pin"
    assert start.payload["extra_attrs"]["pin"] == "1"
    assert start.payload["extra_attrs"]["symbol-uuid"] == "symbol-uuid"
    assert start.payload["extra_attrs"]["lib-pin-uuid"] == "lib-pin-uuid"


def test_symbol_instance_pin_text_uses_selected_alternate_name():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sch_symbol import SchSymbolPin
    from kicad_monkey.kicad_sym_pin import PinAlternate, SymPin

    pin = SymPin(
        electrical_type=PinElectricalType.BIDIRECTIONAL,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        at_angle=0.0,
        length=2.54,
        name="PA5",
        number="H6",
        alternates=[
            PinAlternate(
                name="UART0_RX",
                electrical_type=PinElectricalType.BIDIRECTIONAL,
                graphic_style=PinGraphicStyle.LINE,
            )
        ],
    )
    sub = LibSubSymbol(name="CPU_1_0", unit=1, style=0, pins=[pin])
    lib_sym = LibSymbol(name="CPU", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="CPU",
            at_x=10.0,
            at_y=20.0,
            unit=1,
            convert=1,
            uuid="symbol-uuid",
            pins=[SchSymbolPin(number="H6", uuid="pin-uuid", alternate="UART0_RX")],
        )
    ]

    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    text_bodies = [op.payload["text"] for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT]

    assert "UART0_RX" in text_bodies
    assert "PA5" not in text_bodies


def test_symbol_instance_zero_length_vertical_pin_text_keeps_orientation():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sym_pin import SymPin

    pin = SymPin(
        electrical_type=PinElectricalType.POWER_IN,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        at_angle=90.0,
        length=0.0,
        name="VCC",
        number="5",
    )
    sub = LibSubSymbol(name="Device:PWR_1_0", unit=1, style=0, pins=[pin])
    lib_sym = LibSymbol(name="Device:PWR", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="Device:PWR",
            at_x=10.0,
            at_y=20.0,
            unit=1,
            convert=1,
            uuid="symbol-uuid",
        )
    ]

    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    text_ops = [op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT]
    number_op = text_ops[0]
    name_op = text_ops[1]

    assert number_op.payload["text"] == "5"
    assert number_op.payload["x"] == 9_746_000
    assert number_op.payload["y"] == 20_000_000
    assert number_op.payload["orient_deg"] == pytest.approx(90.0)

    assert name_op.payload["text"] == "VCC"
    assert name_op.payload["x"] == 10_000_000
    assert name_op.payload["y"] == 19_492_000
    assert name_op.payload["orient_deg"] == pytest.approx(90.0)
    assert name_op.payload["h_align"] == KiCadHorizAlign.LEFT.value


def test_symbol_instance_pin_text_skips_zero_sized_name_and_number():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sym_pin import SymPin

    zero_effects = Effects(font=Font(size_x=0.0, size_y=0.0))
    pin = SymPin(
        electrical_type=PinElectricalType.POWER_IN,
        graphic_style=PinGraphicStyle.LINE,
        at_x=0.0,
        at_y=0.0,
        at_angle=0.0,
        length=2.54,
        name="+3V3",
        name_effects=zero_effects,
        number="1",
        number_effects=zero_effects,
    )
    sub = LibSubSymbol(name="Device:PWR_1_0", unit=1, style=0, pins=[pin])
    lib_sym = LibSymbol(name="Device:PWR", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="Device:PWR",
            at_x=10.0,
            at_y=20.0,
            unit=1,
            convert=1,
            uuid="symbol-uuid",
        )
    ]

    doc = schematic_to_ir(sch)
    rec = doc.records[-1]

    assert any(op.kind == KiCadPlotterOpKind.PLOT_POLY for op in rec.operations)
    assert [op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT] == []


def test_symbol_instance_record_transforms_inverted_pin_geometry():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sch_symbol import SchSymbolPin
    from kicad_monkey.kicad_sym_pin import SymPin

    pin = SymPin(
        electrical_type=PinElectricalType.PASSIVE,
        graphic_style=PinGraphicStyle.INVERTED,
        at_x=0.0,
        at_y=0.0,
        at_angle=0.0,
        length=2.54,
        name="~",
        number="1",
    )
    sub = LibSubSymbol(name="Device:X_1_0", unit=1, style=0, pins=[pin])
    lib_sym = LibSymbol(name="Device:X", pin_numbers_hide=True, subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="Device:X",
            at_x=10.0,
            at_y=20.0,
            unit=1,
            convert=1,
            mirror="y",
            uuid="symbol-uuid",
            pins=[SchSymbolPin(number="1", uuid="pin-uuid")],
        )
    ]

    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    pin_ops = rec.operations[1:-1]
    assert rec.operations[0].kind == KiCadPlotterOpKind.START_BLOCK
    assert rec.operations[-1].kind == KiCadPlotterOpKind.END_BLOCK
    assert [op.kind for op in pin_ops] == [
        KiCadPlotterOpKind.CIRCLE,
        KiCadPlotterOpKind.PLOT_POLY,
    ]
    assert pin_ops[0].payload["cx"] == 8_095_000
    assert pin_ops[0].payload["cy"] == 20_000_000
    assert pin_ops[0].payload["diameter_nm"] == 1_270_000
    assert pin_ops[1].payload["points"] == [
        [8_730_000, 20_000_000],
        [10_000_000, 20_000_000],
    ]


def test_symbol_instance_record_rotated_placement():
    # Phase F-6.4 — rotation transforms each body op coordinate.
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_primitives import Stroke
    from kicad_monkey.kicad_sym_rectangle import SymRectangle

    rect = SymRectangle(
        start_x=0.0, start_y=0.0, end_x=2.0, end_y=0.0,  # horizontal segment
        stroke=Stroke(width=0.0, type="default"),
    )
    sub = LibSubSymbol(name="X_1_0", unit=1, style=0, rectangles=[rect])
    lib_sym = LibSymbol(name="X", subsymbols=[sub])

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="X", at_x=0.0, at_y=0.0, at_angle=90.0,
            unit=1, convert=1, mirror=None, uuid="x1",
        )
    ]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    rect_ops = [op for op in rec.operations if op.kind == KiCadPlotterOpKind.RECT]
    assert len(rect_ops) == 1
    p = rect_ops[0].payload
    # Lib (0,0) → Y-flip (0, 0) → rotate 90 (0, 0) → translate (0,0). Stays (0,0).
    # KiCad schematic placement rotates in screen coordinates, so at=90
    # maps the local +X edge upward in the IR's Y-down space.
    assert (p["x1"], p["y1"]) == (0, 0)
    assert (p["x2"], p["y2"]) == (0, -2_000_000)


def test_symbol_instance_body_text_uses_device_plot_orientation_for_rotation():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sym_text import SymText

    text = SymText(
        text="RGB",
        at_x=2.286,
        at_y=-4.191,
        at_angle=0.0,
        effects=Effects(font=Font(size_x=0.762, size_y=0.762)),
    )
    lib_sym = LibSymbol(
        name="X",
        subsymbols=[LibSubSymbol(name="X_1_0", unit=1, style=0, texts=[text])],
    )

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="X",
            at_x=100.0,
            at_y=200.0,
            at_angle=90.0,
            unit=1,
            convert=1,
            mirror=None,
            uuid="x1",
        )
    ]

    rec = schematic_to_ir(sch).records[-1]
    text_op = next(op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT)

    assert text_op.payload["x"] == 104_191_000
    assert text_op.payload["y"] == 197_714_000
    assert text_op.payload["orient_deg"] == pytest.approx(90.0)
    assert text_op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert text_op.payload["v_align"] == KiCadVertAlign.CENTER.value


def test_symbol_instance_body_text_expands_project_vars_and_multiline():
    from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
    from kicad_monkey.kicad_lib_symbol import LibSymbol
    from kicad_monkey.kicad_sym_text import SymText

    text = SymText(
        text="Copyright ${YEAR}\n\n${LICENSE}\n",
        at_x=1.0,
        at_y=-2.0,
        at_angle=0.0,
        effects=Effects(font=Font(size_x=1.0, size_y=1.0), justify=["left"]),
    )
    lib_sym = LibSymbol(
        name="OHL",
        subsymbols=[LibSubSymbol(name="OHL_1_1", unit=1, style=1, texts=[text])],
    )

    sch = _empty_schematic()
    sch.lib_symbols = [lib_sym]
    sch.symbols = [
        SchSymbol(
            lib_id="OHL",
            at_x=100.0,
            at_y=200.0,
            at_angle=0.0,
            unit=1,
            convert=1,
            mirror=None,
            uuid="ohl1",
        )
    ]

    rec = schematic_to_ir(
        sch,
        project_vars={"YEAR": "2024", "LICENSE": "CERN OHL"},
    ).records[-1]
    text_op = next(op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT)

    assert text_op.payload["text"] == "Copyright 2024\n\nCERN OHL"
    assert text_op.payload["multiline"] is True
    assert text_op.payload["x"] == 101_000_000
    assert text_op.payload["y"] == 202_000_000


def test_symbol_instance_record_emits_visible_property_text_ops():
    # Phase F-6.7 — visible (non-hidden, non-empty) symbol properties
    # are emitted as Text ops at their absolute schematic coords.
    from kicad_monkey.kicad_sym_property import SymProperty

    sch = _empty_schematic()
    sym = SchSymbol(
        lib_id="Device:R", at_x=100.0, at_y=200.0, at_angle=0.0,
        unit=1, convert=1, mirror=None, uuid="r1",
        properties=[
            SymProperty(
                key="Reference", value="R1", id=0,
                at_x=101.0, at_y=198.0, at_angle=0.0, hide=False,
            ),
            SymProperty(
                key="Value", value="10k", id=1,
                at_x=101.0, at_y=202.0, at_angle=0.0, hide=False,
            ),
            SymProperty(
                key="Footprint", value="R_0603", id=2,
                at_x=99.0, at_y=200.0, at_angle=0.0, hide=True,  # hidden
            ),
            SymProperty(
                key="Datasheet", value="", id=3,  # empty value
                at_x=0.0, at_y=0.0, at_angle=0.0, hide=False,
            ),
        ],
    )
    sch.symbols = [sym]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    text_ops = [op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT]
    # Only Reference + Value are visible & non-empty.
    assert len(text_ops) == 2
    assert text_ops[0].payload["text"] == "R1"
    assert (text_ops[0].payload["x"], text_ops[0].payload["y"]) == (
        mm_to_nm(101.0), mm_to_nm(198.0)
    )
    assert text_ops[1].payload["text"] == "10k"
    assert (text_ops[1].payload["x"], text_ops[1].payload["y"]) == (
        mm_to_nm(101.0), mm_to_nm(202.0)
    )


def test_symbol_instance_record_uses_reference_from_matching_sheet_instance():
    from kicad_monkey.kicad_sch_symbol import SchSymbolInstance
    from kicad_monkey.kicad_sym_property import SymProperty

    sch = _empty_schematic()
    sch.symbols = [
        SchSymbol(
            lib_id="Device:R",
            at_x=10.0,
            at_y=20.0,
            properties=[
                SymProperty(
                    key="Reference",
                    value="R?",
                    id=0,
                    at_x=10.0,
                    at_y=20.0,
                    hide=False,
                )
            ],
            instances=[
                SchSymbolInstance(path="/root/a", reference="R1"),
                SchSymbolInstance(path="/root/b", reference="R2"),
            ],
        )
    ]

    doc = schematic_to_ir(sch, sheet_instance_path="/root/b")
    rec = doc.records[-1]
    text_op = next(op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT)

    assert text_op.payload["text"] == "R2"


def test_symbol_property_to_op_uses_default_size_when_effects_absent():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    prop = SymProperty(
        key="Reference", value="C7", id=0,
        at_x=10.0, at_y=20.0, at_angle=90.0,
        effects=None, hide=False,
    )
    op = symbol_property_to_op(prop)
    assert op is not None
    assert op.payload["text"] == "C7"
    assert op.payload["orient_deg"] == pytest.approx(90.0)
    # Default text size is DEFAULT_TEXT_SIZE_MM (1.27 mm).
    assert op.payload["size_x_nm"] == mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    assert op.payload["size_y_nm"] == mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    assert op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value


def test_symbol_property_to_op_uses_project_default_line_width_for_auto_thickness():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    prop = SymProperty(
        key="Reference", value="TP3", id=0,
        at_x=10.0, at_y=20.0,
        effects=Effects(font=Font(size_x=2.0066, size_y=2.0066)),
        hide=False,
    )
    op = symbol_property_to_op(prop, default_line_width_nm=254000)
    assert op is not None
    assert op.payload["pen_width_nm"] == 254000


def test_symbol_property_project_default_line_width_keeps_tiny_text_clamp():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    prop = SymProperty(
        key="Footprint", value="Package_DIP:DIP-8_W7.62mm_LongPads", id=2,
        at_x=10.0, at_y=20.0,
        effects=Effects(font=Font(size_x=0.254, size_y=0.254)),
        hide=False,
    )
    op = symbol_property_to_op(prop, default_line_width_nm=254000)
    assert op is not None
    assert op.payload["pen_width_nm"] == 63_500


def test_symbol_property_to_op_appends_multi_unit_reference_suffix():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    prop = SymProperty(
        key="Reference", value="U10", id=0,
        at_x=10.0, at_y=20.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
        hide=False,
    )
    op = symbol_property_to_op(prop, reference_unit_suffix="B")
    assert op is not None
    assert op.payload["text"] == "U10B"


def test_symbol_property_to_op_includes_shown_property_name():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    prop = SymProperty(
        key="Mounted",
        value="Yes",
        id=5,
        at_x=232.918,
        at_y=164.846,
        show_name=True,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27), justify=["left"]),
        hide=False,
    )

    op = symbol_property_to_op(prop)

    assert op is not None
    assert op.payload["text"] == "Mounted: Yes"


def test_symbol_property_to_op_preserves_href_context():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    prop = SymProperty(
        key="Datasheet",
        value="open",
        id=3,
        at_x=10.0,
        at_y=20.0,
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27),
            href="https://example.test/datasheet",
        ),
        hide=False,
    )

    op = symbol_property_to_op(prop)

    assert op is not None
    assert op.payload["context"]["hyperlink"]["href"] == "https://example.test/datasheet"


def test_symbol_property_to_op_centers_left_berkeley_mono_field_like_kicad():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    _require_outline_font("Berkeley Mono", allow_substitute=False)
    parent = SchSymbol(lib_id="Mechanical:RF_Shield", at_x=0.0, at_y=0.0)
    prop = SymProperty(
        key="Value",
        value="RF Shield Cover",
        id=1,
        at_x=232.664,
        at_y=307.594,
        effects=Effects(
            font=Font(face="Berkeley Mono", size_x=1.27, size_y=1.27),
            justify=["left"],
        ),
        hide=False,
    )

    op = symbol_property_to_op(prop, parent)

    assert op is not None
    assert op.payload["x"] == 240_657_500
    assert op.payload["y"] == 307_594_000


def test_symbol_property_to_op_uses_berkeley_mono_bottom_bbox_center_like_kicad():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    _require_outline_font("Berkeley Mono", bold=True, allow_substitute=False)
    parent = SchSymbol(lib_id="Connector:S8101-46R", at_x=0.0, at_y=0.0)
    prop = SymProperty(
        key="Reference",
        value="M1",
        id=0,
        at_x=332.486,
        at_y=303.276,
        effects=Effects(
            font=Font(
                face="Berkeley Mono",
                size_x=1.905,
                size_y=1.905,
                thickness=0.3048,
                bold=True,
            ),
            justify=["left", "bottom"],
        ),
        hide=False,
    )

    op = symbol_property_to_op(prop, parent)

    assert op is not None
    assert op.payload["x"] == 334_084_700
    assert op.payload["y"] == 302_217_200


def test_symbol_property_to_op_uses_berkeley_mono_top_bbox_center_like_kicad():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    _require_outline_font("Berkeley Mono", bold=True, allow_substitute=False)
    parent = SchSymbol(lib_id="FPGA:XC7Z010", at_x=0.0, at_y=0.0)
    prop = SymProperty(
        key="Reference",
        value="IC1",
        id=0,
        at_x=167.64,
        at_y=161.544,
        effects=Effects(
            font=Font(
                face="Berkeley Mono",
                size_x=1.905,
                size_y=1.905,
                thickness=0.3048,
                bold=True,
            ),
            justify=["left", "top"],
        ),
        hide=False,
    )

    op = symbol_property_to_op(prop, parent, reference_unit_suffix="A")

    assert op is not None
    assert op.payload["text"] == "IC1A"
    assert op.payload["x"] == 170_837_400
    assert op.payload["y"] == 162_602_800


def test_symbol_property_to_op_centers_rotated_overbar_field_like_kicad():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    _require_outline_font("Arial", allow_substitute=False)
    parent = SchSymbol(
        lib_id="PCM_EEZ_unsorted:PCB test point",
        at_x=83.82,
        at_y=81.28,
        at_angle=90.0,
        unit=1,
        convert=1,
        mirror=None,
        uuid="tp1",
    )
    prop = SymProperty(
        key="Label",
        value="~{OE}",
        id=10,
        at_x=85.09,
        at_y=83.82,
        at_angle=90.0,
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27),
            justify=["right", "top"],
        ),
        hide=False,
    )

    op = symbol_property_to_op(prop, parent, default_line_width_nm=152_400)

    assert op is not None
    assert op.payload["x"] == 86_373_500
    assert op.payload["y"] == 82_997_500
    assert op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value


def test_symbol_property_to_op_returns_none_for_hidden_or_empty():
    from kicad_monkey import symbol_property_to_op
    from kicad_monkey.kicad_sym_property import SymProperty

    hidden = SymProperty(key="Footprint", value="X", id=2, hide=True)
    empty = SymProperty(key="Datasheet", value="", id=3, hide=False)
    tilde = SymProperty(key="Datasheet", value="~", id=3, hide=False)
    assert symbol_property_to_op(hidden) is None
    assert symbol_property_to_op(empty) is None
    assert symbol_property_to_op(tilde) is None


def test_schematic_to_ir_emits_netclass_flag_property_fields():
    from kicad_monkey.kicad_sch_label import SchNetclassFlag
    from kicad_monkey.kicad_sym_property import SymProperty

    sch = _empty_schematic()
    sch.netclass_flags = [
        SchNetclassFlag(
            text="",
            at_x=383.286,
            at_y=236.22,
            uuid="flag-1",
            properties=[
                SymProperty(
                    key="Net Class",
                    value="40Z_SE",
                    id=0,
                    at_x=383.286,
                    at_y=236.22,
                    effects=Effects(
                        font=Font(size_x=1.27, size_y=1.27, italic=True),
                        justify=["right"],
                    ),
                    hide=False,
                ),
                SymProperty(
                    key="Component Class",
                    value="",
                    id=1,
                    at_x=383.286,
                    at_y=238.76,
                    effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
                    hide=False,
                ),
            ],
        )
    ]

    doc = schematic_to_ir(sch)
    rec = next(r for r in doc.records if r.kind == "netclass_flag")
    text_ops = [op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT]

    assert len(text_ops) == 1
    op = text_ops[0]
    assert op.payload["text"] == "40Z_SE"
    assert op.payload["x"] == 383_286_000
    assert op.payload["y"] == 236_220_000
    assert op.payload["h_align"] == KiCadHorizAlign.RIGHT.value
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value
    assert op.payload["italic"] is True


def test_schematic_to_ir_emits_round_netclass_flag_marker():
    from kicad_monkey.kicad_sch_label import SchNetclassFlag

    sch = _empty_schematic()
    sch.netclass_flags = [
        SchNetclassFlag(
            at_x=10.0,
            at_y=20.0,
            at_angle=0.0,
            length=2.54,
            shape="round",
            uuid="flag-round",
            effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
        )
    ]

    rec = next(r for r in schematic_to_ir(sch).records if r.kind == "netclass_flag")
    line, circle = rec.operations

    assert line.kind == KiCadPlotterOpKind.THICK_SEGMENT
    assert line.payload["start_x"] == mm_to_nm(10.0)
    assert line.payload["start_y"] == mm_to_nm(20.0)
    assert line.payload["end_x"] == mm_to_nm(10.0)
    assert line.payload["end_y"] == mm_to_nm(17.968)
    assert line.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    assert line.payload["stroke_color"] == LAYER_NETCLASS_REFS
    assert circle.kind == KiCadPlotterOpKind.CIRCLE
    assert circle.payload["cx"] == mm_to_nm(10.0)
    assert circle.payload["cy"] == mm_to_nm(17.46)
    assert circle.payload["diameter_nm"] == mm_to_nm(1.016)
    assert circle.payload["fill"] == KiCadFillType.NO_FILL.value
    assert circle.payload["stroke_color"] == LAYER_NETCLASS_REFS


def test_schematic_to_ir_emits_table_cell_text_like_textbox():
    sch = KiCadSchematic.from_text("""
(kicad_sch (version 20240101) (generator eeschema) (generator_version "10.0")
  (uuid "test-uuid")
  (paper "A4")
  (lib_symbols)
  (table
    (column_count 1)
    (uuid "table-1")
    (cells
      (table_cell "Board shall have 1GB Ethernet interface"
        (exclude_from_sim no)
        (at 96.52 128.905 0)
        (size 236.22 5.715)
        (margins 0.9525 0.9525 0.9525 0.9525)
        (span 1 1)
        (fill (type none))
        (effects
          (font
            (face "Fragment Mono")
            (size 2.54 2.54)
          )
          (justify left)
        )
        (uuid "cell-1")
      )
    )
  )
)
""")

    doc = schematic_to_ir(sch)
    rec = next(r for r in doc.records if r.kind == "table")
    text_op = next(op for op in rec.operations if op.kind == KiCadPlotterOpKind.TEXT)

    assert text_op.payload["text"] == "Board shall have 1GB Ethernet interface"
    assert text_op.payload["x"] == 97_472_500
    assert text_op.payload["y"] == 131_762_500
    assert text_op.payload["h_align"] == KiCadHorizAlign.LEFT.value
    assert text_op.payload["v_align"] == KiCadVertAlign.CENTER.value


def test_sheet_record_carries_outline_rect_and_geometry_extras():
    sch = _empty_schematic()
    sh = SchSheet(at_x=100.0, at_y=200.0, size_x=80.0, size_y=60.0)
    # SchSheet stores sheet_name/sheet_file in properties; for L0 we
    # only need the dataclass-level position/size to make it through.
    sch.sheets = [sh]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    assert rec.kind == "sheet"
    # F-6.8 emits the outline rect (no background fill on a default-
    # constructed sheet — fill_color is None), no pins, no properties.
    assert len(rec.operations) == 2
    outline = rec.operations[0]
    assert outline.kind.value == "Rect"
    assert outline.payload["x1"] == mm_to_nm(100.0)
    assert outline.payload["y1"] == mm_to_nm(200.0)
    assert outline.payload["x2"] == mm_to_nm(180.0)
    assert outline.payload["y2"] == mm_to_nm(260.0)
    assert outline.payload["fill"] == "NO_FILL"
    assert rec.operations[1].payload == outline.payload
    assert rec.extras["at_x_nm"] == mm_to_nm(100.0)
    assert rec.extras["at_y_nm"] == mm_to_nm(200.0)
    assert rec.extras["size_x_nm"] == mm_to_nm(80.0)
    assert rec.extras["size_y_nm"] == mm_to_nm(60.0)


def test_dnp_sheet_is_dimmed_and_crossed_like_kicad():
    sch = _empty_schematic()
    sch.sheets = [
        SchSheet(
            at_x=10.0,
            at_y=20.0,
            size_x=30.0,
            size_y=40.0,
            uuid="dnp-sheet",
            dnp=True,
        )
    ]

    rec = next(r for r in schematic_to_ir(sch).records if r.kind == "sheet")
    assert rec.extras["dnp"] is True
    assert len(rec.operations) == 4
    assert rec.operations[0].payload["stroke_color"] != "#840000FF"

    first, second = rec.operations[-2:]
    assert first.kind == KiCadPlotterOpKind.THICK_SEGMENT
    assert first.payload["start_x"] == mm_to_nm(10.0)
    assert first.payload["start_y"] == mm_to_nm(20.0)
    assert first.payload["end_x"] == mm_to_nm(40.0)
    assert first.payload["end_y"] == mm_to_nm(60.0)
    assert first.payload["stroke_color"] == LAYER_DNP_MARKER
    assert second.payload["start_x"] == mm_to_nm(40.0)
    assert second.payload["start_y"] == mm_to_nm(20.0)
    assert second.payload["end_x"] == mm_to_nm(10.0)
    assert second.payload["end_y"] == mm_to_nm(60.0)
    assert second.payload["stroke_color"] == LAYER_DNP_MARKER


def test_rule_area_emits_closed_source_styled_geometry_and_policy():
    from kicad_monkey.kicad_sch_rule_area import SchRuleArea

    sch = _empty_schematic()
    sch.rule_areas = [
        SchRuleArea(
            locked=True,
            exclude_from_sim=True,
            in_bom=False,
            on_board=False,
            dnp=True,
            shape=SchPolyline(
                points=[(1.0, 2.0), (5.0, 2.0), (5.0, 6.0), (1.0, 6.0)],
                stroke=Stroke(
                    width=0.0,
                    type=StrokeType.DASH,
                    color=(194, 0, 0, 1.0),
                ),
                fill=SymFill(type=SymFillType.NONE),
                uuid="rule-shape",
            ),
        )
    ]

    rec = next(r for r in schematic_to_ir(sch).records if r.kind == "rule_area")
    assert rec.uuid == "rule-shape"
    assert rec.object_id == "rule-shape"
    assert rec.extras == {
        "shape": "polyline",
        "locked": True,
        "exclude_from_sim": True,
        "in_bom": False,
        "on_board": False,
        "dnp": True,
    }
    assert len(rec.operations) == 1
    op = rec.operations[0]
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["points"] == [
        [mm_to_nm(1.0), mm_to_nm(2.0)],
        [mm_to_nm(5.0), mm_to_nm(2.0)],
        [mm_to_nm(5.0), mm_to_nm(6.0)],
        [mm_to_nm(1.0), mm_to_nm(6.0)],
        [mm_to_nm(1.0), mm_to_nm(2.0)],
    ]
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    assert op.payload["line_style"] == KiCadLineStyle.DASH.value
    assert op.payload["stroke_color"] == "#C20000FF"
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value


def test_sheet_record_wraps_sheet_pin_ops_in_group():
    sch = _empty_schematic()
    sh = SchSheet(
        at_x=100.0,
        at_y=200.0,
        size_x=80.0,
        size_y=60.0,
        uuid="sheet-uuid",
    )
    sh.properties = [
        SchSheetProperty(key="Sheetname", value="Child"),
        SchSheetProperty(key="Sheetfile", value="child.kicad_sch"),
    ]
    sh.pins = [
        SchSheetPin(
            name="OUT",
            shape=LabelShape.OUTPUT,
            at_x=100.0,
            at_y=210.0,
            uuid="sheet-pin-uuid",
        )
    ]
    sch.sheets = [sh]

    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    kinds = [op.kind for op in rec.operations]
    start_index = kinds.index(KiCadPlotterOpKind.START_BLOCK)
    end_index = kinds.index(KiCadPlotterOpKind.END_BLOCK)
    pin_ops = rec.operations[start_index + 1:end_index]

    start = rec.operations[start_index]
    assert start.payload["label"] == "sheet-pin-uuid"
    assert start.payload["data_uuid"] == "sheet-pin-uuid"
    assert start.payload["data_ref"] == "sheet_pin"
    assert start.payload["object_id"] == "sheet-pin-uuid"
    assert start.payload["extra_attrs"] == {
        "primitive": "sheet-entry",
        "object-type": "sheet-pin",
        "sheet-uuid": "sheet-uuid",
        "sheet-name": "Child",
        "sheet-file": "child.kicad_sch",
        "pin": "OUT",
        "pin-name": "OUT",
        "shape": "output",
    }
    assert any(op.kind == KiCadPlotterOpKind.TEXT for op in pin_ops)
    assert any(op.kind == KiCadPlotterOpKind.PLOT_POLY for op in pin_ops)


def test_skips_empty_wire_records():
    sch = _empty_schematic()
    sch.wires = [
        SchWire(points=[], uuid="empty"),
        SchWire(points=[(0.0, 0.0), (10.0, 0.0)], uuid="ok"),
    ]
    doc = schematic_to_ir(sch)
    wire_records = [r for r in doc.records if r.kind == "wire"]
    assert len(wire_records) == 1
    assert wire_records[0].object_id == "ok"


# ---------------------------------------------------------------------------
# JSON round-trip
# ---------------------------------------------------------------------------


def test_schematic_doc_round_trips_through_dict():
    sch = _empty_schematic()
    sch.wires = [SchWire(points=[(0.0, 0.0), (10.0, 0.0)], uuid="w1",
                          stroke=Stroke(width=0.2))]
    sch.junctions = [SchJunction(at_x=5.0, at_y=0.0, diameter=1.0,
                                  uuid="j1")]
    doc = schematic_to_ir(sch, source_path="x.kicad_sch")

    data = doc.to_dict()
    rebuilt = KiCadPlotterDocument.from_dict(data)
    assert rebuilt.source_kind == "SCH"
    assert rebuilt.coordinate_space == {"unit": "nm", "y_axis": "down"}
    assert [r.kind for r in rebuilt.records] == [
        "sheet_header", "wire", "junction"
    ]
    wire_rec = rebuilt.records[1]
    assert wire_rec.operations[0].kind == KiCadPlotterOpKind.PLOT_POLY
    assert wire_rec.operations[0].payload["width_nm"] == mm_to_nm(0.2)


# ---------------------------------------------------------------------------
# F-6.8 hierarchical sheet bodies
# ---------------------------------------------------------------------------


def test_sheet_outline_to_op_returns_norm_filled_rect_at_default_width():
    sh = SchSheet(at_x=10.0, at_y=20.0, size_x=80.0, size_y=60.0)
    op = sheet_outline_to_op(sh)
    assert op.kind == KiCadPlotterOpKind.RECT
    assert op.payload["x1"] == mm_to_nm(10.0)
    assert op.payload["y1"] == mm_to_nm(20.0)
    assert op.payload["x2"] == mm_to_nm(90.0)
    assert op.payload["y2"] == mm_to_nm(80.0)
    assert op.payload["fill"] == "NO_FILL"
    # Default Stroke() has width=0 → falls back to wire-default.
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)


def test_sheet_outline_to_op_uses_explicit_stroke_width():
    sh = SchSheet(
        at_x=0.0, at_y=0.0, size_x=10.0, size_y=10.0,
        stroke=Stroke(width=0.5),
    )
    op = sheet_outline_to_op(sh)
    assert op.payload["width_nm"] == mm_to_nm(0.5)


def test_wire_to_op_carries_stroke_style_and_color():
    wire = SchWire(
        points=[(0.0, 0.0), (10.0, 0.0)],
        stroke=Stroke(
            width=0.25,
            type=StrokeType.DASH_DOT,
            color=(10, 20, 30, 0.5),
        ),
    )
    op = wire_to_op(wire)
    assert op is not None
    assert op.payload["width_nm"] == mm_to_nm(0.25)
    assert op.payload["line_style"] == "DASH_DOT"
    assert op.payload["stroke_color"] == "#0A141E80"


def test_sheet_background_to_op_returns_none_when_fill_color_absent():
    sh = SchSheet(at_x=0.0, at_y=0.0, size_x=10.0, size_y=10.0,
                  fill_color=None)
    assert sheet_background_to_op(sh) is None


def test_sheet_background_to_op_returns_none_when_alpha_zero():
    sh = SchSheet(at_x=0.0, at_y=0.0, size_x=10.0, size_y=10.0,
                  fill_color=(255, 255, 255, 0.0))
    assert sheet_background_to_op(sh) is None


def test_sheet_background_to_op_emits_filled_rect_when_alpha_positive():
    sh = SchSheet(at_x=10.0, at_y=20.0, size_x=80.0, size_y=60.0,
                  fill_color=(255, 200, 100, 0.5))
    op = sheet_background_to_op(sh)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.RECT
    assert op.payload["fill"] == "FILLED_SHAPE"
    assert op.payload["fill_color"] == "#FFC86480"
    assert op.payload["stroke_color"] == "#FFC86480"
    assert op.payload["x1"] == mm_to_nm(10.0)
    assert op.payload["y2"] == mm_to_nm(80.0)


def test_sheet_property_to_op_skips_hidden():
    prop = SchSheetProperty(key="Sheetname", value="ChildA",
                            at_x=10.0, at_y=20.0, hide=True)
    assert sheet_property_to_op(prop) is None


def test_sheet_property_from_sexp_treats_effects_hide_as_hidden():
    prop = SchSheetProperty.from_sexp([
        "property",
        "Sheetfile",
        "child_a.kicad_sch",
        ["at", 1.0, 2.0, 0],
        ["effects", ["font", ["size", 1.27, 1.27]], ["hide", "yes"]],
    ])

    assert prop.hide is True
    assert prop.effects is not None
    assert prop.effects.hide is True
    assert sheet_property_to_op(prop) is None


def test_sheet_property_to_op_skips_empty_value():
    prop = SchSheetProperty(key="Sheetname", value="",
                            at_x=10.0, at_y=20.0)
    assert sheet_property_to_op(prop) is None


def test_sheet_property_to_op_emits_text_at_position():
    prop = SchSheetProperty(key="Sheetname", value="ChildA",
                            at_x=10.0, at_y=20.0, at_angle=90.0)
    op = sheet_property_to_op(prop)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "ChildA"
    assert op.payload["x"] == mm_to_nm(10.0)
    assert op.payload["y"] == mm_to_nm(20.0)
    assert op.payload["orient_deg"] == 90.0
    # Default-size fallback when effects is None
    assert op.payload["size_x_nm"] == mm_to_nm(DEFAULT_TEXT_SIZE_MM)


def test_sheet_property_to_op_prefixes_sheetfile_like_kicad():
    prop = SchSheetProperty(key="Sheetfile", value="child_a.kicad_sch")
    op = sheet_property_to_op(prop)
    assert op is not None
    assert op.payload["text"] == "File: child_a.kicad_sch"


def test_sheet_property_to_op_includes_shown_property_name():
    prop = SchSheetProperty(
        key="Supply",
        value="+5v0, +3v3",
        at_x=419.354,
        at_y=247.65,
        show_name=True,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27, thickness=0.254, bold=True)),
    )

    op = sheet_property_to_op(prop)

    assert op is not None
    assert op.payload["text"] == "Supply: +5v0, +3v3"
    assert op.payload["h_align"] == KiCadHorizAlign.CENTER.value
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value


def test_sheet_property_to_op_preserves_href_context():
    prop = SchSheetProperty(
        key="Sheetfile",
        value="child_a.kicad_sch",
        at_x=10.0,
        at_y=20.0,
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27),
            href="https://example.test/sheet",
        ),
    )

    op = sheet_property_to_op(prop)

    assert op is not None
    assert op.payload["context"]["hyperlink"]["href"] == "https://example.test/sheet"


def test_sheet_pin_to_op_emits_text_body_from_name():
    pin = SchSheetPin(name="VCC", at_x=10.0, at_y=20.0, at_angle=0.0)
    op = sheet_pin_to_op(pin)
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "VCC"
    assert op.payload["x"] == mm_to_nm(8.5395)
    assert op.payload["y"] == mm_to_nm(20.0)
    assert op.payload["orient_deg"] == 0.0
    assert op.payload["v_align"] == KiCadVertAlign.CENTER.value
    # Default-size fallback when effects is None.
    assert op.payload["size_x_nm"] == mm_to_nm(DEFAULT_TEXT_SIZE_MM)


def test_sheet_pin_to_op_unescapes_slash_for_display_text():
    pin = SchSheetPin(name="P0.01{slash}I2C_SCL", at_x=10.0, at_y=20.0, at_angle=0.0)
    op = sheet_pin_to_op(pin)
    assert op.payload["text"] == "P0.01/I2C_SCL"


def test_sheet_pin_to_op_uses_effects_size_when_present():
    pin = SchSheetPin(
        name="GND", at_x=0.0, at_y=0.0, at_angle=180.0,
        effects=Effects(font=Font(size_x=2.0, size_y=2.0)),
    )
    op = sheet_pin_to_op(pin)
    assert op.payload["size_x_nm"] == mm_to_nm(2.0)
    assert op.payload["x"] == mm_to_nm(2.3)
    assert op.payload["y"] == 0
    assert op.payload["orient_deg"] == 0.0


def test_sheet_pin_to_op_preserves_href_context():
    pin = SchSheetPin(
        name="GND",
        at_x=0.0,
        at_y=0.0,
        at_angle=180.0,
        effects=Effects(
            font=Font(size_x=2.0, size_y=2.0),
            href="https://example.test/pin",
        ),
    )

    op = sheet_pin_to_op(pin)

    assert op.payload["context"]["hyperlink"]["href"] == "https://example.test/pin"


def test_sheet_pin_to_op_uses_project_text_offset_ratio():
    pin = SchSheetPin(
        name="BLUE",
        at_x=382.27,
        at_y=262.89,
        at_angle=270.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    op = sheet_pin_to_op(pin, text_offset_ratio=0.08)
    assert op.payload["x"] == 382_270_000
    assert op.payload["y"] == 261_518_400
    assert op.payload["orient_deg"] == 90.0


def test_sheet_pin_to_op_uses_project_default_line_width_for_auto_thickness():
    pin = SchSheetPin(
        name="RED", at_x=0.0, at_y=0.0, at_angle=270.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27), justify=["left"]),
    )
    op = sheet_pin_to_op(pin, default_line_width_nm=254000)
    assert op.payload["pen_width_nm"] == 254000

    deco = sheet_pin_decoration_to_op(pin, default_line_width_nm=254000)
    assert deco is not None
    assert deco.payload["width_nm"] == 254000


def test_sheet_pin_to_op_uses_bus_color_for_bus_text():
    pin = SchSheetPin(
        name="PCIE_C{PCIE}", at_x=10.0, at_y=20.0, at_angle=180.0,
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    op = sheet_pin_to_op(pin, default_line_width_nm=254000)
    assert op.payload["color"] == LAYER_BUS


def test_sheet_record_emits_outline_pins_and_visible_properties_in_order():
    sh = SchSheet(at_x=100.0, at_y=200.0, size_x=80.0, size_y=60.0)
    sh.properties = [
        SchSheetProperty(key="Sheetname", value="ChildA",
                         at_x=100.0, at_y=199.0),
        SchSheetProperty(key="Sheetfile", value="child_a.kicad_sch",
                         at_x=100.0, at_y=261.0),
        # Hidden custom field — must be skipped.
        SchSheetProperty(key="Foo", value="bar",
                         at_x=110.0, at_y=210.0, hide=True),
    ]
    p1 = SchSheetPin(name="VCC", at_x=100.0, at_y=210.0)
    p2 = SchSheetPin(name="GND", at_x=180.0, at_y=210.0)
    sh.pins = [p1, p2]
    sh.fill_color = (255, 240, 220, 0.4)

    sch = _empty_schematic()
    sch.sheets = [sh]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    assert rec.kind == "sheet"

    # Order: background, outline, then per-pin (Text + decoration
    # PlotPoly), 2 visible props (Sheetname, Sheetfile; hidden Foo
    # skipped). F-6.9 added the per-sheet-pin triangle decoration.
    kinds = [op.kind for op in rec.operations]
    assert len(rec.operations) == 1 + 1 + (2 * 2) + 2
    assert kinds[0] == KiCadPlotterOpKind.RECT  # background
    assert rec.operations[0].payload["fill"] == "FILLED_SHAPE"
    assert kinds[1] == KiCadPlotterOpKind.RECT  # outline
    assert rec.operations[1].payload["fill"] == "NO_FILL"
    # Pin 1: Text body + triangle decoration
    assert kinds[2] == KiCadPlotterOpKind.TEXT
    assert rec.operations[2].payload["text"] == "VCC"
    assert kinds[3] == KiCadPlotterOpKind.PLOT_POLY
    assert rec.operations[3].payload["fill"] == "NO_FILL"
    # Pin 2: Text body + triangle decoration
    assert kinds[4] == KiCadPlotterOpKind.TEXT
    assert rec.operations[4].payload["text"] == "GND"
    assert kinds[5] == KiCadPlotterOpKind.PLOT_POLY
    # Visible properties
    assert rec.operations[6].payload["text"] == "ChildA"
    assert rec.operations[7].payload["text"] == "File: child_a.kicad_sch"


def test_sheet_record_with_no_pins_no_props_no_fill_emits_two_outlines():
    sh = SchSheet(at_x=10.0, at_y=20.0, size_x=30.0, size_y=40.0)
    sch = _empty_schematic()
    sch.sheets = [sh]
    doc = schematic_to_ir(sch)
    rec = doc.records[-1]
    assert len(rec.operations) == 2
    assert rec.operations[0].kind == KiCadPlotterOpKind.RECT
    assert rec.operations[0].payload["fill"] == "NO_FILL"
    assert rec.operations[1].kind == KiCadPlotterOpKind.RECT
    assert rec.operations[1].payload["fill"] == "NO_FILL"


# ---------------------------------------------------------------------------
# F-6.9: label & sheet-pin decoration shapes
# ---------------------------------------------------------------------------


def _half_size_default_nm() -> int:
    return mm_to_nm(DEFAULT_TEXT_SIZE_MM) // 2


@pytest.mark.parametrize(
    "angle, expected_spin",
    [
        (0.0, 2),     # SPIN_STYLE::RIGHT → HI template
        (90.0, 1),    # SPIN_STYLE::UP
        (180.0, 0),   # SPIN_STYLE::LEFT → HN template
        (270.0, 3),   # SPIN_STYLE::BOTTOM
        (360.0, 2),   # wraps to 0
        (-90.0, 3),   # wraps to 270 (BOTTOM)
        (45.0, 2),    # off-axis falls back to RIGHT (default)
    ],
)
def test_at_angle_to_spin_idx_maps_each_quarter(angle, expected_spin):
    from kicad_monkey.kicad_schematic_to_ir import _at_angle_to_spin_idx
    assert _at_angle_to_spin_idx(angle) == expected_spin


def test_hierarchical_label_decoration_input_left_spin_emits_hn_template():
    # at_angle=180 → SPIN_STYLE::LEFT → idx 0 → HN template:
    #   [(0,0), (-1,-1), (-2,-1), (-2,1), (-1,1), (0,0)]
    label = SchHierarchicalLabel(
        text="DATA", at_x=10.0, at_y=20.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    op = hierarchical_label_decoration_to_op(label)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["fill"] == "NO_FILL"
    h = _half_size_default_nm()
    ax, ay = mm_to_nm(10.0), mm_to_nm(20.0)
    expected = [
        [0 + ax, 0 + ay],
        [-h + ax, -h + ay],
        [-2 * h + ax, -h + ay],
        [-2 * h + ax, h + ay],
        [-h + ax, h + ay],
        [0 + ax, 0 + ay],
    ]
    assert op.payload["points"] == expected


def test_hierarchical_label_decoration_output_right_spin_emits_hi_template():
    # at_angle=0 → RIGHT → idx 2 → OUT_HI:
    #   [(2,0), (1,-1), (0,-1), (0,1), (1,1), (2,0)]
    label = SchHierarchicalLabel(
        text="OUT", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.OUTPUT,
    )
    op = hierarchical_label_decoration_to_op(label)
    h = _half_size_default_nm()
    expected = [
        [2 * h, 0],
        [h, -h],
        [0, -h],
        [0, h],
        [h, h],
        [2 * h, 0],
    ]
    assert op.payload["points"] == expected


def test_hierarchical_label_decoration_bidi_up_emits_5_point_template():
    # at_angle=90 → UP → idx 1 → BIDI_UP:
    #   [(0,0), (-1,-1), (0,-2), (1,-1), (0,0)]
    label = SchHierarchicalLabel(
        text="IO", at_x=0.0, at_y=0.0, at_angle=90.0,
        shape=LabelShape.BIDIRECTIONAL,
    )
    op = hierarchical_label_decoration_to_op(label)
    h = _half_size_default_nm()
    assert op.payload["points"] == [
        [0, 0],
        [-h, -h],
        [0, -2 * h],
        [h, -h],
        [0, 0],
    ]


def test_hierarchical_label_decoration_passive_uses_unspecified_template():
    # at_angle=180 → LEFT → idx 0 → UNSPC_HN:
    #   [(0,-1), (-2,-1), (-2,1), (0,1), (0,-1)]
    label = SchHierarchicalLabel(
        text="P", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.PASSIVE,
    )
    op = hierarchical_label_decoration_to_op(label)
    h = _half_size_default_nm()
    assert op.payload["points"] == [
        [0, -h],
        [-2 * h, -h],
        [-2 * h, h],
        [0, h],
        [0, -h],
    ]


def test_hierarchical_label_decoration_tristate_matches_bidi_template():
    bidi = SchHierarchicalLabel(
        text="A", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.BIDIRECTIONAL,
    )
    tri = SchHierarchicalLabel(
        text="A", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.TRI_STATE,
    )
    assert (
        hierarchical_label_decoration_to_op(bidi).payload["points"]
        == hierarchical_label_decoration_to_op(tri).payload["points"]
    )


@pytest.mark.parametrize("shape", [LabelShape.DOT, LabelShape.ROUND,
                                   LabelShape.DIAMOND, LabelShape.RECTANGLE])
def test_hierarchical_label_decoration_returns_none_for_directive_shapes(shape):
    # DOT/ROUND/DIAMOND/RECTANGLE are SCH_DIRECTIVE_LABEL shapes
    # without a TemplateShape entry.
    label = SchHierarchicalLabel(
        text="N", at_x=0.0, at_y=0.0, shape=shape,
    )
    assert hierarchical_label_decoration_to_op(label) is None


def test_hierarchical_label_decoration_uses_effects_text_height():
    # Effects.font.size_y=2.54mm → halfSize=1.27mm
    label = SchHierarchicalLabel(
        text="X", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.INPUT,
        effects=Effects(font=Font(size_x=2.54, size_y=2.54)),
    )
    op = hierarchical_label_decoration_to_op(label)
    h = mm_to_nm(2.54) // 2
    # IN_HI second point (1, 1) * h
    assert op.payload["points"][1] == [1 * h, 1 * h]


def test_hierarchical_label_decoration_pen_width_matches_default_wire():
    label = SchHierarchicalLabel(
        text="X", at_x=0.0, at_y=0.0, shape=LabelShape.INPUT,
    )
    op = hierarchical_label_decoration_to_op(label)
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)


def test_sheet_pin_decoration_swaps_input_to_output_template():
    # SCH_SHEET_PIN swaps INPUT↔OUTPUT before the TemplateShape lookup,
    # and maps edge angles to sheet-pin spin styles.
    pin = SchSheetPin(
        name="DIN", at_x=532.13, at_y=261.62, at_angle=180.0,
        shape=LabelShape.INPUT,
        effects=Effects(
            font=Font(size_x=1.27, size_y=1.27, thickness=0.254, bold=True),
            justify=["left"],
        ),
    )
    pin_op = sheet_pin_decoration_to_op(pin)
    assert pin_op.payload["points"] == [
        [533400000, 261620000],
        [532765000, 260985000],
        [532130000, 260985000],
        [532130000, 262255000],
        [532765000, 262255000],
        [533400000, 261620000],
    ]
    assert pin_op.payload["width_nm"] == 254000


def test_sheet_pin_decoration_swaps_output_to_input_template():
    pin = SchSheetPin(
        name="GND", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.OUTPUT,
    )
    pin_op = sheet_pin_decoration_to_op(pin)
    h = _half_size_default_nm()
    assert pin_op.payload["points"] == [
        [0, 0],
        [-h, -h],
        [-2 * h, -h],
        [-2 * h, h],
        [-h, h],
        [0, 0],
    ]


def test_sheet_pin_decoration_bidi_unaffected_by_swap():
    # Only INPUT/OUTPUT are swapped; BIDI/TRISTATE/PASSIVE pass through.
    pin = SchSheetPin(
        name="IO", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.BIDIRECTIONAL,
    )
    pin_op = sheet_pin_decoration_to_op(pin)
    h = _half_size_default_nm()
    assert pin_op.payload["points"] == [
        [0, 0],
        [-h, -h],
        [-2 * h, 0],
        [-h, h],
        [0, 0],
    ]


def test_sheet_pin_decoration_returns_none_for_directive_shape():
    pin = SchSheetPin(
        name="X", at_x=0.0, at_y=0.0, shape=LabelShape.ROUND,
    )
    assert sheet_pin_decoration_to_op(pin) is None


def test_hierarchical_label_record_appends_decoration_after_text():
    sch = _empty_schematic()
    sch.hierarchical_labels = [
        SchHierarchicalLabel(
            text="HL", at_x=10.0, at_y=20.0, at_angle=0.0,
            shape=LabelShape.INPUT,
        )
    ]
    doc = schematic_to_ir(sch)
    rec = next(r for r in doc.records if r.kind == "hierarchical_label")
    assert len(rec.operations) == 2
    assert rec.operations[0].kind == KiCadPlotterOpKind.TEXT
    assert rec.operations[0].payload["text"] == "HL"
    assert rec.operations[1].kind == KiCadPlotterOpKind.PLOT_POLY


def test_global_label_record_appends_decoration_after_text():
    # F-6.9b: global-label records now emit [Text, PlotPoly] (arrow
    # box) just like hier-labels emit [Text, PlotPoly] (triangle).
    sch = _empty_schematic()
    sch.global_labels = [
        SchGlobalLabel(
            text="GL", at_x=0.0, at_y=0.0, at_angle=0.0,
            shape=LabelShape.INPUT,
        )
    ]
    doc = schematic_to_ir(sch)
    rec = next(r for r in doc.records if r.kind == "global_label")
    assert len(rec.operations) == 2
    assert rec.operations[0].kind == KiCadPlotterOpKind.TEXT
    assert rec.operations[0].payload["text"] == "GL"
    assert rec.operations[1].kind == KiCadPlotterOpKind.PLOT_POLY
    assert rec.operations[1].payload["fill"] == "NO_FILL"


# ---------------------------------------------------------------------------
# F-6.9b: global-label arrow-box decorations
# ---------------------------------------------------------------------------


def test_schematic_outline_text_width_uses_kicad_outline_metrics_when_available():
    from pathlib import Path

    pytest.importorskip("freetype")
    pytest.importorskip("uharfbuzz")
    if not Path("C:/Windows/Fonts/arial.ttf").exists():
        pytest.skip("Windows Arial font is not available")

    from kicad_monkey.kicad_schematic_to_ir import _schematic_outline_text_width_nm

    width_nm = _schematic_outline_text_width_nm(
        "SMT Testpoint 75mil",
        mm_to_nm(1.8288),
    )
    assert abs(width_nm - mm_to_nm(23.0804)) <= 10_000


def _glabel_box_metrics_default(text: str):
    """Return ``(margin_nm, half_size_nm, line_width_nm, text_width_nm)``
    for a default-sized (no Effects override) global label with the
    given text. Mirrors the math in :func:`global_label_decoration_to_op`.
    """
    from kicad_monkey.kicad_schematic_to_ir import _schematic_outline_text_width_nm
    text_height_nm = mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    margin_nm = int(round(DEFAULT_LABEL_SIZE_RATIO * text_height_nm))
    half_size_nm = (text_height_nm // 2) + margin_nm
    line_width_nm = mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    text_width_nm = _schematic_outline_text_width_nm(
        text, mm_to_nm(DEFAULT_TEXT_SIZE_MM)
    )
    return margin_nm, half_size_nm, line_width_nm, text_width_nm


def test_global_label_decoration_input_left_spin_box_geometry():
    # at_angle=180 → SPIN_STYLE::LEFT (idx 0) → no rotation, anchor at right edge.
    text = "ABC"
    lbl = SchGlobalLabel(
        text=text, at_x=10.0, at_y=20.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    op = global_label_decoration_to_op(lbl)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["fill"] == "NO_FILL"
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    pts = op.payload["points"]
    # 6 outline points + 1 closing duplicate.
    assert len(pts) == 7
    assert pts[0] == pts[-1]

    margin, half, lw, tw = _glabel_box_metrics_default(text)
    symb_len = tw + 2 * margin
    x = symb_len + lw + 3
    y = half + lw + 3
    ax, ay = mm_to_nm(10.0), mm_to_nm(20.0)
    # INPUT, LEFT spin (no rotation), x_offset=-half, pts[0].x += half.
    expected = [
        [ax, ay],                 # apex (origin + half - half)
        [-half + ax, -y + ay],
        [-half - x + ax, -y + ay],
        [-half - x + ax, ay],
        [-half - x + ax, y + ay],
        [-half + ax, y + ay],
        [ax, ay],
    ]
    assert pts == expected


def test_global_label_decoration_output_right_spin_geometry():
    # at_angle=0 → SPIN_STYLE::RIGHT (idx 2) → 180° rotation.
    text = "OUT"
    lbl = SchGlobalLabel(
        text=text, at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.OUTPUT,
    )
    op = global_label_decoration_to_op(lbl)
    pts = op.payload["points"]
    assert len(pts) == 7

    margin, half, lw, tw = _glabel_box_metrics_default(text)
    symb_len = tw + 2 * margin
    x = symb_len + lw + 3
    y = half + lw + 3
    # OUTPUT shape: pts[3].x -= half (apex on left). x_offset = 0.
    raw = [
        (0, 0),
        (0, -y),
        (-x, -y),
        (-x - half, 0),
        (-x, y),
        (0, y),
    ]
    # RIGHT spin: rotate 180° → (-px, -py).
    expected = [[-px, -py] for px, py in raw]
    expected.append(expected[0])
    assert pts == expected


def test_global_label_decoration_bidi_up_spin_geometry():
    # at_angle=90 → SPIN_STYLE::UP (idx 1) → -90° rotation: (x,y) → (y, -x).
    text = "BD"
    lbl = SchGlobalLabel(
        text=text, at_x=0.0, at_y=0.0, at_angle=90.0,
        shape=LabelShape.BIDIRECTIONAL,
    )
    op = global_label_decoration_to_op(lbl)
    pts = op.payload["points"]

    margin, half, lw, tw = _glabel_box_metrics_default(text)
    symb_len = tw + 2 * margin
    x = symb_len + lw + 3
    y = half + lw + 3
    # BIDI: pts[0].x += half AND pts[3].x -= half. x_offset = -half.
    raw = [
        (0 + half, 0),
        (0, -y),
        (-x, -y),
        (-x - half, 0),
        (-x, y),
        (0, y),
    ]
    # Apply x_offset = -half.
    raw = [(px - half, py) for px, py in raw]
    # UP spin rotation: (px, py) → (-py, px).
    expected = [[-py, px] for px, py in raw]
    expected.append(expected[0])
    assert pts == expected


def test_global_label_decoration_passive_is_flat_box():
    # PASSIVE (UNSPECIFIED) shape: no apex, no x_offset; pure rectangle.
    text = "P"
    lbl = SchGlobalLabel(
        text=text, at_x=0.0, at_y=0.0, at_angle=180.0,  # LEFT (idx 0, no rotation)
        shape=LabelShape.PASSIVE,
    )
    op = global_label_decoration_to_op(lbl)
    pts = op.payload["points"]
    margin, half, lw, tw = _glabel_box_metrics_default(text)
    symb_len = tw + 2 * margin
    x = symb_len + lw + 3
    y = half + lw + 3
    expected = [
        [0, 0],
        [0, -y],
        [-x, -y],
        [-x, 0],
        [-x, y],
        [0, y],
        [0, 0],
    ]
    assert pts == expected


def test_global_label_decoration_tristate_matches_bidi_geometry():
    # TRI_STATE shares BIDI geometry verbatim (sch_label.cpp:2337-2342).
    bidi = SchGlobalLabel(
        text="X", at_x=10.0, at_y=20.0, at_angle=0.0,
        shape=LabelShape.BIDIRECTIONAL,
    )
    tri = SchGlobalLabel(
        text="X", at_x=10.0, at_y=20.0, at_angle=0.0,
        shape=LabelShape.TRI_STATE,
    )
    a = global_label_decoration_to_op(bidi)
    b = global_label_decoration_to_op(tri)
    assert a is not None and b is not None
    assert a.payload["points"] == b.payload["points"]


@pytest.mark.parametrize(
    "shape",
    [LabelShape.DOT, LabelShape.ROUND, LabelShape.DIAMOND, LabelShape.RECTANGLE],
)
def test_global_label_decoration_returns_none_for_directive_shapes(shape):
    lbl = SchGlobalLabel(
        text="X", at_x=0.0, at_y=0.0, at_angle=0.0, shape=shape,
    )
    assert global_label_decoration_to_op(lbl) is None


def test_global_label_decoration_uses_effects_text_height():
    # Override font size: size_y=2.54mm doubles the box vertical extent.
    lbl_default = SchGlobalLabel(
        text="GL", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    lbl_big = SchGlobalLabel(
        text="GL", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.INPUT,
        effects=Effects(font=Font(size_x=2.54, size_y=2.54)),
    )
    op_default = global_label_decoration_to_op(lbl_default)
    op_big = global_label_decoration_to_op(lbl_big)
    # The pts[1] y-coord (= -y = -(half + lw + 3)) doubles roughly with text
    # height (margin + half_size both scale with text height).
    y_default = abs(op_default.payload["points"][1][1])
    y_big = abs(op_big.payload["points"][1][1])
    # halfSize = (h/2) + 0.375*h = 0.875*h; y = halfSize + lw + 3.
    # Doubling h takes halfSize from 0.875*h to 1.75*h — roughly 2x.
    # Allow small tolerance for the +lw+3 nm constant.
    assert y_big > y_default
    # Strictly: (y_big - lw - 3) / (y_default - lw - 3) ≈ 2.0
    lw = mm_to_nm(DEFAULT_WIRE_WIDTH_MM)
    ratio = (y_big - lw - 3) / (y_default - lw - 3)
    assert abs(ratio - 2.0) < 0.001


def test_global_label_decoration_text_width_grows_with_text_length():
    short = SchGlobalLabel(
        text="A", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    long = SchGlobalLabel(
        text="ABCDEFGHIJ", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    op_s = global_label_decoration_to_op(short)
    op_l = global_label_decoration_to_op(long)
    # In LEFT spin (no rotation), pts[2].x = -half - x where x = symb_len + lw + 3.
    # symb_len grows with text width → |pts[2].x| grows.
    x_short = abs(op_s.payload["points"][2][0])
    x_long = abs(op_l.payload["points"][2][0])
    assert x_long > x_short


def test_global_label_decoration_uses_visible_width_for_overbar_markup():
    overbar = SchGlobalLabel(
        text="NANO_~{RESET}_0", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.BIDIRECTIONAL,
    )
    visible = SchGlobalLabel(
        text="NANO_RESET_0", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.BIDIRECTIONAL,
    )

    op_overbar = global_label_decoration_to_op(overbar)
    op_visible = global_label_decoration_to_op(visible)

    assert op_overbar.payload["points"] == op_visible.payload["points"]


def test_global_label_decoration_pen_width_matches_default_wire():
    lbl = SchGlobalLabel(
        text="X", at_x=0.0, at_y=0.0, at_angle=0.0,
        shape=LabelShape.INPUT,
    )
    op = global_label_decoration_to_op(lbl)
    assert op.payload["width_nm"] == mm_to_nm(DEFAULT_WIRE_WIDTH_MM)


def test_global_label_decoration_empty_text_still_emits_box():
    # Empty text: text_width = 0; the box still has margin and renders.
    lbl = SchGlobalLabel(
        text="", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    op = global_label_decoration_to_op(lbl)
    assert op is not None
    pts = op.payload["points"]
    assert len(pts) == 7
    assert pts[0] == pts[-1]


def test_global_label_decoration_anchor_offset_translates_polygon():
    a = SchGlobalLabel(
        text="X", at_x=0.0, at_y=0.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    b = SchGlobalLabel(
        text="X", at_x=10.0, at_y=20.0, at_angle=180.0,
        shape=LabelShape.INPUT,
    )
    pa = global_label_decoration_to_op(a).payload["points"]
    pb = global_label_decoration_to_op(b).payload["points"]
    dx, dy = mm_to_nm(10.0), mm_to_nm(20.0)
    for (ax, ay), (bx, by) in zip(pa, pb):
        assert bx - ax == dx
        assert by - ay == dy


def test_local_label_record_has_no_decoration():
    sch = _empty_schematic()
    sch.labels = [SchLabel(text="L", at_x=0.0, at_y=0.0)]
    doc = schematic_to_ir(sch)
    rec = next(r for r in doc.records if r.kind == "label")
    assert len(rec.operations) == 1
    assert rec.operations[0].kind == KiCadPlotterOpKind.TEXT

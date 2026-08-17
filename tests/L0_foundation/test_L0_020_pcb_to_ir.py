"""
Test L0_020: KiCadPcb → IR converter (Phase F-9)

Pure-unit coverage for the parser → IR boundary that turns a parsed
``KiCadPcb`` (mm, Y-down) into a ``KiCadPlotterDocument`` (nm,
Y-down). Mirrors the per-item record layout documented in
``kicad_pcb_to_ir``:

    gr_lines → gr_arcs → gr_circles → gr_rects → gr_polys →
    gr_curves → gr_texts → segments → track_arcs → vias →
    zones → footprints

Distinct from F-7 (`test_L0_017_footprint_to_ir.py`):
* Source is a full PCB document, not a standalone footprint
* PCB-embedded footprints carry ``placement = {x_nm,y_nm,angle_deg}``
  in extras (geometry stays footprint-local)
* Routing/zones carry ``net_id``/``net_name`` from ``NetRef.ordinal``
"""

from __future__ import annotations

import math
from collections import Counter

import pytest

from kicad_monkey import (
    KiCadFillType,
    KiCadProject,
    KiCadPlotterDocument,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadSvgRenderOptions,
    Net,
    gr_arc_to_op,
    gr_circle_to_op,
    gr_curve_to_op,
    gr_curve_to_record,
    gr_line_to_op,
    gr_line_to_record,
    gr_poly_to_op,
    gr_rect_to_op,
    gr_text_box_to_ops,
    gr_text_box_to_record,
    gr_text_to_op,
    gr_text_to_record,
    pcb_footprint_to_record,
    pcb_to_ir,
    render_ir_to_svg,
    track_arc_to_op,
    track_segment_to_op,
    track_segment_to_record,
    via_drill_to_op,
    via_to_op,
    via_to_record,
    zone_filled_polygon_to_op,
    zone_to_record,
)
from kicad_monkey.kicad_base import FillType, PadShape, Stroke as BaseStroke
from kicad_monkey.kicad_fp_line import FpLine
from kicad_monkey.kicad_fp_text import FpText
from kicad_monkey.kicad_fp_text_box import FpTextBox
from kicad_monkey.kicad_pad import Pad
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_pcb_footprint import Footprint
from kicad_monkey.kicad_pcb_gr_arc import GrArc
from kicad_monkey.kicad_pcb_gr_circle import GrCircle
from kicad_monkey.kicad_pcb_gr_curve import GrCurve
from kicad_monkey.kicad_pcb_gr_line import GrLine
from kicad_monkey.kicad_pcb_gr_poly import GrPoly
from kicad_monkey.kicad_pcb_gr_rect import GrRect
from kicad_monkey.kicad_pcb_gr_text import GrText
from kicad_monkey.kicad_pcb_graphics import GrTextBox
from kicad_monkey.kicad_pcb_routing import Arc as TrackArc
from kicad_monkey.kicad_pcb_routing import FrontBackOptBool, NetRef, Segment, Via
from kicad_monkey.kicad_pcb_zone import FilledPolygon, Zone
from kicad_monkey.kicad_primitives import Effects, Font, Stroke
from kicad_monkey.kicad_property import Property


# ---------------------------------------------------------------------------
# Board-level graphics: gr_line / gr_arc / gr_circle / gr_rect / gr_poly /
# gr_curve / gr_text
# ---------------------------------------------------------------------------


def _assert_bounds_tuple(bounds, expected: tuple[float, float, float, float]) -> None:
    assert bounds.is_valid()
    assert bounds.min_x == pytest.approx(expected[0], abs=1e-9)
    assert bounds.min_y == pytest.approx(expected[1], abs=1e-9)
    assert bounds.max_x == pytest.approx(expected[2], abs=1e-9)
    assert bounds.max_y == pytest.approx(expected[3], abs=1e-9)


@pytest.mark.parametrize(
    ("graphic", "expected"),
    [
        (
            GrLine(
                start_x=1.0,
                start_y=2.0,
                end_x=5.0,
                end_y=4.0,
                stroke=BaseStroke(width=0.2),
            ),
            (0.9, 1.9, 5.1, 4.1),
        ),
        (
            GrRect(
                start_x=5.0,
                start_y=1.0,
                end_x=2.0,
                end_y=4.0,
                stroke=BaseStroke(width=0.2),
            ),
            (1.9, 0.9, 5.1, 4.1),
        ),
        (
            GrCircle(
                center_x=10.0,
                center_y=-2.0,
                end_x=13.0,
                end_y=-2.0,
                stroke=BaseStroke(width=0.2),
            ),
            (6.9, -5.1, 13.1, 1.1),
        ),
        (
            GrArc(
                start_x=1.0,
                start_y=0.0,
                mid_x=math.sqrt(0.5),
                mid_y=math.sqrt(0.5),
                end_x=0.0,
                end_y=1.0,
                stroke=BaseStroke(width=0.2),
            ),
            (-0.1, -0.1, 1.1, 1.1),
        ),
        (
            GrPoly(
                points=[(0.0, 0.0), (2.0, 0.0), (1.0, 3.0)],
                stroke=BaseStroke(width=0.2),
            ),
            (-0.1, -0.1, 2.1, 3.1),
        ),
        (
            GrCurve(
                points=[(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0)],
                stroke=BaseStroke(width=0.2),
            ),
            (-0.1, -0.1, 4.1, 3.1),
        ),
    ],
)
def test_polygon_backed_board_graphics_get_bounds_match_kicad_shape_contract(
    graphic,
    expected,
):
    _assert_bounds_tuple(graphic.get_bounds(), expected)


@pytest.mark.parametrize(
    ("graphic", "expected"),
    [
        (
            GrLine(start_x=1.0, start_y=2.0, end_x=5.0, end_y=4.0),
            (1.0, 2.0, 5.0, 4.0),
        ),
        (
            GrArc(
                start_x=1.0,
                start_y=0.0,
                mid_x=math.sqrt(0.5),
                mid_y=math.sqrt(0.5),
                end_x=0.0,
                end_y=1.0,
            ),
            (0.0, 0.0, 1.0, 1.0),
        ),
        (
            GrCurve(points=[(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0)]),
            (0.0, 0.0, 4.0, 3.0),
        ),
    ],
)
def test_polygon_backed_zero_width_graphics_keep_source_geometry_bounds(
    graphic,
    expected,
):
    _assert_bounds_tuple(graphic.get_bounds(), expected)


def test_pcb_get_bounds_accepts_polygon_backed_board_graphics():
    pcb = KiCadPcb()
    pcb.gr_lines.append(
        GrLine(
            start_x=1.0, start_y=2.0, end_x=5.0, end_y=4.0,
            layer="F.SilkS", stroke=BaseStroke(width=0.2),
        )
    )
    pcb.gr_rects.append(
        GrRect(
            start_x=10.0, start_y=10.0, end_x=12.0, end_y=11.0,
            layer="B.SilkS", stroke=BaseStroke(width=0.2),
        )
    )

    bounds = pcb.get_bounds()

    _assert_bounds_tuple(bounds, (0.9, 1.9, 12.1, 11.1))


def test_pcb_get_bounds_layer_filter_accepts_polygon_backed_board_graphics():
    pcb = KiCadPcb()
    pcb.gr_lines.append(
        GrLine(
            start_x=1.0, start_y=2.0, end_x=5.0, end_y=4.0,
            layer="F.SilkS", stroke=BaseStroke(width=0.2),
        )
    )
    pcb.gr_rects.append(
        GrRect(
            start_x=10.0, start_y=10.0, end_x=12.0, end_y=11.0,
            layer="B.SilkS", stroke=BaseStroke(width=0.2),
        )
    )

    bounds = pcb.get_bounds(layers=["F.SilkS"])

    _assert_bounds_tuple(bounds, (0.9, 1.9, 5.1, 4.1))


def test_gr_line_to_op_no_y_flip():
    """PCB coords are Y-down; +Y in input → +Y in IR."""
    line = GrLine(
        start_x=1.0, start_y=2.0, end_x=3.0, end_y=4.0,
        layer="Edge.Cuts", stroke=Stroke(width=0.15),
    )
    op = gr_line_to_op(line)
    assert op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    assert op.payload == {
        "start_x": 1_000_000,
        "start_y": 2_000_000,
        "end_x": 3_000_000,
        "end_y": 4_000_000,
        "width_nm": 150_000,
    }


def test_gr_line_to_record_carries_layer():
    line = GrLine(
        start_x=0.0, start_y=0.0, end_x=1.0, end_y=0.0,
        layer="F.SilkS", stroke=Stroke(width=0.1), uuid="abc",
    )
    rec = gr_line_to_record(line)
    assert isinstance(rec, KiCadPlotterRecord)
    assert rec.kind == "gr_line"
    assert rec.uuid == "abc"
    assert rec.extras["layer"] == "F.SilkS"
    assert len(rec.operations) == 1


def test_gr_arc_to_op_emits_arc_three_point():
    arc = GrArc(
        start_x=0.0, start_y=0.0,
        mid_x=1.0, mid_y=1.0,
        end_x=2.0, end_y=0.0,
        layer="F.SilkS", stroke=Stroke(width=0.1),
    )
    op = gr_arc_to_op(arc)
    assert op.kind == KiCadPlotterOpKind.ARC_THREE_POINT
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value
    assert op.payload["width_nm"] == 100_000


def test_gr_circle_to_op_recovers_radius():
    # center=(0,0), end=(3,4) → radius=5 → diameter=10
    circle = GrCircle(
        center_x=0.0, center_y=0.0, end_x=3.0, end_y=4.0,
        layer="Edge.Cuts", fill=FillType.YES,
    )
    op = gr_circle_to_op(circle)
    assert op.kind == KiCadPlotterOpKind.CIRCLE
    assert op.payload["diameter_nm"] == 10_000_000
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value


def test_gr_rect_to_op_passes_through():
    rect = GrRect(
        start_x=1.0, start_y=2.0, end_x=3.0, end_y=4.0,
        layer="Edge.Cuts", fill=FillType.NO,
    )
    op = gr_rect_to_op(rect)
    assert op.kind == KiCadPlotterOpKind.RECT
    assert op.payload["x1"] == 1_000_000
    assert op.payload["y2"] == 4_000_000
    assert op.payload["fill"] == KiCadFillType.NO_FILL.value


def test_gr_poly_to_op_translates_points():
    poly = GrPoly(
        points=[(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)],
        layer="F.Cu", fill=FillType.YES,
    )
    op = gr_poly_to_op(poly)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    points = op.payload["points"]
    assert len(points) == 3
    assert tuple(points[0]) == (0, 0)
    assert tuple(points[1]) == (1_000_000, 0)
    assert tuple(points[2]) == (500_000, 1_000_000)
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value


def test_gr_curve_to_op_emits_bezier():
    curve = GrCurve(
        points=[(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0)],
        layer="F.SilkS", stroke=Stroke(width=0.1),
    )
    op = gr_curve_to_op(curve)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.BEZIER_CURVE
    assert op.payload["start_x"] == 0
    assert op.payload["end_x"] == 3_000_000


def test_gr_curve_to_op_returns_none_when_short():
    """Parser tolerates malformed curves; IR requires 4 control points."""
    curve = GrCurve(points=[(0.0, 0.0), (1.0, 1.0)], layer="F.SilkS")
    assert gr_curve_to_op(curve) is None


def test_gr_curve_to_record_handles_missing_op():
    curve = GrCurve(points=[(0.0, 0.0)], layer="F.SilkS")
    rec = gr_curve_to_record(curve)
    assert rec.operations == []
    assert rec.extras["layer"] == "F.SilkS"


def test_gr_text_to_op_emits_text():
    text = GrText(
        text="hello", at_x=5.0, at_y=10.0, at_angle=90.0,
        layer="F.SilkS", effects=Effects(),
    )
    op = gr_text_to_op(text)
    assert op is not None
    assert op.kind == KiCadPlotterOpKind.TEXT
    assert op.payload["text"] == "hello"
    assert op.payload["x"] == 5_000_000
    assert op.payload["y"] == 10_000_000
    assert op.payload["orient_deg"] == 90.0
    assert op.payload["h_align"] == "GR_TEXT_H_ALIGN_CENTER"
    assert op.payload["v_align"] == "GR_TEXT_V_ALIGN_CENTER"


def test_gr_text_to_op_skips_empty():
    text = GrText(text="", at_x=0.0, at_y=0.0, layer="F.SilkS")
    assert gr_text_to_op(text) is None


def test_gr_text_to_op_expands_project_text_variables():
    pcb = KiCadPcb()
    pcb.project = type(
        "ProjectStub",
        (),
        {"text_variables": {"SYNTHETIC_FIXTURE": "case101__text_stroke_variable"}},
    )()
    text = GrText(
        text="${SYNTHETIC_FIXTURE}",
        at_x=5.0,
        at_y=10.0,
        layer="F.SilkS",
        effects=Effects(),
    )

    op = gr_text_to_op(text, board=pcb)

    assert op is not None
    assert op.payload["text"] == "case101__text_stroke_variable"


def test_pcb_to_ir_board_text_attaches_generated_render_cache_polygons():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PART" "OK")
\t(gr_text "${PART}"
\t\t(at 10 10 0)
\t\t(layer "F.SilkS")
\t\t(effects
\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t(justify left top)
\t\t)
\t\t(render_cache "STALE" 0
\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    op = next(
        record.operations[0]
        for record in doc.records
        if record.kind == "gr_text" and record.operations
    )

    assert op.payload["text"] == "OK"
    assert op.payload["render_cache_source"] == "python_generated_cache"
    polygons = op.payload["render_cache_polygons"]
    assert polygons
    assert max(coord for polygon in polygons for point in polygon for coord in point) < 50_000_000


def test_pcb_to_ir_board_text_box_attaches_generated_render_cache_polygons():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PART" "OK")
\t(gr_text_box "${PART}"
\t\t(start 10 10)
\t\t(end 30 20)
\t\t(margins 0.5 0.5 0.5 0.5)
\t\t(layer "F.SilkS")
\t\t(effects
\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t(justify left top)
\t\t)
\t\t(border yes)
\t\t(stroke (width 0.15) (type solid))
\t\t(render_cache "STALE" 0
\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    text_ops = [
        op
        for record in doc.records
        if record.kind == "gr_text_box"
        for op in record.operations
        if op.kind == KiCadPlotterOpKind.TEXT
    ]

    assert text_ops
    op = text_ops[0]
    assert op.payload["text"] == "OK"
    assert op.payload["render_cache_source"] == "python_generated_cache"
    polygons = op.payload["render_cache_polygons"]
    assert polygons
    assert max(coord for polygon in polygons for point in polygon for coord in point) < 50_000_000


def test_pcb_to_ir_footprint_text_attaches_local_render_cache_polygons():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(footprint "Test:SvgCache"
\t\t(layer "F.Cu")
\t\t(at 10 10 0)
\t\t(property "Reference" "D1"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(hide yes)
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(property "UserText" "${Reference}-P"
\t\t\t(at 0 4 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t\t(fp_text reference "REF**"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t\t(fp_text_box "${Reference}"
\t\t\t(start 0 8)
\t\t\t(end 20 16)
\t\t\t(margins 0.5 0.5 0.5 0.5)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(border yes)
\t\t\t(stroke (width 0.15) (type solid))
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    footprint = next(record for record in doc.records if record.kind == "footprint")
    text_ops = [op for op in footprint.operations if op.kind == KiCadPlotterOpKind.TEXT]

    cached = [op for op in text_ops if op.payload.get("render_cache_polygons")]
    assert len(cached) == 3
    assert {op.payload["text"] for op in cached} == {"D1", "D1-P"}
    for op in cached:
        polygons = op.payload["render_cache_polygons"]
        assert op.payload["render_cache_source"] == "python_generated_cache"
        assert max(coord for polygon in polygons for point in polygon for coord in point) < 50_000_000


def test_pcb_to_ir_table_and_dimension_text_attach_render_cache_polygons():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external no) (header no))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 20)
\t\t(row_heights 10)
\t\t(cells
\t\t\t(table_cell "${ADDR}:T"
\t\t\t\t(start 10 10)
\t\t\t\t(end 30 20)
\t\t\t\t(margins 0.5 0.5 0.5 0.5)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "F.SilkS")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t\t(render_cache "STALE" 0
\t\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t\t)
\t\t\t)
\t\t)
\t)
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "OK")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t)
\t\t(gr_text "STALE"
\t\t\t(at 20 10 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    table_op = next(
        op
        for record in doc.records
        if record.kind == "table"
        for op in record.operations
        if op.kind == KiCadPlotterOpKind.TEXT
    )
    dimension_op = next(
        op
        for record in doc.records
        if record.kind == "dimension"
        for op in record.operations
        if op.kind == KiCadPlotterOpKind.TEXT
    )

    assert table_op.payload["text"] == "A1:T"
    assert dimension_op.payload["text"] == "OK mm"
    for op in [table_op, dimension_op]:
        assert op.payload["render_cache_source"] == "python_generated_cache"
        polygons = op.payload["render_cache_polygons"]
        assert polygons
        assert max(coord for polygon in polygons for point in polygon for coord in point) < 50_000_000


def test_pcb_to_ir_preserves_holed_render_cache_payload():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(gr_text "O"
\t\t(at 10 10 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (size 2 2) (thickness 0.2)))
\t\t(render_cache "O" 0
\t\t\t(polygon
\t\t\t\t(pts (xy 10 10) (xy 14 10) (xy 14 14) (xy 10 14))
\t\t\t\t(pts (xy 11 11) (xy 13 11) (xy 13 13) (xy 11 13))
\t\t\t)
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    op = next(record.operations[0] for record in doc.records if record.kind == "gr_text")

    assert op.payload["render_cache_source"] == "existing_file_cache"
    assert len(op.payload["render_cache_polygons"][0]) == 4
    typed_cache = op.payload["render_cache"]
    assert typed_cache["schema"] == "kicad.render_cache.v1"
    assert typed_cache["coordinate_space"] == "board"
    assert len(typed_cache["polygons"][0]["contours"]) == 2


def test_pcb_to_ir_table_cell_layer_visibility_uses_cell_layer():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user) (38 "B.SilkS" user))
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external yes))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 20)
\t\t(row_heights 10)
\t\t(cells
\t\t\t(table_cell "BACK"
\t\t\t\t(start 10 10)
\t\t\t\t(end 30 20)
\t\t\t\t(margins 0.5 0.5 0.5 0.5)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "B.SilkS")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    table_record = next(record for record in doc.records if record.kind == "table")

    assert table_record.extras["layers"] == ["B.SilkS", "F.SilkS"]
    svg = render_ir_to_svg(
        doc,
        options=KiCadSvgRenderOptions(visible_layers=("B.SilkS",)),
    )
    assert "<path" in svg
    assert ">BACK<" not in svg


def test_pcb_to_ir_table_grid_geometry_is_ir_not_direct_svg_only():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (37 "F.SilkS" user) (44 "Edge.Cuts" user))
\t(table
\t\t(column_count 2)
\t\t(layer "F.SilkS")
\t\t(border (external yes) (stroke (width 0.3) (type solid)))
\t\t(separators (rows yes) (cols yes) (stroke (width 0.1) (type solid)))
\t\t(column_widths 10 20)
\t\t(row_heights 5 7)
\t\t(cells
\t\t\t(table_cell "" (start 0 0) (end 10 5) (span 1 1) (layer "F.SilkS"))
\t\t\t(table_cell "" (start 10 0) (end 30 5) (span 1 1) (layer "F.SilkS"))
\t\t\t(table_cell "" (start 0 5) (end 10 12) (span 1 1) (layer "F.SilkS"))
\t\t\t(table_cell "" (start 10 5) (end 30 12) (span 1 1) (layer "F.SilkS"))
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    record = next(record for record in doc.records if record.kind == "table")
    segments = [
        op for op in record.operations
        if op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    ]

    assert len(segments) == 8
    assert Counter(op.payload["width_nm"] for op in segments) == Counter(
        {100_000: 4, 300_000: 4}
    )
    assert {op.payload["layer"] for op in segments} == {"F.SilkS"}
    assert {
        (
            op.payload["start_x"],
            op.payload["start_y"],
            op.payload["end_x"],
            op.payload["end_y"],
        )
        for op in segments
    } >= {
        (10_000_000, 0, 10_000_000, 5_000_000),
        (10_000_000, 5_000_000, 10_000_000, 12_000_000),
        (10_000_000, 5_000_000, 0, 5_000_000),
        (30_000_000, 5_000_000, 10_000_000, 5_000_000),
        (0, 0, 30_000_000, 0),
        (30_000_000, 0, 30_000_000, 12_000_000),
        (30_000_000, 12_000_000, 0, 12_000_000),
        (0, 12_000_000, 0, 0),
    }

    svg = render_ir_to_svg(
        doc,
        options=KiCadSvgRenderOptions(visible_layers=("F.SilkS",)),
    )
    assert svg.count("<polyline") >= 8


def test_pcb_to_ir_dimension_geometry_emits_lines_arrows_and_svg():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (37 "F.SilkS" user) (44 "Edge.Cuts" user))
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "10")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t)
\t)
)
""")

    doc = pcb_to_ir(pcb)
    record = next(record for record in doc.records if record.kind == "dimension")
    segments = [
        op for op in record.operations
        if op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    ]

    assert len(segments) == 7
    assert record.extras["layers"] == ["F.SilkS"]
    assert {
        (
            op.payload["start_x"],
            op.payload["start_y"],
            op.payload["end_x"],
            op.payload["end_y"],
        )
        for op in segments
    } >= {
        (0, 0, 0, 2_600_000),
        (10_000_000, 0, 10_000_000, 2_600_000),
        (0, 2_000_000, 10_000_000, 2_000_000),
    }

    svg = render_ir_to_svg(
        doc,
        options=KiCadSvgRenderOptions(visible_layers=("F.SilkS",)),
    )
    assert svg.count("<polyline") >= 7


def _dimension_segment_tuples(
    dimension_sexp: str,
) -> list[tuple[int, int, int, int, int]]:
    pcb = KiCadPcb.from_string(f"""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (41 "Cmts.User" user))
\t{dimension_sexp}
)
""")
    record = next(record for record in pcb_to_ir(pcb).records if record.kind == "dimension")
    return [
        (
            op.payload["start_x"],
            op.payload["start_y"],
            op.payload["end_x"],
            op.payload["end_y"],
            op.payload["width_nm"],
        )
        for op in record.operations
        if op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    ]


def test_pcb_to_ir_leader_dimension_connector_stops_at_text_box():
    segments = _dimension_segment_tuples("""(dimension
\t\t(type leader)
\t\t(layer "Cmts.User")
\t\t(pts (xy 8 8) (xy 20 16))
\t\t(format (units 3) (units_format 0) (precision 4) (override_value "NOTE"))
\t\t(style
\t\t\t(thickness 0.2)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(text_frame 0)
\t\t\t(extension_offset 0)
\t\t\t(keep_text_aligned yes)
\t\t)
\t\t(gr_text "NOTE"
\t\t\t(at 24 16 0)
\t\t\t(layer "Cmts.User")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t)""")

    assert segments[-1] == (
        20_000_000,
        16_000_000,
        21_494_048,
        16_000_000,
        200_000,
    )


def test_pcb_to_ir_leader_dimension_frame_uses_logical_text_box():
    segments = _dimension_segment_tuples("""(dimension
\t\t(type leader)
\t\t(layer "Cmts.User")
\t\t(pts (xy 8 8) (xy 20 16))
\t\t(format (units 3) (units_format 0) (precision 4) (override_value "A1"))
\t\t(style
\t\t\t(thickness 0.2)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(text_frame 1)
\t\t\t(extension_offset 0)
\t\t\t(keep_text_aligned yes)
\t\t)
\t\t(gr_text "A1"
\t\t\t(at 24 16 0)
\t\t\t(layer "Cmts.User")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t)""")

    assert segments[-5:] == [
        (22_470_238, 14_851_190, 22_470_238, 17_148_810, 200_000),
        (22_470_238, 17_148_810, 25_529_762, 17_148_810, 200_000),
        (25_529_762, 17_148_810, 25_529_762, 14_851_190, 200_000),
        (25_529_762, 14_851_190, 22_470_238, 14_851_190, 200_000),
        (20_000_000, 16_000_000, 22_470_238, 16_000_000, 200_000),
    ]


def test_pcb_to_ir_radial_dimension_connector_stops_at_text_box():
    segments = _dimension_segment_tuples("""(dimension
\t\t(type radial)
\t\t(layer "Cmts.User")
\t\t(pts (xy 15 15) (xy 22 15))
\t\t(leader_length 3)
\t\t(format (units 3) (units_format 1) (precision 4))
\t\t(style
\t\t\t(thickness 0.2)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(extension_offset 0)
\t\t\t(keep_text_aligned yes)
\t\t)
\t\t(gr_text "R7"
\t\t\t(at 27 15 0)
\t\t\t(layer "Cmts.User")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t)""")

    assert segments[-3] == (
        22_000_000,
        15_000_000,
        22_041_667,
        15_000_000,
        200_000,
    )


def test_gr_text_to_record_carries_text_and_hide_flag():
    text = GrText(text="REF", at_x=0.0, at_y=0.0, layer="F.SilkS")
    rec = gr_text_to_record(text)
    assert rec.extras["text"] == "REF"
    assert rec.extras["hide"] is False


def test_gr_text_box_to_ops_emits_border_and_text():
    box = GrTextBox(
        text="NOTE",
        start_x=0.0,
        start_y=0.0,
        end_x=5.0,
        end_y=2.0,
        layer="User.Comments",
        effects=Effects(font=Font(size_x=0.8, size_y=0.8), justify=["left", "top"]),
        stroke=Stroke(width=0.1),
        border=True,
    )

    ops = gr_text_box_to_ops(box)

    assert [op.kind for op in ops] == [
        KiCadPlotterOpKind.RECT,
        KiCadPlotterOpKind.TEXT,
    ]
    assert ops[1].payload["text"] == "NOTE"
    assert ops[1].payload["v_align"] == "GR_TEXT_V_ALIGN_TOP"
    assert ops[1].payload["text_as_polygons"] is True


def test_gr_text_box_to_record_carries_layer_and_text():
    box = GrTextBox(
        text="NOTE",
        start_x=0.0,
        start_y=0.0,
        end_x=5.0,
        end_y=2.0,
        layer="User.Comments",
    )

    rec = gr_text_box_to_record(box)

    assert rec.kind == "gr_text_box"
    assert rec.extras["layer"] == "User.Comments"
    assert rec.extras["text"] == "NOTE"


def test_gr_text_box_defaults_to_centered_text_anchor():
    box = GrTextBox(
        text="CENTER",
        start_x=4.0,
        start_y=3.5,
        end_x=16.0,
        end_y=8.5,
        margins=(1.0, 1.0, 1.0, 1.0),
        layer="F.SilkS",
        effects=Effects(font=Font(size_x=1.2, size_y=1.2)),
    )

    ops = gr_text_box_to_ops(box)

    assert ops[0].payload["x"] == 10_000_000
    assert ops[0].payload["y"] == 6_000_000
    assert ops[0].payload["h_align"] == "GR_TEXT_H_ALIGN_CENTER"
    assert ops[0].payload["v_align"] == "GR_TEXT_V_ALIGN_CENTER"


def test_gr_text_box_knockout_synthesizes_typed_render_cache():
    box = GrTextBox(
        text="KNOCKOUT",
        start_x=3.5,
        start_y=3.5,
        end_x=16.5,
        end_y=8.5,
        margins=(1.0, 1.0, 1.0, 1.0),
        layer="F.SilkS",
        effects=Effects(font=Font(size_x=1.2, size_y=1.2, thickness=0.18)),
        stroke=Stroke(width=0.15),
        border=True,
        knockout=True,
    )

    ops = gr_text_box_to_ops(box)
    text_op = next(op for op in ops if op.kind == KiCadPlotterOpKind.TEXT)

    assert text_op.payload["knockout"] is True
    cache = text_op.payload["render_cache"]
    assert cache["knockout"] is True
    contours = cache["polygons"][0]["contours"]
    assert len(contours) > 8
    assert all(len(contour) >= 3 for contour in contours)


# ---------------------------------------------------------------------------
# Routing: segment / track_arc / via
# ---------------------------------------------------------------------------


def test_track_segment_to_op_uses_track_width_directly():
    """Track widths are stored as ``width`` (mm), not via Stroke."""
    seg = Segment(
        start_x=0.0, start_y=0.0, end_x=10.0, end_y=0.0,
        width=0.25, layer="F.Cu",
    )
    op = track_segment_to_op(seg)
    assert op.kind == KiCadPlotterOpKind.THICK_SEGMENT
    assert op.payload["width_nm"] == 250_000


def test_track_segment_to_record_carries_net_extras():
    seg = Segment(
        start_x=0.0, start_y=0.0, end_x=1.0, end_y=0.0,
        width=0.2, layer="F.Cu", net=NetRef(ordinal=7, name="VCC"),
        uuid="seg1", locked=True,
    )
    rec = track_segment_to_record(seg)
    assert rec.kind == "segment"
    assert rec.extras["layer"] == "F.Cu"
    assert rec.extras["locked"] is True
    assert rec.extras["net_id"] == 7
    assert rec.extras["net_name"] == "VCC"


def test_track_arc_to_op_uses_track_width():
    arc = TrackArc(
        start_x=0.0, start_y=0.0,
        mid_x=1.0, mid_y=1.0,
        end_x=2.0, end_y=0.0,
        width=0.3, layer="B.Cu",
    )
    op = track_arc_to_op(arc)
    assert op.kind == KiCadPlotterOpKind.ARC_THREE_POINT
    assert op.payload["width_nm"] == 300_000
    assert op.payload["start_x"] == 2_000_000
    assert op.payload["start_y"] == 0
    assert op.payload["end_x"] == 0
    assert op.payload["end_y"] == 0


def test_via_to_op_emits_flash_pad_circle_with_full_size():
    via = Via(at_x=5.0, at_y=10.0, size=0.8, drill=0.4, layers=["F.Cu", "B.Cu"])
    op = via_to_op(via)
    assert op.kind == KiCadPlotterOpKind.FLASH_PAD_CIRCLE
    assert op.payload["x"] == 5_000_000
    assert op.payload["y"] == 10_000_000
    assert op.payload["diameter_nm"] == 800_000


def test_via_drill_to_op_emits_synthetic_hole():
    via = Via(at_x=5.0, at_y=10.0, size=0.8, drill=0.4, layers=["F.Cu", "B.Cu"])
    op = via_drill_to_op(via)
    assert op.kind == KiCadPlotterOpKind.CIRCLE
    assert op.payload["role"] == "via_drill"
    assert op.payload["layers"] == ["F.Cu", "B.Cu"]
    assert op.payload["cx"] == 5_000_000
    assert op.payload["cy"] == 10_000_000
    assert op.payload["diameter_nm"] == 400_000


def test_via_to_record_carries_drill_and_layers():
    via = Via(
        at_x=0.0, at_y=0.0, size=0.6, drill=0.3,
        layers=["F.Cu", "B.Cu"], net=NetRef(ordinal=2, name="GND"),
        via_type="micro",
    )
    rec = via_to_record(via)
    assert rec.extras["drill"] == 0.3
    assert rec.extras["size"] == 0.6
    assert rec.extras["layers"] == ["F.Cu", "B.Cu"]
    assert rec.extras["via_type"] == "micro"
    assert rec.extras["hole_kind"] == "round"
    assert rec.extras["hole_plating"] == "plated"
    assert rec.extras["hole_render"] == "drill"
    assert rec.extras["net_id"] == 2
    assert rec.extras["net_name"] == "GND"
    assert [op.kind for op in rec.operations] == [
        KiCadPlotterOpKind.FLASH_PAD_CIRCLE,
        KiCadPlotterOpKind.CIRCLE,
    ]
    assert rec.operations[1].payload["role"] == "via_drill"


def test_via_to_record_default_via_type_is_through():
    via = Via(at_x=0.0, at_y=0.0, size=0.6, drill=0.3, layers=["F.Cu"])
    rec = via_to_record(via)
    assert rec.extras["via_type"] == "through"


def test_via_to_record_carries_ipc4761_metadata():
    via = Via(
        at_x=0.0,
        at_y=0.0,
        size=0.3,
        drill=0.15,
        layers=["F.Cu", "B.Cu"],
        tenting=FrontBackOptBool(front=True, back=False),
        covering=FrontBackOptBool(front=False, back=True),
        plugging=FrontBackOptBool(front=False, back=False),
        capping=True,
        filling=True,
    )

    rec = via_to_record(via)

    assert rec.extras["ipc4761_metadata"] == "true"
    assert rec.extras["ipc4761_tenting_front"] == "true"
    assert rec.extras["ipc4761_tenting_back"] == "false"
    assert rec.extras["ipc4761_covering_front"] == "false"
    assert rec.extras["ipc4761_covering_back"] == "true"
    assert rec.extras["ipc4761_plugging_front"] == "false"
    assert rec.extras["ipc4761_plugging_back"] == "false"
    assert rec.extras["ipc4761_capping"] == "true"
    assert rec.extras["ipc4761_filling"] == "true"


# ---------------------------------------------------------------------------
# Zones: filled_polygon → PlotPoly (FILLED_SHAPE)
# ---------------------------------------------------------------------------


def test_zone_filled_polygon_to_op_emits_filled_poly():
    fpoly = FilledPolygon(
        layer="F.Cu",
        points=[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    )
    op = zone_filled_polygon_to_op(fpoly)
    assert op.kind == KiCadPlotterOpKind.PLOT_POLY
    assert op.payload["fill"] == KiCadFillType.FILLED_SHAPE.value
    assert op.payload["width_nm"] == 0
    assert len(op.payload["points"]) == 4


def test_zone_to_record_bundles_all_filled_polygons():
    zone = Zone(
        net=NetRef(ordinal=1, name="GND"),
        layers=["F.Cu", "B.Cu"],
        filled_polygons=[
            FilledPolygon(layer="F.Cu", points=[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]),
            FilledPolygon(
                layer="B.Cu", island=True,
                points=[(2.0, 2.0), (3.0, 2.0), (3.0, 3.0)],
            ),
        ],
    )
    rec = zone_to_record(zone)
    assert rec.kind == "zone_fill"
    assert len(rec.operations) == 2
    assert rec.extras["layers"] == ["F.Cu", "B.Cu"]
    assert rec.extras["fill_layers"] == ["F.Cu", "B.Cu"]
    assert rec.extras["fill_island"] == [False, True]
    assert rec.extras["net_id"] == 1
    assert rec.extras["net_name"] == "GND"


def test_zone_to_record_no_net():
    """Zones without a NetRef should still produce a record (no net keys)."""
    zone = Zone(layers=["F.Cu"])
    rec = zone_to_record(zone)
    assert "net_id" not in rec.extras
    assert "net_name" not in rec.extras


# ---------------------------------------------------------------------------
# Footprints: pcb_footprint_to_record
# ---------------------------------------------------------------------------


def test_pcb_footprint_to_record_uses_library_link_as_object_id():
    fp = Footprint(library_link="lib:R_0603", at_x=10.0, at_y=20.0, at_angle=90.0)
    rec = pcb_footprint_to_record(fp)
    assert rec.kind == "footprint"
    assert rec.object_id == "lib:R_0603"
    assert rec.extras["library_link"] == "lib:R_0603"


def test_pcb_footprint_to_record_carries_placement():
    fp = Footprint(library_link="lib:R_0603", at_x=10.0, at_y=20.0, at_angle=90.0)
    rec = pcb_footprint_to_record(fp)
    placement = rec.extras["placement"]
    assert placement == {
        "x_nm": 10_000_000,
        "y_nm": 20_000_000,
        "angle_deg": 90.0,
    }


def test_pcb_footprint_to_record_emits_pad_op():
    """Pads dispatch onto the FlashPad* op family (in fp-local coords)."""
    pad = Pad(number="1", pad_type="smd", shape=PadShape.CIRCLE,
              at_x=0.5, at_y=0.0, size_x=0.6, size_y=0.6)
    fp = Footprint(library_link="lib:R", pads=[pad])
    rec = pcb_footprint_to_record(fp)
    assert [op.kind for op in rec.operations] == [
        KiCadPlotterOpKind.START_BLOCK,
        KiCadPlotterOpKind.FLASH_PAD_CIRCLE,
        KiCadPlotterOpKind.END_BLOCK,
    ]
    assert rec.operations[0].payload["data_ref"] == "pad"
    assert rec.operations[0].payload["extra_attrs"]["primitive"] == "pad"
    assert rec.operations[0].payload["extra_attrs"]["pad_number"] == "1"
    # Pad coords are footprint-local (no placement applied).
    assert rec.operations[1].payload["x"] == 500_000


def test_pcb_footprint_to_record_tags_child_ops_with_layers():
    """PCB footprint child ops retain their own graphics/pad layers."""
    line = FpLine(
        start_x=0.0,
        start_y=0.0,
        end_x=1.0,
        end_y=0.0,
        layer="B.SilkS",
    )
    pad = Pad(
        number="1",
        pad_type="smd",
        shape=PadShape.CIRCLE,
        at_x=0.5,
        at_y=0.0,
        size_x=0.6,
        size_y=0.6,
        layers=["*.Cu", "*.Mask"],
    )
    fp = Footprint(library_link="lib:R", fp_lines=[line], pads=[pad])

    rec = pcb_footprint_to_record(fp)

    assert rec.operations[0].payload["layer"] == "B.SilkS"
    assert rec.operations[1].kind == KiCadPlotterOpKind.START_BLOCK
    assert rec.operations[2].payload["layers"] == ["*.Cu", "*.Mask"]


def test_pcb_footprint_to_record_orders_reference_then_value_then_others():
    """Property order mirrors F-7's footprint_to_record."""
    fp = Footprint(
        library_link="lib:R",
        properties=[
            Property(name="Datasheet", value="ds.pdf", at_x=0.0, at_y=0.0),
            Property(name="Value", value="1k", at_x=0.0, at_y=1.0),
            Property(name="Reference", value="R1", at_x=0.0, at_y=2.0),
        ],
    )
    rec = pcb_footprint_to_record(fp)
    texts = [op.payload["text"] for op in rec.operations]
    assert texts == ["R1", "1k", "ds.pdf"]


def test_pcb_footprint_to_record_emits_fp_text_box_layer_and_variables():
    box = FpTextBox(
        text="${REFERENCE}",
        start_x=0.0,
        start_y=0.0,
        end_x=4.0,
        end_y=2.0,
        layer="User.4",
        effects=Effects(font=Font(size_x=0.5, size_y=0.5), justify=["left", "top"]),
        stroke=Stroke(width=0.1),
        border=False,
    )
    fp = Footprint(
        library_link="lib:J",
        properties=[Property(name="Reference", value="J1", hide=True)],
        fp_text_boxes=[box],
    )

    rec = pcb_footprint_to_record(fp)

    assert len(rec.operations) == 1
    assert rec.operations[0].kind == KiCadPlotterOpKind.TEXT
    assert rec.operations[0].payload["text"] == "J1"
    assert rec.operations[0].payload["layer"] == "User.4"


def test_pcb_footprint_to_record_skips_metadata_only_properties():
    fp = Footprint(
        library_link="lib:R",
        properties=[
            Property(
                name="ki_fp_filters",
                value="wavenumber:R0402_0.40MM_HD",
                graphical=False,
            ),
        ],
    )

    rec = pcb_footprint_to_record(fp)

    assert rec.operations == []


def test_pcb_footprint_to_record_resolves_fp_text_and_tags_child_metadata():
    fp = Footprint(
        library_link="lib:J",
        uuid="fp-uuid",
        properties=[Property(name="Reference", value="J1", hide=True)],
        fp_texts=[
            FpText(
                text_type="user",
                text="${REFERENCE}",
                at_x=0.0,
                at_y=0.0,
                layer="F.SilkS",
                uuid="text-uuid",
            )
        ],
    )

    rec = pcb_footprint_to_record(fp)

    assert len(rec.operations) == 1
    payload = rec.operations[0].payload
    assert payload["text"] == "J1"
    assert payload["data_ref"] == "fp_text"
    assert payload["data_uuid"] == "text-uuid"
    assert payload["extra_attrs"]["primitive"] == "footprint-text"
    assert payload["extra_attrs"]["footprint_text_role"] == "user"
    assert payload["extra_attrs"]["component"] == "J1"
    assert payload["extra_attrs"]["layer_name"] == "F.SilkS"


# ---------------------------------------------------------------------------
# Top-level pcb_to_ir
# ---------------------------------------------------------------------------


def _empty_pcb(**kwargs) -> KiCadPcb:
    """Minimal KiCadPcb with optional collection overrides."""
    pcb = KiCadPcb()
    pcb.version = 20240101
    pcb.generator = "pytest"
    pcb.generator_version = "0"
    for key, value in kwargs.items():
        setattr(pcb, key, value)
    return pcb


def test_pcb_to_ir_empty_board():
    pcb = _empty_pcb()
    doc = pcb_to_ir(pcb, source_path="/tmp/empty.kicad_pcb")
    assert isinstance(doc, KiCadPlotterDocument)
    assert doc.records == []
    assert doc.source_kind == "PCB"
    assert doc.coordinate_space == {"unit": "nm", "y_axis": "down"}
    assert doc.extras["version"] == 20240101


def test_pcb_to_ir_builds_net_and_netclass_snapshots_once(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import importlib

    converter = importlib.import_module("kicad_monkey.kicad_pcb_to_ir")
    calls = {"net_table": 0, "net_classes": 0}
    original_net_table = KiCadPcb.net_table
    original_net_classes = converter.project_net_name_to_classes

    def counted_net_table(board: KiCadPcb):
        calls["net_table"] += 1
        return original_net_table(board)

    def counted_net_classes(board: KiCadPcb):
        calls["net_classes"] += 1
        return original_net_classes(board)

    monkeypatch.setattr(KiCadPcb, "net_table", counted_net_table)
    monkeypatch.setattr(converter, "project_net_name_to_classes", counted_net_classes)

    pad = Pad(
        number="1",
        pad_type="smd",
        shape=PadShape.CIRCLE,
        at_x=0.0,
        at_y=0.0,
        net=NetRef(ordinal=1),
        size_x=1.0,
        size_y=1.0,
    )
    footprint = Footprint(library_link="Test:R", pads=[pad])
    pcb = _empty_pcb(
        nets=[Net(ordinal=0, name=""), Net(ordinal=1, name="GND")],
        segments=[
            Segment(
                start_x=0.0,
                start_y=0.0,
                end_x=1.0,
                end_y=0.0,
                width=0.2,
                layer="F.Cu",
                net=NetRef(ordinal=1, name="GND"),
            )
        ],
        footprints=[footprint],
    )
    pcb.project = KiCadProject.from_json_dict(
        {
            "net_settings": {
                "netclass_assignments": {"GND": ["Power", "HighCurrent"]}
            }
        }
    )

    document = pcb_to_ir(pcb)

    assert calls == {"net_table": 1, "net_classes": 1}
    segment = next(record for record in document.records if record.kind == "segment")
    assert segment.extras["net_class"] == "Power"
    assert segment.extras["net_classes"] == ["Power", "HighCurrent"]
    assert segment.extras["layer"] == "F.Cu"
    footprint_record = next(record for record in document.records if record.kind == "footprint")
    pad_block = next(
        operation
        for operation in footprint_record.operations
        if operation.kind == KiCadPlotterOpKind.START_BLOCK
    )
    assert pad_block.payload["extra_attrs"]["net"] == "GND"
    assert pad_block.payload["extra_attrs"]["net_class"] == "Power"


def test_pcb_to_ir_omits_netclass_metadata_when_project_settings_are_missing() -> None:
    pcb = _empty_pcb(
        segments=[
            Segment(
                start_x=0.0,
                start_y=0.0,
                end_x=1.0,
                end_y=0.0,
                width=0.2,
                layer="F.Cu",
                net=NetRef(ordinal=1, name="GND"),
            )
        ]
    )

    segment = pcb_to_ir(pcb).records[0]

    assert segment.extras["net_name"] == "GND"
    assert "net_class" not in segment.extras
    assert "net_classes" not in segment.extras


def test_pcb_to_ir_emits_records_in_canonical_order():
    """Per-category order: gr_lines → segments → vias → footprints."""
    pcb = _empty_pcb(
        gr_lines=[GrLine(start_x=0.0, start_y=0.0, end_x=1.0, end_y=0.0)],
        segments=[
            Segment(start_x=0.0, start_y=0.0, end_x=2.0, end_y=0.0,
                    width=0.2, layer="F.Cu"),
        ],
        vias=[Via(at_x=0.0, at_y=0.0, size=0.6, drill=0.3, layers=["F.Cu"])],
        footprints=[Footprint(library_link="lib:R")],
    )
    doc = pcb_to_ir(pcb)
    kinds = [r.kind for r in doc.records]
    assert kinds == ["gr_line", "segment", "via", "footprint"]


def test_pcb_to_ir_carries_paper_and_thickness():
    pcb = _empty_pcb()
    pcb.thickness = 1.6
    pcb.paper = "A4"
    doc = pcb_to_ir(pcb)
    assert doc.extras["thickness_mm"] == 1.6
    assert doc.extras["paper"] == "A4"

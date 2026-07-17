"""
Test L0_009: KiCad Plotter-IR → SVG renderer (Phase F-5)

Covers ``render_op`` op-kind dispatch, ``render_record`` group
wrapping, and the ``render_ir_to_svg`` document envelope. Also runs
two end-to-end smoke checks that thread an F-3 lib-symbol IR and an
F-4 schematic IR through the F-5 renderer.
"""

from __future__ import annotations

import xml.etree.ElementTree as ET

import pytest

from kicad_monkey import (
    KiCadFillType,
    KiCadLineStyle,
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterOpKind,
    KiCadPlotterRecord,
    KiCadSchematic,
    KiCadSvgRenderContext,
    KiCadSvgRenderOptions,
    LibSymbol,
    SchJunction,
    SchWire,
    lib_symbol_to_ir,
    render_ir_to_svg,
    render_op,
    render_record,
    render_records,
    schematic_to_ir,
    styled_plotter_op,
)
from kicad_monkey.kicad_primitives import Stroke


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def ctx() -> KiCadSvgRenderContext:
    """A4-landscape default context."""
    return KiCadSvgRenderContext(
        sheet_width_nm=297_000_000,
        sheet_height_nm=210_000_000,
    )


# ---------------------------------------------------------------------------
# render_op — primitive dispatch
# ---------------------------------------------------------------------------


def test_render_op_circle_unfilled(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.circle(
        cx=10_000_000, cy=20_000_000, diameter_nm=4_000_000,
        fill=KiCadFillType.NO_FILL, width_nm=152_400,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<circle")
    assert 'cx="10"' in svg and 'cy="20"' in svg
    # diameter 4mm -> radius 2mm (integer //2)
    assert 'r="2"' in svg
    assert 'fill="none"' in svg


def test_render_op_circle_filled(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.circle(
        cx=5_000_000, cy=5_000_000, diameter_nm=2_000_000,
        fill=KiCadFillType.FILLED_SHAPE, width_nm=0,
    )
    svg = render_op(op, ctx=ctx)
    assert 'fill="#000000"' in svg
    assert 'stroke-width="0"' in svg


def test_render_op_uses_declarative_fill_color(ctx: KiCadSvgRenderContext) -> None:
    op = styled_plotter_op(
        KiCadPlotterOp.rect(
            x1=0, y1=0, x2=10_000_000, y2=5_000_000,
            fill=KiCadFillType.FILLED_SHAPE, width_nm=0,
        ),
        fill_color="#FFC86480",
        stroke_color="#FFC86480",
    )
    svg = render_op(op, ctx=ctx)
    assert 'fill="#FFC86480"' in svg
    assert 'stroke="#FFC86480"' in svg


def test_render_op_uses_declarative_line_style(ctx: KiCadSvgRenderContext) -> None:
    op = styled_plotter_op(
        KiCadPlotterOp.plot_poly(
            points=[(0, 0), (10_000_000, 0)],
            fill=KiCadFillType.NO_FILL,
            width_nm=254_000,
        ),
        stroke_color="#0A141EFF",
        line_style=KiCadLineStyle.DASH_DOT,
    )
    svg = render_op(op, ctx=ctx)
    assert 'stroke="#0A141EFF"' in svg
    assert "stroke-dasharray=" in svg


def test_render_op_filled_zero_width_is_fill_only(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.rect(
        x1=0, y1=0, x2=1_000_000, y2=1_000_000,
        fill=KiCadFillType.FILLED_WITH_BG_BODYCOLOR, width_nm=0,
    )
    svg = render_op(op, ctx=ctx)
    assert f'fill="{ctx.sheet_area_color}"' in svg
    assert 'stroke-width="0"' in svg


def test_render_op_no_fill_zero_width_uses_default_pen(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.rect(
        x1=0, y1=0, x2=1_000_000, y2=1_000_000,
        fill=KiCadFillType.NO_FILL, width_nm=0,
    )
    svg = render_op(op, ctx=ctx)
    assert 'fill="none"' in svg
    assert 'stroke-width="0.1524"' in svg


def test_render_op_circle_diameter_floor_division(ctx: KiCadSvgRenderContext) -> None:
    # Odd-nm diameter — integer //2 should floor.
    op = KiCadPlotterOp.circle(
        cx=0, cy=0, diameter_nm=3, fill=KiCadFillType.NO_FILL, width_nm=0,
    )
    # Use unitless output so we can read the radius back as nm.
    bare_ctx = KiCadSvgRenderContext(
        sheet_width_nm=10, sheet_height_nm=10,
        options=KiCadSvgRenderOptions(output_unit_per_nm=1.0, output_unit_suffix=""),
    )
    svg = render_op(op, ctx=bare_ctx)
    assert 'r="1"' in svg


def test_render_op_rect(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.rect(
        x1=10_000_000, y1=20_000_000, x2=40_000_000, y2=60_000_000,
        fill=KiCadFillType.NO_FILL, width_nm=152_400, corner_radius_nm=0,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<rect")
    assert 'x="10"' in svg and 'y="20"' in svg
    assert 'width="30"' in svg and 'height="40"' in svg


def test_render_op_arc_three_point(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.arc_three_point(
        start_x=0, start_y=0,
        mid_x=5_000_000, mid_y=5_000_000,
        end_x=10_000_000, end_y=0,
        width_nm=152_400,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<path")
    assert "A " in svg  # SVG arc command


def test_render_op_arc_center_angle(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.arc_center_angle(
        cx=10_000_000,
        cy=10_000_000,
        start_angle_deg=0.0,
        sweep_deg=90.0,
        radius_nm=5_000_000,
        width_nm=152_400,
    )

    svg = render_op(op, ctx=ctx)

    assert svg.startswith("<path")
    assert "A " in svg
    assert "M 15 10" in svg
    assert "10 15" in svg


def test_render_op_bezier(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.bezier_curve(
        start_x=0, start_y=0,
        ctrl1_x=5_000_000, ctrl1_y=10_000_000,
        ctrl2_x=15_000_000, ctrl2_y=10_000_000,
        end_x=20_000_000, end_y=0,
        width_nm=152_400,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<path")
    assert "C " in svg


def test_render_op_plot_poly_filled(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.plot_poly(
        points=[(0, 0), (10_000_000, 0), (5_000_000, 10_000_000)],
        fill=KiCadFillType.FILLED_SHAPE,
        width_nm=152_400,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<polygon")
    assert "0,0" in svg
    assert 'fill="#000000"' in svg


def test_render_op_plot_poly_unfilled(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.plot_poly(
        points=[(0, 0), (10_000_000, 10_000_000)],
        fill=KiCadFillType.NO_FILL,
        width_nm=152_400,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<polyline")
    assert 'fill="none"' in svg


def test_render_op_text_single_line(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.text(
        x=10_000_000, y=20_000_000, text="hello",
        size_x_nm=1_270_000, size_y_nm=1_270_000,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.startswith("<text")
    assert ">hello</text>" in svg


def test_render_op_text_multiline_stacks(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.text(
        x=10_000_000, y=20_000_000, text="one\ntwo\nthree",
        size_x_nm=1_270_000, size_y_nm=1_270_000,
        multiline=True,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.count("<text") == 3
    assert ">one</text>" in svg
    assert ">two</text>" in svg
    assert ">three</text>" in svg


def test_render_op_text_multiline_rotated_stacks_in_kicad_direction(
    ctx: KiCadSvgRenderContext,
) -> None:
    op = KiCadPlotterOp.text(
        x=39_047_829,
        y=54_614_000,
        text="Motor\nSupply",
        size_x_nm=2_540_000,
        size_y_nm=2_540_000,
        orient_deg=90.0,
        h_align="GR_TEXT_H_ALIGN_CENTER",
        v_align="GR_TEXT_V_ALIGN_CENTER",
        multiline=True,
    )
    svg = render_op(op, ctx=ctx)
    assert 'x="36.914229"' in svg
    assert 'transform="rotate(-90 36.914229 54.614)"' in svg
    assert 'x="41.181429"' in svg
    assert 'transform="rotate(-90 41.181429 54.614)"' in svg


def test_render_op_text_multiline_without_newline_is_single(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.text(
        x=0, y=0, text="solo",
        size_x_nm=1_270_000, size_y_nm=1_270_000,
        multiline=True,
    )
    svg = render_op(op, ctx=ctx)
    assert svg.count("<text") == 1


def test_render_op_plot_image_embeds_data_uri(ctx: KiCadSvgRenderContext) -> None:
    # KiCad's PlotImage position is the image center, not the top-left corner.
    op = KiCadPlotterOp.plot_image(
        x=1_000_000,
        y=2_000_000,
        width_nm=3_000_000,
        height_nm=4_000_000,
        scale=1.0,
        image_data_b64="iVBORw0KGgo=",
        image_format="png",
    )

    svg = render_op(op, ctx=ctx)

    assert svg.startswith("<image")
    assert 'x="-0.5"' in svg
    assert 'y="0"' in svg
    assert 'width="3"' in svg
    assert 'height="4"' in svg
    assert 'href="data:image/png;base64,iVBORw0KGgo="' in svg


def test_render_op_plot_image_uses_bmp_mime(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.plot_image(
        x=0,
        y=0,
        width_nm=1_000_000,
        height_nm=1_000_000,
        image_data_b64="Qk0=",
        image_format="bmp",
    )

    svg = render_op(op, ctx=ctx)

    assert 'href="data:image/bmp;base64,Qk0="' in svg


def test_render_op_unsupported_kind_returns_empty(ctx: KiCadSvgRenderContext) -> None:
    """Forward-compat: an unknown / future op kind should silently no-op."""
    op = KiCadPlotterOp(kind="SomeFutureRecorderOp", payload={})
    assert render_op(op, ctx=ctx) == ""


def test_render_op_state_op_returns_empty(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.set_color(color="#FF0000")
    assert render_op(op, ctx=ctx) == ""


# ---------------------------------------------------------------------------
# render_record
# ---------------------------------------------------------------------------


def test_render_record_wraps_in_group(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)
    rec = KiCadPlotterRecord(uuid="ABC-123", kind="wire", object_id="w1", operations=[op])
    out = render_record(rec, ctx=ctx)
    assert out.startswith("<g")
    assert 'id="ABC-123"' in out
    assert 'data-uuid="ABC-123"' in out
    assert 'data-ref="wire"' in out
    assert "<circle" in out
    assert out.rstrip().endswith("</g>")


def test_render_record_empty_ops_returns_placeholder_group(ctx: KiCadSvgRenderContext) -> None:
    rec = KiCadPlotterRecord(uuid="HEADER", kind="sheet_header", object_id="sheet", operations=[])
    out = render_record(rec, ctx=ctx)
    assert out.startswith("<g")
    assert 'id="HEADER"' in out
    assert "<circle" not in out


def test_render_record_materializes_nested_block_group(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)
    rec = KiCadPlotterRecord(
        uuid="symbol-uuid",
        kind="symbol_instance",
        object_id="Device:R",
        operations=[
            KiCadPlotterOp.start_block(
                label="pin-uuid",
                data_uuid="pin-uuid",
                data_ref="symbol_pin",
                object_id="pin-source",
                extra_attrs={"pin": "1", "symbol_uuid": "symbol-uuid"},
            ),
            op,
            KiCadPlotterOp.end_block(),
        ],
    )
    out = render_record(rec, ctx=ctx)
    # Per-record and per-block identity groups (the ones we key off via
    # ``id=``) are still exactly two; Phase B.2(b) adds inner
    # ``<g style="...">`` style buckets around each drawn op for CLI
    # structural parity, but those don't carry an ``id``.
    assert out.count('<g id=') == 2
    assert 'id="symbol-uuid"' in out
    assert 'id="pin-uuid"' in out
    assert 'data-uuid="pin-uuid"' in out
    assert 'data-ref="symbol_pin"' in out
    assert 'data-object-id="pin-source"' in out
    assert 'data-pin="1"' in out
    assert 'data-symbol-uuid="symbol-uuid"' in out
    assert out.index('id="pin-uuid"') < out.index("<circle")


def test_render_record_adds_schematic_enrichment_attrs(
    ctx: KiCadSvgRenderContext,
) -> None:
    op = KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)
    rec = KiCadPlotterRecord(
        uuid="symbol-uuid",
        kind="symbol_instance",
        object_id="Device:R",
        operations=[op],
        extras={
            "reference": "R1",
            "lib_id": "Device:R",
            "lib_name": "Device",
            "unit": 1,
            "convert": 1,
            "in_bom": True,
            "on_board": True,
            "dnp": False,
            "exclude_from_sim": False,
            "in_pos_files": True,
        },
    )

    out = render_record(rec, ctx=ctx)

    assert 'data-primitive="symbol"' in out
    assert 'data-source-kind="schematic"' in out
    assert 'data-component="R1"' in out
    assert 'data-component-uuid="symbol-uuid"' in out
    assert 'data-symbol-library-ref="Device:R"' in out
    assert 'data-symbol-role="component"' in out


def test_render_record_marks_power_symbol_role(ctx: KiCadSvgRenderContext) -> None:
    rec = KiCadPlotterRecord(
        uuid="power-symbol-uuid",
        kind="symbol_instance",
        object_id="power:+5V",
        operations=[KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)],
        extras={"reference": "#PWR01", "lib_id": "power:+5V"},
    )

    out = render_record(rec, ctx=ctx)

    assert 'data-primitive="power-symbol"' in out
    assert 'data-symbol-role="power"' in out


def test_render_record_oracle_profile_suppresses_schematic_enrichment() -> None:
    ctx = KiCadSvgRenderContext(
        sheet_width_nm=297_000_000,
        sheet_height_nm=210_000_000,
        options=KiCadSvgRenderOptions.kicad_native(),
    )
    rec = KiCadPlotterRecord(
        uuid="wire-uuid",
        kind="wire",
        object_id="wire-uuid",
        operations=[KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)],
    )

    out = render_record(rec, ctx=ctx)

    assert 'data-primitive=' not in out
    assert 'data-source-kind=' not in out
    assert 'data-ref=' not in out


def test_render_record_no_group_returns_body_only(ctx: KiCadSvgRenderContext) -> None:
    op = KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)
    rec = KiCadPlotterRecord(uuid="ABC", kind="wire", object_id="w1", operations=[op])
    out = render_record(rec, ctx=ctx, include_group=False)
    # No identity envelope when include_group=False; the inner
    # ``<g style="...">`` style bucket from Phase B.2(b) is expected.
    assert '<g id=' not in out
    assert '<circle' in out


def test_render_records_concatenates_without_envelope(ctx: KiCadSvgRenderContext) -> None:
    r1 = KiCadPlotterRecord(uuid="a", kind="wire", object_id="1",
                            operations=[KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=1)])
    r2 = KiCadPlotterRecord(uuid="b", kind="wire", object_id="2",
                            operations=[KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2)])
    out = render_records([r1, r2], ctx=ctx)
    assert out.count('<g id=') == 2
    assert "<svg" not in out


# ---------------------------------------------------------------------------
# render_ir_to_svg — document envelope
# ---------------------------------------------------------------------------


def test_render_ir_to_svg_envelope_uses_canvas() -> None:
    op = KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)
    rec = KiCadPlotterRecord(uuid="abc", kind="wire", object_id="w", operations=[op])
    doc = KiCadPlotterDocument(
        records=[rec],
        canvas={"width_nm": 297_000_000, "height_nm": 210_000_000},
        source_kind="SCH",
    )
    svg = render_ir_to_svg(doc)
    assert svg.startswith('<?xml')
    assert 'width="297mm"' in svg
    assert 'height="210mm"' in svg
    assert 'viewBox="0 0 297 210"' in svg
    # The wire's group should be in the body.
    assert 'id="abc"' in svg


def test_render_ir_to_svg_draws_sheet_header_last() -> None:
    header = KiCadPlotterRecord(
        uuid="sheet-id",
        kind="sheet_header",
        object_id="sheet",
        operations=[KiCadPlotterOp.rect(x1=0, y1=0, x2=10_000_000, y2=10_000_000)],
    )
    wire = KiCadPlotterRecord(
        uuid="wire-id",
        kind="wire",
        object_id="wire",
        operations=[KiCadPlotterOp.circle(cx=0, cy=0, diameter_nm=2_000_000)],
    )
    doc = KiCadPlotterDocument(
        records=[header, wire],
        canvas={"width_nm": 297_000_000, "height_nm": 210_000_000},
        source_kind="SCH",
    )

    svg = render_ir_to_svg(doc)

    assert svg.index('id="wire-id"') < svg.index('id="sheet-id"')


def test_render_ir_to_svg_draws_sheet_background_before_schematic() -> None:
    background = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.RECT,
        payload={
            "x1": 0,
            "y1": 0,
            "x2": 297_000_000,
            "y2": 210_000_000,
            "fill": KiCadFillType.FILLED_WITH_COLOR.value,
            "fill_color": "#F5F4EFFF",
            "stroke_color": "#F5F4EFFF",
            "width_nm": 0,
            "corner_radius_nm": 0,
        },
    )
    border = KiCadPlotterOp.rect(
        x1=10_000_000,
        y1=10_000_000,
        x2=287_000_000,
        y2=200_000_000,
    )
    header = KiCadPlotterRecord(
        uuid="sheet-id",
        kind="sheet_header",
        object_id="sheet",
        operations=[background, border],
    )
    wire = KiCadPlotterRecord(
        uuid="wire-id",
        kind="wire",
        object_id="wire",
        operations=[
            KiCadPlotterOp.circle(
                cx=50_000_000,
                cy=50_000_000,
                diameter_nm=2_000_000,
            )
        ],
    )
    doc = KiCadPlotterDocument(
        records=[header, wire],
        canvas={"width_nm": 297_000_000, "height_nm": 210_000_000},
        source_kind="SCH",
    )

    svg = render_ir_to_svg(doc)

    assert svg.index('id="sheet-id:background"') < svg.index('id="wire-id"')
    assert svg.index('id="wire-id"') < svg.index('id="sheet-id"')


def test_render_ir_to_svg_falls_back_to_a4_when_canvas_missing() -> None:
    doc = KiCadPlotterDocument(records=[], source_kind="SCH")
    svg = render_ir_to_svg(doc)
    assert 'width="297mm"' in svg
    assert 'height="210mm"' in svg


def test_render_ir_to_svg_is_parseable_xml() -> None:
    op = KiCadPlotterOp.circle(cx=10_000_000, cy=10_000_000, diameter_nm=2_000_000)
    rec = KiCadPlotterRecord(uuid="abc", kind="wire", object_id="w", operations=[op])
    doc = KiCadPlotterDocument(
        records=[rec],
        canvas={"width_nm": 297_000_000, "height_nm": 210_000_000},
    )
    svg = render_ir_to_svg(doc)
    # Strip XML declaration so ET doesn't get confused on multi-root.
    root = ET.fromstring(svg[svg.index("<svg") :])
    assert root.tag.endswith("svg")
    # <rect> background + <g> wire wrapper -> at least 2 children.
    assert len(list(root)) >= 2


def test_render_ir_to_svg_ignores_text_hyperlink_context() -> None:
    base = KiCadPlotterOp.text(
        x=1_000_000,
        y=2_000_000,
        text="linked text",
        size_x_nm=1_000_000,
        size_y_nm=1_000_000,
        color="#000000",
    )
    op = KiCadPlotterOp(
        kind=base.kind,
        payload={
            **base.payload,
            "context": {
                "hyperlink": {"href": "https://example.test/not-rendered"}
            },
        },
    )
    rec = KiCadPlotterRecord(
        uuid="linked-text",
        kind="text",
        object_id="linked-text",
        operations=[op],
    )
    doc = KiCadPlotterDocument(
        records=[rec],
        canvas={"width_nm": 100_000_000, "height_nm": 100_000_000},
        source_kind="SCH",
    )

    svg = render_ir_to_svg(doc)

    assert "linked text" in svg
    assert "https://example.test/not-rendered" not in svg


def test_render_ir_to_svg_options_black_and_white_native_theme() -> None:
    op = KiCadPlotterOp.text(
        x=0, y=0, text="hi", size_x_nm=1_000_000, size_y_nm=1_000_000,
        color="#840000FF",
    )
    rec = KiCadPlotterRecord(uuid="t", kind="text", object_id="t", operations=[op])
    doc = KiCadPlotterDocument(
        records=[rec],
        canvas={"width_nm": 100_000_000, "height_nm": 100_000_000},
    )
    svg = render_ir_to_svg(doc, options=KiCadSvgRenderOptions.black_and_white_native())
    # text colour gets overridden through the semantic black-and-white role theme.
    assert 'fill="#000000"' in svg
    assert "#840000FF" not in svg


def test_render_ir_to_svg_respects_caller_provided_ctx() -> None:
    rec = KiCadPlotterRecord(uuid="x", kind="wire", object_id="w", operations=[])
    doc = KiCadPlotterDocument(
        records=[rec],
        canvas={"width_nm": 100_000_000, "height_nm": 50_000_000},
    )
    custom = KiCadSvgRenderContext(
        sheet_width_nm=100_000_000, sheet_height_nm=50_000_000,
        options=KiCadSvgRenderOptions(background_color="#101010"),
    )
    svg = render_ir_to_svg(doc, ctx=custom)
    assert 'fill="#101010"' in svg


# ---------------------------------------------------------------------------
# End-to-end: schematic -> IR -> SVG
# ---------------------------------------------------------------------------


def test_end_to_end_schematic_ir_to_svg() -> None:
    # Build a tiny schematic with one wire + one junction.
    sch = KiCadSchematic()
    sch.wires.append(SchWire(
        points=[(10.0, 10.0), (30.0, 10.0)],
        stroke=Stroke(width=0.1524),
        uuid="wire-uuid-1",
    ))
    sch.junctions.append(SchJunction(
        at_x=30.0, at_y=10.0,
        diameter=0.9144,
        uuid="junc-uuid-1",
    ))

    doc = schematic_to_ir(sch, source_path="mem.kicad_sch", document_id="d1")
    svg = render_ir_to_svg(doc)

    assert svg.startswith('<?xml')
    assert 'data-uuid="wire-uuid-1"' in svg
    assert 'data-uuid="junc-uuid-1"' in svg
    # Wire emits a polyline (PlotPoly NO_FILL).
    assert "<polyline" in svg
    # Junction emits a filled circle.
    assert "<circle" in svg
    # SVG should be parseable.
    root = ET.fromstring(svg[svg.index("<svg") :])
    assert root.tag.endswith("svg")


def test_end_to_end_lib_symbol_ir_to_svg() -> None:
    """A trivial lib-symbol with one rectangle round-trips through F-3 + F-5."""
    from kicad_monkey import SymRectangle
    from kicad_monkey.kicad_sym_rectangle import SymFill, SymFillType

    sym = LibSymbol(name="R")
    # Symbol body shapes live on a sub-symbol unit (the `_0_1` convention).
    from kicad_monkey import LibSubSymbol
    sub = LibSubSymbol(name="R_0_1")
    sub.rectangles.append(SymRectangle(
        start_x=-2.54, start_y=-1.27, end_x=2.54, end_y=1.27,
        stroke=Stroke(width=0.254),
        fill=SymFill(type=SymFillType.NONE),
    ))
    sym.subsymbols.append(sub)

    doc = lib_symbol_to_ir(sym, source_path="mem.kicad_sym", document_id="r1")
    svg = render_ir_to_svg(doc)

    assert svg.startswith('<?xml')
    assert "<rect" in svg
    root = ET.fromstring(svg[svg.index("<svg") :])
    assert root.tag.endswith("svg")

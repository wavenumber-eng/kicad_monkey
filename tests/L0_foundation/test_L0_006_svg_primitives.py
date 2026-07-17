"""
Test L0_006: SVG primitive layer (Phase F-2)

Pure-unit coverage for the flat ``svg_*`` primitives, the
``KiCadSvgRenderContext``, and the ``KiCadSvgRenderOptions`` factory
profiles. Each test exercises a single primitive against a synthetic
context and asserts on the emitted SVG fragment shape -- no parser,
no oracle.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

from kicad_monkey import (
    KiCadFillType,
    KiCadHorizAlign,
    KiCadJunctionZOrder,
    KiCadLineStyle,
    KiCadSvgRenderContext,
    KiCadSvgRenderOptions,
    KiCadSvgRenderProfile,
    KiCadVariantDimMode,
    KiCadVertAlign,
    SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS,
    SCHEMATIC_SVG_COLOR_ROLES,
    fmt_user_number,
    load_kicad_svg_preference_theme,
    schematic_svg_options_from_preferences,
    schematic_svg_role_color,
    svg_arc,
    svg_bezier,
    svg_circle,
    svg_document,
    svg_ellipse,
    svg_group,
    svg_line,
    svg_path,
    svg_polygon,
    svg_polyline,
    svg_rect,
    svg_text,
    svg_text_or_poly,
    svg_text_poly,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _ctx(**overrides) -> KiCadSvgRenderContext:
    """Build a default ctx with mm-output (1e-6 scale) and the provided overrides."""
    ctx = KiCadSvgRenderContext(
        sheet_width_nm=297_000_000,   # A4 landscape ~ 297 mm
        sheet_height_nm=210_000_000,  # A4 landscape ~ 210 mm
    )
    for k, v in overrides.items():
        setattr(ctx, k, v)
    return ctx


# ---------------------------------------------------------------------------
# fmt_user_number
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "value, expected",
    [
        (0.0, "0"),
        (1.0, "1"),
        (-3.0, "-3"),
        (1.0000000001, "1"),
        (0.5, "0.5"),
        (1.234567, "1.234567"),
        (1.5000000, "1.5"),
        (-0.000000001, "0"),  # near-zero snaps
    ],
)
def test_fmt_user_number(value, expected):
    assert fmt_user_number(value) == expected


# ---------------------------------------------------------------------------
# Options factories
# ---------------------------------------------------------------------------


def test_options_kicad_native_factory():
    o = KiCadSvgRenderOptions.kicad_native()
    assert o.black_and_white is False
    assert o.text_as_polygons is False
    assert o.bezier_as_lines is False
    assert o.junction_z_order == KiCadJunctionZOrder.NATIVE
    assert o.truncate_font_size_for_baseline is True
    assert o.profile == KiCadSvgRenderProfile.ORACLE


def test_options_onscreen_factory():
    o = KiCadSvgRenderOptions.onscreen()
    assert o.junction_z_order == KiCadJunctionZOrder.ALWAYS_ON_TOP
    assert o.truncate_font_size_for_baseline is False


def test_options_enriched_default_factory():
    o = KiCadSvgRenderOptions.enriched_default()
    assert o.profile == KiCadSvgRenderProfile.ENRICHED
    assert o.include_metadata is True
    assert o.include_ids is True
    assert o.junction_z_order == KiCadJunctionZOrder.ALWAYS_ON_TOP


def test_options_polytext_factory():
    o = KiCadSvgRenderOptions.polytext()
    assert o.text_as_polygons is True


def test_options_black_and_white_native_factory():
    o = KiCadSvgRenderOptions.black_and_white_native()
    assert o.black_and_white is False
    assert o.junction_z_order == KiCadJunctionZOrder.NATIVE
    assert o.schematic_role_colors == SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS


def test_options_dataclass_is_mutable():
    o = KiCadSvgRenderOptions()
    assert o.profile == KiCadSvgRenderProfile.ENRICHED
    o.background_color = "#123456"
    assert o.background_color == "#123456"


def test_variant_dim_mode_enum_members():
    members = {m.name for m in KiCadVariantDimMode}
    assert {"NONE", "DIM_OVERLAY", "GREYSCALE"} <= members


# ---------------------------------------------------------------------------
# Context: scale / offset / flip / resolve_color / push_offset
# ---------------------------------------------------------------------------


def test_ctx_default_scale_is_options_value():
    ctx = _ctx()
    assert ctx.effective_scale() == pytest.approx(1e-6)


def test_ctx_explicit_scale_overrides_options():
    ctx = _ctx(scale=2e-6)
    assert ctx.effective_scale() == pytest.approx(2e-6)


def test_ctx_to_user_x_and_y_apply_offset_and_scale():
    ctx = _ctx(offset_x_nm=1_000_000, offset_y_nm=2_000_000)
    assert ctx.to_user_x(5_000_000) == pytest.approx(6.0)   # nm → mm
    assert ctx.to_user_y(8_000_000) == pytest.approx(10.0)


def test_ctx_flip_y_mirrors_around_sheet_height():
    ctx = _ctx(flip_y=True)
    # 0 maps to sheet_height; sheet_height maps to 0 (under default offsets)
    assert ctx.to_user_y(0) == pytest.approx(210.0)
    assert ctx.to_user_y(210_000_000) == pytest.approx(0.0)


def test_ctx_to_user_length_ignores_offset():
    ctx = _ctx(offset_x_nm=99_000_000, offset_y_nm=99_000_000)
    assert ctx.to_user_length(5_000_000) == pytest.approx(5.0)


def test_ctx_resolve_color_passthrough():
    ctx = _ctx()
    assert ctx.resolve_color("#FF0000") == "#FF0000"


def test_ctx_resolve_color_none_returns_current():
    ctx = _ctx(current_color="#0000FF")
    assert ctx.resolve_color(None) == "#0000FF"


def test_ctx_resolve_color_black_and_white_native_uses_role_theme():
    opts = KiCadSvgRenderOptions.black_and_white_native()
    ctx = _ctx(options=opts)
    assert ctx.resolve_color("#009600FF") == "#000000"
    assert ctx.resolve_color("#CC0000FF") == "#000000"
    assert ctx.resolve_color("#FFFFC2FF") == "#FFFFFF"
    assert ctx.resolve_color("#FFFFFF00") == "#FFFFFF"


def test_options_with_schematic_role_colors_validates_and_merges():
    opts = KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
        {"wire": "#222222"},
        component_body="#F8F8F8",
    )

    assert "wire" in SCHEMATIC_SVG_COLOR_ROLES
    assert "foreground" in SCHEMATIC_SVG_COLOR_ROLES
    assert opts.schematic_role_colors == {
        "wire": "#222222",
        "component_body": "#F8F8F8",
    }
    assert schematic_svg_role_color(opts.schematic_role_colors, "symbol_fill") == "#F8F8F8"


def test_options_with_schematic_role_colors_rejects_unknown_role():
    with pytest.raises(ValueError, match="unknown schematic SVG colour role"):
        KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
            {"not_a_role": "#222222"}
        )


def test_ctx_resolve_color_uses_semantic_schematic_roles():
    opts = KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
        wire="#222222",
        component_body="#F8F8F8",
    )
    ctx = _ctx(options=opts)

    assert ctx.resolve_color("#009600FF") == "#222222"
    assert ctx.resolve_color("#FFFFC2FF") == "#F8F8F8"


def test_schematic_svg_options_from_preferences_uses_semantic_roles(tmp_path: Path):
    (tmp_path / "colors").mkdir()
    (tmp_path / "eeschema.json").write_text(
        json.dumps({"appearance": {"color_theme": "review", "default_font": "Arial"}}),
        encoding="utf-8",
    )
    (tmp_path / "colors" / "review.json").write_text(
        json.dumps(
            {
                "schematic": {
                    "background": "#101010",
                    "component_body": "#F8F8F8",
                    "wire": "#222222",
                }
            }
        ),
        encoding="utf-8",
    )

    opts = schematic_svg_options_from_preferences(tmp_path)

    assert opts.schematic_role_colors == {
        "background": "#101010",
        "component_body": "#F8F8F8",
        "wire": "#222222",
    }
    assert opts.font_face_override == "Arial"


def test_svg_preference_theme_without_configured_theme_is_profile_neutral(tmp_path: Path):
    pref = load_kicad_svg_preference_theme(tmp_path)

    assert pref.name == ""
    assert pref.default_font is None
    assert pref.schematic == {}
    assert pref.board == {}


def test_ctx_push_offset_does_not_mutate_self():
    ctx = _ctx(offset_x_nm=1_000_000, offset_y_nm=2_000_000)
    nc = ctx.push_offset(500_000, 300_000)
    assert ctx.offset_x_nm == 1_000_000
    assert ctx.offset_y_nm == 2_000_000
    assert nc.offset_x_nm == 1_500_000
    assert nc.offset_y_nm == 2_300_000


def test_ctx_stroke_scale_falls_back_to_scale():
    ctx = _ctx(scale=2e-6)
    assert ctx.effective_stroke_scale() == pytest.approx(2e-6)
    ctx.stroke_scale = 5e-6
    assert ctx.effective_stroke_scale() == pytest.approx(5e-6)


# ---------------------------------------------------------------------------
# svg_line
# ---------------------------------------------------------------------------


def test_svg_line_basic_shape():
    ctx = _ctx()
    s = svg_line(0, 0, 10_000_000, 0, ctx=ctx, color="#FF0000", width_nm=254_000)
    assert s.startswith("<line ")
    assert s.endswith("/>")
    assert 'x1="0"' in s
    assert 'x2="10"' in s
    assert 'stroke="#FF0000"' in s
    assert 'stroke-linecap="round"' in s


def test_svg_line_dasharray_for_dash_style():
    ctx = _ctx()
    s = svg_line(
        0, 0, 10_000_000, 0,
        ctx=ctx, line_style=KiCadLineStyle.DASH, width_nm=254_000,
    )
    assert "stroke-dasharray=" in s


def test_svg_line_no_dasharray_for_solid():
    ctx = _ctx()
    s = svg_line(0, 0, 10_000_000, 0, ctx=ctx, line_style=KiCadLineStyle.SOLID)
    assert "stroke-dasharray=" not in s


def test_svg_line_uses_semantic_role_theme():
    opts = KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
        wire="#000000"
    )
    ctx = _ctx(options=opts)
    s = svg_line(0, 0, 1_000_000, 0, ctx=ctx, color="#009600FF")
    assert 'stroke="#000000"' in s
    assert "#009600FF" not in s


def test_svg_line_uses_foreground_role_for_custom_schematic_color():
    opts = KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
        foreground="#000000"
    )
    ctx = _ctx(options=opts)
    s = svg_line(0, 0, 1_000_000, 0, ctx=ctx, color="#CC0000FF")
    assert 'stroke="#000000"' in s
    assert "#CC0000FF" not in s


# ---------------------------------------------------------------------------
# svg_rect
# ---------------------------------------------------------------------------


def test_svg_rect_normalises_corner_order():
    ctx = _ctx()
    s = svg_rect(10_000_000, 5_000_000, 0, 0, ctx=ctx)
    assert 'x="0"' in s
    assert 'y="0"' in s
    assert 'width="10"' in s
    assert 'height="5"' in s


def test_svg_rect_corner_radius_emits_rx_ry():
    ctx = _ctx()
    s = svg_rect(0, 0, 10_000_000, 5_000_000, ctx=ctx, corner_radius_nm=1_000_000)
    assert 'rx="1"' in s
    assert 'ry="1"' in s


def test_svg_rect_filled_uses_fill_color():
    ctx = _ctx()
    s = svg_rect(
        0, 0, 5_000_000, 5_000_000,
        ctx=ctx, fill=KiCadFillType.FILLED_WITH_COLOR, fill_color="#00FF00",
    )
    assert 'fill="#00FF00"' in s


def test_svg_rect_no_fill_default():
    ctx = _ctx()
    s = svg_rect(0, 0, 5_000_000, 5_000_000, ctx=ctx)
    assert 'fill="none"' in s


def test_svg_rect_bg_body_color_uses_sheet_color():
    ctx = _ctx(sheet_area_color="#FFEECC")
    s = svg_rect(
        0, 0, 5_000_000, 5_000_000,
        ctx=ctx, fill=KiCadFillType.FILLED_WITH_BG_BODYCOLOR,
    )
    assert 'fill="#FFEECC"' in s


# ---------------------------------------------------------------------------
# svg_circle / svg_ellipse
# ---------------------------------------------------------------------------


def test_svg_circle_basic():
    ctx = _ctx()
    s = svg_circle(5_000_000, 5_000_000, 1_000_000, ctx=ctx)
    assert s.startswith("<circle ")
    assert 'cx="5"' in s
    assert 'cy="5"' in s
    assert 'r="1"' in s


def test_svg_ellipse_basic():
    ctx = _ctx()
    s = svg_ellipse(5_000_000, 5_000_000, 2_000_000, 1_000_000, ctx=ctx)
    assert s.startswith("<ellipse ")
    assert 'rx="2"' in s
    assert 'ry="1"' in s


# ---------------------------------------------------------------------------
# svg_arc
# ---------------------------------------------------------------------------


def test_svg_arc_three_point_emits_path():
    ctx = _ctx()
    # quarter arc from (10,0) thru (~7.07, ~7.07) to (0, 10), centered at origin
    s = svg_arc(
        10_000_000, 0,
        7_071_068, 7_071_068,
        0, 10_000_000,
        ctx=ctx,
    )
    assert s.startswith("<path ")
    assert " A " in s


def test_svg_arc_positive_screen_sweep_uses_sweep_flag_one():
    ctx = _ctx()
    s = svg_arc(
        10_000_000, 0,
        7_071_068, 7_071_068,
        0, 10_000_000,
        ctx=ctx,
    )

    assert re.search(r"A\s+[-0-9.]+\s+[-0-9.]+\s+0\s+0\s+1\s+0\s+10", s)


def test_svg_arc_negative_screen_sweep_uses_sweep_flag_zero():
    ctx = _ctx()
    s = svg_arc(
        0, 10_000_000,
        7_071_068, 7_071_068,
        10_000_000, 0,
        ctx=ctx,
    )

    assert re.search(r"A\s+[-0-9.]+\s+[-0-9.]+\s+0\s+0\s+0\s+10\s+0", s)


def test_svg_arc_collinear_falls_back_to_line():
    ctx = _ctx()
    s = svg_arc(0, 0, 5_000_000, 0, 10_000_000, 0, ctx=ctx)
    assert s.startswith("<line ")


# ---------------------------------------------------------------------------
# svg_polygon / svg_polyline / svg_path
# ---------------------------------------------------------------------------


def test_svg_polygon_emits_points_attr():
    ctx = _ctx()
    s = svg_polygon([(0, 0), (5_000_000, 0), (0, 5_000_000)], ctx=ctx)
    assert s.startswith("<polygon ")
    assert 'points="0,0 5,0 0,5"' in s


def test_svg_polyline_has_no_fill():
    ctx = _ctx()
    s = svg_polyline([(0, 0), (5_000_000, 0)], ctx=ctx)
    assert s.startswith("<polyline ")
    assert 'fill="none"' in s


def test_svg_path_passes_through_d():
    ctx = _ctx()
    s = svg_path("M 0 0 L 10 10", ctx=ctx)
    assert s.startswith("<path ")
    assert 'd="M 0 0 L 10 10"' in s


# ---------------------------------------------------------------------------
# svg_bezier
# ---------------------------------------------------------------------------


def test_svg_bezier_emits_cubic_C_command_by_default():
    ctx = _ctx()
    s = svg_bezier(
        0, 0,
        2_000_000, 5_000_000,
        8_000_000, 5_000_000,
        10_000_000, 0,
        ctx=ctx,
    )
    assert "M 0 0" in s
    assert " C " in s


def test_svg_bezier_flatten_mode_emits_polyline_path():
    opts = KiCadSvgRenderOptions(bezier_as_lines=True, bezier_segment_count=8)
    ctx = _ctx(options=opts)
    s = svg_bezier(
        0, 0,
        2_000_000, 5_000_000,
        8_000_000, 5_000_000,
        10_000_000, 0,
        ctx=ctx,
    )
    # No cubic C command
    assert " C " not in s
    # 8 segments → 1 M + 8 L
    assert s.count(" L ") == 8


# ---------------------------------------------------------------------------
# svg_text
# ---------------------------------------------------------------------------


def test_svg_text_basic_shape():
    ctx = _ctx()
    s = svg_text(
        5_000_000, 5_000_000, "Hello",
        ctx=ctx, color="#000000",
        size_x_nm=1_270_000, size_y_nm=1_270_000,
        h_align=KiCadHorizAlign.LEFT, v_align=KiCadVertAlign.BOTTOM,
    )
    assert s.startswith("<text ")
    assert ">Hello</text>" in s
    assert 'text-anchor="start"' in s
    assert 'dominant-baseline=' not in s
    assert 'font-size="1.6933"' in s
    assert 'textLength="' in s
    assert 'lengthAdjust="spacingAndGlyphs"' in s
    assert 'x="5"' in s
    assert 'y="5"' in s


def test_svg_text_centered_alignment():
    ctx = _ctx()
    s = svg_text(
        0, 0, "X",
        ctx=ctx,
        size_x_nm=1_500_000, size_y_nm=1_500_000,
        h_align=KiCadHorizAlign.CENTER, v_align=KiCadVertAlign.CENTER,
    )
    assert 'text-anchor="middle"' in s
    assert 'dominant-baseline=' not in s
    assert 'font-size="2"' in s
    assert 'y="0.75"' in s


def test_svg_text_xml_escapes_special_chars():
    ctx = _ctx()
    s = svg_text(0, 0, "a & b < c", ctx=ctx)
    assert "&amp;" in s
    assert "&lt;" in s
    assert "<text" in s  # the opening tag itself isn't escaped


def test_svg_text_parameter_substitution():
    ctx = _ctx()
    ctx.parameters["TITLE"] = "MyDesign"
    s = svg_text(0, 0, "Title: ${TITLE}", ctx=ctx)
    assert ">Title: MyDesign</text>" in s


def test_svg_text_unknown_parameter_passes_through():
    ctx = _ctx()
    s = svg_text(0, 0, "${UNKNOWN}", ctx=ctx)
    assert "${UNKNOWN}" in s


def test_svg_text_italic_and_bold_emit_style():
    ctx = _ctx()
    s = svg_text(0, 0, "X", ctx=ctx, italic=True, bold=True)
    assert "font-style: italic" in s
    assert "font-weight: bold" in s


def test_svg_text_font_face_quotes_are_xml_escaped():
    ctx = _ctx()
    s = svg_text(0, 0, "X", ctx=ctx, font_face="Times New Roman")
    assert 'style="font-family: &quot;Times New Roman&quot;"' in s


def test_svg_text_respects_font_face_override():
    opts = KiCadSvgRenderOptions(font_face_override="Arial")
    ctx = _ctx(options=opts)
    s = svg_text(0, 0, "X", ctx=ctx, font_face="Times New Roman")
    assert 'font-family: &quot;Arial&quot;' in s
    assert "Times New Roman" not in s


def test_svg_text_length_uses_kicad_tab_stops():
    ctx = _ctx()
    plain = svg_text(0, 0, "TABLE", ctx=ctx, size_x_nm=1_800_000, font_face="Consolas")
    tabbed = svg_text(0, 0, "\t\t\tTABLE", ctx=ctx, size_x_nm=1_800_000, font_face="Consolas")
    plain_len = float(re.search(r'textLength="([^"]+)"', plain).group(1))
    tabbed_len = float(re.search(r'textLength="([^"]+)"', tabbed).group(1))
    assert tabbed_len > plain_len + 10


def test_svg_text_length_uses_kicad_fontconfig_substitute():
    ctx = _ctx()
    missing_face = svg_text(
        0,
        0,
        "PRODUCTION",
        ctx=ctx,
        size_x_nm=8_890_000,
        font_face="Fragment Mono",
        bold=True,
    )
    substituted = svg_text(
        0,
        0,
        "PRODUCTION",
        ctx=ctx,
        size_x_nm=8_890_000,
        font_face="Bookman Old Style",
        bold=True,
    )
    missing_len = re.search(r'textLength="([^"]+)"', missing_face).group(1)
    substituted_len = re.search(r'textLength="([^"]+)"', substituted).group(1)
    assert missing_len == substituted_len


def test_svg_text_length_uses_kicad_fontconfig_style_substitute():
    ctx = _ctx()
    montserrat_italic = Path("C:/Windows/Fonts/Montserrat-Italic.ttf")
    if not montserrat_italic.exists():
        pytest.skip("Windows Montserrat Italic font is required for this fontconfig fallback test")

    missing_face = svg_text(
        0,
        0,
        "PCB Mixdown",
        ctx=ctx,
        size_x_nm=1_905_000,
        font_face="Fragment Mono",
        italic=True,
    )
    substituted = svg_text(
        0,
        0,
        "PCB Mixdown",
        ctx=ctx,
        size_x_nm=1_905_000,
        font_face="Montserrat",
        italic=True,
    )
    missing_len = re.search(r'textLength="([^"]+)"', missing_face).group(1)
    substituted_len = re.search(r'textLength="([^"]+)"', substituted).group(1)
    assert missing_len == substituted_len
    assert float(missing_len) == pytest.approx(18.7620)


def test_svg_text_length_uses_kicad_markup_subscript_metrics():
    ctx = _ctx()
    s = svg_text(0, 0, "V_{CC}", ctx=ctx, size_x_nm=1_270_000, font_face="Arial")
    text_len = float(re.search(r'textLength="([^"]+)"', s).group(1))
    assert text_len == pytest.approx(2.8276)


def test_outline_font_lookup_preserves_bold_before_regular_embedded_fallback():
    arial = Path("C:/Windows/Fonts/arial.ttf")
    arial_bold = Path("C:/Windows/Fonts/arialbd.ttf")
    if not arial.exists() or not arial_bold.exists():
        pytest.skip("Windows Arial fonts are required for this font lookup test")

    from kicad_monkey.kicad_schematic_to_ir import (
        _EMBEDDED_OUTLINE_FONT_PATHS,
        _cache_clear_outline_fonts,
        _outline_font_path,
    )

    before = dict(_EMBEDDED_OUTLINE_FONT_PATHS)
    try:
        _EMBEDDED_OUTLINE_FONT_PATHS.clear()
        _EMBEDDED_OUTLINE_FONT_PATHS[("arial", False, False)] = str(arial)
        _EMBEDDED_OUTLINE_FONT_PATHS[("arial", True, False)] = str(arial_bold)
        _cache_clear_outline_fonts()
        assert _outline_font_path("Arial", bold=True, italic=True) == str(arial_bold)
    finally:
        _EMBEDDED_OUTLINE_FONT_PATHS.clear()
        _EMBEDDED_OUTLINE_FONT_PATHS.update(before)
        _cache_clear_outline_fonts()


def test_text_box_svg_lines_trim_trailing_source_spaces_for_metrics():
    from kicad_monkey.kicad_primitives import Effects, Font, Stroke
    from kicad_monkey.kicad_sch_text_box import SchTextBox
    from kicad_monkey.kicad_schematic_to_ir import text_box_to_ops
    from kicad_monkey.kicad_sym_rectangle import SymFill

    box = SchTextBox(
        text="alpha \nbeta ",
        at_x=0,
        at_y=0,
        size_x=100,
        size_y=20,
        margins=(0, 0, 0, 0),
        stroke=Stroke(),
        fill=SymFill(),
        effects=Effects(font=Font(size_x=1.27, size_y=1.27)),
    )
    texts = [op.payload["text"] for op in text_box_to_ops(box) if op.kind.name == "TEXT"]
    assert texts == ["alpha", "beta"]


def test_context_color_overrides_apply_to_text():
    opts = KiCadSvgRenderOptions(color_overrides={"#840000FF": "#111111"})
    ctx = _ctx(options=opts)
    s = svg_text(0, 0, "X", ctx=ctx, color="#840000FF")
    assert 'fill="#111111"' in s


def test_svg_text_orientation_emits_rotate_transform():
    ctx = _ctx()
    s = svg_text(
        5_000_000, 10_000_000, "X",
        ctx=ctx,
        orient_deg=90.0,
        size_y_nm=1_500_000,
        v_align=KiCadVertAlign.TOP,
    )
    assert "transform=" in s
    assert "rotate(-90 5 10)" in s
    assert 'y="11.5"' in s


def test_svg_text_no_orient_no_transform():
    ctx = _ctx()
    s = svg_text(0, 0, "X", ctx=ctx, orient_deg=0.0)
    assert "transform=" not in s


def test_svg_text_or_poly_dispatches_to_text_when_polygons_disabled():
    ctx = _ctx()
    assert ctx.options.text_as_polygons is False
    s = svg_text_or_poly(0, 0, "X", ctx=ctx)
    assert s.startswith("<text ")


def test_svg_text_or_poly_dispatches_to_poly_when_enabled():
    opts = KiCadSvgRenderOptions(text_as_polygons=True)
    ctx = _ctx(options=opts)
    s = svg_text_or_poly(0, 0, "X", ctx=ctx)
    # Stroke-font path emits one or more <polyline> strokes, never <text>.
    assert "<text " not in s
    assert "<polyline " in s


def test_svg_text_poly_emits_polyline_strokes():
    ctx = _ctx()
    s = svg_text_poly(0, 0, "X", ctx=ctx)
    assert "<polyline " in s
    assert "<text " not in s


def test_svg_text_poly_empty_text_returns_empty_string():
    ctx = _ctx()
    assert svg_text_poly(0, 0, "", ctx=ctx) == ""


def test_svg_text_uses_kicad_svg_font_size_with_native_truncation_flag():
    opts = KiCadSvgRenderOptions(truncate_font_size_for_baseline=True)
    ctx = _ctx(options=opts)
    # KiCad's SVG font-size is 4/3 of schematic text height; the legacy
    # option is retained for compatibility but no longer truncates output.
    s = svg_text(0, 0, "X", ctx=ctx, size_x_nm=1_500_000, size_y_nm=1_500_000)
    assert 'font-size="2"' in s


# ---------------------------------------------------------------------------
# svg_group
# ---------------------------------------------------------------------------


def test_svg_group_wraps_body():
    body = "<line/>\n<rect/>"
    s = svg_group(body, label="my-grp")
    assert s.startswith("<g ")
    assert s.endswith("</g>")
    assert 'id="my-grp"' in s
    assert "<line/>" in s and "<rect/>" in s


def test_svg_group_data_uuid_and_ref():
    s = svg_group("<x/>", data_uuid="abc-123", data_ref="R1")
    assert 'data-uuid="abc-123"' in s
    assert 'data-ref="R1"' in s


def test_svg_group_transform():
    s = svg_group("<x/>", transform="translate(5 5) rotate(90)")
    assert 'transform="translate(5 5) rotate(90)"' in s


def test_svg_group_no_attrs_minimal():
    s = svg_group("<x/>")
    assert s.startswith("<g>")


# ---------------------------------------------------------------------------
# svg_document
# ---------------------------------------------------------------------------


def test_svg_document_envelope_dimensions():
    ctx = _ctx()
    out = svg_document("<line/>", ctx=ctx)
    assert out.startswith('<?xml version="1.0"')
    assert '<svg ' in out
    assert 'width="297mm"' in out
    assert 'height="210mm"' in out
    assert 'viewBox="0 0 297 210"' in out
    assert 'xmlns:xlink="http://www.w3.org/1999/xlink"' in out
    assert "</svg>" in out
    assert "<line/>" in out


def test_svg_document_includes_background_rect_by_default():
    ctx = _ctx()
    out = svg_document("<line/>", ctx=ctx)
    assert re.search(r'<rect x="0" y="0" .*fill="#FFFFFF"', out) is not None


def test_svg_document_uses_semantic_background_role_color():
    opts = KiCadSvgRenderOptions.enriched_default().with_schematic_role_colors(
        background="#101010"
    )
    ctx = _ctx(options=opts)
    out = svg_document("<line/>", ctx=ctx)
    assert re.search(r'<rect x="0" y="0" .*fill="#101010"', out) is not None


def test_svg_document_skip_background_when_none():
    opts = KiCadSvgRenderOptions(background_color=None)
    ctx = _ctx(options=opts)
    out = svg_document("<line/>", ctx=ctx)
    assert 'fill="#FFFFFF"' not in out


def test_svg_document_skip_xml_declaration():
    opts = KiCadSvgRenderOptions(include_xml_declaration=False)
    ctx = _ctx(options=opts)
    out = svg_document("<line/>", ctx=ctx)
    assert not out.startswith("<?xml")
    assert out.startswith("<svg ")


def test_svg_document_explicit_dimensions_override_ctx():
    ctx = _ctx()
    out = svg_document("<line/>", ctx=ctx, width_nm=100_000_000, height_nm=50_000_000)
    assert 'width="100mm"' in out
    assert 'height="50mm"' in out


def test_svg_document_empty_body_iter():
    ctx = _ctx()
    out = svg_document(["", "<line/>", ""], ctx=ctx)
    assert "<line/>" in out


def test_svg_document_unitless_suffix():
    opts = KiCadSvgRenderOptions(output_unit_suffix="")
    ctx = _ctx(options=opts)
    out = svg_document("<line/>", ctx=ctx)
    assert 'width="297"' in out
    assert "mm" not in out.split("<svg")[1].split(">", 1)[0]


# ---------------------------------------------------------------------------
# Cross-primitive: scale shift updates all output
# ---------------------------------------------------------------------------


def test_alternate_scale_changes_emitted_coords():
    # 1 mm output unit per 1_000_000 nm → 1e-6 (default mm)
    ctx_mm = _ctx()
    s_mm = svg_line(0, 0, 1_000_000, 0, ctx=ctx_mm)
    assert 'x2="1"' in s_mm

    # Switch to user-units = nm directly (scale=1.0)
    ctx_nm = _ctx(scale=1.0)
    s_nm = svg_line(0, 0, 1_000_000, 0, ctx=ctx_nm)
    assert 'x2="1000000"' in s_nm

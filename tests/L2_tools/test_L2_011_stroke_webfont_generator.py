"""
L2_011: KiCad Stroke Webfont Generator Tests

Covers tools/generate_kicad_stroke_webfont.py: outline generation from the
vendored Newstroke stroke tables, weight/italic naming and OS/2 flags, CSS
emission, and the CLI entry point. Small codepoint subsets keep builds fast.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest
from fontTools.ttLib import TTFont

REPO_ROOT = Path(__file__).resolve().parents[2]
GENERATOR_PATH = REPO_ROOT / "tools" / "generate_kicad_stroke_webfont.py"

_spec = importlib.util.spec_from_file_location(
    "generate_kicad_stroke_webfont", GENERATOR_PATH
)
assert _spec is not None and _spec.loader is not None
webfont = importlib.util.module_from_spec(_spec)
# dataclasses resolves cls.__module__ through sys.modules during class
# creation, so the module must be registered before exec.
sys.modules[_spec.name] = webfont
_spec.loader.exec_module(webfont)

# ASCII letters exercised in most tests; small enough to build in well under
# a second per plan.
BASIC_CODEPOINTS = tuple(range(0x20, 0x7F))
GREEK_SYMBOL_CODEPOINTS = (0x0041, 0x03A9, 0x03C0, 0x2126, 0x2202, 0x23DA)

REGULAR_PLAN = webfont.FontBuildPlan(
    weight_name="regular",
    stroke_ratio=webfont.WEIGHT_STROKE_RATIOS["regular"],
    italic=False,
    quad_segs=4,
)


def test_em_scale_targets_cap_height() -> None:
    # Cap centerline span (1.0) plus one stroke width of round-cap ink fills
    # CAP_HEIGHT_EM of the em.
    assert REGULAR_PLAN.em_scale == pytest.approx(0.72 / (1.0 + 1.0 / 8.0))


def test_build_font_data_metrics_and_cap_ink() -> None:
    font_data = webfont.build_font_data(REGULAR_PLAN, BASIC_CODEPOINTS)
    assert font_data.family_name == "KiCad Stroke"
    assert font_data.style_name == "Regular"
    assert font_data.full_name == "KiCad Stroke"
    assert font_data.file_stem == "kicad-stroke"
    assert font_data.weight == 400
    assert not font_data.italic
    glyph_units = REGULAR_PLAN.em_scale * 1000
    # Capital H ink spans the declared cap height (centerline 1.0 plus one
    # stroke width from the round caps).
    h_ys = [y for contour in font_data.contours["uni0048"] for _, y in contour]
    assert max(h_ys) - min(h_ys) == pytest.approx(font_data.cap_height, abs=2)
    # Baseline sits at y=0: H bottom dips only half a stroke below.
    assert min(h_ys) == pytest.approx(-(1.0 / 8.0) / 2.0 * glyph_units, abs=2)
    # Advance is the KiCad glyph width exactly.
    glyph_map = webfont.newstroke_glyph_map()
    assert font_data.advances["uni0048"] == round(
        glyph_map[ord("H")].width * glyph_units
    )
    # Space carries advance but no ink.
    assert font_data.contours["uni0020"] == []
    assert font_data.advances["uni0020"] > 0
    # Typographic line box follows the KiCad interline pitch.
    assert font_data.ascent == round(webfont.DEFAULT_ASCENT_FACTOR * glyph_units)
    assert font_data.ascent + font_data.descent == round(
        webfont.DEFAULT_LINE_SPACING_FACTOR * glyph_units
    )
    assert font_data.win_ascent >= font_data.ascent
    assert font_data.win_descent >= font_data.descent


def test_greek_and_symbol_glyphs_have_ink() -> None:
    font_data = webfont.build_font_data(REGULAR_PLAN, GREEK_SYMBOL_CODEPOINTS)
    for code in GREEK_SYMBOL_CODEPOINTS:
        name = font_data.cmap[code]
        assert font_data.contours[name], f"missing ink for U+{code:04X}"
        assert font_data.advances[name] > 0
    # Newstroke reuses the capital Omega drawing for the Ohm sign.
    assert font_data.contours["uni03A9"] == font_data.contours["uni2126"]
    # Omega is a capital spanning the full cap height.
    omega_ys = [y for contour in font_data.contours["uni03A9"] for _, y in contour]
    assert max(omega_ys) - min(omega_ys) == pytest.approx(font_data.cap_height, abs=3)


def test_unknown_codepoint_is_rejected() -> None:
    with pytest.raises(ValueError, match=r"U\+4E2D"):
        webfont.build_font_data(REGULAR_PLAN, (0x4E2D,))


def test_generate_ttf_woff_and_css(tmp_path: Path) -> None:
    result = webfont.generate_fonts(
        output_dir=tmp_path,
        weights=("regular",),
        formats=("ttf", "woff"),
        quad_segs=4,
        codepoints=BASIC_CODEPOINTS,
    )
    by_format = {font_file.format: font_file.path for font_file in result.font_files}
    assert set(by_format) == {"ttf", "woff"}
    assert by_format["ttf"].name == "kicad-stroke.ttf"
    assert by_format["woff"].is_file()

    parsed = TTFont(by_format["woff"])
    assert parsed.flavor == "woff"
    assert parsed.getBestCmap()[ord("H")] == "uni0048"
    assert parsed["name"].getDebugName(1) == "KiCad Stroke"
    assert parsed["name"].getDebugName(2) == "Regular"
    assert "Newstroke" in parsed["name"].getDebugName(10)
    assert parsed["head"].created == webfont.OPENTYPE_EPOCH
    assert parsed["head"].modified == webfont.OPENTYPE_EPOCH
    assert parsed["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_USE_TYPO_METRICS
    assert parsed["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_REGULAR

    assert result.css_path is not None
    css = result.css_path.read_text(encoding="utf-8")
    assert result.css_path.name == "kicad-monkey-stroke-fonts.css"
    assert "font-family: 'KiCad Stroke';" in css
    assert "url('kicad-stroke.woff') format('woff')" in css
    assert "font-style: normal;" in css


def test_otf_is_real_cff(tmp_path: Path) -> None:
    result = webfont.generate_fonts(
        output_dir=tmp_path,
        weights=("regular",),
        formats=("otf",),
        quad_segs=4,
        codepoints=BASIC_CODEPOINTS,
        write_css=False,
    )
    otf_path = result.font_files[0].path
    parsed = TTFont(otf_path)
    assert parsed.sfntVersion == "OTTO"
    assert "CFF " in parsed
    assert parsed.getBestCmap()[ord("A")] == "uni0041"
    # The intermediate TTF used for webfont conversion is not left behind.
    assert not (tmp_path / "kicad-stroke.ttf").exists()


def test_italic_variant_carries_slant_and_flags(tmp_path: Path) -> None:
    result = webfont.generate_fonts(
        output_dir=tmp_path,
        weights=("regular",),
        italic=True,
        formats=("ttf",),
        quad_segs=4,
        codepoints=BASIC_CODEPOINTS,
    )
    by_name = {font_file.path.name: font_file for font_file in result.font_files}
    assert set(by_name) == {"kicad-stroke.ttf", "kicad-stroke-italic.ttf"}
    italic_file = by_name["kicad-stroke-italic.ttf"]
    assert italic_file.italic
    assert italic_file.full_name == "KiCad Stroke Italic"

    parsed = TTFont(italic_file.path)
    assert parsed["name"].getDebugName(2) == "Italic"
    assert parsed["post"].italicAngle == pytest.approx(
        webfont.ITALIC_ANGLE_DEGREES, abs=0.01
    )
    assert parsed["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_ITALIC
    assert parsed["head"].macStyle == webfont.HEAD_MAC_STYLE_ITALIC

    # KiCad's 1/8 shear leans the ink right: the italic capital I tops out
    # further right than the upright one.
    upright = webfont.build_font_data(REGULAR_PLAN, (ord("I"),))
    italic_plan = webfont.FontBuildPlan(
        weight_name="regular",
        stroke_ratio=webfont.WEIGHT_STROKE_RATIOS["regular"],
        italic=True,
        quad_segs=4,
    )
    slanted = webfont.build_font_data(italic_plan, (ord("I"),))

    def top_x(contours: list[list[tuple[int, int]]]) -> int:
        points = [point for contour in contours for point in contour]
        top = max(y for _, y in points)
        return max(x for x, y in points if y >= top - 2)

    assert top_x(slanted.contours["uni0049"]) > top_x(upright.contours["uni0049"])

    css = (tmp_path / "kicad-monkey-stroke-fonts.css").read_text(encoding="utf-8")
    assert "font-style: italic;" in css
    assert "url('kicad-stroke-italic.ttf') format('truetype')" in css


def test_weight_lineup_flags_and_css(tmp_path: Path) -> None:
    result = webfont.generate_fonts(
        output_dir=tmp_path,
        weights=("light", "regular", "bold"),
        formats=("ttf",),
        quad_segs=4,
        codepoints=BASIC_CODEPOINTS,
    )
    by_name = {font_file.path.name: font_file for font_file in result.font_files}
    assert set(by_name) == {
        "kicad-stroke-light.ttf",
        "kicad-stroke.ttf",
        "kicad-stroke-bold.ttf",
    }
    assert by_name["kicad-stroke-light.ttf"].weight == 300
    assert by_name["kicad-stroke-bold.ttf"].weight == 700
    # KiCad pen rules: light size/12, regular size/8, bold size/5.
    assert by_name["kicad-stroke-light.ttf"].stroke_ratio == pytest.approx(1 / 12)
    assert by_name["kicad-stroke.ttf"].stroke_ratio == pytest.approx(1 / 8)
    assert by_name["kicad-stroke-bold.ttf"].stroke_ratio == pytest.approx(1 / 5)

    bold = TTFont(by_name["kicad-stroke-bold.ttf"].path)
    assert bold["name"].getDebugName(2) == "Bold"
    assert bold["OS/2"].usWeightClass == 700
    assert bold["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_BOLD
    assert not bold["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_REGULAR
    assert bold["head"].macStyle == webfont.HEAD_MAC_STYLE_BOLD
    light = TTFont(by_name["kicad-stroke-light.ttf"].path)
    assert light["OS/2"].usWeightClass == 300
    assert not light["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_BOLD

    assert result.css_path is not None
    css = result.css_path.read_text(encoding="utf-8")
    assert css.count("font-family: 'KiCad Stroke';") == 3
    for weight in (300, 400, 700):
        assert f"font-weight: {weight};" in css

    # Bold strokes are wider: bold H ink dips further below the baseline.
    bold_data = webfont.build_font_data(
        webfont.FontBuildPlan(
            weight_name="bold", stroke_ratio=1 / 5, italic=False, quad_segs=4
        ),
        (ord("H"),),
    )
    regular_data = webfont.build_font_data(REGULAR_PLAN, (ord("H"),))
    bold_min = min(y for c in bold_data.contours["uni0048"] for _, y in c)
    regular_min = min(y for c in regular_data.contours["uni0048"] for _, y in c)
    assert bold_min < regular_min
    # Same cap-height target regardless of weight.
    assert bold_data.cap_height == regular_data.cap_height


def test_invalid_inputs_are_rejected(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="unknown output format"):
        webfont.generate_fonts(
            output_dir=tmp_path, formats=("ttf", "banana"), codepoints=(0x41,)
        )
    with pytest.raises(ValueError, match="unknown weight"):
        webfont.generate_fonts(
            output_dir=tmp_path, weights=("heavy",), codepoints=(0x41,)
        )


def test_cli_builds_ascii_subset(tmp_path: Path) -> None:
    completed = subprocess.run(
        [
            sys.executable,
            str(GENERATOR_PATH),
            "--output-dir",
            str(tmp_path),
            "--formats",
            "ttf",
            "--max-codepoint",
            "0x7F",
            "--quad-segs",
            "4",
            "--no-css",
        ],
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    assert "kicad-stroke.ttf" in completed.stdout
    ttf_path = tmp_path / "kicad-stroke.ttf"
    assert ttf_path.is_file()
    assert not (tmp_path / "kicad-monkey-stroke-fonts.css").exists()
    parsed = TTFont(ttf_path)
    assert parsed.getBestCmap()[ord("A")] == "uni0041"


def test_demo_html_contains_mark_theme_and_engineering_content() -> None:
    art = "  ▓▓▓▓\n▓▓  ▓▓\n"
    theme_css = (
        ':root[data-theme="blue"], [data-theme="blue"] {\n'
        "  --mk-accent: #0aadff;\n"
        "  --mk-bg: #050403;\n"
        "}\n"
    )
    html = webfont.build_demo_html(art, theme_css, "kicad-monkey-stroke-fonts.css")

    assert '<html lang="en" data-theme="blue">' in html
    assert '<meta name="viewport"' in html
    assert '<link rel="icon" href="data:,">' in html
    assert "▓▓  ▓▓" in html
    assert "<h1>kicad monkey</h1>" in html
    assert "--mk-accent: #0aadff;" in html
    assert 'href="kicad-monkey-stroke-fonts.css"' in html
    assert "'KiCad Stroke'" in html
    assert "f₀ = 1/(2π√(LC))" in html
    assert "λ = c/f<br>k = 2π/λ" in html
    assert "Maxwell's equations" in html
    assert (
        '<span class="eq-lhs">∇·D = ρ</span>'
        '<span class="eq-law">(Gauss\'s Law)</span><br>' in html
    )
    assert "GRM188R71C104KA01D" in html
    assert "<h2>FABRICATION NOTES</h2>" in html
    assert "50Ω ±10% SINGLE-ENDED" in html
    assert "Α Β Γ Δ" in html
    assert "α β γ δ" in html
    assert '<td class="label">Light <span class="ratio-light">' in html
    assert '<td class="label">Regular <span class="ratio-regular">' in html
    assert '<td class="label">Bold <span class="ratio-bold">' in html
    assert html.count("<td>R7 4.7kΩ ±1%</td>") == 3
    assert '<input type="radio" name="style-tab" id="tab-regular"' in html
    assert '<label for="tab-italic">Italic</label>' in html
    assert "#tab-italic:checked ~ main { font-style: italic; }" in html
    assert '.ratio-light::after { content: "size/12"; }' in html
    assert '.ratio-regular::after { content: "size/8"; }' in html
    assert '.ratio-bold::after { content: "size/5"; }' in html


def test_demo_html_escapes_art_and_accepts_hyphenated_theme_ids() -> None:
    html = webfont.build_demo_html(
        "<&\n",
        ':root[data-theme="ki-blue"] { --mk-accent: #0aadff; }',
        "fonts.css",
    )
    assert 'data-theme="ki-blue"' in html
    assert "&lt;&amp;" in html


def test_cli_demo_requires_complete_css_configuration(tmp_path: Path) -> None:
    art = tmp_path / "art.txt"
    theme = tmp_path / "theme.css"
    art.write_text("monkey\n", encoding="utf-8")
    theme.write_text(':root[data-theme="blue"] {}\n', encoding="utf-8")

    with pytest.raises(SystemExit, match="must be used together"):
        webfont.main(
            [
                "--output-dir",
                str(tmp_path / "missing-pair"),
                "--demo-art",
                str(art),
            ]
        )
    assert not (tmp_path / "missing-pair").exists()

    with pytest.raises(SystemExit, match="requires CSS output"):
        webfont.main(
            [
                "--output-dir",
                str(tmp_path / "no-css"),
                "--demo-art",
                str(art),
                "--demo-theme-css",
                str(theme),
                "--no-css",
            ]
        )
    assert not (tmp_path / "no-css").exists()

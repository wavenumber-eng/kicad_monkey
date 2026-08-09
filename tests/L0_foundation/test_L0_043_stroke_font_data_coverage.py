"""
L0_043: Stroke Font Data Coverage Tests

Regression tests for the vendored Newstroke glyph table
(``kicad_stroke_font_data.json``). The table mirrors KiCad's
``newstroke_font[]`` array (KiCad 10.0 branch, matching the bundled
kicad-cli rendering reference) and vendors the original CC0 Newstroke
range U+0020..U+2BFF; the differently licensed CJK sections are excluded.

Pins:
- table length and index contract (``codepoint - 0x20``)
- exact legacy ASCII glyph strings so SVG stroke-order parity cannot drift
- decoded stroke coverage for the Greek, math, arrow, and technical
  symbols the SVG renderers and the stroke webfont depend on
"""

from kicad_monkey.kicad_stroke_font import (
    _load_glyph_data,
    get_glyph,
    parse_hershey_glyph,
)

FIRST_CODEPOINT = 0x20
END_CODEPOINT = 0x2C00

# Exact entries from KiCad 10.0 newstroke_font.cpp; stroke ORDER matters for
# SVG parity, so these pin the strings byte-for-byte.
EXPECTED_LITERAL_GLYPHS = {
    " ": "JZ",
    "#": "H]LM[M RRDL_ RYVJV RS_YD",
    "A": "I[MUWU RK[RFY[",
    "e": "I[VZT[P[NZMXMPNNPMTMVNWPWRMT",
}

# Codepoints the demo page and SVG renderers rely on beyond ASCII.
REQUIRED_STROKE_COVERAGE = [
    0x00B0,  # degree sign
    0x00B1,  # plus-minus
    0x00B5,  # micro sign
    0x00D7,  # multiplication sign
    0x00E8,  # e grave
    0x0394,  # Greek capital delta
    0x03A9,  # Greek capital omega
    0x03B1,  # Greek alpha
    0x03C0,  # Greek pi
    0x03C1,  # Greek rho
    0x2022,  # bullet
    0x2126,  # ohm sign
    0x2190,  # left arrow
    0x2192,  # right arrow
    0x2202,  # partial differential
    0x2207,  # nabla
    0x221A,  # square root
    0x2260,  # not equal
    0x2264,  # less-than or equal
    0x2265,  # greater-than or equal
    0x23DA,  # earth ground
]


def test_table_length_covers_newstroke_non_cjk_range():
    data = _load_glyph_data()
    assert len(data) == END_CODEPOINT - FIRST_CODEPOINT


def test_legacy_ascii_glyph_strings_are_unchanged():
    data = _load_glyph_data()
    for char, expected in EXPECTED_LITERAL_GLYPHS.items():
        assert data[ord(char) - FIRST_CODEPOINT] == expected


def test_every_entry_decodes():
    data = _load_glyph_data()
    for index, encoded in enumerate(data):
        glyph = parse_hershey_glyph(encoded)
        assert glyph.width >= 0.0, f"negative width at U+{index + FIRST_CODEPOINT:04X}"


def test_required_symbols_have_ink():
    for codepoint in REQUIRED_STROKE_COVERAGE:
        glyph = get_glyph(chr(codepoint))
        assert glyph is not None, f"U+{codepoint:04X} missing from table"
        assert glyph.strokes, f"U+{codepoint:04X} has no strokes"
        assert glyph.width > 0.0, f"U+{codepoint:04X} has no advance width"


def test_omega_letter_matches_ohm_sign():
    # Newstroke reuses the capital omega drawing for the ohm sign.
    omega = get_glyph("\u03a9")
    ohm = get_glyph("\u2126")
    assert omega is not None and ohm is not None
    assert omega.strokes == ohm.strokes


def test_out_of_range_codepoint_returns_none():
    assert get_glyph("\u2c00") is None
    assert get_glyph("\u4e2d") is None

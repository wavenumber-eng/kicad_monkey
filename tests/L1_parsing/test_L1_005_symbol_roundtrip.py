"""
Subtest: Symbol Library Round-Trip
Stratum: L1_parsing
Purpose: Parse -> serialize -> parse symbol library files with equivalency check

Tests the KiCadSymbolLib OOP model for:
- Parsing without errors
- Version/generator preservation
- Serialization produces valid S-expression
- Round-trip: parse -> serialize -> parse produces equivalent structure
"""

from pathlib import Path

import pytest

from kicad_monkey import KiCadSymbolLib, parse_sexp
from kicad_monkey.kicad_lib_subsymbol import LibSubSymbol
from kicad_monkey.kicad_lib_symbol import LibSymbol
from kicad_monkey.kicad_primitives import Effects
from kicad_monkey.kicad_sym_text import SymText

from conftest import get_symbol_files, get_symbol_test_ids


def test_effects_font_size_parses_kicad_height_width_order():
    effects = Effects.from_sexp(["text", ["effects", ["font", ["size", 3.0, 1.7]]]])

    assert effects.font.size_y == pytest.approx(3.0)
    assert effects.font.size_x == pytest.approx(1.7)
    assert effects.to_sexp()[1][1] == ["size", 3.0, 1.7]


def test_symbol_text_angle_parses_kicad_tenths_of_degree_order():
    text = SymText.from_sexp(["text", "V+", ["at", 1.27, 3.175, 900]])

    assert text.at_angle == pytest.approx(90.0)
    assert text.to_sexp()[2] == ["at", 1.27, 3.175, 900]


def test_symbol_library_writer_uses_kicad_style_formatting_and_quotes_subsymbols():
    symbol = LibSymbol(
        "A-B+%",
        pin_numbers_hide=True,
        pin_names_hide=True,
        subsymbols=[LibSubSymbol("A-B+%_0_1", texts=[SymText("G", hide=True)])],
    )

    text = KiCadSymbolLib(symbols=[symbol]).to_text()
    parsed = parse_sexp(text)
    exported_symbol = next(item for item in parsed if isinstance(item, list) and item[0] == "symbol")
    pin_numbers = next(item for item in exported_symbol if isinstance(item, list) and item[0] == "pin_numbers")
    pin_names = next(item for item in exported_symbol if isinstance(item, list) and item[0] == "pin_names")
    exported_subsymbol = next(item for item in exported_symbol if isinstance(item, list) and item[0] == "symbol")
    exported_text = next(item for item in exported_subsymbol if isinstance(item, list) and item[0] == "text")

    assert "\n\t(version " in text
    assert "\n  (version " not in text
    assert pin_numbers == ["pin_numbers", ["hide", "yes"]]
    assert pin_names == ["pin_names", ["hide", "yes"]]
    assert exported_subsymbol[1] == "A-B+%_0_1"
    assert not any(isinstance(item, list) and item[0] == "hide" for item in exported_text)


class TestSymbolParsing:
    """Test parsing symbol library files."""

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_parse_without_error(self, symbol_file: Path):
        """Symbol file should parse without raising exceptions."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        assert lib is not None
        assert isinstance(lib.symbols, list)

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_has_symbols(self, symbol_file: Path):
        """Parsed library should contain at least one symbol."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        assert len(lib.symbols) >= 1, "Library should have at least one symbol"

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_version_preserved(self, symbol_file: Path):
        """Version number should be preserved from source file."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        assert lib.version > 0, "Version should be positive integer"

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_generator_preserved(self, symbol_file: Path):
        """Generator info should be preserved."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        assert lib.generator, "Generator should not be empty"


class TestSymbolSerialization:
    """Test serializing symbol libraries."""

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_to_sexp_produces_list(self, symbol_file: Path):
        """to_sexp() should produce a valid list structure."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        sexp = lib.to_sexp()
        assert isinstance(sexp, list)
        assert sexp[0] == 'kicad_symbol_lib'

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_to_text_produces_valid_sexp(self, symbol_file: Path):
        """to_text() should produce parseable S-expression."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        text = lib.to_text()
        # Should be parseable
        parsed = parse_sexp(text)
        assert parsed[0] == 'kicad_symbol_lib'

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_symbol_count_preserved(self, symbol_file: Path):
        """Number of symbols should be preserved after round-trip."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        original_count = len(lib.symbols)

        text = lib.to_text()
        lib2 = KiCadSymbolLib.from_text(text)

        assert len(lib2.symbols) == original_count


class TestSymbolRoundTrip:
    """Test full round-trip parsing."""

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_version_roundtrip(self, symbol_file: Path):
        """Version should survive round-trip."""
        lib1 = KiCadSymbolLib.from_file(symbol_file)
        text = lib1.to_text()
        lib2 = KiCadSymbolLib.from_text(text)
        assert lib2.version == lib1.version

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_generator_roundtrip(self, symbol_file: Path):
        """Generator should survive round-trip."""
        lib1 = KiCadSymbolLib.from_file(symbol_file)
        text = lib1.to_text()
        lib2 = KiCadSymbolLib.from_text(text)
        assert lib2.generator == lib1.generator

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_symbol_names_roundtrip(self, symbol_file: Path):
        """Symbol names should survive round-trip."""
        lib1 = KiCadSymbolLib.from_file(symbol_file)
        names1 = [s.name for s in lib1.symbols]

        text = lib1.to_text()
        lib2 = KiCadSymbolLib.from_text(text)
        names2 = [s.name for s in lib2.symbols]

        assert names2 == names1

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_symbol_properties_roundtrip(self, symbol_file: Path):
        """Symbol properties should survive round-trip."""
        lib1 = KiCadSymbolLib.from_file(symbol_file)

        text = lib1.to_text()
        lib2 = KiCadSymbolLib.from_text(text)

        for sym1, sym2 in zip(lib1.symbols, lib2.symbols):
            # Check property count
            assert len(sym2.properties) == len(sym1.properties), \
                f"Property count mismatch for {sym1.name}"

            # Check property keys and values
            for p1, p2 in zip(sym1.properties, sym2.properties):
                assert p2.key == p1.key, f"Property key mismatch: {p1.key} vs {p2.key}"
                assert p2.value == p1.value, f"Property value mismatch for {p1.key}"
                assert p2.id == p1.id, f"Property ID mismatch for {p1.key}"

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_symbol_bulk_content_not_dropped(self, symbol_file: Path):
        """Symbol body content (pins, sub-symbols) survives round-trip.

        Regression for drift root cause #7 (Phase A inventory). The original
        report flagged a 22-vs-183 line collapse on .kicad_sym files; that
        was a side-effect of #1 aborting kicad-cli canonicalisation, not a
        body emitter dropping content. Lock down the corrected diagnosis
        by counting structural children directly.
        """
        src_text = symbol_file.read_text(encoding='utf-8')
        lib = KiCadSymbolLib.from_file(symbol_file)
        emitted = lib.to_text()

        # Whole-file token-bucket counts on the s-expression layer.
        # These cover the body content the inventory worried about
        # (pins, sub-symbols, properties, graphical primitives).
        for tok in ('symbol', 'property', 'pin', 'rectangle',
                    'circle', 'arc', 'polyline'):
            src_n = src_text.count(f'({tok} ') + src_text.count(f'({tok}\n')
            out_n = emitted.count(f'({tok} ') + emitted.count(f'({tok}\n')
            assert out_n == src_n, (
                f"({tok}) child count drift in {symbol_file.name}: "
                f"src={src_n}, emit={out_n}"
            )


class TestSymbolContent:
    """Test symbol content details."""

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_symbol_has_reference(self, symbol_file: Path):
        """Each symbol should have a reference property."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        for symbol in lib.symbols:
            ref = symbol.get_property("Reference")
            assert ref is not None, f"Symbol {symbol.name} missing Reference property"

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_symbol_has_value(self, symbol_file: Path):
        """Each symbol should have a value property."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        for symbol in lib.symbols:
            val = symbol.get_property("Value")
            assert val is not None, f"Symbol {symbol.name} missing Value property"

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_subsymbols_parsed(self, symbol_file: Path):
        """Symbols with graphics should have subsymbols."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        for symbol in lib.symbols:
            # Most symbols should have at least one subsymbol
            # (unless they only use extends)
            if not symbol.extends:
                assert len(symbol.subsymbols) >= 1, \
                    f"Symbol {symbol.name} should have subsymbols"

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_pin_parsing(self, symbol_file: Path):
        """Pins should be parsed correctly."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        for symbol in lib.symbols:
            pins = symbol.get_all_pins()
            for pin in pins:
                # Each pin should have name and number
                assert pin.number is not None, f"Pin missing number in {symbol.name}"
                # Electrical type should be valid
                assert pin.electrical_type is not None


class TestSymbolAccess:
    """Test convenience access methods."""

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_get_symbol_by_name(self, symbol_file: Path):
        """Should be able to get symbol by name."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        if lib.symbols:
            name = lib.symbols[0].name
            found = lib.get_symbol(name)
            assert found is not None
            assert found.name == name

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_iteration(self, symbol_file: Path):
        """Should be able to iterate over symbols."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        count = 0
        for symbol in lib:
            count += 1
            assert symbol.name
        assert count == len(lib.symbols)

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_indexing(self, symbol_file: Path):
        """Should be able to index by name or position."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        if lib.symbols:
            # By index
            sym0 = lib[0]
            assert sym0 == lib.symbols[0]
            # By name
            sym_by_name = lib[lib.symbols[0].name]
            assert sym_by_name == lib.symbols[0]

    @pytest.mark.parametrize("symbol_file", get_symbol_files(), ids=get_symbol_test_ids())
    def test_contains(self, symbol_file: Path):
        """Should support 'in' operator."""
        lib = KiCadSymbolLib.from_file(symbol_file)
        if lib.symbols:
            name = lib.symbols[0].name
            assert name in lib
            assert "nonexistent_symbol_xyz" not in lib


class TestSlice10HidePreservation:
    """Slice 10 — property/pin level (hide yes) preservation.

    KiCad 10's saveField (eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr_lib_cache.cpp)
    emits ``(hide yes)`` as a sibling of ``(effects ...)``, not nested
    inside it. KiCad 9 nested it. We must accept either input form and
    canonicalise emit to the v10 sibling form.
    """

    def test_sym_property_hide_v10_form_roundtrip(self):
        """v10 form: (hide yes) at property level → preserved on emit."""
        from kicad_monkey import parse_sexp
        from kicad_monkey.kicad_sym_property import SymProperty

        src = ('(property "Footprint" "" (id 2) (at 0 0 0) '
               '(show_name no) (do_not_autoplace no) (hide yes) '
               '(effects (font (size 1.27 1.27))))')
        prop = SymProperty.from_sexp(parse_sexp(src))
        assert prop.hide is True
        # Emit must produce the v10 sibling form, not nested in effects.
        out = prop.to_sexp()

        def _has_hide_sibling(sexp):
            return any(isinstance(x, list) and x[:2] == ['hide', 'yes'] for x in sexp)

        assert _has_hide_sibling(out), f"expected (hide yes) sibling, got {out!r}"
        # And not inside effects.
        effects = next(x for x in out if isinstance(x, list) and x[0] == 'effects')
        assert not any(
            (isinstance(item, list) and item[0] == 'hide') or item == 'hide'
            for item in effects[1:]
        ), f"hide should not be nested inside effects, got {effects!r}"

    def test_sym_property_hide_v9_form_promoted(self):
        """v9 form: (hide yes) inside (effects ...) → promoted to property level."""
        from kicad_monkey import parse_sexp
        from kicad_monkey.kicad_sym_property import SymProperty

        src = ('(property "Footprint" "" (id 2) (at 0 0 0) '
               '(effects (font (size 1.27 1.27)) (hide yes)))')
        prop = SymProperty.from_sexp(parse_sexp(src))
        assert prop.hide is True

    def test_sym_property_v10_show_name_dna_sublist_form(self):
        """Slice 12: bare-token show_name / do_not_autoplace was a v9
        form. KiCad 10's reader rejects bare flags inside a property —
        the canonical form is (show_name yes) / (do_not_autoplace yes)
        sub-lists. Verify parse accepts the sub-list AND emit produces it."""
        from kicad_monkey import parse_sexp
        from kicad_monkey.kicad_sym_property import SymProperty

        src = ('(property "Reference" "#PWR090" (id 0) (at 1 2 0) '
               '(show_name yes) (do_not_autoplace yes) '
               '(effects (font (size 1.27 1.27))))')
        prop = SymProperty.from_sexp(parse_sexp(src))
        assert prop.show_name is True
        assert prop.do_not_autoplace is True

        out = prop.to_sexp()
        # Sub-list form, never bare token.
        assert any(isinstance(x, list) and x[:2] == ['show_name', 'yes'] for x in out)
        assert any(isinstance(x, list) and x[:2] == ['do_not_autoplace', 'yes'] for x in out)
        assert 'show_name' not in out, "bare-token form is rejected by KiCad 10"
        assert 'do_not_autoplace' not in out, "bare-token form is rejected by KiCad 10"

    def test_sym_pin_hide_v10_form_roundtrip(self):
        """SymPin: (hide yes) emitted as sub-list, not bare token."""
        from kicad_monkey import parse_sexp
        from kicad_monkey.kicad_sym_pin import SymPin

        src = ('(pin power_in line (at 0 0 0) (length 2.54) (hide yes) '
               '(name "GND" (effects (font (size 1.27 1.27)))) '
               '(number "1" (effects (font (size 1.27 1.27)))))')
        pin = SymPin.from_sexp(parse_sexp(src))
        assert pin.hide is True
        out = pin.to_sexp()
        assert any(isinstance(x, list) and x[:2] == ['hide', 'yes'] for x in out), \
            f"expected (hide yes) sub-list, got {out!r}"

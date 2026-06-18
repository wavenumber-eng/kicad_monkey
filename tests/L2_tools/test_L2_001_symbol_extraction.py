"""
Subtest: Symbol Extraction
Stratum: L2_tools
Purpose: Extract symbols from .kicad_sym files

Tests for KiCad symbol extractors and splitters including:
- Symbol extraction from schematic files
- Library prefix removal
- Symbol library splitting
- Sub-symbol handling
"""

import tempfile
from pathlib import Path

import pytest

from conftest import SPEEDY_SYMBOL_SCHEMATIC_PATH


# ============================================================================
# Module Import Tests
# ============================================================================

def test_module_imports():
    """Test that all extractor modules can be imported."""
    from kicad_monkey.kicad_symbol_extractor import (
        sanitize_filename,
        extract_symbols_from_schematic,
        extract_symbols_from_text,
    )
    from kicad_monkey.kicad_symbol_splitter import split_symbol_library

    assert callable(sanitize_filename)
    assert callable(extract_symbols_from_schematic)
    assert callable(extract_symbols_from_text)
    assert callable(split_symbol_library)


# ============================================================================
# Symbol Extractor Tests
# ============================================================================

class TestSymbolExtractor:
    """Tests for kicad_symbol_extractor.py"""

    def test_sanitize_filename_removes_library_prefix(self):
        """Test that sanitize_filename removes library prefixes."""
        from kicad_monkey.kicad_symbol_extractor import sanitize_filename

        assert sanitize_filename("library:symbol") == "symbol"
        assert sanitize_filename("wn__wavenumber:ASPI-8040S-1R0N-T") == "ASPI-8040S-1R0N-T"

    def test_sanitize_filename_handles_invalid_chars(self):
        """Test that sanitize_filename replaces invalid characters."""
        from kicad_monkey.kicad_symbol_extractor import sanitize_filename

        # Note: colon triggers library prefix removal first, so "symbol<>:test" -> "test"
        assert sanitize_filename("symbol<>:test") == "test"
        assert sanitize_filename("symbol with spaces") == "symbol_with_spaces"
        assert sanitize_filename("symbol<>test") == "symbol__test"

    def test_extract_symbols_from_schematic_file(self):
        """Test extracting symbols from a real schematic file."""
        from kicad_monkey.kicad_symbol_extractor import extract_symbols_from_schematic

        schematic_path = SPEEDY_SYMBOL_SCHEMATIC_PATH

        if not schematic_path.exists():
            pytest.skip(f"Test schematic not found: {schematic_path}")

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)
            count = extract_symbols_from_schematic(schematic_path, output_dir, overwrite=True)

            # Should extract some symbols
            assert count > 0

            # Check that output files were created
            sym_files = list(output_dir.glob("*.kicad_sym"))
            assert len(sym_files) == count

            # Check a specific symbol file if it exists
            aspi_file = output_dir / "ASPI-8040S-1R0N-T.kicad_sym"
            if aspi_file.exists():
                content = aspi_file.read_text()

                # Should have correct generator
                assert 'generator "kicad_symbol_editor"' in content or 'generator kicad_symbol_editor' in content

                # Should have version 9.0
                assert 'generator_version "9.0"' in content or 'generator_version 9.0' in content

                # Symbol name should match filename (no library prefix)
                assert '(symbol "ASPI-8040S-1R0N-T"' in content or '(symbol ASPI-8040S-1R0N-T' in content

    def test_extract_symbols_removes_library_prefix(self):
        """Test that extracted symbols have library prefixes removed."""
        from kicad_monkey.kicad_symbol_extractor import extract_symbols_from_text

        # Sample schematic with library-prefixed symbols
        sample_schematic = """(kicad_sch (version 20211123)
            (lib_symbols
                (symbol "test_lib:TestSymbol"
                    (pin_numbers hide)
                    (property "Reference" "U")
                    (symbol "TestSymbol_1_0"
                        (rectangle (start 0 0) (end 10 10))
                    )
                )
            )
        )"""

        symbols = extract_symbols_from_text(sample_schematic)

        assert len(symbols) == 1
        symbol = symbols[0]
        sexp = str(symbol.to_sexp())

        # Name should not have library prefix
        assert symbol.name == "TestSymbol"
        assert "test_lib:" not in sexp

        # Sub-symbol should also not have library prefix
        assert "TestSymbol_1_0" in sexp


# ============================================================================
# Symbol Library Splitter Tests
# ============================================================================

class TestSymbolLibrarySplitter:
    """Tests for kicad_symbol_library_splitter.py"""

    def test_split_library_creates_individual_files(self):
        """Test splitting a multi-symbol library."""
        from kicad_monkey.kicad_symbol_splitter import split_symbol_library

        # Create a test multi-symbol library
        test_library_content = """(kicad_symbol_lib
            (version 20241209)
            (generator "test_generator")
            (symbol "test_lib:Symbol_A"
                (pin_numbers hide)
                (property "Reference" "R")
                (symbol "Symbol_A_1_0"
                    (rectangle (start 0 0) (end 10 10))
                )
            )
            (symbol "test_lib:Symbol_B"
                (pin_numbers hide)
                (property "Reference" "C")
                (symbol "Symbol_B_1_0"
                    (polyline (pts (xy 0 0) (xy 10 10)))
                )
            )
        )"""

        with tempfile.TemporaryDirectory() as temp_dir:
            # Create test library file
            lib_path = Path(temp_dir) / "test_library.kicad_sym"
            lib_path.write_text(test_library_content)

            # Create output directory
            output_dir = Path(temp_dir) / "output"
            output_dir.mkdir()

            # Split the library
            count = split_symbol_library(lib_path, output_dir, overwrite=True)

            # Should have split into 2 files
            assert count == 2

            # Check that individual files exist
            symbol_a = output_dir / "Symbol_A.kicad_sym"
            symbol_b = output_dir / "Symbol_B.kicad_sym"

            assert symbol_a.exists()
            assert symbol_b.exists()

            # Check Symbol_A content
            content_a = symbol_a.read_text()
            assert 'generator "kicad_symbol_editor"' in content_a or 'generator kicad_symbol_editor' in content_a
            assert 'generator_version "9.0"' in content_a or 'generator_version 9.0' in content_a
            assert '(symbol "Symbol_A"' in content_a or '(symbol Symbol_A' in content_a
            assert 'test_lib:' not in content_a  # No library prefix

            # Check Symbol_B content
            content_b = symbol_b.read_text()
            assert 'generator "kicad_symbol_editor"' in content_b or 'generator kicad_symbol_editor' in content_b
            assert '(symbol "Symbol_B"' in content_b or '(symbol Symbol_B' in content_b
            assert 'test_lib:' not in content_b  # No library prefix

    def test_split_library_handles_subsymbols(self):
        """Test that sub-symbols are properly renamed when splitting."""
        from kicad_monkey.kicad_symbol_splitter import split_symbol_library

        test_library_content = """(kicad_symbol_lib
            (version 20241209)
            (symbol "lib:ComplexSymbol"
                (pin_numbers hide)
                (symbol "ComplexSymbol_1_0"
                    (rectangle (start 0 0) (end 10 10))
                )
                (symbol "ComplexSymbol_2_0"
                    (rectangle (start 0 0) (end 20 20))
                )
            )
        )"""

        with tempfile.TemporaryDirectory() as temp_dir:
            lib_path = Path(temp_dir) / "test_lib.kicad_sym"
            lib_path.write_text(test_library_content)

            output_dir = Path(temp_dir) / "output"
            output_dir.mkdir()

            count = split_symbol_library(lib_path, output_dir, overwrite=True)
            assert count == 1

            # Check that sub-symbols were renamed correctly
            output_file = output_dir / "ComplexSymbol.kicad_sym"
            assert output_file.exists()

            content = output_file.read_text()
            # Main symbol should not have library prefix
            assert '(symbol "ComplexSymbol"' in content or '(symbol ComplexSymbol' in content
            # Sub-symbols should be named correctly
            assert 'ComplexSymbol_1_0' in content
            assert 'ComplexSymbol_2_0' in content
            # Should not have library prefix in sub-symbols
            assert 'lib:' not in content


# ============================================================================
# Run tests standalone
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v"])

"""
Subtest: Footprint Extraction
Stratum: L2_tools
Purpose: Extract footprints from projects

Tests for KiCad footprint extractors including:
- Footprint extraction from PCB files
- Library prefix removal
- STEP model extraction from footprints
- Integration tests for extraction pipeline
"""

import tempfile
from pathlib import Path

import pytest

from conftest import SPEEDY_PCB_PATH, SPEEDY_SYMBOL_SCHEMATIC_PATH, STEP_MODEL_EXTRACT_DIR


# ============================================================================
# Footprint Extractor Tests
# ============================================================================

class TestFootprintExtractor:
    """Tests for kicad_footprint_extractor.py"""

    def test_extract_footprints_from_pcb_file(self):
        """Test extracting footprints from a real PCB file."""
        from kicad_monkey.kicad_footprint_extractor import extract_footprints_from_pcb

        pcb_path = SPEEDY_PCB_PATH

        if not pcb_path.exists():
            pytest.skip(f"Test PCB not found: {pcb_path}")

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)
            count = extract_footprints_from_pcb(pcb_path, output_dir, overwrite=True, unique_only=True)

            # Should extract some footprints
            assert count > 0

            # Check that output files were created
            mod_files = list(output_dir.glob("*.kicad_mod"))
            assert len(mod_files) == count

            # Check that footprints have correct generator
            if mod_files:
                first_file = mod_files[0]
                content = first_file.read_text()

                # Should have pcbnew generator
                assert 'generator pcbnew' in content or 'generator "pcbnew"' in content

    def test_extract_footprints_removes_library_prefix(self):
        """Test that extracted footprints have library prefixes removed."""
        from kicad_monkey.kicad_footprint_extractor import extract_footprints_from_text

        # Sample PCB with library-prefixed footprint
        sample_pcb = """(kicad_pcb (version 20211014)
            (footprint "test_lib:TestFootprint"
                (layer "F.Cu")
                (uuid "12345678-1234-1234-1234-123456789012")
                (at 100 100)
            )
        )"""

        footprints = extract_footprints_from_text(sample_pcb)

        assert len(footprints) == 1
        footprint = footprints[0]
        sexp = str(footprint.to_sexp())

        # Name should not have library prefix
        assert footprint.name == "TestFootprint"
        assert "test_lib:" not in sexp

        # Should have generator fields
        assert footprint.generator


# ============================================================================
# STEP Extractor Tests
# ============================================================================

class TestStepExtractor:
    """Tests for kicad_step_extractor.py"""

    def test_module_import(self):
        """Test that STEP extractor module can be imported."""
        from kicad_monkey.kicad_step_extractor import (
            extract_step_from_directory,
            extract_step_from_footprint,
            extract_step_from_text,
        )

        assert callable(extract_step_from_directory)
        assert callable(extract_step_from_footprint)
        assert callable(extract_step_from_text)

    def test_extract_step_from_directory(self):
        """Test extracting STEP models from all .kicad_mod files in test directory."""
        from kicad_monkey.kicad_step_extractor import extract_step_from_directory

        test_dir = STEP_MODEL_EXTRACT_DIR

        if not test_dir.exists():
            pytest.skip(f"Test directory not found: {test_dir}")

        # Find all .kicad_mod files in the test directory
        kicad_mod_files = list(test_dir.glob("*.kicad_mod"))
        if not kicad_mod_files:
            pytest.skip(f"No .kicad_mod files found in: {test_dir}")

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)

            # Extract STEP models from all footprints in the directory
            success_count, total_count = extract_step_from_directory(
                test_dir, output_dir, recursive=False
            )

            # Should have processed all files
            assert total_count == len(kicad_mod_files)

            # Check that output files were created for successful extractions
            step_files = list(output_dir.glob("*.step")) + list(output_dir.glob("*.stp"))

            # If any files had embedded models, we should have extracted them
            assert len(step_files) == success_count

            # If we extracted any STEP files, verify they start with valid STEP content
            if step_files:
                first_step = step_files[0]
                content = first_step.read_text(encoding='utf-8', errors='ignore')

                # STEP files typically start with "ISO-10303-21;" header
                assert "ISO-10303-21" in content or "STEP" in content.upper()

    def test_extract_step_from_single_file(self):
        """Test extracting STEP model from a single footprint file."""
        from kicad_monkey.kicad_step_extractor import extract_step_from_footprint

        test_dir = STEP_MODEL_EXTRACT_DIR

        if not test_dir.exists():
            pytest.skip(f"Test directory not found: {test_dir}")

        # Find a .kicad_mod file with embedded data
        kicad_mod_files = list(test_dir.glob("*.kicad_mod"))
        if not kicad_mod_files:
            pytest.skip(f"No .kicad_mod files found in: {test_dir}")

        # Try to extract from the first file (may or may not have embedded model)
        test_file = kicad_mod_files[0]

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)

            # Extract (may succeed or fail depending on file content)
            result = extract_step_from_footprint(test_file, output_dir)

            # If extraction succeeded, verify output file
            if result:
                step_files = list(output_dir.glob("*.step")) + list(output_dir.glob("*.stp"))
                assert len(step_files) > 0

                # Verify STEP file content
                step_file = step_files[0]
                assert step_file.stat().st_size > 0

                # Check for valid STEP format
                content = step_file.read_text(encoding='utf-8', errors='ignore')
                assert "ISO-10303-21" in content or "STEP" in content.upper()

    def test_extract_step_no_embedded_model(self):
        """Test that extraction handles files without embedded models gracefully."""
        from kicad_monkey.kicad_step_extractor import extract_step_from_text

        # Sample footprint without embedded model
        sample_footprint = """(footprint "TestFootprint"
            (version 20241229)
            (generator "pcbnew")
            (layer "F.Cu")
            (fp_line (start 0 0) (end 10 10)
                (stroke (width 0.12) (type solid))
                (layer "F.SilkS")
            )
        )"""

        result = extract_step_from_text(sample_footprint)

        # Should return None when no embedded model found
        assert result is None


# ============================================================================
# Integration Tests
# ============================================================================

class TestIntegration:
    """Integration tests for the extractor pipeline."""

    def test_extract_and_verify_generators(self):
        """Test that all extractors use correct generators."""
        from kicad_monkey.kicad_footprint_extractor import extract_footprints_from_pcb
        from kicad_monkey.kicad_symbol_extractor import extract_symbols_from_schematic

        schematic_path = SPEEDY_SYMBOL_SCHEMATIC_PATH
        pcb_path = SPEEDY_PCB_PATH

        if not schematic_path.exists() or not pcb_path.exists():
            pytest.skip("Test files not found")

        with tempfile.TemporaryDirectory() as temp_dir:
            sym_dir = Path(temp_dir) / "symbols"
            fp_dir = Path(temp_dir) / "footprints"
            sym_dir.mkdir()
            fp_dir.mkdir()

            # Extract symbols
            sym_count = extract_symbols_from_schematic(schematic_path, sym_dir, overwrite=True)
            if sym_count > 0:
                # Check first symbol file
                sym_file = next(sym_dir.glob("*.kicad_sym"))
                content = sym_file.read_text()
                assert 'kicad_symbol_editor' in content

            # Extract footprints
            fp_count = extract_footprints_from_pcb(pcb_path, fp_dir, overwrite=True, unique_only=True)
            if fp_count > 0:
                # Check first footprint file
                fp_file = next(fp_dir.glob("*.kicad_mod"))
                content = fp_file.read_text()
                assert 'pcbnew' in content


# ============================================================================
# Run tests standalone
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v"])

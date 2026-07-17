"""
Test L1_006: KiCad Schematic (.kicad_sch) Round-Trip Tests

This test module verifies that schematic files can be parsed and re-serialized
without data loss (round-trip fidelity).

Test Cases:
- Basic parsing and serialization
- Element counts match after round-trip
- Content verification for symbols, wires, labels, etc.
"""

from pathlib import Path

import pytest

from kicad_monkey import (
    KiCadSchematic,
    SchSymbol,
    SchWire,
    SchBus,
    SchJunction,
    SchNoConnect,
    SchLabel,
    SchGlobalLabel,
    SchHierarchicalLabel,
    SchSheet,
    TitleBlock,
    PaperSize,
)
from kicad_monkey.kicad_sexpr import parse_sexp

from conftest import get_schematic_files, get_schematic_test_ids

SCHEMATIC_FILES = get_schematic_files()


def test_schematic_text_href_round_trips_through_effects() -> None:
    text = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-3333-4444-555555555555")
  (paper "A4")
  (text "linked note"
    (exclude_from_sim no)
    (at 1 2 0)
    (effects (font (size 1.27 1.27)) (href "https://example.test/note"))
    (uuid "22222222-3333-4444-5555-666666666666")
  )
)
"""

    sch = KiCadSchematic.from_text(text)

    assert sch.texts[0].effects is not None
    assert sch.texts[0].effects.href == "https://example.test/note"

    emitted = sch.to_text()
    assert '(href "https://example.test/note")' in emitted
    reparsed = KiCadSchematic.from_text(emitted)
    assert reparsed.texts[0].effects is not None
    assert reparsed.texts[0].effects.href == "https://example.test/note"


# ============================================================================
# Test: Basic Parsing
# ============================================================================

class TestSchematicParsing:
    """Test that schematic files can be parsed without errors."""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_parse_schematic(self, sch_file: Path):
        """Parse schematic file."""
        sch = KiCadSchematic.from_file(sch_file)
        assert sch is not None
        assert sch.uuid != ""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_parse_version(self, sch_file: Path):
        """Verify version is parsed correctly."""
        sch = KiCadSchematic.from_file(sch_file)
        # KiCad 9.0 uses version 20250114 or similar
        assert sch.version >= 20200000


# ============================================================================
# Test: Serialization
# ============================================================================

class TestSchematicSerialization:
    """Test that schematic files can be serialized to S-expressions."""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_serialize_to_sexp(self, sch_file: Path):
        """Serialize schematic to S-expression list."""
        sch = KiCadSchematic.from_file(sch_file)
        sexp = sch.to_sexp()
        assert sexp is not None
        assert sexp[0] == "kicad_sch"

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_serialize_to_text(self, sch_file: Path):
        """Serialize schematic to formatted text."""
        sch = KiCadSchematic.from_file(sch_file)
        text = sch.to_text()
        assert text is not None
        assert text.startswith("(kicad_sch")


# ============================================================================
# Test: Round-Trip
# ============================================================================

class TestSchematicRoundTrip:
    """Test that schematic files survive parse-serialize-parse round-trip."""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_roundtrip_reparseable(self, sch_file: Path):
        """Serialized schematic can be re-parsed."""
        sch1 = KiCadSchematic.from_file(sch_file)
        text = sch1.to_text()
        sch2 = KiCadSchematic.from_text(text)
        assert sch2 is not None
        assert sch2.uuid == sch1.uuid

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_roundtrip_symbol_count(self, sch_file: Path):
        """Symbol count matches after round-trip."""
        sch1 = KiCadSchematic.from_file(sch_file)
        text = sch1.to_text()
        sch2 = KiCadSchematic.from_text(text)
        assert len(sch2.symbols) == len(sch1.symbols)

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_roundtrip_wire_count(self, sch_file: Path):
        """Wire count matches after round-trip."""
        sch1 = KiCadSchematic.from_file(sch_file)
        text = sch1.to_text()
        sch2 = KiCadSchematic.from_text(text)
        assert len(sch2.wires) == len(sch1.wires)

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_roundtrip_junction_count(self, sch_file: Path):
        """Junction count matches after round-trip."""
        sch1 = KiCadSchematic.from_file(sch_file)
        text = sch1.to_text()
        sch2 = KiCadSchematic.from_text(text)
        assert len(sch2.junctions) == len(sch1.junctions)

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_roundtrip_label_count(self, sch_file: Path):
        """Label count matches after round-trip."""
        sch1 = KiCadSchematic.from_file(sch_file)
        text = sch1.to_text()
        sch2 = KiCadSchematic.from_text(text)
        assert len(sch2.labels) == len(sch1.labels)

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_roundtrip_sheet_count(self, sch_file: Path):
        """Sheet count matches after round-trip."""
        sch1 = KiCadSchematic.from_file(sch_file)
        text = sch1.to_text()
        sch2 = KiCadSchematic.from_text(text)
        assert len(sch2.sheets) == len(sch1.sheets)


# ============================================================================
# Test: Content Verification
# ============================================================================

class TestSchematicContent:
    """Test that schematic content is correctly parsed."""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_paper_size(self, sch_file: Path):
        """Paper size is parsed."""
        sch = KiCadSchematic.from_file(sch_file)
        assert sch.paper is not None
        assert sch.paper.size != ""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_lib_symbols_parsed(self, sch_file: Path):
        """lib_symbols section is parsed."""
        sch = KiCadSchematic.from_file(sch_file)
        # lib_symbols may be empty if sheet has no symbols
        # but should be a list
        assert isinstance(sch.lib_symbols, list)

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_symbol_properties(self, sch_file: Path):
        """Placed symbols have properties."""
        sch = KiCadSchematic.from_file(sch_file)
        for sym in sch.symbols[:5]:  # Check first 5 symbols
            # All placed symbols should have at least reference and value
            assert sym.lib_id != ""
            # Properties list should exist
            assert isinstance(sym.properties, list)


# ============================================================================
# Test: Convenience Methods
# ============================================================================

class TestSchematicMethods:
    """Test schematic convenience methods."""

    def test_get_symbol_by_reference(self):
        """Test getting symbol by reference designator."""
        # Use led_component which has a symbol D1
        sch_file = next((f for f in SCHEMATIC_FILES if f.name == "led_component.kicad_sch"), None)
        if sch_file is None or not sch_file.exists():
            pytest.skip("led_component.kicad_sch not found")

        sch = KiCadSchematic.from_file(sch_file)
        sym = sch.get_symbol_by_reference("D1")
        assert sym is not None
        assert sym.reference == "D1"

    def test_iter_symbols(self):
        """Test iterating over symbols."""
        sch_file = next((f for f in SCHEMATIC_FILES if f.name == "led_component.kicad_sch"), None)
        if sch_file is None or not sch_file.exists():
            pytest.skip("led_component.kicad_sch not found")

        sch = KiCadSchematic.from_file(sch_file)
        symbols = list(sch)
        assert len(symbols) == len(sch.symbols)

    def test_len_schematic(self):
        """Test len() returns symbol count."""
        sch_file = next((f for f in SCHEMATIC_FILES if f.name == "led_component.kicad_sch"), None)
        if sch_file is None or not sch_file.exists():
            pytest.skip("led_component.kicad_sch not found")

        sch = KiCadSchematic.from_file(sch_file)
        assert len(sch) == len(sch.symbols)


# ============================================================================
# Test: Data-loss invariants (regression guards)
# ============================================================================

class TestSchematicDataLossInvariants:
    """Round-trip invariants for known data-loss bugs.

    Each test guards a specific drift root cause that surfaced during the
    Phase A inventory. Keep one focused assertion per case so a future
    regression points at exactly the bug it broke.
    """

    def test_empty_lib_symbols_block_emitted(self):
        """``(lib_symbols)`` is always emitted, even when empty.

        Regression for drift root cause #2 (Phase A inventory). KiCad emits
        this block unconditionally; dropping it on empty schematics is
        data-loss and breaks downstream tools that key off block presence.
        """
        sch = KiCadSchematic()
        text = sch.to_text()
        assert '(lib_symbols)' in text or '(lib_symbols\n' in text, (
            "schematic with no library symbols must still emit "
            "the empty (lib_symbols) block"
        )

    def test_per_sheet_instances_round_trip(self):
        """Per-sheet ``(instances (project ... (path ... (page ...))))`` round-trips.

        Regression for drift root cause #3 (Phase A inventory). Hierarchical
        schematics encode per-instance page numbers inside each sheet's
        ``(instances ...)`` block; dropping that block on emit re-numbers
        all sheets to "1" on load and breaks the project's hierarchy.
        """
        sch_file = next(
            (f for f in SCHEMATIC_FILES if f.name == "flat_hierarchy.kicad_sch"),
            None,
        )
        if sch_file is None or not sch_file.exists():
            pytest.skip("flat_hierarchy.kicad_sch not in corpus")

        sch = KiCadSchematic.from_file(sch_file)
        sheets_with_instances = [s for s in sch.sheets if s.instances]
        assert sheets_with_instances, (
            "fixture must have at least one sheet with an (instances ...) block "
            "to exercise this round-trip"
        )

        # Re-parse the emitted text and verify per-sheet instances survive.
        text = sch.to_text()
        round_tripped = KiCadSchematic.from_sexp(parse_sexp(text))

        original_paths = {
            s.uuid: sorted((i.project, i.path, i.page) for i in s.instances)
            for s in sch.sheets if s.instances
        }
        round_paths = {
            s.uuid: sorted((i.project, i.path, i.page) for i in s.instances)
            for s in round_tripped.sheets if s.instances
        }
        assert round_paths == original_paths, (
            "per-sheet (instances ...) blocks must round-trip without data loss"
        )


# ============================================================================
# Test: Statistics (for visibility)
# ============================================================================

class TestSchematicStats:
    """Print statistics about parsed schematics for debugging."""

    @pytest.mark.parametrize("sch_file", SCHEMATIC_FILES, ids=get_schematic_test_ids())
    def test_print_stats(self, sch_file: Path, capsys):
        """Print element counts for visibility."""
        sch = KiCadSchematic.from_file(sch_file)

        print(f"\n=== {sch_file.name} ===")
        print(f"  Symbols: {len(sch.symbols)}")
        print(f"  lib_symbols: {len(sch.lib_symbols)}")
        print(f"  Wires: {len(sch.wires)}")
        print(f"  Buses: {len(sch.buses)}")
        print(f"  Junctions: {len(sch.junctions)}")
        print(f"  No Connects: {len(sch.no_connects)}")
        print(f"  Labels: {len(sch.labels)}")
        print(f"  Global Labels: {len(sch.global_labels)}")
        print(f"  Hierarchical Labels: {len(sch.hierarchical_labels)}")
        print(f"  Sheets: {len(sch.sheets)}")
        print(f"  Paper: {sch.paper.size}")

        # Always pass - this test is for visibility
        assert True

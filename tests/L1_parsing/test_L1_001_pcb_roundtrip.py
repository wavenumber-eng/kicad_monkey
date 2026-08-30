"""
Subtest: PCB Round-Trip
Stratum: L1_parsing
Purpose: Parse -> serialize -> parse PCB files with equivalency check

Tests verify that:
1. All .kicad_pcb files can be parsed without errors
2. Parsed files can be serialized back to s-expression format
3. Re-parsing the serialized output produces equivalent data

Round-trip fidelity is measured by:
- Semantic equivalence: All data structures match after round-trip
- Syntactic preservation: Key formatting preserved (numbers, quotes, etc.)
"""

import difflib
from pathlib import Path
from typing import Any, List

import pytest

from kicad_monkey.kicad_sexpr import parse_sexp, QuotedString
from kicad_monkey.kicad_pcb import (
    KiCadPcb, from_kicad_pcb,
)
from kicad_monkey.kicad_base import PadShape, find_element, get_value
from kicad_monkey.testing.corpus import (
    get_kicad_common_board_case_file,
    get_kicad_topic_case_file,
)

from conftest import get_all_pcb_files, get_pcb_test_ids


# ============================================================================
# Parsing Tests
# ============================================================================

class TestParsing:
    """Test that all PCB files can be parsed."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_parse_without_error(self, pcb_path: Path):
        """Test that the PCB file parses without raising exceptions."""
        pcb = from_kicad_pcb(pcb_path)

        # Basic sanity checks
        assert pcb is not None
        assert pcb.version > 0
        assert pcb.generator is not None
        assert len(pcb.layers) > 0

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_version_preserved(self, pcb_path: Path):
        """Test that version number is correctly parsed."""
        content = pcb_path.read_text(encoding='utf-8')
        sexp = parse_sexp(content)
        expected_version = get_value(sexp, 'version')

        pcb = from_kicad_pcb(pcb_path)
        assert pcb.version == expected_version


# ============================================================================
# Serialization Tests
# ============================================================================

class TestSerialization:
    """Test that PCB objects can be serialized."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_to_string_produces_valid_sexp(self, pcb_path: Path):
        """Test that serialized output is valid s-expression."""
        pcb = from_kicad_pcb(pcb_path)
        output = pcb.to_string()

        # Should be parseable
        sexp = parse_sexp(output)
        assert sexp is not None
        assert sexp[0] == 'kicad_pcb'

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_output_ends_with_newline(self, pcb_path: Path):
        """Test POSIX compliance: file ends with newline."""
        pcb = from_kicad_pcb(pcb_path)
        output = pcb.to_string()
        assert output.endswith('\n'), "Output must end with newline (POSIX)"


# ============================================================================
# Round-Trip Tests
# ============================================================================

class TestRoundTrip:
    """Test full round-trip parsing and serialization."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_preserves_version(self, pcb_path: Path):
        """Test that version survives round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert pcb1.version == pcb2.version

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_preserves_layer_count(self, pcb_path: Path):
        """Test that layer count survives round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb1.layers) == len(pcb2.layers)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_preserves_net_count(self, pcb_path: Path):
        """Test that net count survives round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb1.nets) == len(pcb2.nets)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_preserves_footprint_count(self, pcb_path: Path):
        """Test that footprint count survives round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb1.footprints) == len(pcb2.footprints)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_preserves_gr_text_count(self, pcb_path: Path):
        """Test that graphical text count survives round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb1.gr_texts) == len(pcb2.gr_texts)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_preserves_zone_count(self, pcb_path: Path):
        """Test that zone count survives round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb1.zones) == len(pcb2.zones)

    def test_roundtrip_preserves_via_tenting_metadata(self):
        """Via tenting/free metadata should survive parse -> serialize -> parse."""
        via_board = get_kicad_topic_case_file(
            "pcb_roundtrip_features",
            "one_mask_tenting_vias",
            "one_mask_tenting_vias.kicad_pcb",
        )
        if not via_board.exists():
            pytest.skip(f"Input not found: {via_board}")

        pcb1 = from_kicad_pcb(via_board)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        vias1 = {(round(v.at_x, 4), round(v.at_y, 4)): v for v in pcb1.vias}
        vias2 = {(round(v.at_x, 4), round(v.at_y, 4)): v for v in pcb2.vias}

        assert vias1.keys() == vias2.keys()
        for key, v1 in vias1.items():
            v2 = vias2[key]
            assert v1.free == v2.free
            assert v1.tenting == v2.tenting

    def test_roundtrip_preserves_pad_and_footprint_margin_overrides(self):
        """Pad/footprint mask/paste overrides should survive parse -> serialize -> parse."""
        board = get_kicad_common_board_case_file("speedy", "speedy.kicad_pcb")
        if not board.exists():
            pytest.skip(f"Input not found: {board}")

        pcb1 = from_kicad_pcb(board)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        fp1 = next((fp for fp in pcb1.footprints if fp.solder_mask_margin is not None), None)
        fp2 = next((fp for fp in pcb2.footprints if fp.solder_mask_margin is not None), None)
        if fp1 is not None or fp2 is not None:
            assert fp1 is not None and fp2 is not None
            assert fp1.solder_mask_margin == pytest.approx(fp2.solder_mask_margin)

        pad1 = next(
            (
                pad
                for fp in pcb1.footprints
                for pad in fp.pads
                if pad.solder_mask_margin is not None or pad.solder_paste_margin is not None
            ),
            None,
        )
        pad2 = next(
            (
                pad
                for fp in pcb2.footprints
                for pad in fp.pads
                if pad.solder_mask_margin is not None or pad.solder_paste_margin is not None
            ),
            None,
        )
        assert pad1 is not None and pad2 is not None, "Expected pad margin overrides in fixture"
        assert (pad1.solder_mask_margin or 0.0) == pytest.approx(pad2.solder_mask_margin or 0.0)
        assert (pad1.solder_paste_margin or 0.0) == pytest.approx(pad2.solder_paste_margin or 0.0)

    def test_roundtrip_preserves_typed_pad_via_and_footprint_modifier_metadata(self):
        """Typed pad/via machining and footprint pad-group metadata should survive round-trip."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(4 "In1.Cu" signal)
\t\t(6 "In2.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(footprint "Test:NetTie"
\t\t(layer "F.Cu")
\t\t(at 0 0)
\t\t(net_tie_pad_groups "1, 2")
\t\t(duplicate_pad_numbers_are_jumpers no)
\t\t(jumper_pad_groups ("1" "2") ("3" "4"))
\t\t(pad "1" thru_hole circle
\t\t\t(at 0 0)
\t\t\t(size 1.6 1.6)
\t\t\t(drill 0.8)
\t\t\t(layers "*.Cu" "*.Mask")
\t\t\t(remove_unused_layers)
\t\t\t(keep_end_layers no)
\t\t\t(backdrill (size 0.5) (layers "F.Cu" "In1.Cu"))
\t\t\t(tertiary_drill (size 0.4) (layers "B.Cu" "In2.Cu"))
\t\t\t(front_post_machining counterbore (size 1.1) (depth 0.2) (angle 90))
\t\t\t(back_post_machining countersink (size 1.2) (depth 0.25) (angle 75))
\t\t\t(zone_layer_connections "In1.Cu" "In2.Cu")
\t\t)
\t)
\t(via
\t\t(at 5 5)
\t\t(size 1.2)
\t\t(drill 0.6)
\t\t(layers "F.Cu" "B.Cu")
\t\t(backdrill (size 0.45) (layers "F.Cu" "In1.Cu"))
\t\t(tertiary_drill (size 0.35) (layers "B.Cu" "In2.Cu"))
\t\t(front_post_machining counterbore (size 1.0) (depth 0.15) (angle 90))
\t\t(back_post_machining countersink (size 1.1) (depth 0.2) (angle 70))
\t\t(remove_unused_layers yes)
\t\t(keep_end_layers no)
\t\t(start_end_only no)
\t\t(zone_layer_connections "In1.Cu")
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        fp1 = pcb1.footprints[0]
        fp2 = pcb2.footprints[0]
        pad1 = fp1.pads[0]
        pad2 = fp2.pads[0]
        via1 = pcb1.vias[0]
        via2 = pcb2.vias[0]

        assert fp1.net_tie_pad_groups == fp2.net_tie_pad_groups
        assert fp1.duplicate_pad_numbers_are_jumpers is False
        assert fp1.duplicate_pad_numbers_are_jumpers == fp2.duplicate_pad_numbers_are_jumpers
        assert fp1.jumper_pad_groups == fp2.jumper_pad_groups

        assert pad1.backdrill == pad2.backdrill
        assert pad1.tertiary_drill == pad2.tertiary_drill
        assert pad1.front_post_machining == pad2.front_post_machining
        assert pad1.back_post_machining == pad2.back_post_machining
        assert pad1.zone_layer_connections == pad2.zone_layer_connections

        assert via1.backdrill == via2.backdrill
        assert via1.tertiary_drill == via2.tertiary_drill
        assert via1.front_post_machining == via2.front_post_machining
        assert via1.back_post_machining == via2.back_post_machining
        assert via1.remove_unused_layers is True
        assert via1.remove_unused_layers == via2.remove_unused_layers
        assert via1.keep_end_layers is False
        assert via1.keep_end_layers == via2.keep_end_layers
        assert via1.start_end_only is False
        assert via1.start_end_only == via2.start_end_only
        assert via1.zone_layer_connections == via2.zone_layer_connections

    def test_roundtrip_preserves_all_via_unused_layer_policy_forms(self):
        """Via layer-removal flags retain absence, explicit values, and legacy empty forms."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(4 "In1.Cu" signal)
\t\t(6 "In2.Cu" signal)
\t\t(2 "B.Cu" signal)
\t)
\t(via (at 0 0) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu") (uuid "keep-all"))
\t(via (at 1 0) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu")
\t\t(remove_unused_layers yes) (keep_end_layers no) (uuid "remove-all"))
\t(via (at 2 0) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu")
\t\t(remove_unused_layers yes) (keep_end_layers yes) (uuid "keep-span-ends"))
\t(via (at 3 0) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu")
\t\t(start_end_only yes) (uuid "span-ends-only"))
\t(via (at 4 0) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu")
\t\t(remove_unused_layers) (keep_end_layers no) (start_end_only no)
\t\t(uuid "legacy-and-explicit-no"))
)
"""
        expected = [
            (None, None, None),
            (True, False, None),
            (True, True, None),
            (None, None, True),
            (True, False, False),
        ]

        pcb1 = KiCadPcb.from_string(pcb_text)
        assert [
            (via.remove_unused_layers, via.keep_end_layers, via.start_end_only)
            for via in pcb1.vias
        ] == expected

        output = pcb1.to_string()
        assert "(remove_unused_layers yes)" in output
        assert "(keep_end_layers no)" in output
        assert "(start_end_only no)" in output
        assert "\t\t(remove_unused_layers)" not in output

        pcb2 = KiCadPcb.from_string(output)
        assert [
            (via.remove_unused_layers, via.keep_end_layers, via.start_end_only)
            for via in pcb2.vias
        ] == expected

    def test_roundtrip_preserves_auxiliary_layers_and_drill_offsets(self):
        """Modern board layer types and pad drill offsets should parse and round-trip."""
        pcb_text = """(kicad_pcb
\t(version 20240819)
\t(generator "pcbnew")
\t(generator_version "8.99")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(47 "User.1" auxiliary "Assembly aid")
\t)
\t(footprint "Test:OffsetDrill"
\t\t(layer "F.Cu")
\t\t(at 0 0)
\t\t(pad "1" thru_hole oval
\t\t\t(at 0 0)
\t\t\t(size 3 1.7)
\t\t\t(drill oval 1 (offset -0.5 0))
\t\t\t(layers "*.Cu" "*.Mask")
\t\t)
\t\t(pad "2" thru_hole circle
\t\t\t(at 5 0)
\t\t\t(size 1.6 1.6)
\t\t\t(drill 0.8 (offset 0.1 -0.2))
\t\t\t(layers "*.Cu" "*.Mask")
\t\t)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert pcb1.layers[2].layer_type.value == "auxiliary"
        assert pcb1.layers[2].layer_type == pcb2.layers[2].layer_type
        pad1 = pcb2.footprints[0].pads[0]
        pad2 = pcb2.footprints[0].pads[1]

        assert pad1.drill_oval is True
        assert pad1.drill_width == pytest.approx(1.0)
        assert pad1.drill_height is None
        assert pad1.drill_offset_x == pytest.approx(-0.5)
        assert pad1.drill_offset_y == pytest.approx(0.0)
        assert pad2.drill == pytest.approx(0.8)
        assert pad2.drill_offset_x == pytest.approx(0.1)
        assert pad2.drill_offset_y == pytest.approx(-0.2)

    def test_roundtrip_preserves_barcodes_and_richer_dimension_metadata(self):
        """Board/footprint barcodes and KiCad 10 dimension fields should survive round-trip."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(31 "B.Mask" user)
\t\t(32 "F.Mask" user)
\t\t(36 "B.SilkS" user)
\t\t(37 "F.SilkS" user)
\t\t(39 "Cmts.User" user)
\t)
\t(barcode
\t\t(locked yes)
\t\t(at 25 12 90)
\t\t(layer "F.SilkS")
\t\t(size 8 5)
\t\t(text "ABC-123")
\t\t(text_height 1.5)
\t\t(type qr)
\t\t(ecc_level H)
\t\t(hide yes)
\t\t(knockout yes)
\t\t(margins 0.8 0.6)
\t\t(uuid "board-barcode-uuid")
\t)
\t(dimension
\t\t(type aligned)
\t\t(locked yes)
\t\t(layer "Cmts.User")
\t\t(uuid "board-dimension-uuid")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(suppress_zeroes yes)
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0.1)
\t\t\t(keep_text_aligned yes)
\t\t)
\t\t(gr_text "10.0000 mm"
\t\t\t(at 5 2 0)
\t\t\t(layer "Cmts.User")
\t\t\t(uuid "board-dimension-uuid")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t)
\t(footprint "Test:BarcodeCarrier"
\t\t(layer "F.Cu")
\t\t(at 40 50 0)
\t\t(uuid "footprint-uuid")
\t\t(fp_text reference "U1"
\t\t\t(at 0 -2 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t\t(fp_text value "BarcodeCarrier"
\t\t\t(at 0 2 0)
\t\t\t(layer "F.Fab")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t\t(barcode
\t\t\t(locked no)
\t\t\t(at 1 1 0)
\t\t\t(layer "F.SilkS")
\t\t\t(size 4 2)
\t\t\t(text "FP-456")
\t\t\t(text_height 1)
\t\t\t(type code128)
\t\t\t(hide no)
\t\t\t(knockout no)
\t\t\t(uuid "footprint-barcode-uuid")
\t\t)
\t\t(dimension
\t\t\t(type leader)
\t\t\t(layer "Cmts.User")
\t\t\t(uuid "footprint-dimension-uuid")
\t\t\t(pts (xy 1 1) (xy 4 4))
\t\t\t(format
\t\t\t\t(prefix "")
\t\t\t\t(suffix "")
\t\t\t\t(units 0)
\t\t\t\t(units_format 0)
\t\t\t\t(precision 4)
\t\t\t\t(override_value "0.3mm Thickness")
\t\t\t)
\t\t\t(style
\t\t\t\t(thickness 0.1)
\t\t\t\t(arrow_length 1.27)
\t\t\t\t(text_position_mode 0)
\t\t\t\t(text_frame 2)
\t\t\t\t(extension_offset 0.5)
\t\t\t)
\t\t\t(gr_text "0.3mm Thickness"
\t\t\t\t(at 6 4 0)
\t\t\t\t(layer "Cmts.User")
\t\t\t\t(uuid "footprint-dimension-uuid")
\t\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t\t)
\t\t)
\t\t(pad "1" smd rect
\t\t\t(at 0 0)
\t\t\t(size 1 1)
\t\t\t(layers "F.Cu" "F.Mask")
\t\t)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb1.barcodes) == len(pcb2.barcodes) == 1
        assert pcb2.barcodes[0].at_x == pytest.approx(pcb1.barcodes[0].at_x)
        assert pcb2.barcodes[0].at_y == pytest.approx(pcb1.barcodes[0].at_y)
        assert pcb2.barcodes[0].uuid == pcb1.barcodes[0].uuid
        assert pcb2.barcodes[0].barcode_type == "qr"
        assert pcb2.barcodes[0].ecc_level == "H"
        assert pcb2.barcodes[0].show_text is False
        assert pcb2.barcodes[0].knockout is True
        assert pcb2.barcodes[0].margins.x == pytest.approx(0.8)
        assert pcb2.barcodes[0].margins.y == pytest.approx(0.6)

        assert len(pcb1.dimensions) == len(pcb2.dimensions) == 1
        assert pcb2.dimensions[0].locked is True
        assert pcb2.dimensions[0].format.suppress_zeroes is True

        assert len(pcb1.footprints) == len(pcb2.footprints) == 1
        fp1 = pcb1.footprints[0]
        fp2 = pcb2.footprints[0]
        assert len(fp1.barcodes) == len(fp2.barcodes) == 1
        assert fp2.barcodes[0].at_x == pytest.approx(fp1.barcodes[0].at_x)
        assert fp2.barcodes[0].at_y == pytest.approx(fp1.barcodes[0].at_y)
        assert fp2.barcodes[0].uuid == fp1.barcodes[0].uuid
        assert fp2.barcodes[0].barcode_type == "code128"
        assert len(fp1.dimensions) == len(fp2.dimensions) == 1
        assert fp2.dimensions[0].dimension_type == "leader"
        assert fp2.dimensions[0].format.override_value == "0.3mm Thickness"
        assert fp2.dimensions[0].style.text_frame == 2

    def test_roundtrip_preserves_variants_groups_and_footprint_placement_metadata(self):
        """KiCad 10 PCB variants, placement metadata, and component classes must survive round-trip."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(general (thickness 1.6) (legacy_teardrops no))
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(31 "B.Cu" signal)
\t\t(39 "Cmts.User" user)
\t)
\t(variants
\t\t(variant (name "Assembly") (description "Primary build"))
\t\t(variant (name "Debug"))
\t)
\t(footprint "Test:VariantCarrier"
\t\t(layer "F.Cu")
\t\t(at 10 10)
\t\t(attr smd dnp exclude_from_bom)
\t\t(uuid "footprint-variant-uuid")
\t\t(property "Reference" "U1"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "prop-ref")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t\t(property "Value" "VariantCarrier"
\t\t\t(at 0 1.5 0)
\t\t\t(layer "F.Fab")
\t\t\t(uuid "prop-value")
\t\t\t(effects (font (size 1 1) (thickness 0.15)))
\t\t)
\t\t(fp_line
\t\t\t(start 0 0)
\t\t\t(end 1 0)
\t\t\t(stroke (width 0.1) (type default))
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "line-uuid")
\t\t)
\t\t(group ""
\t\t\t(uuid "group-uuid")
\t\t\t(locked yes)
\t\t\t(members "line-uuid" "prop-ref")
\t\t)
\t\t(path "/root-sheet/instance-uuid")
\t\t(sheetname "/Power/")
\t\t(sheetfile "power.kicad_sch")
\t\t(component_classes
\t\t\t(class "Assembly")
\t\t\t(class "Power")
\t\t)
\t\t(variant
\t\t\t(name "Assembly")
\t\t\t(dnp no)
\t\t\t(exclude_from_bom no)
\t\t\t(exclude_from_pos_files yes)
\t\t\t(field (name "Value") (value "AltValue"))
\t\t)
\t\t(pad "1" smd rect
\t\t\t(at 0 0)
\t\t\t(size 1 1)
\t\t\t(layers "F.Cu" "F.Mask")
\t\t)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert [variant.name for variant in pcb2.variants] == ["Assembly", "Debug"]
        assert pcb2.variants[0].description == "Primary build"
        assert pcb2.variants[1].description is None

        assert len(pcb2.footprints) == 1
        footprint = pcb2.footprints[0]
        assert footprint.is_dnp is True
        assert footprint.is_excluded_from_bom is True
        assert footprint.is_excluded_from_pos_files is False

        assert len(footprint.groups) == 1
        assert footprint.groups[0].uuid == "group-uuid"
        assert footprint.groups[0].locked is True
        assert set(footprint.groups[0].members) == {"line-uuid", "prop-ref"}

        assert footprint.path == "/root-sheet/instance-uuid"
        assert footprint.sheetname == "/Power/"
        assert footprint.sheetfile == "power.kicad_sch"
        assert [component_class.name for component_class in footprint.component_classes] == [
            "Assembly",
            "Power",
        ]

        assert len(footprint.variants) == 1
        assert footprint.variants[0].name == "Assembly"
        assert footprint.variants[0].dnp is False
        assert footprint.variants[0].exclude_from_bom is False
        assert footprint.variants[0].exclude_from_pos_files is True
        assert len(footprint.variants[0].fields) == 1
        assert footprint.variants[0].fields[0].name == "Value"
        assert footprint.variants[0].fields[0].value == "AltValue"

        assert "(component_classes" in output
        assert "(path \"/root-sheet/instance-uuid\")" in output
        assert "(sheetname \"/Power/\")" in output
        assert "(sheetfile \"power.kicad_sch\")" in output

    def test_roundtrip_preserves_generated_tuning_patterns(self):
        """Board-level generated tuning patterns should survive round-trip as typed objects.

        KiCad's `(generated ...)` block uses `(id <bare-uuid>)` as the FIRST
        child (NOT `(uuid ...)`) and bare-token members (NOT quoted).
        Misordering or quoting causes kicad-cli to segfault on parse.
        """
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(general (thickness 1.6) (legacy_teardrops no))
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(31 "B.Cu" signal)
\t)
\t(generated
\t\t(id 4f22a815-3048-42b3-86fa-eb71720d35ae)
\t\t(type tuning_pattern)
\t\t(name "Tune 1")
\t\t(layer "F.Cu")
\t\t(locked yes)
\t\t(origin (xy 10 20))
\t\t(target_length 42.5)
\t\t(rounded yes)
\t\t(members 0376a7f9-ca8e-458b-9d43-8cd3a2a6bc63 0461661b-2681-445d-844d-36fe87fd5675)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert len(pcb2.generated_items) == 1
        generated = pcb2.generated_items[0]
        assert generated.uuid == "4f22a815-3048-42b3-86fa-eb71720d35ae"
        assert generated.generator_type == "tuning_pattern"
        assert generated.name == "Tune 1"
        assert generated.layer == "F.Cu"
        assert generated.locked is True
        assert generated.members == [
            "0376a7f9-ca8e-458b-9d43-8cd3a2a6bc63",
            "0461661b-2681-445d-844d-36fe87fd5675",
        ]
        assert [prop.name for prop in generated.properties] == ["origin", "target_length", "rounded"]
        assert "(generated" in output
        assert "(type tuning_pattern)" in output
        # Critical: (id ...) must be the FIRST child of (generated ...).
        assert "(generated\n\t\t(id " in output

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_roundtrip_double_parse_stable(self, pcb_path: Path):
        """Test that parsing output twice produces identical results."""
        pcb1 = from_kicad_pcb(pcb_path)
        output1 = pcb1.to_string()

        pcb2 = KiCadPcb.from_string(output1)
        output2 = pcb2.to_string()

        # Output should be identical after first round-trip
        if output1 != output2:
            # Show diff for debugging
            diff = difflib.unified_diff(
                output1.splitlines(keepends=True),
                output2.splitlines(keepends=True),
                fromfile='first_roundtrip',
                tofile='second_roundtrip',
                n=3
            )
            diff_text = ''.join(list(diff)[:50])  # First 50 lines
            pytest.fail(f"Double round-trip not stable:\n{diff_text}")


# ============================================================================
# Text Elements Tests
# ============================================================================

class TestTextElements:
    """Test text-related element parsing."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_gr_text_text_content_preserved(self, pcb_path: Path):
        """Test that text content of gr_text elements is preserved."""
        pcb = from_kicad_pcb(pcb_path)

        for gr_text in pcb.gr_texts:
            assert gr_text.text is not None
            # Text should be non-empty or at least parseable
            assert isinstance(gr_text.text, str)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_gr_text_layer_preserved(self, pcb_path: Path):
        """Test that layer of gr_text elements is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for gt1, gt2 in zip(pcb1.gr_texts, pcb2.gr_texts):
            assert gt1.layer == gt2.layer

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_gr_text_knockout_preserved(self, pcb_path: Path):
        """Test that knockout flag is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for gt1, gt2 in zip(pcb1.gr_texts, pcb2.gr_texts):
            assert gt1.knockout == gt2.knockout, f"Knockout mismatch for '{gt1.text}'"


# ============================================================================
# Render Cache Tests
# ============================================================================

class TestRenderCache:
    """Test render_cache parsing and preservation."""

    def test_render_cache_preserves_polygon_hole_contours(self):
        """KiCad render_cache polygons can contain exterior and hole contours."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(37 "F.SilkS" user)
\t)
\t(gr_text "HOLE"
\t\t(at 1 2 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(render_cache "HOLE" 0
\t\t\t(polygon
\t\t\t\t(pts (xy 0 0) (xy 2 0) (xy 2 2) (xy 0 2))
\t\t\t\t(pts (xy 0.5 0.5) (xy 1.5 0.5) (xy 1.5 1.5) (xy 0.5 1.5))
\t\t\t)
\t\t)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)

        cache1 = pcb1.gr_texts[0].render_cache
        assert cache1 is not None
        assert len(cache1.polygons) == 1
        polygon1 = cache1.polygons[0]
        assert polygon1.has_holes
        assert len(polygon1.contours) == 2
        assert polygon1.points == [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
        assert polygon1.hole_contours[0].points == [
            (0.5, 0.5),
            (1.5, 0.5),
            (1.5, 1.5),
            (0.5, 1.5),
        ]

        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        cache2 = pcb2.gr_texts[0].render_cache
        assert cache2 is not None
        polygon2 = cache2.polygons[0]
        assert len(polygon2.contours) == 2
        assert polygon2.points == polygon1.points
        assert polygon2.hole_contours[0].points == polygon1.hole_contours[0].points

    def test_text_box_render_caches_roundtrip_for_board_and_footprint_text_boxes(self):
        """PCB text boxes are EDA_TEXT-derived and may carry render_cache blocks."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(37 "F.SilkS" user)
\t)
\t(gr_text_box "Board Box"
\t\t(start 1 1)
\t\t(end 5 3)
\t\t(margins 0.2 0.2 0.2 0.2)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(border yes)
\t\t(render_cache "Board Box" 0
\t\t\t(polygon
\t\t\t\t(pts (xy 1 1) (xy 2 1) (xy 2 2) (xy 1 2))
\t\t\t)
\t\t)
\t)
\t(footprint "Test:Boxed"
\t\t(layer "F.Cu")
\t\t(at 10 10)
\t\t(fp_text_box "Footprint Box"
\t\t\t(start 0 0)
\t\t\t(end 4 2)
\t\t\t(margins 0.1 0.1 0.1 0.1)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(border yes)
\t\t\t(render_cache "Footprint Box" 0
\t\t\t\t(polygon
\t\t\t\t\t(pts (xy 10 10) (xy 11 10) (xy 11 11) (xy 10 11))
\t\t\t\t\t(pts (xy 10.2 10.2) (xy 10.8 10.2) (xy 10.8 10.8) (xy 10.2 10.8))
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())

        board_cache = pcb2.gr_text_boxes[0].render_cache
        assert board_cache is not None
        assert board_cache.polygons[0].points == [
            (1.0, 1.0),
            (2.0, 1.0),
            (2.0, 2.0),
            (1.0, 2.0),
        ]

        fp_cache = pcb2.footprints[0].fp_text_boxes[0].render_cache
        assert fp_cache is not None
        assert fp_cache.polygons[0].has_holes
        assert len(fp_cache.polygons[0].contours) == 2

    def test_polygon_text_boxes_roundtrip_for_board_and_footprint_text_boxes(self):
        """KiCad may serialize rotated text boxes as explicit polygon points."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(37 "F.SilkS" user)
\t)
\t(gr_text_box "Board Poly"
\t\t(pts
\t\t\t(xy 20 30)
\t\t\t(xy 45.980762 15)
\t\t\t(xy 51.980762 25.392305)
\t\t\t(xy 26 40.392305)
\t\t)
\t\t(margins 0.2 0.2 0.2 0.2)
\t\t(angle 30)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(border yes)
\t)
\t(footprint "Test:Boxed"
\t\t(layer "F.Cu")
\t\t(at 10 10 30)
\t\t(fp_text_box "Footprint Poly"
\t\t\t(pts
\t\t\t\t(xy 0 0)
\t\t\t\t(xy 4 0)
\t\t\t\t(xy 4 2)
\t\t\t\t(xy 0 2)
\t\t\t)
\t\t\t(margins 0.1 0.1 0.1 0.1)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(border yes)
\t\t)
\t)
)
"""
        board_points = [
            (20.0, 30.0),
            (45.980762, 15.0),
            (51.980762, 25.392305),
            (26.0, 40.392305),
        ]
        footprint_points = [(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)]

        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert pcb1.gr_text_boxes[0].polygon_points == board_points
        assert pcb2.gr_text_boxes[0].polygon_points == board_points
        assert pcb2.footprints[0].fp_text_boxes[0].polygon_points == footprint_points
        assert "(pts" in output
        assert "(start 20 30)" not in output

    def test_table_cell_render_cache_roundtrips_with_text_effects(self):
        """PCB table cells are text boxes and may carry render_cache blocks."""
        pcb_text = """(kicad_pcb
\t(version 20260206)
\t(generator "pcbnew")
\t(generator_version "10.0")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(37 "F.SilkS" user)
\t)
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external no) (header no))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 30)
\t\t(row_heights 12)
\t\t(cells
\t\t\t(table_cell "Cell"
\t\t\t\t(locked yes)
\t\t\t\t(start 20 10)
\t\t\t\t(end 50 22)
\t\t\t\t(margins 0.6 0.6 0.6 0.6)
\t\t\t\t(span 1 1)
\t\t\t\t(angle 30)
\t\t\t\t(layer "F.SilkS")
\t\t\t\t(uuid "table-cell-uuid")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t\t(render_cache "Cell" 30
\t\t\t\t\t(polygon
\t\t\t\t\t\t(pts (xy 20 10) (xy 21 10) (xy 21 11) (xy 20 11))
\t\t\t\t\t)
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
"""
        pcb1 = KiCadPcb.from_string(pcb_text)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)
        cell = pcb2.tables[0].cells[0]

        assert cell.locked is True
        assert cell.angle == pytest.approx(30.0)
        assert cell.effects is not None
        assert cell.effects.font.face == "Arial"
        assert cell.effects.justify == ["left", "top"]
        assert cell.render_cache is not None
        assert cell.render_cache.text == "Cell"
        assert cell.render_cache.polygons[0].points == [
            (20.0, 10.0),
            (21.0, 10.0),
            (21.0, 11.0),
            (20.0, 11.0),
        ]
        assert "(render_cache" in output

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_render_cache_polygon_count_preserved(self, pcb_path: Path):
        """Test that render_cache polygon count is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for gt1, gt2 in zip(pcb1.gr_texts, pcb2.gr_texts):
            if gt1.render_cache:
                assert gt2.render_cache is not None
                assert len(gt1.render_cache.polygons) == len(gt2.render_cache.polygons)


# ============================================================================
# Embedded Files Tests
# ============================================================================

class TestEmbeddedFiles:
    """Test embedded file handling."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_embedded_files_detected(self, pcb_path: Path):
        """Test that embedded files are detected if present."""
        content = pcb_path.read_text(encoding='utf-8')
        has_embedded = 'embedded_files' in content and '(file' in content

        pcb = from_kicad_pcb(pcb_path)

        if has_embedded:
            # Should have parsed some embedded files
            total = len(pcb.embedded_files)
            for fp in pcb.footprints:
                total += len(fp.embedded_files)
            assert total > 0, "Expected embedded files but found none"

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_embedded_fonts_flag_preserved(self, pcb_path: Path):
        """Test that embedded_fonts flag is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        assert pcb1.embedded_fonts == pcb2.embedded_fonts


# ============================================================================
# Footprints Tests
# ============================================================================

class TestFootprints:
    """Test footprint parsing."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_footprint_library_link_preserved(self, pcb_path: Path):
        """Test that footprint library link is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for fp1, fp2 in zip(pcb1.footprints, pcb2.footprints):
            assert fp1.library_link == fp2.library_link

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_footprint_position_preserved(self, pcb_path: Path):
        """Test that footprint position is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for fp1, fp2 in zip(pcb1.footprints, pcb2.footprints):
            assert abs(fp1.at_x - fp2.at_x) < 0.001
            assert abs(fp1.at_y - fp2.at_y) < 0.001
            assert abs(fp1.at_angle - fp2.at_angle) < 0.001

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_footprint_pad_count_preserved(self, pcb_path: Path):
        """Test that pad count is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for fp1, fp2 in zip(pcb1.footprints, pcb2.footprints):
            assert len(fp1.pads) == len(fp2.pads)

    def test_custom_pad_primitives_roundtrip_preserved(self):
        """Custom pad primitives/options should survive parse -> serialize -> parse."""
        custom_board = get_kicad_topic_case_file(
            "pcb_roundtrip_features",
            "one_custom_pad",
            "one_custom_pad.kicad_pcb",
        )
        if not custom_board.exists():
            pytest.skip(f"Input not found: {custom_board}")

        pcb1 = from_kicad_pcb(custom_board)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        pads1 = [
            pad
            for fp in pcb1.footprints
            for pad in fp.pads
            if pad.shape == PadShape.CUSTOM
        ]
        pads2 = [
            pad
            for fp in pcb2.footprints
            for pad in fp.pads
            if pad.shape == PadShape.CUSTOM
        ]

        assert pads1, "Expected at least one custom pad in fixture"
        assert len(pads1) == len(pads2)

        pad1 = pads1[0]
        pad2 = pads2[0]
        assert pad1.custom_options is not None
        assert pad1.custom_options.anchor == "rect"
        assert pad1.custom_options.clearance == "outline"
        assert pad2.custom_options is not None
        assert pad2.custom_options.anchor == "rect"
        assert pad2.custom_options.clearance == "outline"

        assert len(pad1.custom_primitives) == 1
        assert len(pad2.custom_primitives) == 1
        prim1 = pad1.custom_primitives[0]
        prim2 = pad2.custom_primitives[0]
        assert prim1.primitive_type == "gr_poly"
        assert prim2.primitive_type == "gr_poly"
        assert len(prim1.points) == 6
        assert len(prim2.points) == 6

    def test_chamfered_roundrect_pad_roundtrip_preserved(self):
        """Chamfer pad metadata should survive parse -> serialize -> parse."""
        chamfer_board = get_kicad_topic_case_file(
            "pcb_roundtrip_features",
            "one_chamfer_roundrect",
            "one_chamfer_roundrect.kicad_pcb",
        )
        if not chamfer_board.exists():
            pytest.skip(f"Input not found: {chamfer_board}")

        pcb1 = from_kicad_pcb(chamfer_board)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        pads1 = [
            pad
            for fp in pcb1.footprints
            for pad in fp.pads
            if pad.shape == PadShape.ROUNDRECT and pad.chamfer_corners
        ]
        pads2 = [
            pad
            for fp in pcb2.footprints
            for pad in fp.pads
            if pad.shape == PadShape.ROUNDRECT and pad.chamfer_corners
        ]

        assert pads1, "Expected at least one chamfered roundrect pad in fixture"
        assert len(pads1) == len(pads2)

        pad1 = pads1[0]
        pad2 = pads2[0]
        assert pad1.roundrect_rratio == 0
        assert pad2.roundrect_rratio == 0
        assert pad1.chamfer_ratio == pytest.approx(0.2)
        assert pad2.chamfer_ratio == pytest.approx(0.2)
        assert set(pad1.chamfer_corners) == {"top_left", "top_right", "bottom_left", "bottom_right"}
        assert set(pad2.chamfer_corners) == {"top_left", "top_right", "bottom_left", "bottom_right"}


# ============================================================================
# Zones Tests
# ============================================================================

class TestZones:
    """Test zone parsing."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_zone_net_preserved(self, pcb_path: Path):
        """Test that zone net is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for z1, z2 in zip(pcb1.zones, pcb2.zones):
            assert z1.net == z2.net

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_zone_polygon_count_preserved(self, pcb_path: Path):
        """Test that zone polygon count is preserved."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()
        pcb2 = KiCadPcb.from_string(output)

        for z1, z2 in zip(pcb1.zones, pcb2.zones):
            assert len(z1.polygons) == len(z2.polygons)
            assert len(z1.filled_polygons) == len(z2.filled_polygons)


# ============================================================================
# Semantic Comparison Helpers
# ============================================================================

def compare_sexp_values(v1: Any, v2: Any, path: str = "") -> List[str]:
    """Compare two s-expression values and return list of differences."""
    diffs = []

    if type(v1) is not type(v2):
        # Allow QuotedString vs str comparison
        if isinstance(v1, (str, QuotedString)) and isinstance(v2, (str, QuotedString)):
            if str(v1) != str(v2):
                diffs.append(f"{path}: '{v1}' != '{v2}'")
        else:
            diffs.append(f"{path}: type mismatch {type(v1).__name__} vs {type(v2).__name__}")
    elif isinstance(v1, list):
        if len(v1) != len(v2):
            diffs.append(f"{path}: list length {len(v1)} vs {len(v2)}")
        else:
            for i, (e1, e2) in enumerate(zip(v1, v2)):
                tag = f"[{e1[0]}]" if isinstance(e1, list) and e1 else f"[{i}]"
                diffs.extend(compare_sexp_values(e1, e2, f"{path}{tag}"))
    elif isinstance(v1, float):
        if abs(v1 - v2) > 1e-6:
            diffs.append(f"{path}: {v1} != {v2}")
    elif v1 != v2:
        diffs.append(f"{path}: '{v1}' != '{v2}'")

    return diffs


class TestSemanticEquivalence:
    """Test that round-trip preserves semantic content."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_sexp_roundtrip_semantic_match(self, pcb_path: Path):
        """Test that s-expression content matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        output = pcb1.to_string()

        # Parse both original and output
        original_sexp = parse_sexp(pcb_path.read_text(encoding='utf-8'))
        roundtrip_sexp = parse_sexp(output)

        # Compare key elements
        elements_to_check = ['version', 'generator', 'generator_version']

        for elem_name in elements_to_check:
            orig_elem = find_element(original_sexp, elem_name)
            rt_elem = find_element(roundtrip_sexp, elem_name)

            if orig_elem:
                assert rt_elem is not None, f"Missing {elem_name} after round-trip"
                diffs = compare_sexp_values(orig_elem, rt_elem, elem_name)
                assert not diffs, f"Differences in {elem_name}: {diffs}"


# ============================================================================
# Run tests standalone
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v"])

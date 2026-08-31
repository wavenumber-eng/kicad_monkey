"""
Subtest: Zone Fill Emission
Stratum: L1_parsing
Purpose: Zone.to_sexp must never emit `(fill no ...)`.

KiCad's own writer (pcb_io_kicad_sexpr.cpp:3247-3251) prints "(fill" and
appends a bare "yes" token only when the zone is filled. The parser
(pcb_io_kicad_sexpr_parser.cpp, case T_fill) has no "no" case, so a board
containing `(fill no ...)` fails to load ("Failed to load board", exit 3).
"""

import pytest

from kicad_monkey import KiCadPcb
from kicad_monkey.kicad_pcb_other import NetRef
from kicad_monkey.kicad_pcb_zone import Keepout, Zone
from kicad_monkey.kicad_sexpr import build_sexp, parse_sexp
from kicad_monkey.kicad_zone_filler import ZoneFiller


def _fill_elem(zone: Zone) -> list:
    sexp = zone.to_sexp()
    fills = [e for e in sexp if isinstance(e, list) and e and e[0] == 'fill']
    assert len(fills) == 1
    return fills[0]


class TestZoneFillEmission:
    """Fill token emission must match KiCad's writer exactly."""

    def test_unfilled_zone_emits_bare_fill(self):
        fill = _fill_elem(Zone(layers=["F.Cu"], fill_enabled=False))
        assert fill[0] == 'fill'
        assert 'no' not in fill
        assert 'yes' not in fill
        # First sub-element is the thermal_gap list, not a bool token.
        assert isinstance(fill[1], list)
        assert fill[1][0] == 'thermal_gap'

    def test_filled_zone_emits_yes_token(self):
        fill = _fill_elem(Zone(layers=["F.Cu"], fill_enabled=True))
        assert fill[1] == 'yes'

    def test_keepout_zone_emits_bare_fill(self):
        fill = _fill_elem(Zone(layers=["F.Cu"], keepout=Keepout()))
        assert 'yes' not in fill
        assert 'no' not in fill

    def test_serialized_unfilled_zone_never_contains_fill_no(self):
        text = build_sexp(Zone(layers=["F.Cu"], fill_enabled=False).to_sexp())
        assert '(fill no' not in text
        assert '(fill\n' in text or '(fill ' in text or '(fill(' in text


class TestZoneFillRoundTrip:
    """Parse -> emit keeps the source fill form for both KiCad spellings."""

    def test_bare_fill_roundtrip_preserved(self):
        source = (
            '(zone (net 0) (net_name "") (layer "B.Cu") (uuid "u1") '
            '(hatch edge 0.5) (connect_pads (clearance 0.5)) '
            '(min_thickness 0.25) (filled_areas_thickness no) '
            '(fill (thermal_gap 0.5) (thermal_bridge_width 0.5)) '
            '(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))'
        )
        zone = Zone.from_sexp(parse_sexp(source))
        assert zone.fill_enabled is False
        fill = _fill_elem(zone)
        assert isinstance(fill[1], list)
        assert fill[1][0] == 'thermal_gap'

    def test_fill_yes_roundtrip_preserved(self):
        source = (
            '(zone (net 0) (net_name "") (layer "F.Cu") (uuid "u2") '
            '(hatch edge 0.5) (connect_pads (clearance 0.5)) '
            '(min_thickness 0.25) (filled_areas_thickness no) '
            '(fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5)) '
            '(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))'
        )
        zone = Zone.from_sexp(parse_sexp(source))
        assert zone.fill_enabled is True
        fill = _fill_elem(zone)
        assert fill[1] == 'yes'

    def test_legacy_fill_no_input_normalizes_to_bare_fill(self):
        # Files previously written by kicad_monkey may carry the invalid
        # `(fill no ...)` form; re-emitting must repair it.
        source = (
            '(zone (net 0) (net_name "") (layer "F.Cu") (uuid "u3") '
            '(hatch edge 0.5) (connect_pads (clearance 0.5)) '
            '(min_thickness 0.25) (filled_areas_thickness no) '
            '(fill no (thermal_gap 0.5) (thermal_bridge_width 0.5)) '
            '(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))'
        )
        zone = Zone.from_sexp(parse_sexp(source))
        assert zone.fill_enabled is False
        text = build_sexp(zone.to_sexp())
        assert '(fill no' not in text


def test_zone_filler_uses_can_flash_layer_for_same_net_thermal_eligibility() -> None:
    pcb = KiCadPcb.from_string(
        '''(kicad_pcb
          (layers
            (0 "F.Cu" signal)
            (2 "In1.Cu" power)
            (4 "In2.Cu" power)
            (31 "B.Cu" signal))
          (net 1 "N")
          (footprint "Test:Policy" (layer "F.Cu") (at 10 10)
            (pad "1" thru_hole circle (at 0 0) (size 1 1) (drill 0.5)
              (layers "*.Cu" "*.Mask") (remove_unused_layers yes)
              (keep_end_layers no) (zone_layer_connections "In2.Cu")
              (net 1 "N") (uuid "pad"))))'''
    )
    zone = Zone(net=NetRef(ordinal=1, name="N"), layers=["In1.Cu"])
    filler = ZoneFiller(pcb)

    thermal, clearance = filler._categorize_pads(zone, "In1.Cu")
    assert thermal == [pcb.footprints[0].pads[0]]
    assert clearance == []

    thermal, clearance = filler._categorize_pads(zone, "In2.Cu")
    assert thermal == [pcb.footprints[0].pads[0]]
    assert clearance == []


def test_zone_filler_keeps_physical_holes_when_copper_does_not_flash() -> None:
    pcb = KiCadPcb.from_string(
        '''(kicad_pcb
          (layers
            (0 "F.Cu" signal)
            (2 "In1.Cu" power)
            (31 "B.Cu" signal))
          (net 1 "ZONE")
          (net 2 "OTHER")
          (via (at 5 5) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu")
            (remove_unused_layers yes) (keep_end_layers no) (net 1) (uuid "via"))
          (footprint "Test:Hole" (layer "F.Cu") (at 10 20 90)
            (pad "1" np_thru_hole oval (at 2 2 0) (size 0.5 1.0)
              (drill oval 0.5 1.0)
              (layers "*.Mask") (net 2 "OTHER") (uuid "hole"))))'''
    )
    zone = Zone(
        net=NetRef(ordinal=1, name="ZONE"),
        layers=["In1.Cu"],
        connect_pads_clearance=0.0,
    )
    filler = ZoneFiller(pcb)

    thermal, clearance = filler._categorize_pads(zone, "In1.Cu")
    assert thermal == []
    assert clearance == [pcb.footprints[0].pads[0]]
    pad_hole = filler._get_pad_hole_polygon(clearance[0], pcb.footprints[0])
    min_x, min_y, max_x, max_y = pad_hole.bounds()
    assert max_x - min_x == pytest.approx(0.5, abs=1e-6)
    assert max_y - min_y == pytest.approx(1.0, abs=1e-6)
    assert (min_x + max_x) / 2 == pytest.approx(12.0, abs=1e-6)
    assert (min_y + max_y) / 2 == pytest.approx(18.0, abs=1e-6)

    holes = filler._build_clearance_holes(zone, "In1.Cu", [])
    min_x, min_y, max_x, max_y = holes.bounds()
    assert max_x - min_x == pytest.approx(0.4, abs=0.01)
    assert max_y - min_y == pytest.approx(0.4, abs=0.01)
    assert (min_x + max_x) / 2 == pytest.approx(5.0, abs=0.01)
    assert (min_y + max_y) / 2 == pytest.approx(5.0, abs=0.01)

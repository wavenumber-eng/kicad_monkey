"""
Subtest: Zone Fill Emission
Stratum: L1_parsing
Purpose: Zone.to_sexp must never emit `(fill no ...)`.

KiCad's own writer (pcb_io_kicad_sexpr.cpp:3247-3251) prints "(fill" and
appends a bare "yes" token only when the zone is filled. The parser
(pcb_io_kicad_sexpr_parser.cpp, case T_fill) has no "no" case, so a board
containing `(fill no ...)` fails to load ("Failed to load board", exit 3).
"""

from kicad_monkey.kicad_pcb_zone import Keepout, Zone
from kicad_monkey.kicad_sexpr import build_sexp, parse_sexp


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

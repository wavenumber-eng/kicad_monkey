"""Full-board PCB net lookup cache coverage."""

from __future__ import annotations

import kicad_monkey.kicad_pcb as pcb_mod
from kicad_monkey import KiCadPcb, NetRef


_PCB_TEXT = """(kicad_pcb
  (version 20240108)
  (generator "kicad")
  (generator_version "10.0.3")
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal))
  (net 0 "")
  (net 1 "/A")
  (footprint "Device:R" (layer "F.Cu") (at 1 2 0)
    (property "Reference" "R1" (at 0 0 0) (layer "F.SilkS"))
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "/A"))
    (pad "2" smd rect (at 1 0) (size 1 1) (layers "F.Cu") (net 1 "/A")))
  (segment (start 0 0) (end 1 0) (width 0.1) (layer "F.Cu") (net 1) (uuid "seg1"))
  (via (at 1 1) (size 0.4) (drill 0.2) (layers "F.Cu" "B.Cu") (net 1) (uuid "via1"))
)
"""


def test_full_board_builds_net_lookup_maps_once(monkeypatch) -> None:
    calls = 0
    original = pcb_mod._pcb_net_lookup_maps

    def counted(pcb):
        nonlocal calls
        calls += 1
        return original(pcb)

    monkeypatch.setattr(pcb_mod, "_pcb_net_lookup_maps", counted)

    pcb = KiCadPcb.from_string(_PCB_TEXT)

    assert calls == 1
    assert pcb.footprints[0].pads[0].net.name == "/A"
    assert pcb.footprints[0].pads[1].net.name == "/A"
    assert pcb.segments[0].net.name == "/A"
    assert pcb.vias[0].net.name == "/A"


def test_resolve_net_ref_uses_board_net_table_once_per_call(monkeypatch) -> None:
    pcb = KiCadPcb.from_string(_PCB_TEXT)
    calls = 0
    original = pcb.net_name_by_ordinal

    def counted():
        nonlocal calls
        calls += 1
        return original()

    monkeypatch.setattr(pcb, "net_name_by_ordinal", counted)

    resolved = pcb.resolve_net_ref(NetRef(ordinal=1, name=""))
    assert resolved.name == "/A"
    assert calls == 1

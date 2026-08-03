"""
Test L0_043: NetTable snapshot for bulk net resolution

``KiCadPcb.resolve_net_ref`` rebuilds the board's net mapping for every
reference it is handed. That is the right trade for a single lookup and the
wrong one for a whole-board pass, where every pad, track, arc, via and zone
carries a reference: the cost becomes O(elements x nets).

``NetTable`` is the shared snapshot that makes such a pass linear. These tests
pin the part that matters — that resolving through the snapshot returns exactly
what resolving through the board returns — plus the deliberate decision to
expose it as a snapshot rather than a cache hidden on the mutable board.

Exercises:
- ``NetTable`` exported from ``kicad_monkey`` (lazy loader)
- name and ordinal are filled in from either direction
- an unknown ordinal or name leaves the reference untouched
- ``name_of`` tolerates a missing net reference
- board-level ``resolve_net_name`` and the snapshot agree
- a snapshot taken before a net rename does not observe the rename
"""

from __future__ import annotations

from kicad_monkey import KiCadPcb, Net, NetRef, NetTable


def _table() -> NetTable:
    return NetTable.from_name_by_ordinal({0: "", 1: "GND", 2: "VCC"})


class TestNetTableResolution:
    def test_an_ordinal_resolves_to_its_name(self) -> None:
        assert _table().name_of(NetRef(ordinal=2)) == "VCC"

    def test_a_name_resolves_to_its_ordinal(self) -> None:
        resolved = NetRef(name="GND").resolve_against(_table())

        assert resolved.ordinal == 1
        assert resolved.name == "GND"

    def test_an_unknown_ordinal_is_left_alone(self) -> None:
        resolved = NetRef(ordinal=99).resolve_against(_table())

        assert resolved.ordinal == 99
        assert resolved.name == ""

    def test_an_unknown_name_keeps_no_ordinal(self) -> None:
        resolved = NetRef(name="NOT_A_NET").resolve_against(_table())

        assert resolved.ordinal is None
        assert resolved.name == "NOT_A_NET"

    def test_a_missing_reference_resolves_to_an_empty_name(self) -> None:
        assert _table().name_of(None) == ""

    def test_the_unnamed_net_does_not_claim_an_ordinal(self) -> None:
        """Net 0 has an empty name; it must not become a lookup key."""
        assert "" not in _table().ordinal_by_name


class TestNetTableMatchesTheBoard:
    def _board(self) -> KiCadPcb:
        pcb = KiCadPcb()
        pcb.nets = [Net(ordinal=0, name=""), Net(ordinal=1, name="GND"), Net(ordinal=2, name="VCC")]
        return pcb

    def test_the_snapshot_agrees_with_per_call_resolution(self) -> None:
        """The whole point of the snapshot is that it changes nothing but cost."""
        pcb = self._board()
        table = pcb.net_table()

        for reference in (
            NetRef(ordinal=1),
            NetRef(ordinal=2),
            NetRef(name="GND"),
            NetRef(ordinal=99),
            NetRef(),
            None,
        ):
            assert table.name_of(reference) == pcb.resolve_net_name(reference)

    def test_a_snapshot_does_not_observe_a_later_rename(self) -> None:
        """This is why it is a snapshot and not a cache kept on the board.

        A cache hidden behind ``resolve_net_name`` would have to guess when the
        net table changed. Making the lifetime the caller's choice means the
        stale window is visible in the code that opted into it.
        """
        pcb = self._board()
        stale = pcb.net_table()

        pcb.nets[1].name = "GROUND"

        assert stale.name_of(NetRef(ordinal=1)) == "GND"
        assert pcb.resolve_net_name(NetRef(ordinal=1)) == "GROUND"
        assert pcb.net_table().name_of(NetRef(ordinal=1)) == "GROUND"


class TestNetTableIsReadOnly:
    def test_mappings_reject_mutation(self) -> None:
        table = _table()

        try:
            table.name_by_ordinal[1] = "MUTATED"  # type: ignore[index]
            raise AssertionError("name_by_ordinal accepted mutation")
        except TypeError:
            pass

        try:
            table.ordinal_by_name["GND"] = 99  # type: ignore[index]
            raise AssertionError("ordinal_by_name accepted mutation")
        except TypeError:
            pass

        assert table.name_of(NetRef(ordinal=1)) == "GND"
        assert table.name_of(NetRef(name="GND")) == "GND"

    def test_from_name_by_ordinal_copies_caller_input(self) -> None:
        source = {0: "", 1: "GND", 2: "VCC"}
        table = NetTable.from_name_by_ordinal(source)
        source[1] = "MUTATED"

        assert table.name_of(NetRef(ordinal=1)) == "GND"

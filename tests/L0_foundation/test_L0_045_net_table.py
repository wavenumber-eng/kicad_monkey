"""Net-table snapshots for linear bulk board resolution."""

from __future__ import annotations

import pytest

from kicad_monkey import KiCadPcb, Net, NetRef, NetTable


def _table() -> NetTable:
    return NetTable.from_name_by_ordinal({0: "", 1: "GND", 2: "VCC"})


@pytest.mark.parametrize(
    ("reference", "ordinal", "name"),
    [
        (NetRef(ordinal=2), 2, "VCC"),
        (NetRef(name="GND"), 1, "GND"),
        (NetRef(ordinal=99), 99, ""),
        (NetRef(name="NOT_A_NET"), None, "NOT_A_NET"),
        (None, None, ""),
    ],
)
def test_net_table_resolves_like_board_lookup(
    reference: NetRef | None,
    ordinal: int | None,
    name: str,
) -> None:
    resolved = _table().resolve(reference)

    assert resolved.ordinal == ordinal
    assert resolved.name == name
    assert _table().name_of(reference) == name


def test_net_table_is_an_immutable_explicit_snapshot() -> None:
    pcb = KiCadPcb()
    pcb.nets = [Net(ordinal=0, name=""), Net(ordinal=1, name="GND")]
    snapshot = pcb.net_table()

    pcb.nets[1].name = "GROUND"

    assert snapshot.name_of(NetRef(ordinal=1)) == "GND"
    assert pcb.resolve_net_name(NetRef(ordinal=1)) == "GROUND"
    with pytest.raises(TypeError):
        snapshot.name_by_ordinal[1] = "MUTATED"  # type: ignore[index]
    with pytest.raises(TypeError):
        snapshot.ordinal_by_name["MUTATED"] = 1  # type: ignore[index]


def test_net_table_public_constructor_detaches_and_derives_reverse_lookup() -> None:
    names = {0: "", 1: "GND"}
    snapshot = NetTable(names)

    names[1] = "GROUND"
    names[2] = "VCC"

    assert dict(snapshot.name_by_ordinal) == {0: "", 1: "GND"}
    assert dict(snapshot.ordinal_by_name) == {"GND": 1}
    with pytest.raises(TypeError):
        snapshot.name_by_ordinal[1] = "MUTATED"  # type: ignore[index]
    with pytest.raises(TypeError):
        snapshot.ordinal_by_name["MUTATED"] = 1  # type: ignore[index]


def test_net_table_excludes_the_empty_name_from_reverse_lookup() -> None:
    assert "" not in _table().ordinal_by_name

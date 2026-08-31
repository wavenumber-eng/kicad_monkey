"""
Test L0_023: bus-label parsing + expansion (Phase G — Slice N-2).

Pure-unit coverage for :mod:`kicad_monkey.kicad_bus_expansion`. Inputs
are bare strings + a synthetic alias dict; no on-disk fixture needed.

Aligns with KiCad's :func:`NET_SETTINGS::ParseBusVector` /
:func:`ParseBusGroup` (``common/project/net_settings.cpp``):

* Vector ``BUS[a..b]`` → members emitted in **ascending** index order
  regardless of which end is larger (KiCad swaps).
* Suffix may be ``"+" | "-" | "P" | "N"``, applied to every member.
* Group ``{a, b, c}`` separates members on commas or spaces.
* Group prefix prepends to every member — e.g. ``MIX{A,B}`` →
  ``["MIXA", "MIXB"]``.
* Plain text returns as a single-element list.
* Alias references and group-member alias references recurse.
"""

from __future__ import annotations

import pytest

from kicad_monkey import (
    expand_bus_label,
    is_bus_label,
    parse_bus_group,
    parse_bus_vector,
)


# ---------------------------------------------------------------------------
# parse_bus_vector
# ---------------------------------------------------------------------------


def test_vector_descending_swaps_to_ascending():
    """KiCad always emits ascending order — `D[7..0]` → D0..D7."""
    prefix, members = parse_bus_vector("D[7..0]")
    assert prefix == "D"
    assert members == ["D0", "D1", "D2", "D3", "D4", "D5", "D6", "D7"]


def test_vector_ascending_preserved():
    prefix, members = parse_bus_vector("BUS[0..3]")
    assert prefix == "BUS"
    assert members == ["BUS0", "BUS1", "BUS2", "BUS3"]


def test_vector_single_element_range_rejected():
    """`X[3..3]` is illegal in KiCad's parser — returns None."""
    assert parse_bus_vector("X[3..3]") is None


def test_vector_with_plus_suffix_applied_per_member():
    prefix, members = parse_bus_vector("DIFF[0..1]+")
    assert prefix == "DIFF"
    assert members == ["DIFF0+", "DIFF1+"]


def test_vector_with_minus_suffix():
    prefix, members = parse_bus_vector("D[0..1]-")
    assert members == ["D0-", "D1-"]


def test_vector_with_P_suffix():
    prefix, members = parse_bus_vector("DP[0..1]P")
    assert members == ["DP0P", "DP1P"]


def test_vector_with_N_suffix():
    prefix, members = parse_bus_vector("DN[0..1]N")
    assert members == ["DN0N", "DN1N"]


def test_vector_invalid_suffix_rejected():
    """Unrecognised suffix (e.g. `Z`) → not a vector."""
    assert parse_bus_vector("D[0..1]Z") is None


def test_vector_no_prefix_rejected():
    assert parse_bus_vector("[0..3]") is None


def test_vector_missing_close_bracket_rejected():
    assert parse_bus_vector("D[0..3") is None


def test_vector_non_numeric_indices_rejected():
    assert parse_bus_vector("D[A..B]") is None


def test_vector_zero_to_higher_count_correct():
    _, members = parse_bus_vector("V[0..15]")
    assert len(members) == 16
    assert members[0] == "V0"
    assert members[-1] == "V15"


def test_plain_label_is_not_vector():
    assert parse_bus_vector("VCC") is None


def test_group_form_is_not_vector():
    assert parse_bus_vector("{A,B,C}") is None


# ---------------------------------------------------------------------------
# parse_bus_group
# ---------------------------------------------------------------------------


def test_empty_prefix_group_with_commas():
    prefix, members = parse_bus_group("{A,B,C}")
    assert prefix == ""
    assert members == ["A", "B", "C"]


def test_empty_prefix_group_with_spaces():
    prefix, members = parse_bus_group("{A B C}")
    assert prefix == ""
    assert members == ["A", "B", "C"]


def test_group_with_prefix_applied_at_expand_time():
    """`parse_bus_group` returns raw members; prefix concat is at expansion."""
    prefix, members = parse_bus_group("MIX{A,B}")
    assert prefix == "MIX"
    assert members == ["A", "B"]


def test_group_member_with_braces_kept_intact():
    """Nested `{…}` member (e.g. for vector recursion) preserved as-is."""
    prefix, members = parse_bus_group("{D[0..3],CLK}")
    # Whitespace-trimmed by separator; braces preserved.
    assert members == ["D[0..3]", "CLK"]


def test_group_unmatched_open_brace_rejected():
    assert parse_bus_group("{A,B,C") is None


def test_group_with_trailing_garbage_rejected():
    assert parse_bus_group("{A,B}xx") is None


def test_group_no_open_brace_rejected():
    assert parse_bus_group("ABC") is None


def test_group_empty_member_list_rejected():
    assert parse_bus_group("{}") is None


def test_group_mixed_separators():
    """Mix of comma + space separators (KiCad accepts both)."""
    prefix, members = parse_bus_group("{A, B,C  D}")
    assert members == ["A", "B", "C", "D"]


# ---------------------------------------------------------------------------
# is_bus_label
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text",
    [
        "D[7..0]",
        "D[0..7]",
        "D[0..1]+",
        "{A,B,C}",
        "MIX{A,B}",
    ],
)
def test_is_bus_label_recognises_bus_forms(text):
    assert is_bus_label(text)


@pytest.mark.parametrize(
    "text",
    [
        "VCC",
        "/SIG",
        "Net-(R1-1)",
        "GND",
    ],
)
def test_is_bus_label_rejects_plain_nets(text):
    assert not is_bus_label(text)


# ---------------------------------------------------------------------------
# expand_bus_label
# ---------------------------------------------------------------------------


def test_expand_plain_label_returns_singleton():
    assert expand_bus_label("VCC") == ["VCC"]
    assert expand_bus_label("GND", {}) == ["GND"]


def test_expand_vector_label():
    assert expand_bus_label("D[0..3]") == ["D0", "D1", "D2", "D3"]


def test_expand_group_label_no_prefix():
    assert expand_bus_label("{A,B,C}") == ["A", "B", "C"]


def test_expand_group_label_with_prefix():
    """Group prefix prepends to each member with ``.`` separator.

    Matches KiCad's ``SCH_CONNECTION::ConfigureFromLabel`` rule
    (eeschema/sch_connection.cpp): a named bus-group prefix is joined
    to each member with a dot (``prefix += "."``). Unnamed groups
    (``{a,b}``) leave members bare.
    """
    assert expand_bus_label("MIX{A,B}") == ["MIX.A", "MIX.B"]


def test_expand_group_with_vector_member_recurses():
    """Vector inside group expands recursively."""
    assert expand_bus_label("{D[0..2],CLK}") == ["D0", "D1", "D2", "CLK"]


def test_expand_alias_direct_reference():
    """Bare alias name → its members."""
    aliases = {"MEM": ["A0", "A1", "A2"]}
    assert expand_bus_label("MEM", aliases) == ["A0", "A1", "A2"]


def test_expand_alias_with_vector_member_recurses():
    """Alias whose member is a vector — recursively expand."""
    aliases = {"MEM": ["A[0..1]", "WE"]}
    assert expand_bus_label("MEM", aliases) == ["A0", "A1", "WE"]


def test_expand_group_member_is_alias():
    """Alias referenced inside group expands."""
    aliases = {"MEM": ["A0", "A1"]}
    assert expand_bus_label("{MEM,CLK}", aliases) == ["A0", "A1", "CLK"]


def test_expand_alias_chain_recursion():
    """Alias referencing another alias recurses."""
    aliases = {
        "INNER": ["X0", "X1"],
        "OUTER": ["INNER", "Y"],
    }
    assert expand_bus_label("OUTER", aliases) == ["X0", "X1", "Y"]


def test_expand_alias_cycle_fails_closed():
    """Project-derived aliases must not recurse indefinitely when malformed."""
    aliases = {
        "A": ["B"],
        "B": ["C"],
        "C": ["A"],
    }
    with pytest.raises(ValueError, match="bus alias cycle includes 'A'"):
        expand_bus_label("A", aliases)


def test_expand_no_aliases_falls_through_for_unknown_name():
    """Unknown name + no bus syntax → fall through to plain singleton."""
    assert expand_bus_label("UNKNOWN_NAME") == ["UNKNOWN_NAME"]


def test_expand_with_descending_vector_yields_ascending():
    """Confirm `expand_bus_label("D[7..0]")` matches KiCad's ascending output."""
    assert expand_bus_label("D[7..0]") == [
        "D0",
        "D1",
        "D2",
        "D3",
        "D4",
        "D5",
        "D6",
        "D7",
    ]


def test_expand_group_with_suffix_vector_member():
    """Group containing a vector with suffix expands fully."""
    assert expand_bus_label("{D[0..1]+,CLK}") == ["D0+", "D1+", "CLK"]

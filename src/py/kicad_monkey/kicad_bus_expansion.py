"""
Bus-label parsing and expansion.

Ports KiCad's :func:`NET_SETTINGS::ParseBusVector` and
:func:`NET_SETTINGS::ParseBusGroup` (``common/project/net_settings.cpp``)
to Python. The netlist compiler uses these helpers to:

* detect whether a label drives a bus (vs. a single net)
* turn a vector label like ``D[7..0]`` into ``[D0, D1, â€¦, D7]``
* turn a sparse group like ``{D0, D2, D4}`` into ``[D0, D2, D4]``
* expand a bus alias reference into its declared members

Public API:

* :func:`is_bus_label` â€” predicate (matches KiCad's ``IsBusLabel``).
* :func:`parse_bus_vector` â€” ``"D[7..0]"`` â†’ ``("D", ["D0", â€¦ "D7"])``.
* :func:`parse_bus_group`  â€” ``"{a,b,c}"`` â†’ ``("",  ["a", "b", "c"])``.
* :func:`expand_bus_label` â€” top-level dispatch that handles bus aliases
  + recursive group-member expansion. Plain (non-bus) text returns
  ``[text]``.

Quirks faithfully replicated:

* Vector ranges are emitted in **ascending** index order regardless of
  which side is larger (KiCad swaps ``begin > end``).
* Vector suffix may be one of ``"+"``, ``"-"``, ``"P"``, ``"N"`` and
  is appended to every member.
* Group members can themselves be vectors or alias references; we
  recursively expand.

Known unsupported bus-label syntax:

* Formatting markers (``^{â€¦}``, ``_{â€¦}``, ``~{â€¦}``) inside prefixes â€”
  KiCad treats these as part of the signal name; our port keeps them
  intact via the same brace-nesting rules but doesn't strip them.
* Quoted strings inside prefixes / member names â€” wrapper for embedded
  spaces; we treat them as literal characters.
* ``EscapeString`` / ``UnescapeString`` round-trips â€” we keep names
  raw; the netlist emit layer applies its own escaping.
"""

from __future__ import annotations

from typing import Dict, List, Optional, Tuple


def canonical_bus_member_name(text: str) -> str:
    """Normalize bus-member names for connectivity matching.

    KiCad stores literal slashes in bus-alias member declarations but
    schematic labels that carry those members can appear escaped as
    ``{slash}``. Net-name emission still applies CTX_NETNAME escaping;
    this helper is only for equality comparisons while resolving bus
    membership.
    """
    return (text or "").replace("{slash}", "/")


# ---------------------------------------------------------------------------
# Vector parser:  prefix[start..end][suffix]
# ---------------------------------------------------------------------------


_VECTOR_SUFFIX_CHARS = ("+", "-", "P", "N")


def parse_bus_vector(text: str) -> Optional[Tuple[str, List[str]]]:
    """Try to parse ``text`` as a bus-vector label.

    Returns ``(prefix, members)`` on success or ``None`` when ``text``
    does not match the vector grammar. Mirror of
    ``NET_SETTINGS::ParseBusVector``.

    Members are emitted in ascending index order (e.g. ``D[7..0]`` and
    ``D[0..7]`` both yield ``["D0", "D1", â€¦, "D7"]``); KiCad swaps
    ``begin > end`` before iterating.
    """
    n = len(text)
    if n < 4:  # need at least "X[N]"
        return None

    # ---- prefix ----------------------------------------------------------
    i = 0
    prefix_chars: List[str] = []
    brace_depth = 0
    while i < n:
        ch = text[i]
        if ch == "{":
            # KiCad allows {â€¦} only when preceded by a formatting marker
            # (^, _, ~). In the simple grammar we treat any { as illegal
            # for the vector form unless it's nested inside one we're
            # already tracking.
            if i > 0 and text[i - 1] in "^_~":
                brace_depth += 1
                prefix_chars.append(ch)
                i += 1
                continue
            return None
        if ch == "}":
            if brace_depth == 0:
                return None
            brace_depth -= 1
            prefix_chars.append(ch)
            i += 1
            continue
        if ch == " " or ch == "]":
            return None
        if ch == "[":
            break
        prefix_chars.append(ch)
        i += 1
    else:
        return None  # never saw '['

    if brace_depth != 0:
        return None

    prefix = "".join(prefix_chars)
    if not prefix:
        return None

    # ---- start index -----------------------------------------------------
    i += 1  # skip '['
    if i >= n:
        return None
    start_chars: List[str] = []
    while i < n:
        # ".." separator
        if text[i] == "." and i + 1 < n and text[i + 1] == ".":
            i += 2
            break
        if not text[i].isdigit():
            return None
        start_chars.append(text[i])
        i += 1
    else:
        return None
    if not start_chars:
        return None
    begin = int("".join(start_chars))

    # ---- end index -------------------------------------------------------
    end_chars: List[str] = []
    while i < n:
        if text[i] == "]":
            i += 1
            break
        if not text[i].isdigit():
            return None
        end_chars.append(text[i])
        i += 1
    else:
        return None
    if not end_chars:
        return None
    end = int("".join(end_chars))

    # ---- optional suffix (only +, -, P, N or closing brace) --------------
    suffix_chars: List[str] = []
    while i < n:
        ch = text[i]
        if ch == "}":
            if brace_depth == 0:
                # Stray closing brace â€” only legal if vector lives
                # inside outer formatting we already consumed.
                return None
            brace_depth -= 1
            i += 1
            continue
        if ch in _VECTOR_SUFFIX_CHARS:
            suffix_chars.append(ch)
            i += 1
            continue
        return None

    if brace_depth != 0:
        return None

    if begin == end:
        return None
    if begin > end:
        begin, end = end, begin

    suffix = "".join(suffix_chars)
    members = [f"{prefix}{idx}{suffix}" for idx in range(begin, end + 1)]
    return prefix, members


# ---------------------------------------------------------------------------
# Group parser:  prefix{member1,member2,...}
# ---------------------------------------------------------------------------


def parse_bus_group(text: str) -> Optional[Tuple[str, List[str]]]:
    """Try to parse ``text`` as a bus-group label.

    Returns ``(prefix, members)`` on success or ``None`` when ``text``
    does not match. Mirror of ``NET_SETTINGS::ParseBusGroup``.

    The prefix may be empty (``"{a,b,c}"`` â†’ ``("", ["a","b","c"])``).
    Member separators are bare commas or spaces; brace-nested members
    keep their braces intact (so a vector member ``D[1..2]`` stays
    intact for recursive expansion by :func:`expand_bus_label`).
    """
    n = len(text)
    if n < 3:  # need at least "{x}"
        return None

    # ---- prefix ----------------------------------------------------------
    i = 0
    prefix_chars: List[str] = []
    brace_depth = 0
    while i < n:
        ch = text[i]
        if ch == "{":
            # `{` opens member list only when NOT preceded by a
            # formatting marker (^/_/~). Otherwise it's part of the
            # prefix's formatting block.
            if i > 0 and text[i - 1] in "^_~":
                brace_depth += 1
                prefix_chars.append(ch)
                i += 1
                continue
            break  # start of member list
        if ch == "}":
            if brace_depth == 0:
                return None
            brace_depth -= 1
            prefix_chars.append(ch)
            i += 1
            continue
        if ch == " " or ch == "[" or ch == "]":
            return None
        prefix_chars.append(ch)
        i += 1
    else:
        return None  # never saw the opening '{'

    if brace_depth != 0:
        return None

    prefix = "".join(prefix_chars)

    # ---- members ---------------------------------------------------------
    i += 1  # skip '{'
    if i >= n:
        return None

    members: List[str] = []
    cur: List[str] = []
    member_brace_depth = 0
    closed = False

    while i < n:
        ch = text[i]
        if ch == "{":
            # Brace-nested members (formatted names, vectors): keep raw.
            member_brace_depth += 1
            cur.append(ch)
            i += 1
            continue
        if ch == "}":
            if member_brace_depth > 0:
                member_brace_depth -= 1
                cur.append(ch)
                i += 1
                continue
            # End of group.
            if cur:
                members.append("".join(cur))
                cur = []
            closed = True
            i += 1
            break
        if member_brace_depth == 0 and (ch == "," or ch == " "):
            # Separator at top level.
            if cur:
                members.append("".join(cur))
                cur = []
            i += 1
            continue
        cur.append(ch)
        i += 1

    if not closed:
        return None
    if member_brace_depth != 0:
        return None
    # Anything trailing the closing brace is illegal.
    if i != n:
        return None
    if not members:
        return None

    return prefix, members


# ---------------------------------------------------------------------------
# Predicates + top-level expansion
# ---------------------------------------------------------------------------


def is_bus_label(text: str) -> bool:
    """Return True when ``text`` parses as either a vector or a group.

    Matches :func:`SCH_CONNECTION::IsBusLabel`. Note this does NOT cover
    plain bus-alias references â€” KiCad detects those at the
    ``CONNECTION_GRAPH`` level via the alias map. Use
    :func:`expand_bus_label` (with an alias dict) for full bus
    detection in netlist contexts.
    """
    return parse_bus_vector(text) is not None or parse_bus_group(text) is not None


def expand_bus_label(
    text: str,
    bus_aliases: Optional[Dict[str, List[str]]] = None,
) -> List[str]:
    """Return the list of net names a bus label expands to.

    Dispatch order:

    1. Bus alias name â€” if ``text`` is a key in ``bus_aliases``,
       expand to its members (each member is itself recursively
       expanded so a member like ``"D[0..3]"`` yields four nets).
    2. Bus vector â€” ``D[7..0]`` â†’ ``[D0, D1, â€¦, D7]``.
    3. Bus group â€” ``{a, b, c}`` â†’ ``[a, b, c]``, with each member
       recursively expanded.
    4. Plain text â€” single-element list ``[text]`` (the fall-through
       case, so callers can blindly call this on any label).
    """
    return _expand_bus_label(text, bus_aliases or {}, set())


def _expand_bus_label(
    text: str,
    aliases: Dict[str, List[str]],
    active_aliases: set[str],
) -> List[str]:
    """Recursive implementation with explicit alias-cycle detection."""

    # 1. Direct alias reference.
    if text in aliases:
        return _expand_alias(text, aliases, active_aliases)

    # 2. Vector form.
    parsed = parse_bus_vector(text)
    if parsed is not None:
        return parsed[1]

    # 3. Group form (members may recurse â€” alias inside group, vector
    #    inside group, etc.).
    parsed = parse_bus_group(text)
    if parsed is not None:
        return _expand_group(parsed, aliases, active_aliases)

    # 4. Plain net.
    return [text]


def _expand_alias(
    text: str,
    aliases: Dict[str, List[str]],
    active_aliases: set[str],
) -> List[str]:
    if text in active_aliases:
        raise ValueError(f"bus alias cycle includes {text!r}")
    active_aliases.add(text)
    out: List[str] = []
    try:
        for member in aliases[text]:
            out.extend(_expand_bus_label(member, aliases, active_aliases))
        return out
    finally:
        active_aliases.remove(text)


def _expand_group(
    parsed: Tuple[str, List[str]],
    aliases: Dict[str, List[str]],
    active_aliases: set[str],
) -> List[str]:
    prefix, members = parsed
    out: List[str] = []
    for member in members:
        nested = member in aliases or is_bus_label(member)
        expanded = (
            _expand_bus_label(member, aliases, active_aliases) if nested else [member]
        )
        out.extend(f"{prefix}.{name}" if prefix else name for name in expanded)
    return out


__all__ = [
    "canonical_bus_member_name",
    "is_bus_label",
    "parse_bus_vector",
    "parse_bus_group",
    "expand_bus_label",
]

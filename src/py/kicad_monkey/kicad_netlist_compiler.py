"""
Single-sheet netlist compiler.

Walks one :class:`~kicad_monkey.KiCadSchematic` and emits the connectivity
sub-graphs plus a per-sheet :class:`~kicad_monkey.KiCadNetlist` populated with
resolved nets.

Pipeline (mirrors KiCad's ``CONNECTION_GRAPH::buildConnectionGraph``):

1. Build a :class:`ConnectivityGraph` over wires + buses + bus entries +
   junctions.
2. Index every potential **driver** (label, global label, hier label,
   sheet pin, regular pin, power pin) by its world-space coord.
3. Walk components — group drivers by which connected component their
   coord belongs to.
4. For each component, pick the highest-priority driver per
   :class:`KiCadDriverPriority` (KiCad's ``compareDrivers``). Tie-break
   alphabetically on driver name; falls back to insertion order.
5. Name the resulting net per the KiCad rules:
   * GLOBAL label  → ``/<text>``
   * GLOBAL_POWER_PIN  → bare ``<symbol-value>`` (e.g. ``GND``)
   * LOCAL_POWER_PIN   → bare ``<symbol-value>``
   * LOCAL_LABEL  → ``<sheet_path><text>``
   * HIER_LABEL   → ``<sheet_path><text>``
   * SHEET_PIN    → ``<sheet_path><text>``
   * PIN / NONE   → auto-generated ``Net-(<ref>-<pin>)``

The design-level compiler performs cross-sheet hierarchy, global-label, and
power-symbol merges across these single-sheet results.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Dict, Iterable, List, Optional, Set, Tuple

from .kicad_bus_connectivity import (
    build_bus_subgraphs,
    collect_bus_aliases,
    merge_bus_member_taps_within_sheet,
)
from .kicad_netlist_model import (
    KiCadDriverKind,
    KiCadDriverPriority,
    KiCadNet,
    KiCadNetEndpoint,
    KiCadNetlist,
    KiCadNetlistTerminal,
)
from .kicad_schematic_connectivity import (
    ConnectivityGraph,
    CoordKey,
    detect_no_connects,
    iter_symbol_pins,
    snap_mm_to_iu,
)
from .kicad_schematic_ids import schematic_pin_group_id, schematic_sheet_pin_group_id
from .kicad_schematic_to_ir import placed_symbol_pin_has_drawing

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .kicad_lib_symbol import LibSymbol
    from .kicad_sch_symbol import SchSymbol
    from .kicad_schematic import KiCadSchematic
    from .kicad_sym_pin import SymPin


_GRAPHICAL_KEYS = (
    "wires",
    "junctions",
    "labels",
    "power_ports",
    "ports",
    "sheet_entries",
)


def _empty_graphical_map() -> Dict[str, List[str]]:
    return {key: [] for key in _GRAPHICAL_KEYS}


def _graphical_map_copy(graphical: Dict[str, List[str]]) -> Dict[str, List[str]]:
    return {key: list(graphical.get(key, ())) for key in _GRAPHICAL_KEYS}


def _add_graphical_id(
    graphical: Dict[str, List[str]],
    bucket: str,
    svg_id: str,
) -> None:
    if not svg_id:
        return
    values = graphical.setdefault(bucket, [])
    if svg_id not in values:
        values.append(svg_id)


def _endpoint_id(
    role: str,
    *,
    object_id: str,
    element_id: str,
    name: str,
    coord: CoordKey,
) -> str:
    stable_id = object_id or element_id
    if stable_id:
        return f"{role}:{stable_id}"
    return f"{role}:{name}:{coord[0]}:{coord[1]}"


def _label_driver_endpoint(
    driver: "_LabelDriver",
    *,
    source_sheet: str,
) -> Optional[KiCadNetEndpoint]:
    role_by_kind = {
        KiCadDriverKind.HIER_LABEL: "port",
        KiCadDriverKind.SHEET_PIN: "sheet_entry",
    }
    role = role_by_kind.get(driver.kind)
    if role is None:
        return None

    element_id = driver.svg_uuid or driver.source_uuid
    object_id = driver.source_uuid or element_id
    return KiCadNetEndpoint(
        endpoint_id=_endpoint_id(
            role,
            object_id=object_id,
            element_id=element_id,
            name=driver.name,
            coord=driver.coord,
        ),
        role=role,
        element_id=element_id,
        object_id=object_id,
        name=driver.name,
        source_sheet=source_sheet,
        connection_point=driver.coord,
    )


def _power_pin_endpoint(
    driver: "_PinDriver",
    *,
    source_sheet: str,
) -> Optional[KiCadNetEndpoint]:
    if not (driver.is_power and driver.designator.startswith("#")):
        return None

    role = "power_port"
    element_id = driver.svg_uuid or driver.source_uuid
    object_id = driver.source_uuid or element_id
    name = driver.power_value or driver.pin_name or driver.name
    return KiCadNetEndpoint(
        endpoint_id=_endpoint_id(
            role,
            object_id=object_id,
            element_id=element_id,
            name=name,
            coord=driver.coord,
        ),
        role=role,
        element_id=element_id,
        object_id=object_id,
        name=name,
        source_sheet=source_sheet,
        connection_point=driver.coord,
    )


def _append_unique_endpoint(
    net: KiCadNet, endpoint: Optional[KiCadNetEndpoint]
) -> None:
    if endpoint is None or not endpoint.endpoint_id:
        return
    key = (endpoint.endpoint_id, endpoint.role, endpoint.source_sheet)
    for existing in net.endpoints:
        if (existing.endpoint_id, existing.role, existing.source_sheet) == key:
            return
    net.add_endpoint(endpoint)


# ---------------------------------------------------------------------------
# Driver-candidate dataclasses
# ---------------------------------------------------------------------------


@dataclass
class _PinDriver:
    """A regular component pin — priority depends on power-symbol status."""

    designator: str
    pin_number: str
    pin_name: str
    pin_type: str
    coord: CoordKey
    priority: KiCadDriverPriority
    is_power: bool
    power_value: str = ""  # symbol-value for power symbols; "" otherwise
    # ``has_multiple`` mirrors KiCad's ``SCH_PIN::GetDefaultNetName``
    # rule: True iff another pin on the same parent symbol shares this
    # pin's shown name (with a different shown number, and matching
    # NC-status). When True, KiCad appends ``-Pad<number>`` to the
    # auto-name suffix to disambiguate.
    has_multiple: bool = False
    # Unit-suffixed reference (e.g. ``IC1A`` for unit 1 of a multi-unit
    # symbol). KiCad's ``SCH_PIN::GetDefaultNetName`` uses this in the
    # "high-quality" branch — when the pin's shown name is non-empty
    # and != its shown number, the net name must carry the unit token
    # since pin names aren't unique across units. Equals
    # :attr:`designator` for single-unit symbols.
    designator_with_unit: str = ""
    # Number of active pins on the parent symbol for this sheet/unit.
    # KiCad's connection graph treats isolated one-pin component symbols
    # as forced no-connects even when that single pin has a visible name;
    # multi-pin symbols can still produce high-quality ``Net-(ref-name)``
    # auto names for isolated named pins.
    parent_pin_count: int = 0
    # True for hidden ``power_in`` pins on ordinary symbols. KiCad lets
    # these implicitly join a power net by pin name, but an explicit
    # power symbol on the same merged net wins the naming tiebreak.
    is_implicit_hidden_power: bool = False
    # Source/render identity for graphical linkage. ``svg_uuid`` is the
    # parent symbol group; ``pin_svg_uuid`` is the visible pin group when
    # a placed pin is actually rendered.
    source_uuid: str = ""
    svg_uuid: str = ""
    pin_svg_uuid: str = ""

    @property
    def name(self) -> str:
        # Used only for tie-breaking — alphabetical on the pin's
        # canonical (ref, pin) string.
        return f"{self.designator}-{self.pin_number}"


@dataclass
class _LabelDriver:
    """A label / global label / hier label / sheet pin.

    ``shape`` carries the label-flag shape (``"input"`` / ``"output"`` /
    etc.) when meaningful — hier labels and sheet pins have it; plain
    local/global labels leave it empty. KiCad's ``compareDrivers``
    rule 4 (``connection_graph.cpp:193-206``) uses it to prefer the
    ``L_OUTPUT`` sheet pin when two ``SCH_SHEET_PIN_T`` drivers tie on
    priority, name-independent.
    """

    text: str
    coord: CoordKey
    priority: KiCadDriverPriority
    kind: KiCadDriverKind
    shape: str = ""
    # Source/render identity for schematic-viz net highlighting. Most
    # label-like objects render as their own SVG group keyed by UUID.
    # Sheet pins render as nested groups under the parent sheet, so
    # ``svg_uuid`` is that sheet-pin group while ``source_uuid`` remains
    # the pin UUID.
    source_uuid: str = ""
    svg_uuid: str = ""
    # Schematic discovery order. This keeps duplicate weak sheet-pin
    # suffixing stable when final net materialisation order differs from
    # schematic driver discovery order.
    source_order: int = 0
    # Optional sheet path to use when this driver wins net naming.
    # Single-sheet compilation fills this with the current sheet path;
    # design-level compilation may override sheet-pin naming for special
    # hierarchy cases such as off-board child sheets.
    sheet_path: str = ""

    @property
    def name(self) -> str:
        return self.text


# ---------------------------------------------------------------------------
# Subgraph — a connected component of coords + its driver candidates
# ---------------------------------------------------------------------------


@dataclass
class Subgraph:
    """One connected component on a sheet — pre-naming.

    * ``coords`` — every snapped coord key in this component (wires,
      junctions, bus-entry endpoints, pin coords).
    * ``pin_drivers`` — every pin candidate landing on a coord.
    * ``label_drivers`` — every label / sheet-pin candidate landing on
      a coord.
    * ``chosen_priority`` — :class:`KiCadDriverPriority` of the picked
      driver (``NONE`` when no driver was found).
    * ``chosen_kind`` — symbolic kind string (matches
      :class:`KiCadDriverKind`).
    * ``chosen_name`` — raw driver text (label text, power-symbol
      value, or the formatted ``ref-pin`` for the auto-name fallback).
    * ``no_connect`` — ``True`` when any coord in the component carries
      a ``no_connect`` marker.
    """

    coords: Set[CoordKey] = field(default_factory=set)
    pin_drivers: List[_PinDriver] = field(default_factory=list)
    label_drivers: List[_LabelDriver] = field(default_factory=list)
    graphical: Dict[str, List[str]] = field(default_factory=_empty_graphical_map)
    chosen_priority: KiCadDriverPriority = KiCadDriverPriority.NONE
    chosen_kind: KiCadDriverKind = KiCadDriverKind.NONE
    chosen_name: str = ""
    chosen_sheet_path: str = ""
    no_connect: bool = False


# ---------------------------------------------------------------------------
# Driver resolution
# ---------------------------------------------------------------------------


def _resolve_driver(sg: Subgraph) -> None:
    """Pick the highest-priority driver candidate.

    Mirrors KiCad's ``compareDrivers`` ordering. Ties are broken
    alphabetically on the driver's display name; further ties (same
    name, same priority) fall back to insertion order — the first
    candidate seen wins. This matches the C++ ``std::stable_sort`` +
    priority compare.
    """
    candidates: List[
        Tuple[
            KiCadDriverPriority,
            int,
            str,
            KiCadDriverKind,
            str,
            int,
            str,
        ]
    ] = []

    # Label-style drivers
    for idx, ld in enumerate(sg.label_drivers):
        candidates.append(
            (ld.priority, 0, ld.name, ld.kind, ld.name, idx, ld.sheet_path)
        )

    # Pin-style drivers — only contribute if no higher-priority label
    # exists (priority comparison handles this naturally).
    pin_offset = len(sg.label_drivers)
    for idx, pd in enumerate(sg.pin_drivers):
        # Power symbols pull their net name from the symbol value, not the pin number.
        display = pd.power_value if pd.is_power and pd.power_value else pd.name
        implicit_rank = 1 if pd.is_implicit_hidden_power else 0
        candidates.append(
            (
                pd.priority,
                implicit_rank,
                display,
                _pin_kind(pd.priority),
                display,
                pin_offset + idx,
                "",
            )
        )

    if not candidates:
        sg.chosen_priority = KiCadDriverPriority.NONE
        sg.chosen_kind = KiCadDriverKind.NONE
        sg.chosen_name = ""
        sg.chosen_sheet_path = ""
        return

    # Sort by (-priority, implicit-hidden-power rank, name, insertion_idx)
    # — highest priority wins, explicit power symbols beat implicit
    # hidden-power pins, then alphabetical ties and insertion order.
    candidates.sort(key=lambda t: (-int(t[0]), t[1], t[2], t[5]))
    best = candidates[0]
    sg.chosen_priority = KiCadDriverPriority(best[0])
    sg.chosen_kind = best[3]
    sg.chosen_name = best[4]
    sg.chosen_sheet_path = best[6] or ""


def _pin_kind(priority: KiCadDriverPriority) -> KiCadDriverKind:
    if priority == KiCadDriverPriority.GLOBAL_POWER_PIN:
        return KiCadDriverKind.GLOBAL_POWER_PIN
    if priority == KiCadDriverPriority.LOCAL_POWER_PIN:
        return KiCadDriverKind.LOCAL_POWER_PIN
    return KiCadDriverKind.PIN


# ---------------------------------------------------------------------------
# Net naming
# ---------------------------------------------------------------------------


def _escape_netname(s: str) -> str:
    """Mirror KiCad's ``EscapeString(..., CTX_NETNAME)`` for net-name segments.

    Only ``/`` and bare ``\\n``/``\\r`` are special: ``/`` → ``{slash}``,
    newlines are dropped. Everything else passes through. See
    ``common/string_utils.cpp::EscapeString``.
    """
    if not s:
        return s
    return s.replace("\r", "").replace("\n", "").replace("/", "{slash}")


def _normalize_netlist_pin_number(pin_number: str) -> str:
    """Normalize KiCad's hidden/blank pin-number sentinel for netlist output."""
    return "" if pin_number == "~" else pin_number


def name_net(sg: Subgraph, sheet_path: str = "/") -> Tuple[str, bool]:
    """Return ``(net_name, auto_named)`` for a resolved subgraph.

    ``sheet_path`` is the canonical sheet path (e.g. ``"/"`` for root,
    ``"/sub/"`` for a child sheet) — must end with a ``/``.
    """
    if not sheet_path.endswith("/"):
        sheet_path = sheet_path + "/"

    kind = sg.chosen_kind

    if kind == KiCadDriverKind.GLOBAL_LABEL:
        # kicad-cli's netlist exporter emits global-label net names
        # without a leading sheet-path prefix (parity with power
        # symbols). Local/hier labels keep the prefix; globals don't.
        return _escape_netname(sg.chosen_name), False

    if kind in (KiCadDriverKind.GLOBAL_POWER_PIN, KiCadDriverKind.LOCAL_POWER_PIN):
        return _escape_netname(sg.chosen_name), False

    if kind in (
        KiCadDriverKind.LOCAL_LABEL,
        KiCadDriverKind.HIER_LABEL,
        KiCadDriverKind.SHEET_PIN,
    ):
        # Sheet-path segments are already escaped at build time; the
        # label text is the un-escaped raw author string, so apply the
        # ``CTX_NETNAME`` escape (``/`` → ``{slash}``) here. Mirrors
        # KiCad's ``SCH_CONNECTION::ConfigureFromLabel`` /
        # ``Name()`` which composes pre-escaped name segments.
        scoped_sheet_path = sg.chosen_sheet_path or sheet_path
        if not scoped_sheet_path.endswith("/"):
            scoped_sheet_path = scoped_sheet_path + "/"
        return f"{scoped_sheet_path}{_escape_netname(sg.chosen_name)}", False

    # No driver / pure pin — auto-name from the best pin candidate.
    #
    # Suffix + prefix follow KiCad's ``SCH_PIN::GetDefaultNetName``
    # (``eeschema/sch_pin.cpp``):
    #
    # * ``unconnected = (pin.type == NC) or aForceNoConnect``. KiCad's
    #   ``CONNECTION_GRAPH`` forces unconnected when a subgraph has
    #   no strong driver and only a single pin driver — i.e. an
    #   isolated pin.
    # * If the pin name is empty / ``"~"`` / equals its number → suffix
    #   is bare ``Pad<number>`` (a "low quality" name in KiCad's
    #   tiebreak).
    # * Otherwise suffix is the pin name. If unconnected, append
    #   ``-Pad<number>`` to disambiguate.
    #
    # Pin tiebreak (``compareDrivers`` in connection_graph.cpp):
    # high-quality (no "-Pad") names win over low-quality ones; ties
    # break alphabetically on the full pin-name segment, then on
    # insertion order.
    if sg.pin_drivers:
        is_isolated = len(sg.pin_drivers) == 1
        single_pin = sg.pin_drivers[0] if is_isolated else None
        single_pin_type = single_pin.pin_type if single_pin is not None else ""
        isolated_one_pin_symbol = (
            single_pin is not None
            and int(getattr(single_pin, "parent_pin_count", 0) or 0) == 1
        )
        weak_isolated = is_isolated and (
            single_pin_type in ("passive", "unspecified")
            or "no_connect" in single_pin_type
            or isolated_one_pin_symbol
        )
        # ``sg.no_connect`` flags subgraph-level NC markers. Explicit
        # NC pins, weak isolated pins, and isolated one-pin symbols flip
        # the prefix to ``unconnected-``; visible named pins on multi-pin
        # symbols can keep KiCad's normal ``Net-(ref-pinname)`` form.
        sg_unconnected = sg.no_connect or weak_isolated

        def _pin_suffix(pd: "_PinDriver", forced_unconn: bool) -> Tuple[str, bool]:
            """Return (suffix, is_unconnected) per KiCad rules.

            Mirrors ``SCH_PIN::GetDefaultNetName`` in
            ``eeschema/sch_pin.cpp``:

            * Empty / ``"~"`` / name-equals-number → bare
              ``Pad<num>`` (low-quality name).
            * Otherwise ``<name>``; append ``-Pad<num>`` when the pin
              is unconnected (NC or isolated) **or** when another pin
              on the same symbol shares this shown name (``has_multiple``).
            * Pin names and pad numbers are escaped with
              ``CTX_NETNAME`` rules (``/`` → ``{slash}``).
            """
            unconn = forced_unconn or pd.pin_type == "no_connect"
            pname = pd.pin_name or ""
            pad = _escape_netname(_normalize_netlist_pin_number(pd.pin_number))
            if pname in ("", "~") or pname == pd.pin_number:
                return f"Pad{pad}", unconn
            ename = _escape_netname(pname)
            if unconn or pd.has_multiple:
                return f"{ename}-Pad{pad}", unconn
            return ename, unconn

        # Score each pin: (low_quality_flag, sort_key, designator, pin_number, idx)
        # — low_quality_flag=False (no "-Pad") sorts before True. Sort key is
        # the suffix itself for alphabetical comparison.
        #
        # Each candidate also tracks ``ref`` — the reference designator
        # to emit in the final ``Net-(<ref>-<suffix>)``. KiCad uses the
        # unit-suffixed ref (e.g. ``IC1A``) when the pin name is
        # non-empty and != its number (the "high-quality" branch); the
        # bare ref otherwise.
        candidates = []
        # KiCad's auto-name skips pins on ``#``-prefixed symbols
        # (power symbols, PWR_FLAGs). Their pins exist in the graph
        # for connectivity but never name a net — only real-component
        # pins drive ``Net-(<ref>-<suffix>)`` synthesis. Mirrors the
        # ``writeListOfNets`` ``#``-prefix filter at emit time, but
        # applied earlier here so the chosen name doesn't reference a
        # symbol that won't appear as a terminal.
        eligible = [pd for pd in sg.pin_drivers if not pd.designator.startswith("#")]
        if not eligible:
            eligible = list(sg.pin_drivers)
        for idx, pd in enumerate(eligible):
            suffix, unconn = _pin_suffix(pd, sg_unconnected)
            bare_pad = suffix.startswith("Pad")
            low_quality = "-Pad" in suffix or bare_pad
            ref = (
                pd.designator
                if bare_pad
                else (pd.designator_with_unit or pd.designator)
            )
            full_segment = f"{ref}-{suffix}"
            candidates.append(
                (
                    low_quality,
                    full_segment,
                    ref,
                    pd.pin_number,
                    idx,
                    pd,
                    suffix,
                    unconn,
                    ref,
                )
            )
        candidates.sort(key=lambda t: (t[0], t[1], t[2], t[3], t[4]))
        best = candidates[0]
        suffix = best[6]
        is_unconnected = best[7]
        ref = best[8]

        prefix = "unconnected-(" if is_unconnected else "Net-("
        return f"{prefix}{ref}-{suffix})", True

    # Truly empty subgraph — generate a synthetic placeholder. This
    # happens only when callers feed a subgraph with no pins and no
    # labels (e.g. a free-floating wire stub).
    return "unconnected", True


# ---------------------------------------------------------------------------
# Driver collection — turn parsed schematic items into _PinDriver / _LabelDriver
# ---------------------------------------------------------------------------


def _is_power_symbol(symbol: "SchSymbol", lib_symbol: Optional["LibSymbol"]) -> bool:
    """Heuristic: a symbol is a power symbol when its lib is ``power:`` or
    ``LibSymbol.power == True``. Either path matches KiCad's
    ``LIB_SYMBOL::IsPower`` semantics.

    ``power:PWR_FLAG`` is explicitly excluded — PWR_FLAG carries the
    ``(power)`` token but is an ERC-only flag (no net-naming role).
    Letting it drive would alphabetically out-rank a real ``+5V``/
    ``GND`` peer on the same coord and collapse unrelated power nets
    when the cross-sheet ``power_value`` merge later groups every
    subgraph that mentions ``"PWR_FLAG"``.
    """
    if symbol.lib_id == "power:PWR_FLAG":
        return False
    if lib_symbol is not None and getattr(lib_symbol, "power", False):
        return True
    return symbol.lib_id.startswith("power:")


def _power_symbol_kind(lib_symbol: Optional["LibSymbol"]) -> KiCadDriverPriority:
    """Distinguish global vs. local power symbols.

    KiCad 10 added a ``power_kind`` token (``"global"`` / ``"local"``);
    older corpora omit it and default to global. Mirror that.
    """
    if lib_symbol is None:
        return KiCadDriverPriority.GLOBAL_POWER_PIN
    kind = getattr(lib_symbol, "power_kind", None)
    if kind == "local":
        return KiCadDriverPriority.LOCAL_POWER_PIN
    return KiCadDriverPriority.GLOBAL_POWER_PIN


def _resolve_instance_unit(
    sym,
    sheet_path: str,
    legacy_unit_lookup: Optional[Dict[str, int]] = None,
    canonical_path: Optional[str] = None,
) -> int:
    """Resolve the per-sheet-path unit for *sym*.

    Mirrors :func:`_resolve_instance_reference`'s resolution chain but
    returns the unit number. KiCad stores unit overrides per-instance:
    legacy fixtures keep them in ``(symbol_instances …)`` on the top
    schematic, modern fixtures in each symbol's ``(instances …)`` block.
    Placements with no ``(unit …)`` token in the schematic body default
    to 1, so for multi-instance multi-unit placements (a single
    sub-sheet placed twice on the same parent, with each instance
    carrying a different unit assignment) the only authoritative source
    is the instance lookup.
    """
    instances = getattr(sym, "instances", None) or []

    if instances:
        if canonical_path:
            cp = canonical_path.rstrip("/")
            for inst in instances:
                if inst.path.rstrip("/") == cp:
                    return int(getattr(inst, "unit", 1) or 1)

        target = sheet_path.rstrip("/")
        for inst in instances:
            if inst.path.rstrip("/") == target:
                return int(getattr(inst, "unit", 1) or 1)
        if target:
            for inst in instances:
                ipath = inst.path.rstrip("/")
                if ipath and (ipath.endswith(target) or target.endswith(ipath)):
                    return int(getattr(inst, "unit", 1) or 1)

    if legacy_unit_lookup:
        sym_uuid = getattr(sym, "uuid", "") or ""
        if sym_uuid:
            key = f"{sheet_path.rstrip('/')}/{sym_uuid}"
            unit = legacy_unit_lookup.get(key)
            if unit:
                return int(unit)

    if instances:
        return int(getattr(instances[0], "unit", 1) or 1)
    return int(getattr(sym, "unit", 1) or 1)


def _resolve_instance_reference(
    sym,
    sheet_path: str,
    legacy_lookup: Optional[Dict[str, str]] = None,
    canonical_path: Optional[str] = None,
) -> str:
    """Resolve the per-sheet-path reference for *sym*.

    KiCad stores per-instance reference overrides in two places:

    * Modern (post-20210126) — per-symbol ``(instances …)`` block,
      surfaced as :attr:`SchSymbol.instances`. Path keys are sheet paths
      in UUID form *including the top schematic's own UUID* as the leading
      segment (e.g. ``/<top>/<child>``).
    * Legacy (≤ 20201015) — schematic-level ``(symbol_instances …)``
      block on the *top* schematic, with path keys ``/<sheet>/<symbol>``
      (i.e. the symbol's own UUID forms the final segment). Stored on
      :attr:`KiCadSchematic.symbol_instances`. The caller is expected to
      flatten this into ``legacy_lookup`` keyed by ``inst.path.rstrip("/")``.

    ``canonical_path`` (when provided) is the design walker's authoritative
    modern instance path for the symbol on this sheet — built from the top
    schematic's own UUID prepended to ``cs.sheet_path``. When supplied this
    is the *preferred* match key; the suffix heuristic is only a fall-back
    for standalone-loaded sub-sheets where ``canonical_path`` may be wrong.

    Resolution order:

    1. modern exact match against ``canonical_path`` (when supplied),
    2. modern exact ``inst.path == sheet_path`` (trailing-``/`` normalised),
    3. modern suffix-match — ``inst.path`` is the trailing suffix of
       ``sheet_path`` (standalone sub-sheet load),
    4. legacy lookup keyed by ``f"{sheet_path}{sym.uuid}"``,
    5. fall back to the first modern instance entry,
    6. last-resort fall back to ``sym.reference``.
    """
    instances = getattr(sym, "instances", None) or []

    if instances:
        if canonical_path:
            cp = canonical_path.rstrip("/")
            for inst in instances:
                if inst.path.rstrip("/") == cp:
                    return inst.reference or sym.reference

        target = sheet_path.rstrip("/")
        for inst in instances:
            if inst.path.rstrip("/") == target:
                return inst.reference or sym.reference
        if target:
            for inst in instances:
                ipath = inst.path.rstrip("/")
                if ipath and (ipath.endswith(target) or target.endswith(ipath)):
                    return inst.reference or sym.reference

    if legacy_lookup:
        sym_uuid = getattr(sym, "uuid", "") or ""
        if sym_uuid:
            key = f"{sheet_path.rstrip('/')}/{sym_uuid}"
            ref = legacy_lookup.get(key)
            if ref:
                return ref

    if instances:
        return instances[0].reference or sym.reference
    return sym.reference


def _parse_alpha_numeric_pin(pin_num: str) -> Tuple[str, int]:
    """Mirror ``ParseAlphaNumericPin`` from ``common/string_utils.cpp``.

    Splits a pin number into a non-numeric prefix and a trailing
    integer (``-1`` when no numeric suffix). E.g. ``"A12"`` →
    ``("A", 12)``, ``"5"`` → ``("", 5)``, ``"foo"`` → ``("foo", -1)``.
    """
    num_start = len(pin_num)
    for i in range(len(pin_num) - 1, -1, -1):
        if not pin_num[i].isdigit():
            num_start = i + 1
            break
        if i == 0:
            num_start = 0
    if num_start < len(pin_num):
        try:
            return pin_num[:num_start], int(pin_num[num_start:])
        except ValueError:
            return pin_num, -1
    return pin_num, -1


def _expand_stacked_pin_notation(pin_name: str) -> Tuple[List[str], bool]:
    """Mirror ``ExpandStackedPinNotation`` from ``common/string_utils.cpp``.

    KiCad supports a "stacked pin" notation where a single library pin
    represents several physical pads, e.g. ``"[2,4]"`` or
    ``"[A1-A4, B7]"``. Returns ``(expanded_list, valid)``: when the
    notation is malformed or the input is a plain number, the list
    holds the input verbatim and ``valid`` reflects whether the input
    was syntactically clean.
    """
    has_open = "[" in pin_name
    has_close = "]" in pin_name
    if has_open or has_close:
        if not (pin_name.startswith("[") and pin_name.endswith("]")):
            return [pin_name], False
    if not (pin_name.startswith("[") and pin_name.endswith("]")):
        return [pin_name], True

    inner = pin_name[1:-1]
    expanded: List[str] = []
    for raw_part in inner.split(","):
        part = raw_part.strip()
        if not part:
            continue
        if "-" in part:
            start_txt, _, end_txt = part.partition("-")
            start_txt, end_txt = start_txt.strip(), end_txt.strip()
            sp, sv = _parse_alpha_numeric_pin(start_txt)
            ep, ev = _parse_alpha_numeric_pin(end_txt)
            if sp != ep or sv == -1 or ev == -1 or sv > ev:
                return [pin_name], False
            for ii in range(sv, ev + 1):
                expanded.append(f"{sp}{ii}" if sp else f"{ii}")
        else:
            expanded.append(part)
    return expanded, True


def _letter_sub_reference(unit: int, initial_letter: int = ord("A")) -> str:
    """Mirror ``LIB_SYMBOL::LetterSubReference`` from eeschema/lib_symbol.cpp.

    Converts a 1-based unit index into a letter suffix using
    ``initial_letter`` as the ASCII base. Beyond 26, prepends additional
    letters (so unit 27 → ``AA``). ``initial_letter`` is typically ord('A')
    (uppercase, KiCad default) or ord('a') (lowercase).
    """
    if unit < 1:
        return ""
    suffix = ""
    while unit > 0:
        u = (unit - 1) % 26
        suffix = chr(initial_letter + u) + suffix
        unit = (unit - u) // 26
    return suffix


def _subpart_reference(
    unit: int,
    subpart_first_id: int,
    subpart_id_separator: int,
    add_separator: bool = False,
) -> str:
    """Mirror ``SCHEMATIC_SETTINGS::SubReference`` from
    ``eeschema/schematic_settings.cpp``.

    Returns the unit token (e.g. ``"A"``, ``"-A"``, ``"1"``).
    ``subpart_first_id`` selects letters ('A'/'a') or digits ('1');
    ``subpart_id_separator`` is the ASCII code of an optional
    separator (``0`` = no separator).
    """
    if unit < 1:
        return ""
    sub = ""
    if subpart_id_separator != 0 and add_separator:
        sub = chr(subpart_id_separator)
    if ord("0") <= subpart_first_id <= ord("9"):
        sub += str(unit)
    else:
        sub += _letter_sub_reference(unit, subpart_first_id)
    return sub


def _collect_pin_drivers(
    schematic: "KiCadSchematic",
    cgraph: ConnectivityGraph,
    sheet_path: str = "/",
    legacy_lookup: Optional[Dict[str, str]] = None,
    canonical_path: Optional[str] = None,
    subpart_first_id: int = ord("A"),
    subpart_id_separator: int = 0,
    legacy_unit_lookup: Optional[Dict[str, int]] = None,
    include_off_board_symbols: bool = False,
) -> List[_PinDriver]:
    """For every placed symbol, transform each lib pin to world coords
    and emit a :class:`_PinDriver` candidate. Pins are also seeded into
    the connectivity graph as solo nodes so the component-walk picks
    them up even when no wire lands on them (KiCad allows pin-to-pin
    direct contact).

    ``sheet_path`` is forwarded to :func:`_resolve_instance_reference`
    so the per-instance designator lands on the resulting terminal —
    repeated sub-sheet placements need their own per-placement ref.
    """
    out: List[_PinDriver] = []
    virtual_hidden_nc_seq = 0

    for symbol in schematic.symbols:
        if not include_off_board_symbols and not getattr(symbol, "on_board", True):
            continue
        if hasattr(schematic, "get_lib_symbol_for_symbol"):
            lib_symbol = schematic.get_lib_symbol_for_symbol(symbol)
        else:
            lib_symbol = schematic.get_lib_symbol(symbol.lib_id)
        if lib_symbol is None:
            # Without the lib definition we can't compute pin coords —
            # this single-sheet pass records no diagnostic and skips it.
            continue

        is_power = _is_power_symbol(symbol, lib_symbol)
        power_priority = (
            _power_symbol_kind(lib_symbol) if is_power else KiCadDriverPriority.PIN
        )
        power_value = symbol.value if is_power else ""

        designator = _resolve_instance_reference(
            symbol,
            sheet_path,
            legacy_lookup,
            canonical_path,
        )

        # Multi-unit suffix: only meaningful for symbols whose lib def
        # declares >1 unit. ``add_separator=True`` mirrors KiCad's
        # ``GetRef(path, true)`` which always asks for the separator;
        # when ``subpart_id_separator == 0`` the helper returns the
        # bare letter (e.g. ``A``).
        #
        # Unit must be resolved per sheet-path (mirrors KiCad's
        # ``SCH_SYMBOL::GetUnit(path)``) — legacy schematics omit
        # ``(unit N)`` from the symbol body and only stamp the unit
        # via the top-level ``(symbol_instances …)`` block, so a
        # single sub-sheet placed twice with different units would
        # otherwise read as unit 1 for both placements.
        unit = _resolve_instance_unit(
            symbol,
            sheet_path,
            legacy_unit_lookup,
            canonical_path,
        )
        unit_count = int(getattr(lib_symbol, "unit_count", 1) or 1)
        if unit_count > 1:
            unit_suffix = _subpart_reference(
                unit,
                subpart_first_id,
                subpart_id_separator,
                add_separator=True,
            )
            designator_with_unit = f"{designator}{unit_suffix}"
        else:
            designator_with_unit = designator

        # Materialise the symbol's full pin set first so we can compute
        # ``has_multiple`` per pin — KiCad's ``GetDefaultNetName`` walks
        # every pin on the parent symbol to detect duplicate shown names.
        # ``unit_override`` ensures pin selection follows the per-sheet
        # resolved unit (not the schematic body's literal ``unit=1`` for
        # legacy fixtures whose unit lives in ``(symbol_instances …)``).
        symbol_pins = list(iter_symbol_pins(symbol, lib_symbol, unit_override=unit))
        placed_pin_uuid_by_number = {
            str(getattr(pin, "number", "") or ""): str(getattr(pin, "uuid", "") or "")
            for pin in getattr(symbol, "pins", ()) or ()
        }

        materialized_pins: List[Tuple[str, float, float, "SymPin", CoordKey]] = []
        pin_coords_by_number: Dict[str, List[CoordKey]] = {}
        for pin_number, wx, wy, lib_pin in symbol_pins:
            ptype = _pin_type_to_string(lib_pin.electrical_type)
            if ptype == "no_connect" and getattr(lib_pin, "hide", False):
                base = snap_mm_to_iu(wx, wy)
                # KiCad's symbol libraries often park many hidden NC pins
                # at one dummy coordinate. They still export as separate
                # unconnected nets, so do not let the shared coordinate
                # collapse them into one subgraph.
                coord = cgraph.add_key_node(
                    (
                        base[0] - 1_000_000_000_000 - virtual_hidden_nc_seq,
                        base[1],
                    )
                )
                virtual_hidden_nc_seq += 1
            else:
                coord = cgraph.add_node(wx, wy)
            materialized_pins.append((pin_number, wx, wy, lib_pin, coord))
            pin_coords_by_number.setdefault(pin_number, []).append(coord)

        def _union_pin_coords(coords: List[CoordKey]) -> None:
            if len(coords) < 2:
                return
            base = coords[0]
            for other in coords[1:]:
                cgraph.union(base, other)

        # KiCad 9/10 jumper-symbol semantics:
        # * ``duplicate_pin_numbers_are_jumpers`` internally connects all
        #   pins on this symbol placement that share the same pin number.
        # * ``jumper_pin_groups`` internally connects every listed pin
        #   number in the group. If a listed number is duplicated, all
        #   physical pins with that number participate.
        if getattr(lib_symbol, "duplicate_pin_numbers_are_jumpers", False):
            for coords in pin_coords_by_number.values():
                _union_pin_coords(coords)

        for group in getattr(lib_symbol, "jumper_pin_groups", ()):
            coords: List[CoordKey] = []
            for pin_number in group:
                coords.extend(pin_coords_by_number.get(str(pin_number), ()))
            _union_pin_coords(coords)

        # has_multiple: True iff another pin on the same symbol shares
        # this pin's shown name (with a different number, and matching
        # NC-status). Mirrors KiCad's ``SCH_PIN::GetDefaultNetName`` —
        # the comparison iterates **every** pin on the parent symbol,
        # including pins whose shown name happens to equal their shown
        # number (those still contribute to other pins' has_multiple).
        nc_str = "no_connect"
        pin_index_by_name: Dict[str, List[Tuple[str, str]]] = {}
        for pin_number, _wx, _wy, lib_pin in symbol_pins:
            pname = lib_pin.name or ""
            ptype = _pin_type_to_string(lib_pin.electrical_type)
            pin_index_by_name.setdefault(pname, []).append((pin_number, ptype))

        for pin_number, wx, wy, lib_pin, coord in materialized_pins:
            ptype = _pin_type_to_string(lib_pin.electrical_type)
            pname = lib_pin.name or ""
            has_multiple = False
            entries = pin_index_by_name.get(pname)
            if entries is not None:
                self_is_nc = ptype == nc_str
                for other_num, other_type in entries:
                    if other_num == pin_number:
                        continue
                    other_is_nc = other_type == nc_str
                    if self_is_nc == other_is_nc:
                        has_multiple = True
                        break

            # Stacked-pin expansion: a lib pin numbered ``"[2,4]"`` or
            # ``"[A1-A4]"`` represents several physical pads at the
            # same coord. KiCad's netlist exporter
            # (netlist_exporter_base.cpp::CreatePinList) walks
            # ``GetStackedPinNumbers`` and emits one terminal per
            # expanded number, transforming the pin name to
            # ``<base>_<num>`` (or just ``<num>`` when base is empty).
            expanded_numbers, _valid = _expand_stacked_pin_notation(pin_number)
            is_stacked = len(expanded_numbers) > 1 or (
                expanded_numbers and expanded_numbers[0] != pin_number
            )
            # Per-pin power classification mirrors KiCad's
            # ``SCH_PIN::IsGlobalPower`` / ``IsLocalPower``
            # (``eeschema/sch_pin.cpp:439-462``): a pin is a power
            # driver iff its electrical type is ``power_in`` AND its
            # parent symbol is a power symbol. PWR_FLAG carries a
            # ``power_out`` pin so it never drives a power net —
            # without this gate, PWR_FLAG instances would all merge
            # together by ``power_value`` and collapse unrelated
            # power rails (+12V, +12C, GND, HT, …) into one giant
            # net via the cross-sheet ``power_value`` merge.
            pin_is_power = bool(is_power and ptype == "power_in")
            pin_priority = power_priority if pin_is_power else KiCadDriverPriority.PIN
            pin_power_value = power_value if pin_is_power else ""
            pin_is_implicit_hidden_power = False

            # Hidden ``power_in`` pin auto-merge: KiCad treats every
            # hidden ``power_in`` pin on a NON-power symbol as an
            # implicit global power pin whose net is named after the
            # pin's shown name (CONNECTION_GRAPH injects a virtual
            # SCH_LABEL at the pin coord during graph build). Combined
            # with the cross-sheet ``power_value`` merge, every hidden
            # ``VCC`` / ``GND`` / ``+5V`` pin across the design joins
            # the global net of that name, including with visible
            # power symbols of the same value via wire-connected
            # co-located drivers on any sheet.
            if (
                not is_power
                and ptype == "power_in"
                and getattr(lib_pin, "hide", False)
                and pname
                and pname != "~"
                and pname != pin_number
            ):
                pin_is_power = True
                pin_priority = KiCadDriverPriority.GLOBAL_POWER_PIN
                pin_power_value = pname
                pin_is_implicit_hidden_power = True

            source_pin_uuid = placed_pin_uuid_by_number.get(str(pin_number), "")
            if not placed_symbol_pin_has_drawing(symbol, lib_symbol, lib_pin):
                # Hidden pins and pins whose geometry/text both collapse have
                # no independently rendered drawing element.
                # Keep that absence explicit so semantic graph consumers do not
                # mistake the parent symbol group for a pin-level selector.
                pin_svg_uuid = ""
            else:
                pin_svg_uuid = (
                    schematic_pin_group_id(
                        symbol_uuid=getattr(symbol, "uuid", "") or "",
                        pin_number=str(pin_number),
                        source_pin_uuid=source_pin_uuid,
                    )
                    or getattr(symbol, "uuid", "")
                    or ""
                )
            for expanded_num in expanded_numbers:
                if is_stacked:
                    expanded_pname = (
                        f"{pname}_{expanded_num}" if pname else expanded_num
                    )
                else:
                    expanded_pname = lib_pin.name
                out.append(
                    _PinDriver(
                        designator=designator,
                        pin_number=_normalize_netlist_pin_number(expanded_num),
                        pin_name=expanded_pname,
                        pin_type=ptype,
                        coord=coord,
                        priority=pin_priority,
                        is_power=pin_is_power,
                        power_value=pin_power_value,
                        has_multiple=has_multiple,
                        designator_with_unit=designator_with_unit,
                        parent_pin_count=len(symbol_pins),
                        is_implicit_hidden_power=pin_is_implicit_hidden_power,
                        source_uuid=source_pin_uuid,
                        svg_uuid=getattr(symbol, "uuid", "") or "",
                        pin_svg_uuid=pin_svg_uuid,
                    )
                )

    return out


def _pin_type_to_string(electrical_type) -> str:
    """Map :class:`PinElectricalType` → kicad-cli pintype string."""
    # Avoid hard-coding the enum dependency — fall back to the value's
    # string (PinElectricalType uses lowercase token values matching
    # KiCad's own emit).
    val = getattr(electrical_type, "value", str(electrical_type))
    return str(val)


def _collect_label_drivers(
    schematic: "KiCadSchematic",
    cgraph: ConnectivityGraph,
    sheet_path: str = "/",
) -> List[_LabelDriver]:
    """Emit one driver per label / global label / hier label / sheet pin.

    Sheet pins live on :class:`SchSheet` placeholders on the parent
    sheet — every pin contributes a SHEET_PIN-priority driver at the
    pin's parent-sheet coord. The design-level merge pairs each sheet pin
    with the matching ``hierarchical_label`` inside the child schematic.
    """
    out: List[_LabelDriver] = []
    source_order = 0

    def _next_source_order() -> int:
        nonlocal source_order
        order = source_order
        source_order += 1
        return order

    for label in getattr(schematic, "labels", ()):
        coord = cgraph.add_node(label.at_x, label.at_y)
        out.append(
            _LabelDriver(
                text=label.text,
                coord=coord,
                priority=KiCadDriverPriority.LOCAL_LABEL,
                kind=KiCadDriverKind.LOCAL_LABEL,
                source_uuid=getattr(label, "uuid", "") or "",
                svg_uuid=getattr(label, "uuid", "") or "",
                source_order=_next_source_order(),
                sheet_path=sheet_path,
            )
        )

    for label in getattr(schematic, "global_labels", ()):
        coord = cgraph.add_node(label.at_x, label.at_y)
        out.append(
            _LabelDriver(
                text=label.text,
                coord=coord,
                priority=KiCadDriverPriority.GLOBAL,
                kind=KiCadDriverKind.GLOBAL_LABEL,
                source_uuid=getattr(label, "uuid", "") or "",
                svg_uuid=getattr(label, "uuid", "") or "",
                source_order=_next_source_order(),
                sheet_path=sheet_path,
            )
        )

    for label in getattr(schematic, "hierarchical_labels", ()):
        coord = cgraph.add_node(label.at_x, label.at_y)
        shape = getattr(getattr(label, "shape", None), "value", "") or ""
        out.append(
            _LabelDriver(
                text=label.text,
                coord=coord,
                priority=KiCadDriverPriority.HIER_LABEL,
                kind=KiCadDriverKind.HIER_LABEL,
                shape=str(shape),
                source_uuid=getattr(label, "uuid", "") or "",
                svg_uuid=getattr(label, "uuid", "") or "",
                source_order=_next_source_order(),
                sheet_path=sheet_path,
            )
        )

    # Sheet pins on hierarchical sheet placements.
    for sheet in getattr(schematic, "sheets", ()):
        for pin in getattr(sheet, "pins", ()):
            coord = cgraph.add_node(pin.at_x, pin.at_y)
            shape = getattr(getattr(pin, "shape", None), "value", "") or ""
            sheet_pin_svg_uuid = (
                schematic_sheet_pin_group_id(
                    sheet_uuid=getattr(sheet, "uuid", "") or "",
                    pin_name=getattr(pin, "name", "") or "",
                    source_pin_uuid=getattr(pin, "uuid", "") or "",
                )
                or getattr(sheet, "uuid", "")
                or ""
            )
            out.append(
                _LabelDriver(
                    text=pin.name,
                    coord=coord,
                    priority=KiCadDriverPriority.SHEET_PIN,
                    kind=KiCadDriverKind.SHEET_PIN,
                    shape=str(shape),
                    source_uuid=getattr(pin, "uuid", "") or "",
                    svg_uuid=sheet_pin_svg_uuid,
                    source_order=_next_source_order(),
                    sheet_path=sheet_path,
                )
            )

    return out


# ---------------------------------------------------------------------------
# Sheet-level compile
# ---------------------------------------------------------------------------


def _segments_from_polyline(
    points: List[Tuple[float, float]],
) -> List[Tuple[CoordKey, CoordKey]]:
    """Return the snapped-coord segments of a polyline (consecutive pairs)."""
    segs: List[Tuple[CoordKey, CoordKey]] = []
    prev: Optional[CoordKey] = None
    for x, y in points:
        cur = snap_mm_to_iu(float(x), float(y))
        if prev is not None and prev != cur:
            segs.append((prev, cur))
        prev = cur
    return segs


def _point_on_segment(
    p: CoordKey,
    a: CoordKey,
    b: CoordKey,
) -> bool:
    """True iff the integer-IU point ``p`` lies on the closed segment ``a-b``.

    Uses exact integer cross-product + bounding-box checks — no
    floating-point. Endpoints are inclusive (``p == a`` or ``p == b``
    returns True), matching KiCad's ``SEG::Contains`` semantics.
    """
    px, py = p
    ax, ay = a
    bx, by = b
    # Collinearity: 2D cross product of (b-a) and (p-a) must be zero.
    cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax)
    if cross != 0:
        return False
    # Bounding box: p between a and b on both axes.
    if px < min(ax, bx) or px > max(ax, bx):
        return False
    if py < min(ay, by) or py > max(ay, by):
        return False
    return True


def _attach_drivers_to_segments(
    cgraph: ConnectivityGraph,
    driver_coords: Iterable[CoordKey],
    segments: Iterable[Tuple[CoordKey, CoordKey]],
) -> None:
    """Union every driver coord into any wire/bus segment it lies on.

    KiCad's ``CONNECTION_GRAPH`` treats labels and pins as point-on-
    segment, not just point-on-vertex — a label landing in the middle of
    a horizontal wire is on the same net as the wire's endpoints. This
    helper closes that gap by walking driver coords against every
    segment and unioning into the segment's endpoints when collinear.
    """
    seg_list = list(segments)
    for p in driver_coords:
        for a, b in seg_list:
            if p == a or p == b:
                continue  # Already an endpoint — union via add_node.
            if _point_on_segment(p, a, b):
                cgraph.union(p, a)
                cgraph.union(p, b)
                # Don't break — a coord can lie on multiple overlapping
                # segments (T-junctions that aren't junction-marked).


def compile_sheet_subgraphs(
    schematic: "KiCadSchematic",
    sheet_path: str = "/",
    legacy_lookup: Optional[Dict[str, str]] = None,
    canonical_path: Optional[str] = None,
    bus_aliases: Optional[Dict[str, List[str]]] = None,
    subpart_first_id: int = ord("A"),
    subpart_id_separator: int = 0,
    legacy_unit_lookup: Optional[Dict[str, int]] = None,
    include_off_board_symbols: bool = False,
) -> List[Subgraph]:
    """Single-sheet compile — return resolved :class:`Subgraph` list.

    Output order is deterministic: components are iterated from the
    connectivity graph and sorted by (chosen-name, lowest-coord-key) for
    stability across runs.
    """
    cgraph = ConnectivityGraph()

    # Collect wire / bus segments for later point-on-segment attach.
    segments: List[Tuple[CoordKey, CoordKey]] = []
    wire_segments: List[Tuple[CoordKey, CoordKey]] = []

    # Wires + buses contribute edges; bus entries contribute their
    # diagonal; junctions register as nodes.
    for wire in getattr(schematic, "wires", ()):
        cgraph.add_wire(wire)
        wire_segs = _segments_from_polyline(list(wire.points))
        wire_segments.extend(wire_segs)
        segments.extend(wire_segs)
    for bus in getattr(schematic, "buses", ()):
        cgraph.add_bus(bus)
        segments.extend(_segments_from_polyline(list(bus.points)))
    for entry in getattr(schematic, "bus_entries", ()):
        a, b = cgraph.add_bus_entry(entry)
        segments.append((a, b))
    junction_coords = cgraph.add_junctions(getattr(schematic, "junctions", ()))
    _attach_drivers_to_segments(cgraph, junction_coords, wire_segments)

    # Drivers (after coord nodes are seeded so component walk picks them up).
    pin_drivers = _collect_pin_drivers(
        schematic,
        cgraph,
        sheet_path,
        legacy_lookup,
        canonical_path,
        subpart_first_id=subpart_first_id,
        subpart_id_separator=subpart_id_separator,
        legacy_unit_lookup=legacy_unit_lookup,
        include_off_board_symbols=include_off_board_symbols,
    )
    label_drivers = _collect_label_drivers(schematic, cgraph, sheet_path=sheet_path)

    # Attach LABEL coords (only) to any wire segments they lie on.
    # KiCad's CONNECTION_GRAPH registers pins at their single exact
    # GetPosition() coord — pins never participate in point-on-segment
    # matching. Doing so for pins causes hidden NC pins lying mid-wire
    # (e.g. ICs with explicit no_connect on internal pads) to merge
    # into the wire's subgraph and corrupt net assignment.
    label_coords = [ld.coord for ld in label_drivers]
    _attach_drivers_to_segments(cgraph, label_coords, segments)

    # Same-sheet name-equality merge for LOCAL_LABEL, HIER_LABEL, and
    # power-symbol driver values.
    # KiCad's CONNECTION_GRAPH (eeschema/connection_graph.cpp
    # collectAllDriverValues) caches subgraphs under
    # ``m_local_label_cache[(sheet, name)]`` keyed by ``name`` only —
    # LOCAL_LABEL / HIER_LABEL subgraphs and same-named power symbols
    # are unified on the same sheet regardless of kind. So a wire
    # carrying local label ``DDR3_~{RESET}``, a separate wire carrying
    # hier-label ``DDR3_~{RESET}``, and a same-sheet power symbol of
    # that value all collapse to one subgraph.
    by_label_text: Dict[str, CoordKey] = {}

    def _merge_same_sheet_driver(text: str, coord: CoordKey) -> None:
        if not text:
            return
        first = by_label_text.get(text)
        if first is None:
            by_label_text[text] = coord
        else:
            cgraph.union(first, coord)

    for ld in label_drivers:
        if ld.kind not in (KiCadDriverKind.LOCAL_LABEL, KiCadDriverKind.HIER_LABEL):
            continue
        _merge_same_sheet_driver(ld.text, ld.coord)
    for pd in pin_drivers:
        if pd.is_power and pd.power_value:
            _merge_same_sheet_driver(pd.power_value, pd.coord)

    # Within-sheet bus member merge — buses live in a separate
    # connectivity domain, but two wire-stubs tapping the same bus
    # member must collapse to one net. Build the bus subgraphs (with
    # member expansion via the design's bus-aliases) and union the
    # matching wire-UF roots in ``cgraph`` before component grouping.
    aliases = bus_aliases if bus_aliases is not None else collect_bus_aliases(schematic)
    bus_subgraphs = build_bus_subgraphs(schematic, aliases)
    merge_bus_member_taps_within_sheet(cgraph, bus_subgraphs, label_drivers)

    # Group drivers by their component root.
    pin_by_root: Dict[CoordKey, List[_PinDriver]] = {}
    label_by_root: Dict[CoordKey, List[_LabelDriver]] = {}

    for pd in pin_drivers:
        root = cgraph.find(pd.coord)
        pin_by_root.setdefault(root, []).append(pd)
    for ld in label_drivers:
        root = cgraph.find(ld.coord)
        label_by_root.setdefault(root, []).append(ld)

    graphical_by_root: Dict[CoordKey, Dict[str, List[str]]] = {}

    def _graphical_for(root: CoordKey) -> Dict[str, List[str]]:
        return graphical_by_root.setdefault(root, _empty_graphical_map())

    for wire in getattr(schematic, "wires", ()):
        points = list(getattr(wire, "points", ()) or ())
        if not points:
            continue
        root = cgraph.find(snap_mm_to_iu(float(points[0][0]), float(points[0][1])))
        _add_graphical_id(
            _graphical_for(root), "wires", getattr(wire, "uuid", "") or ""
        )

    for junction in getattr(schematic, "junctions", ()):
        coord = snap_mm_to_iu(float(junction.at_x), float(junction.at_y))
        root = cgraph.find(coord)
        _add_graphical_id(
            _graphical_for(root),
            "junctions",
            getattr(junction, "uuid", "") or "",
        )

    for ld in label_drivers:
        root = cgraph.find(ld.coord)
        if ld.kind == KiCadDriverKind.SHEET_PIN:
            _add_graphical_id(
                _graphical_for(root),
                "sheet_entries",
                ld.svg_uuid or ld.source_uuid,
            )
        elif ld.kind == KiCadDriverKind.HIER_LABEL:
            _add_graphical_id(
                _graphical_for(root), "ports", ld.svg_uuid or ld.source_uuid
            )
        elif ld.kind in (KiCadDriverKind.LOCAL_LABEL, KiCadDriverKind.GLOBAL_LABEL):
            _add_graphical_id(
                _graphical_for(root), "labels", ld.svg_uuid or ld.source_uuid
            )

    for pd in pin_drivers:
        if not (pd.is_power and pd.designator.startswith("#")):
            continue
        root = cgraph.find(pd.coord)
        _add_graphical_id(
            _graphical_for(root), "power_ports", pd.svg_uuid or pd.source_uuid
        )

    nc_coords = detect_no_connects(schematic)

    # Build subgraphs from every component touched by a driver.
    subgraphs: List[Subgraph] = []
    seen_roots: Set[CoordKey] = set()

    all_components = cgraph.components()
    # Map root-of-each-component to the full coord set (root may differ
    # across find() calls due to path compression, so use any key per
    # component as a proxy and re-root via find()).
    for comp_coords in all_components:
        any_coord = next(iter(comp_coords))
        root = cgraph.find(any_coord)
        if root in seen_roots:
            continue
        seen_roots.add(root)

        sg = Subgraph(
            coords=set(comp_coords),
            pin_drivers=list(pin_by_root.get(root, ())),
            label_drivers=list(label_by_root.get(root, ())),
            graphical=_graphical_map_copy(graphical_by_root.get(root, {})),
            no_connect=any(c in nc_coords for c in comp_coords),
        )
        _resolve_driver(sg)
        subgraphs.append(sg)

    # Stable order: by chosen_name, then by lexically smallest coord.
    subgraphs.sort(
        key=lambda s: (s.chosen_name or "", min(s.coords) if s.coords else (0, 0))
    )
    return subgraphs


def _uniquify_duplicate_net_names(nets: List[KiCadNet]) -> None:
    """Apply KiCad-style ``_N`` suffixes to repeated weak sheet-pin nets."""
    seen: Dict[str, int] = {}
    for net in nets:
        if net.driver_kind != str(KiCadDriverKind.SHEET_PIN):
            continue
        base = net.name
        count = seen.get(base, 0)
        if count:
            net.name = f"{base}_{count}"
        seen[base] = count + 1


def compile_sheet_netlist(
    schematic: "KiCadSchematic",
    sheet_path: str = "/",
    *,
    code_offset: int = 1,
) -> KiCadNetlist:
    """Convenience wrapper — compile + naming + materialise into a
    :class:`KiCadNetlist`.

    Subgraphs with no drivers AND no pin candidates (free-floating
    wire stubs) are dropped — they wouldn't appear in kicad-cli output
    either.
    """
    subgraphs = compile_sheet_subgraphs(schematic, sheet_path=sheet_path)
    netlist = KiCadNetlist()

    code = code_offset
    for sg in subgraphs:
        if not sg.pin_drivers and not sg.label_drivers:
            # Floating wire — KiCad doesn't emit this either.
            continue

        net_name, auto_named = name_net(sg, sheet_path=sheet_path)
        net = KiCadNet(
            name=net_name,
            code=code,
            driver_priority=int(sg.chosen_priority),
            driver_kind=str(sg.chosen_kind),
            auto_named=auto_named,
            graphical=_graphical_map_copy(sg.graphical),
        )

        # Terminals: every pin driver becomes a node on the net (sorted
        # by (designator, pin_number) for determinism).
        ordered_pins = sorted(
            sg.pin_drivers, key=lambda p: (p.designator, p.pin_number)
        )
        for pd in ordered_pins:
            if not pd.designator:
                # Power symbols sometimes have empty references in
                # incomplete fixtures — we still emit but with the
                # placeholder. KiCad does the same.
                continue
            net.add_terminal(
                KiCadNetlistTerminal(
                    designator=pd.designator,
                    pin=pd.pin_number,
                    pin_name=pd.pin_name,
                    pin_type=pd.pin_type,
                    sheet_path=sheet_path,
                    source_pin_id=pd.source_uuid,
                    svg_id=pd.pin_svg_uuid or pd.svg_uuid,
                )
            )

        for ld in sg.label_drivers:
            _append_unique_endpoint(
                net,
                _label_driver_endpoint(ld, source_sheet=sheet_path),
            )
        for pd in sg.pin_drivers:
            _append_unique_endpoint(
                net,
                _power_pin_endpoint(pd, source_sheet=sheet_path),
            )

        netlist.nets.append(net)
        code += 1

    _uniquify_duplicate_net_names(netlist.nets)
    return netlist


__all__ = [
    "Subgraph",
    "compile_sheet_subgraphs",
    "compile_sheet_netlist",
    "name_net",
]

"""
Bus connectivity and member-level cross-tap merging.

Buses in KiCad are a *separate* connectivity domain from wires. A bus
carries multiple named signals (members) and is named by a bus-form
label/hier_label/sheet_pin (e.g. ``D[0..7]``, ``{SCL,SDA}``,
``Foo{Bus1}``). Wires tap into a bus via :class:`SchBusEntry` and pick
up the bus's chosen-member name via a local label on the wire stub.

This module builds, for each sheet:

* :class:`BusSubgraph` records â€” one per physically-connected bus on
  the sheet, with its drivers + tapped wire-side coords.
* A coord lookup so other compilers can ask "is this coord on a bus,
  and if so which one?".

It also exposes :func:`merge_bus_member_taps_within_sheet`, which
unions wire union-find roots that tap the *same* bus member name. This
is the within-sheet equivalent of KiCad's
``CONNECTION_GRAPH::propagateToNeighbors`` for bus members:
two wire stubs labeled ``ROW0`` and physically connected only through
a bus collapse to one net once the bus's chosen name expands to
include ``ROW0`` as a member.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import (
    TYPE_CHECKING,
    Dict,
    Iterable,
    List,
    Optional,
    Set,
    Tuple,
)

from .kicad_bus_expansion import (
    canonical_bus_member_name,
    expand_bus_label,
    is_bus_label,
)
from .kicad_netlist_model import KiCadDriverKind, KiCadDriverPriority
from .kicad_schematic_connectivity import (
    ConnectivityGraph,
    CoordKey,
    snap_mm_to_iu,
)

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .kicad_schematic import KiCadSchematic


# ---------------------------------------------------------------------------
# Driver record â€” narrowed copy of compiler._LabelDriver to avoid an
# import cycle.
# ---------------------------------------------------------------------------


@dataclass
class BusDriver:
    """A label-style driver that lands on bus coords."""

    text: str
    coord: CoordKey
    priority: KiCadDriverPriority
    kind: KiCadDriverKind


@dataclass
class BusSubgraph:
    """One physically-connected bus on a sheet.

    * ``coords`` â€” every snapped coord key that lies on a bus segment
      (including bus-entry bus-side endpoints).
    * ``drivers`` â€” labels / hier-labels / sheet-pins / global-labels
      whose coord falls on the bus.
    * ``tap_wire_coords`` â€” wire-side endpoint of each bus_entry on this
      bus. Other compilers map these to wire-UF roots to figure out
      which wires tap which member.
    * ``chosen_name`` â€” the bus's resolved name (bus expression),
      picked by ``compareDrivers`` priority + alphabetical tiebreak
      across the bus drivers. Empty when no bus-form driver was found.
    * ``chosen_priority`` / ``chosen_kind`` â€” provenance of the chosen
      driver, used by the cross-sheet merge.
    * ``members`` â€” the ordered list of expanded member names per
      :func:`expand_bus_label`. Empty when no chosen-name.
    """

    coords: Set[CoordKey] = field(default_factory=set)
    drivers: List[BusDriver] = field(default_factory=list)
    tap_wire_coords: List[CoordKey] = field(default_factory=list)
    chosen_name: str = ""
    chosen_priority: KiCadDriverPriority = KiCadDriverPriority.NONE
    chosen_kind: KiCadDriverKind = KiCadDriverKind.NONE
    members: List[str] = field(default_factory=list)


@dataclass
class _CoordUnionFind:
    parent: Dict[CoordKey, CoordKey] = field(default_factory=dict)

    def make(self, key: CoordKey) -> None:
        if key not in self.parent:
            self.parent[key] = key

    def find(self, key: CoordKey) -> CoordKey:
        self.make(key)
        while self.parent[key] != key:
            self.parent[key] = self.parent[self.parent[key]]
            key = self.parent[key]
        return key

    def union(self, a: CoordKey, b: CoordKey) -> None:
        root_a, root_b = self.find(a), self.find(b)
        if root_a != root_b:
            self.parent[root_b] = root_a


# ---------------------------------------------------------------------------
# Point-on-segment helper (duplicates the integer cross-product test in
# kicad_netlist_compiler to avoid an import cycle).
# ---------------------------------------------------------------------------


def _point_on_segment(p: CoordKey, a: CoordKey, b: CoordKey) -> bool:
    px, py = p
    ax, ay = a
    bx, by = b
    cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax)
    if cross != 0:
        return False
    if px < min(ax, bx) or px > max(ax, bx):
        return False
    if py < min(ay, by) or py > max(ay, by):
        return False
    return True


# ---------------------------------------------------------------------------
# Bus-alias dict helpers
# ---------------------------------------------------------------------------


def collect_bus_aliases(schematic: "KiCadSchematic") -> Dict[str, List[str]]:
    """Flatten ``schematic.bus_aliases`` into a ``{name: [members]}`` dict.

    Returns an empty dict when the schematic has no aliases.
    """
    out: Dict[str, List[str]] = {}
    for alias in getattr(schematic, "bus_aliases", ()) or ():
        name = getattr(alias, "name", "") or ""
        members = list(getattr(alias, "members", ()) or [])
        if name:
            out[name] = members
    return out


def _collect_bus_segments(
    schematic: "KiCadSchematic",
    bus_uf: _CoordUnionFind,
) -> List[Tuple[CoordKey, CoordKey]]:
    bus_segments: List[Tuple[CoordKey, CoordKey]] = []
    for bus in getattr(schematic, "buses", ()) or ():
        prev: Optional[CoordKey] = None
        for x_mm, y_mm in bus.points:
            cur = snap_mm_to_iu(float(x_mm), float(y_mm))
            bus_uf.make(cur)
            if prev is not None and prev != cur:
                bus_uf.union(prev, cur)
                bus_segments.append((prev, cur))
            prev = cur
    return bus_segments


def _collect_wire_segments(schematic: "KiCadSchematic") -> List[Tuple[CoordKey, CoordKey]]:
    wire_segments: List[Tuple[CoordKey, CoordKey]] = []
    for wire in getattr(schematic, "wires", ()) or ():
        prev = None
        for x_mm, y_mm in wire.points:
            cur = snap_mm_to_iu(float(x_mm), float(y_mm))
            if prev is not None and prev != cur:
                wire_segments.append((prev, cur))
            prev = cur
    return wire_segments


def _classify_bus_entry_tap(
    a: CoordKey,
    b: CoordKey,
    bus_segments: List[Tuple[CoordKey, CoordKey]],
    wire_segments: List[Tuple[CoordKey, CoordKey]],
) -> Tuple[CoordKey, CoordKey]:
    a_on_bus = any(_point_on_segment(a, p, q) for p, q in bus_segments)
    b_on_bus = any(_point_on_segment(b, p, q) for p, q in bus_segments)
    a_on_wire = any(_point_on_segment(a, p, q) for p, q in wire_segments)
    b_on_wire = any(_point_on_segment(b, p, q) for p, q in wire_segments)

    if a_on_bus and not b_on_bus:
        return a, b
    if b_on_bus and not a_on_bus:
        return b, a
    if a_on_bus and b_on_bus:
        if a_on_wire and not b_on_wire:
            return b, a
        return a, b
    return b, a


def _collect_bus_entry_taps(
    schematic: "KiCadSchematic",
    bus_segments: List[Tuple[CoordKey, CoordKey]],
    wire_segments: List[Tuple[CoordKey, CoordKey]],
    bus_uf: _CoordUnionFind,
) -> List[Tuple[CoordKey, CoordKey]]:
    taps: List[Tuple[CoordKey, CoordKey]] = []
    for entry in getattr(schematic, "bus_entries", ()) or ():
        a = snap_mm_to_iu(entry.at_x, entry.at_y)
        b = snap_mm_to_iu(entry.at_x + entry.size_x, entry.at_y + entry.size_y)
        bus_side, wire_side = _classify_bus_entry_tap(a, b, bus_segments, wire_segments)
        bus_uf.make(bus_side)
        for p, q in bus_segments:
            if _point_on_segment(bus_side, p, q):
                bus_uf.union(bus_side, p)
                break
        taps.append((bus_side, wire_side))
    return taps


def _group_bus_coords(bus_uf: _CoordUnionFind) -> Dict[CoordKey, Set[CoordKey]]:
    bus_groups: Dict[CoordKey, Set[CoordKey]] = {}
    for key in bus_uf.parent:
        bus_groups.setdefault(bus_uf.find(key), set()).add(key)
    return bus_groups


def _group_bus_segments_by_root(
    bus_segments: List[Tuple[CoordKey, CoordKey]],
    bus_uf: _CoordUnionFind,
) -> Dict[CoordKey, List[Tuple[CoordKey, CoordKey]]]:
    segs_by_root: Dict[CoordKey, List[Tuple[CoordKey, CoordKey]]] = {}
    for a, b in bus_segments:
        segs_by_root.setdefault(bus_uf.find(a), []).append((a, b))
    return segs_by_root


def _build_bus_subgraph_records(
    bus_groups: Dict[CoordKey, Set[CoordKey]],
    bus_entry_taps: List[Tuple[CoordKey, CoordKey]],
    bus_uf: _CoordUnionFind,
) -> tuple[List[BusSubgraph], Dict[CoordKey, int]]:
    root_to_idx: Dict[CoordKey, int] = {}
    out: List[BusSubgraph] = []
    for root, coords in bus_groups.items():
        root_to_idx[root] = len(out)
        out.append(BusSubgraph(coords=set(coords)))

    for bus_side, wire_side in bus_entry_taps:
        root = bus_uf.find(bus_side)
        out[root_to_idx[root]].tap_wire_coords.append(wire_side)
    return out, root_to_idx


def _find_bus_subgraph_index(
    coord: CoordKey,
    bus_uf: _CoordUnionFind,
    root_to_idx: Dict[CoordKey, int],
    segs_by_root: Dict[CoordKey, List[Tuple[CoordKey, CoordKey]]],
) -> Optional[int]:
    if coord in bus_uf.parent:
        return root_to_idx[bus_uf.find(coord)]
    for root, segs in segs_by_root.items():
        for p, q in segs:
            if _point_on_segment(coord, p, q):
                return root_to_idx[root]
    return None


def _attach_bus_driver(
    driver: BusDriver,
    out: List[BusSubgraph],
    orphans: List[BusDriver],
    bus_aliases: Dict[str, List[str]],
    *,
    bus_uf: _CoordUnionFind,
    root_to_idx: Dict[CoordKey, int],
    segs_by_root: Dict[CoordKey, List[Tuple[CoordKey, CoordKey]]],
) -> None:
    idx = _find_bus_subgraph_index(driver.coord, bus_uf, root_to_idx, segs_by_root)
    if idx is not None:
        out[idx].drivers.append(driver)
    elif is_bus_label(driver.text) or driver.text in bus_aliases:
        orphans.append(driver)


def _attach_schematic_bus_drivers(
    schematic: "KiCadSchematic",
    out: List[BusSubgraph],
    bus_aliases: Dict[str, List[str]],
    *,
    bus_uf: _CoordUnionFind,
    root_to_idx: Dict[CoordKey, int],
    segs_by_root: Dict[CoordKey, List[Tuple[CoordKey, CoordKey]]],
) -> List[BusDriver]:
    orphans: List[BusDriver] = []

    for labels, priority, kind in (
        (getattr(schematic, "labels", ()) or (), KiCadDriverPriority.LOCAL_LABEL, KiCadDriverKind.LOCAL_LABEL),
        (getattr(schematic, "global_labels", ()) or (), KiCadDriverPriority.GLOBAL, KiCadDriverKind.GLOBAL_LABEL),
        (getattr(schematic, "hierarchical_labels", ()) or (), KiCadDriverPriority.HIER_LABEL, KiCadDriverKind.HIER_LABEL),
    ):
        for label in labels:
            _attach_bus_driver(
                BusDriver(
                    text=label.text,
                    coord=snap_mm_to_iu(label.at_x, label.at_y),
                    priority=priority,
                    kind=kind,
                ),
                out,
                orphans,
                bus_aliases,
                bus_uf=bus_uf,
                root_to_idx=root_to_idx,
                segs_by_root=segs_by_root,
            )

    for sheet in getattr(schematic, "sheets", ()) or ():
        for pin in getattr(sheet, "pins", ()) or ():
            _attach_bus_driver(
                BusDriver(
                    text=pin.name,
                    coord=snap_mm_to_iu(pin.at_x, pin.at_y),
                    priority=KiCadDriverPriority.SHEET_PIN,
                    kind=KiCadDriverKind.SHEET_PIN,
                ),
                out,
                orphans,
                bus_aliases,
                bus_uf=bus_uf,
                root_to_idx=root_to_idx,
                segs_by_root=segs_by_root,
            )
    return orphans


def _add_orphan_bus_driver_subgraphs(out: List[BusSubgraph], orphans: List[BusDriver]) -> None:
    by_text: Dict[str, int] = {}
    for driver in orphans:
        idx = by_text.get(driver.text)
        if idx is None:
            idx = len(out)
            out.append(BusSubgraph(coords={driver.coord}))
            by_text[driver.text] = idx
        else:
            out[idx].coords.add(driver.coord)
        out[idx].drivers.append(driver)


def _resolve_bus_subgraph_names(
    out: List[BusSubgraph],
    bus_aliases: Dict[str, List[str]],
) -> None:
    for subgraph in out:
        bus_form_drivers = [
            driver for driver in subgraph.drivers
            if is_bus_label(driver.text) or driver.text in bus_aliases
        ]
        if not bus_form_drivers:
            continue
        indexed = list(enumerate(bus_form_drivers))
        indexed.sort(key=lambda t: (-int(t[1].priority), t[1].text, t[0]))
        _, best = indexed[0]
        subgraph.chosen_name = best.text
        subgraph.chosen_priority = best.priority
        subgraph.chosen_kind = best.kind
        subgraph.members = list(expand_bus_label(best.text, bus_aliases))


# ---------------------------------------------------------------------------
# BusSubgraph builder
# ---------------------------------------------------------------------------


def build_bus_subgraphs(
    schematic: "KiCadSchematic",
    bus_aliases: Optional[Dict[str, List[str]]] = None,
) -> List[BusSubgraph]:
    """Build :class:`BusSubgraph` records for every bus on the sheet."""
    if bus_aliases is None:
        bus_aliases = collect_bus_aliases(schematic)

    bus_uf = _CoordUnionFind()
    bus_segments = _collect_bus_segments(schematic, bus_uf)
    wire_segments = _collect_wire_segments(schematic)
    bus_entry_taps = _collect_bus_entry_taps(
        schematic,
        bus_segments,
        wire_segments,
        bus_uf,
    )
    segs_by_root = _group_bus_segments_by_root(bus_segments, bus_uf)
    out, root_to_idx = _build_bus_subgraph_records(
        _group_bus_coords(bus_uf),
        bus_entry_taps,
        bus_uf,
    )
    orphans = _attach_schematic_bus_drivers(
        schematic,
        out,
        bus_aliases,
        bus_uf=bus_uf,
        root_to_idx=root_to_idx,
        segs_by_root=segs_by_root,
    )
    _add_orphan_bus_driver_subgraphs(out, orphans)
    _resolve_bus_subgraph_names(out, bus_aliases)
    return out


# ---------------------------------------------------------------------------
# ---------------------------------------------------------------------------
# Within-sheet member merge â€” wire UF mutation
# ---------------------------------------------------------------------------


def merge_bus_member_taps_within_sheet(
    cgraph: ConnectivityGraph,
    bus_subgraphs: Iterable[BusSubgraph],
    wire_label_drivers: Iterable["object"],
) -> None:
    """Union wire-UF roots that tap the same bus member.

    For each bus subgraph that has a resolved member list, look at every
    wire stub tapping out of it. The wire stub's name (its LOCAL_LABEL
    driver, when present) tells us which member it represents. Wires
    representing the same member must end up on the same net â€” so we
    union their wire-UF roots in ``cgraph``.

    ``wire_label_drivers`` is the compiler's already-collected
    :class:`_LabelDriver` list (only ``LOCAL_LABEL`` entries are
    inspected). Caller must have run ``_attach_drivers_to_segments``
    first so each label coord is properly unioned into its wire
    component.
    """
    # Build a fast root â†’ first-seen label-text map for LOCAL labels.
    label_text_by_root: Dict[CoordKey, str] = {}
    for ld in wire_label_drivers:
        kind = getattr(ld, "kind", None)
        if kind != KiCadDriverKind.LOCAL_LABEL:
            continue
        coord = getattr(ld, "coord", None)
        if coord is None:
            continue
        root = cgraph.find(coord)
        label_text_by_root.setdefault(
            root,
            canonical_bus_member_name(getattr(ld, "text", "") or ""),
        )

    for bs in bus_subgraphs:
        if not bs.members:
            continue
        member_set = {canonical_bus_member_name(m) for m in bs.members}
        # Group tap-wire UF roots by member name.
        roots_by_member: Dict[str, List[CoordKey]] = {}
        for tap_coord in bs.tap_wire_coords:
            if not cgraph.has(tap_coord[0] / 1, tap_coord[1] / 1):
                # ``has`` expects mm; use raw key check instead.
                pass
            # Direct check by raw key: ``ConnectivityGraph._parent`` is private,
            # so use ``find`` which seeds the node on access. We only want to
            # consult existing roots â€” skip when the coord wasn't seeded.
            # ``find`` would create a fresh singleton; cheap and harmless but
            # we still want a real wire-side coord.
            root = cgraph.find(tap_coord)
            text = label_text_by_root.get(root)
            if text is None or text not in member_set:
                continue
            roots_by_member.setdefault(text, []).append(root)
        for roots in roots_by_member.values():
            if len(roots) < 2:
                continue
            for k in roots[1:]:
                cgraph.union(roots[0], k)


__all__ = [
    "BusDriver",
    "BusSubgraph",
    "build_bus_subgraphs",
    "collect_bus_aliases",
    "merge_bus_member_taps_within_sheet",
]

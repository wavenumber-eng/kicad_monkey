"""Canonical parent-first traversal of a KiCad schematic hierarchy."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Iterator

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .kicad_sch_sheet import SchSheet
    from .kicad_schematic import KiCadSchematic


def _join_path(parent: str, child: str) -> str:
    parent_segment = str(parent or "").strip("/")
    base = "/" if not parent_segment else f"/{parent_segment}/"
    segment = str(child or "").strip("/")
    return base if not segment else f"{base}{segment}/"


def _source_path(schematic: "KiCadSchematic") -> Path | None:
    source = getattr(schematic, "source_path", None)
    return None if source is None else Path(str(source))


@dataclass(frozen=True)
class KiCadSheetOccurrence:
    """One realized placement of a schematic, including inherited policy."""

    index: int
    schematic: "KiCadSchematic" = field(compare=False, repr=False)
    sheet_path: str = "/"
    sheet_path_uuids: str = "/"
    sheet_instance_path: str | None = None
    sheet_name: str = "root"
    sheet_file: str = ""
    sheet_symbol: "SchSheet | None" = field(default=None, compare=False, repr=False)
    parent: "KiCadSheetOccurrence | None" = field(
        default=None, compare=False, repr=False
    )
    raw_dnp: bool = False
    raw_in_bom: bool = True
    raw_on_board: bool = True
    raw_exclude_from_sim: bool = False
    effective_dnp: bool = False
    effective_in_bom: bool = True
    effective_on_board: bool = True
    effective_exclude_from_sim: bool = False

    @property
    def occurrence_address(self) -> str:
        """Stable source occurrence selector, preferring KiCad's instance path."""
        return self.sheet_instance_path or self.sheet_path_uuids


def walk_schematic_occurrences(
    top: "KiCadSchematic",
    *,
    include_off_board: bool = True,
) -> Iterator[KiCadSheetOccurrence]:
    """Yield root then descendants in source order.

    Policy is folded across every placed-sheet ancestor.  ``include_off_board``
    controls whether occurrences excluded from the board domain are traversed;
    the complete, variant-neutral schematic graph uses the default ``True``.
    """

    source = _source_path(top)
    top_name = source.stem if source is not None else "root"
    top_uuid = str(getattr(top, "uuid", "") or "")
    root = KiCadSheetOccurrence(
        index=1,
        schematic=top,
        sheet_instance_path=f"/{top_uuid}" if top_uuid else None,
        sheet_name=top_name,
    )
    yield root
    next_index = 2

    def walk(parent: KiCadSheetOccurrence, ancestry: frozenset[int]):
        nonlocal next_index
        parent_schematic = parent.schematic
        for sheet in getattr(parent_schematic, "sheets", ()) or ():
            child = getattr(parent_schematic, "sub_schematics", {}).get(
                sheet.sheet_file
            )
            if child is None or id(child) in ancestry:
                continue

            sheet_name = sheet.sheet_name or sheet.sheet_file
            sheet_uuid = sheet.uuid or sheet.sheet_file
            uuid_path = _join_path(parent.sheet_path_uuids, sheet_uuid)
            instance_path = None
            if parent.sheet_instance_path and sheet.uuid:
                instance_path = f"{parent.sheet_instance_path.rstrip('/')}/{sheet.uuid}"

            raw_dnp = bool(getattr(sheet, "dnp", False))
            raw_in_bom = bool(getattr(sheet, "in_bom", True))
            raw_on_board = bool(getattr(sheet, "on_board", True))
            raw_exclude_from_sim = bool(getattr(sheet, "exclude_from_sim", False))
            occurrence = KiCadSheetOccurrence(
                index=next_index,
                schematic=child,
                sheet_path=_join_path(parent.sheet_path, sheet_name),
                sheet_path_uuids=uuid_path,
                sheet_instance_path=instance_path,
                sheet_name=sheet_name,
                sheet_file=sheet.sheet_file,
                sheet_symbol=sheet,
                parent=parent,
                raw_dnp=raw_dnp,
                raw_in_bom=raw_in_bom,
                raw_on_board=raw_on_board,
                raw_exclude_from_sim=raw_exclude_from_sim,
                effective_dnp=parent.effective_dnp or raw_dnp,
                effective_in_bom=parent.effective_in_bom and raw_in_bom,
                effective_on_board=parent.effective_on_board and raw_on_board,
                effective_exclude_from_sim=(
                    parent.effective_exclude_from_sim or raw_exclude_from_sim
                ),
            )
            next_index += 1
            if not include_off_board and not occurrence.effective_on_board:
                continue
            yield occurrence
            yield from walk(occurrence, ancestry | {id(child)})

    yield from walk(root, frozenset({id(top)}))


__all__ = ["KiCadSheetOccurrence", "walk_schematic_occurrences"]

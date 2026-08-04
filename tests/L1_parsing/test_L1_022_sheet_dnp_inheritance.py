"""
Test L1_022: DNP inherited from a hierarchical sheet

KiCad 9 lets a hierarchical sheet be marked "do not populate", and resolves
that across the *whole* instance path — ``SCH_SHEET_PATH::GetDNP`` walks every
sheet, so a part nested three levels below a DNP sheet is DNP too.

The model used to look only at the sheet a symbol sat directly inside, and only
when emitting the netlist's ``dnp`` marker property; the parsed
``KiCadNetlistComponent.dnp`` and the assembly's ``effective_dnp`` ignored
sheets entirely. Consumers therefore saw a part that was DNP in the netlist
text but populated in every parsed view of the same design.

The upstream QA ``variants`` project is the fixture: one parent plus one
sub-sheet, copied to a temp dir so the sheet's own ``dnp`` flag can be flipped.
"""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from kicad_monkey import KiCadSchematic
from kicad_monkey.kicad_variants import assemble, resolve_symbol
from kicad_monkey.testing.corpus import get_kicad_upstream_qa_dir

SHEET_FILE = "pic_sockets.kicad_sch"


@pytest.fixture
def variants_dir() -> Path:
    return get_kicad_upstream_qa_dir() / "eeschema" / "variants"


def _project_copy(
    source: Path,
    destination: Path,
    *,
    sheet_dnp: bool | None = None,
    sheet_in_bom: bool | None = None,
    sheet_on_board: bool | None = None,
) -> Path:
    """Copy the hierarchical fixture, optionally rewriting sub-sheet flags.

    Only the `(sheet ...)` block is touched. Placed symbols carry matching
    tokens at the same indent depth, so a blanket replace would flip the whole
    design and quietly turn the control cases green for the wrong reason.
    """
    destination.mkdir(parents=True, exist_ok=True)
    for name in ("variants.kicad_sch", SHEET_FILE):
        shutil.copy(source / name, destination / name)

    top = destination / "variants.kicad_sch"
    text = top.read_text(encoding="utf-8")

    sheet_start = text.index("\n\t(sheet\n")
    sheet_end = text.index("\n\t)", sheet_start) + 1
    sheet_block = text[sheet_start:sheet_end]

    def _rewrite_flag(block: str, name: str, value: bool) -> str:
        flag_start = block.index(f"({name} ")
        flag_end = block.index(")", flag_start) + 1
        marker = f"({name} {'yes' if value else 'no'})"
        return block[:flag_start] + marker + block[flag_end:]

    if sheet_dnp is not None:
        sheet_block = _rewrite_flag(sheet_block, "dnp", sheet_dnp)
    if sheet_in_bom is not None:
        sheet_block = _rewrite_flag(sheet_block, "in_bom", sheet_in_bom)
    if sheet_on_board is not None:
        sheet_block = _rewrite_flag(sheet_block, "on_board", sheet_on_board)

    top.write_text(text[:sheet_start] + sheet_block + text[sheet_end:], encoding="utf-8")
    return top


def _placements(schematic: KiCadSchematic) -> dict[str, tuple[str, bool]]:
    """``{reference: (sheet_path, the symbol's own dnp flag)}``."""
    placements: dict[str, tuple[str, bool]] = {}
    for sym, sheet_path, _owner in schematic.walk_symbols(
        include_off_board_sheets=True
    ):
        reference = resolve_symbol(sym, None, sheet_path=sheet_path).reference
        if reference:
            placements[reference] = (sheet_path, bool(sym.dnp))
    return placements


def _refs_inside_the_sub_sheet(schematic: KiCadSchematic) -> set[str]:
    top_prefix = "/" + (schematic.uuid or "")
    return {
        reference
        for reference, (sheet_path, own_dnp) in _placements(schematic).items()
        if sheet_path != top_prefix and not own_dnp
    }


def _refs_on_the_top_sheet(schematic: KiCadSchematic) -> set[str]:
    top_prefix = "/" + (schematic.uuid or "")
    return {
        reference
        for reference, (sheet_path, own_dnp) in _placements(schematic).items()
        if sheet_path == top_prefix and not own_dnp
    }


class TestSheetPathWalk:
    def test_sheet_paths_join_with_the_symbol_walk(self, variants_dir: Path) -> None:
        sch = KiCadSchematic.from_file(variants_dir / "variants.kicad_sch")

        sheet_paths = {path for path, _sheet, _parent in sch.walk_sheet_paths()}
        symbol_paths = {path for _sym, path, _owner in sch.walk_symbols()}

        assert sheet_paths, "the fixture has a sub-sheet"
        # Every path the sheet walker reports is one the symbol walker hands to
        # the symbols placed inside that sheet — that join is the whole point.
        assert sheet_paths <= symbol_paths

    def test_a_parent_is_yielded_before_its_child(self, variants_dir: Path) -> None:
        sch = KiCadSchematic.from_file(variants_dir / "variants.kicad_sch")

        seen: set[str] = set()
        for path, _sheet, parent_path in sch.walk_sheet_paths():
            assert not seen or parent_path in seen, "child yielded before its parent"
            seen.add(path)


class TestAssemblyInheritsSheetDnp:
    def test_a_dnp_sheet_marks_the_parts_inside_it(
        self, variants_dir: Path, tmp_path: Path
    ) -> None:
        sch = KiCadSchematic.from_file(
            _project_copy(variants_dir, tmp_path / "dnp", sheet_dnp=True)
        )
        inside = _refs_inside_the_sub_sheet(sch)
        by_ref = {c.reference: c for c in assemble(sch)}

        assert inside, "the fixture places symbols inside the sub-sheet"
        assert all(by_ref[reference].effective_dnp for reference in inside)

    def test_parts_outside_the_dnp_sheet_are_untouched(
        self, variants_dir: Path, tmp_path: Path
    ) -> None:
        sch = KiCadSchematic.from_file(
            _project_copy(variants_dir, tmp_path / "dnp", sheet_dnp=True)
        )
        outside = _refs_on_the_top_sheet(sch)
        by_ref = {c.reference: c for c in assemble(sch)}

        assert outside, "the fixture places symbols on the top sheet"
        assert not any(by_ref[reference].effective_dnp for reference in outside)

    def test_a_populated_sheet_leaves_its_parts_populated(
        self, variants_dir: Path, tmp_path: Path
    ) -> None:
        """The control: same project, sheet flag off, nothing inherits."""
        sch = KiCadSchematic.from_file(
            _project_copy(variants_dir, tmp_path / "populated", sheet_dnp=False)
        )
        inside = _refs_inside_the_sub_sheet(sch)
        by_ref = {c.reference: c for c in assemble(sch)}

        assert inside, "the fixture places symbols inside the sub-sheet"
        assert not any(by_ref[reference].effective_dnp for reference in inside)


class TestAssemblyInheritsSheetBomAndBoard:
    def test_in_bom_no_sheet_clears_effective_in_bom(
        self, variants_dir: Path, tmp_path: Path
    ) -> None:
        sch = KiCadSchematic.from_file(
            _project_copy(variants_dir, tmp_path / "bom", sheet_in_bom=False)
        )
        inside = _refs_inside_the_sub_sheet(sch)
        outside = _refs_on_the_top_sheet(sch)
        by_ref = {c.reference: c for c in assemble(sch)}

        assert inside and outside
        assert all(not by_ref[reference].effective_in_bom for reference in inside)
        # Virtual refs and symbols already marked out of BOM stay False;
        # only BOM-eligible top-sheet parts must remain True.
        eligible_outside = {
            reference
            for reference in outside
            if by_ref[reference].symbol is not None
            and by_ref[reference].symbol.in_bom
            and not reference.startswith("#")
        }
        assert eligible_outside
        assert all(
            by_ref[reference].effective_in_bom for reference in eligible_outside
        )

    def test_on_board_no_sheet_clears_effective_on_board(
        self, variants_dir: Path, tmp_path: Path
    ) -> None:
        sch = KiCadSchematic.from_file(
            _project_copy(variants_dir, tmp_path / "board", sheet_on_board=False)
        )
        inside = _refs_inside_the_sub_sheet(sch)
        outside = _refs_on_the_top_sheet(sch)
        by_ref = {c.reference: c for c in assemble(sch)}

        assert inside and outside
        assert all(not by_ref[reference].effective_on_board for reference in inside)
        assert all(by_ref[reference].effective_on_board for reference in outside)


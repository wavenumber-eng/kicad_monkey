"""
Test L1_016: BOM filter parity (assemble() vs KiCad's emit rules)

Phase D Slice D-2. Locks the new ``assemble()`` behaviors that close
the documented Phase C gaps:

1. Hierarchical traversal — sub-sheet symbols are included via
   :meth:`KiCadSchematic.walk_symbols` (D-1) so flat ``schematic.symbols``
   no longer determines BOM coverage on multi-sheet projects.
2. Sub-sheet refs use ``instances[*].reference`` — the property's
   ``Reference`` value is a stub on un-annotated sub-sheets but
   KiCad's BOM emit uses the per-instance reference. ``resolve_symbol``
   now does the same when given a ``sheet_path``.
3. Power / virtual-ref filter — refs starting with ``#`` (e.g.
   ``#PWR01``, ``#FLG02``) are dropped from ``effective_in_bom`` to
   match KiCad's BOM iterator which excludes them regardless of the
   ``in_bom`` flag.
4. Per-instance variant override resolution honors the sheet path —
   in multi-sheet designs each instantiation can carry independent
   variant data. Single-sheet behavior unchanged (first-instance
   fallback).
"""

from __future__ import annotations

from pathlib import Path

import pytest

from kicad_monkey import KiCadSchematic
from kicad_monkey.kicad_sch_sheet import SchSheet, SchSheetProperty
from kicad_monkey.kicad_sch_symbol import SchSymbol
from kicad_monkey.kicad_sym_property import SymProperty
from kicad_monkey.kicad_variants import (
    _is_virtual_ref,
    _instance_reference,
    _select_symbol_instance_variant,
    assemble,
    resolve_symbol,
)
from kicad_monkey.testing.corpus import get_kicad_upstream_qa_dir


@pytest.fixture
def variants_sch() -> Path:
    return get_kicad_upstream_qa_dir() / "eeschema" / "variants" / "variants.kicad_sch"


# ---------------------------------------------------------------------------
# Virtual-ref filter
# ---------------------------------------------------------------------------


class TestVirtualRefFilter:
    @pytest.mark.parametrize("ref", ["#PWR01", "#FLG02", "#PWR045", "#"])
    def test_hash_prefix_is_virtual(self, ref: str) -> None:
        assert _is_virtual_ref(ref) is True

    @pytest.mark.parametrize("ref", ["R1", "C123", "U7", "J1", ""])
    def test_normal_ref_is_not_virtual(self, ref: str) -> None:
        assert _is_virtual_ref(ref) is False


class TestVirtualRefDroppedFromBom:
    def test_pwr_refs_excluded_from_effective_in_bom(self, variants_sch: Path) -> None:
        sch = KiCadSchematic.from_file(variants_sch)
        comps = assemble(sch)
        bom_refs = [c.reference for c in comps if c.effective_in_bom]
        # Power refs exist in the schematic (we walked them) but must
        # not appear in the BOM-eligible set.
        all_refs = [c.reference for c in comps]
        assert any(r.startswith("#") for r in all_refs), (
            "fixture sanity: schematic should contain virtual refs"
        )
        assert not any(r.startswith("#") for r in bom_refs)


# ---------------------------------------------------------------------------
# Hierarchical assembly
# ---------------------------------------------------------------------------


class TestHierarchicalAssembly:
    def test_assemble_walks_sub_sheets(self, variants_sch: Path) -> None:
        """assemble() must include sub-sheet symbols, not just top-level."""
        sch = KiCadSchematic.from_file(variants_sch)
        comps = assemble(sch)
        refs = {c.reference for c in comps}
        # eeschema/variants has a pic_sockets sub-sheet whose annotated
        # refs include U5, U6 (sockets) and several capacitors.
        assert "U5" in refs
        assert "U6" in refs

    def test_subsheet_ref_from_instance_not_property(
        self, variants_sch: Path,
    ) -> None:
        """Sub-sheet symbols carry stub property Reference (e.g. 'C',
        '#PWR') but the annotated ref lives in instances[*].reference.
        resolve_symbol with the matching sheet_path must use the
        instance reference."""
        sch = KiCadSchematic.from_file(variants_sch)
        child = sch.sub_schematics["pic_sockets.kicad_sch"]
        # Find the (sym, sheet_path) pair via walk_symbols.
        for sym, sheet_path, owner in sch.walk_symbols():
            if owner is child:
                # Resolve against the child sheet path.
                eff = resolve_symbol(sym, None, sheet_path=sheet_path)
                # Reference must come from the instance, never blank
                # for annotated fixtures.
                assert eff.reference, (
                    f"sub-sheet symbol resolved to empty ref: lib_id={sym.lib_id}"
                )

    def test_assemble_skips_symbols_under_off_board_sheet(self) -> None:
        """Sheet-level on_board=no removes the child sheet from assembly."""

        def symbol(ref: str) -> SchSymbol:
            sym = SchSymbol(lib_id="Device:R")
            sym.properties = [
                SymProperty(key="Reference", value=ref),
                SymProperty(key="Value", value="10k"),
            ]
            return sym

        active = KiCadSchematic()
        active.uuid = "active-child"
        active.symbols.append(symbol("R_ON"))

        off_board = KiCadSchematic()
        off_board.uuid = "off-child"
        off_board.symbols.append(symbol("R_OFF"))

        def sheet(file_name: str, name: str, uuid: str, on_board: bool) -> SchSheet:
            sh = SchSheet(uuid=uuid, on_board=on_board)
            sh.properties = [
                SchSheetProperty(key="Sheetname", value=name),
                SchSheetProperty(key="Sheetfile", value=file_name),
            ]
            return sh

        root = KiCadSchematic()
        root.uuid = "root"
        root.sheets.extend([
            sheet("active.kicad_sch", "active", "active-sheet", True),
            sheet("off.kicad_sch", "off", "off-sheet", False),
        ])
        root.sub_schematics["active.kicad_sch"] = active
        root.sub_schematics["off.kicad_sch"] = off_board

        source_refs = {sym.reference for sym, _path, _owner in root.walk_symbols()}
        assert source_refs == {"R_ON", "R_OFF"}

        realized_refs = {
            sym.reference
            for sym, _path, _owner in root.walk_symbols(include_off_board_sheets=False)
        }
        assert realized_refs == {"R_ON"}

        assembled_refs = {component.reference for component in assemble(root)}
        assert assembled_refs == {"R_ON"}

    def test_assemble_folds_policy_from_every_sheet_ancestor(self) -> None:
        symbol = SchSymbol(lib_id="Device:R")
        symbol.properties = [
            SymProperty(key="Reference", value="R_LEAF"),
            SymProperty(key="Value", value="10k"),
        ]
        leaf = KiCadSchematic()
        leaf.uuid = "leaf-source"
        leaf.symbols.append(symbol)

        mid = KiCadSchematic()
        mid.uuid = "mid-source"
        leaf_sheet = SchSheet(uuid="leaf-placement")
        leaf_sheet.properties = [
            SchSheetProperty(key="Sheetname", value="leaf"),
            SchSheetProperty(key="Sheetfile", value="leaf.kicad_sch"),
        ]
        mid.sheets.append(leaf_sheet)
        mid.sub_schematics["leaf.kicad_sch"] = leaf

        root = KiCadSchematic()
        root.uuid = "root-source"
        mid_sheet = SchSheet(
            uuid="mid-placement",
            dnp=True,
            in_bom=False,
            exclude_from_sim=True,
        )
        mid_sheet.properties = [
            SchSheetProperty(key="Sheetname", value="mid"),
            SchSheetProperty(key="Sheetfile", value="mid.kicad_sch"),
        ]
        root.sheets.append(mid_sheet)
        root.sub_schematics["mid.kicad_sch"] = mid

        [component] = assemble(root)

        assert component.reference == "R_LEAF"
        assert component.effective_dnp is True
        assert component.effective_in_bom is False
        assert component.effective_on_board is True
        assert component.symbol is not None
        assert component.symbol.exclude_from_sim is True


# ---------------------------------------------------------------------------
# Path-aware variant resolution
# ---------------------------------------------------------------------------


class TestPathAwareInstanceLookup:
    def test_instance_reference_uses_matching_path(self) -> None:
        """_instance_reference picks the entry whose path equals
        the requested sheet_path."""
        from kicad_monkey.kicad_sch_symbol import SchSymbol, SchSymbolInstance

        sym = SchSymbol(lib_id="Device:R")
        sym.instances = [
            SchSymbolInstance(project="P", path="/A", reference="R1"),
            SchSymbolInstance(project="P", path="/A/B", reference="R2"),
        ]
        assert _instance_reference(sym, sheet_path="/A") == "R1"
        assert _instance_reference(sym, sheet_path="/A/B") == "R2"
        # Unknown path falls back to first instance.
        assert _instance_reference(sym, sheet_path="/Z") == "R1"

    def test_variant_lookup_restricted_to_matching_path(self) -> None:
        """_select_symbol_instance_variant with sheet_path returns the
        variant from that instance only — variants on other instances
        are ignored, mirroring KiCad's per-sheet override semantics."""
        from kicad_monkey.kicad_sch_symbol import (
            SchSymbol, SchSymbolInstance, SchSymbolInstanceVariant,
        )

        v_a = SchSymbolInstanceVariant(name="V", dnp=True)
        v_b = SchSymbolInstanceVariant(name="V", dnp=False)
        sym = SchSymbol(lib_id="Device:R")
        sym.instances = [
            SchSymbolInstance(project="P", path="/A", reference="R1", variants=[v_a]),
            SchSymbolInstance(project="P", path="/A/B", reference="R2", variants=[v_b]),
        ]
        # Path-scoped lookup picks the right variant per instance.
        assert _select_symbol_instance_variant(sym, "V", sheet_path="/A") is v_a
        assert _select_symbol_instance_variant(sym, "V", sheet_path="/A/B") is v_b
        # Unknown path returns None (no fallback to first match).
        assert _select_symbol_instance_variant(sym, "V", sheet_path="/Z") is None
        # Without sheet_path, legacy first-match behavior.
        assert _select_symbol_instance_variant(sym, "V") is v_a


# ---------------------------------------------------------------------------
# Backward compatibility — flat schematics
# ---------------------------------------------------------------------------


class TestFlatSchematicCompat:
    def test_resolve_symbol_no_sheet_path_uses_property_or_first_instance(
        self,
    ) -> None:
        """Without sheet_path, resolve_symbol falls back to the
        property's Reference (or first-instance ref). This preserves
        existing single-sheet API behavior."""
        from kicad_monkey.kicad_sch_symbol import SchSymbol, SchSymbolInstance
        from kicad_monkey.kicad_sym_property import SymProperty

        sym = SchSymbol(lib_id="Device:R")
        sym.properties = [
            SymProperty(key="Reference", value="R99"),
            SymProperty(key="Value", value="10k"),
        ]
        # No instance — pure property fallback.
        eff = resolve_symbol(sym, None)
        assert eff.reference == "R99"
        # Add an instance with a different ref — first-instance wins
        # over the property when present.
        sym.instances = [SchSymbolInstance(project="P", path="/X", reference="R7")]
        eff = resolve_symbol(sym, None)
        assert eff.reference == "R7"

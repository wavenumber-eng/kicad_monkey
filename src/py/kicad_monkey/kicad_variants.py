"""
Variant model layer — bridges KiCad's three legacy variant carriers
(``.kicad_pro`` ``schematic.variants`` / PCB top-level ``(variants ...)`` /
per-symbol & per-footprint override blocks) into one queryable model.

This module is purely additive on top of the existing low-level
dataclasses. It does not mutate them; the resolver layer consumes
``VariantOverride`` produced here.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import TYPE_CHECKING, Any, Iterable, Iterator, Optional
import warnings

from .kicad_project import ProjectVariant
from .kicad_schematic_occurrence import walk_schematic_occurrences

if TYPE_CHECKING:  # pragma: no cover — typing-only
    from .kicad_project import KiCadProject
    from .kicad_pcb import KiCadPcb
    from .kicad_schematic import KiCadSchematic


# ---------------------------------------------------------------------------
# Normalized override (resolver-friendly)
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class VariantOverride:
    """One resolved variant override for one component (symbol or footprint).

    Each ``Optional[bool]`` is ``None`` when the source carrier did not
    set that field — meaning "fall back to the base property". Field
    overrides are a name → value map (KiCad's override list is
    authoritative for which fields differ; missing keys mean "no
    override on that field").

    The same shape is used regardless of source carrier, so the
    resolver can treat per-symbol-instance, per-sheet, and
    per-footprint overrides uniformly.
    """

    reference: str
    variant_name: str
    dnp: Optional[bool] = None
    exclude_from_bom: Optional[bool] = None
    exclude_from_pos_files: Optional[bool] = None
    exclude_from_sim: Optional[bool] = None
    in_bom: Optional[bool] = None
    on_board: Optional[bool] = None
    in_pos_files: Optional[bool] = None
    fields: dict = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Catalog: the SET of variants a project knows about
# ---------------------------------------------------------------------------

@dataclass
class VariantCatalog:
    """Set of variants known to a project.

    Sources, in priority order:

    1. ``.kicad_pro`` ``schematic.variants`` (canonical — KiCad's UI
       is the only thing that adds/removes entries here).
    2. PCB top-level ``(variants ...)`` block (mirror of #1).
    3. Names mentioned in any per-symbol / per-footprint override
       block (defensive — surfaces inconsistency if an override
       references a variant that's not in the canonical catalog).

    The catalog is read-only; mutation belongs to project read/write helpers.
    """

    variants: list[ProjectVariant] = field(default_factory=list)

    def __contains__(self, name: str) -> bool:
        return any(v.name == name for v in self.variants)

    def __iter__(self) -> Iterator[ProjectVariant]:
        return iter(self.variants)

    def __len__(self) -> int:
        return len(self.variants)

    @property
    def names(self) -> list[str]:
        return [v.name for v in self.variants]

    def get(self, name: str) -> Optional[ProjectVariant]:
        for v in self.variants:
            if v.name == name:
                return v
        return None

    # -- Source-specific factories -----------------------------------------

    @classmethod
    def from_project(cls, project: "KiCadProject") -> "VariantCatalog":
        """Catalog derived from the canonical ``schematic.variants`` block."""
        return cls(variants=list(project.variants))

    @classmethod
    def from_pcb(cls, pcb: "KiCadPcb") -> "VariantCatalog":
        """Catalog derived from the PCB top-level ``(variants ...)`` block."""
        return cls(variants=[
            ProjectVariant(name=bv.name, description=bv.description)
            for bv in getattr(pcb, "variants", []) or []
        ])

    @classmethod
    def from_overrides(
        cls,
        *,
        schematic: "KiCadSchematic | None" = None,
        pcb: "KiCadPcb | None" = None,
    ) -> "VariantCatalog":
        """Catalog reverse-engineered from override-block names.

        Useful as a defensive cross-check: if an override mentions a
        variant that the project catalog does not, something's wrong.
        Descriptions are unknown here so are left ``None``.
        """
        names: list[str] = []
        seen: set[str] = set()

        def _push(name: str) -> None:
            if name and name not in seen:
                names.append(name)
                seen.add(name)

        if schematic is not None:
            for sym in getattr(schematic, "symbols", []) or []:
                for inst in getattr(sym, "instances", []) or []:
                    for v in getattr(inst, "variants", []) or []:
                        _push(v.name)
            for sheet in getattr(schematic, "sheets", []) or []:
                for inst in getattr(sheet, "instances", []) or []:
                    for v in getattr(inst, "variants", []) or []:
                        _push(v.name)

        if pcb is not None:
            for fp in getattr(pcb, "footprints", []) or []:
                for v in getattr(fp, "variants", []) or []:
                    _push(v.name)

        return cls(variants=[ProjectVariant(name=n, description=None) for n in names])

    # -- Discovery: union all sources, warn on inconsistency ---------------

    @classmethod
    def discover(
        cls,
        project: "KiCadProject | None" = None,
        schematic: "KiCadSchematic | None" = None,
        pcb: "KiCadPcb | None" = None,
        *,
        warn_on_inconsistency: bool = True,
    ) -> "VariantCatalog":
        """Union all available sources into a single catalog.

        The canonical ``.kicad_pro`` catalog wins for descriptions and
        ordering; PCB-side and override-side names that aren't in the
        canonical catalog are appended at the end, and a warning is
        emitted (one per source class).
        """
        canonical = cls.from_project(project) if project is not None else cls()
        canonical_names = set(canonical.names)
        merged: list[ProjectVariant] = list(canonical.variants)
        seen: set[str] = set(canonical_names)

        def _merge(other: "VariantCatalog", source_label: str) -> None:
            extra: list[str] = []
            for v in other.variants:
                if v.name in seen:
                    continue
                merged.append(v)
                seen.add(v.name)
                extra.append(v.name)
            if extra and warn_on_inconsistency and project is not None:
                warnings.warn(
                    f"variant catalog: {source_label} mentions variants "
                    f"missing from the canonical .kicad_pro catalog: "
                    f"{extra}",
                    stacklevel=3,
                )

        if pcb is not None:
            _merge(cls.from_pcb(pcb), "PCB top-level (variants ...) block")

        if schematic is not None or pcb is not None:
            _merge(
                cls.from_overrides(schematic=schematic, pcb=pcb),
                "per-symbol/per-footprint override blocks",
            )

        return cls(variants=merged)


# ---------------------------------------------------------------------------
# Lowering helpers (carriers → VariantOverride)
# ---------------------------------------------------------------------------

def _override_from_sch_instance_variant(
    reference: str, sv: Any
) -> VariantOverride:
    """Lower a ``SchSymbolInstanceVariant`` to a ``VariantOverride``."""
    return VariantOverride(
        reference=reference,
        variant_name=sv.name,
        dnp=sv.dnp,
        exclude_from_sim=sv.exclude_from_sim,
        in_bom=sv.in_bom,
        on_board=sv.on_board,
        in_pos_files=sv.in_pos_files,
        fields={fname: fvalue for fname, fvalue in (sv.fields or [])},
    )


def _override_from_footprint_variant(
    reference: str, fv: Any
) -> VariantOverride:
    """Lower a ``FootprintVariant`` to a ``VariantOverride``."""
    return VariantOverride(
        reference=reference,
        variant_name=fv.name,
        dnp=fv.dnp,
        exclude_from_bom=fv.exclude_from_bom,
        exclude_from_pos_files=fv.exclude_from_pos_files,
        fields={f.name: f.value for f in (fv.fields or [])},
    )


def collect_symbol_overrides(
    schematic: "KiCadSchematic", *, variant_name: str | None = None,
) -> Iterable[VariantOverride]:
    """Yield ``VariantOverride`` for every symbol-instance variant block.

    If *variant_name* is given, only yield overrides matching that name.
    """
    for sym in getattr(schematic, "symbols", []) or []:
        ref = getattr(sym, "reference", "") or ""
        for inst in getattr(sym, "instances", []) or []:
            for sv in getattr(inst, "variants", []) or []:
                if variant_name is not None and sv.name != variant_name:
                    continue
                yield _override_from_sch_instance_variant(ref, sv)


def collect_footprint_overrides(
    pcb: "KiCadPcb", *, variant_name: str | None = None,
) -> Iterable[VariantOverride]:
    """Yield ``VariantOverride`` for every footprint variant block."""
    for fp in getattr(pcb, "footprints", []) or []:
        ref = getattr(fp, "reference", "") or ""
        for fv in getattr(fp, "variants", []) or []:
            if variant_name is not None and fv.name != variant_name:
                continue
            yield _override_from_footprint_variant(ref, fv)


# ---------------------------------------------------------------------------
# Effective-properties resolver
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class EffectiveSymbolProperties:
    """Per-symbol effective properties under one variant."""

    reference: str
    lib_id: str
    value: str
    dnp: bool
    exclude_from_sim: bool
    in_bom: bool
    on_board: bool
    in_pos_files: bool
    fields: dict = field(default_factory=dict)


@dataclass(frozen=True)
class EffectiveFootprintProperties:
    """Per-footprint effective properties under one variant."""

    reference: str
    footprint_lib: str
    dnp: bool
    exclude_from_bom: bool
    exclude_from_pos_files: bool
    fields: dict = field(default_factory=dict)


def _select_symbol_instance_variant(
    sym: Any, variant_name: str, sheet_path: Optional[str] = None,
) -> Any | None:
    """Return the matching ``SchSymbolInstanceVariant`` for *sym* under
    *variant_name*, or ``None`` if no instance has that variant.

    When *sheet_path* is given (the hierarchical UUID prefix from
    :meth:`KiCadSchematic.walk_symbols`), the lookup is restricted to
    the instance whose ``path`` matches that prefix — this is how KiCad
    resolves per-instance overrides in multi-sheet designs (each sheet
    instantiation can carry independent variant data). With
    ``sheet_path=None`` the legacy first-match behavior applies, which
    is correct for flat schematics where there's exactly one instance.
    """
    instances = getattr(sym, "instances", []) or []
    if sheet_path is not None:
        for inst in instances:
            if inst.path == sheet_path:
                for v in getattr(inst, "variants", []) or []:
                    if v.name == variant_name:
                        return v
                return None
        return None
    for inst in instances:
        for v in getattr(inst, "variants", []) or []:
            if v.name == variant_name:
                return v
    return None


def _instance_reference(sym: Any, sheet_path: Optional[str] = None) -> str:
    """Return the symbol's effective reference for the given sheet path.

    KiCad annotates references per-instance: ``(instances (project ...
    (path "/..." (reference "C7"))))``. The ``(property "Reference"
    "...")`` block on the symbol itself is the *default* shown in the
    canvas and may be a stub (``"C"``, ``"#PWR"``) on sub-sheet symbols
    until they're annotated.

    With *sheet_path*, return the reference from the matching instance.
    Without, fall back to the first instance, or finally to the
    property's Reference value (legacy / flat schematic behavior).
    """
    instances = getattr(sym, "instances", []) or []
    if sheet_path is not None:
        for inst in instances:
            inst_path = getattr(inst, "path", "")
            inst_ref = getattr(inst, "reference", "")
            if inst_path == sheet_path and inst_ref:
                return inst_ref
    if instances:
        first_ref = getattr(instances[0], "reference", "")
        if first_ref:
            return first_ref
    # Fall back to the property — used by flat schematics that don't
    # carry instance blocks (or by tests that build symbols directly).
    for prop in getattr(sym, "properties", []) or []:
        key = getattr(prop, "key", None) or getattr(prop, "name", "")
        if key == "Reference":
            return getattr(prop, "value", "") or ""
    return getattr(sym, "reference", "") or ""


def _is_virtual_ref(ref: str) -> bool:
    """True for KiCad's "virtual" references that BOM emit excludes.

    KiCad's BOM exporter (``eeschema/sch_io/...`` and the BOM iterator
    in ``sch_screen.cpp``) drops symbols whose reference starts with
    ``#`` — this is the classic power-symbol convention (``#PWR01``,
    ``#FLG02``, etc.) used for net-ties and global power flags. They
    carry ``in_bom yes`` in the file format but are filtered at emit
    time, so we have to mirror that filter to match kicad-cli output.
    """
    return bool(ref) and ref.startswith("#")


def _symbol_base_fields(sym: Any) -> dict:
    """Lift SymProperty list → ``{key: value}`` dict for resolution."""
    out: dict = {}
    for prop in getattr(sym, "properties", []) or []:
        # SchSymbol uses SymProperty(key, value)
        key = getattr(prop, "key", None) or getattr(prop, "name", "")
        if key:
            out[key] = getattr(prop, "value", "")
    return out


def _footprint_base_fields(fp: Any) -> dict:
    """Lift Property list → ``{name: value}`` dict for resolution."""
    out: dict = {}
    for prop in getattr(fp, "properties", []) or []:
        # PCB Property uses Property(name, value)
        key = getattr(prop, "name", None) or getattr(prop, "key", "")
        if key:
            out[key] = getattr(prop, "value", "")
    return out


def _footprint_reference(fp: Any) -> str:
    """Extract the Reference value from a footprint's properties list."""
    for prop in getattr(fp, "properties", []) or []:
        if getattr(prop, "name", None) == "Reference":
            return getattr(prop, "value", "") or ""
    return ""


def _footprint_select_variant(fp: Any, variant_name: str) -> Any | None:
    for v in getattr(fp, "variants", []) or []:
        if v.name == variant_name:
            return v
    return None


def resolve_symbol(
    sym: Any, variant_name: Optional[str],
    sheet_path: Optional[str] = None,
) -> EffectiveSymbolProperties:
    """Compute effective properties for *sym* under *variant_name*.

    With ``variant_name=None`` the base properties are returned
    untouched. Otherwise the matching per-instance variant override
    is folded over the base values:

    - Each scalar bool override falls back to the base value when the
      override is ``None`` (KiCad elides the token for "same as base"
      so ``None`` round-trips that absence).
    - Field overrides **replace by name** — the base property dict is
      the starting set; override entries overwrite matching keys; new
      keys in the override are added.

    *sheet_path*, when given, is the hierarchical UUID prefix
    identifying which instance the resolution applies to. This selects
    both the correct reference (sub-sheet symbols get their annotated
    ref from ``instances[*].reference`` rather than the property stub)
    and the correct per-instance variant override in multi-sheet
    designs. With ``sheet_path=None`` the resolver uses the first
    instance / property fallback, which is correct for flat schematics.
    """
    base_fields = _symbol_base_fields(sym)
    base_value = base_fields.get("Value", "") or getattr(sym, "value", "") or ""
    # Reference comes from the path-matched instance when known —
    # the property's Reference may be an unannotated stub on subsheets.
    base_ref = _instance_reference(sym, sheet_path) or base_fields.get(
        "Reference", ""
    ) or getattr(sym, "reference", "") or ""

    base = EffectiveSymbolProperties(
        reference=base_ref,
        lib_id=getattr(sym, "lib_id", "") or "",
        value=base_value,
        dnp=bool(getattr(sym, "dnp", False)),
        exclude_from_sim=bool(getattr(sym, "exclude_from_sim", False)),
        in_bom=bool(getattr(sym, "in_bom", True)),
        on_board=bool(getattr(sym, "on_board", True)),
        in_pos_files=bool(getattr(sym, "in_pos_files", True)),
        fields=dict(base_fields),
    )

    if variant_name is None:
        return base

    sv = _select_symbol_instance_variant(sym, variant_name, sheet_path=sheet_path)
    if sv is None:
        return base

    merged_fields = dict(base.fields)
    for fname, fvalue in sv.fields or []:
        merged_fields[fname] = fvalue

    return EffectiveSymbolProperties(
        reference=base.reference,
        lib_id=base.lib_id,
        value=merged_fields.get("Value", base.value),
        dnp=base.dnp if sv.dnp is None else sv.dnp,
        exclude_from_sim=(
            base.exclude_from_sim if sv.exclude_from_sim is None else sv.exclude_from_sim
        ),
        in_bom=base.in_bom if sv.in_bom is None else sv.in_bom,
        on_board=base.on_board if sv.on_board is None else sv.on_board,
        in_pos_files=(
            base.in_pos_files if sv.in_pos_files is None else sv.in_pos_files
        ),
        fields=merged_fields,
    )


def resolve_footprint(
    fp: Any, variant_name: Optional[str]
) -> EffectiveFootprintProperties:
    """Compute effective properties for footprint *fp* under *variant_name*."""
    base_fields = _footprint_base_fields(fp)
    base_ref = _footprint_reference(fp)

    base = EffectiveFootprintProperties(
        reference=base_ref,
        footprint_lib=getattr(fp, "library_link", "") or "",
        dnp=bool(getattr(fp, "is_dnp", False)),
        exclude_from_bom=bool(getattr(fp, "is_excluded_from_bom", False)),
        exclude_from_pos_files=bool(getattr(fp, "is_excluded_from_pos_files", False)),
        fields=dict(base_fields),
    )

    if variant_name is None:
        return base

    fv = _footprint_select_variant(fp, variant_name)
    if fv is None:
        return base

    merged_fields = dict(base.fields)
    for f in fv.fields or []:
        merged_fields[f.name] = f.value

    return EffectiveFootprintProperties(
        reference=base.reference,
        footprint_lib=base.footprint_lib,
        dnp=base.dnp if fv.dnp is None else fv.dnp,
        exclude_from_bom=(
            base.exclude_from_bom if fv.exclude_from_bom is None else fv.exclude_from_bom
        ),
        exclude_from_pos_files=(
            base.exclude_from_pos_files
            if fv.exclude_from_pos_files is None
            else fv.exclude_from_pos_files
        ),
        fields=merged_fields,
    )


# ---------------------------------------------------------------------------
# Cross-domain assembly view
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class AssemblyComponent:
    """Per-component view joining the symbol and footprint sides.

    Components are keyed by reference designator. Either side can be
    ``None`` (e.g. a schematic-only "do not place" component, or a PCB
    placeholder without a schematic symbol). The convenience flags
    collapse the two-sided override semantics into a single boolean
    suitable for BOM / pick-and-place output.

    ``effective_in_pos_files`` mirrors KiCad's pos-export filter: a
    component appears in ``kicad-cli pcb export pos`` output iff its
    footprint does not have ``exclude_from_pos_files`` set (PCB-side
    authoritative — the schematic ``in_pos_files`` flag is only used
    when no footprint exists on the PCB side). Virtual refs (``#``
    prefix) are filtered defensively.
    """

    reference: str
    symbol: Optional[EffectiveSymbolProperties]
    footprint: Optional[EffectiveFootprintProperties]
    effective_dnp: bool
    effective_in_bom: bool
    effective_on_board: bool
    effective_in_pos_files: bool


def _symbol_dnp(sym: Optional[EffectiveSymbolProperties]) -> bool:
    return bool(sym.dnp) if sym is not None else False


def _footprint_dnp(fp: Optional[EffectiveFootprintProperties]) -> bool:
    return bool(fp.dnp) if fp is not None else False


def assemble(
    schematic: Optional["KiCadSchematic"],
    pcb: Optional["KiCadPcb"] = None,
    variant_name: Optional[str] = None,
) -> list[AssemblyComponent]:
    """Join schematic + PCB by reference designator under *variant_name*.

    Returns one :class:`AssemblyComponent` per unique reference. The
    iteration order preserves schematic order first (the "designed"
    side), with PCB-only references appended at the end (defensive —
    surfaces components that exist on the board without a schematic
    counterpart).

    When *schematic* has been loaded via :meth:`KiCadSchematic.from_file`
    (or constructed via the path constructor), sub-sheet symbols are
    walked too — each is resolved with its hierarchical sheet path so
    multi-sheet variant overrides resolve correctly and sub-sheet refs
    use ``instances[*].reference`` instead of the property stub.

    ``effective_in_bom`` excludes virtual refs (anything starting with
    ``#`` — power symbols, flags) to match KiCad's BOM emit filter.
    """
    sym_by_ref: dict[str, EffectiveSymbolProperties] = {}
    if schematic is not None:
        # The shared occurrence walker supplies the exact instance path and
        # policy inherited through every placed-sheet ancestor.
        for occurrence in walk_schematic_occurrences(
            schematic, include_off_board=False
        ):
            for sym in getattr(occurrence.schematic, "symbols", []) or []:
                eff = resolve_symbol(
                    sym,
                    variant_name,
                    sheet_path=occurrence.sheet_instance_path,
                )
                eff = replace(
                    eff,
                    dnp=eff.dnp or occurrence.effective_dnp,
                    exclude_from_sim=(
                        eff.exclude_from_sim
                        or occurrence.effective_exclude_from_sim
                    ),
                    in_bom=eff.in_bom and occurrence.effective_in_bom,
                    on_board=eff.on_board and occurrence.effective_on_board,
                )
                if eff.reference:
                    # Last writer wins on duplicate refs (multi-unit symbols
                    # share a reference; the final unit's resolution is
                    # canonical for assembly purposes).
                    sym_by_ref[eff.reference] = eff

    fp_by_ref: dict[str, EffectiveFootprintProperties] = {}
    if pcb is not None:
        for fp in getattr(pcb, "footprints", []) or []:
            eff = resolve_footprint(fp, variant_name)
            if eff.reference:
                fp_by_ref[eff.reference] = eff

    out: list[AssemblyComponent] = []
    seen: set[str] = set()

    for ref, sym in sym_by_ref.items():
        fp = fp_by_ref.get(ref)
        # KiCad 10 treats either side flagging dnp as authoritative;
        # likewise for in_bom / on_board (effective AND).
        eff_dnp = _symbol_dnp(sym) or _footprint_dnp(fp)
        eff_in_bom = (
            bool(sym.in_bom)
            and not _is_virtual_ref(ref)
            and ((not fp.exclude_from_bom) if fp is not None else True)
        )
        eff_on_board = bool(sym.on_board)
        # Pos eligibility: PCB side is authoritative when present
        # (kicad-cli pcb export pos walks the .kicad_pcb only). For
        # symbol-only components there's no pos row anyway, but we
        # surface the schematic intent for consumers that do their
        # own joins.
        eff_in_pos = (not _is_virtual_ref(ref)) and (
            (not fp.exclude_from_pos_files) if fp is not None
            else bool(sym.in_pos_files)
        )
        out.append(AssemblyComponent(
            reference=ref, symbol=sym, footprint=fp,
            effective_dnp=eff_dnp,
            effective_in_bom=eff_in_bom,
            effective_on_board=eff_on_board,
            effective_in_pos_files=eff_in_pos,
        ))
        seen.add(ref)

    # PCB-only refs (no matching schematic symbol)
    for ref, fp in fp_by_ref.items():
        if ref in seen:
            continue
        out.append(AssemblyComponent(
            reference=ref, symbol=None, footprint=fp,
            effective_dnp=_footprint_dnp(fp),
            effective_in_bom=(not _is_virtual_ref(ref)) and (not fp.exclude_from_bom),
            effective_on_board=True,
            effective_in_pos_files=(not _is_virtual_ref(ref))
                and (not fp.exclude_from_pos_files),
        ))

    return out


__all__ = [
    "AssemblyComponent",
    "EffectiveFootprintProperties",
    "EffectiveSymbolProperties",
    "VariantOverride",
    "VariantCatalog",
    "assemble",
    "collect_footprint_overrides",
    "collect_symbol_overrides",
    "resolve_footprint",
    "resolve_symbol",
]

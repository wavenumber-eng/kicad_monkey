"""KiCad adapter for BOM, pick-and-place, and JLC output commands."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import cast

from kicad_monkey import KiCadDesign, VariantCatalog, assemble
from kicad_monkey.kicad_variants import AssemblyComponent, EffectiveFootprintProperties

_MM_PER_MIL = 0.0254
_FRONT_LAYERS = {"F.Cu", "F.Adhes", "F.Paste", "F.SilkS", "F.Mask", "F.CrtYd", "F.Fab"}
_BACK_LAYERS = {"B.Cu", "B.Adhes", "B.Paste", "B.SilkS", "B.Mask", "B.CrtYd", "B.Fab"}
_STANDARD_FIELD_NAMES = {
    "reference",
    "value",
    "footprint",
}


@dataclass(frozen=True, slots=True)
class KiCadPnpEntry:
    """Placement record consumed by the shared BOM/PnP normalization layer."""

    designator: str
    comment: str
    layer: str
    footprint: str
    center_x: float
    center_y: float
    rotation: float
    description: str = ""
    parameters: dict[str, str] = field(default_factory=dict)


@dataclass(slots=True)
class KiCadManufacturingDesign:
    """Manufacturing command surface over a KiCad design."""

    design: KiCadDesign
    source_path: Path

    @classmethod
    def from_file(cls, path: Path | str) -> KiCadManufacturingDesign:
        """Load a KiCad project, schematic, or PCB file for manufacturing output."""
        source_path = Path(path).resolve()
        return cls(design=KiCadDesign.from_file(source_path), source_path=source_path)

    @property
    def project(self) -> object | None:
        """Expose the project object for shared output-template helpers."""
        return self.design.project

    def get_variants(self) -> list[str]:
        """Return available KiCad build variant names."""
        catalog = VariantCatalog.discover(
            project=self.design.project,
            schematic=self.design.top_schematic,
            pcb=self.design.pcb,
            warn_on_inconsistency=False,
        )
        return catalog.names

    def to_bom(self, variant: str | None = None) -> list[dict[str, object]]:
        """Return schematic-sourced BOM dictionaries."""
        schematic = self.design.top_schematic
        if schematic is None:
            raise ValueError("BOM generation requires a .kicad_sch or .kicad_pro input")

        sheet_by_ref = _sheet_path_by_reference(schematic, variant)
        rows: list[dict[str, object]] = []
        for component in assemble(schematic, self.design.pcb, variant):
            if not component.effective_in_bom:
                continue

            symbol = component.symbol
            footprint = component.footprint
            symbol_fields = dict(getattr(symbol, "fields", {}) or {})
            footprint_fields = dict(getattr(footprint, "fields", {}) or {})
            parameters = _merged_parameters(symbol_fields, footprint_fields)
            value = _first_text(
                symbol.value if symbol is not None else "",
                symbol_fields.get("Value", ""),
                footprint_fields.get("Value", ""),
            )
            footprint_name = _first_text(
                symbol_fields.get("Footprint", ""),
                footprint.footprint_lib if footprint is not None else "",
            )
            description = _first_text(
                symbol_fields.get("Description", ""),
                footprint_fields.get("Description", ""),
            )
            rows.append(
                {
                    "designator": component.reference,
                    "value": value,
                    "footprint": footprint_name,
                    "library_ref": symbol.lib_id if symbol is not None else footprint_name,
                    "description": description,
                    "sheet": sheet_by_ref.get(component.reference, ""),
                    "parameters": parameters,
                    "dnp": component.effective_dnp,
                }
            )
        return rows

    def to_pnp(
        self,
        variant: str | None = None,
        units: str = "mm",
        exclude_no_bom: bool = False,
        position_mode: str = "component-center",
    ) -> list[KiCadPnpEntry]:
        """Return PCB-sourced placement entries.

        KiCad PCB files carry one placement point per footprint, matching
        KiCad's own ASCII position-file exporter. The current ``position_mode``
        is ``component-center`` and resolves to that footprint coordinate.
        Coordinates intentionally match the appz ``bom_cruncher`` KiCad path:
        footprint positions are exported relative to KiCad's aux axis origin,
        also called the drill/place file origin, with KiCad's positive-down
        board Y converted to the shared positive-up placement frame.
        """
        del position_mode
        schematic = self.design.top_schematic
        pcb = self.design.pcb
        if pcb is None:
            raise ValueError("PnP generation requires a .kicad_pcb or .kicad_pro input")

        scale = 1.0 if units == "mm" else 1.0 / _MM_PER_MIL
        raw_footprints = _raw_footprint_by_ref(pcb)
        origin_x_mm, origin_y_mm = _aux_axis_origin_mm(pcb)
        entries: list[KiCadPnpEntry] = []
        for component in assemble(schematic, pcb, variant):
            footprint = component.footprint
            if footprint is None:
                continue
            if not _include_pnp_component(component, exclude_no_bom=exclude_no_bom):
                continue

            raw_fp = raw_footprints.get(component.reference)
            center_x_mm, center_y_mm = _footprint_position_from_aux_origin(
                raw_fp,
                origin_x_mm=origin_x_mm,
                origin_y_mm=origin_y_mm,
            )
            footprint_fields = dict(footprint.fields)
            symbol_fields = dict(getattr(component.symbol, "fields", {}) or {})
            parameters = _merged_parameters(symbol_fields, footprint_fields)
            entries.append(
                KiCadPnpEntry(
                    designator=component.reference,
                    comment=_first_text(
                        footprint_fields.get("Value", ""),
                        symbol_fields.get("Value", ""),
                    ),
                    layer=_placement_layer(raw_fp, footprint),
                    footprint=footprint.footprint_lib,
                    center_x=center_x_mm * scale,
                    center_y=center_y_mm * scale,
                    rotation=_normalize_degrees(_footprint_angle(raw_fp)),
                    description=_first_text(
                        footprint_fields.get("Description", ""),
                        symbol_fields.get("Description", ""),
                    ),
                    parameters=parameters,
                )
            )
        return entries


def _include_pnp_component(
    component: AssemblyComponent,
    *,
    exclude_no_bom: bool,
) -> bool:
    """Return whether an assembled component should appear in PnP output."""
    return (
        component.effective_in_pos_files
        and not component.effective_dnp
        and component.effective_on_board
        and (component.effective_in_bom or not exclude_no_bom)
    )


def _sheet_path_by_reference(schematic: object, variant: str | None) -> dict[str, str]:
    """Return a reference-to-KiCad-sheet-path lookup for BOM rows."""
    from kicad_monkey import resolve_symbol

    lookup: dict[str, str] = {}
    walk_symbols = getattr(schematic, "walk_symbols", None)
    if not callable(walk_symbols):
        return lookup
    symbol_items = cast(Iterable[tuple[object, str | None, object]], walk_symbols())
    for symbol, sheet_path, _owner in symbol_items:
        effective = resolve_symbol(symbol, variant, sheet_path=sheet_path)
        if effective.reference:
            lookup[effective.reference] = sheet_path or ""
    return lookup


def _raw_footprint_by_ref(pcb: object) -> dict[str, object]:
    """Return raw PCB footprint objects keyed by reference designator."""
    result: dict[str, object] = {}
    for footprint in getattr(pcb, "footprints", []) or []:
        reference = _footprint_property(footprint, "Reference")
        if reference:
            result[reference] = footprint
    return result


def _footprint_property(footprint: object, name: str) -> str:
    get_property_value = getattr(footprint, "get_property_value", None)
    if callable(get_property_value):
        return str(get_property_value(name, "") or "")
    for prop in getattr(footprint, "properties", []) or []:
        if getattr(prop, "name", "") == name:
            return str(getattr(prop, "value", "") or "")
    return ""


def _aux_axis_origin_mm(pcb: object) -> tuple[float, float]:
    """Return KiCad's aux axis/drill-place file origin in board millimeters."""
    origin = getattr(pcb, "aux_axis_origin_mm", (0.0, 0.0))
    if callable(origin):
        origin = origin()
    try:
        origin_x, origin_y = origin  # type: ignore[misc]
    except (TypeError, ValueError):
        return (0.0, 0.0)
    return (float(origin_x or 0.0), float(origin_y or 0.0))


def _footprint_position_from_aux_origin(
    footprint: object | None,
    *,
    origin_x_mm: float,
    origin_y_mm: float,
) -> tuple[float, float]:
    """Match bom_cruncher's KiCad PnP coordinate conversion."""
    if footprint is None:
        return (0.0, 0.0)
    return (_footprint_x(footprint) - origin_x_mm, origin_y_mm - _footprint_y(footprint))


def _footprint_x(footprint: object | None) -> float:
    return float(getattr(footprint, "at_x", 0.0) or 0.0)


def _footprint_y(footprint: object | None) -> float:
    return float(getattr(footprint, "at_y", 0.0) or 0.0)


def _footprint_angle(footprint: object | None) -> float:
    return float(getattr(footprint, "at_angle", 0.0) or 0.0)


def _placement_layer(
    raw_footprint: object | None,
    effective_footprint: EffectiveFootprintProperties,
) -> str:
    raw_layer = str(getattr(raw_footprint, "layer", "") or "")
    if raw_layer in _FRONT_LAYERS:
        return "top"
    if raw_layer in _BACK_LAYERS:
        return "bottom"
    field_layer = str(effective_footprint.fields.get("Layer", "") or "")
    if field_layer in _BACK_LAYERS:
        return "bottom"
    return "top"


def _normalize_degrees(value: float) -> float:
    normalized = value % 360.0
    return 0.0 if abs(normalized - 360.0) < 1e-9 else normalized


def _first_text(*values: object) -> str:
    """Return the first non-empty stripped string."""
    for value in values:
        text = str(value or "").strip()
        if text:
            return text
    return ""


def _merged_parameters(
    *field_maps: Mapping[str, object],
) -> dict[str, str]:
    """Merge KiCad symbol/footprint fields into string parameters."""
    merged: dict[str, str] = {}
    for fields in field_maps:
        for name, value in fields.items():
            key = str(name or "")
            if not key or key.casefold() in _STANDARD_FIELD_NAMES:
                continue
            if key not in merged and str(value or ""):
                merged[key] = str(value)
    return merged

"""Reviewable public API contract for promoted KiCad Monkey exports."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from importlib import import_module
from typing import Any


class PublicApiStratum(StrEnum):
    """High-level API areas used for review and signoff."""

    FOUNDATION = "foundation"
    SCHEMATIC = "schematic"
    PCB = "pcb"
    PROJECT = "project"
    RENDERING = "rendering"
    UTILITIES = "utilities"


@dataclass(frozen=True, slots=True)
class PublicApiExport:
    """One promoted root-level public API export."""

    name: str
    stratum: PublicApiStratum
    requires_marker: bool = False


PUBLIC_API_EXPORTS: tuple[PublicApiExport, ...] = (
    # Package metadata.
    PublicApiExport("__version__", PublicApiStratum.FOUNDATION),
    PublicApiExport("Version", PublicApiStratum.FOUNDATION),
    PublicApiExport("parse_version", PublicApiStratum.FOUNDATION),
    PublicApiExport("version", PublicApiStratum.FOUNDATION),
    # Low-level parsing and generic object query.
    PublicApiExport("parse_sexp", PublicApiStratum.FOUNDATION),
    PublicApiExport("build_sexp", PublicApiStratum.FOUNDATION),
    PublicApiExport("format_sexp", PublicApiStratum.FOUNDATION),
    PublicApiExport("SexpSelector", PublicApiStratum.FOUNDATION, requires_marker=True),
    PublicApiExport("SexpFormSpan", PublicApiStratum.FOUNDATION, requires_marker=True),
    PublicApiExport("iter_sexp_form_spans", PublicApiStratum.FOUNDATION),
    PublicApiExport("iter_sexp_file_form_spans", PublicApiStratum.FOUNDATION),
    PublicApiExport("parse_sexp_span", PublicApiStratum.FOUNDATION),
    PublicApiExport("iter_kicad_objects_from_text", PublicApiStratum.FOUNDATION),
    PublicApiExport("iter_kicad_objects_from_file", PublicApiStratum.FOUNDATION),
    PublicApiExport("find_element", PublicApiStratum.FOUNDATION),
    PublicApiExport("find_all_elements", PublicApiStratum.FOUNDATION),
    PublicApiExport("get_value", PublicApiStratum.FOUNDATION),
    PublicApiExport("get_values", PublicApiStratum.FOUNDATION),
    PublicApiExport("replace_element", PublicApiStratum.FOUNDATION),
    PublicApiExport("remove_element", PublicApiStratum.FOUNDATION),
    PublicApiExport("set_value", PublicApiStratum.FOUNDATION),
    PublicApiExport(
        "KiCadObjectCollection",
        PublicApiStratum.FOUNDATION,
        requires_marker=True,
    ),
    # Schematic, schematic symbol, and symbol-library OOP facades.
    PublicApiExport("KiCadSchematic", PublicApiStratum.SCHEMATIC, requires_marker=True),
    PublicApiExport("SchSymbol", PublicApiStratum.SCHEMATIC, requires_marker=True),
    PublicApiExport("SchSheet", PublicApiStratum.SCHEMATIC, requires_marker=True),
    PublicApiExport(
        "KiCadSymbolLib",
        PublicApiStratum.SCHEMATIC,
        requires_marker=True,
    ),
    PublicApiExport("LibSymbol", PublicApiStratum.SCHEMATIC, requires_marker=True),
    PublicApiExport("LibSubSymbol", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymProperty", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymPin", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymRectangle", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymCircle", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymArc", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymPolyline", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymBezier", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymText", PublicApiStratum.SCHEMATIC),
    PublicApiExport("SymTextBox", PublicApiStratum.SCHEMATIC),
    PublicApiExport("StandardPropertyKey", PublicApiStratum.SCHEMATIC),
    PublicApiExport("StandardSheetPropertyKey", PublicApiStratum.SCHEMATIC),
    PublicApiExport("PropertyId", PublicApiStratum.SCHEMATIC),
    PublicApiExport("PinElectricalType", PublicApiStratum.SCHEMATIC),
    PublicApiExport("PinGraphicStyle", PublicApiStratum.SCHEMATIC),
    PublicApiExport("LabelShape", PublicApiStratum.SCHEMATIC),
    # PCB and footprint OOP facades.
    PublicApiExport("KiCadPcb", PublicApiStratum.PCB, requires_marker=True),
    PublicApiExport("KiCadPcbProjection", PublicApiStratum.PCB, requires_marker=True),
    PublicApiExport("ProjectedSource", PublicApiStratum.PCB, requires_marker=True),
    PublicApiExport("PcbModelReference", PublicApiStratum.PCB, requires_marker=True),
    PublicApiExport("KiCadFootprint", PublicApiStratum.PCB, requires_marker=True),
    PublicApiExport("Footprint", PublicApiStratum.PCB, requires_marker=True),
    PublicApiExport("Pad", PublicApiStratum.PCB),
    PublicApiExport("FpText", PublicApiStratum.PCB),
    PublicApiExport("FpLine", PublicApiStratum.PCB),
    PublicApiExport("FpTextBox", PublicApiStratum.PCB),
    PublicApiExport("GrLine", PublicApiStratum.PCB),
    PublicApiExport("GrText", PublicApiStratum.PCB),
    PublicApiExport("GrTextBox", PublicApiStratum.PCB),
    PublicApiExport("Net", PublicApiStratum.PCB),
    PublicApiExport("Layer", PublicApiStratum.PCB),
    PublicApiExport("KICAD_COPPER_GEOMETRY_SCHEMA", PublicApiStratum.PCB),
    PublicApiExport("KICAD_COPPER_GEOMETRY_ACCEPTED_SCHEMAS", PublicApiStratum.PCB),
    PublicApiExport("COPPER_FEATURE_KINDS", PublicApiStratum.PCB),
    PublicApiExport("COPPER_DRILL_KINDS", PublicApiStratum.PCB),
    PublicApiExport(
        "KiCadCopperGeometryDocument",
        PublicApiStratum.PCB,
        requires_marker=True,
    ),
    PublicApiExport(
        "KiCadCopperFeature",
        PublicApiStratum.PCB,
        requires_marker=True,
    ),
    PublicApiExport(
        "KiCadCopperDrill",
        PublicApiStratum.PCB,
        requires_marker=True,
    ),
    PublicApiExport(
        "KiCadCopperLayer",
        PublicApiStratum.PCB,
        requires_marker=True,
    ),
    PublicApiExport(
        "KiCadCopperNet",
        PublicApiStratum.PCB,
        requires_marker=True,
    ),
    PublicApiExport("emit_pcb_copper_geometry", PublicApiStratum.PCB),
    # Project and design-level aggregate facades.
    PublicApiExport("KiCadProject", PublicApiStratum.PROJECT, requires_marker=True),
    PublicApiExport("KiCadProjectSidecar", PublicApiStratum.PROJECT),
    PublicApiExport("ProjectVariant", PublicApiStratum.PROJECT),
    PublicApiExport("KiCadDesign", PublicApiStratum.PROJECT, requires_marker=True),
    PublicApiExport(
        "KiCadSchematicInstance",
        PublicApiStratum.PROJECT,
        requires_marker=True,
    ),
    PublicApiExport("find_adjacent_kicad_project_path", PublicApiStratum.PROJECT),
    # IR and SVG rendering entry points.
    PublicApiExport("KICAD_PLOTTER_IR_SCHEMA", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadPlotterDocument", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadPlotterRecord", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadPlotterOp", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadPlotterOpKind", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadFillType", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadLineStyle", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadHorizAlign", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadVertAlign", PublicApiStratum.RENDERING),
    PublicApiExport("KiCadPenAction", PublicApiStratum.RENDERING),
    PublicApiExport("schematic_to_ir", PublicApiStratum.RENDERING),
    PublicApiExport("lib_symbol_to_ir", PublicApiStratum.RENDERING),
    PublicApiExport("footprint_to_ir", PublicApiStratum.RENDERING),
    PublicApiExport("pcb_to_ir", PublicApiStratum.RENDERING),
    PublicApiExport("render_ir_to_svg", PublicApiStratum.RENDERING),
    PublicApiExport("render_pcb_ir_to_svg", PublicApiStratum.RENDERING),
    PublicApiExport("render_schematic_svg", PublicApiStratum.RENDERING),
    PublicApiExport("render_symbol_svg", PublicApiStratum.RENDERING),
    PublicApiExport("render_library_svg", PublicApiStratum.RENDERING),
    PublicApiExport("SCHEMATIC_SVG_COLOR_ROLES", PublicApiStratum.RENDERING),
    PublicApiExport("SCHEMATIC_SVG_ROLE_COLORS", PublicApiStratum.RENDERING),
    PublicApiExport(
        "SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS",
        PublicApiStratum.RENDERING,
    ),
    PublicApiExport("normalize_schematic_svg_color_role", PublicApiStratum.RENDERING),
    PublicApiExport("schematic_svg_role_color", PublicApiStratum.RENDERING),
    PublicApiExport("schematic_svg_role_color_overrides", PublicApiStratum.RENDERING),
    # File-level utilities that remain package-level public API.
    PublicApiExport("read_kicad_pro_parameters", PublicApiStratum.UTILITIES),
    PublicApiExport("KiCadEnvironment", PublicApiStratum.UTILITIES),
    PublicApiExport("KiCadFilterPipeline", PublicApiStratum.UTILITIES),
    PublicApiExport("KiCadNameIndex", PublicApiStratum.UTILITIES),
    PublicApiExport("extract_footprints_from_text", PublicApiStratum.UTILITIES),
    PublicApiExport("extract_symbols_from_text", PublicApiStratum.UTILITIES),
    PublicApiExport("extract_step_from_text", PublicApiStratum.UTILITIES),
)

PUBLIC_API_ROOT_NAMES: tuple[str, ...] = tuple(
    export.name for export in PUBLIC_API_EXPORTS
)
PUBLIC_API_MARKER_ROOT_NAMES: tuple[str, ...] = tuple(
    export.name for export in PUBLIC_API_EXPORTS if export.requires_marker
)


def iter_public_api_exports() -> tuple[PublicApiExport, ...]:
    """Return the promoted API exports in review order."""
    return PUBLIC_API_EXPORTS


def resolve_public_api_root(name: str) -> Any:
    """Resolve a promoted public API export from the package root."""
    if name not in PUBLIC_API_ROOT_NAMES:
        raise KeyError(f"{name!r} is not a promoted public API root")
    package = import_module("kicad_monkey")
    return getattr(package, name)


def collect_public_api_contract_failures() -> list[str]:
    """Return reviewable failures for the promoted public API surface."""
    failures: list[str] = []
    package = import_module("kicad_monkey")
    package_exports = tuple(getattr(package, "__all__", ()))
    package_export_set = set(package_exports)

    if len(package_exports) != len(package_export_set):
        failures.append("kicad_monkey.__all__ contains duplicate names")

    if len(PUBLIC_API_ROOT_NAMES) != len(set(PUBLIC_API_ROOT_NAMES)):
        failures.append("PUBLIC_API_ROOT_NAMES contains duplicate names")

    for export in PUBLIC_API_EXPORTS:
        if export.name not in package_export_set:
            failures.append(f"{export.name}: missing from kicad_monkey.__all__")
            continue

        try:
            obj = getattr(package, export.name)
        except Exception as exc:
            failures.append(f"{export.name}: failed to resolve: {exc!r}")
            continue

        if export.requires_marker and getattr(obj, "__public_api__", False) is not True:
            failures.append(f"{export.name}: missing __public_api__ marker")

    return failures


__all__ = [
    "PUBLIC_API_EXPORTS",
    "PUBLIC_API_MARKER_ROOT_NAMES",
    "PUBLIC_API_ROOT_NAMES",
    "PublicApiExport",
    "PublicApiStratum",
    "collect_public_api_contract_failures",
    "iter_public_api_exports",
    "resolve_public_api_root",
]

"""BOM command for kicad_cruncher."""

import argparse
import csv
import json
import logging
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Protocol

from kicad_cruncher.bom_pnp_cli_common import (
    configured_output_root,
    load_optional_bom_pnp_config,
    load_or_create_bom_pnp_config,
    project_parameters_from_design,
    warn_for_unknown_variants,
    write_config_template_if_requested,
    write_used_config_snapshot,
)
from kicad_cruncher.bom_pnp_model import (
    BOM_GROUPED_DEFAULT_COLUMNS,
    BOM_PNP_DEFAULT_CONFIG_NAME,
    JLC_BOM_COLUMNS,
    BomPnpConfig,
    GroupedBomLine,
    NormalizedBomComponent,
    bom_raw_payload,
    configured_output_file,
    designator_sort_key,
    filter_bom_components,
    flat_raw_bom_payload,
    group_bom_components,
    grouped_bom_payload,
    grouped_bom_table_rows,
    jlc_bom_rows,
    make_pcb_line_item,
    normalize_bom_components,
    ordered_bom_lines,
    select_variant_names,
)
from kicad_cruncher.kicad_cruncher_common import (
    find_kicad_project_in_cwd,
    resolve_output_dir,
)
from kicad_cruncher.kicad_manufacturing_design import KiCadManufacturingDesign
from kicad_cruncher.output_path_templates import TemplateValue
from kicad_cruncher.simple_xlsx import write_xlsx_table

log = logging.getLogger(__name__)

BOM_CSV_ENCODING = "utf-8-sig"

BOM_FIXED_COLUMNS = [
    "Designator",
    "Value",
    "Footprint",
    "Library Ref",
    "Description",
    "Sheet",
    "DNP",
]


class _BomDesign(Protocol):
    """Protocol for the design methods used by configured BOM output."""

    def to_bom(self, variant: str | None = None) -> list[dict]:
        """Return schematic-sourced BOM dictionaries."""
        ...

    def to_pnp(
        self,
        variant: str | None = None,
        units: str = "mm",
        exclude_no_bom: bool = False,
        position_mode: str = "component-center",
    ) -> Sequence[object]:
        """Return PCB-sourced placement entries."""
        ...


def _bom_parameter_columns(bom: list[dict]) -> list[str]:
    all_params = set()
    for comp in bom:
        all_params.update(comp.get("parameters", {}).keys())
    return sorted(all_params)


def _bom_rows(
    bom: list[dict], *, param_columns: list[str] | None = None
) -> list[list[str]]:
    columns = (
        param_columns if param_columns is not None else _bom_parameter_columns(bom)
    )
    rows: list[list[str]] = []
    for comp in sorted(
        bom,
        key=lambda c: designator_sort_key(str(c.get("designator", ""))),
    ):
        params = comp.get("parameters", {})
        rows.append(
            [
                str(comp.get("designator", "")),
                str(comp.get("value", "")),
                str(comp.get("footprint", "")),
                str(comp.get("library_ref", "")),
                str(comp.get("description", "")),
                str(comp.get("sheet", "")),
                "Yes" if comp.get("dnp") else "No",
                *[str(params.get(param_name, "")) for param_name in columns],
            ]
        )
    return rows


def _write_bom_csv(output_file: Path, bom: list[dict]) -> None:
    param_columns = _bom_parameter_columns(bom)
    with open(output_file, "w", newline="", encoding=BOM_CSV_ENCODING) as f:
        writer = csv.writer(f)
        writer.writerow(BOM_FIXED_COLUMNS + param_columns)
        writer.writerows(_bom_rows(bom, param_columns=param_columns))


def _write_named_rows_csv(
    output_file: Path,
    columns: Sequence[str],
    rows: Sequence[Mapping[str, str]],
) -> None:
    """Write named rows to CSV using a fixed column order."""
    with open(output_file, "w", newline="", encoding=BOM_CSV_ENCODING) as f:
        writer = csv.DictWriter(f, fieldnames=list(columns), extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def _bom_output_extension(output_format: str) -> str:
    """Return the file extension for a BOM output format."""
    json_formats = {
        "json",
        "raw-json",
        "generic-json",
        "legacy-json",
        "grouped-json",
    }
    if output_format in json_formats:
        return "json"
    if output_format in {"xlsx", "grouped-xlsx", "jlc-xlsx"}:
        return "xlsx"
    return "csv"


def _write_bom_output(
    output_file: Path,
    bom: list[dict],
    *,
    output_format: str,
    source: Path,
    variant: str | None,
) -> None:
    """Write one BOM output artifact for the selected format."""
    if output_format == "json":
        with open(output_file, "w", encoding="utf-8") as f:
            json.dump(bom, f, indent=2)
        return
    if output_format == "raw-json":
        with open(output_file, "w", encoding="utf-8") as f:
            json.dump(flat_raw_bom_payload(bom), f, indent=2)
        return
    if output_format in {"generic-json", "legacy-json"}:
        payload = _generic_bom_payload(bom, source=source, variant=variant)
        with open(output_file, "w", encoding="utf-8") as f:
            json.dump(payload, f, indent=2)
        return
    if output_format == "grouped-json":
        normalized = normalize_bom_components(bom)
        lines = group_bom_components(normalized)
        payload = grouped_bom_payload(lines, source=source, variant=variant)
        with open(output_file, "w", encoding="utf-8") as f:
            json.dump(payload, f, indent=2)
        return
    if output_format == "grouped-csv":
        normalized = normalize_bom_components(bom)
        lines = group_bom_components(normalized)
        rows = grouped_bom_table_rows(lines)
        _write_named_rows_csv(output_file, BOM_GROUPED_DEFAULT_COLUMNS, rows)
        return
    if output_format == "grouped-xlsx":
        normalized = normalize_bom_components(bom)
        lines = group_bom_components(normalized)
        rows = grouped_bom_table_rows(lines)
        write_xlsx_table(
            output_file,
            columns=BOM_GROUPED_DEFAULT_COLUMNS,
            rows=rows,
            sheet_name="BOM",
        )
        return
    if output_format == "jlc-csv":
        normalized = normalize_bom_components(bom)
        lines = group_bom_components(normalized)
        rows = jlc_bom_rows(lines)
        _write_named_rows_csv(output_file, JLC_BOM_COLUMNS, rows)
        return
    if output_format == "jlc-xlsx":
        normalized = normalize_bom_components(bom)
        lines = group_bom_components(normalized)
        rows = jlc_bom_rows(lines)
        write_xlsx_table(
            output_file,
            columns=JLC_BOM_COLUMNS,
            rows=rows,
            sheet_name="JLC BOM",
        )
        return
    if output_format == "xlsx":
        _write_bom_xlsx(output_file, bom)
        return
    _write_bom_csv(output_file, bom)


def _configured_bom_artifacts(
    output_root: Path,
    raw_bom: list[dict],
    *,
    config: BomPnpConfig,
    source: Path,
    variant: str | None,
    project_parameters: Mapping[str, TemplateValue],
    output_kinds: Sequence[str] | None = None,
    command: str = "bom",
) -> list[Path]:
    """Write all configured BOM artifacts and return their paths."""
    kinds = tuple(output_kinds or config.bom_outputs)
    components = normalize_bom_components(raw_bom, config.field_aliases)
    components = filter_bom_components(components, include_dnp=config.include_dnp)
    pcb_line = make_pcb_line_item(
        config,
        project_parameters,
        variant_name=variant,
    )
    if pcb_line is not None:
        components.append(pcb_line)
    lines = group_bom_components(
        components,
        group_fields=config.bom_group_fields,
        split_dnp=config.split_dnp,
        prefix_order=config.prefix_order,
    )
    written: list[Path] = []
    for output_kind in kinds:
        output_file = configured_output_file(
            output_root,
            config,
            source=source,
            command=command,
            output_kind=output_kind,
            extension=_bom_output_extension(output_kind),
            project_parameters=project_parameters,
            variant_name=variant,
        )
        _write_configured_bom_artifact(
            output_file,
            output_kind,
            raw_bom=raw_bom,
            components=components,
            lines=lines,
            config=config,
            source=source,
            variant=variant,
        )
        write_used_config_snapshot(output_file, config)
        written.append(output_file)
    return written


def _write_configured_bom_artifact(
    output_file: Path,
    output_kind: str,
    *,
    raw_bom: list[dict],
    components: Sequence[NormalizedBomComponent],
    lines: Sequence[GroupedBomLine],
    config: BomPnpConfig,
    source: Path,
    variant: str | None,
) -> None:
    """Write one configured BOM artifact."""
    if output_kind == "raw-json":
        output_file.write_text(
            json.dumps(flat_raw_bom_payload(raw_bom), indent=2),
            encoding="utf-8",
        )
        return
    if output_kind == "legacy-json":
        payload = _generic_bom_payload(raw_bom, source=source, variant=variant)
        output_file.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        return
    if output_kind == "grouped-json":
        payload = grouped_bom_payload(
            ordered_bom_lines(lines, dnp_placement=config.dnp_placement),
            source=source,
            variant=variant,
        )
        output_file.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        return
    if output_kind == "grouped-csv":
        ordered_lines = ordered_bom_lines(
            lines,
            dnp_placement=config.dnp_placement,
        )
        rows = grouped_bom_table_rows(
            ordered_lines,
            fields=config.bom_output_fields,
            dnp_placement=None,
        )
        _write_named_rows_csv(output_file, config.bom_output_fields, rows)
        return
    if output_kind == "grouped-xlsx":
        ordered_lines = ordered_bom_lines(
            lines,
            dnp_placement=config.dnp_placement,
        )
        rows = grouped_bom_table_rows(
            ordered_lines,
            fields=config.bom_output_fields,
            dnp_placement=None,
        )
        write_xlsx_table(
            output_file,
            columns=config.bom_output_fields,
            rows=rows,
            sheet_name="BOM",
            highlighted_rows=[line.dnp for line in ordered_lines]
            if config.highlight_dnp_rows
            else (),
        )
        return
    if output_kind == "jlc-csv":
        rows = jlc_bom_rows(
            ordered_bom_lines(lines, dnp_placement=config.dnp_placement),
            include_dnp=config.include_dnp,
        )
        _write_named_rows_csv(output_file, JLC_BOM_COLUMNS, rows)
        return
    if output_kind == "jlc-xlsx":
        rows = jlc_bom_rows(
            ordered_bom_lines(lines, dnp_placement=config.dnp_placement),
            include_dnp=config.include_dnp,
        )
        write_xlsx_table(
            output_file,
            columns=JLC_BOM_COLUMNS,
            rows=rows,
            sheet_name="JLC BOM",
        )
        return
    raise ValueError(f"Unsupported configured BOM output: {output_kind}")


def _entry_field(entry: object, name: str) -> object:
    """Read one field from a mapping or object-like entry."""
    if isinstance(entry, Mapping):
        return entry.get(name)
    return getattr(entry, name, None)


def _pnp_entry_to_bom_dict(entry: object) -> dict[str, object]:
    """Convert a placement entry into PCB-sourced BOM-like component data."""
    parameters = _entry_field(entry, "parameters")
    if not isinstance(parameters, Mapping):
        parameters = {}
    return {
        "designator": str(_entry_field(entry, "designator") or ""),
        "value": str(_entry_field(entry, "comment") or ""),
        "footprint": str(_entry_field(entry, "footprint") or ""),
        "library_ref": "",
        "description": str(_entry_field(entry, "description") or ""),
        "sheet": "",
        "parameters": {str(key): str(value) for key, value in parameters.items()},
        "dnp": False,
    }


def _bom_from_configured_source(
    design: _BomDesign,
    config: BomPnpConfig,
    *,
    variant: str | None,
) -> list[dict]:
    """Return BOM rows from the selected configured data source."""
    if config.bom_source_mode == "pcb":
        pnp_entries = design.to_pnp(
            variant=variant,
            units="mm",
            position_mode=config.pnp_position_mode,
            exclude_no_bom=True,
        )
        return [_pnp_entry_to_bom_dict(entry) for entry in pnp_entries]
    return design.to_bom(variant=variant)


def _generic_bom_payload(
    bom: list[dict],
    *,
    source: Path,
    variant: str | None,
) -> dict:
    param_columns = _bom_parameter_columns(bom)
    rows = _bom_rows(bom, param_columns=param_columns)
    columns = BOM_FIXED_COLUMNS + param_columns
    components = [
        {column: row[index] for index, column in enumerate(columns)} for row in rows
    ]
    normalized_components = normalize_bom_components(bom)
    return {
        "schema": "wn.kicad_cruncher.bom.v1",
        "source": {
            "path": str(source),
            "name": source.name,
            "stem": source.stem,
        },
        "variant": variant,
        "component_count": len(bom),
        "dnp_count": sum(1 for component in bom if component.get("dnp")),
        "columns": columns,
        "parameter_columns": param_columns,
        "components": components,
        "raw_components": bom,
        "normalized": bom_raw_payload(
            normalized_components,
            source=source,
            variant=variant,
        ),
    }


def _write_bom_xlsx(output_file: Path, bom: list[dict]) -> None:
    param_columns = _bom_parameter_columns(bom)
    columns = [
        "DNP",
        "Designator",
        "Value",
        "Footprint",
        "Library Ref",
        "Description",
        "Sheet",
        *param_columns,
    ]
    sorted_bom = sorted(
        bom,
        key=lambda component: designator_sort_key(str(component.get("designator", ""))),
    )
    rows = [
        _legacy_bom_xlsx_row(component, param_columns) for component in sorted_bom
    ]
    write_xlsx_table(
        output_file,
        columns=columns,
        rows=rows,
        sheet_name="BOM",
        highlighted_rows=[bool(component.get("dnp")) for component in sorted_bom],
    )


def _legacy_bom_xlsx_row(
    component: Mapping[str, object],
    param_columns: Sequence[str],
) -> dict[str, str]:
    """Return one legacy BOM XLSX row with DNP as the first review column."""
    parameters = component.get("parameters", {})
    if not isinstance(parameters, Mapping):
        parameters = {}
    row = {
        "DNP": "Yes" if component.get("dnp") else "No",
        "Designator": str(component.get("designator", "")),
        "Value": str(component.get("value", "")),
        "Footprint": str(component.get("footprint", "")),
        "Library Ref": str(component.get("library_ref", "")),
        "Description": str(component.get("description", "")),
        "Sheet": str(component.get("sheet", "")),
    }
    row.update(
        {param_name: str(parameters.get(param_name, "")) for param_name in param_columns}
    )
    return row


def cmd_bom(args: argparse.Namespace) -> int:
    """
    Handle bom subcommand - generate BOM from KiCad schematic/project files.

    REQ-CLI-004: BOM generation with variant support (CSV or JSON format).

    Args:
        args: Parsed argparse namespace with file and output options.

    Returns:
        Exit code (0 for success, 1 for error).
    """
    if write_config_template_if_requested(
        getattr(args, "write_config", None),
        getattr(args, "file", None),
        log,
    ):
        return 0

    input_file = _resolve_bom_input_file(getattr(args, "file", None))
    if input_file is None:
        return 1
    design = KiCadManufacturingDesign.from_file(input_file)

    available_variants = design.get_variants()
    _log_available_variants(available_variants)

    requested_format = _requested_bom_format(args)
    config, config_mode = _load_bom_command_config(args, requested_format)
    variants_to_process = select_variant_names(
        available_variants,
        config,
        cli_variant=getattr(args, "variant", None),
        cli_all_variants=getattr(args, "all_variants", False),
    )
    warn_for_unknown_variants(log, variants_to_process, available_variants)

    project_parameters = project_parameters_from_design(design)
    output_dir = (
        configured_output_root(args.output)
        if config_mode
        else resolve_output_dir(args.output, "bom")
    )

    files_written = 0
    for var in variants_to_process:
        files_written += _write_bom_variant(
            output_dir,
            input_file,
            design,
            config=config,
            config_mode=config_mode,
            requested_format=requested_format,
            variant=var,
            project_parameters=project_parameters,
        )

    log.info(f"Generated {files_written} BOM file(s) in {output_dir}")
    return 0


def _requested_bom_format(args: argparse.Namespace) -> str | None:
    """Return the explicit single-output format, if supplied."""
    requested_format_value = getattr(args, "format", None)
    return requested_format_value if isinstance(requested_format_value, str) else None


def _load_bom_command_config(
    args: argparse.Namespace,
    requested_format: str | None,
) -> tuple[BomPnpConfig, bool]:
    """Load BOM/PnP config and return ``(config, config_mode)``."""
    if requested_format is None:
        config, config_path, created_config = load_or_create_bom_pnp_config(
            getattr(args, "config", None)
        )
        if created_config:
            log.info("Created BOM/PnP config template: %s", config_path)
        return config, True
    config, _config_path = load_optional_bom_pnp_config(getattr(args, "config", None))
    return config, False


def _log_available_variants(available_variants: Sequence[str]) -> None:
    """Log the variant catalog found in the loaded design."""
    if available_variants:
        log.info("Available variants: %s", ", ".join(available_variants))
    else:
        log.info("No variants defined in project")


def _write_bom_variant(
    output_dir: Path,
    input_file: Path,
    design: KiCadManufacturingDesign,
    *,
    config: BomPnpConfig,
    config_mode: bool,
    requested_format: str | None,
    variant: str | None,
    project_parameters: Mapping[str, TemplateValue],
) -> int:
    """Write configured or legacy BOM artifacts for one variant."""
    bom = _bom_from_configured_source(design, config, variant=variant)
    if config_mode:
        written = _configured_bom_artifacts(
            output_dir,
            bom,
            config=config,
            source=input_file,
            variant=variant,
            project_parameters=project_parameters,
        )
        output_names = ", ".join(path.name for path in written)
        files_written = len(written)
    else:
        assert requested_format is not None
        output_file = _legacy_bom_output_path(
            output_dir,
            input_file,
            variant=variant,
            output_format=requested_format,
        )
        _write_bom_output(
            output_file,
            bom,
            output_format=requested_format,
            source=input_file,
            variant=variant,
        )
        output_names = output_file.name
        files_written = 1

    log.info("BOM (%s): %s components -> %s", variant or "base", len(bom), output_names)
    dnp_count = sum(1 for component in bom if component["dnp"])
    if dnp_count > 0:
        log.info("  DNP (Do Not Populate): %s", dnp_count)
    return files_written


def _legacy_bom_output_path(
    output_dir: Path,
    input_file: Path,
    *,
    variant: str | None,
    output_format: str,
) -> Path:
    """Return the legacy single-format BOM output path."""
    ext = _bom_output_extension(output_format)
    variant_part = f"_{variant}" if variant else ""
    return output_dir / f"{input_file.stem}{variant_part}_bom.{ext}"


def _resolve_bom_input_file(file_arg: str | None) -> Path | None:
    """Resolve an explicit or auto-detected KiCad BOM input file."""
    if file_arg:
        input_file = Path(file_arg).resolve()
        if not input_file.exists():
            log.error("File not found: %s", input_file)
            return None
    else:
        input_file = find_kicad_project_in_cwd()
        if input_file is None:
            schematics = sorted(
                path for path in Path.cwd().glob("*.kicad_sch") if path.is_file()
            )
            input_file = schematics[0] if len(schematics) == 1 else None
        if input_file is None:
            log.error(
                "No file specified and no single .kicad_pro/.kicad_sch found "
                "in current directory"
            )
            log.info("Usage: kicad-cruncher bom [project.kicad_pro | schematic.kicad_sch]")
            return None
        log.info("Auto-detected KiCad input: %s", input_file.name)

    if input_file.suffix.lower() not in {".kicad_pro", ".kicad_sch", ".kicad_pcb"}:
        log.error("Unsupported file type: %s", input_file.suffix)
        log.info("Supported types: .kicad_pro, .kicad_sch, .kicad_pcb")
        return None
    return input_file


def register_parser(
    subparsers: argparse._SubParsersAction,
) -> argparse.ArgumentParser:
    # bom subcommand - Generate BOM from KiCad schematic/project files
    bom_parser = subparsers.add_parser(
        "bom",
        help="generate BOM from KiCad schematic/project files (CSV, JSON, or XLSX)",
        description="Generate Bill of Materials (BOM) from KiCad .kicad_sch or "
        ".kicad_pro files. "
        "CSV/XLSX formats include parameters as columns; JSON preserves nested structure. "
        "Config-driven runs can emit raw JSON, grouped tables, and JLCPCB BOM "
        "upload columns in one invocation. Config group_fields controls which "
        "canonical fields must match before parts collapse into one line item.",
        epilog="Examples:\n"
        "  kicad-cruncher bom project.kicad_pro\n"
        "  kicad-cruncher bom schematic.kicad_sch\n"
        "  kicad-cruncher bom                                  # Auto-detect .kicad_pro\n"
        "  kicad-cruncher bom project.kicad_pro --variant V1   # Single variant\n"
        "  kicad-cruncher bom project.kicad_pro --all-variants # All variants\n"
        "  kicad-cruncher bom project.kicad_pro --format json  # JSON output\n"
        "  kicad-cruncher bom project.kicad_pro --format generic-json\n"
        "  kicad-cruncher bom project.kicad_pro --format grouped-json\n"
        "  kicad-cruncher bom project.kicad_pro --format jlc-csv\n"
        "  kicad-cruncher bom project.kicad_pro --format jlc-xlsx\n"
        "  kicad-cruncher bom project.kicad_pro --format xlsx  # XLSX output\n"
        "  kicad-cruncher bom --write-config bom.config\n"
        "  kicad-cruncher bom project.kicad_pro --config bom.config\n"
        "  kicad-cruncher bom project.kicad_pro -o output_dir/",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    bom_parser.add_argument(
        "file",
        nargs="?",
        help="KiCad .kicad_sch, .kicad_pro, or .kicad_pcb file (optional if one .kicad_pro in CWD)",
    )
    bom_parser.add_argument(
        "-o", "--output", type=Path, help="output directory (default: ./output/bom)"
    )
    bom_parser.add_argument(
        "--format",
        choices=[
            "csv",
            "json",
            "raw-json",
            "generic-json",
            "legacy-json",
            "grouped-json",
            "grouped-csv",
            "grouped-xlsx",
            "jlc-csv",
            "jlc-xlsx",
            "xlsx",
        ],
        default=None,
        help="single output format; overrides multi-output config mode",
    )
    bom_parser.add_argument(
        "--config",
        type=Path,
        help=(
            "BOM/PnP JSON/JSONC config "
            "(default: ./bom.config; created when omitted in config mode)"
        ),
    )
    bom_parser.add_argument(
        "--write-config",
        nargs="?",
        const=Path(BOM_PNP_DEFAULT_CONFIG_NAME),
        type=Path,
        metavar="PATH",
        help="write a documented JSONC BOM/PnP config template",
    )
    bom_parser.add_argument(
        "--variant", type=str, help="filter by specific variant name"
    )
    bom_parser.add_argument(
        "--all-variants",
        action="store_true",
        help="generate BOM for all variants (plus base)",
    )
    bom_parser.set_defaults(handler=cmd_bom)
    return bom_parser

"""PnP command for kicad_cruncher."""

import argparse
import csv
import json
import logging
from collections.abc import Mapping, Sequence
from pathlib import Path

from kicad_cruncher.bom_pnp_cli_common import (
    configured_output_root,
    load_optional_bom_pnp_config,
    project_parameters_from_design,
    warn_for_unknown_variants,
    write_config_template_if_requested,
    write_used_config_snapshot,
)
from kicad_cruncher.bom_pnp_model import (
    BOM_PNP_DEFAULT_CONFIG_NAME,
    JLC_CPL_COLUMNS,
    PNP_POSITION_MODES,
    BomPnpConfig,
    NormalizedPlacement,
    configured_output_file,
    jlc_cpl_rows,
    normalize_pnp_entries,
    normalize_pnp_position_mode,
    pnp_payload,
    pnp_table_rows,
    select_variant_names,
    sort_placements,
)
from kicad_cruncher.kicad_cruncher_common import (
    find_kicad_project_in_cwd,
    resolve_output_dir,
)
from kicad_cruncher.kicad_manufacturing_design import KiCadManufacturingDesign
from kicad_cruncher.output_path_templates import TemplateValue
from kicad_cruncher.simple_xlsx import write_xlsx_table

log = logging.getLogger(__name__)

PNP_CSV_ENCODING = "utf-8-sig"


PNP_FIXED_COLUMNS = [
    "Designator",
    "Comment",
    "Layer",
    "Footprint",
    "Center-X({units})",
    "Center-Y({units})",
    "Rotation",
    "Description",
]


def _write_pnp_csv(
    output_file: Path,
    placements: Sequence[object],
    *,
    units: str,
) -> None:
    """Write normalized PnP rows with parameters flattened into columns."""
    normalized = normalize_pnp_entries(placements, units=units)
    param_columns = sorted(
        {param_name for entry in normalized for param_name in entry.parameters}
    )
    fixed_columns = [column.format(units=units) for column in PNP_FIXED_COLUMNS]

    with open(output_file, "w", newline="", encoding=PNP_CSV_ENCODING) as f:
        writer = csv.writer(f)
        writer.writerow(fixed_columns + param_columns)
        for entry in sort_placements(normalized):
            row = [
                entry.designator,
                entry.comment,
                entry.layer,
                entry.footprint,
                f"{entry.center_x:.4f}",
                f"{entry.center_y:.4f}",
                f"{entry.rotation:.2f}",
                entry.description,
            ]
            row.extend(
                entry.parameters.get(param_name, "") for param_name in param_columns
            )
            writer.writerow(row)


def _write_jlc_cpl_csv(
    output_file: Path,
    placements: Sequence[object],
    *,
    units: str,
) -> None:
    """Write normalized placements in JLCPCB CPL upload format."""
    normalized = normalize_pnp_entries(placements, units=units)
    rows = jlc_cpl_rows(normalized)
    with open(output_file, "w", newline="", encoding=PNP_CSV_ENCODING) as f:
        writer = csv.DictWriter(
            f,
            fieldnames=list(JLC_CPL_COLUMNS),
            extrasaction="ignore",
        )
        writer.writeheader()
        writer.writerows(rows)


def _write_named_rows_csv(
    output_file: Path,
    columns: Sequence[str],
    rows: Sequence[Mapping[str, str]],
) -> None:
    """Write named rows to CSV using a fixed column order."""
    with open(output_file, "w", newline="", encoding=PNP_CSV_ENCODING) as f:
        writer = csv.DictWriter(f, fieldnames=list(columns), extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def _pnp_output_extension(output_format: str) -> str:
    """Return the file extension for a PnP output format."""
    if output_format in {"json"}:
        return "json"
    if output_format in {"xlsx", "jlc-cpl-xlsx"}:
        return "xlsx"
    return "csv"


def _write_pnp_xlsx(
    output_file: Path,
    placements: Sequence[object],
    *,
    units: str,
) -> None:
    """Write normalized PnP rows as a single-sheet XLSX workbook."""
    normalized = normalize_pnp_entries(placements, units=units)
    param_columns = sorted(
        {param_name for entry in normalized for param_name in entry.parameters}
    )
    columns = [column.format(units=units) for column in PNP_FIXED_COLUMNS]
    rows = []
    for entry in sort_placements(normalized):
        row: dict[str, str] = {
            columns[0]: entry.designator,
            columns[1]: entry.comment,
            columns[2]: entry.layer,
            columns[3]: entry.footprint,
            columns[4]: f"{entry.center_x:.4f}",
            columns[5]: f"{entry.center_y:.4f}",
            columns[6]: f"{entry.rotation:.2f}",
            columns[7]: entry.description,
        }
        row.update(
            {
                param_name: entry.parameters.get(param_name, "")
                for param_name in param_columns
            }
        )
        rows.append(row)
    write_xlsx_table(
        output_file,
        columns=(*columns, *param_columns),
        rows=rows,
        sheet_name="PnP",
    )


def _configured_pnp_artifacts(
    output_root: Path,
    placements: Sequence[object],
    *,
    config: BomPnpConfig,
    source: Path,
    variant: str | None,
    units: str,
    position_mode: str,
    project_parameters: Mapping[str, TemplateValue],
    output_kinds: Sequence[str] | None = None,
    command: str = "pnp",
) -> list[Path]:
    """Write all configured PnP artifacts and return their paths."""
    kinds = tuple(output_kinds or config.pnp_outputs)
    normalized = normalize_pnp_entries(
        placements,
        units=units,
        aliases=config.field_aliases,
    )
    written: list[Path] = []
    for output_kind in kinds:
        output_file = configured_output_file(
            output_root,
            config,
            source=source,
            command=command,
            output_kind=output_kind,
            extension=_pnp_output_extension(output_kind),
            project_parameters=project_parameters,
            variant_name=variant,
        )
        _write_configured_pnp_artifact(
            output_file,
            output_kind,
            normalized=normalized,
            config=config,
            source=source,
            variant=variant,
            units=units,
            position_mode=position_mode,
        )
        write_used_config_snapshot(output_file, config)
        written.append(output_file)
    return written


def _write_configured_pnp_artifact(
    output_file: Path,
    output_kind: str,
    *,
    normalized: Sequence[NormalizedPlacement],
    config: BomPnpConfig,
    source: Path,
    variant: str | None,
    units: str,
    position_mode: str,
) -> None:
    """Write one configured PnP artifact."""
    if output_kind == "json":
        payload = pnp_payload(
            normalized,
            source=source,
            variant=variant,
            units=units,
            position_mode=position_mode,
            layer_order=config.layer_order,
            prefix_order=config.prefix_order,
        )
        output_file.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        return
    if output_kind == "csv":
        rows = pnp_table_rows(
            normalized,
            fields=config.pnp_output_fields,
            layer_order=config.layer_order,
            prefix_order=config.prefix_order,
        )
        _write_named_rows_csv(output_file, config.pnp_output_fields, rows)
        return
    if output_kind == "xlsx":
        rows = pnp_table_rows(
            normalized,
            fields=config.pnp_output_fields,
            layer_order=config.layer_order,
            prefix_order=config.prefix_order,
        )
        write_xlsx_table(
            output_file,
            columns=config.pnp_output_fields,
            rows=rows,
            sheet_name="PnP",
        )
        return
    if output_kind == "jlc-cpl":
        rows = jlc_cpl_rows(
            normalized,
            layer_order=config.layer_order,
            prefix_order=config.prefix_order,
        )
        _write_named_rows_csv(output_file, JLC_CPL_COLUMNS, rows)
        return
    if output_kind == "jlc-cpl-xlsx":
        rows = jlc_cpl_rows(
            normalized,
            layer_order=config.layer_order,
            prefix_order=config.prefix_order,
        )
        write_xlsx_table(
            output_file,
            columns=JLC_CPL_COLUMNS,
            rows=rows,
            sheet_name="JLC CPL",
        )
        return
    raise ValueError(f"Unsupported configured PnP output: {output_kind}")


def _pnp_format_option_error(output_format: str, units: str) -> str:
    """Return an option error message for incompatible PnP options."""
    if output_format in {"jlc-cpl", "jlc-cpl-xlsx"} and units != "mm":
        return "JLC CPL output requires --units mm because JLCPCB CPL uses mm"
    return ""


def _write_legacy_pnp_output(
    output_dir: Path,
    input_file: Path,
    placements: Sequence[object],
    *,
    output_format: str,
    variant: str | None,
    units: str,
    position_mode: str,
) -> Path:
    """Write one legacy single-format PnP output and return its path."""
    ext = _pnp_output_extension(output_format)
    variant_part = f"_{variant}" if variant else ""
    output_file = output_dir / f"{input_file.stem}{variant_part}_pnp.{ext}"

    if output_format == "json":
        normalized = normalize_pnp_entries(placements, units=units)
        output_data = pnp_payload(
            normalized,
            source=input_file,
            variant=variant,
            units=units,
            position_mode=position_mode,
        )
        output_file.write_text(json.dumps(output_data, indent=2), encoding="utf-8")
        return output_file
    if output_format == "jlc-cpl":
        _write_jlc_cpl_csv(output_file, placements, units=units)
        return output_file
    if output_format == "jlc-cpl-xlsx":
        normalized = normalize_pnp_entries(placements, units=units)
        rows = jlc_cpl_rows(normalized)
        write_xlsx_table(
            output_file,
            columns=JLC_CPL_COLUMNS,
            rows=rows,
            sheet_name="JLC CPL",
        )
        return output_file
    if output_format == "xlsx":
        _write_pnp_xlsx(output_file, placements, units=units)
        return output_file
    _write_pnp_csv(output_file, placements, units=units)
    return output_file


def cmd_pnp(args: argparse.Namespace) -> int:
    """
    Handle pnp subcommand - generate Pick-and-Place from KiCad project/PCB files.

    REQ-CLI-005: PnP generation with variant support (CSV or JSON format).

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

    input_file = _resolve_pnp_input_file(getattr(args, "file", None))
    if input_file is None:
        return 1
    design = KiCadManufacturingDesign.from_file(input_file)

    available_variants = design.get_variants()
    _log_available_variants(available_variants)

    config, config_mode = _load_pnp_command_config(args)
    variants_to_process = select_variant_names(
        available_variants,
        config,
        cli_variant=getattr(args, "variant", None),
        cli_all_variants=getattr(args, "all_variants", False),
    )
    warn_for_unknown_variants(log, variants_to_process, available_variants)

    units = _pnp_units(args, config)
    position_mode = _pnp_position_mode(args, config)
    exclude_no_bom = _pnp_exclude_no_bom(args, config)
    output_format = getattr(args, "format", None) or "csv"
    option_error = _pnp_command_option_error(
        config,
        config_mode=config_mode,
        output_format=output_format,
        units=units,
    )
    if option_error:
        log.error(option_error)
        return 1
    output_dir = (
        configured_output_root(args.output)
        if config_mode
        else resolve_output_dir(args.output, "pnp")
    )
    project_parameters = project_parameters_from_design(design)

    files_written = 0
    for var in variants_to_process:
        variant_files = _write_pnp_variant(
            output_dir,
            input_file,
            design,
            config=config,
            config_mode=config_mode,
            output_format=output_format,
            variant=var,
            units=units,
            position_mode=position_mode,
            exclude_no_bom=exclude_no_bom,
            project_parameters=project_parameters,
        )
        if variant_files is None:
            return 1
        files_written += variant_files

    log.info(f"Generated {files_written} PnP file(s) in {output_dir}")
    return 0


def _load_pnp_command_config(args: argparse.Namespace) -> tuple[BomPnpConfig, bool]:
    """Load optional PnP config and return ``(config, config_mode)``."""
    config, config_path = load_optional_bom_pnp_config(getattr(args, "config", None))
    config_mode = config_path is not None and getattr(args, "format", None) is None
    return config, config_mode


def _log_available_variants(available_variants: Sequence[str]) -> None:
    """Log the variant catalog found in the loaded design."""
    if available_variants:
        log.info("Available variants: %s", ", ".join(available_variants))
    else:
        log.info("No variants defined in project")


def _pnp_units(args: argparse.Namespace, config: BomPnpConfig) -> str:
    """Return the effective PnP coordinate units."""
    return getattr(args, "units", None) or config.pnp_units


def _pnp_position_mode(args: argparse.Namespace, config: BomPnpConfig) -> str:
    """Return the effective PnP position mode."""
    return normalize_pnp_position_mode(
        getattr(args, "position_mode", None) or config.pnp_position_mode
    )


def _pnp_exclude_no_bom(args: argparse.Namespace, config: BomPnpConfig) -> bool:
    """Return whether PnP output should omit no-BOM components."""
    return bool(getattr(args, "exclude_no_bom", False) or config.pnp_exclude_no_bom)


def _pnp_command_option_error(
    config: BomPnpConfig,
    *,
    config_mode: bool,
    output_format: str,
    units: str,
) -> str:
    """Return the effective option compatibility error, if any."""
    jlc_requested = config_mode and any(
        kind in {"jlc-cpl", "jlc-cpl-xlsx"} for kind in config.pnp_outputs
    )
    checked_format = "jlc-cpl-xlsx" if jlc_requested else output_format
    return _pnp_format_option_error(checked_format, units)


def _write_pnp_variant(
    output_dir: Path,
    input_file: Path,
    design: KiCadManufacturingDesign,
    *,
    config: BomPnpConfig,
    config_mode: bool,
    output_format: str,
    variant: str | None,
    units: str,
    position_mode: str,
    exclude_no_bom: bool,
    project_parameters: Mapping[str, TemplateValue],
) -> int | None:
    """Write configured or legacy PnP artifacts for one variant."""
    try:
        pnp = design.to_pnp(
            variant=variant,
            units=units,
            position_mode=position_mode,
            exclude_no_bom=exclude_no_bom,
        )
    except ValueError as e:
        log.error("PnP generation failed: %s", e)
        return None

    if config_mode:
        written = _configured_pnp_artifacts(
            output_dir,
            pnp,
            config=config,
            source=input_file,
            variant=variant,
            units=units,
            position_mode=position_mode,
            project_parameters=project_parameters,
        )
        output_names = ", ".join(path.name for path in written)
        files_written = len(written)
    else:
        output_file = _write_legacy_pnp_output(
            output_dir,
            input_file,
            pnp,
            output_format=output_format,
            variant=variant,
            units=units,
            position_mode=position_mode,
        )
        output_names = output_file.name
        files_written = 1

    log.info("PnP (%s): %s placements -> %s", variant or "base", len(pnp), output_names)
    return files_written


def _resolve_pnp_input_file(file_arg: str | None) -> Path | None:
    """Resolve an explicit or auto-detected KiCad PnP input file."""
    if file_arg:
        input_file = Path(file_arg).resolve()
        if not input_file.exists():
            log.error("File not found: %s", input_file)
            return None
    else:
        input_file = find_kicad_project_in_cwd()
        if input_file is None:
            pcbs = sorted(path for path in Path.cwd().glob("*.kicad_pcb") if path.is_file())
            input_file = pcbs[0] if len(pcbs) == 1 else None
        if input_file is None:
            log.error(
                "No file specified and no single .kicad_pro/.kicad_pcb found "
                "in current directory"
            )
            log.info("Usage: kicad-cruncher pnp [project.kicad_pro | board.kicad_pcb]")
            return None
        log.info("Auto-detected KiCad input: %s", input_file.name)

    if input_file.suffix.lower() not in {".kicad_pro", ".kicad_pcb"}:
        log.error(
            "PnP generation requires a .kicad_pro or .kicad_pcb file, got: %s",
            input_file.suffix,
        )
        log.info("Supported types: .kicad_pro, .kicad_pcb")
        return None
    return input_file


def register_parser(
    subparsers: argparse._SubParsersAction,
) -> argparse.ArgumentParser:
    # pnp subcommand - Generate Pick-and-Place from KiCad project/PCB files
    pnp_parser = subparsers.add_parser(
        "pnp",
        help="generate Pick-and-Place from KiCad project/PCB files (CSV, JSON, XLSX, or JLC CPL)",
        description="Generate Pick-and-Place (PnP) from KiCad .kicad_pro or "
        ".kicad_pcb files. Includes all component parameters. "
        "Config-driven runs can emit JSON, CSV, XLSX, and JLCPCB CPL "
        "upload columns in one invocation. Config output_fields can include "
        "generated placement fields such as center_x, center_y, rotation, "
        "and units, canonical fields such as manufacturer or mpn, and exact "
        "raw KiCad parameter names.",
        epilog="Examples:\n"
        "  kicad-cruncher pnp project.kicad_pro\n"
        "  kicad-cruncher pnp                                  # Auto-detect .kicad_pro\n"
        "  kicad-cruncher pnp project.kicad_pro --variant V1   # Single variant\n"
        "  kicad-cruncher pnp project.kicad_pro --all-variants # All variants\n"
        "  kicad-cruncher pnp project.kicad_pro --units mils   # Use mils instead of mm\n"
        "  kicad-cruncher pnp project.kicad_pro --position-mode component-center\n"
        "  kicad-cruncher pnp project.kicad_pro --format json  # JSON output\n"
        "  kicad-cruncher pnp project.kicad_pro --format xlsx\n"
        "  kicad-cruncher pnp project.kicad_pro --format jlc-cpl\n"
        "  kicad-cruncher pnp project.kicad_pro --format jlc-cpl-xlsx\n"
        "  kicad-cruncher pnp --write-config bom.config\n"
        "  kicad-cruncher pnp project.kicad_pro --config bom.config\n"
        "  kicad-cruncher pnp project.kicad_pro -o output_dir/",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    pnp_parser.add_argument(
        "file",
        nargs="?",
        help="KiCad .kicad_pro or .kicad_pcb file (optional if one .kicad_pro in CWD)",
    )
    pnp_parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="output directory (default: ./output/pnp)",
    )
    pnp_parser.add_argument(
        "--format",
        choices=["csv", "json", "xlsx", "jlc-cpl", "jlc-cpl-xlsx"],
        default=None,
        help="single output format; overrides multi-output config mode",
    )
    pnp_parser.add_argument(
        "--config",
        type=Path,
        help="BOM/PnP JSON/JSONC config (default: ./bom.config if present)",
    )
    pnp_parser.add_argument(
        "--write-config",
        nargs="?",
        const=Path(BOM_PNP_DEFAULT_CONFIG_NAME),
        type=Path,
        metavar="PATH",
        help="write a documented JSONC BOM/PnP config template",
    )
    pnp_parser.add_argument(
        "--variant",
        type=str,
        help="filter by specific variant name",
    )
    pnp_parser.add_argument(
        "--all-variants",
        action="store_true",
        help="generate PnP for all variants (plus base)",
    )
    pnp_parser.add_argument(
        "--units",
        choices=["mm", "mils"],
        default=None,
        help="coordinate units (default: config value or mm)",
    )
    pnp_parser.add_argument(
        "--position-mode",
        choices=list(PNP_POSITION_MODES),
        default=None,
        help="placement position mode (default: config value or component-center)",
    )
    pnp_parser.add_argument(
        "--exclude-no-bom",
        action="store_true",
        help="exclude placement-eligible parts that are not BOM-eligible",
    )
    pnp_parser.set_defaults(handler=cmd_pnp)
    return pnp_parser

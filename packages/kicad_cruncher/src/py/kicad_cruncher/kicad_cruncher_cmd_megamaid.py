"""Project-library extraction command."""

from __future__ import annotations

import argparse
import json
import logging
import time
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

from kicad_cruncher.kicad_cruncher_common import find_kicad_project_in_cwd, resolve_output_dir

if TYPE_CHECKING:
    from kicad_monkey.kicad_library_extraction import KiCadFootprintExtractionRecord

log = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class _LibraryOutputLayout:
    output_dir: Path
    symbols_dir: Path
    footprints_dir: Path
    models_dir: Path
    symbol_nickname: str
    footprint_nickname: str


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _resolve_input_project(file_arg: str | None) -> Path | None:
    if file_arg:
        path = Path(file_arg)
        if not path.exists():
            log.error("Input project does not exist: %s", path)
            return None
        if path.suffix != ".kicad_pro":
            log.error("library extraction requires a .kicad_pro input: %s", path)
            return None
        return path

    project = find_kicad_project_in_cwd()
    if project is None:
        log.error("No input provided and current directory does not contain exactly one .kicad_pro")
        return None
    return project


def _manifest_relative(path: Path, output_dir: Path) -> str:
    try:
        return str(path.relative_to(output_dir))
    except ValueError:
        return str(path)


def _resolve_library_output_dir(
    output_dir: Path,
    value: str | Path | None,
    default_name: str,
    *,
    ensure_pretty_suffix: bool = False,
) -> Path:
    raw = Path(value) if value is not None else Path(default_name)
    if ensure_pretty_suffix and raw.suffix != ".pretty":
        raw = raw.with_suffix(".pretty")
    return raw if raw.is_absolute() else output_dir / raw


def _default_library_nickname(value: str | None, library_dir: Path, *, pretty: bool = False) -> str:
    if value:
        return value
    return library_dir.stem if pretty else library_dir.name


def _resolve_library_layout(
    *,
    project_path: Path,
    output_dir: Path,
    project_stem_library_dirs: bool,
    symbol_library_dir: str | Path | None,
    footprint_library_dir: str | Path | None,
    symbol_library_name: str | None,
    footprint_library_name: str | None,
) -> _LibraryOutputLayout:
    default_symbol_dir = project_path.stem if project_stem_library_dirs else "symbols"
    default_footprint_dir = (
        f"{project_path.stem}.pretty" if project_stem_library_dirs else "footprints.pretty"
    )
    symbols_dir = _resolve_library_output_dir(output_dir, symbol_library_dir, default_symbol_dir)
    footprints_dir = _resolve_library_output_dir(
        output_dir,
        footprint_library_dir,
        default_footprint_dir,
        ensure_pretty_suffix=True,
    )
    return _LibraryOutputLayout(
        output_dir=output_dir,
        symbols_dir=symbols_dir,
        footprints_dir=footprints_dir,
        models_dir=output_dir / "models",
        symbol_nickname=_default_library_nickname(symbol_library_name, symbols_dir),
        footprint_nickname=_default_library_nickname(
            footprint_library_name,
            footprints_dir,
            pretty=True,
        ),
    )


def _manifest_payload(
    *,
    schema: str,
    project_path: Path,
    output_dir: Path,
    symbols_dir: Path,
    footprints_dir: Path,
    models_dir: Path,
    mode: str,
    symbol_files: tuple[Path, ...],
    footprint_files: tuple[Path, ...],
    model_files: tuple[Path, ...],
    metadata_path: Path,
    validation: dict[str, object] | None,
    library_tables: dict[str, object] | None,
) -> dict[str, object]:
    payload: dict[str, object] = {
        "schema": schema,
        "project": str(project_path),
        "mode": mode,
        "symbols": {
            "directory": _manifest_relative(symbols_dir, output_dir),
            "count": len(symbol_files),
            "files": [_manifest_relative(path, output_dir) for path in symbol_files],
        },
        "footprints": {
            "directory": _manifest_relative(footprints_dir, output_dir),
            "count": len(footprint_files),
            "files": [_manifest_relative(path, output_dir) for path in footprint_files],
        },
        "models": {
            "directory": _manifest_relative(models_dir, output_dir),
            "count": len(model_files),
            "files": [_manifest_relative(path, output_dir) for path in model_files],
        },
        "metadata": _manifest_relative(metadata_path, output_dir),
        "validation": validation,
    }
    if library_tables is not None:
        payload["library_tables"] = library_tables
    return payload


def _readme_text(manifest: dict[str, object], *, title: str) -> str:
    symbols = manifest["symbols"]
    footprints = manifest["footprints"]
    models = manifest["models"]
    assert isinstance(symbols, dict)
    assert isinstance(footprints, dict)
    assert isinstance(models, dict)
    library_tables = manifest.get("library_tables")
    table_text = ""
    mutation_text = (
        "This command is non-destructive. It does not edit the project, relink "
        "schematic symbols, relink PCB footprints, or update lib tables.\n"
    )
    if isinstance(library_tables, dict):
        table_text = f"- Project library tables: `{library_tables.get('changed')}` changed\n"
        mutation_text = (
            "This command updates the project-local `sym-lib-table` and "
            "`fp-lib-table` entries when missing. It does not relink schematic "
            "symbols or PCB footprints.\n"
        )

    return (
        f"# {title}\n\n"
        f"Source project: `{manifest['project']}`\n\n"
        f"Mode: `{manifest['mode']}`\n\n"
        "Generated artifacts:\n\n"
        f"- Symbols: `{symbols['directory']}` ({symbols['count']})\n"
        f"- Footprints: `{footprints['directory']}` ({footprints['count']})\n"
        f"- STEP models: `{models['directory']}` ({models['count']})\n"
        f"- Metadata: `{manifest['metadata']}`\n"
        f"{table_text}"
        f"- Manifest: `{manifest['manifest_filename']}`\n\n"
        f"{mutation_text}"
    )


def _validate_outputs(
    *,
    symbols_dir: Path,
    footprints_dir: Path,
    kicad_cli: Path | None,
) -> dict[str, object]:
    from kicad_monkey.kicad_library_extraction import (
        validate_pretty_library_with_kicad_cli,
        validate_symbol_library_with_kicad_cli,
    )

    symbol_results = [
        validate_symbol_library_with_kicad_cli(path, kicad_cli=kicad_cli).to_dict()
        for path in sorted(symbols_dir.glob("*.kicad_sym"))
    ]
    footprint_result = validate_pretty_library_with_kicad_cli(
        footprints_dir,
        kicad_cli=kicad_cli,
    ).to_dict()
    ok = all(bool(result["ok"]) for result in symbol_results) and bool(footprint_result["ok"])
    return {
        "ok": ok,
        "symbol_results": symbol_results,
        "footprint_result": footprint_result,
    }


def _validate_outputs_if_requested(
    *,
    args: argparse.Namespace,
    layout: _LibraryOutputLayout,
    command_label: str,
) -> dict[str, object] | None:
    if not bool(args.validate_kicad_cli):
        return None

    started = time.perf_counter()
    validation = _validate_outputs(
        symbols_dir=layout.symbols_dir,
        footprints_dir=layout.footprints_dir,
        kicad_cli=args.kicad_cli,
    )
    _log_stage_done(f"{command_label}: completed KiCad CLI validation", started)
    return validation


def _log_stage_done(message: str, started_at: float) -> None:
    log.info("%s in %.2fs", message, time.perf_counter() - started_at)


def _extract_model_files(
    *,
    args: argparse.Namespace,
    project_path: Path,
    layout: _LibraryOutputLayout,
    footprint_records: Iterable[KiCadFootprintExtractionRecord],
    command_label: str,
    embed_models: bool,
    write_models: bool,
    extract_all_embedded_models: bool,
    preserve_model_filenames: bool,
) -> tuple[Path, ...]:
    from kicad_monkey.kicad_library_extraction import (
        extract_3d_models,
        extract_3d_models_from_footprint_records,
    )

    if not write_models:
        log.info("%s: skipped STEP model folder export", command_label)
        return ()

    started = time.perf_counter()
    full_model_scan = extract_all_embedded_models or bool(args.all_embedded_models)
    if embed_models and not full_model_scan:
        model_files = extract_3d_models_from_footprint_records(
            footprint_records,
            layout.models_dir,
            dedupe_payloads=not preserve_model_filenames,
        )
    else:
        model_files = extract_3d_models(
            project_path,
            layout.models_dir,
            dedupe_payloads=not preserve_model_filenames,
        )
    _log_stage_done(f"{command_label}: wrote {len(model_files)} STEP models", started)
    return model_files


def _ensure_project_tables_if_requested(
    *,
    project_path: Path,
    layout: _LibraryOutputLayout,
    command_label: str,
    update_project_library_tables: bool,
) -> dict[str, object] | None:
    from kicad_monkey.kicad_project_libraries import ensure_project_library_table_entries

    if not update_project_library_tables:
        return None

    started = time.perf_counter()
    table_result = ensure_project_library_table_entries(
        project_path,
        symbol_library_path=layout.symbols_dir,
        footprint_library_path=layout.footprints_dir,
        symbol_nickname=layout.symbol_nickname,
        footprint_nickname=layout.footprint_nickname,
    )
    _log_stage_done(
        f"{command_label}: checked project library tables",
        started,
    )
    return table_result.to_dict()


def _run_library_extraction(
    args: argparse.Namespace,
    *,
    command_label: str,
    output_default: str,
    mode_value: str,
    dedupe_value: str,
    manifest_schema: str,
    manifest_filename: str,
    readme_title: str,
    update_project_library_tables: bool = False,
    project_stem_library_dirs: bool = False,
    symbol_library_dir: str | Path | None = None,
    footprint_library_dir: str | Path | None = None,
    symbol_library_name: str | None = None,
    footprint_library_name: str | None = None,
    footprint_dedupe_value: str | None = None,
    write_models: bool = True,
    extract_all_embedded_models: bool = False,
    preserve_model_filenames: bool = False,
) -> int:
    """Extract KiCad library artifacts from a project."""
    from kicad_monkey.kicad_library_extraction import (
        KiCadExtractionDedupePolicy,
        KiCadExtractionMode,
        extract_footprints,
        extract_symbols,
        write_extraction_metadata_bundle,
        write_pretty_library,
        write_symbol_folder_library,
    )

    project_path = _resolve_input_project(str(args.file) if args.file else None)
    if project_path is None:
        return 1

    output_dir = resolve_output_dir(args.output, output_default)
    layout = _resolve_library_layout(
        project_path=project_path,
        output_dir=output_dir,
        project_stem_library_dirs=project_stem_library_dirs,
        symbol_library_dir=symbol_library_dir,
        footprint_library_dir=footprint_library_dir,
        symbol_library_name=symbol_library_name,
        footprint_library_name=footprint_library_name,
    )
    mode = KiCadExtractionMode(mode_value)
    symbol_dedupe_policy = KiCadExtractionDedupePolicy(dedupe_value)
    footprint_dedupe_policy = KiCadExtractionDedupePolicy(footprint_dedupe_value or dedupe_value)
    embed_models = not bool(args.no_embed_models)
    embed_external_models = not bool(args.no_embed_external_models)

    try:
        log.info("%s: extracting from %s", command_label, project_path)
        started = time.perf_counter()
        symbol_records = extract_symbols(
            project_path,
            mode=mode,
            dedupe_policy=symbol_dedupe_policy,
        )
        _log_stage_done(f"{command_label}: extracted {len(symbol_records)} symbols", started)

        started = time.perf_counter()
        footprint_records = extract_footprints(
            project_path,
            mode=mode,
            embed_models=embed_models,
            embed_external_models=embed_external_models,
            dedupe_policy=footprint_dedupe_policy,
        )
        _log_stage_done(f"{command_label}: extracted {len(footprint_records)} footprints", started)

        started = time.perf_counter()
        symbol_files = write_symbol_folder_library(symbol_records, layout.symbols_dir)
        footprint_files = write_pretty_library(footprint_records, layout.footprints_dir)
        asset_count_message = (
            f"{command_label}: wrote {len(symbol_files)} symbol files "
            f"and {len(footprint_files)} footprint files"
        )
        _log_stage_done(
            asset_count_message,
            started,
        )

        model_files = _extract_model_files(
            args=args,
            project_path=project_path,
            layout=layout,
            footprint_records=footprint_records,
            command_label=command_label,
            embed_models=embed_models,
            write_models=write_models,
            extract_all_embedded_models=extract_all_embedded_models,
            preserve_model_filenames=preserve_model_filenames,
        )

        started = time.perf_counter()
        metadata_path = write_extraction_metadata_bundle(
            project_path,
            output_dir / "library_extraction.json",
            mode=mode,
            symbol_records=symbol_records,
            footprint_records=footprint_records,
            include_asset_scan=bool(args.include_asset_scan),
            schema="kicad_cruncher.library_extraction_bundle.a0",
        )
        _log_stage_done(f"{command_label}: wrote metadata", started)

        library_tables = _ensure_project_tables_if_requested(
            project_path=project_path,
            layout=layout,
            command_label=command_label,
            update_project_library_tables=update_project_library_tables,
        )
        validation = _validate_outputs_if_requested(
            args=args,
            layout=layout,
            command_label=command_label,
        )
        manifest = _manifest_payload(
            schema=manifest_schema,
            project_path=project_path,
            output_dir=layout.output_dir,
            symbols_dir=layout.symbols_dir,
            footprints_dir=layout.footprints_dir,
            models_dir=layout.models_dir,
            mode=mode.value,
            symbol_files=symbol_files,
            footprint_files=footprint_files,
            model_files=model_files,
            metadata_path=metadata_path,
            validation=validation,
            library_tables=library_tables,
        )
        manifest["manifest_filename"] = manifest_filename
        manifest_path = output_dir / manifest_filename
        _write_json(manifest_path, manifest)
        (output_dir / "README.md").write_text(
            _readme_text(manifest, title=readme_title),
            encoding="utf-8",
        )
    except Exception as exc:
        log.error("%s failed: %s", command_label, exc)
        return 1

    if validation is not None and not validation["ok"]:
        log.error("%s wrote artifacts but KiCad CLI validation failed", command_label)
        return 1

    log.info(
        "%s: %d symbols, %d footprints, %d STEP models -> %s",
        command_label,
        len(symbol_files),
        len(footprint_files),
        len(model_files),
        layout.output_dir,
    )
    return 0


def cmd_megamaid(args: argparse.Namespace) -> int:
    """Extract a cleaned library-ingestion bundle for lib_cruncher."""
    return _run_library_extraction(
        args,
        command_label="Megamaid",
        output_default="megamaid",
        mode_value="internal",
        dedupe_value=str(args.dedupe),
        manifest_schema="kicad_cruncher.megamaid_manifest.a0",
        manifest_filename="megamaid_manifest.json",
        readme_title="KiCad Megamaid Library Extraction",
    )


def _add_common_library_args(parser: argparse.ArgumentParser, *, output_default: str) -> None:
    parser.add_argument(
        "file",
        nargs="?",
        help="KiCad .kicad_pro project; optional when one .kicad_pro is in CWD",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help=f"output directory (default: ./output/{output_default})",
    )


def _add_model_validation_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--no-embed-models",
        action="store_true",
        help="do not copy board-level embedded model payloads into extracted footprints",
    )
    parser.add_argument(
        "--no-embed-external-models",
        action="store_true",
        help="do not embed resolvable external STEP/STP model references into extracted footprints",
    )
    parser.add_argument(
        "--all-embedded-models",
        action="store_true",
        help="scan the full project and write every embedded STEP/STP payload to models/",
    )
    parser.add_argument(
        "--include-asset-scan",
        action="store_true",
        help="include full project asset/model scan in library_extraction.json",
    )
    parser.add_argument(
        "--no-asset-scan",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--validate-kicad-cli",
        action="store_true",
        help="validate generated symbol and footprint libraries with kicad-cli upgrade",
    )
    parser.add_argument(
        "--kicad-cli",
        type=Path,
        help="explicit kicad-cli executable for --validate-kicad-cli",
    )


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the megamaid library-ingestion command parser."""
    parser = subparsers.add_parser(
        "megamaid",
        aliases=["library-extract", "lib-extract"],
        help="extract cleaned KiCad library artifacts for lib_cruncher",
        description=(
            "Extract cleaned symbols, footprints, metadata, and embedded STEP models "
            "from a KiCad project into a non-destructive lib_cruncher ingestion bundle."
        ),
    )
    _add_common_library_args(parser, output_default="megamaid")
    parser.add_argument(
        "--dedupe",
        choices=("name", "fingerprint"),
        default="name",
        help="dedupe policy for internal extraction",
    )
    _add_model_validation_args(parser)
    parser.set_defaults(handler=cmd_megamaid)
    return parser

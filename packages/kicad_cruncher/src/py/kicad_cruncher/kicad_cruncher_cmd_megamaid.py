"""Project-library extraction command."""

from __future__ import annotations

import argparse
import json
import logging
import time
from pathlib import Path

from kicad_cruncher.kicad_cruncher_common import find_kicad_project_in_cwd, resolve_output_dir

log = logging.getLogger(__name__)


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


def _manifest_payload(
    *,
    schema: str,
    project_path: Path,
    output_dir: Path,
    mode: str,
    symbol_files: tuple[Path, ...],
    footprint_files: tuple[Path, ...],
    model_files: tuple[Path, ...],
    metadata_path: Path,
    validation: dict[str, object] | None,
) -> dict[str, object]:
    return {
        "schema": schema,
        "project": str(project_path),
        "mode": mode,
        "symbols": {
            "directory": "symbols",
            "count": len(symbol_files),
            "files": [str(path.relative_to(output_dir)) for path in symbol_files],
        },
        "footprints": {
            "directory": "footprints.pretty",
            "count": len(footprint_files),
            "files": [str(path.relative_to(output_dir)) for path in footprint_files],
        },
        "models": {
            "directory": "models",
            "count": len(model_files),
            "files": [str(path.relative_to(output_dir)) for path in model_files],
        },
        "metadata": str(metadata_path.relative_to(output_dir)),
        "validation": validation,
    }


def _readme_text(manifest: dict[str, object]) -> str:
    symbols = manifest["symbols"]
    footprints = manifest["footprints"]
    models = manifest["models"]
    assert isinstance(symbols, dict)
    assert isinstance(footprints, dict)
    assert isinstance(models, dict)
    return (
        "# KiCad Megamaid Library Extraction\n\n"
        f"Source project: `{manifest['project']}`\n\n"
        f"Mode: `{manifest['mode']}`\n\n"
        "Generated artifacts:\n\n"
        f"- Symbols: `{symbols['directory']}` ({symbols['count']})\n"
        f"- Footprints: `{footprints['directory']}` ({footprints['count']})\n"
        f"- STEP models: `{models['directory']}` ({models['count']})\n"
        f"- Metadata: `{manifest['metadata']}`\n"
        f"- Manifest: `{manifest['manifest_filename']}`\n\n"
        "This command is non-destructive. It does not edit the project, relink "
        "schematic symbols, relink PCB footprints, or update lib tables.\n"
    )


def _validate_outputs(
    *,
    output_dir: Path,
    kicad_cli: Path | None,
) -> dict[str, object]:
    from kicad_monkey.kicad_library_extraction import (
        validate_pretty_library_with_kicad_cli,
        validate_symbol_library_with_kicad_cli,
    )

    symbols_dir = output_dir / "symbols"
    footprints_dir = output_dir / "footprints.pretty"
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


def _log_stage_done(message: str, started_at: float) -> None:
    log.info("%s in %.2fs", message, time.perf_counter() - started_at)


def _run_library_extraction(
    args: argparse.Namespace,
    *,
    command_label: str,
    output_default: str,
    mode_value: str,
    dedupe_value: str,
    manifest_schema: str,
    manifest_filename: str,
) -> int:
    """Extract KiCad library artifacts from a project."""
    from kicad_monkey.kicad_library_extraction import (
        KiCadExtractionDedupePolicy,
        KiCadExtractionMode,
        extract_3d_models,
        extract_3d_models_from_footprint_records,
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
    mode = KiCadExtractionMode(mode_value)
    dedupe_policy = KiCadExtractionDedupePolicy(dedupe_value)
    embed_models = not bool(args.no_embed_models)
    embed_external_models = not bool(args.no_embed_external_models)

    try:
        log.info("%s: extracting from %s", command_label, project_path)
        started = time.perf_counter()
        symbol_records = extract_symbols(project_path, mode=mode, dedupe_policy=dedupe_policy)
        _log_stage_done(f"{command_label}: extracted {len(symbol_records)} symbols", started)

        started = time.perf_counter()
        footprint_records = extract_footprints(
            project_path,
            mode=mode,
            embed_models=embed_models,
            embed_external_models=embed_external_models,
            dedupe_policy=dedupe_policy,
        )
        _log_stage_done(f"{command_label}: extracted {len(footprint_records)} footprints", started)

        started = time.perf_counter()
        symbol_files = write_symbol_folder_library(symbol_records, output_dir / "symbols")
        footprint_files = write_pretty_library(footprint_records, output_dir / "footprints.pretty")
        asset_count_message = (
            f"{command_label}: wrote {len(symbol_files)} symbol files "
            f"and {len(footprint_files)} footprint files"
        )
        _log_stage_done(
            asset_count_message,
            started,
        )

        started = time.perf_counter()
        if embed_models and not bool(args.all_embedded_models):
            model_files = extract_3d_models_from_footprint_records(
                footprint_records,
                output_dir / "models",
            )
        else:
            model_files = extract_3d_models(project_path, output_dir / "models")
        _log_stage_done(f"{command_label}: wrote {len(model_files)} STEP models", started)

        started = time.perf_counter()
        metadata_path = write_extraction_metadata_bundle(
            project_path,
            output_dir / "library_extraction.json",
            mode=mode,
            symbol_records=symbol_records,
            footprint_records=footprint_records,
            include_asset_scan=bool(args.include_asset_scan),
        )
        _log_stage_done(f"{command_label}: wrote metadata", started)

        started = time.perf_counter()
        validation = (
            _validate_outputs(output_dir=output_dir, kicad_cli=args.kicad_cli)
            if bool(args.validate_kicad_cli)
            else None
        )
        if validation is not None:
            _log_stage_done(f"{command_label}: completed KiCad CLI validation", started)
        manifest = _manifest_payload(
            schema=manifest_schema,
            project_path=project_path,
            output_dir=output_dir,
            mode=mode.value,
            symbol_files=symbol_files,
            footprint_files=footprint_files,
            model_files=model_files,
            metadata_path=metadata_path,
            validation=validation,
        )
        manifest["manifest_filename"] = manifest_filename
        manifest_path = output_dir / manifest_filename
        _write_json(manifest_path, manifest)
        (output_dir / "README.md").write_text(_readme_text(manifest), encoding="utf-8")
    except Exception as exc:
        log.error("Megamaid extraction failed: %s", exc)
        return 1

    if validation is not None and not validation["ok"]:
        log.error("Megamaid extraction wrote artifacts but KiCad CLI validation failed")
        return 1

    log.info(
        "%s: %d symbols, %d footprints, %d STEP models -> %s",
        command_label,
        len(symbol_files),
        len(footprint_files),
        len(model_files),
        output_dir,
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
        manifest_schema="kicad_cruncher.megamaid_manifest.v0",
        manifest_filename="megamaid_manifest.json",
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

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
from kicad_cruncher.logging_utils import stage_done_text, stage_progress_text, stage_start_text

if TYPE_CHECKING:
    from kicad_monkey.kicad_library_extraction import (
        KiCadEmbeddedAssetRecord,
        KiCadFootprintExtractionRecord,
        KiCadSymbolExtractionRecord,
    )

    from kicad_cruncher.kicad_cruncher_cmd_design import DesignReviewBundle

log = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class _LibraryOutputLayout:
    output_dir: Path
    symbols_dir: Path
    footprints_dir: Path
    models_dir: Path
    embedded_assets_dir: Path
    design_review_dir: Path
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
        return str(path.relative_to(output_dir)).replace("\\", "/")
    except ValueError:
        return str(path).replace("\\", "/")


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
        embedded_assets_dir=output_dir / "embedded_assets",
        design_review_dir=output_dir / "design_review",
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
    embedded_assets_dir: Path,
    mode: str,
    symbol_files: tuple[Path, ...],
    footprint_files: tuple[Path, ...],
    model_files: tuple[Path, ...],
    embedded_asset_records: tuple[KiCadEmbeddedAssetRecord, ...],
    design_review_bundle: DesignReviewBundle | None,
    metadata_path: Path,
    validation: dict[str, object] | None,
    library_tables: dict[str, object] | None,
    source_relink: dict[str, object] | None,
    source_relink_path: Path | None,
) -> dict[str, object]:
    embedded_asset_files = tuple(
        sorted(
            {Path(record.output_file) for record in embedded_asset_records if record.output_file},
            key=lambda path: str(path),
        )
    )
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
    if embedded_asset_records:
        payload["embedded_assets"] = {
            "directory": _manifest_relative(embedded_assets_dir, output_dir),
            "record_count": len(embedded_asset_records),
            "file_count": len(embedded_asset_files),
            "files": [_manifest_relative(path, output_dir) for path in embedded_asset_files],
            "records": [
                {
                    **record.to_dict(),
                    "output_file": _manifest_relative(Path(record.output_file), output_dir),
                }
                for record in embedded_asset_records
            ],
        }
    if design_review_bundle is not None:
        payload["design_review"] = {
            "directory": _manifest_relative(design_review_bundle.output_dir, output_dir),
            "manifest": _manifest_relative(design_review_bundle.manifest_path, output_dir),
            "readme": _manifest_relative(design_review_bundle.readme_path, output_dir),
            "design_json": _manifest_relative(design_review_bundle.design_json_path, output_dir),
            "netlist_json": _manifest_relative(design_review_bundle.netlist_json_path, output_dir),
            "netlist_kicad_sexpr": _manifest_relative(
                design_review_bundle.netlist_kicad_sexpr_path,
                output_dir,
            ),
            "component_count": design_review_bundle.component_count,
            "net_count": design_review_bundle.net_count,
            "schematic_svg_count": len(design_review_bundle.schematic_svgs),
            "pcb_svg_count": len(design_review_bundle.pcb_svgs),
            "schematic_svgs": [
                _manifest_relative(design_review_bundle.output_dir / str(item["file"]), output_dir)
                for item in design_review_bundle.schematic_svgs
            ],
            "pcb_svgs": [
                _manifest_relative(design_review_bundle.output_dir / str(item["file"]), output_dir)
                for item in design_review_bundle.pcb_svgs
            ],
        }
    if library_tables is not None:
        payload["library_tables"] = library_tables
    if source_relink is not None and source_relink_path is not None:
        summary = source_relink.get("summary", {})
        payload["source_relink"] = {
            "mode": source_relink.get("mode"),
            "changed": source_relink.get("changed"),
            "report": _manifest_relative(source_relink_path, output_dir),
            "summary": summary if isinstance(summary, dict) else {},
        }
    return payload


def _readme_text(manifest: dict[str, object], *, title: str) -> str:
    symbols = manifest["symbols"]
    footprints = manifest["footprints"]
    models = manifest["models"]
    assert isinstance(symbols, dict)
    assert isinstance(footprints, dict)
    assert isinstance(models, dict)
    embedded_assets = manifest.get("embedded_assets")
    embedded_assets_text = ""
    if isinstance(embedded_assets, dict):
        embedded_assets_text = (
            "- Embedded assets: "
            f"`{embedded_assets['directory']}` "
            f"({embedded_assets['file_count']} files, "
            f"{embedded_assets['record_count']} records)\n"
        )
    design_review = manifest.get("design_review")
    design_review_text = ""
    if isinstance(design_review, dict):
        design_review_text = (
            "- Design review: "
            f"`{design_review['directory']}` "
            f"({design_review['schematic_svg_count']} schematic SVGs, "
            f"{design_review['pcb_svg_count']} PCB SVGs, "
            f"{design_review['net_count']} nets)\n"
        )
    library_tables = manifest.get("library_tables")
    source_relink = manifest.get("source_relink")
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
    if isinstance(source_relink, dict):
        if source_relink.get("mode") == "apply":
            mutation_text = (
                "This command updated project-local library table entries when "
                "missing and relinked source schematic/PCB library references. "
                f"Review `{source_relink.get('report')}` for the exact changes.\n"
            )
        else:
            mutation_text = (
                "This command planned source schematic/PCB library relinks without "
                f"editing source files. Review `{source_relink.get('report')}`.\n"
            )

    return (
        f"# {title}\n\n"
        f"Source project: `{manifest['project']}`\n\n"
        f"Mode: `{manifest['mode']}`\n\n"
        "Generated artifacts:\n\n"
        f"- Symbols: `{symbols['directory']}` ({symbols['count']})\n"
        f"- Footprints: `{footprints['directory']}` ({footprints['count']})\n"
        f"- STEP models: `{models['directory']}` ({models['count']})\n"
        f"{embedded_assets_text}"
        f"{design_review_text}"
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
    _log_stage_start(f"{command_label}: validating generated libraries with KiCad CLI")
    validation = _validate_outputs(
        symbols_dir=layout.symbols_dir,
        footprints_dir=layout.footprints_dir,
        kicad_cli=args.kicad_cli,
    )
    _log_stage_done(f"{command_label}: completed KiCad CLI validation", started)
    return validation


def _log_stage_done(message: str, started_at: float) -> None:
    log.info("%s", stage_done_text(f"{message} in {time.perf_counter() - started_at:.2f}s"))


def _log_stage_start(message: str) -> None:
    log.info("%s", stage_start_text(message))


def _log_stage_progress(message: str) -> None:
    log.info("%s", stage_progress_text(message))


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
    if full_model_scan:
        _log_stage_start(f"{command_label}: scanning project for STEP/STP model payloads")
    else:
        _log_stage_start(f"{command_label}: writing STEP/STP models from extracted footprints")
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


def _extract_embedded_asset_records(
    *,
    project_path: Path,
    layout: _LibraryOutputLayout,
    command_label: str,
    write_embedded_assets: bool,
) -> tuple[KiCadEmbeddedAssetRecord, ...]:
    from kicad_monkey.kicad_library_extraction import extract_embedded_assets

    if not write_embedded_assets:
        return ()

    started = time.perf_counter()
    _log_stage_start(f"{command_label}: extracting embedded files, images, and bitmaps")
    asset_records = extract_embedded_assets(
        project_path,
        layout.embedded_assets_dir,
        progress=lambda message: _log_stage_progress(
            f"{command_label}: embedded assets: {message}"
        ),
    )
    _log_stage_done(
        f"{command_label}: wrote {len({record.output_file for record in asset_records})} "
        f"embedded asset files",
        started,
    )
    return asset_records


def _write_design_review_bundle_if_requested(
    *,
    project_path: Path,
    layout: _LibraryOutputLayout,
    command_label: str,
    write_design_review: bool,
) -> DesignReviewBundle | None:
    from kicad_cruncher.kicad_cruncher_cmd_design import write_design_review_bundle

    if not write_design_review:
        return None

    started = time.perf_counter()
    _log_stage_start(f"{command_label}: starting design review bundle")
    bundle = write_design_review_bundle(
        project_path,
        layout.design_review_dir,
        include_indexes=True,
        progress=lambda message: _log_stage_progress(f"{command_label}: design review: {message}"),
    )
    _log_stage_done(
        f"{command_label}: wrote design review bundle "
        f"({len(bundle.schematic_svgs)} schematic SVGs, {len(bundle.pcb_svgs)} PCB SVGs)",
        started,
    )
    return bundle


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
    _log_stage_start(f"{command_label}: checking project-local library tables")
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


def _source_relink_validation(source_relink: dict[str, object], key: str) -> dict[str, object]:
    validation = source_relink.get(key, {})
    return validation if isinstance(validation, dict) else {}


def _source_relink_validation_ok(source_relink: dict[str, object], key: str) -> bool:
    return bool(_source_relink_validation(source_relink, key).get("ok", True))


def _source_relink_remaining_issue_count(source_relink: dict[str, object], key: str) -> object:
    return _source_relink_validation(source_relink, key).get("remaining_issue_count", 0)


def _raise_if_source_relink_blocked(
    *,
    source_relink_mode: str,
    source_relink: dict[str, object],
    source_relink_path: Path,
) -> None:
    if source_relink_mode != "apply":
        return
    if _source_relink_validation_ok(source_relink, "cache_link_validation") and (
        _source_relink_validation_ok(source_relink, "cache_unit_validation")
    ):
        return
    raise RuntimeError(
        "source relink blocked by unresolved schematic cache issues "
        f"(links={_source_relink_remaining_issue_count(source_relink, 'cache_link_validation')}, "
        f"units={_source_relink_remaining_issue_count(source_relink, 'cache_unit_validation')}); "
        f"review {source_relink_path}"
    )


def _relink_project_sources_if_requested(
    *,
    project_path: Path,
    output_dir: Path,
    layout: _LibraryOutputLayout,
    command_label: str,
    source_relink_mode: str,
    source_relink_repair_cache_links: bool,
    symbol_records: Iterable[KiCadSymbolExtractionRecord],
    footprint_records: Iterable[KiCadFootprintExtractionRecord],
) -> tuple[dict[str, object] | None, Path | None]:
    if source_relink_mode == "none":
        return None, None

    from kicad_cruncher.kicad_cruncher_project_lib_relink import (
        build_footprint_relink_map,
        build_symbol_relink_map,
        relink_project_sources,
    )

    action_label = "planning source relinks"
    if source_relink_mode == "apply":
        action_label = "relinking source files"
    started = time.perf_counter()
    _log_stage_start(f"{command_label}: {action_label}")
    source_relink = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname=layout.symbol_nickname,
        footprint_library_nickname=layout.footprint_nickname,
        symbol_member_map=build_symbol_relink_map(symbol_records),
        footprint_member_map=build_footprint_relink_map(footprint_records),
        dry_run=source_relink_mode == "dry_run",
        repair_cache_links=source_relink_repair_cache_links,
        fail_on_cache_link_issues=source_relink_mode == "apply",
    )
    source_relink_path = output_dir / "source_relink.json"
    _write_json(source_relink_path, source_relink)
    summary = source_relink.get("summary", {})
    changes = summary.get("changes", 0) if isinstance(summary, dict) else 0
    _log_stage_done(
        f"{command_label}: {action_label} ({changes} changes)",
        started,
    )
    _raise_if_source_relink_blocked(
        source_relink_mode=source_relink_mode,
        source_relink=source_relink,
        source_relink_path=source_relink_path,
    )
    return source_relink, source_relink_path


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
    default_output_dir: str | Path | None = None,
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
    write_embedded_assets: bool = False,
    skip_power_symbols: bool = False,
    write_design_review: bool = False,
    source_relink_mode: str = "none",
    source_relink_repair_cache_links: bool = False,
) -> int:
    """Extract KiCad library artifacts from a project."""
    from kicad_monkey.kicad_library_extraction import (
        KiCadExtractionDedupePolicy,
        KiCadExtractionMode,
        build_footprint_library_link_map,
        extract_footprints,
        extract_symbols,
        write_extraction_metadata_bundle,
        write_pretty_library,
        write_symbol_folder_library,
    )

    project_path = _resolve_input_project(str(args.file) if args.file else None)
    if project_path is None:
        return 1

    output_dir = resolve_output_dir(
        args.output,
        output_default,
        default_dir=default_output_dir,
    )
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
        _log_stage_start(f"{command_label}: extracting from {project_path}")
        _log_stage_start(f"{command_label}: extracting schematic symbols")
        started = time.perf_counter()
        symbol_records = extract_symbols(
            project_path,
            mode=mode,
            dedupe_policy=symbol_dedupe_policy,
            skip_power_symbols=skip_power_symbols,
        )
        _log_stage_done(f"{command_label}: extracted {len(symbol_records)} symbols", started)

        _log_stage_start(f"{command_label}: extracting PCB footprints")
        started = time.perf_counter()
        footprint_records = extract_footprints(
            project_path,
            mode=mode,
            embed_models=embed_models,
            embed_external_models=embed_external_models,
            dedupe_policy=footprint_dedupe_policy,
        )
        _log_stage_done(f"{command_label}: extracted {len(footprint_records)} footprints", started)

        _log_stage_start(f"{command_label}: writing symbol and footprint libraries")
        started = time.perf_counter()
        symbol_write_kwargs = {}
        if mode == KiCadExtractionMode.PROJECT_LOCAL:
            symbol_write_kwargs = {
                "footprint_library_nickname": layout.footprint_nickname,
                "footprint_name_map": build_footprint_library_link_map(footprint_records),
            }
        symbol_files = write_symbol_folder_library(
            symbol_records,
            layout.symbols_dir,
            **symbol_write_kwargs,
        )
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
        embedded_asset_records = _extract_embedded_asset_records(
            project_path=project_path,
            layout=layout,
            command_label=command_label,
            write_embedded_assets=write_embedded_assets,
        )
        design_review_bundle = _write_design_review_bundle_if_requested(
            project_path=project_path,
            layout=layout,
            command_label=command_label,
            write_design_review=write_design_review,
        )

        _log_stage_start(f"{command_label}: writing library extraction metadata")
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
        source_relink, source_relink_path = _relink_project_sources_if_requested(
            project_path=project_path,
            output_dir=output_dir,
            layout=layout,
            command_label=command_label,
            source_relink_mode=source_relink_mode,
            source_relink_repair_cache_links=source_relink_repair_cache_links,
            symbol_records=symbol_records,
            footprint_records=footprint_records,
        )
        validation = _validate_outputs_if_requested(
            args=args,
            layout=layout,
            command_label=command_label,
        )
        _log_stage_start(f"{command_label}: writing manifest and README")
        started = time.perf_counter()
        manifest = _manifest_payload(
            schema=manifest_schema,
            project_path=project_path,
            output_dir=layout.output_dir,
            symbols_dir=layout.symbols_dir,
            footprints_dir=layout.footprints_dir,
            models_dir=layout.models_dir,
            embedded_assets_dir=layout.embedded_assets_dir,
            mode=mode.value,
            symbol_files=symbol_files,
            footprint_files=footprint_files,
            model_files=model_files,
            embedded_asset_records=embedded_asset_records,
            design_review_bundle=design_review_bundle,
            metadata_path=metadata_path,
            validation=validation,
            library_tables=library_tables,
            source_relink=source_relink,
            source_relink_path=source_relink_path,
        )
        manifest["manifest_filename"] = manifest_filename
        manifest_path = output_dir / manifest_filename
        _write_json(manifest_path, manifest)
        (output_dir / "README.md").write_text(
            _readme_text(manifest, title=readme_title),
            encoding="utf-8",
        )
        _log_stage_done(f"{command_label}: wrote manifest and README", started)
    except Exception as exc:
        log.error("%s failed: %s", command_label, exc)
        return 1

    if validation is not None and not validation["ok"]:
        log.error("%s wrote artifacts but KiCad CLI validation failed", command_label)
        return 1

    log.info(
        "%s",
        stage_done_text(
            f"{command_label}: {len(symbol_files)} symbols, {len(footprint_files)} footprints, "
            f"{len(model_files)} STEP models -> {layout.output_dir}"
        ),
    )
    return 0


def cmd_megamaid(args: argparse.Namespace) -> int:
    """Aggressively dissect a KiCad project into reviewable artifacts."""
    return _run_library_extraction(
        args,
        command_label="Megamaid",
        output_default="megamaid",
        mode_value="internal",
        dedupe_value=str(args.dedupe),
        manifest_schema="kicad_cruncher.megamaid_manifest.a0",
        manifest_filename="megamaid_manifest.json",
        readme_title="KiCad Megamaid Project Dissection",
        write_embedded_assets=True,
        write_design_review=True,
    )


def cmd_lib_extract(args: argparse.Namespace) -> int:
    """Extract a cleaned library-ingestion bundle for lib_cruncher."""
    return _run_library_extraction(
        args,
        command_label="Lib extract",
        output_default="lib-extract",
        mode_value="internal",
        dedupe_value=str(args.dedupe),
        manifest_schema="kicad_cruncher.lib_extract_manifest.a0",
        manifest_filename="lib_extract_manifest.json",
        readme_title="KiCad Lib Extract Library Extraction",
        skip_power_symbols=True,
    )


def _add_common_library_args(
    parser: argparse.ArgumentParser,
    *,
    output_default: str,
    default_output_dir: str | Path | None = None,
) -> None:
    default_help = f"./{default_output_dir}" if default_output_dir else f"./output/{output_default}"
    parser.add_argument(
        "file",
        nargs="?",
        help="KiCad .kicad_pro project; optional when one .kicad_pro is in CWD",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help=f"output directory (default: {default_help})",
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


def _add_dedupe_arg(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--dedupe",
        choices=("name", "fingerprint"),
        default="name",
        help="dedupe policy for internal extraction",
    )


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the megamaid command parser."""
    parser = subparsers.add_parser(
        "megamaid",
        help="aggressively dissect a KiCad project into library, embedded, and review assets",
        description=(
            "Extract cleaned symbols, footprints, metadata, embedded STEP models, "
            "all decoded embedded project assets, design JSON, netlists, schematic SVGs, "
            "and PCB review SVGs into a non-destructive megamaid project dissection bundle."
        ),
    )
    _add_common_library_args(parser, output_default="megamaid")
    _add_dedupe_arg(parser)
    _add_model_validation_args(parser)
    parser.set_defaults(handler=cmd_megamaid)
    return parser

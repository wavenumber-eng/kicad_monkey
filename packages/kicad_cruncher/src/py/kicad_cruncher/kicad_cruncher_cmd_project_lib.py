"""Metadata-preserving project-local library extraction command."""

from __future__ import annotations

import argparse

from kicad_cruncher.kicad_cruncher_cmd_megamaid import (
    _add_common_library_args,
    _add_model_validation_args,
    _run_library_extraction,
)


def cmd_project_lib(args: argparse.Namespace) -> int:
    """Extract a metadata-preserving project-local library bundle."""
    return _run_library_extraction(
        args,
        command_label="Project library extraction",
        output_default="project-lib",
        default_output_dir="local-library",
        mode_value="project_local",
        dedupe_value="name",
        manifest_schema="kicad_cruncher.project_lib_manifest.a0",
        manifest_filename="project_lib_manifest.json",
        readme_title="KiCad Project Library Extraction",
        update_project_library_tables=not bool(args.no_update_library_tables),
        project_stem_library_dirs=True,
        symbol_library_dir=args.symbol_library_dir,
        footprint_library_dir=args.footprint_library_dir,
        symbol_library_name=args.symbol_library_name,
        footprint_library_name=args.footprint_library_name,
        footprint_dedupe_value="library_link",
        write_models=bool(args.include_models or args.all_embedded_models),
        extract_all_embedded_models=True,
        preserve_model_filenames=True,
    )


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the metadata-preserving project-local library command parser."""
    parser = subparsers.add_parser(
        "project-lib",
        aliases=["project-library", "project-local-lib", "local-library"],
        help="extract metadata-preserving KiCad project-local libraries",
        description=(
            "Extract symbols, footprints, metadata, and embedded STEP models from a "
            "KiCad project into metadata-preserving project-local library artifacts."
        ),
    )
    _add_common_library_args(
        parser,
        output_default="project-lib",
        default_output_dir="local-library",
    )
    parser.add_argument(
        "--symbol-library-dir",
        help="symbol library folder under the output directory (default: .kicad_pro stem)",
    )
    parser.add_argument(
        "--footprint-library-dir",
        help=(
            "footprint .pretty folder under the output directory "
            "(default: <.kicad_pro stem>.pretty)"
        ),
    )
    parser.add_argument(
        "--symbol-library-name",
        help="symbol library nickname to add to sym-lib-table (default: symbol folder name)",
    )
    parser.add_argument(
        "--footprint-library-name",
        help="footprint library nickname to add to fp-lib-table (default: .pretty folder stem)",
    )
    parser.add_argument(
        "--include-models",
        action="store_true",
        help="write decoded embedded STEP/STP model files into models/",
    )
    parser.add_argument(
        "--no-update-library-tables",
        action="store_true",
        help="extract artifacts without editing project sym-lib-table/fp-lib-table",
    )
    _add_model_validation_args(parser)
    parser.set_defaults(handler=cmd_project_lib)
    return parser

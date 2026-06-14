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
        mode_value="project_local",
        dedupe_value="name",
        manifest_schema="kicad_cruncher.project_lib_manifest.v0",
        manifest_filename="project_lib_manifest.json",
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
    _add_common_library_args(parser, output_default="project-lib")
    _add_model_validation_args(parser)
    parser.set_defaults(handler=cmd_project_lib)
    return parser

"""Clean library-ingestion extraction command."""

from __future__ import annotations

import argparse

from kicad_cruncher.kicad_cruncher_cmd_megamaid import (
    _add_common_library_args,
    _add_dedupe_arg,
    _add_model_validation_args,
    cmd_lib_extract,
)


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the cleaned library extraction command parser."""
    parser = subparsers.add_parser(
        "lib-extract",
        aliases=["library-extract"],
        help="extract cleaned KiCad library artifacts for lib_cruncher",
        description=(
            "Extract cleaned symbols, footprints, metadata, and embedded STEP models "
            "from a KiCad project into a non-destructive lib_cruncher ingestion bundle."
        ),
    )
    _add_common_library_args(parser, output_default="lib-extract")
    _add_dedupe_arg(parser)
    _add_model_validation_args(parser)
    parser.set_defaults(handler=cmd_lib_extract)
    return parser

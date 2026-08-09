"""schematic command group for KiCad Cruncher."""

from __future__ import annotations

import argparse
import json


def cmd_schematic(args: argparse.Namespace) -> int:
    """Run one schematic command."""
    action = str(getattr(args, "schematic_action", ""))
    if action == "clean":
        return _cmd_schematic_clean(args)
    print("schematic subcommand required")
    return 2


def _cmd_schematic_clean(args: argparse.Namespace) -> int:
    _ = args
    print(
        json.dumps(
            {
                "schema": "kicad_cruncher.schematic.clean.plan.a0",
                "status": "deferred",
                "message": "Schematic clean follows the PCB clean implementation.",
            },
            indent=2,
        )
    )
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the schematic command parser."""
    parser = subparsers.add_parser("schematic", help="Run schematic utility commands")
    schematic_subparsers = parser.add_subparsers(
        dest="schematic_action",
        metavar="<schematic-action>",
    )
    clean_parser = schematic_subparsers.add_parser(
        "clean",
        help="Plan future schematic parameter cleanup",
    )
    clean_parser.set_defaults(handler=cmd_schematic)
    parser.set_defaults(handler=cmd_schematic)
    return parser

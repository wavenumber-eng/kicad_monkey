"""pcb command group for KiCad Cruncher."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from kicad_cruncher.kicad_cruncher_pcb_clean import (
    PCB_CLEAN_CONFIG_FILENAME,
    apply_pcb_clean,
    plan_pcb_clean,
    write_default_pcb_clean_config,
)


def cmd_pcb(args: argparse.Namespace) -> int:
    """Run one PCB command."""
    action = str(getattr(args, "pcb_action", ""))
    if action == "clean":
        return _cmd_pcb_clean(args)
    print("pcb subcommand required")
    return 2


def _cmd_pcb_clean(args: argparse.Namespace) -> int:
    write_config = getattr(args, "write_config", None)
    if write_config is not None:
        write_default_pcb_clean_config(Path(write_config))
        print(f"Wrote PCB clean config: {write_config}")
        return 0
    board = getattr(args, "file", None)
    config = getattr(args, "config", None)
    if bool(getattr(args, "apply", False)):
        result = apply_pcb_clean(
            board_path=Path(board) if board is not None else None,
            config_path=Path(config) if config is not None else None,
        )
        print(json.dumps(result, indent=2))
        return 0 if result["status"] == "applied" else 1
    plan = plan_pcb_clean(
        board_path=Path(board) if board is not None else None,
        config_path=Path(config) if config is not None else None,
        dry_run=bool(getattr(args, "dry_run", False)),
    )
    print(json.dumps(plan, indent=2))
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the pcb command parser."""
    parser = subparsers.add_parser("pcb", help="Run PCB utility commands")
    pcb_subparsers = parser.add_subparsers(dest="pcb_action", metavar="<pcb-action>")

    clean_parser = pcb_subparsers.add_parser(
        "clean",
        help="Plan PCB layer cleanup with a JSONC config",
    )
    clean_parser.add_argument("file", nargs="?", help=".kicad_pcb or .kicad_pro input")
    clean_parser.add_argument(
        "--config",
        help=f"PCB clean JSONC config ({PCB_CLEAN_CONFIG_FILENAME})",
    )
    clean_parser.add_argument("--write-config", help="Write the default PCB clean config")
    clean_mode = clean_parser.add_mutually_exclusive_group()
    clean_mode.add_argument("--dry-run", action="store_true", help="Write a cleanup plan only")
    clean_mode.add_argument("--apply", action="store_true", help="Apply cleanup mutations")
    clean_parser.set_defaults(handler=cmd_pcb)

    parser.set_defaults(handler=cmd_pcb)
    return parser

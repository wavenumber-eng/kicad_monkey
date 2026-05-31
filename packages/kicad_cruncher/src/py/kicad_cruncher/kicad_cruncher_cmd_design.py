"""Design JSON command for kicad_cruncher."""

from __future__ import annotations

import argparse
import logging
from pathlib import Path

from kicad_cruncher.kicad_cruncher_common import (
    find_kicad_project_in_cwd,
    resolve_output_dir,
    supported_design_input_suffixes,
)

log = logging.getLogger(__name__)


def _resolve_input_file(raw_file: str | None) -> Path | None:
    """Resolve an explicit file or auto-detect a project in the current directory."""
    if raw_file:
        input_file = Path(raw_file).resolve()
        if input_file.exists():
            return input_file
        log.error("File not found: %s", input_file)
        return None

    input_file = find_kicad_project_in_cwd()
    if input_file is None:
        log.error("No file specified and no single .kicad_pro found in current directory")
        log.info("Usage: kicad-cruncher design [project.kicad_pro | schematic.kicad_sch]")
        return None
    log.info("Auto-detected project: %s", input_file.name)
    return input_file.resolve()


def _validate_input_suffix(input_file: Path) -> bool:
    """Return whether the input file suffix is supported for design JSON."""
    suffix = input_file.suffix.lower()
    if suffix in supported_design_input_suffixes():
        return True
    log.error("Unsupported file type: %s", suffix)
    log.info("Supported types: .kicad_pro, .kicad_sch")
    return False


def cmd_design(args: argparse.Namespace) -> int:
    """Generate KiCad-native design JSON from a project or schematic."""
    from kicad_monkey import KiCadDesign

    input_file = _resolve_input_file(str(args.file) if args.file else None)
    if input_file is None:
        return 1
    if not _validate_input_suffix(input_file):
        return 1

    output_dir = resolve_output_dir(args.output, "design")
    output_file = output_dir / f"{input_file.stem}_design.json"
    include_indexes = not bool(args.no_indexes)

    try:
        design = KiCadDesign.from_file(input_file)
        design.save_json(output_file, include_indexes=include_indexes)
        payload = design.to_json(include_indexes=include_indexes)
    except Exception as exc:
        log.error("Design JSON generation failed: %s", exc)
        return 1

    log.info(
        "Design JSON: %d components, %d nets -> %s",
        len(payload.get("components", [])),
        len(payload.get("nets", [])),
        output_file,
    )
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the design command parser."""
    design_parser = subparsers.add_parser(
        "design",
        help="generate KiCad-native design JSON",
        description=(
            "Generate KiCad-native design JSON from .kicad_pro or .kicad_sch files. "
            "The output includes project metadata, schematic hierarchy, components, "
            "nets, variants, and optional lookup indexes."
        ),
        epilog=(
            "Examples:\n"
            "  kicad-cruncher design project.kicad_pro\n"
            "  kicad-cruncher design schematic.kicad_sch\n"
            "  kicad-cruncher design                    # Auto-detect one .kicad_pro in CWD\n"
            "  kicad-cruncher design project.kicad_pro --no-indexes\n"
            "  kicad-cruncher design project.kicad_pro -o output_dir/"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    design_parser.add_argument(
        "file",
        nargs="?",
        help="KiCad project or schematic file; optional when one .kicad_pro is in CWD",
    )
    design_parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="output directory (default: ./output/design)",
    )
    design_parser.add_argument(
        "--no-indexes",
        action="store_true",
        help="exclude lookup indexes from JSON",
    )
    design_parser.set_defaults(handler=cmd_design)
    return design_parser


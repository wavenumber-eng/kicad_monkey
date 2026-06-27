"""New KiCad project scaffolding command (``project create``).

Three front-ends converge on one options object that drives this repo's
``create_project`` assembly, which composes kicad_monkey's object model:

    flags / jsonc / --tui (form)  ->  KiCadProjectCreateOptions  ->  create_project()
"""

from __future__ import annotations

import argparse
import logging
from pathlib import Path
from typing import TYPE_CHECKING

from kicad_monkey import KICAD_PAGE_SIZES

from kicad_cruncher.kicad_cruncher_project_create import DEFAULT_PAGE_SIZE

if TYPE_CHECKING:
    from kicad_cruncher.kicad_cruncher_project_create import (
        KiCadProjectCreateOptions,
        KiCadProjectCreateResult,
    )

log = logging.getLogger(__name__)


def _default_page_size() -> str:
    """The project-create default page size."""
    return DEFAULT_PAGE_SIZE


def _parse_text_vars(items: list[str] | None) -> dict[str, str]:
    out: dict[str, str] = {}
    for item in items or []:
        if "=" not in item:
            raise ValueError(f"--text-var must be NAME=VALUE, got {item!r}")
        name, value = item.split("=", 1)
        name = name.strip()
        if not name:
            raise ValueError(f"--text-var has an empty NAME: {item!r}")
        out[name] = value
    return out


def _parse_lib_specs(items: list[str] | None) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    for item in items or []:
        name, sep, uri = item.partition("=")
        name, uri = name.strip(), uri.strip()
        if not sep or not name or not uri:
            raise ValueError(f"library must be NICK=URI, got {item!r}")
        out.append({"name": name, "uri": uri})
    return out


def _options_from_args(args: argparse.Namespace) -> KiCadProjectCreateOptions:
    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions

    return KiCadProjectCreateOptions(
        name=args.name,
        directory=args.directory or ".",
        page_size=args.size or _default_page_size(),
        sheet_name=args.sheet_name or "",
        worksheet=args.worksheet,
        embed_worksheet=not args.no_embed_worksheet,
        create_pcb=bool(args.with_pcb),
        create_lib_tables=not args.no_lib_tables,
        create_subdirectory=not args.no_subdir,
        text_variables=_parse_text_vars(args.text_var),
        revision=args.rev or "",
        company=args.company or "",
        date=args.date or "",
        comments=list(args.comment or []),
        symbol_libraries=_parse_lib_specs(args.symbol_lib),
        footprint_libraries=_parse_lib_specs(args.footprint_lib),
    )


def _ask(label: str, default: str = "") -> str:
    suffix = f" [{default}]" if default else ""
    return input(f"  {label}{suffix}: ").strip() or default


def _prompt_text_vars(initial: dict[str, str]) -> dict[str, str]:
    text_vars = dict(initial)
    print("  Text variables (NAME=VALUE, blank line to finish):")
    while True:
        entry = input("    > ").strip()
        if not entry:
            break
        if "=" in entry:
            key, val = entry.split("=", 1)
            text_vars[key.strip()] = val
        else:
            print("    (expected NAME=VALUE)")
    return text_vars


def _interactive_options(args: argparse.Namespace) -> KiCadProjectCreateOptions:
    """Prompt fallback for ``--tui`` when textual is unavailable."""
    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions

    print("\n  NEW KICAD PROJECT  (press Enter to accept the [default])\n")
    name = ""
    while not name:
        name = _ask("Project name", args.name or "")
    size = _ask(f"Page size {list(KICAD_PAGE_SIZES)}", args.size or _default_page_size())
    directory = _ask("Directory", args.directory or ".")
    sheet_name = _ask("Top sheet name", args.sheet_name or name)
    worksheet = _ask("Worksheet .wks (blank for none)", str(args.worksheet or ""))
    with_pcb_default = "y" if args.with_pcb else "n"
    with_pcb = _ask("Also create a .kicad_pcb? (y/N)", with_pcb_default).lower().startswith("y")
    text_vars = _prompt_text_vars(_parse_text_vars(args.text_var))

    return KiCadProjectCreateOptions(
        name=name,
        directory=directory,
        page_size=size,
        sheet_name=sheet_name,
        worksheet=worksheet or None,
        embed_worksheet=not args.no_embed_worksheet,
        create_pcb=with_pcb,
        create_lib_tables=not args.no_lib_tables,
        create_subdirectory=not args.no_subdir,
        text_variables=text_vars,
    )


def _defaults_from_args(args: argparse.Namespace) -> dict:
    return {
        "name": args.name or "",
        "directory": args.directory or str(Path.cwd()),
        "page_size": args.size or _default_page_size(),
        "sheet_name": args.sheet_name or "",
        "worksheet": args.worksheet or "",
        "embed_worksheet": not args.no_embed_worksheet,
        "create_pcb": bool(args.with_pcb),
        "create_lib_tables": not args.no_lib_tables,
        "text_variables": _parse_text_vars(args.text_var),
        "revision": args.rev or "",
        "company": args.company or "",
        "date": args.date or "",
        "comments": list(args.comment or []),
        "symbol_libraries": _parse_lib_specs(args.symbol_lib),
        "footprint_libraries": _parse_lib_specs(args.footprint_lib),
    }


def _emit_config(out: Path) -> int:
    from kicad_cruncher.kicad_cruncher_project_create_config import default_config_text

    try:
        out.write_text(default_config_text(), encoding="utf-8")
    except OSError as exc:
        log.error("could not write config: %s", exc)
        return 1
    log.info("Wrote project-create config template: %s", out)
    return 0


def _resolve_tui_options(args: argparse.Namespace) -> KiCadProjectCreateOptions | None:
    """Options from the interactive form (or prompt fallback); None if cancelled."""
    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions

    try:
        from kicad_cruncher.kicad_cruncher_project_create_tui import run_project_create_tui
    except ImportError:
        log.warning("textual not installed — falling back to prompts")
        return _interactive_options(args)
    chosen = run_project_create_tui(_defaults_from_args(args))
    if chosen is None:
        return None
    return KiCadProjectCreateOptions(**chosen)


def _log_result(result: KiCadProjectCreateResult, options: KiCadProjectCreateOptions) -> None:
    log.info("Created KiCad project: %s", result.project_dir)
    log.info("  project    : %s", result.project_file.name)
    log.info("  schematic  : %s  (paper %s)", result.schematic_file.name, options.page_size)
    if result.worksheet_file is not None:
        mode = "embedded" if options.embed_worksheet else "referenced"
        log.info("  worksheet  : %s  (%s)", Path(result.worksheet_file).name, mode)
    if result.symbol_table is not None:
        log.info("  lib tables : sym-lib-table, fp-lib-table")
    if result.pcb_file is not None:
        log.info("  board      : %s", result.pcb_file.name)


def cmd_project_create(args: argparse.Namespace) -> int:
    """Scaffold a new KiCad project."""
    from kicad_cruncher.kicad_cruncher_project_create import create_project
    from kicad_cruncher.kicad_cruncher_project_create_config import options_from_config

    if args.emit_config is not None:
        return _emit_config(Path(args.emit_config))

    try:
        if args.config:
            options = options_from_config(Path(args.config))
        elif args.tui:
            options = _resolve_tui_options(args)
            if options is None:
                log.info("project create: cancelled")
                return 0
        elif not args.name:
            log.error("project create: --name is required (or use --tui / --config)")
            return 2
        else:
            options = _options_from_args(args)
        result = create_project(options)
    except (ValueError, OSError) as exc:
        log.error("project create failed: %s", exc)
        return 1

    _log_result(result, options)
    return 0


def _cmd_project(args: argparse.Namespace) -> int:
    """``project`` with no subcommand → show help."""
    args.project_parser.print_help()
    return 2


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the ``project`` command and its ``create`` subcommand."""
    project = subparsers.add_parser(
        "project",
        help="create and manage KiCad projects",
        description="KiCad project workflows. Use `project create` to scaffold a new project.",
    )
    project_sub = project.add_subparsers(dest="project_command")

    create = project_sub.add_parser(
        "create",
        help="create a new KiCad project",
        description=(
            "Scaffold a new KiCad project (.kicad_pro + top-level .kicad_sch, optional "
            "embedded worksheet, library tables, and PCB). Use --tui for the interactive form."
        ),
    )
    create.add_argument("--name", help="project name (also the file stem)")
    create.add_argument(
        "--size", "--page-size", dest="size", choices=KICAD_PAGE_SIZES,
        help="schematic page size from KiCad's standard set (default owned by kicad_monkey)",
    )
    create.add_argument(
        "--dir", "--directory", dest="directory",
        help="directory to create the project in (default: current dir)",
    )
    create.add_argument("--sheet-name", help="top-level sheet/title name (default: project name)")
    create.add_argument("--worksheet", help="path to a .wks drawing sheet to embed")
    create.add_argument(
        "--no-embed-worksheet", action="store_true",
        help="reference the worksheet by path instead of embedding it",
    )
    create.add_argument("--with-pcb", action="store_true", help="also create a blank .kicad_pcb")
    create.add_argument(
        "--no-lib-tables", action="store_true",
        help="do not create empty sym-lib-table / fp-lib-table",
    )
    create.add_argument(
        "--no-subdir", action="store_true",
        help="write into --dir directly instead of a <name>/ subfolder",
    )
    create.add_argument(
        "--text-var", action="append", metavar="NAME=VALUE",
        help="add a project text variable (repeatable)",
    )
    create.add_argument("--company", help="schematic title-block company")
    create.add_argument("--rev", "--revision", dest="rev", help="schematic title-block revision")
    create.add_argument("--date", help="schematic title-block date (e.g. 2026-06-22)")
    create.add_argument(
        "--comment", action="append", metavar="TEXT",
        help="title-block comment line 1..9 (repeatable)",
    )
    create.add_argument(
        "--symbol-lib", action="append", metavar="NICK=URI",
        help="add a symbol library to sym-lib-table (repeatable)",
    )
    create.add_argument(
        "--footprint-lib", action="append", metavar="NICK=URI",
        help="add a footprint library to fp-lib-table (repeatable)",
    )
    create.add_argument(
        "--config", metavar="FILE.jsonc",
        help="load all options from a JSONC config file",
    )
    create.add_argument(
        "--emit-config", nargs="?", const="project-create.jsonc", metavar="FILE.jsonc",
        help="write a commented default JSONC config and exit",
    )
    create.add_argument(
        "--tui", action="store_true",
        help="open the interactive terminal form instead of using flags",
    )
    create.set_defaults(handler=cmd_project_create)

    project.set_defaults(handler=_cmd_project, project_parser=project)
    return project

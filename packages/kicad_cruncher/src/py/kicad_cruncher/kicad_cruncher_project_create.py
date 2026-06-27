"""Application-level new-project assembly for ``kcr project create``.

This module owns the *typed options schema* and the *assembly orchestration*
that all front-ends (CLI flags, ``.jsonc`` config, and the interactive ``--tui``
form) converge on:

    flags / jsonc / --tui (form)  ->  KiCadProjectCreateOptions  ->  create_project()

Construction is delegated entirely to kicad_monkey's object model (the
``KiCadProject`` aggregate, ``KiCadSchematic``, ``KiCadPcb``, library tables and
worksheet embedding). kicad_monkey holds no knowledge of these front-ends; this
layer only gathers input and decides *which* primitives to compose.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kicad_monkey import KiCadProject

# Command-level default page size (KiCad "D" / 22x17in). The page-size choice
# list itself lives in kicad_monkey (KICAD_PAGE_SIZES); this is a UX default.
DEFAULT_PAGE_SIZE = "D"


@dataclass
class KiCadProjectCreateOptions:
    """Everything needed to scaffold a new KiCad project.

    One field per user-facing knob. Front-ends (flags, jsonc, TUI) map onto this
    schema; :func:`create_project` consumes it and drives the kicad_monkey
    object model.
    """

    name: str
    directory: Path | str
    page_size: str = DEFAULT_PAGE_SIZE
    sheet_name: str = ""                       # top-level sheet / title name
    worksheet: Path | str | None = None        # optional .wks to embed/reference
    embed_worksheet: bool = True
    create_pcb: bool = False
    create_lib_tables: bool = True
    create_subdirectory: bool = True           # write into <directory>/<name>/
    text_variables: dict[str, str] = field(default_factory=dict)

    # schematic title-block metadata (border fields)
    revision: str = ""
    company: str = ""
    date: str = ""
    comments: list[str] = field(default_factory=list)   # title-block comments 1..N

    # project library tables — existing libraries to reference by nickname.
    # Each entry: {"name": <nickname>, "uri": <path/URI>, "type": "KiCad" (optional)}.
    symbol_libraries: list[dict[str, str]] = field(default_factory=list)
    footprint_libraries: list[dict[str, str]] = field(default_factory=list)


@dataclass
class KiCadProjectCreateResult:
    """Paths produced by :func:`create_project`."""

    project_dir: Path
    project_file: Path
    schematic_file: Path
    worksheet_file: Path | None = None
    pcb_file: Path | None = None
    symbol_table: Path | None = None
    footprint_table: Path | None = None


def _validated_lib_spec(spec: dict[str, str]) -> tuple[str, str, str]:
    """Return ``(nickname, uri, type)`` from a library spec, or raise."""
    name = (spec.get("name") or "").strip()
    uri = (spec.get("uri") or "").strip()
    if not name or not uri:
        raise ValueError(f"library entry needs both 'name' and 'uri': {spec!r}")
    return name, uri, spec.get("type") or "KiCad"


def _seed_library_tables(project: KiCadProject, options: KiCadProjectCreateOptions) -> None:
    """Seed referenced symbol/footprint libraries and optional empty tables."""
    for spec in options.symbol_libraries:
        nick, uri, lib_type = _validated_lib_spec(spec)
        project.add_symbol_library(nick, uri, library_type=lib_type)
    for spec in options.footprint_libraries:
        nick, uri, lib_type = _validated_lib_spec(spec)
        project.add_footprint_library(nick, uri, library_type=lib_type)
    if options.create_lib_tables:
        project.ensure_library_tables()


def create_project(options: KiCadProjectCreateOptions) -> KiCadProjectCreateResult:
    """Scaffold a new KiCad project from ``options`` and return the written paths.

    Composes kicad_monkey's :class:`~kicad_monkey.KiCadProject` aggregate: a
    blank ``.kicad_pro`` (with text variables) and a top-level ``.kicad_sch`` at
    the requested page size whose title block carries the sheet metadata. A
    worksheet is embedded into the schematic (and referenced from the project as
    ``kicad-embed://``) by default, or referenced by path when
    ``embed_worksheet`` is off. Library tables and an optional blank PCB are
    written per the toggles.
    """
    from kicad_monkey import KiCadProject

    name = options.name
    if not name:
        raise ValueError("project name must not be empty")

    base = Path(options.directory)
    project_dir = base / name if options.create_subdirectory else base

    # KiCadProject.create is kicad_monkey's canonical public constructor for a
    # new on-disk project (seeds the default .kicad_pro and binds name/directory);
    # the bare KiCadProject(...) dataclass constructor is the existing-JSON facade.
    project = KiCadProject.create(name, project_dir)
    for key, value in options.text_variables.items():
        project.set_text_variable(key, value)

    project.add_schematic(
        sheet_name=options.sheet_name or None,
        page_size=options.page_size,
        revision=options.revision,
        company=options.company,
        date=options.date,
        comments=options.comments,
    )

    if options.worksheet is not None:
        project.set_worksheet(options.worksheet, embed=options.embed_worksheet)

    _seed_library_tables(project, options)

    if options.create_pcb:
        project.add_pcb(page_size=options.page_size)

    files = project.write_project(project_dir)
    return KiCadProjectCreateResult(
        project_dir=files.project_dir,
        project_file=files.project_file,
        schematic_file=files.schematic_file or project_dir / f"{name}.kicad_sch",
        worksheet_file=files.worksheet_file,
        pcb_file=files.pcb_file,
        symbol_table=files.symbol_table,
        footprint_table=files.footprint_table,
    )

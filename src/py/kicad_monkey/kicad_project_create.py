"""New-project scaffolding primitives for ``kcr project create``.

This module owns the *typed options schema* and the *assembly primitive* that all
front-ends (CLI flags, ``.jsonc`` config, and the interactive ``--cli`` TUI in
``kicad_cruncher``) converge on:

    flags / jsonc / TUI  ->  KiCadProjectCreateOptions  ->  create_project(...)

The schema is intentionally the single source of truth, so adding a field gives
every front-end the same new knob.  Detailed semantics (worksheet embedding,
optional PCB, library tables) are filled in across later phases.
"""

from __future__ import annotations

import base64 as _base64
import hashlib as _hashlib
import uuid as _uuid
from dataclasses import dataclass, field
from pathlib import Path

from .kicad_model import EmbeddedFile
from .kicad_project import KiCadProject
from .kicad_project_libraries import (
    KiCadLibraryTable,
    KiCadLibraryTableEntry,
    KiCadLibraryTableKind,
)


@dataclass
class KiCadProjectCreateOptions:
    """Everything needed to scaffold a new KiCad project.

    One field per user-facing knob.  Front-ends (flags, jsonc, TUI) map onto this
    schema; :func:`create_project` consumes it.
    """

    name: str
    directory: Path | str
    page_size: str = "D"
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


def _embed_worksheet(path: Path) -> EmbeddedFile:
    """Compress a ``.wks`` into a KiCad ``(file (type worksheet) …)`` payload.

    Same scheme KiCad reads: zstd-compressed, base64-encoded data with a SHA-256
    checksum of the raw bytes (KiCad accepts the SHA-256 form).
    """
    import zstandard

    payload = path.read_bytes()
    compressed = zstandard.ZstdCompressor().compress(payload)
    return EmbeddedFile(
        name=path.name,
        file_type="worksheet",
        data=_base64.b64encode(compressed).decode("ascii"),
        checksum=_hashlib.sha256(payload).hexdigest(),
    )


def _library_entries(specs: list[dict[str, str]]) -> list[KiCadLibraryTableEntry]:
    """Build library-table entries from ``{name, uri, type?}`` mappings."""
    entries: list[KiCadLibraryTableEntry] = []
    for spec in specs:
        name = (spec.get("name") or "").strip()
        uri = (spec.get("uri") or "").strip()
        if not name or not uri:
            raise ValueError(f"library entry needs both 'name' and 'uri': {spec!r}")
        entries.append(
            KiCadLibraryTableEntry(name=name, uri=uri, library_type=spec.get("type") or "KiCad")
        )
    return entries


def create_project(options: KiCadProjectCreateOptions) -> KiCadProjectCreateResult:
    """Scaffold a new KiCad project from ``options`` and return the written paths.

    Writes a byte-faithful blank ``.kicad_pro`` (with text variables) and a blank
    top-level ``.kicad_sch`` at the requested page size, whose title block carries
    the sheet name.  A worksheet is embedded into the schematic (and referenced
    from the project as ``kicad-embed://…``) by default, or referenced by external
    path when ``embed_worksheet`` is off.  Empty library tables and an optional
    blank PCB are written per the toggles.
    """
    # Imported lazily to keep the project module import-light.
    from .kicad_schematic import KiCadSchematic
    from .kicad_sch_title_block import PaperSize, TitleBlock

    name = options.name
    if not name:
        raise ValueError("project name must not be empty")

    base = Path(options.directory)
    project_dir = base / name if options.create_subdirectory else base
    project_dir.mkdir(parents=True, exist_ok=True)

    project_file = project_dir / f"{name}.kicad_pro"
    schematic_file = project_dir / f"{name}.kicad_sch"

    # --- .kicad_pro ---------------------------------------------------------
    project = KiCadProject.new(name=name, project_path=project_file)
    for key, value in options.text_variables.items():
        project.set_text_variable(key, value)

    # --- top-level .kicad_sch ----------------------------------------------
    schematic = KiCadSchematic()
    schematic.uuid = str(_uuid.uuid4())   # blank constructor leaves this empty
    schematic.paper = PaperSize(size=options.page_size)
    schematic.title_block = TitleBlock(
        title=options.sheet_name or name,
        rev=options.revision,
        company=options.company,
        date=options.date,
        comments={i + 1: c for i, c in enumerate(options.comments) if c},
    )

    # --- worksheet (embed | reference | none) ------------------------------
    worksheet_file: Path | None = None
    if options.worksheet is not None:
        wks_path = Path(options.worksheet)
        if options.embed_worksheet:
            embedded = _embed_worksheet(wks_path)
            schematic.embedded_files.append(embedded)
            schematic.embedded_fonts = True  # KiCad sets this once files are embedded
            project.set_path(
                "schematic.page_layout_descr_file",
                f"kicad-embed://{embedded.name}",
            )
            worksheet_file = wks_path
        else:
            project.set_path("schematic.page_layout_descr_file", str(wks_path))
            worksheet_file = wks_path

    project.save()
    schematic.save(schematic_file)

    # --- library tables (empty, or seeded with referenced libraries) -------
    symbol_entries = _library_entries(options.symbol_libraries)
    footprint_entries = _library_entries(options.footprint_libraries)
    symbol_table = footprint_table = None
    if options.create_lib_tables or symbol_entries:
        symbol_table = project_dir / KiCadLibraryTableKind.SYMBOL.file_name
        KiCadLibraryTable(
            kind=KiCadLibraryTableKind.SYMBOL, entries=symbol_entries
        ).write(symbol_table)
    if options.create_lib_tables or footprint_entries:
        footprint_table = project_dir / KiCadLibraryTableKind.FOOTPRINT.file_name
        KiCadLibraryTable(
            kind=KiCadLibraryTableKind.FOOTPRINT, entries=footprint_entries
        ).write(footprint_table)

    # --- optional blank PCB -------------------------------------------------
    pcb_file = None
    if options.create_pcb:
        from .kicad_pcb import KiCadPcb

        pcb = KiCadPcb()
        pcb.paper = options.page_size
        pcb_file = project_dir / f"{name}.kicad_pcb"
        pcb.save(pcb_file)

    return KiCadProjectCreateResult(
        project_dir=project_dir,
        project_file=project_file,
        schematic_file=schematic_file,
        worksheet_file=worksheet_file,
        pcb_file=pcb_file,
        symbol_table=symbol_table,
        footprint_table=footprint_table,
    )


__all__ = [
    "KiCadProjectCreateOptions",
    "KiCadProjectCreateResult",
    "create_project",
]

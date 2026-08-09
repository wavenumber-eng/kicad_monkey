"""JSONC config for ``kcr project create`` (load + emit a commented default).

The config keys map 1:1 to this repo's ``KiCadProjectCreateOptions`` — the same
schema the flags and the ``--tui`` form populate.
"""

from __future__ import annotations

from dataclasses import fields
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

from kicad_cruncher.config_json import (
    JsoncCommentMap,
    load_json_config,
    render_commented_jsonc,
)

if TYPE_CHECKING:
    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions


def default_config() -> dict:
    """An example config — every default sourced from the options schema."""
    from dataclasses import asdict

    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions

    return asdict(KiCadProjectCreateOptions(name="My First Project", directory="."))


_COMMENTS: JsoncCommentMap = {
    "name": "Project name — also the .kicad_pro / .kicad_sch file stem.",
    "directory": (
        "Where to create the project (a <name>/ subfolder is added "
        "unless create_subdirectory is false)."
    ),
    "page_size": "Schematic page size: A4 A3 A2 A1 A0 | A B C D E | USLetter USLegal USLedger.",
    "sheet_name": "Top-level sheet / title-block title (defaults to the project name).",
    "worksheet": "Path to a .wks drawing sheet to embed/reference. Empty = KiCad built-in default.",
    "embed_worksheet": (
        "true = embed the .wks into the schematic (self-contained); "
        "false = reference it by path."
    ),
    "create_pcb": "Also create a blank .kicad_pcb.",
    "create_lib_tables": (
        "Create empty sym-lib-table / fp-lib-table "
        "(always created when libraries are listed below)."
    ),
    "create_subdirectory": (
        "Create <directory>/<name>/ instead of writing into <directory> directly."
    ),
    "revision": "Schematic title-block revision.",
    "company": "Schematic title-block company.",
    "date": "Schematic title-block date (e.g. 2026-06-22); empty leaves it blank.",
    "comments": 'Title-block comment lines 1..9, e.g. ["Confidential"].',
    "text_variables": 'Project text variables, e.g. {"REV": "A"} — usable as ${REV} in the design.',
    "symbol_libraries": (
        'Symbol libraries for sym-lib-table: '
        '[{"name": "WN", "uri": "${KIPRJMOD}/WN.kicad_sym"}].'
    ),
    "footprint_libraries": (
        'Footprint libraries for fp-lib-table: '
        '[{"name": "WN", "uri": "${KIPRJMOD}/WN.pretty"}].'
    ),
}

_HEADER = (
    "kcr project create — configuration",
    "Edit, then run:  kcr project create --config this-file.jsonc",
    "Every key maps to a KiCadProjectCreateOptions field.",
)


def default_config_text() -> str:
    """Render the commented JSONC template."""
    return render_commented_jsonc(
        default_config(), comments_by_path=_COMMENTS, header_lines=_HEADER
    )


def options_from_config(path: Path) -> KiCadProjectCreateOptions:
    """Load a JSONC config into a ``KiCadProjectCreateOptions``."""
    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions

    raw = load_json_config(path)
    valid = {f.name for f in fields(KiCadProjectCreateOptions)}
    unknown = sorted(set(raw) - valid)
    if unknown:
        raise ValueError(f"unknown config keys: {', '.join(unknown)}")
    if not raw.get("name"):
        raise ValueError("config must set a non-empty 'name'")
    return KiCadProjectCreateOptions(**cast("dict[str, Any]", raw))

"""Shared helpers for KiCad Cruncher command modules."""

from __future__ import annotations

from pathlib import Path


def resolve_output_dir(output: Path | None, command_name: str) -> Path:
    """Resolve and create a command output directory.

    All output-producing commands default to ``./output/<command>/``. An
    explicit ``-o/--output`` value replaces the whole output directory rather
    than being nested under the command name.
    """
    output_dir = output if output is not None else Path("output") / command_name
    output_dir.mkdir(parents=True, exist_ok=True)
    return output_dir


def find_kicad_project_in_cwd(cwd: Path | None = None) -> Path | None:
    """Return the single `.kicad_pro` in a directory, if exactly one exists."""
    search_dir = cwd if cwd is not None else Path.cwd()
    projects = sorted(path for path in search_dir.glob("*.kicad_pro") if path.is_file())
    if len(projects) != 1:
        return None
    return projects[0]


def supported_design_input_suffixes() -> tuple[str, ...]:
    """Return suffixes accepted by the design JSON command."""
    return (".kicad_pro", ".kicad_sch")

"""Rack tests for the ``project create`` command (issue #4).

Covers the cruncher's input-gathering layers — flag parsing, JSONC config
load/emit, and the interactive TUI submission — plus the command's end-to-end
behavior and the ``create_project`` assembly it owns. The underlying KiCad
construction is owned and tested by ``kicad-monkey``; here we verify the cruncher
maps every front-end onto its options and composes the project correctly.
"""

from __future__ import annotations

import argparse
import asyncio
import json
from pathlib import Path

import pytest
from kicad_cruncher.kicad_cruncher_cmd_project import (
    _options_from_args,
    _parse_lib_specs,
    _parse_text_vars,
    cmd_project_create,
)
from kicad_cruncher.kicad_cruncher_project_create_config import (
    default_config_text,
    options_from_config,
)


def _ns(**overrides: object) -> argparse.Namespace:
    """Build a project-create args namespace with all flags defaulted to None/False."""
    base: dict[str, object] = {
        "name": None, "size": None, "directory": None, "sheet_name": None,
        "worksheet": None, "no_embed_worksheet": False, "with_pcb": False,
        "no_lib_tables": False, "no_subdir": False, "text_var": None,
        "company": None, "rev": None, "date": None, "comment": None,
        "symbol_lib": None, "footprint_lib": None, "config": None,
        "emit_config": None, "tui": False,
    }
    base.update(overrides)
    return argparse.Namespace(**base)


# --- flag parsing ----------------------------------------------------------

def test_parse_text_vars_ok() -> None:
    assert _parse_text_vars(["REV=A", "X=y=z"]) == {"REV": "A", "X": "y=z"}


def test_parse_text_vars_rejects_bad() -> None:
    with pytest.raises(ValueError):
        _parse_text_vars(["NOEQUALS"])
    with pytest.raises(ValueError):
        _parse_text_vars(["=novalue"])


def test_parse_lib_specs_ok() -> None:
    assert _parse_lib_specs(["WN=path/WN.kicad_sym"]) == [
        {"name": "WN", "uri": "path/WN.kicad_sym"}
    ]


def test_parse_lib_specs_rejects_bad() -> None:
    with pytest.raises(ValueError):
        _parse_lib_specs(["NOEQUALS"])


def test_options_from_args_maps_all_fields() -> None:
    opts = _options_from_args(_ns(
        name="Board", size="A3", directory="out", company="WN", rev="B",
        text_var=["REV=B"], symbol_lib=["S=u.kicad_sym"], footprint_lib=["F=u.pretty"],
        with_pcb=True,
    ))
    assert opts.name == "Board" and opts.page_size == "A3"
    assert opts.company == "WN" and opts.revision == "B" and opts.create_pcb is True
    assert opts.text_variables == {"REV": "B"}
    assert opts.symbol_libraries == [{"name": "S", "uri": "u.kicad_sym"}]
    assert opts.footprint_libraries == [{"name": "F", "uri": "u.pretty"}]


def test_options_from_args_default_page_size_is_command_default() -> None:
    from kicad_cruncher.kicad_cruncher_project_create import DEFAULT_PAGE_SIZE

    assert _options_from_args(_ns(name="X")).page_size == DEFAULT_PAGE_SIZE


# --- JSONC config ----------------------------------------------------------

def test_default_config_text_is_commented_jsonc(tmp_path: Path) -> None:
    text = default_config_text()
    assert text.startswith("//")                 # header comment block
    assert '"symbol_libraries"' in text
    cfg = tmp_path / "pc.jsonc"
    cfg.write_text(text, encoding="utf-8")
    opts = options_from_config(cfg)              # template round-trips
    assert opts.name == "My First Project" and opts.worksheet is None


def test_config_rejects_unknown_keys(tmp_path: Path) -> None:
    cfg = tmp_path / "bad.jsonc"
    cfg.write_text('{"name": "X", "bogus": 1}', encoding="utf-8")
    with pytest.raises(ValueError):
        options_from_config(cfg)


def test_config_requires_name(tmp_path: Path) -> None:
    cfg = tmp_path / "noname.jsonc"
    cfg.write_text('{"directory": "."}', encoding="utf-8")
    with pytest.raises(ValueError):
        options_from_config(cfg)


# --- command behavior ------------------------------------------------------

def test_cmd_emit_config_writes_template(tmp_path: Path) -> None:
    out = tmp_path / "pc.jsonc"
    assert cmd_project_create(_ns(emit_config=str(out))) == 0
    assert out.exists() and out.read_text(encoding="utf-8").startswith("//")


def test_cmd_requires_name_without_tui_or_config() -> None:
    assert cmd_project_create(_ns()) == 2


def test_cmd_flags_scaffold_project(tmp_path: Path) -> None:
    from kicad_monkey import KiCadSchematic

    assert cmd_project_create(_ns(
        name="CmdMade", directory=str(tmp_path), company="WN", rev="A",
    )) == 0
    proj = tmp_path / "CmdMade"
    assert (proj / "CmdMade.kicad_pro").exists()
    sch = KiCadSchematic.from_file(proj / "CmdMade.kicad_sch")
    assert sch.title_block is not None
    assert sch.title_block.company == "WN" and sch.title_block.rev == "A"


def test_cmd_config_scaffolds_project(tmp_path: Path) -> None:
    cfg = tmp_path / "pc.jsonc"
    cfg.write_text(json.dumps({"name": "FromCfg", "directory": str(tmp_path)}), encoding="utf-8")
    assert cmd_project_create(_ns(config=str(cfg))) == 0
    assert (tmp_path / "FromCfg" / "FromCfg.kicad_pro").exists()


# --- interactive TUI -------------------------------------------------------

def test_tui_submit_builds_valid_options() -> None:
    from kicad_cruncher.kicad_cruncher_project_create import KiCadProjectCreateOptions
    from kicad_cruncher.kicad_cruncher_project_create_tui import ProjectCreateApp

    async def run() -> dict | None:
        app = ProjectCreateApp({
            "name": "TuiMade", "company": "WN",
            "symbol_libraries": [{"name": "S", "uri": "u.kicad_sym"}],
        })
        async with app.run_test() as pilot:
            await pilot.pause()
            app._submit()
            await pilot.pause()
        return app.return_value

    chosen = asyncio.run(run())
    assert chosen is not None
    assert chosen["name"] == "TuiMade" and chosen["company"] == "WN"
    opts = KiCadProjectCreateOptions(**chosen)   # the dict is a valid options payload
    assert opts.symbol_libraries == [{"name": "S", "uri": "u.kicad_sym"}]

"""Subtest: KiCad new-project scaffolding
Stratum: L2_tools
Purpose: Primitives behind ``kcr project create`` — blank project, schematic with
page size + sheet name, embedded/referenced worksheet, lib tables, optional PCB.

The user-facing command lives in ``kicad_cruncher``; these tests cover the
``kicad_monkey`` APIs it composes (issue #4).
"""

from __future__ import annotations

import base64
import subprocess
from pathlib import Path

import pytest

from kicad_monkey import (
    KiCadPcb,
    KiCadProject,
    KiCadProjectCreateOptions,
    KiCadProjectCreateResult,
    KiCadSchematic,
    create_project,
)
from kicad_monkey.kicad_library_extraction import resolve_kicad_cli
from kicad_monkey.kicad_model import EmbeddedFile


def test_kicad_project_new_is_blank_kicad10_default() -> None:
    """KiCadProject.new() matches KiCad 10's default project structure."""
    raw = KiCadProject.new().to_json()
    assert raw["meta"] == {"filename": "kicad.kicad_pro", "version": 1}
    assert raw["text_variables"] == {}
    assert raw["sheets"] == []
    assert raw["pcbnew"] == {"page_layout_descr_file": ""}
    assert set(raw) == {
        "board", "boards", "libraries", "meta",
        "net_settings", "pcbnew", "sheets", "text_variables",
    }


def test_kicad_project_new_named_sets_filename() -> None:
    assert KiCadProject.new(name="My Board").get_path("meta.filename") == "My Board.kicad_pro"


def test_create_project_minimal(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(
        name="Demo", directory=tmp_path, page_size="D", sheet_name="Top Level",
        text_variables={"REV": "A"}, create_lib_tables=False,
    ))
    assert isinstance(res, KiCadProjectCreateResult)
    assert res.project_file.exists() and res.schematic_file.exists()

    proj = KiCadProject.from_file(res.project_file)
    assert proj.get_text_variable("REV") == "A"
    assert proj.get_path("meta.filename") == "Demo.kicad_pro"

    sch = KiCadSchematic.from_file(res.schematic_file)
    assert sch.paper.size == "D"
    assert sch.title_block is not None and sch.title_block.title == "Top Level"
    assert sch.uuid  # a real uuid, not empty


def test_create_project_default_sheet_name_is_project_name(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(name="MyProj", directory=tmp_path))
    sch = KiCadSchematic.from_file(res.schematic_file)
    assert sch.title_block is not None and sch.title_block.title == "MyProj"


def test_create_project_subdirectory_layout(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(name="Sub Project", directory=tmp_path))
    assert res.project_dir == tmp_path / "Sub Project"
    res_flat = create_project(KiCadProjectCreateOptions(
        name="Flat", directory=tmp_path / "flat", create_subdirectory=False,
    ))
    assert res_flat.project_dir == tmp_path / "flat"


def test_create_project_embeds_worksheet(tmp_path: Path) -> None:
    """A worksheet embeds into the schematic and the project references it."""
    zstandard = pytest.importorskip("zstandard")
    wks = tmp_path / "Company.kicad_wks"
    wks_bytes = b"(kicad_wks (version 1) (setup) )\n"
    wks.write_bytes(wks_bytes)

    res = create_project(KiCadProjectCreateOptions(
        name="Emb", directory=tmp_path / "out", worksheet=wks, embed_worksheet=True,
    ))

    proj = KiCadProject.from_file(res.project_file)
    assert proj.get_path("schematic.page_layout_descr_file") == "kicad-embed://Company.kicad_wks"

    sch = KiCadSchematic.from_file(res.schematic_file)
    worksheets = [f for f in sch.embedded_files if f.file_type == "worksheet"]
    assert len(worksheets) == 1 and worksheets[0].name == "Company.kicad_wks"
    decompressed = zstandard.ZstdDecompressor().decompress(base64.b64decode(worksheets[0].data))
    assert decompressed == wks_bytes  # byte-faithful embed


def test_create_project_references_worksheet_when_embed_off(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(
        name="Ref", directory=tmp_path,
        worksheet="C:/wks/Company.kicad_wks", embed_worksheet=False,
    ))
    proj = KiCadProject.from_file(res.project_file)
    ref = proj.get_path("schematic.page_layout_descr_file") or ""
    assert "kicad-embed" not in ref and "Company.kicad_wks" in ref
    assert KiCadSchematic.from_file(res.schematic_file).embedded_files == []


def test_create_project_no_worksheet(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(name="None", directory=tmp_path, worksheet=None))
    proj = KiCadProject.from_file(res.project_file)
    assert proj.get_path("schematic") is None
    assert KiCadSchematic.from_file(res.schematic_file).embedded_files == []


def test_create_project_lib_tables_and_optional_pcb(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(
        name="Full", directory=tmp_path, page_size="D",
        create_lib_tables=True, create_pcb=True,
    ))
    assert res.symbol_table is not None and res.symbol_table.exists()
    assert res.footprint_table is not None and res.footprint_table.exists()
    assert res.symbol_table.read_text(encoding="utf-8").startswith("(sym_lib_table")
    assert res.footprint_table.read_text(encoding="utf-8").startswith("(fp_lib_table")
    assert res.pcb_file is not None and res.pcb_file.exists()
    assert KiCadPcb.from_file(res.pcb_file).paper == "D"


def test_create_project_no_lib_tables_or_pcb_by_request(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(
        name="Lean", directory=tmp_path, create_lib_tables=False, create_pcb=False,
    ))
    assert res.symbol_table is None and res.footprint_table is None and res.pcb_file is None
    names = {p.name for p in res.project_dir.iterdir()}
    assert names == {"Lean.kicad_pro", "Lean.kicad_sch"}


def test_create_project_title_block_metadata(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(
        name="Meta", directory=tmp_path,
        revision="A", company="Wavenumber", date="2026-06-22", comments=["Confidential"],
    ))
    tb = KiCadSchematic.from_file(res.schematic_file).title_block
    assert tb is not None
    assert tb.rev == "A" and tb.company == "Wavenumber" and tb.date == "2026-06-22"
    assert tb.comments == {1: "Confidential"}


def test_create_project_seeds_library_tables(tmp_path: Path) -> None:
    res = create_project(KiCadProjectCreateOptions(
        name="Libs", directory=tmp_path, create_lib_tables=False,
        symbol_libraries=[{"name": "WN", "uri": "${KIPRJMOD}/WN.kicad_sym"}],
        footprint_libraries=[{"name": "WN_FP", "uri": "${KIPRJMOD}/WN.pretty"}],
    ))
    assert res.symbol_table is not None and "WN" in res.symbol_table.read_text(encoding="utf-8")
    assert res.footprint_table is not None and "WN_FP" in res.footprint_table.read_text(encoding="utf-8")


def test_create_project_rejects_library_without_uri(tmp_path: Path) -> None:
    with pytest.raises(ValueError):
        create_project(KiCadProjectCreateOptions(
            name="Bad", directory=tmp_path, symbol_libraries=[{"name": "X"}],
        ))


def test_schematic_preserves_embedded_files_on_roundtrip() -> None:
    """KiCadSchematic now round-trips (embedded_files); previously they were dropped."""
    sch = KiCadSchematic()
    sch.uuid = "11111111-1111-1111-1111-111111111111"
    sch.embedded_files.append(
        EmbeddedFile(name="W.kicad_wks", file_type="worksheet", data="QUFBQQ==", checksum="abc")
    )
    reparsed = KiCadSchematic.from_text(sch.to_text())
    assert len(reparsed.embedded_files) == 1
    assert reparsed.embedded_files[0].name == "W.kicad_wks"
    assert reparsed.embedded_files[0].file_type == "worksheet"


def test_create_project_requires_name(tmp_path: Path) -> None:
    with pytest.raises(ValueError):
        create_project(KiCadProjectCreateOptions(name="", directory=tmp_path))


@pytest.mark.slow
def test_create_project_schematic_accepted_by_kicad_cli(tmp_path: Path) -> None:
    """The generated schematic loads cleanly in KiCad's own CLI."""
    cli = resolve_kicad_cli()
    if cli is None:
        pytest.skip("kicad-cli not found")
    res = create_project(KiCadProjectCreateOptions(
        name="CliCheck", directory=tmp_path, page_size="D", sheet_name="Top",
    ))
    report = tmp_path / "erc.rpt"
    completed = subprocess.run(
        [str(cli), "sch", "erc", "--output", str(report), str(res.schematic_file)],
        capture_output=True, text=True, timeout=120, check=False,
    )
    assert completed.returncode == 0, completed.stderr or completed.stdout

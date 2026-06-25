"""Subtest: KiCad new-project scaffolding primitives
Stratum: L2_tools
Purpose: Object-model primitives behind a new-project command — blank project,
the ``KiCadProject`` aggregate builder (schematic with page size + sheet name,
embedded/referenced worksheet, seeded lib tables, optional PCB), and the default
board layer stack.

The user-facing command and its option schema live in ``kicad_cruncher``; these
tests cover the ``kicad_monkey`` APIs it composes (issue #4). kicad_monkey holds
no knowledge of how inputs are gathered.
"""

from __future__ import annotations

import base64
import subprocess
from pathlib import Path

import pytest

from kicad_monkey import (
    KiCadPcb,
    KiCadProject,
    KiCadProjectFiles,
    KiCadSchematic,
)
from kicad_monkey.kicad_base import LayerType
from kicad_monkey.kicad_library_extraction import resolve_kicad_cli
from kicad_monkey.kicad_model import EmbeddedFile
from kicad_monkey.kicad_pcb_other import Layer


# --- blank project seed -----------------------------------------------------

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


# --- aggregate constructor --------------------------------------------------

def test_project_positional_constructor_seeds_blank(tmp_path: Path) -> None:
    """``KiCadProject(name, directory)`` is a writable blank project."""
    prj = KiCadProject("My Board", tmp_path)
    assert prj.name == "My Board"
    assert prj.directory == tmp_path
    assert prj.get_path("meta.filename") == "My Board.kicad_pro"
    # text_variables behaves as a plain attribute the caller can populate
    prj.set_text_variable("REV", "A")
    assert prj.get_text_variable("REV") == "A"


def test_project_create_classmethod_matches_constructor(tmp_path: Path) -> None:
    prj = KiCadProject.create("Demo", tmp_path)
    assert prj.name == "Demo" and prj.directory == tmp_path


# --- aggregate assembly -----------------------------------------------------

def test_add_schematic_sets_page_size_and_title(tmp_path: Path) -> None:
    prj = KiCadProject("Demo", tmp_path)
    sch = prj.add_schematic(page_size="D", sheet_name="Top Level")
    assert sch.paper.size == "D"
    assert sch.title_block is not None and sch.title_block.title == "Top Level"
    assert sch.uuid  # a real uuid, not empty
    assert prj.schematic is sch


def test_add_schematic_defaults_title_to_project_name(tmp_path: Path) -> None:
    prj = KiCadProject("MyProj", tmp_path)
    sch = prj.add_schematic()
    assert sch.title_block is not None and sch.title_block.title == "MyProj"


def test_write_project_minimal(tmp_path: Path) -> None:
    prj = KiCadProject("Demo", tmp_path)
    prj.set_text_variable("REV", "A")
    prj.add_schematic(page_size="D", sheet_name="Top Level")
    files = prj.write_project()

    assert isinstance(files, KiCadProjectFiles)
    assert files.project_file.exists() and files.schematic_file is not None
    assert files.schematic_file.exists()
    assert {p.name for p in files.project_dir.iterdir()} == {"Demo.kicad_pro", "Demo.kicad_sch"}

    proj = KiCadProject.from_file(files.project_file)
    assert proj.get_text_variable("REV") == "A"
    assert proj.get_path("meta.filename") == "Demo.kicad_pro"

    sch = KiCadSchematic.from_file(files.schematic_file)
    assert sch.paper.size == "D" and sch.uuid


def test_write_project_directory_override(tmp_path: Path) -> None:
    prj = KiCadProject("Flat", tmp_path)
    prj.add_schematic()
    target = tmp_path / "elsewhere"
    files = prj.write_project(target)
    assert files.project_dir == target
    assert (target / "Flat.kicad_pro").exists()


def test_embed_worksheet_into_schematic(tmp_path: Path) -> None:
    """A worksheet embeds into the schematic and the project references it."""
    zstandard = pytest.importorskip("zstandard")
    wks = tmp_path / "Company.kicad_wks"
    wks_bytes = b"(kicad_wks (version 1) (setup) )\n"
    wks.write_bytes(wks_bytes)

    prj = KiCadProject("Emb", tmp_path / "out")
    prj.add_schematic()
    prj.set_worksheet(wks, embed=True)
    files = prj.write_project()

    proj = KiCadProject.from_file(files.project_file)
    assert proj.get_path("schematic.page_layout_descr_file") == "kicad-embed://Company.kicad_wks"

    assert files.schematic_file is not None
    sch = KiCadSchematic.from_file(files.schematic_file)
    worksheets = [f for f in sch.embedded_files if f.file_type == "worksheet"]
    assert len(worksheets) == 1 and worksheets[0].name == "Company.kicad_wks"
    decompressed = zstandard.ZstdDecompressor().decompress(base64.b64decode(worksheets[0].data))
    assert decompressed == wks_bytes  # byte-faithful embed


def test_reference_worksheet_when_embed_off(tmp_path: Path) -> None:
    prj = KiCadProject("Ref", tmp_path)
    prj.add_schematic()
    prj.set_worksheet("C:/wks/Company.kicad_wks", embed=False)
    files = prj.write_project()
    proj = KiCadProject.from_file(files.project_file)
    ref = proj.get_path("schematic.page_layout_descr_file") or ""
    assert "kicad-embed" not in ref and "Company.kicad_wks" in ref
    assert files.schematic_file is not None
    assert KiCadSchematic.from_file(files.schematic_file).embedded_files == []


def test_set_worksheet_embed_requires_schematic(tmp_path: Path) -> None:
    prj = KiCadProject("NoSch", tmp_path)
    with pytest.raises(ValueError):
        prj.set_worksheet(tmp_path / "x.kicad_wks", embed=True)


def test_write_project_no_worksheet(tmp_path: Path) -> None:
    prj = KiCadProject("None", tmp_path)
    prj.add_schematic()
    files = prj.write_project()
    proj = KiCadProject.from_file(files.project_file)
    assert proj.get_path("schematic") is None
    assert files.schematic_file is not None
    assert KiCadSchematic.from_file(files.schematic_file).embedded_files == []


def test_library_tables_and_optional_pcb(tmp_path: Path) -> None:
    prj = KiCadProject("Full", tmp_path)
    prj.add_schematic(page_size="D")
    prj.ensure_library_tables()
    prj.add_pcb(page_size="D")
    files = prj.write_project()

    assert files.symbol_table is not None and files.symbol_table.exists()
    assert files.footprint_table is not None and files.footprint_table.exists()
    assert files.symbol_table.read_text(encoding="utf-8").startswith("(sym_lib_table")
    assert files.footprint_table.read_text(encoding="utf-8").startswith("(fp_lib_table")
    assert files.pcb_file is not None and files.pcb_file.exists()
    pcb = KiCadPcb.from_file(files.pcb_file)
    assert pcb.paper == "D"
    layer_names = {layer.canonical_name for layer in pcb.layers}
    assert {"F.Cu", "B.Cu", "Edge.Cuts"} <= layer_names


def test_write_project_lean_by_default(tmp_path: Path) -> None:
    """Without library tables or a PCB requested, only the core pair is written."""
    prj = KiCadProject("Lean", tmp_path)
    prj.add_schematic()
    files = prj.write_project()
    assert files.symbol_table is None and files.footprint_table is None
    assert files.pcb_file is None
    assert {p.name for p in files.project_dir.iterdir()} == {"Lean.kicad_pro", "Lean.kicad_sch"}


def test_title_block_metadata(tmp_path: Path) -> None:
    prj = KiCadProject("Meta", tmp_path)
    prj.add_schematic(
        revision="A", company="Wavenumber", date="2026-06-22", comments=["Confidential"],
    )
    files = prj.write_project()
    assert files.schematic_file is not None
    tb = KiCadSchematic.from_file(files.schematic_file).title_block
    assert tb is not None
    assert tb.rev == "A" and tb.company == "Wavenumber" and tb.date == "2026-06-22"
    assert tb.comments == {1: "Confidential"}


def test_seed_library_tables(tmp_path: Path) -> None:
    prj = KiCadProject("Libs", tmp_path)
    prj.add_schematic()
    prj.add_symbol_library("WN", "${KIPRJMOD}/WN.kicad_sym")
    prj.add_footprint_library("WN_FP", "${KIPRJMOD}/WN.pretty")
    files = prj.write_project()
    assert files.symbol_table is not None and "WN" in files.symbol_table.read_text(encoding="utf-8")
    assert files.footprint_table is not None
    assert "WN_FP" in files.footprint_table.read_text(encoding="utf-8")


def test_write_project_requires_name_and_directory(tmp_path: Path) -> None:
    blank = KiCadProject()  # no name/directory
    with pytest.raises(ValueError):
        blank.write_project(tmp_path)


# --- embedded files round-trip (latent data-loss fix) -----------------------

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


def test_embedded_file_from_worksheet_packs_zstd_sha256(tmp_path: Path) -> None:
    zstandard = pytest.importorskip("zstandard")
    import hashlib

    wks = tmp_path / "S.kicad_wks"
    payload = b"(kicad_wks (version 1))\n"
    wks.write_bytes(payload)
    ef = EmbeddedFile.from_worksheet(wks)
    assert ef.file_type == "worksheet" and ef.name == "S.kicad_wks"
    assert ef.checksum == hashlib.sha256(payload).hexdigest()
    assert zstandard.ZstdDecompressor().decompress(base64.b64decode(ef.data)) == payload


# --- default board layer stack (built via the object model) -----------------

def test_kicad_pcb_new_builds_layer_objects() -> None:
    """KiCadPcb.new() composes the default stack from ``Layer`` objects."""
    pcb = KiCadPcb.new(paper="D")
    assert pcb.paper == "D" and pcb.setup_sexp is not None
    assert len(pcb.layers) == 24
    assert all(isinstance(layer, Layer) for layer in pcb.layers)
    copper = {layer.canonical_name for layer in pcb.layers if layer.layer_type is LayerType.SIGNAL}
    assert copper == {"F.Cu", "B.Cu"}
    names = {layer.canonical_name for layer in pcb.layers}
    assert {"F.Cu", "B.Cu", "Edge.Cuts", "F.SilkS", "B.SilkS"} <= names


@pytest.mark.slow
def test_pcb_accepted_by_kicad_cli(tmp_path: Path) -> None:
    """The generated board loads cleanly in KiCad's own CLI (no '0 layers')."""
    cli = resolve_kicad_cli()
    if cli is None:
        pytest.skip("kicad-cli not found")
    prj = KiCadProject("PcbCli", tmp_path)
    prj.add_schematic()
    prj.add_pcb()
    files = prj.write_project()
    assert files.pcb_file is not None
    report = tmp_path / "drc.rpt"
    completed = subprocess.run(
        [str(cli), "pcb", "drc", "--output", str(report), str(files.pcb_file)],
        capture_output=True, text=True, timeout=120, check=False,
    )
    assert completed.returncode == 0, completed.stderr or completed.stdout


@pytest.mark.slow
def test_schematic_accepted_by_kicad_cli(tmp_path: Path) -> None:
    """The generated schematic loads cleanly in KiCad's own CLI."""
    cli = resolve_kicad_cli()
    if cli is None:
        pytest.skip("kicad-cli not found")
    prj = KiCadProject("CliCheck", tmp_path)
    prj.add_schematic(page_size="D", sheet_name="Top")
    files = prj.write_project()
    assert files.schematic_file is not None
    report = tmp_path / "erc.rpt"
    completed = subprocess.run(
        [str(cli), "sch", "erc", "--output", str(report), str(files.schematic_file)],
        capture_output=True, text=True, timeout=120, check=False,
    )
    assert completed.returncode == 0, completed.stderr or completed.stdout

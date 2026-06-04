"""Public workflow tests for BOM, PnP, and JLC manufacturing commands."""

from __future__ import annotations

import json
import subprocess
import sys
import zipfile
from pathlib import Path

import pytest
from kicad_cruncher.bom_pnp_model import (
    load_bom_pnp_config,
    normalize_pnp_entries,
    pnp_table_rows,
    write_bom_pnp_config,
)
from kicad_cruncher.kicad_manufacturing_design import (
    KiCadManufacturingDesign,
    KiCadPnpEntry,
)

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_YOSHI_PROJECT = (
    _PROJECT_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "yoshi_mainboard"
    / "input"
    / "11-10080__yoshi-mainboard__A.kicad_pro"
)


def _run_cli(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    """Run the current checkout's CLI through the active Python environment."""
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=cwd or _PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _bom_by_ref(rows: list[dict[str, object]]) -> dict[str, dict[str, object]]:
    return {str(row["designator"]): row for row in rows}


def _placement_refs(payload: dict[str, object]) -> set[str]:
    placements = payload["placements"]
    assert isinstance(placements, list)
    return {str(row["designator"]) for row in placements}


def _pnp_by_ref(rows: list[KiCadPnpEntry]) -> dict[str, KiCadPnpEntry]:
    return {row.designator: row for row in rows}


def test_default_bom_pnp_config_template_documents_and_loads(tmp_path: Path) -> None:
    """Verify the generated generic config is documented JSONC."""
    config_path = tmp_path / "bom.config"

    write_bom_pnp_config(config_path)

    text = config_path.read_text(encoding="utf-8")
    assert text.startswith("/*")
    assert text.count("/*") == 1
    assert text.count("*/") == 1
    header, json_body = text.split("*/", 1)
    assert "KiCad Cruncher BOM/PnP/JLC config" in header
    assert "field_aliases" in header
    assert "variants" in header
    assert 'source_mode "schematic"' in header
    assert 'source_mode "pcb"' in header
    assert 'source_mode "merged"' in header
    assert "group_fields controls" in header
    assert "output_fields controls generic PnP" in header
    assert "DNP parts are always omitted from PnP/CPL outputs" in header
    assert 'position_mode defaults to "component-center"' in header
    assert "aux axis / drill-place file origin" in header
    assert "dir_template" in header
    assert "/*" not in json_body
    assert "//" not in json_body

    parsed_json = json.loads(json_body)
    assert parsed_json["schema"] == "kicad_cruncher.bom.config.v1"
    assert parsed_json["pnp"]["position_mode"] == "component-center"
    loaded = load_bom_pnp_config(config_path)
    assert loaded.bom_outputs == ("raw-json", "grouped-xlsx")
    assert loaded.pnp_outputs == ("json", "csv")
    assert loaded.bom_group_fields == ("mfg", "mpn", "description")
    assert loaded.pnp_position_mode == "component-center"
    assert loaded.output_dir_template == "{Command}"

    legacy_config_path = tmp_path / "legacy-bom.config.json"
    parsed_json["schema"] = "wn.kicad_cruncher.bom.config.v1"
    legacy_config_path.write_text(json.dumps(parsed_json), encoding="utf-8")
    assert load_bom_pnp_config(legacy_config_path).schema == (
        "kicad_cruncher.bom.config.v1"
    )


def test_yoshi_accelerometer_variants_drive_bom_and_pnp_selection() -> None:
    """Verify KiCad variant overrides select one accelerometer option at a time."""
    design = KiCadManufacturingDesign.from_file(_YOSHI_PROJECT)

    assert design.get_variants() == ["ADXL355", "IIM42352"]

    base_bom = _bom_by_ref(design.to_bom())
    adxl_bom = _bom_by_ref(design.to_bom("ADXL355"))
    iim_bom = _bom_by_ref(design.to_bom("IIM42352"))

    assert base_bom["U2"]["dnp"] is True
    assert base_bom["U3"]["dnp"] is False
    assert adxl_bom["U2"]["dnp"] is False
    assert adxl_bom["U3"]["dnp"] is True
    assert iim_bom["U2"]["dnp"] is True
    assert iim_bom["U3"]["dnp"] is False

    adxl_pnp = _pnp_by_ref(design.to_pnp("ADXL355"))
    iim_pnp = _pnp_by_ref(design.to_pnp("IIM42352"))
    assert "U2" in adxl_pnp
    assert "U3" not in adxl_pnp
    assert "U2" not in iim_pnp
    assert "U3" in iim_pnp

    assert adxl_pnp["U2"].center_x == pytest.approx(1.6)
    assert adxl_pnp["U2"].center_y == pytest.approx(-6.195)
    assert adxl_pnp["U2"].rotation == pytest.approx(0.0)
    assert iim_pnp["U3"].center_x == pytest.approx(1.6)
    assert iim_pnp["U3"].center_y == pytest.approx(-6.195)
    assert iim_pnp["U3"].rotation == pytest.approx(90.0)
    assert adxl_pnp["J1"].center_x == pytest.approx(0.0)
    assert adxl_pnp["J1"].center_y == pytest.approx(0.0)


def test_yoshi_pnp_configured_table_fields_resolve_virtual_and_alias_fields() -> None:
    """Verify configured PnP fields can mix generated values and parameters."""
    design = KiCadManufacturingDesign.from_file(_YOSHI_PROJECT)
    placements = normalize_pnp_entries(design.to_pnp("IIM42352"), units="mm")

    rows = pnp_table_rows(
        placements,
        fields=(
            "designator",
            "units",
            "mfg",
            "mpn",
            "part_number",
            "center_x",
            "center_y",
            "rotation",
        ),
    )

    u3 = next(row for row in rows if row["designator"] == "U3")
    assert u3["units"] == "mm"
    assert u3["mfg"] == "TDK InvenSense"
    assert u3["mpn"] == "IIM-42352"
    assert u3["part_number"] == "IIM-42352"
    assert u3["center_x"] == "1.6"
    assert u3["center_y"] == "-6.195"
    assert u3["rotation"] == "90"



def test_bom_pnp_and_jlc_commands_emit_yoshi_variant_outputs(tmp_path: Path) -> None:
    """Verify the public commands generate reviewable variant manufacturing files."""
    bom_dir = tmp_path / "bom"
    pnp_dir = tmp_path / "pnp"
    jlc_root = tmp_path / "jlc"

    bom_result = _run_cli(
        "bom",
        str(_YOSHI_PROJECT),
        "--variant",
        "ADXL355",
        "--format",
        "raw-json",
        "-o",
        str(bom_dir),
    )
    assert bom_result.returncode == 0, bom_result.stderr
    bom_path = bom_dir / "11-10080__yoshi-mainboard__A_ADXL355_bom.json"
    bom_rows = json.loads(bom_path.read_text(encoding="utf-8"))
    bom_by_ref = _bom_by_ref(bom_rows)
    assert bom_by_ref["U2"]["dnp"] is False
    assert bom_by_ref["U3"]["dnp"] is True

    pnp_result = _run_cli(
        "pnp",
        str(_YOSHI_PROJECT),
        "--variant",
        "ADXL355",
        "--format",
        "json",
        "-o",
        str(pnp_dir),
    )
    assert pnp_result.returncode == 0, pnp_result.stderr
    pnp_path = pnp_dir / "11-10080__yoshi-mainboard__A_ADXL355_pnp.json"
    pnp_payload = json.loads(pnp_path.read_text(encoding="utf-8"))
    assert pnp_payload["schema"] == "wn.kicad_cruncher.pnp.v1"
    pnp_refs = _placement_refs(pnp_payload)
    assert "U2" in pnp_refs
    assert "U3" not in pnp_refs

    jlc_result = _run_cli(
        "jlc",
        str(_YOSHI_PROJECT),
        "--variant",
        "ADXL355",
        "-o",
        str(jlc_root),
    )
    assert jlc_result.returncode == 0, jlc_result.stderr
    bom_workbooks = list(jlc_root.rglob("*_jlc-xlsx.xlsx"))
    cpl_workbooks = list(jlc_root.rglob("*_jlc-cpl-xlsx.xlsx"))
    assert len(bom_workbooks) == 1
    assert len(cpl_workbooks) == 1
    assert zipfile.is_zipfile(bom_workbooks[0])
    assert zipfile.is_zipfile(cpl_workbooks[0])
    assert (bom_workbooks[0].parent / "bom.config.used.json").exists()

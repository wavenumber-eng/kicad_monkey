"""L3 public workflow tests for megamaid library extraction."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
from kicad_cruncher.kicad_cruncher_cmd_health import _health_payload
from kicad_monkey.kicad_library_extraction import resolve_kicad_cli

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_FOUR_CH_PROJECT = (
    _PROJECT_ROOT
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "4-ch-backplane"
    / "input"
    / "4-ch-backplane.kicad_pro"
)


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=_PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _fake_kicad_cli(tmp_path: Path) -> tuple[Path, Path]:
    log_path = tmp_path / "fake-kicad-cli.log"
    if os.name == "nt":
        cli_path = tmp_path / "kicad-cli.cmd"
        cli_path.write_text(
            f'@echo off\r\necho %*>>"{log_path}"\r\nexit /b 0\r\n',
            encoding="utf-8",
        )
        return cli_path, log_path

    cli_path = tmp_path / "kicad-cli"
    cli_path.write_text(
        f"#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log_path}'\nexit 0\n",
        encoding="utf-8",
    )
    cli_path.chmod(0o755)
    return cli_path, log_path


def test_megamaid_extracts_4ch_backplane_bundle(tmp_path: Path) -> None:
    """Verify megamaid writes a usable extraction bundle for the real-world fixture."""
    output_dir = tmp_path / "megamaid"
    result = _run_cli(
        "megamaid",
        str(_FOUR_CH_PROJECT),
        "--output",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr
    assert "Megamaid: extracting schematic symbols" in result.stdout
    assert "Megamaid: extracting embedded files, images, and bitmaps" in result.stdout
    assert "Megamaid: embedded assets: inventory found" in result.stdout
    assert "Megamaid: starting design review bundle" in result.stdout
    assert "Megamaid: design review: rendering schematic SVG" in result.stdout
    assert "Megamaid: design review: rendering PCB review SVG" in result.stdout
    assert "Megamaid: wrote design review bundle" in result.stdout

    manifest_path = output_dir / "megamaid_manifest.json"
    metadata_path = output_dir / "library_extraction.json"
    readme_path = output_dir / "README.md"
    symbols_dir = output_dir / "symbols"
    footprints_dir = output_dir / "footprints.pretty"
    models_dir = output_dir / "models"
    embedded_assets_dir = output_dir / "embedded_assets"
    design_review_dir = output_dir / "design_review"

    assert manifest_path.is_file()
    assert metadata_path.is_file()
    assert readme_path.is_file()
    assert symbols_dir.is_dir()
    assert footprints_dir.is_dir()
    assert models_dir.is_dir()
    assert embedded_assets_dir.is_dir()
    assert design_review_dir.is_dir()

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

    assert manifest["schema"] == "kicad_cruncher.megamaid_manifest.a0"
    assert manifest["mode"] == "internal"
    assert manifest["symbols"]["count"] >= 65
    assert manifest["footprints"]["count"] >= 40
    assert manifest["models"]["count"] >= 1
    assert manifest["metadata"] == "library_extraction.json"
    assert manifest["embedded_assets"]["directory"] == "embedded_assets"
    assert manifest["embedded_assets"]["record_count"] >= 1
    assert manifest["embedded_assets"]["file_count"] >= 1
    assert manifest["embedded_assets"]["records"]
    assert manifest["design_review"]["directory"] == "design_review"
    assert manifest["design_review"]["manifest"] == "design_review/design_review_manifest.json"
    assert manifest["design_review"]["design_json"].endswith("_design.json")
    assert manifest["design_review"]["netlist_json"].endswith("_netlist.json")
    assert manifest["design_review"]["netlist_kicad_sexpr"].endswith("_netlist.net")
    assert manifest["design_review"]["component_count"] >= 1
    assert manifest["design_review"]["net_count"] >= 1
    assert manifest["design_review"]["schematic_svg_count"] >= 1
    assert manifest["design_review"]["pcb_svg_count"] >= 1
    assert (output_dir / manifest["design_review"]["manifest"]).is_file()
    assert (output_dir / manifest["design_review"]["design_json"]).is_file()
    assert (output_dir / manifest["design_review"]["netlist_json"]).is_file()
    assert (output_dir / manifest["design_review"]["netlist_kicad_sexpr"]).is_file()
    assert all(
        (output_dir / path).is_file()
        for path in manifest["design_review"]["schematic_svgs"]
    )
    assert all((output_dir / path).is_file() for path in manifest["design_review"]["pcb_svgs"])
    assert metadata["schema"] == "kicad_cruncher.library_extraction_bundle.a0"
    assert metadata["mode"] == "internal"
    assert "assets" not in metadata
    assert len(metadata["symbols"]) == manifest["symbols"]["count"]
    assert len(metadata["footprints"]) == manifest["footprints"]["count"]

    assert len(list(symbols_dir.glob("*.kicad_sym"))) == manifest["symbols"]["count"]
    assert len(list(footprints_dir.glob("*.kicad_mod"))) == manifest["footprints"]["count"]
    model_count = sum(
        1 for path in models_dir.iterdir() if path.suffix.lower() in {".step", ".stp"}
    )
    assert model_count == manifest["models"]["count"]
    readme_text = readme_path.read_text(encoding="utf-8")
    assert "Embedded assets" in readme_text
    assert "Design review" in readme_text
    assert "non-destructive" in readme_text


def test_lib_extract_validate_kicad_cli_runs_stripped_symbols(tmp_path: Path) -> None:
    """Verify lib-extract validates generated stripped symbols through kicad-cli."""
    output_dir = tmp_path / "lib-extract"
    fake_cli, fake_cli_log = _fake_kicad_cli(tmp_path)
    result = _run_cli(
        "lib-extract",
        str(_FOUR_CH_PROJECT),
        "--output",
        str(output_dir),
        "--validate-kicad-cli",
        "--kicad-cli",
        str(fake_cli),
    )

    assert result.returncode == 0, result.stderr

    manifest = json.loads((output_dir / "lib_extract_manifest.json").read_text(encoding="utf-8"))
    metadata = json.loads((output_dir / "library_extraction.json").read_text(encoding="utf-8"))
    validation = manifest["validation"]
    assert validation["ok"] is True
    assert len(validation["symbol_results"]) == manifest["symbols"]["count"]
    assert all(item["ok"] for item in validation["symbol_results"])
    assert validation["footprint_result"]["ok"] is True
    assert len(metadata["symbols"]) == manifest["symbols"]["count"]
    assert all(
        symbol["raw_fields"].get("Reference") != "#PWR"
        for symbol in metadata["symbols"]
    )
    assert not any(
        symbol["name"].startswith("power:")
        for symbol in metadata["symbols"]
    )
    assert any(
        {"description", "mfg", "mpn", "value"} <= set(symbol["canonical_fields"])
        for symbol in metadata["symbols"]
    )

    cli_calls = fake_cli_log.read_text(encoding="utf-8").splitlines()
    assert any(call.startswith("sym upgrade ") and ".kicad_sym" in call for call in cli_calls)
    assert any(call.startswith("fp upgrade ") and "footprints.pretty" in call for call in cli_calls)


def test_lib_extract_live_kicad_cli_accepts_stripped_symbols(tmp_path: Path) -> None:
    """Optionally verify generated stripped libraries with a real local kicad-cli."""
    kicad_cli = resolve_kicad_cli()
    if kicad_cli is None:
        pytest.skip("kicad-cli not found; install KiCad to run live library validation")

    output_dir = tmp_path / "lib-extract-live"
    result = _run_cli(
        "lib-extract",
        str(_FOUR_CH_PROJECT),
        "--output",
        str(output_dir),
        "--validate-kicad-cli",
        "--kicad-cli",
        str(kicad_cli),
    )

    assert result.returncode == 0, result.stderr

    manifest = json.loads((output_dir / "lib_extract_manifest.json").read_text(encoding="utf-8"))
    validation = manifest["validation"]
    assert validation["ok"] is True
    assert len(validation["symbol_results"]) == manifest["symbols"]["count"]
    assert validation["footprint_result"]["ok"] is True


def test_project_lib_extracts_4ch_backplane_bundle(tmp_path: Path) -> None:
    """Verify project-lib writes metadata-preserving project-local artifacts."""
    project_copy = tmp_path / "4-ch-backplane"
    shutil.copytree(_FOUR_CH_PROJECT.parent, project_copy)
    project_path = project_copy / _FOUR_CH_PROJECT.name
    output_dir = tmp_path / "project-lib"
    result = _run_cli(
        "project-lib",
        str(project_path),
        "--output",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr

    manifest_path = output_dir / "project_lib_manifest.json"
    metadata_path = output_dir / "library_extraction.json"
    symbols_dir = output_dir / "4-ch-backplane"
    footprints_dir = output_dir / "4-ch-backplane.pretty"
    models_dir = output_dir / "models"

    assert manifest_path.is_file()
    assert metadata_path.is_file()
    assert symbols_dir.is_dir()
    assert footprints_dir.is_dir()
    assert not models_dir.exists()

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

    assert manifest["schema"] == "kicad_cruncher.project_lib_manifest.a0"
    assert manifest["mode"] == "project_local"
    assert manifest["symbols"]["directory"] == "4-ch-backplane"
    assert manifest["footprints"]["directory"] == "4-ch-backplane.pretty"
    assert manifest["symbols"]["count"] >= 65
    assert 40 <= manifest["footprints"]["count"] < 832
    assert manifest["models"]["count"] == 0
    assert metadata["schema"] == "kicad_cruncher.library_extraction_bundle.a0"
    assert metadata["mode"] == "project_local"
    assert "assets" not in metadata
    assert len(metadata["symbols"]) == manifest["symbols"]["count"]
    assert len(metadata["footprints"]) == manifest["footprints"]["count"]
    assert manifest["library_tables"]["changed"] is True
    assert manifest["library_tables"]["symbol"]["action"] == "added"
    assert manifest["library_tables"]["footprint"]["action"] == "added"

    symbol_names = {path.name for path in symbols_dir.glob("*.kicad_sym")}
    assert "+3v3.kicad_sym" in symbol_names
    assert "+3v3_2.kicad_sym" not in symbol_names
    assert "ERJ-2RKF1002X_1.kicad_sym" in symbol_names
    assert "ERJ-2RKF1002X_1_2.kicad_sym" not in symbol_names
    assert "KSZ9896CTXC.kicad_sym" in symbol_names
    assert (footprints_dir / "TQFP128_EP_TX_MCH.kicad_mod").is_file()
    ksz_text = (symbols_dir / "KSZ9896CTXC.kicad_sym").read_text(encoding="utf-8")
    assert '(symbol "KSZ9896CTXC"' in ksz_text
    assert 'Footprint" "4-ch-backplane:TQFP128_EP_TX_MCH"' in ksz_text
    assert "KSZ9896CTXC_1_1" in ksz_text
    assert "KSZ9896CTXC_2_1" in ksz_text
    assert "KSZ9896CTXC_3_1" in ksz_text

    assert "4-ch-backplane" in (project_copy / "sym-lib-table").read_text(
        encoding="utf-8"
    )
    assert "4-ch-backplane" in (project_copy / "fp-lib-table").read_text(
        encoding="utf-8"
    )


def test_project_lib_table_update_is_idempotent(tmp_path: Path) -> None:
    """Verify repeated project-lib runs do not duplicate local table entries."""
    project_copy = tmp_path / "4-ch-backplane"
    shutil.copytree(_FOUR_CH_PROJECT.parent, project_copy)
    project_path = project_copy / _FOUR_CH_PROJECT.name
    output_dir = tmp_path / "project-lib"
    command = (
        "project-lib",
        str(project_path),
        "--output",
        str(output_dir),
        "--symbol-library-dir",
        "local-symbols",
        "--footprint-library-dir",
        "local-footprints",
        "--symbol-library-name",
        "local-symbols-nick",
        "--footprint-library-name",
        "local-footprints-nick",
    )

    first = _run_cli(*command)
    assert first.returncode == 0, first.stderr
    second = _run_cli(*command)
    assert second.returncode == 0, second.stderr

    manifest = json.loads((output_dir / "project_lib_manifest.json").read_text(encoding="utf-8"))
    assert manifest["symbols"]["directory"] == "local-symbols"
    assert manifest["footprints"]["directory"] == "local-footprints.pretty"
    assert manifest["library_tables"]["changed"] is False
    assert manifest["library_tables"]["symbol"]["action"] == "already_exists"
    assert manifest["library_tables"]["footprint"]["action"] == "already_exists"
    assert (project_copy / "sym-lib-table").read_text(encoding="utf-8").count(
        '(name "local-symbols-nick")'
    ) == 1
    assert (project_copy / "fp-lib-table").read_text(encoding="utf-8").count(
        '(name "local-footprints-nick")'
    ) == 1


def test_health_scans_4ch_backplane_assets(tmp_path: Path) -> None:
    """Verify health writes an asset diagnostic report."""
    output_dir = tmp_path / "health"
    result = _run_cli(
        "health",
        str(_FOUR_CH_PROJECT),
        "--output",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr

    report_path = output_dir / "project_health.json"
    readme_path = output_dir / "README.md"

    assert report_path.is_file()
    assert readme_path.is_file()
    assert "KiCad project health:" in result.stdout
    assert "JSON:" in result.stdout

    report = json.loads(report_path.read_text(encoding="utf-8"))

    assert report["schema"] == "kicad_cruncher.project_health.a0"
    assert "ok" in report
    assert report["summary"]["schematics"] >= 1
    assert report["summary"]["pcbs"] >= 1
    assert report["summary"]["model_references"] >= 1
    assert report["assets"]["model_references"]
    readme = readme_path.read_text(encoding="utf-8")
    assert "project_health.json" in readme
    if report["summary"]["issues"]:
        assert "Issue kinds:" in readme


def test_health_payload_counts_footprints_without_models() -> None:
    """Verify placed footprints without model records are reported as health issues."""
    report = _health_payload(
        Path("project.kicad_pro"),
        {
            "schematics": [],
            "pcbs": ["board.kicad_pcb"],
            "symbol_libraries": [],
            "pretty_libraries": [],
            "footprint_files": [],
            "model_references": [],
            "footprints_without_models": [
                {
                    "footprint": "Device:R_0603",
                    "source_path": "board.kicad_pcb",
                    "designators": ["R1", "R2"],
                    "instance_count": 2,
                }
            ],
            "diagnostics": [],
        },
    )

    summary = report["summary"]
    issues = report["issues"]
    assert isinstance(summary, dict)
    assert isinstance(issues, dict)
    footprints_without_models = issues["footprints_without_models"]
    assert isinstance(footprints_without_models, list)
    first_footprint_issue = footprints_without_models[0]
    assert isinstance(first_footprint_issue, dict)

    assert report["ok"] is False
    assert summary["issues"] == 1
    assert summary["footprints_without_models"] == 1
    assert summary["footprint_instances_without_models"] == 2
    assert summary["issue_kinds"] == {"footprint_without_model": 1}
    assert first_footprint_issue["designators"] == ["R1", "R2"]


def test_health_payload_dedups_missing_model_across_libraries() -> None:
    """Verify one missing model file is counted once even with many references.

    Regression for issue #3: project-lib copies the footprint into the generated
    local library and leaves the model reference external, so the same missing
    STEP is scanned from both the base PCB and the library copy. Health must
    report one distinct missing model with both reference sites listed, not two.
    """
    missing_model = "${KICAD9_3DMODEL_DIR}/Diode_SMD.3dshapes/Diodes_UDFN2020-6_Type-F.step"
    report = _health_payload(
        Path("project.kicad_pro"),
        {
            "schematics": [],
            "pcbs": ["board.kicad_pcb"],
            "symbol_libraries": [],
            "pretty_libraries": [],
            "footprint_files": [],
            "model_references": [
                {
                    "source_kind": "footprint",
                    "source_path": "footprints.pretty/Diodes_UDFN2020-6.kicad_mod",
                    "owner": "Diodes_UDFN2020-6",
                    "model_path": missing_model,
                    "reference_kind": "env_var",
                    "resolved_path": "",
                    "exists": False,
                },
                {
                    "source_kind": "footprint",
                    "source_path": "local-library/Diodes_UDFN2020-6.kicad_mod",
                    "owner": "Diodes_UDFN2020-6",
                    "model_path": missing_model,
                    "reference_kind": "env_var",
                    "resolved_path": "",
                    "exists": False,
                },
            ],
            "footprints_without_models": [],
            "diagnostics": [],
        },
    )

    summary = report["summary"]
    issues = report["issues"]
    assert isinstance(summary, dict)
    assert isinstance(issues, dict)

    assert report["ok"] is False
    assert summary["model_references"] == 2
    assert summary["issues"] == 1
    assert summary["issue_kinds"] == {"missing_or_unresolved_model": 1}

    model_issues = issues["model_references"]
    assert isinstance(model_issues, list)
    assert len(model_issues) == 1
    grouped = model_issues[0]
    assert isinstance(grouped, dict)
    assert grouped["model_path"] == missing_model
    assert grouped["reference_count"] == 2
    references = grouped["references"]
    assert isinstance(references, list)
    assert {str(site["source_path"]) for site in references} == {
        "footprints.pretty/Diodes_UDFN2020-6.kicad_mod",
        "local-library/Diodes_UDFN2020-6.kicad_mod",
    }


def test_library_extract_alias_help_starts() -> None:
    """Verify the library extraction alias starts."""
    for alias in ("library-extract",):
        result = _run_cli(alias, "--help")

        assert result.returncode == 0, result.stderr
        assert "lib_cruncher ingestion bundle" in result.stdout


def test_project_lib_alias_help_starts() -> None:
    """Verify the project-local aliases are wired to the same command surface."""
    for alias in ("project-library", "project-local-lib", "local-library"):
        result = _run_cli(alias, "--help")

        assert result.returncode == 0, result.stderr
        assert "metadata-preserving project-local" in result.stdout


def test_project_health_old_name_is_not_registered() -> None:
    """Verify the pre-release health command rename has no compatibility alias."""
    result = _run_cli("project-health", "--help")

    assert result.returncode != 0
    assert "invalid choice" in result.stderr

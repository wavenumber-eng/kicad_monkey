"""L3 public workflow tests for megamaid library extraction."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from kicad_cruncher.kicad_cruncher_cmd_health import _health_payload

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

    manifest_path = output_dir / "megamaid_manifest.json"
    metadata_path = output_dir / "library_extraction.json"
    readme_path = output_dir / "README.md"
    symbols_dir = output_dir / "symbols"
    footprints_dir = output_dir / "footprints.pretty"
    models_dir = output_dir / "models"

    assert manifest_path.is_file()
    assert metadata_path.is_file()
    assert readme_path.is_file()
    assert symbols_dir.is_dir()
    assert footprints_dir.is_dir()
    assert models_dir.is_dir()

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

    assert manifest["schema"] == "kicad_cruncher.megamaid_manifest.a0"
    assert manifest["mode"] == "internal"
    assert manifest["symbols"]["count"] >= 70
    assert manifest["footprints"]["count"] >= 40
    assert manifest["models"]["count"] >= 1
    assert manifest["metadata"] == "library_extraction.json"
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
    assert "non-destructive" in readme_path.read_text(encoding="utf-8")


def test_project_lib_extracts_4ch_backplane_bundle(tmp_path: Path) -> None:
    """Verify project-lib writes metadata-preserving project-local artifacts."""
    output_dir = tmp_path / "project-lib"
    result = _run_cli(
        "project-lib",
        str(_FOUR_CH_PROJECT),
        "--output",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr

    manifest_path = output_dir / "project_lib_manifest.json"
    metadata_path = output_dir / "library_extraction.json"
    symbols_dir = output_dir / "symbols"
    footprints_dir = output_dir / "footprints.pretty"

    assert manifest_path.is_file()
    assert metadata_path.is_file()
    assert symbols_dir.is_dir()
    assert footprints_dir.is_dir()

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

    assert manifest["schema"] == "kicad_cruncher.project_lib_manifest.a0"
    assert manifest["mode"] == "project_local"
    assert manifest["symbols"]["count"] >= 70
    assert manifest["footprints"]["count"] > 100
    assert metadata["schema"] == "kicad_cruncher.library_extraction_bundle.a0"
    assert metadata["mode"] == "project_local"
    assert "assets" not in metadata
    assert len(metadata["symbols"]) == manifest["symbols"]["count"]
    assert len(metadata["footprints"]) == manifest["footprints"]["count"]


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

    assert report["ok"] is False
    assert report["summary"]["issues"] == 1
    assert report["summary"]["footprints_without_models"] == 1
    assert report["summary"]["footprint_instances_without_models"] == 2
    assert report["summary"]["issue_kinds"] == {"footprint_without_model": 1}
    assert report["issues"]["footprints_without_models"][0]["designators"] == ["R1", "R2"]


def test_megamaid_alias_help_starts() -> None:
    """Verify the public aliases are wired to the same command surface."""
    for alias in ("library-extract", "lib-extract"):
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

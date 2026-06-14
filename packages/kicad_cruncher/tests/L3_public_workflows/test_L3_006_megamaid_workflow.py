"""L3 public workflow tests for megamaid library extraction."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

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
        "--no-asset-scan",
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

    assert manifest["schema"] == "kicad_cruncher.megamaid_manifest.v0"
    assert manifest["mode"] == "internal"
    assert manifest["symbols"]["count"] >= 70
    assert manifest["footprints"]["count"] >= 40
    assert manifest["models"]["count"] >= 1
    assert manifest["metadata"] == "library_extraction.json"
    assert metadata["schema"] == "kicad_monkey.library_extraction_bundle.v1"
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
        "--no-asset-scan",
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

    assert manifest["schema"] == "kicad_cruncher.project_lib_manifest.v0"
    assert manifest["mode"] == "project_local"
    assert manifest["symbols"]["count"] >= 70
    assert manifest["footprints"]["count"] > 100
    assert metadata["schema"] == "kicad_monkey.library_extraction_bundle.v1"
    assert metadata["mode"] == "project_local"
    assert "assets" not in metadata
    assert len(metadata["symbols"]) == manifest["symbols"]["count"]
    assert len(metadata["footprints"]) == manifest["footprints"]["count"]


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

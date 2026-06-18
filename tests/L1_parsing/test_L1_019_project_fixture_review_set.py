"""Review coverage for package-local real-world project fixtures."""

from __future__ import annotations

from pathlib import Path

import pytest

from _suite_paths import TEST_CORPUS_ROOT
from kicad_monkey.kicad_project import KiCadProject


PROJECT_CASES_ROOT = TEST_CORPUS_ROOT / "kicad" / "projects"
PROJECT_CASE_NAMES = (
    "4-ch-backplane",
    "canbob",
    "celebration_led_assembly",
    "cern_wren_eda_04903",
    "charge_indicator",
    "charge_indicator_assembly",
    "cm5_minima_rev2",
    "eez_dcp405plus",
    "icepi_sbc",
    "icepi_zero_v13",
    "jumperless_v5r7",
    "kibuzzard",
    "led_component",
    "nrf9151_feather",
    "speedy_processing_module",
    "synthetic-netlist-hierarchy",
    "taillight",
    "taillight_assembly",
    "yoshi_mainboard",
)


def _project_dirs() -> list[Path]:
    return sorted(
        path
        for path in PROJECT_CASES_ROOT.iterdir()
        if path.is_dir() and list(path.glob("input/**/*.kicad_pro"))
    )


def _project_files() -> list[Path]:
    return sorted(PROJECT_CASES_ROOT.glob("*/input/**/*.kicad_pro"))


def test_real_world_project_review_set_inventory() -> None:
    """Verify all copied project cases are present under the test tree."""
    assert [path.name for path in _project_dirs()] == list(PROJECT_CASE_NAMES)
    assert len(_project_files()) == 21


def test_real_world_project_review_set_excludes_generated_and_local_metadata() -> None:
    """Keep copied fixtures free of local metadata and generated output trees."""
    excluded_dirs = {
        path.name
        for path in PROJECT_CASES_ROOT.rglob("*")
        if path.is_dir()
        and path.name in {"output", "review", "review_tmp", ".git", ".history"}
    }

    assert excluded_dirs == set()


@pytest.mark.parametrize(
    "project_file",
    _project_files(),
    ids=lambda path: path.relative_to(PROJECT_CASES_ROOT).as_posix(),
)
def test_real_world_project_files_load(project_file: Path) -> None:
    """Every staged real-world project file should load as JSON project data."""
    project = KiCadProject.from_file(project_file)

    assert project.project_path == project_file
    assert project.raw

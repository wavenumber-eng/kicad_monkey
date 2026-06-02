"""Public workflow tests for the design command."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CORPUS_ROOT = _PROJECT_ROOT / "tests" / "corpus" / "kicad"
_CORPUS_LED_PCB = (
    _CORPUS_ROOT / "board_svg" / "input" / "led_component" / "led_component.kicad_pcb"
)
_CORPUS_PROJECT_CASES = (
    pytest.param(
        _CORPUS_ROOT / "board_svg" / "input" / "led_component" / "led_component.kicad_pro",
        "led_component_design.json",
        1,
        6,
        id="led_component",
    ),
    pytest.param(
        _CORPUS_ROOT
        / "projects"
        / "taillight"
        / "input"
        / "11-10045__taillight__C.kicad_pro",
        "11-10045__taillight__C_design.json",
        97,
        75,
        id="taillight",
    ),
    pytest.param(
        _CORPUS_ROOT
        / "projects"
        / "charge_indicator"
        / "input"
        / "11-10043__charge_indicator__C.kicad_pro",
        "11-10043__charge_indicator__C_design.json",
        117,
        76,
        id="charge_indicator",
    ),
)

_MIN_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-3333-4444-555555555555")
  (paper "A4")
)
"""

_MIN_PCB_TEXT = """(kicad_pcb
  (version 20241229)
  (generator "pcbnew")
  (generator_version "9.0")
  (general (thickness 1.6) (legacy_teardrops no))
  (paper "A4")
  (layers)
  (embedded_fonts no)
)
"""


def _run_cli(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    """Run the current checkout's CLI through the active Python environment."""
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=cwd or _PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _write_synthetic_project(root: Path) -> Path:
    """Write a redistributable minimal KiCad project fixture."""
    project_path = root / "demo.kicad_pro"
    project_path.write_text(
        json.dumps(
            {
                "text_variables": {"TITLE": "Demo"},
                "schematic": {
                    "variants": [
                        {"name": "Default"},
                        {"name": "Alt", "description": "alternate assembly"},
                    ]
                },
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    (root / "demo.kicad_sch").write_text(_MIN_SCH_TEXT, encoding="utf-8")
    (root / "demo.kicad_pcb").write_text(_MIN_PCB_TEXT, encoding="utf-8")
    return project_path


def _write_pcb_svg_config(root: Path, *, include_hlr: bool) -> Path:
    """Write a focused pcb.svg.config.a0 test config."""
    config_path = root / "pcb.svg.config"
    views = [
        {
            "name": "top_view",
            "enabled": True,
            "group_id": "pcb-svg-view-top",
            "output_svg": "top_view/{board}__top_view.svg",
            "layers": ["BOARD_OUTLINE", "F.Cu", "F.SilkS"],
            "assembly_hlr_mode": "none",
        }
    ]
    if include_hlr:
        views = [
            {
                "name": "assembly_top_view",
                "enabled": True,
                "group_id": "pcb-svg-view-assembly-top",
                "output_svg": "assembly_top_view/{board}__assembly_top_view.svg",
                "layers": ["BOARD_OUTLINE", "F.Cu", "ASSEMBLY_HLR_TOP"],
                "assembly_hlr_mode": "simple",
                "styles": {"assembly_hlr": {"curve_mode": "polyline"}},
            }
        ]
    config_path.write_text(
        json.dumps(
            {
                "schema": "pcb.svg.config.a0",
                "global": {"include_metadata": True, "show_empty_layers": False},
                "layer_outputs": {
                    "enabled": not include_hlr,
                    "layers": "auto",
                    "include_special_layers": ["BOARD_OUTLINE"],
                },
                "views": views,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return config_path


def test_design_command_generates_project_json(tmp_path: Path) -> None:
    """Verify design writes a KiCad-native JSON payload for a project."""
    project_path = _write_synthetic_project(tmp_path)
    output_dir = tmp_path / "out"

    result = _run_cli("design", str(project_path), "-o", str(output_dir))

    assert result.returncode == 0, result.stderr + result.stdout
    output_file = output_dir / "demo_design.json"
    assert output_file.exists()
    payload = json.loads(output_file.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.design.a0"
    assert payload["generator"] == "kicad_monkey"
    assert payload["project"]["text_variables"] == {"TITLE": "Demo"}
    assert isinstance(payload["components"], list)
    assert isinstance(payload["nets"], list)
    assert "indexes" in payload


@pytest.mark.parametrize(
    ("project_path", "output_name", "component_count", "net_count"), _CORPUS_PROJECT_CASES
)
def test_design_command_uses_copied_kicad_monkey_corpus_projects(
    tmp_path: Path, project_path: Path, output_name: str, component_count: int, net_count: int
) -> None:
    """Verify design runs against copied real KiCad Monkey corpus projects."""
    output_dir = tmp_path / "out"

    result = _run_cli("design", str(project_path), "-o", str(output_dir))

    assert result.returncode == 0, result.stderr + result.stdout
    payload = json.loads((output_dir / output_name).read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.design.a0"
    assert len(payload["components"]) == component_count
    assert len(payload["nets"]) == net_count
    assert "pnp" in payload


def test_design_command_can_auto_detect_single_project(tmp_path: Path) -> None:
    """Verify design auto-detects one project in the working directory."""
    _write_synthetic_project(tmp_path)

    result = _run_cli("design", "--no-indexes", cwd=tmp_path)

    assert result.returncode == 0, result.stderr + result.stdout
    output_file = tmp_path / "output" / "design" / "demo_design.json"
    payload = json.loads(output_file.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.design.a0"
    assert "indexes" not in payload


def test_design_command_rejects_pcb_only_input(tmp_path: Path) -> None:
    """Verify the first design command slice rejects PCB-only inputs."""
    pcb_path = tmp_path / "demo.kicad_pcb"
    pcb_path.write_text(_MIN_PCB_TEXT, encoding="utf-8")

    result = _run_cli("design", str(pcb_path), cwd=tmp_path)

    assert result.returncode == 1
    assert "Unsupported file type" in result.stdout


def test_pcb_svg_command_uses_public_kicad_pcb_with_explicit_config(tmp_path: Path) -> None:
    """Exercise pcb-svg layer outputs and configured views against a copied PCB."""
    config_path = _write_pcb_svg_config(tmp_path, include_hlr=False)
    output_dir = tmp_path / "pcb-svg"

    result = _run_cli(
        "pcb-svg",
        str(_CORPUS_LED_PCB),
        "--config",
        str(config_path),
        "-o",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr + result.stdout
    manifest = json.loads((output_dir / "led_component__views.json").read_text(encoding="utf-8"))
    assert manifest["schema"] == "pcb.svg.manifest.a0"
    assert manifest["board"] == "led_component"
    assert "F.Cu" in manifest["layer_outputs"]
    assert "B.Cu" in manifest["layer_outputs"]
    assert (output_dir / "layers" / "led_component__F.Cu.svg").exists()
    assert (output_dir / "layers" / "led_component__B.Cu.svg").exists()
    assert (output_dir / "top_view" / "led_component__top_view.svg").exists()


def test_pcb_svg_assembly_view_uses_geometer_hlr(tmp_path: Path) -> None:
    """Exercise pcb-svg assembly HLR against an embedded STEP model."""
    config_path = _write_pcb_svg_config(tmp_path, include_hlr=True)
    output_dir = tmp_path / "pcb-svg-hlr"

    result = _run_cli(
        "pcb-svg",
        str(_CORPUS_LED_PCB),
        "--config",
        str(config_path),
        "-o",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr + result.stdout
    svg = (output_dir / "assembly_top_view" / "led_component__assembly_top_view.svg").read_text(
        encoding="utf-8"
    )
    assert 'id="assembly-overlay"' in svg
    assert 'data-assembly-symbol="simple"' in svg
    assert 'data-projection="simple"' in svg
    assert "<line " in svg

"""Public workflow tests for the design command."""

from __future__ import annotations

import json
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

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


def _read_json(path: Path) -> dict[str, Any]:
    """Read a JSON object from a generated artifact."""
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(payload, dict)
    return payload


def _assert_design_review_bundle(
    output_dir: Path,
    *,
    design_json_name: str,
    expect_pcb_svgs: bool,
) -> dict[str, Any]:
    """Verify the design command emitted the shared review bundle layout."""
    manifest_path = output_dir / "design_review_manifest.json"
    readme_path = output_dir / "README.md"

    assert manifest_path.exists()
    assert readme_path.exists()
    manifest = _read_json(manifest_path)
    assert manifest["schema"] == "kicad_cruncher.design_review_manifest.a0"
    assert manifest["design_json"] == design_json_name
    assert manifest["readme"] == "README.md"
    assert (output_dir / design_json_name).exists()

    readme_text = readme_path.read_text(encoding="utf-8")
    assert "KiCad Design Review Bundle" in readme_text
    assert "Design JSON Relationships" in readme_text
    assert "Schematic SVGs" in readme_text
    assert "PCB Review SVGs" in readme_text
    assert "kicad_monkey.design.a0" in readme_text

    schematic_svgs = manifest["schematic_svgs"]
    assert isinstance(schematic_svgs, list)
    assert schematic_svgs
    for item in schematic_svgs:
        svg_path = output_dir / item["file"]
        assert svg_path.exists()
        schematic_svg = svg_path.read_text(encoding="utf-8")
        assert "<svg" in schematic_svg
        assert item["sheet_path"]
        assert item["sheet_instance_path"]

    pcb_svgs = manifest["pcb_svgs"]
    assert isinstance(pcb_svgs, list)
    if expect_pcb_svgs:
        assert pcb_svgs
    else:
        assert pcb_svgs == []
    return manifest


def _assert_pcb_review_svg_contract(output_dir: Path, item: dict[str, Any]) -> None:
    """Verify a generated PCB review SVG carries the visual review contract."""
    svg_path = output_dir / item["file"]
    assert svg_path.exists()
    svg_text = svg_path.read_text(encoding="utf-8")
    assert 'data-review-theme="kicad_cruncher.design_review.pcb_svg.a0"' in svg_text
    assert f'data-review-layer="{item["layer"]}"' in svg_text
    assert 'data-review-draw-order="tracks,polygons-zones,edge-cuts,pads,drills-slots"' in svg_text
    assert "#D0D0D0" in svg_text
    assert "#000000" in svg_text
    if int(item["drill_slot_overlay_count"]) > 0:
        assert 'id="design-review-drills-slots"' in svg_text
        assert "data-hole-kind=" in svg_text
        assert "data-hole-plating=" in svg_text


def _review_attrs_by_source_uuid(
    svg_text: str,
    *,
    review_object: str,
    component: str,
) -> dict[str, dict[str, str]]:
    """Index review overlay SVG attributes for a component by source UUID."""
    root = ET.fromstring(svg_text)
    attrs_by_uuid: dict[str, dict[str, str]] = {}
    for element in root.iter():
        attrs = element.attrib
        if attrs.get("data-review-object") != review_object:
            continue
        if attrs.get("data-component") != component:
            continue
        source_uuid = attrs.get("data-source-uuid")
        if source_uuid:
            attrs_by_uuid[source_uuid] = dict(attrs)
    return attrs_by_uuid


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
    _assert_design_review_bundle(
        output_dir,
        design_json_name="demo_design.json",
        expect_pcb_svgs=False,
    )


@pytest.mark.parametrize("command", ("design-review", "dr"))
def test_design_aliases_generate_review_bundle(tmp_path: Path, command: str) -> None:
    """Verify public aliases generate the same review bundle shape."""
    project_path = _write_synthetic_project(tmp_path)
    output_dir = tmp_path / command

    result = _run_cli(command, str(project_path), "-o", str(output_dir))

    assert result.returncode == 0, result.stderr + result.stdout
    _assert_design_review_bundle(
        output_dir,
        design_json_name="demo_design.json",
        expect_pcb_svgs=False,
    )


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
    payload = _read_json(output_dir / output_name)
    assert payload["schema"] == "kicad_monkey.design.a0"
    assert len(payload["components"]) == component_count
    assert len(payload["nets"]) == net_count
    assert "pnp" in payload
    manifest = _assert_design_review_bundle(
        output_dir,
        design_json_name=output_name,
        expect_pcb_svgs=True,
    )
    assert any(item["layer"] == "F.Cu" for item in manifest["pcb_svgs"])
    assert any(item["layer"] == "B.Cu" for item in manifest["pcb_svgs"])
    _assert_pcb_review_svg_contract(output_dir, manifest["pcb_svgs"][0])
    all_pcb_svg_text = "\n".join(
        (output_dir / item["file"]).read_text(encoding="utf-8")
        for item in manifest["pcb_svgs"]
    )
    if output_name != "led_component_design.json":
        assert "#B8B8B8" in all_pcb_svg_text


def test_design_review_pcb_overlay_distinguishes_pth_and_npth_pads(tmp_path: Path) -> None:
    """Verify design-review drill overlays keep real PTH and NPTH pads distinct."""
    project_path = (
        _CORPUS_ROOT
        / "projects"
        / "taillight"
        / "input"
        / "11-10045__taillight__C.kicad_pro"
    )
    output_dir = tmp_path / "out"

    result = _run_cli("dr", str(project_path), "-o", str(output_dir))

    assert result.returncode == 0, result.stderr + result.stdout
    manifest = _read_json(output_dir / "design_review_manifest.json")
    front_copper = next(item for item in manifest["pcb_svgs"] if item["layer"] == "F.Cu")
    svg_text = (output_dir / front_copper["file"]).read_text(encoding="utf-8")
    j1_pad_holes = _review_attrs_by_source_uuid(
        svg_text,
        review_object="pad-hole",
        component="J1",
    )

    npth_pad = j1_pad_holes["7f60e5a9-d550-4d97-99c7-c6445de4e457"]
    assert npth_pad["data-hole-plating"] == "non-plated"
    assert npth_pad["data-hole-kind"] == "round"
    assert npth_pad["fill"] == "#DC2626"
    assert "data-pad-number" not in npth_pad

    pth_pad = j1_pad_holes["5c2e78b7-48b3-4842-94d6-1a03bfcd6e8d"]
    assert pth_pad["data-hole-plating"] == "plated"
    assert pth_pad["data-hole-kind"] == "round"
    assert pth_pad["fill"] == "#2563EB"
    assert pth_pad["data-pad-number"] == "1"

    assert sum(attrs["data-hole-plating"] == "non-plated" for attrs in j1_pad_holes.values()) == 4
    assert sum(attrs["data-hole-plating"] == "plated" for attrs in j1_pad_holes.values()) == 4


def test_design_command_can_auto_detect_single_project(tmp_path: Path) -> None:
    """Verify design auto-detects one project in the working directory."""
    _write_synthetic_project(tmp_path)

    result = _run_cli("design", "--no-indexes", cwd=tmp_path)

    assert result.returncode == 0, result.stderr + result.stdout
    output_file = tmp_path / "output" / "design" / "demo_design.json"
    payload = _read_json(output_file)
    assert payload["schema"] == "kicad_monkey.design.a0"
    assert "indexes" not in payload
    _assert_design_review_bundle(
        tmp_path / "output" / "design",
        design_json_name="demo_design.json",
        expect_pcb_svgs=False,
    )


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

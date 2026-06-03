"""Public workflow tests for the design command."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path
from typing import Any

import pytest
from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
    _classify_edge_cut_regions,
    _interior_board_regions,
    _outer_board_region,
    render_pcb_svg_composition,
)
from kicad_cruncher.kicad_cruncher_pcb_svg_config import _PcbSvgConfig
from kicad_monkey import KiCadPcb
from kicad_monkey.kicad_pcb_bounds import compute_pcb_svg_bounding_box

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CORPUS_ROOT = _PROJECT_ROOT / "tests" / "corpus" / "kicad"
_SVG_COLOR_RE = re.compile(r"#[0-9A-Fa-f]{6}")
_CORPUS_LED_PCB = (
    _CORPUS_ROOT / "board_svg" / "input" / "led_component" / "led_component.kicad_pcb"
)
_CORPUS_CUTOUT_TEST_PCB = (
    _CORPUS_ROOT / "projects" / "cutout_test" / "cutout_test.kicad_pcb"
)
_CORPUS_PROJECT_CASES = (
    pytest.param(
        _CORPUS_ROOT / "board_svg" / "input" / "led_component" / "led_component.kicad_pro",
        "led_component_design.json",
        1,
        6,
        1,
        2,
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
        6,
        4,
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
        6,
        4,
        id="charge_indicator",
    ),
    pytest.param(
        _CORPUS_ROOT
        / "projects"
        / "yoshi_mainboard"
        / "input"
        / "11-10080__yoshi-mainboard__A.kicad_pro",
        "11-10080__yoshi-mainboard__A_design.json",
        38,
        58,
        1,
        6,
        id="yoshi_mainboard",
    ),
    pytest.param(
        _CORPUS_ROOT
        / "projects"
        / "speedy_processing_module"
        / "input"
        / "11-10084__speedy_processing_module__B.kicad_pro",
        "11-10084__speedy_processing_module__B_design.json",
        534,
        500,
        17,
        10,
        id="speedy_processing_module",
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

_SYNTHETIC_CUTOUT_PCB_TEXT = """(kicad_pcb
  (version 20241229)
  (generator "pcbnew")
  (generator_version "9.0")
  (general (thickness 1.6) (legacy_teardrops no))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (25 "Edge.Cuts" user)
  )
  (gr_rect
    (start 0 0)
    (end 60 40)
    (stroke (width 0.1) (type solid))
    (fill no)
    (layer "Edge.Cuts")
    (uuid "00000000-0000-4000-8000-000000000001")
  )
  (gr_line
    (start 15 14)
    (end 25 14)
    (stroke (width 0.1) (type solid))
    (layer "Edge.Cuts")
    (uuid "00000000-0000-4000-8000-000000000002")
  )
  (gr_arc
    (start 25 14)
    (mid 29 18)
    (end 25 22)
    (stroke (width 0.1) (type solid))
    (layer "Edge.Cuts")
    (uuid "00000000-0000-4000-8000-000000000003")
  )
  (gr_line
    (start 25 22)
    (end 15 22)
    (stroke (width 0.1) (type solid))
    (layer "Edge.Cuts")
    (uuid "00000000-0000-4000-8000-000000000004")
  )
  (gr_arc
    (start 15 22)
    (mid 11 18)
    (end 15 14)
    (stroke (width 0.1) (type solid))
    (layer "Edge.Cuts")
    (uuid "00000000-0000-4000-8000-000000000005")
  )
  (gr_circle
    (center 45 26)
    (end 48 26)
    (stroke (width 0.1) (type solid))
    (fill no)
    (layer "Edge.Cuts")
    (uuid "00000000-0000-4000-8000-000000000006")
  )
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


def _write_synthetic_cutout_pcb(root: Path) -> Path:
    """Write a minimal PCB with generic internal Edge.Cuts regions."""
    pcb_path = root / "cutout_regions.kicad_pcb"
    pcb_path.write_text(_SYNTHETIC_CUTOUT_PCB_TEXT, encoding="utf-8")
    return pcb_path


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


def _write_pcb_svg_virtual_config(root: Path) -> Path:
    """Write a focused config for virtual layer composition tests."""
    config_path = root / "pcb.svg.config"
    config_path.write_text(
        json.dumps(
            {
                "schema": "pcb.svg.config.a0",
                "global": {"include_metadata": True, "show_empty_layers": True},
                "layer_outputs": {"enabled": False},
                "views": [
                    {
                        "name": "board_cutouts",
                        "enabled": True,
                        "group_id": "pcb-svg-view-board-cutouts",
                        "output_svg": "views/{board}__board_cutouts.svg",
                        "layers": ["BOARD_OUTLINE", "BOARD_CUTOUTS"],
                        "assembly_hlr_mode": "none",
                    },
                    {
                        "name": "top_pin1_view",
                        "enabled": True,
                        "group_id": "pcb-svg-view-top-pin1",
                        "output_svg": "views/{board}__top_pin1_view.svg",
                        "layers": ["BOARD_OUTLINE", "F.Cu", "DRILLS", "SLOTS", "PIN1_TOP"],
                        "assembly_hlr_mode": "none",
                    },
                ],
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
        assert "kicad_monkey.schematic.svg.enrichment.a0" in schematic_svg
        assert 'data-review-theme="kicad_cruncher.design_review.schematic_svg.a0"' in schematic_svg
        colors = {match.upper() for match in _SVG_COLOR_RE.findall(schematic_svg)}
        assert colors <= {"#000000", "#FFFFFF"}
        assert "#000000" in colors
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
    assert "#000000" in svg_text
    root = ET.fromstring(svg_text)
    has_trace_or_zone = any(
        element.attrib.get("data-ref") in {"segment", "track_arc", "via", "zone_fill"}
        or element.attrib.get("data-primitive") in {"track", "via", "zone"}
        for element in root.iter()
    )
    if has_trace_or_zone:
        assert "#B8B8B8" in svg_text
    edge_cut_subtrees = [
        ET.tostring(element, encoding="unicode")
        for element in root.iter()
        if element.attrib.get("data-layer-name") == "Edge.Cuts"
        or "Edge.Cuts" in element.attrib.get("data-layer-names", "")
    ]
    assert edge_cut_subtrees
    assert any("#000000" in subtree for subtree in edge_cut_subtrees)
    assert all("#D0D0D0" not in subtree for subtree in edge_cut_subtrees)
    assert 'id="design-review-drills-slots"' not in svg_text
    assert "data-review-object=" not in svg_text
    assert "data-source-uuid=" not in svg_text
    assert "data-hole-plated=" not in svg_text
    assert 'data-hole-plating="non-plated"' not in svg_text
    if int(item["drill_slot_record_count"]) > 0:
        assert 'data-primitive="pad-hole"' in svg_text or 'data-primitive="via-hole"' in svg_text
        assert "data-hole-kind=" in svg_text
        assert "data-hole-plating=" in svg_text
        assert any(color in svg_text for color in ("#2563EB", "#0891B2", "#DC2626", "#F97316"))


def _review_attrs_by_source_uuid(
    svg_text: str,
    *,
    component: str,
) -> dict[str, dict[str, str]]:
    """Index KiCad Monkey enriched hole attributes for a component by owner UUID."""
    root = ET.fromstring(svg_text)
    attrs_by_uuid: dict[str, dict[str, str]] = {}
    for element in root.iter():
        attrs = element.attrib
        if attrs.get("data-primitive") not in {"pad-hole", "via-hole"}:
            continue
        if attrs.get("data-component") != component:
            continue
        source_uuid = attrs.get("data-hole-owner")
        if source_uuid:
            attrs_by_uuid[source_uuid] = dict(attrs)
    return attrs_by_uuid


def _review_svgs_by_source_uuid(
    svg_text: str,
    *,
    component: str,
) -> dict[str, str]:
    """Index KiCad Monkey enriched hole SVG subtrees for a component by owner UUID."""
    root = ET.fromstring(svg_text)
    svg_by_uuid: dict[str, str] = {}
    for element in root.iter():
        attrs = element.attrib
        if attrs.get("data-primitive") not in {"pad-hole", "via-hole"}:
            continue
        if attrs.get("data-component") != component:
            continue
        source_uuid = attrs.get("data-hole-owner")
        if source_uuid:
            svg_by_uuid[source_uuid] = ET.tostring(element, encoding="unicode")
    return svg_by_uuid


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
    (
        "project_path",
        "output_name",
        "component_count",
        "net_count",
        "schematic_svg_count",
        "pcb_svg_count",
    ),
    _CORPUS_PROJECT_CASES,
)
def test_design_command_uses_copied_kicad_monkey_corpus_projects(
    tmp_path: Path,
    project_path: Path,
    output_name: str,
    component_count: int,
    net_count: int,
    schematic_svg_count: int,
    pcb_svg_count: int,
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
    assert len(manifest["schematic_svgs"]) == schematic_svg_count
    assert len(manifest["pcb_svgs"]) == pcb_svg_count
    assert any(item["layer"] == "F.Cu" for item in manifest["pcb_svgs"])
    assert any(item["layer"] == "B.Cu" for item in manifest["pcb_svgs"])
    _assert_pcb_review_svg_contract(output_dir, manifest["pcb_svgs"][0])
    all_pcb_svg_text = "\n".join(
        (output_dir / item["file"]).read_text(encoding="utf-8")
        for item in manifest["pcb_svgs"]
    )
    if output_name != "led_component_design.json":
        assert "#B8B8B8" in all_pcb_svg_text


def test_design_review_pcb_records_distinguish_pth_and_npth_pads(tmp_path: Path) -> None:
    """Verify design-review drill records keep real PTH and NPTH pads distinct."""
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
        component="J1",
    )
    j1_pad_hole_svgs = _review_svgs_by_source_uuid(
        svg_text,
        component="J1",
    )

    npth_pad = j1_pad_holes["7f60e5a9-d550-4d97-99c7-c6445de4e457"]
    assert npth_pad["data-hole-plating"] == "non_plated"
    assert npth_pad["data-hole-kind"] == "round"
    assert npth_pad["data-pad-type"] == "np_thru_hole"
    assert npth_pad["data-hole-diameter-mm"] == "2.5"
    assert "data-pad-number" not in npth_pad
    assert "#DC2626" in j1_pad_hole_svgs["7f60e5a9-d550-4d97-99c7-c6445de4e457"]

    pth_pad = j1_pad_holes["5c2e78b7-48b3-4842-94d6-1a03bfcd6e8d"]
    assert pth_pad["data-hole-plating"] == "plated"
    assert pth_pad["data-hole-kind"] == "round"
    assert pth_pad["data-pad-type"] == "thru_hole"
    assert pth_pad["data-hole-diameter-mm"] == "0.889"
    assert pth_pad["data-pad-number"] == "1"
    assert "#2563EB" in j1_pad_hole_svgs["5c2e78b7-48b3-4842-94d6-1a03bfcd6e8d"]

    assert sum(attrs["data-hole-plating"] == "non_plated" for attrs in j1_pad_holes.values()) == 4
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
    assert manifest["layer_outputs"]["F.Cu"]["layers"] == [
        "F.Cu",
        "Edge.Cuts",
        "DRILLS",
        "SLOTS",
    ]
    assert manifest["layer_outputs"]["F.Cu"]["context_layers"] == [
        "Edge.Cuts",
        "DRILLS",
        "SLOTS",
    ]
    assert manifest["layer_outputs"]["Edge.Cuts"]["layers"] == ["Edge.Cuts"]
    assert manifest["layer_outputs"]["Edge.Cuts"]["context_layers"] == []
    assert manifest["layer_outputs"]["BOARD_OUTLINE"]["virtual"] is True
    assert manifest["layer_outputs"]["BOARD_OUTLINE"]["layers"] == ["BOARD_OUTLINE"]
    assert (output_dir / "layers" / "led_component__F.Cu.svg").exists()
    assert (output_dir / "layers" / "led_component__B.Cu.svg").exists()
    assert (output_dir / "layers" / "led_component__Edge.Cuts.svg").exists()
    assert (output_dir / "layers" / "led_component__virtual__board_outline.svg").exists()
    front_layer_svg = (output_dir / "layers" / "led_component__F.Cu.svg").read_text(
        encoding="utf-8"
    )
    edge_cuts_svg = (output_dir / "layers" / "led_component__Edge.Cuts.svg").read_text(
        encoding="utf-8"
    )
    assert 'data-layer-name="Edge.Cuts"' in front_layer_svg
    assert 'data-layer-token="BOARD_OUTLINE"' not in front_layer_svg
    assert 'data-layer-token="BOARD_OUTLINE"' not in edge_cuts_svg
    assert 'data-layer-token="DRILLS"' not in edge_cuts_svg
    assert 'data-layer-token="SLOTS"' not in edge_cuts_svg
    assert (output_dir / "top_view" / "led_component__top_view.svg").exists()


def test_pcb_svg_layer_context_and_virtual_outputs_can_be_disabled(
    tmp_path: Path,
) -> None:
    """Verify physical outputs can stay raw while standalone virtual files are disabled."""
    config_path = _write_pcb_svg_config(tmp_path, include_hlr=False)
    config_payload = _read_json(config_path)
    config_payload["layer_outputs"]["add_edge_cuts_to_physical_layers"] = False
    config_payload["layer_outputs"]["add_drills_to_physical_layers"] = False
    config_payload["layer_outputs"]["add_slots_to_physical_layers"] = False
    config_payload["layer_outputs"]["write_virtual_layers"] = False
    config_path.write_text(json.dumps(config_payload, indent=2), encoding="utf-8")
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
    manifest = _read_json(output_dir / "led_component__views.json")
    assert manifest["layer_outputs"]["F.Cu"]["layers"] == ["F.Cu"]
    assert manifest["layer_outputs"]["F.Cu"]["context_layers"] == []
    assert "BOARD_OUTLINE" not in manifest["layer_outputs"]
    assert "DRILLS" not in manifest["layer_outputs"]
    assert "SLOTS" not in manifest["layer_outputs"]
    assert not (output_dir / "layers" / "led_component__virtual__board_outline.svg").exists()
    front_layer_svg = (output_dir / "layers" / "led_component__F.Cu.svg").read_text(
        encoding="utf-8"
    )
    root = ET.fromstring(front_layer_svg)
    assert not any(
        element.attrib.get("data-layer-name") == "Edge.Cuts"
        for element in root.iter()
    )
    assert 'data-layer-token="DRILLS"' not in front_layer_svg
    assert 'data-layer-token="SLOTS"' not in front_layer_svg


def test_pcb_svg_default_config_exposes_altium_style_virtual_views() -> None:
    """Verify the default A0 config includes the expected virtual layer views."""
    config = _PcbSvgConfig.default()
    views = {view.name: view for view in config.views}

    assert config.layer_outputs["layers"] == "auto"
    assert config.layer_outputs["add_edge_cuts_to_physical_layers"] is True
    assert config.layer_outputs["add_drills_to_physical_layers"] is True
    assert config.layer_outputs["add_slots_to_physical_layers"] is True
    assert config.layer_outputs["write_virtual_layers"] is True
    assert config.layer_outputs["include_special_layers"] == [
        "BOARD_OUTLINE",
        "BOARD_CUTOUTS",
        "DRILLS",
        "SLOTS",
    ]
    assert views["board_cutouts"].layers == ["BOARD_OUTLINE", "BOARD_CUTOUTS"]
    assert views["top_pin1_view"].layers == [
        "BOARD_OUTLINE",
        "TOP",
        "DRILLS",
        "SLOTS",
        "PIN1_TOP",
        "ASSEMBLY_HLR_TOP",
    ]
    assert views["bottom_pin1_view"].layers == [
        "BOARD_OUTLINE",
        "BOTTOM",
        "DRILLS",
        "SLOTS",
        "PIN1_BOTTOM",
        "ASSEMBLY_HLR_BOTTOM",
    ]
    assert "top_hlr_bounding_boxes" in views
    assert "bottom_hlr_bounding_boxes" in views


def test_pcb_svg_virtual_layers_use_full_board_canvas_origin() -> None:
    """Verify virtual overlays align with KiCad Monkey's all-layer SVG canvas."""
    pcb = KiCadPcb.from_file(
        _CORPUS_ROOT
        / "projects"
        / "taillight"
        / "input"
        / "11-10045__taillight__C.kicad_pcb"
    )
    config = _PcbSvgConfig.default()
    composition = render_pcb_svg_composition(
        pcb,
        ["F.Cu", "BOARD_OUTLINE"],
        styles=config.global_options.styles,
        group_id="pcb-svg-test-origin",
        config=config,
    )
    root = ET.fromstring(composition.svg_text)
    outline_group = next(
        element
        for element in root.iter()
        if element.attrib.get("id") == "pcb-svg-board-outline"
    )
    outline_path = next(
        element
        for element in outline_group.iter()
        if element.tag.endswith("path")
    )
    match = re.match(r"M ([\d.-]+) ([\d.-]+)", outline_path.attrib["d"])
    assert match is not None
    first_svg_point = (float(match.group(1)), float(match.group(2)))

    outer_region = _outer_board_region(_classify_edge_cut_regions(pcb))
    assert outer_region is not None
    first_board_point = outer_region.points[0]
    full_bbox = compute_pcb_svg_bounding_box(pcb, None)
    layer_bbox = compute_pcb_svg_bounding_box(pcb, ["F.Cu", "Edge.Cuts"])
    expected = (
        first_board_point[0] - full_bbox.min_x,
        first_board_point[1] - full_bbox.min_y,
    )
    layer_specific_origin = (
        first_board_point[0] - layer_bbox.min_x,
        first_board_point[1] - layer_bbox.min_y,
    )

    assert first_svg_point == pytest.approx(expected, abs=0.0001)
    assert abs(first_svg_point[0] - layer_specific_origin[0]) > 1.0
    assert abs(first_svg_point[1] - layer_specific_origin[1]) > 1.0


def test_pcb_svg_yoshi_board_outline_loop_survives_arc_float_noise() -> None:
    """Verify yoshi's arc/line Edge.Cuts profile is not mistaken for a cutout."""
    pcb = KiCadPcb.from_file(
        _CORPUS_ROOT
        / "projects"
        / "yoshi_mainboard"
        / "input"
        / "11-10080__yoshi-mainboard__A.kicad_pcb"
    )
    regions = _classify_edge_cut_regions(pcb)
    outer_region = _outer_board_region(regions)
    cutouts = _interior_board_regions(regions)

    assert outer_region is not None
    assert outer_region.source_kind == "gr_arc+gr_line"
    assert outer_region.area == pytest.approx(361.879, abs=0.001)
    assert len(cutouts) == 5
    assert {region.source_kind for region in cutouts} == {"gr_circle"}


def test_pcb_svg_board_cutouts_detect_generic_internal_closed_regions(tmp_path: Path) -> None:
    """Verify BOARD_CUTOUTS is synthesized from any internal closed Edge.Cuts region."""
    pcb_path = _write_synthetic_cutout_pcb(tmp_path)
    config_path = _write_pcb_svg_virtual_config(tmp_path)
    output_dir = tmp_path / "pcb-svg"

    result = _run_cli(
        "pcb-svg",
        str(pcb_path),
        "--config",
        str(config_path),
        "--views",
        "cutouts",
        "-o",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr + result.stdout
    svg = (output_dir / "views" / "cutout_regions__board_cutouts.svg").read_text(
        encoding="utf-8"
    )
    assert 'id="board-cutout-hatch"' in svg
    assert 'data-layer-token="BOARD_OUTLINE"' in svg
    assert 'data-layer-token="BOARD_CUTOUTS"' in svg
    assert 'data-cutout-count="2"' in svg
    assert 'data-source-kinds="gr_arc+gr_line"' in svg
    assert 'data-source-kinds="gr_circle"' in svg
    cutout_group = svg.split('id="pcb-svg-board-cutouts"', 1)[1]
    assert 'data-source-kinds="gr_rect"' not in cutout_group


def test_pcb_svg_cutout_project_detects_generalized_edge_cut_regions(
    tmp_path: Path,
) -> None:
    """Verify the cutout signoff fixture detects closed Edge.Cuts primitives only."""
    config_path = _write_pcb_svg_virtual_config(tmp_path)
    output_dir = tmp_path / "pcb-svg"

    result = _run_cli(
        "pcb-svg",
        str(_CORPUS_CUTOUT_TEST_PCB),
        "--config",
        str(config_path),
        "--views",
        "cutouts",
        "-o",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr + result.stdout
    svg = (output_dir / "views" / "cutout_test__board_cutouts.svg").read_text(
        encoding="utf-8"
    )
    assert 'id="board-cutout-hatch"' in svg
    assert 'data-layer-token="BOARD_OUTLINE"' in svg
    assert 'data-layer-token="BOARD_CUTOUTS"' in svg
    assert 'data-cutout-count="8"' in svg
    root = ET.fromstring(svg)
    cutout_elements = [
        element
        for element in root.iter()
        if element.attrib.get("data-feature") == "board-cutout"
    ]
    assert len(cutout_elements) == 8
    assert Counter(
        element.attrib.get("data-source-kinds") for element in cutout_elements
    ) == Counter(
        {
            "gr_rect": 2,
            "gr_arc+gr_line": 2,
            "gr_circle": 1,
            "gr_curve": 1,
            "gr_line": 1,
            "gr_poly": 1,
        }
    )
    source_uuids = ",".join(
        element.attrib.get("data-source-uuids", "") for element in cutout_elements
    )
    assert "2f5a58a7-37b3-4cdf-a529-a917340a8c17" not in source_uuids
    assert "1b50bc9f-2d6e-41c9-a172-e3815c9c4059" not in source_uuids


def test_pcb_svg_pin1_view_uses_virtual_markers_and_enriched_drill_metadata(
    tmp_path: Path,
) -> None:
    """Verify pin-1 and drill virtual layers compose with KiCad Monkey enrichment."""
    config_path = _write_pcb_svg_virtual_config(tmp_path)
    output_dir = tmp_path / "pcb-svg"
    project_path = (
        _CORPUS_ROOT
        / "projects"
        / "taillight"
        / "input"
        / "11-10045__taillight__C.kicad_pro"
    )

    result = _run_cli(
        "pcb-svg",
        str(project_path),
        "--config",
        str(config_path),
        "--views",
        "top-pin1",
        "-o",
        str(output_dir),
    )

    assert result.returncode == 0, result.stderr + result.stdout
    svg = (
        output_dir / "views" / "11-10045__taillight__C__top_pin1_view.svg"
    ).read_text(encoding="utf-8")
    assert 'id="pcb-svg-board-outline"' in svg
    assert 'id="pcb-svg-drills"' in svg
    assert 'data-layer-token="PIN1_TOP"' in svg
    assert 'data-primitive="pin1-marker"' in svg
    assert 'data-component="J1"' in svg
    assert 'data-pad-number="1"' in svg
    assert 'data-hole-plating="plated"' in svg
    assert 'data-hole-plating="non_plated"' in svg
    assert "#90EE90" in svg
    assert "#ADD8E6" in svg


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

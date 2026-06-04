"""Public workflow tests for the pcb-layer-step command."""

from __future__ import annotations

import argparse
import json
import sys
import types
from pathlib import Path
from typing import Any

import pytest
from kicad_cruncher.kicad_cruncher_cmd_pcb_layer_step import cmd_pcb_layer_step
from kicad_cruncher.kicad_cruncher_pcb_layer_step import write_default_pcb_layer_step_config
from kicad_cruncher.kicad_cruncher_pcb_layer_step_config import resolve_pcb_layer_selector

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CORPUS_ROOT = _PROJECT_ROOT / "tests" / "corpus" / "kicad"

_LAYER_STEP_PROJECT_CASES = (
    pytest.param(
        _CORPUS_ROOT
        / "projects"
        / "yoshi_mainboard"
        / "input"
        / "11-10080__yoshi-mainboard__A.kicad_pro",
        "11-10080__yoshi-mainboard__A",
        8,
        id="yoshi_mainboard",
    ),
    pytest.param(
        _CORPUS_ROOT / "projects" / "taillight" / "input" / "11-10045__taillight__C.kicad_pro",
        "11-10045__taillight__C",
        4,
        id="taillight",
    ),
)


def _args(project_path: Path, config_path: Path, output_dir: Path) -> argparse.Namespace:
    return argparse.Namespace(
        file=str(project_path),
        output=output_dir,
        config=config_path,
        init_config=False,
        force_config=False,
        pcbdoc=None,
        layer=None,
        thickness_mm=None,
        z_mm=None,
        copper_color=None,
        outline_width_mm=None,
        outline_color=None,
        board_cutout_color=None,
        exclude_poured_polygons=False,
        outline_only=False,
        no_board_outline=False,
        no_board_cutouts=False,
        no_hole_cuts=False,
        drill_hole_mode=None,
        max_boolean_drill_cuts=None,
        drill_hole_color=None,
        drill_plated_hole_color=None,
        drill_non_plated_hole_color=None,
        drill_overlay_thickness_mm=None,
        drill_minimum_diameter_mm=None,
        drill_hole_shape=None,
        drill_ring_width_mm=None,
        drill_plated_ring_shape=None,
        no_fuse=False,
        arc_segments=None,
    )


def _install_fake_geometer(monkeypatch: pytest.MonkeyPatch) -> list[dict[str, Any]]:
    requests: list[dict[str, Any]] = []

    def write_planar_step(request: dict[str, Any], output_path: Path) -> None:
        requests.append(request)
        Path(output_path).write_text("ISO-10303-21;\nEND-ISO-10303-21;\n", encoding="utf-8")

    fake = types.SimpleNamespace(write_planar_step=write_planar_step)
    monkeypatch.setitem(sys.modules, "geometer", fake)
    return requests


def _points_size(points: list[Any]) -> tuple[float, float]:
    xs = [float(point[0]) for point in points]
    ys = [float(point[1]) for point in points]
    return (max(xs) - min(xs), max(ys) - min(ys))


def _arc_center_delta(ring: dict[str, Any]) -> tuple[float, float]:
    centers = [
        segment["center"]
        for segment in ring.get("segments", [])
        if isinstance(segment, dict) and isinstance(segment.get("center"), list)
    ]
    if len(centers) < 2:
        return (0.0, 0.0)
    return (
        abs(float(centers[1][0]) - float(centers[0][0])),
        abs(float(centers[1][1]) - float(centers[0][1])),
    )


@pytest.mark.parametrize(
    ("project_path", "board_key", "test_point_count"), _LAYER_STEP_PROJECT_CASES
)
def test_pcb_layer_step_default_fixture_alignment_outputs_for_public_boards(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    project_path: Path,
    board_key: str,
    test_point_count: int,
) -> None:
    """Verify default fixture-alignment STEP output on yoshi and taillight boards."""
    requests = _install_fake_geometer(monkeypatch)
    config_path = tmp_path / "pcb-layer-step.json"
    output_dir = tmp_path / "out"
    write_default_pcb_layer_step_config(config_path)

    rc = cmd_pcb_layer_step(_args(project_path, config_path, output_dir))

    assert rc == 0
    assert len(requests) == 1
    request = requests[0]
    assert request["schema"] == "geometry.planar_step.request.a0"
    body_by_id = {body["id"]: body for body in request["bodies"]}
    assert body_by_id["test_points"]["color"] == "#FF0000"
    assert len(body_by_id["test_points"]["regions"]) == test_point_count
    assert body_by_id["test_points"].get("cutouts")
    assert "board_outline" in body_by_id
    assert body_by_id["board_outline"]["color"] == "#111111"
    if board_key == "11-10045__taillight__C":
        plated_regions = body_by_id["plated_drill_holes"]["regions"]
        j1_pad_ring = plated_regions[0]["outer"]["points"]
        width, height = _points_size(j1_pad_ring)
        assert width == pytest.approx(1.651)
        assert height == pytest.approx(3.175)

    step_path = output_dir / f"{board_key}__fixture_alignment.step"
    manifest_path = output_dir / f"{board_key}__fixture_alignment.json"
    assert step_path.exists()
    assert manifest_path.exists()
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["schema"] == "wn.kicad_cruncher.pcb_layer_step.v1"
    assert manifest["backend"] == "geometer.planar_step"
    assert manifest["board"] == board_key
    assert manifest["layer"]["json_name"] == "B.Cu"
    assert manifest["coordinate_origin"]["mode"] == "kicad_aux_axis_origin"
    assert manifest["counts"]["source_layer_geometries"] == test_point_count
    assert manifest["counts"]["copper_bodies"] == 1
    assert manifest["counts"]["outline_bodies"] == 1
    assert manifest["counts"]["drill_overlay_geometries"] >= 1


def test_pcb_layer_step_taillight_omits_placement_rule_zones_from_copper(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify KiCad placement/keepout zones are not treated as copper pours."""
    requests = _install_fake_geometer(monkeypatch)
    config_path = tmp_path / "pcb-layer-step-polygons.json"
    output_dir = tmp_path / "out"
    config_path.write_text(
        json.dumps(
            {
                "schema": "wn.kicad_cruncher.pcb_layer_step.config.v2",
                "defaults": {
                    "layer": "bottom",
                    "include_board_outline": True,
                    "include_board_cutouts": False,
                },
                "outputs": [
                    {
                        "name": "polygon_review",
                        "output_step": "{board}__polygon_review.step",
                        "features": {
                            "component_pads": False,
                            "free_pads": False,
                            "tracks": False,
                            "arcs": False,
                            "polygons": {"enabled": True, "body": "polygons"},
                            "regions": False,
                            "vias": False,
                        },
                        "drills": {"mode": "none"},
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    rc = cmd_pcb_layer_step(
        _args(
            _CORPUS_ROOT / "projects" / "taillight" / "input" / "11-10045__taillight__C.kicad_pro",
            config_path,
            output_dir,
        )
    )

    assert rc == 0
    assert len(requests) == 1
    body_by_id = {body["id"]: body for body in requests[0]["bodies"]}
    polygon_body = body_by_id.get("polygons", {"regions": []})
    placement_rule_sized_regions = [
        region
        for region in polygon_body["regions"]
        if _points_size(region["outer"]["points"])
        == pytest.approx((15.65, 13.55), abs=0.001)
    ]
    assert placement_rule_sized_regions == []


def test_pcb_layer_step_yoshi_usb_slot_holes_preserve_orientation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify USB shield oval drill holes are horizontal for the Yoshi J1 connector."""
    requests = _install_fake_geometer(monkeypatch)
    config_path = tmp_path / "pcb-layer-step-drills.json"
    output_dir = tmp_path / "out"
    config_path.write_text(
        json.dumps(
            {
                "schema": "wn.kicad_cruncher.pcb_layer_step.config.v2",
                "defaults": {
                    "layer": "bottom",
                    "include_copper": False,
                    "include_board_outline": False,
                    "include_board_cutouts": False,
                },
                "outputs": [
                    {
                        "name": "drill_review",
                        "output_step": "{board}__drill_review.step",
                        "features": {
                            "component_pads": False,
                            "free_pads": False,
                            "tracks": False,
                            "arcs": False,
                            "polygons": False,
                            "regions": False,
                            "vias": False,
                        },
                        "drills": {
                            "mode": "overlay",
                            "minimum_diameter_mm": 0.0,
                            "shape": "ring",
                            "color": "#666666",
                            "plated_color": "#666666",
                            "non_plated_color": "#00AEEF",
                            "ring_width_mm": 0.12,
                            "plated_ring_shape": "pad",
                            "overlay_thickness_mm": 0.001,
                        },
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    rc = cmd_pcb_layer_step(
        _args(
            _CORPUS_ROOT
            / "projects"
            / "yoshi_mainboard"
            / "input"
            / "11-10080__yoshi-mainboard__A.kicad_pro",
            config_path,
            output_dir,
        )
    )

    assert rc == 0
    assert len(requests) == 1
    body_by_id = {body["id"]: body for body in requests[0]["bodies"]}
    plated_regions = body_by_id["plated_drill_holes"]["regions"]
    horizontal_usb_slots = []
    for region in plated_regions:
        outer_center_dx, outer_center_dy = _arc_center_delta(region["outer"])
        if (outer_center_dx, outer_center_dy) != pytest.approx((0.5, 0.0), abs=0.001):
            continue
        for hole in region.get("holes", []):
            hole_center_dx, hole_center_dy = _arc_center_delta(hole)
            if (hole_center_dx, hole_center_dy) == pytest.approx((0.5, 0.0), abs=0.001):
                horizontal_usb_slots.append(region)

    assert len(horizontal_usb_slots) >= 4


def test_pcb_layer_step_all_copper_fuses_and_clips_trace_body(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify all-copper trace bodies request Geometer fusion with pad/via clipping."""
    requests = _install_fake_geometer(monkeypatch)
    config_path = tmp_path / "pcb-layer-step-all-copper.json"
    output_dir = tmp_path / "out"
    copper_color = "#B87333"
    config_path.write_text(
        json.dumps(
            {
                "schema": "wn.kicad_cruncher.pcb_layer_step.config.v2",
                "defaults": {
                    "layer": "bottom",
                    "include_board_outline": False,
                    "include_board_cutouts": False,
                },
                "outputs": [
                    {
                        "name": "all_copper",
                        "output_step": "{board}__all_copper.step",
                        "features": {
                            "component_pads": {"mode": "all"},
                            "free_pads": True,
                            "tracks": {
                                "enabled": True,
                                "color": copper_color,
                                "body": "tracks",
                            },
                            "arcs": True,
                            "polygons": False,
                            "regions": False,
                            "vias": True,
                        },
                        "colors": {"default_copper": copper_color},
                        "drills": {"mode": "none"},
                        "fuse_copper": True,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    rc = cmd_pcb_layer_step(
        _args(
            _CORPUS_ROOT
            / "projects"
            / "yoshi_mainboard"
            / "input"
            / "11-10080__yoshi-mainboard__A.kicad_pro",
            config_path,
            output_dir,
        )
    )

    assert rc == 0
    assert len(requests) == 1
    body_by_id = {body["id"]: body for body in requests[0]["bodies"]}
    assert body_by_id["tracks"]["color"] == copper_color
    assert body_by_id["copper"]["color"] == copper_color
    assert body_by_id["tracks"]["fuse_regions"] is True
    assert body_by_id["copper"]["fuse_regions"] is True
    assert len(body_by_id["tracks"]["cutouts"]) > len(body_by_id["copper"]["cutouts"])


def test_pcb_layer_step_layer_selector_accepts_common_aliases() -> None:
    """Verify config-compatible layer selectors resolve to KiCad layer tokens."""
    assert resolve_pcb_layer_selector("bottom") == "B.Cu"
    assert resolve_pcb_layer_selector("BOTTOM") == "B.Cu"
    assert resolve_pcb_layer_selector(31) == "B.Cu"
    assert resolve_pcb_layer_selector("top") == "F.Cu"
    assert resolve_pcb_layer_selector(0) == "F.Cu"

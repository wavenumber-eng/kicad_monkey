"""Independent acceptance for typed Rust zone and fill-cache authoring."""

from __future__ import annotations

import math
import os
from pathlib import Path
import shutil
import subprocess

import pytest

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadPcb


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def _arc_semantics(arc) -> tuple[str, str]:
    start = (arc.start_x, arc.start_y)
    mid = (arc.mid_x, arc.mid_y)
    end = (arc.end_x, arc.end_y)
    denominator = 2.0 * (
        start[0] * (mid[1] - end[1])
        + mid[0] * (end[1] - start[1])
        + end[0] * (start[1] - mid[1])
    )

    def squared(point: tuple[float, float]) -> float:
        return point[0] * point[0] + point[1] * point[1]

    center_x = (
        squared(start) * (mid[1] - end[1])
        + squared(mid) * (end[1] - start[1])
        + squared(end) * (start[1] - mid[1])
    ) / denominator
    center_y = (
        squared(start) * (end[0] - mid[0])
        + squared(mid) * (start[0] - end[0])
        + squared(end) * (mid[0] - start[0])
    ) / denominator

    def angle(point: tuple[float, float]) -> float:
        return math.atan2(point[1] - center_y, point[0] - center_x)

    start_angle = angle(start)
    clockwise_sweep = (angle(end) - start_angle) % math.tau
    clockwise_mid = (angle(mid) - start_angle) % math.tau
    if clockwise_mid <= clockwise_sweep + 1e-9:
        direction = "cw"
        sweep = clockwise_sweep
    else:
        direction = "ccw"
        sweep = math.tau - clockwise_sweep
    return direction, "major" if sweep > math.pi else "minor"


@pytest.fixture(scope="module")
def authored_board(tmp_path_factory: pytest.TempPathFactory) -> Path:
    output = tmp_path_factory.mktemp("rust-zone-authoring")
    environment = os.environ.copy()
    environment["KM_ZONE_OUTPUT_DIR"] = str(output)
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "kicad-monkey-core",
            "--test",
            "pcb_authoring_zone_slice",
        ],
        cwd=PACKAGE_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    board = output / "native-zone.kicad_pcb"
    assert board.is_file()
    return board


def assert_zone_semantics(board_path: Path, *, upgraded: bool) -> None:
    board = KiCadPcb.from_file(board_path)
    assert len(board.zones) == 1
    zone = board.zones[0]
    expected_zone_ordinal = None if upgraded else 1
    assert (zone.net.ordinal, zone.net.name) == (expected_zone_ordinal, "GND")
    assert zone.layers == ["F.Cu", "In1.Cu"]
    assert zone.layers_plural is True
    assert zone.locked is True
    assert zone.uuid == "00000000-0000-0000-0000-000000000064"
    assert zone.name == "GROUND POUR"
    assert (zone.hatch_style, zone.hatch_pitch) == ("edge", 0.5)
    assert zone.priority == 3
    assert zone.connect_pads_clearance == 0.2
    assert zone.min_thickness == 0.25
    assert zone.filled_areas_thickness is False
    assert zone.fill_enabled is True
    assert (zone.thermal_gap, zone.thermal_bridge_width) == (0.3, 0.4)
    assert zone.island_removal_mode == 2
    assert zone.island_area_min == 1.0
    assert len(zone.polygons) == 1
    assert [tuple(point) for point in zone.polygons[0].points] == [
        (1.0, 1.0),
        (29.0, 1.0),
        (29.0, 29.0),
        (1.0, 29.0),
    ]
    authored_cache = [
        (chain.layer, chain.island, [tuple(point) for point in chain.points])
        for chain in zone.filled_polygons
    ]
    expected_cache = [
        (
            "F.Cu",
            False,
            [
                (29.0, 29.0),
                (1.0, 29.0),
                (1.0, 10.0),
                (10.0, 10.0),
                (10.0, 20.0),
                (20.0, 20.0),
                (20.0, 10.0),
                (10.0, 10.0),
                (1.0, 10.0),
                (1.0, 1.0),
                (29.0, 1.0),
            ],
        ),
        ("F.Cu", True, [(3.0, 3.0), (4.0, 3.0), (3.5, 4.0)]),
        (
            "In1.Cu",
            False,
            [(2.0, 2.0), (28.0, 2.0), (28.0, 28.0), (2.0, 28.0)],
        ),
    ]
    if upgraded:
        # KiCad owns fill realization and recomputes retained cache chains while
        # upgrading; exact authored-cache evidence is asserted before this step.
        assert authored_cache
        assert {chain[0] for chain in authored_cache} == {"F.Cu", "In1.Cu"}
        assert all(len(points) >= 3 for _, _, points in authored_cache)
        assert all(
            math.isfinite(coordinate)
            for _, _, points in authored_cache
            for point in points
            for coordinate in point
        )
    else:
        assert authored_cache == expected_cache
    expected_unconnected_ordinal = None if upgraded else 0
    assert board.segments[0].net.ordinal == expected_unconnected_ordinal
    assert board.segments[0].layer == "F.Cu"
    assert not board.segments[0].net.name
    assert len(board.arcs) == 4
    assert all(arc.net.ordinal == expected_unconnected_ordinal for arc in board.arcs)
    assert all(arc.layer == "In1.Cu" for arc in board.arcs)
    assert all(not arc.net.name for arc in board.arcs)
    assert {arc.uuid: _arc_semantics(arc) for arc in board.arcs} == {
        "00000000-0000-0000-0000-0000000000c9": ("cw", "minor"),
        "00000000-0000-0000-0000-0000000000ca": ("cw", "major"),
        "00000000-0000-0000-0000-0000000000cb": ("ccw", "minor"),
        "00000000-0000-0000-0000-0000000000cc": ("ccw", "major"),
    }


def test_python_reader_accepts_authored_zone_and_cache_chains(
    authored_board: Path,
) -> None:
    source = authored_board.read_text(encoding="utf-8")
    assert "(connect_pads yes" not in source
    assert "(filled_areas_thickness no)" in source
    assert_zone_semantics(authored_board, upgraded=False)
    assert_rule_area_semantics(authored_board.with_name("native-rule-areas.kicad_pcb"))


def assert_rule_area_semantics(board_path: Path, *, upgraded: bool = False) -> None:
    board = KiCadPcb.from_file(board_path)
    areas = sorted(board.zones, key=lambda area: area.uuid)
    assert len(areas) == 4
    expected_sources = [
        None,
        (False, "/Power stage"),
        (True, 'Fast "IO"'),
        (True, "Local group"),
    ]
    for index, (area, placement) in enumerate(
        zip(areas, expected_sources, strict=True)
    ):
        assert area.uuid == f"00000000-0000-0000-0000-{300 + index:012x}"
        assert area.layers == ["F.Cu", "B.Cu"]
        assert area.locked is True
        assert area.name == f"Rule {index}"
        assert not area.filled_polygons
        assert area.keepout is not None
        assert (
            area.keepout.tracks,
            area.keepout.vias,
            area.keepout.pads,
            area.keepout.copperpour,
            area.keepout.footprints,
        ) == ("not_allowed", "allowed", "allowed", "not_allowed", "allowed")
        if placement is None and upgraded:
            # KiCad serializes disabled/empty placement defaults on every rule
            # area. Fresh source absence is checked separately above.
            assert area.placement is not None
            assert (area.placement.enabled, area.placement.source) == (False, "")
            assert area.placement.source_type.value == "sheetname"
        elif placement is None:
            assert area.placement is None
        else:
            assert area.placement is not None
            assert (area.placement.enabled, area.placement.source) == placement
            assert (
                area.placement.source_type.value
                == ["", "sheetname", "component_class", "group"][index]
            )
        assert [
            [tuple(point) for point in polygon.points] for polygon in area.polygons
        ] == [
            [(1.0, 1.0), (9.0, 1.0), (9.0, 9.0), (1.0, 9.0)],
            [(3.0, 3.0), (3.0, 5.0), (5.0, 5.0), (5.0, 3.0)],
        ]


def test_kicad_cli_refills_authored_zone_cache_chains_when_available(
    authored_board: Path, tmp_path: Path
) -> None:
    cli = resolve_kicad_cli(required_capability="any")
    if cli is None:
        pytest.skip("no kicad-cli is available for authored-zone acceptance")
    upgraded = tmp_path / authored_board.name
    shutil.copy2(authored_board, upgraded)
    completed = subprocess.run(
        [str(cli), "pcb", "upgrade", "--force", str(upgraded)],
        cwd=PACKAGE_ROOT,
        env=kicad_cli_subprocess_env(cli),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    upgraded_source = upgraded.read_text(encoding="utf-8")
    assert "(connect_pads yes" not in upgraded_source
    assert_zone_semantics(upgraded, upgraded=True)

    solid_board = authored_board.with_name("native-zone-solid.kicad_pcb")
    assert solid_board.is_file()
    assert "(connect_pads yes" in solid_board.read_text(encoding="utf-8")
    upgraded_solid = tmp_path / solid_board.name
    shutil.copy2(solid_board, upgraded_solid)
    completed = subprocess.run(
        [str(cli), "pcb", "upgrade", "--force", str(upgraded_solid)],
        cwd=PACKAGE_ROOT,
        env=kicad_cli_subprocess_env(cli),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert "(connect_pads yes" in upgraded_solid.read_text(encoding="utf-8")

    rule_board = authored_board.with_name("native-rule-areas.kicad_pcb")
    upgraded_rules = tmp_path / rule_board.name
    shutil.copy2(rule_board, upgraded_rules)
    completed = subprocess.run(
        [str(cli), "pcb", "upgrade", "--force", str(upgraded_rules)],
        cwd=PACKAGE_ROOT,
        env=kicad_cli_subprocess_env(cli),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert_rule_area_semantics(upgraded_rules, upgraded=True)

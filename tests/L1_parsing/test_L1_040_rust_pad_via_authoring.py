"""Independent acceptance for typed Rust pad and via source authoring."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess

import pytest

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadPcb


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def authored_board(tmp_path_factory: pytest.TempPathFactory) -> Path:
    output = tmp_path_factory.mktemp("rust-pad-via-authoring")
    environment = os.environ.copy()
    environment["KM_PAD_VIA_OUTPUT_DIR"] = str(output)
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "kicad-monkey-core",
            "--test",
            "pcb_authoring_pad_via_slice",
        ],
        cwd=PACKAGE_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Rust pad/via authoring gate failed:\n{completed.stdout}\n{completed.stderr}"
    )
    board = output / "native-pad-via.kicad_pcb"
    assert board.is_file()
    return board


def assert_pad_via_semantics(
    board_path: Path, *, expect_via_net_ordinals: bool = True
) -> None:
    board = KiCadPcb.from_file(board_path)
    pads = board.footprints[0].pads
    assert len(pads) == 3

    trapezoid = pads[0]
    assert trapezoid.shape.value == "trapezoid"
    assert (trapezoid.rect_delta_x, trapezoid.rect_delta_y) == (0.2, -0.1)
    assert trapezoid.solder_mask_margin == 0.05
    assert trapezoid.solder_paste_margin == -0.02
    assert trapezoid.solder_paste_margin_ratio == -0.1
    assert trapezoid.clearance == 0.08
    assert trapezoid.thermal_bridge_width == 0.3
    assert trapezoid.thermal_bridge_angle == 45.0
    assert trapezoid.thermal_gap == 0.2
    assert trapezoid.zone_connect == 2

    chamfered = pads[1]
    assert chamfered.shape.value == "roundrect"
    assert chamfered.roundrect_rratio == 0.15
    assert chamfered.chamfer_ratio == 0.2
    assert chamfered.chamfer_corners == ["top_left", "bottom_right"]

    plated_slot = pads[2]
    assert plated_slot.pad_type.value == "thru_hole"
    assert plated_slot.drill_oval is True
    assert (plated_slot.drill_width, plated_slot.drill_height) == (0.8, 1.6)
    assert (plated_slot.drill_offset_x, plated_slot.drill_offset_y) == (0.1, -0.1)
    assert plated_slot.remove_unused_layers is True
    assert plated_slot.keep_end_layers is False
    assert plated_slot.zone_connect == 3
    assert plated_slot.zone_layer_connections is not None
    assert plated_slot.zone_layer_connections.forced_layers == ("In1.Cu", "In2.Cu")

    vias = board.vias
    assert len(vias) == 6
    by_x = {via.at_x: via for via in vias}
    through = by_x[10.0]
    blind = by_x[12.0]
    buried = by_x[14.0]
    micro = by_x[16.0]
    start_end_only = by_x[18.0]
    unconnected = by_x[20.0]
    assert through.via_type is None
    assert through.layers == ["F.Cu", "B.Cu"]
    assert through.free is True
    assert (through.tenting.front, through.tenting.back) == (False, True)
    assert through.remove_unused_layers is True
    assert through.keep_end_layers is True
    assert through.start_end_only is None
    assert through.zone_layer_connections is not None
    assert through.zone_layer_connections.forced_layers == ("In2.Cu",)
    assert blind.via_type == "blind"
    assert blind.layers == ["F.Cu", "In1.Cu"]
    assert blind.covering.front is True
    assert blind.covering.back is None
    assert blind.capping is True
    assert buried.via_type == "blind"
    assert buried.layers == ["In1.Cu", "In2.Cu"]
    assert buried.plugging.front is None
    assert buried.plugging.back is False
    assert buried.filling is False
    assert micro.via_type == "micro"
    assert micro.layers == ["In2.Cu", "B.Cu"]
    assert (micro.size, micro.drill) == (0.3, 0.1)
    assert start_end_only.layers == ["F.Cu", "B.Cu"]
    assert start_end_only.start_end_only is True
    connected = [through, blind, buried, micro, start_end_only]
    assert all(via.net.name == "SIGNAL" for via in connected)
    expected_ordinal = 1 if expect_via_net_ordinals else None
    assert all(via.net.ordinal == expected_ordinal for via in connected)
    expected_unconnected_ordinal = 0 if expect_via_net_ordinals else None
    assert unconnected.net.ordinal == expected_unconnected_ordinal
    assert not unconnected.net.name


def test_python_reader_independently_accepts_typed_pad_via_source(
    authored_board: Path,
) -> None:
    assert_pad_via_semantics(authored_board)


def test_kicad_cli_preserves_typed_pad_via_source_when_available(
    authored_board: Path, tmp_path: Path
) -> None:
    cli = resolve_kicad_cli(required_capability="any")
    if cli is None:
        pytest.skip("no kicad-cli is available for ongoing authored-source acceptance")
    upgraded = tmp_path / authored_board.name
    shutil.copy2(authored_board, upgraded)
    command = [str(cli), "pcb", "upgrade", "--force", str(upgraded)]
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        env=kicad_cli_subprocess_env(cli),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, (
        f"kicad-cli rejected authored source: {' '.join(command)}\n"
        f"{completed.stdout}\n{completed.stderr}"
    )
    # KiCad 10 canonicalizes 20241229 numeric via net references to name-only
    # references; connectivity remains exact through the board net declaration.
    assert_pad_via_semantics(upgraded, expect_via_net_ordinals=False)


def test_kicad_cli_pins_explicit_false_remove_unused_behavior(
    authored_board: Path, tmp_path: Path
) -> None:
    cli = resolve_kicad_cli(required_capability="any")
    if cli is None:
        pytest.skip("no kicad-cli is available for authored-source loss probes")
    source = authored_board.read_text(encoding="utf-8")
    source = source.replace(
        "\t\t\t(remove_unused_layers yes)\n\t\t\t(keep_end_layers no)\n",
        "\t\t\t(remove_unused_layers no)\n",
        1,
    )
    source = source.replace(
        "\t\t(remove_unused_layers yes)\n\t\t(keep_end_layers yes)\n",
        "\t\t(remove_unused_layers no)\n",
        1,
    )
    assert source.count("remove_unused_layers no") == 2
    probe = tmp_path / "loss-probe.kicad_pcb"
    probe.write_text(source, encoding="utf-8")
    completed = subprocess.run(
        [str(cli), "pcb", "upgrade", "--force", str(probe)],
        cwd=PACKAGE_ROOT,
        env=kicad_cli_subprocess_env(cli),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    board = KiCadPcb.from_file(probe)
    assert board.footprints[0].pads[2].remove_unused_layers is False
    assert (
        next(via for via in board.vias if via.at_x == 10.0).remove_unused_layers is None
    )

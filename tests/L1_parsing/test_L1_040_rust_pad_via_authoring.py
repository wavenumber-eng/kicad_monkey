"""Independent acceptance for typed Rust pad and via source authoring."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess

import pytest

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadPcb
from kicad_monkey.kicad_base import get_value, get_values
from kicad_monkey.kicad_sexpr import parse_sexp


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
    assert len(pads) == 4
    by_uuid = {pad.uuid: pad for pad in pads}
    assert len(by_uuid) == len(pads)
    cut_only = by_uuid["00000000-0000-0000-0000-000000000068"]
    assert cut_only.pad_type.value == "np_thru_hole"
    assert cut_only.layers == []
    assert (cut_only.at_x, cut_only.at_y) == (4.0, 3.0)
    assert cut_only.drill_oval is False
    assert (cut_only.drill, cut_only.drill_offset_x, cut_only.drill_offset_y) == (
        0.8,
        0.1,
        -0.1,
    )
    assert not cut_only.net.name

    trapezoid = by_uuid["00000000-0000-0000-0000-000000000065"]
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

    chamfered = by_uuid["00000000-0000-0000-0000-000000000066"]
    assert chamfered.shape.value == "roundrect"
    assert chamfered.roundrect_rratio == 0.15
    assert chamfered.chamfer_ratio == 0.2
    assert chamfered.chamfer_corners == ["top_left", "bottom_right"]

    plated_slot = by_uuid["00000000-0000-0000-0000-000000000067"]
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
    assert through.backdrill is not None
    assert through.backdrill.size == 0.5
    assert (through.backdrill.layers.start, through.backdrill.layers.end) == (
        "B.Cu",
        "In2.Cu",
    )
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
    assert_padstack_semantics(authored_board, upgraded=False)
    assert_padstack_semantics(
        authored_board.with_name("SparseStack.kicad_mod"), upgraded=False
    )


def assert_padstack_semantics(path: Path, *, upgraded: bool) -> None:
    # Public source-tree reader: the legacy Python pad DTO has no typed stack.
    source = parse_sexp(path.read_text(encoding="utf-8"))
    standalone = source[0] == "footprint"
    footprint = (
        source
        if standalone
        else next(
            item for item in source if isinstance(item, list) and item[0] == "footprint"
        )
    )
    pad_uuid = (
        "00000000-0000-0000-0000-00000000012c"
        if standalone
        else "00000000-0000-0000-0000-000000000067"
    )
    pad = next(
        item
        for item in footprint
        if isinstance(item, list)
        and item[0] == "pad"
        and get_value(item, "uuid") == pad_uuid
    )
    stack = next(
        item for item in pad if isinstance(item, list) and item[0] == "padstack"
    )
    rows = {
        item[1]: item for item in stack if isinstance(item, list) and item[0] == "layer"
    }
    assert get_value(stack, "mode") == ("custom" if standalone else "front_inner_back")
    assert get_value(rows["B.Cu"], "shape") == ("custom" if standalone else "rect")
    assert get_values(rows["B.Cu"], "size") == [1.4, 1.8]
    assert get_values(rows["B.Cu"], "offset") == [0.2, -0.3]
    assert get_value(rows["B.Cu"], "zone_connect") == -1
    assert get_value(rows["B.Cu"], "clearance") == 0
    assert get_value(rows["B.Cu"], "thermal_gap") == 0.2
    assert get_value(rows["B.Cu"], "thermal_bridge_width") == 0.25
    if standalone:
        options = next(
            item
            for item in rows["B.Cu"]
            if isinstance(item, list) and item[0] == "options"
        )
        assert get_value(options, "anchor") == "circle"
        assert get_value(options, "clearance") is None
        primitives = next(
            item
            for item in rows["B.Cu"]
            if isinstance(item, list) and item[0] == "primitives"
        )
        assert len(primitives) == 2
        line = primitives[1]
        assert line[0] == "gr_line"
        assert get_values(line, "start") == [-0.4, 0.0]
        assert get_values(line, "end") == [0.4, 0.2]
        assert get_value(line, "width") == 0.1
        assert len(rows) == (31 if upgraded else 1)
        if upgraded:
            # KiCad's library save expands absent copper rows using its own
            # base-geometry fallback; the source author never fabricated them.
            assert get_values(rows["In1.Cu"], "size") == [2.0, 2.0]
            assert get_value(rows["In1.Cu"], "shape") == "circle"
    else:
        assert set(rows) == {"Inner", "B.Cu"}
        assert get_value(rows["Inner"], "shape") == "circle"
        via = next(
            item
            for item in source
            if isinstance(item, list)
            and item[0] == "via"
            and get_value(item, "uuid") == "00000000-0000-0000-0000-0000000000c9"
        )
        via_stack = next(
            item for item in via if isinstance(item, list) and item[0] == "padstack"
        )
        via_rows = {
            item[1]: item
            for item in via_stack
            if isinstance(item, list) and item[0] == "layer"
        }
        assert get_value(via_stack, "mode") == "custom"
        assert get_value(via_rows["In1.Cu"], "size") == 0.6
        assert get_value(via_rows["B.Cu"], "size") == 0.7
        assert len(via_rows) == (3 if upgraded else 2)
        if upgraded:
            assert get_value(via_rows["In2.Cu"], "size") == 0.8


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
    assert_padstack_semantics(upgraded, upgraded=True)
    library = tmp_path / "Stacks.pretty"
    library.mkdir()
    footprint = library / "SparseStack.kicad_mod"
    shutil.copy2(authored_board.with_name(footprint.name), footprint)
    completed = subprocess.run(
        [str(cli), "fp", "upgrade", "--force", str(library)],
        cwd=PACKAGE_ROOT,
        env=kicad_cli_subprocess_env(cli),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert_padstack_semantics(footprint, upgraded=True)


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
    plated_slot = next(
        pad
        for pad in board.footprints[0].pads
        if pad.uuid == "00000000-0000-0000-0000-000000000067"
    )
    assert plated_slot.remove_unused_layers is False
    assert (
        next(via for via in board.vias if via.at_x == 10.0).remove_unused_layers is None
    )

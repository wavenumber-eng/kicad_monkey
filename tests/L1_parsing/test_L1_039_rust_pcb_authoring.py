"""Independent acceptance for the Rust fresh PCB/footprint authoring slice."""

from __future__ import annotations

import base64
import os
from pathlib import Path
import shutil
import subprocess

import pytest
import zstandard

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadFootprint, KiCadPcb
from kicad_monkey.kicad_base import find_element, get_value, get_values


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def _assert_board_tenting(board: KiCadPcb) -> None:
    clause = find_element(board.setup_sexp, "tenting")
    assert clause is not None
    # KiCad 9 uses bare side flags; KiCad 10 saves nested explicit booleans.
    assert "front" in clause or get_value(clause, "front", "no") == "yes"
    assert "back" not in clause and get_value(clause, "back", "no") == "no"


def _assert_custom_composition(pad) -> None:
    assert pad.uuid == "00000000-0000-0000-0000-000000000194"
    assert pad.die_length == -0.25
    assert (pad.custom_options.anchor, pad.custom_options.clearance) == (
        "circle",
        "convexhull",
    )
    primitives = pad.custom_primitives
    assert [item.primitive_type for item in primitives] == [
        "gr_poly",
        "gr_line",
        "gr_arc",
        "gr_circle",
        "gr_rect",
        "gr_curve",
        "gr_poly",
    ]
    assert [item.width for item in primitives] == [0.01, 0.2, 0.1, 0.0, 0.1, 0.1, 0.0]
    assert [primitives[index].is_filled for index in (0, 3, 4, 6)] == [
        True,
        True,
        False,
        True,
    ]
    assert primitives[0].points == [(-1.0, -0.5), (1.0, -0.5), (0.0, 1.0)]
    assert primitives[5].points == [(-1.0, 0.0), (-0.5, 1.0), (0.5, 1.0), (1.0, 0.0)]
    # These public source primitives round-trip non-polygon geometry verbatim.
    arc = primitives[2].to_sexp()
    assert get_values(arc, "start") == [1.0, 0.0]
    assert get_values(arc, "mid") == [0.0, 1.0]
    assert get_values(arc, "end") == [-1.0, 0.0]
    assert get_values(primitives[4].to_sexp(), "radius") == [0.1]


def _assert_board_metadata(board) -> None:
    groups = {group.name: group for group in board.groups}
    assert set(groups) == {'Routes "A"', "Assembly"}
    routes = groups['Routes "A"']
    assert routes.locked
    assert routes.uuid == "00000000-0000-0000-0000-000000000320"
    assert set(routes.members) == {
        "00000000-0000-0000-0000-00000000012c",
        "00000000-0000-0000-0000-00000000012d",
    }
    assert set(groups["Assembly"].members) == {
        routes.uuid,
        "00000000-0000-0000-0000-000000000064",
    }
    assert board.get_property("REVISION") == 'A "prototype"'
    assert board.get_property("EMPTY") == ""
    pad = next(
        pad
        for footprint in board.footprints
        for pad in footprint.pads
        if pad.uuid == "00000000-0000-0000-0000-000000000066"
    )
    assert (pad.pinfunction, pad.pintype, pad.die_length) == ("VDD", "power_in", 0.75)


def _effective_surface_policy(pad, footprint, board) -> tuple[float, float, float]:
    def resolve(local_name: str, board_name: str) -> float:
        pad_value = getattr(pad, local_name)
        footprint_value = getattr(footprint, local_name)
        if pad_value is not None:
            return pad_value
        if footprint_value is not None:
            return footprint_value
        return getattr(board, board_name)

    return (
        resolve("solder_mask_margin", "pad_to_mask_clearance"),
        resolve("solder_paste_margin", "pad_to_paste_clearance"),
        resolve("solder_paste_margin_ratio", "pad_to_paste_clearance_ratio"),
    )


@pytest.fixture(scope="module")
def authored_sources(tmp_path_factory: pytest.TempPathFactory) -> tuple[Path, Path]:
    output = tmp_path_factory.mktemp("rust-authoring")
    environment = os.environ.copy()
    environment["KM_AUTHORING_OUTPUT_DIR"] = str(output)
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "kicad-monkey-core",
            "--test",
            "pcb_authoring_slice",
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
        f"Rust authoring gate failed:\n{completed.stdout}\n{completed.stderr}"
    )
    board = output / "native-authored.kicad_pcb"
    footprint = output / "Demo_Standalone.kicad_mod"
    assert board.is_file() and footprint.is_file()
    return board, footprint


def test_python_reader_independently_accepts_authored_semantics(
    authored_sources: tuple[Path, Path],
) -> None:
    board_path, footprint_path = authored_sources
    board = KiCadPcb.from_file(board_path)
    _assert_board_tenting(board)
    assert board.version == 20241229
    _assert_board_metadata(board)
    assert [(net.ordinal, net.name) for net in board.nets] == [(1, "GND")]
    assert [
        (line.start_x, line.start_y, line.end_x, line.end_y, line.layer)
        for line in board.gr_lines
    ] == [
        (0.0, 0.0, 50.0, 0.0, "Edge.Cuts"),
        (50.0, 0.0, 50.0, 40.0, "Edge.Cuts"),
        (50.0, 40.0, 0.0, 40.0, "Edge.Cuts"),
        (0.0, 40.0, 0.0, 0.0, "Edge.Cuts"),
        (20.0, 15.0, 30.0, 15.0, "Edge.Cuts"),
        (30.0, 15.0, 30.0, 25.0, "Edge.Cuts"),
        (30.0, 25.0, 20.0, 25.0, "Edge.Cuts"),
        (20.0, 25.0, 20.0, 15.0, "Edge.Cuts"),
    ]
    assert [
        (item.layer, item.at_x, item.at_y, item.at_angle, len(item.pads))
        for item in board.footprints
    ] == [
        ("F.Cu", 10.0, 20.0, 0.0, 3),
        ("B.Cu", 40.0, 30.0, 90.0, 3),
    ]
    assert [item.properties[0].value for item in board.footprints] == ["U1", "U1"]
    assert board.footprints[0].uuid != board.footprints[1].uuid
    assert all("dnp" in item.attr for item in board.footprints)
    assert board.footprints[0].locked is True
    assert board.footprints[0].path == (
        "/00000000-0000-0000-0000-0000000002bc/00000000-0000-0000-0000-0000000002bd"
    )
    assert (board.footprints[0].sheetname, board.footprints[0].sheetfile) == (
        "Power",
        "power.kicad_sch",
    )
    assert (board.footprints[0].clearance, board.footprints[0].zone_connect) == (0.0, 2)
    assert (board.footprints[1].clearance, board.footprints[1].zone_connect) == (
        None,
        None,
    )
    assert (
        board.pad_to_mask_clearance,
        board.pad_to_paste_clearance,
        board.pad_to_paste_clearance_ratio,
    ) == (0.01, -0.005, -0.02)
    assert (
        board.footprints[0].solder_mask_margin,
        board.footprints[0].solder_paste_margin,
        board.footprints[0].solder_paste_margin_ratio,
    ) == (0.03, -0.01, -0.05)
    assert (
        board.footprints[1].solder_mask_margin,
        board.footprints[1].solder_paste_margin,
        board.footprints[1].solder_paste_margin_ratio,
    ) == (None, None, None)
    assert board.footprints[0].pads[0].net.name == "GND"
    assert board.footprints[0].pads[0].layers == ["*.Cu", "*.Mask"]
    assert board.footprints[0].pads[0].solder_mask_margin == 0.05
    assert board.footprints[0].pads[0].remove_unused_layers is True
    assert board.footprints[0].pads[0].keep_end_layers is True
    assert board.footprints[0].pads[1].drill_oval is True
    assert board.footprints[0].pads[1].drill_height == 1.6
    assert (
        board.footprints[0].pads[1].at_x,
        board.footprints[0].pads[1].at_y,
        board.footprints[0].pads[1].at_angle,
        board.footprints[0].pads[1].drill_offset_x,
        board.footprints[0].pads[1].drill_offset_y,
    ) == (3.0, 0.0, 90.0, 0.1, -0.1)
    assert (
        board.footprints[0].pads[2].solder_mask_margin,
        board.footprints[0].pads[2].solder_paste_margin,
        board.footprints[0].pads[2].solder_paste_margin_ratio,
    ) == (0.05, -0.02, -0.1)
    assert (
        board.footprints[1].pads[2].solder_mask_margin,
        board.footprints[1].pads[2].solder_paste_margin,
        board.footprints[1].pads[2].solder_paste_margin_ratio,
    ) == (None, None, None)
    assert _effective_surface_policy(
        board.footprints[0].pads[2], board.footprints[0], board
    ) == (0.05, -0.02, -0.1)
    assert _effective_surface_policy(
        board.footprints[0].pads[1], board.footprints[0], board
    ) == (0.03, -0.01, -0.05)
    assert _effective_surface_policy(
        board.footprints[1].pads[2], board.footprints[1], board
    ) == (0.01, -0.005, -0.02)
    assert (
        board.footprints[1].pads[0].at_x,
        board.footprints[1].pads[0].at_y,
        board.footprints[1].properties[0].at_x,
        board.footprints[1].properties[0].at_y,
        board.footprints[1].properties[0].layer,
        board.footprints[1].fp_lines[0].start_x,
        board.footprints[1].fp_lines[0].end_x,
        board.footprints[1].fp_lines[0].layer,
    ) == (0.0, 0.0, 0.0, -2.0, "B.SilkS", -1.0, 1.0, "B.SilkS")
    front_numbered_pad = next(
        item for item in board.footprints[0].pads if item.number == "1"
    )
    bottom_numbered_pad = next(
        item for item in board.footprints[1].pads if item.number == "1"
    )
    assert (front_numbered_pad.uuid, bottom_numbered_pad.uuid) == (
        "00000000-0000-0000-0000-000000000066",
        "00000000-0000-0000-0000-0000000000ca",
    )
    assert len(board.embedded_files) == 1
    assert len(board.footprints[0].embedded_files) == 1
    board_resource = board.embedded_files[0]
    footprint_resource = board.footprints[0].embedded_files[0]
    assert (board_resource.name, footprint_resource.name) == (
        "board.step",
        "footprint.step",
    )
    assert (
        zstandard.decompress(
            base64.b64decode(board_resource.data), max_output_size=1024
        )
        == b"board-owned shared model"
    )
    assert (
        zstandard.decompress(
            base64.b64decode(footprint_resource.data), max_output_size=1024
        )
        == b"footprint-owned shared model"
    )
    model = board.footprints[0].models[0]
    assert model.path == "kicad-embed://footprint.step"
    assert model.offset == (0.5, 1.0, 1.5)
    assert model.scale == (1.0, 2.0, 1.0)
    assert model.rotate == (0.0, 45.0, 90.0)
    segment = board.segments[0]
    assert (
        segment.start_x,
        segment.start_y,
        segment.end_x,
        segment.end_y,
        segment.width,
        segment.layer,
        segment.net.ordinal,
        segment.net.name,
        segment.uuid,
    ) == (
        10.0,
        20.0,
        20.0,
        20.0,
        0.25,
        "F.Cu",
        1,
        "GND",
        "00000000-0000-0000-0000-00000000012c",
    )
    arc = board.arcs[0]
    assert (
        arc.start_x,
        arc.start_y,
        arc.mid_x,
        arc.mid_y,
        arc.end_x,
        arc.end_y,
        arc.width,
        arc.layer,
        arc.net.ordinal,
        arc.net.name,
        arc.uuid,
    ) == (
        20.0,
        20.0,
        25.0,
        25.0,
        30.0,
        20.0,
        0.25,
        "F.Cu",
        1,
        "GND",
        "00000000-0000-0000-0000-00000000012d",
    )

    footprint = KiCadFootprint.from_string(footprint_path.read_text(encoding="utf-8"))
    assert footprint.name == "Demo_Standalone"
    assert footprint.attr == ["dnp"]
    assert (footprint.clearance, footprint.zone_connect) == (0.12, 0)
    assert (
        footprint.solder_mask_margin,
        footprint.solder_paste_margin,
        footprint.solder_paste_margin_ratio,
    ) == (0.02, -0.01, -0.05)
    assert len(footprint.properties) == 2
    assert len(footprint.fp_texts) == 1
    assert footprint.fp_texts[0].text_type == "user"
    assert footprint.fp_texts[0].text == "ASSEMBLY"
    assert footprint.fp_texts[0].effects.font.size_x == 1.25
    assert footprint.fp_texts[0].effects.font.size_y == 0.75
    assert len(footprint.fp_rects) == 1
    assert len(footprint.pads) == 2
    assert footprint.pads[0].shape.value == "custom"
    _assert_custom_composition(footprint.pads[0])
    assert footprint.pads[1].pad_type.value == "np_thru_hole"
    assert footprint.pads[1].layers == []
    assert footprint.pads[1].drill_height == 1.6
    assert len(footprint.models) == 1
    assert footprint.models[0].path == "kicad-embed://native.step"
    assert footprint.models[0].offset == (1.0, 2.0, 3.0)
    assert len(footprint.embedded_files) == 1
    resource = footprint.embedded_files[0]
    assert resource.name == "native.step"
    assert resource.file_type == "model"
    assert footprint.models[0].path.removeprefix("kicad-embed://") == resource.name
    assert zstandard.decompress(
        base64.b64decode(resource.data), max_output_size=1024
    ) == (b"tiny synthetic model bytes")


def test_kicad_cli_accepts_fresh_board_and_footprint_when_available(
    authored_sources: tuple[Path, Path], tmp_path: Path
) -> None:
    cli = resolve_kicad_cli(required_capability="any")
    if cli is None:
        pytest.skip("no kicad-cli is available for ongoing authored-source acceptance")
    board_source, footprint_source = authored_sources
    board = tmp_path / board_source.name
    shutil.copy2(board_source, board)
    library = tmp_path / "Native.pretty"
    library.mkdir()
    shutil.copy2(footprint_source, library / footprint_source.name)
    environment = kicad_cli_subprocess_env(cli)
    for command in [
        [str(cli), "pcb", "upgrade", "--force", str(board)],
        [str(cli), "fp", "upgrade", "--force", str(library)],
    ]:
        completed = subprocess.run(
            command,
            cwd=PACKAGE_ROOT,
            env=environment,
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

    upgraded_board = KiCadPcb.from_file(board)
    _assert_board_tenting(upgraded_board)
    assert len(upgraded_board.gr_lines) == 8
    upgraded_resources = {item.name: item for item in upgraded_board.embedded_files}
    assert set(upgraded_resources) == {"board.step", "footprint.step"}
    assert (
        zstandard.decompress(
            base64.b64decode(upgraded_resources["board.step"].data),
            max_output_size=1024,
        )
        == b"board-owned shared model"
    )
    assert (
        zstandard.decompress(
            base64.b64decode(upgraded_resources["footprint.step"].data),
            max_output_size=1024,
        )
        == b"footprint-owned shared model"
    )
    _assert_board_metadata(upgraded_board)
    assert [item.properties[0].value for item in upgraded_board.footprints] == [
        "U1",
        "U1",
    ]
    assert upgraded_board.footprints[0].uuid != upgraded_board.footprints[1].uuid
    assert all("dnp" in item.attr for item in upgraded_board.footprints)
    assert (
        upgraded_board.pad_to_mask_clearance,
        upgraded_board.pad_to_paste_clearance,
        upgraded_board.pad_to_paste_clearance_ratio,
    ) == (0.01, -0.005, -0.02)
    upgraded_front = next(
        item for item in upgraded_board.footprints if item.layer == "F.Cu"
    )
    upgraded_bottom = next(
        item for item in upgraded_board.footprints if item.layer == "B.Cu"
    )
    assert upgraded_front.locked is True
    assert upgraded_front.path == (
        "/00000000-0000-0000-0000-0000000002bc/00000000-0000-0000-0000-0000000002bd"
    )
    assert (upgraded_front.sheetname, upgraded_front.sheetfile) == (
        "Power",
        "power.kicad_sch",
    )
    assert (upgraded_front.clearance, upgraded_front.zone_connect) == (0.0, 2)
    assert (upgraded_bottom.clearance, upgraded_bottom.zone_connect) == (None, None)
    upgraded_front_numbered_pad = next(
        item for item in upgraded_front.pads if item.number == "1"
    )
    upgraded_bottom_numbered_pad = next(
        item for item in upgraded_bottom.pads if item.number == "1"
    )
    assert (
        upgraded_front_numbered_pad.uuid,
        upgraded_bottom_numbered_pad.uuid,
    ) == (
        "00000000-0000-0000-0000-000000000066",
        "00000000-0000-0000-0000-0000000000ca",
    )
    assert (
        upgraded_front.solder_mask_margin,
        upgraded_front.solder_paste_margin,
        upgraded_front.solder_paste_margin_ratio,
    ) == (0.03, -0.01, -0.05)
    assert upgraded_front.models[0].path == "kicad-embed://footprint.step"
    assert upgraded_front.embedded_files[0].name == "footprint.step"
    assert not upgraded_front.embedded_files[0].data
    assert (
        upgraded_board.segments[0].start_x,
        upgraded_board.segments[0].end_x,
        upgraded_board.segments[0].net.name,
    ) == (10.0, 20.0, "GND")
    upgraded_th_pad = next(
        item for item in upgraded_board.footprints[0].pads if item.number == "1"
    )
    assert upgraded_th_pad.solder_mask_margin == 0.05
    assert upgraded_th_pad.remove_unused_layers is True
    assert upgraded_th_pad.keep_end_layers is True
    upgraded_smd_pad = next(item for item in upgraded_front.pads if item.number == "2")
    assert (
        upgraded_smd_pad.solder_mask_margin,
        upgraded_smd_pad.solder_paste_margin,
        upgraded_smd_pad.solder_paste_margin_ratio,
    ) == (0.05, -0.02, -0.1)
    upgraded_inherited_front_pad = next(
        item for item in upgraded_front.pads if item.pad_type.value == "np_thru_hole"
    )
    upgraded_inherited_bottom_pad = next(
        item for item in upgraded_bottom.pads if item.number == "2"
    )
    assert _effective_surface_policy(
        upgraded_smd_pad, upgraded_front, upgraded_board
    ) == (0.05, -0.02, -0.1)
    assert _effective_surface_policy(
        upgraded_inherited_front_pad, upgraded_front, upgraded_board
    ) == (0.03, -0.01, -0.05)
    assert _effective_surface_policy(
        upgraded_inherited_bottom_pad, upgraded_bottom, upgraded_board
    ) == (0.01, -0.005, -0.02)
    upgraded_npth_pad = next(
        item
        for item in upgraded_board.footprints[0].pads
        if item.pad_type.value == "np_thru_hole"
    )
    assert not upgraded_npth_pad.net.name
    upgraded_footprint = KiCadFootprint.from_string(
        (library / footprint_source.name).read_text(encoding="utf-8")
    )
    upgraded_properties = {
        item.name: item.value for item in upgraded_footprint.properties
    }
    assert (upgraded_footprint.clearance, upgraded_footprint.zone_connect) == (0.12, 0)
    assert upgraded_footprint.attr == ["dnp"]
    assert upgraded_properties["Reference"] == "REF**"
    assert upgraded_properties["Value"] == "Demo_Standalone"
    assert (
        upgraded_footprint.solder_mask_margin,
        upgraded_footprint.solder_paste_margin,
        upgraded_footprint.solder_paste_margin_ratio,
    ) == (0.02, -0.01, -0.05)
    assert [
        (item.text_type, item.text, item.effects.font.size_x, item.effects.font.size_y)
        for item in upgraded_footprint.fp_texts
    ] == [("user", "ASSEMBLY", 1.25, 0.75)]
    assert upgraded_footprint.models[0].path == "kicad-embed://native.step"
    assert upgraded_footprint.embedded_files[0].name == "native.step"
    upgraded_cut = next(
        pad for pad in upgraded_footprint.pads if pad.pad_type.value == "np_thru_hole"
    )
    assert upgraded_cut.layers == []
    assert (upgraded_cut.drill_width, upgraded_cut.drill_height) == (0.8, 1.6)
    assert (upgraded_cut.drill_offset_x, upgraded_cut.drill_offset_y) == (0.1, -0.1)
    upgraded_smd_pad = next(pad for pad in upgraded_footprint.pads if pad.number == "1")
    _assert_custom_composition(upgraded_smd_pad)
    assert upgraded_smd_pad.solder_mask_margin == 0.04
    assert upgraded_smd_pad.solder_paste_margin == -0.02
    assert upgraded_smd_pad.solder_paste_margin_ratio == -0.1

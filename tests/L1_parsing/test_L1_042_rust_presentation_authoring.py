"""Independent acceptance for typed Rust presentation-source authoring."""

from __future__ import annotations

import math
import os
from pathlib import Path
import shutil
import subprocess

import pytest

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadFootprint, KiCadPcb


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def authored_sources(tmp_path_factory: pytest.TempPathFactory) -> tuple[Path, Path]:
    output = tmp_path_factory.mktemp("rust-presentation-authoring")
    environment = os.environ.copy()
    environment["KM_PRESENTATION_OUTPUT_DIR"] = str(output)
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "kicad-monkey-core",
            "--test",
            "pcb_authoring_presentation_slice",
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
    board = output / "native-presentation.kicad_pcb"
    footprint = output / "Demo_Presentation.kicad_mod"
    assert board.is_file() and footprint.is_file()
    return board, footprint


def cache_signature(
    cache: object,
) -> tuple[str, float, list[list[list[tuple[float, float]]]]]:
    assert cache is not None
    return (
        cache.text,
        cache.angle,
        [
            [[tuple(point) for point in contour.points] for contour in polygon.contours]
            for polygon in cache.polygons
        ],
    )


def assert_finite_nonempty_cache(cache: object, *, text: str, angle: float) -> None:
    cache_text, cache_angle, polygons = cache_signature(cache)
    assert (cache_text, cache_angle) == (text, angle)
    assert polygons
    assert all(contours for contours in polygons)
    assert all(len(points) >= 3 for contours in polygons for points in contours)
    assert all(
        math.isfinite(coordinate)
        for contours in polygons
        for points in contours
        for point in points
        for coordinate in point
    )


def assert_presentation_semantics(
    board_path: Path, footprint_path: Path, *, upgraded: bool
) -> None:
    board = KiCadPcb.from_file(board_path)
    assert len(board.gr_circles) == 1
    circle = board.gr_circles[0]
    assert (circle.center_x, circle.center_y, circle.end_x, circle.end_y) == (
        10.0,
        10.0,
        12.0,
        10.0,
    )
    assert circle.layer == "B.SilkS"

    assert len(board.gr_texts) == 2
    ttf, native = board.gr_texts
    assert (ttf.text, ttf.at_x, ttf.at_y, ttf.at_angle, ttf.layer) == (
        "BOARD-TTF",
        15.0,
        10.0,
        90.0,
        "F.SilkS",
    )
    assert (
        ttf.effects.font.face,
        ttf.effects.font.size_x,
        ttf.effects.font.size_y,
        ttf.effects.font.thickness,
        ttf.effects.font.line_spacing,
        ttf.effects.font.bold,
        ttf.effects.font.italic,
    ) == ("Arial", 1.2, 0.8, 0.12, 1.1, True, True)
    assert ttf.effects.justify == ["right", "top", "mirror"]
    if upgraded:
        assert_finite_nonempty_cache(ttf.render_cache, text="BOARD-TTF", angle=90.0)
    else:
        assert cache_signature(ttf.render_cache) == (
            "BOARD-TTF",
            90.0,
            [
                [
                    [(1.0, 0.0), (2.25, 3.0), (4.0, 5.5)],
                    [(1.5, 1.0), (1.75, 1.5), (2.0, 1.0)],
                ]
            ],
        )
    assert (
        native.text,
        native.at_angle,
        native.layer,
        native.knockout,
        native.effects.font.face,
        native.effects.justify,
        native.render_cache,
    ) == (
        "BOARD-NATIVE",
        -45.0,
        "B.SilkS",
        True,
        None,
        ["left", "bottom", "mirror"],
        None,
    )

    assert len(board.gr_text_boxes) == 1
    board_box = board.gr_text_boxes[0]
    assert (
        board_box.text,
        board_box.start_x,
        board_box.end_x,
        board_box.angle,
        board_box.layer,
        board_box.locked,
        board_box.border,
        board_box.knockout,
        board_box.effects.font.face,
        board_box.effects.justify,
    ) == (
        "BOARD-BOX",
        20.0,
        28.0,
        30.0,
        "B.SilkS",
        True,
        False,
        True,
        None,
        ["left", "bottom", "mirror"],
    )

    occurrence = next(item for item in board.footprints if item.at_angle == 135.0)
    assert (
        occurrence.layer,
        occurrence.at_x,
        occurrence.at_y,
        occurrence.at_angle,
    ) == (
        "B.Cu",
        30.0,
        20.0,
        135.0,
    )
    reference = occurrence.properties[0]
    assert (
        reference.name,
        reference.value,
        reference.at_x,
        reference.at_y,
        reference.at_angle,
        reference.layer,
        reference.unlocked,
        reference.effects.font.face,
        reference.effects.justify,
    ) == (
        "Reference",
        "R1",
        1.0,
        2.0,
        15.0,
        "B.SilkS",
        True,
        "Arial",
        ["right", "top", "mirror"],
    )
    if upgraded:
        assert_finite_nonempty_cache(reference.render_cache, text="R1", angle=15.0)
        occurrence_points = [
            point
            for polygon in reference.render_cache.polygons
            for contour in polygon.contours
            for point in contour.points
        ]
        assert all(20.0 < x < 40.0 and 5.0 < y < 30.0 for x, y in occurrence_points)
    else:
        assert cache_signature(reference.render_cache) == (
            "R1",
            15.0,
            [
                [
                    [
                        (30.70710678118655, 15.878679656440358),
                        (31.957106781186553, 18.878679656440358),
                        (33.70710678118655, 21.378679656440358),
                    ],
                    [
                        (31.207106781186553, 16.878679656440358),
                        (31.457106781186553, 17.378679656440358),
                        (31.707106781186553, 16.878679656440358),
                    ],
                ]
            ],
        )
    footprint_text = occurrence.fp_texts[0]
    expected_text_angle = 330.0 if upgraded else -30.0
    assert (
        footprint_text.text,
        footprint_text.at_angle,
        footprint_text.layer,
        footprint_text.knockout,
        footprint_text.unlocked,
        footprint_text.effects.font.face,
        footprint_text.render_cache,
    ) == (
        "NATIVE~{A}",
        expected_text_angle,
        "B.SilkS",
        True,
        True,
        None,
        None,
    )
    assert occurrence.fp_lines[0].layer == "B.SilkS"
    boxed_occurrence = next(item for item in board.footprints if item.fp_text_boxes)
    assert (
        boxed_occurrence.layer,
        boxed_occurrence.at_x,
        boxed_occurrence.at_y,
        boxed_occurrence.at_angle,
    ) == ("F.Cu", 50.0, 20.0, 0.0)
    occurrence_box = boxed_occurrence.fp_text_boxes[0]
    expected_polygon = [
        (-3.0, -2.0),
        (3.0, -2.0),
        (3.0, 2.0),
        (-3.0, 2.0),
    ]
    assert (
        occurrence_box.text,
        occurrence_box.start_x,
        occurrence_box.start_y,
        occurrence_box.end_x,
        occurrence_box.end_y,
        occurrence_box.polygon_points,
        occurrence_box.angle,
        occurrence_box.layer,
        occurrence_box.locked,
    ) == (
        "BOX",
        -3.0,
        -2.0,
        3.0,
        2.0,
        expected_polygon,
        45.0,
        "B.SilkS",
        True,
    )

    footprint = KiCadFootprint.from_string(footprint_path.read_text(encoding="utf-8"))
    standalone_reference = footprint.properties[0]
    assert standalone_reference.effects.font.face == "Arial"
    assert standalone_reference.effects.justify == ["right", "top", "mirror"]
    if upgraded:
        assert_finite_nonempty_cache(
            standalone_reference.render_cache, text="R1", angle=15.0
        )
    else:
        assert cache_signature(standalone_reference.render_cache) == (
            "R1",
            15.0,
            [
                [
                    [(1.0, 0.0), (2.25, 3.0), (4.0, 5.5)],
                    [(1.5, 1.0), (1.75, 1.5), (2.0, 1.0)],
                ]
            ],
        )
    standalone_text = footprint.fp_texts[0]
    assert standalone_text.at_angle == expected_text_angle
    assert standalone_text.knockout is True
    assert standalone_text.unlocked is True
    assert standalone_text.render_cache is None
    standalone_box = footprint.fp_text_boxes[0]
    assert (
        standalone_box.text,
        standalone_box.locked,
        standalone_box.border,
        standalone_box.knockout,
        standalone_box.effects.font.face,
    ) == ("BOX", False, True, False, None)


def test_python_reader_accepts_authored_presentation_source(
    authored_sources: tuple[Path, Path],
) -> None:
    assert_presentation_semantics(*authored_sources, upgraded=False)


def test_kicad_cli_accepts_and_normalizes_presentation_source(
    authored_sources: tuple[Path, Path], tmp_path: Path
) -> None:
    cli = resolve_kicad_cli(required_capability="any")
    if cli is None:
        pytest.skip("no kicad-cli is available for presentation-source acceptance")
    board_source, footprint_source = authored_sources
    board = tmp_path / board_source.name
    shutil.copy2(board_source, board)
    library = tmp_path / "Presentation.pretty"
    library.mkdir()
    footprint = library / footprint_source.name
    shutil.copy2(footprint_source, footprint)
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
        assert completed.returncode == 0, completed.stderr
    assert_presentation_semantics(board, footprint, upgraded=True)
    completed = subprocess.run(
        [str(cli), "pcb", "upgrade", "--force", str(board)],
        cwd=PACKAGE_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert_presentation_semantics(board, footprint, upgraded=True)

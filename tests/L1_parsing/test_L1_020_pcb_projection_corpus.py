"""PCB projection parity checks on real KiCad corpus boards."""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path

import pytest

from kicad_monkey import KiCadPcb, KiCadPcbProjection


_PROJECT_ROOT = Path(__file__).resolve().parents[2]

_CORPUS_BOARDS = (
    _PROJECT_ROOT
    / "tests"
    / "corpus"
    / ".unpacked"
    / "kicad"
    / "projects"
    / "4-ch-backplane"
    / "input"
    / "4-ch-backplane.kicad_pcb",
    _PROJECT_ROOT
    / "tests"
    / "corpus"
    / ".unpacked"
    / "kicad"
    / "projects"
    / "speedy_processing_module"
    / "input"
    / "11-10084__speedy_processing_module__B.kicad_pcb",
)

_PCB_COLLECTIONS = (
    "layers",
    "nets",
    "properties",
    "variants",
    "gr_texts",
    "gr_lines",
    "gr_rects",
    "gr_arcs",
    "gr_circles",
    "gr_polys",
    "gr_curves",
    "gr_text_boxes",
    "images",
    "barcodes",
    "tables",
    "footprints",
    "zones",
    "dimensions",
    "segments",
    "vias",
    "arcs",
    "groups",
    "generated_items",
    "embedded_files",
    "unknown_elements",
)


@lru_cache(maxsize=None)
def _full_board(path_text: str) -> KiCadPcb:
    return KiCadPcb.from_file(Path(path_text))


@lru_cache(maxsize=None)
def _projection(path_text: str) -> KiCadPcbProjection:
    return KiCadPcbProjection.from_file(Path(path_text))


@pytest.mark.parametrize("board_path", _CORPUS_BOARDS, ids=lambda path: path.parent.parent.name)
def test_pcb_projection_collection_counts_match_full_parser(board_path: Path) -> None:
    assert board_path.exists(), f"missing corpus board: {board_path}"

    board = _full_board(str(board_path))
    projection = _projection(str(board_path))

    for collection_name in _PCB_COLLECTIONS:
        projected = getattr(projection, collection_name)()
        full = getattr(board, collection_name)
        assert len(projected) == len(full), collection_name


@pytest.mark.parametrize("board_path", _CORPUS_BOARDS, ids=lambda path: path.parent.parent.name)
def test_pcb_projection_nested_pad_and_model_counts_match_full_parser(board_path: Path) -> None:
    assert board_path.exists(), f"missing corpus board: {board_path}"

    board = _full_board(str(board_path))
    projection = _projection(str(board_path))

    assert len(projection.pads()) == sum(len(footprint.pads) for footprint in board.footprints)
    assert len(projection.model_references()) == sum(len(footprint.models) for footprint in board.footprints)


@pytest.mark.parametrize("board_path", _CORPUS_BOARDS, ids=lambda path: path.parent.parent.name)
def test_pcb_projection_core_fields_match_full_parser(board_path: Path) -> None:
    assert board_path.exists(), f"missing corpus board: {board_path}"

    board = _full_board(str(board_path))
    projection = _projection(str(board_path))

    if board.footprints:
        projected_footprint = projection.footprints()[0]
        full_footprint = board.footprints[0]
        assert projected_footprint.library_link == full_footprint.library_link
        assert projected_footprint.get_property_value("Reference") == full_footprint.get_property_value("Reference")
        assert projection.source_span(projected_footprint).head in {"footprint", "module"}

    if board.vias:
        projected_via = projection.vias()[0]
        full_via = board.vias[0]
        assert projected_via.at_x == full_via.at_x
        assert projected_via.at_y == full_via.at_y
        assert projected_via.net == full_via.net

    if board.segments:
        projected_segment = projection.segments()[0]
        full_segment = board.segments[0]
        assert projected_segment.start_x == full_segment.start_x
        assert projected_segment.end_x == full_segment.end_x
        assert projected_segment.net == full_segment.net

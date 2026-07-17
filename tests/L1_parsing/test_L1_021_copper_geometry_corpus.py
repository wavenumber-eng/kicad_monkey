"""Copper geometry parity checks on real KiCad corpus boards."""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path

import pytest

from kicad_monkey import KiCadPcb, KiCadPcbProjection
from kicad_monkey.kicad_copper_geometry import emit_pcb_copper_geometry


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


@lru_cache(maxsize=None)
def _full_board(path_text: str) -> KiCadPcb:
    return KiCadPcb.from_file(Path(path_text))


@lru_cache(maxsize=None)
def _full_document(path_text: str):
    return emit_pcb_copper_geometry(_full_board(path_text))


def _polygon_area_nm2(ring: tuple[tuple[int, int], ...]) -> int:
    return abs(
        sum(
            ring[index][0] * ring[(index + 1) % len(ring)][1]
            - ring[(index + 1) % len(ring)][0] * ring[index][1]
            for index in range(len(ring))
        )
    ) // 2


@pytest.mark.parametrize("board_path", _CORPUS_BOARDS, ids=lambda path: path.parent.parent.name)
def test_copper_geometry_projection_matches_full_board(board_path: Path) -> None:
    assert board_path.exists(), f"missing corpus board: {board_path}"
    projected = emit_pcb_copper_geometry(KiCadPcbProjection.from_file(board_path))
    full = _full_document(str(board_path))

    assert projected.to_dict() == full.to_dict()
    assert projected.bounds_nm == full.bounds_nm
    assert sum(_polygon_area_nm2(item.outer_nm) for item in projected.features) == sum(
        _polygon_area_nm2(item.outer_nm) for item in full.features
    )


@pytest.mark.parametrize("board_path", _CORPUS_BOARDS, ids=lambda path: path.parent.parent.name)
def test_copper_geometry_matches_plotter_ir_family_identity(board_path: Path) -> None:
    """The replacement path must preserve Plotter IR copper-family identity."""
    board = _full_board(str(board_path))
    document = _full_document(str(board_path))
    ir = board.to_ir()

    record_counts: dict[str, int] = {}
    ir_net_names: set[str] = set()
    for record in ir.records:
        record_counts[record.kind] = record_counts.get(record.kind, 0) + 1
        net_name = str(record.extras.get("net_name") or "")
        if net_name:
            ir_net_names.add(net_name)

    feature_counts = {
        kind: sum(1 for feature in document.features if feature.kind == kind)
        for kind in ("track", "track_arc", "via", "pad", "zone_fill")
    }
    assert feature_counts["track"] == record_counts.get("segment", 0)
    assert feature_counts["track_arc"] == record_counts.get("track_arc", 0)
    assert feature_counts["via"] == record_counts.get("via", 0)
    assert feature_counts["zone_fill"] == sum(
        len(zone.filled_polygons) for zone in board.zones
    )
    assert feature_counts["pad"] <= sum(
        len(footprint.pads) for footprint in board.footprints
    )
    assert ir_net_names <= {net.name for net in document.nets}

    layer_count = len(document.layers)
    net_count = len(document.nets)
    assert all(
        feature.layer_indexes
        and all(0 <= index < layer_count for index in feature.layer_indexes)
        and (feature.net_index is None or 0 <= feature.net_index < net_count)
        and len(feature.outer_nm) >= 3
        for feature in document.features
    )
    assert all(
        drill.layer_indexes
        and all(0 <= index < layer_count for index in drill.layer_indexes)
        and drill.width_nm > 0
        and drill.height_nm > 0
        for drill in document.drills
    )

"""PCB graphical bounds oracle checks against KiCad's bbox exporter.

The patched KiCad CLI command ``kicad-cli pcb export bbox`` reports
``BOARD_ITEM::GetBoundingBox()`` for board drawings. These tests compare
``KiCadPcb`` graphical shape ``get_bounds()`` results against that KiCad-side
oracle in pcb internal units.
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import KiCadPcb
from kicad_monkey.testing.corpus import (
    get_kicad_common_case_dir,
    get_kicad_corpus_root,
    get_kicad_pcb_foundation_dir,
)


BBOX_SCHEMA = "kicad.pcb_bbox.v1"
IU_PER_MM = 1_000_000
PCB_SHAPE_FORMS = {
    "gr_line": "gr_lines",
    "gr_rect": "gr_rects",
    "gr_arc": "gr_arcs",
    "gr_circle": "gr_circles",
    "gr_poly": "gr_polys",
    "gr_curve": "gr_curves",
}
BBOX_KEYS = ("left_iu", "top_iu", "right_iu", "bottom_iu")
FORM_TOLERANCE_IU = {
    # KiCad and kicad_monkey both flatten Beziers before measuring bounds, but
    # their tessellation steps differ by a few microns.
    "gr_curve": 5_000,
}


@dataclass(frozen=True)
class PcbBoundsOracleCase:
    case_id: str
    board_path: Path
    required_forms: tuple[str, ...]


def _synthetic_case(case_id: str, filename: str, *forms: str) -> PcbBoundsOracleCase:
    return PcbBoundsOracleCase(
        case_id=case_id,
        board_path=get_kicad_pcb_foundation_dir() / case_id / "input" / filename,
        required_forms=forms,
    )


SYNTHETIC_BOUNDS_CASES = (
    _synthetic_case(
        "case001__track_top_1mil",
        "case001__track_top_1mil.kicad_pcb",
        "gr_line",
    ),
    _synthetic_case(
        "case005__track_top_default",
        "one_track_top_copper.kicad_pcb",
        "gr_rect",
    ),
    _synthetic_case(
        "case009__arc_silk_solid",
        "silk_arc_top.kicad_pcb",
        "gr_arc",
        "gr_rect",
    ),
    _synthetic_case(
        "case235__circle_silk",
        "silk_circle_top.kicad_pcb",
        "gr_circle",
        "gr_rect",
    ),
    _synthetic_case(
        "case237__poly_silk",
        "silk_poly_top.kicad_pcb",
        "gr_poly",
        "gr_rect",
    ),
    _synthetic_case(
        "case234__bezier_silk",
        "silk_bezier_top.kicad_pcb",
        "gr_curve",
        "gr_rect",
    ),
)


def _real_world_cases() -> tuple[PcbBoundsOracleCase, ...]:
    return (
        PcbBoundsOracleCase(
            case_id="tiny_tapeout",
            board_path=get_kicad_common_case_dir("tiny_tapeout") / "input" / "tinytapeout-demo.kicad_pcb",
            required_forms=("gr_line", "gr_rect", "gr_arc", "gr_circle", "gr_poly"),
        ),
        PcbBoundsOracleCase(
            case_id="4-ch-backplane",
            board_path=(
                get_kicad_corpus_root()
                / "projects"
                / "4-ch-backplane"
                / "input"
                / "4-ch-backplane.kicad_pcb"
            ),
            required_forms=("gr_line", "gr_arc"),
        ),
    )


@pytest.fixture(scope="module")
def kicad_bbox_cli() -> Path:
    cli = resolve_kicad_cli(required_capability="pcb_bbox")
    if cli is None:
        pytest.skip("kicad-cli with 'pcb export bbox' not found")
    return cli


def _export_bbox_json(
    *,
    kicad_cli: Path,
    board_path: Path,
    output_dir: Path,
) -> dict[str, Any]:
    output_path = output_dir / f"{board_path.stem}.bbox.json"
    result = subprocess.run(
        [
            str(kicad_cli),
            "pcb",
            "export",
            "bbox",
            "--output",
            str(output_path),
            str(board_path),
        ],
        capture_output=True,
        text=True,
        env=kicad_cli_subprocess_env(kicad_cli),
        timeout=120,
    )
    assert result.returncode == 0, (
        f"kicad-cli pcb export bbox failed for {board_path}\n"
        f"stdout:\n{result.stdout}\n"
        f"stderr:\n{result.stderr}"
    )

    data = json.loads(output_path.read_text(encoding="utf-8"))
    assert data["schema"] == BBOX_SCHEMA
    return data


def _to_iu(mm: float) -> int:
    return int(round(mm * IU_PER_MM))


def _shape_bounds_by_uuid(pcb: KiCadPcb) -> dict[str, tuple[str, dict[str, int]]]:
    by_uuid: dict[str, tuple[str, dict[str, int]]] = {}

    for form, collection_name in PCB_SHAPE_FORMS.items():
        for shape in getattr(pcb, collection_name):
            uuid = getattr(shape, "uuid", None)
            if not uuid:
                continue

            bounds = shape.get_bounds()
            assert bounds.is_valid(), f"{form} {uuid} returned invalid bounds"
            by_uuid[str(uuid)] = (
                form,
                {
                    "left_iu": _to_iu(bounds.min_x),
                    "top_iu": _to_iu(bounds.min_y),
                    "right_iu": _to_iu(bounds.max_x),
                    "bottom_iu": _to_iu(bounds.max_y),
                },
            )

    return by_uuid


def _oracle_shape_records(data: dict[str, Any]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []

    for record in data.get("drawings", []):
        if not isinstance(record, dict):
            continue
        if record.get("class") != "PCB_SHAPE":
            continue
        if record.get("kicad_form") not in PCB_SHAPE_FORMS:
            continue
        records.append(record)

    return records


def _assert_case_matches_kicad_bbox_oracle(
    *,
    case: PcbBoundsOracleCase,
    kicad_cli: Path,
    tmp_path: Path,
) -> None:
    assert case.board_path.exists(), f"missing PCB fixture: {case.board_path}"

    pcb = KiCadPcb.from_file(case.board_path)
    python_shapes = _shape_bounds_by_uuid(pcb)
    oracle_data = _export_bbox_json(
        kicad_cli=kicad_cli,
        board_path=case.board_path,
        output_dir=tmp_path,
    )
    oracle_records = _oracle_shape_records(oracle_data)
    assert oracle_records, f"{case.case_id}: KiCad bbox oracle returned no PCB_SHAPE records"

    observed_forms = {str(record["kicad_form"]) for record in oracle_records}
    missing_forms = set(case.required_forms) - observed_forms
    assert not missing_forms, f"{case.case_id}: missing oracle forms: {sorted(missing_forms)}"

    for record in oracle_records:
        uuid = str(record.get("uuid") or "")
        oracle_form = str(record["kicad_form"])
        assert uuid in python_shapes, f"{case.case_id}: missing Python shape for {oracle_form} uuid={uuid}"

        python_form, python_bbox = python_shapes[uuid]
        assert python_form == oracle_form, (
            f"{case.case_id}: form mismatch for uuid={uuid}: "
            f"Python={python_form}, KiCad={oracle_form}"
        )

        oracle_bbox = {key: int(record["bbox"][key]) for key in BBOX_KEYS}
        tolerance = FORM_TOLERANCE_IU.get(oracle_form, 0)
        deltas = {
            key: python_bbox[key] - oracle_bbox[key]
            for key in BBOX_KEYS
            if abs(python_bbox[key] - oracle_bbox[key]) > tolerance
        }
        assert not deltas, (
            f"{case.case_id}: bbox mismatch for {oracle_form} uuid={uuid}; "
            f"tolerance={tolerance} IU, deltas={deltas}, "
            f"Python={python_bbox}, KiCad={oracle_bbox}"
        )


@pytest.mark.parametrize("case", SYNTHETIC_BOUNDS_CASES, ids=lambda case: case.case_id)
def test_synthetic_pcb_shape_bounds_match_kicad_bbox_oracle(
    case: PcbBoundsOracleCase,
    kicad_bbox_cli: Path,
    tmp_path: Path,
) -> None:
    _assert_case_matches_kicad_bbox_oracle(
        case=case,
        kicad_cli=kicad_bbox_cli,
        tmp_path=tmp_path,
    )


@pytest.mark.parametrize("case", _real_world_cases(), ids=lambda case: case.case_id)
def test_real_world_pcb_shape_bounds_match_kicad_bbox_oracle(
    case: PcbBoundsOracleCase,
    kicad_bbox_cli: Path,
    tmp_path: Path,
) -> None:
    if not case.board_path.exists():
        pytest.skip(f"real-world PCB fixture not present: {case.board_path}")

    _assert_case_matches_kicad_bbox_oracle(
        case=case,
        kicad_cli=kicad_bbox_cli,
        tmp_path=tmp_path,
    )

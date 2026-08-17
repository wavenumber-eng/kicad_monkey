r"""Generate synthetic KiCad PCB fixtures exercising dimension annotations.

One board per dimension flavor — kept tight so the IR-vs-kicad-cli oracle
in ``L3_rendering/test_L3_007_pcb_ir_svg_oracle.py`` can pin geometry
parity per case.

Layout (manifest-friendly, post-2026-05-17 pcb_foundation migration):

    kicad/pcb_foundation/<case_id>/input/<case_id>.kicad_pcb
    kicad/pcb_foundation/<case_id>/input/<case_id>.kicad_pro
    kicad/pcb_foundation/<case_id>/input/<case_id>.kicad_sch
    kicad/pcb_foundation/<case_id>/input/case_metadata.json

Cases:

* ``dim_aligned_horizontal``    — aligned dim along +X
* ``dim_orthogonal_horizontal`` — orthogonal dim, orientation=0
* ``dim_orthogonal_vertical``   — orthogonal dim, orientation=1
* ``dim_leader_plain``          — leader (no text frame)
* ``dim_leader_frame_rect``     — leader with text_frame=1 (rectangle frame)
* ``dim_radial``                — radial dim with leader knee
* ``dim_center``                — center crosshair

All dimensions land on ``Cmts.User`` so the test layer set is a single
non-copper, non-mask user layer (no drill synthesis, no mask expansion).
"""

from __future__ import annotations

import argparse
import json
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Tuple

from kicad_monkey.kicad_base import LayerType, Stroke, StrokeType
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_pcb_gr_line import GrLine
from kicad_monkey.kicad_pcb_gr_text import Effects as GrTextEffects
from kicad_monkey.kicad_pcb_gr_text import Font as GrTextFont
from kicad_monkey.kicad_pcb_gr_text import GrText
from kicad_monkey.kicad_pcb_other import (
    Dimension,
    DimensionFormat,
    DimensionStyle,
    Layer,
    Net,
)


BOARD_VERSION = 20241229
GENERATOR_VERSION = "9.0"
NAMESPACE = uuid.uuid5(uuid.NAMESPACE_URL, "wavenumber/kicad/synthetic-dimensions")


def _uid(*parts: object) -> str:
    return str(uuid.uuid5(NAMESPACE, "/".join(str(p) for p in parts)))


def _q(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _kicad_layers() -> list[Layer]:
    return [
        Layer(0, "F.Cu", LayerType.SIGNAL),
        Layer(31, "B.Cu", LayerType.SIGNAL),
        Layer(34, "B.Paste", LayerType.USER),
        Layer(35, "F.Paste", LayerType.USER),
        Layer(36, "B.SilkS", LayerType.USER, "B.Silkscreen"),
        Layer(37, "F.SilkS", LayerType.USER, "F.Silkscreen"),
        Layer(38, "B.Mask", LayerType.USER),
        Layer(39, "F.Mask", LayerType.USER),
        Layer(40, "Dwgs.User", LayerType.USER, "User.Drawings"),
        Layer(41, "Cmts.User", LayerType.USER, "User.Comments"),
        Layer(44, "Edge.Cuts", LayerType.USER),
        Layer(45, "Margin", LayerType.USER),
        Layer(46, "B.CrtYd", LayerType.USER, "B.Courtyard"),
        Layer(47, "F.CrtYd", LayerType.USER, "F.Courtyard"),
        Layer(48, "B.Fab", LayerType.USER),
        Layer(49, "F.Fab", LayerType.USER),
    ]


def _setup_sexp() -> list[object]:
    return [
        "setup",
        ["pad_to_mask_clearance", 0.0],
        ["allow_soldermask_bridges_in_footprints", "no"],
    ]


def _outline(width: float, height: float, case_id: str) -> list[GrLine]:
    corners = ((0.0, 0.0), (width, 0.0), (width, height), (0.0, height))
    out: list[GrLine] = []
    for i, (start, end) in enumerate(zip(corners, corners[1:] + corners[:1]), start=1):
        out.append(
            GrLine(
                start_x=start[0],
                start_y=start[1],
                end_x=end[0],
                end_y=end[1],
                layer="Edge.Cuts",
                stroke=Stroke(width=0.05, type=StrokeType.DEFAULT),
                uuid=_uid(case_id, "outline", i),
            )
        )
    return out


def _gr_text(
    *,
    case_id: str,
    text: str,
    x: float,
    y: float,
    angle: float = 0.0,
    layer: str = "Cmts.User",
    size: float = 1.0,
    thickness: float = 0.15,
) -> GrText:
    return GrText(
        text=text,
        at_x=x,
        at_y=y,
        at_angle=angle,
        layer=layer,
        uuid=_uid(case_id, "gr_text"),
        effects=GrTextEffects(
            font=GrTextFont(size_x=size, size_y=size, thickness=thickness),
        ),
    )


def _dim_format(
    precision: int = 4,
    *,
    units_format: int = 1,
    override_value: str | None = None,
) -> DimensionFormat:
    return DimensionFormat(
        prefix="",
        suffix="",
        units=3,           # automatic (mm for current corpus)
        units_format=units_format,
        precision=precision,
        override_value=override_value,
    )


def _dim_style(*, dim_type: str, text_frame: int | None = None) -> DimensionStyle:
    return DimensionStyle(
        thickness=0.2,
        arrow_length=1.27,
        text_position_mode=0,
        arrow_direction="outward",
        extension_height=0.58642,
        extension_offset=0.0,
        keep_text_aligned=True,
        text_frame=text_frame,
    )


@dataclass(frozen=True)
class DimensionCase:
    case_id: str
    board_size: Tuple[float, float]
    builder: Callable[[str], Dimension]
    description: str


def _build_aligned_horizontal(case_id: str) -> Dimension:
    points = [(5.0, 10.0), (25.0, 10.0)]  # 20 mm horizontal
    height = 5.0                          # offset above the line
    text = _gr_text(case_id=case_id, text="20 mm", x=15.0, y=4.5)
    return Dimension(
        dimension_type="aligned",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        height=height,
        format=_dim_format(),
        style=_dim_style(dim_type="aligned"),
        gr_text=text,
    )


def _build_orthogonal_horizontal(case_id: str) -> Dimension:
    points = [(5.0, 10.0), (25.0, 18.0)]
    height = 8.0
    text = _gr_text(case_id=case_id, text="20 mm", x=15.0, y=20.0)
    return Dimension(
        dimension_type="orthogonal",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        height=height,
        orientation=0,
        format=_dim_format(),
        style=_dim_style(dim_type="orthogonal"),
        gr_text=text,
    )


def _build_orthogonal_vertical(case_id: str) -> Dimension:
    points = [(5.0, 5.0), (15.0, 25.0)]
    height = 12.0
    text = _gr_text(case_id=case_id, text="20 mm", x=22.0, y=15.0)
    return Dimension(
        dimension_type="orthogonal",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        height=height,
        orientation=1,
        format=_dim_format(),
        style=_dim_style(dim_type="orthogonal"),
        gr_text=text,
    )


def _build_leader_plain(case_id: str) -> Dimension:
    points = [(8.0, 8.0), (20.0, 16.0)]
    # KiCad CLI uses ``format.override_value`` as the leader text and ignores
    # the embedded ``gr_text`` content for leaders. ``units_format=0``
    # suppresses the " mm" suffix so the rendered text is exactly the label.
    text = _gr_text(case_id=case_id, text="NOTE", x=24.0, y=16.0)
    return Dimension(
        dimension_type="leader",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        height=0.0,
        format=_dim_format(units_format=0, override_value="NOTE"),
        style=_dim_style(dim_type="leader", text_frame=0),
        gr_text=text,
    )


def _build_leader_frame_rect(case_id: str) -> Dimension:
    points = [(8.0, 8.0), (20.0, 16.0)]
    text = _gr_text(case_id=case_id, text="A1", x=24.0, y=16.0)
    return Dimension(
        dimension_type="leader",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        height=0.0,
        format=_dim_format(units_format=0, override_value="A1"),
        style=_dim_style(dim_type="leader", text_frame=1),  # rectangle
        gr_text=text,
    )


def _build_radial(case_id: str) -> Dimension:
    points = [(15.0, 15.0), (22.0, 15.0)]
    text = _gr_text(case_id=case_id, text="R7", x=27.0, y=15.0)
    return Dimension(
        dimension_type="radial",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        leader_length=3.0,
        format=_dim_format(),
        style=_dim_style(dim_type="radial"),
        gr_text=text,
    )


def _build_center(case_id: str) -> Dimension:
    # Center dimension only needs two points: center + arm end.
    points = [(15.0, 15.0), (18.0, 15.0)]
    return Dimension(
        dimension_type="center",
        layer="Cmts.User",
        uuid=_uid(case_id, "dimension"),
        points=points,
        format=_dim_format(),
        style=_dim_style(dim_type="center"),
        gr_text=None,  # center has no text
    )


CASES: tuple[DimensionCase, ...] = (
    DimensionCase("dim_aligned_horizontal", (30.0, 20.0), _build_aligned_horizontal,
                  "Aligned dimension along +X with crossbar above."),
    DimensionCase("dim_orthogonal_horizontal", (30.0, 30.0), _build_orthogonal_horizontal,
                  "Orthogonal dimension, orientation=0 (horizontal arm)."),
    DimensionCase("dim_orthogonal_vertical", (30.0, 30.0), _build_orthogonal_vertical,
                  "Orthogonal dimension, orientation=1 (vertical arm)."),
    DimensionCase("dim_leader_plain", (30.0, 25.0), _build_leader_plain,
                  "Leader dimension, no text frame."),
    DimensionCase("dim_leader_frame_rect", (30.0, 25.0), _build_leader_frame_rect,
                  "Leader dimension with text_frame=1 (rectangle frame)."),
    DimensionCase("dim_radial", (35.0, 30.0), _build_radial,
                  "Radial dimension with leader knee."),
    DimensionCase("dim_center", (30.0, 30.0), _build_center,
                  "Center crosshair dimension."),
)


def build_dimension_board(case: DimensionCase) -> KiCadPcb:
    width, height = case.board_size
    pcb = KiCadPcb()
    pcb.version = BOARD_VERSION
    pcb.generator = "generate_kicad_synthetic_dimensions.py"
    pcb.generator_version = GENERATOR_VERSION
    pcb.thickness = 1.6
    pcb.paper = "A4"
    pcb.layers = _kicad_layers()
    pcb.setup_sexp = _setup_sexp()
    pcb.nets = [Net(0, "")]
    pcb.gr_lines = _outline(width, height, case.case_id)
    pcb.dimensions = [case.builder(case.case_id)]
    return pcb


def _minimal_project_json(project_name: str) -> str:
    data = {
        "board": {
            "design_settings": {
                "defaults": {
                    "board_outline_line_width": 0.1,
                    "copper_line_width": 0.2,
                    "silk_line_width": 0.12,
                }
            }
        },
        "meta": {"filename": f"{project_name}.kicad_pro", "version": 1},
        "net_settings": {
            "classes": [{"name": "Default", "clearance": 0.2, "track_width": 0.25}],
            "meta": {"version": 3},
            "net_colors": {},
            "netclass_assignments": {},
            "netclass_patterns": [],
        },
        "project": {"files": []},
        "text_variables": {"SYNTHETIC_FIXTURE": project_name},
    }
    return json.dumps(data, indent=2, sort_keys=True) + "\n"


def _minimal_schematic_text(project_name: str) -> str:
    return (
        f"(kicad_sch\n"
        f"  (version 20250114)\n"
        f"  (generator {_q('generate_kicad_synthetic_dimensions.py')})\n"
        f"  (generator_version {_q(GENERATOR_VERSION)})\n"
        f"  (uuid {_q(_uid(project_name, 'schematic'))})\n"
        f"  (paper {_q('A4')})\n"
        f"  (lib_symbols)\n"
        f"  (sheet_instances\n"
        f"    (path {_q('/')}\n"
        f"      (page {_q('1')})\n"
        f"    )\n"
        f"  )\n"
        f"  (embedded_fonts no)\n"
        f")\n"
    )


def _case_metadata(case: DimensionCase) -> dict[str, object]:
    return {
        "origin": "synthetic",
        "status": "active",
        "domains": ["board_svg", "pcb_ir", "pcb_data_models"],
        "tags": ["synthetic", "data_models", "board_svg", "dimensions",
                 "focused_feature_case"],
        "test_intent": (
            f"Synthetic dimension fixture: {case.description} "
            "Used by L3_007 IR-vs-kicad-cli oracle to pin per-type "
            "dimension geometry parity."
        ),
        "feature_coverage": {
            "pcb": ["dimensions"],
            "board_svg": ["dimension_rendering"],
        },
        "oracle_policy": {"board_svg": "smoke", "pcb_ir": "smoke"},
        "provenance": {
            "source_kind": "synthetic",
            "source_path": None,
            "license_usage": "test_fixture",
            "generator": "kicad_monkey/scripts/generate_kicad_synthetic_dimensions.py",
        },
        "notes": [
            f"Generated dimension case: {case.case_id}.",
            "Regenerate instead of hand-editing the .kicad_pcb or metadata.",
        ],
    }


def _write_case(corpus_root: Path, case: DimensionCase, *, force: bool, dry_run: bool) -> list[Path]:
    case_dir = corpus_root / "kicad" / "pcb_foundation" / case.case_id / "input"
    board_path = case_dir / f"{case.case_id}.kicad_pcb"
    project_path = case_dir / f"{case.case_id}.kicad_pro"
    schematic_path = case_dir / f"{case.case_id}.kicad_sch"
    metadata_path = case_dir / "case_metadata.json"

    files = [
        (board_path, build_dimension_board(case).to_string()),
        (project_path, _minimal_project_json(case.case_id)),
        (schematic_path, _minimal_schematic_text(case.case_id)),
        (metadata_path, json.dumps(_case_metadata(case), indent=2, sort_keys=True) + "\n"),
    ]

    existing = [p for p, _ in files if p.exists()]
    if existing and not force:
        joined = "\n".join(str(p) for p in existing)
        raise FileExistsError(f"refusing to overwrite without --force:\n{joined}")

    if dry_run:
        return [p for p, _ in files]

    for path, text in files:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")
    return [p for p, _ in files]


def main(argv: list[str] | None = None) -> int:
    repo_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--corpus-root",
        type=Path,
        default=repo_root / "tests" / "corpus",
        help="Writable authoring root containing the kicad corpus folder.",
    )
    parser.add_argument(
        "--cases",
        nargs="*",
        default=None,
        help="Subset of case ids to write (defaults to all).",
    )
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    selected = CASES
    if args.cases:
        wanted = set(args.cases)
        unknown = wanted - {c.case_id for c in CASES}
        if unknown:
            print(f"unknown cases: {sorted(unknown)}", file=sys.stderr)
            return 2
        selected = tuple(c for c in CASES if c.case_id in wanted)

    for case in selected:
        paths = _write_case(args.corpus_root, case, force=args.force, dry_run=args.dry_run)
        action = "Would write" if args.dry_run else "Wrote"
        for path in paths:
            print(f"{action} {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

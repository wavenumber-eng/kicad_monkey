"""Regenerate the bounded board plotter parity vectors from Python."""

from __future__ import annotations

import argparse
from contextlib import contextmanager, nullcontext
import json
import sys
from pathlib import Path
from typing import Any, Iterator

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src" / "py"))

from kicad_monkey.kicad_pcb import KiCadPcb  # noqa: E402
from kicad_monkey.kicad_pcb_to_ir import pcb_to_ir  # noqa: E402
from kicad_monkey.kicad_project import KiCadProject  # noqa: E402

VECTOR_PATH = ROOT / "tests" / "parity" / "board_plotter_a0_vectors.json"

# Keys whose values the Rust serializer emits as floats.
FLOAT_KEYS = {"thickness_mm", "drill", "size", "orient_deg", "angle"}


@contextmanager
def without_shapely() -> Iterator[None]:
    """Pin the governed no-Shapely synthetic-knockout baseline."""
    saved = {
        name: sys.modules.pop(name)
        for name in list(sys.modules)
        if name == "shapely" or name.startswith("shapely.")
    }
    sys.modules["shapely"] = None
    try:
        yield
    finally:
        del sys.modules["shapely"]
        sys.modules.update(saved)


def norm(value: Any, key: str | None = None) -> Any:
    if isinstance(value, dict):
        return {k: norm(v, k) for k, v in value.items()}
    if isinstance(value, list):
        return [norm(v, key) for v in value]
    if isinstance(value, float) and key not in FLOAT_KEYS and value.is_integer():
        return int(value)
    return value


def expected_for(vector: dict[str, Any]) -> dict[str, Any]:
    pcb = KiCadPcb.from_string(vector["source"])
    project_raw = {}
    if vector.get("net_class_assignments") is not None:
        project_raw["net_settings"] = {
            "netclass_assignments": vector["net_class_assignments"]
        }
    if vector.get("text_variables") is not None:
        project_raw["text_variables"] = vector["text_variables"]
    if project_raw:
        pcb.project = KiCadProject.from_json_dict(project_raw)
    oracle_mode = vector.get("oracle_mode")
    if oracle_mode not in (None, "without_shapely"):
        raise ValueError(f"unknown oracle mode: {oracle_mode}")
    oracle = without_shapely() if oracle_mode == "without_shapely" else nullcontext()
    with oracle:
        document = pcb_to_ir(
            pcb,
            source_path=vector["source_path"],
            document_id=vector["document_id"],
        ).to_dict()
    return norm(document)


HEADER = """(kicad_pcb
  (version 20240108)
  (generator pcbnew)
  (generator_version "8.0")
  (general (thickness 1.6))
  (paper "A4")
"""

TEXT_SOURCE = (
    HEADER
    + """  (property "Revision" "rev-C")
  (gr_text "plain" (at 1 2) (layer "F.SilkS") (uuid "text-plain"))
  (gr_text "styled" (at 3 4 90) (layer "B.SilkS") (effects (font (size 2 1.5) (thickness 0.3) bold (italic yes)) (justify right top mirror)) (uuid "text-styled"))
  (gr_text "" (at 0 0) (uuid "text-empty"))
  (gr_text "Rev ${Revision} of ${project} ${TITLE}" (at 5 6) (uuid "text-vars"))
  (gr_text "cached" (at 7 8 45) (effects (font (size 1 1))) (render_cache "cached" 45 (polygon (pts (xy 7 8) (xy 8 8) (xy 8 9)))) (uuid "text-cache"))
  (gr_text "cached" (at 7 8) (render_cache "stale" 0 (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))) (uuid "text-cache-stale"))
  (gr_text "KO" (at 1 1) (layer "F.SilkS" knockout) (effects (font (size 1.8 1.8) (thickness 0.2))) (render_cache "KO" 0 (polygon (pts (xy 0.5 0.5) (xy 1.5 0.5) (xy 1.5 1.2)) (pts (xy 0.7 0.7) (xy 1.2 0.7) (xy 1.2 1)))) (uuid "text-knockout"))
  (gr_text "faced" (at 2 3) (effects (font (face "Arial") (size 1.27 1.27))) (render_cache "faced" 0 (polygon (pts (xy 2 3) (xy 3 3) (xy 3 4)))) (uuid "text-face-cache"))
  (gr_text "colored" (at 0 0) (effects (font (color 10 20 30 0.3))) (uuid "text-color-ignored"))
)"""
)

TEXT_BOX_SOURCE = (
    HEADER
    + """  (gr_text_box "" (start 0 0) (end 10 5) (border yes) (stroke (width -1) (type solid)) (layer "F.SilkS") (uuid "tb-neg"))
  (gr_text_box "zero" (start 0 0) (end 4 2) (border yes) (layer "Cmts.User") (uuid "tb-zero"))
  (gr_text_box "thin" (start 0 0) (end 4 2) (border yes) (stroke (width 0.05) (type dash)) (uuid "tb-thin"))
  (gr_text_box "RB" (start 0 0) (end 6 3) (margins 0.5 0.25 0.75 0.4) (effects (justify right bottom)) (uuid "tb-margins-rb"))
  (gr_text_box "LT" (start 2 1) (end 8 4) (margins 0.5 0.25 0.75 0.4) (effects (justify left top)) (uuid "tb-margins-lt"))
  (gr_text_box "rot" (start 0 0) (end 3 2) (angle 90) (uuid "tb-angle"))
  (gr_text_box "mirrored" (start 0 0) (end 3 2) (effects (justify mirror)) (uuid "tb-mirror"))
  (gr_text_box "L1\nL2" (start 0 0) (end 5 3) (uuid "tb-multiline"))
  (gr_text_box "pts" (pts (xy 1 1) (xy 6 1) (xy 6 4) (xy 1 4)) (uuid "tb-pts"))
  (gr_text_box "KOB" (start 0 0) (end 4 2) (knockout yes) (effects (font (size 1 1) (thickness 0.2))) (render_cache "KOB" 0 (polygon (pts (xy 1 1) (xy 3 1) (xy 3 1.5)))) (uuid "tb-knockout-cache"))
  (gr_text_box "A A" (start 0 0) (end 5.2 2) (effects (font (size 1 2.1))) (uuid "tb-wrap-exact"))
  (gr_text_box "A A" (start 0 0) (end 5.19 2) (effects (font (size 1 2.1))) (uuid "tb-wrap-over"))
  (gr_text_box "A A\nA A" (start 0 0) (end 5.19 3) (effects (font (size 1 2.1))) (uuid "tb-wrap-paragraphs"))
  (gr_text_box "A A" (start 0 0) (end 6.19 2) (margins 0.5 0 0.5 0) (effects (font (size 1 2.1))) (uuid "tb-wrap-margins"))
  (gr_text_box "A A" (start 0 0) (end 5.19 2) (angle 90) (effects (font (size 1 2.1))) (uuid "tb-wrap-angle"))
  (gr_text_box "${WRAP}" (start 0 0) (end 5.19 2) (effects (font (size 1 2.1))) (uuid "tb-wrap-variable"))
  (gr_text_box "A _{A A}" (start 0 0) (end 7 2) (effects (font (size 1 2.1))) (uuid "tb-wrap-markup"))
  (gr_text_box "opaque" (start 0 0) (end 20 2) (effects (font (color 10 20 30 1))) (uuid "tb-color-opaque"))
  (gr_text_box "fractional" (start 0 0) (end 20 2) (effects (font (color 40 50 60 0.3))) (uuid "tb-color-fractional"))
  (gr_text_box "clear" (start 0 0) (end 20 2) (effects (font (color 70 80 90 0))) (uuid "tb-color-clear"))
)"""
)

TABLE_SOURCE = (
    HEADER
    + """  (property "PROJECT" "board-project")
  (via (at -1 -1) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (uuid "via-before-table"))
  (image (at 1 1) (layer "F.SilkS") (data "YWJj") (uuid "ignored-image"))
  (barcode (at 2 2) (layer "F.SilkS") (size 4 2) (text "IGNORED") (uuid "ignored-barcode"))
  (group "Ignored" (id "ignored-group") (members "table-grid"))
  (generated (id "ignored-generated") (type tuned_delay) (name "Ignored") (layer "F.Cu"))
  (table (column_count 2) (layer "Dwgs.User")
    (border (external yes) (header yes) (stroke (width 0.3) (type dash) (color 1 2 3 1)))
    (separators (rows yes) (cols yes) (stroke (width 0.1) (type dot)))
    (cells
      (table_cell "plain" (start 0 0) (end 10 5) (layer "Dwgs.User"))
      (table_cell "silent-cache" (start 10 0) (end 20 5) (layer "B.SilkS")
        (effects (font (size 1 1)))
        (render_cache "silent-cache" 0 (polygon (pts (xy 10 1) (xy 11 1) (xy 11 2)))))
      (table_cell "reversed" (start 10 10) (end 0 5) (span 2 1) (layer "Dwgs.User"))
      (table_cell "${ADDR}-${ROW}-${COL}-${LAYER}-${PROJECT}" (start 20 10) (end 10 5)
        (margins 0.5 0.25 0.75 0.4) (angle 90) (layer "F.SilkS")
        (effects (font (face "Arial") (size 1.2 1.5) (thickness 0.2) bold italic (color 10 20 30 0.3))
          (justify right top mirror))
        (render_cache "B2-2-2-F.SilkS-board-project" 37
          (polygon (pts (xy 12 6) (xy 14 6) (xy 14 7)))))
    )
    (uuid "table-grid"))
  (table (column_count 1) (layer "F.Cu") (border (external no)) (separators (rows no) (cols no)) (uuid "table-empty"))
  (table (column_count 0) (layer "Cmts.User")
    (border (external no)) (separators (rows no) (cols no))
    (cells
      (table_cell "${EMPTY}" (start 30 0) (end 40 5) (layer "Cmts.User")
        (effects (font (face "Arial") (size 1 1))))
      (table_cell "${EMPTY}" (start 30 5) (end 40 10) (layer "Cmts.User")
        (effects (font (face "Arial") (size 1 1)))
        (render_cache "stale" 0
          (polygon (pts (xy 30 5) (xy 31 5) (xy 31 6)))))
      (table_cell "${EMPTY}" (start 30 10) (end 40 15) (layer "Cmts.User")
        (effects (font (face "Arial") (size 1 1)))
        (render_cache "" 37
          (polygon (pts (xy 30 10) (xy 31 10) (xy 31 11)))))
      (table_cell "${ROW}-${COL}-${ADDR}" (start 30 15) (end 40 20) (layer "Cmts.User")
        (effects (font (face "Arial") (size 1 1)))
        (render_cache "${ROW}-${COL}-${ADDR}" 0
          (polygon (pts (xy 30 15) (xy 31 15) (xy 31 16))))))
    (uuid "table-empty-resolved"))
  (zone (net 0) (net_name "") (layers "F.Cu")
    (filled_polygon (layer "F.Cu") (pts (xy 0 20) (xy 2 20) (xy 2 22)))
    (uuid "zone-after-table"))
)"""
)

DIMENSION_SOURCE = (
    HEADER
    + """  (table (column_count 0) (layer "Dwgs.User")
    (border (external no)) (separators (rows no) (cols no)) (uuid "table-before-dimensions"))
  (dimension (type aligned) (layer "Dwgs.User") (pts (xy 0 0) (xy 10 0)) (height 3)
    (format (override_value "1") (units_format 0))
    (style (thickness 0.2) (arrow_length 1) (arrow_direction outward) (extension_height 0.5))
    (gr_text "authored" (at 5 3) (layer "F.SilkS") (effects (font (size 1 1))) (uuid "aligned-text"))
    (uuid "dimension-aligned"))
  (dimension (type orthogonal) (layer "Cmts.User") (pts (xy 0 0) (xy 5 2)) (height 2) (orientation 0)
    (style (thickness 0.15) (arrow_length 0.8) (arrow_direction inward) (extension_height 0.3))
    (uuid "dimension-orthogonal"))
  (dimension (type radial) (layer "Dwgs.User") (pts (xy 20 0) (xy 22 0)) (leader_length 2)
    (format (override_value "R") (units_format 0))
    (style (thickness 0.1) (arrow_length 0.5) (keep_text_aligned no))
    (gr_text "authored" (at 25 0) (layer "B.SilkS")
      (effects (font (face "Arial") (size 1 1)))
      (render_cache "R" 0 (polygon (pts (xy 24.5 -0.5) (xy 25.5 -0.5) (xy 25.5 0.5))))
      (uuid "radial-text"))
    (uuid "dimension-radial"))
  (dimension (type leader) (layer "Cmts.User") (pts (xy 30 0) (xy 32 1))
    (format (override_value "L") (units_format 0))
    (style (thickness 0.1) (arrow_length 0.5) (extension_offset 0.2) (text_frame 1))
    (gr_text "authored" (at 35 1 30) (effects (font (size 1 1))) (uuid "leader-text"))
    (uuid "dimension-leader"))
  (dimension (type center) (layer "Dwgs.User") (pts (xy 40 0) (xy 41 0))
    (format (override_value "A_{B}^{C}~{D}") (units_format 0))
    (style (thickness 0.1) (arrow_length 0.5))
    (gr_text "authored" (at 40 0) (effects (font (size 1 1))) (uuid "center-text"))
    (uuid "dimension-center"))
  (zone (net 0) (net_name "") (layers "F.Cu")
    (filled_polygon (layer "F.Cu") (pts (xy 0 20) (xy 2 20) (xy 2 22)))
    (uuid "zone-after-dimensions"))
)"""
)

NEW_VECTORS = [
    {
        "id": "board-text-follows-python-serializer",
        "source": TEXT_SOURCE,
        "source_path": "boards/text.kicad_pcb",
        "document_id": "board-text",
        "text_variables": {"Revision": "rev-B", "PROJECT": "monkey"},
    },
    {
        "id": "text-boxes-bundle-border-and-alignment",
        "source": TEXT_BOX_SOURCE,
        "source_path": "boards/text_boxes.kicad_pcb",
        "document_id": "board-text-boxes",
        "text_variables": {"WRAP": "A A"},
        "oracle_mode": "without_shapely",
    },
    {
        "id": "tables-precede-zones-and-bound-cached-cells",
        "source": TABLE_SOURCE,
        "source_path": "boards/tables.kicad_pcb",
        "document_id": "board-tables",
        "text_variables": {"PROJECT": "sidecar-project", "EMPTY": ""},
    },
    {
        "id": "dimensions-follow-tables-and-precede-zones",
        "source": DIMENSION_SOURCE,
        "source_path": "boards/dimensions.kicad_pcb",
        "document_id": "board-dimensions",
    },
]


def generate_vectors() -> dict[str, Any]:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    drift: list[str] = []
    new_ids = {str(vector["id"]) for vector in NEW_VECTORS}
    vectors = [vector for vector in payload["vectors"] if vector["id"] not in new_ids]
    for vector in vectors:
        regenerated = expected_for(vector)
        if regenerated != vector["expected"]:
            drift.append(vector["id"])
        vector["expected"] = regenerated
    if drift:
        raise SystemExit(f"existing vectors drifted: {drift}")
    for vector in NEW_VECTORS:
        vector = dict(vector)
        vector["expected"] = expected_for(vector)
        vectors.append(vector)
    payload["vectors"] = vectors
    return payload


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    payload = generate_vectors()
    encoded = (json.dumps(payload, indent=1) + "\n").encode()
    if args.check:
        if VECTOR_PATH.read_bytes() != encoded:
            raise SystemExit(f"stale board plotter vectors: {VECTOR_PATH}")
        return
    VECTOR_PATH.write_bytes(encoded)
    print("ok:", [vector["id"] for vector in payload["vectors"]])


if __name__ == "__main__":
    main()

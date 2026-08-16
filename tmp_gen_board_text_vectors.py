"""Temp: regenerate board plotter parity vectors + add slice-4 text vectors."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "src" / "py"))

from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_pcb_to_ir import pcb_to_ir
from kicad_monkey.kicad_project import KiCadProject

VECTOR_PATH = ROOT / "tests" / "parity" / "board_plotter_a0_vectors.json"

# Keys whose values the Rust serializer emits as floats.
FLOAT_KEYS = {"thickness_mm", "drill", "size", "orient_deg", "angle"}


class BlockShapely:
    """Pin the no-Shapely synthetic-knockout baseline during oracle runs."""

    def __enter__(self):
        self.saved = {
            name: sys.modules.pop(name)
            for name in list(sys.modules)
            if name == "shapely" or name.startswith("shapely.")
        }
        sys.modules["shapely"] = None
        return self

    def __exit__(self, *exc):
        del sys.modules["shapely"]
        sys.modules.update(self.saved)
        return False


def norm(value, key=None):
    if isinstance(value, dict):
        return {k: norm(v, k) for k, v in value.items()}
    if isinstance(value, list):
        return [norm(v, key) for v in value]
    if isinstance(value, float) and key not in FLOAT_KEYS and value.is_integer():
        return int(value)
    return value


def expected_for(vector):
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
    with BlockShapely():
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
    },
]


def main() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    drift = []
    vectors = [v for v in payload["vectors"] if v["id"] not in {n["id"] for n in NEW_VECTORS}]
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
    with VECTOR_PATH.open("w", encoding="utf-8", newline="\n") as handle:
        json.dump(payload, handle, indent=1)
        handle.write("\n")
    print("ok:", [v["id"] for v in vectors])


if __name__ == "__main__":
    main()

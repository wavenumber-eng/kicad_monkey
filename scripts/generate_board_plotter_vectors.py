"""Regenerate the bounded board plotter parity vectors from Python."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
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
    with without_shapely():
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

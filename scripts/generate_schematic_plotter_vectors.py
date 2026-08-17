"""Regenerate the bounded schematic-foundation plotter parity vectors."""

from __future__ import annotations

import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import sys
from typing import Any
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src" / "py"))

from kicad_monkey import KiCadSchematic, schematic_to_ir  # noqa: E402
from kicad_monkey.kicad_drawing_sheet import (  # noqa: E402
    load_default_drawing_sheet,
)
from kicad_monkey.kicad_worksheet import KiCadWorksheet  # noqa: E402
from kicad_monkey import kicad_schematic_to_ir as schematic_ir  # noqa: E402


VECTOR_PATH = ROOT / "tests" / "parity" / "schematic_plotter_a0_vectors.json"

# Generated Rust bindings retain these TypeSpec float64 fields as JSON numbers.
FLOAT_KEYS = {
    "angle_deg",
    "orient_deg",
    "paper_height_mm",
    "paper_width_mm",
    "scale",
}


def norm(value: Any, key: str | None = None) -> Any:
    if isinstance(value, dict):
        return {str(name): norm(child, str(name)) for name, child in value.items()}
    if isinstance(value, list):
        return [norm(child, key) for child in value]
    if isinstance(value, float) and key not in FLOAT_KEYS and value.is_integer():
        return int(value)
    return value


def _worksheet(vector: dict[str, Any]) -> KiCadWorksheet:
    source = vector.get("worksheet_source")
    if source is None:
        return load_default_drawing_sheet()
    return KiCadWorksheet.from_text(source)


def expected_for(vector: dict[str, Any]) -> dict[str, Any]:
    """Run the public Python producer with every external input injected.

    P5_060 deliberately forbids native project/worksheet path discovery. Patch
    the Python authority's discovery helpers so this generator is hermetic and
    models the future Rust context sidecars exactly.
    """

    schematic = KiCadSchematic.from_text(vector["source"])
    worksheet = _worksheet(vector)
    drawing_settings = vector.get("drawing_settings") or {}
    project_sheet_count = vector.get("project_sheet_count")
    with ExitStack() as stack:
        stack.enter_context(
            patch.object(
                schematic_ir,
                "_project_worksheet_for_schematic",
                return_value=worksheet,
            )
        )
        stack.enter_context(
            patch.object(
                schematic_ir,
                "_schematic_project_drawing_settings",
                return_value=drawing_settings,
            )
        )
        stack.enter_context(
            patch.object(
                schematic_ir,
                "_schematic_project_text_variables",
                return_value={},
            )
        )
        stack.enter_context(
            patch.object(
                schematic_ir,
                "_schematic_project_sheet_count",
                return_value=project_sheet_count,
            )
        )
        stack.enter_context(
            patch.object(schematic_ir, "_register_embedded_fonts_for_schematic")
        )
        document = schematic_to_ir(
            schematic,
            source_path=vector["source_path"],
            document_id=vector["document_id"],
            sheet_index=vector.get("sheet_index", 1),
            sheet_count=vector.get("sheet_count", 1),
            sheet_path=vector.get("sheet_path", "/"),
            sheet_name=vector.get("sheet_name", ""),
            project_vars=vector.get("project_variables"),
        ).to_dict()
    return norm(document)


COMPACT_SOURCE = """(kicad_sch
  (version 20240101)
  (generator eeschema)
  (generator_version "10.0")
  (uuid "sch-1")
  (paper "User" 100 80 portrait)
  (title_block
    (title "${PROJECT}")
    (rev "A")
    (comment 1 "C"))
  (lib_symbols)
  (wire
    (pts (xy 1 2) (xy 3 4))
    (stroke (width 0) (type default))
    (uuid "w"))
  (bus
    (pts (xy 5 6) (xy 7 8))
    (stroke (width 0.2) (type dash) (color 1 2 3 0.5))
    (uuid "b"))
  (bus_entry
    (at 7 8)
    (size 2.54 -2.54)
    (stroke (width -1) (type dot))
    (uuid "e"))
  (junction
    (at 9 10)
    (diameter 0)
    (color 10 20 30 0.5)
    (uuid "j"))
  (no_connect (at 11 12) (uuid "n"))
  (sheet_instances (path "/" (page "1"))))"""

COMPACT_WORKSHEET = """(kicad_wks
  (version 20210606)
  (generator pl_editor)
  (setup
    (textsize 1 1)
    (linewidth 0.15)
    (textlinewidth 0.15)
    (left_margin 0)
    (right_margin 0)
    (top_margin 0)
    (bottom_margin 0))
  (tbtext "${PROJECT}-${TITLE}-${#}/${##}-${SHEETNAME}"
    (name "")
    (pos 1 2 ltcorner)))"""

DEFAULT_HEADER_SOURCE = """(kicad_sch
  (version 20240101)
  (generator eeschema)
  (generator_version "10.0")
  (uuid "default-sheet")
  (paper "A4")
  (title_block
    (title "Default")
    (date "2026-08-17")
    (rev "R1")
    (company "Monkey")
    (comment 1 "Foundation"))
  (lib_symbols)
  (sheet_instances (path "/" (page "1"))))"""

BITMAP_TRANSPARENT_JUNCTION_SOURCE = """(kicad_sch
  (version 20240101)
  (generator eeschema)
  (generator_version "10.0")
  (uuid "bitmap-junction")
  (paper "User" 20 20)
  (lib_symbols)
  (junction
    (at 10 10)
    (diameter 0)
    (color 0 0 0 0)
    (uuid "transparent-junction")))"""

ONE_PIXEL_PNG = (
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8A"
    "AQUBAScY42YAAAAASUVORK5CYII="
)

BITMAP_WORKSHEET = f"""(kicad_wks
  (version 20210606)
  (generator pl_editor)
  (setup
    (left_margin 0)
    (right_margin 0)
    (top_margin 0)
    (bottom_margin 0))
  (bitmap
    (name "")
    (pos 3 4 ltcorner)
    (scale 1)
    (data "{ONE_PIXEL_PNG}")))"""


def vectors() -> list[dict[str, Any]]:
    return [
        {
            "id": "custom-worksheet-and-connectivity-family-order",
            "source": COMPACT_SOURCE,
            "source_path": "foundation.kicad_sch",
            "document_id": "foundation",
            "worksheet_source": COMPACT_WORKSHEET,
            "project_variables": {"PROJECT": "PX", "TITLE": "bad"},
            "sheet_index": 2,
            "sheet_count": 3,
            "sheet_path": "/child",
            "sheet_name": "Child",
        },
        {
            "id": "default-worksheet-header-only",
            "source": DEFAULT_HEADER_SOURCE,
            "source_path": "default.kicad_sch",
            "document_id": "default-sheet",
            "project_variables": {},
            "sheet_index": 1,
            "sheet_count": 1,
            "sheet_path": "/",
            "sheet_name": "IgnoredAtRoot",
        },
        {
            "id": "valid-bitmap-and-transparent-junction",
            "source": BITMAP_TRANSPARENT_JUNCTION_SOURCE,
            "source_path": "bitmap-junction.kicad_sch",
            "document_id": "bitmap-junction",
            "worksheet_source": BITMAP_WORKSHEET,
            "project_variables": {},
            "sheet_index": 1,
            "sheet_count": 1,
            "sheet_path": "/",
            "sheet_name": "",
        },
    ]


def payload() -> dict[str, Any]:
    generated = vectors()
    for vector in generated:
        vector["expected"] = expected_for(vector)
    return {
        "schema": "kicad_monkey.schematic_plotter_parity.a0",
        "vectors": generated,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    rendered = json.dumps(payload(), indent=1, ensure_ascii=False) + "\n"
    if args.check:
        if not VECTOR_PATH.is_file() or VECTOR_PATH.read_text(encoding="utf-8") != rendered:
            raise SystemExit(f"stale schematic plotter vectors: {VECTOR_PATH}")
        return 0
    VECTOR_PATH.write_text(rendered, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

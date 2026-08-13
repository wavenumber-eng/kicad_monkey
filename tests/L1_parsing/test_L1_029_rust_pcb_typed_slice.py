"""Rack ownership for the native Rust PCB reader/writer vertical slice."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess

from _suite_paths import TEST_CORPUS_ROOT
from kicad_monkey import KiCadPcb


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
CORPUS_ROOT = Path(os.environ.get("WN_TEST_CORPUS", TEST_CORPUS_ROOT))
CORPUS_BOARDS = (
    CORPUS_ROOT
    / "kicad/projects/4-ch-backplane/input/4-ch-backplane.kicad_pcb",
    CORPUS_ROOT
    / "kicad/projects/speedy_processing_module/input/11-10084__speedy_processing_module__B.kicad_pcb",
)
EXTENDED_CARRIERS = """(kicad_pcb
  (version 20260206)
  (generator pcbnew)
  (generator_version "10.0")
  (general (thickness 1.8) (legacy_teardrops yes))
  (paper "A3")
  (setup (pad_to_mask_clearance 0.05) (pad_to_paste_clearance -0.01)
    (pad_to_paste_clearance_ratio -0.1))
  (embedded_fonts yes)
  (variants (variant (name "Production") (description "Loaded"))
    (variant (name "No RF")))
  (image (at 1 2) (layer "F.SilkS") (scale 2) (locked yes)
    (data "YWJj" "ZA==") (uuid image-id))
  (barcode (locked yes) (at 3 4 90) (layer "B.SilkS") (size 10 5)
    (text "ABC") (text_height 1.2) (type qrcode) (ecc_level H)
    (hide yes) (knockout yes) (margins 0.5 0.75) (uuid barcode-id))
  (table (column_count 2) (layer "F.Cu")
    (border (external no) (header yes))
    (separators (rows no) (cols yes))
    (column_widths 10 20) (row_heights 5 6)
    (cells
      (table_cell "A" (start 0 0) (end 10 5) (margins 1 2 3 4)
        (span 2 1) (angle 90) (layer "F.Cu") (locked yes) (uuid cell-id))
      (table_cell "B" (start 10 0) (end 20 5) (layer "F.Cu")))
    (uuid table-id))
)"""


def _run(command: list[str], *, timeout: int = 180) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )
    return completed


def _projection_executable() -> Path:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust PCB gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "pcb_projection_gate",
        ]
    )
    return PACKAGE_ROOT / "target/debug/examples" / (
        "pcb_projection_gate.exe" if os.name == "nt" else "pcb_projection_gate"
    )


def test_rack_runs_native_pcb_reader_writer_correctness_gate() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust PCB gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "pcb_typed_slice",
        ]
    )


def test_native_pcb_projection_matches_python_on_promoted_corpus() -> None:
    missing = [str(path) for path in CORPUS_BOARDS if not path.is_file()]
    assert not missing, (
        "required promoted PCB corpus evidence is unavailable; missing: "
        + ", ".join(missing)
    )
    executable = _projection_executable()
    summaries = json.loads(
        _run([str(executable), *(str(path) for path in CORPUS_BOARDS)]).stdout
    )
    assert len(summaries) == len(CORPUS_BOARDS)
    for board_path, summary in zip(CORPUS_BOARDS, summaries, strict=True):
        _assert_summary_matches_python(board_path, summary)


def test_newer_sparse_carriers_match_python_on_durable_vector(tmp_path: Path) -> None:
    board_path = tmp_path / "extended-carriers.kicad_pcb"
    board_path.write_text(EXTENDED_CARRIERS, encoding="utf-8")
    summaries = json.loads(
        _run([str(_projection_executable()), str(board_path)]).stdout
    )
    assert len(summaries) == 1
    _assert_summary_matches_python(board_path, summaries[0])


def _assert_summary_matches_python(board_path: Path, summary: dict) -> None:
    board = KiCadPcb.from_file(board_path)
    assert summary["source_bytes"] == board_path.stat().st_size
    assert summary["counts"] == {
        "layers": len(board.layers),
        "nets": len(board.nets),
        "properties": len(board.properties),
        "variants": len(board.variants),
        "footprints": len(board.footprints),
        "pads": sum(len(footprint.pads) for footprint in board.footprints),
        "models": sum(len(footprint.models) for footprint in board.footprints),
        "segments": len(board.segments),
        "vias": len(board.vias),
        "zones": len(board.zones),
        "gr_texts": len(board.gr_texts),
        "gr_lines": len(board.gr_lines),
        "gr_rects": len(board.gr_rects),
        "gr_arcs": len(board.gr_arcs),
        "gr_circles": len(board.gr_circles),
        "gr_polys": len(board.gr_polys),
        "gr_curves": len(board.gr_curves),
        "gr_text_boxes": len(board.gr_text_boxes),
        "images": len(board.images),
        "barcodes": len(board.barcodes),
        "tables": len(board.tables),
        "table_cells": sum(len(table.cells) for table in board.tables),
        "arcs": len(board.arcs),
        "dimensions": len(board.dimensions),
        "groups": len(board.groups),
        "generated_items": len(board.generated_items),
        "embedded_files": len(board.embedded_files),
    }
    assert summary["metadata"] == {
        "version": board.version,
        "generator": board.generator,
        "generator_version": board.generator_version,
        "paper": board.paper,
        "thickness": board.thickness,
        "legacy_teardrops": board.legacy_teardrops,
        "embedded_fonts": board.embedded_fonts,
        "pad_to_mask_clearance": board.pad_to_mask_clearance,
        "pad_to_paste_clearance": board.pad_to_paste_clearance,
        "pad_to_paste_clearance_ratio": board.pad_to_paste_clearance_ratio,
    }
    if board.variants:
        variant = board.variants[0]
        assert summary["first_variant"] == {
            "name": variant.name,
            "description": variant.description,
        }
    else:
        assert summary["first_variant"] is None
    if board.images:
        image = board.images[0]
        assert summary["first_image"] == {
            "at_x": image.at_x,
            "at_y": image.at_y,
            "scale": image.scale,
            "layer": image.layer,
            "locked": image.locked,
            "encoded_data_bytes": len(image.data),
            "uuid": image.uuid or None,
        }
    else:
        assert summary["first_image"] is None
    if board.barcodes:
        barcode = board.barcodes[0]
        assert summary["first_barcode"] == {
            "at_x": barcode.at_x,
            "at_y": barcode.at_y,
            "angle": barcode.at_angle,
            "layer": barcode.layer,
            "width": barcode.width,
            "height": barcode.height,
            "text": barcode.text,
            "text_height": barcode.text_height,
            "kind": barcode.barcode_type,
            "ecc_level": barcode.ecc_level,
            "locked": barcode.locked,
            "show_text": barcode.show_text,
            "knockout": barcode.knockout,
            "margin_x": barcode.margins.x,
            "margin_y": barcode.margins.y,
            "uuid": barcode.uuid or None,
        }
    else:
        assert summary["first_barcode"] is None
    if board.tables:
        table = board.tables[0]
        assert summary["first_table"] == {
            "column_count": table.column_count,
            "layer": table.layer,
            "border_external": table.border_external,
            "border_header": table.border_header,
            "separator_rows": table.separators_rows,
            "separator_columns": table.separators_cols,
            "column_widths": table.column_widths,
            "row_heights": table.row_heights,
            "cell_count": len(table.cells),
            "uuid": table.uuid or None,
        }
    else:
        assert summary["first_table"] is None

    first_cell = next(
        (
            (table_index, cell)
            for table_index, table in enumerate(board.tables)
            for cell in table.cells
        ),
        None,
    )
    if first_cell is None:
        assert summary["first_table_cell"] is None
    else:
        table_index, cell = first_cell
        assert summary["first_table_cell"] == {
            "table_index": table_index,
            "text": cell.text,
            "start_x": cell.start_x,
            "start_y": cell.start_y,
            "end_x": cell.end_x,
            "end_y": cell.end_y,
            "margins": list(cell.margins),
            "column_span": cell.span[0],
            "row_span": cell.span[1],
            "angle": cell.angle,
            "layer": cell.layer,
            "locked": cell.locked,
            "uuid": cell.uuid or None,
        }
    if board.footprints:
        assert summary["first_footprint"] == {
            "library_link": board.footprints[0].library_link,
            "reference": board.footprints[0].get_property_value("Reference"),
        }
    if board.segments:
        assert summary["first_segment"] == {
            "start_x": board.segments[0].start_x,
            "end_x": board.segments[0].end_x,
            "net": {
                "ordinal": board.segments[0].net.ordinal,
                "name": board.segments[0].net.name or None,
            },
        }
    if board.vias:
        assert summary["first_via"] == {
            "at_x": board.vias[0].at_x,
            "at_y": board.vias[0].at_y,
            "net": {
                "ordinal": board.vias[0].net.ordinal,
                "name": board.vias[0].net.name or None,
            },
        }
    if summary["first_graphic"] is not None:
        graphic_collections = {
            "gr_text": board.gr_texts,
            "gr_line": board.gr_lines,
            "gr_rect": board.gr_rects,
            "gr_arc": board.gr_arcs,
            "gr_circle": board.gr_circles,
            "gr_poly": board.gr_polys,
            "gr_curve": board.gr_curves,
            "gr_text_box": board.gr_text_boxes,
        }
        kind = summary["first_graphic"]["kind"]
        graphic = graphic_collections[kind][0]
        assert summary["first_graphic"]["layer"] == graphic.layer
        if kind in {"gr_text", "gr_text_box"}:
            assert summary["first_graphic"]["text"] == graphic.text
    if board.arcs:
        arc = board.arcs[0]
        assert summary["first_arc"] == {
            "start_x": arc.start_x,
            "mid_x": arc.mid_x,
            "end_x": arc.end_x,
            "net": {"ordinal": arc.net.ordinal, "name": arc.net.name or None},
        }
    if board.dimensions:
        dimension = board.dimensions[0]
        assert summary["first_dimension"] == {
            "kind": dimension.dimension_type,
            "layer": dimension.layer,
            "point_count": len(dimension.points),
            "uuid": dimension.uuid or None,
        }
    if board.groups:
        group = board.groups[0]
        assert summary["first_group"] == {
            "name": group.name,
            "uuid": group.uuid or None,
            "member_count": len(group.members),
        }
    if board.generated_items:
        generated = board.generated_items[0]
        assert summary["first_generated"] == {
            "kind": generated.generator_type or None,
            "name": generated.name or None,
            "uuid": generated.uuid or None,
            "member_count": len(generated.members),
        }
    if board.embedded_files:
        embedded = board.embedded_files[0]
        assert summary["first_embedded_file"] == {
            "name": embedded.name,
            "file_type": embedded.file_type,
            "checksum": embedded.checksum or None,
            "encoded_data_bytes": len(embedded.data),
        }
    pads = [pad for footprint in board.footprints for pad in footprint.pads]
    if pads:
        pad = pads[0]
        assert summary["first_pad"] == {
            "number": pad.number,
            "kind": pad.pad_type.value,
            "shape": pad.shape.value,
            "at_x": pad.at_x,
            "at_y": pad.at_y,
            "size_x": pad.size_x,
            "size_y": pad.size_y,
            "layers": pad.layers,
            "net": {
                "ordinal": pad.net.ordinal,
                "name": pad.net.name or None,
            },
        }
    models = [model for footprint in board.footprints for model in footprint.models]
    if models:
        model = models[0]
        assert summary["first_model"] == {
            "path": model.path,
            "offset": list(model.offset),
            "scale": list(model.scale),
            "rotate": list(model.rotate),
        }

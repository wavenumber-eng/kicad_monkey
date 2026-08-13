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
SPARSE_TABLE_BLOCKS = """(kicad_pcb
  (table (uuid absent-blocks))
  (table (border) (separators) (uuid sparse-blocks))
)"""
ZONE_CARRIERS = """(kicad_pcb
  (net 1 "GND")
  (zone (net 1) (net_name "GND") (locked yes) (layers "F.Cu" "B.Cu")
    (uuid zone-copper) (name "Power") (hatch edge 0.6) (priority 3)
    (placement (enabled yes) (component_class "RF"))
    (connect_pads (clearance 0.7))
    (min_thickness 0.3) (filled_areas_thickness yes)
    (fill yes (thermal_gap 0.4) (thermal_bridge_width 0.6)
      (island_removal_mode 2) (island_area_min 5))
    (property (layer "F.Cu") (hatch_position (xy 1 2)))
    (polygon (pts (xy 0 0) (xy 10 0) (xy 10 10)))
    (filled_polygon (layer "F.Cu") (island)
      (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10))))
  (zone (net 0) (layer "F.Cu") (uuid zone-keepout)
    (keepout (tracks allowed) (vias not_allowed))
    (polygon (pts (xy 1 1) (xy 2 1) (xy 2 2))))
)"""
PHYSICAL_CARRIERS = """(kicad_pcb
  (footprint "Demo:Part" (layer "B.Cu") (at 100 50 90) (locked yes)
    (path "/root/U1") (sheetname "RF") (sheetfile "rf.kicad_sch") (uuid fp-id)
    (descr "Physical carrier") (tags "durable vector")
    (attr smd dnp exclude_from_bom) (embedded_fonts yes)
    (duplicate_pad_numbers_are_jumpers no)
    (solder_mask_margin 0.1) (solder_paste_margin -0.02)
    (solder_paste_margin_ratio -0.15) (clearance 0.25) (zone_connect 2)
    (property "Reference" "U1" (at 0 -2 0) (layer "F.SilkS")
      (hide yes) (unlocked yes) (uuid property-id))
    (property "Datasheet" "https://example.invalid")
    (fp_line (start -1 0) (end 1 0) (stroke (width 0.1) (type default))
      (layer "Edge.Cuts") (uuid fp-edge))
    (fp_circle (center 0 0) (end 1 0) (stroke (width 0.1) (type default))
      (fill none) (layer "F.SilkS"))
    (pad "1" thru_hole circle (at 1 2 30) (size 2 2)
      (drill 0.8 (offset 0.1 -0.2)) (layers "*.Cu" "*.Mask") (uuid pad-1))
    (pad "" np_thru_hole oval (at -2 3) (size 2 3)
      (drill oval 1 2) (layers "*.Cu" "*.Mask") (uuid pad-2)))
  (gr_line (start 0 0) (end 10 0) (stroke (width 0.05) (type default))
    (layer "Edge.Cuts") (uuid board-edge))
  (via (at 4 5) (size 1) (drill 0.4) (layers "F.Cu" "B.Cu") (uuid via-1))
)"""


def _run(command: list[str], *, timeout: int = 180) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
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
            "--test",
            "pcb_zone_slice",
            "--test",
            "pcb_physical_slice",
            "--test",
            "pcb_document_slice",
            "--test",
            "pcb_footprint_children_slice",
            "--test",
            "pcb_setup_slice",
        ]
    )


def test_native_pcb_projection_matches_python_on_promoted_corpus() -> None:
    missing = [str(path) for path in CORPUS_BOARDS if not path.is_file()]
    assert not missing, (
        "required promoted PCB corpus evidence is unavailable. Restore and verify it "
        "with `uv run --extra test python scripts/kicad_corpus_archive.py restore "
        "--check-zip`, or set WN_TEST_CORPUS to a reviewed corpus root. Missing: "
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
    inputs = {
        "extended-carriers.kicad_pcb": EXTENDED_CARRIERS,
        "sparse-table-blocks.kicad_pcb": SPARSE_TABLE_BLOCKS,
        "zone-carriers.kicad_pcb": ZONE_CARRIERS,
        "physical-carriers.kicad_pcb": PHYSICAL_CARRIERS,
    }
    board_paths = []
    for name, source in inputs.items():
        board_path = tmp_path / name
        board_path.write_text(source, encoding="utf-8")
        board_paths.append(board_path)
    summaries = json.loads(
        _run(
            [str(_projection_executable()), *(str(path) for path in board_paths)]
        ).stdout
    )
    assert len(summaries) == len(board_paths)
    for board_path, summary in zip(board_paths, summaries, strict=True):
        _assert_summary_matches_python(board_path, summary)


def _assert_summary_matches_python(board_path: Path, summary: dict) -> None:
    board = KiCadPcb.from_file(board_path)
    assert summary["source_bytes"] == board_path.stat().st_size
    assert summary["counts"] == {
        "layers": len(board.layers),
        "nets": len(board.nets),
        "properties": len(board.properties),
        "variants": len(board.variants),
        "footprints": len(board.footprints),
        "footprint_properties": sum(
            len(footprint.properties) for footprint in board.footprints
        ),
        "pads": sum(len(footprint.pads) for footprint in board.footprints),
        "models": sum(len(footprint.models) for footprint in board.footprints),
        "footprint_graphics": sum(
            len(footprint.fp_lines)
            + len(footprint.fp_arcs)
            + len(footprint.fp_rects)
            + len(footprint.fp_circles)
            + len(footprint.fp_polys)
            for footprint in board.footprints
        ),
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
    stackup = board.stackup
    assert summary["setup"] == (
        {
            "aux_axis_origin": list(board.aux_axis_origin_mm),
            "stackup": (
                {
                    "layer_count": len(stackup.layers),
                    "copper_finish": stackup.copper_finish,
                    "dielectric_constraints": stackup.dielectric_constraints,
                    "edge_connector": stackup.edge_connector.value,
                    "edge_plating": stackup.edge_plating,
                    "first_layer": (
                        {
                            "name": stackup.layers[0].name,
                            "type_name": stackup.layers[0].type_name,
                            "thickness": stackup.layers[0].thickness,
                            "thickness_locked": stackup.layers[0].thickness_locked,
                            "material": stackup.layers[0].material,
                            "epsilon_r": stackup.layers[0].epsilon_r,
                            "loss_tangent": stackup.layers[0].loss_tangent,
                            "color": stackup.layers[0].color,
                        }
                        if stackup.layers
                        else None
                    ),
                }
                if stackup is not None
                else None
            ),
        }
        if board.setup_sexp is not None
        else None
    )
    assert summary["zone_metrics"] == {
        "authored_polygons": sum(len(zone.polygons) for zone in board.zones),
        "filled_polygons": sum(len(zone.filled_polygons) for zone in board.zones),
        "authored_points": sum(
            len(polygon.points) for zone in board.zones for polygon in zone.polygons
        ),
        "filled_points": sum(
            len(polygon.points)
            for zone in board.zones
            for polygon in zone.filled_polygons
        ),
        "keepouts": sum(zone.keepout is not None for zone in board.zones),
        "placements": sum(zone.placement is not None for zone in board.zones),
        "layer_properties": sum(
            len(zone.layer_properties) for zone in board.zones
        ),
    }
    if board.zones:
        zone = board.zones[0]
        assert summary["first_zone"] == {
            "net": {"ordinal": zone.net.ordinal, "name": zone.net.name or None},
            "has_explicit_net_name": zone.has_explicit_net_name,
            "layers": zone.layers,
            "layers_plural": zone.layers_plural,
            "locked": zone.locked,
            "uuid": zone.uuid or None,
            "name": zone.name,
            "hatch_style": zone.hatch_style,
            "hatch_pitch": zone.hatch_pitch,
            "priority": zone.priority,
            "connect_pads_clearance": zone.connect_pads_clearance,
            "min_thickness": zone.min_thickness,
            "filled_areas_thickness": zone.filled_areas_thickness,
            "fill_enabled": zone.fill_enabled,
            "thermal_gap": zone.thermal_gap,
            "thermal_bridge_width": zone.thermal_bridge_width,
            "island_removal_mode": zone.island_removal_mode,
            "island_area_min": zone.island_area_min,
            "keepout": (
                {
                    "tracks": zone.keepout.tracks,
                    "vias": zone.keepout.vias,
                    "pads": zone.keepout.pads,
                    "copperpour": zone.keepout.copperpour,
                    "footprints": zone.keepout.footprints,
                }
                if zone.keepout
                else None
            ),
            "placement": (
                {
                    "enabled": zone.placement.enabled,
                    "source_type": zone.placement.source_type.value,
                    "source": zone.placement.source,
                }
                if zone.placement
                else None
            ),
            "first_layer_property": (
                {
                    "layer": zone.layer_properties[0][0],
                    "hatch_offset": list(zone.layer_properties[0][1]),
                }
                if zone.layer_properties
                else None
            ),
            "first_authored_points": (
                [list(point) for point in zone.polygons[0].points]
                if zone.polygons
                else None
            ),
            "first_filled": (
                {
                    "layer": zone.filled_polygons[0].layer,
                    "island": zone.filled_polygons[0].island,
                    "points": [
                        list(point) for point in zone.filled_polygons[0].points
                    ],
                }
                if zone.filled_polygons
                else None
            ),
        }
    else:
        assert summary["first_zone"] is None
    _assert_physical_summary(board, summary)
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
        footprint = board.footprints[0]
        assert summary["first_footprint"] == {
            "library_link": footprint.library_link,
            "reference": footprint.get_property_value("Reference") or None,
            "value": footprint.get_property_value("Value") or None,
            "layer": footprint.layer,
            "description": footprint.descr,
            "tags": footprint.tags,
            "attributes": footprint.attr,
            "locked": footprint.locked,
            "embedded_fonts": footprint.embedded_fonts,
            "duplicate_pad_numbers_are_jumpers": (
                footprint.duplicate_pad_numbers_are_jumpers
            ),
            "solder_mask_margin": footprint.solder_mask_margin,
            "solder_paste_margin": footprint.solder_paste_margin,
            "solder_paste_margin_ratio": footprint.solder_paste_margin_ratio,
            "clearance": footprint.clearance,
            "zone_connect": footprint.zone_connect,
            "property_count": len(footprint.properties),
            "graphic_count": (
                len(footprint.fp_lines)
                + len(footprint.fp_arcs)
                + len(footprint.fp_circles)
                + len(footprint.fp_rects)
                + len(footprint.fp_polys)
            ),
        }
        first_property = next(
            (
                (footprint_index, prop)
                for footprint_index, footprint in enumerate(board.footprints)
                for prop in footprint.properties
            ),
            None,
        )
        if first_property is None:
            assert summary["first_footprint_property"] is None
        else:
            footprint_index, prop = first_property
            assert summary["first_footprint_property"] == {
                "footprint_index": footprint_index,
                "name": prop.name,
                "value": prop.value,
                "at_x": prop.at_x,
                "at_y": prop.at_y,
                "angle": prop.at_angle,
                "layer": prop.layer,
                "hidden": prop.hide,
                "unlocked": prop.unlocked,
                "graphical": prop.graphical,
                "uuid": prop.uuid,
            }
        _assert_first_footprint_graphic(board, summary)
    else:
        assert summary["first_footprint"] is None
        assert summary["first_footprint_property"] is None
        assert summary["first_footprint_graphic"] is None
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


def _assert_physical_summary(board: KiCadPcb, summary: dict) -> None:
    pad_holes = [
        (index, footprint_index, pad)
        for index, (footprint_index, pad) in enumerate(
            (footprint_index, pad)
            for footprint_index, footprint in enumerate(board.footprints)
            for pad in footprint.pads
        )
        if pad.drill is not None and pad.drill > 0
    ]
    via_holes = [(index, via) for index, via in enumerate(board.vias) if via.drill > 0]
    profile = board.board_outline_carriers()
    assert summary["physical_metrics"] == {
        "footprint_transforms": len(board.footprints),
        "holes": len(pad_holes) + len(via_holes),
        "pad_holes": len(pad_holes),
        "via_holes": len(via_holes),
        "profile_primitives": len(profile),
        "board_profile": sum(item.owner_kind == "board" for item in profile),
        "footprint_profile": sum(item.owner_kind == "footprint" for item in profile),
    }
    if board.footprints:
        footprint = board.footprints[0]
        assert summary["first_footprint_transform"] == {
            "footprint_index": 0,
            "x": footprint.at_x,
            "y": footprint.at_y,
            "angle": footprint.at_angle,
            "layer": footprint.layer,
            "locked": footprint.locked,
            "path": footprint.placement.path or None,
            "sheet_name": footprint.placement.sheetname or None,
            "sheet_file": footprint.placement.sheetfile or None,
            "uuid": footprint.uuid or None,
        }
    else:
        assert summary["first_footprint_transform"] is None

    if pad_holes:
        owner_index, footprint_index, pad = pad_holes[0]
        width = pad.drill_width if pad.drill_oval else pad.drill
        height = pad.drill_height if pad.drill_oval else pad.drill
        assert summary["first_hole"] == {
            "owner": "pad",
            "owner_index": owner_index,
            "footprint_index": footprint_index,
            "center": [pad.at_x, pad.at_y],
            "offset": [pad.drill_offset_x or 0.0, pad.drill_offset_y or 0.0],
            "shape": "oval" if pad.drill_oval else "round",
            "width": width,
            "height": height if height is not None else width,
            "angle": pad.at_angle,
            "plated": pad.pad_type.value != "np_thru_hole",
            "layers": pad.layers,
            "uuid": pad.uuid or None,
        }
    elif via_holes:
        owner_index, via = via_holes[0]
        assert summary["first_hole"] == {
            "owner": "via",
            "owner_index": owner_index,
            "footprint_index": None,
            "center": [via.at_x, via.at_y],
            "offset": [0.0, 0.0],
            "shape": "round",
            "width": via.drill,
            "height": via.drill,
            "angle": 0.0,
            "plated": True,
            "layers": via.layers,
            "uuid": via.uuid or None,
        }
    else:
        assert summary["first_hole"] is None


def _assert_first_footprint_graphic(board: KiCadPcb, summary: dict) -> None:
    collection_by_head = {
        "fp_line": ("gr_line", "fp_lines"),
        "fp_arc": ("gr_arc", "fp_arcs"),
        "fp_circle": ("gr_circle", "fp_circles"),
        "fp_rect": ("gr_rect", "fp_rects"),
        "fp_poly": ("gr_poly", "fp_polys"),
    }
    first = None
    for footprint_index, footprint in enumerate(board.footprints):
        for element in footprint._raw_sexp or ():
            if not isinstance(element, list) or not element:
                continue
            selected = collection_by_head.get(element[0])
            if selected is not None:
                kind, collection_name = selected
                first = (footprint_index, kind, getattr(footprint, collection_name)[0])
                break
        if first is not None:
            break
    if first is None:
        assert summary["first_footprint_graphic"] is None
        return
    footprint_index, kind, graphic = first
    assert summary["first_footprint_graphic"] == {
        "footprint_index": footprint_index,
        "kind": kind,
        "layer": graphic.layer,
        "stroke_width": graphic.stroke.width,
        "stroke_kind": graphic.stroke.type.value,
        "fill": graphic.fill.value if hasattr(graphic, "fill") else None,
        "point_count": len(graphic.points) if hasattr(graphic, "points") else 0,
        "uuid": graphic.uuid,
    }

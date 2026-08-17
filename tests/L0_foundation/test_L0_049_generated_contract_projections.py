"""Generated Python and TypeScript transport-projection gate."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    BoardPlotDocumentA0,
    CompiledSchematicGraphA0,
    FootprintPlotDocumentA0,
    SchematicPlotDocumentA0,
    SExpressionBuildRequestA0,
    decode_board_plot_document_a0,
    decode_compiled_schematic_graph_a0,
    decode_footprint_plot_document_a0,
    decode_schematic_plot_document_a0,
    decode_schematic_plot_request_a0,
    decode_sexpr_build_request_a0,
    decode_symbol_plot_document_a0,
)


PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def _run(command: list[str]) -> None:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )


def _vector_expected(file_name: str, vector_id: str) -> dict:
    payload = json.loads(
        (PACKAGE_ROOT / "tests/parity" / file_name).read_text(encoding="utf-8")
    )
    return next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == vector_id
    )


def _first_operation(document: dict, kind: str) -> dict:
    return next(
        operation
        for record in document["records"]
        for operation in record["operations"]
        if operation["kind"] == kind
    )


def test_python_projection_is_strict_and_uses_wire_field_names() -> None:
    encoded = b"""{
        "type": "kicad_monkey.sexpr_build.request",
        "version": "a0",
        "root": {"kind": "atom", "text": "footprint"},
        "max_output_bytes": "4096",
        "max_depth": 16,
        "max_nodes": 64
    }"""
    request = decode_sexpr_build_request_a0(encoded)
    assert isinstance(request, SExpressionBuildRequestA0)
    assert request.type_ == "kicad_monkey.sexpr_build.request"
    reencoded = msgspec.json.encode(request)
    assert json.loads(reencoded) == json.loads(encoded)
    assert b'"type":"kicad_monkey.sexpr_build.request"' in reencoded

    with pytest.raises(msgspec.ValidationError, match="unknown_field"):
        decode_sexpr_build_request_a0(encoded[:-2] + b', "unknown_field": true}')


def test_generated_projections_are_current_and_typescript_compiles() -> None:
    npm = shutil.which("npm")
    assert npm is not None, "npm is required for generated contract checks"
    _run([npm, "run", "check:python-generation"])
    _run([npm, "run", "check:typescript-generation"])


def test_generated_compiled_graph_projection_accepts_registered_vector() -> None:
    vectors = json.loads(
        (PACKAGE_ROOT / "tests/parity/compiled_schematic_graph_a0_vectors.json").read_text(
            encoding="utf-8"
        )
    )
    assert isinstance(
        decode_compiled_schematic_graph_a0(json.dumps(vectors["graph"]).encode()),
        CompiledSchematicGraphA0,
    )


@pytest.mark.parametrize("version", [-9_007_199_254_740_991, 9_007_199_254_740_991])
def test_python_plotter_projection_accepts_javascript_safe_boundaries(version: int) -> None:
    payload = _footprint_version_payload(version)
    assert isinstance(decode_footprint_plot_document_a0(payload), FootprintPlotDocumentA0)


@pytest.mark.parametrize("version", [-9_007_199_254_740_992, 9_007_199_254_740_992])
def test_python_plotter_projection_rejects_unsafe_integer_neighbors(version: int) -> None:
    payload = _footprint_version_payload(version)
    with pytest.raises(msgspec.ValidationError):
        decode_footprint_plot_document_a0(payload)


def _footprint_version_payload(version: int) -> bytes:
    return json.dumps(
        {
            "schema": "kicad.plotter_ir.a0",
            "source_kind": "MOD",
            "total_operations": 0,
            "records": [
                {
                    "uuid": "",
                    "kind": "footprint",
                    "object_id": "boundary",
                    "operation_count": 0,
                    "operations": [],
                    "name": "boundary",
                    "layer": "F.Cu",
                    "locked": False,
                    "placed": False,
                    "descr": "",
                    "tags": "",
                    "attr": [],
                }
            ],
            "document_id": "boundary",
            "coordinate_space": {"unit": "nm", "y_axis": "down"},
            "version": version,
            "generator": "pcbnew",
            "generator_version": "10.0",
        }
    ).encode()


def test_python_plotter_decoder_enforces_graphic_and_drill_semantics() -> None:
    vectors = json.loads(
        (PACKAGE_ROOT / "tests" / "parity" / "footprint_plotter_a0_vectors.json")
        .read_text(encoding="utf-8")
    )
    valid = vectors["vectors"][0]["expected"]
    assert isinstance(
        decode_footprint_plot_document_a0(json.dumps(valid).encode()),
        FootprintPlotDocumentA0,
    )

    missing_layer = json.loads(json.dumps(valid))
    del missing_layer["records"][0]["operations"][0]["layer"]
    with pytest.raises(msgspec.ValidationError, match="conflicting_plotter_fields"):
        decode_footprint_plot_document_a0(json.dumps(missing_layer).encode())

    contradictory = json.loads(json.dumps(valid))
    operation = contradictory["records"][0]["operations"][0]
    operation["layers"] = ["F.Cu"]
    operation["mask_margin_nm"] = 0
    with pytest.raises(msgspec.ValidationError, match="conflicting_plotter_fields"):
        decode_footprint_plot_document_a0(json.dumps(contradictory).encode())

    arbitrary_role = json.loads(json.dumps(valid))
    operation = arbitrary_role["records"][0]["operations"][0]
    del operation["layer"]
    operation["role"] = "arbitrary"
    operation["layers"] = ["F.Cu"]
    with pytest.raises(msgspec.ValidationError):
        decode_footprint_plot_document_a0(json.dumps(arbitrary_role).encode())

    promoted = vectors["vectors"][1]["expected"]
    static_missing_layer = json.loads(json.dumps(promoted))
    static_operation = next(
        operation
        for operation in static_missing_layer["records"][0]["operations"]
        if operation["kind"] == "ArcThreePoint"
    )
    del static_operation["layer"]
    with pytest.raises(msgspec.ValidationError, match="missing_layer"):
        decode_footprint_plot_document_a0(json.dumps(static_missing_layer).encode())


def test_python_board_validator_rejects_shared_plot_image_arm() -> None:
    payload = {
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "PCB",
        "total_operations": 1,
        "records": [
            {
                "uuid": "graphic",
                "kind": "gr_line",
                "object_id": "graphic",
                "operation_count": 1,
                "operations": [
                    {
                        "kind": "PlotImage",
                        "index": 0,
                        "x": 0,
                        "y": 0,
                        "width_nm": 0,
                        "height_nm": 0,
                        "scale": 1.0,
                        "image_data_b64": "",
                        "image_format": "png",
                    }
                ],
                "layer": "F.SilkS",
            }
        ],
        "document_id": "fail-closed",
        "coordinate_space": {"unit": "nm", "y_axis": "down"},
        "version": 1,
        "generator": "test",
        "generator_version": "1",
        "thickness_mm": 1.6,
        "paper": "A4",
    }
    with pytest.raises(msgspec.ValidationError, match="invalid_board_operation"):
        decode_board_plot_document_a0(json.dumps(payload).encode())

    # Prove the rejection is semantic: the promoted shared arm remains part of
    # the generated transport union and can decode into the board root.
    decoder = msgspec.json.Decoder(BoardPlotDocumentA0)
    assert isinstance(decoder.decode(json.dumps(payload).encode()), BoardPlotDocumentA0)


def test_new_annotation_fields_remain_fail_closed_for_existing_python_producers() -> None:
    cases = (
        (
            "board_plotter_a0_vectors.json",
            "board-text-follows-python-serializer",
            decode_board_plot_document_a0,
        ),
        (
            "footprint_plotter_a0_vectors.json",
            "standalone-properties-text-and-text-box",
            decode_footprint_plot_document_a0,
        ),
        (
            "symbol_plotter_a0_vectors.json",
            "styled-body-and-pin-text",
            decode_symbol_plot_document_a0,
        ),
    )
    for file_name, vector_id, decoder in cases:
        document = _vector_expected(file_name, vector_id)
        _first_operation(document, "Text")["context"] = {
            "hyperlink": {"href": "https://example.test"}
        }
        with pytest.raises(msgspec.ValidationError):
            decoder(json.dumps(document).encode())

    footprint = _vector_expected(
        "footprint_plotter_a0_vectors.json", "solid-line-with-metadata"
    )
    _first_operation(footprint, "ThickSegment")["stroke_color"] = "#484848FF"
    with pytest.raises(msgspec.ValidationError, match="invalid_segment_color"):
        decode_footprint_plot_document_a0(json.dumps(footprint).encode())


def test_schematic_request_enforces_annotation_settings_and_limits() -> None:
    request = {
        "type": "kicad_monkey.schematic_plot.request",
        "version": "a0",
        "sheet_index": 1,
        "sheet_count": 1,
        "sheet_path": "/",
        "sheet_name": "",
        "worksheet_mode": "default",
        "text_offset_ratio": 0.15,
        "default_line_width_nm": 152_400,
        "max_source_bytes": "4096",
        "max_worksheet_bytes": "4096",
        "max_output_bytes": "4096",
        "max_depth": 64,
        "max_parse_nodes": 1000,
        "max_selected_forms": 1000,
        "max_records": 100,
        "max_operations": 100,
        "max_points": 100,
        "max_input_points": 100,
        "max_text_bytes": "4096",
        "max_metadata_bytes": "4096",
        "max_wires": 10,
        "max_buses": 10,
        "max_bus_entries": 10,
        "max_junctions": 10,
        "max_no_connects": 10,
        "max_labels": 10,
        "max_global_labels": 10,
        "max_hierarchical_labels": 10,
        "max_netclass_flags": 10,
        "max_netclass_flag_properties": 10,
        "max_texts": 10,
        "max_text_boxes": 10,
        "max_text_box_lines": 100,
        "max_polylines": 10,
        "max_arcs": 10,
        "max_circles": 10,
        "max_rectangles": 10,
        "max_beziers": 10,
        "max_rule_areas": 10,
        "max_images": 10,
        "max_tables": 10,
        "max_table_cells": 100,
        "max_table_cell_lines": 100,
        "max_image_data_parts": 10,
        "max_image_encoded_bytes": "4096",
        "max_image_decoded_bytes": "4096",
        "max_image_width_px": 4096,
        "max_image_height_px": 4096,
        "max_image_pixels": "16777216",
        "max_image_decode_work": "16777216",
        "max_text_variables": 10,
        "max_text_variable_bytes": "4096",
        "max_worksheet_items": 10,
        "max_worksheet_repeats": 10,
        "max_worksheet_point_sets": 10,
        "max_worksheet_points": 10,
        "max_worksheet_bitmap_data_parts": 10,
        "max_worksheet_bitmap_encoded_bytes": "4096",
        "max_worksheet_bitmap_decoded_bytes": "4096",
        "max_worksheet_bitmap_width_px": 10,
        "max_worksheet_bitmap_height_px": 10,
        "max_worksheet_bitmap_pixels": "100",
        "max_worksheet_bitmap_decode_work": "4096",
    }
    decode_schematic_plot_request_a0(json.dumps(request).encode())

    for field in (
        "text_offset_ratio",
        "default_line_width_nm",
        "max_labels",
        "max_netclass_flag_properties",
        "max_text_box_lines",
        "max_rule_areas",
        "max_table_cell_lines",
        "max_image_decode_work",
    ):
        mutation = dict(request)
        del mutation[field]
        with pytest.raises(msgspec.ValidationError):
            decode_schematic_plot_request_a0(json.dumps(mutation).encode())

    for field, value in (
        ("text_offset_ratio", -0.01),
        ("default_line_width_nm", 84_699),
        ("default_line_width_nm", 9_007_199_254_740_992),
        ("max_polylines", 4_294_967_296),
        ("max_image_width_px", 4_294_967_296),
        ("max_image_encoded_bytes", "not-a-number"),
        ("max_image_pixels", "18446744073709551616"),
    ):
        mutation = dict(request)
        mutation[field] = value
        with pytest.raises(msgspec.ValidationError):
            decode_schematic_plot_request_a0(json.dumps(mutation).encode())


def test_python_schematic_contract_preserves_nullable_color_and_validates_png() -> None:
    vectors = json.loads(
        (PACKAGE_ROOT / "tests/parity/schematic_plotter_a0_vectors.json").read_text(
            encoding="utf-8"
        )
    )
    valid = vectors["vectors"][2]["expected"]
    document = decode_schematic_plot_document_a0(json.dumps(valid).encode())
    assert isinstance(document, SchematicPlotDocumentA0)
    assert json.loads(msgspec.json.encode(document)) == valid

    junction_index = next(
        index for index, record in enumerate(valid["records"]) if record["kind"] == "junction"
    )
    image_index = next(
        index
        for index, operation in enumerate(valid["records"][0]["operations"])
        if operation["kind"] == "PlotImage"
    )

    missing_color = json.loads(json.dumps(valid))
    del missing_color["records"][junction_index]["color"]
    decoded_missing = decode_schematic_plot_document_a0(json.dumps(missing_color).encode())
    assert "color" not in json.loads(msgspec.json.encode(decoded_missing))["records"][junction_index]

    mismatched_color = json.loads(json.dumps(valid))
    mismatched_color["records"][junction_index]["color"] = "#11223344"
    with pytest.raises(msgspec.ValidationError, match="invalid_junction"):
        decode_schematic_plot_document_a0(json.dumps(mismatched_color).encode())

    malformed_payloads: list[object] = [
        "AAAA",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAA=",
        "iVBORw0KGgoAAAANSkhEUgAAAAEAAAABCAYAAAA=",
        "iVBORw0KGgoAAAANSUhEUgAAAAAAAAABCAYAAAA=",
        valid["records"][0]["operations"][image_index]["image_data_b64"].replace(
            "KGgo", "KG\ngo", 1
        ),
        valid["records"][0]["operations"][image_index]["image_data_b64"] + "====",
    ]
    for payload in malformed_payloads:
        malformed = json.loads(json.dumps(valid))
        malformed["records"][0]["operations"][image_index]["image_data_b64"] = payload
        with pytest.raises(msgspec.ValidationError, match="invalid_worksheet_image"):
            decode_schematic_plot_document_a0(json.dumps(malformed).encode())

    worksheet = vectors["vectors"][1]["expected"]
    worksheet_operations = worksheet["records"][0]["operations"]
    worksheet_rect_index = next(
        index
        for index, operation in enumerate(worksheet_operations[1:], start=1)
        if operation["kind"] == "Rect"
    )
    worksheet_polyline_index = next(
        index
        for index, operation in enumerate(worksheet_operations[1:], start=1)
        if operation["kind"] == "PlotPoly"
    )

    malformed_rect = json.loads(json.dumps(worksheet))
    malformed_rect["records"][0]["operations"][worksheet_rect_index]["fill"] = (
        "FILLED_SHAPE"
    )
    with pytest.raises(msgspec.ValidationError, match="invalid_worksheet_rect"):
        decode_schematic_plot_document_a0(json.dumps(malformed_rect).encode())

    malformed_polyline = json.loads(json.dumps(worksheet))
    malformed_polyline["records"][0]["operations"][worksheet_polyline_index][
        "points"
    ].append([2, 2])
    with pytest.raises(msgspec.ValidationError, match="invalid_worksheet_polyline"):
        decode_schematic_plot_document_a0(json.dumps(malformed_polyline).encode())


def test_python_schematic_filled_shape_accepts_authoritative_optional_color() -> None:
    payload = json.loads(
        (PACKAGE_ROOT / "tests/parity/schematic_plotter_a0_vectors.json").read_text(
            encoding="utf-8"
        )
    )
    graphics = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"]
        == "schematic-graphics-rules-images-and-table-family-order"
    )

    explicit = json.loads(json.dumps(graphics))
    explicit["records"][3]["operations"][0]["fill_color"] = "#01020304"
    decode_schematic_plot_document_a0(json.dumps(explicit).encode())

    absent = json.loads(json.dumps(graphics))
    del absent["records"][3]["operations"][0]["fill_color"]
    decode_schematic_plot_document_a0(json.dumps(absent).encode())

    invalid = json.loads(json.dumps(graphics))
    invalid["records"][3]["operations"][0]["fill_color"] = "#abcdef12"
    with pytest.raises(msgspec.ValidationError, match="invalid_graphic_style"):
        decode_schematic_plot_document_a0(json.dumps(invalid).encode())

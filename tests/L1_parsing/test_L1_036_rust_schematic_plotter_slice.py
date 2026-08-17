"""Rack ownership for the Rust schematic-to-plotter-IR native slices."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
from unittest.mock import patch

from jsonschema import Draft202012Validator
import msgspec
import pytest

from kicad_monkey import kicad_schematic_to_ir as schematic_ir
from kicad_monkey.contracts.generated import decode_schematic_plot_document_a0


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTOR_PATH = PACKAGE_ROOT / "tests" / "parity" / "schematic_plotter_a0_vectors.json"
SLICE_SCHEMA_PATH = (
    PACKAGE_ROOT / "contracts" / "generated" / "schema" / "SchematicPlotDocument.json"
)
ESTABLISHED_SCHEMA_PATH = (
    PACKAGE_ROOT / "docs" / "contracts" / "kicad_plotter_ir_a0.schema.json"
)

sys.path.insert(0, str(PACKAGE_ROOT / "scripts"))
from generate_schematic_plotter_vectors import expected_for  # noqa: E402


def _run(command: list[str]) -> None:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=240,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )


def _clone(value: dict) -> dict:
    return json.loads(json.dumps(value))


def _vector(payload: dict, vector_id: str) -> dict:
    return next(vector for vector in payload["vectors"] if vector["id"] == vector_id)


def test_shared_schematic_vectors_match_python_and_both_schemas() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.schematic_plotter_parity.a0"

    slice_schema = json.loads(SLICE_SCHEMA_PATH.read_text(encoding="utf-8"))
    established_schema = json.loads(ESTABLISHED_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(slice_schema)
    Draft202012Validator.check_schema(established_schema)
    safe_integer = slice_schema["$defs"]["JavaScriptSafeInteger"]
    assert safe_integer["minimum"] == -9_007_199_254_740_991
    assert safe_integer["maximum"] == 9_007_199_254_740_991

    for vector in payload["vectors"]:
        actual = expected_for(vector)
        assert actual == vector["expected"], vector["id"]
        Draft202012Validator(slice_schema).validate(actual)
        Draft202012Validator(established_schema).validate(actual)
        generated = decode_schematic_plot_document_a0(
            json.dumps(actual).encode("utf-8")
        )
        assert json.loads(msgspec.json.encode(generated)) == actual

    compact = _vector(
        payload, "custom-worksheet-connectivity-and-annotation-family-order"
    )["expected"]
    assert compact["total_operations"] == 21
    assert [record["kind"] for record in compact["records"]] == [
        "sheet_header",
        "wire",
        "bus",
        "bus_entry",
        "junction",
        "no_connect",
        "label",
        "global_label",
        "hierarchical_label",
        "netclass_flag",
        "text",
        "text_box",
    ]
    assert compact["records"][0]["operations"][1]["text"] == "PX-PX-2/3-Child"
    assert compact["records"][4]["operations"][0]["diameter_nm"] == 914_400
    assert compact["records"][5]["operations"][0]["points"] == [
        [10_390_400, 11_390_400],
        [11_609_600, 12_609_600],
    ]

    local = compact["records"][6]
    assert local["object_id"] == local["text"] == "BUS{0..1}"
    assert local["operations"][0] == {
        "kind": "Text",
        "index": 0,
        "x": 675_000,
        "y": 2_000_000,
        "text": "BUS{0..1}",
        "color": "#000084FF",
        "orient_deg": 90.0,
        "size_x_nm": 1_000_000,
        "size_y_nm": 1_000_000,
        "h_align": "GR_TEXT_H_ALIGN_LEFT",
        "v_align": "GR_TEXT_V_ALIGN_BOTTOM",
        "pen_width_nm": 203_200,
        "italic": False,
        "bold": False,
        "multiline": False,
        "font_face": "Arial",
    }

    global_label = compact["records"][7]
    assert (global_label["text"], global_label["shape"]) == ("", "output")
    assert global_label["operations"][1]["points"] == [
        [3_000_000, 4_000_000],
        [3_000_000, 2_972_597],
        [2_097_597, 2_972_597],
        [1_222_597, 4_000_000],
        [2_097_597, 5_027_403],
        [3_000_000, 5_027_403],
        [3_000_000, 4_000_000],
    ]

    hierarchical = compact["records"][8]
    assert hierarchical["text"] == "${A}"
    assert hierarchical["operations"][0]["text"] == "${A}"
    assert hierarchical["operations"][0]["x"] == 5_000_000
    assert hierarchical["operations"][0]["y"] == 8_200_000
    assert hierarchical["operations"][1]["points"] == [
        [5_000_000, 6_000_000],
        [5_500_000, 5_500_000],
        [5_500_000, 5_000_000],
        [4_500_000, 5_000_000],
        [4_500_000, 5_500_000],
        [5_000_000, 6_000_000],
    ]

    netclass = compact["records"][9]
    assert [operation["kind"] for operation in netclass["operations"]] == [
        "ThickSegment",
        "Circle",
        "Text",
    ]
    assert netclass["operations"][0]["end_x"] == 9_644_400
    assert netclass["operations"][1]["diameter_nm"] == 711_200
    assert netclass["operations"][2]["text"] == "Net Class: ${B}"
    assert all(
        operation.get("width_nm", operation.get("pen_width_nm")) in {0, 203_200}
        for operation in netclass["operations"]
    )

    ordinary = compact["records"][10]
    # Built-in TITLE is resolved from the title block before the colliding
    # project value, while variable matching remains exact-case and one-pass.
    assert ordinary["text"] == "\n${B}-${UNKNOWN}--PX-${title}"
    assert ordinary["operations"][0]["text"] == ordinary["text"]
    assert (ordinary["operations"][0]["x"], ordinary["operations"][0]["y"]) == (
        9_400_000,
        9_750_000,
    )

    text_box = compact["records"][11]
    assert text_box["text"] == "first\n\nsecond\n"
    assert [operation["kind"] for operation in text_box["operations"]] == [
        "Rect",
        "Rect",
        "Text",
        "Text",
    ]
    assert text_box["operations"][0]["fill_color"] == "#01020380"
    assert text_box["operations"][0]["width_nm"] == 0
    assert text_box["operations"][1]["fill"] == "NO_FILL"
    assert [operation["text"] for operation in text_box["operations"][2:]] == [
        "first",
        "second",
    ]
    assert [operation["x"] for operation in text_box["operations"][2:]] == [
        10_000_000,
        13_360_000,
    ]
    assert all(
        operation["context"]["hyperlink"]["href"] == "https://example.test/box"
        for operation in text_box["operations"][2:]
    )

    default_header = _vector(payload, "default-worksheet-header-only")["expected"]
    assert default_header["total_operations"] == 59
    assert [record["kind"] for record in default_header["records"]] == [
        "sheet_header"
    ]

    bitmap = _vector(payload, "valid-bitmap-and-transparent-junction")["expected"]
    assert bitmap["total_operations"] == 3
    assert [record["kind"] for record in bitmap["records"]] == [
        "sheet_header",
        "junction",
    ]
    image = bitmap["records"][0]["operations"][1]
    assert image == {
        "kind": "PlotImage",
        "index": 1,
        "x": 3_000_000,
        "y": 4_000_000,
        "width_nm": 84_667,
        "height_nm": 84_667,
        "scale": 1.0,
        "image_data_b64": (
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8A"
            "AQUBAScY42YAAAAASUVORK5CYII="
        ),
        "image_format": "png",
        "stroke_color": "#840000FF",
    }
    transparent = bitmap["records"][1]
    assert transparent["color"] is None
    assert transparent["operations"][0]["stroke_color"] == "#009600FF"
    assert transparent["operations"][0]["fill_color"] == "#009600FF"

    metric_vector = _vector(
        payload, "explicit-font-metrics-for-schematic-annotations"
    )
    assert metric_vector["font_resource"] == {
        "face": "KiCad Monkey Shaping Fixture",
        "bold": False,
        "italic": False,
        "font_path": "tests/parity/fonts/shaping-variable-fixture.ttf",
        "font_sha256": (
            "faa68bc8dee69291f89b181de3caa97172ac346900af996a9f5adc9045119e36"
        ),
        "shaping_case_id": "fixture_default_variation_axis",
    }
    metric = metric_vector["expected"]
    assert metric["total_operations"] == 7
    assert [record["kind"] for record in metric["records"]] == [
        "sheet_header",
        "global_label",
        "text",
        "text_box",
    ]
    assert metric["records"][1]["operations"][1]["points"] == [
        [1_000_000, 2_000_000],
        [1_000_000, 3_027_403],
        [3_301_403, 3_027_403],
        [3_301_403, 2_000_000],
        [3_301_403, 972_597],
        [1_000_000, 972_597],
        [1_000_000, 2_000_000],
    ]
    assert metric["records"][2]["operations"][0]["y"] == 3_588_900
    assert [
        (operation["text"], operation["x"], operation["y"])
        for operation in metric["records"][3]["operations"][1:]
    ] == [
        ("AB", 6_250_000, 6_660_000),
        ("AB", 6_250_000, 8_340_000),
    ]

    symbols = _vector(
        payload, "placed-symbols-pins-fields-dnp-and-overplots"
    )["expected"]
    assert symbols["total_operations"] == 37
    assert [record["kind"] for record in symbols["records"]] == [
        "sheet_header",
        "symbol_instance",
        "symbol_instance",
        "symbol_overplot",
        "symbol_overplot",
    ]
    first = symbols["records"][1]
    assert first["reference"] == "U7"
    assert first["mirror"] is None
    assert [operation["kind"] for operation in first["operations"][3:8]] == [
        "StartBlock",
        "PlotPoly",
        "Text",
        "Text",
        "EndBlock",
    ]
    assert first["operations"][3]["extra_attrs"] == {
        "primitive": "pin",
        "object-type": "pin",
        "pin": "1",
        "symbol-uuid": "placed-1",
        "designator": "U?",
        "lib-pin-uuid": "lib-pin-1",
    }
    assert first["operations"][9]["context"]["hyperlink"]["href"] == (
        "https://example.test/part"
    )
    assert [operation["stroke_color"] for operation in first["operations"][-2:]] == [
        "#DC090DD9",
        "#DC090DD9",
    ]
    assert symbols["records"][3]["uuid"] == "placed-1:overplot"
    assert symbols["records"][3]["operations"][2]["label"] == (
        "placed-pin-1__overplot"
    )
    second = symbols["records"][2]
    assert (second["at_angle_deg"], second["mirror"]) == (90.0, "x")
    assert second["operations"][4]["points"] == [
        [10_000_000, 9_000_000],
        [10_000_000, 8_000_000],
    ]
    assert (
        second["operations"][5]["orient_deg"],
        second["operations"][6]["h_align"],
    ) == (90.0, "GR_TEXT_H_ALIGN_RIGHT")
    assert first["operations"][9]["y"] == 12_462_900

    graphics_vector = _vector(
        payload, "schematic-graphics-rules-images-and-table-family-order"
    )
    assert graphics_vector["image_resources"] == [
        {
            "image_format": "png",
            "image_sha256": (
                "c2bfda6df6b855b24bb53c7131a8af317723ec31fd901b4a6cd8257df81c8cd2"
            ),
        },
        {
            "image_format": "jpeg",
            "image_sha256": (
                "6a99497d81003845d473989d76613e2b3d92e0b9a8c33e303e402fc593e3bb66"
            ),
        },
        {
            "image_format": "bmp",
            "image_sha256": (
                "3e213bd3ae10450193c8f943ce09f97a6c03800dac7614c2788f7eed361e70e0"
            ),
        },
    ]
    graphics = graphics_vector["expected"]
    assert graphics["total_operations"] == 25
    assert [record["kind"] for record in graphics["records"]] == [
        "sheet_header",
        "graphic_polyline",
        "graphic_arc",
        "graphic_circle",
        "graphic_rectangle",
        "graphic_bezier",
        "rule_area",
        "rule_area",
        "rule_area",
        "rule_area",
        "rule_area",
        "image",
        "image",
        "image",
        "table",
    ]
    assert [record["uuid"] for record in graphics["records"][1:]] == [
        "graphic-polyline",
        "graphic-arc",
        "graphic-circle",
        "graphic-rectangle",
        "graphic-bezier",
        "rule-bezier",
        "rule-circle",
        "rule-rectangle",
        "rule-arc",
        "rule-polyline",
        "image-bmp",
        "image-jpeg",
        "image-png",
        "graphics-table",
    ]
    polyline, arc, circle, rectangle, bezier = graphics["records"][1:6]
    assert polyline["operations"][0]["points"] == [
        [1_000_000, 1_000_000],
        [2_000_000, 1_000_000],
        [2_000_000, 2_000_000],
    ]
    assert polyline["operations"][0]["width_nm"] == 152_400
    assert polyline["operations"][0]["line_style"] == "DASH_DOT_DOT"
    assert [operation["fill"] for operation in arc["operations"]] == [
        "FILLED_WITH_BG_BODYCOLOR",
        "NO_FILL",
    ]
    assert arc["operations"][0]["width_nm"] == 0
    assert arc["operations"][0]["fill_color"] == "#F5F4EFFF"
    assert circle["operations"][0]["fill"] == "FILLED_SHAPE"
    assert circle["operations"][0]["width_nm"] == 0
    assert circle["operations"][0]["stroke_color"] == "#0000C2FF"
    assert [operation["fill"] for operation in rectangle["operations"]] == [
        "FILLED_WITH_COLOR",
        "NO_FILL",
    ]
    assert rectangle["operations"][0]["fill_color"] == "#00FFFFFF"
    assert rectangle["operations"][1]["corner_radius_nm"] == 500_000
    assert bezier["operations"][0]["kind"] == "BezierCurve"
    assert "fill" not in bezier["operations"][0]
    assert "fill_color" not in bezier["operations"][0]

    rules = graphics["records"][6:11]
    assert [record["shape"] for record in rules] == [
        "bezier",
        "circle",
        "rectangle",
        "arc",
        "polyline",
    ]
    assert [record["operation_count"] for record in rules] == [1, 2, 2, 1, 1]
    assert {
        key: rules[2][key]
        for key in ("locked", "exclude_from_sim", "in_bom", "on_board", "dnp")
    } == {
        "locked": True,
        "exclude_from_sim": True,
        "in_bom": False,
        "on_board": False,
        "dnp": True,
    }
    assert rules[4]["operations"][0]["points"] == [
        [12_000_000, 13_000_000],
        [14_000_000, 13_000_000],
        [14_000_000, 15_000_000],
        [12_000_000, 13_000_000],
    ]

    images = graphics["records"][11:14]
    assert [record["image_format"] for record in images] == ["bmp", "jpeg", "png"]
    assert [
        (record["width_nm"], record["height_nm"], record["scale"])
        for record in images
    ] == [
        (357_746, 268_310, 0.5),
        (1_587_500, 1_058_333, 1.5),
        (1_058_333, 1_587_500, 2.0),
    ]
    for record in images:
        operation = record["operations"][0]
        assert operation["image_format"] == record["image_format"]
        assert operation["width_nm"] == record["width_nm"]
        assert operation["height_nm"] == record["height_nm"]
        assert operation["scale"] == record["scale"]

    table = graphics["records"][14]
    assert table["cell_count"] == 3
    assert [operation["kind"] for operation in table["operations"]] == [
        "Rect",
        "Rect",
        "Text",
        "Rect",
        "Rect",
        "Text",
        "Text",
    ]
    assert [
        operation["text"]
        for operation in table["operations"]
        if operation["kind"] == "Text"
    ] == ["Graphics-lower-project", "first", "second"]
    assert table["operations"][2]["context"]["hyperlink"]["href"] == (
        "https://example.test/cell"
    )
    assert all(
        "render_cache" not in operation and "render_cache_polygons" not in operation
        for operation in table["operations"]
    )

    metric_table_vector = _vector(
        payload, "explicit-font-metrics-for-schematic-table"
    )
    assert metric_table_vector["font_resource"] == metric_vector["font_resource"]
    metric_table = metric_table_vector["expected"]
    assert metric_table["total_operations"] == 4
    assert [record["kind"] for record in metric_table["records"]] == [
        "sheet_header",
        "table",
    ]
    assert metric_table["records"][1]["cell_count"] == 1
    assert [
        (operation["text"], operation["x"], operation["y"])
        for operation in metric_table["records"][1]["operations"][1:]
    ] == [
        ("AB", 6_250_000, 6_660_000),
        ("AB", 6_250_000, 8_340_000),
    ]

    # The established contract is intentionally forward tolerant, while this
    # promoted slice rejects fields and vocabulary it has not implemented.
    future = _clone(compact)
    future["future_field"] = {"ignored_by_generic_consumer": True}
    Draft202012Validator(established_schema).validate(future)
    with pytest.raises(msgspec.ValidationError):
        decode_schematic_plot_document_a0(json.dumps(future).encode("utf-8"))


def test_schematic_contract_rejects_noncanonical_structure_and_semantics() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    compact = _vector(
        payload, "custom-worksheet-connectivity-and-annotation-family-order"
    )["expected"]
    graphics = _vector(
        payload, "schematic-graphics-rules-images-and-table-family-order"
    )["expected"]
    symbols = _vector(
        payload, "placed-symbols-pins-fields-dnp-and-overplots"
    )["expected"]

    mutations = []

    unknown_record = _clone(compact)
    unknown_record["records"][1]["kind"] = "future_wire"
    mutations.append(unknown_record)

    unknown_operation = _clone(compact)
    unknown_operation["records"][1]["operations"][0]["kind"] = "Future"
    mutations.append(unknown_operation)

    malformed_point = _clone(compact)
    malformed_point["records"][1]["operations"][0]["points"][0] = [0]
    mutations.append(malformed_point)

    missing_canvas = _clone(compact)
    del missing_canvas["canvas"]
    mutations.append(missing_canvas)

    wrong_order = _clone(compact)
    wrong_order["records"][1:3] = reversed(wrong_order["records"][1:3])
    mutations.append(wrong_order)

    wrong_identity = _clone(compact)
    wrong_identity["records"][1]["object_id"] = "not-w"
    mutations.append(wrong_identity)

    wrong_local_index = _clone(compact)
    wrong_local_index["records"][5]["operations"][1]["index"] = 2
    mutations.append(wrong_local_index)

    wrong_total = _clone(compact)
    wrong_total["total_operations"] -= 1
    mutations.append(wrong_total)

    wrong_annotation_phase = _clone(compact)
    wrong_annotation_phase["records"][6:8] = reversed(
        wrong_annotation_phase["records"][6:8]
    )
    mutations.append(wrong_annotation_phase)

    local_with_decoration = _clone(compact)
    local_with_decoration["records"][6]["operations"].append(
        _clone(local_with_decoration["records"][7]["operations"][1])
    )
    local_with_decoration["records"][6]["operations"][1]["index"] = 1
    local_with_decoration["records"][6]["operation_count"] = 2
    local_with_decoration["total_operations"] += 1
    mutations.append(local_with_decoration)

    reversed_global_ops = _clone(compact)
    reversed_global_ops["records"][7]["operations"].reverse()
    for index, operation in enumerate(reversed_global_ops["records"][7]["operations"]):
        operation["index"] = index
    mutations.append(reversed_global_ops)

    reversed_netclass_marker = _clone(compact)
    reversed_netclass_marker["records"][9]["operations"][0:2] = reversed(
        reversed_netclass_marker["records"][9]["operations"][0:2]
    )
    for index, operation in enumerate(reversed_netclass_marker["records"][9]["operations"]):
        operation["index"] = index
    mutations.append(reversed_netclass_marker)

    text_box_text_first = _clone(compact)
    operations = text_box_text_first["records"][11]["operations"]
    operations.insert(0, operations.pop(2))
    for index, operation in enumerate(operations):
        operation["index"] = index
    mutations.append(text_box_text_first)

    wrong_graphics_phase = _clone(graphics)
    wrong_graphics_phase["records"][1:3] = reversed(
        wrong_graphics_phase["records"][1:3]
    )
    mutations.append(wrong_graphics_phase)

    reversed_graphic_fill = _clone(graphics)
    reversed_graphic_fill["records"][2]["operations"].reverse()
    for index, operation in enumerate(
        reversed_graphic_fill["records"][2]["operations"]
    ):
        operation["index"] = index
    mutations.append(reversed_graphic_fill)

    wrong_rule_shape = _clone(graphics)
    wrong_rule_shape["records"][6]["shape"] = "circle"
    mutations.append(wrong_rule_shape)

    unclosed_rule_polyline = _clone(graphics)
    unclosed_rule_polyline["records"][10]["operations"][0]["points"].pop()
    mutations.append(unclosed_rule_polyline)

    mismatched_image_extent = _clone(graphics)
    mismatched_image_extent["records"][11]["width_nm"] += 1
    mutations.append(mismatched_image_extent)

    invalid_image_payload = _clone(graphics)
    invalid_image_payload["records"][12]["operations"][0]["image_data_b64"] = "%%%"
    mutations.append(invalid_image_payload)

    wrong_table_cell_count = _clone(graphics)
    wrong_table_cell_count["records"][14]["cell_count"] = 2
    mutations.append(wrong_table_cell_count)

    table_text_first = _clone(graphics)
    table_operations = table_text_first["records"][14]["operations"]
    table_operations.insert(0, table_operations.pop(2))
    for index, operation in enumerate(table_operations):
        operation["index"] = index
    mutations.append(table_text_first)

    table_with_cache = _clone(graphics)
    table_with_cache["records"][14]["operations"][2]["render_cache_polygons"] = [
        [[0, 0], [1, 0], [0, 1]]
    ]
    mutations.append(table_with_cache)

    wrong_symbol_identity = _clone(symbols)
    wrong_symbol_identity["records"][1]["object_id"] = "other"
    mutations.append(wrong_symbol_identity)

    wrong_symbol_phase = _clone(symbols)
    wrong_symbol_phase["records"][2:4] = reversed(
        wrong_symbol_phase["records"][2:4]
    )
    mutations.append(wrong_symbol_phase)

    wrong_pin_parent = _clone(symbols)
    wrong_pin_parent["records"][1]["operations"][3]["extra_attrs"][
        "symbol-uuid"
    ] = "placed-2"
    mutations.append(wrong_pin_parent)

    foreign_pin_attr = _clone(symbols)
    foreign_pin_attr["records"][1]["operations"][3]["extra_attrs"]["foreign"] = "x"
    mutations.append(foreign_pin_attr)

    pin_hyperlink = _clone(symbols)
    pin_hyperlink["records"][1]["operations"][5]["context"] = {
        "hyperlink": {"href": "https://example.test/pin"}
    }
    mutations.append(pin_hyperlink)

    wrong_overplot_uuid = _clone(symbols)
    wrong_overplot_uuid["records"][3]["uuid"] = "wrong:overplot"
    mutations.append(wrong_overplot_uuid)

    for mutation in mutations:
        with pytest.raises(msgspec.ValidationError):
            decode_schematic_plot_document_a0(json.dumps(mutation).encode("utf-8"))


def test_python_oracle_uses_injected_context_without_path_discovery() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    vectors = [
        _vector(payload, "custom-worksheet-connectivity-and-annotation-family-order"),
        _vector(payload, "explicit-font-metrics-for-schematic-annotations"),
        _vector(
            payload, "schematic-graphics-rules-images-and-table-family-order"
        ),
        _vector(payload, "explicit-font-metrics-for-schematic-table"),
        _vector(payload, "placed-symbols-pins-fields-dnp-and-overplots"),
    ]
    discovery_helpers = (
        "_project_file_for_schematic_path",
        "_project_raw_for_schematic_path",
        "_resolve_project_layout_file_near_schematic",
        "_embedded_file_text_from_schematic_path",
        "_register_embedded_fonts_from_schematic_path",
    )
    patches = [
        patch.object(
            schematic_ir,
            name,
            side_effect=AssertionError(f"unexpected path discovery through {name}"),
        )
        for name in discovery_helpers
    ]
    for active_patch in patches:
        active_patch.start()
    try:
        for vector in vectors:
            assert expected_for(vector) == vector["expected"]
    finally:
        for active_patch in reversed(patches):
            active_patch.stop()


def test_rust_core_consumes_the_shared_schematic_vector() -> None:
    _run([sys.executable, "scripts/generate_schematic_plotter_vectors.py", "--check"])
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust schematic plotter gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-core",
            "--test",
            "schematic_plotter_slice",
        ]
    )
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-contracts",
            "--test",
            "schematic_plot_contracts",
        ]
    )

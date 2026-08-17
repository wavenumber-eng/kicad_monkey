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

from kicad_monkey import KiCadSchematic, kicad_schematic_to_ir as schematic_ir
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

    sheets_vector = _vector(payload, "hierarchical-sheets-follow-symbol-overplots")
    assert sheets_vector["font_resource"] == metric_vector["font_resource"]
    assert sheets_vector["source"].index("  (sheet\n") < sheets_vector[
        "source"
    ].index("  (lib_symbols")
    sheets = sheets_vector["expected"]
    assert sheets["total_operations"] == 67
    assert [record["kind"] for record in sheets["records"]] == [
        "sheet_header",
        "symbol_instance",
        "symbol_instance",
        "symbol_overplot",
        "symbol_overplot",
        "sheet",
        "sheet",
    ]
    rich, clear = sheets["records"][-2:]
    assert {
        key: rich[key]
        for key in (
            "uuid",
            "object_id",
            "sheet_name",
            "sheet_file",
            "at_x_nm",
            "at_y_nm",
            "size_x_nm",
            "size_y_nm",
            "dnp",
        )
    } == {
        "uuid": "sheet-rich",
        "object_id": "Child",
        "sheet_name": "Child",
        "sheet_file": "child.kicad_sch",
        "at_x_nm": 4_000_000,
        "at_y_nm": 20_000_000,
        "size_x_nm": 8_000_000,
        "size_y_nm": 6_000_000,
        "dnp": True,
    }
    assert not {
        "exclude_from_sim",
        "in_bom",
        "on_board",
        "fields_autoplaced",
        "instances",
        "variants",
    }.intersection(rich)
    assert [operation["index"] for operation in rich["operations"]] == list(
        range(27)
    )
    assert [operation["kind"] for operation in rich["operations"]] == [
        "Rect",
        "Rect",
        *(["StartBlock", "Text", "PlotPoly", "EndBlock"] * 5),
        "Text",
        "Text",
        "Text",
        "ThickSegment",
        "ThickSegment",
    ]
    assert rich["operations"][0] == {
        "kind": "Rect",
        "index": 0,
        "x1": 4_000_000,
        "y1": 20_000_000,
        "x2": 12_000_000,
        "y2": 26_000_000,
        "fill": "FILLED_SHAPE",
        "width_nm": 0,
        "corner_radius_nm": 0,
        "stroke_color": "#F0E6DC80",
        "fill_color": "#F0E6DC80",
    }
    assert {
        key: rich["operations"][1][key]
        for key in ("fill", "width_nm", "stroke_color", "line_style")
    } == {
        "fill": "NO_FILL",
        "width_nm": 200_000,
        "stroke_color": "#8C8B89FF",
        "line_style": "DASH",
    }
    pin_starts = rich["operations"][2:22:4]
    assert [operation["label"] for operation in pin_starts] == [
        "sheet-pin-bus",
        "sheet-pin-out",
        "sheet-pin-bidi",
        "sheet-pin-tri",
        "sheet-rich__sheet_pin__PASS",
    ]
    assert [operation["extra_attrs"]["shape"] for operation in pin_starts] == [
        "input",
        "output",
        "bidirectional",
        "tri_state",
        "passive",
    ]
    assert all(
        operation["extra_attrs"]["sheet-uuid"] == "sheet-rich"
        for operation in pin_starts
    )
    pin_texts = rich["operations"][3:22:4]
    assert [
        (
            operation["text"],
            operation["x"],
            operation["y"],
            operation["orient_deg"],
        )
        for operation in pin_texts
    ] == [
        ("BUS{0..1}", 5_150_000, 22_000_000, 0.0),
        ("OUT/N", 10_850_000, 23_000_000, 0.0),
        ("BIDI", 6_000_000, 21_150_000, 90.0),
        ("TRI", 8_000_000, 24_850_000, 90.0),
        ("PASS", 11_150_000, 20_000_000, 0.0),
    ]
    assert pin_texts[0]["color"] == "#9C9B99FF"
    assert pin_texts[1]["context"]["hyperlink"]["href"] == (
        "https://example.test/sheet-pin"
    )
    assert [operation["points"] for operation in rich["operations"][4:22:4]] == [
        [
            [5_000_000, 22_000_000],
            [4_500_000, 21_500_000],
            [4_000_000, 21_500_000],
            [4_000_000, 22_500_000],
            [4_500_000, 22_500_000],
            [5_000_000, 22_000_000],
        ],
        [
            [12_000_000, 23_000_000],
            [11_500_000, 22_500_000],
            [11_000_000, 22_500_000],
            [11_000_000, 23_500_000],
            [11_500_000, 23_500_000],
            [12_000_000, 23_000_000],
        ],
        [
            [6_000_000, 20_000_000],
            [5_500_000, 20_500_000],
            [6_000_000, 21_000_000],
            [6_500_000, 20_500_000],
            [6_000_000, 20_000_000],
        ],
        [
            [8_000_000, 26_000_000],
            [7_500_000, 25_500_000],
            [8_000_000, 25_000_000],
            [8_500_000, 25_500_000],
            [8_000_000, 26_000_000],
        ],
        [
            [10_000_000, 19_500_000],
            [11_000_000, 19_500_000],
            [11_000_000, 20_500_000],
            [10_000_000, 20_500_000],
            [10_000_000, 19_500_000],
        ],
    ]
    fields = rich["operations"][22:25]
    assert [operation["text"] for operation in fields] == [
        "Sheetname: Child",
        "File: child.kicad_sch",
        "${PROJECT}",
    ]
    assert fields[0]["context"]["hyperlink"]["href"] == (
        "https://example.test/sheet"
    )
    assert all(operation["text"] not in {"ignored", ""} for operation in fields)
    assert [operation["stroke_color"] for operation in rich["operations"][-2:]] == [
        "#DC090DD9",
        "#DC090DD9",
    ]
    assert [
        (
            operation["start_x"],
            operation["start_y"],
            operation["end_x"],
            operation["end_y"],
            operation["width_nm"],
        )
        for operation in rich["operations"][-2:]
    ] == [
        (193_090, 18_857_927, 15_806_910, 27_142_073, 457_200),
        (15_806_910, 18_857_927, 193_090, 27_142_073, 457_200),
    ]
    assert clear["dnp"] is False
    assert clear["operation_count"] == 3
    first_outline, second_outline = clear["operations"][:2]
    assert first_outline["fill"] == second_outline["fill"] == "NO_FILL"
    assert {k: v for k, v in first_outline.items() if k != "index"} == {
        k: v for k, v in second_outline.items() if k != "index"
    }

    undecorated_vector = _vector(
        payload, "hierarchical-sheet-undecorated-shapes-and-zero-width-border"
    )
    assert "font_resource" not in undecorated_vector
    assert "(stroke (width -1) (type dot))" in undecorated_vector["source"]
    undecorated = undecorated_vector["expected"]
    assert undecorated["total_operations"] == 15
    assert [record["kind"] for record in undecorated["records"]] == [
        "sheet_header",
        "sheet",
    ]
    directive_sheet = undecorated["records"][1]
    assert {
        key: directive_sheet[key]
        for key in (
            "uuid",
            "object_id",
            "sheet_name",
            "sheet_file",
            "at_x_nm",
            "at_y_nm",
            "size_x_nm",
            "size_y_nm",
            "dnp",
        )
    } == {
        "uuid": "sheet-undecorated",
        "object_id": "DirectiveShapes",
        "sheet_name": "DirectiveShapes",
        "sheet_file": "directive-shapes.kicad_sch",
        "at_x_nm": 2_000_000,
        "at_y_nm": 2_000_000,
        "size_x_nm": 10_000_000,
        "size_y_nm": 8_000_000,
        "dnp": False,
    }
    assert directive_sheet["operation_count"] == 14
    assert [operation["index"] for operation in directive_sheet["operations"]] == list(
        range(14)
    )
    assert [operation["kind"] for operation in directive_sheet["operations"]] == [
        "Rect",
        "Rect",
        *(["StartBlock", "Text", "EndBlock"] * 4),
    ]
    first_zero, second_zero = directive_sheet["operations"][:2]
    assert (first_zero["width_nm"], first_zero["line_style"]) == (0, "DOT")
    assert {key: value for key, value in first_zero.items() if key != "index"} == {
        key: value for key, value in second_zero.items() if key != "index"
    }
    directive_starts = directive_sheet["operations"][2::3]
    assert [operation["label"] for operation in directive_starts] == [
        "sheet-pin-dot",
        "sheet-pin-round",
        "sheet-pin-diamond",
        "sheet-pin-rectangle",
    ]
    assert [operation["extra_attrs"]["shape"] for operation in directive_starts] == [
        "dot",
        "round",
        "diamond",
        "rectangle",
    ]
    assert all(
        operation["extra_attrs"]["sheet-uuid"] == "sheet-undecorated"
        for operation in directive_starts
    )
    directive_texts = directive_sheet["operations"][3::3]
    assert [
        (operation["text"], operation["x"], operation["y"], operation["orient_deg"])
        for operation in directive_texts
    ] == [
        ("DOT", 3_150_000, 3_000_000, 0.0),
        ("ROUND", 10_850_000, 4_000_000, 0.0),
        ("DIAMOND", 5_000_000, 3_150_000, 90.0),
        ("RECTANGLE", 8_000_000, 8_850_000, 90.0),
    ]
    assert all(operation["color"] == "#006464FF" for operation in directive_texts)
    assert all(operation["font_face"] == "Arial" for operation in directive_texts)
    assert not any(
        operation["kind"] == "PlotPoly" for operation in directive_sheet["operations"]
    )

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
    sheets = _vector(
        payload, "hierarchical-sheets-follow-symbol-overplots"
    )["expected"]
    undecorated_sheets = _vector(
        payload, "hierarchical-sheet-undecorated-shapes-and-zero-width-border"
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

    wrong_sheet_phase = _clone(sheets)
    wrong_sheet_phase["records"][4:6] = reversed(
        wrong_sheet_phase["records"][4:6]
    )
    mutations.append(wrong_sheet_phase)

    wrong_sheet_identity = _clone(sheets)
    wrong_sheet_identity["records"][5]["object_id"] = "Other"
    mutations.append(wrong_sheet_identity)

    reversed_sheet_body = _clone(sheets)
    reversed_sheet_body["records"][5]["operations"][0:2] = reversed(
        reversed_sheet_body["records"][5]["operations"][0:2]
    )
    for index, operation in enumerate(
        reversed_sheet_body["records"][5]["operations"]
    ):
        operation["index"] = index
    mutations.append(reversed_sheet_body)

    mismatched_transparent_outline = _clone(sheets)
    mismatched_transparent_outline["records"][6]["operations"][1]["x2"] += 1
    mutations.append(mismatched_transparent_outline)

    text_before_sheet_pin_block = _clone(sheets)
    pin_operations = text_before_sheet_pin_block["records"][5]["operations"]
    pin_operations[2:4] = reversed(pin_operations[2:4])
    for index, operation in enumerate(pin_operations):
        operation["index"] = index
    mutations.append(text_before_sheet_pin_block)

    wrong_sheet_pin_parent = _clone(sheets)
    wrong_sheet_pin_parent["records"][5]["operations"][2]["extra_attrs"][
        "sheet-uuid"
    ] = "sheet-clear"
    mutations.append(wrong_sheet_pin_parent)

    wrong_sheet_pin_shape = _clone(sheets)
    wrong_sheet_pin_shape["records"][5]["operations"][10]["extra_attrs"][
        "shape"
    ] = "input"
    mutations.append(wrong_sheet_pin_shape)

    malformed_sheet_pin_decoration = _clone(sheets)
    malformed_sheet_pin_decoration["records"][5]["operations"][20]["points"][
        0
    ][0] += 1
    mutations.append(malformed_sheet_pin_decoration)

    inconsistent_sheet_dnp = _clone(sheets)
    inconsistent_sheet_dnp["records"][5]["dnp"] = False
    mutations.append(inconsistent_sheet_dnp)

    malformed_sheet_dnp_marker = _clone(sheets)
    malformed_sheet_dnp_marker["records"][5]["operations"][-1][
        "width_nm"
    ] += 1
    mutations.append(malformed_sheet_dnp_marker)

    leaked_sheet_instance_metadata = _clone(sheets)
    leaked_sheet_instance_metadata["records"][5]["exclude_from_sim"] = True
    mutations.append(leaked_sheet_instance_metadata)

    decorated_shape_without_decoration = _clone(undecorated_sheets)
    decorated_shape_without_decoration["records"][1]["operations"][2][
        "extra_attrs"
    ]["shape"] = "input"
    mutations.append(decorated_shape_without_decoration)

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
        _vector(payload, "hierarchical-sheets-follow-symbol-overplots"),
        _vector(
            payload,
            "hierarchical-sheet-undecorated-shapes-and-zero-width-border",
        ),
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
    patches.append(
        patch.object(
            KiCadSchematic,
            "_load_sub_sheets",
            side_effect=AssertionError("unexpected hierarchical sheet discovery"),
        )
    )
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

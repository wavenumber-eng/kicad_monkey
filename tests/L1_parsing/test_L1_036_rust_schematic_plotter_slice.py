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

    for mutation in mutations:
        with pytest.raises(msgspec.ValidationError):
            decode_schematic_plot_document_a0(json.dumps(mutation).encode("utf-8"))


def test_python_oracle_uses_injected_context_without_path_discovery() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    vectors = [
        _vector(payload, "custom-worksheet-connectivity-and-annotation-family-order"),
        _vector(payload, "explicit-font-metrics-for-schematic-annotations"),
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

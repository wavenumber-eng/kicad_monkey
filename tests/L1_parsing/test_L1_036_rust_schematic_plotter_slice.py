"""Rack ownership for the Rust schematic-to-plotter-IR foundation slice."""

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

    compact = payload["vectors"][0]["expected"]
    assert compact["total_operations"] == 8
    assert [record["kind"] for record in compact["records"]] == [
        "sheet_header",
        "wire",
        "bus",
        "bus_entry",
        "junction",
        "no_connect",
    ]
    assert compact["records"][0]["operations"][1]["text"] == "PX-PX-2/3-Child"
    assert compact["records"][4]["operations"][0]["diameter_nm"] == 914_400
    assert compact["records"][5]["operations"][0]["points"] == [
        [10_390_400, 11_390_400],
        [11_609_600, 12_609_600],
    ]

    default_header = payload["vectors"][1]["expected"]
    assert default_header["total_operations"] == 59
    assert [record["kind"] for record in default_header["records"]] == [
        "sheet_header"
    ]

    bitmap = payload["vectors"][2]["expected"]
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

    # The established contract is intentionally forward tolerant, while this
    # promoted slice rejects fields and vocabulary it has not implemented.
    future = _clone(compact)
    future["future_field"] = {"ignored_by_generic_consumer": True}
    Draft202012Validator(established_schema).validate(future)
    with pytest.raises(msgspec.ValidationError):
        decode_schematic_plot_document_a0(json.dumps(future).encode("utf-8"))


def test_schematic_contract_rejects_noncanonical_structure_and_semantics() -> None:
    compact = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))["vectors"][0][
        "expected"
    ]

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

    for mutation in mutations:
        with pytest.raises(msgspec.ValidationError):
            decode_schematic_plot_document_a0(json.dumps(mutation).encode("utf-8"))


def test_python_oracle_uses_injected_context_without_path_discovery() -> None:
    vector = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))["vectors"][0]
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

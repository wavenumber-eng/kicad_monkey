"""Rack ownership for the non-text Rust library-symbol plotter slice."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess

from jsonschema import Draft202012Validator
import msgspec
import pytest

from kicad_monkey import KiCadSymbolLib, lib_symbol_to_ir
from kicad_monkey.contracts.generated import decode_symbol_plot_document_a0


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTOR_PATH = PACKAGE_ROOT / "tests" / "parity" / "symbol_plotter_a0_vectors.json"
SLICE_SCHEMA_PATH = (
    PACKAGE_ROOT / "contracts" / "generated" / "schema" / "SymbolPlotDocument.json"
)
ESTABLISHED_SCHEMA_PATH = (
    PACKAGE_ROOT / "docs" / "contracts" / "kicad_plotter_ir_a0.schema.json"
)


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


def _contract_projection(document: dict) -> dict:
    projected = {
        key: document[key]
        for key in (
            "schema",
            "source_kind",
            "total_operations",
            "records",
            "source_path",
            "document_id",
            "coordinate_space",
        )
    }
    header = projected["records"][0]
    if header.get("extends") is None:
        del header["extends"]
    return _normalize_integral_coordinates(projected)


def _normalize_integral_coordinates(value):
    if isinstance(value, dict):
        return {key: _normalize_integral_coordinates(child) for key, child in value.items()}
    if isinstance(value, list):
        return [_normalize_integral_coordinates(child) for child in value]
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return value


def test_symbol_vector_matches_python_and_typespec_contract() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.symbol_plotter_parity.a0"
    slice_schema = json.loads(SLICE_SCHEMA_PATH.read_text(encoding="utf-8"))
    established_schema = json.loads(ESTABLISHED_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(slice_schema)

    for vector in payload["vectors"]:
        library = KiCadSymbolLib.from_text(vector["source"])
        symbol = next(value for value in library.symbols if value.name == vector["symbol_name"])
        actual = lib_symbol_to_ir(
            symbol,
            unit=vector["unit"],
            style=vector["style"],
            source_path=vector["source_path"],
            document_id=vector["document_id"],
        ).to_dict()
        projected = _contract_projection(actual)
        assert projected == vector["expected"], vector["id"]
        Draft202012Validator(slice_schema).validate(projected)
        Draft202012Validator(established_schema).validate(projected)
        decoded = decode_symbol_plot_document_a0(json.dumps(projected).encode())
        assert json.loads(msgspec.json.encode(decoded)) == projected


def test_symbol_semantic_validator_rejects_cross_domain_states() -> None:
    expected = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))["vectors"][0]["expected"]
    missing_header = json.loads(json.dumps(expected))
    missing_header["records"] = missing_header["records"][1:]
    with pytest.raises(msgspec.ValidationError, match="missing_symbol_header"):
        decode_symbol_plot_document_a0(json.dumps(missing_header).encode())

    layered = json.loads(json.dumps(expected))
    layered["records"][1]["operations"][0]["layer"] = "F.SilkS"
    with pytest.raises(msgspec.ValidationError, match="invalid_symbol_operation"):
        decode_symbol_plot_document_a0(json.dumps(layered).encode())

    pad = json.loads(json.dumps(expected))
    pad["records"][1]["operations"][0] = {
        "kind": "FlashPadCircle",
        "index": 0,
        "x": 0,
        "y": 0,
        "diameter_nm": 1,
        "layers": ["F.Cu"],
        "mask_margin_nm": 0,
    }
    with pytest.raises(msgspec.ValidationError, match="invalid_symbol_operation"):
        decode_symbol_plot_document_a0(json.dumps(pad).encode())


def _pin_style_source() -> str:
    styles = (
        "line",
        "inverted",
        "clock",
        "inverted_clock",
        "input_low",
        "clock_low",
        "output_low",
        "edge_clock_high",
        "non_logic",
    )
    pins = "".join(
        f'(pin passive {style} (at 0 {index * 2.54} 0) (length 2.54) '
        '(name "") (number ""))'
        for index, style in enumerate(styles)
    )
    return f'(kicad_symbol_lib (symbol "Pins" (symbol "Pins_1_1" {pins})))'


def test_every_non_text_pin_style_matches_python_geometry() -> None:
    library = KiCadSymbolLib.from_text(_pin_style_source())
    symbol = next(value for value in library.symbols if value.name == "Pins")
    operations = lib_symbol_to_ir(symbol, unit=1, style=0).to_dict()["records"][1][
        "operations"
    ]

    assert [operation["kind"] for operation in operations] == [
        "PlotPoly", "Circle", "PlotPoly", "PlotPoly", "PlotPoly",
        "Circle", "PlotPoly", "PlotPoly", "PlotPoly", "PlotPoly",
        "PlotPoly", "PlotPoly", "PlotPoly", "PlotPoly", "PlotPoly",
        "PlotPoly", "PlotPoly", "PlotPoly", "PlotPoly", "PlotPoly",
    ]
    assert [(operations[index]["cx"], operations[index]["cy"]) for index in (1, 5)] == [
        (1_905_000, -2_540_000),
        (1_905_000, -7_620_000),
    ]
    assert [
        operation["points"] for operation in operations if operation["kind"] == "PlotPoly"
    ] == [
        [[2_540_000, 0], [0, 0]],
        [[1_270_000, -2_540_000], [0, -2_540_000]],
        [[2_540_000, -5_080_000], [0, -5_080_000]],
        [[2_540_000, -4_445_000], [3_810_000, -5_080_000], [2_540_000, -5_715_000]],
        [[1_270_000, -7_620_000], [0, -7_620_000]],
        [[2_540_000, -6_985_000], [3_810_000, -7_620_000], [2_540_000, -8_255_000]],
        [[2_540_000, -10_160_000], [0, -10_160_000]],
        [[1_270_000, -10_160_000], [1_270_000, -11_430_000], [2_540_000, -10_160_000]],
        [[2_540_000, -12_700_000], [0, -12_700_000]],
        [[2_540_000, -12_065_000], [3_810_000, -12_700_000], [2_540_000, -13_335_000]],
        [[1_270_000, -12_700_000], [1_270_000, -13_970_000], [2_540_000, -12_700_000]],
        [[2_540_000, -15_240_000], [0, -15_240_000]],
        [[2_540_000, -16_510_000], [1_270_000, -15_240_000]],
        [[2_540_000, -17_145_000], [1_270_000, -17_780_000], [2_540_000, -18_415_000]],
        [[1_270_000, -17_780_000], [0, -17_780_000]],
        [[2_540_000, -20_320_000], [0, -20_320_000]],
        [[3_175_000, -20_955_000], [1_905_000, -19_685_000]],
        [[3_175_000, -19_685_000], [1_905_000, -20_955_000]],
    ]


def test_rust_symbol_core_and_host_adapter_are_rack_orchestrated() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust plotter-IR gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "symbol_plotter_slice",
        ]
    )
    _run([cargo, "test", "--locked", "--package", "kicad-monkey-wasm"])

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

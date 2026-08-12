"""Rack ownership for the first Rust footprint-to-plotter-IR slice."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess

from jsonschema import Draft202012Validator

from kicad_monkey.kicad_footprint import KiCadFootprint
from kicad_monkey.kicad_footprint_to_ir import footprint_to_ir


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTOR_PATH = PACKAGE_ROOT / "tests" / "parity" / "footprint_plotter_a0_vectors.json"
SLICE_SCHEMA_PATH = (
    PACKAGE_ROOT / "contracts" / "generated" / "schema" / "FootprintPlotDocument.json"
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


def test_shared_solid_line_vector_matches_python_and_both_ir_schemas() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.footprint_plotter_parity.a0"

    slice_schema = json.loads(SLICE_SCHEMA_PATH.read_text(encoding="utf-8"))
    established_schema = json.loads(ESTABLISHED_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(slice_schema)
    Draft202012Validator.check_schema(established_schema)
    safe_integer = slice_schema["$defs"]["JavaScriptSafeInteger"]
    assert safe_integer["minimum"] == -9_007_199_254_740_991
    assert safe_integer["maximum"] == 9_007_199_254_740_991

    for vector in payload["vectors"]:
        footprint = KiCadFootprint.from_string(vector["source"])
        actual = footprint_to_ir(
            footprint,
            source_path=vector["source_path"],
            document_id=vector["document_id"],
        ).to_dict()
        assert actual == vector["expected"], vector["id"]
        Draft202012Validator(slice_schema).validate(actual)
        Draft202012Validator(established_schema).validate(actual)

        unsafe = json.loads(json.dumps(actual))
        unsafe["version"] = 9_007_199_254_740_992
        assert list(Draft202012Validator(slice_schema).iter_errors(unsafe))


def test_rust_core_and_host_adapter_consume_the_shared_vector() -> None:
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
            "footprint_plotter_slice",
        ]
    )
    _run([cargo, "test", "--locked", "--package", "kicad-monkey-wasm"])

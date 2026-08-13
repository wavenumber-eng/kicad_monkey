"""Generated Python and TypeScript transport-projection gate."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    FootprintPlotDocumentA0,
    SExpressionBuildRequestA0,
    decode_footprint_plot_document_a0,
    decode_sexpr_build_request_a0,
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


@pytest.mark.parametrize("version", [-9_007_199_254_740_991, 9_007_199_254_740_991])
def test_python_plotter_projection_accepts_javascript_safe_boundaries(version: int) -> None:
    payload = json.dumps(
        {
            "schema": "kicad.plotter_ir.a0",
            "source_kind": "MOD",
            "total_operations": 0,
            "records": [],
            "document_id": "boundary",
            "coordinate_space": {"unit": "nm", "y_axis": "down"},
            "version": version,
            "generator": "pcbnew",
            "generator_version": "10.0",
        }
    ).encode()
    assert isinstance(decode_footprint_plot_document_a0(payload), FootprintPlotDocumentA0)


@pytest.mark.parametrize("version", [-9_007_199_254_740_992, 9_007_199_254_740_992])
def test_python_plotter_projection_rejects_unsafe_integer_neighbors(version: int) -> None:
    payload = json.dumps(
        {
            "schema": "kicad.plotter_ir.a0",
            "source_kind": "MOD",
            "total_operations": 0,
            "records": [],
            "document_id": "boundary",
            "coordinate_space": {"unit": "nm", "y_axis": "down"},
            "version": version,
            "generator": "pcbnew",
            "generator_version": "10.0",
        }
    ).encode()
    with pytest.raises(msgspec.ValidationError):
        decode_footprint_plot_document_a0(payload)


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

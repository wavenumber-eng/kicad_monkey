"""Generated Python and TypeScript transport-projection gate."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    SExpressionBuildRequestA0,
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

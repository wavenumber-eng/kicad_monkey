"""Generated source-bundle manifest contract gate."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import msgspec
import pytest
from jsonschema import Draft202012Validator

from kicad_monkey.contracts.generated import (
    SourceBundleManifestA0,
    decode_source_bundle_manifest_a0,
)

VECTORS = Path(__file__).resolve().parents[1] / "parity/source_bundle_a0_vectors.json"
SCHEMA = (
    Path(__file__).resolve().parents[2]
    / "contracts/generated/schema/SourceBundleManifest.json"
)
PACKAGE_ROOT = Path(__file__).resolve().parents[2]


def _run_capped_rust(command: list[str]) -> None:
    environment = os.environ.copy()
    environment["CARGO_BUILD_JOBS"] = "4"
    environment["RUST_TEST_THREADS"] = "2"
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
        env=environment,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )


def test_generated_source_bundle_manifest_is_strict_and_keeps_bytes_out_of_band() -> (
    None
):
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    manifest = vectors["manifest"]
    decoded = decode_source_bundle_manifest_a0(json.dumps(manifest).encode())
    assert isinstance(decoded, SourceBundleManifestA0)
    assert [source.slot for source in decoded.sources] == [0, 1]
    assert [source.source_bytes for source in decoded.sources] == ["2", "45"]
    assert "buffers_utf8" not in manifest

    unknown = {**manifest, "buffers": vectors["buffers_utf8"]}
    with pytest.raises(msgspec.ValidationError, match="unknown field"):
        decode_source_bundle_manifest_a0(json.dumps(unknown).encode())

    wrong_literal = {**manifest, "version": "a1"}
    with pytest.raises(msgspec.ValidationError):
        decode_source_bundle_manifest_a0(json.dumps(wrong_literal).encode())


def test_shared_integer_transport_cases_match_json_schema_and_python() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    validator = Draft202012Validator(schema)
    for case in vectors["transport_cases"]:
        candidate = json.loads(json.dumps(vectors["manifest"]))
        candidate["sources"][0][case["field"]] = case["value"]
        schema_valid = not list(validator.iter_errors(candidate))
        assert schema_valid is case.get("schema_valid", case["valid"]), case["id"]
        if case["valid"]:
            decode_source_bundle_manifest_a0(json.dumps(candidate).encode())
        else:
            with pytest.raises(msgspec.ValidationError):
                decode_source_bundle_manifest_a0(json.dumps(candidate).encode())


def test_native_source_bundle_contract_and_resource_evidence() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for native source-bundle evidence"
    _run_capped_rust(
        [
            cargo,
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-contracts",
            "--test",
            "generated_contracts",
            "source_bundle_integer_transport_matches_shared_boundaries_and_failures",
            "--",
            "--exact",
            "--test-threads",
            "2",
        ]
    )
    _run_capped_rust(
        [
            cargo,
            "test",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-core",
            "--test",
            "schematic_bundle",
            "manifest",
            "--",
            "--test-threads",
            "2",
        ]
    )

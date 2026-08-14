"""Generated source-bundle manifest contract gate."""

from __future__ import annotations

import json
from pathlib import Path

import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    SourceBundleManifestA0,
    decode_source_bundle_manifest_a0,
)

VECTORS = Path(__file__).resolve().parents[1] / "parity/source_bundle_a0_vectors.json"


def test_generated_source_bundle_manifest_is_strict_and_keeps_bytes_out_of_band() -> None:
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

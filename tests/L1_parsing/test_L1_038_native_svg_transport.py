"""Real Python-to-Rust gate for deterministic native base SVG."""

from __future__ import annotations

import json
import hashlib
import os
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

import pytest

from kicad_monkey.kicad_native import (
    KiCadNativeError,
    kicad_native_handshake,
    kicad_native_handshake_a1,
    native_render_svg,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
NATIVE_BINARY = PACKAGE_ROOT / "target" / "debug" / (
    "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
)


def _build_native_binary() -> None:
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = "4"
    env["RUST_TEST_THREADS"] = "2"
    completed = subprocess.run(
        ["cargo", "build", "--locked", "--jobs", "4", "--package", "kicad-monkey-native"],
        cwd=PACKAGE_ROOT,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=300,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr


def test_real_native_process_renders_a_frozen_document() -> None:
    _build_native_binary()
    vectors = json.loads(
        (PACKAGE_ROOT / "tests/parity/footprint_plotter_a0_vectors.json").read_text(
            encoding="utf-8"
        )
    )
    document = vectors["vectors"][0]["expected"]
    first = native_render_svg(
        document,
        document_kind="footprint",
        viewport={"min_x_nm": 0, "min_y_nm": -2_000_000, "width_nm": 2_000_000, "height_nm": 3_000_000},
        executable=NATIVE_BINARY,
    )
    second = native_render_svg(
        document,
        document_kind="footprint",
        viewport={"min_x_nm": 0, "min_y_nm": -2_000_000, "width_nm": 2_000_000, "height_nm": 3_000_000},
        executable=NATIVE_BINARY,
    )
    assert kicad_native_handshake(executable=NATIVE_BINARY)["operations"] == ["design-facts"]
    assert kicad_native_handshake_a1(executable=NATIVE_BINARY)["operations"] == [
        "design-facts",
        "render-svg",
    ]
    assert first == second
    assert first.source_kind == "MOD"
    assert first.document_id == "fixture"
    assert ET.fromstring(first.svg_utf8).tag == "{http://www.w3.org/2000/svg}svg"
    assert 'data-ref="footprint"' in first.svg_utf8


def test_every_frozen_document_matches_the_pinned_native_svg_snapshot() -> None:
    _build_native_binary()
    snapshots = json.loads(
        (PACKAGE_ROOT / "tests/parity/native_svg_a0_vectors.json").read_text(encoding="utf-8")
    )
    assert snapshots["case_count"] == 30
    assert snapshots["svg_case_count"] == 29
    assert snapshots["rejected_case_count"] == 1
    loaded: dict[str, dict[str, dict[str, object]]] = {}
    for case in snapshots["cases"]:
        source_name = case["source_vector"]
        source = loaded.get(source_name)
        if source is None:
            vectors = json.loads(
                (PACKAGE_ROOT / "tests/parity" / source_name).read_text(encoding="utf-8")
            )
            source = {vector["id"]: vector for vector in vectors["vectors"]}
            loaded[source_name] = source
        document = source[case["source_id"]]["expected"]
        canonical = json.dumps(
            document, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode()
        assert hashlib.sha256(canonical).hexdigest() == case["document_sha256"]
        if case["outcome"] == "rejected":
            with pytest.raises(KiCadNativeError, match=case["error_contains"]):
                native_render_svg(
                    document,
                    document_kind=case["producer"],
                    viewport=case["viewport"],
                    executable=NATIVE_BINARY,
                )
            continue
        assert case["outcome"] == "svg"
        result = native_render_svg(
            document,
            document_kind=case["producer"],
            viewport=case["viewport"],
            executable=NATIVE_BINARY,
        )
        assert result.svg_bytes == case["svg_bytes"]
        assert result.svg_sha256 == case["svg_sha256"]

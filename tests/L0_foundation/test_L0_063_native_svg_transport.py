"""Strict client and generated-contract tests for native base SVG."""

from __future__ import annotations

import hashlib
import json
from copy import deepcopy
from pathlib import Path

import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    decode_native_handshake_a1,
    decode_native_svg_render_request_a0,
    decode_native_svg_render_result_a0,
)
from kicad_monkey.kicad_native import (
    KiCadNativeError,
    kicad_native_handshake_a1,
    native_render_svg,
)


REPO_ROOT = Path(__file__).resolve().parents[2]


def _document() -> dict[str, object]:
    return {
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "MOD",
        "total_operations": 1,
        "records": [
            {
                "uuid": "line&one",
                "kind": "footprint",
                "object_id": "Demo",
                "operation_count": 1,
                "operations": [
                    {
                        "kind": "ThickSegment",
                        "index": 0,
                        "start_x": 0,
                        "start_y": 0,
                        "end_x": 1_000_000,
                        "end_y": 0,
                        "width_nm": 100_000,
                        "layer": "F.SilkS",
                    }
                ],
                "name": "Demo",
                "layer": "F.Cu",
                "locked": False,
                "placed": False,
                "descr": "",
                "tags": "",
                "attr": [],
            }
        ],
        "source_path": "demo.kicad_mod",
        "document_id": "demo",
        "coordinate_space": {"unit": "nm", "y_axis": "down"},
        "version": 20260101,
        "generator": "pcbnew",
        "generator_version": "10.0",
    }


def _svg_request(
    document: dict[str, object],
    *,
    kind: str = "footprint",
    viewport: dict[str, int] | None = None,
) -> dict[str, object]:
    return {
        "type": "kicad_monkey.native.svg.request",
        "version": "a0",
        "profile": "plotter-base-a0",
        "document": {"kind": kind, "value": document},
        "viewport": viewport
        or {
            "min_x_nm": 0,
            "min_y_nm": 0,
            "width_nm": 2_000_000,
            "height_nm": 1_000_000,
        },
        "limits": {
            "max_records": 1,
            "max_operations": 1,
            "max_points": "10",
            "max_text_bytes": "100",
            "max_image_encoded_bytes": "100",
            "max_block_depth": 1,
            "max_svg_elements": "10",
            "max_render_work": "10000",
            "max_svg_bytes": "10000",
            "max_result_bytes": "20000",
        },
    }


def _encode(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":")).encode()


def _svg_result(svg: str = "<svg>\u00e9</svg>\n") -> dict[str, object]:
    encoded = svg.encode()
    return {
        "type": "kicad_monkey.native.svg.result",
        "version": "a0",
        "engine_version": "0.1.0",
        "profile": "plotter-base-a0",
        "source_kind": "MOD",
        "document_id": "demo",
        "svg_utf8": svg,
        "svg_bytes": str(len(encoded)),
        "svg_sha256": hashlib.sha256(encoded).hexdigest(),
    }


def test_generated_svg_contracts_apply_semantic_validation() -> None:
    handshake = {
        "type": "kicad_monkey.native.handshake",
        "version": "a1",
        "engine_version": "0.1.0",
        "operations": ["design-facts", "render-svg"],
    }
    decoded_handshake = decode_native_handshake_a1(_encode(handshake))
    assert decoded_handshake.operations == ("design-facts", "render-svg")

    request = _svg_request(_document())
    decode_native_svg_render_request_a0(_encode(request))
    result = _svg_result()
    decode_native_svg_render_result_a0(_encode(result))

    mutations = []
    reversed_handshake = deepcopy(handshake)
    reversed_handshake["operations"] = ["render-svg", "design-facts"]
    mutations.append((decode_native_handshake_a1, reversed_handshake))
    duplicate_handshake = deepcopy(handshake)
    duplicate_handshake["operations"] = ["design-facts", "design-facts"]
    mutations.append((decode_native_handshake_a1, duplicate_handshake))
    wrong_count = deepcopy(request)
    wrong_count["document"]["value"]["total_operations"] = 0
    mutations.append((decode_native_svg_render_request_a0, wrong_count))
    wrong_source = deepcopy(request)
    wrong_source["document"]["value"]["source_kind"] = "PCB"
    mutations.append((decode_native_svg_render_request_a0, wrong_source))
    empty_request_identity = deepcopy(request)
    empty_request_identity["document"]["value"]["document_id"] = ""
    mutations.append((decode_native_svg_render_request_a0, empty_request_identity))
    over_uint64 = deepcopy(request)
    over_uint64["limits"]["max_points"] = "18446744073709551616"
    mutations.append((decode_native_svg_render_request_a0, over_uint64))
    empty_identity = deepcopy(result)
    empty_identity["document_id"] = ""
    mutations.append((decode_native_svg_render_result_a0, empty_identity))
    empty_svg = deepcopy(result)
    empty_svg["svg_utf8"] = ""
    mutations.append((decode_native_svg_render_result_a0, empty_svg))
    invalid_source_kind = deepcopy(result)
    invalid_source_kind["source_kind"] = "MODX"
    mutations.append((decode_native_svg_render_result_a0, invalid_source_kind))
    wrong_bytes = deepcopy(result)
    wrong_bytes["svg_bytes"] = "1"
    mutations.append((decode_native_svg_render_result_a0, wrong_bytes))
    wrong_hash = deepcopy(result)
    wrong_hash["svg_sha256"] = "0" * 64
    mutations.append((decode_native_svg_render_result_a0, wrong_hash))

    for decoder, mutation in mutations:
        with pytest.raises(msgspec.ValidationError):
            decoder(_encode(mutation))


def test_generated_schematic_svg_contract_requires_canvas_viewport() -> None:
    vectors = json.loads(
        (REPO_ROOT / "tests/parity/schematic_plotter_a0_vectors.json").read_text(
            encoding="utf-8"
        )
    )
    document = vectors["vectors"][0]["expected"]
    canvas = document["canvas"]
    viewport = {
        "min_x_nm": 0,
        "min_y_nm": 0,
        "width_nm": canvas["width_nm"],
        "height_nm": canvas["height_nm"],
    }
    request = _svg_request(document, kind="schematic", viewport=viewport)
    decode_native_svg_render_request_a0(_encode(request))

    request["viewport"]["min_x_nm"] = 1
    with pytest.raises(msgspec.ValidationError, match="viewport_mismatch"):
        decode_native_svg_render_request_a0(_encode(request))


def test_svg_client_validates_a1_handshake_and_hashed_result(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()
    svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"/>\n'
    digest = hashlib.sha256(svg.encode()).hexdigest()

    def fake_run(_executable, command, _request, **_kwargs):
        if command == "handshake-a1":
            return json.dumps(
                {
                    "type": "kicad_monkey.native.handshake",
                    "version": "a1",
                    "engine_version": "0.1.0",
                    "operations": ["design-facts", "render-svg"],
                }
            ).encode()
        return json.dumps(
            {
                "type": "kicad_monkey.native.svg.result",
                "version": "a0",
                "engine_version": "0.1.0",
                "profile": "plotter-base-a0",
                "source_kind": "MOD",
                "document_id": "demo",
                "svg_utf8": svg,
                "svg_bytes": str(len(svg.encode())),
                "svg_sha256": digest,
            }
        ).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    handshake = kicad_native_handshake_a1(executable=executable)
    result = native_render_svg(
        _document(),
        document_kind="footprint",
        viewport={
            "min_x_nm": 0,
            "min_y_nm": 0,
            "width_nm": 2_000_000,
            "height_nm": 1_000_000,
        },
        executable=executable,
    )
    assert handshake["operations"] == ["design-facts", "render-svg"]
    assert result.svg_sha256 == digest
    assert result.svg_bytes == len(svg.encode())


def test_svg_client_rejects_hash_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()

    def fake_run(_executable, command, _request, **_kwargs):
        if command == "handshake-a1":
            return b'{"type":"kicad_monkey.native.handshake","version":"a1","engine_version":"0.1.0","operations":["design-facts","render-svg"]}'
        return b'{"type":"kicad_monkey.native.svg.result","version":"a0","engine_version":"0.1.0","profile":"plotter-base-a0","source_kind":"MOD","document_id":"demo","svg_utf8":"<svg/>","svg_bytes":"6","svg_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}'

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    with pytest.raises(KiCadNativeError, match="hash"):
        native_render_svg(
            _document(),
            document_kind="footprint",
            viewport={
                "min_x_nm": 0,
                "min_y_nm": 0,
                "width_nm": 2_000_000,
                "height_nm": 1_000_000,
            },
            executable=executable,
        )


def test_svg_client_rejects_document_kind_mismatch_before_process(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()
    calls: list[str] = []

    def fake_run(_executable, command, _request, **_kwargs):
        calls.append(command)
        return b'{"type":"kicad_monkey.native.handshake","version":"a1","engine_version":"0.1.0","operations":["design-facts","render-svg"]}'

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    with pytest.raises(KiCadNativeError, match="source_kind"):
        native_render_svg(
            _document(),
            document_kind="board",
            viewport={
                "min_x_nm": 0,
                "min_y_nm": 0,
                "width_nm": 2_000_000,
                "height_nm": 1_000_000,
            },
            executable=executable,
        )
    assert calls == ["handshake-a1"]


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        ({"source_kind": "PCB"}, "identity"),
        ({"document_id": "other"}, "identity"),
        (
            {
                "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"><script/></svg>'
            },
            "unsupported element",
        ),
        (
            {
                "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000" onload="alert(1)"/>'
            },
            "unsupported attribute",
        ),
        (
            {
                "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"><image href="https://example.test/image.png"/></svg>'
            },
            "unsafe href",
        ),
        (
            {
                "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"><rect x="0" y="0" width="1" height="1" fill="url(https://example.test/paint)"/></svg>'
            },
            "unsafe paint",
        ),
        (
            {"svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"/>'},
            "viewBox",
        ),
        (
            {"svg_utf8": '<svg xmlns="urn:not-svg" viewBox="0 0 2000000 1000000"/>'},
            "root",
        ),
        (
            {
                "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"><g id="same"/><g id="same"/></svg>'
            },
            "duplicate id",
        ),
        (
            {
                "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"><g id=""/></svg>'
            },
            "empty id",
        ),
        (
            {
                "svg_utf8": '<!DOCTYPE svg [<!ENTITY x "boom">]><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000">&x;</svg>'
            },
            "forbidden XML",
        ),
    ],
)
def test_svg_client_rejects_hostile_correct_hash_results(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: dict[str, str],
    message: str,
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()
    payload = {
        "type": "kicad_monkey.native.svg.result",
        "version": "a0",
        "engine_version": "0.1.0",
        "profile": "plotter-base-a0",
        "source_kind": "MOD",
        "document_id": "demo",
        "svg_utf8": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"/>',
        "svg_bytes": "0",
        "svg_sha256": "0" * 64,
    }
    payload.update(mutation)
    svg = payload["svg_utf8"].encode()
    payload["svg_bytes"] = str(len(svg))
    payload["svg_sha256"] = hashlib.sha256(svg).hexdigest()

    def fake_run(_executable, command, _request, **_kwargs):
        if command == "handshake-a1":
            return b'{"type":"kicad_monkey.native.handshake","version":"a1","engine_version":"0.1.0","operations":["design-facts","render-svg"]}'
        return json.dumps(payload, separators=(",", ":")).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    with pytest.raises(KiCadNativeError, match=message):
        native_render_svg(
            _document(),
            document_kind="footprint",
            viewport={
                "min_x_nm": 0,
                "min_y_nm": 0,
                "width_nm": 2_000_000,
                "height_nm": 1_000_000,
            },
            executable=executable,
        )


def test_svg_client_rejects_result_over_requested_svg_ceiling(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()
    svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000000 1000000"/>'

    def fake_run(_executable, command, _request, **_kwargs):
        if command == "handshake-a1":
            return b'{"type":"kicad_monkey.native.handshake","version":"a1","engine_version":"0.1.0","operations":["design-facts","render-svg"]}'
        return json.dumps(
            {
                "type": "kicad_monkey.native.svg.result",
                "version": "a0",
                "engine_version": "0.1.0",
                "profile": "plotter-base-a0",
                "source_kind": "MOD",
                "document_id": "demo",
                "svg_utf8": svg,
                "svg_bytes": str(len(svg.encode())),
                "svg_sha256": hashlib.sha256(svg.encode()).hexdigest(),
            },
            separators=(",", ":"),
        ).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    limits = {
        "max_records": 10,
        "max_operations": 10,
        "max_points": "100",
        "max_text_bytes": "100",
        "max_image_encoded_bytes": "100",
        "max_block_depth": 10,
        "max_svg_elements": "100",
        "max_render_work": "10000",
        "max_svg_bytes": "1",
        "max_result_bytes": "10000",
    }
    with pytest.raises(KiCadNativeError, match="requested SVG byte ceiling"):
        native_render_svg(
            _document(),
            document_kind="footprint",
            viewport={
                "min_x_nm": 0,
                "min_y_nm": 0,
                "width_nm": 2_000_000,
                "height_nm": 1_000_000,
            },
            limits=limits,
            executable=executable,
        )

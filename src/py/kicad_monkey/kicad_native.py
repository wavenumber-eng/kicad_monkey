"""Fail-closed client for the packaged KiCad Monkey native process.

The process boundary is deliberately explicit: callers either provide an
executable path or use the package-owned binary.  This module never searches
``PATH`` and never falls back to a Python implementation.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import subprocess
import tempfile
import threading
import time
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Mapping, Sequence, cast

import msgspec

from .contracts.generated import (
    decode_compiled_schematic_graph_a0,
    decode_native_design_facts_request_a0,
    decode_native_design_facts_result_a0,
    decode_native_error_a0,
    decode_native_handshake_a0,
    decode_native_handshake_a1,
    decode_native_svg_render_request_a0,
    decode_native_svg_render_result_a0,
)
from .kicad_base import get_value
from .kicad_compiled_schematic_graph import validate_compiled_schematic_graph
from .kicad_sexpr import SexprError, parse_sexp

if TYPE_CHECKING:
    from .kicad_design import KiCadDesign

_PROTOCOL_VERSION = "a0"
_REQUEST_TYPE = "kicad_monkey.native.design_facts.request"
_ERROR_TYPE = "kicad_monkey.native.error"
_NATIVE_ENV = "KICAD_MONKEY_NATIVE"
_MAX_HANDSHAKE_BYTES = 64 * 1024
_MAX_REQUEST_BYTES = 1024 * 1024
_MAX_REQUEST_STRING_BYTES = 64 * 1024
_MAX_REQUEST_NODES = 64 * 1024
_DEFAULT_OUTPUT_BYTES = 64 * 1024 * 1024
_DEFAULT_TIMEOUT_SECONDS = 120.0
_MAX_SVG_REQUEST_BYTES = 256 * 1024 * 1024
_MAX_SVG_REQUEST_NODES = 8 * 1024 * 1024
_SVG_REQUEST_TYPE = "kicad_monkey.native.svg.request"
_SVG_PROFILE = "plotter-base-a0"
_SVG_NAMESPACE = "http://www.w3.org/2000/svg"
_SVG_TAGS = frozenset(
    {
        "svg",
        "rect",
        "g",
        "line",
        "path",
        "circle",
        "polygon",
        "polyline",
        "text",
        "image",
        "ellipse",
    }
)
_SVG_ATTRIBUTES = frozenset(
    {
        "width",
        "height",
        "viewBox",
        "x",
        "y",
        "fill",
        "transform",
        "id",
        "data-ref",
        "data-object-id",
        "data-uuid",
        "x1",
        "y1",
        "x2",
        "y2",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
        "stroke-dasharray",
        "fill-opacity",
        "stroke-opacity",
        "cx",
        "cy",
        "r",
        "rx",
        "ry",
        "points",
        "d",
        "font-size",
        "font-family",
        "font-weight",
        "font-style",
        "text-anchor",
        "dominant-baseline",
        "preserveAspectRatio",
        "href",
        "fill-rule",
    }
)
_SVG_IMAGE_HREF_PREFIXES = (
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/bmp;base64,",
)
_BASE64_CHARACTERS = frozenset(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
)


class KiCadNativeError(RuntimeError):
    """A native transport, protocol, resource, or operation failure."""


@dataclass(frozen=True, slots=True)
class KiCadNativeDesignFacts:
    """Validated facts returned by the native ``design-facts`` operation."""

    engine_version: str
    compiled_schematic_graph: dict[str, object]
    kicad_netlist: str
    design_fingerprint: str | None = None


@dataclass(frozen=True, slots=True)
class KiCadNativeSvg:
    """Validated deterministic SVG returned by the native base serializer."""

    engine_version: str
    source_kind: str
    document_id: str
    svg_utf8: str
    svg_bytes: int
    svg_sha256: str


def resolve_kicad_native_executable(
    executable: Path | str | None = None,
) -> Path:
    """Resolve an explicit or package-owned native executable.

    ``KICAD_MONKEY_NATIVE`` is an explicit development/test override.  No
    ambient ``PATH`` search is performed.
    """

    raw = executable
    if raw is None:
        raw = os.environ.get(_NATIVE_ENV)
    if raw is None:
        filename = "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
        raw = Path(__file__).resolve().parent / "_native" / filename
    path = Path(raw).expanduser().resolve()
    if not path.is_file():
        raise KiCadNativeError(f"KiCad Monkey native executable is unavailable: {path}")
    return path


def kicad_native_handshake(
    *,
    executable: Path | str | None = None,
    timeout: float = 10.0,
) -> dict[str, object]:
    """Return a validated native protocol handshake."""

    output = _run_native_command(
        resolve_kicad_native_executable(executable),
        "handshake",
        b"",
        maximum_output_bytes=_MAX_HANDSHAKE_BYTES,
        timeout=timeout,
    )
    try:
        decoded = decode_native_handshake_a0(output)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native handshake violates its contract: {error}") from error
    payload = cast(dict[str, object], msgspec.to_builtins(decoded))
    engine_version = payload.get("engine_version")
    operations = payload.get("operations")
    if not isinstance(engine_version, str) or not engine_version:
        raise KiCadNativeError("native handshake engine_version is invalid")
    if operations != ["design-facts"]:
        raise KiCadNativeError("native handshake operations are unsupported")
    return payload


def kicad_native_handshake_a1(
    *,
    executable: Path | str | None = None,
    timeout: float = 10.0,
) -> dict[str, object]:
    """Return the expanded handshake while preserving the closed a0 handshake."""

    output = _run_native_command(
        resolve_kicad_native_executable(executable),
        "handshake-a1",
        b"",
        maximum_output_bytes=_MAX_HANDSHAKE_BYTES,
        timeout=timeout,
    )
    try:
        decoded = decode_native_handshake_a1(output)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native a1 handshake violates its contract: {error}") from error
    payload = cast(dict[str, object], msgspec.to_builtins(decoded))
    if not isinstance(payload.get("engine_version"), str) or not payload["engine_version"]:
        raise KiCadNativeError("native a1 handshake engine_version is invalid")
    operations = payload.get("operations")
    if not isinstance(operations, (list, tuple)) or tuple(operations) != (
        "design-facts",
        "render-svg",
    ):
        raise KiCadNativeError("native a1 handshake operations are unsupported")
    payload["operations"] = list(operations)
    return payload


def native_render_svg(
    document: Mapping[str, object],
    *,
    document_kind: str,
    viewport: Mapping[str, object],
    limits: Mapping[str, object] | None = None,
    executable: Path | str | None = None,
    timeout: float = _DEFAULT_TIMEOUT_SECONDS,
) -> KiCadNativeSvg:
    """Render one frozen Plotter-IR document through the native base profile."""

    if document_kind not in {"footprint", "symbol", "board", "schematic"}:
        raise KiCadNativeError("native SVG document_kind is unsupported")
    native = resolve_kicad_native_executable(executable)
    handshake = kicad_native_handshake_a1(executable=native, timeout=min(timeout, 10.0))
    selected_limits = dict(limits or _default_svg_limits())
    request = {
        "type": _SVG_REQUEST_TYPE,
        "version": _PROTOCOL_VERSION,
        "profile": _SVG_PROFILE,
        "document": {"kind": document_kind, "value": dict(document)},
        "viewport": dict(viewport),
        "limits": selected_limits,
    }
    request_bytes = _encode_svg_request_bounded(request)
    try:
        decode_native_svg_render_request_a0(request_bytes)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native SVG request violates its contract: {error}") from error
    _validate_svg_request_semantics(document, document_kind, viewport, selected_limits)
    maximum_result_bytes = _canonical_svg_limit(
        selected_limits.get("max_result_bytes"), "max_result_bytes"
    )
    output = _run_native_command(
        native,
        "render-svg",
        request_bytes,
        maximum_output_bytes=maximum_result_bytes,
        timeout=timeout,
    )
    try:
        decoded = decode_native_svg_render_result_a0(output)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native SVG result violates its contract: {error}") from error
    payload = cast(dict[str, object], msgspec.to_builtins(decoded))
    if payload.get("engine_version") != handshake["engine_version"]:
        raise KiCadNativeError("native engine version changed between handshake and SVG render")
    svg = payload.get("svg_utf8")
    encoded_bytes = payload.get("svg_bytes")
    digest = payload.get("svg_sha256")
    if not isinstance(svg, str) or not isinstance(encoded_bytes, str) or not isinstance(digest, str):
        raise KiCadNativeError("native SVG result fields are malformed")
    size = _canonical_svg_limit(encoded_bytes, "svg_bytes")
    actual = svg.encode("utf-8")
    if size != len(actual):
        raise KiCadNativeError("native SVG byte count does not match its payload")
    if hashlib.sha256(actual).hexdigest() != digest:
        raise KiCadNativeError("native SVG hash does not match its payload")
    source_kind = payload.get("source_kind")
    document_id = payload.get("document_id")
    if not isinstance(source_kind, str) or not isinstance(document_id, str):
        raise KiCadNativeError("native SVG result identity is malformed")
    expected_source_kind = {
        "footprint": "MOD",
        "symbol": "SYM",
        "board": "PCB",
        "schematic": "SCH",
    }[document_kind]
    if source_kind != expected_source_kind or document_id != document.get(
        "document_id"
    ):
        raise KiCadNativeError("native SVG result identity does not match its request")
    maximum_svg_bytes = _canonical_svg_limit(
        selected_limits.get("max_svg_bytes"), "max_svg_bytes"
    )
    if size > maximum_svg_bytes:
        raise KiCadNativeError(
            "native SVG payload exceeds the requested SVG byte ceiling"
        )
    _validate_native_svg(svg, viewport)
    return KiCadNativeSvg(
        engine_version=cast(str, payload["engine_version"]),
        source_kind=source_kind,
        document_id=document_id,
        svg_utf8=svg,
        svg_bytes=size,
        svg_sha256=digest,
    )


def native_design_facts(
    *,
    bundle_root: Path | str,
    manifest: Mapping[str, object],
    file_slots: Sequence[Mapping[str, object]],
    limits: Mapping[str, object],
    source_path: str,
    date: str = "",
    tool: str = "kicad-monkey-native",
    executable: Path | str | None = None,
    timeout: float = _DEFAULT_TIMEOUT_SECONDS,
) -> KiCadNativeDesignFacts:
    """Run one strict, bounded native design-facts request."""

    native = resolve_kicad_native_executable(executable)
    handshake = kicad_native_handshake(executable=native, timeout=min(timeout, 10.0))
    request = {
        "type": _REQUEST_TYPE,
        "version": _PROTOCOL_VERSION,
        "bundle_root": str(Path(bundle_root).resolve()),
        "manifest": dict(manifest),
        "file_slots": [dict(slot) for slot in file_slots],
        "limits": dict(limits),
        "netlist": {"source_path": source_path, "date": date, "tool": tool},
    }
    maximum_output_bytes = _canonical_limit(limits.get("max_output_bytes"))
    request_bytes = _encode_request_bounded(request)
    try:
        decode_native_design_facts_request_a0(request_bytes)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native design-facts request violates its contract: {error}") from error
    output = _run_native_command(
        native,
        "design-facts",
        request_bytes,
        maximum_output_bytes=maximum_output_bytes,
        timeout=timeout,
    )
    try:
        decoded_result = decode_native_design_facts_result_a0(output)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native design-facts result violates its contract: {error}") from error
    payload = cast(dict[str, object], msgspec.to_builtins(decoded_result))
    if payload.get("engine_version") != handshake["engine_version"]:
        raise KiCadNativeError("native engine version changed between handshake and operation")
    if payload.get("kicad_netlist_version") != "E":
        raise KiCadNativeError("native netlist version is unsupported")
    netlist = payload.get("kicad_netlist")
    if not isinstance(netlist, str):
        raise KiCadNativeError("native netlist payload is not text")
    _validate_version_e_netlist(netlist)
    graph_value = cast(dict[str, object], payload["compiled_schematic_graph"])
    graph_bytes = json.dumps(graph_value, separators=(",", ":")).encode("utf-8")
    try:
        decoded = decode_compiled_schematic_graph_a0(graph_bytes)
    except msgspec.ValidationError as error:
        raise KiCadNativeError(f"native compiled graph violates its contract: {error}") from error
    graph = cast(dict[str, object], msgspec.to_builtins(decoded))
    try:
        validate_compiled_schematic_graph(graph)
    except (TypeError, ValueError) as error:
        raise KiCadNativeError(f"native compiled graph is semantically invalid: {error}") from error
    return KiCadNativeDesignFacts(
        engine_version=cast(str, payload["engine_version"]),
        compiled_schematic_graph=graph,
        kicad_netlist=netlist,
    )


def native_design_facts_for_design(
    design: KiCadDesign,
    *,
    executable: Path | str | None = None,
    timeout: float = _DEFAULT_TIMEOUT_SECONDS,
) -> KiCadNativeDesignFacts:
    """Build an explicit source bundle for an already-loaded design."""

    root, sources = _design_source_payloads(design)
    manifest_sources: list[dict[str, object]] = []
    slots: list[dict[str, object]] = []
    sizes: list[int] = []
    for slot, (path, kind, source_bytes) in enumerate(sources):
        relative = _portable_relative(path, root)
        size = len(source_bytes)
        sizes.append(size)
        manifest_sources.append(
            {"path": relative, "kind": kind, "slot": slot, "source_bytes": str(size)}
        )
        slots.append({"slot": slot, "path": relative})
    top_path = _source_path(design.top_schematic)
    assert top_path is not None
    root_relative = _portable_relative(top_path.resolve(), root)
    manifest: dict[str, object] = {
        "schema": "kicad_monkey.source_bundle_manifest.a0",
        "type": "kicad_monkey.source_bundle_manifest",
        "version": "a0",
        "root_schematic_path": root_relative,
        "sources": manifest_sources,
    }
    if design.project_path is not None:
        manifest["project_path"] = _portable_relative(design.project_path.resolve(), root)
    total = sum(sizes)
    limits = {
        "max_sources": len(sources),
        "max_source_bytes": str(max(sizes, default=0)),
        "max_total_source_bytes": str(total),
        "max_path_bytes": 4096,
        "max_output_bytes": str(_DEFAULT_OUTPUT_BYTES),
    }
    fingerprint = _source_fingerprint(manifest, sources)
    with tempfile.TemporaryDirectory(prefix="kicad-monkey-native-") as staging_text:
        staging = Path(staging_text)
        for path, _kind, source_bytes in sources:
            destination = staging / _portable_relative(path, root)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source_bytes)
        result = native_design_facts(
            bundle_root=staging,
            manifest=manifest,
            file_slots=slots,
            limits=limits,
            source_path=root_relative,
            executable=executable,
            timeout=timeout,
        )
    return KiCadNativeDesignFacts(
        engine_version=result.engine_version,
        compiled_schematic_graph=result.compiled_schematic_graph,
        kicad_netlist=result.kicad_netlist,
        design_fingerprint=fingerprint,
    )


def _design_source_payloads(design: KiCadDesign) -> tuple[Path, list[tuple[Path, str, bytes]]]:
    top = design.top_schematic
    top_path = _source_path(top)
    if top is None or top_path is None:
        raise KiCadNativeError("native design facts require a file-backed top schematic")
    root = (design.project_path or top_path).resolve().parent
    sources: list[tuple[Path, str, bytes]] = []
    if design.project_path is not None:
        if design.project is None:
            raise KiCadNativeError("the file-backed project is not loaded")
        sources.append(
            (
                design.project_path.resolve(),
                "project",
                design.project.to_text().encode("utf-8"),
            )
        )
    seen: set[Path] = set()
    for occurrence in design.schematic_instances():
        schematic = occurrence.schematic
        path = _source_path(schematic)
        if path is None:
            raise KiCadNativeError("native design facts require file-backed child schematics")
        resolved = path.resolve()
        if resolved not in seen:
            seen.add(resolved)
            sources.append((resolved, "schematic", schematic.to_text().encode("utf-8")))
    if top_path.resolve() not in seen:
        sources.append((top_path.resolve(), "schematic", top.to_text().encode("utf-8")))
    return root, sources


def _source_fingerprint(
    manifest: Mapping[str, object], sources: Sequence[tuple[Path, str, bytes]]
) -> str:
    digest = hashlib.sha256()
    digest.update(json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode())
    for _path, _kind, source_bytes in sources:
        digest.update(len(source_bytes).to_bytes(8, "big"))
        digest.update(source_bytes)
    return digest.hexdigest()


def _design_fingerprint(design: KiCadDesign) -> str:
    root, sources = _design_source_payloads(design)
    manifest_sources = [
        {
            "path": _portable_relative(path, root),
            "kind": kind,
            "slot": slot,
            "source_bytes": str(len(source_bytes)),
        }
        for slot, (path, kind, source_bytes) in enumerate(sources)
    ]
    top_path = _source_path(design.top_schematic)
    assert top_path is not None
    manifest: dict[str, object] = {
        "schema": "kicad_monkey.source_bundle_manifest.a0",
        "type": "kicad_monkey.source_bundle_manifest",
        "version": "a0",
        "root_schematic_path": _portable_relative(top_path.resolve(), root),
        "sources": manifest_sources,
    }
    if design.project_path is not None:
        manifest["project_path"] = _portable_relative(design.project_path.resolve(), root)
    return _source_fingerprint(manifest, sources)


def _source_path(value: object) -> Path | None:
    raw = getattr(value, "source_path", None)
    return Path(raw) if raw is not None else None


def _portable_relative(path: Path, root: Path) -> str:
    try:
        relative = path.relative_to(root)
    except ValueError as error:
        raise KiCadNativeError(f"native source lies outside the bundle root: {path}") from error
    if not relative.parts or any(part in {"", ".", ".."} for part in relative.parts):
        raise KiCadNativeError(f"native source path is not portable: {relative}")
    return relative.as_posix()


def _canonical_limit(value: object) -> int:
    if not isinstance(value, str) or not value or not value.isascii() or not value.isdecimal():
        raise KiCadNativeError("max_output_bytes must be canonical unsigned decimal text")
    if len(value) > 1 and value.startswith("0"):
        raise KiCadNativeError("max_output_bytes must not contain leading zeroes")
    maximum = int(value)
    if maximum > 256 * 1024 * 1024:
        raise KiCadNativeError("max_output_bytes exceeds the native application ceiling")
    return maximum


def _encode_request_bounded(value: Mapping[str, object]) -> bytes:
    pending: list[object] = [value]
    nodes = 0
    while pending:
        item = pending.pop()
        nodes += 1
        if nodes > _MAX_REQUEST_NODES:
            raise KiCadNativeError("native request exceeds its structural node ceiling")
        if isinstance(item, str):
            if len(item.encode("utf-8")) > _MAX_REQUEST_STRING_BYTES:
                raise KiCadNativeError("native request string exceeds its byte ceiling")
        elif isinstance(item, Mapping):
            pending.extend(item.keys())
            pending.extend(item.values())
        elif isinstance(item, Sequence) and not isinstance(item, (bytes, bytearray)):
            pending.extend(item)
    output = bytearray()
    encoder = json.JSONEncoder(separators=(",", ":"), ensure_ascii=False)
    for text in encoder.iterencode(value):
        chunk = text.encode("utf-8")
        if len(output) + len(chunk) > _MAX_REQUEST_BYTES:
            raise KiCadNativeError("native design-facts request exceeds the 1 MiB ceiling")
        output.extend(chunk)
    return bytes(output)


def _default_svg_limits() -> dict[str, object]:
    return {
        "max_records": 1_000_000,
        "max_operations": 4_000_000,
        "max_points": "16000000",
        "max_text_bytes": str(256 * 1024 * 1024),
        "max_image_encoded_bytes": str(256 * 1024 * 1024),
        "max_block_depth": 4096,
        "max_svg_elements": "8000000",
        "max_render_work": "64000000",
        "max_svg_bytes": str(512 * 1024 * 1024),
        "max_result_bytes": str(768 * 1024 * 1024),
    }


def _canonical_svg_limit(value: object, name: str) -> int:
    if not isinstance(value, str) or not value or not value.isascii() or not value.isdecimal():
        raise KiCadNativeError(f"{name} must be canonical unsigned decimal text")
    if len(value) > 1 and value.startswith("0"):
        raise KiCadNativeError(f"{name} must not contain leading zeroes")
    parsed = int(value)
    if parsed > 768 * 1024 * 1024:
        raise KiCadNativeError(f"{name} exceeds the native SVG application ceiling")
    return parsed


def _validate_svg_request_semantics(
    document: Mapping[str, object],
    document_kind: str,
    viewport: Mapping[str, object],
    limits: Mapping[str, object],
) -> None:
    expected_source = {
        "footprint": "MOD",
        "symbol": "SYM",
        "board": "PCB",
        "schematic": "SCH",
    }[document_kind]
    if document.get("source_kind") != expected_source:
        raise KiCadNativeError("native SVG document kind and source_kind do not match")
    for name in (
        "max_points",
        "max_text_bytes",
        "max_image_encoded_bytes",
        "max_svg_elements",
        "max_render_work",
        "max_svg_bytes",
        "max_result_bytes",
    ):
        _canonical_svg_limit(limits.get(name), name)
    if document_kind == "schematic":
        canvas = document.get("canvas")
        if not isinstance(canvas, Mapping):
            raise KiCadNativeError("native schematic SVG document canvas is missing")
        expected = {
            "min_x_nm": 0,
            "min_y_nm": 0,
            "width_nm": canvas.get("width_nm"),
            "height_nm": canvas.get("height_nm"),
        }
        if dict(viewport) != expected:
            raise KiCadNativeError("native schematic SVG viewport does not match its canvas")


def _encode_svg_request_bounded(value: Mapping[str, object]) -> bytes:
    pending: list[object] = [value]
    nodes = 0
    while pending:
        item = pending.pop()
        nodes += 1
        if nodes > _MAX_SVG_REQUEST_NODES:
            raise KiCadNativeError("native SVG request exceeds its structural node ceiling")
        if isinstance(item, Mapping):
            pending.extend(item.keys())
            pending.extend(item.values())
        elif isinstance(item, Sequence) and not isinstance(item, (str, bytes, bytearray)):
            pending.extend(item)
    output = bytearray()
    encoder = json.JSONEncoder(separators=(",", ":"), ensure_ascii=False)
    for text in encoder.iterencode(value):
        chunk = text.encode("utf-8")
        if len(output) + len(chunk) > _MAX_SVG_REQUEST_BYTES:
            raise KiCadNativeError("native SVG request exceeds the 256 MiB ceiling")
        output.extend(chunk)
    return bytes(output)


def _validate_native_svg(svg: str, viewport: Mapping[str, object]) -> None:
    if any(ord(character) < 0x20 and character not in "\t\n\r" for character in svg):
        raise KiCadNativeError("native SVG contains an XML control character")
    declarations = svg
    if declarations.startswith("<?xml"):
        end = declarations.find("?>")
        if end < 0:
            raise KiCadNativeError("native SVG XML declaration is unterminated")
        declarations = declarations[end + 2 :]
    if "<!" in declarations or "<?" in declarations:
        raise KiCadNativeError("native SVG contains a forbidden XML declaration")
    try:
        root = ET.fromstring(svg)
    except ET.ParseError as error:
        raise KiCadNativeError(f"native SVG is malformed XML: {error}") from error
    expected_root = f"{{{_SVG_NAMESPACE}}}svg"
    if root.tag != expected_root:
        raise KiCadNativeError("native SVG root is not an SVG-namespace svg element")
    expected_view_box = f"0 0 {viewport.get('width_nm')} {viewport.get('height_nm')}"
    if root.attrib.get("viewBox") != expected_view_box:
        raise KiCadNativeError(
            "native SVG viewBox does not match the requested viewport"
        )

    identifiers: set[str] = set()
    for element in root.iter():
        if not isinstance(element.tag, str) or not element.tag.startswith(
            f"{{{_SVG_NAMESPACE}}}"
        ):
            raise KiCadNativeError("native SVG contains a foreign-namespace element")
        tag = element.tag.rsplit("}", 1)[-1]
        if tag not in _SVG_TAGS:
            raise KiCadNativeError(f"native SVG contains unsupported element {tag}")
        for name, value in element.attrib.items():
            if name not in _SVG_ATTRIBUTES and not name.startswith("data-"):
                raise KiCadNativeError(
                    f"native SVG contains unsupported attribute {name}"
                )
            if name.startswith("on") or name in {"style", "class"}:
                raise KiCadNativeError(f"native SVG contains unsafe attribute {name}")
            if name == "id":
                if not value:
                    raise KiCadNativeError("native SVG contains an empty id")
                if value in identifiers:
                    raise KiCadNativeError("native SVG contains a duplicate id")
                identifiers.add(value)
            if name == "href":
                if tag != "image" or not _safe_image_href(value):
                    raise KiCadNativeError("native SVG contains an unsafe href")
            if name in {"fill", "stroke"} and not _safe_svg_paint(value):
                raise KiCadNativeError("native SVG contains an unsafe paint value")


def _safe_image_href(value: str) -> bool:
    prefix = next(
        (item for item in _SVG_IMAGE_HREF_PREFIXES if value.startswith(item)), None
    )
    if prefix is None:
        return False
    payload = value[len(prefix) :]
    if not payload or len(payload) % 4 != 0:
        return False
    unpadded = payload.rstrip("=")
    padding = len(payload) - len(unpadded)
    return padding <= 2 and all(
        character in _BASE64_CHARACTERS for character in unpadded
    )


def _safe_svg_paint(value: str) -> bool:
    if value == "none":
        return True
    return (
        len(value) == 7
        and value.startswith("#")
        and all(character in "0123456789abcdefABCDEF" for character in value[1:])
    )


def _validate_version_e_netlist(text: str) -> None:
    try:
        root = parse_sexp(text)
    except SexprError as error:
        raise KiCadNativeError(f"native netlist is malformed: {error}") from error
    if not isinstance(root, list) or not root or root[0] != "export":
        raise KiCadNativeError("native netlist is not an export document")
    if get_value(root, "version") != "E":
        raise KiCadNativeError("native netlist document is not version E")


def _run_native_command(
    executable: Path,
    command: str,
    request: bytes,
    *,
    maximum_output_bytes: int,
    timeout: float,
) -> bytes:
    if not math.isfinite(timeout) or timeout <= 0:
        raise KiCadNativeError("native timeout must be finite and positive")
    deadline = time.monotonic() + timeout
    try:
        process = subprocess.Popen(
            [str(executable), command],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise KiCadNativeError(f"native {command} execution failed: {error}") from error
    assert process.stdin is not None and process.stdout is not None and process.stderr is not None
    output = bytearray()
    diagnostic = bytearray()
    exceeded: list[str] = []
    stream_errors: list[tuple[str, OSError]] = []
    input_errors: list[OSError] = []

    def kill_process() -> None:
        if process.poll() is None:
            try:
                process.kill()
            except OSError:
                pass

    def drain(stream, destination: bytearray, maximum: int, name: str) -> None:
        try:
            while chunk := stream.read(64 * 1024):
                remaining = maximum - len(destination)
                if len(chunk) > remaining:
                    destination.extend(chunk[: max(remaining, 0)])
                    exceeded.append(name)
                    kill_process()
                    return
                destination.extend(chunk)
        except OSError as error:
            stream_errors.append((name, error))

    def write_request() -> None:
        try:
            process.stdin.write(request)
        except (BrokenPipeError, OSError) as error:
            input_errors.append(error)
        finally:
            try:
                process.stdin.close()
            except OSError:
                pass

    input_thread = threading.Thread(target=write_request, daemon=True)
    stdout_thread = threading.Thread(
        target=drain,
        args=(process.stdout, output, maximum_output_bytes, "stdout"),
        daemon=True,
    )
    stderr_thread = threading.Thread(
        target=drain,
        args=(process.stderr, diagnostic, _MAX_HANDSHAKE_BYTES, "stderr"),
        daemon=True,
    )
    input_thread.start()
    stdout_thread.start()
    stderr_thread.start()
    timed_out = False
    try:
        returncode = process.wait(timeout=max(deadline - time.monotonic(), 0.0))
    except subprocess.TimeoutExpired:
        timed_out = True
        kill_process()
        returncode = process.wait()
    finally:
        for thread in (input_thread, stdout_thread, stderr_thread):
            thread.join(timeout=max(deadline - time.monotonic(), 0.0))
    if timed_out or any(
        thread.is_alive() for thread in (input_thread, stdout_thread, stderr_thread)
    ):
        kill_process()
        raise KiCadNativeError(f"native {command} timed out")
    if exceeded:
        raise KiCadNativeError(f"native {command} {exceeded[0]} exceeds its byte ceiling")
    if stream_errors:
        name, error = stream_errors[0]
        raise KiCadNativeError(f"native {command} {name} capture failed: {error}") from error
    if returncode != 0:
        message = f"native {command} failed with exit code {returncode}"
        if diagnostic:
            try:
                decoded_error = decode_native_error_a0(bytes(diagnostic))
                error_payload = cast(dict[str, object], msgspec.to_builtins(decoded_error))
            except msgspec.ValidationError:
                pass
            else:
                if error_payload.get("type") == _ERROR_TYPE:
                    kind = error_payload.get("kind")
                    detail = error_payload.get("message")
                    if isinstance(kind, str) and isinstance(detail, str):
                        message = f"native {command} failed ({kind}): {detail}"
        raise KiCadNativeError(message)
    if input_errors:
        error = input_errors[0]
        raise KiCadNativeError(f"native {command} input failed: {error}") from error
    if diagnostic:
        raise KiCadNativeError(f"native {command} wrote unexpected stderr")
    return bytes(output)


__all__ = [
    "KiCadNativeDesignFacts",
    "KiCadNativeError",
    "KiCadNativeSvg",
    "kicad_native_handshake",
    "kicad_native_handshake_a1",
    "native_design_facts",
    "native_design_facts_for_design",
    "native_render_svg",
    "resolve_kicad_native_executable",
]

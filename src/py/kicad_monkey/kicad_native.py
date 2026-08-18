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


class KiCadNativeError(RuntimeError):
    """A native transport, protocol, resource, or operation failure."""


@dataclass(frozen=True, slots=True)
class KiCadNativeDesignFacts:
    """Validated facts returned by the native ``design-facts`` operation."""

    engine_version: str
    compiled_schematic_graph: dict[str, object]
    kicad_netlist: str
    design_fingerprint: str | None = None


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
    "kicad_native_handshake",
    "native_design_facts",
    "native_design_facts_for_design",
    "resolve_kicad_native_executable",
]

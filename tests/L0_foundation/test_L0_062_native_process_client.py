"""Focused tests for the fail-closed native process client."""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import pytest

from kicad_monkey import KiCadDesign
from kicad_monkey.kicad_compiled_schematic_graph import (
    build_compiled_schematic_graph,
)
from kicad_monkey.kicad_native import (
    KiCadNativeDesignFacts,
    KiCadNativeError,
    _run_native_command,
    kicad_native_handshake,
    native_design_facts,
    native_design_facts_for_design,
    resolve_kicad_native_executable,
)

_SCHEMATIC = """(kicad_sch
  (version 20260306)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid root)
  (paper "A4")
  (title_block (title "original"))
)
"""


def _write_design(root: Path) -> KiCadDesign:
    (root / "demo.kicad_pro").write_text("{}\n", encoding="utf-8")
    (root / "demo.kicad_sch").write_text(_SCHEMATIC, encoding="utf-8")
    return KiCadDesign.from_project_file(root / "demo.kicad_pro")


def _write_process_helper(tmp_path: Path, source: str) -> Path:
    helper = tmp_path / "native-helper.py"
    helper.write_text(source, encoding="utf-8")
    return helper


def test_resolver_never_searches_path(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("KICAD_MONKEY_NATIVE", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    (tmp_path / "kicad-monkey-native.exe").write_bytes(b"not selected")

    with pytest.raises(KiCadNativeError, match="unavailable"):
        resolve_kicad_native_executable()


def test_handshake_rejects_unknown_protocol_state(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()

    def fake_run(*_args, **_kwargs):
        return json.dumps(
            {
                "type": "kicad_monkey.native.handshake",
                "version": "a1",
                "engine_version": "0.1.0",
                "operations": ["design-facts"],
            }
        ).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    with pytest.raises(KiCadNativeError, match="violates its contract"):
        kicad_native_handshake(executable=executable)


def test_design_facts_strict_decodes_graph_and_versions(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    design = _write_design(tmp_path)
    graph = build_compiled_schematic_graph(design).to_json()
    executable = tmp_path / "native.exe"
    executable.touch()
    calls: list[str] = []

    def fake_run(_executable, command, _request, **_kwargs):
        calls.append(command)
        if command == "handshake":
            return json.dumps(
                {
                    "type": "kicad_monkey.native.handshake",
                    "version": "a0",
                    "engine_version": "0.1.0",
                    "operations": ["design-facts"],
                }
            ).encode()
        return json.dumps(
            {
                "type": "kicad_monkey.native.design_facts.result",
                "version": "a0",
                "engine_version": "0.1.0",
                "compiled_schematic_graph": graph,
                "kicad_netlist_version": "E",
                "kicad_netlist": "(export (version \"E\"))",
            }
        ).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    result = native_design_facts(
        bundle_root=tmp_path,
        manifest={
            "schema": "kicad_monkey.source_bundle_manifest.a0",
            "type": "kicad_monkey.source_bundle_manifest",
            "version": "a0",
            "root_schematic_path": "demo.kicad_sch",
            "sources": [],
        },
        file_slots=[],
        limits={
            "max_sources": 0,
            "max_source_bytes": "0",
            "max_total_source_bytes": "0",
            "max_path_bytes": 4096,
            "max_output_bytes": "1048576",
        },
        source_path="demo.kicad_sch",
        executable=executable,
    )

    assert calls == ["handshake", "design-facts"]
    assert result.compiled_schematic_graph == graph
    assert result.kicad_netlist.startswith("(export")


def test_design_facts_rejects_malformed_netlist(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    design = _write_design(tmp_path)
    graph = build_compiled_schematic_graph(design).to_json()
    executable = tmp_path / "native.exe"
    executable.touch()

    def fake_run(_executable, command, _request, **_kwargs):
        if command == "handshake":
            return json.dumps(
                {
                    "type": "kicad_monkey.native.handshake",
                    "version": "a0",
                    "engine_version": "0.1.0",
                    "operations": ["design-facts"],
                }
            ).encode()
        return json.dumps(
            {
                "type": "kicad_monkey.native.design_facts.result",
                "version": "a0",
                "engine_version": "0.1.0",
                "compiled_schematic_graph": graph,
                "kicad_netlist_version": "E",
                "kicad_netlist": "not an s-expression",
            }
        ).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    with pytest.raises(KiCadNativeError, match="netlist is malformed"):
        native_design_facts(
            bundle_root=tmp_path,
            manifest={
                "schema": "kicad_monkey.source_bundle_manifest.a0",
                "type": "kicad_monkey.source_bundle_manifest",
                "version": "a0",
                "root_schematic_path": "demo.kicad_sch",
                "sources": [],
            },
            file_slots=[],
            limits={
                "max_sources": 0,
                "max_source_bytes": "0",
                "max_total_source_bytes": "0",
                "max_path_bytes": 4096,
                "max_output_bytes": "1048576",
            },
            source_path="demo.kicad_sch",
            executable=executable,
        )


def test_request_string_ceiling_fails_before_operation_execution(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    executable = tmp_path / "native.exe"
    executable.touch()
    calls: list[str] = []

    def fake_run(_executable, command, _request, **_kwargs):
        calls.append(command)
        assert command == "handshake"
        return json.dumps(
            {
                "type": "kicad_monkey.native.handshake",
                "version": "a0",
                "engine_version": "0.1.0",
                "operations": ["design-facts"],
            }
        ).encode()

    monkeypatch.setattr("kicad_monkey.kicad_native._run_native_command", fake_run)
    with pytest.raises(KiCadNativeError, match="request string"):
        native_design_facts(
            bundle_root=tmp_path,
            manifest={
                "schema": "kicad_monkey.source_bundle_manifest.a0",
                "type": "kicad_monkey.source_bundle_manifest",
                "version": "a0",
                "root_schematic_path": "x" * (64 * 1024 + 1),
                "sources": [],
            },
            file_slots=[],
            limits={
                "max_sources": 0,
                "max_source_bytes": "0",
                "max_total_source_bytes": "0",
                "max_path_bytes": 4096,
                "max_output_bytes": "1048576",
            },
            source_path="demo.kicad_sch",
            executable=executable,
        )
    assert calls == ["handshake"]


@pytest.mark.parametrize(
    ("stream", "expected"),
    [("stdout", "stdout exceeds"), ("stderr", "stderr exceeds")],
)
def test_process_capture_terminates_at_independent_byte_ceilings(
    tmp_path: Path, stream: str, expected: str
) -> None:
    helper = _write_process_helper(
        tmp_path,
        (
            "import sys\n"
            f"target = sys.{stream}.buffer\n"
            "target.write(b'x' * (128 * 1024))\n"
            "target.flush()\n"
        ),
    )
    maximum = 1024 if stream == "stdout" else 1024 * 1024
    with pytest.raises(KiCadNativeError, match=expected):
        _run_native_command(
            Path(sys.executable),
            str(helper),
            b"",
            maximum_output_bytes=maximum,
            timeout=2.0,
        )


def test_process_timeout_bounds_a_child_that_never_reads_stdin(tmp_path: Path) -> None:
    helper = _write_process_helper(tmp_path, "import time\ntime.sleep(5)\n")
    started = time.monotonic()
    with pytest.raises(KiCadNativeError, match="timed out"):
        _run_native_command(
            Path(sys.executable),
            str(helper),
            b"x" * (1024 * 1024),
            maximum_output_bytes=1024,
            timeout=0.2,
        )
    assert time.monotonic() - started < 1.0


@pytest.mark.parametrize("timeout", [float("nan"), float("inf"), 0.0, -1.0])
def test_process_timeout_requires_a_finite_positive_value(
    tmp_path: Path, timeout: float
) -> None:
    helper = _write_process_helper(tmp_path, "raise SystemExit(0)\n")
    with pytest.raises(KiCadNativeError, match="finite and positive"):
        _run_native_command(
            Path(sys.executable),
            str(helper),
            b"",
            maximum_output_bytes=1024,
            timeout=timeout,
        )


def test_process_timeout_bounds_inherited_output_pipes(tmp_path: Path) -> None:
    helper = _write_process_helper(
        tmp_path,
        (
            "import subprocess, sys\n"
            "subprocess.Popen([sys.executable, '-c', "
            "'import time; time.sleep(1)'])\n"
        ),
    )
    started = time.monotonic()
    with pytest.raises(KiCadNativeError, match="timed out"):
        _run_native_command(
            Path(sys.executable),
            str(helper),
            b"",
            maximum_output_bytes=1024,
            timeout=0.2,
        )
    assert time.monotonic() - started < 1.0


def test_design_bundle_is_explicit_and_uses_relative_slots(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    design = _write_design(tmp_path)
    expected = KiCadNativeDesignFacts("0.1.0", {}, "(export)")
    captured: dict[str, object] = {}

    def fake_design_facts(**kwargs):
        captured.update(kwargs)
        bundle_root = Path(kwargs["bundle_root"])
        captured["staged_schematic"] = (bundle_root / "demo.kicad_sch").read_text(
            encoding="utf-8"
        )
        return expected

    monkeypatch.setattr("kicad_monkey.kicad_native.native_design_facts", fake_design_facts)
    result = native_design_facts_for_design(design, executable=tmp_path / "native.exe")

    assert result.engine_version == expected.engine_version
    assert result.design_fingerprint is not None
    assert Path(captured["bundle_root"]).name.startswith("kicad-monkey-native-")
    manifest = captured["manifest"]
    assert isinstance(manifest, dict)
    assert manifest["project_path"] == "demo.kicad_pro"
    assert manifest["root_schematic_path"] == "demo.kicad_sch"
    assert [source["path"] for source in manifest["sources"]] == [
        "demo.kicad_pro",
        "demo.kicad_sch",
    ]
    assert captured["file_slots"] == [
        {"slot": 0, "path": "demo.kicad_pro"},
        {"slot": 1, "path": "demo.kicad_sch"},
    ]


def test_design_bundle_serializes_current_in_memory_state(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    design = _write_design(tmp_path)
    assert design.top_schematic is not None
    design.top_schematic.title_block.title = "unsaved mutation"
    staged: dict[str, str] = {}

    def fake_design_facts(**kwargs):
        root = Path(kwargs["bundle_root"])
        staged["schematic"] = (root / "demo.kicad_sch").read_text(encoding="utf-8")
        return KiCadNativeDesignFacts("0.1.0", {}, "(export)")

    monkeypatch.setattr("kicad_monkey.kicad_native.native_design_facts", fake_design_facts)
    native_design_facts_for_design(design, executable=tmp_path / "native.exe")

    assert "unsaved mutation" in staged["schematic"]
    assert "unsaved mutation" not in (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")

"""Public design CLI compatibility and no-fallback evidence."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

import kicad_monkey.kicad_ir_to_svg as legacy_svg
import pytest
from jsonschema import Draft202012Validator
from kicad_cruncher import kicad_cruncher_cmd_design as design_cmd
from kicad_monkey import KiCadDesign, KiCadPcb, get_value, parse_sexp

_PACKAGE_ROOT = Path(__file__).resolve().parents[2]
_WORKSPACE = Path(__file__).resolve().parents[4]
_PROJECT = (
    _PACKAGE_ROOT / "tests" / "corpus" / "kicad" / "projects" / "hlr_test" / "hlr_test.kicad_pro"
)
_SCHEMA = _PACKAGE_ROOT / "docs" / "contracts" / "design_review_manifest.a0.schema.json"
_NATIVE_EXE = _WORKSPACE / "target" / "debug" / (
    "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
)


def _console_script(name: str) -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    # Do not resolve the virtual-environment interpreter symlink: console
    # scripts live beside that link, not beside uv's managed base interpreter.
    candidate = Path(sys.executable).parent / f"{name}{suffix}"
    assert candidate.is_file(), f"installed console script is required: {candidate}"
    return candidate


def _entry_matrix() -> tuple[tuple[str, list[str]], ...]:
    return (
        ("kicad-cruncher-design", [str(_console_script("kicad-cruncher")), "design"]),
        ("kcr-design-review", [str(_console_script("kcr")), "design-review"]),
        ("python-module-dr", [sys.executable, "-m", "kicad_cruncher", "dr"]),
    )


def _native_env(*, executable: Path = _NATIVE_EXE) -> dict[str, str]:
    env = dict(os.environ)
    env.update(
        {
            "KICAD_CRUNCHER_NATIVE_DESIGN_FACTS": "1",
            "KICAD_CRUNCHER_NATIVE_PHYSICAL": "1",
            "KICAD_MONKEY_NATIVE": str(executable),
            "NO_COLOR": "1",
            "PYTHONUTF8": "1",
        }
    )
    return env


def _run(command: list[str], *, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=_WORKSPACE,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )


def _load_manifest(output: Path) -> dict[str, Any]:
    manifest = json.loads((output / "design_review_manifest.json").read_text("utf-8"))
    schema = json.loads(_SCHEMA.read_text("utf-8"))
    Draft202012Validator(schema).validate(manifest)
    return manifest


def _artifact_files(manifest: dict[str, Any]) -> set[str]:
    graph = manifest["compiled_schematic_graph"]
    assert isinstance(graph, dict)
    schematic = manifest["schematic_svgs"]
    pcb = manifest["pcb_svgs"]
    assert isinstance(schematic, list)
    assert isinstance(pcb, list)
    return {
        "README.md",
        "design_review_manifest.json",
        str(manifest["design_json"]),
        str(graph["file"]),
        str(manifest["netlist_json"]),
        str(manifest["netlist_kicad_sexpr"]),
        *(str(item["file"]) for item in schematic),
        *(str(item["file"]) for item in pcb),
    }


def _assert_native_bundle(output: Path) -> dict[str, Any]:
    manifest = _load_manifest(output)
    actual_files = {
        path.relative_to(output).as_posix() for path in output.rglob("*") if path.is_file()
    }
    assert actual_files == _artifact_files(manifest)
    assert manifest["design_json"] == "hlr_test_design.json"
    assert manifest["netlist_json"] == "hlr_test_netlist.json"
    assert manifest["netlist_kicad_sexpr"] == "hlr_test_netlist.net"
    assert manifest["readme"] == "README.md"

    facts = manifest["design_facts"]
    assert isinstance(facts, dict)
    assert facts["backend"] == "kicad-monkey-native"
    assert facts["resource_profile"] == "design-facts-bounded-a1"
    assert isinstance(facts["engine_version"], str) and facts["engine_version"]

    graph_record = manifest["compiled_schematic_graph"]
    assert isinstance(graph_record, dict)
    graph = json.loads((output / str(graph_record["file"])).read_text("utf-8"))
    design = json.loads((output / str(manifest["design_json"])).read_text("utf-8"))
    assert graph == design["compiled_schematic_graph"]
    canonical_graph = json.dumps(
        graph, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    assert hashlib.sha256(canonical_graph).hexdigest() == facts[
        "compiled_schematic_graph_sha256"
    ]

    netlist_bytes = (output / str(manifest["netlist_kicad_sexpr"])).read_bytes()
    assert len(netlist_bytes) == facts["kicad_netlist_bytes"]
    assert hashlib.sha256(netlist_bytes).hexdigest() == facts["kicad_netlist_sha256"]
    netlist = parse_sexp(netlist_bytes.decode("utf-8"))
    design_block = next(
        child
        for child in netlist[1:]
        if isinstance(child, list) and child and child[0] == "design"
    )
    assert get_value(design_block, "source") == str(_PROJECT.with_suffix(".kicad_sch"))
    assert get_value(design_block, "date") == ""
    assert get_value(design_block, "tool") == "kicad_cruncher"

    schematic = manifest["schematic_svgs"]
    pcb = manifest["pcb_svgs"]
    assert isinstance(schematic, list) and len(schematic) == 1
    assert isinstance(pcb, list) and {item["layer"] for item in pcb} == {"F.Cu", "B.Cu"}
    assert all(
        "kicad_monkey.pcb.svg.enrichment.a0"
        in (output / str(item["file"])).read_text("utf-8")
        for item in pcb
    )
    return manifest


def _tree_digest(root: Path) -> dict[str, str]:
    return {
        path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def test_all_public_design_aliases_and_entrypoints_publish_the_exact_native_bundle(
    tmp_path: Path,
) -> None:
    assert _PROJECT.is_file()
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    outputs: list[dict[str, str]] = []
    for name, prefix in _entry_matrix():
        output = tmp_path / name
        completed = _run([*prefix, str(_PROJECT), "-o", str(output)], env=_native_env())
        assert completed.returncode == 0, completed.stdout + completed.stderr
        assert "Design review: starting bundle" in completed.stdout
        assert "Design review:" in completed.stdout
        assert completed.stderr == ""
        assert "Traceback" not in completed.stdout
        _assert_native_bundle(output)
        outputs.append(_tree_digest(output))

    assert outputs[0] == outputs[1] == outputs[2]


def test_all_public_entrypoints_return_2_for_usage_errors_without_artifacts(
    tmp_path: Path,
) -> None:
    for name, prefix in _entry_matrix():
        output = tmp_path / name
        completed = _run([*prefix, "--not-a-real-option", "-o", str(output)], env=_native_env())
        assert completed.returncode == 2
        assert completed.stdout == ""
        assert "usage:" in completed.stderr
        assert "unrecognized arguments" in completed.stderr
        assert "Traceback" not in completed.stderr
        assert not output.exists()


def test_all_public_entrypoints_return_1_without_partial_or_legacy_retry(
    tmp_path: Path,
) -> None:
    missing_native = tmp_path / "missing-native.exe"
    for name, prefix in _entry_matrix():
        output = tmp_path / name
        completed = _run(
            [*prefix, str(_PROJECT), "-o", str(output)],
            env=_native_env(executable=missing_native),
        )
        assert completed.returncode == 1
        assert "Design review generation failed" in completed.stdout
        assert completed.stderr == ""
        assert "Traceback" not in completed.stdout
        assert not output.exists()


def test_selected_native_providers_never_call_legacy_graph_netlist_or_pcb_svg(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", "1")
    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_PHYSICAL", "1")
    monkeypatch.setenv("KICAD_MONKEY_NATIVE", str(_NATIVE_EXE))

    def forbidden(*_args: object, **_kwargs: object) -> object:
        raise AssertionError("selected native provider retried through legacy Python")

    monkeypatch.setattr(
        "kicad_monkey.kicad_design_json.build_compiled_schematic_graph", forbidden
    )
    monkeypatch.setattr(KiCadDesign, "to_kicad_netlist_sexpr", forbidden)
    monkeypatch.setattr(KiCadPcb, "to_svg", forbidden)
    original_render_ir_to_svg = legacy_svg.render_ir_to_svg

    def forbid_pcb_ir_render(document: object, *args: object, **kwargs: object) -> object:
        if str(getattr(document, "source_kind", "")).upper().endswith("PCB"):
            return forbidden(document, *args, **kwargs)
        return original_render_ir_to_svg(document, *args, **kwargs)  # type: ignore[arg-type]

    monkeypatch.setattr(legacy_svg, "render_ir_to_svg", forbid_pcb_ir_render)

    output = tmp_path / "review"
    bundle = design_cmd.write_design_review_bundle(_PROJECT, output)
    assert bundle.manifest["design_facts"]["backend"] == "kicad-monkey-native"  # type: ignore[index]
    _assert_native_bundle(output)


def test_invalid_internal_manifest_fails_before_transactional_publication(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", "1")
    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_PHYSICAL", "1")
    monkeypatch.setenv("KICAD_MONKEY_NATIVE", str(_NATIVE_EXE))
    original = design_cmd._schematic_svg_artifact

    def unsafe_artifact(*args: object, **kwargs: object) -> dict[str, object]:
        artifact = original(*args, **kwargs)  # type: ignore[arg-type]
        artifact["file"] = "../escape.svg"
        return artifact

    monkeypatch.setattr(design_cmd, "_schematic_svg_artifact", unsafe_artifact)
    output = tmp_path / "review"
    with pytest.raises(ValueError, match="not safe bundle-relative"):
        design_cmd.write_design_review_bundle(_PROJECT, output)

    assert not output.exists()
    assert list(tmp_path.glob(".kicad-cruncher-design-*")) == []

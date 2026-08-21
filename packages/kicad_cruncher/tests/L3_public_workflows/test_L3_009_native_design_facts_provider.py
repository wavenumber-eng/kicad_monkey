"""Cross-package evidence for the Windows no-fallback design-facts provider."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

import pytest
from kicad_cruncher import kicad_cruncher_cmd_design as design_cmd
from kicad_cruncher import kicad_cruncher_native_design as native_design
from kicad_cruncher import kicad_cruncher_native_physical as native_physical
from kicad_cruncher.kicad_cruncher_native_design import (
    NativeDesignFactsProvider,
    use_native_design_facts_provider,
)
from kicad_monkey import (
    KiCadDesign,
    KiCadNativeDesignFacts,
    get_value,
    native_design_facts_for_design,
    parse_sexp,
)

_WORKSPACE = Path(__file__).resolve().parents[4]
_NATIVE_EXE = _WORKSPACE / "target" / "debug" / (
    "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
)


@pytest.mark.parametrize(
    ("machine", "selected_by_default"),
    (("AMD64", True), ("x86_64", True), ("ARM64", False), ("aarch64", False)),
)
def test_native_provider_default_is_windows_x64_only(
    monkeypatch: pytest.MonkeyPatch,
    machine: str,
    selected_by_default: bool,
) -> None:
    """Keep the hard switch on its governed x64 target; opt-in stays portable."""

    monkeypatch.delenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", raising=False)
    monkeypatch.delenv("KICAD_CRUNCHER_NATIVE_PHYSICAL", raising=False)
    monkeypatch.setattr(native_design.sys, "platform", "win32")
    monkeypatch.setattr(native_physical.sys, "platform", "win32")
    monkeypatch.setattr(native_design.platform, "machine", lambda: machine)
    monkeypatch.setattr(native_physical.platform, "machine", lambda: machine)
    assert native_design.use_native_design_facts_provider() is selected_by_default
    assert native_physical.use_native_physical_provider() is selected_by_default

    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", "1")
    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_PHYSICAL", "1")
    assert native_design.use_native_design_facts_provider()
    assert native_physical.use_native_physical_provider()


_SCHEMATIC = """(kicad_sch
  (version 20260306)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid root)
  (paper "A4")
  (lib_symbols
    (symbol "Demo:One"
      (symbol "Demo:One_1_1"
        (pin passive line (at 0 0 0) (name "P") (number "1")))))
  (symbol
    (lib_id "Demo:One")
    (lib_name "Demo:One")
    (at 0 0 0)
    (uuid root-symbol)
    (property "Reference" "R1")
    (property "Value" "One"))
)
"""


def _write_project(root: Path) -> Path:
    project = root / "demo.kicad_pro"
    project.write_text("{}\n", encoding="utf-8")
    (root / "demo.kicad_sch").write_text(_SCHEMATIC, encoding="utf-8")
    return project


def _native_facts(project: Path) -> KiCadNativeDesignFacts:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    design = KiCadDesign.from_project_file(project)
    return native_design_facts_for_design(
        design,
        source_path=str(project.with_suffix(".kicad_sch")),
        date="",
        tool="kicad_cruncher",
        executable=_NATIVE_EXE,
    )


def _schematic_enrichment(svg_path: Path) -> dict[str, object]:
    root = ET.fromstring(svg_path.read_text(encoding="utf-8"))
    metadata = next(
        element
        for element in root.iter()
        if element.tag.rsplit("}", 1)[-1] == "metadata"
        and element.attrib.get("id") == "schematic-enrichment-a0"
    )
    payload = json.loads("".join(metadata.itertext()))
    assert isinstance(payload, dict)
    return payload


class _CountingProvider:
    def __init__(self, facts: KiCadNativeDesignFacts) -> None:
        self.facts = facts
        self.calls = 0
        self.metadata: tuple[str, str, str] | None = None

    def design_facts(
        self,
        _design: KiCadDesign,
        *,
        source_path: str,
        date: str,
        tool: str,
    ) -> KiCadNativeDesignFacts:
        self.calls += 1
        self.metadata = (source_path, date, tool)
        return self.facts


def test_selected_native_facts_are_injected_once_without_python_graph_or_net_writer(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project = _write_project(tmp_path)
    facts = _native_facts(project)
    provider = _CountingProvider(facts)
    monkeypatch.setattr(design_cmd, "selected_design_facts_provider", lambda: provider)

    def forbidden(*_args: object, **_kwargs: object) -> object:
        raise AssertionError("selected native facts must not retry through Python")

    monkeypatch.setattr(
        "kicad_monkey.kicad_design_json.build_compiled_schematic_graph", forbidden
    )
    monkeypatch.setattr(KiCadDesign, "to_kicad_netlist_sexpr", forbidden)
    output = tmp_path / "review"
    bundle = design_cmd.write_design_review_bundle(project, output)

    assert provider.calls == 1
    assert provider.metadata == (str(tmp_path / "demo.kicad_sch"), "", "kicad_cruncher")
    design_payload = json.loads(bundle.design_json_path.read_text("utf-8"))
    graph = json.loads(bundle.compiled_schematic_graph_path.read_text("utf-8"))
    assert graph == facts.compiled_schematic_graph == design_payload["compiled_schematic_graph"]
    published_netlist = bundle.netlist_kicad_sexpr_path.read_bytes()
    assert published_netlist == facts.kicad_netlist.encode("utf-8")
    assert len(published_netlist) == facts.kicad_netlist_bytes
    assert hashlib.sha256(published_netlist).hexdigest() == facts.kicad_netlist_sha256
    assert json.loads(bundle.netlist_json_path.read_text("utf-8"))["schema"] == (
        "kicad_monkey.netlist.a0"
    )
    provenance = bundle.manifest["design_facts"]
    assert provenance == {
        "backend": "kicad-monkey-native",
        "engine_version": facts.engine_version,
        "resource_profile": "design-facts-bounded-a1",
        "source_snapshot_sha256": facts.source_snapshot_sha256,
        "compiled_schematic_graph_sha256": hashlib.sha256(
            json.dumps(
                graph,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            ).encode("utf-8")
        ).hexdigest(),
        "kicad_netlist_bytes": facts.kicad_netlist_bytes,
        "kicad_netlist_sha256": facts.kicad_netlist_sha256,
    }
    page_rows = graph["page_occurrences"]
    link_rows = graph["graphical_artifact_links"]
    schematic_svgs = bundle.manifest["schematic_svgs"]
    assert isinstance(page_rows, list)
    assert isinstance(link_rows, list)
    assert isinstance(schematic_svgs, list)
    page_refs = {row["id"] for row in page_rows if isinstance(row, dict)}
    link_refs = {row["id"] for row in link_rows if isinstance(row, dict)}
    for item in schematic_svgs:
        assert isinstance(item, dict)
        assert isinstance(item["file"], str)
        svg_path = output / item["file"]
        graph_view = _schematic_enrichment(svg_path)["compiled_schematic_graph_view"]
        assert isinstance(graph_view, dict)
        assert graph_view["page_occurrence_ref"] in page_refs
        graph_link_refs = graph_view["graphical_artifact_link_refs"]
        graph_artifact = graph_view["graph_artifact"]
        assert isinstance(graph_link_refs, list)
        assert all(isinstance(value, str) for value in graph_link_refs)
        assert isinstance(graph_artifact, str)
        assert set(graph_link_refs) <= link_refs
        assert (svg_path.parent / graph_artifact).resolve() == (
            bundle.compiled_schematic_graph_path.resolve()
        )


def test_native_facts_failure_preserves_the_published_tree(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project = _write_project(tmp_path)
    output = tmp_path / "review"
    output.mkdir()
    sentinel = output / "keep.txt"
    sentinel.write_text("previous", encoding="utf-8")

    class FailingProvider:
        def design_facts(self, *_args: object, **_kwargs: object) -> object:
            raise RuntimeError("native design-facts sentinel failure")

    monkeypatch.setattr(
        design_cmd, "selected_design_facts_provider", lambda: FailingProvider()
    )
    with pytest.raises(RuntimeError, match="native design-facts sentinel failure"):
        design_cmd.write_design_review_bundle(project, output)

    assert sentinel.read_text("utf-8") == "previous"
    assert sorted(path.name for path in output.iterdir()) == ["keep.txt"]


def test_provider_selection_is_windows_x64_or_explicit_opt_in(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", raising=False)
    monkeypatch.setattr(native_design.sys, "platform", "linux")
    assert not use_native_design_facts_provider()
    monkeypatch.setenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", "1")
    assert use_native_design_facts_provider()
    monkeypatch.setattr(native_design.sys, "platform", "win32")
    monkeypatch.setattr(native_design.platform, "machine", lambda: "AMD64")
    monkeypatch.delenv("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS", raising=False)
    assert use_native_design_facts_provider()


@pytest.mark.parametrize("command", ("design", "design-review", "dr"))
def test_public_cli_uses_deterministic_actual_native_design_facts(
    tmp_path: Path, command: str
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    project = _write_project(tmp_path)
    env = dict(os.environ)
    env["KICAD_CRUNCHER_NATIVE_DESIGN_FACTS"] = "1"
    env["KICAD_MONKEY_NATIVE"] = str(_NATIVE_EXE)
    outputs: list[tuple[dict[str, object], bytes, bytes]] = []
    for ordinal in range(2):
        output = tmp_path / f"{command}-{ordinal}"
        completed = subprocess.run(
            [sys.executable, "-m", "kicad_cruncher", command, str(project), "-o", str(output)],
            cwd=_WORKSPACE,
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=180,
            check=False,
        )
        assert completed.returncode == 0, completed.stdout + completed.stderr
        manifest = json.loads((output / "design_review_manifest.json").read_text("utf-8"))
        provenance = manifest["design_facts"]
        assert provenance["backend"] == "kicad-monkey-native"
        assert provenance["resource_profile"] == "design-facts-bounded-a1"
        netlist_text = (output / manifest["netlist_kicad_sexpr"]).read_text("utf-8")
        netlist_root = parse_sexp(netlist_text)
        design_block = next(
            child
            for child in netlist_root[1:]
            if isinstance(child, list) and child and child[0] == "design"
        )
        assert get_value(design_block, "source") == str(project.with_suffix(".kicad_sch"))
        assert get_value(design_block, "date") == ""
        assert get_value(design_block, "tool") == "kicad_cruncher"
        outputs.append(
            (
                provenance,
                (output / manifest["compiled_schematic_graph"]["file"]).read_bytes(),
                (output / manifest["netlist_kicad_sexpr"]).read_bytes(),
            )
        )
    assert outputs[0] == outputs[1]


def test_native_provider_forwards_established_netlist_metadata(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project = _write_project(tmp_path)
    design = KiCadDesign.from_project_file(project)
    captured: dict[str, object] = {}

    def fake_native(current: KiCadDesign, **kwargs: object) -> KiCadNativeDesignFacts:
        captured["design"] = current
        captured.update(kwargs)
        return _native_facts(project)

    monkeypatch.setattr("kicad_monkey.native_design_facts_for_design", fake_native)
    provider = NativeDesignFactsProvider(executable=tmp_path / "native.exe", timeout=7.0)
    provider.design_facts(
        design,
        source_path=str(tmp_path / "demo.kicad_sch"),
        date="",
        tool="kicad_cruncher",
    )

    assert captured["design"] is design
    assert captured["source_path"] == str(tmp_path / "demo.kicad_sch")
    assert captured["date"] == ""
    assert captured["tool"] == "kicad_cruncher"
    assert captured["executable"] == tmp_path / "native.exe"
    assert captured["timeout"] == 7.0

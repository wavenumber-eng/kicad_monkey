"""
Test L0_016: KiCadDesign aggregator (Phase F-6.5 follow-on)

Pure-unit coverage for ``kicad_design.KiCadDesign``: ties together
``.kicad_pro`` + ``.kicad_sch`` + ``.kicad_pcb`` for a single project,
and centralises cross-document ``${VAR}`` resolution by threading
``KiCadProject.text_variables`` into ``schematic_to_ir`` automatically.

Exercises:
- ``KiCadDesign`` exported from ``kicad_monkey`` (lazy loader)
- ``from_project_file`` discovers same-stem ``.kicad_sch`` and ``.kicad_pcb``
- ``from_schematic_file`` discovers adjacent ``.kicad_pro``
- ``from_pcb_file`` retains pcb_path + finds project but doesn't auto-load
- ``text_variables`` proxies the project's dict (returns a copy)
- ``text_variables`` is empty when no project attached
- ``top_schematic`` returns first loaded schematic (or None)
- ``pcb`` lazy-loads from ``pcb_path`` on first access
- ``to_schematic_ir`` threads ``project_vars`` from ``text_variables``
- ``to_schematic_ir`` ``extra_vars`` overrides project-level defaults
- ``to_schematic_ir`` raises when no schematic available
- Project-defined ``${VAR}`` actually substitutes through to the
  drawing-sheet text ops (the F-6.5 follow-on motivating example)
"""

from __future__ import annotations

import json

import msgspec
import pytest

from kicad_monkey import KiCadDesign
from kicad_monkey.kicad_project import KiCadProject
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_plotter_ir import KiCadPlotterOpKind


# ---------------------------------------------------------------------------
# Minimal fixtures
# ---------------------------------------------------------------------------


_MIN_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-3333-4444-555555555555")
  (paper "A4")
  (title_block
    (title "OriginalTitle")
    (date "2026-05-09")
    (rev "A")
  )
)
"""


def _write_min_sch(path):
    path.write_text(_MIN_SCH_TEXT, encoding="utf-8")


def _write_pro(path, text_variables):
    path.write_text(
        json.dumps({"text_variables": dict(text_variables)}, indent=2),
        encoding="utf-8",
    )


def _write_min_pcb(path):
    # Smallest s-expr KiCadPcb tolerates; we never actually parse this in
    # the tests below — we just want pcb_path to point at an existing file
    # for the discovery assertions.
    path.write_text(
        "(kicad_pcb (version 20241229) (generator \"pcbnew\") (general (thickness 1.6)))\n",
        encoding="utf-8",
    )


# ---------------------------------------------------------------------------
# Public surface
# ---------------------------------------------------------------------------


class TestPublicSurface:
    def test_kicad_design_lazy_exposed(self):
        # KiCadDesign should resolve via the package __getattr__ shim.
        import kicad_monkey

        assert kicad_monkey.KiCadDesign is KiCadDesign
        # And it must come from the dedicated module, not somewhere else.
        assert KiCadDesign.__module__ == "kicad_monkey.kicad_design"


# ---------------------------------------------------------------------------
# Constructors
# ---------------------------------------------------------------------------


class TestFromProjectFile:
    def test_from_project_file_loads_project_and_same_stem_schematic(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {"MY_TITLE": "Hello, world"})
        _write_min_sch(sch)

        d = KiCadDesign.from_project_file(pro)

        assert isinstance(d.project, KiCadProject)
        assert d.project.text_variables == {"MY_TITLE": "Hello, world"}
        assert d.project_path == pro
        assert len(d.schematics) == 1
        assert isinstance(d.schematics[0], KiCadSchematic)

    def test_from_project_file_picks_up_adjacent_pcb(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        pcb = tmp_path / "demo.kicad_pcb"
        _write_pro(pro, {})
        _write_min_sch(sch)
        _write_min_pcb(pcb)

        d = KiCadDesign.from_project_file(pro)
        assert d.pcb_path == pcb

    def test_from_project_file_no_schematic_yields_empty_list(self, tmp_path):
        # Project file alone — no `.kicad_sch` companion.
        pro = tmp_path / "lonely.kicad_pro"
        _write_pro(pro, {"FOO": "bar"})

        d = KiCadDesign.from_project_file(pro)
        assert d.schematics == []
        assert d.pcb_path is None
        assert d.project is not None  # project still loaded

    def test_from_project_file_skips_pcb_when_only_sch_present(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {})
        _write_min_sch(sch)

        d = KiCadDesign.from_project_file(pro)
        assert d.pcb_path is None


class TestFromSchematicFile:
    def test_from_schematic_file_discovers_adjacent_project(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {"MY_TITLE": "Bench"})
        _write_min_sch(sch)

        d = KiCadDesign.from_schematic_file(sch)
        assert d.project is not None
        assert d.project.text_variables["MY_TITLE"] == "Bench"
        assert len(d.schematics) == 1

    def test_from_schematic_file_no_project(self, tmp_path):
        sch = tmp_path / "orphan.kicad_sch"
        _write_min_sch(sch)

        d = KiCadDesign.from_schematic_file(sch)
        assert d.project is None
        assert d.project_path is None
        assert len(d.schematics) == 1


class TestFromPcbFile:
    def test_from_pcb_file_retains_path_and_finds_project(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        pcb = tmp_path / "demo.kicad_pcb"
        _write_pro(pro, {"X": "y"})
        _write_min_pcb(pcb)

        d = KiCadDesign.from_pcb_file(pcb)
        assert d.pcb_path == pcb
        assert d.project is not None
        # schematics list is empty — from_pcb_file doesn't crawl for sch.
        assert d.schematics == []

    def test_from_pcb_file_does_not_eagerly_parse_pcb(self, tmp_path):
        # An invalid .kicad_pcb on disk should still construct a design;
        # parse failure is deferred to the first .pcb access.
        pcb = tmp_path / "broken.kicad_pcb"
        pcb.write_text("not a real pcb file", encoding="utf-8")

        # No exception expected here.
        d = KiCadDesign.from_pcb_file(pcb)
        assert d.pcb_path == pcb
        assert d._pcb is None  # noqa: SLF001 — checking lazy flag


# ---------------------------------------------------------------------------
# Properties
# ---------------------------------------------------------------------------


class TestTextVariables:
    def test_text_variables_empty_without_project(self):
        d = KiCadDesign()
        assert d.text_variables == {}

    def test_text_variables_returns_copy(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        _write_pro(pro, {"A": "1", "B": "2"})
        d = KiCadDesign.from_project_file(pro)

        snap = d.text_variables
        snap["mutated"] = "yes"
        # The project's own dict must remain untouched.
        assert "mutated" not in d.project.text_variables


class TestTopSchematic:
    def test_top_schematic_none_when_empty(self):
        assert KiCadDesign().top_schematic is None

    def test_top_schematic_is_first(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {})
        _write_min_sch(sch)
        d = KiCadDesign.from_project_file(pro)
        assert d.top_schematic is d.schematics[0]


class TestDesignJsonCompiledGraphInjection:
    def test_prevalidated_graph_bypasses_graph_rebuild(self, tmp_path, monkeypatch):
        sch = tmp_path / "demo.kicad_sch"
        _write_min_sch(sch)
        design = KiCadDesign.from_schematic_file(sch)

        from kicad_monkey.kicad_compiled_schematic_graph import (
            build_compiled_schematic_graph,
        )
        from kicad_monkey.kicad_native import (
            KiCadNativeDesignFacts,
            _design_fingerprint,
        )

        graph = build_compiled_schematic_graph(design).to_json()
        facts = KiCadNativeDesignFacts(
            engine_version="0.1.0",
            compiled_schematic_graph=graph,
            kicad_netlist="(export (version \"E\"))",
            design_fingerprint=_design_fingerprint(design),
        )

        def unexpected_rebuild(_design):
            raise AssertionError("the injected compiled graph must be reused")

        monkeypatch.setattr(
            "kicad_monkey.kicad_design_json.build_compiled_schematic_graph",
            unexpected_rebuild,
        )
        payload = design.to_json(compiled_schematic_graph=facts)

        assert payload["compiled_schematic_graph"] == graph
        assert payload["compiled_schematic_graph"] is not graph

    def test_injected_graph_must_match_current_design_state(self, tmp_path):
        sch = tmp_path / "demo.kicad_sch"
        _write_min_sch(sch)
        design = KiCadDesign.from_schematic_file(sch)

        from kicad_monkey.kicad_compiled_schematic_graph import (
            build_compiled_schematic_graph,
        )
        from kicad_monkey.kicad_native import (
            KiCadNativeDesignFacts,
            _design_fingerprint,
        )

        facts = KiCadNativeDesignFacts(
            engine_version="0.1.0",
            compiled_schematic_graph=build_compiled_schematic_graph(design).to_json(),
            kicad_netlist="(export (version \"E\"))",
            design_fingerprint=_design_fingerprint(design),
        )
        assert design.top_schematic is not None
        design.top_schematic.title_block.title = "mutated"

        with pytest.raises(ValueError, match="current design state"):
            design.to_json(compiled_schematic_graph=facts)

    def test_injected_graph_is_structurally_strict(self, tmp_path):
        sch = tmp_path / "demo.kicad_sch"
        _write_min_sch(sch)
        design = KiCadDesign.from_schematic_file(sch)

        from kicad_monkey.kicad_compiled_schematic_graph import (
            build_compiled_schematic_graph,
        )
        from kicad_monkey.kicad_native import (
            KiCadNativeDesignFacts,
            _design_fingerprint,
        )

        graph = build_compiled_schematic_graph(design).to_json()
        graph["unexpected"] = True
        facts = KiCadNativeDesignFacts(
            engine_version="0.1.0",
            compiled_schematic_graph=graph,
            kicad_netlist="(export (version \"E\"))",
            design_fingerprint=_design_fingerprint(design),
        )

        with pytest.raises(msgspec.ValidationError, match="unknown field"):
            design.to_json(compiled_schematic_graph=facts)


# ---------------------------------------------------------------------------
# Cross-document ${VAR} resolution — the motivating use case
# ---------------------------------------------------------------------------


def _find_title_substituted(doc, expected):
    """Scan the leading sheet_header record's Text ops for ``expected``."""
    header = doc.records[0]
    assert header.kind == "sheet_header"
    for op in header.operations:
        if op.kind != KiCadPlotterOpKind.TEXT:
            continue
        body = op.payload.get("text", "")
        if expected in body:
            return True
    return False


class TestToSchematicIr:
    def test_to_schematic_ir_threads_text_variables(self, tmp_path):
        # Title-block built-ins win over project variables, matching
        # KiCad's worksheet resolver.  Project variables still thread
        # through when the title-block field itself references them.
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {"TITLE": "ProjTitle", "CUSTOM_TITLE": "CustomTitle"})
        sch.write_text(
            _MIN_SCH_TEXT.replace("OriginalTitle", "${CUSTOM_TITLE}"),
            encoding="utf-8",
        )

        d = KiCadDesign.from_project_file(pro)
        doc = d.to_schematic_ir()

        assert _find_title_substituted(doc, "CustomTitle")
        assert not _find_title_substituted(doc, "ProjTitle")
        assert not _find_title_substituted(doc, "OriginalTitle")

    def test_to_schematic_ir_without_project_uses_title_block(self, tmp_path):
        # Sanity check: with no project attached, the title-block's own
        # title is what surfaces in the drawing-sheet text — no
        # cross-doc override is silently injected.
        sch = tmp_path / "demo.kicad_sch"
        _write_min_sch(sch)
        d = KiCadDesign.from_schematic_file(sch)

        doc = d.to_schematic_ir()
        assert _find_title_substituted(doc, "OriginalTitle")

    def test_to_schematic_ir_extra_vars_overrides_project(self, tmp_path):
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {"CUSTOM_TITLE": "ProjTitle"})
        sch.write_text(
            _MIN_SCH_TEXT.replace("OriginalTitle", "${CUSTOM_TITLE}"),
            encoding="utf-8",
        )

        d = KiCadDesign.from_project_file(pro)
        doc = d.to_schematic_ir(extra_vars={"CUSTOM_TITLE": "OverrideTitle"})

        assert _find_title_substituted(doc, "OverrideTitle")
        assert not _find_title_substituted(doc, "ProjTitle")

    def test_to_schematic_ir_no_schematic_raises(self):
        d = KiCadDesign()
        with pytest.raises(ValueError, match="no schematic"):
            d.to_schematic_ir()

    def test_to_schematic_ir_explicit_schematic_argument(self, tmp_path):
        # A design loaded from a project may carry a top schematic, but
        # callers can pass any other parsed schematic explicitly. The
        # explicit one wins, and project_vars still threads through.
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        _write_pro(pro, {"CUSTOM_TITLE": "ProjTitle"})
        _write_min_sch(sch)

        # A second standalone schematic with its own title-block.
        alt = KiCadSchematic.from_text(
            """(kicad_sch (version 20250114) (generator "eeschema")
              (generator_version "9.0")
              (uuid "deadbeef-0000-0000-0000-000000000000")
              (paper "A4")
              (title_block (title "${CUSTOM_TITLE}"))
            )
            """
        )

        d = KiCadDesign.from_project_file(pro)
        doc = d.to_schematic_ir(alt)
        assert _find_title_substituted(doc, "ProjTitle")


# ---------------------------------------------------------------------------
# Lazy PCB
# ---------------------------------------------------------------------------


class TestPcbLazyLoad:
    def test_pcb_returns_none_without_path(self):
        assert KiCadDesign().pcb is None


# ---------------------------------------------------------------------------
# Recursive ${VAR} expansion through the full design pipeline
# ---------------------------------------------------------------------------


_NESTED_SCH_TEXT = """(kicad_sch (version 20250114) (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-3333-4444-555555555555")
  (paper "A4")
  (title_block
    (title "${MY_TITLE}")
    (date "2026-05-09")
    (rev "A")
  )
)
"""


class TestRecursiveProjectVars:
    def test_title_block_var_resolved_via_project_text_variables(self, tmp_path):
        # Canonical user-facing scenario: schematic's title-block
        # field is the literal ``${MY_TITLE}`` placeholder; the
        # ``.kicad_pro`` defines ``MY_TITLE``. With recursive
        # ``${VAR}`` expansion the rendered drawing-sheet ``${TITLE}``
        # tbtext picks up the project's value via two passes.
        pro = tmp_path / "demo.kicad_pro"
        sch = tmp_path / "demo.kicad_sch"
        pro.write_text(
            json.dumps({"text_variables": {"MY_TITLE": "RealTitle"}}, indent=2),
            encoding="utf-8",
        )
        sch.write_text(_NESTED_SCH_TEXT, encoding="utf-8")

        d = KiCadDesign.from_project_file(pro)
        doc = d.to_schematic_ir()

        assert _find_title_substituted(doc, "RealTitle")
        # And the literal placeholder must NOT survive into the
        # rendered tbtext.
        assert not _find_title_substituted(doc, "${MY_TITLE}")

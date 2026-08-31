"""
Test L1_011: KiCad Project (.kicad_pro) Reader Tests

Phase C Slice C-1: pure-read coverage for ``KiCadProject`` against the
upstream-QA mirror. The reader preserves the full JSON in ``raw`` so a
later mutation slice (C-7) can round-trip without loss; this stratum
only checks the typed views align with the underlying JSON.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from kicad_monkey import KiCadProject, KiCadProjectSidecar, ProjectVariant
from kicad_monkey.testing.corpus import get_kicad_upstream_qa_dir


def _all_kicad_pro_files() -> list[Path]:
    return sorted(get_kicad_upstream_qa_dir().rglob("*.kicad_pro"))


def _kicad_pro_ids() -> list[str]:
    return [
        f.relative_to(get_kicad_upstream_qa_dir()).as_posix()
        for f in _all_kicad_pro_files()
    ]


PROJECT_FILES = _all_kicad_pro_files()


# ===========================================================================
# Reader basics
# ===========================================================================


class TestKiCadProjectReader:
    """Basic parse coverage for every .kicad_pro in the upstream QA mirror."""

    @pytest.mark.parametrize("project_file", PROJECT_FILES, ids=_kicad_pro_ids())
    def test_from_file_loads(self, project_file: Path) -> None:
        proj = KiCadProject.from_file(project_file)
        assert proj.project_path == project_file
        assert isinstance(proj.raw, dict)
        assert proj.raw, "raw JSON must not be empty"

    @pytest.mark.parametrize("project_file", PROJECT_FILES, ids=_kicad_pro_ids())
    def test_from_text_matches_from_file(self, project_file: Path) -> None:
        from_file = KiCadProject.from_file(project_file)
        from_text = KiCadProject.from_text(project_file.read_text(encoding="utf-8"))
        assert from_text.raw == from_file.raw
        assert from_text.variants == from_file.variants

    @pytest.mark.parametrize("project_file", PROJECT_FILES, ids=_kicad_pro_ids())
    def test_meta_filename_matches(self, project_file: Path) -> None:
        """``meta.filename`` (when present) is surfaced via ``get_path``."""
        proj = KiCadProject.from_file(project_file)
        raw_meta = proj.raw.get("meta", {}) or {}
        if "filename" in raw_meta:
            assert proj.get_path("meta.filename") == raw_meta["filename"]

    @pytest.mark.parametrize("project_file", PROJECT_FILES, ids=_kicad_pro_ids())
    def test_legacy_lib_list_present_when_in_raw(self, project_file: Path) -> None:
        proj = KiCadProject.from_file(project_file)
        sch = proj.raw.get("schematic", {}) or {}
        if "legacy_lib_list" in sch:
            assert proj.get_path("schematic.legacy_lib_list") == sch["legacy_lib_list"]


# ===========================================================================
# Variant catalog (the C-1 deliverable)
# ===========================================================================


class TestKiCadProjectVariants:
    """``KiCadProject.variants`` mirrors ``schematic.variants`` exactly."""

    @pytest.mark.parametrize("project_file", PROJECT_FILES, ids=_kicad_pro_ids())
    def test_variant_catalog_matches_json(self, project_file: Path) -> None:
        raw = json.loads(project_file.read_text(encoding="utf-8"))
        expected = (raw.get("schematic", {}) or {}).get("variants", []) or []
        proj = KiCadProject.from_file(project_file)

        assert len(proj.variants) == len(expected), (
            f"variant count mismatch on {project_file.name}"
        )
        for typed, source in zip(proj.variants, expected):
            assert isinstance(typed, ProjectVariant)
            assert typed.name == str(source.get("name", "") or "")
            if "description" in source and source["description"] is not None:
                assert typed.description == str(source["description"])
            else:
                assert typed.description is None

    def test_variants_kicad_pro_catalog(self) -> None:
        """The cli/variants fixture is the canonical C-6 BOM-oracle target."""
        proj = KiCadProject.from_file(
            get_kicad_upstream_qa_dir() / "cli" / "variants" / "variants.kicad_pro"
        )
        names = [v.name for v in proj.variants]
        assert names == ["Variant 1", "Variant2"], (
            "cli/variants fixture must surface the upstream variant catalog "
            "for the C-6 BOM oracle"
        )
        descriptions = {v.name: v.description for v in proj.variants}
        assert descriptions["Variant 1"] == "testing variant 1 description"
        assert descriptions["Variant2"] == "Test of variant 2 desc"

    def test_no_variants_block_means_empty_list(self, tmp_path: Path) -> None:
        """A project with no variants block exposes ``variants == []``."""
        sample = tmp_path / "noversions.kicad_pro"
        sample.write_text('{"schematic": {}, "meta": {"version": 3}}', encoding="utf-8")
        proj = KiCadProject.from_file(sample)
        assert proj.variants == []


class TestKiCadProjectBusAliases:
    """KiCad 10 ``schematic.bus_aliases`` normalized semantic view."""

    def test_names_and_members_follow_kicad_whitespace_rules(self) -> None:
        project = KiCadProject.from_json_dict(
            {
                "schematic": {
                    "bus_aliases": {
                        " CTRL ": [" A ", "", 12, " A "],
                        "EMPTY": ["   ", None],
                        "CTRL": ["B", "B"],
                        "IGNORED": "not-an-array",
                        "   ": ["X"],
                    }
                }
            }
        )

        assert project.bus_aliases == {
            "EMPTY": [],
            "CTRL": ["B", "B"],
        }

    @pytest.mark.parametrize(
        "source",
        [
            {},
            {"schematic": None},
            {"schematic": ["not-an-object"]},
            {"schematic": {"bus_aliases": []}},
            {"schematic": {"bus_aliases": {"A": None}}},
        ],
    )
    def test_missing_and_wrong_shaped_alias_data_is_ignored(self, source: dict) -> None:
        assert KiCadProject.from_json_dict(source).bus_aliases == {}

    def test_raw_json_is_preserved_while_typed_aliases_are_normalized(self) -> None:
        raw = {"schematic": {"bus_aliases": {" CTRL ": [" A ", ""]}}}
        project = KiCadProject.from_json_dict(raw)

        assert project.bus_aliases == {"CTRL": ["A"]}
        assert project.raw == raw

    def test_alias_view_is_fresh_and_set_path_updates_the_authoritative_raw_data(
        self,
    ) -> None:
        project = KiCadProject.from_json_dict(
            {"schematic": {"bus_aliases": {"CTRL": ["A", "B"]}}}
        )

        detached = project.bus_aliases
        detached["CTRL"] = []
        assert project.bus_aliases == {"CTRL": ["A", "B"]}

        project.set_path("schematic.bus_aliases", {"CTRL": [], "NEXT": [" C "]})
        assert project.bus_aliases == {"CTRL": [], "NEXT": ["C"]}
        assert project.to_json()["schematic"]["bus_aliases"] == {
            "CTRL": [],
            "NEXT": [" C "],
        }


# ===========================================================================
# Backward-compat alias
# ===========================================================================


class TestKiCadProjectSidecarAlias:
    """`KiCadProjectSidecar` continues to import / behave like `KiCadProject`."""

    def test_alias_is_kicad_project(self) -> None:
        assert KiCadProjectSidecar is KiCadProject

    def test_alias_loads_existing_fixture(self) -> None:
        proj = KiCadProjectSidecar.from_file(
            get_kicad_upstream_qa_dir() / "cli" / "variants" / "variants.kicad_pro"
        )
        assert proj.variants  # non-empty since fixture has 2 variants
        assert proj.net_settings is not None

"""Contract and failure-boundary tests for the design-review manifest."""

from __future__ import annotations

import argparse
import copy
import json
import logging
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator
from kicad_cruncher import kicad_cruncher_cmd_design as design_cmd

_PACKAGE_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _PACKAGE_ROOT / "docs" / "contracts" / "design_review_manifest.a0.schema.json"


def _manifest() -> dict[str, Any]:
    counts = {
        "unit_definitions": 1,
        "page_definitions": 1,
        "unit_occurrences": 1,
        "page_occurrences": 1,
        "hierarchy_occurrences": 0,
        "component_occurrences": 1,
        "local_net_occurrences": 0,
        "terminal_occurrences": 1,
        "hierarchy_terminal_bindings": 0,
        "graphical_artifact_links": 1,
    }
    return {
        "schema": "kicad_cruncher.design_review_manifest.a0",
        "input": "C:/fixture/demo.kicad_pro",
        "design_json": "demo_design.json",
        "compiled_schematic_graph": {
            "file": "demo_compiled_schematic_graph.json",
            "schema": "kicad_monkey.compiled_schematic_graph.a0",
            "type": "sch.compiled_schematic_graph",
            "identity_namespace": "sch.compiled_schematic_graph.a0",
            "counts": counts,
            "linkage_contract": "kicad_monkey.schematic.svg.compiled_graph_linkage.a0",
        },
        "netlist_json": "demo_netlist.json",
        "netlist_kicad_sexpr": "demo_netlist.net",
        "schematic_svgs": [
            {
                "file": "schematics/01_demo.svg",
                "sheet_number": 1,
                "sheet_count": 1,
                "sheet_name": "demo",
                "sheet_path": "/",
                "sheet_path_uuids": "/",
                "sheet_instance_path": "/root",
                "source": "C:/fixture/demo.kicad_sch",
                "page_occurrence_ref": "sch.page_occurrence:root",
                "artifact_key": "sch.dwg_scene",
                "graph_link_count": 1,
                "resolved_svg_identity_count": 1,
            }
        ],
        "pcb_svgs": [
            {
                "file": "pcb/copper_layers/demo__F.Cu__review.svg",
                "layer": "F.Cu",
                "included_layers": ["F.Cu", "Edge.Cuts"],
                "drill_slot_record_count": 0,
            }
        ],
        "readme": "README.md",
        "design_facts": {
            "backend": "kicad-monkey-native",
            "engine_version": "0.5.0",
            "resource_profile": "design-facts-bounded-a1",
            "source_snapshot_sha256": "1" * 64,
            "compiled_schematic_graph_sha256": "2" * 64,
            "kicad_netlist_bytes": 123,
            "kicad_netlist_sha256": "3" * 64,
        },
    }


def _validator() -> Draft202012Validator:
    schema = json.loads(_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_design_review_manifest_schema_accepts_native_and_retained_python_wire() -> None:
    validator = _validator()
    native = _manifest()
    validator.validate(native)

    retained_python = copy.deepcopy(native)
    del retained_python["design_facts"]
    validator.validate(retained_python)


@pytest.mark.parametrize(
    "mutate",
    (
        lambda value: value.update({"unexpected": True}),
        lambda value: value["compiled_schematic_graph"].update({"unexpected": True}),
        lambda value: value["compiled_schematic_graph"]["counts"].pop("unit_definitions"),
        lambda value: value["schematic_svgs"][0].update({"unexpected": True}),
        lambda value: value["design_facts"].update({"kicad_netlist_sha256": "A" * 64}),
        lambda value: value.__setitem__("design_json", "/absolute.json"),
        lambda value: value.__setitem__("design_json", "C:/absolute.json"),
        lambda value: value.__setitem__("design_json", "nested\\artifact.json"),
        lambda value: value.__setitem__("design_json", "."),
        lambda value: value.__setitem__("design_json", ".."),
        lambda value: value.__setitem__("design_json", "nested/./artifact.json"),
        lambda value: value.__setitem__("design_json", "nested/../artifact.json"),
        lambda value: value.__setitem__("design_json", "nested//artifact.json"),
    ),
)
def test_design_review_manifest_schema_rejects_non_wire_mutations(mutate: Any) -> None:
    manifest = _manifest()
    mutate(manifest)
    assert list(_validator().iter_errors(manifest))


def test_prepublication_manifest_validation_requires_contained_existing_files(
    tmp_path: Path,
) -> None:
    manifest = _manifest()
    for relative in design_cmd._manifest_artifact_paths(manifest):
        artifact = tmp_path.joinpath(*relative.split("/"))
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(b"fixture")

    design_cmd._validate_design_review_manifest_artifacts(manifest, tmp_path)

    manifest["design_json"] = "nested/../escape.json"
    with pytest.raises(ValueError, match="not safe bundle-relative"):
        design_cmd._validate_design_review_manifest_artifacts(manifest, tmp_path)

    manifest["design_json"] = "missing.json"
    with pytest.raises(ValueError, match="does not exist before publication"):
        design_cmd._validate_design_review_manifest_artifacts(manifest, tmp_path)


def test_cmd_design_failure_leaves_new_destination_absent(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    source = tmp_path / "demo.kicad_sch"
    source.write_text("(kicad_sch)", encoding="utf-8")
    output = tmp_path / "review"

    def fail(*_args: object, **_kwargs: object) -> object:
        raise RuntimeError("native sentinel failure")

    monkeypatch.setattr(design_cmd, "write_design_review_bundle", fail)
    args = argparse.Namespace(file=str(source), output=output, no_indexes=False)
    with caplog.at_level(logging.ERROR):
        assert design_cmd.cmd_design(args) == 1

    assert not output.exists()
    assert "Design review generation failed: native sentinel failure" in caplog.text


def test_cmd_design_handles_output_resolution_failure_without_traceback(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    source = tmp_path / "demo.kicad_sch"
    source.write_text("(kicad_sch)", encoding="utf-8")

    def fail(_output: Path | None) -> Path:
        raise OSError("output resolution sentinel failure")

    monkeypatch.setattr(design_cmd, "_resolve_design_output_dir", fail)
    args = argparse.Namespace(file=str(source), output=tmp_path / "review", no_indexes=False)
    with caplog.at_level(logging.ERROR):
        assert design_cmd.cmd_design(args) == 1

    assert "Design review generation failed: output resolution sentinel failure" in caplog.text

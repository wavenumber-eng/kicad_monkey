"""Rack-owned signoff for Rust L0, TypeSpec generation, and WASM."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

from wn_dev_std.rust_policy import check_rust_policy

from _toolchain_paths import typespec_executable


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = PACKAGE_ROOT / "contracts" / "generated" / "schema"
SCOPE_PATH = PACKAGE_ROOT / "tests" / "parity" / "scope.toml"
PHASE5_FREEZE_PATH = PACKAGE_ROOT / "tests" / "parity" / "plotter_ir_phase5_freeze.json"
PHASE5_FREEZE_RELATIVE_PATH = "tests/parity/plotter_ir_phase5_freeze.json"
PHASE6_FREEZE_PATH = (
    PACKAGE_ROOT / "tests" / "parity" / "native_phase6_exit_freeze.json"
)
PHASE6_FREEZE_RELATIVE_PATH = "tests/parity/native_phase6_exit_freeze.json"
_STRICT_PLOT_SCHEMA_NAMES = {
    f"{producer}Plot{suffix}.json"
    for producer in ("Board", "Footprint", "Schematic", "Symbol")
    for suffix in ("Document", "Request", "Result")
}
_PHASE5_UNION_ARM_COUNTS = {
    ("FootprintPlotDocument.json", "PlotterOperation"): 14,
    ("SymbolPlotDocument.json", "SymbolPlotRecord"): 2,
    ("BoardPlotDocument.json", "BoardPlotRecord"): 10,
    ("BoardPlotDocument.json", "BoardFootprintOperation"): 15,
    ("SchematicPlotDocument.json", "SchematicPlotRecord"): 23,
    ("SchematicPlotDocument.json", "SchematicSymbolOperation"): 16,
    ("SchematicPlotDocument.json", "SchematicSheetOperation"): 6,
}
_PHASE6_ARTIFACT_PATHS = (
    "contracts/generated/schema/NativeDesignFactsRequest.json",
    "contracts/generated/schema/NativeDesignFactsRequestA1.json",
    "contracts/generated/schema/NativeDesignFactsResult.json",
    "contracts/generated/schema/NativeDesignFactsResultA1.json",
    "contracts/generated/schema/NativeError.json",
    "contracts/generated/schema/NativeHandshake.json",
    "contracts/generated/schema/NativeHandshakeA1.json",
    "contracts/generated/schema/NativeHandshakeA2.json",
    "contracts/generated/schema/NativeSvgRenderRequest.json",
    "contracts/generated/schema/NativeSvgRenderResult.json",
    "contracts/generated/schema/SourceBundleManifest.json",
    "contracts/generated/schema/CompiledSchematicGraph.json",
    "packages/kicad_cruncher/docs/contracts/command_manifest.a0.json",
    "packages/kicad_cruncher/docs/contracts/design_review_manifest.a0.schema.json",
)
_PHASE6_HANDSHAKE_PATHS = (
    "contracts/generated/schema/NativeHandshake.json",
    "contracts/generated/schema/NativeHandshakeA1.json",
    "contracts/generated/schema/NativeHandshakeA2.json",
)
_PHASE6_RACK_WORKFLOWS = (
    "phase6-exit.yml",
    "phase6-native-design-facts.yml",
    "phase6-native-full-cli.yml",
    "phase6-native-physical-provider.yml",
    "phase6-native-svg.yml",
)


def _run(command: list[str], *, timeout: int = 300) -> None:
    environment = os.environ.copy()
    environment["CARGO_BUILD_JOBS"] = "4"
    environment["RUST_TEST_THREADS"] = "2"
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
        env=environment,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )


def _schema_hashes() -> dict[str, str]:
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(SCHEMA_ROOT.glob("*.json"))
    }


def _canonical_json_sha256(path: Path) -> str:
    payload = json.loads(path.read_text(encoding="utf-8"))
    canonical = json.dumps(
        payload,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def _phase5_freeze_path(relative_path: str) -> Path:
    path = (PACKAGE_ROOT / relative_path).resolve()
    path.relative_to(PACKAGE_ROOT.resolve())
    return path


def _kind_literals(schema: dict[str, object], definition_name: str) -> list[str]:
    definitions = schema["$defs"]
    assert isinstance(definitions, dict)
    definition = definitions[definition_name]
    assert isinstance(definition, dict)
    properties = definition["properties"]
    assert isinstance(properties, dict)
    kind = properties["kind"]
    assert isinstance(kind, dict)
    if "const" in kind:
        assert isinstance(kind["const"], str)
        return [kind["const"]]
    kind_ref = kind.get("$ref")
    assert isinstance(kind_ref, str) and kind_ref.startswith("#/$defs/")
    kind_definition = definitions[kind_ref.removeprefix("#/$defs/")]
    assert isinstance(kind_definition, dict)
    literals = kind_definition["enum"]
    assert isinstance(literals, list) and all(
        isinstance(literal, str) for literal in literals
    )
    return literals


def _union_arms(
    schema: dict[str, object], definition_name: str
) -> list[dict[str, object]]:
    definitions = schema["$defs"]
    assert isinstance(definitions, dict)
    definition = definitions[definition_name]
    assert isinstance(definition, dict)
    arms = definition["anyOf"]
    assert isinstance(arms, list)
    projection: list[dict[str, object]] = []
    for arm in arms:
        assert isinstance(arm, dict) and set(arm) == {"$ref"}
        reference = arm["$ref"]
        assert isinstance(reference, str) and reference.startswith("#/$defs/")
        projection.append(
            {
                "ref": reference,
                "kinds": _kind_literals(schema, reference.removeprefix("#/$defs/")),
            }
        )
    return projection


def test_phase5_plotter_contract_freeze_manifest_is_exact() -> None:
    manifest = json.loads(PHASE5_FREEZE_PATH.read_text(encoding="utf-8"))
    assert manifest["schema"] == "kicad_monkey.plotter_ir_phase5_freeze.v1"
    assert manifest["contract_version"] == "a0"

    artifacts = manifest["artifacts"]
    assert isinstance(artifacts, list)
    artifact_paths = [entry["path"] for entry in artifacts]
    assert len(artifact_paths) == len(set(artifact_paths)) == 12
    assert {Path(path).name for path in artifact_paths} == _STRICT_PLOT_SCHEMA_NAMES
    for entry in artifacts:
        path = _phase5_freeze_path(entry["path"])
        assert path.parent == SCHEMA_ROOT.resolve()
        assert _canonical_json_sha256(path) == entry["canonical_sha256"], (
            f"frozen strict schema changed: {entry['path']}"
        )

    unions = manifest["unions"]
    assert isinstance(unions, list)
    union_keys = {(Path(entry["path"]).name, entry["definition"]) for entry in unions}
    assert union_keys == set(_PHASE5_UNION_ARM_COUNTS)
    for entry in unions:
        path = _phase5_freeze_path(entry["path"])
        key = (path.name, entry["definition"])
        expected_count = _PHASE5_UNION_ARM_COUNTS[key]
        assert entry["expected_arm_count"] == expected_count
        assert len(entry["arms"]) == expected_count
        schema = json.loads(path.read_text(encoding="utf-8"))
        assert _union_arms(schema, entry["definition"]) == entry["arms"], (
            f"frozen union changed: {path.name}#/$defs/{entry['definition']}"
        )
        for mirror_relative_path in entry.get("mirror_paths", []):
            mirror_path = _phase5_freeze_path(mirror_relative_path)
            assert mirror_path.parent == SCHEMA_ROOT.resolve()
            mirror_schema = json.loads(mirror_path.read_text(encoding="utf-8"))
            assert _union_arms(mirror_schema, entry["definition"]) == entry["arms"], (
                f"frozen union mirror changed: "
                f"{mirror_path.name}#/$defs/{entry['definition']}"
            )


def _handshake_operations(schema: dict[str, object]) -> list[str]:
    properties = schema["properties"]
    assert isinstance(properties, dict)
    operations = properties["operations"]
    assert isinstance(operations, dict)
    if "prefixItems" in operations:
        items = operations["prefixItems"]
        assert isinstance(items, list)
        values = [item["const"] for item in items]
    else:
        item = operations["items"]
        assert isinstance(item, dict)
        values = [item["const"]]
    assert all(isinstance(value, str) for value in values)
    assert operations["minItems"] == operations["maxItems"] == len(values)
    return values


def test_phase6_native_contract_and_cli_freeze_manifest_is_exact() -> None:
    manifest = json.loads(PHASE6_FREEZE_PATH.read_text(encoding="utf-8"))
    assert manifest["schema"] == "kicad_monkey.native_phase6_exit_freeze.v1"
    artifacts = manifest["artifacts"]
    assert isinstance(artifacts, list)
    paths = [entry["path"] for entry in artifacts]
    assert paths == list(_PHASE6_ARTIFACT_PATHS)
    assert len(paths) == len(set(paths)) == len(_PHASE6_ARTIFACT_PATHS)
    assert sum(Path(path).name.startswith("Native") for path in paths) == 10
    assert {Path(path).name for path in paths if "generated/schema" in path} == {
        "NativeDesignFactsRequest.json",
        "NativeDesignFactsRequestA1.json",
        "NativeDesignFactsResult.json",
        "NativeDesignFactsResultA1.json",
        "NativeError.json",
        "NativeHandshake.json",
        "NativeHandshakeA1.json",
        "NativeHandshakeA2.json",
        "NativeSvgRenderRequest.json",
        "NativeSvgRenderResult.json",
        "SourceBundleManifest.json",
        "CompiledSchematicGraph.json",
    }
    for entry in artifacts:
        path = _phase5_freeze_path(entry["path"])
        assert path.is_file(), entry["path"]
        assert _canonical_json_sha256(path) == entry["canonical_sha256"], (
            f"frozen Phase 6 contract changed: {entry['path']}"
        )
    handshakes = manifest["handshakes"]
    assert [entry["path"] for entry in handshakes] == list(
        _PHASE6_HANDSHAKE_PATHS
    )
    assert [entry["operations"] for entry in handshakes] == [
        ["design-facts"],
        ["design-facts", "render-svg"],
        ["design-facts", "render-svg", "design-facts-a1"],
    ]
    for entry in handshakes:
        path = _phase5_freeze_path(entry["path"])
        schema = json.loads(path.read_text(encoding="utf-8"))
        assert _handshake_operations(schema) == entry["operations"]
    phase5 = manifest["phase5_freeze"]
    assert phase5["path"] == PHASE5_FREEZE_RELATIVE_PATH
    assert _canonical_json_sha256(PHASE5_FREEZE_PATH) == phase5["canonical_sha256"]


def test_l0_parity_registry_governs_required_implementation_gaps() -> None:
    payload = tomllib.loads(SCOPE_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.parity_scope.a0"
    assert payload["review_state"] in {"in_progress", "accepted"}
    surfaces = payload["surfaces"]
    assert len({surface["id"] for surface in surfaces}) == len(surfaces)
    rack_case_paths: dict[str, str] = {}
    for stratum_path in (PACKAGE_ROOT / "tests").rglob("STRATUM.toml"):
        stratum = tomllib.loads(stratum_path.read_text(encoding="utf-8"))
        for subtest in stratum.get("subtests", []):
            rack_case = subtest.get("id")
            if rack_case is None:
                stem_parts = Path(subtest["file"]).stem.removeprefix("test_").split("_")
                if len(stem_parts) < 2:
                    continue
                rack_case = "_".join(stem_parts[:2])
            test_path = (stratum_path.parent / subtest["file"]).relative_to(
                PACKAGE_ROOT
            )
            normalized_path = test_path.as_posix()
            assert rack_case not in rack_case_paths, f"duplicate Rack id: {rack_case}"
            rack_case_paths[rack_case] = normalized_path
    for surface in surfaces:
        disposition = surface["disposition"]
        status = surface["status"]
        assert disposition in {
            "required",
            "replaced_by_contract",
            "required_bounded_slice",
            "deferred_geometer_service",
        }, surface["id"]
        assert status in {"planned", "review_ready", "closed", "deferred"}, surface[
            "id"
        ]
        if disposition in {"required", "replaced_by_contract"}:
            assert status in {"closed", "review_ready"}, surface["id"]
        elif disposition == "required_bounded_slice":
            assert status in {"planned", "review_ready", "closed"}, surface["id"]
        else:
            assert disposition == "deferred_geometer_service", surface["id"]
            assert status == "deferred", surface["id"]
        if status == "planned":
            assert payload["review_state"] == "in_progress", surface["id"]
            assert not surface["rack_cases"], surface["id"]
        elif disposition == "required_bounded_slice":
            assert surface["rack_cases"], surface["id"]
        if disposition == "required_bounded_slice":
            promotion_fields = (
                "vector_evidence",
                "implementation_evidence",
                "semantic_resource_evidence",
            )
            for field in promotion_fields:
                assert field in surface, f"missing {surface['id']} field: {field}"
                if status == "planned":
                    assert not surface[field], f"premature {surface['id']} {field}"
                else:
                    assert surface[field], f"missing {surface['id']} {field}"
                for evidence in surface[field]:
                    assert (PACKAGE_ROOT / evidence).exists(), (
                        f"missing {surface['id']} {field}: {evidence}"
                    )
        if surface["id"] == "plotter_ir.phase5_exit" and status != "planned":
            assert surface["rack_cases"] == ["L3_022"]
            assert (
                rack_case_paths["L3_022"]
                == "tests/L3_rendering/test_L3_022_rust_phase5_exit.py"
            )
            assert (
                "tests/L3_rendering/test_L3_022_rust_phase5_exit.py"
                in surface["semantic_resource_evidence"]
            )
            assert PHASE5_FREEZE_RELATIVE_PATH in surface["evidence"]
        if surface["id"] == "cruncher.phase6_exit" and status != "planned":
            assert surface["rack_cases"] == ["L3_027"]
            assert (
                rack_case_paths["L3_027"]
                == "tests/L3_rendering/test_L3_027_rust_phase6_exit.py"
            )
            assert PHASE6_FREEZE_RELATIVE_PATH in surface["evidence"]
        for rack_case in surface["rack_cases"]:
            assert rack_case in rack_case_paths, (
                f"unknown {surface['id']} Rack case: {rack_case}"
            )
        for evidence in surface["evidence"]:
            assert (PACKAGE_ROOT / evidence).exists(), (
                f"missing {surface['id']} evidence: {evidence}"
            )
    if payload["review_state"] == "accepted":
        assert payload["milestone"] == "rust_phase6_native_cruncher_closed"
        assert all(
            surface["status"] == "closed"
            for surface in surfaces
            if surface["disposition"] == "required_bounded_slice"
        )
        phase6_surfaces = {
            surface["id"]: surface
            for surface in surfaces
            if surface["id"].startswith("cruncher.")
        }
        assert set(phase6_surfaces) == {
            "cruncher.native_operation_transport_package",
            "cruncher.native_svg",
            "cruncher.no_fallback_physical_provider",
            "cruncher.native_design_facts",
            "cruncher.native_full_cli",
            "cruncher.phase6_exit",
        }
        assert all(
            surface["status"] == "closed" for surface in phase6_surfaces.values()
        )


def test_typespec_and_generated_rust_contracts_are_clean() -> None:
    npm = shutil.which("npm")
    cargo = shutil.which("cargo")
    assert npm is not None, "npm is required for TypeSpec generation checks"
    assert cargo is not None, "cargo is required for Rust contract generation checks"
    assert typespec_executable(PACKAGE_ROOT).exists(), (
        "TypeSpec dependencies are missing; run `npm ci`"
    )
    before = _schema_hashes()
    _run([npm, "run", "check:typespec"])
    _run([npm, "run", "generate:contracts"])
    assert _schema_hashes() == before, "TypeSpec-generated JSON Schemas were stale"
    _run([npm, "run", "check:python-generation"])
    _run([npm, "run", "check:typescript-generation"])
    _run(
        [
            cargo,
            "run",
            "--package",
            "kicad-monkey-codegen",
            "--locked",
            "--",
            "--check",
        ]
    )


def test_typespec_launcher_name_is_cross_platform() -> None:
    assert typespec_executable(PACKAGE_ROOT, platform="nt").name == "tsp.cmd"
    assert typespec_executable(PACKAGE_ROOT, platform="posix").name == "tsp"


def test_ci_uses_the_repository_pinned_rust_toolchain() -> None:
    toolchain = tomllib.loads(
        (PACKAGE_ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    )["toolchain"]["channel"]
    assert toolchain == "1.98.0"
    action = "dtolnay/rust-toolchain@"
    pinned_action = f"{action}{toolchain}"
    governed = 0
    for workflow in sorted((PACKAGE_ROOT / ".github" / "workflows").glob("*.yml")):
        text = workflow.read_text(encoding="utf-8")
        uses = text.count(action)
        if uses:
            governed += uses
            assert text.count(pinned_action) == uses, workflow
    assert governed > 0


def test_ci_runs_the_phase5_typespec_gate_on_linux() -> None:
    workflow = (PACKAGE_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    assert "name: Run Linux TypeSpec Phase 5 gate" in workflow
    assert "if: runner.os == 'Linux'" in workflow
    assert (
        "test_L3_022_rust_phase5_exit.py::"
        "test_phase5_contract_freeze_and_codegen_are_current"
    ) in workflow


def test_phase6_rack_workflows_restore_reviewed_corpus_before_rack() -> None:
    workflows = PACKAGE_ROOT / ".github" / "workflows"
    required_fragments = (
        "scripts/kicad_corpus_archive.py metadata",
        "scripts/kicad_corpus_archive.py restore --check-zip",
        "scripts/package_kicad_corpus.py --check",
        "KM_CORPUS=$corpus",
    )
    for workflow_name in _PHASE6_RACK_WORKFLOWS:
        workflow = workflows / workflow_name
        text = workflow.read_text(encoding="utf-8")
        rack_offset = text.index("tests/rack.py")
        for fragment in required_fragments:
            assert fragment in text, f"{workflow_name} is missing {fragment}"
            assert text.index(fragment) < rack_offset, (
                f"{workflow_name} restores the reviewed corpus after Rack starts"
            )


def test_wn_dev_std_rust_hygiene_profile_passes() -> None:
    config = tomllib.loads(
        (PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    )["tool"]["wn_dev_std"]
    checks = check_rust_policy(PACKAGE_ROOT, config, "rust-app")
    failures = [f"{check.name}: {check.detail}" for check in checks if not check.passed]
    assert not failures, "\n".join(failures)


def test_rust_l0_quality_and_real_wasm_smoke() -> None:
    cargo = shutil.which("cargo")
    runner = shutil.which("wasm-bindgen-test-runner")
    assert cargo is not None, "cargo is required for Rust L0 signoff"
    assert runner is not None, (
        "install the lock-compatible WASM runner with "
        "`cargo install wasm-bindgen-cli --version 0.2.127 --locked`"
    )

    _run([cargo, "fmt", "--all", "--", "--check"])
    _run([cargo, "check", "--workspace", "--all-targets", "--locked"])
    _run(
        [
            cargo,
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]
    )
    _run(
        [
            cargo,
            "test",
            "--package",
            "kicad-monkey-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--locked",
        ]
    )


def test_wasm_operation_families_build_independently() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for WASM feature checks"
    for feature in ("sexpr", "footprint", "symbol"):
        _run(
            [
                cargo,
                "check",
                "--package",
                "kicad-monkey-wasm",
                "--target",
                "wasm32-unknown-unknown",
                "--no-default-features",
                "--features",
                feature,
                "--locked",
            ]
        )

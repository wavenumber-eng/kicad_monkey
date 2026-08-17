"""L3 corpus-manifest hygiene checks."""

from __future__ import annotations

from pathlib import Path

import pytest

from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    get_kicad_corpus_root,
    load_kicad_corpus_manifest,
    resolve_kicad_manifest_path,
)

_DEBRIS_FILE_NAMES = {".DS_Store", "Thumbs.db", "fp-info-cache"}
_DEBRIS_SUFFIXES = {".bak", ".kicad_prl", ".lck", ".log", ".tmp", ".zip"}
_DEBRIS_DIR_NAMES = {
    ".git",
    ".history",
    ".pytest_cache",
    "__pycache__",
    "_stage",
    "output",
    "review",
    "review_tmp",
}


def _is_debris(path: Path) -> bool:
    if path.is_dir():
        return path.name in _DEBRIS_DIR_NAMES or path.name.lower().endswith("-backups")
    name = path.name.lower()
    return (
        path.name in _DEBRIS_FILE_NAMES
        or path.suffix.lower() in _DEBRIS_SUFFIXES
        or name.startswith("~")
        and name.endswith((".kicad_pro.lck", ".kicad_prl.lck"))
    )


def _manifest():
    try:
        return load_kicad_corpus_manifest(required=True)
    except Exception as exc:
        pytest.skip(f"KiCad corpus manifest unavailable: {exc}")


def test_manifest_schema_and_required_real_world_cases():
    manifest = _manifest()
    assert manifest["schema"] == "kicad_monkey.corpus_manifest.v1"
    assert manifest["support_policy"]["strict_oracle_lane"] == "KiCad 9/10 S-expression"
    assert manifest["cases"]

    case_ids = {case["id"] for case in manifest["cases"]}
    assert {
        "real_world/charge_indicator",
        "real_world/4-ch-backplane",
        "real_world/taillight",
        "real_world/speedy_processing_module",
        "real_world/cern_wren_eda_04903",
        "real_world/eez_dcp405plus",
        "real_world/icepi_zero_v13",
        "real_world/jumperless_v5r7",
        "real_world/nrf9151_feather",
    }.issubset(case_ids)


def test_manifest_paths_resolve_for_active_cases():
    _manifest()
    root = get_kicad_corpus_root()
    assert root.is_dir()

    for case in _manifest()["cases"]:
        if case.get("status") != "active":
            continue
        for key in ("input_root", "input_file", "project_file", "top_schematic", "board_file"):
            value = case.get(key)
            if not value:
                continue
            path = resolve_kicad_manifest_path(case, key)
            assert path is not None and path.exists(), f"{case['id']} missing {key}: {value}"


def test_normalized_input_roots_are_clean():
    _manifest()
    for case in _manifest()["cases"]:
        if case.get("status") != "active" or case.get("layout") != "normalized":
            continue
        known_debris = case.get("hygiene", {}).get("known_debris") or []
        assert not known_debris, f"{case['id']} has generated/editor debris: {known_debris}"


def test_corpus_tree_has_no_editor_or_backup_debris():
    _manifest()
    root = get_kicad_corpus_root()
    offenders = [
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if _is_debris(path)
    ]
    assert not offenders, "generated/editor debris found in corpus:\n" + "\n".join(offenders[:50])


def test_real_world_domains_are_promoted():
    _manifest()
    for case_id in (
        "real_world/charge_indicator",
        "real_world/4-ch-backplane",
        "real_world/taillight",
        "real_world/speedy_processing_module",
    ):
        case = get_kicad_corpus_case(case_id)
        assert case is not None
        assert case["layout"] == "normalized"
        assert case["kicad_version_lane"] == "strict_9_10"
        assert {
            "netlist",
            "schematic_ir",
            "schematic_svg",
            "board_svg",
            "pcb_ir",
        }.issubset(set(case["domains"]))


def test_real_world_cases_are_normalized_and_clean():
    _manifest()
    for case in _manifest()["cases"]:
        if case.get("origin") != "real_world" or case.get("status") != "active":
            continue
        assert case["layout"] == "normalized"
        assert not case["hygiene"]["known_debris"], case["id"]


def test_public_real_world_active_cases_are_strict_and_provenanced():
    _manifest()
    public_case_ids = {
        "real_world/canbob",
        "real_world/cern_wren_eda_04903",
        "real_world/cm5_minima_rev2",
        "real_world/eez_dcp405plus",
        "real_world/icepi_sbc",
        "real_world/icepi_zero_v13",
        "real_world/jumperless_v5r7",
        "real_world/nrf9151_feather",
    }
    for case_id in public_case_ids:
        case = get_kicad_corpus_case(case_id)
        assert case is not None
        assert case["status"] == "active"
        assert case["kicad_version_lane"] == "strict_9_10"
        assert case["provenance"]["source_kind"] == "public_project_copy"
        assert case["provenance"]["license_usage"] == "public_validation_fixture"
        assert {
            "netlist",
            "netlist_project_corpus",
            "schematic_ir",
            "schematic_svg",
            "board_svg",
        }.issubset(set(case["domains"]))


def test_public_stress_candidates_are_promoted_after_netlist_parity():
    _manifest()
    for case_id in (
        "real_world/canbob",
        "real_world/cm5_minima_rev2",
        "real_world/icepi_sbc",
    ):
        case = get_kicad_corpus_case(case_id, required=False)
        assert case is not None
        assert case["status"] == "active"
        assert "known_netlist_drift" not in case.get("tags", [])
        assert not case["hygiene"]["known_debris"]


def test_mimxrt685_symbol_library_case_is_multi_unit():
    _manifest()
    case = get_kicad_corpus_case("internal_library/symbol_svg/MIMXRT685SFVKB")
    assert case is not None
    assert {"symbol_ir", "symbol_svg", "symbol_library"}.issubset(case["domains"])
    assert case["symbol_unit_count"] >= 8


def test_schematic_svg_manifest_has_synthetic_and_real_world_coverage():
    _manifest()
    active = [
        case for case in _manifest()["cases"]
        if case.get("status") == "active" and "schematic_svg" in case.get("domains", [])
    ]
    origins = {case["origin"] for case in active}
    assert {"synthetic", "real_world"}.issubset(origins)
    assert sum(1 for case in active if case["origin"] == "real_world") >= 12


def test_schematic_recorder_manifest_has_frozen_oracle_cases():
    _manifest()
    active = [
        case for case in _manifest()["cases"]
        if case.get("status") == "active"
        and "schematic_recorder_drift" in case.get("domains", [])
    ]
    case_ids = {case["id"] for case in active}
    assert {
        "reference_recorder/ADC_PWR.1",
        "reference_recorder/complex_hierarchy.1",
        "reference_recorder/led_component.1",
        "reference_recorder/sallen_key.1",
    }.issubset(case_ids)
    for case in active:
        recorder_file = resolve_kicad_manifest_path(case, "recorder_file")
        assert recorder_file is not None and recorder_file.exists()
        assert case["oracle_policy"]["schematic_recorder_drift"] == "kicad_recorder_dump"
        assert case["op_equivalence_strategy"] == "windowed_by_kind"
        assert case["op_equivalence_tolerance_nm"] <= 10000
        assert case.get("op_equivalence_compare_styles", True) is True
        assert case.get("max_op_equivalence_style_mismatches", 0) == 0
        assert case["op_equivalence_ignore_stroked_text_runs"] is True
        assert case["min_op_equivalence_matched_pairs"] > 0
        assert case["min_op_equivalence_match_ratio"] >= 0.6


def test_public_official_library_samples_cover_expected_categories():
    _manifest()
    public = [
        case for case in _manifest()["cases"]
        if case.get("origin") == "public_library" and case.get("status") == "active"
    ]
    symbol_categories = {
        case["public_library_category"]
        for case in public
        if "symbol_library" in case.get("domains", [])
    }
    footprint_categories = {
        case["public_library_category"]
        for case in public
        if "footprint_library" in case.get("domains", [])
    }
    assert {
        "connector",
        "fpga",
        "interface",
        "mcu",
        "memory",
        "passive",
        "power",
        "rf",
    }.issubset(symbol_categories)
    assert {
        "connector",
        "mechanical",
        "pad",
        "rf_castellated",
        "smd_ic",
        "smd_passive",
    }.issubset(footprint_categories)

"""Build the KiCad corpus manifest.

The manifest is a registry over the package authoring ``tests/corpus/kicad`` tree. It
does not move files; it labels the current layout so tests can stop relying on
ad hoc recursive discovery while the corpus is normalized case by case.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
PY_SRC = REPO_ROOT / "src" / "py"
if str(PY_SRC) not in sys.path:
    sys.path.insert(0, str(PY_SRC))


STRICT_KICAD_9_10_MIN_SCHEMATIC_VERSION = 20240716
KICAD_7_8_COMPAT_MIN_SCHEMATIC_VERSION = 20230121


def _rel(root: Path, path: Path | None) -> str | None:
    if path is None:
        return None
    return path.relative_to(root).as_posix()


def _schematic_file_format_version(path: Path) -> int | None:
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines()[:12]:
        stripped = line.strip()
        if stripped.startswith("(version ") and stripped.endswith(")"):
            value = stripped.removeprefix("(version ").removesuffix(")")
            return int(value) if value.isdigit() else None
    return None


def _version_lane(version: int | None) -> str:
    if version is None:
        return "unknown"
    if version >= STRICT_KICAD_9_10_MIN_SCHEMATIC_VERSION:
        return "strict_9_10"
    if version >= KICAD_7_8_COMPAT_MIN_SCHEMATIC_VERSION:
        return "compat_7_8_candidate"
    return "unsupported_legacy"


def _find_project_root(project_dir: Path) -> tuple[str, Path]:
    input_dir = project_dir / "input"
    if input_dir.is_dir():
        return "normalized", input_dir
    return "legacy_flat", project_dir


def _load_project_metadata(project_dir: Path) -> dict[str, Any]:
    metadata_path = project_dir / "case_metadata.json"
    if not metadata_path.is_file():
        return {}
    data = json.loads(metadata_path.read_text(encoding="utf-8-sig"))
    if not isinstance(data, dict):
        raise ValueError(f"Project metadata must be a JSON object: {metadata_path}")
    return data


def _metadata_string_list(
    metadata: dict[str, Any],
    key: str,
    *,
    label: str,
) -> list[str] | None:
    value = metadata.get(key)
    if value is None:
        return None
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(f"{key} metadata must be a string list: {label}")
    return value


def _find_top_schematic(project_file: Path) -> Path | None:
    candidate = project_file.with_suffix(".kicad_sch")
    if candidate.is_file():
        return candidate
    schs = sorted(project_file.parent.glob("*.kicad_sch"))
    return schs[0] if schs else None


def _known_debris(root: Path) -> list[str]:
    patterns = (
        "*.kicad_prl",
        "*.lck",
        "~*.lck",
        "_test.net",
        "_fresh.net",
        "_video_ours.net",
    )
    debris: set[str] = set()
    for pattern in patterns:
        for path in root.rglob(pattern):
            debris.add(path.relative_to(root).as_posix())
    for dirname in ("_stage", "__pycache__"):
        for path in root.rglob(dirname):
            if path.is_dir():
                debris.add(path.relative_to(root).as_posix() + "/")
    return sorted(debris)


def _project_domains(root: Path, top_sch: Path | None, board: Path | None) -> list[str]:
    domains = ["netlist", "netlist_project_corpus"]
    if top_sch is not None:
        domains.extend(["schematic_ir", "schematic_svg"])
    if board is not None:
        domains.extend(["board_svg", "pcb_ir"])
    if list(root.rglob("*.kicad_sym")):
        domains.append("symbol_library")
    if list(root.rglob("*.kicad_mod")) or list(root.rglob("*.pretty")):
        domains.append("footprint_library")
    return domains


def _project_case(kicad_root: Path, project_dir: Path) -> dict[str, Any] | None:
    layout, input_root = _find_project_root(project_dir)
    metadata = _load_project_metadata(project_dir)
    project_files = sorted(input_root.glob("*.kicad_pro"))
    if not project_files:
        return None
    preferred_project_file = metadata.get("preferred_project_file")
    if preferred_project_file:
        project_file = input_root / str(preferred_project_file)
        if not project_file.is_file():
            raise FileNotFoundError(
                f"preferred_project_file not found for {project_dir.name}: {project_file}"
            )
    else:
        project_file = project_files[0]
    top_sch = _find_top_schematic(project_file)
    board = project_file.with_suffix(".kicad_pcb")
    if not board.is_file():
        boards = sorted(input_root.glob("*.kicad_pcb"))
        board = boards[0] if boards else None

    version = _schematic_file_format_version(top_sch) if top_sch else None
    output_root = project_dir / "output"
    reference_output_root = project_dir / "reference_output"

    provenance = {
        "source_kind": "corpus_project_copy",
        "source_path": None,
        "license_usage": "test_fixture",
    }
    provenance.update(metadata.get("provenance") or {})

    domains = _metadata_string_list(
        metadata,
        "domains",
        label=project_dir.name,
    ) or _project_domains(input_root, top_sch, board)

    oracle_policy = {
        "netlist": "kicad_cli_live",
        "schematic_ir": "smoke",
        "schematic_svg": "smoke",
        "board_svg": "smoke",
    }
    oracle_policy.update(metadata.get("oracle_policy") or {})

    case: dict[str, Any] = {
        "id": f"real_world/{project_dir.name}",
        "name": project_dir.name,
        "origin": metadata.get("origin", "real_world"),
        "status": metadata.get("status", "active"),
        "layout": layout,
        "cad_family": "kicad",
        "format": "s_expression",
        "kicad_version_lane": _version_lane(version),
        "schematic_version": version,
        "input_root": _rel(kicad_root, input_root),
        "output_root": _rel(kicad_root, output_root),
        "reference_output_root": _rel(kicad_root, reference_output_root),
        "project_file": _rel(kicad_root, project_file),
        "top_schematic": _rel(kicad_root, top_sch),
        "input_file": _rel(kicad_root, top_sch),
        "board_file": _rel(kicad_root, board),
        "schematics": [
            _rel(kicad_root, path)
            for path in sorted(input_root.rglob("*.kicad_sch"))
        ],
        "symbol_libraries": [
            _rel(kicad_root, path)
            for path in sorted(input_root.rglob("*.kicad_sym"))
        ],
        "footprint_libraries": [
            _rel(kicad_root, path)
            for path in sorted(input_root.rglob("*.pretty"))
        ],
        "domains": domains,
        "oracle_policy": oracle_policy,
        "provenance": provenance,
        "hygiene": {
            "normalized": layout == "normalized",
            "known_debris": _known_debris(input_root),
        },
    }
    for key in ("tags", "notes", "promotion_reason"):
        if key in metadata:
            case[key] = metadata[key]
    return case


def _topic_case(
    kicad_root: Path,
    *,
    topic: str,
    case_id: str,
    input_file: Path,
    domains: list[str],
    origin: str,
    metadata: dict[str, Any] | None = None,
    extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
    topic_root = kicad_root / topic
    metadata = metadata or {}
    domains = _metadata_string_list(
        metadata,
        "domains",
        label=f"{topic}/{case_id}",
    ) or domains
    origin = str(metadata.get("origin", origin))

    oracle_policy = {"svg": "kicad_cli_or_smoke"}
    oracle_policy.update(metadata.get("oracle_policy") or {})

    provenance = {
        "source_kind": origin,
        "source_path": None,
        "license_usage": "test_fixture",
    }
    provenance.update(metadata.get("provenance") or {})

    case: dict[str, Any] = {
        "id": f"{origin}/{topic}/{case_id}",
        "name": case_id,
        "origin": origin,
        "status": metadata.get("status", "active"),
        "layout": "topic_bucket",
        "cad_family": "kicad",
        "format": "s_expression",
        "input_root": _rel(kicad_root, topic_root / "input"),
        "output_root": _rel(kicad_root, topic_root / "output"),
        "reference_output_root": _rel(kicad_root, topic_root / "reference_output"),
        "input_file": _rel(kicad_root, input_file),
        "domains": domains,
        "oracle_policy": oracle_policy,
        "provenance": provenance,
        "hygiene": {
            "normalized": False,
            "known_debris": _known_debris(topic_root / "input")
            if (topic_root / "input").exists()
            else [],
        },
    }
    if extra:
        case.update(extra)
    for key in (
        "coverage",
        "description",
        "feature_coverage",
        "notes",
        "promotion_reason",
        "tags",
        "test_intent",
    ):
        if key in metadata:
            case[key] = metadata[key]
    return case


def _symbol_unit_count(path: Path, symbol_name: str | None = None) -> int | None:
    try:
        from kicad_monkey import KiCadSymbolLib

        lib = KiCadSymbolLib.from_file(path)
        if symbol_name:
            symbol = lib.get_symbol(symbol_name)
            return symbol.unit_count if symbol is not None else None
        if not lib.symbols:
            return None
        return max(symbol.unit_count for symbol in lib.symbols)
    except Exception:
        return None


def _pcb_foundation_case(
    kicad_root: Path,
    *,
    case_id: str,
    input_file: Path,
    metadata: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Per-case manifest entry for ``pcb_foundation/<case>/{input,reference_output,output}``."""
    case_root = kicad_root / "pcb_foundation" / case_id
    metadata = metadata or {}
    domains = _metadata_string_list(
        metadata,
        "domains",
        label=f"pcb_foundation/{case_id}",
    ) or ["pcb_foundation", "pcb_ir", "board_svg"]
    origin = str(metadata.get("origin", "synthetic"))

    oracle_policy = {"svg": "kicad_cli_or_smoke"}
    oracle_policy.update(metadata.get("oracle_policy") or {})

    provenance = {
        "source_kind": origin,
        "source_path": None,
        "license_usage": "test_fixture",
    }
    provenance.update(metadata.get("provenance") or {})

    case: dict[str, Any] = {
        "id": f"{origin}/pcb_foundation/{case_id}",
        "name": case_id,
        "origin": origin,
        "status": metadata.get("status", "active"),
        "layout": "case_bucket",
        "cad_family": "kicad",
        "format": "s_expression",
        "input_root": _rel(kicad_root, case_root / "input"),
        "output_root": _rel(kicad_root, case_root / "output"),
        "reference_output_root": _rel(kicad_root, case_root / "reference_output"),
        "input_file": _rel(kicad_root, input_file),
        "domains": domains,
        "oracle_policy": oracle_policy,
        "provenance": provenance,
        "hygiene": {
            "normalized": False,
            "known_debris": _known_debris(case_root / "input")
            if (case_root / "input").exists()
            else [],
        },
    }
    for key in (
        "coverage",
        "description",
        "feature_coverage",
        "notes",
        "promotion_reason",
        "tags",
        "test_intent",
    ):
        if key in metadata:
            case[key] = metadata[key]
    return case


def _topic_cases(kicad_root: Path) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []

    pcb_foundation_root = kicad_root / "pcb_foundation"
    if pcb_foundation_root.is_dir():
        for board in sorted(pcb_foundation_root.glob("*/input/*.kicad_pcb")):
            case_id = board.parent.parent.name
            metadata = _load_project_metadata(board.parent)
            cases.append(
                _pcb_foundation_case(
                    kicad_root,
                    case_id=case_id,
                    input_file=board,
                    metadata=metadata,
                )
            )

    # Legacy ``board_svg/input/<case>/`` synthetic boards were folded into
    # the per-case ``pcb_foundation/<case>/`` layout on 2026-05-17. The
    # legacy enumerator above (``pcb_foundation_root``) now picks all of
    # them up; no separate board_svg loop is needed.

    for sch in sorted((kicad_root / "schematic_svg" / "input").glob("*.kicad_sch")):
        version = _schematic_file_format_version(sch)
        cases.append(
            _topic_case(
                kicad_root,
                topic="schematic_svg",
                case_id=sch.stem,
                input_file=sch,
                domains=["schematic_ir", "schematic_svg"],
                origin="synthetic",
                extra={
                    "schematic_version": version,
                    "kicad_version_lane": _version_lane(version),
                },
            )
        )

    for sym in sorted((kicad_root / "symbol_svg" / "input").glob("*.kicad_sym")):
        is_internal_library = sym.stem == "MIMXRT685SFVKB"
        origin = "internal_library" if is_internal_library else "synthetic"
        if is_internal_library:
            provenance = {
                "source_kind": "internal_library_copy",
                "source_path": (
                    "C:/eli/wn-hw/libz/wn-general/symbols/kicad/"
                    "MIMXRT685SFVKB.kicad_sym"
                ),
                "license_usage": "internal_validation_only",
            }
        else:
            provenance = {
                "source_kind": "synthetic",
                "source_path": None,
                "license_usage": "test_fixture",
            }
        cases.append(
            _topic_case(
                kicad_root,
                topic="symbol_svg",
                case_id=sym.stem,
                input_file=sym,
                domains=["symbol_ir", "symbol_svg", "symbol_library"],
                origin=origin,
                extra={
                    "symbol_unit_count": _symbol_unit_count(sym),
                    "provenance": provenance,
                },
            )
        )

    for fp in sorted((kicad_root / "footprint_svg" / "input").glob("*.kicad_mod")):
        cases.append(
            _topic_case(
                kicad_root,
                topic="footprint_svg",
                case_id=fp.stem,
                input_file=fp,
                domains=["footprint_ir", "footprint_svg", "footprint_library"],
                origin="internal_library",
            )
        )

    return cases


def _reference_recorder_cases(kicad_root: Path) -> list[dict[str, Any]]:
    """Manifest cases pairing frozen KiCad recorder dumps with source schematics."""
    recorder_root = kicad_root / "common" / "reference_recorder_dumps"
    if not recorder_root.is_dir():
        return []

    source_by_recorder = {
        "ADC_PWR.1": "common/reference_schematics/input/ADC_PWR.kicad_sch",
        "complex_hierarchy.1": "common/complex_hierarchy/input/complex_hierarchy.kicad_sch",
        "complex_hierarchy.2": "common/complex_hierarchy/input/ampli_ht.kicad_sch",
        "complex_hierarchy.3": "common/complex_hierarchy/input/ampli_ht.kicad_sch",
        "led_component.1": "common/reference_schematics/input/led_component.kicad_sch",
        "sallen_key.1": "common/reference_schematics/input/sallen_key.kicad_sch",
    }
    active_cases = {
        "ADC_PWR.1",
        "complex_hierarchy.1",
        "led_component.1",
        "sallen_key.1",
    }
    expected_canvas_drift = {
        "ADC_PWR.1": [0, 0],
        "complex_hierarchy.1": [0, 0],
        "led_component.1": [0, 0],
        "sallen_key.1": [0, 0],
    }
    # Active recorder cases gate actual op shape/coordinate/style matches after
    # recorder internal-unit normalization. Folded stroke-font render runs are
    # ignored here because these dumps also retain high-level Text ops.
    # Active recorder cases gate strict normalized op parity.  Text styling,
    # project drawing settings, hierarchy sheet plotting, and pin graphic
    # styles are all expected to match the recorder within the current 10um
    # coordinate tolerance.
    op_equivalence_thresholds = {
        "ADC_PWR.1": {
            "min_matched_pairs": 326,
            "min_match_ratio": 1.0,
            "max_monkey_short": 0,
            "max_monkey_long": 0,
            "first_divergence": None,
            "notes": (
                "Text-aware recorder baseline; strict style comparison reaches "
                "100% normalized op parity within the current 10um coordinate "
                "tolerance."
            ),
        },
        "complex_hierarchy.1": {
            "min_matched_pairs": 280,
            "min_match_ratio": 1.0,
            "max_monkey_short": 0,
            "max_monkey_long": 0,
            "first_divergence": None,
            "notes": (
                "Text-aware recorder baseline; strict style comparison has "
                "zero style drift and reaches 100% normalized op parity, "
                "including hierarchy sheet plotting and project drawing "
                "settings, within the current 10um coordinate tolerance."
            ),
        },
        "led_component.1": {
            "min_matched_pairs": 89,
            "min_match_ratio": 1.0,
            "max_monkey_short": 0,
            "max_monkey_long": 0,
            "first_divergence": None,
            "notes": (
                "Text-aware recorder baseline; strict style comparison reaches "
                "100% normalized op parity within the current 10um coordinate "
                "tolerance."
            ),
        },
        "sallen_key.1": {
            "min_matched_pairs": 201,
            "min_match_ratio": 1.0,
            "max_monkey_short": 0,
            "max_monkey_long": 0,
            "first_divergence": None,
            "notes": (
                "Text-aware recorder baseline; strict style comparison reaches "
                "100% normalized op parity within the current 10um coordinate "
                "tolerance."
            ),
        },
    }

    cases: list[dict[str, Any]] = []
    for recorder in sorted(recorder_root.glob("*.json")):
        case_name = recorder.stem
        source_rel = source_by_recorder.get(case_name)
        if not source_rel:
            continue
        source_file = kicad_root / source_rel
        status = "active" if case_name in active_cases else "reference_only"
        extra_notes = []
        if status != "active":
            extra_notes.append(
                "Recorder dump is retained as a frozen reference; activation "
                "is deferred until sheet-instance mapping is explicit."
            )
        elif case_name in op_equivalence_thresholds:
            extra_notes.append(op_equivalence_thresholds[case_name]["notes"])
        version = _schematic_file_format_version(source_file) if source_file.is_file() else None
        case = {
            "id": f"reference_recorder/{case_name}",
            "name": case_name,
            "origin": "reference_recorder",
            "status": status,
            "layout": "oracle_pair",
            "cad_family": "kicad",
            "format": "s_expression",
            "kicad_version_lane": _version_lane(version),
            "schematic_version": version,
            "input_root": "common/reference_schematics/input",
            "output_root": "common/reference_schematics/output",
            "reference_output_root": "common/reference_recorder_dumps",
            "input_file": source_rel,
            "recorder_file": _rel(kicad_root, recorder),
            "domains": ["schematic_ir", "schematic_recorder_drift"],
            "oracle_policy": {
                "schematic_ir": "kicad_recorder_dump",
                "schematic_recorder_drift": "kicad_recorder_dump",
            },
            "min_recorder_geometric_ops": 50 if status == "active" else 1,
            "min_coverage_ratio": 0.95,
            "expected_canvas_drift_nm": expected_canvas_drift.get(case_name),
            "expected_recorder_only_kinds": ["StrokedTextRun"],
            "expected_monkey_only_kinds": [],
            "required_recorder_kinds": ["PlotPoly", "Text"],
            "provenance": {
                "source_kind": "kicad_recorder_plotter_dump",
                "source_path": "common/reference_recorder_dumps",
                "license_usage": "test_fixture",
            },
            "hygiene": {
                "normalized": False,
                "known_debris": [],
            },
            "notes": " ".join(extra_notes),
        }
        if status == "active":
            thresholds = op_equivalence_thresholds[case_name]
            case.update(
                {
                    "op_equivalence_strategy": "windowed_by_kind",
                    "op_equivalence_tolerance_nm": 10000,
                    "op_equivalence_match_window": 0,
                    "op_equivalence_compare_styles": True,
                    "op_equivalence_fold_pen_to_runs": True,
                    "op_equivalence_ignore_stroked_text_runs": True,
                    "min_op_equivalence_matched_pairs": thresholds["min_matched_pairs"],
                    "min_op_equivalence_match_ratio": thresholds["min_match_ratio"],
                    "max_op_equivalence_monkey_short": thresholds["max_monkey_short"],
                    "max_op_equivalence_monkey_long": thresholds["max_monkey_long"],
                    "max_op_equivalence_style_mismatches": 0,
                }
            )
            if thresholds["first_divergence"]:
                case["expected_op_equivalence_first_divergence_kind"] = thresholds[
                    "first_divergence"
                ]
        cases.append(case)
    return cases


def _real_world_recorder_cases(kicad_root: Path) -> list[dict[str, Any]]:
    """Manifest cases pairing real-world project sheets with recorder dumps."""
    def active(
        project_id: str,
        input_file: str,
        *,
        canvas: list[int],
        matched: int,
        ratio: float,
        short: int,
        long: int,
        first: str,
        recorder_only: list[str] | None = None,
        monkey_only: list[str] | None = None,
        required: list[str] | None = None,
    ) -> dict[str, Any]:
        return {
            "project_id": project_id,
            "input_file": input_file,
            "status": "active",
            "expected_canvas_drift_nm": canvas,
            "expected_recorder_only_kinds": recorder_only or ["StrokedTextRun"],
            "expected_monkey_only_kinds": monkey_only or [],
            "required_recorder_kinds": required or ["PlotPoly", "Text"],
            "thresholds": {
                "min_matched_pairs": matched,
                "min_match_ratio": ratio,
                "max_monkey_short": short,
                "max_monkey_long": long,
                "max_style": 0,
                "first_divergence": first,
            },
            "notes": (
                "Active top-level real-world recorder oracle. Thresholds "
                "capture current normalized strict-style parity at 10um; "
                "remaining drift is tracked for follow-on IR vocabulary work."
            ),
        }

    def reference(
        project_id: str,
        input_file: str,
        *,
        canvas: list[int],
        recorder_only: list[str] | None = None,
        monkey_only: list[str] | None = None,
        required: list[str] | None = None,
        reason: str,
    ) -> dict[str, Any]:
        return {
            "project_id": project_id,
            "input_file": input_file,
            "status": "reference_only",
            "expected_canvas_drift_nm": canvas,
            "expected_recorder_only_kinds": recorder_only or ["StrokedTextRun"],
            "expected_monkey_only_kinds": monkey_only or [],
            "required_recorder_kinds": required or ["PlotPoly", "Text"],
            "notes": (
                "Recorder dump is retained as a frozen real-world reference; "
                f"activation is deferred until {reason}."
            ),
        }

    specs: dict[str, dict[str, Any]] = {
        "canbob.1": reference(
            "canbob",
            "projects/canbob/input/CANBOB (MAGE-CANBOB-003).kicad_sch",
            canvas=[-11000, 2200],
            recorder_only=["PlotImage", "StrokedTextRun"],
            reason="its top-level normalized match ratio clears the smoke "
            "coverage gate but is still below the active recorder floor",
        ),
        "cern_wren_eda_04903.1": reference(
            "cern_wren_eda_04903",
            "projects/cern_wren_eda_04903/input/EDA-04903-V1-0.kicad_sch",
            canvas=[-11000, 2200],
            recorder_only=["PlotImage", "StrokedTextRun", "ThickSegment"],
            reason="style and vocabulary drift are triaged",
        ),
        "cm5_minima_rev2.1": active(
            "cm5_minima_rev2",
            "projects/cm5_minima_rev2/input/CM5_MINIMA_2.kicad_sch",
            canvas=[0, 0],
            matched=584,
            ratio=1.0,
            short=0,
            long=0,
            first="",
            recorder_only=["StrokedTextRun", "ThickSegment"],
        ),
        "charge_indicator.1": active(
            "charge_indicator",
            (
                "projects/charge_indicator/input/"
                "11-10043__charge_indicator__C.kicad_sch"
            ),
            canvas=[0, 0],
            matched=461,
            ratio=1.0,
            short=0,
            long=0,
            first="",
            recorder_only=["PlotImage", "StrokedTextRun"],
        ),
        "taillight.1": reference(
            "taillight",
            (
                "projects/taillight/input/"
                "11-10045__taillight__C.kicad_sch"
            ),
            canvas=[0, 0],
            reason="its top-level normalized match ratio is below the active "
            "recorder floor",
        ),
        "eez_dcp405plus.1": active(
            "eez_dcp405plus",
            "projects/eez_dcp405plus/input/EEZ DIB DCP405plus.kicad_sch",
            canvas=[0, 0],
            matched=291,
            ratio=1.0,
            short=0,
            long=0,
            first="",
        ),
        "icepi_sbc.1": reference(
            "icepi_sbc",
            "projects/icepi_sbc/input/cm0.kicad_sch",
            canvas=[2200, 7200],
            recorder_only=["PlotImage", "StrokedTextRun"],
            reason="its top-level normalized match ratio is just below the "
            "active recorder floor",
        ),
        "icepi_zero_v13.1": active(
            "icepi_zero_v13",
            "projects/icepi_zero_v13/input/icepi-zero.kicad_sch",
            canvas=[0, 0],
            matched=775,
            ratio=1.0,
            short=0,
            long=0,
            first="",
            recorder_only=["PlotImage", "StrokedTextRun", "ThickSegment"],
        ),
        "jumperless_v5r7.1": active(
            "jumperless_v5r7",
            "projects/jumperless_v5r7/input/JumperlessV5r7.kicad_sch",
            canvas=[0, 0],
            matched=8096,
            ratio=1.0,
            short=0,
            long=0,
            first="",
            recorder_only=["StrokedTextRun", "ThickSegment"],
        ),
        "nrf9151_feather.1": reference(
            "nrf9151_feather",
            "projects/nrf9151_feather/input/nRF9151_Feather.kicad_sch",
            canvas=[2200, 7200],
            recorder_only=["ArcThreePoint", "StrokedTextRun", "ThickSegment"],
            reason="large symbol arc/vocabulary drift is triaged",
        ),
        "speedy_processing_module.1": reference(
            "speedy_processing_module",
            (
                "projects/speedy_processing_module/input/"
                "11-10084__speedy_processing_module__B.kicad_sch"
            ),
            canvas=[0, 0],
            recorder_only=["PlotImage", "StrokedTextRun"],
            monkey_only=["PlotPoly"],
            required=["Rect", "Text"],
            reason="the embedded custom drawing sheet/image drift has "
            "one-to-one declarative partners",
        ),
    }

    cases: list[dict[str, Any]] = []
    for case_name, spec in sorted(specs.items()):
        project_id = str(spec["project_id"])
        recorder = (
            kicad_root
            / "projects"
            / project_id
            / "reference_output"
            / "recorder_dumps"
            / f"{case_name}.json"
        )
        if not recorder.is_file():
            continue
        source_file = kicad_root / str(spec["input_file"])
        if not source_file.is_file():
            continue
        version = _schematic_file_format_version(source_file)
        status = str(spec.get("status", "reference_only"))
        project_root = kicad_root / "projects" / project_id
        input_root = project_root / "input"
        output_root = project_root / "output"
        reference_root = project_root / "reference_output" / "recorder_dumps"
        case: dict[str, Any] = {
            "id": f"real_world_recorder/{case_name}",
            "name": case_name,
            "origin": "real_world_recorder",
            "status": status,
            "layout": "oracle_pair",
            "cad_family": "kicad",
            "format": "s_expression",
            "kicad_version_lane": _version_lane(version),
            "schematic_version": version,
            "input_root": _rel(kicad_root, input_root),
            "output_root": _rel(kicad_root, output_root),
            "reference_output_root": _rel(kicad_root, reference_root),
            "input_file": _rel(kicad_root, source_file),
            "recorder_file": _rel(kicad_root, recorder),
            "domains": ["schematic_ir", "schematic_recorder_drift"],
            "oracle_policy": {
                "schematic_ir": "kicad_recorder_dump",
                "schematic_recorder_drift": "kicad_recorder_dump",
            },
            "min_recorder_geometric_ops": 50 if status == "active" else 1,
            "min_coverage_ratio": 0.95,
            "expected_canvas_drift_nm": spec.get("expected_canvas_drift_nm"),
            "expected_recorder_only_kinds": spec.get(
                "expected_recorder_only_kinds", []
            ),
            "expected_monkey_only_kinds": spec.get("expected_monkey_only_kinds", []),
            "required_recorder_kinds": spec.get(
                "required_recorder_kinds", ["PlotPoly", "Text"]
            ),
            "provenance": {
                "source_kind": "kicad_recorder_plotter_dump",
                "source_path": _rel(kicad_root, reference_root),
                "license_usage": "internal_validation_only",
            },
            "hygiene": {
                "normalized": True,
                "known_debris": [],
            },
            "notes": spec.get("notes", ""),
        }
        thresholds = spec.get("thresholds") if status == "active" else None
        if thresholds:
            case.update(
                {
                    "op_equivalence_strategy": "windowed_by_kind",
                    "op_equivalence_tolerance_nm": 10000,
                    "op_equivalence_match_window": 0,
                    "op_equivalence_compare_styles": True,
                    "op_equivalence_fold_pen_to_runs": True,
                    "op_equivalence_ignore_stroked_text_runs": True,
                    "min_op_equivalence_matched_pairs": thresholds[
                        "min_matched_pairs"
                    ],
                    "min_op_equivalence_match_ratio": thresholds["min_match_ratio"],
                    "max_op_equivalence_monkey_short": thresholds[
                        "max_monkey_short"
                    ],
                    "max_op_equivalence_monkey_long": thresholds[
                        "max_monkey_long"
                    ],
                    "max_op_equivalence_style_mismatches": thresholds.get(
                        "max_style", 0
                    ),
                }
            )
            if thresholds.get("first_divergence"):
                case["expected_op_equivalence_first_divergence_kind"] = thresholds[
                    "first_divergence"
                ]
        cases.append(case)
    return cases


def _public_library_symbol_cases(kicad_root: Path) -> list[dict[str, Any]]:
    root = kicad_root / "public_libraries" / "kicad_official_symbols"
    input_root = root / "input"
    if not input_root.is_dir():
        return []

    category_by_library = {
        "Connector_Generic": "connector",
        "Device": "passive",
        "FPGA_Lattice": "fpga",
        "Interface_USB": "interface",
        "MCU_NXP_LPC": "mcu",
        "Memory_Flash": "memory",
        "RF_Module": "rf",
        "Regulator_Linear": "power",
    }
    cases: list[dict[str, Any]] = []
    for sym in sorted(input_root.glob("*.kicad_sym")):
        stem = sym.stem
        library_name, _, symbol_name = stem.partition("__")
        if not symbol_name:
            symbol_name = stem
        cases.append(
            {
                "id": f"public_library/kicad_official_symbols/{stem}",
                "name": symbol_name,
                "origin": "public_library",
                "status": "active",
                "layout": "public_library_sample",
                "cad_family": "kicad",
                "format": "s_expression",
                "input_root": _rel(kicad_root, input_root),
                "output_root": _rel(kicad_root, root / "output"),
                "reference_output_root": _rel(kicad_root, root / "reference_output"),
                "input_file": _rel(kicad_root, sym),
                "domains": ["symbol_ir", "symbol_svg", "symbol_library"],
                "symbol_name": symbol_name,
                "symbol_unit_count": _symbol_unit_count(sym, symbol_name),
                "public_library_category": category_by_library.get(
                    library_name, "other"
                ),
                "oracle_policy": {
                    "symbol_ir": "semantic_contract",
                    "symbol_svg": "semantic_contract",
                },
                "provenance": {
                    "source_kind": "official_kicad_library_sample",
                    "source_path": "https://gitlab.com/kicad/libraries/kicad-symbols",
                    "license_usage": "public_validation_fixture",
                },
                "hygiene": {
                    "normalized": False,
                    "known_debris": _known_debris(input_root),
                },
            }
        )
    return cases


def _public_library_footprint_cases(kicad_root: Path) -> list[dict[str, Any]]:
    root = kicad_root / "public_libraries" / "kicad_official_footprints"
    input_root = root / "input"
    if not input_root.is_dir():
        return []

    category_by_library = {
        "Connector_PinHeader_2.54mm": "connector",
        "Connector_USB": "connector",
        "MountingHole": "mechanical",
        "Package_QFP": "smd_ic",
        "Package_SO": "smd_ic",
        "RF_Module": "rf_castellated",
        "Resistor_SMD": "smd_passive",
        "TestPoint": "pad",
    }
    cases: list[dict[str, Any]] = []
    for fp in sorted(input_root.glob("*.kicad_mod")):
        stem = fp.stem
        library_name, _, footprint_name = stem.partition("__")
        if not footprint_name:
            footprint_name = stem
        cases.append(
            {
                "id": f"public_library/kicad_official_footprints/{stem}",
                "name": footprint_name,
                "origin": "public_library",
                "status": "active",
                "layout": "public_library_sample",
                "cad_family": "kicad",
                "format": "s_expression",
                "input_root": _rel(kicad_root, input_root),
                "output_root": _rel(kicad_root, root / "output"),
                "reference_output_root": _rel(kicad_root, root / "reference_output"),
                "input_file": _rel(kicad_root, fp),
                "domains": ["footprint_ir", "footprint_svg", "footprint_library"],
                "footprint_name": footprint_name,
                "public_library_category": category_by_library.get(
                    library_name, "other"
                ),
                "oracle_policy": {
                    "footprint_ir": "semantic_contract",
                    "footprint_svg": "semantic_contract",
                },
                "provenance": {
                    "source_kind": "official_kicad_library_sample",
                    "source_path": "https://gitlab.com/kicad/libraries/kicad-footprints",
                    "license_usage": "public_validation_fixture",
                },
                "hygiene": {
                    "normalized": False,
                    "known_debris": _known_debris(input_root),
                },
            }
        )
    return cases


def _public_library_cases(kicad_root: Path) -> list[dict[str, Any]]:
    return (
        _public_library_symbol_cases(kicad_root)
        + _public_library_footprint_cases(kicad_root)
    )


def _skip_project_corpus_path(path: Path) -> bool:
    return any(
        part in {
            "_stage",
            "output",
            "reference_output",
            "project_corpus_reference_output",
        }
        for part in path.parts
    )


def _broad_project_netlist_cases(kicad_root: Path) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for bucket in ("common", "upstream_qa", "netlist"):
        root = kicad_root / bucket
        if not root.exists():
            continue
        for project_file in sorted(root.rglob("*.kicad_pro")):
            if _skip_project_corpus_path(project_file):
                continue
            top_sch = _find_top_schematic(project_file)
            if top_sch is None:
                continue
            version = _schematic_file_format_version(top_sch)
            lane = _version_lane(version)
            if lane != "strict_9_10":
                continue
            rel_stem = project_file.relative_to(kicad_root).with_suffix("").as_posix()
            board = project_file.with_suffix(".kicad_pcb")
            cases.append(
                {
                    "id": f"project_corpus/{rel_stem}",
                    "name": project_file.stem,
                    "origin": "project_corpus",
                    "status": "active",
                    "layout": "project_tree",
                    "cad_family": "kicad",
                    "format": "s_expression",
                    "kicad_version_lane": lane,
                    "schematic_version": version,
                    "input_root": _rel(kicad_root, project_file.parent),
                    "output_root": "netlist/project_corpus_output",
                    "reference_output_root": "netlist/project_corpus_reference_output",
                    "project_file": _rel(kicad_root, project_file),
                    "top_schematic": _rel(kicad_root, top_sch),
                    "board_file": _rel(kicad_root, board if board.is_file() else None),
                    "schematics": [
                        _rel(kicad_root, path)
                        for path in sorted(project_file.parent.rglob("*.kicad_sch"))
                        if not _skip_project_corpus_path(path)
                    ],
                    "domains": ["netlist", "netlist_project_corpus"],
                    "oracle_policy": {"netlist": "kicad_cli_live"},
                    "provenance": {
                        "source_kind": f"{bucket}_project_corpus",
                        "source_path": None,
                        "license_usage": "test_fixture",
                    },
                    "hygiene": {
                        "normalized": False,
                        "known_debris": _known_debris(project_file.parent),
                    },
                }
            )
    return cases


def _upstream_netlist_cases(kicad_root: Path) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    root = kicad_root / "netlist" / "upstream_qa"
    if not root.is_dir():
        return cases
    for case_dir in sorted(path for path in root.iterdir() if path.is_dir()):
        sch = case_dir / f"{case_dir.name}.kicad_sch"
        if not sch.is_file():
            schs = sorted(case_dir.glob("*.kicad_sch"))
            sch = schs[0] if schs else None
        version = _schematic_file_format_version(sch) if sch else None
        lane = _version_lane(version)
        cases.append(
            {
                "id": f"upstream_qa/netlist/{case_dir.name}",
                "name": case_dir.name,
                "origin": "upstream_qa",
                "status": "active" if lane == "strict_9_10" else "reference_only",
                "layout": "case_dir",
                "cad_family": "kicad",
                "format": "s_expression",
                "kicad_version_lane": lane,
                "schematic_version": version,
                "input_root": _rel(kicad_root, case_dir),
                "output_root": _rel(kicad_root, case_dir),
                "reference_output_root": _rel(kicad_root, case_dir),
                "top_schematic": _rel(kicad_root, sch),
                "domains": ["netlist"],
                "oracle_policy": {"netlist": "kicad_cli_live"},
                "provenance": {
                    "source_kind": "kicad_source_qa_mirror",
                    "source_path": "qa/data/eeschema/netlists",
                    "license_usage": "upstream_test_fixture",
                },
                "hygiene": {
                    "normalized": False,
                    "known_debris": _known_debris(case_dir),
                },
            }
        )
    return cases


def build_manifest(kicad_root: Path) -> dict[str, Any]:
    projects_root = kicad_root / "projects"
    cases: list[dict[str, Any]] = []
    if projects_root.is_dir():
        for project_dir in sorted(path for path in projects_root.iterdir() if path.is_dir()):
            case = _project_case(kicad_root, project_dir)
            if case is not None:
                cases.append(case)

    cases.extend(_broad_project_netlist_cases(kicad_root))
    cases.extend(_upstream_netlist_cases(kicad_root))
    cases.extend(_topic_cases(kicad_root))
    cases.extend(_public_library_cases(kicad_root))
    cases.extend(_reference_recorder_cases(kicad_root))
    cases.extend(_real_world_recorder_cases(kicad_root))
    cases.sort(key=lambda item: item["id"])

    return {
        "schema": "kicad_monkey.corpus_manifest.v1",
        "generated_utc": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "root": ".",
        "support_policy": {
            "strict_oracle_lane": "KiCad 9/10 S-expression",
            "compatibility_lanes": ["KiCad 7/8 S-expression candidate"],
            "unsupported": ["KiCad 5 legacy .sch"],
            "strict_min_schematic_version": STRICT_KICAD_9_10_MIN_SCHEMATIC_VERSION,
            "compat_min_schematic_version": KICAD_7_8_COMPAT_MIN_SCHEMATIC_VERSION,
        },
        "counts": {
            "cases": len(cases),
            "active": sum(1 for case in cases if case.get("status") == "active"),
            "real_world": sum(1 for case in cases if case.get("origin") == "real_world"),
            "synthetic": sum(1 for case in cases if case.get("origin") == "synthetic"),
            "upstream_qa": sum(1 for case in cases if case.get("origin") == "upstream_qa"),
        },
        "cases": cases,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    default_corpus = REPO_ROOT / "tests" / "corpus"
    parser.add_argument("--corpus-root", type=Path, default=default_corpus)
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args()

    kicad_root = args.corpus_root / "kicad"
    if not kicad_root.is_dir():
        raise FileNotFoundError(f"KiCad corpus root not found: {kicad_root}")
    output = args.output or (kicad_root / "manifest.json")
    manifest = build_manifest(kicad_root)
    output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {output} ({manifest['counts']['cases']} cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

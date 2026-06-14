"""Project health and asset-reference diagnostics command."""

from __future__ import annotations

import argparse
import json
import logging
import time
from collections import Counter
from pathlib import Path

from kicad_cruncher.kicad_cruncher_common import find_kicad_project_in_cwd, resolve_output_dir

log = logging.getLogger(__name__)


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _resolve_input_project(file_arg: str | None) -> Path | None:
    if file_arg:
        path = Path(file_arg)
        if not path.exists():
            log.error("Input project does not exist: %s", path)
            return None
        if path.suffix != ".kicad_pro":
            log.error("project-health requires a .kicad_pro input: %s", path)
            return None
        return path

    project = find_kicad_project_in_cwd()
    if project is None:
        log.error("No input provided and current directory does not contain exactly one .kicad_pro")
        return None
    return project


def _dict_list(value: object) -> list[dict[str, object]]:
    if not isinstance(value, list | tuple):
        return []
    out: list[dict[str, object]] = []
    for item in value:
        if isinstance(item, dict):
            out.append({str(key): item[key] for key in item})
    return out


def _string_list(value: object) -> list[str]:
    if not isinstance(value, list | tuple):
        return []
    return [str(item) for item in value]


def _sequence_len(value: object) -> int:
    if isinstance(value, list | tuple):
        return len(value)
    return 0


def _model_reference_issue(ref: dict[str, object]) -> str | None:
    kind = str(ref.get("reference_kind", ""))
    if kind == "embedded":
        if not bool(ref.get("has_embedded_payload")):
            return "missing_embedded_payload"
        return None
    if not bool(ref.get("exists")):
        return "missing_or_unresolved_model"
    return None


def _project_health_payload(project_path: Path, asset_scan: dict[str, object]) -> dict[str, object]:
    model_refs = _dict_list(asset_scan.get("model_references", []))
    diagnostics = _string_list(asset_scan.get("diagnostics", ()))
    kind_counts = Counter(str(ref.get("reference_kind", "unknown")) for ref in model_refs)
    issue_refs: list[dict[str, object]] = []
    issue_counts: Counter[str] = Counter()
    for ref in model_refs:
        issue = _model_reference_issue(ref)
        if issue is None:
            continue
        issue_counts[issue] += 1
        issue_ref = dict(ref)
        issue_ref["issue"] = issue
        issue_refs.append(issue_ref)

    issue_counts["diagnostic"] = len(diagnostics)
    issue_count = len(issue_refs) + len(diagnostics)
    return {
        "schema": "kicad_cruncher.project_health.v0",
        "project": str(project_path),
        "ok": issue_count == 0,
        "summary": {
            "schematics": _sequence_len(asset_scan.get("schematics", ())),
            "pcbs": _sequence_len(asset_scan.get("pcbs", ())),
            "symbol_libraries": _sequence_len(asset_scan.get("symbol_libraries", ())),
            "pretty_libraries": _sequence_len(asset_scan.get("pretty_libraries", ())),
            "footprint_files": _sequence_len(asset_scan.get("footprint_files", ())),
            "model_references": len(model_refs),
            "model_reference_kinds": dict(sorted(kind_counts.items())),
            "issues": issue_count,
            "issue_kinds": dict(sorted(issue_counts.items())),
        },
        "issues": {
            "model_references": issue_refs,
            "diagnostics": diagnostics,
        },
        "assets": asset_scan,
    }


def _readme_text(report: dict[str, object]) -> str:
    summary = report["summary"]
    assert isinstance(summary, dict)
    return (
        "# KiCad Project Health\n\n"
        f"Source project: `{report['project']}`\n\n"
        f"Status: `{'ok' if report['ok'] else 'issues'}`\n\n"
        "Summary:\n\n"
        f"- Schematics: `{summary['schematics']}`\n"
        f"- PCBs: `{summary['pcbs']}`\n"
        f"- Symbol libraries: `{summary['symbol_libraries']}`\n"
        f"- Pretty libraries: `{summary['pretty_libraries']}`\n"
        f"- Footprint files: `{summary['footprint_files']}`\n"
        f"- Model references: `{summary['model_references']}`\n"
        f"- Issues: `{summary['issues']}`\n\n"
        "Generated artifacts:\n\n"
        "- Report: `project_health.json`\n\n"
        "This command is non-destructive. It scans project assets and model "
        "references without editing schematic, PCB, or library files.\n"
    )


def cmd_project_health(args: argparse.Namespace) -> int:
    """Run non-destructive project asset health checks."""
    from kicad_monkey.kicad_library_extraction import scan_project_assets

    project_path = _resolve_input_project(str(args.file) if args.file else None)
    if project_path is None:
        return 1

    output_dir = resolve_output_dir(args.output, "project-health")
    try:
        started = time.perf_counter()
        log.info("Project health: scanning %s", project_path)
        asset_scan = scan_project_assets(project_path).to_dict()
        report = _project_health_payload(project_path, asset_scan)
        report_path = output_dir / "project_health.json"
        _write_json(report_path, report)
        (output_dir / "README.md").write_text(_readme_text(report), encoding="utf-8")
    except Exception as exc:
        log.error("Project health failed: %s", exc)
        return 1

    summary = report["summary"]
    assert isinstance(summary, dict)
    log.info(
        "Project health: %d model refs, %d issues -> %s in %.2fs",
        summary["model_references"],
        summary["issues"],
        report_path,
        time.perf_counter() - started,
    )
    if bool(args.fail_on_issues) and not bool(report["ok"]):
        return 1
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the project-health command parser."""
    parser = subparsers.add_parser(
        "project-health",
        aliases=["project-check", "asset-check"],
        help="scan KiCad project assets and model references",
        description=(
            "Scan a KiCad project for local assets, embedded model references, "
            "external STEP/STP references, and missing model payloads."
        ),
    )
    parser.add_argument(
        "file",
        nargs="?",
        help="KiCad .kicad_pro project; optional when one .kicad_pro is in CWD",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="output directory (default: ./output/project-health)",
    )
    parser.add_argument(
        "--fail-on-issues",
        action="store_true",
        help="return a nonzero exit code when diagnostics or missing models are found",
    )
    parser.set_defaults(handler=cmd_project_health)
    return parser

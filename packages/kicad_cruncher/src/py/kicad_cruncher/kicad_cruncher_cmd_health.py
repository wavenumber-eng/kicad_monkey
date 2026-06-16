"""Project asset health and model-reference diagnostics command."""

from __future__ import annotations

import argparse
import json
import logging
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, TimeoutError
from pathlib import Path
from typing import cast

from kicad_cruncher.kicad_cruncher_common import find_kicad_project_in_cwd, resolve_output_dir

log = logging.getLogger(__name__)

_SCAN_PROGRESS_INTERVAL_S = 3.0


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _scan_assets_with_progress(project_path: Path) -> dict[str, object]:
    from kicad_monkey.kicad_library_extraction import scan_project_assets

    started = time.perf_counter()
    log.info("Project health: scanning project assets and 3D model references")
    last_progress = ["starting scan"]

    def progress(message: str) -> None:
        last_progress[0] = message
        log.info("Project health: %s", message)

    with ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(scan_project_assets, project_path, progress=progress)
        while True:
            try:
                return cast(
                    dict[str, object],
                    future.result(timeout=_SCAN_PROGRESS_INTERVAL_S).to_dict(),
                )
            except TimeoutError:
                elapsed_s = time.perf_counter() - started
                log.info(
                    "Project health: still scanning assets "
                    "(%.0fs elapsed; current: %s)",
                    elapsed_s,
                    last_progress[0],
                )


def _resolve_input_project(file_arg: str | None) -> Path | None:
    if file_arg:
        path = Path(file_arg)
        if not path.exists():
            log.error("Input project does not exist: %s", path)
            return None
        if path.suffix != ".kicad_pro":
            log.error("health requires a .kicad_pro input: %s", path)
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


def _object_int(value: object) -> int:
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError:
            return 0
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


def _health_payload(project_path: Path, asset_scan: dict[str, object]) -> dict[str, object]:
    model_refs = _dict_list(asset_scan.get("model_references", []))
    footprints_without_models = _dict_list(asset_scan.get("footprints_without_models", []))
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

    if footprints_without_models:
        issue_counts["footprint_without_model"] = len(footprints_without_models)
    if diagnostics:
        issue_counts["diagnostic"] = len(diagnostics)
    issue_count = len(issue_refs) + len(footprints_without_models) + len(diagnostics)
    footprint_instances_without_models = sum(
        _object_int(ref.get("instance_count", 0)) for ref in footprints_without_models
    )
    return {
        "schema": "kicad_cruncher.project_health.a0",
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
            "footprints_without_models": len(footprints_without_models),
            "footprint_instances_without_models": footprint_instances_without_models,
            "issues": issue_count,
            "issue_kinds": dict(sorted(issue_counts.items())),
        },
        "issues": {
            "model_references": issue_refs,
            "footprints_without_models": footprints_without_models,
            "diagnostics": diagnostics,
        },
        "assets": asset_scan,
    }


def _issue_kind_lines(summary: dict[object, object]) -> list[str]:
    issue_kinds = summary.get("issue_kinds", {})
    if not isinstance(issue_kinds, dict) or not issue_kinds:
        return []
    return [
        f"- {kind}: `{count}`"
        for kind, count in sorted(issue_kinds.items(), key=lambda item: str(item[0]))
    ]


def _model_issue_example(ref: dict[str, object]) -> str:
    issue = str(ref.get("issue", "model_reference"))
    model_path = str(ref.get("model_path", "") or ref.get("path", "") or "<unknown model>")
    source = str(ref.get("source", "") or ref.get("source_path", "") or "")
    if source:
        return f"- {issue}: `{model_path}` from `{source}`"
    return f"- {issue}: `{model_path}`"


def _model_issue_example_lines(model_refs: object, *, limit: int) -> list[str]:
    if not isinstance(model_refs, list):
        return []

    lines_by_kind: dict[str, list[str]] = {}
    for ref in model_refs:
        if not isinstance(ref, dict):
            continue
        issue = str(ref.get("issue", "model_reference"))
        lines = lines_by_kind.setdefault(issue, [])
        example = _model_issue_example(ref)
        if example not in lines and len(lines) < 2:
            lines.append(example)

    lines: list[str] = []
    for issue in sorted(lines_by_kind):
        lines.extend(lines_by_kind[issue])
        if len(lines) >= limit:
            return lines[:limit]
    return lines


def _diagnostic_example_lines(diagnostics: object, *, limit: int) -> list[str]:
    if not isinstance(diagnostics, list) or limit <= 0:
        return []
    return [f"- diagnostic: `{diagnostic}`" for diagnostic in diagnostics[:limit]]


def _footprint_without_model_example(ref: dict[str, object]) -> str:
    footprint = str(ref.get("footprint", "") or "<unknown footprint>")
    designators = _string_list(ref.get("designators", ()))
    shown = ", ".join(designators)
    return f"- footprint_without_model: `{footprint}` refs `{shown}`"


def _footprint_without_model_example_lines(footprints: object, *, limit: int) -> list[str]:
    if not isinstance(footprints, list) or limit <= 0:
        return []
    lines: list[str] = []
    for ref in footprints[:limit]:
        if isinstance(ref, dict):
            lines.append(_footprint_without_model_example(ref))
    return lines


def _issue_example_lines(report: dict[str, object], *, limit: int = 12) -> list[str]:
    issues = report.get("issues", {})
    if not isinstance(issues, dict):
        return []

    lines = _model_issue_example_lines(issues.get("model_references", []), limit=limit)
    remaining = limit - len(lines)
    lines.extend(
        _footprint_without_model_example_lines(
            issues.get("footprints_without_models", []),
            limit=remaining,
        )
    )
    remaining = limit - len(lines)
    lines.extend(_diagnostic_example_lines(issues.get("diagnostics", []), limit=remaining))
    return lines


def _terminal_issue_kind_lines(summary: dict[object, object]) -> list[str]:
    issue_kinds = summary.get("issue_kinds", {})
    if not isinstance(issue_kinds, dict) or not issue_kinds:
        return []
    return [
        f"  - {kind}: {count}"
        for kind, count in sorted(issue_kinds.items(), key=lambda item: str(item[0]))
    ]


def _terminal_example_lines(report: dict[str, object], *, limit: int = 12) -> list[str]:
    return [line.replace("`", "") for line in _issue_example_lines(report, limit=limit)]


def _terminal_summary(
    report: dict[str, object],
    *,
    report_path: Path,
    readme_path: Path,
    elapsed_s: float,
) -> str:
    summary = report["summary"]
    assert isinstance(summary, dict)
    status = "ok" if bool(report["ok"]) else "issues"
    lines = [
        f"KiCad project health: {status}",
        f"  Project: {report['project']}",
        f"  Schematics: {summary['schematics']}",
        f"  PCBs: {summary['pcbs']}",
        f"  Footprint files: {summary['footprint_files']}",
        f"  Model references: {summary['model_references']}",
        f"  Footprints without models: {summary['footprints_without_models']}",
        f"  Footprint instances without models: "
        f"{summary['footprint_instances_without_models']}",
        f"  Issues: {summary['issues']}",
    ]
    issue_kind_lines = _terminal_issue_kind_lines(summary)
    if issue_kind_lines:
        lines.append("  Issue kinds:")
        lines.extend(issue_kind_lines)
    example_lines = _terminal_example_lines(report)
    if example_lines:
        lines.append("  Examples:")
        lines.extend(f"    {line[2:] if line.startswith('- ') else line}" for line in example_lines)
    lines.extend(
        [
            f"  JSON: {report_path}",
            f"  README: {readme_path}",
            f"  Elapsed: {elapsed_s:.2f}s",
        ]
    )
    return "\n".join(lines)


def _readme_text(report: dict[str, object]) -> str:
    summary = report["summary"]
    assert isinstance(summary, dict)
    issue_kind_lines = _issue_kind_lines(summary)
    issue_example_lines = _issue_example_lines(report)
    issue_kinds = "\n".join(issue_kind_lines)
    issue_examples = "\n".join(issue_example_lines)
    details = ""
    if issue_kinds:
        details += "\nIssue kinds:\n\n" + issue_kinds + "\n"
    if issue_examples:
        details += "\nExample issues:\n\n" + issue_examples + "\n"

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
        f"- Footprints without models: `{summary['footprints_without_models']}`\n"
        f"- Footprint instances without models: "
        f"`{summary['footprint_instances_without_models']}`\n"
        f"- Issues: `{summary['issues']}`\n\n"
        f"{details}"
        "Generated artifacts:\n\n"
        "- Report: `project_health.json`\n\n"
        "This command is non-destructive. It scans project assets and model "
        "references without editing schematic, PCB, or library files.\n"
    )


def cmd_health(args: argparse.Namespace) -> int:
    """Run non-destructive project asset health checks."""
    project_path = _resolve_input_project(str(args.file) if args.file else None)
    if project_path is None:
        return 1

    output_dir = resolve_output_dir(args.output, "health")
    try:
        started = time.perf_counter()
        log.info("Project health: scanning %s", project_path)
        asset_scan = _scan_assets_with_progress(project_path)
        report = _health_payload(project_path, asset_scan)
        report_path = output_dir / "project_health.json"
        readme_path = output_dir / "README.md"
        log.info("Project health: writing report bundle to %s", output_dir)
        _write_json(report_path, report)
        readme_path.write_text(_readme_text(report), encoding="utf-8")
    except Exception as exc:
        log.error("Project health failed: %s", exc)
        return 1

    log.info(
        _terminal_summary(
            report,
            report_path=report_path,
            readme_path=readme_path,
            elapsed_s=time.perf_counter() - started,
        )
    )
    if bool(args.fail_on_issues) and not bool(report["ok"]):
        return 1
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the health command parser."""
    parser = subparsers.add_parser(
        "health",
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
        help="output directory (default: ./output/health)",
    )
    parser.add_argument(
        "--fail-on-issues",
        action="store_true",
        help="return a nonzero exit code when diagnostics or missing models are found",
    )
    parser.set_defaults(handler=cmd_health)
    return parser

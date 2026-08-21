"""Coverage histograms for KiCad plotter IR and recorder parity work."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

from .kicad_plotter_ir import KICAD_PLOTTER_IR_ACCEPTED_SCHEMAS, KiCadPlotterOpKind
from .kicad_recorder_loader import load_recorder_file
from .testing.corpus import get_kicad_corpus_root


SCHEMA = "kicad_monkey.ir_coverage_report.v1"

RECORDER_STATE_KINDS = {
    "SetColor",
    "SetCurrentLineWidth",
    "SetDash",
    "SetViewport",
    "SetPageSettings",
    "StartPlot",
    "EndPlot",
    "StartBlock",
    "EndBlock",
    "PenTo",
}

SCHEMATIC_GEOMETRY_KINDS = {
    "ArcCenterAngle",
    "ArcThreePoint",
    "BezierCurve",
    "Circle",
    "PlotImage",
    "PlotPoly",
    "Rect",
    "Text",
    "ThickArc",
    "ThickSegment",
}

STYLE_KEYS = {
    "bold",
    "color",
    "fill",
    "fill_color",
    "font_face",
    "h_align",
    "italic",
    "line_style",
    "multiline",
    "pen_width_nm",
    "stroke_color",
    "v_align",
    "width_nm",
}

SYNTHETIC_GAP_FIXTURES = {
    "ArcCenterAngle": {
        "domain": "schematic_geometry",
        "suggested_fixture": "synthetic schematic/lib-symbol arc fixture that forces center+angle arc IR",
        "reason": "schematic arcs in current corpus are represented as ArcThreePoint, so center-angle extraction is unobserved",
    },
    "ThickArc": {
        "domain": "schematic_geometry",
        "suggested_fixture": "synthetic symbol/worksheet arc fixture with non-default stroke width",
        "reason": "thick arc SVG and equivalence paths need direct coverage independent of three-point arcs",
    },
    "FlashPadCustom": {
        "domain": "footprint_geometry",
        "suggested_fixture": "synthetic footprint with a custom pad containing polygon primitives",
        "reason": "public footprint samples do not currently exercise custom pad flash IR",
    },
    "FlashPadTrapez": {
        "domain": "footprint_geometry",
        "suggested_fixture": "synthetic footprint with trapezoid pad delta enabled",
        "reason": "public footprint samples do not currently exercise trapezoid pad flash IR",
    },
    "FlashRegularPolygon": {
        "domain": "footprint_geometry",
        "suggested_fixture": "synthetic footprint with circular/regular-polygon pad primitives",
        "reason": "public footprint samples do not currently exercise regular polygon flash IR",
    },
}


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def _inc(counter: Counter[str], key: object, amount: int = 1) -> None:
    counter[str(key)] += int(amount)


def _counter_dict(counter: Counter[str]) -> dict[str, int]:
    return {key: int(counter[key]) for key in sorted(counter)}


def _top(counter: Counter[str], *, limit: int) -> dict[str, int]:
    if limit <= 0:
        return _counter_dict(counter)
    return {key: int(value) for key, value in counter.most_common(limit)}


def _manifest_cases(
    manifest: dict[str, Any],
    *,
    statuses: set[str] | None,
) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for case in manifest.get("cases") or []:
        if not isinstance(case, dict):
            continue
        if statuses is not None and str(case.get("status", "")) not in statuses:
            continue
        out.append(case)
    return out


def _iter_ir_files(kicad_root: Path) -> Iterable[Path]:
    for path in sorted(kicad_root.rglob("output/*_ir/*.json")):
        if path.is_file():
            yield path


def _add_ir_file(
    path: Path,
    *,
    op_hist: Counter[str],
    record_hist: Counter[str],
    op_by_record: Counter[str],
    payload_keys_by_kind: dict[str, Counter[str]],
    style_values: dict[str, Counter[str]],
) -> bool:
    data = _read_json(path)
    if not data or data.get("schema") not in KICAD_PLOTTER_IR_ACCEPTED_SCHEMAS:
        return False

    for record in data.get("records") or []:
        if not isinstance(record, dict):
            continue
        record_kind = str(record.get("kind", ""))
        _inc(record_hist, record_kind)
        for op in record.get("operations") or []:
            if not isinstance(op, dict):
                continue
            kind = str(op.get("kind", ""))
            _inc(op_hist, kind)
            _inc(op_by_record, f"{record_kind}/{kind}")
            key_hist = payload_keys_by_kind.setdefault(kind, Counter())
            for key, value in op.items():
                if key not in {"kind", "index"}:
                    _inc(key_hist, key)
                if key in STYLE_KEYS:
                    _inc(style_values.setdefault(f"{kind}.{key}", Counter()), value)
    return True


def _add_recorder_file(path: Path, *, op_hist: Counter[str]) -> bool:
    try:
        doc = load_recorder_file(path)
    except Exception:
        return False
    for record in doc.records:
        for op in record.operations:
            kind = str(getattr(op.kind, "value", op.kind))
            _inc(op_hist, kind)
    return True


def _read_recorder_reports(
    case: dict[str, Any],
    *,
    generated_root: Path,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None]:
    output_root_value = case.get("output_root")
    name = str(case.get("name") or "")
    if not output_root_value or not name:
        return None, None
    output_root = generated_root / str(output_root_value)
    safe_name = "".join(ch if ch.isalnum() or ch in "_.-" else "_" for ch in name).strip("_")
    drift = _read_json(output_root / "recorder_drift" / f"{safe_name}.json")
    equiv = _read_json(output_root / "op_equivalence" / f"{safe_name}.json")
    return drift, equiv


def build_ir_coverage_report(
    kicad_root: Path,
    *,
    generated_root: Path | None = None,
    statuses: Iterable[str] | None = ("active", "reference_only"),
    top_limit: int = 40,
) -> dict[str, Any]:
    """Build an aggregate histogram report from corpus IR outputs."""
    kicad_root = kicad_root.resolve()
    generated_root = (generated_root or kicad_root).resolve()
    manifest_path = kicad_root / "manifest.json"
    manifest = _read_json(manifest_path)
    if manifest is None:
        raise FileNotFoundError(f"KiCad corpus manifest not found or invalid: {manifest_path}")

    status_set = None if statuses is None else {str(status) for status in statuses}
    cases = _manifest_cases(manifest, statuses=status_set)

    monkey_op_hist: Counter[str] = Counter()
    monkey_record_hist: Counter[str] = Counter()
    monkey_op_by_record: Counter[str] = Counter()
    payload_keys_by_kind: dict[str, Counter[str]] = {}
    style_values: dict[str, Counter[str]] = {}
    ir_files = 0
    for path in _iter_ir_files(generated_root):
        if _add_ir_file(
            path,
            op_hist=monkey_op_hist,
            record_hist=monkey_record_hist,
            op_by_record=monkey_op_by_record,
            payload_keys_by_kind=payload_keys_by_kind,
            style_values=style_values,
        ):
            ir_files += 1

    recorder_op_hist: Counter[str] = Counter()
    recorder_files = 0
    recorder_cases = [case for case in cases if case.get("recorder_file")]
    for case in recorder_cases:
        recorder_file = kicad_root / str(case["recorder_file"])
        if recorder_file.exists() and _add_recorder_file(recorder_file, op_hist=recorder_op_hist):
            recorder_files += 1

    recorder_delta_positive: Counter[str] = Counter()
    monkey_delta_positive: Counter[str] = Counter()
    equivalence_rows: list[dict[str, Any]] = []
    drift_reports = 0
    equivalence_reports = 0

    for case in recorder_cases:
        drift, equiv = _read_recorder_reports(case, generated_root=generated_root)
        if drift:
            drift_reports += 1
            delta = ((drift.get("op_hist") or {}).get("delta") or {})
            if isinstance(delta, dict):
                for kind, value in delta.items():
                    try:
                        amount = int(value)
                    except (TypeError, ValueError):
                        continue
                    if amount > 0:
                        _inc(recorder_delta_positive, kind, amount)
                    elif amount < 0:
                        _inc(monkey_delta_positive, kind, -amount)
        if equiv:
            equivalence_reports += 1
            stream_sizes = equiv.get("stream_sizes") or {}
            outcomes = equiv.get("pair_outcomes") or {}
            length = equiv.get("length_divergence") or {}
            first = equiv.get("first_divergence") or {}
            recorder_total = int(stream_sizes.get("recorder_total", 0) or 0)
            matched = int(outcomes.get("matched", 0) or 0)
            ratio = float(equiv.get("match_ratio", 0.0) or 0.0)
            if recorder_total and not ratio:
                ratio = matched / recorder_total
            equivalence_rows.append(
                {
                    "id": case.get("id", ""),
                    "name": case.get("name", ""),
                    "origin": case.get("origin", ""),
                    "status": case.get("status", ""),
                    "equivalent": bool(equiv.get("equivalent", False)),
                    "matched_pairs": matched,
                    "recorder_total": recorder_total,
                    "monkey_total": int(stream_sizes.get("monkey_total", 0) or 0),
                    "match_ratio": ratio,
                    "monkey_short": int(length.get("monkey_short", 0) or 0),
                    "monkey_long": int(length.get("monkey_long", 0) or 0),
                    "style_mismatches": int(outcomes.get("style_mismatches", 0) or 0),
                    "first_divergence_kind": first.get("kind") if isinstance(first, dict) else None,
                    "first_divergence_details": first.get("details") if isinstance(first, dict) else None,
                }
            )

    known_op_kinds = {kind.value for kind in KiCadPlotterOpKind}
    seen_op_kinds = set(monkey_op_hist) | set(recorder_op_hist)
    unseen_known = sorted(known_op_kinds - seen_op_kinds)
    unseen_schematic_geometry = sorted(SCHEMATIC_GEOMETRY_KINDS - seen_op_kinds)
    synthetic_gap_plan = [
        {
            "kind": kind,
            **SYNTHETIC_GAP_FIXTURES.get(
                kind,
                {
                    "domain": "unknown",
                    "suggested_fixture": "add a focused synthetic corpus fixture",
                    "reason": "known plotter op kind is not represented in current corpus outputs",
                },
            ),
        }
        for kind in unseen_known
    ]

    non_equivalent = [
        row for row in equivalence_rows if not row["equivalent"]
    ]
    non_equivalent.sort(
        key=lambda row: (
            int(row["monkey_short"]) + int(row["monkey_long"]) + int(row["style_mismatches"]),
            row["match_ratio"],
        ),
        reverse=True,
    )

    return {
        "schema": SCHEMA,
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "kicad_root": str(kicad_root),
        "summary": {
            "manifest_cases": len(cases),
            "ir_files": ir_files,
            "recorder_cases": len(recorder_cases),
            "recorder_files": recorder_files,
            "drift_reports": drift_reports,
            "equivalence_reports": equivalence_reports,
            "equivalent_reports": sum(1 for row in equivalence_rows if row["equivalent"]),
            "non_equivalent_reports": sum(1 for row in equivalence_rows if not row["equivalent"]),
            "monkey_total_ops": sum(monkey_op_hist.values()),
            "recorder_total_ops": sum(recorder_op_hist.values()),
            "known_plotter_op_kinds": len(known_op_kinds),
            "seen_plotter_op_kinds": len(seen_op_kinds),
        },
        "coverage": {
            "known_plotter_op_kinds": sorted(known_op_kinds),
            "seen_plotter_op_kinds": sorted(seen_op_kinds),
            "unseen_known_plotter_op_kinds": unseen_known,
            "unseen_known_schematic_geometry_kinds": unseen_schematic_geometry,
            "synthetic_gap_plan": synthetic_gap_plan,
        },
        "histograms": {
            "monkey_op_kind": _counter_dict(monkey_op_hist),
            "recorder_op_kind": _counter_dict(recorder_op_hist),
            "monkey_record_kind": _counter_dict(monkey_record_hist),
            "monkey_op_kind_by_record_kind": _counter_dict(monkey_op_by_record),
            "payload_keys_by_op_kind": {
                kind: _counter_dict(counter)
                for kind, counter in sorted(payload_keys_by_kind.items())
            },
            "style_values_by_op_key_top": {
                key: _top(counter, limit=top_limit)
                for key, counter in sorted(style_values.items())
            },
            "recorder_delta_positive_by_op_kind": _counter_dict(recorder_delta_positive),
            "monkey_delta_positive_by_op_kind": _counter_dict(monkey_delta_positive),
            "recorder_delta_positive_geometry_by_op_kind": _counter_dict(
                Counter(
                    {
                        kind: value
                        for kind, value in recorder_delta_positive.items()
                        if kind not in RECORDER_STATE_KINDS
                    }
                )
            ),
            "monkey_delta_positive_geometry_by_op_kind": _counter_dict(
                Counter(
                    {
                        kind: value
                        for kind, value in monkey_delta_positive.items()
                        if kind not in RECORDER_STATE_KINDS
                    }
                )
            ),
        },
        "equivalence": {
            "cases": sorted(equivalence_rows, key=lambda row: str(row["name"])),
            "non_equivalent_ranked": non_equivalent,
        },
    }


def _markdown_table(headers: list[str], rows: list[list[object]]) -> str:
    if not rows:
        return "_None._\n"
    out = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        out.append("| " + " | ".join(str(cell) for cell in row) + " |")
    return "\n".join(out) + "\n"


def render_ir_coverage_markdown(report: dict[str, Any], *, top_limit: int = 20) -> str:
    summary = report["summary"]
    hist = report["histograms"]
    coverage = report["coverage"]
    equivalence = report["equivalence"]

    lines: list[str] = [
        "# KiCad Monkey IR Coverage",
        "",
        f"Generated: `{report['generated_at']}`",
        f"Corpus: `{report['kicad_root']}`",
        "",
        "## Summary",
        "",
        _markdown_table(
            ["metric", "value"],
            [[key, value] for key, value in summary.items()],
        ),
        "",
        "## Known Plotter Op Coverage",
        "",
        f"Seen known op kinds: {summary['seen_plotter_op_kinds']} / {summary['known_plotter_op_kinds']}",
        "",
        "Unseen known plotter op kinds: "
        + (", ".join(coverage["unseen_known_plotter_op_kinds"]) or "none"),
        "",
        "Unseen schematic geometry kinds: "
        + (", ".join(coverage["unseen_known_schematic_geometry_kinds"]) or "none"),
        "",
        "## Synthetic Gap Plan",
        "",
        _markdown_table(
            ["kind", "domain", "suggested fixture", "reason"],
            [
                [
                    row["kind"],
                    row["domain"],
                    row["suggested_fixture"],
                    row["reason"],
                ]
                for row in coverage.get("synthetic_gap_plan", [])
            ],
        ),
        "",
        "## Top Monkey IR Op Kinds",
        "",
        _markdown_table(
            ["kind", "count"],
            [[key, value] for key, value in Counter(hist["monkey_op_kind"]).most_common(top_limit)],
        ),
        "",
        "## Top Recorder Op Kinds",
        "",
        _markdown_table(
            ["kind", "count"],
            [[key, value] for key, value in Counter(hist["recorder_op_kind"]).most_common(top_limit)],
        ),
        "",
        "## Aggregate Recorder-Monkey Geometry Deltas",
        "",
        "Positive recorder deltas mean recorder has more ops of that kind than monkey.",
        "",
        _markdown_table(
            ["recorder extra kind", "count"],
            [
                [key, value]
                for key, value in Counter(
                    hist["recorder_delta_positive_geometry_by_op_kind"]
                ).most_common(top_limit)
            ],
        ),
        "",
        "Positive monkey deltas mean monkey has more ops of that kind than recorder.",
        "",
        _markdown_table(
            ["monkey extra kind", "count"],
            [
                [key, value]
                for key, value in Counter(
                    hist["monkey_delta_positive_geometry_by_op_kind"]
                ).most_common(top_limit)
            ],
        ),
        "",
        "## Non-Equivalent Recorder Cases",
        "",
        _markdown_table(
            [
                "case",
                "matched",
                "recorder",
                "ratio",
                "short",
                "long",
                "style",
                "first",
            ],
            [
                [
                    row["name"],
                    row["matched_pairs"],
                    row["recorder_total"],
                    f"{float(row['match_ratio']):.3f}",
                    row["monkey_short"],
                    row["monkey_long"],
                    row["style_mismatches"],
                    row["first_divergence_kind"] or "",
                ]
                for row in equivalence["non_equivalent_ranked"]
            ],
        ),
        "",
    ]
    return "\n".join(lines)


def write_ir_coverage_report(
    report: dict[str, Any],
    *,
    output_json: Path,
    output_md: Path,
    top_limit: int = 20,
) -> None:
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    output_md.parent.mkdir(parents=True, exist_ok=True)
    output_md.write_text(render_ir_coverage_markdown(report, top_limit=top_limit), encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--kicad-root",
        type=Path,
        default=None,
        help="Path to the KiCad corpus root. Defaults to resolved KM_CORPUS.",
    )
    parser.add_argument(
        "--status",
        action="append",
        default=None,
        help=(
            "Manifest status to include. Repeat for multiple statuses. "
            "Defaults to active and reference_only."
        ),
    )
    parser.add_argument(
        "--generated-root",
        type=Path,
        default=None,
        help="Optional root containing remapped generated corpus outputs.",
    )
    parser.add_argument("--output-json", type=Path, default=None)
    parser.add_argument("--output-md", type=Path, default=None)
    parser.add_argument("--top-limit", type=int, default=40)
    args = parser.parse_args(argv)
    kicad_root = args.kicad_root or get_kicad_corpus_root()
    generated_root = args.generated_root
    if generated_root is None and os.environ.get("KM_CORPUS_OUTPUT_ROOT"):
        generated_root = Path(os.environ["KM_CORPUS_OUTPUT_ROOT"])

    statuses = args.status if args.status is not None else ["active", "reference_only"]
    report = build_ir_coverage_report(
        kicad_root,
        generated_root=generated_root,
        statuses=statuses,
        top_limit=args.top_limit,
    )
    report_root = Path(tempfile.gettempdir()) / "kicad-monkey" / "reports"
    output_json = args.output_json or (report_root / "ir_coverage_report.json")
    output_md = args.output_md or (report_root / "ir_coverage_report.md")
    write_ir_coverage_report(
        report,
        output_json=output_json,
        output_md=output_md,
        top_limit=args.top_limit,
    )
    print(output_json)
    print(output_md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

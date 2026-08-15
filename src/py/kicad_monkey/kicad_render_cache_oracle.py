"""KiCad render-cache oracle helpers.

The primary oracle path is KiCad's own PCB save/upgrade writer: load a board,
force a save, and parse the `(render_cache ...)` blocks that KiCad generated
from semantic text objects.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from collections import Counter
from dataclasses import dataclass, field
from datetime import datetime, timezone
from math import hypot, isclose
from pathlib import Path
from typing import Any, Iterable, Optional

from .kicad_pcb import KiCadPcb
from .kicad_primitives import RenderCache
from .kicad_render_cache import (
    RenderCacheRequest,
    RenderCacheResolver,
    render_cache_request_for_board_text,
    render_cache_request_for_dimension_text,
    render_cache_request_for_footprint_property,
    render_cache_request_for_footprint_text,
    render_cache_request_for_footprint_text_box,
    render_cache_request_for_table_cell,
)
from .kicad_sexpr import build_sexp, parse_sexp


class KiCadRenderCacheOracleError(RuntimeError):
    """Raised when the external KiCad cache oracle cannot produce output."""


@dataclass(frozen=True)
class RenderCacheOracleEntry:
    """One render cache recovered from a KiCad-rewritten board."""

    object_path: str
    object_type: str
    text: str
    cache: RenderCache
    layer: str = ""
    uuid: Optional[str] = None


@dataclass
class RenderCacheOracleResult:
    """Result from the KiCad save-cache oracle."""

    source_pcb: Path
    oracle_pcb: Path
    entries: list[RenderCacheOracleEntry] = field(default_factory=list)
    stdout: str = ""
    stderr: str = ""


@dataclass
class RenderCacheComparison:
    """Strict typed comparison result for two render-cache payloads."""

    reasons: list[str] = field(default_factory=list)
    max_point_delta: float = 0.0
    compared_points: int = 0

    @property
    def matched(self) -> bool:
        return not self.reasons


@dataclass
class RenderCacheEntrySetComparison:
    """Comparison result for two sets of render-cache oracle entries."""

    missing_keys: list[str] = field(default_factory=list)
    extra_keys: list[str] = field(default_factory=list)
    duplicate_keys: list[str] = field(default_factory=list)
    entry_results: dict[str, RenderCacheComparison] = field(default_factory=dict)

    @property
    def matched(self) -> bool:
        return (
            not self.missing_keys
            and not self.extra_keys
            and not self.duplicate_keys
            and all(result.matched for result in self.entry_results.values())
        )


@dataclass(frozen=True)
class RenderCacheCoverageSummary:
    """Histogram-style coverage summary for recovered render caches."""

    entry_count: int
    object_type_counts: dict[str, int]
    layer_counts: dict[str, int]
    polygon_count: int
    contour_count: int
    hole_polygon_count: int
    max_contours_per_polygon: int

    def missing_object_types(self, required_types: Iterable[str]) -> list[str]:
        """Return required object types that are absent from this summary."""

        return [
            object_type
            for object_type in required_types
            if self.object_type_counts.get(object_type, 0) == 0
        ]


@dataclass(frozen=True)
class RenderCacheCoverageObject:
    """One text-bearing PCB object in render-cache coverage accounting."""

    object_path: str
    object_type: str
    text: str
    layer: str
    font_face: str
    outline_font: bool
    existing_cache_state: str
    resolved_cache_source: str
    usable: bool
    exact: bool
    validation_reasons: tuple[str, ...] = ()
    validation_warnings: tuple[str, ...] = ()


def strip_render_cache_blocks_from_sexp(node: Any) -> Any:
    """Remove every nested `(render_cache ...)` element from parsed S-expr data."""

    if not isinstance(node, list):
        return node

    stripped: list[Any] = []
    for child in node:
        if isinstance(child, list) and child and child[0] == "render_cache":
            continue
        stripped.append(strip_render_cache_blocks_from_sexp(child))
    return stripped


def strip_render_cache_blocks(text: str) -> str:
    """Return KiCad S-expression text with all render caches removed."""

    return build_sexp(strip_render_cache_blocks_from_sexp(parse_sexp(text))) + "\n"


def extract_render_cache_entries_from_pcb(pcb: KiCadPcb) -> list[RenderCacheOracleEntry]:
    """Collect typed render caches from a parsed board."""

    entries: list[RenderCacheOracleEntry] = []

    def add(
        *,
        object_path: str,
        object_type: str,
        text: str,
        cache: Optional[RenderCache],
        layer: str = "",
        uuid: Optional[str] = None,
    ) -> None:
        if cache is None:
            return
        entries.append(
            RenderCacheOracleEntry(
                object_path=object_path,
                object_type=object_type,
                text=text,
                cache=cache,
                layer=layer,
                uuid=uuid,
            )
        )

    for index, text in enumerate(getattr(pcb, "gr_texts", []) or []):
        add(
            object_path=f"gr_text[{index}]",
            object_type="gr_text",
            text=getattr(text, "text", ""),
            cache=getattr(text, "render_cache", None),
            layer=getattr(text, "layer", ""),
            uuid=getattr(text, "uuid", None),
        )

    for index, text_box in enumerate(getattr(pcb, "gr_text_boxes", []) or []):
        add(
            object_path=f"gr_text_box[{index}]",
            object_type="gr_text_box",
            text=getattr(text_box, "text", ""),
            cache=getattr(text_box, "render_cache", None),
            layer=getattr(text_box, "layer", ""),
            uuid=getattr(text_box, "uuid", None),
        )

    for table_index, table in enumerate(getattr(pcb, "tables", []) or []):
        for cell_index, cell in enumerate(getattr(table, "cells", []) or []):
            add(
                object_path=f"table[{table_index}]/table_cell[{cell_index}]",
                object_type="table_cell",
                text=getattr(cell, "text", ""),
                cache=getattr(cell, "render_cache", None),
                layer=getattr(cell, "layer", getattr(table, "layer", "")),
                uuid=getattr(cell, "uuid", None),
            )

    for dimension_index, dimension in enumerate(getattr(pcb, "dimensions", []) or []):
        text = getattr(dimension, "gr_text", None)
        if text is None:
            continue
        add(
            object_path=f"dimension[{dimension_index}]/gr_text",
            object_type="dimension",
            text=getattr(text, "text", ""),
            cache=getattr(text, "render_cache", None),
            layer=getattr(text, "layer", getattr(dimension, "layer", "")),
            uuid=getattr(text, "uuid", getattr(dimension, "uuid", None)),
        )

    for footprint_index, footprint in enumerate(getattr(pcb, "footprints", []) or []):
        footprint_id = (
            getattr(footprint, "reference", None)
            or getattr(footprint, "name", None)
            or str(footprint_index)
        )
        footprint_path = f"footprint[{footprint_index}:{footprint_id}]"

        for text_index, text in enumerate(getattr(footprint, "fp_texts", []) or []):
            add(
                object_path=f"{footprint_path}/fp_text[{text_index}]",
                object_type="fp_text",
                text=getattr(text, "text", ""),
                cache=getattr(text, "render_cache", None),
                layer=getattr(text, "layer", ""),
                uuid=getattr(text, "uuid", None),
            )

        for prop_index, prop in enumerate(getattr(footprint, "properties", []) or []):
            add(
                object_path=f"{footprint_path}/property[{prop_index}:{getattr(prop, 'name', '')}]",
                object_type="property",
                text=getattr(prop, "value", ""),
                cache=getattr(prop, "render_cache", None),
                layer=getattr(prop, "layer", ""),
                uuid=getattr(prop, "uuid", None),
            )

        for box_index, text_box in enumerate(getattr(footprint, "fp_text_boxes", []) or []):
            add(
                object_path=f"{footprint_path}/fp_text_box[{box_index}]",
                object_type="fp_text_box",
                text=getattr(text_box, "text", ""),
                cache=getattr(text_box, "render_cache", None),
                layer=getattr(text_box, "layer", ""),
                uuid=getattr(text_box, "uuid", None),
            )

        for table_index, table in enumerate(getattr(footprint, "tables", []) or []):
            for cell_index, cell in enumerate(getattr(table, "cells", []) or []):
                add(
                    object_path=f"{footprint_path}/table[{table_index}]/table_cell[{cell_index}]",
                    object_type="table_cell",
                    text=getattr(cell, "text", ""),
                    cache=getattr(cell, "render_cache", None),
                    layer=getattr(cell, "layer", getattr(table, "layer", "")),
                    uuid=getattr(cell, "uuid", None),
                )

        for dimension_index, dimension in enumerate(getattr(footprint, "dimensions", []) or []):
            text = getattr(dimension, "gr_text", None)
            if text is None:
                continue
            add(
                object_path=f"{footprint_path}/dimension[{dimension_index}]/gr_text",
                object_type="dimension",
                text=getattr(text, "text", ""),
                cache=getattr(text, "render_cache", None),
                layer=getattr(text, "layer", getattr(dimension, "layer", "")),
                uuid=getattr(text, "uuid", getattr(dimension, "uuid", None)),
            )

    return entries


def summarize_render_cache_entries(
    entries: Iterable[RenderCacheOracleEntry],
) -> RenderCacheCoverageSummary:
    """Build coverage histograms for oracle-generated render caches."""

    entry_list = list(entries)
    object_type_counts = Counter(entry.object_type for entry in entry_list)
    layer_counts = Counter(entry.layer for entry in entry_list if entry.layer)
    polygon_count = 0
    contour_count = 0
    hole_polygon_count = 0
    max_contours_per_polygon = 0

    for entry in entry_list:
        for polygon in entry.cache.polygons:
            polygon_count += 1
            polygon_contours = len(polygon.contours)
            contour_count += polygon_contours
            max_contours_per_polygon = max(max_contours_per_polygon, polygon_contours)
            if polygon_contours > 1:
                hole_polygon_count += 1

    return RenderCacheCoverageSummary(
        entry_count=len(entry_list),
        object_type_counts=dict(sorted(object_type_counts.items())),
        layer_counts=dict(sorted(layer_counts.items())),
        polygon_count=polygon_count,
        contour_count=contour_count,
        hole_polygon_count=hole_polygon_count,
        max_contours_per_polygon=max_contours_per_polygon,
    )


def _has_outline_font(text_object: Any) -> bool:
    effects = getattr(text_object, "effects", None)
    font = getattr(effects, "font", None)
    return bool(getattr(font, "face", None))


def collect_render_cache_requests_from_pcb(pcb: KiCadPcb) -> list[RenderCacheRequest]:
    """Collect semantic render-cache requests for every modeled PCB text object."""

    requests: list[RenderCacheRequest] = []

    # Empty-text objects are collected too: KiCad saves a polygon-less
    # render_cache for empty outline-font texts (fp_text "" reference
    # placeholders, blank Datasheet properties), so parity requires a
    # request for every text object regardless of resolved content.
    for index, text in enumerate(getattr(pcb, "gr_texts", []) or []):
        requests.append(
            render_cache_request_for_board_text(
                text,
                pcb,
                object_type="gr_text",
                object_path=f"gr_text[{index}]",
                include_text_params=_has_outline_font(text),
            )
        )

    for index, text_box in enumerate(getattr(pcb, "gr_text_boxes", []) or []):
        requests.append(
            render_cache_request_for_board_text(
                text_box,
                pcb,
                object_type="gr_text_box",
                object_path=f"gr_text_box[{index}]",
                include_text_params=_has_outline_font(text_box),
            )
        )

    for table_index, table in enumerate(getattr(pcb, "tables", []) or []):
        for cell_index, cell in enumerate(getattr(table, "cells", []) or []):
            requests.append(
                render_cache_request_for_table_cell(
                    cell,
                    table,
                    pcb,
                    object_path=f"table[{table_index}]/table_cell[{cell_index}]",
                    include_text_params=_has_outline_font(cell),
                )
            )

    for dimension_index, dimension in enumerate(getattr(pcb, "dimensions", []) or []):
        text = (
            dimension.resolved_gr_text()
            if hasattr(dimension, "resolved_gr_text")
            else getattr(dimension, "gr_text", None)
        )
        if text is None:
            continue
        requests.append(
            render_cache_request_for_dimension_text(
                dimension,
                pcb,
                object_path=f"dimension[{dimension_index}]/gr_text",
                include_text_params=_has_outline_font(text),
            )
        )

    for footprint_index, footprint in enumerate(getattr(pcb, "footprints", []) or []):
        footprint_id = (
            getattr(footprint, "reference", None)
            or getattr(footprint, "name", None)
            or str(footprint_index)
        )
        footprint_path = f"footprint[{footprint_index}:{footprint_id}]"

        for text_index, text in enumerate(getattr(footprint, "fp_texts", []) or []):
            requests.append(
                render_cache_request_for_footprint_text(
                    text,
                    footprint,
                    object_path=f"{footprint_path}/fp_text[{text_index}]",
                    include_text_params=_has_outline_font(text),
                )
            )

        for prop_index, prop in enumerate(getattr(footprint, "properties", []) or []):
            requests.append(
                render_cache_request_for_footprint_property(
                    prop,
                    footprint,
                    object_path=f"{footprint_path}/property[{prop_index}:{getattr(prop, 'name', '')}]",
                    include_text_params=_has_outline_font(prop),
                )
            )

        for box_index, text_box in enumerate(getattr(footprint, "fp_text_boxes", []) or []):
            requests.append(
                render_cache_request_for_footprint_text_box(
                    text_box,
                    footprint,
                    object_path=f"{footprint_path}/fp_text_box[{box_index}]",
                    include_text_params=_has_outline_font(text_box),
                )
            )

        for table_index, table in enumerate(getattr(footprint, "tables", []) or []):
            for cell_index, cell in enumerate(getattr(table, "cells", []) or []):
                requests.append(
                    render_cache_request_for_table_cell(
                        cell,
                        table,
                        footprint=footprint,
                        object_path=f"{footprint_path}/table[{table_index}]/table_cell[{cell_index}]",
                        include_text_params=_has_outline_font(cell),
                    )
                )

        for dimension_index, dimension in enumerate(getattr(footprint, "dimensions", []) or []):
            text = getattr(dimension, "gr_text", None)
            if text is None:
                continue
            requests.append(
                render_cache_request_for_dimension_text(
                    dimension,
                    object_path=f"{footprint_path}/dimension[{dimension_index}]/gr_text",
                    include_text_params=_has_outline_font(text),
                )
            )

    return requests


def _existing_cache_state(request: RenderCacheRequest, resolver: RenderCacheResolver) -> str:
    validation = resolver.validate_cache(request)
    if request.render_cache is None:
        return "missing"
    if validation.valid:
        return "present_valid"
    return "present_invalid"


def _coverage_object_from_request(
    request: RenderCacheRequest,
    resolver: RenderCacheResolver,
) -> RenderCacheCoverageObject:
    initial_state = _existing_cache_state(request, resolver)
    try:
        result = resolver.ensure_cache(request)
        source = result.source.value
        usable = result.usable
        exact = result.exact
        reasons = tuple(result.validation.reasons)
        warnings = tuple(result.validation.warnings)
    except Exception as exc:  # pragma: no cover - defensive report path
        source = "resolver_error"
        usable = False
        exact = False
        reasons = (type(exc).__name__, str(exc))
        warnings = ()

    return RenderCacheCoverageObject(
        object_path=request.object_path,
        object_type=request.object_type,
        text=request.text,
        layer="",
        font_face=request.font_face or "",
        outline_font=bool(request.font_face),
        existing_cache_state=initial_state,
        resolved_cache_source=source,
        usable=usable,
        exact=exact,
        validation_reasons=reasons,
        validation_warnings=warnings,
    )


def summarize_render_cache_requests(
    requests: Iterable[RenderCacheRequest],
) -> dict[str, Any]:
    """Build the completeness histogram for semantic render-cache requests."""

    resolver = RenderCacheResolver()
    objects = [
        _coverage_object_from_request(request, resolver)
        for request in requests
    ]

    object_type_counts = Counter(obj.object_type for obj in objects)
    font_counts = Counter(obj.font_face or "<stroke_default>" for obj in objects)
    existing_state_counts = Counter(obj.existing_cache_state for obj in objects)
    resolved_source_counts = Counter(obj.resolved_cache_source for obj in objects)
    outline_counts = Counter("outline_font" if obj.outline_font else "stroke_default" for obj in objects)
    warning_counts: Counter[str] = Counter()
    reason_counts: Counter[str] = Counter()
    for obj in objects:
        warning_counts.update(obj.validation_warnings)
        reason_counts.update(obj.validation_reasons)

    gaps = [
        {
            "object_path": obj.object_path,
            "object_type": obj.object_type,
            "text": obj.text,
            "font_face": obj.font_face,
            "existing_cache_state": obj.existing_cache_state,
            "resolved_cache_source": obj.resolved_cache_source,
            "validation_reasons": list(obj.validation_reasons),
            "validation_warnings": list(obj.validation_warnings),
        }
        for obj in objects
        if obj.outline_font and not obj.usable
    ]

    return {
        "object_count": len(objects),
        "usable_count": sum(1 for obj in objects if obj.usable),
        "exact_count": sum(1 for obj in objects if obj.exact),
        "gap_count": len(gaps),
        "histograms": {
            "object_type": dict(sorted(object_type_counts.items())),
            "font_face": dict(sorted(font_counts.items())),
            "font_kind": dict(sorted(outline_counts.items())),
            "existing_cache_state": dict(sorted(existing_state_counts.items())),
            "resolved_cache_source": dict(sorted(resolved_source_counts.items())),
            "validation_reasons": dict(sorted(reason_counts.items())),
            "validation_warnings": dict(sorted(warning_counts.items())),
        },
        "gaps": gaps,
    }


def build_render_cache_coverage_report_from_pcb(
    pcb: KiCadPcb,
    *,
    source_path: str = "",
) -> dict[str, Any]:
    """Build a coverage report for one parsed PCB."""

    requests = collect_render_cache_requests_from_pcb(pcb)
    summary = summarize_render_cache_requests(requests)
    return {
        "schema": "kicad_monkey.render_cache_coverage_report.v1",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "source_path": source_path,
        **summary,
    }


def _read_manifest(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def _manifest_pcb_cases(
    kicad_root: Path,
    *,
    statuses: Iterable[str] | None,
) -> list[tuple[dict[str, Any], Path]]:
    manifest = _read_manifest(kicad_root / "manifest.json")
    status_set = None if statuses is None else {str(status) for status in statuses}
    cases: list[tuple[dict[str, Any], Path]] = []
    for case in manifest.get("cases") or []:
        if not isinstance(case, dict):
            continue
        if status_set is not None and str(case.get("status", "")) not in status_set:
            continue
        for key in ("board_file", "input_file"):
            value = case.get(key)
            if not value:
                continue
            path = kicad_root / str(value)
            if path.suffix == ".kicad_pcb":
                cases.append((case, path))
                break
    return cases


def build_render_cache_coverage_report(
    kicad_root: Path,
    *,
    statuses: Iterable[str] | None = ("active", "reference_only"),
) -> dict[str, Any]:
    """Build a manifest-driven render-cache coverage report for PCB cases."""

    kicad_root = kicad_root.resolve()
    case_reports: list[dict[str, Any]] = []
    parse_errors: list[dict[str, str]] = []
    aggregate_requests: list[RenderCacheRequest] = []
    origins: Counter[str] = Counter()

    for case, path in _manifest_pcb_cases(kicad_root, statuses=statuses):
        if not path.exists():
            parse_errors.append({
                "id": str(case.get("id", "")),
                "path": str(path),
                "error": "missing_file",
            })
            continue
        try:
            pcb = KiCadPcb.from_file(path)
            requests = collect_render_cache_requests_from_pcb(pcb)
        except Exception as exc:  # pragma: no cover - defensive report path
            parse_errors.append({
                "id": str(case.get("id", "")),
                "path": str(path),
                "error": f"{type(exc).__name__}: {exc}",
            })
            continue

        aggregate_requests.extend(requests)
        origin = str(case.get("origin", ""))
        origins[origin] += 1
        case_summary = summarize_render_cache_requests(requests)
        case_reports.append({
            "id": str(case.get("id", "")),
            "name": str(case.get("name", "")),
            "origin": origin,
            "status": str(case.get("status", "")),
            "path": str(path),
            "object_count": case_summary["object_count"],
            "usable_count": case_summary["usable_count"],
            "gap_count": case_summary["gap_count"],
            "histograms": case_summary["histograms"],
            "gaps": case_summary["gaps"],
        })

    aggregate = summarize_render_cache_requests(aggregate_requests)
    return {
        "schema": "kicad_monkey.render_cache_coverage_report.v1",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "kicad_root": str(kicad_root),
        "summary": {
            "manifest_pcb_cases": len(case_reports),
            "parse_error_count": len(parse_errors),
            "origins": dict(sorted(origins.items())),
            "object_count": aggregate["object_count"],
            "usable_count": aggregate["usable_count"],
            "exact_count": aggregate["exact_count"],
            "gap_count": aggregate["gap_count"],
        },
        "histograms": aggregate["histograms"],
        "gaps": aggregate["gaps"],
        "case_reports": case_reports,
        "parse_errors": parse_errors,
    }


def render_cache_coverage_markdown(report: dict[str, Any], *, top_limit: int = 40) -> str:
    """Render a compact Markdown summary for review artifacts."""

    summary = report.get("summary") or report
    histograms = report.get("histograms") or {}
    lines = [
        "# KiCad Render Cache Coverage",
        "",
        f"- generated_at: `{report.get('generated_at', '')}`",
        f"- object_count: `{summary.get('object_count', 0)}`",
        f"- usable_count: `{summary.get('usable_count', 0)}`",
        f"- exact_count: `{summary.get('exact_count', 0)}`",
        f"- gap_count: `{summary.get('gap_count', 0)}`",
        "",
    ]
    for name, values in histograms.items():
        if not isinstance(values, dict):
            continue
        lines.append(f"## {name}")
        for key, value in sorted(values.items(), key=lambda item: (-int(item[1]), str(item[0])))[:top_limit]:
            lines.append(f"- `{key}`: {value}")
        lines.append("")

    gaps = report.get("gaps") or []
    if gaps:
        lines.append("## Gaps")
        for gap in gaps[:top_limit]:
            lines.append(
                f"- `{gap.get('object_path', '')}` "
                f"({gap.get('object_type', '')}, {gap.get('resolved_cache_source', '')})"
            )
        lines.append("")
    return "\n".join(lines)


def write_render_cache_coverage_report(
    report: dict[str, Any],
    *,
    output_json: Path,
    output_md: Path,
    top_limit: int = 40,
) -> None:
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    output_md.parent.mkdir(parents=True, exist_ok=True)
    output_md.write_text(
        render_cache_coverage_markdown(report, top_limit=top_limit),
        encoding="utf-8",
    )


def compare_render_caches(
    expected: RenderCache,
    actual: RenderCache,
    *,
    tolerance: float = 1e-6,
    angle_tolerance: float = 1e-9,
) -> RenderCacheComparison:
    """Compare two typed render caches without SVG parsing."""

    comparison = RenderCacheComparison()

    if expected.text != actual.text:
        comparison.reasons.append("cache_text_mismatch")
    if not isclose(expected.angle, actual.angle, abs_tol=angle_tolerance):
        comparison.reasons.append("cache_angle_mismatch")
    if len(expected.polygons) != len(actual.polygons):
        comparison.reasons.append("polygon_count_mismatch")

    for polygon_index, (expected_polygon, actual_polygon) in enumerate(
        zip(expected.polygons, actual.polygons)
    ):
        if len(expected_polygon.contours) != len(actual_polygon.contours):
            comparison.reasons.append(
                f"polygon_{polygon_index}_contour_count_mismatch"
            )

        for contour_index, (expected_contour, actual_contour) in enumerate(
            zip(expected_polygon.contours, actual_polygon.contours)
        ):
            if len(expected_contour.points) != len(actual_contour.points):
                comparison.reasons.append(
                    f"polygon_{polygon_index}_contour_{contour_index}_point_count_mismatch"
                )

            for expected_point, actual_point in zip(
                expected_contour.points,
                actual_contour.points,
            ):
                delta = hypot(
                    expected_point[0] - actual_point[0],
                    expected_point[1] - actual_point[1],
                )
                comparison.compared_points += 1
                comparison.max_point_delta = max(comparison.max_point_delta, delta)
                if delta > tolerance:
                    reason = (
                        f"point_delta_exceeds_tolerance:"
                        f"polygon={polygon_index}:contour={contour_index}"
                    )
                    if reason not in comparison.reasons:
                        comparison.reasons.append(reason)

    return comparison


def compare_render_cache_entries(
    expected: RenderCacheOracleEntry,
    actual: RenderCacheOracleEntry,
    *,
    tolerance: float = 1e-6,
    angle_tolerance: float = 1e-9,
) -> RenderCacheComparison:
    """Compare two oracle entries and their typed cache payloads."""

    comparison = compare_render_caches(
        expected.cache,
        actual.cache,
        tolerance=tolerance,
        angle_tolerance=angle_tolerance,
    )
    if expected.object_type != actual.object_type:
        comparison.reasons.append("object_type_mismatch")
    if expected.text != actual.text:
        comparison.reasons.append("entry_text_mismatch")
    if expected.layer != actual.layer:
        comparison.reasons.append("layer_mismatch")
    if expected.uuid and actual.uuid and expected.uuid != actual.uuid:
        comparison.reasons.append("uuid_mismatch")
    return comparison


def _entry_compare_key(entry: RenderCacheOracleEntry) -> str:
    return entry.uuid or entry.object_path


def _index_entries(
    entries: Iterable[RenderCacheOracleEntry],
) -> tuple[dict[str, RenderCacheOracleEntry], list[str]]:
    indexed: dict[str, RenderCacheOracleEntry] = {}
    duplicates: list[str] = []
    for entry in entries:
        key = _entry_compare_key(entry)
        if key in indexed:
            duplicates.append(key)
            continue
        indexed[key] = entry
    return indexed, duplicates


def compare_render_cache_entry_sets(
    expected: Iterable[RenderCacheOracleEntry],
    actual: Iterable[RenderCacheOracleEntry],
    *,
    tolerance: float = 1e-6,
    angle_tolerance: float = 1e-9,
) -> RenderCacheEntrySetComparison:
    """Compare two render-cache entry sets by UUID, falling back to object path."""

    expected_index, expected_duplicates = _index_entries(expected)
    actual_index, actual_duplicates = _index_entries(actual)
    expected_keys = set(expected_index)
    actual_keys = set(actual_index)

    comparison = RenderCacheEntrySetComparison(
        missing_keys=sorted(expected_keys - actual_keys),
        extra_keys=sorted(actual_keys - expected_keys),
        duplicate_keys=sorted(set(expected_duplicates + actual_duplicates)),
    )

    for key in sorted(expected_keys & actual_keys):
        comparison.entry_results[key] = compare_render_cache_entries(
            expected_index[key],
            actual_index[key],
            tolerance=tolerance,
            angle_tolerance=angle_tolerance,
        )

    return comparison


def run_kicad_pcb_render_cache_save_oracle(
    *,
    kicad_cli: Path,
    source_pcb: Path,
    work_dir: Path,
    strip_existing_caches: bool = True,
    timeout: int = 60,
) -> RenderCacheOracleResult:
    """Regenerate render caches by forcing KiCad to save a temporary board."""

    work_dir.mkdir(parents=True, exist_ok=True)
    oracle_pcb = work_dir / source_pcb.name

    if strip_existing_caches:
        oracle_pcb.write_text(
            strip_render_cache_blocks(source_pcb.read_text(encoding="utf-8")),
            encoding="utf-8",
        )
    else:
        oracle_pcb.write_text(source_pcb.read_text(encoding="utf-8"), encoding="utf-8")

    result = subprocess.run(
        [str(kicad_cli), "pcb", "upgrade", "--force", str(oracle_pcb)],
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if result.returncode != 0:
        raise KiCadRenderCacheOracleError(
            "kicad-cli pcb upgrade --force failed for "
            f"{oracle_pcb} with rc={result.returncode}\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )

    pcb = KiCadPcb.from_file(oracle_pcb)
    return RenderCacheOracleResult(
        source_pcb=source_pcb,
        oracle_pcb=oracle_pcb,
        entries=extract_render_cache_entries_from_pcb(pcb),
        stdout=result.stdout,
        stderr=result.stderr,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--kicad-root",
        type=Path,
        default=Path(os.environ.get("WN_TEST_CORPUS", "tests/corpus")) / "kicad",
        help="Path to the KiCad corpus root containing manifest.json.",
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
    parser.add_argument("--output-json", type=Path, default=None)
    parser.add_argument("--output-md", type=Path, default=None)
    parser.add_argument("--top-limit", type=int, default=40)
    args = parser.parse_args(argv)

    statuses = args.status if args.status is not None else ["active", "reference_only"]
    report = build_render_cache_coverage_report(args.kicad_root, statuses=statuses)
    output_json = args.output_json or (
        args.kicad_root / "review" / "render_cache_coverage_report.json"
    )
    output_md = args.output_md or (
        args.kicad_root / "review" / "render_cache_coverage_report.md"
    )
    write_render_cache_coverage_report(
        report,
        output_json=output_json,
        output_md=output_md,
        top_limit=args.top_limit,
    )
    print(output_json)
    print(output_md)
    return 0


__all__ = [
    "KiCadRenderCacheOracleError",
    "RenderCacheComparison",
    "RenderCacheCoverageObject",
    "RenderCacheCoverageSummary",
    "RenderCacheEntrySetComparison",
    "RenderCacheOracleEntry",
    "RenderCacheOracleResult",
    "build_render_cache_coverage_report",
    "build_render_cache_coverage_report_from_pcb",
    "collect_render_cache_requests_from_pcb",
    "compare_render_cache_entries",
    "compare_render_cache_entry_sets",
    "compare_render_caches",
    "extract_render_cache_entries_from_pcb",
    "render_cache_coverage_markdown",
    "run_kicad_pcb_render_cache_save_oracle",
    "summarize_render_cache_entries",
    "summarize_render_cache_requests",
    "strip_render_cache_blocks",
    "strip_render_cache_blocks_from_sexp",
    "write_render_cache_coverage_report",
]


if __name__ == "__main__":
    raise SystemExit(main())

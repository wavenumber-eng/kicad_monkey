"""
Loader for ``kicad.plotter_recorder.v1`` JSON dumps.

The KiCad-side ``RECORDER_PLOTTER`` instrumentation emits a JSON document
with schema id ``kicad.plotter_recorder.v1`` containing every
``PLOTTER`` virtual call captured during a ``kicad-cli sch export svg``
run. This module loads that dump and translates it into the canonical
``kicad.plotter_ir.a0`` document type
(:class:`~kicad_monkey.kicad_plotter_ir.KiCadPlotterDocument`) that the
rest of the kicad_monkey toolkit consumes.

The two schemas differ in field naming on a handful of ops because the
recorder field names track the C++ method-parameter names verbatim,
while the Python IR predates the recorder and uses earlier-chosen
names. The translation table is intentionally small and lives here
(rather than mutating either schema) so both representations stay
faithful to their source-of-truth:

    SetDash:         width_nm        -> line_width_nm
    PenTo:           plume           -> action
    StartPlot:       page_number     -> page_name
    SetPageSettings: type            -> page_type
    LINE_STYLE:      DASHDOT         -> DASH_DOT
    LINE_STYLE:      DASHDOTDOT      -> DASH_DOT_DOT

All other ops (Circle, ArcThreePoint, BezierCurve, Rect, PlotPoly,
Text, PlotImage, Flash*Pad family, ThickSegment, ThickArc, SetColor,
SetCurrentLineWidth, SetViewport, StartBlock, EndBlock, EndPlot) match
field-for-field.

Recorder geometry payloads are KiCad plotter internal units, not nm, even
when a payload key has an ``_nm`` suffix. At document-load time we use the
first ``SetViewport.ius_per_decimil`` value to normalize op coordinates and
lengths into the canonical plotter-IR nanometre coordinate space. Canvas and
page-size metadata are already in nm and are not scaled.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

from .kicad_plotter_ir import (
    KiCadPlotterDocument,
    KiCadPlotterOp,
    KiCadPlotterRecord,
    _coerce_kind,
)


# =============================================================================
# Schema constant
# =============================================================================


KICAD_PLOTTER_RECORDER_SCHEMA = "kicad.plotter_recorder.v1"


# One decimil is 0.0001 inch = 2540 nm. Recorder geometry payloads are
# KiCad plotter internal units, and SetViewport tells us how many of
# those units fit in one decimil.
_NM_PER_DECIMIL = 2540.0


_COORD_OR_LENGTH_KEYS: frozenset[str] = frozenset(
    {
        "x",
        "y",
        "cx",
        "cy",
        "x1",
        "y1",
        "x2",
        "y2",
        "start_x",
        "start_y",
        "mid_x",
        "mid_y",
        "end_x",
        "end_y",
        "ctrl1_x",
        "ctrl1_y",
        "ctrl2_x",
        "ctrl2_y",
        "offset_x_nm",
        "offset_y_nm",
        "width_nm",
        "height_nm",
        "line_width_nm",
        "diameter_nm",
        "radius_nm",
        "size_x_nm",
        "size_y_nm",
        "corner_radius_nm",
        "pen_width_nm",
        "tolerance_nm",
    }
)


_PAGE_SIZE_KINDS: frozenset[str] = frozenset({"SetPageSettings"})


# =============================================================================
# Translation tables
# =============================================================================


_LINE_STYLE_RENAMES: dict[str, str] = {
    "DASHDOT": "DASH_DOT",
    "DASHDOTDOT": "DASH_DOT_DOT",
}


def _normalise_line_style(value: Any) -> Any:
    if isinstance(value, str):
        return _LINE_STYLE_RENAMES.get(value, value)
    return value


def _kind_str(op: KiCadPlotterOp) -> str:
    kind = op.kind
    return str(getattr(kind, "value", kind))


def _scale_number_to_nm(value: Any, nm_per_internal_unit: float) -> Any:
    if isinstance(value, bool):
        return value
    if isinstance(value, int):
        return int(round(float(value) * nm_per_internal_unit))
    if isinstance(value, float):
        return float(value) * nm_per_internal_unit
    return value


def _scale_point_list(points: Any, nm_per_internal_unit: float) -> Any:
    if not isinstance(points, list):
        return points

    out: list[Any] = []
    for point in points:
        if isinstance(point, (list, tuple)) and len(point) >= 2:
            x = _scale_number_to_nm(point[0], nm_per_internal_unit)
            y = _scale_number_to_nm(point[1], nm_per_internal_unit)
            out.append([x, y, *list(point[2:])])
        else:
            out.append(copy.deepcopy(point))
    return out


def _scale_polygon_list(polygons: Any, nm_per_internal_unit: float) -> Any:
    if not isinstance(polygons, list):
        return polygons
    return [_scale_point_list(ring, nm_per_internal_unit) for ring in polygons]


def _viewport_nm_per_internal_unit(ops: list[KiCadPlotterOp]) -> float | None:
    for op in ops:
        if _kind_str(op) != "SetViewport":
            continue
        try:
            raw_ius_per_decimil = op.payload.get("ius_per_decimil")
            if raw_ius_per_decimil is None:
                return None
            ius_per_decimil = float(raw_ius_per_decimil)
        except (TypeError, ValueError):
            return None
        if ius_per_decimil <= 0.0:
            return None
        return _NM_PER_DECIMIL / ius_per_decimil
    return None


def _normalise_op_units(
    op: KiCadPlotterOp,
    *,
    nm_per_internal_unit: float,
) -> KiCadPlotterOp:
    kind = _kind_str(op)
    if kind in _PAGE_SIZE_KINDS:
        return op

    payload = copy.deepcopy(op.payload)
    for key in list(payload):
        if key in _COORD_OR_LENGTH_KEYS:
            payload[key] = _scale_number_to_nm(payload[key], nm_per_internal_unit)

    if "points" in payload:
        payload["points"] = _scale_point_list(
            payload["points"],
            nm_per_internal_unit,
        )
    if "corners" in payload:
        payload["corners"] = _scale_point_list(
            payload["corners"],
            nm_per_internal_unit,
        )
    if "polygons" in payload:
        payload["polygons"] = _scale_polygon_list(
            payload["polygons"],
            nm_per_internal_unit,
        )

    return KiCadPlotterOp(kind=op.kind, payload=payload)


def normalise_recorder_op_units(
    ops: list[KiCadPlotterOp],
) -> tuple[list[KiCadPlotterOp], float | None]:
    """
    Convert recorder op payload coordinates from KiCad plotter internal
    units to nanometres using the first SetViewport op.

    Returns ``(ops, nm_per_internal_unit)``. If no usable viewport is
    present, the original ops are returned unchanged with ``None``.
    """
    nm_per_internal_unit = _viewport_nm_per_internal_unit(ops)
    if nm_per_internal_unit is None:
        return ops, None
    return (
        [
            _normalise_op_units(op, nm_per_internal_unit=nm_per_internal_unit)
            for op in ops
        ],
        nm_per_internal_unit,
    )


# =============================================================================
# Op translation
# =============================================================================


def translate_recorder_op(raw_op: dict[str, Any]) -> KiCadPlotterOp:
    """
    Translate one recorder op dict (``{"kind": ..., "payload": {...}}``)
    into a canonical :class:`KiCadPlotterOp` whose payload uses the
    ``kicad.plotter_ir.a0`` field names.

    Unknown op kinds round-trip with their raw kind string preserved
    (matching :class:`KiCadPlotterOp` forward-compat behaviour).
    """
    if not isinstance(raw_op, dict):
        raise TypeError(f"recorder op must be a dict, got {type(raw_op).__name__}")

    kind = str(raw_op.get("kind", ""))
    payload = copy.deepcopy(raw_op.get("payload") or {})

    if kind == "PenTo" and "plume" in payload:
        payload["action"] = payload.pop("plume")
    elif kind == "PlotImage":
        width_px = payload.get("width_px")
        height_px = payload.get("height_px")
        scale_factor = payload.get("scale_factor")
        if (
            width_px is not None
            and height_px is not None
            and scale_factor is not None
        ):
            try:
                payload.setdefault("width_nm", float(width_px) * float(scale_factor))
                payload.setdefault("height_nm", float(height_px) * float(scale_factor))
            except (TypeError, ValueError):
                pass
    elif kind == "SetDash":
        if "width_nm" in payload and "line_width_nm" not in payload:
            payload["line_width_nm"] = payload.pop("width_nm")
        if "line_style" in payload:
            payload["line_style"] = _normalise_line_style(payload["line_style"])
    elif kind == "StartPlot" and "page_number" in payload:
        payload["page_name"] = payload.pop("page_number")
    elif kind == "SetPageSettings" and "type" in payload:
        payload["page_type"] = payload.pop("type")

    return KiCadPlotterOp(kind=_coerce_kind(kind), payload=payload)


# =============================================================================
# Canvas translation
# =============================================================================


def translate_recorder_canvas(raw_canvas: Any) -> dict[str, Any] | None:
    """
    Translate the recorder ``canvas`` block into the IR ``canvas``
    field. The recorder uses ``type`` (mirroring ``PAGE_INFO::GetType()``);
    the IR uses ``page_type`` to match the ``SetPageSettings`` op.
    """
    if not isinstance(raw_canvas, dict):
        return None

    out = copy.deepcopy(raw_canvas)
    if "type" in out and "page_type" not in out:
        out["page_type"] = out.pop("type")
    return out


# =============================================================================
# Document loaders
# =============================================================================


def load_recorder_dict(
    data: dict[str, Any],
    *,
    source_path: str | Path | None = None,
    document_id: str | None = None,
) -> KiCadPlotterDocument:
    """
    Translate a parsed ``kicad.plotter_recorder.v1`` dict into a
    :class:`KiCadPlotterDocument` containing a single
    ``"recorder_dump"`` record holding all translated ops in order.

    The recorder schema is flat (one ops list per dump file); we wrap
    those ops into one synthetic record so downstream tooling that
    iterates ``doc.records[].operations[]`` works without special-case
    code.
    """
    if not isinstance(data, dict):
        raise TypeError(f"recorder payload must be a dict, got {type(data).__name__}")

    schema = str(data.get("schema", "")).strip()
    if schema != KICAD_PLOTTER_RECORDER_SCHEMA:
        raise ValueError(
            f"Unexpected recorder schema: {schema!r} "
            f"(expected {KICAD_PLOTTER_RECORDER_SCHEMA!r})"
        )

    ops_data = data.get("ops") or []
    if not isinstance(ops_data, list):
        raise ValueError("recorder 'ops' must be a list")

    translated_ops = [
        translate_recorder_op(op)
        for op in ops_data
        if isinstance(op, dict)
    ]
    operations, nm_per_internal_unit = normalise_recorder_op_units(translated_ops)

    canvas = translate_recorder_canvas(data.get("canvas"))

    source_str = str(source_path) if source_path is not None else None
    object_id = document_id
    if object_id is None and source_str:
        object_id = Path(source_str).stem
    if object_id is None:
        object_id = "recorder_dump"

    record = KiCadPlotterRecord(
        uuid="",
        kind="recorder_dump",
        object_id=object_id,
        operations=operations,
    )

    return KiCadPlotterDocument(
        records=[record],
        source_path=source_str,
        source_kind="SCH",
        document_id=document_id,
        canvas=canvas,
        coordinate_space={"unit": "nm", "y_axis": "down"},
        extras={
            "recorder_units": {
                "source_unit": "plotter_internal",
                "nm_per_internal_unit": nm_per_internal_unit,
            }
        }
        if nm_per_internal_unit is not None
        else {},
    )


def load_recorder_file(path: str | Path) -> KiCadPlotterDocument:
    """
    Load and translate a ``kicad.plotter_recorder.v1`` JSON file.
    """
    file_path = Path(path)
    raw = json.loads(file_path.read_text(encoding="utf-8-sig"))
    return load_recorder_dict(raw, source_path=file_path, document_id=file_path.stem)


__all__ = [
    "KICAD_PLOTTER_RECORDER_SCHEMA",
    "load_recorder_dict",
    "load_recorder_file",
    "normalise_recorder_op_units",
    "translate_recorder_canvas",
    "translate_recorder_op",
]

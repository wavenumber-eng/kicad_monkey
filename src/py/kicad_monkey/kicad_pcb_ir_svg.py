"""
KiCad PCB to IR to SVG renderer.

This is the public board SVG path. It composes:

    pcb_to_ir(pcb) → KiCadPlotterDocument
    compute_pcb_svg_bounding_box(pcb, None) → mm bounding box (all layers)
    KiCadSvgRenderContext(sheet_*, offset_*) → translates content so the
        bbox origin lands at user (0, 0)
    render_ir_to_svg(doc, ctx=ctx) → final SVG document

Coordinate conventions:
* IR records are emitted in nm with Y-down, matching ``coordinate_space``
  on documents produced by :func:`pcb_to_ir`.
* Bounding boxes from :func:`compute_pcb_svg_bounding_box` are in mm, so we
  scale by ``mm_to_nm`` before populating the context.
* The viewBox semantics are ``0 0 width height`` in user units (mm by default
  via :class:`KiCadSvgRenderOptions`).

Layer filtering is record-granular: a record is kept when any of its
layer-bearing extras or ops intersects the requested set, and otherwise
dropped. Surviving multi-layer records also get per-op layer filtering
(e.g. footprints whose ops are tagged with their own layers via
``_op_with_pcb_layer``) so a ``layers=["F.SilkS"]`` request does not drag
F.Fab fp_lines into the output. Block markers and ops without any layer
metadata pass through.
"""

from __future__ import annotations

import math
from typing import TYPE_CHECKING, Iterable, Optional, Sequence

if TYPE_CHECKING:
    from .kicad_pcb import KiCadPcb
    from .kicad_plotter_ir import KiCadPlotterRecord
    from .kicad_sch_svg_renderer import KiCadSvgRenderOptions


# Stroke width used for synthesized drill outlines on non-copper layers.
# Matches KiCad's documentation-layer drill-outline convention (0.1 mm).
_DRILL_OUTLINE_STROKE_MM = 0.1

_LAYER_ALIASES: dict[str, tuple[str, ...]] = {
    "Cmts.User": ("User.Comments",),
    "User.Comments": ("Cmts.User",),
    "Dwgs.User": ("User.Drawings",),
    "User.Drawings": ("Dwgs.User",),
    "Eco1.User": ("User.Eco1",),
    "User.Eco1": ("Eco1.User",),
    "Eco2.User": ("User.Eco2",),
    "User.Eco2": ("Eco2.User",),
}


def _is_copper_layer(layer: str) -> bool:
    """Cheap copper-layer check (handles F.Cu, B.Cu, InN.Cu)."""
    return layer.endswith(".Cu")


def _is_mask_layer(layer: str) -> bool:
    return layer.endswith(".Mask")


def _record_layer_set(record: "KiCadPlotterRecord") -> set[str]:
    """Collect every layer name a record touches (extras + ops)."""

    layers: set[str] = set()
    extras = record.extras or {}

    single = extras.get("layer")
    if isinstance(single, str) and single:
        layers.add(single)

    for key in ("layers", "fill_layers"):
        value = extras.get(key)
        if isinstance(value, (list, tuple)):
            for entry in value:
                if isinstance(entry, str) and entry:
                    layers.add(entry)

    for op in record.operations or ():
        payload = op.payload or {}
        op_single = payload.get("layer")
        if isinstance(op_single, str) and op_single:
            layers.add(op_single)
        op_multi = payload.get("layers")
        if isinstance(op_multi, (list, tuple)):
            for entry in op_multi:
                if isinstance(entry, str) and entry:
                    layers.add(entry)

    return layers


def _op_layer_set(op) -> set[str]:
    """Collect every layer name an op declares via its payload."""

    payload = getattr(op, "payload", None) or {}
    layers: set[str] = set()
    single = payload.get("layer")
    if isinstance(single, str) and single:
        layers.add(single)
    multi = payload.get("layers")
    if isinstance(multi, (list, tuple)):
        for entry in multi:
            if isinstance(entry, str) and entry:
                layers.add(entry)
    return layers


def _layer_matches_wanted(declared_layer: str, wanted: set[str]) -> bool:
    """Test whether a single declared layer name reaches any wanted layer.

    Handles the KiCad wildcards that show up on pad/via layer lists:
    ``*.Cu`` matches any ``.Cu`` layer, ``*.Mask`` matches any ``.Mask``
    layer, and ``F&B.Cu`` matches both ``F.Cu`` and ``B.Cu``.
    """

    if declared_layer in wanted:
        return True
    if any(alias in wanted for alias in _LAYER_ALIASES.get(declared_layer, ())):
        return True
    if declared_layer == "*.Cu":
        return any(w.endswith(".Cu") for w in wanted)
    if declared_layer == "*.Mask":
        return any(w.endswith(".Mask") for w in wanted)
    if declared_layer == "F&B.Cu":
        return "F.Cu" in wanted or "B.Cu" in wanted
    return False


def _copper_layer_index(layer: str) -> int | None:
    if layer == "F.Cu":
        return 0
    if layer == "B.Cu":
        return 10_000
    if layer.startswith("In") and layer.endswith(".Cu"):
        inner = layer[2:-3]
        if inner.isdigit():
            return int(inner)
    return None


def _layer_set_matches_wanted(
    declared_layers: set[str],
    wanted: set[str],
    *,
    allow_copper_span: bool = False,
) -> bool:
    if any(_layer_matches_wanted(layer, wanted) for layer in declared_layers):
        return True
    if not allow_copper_span:
        return False
    copper_indices = [
        index for layer in declared_layers
        if (index := _copper_layer_index(layer)) is not None
    ]
    if len(copper_indices) < 2:
        return False
    low = min(copper_indices)
    high = max(copper_indices)
    return any(
        (wanted_index := _copper_layer_index(layer)) is not None
        and low <= wanted_index <= high
        for layer in wanted
    )


def _filter_record_ops_by_layer(
    record: "KiCadPlotterRecord", wanted: set[str]
) -> "KiCadPlotterRecord":
    """Drop ops inside ``record`` whose declared layer set misses ``wanted``.

    Ops without any layer metadata (block markers, sheet-level meta ops)
    pass through. Records with no operations are returned unchanged.
    """

    ops = record.operations or ()
    if not ops:
        return record
    fill_layers = (record.extras or {}).get("fill_layers")
    if isinstance(fill_layers, list) and fill_layers:
        from dataclasses import replace

        fill_count = min(len(fill_layers), len(ops))
        kept_indexes = [
            index
            for index in range(fill_count)
            if _layer_matches_wanted(str(fill_layers[index]), wanted)
        ]
        tail_extras = dict(record.extras or {})
        tail_extras.pop("fill_layers", None)
        tail_extras.pop("fill_island", None)
        tail_record = replace(
            record,
            operations=list(ops[fill_count:]),
            extras=tail_extras,
        )
        tail = _filter_record_ops_by_layer(tail_record, wanted).operations
        new_extras = dict(record.extras or {})
        new_extras["fill_layers"] = [fill_layers[index] for index in kept_indexes]
        fill_island = new_extras.get("fill_island")
        if isinstance(fill_island, list):
            new_extras["fill_island"] = [
                fill_island[index]
                for index in kept_indexes
                if index < len(fill_island)
            ]
        return replace(
            record,
            operations=[ops[index] for index in kept_indexes] + list(tail),
            extras=new_extras,
        )
    new_ops = []
    changed = False
    operation_index = 0
    while operation_index < len(ops):
        op = ops[operation_index]
        kind = _plotter_operation_kind(op)
        if kind == "StartBlock":
            if (
                operation_index + 2 < len(ops)
                and _plotter_operation_kind(ops[operation_index + 2]) == "EndBlock"
            ):
                inner = ops[operation_index + 1]
                inner_layers = _op_layer_set(inner)
                role = str((getattr(inner, "payload", None) or {}).get("role", ""))
                allow_copper_span = role in {"via_aperture", "via_drill"}
                if (
                    role == "npth_hole"
                    or not inner_layers
                    or _layer_set_matches_wanted(
                        inner_layers,
                        wanted,
                        allow_copper_span=allow_copper_span,
                    )
                ):
                    new_ops.extend(ops[operation_index : operation_index + 3])
                else:
                    changed = True
                operation_index += 3
                continue
            new_ops.append(op)
            operation_index += 1
            continue
        if kind == "EndBlock":
            new_ops.append(op)
            operation_index += 1
            continue
        op_layers = _op_layer_set(op)
        role = str((getattr(op, "payload", None) or {}).get("role", ""))
        if role == "npth_hole":
            new_ops.append(op)
            operation_index += 1
            continue
        allow_copper_span = role in {"via_aperture", "via_drill"}
        if not op_layers or _layer_set_matches_wanted(
            op_layers,
            wanted,
            allow_copper_span=allow_copper_span,
        ):
            new_ops.append(op)
        else:
            changed = True
        operation_index += 1
    if not changed:
        return record
    from dataclasses import replace

    return replace(record, operations=new_ops)


def _plotter_operation_kind(operation: object) -> str:
    kind = getattr(operation, "kind", "")
    return str(getattr(kind, "value", kind))


def _record_has_npth_hole(record: "KiCadPlotterRecord") -> bool:
    return any(
        str((getattr(op, "payload", None) or {}).get("role", "")) == "npth_hole"
        for op in record.operations or ()
    )


def _filter_records_by_layer(
    records: Iterable["KiCadPlotterRecord"],
    layers: Optional[Sequence[str]],
) -> list["KiCadPlotterRecord"]:
    """Keep records whose layer set intersects ``layers``.

    Records that don't expose any layer info (sheet headers, pure
    transforms) pass through unconditionally. When ``layers`` is ``None``
    every record is kept.

    Surviving records additionally have their internal ops filtered so
    multi-layer records (footprints whose fp_lines / fp_texts span multiple
    layers) only emit ops on the requested layers.
    """

    if layers is None:
        return list(records)

    wanted = {layer for layer in layers if isinstance(layer, str) and layer}
    if not wanted:
        return list(records)

    kept: list["KiCadPlotterRecord"] = []
    for record in records:
        record_layers = _record_layer_set(record)
        if _record_has_npth_hole(record) or not record_layers or _layer_set_matches_wanted(
            record_layers,
            wanted,
            allow_copper_span=(record.kind == "via"),
        ):
            kept.append(_filter_record_ops_by_layer(record, wanted))
    return kept


def _synthesize_pad_drill_outlines_for_layer(
    pcb: "KiCadPcb", layer: str
) -> list["KiCadPlotterRecord"]:
    """Synthesize pad drill outline records for one non-copper/non-mask layer.

    Mirrors ``kicad-cli pcb export svg --drill-shape-opt 2`` behaviour:
    each through-hole pad's drill aperture is emitted as a stroked outline
    on the requested layer. Vias are intentionally excluded — kicad-cli
    does not emit via drill outlines on Edge.Cuts / silkscreen / fab.

    Oval drills → thick segment of length (major - minor) and width minor
    (a pill shape). Round drills → stroked circle of diameter ``drill``.
    """

    from .kicad_base import PadType
    from .kicad_lib_symbol_to_ir import mm_to_nm
    from .kicad_plotter_ir import (
        KiCadFillType,
        KiCadPlotterOp,
        KiCadPlotterRecord,
    )

    records: list[KiCadPlotterRecord] = []
    for fp in getattr(pcb, "footprints", None) or []:
        fp_angle = float(getattr(fp, "at_angle", 0.0) or 0.0)
        fp_x = float(getattr(fp, "at_x", 0.0) or 0.0)
        fp_y = float(getattr(fp, "at_y", 0.0) or 0.0)
        fp_rad = math.radians(fp_angle)
        fp_cos = math.cos(fp_rad)
        fp_sin = math.sin(fp_rad)
        for pad in getattr(fp, "pads", None) or []:
            pad_type_value = (
                pad.pad_type.value if isinstance(pad.pad_type, PadType) else str(pad.pad_type)
            )
            if pad_type_value != PadType.THRU_HOLE.value:
                continue

            pad_local_x = float(pad.at_x)
            pad_local_y = float(pad.at_y)
            offset_x = float(pad.drill_offset_x or 0.0)
            offset_y = float(pad.drill_offset_y or 0.0)
            if offset_x or offset_y:
                pa = math.radians(float(pad.at_angle))
                ox = offset_x * math.cos(pa) - offset_y * math.sin(pa)
                oy = offset_x * math.sin(pa) + offset_y * math.cos(pa)
                pad_local_x += ox
                pad_local_y += oy

            abs_x = fp_x + pad_local_x * fp_cos - pad_local_y * fp_sin
            abs_y = fp_y + pad_local_x * fp_sin + pad_local_y * fp_cos

            has_oval_drill = (
                bool(getattr(pad, "drill_oval", False))
                and getattr(pad, "drill_width", None)
                and getattr(pad, "drill_height", None)
                and pad.drill_width > 0
                and pad.drill_height > 0
            )

            outline_op: Optional["KiCadPlotterOp"] = None
            if has_oval_drill:
                major = max(pad.drill_width, pad.drill_height)
                minor = min(pad.drill_width, pad.drill_height)
                if major <= 0 or minor <= 0:
                    continue
                total_orient = fp_angle + float(pad.at_angle)
                theta = math.radians(-total_orient)
                if pad.drill_height > pad.drill_width:
                    theta += math.pi / 2.0
                half_length = (major - minor) / 2.0
                dx = math.cos(theta) * half_length
                dy = math.sin(theta) * half_length
                if abs(dx) < 1e-9 and abs(dy) < 1e-9:
                    outline_op = KiCadPlotterOp.circle(
                        cx=mm_to_nm(abs_x),
                        cy=mm_to_nm(abs_y),
                        diameter_nm=mm_to_nm(minor),
                        fill=KiCadFillType.NO_FILL,
                        width_nm=mm_to_nm(_DRILL_OUTLINE_STROKE_MM),
                    )
                else:
                    outline_op = KiCadPlotterOp.thick_segment(
                        start_x=mm_to_nm(abs_x - dx),
                        start_y=mm_to_nm(abs_y - dy),
                        end_x=mm_to_nm(abs_x + dx),
                        end_y=mm_to_nm(abs_y + dy),
                        width_nm=mm_to_nm(minor),
                    )
            else:
                drill = getattr(pad, "drill", None)
                if drill and drill > 0:
                    outline_op = KiCadPlotterOp.circle(
                        cx=mm_to_nm(abs_x),
                        cy=mm_to_nm(abs_y),
                        diameter_nm=mm_to_nm(drill),
                        fill=KiCadFillType.NO_FILL,
                        width_nm=mm_to_nm(_DRILL_OUTLINE_STROKE_MM),
                    )
            if outline_op is None:
                continue

            uuid = (pad.uuid or "") + f":drill_outline:{layer}"
            records.append(
                KiCadPlotterRecord(
                    uuid=uuid,
                    kind="pad_drill_outline",
                    object_id="pad_drill_outline",
                    operations=[outline_op],
                    extras={"layer": layer},
                )
            )
    return records


def render_pcb_ir_to_svg(
    pcb: "KiCadPcb",
    *,
    layers: Optional[Sequence[str]] = None,
    fill: str = "#000000",
    stroke: str = "#000000",
    black_and_white: bool = True,
    profile: str | None = None,
    options: "Optional[KiCadSvgRenderOptions]" = None,
) -> str:
    """Render a :class:`KiCadPcb` to SVG via the plotter-IR pipeline.

    Always uses the all-layer bounding box for viewBox sizing. When
    ``layers`` is provided, records whose layer set does not intersect the
    requested set are dropped before rendering.
    """

    from dataclasses import replace

    from .kicad_ir_to_svg import render_ir_to_svg
    from .kicad_lib_symbol_to_ir import mm_to_nm
    from .kicad_pcb_bounds import compute_pcb_svg_bounding_box, empty_pcb_svg
    from .kicad_pcb_svg_enrichment import (
        pcb_root_svg_attrs,
        pcb_svg_enrichment_metadata_element,
        pcb_svg_enrichment_payload,
    )
    from .kicad_pcb_to_ir import pcb_to_ir
    from .kicad_sch_svg_renderer import (
        KiCadSvgRenderContext,
        KiCadSvgRenderOptions,
        KiCadSvgRenderProfile,
    )

    bbox = compute_pcb_svg_bounding_box(pcb, None)
    if bbox.is_empty:
        return empty_pcb_svg()

    min_x_nm = mm_to_nm(bbox.min_x)
    min_y_nm = mm_to_nm(bbox.min_y)
    width_nm = mm_to_nm(bbox.width)
    height_nm = mm_to_nm(bbox.height)

    base_opts = options if options is not None else KiCadSvgRenderOptions()
    resolved_profile = (
        KiCadSvgRenderProfile(profile)
        if profile is not None
        else base_opts.profile
    )
    opts = replace(
        base_opts,
        black_and_white=black_and_white,
        default_fill_color=fill,
        default_stroke_color=stroke,
        visible_layers=tuple(layers) if layers is not None else None,
        profile=resolved_profile,
    )
    ctx = KiCadSvgRenderContext(
        sheet_width_nm=width_nm,
        sheet_height_nm=height_nm,
        offset_x_nm=-min_x_nm,
        offset_y_nm=-min_y_nm,
        options=opts,
    )

    source_path = getattr(pcb, "source_path", None)
    doc = pcb_to_ir(pcb, source_path=str(source_path) if source_path else None)
    if layers is not None:
        records = list(doc.records)
        # Synthesize pad drill outlines once for a combined documentation-layer
        # render, matching ``kicad-cli --drill-shape-opt 2`` without
        # duplicating the same hole for F.SilkS + Edge.Cuts views.
        includes_copper_or_mask = any(
            layer and (_is_copper_layer(layer) or _is_mask_layer(layer))
            for layer in layers
        )
        first_drill_outline_layer = None
        if not includes_copper_or_mask:
            first_drill_outline_layer = next(
                (
                    layer for layer in layers
                    if layer
                    and not _is_copper_layer(layer)
                    and not _is_mask_layer(layer)
                ),
                None,
            )
        if first_drill_outline_layer is not None:
            records.extend(
                _synthesize_pad_drill_outlines_for_layer(
                    pcb, first_drill_outline_layer
                )
            )
        filtered = _filter_records_by_layer(records, layers)
        doc = replace(doc, records=filtered)

    profile_value = (
        resolved_profile.value
        if isinstance(resolved_profile, KiCadSvgRenderProfile)
        else str(resolved_profile)
    )
    root_attrs = None
    metadata_elements = None
    if profile_value != KiCadSvgRenderProfile.ORACLE.value:
        payload = pcb_svg_enrichment_payload(
            pcb,
            layers=layers,
            bbox=bbox,
            profile=profile_value,
        )
        root_attrs = pcb_root_svg_attrs(
            pcb,
            layers=layers,
            profile=profile_value,
        )
        metadata_elements = [pcb_svg_enrichment_metadata_element(payload)]

    return render_ir_to_svg(
        doc,
        ctx=ctx,
        root_extra_attrs=root_attrs,
        metadata_elements=metadata_elements,
    )


__all__ = ["render_pcb_ir_to_svg"]

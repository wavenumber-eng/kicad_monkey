"""No-fallback native provider for Cruncher's physical PCB SVG base."""

from __future__ import annotations

import hashlib
import os
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from kicad_monkey import KiCadPlotterDocument
    from kicad_monkey.kicad_pcb_bounds import BoundingBox

_SVG_NS = "http://www.w3.org/2000/svg"
_NM_TO_MM = "0.000001"
_DRILL_ROLES = {"pad_drill", "via_drill", "npth_hole", "via_mask_drill"}


@dataclass(frozen=True, slots=True)
class PhysicalSvgProvenance:
    """Validated provenance for one native physical SVG render."""

    backend: str
    engine_version: str
    profile: str
    document_id: str
    document_sha256: str
    svg_bytes: int
    svg_sha256: str


@dataclass(frozen=True, slots=True)
class PhysicalSvgArtifact:
    """A physical base SVG plus its native provenance."""

    svg_text: str
    provenance: PhysicalSvgProvenance


class PhysicalSvgProvider(Protocol):
    """Cruncher-owned provider boundary for physical PCB base SVG."""

    def render_pcb_root(
        self,
        document: KiCadPlotterDocument,
        bbox: BoundingBox,
    ) -> PhysicalSvgArtifact: ...


@dataclass(frozen=True, slots=True)
class NativePhysicalProvider:
    """Render a strict board Plotter-IR document through Monkey's native sidecar.

    This provider never catches a native failure and never retries through the
    Python renderer.  Platform selection happens before rendering begins.
    """

    executable: Path | str | None = None
    timeout: float = 120.0

    def render_pcb_root(
        self,
        document: KiCadPlotterDocument,
        bbox: BoundingBox,
    ) -> PhysicalSvgArtifact:
        from kicad_monkey import mm_to_nm, native_render_svg

        document_payload = _strict_wire_value(document.to_dict())
        if not isinstance(document_payload, dict):
            raise TypeError("native physical PCB document must project to an object")
        document_id = document_payload.get("document_id")
        if not isinstance(document_id, str) or not document_id:
            raise ValueError("native physical PCB document_id must be nonempty")
        viewport = {
            "min_x_nm": mm_to_nm(float(bbox.min_x)),
            "min_y_nm": mm_to_nm(float(bbox.min_y)),
            "width_nm": mm_to_nm(float(bbox.width)),
            "height_nm": mm_to_nm(float(bbox.height)),
        }
        if viewport["width_nm"] <= 0 or viewport["height_nm"] <= 0:
            raise ValueError("native physical PCB viewport must be nonempty")

        result = native_render_svg(
            document_payload,
            document_kind="board",
            viewport=viewport,
            executable=self.executable,
            timeout=self.timeout,
        )
        normalized = _normalize_native_board_svg(result.svg_utf8, document, viewport)
        document_bytes = _canonical_document_bytes(document_payload)
        normalized_bytes = normalized.encode("utf-8")
        return PhysicalSvgArtifact(
            svg_text=normalized,
            provenance=PhysicalSvgProvenance(
                backend="kicad-monkey-native",
                engine_version=result.engine_version,
                profile="plotter-base-a0",
                document_id=document_id,
                document_sha256=hashlib.sha256(document_bytes).hexdigest(),
                svg_bytes=len(normalized_bytes),
                svg_sha256=hashlib.sha256(normalized_bytes).hexdigest(),
            ),
        )


def use_native_physical_provider() -> bool:
    """Return whether this platform is promoted to the native provider."""

    return sys.platform == "win32" or os.environ.get("KICAD_CRUNCHER_NATIVE_PHYSICAL") == "1"


def board_document_id(pcb: object) -> str:
    """Return a deterministic, current-state identity for a PCB document."""

    to_string = getattr(pcb, "to_string", None)
    if not callable(to_string):
        raise TypeError("PCB source does not provide to_string()")
    payload = str(to_string()).encode("utf-8")
    return f"pcb-sha256:{hashlib.sha256(payload).hexdigest()}"


def _canonical_document_bytes(document: dict[str, object]) -> bytes:
    import json

    return json.dumps(
        document,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _strict_wire_value(value: object) -> object:
    """Canonicalize integer-valued legacy floats for the strict Plotter contract."""

    if isinstance(value, float) and value.is_integer():
        return int(value)
    if isinstance(value, dict):
        return {
            str(key): _strict_wire_value(item)
            for key, item in value.items()
            if not str(key).startswith("_provider_")
        }
    if isinstance(value, list):
        return [_strict_wire_value(item) for item in value]
    if isinstance(value, tuple):
        return [_strict_wire_value(item) for item in value]
    return value


def _normalize_native_board_svg(
    svg_text: str,
    document: KiCadPlotterDocument,
    viewport: dict[str, int],
) -> str:
    """Normalize P6_010 nanometre output to Cruncher's millimetre topology."""

    root = ET.fromstring(svg_text)
    if _local_name(root.tag) != "svg":
        raise ValueError("native physical SVG root is invalid")
    children = list(root)
    if len(children) != 2 or _local_name(children[0].tag) != "rect":
        raise ValueError("native physical SVG base topology is invalid")
    viewport_group = children[1]
    if _local_name(viewport_group.tag) != "g":
        raise ValueError("native physical SVG viewport group is missing")
    record_groups = list(viewport_group)
    if len(record_groups) != len(document.records):
        raise ValueError("native physical SVG record topology does not match Plotter-IR")

    width_mm = _nm_as_mm(viewport["width_nm"])
    height_mm = _nm_as_mm(viewport["height_nm"])
    root.set("viewBox", f"0 0 {width_mm} {height_mm}")
    root.set("width", f"{width_mm}mm")
    root.set("height", f"{height_mm}mm")
    root.remove(viewport_group)
    viewport_transform = viewport_group.get("transform")
    expected_viewport_transform = (
        f'translate({-viewport["min_x_nm"]} {-viewport["min_y_nm"]})'
    )
    if viewport_transform != expected_viewport_transform:
        raise ValueError("native physical SVG viewport transform is invalid")

    background = children[0]
    background.set("transform", _prepend_scale(background.get("transform")))
    drill_overlays: list[ET.Element] = []
    for group, record in zip(record_groups, document.records, strict=True):
        _validate_native_record_group(group, record)
        group.set(
            "transform",
            _scaled_transform(viewport_transform, group.get("transform")),
        )
        overlay, normal_operations = _split_native_drill_group(group, record)
        _apply_record_enrichment(group, record, normal_operations)
        if overlay is not None:
            drill_overlays.append(overlay)
        root.append(group)
    root.extend(drill_overlays)
    return ET.tostring(root, encoding="unicode", short_empty_elements=True)


def _validate_native_record_group(group: ET.Element, record: object) -> None:
    if _local_name(group.tag) != "g":
        raise ValueError("native physical SVG record element is not a group")
    expected = {
        "id": str(getattr(record, "uuid", "") or ""),
        "data-ref": str(getattr(record, "kind", "") or ""),
        "data-object-id": str(getattr(record, "object_id", "") or ""),
    }
    for name, value in expected.items():
        actual = group.attrib.get(name, "")
        if actual != value:
            raise ValueError(f"native physical SVG record {name} does not match Plotter-IR")


def _apply_record_enrichment(
    element: ET.Element,
    record: object,
    operations: list[object],
) -> None:
    from kicad_monkey.kicad_pcb_svg_enrichment import pcb_record_svg_data_attrs

    attrs = pcb_record_svg_data_attrs(record, operations)
    extras = getattr(record, "extras", None) or {}
    if extras.get("_provider_hole_owner"):
        attrs.update(
            {
                "data-primitive": "pad-hole",
                "data-hole-owner": extras["_provider_hole_owner"],
                "data-hole-kind": extras["_provider_hole_kind"],
                "data-hole-plating": extras["_provider_hole_plating"],
                "data-hole-render": "drill",
            }
        )
    for name, value in attrs.items():
        if value is not None and str(value):
            element.set(str(name).replace("_", "-"), str(value))


def _split_native_drill_group(
    element: ET.Element,
    record: object,
) -> tuple[ET.Element | None, list[object]]:
    operations = list(getattr(record, "operations", ()) or ())
    if not _operations_have_drill_role(operations):
        return None, operations
    children = list(element)
    operation_groups = _drawable_operation_groups(operations)
    if len(children) != len(operation_groups):
        raise ValueError("native physical SVG operation topology does not match Plotter-IR")
    drill_pairs, normal_operations = _partition_drill_groups(operation_groups, children)
    if not drill_pairs:
        return None, operations
    overlay, drill_operations = _build_drill_overlay(element, record, drill_pairs)
    _enrich_drill_overlay(overlay, record, drill_operations)
    return overlay, normal_operations


def _operations_have_drill_role(operations: list[object]) -> bool:
    return any(_operation_role(operation) in _DRILL_ROLES for operation in operations)


def _operation_role(operation: object) -> str:
    payload = getattr(operation, "payload", None) or {}
    return str(payload.get("role", ""))


def _partition_drill_groups(
    operation_groups: list[list[object]],
    children: list[ET.Element],
) -> tuple[list[tuple[list[object], ET.Element]], list[object]]:
    drill_pairs: list[tuple[list[object], ET.Element]] = []
    normal_operations: list[object] = []
    for operation_group, child in zip(operation_groups, children, strict=True):
        if _operation_group_is_drill(operation_group):
            drill_pairs.append((operation_group, child))
        else:
            normal_operations.extend(operation_group)
    return drill_pairs, normal_operations


def _operation_group_is_drill(operation_group: list[object]) -> bool:
    draw_operations = [
        operation
        for operation in operation_group
        if _operation_kind(operation) not in {"StartBlock", "EndBlock"}
    ]
    return bool(draw_operations) and {
        _operation_role(operation) for operation in draw_operations
    } <= _DRILL_ROLES


def _build_drill_overlay(
    element: ET.Element,
    record: object,
    drill_pairs: list[tuple[list[object], ET.Element]],
) -> tuple[ET.Element, list[object]]:
    overlay = ET.Element(
        f"{{{_SVG_NS}}}g",
        {"transform": element.get("transform", "")},
    )
    uuid = str(getattr(record, "uuid", "") or "")
    if uuid:
        overlay.set("id", f"{uuid}:drill_overlay")
        overlay.set("data-uuid", f"{uuid}:drill_overlay")
    overlay.set("data-ref", "drill_overlay")
    overlay.set("data-object-id", str(getattr(record, "object_id", "")))
    drill_operations: list[object] = []
    for operation_group, child in drill_pairs:
        element.remove(child)
        overlay.append(child)
        drill_operations.extend(operation_group)
    return overlay, drill_operations


def _enrich_drill_overlay(
    overlay: ET.Element,
    record: object,
    drill_operations: list[object],
) -> None:
    from kicad_monkey.kicad_pcb_svg_enrichment import pcb_record_svg_data_attrs

    attrs = pcb_record_svg_data_attrs(
        record,
        drill_operations,
        data_ref="drill_overlay",
    )
    for name, value in attrs.items():
        if value is not None and str(value):
            overlay.set(str(name).replace("_", "-"), str(value))


def _drawable_operation_groups(operations: list[object]) -> list[list[object]]:
    groups: list[list[object]] = []
    operation_index = 0
    while operation_index < len(operations):
        operation = operations[operation_index]
        if _operation_kind(operation) != "StartBlock":
            if _operation_kind(operation) == "EndBlock":
                raise ValueError("native physical SVG has an orphan EndBlock")
            groups.append([operation])
            operation_index += 1
            continue
        depth = 1
        cursor = operation_index + 1
        while cursor < len(operations) and depth:
            kind = _operation_kind(operations[cursor])
            if kind == "StartBlock":
                depth += 1
            elif kind == "EndBlock":
                depth -= 1
            cursor += 1
        if depth:
            raise ValueError("native physical SVG has an unclosed StartBlock")
        groups.append(operations[operation_index:cursor])
        operation_index = cursor
    return groups


def _operation_kind(operation: object) -> str:
    kind = getattr(operation, "kind", "")
    value = getattr(kind, "value", kind)
    return str(value)


def _prepend_scale(transform: str | None) -> str:
    return f"scale({_NM_TO_MM})" if not transform else f"scale({_NM_TO_MM}) {transform}"


def _scaled_transform(viewport: str, record: str | None) -> str:
    return f"scale({_NM_TO_MM}) {viewport}" if not record else (
        f"scale({_NM_TO_MM}) {viewport} {record}"
    )


def _nm_as_mm(value: int) -> str:
    sign = "-" if value < 0 else ""
    digits = str(abs(value))
    whole = digits[:-6] or "0"
    fraction = digits[-6:].rjust(6, "0").rstrip("0")
    return f"{sign}{whole}.{fraction}" if fraction else f"{sign}{whole}"


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


ET.register_namespace("", _SVG_NS)


__all__ = [
    "NativePhysicalProvider",
    "PhysicalSvgArtifact",
    "PhysicalSvgProvenance",
    "PhysicalSvgProvider",
    "board_document_id",
    "use_native_physical_provider",
]

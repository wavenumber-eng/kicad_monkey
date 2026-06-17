"""Lightweight footprint metadata filters.

These filters operate directly on parsed footprint s-expressions and avoid the
geometry dependencies required by the 3D/outline footprint filters.
"""

from __future__ import annotations

from typing import Any

from .kicad_base import unquote_string


FOOTPRINT_LIBRARY_PROPERTIES_TO_KEEP = {"Reference", "Value"}
FOOTPRINT_INSTANCE_ATTRS_TO_REMOVE = {
    "dnp",
    "exclude_from_bom",
    "exclude_from_pos_files",
}


def fp_filter__remove_schematic_inherited_metadata(
    unfiltered_s_expression: Any,
    *,
    keep_properties: set[str] | None = None,
) -> Any:
    """Remove schematic/board-instance metadata from a footprint library item.

    Board footprints can carry symbol-derived fields such as manufacturer data,
    ``cad-reference``, Datasheet, Description, and ``ki_fp_filters``.  Those are
    part-instance/library-management metadata, not reusable footprint geometry.
    Keep only the KiCad footprint convention fields by default: Reference and
    Value.  PTH pad settings such as ``remove_unused_layers`` are intentionally
    untouched.
    """
    keep = FOOTPRINT_LIBRARY_PROPERTIES_TO_KEEP if keep_properties is None else keep_properties
    for index in range(len(unfiltered_s_expression) - 1, -1, -1):
        elem = unfiltered_s_expression[index]
        if not isinstance(elem, list) or not elem:
            continue
        if elem[0] == "property" and len(elem) >= 2:
            name = unquote_string(elem[1])
            if name not in keep:
                unfiltered_s_expression.pop(index)
            continue
        if elem[0] == "attr":
            filtered = [
                token for token in elem[1:]
                if unquote_string(token) not in FOOTPRINT_INSTANCE_ATTRS_TO_REMOVE
            ]
            if filtered:
                elem[:] = ["attr", *filtered]
            else:
                unfiltered_s_expression.pop(index)
    return unfiltered_s_expression


__all__ = [
    "FOOTPRINT_INSTANCE_ATTRS_TO_REMOVE",
    "FOOTPRINT_LIBRARY_PROPERTIES_TO_KEEP",
    "fp_filter__remove_schematic_inherited_metadata",
]

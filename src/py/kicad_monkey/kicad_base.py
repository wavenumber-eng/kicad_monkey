"""
KiCad Base - Enums, utilities, and helper functions.

This module provides the foundational types and utilities used across
all KiCad file parsers (PCB, footprint, symbol, schematic).

KiCad Source Reference:
    Version: 9.0.0-rc3-4364-g5f555f4d63
    Commit: 5f555f4d63b970e410d567d1f79e05e8ce41b9d8
    Date: 2025-11-27
    Source: https://gitlab.com/kicad/code/kicad
    Key files referenced:
    - common/font/outline_font.cpp - Text rendering constants
    - libs/kimath/src/convert_basic_shapes_to_polygon.cpp - Shape conversion
    - include/font/outline_font.h - Font constants (OUTLINE_FONT_SIZE_COMPENSATION)
    - common/io/kicad/kicad_io_utils.cpp - S-expression formatting
    - pcbnew/pcb_text.cpp - Knockout text rendering
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable, Final, Iterator, List, Optional, Tuple, TYPE_CHECKING

# Import string classes and formatting from kicad_sexpr (canonical location)
from .kicad_sexpr import (
    QuotedString,
    FormattedDataBlock,
    format_float,
    quote_string,
    INDENT_CHAR,
    INDENT_SIZE,
    XY_COLUMN_LIMIT,
    TOKEN_WRAP_THRESHOLD,
    MIME_BASE64_LENGTH,
)

if TYPE_CHECKING:
    from .kicad_pcb_polygon_ops import PolygonSet


# =============================================================================
# Enums
# =============================================================================

class LayerType(Enum):
    SIGNAL = "signal"
    POWER = "power"
    MIXED = "mixed"
    JUMPER = "jumper"
    AUXILIARY = "auxiliary"
    USER = "user"


class StrokeType(Enum):
    SOLID = "solid"
    DASH = "dash"
    DOT = "dot"
    DASH_DOT = "dash_dot"
    DASH_DOT_DOT = "dash_dot_dot"
    DEFAULT = "default"


class FillType(Enum):
    NONE = "none"
    SOLID = "solid"
    YES = "yes"
    NO = "no"


class HAlign(Enum):
    LEFT = "left"
    CENTER = "center"
    RIGHT = "right"


class VAlign(Enum):
    TOP = "top"
    CENTER = "center"
    BOTTOM = "bottom"


class PadType(Enum):
    THRU_HOLE = "thru_hole"
    SMD = "smd"
    CONNECT = "connect"
    NP_THRU_HOLE = "np_thru_hole"


class PadShape(Enum):
    CIRCLE = "circle"
    RECT = "rect"
    OVAL = "oval"
    TRAPEZOID = "trapezoid"
    ROUNDRECT = "roundrect"
    CUSTOM = "custom"


class ZoneConnectionType(Enum):
    INHERITED = 0
    NONE = 1
    THERMAL = 2
    SOLID = 3


# -----------------------------------------------------------------------------
# Board Stackup Enums (KiCad 9.0)
# Reference: kicad/pcbnew/board_stackup_manager/board_stackup.h
# -----------------------------------------------------------------------------

class StackupItemType(Enum):
    """
    Type of layer in the board stackup.

    Reference: BOARD_STACKUP_ITEM_TYPE enum in board_stackup.h lines 42-52
    """
    UNDEFINED = "undefined"      # BS_ITEM_TYPE_UNDEFINED
    COPPER = "copper"            # BS_ITEM_TYPE_COPPER
    DIELECTRIC = "dielectric"    # BS_ITEM_TYPE_DIELECTRIC (core, prepreg)
    SOLDERPASTE = "solderpaste"  # BS_ITEM_TYPE_SOLDERPASTE
    SOLDERMASK = "soldermask"    # BS_ITEM_TYPE_SOLDERMASK
    SILKSCREEN = "silkscreen"    # BS_ITEM_TYPE_SILKSCREEN


class EdgeConnectorConstraint(Enum):
    """
    Edge connector fabrication constraints.

    Reference: BS_EDGE_CONNECTOR_CONSTRAINTS enum in board_stackup.h lines 55-60
    """
    NONE = "none"          # BS_EDGE_CONNECTOR_NONE - No edge connector
    IN_USE = "yes"         # BS_EDGE_CONNECTOR_IN_USE - Has edge connectors
    BEVELLED = "bevelled"  # BS_EDGE_CONNECTOR_BEVELLED - Bevelled edge connectors


# -----------------------------------------------------------------------------
# Zone Placement Enums (KiCad 9.0)
# Reference: kicad/pcbnew/zone_settings.h
# -----------------------------------------------------------------------------

class PlacementSourceType(Enum):
    """
    Source type for Placement Rule Areas (multi-channel design).

    Placement Rule Areas define regions where footprints with matching criteria
    are automatically placed together. Used for replicating multi-channel designs.

    Reference: PLACEMENT_SOURCE_T enum in zone_settings.h lines 77-83
    """
    SHEETNAME = "sheetname"            # Match footprints from schematic sheet path
    COMPONENT_CLASS = "component_class"  # Match footprints in component class
    GROUP_PLACEMENT = "group"          # Match footprints in named group
    # Note: DESIGN_BLOCK is transitory and not saved to file


# -----------------------------------------------------------------------------
# Common KiCad layer-name tokens
# -----------------------------------------------------------------------------

FRONT_COPPER_LAYER: Final[str] = "F.Cu"
BACK_COPPER_LAYER: Final[str] = "B.Cu"
FRONT_SILKSCREEN_LAYER: Final[str] = "F.SilkS"
BACK_SILKSCREEN_LAYER: Final[str] = "B.SilkS"
FRONT_MASK_LAYER: Final[str] = "F.Mask"
BACK_MASK_LAYER: Final[str] = "B.Mask"
FRONT_PASTE_LAYER: Final[str] = "F.Paste"
BACK_PASTE_LAYER: Final[str] = "B.Paste"
EDGE_CUTS_LAYER: Final[str] = "Edge.Cuts"


# =============================================================================
# Common Data Classes
# =============================================================================

# Import DEFAULT_ERROR_MM at runtime to avoid circular import
def _get_default_error() -> float:
    from .kicad_pcb_polygon_ops import DEFAULT_ERROR_MM
    return DEFAULT_ERROR_MM


@dataclass(slots=True)
class Stroke:
    """Stroke parameters for graphical elements."""
    width: float = 0.0
    type: StrokeType = StrokeType.DEFAULT
    color: Optional[Tuple[int, int, int, float]] = None


# =============================================================================
# ToPolyMixin - Adds polygon conversion to graphical elements
# =============================================================================

class ToPolyMixin(ABC):
    """
    Mixin that adds polygon conversion capabilities to PCB elements.

    Subclasses must implement _to_poly() to convert their geometry to polygons.
    The to_svg() method is provided automatically based on _to_poly() output.
    """

    @abstractmethod
    def _to_poly(self, error: float) -> "PolygonSet":
        """
        Convert element geometry to a polygon set.

        Args:
            error: Maximum approximation error for curves (in mm)

        Returns:
            PolygonSet containing the element's polygon representation
        """
        pass

    def to_svg(self, *args: Any, **kwargs: Any) -> Any:
        """
        Convert element to SVG path element.

        Args:
            error: Maximum approximation error for curves
            fill: SVG fill color (default: derived from element)
            stroke: SVG stroke color
            stroke_width: SVG stroke width
            **attrs: Additional SVG attributes

        Returns:
            SVG <path> element string
        """
        error = kwargs.pop("error", args[0] if args else None)
        fill = kwargs.pop("fill", None)
        stroke = kwargs.pop("stroke", None)
        stroke_width = kwargs.pop("stroke_width", None)
        attrs = kwargs

        if error is None:
            error = _get_default_error()
        poly = self._to_poly(error)

        if poly.is_empty():
            return ''

        path_data = poly.to_svg_path()

        # Build attributes
        attr_parts = [f'd="{path_data}"']

        if fill is not None:
            attr_parts.append(f'fill="{fill}"')
        else:
            attr_parts.append('fill="currentColor"')

        if stroke is not None:
            attr_parts.append(f'stroke="{stroke}"')

        if stroke_width is not None:
            attr_parts.append(f'stroke-width="{stroke_width}"')

        # Use evenodd fill rule for proper hole rendering
        attr_parts.append('fill-rule="evenodd"')

        for key, value in attrs.items():
            # Convert Python underscores to SVG hyphens
            svg_key = key.replace('_', '-')
            attr_parts.append(f'{svg_key}="{value}"')

        return f'<path {" ".join(attr_parts)}/>'

    def to_svg_group(
        self,
        error: float | None = None,
        **group_attrs: Any,
    ) -> str:
        """
        Wrap SVG output in a group element with attributes.

        Args:
            error: Maximum approximation error for curves
            **group_attrs: Attributes for the <g> element

        Returns:
            SVG <g> element containing the path
        """
        if error is None:
            error = _get_default_error()
        path = self.to_svg(error)
        if not path:
            return ''

        attr_parts = []
        for key, value in group_attrs.items():
            svg_key = key.replace('_', '-')
            attr_parts.append(f'{svg_key}="{value}"')

        if attr_parts:
            return f'<g {" ".join(attr_parts)}>{path}</g>'
        return f'<g>{path}</g>'


# =============================================================================
# S-Expression Utilities
# =============================================================================

def find_element(sexp: list, name: str) -> Optional[list]:
    """Find first sub-element by name."""
    if not isinstance(sexp, list):
        return None
    for elem in sexp:
        if isinstance(elem, list) and len(elem) > 0 and elem[0] == name:
            return elem
    return None


def find_all_elements(sexp: list, name: str) -> List[list]:
    """Find all sub-elements by name."""
    if not isinstance(sexp, list):
        return []
    return [elem for elem in sexp if isinstance(elem, list) and len(elem) > 0 and elem[0] == name]


def get_value(sexp: list, name: str, default: Any = None) -> Any:
    """Get value of named element (first value after the name)."""
    elem = find_element(sexp, name)
    if elem and len(elem) > 1:
        return elem[1]
    return default


def get_values(sexp: list, name: str) -> List[Any]:
    """Get all values of named element (all values after the name)."""
    elem = find_element(sexp, name)
    if elem and len(elem) > 1:
        return elem[1:]
    return []


def has_flag(sexp: list, name: str) -> bool:
    """Check if a flag (bare token) exists in the s-expression."""
    if not isinstance(sexp, list):
        return False
    return name in sexp


def parse_maybe_absent_bool(sexp: list, name: str, default_when_empty: bool = True) -> Optional[bool]:
    """Mirror KiCad's ``parseMaybeAbsentBool`` semantics for legacy/v10 flags.

    Returns ``None`` when the flag is absent, otherwise resolves the three
    forms KiCad has used across versions:

    - bare token  ``name``                    -> ``True`` (legacy)
    - empty list  ``(name)``                  -> ``default_when_empty``
    - sub-list    ``(name yes/no)``           -> the explicit boolean

    KiCad 10 emits the sub-list form via ``KICAD_FORMAT::FormatBool`` while
    its parser still accepts the empty form via ``parseMaybeAbsentBool``
    (see ``eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr_parser.cpp``).
    """
    if has_flag(sexp, name):
        return True
    elem = find_element(sexp, name)
    if elem is None:
        return None
    if len(elem) <= 1:
        return default_when_empty
    return elem[1] == 'yes'


def get_at(sexp: list) -> Tuple[float, float, float]:
    """Extract (x, y, angle) from 'at' element."""
    at_elem = find_element(sexp, 'at')
    if at_elem:
        x = float(at_elem[1]) if len(at_elem) > 1 else 0.0
        y = float(at_elem[2]) if len(at_elem) > 2 else 0.0
        angle = 0.0
        if len(at_elem) > 3:
            try:
                angle = float(at_elem[3])
            except (TypeError, ValueError):
                angle = 0.0
        return (x, y, angle)
    return (0.0, 0.0, 0.0)


# format_float and quote_string are imported from kicad_sexpr (see imports above)


def unquote_string(s: Any) -> str:
    """Get the string value, handling QuotedString."""
    if isinstance(s, QuotedString):
        return str(s)
    return str(s) if s is not None else ""


# =============================================================================
# S-Expression Mutation Primitives
# =============================================================================

def replace_element(sexp: list, name: str, new_elem: list) -> bool:
    """Replace the first child element matching ``name`` in place.

    Returns True if a replacement happened, False if no match was found.
    """
    if not isinstance(sexp, list):
        return False
    for i, elem in enumerate(sexp):
        if isinstance(elem, list) and len(elem) > 0 and elem[0] == name:
            sexp[i] = new_elem
            return True
    return False


def remove_element(sexp: list, name: str) -> Optional[list]:
    """Remove the first child element matching ``name`` in place.

    Returns the removed element or None if no match was found.
    """
    if not isinstance(sexp, list):
        return None
    for i, elem in enumerate(sexp):
        if isinstance(elem, list) and len(elem) > 0 and elem[0] == name:
            return sexp.pop(i)
    return None


def remove_all_elements(sexp: list, name: str) -> List[list]:
    """Remove every child element matching ``name`` in place.

    Returns the list of removed elements (in original order). Empty list if
    none were present.
    """
    if not isinstance(sexp, list):
        return []
    removed = []
    i = 0
    while i < len(sexp):
        elem = sexp[i]
        if isinstance(elem, list) and len(elem) > 0 and elem[0] == name:
            removed.append(sexp.pop(i))
        else:
            i += 1
    return removed


def set_value(sexp: list, name: str, value: Any) -> None:
    """Set ``(name value)`` on ``sexp`` in place — replaces if present, appends otherwise.

    Always emits the sub-list form ``[name, value]``. The bare-flag form
    (``name`` as a string token) is not used by this helper; callers that
    need bare flags should manipulate the list directly.
    """
    if not isinstance(sexp, list):
        raise TypeError(f"set_value: sexp must be a list, got {type(sexp).__name__}")
    new_elem = [name, value]
    if not replace_element(sexp, name, new_elem):
        sexp.append(new_elem)


def walk(sexp: Any) -> Iterator[list]:
    """Depth-first generator yielding every list node in the tree, including ``sexp`` itself.

    Skips non-list leaves (strings, numbers, QuotedString, etc.).
    """
    if isinstance(sexp, list):
        yield sexp
        for child in sexp:
            yield from walk(child)


def find_path(sexp: list, *names: str) -> Optional[list]:
    """Nested element lookup. ``find_path(pcb, 'setup', 'pcbplotparams')``
    returns the ``(pcbplotparams ...)`` element inside the ``(setup ...)``
    element of ``pcb``, or None if any segment is missing.
    """
    current = sexp
    for name in names:
        current = find_element(current, name)
        if current is None:
            return None
    return current


def transform_descendants(sexp: list, name: str, fn: Callable[[list], list]) -> int:
    """Depth-first replace-in-place: every descendant list whose first
    element matches ``name`` is passed to ``fn`` and replaced by its
    return value.

    ``fn`` receives the matched element; if it returns the same list object
    (mutated in place) or a new list, the parent slot is updated.

    Returns the number of replacements performed. Does not recurse into
    the replaced subtree (post-replace traversal stops at that node) to
    avoid loops if ``fn`` returns a tree containing further matches.
    """
    if not isinstance(sexp, list):
        return 0
    count = 0
    for i, child in enumerate(sexp):
        if isinstance(child, list) and len(child) > 0 and child[0] == name:
            sexp[i] = fn(child)
            count += 1
        elif isinstance(child, list):
            count += transform_descendants(child, name, fn)
    return count


# Re-export from kicad_sexpr for convenience
__all__ = [
    # Constants (from kicad_sexpr)
    'INDENT_CHAR',
    'INDENT_SIZE',
    'XY_COLUMN_LIMIT',
    'TOKEN_WRAP_THRESHOLD',
    'MIME_BASE64_LENGTH',
    # Enums
    'LayerType',
    'StrokeType',
    'FillType',
    'HAlign',
    'VAlign',
    'PadType',
    'PadShape',
    'ZoneConnectionType',
    'StackupItemType',
    'EdgeConnectorConstraint',
    'PlacementSourceType',
    'FRONT_COPPER_LAYER',
    'BACK_COPPER_LAYER',
    'FRONT_SILKSCREEN_LAYER',
    'BACK_SILKSCREEN_LAYER',
    'FRONT_MASK_LAYER',
    'BACK_MASK_LAYER',
    'FRONT_PASTE_LAYER',
    'BACK_PASTE_LAYER',
    'EDGE_CUTS_LAYER',
    # Classes
    'Stroke',
    'ToPolyMixin',
    # Utilities — read
    'find_element',
    'find_all_elements',
    'get_value',
    'get_values',
    'has_flag',
    'parse_maybe_absent_bool',
    'get_at',
    'format_float',
    'quote_string',
    'unquote_string',
    # Utilities — mutation primitives
    'replace_element',
    'remove_element',
    'remove_all_elements',
    'set_value',
    'walk',
    'find_path',
    'transform_descendants',
    # String classes (from kicad_sexpr)
    'QuotedString',
    'FormattedDataBlock',
]

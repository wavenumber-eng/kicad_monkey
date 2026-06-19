"""
GrLine - Graphical line segment element (gr_line)

Represents a line segment with stroke width, typically used for board outlines,
silkscreen graphics, and other layer-based graphics.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Optional

from .kicad_base import (
    EDGE_CUTS_LAYER,
    ToPolyMixin,
    Stroke,
    StrokeType,
    QuotedString,
    find_element,
    get_value,
    unquote_string,
)
from .kicad_pcb_polygon_ops import PolygonSet, oval_to_polygon, DEFAULT_ERROR_MM

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox


@dataclass
class GrLine(ToPolyMixin):
    """
    Graphical line element (gr_line).

    A line segment defined by start and end points with a stroke width.
    The polygon representation is a capsule (stadium) shape.
    """
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    angle: float = 0.0
    layer: str = EDGE_CUTS_LAYER
    stroke: Stroke = field(default_factory=Stroke)
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrLine':
        """Parse from s-expression."""
        start = find_element(sexp, 'start')
        end = find_element(sexp, 'end')

        start_x = float(start[1]) if start else 0.0
        start_y = float(start[2]) if start else 0.0
        end_x = float(end[1]) if end else 0.0
        end_y = float(end[2]) if end else 0.0

        angle = float(get_value(sexp, 'angle', 0.0))
        layer = unquote_string(get_value(sexp, 'layer', EDGE_CUTS_LAYER))

        # Parse stroke
        stroke_elem = find_element(sexp, 'stroke')
        if stroke_elem:
            width = float(get_value(stroke_elem, 'width', 0.0))
            type_str = get_value(stroke_elem, 'type', 'default')
            stroke_type = StrokeType(type_str) if type_str else StrokeType.DEFAULT
            stroke = Stroke(width=width, type=stroke_type)
        else:
            # Legacy format: width at top level
            width = float(get_value(sexp, 'width', 0.0))
            stroke = Stroke(width=width)

        uuid = unquote_string(get_value(sexp, 'uuid'))

        return cls(
            start_x=start_x, start_y=start_y,
            end_x=end_x, end_y=end_y,
            angle=angle,
            layer=layer,
            stroke=stroke,
            uuid=uuid,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        result = ['gr_line',
                  ['start', self.start_x, self.start_y],
                  ['end', self.end_x, self.end_y]]
        if self.angle != 0:
            result.append(['angle', self.angle])
        result.append(['stroke',
                      ['width', self.stroke.width],
                      ['type', self.stroke.type.value]])
        result.append(['layer', QuotedString(self.layer)])
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert line to polygon (capsule/stadium shape).

        The line is converted to an oval shape with semicircular ends.
        """
        width = self.stroke.width
        if width <= 0:
            # Zero-width line - no polygon
            return PolygonSet()

        start = (self.start_x, self.start_y)
        end = (self.end_x, self.end_y)

        contour = oval_to_polygon(start, end, width, error)
        return PolygonSet(outlines=[contour])

    def get_bounds(self) -> "BoundingBox":
        """Return KiCad-style source geometry bounds."""
        from .kicad_pcb_shape_bounds import segment_bounds

        return segment_bounds(self.start, self.end, self.stroke.width)

    @property
    def start(self) -> tuple[float, float]:
        """Get start point as tuple."""
        return (self.start_x, self.start_y)

    @property
    def end(self) -> tuple[float, float]:
        """Get end point as tuple."""
        return (self.end_x, self.end_y)

    @property
    def width(self) -> float:
        """Get stroke width."""
        return self.stroke.width

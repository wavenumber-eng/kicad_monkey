"""
GrArc - Graphical arc element (gr_arc)

Represents an arc defined by start, mid, and end points with stroke width.
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
from .kicad_pcb_polygon_ops import PolygonSet, arc_to_polygon, DEFAULT_ERROR_MM

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox


@dataclass
class GrArc(ToPolyMixin):
    """
    Graphical arc element (gr_arc).

    An arc defined by three points: start, mid (on the arc), and end.
    The polygon representation is a thick arc with semicircular end caps.
    """
    start_x: float
    start_y: float
    mid_x: float
    mid_y: float
    end_x: float
    end_y: float
    layer: str = EDGE_CUTS_LAYER
    stroke: Stroke = field(default_factory=Stroke)
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrArc':
        """Parse from s-expression."""
        start = find_element(sexp, 'start')
        mid = find_element(sexp, 'mid')
        end = find_element(sexp, 'end')

        # Parse stroke
        stroke_elem = find_element(sexp, 'stroke')
        if stroke_elem:
            width = float(get_value(stroke_elem, 'width', 0.0))
            type_str = get_value(stroke_elem, 'type', 'default')
            stroke_type = StrokeType(type_str) if type_str else StrokeType.DEFAULT
            stroke = Stroke(width=width, type=stroke_type)
        else:
            width = float(get_value(sexp, 'width', 0.0))
            stroke = Stroke(width=width)

        return cls(
            start_x=float(start[1]) if start else 0.0,
            start_y=float(start[2]) if start else 0.0,
            mid_x=float(mid[1]) if mid else 0.0,
            mid_y=float(mid[2]) if mid else 0.0,
            end_x=float(end[1]) if end else 0.0,
            end_y=float(end[2]) if end else 0.0,
            layer=unquote_string(get_value(sexp, 'layer', EDGE_CUTS_LAYER)),
            stroke=stroke,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        result = ['gr_arc',
                  ['start', self.start_x, self.start_y],
                  ['mid', self.mid_x, self.mid_y],
                  ['end', self.end_x, self.end_y],
                  ['stroke',
                   ['width', self.stroke.width],
                   ['type', self.stroke.type.value]],
                  ['layer', QuotedString(self.layer)]]
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert arc to polygon.

        Creates a thick arc shape with semicircular end caps.
        """
        width = self.stroke.width
        if width <= 0:
            return PolygonSet()

        start = (self.start_x, self.start_y)
        mid = (self.mid_x, self.mid_y)
        end = (self.end_x, self.end_y)

        contour = arc_to_polygon(start, mid, end, width, error)
        return PolygonSet(outlines=[contour])

    def get_bounds(self) -> "BoundingBox":
        """Return KiCad-style source geometry bounds."""
        from .kicad_pcb_shape_bounds import arc_bounds

        return arc_bounds(self.start, self.mid, self.end, self.stroke.width)

    @property
    def start(self) -> tuple[float, float]:
        """Get start point as tuple."""
        return (self.start_x, self.start_y)

    @property
    def mid(self) -> tuple[float, float]:
        """Get mid point as tuple."""
        return (self.mid_x, self.mid_y)

    @property
    def end(self) -> tuple[float, float]:
        """Get end point as tuple."""
        return (self.end_x, self.end_y)

    @property
    def width(self) -> float:
        """Get stroke width."""
        return self.stroke.width

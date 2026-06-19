"""
GrRect - Graphical rectangle element (gr_rect)

Represents a rectangle defined by two opposite corners.
Can be filled or unfilled (stroke only).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, List, Optional, Tuple

from .kicad_base import (
    EDGE_CUTS_LAYER,
    ToPolyMixin,
    Stroke,
    StrokeType,
    FillType,
    QuotedString,
    find_element,
    get_value,
    unquote_string,
)
from .kicad_pcb_polygon_ops import (
    PolygonSet,
    rect_to_polygon,
    oval_to_polygon,
    DEFAULT_ERROR_MM,
)

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox


@dataclass
class GrRect(ToPolyMixin):
    """
    Graphical rectangle element (gr_rect).

    A rectangle defined by start (one corner) and end (opposite corner).
    Can be filled (solid) or unfilled (stroke outline only).
    """
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    layer: str = EDGE_CUTS_LAYER
    stroke: Stroke = field(default_factory=Stroke)
    fill: FillType = FillType.NO
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrRect':
        """Parse from s-expression."""
        start = find_element(sexp, 'start')
        end = find_element(sexp, 'end')

        start_x = float(start[1]) if start else 0.0
        start_y = float(start[2]) if start else 0.0
        end_x = float(end[1]) if end else 0.0
        end_y = float(end[2]) if end else 0.0

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

        # Parse fill
        fill_val = get_value(sexp, 'fill', 'no')
        try:
            fill = FillType(fill_val) if isinstance(fill_val, str) else FillType.NO
        except ValueError:
            fill = FillType.NO

        return cls(
            start_x=start_x, start_y=start_y,
            end_x=end_x, end_y=end_y,
            layer=unquote_string(get_value(sexp, 'layer', EDGE_CUTS_LAYER)),
            stroke=stroke,
            fill=fill,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        result = ['gr_rect',
                  ['start', self.start_x, self.start_y],
                  ['end', self.end_x, self.end_y],
                  ['stroke',
                   ['width', self.stroke.width],
                   ['type', self.stroke.type.value]],
                  ['fill', self.fill.value],
                  ['layer', QuotedString(self.layer)]]
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    @property
    def start(self) -> Tuple[float, float]:
        """Get start corner as tuple."""
        return (self.start_x, self.start_y)

    @property
    def end(self) -> Tuple[float, float]:
        """Get end corner as tuple."""
        return (self.end_x, self.end_y)

    @property
    def width(self) -> float:
        """Get stroke width."""
        return self.stroke.width

    @property
    def is_filled(self) -> bool:
        """Check if rectangle is filled."""
        return self.fill in (FillType.SOLID, FillType.YES)

    def get_corners(self) -> List[Tuple[float, float]]:
        """Get all four corners in order (clockwise from top-left)."""
        x1, y1 = min(self.start_x, self.end_x), min(self.start_y, self.end_y)
        x2, y2 = max(self.start_x, self.end_x), max(self.start_y, self.end_y)
        return [(x1, y1), (x2, y1), (x2, y2), (x1, y2)]

    def get_bounds(self) -> "BoundingBox":
        """Return KiCad-style source geometry bounds."""
        from .kicad_pcb_shape_bounds import rect_bounds

        return rect_bounds(self.start, self.end, self.stroke.width)

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert rectangle to polygon.

        For filled rectangles: solid rectangle (optionally expanded by stroke).
        For unfilled rectangles: four capsule shapes for the edges.
        """
        w = self.stroke.width

        if self.is_filled:
            # Filled rectangle - use corners directly, expand by half stroke
            contour = rect_to_polygon(self.start, self.end, w)
            return PolygonSet(outlines=[contour])
        else:
            # Unfilled rectangle - create capsules for each edge
            if w <= 0:
                # Zero-width: just the outline
                corners = self.get_corners()
                return PolygonSet(outlines=[corners])

            # Create four edge capsules (one for each side)
            corners = self.get_corners()
            all_contours = []

            for i in range(4):
                p1 = corners[i]
                p2 = corners[(i + 1) % 4]
                contour = oval_to_polygon(p1, p2, w, error)
                all_contours.append(contour)

            return PolygonSet(outlines=all_contours)

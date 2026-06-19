"""
GrCurve - Graphical Bezier curve element (gr_curve)

Represents a cubic Bezier curve defined by 4 control points.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, List, Optional, Tuple

from .kicad_base import (
    FRONT_SILKSCREEN_LAYER,
    ToPolyMixin,
    Stroke,
    StrokeType,
    QuotedString,
    find_element,
    find_all_elements,
    get_value,
    unquote_string,
)
from .kicad_pcb_polygon_ops import (
    PolygonSet,
    bezier_to_polyline,
    oval_to_polygon,
    DEFAULT_ERROR_MM,
)

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox


@dataclass
class GrCurve(ToPolyMixin):
    """
    Graphical Bezier curve element (gr_curve).

    A cubic Bezier curve defined by 4 control points: P0, P1, P2, P3.
    - P0: Start point
    - P1: First control point
    - P2: Second control point
    - P3: End point

    The curve starts at P0, ends at P3, and is influenced by P1 and P2.
    """
    points: List[Tuple[float, float]] = field(default_factory=list)  # 4 control points
    layer: str = FRONT_SILKSCREEN_LAYER
    stroke: Stroke = field(default_factory=Stroke)
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrCurve':
        """Parse from s-expression."""
        pts_elem = find_element(sexp, 'pts')
        points = []
        if pts_elem:
            for xy in find_all_elements(pts_elem, 'xy'):
                if len(xy) >= 3:
                    points.append((float(xy[1]), float(xy[2])))

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
            points=points,
            layer=unquote_string(get_value(sexp, 'layer', FRONT_SILKSCREEN_LAYER)),
            stroke=stroke,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        pts = ['pts'] + [['xy', p[0], p[1]] for p in self.points]
        result = ['gr_curve', pts,
                  ['stroke',
                   ['width', self.stroke.width],
                   ['type', self.stroke.type.value]],
                  ['layer', QuotedString(self.layer)]]
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    @property
    def width(self) -> float:
        """Get stroke width."""
        return self.stroke.width

    @property
    def p0(self) -> Optional[Tuple[float, float]]:
        """Get start point."""
        return self.points[0] if len(self.points) > 0 else None

    @property
    def p1(self) -> Optional[Tuple[float, float]]:
        """Get first control point."""
        return self.points[1] if len(self.points) > 1 else None

    @property
    def p2(self) -> Optional[Tuple[float, float]]:
        """Get second control point."""
        return self.points[2] if len(self.points) > 2 else None

    @property
    def p3(self) -> Optional[Tuple[float, float]]:
        """Get end point."""
        return self.points[3] if len(self.points) > 3 else None

    def get_bounds(self) -> "BoundingBox":
        """Return KiCad-style source geometry bounds."""
        from .kicad_pcb_shape_bounds import bezier_bounds

        return bezier_bounds(self.points, self.stroke.width)

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert Bezier curve to polygon.

        First converts the curve to a polyline, then creates capsules
        for each segment with the stroke width.
        """
        if len(self.points) != 4:
            return PolygonSet()

        w = self.stroke.width
        if w <= 0:
            return PolygonSet()

        p0, p1, p2, p3 = self.points

        # Convert bezier to polyline
        polyline = bezier_to_polyline(p0, p1, p2, p3, error)

        if len(polyline) < 2:
            return PolygonSet()

        # Create capsules for each segment
        # For now, just create a simple outline from first to last
        all_contours = []
        for i in range(len(polyline) - 1):
            contour = oval_to_polygon(polyline[i], polyline[i + 1], w, error)
            all_contours.append(contour)

        # Return first segment's contour for now
        # Full implementation would union all segments
        if all_contours:
            return PolygonSet(outlines=[all_contours[0]])

        return PolygonSet()

    def to_polyline(self, error: float = DEFAULT_ERROR_MM) -> List[Tuple[float, float]]:
        """
        Convert Bezier curve to polyline points (without stroke width).

        Useful for applications that handle stroke separately.
        """
        if len(self.points) != 4:
            return []

        p0, p1, p2, p3 = self.points
        return bezier_to_polyline(p0, p1, p2, p3, error)

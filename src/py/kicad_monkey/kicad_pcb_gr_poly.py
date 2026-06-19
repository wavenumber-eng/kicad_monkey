"""
GrPoly - Graphical polygon element (gr_poly)

Represents a polygon defined by a list of vertices.
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
from .kicad_pcb_polygon_ops import PolygonSet, oval_to_polygon, DEFAULT_ERROR_MM

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox


@dataclass
class GrPoly(ToPolyMixin):
    """
    Graphical polygon element (gr_poly).

    A polygon defined by a list of (x, y) points.
    Can be filled (solid) or unfilled (stroke outline only).

    KiCad permits ``(pts ...)`` to mix ``(xy x y)`` and ``(arc (start ...)
    (mid ...) (end ...))`` children (see PCB_PLUGIN parser ``parsePoint``/
    ``parseArc`` paths). We capture the ordered child list verbatim in
    ``pts_segments`` so round-trip emit can replay arcs we do not model in
    the simple ``points`` view; ``points`` keeps the xy-only projection used
    by geometry/SVG consumers.
    """
    points: List[Tuple[float, float]] = field(default_factory=list)
    pts_segments: List[list] = field(default_factory=list)
    layer: str = EDGE_CUTS_LAYER
    stroke: Stroke = field(default_factory=Stroke)
    fill: FillType = FillType.NO
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrPoly':
        """Parse from s-expression."""
        pts_elem = find_element(sexp, 'pts')
        points: List[Tuple[float, float]] = []
        pts_segments: List[list] = []
        has_non_xy = False
        if pts_elem:
            for child in pts_elem[1:]:
                if not isinstance(child, list) or not child:
                    continue
                tag = child[0]
                if tag == 'xy' and len(child) >= 3:
                    points.append((float(child[1]), float(child[2])))
                elif tag == 'arc':
                    has_non_xy = True
                else:
                    has_non_xy = True
                pts_segments.append(child)
            if not has_non_xy:
                # Pure xy polygon — drop the verbatim copy; the ``points`` view
                # is sufficient and the synthesized emit matches upstream.
                pts_segments = []

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
            points=points,
            pts_segments=pts_segments,
            layer=unquote_string(get_value(sexp, 'layer', EDGE_CUTS_LAYER)),
            stroke=stroke,
            fill=fill,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        if self.pts_segments:
            # Replay arc-bearing pts verbatim (we do not model arc geometry).
            pts = ['pts'] + list(self.pts_segments)
        else:
            pts = ['pts'] + [['xy', p[0], p[1]] for p in self.points]
        result = ['gr_poly', pts,
                  ['stroke',
                   ['width', self.stroke.width],
                   ['type', self.stroke.type.value]],
                  ['fill', self.fill.value],
                  ['layer', QuotedString(self.layer)]]
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    @property
    def width(self) -> float:
        """Get stroke width."""
        return self.stroke.width

    @property
    def is_filled(self) -> bool:
        """Check if polygon is filled."""
        return self.fill in (FillType.SOLID, FillType.YES)

    def get_bounds(self) -> "BoundingBox":
        """Return KiCad-style source geometry bounds."""
        from .kicad_pcb_shape_bounds import poly_bounds

        return poly_bounds(self.points, self.stroke.width)

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert polygon element to polygon set.

        For filled polygons: returns the points directly.
        For unfilled polygons: creates capsules for each edge.
        """
        if not self.points:
            return PolygonSet()

        w = self.stroke.width

        if self.is_filled:
            # Filled polygon - use points directly
            return PolygonSet(outlines=[list(self.points)])
        else:
            # Unfilled polygon - stroke outline only
            if w <= 0:
                return PolygonSet(outlines=[list(self.points)])

            # Create capsules for each edge
            all_contours = []
            n = len(self.points)
            for i in range(n):
                p1 = self.points[i]
                p2 = self.points[(i + 1) % n]
                contour = oval_to_polygon(p1, p2, w, error)
                all_contours.append(contour)

            return PolygonSet(outlines=all_contours)

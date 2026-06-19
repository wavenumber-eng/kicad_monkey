"""
GrCircle - Graphical circle element (gr_circle)

Represents a circle defined by center and a point on the circumference.
Can be filled (solid circle) or unfilled (ring).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Optional

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
    circle_to_polygon,
    ring_to_polygon,
    DEFAULT_ERROR_MM,
)

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox


@dataclass
class GrCircle(ToPolyMixin):
    """
    Graphical circle element (gr_circle).

    A circle defined by center point and a point on the circumference (end).
    The radius is computed as the distance between center and end.

    For filled circles (fill=solid), produces a solid disk.
    For unfilled circles (fill=none/no), produces a ring with stroke width.
    """
    center_x: float
    center_y: float
    end_x: float
    end_y: float
    layer: str = EDGE_CUTS_LAYER
    stroke: Stroke = field(default_factory=Stroke)
    fill: FillType = FillType.NO
    locked: bool = False
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrCircle':
        """Parse from s-expression."""
        center = find_element(sexp, 'center')
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

        # Parse fill
        fill_val = get_value(sexp, 'fill', 'no')
        try:
            fill = FillType(fill_val) if isinstance(fill_val, str) else FillType.NO
        except ValueError:
            fill = FillType.NO

        locked_elem = find_element(sexp, 'locked')
        locked = bool(locked_elem) and (
            len(locked_elem) <= 1
            or unquote_string(locked_elem[1]).lower() in ('yes', 'true', '1')
        )

        return cls(
            center_x=float(center[1]) if center else 0.0,
            center_y=float(center[2]) if center else 0.0,
            end_x=float(end[1]) if end else 0.0,
            end_y=float(end[2]) if end else 0.0,
            layer=unquote_string(get_value(sexp, 'layer', EDGE_CUTS_LAYER)),
            stroke=stroke,
            fill=fill,
            locked=locked,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        # Per pcb_io_kicad_sexpr.cpp:1092-1098 (PCB_SHAPE format), (locked yes)
        # is emitted between (fill ...) and (layer ...).
        result: list = ['gr_circle',
                        ['center', self.center_x, self.center_y],
                        ['end', self.end_x, self.end_y],
                        ['stroke',
                         ['width', self.stroke.width],
                         ['type', self.stroke.type.value]],
                        ['fill', self.fill.value]]
        if self.locked:
            result.append(['locked', 'yes'])
        result.append(['layer', QuotedString(self.layer)])
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    @property
    def radius(self) -> float:
        """Calculate radius from center to end point."""
        dx = self.end_x - self.center_x
        dy = self.end_y - self.center_y
        return math.sqrt(dx * dx + dy * dy)

    @property
    def center(self) -> tuple[float, float]:
        """Get center point as tuple."""
        return (self.center_x, self.center_y)

    @property
    def width(self) -> float:
        """Get stroke width."""
        return self.stroke.width

    @property
    def is_filled(self) -> bool:
        """Check if circle is filled."""
        return self.fill in (FillType.SOLID, FillType.YES)

    def get_bounds(self) -> "BoundingBox":
        """Return KiCad-style source geometry bounds."""
        from .kicad_pcb_shape_bounds import circle_bounds

        return circle_bounds(self.center, (self.end_x, self.end_y), self.stroke.width)

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert circle to polygon.

        For filled circles: solid disk with radius extended by half stroke width.
        For unfilled circles: ring (annulus) with stroke width.
        """
        r = self.radius
        w = self.stroke.width
        center = self.center

        if self.is_filled:
            # Solid disk - extend radius by half stroke width for outline
            outer_radius = r + w / 2 if w > 0 else r
            contour = circle_to_polygon(center, outer_radius, error)
            return PolygonSet(outlines=[contour])
        else:
            # Ring (unfilled circle with stroke)
            if w <= 0:
                # Zero-width unfilled circle - just the outline
                contour = circle_to_polygon(center, r, error)
                return PolygonSet(outlines=[contour])

            return ring_to_polygon(center, r, w, error)

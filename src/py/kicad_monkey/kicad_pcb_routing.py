"""
KiCad PCB Routing - Track segments, vias, and arcs

PCB routing/track elements.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox, SvgRenderContext

from .kicad_sexpr import QuotedString, SexpList
from .kicad_base import (
    ToPolyMixin,
    find_element,
    get_value,
    get_at,
    has_flag,
    parse_maybe_absent_bool,
    unquote_string,
)
from .kicad_pcb_polygon_ops import (
    PolygonSet,
    oval_to_polygon,
    circle_to_polygon,
    arc_to_polygon,
    DEFAULT_ERROR_MM,
)
from .kicad_pcb_other import (
    DrillProps,
    NetRef,
    PostMachiningProps,
    ZoneLayerConnections,
)


@dataclass
class Segment(ToPolyMixin):
    """Track segment."""
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    width: float
    layer: str
    net: NetRef = field(default_factory=NetRef)
    uuid: Optional[str] = None
    locked: bool = False
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Segment':
        start = find_element(sexp, 'start')
        end = find_element(sexp, 'end')
        raw_net = get_value(sexp, 'net', 0)
        net = NetRef.from_raw_token(raw_net)

        return cls(
            start_x=float(start[1]) if start else 0.0,
            start_y=float(start[2]) if start else 0.0,
            end_x=float(end[1]) if end else 0.0,
            end_y=float(end[2]) if end else 0.0,
            width=float(get_value(sexp, 'width', 0.0)),
            layer=unquote_string(get_value(sexp, 'layer', '')),
            net=net,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            locked=has_flag(sexp, 'locked'),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result: SexpList = ['segment',
                  ['start', self.start_x, self.start_y],
                  ['end', self.end_x, self.end_y],
                  ['width', self.width],
                  ['layer', QuotedString(self.layer)]]
        net_elem = self.net.to_inline_net_sexp()
        if net_elem:
            result.append(net_elem)
        if self.locked:
            result.append('locked')
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """Convert track segment to polygon (capsule/stadium shape)."""
        if self.width <= 0:
            return PolygonSet()
        start = (self.start_x, self.start_y)
        end = (self.end_x, self.end_y)
        contour = oval_to_polygon(start, end, self.width, error)
        return PolygonSet(outlines=[contour])

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of this segment.."""
        from .kicad_geometry import BoundingBox

        hw = self.width / 2
        return BoundingBox(
            min_x=min(self.start_x, self.end_x) - hw,
            min_y=min(self.start_y, self.end_y) - hw,
            max_x=max(self.start_x, self.end_x) + hw,
            max_y=max(self.start_y, self.end_y) + hw
        )

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> List[str]:
        """Render this segment to SVG elements.."""
        from .kicad_geometry import SvgRenderContext

        if ctx is None:
            ctx = SvgRenderContext()

        if not ctx.layer_visible(self.layer):
            return []

        sx = self.start_x + ctx.offset_x
        sy = self.start_y + ctx.offset_y
        ex = self.end_x + ctx.offset_x
        ey = self.end_y + ctx.offset_y

        return [
            f'<path d="M{ctx.fmt(sx)} {ctx.fmt(sy)} L{ctx.fmt(ex)} {ctx.fmt(ey)}" '
            f'style="fill:none; stroke:{ctx.stroke}; stroke-width:{ctx.fmt(self.width)}; '
            f'stroke-linecap:round; stroke-linejoin:round;" />'
        ]


def _parse_opt_bool_token(tok: object) -> Optional[bool]:
    """Parse a yes/no/none token into Optional[bool]. None token -> None."""
    s = unquote_string(tok).lower() if tok is not None else ''
    if s == 'yes':
        return True
    if s == 'no':
        return False
    return None


def _format_opt_bool(value: Optional[bool]) -> str:
    """Format Optional[bool] as yes/no/none."""
    if value is True:
        return 'yes'
    if value is False:
        return 'no'
    return 'none'


@dataclass
class FrontBackOptBool:
    """Optional front/back bool pair, e.g. (tenting (front yes) (back no))."""
    front: Optional[bool] = None
    back: Optional[bool] = None

    @classmethod
    def from_sexp(cls, sexp: Optional[list]) -> Optional['FrontBackOptBool']:
        if sexp is None:
            return None
        front_elem = find_element(sexp, 'front')
        back_elem = find_element(sexp, 'back')
        front = _parse_opt_bool_token(front_elem[1]) if front_elem and len(front_elem) > 1 else None
        back = _parse_opt_bool_token(back_elem[1]) if back_elem and len(back_elem) > 1 else None
        # Only return an instance if at least one side has a value
        if front is None and back is None:
            return None
        return cls(front=front, back=back)

    def to_sexp(self, tag: str) -> list:
        return [tag,
                ['front', _format_opt_bool(self.front)],
                ['back', _format_opt_bool(self.back)]]


@dataclass
class Via(ToPolyMixin):
    """Via element."""
    at_x: float
    at_y: float
    size: float
    drill: float
    layers: List[str] = field(default_factory=list)
    free: bool = False
    tenting: Optional[FrontBackOptBool] = None
    covering: Optional[FrontBackOptBool] = None
    plugging: Optional[FrontBackOptBool] = None
    capping: Optional[bool] = None
    filling: Optional[bool] = None
    net: NetRef = field(default_factory=NetRef)
    backdrill: Optional[DrillProps] = None
    tertiary_drill: Optional[DrillProps] = None
    front_post_machining: Optional[PostMachiningProps] = None
    back_post_machining: Optional[PostMachiningProps] = None
    zone_layer_connections: Optional[ZoneLayerConnections] = None
    uuid: Optional[str] = None
    via_type: Optional[str] = None
    remove_unused_layers: Optional[bool] = None
    keep_end_layers: Optional[bool] = None
    start_end_only: Optional[bool] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Via':
        x, y, _ = get_at(sexp)
        layers_elem = find_element(sexp, 'layers')
        layers = [unquote_string(layer) for layer in layers_elem[1:]] if layers_elem else []
        raw_net = get_value(sexp, 'net', 0)
        net = NetRef.from_raw_token(raw_net)

        # Check for via type (blind, buried, micro)
        via_type = None
        if has_flag(sexp, 'blind'):
            via_type = 'blind'
        elif has_flag(sexp, 'buried'):
            via_type = 'buried'
        elif has_flag(sexp, 'micro'):
            via_type = 'micro'

        free = False
        free_elem = find_element(sexp, 'free')
        if free_elem and len(free_elem) > 1:
            free = unquote_string(free_elem[1]).lower() == 'yes'

        tenting = FrontBackOptBool.from_sexp(find_element(sexp, 'tenting'))
        covering = FrontBackOptBool.from_sexp(find_element(sexp, 'covering'))
        plugging = FrontBackOptBool.from_sexp(find_element(sexp, 'plugging'))

        capping = None
        capping_elem = find_element(sexp, 'capping')
        if capping_elem and len(capping_elem) > 1:
            capping = _parse_opt_bool_token(capping_elem[1])

        filling = None
        filling_elem = find_element(sexp, 'filling')
        if filling_elem and len(filling_elem) > 1:
            filling = _parse_opt_bool_token(filling_elem[1])

        backdrill = None
        parsed_backdrill = DrillProps.from_sexp(find_element(sexp, "backdrill"))
        if parsed_backdrill:
            backdrill = parsed_backdrill

        tertiary_drill = None
        parsed_tertiary_drill = DrillProps.from_sexp(find_element(sexp, "tertiary_drill"))
        if parsed_tertiary_drill:
            tertiary_drill = parsed_tertiary_drill

        front_post_machining = None
        parsed_front_post_machining = PostMachiningProps.from_sexp(
            find_element(sexp, "front_post_machining")
        )
        if parsed_front_post_machining:
            front_post_machining = parsed_front_post_machining

        back_post_machining = None
        parsed_back_post_machining = PostMachiningProps.from_sexp(
            find_element(sexp, "back_post_machining")
        )
        if parsed_back_post_machining:
            back_post_machining = parsed_back_post_machining

        zone_layer_connections = None
        zone_layer_connections_elem = find_element(sexp, "zone_layer_connections")
        if zone_layer_connections_elem is not None:
            zone_layer_connections = ZoneLayerConnections.from_sexp(zone_layer_connections_elem)

        return cls(
            at_x=x, at_y=y,
            size=float(get_value(sexp, 'size', 0.0)),
            drill=float(get_value(sexp, 'drill', 0.0)),
            layers=layers,
            free=free,
            tenting=tenting,
            covering=covering,
            plugging=plugging,
            capping=capping,
            filling=filling,
            net=net,
            backdrill=backdrill,
            tertiary_drill=tertiary_drill,
            front_post_machining=front_post_machining,
            back_post_machining=back_post_machining,
            zone_layer_connections=zone_layer_connections,
            remove_unused_layers=parse_maybe_absent_bool(sexp, "remove_unused_layers"),
            keep_end_layers=parse_maybe_absent_bool(sexp, "keep_end_layers"),
            start_end_only=parse_maybe_absent_bool(sexp, "start_end_only"),
            uuid=unquote_string(get_value(sexp, 'uuid')),
            via_type=via_type,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result: SexpList = ['via']
        if self.via_type:
            result.append(self.via_type)
        result.append(['at', self.at_x, self.at_y])
        result.append(['size', self.size])
        result.append(['drill', self.drill])
        result.append(['layers'] + [QuotedString(layer) for layer in self.layers])
        if self.free:
            result.append(['free', 'yes'])
        if self.tenting is not None:
            result.append(self.tenting.to_sexp('tenting'))
        if self.capping is not None:
            result.append(['capping', _format_opt_bool(self.capping)])
        if self.covering is not None:
            result.append(self.covering.to_sexp('covering'))
        if self.plugging is not None:
            result.append(self.plugging.to_sexp('plugging'))
        if self.filling is not None:
            result.append(['filling', _format_opt_bool(self.filling)])
        if self.backdrill:
            result.append(self.backdrill.to_sexp("backdrill"))
        if self.tertiary_drill:
            result.append(self.tertiary_drill.to_sexp("tertiary_drill"))
        if self.front_post_machining:
            result.append(self.front_post_machining.to_sexp("front_post_machining"))
        if self.back_post_machining:
            result.append(self.back_post_machining.to_sexp("back_post_machining"))
        for name, value in (
            ("remove_unused_layers", self.remove_unused_layers),
            ("keep_end_layers", self.keep_end_layers),
            ("start_end_only", self.start_end_only),
        ):
            if value is not None:
                result.append([name, "yes" if value else "no"])
        if self.zone_layer_connections is not None:
            result.append(self.zone_layer_connections.to_sexp())
        net_elem = self.net.to_inline_net_sexp()
        if net_elem:
            result.append(net_elem)
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """Convert via to polygon (annular ring - circle with drill hole)."""
        if self.size <= 0:
            return PolygonSet()
        # Outer circle (via pad)
        outer = circle_to_polygon((self.at_x, self.at_y), self.size / 2, error)
        result = PolygonSet(outlines=[outer])
        # Inner hole (drill)
        if self.drill > 0:
            inner = circle_to_polygon((self.at_x, self.at_y), self.drill / 2, error)
            result.holes.append(inner)
        return result

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of this via.."""
        from .kicad_geometry import BoundingBox

        r = self.size / 2
        return BoundingBox(
            min_x=self.at_x - r,
            min_y=self.at_y - r,
            max_x=self.at_x + r,
            max_y=self.at_y + r
        )

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> List[str]:
        """Render this via to SVG elements.."""
        from .kicad_geometry import SvgRenderContext

        if ctx is None:
            ctx = SvgRenderContext()

        # Vias span multiple layers - check if any layer is visible
        if ctx.layers is not None:
            if not any(ctx.layer_visible(layer) for layer in self.layers):
                return []

        cx = self.at_x + ctx.offset_x
        cy = self.at_y + ctx.offset_y
        r = self.size / 2

        return [
            f'<circle cx="{ctx.fmt(cx)}" cy="{ctx.fmt(cy)}" r="{ctx.fmt(r)}" '
            f'style="fill:{ctx.fill}; stroke:none;" />'
        ]


@dataclass
class Arc(ToPolyMixin):
    """Track arc."""
    start_x: float
    start_y: float
    mid_x: float
    mid_y: float
    end_x: float
    end_y: float
    width: float
    layer: str
    net: NetRef = field(default_factory=NetRef)
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Arc':
        start = find_element(sexp, 'start')
        mid = find_element(sexp, 'mid')
        end = find_element(sexp, 'end')
        raw_net = get_value(sexp, 'net', 0)
        net = NetRef.from_raw_token(raw_net)

        return cls(
            start_x=float(start[1]) if start else 0.0,
            start_y=float(start[2]) if start else 0.0,
            mid_x=float(mid[1]) if mid else 0.0,
            mid_y=float(mid[2]) if mid else 0.0,
            end_x=float(end[1]) if end else 0.0,
            end_y=float(end[2]) if end else 0.0,
            width=float(get_value(sexp, 'width', 0.0)),
            layer=unquote_string(get_value(sexp, 'layer', '')),
            net=net,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result = ['arc',
                  ['start', self.start_x, self.start_y],
                  ['mid', self.mid_x, self.mid_y],
                  ['end', self.end_x, self.end_y],
                  ['width', self.width],
                  ['layer', QuotedString(self.layer)]]
        net_elem = self.net.to_inline_net_sexp()
        if net_elem:
            result.append(net_elem)
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """Convert track arc to thick arc polygon."""
        if self.width <= 0:
            return PolygonSet()
        start = (self.start_x, self.start_y)
        mid = (self.mid_x, self.mid_y)
        end = (self.end_x, self.end_y)
        contour = arc_to_polygon(start, mid, end, self.width, error)
        return PolygonSet(outlines=[contour])

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of this arc.."""
        from .kicad_geometry import BoundingBox

        # Use _to_poly for accurate arc bounds including width
        poly = self._to_poly()
        if not poly.outlines:
            return BoundingBox()

        bbox = BoundingBox()
        for x, y in poly.outlines[0]:
            bbox.expand((x, y))
        return bbox

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> List[str]:
        """Render this arc to SVG elements.."""
        from .kicad_geometry import SvgRenderContext

        if ctx is None:
            ctx = SvgRenderContext()

        if not ctx.layer_visible(self.layer):
            return []

        # Calculate SVG arc parameters (same logic as compute_svg_arc_params)
        start = (self.start_x, self.start_y)
        mid = (self.mid_x, self.mid_y)
        end = (self.end_x, self.end_y)

        # Calculate center of circle through 3 points
        ax, ay = start
        bx, by = mid
        cx, cy = end

        # Midpoints of segments AB and BC
        d_ab = ((ax + bx) / 2, (ay + by) / 2)
        d_bc = ((bx + cx) / 2, (by + cy) / 2)

        # Direction vectors
        ab = (bx - ax, by - ay)
        bc = (cx - bx, cy - by)

        # Perpendicular directions
        perp_ab = (-ab[1], ab[0])
        perp_bc = (-bc[1], bc[0])

        # Find intersection of perpendicular bisectors
        det = perp_ab[0] * perp_bc[1] - perp_ab[1] * perp_bc[0]

        if abs(det) < 1e-10:
            # Collinear - render as line
            sx = self.start_x + ctx.offset_x
            sy = self.start_y + ctx.offset_y
            ex = self.end_x + ctx.offset_x
            ey = self.end_y + ctx.offset_y
            return [
                f'<path d="M{ctx.fmt(sx)} {ctx.fmt(sy)} L{ctx.fmt(ex)} {ctx.fmt(ey)}" '
                f'style="fill:none; stroke:{ctx.stroke}; stroke-width:{ctx.fmt(self.width)}; '
                f'stroke-linecap:round; stroke-linejoin:round;" />'
            ]

        dx = d_bc[0] - d_ab[0]
        dy = d_bc[1] - d_ab[1]
        t = (dx * perp_bc[1] - dy * perp_bc[0]) / det

        center_x = d_ab[0] + t * perp_ab[0]
        center_y = d_ab[1] + t * perp_ab[1]

        radius = math.sqrt((ax - center_x) ** 2 + (ay - center_y) ** 2)

        # Determine arc direction
        v1 = (cx - ax, cy - ay)
        v2 = (bx - ax, by - ay)
        cross = v1[0] * v2[1] - v1[1] * v2[0]
        is_cw = cross <= 0

        if is_cw:
            svg_start = end
            svg_end = start
        else:
            svg_start = start
            svg_end = end

        # Calculate angle span
        angle_at_start = math.atan2(ay - center_y, ax - center_x)
        angle_at_end = math.atan2(cy - center_y, cx - center_x)
        angle = angle_at_end - angle_at_start

        if is_cw:
            while angle < 0:
                angle += 2 * math.pi
        else:
            while angle > 0:
                angle -= 2 * math.pi

        large_arc_flag = 1 if abs(angle) > math.pi else 0
        sweep_flag = 0

        # Apply offset
        sx = svg_start[0] + ctx.offset_x
        sy = svg_start[1] + ctx.offset_y
        ex = svg_end[0] + ctx.offset_x
        ey = svg_end[1] + ctx.offset_y

        return [
            f'<path d="M{ctx.fmt(sx)} {ctx.fmt(sy)} '
            f'A{ctx.fmt(radius)} {ctx.fmt(radius)} 0 {large_arc_flag} {sweep_flag} '
            f'{ctx.fmt(ex)} {ctx.fmt(ey)}" '
            f'style="fill:none; stroke:{ctx.stroke}; stroke-width:{ctx.fmt(self.width)}; '
            f'stroke-linecap:round; stroke-linejoin:round;" />'
        ]


__all__ = [
    'Segment',
    'Via',
    'Arc',
]

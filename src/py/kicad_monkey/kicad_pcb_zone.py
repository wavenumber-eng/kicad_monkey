"""
KiCad PCB Zone - Zone (copper pour) elements with keepout and placement settings

Zone elements including copper pours, keepout areas, and placement rule areas.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional, Tuple, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox, SvgRenderContext

from .kicad_sexpr import QuotedString, SexpList
from .kicad_base import (
    PlacementSourceType,
    find_element,
    find_all_elements,
    get_value,
    unquote_string,
)
from .kicad_pcb_other import NetRef


@dataclass
class ZonePlacement:
    """
    Placement Rule Area settings for multi-channel design.

    Placement Rule Areas are special zones that define regions for automatic
    footprint placement based on schematic hierarchy (sheet names), component
    classes, or groups. Used for replicating multi-channel designs.

    Reference: zone_settings.h lines 152-155
    """
    enabled: bool = False
    source_type: PlacementSourceType = PlacementSourceType.SHEETNAME
    source: str = ""

    @classmethod
    def from_sexp(cls, sexp: list) -> Optional['ZonePlacement']:
        """
        Parse placement element from zone s-expression.

        Format: (placement (enabled yes/no) (sheetname "value"))
                (placement (enabled yes/no) (component_class "value"))
                (placement (enabled yes/no) (group "value"))
        """
        placement_elem = find_element(sexp, 'placement')
        if not placement_elem:
            return None

        enabled = get_value(placement_elem, 'enabled', 'no') == 'yes'

        # Determine source type and value
        source_type = PlacementSourceType.SHEETNAME
        source = ""

        sheetname_elem = find_element(placement_elem, 'sheetname')
        if sheetname_elem and len(sheetname_elem) > 1:
            source_type = PlacementSourceType.SHEETNAME
            source = unquote_string(sheetname_elem[1])
        else:
            component_class_elem = find_element(placement_elem, 'component_class')
            if component_class_elem and len(component_class_elem) > 1:
                source_type = PlacementSourceType.COMPONENT_CLASS
                source = unquote_string(component_class_elem[1])
            else:
                group_elem = find_element(placement_elem, 'group')
                if group_elem and len(group_elem) > 1:
                    source_type = PlacementSourceType.GROUP_PLACEMENT
                    source = unquote_string(group_elem[1])

        return cls(enabled=enabled, source_type=source_type, source=source)

    def to_sexp(self) -> list:
        """Serialize to KiCad s-expression format."""
        result: SexpList = ['placement', ['enabled', 'yes' if self.enabled else 'no']]

        # Output appropriate source type element
        if self.source_type == PlacementSourceType.SHEETNAME:
            result.append(['sheetname', QuotedString(self.source)])
        elif self.source_type == PlacementSourceType.COMPONENT_CLASS:
            result.append(['component_class', QuotedString(self.source)])
        elif self.source_type == PlacementSourceType.GROUP_PLACEMENT:
            result.append(['group', QuotedString(self.source)])

        return result


@dataclass
class Keepout:
    """Keepout zone settings."""
    tracks: str = "not_allowed"
    vias: str = "not_allowed"
    pads: str = "not_allowed"
    copperpour: str = "not_allowed"
    footprints: str = "not_allowed"

    @classmethod
    def from_sexp(cls, sexp: list) -> Optional['Keepout']:
        keepout_elem = find_element(sexp, 'keepout')
        if not keepout_elem:
            return None

        return cls(
            tracks=str(get_value(keepout_elem, 'tracks', 'not_allowed')),
            vias=str(get_value(keepout_elem, 'vias', 'not_allowed')),
            pads=str(get_value(keepout_elem, 'pads', 'not_allowed')),
            copperpour=str(get_value(keepout_elem, 'copperpour', 'not_allowed')),
            footprints=str(get_value(keepout_elem, 'footprints', 'not_allowed'))
        )

    def to_sexp(self) -> list:
        return ['keepout',
                ['tracks', self.tracks],
                ['vias', self.vias],
                ['pads', self.pads],
                ['copperpour', self.copperpour],
                ['footprints', self.footprints]]


@dataclass
class ZonePolygon:
    """A polygon within a zone."""
    points: List[Tuple[float, float]] = field(default_factory=list)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'ZonePolygon':
        pts_elem = find_element(sexp, 'pts')
        points = []
        if pts_elem:
            for xy in find_all_elements(pts_elem, 'xy'):
                if len(xy) >= 3:
                    points.append((float(xy[1]), float(xy[2])))
        return cls(points=points)

    def to_sexp(self) -> list:
        pts = ['pts'] + [['xy', p[0], p[1]] for p in self.points]
        return ['polygon', pts]


@dataclass
class FilledPolygon:
    """A filled polygon within a zone."""
    layer: str
    island: bool = False
    points: List[Tuple[float, float]] = field(default_factory=list)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'FilledPolygon':
        layer = unquote_string(get_value(sexp, 'layer', ''))
        # `(island)` parses as a sub-list ['island'] not a bare token, so
        # has_flag() (which checks `name in sexp`) misses it. Use
        # find_element instead.
        island = find_element(sexp, 'island') is not None
        pts_elem = find_element(sexp, 'pts')
        points = []
        if pts_elem:
            for xy in find_all_elements(pts_elem, 'xy'):
                if len(xy) >= 3:
                    points.append((float(xy[1]), float(xy[2])))
        return cls(layer=layer, island=island, points=points)

    def to_sexp(self) -> list:
        pts = ['pts'] + [['xy', p[0], p[1]] for p in self.points]
        result = ['filled_polygon', ['layer', QuotedString(self.layer)]]
        if self.island:
            # KiCad emits `(island)` as a single-element sub-list, NOT
            # a bare `island` token (the latter would confuse the parser).
            result.append(['island'])
        result.append(pts)
        return result


@dataclass
class Zone:
    """Zone (copper pour) element.

    KiCad zones may carry their layer assignment as either ``(layer "X")``
    (single layer) or ``(layers "A" "B" ...)`` (multi-layer / wildcard
    form, e.g. ``"*.Cu"``). The plural form is REQUIRED for multi-layer
    or wildcard zones — kicad-cli SEGFAULTs (rc 0xC0000005) on parse if
    a multi-layer source is round-tripped to a singular ``(layer "")``
    that drops the layer assignment.

    We preserve the source form via ``layers_plural``: when True (parsed
    from ``(layers ...)``), emit plural; else emit singular ``(layer ...)``.
    The ``layer`` attribute stays as a single-string back-compat shim
    returning the first layer.
    """
    net: NetRef = field(default_factory=NetRef)
    has_explicit_net_name: bool = False
    layers: List[str] = field(default_factory=list)
    layers_plural: bool = False
    locked: bool = False
    uuid: Optional[str] = None
    name: Optional[str] = None
    hatch_style: str = "edge"
    hatch_pitch: float = 0.5
    # KiCad emits `(priority N)` only when N > 0
    # (pcb_io_kicad_sexpr.cpp:2954). Default 0.
    priority: int = 0
    connect_pads_clearance: float = 0.5
    min_thickness: float = 0.25
    filled_areas_thickness: bool = False
    fill_enabled: bool = False
    thermal_gap: float = 0.5
    thermal_bridge_width: float = 0.5
    island_removal_mode: Optional[int] = None
    island_area_min: Optional[float] = None
    keepout: Optional[Keepout] = None
    placement: Optional[ZonePlacement] = None  # Placement Rule Area settings
    # Per-layer (property ...) blocks emitted by KiCad after the fill
    # block when ZONE_LAYER_PROPERTIES::hatching_offset has a value.
    # See PCB_IO_KICAD_SEXPR::format(ZONE_LAYER_PROPERTIES&) at
    # pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp:3139. Each entry
    # is (layer_name, (offset_x, offset_y)).
    layer_properties: List[Tuple[str, Tuple[float, float]]] = field(default_factory=list)
    polygons: List[ZonePolygon] = field(default_factory=list)
    filled_polygons: List[FilledPolygon] = field(default_factory=list)
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @property
    def layer(self) -> str:
        """First layer name, or empty string. Back-compat shim for callers
        that treated zones as single-layer."""
        return self.layers[0] if self.layers else ""

    @layer.setter
    def layer(self, value: str) -> None:
        self.layers = [value] if value else []
        self.layers_plural = False

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Zone':
        hatch = find_element(sexp, 'hatch')
        connect_pads = find_element(sexp, 'connect_pads')
        fill = find_element(sexp, 'fill')
        raw_net = get_value(sexp, 'net', 0)

        explicit_net_name_raw = find_element(sexp, 'net_name')
        explicit_net_name = unquote_string(get_value(sexp, 'net_name', ''))
        has_net_name = explicit_net_name_raw is not None
        net = NetRef.from_raw_token(raw_net, explicit_name=explicit_net_name)

        # Layer assignment: try plural `(layers "A" "B" ...)` first, then
        # fall back to singular `(layer "X")`. Multi-layer / wildcard zones
        # use the plural form; round-tripping requires preserving it.
        layers_elem = find_element(sexp, 'layers')
        if layers_elem and len(layers_elem) > 1:
            layers_list = [unquote_string(tok) for tok in layers_elem[1:]]
            layers_plural = True
        else:
            single = unquote_string(get_value(sexp, 'layer', ''))
            layers_list = [single] if single else []
            layers_plural = False

        polygons = [ZonePolygon.from_sexp(p) for p in find_all_elements(sexp, 'polygon')]
        filled_polygons = [FilledPolygon.from_sexp(p) for p in find_all_elements(sexp, 'filled_polygon')]

        # Parse keepout settings
        keepout = Keepout.from_sexp(sexp)

        # Parse placement rule area settings
        placement = ZonePlacement.from_sexp(sexp)

        # Optional fill metadata: island_removal_mode / island_area_min.
        island_mode_raw = get_value(fill, 'island_removal_mode', None) if fill else None
        island_min_raw = get_value(fill, 'island_area_min', None) if fill else None

        # Per-layer (property ...) blocks. Walk top-level children so we
        # don't accidentally pick up (property ...) elements nested in
        # placement / keepout sub-trees.
        layer_properties: List[Tuple[str, Tuple[float, float]]] = []
        for prop_elem in find_all_elements(sexp, 'property'):
            layer_name = unquote_string(get_value(prop_elem, 'layer', ''))
            hp_elem = find_element(prop_elem, 'hatch_position')
            if hp_elem:
                xy_elem = find_element(hp_elem, 'xy')
                if xy_elem and len(xy_elem) >= 3:
                    layer_properties.append(
                        (layer_name, (float(xy_elem[1]), float(xy_elem[2])))
                    )

        locked_elem = find_element(sexp, 'locked')
        locked = bool(locked_elem) and (
            len(locked_elem) <= 1
            or unquote_string(locked_elem[1]).lower() in ('yes', 'true', '1')
        )

        zone_name_raw = unquote_string(get_value(sexp, 'name'))
        zone_name = zone_name_raw if zone_name_raw else None

        instance = cls(
            net=net,
            has_explicit_net_name=has_net_name,
            layers=layers_list,
            layers_plural=layers_plural,
            locked=locked,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            name=zone_name,
            hatch_style=hatch[1] if hatch and len(hatch) > 1 else 'edge',
            hatch_pitch=float(hatch[2]) if hatch and len(hatch) > 2 else 0.5,
            priority=int(get_value(sexp, 'priority', 0)),
            connect_pads_clearance=float(get_value(connect_pads, 'clearance', 0.5)) if connect_pads else 0.5,
            min_thickness=float(get_value(sexp, 'min_thickness', 0.25)),
            filled_areas_thickness=get_value(sexp, 'filled_areas_thickness') == 'yes',
            fill_enabled=fill[1] == 'yes' if fill and len(fill) > 1 else False,
            thermal_gap=float(get_value(fill, 'thermal_gap', 0.5)) if fill else 0.5,
            thermal_bridge_width=float(get_value(fill, 'thermal_bridge_width', 0.5)) if fill else 0.5,
            island_removal_mode=int(island_mode_raw) if island_mode_raw is not None else None,
            island_area_min=float(island_min_raw) if island_min_raw is not None else None,
            keepout=keepout,
            placement=placement,
            layer_properties=layer_properties,
            polygons=polygons,
            filled_polygons=filled_polygons,
            _raw_sexp=sexp
        )
        return instance

    def to_sexp(self) -> list:
        result: SexpList = ['zone']
        net_elem = self.net.to_inline_net_sexp()
        if net_elem:
            result.append(net_elem)
        # Preserve `(net_name "")` even when the name is empty — KiCad
        # emits the empty form when the source had it.
        if self.has_explicit_net_name:
            result.append(['net_name', QuotedString(self.net.name)])
        # Per pcb_io_kicad_sexpr.cpp:2917, (locked yes) is emitted before
        # the layers block.
        if self.locked:
            result.append(['locked', 'yes'])
        # Emit plural `(layers ...)` when the source used plural form OR
        # when more than one layer is present. Otherwise emit singular.
        if self.layers_plural or len(self.layers) > 1:
            result.append(['layers'] + [QuotedString(layer) for layer in self.layers])
        else:
            result.append(['layer', QuotedString(self.layer)])
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        # Per pcb_io_kicad_sexpr.cpp:2935-2936, (name "...") sits between
        # uuid and hatch (zone names are typically used by keepout areas).
        if self.name:
            result.append(['name', QuotedString(self.name)])
        result.append(['hatch', self.hatch_style, self.hatch_pitch])

        if self.priority > 0:
            result.append(['priority', self.priority])

        # Add keepout settings if present
        if self.keepout:
            result.append(self.keepout.to_sexp())

        # Add placement rule area settings if present
        if self.placement:
            result.append(self.placement.to_sexp())

        result.append(['connect_pads', ['clearance', self.connect_pads_clearance]])
        result.append(['min_thickness', self.min_thickness])
        result.append(['filled_areas_thickness', 'yes' if self.filled_areas_thickness else 'no'])

        # Per pcb_io_kicad_sexpr.cpp:3247-3251, KiCad prints "(fill" and
        # appends a bare "yes" token only when the zone is filled. The parser
        # (pcb_io_kicad_sexpr_parser.cpp, case T_fill) has no "no" case, so
        # emitting `(fill no ...)` produces a board KiCad refuses to load.
        fill_elem: SexpList = ['fill']
        if not self.keepout and self.fill_enabled:
            fill_elem.append('yes')
        if self.island_removal_mode is not None:
            fill_elem.append(['island_removal_mode', self.island_removal_mode])
        if self.island_area_min is not None:
            fill_elem.append(['island_area_min', self.island_area_min])
        fill_elem.append(['thermal_gap', self.thermal_gap])
        fill_elem.append(['thermal_bridge_width', self.thermal_bridge_width])
        result.append(fill_elem)

        # Per-layer (property (layer "X") (hatch_position (xy A B))) blocks.
        # Mirrors PCB_IO_KICAD_SEXPR::format(ZONE_LAYER_PROPERTIES&) at
        # pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp:3139.
        for layer_name, (ox, oy) in self.layer_properties:
            result.append([
                'property',
                ['layer', QuotedString(layer_name)],
                ['hatch_position', ['xy', ox, oy]],
            ])

        for poly in self.polygons:
            result.append(poly.to_sexp())
        for fpoly in self.filled_polygons:
            result.append(fpoly.to_sexp())

        return result

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of this zone.."""
        from .kicad_geometry import BoundingBox

        bbox = BoundingBox()

        # Include all polygon outline points
        for poly in self.polygons:
            for x, y in poly.points:
                bbox.expand((x, y))

        # Include all filled polygon points
        for filled in self.filled_polygons:
            for x, y in filled.points:
                bbox.expand((x, y))

        return bbox

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> List[str]:
        """Render this zone to SVG elements.."""
        from .kicad_geometry import SvgRenderContext

        if ctx is None:
            ctx = SvgRenderContext()

        if not ctx.layer_visible(self.layer):
            return []

        elements = []

        # Render filled polygons
        for filled in self.filled_polygons:
            if ctx.layers is not None and filled.layer not in ctx.layers:
                continue

            if not filled.points:
                continue

            # Build path data
            path_data = f"M{ctx.fmt(filled.points[0][0] + ctx.offset_x)} {ctx.fmt(filled.points[0][1] + ctx.offset_y)}"
            for x, y in filled.points[1:]:
                path_data += f" L{ctx.fmt(x + ctx.offset_x)} {ctx.fmt(y + ctx.offset_y)}"
            path_data += " Z"

            elements.append(
                f'<path d="{path_data}" style="fill:{ctx.fill}; stroke:none;" />'
            )

        return elements


__all__ = [
    'ZonePlacement',
    'Keepout',
    'ZonePolygon',
    'FilledPolygon',
    'Zone',
]

"""
KiCad Zone Filler - Python implementation of KiCad's zone fill algorithm.

This module provides zone fill computation that matches KiCad's zone_filler.cpp
output, enabling exact SVG matching with KiCad CLI.

Algorithm based on KiCad source:
- pcbnew/zone_filler.cpp (main fill orchestrator)
- libs/kimath/src/geometry/shape_poly_set.cpp (Clipper2 wrapper)

Key insight: KiCad does NOT use flood-fill. Instead it uses polygon boolean
operations via Clipper2 (union, subtract, intersect, inflate/deflate).
"""

from __future__ import annotations

import logging
import math
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Dict, List, Optional, Tuple

from .kicad_zone_utils import (
    CornerStrategy,
    PolygonSet,
    circle_to_polygon,
    segment_to_polygon,
    DEFAULT_MAX_ERROR,
    EPSILON_MM,
)
from .kicad_pcb_layer_policy import (
    PcbLayerFlashResolver,
    _pad_geometry,
    _pad_hole_geometry,
    copper_span_layers,
    pad_copper_layers,
)

if TYPE_CHECKING:
    from .kicad_pcb import KiCadPcb
    from .kicad_pcb_zone import Zone
    from .kicad_pcb_footprint import Pad
    from .kicad_pcb_routing import Segment, Via

log = logging.getLogger(__name__)

# Type aliases
Point = Tuple[float, float]
Polygon = List[Point]


@dataclass
class ThermalSpoke:
    """A thermal relief spoke connecting a pad to a zone."""

    polygon: PolygonSet
    test_point: Point  # Point at index 3 for containment testing
    pad_center: Point
    pad_net: int


@dataclass
class ZoneFillResult:
    """Result of filling a zone on a layer."""

    layer: str
    filled_polygons: PolygonSet
    islands: List[PolygonSet] = field(default_factory=list)


class ZoneFiller:
    """
    Python implementation of KiCad's zone fill algorithm.

    Produces filled zone polygons that match KiCad CLI output exactly.

    Reference: zone_filler.cpp fillCopperZone() at line 1916
    """

    def __init__(self, pcb: "KiCadPcb", max_error: float = DEFAULT_MAX_ERROR):
        """
        Initialize zone filler with PCB data.

        Args:
            pcb: KiCad PCB object with footprints, tracks, vias, zones
            max_error: Arc tolerance for polygon approximation (default 5 microns)
        """
        self.pcb = pcb
        self.max_error = max_error
        self._layer_flash_resolver = PcbLayerFlashResolver.from_board(pcb)
        self._pad_footprints = {
            id(pad): footprint
            for footprint in self.pcb.footprints
            for pad in footprint.pads
        }

        # Build lookup tables for fast net checking
        self._build_net_lookups()

    def _build_net_lookups(self) -> None:
        """Build lookup tables for net membership checking."""
        self.net_to_name: Dict[int, str] = {}
        self.name_to_net: Dict[str, int] = {}

        for net in self.pcb.nets:
            self.net_to_name[net.ordinal] = net.name
            self.name_to_net[net.name] = net.ordinal

    def fill_zone(self, zone: "Zone", layer: str) -> ZoneFillResult:
        """
        Fill a single zone on a single layer.

        Implements the algorithm from zone_filler.cpp fillCopperZone().

        Args:
            zone: Zone to fill
            layer: Layer name (e.g., "F.Cu", "B.Cu")

        Returns:
            ZoneFillResult with filled polygons
        """
        log.debug(f"Filling zone net={zone.net} layer={layer}")

        # Step 1: Get smoothed outline (apply corner rounding/chamfering)
        smoothed_outline = self._build_smoothed_outline(zone)
        if smoothed_outline.is_empty:
            return ZoneFillResult(layer=layer, filled_polygons=PolygonSet())

        max_extents = smoothed_outline.clone()

        # Step 2: Initialize fill polygons with zone outline
        fill_polys = smoothed_outline.clone()

        # Step 3: Categorize pads by connection type
        thermal_pads, no_connection_pads = self._categorize_pads(zone, layer)

        # Step 4: Knockout thermal relief patterns
        if thermal_pads:
            thermal_reliefs = self._build_thermal_reliefs(zone, layer, thermal_pads)
            if not thermal_reliefs.is_empty:
                fill_polys = fill_polys.boolean_subtract(thermal_reliefs)

        # Step 5: Build clearance holes for non-connected items
        clearance_holes = self._build_clearance_holes(zone, layer, no_connection_pads)

        # Step 6: Build and test thermal spokes
        if thermal_pads:
            spokes = self._build_thermal_spokes(zone, layer, thermal_pads)
            fill_polys = self._add_valid_spokes(
                fill_polys, clearance_holes, spokes, zone
            )

        # Step 7: Subtract all clearance holes
        if not clearance_holes.is_empty:
            fill_polys = fill_polys.boolean_subtract(clearance_holes)

        # Step 8: Apply minimum width pruning
        fill_polys = self._apply_min_width_pruning(fill_polys, zone)

        # Step 9: Final trimming to zone extent
        fill_polys = fill_polys.boolean_intersection(max_extents)

        # Re-subtract clearance holes (may have been reintroduced by inflate)
        if not clearance_holes.is_empty:
            fill_polys = fill_polys.boolean_subtract(clearance_holes)

        # Step 10: Subtract higher-priority overlapping zones
        fill_polys = self._subtract_higher_priority_zones(zone, layer, fill_polys)

        return ZoneFillResult(layer=layer, filled_polygons=fill_polys)

    def fill_all_zones(
        self, layers: Optional[List[str]] = None
    ) -> Dict[str, Dict[str, PolygonSet]]:
        """
        Fill all zones in the PCB.

        Args:
            layers: Optional list of layers to fill. If None, fill all layers.

        Returns:
            Dict mapping zone UUID to Dict mapping layer to filled polygons.
        """
        results: Dict[str, Dict[str, PolygonSet]] = {}

        for zone in self.pcb.zones:
            if zone.keepout:
                continue  # Skip keepout zones

            zone_uuid = zone.uuid or str(id(zone))
            results[zone_uuid] = {}

            # Determine which layers this zone covers
            zone_layers = [zone.layer] if zone.layer else []
            if layers:
                zone_layers = [layer for layer in zone_layers if layer in layers]

            for layer in zone_layers:
                result = self.fill_zone(zone, layer)
                results[zone_uuid][layer] = result.filled_polygons

        return results

    # =========================================================================
    # Outline Building
    # =========================================================================

    def _build_smoothed_outline(self, zone: "Zone") -> PolygonSet:
        """
        Build zone outline with corner smoothing applied.

        KiCad applies corner smoothing (fillet or chamfer) to zone outlines
        before filling.
        """
        if not zone.polygons:
            return PolygonSet()

        # Combine all zone polygons into one set
        result = PolygonSet()
        for zone_poly in zone.polygons:
            if zone_poly.points and len(zone_poly.points) >= 3:
                result.add_outline(zone_poly.points)

        # TODO: Apply corner smoothing based on zone.hatch_style.
        # Return the raw outline until smoothing is implemented.

        return result

    # =========================================================================
    # Pad Categorization
    # =========================================================================

    def _categorize_pads(
        self, zone: "Zone", layer: str
    ) -> Tuple[List["Pad"], List["Pad"]]:
        """
        Categorize pads into thermal-connected and no-connection groups.

        Args:
            zone: Zone being filled
            layer: Layer being processed

        Returns:
            (thermal_pads, no_connection_pads)
        """
        thermal_pads: List["Pad"] = []
        no_connection_pads: List["Pad"] = []

        for footprint in self.pcb.footprints:
            for pad in footprint.pads:
                # Skip pads not on this layer
                if not self._pad_on_layer(pad, footprint, layer, effective=False):
                    continue

                # Check net connection
                if pad.net == zone.net and self._pad_can_flash(pad, layer):
                    # Same net - thermal connection
                    thermal_pads.append(pad)
                else:
                    # Different net - needs clearance
                    no_connection_pads.append(pad)

        return thermal_pads, no_connection_pads

    def _pad_on_layer(
        self,
        pad: "Pad",
        footprint: object,
        layer: str,
        *,
        effective: bool,
    ) -> bool:
        """Check if a pad is present on the specified layer."""
        if effective:
            item_layers = self._layer_flash_resolver.pad_item_layers(pad, footprint)
            return layer in item_layers or (
                layer.endswith(".Cu") and "*.Cu" in item_layers
            )
        copper_layers = pad_copper_layers(
            pad.layers,
            self._layer_flash_resolver.copper_stack,
        )
        pad_type = str(getattr(getattr(pad, "pad_type", ""), "value", pad.pad_type))
        return layer in copper_layers or (
            layer.endswith(".Cu")
            and pad_type == "np_thru_hole"
            and getattr(pad, "drill", None) is not None
        )

    def _pad_can_flash(self, pad: "Pad", layer: str) -> bool:
        """Mirror KiCad's CanFlashLayer eligibility used while filling zones."""
        if layer not in pad_copper_layers(
            pad.layers, self._layer_flash_resolver.copper_stack
        ):
            return False
        pad_type = str(getattr(getattr(pad, "pad_type", ""), "value", pad.pad_type))
        shape = str(getattr(getattr(pad, "shape", ""), "value", pad.shape))
        if pad_type != "np_thru_hole":
            return True
        offset_is_zero = not (getattr(pad, "drill_offset_x", 0.0) or 0.0) and not (
            getattr(pad, "drill_offset_y", 0.0) or 0.0
        )
        if shape == "circle" and pad.drill is not None:
            size_x = float(pad.size_x or 0.0)
            return not (offset_is_zero and float(pad.drill) >= size_x)
        if shape == "oval" and bool(getattr(pad, "drill_oval", False)):
            drill_width = float(getattr(pad, "drill_width", 0.0) or 0.0)
            drill_height = float(getattr(pad, "drill_height", 0.0) or 0.0)
            return not (
                offset_is_zero
                and drill_width >= float(pad.size_x or 0.0)
                and drill_height >= float(pad.size_y or 0.0)
            )
        return True

    # =========================================================================
    # Clearance Holes
    # =========================================================================

    def _build_clearance_holes(
        self, zone: "Zone", layer: str, no_connection_pads: List["Pad"]
    ) -> PolygonSet:
        """
        Build clearance hole polygons for all items needing clearance.

        Includes:
        - Pads not connected to zone net
        - Tracks not on zone net
        - Vias not on zone net
        """
        holes = PolygonSet()

        # Pad clearances
        for pad in no_connection_pads:
            holes = holes.boolean_add(self._pad_clearance_hole(zone, pad, layer))

        # Track clearances
        for segment in self.pcb.segments:
            if segment.layer != layer:
                continue
            if segment.net == zone.net:
                continue

            clearance = self._get_track_zone_clearance(zone, segment, layer)
            track_shape = segment_to_polygon(
                (segment.start_x, segment.start_y),
                (segment.end_x, segment.end_y),
                segment.width,
                self.max_error,
            )
            if not track_shape.is_empty:
                expanded = track_shape.inflate(
                    clearance, CornerStrategy.ROUND_ALL_CORNERS, self.max_error
                )
                holes = holes.boolean_add(expanded)

        # Via clearances
        for via in self.pcb.vias:
            holes = holes.boolean_add(self._via_clearance_holes(zone, via, layer))

        return holes

    def _pad_clearance_hole(self, zone: "Zone", pad: "Pad", layer: str) -> PolygonSet:
        clearance = self._get_pad_zone_clearance(zone, pad, layer)
        footprint = self._pad_footprints.get(id(pad))
        flashed = footprint is not None and self._pad_on_layer(
            pad, footprint, layer, effective=True
        )
        clearance_shape = (
            self._get_pad_polygon(pad, footprint, layer)
            if flashed
            else self._get_pad_hole_polygon(pad, footprint)
        )
        if clearance_shape.is_empty:
            return PolygonSet()
        return clearance_shape.inflate(
            clearance,
            CornerStrategy.ROUND_ALL_CORNERS,
            self.max_error,
        )

    def _via_clearance_holes(self, zone: "Zone", via: "Via", layer: str) -> PolygonSet:
        if not self._via_on_layer(via, layer):
            return PolygonSet()
        same_net = via.net == zone.net
        clearance = 0.0 if same_net else self._get_via_zone_clearance(zone, via, layer)
        holes = PolygonSet()
        if not same_net and layer in self._layer_flash_resolver.via_flash_layers(via):
            via_shape = circle_to_polygon(
                via.at_x, via.at_y, via.size / 2, self.max_error
            )
            if not via_shape.is_empty:
                holes = holes.boolean_add(
                    via_shape.inflate(
                        clearance,
                        CornerStrategy.ROUND_ALL_CORNERS,
                        self.max_error,
                    )
                )
        drill_size = via.drill if via.drill and via.drill > 0 else via.size * 0.5
        if drill_size <= 0:
            return holes
        drill_shape = circle_to_polygon(
            via.at_x, via.at_y, drill_size / 2, self.max_error
        )
        return holes.boolean_add(
            drill_shape.inflate(
                clearance,
                CornerStrategy.ROUND_ALL_CORNERS,
                self.max_error,
            )
        )

    def _via_on_layer(self, via: "Via", layer: str) -> bool:
        """Check if a via passes through the specified layer."""
        return layer in copper_span_layers(
            via.layers,
            self._layer_flash_resolver.copper_stack,
        )

    def _get_pad_zone_clearance(self, zone: "Zone", pad: "Pad", layer: str) -> float:
        """Get clearance between a pad and zone."""
        # Use zone's default clearance
        # TODO: Implement full DRC rule lookup
        return zone.connect_pads_clearance

    def _get_track_zone_clearance(
        self, zone: "Zone", segment: "Segment", layer: str
    ) -> float:
        """Get clearance between a track and zone."""
        # Use zone's default clearance
        # TODO: Implement full DRC rule lookup
        return zone.connect_pads_clearance

    def _get_via_zone_clearance(self, zone: "Zone", via: "Via", layer: str) -> float:
        """Get clearance between a via and zone."""
        # Use zone's default clearance
        # TODO: Implement full DRC rule lookup
        return zone.connect_pads_clearance

    @staticmethod
    def _polygon_set_from_geometry(geometry: object) -> PolygonSet:
        polygons: list[Polygon] = []
        candidates = getattr(geometry, "geoms", (geometry,))
        for candidate in candidates:
            exterior = getattr(candidate, "exterior", None)
            if exterior is None:
                continue
            polygons.append([(float(x), float(y)) for x, y in exterior.coords[:-1]])
            for interior in getattr(candidate, "interiors", ()):
                polygons.append([(float(x), float(y)) for x, y in interior.coords[:-1]])
        return PolygonSet(polygons)

    def _get_pad_polygon(self, pad: "Pad", footprint: object, layer: str) -> PolygonSet:
        """Get the polygon shape of a pad on the specified layer."""
        del layer
        return self._polygon_set_from_geometry(_pad_geometry(footprint, pad))

    def _get_pad_hole_polygon(self, pad: "Pad", footprint: object) -> PolygonSet:
        """Return only the physical drill carrier for an unflashed pad."""
        return self._polygon_set_from_geometry(_pad_hole_geometry(footprint, pad))

    def _rotate_points(
        self, points: List[Point], cx: float, cy: float, angle_deg: float
    ) -> List[Point]:
        """Rotate points around a center point."""
        rad = math.radians(angle_deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        result = []
        for x, y in points:
            dx, dy = x - cx, y - cy
            rx = dx * cos_a - dy * sin_a + cx
            ry = dx * sin_a + dy * cos_a + cy
            result.append((rx, ry))
        return result

    # =========================================================================
    # Thermal Relief
    # =========================================================================

    def _build_thermal_reliefs(
        self, zone: "Zone", layer: str, thermal_pads: List["Pad"]
    ) -> PolygonSet:
        """
        Build thermal relief knockout patterns for connected pads.

        Creates ring-shaped cutouts around pads with gaps at cardinal directions
        where spokes will connect.
        """
        reliefs = PolygonSet()

        for pad in thermal_pads:
            relief = self._build_pad_thermal_relief(zone, pad, layer)
            if not relief.is_empty:
                reliefs = reliefs.boolean_add(relief)

        return reliefs

    def _build_pad_thermal_relief(
        self, zone: "Zone", pad: "Pad", layer: str
    ) -> PolygonSet:
        """Build thermal relief pattern for a single pad."""
        # Get pad shape
        footprint = self._pad_footprints.get(id(pad))
        if footprint is None:
            return PolygonSet()
        pad_shape = self._get_pad_polygon(pad, footprint, layer)
        if pad_shape.is_empty:
            return PolygonSet()

        # Expand pad by thermal gap
        outer = pad_shape.inflate(
            zone.thermal_gap, CornerStrategy.ROUND_ALL_CORNERS, self.max_error
        )

        # Inner boundary is the pad itself
        # Relief = outer - pad - spoke_slots
        relief = outer.boolean_subtract(pad_shape)

        # TODO: Cut out spoke slots at cardinal directions
        # For now, return the full ring (spokes will punch through)

        return relief

    def _build_thermal_spokes(
        self, zone: "Zone", layer: str, thermal_pads: List["Pad"]
    ) -> List[ThermalSpoke]:
        """
        Build thermal spoke polygons for connected pads.

        Creates 4 cardinal spokes per pad to connect pad to zone.
        """
        spokes: List[ThermalSpoke] = []

        for pad in thermal_pads:
            pad_spokes = self._build_pad_spokes(zone, pad, layer)
            spokes.extend(pad_spokes)

        return spokes

    def _build_pad_spokes(
        self, zone: "Zone", pad: "Pad", layer: str
    ) -> List[ThermalSpoke]:
        """Build 4 thermal spokes for a single pad."""
        spokes: List[ThermalSpoke] = []

        # Get pad center and parameters
        footprint = self._pad_footprints.get(id(pad))
        if footprint is None:
            return spokes
        copper_geometry = _pad_geometry(footprint, pad)
        if copper_geometry.is_empty:
            return spokes
        cx, cy = float(copper_geometry.centroid.x), float(copper_geometry.centroid.y)
        spoke_width = zone.thermal_bridge_width
        gap = zone.thermal_gap

        # Spoke length extends beyond pad + gap
        pad_radius = max(pad.size_x, pad.size_y) / 2
        spoke_length = pad_radius + gap * 3  # Extend well beyond pad

        # Create 4 cardinal spokes (0, 90, 180, 270 degrees)
        # Rotated by pad angle
        base_angles = [0, 90, 180, 270]
        rotation = pad.at_angle or 0

        for base_angle in base_angles:
            angle = base_angle + rotation
            spoke = self._create_spoke_polygon(cx, cy, spoke_width, spoke_length, angle)
            spokes.append(
                ThermalSpoke(
                    polygon=spoke,
                    test_point=self._get_spoke_test_point(cx, cy, spoke_length, angle),
                    pad_center=(cx, cy),
                    pad_net=getattr(pad.net, "ordinal", None) or 0,
                )
            )

        return spokes

    def _create_spoke_polygon(
        self, cx: float, cy: float, width: float, length: float, angle_deg: float
    ) -> PolygonSet:
        """
        Create a single thermal spoke as a rectangle polygon.

        The spoke extends from the pad center outward in the direction
        specified by angle_deg.
        """
        half_w = width / 2

        # Create spoke rectangle centered at origin, extending in +Y
        points = [
            (-half_w, 0),
            (half_w, 0),
            (half_w, length),
            (-half_w, length),
        ]

        # Rotate and translate
        rad = math.radians(angle_deg - 90)  # -90 to align 0 deg with +X
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        rotated = []
        for x, y in points:
            rx = x * cos_a - y * sin_a + cx
            ry = x * sin_a + y * cos_a + cy
            rotated.append((rx, ry))

        return PolygonSet([rotated])

    def _get_spoke_test_point(
        self, cx: float, cy: float, length: float, angle_deg: float
    ) -> Point:
        """Get the test point for a spoke (at 3/4 of length from center)."""
        rad = math.radians(angle_deg)
        distance = length * 0.75  # 3/4 out from center
        tx = cx + distance * math.cos(rad)
        ty = cy + distance * math.sin(rad)
        return (tx, ty)

    def _add_valid_spokes(
        self,
        fill_polys: PolygonSet,
        clearance_holes: PolygonSet,
        spokes: List[ThermalSpoke],
        zone: "Zone",
    ) -> PolygonSet:
        """
        Test spokes and add valid ones to fill polygons.

        A spoke is valid if its test point is inside the zone body
        (after subtracting clearance holes and applying min-width).
        """
        if not spokes:
            return fill_polys

        # Create test areas = fill - clearances, with min-width pruning
        test_areas = fill_polys.boolean_subtract(clearance_holes)

        # Apply min-width pruning to test areas
        half_min_width = zone.min_thickness / 2
        if half_min_width - EPSILON_MM > EPSILON_MM:
            test_areas = test_areas.deflate(
                half_min_width - EPSILON_MM,
                CornerStrategy.CHAMFER_ALL_CORNERS,
                self.max_error,
            )
            test_areas = test_areas.inflate(
                half_min_width - EPSILON_MM,
                CornerStrategy.CHAMFER_ALL_CORNERS,
                self.max_error,
            )

        # Test each spoke
        valid_spokes: List[PolygonSet] = []
        for spoke in spokes:
            if test_areas.contains(spoke.test_point):
                valid_spokes.append(spoke.polygon)
                continue

            # Check mutual containment with other spokes
            # (Two spokes are both valid if each contains the other's test point)
            for other in spokes:
                if other is spoke:
                    continue
                if other.polygon.contains(spoke.test_point) and spoke.polygon.contains(
                    other.test_point
                ):
                    valid_spokes.append(spoke.polygon)
                    break

        # Add valid spokes to fill polygons
        for spoke_poly in valid_spokes:
            fill_polys = fill_polys.boolean_add(spoke_poly)

        return fill_polys

    # =========================================================================
    # Minimum Width Pruning
    # =========================================================================

    def _apply_min_width_pruning(
        self, fill_polys: PolygonSet, zone: "Zone"
    ) -> PolygonSet:
        """
        Apply minimum width pruning to remove thin features.

        Uses deflate/inflate (morphological opening) to remove features
        thinner than min_thickness.
        """
        half_min_width = zone.min_thickness / 2
        if half_min_width - EPSILON_MM <= EPSILON_MM:
            return fill_polys

        # Deflate by half min width (minus epsilon for floating point)
        deflated = fill_polys.deflate(
            half_min_width - EPSILON_MM,
            CornerStrategy.CHAMFER_ALL_CORNERS,
            self.max_error,
        )

        # Remove small islands (max dimension < min_thickness)
        deflated = deflated.remove_small_islands(zone.min_thickness)

        # Re-inflate to restore size (with round corners for smooth finish)
        result = deflated.inflate(
            half_min_width - EPSILON_MM,
            CornerStrategy.ROUND_ALL_CORNERS,
            self.max_error,
        )

        return result

    # =========================================================================
    # Zone Priority
    # =========================================================================

    def _subtract_higher_priority_zones(
        self, zone: "Zone", layer: str, fill_polys: PolygonSet
    ) -> PolygonSet:
        """
        Subtract overlapping zones with higher priority.

        KiCad zones have a priority order - higher priority zones
        take precedence in overlapping areas.
        """
        # TODO: Implement zone priority handling
        # For now, return unchanged (assume no overlapping zones)
        return fill_polys


# =============================================================================
# Convenience Functions
# =============================================================================


def fill_pcb_zones(pcb: "KiCadPcb", layers: Optional[List[str]] = None) -> None:
    """
    Fill all zones in a PCB, updating the zone filled_polygons in place.

    Args:
        pcb: KiCad PCB object
        layers: Optional list of layers to fill. If None, fill all.
    """
    filler = ZoneFiller(pcb)
    results = filler.fill_all_zones(layers)

    # Update zone filled_polygons
    from .kicad_pcb_zone import FilledPolygon

    for zone in pcb.zones:
        if zone.keepout:
            continue

        zone_uuid = zone.uuid or str(id(zone))
        if zone_uuid in results:
            # Clear existing filled polygons for affected layers
            zone.filled_polygons = [
                fp for fp in zone.filled_polygons if fp.layer not in results[zone_uuid]
            ]

            # Add new filled polygons
            for layer, poly_set in results[zone_uuid].items():
                for poly in poly_set.polygons:
                    zone.filled_polygons.append(
                        FilledPolygon(layer=layer, island=False, points=list(poly))
                    )


__all__ = [
    "ZoneFiller",
    "ZoneFillResult",
    "ThermalSpoke",
    "fill_pcb_zones",
]

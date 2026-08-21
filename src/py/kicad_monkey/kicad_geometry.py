"""Neutral geometry structures for KiCad rendering.

This module defines geometry primitives that are independent of any specific
output format (SVG, OpenGL, etc.). The renderer produces these structures,
and serializers convert them to various output formats.

Architecture:
    Input (KiCad expr/IPC API) -> Renderer -> Geometry -> Serializers (SVG, OpenGL, etc.)

Design Notes for C++ Porting:
    - All dataclasses use slots=True for struct-like memory layout
    - Explicit type hints on all fields and methods
    - Simple POD-like structures where possible
    - Avoid Python-specific idioms (dynamic typing, **kwargs, etc.)
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass, field
from enum import IntEnum
from typing import Dict, List, Optional, Tuple, Iterator, Any, Protocol, runtime_checkable

from .kicad_base import FRONT_SILKSCREEN_LAYER

# =============================================================================
# Basic Types
# =============================================================================

# Simple 2D point as tuple - maps to std::pair<double, double> in C++
Point = Tuple[float, float]


@dataclass(slots=True)
class Vec2:
    """2D vector/point structure.

    Alternative to Point tuple for more explicit C++ mapping.
    Maps to: struct Vec2 { double x; double y; };
    """
    x: float = 0.0
    y: float = 0.0

    def to_tuple(self) -> Point:
        """Convert to tuple form."""
        return (self.x, self.y)

    @classmethod
    def from_tuple(cls, pt: Point) -> Vec2:
        """Create from tuple."""
        return cls(pt[0], pt[1])


class HAlign(IntEnum):
    """Horizontal text alignment.

    Maps to: enum class HAlign : int { LEFT = 0, CENTER = 1, RIGHT = 2 };
    """
    LEFT = 0
    CENTER = 1
    RIGHT = 2


class VAlign(IntEnum):
    """Vertical text alignment.

    Maps to: enum class VAlign : int { TOP = 0, CENTER = 1, BOTTOM = 2 };
    """
    TOP = 0
    CENTER = 1
    BOTTOM = 2


# =============================================================================
# Geometry Primitives
# =============================================================================

@dataclass(slots=True)
class BoundingBox:
    """Axis-aligned bounding box in mm.

    Maps to: struct BoundingBox { double min_x, min_y, max_x, max_y; };
    """
    min_x: float = float('inf')
    min_y: float = float('inf')
    max_x: float = float('-inf')
    max_y: float = float('-inf')

    def get_width(self) -> float:
        """Get box width."""
        if self.is_valid():
            return self.max_x - self.min_x
        return 0.0

    def get_height(self) -> float:
        """Get box height."""
        if self.is_valid():
            return self.max_y - self.min_y
        return 0.0

    def get_center(self) -> Point:
        """Get center point."""
        return ((self.min_x + self.max_x) / 2.0, (self.min_y + self.max_y) / 2.0)

    def is_valid(self) -> bool:
        """Check if bounds are valid (min <= max)."""
        return self.min_x <= self.max_x and self.min_y <= self.max_y

    def expand(self, point: Point) -> None:
        """Expand bounds to include a point (mutates self)."""
        x: float = point[0]
        y: float = point[1]
        if x < self.min_x:
            self.min_x = x
        if x > self.max_x:
            self.max_x = x
        if y < self.min_y:
            self.min_y = y
        if y > self.max_y:
            self.max_y = y

    def expand_by(self, margin: float) -> BoundingBox:
        """Return new bounds expanded by margin on all sides."""
        return BoundingBox(
            min_x=self.min_x - margin,
            min_y=self.min_y - margin,
            max_x=self.max_x + margin,
            max_y=self.max_y + margin
        )

    def expand_by_xy(self, margin_x: float, margin_y: float) -> BoundingBox:
        """Return new bounds expanded by different X and Y margins.

        This is useful for proportional margins (e.g., KiCad's 1.2x multiplier).
        """
        return BoundingBox(
            min_x=self.min_x - margin_x,
            min_y=self.min_y - margin_y,
            max_x=self.max_x + margin_x,
            max_y=self.max_y + margin_y
        )

    def union(self, other: BoundingBox) -> BoundingBox:
        """Return union of two bounding boxes."""
        return BoundingBox(
            min_x=min(self.min_x, other.min_x),
            min_y=min(self.min_y, other.min_y),
            max_x=max(self.max_x, other.max_x),
            max_y=max(self.max_y, other.max_y)
        )

    def merge(self, other: BoundingBox) -> None:
        """Merge another bounding box into this one (mutates self)."""
        if other.is_valid():
            if other.min_x < self.min_x:
                self.min_x = other.min_x
            if other.min_y < self.min_y:
                self.min_y = other.min_y
            if other.max_x > self.max_x:
                self.max_x = other.max_x
            if other.max_y > self.max_y:
                self.max_y = other.max_y

    @property
    def is_empty(self) -> bool:
        """Check if bounds are empty (not yet expanded)."""
        return self.min_x == float('inf')

    @staticmethod
    def from_points(points: List[Point]) -> BoundingBox:
        """Create bounding box from list of points."""
        box = BoundingBox()
        for p in points:
            box.expand(p)
        return box

    # Property aliases for compatibility
    @property
    def width(self) -> float:
        return self.get_width()

    @property
    def height(self) -> float:
        return self.get_height()

    @property
    def center(self) -> Point:
        return self.get_center()


@dataclass(slots=True)
class Contour:
    """A closed polygon contour (list of points in mm).

    The contour is assumed to be closed - the last point connects back to the first.
    Points are stored in order (either CW or CCW depending on whether it's an
    outer boundary or a hole).

    Maps to: struct Contour { std::vector<Point> points; };
    """
    points: List[Point] = field(default_factory=list)

    def get_bounds(self) -> BoundingBox:
        """Get bounding box of this contour."""
        return BoundingBox.from_points(self.points)

    def is_closed(self) -> bool:
        """Check if contour is explicitly closed (last == first)."""
        if len(self.points) < 2:
            return False
        return self.points[0] == self.points[-1]

    def close(self) -> None:
        """Ensure contour is explicitly closed (mutates self)."""
        if len(self.points) > 0 and not self.is_closed():
            self.points.append(self.points[0])

    def point_count(self) -> int:
        """Get number of points."""
        return len(self.points)

    def get_point(self, index: int) -> Point:
        """Get point at index."""
        return self.points[index]

    # Python iteration support
    def __len__(self) -> int:
        return len(self.points)

    def __iter__(self) -> Iterator[Point]:
        return iter(self.points)

    def __getitem__(self, idx: int) -> Point:
        return self.points[idx]

    # Property alias for compatibility
    @property
    def bounds(self) -> BoundingBox:
        return self.get_bounds()


@dataclass(slots=True)
class RenderedGeometry:
    """Result of rendering - a collection of polygon contours.

    This is the neutral geometry form that can be serialized to any output format.
    Contains both the geometry and metadata about what was rendered.

    Maps to:
        struct RenderedGeometry {
            std::vector<Contour> contours;
            std::string source_text;
            std::string layer;
            bool is_knockout;
        };
    """
    contours: List[Contour] = field(default_factory=list)
    #: Contiguous per-glyph (and per-overbar) contour counts in draw order,
    #: summing to ``len(contours)``.  Empty when the producer does not track
    #: glyph grouping.
    contour_group_sizes: List[int] = field(default_factory=list)
    source_text: str = ""
    layer: str = ""
    is_knockout: bool = False

    def get_bounds(self) -> BoundingBox:
        """Get combined bounding box of all contours."""
        if len(self.contours) == 0:
            return BoundingBox()

        combined: BoundingBox = self.contours[0].get_bounds()
        for i in range(1, len(self.contours)):
            combined = combined.union(self.contours[i].get_bounds())
        return combined

    def get_point_count(self) -> int:
        """Total number of points across all contours."""
        total: int = 0
        for contour in self.contours:
            total += contour.point_count()
        return total

    def get_contour_count(self) -> int:
        """Number of contours."""
        return len(self.contours)

    def add_contour(self, points: List[Point]) -> None:
        """Add a contour from a list of points.

        Degenerate (<3 point) contours are retained: KiCad keeps rings that
        collapse below three points in the glyph polygon set and publishes
        them in the saved render cache, so downstream cache consumers decide
        whether to draw or drop them.
        """
        if points:
            self.contours.append(Contour(points=list(points)))

    def add_contour_obj(self, contour: Contour) -> None:
        """Add an existing Contour object."""
        if contour.point_count() >= 3:
            self.contours.append(contour)

    def transform(
        self,
        translate_x: float = 0.0,
        translate_y: float = 0.0,
        scale: float = 1.0,
        rotate_deg: float = 0.0,
        mirror_x: bool = False
    ) -> RenderedGeometry:
        """Return a transformed copy of this geometry."""
        rad: float = math.radians(rotate_deg)
        cos_a: float = math.cos(rad)
        sin_a: float = math.sin(rad)

        new_contours: List[Contour] = []

        for contour in self.contours:
            new_points: List[Point] = []

            for point in contour.points:
                x: float = point[0]
                y: float = point[1]

                # Mirror
                if mirror_x:
                    x = -x

                # Scale
                x = x * scale
                y = y * scale

                # Rotate (KiCad uses CW rotation)
                if rotate_deg != 0.0:
                    rx: float = x * cos_a + y * sin_a
                    ry: float = y * cos_a - x * sin_a
                    x = rx
                    y = ry

                # Translate
                x = x + translate_x
                y = y + translate_y

                new_points.append((x, y))

            new_contours.append(Contour(points=new_points))

        return RenderedGeometry(
            contours=new_contours,
            source_text=self.source_text,
            layer=self.layer,
            is_knockout=self.is_knockout
        )

    def merge(self, other: RenderedGeometry) -> None:
        """Merge another geometry into this one (mutates self)."""
        for contour in other.contours:
            self.contours.append(contour)

    # Property aliases for compatibility
    @property
    def bounds(self) -> BoundingBox:
        return self.get_bounds()

    @property
    def point_count(self) -> int:
        return self.get_point_count()

    @property
    def contour_count(self) -> int:
        return self.get_contour_count()


# =============================================================================
# Text Parameters
# =============================================================================

@dataclass(slots=True)
class TextParams:
    """Parameters for a text object - unified input format.

    This is the single source of truth for text parameters. Use the factory
    methods to create from various input sources (KiCad expressions, IPC API, etc.)

    Maps to:
        struct TextParams {
            std::string text;
            std::string font_name;
            double size_x, size_y;
            double position_x, position_y;
            double angle;
            bool bold, italic, mirrored;
            HAlign h_align;
            VAlign v_align;
            double stroke_width;
            double line_spacing;
            bool knockout;
            std::string layer;
            std::vector<std::vector<Point>> render_cache;
        };
    """
    text: str
    font_name: str = "Arial"
    size_x: float = 1.0
    size_y: float = 1.0
    position_x: float = 0.0
    position_y: float = 0.0
    angle: float = 0.0
    bold: bool = False
    italic: bool = False
    mirrored: bool = False
    h_align: HAlign = HAlign.CENTER
    v_align: VAlign = VAlign.CENTER
    stroke_width: float = 0.0
    line_spacing: float = 1.0
    knockout: bool = False
    layer: str = FRONT_SILKSCREEN_LAYER
    render_cache: List[List[Point]] = field(default_factory=list)

    # =========================================================================
    # Factory Methods - Input Sources
    # =========================================================================

    @staticmethod
    def create_default(text: str) -> TextParams:
        """Create with default parameters."""
        return TextParams(text=text)

    @staticmethod
    def from_dict(d: Dict[str, Any]) -> TextParams:
        """Create from a dictionary."""
        # Extract values with explicit type conversion
        text: str = str(d.get('text', ''))
        font_name: str = str(d.get('font_name', 'Arial'))

        # Size - support both size_x/size_y and single 'size'
        default_size: float = float(d.get('size', 1.0))
        size_x: float = float(d.get('size_x', default_size))
        size_y: float = float(d.get('size_y', default_size))

        # Position - support both position_x/position_y and x/y
        position_x: float = float(d.get('position_x', d.get('x', 0.0)))
        position_y: float = float(d.get('position_y', d.get('y', 0.0)))

        angle: float = float(d.get('angle', 0.0))
        bold: bool = bool(d.get('bold', False))
        italic: bool = bool(d.get('italic', False))
        mirrored: bool = bool(d.get('mirrored', False))
        h_align: HAlign = HAlign(int(d.get('h_align', 1)))
        v_align: VAlign = VAlign(int(d.get('v_align', 1)))
        stroke_width: float = float(d.get('stroke_width', 0.0))
        line_spacing: float = float(d.get('line_spacing', 1.0))
        knockout: bool = bool(d.get('knockout', False))
        layer: str = str(d.get('layer', FRONT_SILKSCREEN_LAYER))
        render_cache: List[List[Point]] = d.get('render_cache', [])

        return TextParams(
            text=text,
            font_name=font_name,
            size_x=size_x,
            size_y=size_y,
            position_x=position_x,
            position_y=position_y,
            angle=angle,
            bold=bold,
            italic=italic,
            mirrored=mirrored,
            h_align=h_align,
            v_align=v_align,
            stroke_width=stroke_width,
            line_spacing=line_spacing,
            knockout=knockout,
            layer=layer,
            render_cache=render_cache,
        )

    @staticmethod
    def from_kicad_expression(expr: str) -> TextParams:
        """Parse from KiCad S-expression string.

        Example input:
            (gr_text "TEST" (at 5 2.5) (layer "F.SilkS")
              (effects (font (face "Arial") (size 1 1) (thickness 0.15))
                       (justify center)))
        """
        # Extract text content
        text_match = re.search(r'\(gr_text\s+"([^"]*)"', expr)
        text: str = text_match.group(1) if text_match else ""

        # Extract position
        at_match = re.search(r'\(at\s+([\d.-]+)\s+([\d.-]+)(?:\s+([\d.-]+))?\)', expr)
        position_x: float = float(at_match.group(1)) if at_match else 0.0
        position_y: float = float(at_match.group(2)) if at_match else 0.0
        angle: float = float(at_match.group(3)) if (at_match and at_match.group(3)) else 0.0

        # Extract layer and knockout
        layer_match = re.search(r'\(layer\s+"([^"]+)"(\s+knockout)?\)', expr)
        layer: str = layer_match.group(1) if layer_match else FRONT_SILKSCREEN_LAYER
        knockout: bool = (layer_match.group(2) is not None) if layer_match else False

        # Extract font info
        font_match = re.search(r'\(font(?:\s*\(face\s+"([^"]+)"\))?', expr)
        font_name: str = font_match.group(1) if (font_match and font_match.group(1)) else "Arial"

        size_match = re.search(r'\(size\s+([\d.]+)\s+([\d.]+)\)', expr)
        size_x: float = float(size_match.group(1)) if size_match else 1.0
        size_y: float = float(size_match.group(2)) if size_match else 1.0

        thickness_match = re.search(r'\(thickness\s+([\d.]+)\)', expr)
        stroke_width: float = float(thickness_match.group(1)) if thickness_match else 0.15
        line_spacing_match = re.search(r'\(line_spacing\s+([\d.]+)\)', expr)
        line_spacing: float = float(line_spacing_match.group(1)) if line_spacing_match else 1.0

        # Extract alignment
        h_align: HAlign = HAlign.CENTER
        v_align: VAlign = VAlign.CENTER
        justify_match = re.search(r'\(justify\s+(\w+)(?:\s+(\w+))?(?:\s+(\w+))?\)', expr)
        if justify_match:
            groups = [justify_match.group(1), justify_match.group(2), justify_match.group(3)]
            for g in groups:
                if g is not None:
                    g_lower: str = g.lower()
                    if g_lower == 'left':
                        h_align = HAlign.LEFT
                    elif g_lower == 'right':
                        h_align = HAlign.RIGHT
                    elif g_lower == 'top':
                        v_align = VAlign.TOP
                    elif g_lower == 'bottom':
                        v_align = VAlign.BOTTOM

        # Check for mirror, bold, italic
        mirrored: bool = 'mirror' in expr
        bold: bool = '(bold yes)' in expr or '(bold)' in expr
        italic: bool = '(italic yes)' in expr or '(italic)' in expr

        return TextParams(
            text=text,
            font_name=font_name,
            size_x=size_x,
            size_y=size_y,
            position_x=position_x,
            position_y=position_y,
            angle=angle,
            bold=bold,
            italic=italic,
            mirrored=mirrored,
            h_align=h_align,
            v_align=v_align,
            stroke_width=stroke_width,
            line_spacing=line_spacing,
            knockout=knockout,
            layer=layer,
        )

    @staticmethod
    def from_ipc_api(text_obj: Any) -> TextParams:
        """Create from KiCad IPC API text object (kipy).

        This handles the kipy library's text object representation from the
        IPC API connection to KiCad.
        """
        def get_attr_safe(obj: Any, name: str, default: Any) -> Any:
            """Safely get attribute with default."""
            if obj is None:
                return default
            if hasattr(obj, name):
                val = getattr(obj, name)
                if val is not None:
                    return val
            return default

        # Get text
        text: str = str(get_attr_safe(text_obj, 'text', ''))
        if text == '':
            text = str(get_attr_safe(text_obj, 'value', ''))

        # Get position
        pos = get_attr_safe(text_obj, 'position', None)
        if pos is None:
            pos = get_attr_safe(text_obj, 'at', None)

        position_x: float = 0.0
        position_y: float = 0.0
        angle: float = 0.0

        if pos is not None:
            if hasattr(pos, 'x'):
                position_x = float(pos.x)
                position_y = float(pos.y)
                angle = float(getattr(pos, 'angle', 0.0) or 0.0)
            elif isinstance(pos, (tuple, list)):
                position_x = float(pos[0])
                position_y = float(pos[1])
                if len(pos) > 2:
                    angle = float(pos[2])
        else:
            position_x = float(get_attr_safe(text_obj, 'x', 0.0))
            if position_x == 0.0:
                position_x = float(get_attr_safe(text_obj, 'position_x', 0.0))
            position_y = float(get_attr_safe(text_obj, 'y', 0.0))
            if position_y == 0.0:
                position_y = float(get_attr_safe(text_obj, 'position_y', 0.0))
            angle = float(get_attr_safe(text_obj, 'angle', 0.0))
            if angle == 0.0:
                angle = float(get_attr_safe(text_obj, 'rotation', 0.0))

        # Get font and effects
        effects = get_attr_safe(text_obj, 'effects', None)
        font = get_attr_safe(effects, 'font', None) if effects else None

        font_name: str = "Arial"
        size_x: float = 1.0
        size_y: float = 1.0
        stroke_width: float = 0.15
        line_spacing: float = 1.0
        bold: bool = False
        italic: bool = False

        if font is not None:
            font_name = str(get_attr_safe(font, 'face', 'Arial'))
            if font_name == 'Arial':
                font_name = str(get_attr_safe(font, 'name', 'Arial'))

            size = get_attr_safe(font, 'size', None)
            if size is not None:
                if hasattr(size, 'x'):
                    size_x = float(size.x)
                    size_y = float(size.y)
                elif isinstance(size, (tuple, list)):
                    size_x = float(size[0])
                    size_y = float(size[1])
                else:
                    size_x = float(size)
                    size_y = float(size)

            stroke_width = float(get_attr_safe(font, 'thickness', 0.15))
            line_spacing = float(get_attr_safe(font, 'line_spacing', 1.0) or 1.0)
            bold = bool(get_attr_safe(font, 'bold', False))
            italic = bool(get_attr_safe(font, 'italic', False))

        # Get alignment
        justify = get_attr_safe(effects, 'justify', None) if effects else None
        h_align: HAlign = HAlign.CENTER
        v_align: VAlign = VAlign.CENTER
        mirrored: bool = False

        if justify is not None:
            h_str: str = str(get_attr_safe(justify, 'horizontal', 'center'))
            if h_str == 'center':
                h_str = str(get_attr_safe(justify, 'h', 'center'))
            v_str: str = str(get_attr_safe(justify, 'vertical', 'center'))
            if v_str == 'center':
                v_str = str(get_attr_safe(justify, 'v', 'center'))
            mirrored = bool(get_attr_safe(justify, 'mirror', False))

            if h_str == 'left':
                h_align = HAlign.LEFT
            elif h_str == 'right':
                h_align = HAlign.RIGHT
            if v_str == 'top':
                v_align = VAlign.TOP
            elif v_str == 'bottom':
                v_align = VAlign.BOTTOM

        # Get layer and knockout
        layer_val = get_attr_safe(text_obj, 'layer', FRONT_SILKSCREEN_LAYER)
        layer: str = str(layer_val.name) if hasattr(layer_val, 'name') else str(layer_val)
        knockout: bool = bool(get_attr_safe(text_obj, 'knockout', False))

        return TextParams(
            text=text,
            font_name=font_name,
            size_x=size_x,
            size_y=size_y,
            position_x=position_x,
            position_y=position_y,
            angle=angle,
            bold=bold,
            italic=italic,
            mirrored=mirrored,
            h_align=h_align,
            v_align=v_align,
            stroke_width=stroke_width,
            line_spacing=line_spacing,
            knockout=knockout,
            layer=layer,
        )

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            'text': self.text,
            'font_name': self.font_name,
            'size_x': self.size_x,
            'size_y': self.size_y,
            'position_x': self.position_x,
            'position_y': self.position_y,
            'angle': self.angle,
            'bold': self.bold,
            'italic': self.italic,
            'mirrored': self.mirrored,
            'h_align': int(self.h_align),
            'v_align': int(self.v_align),
            'stroke_width': self.stroke_width,
            'line_spacing': self.line_spacing,
            'knockout': self.knockout,
            'layer': self.layer,
            'render_cache': self.render_cache,
        }


# =============================================================================
# Protocols
# =============================================================================

@runtime_checkable
class Bounded(Protocol):
    """Protocol for elements with spatial bounds.

    All spatial elements MUST implement get_bounds().

    Used by:
    - SVG rendering (viewport calculation)
    - Filters (spatial queries)
    - Collision detection
    - Any spatial algorithm

    Maps to: concept Bounded = requires(T t) { { t.get_bounds() } -> BoundingBox; };
    """

    def get_bounds(self) -> BoundingBox:
        """Get axis-aligned bounding box of this element.

        Returns:
            BoundingBox with min/max coordinates in mm.
        """
        ...


@runtime_checkable
class SvgRenderable(Protocol):
    """Protocol for elements that can render to SVG.

    All renderable elements MUST implement to_svg().

    Each element renders itself, returning SVG element strings.
    Parent containers compose child elements.

    Maps to: concept SvgRenderable = requires(T t, SvgRenderContext ctx) {
        { t.to_svg(ctx) } -> std::vector<std::string>;
    };
    """

    def to_svg(self, ctx: Optional['SvgRenderContext'] = None) -> List[str]:
        """Render this element to SVG.

        Args:
            ctx: Render context with transform and style options.
                 If None, use default context.

        Returns:
            List of SVG element strings (e.g., '<path d="..."/>', '<circle .../>').
            NOT a complete SVG document - just the element(s).
        """
        ...


# =============================================================================
# SVG Render Context
# =============================================================================

@dataclass(slots=True)
class SvgRenderContext:
    """Context for SVG rendering operations.

    SVG rendering MUST use SvgRenderContext.

    Carries transform, style, and filter options through the render tree.
    Enables nested transforms (e.g., footprint elements within a PCB).

    Maps to:
        struct SvgRenderContext {
            double offset_x, offset_y;
            double rotation;
            std::optional<std::vector<std::string>> layers;
            std::string fill, stroke;
            bool black_and_white;
            double arc_error_mm;
            int precision;
        };
    """
    # Transform
    offset_x: float = 0.0
    offset_y: float = 0.0
    rotation: float = 0.0  # Degrees, clockwise

    # Layer filter (None = all layers)
    layers: Optional[List[str]] = None

    # Style
    fill: str = "#000000"
    stroke: str = "#000000"
    black_and_white: bool = True

    # Rendering options
    arc_error_mm: float = 0.005  # ARC_HIGH_DEF for arc segment approximation
    precision: int = 4  # Decimal places for SVG coordinates

    def with_offset(self, dx: float, dy: float) -> 'SvgRenderContext':
        """Return new context with additional offset."""
        return SvgRenderContext(
            offset_x=self.offset_x + dx,
            offset_y=self.offset_y + dy,
            rotation=self.rotation,
            layers=self.layers,
            fill=self.fill,
            stroke=self.stroke,
            black_and_white=self.black_and_white,
            arc_error_mm=self.arc_error_mm,
            precision=self.precision,
        )

    def with_rotation(self, angle_deg: float) -> 'SvgRenderContext':
        """Return new context with additional rotation."""
        return SvgRenderContext(
            offset_x=self.offset_x,
            offset_y=self.offset_y,
            rotation=self.rotation + angle_deg,
            layers=self.layers,
            fill=self.fill,
            stroke=self.stroke,
            black_and_white=self.black_and_white,
            arc_error_mm=self.arc_error_mm,
            precision=self.precision,
        )

    def with_transform(self, dx: float, dy: float, angle_deg: float) -> 'SvgRenderContext':
        """Return new context with additional offset and rotation."""
        return SvgRenderContext(
            offset_x=self.offset_x + dx,
            offset_y=self.offset_y + dy,
            rotation=self.rotation + angle_deg,
            layers=self.layers,
            fill=self.fill,
            stroke=self.stroke,
            black_and_white=self.black_and_white,
            arc_error_mm=self.arc_error_mm,
            precision=self.precision,
        )

    def layer_visible(self, layer: str) -> bool:
        """Check if a layer should be rendered."""
        if self.layers is None:
            return True
        return layer in self.layers

    def fmt(self, value: float) -> str:
        """Format a coordinate value for SVG output."""
        return f"{value:.{self.precision}f}"


# =============================================================================
# Geometry Utilities
# =============================================================================

def rotate_point(
    x: float,
    y: float,
    angle_deg: float,
    cx: float = 0.0,
    cy: float = 0.0
) -> Tuple[float, float]:
    """Rotate a point around a center.

    Args:
        x, y: Point to rotate
        angle_deg: Rotation angle in degrees (positive = counter-clockwise in math coords)
        cx, cy: Center of rotation (default: origin)

    Returns:
        Rotated (x, y) coordinates

    Note:
        KiCad uses clockwise rotation convention (Y-axis points down in SVG).
        Caller should negate angle if needed for KiCad compatibility.
    """
    if angle_deg == 0.0:
        return (x, y)

    rad: float = math.radians(angle_deg)
    cos_a: float = math.cos(rad)
    sin_a: float = math.sin(rad)
    dx: float = x - cx
    dy: float = y - cy

    return (
        cx + dx * cos_a - dy * sin_a,
        cy + dx * sin_a + dy * cos_a
    )


def get_arc_to_segment_count(
    radius: float,
    error_max: float,
    arc_angle_deg: float = 360.0
) -> int:
    """Calculate number of segments to approximate an arc.

    Matches KiCad's GetArcToSegmentCount from geometry_utils.cpp.

    Args:
        radius: Arc radius in mm
        error_max: Maximum error (distance from segment to arc) in mm
        arc_angle_deg: Arc angle in degrees (default 360 for full circle)

    Returns:
        Number of segments (minimum 2)

    Reference:
        KiCad source: common/geometry/geometry_utils.cpp
        Formula derived from chord-to-arc deviation geometry.
    """
    MIN_SEGCOUNT_FOR_CIRCLE: int = 8

    # Avoid divide-by-zero
    radius = max(0.001, radius)  # 1 micrometer minimum
    error_max = max(0.001, error_max)

    # Error relative to radius
    rel_error: float = error_max / radius

    # Clamp to valid acos range
    rel_error = min(rel_error, 1.0)

    # Minimal arc increment in degrees
    arc_increment: float = 180.0 / math.pi * math.acos(1.0 - rel_error) * 2.0

    # Ensure minimal arc increment for a full circle
    arc_increment = min(360.0 / MIN_SEGCOUNT_FOR_CIRCLE, arc_increment)

    seg_count: int = round(abs(arc_angle_deg) / arc_increment)

    # Minimum 2 segments
    return max(seg_count, 2)

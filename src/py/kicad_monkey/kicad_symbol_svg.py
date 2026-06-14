"""
KiCad Symbol SVG Renderer

Renders symbol library files (.kicad_sym) to SVG format.
Supports color themes with sensible defaults matching KiCad's behavior.

Usage:
    from kicad.kicad_symbol_svg import render_symbol_svg, SymbolTheme, SymbolSvgContext

    # Render with defaults
    svg = render_symbol_svg(lib_symbol)

    # Render with custom theme
    theme = SymbolTheme(black_and_white=True)
    svg = render_symbol_svg(lib_symbol, theme=theme)

    # Render specific unit
    svg = render_symbol_svg(lib_symbol, unit=2, theme=theme)

    # Use context directly for element-level rendering
    ctx = SymbolSvgContext(theme=theme)
    element_svg = rectangle.to_svg(ctx)

Reference:
    KiCad CLI: kicad-cli sym export svg --help
    KiCad source:
        - eeschema/sch_painter.cpp (SCH_PAINTER::draw)
        - eeschema/symbol_editor/symbol_editor_settings.cpp (default colors)
        - common/plotters/SVG_plotter.cpp (SVG output)
        - include/render_settings.h (RENDER_SETTINGS base class)
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, List, Optional, Tuple

from .kicad_geometry import BoundingBox
from .kicad_stroke_font import get_renderer as get_stroke_font_renderer
from .kicad_sym_rectangle import SymFillType

if TYPE_CHECKING:
    from .kicad_lib_symbol import LibSymbol
    from .kicad_lib_subsymbol import LibSubSymbol
    from .kicad_symbol_lib import KiCadSymbolLib


# =============================================================================
# Theme - Color and Style Definitions
# =============================================================================

@dataclass
class SymbolTheme:
    """Color theme for symbol rendering.

    KiCad uses color schemes that define colors for different element types.
    When elements have stroke_width=0 or no color, they inherit from the theme.

    Default values match KiCad CLI SVG output exactly.
    Colors extracted from kicad-cli sym export svg output.
    """
    # Body graphics (rectangles, circles, arcs, polylines)
    # KiCad CLI uses #840000 for body strokes
    body_outline: str = "#840000"
    body_fill: str = "#FFFFC2"     # Device background fill - matches kicad-cli
    body_stroke_width: float = 0.1524  # 6 mils - KiCad default

    # Pins - KiCad CLI uses #840000 for pin lines (same as body)
    pin_color: str = "#840000"
    pin_stroke_width: float = 0.1524  # 6 mils

    # Text colors - KiCad CLI uses #006464 for reference/value/pin names
    # and #A90000 for pin numbers
    text_color: str = "#006464"
    pin_name_color: str = "#006464"
    pin_number_color: str = "#A90000"
    reference_color: str = "#006464"
    value_color: str = "#006464"
    field_color: str = "#006464"

    # Background
    background_color: str = "#FFFFFF"

    # Options
    black_and_white: bool = False
    include_hidden_pins: bool = False
    include_hidden_fields: bool = False

    # Pin text visibility (can override symbol settings)
    show_pin_names: Optional[bool] = None   # None = use symbol setting
    show_pin_numbers: Optional[bool] = None  # None = use symbol setting

    def get_body_outline(self) -> str:
        """Get body outline color, respecting B&W mode."""
        return "#000000" if self.black_and_white else self.body_outline

    def get_body_fill(self) -> str:
        """Get body fill color, respecting B&W mode."""
        return "none" if self.black_and_white else self.body_fill

    def get_pin_color(self) -> str:
        """Get pin color, respecting B&W mode."""
        return "#000000" if self.black_and_white else self.pin_color

    def get_text_color(self) -> str:
        """Get text color, respecting B&W mode."""
        return "#000000" if self.black_and_white else self.text_color

    def get_pin_name_color(self) -> str:
        """Get pin name color, respecting B&W mode."""
        return "#000000" if self.black_and_white else self.pin_name_color

    def get_pin_number_color(self) -> str:
        """Get pin number color, respecting B&W mode."""
        return "#000000" if self.black_and_white else self.pin_number_color


# =============================================================================
# SVG Render Context - Passed to all to_svg() methods
# =============================================================================

@dataclass
class SymbolSvgContext:
    """
    Context for symbol SVG rendering.

    This context is passed to all element to_svg() methods, carrying:
    - Theme (colors, default stroke widths)
    - Transform state (offset, rotation, mirror)
    - Visibility options (hidden pins, hidden fields)
    - Output precision

    Elements use this context to determine their appearance without
    needing to know about the overall rendering setup.

    This extends the basic SvgRenderContext pattern with symbol-specific
    settings like theme colors and pin visibility.
    """
    # Theme for colors and defaults
    theme: SymbolTheme = field(default_factory=SymbolTheme)

    # Transform (for composed rendering like schematic symbols)
    offset_x: float = 0.0
    offset_y: float = 0.0
    rotation: float = 0.0  # Degrees
    mirror_x: bool = False
    mirror_y: bool = False

    # Coordinate translation (to make viewBox start at 0,0 like KiCad CLI)
    translate_x: float = 0.0
    translate_y: float = 0.0

    # Visibility
    show_pin_names: bool = True
    show_pin_numbers: bool = True
    show_hidden_pins: bool = False
    show_hidden_fields: bool = False

    # Output precision
    precision: int = 4

    def fmt(self, value: float) -> str:
        """Format a coordinate value for SVG output."""
        return f"{value:.{self.precision}f}"

    def tx(self, x: float) -> float:
        """Translate X coordinate to viewBox-relative coordinates."""
        return x + self.translate_x

    def ty(self, y: float) -> float:
        """Translate Y coordinate to viewBox-relative coordinates.

        KiCad symbols use Y-up coordinates, SVG uses Y-down.
        Transform: SVG_y = -KiCad_y + translate_y
        """
        return -y + self.translate_y

    def transform_point(self, x: float, y: float) -> Tuple[float, float]:
        """Apply current transform to a point."""
        # Mirror
        if self.mirror_x:
            x = -x
        if self.mirror_y:
            y = -y

        # Rotate
        if self.rotation != 0:
            rad = math.radians(self.rotation)
            cos_a = math.cos(rad)
            sin_a = math.sin(rad)
            x, y = x * cos_a - y * sin_a, x * sin_a + y * cos_a

        # Translate
        x += self.offset_x
        y += self.offset_y

        return x, y

    def with_offset(self, dx: float, dy: float) -> 'SymbolSvgContext':
        """Return new context with additional offset."""
        return SymbolSvgContext(
            theme=self.theme,
            offset_x=self.offset_x + dx,
            offset_y=self.offset_y + dy,
            rotation=self.rotation,
            mirror_x=self.mirror_x,
            mirror_y=self.mirror_y,
            show_pin_names=self.show_pin_names,
            show_pin_numbers=self.show_pin_numbers,
            show_hidden_pins=self.show_hidden_pins,
            show_hidden_fields=self.show_hidden_fields,
            precision=self.precision,
        )

    def with_transform(
        self,
        dx: float = 0.0,
        dy: float = 0.0,
        rotation: float = 0.0,
        mirror_x: bool = False,
        mirror_y: bool = False,
    ) -> 'SymbolSvgContext':
        """Return new context with additional transform."""
        # Compose transforms
        new_mirror_x = self.mirror_x != mirror_x  # XOR
        new_mirror_y = self.mirror_y != mirror_y
        new_rotation = self.rotation + rotation

        return SymbolSvgContext(
            theme=self.theme,
            offset_x=self.offset_x + dx,
            offset_y=self.offset_y + dy,
            rotation=new_rotation,
            mirror_x=new_mirror_x,
            mirror_y=new_mirror_y,
            show_pin_names=self.show_pin_names,
            show_pin_numbers=self.show_pin_numbers,
            show_hidden_pins=self.show_hidden_pins,
            show_hidden_fields=self.show_hidden_fields,
            precision=self.precision,
        )

    # Convenience accessors for theme values
    @property
    def body_stroke(self) -> str:
        """Get body outline stroke color."""
        return self.theme.get_body_outline()

    @property
    def body_fill(self) -> str:
        """Get body fill color."""
        return self.theme.get_body_fill()

    @property
    def body_stroke_width(self) -> float:
        """Get default body stroke width."""
        return self.theme.body_stroke_width

    @property
    def pin_stroke(self) -> str:
        """Get pin stroke color."""
        return self.theme.get_pin_color()

    @property
    def pin_stroke_width(self) -> float:
        """Get pin stroke width."""
        return self.theme.pin_stroke_width

    @property
    def text_color(self) -> str:
        """Get text color."""
        return self.theme.get_text_color()


# =============================================================================
# Render Options
# =============================================================================

@dataclass
class SymbolRenderOptions:
    """Options controlling what gets rendered."""
    unit: int = 1           # Which unit to render (1-based)
    style: int = 0          # 0 = normal, 1 = De Morgan alternate
    # Symbol previews default to body + pins + pin labels only.  Set this
    # when a caller wants KiCad CLI-style Reference/Value field output.
    include_properties: bool = False
    margin_ratio: float = 0.2  # KiCad uses 1.2x = 20% margin (10% each side)


_PIN_TEXT_MARGIN_MM = 0.1016  # 4 mils; KiCad's PIN_TEXT_MARGIN.


def render_symbol_svg(
    symbol: 'LibSymbol',
    theme: Optional[SymbolTheme] = None,
    options: Optional[SymbolRenderOptions] = None,
) -> str:
    """
    Render a symbol to SVG format.

    Output matches KiCad CLI's `kicad-cli sym export svg` format:
    - ViewBox starts at 0,0 (all coordinates translated)
    - Inline styles (no CSS classes)
    - Text rendered as stroke font paths
    - Colors: #840000 for body, #006464 for text

    Args:
        symbol: LibSymbol to render
        theme: Color theme (defaults to KiCad CLI colors)
        options: Render options (unit, style, margins)

    Returns:
        Complete SVG document string
    """
    if theme is None:
        theme = SymbolTheme()
    if options is None:
        options = SymbolRenderOptions()

    # Determine pin visibility from symbol or theme override
    show_pin_names = theme.show_pin_names
    if show_pin_names is None:
        show_pin_names = not symbol.pin_names_hide

    show_pin_numbers = theme.show_pin_numbers
    if show_pin_numbers is None:
        show_pin_numbers = not symbol.pin_numbers_hide

    # Find subsymbols for the requested unit/style
    subsymbols = _get_subsymbols_for_unit(symbol, options.unit, options.style)

    if not subsymbols:
        return _empty_svg(symbol.name)

    # Calculate bounding box
    bbox = _compute_symbol_bounds(symbol, subsymbols, options, show_pin_names, show_pin_numbers)

    if not bbox.is_valid():
        return _empty_svg(symbol.name)

    # Add margin - KiCad uses 1.2x multiplier (20% larger = 10% margin each side)
    # This matches: pageInfo.SetHeightMils(symbolBB.GetHeight() * 1.2)
    margin_x = bbox.width * options.margin_ratio / 2  # Half on each side
    margin_y = bbox.height * options.margin_ratio / 2
    bbox = bbox.expand_by_xy(margin_x, margin_y)

    # Create render context with theme and visibility settings
    # KiCad centers on bbox.center, then coordinates map to viewBox center
    # Transform: svg_x = (kicad_x - bbox.center_x) + viewBox_width/2
    #           svg_y = viewBox_height/2 - (kicad_y - bbox.center_y)
    # Which simplifies to:
    #           svg_x = kicad_x + (-bbox.center_x + viewBox_width/2)
    #           svg_y = -kicad_y + (bbox.center_y + viewBox_height/2)
    bbox_center_x = (bbox.min_x + bbox.max_x) / 2
    bbox_center_y = (bbox.min_y + bbox.max_y) / 2
    vb_width = bbox.width
    vb_height = bbox.height

    ctx = SymbolSvgContext(
        theme=theme,
        translate_x=-bbox_center_x + vb_width / 2,
        translate_y=bbox_center_y + vb_height / 2,  # For Y flip: SVG_y = -kicad_y + translate_y
        show_pin_names=show_pin_names,
        show_pin_numbers=show_pin_numbers,
        show_hidden_pins=theme.include_hidden_pins,
        show_hidden_fields=theme.include_hidden_fields,
    )

    # Build SVG - match KiCad CLI format exactly
    svg_parts = []

    # Header (KiCad CLI format)
    vb_width = bbox.width
    vb_height = bbox.height
    svg_parts.append('<?xml version="1.0" standalone="no"?>')
    svg_parts.append(' <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" ')
    svg_parts.append(' "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"> ')
    svg_parts.append('<svg')
    svg_parts.append('  xmlns:svg="http://www.w3.org/2000/svg"')
    svg_parts.append('  xmlns="http://www.w3.org/2000/svg"')
    svg_parts.append('  xmlns:xlink="http://www.w3.org/1999/xlink"')
    svg_parts.append('  version="1.1"')
    svg_parts.append(f'  width="{vb_width:.4f}mm" height="{vb_height:.4f}mm" viewBox="0.0000 0.0000 {vb_width:.4f} {vb_height:.4f}">')
    svg_parts.append(f'<title>SVG Image created as {_escape_xml(symbol.name)}_unit{options.unit}.svg </title>')
    svg_parts.append('  <desc>Image generated by KiCad Python </desc>')

    # Body graphics group - inline style
    body_style = ('fill:#000000; fill-opacity:1.0000;stroke:#000000; stroke-opacity:1.0000;\n'
                  'stroke-linecap:round; stroke-linejoin:round;')
    svg_parts.append(f'<g style="{body_style}"')
    svg_parts.append(' transform="translate(0 0) scale(1 1)">')

    # Render subsymbols (graphics only, not pins)
    for subsym in subsymbols:
        svg_parts.extend(_render_subsymbol_graphics(subsym, ctx))

    svg_parts.append('</g>')

    # Pin lines group
    pin_style = (f'fill:none; \n'
                 f'stroke:{theme.get_body_outline()}; stroke-width:{theme.pin_stroke_width:.4f}; stroke-opacity:1; \n'
                 f'stroke-linecap:round; stroke-linejoin:round;')
    svg_parts.append(f'<g style="{pin_style}">')
    for subsym in subsymbols:
        svg_parts.extend(_render_subsymbol_pins(subsym, ctx))
    svg_parts.append('</g>')

    # Text group (properties, pin names/numbers) using stroke font
    text_style = (f'fill:none; \n'
                  f'stroke:{theme.get_text_color()}; stroke-width:{theme.pin_stroke_width:.4f}; stroke-opacity:1; \n'
                  f'stroke-linecap:round; stroke-linejoin:round;')
    svg_parts.append(f'<g style="{text_style}">')

    # Render properties (reference, value, etc.) if requested
    if options.include_properties:
        svg_parts.extend(_render_properties_stroke(symbol, ctx))

    for subsym in subsymbols:
        svg_parts.extend(_render_subsymbol_pin_texts(subsym, symbol, ctx))

    svg_parts.append('</g> ')
    svg_parts.append('</svg>')

    return '\n'.join(svg_parts)


def render_library_svg(
    library: 'KiCadSymbolLib',
    output_dir: Path,
    theme: Optional[SymbolTheme] = None,
    options: Optional[SymbolRenderOptions] = None,
) -> int:
    """
    Render all symbols in a library to individual SVG files.

    Args:
        library: KiCadSymbolLib to render
        output_dir: Directory to write SVG files
        theme: Color theme
        options: Render options

    Returns:
        Number of symbols rendered
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    count = 0
    for symbol in library.symbols:
        svg_content = render_symbol_svg(symbol, theme, options)
        output_file = output_dir / f"{_sanitize_filename(symbol.name)}.svg"
        output_file.write_text(svg_content, encoding='utf-8')
        count += 1

    return count


def _get_subsymbols_for_unit(
    symbol: 'LibSymbol',
    unit: int,
    style: int
) -> List['LibSubSymbol']:
    """Get subsymbols that apply to the specified unit/style.

    KiCad convention:
    - Unit 0 means "all units" (common graphics)
    - Style 0 = normal, Style 1 = De Morgan
    - Subsymbol name format: "SymbolName_unit_style"

    Note: Some symbols only have style=1 subsymbols (e.g., MCUs).
    If no subsymbols match the requested style, we try to find any
    subsymbols for the unit regardless of style.
    """
    # First, try to find exact matches for unit and style
    result = []
    for subsym in symbol.subsymbols:
        # Unit 0 applies to all units
        if subsym.unit == 0 or subsym.unit == unit:
            # Style must match (0 for common, or specific style)
            if subsym.style == 0 or subsym.style == style:
                result.append(subsym)

    # If no matches, try any subsymbol for this unit (any style)
    if not result:
        for subsym in symbol.subsymbols:
            if subsym.unit == 0 or subsym.unit == unit:
                result.append(subsym)

    return result


def _compute_symbol_bounds(
    symbol: 'LibSymbol',
    subsymbols: List['LibSubSymbol'],
    options: SymbolRenderOptions,
    show_pin_names: bool,
    show_pin_numbers: bool,
) -> BoundingBox:
    """Compute overall bounding box for symbol rendering.

    KiCad's GetUnitBoundingBox includes:
    - Body graphics (polylines, rectangles, arcs, circles, text)
    - Pin wire lines
    - Pin name text boxes (if visible)
    - Pin number text boxes (if visible)

    KiCad does NOT include Reference/Value properties in the bounding box.
    These are rendered relative to symbol origin, often outside the viewBox.
    """
    bbox = BoundingBox()

    # Get pin_names_offset from symbol (affects text placement)
    pin_name_offset = symbol.pin_names_offset  # Default 0.508mm = 20 mils

    # Include all graphic elements from subsymbols
    for subsym in subsymbols:
        subsym_bbox = subsym.get_bounds()
        bbox.merge(subsym_bbox)

        # Include pin bounds with proper text box calculation
        for pin in subsym.pins:
            # Pin wire bounds
            pin_bbox = pin.get_bounds()
            bbox.merge(pin_bbox)

            # Pin name text box (if visible)
            if show_pin_names and pin.name and pin.name != "~":
                name_bbox = _get_pin_name_bbox(pin, pin_name_offset)
                if name_bbox:
                    bbox.merge(name_bbox)

            # Pin number text box (always calculated if visible)
            if show_pin_numbers and pin.number:
                num_bbox = _get_pin_number_bbox(pin, pin_name_offset,
                                                show_pin_names and bool(pin.name and pin.name != "~"))
                if num_bbox:
                    bbox.merge(num_bbox)

    # NOTE: KiCad does NOT include properties (Reference/Value) in bbox
    # They render relative to symbol origin, often outside the viewBox

    return bbox


def _get_pin_name_bbox(pin, pin_name_offset: float) -> Optional[BoundingBox]:
    """Calculate bounding box for pin name text.

    Based on KiCad's PIN_LAYOUT_CACHE::getUntransformedPinNameBox():
    - If pin_name_offset > 0: Name is INSIDE the symbol (to the right of body end)
    - If pin_name_offset == 0: Name is ABOVE the pin wire (centered along length)
    """
    if not pin.name or pin.name == "~":
        return None

    # Get text size from pin effects, or use default
    name_size = 1.27  # Default KiCad pin name size
    if pin.name_effects and pin.name_effects.font:
        name_size = pin.name_effects.font.size_y

    # Estimate text extents (width = char_count * size * factor)
    text_width = len(pin.name) * name_size * 0.7  # Approximate character width
    text_height = name_size * 1.1  # Approximate height with some margin

    pin_length = pin.length
    rad = math.radians(pin.at_angle)

    # Calculate box center in untransformed (PIN_RIGHT) coordinates
    if pin_name_offset > 0:
        # Name inside the pin body
        center_x = pin_length + text_width / 2 + pin_name_offset
        center_y = 0
    else:
        # Name above the pin (centered along length)
        text_offset = 0.09144  # 24 * 0.15 mils = 3.6 mils = 0.09144mm
        center_x = pin_length / 2
        center_y = text_height / 2 + text_offset  # Above in Y-up coords

    # Create box in untransformed coordinates
    half_w = text_width / 2
    half_h = text_height / 2
    box_min_x = center_x - half_w
    box_min_y = center_y - half_h
    box_max_x = center_x + half_w
    box_max_y = center_y + half_h

    # Transform box corners based on pin orientation
    # Corners in untransformed space
    corners = [
        (box_min_x, box_min_y),
        (box_max_x, box_min_y),
        (box_max_x, box_max_y),
        (box_min_x, box_max_y),
    ]

    # Transform corners to symbol coordinates
    cos_a = math.cos(rad)
    sin_a = math.sin(rad)
    transformed = []
    for cx, cy in corners:
        # Rotate around origin, then translate to pin position
        tx = cx * cos_a - cy * sin_a + pin.at_x
        ty = cx * sin_a + cy * cos_a + pin.at_y
        transformed.append((tx, ty))

    # Create bbox from transformed corners
    result = BoundingBox()
    for tx, ty in transformed:
        result.expand((tx, ty))

    return result


def _get_pin_number_bbox(pin, pin_name_offset: float, show_name: bool) -> Optional[BoundingBox]:
    """Calculate bounding box for pin number text.

    Based on KiCad's PIN_LAYOUT_CACHE::getUntransformedPinNumberBox():
    - Number is centered along pin length
    - For horizontal pins (0°, 180°): number is ABOVE the pin wire
    - For vertical pins (90°, 270°): number is to the LEFT of the pin wire
    - If both name and number shown with name outside: positions may swap
    """
    if not pin.number:
        return None

    # Get text size from pin effects, or use default
    num_size = 1.27  # Default KiCad pin number size
    if pin.number_effects and pin.number_effects.font:
        num_size = pin.number_effects.font.size_y

    # Estimate text extents using KiCad stroke font metrics
    # KiCad stroke font: character width ≈ font_size (including spacing)
    # Single digits render at approximately font_size width
    char_width = num_size * 1.0
    text_width = len(pin.number) * char_width
    text_height = num_size

    pin_length = pin.length
    angle = pin.at_angle

    # Normalize angle to 0, 90, 180, 270
    norm_angle = int(angle) % 360
    is_vertical = norm_angle in (90, 270)

    # Text offset from pin wire to text edge
    # KiCad stroke font has additional spacing - empirically derived to match KiCad CLI output
    # The visible offset is approximately 0.42mm (text_edge to wire)
    text_offset = 0.42

    # Calculate pin wire center point
    # Pin origin is at connection point, wire extends toward symbol body
    # In KiCad: angle 0=right, 90=up, 180=left, 270=down
    rad = math.radians(angle)
    wire_center_x = pin.at_x + (pin_length / 2) * math.cos(rad)
    wire_center_y = pin.at_y + (pin_length / 2) * math.sin(rad)

    if is_vertical:
        # For vertical pins: number to the LEFT of pin wire
        # The offset is perpendicular to the pin direction
        number_center_x = wire_center_x - (text_width / 2 + text_offset)
        number_center_y = wire_center_y
        half_w = text_width / 2
        half_h = text_height / 2
    else:
        # For horizontal pins: number ABOVE the pin wire
        number_center_x = wire_center_x
        number_center_y = wire_center_y + (text_height / 2 + text_offset)
        half_w = text_width / 2
        half_h = text_height / 2

    # Create bounding box
    result = BoundingBox()
    result.expand((number_center_x - half_w, number_center_y - half_h))
    result.expand((number_center_x + half_w, number_center_y + half_h))

    return result


def _expand_for_pin_text(bbox: BoundingBox, pin, text_len: float, is_name: bool):
    """Expand bounding box to account for pin text. (Legacy function, kept for reference)"""
    rad = math.radians(pin.at_angle)
    end_x, end_y = pin.end_point

    if is_name:
        # Name is beyond pin end
        offset = text_len + 0.5
        text_x = end_x + offset * math.cos(rad)
        text_y = end_y - offset * math.sin(rad)
    else:
        # Number is near pin origin
        offset = 0.5
        text_x = pin.at_x + offset * math.sin(rad)
        text_y = pin.at_y + offset * math.cos(rad)

    bbox.expand((text_x - text_len/2, text_y - 0.5))
    bbox.expand((text_x + text_len/2, text_y + 0.5))


# =============================================================================
# KiCad CLI-Style Rendering Functions
# =============================================================================
# These functions produce output that matches kicad-cli sym export svg exactly:
# - Inline styles (no CSS classes)
# - Path elements with M/L commands
# - Translated coordinates (viewBox at 0,0)
# - Stroke font text


def _subsymbol_stroke_style(elem, ctx: SymbolSvgContext) -> str:
    """Per-element stroke style: honor (stroke (width ...)) when set."""
    theme = ctx.theme
    width = getattr(getattr(elem, 'stroke', None), 'width', 0.0) or 0.0
    if width <= 0:
        width = theme.body_stroke_width
    return (f'fill:none; \n'
            f'stroke:{theme.get_body_outline()}; stroke-width:{width:.4f}; stroke-opacity:1; \n'
            f'stroke-linecap:round; stroke-linejoin:round;fill:none')


def _subsymbol_fill_color(elem, ctx: SymbolSvgContext) -> Optional[str]:
    """SVG fill color for the element, or None when not filled."""
    fill = getattr(elem, 'fill', None)
    if fill is None:
        return None
    stroke_color = ctx.theme.get_body_outline()
    if fill.type == SymFillType.BACKGROUND:
        color = ctx.theme.get_body_fill()
        return None if color == 'none' else color
    if fill.type == SymFillType.OUTLINE:
        return stroke_color
    if fill.type == SymFillType.COLOR and fill.color:
        r, g, b = fill.color[0], fill.color[1], fill.color[2]
        return f'#{r:02X}{g:02X}{b:02X}'
    return None


def _append_subsymbol_fill(lines: list[str], shape_svg: str, color: str) -> None:
    # kicad-cli emits background fills as a stroke-free group drawn beneath the outline pass.
    lines.append(f'<g style="fill:{color}; fill-opacity:1.0000; stroke:none;">')
    lines.append(shape_svg)
    lines.append('</g>')


def _append_subsymbol_fill_path(
    lines: list[str],
    points: list[tuple[float, float]],
    color: str,
    ctx: SymbolSvgContext,
) -> None:
    # kicad-cli emits polygonal fills as one self-styled closed path.
    pts = list(points)
    if len(pts) >= 2 and pts[0] == pts[-1]:
        pts = pts[:-1]
    if len(pts) < 3:
        return
    lines.append(
        f'<path style="fill:{color}; fill-opacity:1.0000; '
        f'stroke:none;fill-rule:evenodd;"'
    )
    lines.append(f'd="M {ctx.fmt(ctx.tx(pts[0][0]))},{ctx.fmt(ctx.ty(pts[0][1]))}')
    for x, y in pts[1:]:
        lines.append(f'{ctx.fmt(ctx.tx(x))},{ctx.fmt(ctx.ty(y))}')
    lines.append('Z" /> ')


def _append_subsymbol_fill_graphics(
    lines: list[str],
    subsym: 'LibSubSymbol',
    ctx: SymbolSvgContext,
) -> None:
    for rect in subsym.rectangles:
        color = _subsymbol_fill_color(rect, ctx)
        if color:
            x1, y1 = ctx.tx(rect.start_x), ctx.ty(rect.start_y)
            x2, y2 = ctx.tx(rect.end_x), ctx.ty(rect.end_y)
            _append_subsymbol_fill(
                lines,
                f'<rect x="{ctx.fmt(min(x1, x2))}" y="{ctx.fmt(min(y1, y2))}" '
                f'width="{ctx.fmt(abs(x2 - x1))}" height="{ctx.fmt(abs(y2 - y1))}" '
                f'rx="0.0000" />',
                color,
            )
    for circle in subsym.circles:
        color = _subsymbol_fill_color(circle, ctx)
        if color:
            _append_subsymbol_fill(
                lines,
                f'<circle cx="{ctx.fmt(ctx.tx(circle.center_x))}" '
                f'cy="{ctx.fmt(ctx.ty(circle.center_y))}" '
                f'r="{ctx.fmt(circle.radius)}" />',
                color,
            )
    for poly in subsym.polylines:
        color = _subsymbol_fill_color(poly, ctx)
        if color and len(poly.points) >= 3:
            _append_subsymbol_fill_path(lines, list(poly.points), color, ctx)
    for arc in subsym.arcs:
        color = _subsymbol_fill_color(arc, ctx)
        if color:
            arc_points = _arc_to_points(
                arc.start_x,
                arc.start_y,
                arc.mid_x,
                arc.mid_y,
                arc.end_x,
                arc.end_y,
                segments=16,
            )
            _append_subsymbol_fill_path(lines, arc_points, color, ctx)


def _subsymbol_path_d(
    points: list[tuple[float, float]],
    ctx: SymbolSvgContext,
    *,
    translate: bool = True,
) -> str:
    if translate:
        path_d = f"M {ctx.fmt(ctx.tx(points[0][0]))},{ctx.fmt(ctx.ty(points[0][1]))}"
        for x, y in points[1:]:
            path_d += f"\n{ctx.fmt(ctx.tx(x))},{ctx.fmt(ctx.ty(y))}"
        return path_d
    path_d = f"M {ctx.fmt(points[0][0])},{ctx.fmt(points[0][1])}"
    for x, y in points[1:]:
        path_d += f"\n{ctx.fmt(x)},{ctx.fmt(y)}"
    return path_d


def _append_subsymbol_path(lines: list[str], *, style: str, path_d: str) -> None:
    lines.append(f'<path style="{style}"')
    lines.append(f'd="{path_d}')
    lines.append('" /> ')


def _append_subsymbol_stroke_graphics(
    lines: list[str],
    subsym: 'LibSubSymbol',
    ctx: SymbolSvgContext,
) -> None:
    for poly in subsym.polylines:
        if len(poly.points) >= 2:
            _append_subsymbol_path(
                lines,
                style=_subsymbol_stroke_style(poly, ctx),
                path_d=_subsymbol_path_d(list(poly.points), ctx),
            )
    for rect in subsym.rectangles:
        _append_subsymbol_rectangle_strokes(lines, rect, ctx)
    for circle in subsym.circles:
        _append_subsymbol_circle_stroke(lines, circle, ctx)
    for arc in subsym.arcs:
        _append_subsymbol_arc_stroke(lines, arc, ctx)


def _append_subsymbol_rectangle_strokes(lines: list[str], rect, ctx: SymbolSvgContext) -> None:
    style = _subsymbol_stroke_style(rect, ctx)
    x1, y1 = ctx.tx(rect.start_x), ctx.ty(rect.start_y)
    x2, y2 = ctx.tx(rect.end_x), ctx.ty(rect.end_y)
    for (ex1, ey1), (ex2, ey2) in (
        ((x1, y1), (x2, y1)),
        ((x2, y1), (x2, y2)),
        ((x2, y2), (x1, y2)),
        ((x1, y2), (x1, y1)),
    ):
        lines.append(f'<path style="{style}"')
        lines.append(f'd="M {ctx.fmt(ex1)},{ctx.fmt(ey1)}')
        lines.append(f'{ctx.fmt(ex2)},{ctx.fmt(ey2)}" /> ')


def _append_subsymbol_circle_stroke(lines: list[str], circle, ctx: SymbolSvgContext) -> None:
    cx = ctx.tx(circle.center_x)
    cy = ctx.ty(circle.center_y)
    points = [
        (
            cx + circle.radius * math.cos(2 * math.pi * i / 16),
            cy + circle.radius * math.sin(2 * math.pi * i / 16),
        )
        for i in range(17)
    ]
    if points:
        _append_subsymbol_path(
            lines,
            style=_subsymbol_stroke_style(circle, ctx),
            path_d=_subsymbol_path_d(points, ctx, translate=False),
        )


def _append_subsymbol_arc_stroke(lines: list[str], arc, ctx: SymbolSvgContext) -> None:
    arc_points = _arc_to_points(
        arc.start_x,
        arc.start_y,
        arc.mid_x,
        arc.mid_y,
        arc.end_x,
        arc.end_y,
        segments=16,
    )
    if arc_points:
        _append_subsymbol_path(
            lines,
            style=_subsymbol_stroke_style(arc, ctx),
            path_d=_subsymbol_path_d(
                [(ctx.tx(x), ctx.ty(y)) for x, y in arc_points],
                ctx,
                translate=False,
            ),
        )


def _render_subsymbol_graphics(
    subsym: 'LibSubSymbol',
    ctx: SymbolSvgContext,
) -> List[str]:
    """Render a subsymbol's graphics elements (polylines, rectangles, etc).

    Uses KiCad CLI format with inline styles and translated coordinates.
    """
    lines: list[str] = []
    fill_lines: list[str] = []
    _append_subsymbol_fill_graphics(fill_lines, subsym, ctx)
    lines.extend(fill_lines)
    _append_subsymbol_stroke_graphics(lines, subsym, ctx)
    return lines


def _render_subsymbol_pins(
    subsym: 'LibSubSymbol',
    ctx: SymbolSvgContext,
) -> List[str]:
    """Render pin lines as path elements.

    Uses KiCad CLI format: <path d="M x1 y1 L x2 y2" />
    """
    lines = []

    for pin in subsym.pins:
        if pin.hide and not ctx.show_hidden_pins:
            continue

        # Calculate pin endpoints
        # Pin's 'at' position is the external connection point
        # Pin angle indicates direction INTO the symbol
        # Body end = at + length * direction_vector (matching pin.get_bounds() formula)
        rad = math.radians(pin.at_angle)
        # External connection point (where nets connect)
        ext_x = ctx.tx(pin.at_x)
        ext_y = ctx.ty(pin.at_y)
        # Body connection point (inside symbol)
        # Formula matches pin.get_bounds() and pin.end_point
        # In Y-up coords: body = at + length * direction_vector
        body_x = ctx.tx(pin.at_x + pin.length * math.cos(rad))
        body_y = ctx.ty(pin.at_y + pin.length * math.sin(rad))

        # Render as path with L command
        # KiCad CLI draws from body connection to external connection
        lines.append(f'<path d="M{ctx.fmt(body_x)} {ctx.fmt(body_y)}')
        lines.append(f'L{ctx.fmt(ext_x)} {ctx.fmt(ext_y)}')
        lines.append('" />')

    return lines


def _render_properties_stroke(
    symbol: 'LibSymbol',
    ctx: SymbolSvgContext,
) -> List[str]:
    """Render properties using stroke font.

    Matches KiCad CLI format with hidden <text> element and visible stroke paths.

    KiCad CLI 'sym export svg' only renders Reference and Value properties.
    Other properties (Footprint, Datasheet, custom properties) are not shown.
    """
    lines = []
    renderer = get_stroke_font_renderer()

    # KiCad CLI only renders these property names
    RENDERED_PROPERTIES = {"Reference", "Value"}

    for prop in symbol.properties:
        # Only render Reference and Value properties (matching KiCad CLI)
        if prop.key not in RENDERED_PROPERTIES:
            continue

        # Skip hidden unless requested
        if prop.effects and prop.effects.hide and not ctx.show_hidden_fields:
            continue

        # Get font size
        font_size_y = 1.27
        font_size_x = 1.27
        if prop.effects and prop.effects.font:
            font_size_y = prop.effects.font.size_y
            font_size_x = prop.effects.font.size_x if prop.effects.font.size_x else font_size_y

        # Get alignment - KiCad defaults to center/center for text
        h_align = "center"
        v_align = "center"
        if prop.effects and prop.effects.justify:
            if "left" in prop.effects.justify:
                h_align = "left"
            elif "right" in prop.effects.justify:
                h_align = "right"
            else:
                h_align = "center"
            if "top" in prop.effects.justify:
                v_align = "top"
            elif "bottom" in prop.effects.justify:
                v_align = "bottom"

        # Translate position
        pos_x = ctx.tx(prop.at_x)
        pos_y = ctx.ty(prop.at_y)

        text = prop.value
        if not text:
            continue

        # KiCad CLI adds "?" to Reference designator for symbols (unassigned)
        if prop.key == "Reference" and "?" not in text:
            text = text + "?"

        # Hidden text element (for accessibility)
        text_len = len(text) * font_size_x * 0.6  # Approximate
        anchor = "start" if h_align == "left" else ("end" if h_align == "right" else "middle")
        lines.append(f'<text x="{ctx.fmt(pos_x)}" y="{ctx.fmt(pos_y)}"')
        lines.append(f'textLength="{ctx.fmt(text_len)}" font-size="{ctx.fmt(font_size_y * 1.27)}" lengthAdjust="spacingAndGlyphs"')
        lines.append(f'text-anchor="{anchor}" opacity="0" stroke-opacity="0">{_escape_xml(text)}</text>')

        # Stroke font paths
        lines.append(f'<g class="stroked-text"><desc>{_escape_xml(text)}</desc>')

        polylines = renderer.render_text_polylines(
            text=text,
            pos_x=pos_x,
            pos_y=pos_y,
            size_x=font_size_x,
            size_y=font_size_y,
            angle=prop.at_angle if hasattr(prop, 'at_angle') else 0.0,
            h_align=h_align,
            v_align=v_align,
        )

        for polyline in polylines:
            if len(polyline) < 2:
                continue
            # Each segment as separate path (matching KiCad CLI)
            for i in range(len(polyline) - 1):
                x1, y1 = polyline[i]
                x2, y2 = polyline[i + 1]
                lines.append(f'<path d="M{ctx.fmt(x1)} {ctx.fmt(y1)}')
                lines.append(f'L{ctx.fmt(x2)} {ctx.fmt(y2)}')
                lines.append('" />')

        lines.append('</g>')

    return lines


def _text_effect_sizes(effects) -> tuple[float, float]:
    font = getattr(effects, "font", None) if effects is not None else None
    if font is None:
        return 1.27, 1.27
    size_y = float(getattr(font, "size_y", 1.27) or 0.0)
    size_x = float(getattr(font, "size_x", size_y) or 0.0)
    return size_x, size_y


def _text_effects_visible(effects) -> bool:
    size_x, size_y = _text_effect_sizes(effects)
    return abs(size_x) > 0.0 and abs(size_y) > 0.0


def _text_effect_pen_width(effects, *, default: float) -> float:
    font = getattr(effects, "font", None) if effects is not None else None
    thickness = getattr(font, "thickness", None) if font is not None else None
    if thickness is not None and float(thickness) > 0.0:
        return float(thickness)
    return default


def _auto_pin_number_pen_width(effects) -> float:
    size_x, size_y = _text_effect_sizes(effects)
    text_size = min(abs(size_x), abs(size_y))
    if text_size <= 0.0:
        return 0.0
    return text_size / 5.0


def _append_stroked_text(
    lines: list[str],
    *,
    text: str,
    pos_x: float,
    pos_y: float,
    size_x: float,
    size_y: float,
    angle: float,
    h_align: str,
    v_align: str,
    class_name: str,
    ctx: SymbolSvgContext,
    stroke_color: Optional[str] = None,
) -> None:
    if not text:
        return

    renderer = get_stroke_font_renderer()
    anchor = "middle"
    if h_align == "left":
        anchor = "start"
    elif h_align == "right":
        anchor = "end"
    text_len = len(text) * size_x * 0.6
    escaped = _escape_xml(text)
    if stroke_color is not None:
        # Per-role color override (e.g. pin numbers use #A90000 in the
        # kicad-cli default theme while the enclosing text group is #006464).
        lines.append(
            f'<g style="fill:none; stroke:{stroke_color}; '
            f'stroke-opacity:1; stroke-linecap:round; stroke-linejoin:round;">'
        )
    lines.append(f'<text x="{ctx.fmt(pos_x)}" y="{ctx.fmt(pos_y)}"')
    lines.append(
        f'textLength="{ctx.fmt(text_len)}" font-size="{ctx.fmt(size_y)}" '
        f'lengthAdjust="spacingAndGlyphs" text-anchor="{anchor}" '
        f'opacity="0" stroke-opacity="0">{escaped}</text>'
    )
    lines.append(f'<g class="stroked-text {class_name}"><desc>{escaped}</desc>')

    for polyline in renderer.render_text_polylines(
        text=text,
        pos_x=pos_x,
        pos_y=pos_y,
        size_x=size_x,
        size_y=size_y,
        angle=angle,
        h_align=h_align,
        v_align=v_align,
    ):
        if len(polyline) < 2:
            continue
        for i in range(len(polyline) - 1):
            x1, y1 = polyline[i]
            x2, y2 = polyline[i + 1]
            lines.append(f'<path d="M{ctx.fmt(x1)} {ctx.fmt(y1)}')
            lines.append(f'L{ctx.fmt(x2)} {ctx.fmt(y2)}')
            lines.append('" />')

    lines.append('</g>')
    if stroke_color is not None:
        lines.append('</g>')


def _render_subsymbol_pin_texts(
    subsym: 'LibSubSymbol',
    symbol: 'LibSymbol',
    ctx: SymbolSvgContext,
) -> List[str]:
    """Render pin names and numbers after pin shafts so text stays visible."""
    lines: list[str] = []

    for pin in subsym.pins:
        if pin.hide and not ctx.show_hidden_pins:
            continue

        angle = int(round(float(pin.at_angle))) % 360
        rad = math.radians(float(pin.at_angle))
        root_x = pin.at_x + pin.length * math.cos(rad)
        root_y = pin.at_y + pin.length * math.sin(rad)
        midpoint_x = (root_x + pin.at_x) / 2.0
        midpoint_y = (root_y + pin.at_y) / 2.0
        horizontal = angle in {0, 180}
        pin_right = horizontal and math.cos(rad) > 0.0
        pin_down = (not horizontal) and math.sin(rad) < 0.0
        text_orient = 0.0 if horizontal else 90.0

        draws_name = bool(
            ctx.show_pin_names
            and pin.name
            and pin.name != "~"
            and _text_effects_visible(pin.name_effects)
        )

        if (
            ctx.show_pin_numbers
            and pin.number
            and _text_effects_visible(pin.number_effects)
        ):
            size_x, size_y = _text_effect_sizes(pin.number_effects)
            pen_width = (
                _text_effect_pen_width(pin.number_effects, default=0.0)
                or _auto_pin_number_pen_width(pin.number_effects)
                or ctx.pin_stroke_width
            )
            text_clearance = _PIN_TEXT_MARGIN_MM + pen_width
            h_align = "center"
            v_align = "bottom"

            if symbol.pin_names_offset > 0 or not draws_name:
                if horizontal:
                    num_x = midpoint_x
                    num_y = root_y + text_clearance
                else:
                    num_x = root_x - text_clearance
                    num_y = midpoint_y
            elif horizontal:
                num_x = midpoint_x
                num_y = root_y - text_clearance
                v_align = "top"
            else:
                num_x = root_x + text_clearance
                num_y = midpoint_y
                v_align = "top"

            _append_stroked_text(
                lines,
                text=pin.number,
                pos_x=ctx.tx(num_x),
                pos_y=ctx.ty(num_y),
                size_x=size_x,
                size_y=size_y,
                angle=text_orient,
                h_align=h_align,
                v_align=v_align,
                class_name="pin-number",
                ctx=ctx,
                stroke_color=ctx.theme.get_pin_number_color(),
            )

        if draws_name:
            size_x, size_y = _text_effect_sizes(pin.name_effects)
            pen_width = _text_effect_pen_width(
                pin.name_effects,
                default=ctx.pin_stroke_width,
            )
            text_clearance = _PIN_TEXT_MARGIN_MM + pen_width
            h_align = "center"
            v_align = "bottom"

            if symbol.pin_names_offset > 0:
                offset = float(symbol.pin_names_offset)
                if horizontal:
                    name_x = root_x + offset if pin_right else root_x - offset
                    name_y = root_y
                    h_align = "left" if pin_right else "right"
                else:
                    name_x = root_x
                    name_y = root_y - offset if pin_down else root_y + offset
                    h_align = "right" if pin_down else "left"
                v_align = "center"
            elif horizontal:
                name_x = midpoint_x
                name_y = root_y + text_clearance
            else:
                name_x = root_x - text_clearance
                name_y = midpoint_y

            _append_stroked_text(
                lines,
                text=pin.name,
                pos_x=ctx.tx(name_x),
                pos_y=ctx.ty(name_y),
                size_x=size_x,
                size_y=size_y,
                angle=text_orient,
                h_align=h_align,
                v_align=v_align,
                class_name="pin-name",
                ctx=ctx,
            )

    return lines


def _render_subsymbol(
    subsym: 'LibSubSymbol',
    ctx: SymbolSvgContext,
) -> List[str]:
    """Render a subsymbol's graphics and pins.

    Args:
        subsym: The subsymbol to render
        ctx: Render context with theme, visibility settings, and transforms
    """
    lines = []
    lines.append(f'  <g id="subsymbol_{subsym.name}">')

    # Render filled shapes first (back to front)
    for rect in subsym.rectangles:
        lines.extend(_render_rectangle(rect, ctx))

    for circle in subsym.circles:
        lines.extend(_render_circle(circle, ctx))

    for poly in subsym.polylines:
        lines.extend(_render_polyline(poly, ctx))

    for arc in subsym.arcs:
        lines.extend(_render_arc(arc, ctx))

    for bezier in subsym.beziers:
        lines.extend(_render_bezier(bezier, ctx))

    # Render text
    for text in subsym.texts:
        lines.extend(_render_text(text, ctx))

    # Render pins
    for pin in subsym.pins:
        if pin.hide and not ctx.show_hidden_pins:
            continue
        lines.extend(_render_pin(pin, ctx))

    lines.append('  </g>')
    return lines


def _render_rectangle(rect, ctx: SymbolSvgContext) -> List[str]:
    """Render a rectangle element."""
    from .kicad_sym_rectangle import SymFillType

    x = min(rect.start_x, rect.end_x)
    y = min(rect.start_y, rect.end_y)
    w = abs(rect.end_x - rect.start_x)
    h = abs(rect.end_y - rect.start_y)

    # Determine fill and stroke - use theme defaults if element has 0 width
    stroke_width = rect.stroke.width if rect.stroke.width > 0 else ctx.body_stroke_width

    if rect.fill.type == SymFillType.NONE:
        css_class = "body-outline"
    elif rect.fill.type == SymFillType.OUTLINE:
        css_class = "body-outline"
    else:  # BACKGROUND or COLOR
        css_class = "body-filled"

    return [f'    <rect x="{ctx.fmt(x)}" y="{ctx.fmt(y)}" width="{ctx.fmt(w)}" height="{ctx.fmt(h)}" '
            f'class="{css_class}" stroke-width="{ctx.fmt(stroke_width)}"/>']


def _render_circle(circle, ctx: SymbolSvgContext) -> List[str]:
    """Render a circle element."""
    from .kicad_sym_rectangle import SymFillType

    stroke_width = circle.stroke.width if circle.stroke.width > 0 else ctx.body_stroke_width

    if circle.fill.type in (SymFillType.NONE, SymFillType.OUTLINE):
        css_class = "body-outline"
    else:
        css_class = "body-filled"

    return [f'    <circle cx="{ctx.fmt(circle.center_x)}" cy="{ctx.fmt(circle.center_y)}" '
            f'r="{ctx.fmt(circle.radius)}" class="{css_class}" stroke-width="{ctx.fmt(stroke_width)}"/>']


def _render_polyline(poly, ctx: SymbolSvgContext) -> List[str]:
    """Render a polyline element."""
    from .kicad_sym_rectangle import SymFillType

    if len(poly.points) < 2:
        return []

    stroke_width = poly.stroke.width if poly.stroke.width > 0 else ctx.body_stroke_width

    # Build path
    points_str = " ".join(f"{ctx.fmt(x)},{ctx.fmt(y)}" for x, y in poly.points)

    if poly.fill.type in (SymFillType.NONE, SymFillType.OUTLINE):
        css_class = "body-outline"
        fill = "none"
    else:
        css_class = "body-filled"
        fill = ctx.body_fill

    return [f'    <polyline points="{points_str}" class="{css_class}" '
            f'fill="{fill}" stroke-width="{ctx.fmt(stroke_width)}"/>']


def _render_arc(arc, ctx: SymbolSvgContext) -> List[str]:
    """Render an arc element."""
    from .kicad_sym_rectangle import SymFillType

    stroke_width = arc.stroke.width if arc.stroke.width > 0 else ctx.body_stroke_width

    # KiCad arcs are defined by start, mid, end points
    # We need to calculate the arc parameters for SVG
    # SVG uses center, radius, and angles

    # For now, approximate with a polyline
    # TODO: Proper arc calculation using SVG arc command
    points = _arc_to_points(arc.start_x, arc.start_y, arc.mid_x, arc.mid_y,
                            arc.end_x, arc.end_y, segments=16)

    if not points:
        return []

    points_str = " ".join(f"{ctx.fmt(x)},{ctx.fmt(y)}" for x, y in points)

    if arc.fill.type in (SymFillType.NONE, SymFillType.OUTLINE):
        fill = "none"
    else:
        fill = ctx.body_fill

    return [f'    <polyline points="{points_str}" class="body-outline" '
            f'fill="{fill}" stroke-width="{ctx.fmt(stroke_width)}"/>']


def _render_bezier(bezier, ctx: SymbolSvgContext) -> List[str]:
    """Render a bezier curve."""
    if len(bezier.points) < 4:
        return []

    stroke_width = bezier.stroke.width if bezier.stroke.width > 0 else ctx.body_stroke_width

    # SVG cubic bezier: M x0,y0 C x1,y1 x2,y2 x3,y3
    p = bezier.points
    path = f"M {ctx.fmt(p[0][0])},{ctx.fmt(p[0][1])}"

    # Process control points in groups of 3 (for cubic bezier)
    i = 1
    while i + 2 < len(p):
        path += f" C {ctx.fmt(p[i][0])},{ctx.fmt(p[i][1])} {ctx.fmt(p[i+1][0])},{ctx.fmt(p[i+1][1])} {ctx.fmt(p[i+2][0])},{ctx.fmt(p[i+2][1])}"
        i += 3

    return [f'    <path d="{path}" class="body-outline" fill="none" stroke-width="{ctx.fmt(stroke_width)}"/>']


def _render_text(text, ctx: SymbolSvgContext) -> List[str]:
    """Render a text element."""
    font_size = 1.27
    if text.effects and text.effects.font:
        font_size = text.effects.font.size_y

    # Handle text anchor based on justification
    # Note: Effects.justify is a List[str] like ['left', 'top'], not a Justify object
    anchor = "middle"
    if text.effects and text.effects.justify:
        if "left" in text.effects.justify:
            anchor = "start"
        elif "right" in text.effects.justify:
            anchor = "end"

    escaped = _escape_xml(text.text)
    return [f'    <text x="{ctx.fmt(text.at_x)}" y="{ctx.fmt(text.at_y)}" '
            f'class="property" font-size="{ctx.fmt(font_size)}" '
            f'text-anchor="{anchor}" dominant-baseline="middle">{escaped}</text>']


def _render_pin(pin, ctx: SymbolSvgContext) -> List[str]:
    """Render a pin with optional name and number.

    Args:
        pin: The SymPin to render
        ctx: Render context with visibility settings (show_pin_names, show_pin_numbers)
    """
    from .kicad_sch_enums import PinGraphicStyle

    lines = []

    # Calculate pin endpoints
    rad = math.radians(pin.at_angle)
    end_x = pin.at_x + pin.length * math.cos(rad)
    end_y = pin.at_y - pin.length * math.sin(rad)  # Y inverted in KiCad

    # Determine effective end point (may be shortened for decorations)
    effective_end_x = end_x
    effective_end_y = end_y
    decoration_radius = 0.3  # Radius of inversion bubble

    if pin.graphic_style == PinGraphicStyle.INVERTED:
        # Shorten line for inversion bubble
        effective_end_x = end_x - decoration_radius * 2 * math.cos(rad)
        effective_end_y = end_y + decoration_radius * 2 * math.sin(rad)

    # Pin line
    lines.append(f'    <line x1="{ctx.fmt(pin.at_x)}" y1="{ctx.fmt(pin.at_y)}" '
                f'x2="{ctx.fmt(effective_end_x)}" y2="{ctx.fmt(effective_end_y)}" class="pin"/>')

    # Pin graphic style decorations
    if pin.graphic_style == PinGraphicStyle.INVERTED:
        # Inversion bubble at end
        bubble_x = end_x - decoration_radius * math.cos(rad)
        bubble_y = end_y + decoration_radius * math.sin(rad)
        lines.append(f'    <circle cx="{ctx.fmt(bubble_x)}" cy="{ctx.fmt(bubble_y)}" '
                    f'r="{ctx.fmt(decoration_radius)}" class="pin-decoration"/>')

    elif pin.graphic_style == PinGraphicStyle.CLOCK:
        # Clock wedge at connection point
        wedge_size = 0.4
        # Triangle pointing inward
        p1_x = pin.at_x
        p1_y = pin.at_y
        p2_x = pin.at_x + wedge_size * math.cos(rad + math.pi/4)
        p2_y = pin.at_y - wedge_size * math.sin(rad + math.pi/4)
        p3_x = pin.at_x + wedge_size * math.cos(rad - math.pi/4)
        p3_y = pin.at_y - wedge_size * math.sin(rad - math.pi/4)
        lines.append(f'    <polyline points="{ctx.fmt(p2_x)},{ctx.fmt(p2_y)} {ctx.fmt(p1_x)},{ctx.fmt(p1_y)} '
                    f'{ctx.fmt(p3_x)},{ctx.fmt(p3_y)}" class="pin-decoration" fill="none"/>')

    elif pin.graphic_style == PinGraphicStyle.INVERTED_CLOCK:
        # Both inversion bubble and clock
        bubble_x = end_x - decoration_radius * math.cos(rad)
        bubble_y = end_y + decoration_radius * math.sin(rad)
        lines.append(f'    <circle cx="{ctx.fmt(bubble_x)}" cy="{ctx.fmt(bubble_y)}" '
                    f'r="{ctx.fmt(decoration_radius)}" class="pin-decoration"/>')
        # Clock wedge
        wedge_size = 0.4
        p1_x = pin.at_x
        p1_y = pin.at_y
        p2_x = pin.at_x + wedge_size * math.cos(rad + math.pi/4)
        p2_y = pin.at_y - wedge_size * math.sin(rad + math.pi/4)
        p3_x = pin.at_x + wedge_size * math.cos(rad - math.pi/4)
        p3_y = pin.at_y - wedge_size * math.sin(rad - math.pi/4)
        lines.append(f'    <polyline points="{ctx.fmt(p2_x)},{ctx.fmt(p2_y)} {ctx.fmt(p1_x)},{ctx.fmt(p1_y)} '
                    f'{ctx.fmt(p3_x)},{ctx.fmt(p3_y)}" class="pin-decoration" fill="none"/>')

    # Pin number (near symbol body) - use visibility from context
    if ctx.show_pin_numbers and pin.number:
        font_size = 0.8
        if pin.number_effects and pin.number_effects.font:
            font_size = pin.number_effects.font.size_y

        # Position number perpendicular to pin, offset from body
        num_offset = 0.4
        perp_rad = rad + math.pi / 2  # Perpendicular

        # Position along pin (closer to body)
        along_offset = pin.length * 0.25
        num_x = pin.at_x + along_offset * math.cos(rad) + num_offset * math.cos(perp_rad)
        num_y = pin.at_y - along_offset * math.sin(rad) - num_offset * math.sin(perp_rad)

        escaped = _escape_xml(pin.number)
        lines.append(f'    <text x="{ctx.fmt(num_x)}" y="{ctx.fmt(num_y)}" '
                    f'class="pin-number" font-size="{ctx.fmt(font_size)}" '
                    f'text-anchor="middle" dominant-baseline="middle">{escaped}</text>')

    # Pin name (beyond pin end) - use visibility from context
    if ctx.show_pin_names and pin.name and pin.name != "~":
        font_size = 0.8
        if pin.name_effects and pin.name_effects.font:
            font_size = pin.name_effects.font.size_y

        # Position name beyond the pin end
        name_offset = 0.5
        name_x = end_x + name_offset * math.cos(rad)
        name_y = end_y - name_offset * math.sin(rad)

        # Text anchor depends on pin direction
        angle_norm = pin.at_angle % 360
        if angle_norm == 0:  # Pointing right
            anchor = "start"
        elif angle_norm == 180:  # Pointing left
            anchor = "end"
        else:
            anchor = "middle"

        escaped = _escape_xml(pin.name)
        lines.append(f'    <text x="{ctx.fmt(name_x)}" y="{ctx.fmt(name_y)}" '
                    f'class="pin-name" font-size="{ctx.fmt(font_size)}" '
                    f'text-anchor="{anchor}" dominant-baseline="middle">{escaped}</text>')

    return lines


def _render_properties(symbol: 'LibSymbol', ctx: SymbolSvgContext) -> List[str]:
    """Render symbol properties (Reference, Value, etc.).

    Args:
        symbol: The LibSymbol containing properties
        ctx: Render context with visibility settings (show_hidden_fields)
    """
    lines = []

    for prop in symbol.properties:
        # Skip hidden unless requested
        if prop.effects and prop.effects.hide and not ctx.show_hidden_fields:
            continue

        font_size = 1.27
        if prop.effects and prop.effects.font:
            font_size = prop.effects.font.size_y

        # Handle text anchor
        # Note: Effects.justify is a List[str] like ['left', 'top'], not a Justify object
        anchor = "middle"
        if prop.effects and prop.effects.justify:
            if "left" in prop.effects.justify:
                anchor = "start"
            elif "right" in prop.effects.justify:
                anchor = "end"

        escaped = _escape_xml(prop.value)
        lines.append(f'  <text x="{ctx.fmt(prop.at_x)}" y="{ctx.fmt(prop.at_y)}" '
                    f'class="property" font-size="{ctx.fmt(font_size)}" '
                    f'text-anchor="{anchor}" dominant-baseline="middle">{escaped}</text>')

    return lines


def _arc_to_points(
    start_x: float, start_y: float,
    mid_x: float, mid_y: float,
    end_x: float, end_y: float,
    segments: int = 16
) -> List[tuple]:
    """Convert 3-point arc to polyline approximation.

    Given start, mid, end points, calculate arc center and generate points.
    """
    # Calculate circle through three points
    # Using circumcircle formula
    ax, ay = start_x, start_y
    bx, by = mid_x, mid_y
    cx, cy = end_x, end_y

    d = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by))
    if abs(d) < 1e-10:
        # Points are collinear, return straight line
        return [(ax, ay), (bx, by), (cx, cy)]

    # Circle center
    ux = ((ax*ax + ay*ay) * (by - cy) + (bx*bx + by*by) * (cy - ay) + (cx*cx + cy*cy) * (ay - by)) / d
    uy = ((ax*ax + ay*ay) * (cx - bx) + (bx*bx + by*by) * (ax - cx) + (cx*cx + cy*cy) * (bx - ax)) / d

    # Radius
    r = math.sqrt((ax - ux)**2 + (ay - uy)**2)

    # Angles
    start_angle = math.atan2(ay - uy, ax - ux)
    mid_angle = math.atan2(by - uy, bx - ux)
    end_angle = math.atan2(cy - uy, cx - ux)

    # Determine direction (CW or CCW) based on mid point
    # Normalize angles to [0, 2*pi)
    def normalize(a):
        while a < 0:
            a += 2 * math.pi
        while a >= 2 * math.pi:
            a -= 2 * math.pi
        return a

    start_angle = normalize(start_angle)
    mid_angle = normalize(mid_angle)
    end_angle = normalize(end_angle)

    # Check if we go CCW (start -> mid -> end) or CW
    def angle_between(a, b, c):
        """Check if b is between a and c going CCW."""
        if a <= c:
            return a <= b <= c
        else:
            return b >= a or b <= c

    if angle_between(start_angle, mid_angle, end_angle):
        # CCW
        if end_angle <= start_angle:
            end_angle += 2 * math.pi
    else:
        # CW - swap direction
        if end_angle >= start_angle:
            end_angle -= 2 * math.pi

    # Generate points
    points = []
    for i in range(segments + 1):
        t = i / segments
        angle = start_angle + t * (end_angle - start_angle)
        x = ux + r * math.cos(angle)
        y = uy + r * math.sin(angle)
        points.append((x, y))

    return points


def _empty_svg(name: str) -> str:
    """Return an empty SVG document."""
    return f'''<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN"
  "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg xmlns="http://www.w3.org/2000/svg" version="1.1"
  width="10mm" height="10mm" viewBox="0 0 10 10">
  <title>{_escape_xml(name)}</title>
</svg>
'''


def _escape_xml(text: str) -> str:
    """Escape special XML characters."""
    return (text
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace('"', "&quot;")
            .replace("'", "&apos;"))


def _sanitize_filename(name: str) -> str:
    """Sanitize symbol name for use as filename."""
    # Replace problematic characters
    replacements = {
        '/': '_',
        '\\': '_',
        ':': '_',
        '*': '_',
        '?': '_',
        '"': '_',
        '<': '_',
        '>': '_',
        '|': '_',
    }
    for old, new in replacements.items():
        name = name.replace(old, new)
    return name


__all__ = [
    'SymbolTheme',
    'SymbolSvgContext',
    'SymbolRenderOptions',
    'render_symbol_svg',
    'render_library_svg',
]

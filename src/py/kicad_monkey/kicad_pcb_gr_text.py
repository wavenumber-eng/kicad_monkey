"""
GrText - Graphical text element (gr_text)

Represents text on PCB layers with full support for:
- Multiple fonts (outline fonts via FreeType)
- Bold, italic, mirrored text
- Knockout text (text with background cutout)
- Rotation and alignment
- Integration with kicad_text.py renderer for accurate polygon conversion
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional, Tuple, TYPE_CHECKING

from .kicad_base import (
    FRONT_SILKSCREEN_LAYER,
    ToPolyMixin,
    QuotedString,
    find_element,
    get_value,
    has_flag,
    unquote_string,
)
from .kicad_primitives import RenderCache
from .kicad_pcb_polygon_ops import PolygonSet, DEFAULT_ERROR_MM
from .kicad_sexpr import SexpList

# Import from geometry module for TextParams
if TYPE_CHECKING:
    from .kicad_geometry import TextParams, RenderedGeometry
    from .kicad_text import KiCadTextRenderer


# =============================================================================
# Supporting Data Classes
# =============================================================================

@dataclass(slots=True)
class Font:
    """Font parameters for text elements.

    `thickness` is `None` when the source omitted `(thickness ...)` —
    KiCad's "auto thickness" form, elided on canonical emit per
    ``EDA_TEXT::Format`` in `kicad/common/eda_text.cpp`.

    KiCad serializes ``(size height width)`` even though the in-memory
    ``TEXT_ATTRIBUTES`` vector stores ``x=width`` and ``y=height``
    (``parseEDA_TEXT`` reads height then width), so ``size_x`` holds the
    second file value and ``size_y`` the first.
    """
    face: Optional[str] = None
    size_x: float = 1.27
    size_y: float = 1.27
    thickness: Optional[float] = None
    bold: bool = False
    italic: bool = False
    line_spacing: Optional[float] = None

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Font':
        """Parse from s-expression."""
        font_elem = find_element(sexp, 'font')
        if not font_elem:
            return cls()

        face = get_value(font_elem, 'face')
        size_elem = find_element(font_elem, 'size')
        size_y = float(size_elem[1]) if size_elem and len(size_elem) > 1 else 1.27
        size_x = float(size_elem[2]) if size_elem and len(size_elem) > 2 else 1.27
        thickness_elem = find_element(font_elem, 'thickness')
        thickness = float(thickness_elem[1]) if thickness_elem and len(thickness_elem) > 1 else None
        line_spacing_elem = find_element(font_elem, 'line_spacing')
        line_spacing = (
            float(line_spacing_elem[1])
            if line_spacing_elem and len(line_spacing_elem) > 1
            else None
        )
        bold = has_flag(font_elem, 'bold') or get_value(font_elem, 'bold') == 'yes'
        italic = has_flag(font_elem, 'italic') or get_value(font_elem, 'italic') == 'yes'

        return cls(
            face=unquote_string(face) if face else None,
            size_x=size_x,
            size_y=size_y,
            thickness=thickness,
            line_spacing=line_spacing,
            bold=bold,
            italic=italic
        )

    @property
    def effective_thickness(self) -> float:
        """Resolved thickness for renderers needing a concrete pen width.
        Falls back to KiCad's normal/bold auto-thickness rules when None."""
        if self.thickness is not None:
            return self.thickness
        text_width = abs(self.size_x) or abs(self.size_y)
        if not text_width:
            return 0.15
        pen_width = text_width / 5.0 if self.bold else text_width / 8.0
        min_size = min(abs(self.size_x), abs(self.size_y))
        if min_size:
            pen_width = min(pen_width, min_size * 0.25)
        return pen_width

    def to_sexp(self) -> list:
        result: SexpList = ['font']
        if self.face:
            result.append(['face', QuotedString(self.face)])
        result.append(['size', self.size_y, self.size_x])
        if self.line_spacing is not None and self.line_spacing != 1.0:
            result.append(['line_spacing', self.line_spacing])
        if self.thickness is not None:
            result.append(['thickness', self.thickness])
        if self.bold:
            result.append(['bold', 'yes'])
        if self.italic:
            result.append(['italic', 'yes'])
        return result


@dataclass(slots=True)
class Effects:
    """Text effects including font and justification."""
    font: Font = field(default_factory=Font)
    justify: Optional[List[str]] = None
    hide: bool = False

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Effects':
        """Parse from s-expression."""
        effects_elem = find_element(sexp, 'effects')
        if not effects_elem:
            return cls()

        font = Font.from_sexp(effects_elem)
        justify_elem = find_element(effects_elem, 'justify')
        justify = list(justify_elem[1:]) if justify_elem else None
        # KiCad 9 nested (hide yes) inside (effects ...); KiCad 10 hoists
        # it to the parent text element. Accept either form on parse.
        hide = has_flag(effects_elem, 'hide') or get_value(effects_elem, 'hide') == 'yes'

        return cls(font=font, justify=justify, hide=hide)

    def to_sexp(self) -> list:
        result = ['effects', self.font.to_sexp()]
        if self.justify:
            result.append(['justify'] + self.justify)
        if self.hide:
            result.append('hide')
        return result


# =============================================================================
# GrText Element
# =============================================================================

def _get_at(sexp: list) -> Tuple[float, float, float]:
    """Extract (x, y, angle) from 'at' element."""
    at_elem = find_element(sexp, 'at')
    if at_elem:
        x = float(at_elem[1]) if len(at_elem) > 1 else 0.0
        y = float(at_elem[2]) if len(at_elem) > 2 else 0.0
        angle = float(at_elem[3]) if len(at_elem) > 3 else 0.0
        return (x, y, angle)
    return (0.0, 0.0, 0.0)


@dataclass
class GrText(ToPolyMixin):
    """
    Graphical text element (gr_text).

    Supports full KiCad text rendering including:
    - Custom fonts (via font face name)
    - Bold, italic, mirrored text
    - Knockout text (text with background cutout)
    - Pre-computed render_cache for complex fonts

    Polygon conversion uses the kicad_text.py renderer for accurate results.
    """
    text: str
    at_x: float
    at_y: float
    at_angle: float = 0.0
    layer: str = FRONT_SILKSCREEN_LAYER
    knockout: bool = False
    uuid: Optional[str] = None
    effects: Effects = field(default_factory=Effects)
    render_cache: Optional[RenderCache] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    # Class-level renderer instance (lazy initialized)
    _renderer: Optional['KiCadTextRenderer'] = None

    @classmethod
    def from_sexp(cls, sexp: list) -> 'GrText':
        """Parse from s-expression."""
        text = unquote_string(sexp[1])
        x, y, angle = _get_at(sexp)

        layer_elem = find_element(sexp, 'layer')
        layer = unquote_string(layer_elem[1]) if layer_elem else FRONT_SILKSCREEN_LAYER
        knockout = has_flag(layer_elem, 'knockout') if layer_elem else False

        uuid = unquote_string(get_value(sexp, 'uuid'))
        effects = Effects.from_sexp(sexp)
        render_cache = RenderCache.from_sexp(sexp)

        return cls(
            text=text,
            at_x=x, at_y=y, at_angle=angle,
            layer=layer,
            knockout=knockout,
            uuid=uuid,
            effects=effects,
            render_cache=render_cache,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to s-expression."""
        result = ['gr_text', QuotedString(self.text)]

        # KiCad's reader requires the angle slot even when zero (drift inventory #1).
        result.append(['at', self.at_x, self.at_y, self.at_angle])

        layer_elem = ['layer', QuotedString(self.layer)]
        if self.knockout:
            layer_elem.append('knockout')
        result.append(layer_elem)

        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])

        result.append(self.effects.to_sexp())

        if self.render_cache:
            result.append(self.render_cache.to_sexp())

        return result

    @property
    def position(self) -> Tuple[float, float]:
        """Get text position as tuple."""
        return (self.at_x, self.at_y)

    @property
    def font(self) -> Font:
        """Get font from effects."""
        return self.effects.font

    @property
    def is_bold(self) -> bool:
        """Check if text is bold."""
        return self.effects.font.bold

    @property
    def is_italic(self) -> bool:
        """Check if text is italic."""
        return self.effects.font.italic

    @property
    def is_mirrored(self) -> bool:
        """Check if text is mirrored (from justify)."""
        if self.effects.justify:
            return 'mirror' in self.effects.justify
        return False

    @property
    def h_align(self) -> str:
        """Get horizontal alignment."""
        if self.effects.justify:
            if 'left' in self.effects.justify:
                return 'left'
            elif 'right' in self.effects.justify:
                return 'right'
        return 'center'

    @property
    def v_align(self) -> str:
        """Get vertical alignment."""
        if self.effects.justify:
            if 'top' in self.effects.justify:
                return 'top'
            elif 'bottom' in self.effects.justify:
                return 'bottom'
        return 'center'

    def to_text_params(self) -> 'TextParams':
        """
        Convert to TextParams for use with KiCadTextRenderer.

        Returns:
            TextParams object suitable for rendering
        """
        # Import here to avoid circular imports
        from .kicad_geometry import TextParams, HAlign, VAlign

        # Map alignments
        h_align_map = {'left': HAlign.LEFT, 'center': HAlign.CENTER, 'right': HAlign.RIGHT}
        v_align_map = {'top': VAlign.TOP, 'center': VAlign.CENTER, 'bottom': VAlign.BOTTOM}

        # Convert render_cache if present
        render_cache_points: List[List[Tuple[float, float]]] = []
        if self.render_cache:
            for poly in self.render_cache.polygons:
                points = [(p[0], p[1]) for p in poly.points]
                render_cache_points.append(points)

        return TextParams(
            text=self.text,
            font_name=self.font.face or "Arial",
            size_x=self.font.size_x,
            size_y=self.font.size_y,
            position_x=self.at_x,
            position_y=self.at_y,
            angle=self.at_angle,
            bold=self.is_bold,
            italic=self.is_italic,
            mirrored=self.is_mirrored,
            h_align=h_align_map.get(self.h_align, HAlign.CENTER),
            v_align=v_align_map.get(self.v_align, VAlign.CENTER),
            stroke_width=self.font.effective_thickness,
            line_spacing=self.font.line_spacing or 1.0,
            knockout=self.knockout,
            layer=self.layer,
            render_cache=render_cache_points,
        )

    @classmethod
    def _get_renderer(cls) -> 'KiCadTextRenderer':
        """Get or create the text renderer instance."""
        if cls._renderer is None:
            from .kicad_text import KiCadTextRenderer
            cls._renderer = KiCadTextRenderer()
        return cls._renderer

    def _to_poly(self, error: float = DEFAULT_ERROR_MM) -> PolygonSet:
        """
        Convert text to polygon using the KiCad text renderer.

        If render_cache is present in the PCB file, uses that directly.
        Otherwise, renders text using FreeType + HarfBuzz.

        Args:
            error: Not used for text (text renderer has its own tolerance)

        Returns:
            PolygonSet with text polygon contours
        """
        # If we have render_cache, use it directly
        if self.render_cache and self.render_cache.polygons:
            contours = [list(poly.points) for poly in self.render_cache.polygons]
            return PolygonSet(outlines=contours)

        # Otherwise, use the text renderer
        try:
            renderer = self._get_renderer()
            params = self.to_text_params()
            geometry = renderer.render(params)

            # Convert RenderedGeometry contours to PolygonSet
            contours = []
            for contour in geometry.contours:
                # Handle both Point objects and tuple points
                points = []
                for p in contour.points:
                    px = getattr(p, "x", None)
                    py = getattr(p, "y", None)
                    if px is not None and py is not None:
                        points.append((px, py))
                    else:
                        points.append((p[0], p[1]))
                contours.append(points)

            return PolygonSet(outlines=contours)

        except Exception as e:
            # If rendering fails, return empty polygon
            import warnings
            warnings.warn(f"Text rendering failed for '{self.text}': {e}")
            return PolygonSet()

    def render(self) -> 'RenderedGeometry':
        """
        Render text to RenderedGeometry using the KiCad text renderer.

        This is a higher-level method that returns the full RenderedGeometry
        object with bounding box and metadata.

        Returns:
            RenderedGeometry object
        """
        renderer = self._get_renderer()
        params = self.to_text_params()
        return renderer.render(params)

    def get_knockout_margin(self) -> float:
        """
        Calculate knockout text margin using KiCad's formula.

        From KiCad gr_text.h GetKnockoutTextMargin():
            margin = max(thickness / 2.0, size_y / 9.0)

        Returns:
            Margin in mm to expand bounding box for knockout background
        """
        return max(self.font.effective_thickness / 2.0, self.font.size_y / 9.0)

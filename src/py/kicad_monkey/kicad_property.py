"""
KiCad Property Element

One class per file.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import math
from typing import List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_footprint import KiCadFootprint
    from .kicad_geometry import BoundingBox, SvgRenderContext, TextParams
from .kicad_defaults import KICAD_DEFAULT_TEXT_SIZE_MM
from .kicad_sexpr import QuotedString
from .kicad_base import (
    FRONT_SILKSCREEN_LAYER,
    find_element,
    get_value,
    get_at,
    has_flag,
    unquote_string,
)
from .kicad_primitives import Effects, RenderCache


@dataclass
class Property:
    """Footprint property."""
    name: str
    value: str
    at_x: float = 0.0
    at_y: float = 0.0
    at_angle: float = 0.0
    layer: str = FRONT_SILKSCREEN_LAYER
    hide: bool = False
    unlocked: bool = False
    uuid: Optional[str] = None
    effects: Effects = field(default_factory=Effects)
    render_cache: Optional[RenderCache] = None
    graphical: bool = True
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Property':
        name = unquote_string(sexp[1])
        value = unquote_string(sexp[2])
        x, y, angle = get_at(sexp)
        layer_elem = find_element(sexp, 'layer')
        layer = (
            unquote_string(layer_elem[1])
            if layer_elem and len(layer_elem) > 1
            else FRONT_SILKSCREEN_LAYER
        )
        effects = Effects.from_sexp(sexp)
        hide = (
            has_flag(sexp, 'hide')
            or get_value(sexp, 'hide') == 'yes'
            or bool(effects.hide)
        )
        unlocked = has_flag(sexp, 'unlocked') or get_value(sexp, 'unlocked') == 'yes'
        uuid = unquote_string(get_value(sexp, 'uuid'))
        render_cache = RenderCache.from_sexp(sexp)
        graphical = find_element(sexp, 'at') is not None and layer_elem is not None

        return cls(
            name=name, value=value,
            at_x=x, at_y=y, at_angle=angle,
            layer=layer, hide=hide, unlocked=unlocked,
            uuid=uuid, effects=effects, render_cache=render_cache,
            graphical=graphical,
            _raw_sexp=sexp
        )

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of this property.."""
        from .kicad_geometry import BoundingBox, rotate_point

        font_width = KICAD_DEFAULT_TEXT_SIZE_MM
        font_height = KICAD_DEFAULT_TEXT_SIZE_MM
        if self.effects and self.effects.font:
            font_width = self.effects.font.size_x
            font_height = self.effects.font.size_y

        # Estimate text dimensions
        text = self.value or 'X'
        text_width = len(text) * font_width * 0.7
        text_height = font_height

        # Text is centered at position by default
        half_w = text_width / 2
        half_h = text_height / 2

        # Create corners and rotate if needed
        corners = [
            (-half_w, -half_h),
            (half_w, -half_h),
            (half_w, half_h),
            (-half_w, half_h),
        ]

        if self.at_angle != 0:
            corners = [rotate_point(cx, cy, -self.at_angle) for cx, cy in corners]

        bbox = BoundingBox()
        for cx, cy in corners:
            bbox.expand((self.at_x + cx, self.at_y + cy))

        return bbox

    @property
    def is_mirrored(self) -> bool:
        """Check if text is mirrored."""
        if self.effects.justify:
            return 'mirror' in self.effects.justify
        return False

    @property
    def h_align(self) -> str:
        """Get horizontal alignment."""
        if self.effects.justify:
            if 'left' in self.effects.justify:
                return 'left'
            if 'right' in self.effects.justify:
                return 'right'
        return 'center'

    @property
    def v_align(self) -> str:
        """Get vertical alignment."""
        if self.effects.justify:
            if 'top' in self.effects.justify:
                return 'top'
            if 'bottom' in self.effects.justify:
                return 'bottom'
        return 'center'

    @staticmethod
    def _rotate_point(x: float, y: float, angle: float) -> tuple[float, float]:
        radians = math.radians(angle)
        cos_a = math.cos(radians)
        sin_a = math.sin(radians)
        return (x * cos_a + y * sin_a, y * cos_a - x * sin_a)

    def to_text_params(self, footprint: 'Optional[KiCadFootprint]' = None) -> 'TextParams':
        """Convert footprint property text to `TextParams` for outline rendering."""

        from .kicad_geometry import HAlign, TextParams, VAlign
        from .kicad_fp_text import keep_upright_draw_angle

        h_align_map = {'left': HAlign.LEFT, 'center': HAlign.CENTER, 'right': HAlign.RIGHT}
        v_align_map = {'top': VAlign.TOP, 'center': VAlign.CENTER, 'bottom': VAlign.BOTTOM}
        font = self.effects.font
        position_x = self.at_x
        position_y = self.at_y
        angle = self.at_angle
        if footprint is not None and not self.unlocked:
            angle = keep_upright_draw_angle(angle)

        if footprint is not None:
            fp_angle = float(getattr(footprint, "at_angle", 0.0) or 0.0)
            fp_x = float(getattr(footprint, "at_x", 0.0) or 0.0)
            fp_y = float(getattr(footprint, "at_y", 0.0) or 0.0)
            position_x, position_y = self._rotate_point(self.at_x, self.at_y, fp_angle)
            position_x += fp_x
            position_y += fp_y

        return TextParams(
            text=self.value,
            font_name=font.face or "Arial",
            size_x=font.size_x,
            size_y=font.size_y,
            position_x=position_x,
            position_y=position_y,
            angle=angle,
            bold=font.bold,
            italic=font.italic,
            mirrored=self.is_mirrored,
            h_align=h_align_map.get(self.h_align, HAlign.CENTER),
            v_align=v_align_map.get(self.v_align, VAlign.CENTER),
            stroke_width=font.effective_thickness,
            line_spacing=font.line_spacing or 1.0,
            layer=self.layer,
        )

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> List[str]:
        """Render this property to SVG..

        Note: KiCad footprint SVG exports do not render property text as <text> elements.
        Text is converted to stroked outlines for CAM output. This method returns
        an empty list to match that behavior. Future versions may add outline rendering.
        """
        from .kicad_geometry import SvgRenderContext

        if ctx is None:
            ctx = SvgRenderContext()

        if not ctx.layer_visible(self.layer):
            return []

        if self.hide:
            return []

        # Properties are not rendered as SVG text elements in footprint exports
        # KiCad converts text to stroke outlines for CAM compatibility
        return []

    def to_sexp(self) -> list:
        result = ['property', QuotedString(self.name), QuotedString(self.value)]
        if not self.graphical:
            return result

        # KiCad's reader requires the angle slot even when zero (drift inventory #1).
        result.append(['at', self.at_x, self.at_y, self.at_angle])

        if self.unlocked:
            result.append(['unlocked', 'yes'])

        result.append(['layer', QuotedString(self.layer)])

        if self.hide:
            result.append(['hide', 'yes'])

        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])

        result.append(self.effects.to_sexp())

        if self.render_cache:
            result.append(self.render_cache.to_sexp())

        return result

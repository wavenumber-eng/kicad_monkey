"""
Text element in a symbol.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Optional

from .kicad_sexpr import QuotedString
from .kicad_base import find_element, get_value, get_at, has_flag, unquote_string
from .kicad_primitives import Effects


if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox, SvgRenderContext


@dataclass
class SymText:
    """Text element in a symbol.

    Static text that appears on the symbol graphics (not a property).
    """
    text: str
    at_x: float = 0.0
    at_y: float = 0.0
    at_angle: float = 0.0
    effects: Optional[Effects] = None
    hide: bool = False
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'SymText':
        """Parse from (text "content" (at X Y A) (effects ...))."""
        text = unquote_string(sexp[1]) if len(sexp) > 1 else ""

        x, y, angle = get_at(sexp)
        angle = angle / 10.0

        effects_elem = find_element(sexp, 'effects')
        effects = Effects.from_sexp(sexp) if effects_elem else None
        # KiCad 10 emits (hide yes) at the text level while older files may
        # carry a nested hide flag inside effects.
        hide = (
            has_flag(sexp, 'hide')
            or get_value(sexp, 'hide') == 'yes'
            or (effects is not None and effects.hide)
        )

        uuid = unquote_string(get_value(sexp, 'uuid')) if get_value(sexp, 'uuid') else None

        return cls(
            text=text,
            at_x=x, at_y=y, at_angle=angle,
            effects=effects, hide=hide, uuid=uuid,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to S-expression list."""
        result = ['text', QuotedString(self.text)]

        # KiCad's reader requires the angle slot even when zero (drift inventory #1).
        result.append(['at', self.at_x, self.at_y, int(round(self.at_angle * 10.0))])

        if self.effects:
            # KiCad's symbol-library parser only accepts `(at ...)` and
            # `(effects ...)` under static symbol text.  Hidden library text
            # is no longer supported by KiCad and is converted to a hidden
            # field on load, so do not emit either legacy or text-level hide
            # flags here.
            if self.effects.hide or self.hide:
                effects_emit = Effects(
                    font=self.effects.font,
                    justify=self.effects.justify,
                    hide=False,
                    href=self.effects.href,
                ).to_sexp()
            else:
                effects_emit = self.effects.to_sexp()
            result.append(effects_emit)

        if self.uuid:
            result.append(['uuid', self.uuid])

        return result

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of the text (approximate)."""
        from .kicad_geometry import BoundingBox
        font_size = 1.27
        if self.effects and self.effects.font:
            font_size = self.effects.font.size_y
        text_width = len(self.text or 'X') * font_size * 0.7
        half_w, half_h = text_width / 2, font_size / 2
        bbox = BoundingBox()
        bbox.expand((self.at_x - half_w, self.at_y - half_h))
        bbox.expand((self.at_x + half_w, self.at_y + half_h))
        return bbox

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> list[str]:
        """Render text to SVG element."""
        from .kicad_geometry import SvgRenderContext
        if ctx is None:
            ctx = SvgRenderContext()

        font_size = 1.27
        if self.effects and self.effects.font:
            font_size = self.effects.font.size_y

        # Handle rotation
        transform = ""
        if self.at_angle != 0:
            transform = f' transform="rotate({-self.at_angle} {ctx.fmt(self.at_x)} {ctx.fmt(self.at_y)})"'

        return [f'<text x="{ctx.fmt(self.at_x)}" y="{ctx.fmt(self.at_y)}" '
                f'font-size="{ctx.fmt(font_size)}" text-anchor="middle" '
                f'dominant-baseline="middle"{transform}>{self.text}</text>']


__all__ = ['SymText']

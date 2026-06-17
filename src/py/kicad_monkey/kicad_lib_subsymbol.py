"""
Sub-symbol representing a unit/style variant.

Multi-unit symbols (like op-amps) have multiple sub-symbols:
- LM324_1_0: Unit 1, normal style
- LM324_1_1: Unit 1, De Morgan style
- LM324_2_0: Unit 2, normal style
etc.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, List, Optional

from .kicad_base import find_all_elements, find_element, unquote_string
from .kicad_sexpr import QuotedString, SexpList

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox, SvgRenderContext
    from .kicad_sym_pin import SymPin
    from .kicad_sym_arc import SymArc
    from .kicad_sym_circle import SymCircle
    from .kicad_sym_polyline import SymPolyline
    from .kicad_sym_rectangle import SymRectangle
    from .kicad_sym_bezier import SymBezier
    from .kicad_sym_text import SymText
    from .kicad_sym_text_box import SymTextBox


@dataclass
class LibSubSymbol:
    """Sub-symbol representing a unit/style variant.

    Multi-unit symbols (like op-amps, quad gates) have multiple sub-symbols.
    The naming convention is "ParentName_unit_style":
    - unit: 1-based unit number (0 = common to all units)
    - style: 0 = normal, 1 = De Morgan alternate

    Examples:
        - LM324_0_0: Common elements for all units
        - LM324_1_0: Unit 1, normal style
        - LM324_1_1: Unit 1, De Morgan style
        - LM324_2_0: Unit 2, normal style
    """
    name: str  # e.g., "LM324_1_0"
    unit: int = 1  # Unit number (1-based, 0 = all units)
    style: int = 0  # 0 = normal, 1 = De Morgan alternate
    unit_name: Optional[str] = None  # Optional KiCad display name for this unit

    # Graphic elements
    arcs: List['SymArc'] = field(default_factory=list)
    circles: List['SymCircle'] = field(default_factory=list)
    polylines: List['SymPolyline'] = field(default_factory=list)
    rectangles: List['SymRectangle'] = field(default_factory=list)
    beziers: List['SymBezier'] = field(default_factory=list)
    texts: List['SymText'] = field(default_factory=list)
    text_boxes: List['SymTextBox'] = field(default_factory=list)
    pins: List['SymPin'] = field(default_factory=list)

    _raw_sexp: list | None = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list, parent_name: str) -> 'LibSubSymbol':
        """Parse from (symbol "ParentName_1_0" ...) element."""
        from .kicad_sym_pin import SymPin
        from .kicad_sym_arc import SymArc
        from .kicad_sym_circle import SymCircle
        from .kicad_sym_polyline import SymPolyline
        from .kicad_sym_rectangle import SymRectangle
        from .kicad_sym_bezier import SymBezier
        from .kicad_sym_text import SymText
        from .kicad_sym_text_box import SymTextBox

        name = unquote_string(sexp[1])

        # Parse unit/style from name suffix (e.g., "_1_0")
        unit, style = 1, 0
        if '_' in name:
            parts = name.rsplit('_', 2)
            if len(parts) >= 3:
                try:
                    style = int(parts[-1])
                    unit = int(parts[-2])
                except ValueError:
                    pass

        # Parse graphic elements
        unit_name_elem = find_element(sexp, 'unit_name')
        unit_name = (
            unquote_string(unit_name_elem[1])
            if unit_name_elem and len(unit_name_elem) > 1
            else None
        )
        arcs = [SymArc.from_sexp(e) for e in find_all_elements(sexp, 'arc')]
        circles = [SymCircle.from_sexp(e) for e in find_all_elements(sexp, 'circle')]
        polylines = [SymPolyline.from_sexp(e) for e in find_all_elements(sexp, 'polyline')]
        rectangles = [SymRectangle.from_sexp(e) for e in find_all_elements(sexp, 'rectangle')]
        beziers = [SymBezier.from_sexp(e) for e in find_all_elements(sexp, 'bezier')]
        texts = [SymText.from_sexp(e) for e in find_all_elements(sexp, 'text')]
        text_boxes = [SymTextBox.from_sexp(e) for e in find_all_elements(sexp, 'text_box')]
        pins = [SymPin.from_sexp(e) for e in find_all_elements(sexp, 'pin')]

        return cls(
            name=name,
            unit=unit,
            style=style,
            unit_name=unit_name,
            arcs=arcs,
            circles=circles,
            polylines=polylines,
            rectangles=rectangles,
            beziers=beziers,
            texts=texts,
            text_boxes=text_boxes,
            pins=pins,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to S-expression list.

        KiCad quotes sub-symbol names and appends the unit/style suffix inside
        the quoted token.  This matters for base names containing characters
        such as ``+`` or ``%``.
        """
        result: SexpList = ['symbol', QuotedString(self.name)]

        if self.unit_name is not None:
            result.append(['unit_name', QuotedString(self.unit_name)])

        for arc in self.arcs:
            result.append(arc.to_sexp())
        for circle in self.circles:
            result.append(circle.to_sexp())
        for poly in self.polylines:
            result.append(poly.to_sexp())
        for rect in self.rectangles:
            result.append(rect.to_sexp())
        for bezier in self.beziers:
            result.append(bezier.to_sexp())
        for text in self.texts:
            result.append(text.to_sexp())
        for tbox in self.text_boxes:
            result.append(tbox.to_sexp())
        for pin in self.pins:
            result.append(pin.to_sexp())

        return result

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of all elements including text."""
        from .kicad_geometry import BoundingBox
        bbox = BoundingBox()
        for elem_list in [self.arcs, self.circles, self.polylines,
                          self.rectangles, self.beziers, self.pins,
                          self.texts, self.text_boxes]:
            for elem in elem_list:
                bbox.merge(elem.get_bounds())
        return bbox

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> list[str]:
        """Render sub-symbol to SVG elements.

        Rendering order: rectangles, circles, polylines, arcs, beziers, pins, texts.
        """
        from .kicad_geometry import SvgRenderContext
        if ctx is None:
            ctx = SvgRenderContext()

        lines = []
        # Render in order: fills first, then strokes, then pins
        for rect in self.rectangles:
            lines.extend(rect.to_svg(ctx))
        for circle in self.circles:
            lines.extend(circle.to_svg(ctx))
        for poly in self.polylines:
            lines.extend(poly.to_svg(ctx))
        for arc in self.arcs:
            lines.extend(arc.to_svg(ctx))
        for bezier in self.beziers:
            lines.extend(bezier.to_svg(ctx))
        for pin in self.pins:
            lines.extend(pin.to_svg(ctx))
        for text in self.texts:
            lines.extend(text.to_svg(ctx))

        return lines

    @property
    def is_common(self) -> bool:
        """Check if this sub-symbol contains elements common to all units."""
        return self.unit == 0

    @property
    def is_demorgan(self) -> bool:
        """Check if this is a De Morgan alternate style."""
        return self.style == 1


__all__ = ['LibSubSymbol']

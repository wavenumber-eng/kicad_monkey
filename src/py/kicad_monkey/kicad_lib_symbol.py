"""
Symbol definition in a .kicad_sym library.

This is the main symbol class that contains all properties, pin settings,
and sub-symbols (units/styles).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Iterator, List, Optional

from ._api_markers import public_api
from .kicad_sexpr import QuotedString
from .kicad_base import find_element, find_all_elements, get_value, has_flag, unquote_string
from .kicad_defaults import KICAD_DEFAULT_PIN_NAME_OFFSET_MM
from .kicad_sch_enums import (
    PropertyId,
    StandardPropertyKey,
    standard_property_id_for_key,
)

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox, SvgRenderContext
    from .kicad_sym_property import SymProperty
    from .kicad_lib_subsymbol import LibSubSymbol


@public_api
@dataclass
class LibSymbol:
    """Symbol definition in a .kicad_sym library.

    Supports symbol inheritance via 'extends' keyword.
    Power symbols typically extend a base symbol.

    Attributes:
        name: Symbol name (unique within library)
        extends: Parent symbol name if this symbol extends another
        pin_numbers_hide: Hide pin numbers on schematic
        pin_names_hide: Hide pin names on schematic
        pin_names_offset: Offset of pin names from symbol body
        in_bom: Include in Bill of Materials
        on_board: Include on PCB
        exclude_from_sim: Exclude from SPICE simulation
        power: This is a power symbol
        properties: List of symbol properties
        subsymbols: List of unit/style variants
    """
    name: str

    # Symbol inheritance (power symbols, etc.)
    extends: Optional[str] = None  # Parent symbol name if this symbol extends another

    # Pin visibility controls
    pin_numbers_hide: bool = False
    pin_names_hide: bool = False
    pin_names_offset: float = KICAD_DEFAULT_PIN_NAME_OFFSET_MM

    # Symbol flags
    in_bom: bool = True
    on_board: bool = True
    exclude_from_sim: bool = False  # KiCad 9
    power: bool = False  # Power symbol flag
    power_kind: Optional[str] = None  # KiCad 10: 'global' or 'local' (after `(power ...)`)
    duplicate_pin_numbers_are_jumpers: bool = False
    jumper_pin_groups: List[List[str]] = field(default_factory=list)

    # Multi-body-style support (KiCad 10). Mirrors LIB_SYMBOL::IsMultiBodyStyle
    # at sch_io_kicad_sexpr_lib_cache.cpp:410. KiCad emits one of:
    #   (body_styles demorgan)
    #   (body_styles "name1" "name2" ...)
    # parseBodyStyles (sch_io_kicad_sexpr_parser.cpp:940) tolerates both
    # the demorgan token and quoted names appearing in any order.
    has_demorgan_body_styles: bool = False
    body_style_names: List[str] = field(default_factory=list)

    # Content
    properties: List['SymProperty'] = field(default_factory=list)
    subsymbols: List['LibSubSymbol'] = field(default_factory=list)

    # Embedded data
    embedded_fonts: List[str] = field(default_factory=list)

    # Round-trip preservation
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    @public_api
    def from_sexp(cls, sexp: list) -> 'LibSymbol':
        """Parse symbol from S-expression."""
        from .kicad_sym_property import SymProperty
        from .kicad_lib_subsymbol import LibSubSymbol

        name = unquote_string(sexp[1])

        # Check for extends (symbol inheritance)
        extends_elem = find_element(sexp, 'extends')
        extends = unquote_string(extends_elem[1]) if extends_elem else None

        # Pin visibility
        # KiCad 9 format uses (hide yes) instead of bare 'hide' flag
        pin_numbers_elem = find_element(sexp, 'pin_numbers')
        pin_numbers_hide = (
            has_flag(pin_numbers_elem, 'hide') or  # Legacy format
            get_value(pin_numbers_elem, 'hide', None) == 'yes'  # KiCad 9 format
        ) if pin_numbers_elem else False

        pin_names_elem = find_element(sexp, 'pin_names')
        pin_names_hide = (
            has_flag(pin_names_elem, 'hide') or  # Legacy format
            get_value(pin_names_elem, 'hide', None) == 'yes'  # KiCad 9 format
        ) if pin_names_elem else False
        pin_names_offset = (
            float(get_value(pin_names_elem, 'offset', KICAD_DEFAULT_PIN_NAME_OFFSET_MM))
            if pin_names_elem else KICAD_DEFAULT_PIN_NAME_OFFSET_MM
        )

        # Flags
        in_bom = get_value(sexp, 'in_bom', 'yes') == 'yes'
        on_board = get_value(sexp, 'on_board', 'yes') == 'yes'
        exclude_from_sim = get_value(sexp, 'exclude_from_sim', 'no') == 'yes'
        duplicate_pin_numbers_are_jumpers = (
            str(get_value(sexp, 'duplicate_pin_numbers_are_jumpers', 'no')).lower()
            in ('yes', 'true', '1')
        )
        jumper_pin_groups: List[List[str]] = []
        jumper_groups_elem = find_element(sexp, 'jumper_pin_groups')
        if jumper_groups_elem:
            for group_elem in jumper_groups_elem[1:]:
                if isinstance(group_elem, list):
                    group = [unquote_string(tok) for tok in group_elem]
                    if group:
                        jumper_pin_groups.append(group)

        # Power symbol flag — KiCad 9 used legacy `(power)` empty list
        # (defaulting to global); KiCad 10's parser at
        # `eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr_parser.cpp:377`
        # accepts `(power)` / `(power global)` / `(power local)` and the
        # emit at `sch_io_kicad_sexpr_lib_cache.cpp:401` always writes the
        # explicit form.
        power = False
        power_kind: Optional[str] = None
        power_elem = find_element(sexp, 'power')
        if power_elem is not None:
            power = True
            if len(power_elem) > 1 and power_elem[1] in ('global', 'local'):
                power_kind = power_elem[1]
        elif has_flag(sexp, 'power'):
            # Legacy bare-token form (defensive — not seen in real fixtures).
            power = True

        # Body styles: parseBodyStyles at
        # eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr_parser.cpp:940
        # treats `demorgan` as a flag and collects every other token as a
        # body-style name.
        has_demorgan_body_styles = False
        body_style_names: List[str] = []
        body_styles_elem = find_element(sexp, 'body_styles')
        if body_styles_elem:
            for tok in body_styles_elem[1:]:
                if tok == 'demorgan':
                    has_demorgan_body_styles = True
                else:
                    body_style_names.append(unquote_string(tok))

        # Properties
        properties = []
        for prop_elem in find_all_elements(sexp, 'property'):
            properties.append(SymProperty.from_sexp(prop_elem))

        # Sub-symbols (units/styles) - nested (symbol ...) elements
        subsymbols = []
        for sub_elem in find_all_elements(sexp, 'symbol'):
            subsymbols.append(LibSubSymbol.from_sexp(sub_elem, name))

        # Embedded fonts
        embedded_fonts = []
        for font_elem in find_all_elements(sexp, 'embedded_fonts'):
            if len(font_elem) > 1:
                embedded_fonts.append(unquote_string(font_elem[1]))

        return cls(
            name=name,
            extends=extends,
            pin_numbers_hide=pin_numbers_hide,
            pin_names_hide=pin_names_hide,
            pin_names_offset=pin_names_offset,
            in_bom=in_bom,
            on_board=on_board,
            exclude_from_sim=exclude_from_sim,
            power=power,
            power_kind=power_kind,
            duplicate_pin_numbers_are_jumpers=duplicate_pin_numbers_are_jumpers,
            jumper_pin_groups=jumper_pin_groups,
            has_demorgan_body_styles=has_demorgan_body_styles,
            body_style_names=body_style_names,
            properties=properties,
            subsymbols=subsymbols,
            embedded_fonts=embedded_fonts,
            _raw_sexp=sexp
        )

    @public_api
    def to_sexp(self) -> list:
        """Serialize to S-expression."""
        result = ['symbol', QuotedString(self.name)]

        # Extends (inheritance)
        if self.extends:
            result.append(['extends', QuotedString(self.extends)])

        # Body styles — emit in KiCad order, between (power ...) and
        # (pin_numbers ...). Mirrors sch_io_kicad_sexpr_lib_cache.cpp:410.
        # `extends` symbols inherit body styles from their parent, so we
        # only emit on root symbols (matches `aSymbol->IsRoot()` guard).
        if not self.extends and (self.has_demorgan_body_styles
                                 or self.body_style_names):
            body_styles: list = ['body_styles']
            if self.has_demorgan_body_styles:
                body_styles.append('demorgan')
            for n in self.body_style_names:
                body_styles.append(QuotedString(n))
            result.append(body_styles)

        # Pin settings
        if self.pin_numbers_hide:
            result.append(['pin_numbers', ['hide', 'yes']])

        pin_names: list = ['pin_names']
        if self.pin_names_offset != KICAD_DEFAULT_PIN_NAME_OFFSET_MM:
            pin_names.append(['offset', self.pin_names_offset])
        if self.pin_names_hide:
            pin_names.append(['hide', 'yes'])
        if len(pin_names) > 1:
            result.append(pin_names)

        # Flags
        result.append(['in_bom', 'yes' if self.in_bom else 'no'])
        result.append(['on_board', 'yes' if self.on_board else 'no'])

        if self.exclude_from_sim:
            result.append(['exclude_from_sim', 'yes'])

        # KiCad 10 emits `(power global)` / `(power local)` explicitly
        # (`sch_io_kicad_sexpr_lib_cache.cpp:401`). Default to `global`
        # when only `power=True` was set, matching KiCad's default for
        # the legacy `(power)` empty form.
        if self.power:
            kind = self.power_kind if self.power_kind else 'global'
            result.append(['power', kind])

        if self.duplicate_pin_numbers_are_jumpers:
            result.append(['duplicate_pin_numbers_are_jumpers', 'yes'])

        if self.jumper_pin_groups:
            result.append(
                ['jumper_pin_groups']
                + [[QuotedString(pin) for pin in group] for group in self.jumper_pin_groups]
            )

        # Properties
        for prop in self.properties:
            result.append(prop.to_sexp())

        # Sub-symbols
        for subsym in self.subsymbols:
            result.append(subsym.to_sexp())

        return result

    @public_api
    def get_property(self, key: str | StandardPropertyKey) -> 'SymProperty | None':
        """Get property by key name."""
        key_text = str(key)
        for prop in self.properties:
            if prop.key == key_text:
                return prop
        return None

    @public_api
    def get_property_value(self, key: str | StandardPropertyKey, default: str = "") -> str:
        """Get property value by key name."""
        prop = self.get_property(key)
        return prop.value if prop else default

    @public_api
    def set_property_value(
        self,
        key: str | StandardPropertyKey,
        value: str,
        *,
        create: bool = False,
    ) -> bool:
        """Set property value. Returns True if a property was updated or created."""
        prop = self.get_property(key)
        if prop is not None:
            prop.value = value
            return True
        if create:
            self.upsert_property(key, value)
            return True
        return False

    @public_api
    def upsert_property(
        self,
        key: str | StandardPropertyKey,
        value: str,
        *,
        property_id: int | PropertyId | None = None,
    ) -> 'SymProperty':
        """Create or update a property and return the property object."""
        from .kicad_sym_property import SymProperty

        prop = self.get_property(key)
        if prop is not None:
            prop.value = value
            return prop
        key_text = str(key)
        if property_id is None:
            property_id = standard_property_id_for_key(key_text)
        if property_id is None:
            property_id = _next_user_property_id(self.properties)
        prop = SymProperty(key_text, value, id=int(property_id))
        self.properties.append(prop)
        return prop

    @public_api
    def remove_property(self, key: str | StandardPropertyKey) -> bool:
        """Remove a property by key name."""
        key_text = str(key)
        for index, prop in enumerate(self.properties):
            if prop.key == key_text:
                del self.properties[index]
                return True
        return False

    @public_api
    def iter_properties(self) -> Iterator['SymProperty']:
        """Iterate over symbol properties."""
        return iter(self.properties)

    @property
    def reference(self) -> str:
        """Get reference designator prefix (e.g., 'U', 'R')."""
        return self.get_property_value(StandardPropertyKey.REFERENCE, "U")

    @property
    def value(self) -> str:
        """Get value property."""
        return self.get_property_value(StandardPropertyKey.VALUE, self.name)

    @property
    def footprint(self) -> str:
        """Get footprint property."""
        return self.get_property_value(StandardPropertyKey.FOOTPRINT, "")

    @property
    def datasheet(self) -> str:
        """Get datasheet property."""
        return self.get_property_value(StandardPropertyKey.DATASHEET, "")

    @property
    def description(self) -> str:
        """Get description property."""
        return self.get_property_value(StandardPropertyKey.DESCRIPTION, "")

    @property
    def unit_count(self) -> int:
        """Get the number of units in this symbol."""
        if not self.subsymbols:
            return 1
        max_unit = 0
        for sub in self.subsymbols:
            if sub.unit > max_unit:
                max_unit = sub.unit
        return max(1, max_unit)

    @property
    def has_demorgan(self) -> bool:
        """Check if this symbol has De Morgan alternate representation."""
        return any(sub.style == 1 for sub in self.subsymbols)

    def get_subsymbol(self, unit: int = 1, style: int = 0) -> 'LibSubSymbol | None':
        """Get subsymbol for specific unit and style."""
        for sub in self.subsymbols:
            if sub.unit == unit and sub.style == style:
                return sub
        return None

    def get_all_pins(self) -> List:
        """Get all pins from all subsymbols."""
        pins = []
        for sub in self.subsymbols:
            pins.extend(sub.pins)
        return pins

    @public_api
    def to_ir(
        self,
        *,
        unit: int | None = None,
        part_id: int | None = None,
        style: int = 0,
        source_path: str | None = None,
        document_id: str | None = None,
        **kwargs,
    ):
        """Render this symbol definition to plotter IR."""
        from .kicad_lib_symbol_to_ir import lib_symbol_to_ir

        selected_unit = _resolve_unit_alias(unit=unit, part_id=part_id)
        _validate_unit(self, selected_unit)
        return lib_symbol_to_ir(
            self,
            unit=selected_unit,
            style=style,
            source_path=source_path,
            document_id=document_id or self.name,
            **kwargs,
        )

    def get_bounds(self) -> 'BoundingBox':
        """Get bounding box of all graphical elements."""
        from .kicad_geometry import BoundingBox
        bbox = BoundingBox()
        for subsym in self.subsymbols:
            bbox.merge(subsym.get_bounds())
        return bbox

    def to_svg(self, ctx: 'SvgRenderContext | None' = None, unit: int = 1, style: int = 0) -> List[str]:
        """Render symbol to SVG elements.

        Args:
            ctx: SVG render context
            unit: Unit number to render (1-based)
            style: Style to render (0=normal, 1=De Morgan)

        Returns:
            List of SVG element strings
        """
        from .kicad_geometry import SvgRenderContext
        if ctx is None:
            ctx = SvgRenderContext()

        lines = []

        # Render common elements (unit 0) and the specified unit
        for subsym in self.subsymbols:
            if subsym.style == style and (subsym.unit == 0 or subsym.unit == unit):
                lines.extend(subsym.to_svg(ctx))

        return lines


def _next_user_property_id(properties: list['SymProperty']) -> int:
    max_id = max((int(prop.id) for prop in properties), default=int(PropertyId.USER_START) - 1)
    return max(max_id + 1, int(PropertyId.USER_START))


def _resolve_unit_alias(*, unit: int | None, part_id: int | None) -> int | None:
    if unit is not None and part_id is not None and int(unit) != int(part_id):
        raise ValueError(f"unit ({unit}) and part_id ({part_id}) disagree")
    selected = unit if unit is not None else part_id
    if selected is None:
        return None
    selected_int = int(selected)
    if selected_int < 1:
        raise ValueError("unit/part_id must be >= 1")
    return selected_int


def _validate_unit(symbol: LibSymbol, unit: int | None) -> None:
    if unit is None:
        return
    if unit > symbol.unit_count:
        raise ValueError(
            f"unit {unit} exceeds symbol unit_count {symbol.unit_count} "
            f"for '{symbol.name}'"
        )


__all__ = ['LibSymbol']

"""
KiCad PCB Other - Miscellaneous PCB elements

Layer, Net, Dimension, Image, Table, Group, TitleBlock, Stackup, etc.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import math
from typing import Any, Dict, List, Optional, Tuple, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_footprint import KiCadFootprint
    from .kicad_geometry import TextParams

from .kicad_sexpr import QuotedString, SexpList
from .kicad_base import (
    FRONT_COPPER_LAYER,
    FRONT_SILKSCREEN_LAYER,
    LayerType,
    StackupItemType,
    EdgeConnectorConstraint,
    MIME_BASE64_LENGTH,
    find_element,
    find_all_elements,
    get_value,
    has_flag,
    unquote_string,
)
from .kicad_primitives import Stroke, Effects, RenderCache
from .kicad_pcb_gr_text import GrText
from .kicad_pcb_graphics import GrTextBox


def _parse_yes_no_bool(sexp: Optional[list], default: bool = False) -> bool:
    """Parse a KiCad yes/no bool element, tolerating absent values."""
    if not sexp:
        return default
    if len(sexp) == 1:
        return True
    return unquote_string(sexp[1]).lower() in ("yes", "true", "1")


@dataclass
class Layer:
    """Layer definition."""
    ordinal: int
    canonical_name: str
    layer_type: LayerType
    user_name: Optional[str] = None

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Layer':
        ordinal = int(sexp[0])
        canonical_name = unquote_string(sexp[1])
        layer_type = LayerType(sexp[2]) if len(sexp) > 2 else LayerType.USER
        user_name = unquote_string(sexp[3]) if len(sexp) > 3 else None
        return cls(ordinal=ordinal, canonical_name=canonical_name,
                   layer_type=layer_type, user_name=user_name)

    def to_sexp(self) -> list:
        result: SexpList = [self.ordinal, QuotedString(self.canonical_name), self.layer_type.value]
        if self.user_name:
            result.append(QuotedString(self.user_name))
        return result


@dataclass
class Net:
    """Net definition."""
    ordinal: int
    name: str

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Net':
        return cls(ordinal=int(sexp[1]), name=unquote_string(sexp[2]))

    def to_sexp(self) -> list:
        return ['net', self.ordinal, QuotedString(self.name)]


@dataclass(frozen=True)
class NetTable:
    """A board's net table, arranged for repeated lookup.

    Resolving a single net reference against the board is cheap. Resolving
    every one of them is not: a board's pads, tracks, arcs, vias and zones all
    carry a reference, and rebuilding this mapping for each of them turns a
    whole-board pass into O(elements x nets). On a 1,388-net design that was
    126 million attribute reads to project 30,000 elements.

    This is a snapshot, deliberately rather than a cache hidden on the board:
    a board is mutable, and a cache that silently outlived a net rename would
    trade a speed problem for a correctness one. Build one when about to
    resolve in bulk, and build a fresh one after changing the net table.
    """

    name_by_ordinal: Dict[int, str]
    ordinal_by_name: Dict[str, int]

    @classmethod
    def from_name_by_ordinal(cls, name_by_ordinal: Dict[int, str]) -> 'NetTable':
        return cls(
            name_by_ordinal=name_by_ordinal,
            ordinal_by_name={
                name: ordinal
                for ordinal, name in name_by_ordinal.items()
                if name
            },
        )

    def name_of(self, net_ref: Optional['NetRef']) -> str:
        """The resolved net name for a reference, or an empty string."""
        if net_ref is None:
            return ""
        return str(net_ref.resolve_against(self).name or "")


@dataclass(frozen=True)
class NetRef:
    """Reference to a net as carried on KiCad board elements."""

    ordinal: Optional[int] = None
    name: str = ""

    @classmethod
    def from_pad_sexp(cls, net_elem: Optional[list]) -> 'NetRef':
        if not net_elem or len(net_elem) <= 1:
            return cls()
        if len(net_elem) > 2:
            return cls(ordinal=int(net_elem[1]), name=unquote_string(net_elem[2]))
        raw_value = net_elem[1]
        try:
            return cls(ordinal=int(raw_value))
        except (TypeError, ValueError):
            return cls(name=unquote_string(raw_value))

    @classmethod
    def from_raw_token(cls, raw_token: Any, explicit_name: str = "") -> 'NetRef':
        try:
            return cls(ordinal=int(raw_token), name=str(explicit_name or ""))
        except (TypeError, ValueError):
            name = str(explicit_name or "") or unquote_string(raw_token)
            return cls(name=name)

    def __bool__(self) -> bool:
        return self.ordinal is not None or bool(str(self.name or "").strip())

    def with_ordinal(self, ordinal: Optional[int]) -> 'NetRef':
        return NetRef(ordinal=ordinal, name=self.name)

    def with_name(self, name: str) -> 'NetRef':
        return NetRef(ordinal=self.ordinal, name=str(name or ""))

    def resolve_ordinal(self, ordinal_by_name: Dict[str, int]) -> 'NetRef':
        if self.ordinal is not None or not self.name:
            return self
        ordinal = ordinal_by_name.get(self.name)
        if ordinal is None:
            return self
        return self.with_ordinal(int(ordinal))

    def resolve_name(self, name_by_ordinal: Dict[int, str]) -> 'NetRef':
        if self.name or self.ordinal is None:
            return self
        name = name_by_ordinal.get(int(self.ordinal), "")
        if not name:
            return self
        return self.with_name(str(name))

    def to_pad_sexp(self) -> Optional[list]:
        if not self:
            return None
        if self.ordinal is not None and self.name:
            return ['net', int(self.ordinal), QuotedString(self.name)]
        if self.ordinal is not None:
            return ['net', int(self.ordinal)]
        return ['net', QuotedString(self.name)]

    def resolve_against(self, table: 'NetTable') -> 'NetRef':
        """Fill in whichever of name/ordinal this reference is missing."""
        return self.resolve_name(table.name_by_ordinal).resolve_ordinal(
            table.ordinal_by_name
        )

    def to_inline_net_sexp(self) -> Optional[list]:
        if not self:
            return None
        if self.ordinal is not None:
            return ['net', int(self.ordinal)]
        return ['net', QuotedString(self.name)]


@dataclass
class OutlineCarrier:
    """Board-outline carrier item exposed by the KiCad OOP model."""

    owner_kind: str
    owner_ref: str
    item: Any


@dataclass(frozen=True)
class DrillLayerSpan:
    """Layer span attached to backdrill / tertiary drill definitions."""

    start: str = ""
    end: str = ""

    @classmethod
    def from_layers_sexp(cls, layers_elem: Optional[list]) -> "DrillLayerSpan":
        if not layers_elem or len(layers_elem) < 3:
            return cls()
        return cls(
            start=unquote_string(layers_elem[1]),
            end=unquote_string(layers_elem[2]),
        )

    def __bool__(self) -> bool:
        return bool(self.start or self.end)

    def to_layers_sexp(self) -> Optional[list]:
        if not self:
            return None
        return ["layers", QuotedString(self.start), QuotedString(self.end)]


@dataclass(frozen=True)
class DrillProps:
    """Typed KiCad pad/via drill modifier."""

    size: Optional[float] = None
    layers: DrillLayerSpan = field(default_factory=DrillLayerSpan)

    @classmethod
    def from_sexp(cls, sexp: Optional[list]) -> "DrillProps":
        if not sexp:
            return cls()

        size: Optional[float] = None
        layers = DrillLayerSpan()

        for child in sexp[1:]:
            if not isinstance(child, list) or not child:
                continue
            key = str(child[0])
            if key == "size" and len(child) > 1:
                size = float(child[1])
            elif key == "layers":
                layers = DrillLayerSpan.from_layers_sexp(child)

        return cls(size=size, layers=layers)

    def __bool__(self) -> bool:
        return self.size is not None or bool(self.layers)

    def to_sexp(self, tag_name: str) -> Optional[list]:
        if not self:
            return None
        result: SexpList = [tag_name]
        if self.size is not None:
            result.append(["size", self.size])
        layers_elem = self.layers.to_layers_sexp()
        if layers_elem:
            result.append(layers_elem)
        return result


@dataclass(frozen=True)
class PostMachiningProps:
    """Typed KiCad pad/via post-machining definition."""

    mode: str = ""
    size: Optional[float] = None
    depth: Optional[float] = None
    angle: Optional[float] = None

    @classmethod
    def from_sexp(cls, sexp: Optional[list]) -> "PostMachiningProps":
        if not sexp or len(sexp) < 2:
            return cls()

        mode = unquote_string(sexp[1])
        size: Optional[float] = None
        depth: Optional[float] = None
        angle: Optional[float] = None

        for child in sexp[2:]:
            if not isinstance(child, list) or not child:
                continue
            key = str(child[0])
            if key == "size" and len(child) > 1:
                size = float(child[1])
            elif key == "depth" and len(child) > 1:
                depth = float(child[1])
            elif key == "angle" and len(child) > 1:
                angle = float(child[1])

        return cls(mode=mode, size=size, depth=depth, angle=angle)

    def __bool__(self) -> bool:
        return bool(self.mode)

    def to_sexp(self, tag_name: str) -> Optional[list]:
        if not self:
            return None
        result: SexpList = [tag_name, self.mode]
        if self.size is not None:
            result.append(["size", self.size])
        if self.depth is not None:
            result.append(["depth", self.depth])
        if self.angle is not None:
            result.append(["angle", self.angle])
        return result


@dataclass(frozen=True)
class ZoneLayerConnections:
    """Explicit flashed-layer override set for PTH pads / vias."""

    forced_layers: Tuple[str, ...] = ()

    @classmethod
    def from_sexp(cls, sexp: Optional[list]) -> "ZoneLayerConnections":
        if not sexp:
            return cls()
        return cls(
            forced_layers=tuple(unquote_string(token) for token in sexp[1:]),
        )

    def to_sexp(self) -> list:
        return ["zone_layer_connections"] + [QuotedString(layer) for layer in self.forced_layers]


@dataclass(frozen=True)
class PadNameGroup:
    """Typed footprint pad-group carrier used for KiCad net-tie/jumper metadata.

    ``raw_token`` preserves the original ``net_tie_pad_groups`` string verbatim
    (KiCad upstream stores the user-entered string as a single ``wxString`` and
    re-emits it without rejoining; see ``pcb_io_kicad_sexpr.cpp:1391-1396``).
    Different fixtures use different separators (``","`` vs ``", "``); we keep
    the original on round-trip.
    """

    pad_names: Tuple[str, ...] = ()
    raw_token: Optional[str] = None

    @classmethod
    def from_net_tie_token(cls, token: Any) -> "PadNameGroup":
        raw = unquote_string(token)
        pad_names = tuple(part.strip() for part in raw.split(",") if part.strip())
        if pad_names:
            return cls(pad_names=pad_names, raw_token=raw)
        raw = raw.strip()
        return cls(pad_names=(raw,), raw_token=raw) if raw else cls()

    @classmethod
    def from_jumper_group_sexp(cls, sexp: list) -> "PadNameGroup":
        return cls(
            pad_names=tuple(unquote_string(token) for token in sexp if token is not None),
        )

    def __bool__(self) -> bool:
        return bool(self.pad_names)

    def to_net_tie_token(self) -> QuotedString:
        if self.raw_token is not None:
            return QuotedString(self.raw_token)
        return QuotedString(", ".join(self.pad_names))

    def to_jumper_group_sexp(self) -> list:
        return [QuotedString(pad_name) for pad_name in self.pad_names]


@dataclass(frozen=True)
class BarcodeMargins:
    """Explicit barcode knockout margins."""

    x: float = 0.0
    y: float = 0.0

    @classmethod
    def from_sexp(cls, sexp: Optional[list]) -> "BarcodeMargins":
        if not sexp or len(sexp) < 3:
            return cls()
        return cls(x=float(sexp[1]), y=float(sexp[2]))

    def __bool__(self) -> bool:
        return not (self.x == 0.0 and self.y == 0.0)

    def to_sexp(self) -> Optional[list]:
        if not self:
            return None
        return ["margins", self.x, self.y]


@dataclass
class Barcode:
    """KiCad PCB or footprint barcode element."""

    at_x: float = 0.0
    at_y: float = 0.0
    at_angle: float = 0.0
    layer: str = FRONT_SILKSCREEN_LAYER
    width: float = 0.0
    height: float = 0.0
    text: str = ""
    text_height: float = 1.0
    barcode_type: str = "code39"
    ecc_level: Optional[str] = None
    locked: bool = False
    show_text: bool = True
    knockout: bool = False
    margins: BarcodeMargins = field(default_factory=BarcodeMargins)
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> "Barcode":
        at_elem = find_element(sexp, "at")
        size_elem = find_element(sexp, "size")
        return cls(
            at_x=float(at_elem[1]) if at_elem and len(at_elem) > 1 else 0.0,
            at_y=float(at_elem[2]) if at_elem and len(at_elem) > 2 else 0.0,
            at_angle=float(at_elem[3]) if at_elem and len(at_elem) > 3 else 0.0,
            layer=unquote_string(get_value(sexp, "layer", FRONT_SILKSCREEN_LAYER)),
            width=float(size_elem[1]) if size_elem and len(size_elem) > 1 else 0.0,
            height=float(size_elem[2]) if size_elem and len(size_elem) > 2 else 0.0,
            text=unquote_string(get_value(sexp, "text", "")),
            text_height=float(get_value(sexp, "text_height", 1.0)),
            barcode_type=str(get_value(sexp, "type", "code39")),
            ecc_level=unquote_string(get_value(sexp, "ecc_level")) or None,
            locked=_parse_yes_no_bool(find_element(sexp, "locked"), False),
            show_text=not _parse_yes_no_bool(find_element(sexp, "hide"), False),
            knockout=_parse_yes_no_bool(find_element(sexp, "knockout"), False),
            margins=BarcodeMargins.from_sexp(find_element(sexp, "margins")),
            uuid=unquote_string(get_value(sexp, "uuid")) or None,
            _raw_sexp=sexp,
        )

    def to_sexp(self) -> list:
        result = [
            "barcode",
            ["locked", "yes" if self.locked else "no"],
            ["at", self.at_x, self.at_y, self.at_angle],
            ["layer", QuotedString(self.layer)],
            ["size", self.width, self.height],
            ["text", QuotedString(self.text)],
            ["text_height", self.text_height],
            ["type", self.barcode_type],
            ["hide", "yes" if not self.show_text else "no"],
            ["knockout", "yes" if self.knockout else "no"],
        ]
        if self.ecc_level:
            result.append(["ecc_level", self.ecc_level])
        margins_elem = self.margins.to_sexp()
        if margins_elem:
            result.append(margins_elem)
        if self.uuid:
            result.append(["uuid", QuotedString(self.uuid)])
        return result


@dataclass
class BoardProperty:
    """
    Board-level custom property (key-value pair).

    These are user-defined metadata fields for the PCB.
    """
    key: str
    value: str

    @classmethod
    def from_sexp(cls, sexp: list) -> 'BoardProperty':
        key = unquote_string(sexp[1])
        value = unquote_string(sexp[2]) if len(sexp) > 2 else ""
        return cls(key=key, value=value)

    def to_sexp(self) -> list:
        return ['property', QuotedString(self.key), QuotedString(self.value)]


@dataclass(frozen=True)
class BoardVariant:
    """Board-level PCB variant registry entry."""

    name: str
    description: Optional[str] = None

    @classmethod
    def from_sexp(cls, sexp: list) -> "BoardVariant":
        return cls(
            name=unquote_string(get_value(sexp, "name", "")),
            description=unquote_string(get_value(sexp, "description")) or None,
        )

    def to_sexp(self) -> list:
        result = ["variant", ["name", QuotedString(self.name)]]
        if self.description:
            result.append(["description", QuotedString(self.description)])
        return result


@dataclass(frozen=True)
class FootprintVariantField:
    """Per-variant footprint field override."""

    name: str
    value: str

    @classmethod
    def from_sexp(cls, sexp: list) -> "FootprintVariantField":
        return cls(
            name=unquote_string(get_value(sexp, "name", "")),
            value=unquote_string(get_value(sexp, "value", "")),
        )

    def to_sexp(self) -> list:
        return [
            "field",
            ["name", QuotedString(self.name)],
            ["value", QuotedString(self.value)],
        ]


@dataclass
class FootprintVariant:
    """Per-footprint variant override block introduced in KiCad 10."""

    name: str
    dnp: Optional[bool] = None
    exclude_from_bom: Optional[bool] = None
    exclude_from_pos_files: Optional[bool] = None
    fields: List[FootprintVariantField] = field(default_factory=list)
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> "FootprintVariant":
        return cls(
            name=unquote_string(get_value(sexp, "name", "")),
            dnp=(
                _parse_yes_no_bool(find_element(sexp, "dnp"), False)
                if find_element(sexp, "dnp") is not None
                else None
            ),
            exclude_from_bom=(
                _parse_yes_no_bool(find_element(sexp, "exclude_from_bom"), False)
                if find_element(sexp, "exclude_from_bom") is not None
                else None
            ),
            exclude_from_pos_files=(
                _parse_yes_no_bool(find_element(sexp, "exclude_from_pos_files"), False)
                if find_element(sexp, "exclude_from_pos_files") is not None
                else None
            ),
            fields=[
                FootprintVariantField.from_sexp(field_elem)
                for field_elem in find_all_elements(sexp, "field")
            ],
            _raw_sexp=sexp,
        )

    def to_sexp(self) -> list:
        result = ["variant", ["name", QuotedString(self.name)]]
        if self.dnp is not None:
            result.append(["dnp", "yes" if self.dnp else "no"])
        if self.exclude_from_bom is not None:
            result.append(
                ["exclude_from_bom", "yes" if self.exclude_from_bom else "no"]
            )
        if self.exclude_from_pos_files is not None:
            result.append(
                [
                    "exclude_from_pos_files",
                    "yes" if self.exclude_from_pos_files else "no",
                ]
            )
        for field_override in self.fields:
            result.append(field_override.to_sexp())
        return result


@dataclass(frozen=True)
class FootprintPlacement:
    """Schematic-placement metadata attached to a board footprint."""

    path: str = ""
    sheetname: str = ""
    sheetfile: str = ""

    @classmethod
    def from_footprint_sexp(cls, sexp: list) -> "FootprintPlacement":
        return cls(
            path=unquote_string(get_value(sexp, "path")),
            sheetname=unquote_string(get_value(sexp, "sheetname")),
            sheetfile=unquote_string(get_value(sexp, "sheetfile")),
        )

    def __bool__(self) -> bool:
        return bool(self.path or self.sheetname or self.sheetfile)

    def to_sexp_elements(self) -> List[list]:
        result: List[list] = []
        if self.path:
            result.append(["path", QuotedString(self.path)])
        if self.sheetname:
            result.append(["sheetname", QuotedString(self.sheetname)])
        if self.sheetfile:
            result.append(["sheetfile", QuotedString(self.sheetfile)])
        return result


@dataclass(frozen=True)
class ComponentClassRef:
    """Static component-class membership carried on a footprint."""

    name: str = ""

    @classmethod
    def from_sexp(cls, sexp: list) -> "ComponentClassRef":
        if len(sexp) < 2:
            return cls()
        return cls(name=unquote_string(sexp[1]))

    def __bool__(self) -> bool:
        return bool(self.name)

    def to_sexp(self) -> list:
        return ["class", QuotedString(self.name)]


@dataclass
class GeneratedProperty:
    """Opaque generated-item property preserved as a typed child record."""

    name: str = ""
    raw_sexp: list = field(default_factory=list)

    @classmethod
    def from_sexp(cls, sexp: list) -> "GeneratedProperty":
        return cls(name=str(sexp[0]) if sexp else "", raw_sexp=sexp)

    def to_sexp(self) -> list:
        return self.raw_sexp


@dataclass
class GeneratedObject:
    """Board-level generated object such as a tuning pattern."""

    uuid: Optional[str] = None
    generator_type: str = ""
    name: str = ""
    layer: str = ""
    locked: bool = False
    properties: List[GeneratedProperty] = field(default_factory=list)
    members: List[str] = field(default_factory=list)
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> "GeneratedObject":
        # KiCad uses `(id <bare-uuid>)` for the generated-object identifier
        # — NOT `(uuid ...)` like most other board children.
        uuid = unquote_string(get_value(sexp, "id"))
        generator_type = unquote_string(get_value(sexp, "type"))
        name = unquote_string(get_value(sexp, "name"))
        layer = unquote_string(get_value(sexp, "layer"))
        locked = has_flag(sexp, "locked") or _parse_yes_no_bool(find_element(sexp, "locked"), False)

        members_elem = find_element(sexp, "members")
        members = [unquote_string(token) for token in members_elem[1:]] if members_elem else []

        known_elements = {"id", "type", "name", "layer", "locked", "members"}
        properties = [
            GeneratedProperty.from_sexp(elem)
            for elem in sexp[1:]
            if isinstance(elem, list) and elem and elem[0] not in known_elements
        ]

        return cls(
            uuid=uuid,
            generator_type=generator_type,
            name=name,
            layer=layer,
            locked=locked,
            properties=properties,
            members=members,
            _raw_sexp=sexp,
        )

    def to_sexp(self) -> list:
        result: SexpList = ["generated"]
        # KiCad's parser requires `(id ...)` to be the FIRST child of
        # `(generated ...)`. Misordering causes a kicad-cli segfault
        # (rc 0xC0000005). Members are emitted as bare uuid tokens
        # (no quotes) — matches the KiCad canonical form.
        if self.uuid:
            result.append(["id", self.uuid])
        if self.generator_type:
            result.append(["type", self.generator_type])
        if self.name:
            result.append(["name", QuotedString(self.name)])
        if self.layer:
            result.append(["layer", QuotedString(self.layer)])
        if self.locked:
            result.append(["locked", "yes"])
        for prop in self.properties:
            result.append(prop.to_sexp())
        if self.members:
            result.append(["members"] + list(self.members))
        return result


# -----------------------------------------------------------------------------
# Board Stackup Classes (KiCad 9.0)
# -----------------------------------------------------------------------------

@dataclass
class StackupLayerSubLayer:
    """Dielectric sublayer parameters for complex stackups."""
    thickness: float = 0.0
    thickness_locked: bool = False
    material: Optional[str] = None
    epsilon_r: Optional[float] = None
    loss_tangent: Optional[float] = None
    color: Optional[str] = None


@dataclass
class StackupLayer:
    """A single layer in the board stackup."""
    name: str = ""
    type_name: str = ""
    thickness: float = 0.0
    thickness_locked: bool = False
    material: Optional[str] = None
    epsilon_r: Optional[float] = None
    loss_tangent: Optional[float] = None
    color: Optional[str] = None
    sublayers: List[StackupLayerSubLayer] = field(default_factory=list)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'StackupLayer':
        name = unquote_string(sexp[1])
        type_name = unquote_string(get_value(sexp, 'type', ''))

        color = unquote_string(get_value(sexp, 'color'))
        material = unquote_string(get_value(sexp, 'material'))
        epsilon_r = get_value(sexp, 'epsilon_r')
        loss_tangent = get_value(sexp, 'loss_tangent')

        thickness = 0.0
        thickness_locked = False
        thickness_elem = find_element(sexp, 'thickness')
        if thickness_elem:
            thickness = float(thickness_elem[1])
            thickness_locked = 'locked' in thickness_elem

        return cls(
            name=name,
            type_name=type_name,
            thickness=thickness,
            thickness_locked=thickness_locked,
            material=material,
            epsilon_r=float(epsilon_r) if epsilon_r else None,
            loss_tangent=float(loss_tangent) if loss_tangent else None,
            color=color,
        )

    def to_sexp(self) -> list:
        result = ['layer', QuotedString(self.name), ['type', QuotedString(self.type_name)]]

        if self.color:
            result.append(['color', QuotedString(self.color)])

        if self.thickness > 0:
            thickness_elem = ['thickness', self.thickness]
            if self.thickness_locked:
                thickness_elem.append('locked')
            result.append(thickness_elem)

        if self.material:
            result.append(['material', QuotedString(self.material)])

        if self.epsilon_r is not None:
            result.append(['epsilon_r', self.epsilon_r])

        if self.loss_tangent is not None:
            result.append(['loss_tangent', self.loss_tangent])

        return result

    def get_item_type(self) -> StackupItemType:
        tn = self.type_name.lower()
        if tn == 'copper':
            return StackupItemType.COPPER
        elif tn in ('core', 'prepreg'):
            return StackupItemType.DIELECTRIC
        elif 'solder mask' in tn or tn == 'soldermask':
            return StackupItemType.SOLDERMASK
        elif 'silk' in tn or tn == 'silkscreen':
            return StackupItemType.SILKSCREEN
        elif 'paste' in tn or tn == 'solderpaste':
            return StackupItemType.SOLDERPASTE
        return StackupItemType.UNDEFINED


@dataclass
class Stackup:
    """Complete board stackup definition."""
    layers: List[StackupLayer] = field(default_factory=list)
    copper_finish: Optional[str] = None
    dielectric_constraints: bool = False
    edge_connector: EdgeConnectorConstraint = EdgeConnectorConstraint.NONE
    edge_plating: bool = False

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Stackup':
        layers = []
        for elem in sexp:
            if isinstance(elem, list) and len(elem) > 0:
                if elem[0] == 'layer':
                    layers.append(StackupLayer.from_sexp(elem))

        copper_finish = unquote_string(get_value(sexp, 'copper_finish'))

        dielectric_constraints = False
        dc_elem = find_element(sexp, 'dielectric_constraints')
        if dc_elem and len(dc_elem) > 1:
            dielectric_constraints = dc_elem[1] in ('yes', True)

        edge_connector = EdgeConnectorConstraint.NONE
        ec_elem = find_element(sexp, 'edge_connector')
        if ec_elem and len(ec_elem) > 1:
            ec_val = unquote_string(ec_elem[1])
            if ec_val == 'bevelled':
                edge_connector = EdgeConnectorConstraint.BEVELLED
            elif ec_val == 'yes':
                edge_connector = EdgeConnectorConstraint.IN_USE

        edge_plating = False
        ep_elem = find_element(sexp, 'edge_plating')
        if ep_elem and len(ep_elem) > 1:
            edge_plating = ep_elem[1] in ('yes', True)

        return cls(
            layers=layers,
            copper_finish=copper_finish,
            dielectric_constraints=dielectric_constraints,
            edge_connector=edge_connector,
            edge_plating=edge_plating
        )

    def to_sexp(self) -> list:
        result: SexpList = ['stackup']

        for layer in self.layers:
            result.append(layer.to_sexp())

        if self.copper_finish:
            result.append(['copper_finish', QuotedString(self.copper_finish)])

        result.append(['dielectric_constraints', 'yes' if self.dielectric_constraints else 'no'])

        if self.edge_connector != EdgeConnectorConstraint.NONE:
            result.append(['edge_connector', self.edge_connector.value])

        if self.edge_plating:
            result.append(['edge_plating', 'yes'])

        return result

    def get_board_thickness(self) -> float:
        total = 0.0
        for layer in self.layers:
            item_type = layer.get_item_type()
            if item_type in (StackupItemType.COPPER, StackupItemType.DIELECTRIC,
                           StackupItemType.SOLDERMASK):
                total += layer.thickness
                for sub in layer.sublayers:
                    total += sub.thickness
        return total

    def get_copper_layers(self) -> List[StackupLayer]:
        return [layer for layer in self.layers if layer.get_item_type() == StackupItemType.COPPER]

    def get_dielectric_layers(self) -> List[StackupLayer]:
        return [layer for layer in self.layers if layer.get_item_type() == StackupItemType.DIELECTRIC]


# -----------------------------------------------------------------------------
# Dimension
# -----------------------------------------------------------------------------

@dataclass
class DimensionFormat:
    """Format settings for dimension annotations."""
    prefix: str = ""
    suffix: str = ""
    units: int = 2
    units_format: int = 1
    precision: int = 4
    override_value: Optional[str] = None
    suppress_zeroes: bool = False

    @classmethod
    def from_sexp(cls, sexp: list) -> 'DimensionFormat':
        format_elem = find_element(sexp, 'format')
        if not format_elem:
            return cls()

        return cls(
            prefix=unquote_string(get_value(format_elem, 'prefix', '')),
            suffix=unquote_string(get_value(format_elem, 'suffix', '')),
            units=int(get_value(format_elem, 'units', 2)),
            units_format=int(get_value(format_elem, 'units_format', 1)),
            precision=int(get_value(format_elem, 'precision', 4)),
            override_value=unquote_string(get_value(format_elem, 'override_value')) if get_value(format_elem, 'override_value') else None,
            suppress_zeroes=_parse_yes_no_bool(find_element(format_elem, 'suppress_zeroes'), False),
        )

    def to_sexp(self) -> list:
        result = ['format',
                  ['prefix', QuotedString(self.prefix)],
                  ['suffix', QuotedString(self.suffix)],
                  ['units', self.units],
                  ['units_format', self.units_format],
                  ['precision', self.precision]]
        if self.override_value:
            result.append(['override_value', QuotedString(self.override_value)])
        if self.suppress_zeroes:
            result.append(['suppress_zeroes', 'yes'])
        return result


@dataclass
class DimensionStyle:
    """Style settings for dimension annotations."""
    thickness: float = 0.2
    arrow_length: float = 1.27
    text_position_mode: int = 0
    arrow_direction: str = "outward"
    extension_height: float = 0.58642
    extension_offset: float = 0.0
    keep_text_aligned: bool = True
    text_frame: Optional[int] = None

    @classmethod
    def from_sexp(cls, sexp: list) -> 'DimensionStyle':
        style_elem = find_element(sexp, 'style')
        if not style_elem:
            return cls()

        return cls(
            thickness=float(get_value(style_elem, 'thickness', 0.2)),
            arrow_length=float(get_value(style_elem, 'arrow_length', 1.27)),
            text_position_mode=int(get_value(style_elem, 'text_position_mode', 0)),
            arrow_direction=str(get_value(style_elem, 'arrow_direction', 'outward')),
            extension_height=float(get_value(style_elem, 'extension_height', 0.58642)),
            extension_offset=float(get_value(style_elem, 'extension_offset', 0.0)),
            keep_text_aligned=_parse_yes_no_bool(find_element(style_elem, 'keep_text_aligned'), False),
            text_frame=int(get_value(style_elem, 'text_frame')) if get_value(style_elem, 'text_frame') is not None else None,
        )

    def to_sexp(self, dimension_type: str = 'aligned') -> list:
        """Serialize style. Per ``PCB_IO_KICAD_SEXPR::format`` (pcbnew/pcb_io/
        kicad_sexpr/pcb_io_kicad_sexpr.cpp:947-981), the emitted child tokens
        depend on the dimension type:

        * ``arrow_direction`` — only for ``aligned``/``orthogonal``
          (parser asserts otherwise via ``case T_arrow_direction`` /
          dimension type checks).
        * ``extension_height`` — only for ``aligned``/``orthogonal``
          (``T_extension_height`` parser does ``dynamic_cast<PCB_DIM_ALIGNED*>``
          and ``wxCHECK_MSG(aligned, ...)`` — emitting it on a leader/radial/
          center dimension causes kicad-cli to refuse the file).
        * ``text_frame`` — only for ``leader``.
        * ``extension_offset`` — always emitted.
        """
        result: list = ['style',
                        ['thickness', self.thickness],
                        ['arrow_length', self.arrow_length],
                        ['text_position_mode', self.text_position_mode]]
        if dimension_type in ('aligned', 'orthogonal'):
            result.append(['arrow_direction', self.arrow_direction])
            result.append(['extension_height', self.extension_height])
        if dimension_type == 'leader' and self.text_frame is not None:
            result.append(['text_frame', self.text_frame])
        result.append(['extension_offset', self.extension_offset])
        if self.keep_text_aligned:
            result.append(['keep_text_aligned', 'yes'])
        return result


@dataclass
class Dimension:
    """Dimension annotation element."""
    dimension_type: str = "aligned"
    layer: str = "Cmts.User"
    uuid: Optional[str] = None
    locked: bool = False
    points: List[Tuple[float, float]] = field(default_factory=list)
    height: float = 0.0
    leader_length: Optional[float] = None
    orientation: Optional[int] = None
    format: DimensionFormat = field(default_factory=DimensionFormat)
    style: DimensionStyle = field(default_factory=DimensionStyle)
    gr_text: Optional[GrText] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @staticmethod
    def _resize_vector(vector: Tuple[float, float], length: float) -> Tuple[float, float]:
        x, y = vector
        norm = math.hypot(x, y)
        if norm == 0.0:
            return (0.0, 0.0)
        scale = length / norm
        return (x * scale, y * scale)

    @staticmethod
    def _rotate_vector(vector: Tuple[float, float], angle: float) -> Tuple[float, float]:
        x, y = vector
        rad = math.radians(angle)
        sin_a = math.sin(rad)
        cos_a = math.cos(rad)
        return (y * sin_a + x * cos_a, y * cos_a - x * sin_a)

    @staticmethod
    def _vector_angle(vector: Tuple[float, float]) -> float:
        x, y = vector
        if x == 0.0 and y == 0.0:
            return 0.0
        return math.degrees(math.atan2(y, x))

    @staticmethod
    def _normalize_angle(angle: float) -> float:
        angle = angle % 360.0
        if math.isclose(angle, 360.0, abs_tol=1e-9):
            return 0.0
        return angle

    @staticmethod
    def _format_dimension_value(value_mm: float, fmt: DimensionFormat) -> str:
        units = fmt.units
        value = value_mm
        if units == 0:
            value = value_mm / 25.4
        elif units == 1:
            value = value_mm / 0.0254

        precision = int(fmt.precision)
        if precision >= 6:
            if units == 1:
                precision = max(0, precision - 7)
            elif units == 2:
                precision -= 5
            else:
                precision -= 4

        text = f"{value:.{max(0, precision)}f}"
        if fmt.suppress_zeroes:
            text = text.rstrip("0").rstrip(".")

        if fmt.override_value:
            text = fmt.override_value

        unit_text = {0: " in", 1: " mils", 2: " mm", 3: " mm"}.get(units, " mm")
        if fmt.units_format == 1:
            text += unit_text
        elif fmt.units_format == 2:
            text += f" ({unit_text.lstrip()})"

        return f"{fmt.prefix}{text}{fmt.suffix}"

    def _aligned_crossbar(self) -> Optional[Tuple[Tuple[float, float], Tuple[float, float]]]:
        if self.dimension_type != "aligned" or len(self.points) < 2:
            return None

        start = self.points[0]
        end = self.points[1]
        dimension = (end[0] - start[0], end[1] - start[1])
        if self.height > 0.0:
            extension = (-dimension[1], dimension[0])
        else:
            extension = (dimension[1], -dimension[0])

        crossbar_distance = self._resize_vector(extension, abs(self.height))
        crossbar_start = (
            start[0] + crossbar_distance[0],
            start[1] + crossbar_distance[1],
        )
        crossbar_end = (
            end[0] + crossbar_distance[0],
            end[1] + crossbar_distance[1],
        )
        return crossbar_start, crossbar_end

    def _orthogonal_crossbar(self) -> Optional[Tuple[Tuple[float, float], Tuple[float, float]]]:
        if self.dimension_type != "orthogonal" or len(self.points) < 2:
            return None

        start = self.points[0]
        end = self.points[1]
        if self.orientation == 1:
            crossbar_start = (start[0] + self.height, start[1])
            crossbar_end = (crossbar_start[0], end[1])
        else:
            crossbar_start = (start[0], start[1] + self.height)
            crossbar_end = (end[0], crossbar_start[1])
        return crossbar_start, crossbar_end

    def _dimension_crossbar(self) -> Optional[Tuple[Tuple[float, float], Tuple[float, float]]]:
        if self.dimension_type == "orthogonal":
            return self._orthogonal_crossbar()
        return self._aligned_crossbar()

    def _radial_knee(self) -> Optional[Tuple[float, float]]:
        if self.dimension_type != "radial" or len(self.points) < 2:
            return None
        if self.leader_length is None:
            return None

        start = self.points[0]
        end = self.points[1]
        radial = (end[0] - start[0], end[1] - start[1])
        leader = self._resize_vector(radial, self.leader_length)
        return (end[0] + leader[0], end[1] + leader[1])

    def _radial_text_angle(self, text_object: GrText) -> Optional[float]:
        if not self.style.keep_text_aligned:
            return None

        knee = self._radial_knee()
        if knee is None:
            return None

        text_line = (text_object.at_x - knee[0], text_object.at_y - knee[1])
        angle = self._normalize_angle(360.0 - self._vector_angle(text_line))
        if angle > 90.0 and angle <= 270.0:
            angle -= 180.0
        return float(math.floor(angle + 0.5))

    def resolved_gr_text(self) -> Optional[GrText]:
        """Return a KiCad-updated dimension text object for cache generation."""

        if self.gr_text is None:
            return None

        text_object = self.gr_text
        text = self._format_dimension_value(self.measured_value_mm(), self.format)
        at_x = text_object.at_x
        at_y = text_object.at_y
        at_angle = text_object.at_angle

        crossbar = self._dimension_crossbar()
        if crossbar is not None:
            crossbar_start, crossbar_end = crossbar
            crossbar_center = (
                (crossbar_end[0] - crossbar_start[0]) / 2.0,
                (crossbar_end[1] - crossbar_start[1]) / 2.0,
            )

            if self.style.text_position_mode == 0:
                text_offset_distance = (
                    text_object.font.effective_thickness + text_object.font.size_y
                )
                if math.isclose(crossbar_center[0], 0.0, abs_tol=1e-12):
                    rotation = 90.0 * (-1.0 if crossbar_center[1] > 0.0 else 1.0)
                elif crossbar_center[0] < 0.0:
                    rotation = -90.0
                else:
                    rotation = 90.0

                offset = self._rotate_vector(crossbar_center, rotation)
                offset = self._resize_vector(offset, text_offset_distance)
                text_offset = (
                    crossbar_center[0] + offset[0],
                    crossbar_center[1] + offset[1],
                )
                at_x = crossbar_start[0] + text_offset[0]
                at_y = crossbar_start[1] + text_offset[1]
            elif self.style.text_position_mode == 1:
                at_x = crossbar_start[0] + crossbar_center[0]
                at_y = crossbar_start[1] + crossbar_center[1]

            if self.style.keep_text_aligned:
                at_angle = self._normalize_angle(360.0 - self._vector_angle(crossbar_center))
                if at_angle > 90.0 and at_angle <= 270.0:
                    at_angle -= 180.0
        elif self.dimension_type == "radial":
            radial_angle = self._radial_text_angle(text_object)
            if radial_angle is not None:
                at_angle = radial_angle

        return GrText(
            text=text,
            at_x=at_x,
            at_y=at_y,
            at_angle=at_angle,
            layer=text_object.layer or self.layer,
            knockout=text_object.knockout,
            uuid=text_object.uuid or self.uuid,
            effects=text_object.effects,
            render_cache=text_object.render_cache,
        )

    def measured_value_mm(self) -> float:
        if len(self.points) < 2:
            return 0.0
        start = self.points[0]
        end = self.points[1]
        if self.dimension_type == "aligned":
            return math.hypot(end[0] - start[0], end[1] - start[1])
        if self.dimension_type == "orthogonal":
            if self.orientation == 1:
                return abs(end[1] - start[1])
            return abs(end[0] - start[0])
        return math.hypot(end[0] - start[0], end[1] - start[1])

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Dimension':
        dimension_type = str(get_value(sexp, 'type', 'aligned'))
        layer = unquote_string(get_value(sexp, 'layer', 'Cmts.User'))
        uuid = unquote_string(get_value(sexp, 'uuid'))
        height = float(get_value(sexp, 'height', 0.0))
        leader_length = get_value(sexp, 'leader_length')
        orientation = get_value(sexp, 'orientation')

        pts_elem = find_element(sexp, 'pts')
        points = []
        if pts_elem:
            for xy in find_all_elements(pts_elem, 'xy'):
                if len(xy) >= 3:
                    points.append((float(xy[1]), float(xy[2])))

        format_obj = DimensionFormat.from_sexp(sexp)
        style_obj = DimensionStyle.from_sexp(sexp)

        # Parse the gr_text inside the dimension
        gr_text_elem = find_element(sexp, 'gr_text')
        gr_text = GrText.from_sexp(gr_text_elem) if gr_text_elem else None

        return cls(
            dimension_type=dimension_type,
            layer=layer,
            uuid=uuid,
            locked=_parse_yes_no_bool(find_element(sexp, 'locked'), False),
            points=points,
            height=height,
            leader_length=float(leader_length) if leader_length is not None else None,
            orientation=int(orientation) if orientation is not None else None,
            format=format_obj,
            style=style_obj,
            gr_text=gr_text,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result = ['dimension', ['type', self.dimension_type]]
        if self.locked:
            result.append(['locked', 'yes'])
        result.append(['layer', QuotedString(self.layer)])
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])

        if self.points:
            pts = ['pts'] + [['xy', p[0], p[1]] for p in self.points]
            result.append(pts)

        if self.dimension_type in ('aligned', 'orthogonal'):
            result.append(['height', self.height])
        if self.leader_length is not None:
            result.append(['leader_length', self.leader_length])
        if self.orientation is not None:
            result.append(['orientation', self.orientation])
        if self.dimension_type != 'center':
            result.append(self.format.to_sexp())
        result.append(self.style.to_sexp(self.dimension_type))

        if self.gr_text:
            result.append(self.gr_text.to_sexp())

        return result


# -----------------------------------------------------------------------------
# Image
# -----------------------------------------------------------------------------

@dataclass
class Image:
    """Embedded image element.

    Mirrors PCB_IO_KICAD_SEXPR::format(PCB_REFERENCE_IMAGE*) at
    pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp:1116. Emit order is
    (at), layer, [scale], [locked yes], data, uuid.
    """
    at_x: float = 0.0
    at_y: float = 0.0
    scale: float = 1.0
    layer: str = FRONT_SILKSCREEN_LAYER
    locked: bool = False
    data: str = ""  # Base64 encoded image data
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Image':
        at_elem = find_element(sexp, 'at')
        at_x = float(at_elem[1]) if at_elem and len(at_elem) > 1 else 0.0
        at_y = float(at_elem[2]) if at_elem and len(at_elem) > 2 else 0.0

        scale = float(get_value(sexp, 'scale', 1.0))
        layer = unquote_string(get_value(sexp, 'layer', FRONT_SILKSCREEN_LAYER))
        locked = _parse_yes_no_bool(find_element(sexp, 'locked'))

        # Data is stored as multiple quoted strings
        data_elem = find_element(sexp, 'data')
        data = ""
        if data_elem:
            data = "".join(unquote_string(s) for s in data_elem[1:])

        uuid = unquote_string(get_value(sexp, 'uuid'))

        return cls(
            at_x=at_x, at_y=at_y,
            scale=scale,
            layer=layer,
            locked=locked,
            data=data,
            uuid=uuid,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result = ['image', ['at', self.at_x, self.at_y]]
        result.append(['layer', QuotedString(self.layer)])
        if self.scale != 1.0:
            result.append(['scale', self.scale])
        if self.locked:
            result.append(['locked', 'yes'])
        # Split data into lines for proper formatting
        if self.data:
            data_lines = [self.data[i:i+MIME_BASE64_LENGTH]
                         for i in range(0, len(self.data), MIME_BASE64_LENGTH)]
            data_elem = ['data'] + [QuotedString(line) for line in data_lines]
            result.append(data_elem)
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result


# -----------------------------------------------------------------------------
# TitleBlock
# -----------------------------------------------------------------------------

@dataclass
class TitleBlock:
    """Title block for drawing sheet."""
    title: str = ""
    date: str = ""
    rev: str = ""
    company: str = ""
    comments: Dict[int, str] = field(default_factory=dict)  # comment number -> text
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'TitleBlock':
        title = unquote_string(get_value(sexp, 'title', ''))
        date = unquote_string(get_value(sexp, 'date', ''))
        rev = unquote_string(get_value(sexp, 'rev', ''))
        company = unquote_string(get_value(sexp, 'company', ''))

        comments = {}
        for comment_elem in find_all_elements(sexp, 'comment'):
            if len(comment_elem) >= 3:
                num = int(comment_elem[1])
                text = unquote_string(comment_elem[2])
                comments[num] = text

        return cls(
            title=title, date=date, rev=rev, company=company,
            comments=comments,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result: SexpList = ['title_block']
        if self.title:
            result.append(['title', QuotedString(self.title)])
        if self.date:
            result.append(['date', QuotedString(self.date)])
        if self.rev:
            result.append(['rev', QuotedString(self.rev)])
        if self.company:
            result.append(['company', QuotedString(self.company)])
        for num in sorted(self.comments.keys()):
            result.append(['comment', num, QuotedString(self.comments[num])])
        return result


# -----------------------------------------------------------------------------
# Table
# -----------------------------------------------------------------------------

@dataclass
class TableCell:
    """Cell within a table element."""
    text: str = ""
    start_x: float = 0.0
    start_y: float = 0.0
    end_x: float = 0.0
    end_y: float = 0.0
    margins: Tuple[float, float, float, float] = (0.0, 0.0, 0.0, 0.0)
    span: Tuple[int, int] = (1, 1)  # col_span, row_span
    angle: float = 0.0
    layer: str = FRONT_COPPER_LAYER
    locked: bool = False
    effects: Optional[Effects] = None
    render_cache: Optional[RenderCache] = None
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'TableCell':
        text = unquote_string(sexp[1]) if len(sexp) > 1 else ""

        start = find_element(sexp, 'start')
        end = find_element(sexp, 'end')
        start_x = float(start[1]) if start else 0.0
        start_y = float(start[2]) if start else 0.0
        end_x = float(end[1]) if end else 0.0
        end_y = float(end[2]) if end else 0.0

        margins_elem = find_element(sexp, 'margins')
        margins = (0.0, 0.0, 0.0, 0.0)
        if margins_elem and len(margins_elem) >= 5:
            margins = (float(margins_elem[1]), float(margins_elem[2]),
                      float(margins_elem[3]), float(margins_elem[4]))

        span_elem = find_element(sexp, 'span')
        span = (1, 1)
        if span_elem and len(span_elem) >= 3:
            span = (int(span_elem[1]), int(span_elem[2]))

        effects = None
        effects_elem = find_element(sexp, 'effects')
        if effects_elem:
            effects = Effects.from_sexp(sexp)

        return cls(
            text=text,
            start_x=start_x, start_y=start_y,
            end_x=end_x, end_y=end_y,
            margins=margins,
            span=span,
            angle=float(get_value(sexp, 'angle', 0.0)),
            layer=unquote_string(get_value(sexp, 'layer', FRONT_COPPER_LAYER)),
            locked=_parse_yes_no_bool(find_element(sexp, 'locked'), False),
            effects=effects,
            render_cache=RenderCache.from_sexp(sexp),
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result = ['table_cell', QuotedString(self.text)]
        if self.locked:
            result.append(['locked', 'yes'])
        result.extend([
            ['start', self.start_x, self.start_y],
            ['end', self.end_x, self.end_y],
        ])
        if any(m != 0.0 for m in self.margins):
            result.append(['margins', self.margins[0], self.margins[1],
                          self.margins[2], self.margins[3]])
        result.append(['span', self.span[0], self.span[1]])
        if self.angle != 0.0:
            result.append(['angle', self.angle])
        result.append(['layer', QuotedString(self.layer)])
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        if self.effects:
            result.append(self.effects.to_sexp())
        if self.render_cache:
            result.append(self.render_cache.to_sexp())
        return result

    @staticmethod
    def _rotate_point(x: float, y: float, angle: float) -> Tuple[float, float]:
        radians = math.radians(angle)
        cos_a = math.cos(radians)
        sin_a = math.sin(radians)
        return (x * cos_a + y * sin_a, y * cos_a - x * sin_a)

    def _as_board_text_box(self, footprint: Optional[KiCadFootprint] = None) -> GrTextBox:
        fp_angle = float(getattr(footprint, "at_angle", 0.0) or 0.0)
        fp_x = float(getattr(footprint, "at_x", 0.0) or 0.0)
        fp_y = float(getattr(footprint, "at_y", 0.0) or 0.0)
        start_x, start_y = self._rotate_point(self.start_x, self.start_y, fp_angle)
        end_x, end_y = self._rotate_point(self.end_x, self.end_y, fp_angle)
        local_corners = [
            (self.start_x, self.start_y),
            (self.end_x, self.start_y),
            (self.end_x, self.end_y),
            (self.start_x, self.end_y),
        ]
        polygon_points = []
        for x, y in local_corners:
            point_x, point_y = self._rotate_point(x, y, fp_angle)
            polygon_points.append((point_x + fp_x, point_y + fp_y))

        return GrTextBox(
            text=self.text,
            start_x=start_x + fp_x,
            start_y=start_y + fp_y,
            end_x=end_x + fp_x,
            end_y=end_y + fp_y,
            margins=self.margins,
            angle=(self.angle + fp_angle) % 360.0,
            polygon_points=polygon_points if fp_angle % 360.0 else None,
            layer=self.layer,
            locked=self.locked,
            effects=self.effects,
            render_cache=self.render_cache,
            uuid=self.uuid,
        )

    def render_cache_text(self, text: Optional[str] = None, footprint: Optional[KiCadFootprint] = None) -> str:
        """Return resolved table-cell text after KiCad text-box wrapping."""

        return self._as_board_text_box(footprint).render_cache_text(text)

    def to_text_params(self, text: Optional[str] = None, footprint: Optional[KiCadFootprint] = None) -> TextParams:
        """Convert a table cell to text params through the text-box layout path."""

        return self._as_board_text_box(footprint).to_text_params(text)


@dataclass
class Table:
    """Table element containing cells."""
    column_count: int = 1
    layer: str = FRONT_COPPER_LAYER
    border_external: bool = True
    border_header: bool = False
    border_stroke: Optional[Stroke] = None
    separators_rows: bool = True
    separators_cols: bool = True
    separators_stroke: Optional[Stroke] = None
    column_widths: List[float] = field(default_factory=list)
    row_heights: List[float] = field(default_factory=list)
    cells: List[TableCell] = field(default_factory=list)
    uuid: Optional[str] = None
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Table':
        column_count = int(get_value(sexp, 'column_count', 1))
        layer = unquote_string(get_value(sexp, 'layer', FRONT_COPPER_LAYER))

        # Border settings
        border_elem = find_element(sexp, 'border')
        border_external = True
        border_header = False
        border_stroke = None
        if border_elem:
            border_external = get_value(border_elem, 'external') == 'yes'
            border_header = get_value(border_elem, 'header') == 'yes'
            stroke_elem = find_element(border_elem, 'stroke')
            if stroke_elem:
                border_stroke = Stroke.from_sexp(border_elem)

        # Separator settings
        sep_elem = find_element(sexp, 'separators')
        separators_rows = True
        separators_cols = True
        separators_stroke = None
        if sep_elem:
            separators_rows = get_value(sep_elem, 'rows') == 'yes'
            separators_cols = get_value(sep_elem, 'cols') == 'yes'
            stroke_elem = find_element(sep_elem, 'stroke')
            if stroke_elem:
                separators_stroke = Stroke.from_sexp(sep_elem)

        # Column widths and row heights
        col_widths_elem = find_element(sexp, 'column_widths')
        column_widths = []
        if col_widths_elem:
            column_widths = [float(v) for v in col_widths_elem[1:]]

        row_heights_elem = find_element(sexp, 'row_heights')
        row_heights = []
        if row_heights_elem:
            row_heights = [float(v) for v in row_heights_elem[1:]]

        # Cells
        cells = []
        cells_elem = find_element(sexp, 'cells')
        if cells_elem:
            for cell_elem in find_all_elements(cells_elem, 'table_cell'):
                cells.append(TableCell.from_sexp(cell_elem))

        return cls(
            column_count=column_count,
            layer=layer,
            border_external=border_external,
            border_header=border_header,
            border_stroke=border_stroke,
            separators_rows=separators_rows,
            separators_cols=separators_cols,
            separators_stroke=separators_stroke,
            column_widths=column_widths,
            row_heights=row_heights,
            cells=cells,
            uuid=unquote_string(get_value(sexp, 'uuid')),
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result = ['table',
                  ['column_count', self.column_count],
                  ['layer', QuotedString(self.layer)]]

        # Border
        border: SexpList = ['border']
        border.append(['external', 'yes' if self.border_external else 'no'])
        border.append(['header', 'yes' if self.border_header else 'no'])
        if self.border_stroke:
            border.append(self.border_stroke.to_sexp())
        result.append(border)

        # Separators
        sep: SexpList = ['separators']
        sep.append(['rows', 'yes' if self.separators_rows else 'no'])
        sep.append(['cols', 'yes' if self.separators_cols else 'no'])
        if self.separators_stroke:
            sep.append(self.separators_stroke.to_sexp())
        result.append(sep)

        # Widths and heights
        if self.column_widths:
            result.append(['column_widths'] + self.column_widths)
        if self.row_heights:
            result.append(['row_heights'] + self.row_heights)

        # Cells
        if self.cells:
            cells_elem: SexpList = ['cells']
            for cell in self.cells:
                cells_elem.append(cell.to_sexp())
            result.append(cells_elem)

        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        return result


# -----------------------------------------------------------------------------
# Group
# -----------------------------------------------------------------------------

@dataclass
class Group:
    """Group of PCB items."""
    name: str
    uuid: Optional[str] = None
    locked: bool = False
    members: List[str] = field(default_factory=list)
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Group':
        name = unquote_string(sexp[1])
        # parseGROUP at pcb_io_kicad_sexpr_parser.cpp:7155 accepts both
        # `(id ...)` (formats [20200811, 20231215)) and `(uuid ...)`.
        # We always emit the canonical (uuid ...) form on the way out.
        uuid = unquote_string(get_value(sexp, 'uuid'))
        if not uuid:
            uuid = unquote_string(get_value(sexp, 'id'))
        locked = has_flag(sexp, 'locked') or _parse_yes_no_bool(find_element(sexp, 'locked'), False)

        members_elem = find_element(sexp, 'members')
        members = [unquote_string(m) for m in members_elem[1:]] if members_elem else []

        return cls(name=name, uuid=uuid, locked=locked, members=members, _raw_sexp=sexp)

    def to_sexp(self) -> list:
        result = ['group', QuotedString(self.name)]
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        if self.locked:
            result.append(['locked', 'yes'])
        if self.members:
            result.append(['members'] + [QuotedString(m) for m in self.members])
        return result


# -----------------------------------------------------------------------------
# Unknown Element
# -----------------------------------------------------------------------------

@dataclass
class UnknownElement:
    """
    Raw S-expression for elements we don't parse.

    This provides forward/backward compatibility by preserving unknown
    elements verbatim during round-trip parsing.
    """
    name: str = ""
    raw_sexp: list = field(default_factory=list)

    def to_sexp(self) -> list:
        return self.raw_sexp


__all__ = [
    'Layer',
    'Net',
    'NetRef',
    'OutlineCarrier',
    'BarcodeMargins',
    'Barcode',
    'BoardProperty',
    'BoardVariant',
    'FootprintVariantField',
    'FootprintVariant',
    'FootprintPlacement',
    'ComponentClassRef',
    'GeneratedProperty',
    'GeneratedObject',
    'StackupLayerSubLayer',
    'StackupLayer',
    'Stackup',
    'DimensionFormat',
    'DimensionStyle',
    'Dimension',
    'Image',
    'TitleBlock',
    'TableCell',
    'Table',
    'Group',
    'UnknownElement',
]

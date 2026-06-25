"""
KiCad PCB File Parser

A Python parser that can deserialize and reserialize KiCad PCB files with 100% round-trip fidelity.

KiCad Source Reference:
    Baseline review tag: 10.0.0-rc2
    Baseline review commit: a2f3efc77
    Upstream comparison head: 37ef308ef3
    Review date: 2026-03-16
    Source: https://gitlab.com/kicad/code/kicad
    File format docs: https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/
    Key files referenced:
    - common/io/kicad/kicad_io_utils.cpp - S-expression formatting
    - pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp - PCB file I/O
    - pcbnew/board.h - BOARD class definition

This module provides:
- `KiCadPcb`: Main class representing a complete PCB file
- `from_kicad_pcb()`: Parse a .kicad_pcb file into Python objects
- `to_kicad_pcb()`: Serialize Python objects back to .kicad_pcb format

The parser preserves all formatting details including:
- Indentation (tabs)
- Number precision
- Quoted vs bare strings
- Base64 embedded data with proper line wrapping
- Order of elements

Note on Net Classes:
    As of KiCad 6.0 (July 2020), net class definitions are stored in the project
    file (.kicad_pro) under `net_settings.classes[]` and `net_settings.netclass_assignments{}`,
    NOT in the PCB file. The legacy `net_class` element was removed from the PCB format.
    See: https://forum.kicad.info/t/new-project-file-format/23705

KiCad PCB File Format Reference:
https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/index.html
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Iterator, List, Optional, Union, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox
    from .kicad_object_collection import KiCadObjectCollection
    from .kicad_plotter_ir import KiCadPlotterDocument
    from .kicad_sch_svg_renderer import KiCadSvgRenderOptions

from ._api_markers import public_api
from .kicad_defaults import (
    KICAD_DEFAULT_BOARD_THICKNESS_MM,
    KICAD_DEFAULT_PAPER,
    KICAD_GENERATOR_VERSION,
    KICAD_PCB_FILE_VERSION,
    KICAD_PCB_GENERATOR,
)
from .kicad_sexpr import parse_sexp, QuotedString

# Import from modular files
from .kicad_base import (
    EDGE_CUTS_LAYER,
    # Constants
    INDENT_CHAR,
    INDENT_SIZE,
    XY_COLUMN_LIMIT,
    TOKEN_WRAP_THRESHOLD,
    MIME_BASE64_LENGTH,
    # Enums
    LayerType,
    StrokeType,
    FillType,
    HAlign,
    VAlign,
    PadType,
    PadShape,
    ZoneConnectionType,
    StackupItemType,
    EdgeConnectorConstraint,
    PlacementSourceType,
    # Utilities
    find_element,
    find_all_elements,
    get_value,
    get_values,
    has_flag,
    get_at,
    format_float,
    quote_string,
    unquote_string,
)

from .kicad_pcb_sexp import SexpWriter

from .kicad_primitives import (
    Stroke,
    Font,
    Effects,
    RenderCacheContour,
    RenderCachePolygon,
    RenderCache,
)

# Import graphics elements from individual files with _to_poly implementations
from .kicad_pcb_gr_text import GrText
from .kicad_pcb_gr_line import GrLine
from .kicad_pcb_gr_rect import GrRect
from .kicad_pcb_gr_arc import GrArc
from .kicad_pcb_gr_circle import GrCircle
from .kicad_pcb_gr_poly import GrPoly
from .kicad_pcb_gr_curve import GrCurve
from .kicad_pcb_graphics import GrTextBox  # GrTextBox doesn't need _to_poly

from .kicad_pcb_footprint import (
    Pad,
    FpText,
    Property,
    FpLine,
    FpPoly,
    Model,
    EmbeddedFile,
    Footprint,
)

from .kicad_pcb_routing import (
    Segment,
    Via,
    Arc,
)

from .kicad_pcb_zone import (
    ZonePlacement,
    Keepout,
    ZonePolygon,
    FilledPolygon,
    Zone,
)

from .kicad_pcb_other import (
    Barcode,
    BarcodeMargins,
    BoardVariant,
    ComponentClassRef,
    GeneratedObject,
    DrillLayerSpan,
    DrillProps,
    FootprintPlacement,
    Layer,
    Net,
    NetRef,
    OutlineCarrier,
    PadNameGroup,
    PostMachiningProps,
    BoardProperty,
    StackupLayerSubLayer,
    StackupLayer,
    Stackup,
    DimensionFormat,
    DimensionStyle,
    Dimension,
    Image,
    TitleBlock,
    TableCell,
    Table,
    Group,
    GeneratedProperty,
    UnknownElement,
    ZoneLayerConnections,
)
from .kicad_project import (
    KiCadProjectSidecar,
    find_adjacent_kicad_project_path,
)


# =============================================================================
# Main KiCad PCB Class
# =============================================================================

_PCB_OBJECT_LIST_NAMES: tuple[str, ...] = (
    "layers",
    "nets",
    "properties",
    "variants",
    "gr_texts",
    "gr_lines",
    "gr_rects",
    "gr_arcs",
    "gr_circles",
    "gr_polys",
    "gr_curves",
    "gr_text_boxes",
    "images",
    "barcodes",
    "tables",
    "footprints",
    "zones",
    "dimensions",
    "segments",
    "vias",
    "arcs",
    "groups",
    "generated_items",
    "embedded_files",
    "unknown_elements",
)

_PCB_OBJECT_LIST_BY_CLASS_NAME: dict[str, str] = {
    "Layer": "layers",
    "Net": "nets",
    "BoardProperty": "properties",
    "BoardVariant": "variants",
    "GrText": "gr_texts",
    "GrLine": "gr_lines",
    "GrRect": "gr_rects",
    "GrArc": "gr_arcs",
    "GrCircle": "gr_circles",
    "GrPoly": "gr_polys",
    "GrCurve": "gr_curves",
    "GrTextBox": "gr_text_boxes",
    "Image": "images",
    "Barcode": "barcodes",
    "Table": "tables",
    "Footprint": "footprints",
    "Zone": "zones",
    "Dimension": "dimensions",
    "Segment": "segments",
    "Via": "vias",
    "Arc": "arcs",
    "Group": "groups",
    "GeneratedObject": "generated_items",
    "EmbeddedFile": "embedded_files",
    "UnknownElement": "unknown_elements",
}

_PCB_KNOWN_ELEMENTS = {
    'kicad_pcb', 'version', 'generator', 'generator_version',
    'general', 'paper', 'title_block', 'layers', 'setup', 'net',
    'property', 'variants',  # Board-level custom metadata / variant registry
    'gr_text', 'gr_line', 'gr_rect', 'gr_arc', 'gr_circle', 'gr_poly',
    'gr_curve', 'gr_text_box', 'barcode', 'image', 'table',
    'footprint', 'module', 'zone', 'dimension', 'segment', 'via', 'arc',
    'group', 'generated', 'embedded_fonts', 'embedded_files',
}

_PCB_COLLECTION_PARSERS: tuple[tuple[str, str, Any], ...] = (
    ('net', 'nets', Net),
    ('property', 'properties', BoardProperty),
    ('gr_text', 'gr_texts', GrText),
    ('gr_line', 'gr_lines', GrLine),
    ('gr_rect', 'gr_rects', GrRect),
    ('gr_arc', 'gr_arcs', GrArc),
    ('gr_circle', 'gr_circles', GrCircle),
    ('gr_poly', 'gr_polys', GrPoly),
    ('gr_curve', 'gr_curves', GrCurve),
    ('gr_text_box', 'gr_text_boxes', GrTextBox),
    ('barcode', 'barcodes', Barcode),
    ('image', 'images', Image),
    ('table', 'tables', Table),
    ('footprint', 'footprints', Footprint),
    ('module', 'footprints', Footprint),
    ('dimension', 'dimensions', Dimension),
    ('group', 'groups', Group),
    ('generated', 'generated_items', GeneratedObject),
)


def _append_parsed_elements(target: list, sexp: list, element_name: str, factory: Any) -> None:
    for elem in find_all_elements(sexp, element_name):
        target.append(factory.from_sexp(elem))


def _parse_pcb_header_and_setup(pcb: "KiCadPcb", sexp: list) -> None:
    pcb.version = int(get_value(sexp, 'version', KICAD_PCB_FILE_VERSION))
    pcb.generator = unquote_string(get_value(sexp, 'generator', KICAD_PCB_GENERATOR))
    pcb.generator_version = unquote_string(get_value(sexp, 'generator_version', KICAD_GENERATOR_VERSION))

    general = find_element(sexp, 'general')
    if general:
        pcb.thickness = float(get_value(general, 'thickness', KICAD_DEFAULT_BOARD_THICKNESS_MM))
        pcb.legacy_teardrops = get_value(general, 'legacy_teardrops') == 'yes'

    pcb.paper = unquote_string(get_value(sexp, 'paper', KICAD_DEFAULT_PAPER))

    title_block_elem = find_element(sexp, 'title_block')
    if title_block_elem:
        pcb.title_block = TitleBlock.from_sexp(title_block_elem)

    pcb.setup_sexp = find_element(sexp, 'setup')
    if pcb.setup_sexp:
        _parse_pcb_setup_settings(pcb)


def _parse_pcb_setup_settings(pcb: "KiCadPcb") -> None:
    setup_sexp = pcb.setup_sexp
    if setup_sexp is None:
        return
    stackup_elem = find_element(setup_sexp, 'stackup')
    if stackup_elem:
        pcb.stackup = Stackup.from_sexp(stackup_elem)

    for element_name, attr_name in (
        ('pad_to_mask_clearance', 'pad_to_mask_clearance'),
        ('pad_to_paste_clearance', 'pad_to_paste_clearance'),
        ('pad_to_paste_clearance_ratio', 'pad_to_paste_clearance_ratio'),
    ):
        value = get_value(setup_sexp, element_name, None)
        if value is not None:
            setattr(pcb, attr_name, float(value))


def _parse_pcb_layers(pcb: "KiCadPcb", sexp: list) -> None:
    layers_elem = find_element(sexp, 'layers')
    if not layers_elem:
        return
    for layer_def in layers_elem[1:]:
        if isinstance(layer_def, list):
            pcb.layers.append(Layer.from_sexp(layer_def))


def _parse_pcb_variants(pcb: "KiCadPcb", sexp: list) -> None:
    variants_elem = find_element(sexp, 'variants')
    if variants_elem:
        _append_parsed_elements(pcb.variants, variants_elem, 'variant', BoardVariant)


def _parse_pcb_collections(pcb: "KiCadPcb", sexp: list) -> None:
    _parse_pcb_layers(pcb, sexp)
    _parse_pcb_variants(pcb, sexp)
    for element_name, attr_name, factory in _PCB_COLLECTION_PARSERS:
        _append_parsed_elements(getattr(pcb, attr_name), sexp, element_name, factory)


def _resolve_pcb_net_ref(pcb: "KiCadPcb", net_ref: NetRef) -> NetRef:
    net_name_by_id = {net.ordinal: net.name for net in pcb.nets}
    net_id_by_name = {net.name: net.ordinal for net in pcb.nets}
    return net_ref.resolve_name(net_name_by_id).resolve_ordinal(net_id_by_name)


def _resolve_pcb_object_net(pcb: "KiCadPcb", obj: Any) -> None:
    net_ref = getattr(obj, 'net', NetRef())
    if isinstance(net_ref, NetRef):
        obj.net = _resolve_pcb_net_ref(pcb, net_ref)


def _resolve_pcb_net_bound_items(pcb: "KiCadPcb", sexp: list) -> None:
    for footprint in pcb.footprints:
        for pad in getattr(footprint, 'pads', []) or []:
            _resolve_pcb_object_net(pcb, pad)

    for elem in find_all_elements(sexp, 'zone'):
        zone = Zone.from_sexp(elem)
        zone.net = _resolve_pcb_net_ref(pcb, zone.net)
        pcb.zones.append(zone)

    for element_name, attr_name, factory in (
        ('segment', 'segments', Segment),
        ('via', 'vias', Via),
        ('arc', 'arcs', Arc),
    ):
        for elem in find_all_elements(sexp, element_name):
            item = factory.from_sexp(elem)
            _resolve_pcb_object_net(pcb, item)
            getattr(pcb, attr_name).append(item)


def _parse_pcb_embedded_files(pcb: "KiCadPcb", sexp: list) -> None:
    pcb.embedded_fonts = get_value(sexp, 'embedded_fonts') == 'yes'
    embedded_files_elem = find_element(sexp, 'embedded_files')
    if embedded_files_elem:
        _append_parsed_elements(pcb.embedded_files, embedded_files_elem, 'file', EmbeddedFile)


def _preserve_unknown_pcb_elements(pcb: "KiCadPcb", sexp: list) -> None:
    for elem in sexp[1:]:
        if isinstance(elem, list) and len(elem) > 0 and elem[0] not in _PCB_KNOWN_ELEMENTS:
            pcb.unknown_elements.append(UnknownElement(name=elem[0], raw_sexp=elem))


_PCB_GRAPHIC_SEXP_ATTRS: tuple[str, ...] = (
    'gr_rects',
    'gr_lines',
    'gr_arcs',
    'gr_circles',
    'gr_polys',
    'gr_curves',
    'gr_text_boxes',
    'gr_texts',
)

_PCB_TRACK_SEXP_ATTRS: tuple[str, ...] = ('segments', 'vias', 'arcs')


def _append_serialized_items(result: list, items: Any) -> None:
    for item in items:
        result.append(item.to_sexp())


def _append_pcb_header(result: list, pcb: "KiCadPcb") -> None:
    result.append([
        'general',
        ['thickness', pcb.thickness],
        ['legacy_teardrops', 'yes' if pcb.legacy_teardrops else 'no'],
    ])
    result.append(['paper', QuotedString(pcb.paper)])
    if pcb.title_block:
        result.append(pcb.title_block.to_sexp())


def _append_pcb_layers(result: list, layers: list[Layer]) -> None:
    layers_elem: list = ['layers']
    _append_serialized_items(layers_elem, layers)
    result.append(layers_elem)


def _append_pcb_variants(result: list, variants: list[BoardVariant]) -> None:
    if not variants:
        return
    variants_elem = ['variants']
    _append_serialized_items(variants_elem, variants)
    result.append(variants_elem)


def _append_pcb_attr_collections(
    result: list,
    pcb: "KiCadPcb",
    attr_names: tuple[str, ...],
) -> None:
    for attr_name in attr_names:
        _append_serialized_items(result, getattr(pcb, attr_name))


def _append_pcb_embedded_files(result: list, pcb: "KiCadPcb") -> None:
    result.append(['embedded_fonts', 'yes' if pcb.embedded_fonts else 'no'])
    if pcb.embedded_files:
        ef_elem = ['embedded_files']
        _append_serialized_items(ef_elem, pcb.embedded_files)
        result.append(ef_elem)


# Default board scaffold matching KiCad's File -> New Board stack, expressed
# through the object model rather than a hard-coded s-expression template.
# ``(ordinal, canonical_name, type, user_name)`` per KiCad's standard layer set.
_DEFAULT_LAYER_STACK: tuple[tuple[int, str, LayerType, str | None], ...] = (
    (0, "F.Cu", LayerType.SIGNAL, None),
    (2, "B.Cu", LayerType.SIGNAL, None),
    (9, "F.Adhes", LayerType.USER, "F.Adhesive"),
    (11, "B.Adhes", LayerType.USER, "B.Adhesive"),
    (13, "F.Paste", LayerType.USER, None),
    (15, "B.Paste", LayerType.USER, None),
    (5, "F.SilkS", LayerType.USER, "F.Silkscreen"),
    (7, "B.SilkS", LayerType.USER, "B.Silkscreen"),
    (1, "F.Mask", LayerType.USER, None),
    (3, "B.Mask", LayerType.USER, None),
    (17, "Dwgs.User", LayerType.USER, "User.Drawings"),
    (19, "Cmts.User", LayerType.USER, "User.Comments"),
    (21, "Eco1.User", LayerType.USER, "User.Eco1"),
    (23, "Eco2.User", LayerType.USER, "User.Eco2"),
    (25, "Edge.Cuts", LayerType.USER, None),
    (27, "Margin", LayerType.USER, None),
    (31, "F.CrtYd", LayerType.USER, "F.Courtyard"),
    (29, "B.CrtYd", LayerType.USER, "B.Courtyard"),
    (35, "F.Fab", LayerType.USER, None),
    (33, "B.Fab", LayerType.USER, None),
    (39, "User.1", LayerType.USER, None),
    (41, "User.2", LayerType.USER, None),
    (43, "User.3", LayerType.USER, None),
    (45, "User.4", LayerType.USER, None),
)


def _default_layer_stack() -> List[Layer]:
    """Build KiCad's default board layer set as ``Layer`` objects."""
    return [
        Layer(ordinal=ordinal, canonical_name=name, layer_type=layer_type, user_name=user_name)
        for ordinal, name, layer_type, user_name in _DEFAULT_LAYER_STACK
    ]


def _default_setup_sexp() -> list:
    """Build KiCad's default board ``(setup)`` block as an s-expression list.

    The setup block is modeled as raw s-expression on :class:`KiCadPcb`
    (``setup_sexp``); this composes the default plotting/soldermask settings a
    fresh KiCad board carries so a blank board opens with sensible defaults.
    """
    return [
        "setup",
        ["pad_to_mask_clearance", 0],
        ["allow_soldermask_bridges_in_footprints", "no"],
        ["tenting", ["front", "yes"], ["back", "yes"]],
        ["covering", ["front", "no"], ["back", "no"]],
        ["plugging", ["front", "no"], ["back", "no"]],
        ["capping", "no"],
        ["filling", "no"],
        [
            "pcbplotparams",
            ["layerselection", "0x00000000_00000000_55555555_5755f5ff"],
            ["plot_on_all_layers_selection", "0x00000000_00000000_00000000_00000000"],
            ["disableapertmacros", "no"],
            ["usegerberextensions", "no"],
            ["usegerberattributes", "yes"],
            ["usegerberadvancedattributes", "yes"],
            ["creategerberjobfile", "yes"],
            ["svgprecision", 4],
            ["plotframeref", "no"],
            ["mode", 1],
            ["useauxorigin", "no"],
            ["plot_black_and_white", "yes"],
            ["subtractmaskfromsilk", "no"],
            ["outputformat", 1],
            ["mirror", "no"],
            ["drillshape", 1],
            ["scaleselection", 1],
            ["outputdirectory", QuotedString("")],
        ],
    ]


@public_api
class KiCadPcb:
    """
    Complete KiCad PCB file representation.

    Supports full round-trip: file -> object -> file with 100% fidelity.

    Example:
        >>> pcb = KiCadPcb("board.kicad_pcb")
        >>> pcb.footprints[0].reference
        >>> pcb.save("output.kicad_pcb")
    """

    def __init__(self, path: Union[str, Path, None] = None) -> None:
        """Create a KiCadPcb.

        Args:
            path: Path to .kicad_pcb file to parse.
                  If None, creates an empty PCB.
        """
        # Header
        self.version: int = KICAD_PCB_FILE_VERSION
        self.generator: str = KICAD_PCB_GENERATOR
        self.generator_version: str = KICAD_GENERATOR_VERSION

        # General
        self.thickness: float = KICAD_DEFAULT_BOARD_THICKNESS_MM
        self.legacy_teardrops: bool = False

        # Page
        self.paper: str = KICAD_DEFAULT_PAPER

        # Layers
        self.layers: List[Layer] = []

        # Setup
        self.setup_sexp: list | None = None
        self.stackup: Stackup | None = None
        self.pad_to_mask_clearance: float = 0.0
        self.pad_to_paste_clearance: float = 0.0
        self.pad_to_paste_clearance_ratio: float = 0.0

        # Nets
        self.nets: List[Net] = []

        # Board-level properties
        self.properties: list = []
        self.variants: list = []
        self.title_block: TitleBlock | None = None

        # Graphics
        self.gr_texts: list = []
        self.gr_lines: list = []
        self.gr_rects: list = []
        self.gr_arcs: list = []
        self.gr_circles: list = []
        self.gr_polys: list = []
        self.gr_curves: list = []
        self.gr_text_boxes: list = []

        # Images, barcodes, tables
        self.images: list = []
        self.barcodes: list = []
        self.tables: list = []

        # Footprints
        self.footprints: list = []

        # Zones
        self.zones: list = []

        # Dimensions
        self.dimensions: list = []

        # Tracks
        self.segments: list = []
        self.vias: list = []
        self.arcs: list = []

        # Groups
        self.groups: list = []

        # Generated board items
        self.generated_items: list = []

        # Embedded files
        self.embedded_fonts: bool = False
        self.embedded_files: list = []

        # Source/project context
        self.source_path = None
        self.project = None

        # Unknown elements
        self.unknown_elements: list = []
        self._raw_sexp = None

        if path is not None:
            self._load_from_file(Path(path))

    def _load_from_file(self, path: Path) -> None:
        """Parse a .kicad_pcb file into this instance."""
        content = path.read_text(encoding='utf-8')
        parsed = self.from_string(content)
        parsed.source_path = path
        project_path = find_adjacent_kicad_project_path(path)
        if project_path is not None:
            parsed.project = KiCadProjectSidecar.from_file(project_path)
        self.__dict__.update(parsed.__dict__)

    @classmethod
    def from_file(cls, path: Union[str, Path]) -> 'KiCadPcb':
        """Load a KiCad PCB file. Deprecated: use ``KiCadPcb(path)``."""
        path = Path(path)
        content = path.read_text(encoding='utf-8')
        pcb = cls.from_string(content)
        pcb.source_path = path
        project_path = find_adjacent_kicad_project_path(path)
        if project_path is not None:
            pcb.project = KiCadProjectSidecar.from_file(project_path)
        return pcb

    @classmethod
    def from_string(cls, content: str) -> 'KiCadPcb':
        """Parse KiCad PCB content from string."""
        sexp = parse_sexp(content)
        return cls.from_sexp(sexp)

    @classmethod
    @public_api
    def new(cls, *, paper: str = KICAD_DEFAULT_PAPER) -> 'KiCadPcb':
        """Create a blank, openable board with KiCad's default 2-layer stack.

        The bare constructor leaves ``layers`` empty, which KiCad rejects with
        "0 is not a valid layer count"; this seeds the standard copper stack and
        a default ``setup`` block — both built through the object model — so the
        board opens in KiCad.
        """
        pcb = cls()
        pcb.layers = _default_layer_stack()
        pcb.setup_sexp = _default_setup_sexp()
        pcb.paper = paper
        return pcb

    @classmethod
    def from_sexp(cls, sexp: list) -> 'KiCadPcb':
        """Parse from s-expression list."""
        if sexp[0] != 'kicad_pcb':
            raise ValueError(f"Expected 'kicad_pcb', got '{sexp[0]}'")

        pcb = cls()
        pcb._raw_sexp = sexp

        _parse_pcb_header_and_setup(pcb, sexp)
        _parse_pcb_collections(pcb, sexp)
        _resolve_pcb_net_bound_items(pcb, sexp)
        _parse_pcb_embedded_files(pcb, sexp)
        _preserve_unknown_pcb_elements(pcb, sexp)

        return pcb

    def to_sexp(self) -> list:
        """Convert to s-expression list."""
        result: list = ['kicad_pcb',
                        ['version', self.version],
                        ['generator', QuotedString(self.generator)],
                        ['generator_version', QuotedString(self.generator_version)]]

        _append_pcb_header(result, self)
        _append_pcb_layers(result, self.layers)

        # Setup
        if self.setup_sexp:
            result.append(self.setup_sexp)

        # Nets
        _append_serialized_items(result, self.nets)

        # Board-level properties
        _append_serialized_items(result, self.properties)

        _append_pcb_variants(result, self.variants)

        # Footprints
        _append_serialized_items(result, self.footprints)

        # Graphics
        _append_pcb_attr_collections(result, self, _PCB_GRAPHIC_SEXP_ATTRS)

        # Barcodes
        _append_serialized_items(result, self.barcodes)

        # Images
        _append_serialized_items(result, self.images)

        # Tables
        _append_serialized_items(result, self.tables)

        # Zones
        _append_serialized_items(result, self.zones)

        # Dimensions
        _append_serialized_items(result, self.dimensions)

        # Tracks
        _append_pcb_attr_collections(result, self, _PCB_TRACK_SEXP_ATTRS)

        # Groups
        _append_serialized_items(result, self.groups)

        _append_serialized_items(result, self.generated_items)

        # Embedded files
        _append_pcb_embedded_files(result, self)

        # Unknown elements (preserved for round-trip compatibility)
        _append_serialized_items(result, self.unknown_elements)

        return result

    def to_string(self) -> str:
        """Serialize to KiCad PCB format string."""
        sexp = self.to_sexp()
        writer = SexpWriter()
        return writer.write(sexp)

    def save(self, path: Union[str, Path]) -> None:
        """Save to a KiCad PCB file. Canonical save method per ADR-0043."""
        path = Path(path)
        content = self.to_string()
        path.write_text(content, encoding='utf-8')

    def to_file(self, path: Union[str, Path]) -> None:
        """Deprecated: use ``save()``."""
        self.save(path)

    @public_api
    def to_ir(
        self,
        *,
        source_path: str | None = None,
        document_id: str | None = None,
    ) -> "KiCadPlotterDocument":
        """Render this board to plotter IR."""
        from .kicad_pcb_to_ir import pcb_to_ir

        if source_path is None and self.source_path is not None:
            source_path = str(self.source_path)
        return pcb_to_ir(self, source_path=source_path, document_id=document_id)

    @public_api
    def iter_objects(self) -> Iterator[object]:
        """Iterate over top-level board-owned objects."""
        for list_name in _PCB_OBJECT_LIST_NAMES:
            yield from getattr(self, list_name, ())

    @public_api
    def add_object(self, obj: object) -> object:
        """Add a typed object to the matching board-owned list."""
        list_name = _PCB_OBJECT_LIST_BY_CLASS_NAME.get(type(obj).__name__)
        if list_name is None:
            raise TypeError(f"unsupported PCB object type: {type(obj).__name__}")
        getattr(self, list_name).append(obj)
        return obj

    @public_api
    def remove_object(self, obj: object) -> bool:
        """Remove an object by identity from its owning board list."""
        for list_name in _PCB_OBJECT_LIST_NAMES:
            collection: list = getattr(self, list_name)
            for index, candidate in enumerate(collection):
                if candidate is obj:
                    del collection[index]
                    return True
        return False

    @public_api
    @property
    def objects(self) -> "KiCadObjectCollection":
        """Live read-only query view over board-owned objects."""
        from .kicad_object_collection import KiCadObjectCollection

        return KiCadObjectCollection(lambda: self.iter_objects(), owner=self)

    @public_api
    def get_property_object(self, key: str) -> BoardProperty | None:
        """Get a board property object by key."""
        for prop in self.properties:
            if prop.key == str(key):
                return prop
        return None

    @public_api
    def get_property(self, key: str) -> str | None:
        """Get a board property value by key."""
        prop = self.get_property_object(key)
        return prop.value if prop is not None else None

    @public_api
    def get_property_value(self, key: str, default: str = "") -> str:
        """Get a board property value, returning default when absent."""
        value = self.get_property(key)
        return value if value is not None else default

    @public_api
    def set_property_value(self, key: str, value: str, *, create: bool = False) -> bool:
        """Set a board property value. Returns True if updated or created."""
        prop = self.get_property_object(key)
        if prop is not None:
            prop.value = value
            return True
        if create:
            self.upsert_property(key, value)
            return True
        return False

    @public_api
    def upsert_property(self, key: str, value: str) -> BoardProperty:
        """Create or update a board property and return the property object."""
        prop = self.get_property_object(key)
        if prop is not None:
            prop.value = value
            return prop
        prop = BoardProperty(str(key), value)
        self.properties.append(prop)
        return prop

    @public_api
    def remove_property(self, key: str) -> bool:
        """Remove a board property by key."""
        key_text = str(key)
        for index, prop in enumerate(self.properties):
            if prop.key == key_text:
                del self.properties[index]
                return True
        return False

    @public_api
    def iter_properties(self) -> Iterator[BoardProperty]:
        """Iterate over board properties."""
        return iter(self.properties)

    @public_api
    @property
    def aux_axis_origin_mm(self) -> tuple[float, float]:
        """Return the KiCad ``setup/aux_axis_origin`` in millimeters."""
        if self.setup_sexp is None:
            return (0.0, 0.0)
        origin = find_element(self.setup_sexp, "aux_axis_origin")
        if not origin or len(origin) < 3:
            return (0.0, 0.0)
        try:
            return (float(origin[1]), float(origin[2]))
        except (TypeError, ValueError):
            return (0.0, 0.0)

    def net_name_by_ordinal(self) -> dict[int, str]:
        """Return the board net-name table keyed by ordinal."""
        return {
            int(getattr(net, 'ordinal', -1)): str(getattr(net, 'name', '') or '')
            for net in (self.nets or [])
            if getattr(net, 'ordinal', None) is not None
        }

    def resolve_net_ref(self, net_ref: NetRef | None) -> NetRef:
        """Resolve a board-element net reference against the PCB net table when possible."""
        if net_ref is None:
            return NetRef()
        return net_ref.resolve_name(self.net_name_by_ordinal()).resolve_ordinal(
            {name: ordinal for ordinal, name in self.net_name_by_ordinal().items() if name}
        )

    def resolve_net_name(self, net_ref: NetRef | None) -> str:
        """Resolve a board-element net name from a NetRef."""
        return str(self.resolve_net_ref(net_ref).name or "")

    def top_level_outline_items(self, layer_name: str = EDGE_CUTS_LAYER) -> List[object]:
        """Return top-level board graphics that carry the board outline."""
        items: List[object] = []
        for collection_name in (
            'gr_lines',
            'gr_arcs',
            'gr_rects',
            'gr_circles',
            'gr_polys',
            'gr_curves',
        ):
            for item in getattr(self, collection_name, []) or []:
                if str(getattr(item, 'layer', '') or '').strip() == layer_name:
                    items.append(item)
        return items

    def footprint_outline_items(self, layer_name: str = EDGE_CUTS_LAYER) -> List[OutlineCarrier]:
        """Return footprint-local outline carriers promoted to board-level meaning."""
        carriers: List[OutlineCarrier] = []
        for fp_index, footprint in enumerate(self.footprints or []):
            owner_ref = str(
                getattr(footprint, 'uuid', '')
                or getattr(footprint, 'library_link', '')
                or f'footprint:{fp_index}'
            )
            for item in footprint.outline_items(layer_name=layer_name):
                carriers.append(
                    OutlineCarrier(
                        owner_kind='footprint',
                        owner_ref=owner_ref,
                        item=item,
                    )
                )
        return carriers

    def board_outline_carriers(
        self,
        *,
        include_top_level: bool = True,
        include_footprint_local: bool = True,
        layer_name: str = EDGE_CUTS_LAYER,
    ) -> List[OutlineCarrier]:
        """Return all currently recognized board-outline carrier items."""
        carriers: List[OutlineCarrier] = []
        if include_top_level:
            for index, item in enumerate(self.top_level_outline_items(layer_name=layer_name)):
                carriers.append(
                    OutlineCarrier(
                        owner_kind='board',
                        owner_ref=f'board:{index}',
                        item=item,
                    )
                )
        if include_footprint_local:
            carriers.extend(self.footprint_outline_items(layer_name=layer_name))
        return carriers

    def get_bounds(self, layers: Optional[List[str]] = None) -> 'BoundingBox':
        """
        Get bounding box of this PCB..

        Args:
            layers: List of layer names to include. If None, all layers.

        Returns:
            BoundingBox containing all elements on specified layers.
        """
        from .kicad_geometry import BoundingBox

        bbox = BoundingBox()

        # Footprints (contain pads, graphics, text)
        for fp in self.footprints:
            bbox.merge(fp.get_bounds())

        # Routing elements
        for seg in self.segments:
            if layers is None or seg.layer in layers:
                bbox.merge(seg.get_bounds())

        for via in self.vias:
            if layers is None or any(layer in via.layers for layer in (layers or [])):
                bbox.merge(via.get_bounds())

        for arc in self.arcs:
            if layers is None or arc.layer in layers:
                bbox.merge(arc.get_bounds())

        # Zones
        for zone in self.zones:
            if layers is None or zone.layer in layers:
                bbox.merge(zone.get_bounds())

        # Graphics
        for gr in self.gr_lines:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_rects:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_circles:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_arcs:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_polys:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_curves:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_texts:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        for gr in self.gr_text_boxes:
            if layers is None or gr.layer in layers:
                bbox.merge(gr.get_bounds())

        return bbox

    def source_inventory(self, detail: str = "summary") -> dict:
        """Return a ``kicad.pcb.source_inventory`` dict describing parser coverage.

        See :mod:`kicad_monkey.kicad_pcb_source_inventory` for the schema. The
        report is source-side only — it does not classify members as mapped /
        derived / unsupported_expected. ``detail`` ∈ {"summary","objects","debug"}.
        """
        from .kicad_pcb_source_inventory import build_pcb_source_inventory

        return build_pcb_source_inventory(self, detail=detail)

    def to_svg(
        self,
        layers: Optional[List[str]] = None,
        fill: str = "#000000",
        stroke: str = "#000000",
        black_and_white: bool = True,
        profile: str | None = None,
        options: Optional["KiCadSvgRenderOptions"] = None,
    ) -> str:
        """
        Render PCB to SVG using the plotter-IR pipeline.

        The public board SVG entry point goes through
        :func:`kicad_pcb_ir_svg.render_pcb_ir_to_svg`.

        Args:
            layers: List of layer names to include. If None, all layers.
            fill: Fill color (default black)
            stroke: Stroke color (default black)
            black_and_white: If True, force all elements to black/white
            profile: Optional SVG output profile, such as ``"enriched"`` or
                ``"oracle"``.
            options: Optional low-level SVG render options.

        Returns:
            Complete SVG document string
        """
        from .kicad_pcb_ir_svg import render_pcb_ir_to_svg

        return render_pcb_ir_to_svg(
            self,
            layers=layers,
            fill=fill,
            stroke=stroke,
            black_and_white=black_and_white,
            profile=profile,
            options=options,
        )

    def to_svg_ir(
        self,
        layers: Optional[List[str]] = None,
        fill: str = "#000000",
        stroke: str = "#000000",
        black_and_white: bool = True,
        profile: str | None = None,
        options: Optional["KiCadSvgRenderOptions"] = None,
    ) -> str:
        """
        Render PCB via the plotter-IR pipeline.

        Kept as an explicit alias for callers that adopted the IR preview
        before :meth:`to_svg` was cut over.
        """
        from .kicad_pcb_ir_svg import render_pcb_ir_to_svg

        return render_pcb_ir_to_svg(
            self,
            layers=layers,
            fill=fill,
            stroke=stroke,
            black_and_white=black_and_white,
            profile=profile,
            options=options,
        )

# =============================================================================
# Convenience Functions
# =============================================================================

def from_kicad_pcb(path: Union[str, Path]) -> KiCadPcb:
    """Load a KiCad PCB file into Python objects."""
    return KiCadPcb.from_file(path)


def to_kicad_pcb(pcb: KiCadPcb, path: Union[str, Path]) -> None:
    """Save Python objects to a KiCad PCB file."""
    pcb.to_file(path)


# =============================================================================
# Re-exports for backwards compatibility
# =============================================================================

__all__ = [
    # Main class
    'KiCadPcb',
    # Convenience functions
    'from_kicad_pcb',
    'to_kicad_pcb',
    # Constants
    'INDENT_CHAR',
    'INDENT_SIZE',
    'XY_COLUMN_LIMIT',
    'TOKEN_WRAP_THRESHOLD',
    'MIME_BASE64_LENGTH',
    # Enums
    'LayerType',
    'StrokeType',
    'FillType',
    'HAlign',
    'VAlign',
    'PadType',
    'PadShape',
    'ZoneConnectionType',
    'StackupItemType',
    'EdgeConnectorConstraint',
    'PlacementSourceType',
    # Utilities
    'find_element',
    'find_all_elements',
    'get_value',
    'get_values',
    'has_flag',
    'get_at',
    'format_float',
    'quote_string',
    'unquote_string',
    'QuotedString',
    # Classes
    'SexpWriter',
    'Stroke',
    'Font',
    'Effects',
    'RenderCacheContour',
    'RenderCachePolygon',
    'RenderCache',
    'GrText',
    'GrLine',
    'GrRect',
    'GrArc',
    'GrCircle',
    'GrPoly',
    'GrCurve',
    'GrTextBox',
    'Pad',
    'FpText',
    'Property',
    'FpLine',
    'FpPoly',
    'Model',
    'EmbeddedFile',
    'Footprint',
    'Segment',
    'Via',
    'Arc',
    'ZonePlacement',
    'Keepout',
    'ZonePolygon',
    'FilledPolygon',
    'Zone',
    'Layer',
    'Net',
    'NetRef',
    'OutlineCarrier',
    'BarcodeMargins',
    'Barcode',
    'BoardVariant',
    'ComponentClassRef',
    'DrillLayerSpan',
    'DrillProps',
    'FootprintPlacement',
    'PostMachiningProps',
    'ZoneLayerConnections',
    'PadNameGroup',
    'GeneratedProperty',
    'GeneratedObject',
    'BoardProperty',
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


# =============================================================================
# Main (for testing)
# =============================================================================

if __name__ == "__main__":
    import sys

    if len(sys.argv) > 1:
        path = sys.argv[1]
        print(f"Loading: {path}")
        pcb = from_kicad_pcb(path)
        print(f"Version: {pcb.version}")
        print(f"Generator: {pcb.generator}")
        print(f"Layers: {len(pcb.layers)}")
        print(f"Nets: {len(pcb.nets)}")
        print(f"Footprints: {len(pcb.footprints)}")
        print(f"Graphics texts: {len(pcb.gr_texts)}")
        print(f"Zones: {len(pcb.zones)}")

        # Round-trip test
        output = pcb.to_string()
        print(f"\nOutput length: {len(output)} chars")

        # Parse again and compare
        pcb2 = KiCadPcb.from_string(output)
        print(f"Round-trip version: {pcb2.version}")
        print(f"Round-trip footprints: {len(pcb2.footprints)}")

"""
KiCad Schematic Document Parser

Top-level parser for .kicad_sch schematic files.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, Iterator, List, Optional, TYPE_CHECKING, cast

from ._api_markers import public_api
from .kicad_defaults import (
    KICAD_GENERATOR_VERSION,
    KICAD_SCHEMATIC_FILE_VERSION,
    KICAD_SCHEMATIC_GENERATOR,
)
from .kicad_sexpr import parse_sexp, build_sexp, format_sexp, QuotedString
from .kicad_base import find_element, find_all_elements, get_value, unquote_string
from .kicad_model import EmbeddedFile

if TYPE_CHECKING:
    from .kicad_lib_symbol import LibSymbol
    from .kicad_sch_sheet import SchSheet
    from .kicad_sch_symbol import SchSymbol


@dataclass
class SheetInstancePath:
    """Sheet instance path data (for hierarchical designs).

    S-expression format (inside sheet_instances):
        (path "/UUID"
            (page "1")
        )
    """
    path: str = ""
    page: str = ""

    @classmethod
    def from_sexp(cls, sexp: list) -> 'SheetInstancePath':
        """Parse from (path "/UUID" (page "1"))."""
        path = unquote_string(sexp[1]) if len(sexp) > 1 else ""
        page = unquote_string(get_value(sexp, 'page', ''))
        return cls(path=path, page=page)

    def to_sexp(self) -> list:
        """Serialize to S-expression list."""
        result = ['path', QuotedString(self.path)]
        if self.page:
            result.append(['page', QuotedString(self.page)])
        return result


@dataclass
class SymbolInstancePath:
    """Symbol instance path data (for hierarchical designs).

    S-expression format (inside symbol_instances):
        (path "/UUID/UUID"
            (reference "R1")
            (unit 1)
            (value "10k")
            (footprint "...")
        )
    """
    path: str = ""
    reference: str = ""
    unit: int = 1
    value: str = ""
    footprint: str = ""

    @classmethod
    def from_sexp(cls, sexp: list) -> 'SymbolInstancePath':
        """Parse from (path "/UUID" (reference "R1") ...)."""
        path = unquote_string(sexp[1]) if len(sexp) > 1 else ""
        reference = unquote_string(get_value(sexp, 'reference', ''))
        unit = int(get_value(sexp, 'unit', 1))
        value = unquote_string(get_value(sexp, 'value', ''))
        footprint = unquote_string(get_value(sexp, 'footprint', ''))
        return cls(path=path, reference=reference, unit=unit, value=value, footprint=footprint)

    def to_sexp(self) -> list:
        """Serialize to S-expression list."""
        result = ['path', QuotedString(self.path)]
        if self.reference:
            result.append(['reference', QuotedString(self.reference)])
        result.append(['unit', self.unit])
        if self.value:
            result.append(['value', QuotedString(self.value)])
        if self.footprint:
            result.append(['footprint', QuotedString(self.footprint)])
        return result


_SCHEMATIC_OBJECT_LIST_NAMES: tuple[str, ...] = (
    "lib_symbols",
    "symbols",
    "wires",
    "buses",
    "bus_entries",
    "bus_aliases",
    "junctions",
    "no_connects",
    "labels",
    "global_labels",
    "hierarchical_labels",
    "netclass_flags",
    "texts",
    "text_boxes",
    "polylines",
    "arcs",
    "circles",
    "rectangles",
    "beziers",
    "images",
    "tables",
    "groups",
    "rule_areas",
    "sheets",
)

_SCHEMATIC_OBJECT_LIST_BY_CLASS_NAME: dict[str, str] = {
    "LibSymbol": "lib_symbols",
    "SchSymbol": "symbols",
    "SchWire": "wires",
    "SchBus": "buses",
    "SchBusEntry": "bus_entries",
    "SchBusAlias": "bus_aliases",
    "SchJunction": "junctions",
    "SchNoConnect": "no_connects",
    "SchLabel": "labels",
    "SchGlobalLabel": "global_labels",
    "SchHierarchicalLabel": "hierarchical_labels",
    "SchNetclassFlag": "netclass_flags",
    "SchText": "texts",
    "SchTextBox": "text_boxes",
    "SchPolyline": "polylines",
    "SchArc": "arcs",
    "SchCircle": "circles",
    "SchRectangle": "rectangles",
    "SchBezier": "beziers",
    "SchImage": "images",
    "SchTable": "tables",
    "SchGroup": "groups",
    "SchRuleArea": "rule_areas",
    "SchSheet": "sheets",
}


def _append_serialized_items(result: list, items: Iterable[Any]) -> None:
    for item in items:
        result.append(item.to_sexp())


def _append_serialized_group_if_present(
    result: list,
    group_name: str,
    items: Iterable[Any],
) -> None:
    item_list = list(items)
    if not item_list:
        return
    group = [group_name]
    _append_serialized_items(group, item_list)
    result.append(group)


def _append_schematic_lib_symbols(result: list, lib_symbols: Iterable[Any]) -> None:
    lib_syms = ['lib_symbols']
    _append_serialized_items(lib_syms, lib_symbols)
    result.append(lib_syms)


@public_api
class KiCadSchematic:
    """KiCad schematic document (.kicad_sch).

    This is the top-level parser for schematic files. It handles:
    - Paper size and title block
    - Local symbol definitions (lib_symbols)
    - Placed symbol instances
    - Wires, buses, junctions, and other connectivity
    - Labels (local, global, hierarchical)
    - Hierarchical sheets
    - Instance path data for multi-sheet designs

    Example:
        >>> sch = KiCadSchematic("design.kicad_sch")
        >>> for sym in sch.symbols:
        ...     print(f"{sym.reference}: {sym.value}")
        >>> sch.save("output.kicad_sch")
    """

    def __init__(self, path: Path | str | None = None):
        """Create a KiCadSchematic.

        Args:
            path: Path to .kicad_sch file to parse.
                  If None, creates an empty schematic.

        Examples:
            >>> sch = KiCadSchematic("design.kicad_sch")  # parse file
            >>> sch = KiCadSchematic()                      # empty
        """
        from .kicad_sch_title_block import PaperSize, TitleBlock

        self.version: int = KICAD_SCHEMATIC_FILE_VERSION
        self.generator: str = KICAD_SCHEMATIC_GENERATOR
        self.generator_version: str = KICAD_GENERATOR_VERSION
        self.uuid: str = ""

        # Paper and title block
        self.paper: PaperSize = PaperSize()
        self.title_block: TitleBlock | None = None

        # Local symbol definitions (copies from libraries)
        self.lib_symbols: list = []

        # Placed elements - connectivity
        self.symbols: list = []
        self.wires: list = []
        self.buses: list = []
        self.bus_entries: list = []
        self.bus_aliases: list = []
        self.junctions: list = []
        self.no_connects: list = []

        # Labels
        self.labels: list = []
        self.global_labels: list = []
        self.hierarchical_labels: list = []
        self.netclass_flags: list = []

        # Graphics and text
        self.texts: list = []
        self.text_boxes: list = []
        self.polylines: list = []
        self.arcs: list = []
        self.circles: list = []
        self.rectangles: list = []
        self.beziers: list = []
        self.images: list = []
        self.tables: list = []
        self.groups: list = []
        self.rule_areas: list = []

        # Hierarchy
        self.sheets: list = []
        self.sheet_instances: list = []
        self.symbol_instances: list = []

        # Hierarchical loading state.
        # ``source_path`` is the on-disk file this instance was loaded from,
        # used to resolve sub-sheet ``Sheetfile`` properties relatively.
        # ``sub_schematics`` caches loaded child schematics keyed by the
        # ``Sheetfile`` value as it appears in the parent .kicad_sch
        # (relative path string). Multiple sheets may reference the same
        # file — they share a single cached entry.
        self.source_path: Optional[Path] = None
        self.sub_schematics: Dict[str, "KiCadSchematic"] = {}

        # KiCad 9 features
        self.embedded_fonts: bool = False
        # Embedded files (worksheet/drawing sheet, fonts, models, …), keyed by
        # ``kicad-embed://<name>`` references elsewhere in the project.
        self.embedded_files: list[EmbeddedFile] = []

        self._raw_sexp = None

        if path is not None:
            self._load_from_file(Path(path))

    def _load_from_file(
        self,
        path: Path,
        _seen: Optional[set[Path]] = None,
        _cache: Optional[Dict[Path, "KiCadSchematic"]] = None,
    ) -> None:
        """Parse a .kicad_sch file into this instance and recurse into sheets.

        ``_seen`` is the current recursion stack used to break cycles in
        pathological hierarchies. ``_cache`` allows the same schematic file to
        be instantiated under multiple parent sheets.
        """
        try:
            resolved_path = path.resolve()
        except (OSError, ValueError):
            resolved_path = path
        text = path.read_text(encoding='utf-8')
        parsed = self.from_text(text)
        # Copy all fields from the parsed instance
        self.__dict__.update(parsed.__dict__)
        self.source_path = resolved_path
        # sub_schematics is an instance attribute on self by now (the
        # update above replaced it with parsed's empty dict); ensure
        # we have a fresh dict before recursing.
        self.sub_schematics = {}
        if _seen is None:
            _seen = set()
        if _cache is None:
            _cache = {}
        _cache[self.source_path] = self
        _seen.add(self.source_path)
        try:
            self._load_sub_sheets(_seen=_seen, _cache=_cache)
        finally:
            _seen.discard(self.source_path)

    def _load_sub_sheets(
        self,
        _seen: Optional[set[Path]] = None,
        _cache: Optional[Dict[Path, "KiCadSchematic"]] = None,
    ) -> None:
        """Walk ``self.sheets`` and load each ``Sheetfile`` reference.

        Missing files and cycles are skipped silently — KiCad itself
        tolerates dangling sheet references at parse time. Errors during
        sub-sheet parsing surface as a warning so the caller still gets
        a usable parent schematic.
        """
        if self.source_path is None:
            return
        if _seen is None:
            _seen = set()
        if _cache is None:
            _cache = {}
        _cache.setdefault(self.source_path, self)
        base_dir = self.source_path.parent
        for sheet in self.sheets:
            sf = sheet.sheet_file
            if not sf or sf in self.sub_schematics:
                continue
            try:
                child_path = (base_dir / sf).resolve()
            except (OSError, ValueError):
                continue
            if not child_path.exists():
                continue
            if child_path in _seen:
                continue
            child = _cache.get(child_path)
            if child is not None:
                self.sub_schematics[sf] = child
                continue
            child = KiCadSchematic()
            _cache[child_path] = child
            try:
                child._load_from_file(child_path, _seen=_seen, _cache=_cache)
            except Exception:  # pragma: no cover — defensive
                # Don't let a malformed sub-sheet break parent load;
                # callers that need strict loading can re-parse the
                # child explicitly and surface the error themselves.
                _cache.pop(child_path, None)
                continue
            self.sub_schematics[sf] = child

    @classmethod
    @public_api
    def from_file(cls, path: Path | str) -> 'KiCadSchematic':
        """Load schematic (and any sub-sheets) from file."""
        return cls(Path(path))

    @classmethod
    def from_text(cls, text: str) -> 'KiCadSchematic':
        """Parse schematic from text."""
        sexp = parse_sexp(text)
        return cls.from_sexp(sexp)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'KiCadSchematic':
        """Parse from S-expression list."""
        from .kicad_lib_symbol import LibSymbol
        from .kicad_sch_title_block import TitleBlock, PaperSize
        from .kicad_sch_symbol import SchSymbol
        from .kicad_sch_wire import SchWire, SchBus, SchBusEntry, SchBusAlias
        from .kicad_sch_junction import SchJunction
        from .kicad_sch_no_connect import SchNoConnect
        from .kicad_sch_label import SchLabel, SchGlobalLabel, SchHierarchicalLabel, SchNetclassFlag
        from .kicad_sch_sheet import SchSheet
        from .kicad_sch_text import SchText
        from .kicad_sch_text_box import SchTextBox
        from .kicad_sch_shapes import SchPolyline, SchRectangle, SchArc, SchCircle, SchBezier
        from .kicad_sch_group import SchGroup
        from .kicad_sch_image import SchImage
        from .kicad_sch_rule_area import SchRuleArea
        from .kicad_sch_table import SchTable

        version = int(get_value(sexp, 'version', KICAD_SCHEMATIC_FILE_VERSION))
        generator = unquote_string(get_value(sexp, 'generator', KICAD_SCHEMATIC_GENERATOR))
        generator_version = unquote_string(get_value(sexp, 'generator_version', KICAD_GENERATOR_VERSION))
        uuid = unquote_string(get_value(sexp, 'uuid', ''))

        # Paper size
        paper_elem = find_element(sexp, 'paper')
        paper = PaperSize.from_sexp(paper_elem) if paper_elem else PaperSize()

        # Title block
        title_block_elem = find_element(sexp, 'title_block')
        title_block = TitleBlock.from_sexp(title_block_elem) if title_block_elem else None

        # lib_symbols section
        lib_symbols = []
        lib_symbols_elem = find_element(sexp, 'lib_symbols')
        if lib_symbols_elem:
            for sym_elem in find_all_elements(lib_symbols_elem, 'symbol'):
                lib_symbols.append(LibSymbol.from_sexp(sym_elem))

        # Placed symbols
        symbols = [SchSymbol.from_sexp(e) for e in find_all_elements(sexp, 'symbol')]

        # Wires and connectivity
        wires = [SchWire.from_sexp(e) for e in find_all_elements(sexp, 'wire')]
        buses = [SchBus.from_sexp(e) for e in find_all_elements(sexp, 'bus')]
        bus_entries = [SchBusEntry.from_sexp(e) for e in find_all_elements(sexp, 'bus_entry')]
        bus_aliases = [SchBusAlias.from_sexp(e) for e in find_all_elements(sexp, 'bus_alias')]
        junctions = [SchJunction.from_sexp(e) for e in find_all_elements(sexp, 'junction')]
        no_connects = [SchNoConnect.from_sexp(e) for e in find_all_elements(sexp, 'no_connect')]

        # Labels
        labels = [SchLabel.from_sexp(e) for e in find_all_elements(sexp, 'label')]
        global_labels = [SchGlobalLabel.from_sexp(e) for e in find_all_elements(sexp, 'global_label')]
        hierarchical_labels = [SchHierarchicalLabel.from_sexp(e) for e in find_all_elements(sexp, 'hierarchical_label')]
        netclass_flags = [SchNetclassFlag.from_sexp(e) for e in find_all_elements(sexp, 'netclass_flag')]

        # Top-level text annotations (SCH_TEXT_T, distinct from SCH_TEXTBOX_T
        # `text_box` and from labels). saveText in
        # eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr.cpp:1431.
        texts = [SchText.from_sexp(e) for e in find_all_elements(sexp, 'text')]
        text_boxes = [SchTextBox.from_sexp(e) for e in find_all_elements(sexp, 'text_box')]

        # Top-level graphic shapes drawn on the notes layer (saveShape in
        # eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr.cpp:1332).
        polylines = [SchPolyline.from_sexp(e) for e in find_all_elements(sexp, 'polyline')]
        rectangles = [SchRectangle.from_sexp(e) for e in find_all_elements(sexp, 'rectangle')]
        arcs = [SchArc.from_sexp(e) for e in find_all_elements(sexp, 'arc')]
        circles = [SchCircle.from_sexp(e) for e in find_all_elements(sexp, 'circle')]
        beziers = [SchBezier.from_sexp(e) for e in find_all_elements(sexp, 'bezier')]

        # Top-level grouping annotations (saveGroup,
        # eeschema/sch_io/kicad_sexpr/sch_io_kicad_sexpr.cpp:1656).
        groups = [SchGroup.from_sexp(e) for e in find_all_elements(sexp, 'group')]

        # Embedded bitmap images (saveBitmap, sch_io_kicad_sexpr.cpp:1035).
        images = [SchImage.from_sexp(e) for e in find_all_elements(sexp, 'image')]

        # Rule areas (saveRuleArea, sch_io_kicad_sexpr.cpp:1373).
        rule_areas = [SchRuleArea.from_sexp(e) for e in find_all_elements(sexp, 'rule_area')]

        # Tables (saveTable, sch_io_kicad_sexpr.cpp:1549).
        tables = [SchTable.from_sexp(e) for e in find_all_elements(sexp, 'table')]

        # Sheets
        sheets = [SchSheet.from_sexp(e) for e in find_all_elements(sexp, 'sheet')]

        # Instance sections
        sheet_instances = []
        si_elem = find_element(sexp, 'sheet_instances')
        if si_elem:
            for path_elem in find_all_elements(si_elem, 'path'):
                sheet_instances.append(SheetInstancePath.from_sexp(path_elem))

        symbol_instances = []
        symi_elem = find_element(sexp, 'symbol_instances')
        if symi_elem:
            for path_elem in find_all_elements(symi_elem, 'path'):
                symbol_instances.append(SymbolInstancePath.from_sexp(path_elem))

        # Embedded fonts flag
        embedded_fonts_val = get_value(sexp, 'embedded_fonts', 'no')
        embedded_fonts = embedded_fonts_val == 'yes'

        # Embedded files (worksheet, fonts, models, …)
        embedded_files: list[EmbeddedFile] = []
        ef_elem = find_element(sexp, 'embedded_files')
        if ef_elem:
            for file_elem in find_all_elements(ef_elem, 'file'):
                embedded_files.append(EmbeddedFile.from_sexp(file_elem))

        sch = cls()
        sch.version = version
        sch.generator = generator
        sch.generator_version = generator_version
        sch.uuid = uuid
        sch.paper = paper
        sch.title_block = title_block
        sch.lib_symbols = lib_symbols
        sch.symbols = symbols
        sch.wires = wires
        sch.buses = buses
        sch.bus_entries = bus_entries
        sch.bus_aliases = bus_aliases
        sch.junctions = junctions
        sch.no_connects = no_connects
        sch.labels = labels
        sch.global_labels = global_labels
        sch.hierarchical_labels = hierarchical_labels
        sch.netclass_flags = netclass_flags
        sch.texts = texts
        sch.text_boxes = text_boxes
        sch.polylines = polylines
        sch.rectangles = rectangles
        sch.arcs = arcs
        sch.circles = circles
        sch.beziers = beziers
        sch.groups = groups
        sch.images = images
        sch.tables = tables
        sch.rule_areas = rule_areas
        sch.sheets = sheets
        sch.sheet_instances = sheet_instances
        sch.symbol_instances = symbol_instances
        sch.embedded_fonts = embedded_fonts
        sch.embedded_files = embedded_files
        sch._raw_sexp = sexp
        return sch

    def embed_worksheet(self, path: Path | str) -> EmbeddedFile:
        """Embed a ``.wks`` drawing sheet into this schematic.

        Packs the worksheet via :meth:`EmbeddedFile.from_worksheet`, appends it
        to :attr:`embedded_files`, and flips :attr:`embedded_fonts` on (KiCad
        sets the flag once any file is embedded). Returns the embedded entry so
        callers can wire its ``kicad-embed://`` reference into the project.
        """
        embedded = EmbeddedFile.from_worksheet(path)
        self.embedded_files.append(embedded)
        self.embedded_fonts = True
        return embedded

    @public_api
    def save(self, path: Path | str) -> None:
        """Save schematic to file. Canonical save method per ADR-0043."""
        path = Path(path)
        text = self.to_text()
        path.write_text(text, encoding='utf-8')

    def to_file(self, path: Path | str) -> None:
        """Deprecated: use ``save()``."""
        self.save(path)

    @public_api
    def to_svg(self, **kwargs) -> str:
        """Render schematic to SVG through the plotter-IR pipeline."""
        from .kicad_sch_svg import render_schematic_svg

        return render_schematic_svg(self, **kwargs)

    @public_api
    def to_ir(self, **kwargs):
        """Render schematic to plotter IR."""
        from .kicad_schematic_to_ir import schematic_to_ir

        return schematic_to_ir(self, **kwargs)

    def to_text(self) -> str:
        """Serialize to formatted S-expression text."""
        from .kicad_sch_image import format_image_data_blocks

        sexp = self.to_sexp()
        raw = build_sexp(sexp)
        formatted = format_sexp(raw, indentation_size=2, max_nesting=2)
        # Image (data ...) blocks sit at depth 3 so format_sexp keeps
        # them inline. Break each base64 chunk onto its own line to
        # match KiCad's FormatStreamData output.
        return format_image_data_blocks(formatted)

    def to_sexp(self) -> list:
        """Serialize to S-expression list."""
        result = [
            'kicad_sch',
            ['version', self.version],
            ['generator', QuotedString(self.generator)],
            ['generator_version', QuotedString(self.generator_version)],
            ['uuid', QuotedString(self.uuid)],
            self.paper.to_sexp()
        ]

        if self.title_block:
            result.append(self.title_block.to_sexp())

        # lib_symbols — KiCad always emits this block, even when empty.
        # See ``CADSTAR_SCH_ARCHIVE_LOADER`` / ``SCH_IO_KICAD_SEXPR_PARSER``: the
        # block is mandatory on round-trip; dropping it is data-loss.
        _append_schematic_lib_symbols(result, self.lib_symbols)

        # Bus aliases
        _append_serialized_items(result, self.bus_aliases)

        # Junctions (placed before wires in KiCad output)
        _append_serialized_items(result, self.junctions)

        # No connects
        _append_serialized_items(result, self.no_connects)

        # Wires
        _append_serialized_items(result, self.wires)

        # Buses
        _append_serialized_items(result, self.buses)

        # Bus entries
        _append_serialized_items(result, self.bus_entries)

        # Labels
        _append_serialized_items(result, self.labels)
        _append_serialized_items(result, self.global_labels)
        _append_serialized_items(result, self.hierarchical_labels)
        _append_serialized_items(result, self.netclass_flags)

        # Rule areas (saveRuleArea, sch_io_kicad_sexpr.cpp:1373)
        _append_serialized_items(result, self.rule_areas)

        # Graphics
        _append_serialized_items(result, self.texts)
        _append_serialized_items(result, self.text_boxes)
        _append_serialized_items(result, self.polylines)
        _append_serialized_items(result, self.arcs)
        _append_serialized_items(result, self.circles)
        _append_serialized_items(result, self.rectangles)
        _append_serialized_items(result, self.beziers)
        _append_serialized_items(result, self.images)
        _append_serialized_items(result, self.tables)

        # Groups (saveGroup, sch_io_kicad_sexpr.cpp:1656)
        _append_serialized_items(result, self.groups)

        # Sheets
        _append_serialized_items(result, self.sheets)

        # Symbols (placed after sheets in KiCad output)
        _append_serialized_items(result, self.symbols)

        # Instance sections
        _append_serialized_group_if_present(
            result,
            'sheet_instances',
            self.sheet_instances,
        )
        _append_serialized_group_if_present(
            result,
            'symbol_instances',
            self.symbol_instances,
        )

        # Embedded fonts
        result.append(['embedded_fonts', 'yes' if self.embedded_fonts else 'no'])

        # Embedded files (worksheet/fonts/models) — KiCad emits these after fonts.
        if self.embedded_files:
            block: list[Any] = ['embedded_files']
            block.extend(ef.to_sexp() for ef in self.embedded_files)
            result.append(block)

        return result

    # Convenience methods
    @public_api
    def get_symbol_by_reference(self, ref: str) -> Optional['SchSymbol']:
        """Get placed symbol by reference designator."""
        for sym in self.symbols:
            if sym.reference == ref:
                return sym
        return None

    @public_api
    def get_lib_symbol(self, lib_id: str) -> Optional['LibSymbol']:
        """Get library symbol definition by lib_id."""
        for sym in self.lib_symbols:
            if sym.name == lib_id or sym.name.endswith(f":{lib_id}"):
                return sym
            # Also check without library prefix
            if ':' in lib_id:
                _, name = lib_id.rsplit(':', 1)
                if sym.name == name or sym.name.endswith(f":{name}"):
                    return sym
        return None

    @public_api
    def get_lib_symbol_for_symbol(self, symbol: 'SchSymbol') -> Optional['LibSymbol']:
        """Get the library symbol definition for a placed symbol instance."""
        lib_name = getattr(symbol, "lib_name", "") or ""
        if lib_name:
            lib_sym = self.get_lib_symbol(lib_name)
            if lib_sym is not None:
                return lib_sym
        lib_id = getattr(symbol, "lib_id", "") or ""
        return self.get_lib_symbol(lib_id) if lib_id else None

    @public_api
    def get_symbols_by_lib_id(self, lib_id: str) -> List['SchSymbol']:
        """Get all placed symbols using a given lib_id."""
        return [s for s in self.symbols if s.lib_id == lib_id]

    @public_api
    def get_sheet_by_name(self, name: str) -> Optional['SchSheet']:
        """Get hierarchical sheet by name."""
        for sheet in self.sheets:
            if sheet.sheet_name == name:
                return sheet
        return None

    @public_api
    def iter_objects(self) -> Iterator[object]:
        """Iterate over top-level schematic-owned objects."""
        for list_name in _SCHEMATIC_OBJECT_LIST_NAMES:
            yield from getattr(self, list_name, ())

    @public_api
    def add_object(self, obj: object) -> object:
        """Add a typed object to the matching schematic-owned list."""
        list_name = _SCHEMATIC_OBJECT_LIST_BY_CLASS_NAME.get(type(obj).__name__)
        if list_name is None:
            raise TypeError(f"unsupported schematic object type: {type(obj).__name__}")
        getattr(self, list_name).append(obj)
        return obj

    @public_api
    def remove_object(self, obj: object) -> bool:
        """Remove an object by identity from its owning schematic list."""
        for list_name in _SCHEMATIC_OBJECT_LIST_NAMES:
            collection: list = getattr(self, list_name)
            for index, candidate in enumerate(collection):
                if candidate is obj:
                    del collection[index]
                    return True
        return False

    @public_api
    def iter_properties(self) -> Iterator[object]:
        """Iterate over properties attached to schematic-owned objects."""
        for obj in self.iter_objects():
            iter_props = getattr(obj, "iter_properties", None)
            if callable(iter_props):
                yield from cast(Callable[[], Iterable[object]], iter_props)()
                continue
            props = getattr(obj, "properties", ()) or ()
            yield from cast(Iterable[object], props)

    @public_api
    @property
    def objects(self):
        """Live read-only query view over schematic-owned objects."""
        from .kicad_object_collection import KiCadObjectCollection

        return KiCadObjectCollection(lambda: self.iter_objects(), owner=self)

    @public_api
    @property
    def properties(self):
        """Live read-only query view over schematic-owned properties."""
        from .kicad_object_collection import KiCadObjectCollection

        return KiCadObjectCollection(lambda: self.iter_properties(), owner=self)

    def __iter__(self) -> Iterator['SchSymbol']:
        """Iterate over placed symbols."""
        return iter(self.symbols)

    def __len__(self) -> int:
        """Number of placed symbols."""
        return len(self.symbols)

    # ------------------------------------------------------------------
    # Hierarchical traversal
    # ------------------------------------------------------------------

    @public_api
    def walk_symbols(
        self, _sheet_path: str = "",
        *,
        include_off_board_sheets: bool = True,
    ) -> Iterator[tuple['SchSymbol', str, 'KiCadSchematic']]:
        """Yield every placed symbol across the full sheet hierarchy.

        Each yielded triple is ``(symbol, sheet_path, owning_schematic)``:

        - ``symbol`` is the :class:`SchSymbol` instance.
        - ``sheet_path`` is the hierarchical UUID prefix that prefixes
          this symbol's instance path. The top-level schematic uses
          ``"/<top_uuid>"``; each level of nesting appends
          ``"/<sheet_uuid>"`` (the parent sheet placeholder's UUID),
          mirroring the structure KiCad emits in ``(instances ...)``.
        - ``owning_schematic`` is the :class:`KiCadSchematic` the symbol
          was parsed from, useful for resolving lib_symbols overrides
          local to that sheet.

        Order: top-level symbols first (in placement order), then each
        sub-sheet recursively in sheet declaration order.
        """
        prefix = _sheet_path or ("/" + self.uuid if self.uuid else "")
        for sym in self.symbols:
            yield (sym, prefix, self)
        for sheet in self.sheets:
            if not include_off_board_sheets and not getattr(sheet, "on_board", True):
                continue
            child = self.sub_schematics.get(sheet.sheet_file)
            if child is None:
                continue
            child_prefix = prefix + "/" + sheet.uuid if sheet.uuid else prefix
            yield from child.walk_symbols(
                _sheet_path=child_prefix,
                include_off_board_sheets=include_off_board_sheets,
            )

    @public_api
    def walk_sheets(self) -> Iterator[tuple['SchSheet', 'KiCadSchematic']]:
        """Yield ``(sheet, child_schematic)`` for every sheet in the hierarchy.

        Sheets whose ``Sheetfile`` could not be resolved (missing on
        disk, cycle, etc.) are still yielded with ``child_schematic``
        set to the cached loaded schematic if any, otherwise skipped.
        Top-level sheets are yielded before their descendants.
        """
        for sheet in self.sheets:
            child = self.sub_schematics.get(sheet.sheet_file)
            if child is None:
                continue
            yield (sheet, child)
            yield from child.walk_sheets()

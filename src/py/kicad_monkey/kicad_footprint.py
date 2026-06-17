"""
KiCad Footprint File Parser

A Python parser for standalone KiCad footprint files (.kicad_mod) with 100% round-trip fidelity.

KiCad Source Reference:
    Version: 9.0.0-rc3-4364-g5f555f4d63
    Commit: 5f555f4d63b970e410d567d1f79e05e8ce41b9d8
    Date: 2025-11-27
    Source: https://gitlab.com/kicad/code/kicad
    File format docs: https://dev-docs.kicad.org/en/file-formats/sexpr-footprint/
    Key files referenced:
    - pcbnew/pcb_io/kicad_sexpr/pcb_io_kicad_sexpr.cpp - Footprint file I/O

This module provides:
- `KiCadFootprint`: Main class representing a complete .kicad_mod file
- `from_kicad_mod()`: Parse a .kicad_mod file into Python objects
- `to_kicad_mod()`: Serialize Python objects back to .kicad_mod format

The parser preserves all formatting details including:
- Indentation (tabs)
- Number precision
- Quoted vs bare strings
- Base64 embedded data with proper line wrapping
- Order of elements

Key Differences from PCB-embedded footprints:
- Standalone files have their own version/generator headers
- No (at x y) position - footprint is at origin
- Pads have no net information
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterator, List, Optional, Union, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox
    from .kicad_object_collection import KiCadObjectCollection
    from .kicad_plotter_ir import KiCadPlotterDocument
    from .kicad_sch_svg_renderer import KiCadSvgRenderOptions
from ._api_markers import public_api
from .kicad_defaults import (
    KICAD_FOOTPRINT_FILE_VERSION,
    KICAD_FOOTPRINT_GENERATOR,
    KICAD_GENERATOR_VERSION,
)
from .kicad_sexpr import parse_sexp, QuotedString

from .kicad_base import (
    FRONT_COPPER_LAYER,
    find_element,
    find_all_elements,
    get_value,
    has_flag,
    unquote_string,
)

from .kicad_pcb_sexp import SexpWriter

from .kicad_pcb_footprint import (
    Pad,
    FpText,
    FpTextBox,
    Property,
    FpLine,
    FpArc,
    FpCircle,
    FpRect,
    FpPoly,
    Model,
    EmbeddedFile,
)

from .kicad_pcb_zone import Zone
from .kicad_pcb_other import PadNameGroup


_FOOTPRINT_OBJECT_LIST_NAMES: tuple[str, ...] = (
    "properties",
    "fp_texts",
    "fp_text_boxes",
    "fp_lines",
    "fp_arcs",
    "fp_circles",
    "fp_rects",
    "fp_polys",
    "pads",
    "zones",
    "net_tie_pad_groups",
    "jumper_pad_groups",
    "models",
    "embedded_files",
)

_FOOTPRINT_OBJECT_LIST_BY_CLASS_NAME: dict[str, str] = {
    "Property": "properties",
    "FpText": "fp_texts",
    "FpTextBox": "fp_text_boxes",
    "FpLine": "fp_lines",
    "FpArc": "fp_arcs",
    "FpCircle": "fp_circles",
    "FpRect": "fp_rects",
    "FpPoly": "fp_polys",
    "Pad": "pads",
    "Zone": "zones",
    "PadNameGroup": "net_tie_pad_groups",
    "Model": "models",
    "EmbeddedFile": "embedded_files",
}


@public_api
class KiCadFootprint:
    """
    Standalone KiCad footprint file (.kicad_mod).

    Supports full round-trip: file -> object -> file with 100% fidelity.

    Example:
        >>> fp = KiCadFootprint("R0603.kicad_mod")
        >>> fp.name
        'R0603'
        >>> len(fp.pads)
        2
        >>> fp.save("output.kicad_mod")
    """

    def __init__(self, path: Union[str, Path, None] = None) -> None:
        """Create a KiCadFootprint.

        Args:
            path: Path to .kicad_mod file to parse.
                  If None, creates an empty footprint.
        """
        # File metadata
        self.version: int = KICAD_FOOTPRINT_FILE_VERSION
        self.generator: str = KICAD_FOOTPRINT_GENERATOR
        self.generator_version: str = KICAD_GENERATOR_VERSION

        # Core footprint identity
        self.name: str = ""
        self.layer: str = FRONT_COPPER_LAYER
        self.locked: bool = False
        self.placed: bool = False
        self.uuid = None
        self.descr = None
        self.tags = None

        # Attributes
        self.attr: list = []

        # Properties
        self.properties: list = []

        # Graphical elements
        self.fp_lines: list = []
        self.fp_arcs: list = []
        self.fp_circles: list = []
        self.fp_rects: list = []
        self.fp_polys: list = []
        self.fp_texts: list = []
        self.fp_text_boxes: list = []

        # Pads
        self.pads: list = []

        # Zones
        self.zones: list = []

        # Pad group metadata
        self.net_tie_pad_groups: list = []
        self.duplicate_pad_numbers_are_jumpers: Optional[bool] = None
        self.jumper_pad_groups: list = []

        # 3D models
        self.models: list = []

        # Embedded files
        self.embedded_fonts: bool = False
        self.embedded_files: list = []

        # Design rules
        self.solder_mask_margin = None
        self.solder_paste_margin = None
        self.solder_paste_margin_ratio = None
        self.clearance = None
        self.zone_connect = None

        self._raw_sexp = None

        if path is not None:
            parsed = self.from_file(Path(path))
            self.__dict__.update(parsed.__dict__)

    @classmethod
    def from_file(cls, path: Union[str, Path]) -> 'KiCadFootprint':
        """Load a KiCad footprint file. Deprecated: use ``KiCadFootprint(path)``."""
        path = Path(path)
        content = path.read_text(encoding='utf-8')
        return cls.from_string(content)

    @classmethod
    def from_string(cls, content: str) -> 'KiCadFootprint':
        """Parse KiCad footprint content from string."""
        sexp = parse_sexp(content)
        return cls.from_sexp(sexp)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'KiCadFootprint':
        """Parse from s-expression list."""
        if sexp[0] != 'footprint':
            raise ValueError(f"Expected 'footprint', got '{sexp[0]}'")

        fp = cls()
        fp._raw_sexp = sexp

        # Name (first argument after 'footprint')
        fp.name = unquote_string(sexp[1])

        # Header metadata
        fp.version = int(get_value(sexp, 'version', KICAD_FOOTPRINT_FILE_VERSION))
        fp.generator = unquote_string(get_value(sexp, 'generator', KICAD_FOOTPRINT_GENERATOR))
        fp.generator_version = unquote_string(get_value(sexp, 'generator_version', KICAD_GENERATOR_VERSION))

        # Flags
        fp.locked = has_flag(sexp, 'locked') or get_value(sexp, 'locked') == 'yes'
        fp.placed = has_flag(sexp, 'placed') or get_value(sexp, 'placed') == 'yes'

        # Core identity
        fp.layer = unquote_string(get_value(sexp, 'layer', FRONT_COPPER_LAYER))
        fp.uuid = unquote_string(get_value(sexp, 'uuid'))
        fp.descr = unquote_string(get_value(sexp, 'descr'))
        fp.tags = unquote_string(get_value(sexp, 'tags'))

        # Attributes
        attr_elem = find_element(sexp, 'attr')
        fp.attr = list(attr_elem[1:]) if attr_elem else []

        # Properties
        fp.properties = [Property.from_sexp(p) for p in find_all_elements(sexp, 'property')]

        # Graphics
        fp.fp_lines = [FpLine.from_sexp(e) for e in find_all_elements(sexp, 'fp_line')]
        fp.fp_arcs = [FpArc.from_sexp(e) for e in find_all_elements(sexp, 'fp_arc')]
        fp.fp_circles = [FpCircle.from_sexp(e) for e in find_all_elements(sexp, 'fp_circle')]
        fp.fp_rects = [FpRect.from_sexp(e) for e in find_all_elements(sexp, 'fp_rect')]
        fp.fp_polys = [FpPoly.from_sexp(e) for e in find_all_elements(sexp, 'fp_poly')]
        fp.fp_texts = [FpText.from_sexp(e) for e in find_all_elements(sexp, 'fp_text')]
        fp.fp_text_boxes = [FpTextBox.from_sexp(e) for e in find_all_elements(sexp, 'fp_text_box')]

        # Pads
        fp.pads = [Pad.from_sexp(p) for p in find_all_elements(sexp, 'pad')]

        # Zones
        fp.zones = [Zone.from_sexp(z) for z in find_all_elements(sexp, 'zone')]

        net_tie_elem = find_element(sexp, "net_tie_pad_groups")
        if net_tie_elem:
            for token in net_tie_elem[1:]:
                group = PadNameGroup.from_net_tie_token(token)
                if group:
                    fp.net_tie_pad_groups.append(group)

        duplicate_jumpers_elem = find_element(sexp, "duplicate_pad_numbers_are_jumpers")
        if duplicate_jumpers_elem is not None:
            if len(duplicate_jumpers_elem) > 1:
                fp.duplicate_pad_numbers_are_jumpers = (
                    unquote_string(duplicate_jumpers_elem[1]).lower() in ("yes", "true", "1")
                )
            else:
                fp.duplicate_pad_numbers_are_jumpers = True

        jumper_groups_elem = find_element(sexp, "jumper_pad_groups")
        if jumper_groups_elem:
            for group_elem in jumper_groups_elem[1:]:
                if isinstance(group_elem, list):
                    group = PadNameGroup.from_jumper_group_sexp(group_elem)
                    if group:
                        fp.jumper_pad_groups.append(group)

        # 3D Models
        fp.models = [Model.from_sexp(m) for m in find_all_elements(sexp, 'model')]

        # Embedded files
        fp.embedded_fonts = get_value(sexp, 'embedded_fonts') == 'yes'

        embedded_files_elem = find_element(sexp, 'embedded_files')
        if embedded_files_elem:
            for file_elem in find_all_elements(embedded_files_elem, 'file'):
                fp.embedded_files.append(EmbeddedFile.from_sexp(file_elem))

        # Design rule overrides
        fp.solder_mask_margin = _get_float_value(sexp, 'solder_mask_margin')
        fp.solder_paste_margin = _get_float_value(sexp, 'solder_paste_margin')
        fp.solder_paste_margin_ratio = _get_float_value(sexp, 'solder_paste_margin_ratio')
        fp.clearance = _get_float_value(sexp, 'clearance')
        zone_connect_val = get_value(sexp, 'zone_connect')
        fp.zone_connect = int(zone_connect_val) if zone_connect_val is not None else None

        return fp

    def to_sexp(self) -> list:
        """
        Convert to s-expression list.

        Element order matches KiCad source (pcb_io_kicad_sexpr.cpp:1130-1448):
        1. version, generator, generator_version
        2. locked/placed flags
        3. layer
        4. uuid
        5. descr, tags
        6. property elements (Reference, Value first)
        7. attr
        8. Design rule overrides
        9. fp_text user elements
        10. Graphics (fp_line, fp_arc, fp_circle, fp_rect, fp_poly)
        11. pad elements
        12. zone elements
        13. embedded_fonts
        14. embedded_files
        15. model elements
        """
        result = ['footprint', QuotedString(self.name)]

        # 1. Header metadata
        result.append(['version', self.version])
        result.append(['generator', QuotedString(self.generator)])
        result.append(['generator_version', QuotedString(self.generator_version)])

        # 2. Flags
        if self.locked:
            result.append(['locked', 'yes'])
        if self.placed:
            result.append(['placed', 'yes'])

        # 3. Layer
        result.append(['layer', QuotedString(self.layer)])

        # 4. UUID
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])

        # 5. Description and tags
        if self.descr:
            result.append(['descr', QuotedString(self.descr)])

        if self.tags:
            result.append(['tags', QuotedString(self.tags)])

        # 6. Properties (Reference and Value first, then others)
        ref_prop = next((p for p in self.properties if p.name == 'Reference'), None)
        val_prop = next((p for p in self.properties if p.name == 'Value'), None)
        other_props = [p for p in self.properties if p.name not in ('Reference', 'Value')]

        if ref_prop:
            result.append(ref_prop.to_sexp())
        if val_prop:
            result.append(val_prop.to_sexp())
        for prop in other_props:
            result.append(prop.to_sexp())

        # 7. Attributes
        if self.attr:
            result.append(['attr'] + list(self.attr))

        if self.net_tie_pad_groups:
            result.append(
                ["net_tie_pad_groups"]
                + [group.to_net_tie_token() for group in self.net_tie_pad_groups]
            )
        if self.duplicate_pad_numbers_are_jumpers is not None:
            result.append(
                [
                    "duplicate_pad_numbers_are_jumpers",
                    "yes" if self.duplicate_pad_numbers_are_jumpers else "no",
                ]
            )
        if self.jumper_pad_groups:
            result.append(
                ["jumper_pad_groups"]
                + [group.to_jumper_group_sexp() for group in self.jumper_pad_groups]
            )

        # 8. Design rule overrides
        if self.solder_mask_margin is not None:
            result.append(['solder_mask_margin', self.solder_mask_margin])
        if self.solder_paste_margin is not None:
            result.append(['solder_paste_margin', self.solder_paste_margin])
        if self.solder_paste_margin_ratio is not None:
            result.append(['solder_paste_margin_ratio', self.solder_paste_margin_ratio])
        if self.clearance is not None:
            result.append(['clearance', self.clearance])
        if self.zone_connect is not None:
            result.append(['zone_connect', self.zone_connect])

        # 9. fp_text user elements
        for fp_text in self.fp_texts:
            result.append(fp_text.to_sexp())

        for fp_text_box in self.fp_text_boxes:
            result.append(fp_text_box.to_sexp())

        # 10. Graphics - drawing primitives
        for fp_line in self.fp_lines:
            result.append(fp_line.to_sexp())

        for fp_arc in self.fp_arcs:
            result.append(fp_arc.to_sexp())

        for fp_circle in self.fp_circles:
            result.append(fp_circle.to_sexp())

        for fp_rect in self.fp_rects:
            result.append(fp_rect.to_sexp())

        for fp_poly in self.fp_polys:
            result.append(fp_poly.to_sexp())

        # 11. Pads
        for pad in self.pads:
            result.append(pad.to_sexp())

        # 12. Zones
        for zone in self.zones:
            result.append(zone.to_sexp())

        # 13. Embedded fonts
        result.append(['embedded_fonts', 'yes' if self.embedded_fonts else 'no'])

        # 14. Embedded files
        if self.embedded_files:
            ef_elem = ['embedded_files']
            for ef in self.embedded_files:
                ef_elem.append(ef.to_sexp())
            result.append(ef_elem)

        # 15. 3D Models (at the end per KiCad source)
        for model in self.models:
            result.append(model.to_sexp())

        return result

    def to_string(self) -> str:
        """Serialize to KiCad footprint format string."""
        sexp = self.to_sexp()
        writer = SexpWriter()
        return writer.write(sexp)

    def save(self, path: Union[str, Path]) -> None:
        """Save to a KiCad footprint file. Canonical save method per ADR-0043."""
        path = Path(path)
        content = self.to_string()
        path.write_text(content, encoding='utf-8')

    def to_file(self, path: Union[str, Path]) -> None:
        """Deprecated: use ``save()``."""
        self.save(path)

    # Convenience properties
    @property
    def reference(self) -> Optional[str]:
        """Get the Reference property value."""
        return self.get_property("Reference")

    @property
    def value(self) -> Optional[str]:
        """Get the Value property value."""
        return self.get_property("Value")

    @public_api
    def to_ir(
        self,
        *,
        source_path: str | None = None,
        document_id: str | None = None,
    ) -> "KiCadPlotterDocument":
        """Render this footprint to plotter IR."""
        from .kicad_footprint_to_ir import footprint_to_ir

        return footprint_to_ir(self, source_path=source_path, document_id=document_id)

    @public_api
    def iter_objects(self) -> Iterator[object]:
        """Iterate over footprint-owned objects."""
        for list_name in _FOOTPRINT_OBJECT_LIST_NAMES:
            yield from getattr(self, list_name, ())

    @public_api
    def add_object(self, obj: object) -> object:
        """Add a typed object to the matching footprint-owned list."""
        list_name = _FOOTPRINT_OBJECT_LIST_BY_CLASS_NAME.get(type(obj).__name__)
        if list_name is None:
            raise TypeError(f"unsupported footprint object type: {type(obj).__name__}")
        getattr(self, list_name).append(obj)
        return obj

    @public_api
    def remove_object(self, obj: object) -> bool:
        """Remove an object by identity from its owning footprint list."""
        for list_name in _FOOTPRINT_OBJECT_LIST_NAMES:
            collection: list = getattr(self, list_name)
            for index, candidate in enumerate(collection):
                if candidate is obj:
                    del collection[index]
                    return True
        return False

    @public_api
    @property
    def objects(self) -> "KiCadObjectCollection":
        """Live read-only query view over footprint-owned objects."""
        from .kicad_object_collection import KiCadObjectCollection

        return KiCadObjectCollection(lambda: self.iter_objects(), owner=self)

    @public_api
    def get_property_object(self, name: str) -> Property | None:
        """Get a footprint property object by name."""
        name_text = str(name)
        for prop in self.properties:
            if prop.name == name_text:
                return prop
        return None

    @public_api
    def get_property(self, name: str) -> str | None:
        """Get a footprint property value by name."""
        prop = self.get_property_object(name)
        return prop.value if prop is not None else None

    @public_api
    def get_property_value(self, name: str, default: str = "") -> str:
        """Get a footprint property value, returning default when absent."""
        value = self.get_property(name)
        return value if value is not None else default

    @public_api
    def set_property_value(self, name: str, value: str, *, create: bool = False) -> bool:
        """Set a footprint property value. Returns True if updated or created."""
        prop = self.get_property_object(name)
        if prop is not None:
            prop.value = value
            return True
        if create:
            self.upsert_property(name, value)
            return True
        return False

    @public_api
    def upsert_property(self, name: str, value: str) -> Property:
        """Create or update a footprint property and return the property object."""
        prop = self.get_property_object(name)
        if prop is not None:
            prop.value = value
            return prop
        prop = Property(str(name), value)
        self.properties.append(prop)
        return prop

    @public_api
    def remove_property(self, name: str) -> bool:
        """Remove a footprint property by name."""
        name_text = str(name)
        for index, prop in enumerate(self.properties):
            if prop.name == name_text:
                del self.properties[index]
                return True
        return False

    @public_api
    def iter_properties(self) -> Iterator[Property]:
        """Iterate over footprint properties."""
        return iter(self.properties)

    def get_bounds(self) -> 'BoundingBox':
        """
        Compute the bounding box of the entire footprint.

        Includes all pads (all layers), text elements, and graphic elements.
        This matches KiCad's ComputeBoundingBox behavior for SVG viewBox..
        """
        from .kicad_geometry import BoundingBox

        bbox = BoundingBox()

        # Include all pads (regardless of layer)
        for pad in self.pads:
            bbox.merge(pad.get_bounds())

        # Include properties (Reference, Value, etc.)
        for prop in self.properties:
            bbox.merge(prop.get_bounds())

        # Include fp_text elements
        for fp_text in self.fp_texts:
            bbox.merge(fp_text.get_bounds())
        for fp_text_box in self.fp_text_boxes:
            bbox.merge(fp_text_box.get_bounds())

        # Include graphic elements
        for line in self.fp_lines:
            bbox.merge(line.get_bounds())
        for arc in self.fp_arcs:
            bbox.merge(arc.get_bounds())
        for circle in self.fp_circles:
            bbox.merge(circle.get_bounds())
        for rect in self.fp_rects:
            bbox.merge(rect.get_bounds())
        for poly in self.fp_polys:
            bbox.merge(poly.get_bounds())

        return bbox

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
        Render footprint to SVG format matching KiCad's output.

        Uses the plotter-IR pipeline under the hood..

        Args:
            layers: List of layer names to include (e.g., ["F.Cu", "F.SilkS"]).
                   If None, all layers are included.
            fill: Fill color for solid shapes (default: black)
            stroke: Stroke color for lines (default: black)
            black_and_white: If True, use black/white only (matches --black-and-white)
            profile: Optional SVG output profile, such as ``"enriched"`` or
                ``"oracle"``.
            options: Optional low-level SVG render options.

        Returns:
            Complete SVG document as string
        """
        from dataclasses import replace

        from .kicad_footprint_to_ir import footprint_to_ir
        from .kicad_ir_to_svg import render_ir_to_svg
        from .kicad_lib_symbol_to_ir import mm_to_nm
        from .kicad_sch_svg_renderer import (
            KiCadSvgRenderContext,
            KiCadSvgRenderOptions,
        )

        # Compute bounding box for entire footprint
        bbox = self.get_bounds()

        if bbox.is_empty:
            return self._empty_svg()

        base_opts = options if options is not None else KiCadSvgRenderOptions()
        opts = replace(
            base_opts,
            black_and_white=black_and_white,
            background_color=None,
            default_fill_color=fill,
            default_stroke_color=stroke,
            visible_layers=tuple(layers) if layers is not None else None,
            text_polyline_per_segment=True,
        )
        if profile is not None:
            from .kicad_sch_svg_renderer import KiCadSvgRenderProfile

            opts = replace(opts, profile=KiCadSvgRenderProfile(profile))
        ctx = KiCadSvgRenderContext(
            sheet_width_nm=mm_to_nm(bbox.width),
            sheet_height_nm=mm_to_nm(bbox.height),
            offset_x_nm=-mm_to_nm(bbox.min_x),
            offset_y_nm=-mm_to_nm(bbox.min_y),
            options=opts,
        )
        doc = footprint_to_ir(self, document_id=self.name)
        records = [
            replace(
                record,
                operations=[
                    op for op in record.operations
                    if str(op.payload.get("role", ""))
                    not in {"pad_drill", "npth_hole"}
                ],
            )
            for record in doc.records
        ]
        return render_ir_to_svg(replace(doc, records=records), ctx=ctx)

    def _empty_svg(self) -> str:
        """Return an empty SVG document."""
        return '''<?xml version="1.0" standalone="no"?>
 <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN"
 "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg
  xmlns:svg="http://www.w3.org/2000/svg"
  xmlns="http://www.w3.org/2000/svg"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  version="1.1" width="0mm" height="0mm" viewBox="0 0 0 0">
<title>Image</title>
</svg>
'''


def _get_float_value(sexp: list, name: str) -> Optional[float]:
    """Get a float value from sexp, or None if not present."""
    val = get_value(sexp, name)
    if val is not None:
        try:
            return float(val)
        except (ValueError, TypeError):
            pass
    return None


# =============================================================================
# Convenience Functions
# =============================================================================

def from_kicad_mod(path: Union[str, Path]) -> KiCadFootprint:
    """Load a KiCad footprint file into Python objects."""
    return KiCadFootprint.from_file(path)


def to_kicad_mod(footprint: KiCadFootprint, path: Union[str, Path]) -> None:
    """Save Python objects to a KiCad footprint file."""
    footprint.to_file(path)


# =============================================================================
# Exports
# =============================================================================

__all__ = [
    # Main class
    'KiCadFootprint',
    # Convenience functions
    'from_kicad_mod',
    'to_kicad_mod',
    # Re-export element classes for convenience
    'Pad',
    'FpText',
    'Property',
    'FpLine',
    'FpArc',
    'FpCircle',
    'FpRect',
    'FpPoly',
    'Model',
    'EmbeddedFile',
    'Zone',
]


# =============================================================================
# Main (for testing)
# =============================================================================

if __name__ == "__main__":
    import sys

    if len(sys.argv) > 1:
        path = sys.argv[1]
        print(f"Loading: {path}")
        fp = from_kicad_mod(path)
        print(f"Name: {fp.name}")
        print(f"Version: {fp.version}")
        print(f"Generator: {fp.generator}")
        print(f"Layer: {fp.layer}")
        print(f"Properties: {len(fp.properties)}")
        print(f"Pads: {len(fp.pads)}")
        print(f"Lines: {len(fp.fp_lines)}")
        print(f"Models: {len(fp.models)}")

        # Round-trip test
        output = fp.to_string()
        print(f"\nOutput length: {len(output)} chars")

        # Parse again and compare
        fp2 = KiCadFootprint.from_string(output)
        print(f"Round-trip name: {fp2.name}")
        print(f"Round-trip pads: {len(fp2.pads)}")

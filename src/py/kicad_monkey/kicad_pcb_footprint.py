"""
KiCad PCB Footprint - Footprint container and element re-exports

This file contains the Footprint class (PCB-embedded footprint) and re-exports
all footprint element classes for backward compatibility.

One class per file - element classes split into individual files.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterator, List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .kicad_geometry import BoundingBox, SvgRenderContext
    from .kicad_object_collection import KiCadObjectCollection
    from .kicad_pcb import KiCadPcb
    from .kicad_plotter_ir import KiCadPlotterDocument

from ._api_markers import public_api
from .kicad_sexpr import QuotedString
from .kicad_base import (
    EDGE_CUTS_LAYER,
    FRONT_COPPER_LAYER,
    find_element,
    find_all_elements,
    get_value,
    get_at,
    has_flag,
    unquote_string,
)
from .kicad_pcb_other import (
    Barcode,
    ComponentClassRef,
    Dimension,
    FootprintPlacement,
    FootprintVariant,
    FootprintVariantField,
    Group,
    Image,
    PadNameGroup,
    Table,
    UnknownElement,
)
from .kicad_pcb_zone import Zone

# Import element classes from individual files
from .kicad_pad import Pad
from .kicad_fp_text import FpText
from .kicad_fp_text_box import FpTextBox
from .kicad_property import Property
from .kicad_fp_line import FpLine
from .kicad_fp_arc import FpArc
from .kicad_fp_circle import FpCircle
from .kicad_fp_rect import FpRect
from .kicad_fp_poly import FpPoly
from .kicad_model import Model, EmbeddedFile


def _get_float_value(sexp: list, key: str) -> Optional[float]:
    """Get an optional float value from s-expression by key."""
    val = get_value(sexp, key)
    return float(val) if val is not None else None


_PCB_FOOTPRINT_OBJECT_LIST_NAMES: tuple[str, ...] = (
    "component_classes",
    "properties",
    "pads",
    "fp_texts",
    "fp_text_boxes",
    "fp_lines",
    "fp_arcs",
    "fp_circles",
    "fp_rects",
    "fp_polys",
    "images",
    "tables",
    "barcodes",
    "dimensions",
    "zones",
    "groups",
    "variants",
    "models",
    "embedded_files",
    "net_tie_pad_groups",
    "jumper_pad_groups",
    "unknown_elements",
)

_PCB_FOOTPRINT_OBJECT_LIST_BY_CLASS_NAME: dict[str, str] = {
    "ComponentClassRef": "component_classes",
    "Property": "properties",
    "Pad": "pads",
    "FpText": "fp_texts",
    "FpTextBox": "fp_text_boxes",
    "FpLine": "fp_lines",
    "FpArc": "fp_arcs",
    "FpCircle": "fp_circles",
    "FpRect": "fp_rects",
    "FpPoly": "fp_polys",
    "Image": "images",
    "Table": "tables",
    "Barcode": "barcodes",
    "Dimension": "dimensions",
    "Zone": "zones",
    "Group": "groups",
    "FootprintVariant": "variants",
    "Model": "models",
    "EmbeddedFile": "embedded_files",
    "PadNameGroup": "net_tie_pad_groups",
    "UnknownElement": "unknown_elements",
}

_FOOTPRINT_TO_SEXP_COLLECTIONS: tuple[str, ...] = (
    "fp_texts",
    "fp_text_boxes",
    "images",
    "tables",
    "barcodes",
    "dimensions",
    "fp_lines",
    "fp_arcs",
    "fp_circles",
    "fp_rects",
    "fp_polys",
    "zones",
    "groups",
    "variants",
    "pads",
    "models",
)


def _append_object_sexps(result: list, objects: list) -> None:
    for obj in objects:
        result.append(obj.to_sexp())


def _append_footprint_identity(result: list, footprint: "Footprint") -> None:
    if footprint.uuid:
        result.append(['uuid', QuotedString(footprint.uuid)])

    # KiCad's reader requires the angle slot even when zero (drift inventory #1).
    result.append(['at', footprint.at_x, footprint.at_y, footprint.at_angle])

    if footprint.locked:
        result.append(['locked', 'yes'])
    if footprint.descr:
        result.append(['descr', QuotedString(footprint.descr)])
    if footprint.tags:
        result.append(['tags', QuotedString(footprint.tags)])


def _append_footprint_properties(result: list, footprint: "Footprint") -> None:
    for prop in footprint.placement.to_sexp_elements():
        result.append(prop)
    _append_object_sexps(result, footprint.properties)

    if footprint.component_classes:
        result.append(
            ["component_classes"]
            + [component_class.to_sexp() for component_class in footprint.component_classes]
        )
    if footprint.attr:
        result.append(['attr'] + footprint.attr)


def _append_footprint_pad_groups(result: list, footprint: "Footprint") -> None:
    if footprint.net_tie_pad_groups:
        result.append(
            ["net_tie_pad_groups"]
            + [group.to_net_tie_token() for group in footprint.net_tie_pad_groups]
        )
    if footprint.duplicate_pad_numbers_are_jumpers is not None:
        result.append(
            [
                "duplicate_pad_numbers_are_jumpers",
                "yes" if footprint.duplicate_pad_numbers_are_jumpers else "no",
            ]
        )
    if footprint.jumper_pad_groups:
        result.append(
            ["jumper_pad_groups"]
            + [group.to_jumper_group_sexp() for group in footprint.jumper_pad_groups]
        )


def _append_footprint_optional_settings(result: list, footprint: "Footprint") -> None:
    for key, value in (
        ("solder_mask_margin", footprint.solder_mask_margin),
        ("solder_paste_margin", footprint.solder_paste_margin),
        ("solder_paste_margin_ratio", footprint.solder_paste_margin_ratio),
        ("clearance", footprint.clearance),
        ("zone_connect", footprint.zone_connect),
    ):
        if value is not None:
            result.append([key, value])


def _append_footprint_embedded_files(result: list, footprint: "Footprint") -> None:
    result.append(['embedded_fonts', 'yes' if footprint.embedded_fonts else 'no'])

    if footprint.embedded_files:
        ef_elem: list = ['embedded_files']
        _append_object_sexps(ef_elem, footprint.embedded_files)
        result.append(ef_elem)


@public_api
@dataclass
class Footprint:
    """Footprint (component) element embedded in a PCB."""
    library_link: str
    layer: str = FRONT_COPPER_LAYER
    at_x: float = 0.0
    at_y: float = 0.0
    at_angle: float = 0.0
    locked: bool = False
    placement: FootprintPlacement = field(default_factory=FootprintPlacement)
    uuid: Optional[str] = None
    descr: Optional[str] = None
    tags: Optional[str] = None
    attr: List[str] = field(default_factory=list)
    component_classes: List[ComponentClassRef] = field(default_factory=list)
    properties: List[Property] = field(default_factory=list)
    pads: List[Pad] = field(default_factory=list)
    fp_lines: List[FpLine] = field(default_factory=list)
    fp_arcs: List[FpArc] = field(default_factory=list)
    fp_circles: List[FpCircle] = field(default_factory=list)
    fp_rects: List[FpRect] = field(default_factory=list)
    fp_polys: List[FpPoly] = field(default_factory=list)
    fp_texts: List[FpText] = field(default_factory=list)
    fp_text_boxes: List[FpTextBox] = field(default_factory=list)
    images: List[Image] = field(default_factory=list)
    tables: List[Table] = field(default_factory=list)
    barcodes: List[Barcode] = field(default_factory=list)
    dimensions: List[Dimension] = field(default_factory=list)
    zones: List[Zone] = field(default_factory=list)
    groups: List[Group] = field(default_factory=list)
    variants: List[FootprintVariant] = field(default_factory=list)
    models: List[Model] = field(default_factory=list)
    embedded_fonts: bool = False
    embedded_files: List[EmbeddedFile] = field(default_factory=list)
    net_tie_pad_groups: List[PadNameGroup] = field(default_factory=list)
    duplicate_pad_numbers_are_jumpers: Optional[bool] = None
    jumper_pad_groups: List[PadNameGroup] = field(default_factory=list)
    solder_mask_margin: Optional[float] = None
    solder_paste_margin: Optional[float] = None
    solder_paste_margin_ratio: Optional[float] = None
    clearance: Optional[float] = None
    zone_connect: Optional[int] = None
    unknown_elements: List[UnknownElement] = field(default_factory=list)
    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'Footprint':
        library_link = unquote_string(sexp[1])
        layer = unquote_string(get_value(sexp, 'layer', FRONT_COPPER_LAYER))
        x, y, angle = get_at(sexp)
        locked = has_flag(sexp, 'locked') or get_value(sexp, 'locked') == 'yes'
        placement = FootprintPlacement.from_footprint_sexp(sexp)
        uuid = unquote_string(get_value(sexp, 'uuid'))
        descr = unquote_string(get_value(sexp, 'descr'))
        tags = unquote_string(get_value(sexp, 'tags'))

        attr_elem = find_element(sexp, 'attr')
        attr = list(attr_elem[1:]) if attr_elem else []

        component_classes: List[ComponentClassRef] = []
        component_classes_elem = find_element(sexp, "component_classes")
        if component_classes_elem:
            component_classes = [
                ComponentClassRef.from_sexp(class_elem)
                for class_elem in find_all_elements(component_classes_elem, "class")
            ]

        properties = [Property.from_sexp(p) for p in find_all_elements(sexp, 'property')]
        pads = [Pad.from_sexp(p) for p in find_all_elements(sexp, 'pad')]
        fp_lines = [FpLine.from_sexp(elem) for elem in find_all_elements(sexp, 'fp_line')]
        fp_arcs = [FpArc.from_sexp(a) for a in find_all_elements(sexp, 'fp_arc')]
        fp_circles = [FpCircle.from_sexp(c) for c in find_all_elements(sexp, 'fp_circle')]
        fp_rects = [FpRect.from_sexp(r) for r in find_all_elements(sexp, 'fp_rect')]
        fp_polys = [FpPoly.from_sexp(p) for p in find_all_elements(sexp, 'fp_poly')]
        fp_texts = [FpText.from_sexp(t) for t in find_all_elements(sexp, 'fp_text')]
        fp_text_boxes = [FpTextBox.from_sexp(t) for t in find_all_elements(sexp, 'fp_text_box')]
        images = [Image.from_sexp(i) for i in find_all_elements(sexp, 'image')]
        tables = [Table.from_sexp(t) for t in find_all_elements(sexp, 'table')]
        barcodes = [Barcode.from_sexp(b) for b in find_all_elements(sexp, 'barcode')]
        dimensions = [Dimension.from_sexp(d) for d in find_all_elements(sexp, 'dimension')]
        zones = [Zone.from_sexp(z) for z in find_all_elements(sexp, 'zone')]
        groups = [Group.from_sexp(g) for g in find_all_elements(sexp, 'group')]
        variants = [FootprintVariant.from_sexp(v) for v in find_all_elements(sexp, 'variant')]
        models = [Model.from_sexp(m) for m in find_all_elements(sexp, 'model')]

        embedded_fonts = get_value(sexp, 'embedded_fonts') == 'yes'

        embedded_files_elem = find_element(sexp, 'embedded_files')
        embedded_files = []
        if embedded_files_elem:
            for file_elem in find_all_elements(embedded_files_elem, 'file'):
                embedded_files.append(EmbeddedFile.from_sexp(file_elem))

        net_tie_pad_groups: List[PadNameGroup] = []
        net_tie_elem = find_element(sexp, "net_tie_pad_groups")
        if net_tie_elem:
            for token in net_tie_elem[1:]:
                group = PadNameGroup.from_net_tie_token(token)
                if group:
                    net_tie_pad_groups.append(group)

        duplicate_pad_numbers_are_jumpers = None
        duplicate_jumpers_elem = find_element(sexp, "duplicate_pad_numbers_are_jumpers")
        if duplicate_jumpers_elem is not None:
            if len(duplicate_jumpers_elem) > 1:
                duplicate_pad_numbers_are_jumpers = (
                    unquote_string(duplicate_jumpers_elem[1]).lower() in ("yes", "true", "1")
                )
            else:
                duplicate_pad_numbers_are_jumpers = True

        jumper_pad_groups: List[PadNameGroup] = []
        jumper_groups_elem = find_element(sexp, "jumper_pad_groups")
        if jumper_groups_elem:
            for group_elem in jumper_groups_elem[1:]:
                if isinstance(group_elem, list):
                    group = PadNameGroup.from_jumper_group_sexp(group_elem)
                    if group:
                        jumper_pad_groups.append(group)

        solder_mask_margin = _get_float_value(sexp, "solder_mask_margin")
        solder_paste_margin = _get_float_value(sexp, "solder_paste_margin")
        solder_paste_margin_ratio = _get_float_value(sexp, "solder_paste_margin_ratio")
        clearance = _get_float_value(sexp, "clearance")
        zone_connect_val = get_value(sexp, "zone_connect")
        zone_connect = int(zone_connect_val) if zone_connect_val is not None else None

        known_elements = {
            "footprint", "layer", "at", "locked", "uuid", "descr", "tags", "attr",
            "path", "sheetname", "sheetfile", "component_classes",
            "property", "pad", "fp_line", "fp_arc", "fp_circle", "fp_rect", "fp_poly",
            "fp_text", "fp_text_box", "image", "table", "barcode", "dimension", "zone", "group",
            "variant", "model", "embedded_fonts", "embedded_files", "net_tie_pad_groups",
            "duplicate_pad_numbers_are_jumpers", "jumper_pad_groups", "solder_mask_margin",
            "solder_paste_margin", "solder_paste_margin_ratio", "clearance", "zone_connect",
        }
        unknown_elements = [
            UnknownElement(name=elem[0], raw_sexp=elem)
            for elem in sexp[1:]
            if isinstance(elem, list) and len(elem) > 0 and elem[0] not in known_elements
        ]

        return cls(
            library_link=library_link,
            layer=layer,
            at_x=x, at_y=y, at_angle=angle,
            locked=locked,
            placement=placement,
            uuid=uuid, descr=descr, tags=tags,
            attr=attr,
            component_classes=component_classes,
            properties=properties,
            pads=pads,
            fp_lines=fp_lines,
            fp_arcs=fp_arcs,
            fp_circles=fp_circles,
            fp_rects=fp_rects,
            fp_polys=fp_polys,
            fp_texts=fp_texts,
            fp_text_boxes=fp_text_boxes,
            images=images,
            tables=tables,
            barcodes=barcodes,
            dimensions=dimensions,
            zones=zones,
            groups=groups,
            variants=variants,
            models=models,
            embedded_fonts=embedded_fonts,
            embedded_files=embedded_files,
            net_tie_pad_groups=net_tie_pad_groups,
            duplicate_pad_numbers_are_jumpers=duplicate_pad_numbers_are_jumpers,
            jumper_pad_groups=jumper_pad_groups,
            solder_mask_margin=solder_mask_margin,
            solder_paste_margin=solder_paste_margin,
            solder_paste_margin_ratio=solder_paste_margin_ratio,
            clearance=clearance,
            zone_connect=zone_connect,
            unknown_elements=unknown_elements,
            _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        result: list = ['footprint', QuotedString(self.library_link)]
        result.append(['layer', QuotedString(self.layer)])
        _append_footprint_identity(result, self)
        _append_footprint_properties(result, self)
        _append_footprint_pad_groups(result, self)
        _append_footprint_optional_settings(result, self)

        for attr_name in _FOOTPRINT_TO_SEXP_COLLECTIONS:
            _append_object_sexps(result, getattr(self, attr_name))

        _append_footprint_embedded_files(result, self)
        _append_object_sexps(result, self.unknown_elements)

        return result

    @property
    def is_dnp(self) -> bool:
        return 'dnp' in self.attr

    @property
    def is_excluded_from_bom(self) -> bool:
        return 'exclude_from_bom' in self.attr

    @property
    def is_excluded_from_pos_files(self) -> bool:
        return 'exclude_from_pos_files' in self.attr

    @property
    def path(self) -> str:
        return self.placement.path

    @property
    def sheetname(self) -> str:
        return self.placement.sheetname

    @property
    def sheetfile(self) -> str:
        return self.placement.sheetfile

    @public_api
    def to_ir(
        self,
        *,
        document_id: str | None = None,
        board: "KiCadPcb | None" = None,
    ) -> "KiCadPlotterDocument":
        """Render this PCB-embedded footprint to a single-record plotter IR document."""
        from .kicad_pcb_to_ir import pcb_footprint_to_record
        from .kicad_plotter_ir import KiCadPlotterDocument

        return KiCadPlotterDocument(
            records=[pcb_footprint_to_record(self, board=board)],
            source_path=None,
            source_kind="PCB_FOOTPRINT",
            document_id=document_id or self.library_link,
            canvas=None,
            coordinate_space={"unit": "nm", "y_axis": "down"},
            background_color=None,
            render_hints=None,
            extras={
                "library_link": self.library_link,
                "layer": self.layer,
            },
        )

    @public_api
    def iter_objects(self) -> Iterator[object]:
        """Iterate over embedded-footprint-owned objects."""
        for list_name in _PCB_FOOTPRINT_OBJECT_LIST_NAMES:
            yield from getattr(self, list_name, ())

    @public_api
    def add_object(self, obj: object) -> object:
        """Add a typed object to the matching embedded-footprint-owned list."""
        list_name = _PCB_FOOTPRINT_OBJECT_LIST_BY_CLASS_NAME.get(type(obj).__name__)
        if list_name is None:
            raise TypeError(f"unsupported footprint object type: {type(obj).__name__}")
        getattr(self, list_name).append(obj)
        return obj

    @public_api
    def remove_object(self, obj: object) -> bool:
        """Remove an object by identity from its owning footprint list."""
        for list_name in _PCB_FOOTPRINT_OBJECT_LIST_NAMES:
            collection: list = getattr(self, list_name)
            for index, candidate in enumerate(collection):
                if candidate is obj:
                    del collection[index]
                    return True
        return False

    @public_api
    @property
    def objects(self) -> "KiCadObjectCollection":
        """Live read-only query view over embedded-footprint-owned objects."""
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
        """Get bounding box of this footprint in board coordinates.."""
        from .kicad_geometry import BoundingBox, rotate_point

        bbox = BoundingBox()
        angle = self.at_angle

        def transform_and_expand(local_bbox: 'BoundingBox') -> None:
            """Transform local bounds to board coordinates and expand bbox."""
            if not local_bbox.is_valid():
                return

            # Get corners of local bounding box
            corners = [
                (local_bbox.min_x, local_bbox.min_y),
                (local_bbox.max_x, local_bbox.min_y),
                (local_bbox.max_x, local_bbox.max_y),
                (local_bbox.min_x, local_bbox.max_y),
            ]

            # Rotate each corner and translate to board position
            for lx, ly in corners:
                if angle != 0:
                    rx, ry = rotate_point(lx, ly, -angle)  # KiCad uses CW rotation
                else:
                    rx, ry = lx, ly
                bbox.expand((rx + self.at_x, ry + self.at_y))

        # Collect bounds from all child elements
        for pad in self.pads:
            transform_and_expand(pad.get_bounds())

        for line in self.fp_lines:
            transform_and_expand(line.get_bounds())

        for arc in self.fp_arcs:
            transform_and_expand(arc.get_bounds())

        for circle in self.fp_circles:
            transform_and_expand(circle.get_bounds())

        for rect in self.fp_rects:
            transform_and_expand(rect.get_bounds())

        for poly in self.fp_polys:
            transform_and_expand(poly.get_bounds())

        for text in self.fp_texts:
            transform_and_expand(text.get_bounds())

        for text_box in self.fp_text_boxes:
            transform_and_expand(text_box.get_bounds())

        for prop in self.properties:
            transform_and_expand(prop.get_bounds())

        return bbox

    def outline_items(self, layer_name: str = EDGE_CUTS_LAYER) -> list[object]:
        """Return footprint-local items that can act as board-outline carriers."""
        items: list[object] = []
        for collection_name in ("fp_lines", "fp_arcs", "fp_rects", "fp_circles", "fp_polys"):
            for item in getattr(self, collection_name, []) or []:
                if str(getattr(item, "layer", "") or "").strip() == layer_name:
                    items.append(item)
        return items

    # ------------------------------------------------------------------
    # Variant override write API
    # ------------------------------------------------------------------

    def set_variant_override(
        self,
        name: str,
        *,
        dnp: Optional[bool] = None,
        exclude_from_bom: Optional[bool] = None,
        exclude_from_pos_files: Optional[bool] = None,
        fields: 'Optional[dict[str, str]]' = None,
        replace_fields: bool = False,
    ) -> FootprintVariant:
        """Add or update a per-footprint variant override.

        Each ``Optional[bool]`` parameter mirrors the variant block's
        elidable scalar tokens (``dnp``, ``exclude_from_bom``,
        ``exclude_from_pos_files``); leave at ``None`` to preserve the
        existing override (or omit the token entirely on a new one).

        ``fields`` merges into the variant's field list by default
        (existing ``FootprintVariantField`` entries with matching names
        are updated in place; new keys are appended). Pass
        ``replace_fields=True`` to replace the whole list (use ``{}`` to
        clear).

        Returns the affected ``FootprintVariant``.
        """
        existing = next((v for v in self.variants if v.name == name), None)
        if existing is None:
            existing = FootprintVariant(name=name)
            self.variants.append(existing)

        if dnp is not None:
            existing.dnp = dnp
        if exclude_from_bom is not None:
            existing.exclude_from_bom = exclude_from_bom
        if exclude_from_pos_files is not None:
            existing.exclude_from_pos_files = exclude_from_pos_files

        if fields is not None or replace_fields:
            incoming = dict(fields) if fields else {}
            if replace_fields:
                existing.fields = [
                    FootprintVariantField(name=k, value=v)
                    for k, v in incoming.items()
                ]
            else:
                seen: set = set()
                merged: List[FootprintVariantField] = []
                for fv in existing.fields:
                    if fv.name in incoming:
                        merged.append(FootprintVariantField(
                            name=fv.name, value=incoming[fv.name]
                        ))
                        seen.add(fv.name)
                    else:
                        merged.append(fv)
                for fname, fvalue in incoming.items():
                    if fname not in seen:
                        merged.append(FootprintVariantField(
                            name=fname, value=fvalue
                        ))
                existing.fields = merged

        return existing

    def remove_variant_override(self, name: str) -> bool:
        """Drop the variant override named ``name``. Returns ``True`` if
        a block was removed."""
        before = len(self.variants)
        self.variants = [v for v in self.variants if v.name != name]
        return len(self.variants) != before

    def to_svg(self, ctx: 'SvgRenderContext | None' = None) -> List[str]:
        """Render this footprint to SVG elements.."""
        from .kicad_geometry import SvgRenderContext

        if ctx is None:
            ctx = SvgRenderContext()

        # Create transformed context for this footprint
        # Apply footprint position and rotation
        fp_ctx = ctx.with_transform(self.at_x, self.at_y, self.at_angle)

        elements = []

        # Render all child elements with transformed context
        for pad in self.pads:
            elements.extend(pad.to_svg(fp_ctx))

        for line in self.fp_lines:
            elements.extend(line.to_svg(fp_ctx))

        for arc in self.fp_arcs:
            elements.extend(arc.to_svg(fp_ctx))

        for circle in self.fp_circles:
            elements.extend(circle.to_svg(fp_ctx))

        for rect in self.fp_rects:
            elements.extend(rect.to_svg(fp_ctx))

        for poly in self.fp_polys:
            elements.extend(poly.to_svg(fp_ctx))

        for text in self.fp_texts:
            elements.extend(text.to_svg(fp_ctx))

        for text_box in self.fp_text_boxes:
            elements.extend(text_box.to_svg(fp_ctx))

        for prop in self.properties:
            elements.extend(prop.to_svg(fp_ctx))

        return elements


# Re-export all element classes for backward compatibility
__all__ = [
    'Pad',
    'FpText',
    'FpTextBox',
    'Property',
    'FpLine',
    'FpArc',
    'FpCircle',
    'FpRect',
    'FpPoly',
    'Model',
    'EmbeddedFile',
    'Footprint',
]

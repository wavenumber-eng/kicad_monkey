"""
KiCad Schematic Symbol Instance

Placed symbol instances in schematic documents.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterator, List, Optional

from ._api_markers import public_api
from .kicad_sexpr import QuotedString
from .kicad_base import (
    find_element,
    find_all_elements,
    get_value,
    parse_maybe_absent_bool,
    unquote_string,
)
from .kicad_sch_enums import (
    PropertyId,
    StandardPropertyKey,
    standard_property_id_for_key,
)


@dataclass
class SchSymbolPin:
    """Pin UUID reference in a placed symbol instance.

    Each placed symbol tracks which pin alternate is selected (if any).

    S-expression format:
        (pin "PIN_NUMBER"
            (uuid "...")
            (alternate "AlternateName")
        )
    """
    number: str = ""
    uuid: str = ""
    alternate: Optional[str] = None

    @classmethod
    def from_sexp(cls, sexp: list) -> 'SchSymbolPin':
        """Parse from (pin "NUMBER" (uuid "...") (alternate "..."))."""
        number = unquote_string(sexp[1]) if len(sexp) > 1 else ""
        uuid = unquote_string(get_value(sexp, 'uuid', ''))

        alternate_elem = find_element(sexp, 'alternate')
        alternate = unquote_string(alternate_elem[1]) if alternate_elem and len(alternate_elem) > 1 else None

        return cls(number=number, uuid=uuid, alternate=alternate)

    def to_sexp(self) -> list:
        """Serialize to S-expression list."""
        result = ['pin', QuotedString(self.number)]
        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])
        if self.alternate:
            result.append(['alternate', QuotedString(self.alternate)])
        return result


@dataclass
class SchSymbolInstanceVariant:
    """Per-variant override on a symbol instance path.

    KiCad emits these inside ``(path ...)`` per
    ``sch_io_kicad_sexpr.cpp:953`` (saveSymbol). Each scalar bool is
    elided when the variant value matches the parent symbol's default,
    so ``Optional[bool]`` round-trips an absent token as ``None``.

    S-expression form::

        (variant (name "VariantName")
            [(dnp yes/no)]
            [(exclude_from_sim yes/no)]
            [(in_bom yes/no)]
            [(on_board yes/no)]
            [(in_pos_files yes/no)]
            [(field (name "...") (value "..."))]*
        )
    """
    name: str = ""
    dnp: Optional[bool] = None
    exclude_from_sim: Optional[bool] = None
    in_bom: Optional[bool] = None
    on_board: Optional[bool] = None
    in_pos_files: Optional[bool] = None
    # Ordered list of (field-name, field-value) overrides.
    fields: List[tuple] = field(default_factory=list)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'SchSymbolInstanceVariant':
        name_elem = find_element(sexp, 'name')
        name = unquote_string(name_elem[1]) if name_elem and len(name_elem) > 1 else ""

        def _opt_bool(tag: str) -> Optional[bool]:
            elem = find_element(sexp, tag)
            if elem is None or len(elem) <= 1:
                return None
            return elem[1] == 'yes'

        fields: List[tuple] = []
        for f_elem in find_all_elements(sexp, 'field'):
            fname_elem = find_element(f_elem, 'name')
            fvalue_elem = find_element(f_elem, 'value')
            fname = unquote_string(fname_elem[1]) if fname_elem and len(fname_elem) > 1 else ""
            fvalue = unquote_string(fvalue_elem[1]) if fvalue_elem and len(fvalue_elem) > 1 else ""
            fields.append((fname, fvalue))

        return cls(
            name=name,
            dnp=_opt_bool('dnp'),
            exclude_from_sim=_opt_bool('exclude_from_sim'),
            in_bom=_opt_bool('in_bom'),
            on_board=_opt_bool('on_board'),
            in_pos_files=_opt_bool('in_pos_files'),
            fields=fields,
        )

    def to_sexp(self) -> list:
        result: list = ['variant', ['name', QuotedString(self.name)]]
        # Order matches saveSymbol (sch_io_kicad_sexpr.cpp:959-972):
        # dnp, exclude_from_sim, in_bom, on_board, in_pos_files, then fields.
        for tag, value in (
            ('dnp', self.dnp),
            ('exclude_from_sim', self.exclude_from_sim),
            ('in_bom', self.in_bom),
            ('on_board', self.on_board),
            ('in_pos_files', self.in_pos_files),
        ):
            if value is not None:
                result.append([tag, 'yes' if value else 'no'])
        for fname, fvalue in self.fields:
            result.append(['field',
                           ['name', QuotedString(fname)],
                           ['value', QuotedString(fvalue)]])
        return result


@dataclass
class SchSymbolInstance:
    """Instance path data for a symbol (project/path specific reference/value).

    S-expression format (inside symbol element):
        (instances
            (project "ProjectName"
                (path "/UUID"
                    (reference "R1")
                    (unit 1)
                    [(variant ...)]*
                )
            )
        )
    """
    project: str = ""
    path: str = ""
    reference: str = ""
    unit: int = 1
    variants: List[SchSymbolInstanceVariant] = field(default_factory=list)

    @classmethod
    def from_sexp(cls, project_name: str, path_elem: list) -> 'SchSymbolInstance':
        """Parse from path element inside instances/project."""
        path = unquote_string(path_elem[1]) if len(path_elem) > 1 else ""
        reference = unquote_string(get_value(path_elem, 'reference', ''))
        unit = int(get_value(path_elem, 'unit', 1))
        variants = [SchSymbolInstanceVariant.from_sexp(v)
                    for v in find_all_elements(path_elem, 'variant')]

        return cls(project=project_name, path=path, reference=reference,
                   unit=unit, variants=variants)


@public_api
@dataclass
class SchSymbol:
    """Placed symbol instance in a schematic.

    This is NOT the symbol definition - it's a placed instance that references
    a symbol from lib_symbols via lib_id.

    S-expression format:
        (symbol
            (lib_id "Library:SymbolName")
            (lib_name "Library")
            (at X Y ANGLE)
            (mirror x)
            (unit N)
            (convert N)
            (exclude_from_sim no)
            (in_bom yes)
            (on_board yes)
            (in_pos_files yes)
            (dnp no)
            (fields_autoplaced)
            (uuid "...")
            (property "Reference" "R1" (at X Y A) (effects ...))
            (property "Value" "10k" ...)
            ...
            (pin "1" (uuid "..."))
            (pin "2" (uuid "..."))
            (instances
                (project "..." (path "..." (reference "R1") (unit 1)))
            )
        )
    """
    lib_id: str = ""  # e.g., "Device:R" or "vendor_parts:ERJ-2RKF1002X"
    lib_name: str = ""  # Explicit library name (optional, KiCad 9)

    at_x: float = 0.0
    at_y: float = 0.0
    at_angle: float = 0.0
    mirror: Optional[str] = None  # "x", "y", or None

    unit: int = 1
    convert: int = 1  # Style variant (1=normal, 2=De Morgan)

    # Flags
    exclude_from_sim: bool = False
    in_bom: bool = True
    on_board: bool = True
    in_pos_files: bool = True
    dnp: bool = False  # Do Not Populate
    fields_autoplaced: bool = False

    # Properties (overrides from symbol definition)
    properties: list = field(default_factory=list)

    # Pin mappings (UUID per pin)
    pins: List[SchSymbolPin] = field(default_factory=list)

    # Instance data (populated from instances section)
    instances: List[SchSymbolInstance] = field(default_factory=list)

    uuid: str = ""

    _raw_sexp: Optional[list] = field(default=None, repr=False)

    @classmethod
    def from_sexp(cls, sexp: list) -> 'SchSymbol':
        """Parse from (symbol (lib_id "...") (at X Y A) ...)."""
        from .kicad_sym_property import SymProperty

        lib_id = unquote_string(get_value(sexp, 'lib_id', ''))
        lib_name = unquote_string(get_value(sexp, 'lib_name', ''))

        # Position
        at_elem = find_element(sexp, 'at')
        at_x = float(at_elem[1]) if at_elem and len(at_elem) > 1 else 0.0
        at_y = float(at_elem[2]) if at_elem and len(at_elem) > 2 else 0.0
        at_angle = float(at_elem[3]) if at_elem and len(at_elem) > 3 else 0.0

        # Mirror
        mirror_elem = find_element(sexp, 'mirror')
        mirror = mirror_elem[1] if mirror_elem and len(mirror_elem) > 1 else None

        unit = int(get_value(sexp, 'unit', 1))
        convert = int(get_value(sexp, 'convert', 1))

        # Flags - handle both keyword flags and value-based flags
        exclude_from_sim_val = get_value(sexp, 'exclude_from_sim', 'no')
        exclude_from_sim = exclude_from_sim_val == 'yes' if exclude_from_sim_val else False

        in_bom_val = get_value(sexp, 'in_bom', 'yes')
        in_bom = in_bom_val == 'yes' if in_bom_val else True

        on_board_val = get_value(sexp, 'on_board', 'yes')
        on_board = on_board_val == 'yes' if on_board_val else True

        # `in_pos_files` is emitted by saveSymbol (sch_io_kicad_sexpr.cpp:774)
        # between on_board and dnp; default true when absent.
        in_pos_files_val = get_value(sexp, 'in_pos_files', 'yes')
        in_pos_files = in_pos_files_val == 'yes' if in_pos_files_val else True

        dnp_val = get_value(sexp, 'dnp', 'no')
        dnp = dnp_val == 'yes' if dnp_val else False

        # KiCad 10 emits `(fields_autoplaced yes)` via KICAD_FORMAT::FormatBool;
        # the parser at sch_io_kicad_sexpr_parser.cpp:3270 uses parseMaybeAbsentBool(true)
        # so the empty `(fields_autoplaced)` form means True.
        fields_autoplaced = parse_maybe_absent_bool(sexp, 'fields_autoplaced') or False

        uuid = unquote_string(get_value(sexp, 'uuid', ''))

        # Properties
        properties = []
        for prop_elem in find_all_elements(sexp, 'property'):
            properties.append(SymProperty.from_sexp(prop_elem))

        # Pins
        pins = []
        for pin_elem in find_all_elements(sexp, 'pin'):
            pins.append(SchSymbolPin.from_sexp(pin_elem))

        # Instances
        instances = []
        instances_elem = find_element(sexp, 'instances')
        if instances_elem:
            for project_elem in find_all_elements(instances_elem, 'project'):
                project_name = unquote_string(project_elem[1]) if len(project_elem) > 1 else ""
                for path_elem in find_all_elements(project_elem, 'path'):
                    instances.append(SchSymbolInstance.from_sexp(project_name, path_elem))

        return cls(
            lib_id=lib_id, lib_name=lib_name,
            at_x=at_x, at_y=at_y, at_angle=at_angle, mirror=mirror,
            unit=unit, convert=convert,
            exclude_from_sim=exclude_from_sim,
            in_bom=in_bom, on_board=on_board, in_pos_files=in_pos_files, dnp=dnp,
            fields_autoplaced=fields_autoplaced,
            properties=properties, pins=pins, instances=instances,
            uuid=uuid, _raw_sexp=sexp
        )

    def to_sexp(self) -> list:
        """Serialize to S-expression list."""
        result = ['symbol', ['lib_id', QuotedString(self.lib_id)]]

        if self.lib_name:
            result.append(['lib_name', QuotedString(self.lib_name)])

        # KiCad's reader requires the angle slot even when zero (drift inventory #1).
        result.append(['at', self.at_x, self.at_y, self.at_angle])

        if self.mirror:
            result.append(['mirror', self.mirror])

        result.append(['unit', self.unit])

        if self.convert != 1:
            result.append(['convert', self.convert])

        result.append(['exclude_from_sim', 'yes' if self.exclude_from_sim else 'no'])
        result.append(['in_bom', 'yes' if self.in_bom else 'no'])
        result.append(['on_board', 'yes' if self.on_board else 'no'])
        result.append(['in_pos_files', 'yes' if self.in_pos_files else 'no'])
        result.append(['dnp', 'yes' if self.dnp else 'no'])

        if self.fields_autoplaced:
            result.append(['fields_autoplaced', 'yes'])

        if self.uuid:
            result.append(['uuid', QuotedString(self.uuid)])

        for prop in self.properties:
            result.append(prop.to_sexp())

        for pin in self.pins:
            result.append(pin.to_sexp())

        # Instances
        if self.instances:
            instances_elem: list = ['instances']
            # Group by project
            by_project: Dict[str, list] = {}
            for inst in self.instances:
                if inst.project not in by_project:
                    by_project[inst.project] = []
                by_project[inst.project].append(inst)

            for project_name, project_instances in by_project.items():
                project_elem: list = ['project', QuotedString(project_name)]
                for inst in project_instances:
                    path_elem: list = ['path', QuotedString(inst.path)]
                    path_elem.append(['reference', QuotedString(inst.reference)])
                    path_elem.append(['unit', inst.unit])
                    for variant in inst.variants:
                        path_elem.append(variant.to_sexp())
                    project_elem.append(path_elem)
                instances_elem.append(project_elem)

            result.append(instances_elem)

        return result

    # Convenience properties
    @property
    def reference(self) -> str:
        """Get reference designator from properties."""
        return self.get_property_value(StandardPropertyKey.REFERENCE)

    @property
    def value(self) -> str:
        """Get value from properties."""
        return self.get_property_value(StandardPropertyKey.VALUE)

    @property
    def footprint(self) -> str:
        """Get footprint from properties."""
        return self.get_property_value(StandardPropertyKey.FOOTPRINT)

    @public_api
    def get_property_object(self, key: str | StandardPropertyKey):
        """Get a property object by key."""
        key_text = str(key)
        for prop in self.properties:
            if prop.key == key_text:
                return prop
        return None

    @public_api
    def get_property(self, key: str | StandardPropertyKey) -> Optional[str]:
        """Get property value by key."""
        prop = self.get_property_object(key)
        return prop.value if prop is not None else None

    @public_api
    def get_property_value(
        self,
        key: str | StandardPropertyKey,
        default: str = "",
    ) -> str:
        """Get property value by key, returning default when absent."""
        value = self.get_property(key)
        return value if value is not None else default

    @public_api
    def set_property_value(
        self,
        key: str | StandardPropertyKey,
        value: str,
        *,
        create: bool = False,
    ) -> bool:
        """Set property value. Returns True if a property was updated or created."""
        prop = self.get_property_object(key)
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
    ):
        """Create or update a property and return the property object."""
        from .kicad_sym_property import SymProperty

        prop = self.get_property_object(key)
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
    def iter_properties(self) -> Iterator[object]:
        """Iterate over symbol instance properties."""
        return iter(self.properties)

    # ------------------------------------------------------------------
    # Variant override write API
    # ------------------------------------------------------------------

    def _find_instance(
        self,
        instance_path: Optional[str] = None,
        project: Optional[str] = None,
    ) -> SchSymbolInstance:
        """Locate the SchSymbolInstance to mutate.

        - If ``instance_path`` is given, match it exactly (and narrow by
          ``project`` if supplied).
        - Otherwise, if exactly one instance matches the (optional)
          project filter, use it.
        - Otherwise, raise ValueError so the caller can disambiguate.
        """
        candidates = [
            inst for inst in self.instances
            if (project is None or inst.project == project)
            and (instance_path is None or inst.path == instance_path)
        ]
        if not candidates:
            raise ValueError(
                f"no symbol instance matches project={project!r}, "
                f"instance_path={instance_path!r} on lib_id={self.lib_id!r}"
            )
        if len(candidates) > 1:
            paths = [(c.project, c.path) for c in candidates]
            raise ValueError(
                f"multiple symbol instances matched ({len(candidates)}); "
                f"pass instance_path= and/or project= to disambiguate. "
                f"Candidates: {paths}"
            )
        return candidates[0]

    def set_variant_override(
        self,
        name: str,
        *,
        dnp: Optional[bool] = None,
        exclude_from_sim: Optional[bool] = None,
        in_bom: Optional[bool] = None,
        on_board: Optional[bool] = None,
        in_pos_files: Optional[bool] = None,
        fields: Optional[Dict[str, str]] = None,
        instance_path: Optional[str] = None,
        project: Optional[str] = None,
        replace_fields: bool = False,
    ) -> SchSymbolInstanceVariant:
        """Add or update a per-variant override on a symbol instance.

        Each ``Optional[bool]`` parameter mirrors the variant block's
        elidable scalar tokens (``dnp``, ``exclude_from_sim``, ``in_bom``,
        ``on_board``, ``in_pos_files``); leave at ``None`` to preserve
        the existing override (or omit the token entirely on a new one).
        Pass ``True``/``False`` to set explicitly.

        ``fields`` is merged into the variant's field list by default
        (existing keys updated in place, new keys appended). Pass
        ``replace_fields=True`` together with ``fields`` to replace the
        entire field list. Pass ``fields={}`` with ``replace_fields=True``
        to clear all field overrides.

        Returns the affected ``SchSymbolInstanceVariant``.
        """
        inst = self._find_instance(instance_path=instance_path, project=project)
        existing = next((v for v in inst.variants if v.name == name), None)
        if existing is None:
            existing = SchSymbolInstanceVariant(name=name)
            inst.variants.append(existing)

        if dnp is not None:
            existing.dnp = dnp
        if exclude_from_sim is not None:
            existing.exclude_from_sim = exclude_from_sim
        if in_bom is not None:
            existing.in_bom = in_bom
        if on_board is not None:
            existing.on_board = on_board
        if in_pos_files is not None:
            existing.in_pos_files = in_pos_files

        if fields is not None or replace_fields:
            incoming = dict(fields) if fields else {}
            if replace_fields:
                existing.fields = [(k, v) for k, v in incoming.items()]
            else:
                # Merge: update in-place where keys match, append the rest.
                seen: set = set()
                merged: List[tuple] = []
                for fname, fvalue in existing.fields:
                    if fname in incoming:
                        merged.append((fname, incoming[fname]))
                        seen.add(fname)
                    else:
                        merged.append((fname, fvalue))
                for fname, fvalue in incoming.items():
                    if fname not in seen:
                        merged.append((fname, fvalue))
                existing.fields = merged

        return existing

    def remove_variant_override(
        self,
        name: str,
        *,
        instance_path: Optional[str] = None,
        project: Optional[str] = None,
    ) -> bool:
        """Drop the variant override named ``name`` from the matching
        instance. Returns ``True`` if a block was removed."""
        inst = self._find_instance(instance_path=instance_path, project=project)
        before = len(inst.variants)
        inst.variants = [v for v in inst.variants if v.name != name]
        return len(inst.variants) != before


def _next_user_property_id(properties: list) -> int:
    max_id = max((int(prop.id) for prop in properties), default=int(PropertyId.USER_START) - 1)
    return max(max_id + 1, int(PropertyId.USER_START))

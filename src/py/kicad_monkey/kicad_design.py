"""KiCad project and design aggregator.

A :class:`KiCadDesign` ties together the on-disk files that make up a
single KiCad project:

* ``.kicad_pro``: :class:`KiCadProject` with text variables, net settings,
  and the variant catalog.
* ``.kicad_sch``: one or more :class:`KiCadSchematic` instances. The
  parser recurses into sub-sheets via ``Sheetfile``; this class owns the
  top-level entry sheet plus any explicitly-loaded extras.
* ``.kicad_pcb``: a lazy :class:`KiCadPcb`, parsed on first access so
  schematic-only workflows do not pay the PCB parse cost.

The aggregator also provides cross-document ``${VAR}`` resolution.
Title-block text in schematics and boards may reference custom variables
defined under ``text_variables`` in the sidecar project file. KiCad's
built-in sheet and title variables take precedence over same-named
project text variables.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
import json
from pathlib import Path
from typing import TYPE_CHECKING, Iterable, Iterator, Optional

from ._api_markers import public_api
from .kicad_project import (
    KiCadProject,
    find_adjacent_kicad_project_path,
)

if TYPE_CHECKING:
    from .kicad_netlist_model import KiCadNet, KiCadNetlist, KiCadNetlistComponent
    from .kicad_pcb import KiCadPcb
    from .kicad_plotter_ir import KiCadPlotterDocument
    from .kicad_sch_sheet import SchSheet
    from .kicad_schematic import KiCadSchematic


def _path_key(path: Path | str) -> str:
    path_obj = Path(path)
    try:
        return str(path_obj.resolve())
    except (OSError, ValueError):
        return str(path_obj)


def _schematic_source_path(schematic: "KiCadSchematic") -> Path | None:
    source = getattr(schematic, "source_path", None)
    if source is None:
        return None
    if isinstance(source, Path):
        return source
    return Path(str(source))


def _normalize_sheet_path(path: str | None) -> str | None:
    if path is None:
        return None
    text = str(path).strip()
    if not text:
        return "/"
    text = "/" + text.strip("/")
    return "/" if text == "/" else f"{text}/"


def _normalize_sheet_instance_path(path: str | None) -> str | None:
    if path is None:
        return None
    text = str(path).strip()
    if not text:
        return None
    text = "/" + text.strip("/")
    return "/" if text == "/" else text.rstrip("/")


def _join_sheet_path(parent: str, child: str) -> str:
    parent = _normalize_sheet_path(parent) or "/"
    child = str(child or "").strip("/")
    return parent if not child else f"{parent}{child}/"


def _sheet_instance_path(parent: str | None, sheet_uuid: str) -> str | None:
    if not parent or not sheet_uuid:
        return None
    return _join_sheet_path(parent, sheet_uuid).rstrip("/")


def _page_number_from_instances(
    instances: Iterable[object] | None,
    target_path: str | None,
) -> int | None:
    normalized_target = _normalize_sheet_instance_path(target_path)
    fallback: int | None = None
    for inst in instances or ():
        page = str(getattr(inst, "page", "") or "")
        if not page.isdigit():
            continue
        page_number = int(page)
        if fallback is None:
            fallback = page_number
        inst_path = _normalize_sheet_instance_path(
            str(getattr(inst, "path", "") or "")
        )
        if normalized_target and inst_path == normalized_target:
            return page_number
    return fallback


@public_api
@dataclass(frozen=True)
class KiCadSchematicInstance:
    """One concrete placement of a schematic in a KiCad hierarchy.

    A single `.kicad_sch` file can appear more than once through repeated
    hierarchical sheets. This record keeps the source schematic together with
    the human sheet path, the KiCad UUID instance path, the parent sheet link,
    and the sheet numbering needed for rendering.
    """

    instance_index: int
    sheet_number: int
    sheet_count: int
    schematic: "KiCadSchematic"
    source_path: Path | None
    sheet_name: str
    sheet_path: str
    sheet_path_uuids: str
    sheet_instance_path: str | None = None
    sheet_symbol: Optional["SchSheet"] = None
    sheet_symbol_uid: str = ""
    sheet_file: str = ""
    parent_sheet_path: str | None = None
    parent_sheet_path_uuids: str | None = None
    parent_sheet_instance_path: str | None = None
    is_top_level: bool = False

    @property
    def source_key(self) -> str:
        """Stable source-file key, falling back to object identity."""
        if self.source_path is None:
            return f"schematic-object:{id(self.schematic)}"
        return _path_key(self.source_path)

    def ir_kwargs(self, *, document_id: str | None = None) -> dict[str, object]:
        """Return keyword arguments for :meth:`KiCadDesign.to_schematic_ir`."""
        resolved_document_id = document_id
        if resolved_document_id is None:
            resolved_document_id = (
                str(getattr(self.schematic, "uuid", "") or "")
                or (self.source_path.stem if self.source_path else "")
                or f"sheet_{self.instance_index}"
            )
        return {
            "sheet_index": self.sheet_number,
            "sheet_count": self.sheet_count,
            "sheet_path": self.sheet_path,
            "sheet_instance_path": self.sheet_instance_path,
            "sheet_name": self.sheet_name,
            "document_id": resolved_document_id,
        }


@public_api
@dataclass
class KiCadDesign:
    """Composed KiCad design: project + schematics + (lazy) PCB.

    Construct via the ``from_*`` classmethods; direct construction is
    supported for tests but won't auto-populate adjacent files.
    """

    project: Optional[KiCadProject] = None
    schematics: list["KiCadSchematic"] = field(default_factory=list)
    pcb_path: Optional[Path] = None
    project_path: Optional[Path] = None
    _pcb: Optional["KiCadPcb"] = field(default=None, repr=False)
    _netlist: Optional["KiCadNetlist"] = field(default=None, repr=False)

    # ------------------------------------------------------------------
    # Constructors
    # ------------------------------------------------------------------

    @classmethod
    @public_api
    def from_file(cls, path: Path | str) -> "KiCadDesign":
        """Load a design from a `.kicad_pro`, `.kicad_sch`, or `.kicad_pcb` path."""
        source_path = Path(path)
        suffix = source_path.suffix.lower()
        if suffix == ".kicad_pro":
            return cls.from_project_file(source_path)
        if suffix == ".kicad_sch":
            return cls.from_schematic_file(source_path)
        if suffix == ".kicad_pcb":
            return cls.from_pcb_file(source_path)
        raise ValueError(
            f"unsupported KiCad design file suffix {source_path.suffix!r}; "
            "expected .kicad_pro, .kicad_sch, or .kicad_pcb"
        )

    @classmethod
    @public_api
    def from_project_file(cls, path: Path | str) -> "KiCadDesign":
        """Load `.kicad_pro` and any adjacent top-level `.kicad_sch` /
        `.kicad_pcb` (matched by stem). Sub-sheets are pulled in
        recursively via the schematic parser.
        """
        from .kicad_schematic import KiCadSchematic

        project_path = Path(path)
        project = KiCadProject.from_file(project_path)

        stem = project_path.stem
        parent = project_path.parent
        sch_candidate = parent / f"{stem}.kicad_sch"
        pcb_candidate = parent / f"{stem}.kicad_pcb"

        schematics: list[KiCadSchematic] = []
        if sch_candidate.is_file():
            schematics.append(KiCadSchematic(sch_candidate))

        pcb_path = pcb_candidate if pcb_candidate.is_file() else None

        return cls(
            project=project,
            schematics=schematics,
            pcb_path=pcb_path,
            project_path=project_path,
        )

    @classmethod
    @public_api
    def from_schematic_file(cls, path: Path | str) -> "KiCadDesign":
        """Load a `.kicad_sch` (with recursive sub-sheets) and any
        adjacent `.kicad_pro` discovered by stem / single-sibling rule.
        """
        from .kicad_schematic import KiCadSchematic

        sch_path = Path(path)
        sch = KiCadSchematic(sch_path)
        project_path = find_adjacent_kicad_project_path(sch_path)
        project = KiCadProject.from_file(project_path) if project_path else None
        return cls(
            project=project,
            schematics=[sch],
            pcb_path=None,
            project_path=project_path,
        )

    @classmethod
    @public_api
    def from_pcb_file(cls, path: Path | str) -> "KiCadDesign":
        """Load a `.kicad_pcb` (lazily) and any adjacent `.kicad_pro`
        discovered by stem / single-sibling rule.
        """
        pcb_path = Path(path)
        project_path = find_adjacent_kicad_project_path(pcb_path)
        project = KiCadProject.from_file(project_path) if project_path else None
        return cls(
            project=project,
            schematics=[],
            pcb_path=pcb_path,
            project_path=project_path,
        )

    # ------------------------------------------------------------------
    # Convenience accessors
    # ------------------------------------------------------------------

    @property
    def text_variables(self) -> dict[str, str]:
        """Project-scoped ``${VAR}`` substitutions.

        Empty dict when no `.kicad_pro` is associated. Returns a copy
        so callers can mutate freely without poisoning the project's
        own dict.
        """
        if self.project is None:
            return {}
        return dict(self.project.text_variables)

    @property
    def pcb(self) -> Optional["KiCadPcb"]:
        """Loaded :class:`KiCadPcb`, parsing on first access."""
        if self._pcb is not None:
            return self._pcb
        if self.pcb_path is None:
            return None
        from .kicad_pcb import KiCadPcb

        self._pcb = KiCadPcb(self.pcb_path)
        return self._pcb

    @property
    def top_schematic(self) -> Optional["KiCadSchematic"]:
        """First-loaded entry-point schematic, if any."""
        return self.schematics[0] if self.schematics else None

    @public_api
    def iter_schematics(self) -> Iterator["KiCadSchematic"]:
        """Iterate over schematics owned by this design."""
        return iter(self.schematics)

    @public_api
    def iter_schematic_instances(self) -> Iterator[KiCadSchematicInstance]:
        """Iterate concrete schematic placements in hierarchy order.

        Unlike :meth:`iter_schematics`, this expands repeated hierarchical
        sheet placements. A reused child sheet therefore yields one
        :class:`KiCadSchematicInstance` per occurrence.
        """
        yield from self.schematic_instances()

    @public_api
    def schematic_instances(self) -> list[KiCadSchematicInstance]:
        """Return concrete schematic placements in hierarchy order."""
        top = self.top_schematic
        if top is None:
            return []

        instances: list[KiCadSchematicInstance] = []
        top_source = _schematic_source_path(top)
        top_name = top_source.stem if top_source is not None else "root"
        top_instance_path = (
            f"/{top.uuid}" if str(getattr(top, "uuid", "") or "") else None
        )
        top_sheet_number = (
            _page_number_from_instances(
                getattr(top, "sheet_instances", ()),
                top_instance_path,
            )
            or 1
        )
        instances.append(
            KiCadSchematicInstance(
                instance_index=1,
                sheet_number=top_sheet_number,
                sheet_count=0,
                schematic=top,
                source_path=top_source,
                sheet_name=top_name,
                sheet_path="/",
                sheet_path_uuids="/",
                sheet_instance_path=top_instance_path,
                sheet_symbol=None,
                sheet_symbol_uid="",
                sheet_file="",
                parent_sheet_path=None,
                parent_sheet_path_uuids=None,
                parent_sheet_instance_path=None,
                is_top_level=True,
            )
        )

        def walk(
            parent: "KiCadSchematic",
            *,
            parent_sheet_path: str,
            parent_sheet_path_uuids: str,
            parent_instance_path: str | None,
        ) -> None:
            for sheet in getattr(parent, "sheets", ()) or ():
                child = getattr(parent, "sub_schematics", {}).get(sheet.sheet_file)
                if child is None:
                    continue

                sheet_name = sheet.sheet_name or Path(sheet.sheet_file).stem
                child_sheet_path = _join_sheet_path(parent_sheet_path, sheet_name)
                child_uuid_path = _join_sheet_path(
                    parent_sheet_path_uuids,
                    sheet.uuid or sheet.sheet_file,
                )
                child_instance_path = _sheet_instance_path(
                    parent_instance_path,
                    getattr(sheet, "uuid", "") or "",
                )
                instances.append(
                    KiCadSchematicInstance(
                        instance_index=len(instances) + 1,
                        sheet_number=_page_number_from_instances(
                            getattr(sheet, "instances", ()),
                            child_instance_path,
                        )
                        or len(instances)
                        + 1,
                        sheet_count=0,
                        schematic=child,
                        source_path=_schematic_source_path(child),
                        sheet_name=sheet_name,
                        sheet_path=child_sheet_path,
                        sheet_path_uuids=child_uuid_path,
                        sheet_instance_path=child_instance_path,
                        sheet_symbol=sheet,
                        sheet_symbol_uid=sheet.uuid or "",
                        sheet_file=sheet.sheet_file,
                        parent_sheet_path=parent_sheet_path,
                        parent_sheet_path_uuids=parent_sheet_path_uuids,
                        parent_sheet_instance_path=parent_instance_path,
                        is_top_level=False,
                    )
                )
                walk(
                    child,
                    parent_sheet_path=child_sheet_path,
                    parent_sheet_path_uuids=child_uuid_path,
                    parent_instance_path=child_instance_path,
                )

        walk(
            top,
            parent_sheet_path="/",
            parent_sheet_path_uuids="/",
            parent_instance_path=top_instance_path,
        )

        sheet_count = len(instances)
        return [
            replace(instance, sheet_count=sheet_count)
            for instance in instances
        ]

    @public_api
    def find_schematic_instances(
        self,
        *,
        schematic: Optional["KiCadSchematic"] = None,
        source_path: Path | str | None = None,
        sheet_path: str | None = None,
        sheet_instance_path: str | None = None,
        sheet_name: str | None = None,
    ) -> list[KiCadSchematicInstance]:
        """Find hierarchy instances by schematic object, source path, or sheet path."""
        source_key = _path_key(source_path) if source_path is not None else None
        normalized_sheet_path = _normalize_sheet_path(sheet_path)
        normalized_instance_path = _normalize_sheet_instance_path(sheet_instance_path)
        matches: list[KiCadSchematicInstance] = []
        for instance in self.iter_schematic_instances():
            if schematic is not None and instance.schematic is not schematic:
                continue
            if source_key is not None and instance.source_key != source_key:
                continue
            if (
                normalized_sheet_path is not None
                and _normalize_sheet_path(instance.sheet_path) != normalized_sheet_path
            ):
                continue
            if (
                normalized_instance_path is not None
                and _normalize_sheet_instance_path(instance.sheet_instance_path)
                != normalized_instance_path
            ):
                continue
            if sheet_name is not None and instance.sheet_name != sheet_name:
                continue
            matches.append(instance)
        return matches

    @public_api
    def schematic_instances_for(
        self,
        schematic_or_path: "KiCadSchematic | Path | str",
    ) -> list[KiCadSchematicInstance]:
        """Return every hierarchy use of a schematic object or source file."""
        if isinstance(schematic_or_path, (str, Path)):
            return self.find_schematic_instances(source_path=schematic_or_path)
        return self.find_schematic_instances(schematic=schematic_or_path)

    @public_api
    def child_schematic_instances(
        self,
        instance_or_sheet_path: KiCadSchematicInstance | str,
    ) -> list[KiCadSchematicInstance]:
        """Return immediate child schematic instances for a hierarchy instance."""
        instances = self.schematic_instances()
        parent = self._schematic_instance_from_reference(instance_or_sheet_path, instances)
        parent_sheet_path = (
            parent.sheet_path
            if parent is not None
            else _normalize_sheet_path(str(instance_or_sheet_path))
        )
        return [
            instance
            for instance in instances
            if _normalize_sheet_path(instance.parent_sheet_path) == parent_sheet_path
        ]

    @public_api
    def parent_schematic_instance(
        self,
        instance_or_sheet_path: KiCadSchematicInstance | str,
    ) -> KiCadSchematicInstance | None:
        """Return the parent hierarchy instance for a schematic instance."""
        instances = self.schematic_instances()
        child = self._schematic_instance_from_reference(instance_or_sheet_path, instances)
        if child is None or child.parent_sheet_path is None:
            return None
        parent_path = _normalize_sheet_path(child.parent_sheet_path)
        for instance in instances:
            if _normalize_sheet_path(instance.sheet_path) == parent_path:
                return instance
        return None

    @public_api
    def add_schematic(self, schematic: "KiCadSchematic") -> "KiCadSchematic":
        """Append a schematic to this design."""
        self.schematics.append(schematic)
        self._netlist = None
        return schematic

    @public_api
    def remove_schematic(self, schematic: "KiCadSchematic") -> bool:
        """Remove a schematic by identity."""
        for index, candidate in enumerate(self.schematics):
            if candidate is schematic:
                del self.schematics[index]
                self._netlist = None
                return True
        return False

    @public_api
    def iter_objects(self, *, include_pcb: bool = True) -> Iterator[object]:
        """Iterate over project, schematic, and optional PCB documents."""
        if self.project is not None:
            yield self.project
        yield from self.schematics
        if include_pcb:
            pcb = self.pcb
            if pcb is not None:
                yield pcb

    @public_api
    @property
    def objects(self):
        """Live read-only query view over design-owned documents."""
        from .kicad_object_collection import KiCadObjectCollection

        return KiCadObjectCollection(lambda: self.iter_objects(), owner=self)

    # ------------------------------------------------------------------
    # IR convenience
    # ------------------------------------------------------------------

    def to_schematic_ir(
        self,
        schematic: Optional["KiCadSchematic"] = None,
        *,
        sheet_index: int = 1,
        sheet_count: int = 1,
        sheet_path: str = "/",
        sheet_instance_path: Optional[str] = None,
        sheet_name: str = "",
        document_id: Optional[str] = None,
        extra_vars: Optional[dict] = None,
    ) -> "KiCadPlotterDocument":
        """Convert a schematic to a :class:`KiCadPlotterDocument` with
        ``project_vars`` automatically populated from the project's
        ``text_variables``.

        ``extra_vars`` overrides project-level text variables when
        supplied. KiCad built-in sheet/title variables still take
        precedence over same-named project text variables. If
        ``schematic`` is ``None`` the design's :attr:`top_schematic`
        is used.
        """
        from .kicad_schematic_to_ir import schematic_to_ir

        target = schematic if schematic is not None else self.top_schematic
        if target is None:
            raise ValueError(
                "no schematic supplied and design has no top schematic loaded"
            )

        merged: dict[str, str] = self.text_variables
        if extra_vars:
            merged.update({str(k): str(v) for k, v in extra_vars.items()})

        source_path = None
        if getattr(target, "source_path", None) is not None:
            source_path = str(target.source_path)

        return schematic_to_ir(
            target,
            source_path=source_path,
            document_id=document_id,
            sheet_index=sheet_index,
            sheet_count=sheet_count,
            sheet_path=sheet_path,
            sheet_instance_path=sheet_instance_path,
            sheet_name=sheet_name,
            project_vars=merged,
        )

    @public_api
    def to_schematic_instance_ir(
        self,
        instance: KiCadSchematicInstance,
        *,
        document_id: str | None = None,
        extra_vars: Optional[dict] = None,
    ) -> "KiCadPlotterDocument":
        """Convert one concrete schematic hierarchy instance to plotter IR."""
        return self.to_schematic_ir(
            schematic=instance.schematic,
            sheet_index=instance.sheet_number,
            sheet_count=instance.sheet_count,
            sheet_path=instance.sheet_path,
            sheet_instance_path=instance.sheet_instance_path,
            sheet_name=instance.sheet_name,
            document_id=document_id
            if document_id is not None
            else str(instance.ir_kwargs()["document_id"]),
            extra_vars=extra_vars,
        )

    @public_api
    def to_pcb_ir(
        self,
        *,
        document_id: Optional[str] = None,
    ) -> "KiCadPlotterDocument":
        """Convert the associated PCB to plotter IR."""
        pcb = self.pcb
        if pcb is None:
            raise ValueError("design has no PCB loaded or associated")
        source_path = str(self.pcb_path) if self.pcb_path is not None else None
        return pcb.to_ir(source_path=source_path, document_id=document_id)

    @public_api
    def to_pcb_svg(self, **kwargs) -> str:
        """Render the associated PCB to SVG."""
        pcb = self.pcb
        if pcb is None:
            raise ValueError("design has no PCB loaded or associated")
        return pcb.to_svg(**kwargs)


    # ------------------------------------------------------------------
    # Netlist API
    # ------------------------------------------------------------------

    def to_netlist(self) -> "KiCadNetlist":
        """Compile (and cache) the unified design netlist.

        Walks the entry-point schematic recursively via
        :func:`compile_design_netlist`, returning a fully-populated
        :class:`KiCadNetlist` (nets / components / libparts /
        design_metadata.sheets). Subsequent calls return the cached
        instance — call :meth:`refresh_netlist` to discard the cache
        and recompile (useful when the underlying schematics have been
        edited in place).

        Raises:
            ValueError: when the design has no top schematic loaded.
        """
        if self._netlist is None:
            self._netlist = self._compile_netlist()
        return self._netlist

    def refresh_netlist(self) -> "KiCadNetlist":
        """Discard the cached netlist and recompile from scratch."""
        self._netlist = None
        return self.to_netlist()

    def to_kicad_netlist_sexpr(
        self,
        *,
        tool: str = "kicad_monkey",
        date: Optional[str] = None,
    ) -> str:
        """Render the design as a kicad-cli–style ``(export ...)`` netlist.

        The ``(source ...)`` line is filled with the top schematic's
        absolute path when known. ``tool`` and ``date`` mirror
        :func:`to_kicad_sexpr` semantics — pass ``date=""`` to suppress
        the timestamp (useful for byte-stable test goldens).
        """
        from .kicad_netlist_kicadsexpr import to_kicad_sexpr

        netlist = self.to_netlist()
        source_path = ""
        top = self.top_schematic
        if top is not None and getattr(top, "source_path", None) is not None:
            source_path = str(top.source_path)
        return to_kicad_sexpr(
            netlist,
            source_path=source_path,
            tool=tool,
            date=date,
        )

    def to_netlist_json(self) -> dict:
        """Render the design as a KiCad-native netlist JSON payload."""
        from .kicad_design_json import kicad_netlist_to_json

        return kicad_netlist_to_json(self.to_netlist())

    def to_json(self, include_indexes: bool = True) -> dict:
        """Render a KiCad-native design JSON payload.

        The payload uses KiCad-owned schema IDs and includes project, sheet,
        component, net, variant, and optional index sections.
        """
        from .kicad_design_json import kicad_design_to_json

        return kicad_design_to_json(self, include_indexes=include_indexes)

    def to_json_text(self, *, include_indexes: bool = True, indent: int = 2) -> str:
        """Render :meth:`to_json` as formatted JSON text."""
        return json.dumps(
            self.to_json(include_indexes=include_indexes),
            indent=indent,
            ensure_ascii=False,
        ) + "\n"

    def save_json(
        self,
        path: Path | str,
        *,
        include_indexes: bool = True,
        indent: int = 2,
    ) -> None:
        """Write :meth:`to_json_text` to disk."""
        Path(path).write_text(
            self.to_json_text(include_indexes=include_indexes, indent=indent),
            encoding="utf-8",
        )

    def to_kicad_netlist_json(self) -> dict:
        """Render the internal netlist as a KiCad-native raw JSON payload."""
        from .kicad_design_json import kicad_netlist_to_json

        return kicad_netlist_to_json(self.to_netlist())

    def get_net(self, name: str) -> Optional["KiCadNet"]:
        """Look up a compiled net by name."""
        return self.to_netlist().get_net(name)

    def get_component(self, reference: str) -> Optional["KiCadNetlistComponent"]:
        """Look up a component by reference designator."""
        return self.to_netlist().get_component(reference)

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _schematic_instance_from_reference(
        self,
        instance_or_path: KiCadSchematicInstance | str,
        instances: list[KiCadSchematicInstance],
    ) -> KiCadSchematicInstance | None:
        if isinstance(instance_or_path, KiCadSchematicInstance):
            return instance_or_path
        normalized_sheet_path = _normalize_sheet_path(str(instance_or_path))
        normalized_instance_path = _normalize_sheet_instance_path(str(instance_or_path))
        for instance in instances:
            if _normalize_sheet_path(instance.sheet_path) == normalized_sheet_path:
                return instance
            if (
                normalized_instance_path is not None
                and _normalize_sheet_instance_path(instance.sheet_instance_path)
                == normalized_instance_path
            ):
                return instance
        return None

    def _compile_netlist(self) -> "KiCadNetlist":
        from .kicad_netlist_design import compile_design_netlist
        from .kicad_netlist_project import apply_project_net_classes

        top = self.top_schematic
        if top is None:
            raise ValueError(
                "cannot compile netlist: design has no top schematic loaded"
            )

        # Multi-unit ref suffixing follows the project's schematic
        # settings (subpart_first_id / subpart_id_separator). Defaults
        # (``A``, no separator) match KiCad's own defaults when no
        # ``.kicad_pro`` is loaded.
        subpart_first_id = ord("A")
        subpart_id_separator = 0
        project = self.project
        if project is not None:
            v = project.get_path("schematic.subpart_first_id")
            if isinstance(v, int):
                subpart_first_id = v
            v = project.get_path("schematic.subpart_id_separator")
            if isinstance(v, int):
                subpart_id_separator = v

        netlist = compile_design_netlist(
            top, self.text_variables,
            subpart_first_id=subpart_first_id,
            subpart_id_separator=subpart_id_separator,
        )
        # Project-side net classes are applied when a .kicad_pro is loaded.
        apply_project_net_classes(netlist, self.project)
        return netlist


__all__ = ["KiCadDesign", "KiCadSchematicInstance"]

"""
KiCad project-sidecar models for `.kicad_pro` JSON data.

This module intentionally covers the source-model surfaces that materially
affect PCB semantics today: net settings and text variables.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from pathlib import Path
from typing import TYPE_CHECKING, Any, Iterator

from ._api_markers import public_api

if TYPE_CHECKING:
    from .kicad_pcb import KiCadPcb
    from .kicad_project_libraries import KiCadLibraryTable
    from .kicad_schematic import KiCadSchematic


@dataclass
class KiCadProjectNetClassPattern:
    """Typed project-level netclass pattern assignment."""

    pattern: str = ""
    netclass_name: str = ""
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectNetClassPattern":
        return cls(
            pattern=str(data.get("pattern", "") or ""),
            netclass_name=str(data.get("netclass", "") or ""),
            raw=dict(data or {}),
        )


@dataclass
class KiCadProjectNetClass:
    """Typed KiCad net-class definition from `net_settings.classes[]`."""

    name: str = ""
    track_width: float | None = None
    clearance: float | None = None
    diff_pair_gap: float | None = None
    diff_pair_width: float | None = None
    diff_pair_via_gap: float | None = None
    via_diameter: float | None = None
    via_drill: float | None = None
    microvia_diameter: float | None = None
    microvia_drill: float | None = None
    bus_width: float | None = None
    wire_width: float | None = None
    pcb_color: str = ""
    schematic_color: str = ""
    line_style: int | None = None
    priority: int | None = None
    tuning_profile: str = ""
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectNetClass":
        return cls(
            name=str(data.get("name", "") or ""),
            track_width=float(data["track_width"]) if data.get("track_width") is not None else None,
            clearance=float(data["clearance"]) if data.get("clearance") is not None else None,
            diff_pair_gap=float(data["diff_pair_gap"]) if data.get("diff_pair_gap") is not None else None,
            diff_pair_width=float(data["diff_pair_width"]) if data.get("diff_pair_width") is not None else None,
            diff_pair_via_gap=float(data["diff_pair_via_gap"]) if data.get("diff_pair_via_gap") is not None else None,
            via_diameter=float(data["via_diameter"]) if data.get("via_diameter") is not None else None,
            via_drill=float(data["via_drill"]) if data.get("via_drill") is not None else None,
            microvia_diameter=float(data["microvia_diameter"]) if data.get("microvia_diameter") is not None else None,
            microvia_drill=float(data["microvia_drill"]) if data.get("microvia_drill") is not None else None,
            bus_width=float(data["bus_width"]) if data.get("bus_width") is not None else None,
            wire_width=float(data["wire_width"]) if data.get("wire_width") is not None else None,
            pcb_color=str(data.get("pcb_color", "") or ""),
            schematic_color=str(data.get("schematic_color", "") or ""),
            line_style=int(data["line_style"]) if data.get("line_style") is not None else None,
            priority=int(data["priority"]) if data.get("priority") is not None else None,
            tuning_profile=str(data.get("tuning_profile", "") or ""),
            raw=dict(data or {}),
        )


@dataclass
class KiCadProjectNetSettings:
    """Typed project-level KiCad net settings from `.kicad_pro`."""

    classes: list[KiCadProjectNetClass] = field(default_factory=list)
    netclass_assignments: dict[str, list[str]] = field(default_factory=dict)
    netclass_patterns: list[KiCadProjectNetClassPattern] = field(default_factory=list)
    net_colors: dict[str, str] = field(default_factory=dict)
    meta: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectNetSettings":
        assignments: dict[str, list[str]] = {}
        for net_name, class_names in (data.get("netclass_assignments", {}) or {}).items():
            assignments[str(net_name or "")] = [str(item or "") for item in (class_names or []) if str(item or "")]
        return cls(
            classes=[
                KiCadProjectNetClass.from_json_dict(item)
                for item in (data.get("classes", []) or [])
                if isinstance(item, dict)
            ],
            netclass_assignments=assignments,
            netclass_patterns=[
                KiCadProjectNetClassPattern.from_json_dict(item)
                for item in (data.get("netclass_patterns", []) or [])
                if isinstance(item, dict)
            ],
            net_colors={
                str(name or ""): str(value or "")
                for name, value in (data.get("net_colors", {}) or {}).items()
            },
            meta=dict(data.get("meta", {}) or {}),
        )


@dataclass
class KiCadProjectDiffPairDimensions:
    """Typed KiCad project diff-pair width/gap preset."""

    width: float | None = None
    gap: float | None = None
    via_gap: float | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectDiffPairDimensions":
        return cls(
            width=float(data["width"]) if data.get("width") is not None else None,
            gap=float(data["gap"]) if data.get("gap") is not None else None,
            via_gap=float(data["via_gap"]) if data.get("via_gap") is not None else None,
            raw=dict(data or {}),
        )


@dataclass
class KiCadProjectTuningPatternDefaults:
    """Typed KiCad tuning-pattern defaults for a given routing family."""

    spacing: float | None = None
    min_amplitude: float | None = None
    max_amplitude: float | None = None
    corner_style: int | None = None
    corner_radius_percentage: int | None = None
    single_sided: bool | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectTuningPatternDefaults":
        return cls(
            spacing=float(data["spacing"]) if data.get("spacing") is not None else None,
            min_amplitude=float(data["min_amplitude"]) if data.get("min_amplitude") is not None else None,
            max_amplitude=float(data["max_amplitude"]) if data.get("max_amplitude") is not None else None,
            corner_style=int(data["corner_style"]) if data.get("corner_style") is not None else None,
            corner_radius_percentage=(
                int(data["corner_radius_percentage"])
                if data.get("corner_radius_percentage") is not None
                else None
            ),
            single_sided=bool(data["single_sided"]) if data.get("single_sided") is not None else None,
            raw=dict(data or {}),
        )


@dataclass
class KiCadProjectTuningPatternSettings:
    """Typed KiCad tuning pattern settings from `board.design_settings`."""

    diff_pair_defaults: KiCadProjectTuningPatternDefaults | None = None
    diff_pair_skew_defaults: KiCadProjectTuningPatternDefaults | None = None
    single_track_defaults: KiCadProjectTuningPatternDefaults | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectTuningPatternSettings":
        return cls(
            diff_pair_defaults=KiCadProjectTuningPatternDefaults.from_json_dict(
                data.get("diff_pair_defaults", {}) or {}
            ),
            diff_pair_skew_defaults=KiCadProjectTuningPatternDefaults.from_json_dict(
                data.get("diff_pair_skew_defaults", {}) or {}
            ),
            single_track_defaults=KiCadProjectTuningPatternDefaults.from_json_dict(
                data.get("single_track_defaults", {}) or {}
            ),
            raw=dict(data or {}),
        )


@dataclass
class KiCadProjectBoardDesignSettings:
    """Typed board-side design settings surface relevant to PCB workflows."""

    diff_pair_dimensions: list[KiCadProjectDiffPairDimensions] = field(default_factory=list)
    tuning_pattern_settings: KiCadProjectTuningPatternSettings | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "KiCadProjectBoardDesignSettings":
        return cls(
            diff_pair_dimensions=[
                KiCadProjectDiffPairDimensions.from_json_dict(item)
                for item in (data.get("diff_pair_dimensions", []) or [])
                if isinstance(item, dict)
            ],
            tuning_pattern_settings=KiCadProjectTuningPatternSettings.from_json_dict(
                data.get("tuning_pattern_settings", {}) or {}
            ),
            raw=dict(data or {}),
        )


@dataclass(frozen=True)
class ProjectVariant:
    """One entry in `.kicad_pro` `schematic.variants` — the canonical
    project-side variant catalog.

    KiCad emits the catalog as a JSON list of ``{name, description}``
    objects; ``description`` is optional in the schema (KiCad omits it
    when blank). We surface it as ``None`` in that case so callers can
    distinguish "no description set" from "empty string".
    """

    name: str
    description: str | None = None

    @classmethod
    def from_json_dict(cls, data: dict[str, Any]) -> "ProjectVariant":
        name = str(data.get("name", "") or "")
        if "description" in data:
            description: str | None = str(data["description"]) if data["description"] is not None else None
        else:
            description = None
        return cls(name=name, description=description)


@dataclass
class KiCadProjectFiles:
    """Paths written by :meth:`KiCadProject.write_project`."""

    project_dir: Path
    project_file: Path
    schematic_file: Path | None = None
    worksheet_file: Path | None = None
    pcb_file: Path | None = None
    symbol_table: Path | None = None
    footprint_table: Path | None = None


@public_api
@dataclass
class KiCadProject:
    """Typed `.kicad_pro` project view and project-creation aggregate.

    This is the canonical reader for KiCad project files. The full
    parsed JSON is preserved verbatim in :attr:`raw` so the write/save path
    can round-trip without loss; typed views like :attr:`variants` and
    :attr:`net_settings` are derived from it.

    For scaffolding new project folders, use :meth:`create`. It starts a blank
    project bound to an on-disk directory; :meth:`add_schematic`,
    :meth:`add_pcb`, :meth:`add_symbol_library`, and
    :meth:`add_footprint_library` attach the pieces through the object model;
    :meth:`write_project` writes the folder. Higher layers drive these methods
    from their own input, so this class holds no knowledge of how inputs are
    gathered.
    """

    project_path: Path | None = None
    text_variables: dict[str, str] = field(default_factory=dict)
    net_settings: KiCadProjectNetSettings | None = None
    board_design_settings: KiCadProjectBoardDesignSettings | None = None
    variants: list[ProjectVariant] = field(default_factory=list)
    raw: dict[str, Any] = field(default_factory=dict)

    # --- new-project assembly state (aggregate builder) ---
    name: str = ""
    directory: Path | None = None
    schematic: "KiCadSchematic | None" = None
    pcb: "KiCadPcb | None" = None
    symbol_library_table: "KiCadLibraryTable | None" = None
    footprint_library_table: "KiCadLibraryTable | None" = None
    worksheet_path: Path | None = None

    def __post_init__(self) -> None:
        if self.project_path is not None:
            self.project_path = Path(self.project_path)
        if self.directory is not None:
            self.directory = Path(self.directory)

    @classmethod
    @public_api
    def from_json_dict(
        cls,
        data: dict[str, Any],
        *,
        project_path: Path | str | None = None,
    ) -> "KiCadProject":
        """Build a project view from a parsed `.kicad_pro` JSON dict."""
        return cls._from_raw(data, project_path=project_path)

    @classmethod
    def from_text(cls, text: str, *, project_path: Path | str | None = None) -> "KiCadProject":
        raw = json.loads(text)
        return cls._from_raw(raw, project_path=project_path)

    @classmethod
    def from_file(cls, path: Path | str) -> "KiCadProject":
        project_path = Path(path)
        raw = json.loads(project_path.read_text(encoding="utf-8"))
        return cls._from_raw(raw, project_path=project_path)

    @classmethod
    def _from_raw(cls, raw: Any, *, project_path: Path | str | None) -> "KiCadProject":
        if not isinstance(raw, dict):
            raise ValueError(".kicad_pro must be a JSON object")
        sch_block = raw.get("schematic", {}) or {}
        variants = [
            ProjectVariant.from_json_dict(item)
            for item in (sch_block.get("variants", []) or [])
            if isinstance(item, dict)
        ]
        return cls(
            project_path=Path(project_path) if project_path is not None else None,
            text_variables={
                str(name or ""): str(value or "")
                for name, value in (raw.get("text_variables", {}) or {}).items()
            },
            net_settings=KiCadProjectNetSettings.from_json_dict(raw.get("net_settings", {}) or {}),
            board_design_settings=KiCadProjectBoardDesignSettings.from_json_dict(
                ((raw.get("board", {}) or {}).get("design_settings", {}) or {})
            ),
            variants=variants,
            raw=dict(raw or {}),
        )

    # ------------------------------------------------------------------
    # Read helpers
    # ------------------------------------------------------------------

    def get_path(self, dotted_key: str, default: Any = None) -> Any:
        """Read a value out of :attr:`raw` by dotted JSON path.

        Example: ``project.get_path("meta.filename")`` →
        ``"variants.kicad_pro"``. Missing keys return *default*.
        """
        node: Any = self.raw
        for part in dotted_key.split("."):
            if not isinstance(node, dict) or part not in node:
                return default
            node = node[part]
        return node

    def get_text_variable(self, name: str, default: str = "") -> str:
        """Return a project text variable value."""
        return self.text_variables.get(str(name), default)

    def set_text_variable(self, name: str, value: str) -> None:
        """Set a project text variable in both typed state and raw JSON."""
        key = str(name)
        self.text_variables[key] = str(value)
        text_vars = self.raw.setdefault("text_variables", {})
        if not isinstance(text_vars, dict):
            raise TypeError(
                f"raw['text_variables'] must be a dict, got {type(text_vars).__name__}"
            )
        text_vars[key] = str(value)

    def remove_text_variable(self, name: str) -> bool:
        """Remove a project text variable by name."""
        key = str(name)
        removed = key in self.text_variables
        self.text_variables.pop(key, None)
        text_vars = self.raw.get("text_variables")
        if isinstance(text_vars, dict) and key in text_vars:
            del text_vars[key]
            removed = True
        return removed

    def get_variant(self, name: str) -> ProjectVariant | None:
        """Return a project variant by name."""
        for variant in self.variants:
            if variant.name == name:
                return variant
        return None

    def iter_variants(self) -> Iterator[ProjectVariant]:
        """Iterate over project variants."""
        return iter(self.variants)

    # ------------------------------------------------------------------
    # Write API
    # ------------------------------------------------------------------

    def to_text(self) -> str:
        """Serialize :attr:`raw` back to JSON, byte-equal with KiCad.

        KiCad writes ``.kicad_pro`` with ``nlohmann::json::dump(2)`` —
        2-space indent, key order preserved, UTF-8, trailing newline.
        ``json.dumps`` with ``indent=2, ensure_ascii=False`` matches
        nlohmann's output verbatim on every fixture in the upstream-QA
        mirror.
        """
        return json.dumps(self.raw, indent=2, ensure_ascii=False) + "\n"

    def to_json(self) -> dict[str, Any]:
        """Return the project JSON object preserved for round-trip writes."""
        return dict(self.raw)

    def save(self, path: Path | str | None = None) -> None:
        """Write :meth:`to_text` to disk.

        If *path* is omitted, write to :attr:`project_path` (set by
        :meth:`from_file`). Raises ``ValueError`` if neither is set.
        """
        target = Path(path) if path is not None else self.project_path
        if target is None:
            raise ValueError(
                "no path supplied to save() and project_path is unset; "
                "pass an explicit path"
            )
        target.write_text(self.to_text(), encoding="utf-8")

    def to_file(self, path: Path | str | None = None) -> None:
        """Deprecated alias for :meth:`save`."""
        self.save(path)

    def set_path(self, dotted_key: str, value: Any) -> None:
        """Set a value in :attr:`raw` by dotted JSON path.

        Intermediate dict nodes are auto-created. Existing non-dict
        values along the path raise ``TypeError`` so callers don't
        silently clobber a list or scalar.

        Example: ``project.set_path("meta.filename", "foo.kicad_pro")``.
        """
        if not dotted_key:
            raise ValueError("dotted_key must not be empty")
        parts = dotted_key.split(".")
        node: Any = self.raw
        for part in parts[:-1]:
            if part not in node:
                node[part] = {}
            elif not isinstance(node[part], dict):
                raise TypeError(
                    f"cannot descend into non-dict at {part!r} "
                    f"(have {type(node[part]).__name__})"
                )
            node = node[part]
        node[parts[-1]] = value

    # ------------------------------------------------------------------
    # New-project assembly (aggregate builder)
    # ------------------------------------------------------------------

    @classmethod
    @public_api
    def create(
        cls,
        name: str,
        directory: Path | str,
        *,
        project_path: Path | str | None = None,
    ) -> "KiCadProject":
        """Start a blank, writable project bound to an on-disk directory.

        This is the canonical public constructor for new project folders. It
        seeds KiCad's default ``.kicad_pro`` JSON and preserves the normal
        dataclass constructor for the existing project JSON facade.
        """
        if not name:
            raise ValueError("project name must not be empty")
        target_dir = Path(directory)
        target_project_path = (
            Path(project_path) if project_path is not None else target_dir / f"{name}.kicad_pro"
        )
        raw = _default_project_raw()
        raw["meta"]["filename"] = f"{name}.kicad_pro"
        project = cls._from_raw(raw, project_path=target_project_path)
        project.name = name
        project.directory = target_dir
        return project

    def add_schematic(
        self,
        *,
        sheet_name: str | None = None,
        page_size: str = "A4",
        revision: str = "",
        company: str = "",
        date: str = "",
        comments: list[str] | None = None,
    ) -> "KiCadSchematic":
        """Attach a blank top-level schematic and return it.

        The title block defaults its title to the sheet name, falling back to
        the project name.
        """
        import uuid

        from .kicad_sch_title_block import PaperSize, TitleBlock
        from .kicad_schematic import KiCadSchematic

        sch = KiCadSchematic()
        sch.uuid = str(uuid.uuid4())  # blank constructor leaves this empty
        sch.paper = PaperSize(size=page_size)
        sch.title_block = TitleBlock(
            title=sheet_name or self.name,
            rev=revision,
            company=company,
            date=date,
            comments={i + 1: c for i, c in enumerate(comments or []) if c},
        )
        self.schematic = sch
        return sch

    def set_worksheet(self, path: Path | str, *, embed: bool = True) -> None:
        """Reference (or embed) a ``.wks`` drawing sheet for the schematic.

        When ``embed`` is true the worksheet is packed into the schematic and
        referenced from the project as ``kicad-embed://``; otherwise it is
        referenced by its external path.
        """
        wks = Path(path)
        if embed:
            if self.schematic is None:
                raise ValueError("add_schematic() before embedding a worksheet")
            embedded = self.schematic.embed_worksheet(wks)
            self.set_path(
                "schematic.page_layout_descr_file", f"kicad-embed://{embedded.name}"
            )
        else:
            self.set_path("schematic.page_layout_descr_file", str(wks))
        self.worksheet_path = wks

    def add_pcb(self, *, page_size: str = "A4") -> "KiCadPcb":
        """Attach a blank board with KiCad's default layer stack and return it."""
        from .kicad_pcb import KiCadPcb

        self.pcb = KiCadPcb.new(paper=page_size)
        return self.pcb

    def _ensure_symbol_table(self) -> "KiCadLibraryTable":
        from .kicad_project_libraries import KiCadLibraryTable, KiCadLibraryTableKind

        if self.symbol_library_table is None:
            self.symbol_library_table = KiCadLibraryTable(
                kind=KiCadLibraryTableKind.SYMBOL, entries=[]
            )
        return self.symbol_library_table

    def _ensure_footprint_table(self) -> "KiCadLibraryTable":
        from .kicad_project_libraries import KiCadLibraryTable, KiCadLibraryTableKind

        if self.footprint_library_table is None:
            self.footprint_library_table = KiCadLibraryTable(
                kind=KiCadLibraryTableKind.FOOTPRINT, entries=[]
            )
        return self.footprint_library_table

    def ensure_library_tables(self) -> None:
        """Attach empty symbol and footprint library tables if not present."""
        self._ensure_symbol_table()
        self._ensure_footprint_table()

    def add_symbol_library(
        self, nickname: str, uri: str, *, library_type: str = "KiCad"
    ) -> Any:
        """Seed the project symbol library table with a referenced library."""
        from .kicad_project_libraries import KiCadLibraryTableEntry

        entry = KiCadLibraryTableEntry(name=nickname, uri=uri, library_type=library_type)
        self._ensure_symbol_table().entries.append(entry)
        return entry

    def add_footprint_library(
        self, nickname: str, uri: str, *, library_type: str = "KiCad"
    ) -> Any:
        """Seed the project footprint library table with a referenced library."""
        from .kicad_project_libraries import KiCadLibraryTableEntry

        entry = KiCadLibraryTableEntry(name=nickname, uri=uri, library_type=library_type)
        self._ensure_footprint_table().entries.append(entry)
        return entry

    def write_project(self, directory: Path | str | None = None) -> KiCadProjectFiles:
        """Write the whole project folder and return the written paths.

        Writes the ``.kicad_pro``, the attached schematic, any attached library
        tables, and the optional board. ``directory`` overrides
        :attr:`directory`.
        """
        from .kicad_project_libraries import KiCadLibraryTableKind

        target_dir = Path(directory) if directory is not None else self.directory
        if target_dir is None:
            raise ValueError("no directory supplied and KiCadProject.directory is unset")
        if not self.name:
            raise ValueError("KiCadProject.name must be set to write a project")
        if not self.raw:
            raise ValueError("project JSON is unset; use KiCadProject.create(...)")
        target_dir.mkdir(parents=True, exist_ok=True)

        project_file = target_dir / f"{self.name}.kicad_pro"
        self.save(project_file)

        schematic_file: Path | None = None
        if self.schematic is not None:
            schematic_file = target_dir / f"{self.name}.kicad_sch"
            self.schematic.save(schematic_file)

        symbol_table: Path | None = None
        if self.symbol_library_table is not None:
            symbol_table = target_dir / KiCadLibraryTableKind.SYMBOL.file_name
            self.symbol_library_table.write(symbol_table)

        footprint_table: Path | None = None
        if self.footprint_library_table is not None:
            footprint_table = target_dir / KiCadLibraryTableKind.FOOTPRINT.file_name
            self.footprint_library_table.write(footprint_table)

        pcb_file: Path | None = None
        if self.pcb is not None:
            pcb_file = target_dir / f"{self.name}.kicad_pcb"
            self.pcb.save(pcb_file)

        return KiCadProjectFiles(
            project_dir=target_dir,
            project_file=project_file,
            schematic_file=schematic_file,
            worksheet_file=self.worksheet_path,
            pcb_file=pcb_file,
            symbol_table=symbol_table,
            footprint_table=footprint_table,
        )

    # ---- Variant catalog mutators -------------------------------------

    def _variants_list(self, *, create: bool = False) -> list[dict[str, Any]]:
        """Return the live ``schematic.variants`` list inside ``raw``."""
        sch = self.raw.setdefault("schematic", {}) if create else (
            self.raw.get("schematic") or {}
        )
        if not isinstance(sch, dict):
            raise TypeError(
                f"raw['schematic'] must be a dict, got {type(sch).__name__}"
            )
        if create:
            v = sch.setdefault("variants", [])
        else:
            v = sch.get("variants") or []
        if not isinstance(v, list):
            raise TypeError(
                f"raw['schematic']['variants'] must be a list, got {type(v).__name__}"
            )
        return v

    def _refresh_typed_variants(self) -> None:
        """Rebuild :attr:`variants` from ``raw`` after a mutation."""
        self.variants = [
            ProjectVariant.from_json_dict(item)
            for item in self._variants_list()
            if isinstance(item, dict)
        ]

    def add_variant(
        self, name: str, description: str | None = None,
    ) -> ProjectVariant:
        """Append a variant to the catalog.

        ``description=None`` omits the key entirely (matches KiCad's
        own emit of variants without descriptions).
        """
        if not name:
            raise ValueError("variant name must not be empty")
        existing = self._variants_list(create=True)
        if any(item.get("name") == name for item in existing if isinstance(item, dict)):
            raise ValueError(f"variant {name!r} already exists")
        entry: dict[str, Any] = {"name": name}
        if description is not None:
            entry["description"] = description
        existing.append(entry)
        self._refresh_typed_variants()
        return self.variants[-1]

    def remove_variant(self, name: str) -> ProjectVariant | None:
        """Remove a variant by name; return the removed entry or None."""
        existing = self._variants_list()
        for i, item in enumerate(existing):
            if isinstance(item, dict) and item.get("name") == name:
                removed_raw = existing.pop(i)
                self._refresh_typed_variants()
                return ProjectVariant.from_json_dict(removed_raw)
        return None

    def rename_variant(self, old_name: str, new_name: str) -> bool:
        """Rename a variant in the catalog.

        Returns True if a rename occurred, False if *old_name* was not
        found. Raises ``ValueError`` if *new_name* already exists.

        Note: this mutates only the project-side catalog. PCB
        ``BoardVariant`` entries and per-symbol / per-footprint
        override blocks reference the variant **by name** elsewhere
        — those callers are responsible for updating their own state
        (see kicad_pcb / kicad_schematic).
        """
        if not new_name:
            raise ValueError("new_name must not be empty")
        existing = self._variants_list()
        names = [item.get("name") for item in existing if isinstance(item, dict)]
        if new_name in names:
            raise ValueError(f"variant {new_name!r} already exists")
        for item in existing:
            if isinstance(item, dict) and item.get("name") == old_name:
                item["name"] = new_name
                self._refresh_typed_variants()
                return True
        return False


# Backward-compat alias for callers that grew up with the
# PCB-workflow-named class. ``KiCadProject`` is the canonical name
# going forward; ``KiCadProjectSidecar`` continues to work but no
# longer carries any unique behavior.
KiCadProjectSidecar = KiCadProject


def _default_project_raw() -> dict[str, Any]:
    """Return a fresh copy of KiCad 10's default ``kicad.kicad_pro`` JSON.

    Key order matches KiCad's own emit so :meth:`KiCadProject.to_text` round-trips
    byte-equal with the stock template.
    """
    return {
        "board": {
            "design_settings": {
                "defaults": {},
                "diff_pair_dimensions": [],
                "drc_exclusions": [],
                "rules": {},
                "track_widths": [],
                "via_dimensions": [],
            }
        },
        "boards": [],
        "libraries": {
            "pinned_footprint_libs": [],
            "pinned_symbol_libs": [],
        },
        "meta": {
            "filename": "kicad.kicad_pro",
            "version": 1,
        },
        "net_settings": {
            "classes": [],
            "meta": {"version": 0},
        },
        "pcbnew": {"page_layout_descr_file": ""},
        "sheets": [],
        "text_variables": {},
    }


def find_adjacent_kicad_project_path(pcb_path: Path | str) -> Path | None:
    """Locate the most likely `.kicad_pro` sidecar for a PCB file."""
    pcb_path = Path(pcb_path)
    exact = pcb_path.with_suffix(".kicad_pro")
    if exact.is_file():
        return exact

    sibling_projects = sorted(pcb_path.parent.glob("*.kicad_pro"))
    if len(sibling_projects) == 1:
        return sibling_projects[0]

    pcb_stem = pcb_path.stem.lower()
    for candidate in sibling_projects:
        if candidate.stem.lower() == pcb_stem:
            return candidate

    return None


__all__ = [
    "KiCadProject",
    "KiCadProjectFiles",
    "KiCadProjectBoardDesignSettings",
    "KiCadProjectDiffPairDimensions",
    "KiCadProjectNetClass",
    "KiCadProjectNetClassPattern",
    "KiCadProjectNetSettings",
    "KiCadProjectSidecar",
    "KiCadProjectTuningPatternDefaults",
    "KiCadProjectTuningPatternSettings",
    "ProjectVariant",
    "find_adjacent_kicad_project_path",
]

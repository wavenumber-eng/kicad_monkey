"""KiCad project-local symbol and footprint library table helpers."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any

from .kicad_sexpr import QuotedString, SexpWriter, parse_sexp


class KiCadLibraryTableKind(Enum):
    """Supported KiCad project library table kinds."""

    SYMBOL = ("sym_lib_table", "sym-lib-table")
    FOOTPRINT = ("fp_lib_table", "fp-lib-table")

    @property
    def root_name(self) -> str:
        """Return the S-expression root element name."""
        return self.value[0]

    @property
    def file_name(self) -> str:
        """Return the KiCad table filename."""
        return self.value[1]


@dataclass(slots=True)
class KiCadLibraryTableEntry:
    """One ``(lib ...)`` entry in a KiCad symbol or footprint library table."""

    name: str
    uri: str
    library_type: str = "KiCad"
    options: str = ""
    descr: str = ""
    hidden: bool = False
    disabled: bool = False
    extra_items: tuple[list[Any], ...] = ()

    @classmethod
    def from_sexp(cls, sexp: list[Any]) -> KiCadLibraryTableEntry:
        """Parse one KiCad ``(lib ...)`` table entry."""
        if not sexp or sexp[0] != "lib":
            raise ValueError("KiCad library table entry must start with 'lib'")

        known = {"name", "type", "uri", "options", "descr", "hidden", "disabled"}
        data: dict[str, str] = {}
        hidden = False
        disabled = False
        extras: list[list[Any]] = []

        for child in sexp[1:]:
            if not isinstance(child, list) or not child:
                continue
            key = str(child[0])
            if key in {"hidden", "disabled"}:
                flag_value = True
                if len(child) > 1:
                    flag_value = str(child[1]).casefold() not in {"no", "false", "0"}
                if key == "hidden":
                    hidden = flag_value
                else:
                    disabled = flag_value
                continue
            if key in known:
                data[key] = str(child[1]) if len(child) > 1 else ""
                continue
            extras.append(child)

        name = data.get("name", "").strip()
        uri = data.get("uri", "").strip()
        if not name:
            raise ValueError("KiCad library table entry is missing a name")
        if not uri:
            raise ValueError(f"KiCad library table entry {name!r} is missing a URI")

        return cls(
            name=name,
            uri=uri,
            library_type=data.get("type", "KiCad"),
            options=data.get("options", ""),
            descr=data.get("descr", ""),
            hidden=hidden,
            disabled=disabled,
            extra_items=tuple(extras),
        )

    def to_sexp(self) -> list[Any]:
        """Serialize this entry as a KiCad ``(lib ...)`` S-expression."""
        result: list[Any] = [
            "lib",
            ["name", QuotedString(self.name)],
            ["type", QuotedString(self.library_type)],
            ["uri", QuotedString(self.uri)],
            ["options", QuotedString(self.options)],
            ["descr", QuotedString(self.descr)],
        ]
        if self.disabled:
            result.append(["disabled"])
        if self.hidden:
            result.append(["hidden"])
        result.extend(self.extra_items)
        return result


@dataclass(slots=True)
class KiCadLibraryTableEdit:
    """Result of ensuring one library table entry exists."""

    table_path: Path
    kind: KiCadLibraryTableKind
    nickname: str
    uri: str
    action: str

    @property
    def changed(self) -> bool:
        """Return whether the table was modified."""
        return self.action in {"added", "updated"}

    def to_dict(self) -> dict[str, object]:
        """Return a JSON-friendly summary."""
        return {
            "table": str(self.table_path),
            "kind": self.kind.name.lower(),
            "nickname": self.nickname,
            "uri": self.uri,
            "action": self.action,
            "changed": self.changed,
        }


@dataclass(slots=True)
class KiCadLibraryTable:
    """A KiCad ``sym-lib-table`` or ``fp-lib-table``."""

    kind: KiCadLibraryTableKind
    version: int = 7
    entries: list[KiCadLibraryTableEntry] = field(default_factory=list)

    @classmethod
    def from_file(cls, path: Path | str, kind: KiCadLibraryTableKind) -> KiCadLibraryTable:
        """Load a KiCad library table, or return an empty table when absent."""
        table_path = Path(path)
        if not table_path.exists():
            return cls(kind=kind)

        sexp = parse_sexp(table_path.read_text(encoding="utf-8"))
        if not isinstance(sexp, list) or not sexp or sexp[0] != kind.root_name:
            raise ValueError(f"{table_path} is not a {kind.root_name} table")

        version = 7
        entries: list[KiCadLibraryTableEntry] = []
        for child in sexp[1:]:
            if not isinstance(child, list) or not child:
                continue
            if child[0] == "version" and len(child) > 1:
                version = int(child[1])
            elif child[0] == "lib":
                entries.append(KiCadLibraryTableEntry.from_sexp(child))

        return cls(kind=kind, version=version, entries=entries)

    def ensure_entry(
        self,
        entry: KiCadLibraryTableEntry,
        *,
        table_path: Path,
        replace_existing: bool = False,
    ) -> KiCadLibraryTableEdit:
        """Ensure an entry exists, keyed by library nickname."""
        for index, existing in enumerate(self.entries):
            if existing.name != entry.name:
                continue
            if replace_existing and existing != entry:
                self.entries[index] = entry
                action = "updated"
            else:
                action = "already_exists"
            return KiCadLibraryTableEdit(
                table_path=table_path,
                kind=self.kind,
                nickname=entry.name,
                uri=entry.uri,
                action=action,
            )

        self.entries.append(entry)
        return KiCadLibraryTableEdit(
            table_path=table_path,
            kind=self.kind,
            nickname=entry.name,
            uri=entry.uri,
            action="added",
        )

    def to_sexp(self) -> list[Any]:
        """Serialize this table as a KiCad S-expression tree."""
        return [
            self.kind.root_name,
            ["version", self.version],
            *(entry.to_sexp() for entry in self.entries),
        ]

    def write(self, path: Path | str) -> None:
        """Write the table to disk."""
        table_path = Path(path)
        table_path.parent.mkdir(parents=True, exist_ok=True)
        table_path.write_text(SexpWriter().write(self.to_sexp()), encoding="utf-8")


@dataclass(slots=True)
class KiCadProjectLibraryTableResult:
    """Result of ensuring project symbol and footprint library table entries."""

    symbol: KiCadLibraryTableEdit
    footprint: KiCadLibraryTableEdit

    @property
    def changed(self) -> bool:
        """Return whether either project table changed."""
        return self.symbol.changed or self.footprint.changed

    def to_dict(self) -> dict[str, object]:
        """Return a JSON-friendly summary."""
        return {
            "changed": self.changed,
            "symbol": self.symbol.to_dict(),
            "footprint": self.footprint.to_dict(),
        }


def kicad_project_uri(project_path: Path | str, target_path: Path | str) -> str:
    """Return a KiCad URI, using ``${KIPRJMOD}`` when target is inside the project."""
    project_root = Path(project_path).resolve().parent
    target = Path(target_path).resolve()
    try:
        rel = target.relative_to(project_root)
    except ValueError:
        return target.as_posix()
    return "${KIPRJMOD}/" + rel.as_posix()


def ensure_project_library_table_entries(
    project_path: Path | str,
    *,
    symbol_library_path: Path | str,
    footprint_library_path: Path | str,
    symbol_nickname: str,
    footprint_nickname: str,
    replace_existing: bool = False,
) -> KiCadProjectLibraryTableResult:
    """Ensure project-local symbol and footprint tables contain library entries."""
    resolved_project = Path(project_path)
    project_root = resolved_project.resolve().parent

    sym_table_path = project_root / KiCadLibraryTableKind.SYMBOL.file_name
    fp_table_path = project_root / KiCadLibraryTableKind.FOOTPRINT.file_name

    symbol_entry = KiCadLibraryTableEntry(
        name=symbol_nickname,
        uri=kicad_project_uri(resolved_project, symbol_library_path),
        descr=f"{symbol_nickname} project symbols",
    )
    footprint_entry = KiCadLibraryTableEntry(
        name=footprint_nickname,
        uri=kicad_project_uri(resolved_project, footprint_library_path),
        descr=f"{footprint_nickname} project footprints",
    )

    sym_table = KiCadLibraryTable.from_file(sym_table_path, KiCadLibraryTableKind.SYMBOL)
    fp_table = KiCadLibraryTable.from_file(fp_table_path, KiCadLibraryTableKind.FOOTPRINT)

    symbol_update = sym_table.ensure_entry(
        symbol_entry,
        table_path=sym_table_path,
        replace_existing=replace_existing,
    )
    footprint_update = fp_table.ensure_entry(
        footprint_entry,
        table_path=fp_table_path,
        replace_existing=replace_existing,
    )

    if symbol_update.changed:
        sym_table.write(sym_table_path)
    if footprint_update.changed:
        fp_table.write(fp_table_path)

    return KiCadProjectLibraryTableResult(
        symbol=symbol_update,
        footprint=footprint_update,
    )


__all__ = [
    "KiCadLibraryTable",
    "KiCadLibraryTableEdit",
    "KiCadLibraryTableEntry",
    "KiCadLibraryTableKind",
    "KiCadProjectLibraryTableResult",
    "ensure_project_library_table_entries",
    "kicad_project_uri",
]

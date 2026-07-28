"""Project-local source relinking helpers for ``project-lib``."""

from __future__ import annotations

import importlib
import re
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, cast

from kicad_monkey.kicad_base import find_all_elements, find_element, unquote_string
from kicad_monkey.kicad_sexpr import QuotedString, SexpSelector, iter_sexp_form_spans

_QUOTED_OR_ATOM = r'"(?:\\.|[^"\\])*"|[^\s()]+'
_SCHEMATIC_LIB_ID_RE = re.compile(rf"(\(\s*lib_id\s+)(?P<value>{_QUOTED_OR_ATOM})")
_SCHEMATIC_FOOTPRINT_RE = re.compile(
    rf"(\(\s*property\s+\"Footprint\"\s+)(?P<value>{_QUOTED_OR_ATOM})"
)
_PCB_FOOTPRINT_RE = re.compile(rf"(\(\s*footprint\s+)(?P<value>{_QUOTED_OR_ATOM})")


@dataclass(frozen=True, slots=True)
class _Replacement:
    start: int
    end: int
    old: str
    new: str
    kind: str


class _SymbolRecordLike(Protocol):
    @property
    def name(self) -> str: ...

    @property
    def symbol(self) -> object: ...


class _FootprintRecordLike(Protocol):
    @property
    def name(self) -> str: ...

    @property
    def library_link(self) -> str: ...

    @property
    def footprint(self) -> object: ...


def _optional_library_extraction_function(name: str) -> object | None:
    try:
        module = importlib.import_module("kicad_monkey.kicad_library_extraction")
    except ImportError:
        return None
    return getattr(module, name, None)


def _library_member_name(name: str) -> str:
    return str(name).split(":", 1)[1] if ":" in str(name) else str(name)


def _safe_asset_filename(name: str) -> str:
    clean = _library_member_name(name)
    for char in r'<>:"/\|?*':
        clean = clean.replace(char, "_")
    for char in " \t\n\r":
        clean = clean.replace(char, "_")
    return clean or "unnamed"


def _unique_stem(stem: str, used: set[str]) -> str:
    candidate = stem
    index = 1
    while candidate.lower() in used:
        index += 1
        candidate = f"{stem}_{index}"
    used.add(candidate.lower())
    return candidate


def build_symbol_relink_map(records: Iterable[_SymbolRecordLike]) -> dict[str, str]:
    """Return original symbol references mapped to generated output members."""
    record_tuple = tuple(records)
    helper = _optional_library_extraction_function("build_symbol_output_member_map")
    if callable(helper):
        result = helper(record_tuple)
        if isinstance(result, dict):
            return {str(key): str(value) for key, value in result.items()}

    out: dict[str, str] = {}
    used: set[str] = set()
    for record in record_tuple:
        member = _unique_stem(_safe_asset_filename(str(record.name)), used)
        keys = {
            str(record.name),
            _library_member_name(str(record.name)),
            str(getattr(record.symbol, "name", "") or ""),
            _library_member_name(str(getattr(record.symbol, "name", "") or "")),
        }
        for key in keys:
            if key:
                out.setdefault(key, member)
    return out


def build_footprint_relink_map(records: Iterable[_FootprintRecordLike]) -> dict[str, str]:
    """Return original footprint references mapped to generated output members."""
    record_tuple = tuple(records)
    helper = _optional_library_extraction_function("build_footprint_output_member_map")
    if callable(helper):
        result = helper(record_tuple)
        if isinstance(result, dict):
            return {str(key): str(value) for key, value in result.items()}

    out: dict[str, str] = {}
    used: set[str] = set()
    for record in record_tuple:
        member = _unique_stem(_safe_asset_filename(str(record.name)), used)
        keys = {
            str(record.name),
            str(record.library_link),
            member,
            _library_member_name(str(record.name)),
            _library_member_name(str(record.library_link)),
            str(getattr(record.footprint, "name", "") or ""),
            _library_member_name(str(getattr(record.footprint, "name", "") or "")),
        }
        for key in keys:
            if key:
                out.setdefault(key, member)
    return out


def _local_library_link(
    value: str,
    *,
    library_nickname: str,
    member_map: dict[str, str],
) -> str:
    if not value or value.startswith(f"{library_nickname}:"):
        return value
    member = member_map.get(value) or member_map.get(_library_member_name(value))
    if member is None:
        return value
    return f"{library_nickname}:{member}"


def _quoted(value: str) -> str:
    return QuotedString(value).get_as_sexp()


def _replace_match_value(
    *,
    absolute_offset: int,
    match: re.Match[str],
    old: str,
    new: str,
    kind: str,
) -> _Replacement | None:
    token_start = match.start("value")
    token_end = match.end("value")
    old_token = match.group("value")
    if old_token not in {_quoted(old), old}:
        return None
    return _Replacement(
        start=absolute_offset + token_start,
        end=absolute_offset + token_end,
        old=old,
        new=new,
        kind=kind,
    )


def _schematic_lib_id_replacement(
    span_text: str,
    *,
    span_offset: int,
    sexp: list[object],
    symbol_library_nickname: str,
    symbol_member_map: dict[str, str],
) -> _Replacement | None:
    lib_id = find_element(sexp, "lib_id")
    if lib_id is None or len(lib_id) <= 1:
        return None
    old = unquote_string(lib_id[1])
    new = _local_library_link(
        old,
        library_nickname=symbol_library_nickname,
        member_map=symbol_member_map,
    )
    if new == old:
        return None
    match = _SCHEMATIC_LIB_ID_RE.search(span_text)
    if match is None:
        return None
    return _replace_match_value(
        absolute_offset=span_offset,
        match=match,
        old=old,
        new=_quoted(new),
        kind="schematic_symbol_lib_id",
    )


def _schematic_footprint_replacement(
    span_text: str,
    *,
    span_offset: int,
    prop: list[object],
    footprint_library_nickname: str,
    footprint_member_map: dict[str, str],
) -> _Replacement | None:
    if len(prop) < 3 or unquote_string(prop[1]) != "Footprint":
        return None
    old = unquote_string(prop[2])
    new = _local_library_link(
        old,
        library_nickname=footprint_library_nickname,
        member_map=footprint_member_map,
    )
    if new == old:
        return None
    match = _SCHEMATIC_FOOTPRINT_RE.search(span_text)
    if match is None:
        return None
    return _replace_match_value(
        absolute_offset=span_offset,
        match=match,
        old=old,
        new=_quoted(new),
        kind="schematic_symbol_footprint",
    )


def _schematic_replacements(
    text: str,
    *,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    symbol_member_map: dict[str, str],
    footprint_member_map: dict[str, str],
) -> list[_Replacement]:
    replacements: list[_Replacement] = []
    selector = SexpSelector(paths={("kicad_sch", "symbol")}, min_depth=1, max_depth=1)
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        span_text = span.text()
        lib_replacement = _schematic_lib_id_replacement(
            span_text,
            span_offset=span.start_offset,
            sexp=sexp,
            symbol_library_nickname=symbol_library_nickname,
            symbol_member_map=symbol_member_map,
        )
        if lib_replacement is not None:
            replacements.append(lib_replacement)

        for prop in find_all_elements(sexp, "property"):
            replacement = _schematic_footprint_replacement(
                span_text,
                span_offset=span.start_offset,
                prop=prop,
                footprint_library_nickname=footprint_library_nickname,
                footprint_member_map=footprint_member_map,
            )
            if replacement is not None:
                replacements.append(replacement)
    return replacements


def _pcb_replacements(
    text: str,
    *,
    footprint_library_nickname: str,
    footprint_member_map: dict[str, str],
) -> list[_Replacement]:
    replacements: list[_Replacement] = []
    selector = SexpSelector(paths={("kicad_pcb", "footprint")}, min_depth=1, max_depth=1)
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        if len(sexp) < 2:
            continue
        old = unquote_string(sexp[1])
        new = _local_library_link(
            old,
            library_nickname=footprint_library_nickname,
            member_map=footprint_member_map,
        )
        if new == old:
            continue
        match = _PCB_FOOTPRINT_RE.search(span.text())
        if match is None:
            continue
        replacement = _replace_match_value(
            absolute_offset=span.start_offset,
            match=match,
            old=old,
            new=_quoted(new),
            kind="pcb_footprint_library_link",
        )
        if replacement is not None:
            replacements.append(replacement)
    return replacements


def _apply_replacements(text: str, replacements: Iterable[_Replacement]) -> str:
    updated = text
    for replacement in sorted(replacements, key=lambda item: item.start, reverse=True):
        updated = updated[:replacement.start] + replacement.new + updated[replacement.end:]
    return updated


def _read_source_text(path: Path) -> str:
    with path.open("r", encoding="utf-8", newline="") as handle:
        return handle.read()


def _write_source_text(path: Path, text: str) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def _file_report(path: Path, replacements: list[_Replacement]) -> dict[str, object]:
    return {
        "path": str(path),
        "changed": bool(replacements),
        "change_count": len(replacements),
        "changes": [
            {
                "kind": replacement.kind,
                "offset": replacement.start,
                "old": replacement.old,
                "new": replacement.new.strip('"'),
            }
            for replacement in replacements
        ],
    }


def _project_source_files(project_path: Path) -> tuple[tuple[Path, ...], tuple[Path, ...]]:
    schematic_helper = _optional_library_extraction_function("iter_project_schematic_files")
    pcb_helper = _optional_library_extraction_function("iter_project_pcb_files")
    if not callable(schematic_helper) or not callable(pcb_helper):
        root = project_path.parent
        return (
            tuple(sorted(root.glob("*.kicad_sch"))),
            tuple(path for path in (root / f"{project_path.stem}.kicad_pcb",) if path.is_file()),
        )
    schematic_files = cast(Callable[[Path], Iterable[Path | str]], schematic_helper)
    pcb_files = cast(Callable[[Path], Iterable[Path | str]], pcb_helper)
    return (
        tuple(Path(path) for path in schematic_files(project_path)),
        tuple(Path(path) for path in pcb_files(project_path)),
    )


def relink_project_sources(
    *,
    project_path: Path,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    symbol_member_map: dict[str, str],
    footprint_member_map: dict[str, str],
    dry_run: bool,
) -> dict[str, object]:
    """Relink source schematic and PCB references to generated local libraries."""
    schematic_paths, pcb_paths = _project_source_files(project_path)
    files: list[dict[str, object]] = []
    files_changed = 0
    change_count = 0

    for path in (*schematic_paths, *pcb_paths):
        text = _read_source_text(path)
        if path.suffix == ".kicad_sch":
            replacements = _schematic_replacements(
                text,
                symbol_library_nickname=symbol_library_nickname,
                footprint_library_nickname=footprint_library_nickname,
                symbol_member_map=symbol_member_map,
                footprint_member_map=footprint_member_map,
            )
        elif path.suffix == ".kicad_pcb":
            replacements = _pcb_replacements(
                text,
                footprint_library_nickname=footprint_library_nickname,
                footprint_member_map=footprint_member_map,
            )
        else:
            replacements = []
        if replacements and not dry_run:
            _write_source_text(path, _apply_replacements(text, replacements))
        if replacements:
            files_changed += 1
            change_count += len(replacements)
        files.append(_file_report(path, replacements))

    return {
        "schema": "kicad_cruncher.source_relink.a0",
        "project": str(project_path),
        "mode": "dry_run" if dry_run else "apply",
        "changed": files_changed > 0,
        "summary": {
            "files_checked": len(files),
            "files_changed": files_changed,
            "changes": change_count,
        },
        "files": files,
    }

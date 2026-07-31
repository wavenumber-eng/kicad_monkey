"""Project-local source relinking helpers for ``project-lib``."""

from __future__ import annotations

import importlib
import re
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, cast

from kicad_monkey.kicad_base import find_all_elements, find_element, unquote_string
from kicad_monkey.kicad_sexpr import (
    QuotedString,
    SexpFormSpan,
    SexpSelector,
    iter_sexp_form_spans,
)

_QUOTED_OR_ATOM = r'"(?:\\.|[^"\\])*"|[^\s()]+'
_SCHEMATIC_LIB_ID_RE = re.compile(rf"(\(\s*lib_id\s+)(?P<value>{_QUOTED_OR_ATOM})")
_SCHEMATIC_LIB_NAME_RE = re.compile(rf"(\(\s*lib_name\s+)(?P<value>{_QUOTED_OR_ATOM})")
_SCHEMATIC_CACHE_SYMBOL_RE = re.compile(rf"(\(\s*symbol\s+)(?P<value>{_QUOTED_OR_ATOM})")
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


@dataclass(frozen=True, slots=True)
class _CacheLinkIssue:
    offset: int
    reference: str
    lib_id: str
    lib_name: str
    cache_lookup: str
    cache_lookup_source: str
    candidate_cache_names: tuple[str, ...]
    repair_candidate: str | None


@dataclass(frozen=True, slots=True)
class _CacheUnitIssue:
    offset: int
    parent_symbol: str
    child_symbol: str
    expected_prefix: str
    reason: str


@dataclass(frozen=True, slots=True)
class _SourceFilePlan:
    replacements: list[_Replacement]
    planned_text: str
    initial_link_issues: list[_CacheLinkIssue]
    remaining_link_issues: list[_CacheLinkIssue]
    initial_unit_issues: list[_CacheUnitIssue]
    remaining_unit_issues: list[_CacheUnitIssue]


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
        lib_name = find_element(sexp, "lib_name")
        lib_name_text = (
            unquote_string(lib_name[1]) if lib_name is not None and len(lib_name) > 1 else ""
        )
        if lib_name_text:
            lib_name_link = _local_library_link(
                lib_name_text,
                library_nickname=symbol_library_nickname,
                member_map=symbol_member_map,
            )
            if lib_name_link != lib_name_text:
                new = lib_name_link
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


def _schematic_reference(sexp: list[object]) -> str:
    for prop in find_all_elements(sexp, "property"):
        if len(prop) >= 3 and unquote_string(prop[1]) == "Reference":
            return unquote_string(prop[2])
    return ""


def _schematic_cache_symbol_names(text: str) -> set[str]:
    names: set[str] = set()
    selector = SexpSelector(
        paths={("kicad_sch", "lib_symbols", "symbol")},
        min_depth=2,
        max_depth=2,
    )
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        if len(sexp) > 1:
            names.add(unquote_string(sexp[1]))
    return names


def _cache_link_candidate_names(cache_names: set[str], lib_name: str) -> tuple[str, ...]:
    member = _library_member_name(lib_name)
    return tuple(
        sorted(
            name
            for name in cache_names
            if name != lib_name and _library_member_name(name) == member
        )
    )


def _schematic_symbol_cache_lookup(sexp: list[object]) -> tuple[str, str, str, str]:
    lib_name = find_element(sexp, "lib_name")
    lib_id = find_element(sexp, "lib_id")
    lib_id_text = unquote_string(lib_id[1]) if lib_id is not None and len(lib_id) > 1 else ""
    lib_name_text = (
        unquote_string(lib_name[1]) if lib_name is not None and len(lib_name) > 1 else ""
    )
    if lib_name_text:
        return lib_id_text, lib_name_text, lib_name_text, "lib_name"
    return lib_id_text, lib_name_text, lib_id_text, "lib_id"


def _cache_link_repair_candidate(
    *,
    cache_lookup_source: str,
    candidates: tuple[str, ...],
) -> str | None:
    if cache_lookup_source == "lib_name" and len(candidates) == 1:
        return candidates[0]
    return None


def _schematic_cache_link_issues(text: str) -> list[_CacheLinkIssue]:
    cache_names = _schematic_cache_symbol_names(text)
    issues: list[_CacheLinkIssue] = []
    selector = SexpSelector(paths={("kicad_sch", "symbol")}, min_depth=1, max_depth=1)
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        lib_id_text, lib_name_text, cache_lookup, cache_lookup_source = (
            _schematic_symbol_cache_lookup(sexp)
        )
        if not cache_lookup or cache_lookup in cache_names:
            continue
        candidates = _cache_link_candidate_names(cache_names, cache_lookup)
        issues.append(
            _CacheLinkIssue(
                offset=span.start_offset,
                reference=_schematic_reference(sexp),
                lib_id=lib_id_text,
                lib_name=lib_name_text,
                cache_lookup=cache_lookup,
                cache_lookup_source=cache_lookup_source,
                candidate_cache_names=candidates,
                repair_candidate=_cache_link_repair_candidate(
                    cache_lookup_source=cache_lookup_source,
                    candidates=candidates,
                ),
            )
        )
    return issues


def _cache_unit_issue_reason(child_name: str, expected_prefix: str) -> str | None:
    if not child_name.startswith(expected_prefix):
        return "prefix"
    if not child_name.startswith(f"{expected_prefix}_"):
        return "suffix"
    suffix = child_name[len(expected_prefix) + 1 :]
    tokens = suffix.split("_")
    if len(tokens) != 2:
        return "suffix"
    try:
        int(tokens[0])
        int(tokens[1])
    except ValueError:
        return "suffix"
    return None


def _schematic_cache_unit_issues(text: str) -> list[_CacheUnitIssue]:
    issues: list[_CacheUnitIssue] = []
    selector = SexpSelector(
        paths={("kicad_sch", "lib_symbols", "symbol")},
        min_depth=2,
        max_depth=2,
    )
    child_selector = SexpSelector(
        paths={("symbol", "symbol")},
        min_depth=1,
        max_depth=1,
    )
    for parent_span in iter_sexp_form_spans(text, selector):
        parent_sexp = cast(list[object], parent_span.parse())
        if len(parent_sexp) <= 1:
            continue
        parent_name = unquote_string(parent_sexp[1])
        expected_prefix = _library_member_name(parent_name)
        parent_text = parent_span.text()
        for child_span in iter_sexp_form_spans(parent_text, child_selector):
            child_sexp = cast(list[object], child_span.parse())
            if len(child_sexp) <= 1:
                continue
            child_name = unquote_string(child_sexp[1])
            reason = _cache_unit_issue_reason(child_name, expected_prefix)
            if reason is None:
                continue
            issues.append(
                _CacheUnitIssue(
                    offset=parent_span.start_offset + child_span.start_offset,
                    parent_symbol=parent_name,
                    child_symbol=child_name,
                    expected_prefix=expected_prefix,
                    reason=reason,
                )
            )
    return issues


def _cache_link_issue_report(path: Path, issue: _CacheLinkIssue) -> dict[str, object]:
    return {
        "path": str(path),
        "offset": issue.offset,
        "reference": issue.reference,
        "lib_id": issue.lib_id,
        "lib_name": issue.lib_name,
        "cache_lookup": issue.cache_lookup,
        "cache_lookup_source": issue.cache_lookup_source,
        "candidate_cache_names": list(issue.candidate_cache_names),
        "repair_candidate": issue.repair_candidate,
    }


def _cache_unit_issue_report(path: Path, issue: _CacheUnitIssue) -> dict[str, object]:
    return {
        "path": str(path),
        "offset": issue.offset,
        "parent_symbol": issue.parent_symbol,
        "child_symbol": issue.child_symbol,
        "expected_prefix": issue.expected_prefix,
        "reason": issue.reason,
    }


def _cache_link_validation_report(
    *,
    initial_issues_by_path: dict[Path, list[_CacheLinkIssue]],
    remaining_issues_by_path: dict[Path, list[_CacheLinkIssue]],
) -> dict[str, object]:
    initial_issues = [
        _cache_link_issue_report(path, issue)
        for path, issues in initial_issues_by_path.items()
        for issue in issues
    ]
    remaining_issues = [
        _cache_link_issue_report(path, issue)
        for path, issues in remaining_issues_by_path.items()
        for issue in issues
    ]
    repairable_count = sum(
        1
        for issues in initial_issues_by_path.values()
        for issue in issues
        if issue.repair_candidate is not None
    )
    return {
        "ok": not remaining_issues,
        "initial_issue_count": len(initial_issues),
        "remaining_issue_count": len(remaining_issues),
        "repairable_issue_count": repairable_count,
        "unrepairable_issue_count": len(remaining_issues),
        "issues": initial_issues,
        "remaining_issues": remaining_issues,
    }


def _cache_unit_validation_report(
    *,
    initial_issues_by_path: dict[Path, list[_CacheUnitIssue]],
    remaining_issues_by_path: dict[Path, list[_CacheUnitIssue]],
) -> dict[str, object]:
    initial_issues = [
        _cache_unit_issue_report(path, issue)
        for path, issues in initial_issues_by_path.items()
        for issue in issues
    ]
    remaining_issues = [
        _cache_unit_issue_report(path, issue)
        for path, issues in remaining_issues_by_path.items()
        for issue in issues
    ]
    return {
        "ok": not remaining_issues,
        "initial_issue_count": len(initial_issues),
        "remaining_issue_count": len(remaining_issues),
        "issues": initial_issues,
        "remaining_issues": remaining_issues,
    }


def _schematic_cache_link_replacement(
    span_text: str,
    *,
    span_offset: int,
    sexp: list[object],
    cache_names: set[str],
) -> _Replacement | None:
    lib_name = find_element(sexp, "lib_name")
    if lib_name is None or len(lib_name) <= 1:
        return None
    old = unquote_string(lib_name[1])
    if not old or old in cache_names:
        return None
    candidates = _cache_link_candidate_names(cache_names, old)
    if len(candidates) != 1:
        return None
    new = candidates[0]
    match = _SCHEMATIC_LIB_NAME_RE.search(span_text)
    if match is None:
        return None
    return _replace_match_value(
        absolute_offset=span_offset,
        match=match,
        old=old,
        new=_quoted(new),
        kind="schematic_symbol_lib_name",
    )


def _schematic_cache_symbol_relink_map(
    text: str,
    *,
    symbol_library_nickname: str,
    symbol_member_map: dict[str, str],
) -> dict[str, str]:
    relink_map: dict[str, str] = {}
    selector = SexpSelector(paths={("kicad_sch", "symbol")}, min_depth=1, max_depth=1)
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        lib_name = find_element(sexp, "lib_name")
        if lib_name is not None and len(lib_name) > 1 and unquote_string(lib_name[1]):
            continue
        lib_id = find_element(sexp, "lib_id")
        if lib_id is None or len(lib_id) <= 1:
            continue
        old = unquote_string(lib_id[1])
        new = _local_library_link(
            old,
            library_nickname=symbol_library_nickname,
            member_map=symbol_member_map,
        )
        if new != old:
            relink_map.setdefault(old, new)
    return relink_map


def _schematic_cache_symbol_replacements(
    text: str,
    *,
    cache_symbol_relink_map: dict[str, str],
) -> list[_Replacement]:
    replacements: list[_Replacement] = []
    if not cache_symbol_relink_map:
        return replacements
    existing_cache_names = _schematic_cache_symbol_names(text)
    planned_cache_names: set[str] = set()
    selector = SexpSelector(
        paths={("kicad_sch", "lib_symbols", "symbol")},
        min_depth=2,
        max_depth=2,
    )
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        if len(sexp) <= 1:
            continue
        old = unquote_string(sexp[1])
        new = cache_symbol_relink_map.get(old)
        if new is None or new == old:
            continue
        if new in existing_cache_names or new in planned_cache_names:
            continue
        match = _SCHEMATIC_CACHE_SYMBOL_RE.search(span.text())
        if match is None:
            continue
        replacement = _replace_match_value(
            absolute_offset=span.start_offset,
            match=match,
            old=old,
            new=_quoted(new),
            kind="schematic_cache_symbol_name",
        )
        if replacement is not None:
            replacements.append(replacement)
            planned_cache_names.add(new)
            replacements.extend(
                _schematic_cache_unit_symbol_replacements(
                    parent_span=span,
                    old_parent=old,
                    new_parent=new,
                )
            )
    return replacements


def _schematic_cache_unit_symbol_replacements(
    *,
    parent_span: SexpFormSpan,
    old_parent: str,
    new_parent: str,
) -> list[_Replacement]:
    replacements: list[_Replacement] = []
    old_member = _library_member_name(old_parent)
    new_member = _library_member_name(new_parent)
    if old_member == new_member:
        return replacements

    child_spans = _direct_cache_unit_symbol_spans(parent_span)
    child_names = _cache_unit_symbol_names(child_spans)
    planned_child_names: set[str] = set()
    for child_span in child_spans:
        old_child = _span_symbol_name(child_span)
        new_child = _cache_unit_child_relink_name(
            old_child=old_child,
            old_member=old_member,
            new_member=new_member,
            existing_names=child_names,
            planned_names=planned_child_names,
        )
        if new_child is None:
            continue
        replacement = _cache_symbol_name_replacement(
            absolute_offset=parent_span.start_offset + child_span.start_offset,
            old=old_child,
            new=new_child,
            span_text=child_span.text(),
        )
        if replacement is not None:
            replacements.append(replacement)
            planned_child_names.add(new_child)
    return replacements


def _direct_cache_unit_symbol_spans(parent_span: SexpFormSpan) -> list[SexpFormSpan]:
    child_selector = SexpSelector(
        paths={("symbol", "symbol")},
        min_depth=1,
        max_depth=1,
    )
    return list(iter_sexp_form_spans(parent_span.text(), child_selector))


def _span_symbol_name(span: SexpFormSpan) -> str:
    sexp = cast(list[object], span.parse())
    return unquote_string(sexp[1]) if len(sexp) > 1 else ""


def _cache_unit_symbol_names(child_spans: Iterable[SexpFormSpan]) -> set[str]:
    return {name for span in child_spans if (name := _span_symbol_name(span))}


def _cache_unit_child_relink_name(
    *,
    old_child: str,
    old_member: str,
    new_member: str,
    existing_names: set[str],
    planned_names: set[str],
) -> str | None:
    if old_child == old_member:
        new_child = new_member
    elif old_child.startswith(f"{old_member}_"):
        new_child = f"{new_member}{old_child[len(old_member) :]}"
    else:
        return None
    if new_child == old_child or new_child in existing_names or new_child in planned_names:
        return None
    return new_child


def _cache_symbol_name_replacement(
    *,
    absolute_offset: int,
    old: str,
    new: str,
    span_text: str,
) -> _Replacement | None:
    match = _SCHEMATIC_CACHE_SYMBOL_RE.search(span_text)
    if match is None:
        return None
    return _replace_match_value(
        absolute_offset=absolute_offset,
        match=match,
        old=old,
        new=_quoted(new),
        kind="schematic_cache_symbol_name",
    )


def _schematic_replacements(
    text: str,
    *,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    symbol_member_map: dict[str, str],
    footprint_member_map: dict[str, str],
    repair_cache_links: bool,
) -> list[_Replacement]:
    replacements: list[_Replacement] = []
    cache_symbol_replacements = _schematic_cache_symbol_replacements(
        text,
        cache_symbol_relink_map=_schematic_cache_symbol_relink_map(
            text,
            symbol_library_nickname=symbol_library_nickname,
            symbol_member_map=symbol_member_map,
        ),
    )
    replacements.extend(cache_symbol_replacements)
    cache_names = set()
    if repair_cache_links:
        cache_planned_text = (
            _apply_replacements(text, cache_symbol_replacements)
            if cache_symbol_replacements
            else text
        )
        cache_names = _schematic_cache_symbol_names(cache_planned_text)
    selector = SexpSelector(paths={("kicad_sch", "symbol")}, min_depth=1, max_depth=1)
    for span in iter_sexp_form_spans(text, selector):
        sexp = cast(list[object], span.parse())
        span_text = span.text()
        if repair_cache_links:
            cache_replacement = _schematic_cache_link_replacement(
                span_text,
                span_offset=span.start_offset,
                sexp=sexp,
                cache_names=cache_names,
            )
            if cache_replacement is not None:
                replacements.append(cache_replacement)

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
        updated = updated[: replacement.start] + replacement.new + updated[replacement.end :]
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


def _schematic_source_file_plan(
    text: str,
    *,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    symbol_member_map: dict[str, str],
    footprint_member_map: dict[str, str],
    repair_cache_links: bool,
) -> _SourceFilePlan:
    initial_link_issues = _schematic_cache_link_issues(text)
    initial_unit_issues = _schematic_cache_unit_issues(text)
    replacements = _schematic_replacements(
        text,
        symbol_library_nickname=symbol_library_nickname,
        footprint_library_nickname=footprint_library_nickname,
        symbol_member_map=symbol_member_map,
        footprint_member_map=footprint_member_map,
        repair_cache_links=repair_cache_links,
    )
    planned_text = _apply_replacements(text, replacements) if replacements else text
    return _SourceFilePlan(
        replacements=replacements,
        planned_text=planned_text,
        initial_link_issues=initial_link_issues,
        remaining_link_issues=_schematic_cache_link_issues(planned_text),
        initial_unit_issues=initial_unit_issues,
        remaining_unit_issues=_schematic_cache_unit_issues(planned_text),
    )


def _pcb_source_file_plan(
    text: str,
    *,
    footprint_library_nickname: str,
    footprint_member_map: dict[str, str],
) -> _SourceFilePlan:
    replacements = _pcb_replacements(
        text,
        footprint_library_nickname=footprint_library_nickname,
        footprint_member_map=footprint_member_map,
    )
    return _SourceFilePlan(
        replacements=replacements,
        planned_text=_apply_replacements(text, replacements) if replacements else text,
        initial_link_issues=[],
        remaining_link_issues=[],
        initial_unit_issues=[],
        remaining_unit_issues=[],
    )


def _source_file_plan(
    path: Path,
    text: str,
    *,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    symbol_member_map: dict[str, str],
    footprint_member_map: dict[str, str],
    repair_cache_links: bool,
) -> _SourceFilePlan:
    if path.suffix == ".kicad_sch":
        return _schematic_source_file_plan(
            text,
            symbol_library_nickname=symbol_library_nickname,
            footprint_library_nickname=footprint_library_nickname,
            symbol_member_map=symbol_member_map,
            footprint_member_map=footprint_member_map,
            repair_cache_links=repair_cache_links,
        )
    if path.suffix == ".kicad_pcb":
        return _pcb_source_file_plan(
            text,
            footprint_library_nickname=footprint_library_nickname,
            footprint_member_map=footprint_member_map,
        )
    return _SourceFilePlan(
        replacements=[],
        planned_text=text,
        initial_link_issues=[],
        remaining_link_issues=[],
        initial_unit_issues=[],
        remaining_unit_issues=[],
    )


def _source_relink_should_block(
    *,
    dry_run: bool,
    fail_on_cache_link_issues: bool,
    cache_link_validation: dict[str, object],
    cache_unit_validation: dict[str, object],
) -> bool:
    return (
        not dry_run
        and fail_on_cache_link_issues
        and (not bool(cache_link_validation["ok"]) or not bool(cache_unit_validation["ok"]))
    )


def relink_project_sources(
    *,
    project_path: Path,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    symbol_member_map: dict[str, str],
    footprint_member_map: dict[str, str],
    dry_run: bool,
    repair_cache_links: bool = False,
    fail_on_cache_link_issues: bool = False,
) -> dict[str, object]:
    """Relink source schematic and PCB references to generated local libraries."""
    schematic_paths, pcb_paths = _project_source_files(project_path)
    files: list[dict[str, object]] = []
    files_changed = 0
    change_count = 0
    pending_writes: list[tuple[Path, str]] = []
    initial_issues_by_path: dict[Path, list[_CacheLinkIssue]] = {}
    remaining_issues_by_path: dict[Path, list[_CacheLinkIssue]] = {}
    initial_unit_issues_by_path: dict[Path, list[_CacheUnitIssue]] = {}
    remaining_unit_issues_by_path: dict[Path, list[_CacheUnitIssue]] = {}

    for path in (*schematic_paths, *pcb_paths):
        text = _read_source_text(path)
        plan = _source_file_plan(
            path,
            text,
            symbol_library_nickname=symbol_library_nickname,
            footprint_library_nickname=footprint_library_nickname,
            symbol_member_map=symbol_member_map,
            footprint_member_map=footprint_member_map,
            repair_cache_links=repair_cache_links,
        )
        initial_issues_by_path[path] = plan.initial_link_issues
        remaining_issues_by_path[path] = plan.remaining_link_issues
        initial_unit_issues_by_path[path] = plan.initial_unit_issues
        remaining_unit_issues_by_path[path] = plan.remaining_unit_issues
        if plan.replacements and not dry_run:
            pending_writes.append((path, plan.planned_text))
        if plan.replacements:
            files_changed += 1
            change_count += len(plan.replacements)
        files.append(_file_report(path, plan.replacements))

    cache_link_validation = _cache_link_validation_report(
        initial_issues_by_path=initial_issues_by_path,
        remaining_issues_by_path=remaining_issues_by_path,
    )
    cache_unit_validation = _cache_unit_validation_report(
        initial_issues_by_path=initial_unit_issues_by_path,
        remaining_issues_by_path=remaining_unit_issues_by_path,
    )
    blocked = _source_relink_should_block(
        dry_run=dry_run,
        fail_on_cache_link_issues=fail_on_cache_link_issues,
        cache_link_validation=cache_link_validation,
        cache_unit_validation=cache_unit_validation,
    )
    if not blocked:
        for path, planned_text in pending_writes:
            _write_source_text(path, planned_text)

    return {
        "schema": "kicad_cruncher.source_relink.a0",
        "project": str(project_path),
        "mode": "dry_run" if dry_run else "apply",
        "applied": not dry_run and not blocked,
        "blocked": blocked,
        "changed": files_changed > 0,
        "summary": {
            "files_checked": len(files),
            "files_changed": files_changed,
            "changes": change_count,
        },
        "cache_link_validation": cache_link_validation,
        "cache_unit_validation": cache_unit_validation,
        "files": files,
    }

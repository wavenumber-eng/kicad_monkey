"""JSONC config loading and rendering helpers for command config files."""

from __future__ import annotations

import json
import re
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import cast

JsoncCommentMap = Mapping[tuple[str, ...] | str, str | Sequence[str]]

_TRAILING_COMMA_RE = re.compile(r",(?=\s*[}\]])")


def _copy_json_string(text: str, index: int, result: list[str]) -> int:
    result.append(text[index])
    index += 1
    escaped = False
    while index < len(text):
        char = text[index]
        result.append(char)
        index += 1
        if escaped:
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == '"':
            break
    return index


def _skip_line_comment(text: str, index: int) -> int:
    index += 2
    while index < len(text) and text[index] not in "\r\n":
        index += 1
    return index


def _skip_block_comment(text: str, index: int) -> int:
    index += 2
    while index + 1 < len(text) and not text.startswith("*/", index):
        index += 1
    return min(index + 2, len(text))


def _strip_jsonc_comments(text: str) -> str:
    """Remove JSONC comments while preserving quoted strings."""
    result: list[str] = []
    index = 0
    while index < len(text):
        char = text[index]
        if char == '"':
            index = _copy_json_string(text, index, result)
            continue
        if text.startswith("//", index):
            index = _skip_line_comment(text, index)
            continue
        if text.startswith("/*", index):
            index = _skip_block_comment(text, index)
            continue
        result.append(char)
        index += 1
    return "".join(result)


def load_json_config(path: Path) -> dict[str, object]:
    """Load a JSON or JSONC object config file."""
    text = path.read_text(encoding="utf-8-sig")
    without_comments = _strip_jsonc_comments(text)
    without_trailing_commas = _TRAILING_COMMA_RE.sub("", without_comments)
    payload = json.loads(without_trailing_commas)
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return cast(dict[str, object], payload)


def render_commented_jsonc(
    value: object,
    *,
    comments_by_path: JsoncCommentMap | None = None,
    comments_by_key: Mapping[str, str | Sequence[str]] | None = None,
    header_lines: Sequence[str] = (),
) -> str:
    """Render a JSON-compatible value with JSONC field comments.

    ``comments_by_path`` keys can be tuple paths such as
    ``("output", "mode")`` or dotted string paths such as ``"output.mode"``.
    ``comments_by_key`` is a fallback for repeated object keys.
    """
    normalized_path_comments = _normalize_comment_map(comments_by_path or {})
    text = _jsonc_dump_value(
        value,
        indent=0,
        path=(),
        comments_by_path=normalized_path_comments,
        comments_by_key=comments_by_key or {},
    )
    if not header_lines:
        return f"{text}\n"
    lines = [f"// {line}" if line else "//" for line in header_lines]
    lines.append(text)
    return "\n".join(lines) + "\n"


def enum_help(description: str, options: Sequence[str]) -> str:
    """Return standard help text for string/enum config fields."""
    return f"{description} Options: {', '.join(options)}."


def _normalize_comment_map(
    comments: JsoncCommentMap,
) -> dict[tuple[str, ...], str | Sequence[str]]:
    normalized: dict[tuple[str, ...], str | Sequence[str]] = {}
    for raw_path, comment in comments.items():
        if isinstance(raw_path, str):
            path = tuple(part for part in raw_path.split(".") if part)
        else:
            path = tuple(raw_path)
        normalized[path] = comment
    return normalized


def _jsonc_dump_value(
    value: object,
    *,
    indent: int,
    path: tuple[str, ...],
    comments_by_path: Mapping[tuple[str, ...], str | Sequence[str]],
    comments_by_key: Mapping[str, str | Sequence[str]],
) -> str:
    if isinstance(value, dict):
        return _jsonc_dump_object(
            value,
            indent=indent,
            path=path,
            comments_by_path=comments_by_path,
            comments_by_key=comments_by_key,
        )
    if isinstance(value, list):
        return _jsonc_dump_list(
            value,
            indent=indent,
            path=path,
            comments_by_path=comments_by_path,
            comments_by_key=comments_by_key,
        )
    return json.dumps(value)


def _jsonc_dump_object(
    value: Mapping[object, object],
    *,
    indent: int,
    path: tuple[str, ...],
    comments_by_path: Mapping[tuple[str, ...], str | Sequence[str]],
    comments_by_key: Mapping[str, str | Sequence[str]],
) -> str:
    if not value:
        return "{}"

    lines = ["{"]
    items = list(value.items())
    for index, (raw_key, item) in enumerate(items):
        key = str(raw_key)
        child_path = (*path, key)
        comment = _jsonc_comment_for_path(
            child_path,
            comments_by_path=comments_by_path,
            comments_by_key=comments_by_key,
        )
        if comment:
            lines.extend(_jsonc_comment_lines(comment, indent + 2))

        item_text = _jsonc_dump_value(
            item,
            indent=indent + 2,
            path=child_path,
            comments_by_path=comments_by_path,
            comments_by_key=comments_by_key,
        )
        item_lines = item_text.splitlines()
        prefix = f"{' ' * (indent + 2)}{json.dumps(key)}: "
        entry_lines = [prefix + item_lines[0]]
        entry_lines.extend(item_lines[1:])
        if index < len(items) - 1:
            entry_lines[-1] += ","
        lines.extend(entry_lines)
    lines.append(f"{' ' * indent}}}")
    return "\n".join(lines)


def _jsonc_dump_list(
    value: list[object],
    *,
    indent: int,
    path: tuple[str, ...],
    comments_by_path: Mapping[tuple[str, ...], str | Sequence[str]],
    comments_by_key: Mapping[str, str | Sequence[str]],
) -> str:
    if not value:
        return "[]"
    if all(not isinstance(item, dict | list) for item in value):
        return json.dumps(value)

    lines = ["["]
    for index, item in enumerate(value):
        item_path = path if isinstance(item, dict | list) else (*path, "*")
        item_text = _jsonc_dump_value(
            item,
            indent=indent + 2,
            path=item_path,
            comments_by_path=comments_by_path,
            comments_by_key=comments_by_key,
        )
        item_lines = item_text.splitlines()
        entry_lines = [f"{' ' * (indent + 2)}{item_lines[0]}"]
        entry_lines.extend(item_lines[1:])
        if index < len(value) - 1:
            entry_lines[-1] += ","
        lines.extend(entry_lines)
    lines.append(f"{' ' * indent}]")
    return "\n".join(lines)


def _jsonc_comment_for_path(
    path: tuple[str, ...],
    *,
    comments_by_path: Mapping[tuple[str, ...], str | Sequence[str]],
    comments_by_key: Mapping[str, str | Sequence[str]],
) -> str | Sequence[str]:
    return comments_by_path.get(path) or comments_by_key.get(path[-1], "")


def _jsonc_comment_lines(comment: str | Sequence[str], indent: int) -> list[str]:
    comment_lines = (
        [comment] if isinstance(comment, str) else [str(line) for line in comment]
    )
    return [f"{' ' * indent}/* {line} */" for line in comment_lines if line]


__all__ = ["enum_help", "load_json_config", "render_commented_jsonc"]

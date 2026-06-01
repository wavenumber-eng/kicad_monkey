"""JSONC config loading helpers for command config files."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import cast

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


__all__ = ["load_json_config"]

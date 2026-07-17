"""KiCad S-expression parsing and formatting utilities."""

from __future__ import annotations

from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
import re
from typing import Any, cast

from ._api_markers import public_api

SexpList = list[Any]


def _freeze_optional_strings(value: Iterable[str] | str | None) -> frozenset[str] | None:
    if value is None:
        return None
    if isinstance(value, str):
        return frozenset({value})
    return frozenset(str(item) for item in value)


def _freeze_paths(
    paths: Iterable[Iterable[str]] | None,
) -> frozenset[tuple[str, ...]] | None:
    if paths is None:
        return None
    return frozenset(tuple(str(part) for part in path) for path in paths)


def _unescape_kicad_string(s: str) -> str:
    """Decode KiCad/dsnlexer string escape sequences into raw characters.

    Mirrors ``DSNLEXER::readString`` (``kicad/common/dsnlexer.cpp:655``)
    which recognises ``\\\"``, ``\\\\``, ``\\a``/``\\b``/``\\f``/``\\n``/``\\r``/``\\t``/``\\v``
    and ``\\xNN`` (1-2 hex digits). Unknown escape sequences fall back
    to the raw backslash so we never silently mangle data.
    """
    out: list[str] = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c == '\\' and i + 1 < n:
            nxt = s[i + 1]
            if nxt in ('"', '\\'):
                out.append(nxt)
                i += 2
                continue
            simple = {
                'a': '\x07', 'b': '\x08', 'f': '\x0c',
                'n': '\n', 'r': '\r', 't': '\t', 'v': '\x0b',
            }
            if nxt in simple:
                out.append(simple[nxt])
                i += 2
                continue
            if nxt == 'x':
                hex_chars = ''
                j = i + 2
                while j < n and j - (i + 2) < 2 and s[j] in '0123456789abcdefABCDEF':
                    hex_chars += s[j]
                    j += 1
                if hex_chars:
                    out.append(chr(int(hex_chars, 16)))
                    i = j
                    continue
                out.append('x')
                i += 2
                continue
            if '0' <= nxt <= '7':
                oct_chars = nxt
                j = i + 2
                while j < n and j - (i + 1) < 3 and '0' <= s[j] <= '7':
                    oct_chars += s[j]
                    j += 1
                out.append(chr(int(oct_chars, 8)))
                i = j
                continue
        out.append(c)
        i += 1
    return ''.join(out)


def _escape_kicad_string(s: str) -> str:
    """Encode a raw string for KiCad's S-expression output.

    Mirrors ``OUTPUTFORMATTER::Quotes`` (``kicad/common/richio.cpp:472``)
    which escapes only ``\\n``, ``\\r``, ``\\\\`` and ``\\\"``. All other
    characters — including tabs and other control characters — pass
    through verbatim, matching KiCad's canonical output exactly.
    """
    out: list[str] = []
    for c in s:
        if c == '\n':
            out.append('\\n')
        elif c == '\r':
            out.append('\\r')
        elif c == '\\':
            out.append('\\\\')
        elif c == '"':
            out.append('\\"')
        else:
            out.append(c)
    return ''.join(out)


class QuotedString(str):
    """
    A string class that represents a quoted string in S-expressions.
    This string will be properly escaped and quoted when serialized to S-expr format.
    """

    def __new__(cls, value: str = "") -> "QuotedString":
        # Store the original unescaped value
        return super().__new__(cls, value)

    def get_as_sexp(self) -> str:
        """
        Returns the string properly escaped and quoted for S-expression output.
        """
        if len(self) == 0:
            return '""'
        return f'"{_escape_kicad_string(self)}"'

    def __repr__(self) -> str:
        return f"QuotedString({super().__repr__()})"


class FormattedDataBlock(str):
    """
    A string that should be output verbatim, preserving internal formatting.

    Used for base64 data blocks with KiCad-style line wrapping:
    - Each line is on its own line
    - First line starts with |
    - Last line ends with |
    - 76 characters per line (MIME_BASE64_LENGTH)

    Example output:
        (data
        |eJztwTEBAAAAwqDvvvvhBkAAAAAAAL4G
        AAAAAABJRMb1AAAAAAAAAAAAAAAAACAA
        AAAAAAAAAAAAAAAAAAIAAAB1pO7AAA==|
        )
    """

    def __new__(cls, value: str = "") -> "FormattedDataBlock":
        return super().__new__(cls, value)

    def __repr__(self) -> str:
        return f"FormattedDataBlock({len(self)} chars)"


def validate_bare_string(s: str) -> None:
    """
    Validates that a bare string doesn't contain characters that require quoting.
    Raises SexprError if the string contains problematic characters.
    """
    if not s:
        raise SexprError("Bare string cannot be empty - use QuotedString for empty strings")

    # Check for characters that require quoting
    if re.search(r'[\s()\"]', s):
        problematic_chars = []
        if ' ' in s or '\t' in s or '\n' in s:
            problematic_chars.append("whitespace")
        if '(' in s or ')' in s:
            problematic_chars.append("parentheses")
        if '"' in s:
            problematic_chars.append("quotes")

        raise SexprError(
            f"Bare string '{s}' contains {', '.join(problematic_chars)} - "
            f"use QuotedString instead"
        )

class SexprError(ValueError):
    """Parse / lex / build error with optional source-location metadata.

    Older callers can keep using ``SexprError(message)`` and matching on
    the message substring. Newer callers pass structured fields
    (``offset``/``line``/``column``/``source_path``/``token_text``) so
    tooling can surface ``file:line:col`` diagnostics or jump to source.
    ``str(exc)`` is the original message with a deterministic location
    suffix when available; the leading message text is preserved so
    existing ``pytest.raises(match=...)`` substring checks keep working.

    Subclasses pin the parse stage that raised:

    * :class:`SexprLexError` — token-level errors from
      :class:`KicadSexprLexer` (unterminated string, unexpected token).
    * :class:`SexprTreeError` — list-tree errors from
      :class:`KicadSexprParser` (missing opener, unbalanced parens,
      unexpected EOF inside a list).
    * :class:`SexprDialectError` — KiCad-specific dialect forms that the
      parser intentionally rejects (reserved for future use; today every
      entry in :data:`PARSER_DIALECT_EXCEPTIONS` is *accepted*).

    Each subclass advertises its origin via the ``phase`` class attribute
    so callers catching the parent can still distinguish stages
    (``except SexprError as e: stage = e.phase``). ``phase`` aligns with
    :class:`SexpRoundtripResult.phase` (``"lex"`` / ``"tree"`` /
    ``"dialect"``).
    """

    phase: str = "unknown"

    def __init__(
        self,
        message: str,
        *,
        offset: int | None = None,
        line: int | None = None,
        column: int | None = None,
        source_path: Any = None,
        token_text: str | None = None,
    ) -> None:
        self.message = message
        self.offset = offset
        self.line = line
        self.column = column
        self.source_path = str(source_path) if source_path is not None else None
        self.token_text = token_text
        super().__init__(self._format())

    def _format(self) -> str:
        out = self.message
        if self.line is not None and self.column is not None:
            out = f"{out} at line {self.line}, column {self.column}"
        elif self.offset is not None:
            out = f"{out} at position {self.offset}"
        if self.source_path is not None:
            out = f"{out} in {self.source_path}"
        if self.token_text:
            out = f"{out} near {self.token_text!r}"
        return out

    def with_source_path(self, source_path: Any) -> "SexprError":
        """Return a copy carrying ``source_path`` for nicer diagnostics.

        Returns a fresh instance of the same concrete subclass so the
        ``phase`` attribute is preserved across the copy.
        """
        if source_path is None:
            return self
        return type(self)(
            self.message,
            offset=self.offset,
            line=self.line,
            column=self.column,
            source_path=source_path,
            token_text=self.token_text,
        )


class SexprLexError(SexprError):
    """Token-level error raised by :class:`KicadSexprLexer`."""

    phase = "lex"


class SexprTreeError(SexprError):
    """List-tree error raised by :class:`KicadSexprParser`."""

    phase = "tree"


class SexprDialectError(SexprError):
    """Dialect-rejected form (reserved; no current call sites)."""

    phase = "dialect"


TOKEN_LEFT = "left"
TOKEN_RIGHT = "right"
TOKEN_ATOM = "atom"
TOKEN_STRING = "string"
TOKEN_NUMBER = "number"

_NUMBER_TOKEN_PATTERN = (
    r"[+-]?(?:(?:[0-9]+(?:\.[0-9]*)?)|(?:\.[0-9]+))(?:[eE][+-]?[0-9]+)?"
)
_NEWLINE_RE = re.compile(r"\r\n|\r|\n")
_SEXPR_TOKEN_RE = re.compile(
    rf"(?P<space>\s+)"
    rf"|(?P<left>\()"
    rf"|(?P<right>\))"
    rf"|(?P<string>\"(?:\\(?:\r\n|[\s\S])|[^\"\\])*\")"
    rf"|(?P<unterminated_string>\"(?:\\(?:\r\n|[\s\S])|[^\"\\])*)"
    rf"|(?P<number>{_NUMBER_TOKEN_PATTERN})(?=$|[\s()])"
    rf"|(?P<atom>[^\s()]+)"
)
_SEXPR_TOKEN_RE_KNOWS_BAR = re.compile(
    rf"(?P<space>\s+)"
    rf"|(?P<left>\()"
    rf"|(?P<right>\))"
    rf"|(?P<string>\"(?:\\(?:\r\n|[\s\S])|[^\"\\])*\")"
    rf"|(?P<unterminated_string>\"(?:\\(?:\r\n|[\s\S])|[^\"\\])*)"
    rf"|(?P<number>{_NUMBER_TOKEN_PATTERN})(?=$|[\s()|])"
    rf"|(?P<atom>[^\s()|]+)"
)


@dataclass(frozen=True)
class SexpToken:
    """A KiCad DSN-style lexical token with source location metadata."""

    kind: str
    text: str
    value: Any
    offset: int
    line: int
    column: int
    separator: str = ""


@public_api
@dataclass(frozen=True)
class SexpSelector:
    """Read-only selector for projection scanning S-expression forms."""

    heads: frozenset[str] | Iterable[str] | str | None = None
    paths: frozenset[tuple[str, ...]] | Iterable[Iterable[str]] | None = None
    min_depth: int | None = None
    max_depth: int | None = None
    prune_heads: frozenset[str] | Iterable[str] | str = frozenset()

    def __post_init__(self) -> None:
        object.__setattr__(self, "heads", _freeze_optional_strings(self.heads))
        object.__setattr__(self, "paths", _freeze_paths(self.paths))
        object.__setattr__(
            self,
            "prune_heads",
            _freeze_optional_strings(self.prune_heads) or frozenset(),
        )
        if self.min_depth is not None and self.min_depth < 0:
            raise ValueError("SexpSelector.min_depth must be non-negative")
        if self.max_depth is not None and self.max_depth < 0:
            raise ValueError("SexpSelector.max_depth must be non-negative")
        if (
            self.min_depth is not None
            and self.max_depth is not None
            and self.min_depth > self.max_depth
        ):
            raise ValueError("SexpSelector.min_depth cannot exceed max_depth")

    def matches(self, span: "SexpFormSpan") -> bool:
        """Return whether ``span`` matches this selector."""
        heads = cast(frozenset[str] | None, self.heads)
        paths = cast(frozenset[tuple[str, ...]] | None, self.paths)
        if heads is not None and span.head not in heads:
            return False
        if paths is not None and span.path not in paths:
            return False
        if self.min_depth is not None and span.depth < self.min_depth:
            return False
        if self.max_depth is not None and span.depth > self.max_depth:
            return False
        return True

    def should_scan_children(
        self,
        *,
        head: str | None,
        path: tuple[str, ...],
        depth: int,
    ) -> bool:
        """Return whether nested forms below this form can match."""
        prune_heads = cast(frozenset[str], self.prune_heads)
        if head is not None and head in prune_heads:
            return False
        if self.paths is not None and not self._can_any_path_match_child(path):
            return False
        return self.max_depth is None or depth < self.max_depth

    def _can_any_path_match_child(self, path: tuple[str, ...]) -> bool:
        """Return whether a descendant of ``path`` can match an exact path."""
        assert self.paths is not None
        next_len = len(path) + 1
        paths = cast(frozenset[tuple[str, ...]], self.paths)
        for target in paths:
            if len(target) < next_len:
                continue
            if target[:len(path)] == path:
                return True
        return False


@public_api
@dataclass(frozen=True)
class SexpFormSpan:
    """Source span for one complete selected S-expression form."""

    head: str | None
    path: tuple[str, ...]
    depth: int
    start_offset: int
    end_offset: int
    line: int
    column: int
    end_line: int
    end_column: int
    source_text: str | None = field(default=None, repr=False, compare=False)
    source_path: str | None = None

    def text(self) -> str:
        """Return the exact source text for this form."""
        if self.source_text is None:
            raise ValueError("SexpFormSpan has no source_text")
        return self.source_text[self.start_offset:self.end_offset]

    def parse(self) -> Any:
        """Parse this selected form into the normal nested-list representation."""
        return parse_sexp(self.text(), source_path=self.source_path)


class _SexpProjectionScanner:
    """Character-level scanner for complete S-expression form spans."""

    def __init__(
        self,
        text: str,
        selector: SexpSelector,
        *,
        source_path: Any = None,
    ) -> None:
        self.text = text
        self.selector = selector
        self.source_path = str(source_path) if source_path is not None else None
        self.pos = 0
        self.line = 1
        self.column = 1
        self.line_start = 0

    def scan(self) -> list[SexpFormSpan]:
        """Return selected spans in source order."""
        spans: list[SexpFormSpan] = []
        while True:
            self._skip_space_and_comments()
            if self.pos >= len(self.text):
                return spans
            if self.text[self.pos] == ")":
                raise SexprTreeError(
                    "Unbalanced closing parenthesis",
                    offset=self.pos,
                    line=self.line,
                    column=self.column,
                    source_path=self.source_path,
                )
            if self.text[self.pos] != "(":
                raise SexprTreeError(
                    "Missing initial opening parenthesis",
                    offset=self.pos,
                    line=self.line,
                    column=self.column,
                    source_path=self.source_path,
                    token_text=self.text[self.pos:self.pos + 1],
                )
            spans.extend(self._scan_form(parent_path=(), depth=0))

    def _scan_form(
        self,
        *,
        parent_path: tuple[str, ...],
        depth: int,
    ) -> list[SexpFormSpan]:
        start_offset = self.pos
        start_line = self.line
        start_column = self.column
        self._consume_expected("(")
        head = self._read_form_head()
        path = (*parent_path, head) if head is not None else parent_path
        children = self._scan_form_body(head=head, path=path, depth=depth)
        end_offset = self.pos
        span = SexpFormSpan(
            head=head,
            path=path,
            depth=depth,
            start_offset=start_offset,
            end_offset=end_offset,
            line=start_line,
            column=start_column,
            end_line=self.line,
            end_column=self.column,
            source_text=self.text,
            source_path=self.source_path,
        )
        if self.selector.matches(span):
            return [span, *children]
        return children

    def _scan_form_body(
        self,
        *,
        head: str | None,
        path: tuple[str, ...],
        depth: int,
    ) -> list[SexpFormSpan]:
        children: list[SexpFormSpan] = []
        scan_children = self.selector.should_scan_children(
            head=head,
            path=path,
            depth=depth,
        )
        while True:
            self._skip_space_and_comments()
            if self.pos >= len(self.text):
                raise SexprTreeError(
                    "Unbalanced opening parenthesis",
                    offset=self.pos,
                    line=self.line,
                    column=self.column,
                    source_path=self.source_path,
                )
            ch = self.text[self.pos]
            if ch == ")":
                self._consume_char()
                return children
            if ch == "(":
                if scan_children:
                    children.extend(self._scan_form(parent_path=path, depth=depth + 1))
                else:
                    self._skip_balanced_form()
                continue
            self._skip_scalar()

    def _read_form_head(self) -> str | None:
        self._skip_space_and_comments()
        if self.pos >= len(self.text) or self.text[self.pos] in "()":
            return None
        if self.text[self.pos] == '"':
            return self._read_quoted_string()
        return self._read_atom()

    def _skip_balanced_form(self) -> None:
        if self.pos >= len(self.text) or self.text[self.pos] != "(":
            return
        depth = 0
        while self.pos < len(self.text):
            self._skip_space_and_comments()
            if self.pos >= len(self.text):
                break
            ch = self.text[self.pos]
            if ch == '"':
                self._read_quoted_string()
                continue
            if ch == "(":
                depth += 1
                self._consume_char()
                continue
            if ch == ")":
                depth -= 1
                self._consume_char()
                if depth == 0:
                    return
                continue
            self._skip_atom()
        raise SexprTreeError(
            "Unbalanced opening parenthesis",
            offset=self.pos,
            line=self.line,
            column=self.column,
            source_path=self.source_path,
        )

    def _skip_scalar(self) -> None:
        if self.text[self.pos] == '"':
            self._read_quoted_string()
        else:
            self._skip_atom()

    def _read_atom(self) -> str:
        start_offset = self.pos
        text = self.text
        text_len = len(text)
        pos = self.pos

        while pos < text_len:
            ch = text[pos]
            if ch.isspace() or ch in "()":
                break
            pos += 1
        if pos == start_offset:
            raise SexprLexError(
                "Unexpected token",
                offset=start_offset,
                line=self.line,
                column=self.column,
                source_path=self.source_path,
            )
        self.pos = pos
        self.column += pos - start_offset
        return text[start_offset:pos]

    def _skip_atom(self) -> None:
        start_offset = self.pos
        text = self.text
        text_len = len(text)
        pos = self.pos

        while pos < text_len:
            ch = text[pos]
            if ch.isspace() or ch in "()":
                break
            pos += 1
        if pos == start_offset:
            raise SexprLexError(
                "Unexpected token",
                offset=start_offset,
                line=self.line,
                column=self.column,
                source_path=self.source_path,
            )
        self.pos = pos
        self.column += pos - start_offset

    def _read_quoted_string(self) -> str:
        start_offset = self.pos
        start_line = self.line
        start_column = self.column
        raw: list[str] = []
        text = self.text
        text_len = len(text)
        pos = self.pos + 1
        line = self.line
        column = self.column + 1
        line_start = self.line_start

        while pos < text_len:
            ch = text[pos]
            if ch == "\r":
                pos += 1
                if pos < text_len and text[pos] == "\n":
                    pos += 1
                ch = "\n"
                line += 1
                column = 1
                line_start = pos
            else:
                pos += 1
                if ch == "\n":
                    line += 1
                    column = 1
                    line_start = pos
                else:
                    column += 1

            if ch == '"':
                self.pos = pos
                self.line = line
                self.column = column
                self.line_start = line_start
                return _unescape_kicad_string("".join(raw))
            if ch == "\\":
                raw.append(ch)
                if pos >= text_len:
                    break
                escaped = text[pos]
                if escaped == "\r":
                    pos += 1
                    if pos < text_len and text[pos] == "\n":
                        pos += 1
                    escaped = "\n"
                    line += 1
                    column = 1
                    line_start = pos
                else:
                    pos += 1
                    if escaped == "\n":
                        line += 1
                        column = 1
                        line_start = pos
                    else:
                        column += 1
                raw.append(escaped)
                continue
            raw.append(ch)

        self.pos = pos
        self.line = line
        self.column = column
        self.line_start = line_start
        raise SexprLexError(
            "Unterminated delimited string",
            offset=start_offset,
            line=start_line,
            column=start_column,
            source_path=self.source_path,
        )

    def _skip_space_and_comments(self) -> None:
        text = self.text
        text_len = len(text)
        pos = self.pos
        line = self.line
        column = self.column
        line_start = self.line_start

        while True:
            start_pos = pos
            while pos < text_len and text[pos].isspace():
                ch = text[pos]
                if ch == "\r":
                    pos += 1
                    if pos < text_len and text[pos] == "\n":
                        pos += 1
                    line += 1
                    column = 1
                    line_start = pos
                    continue

                pos += 1
                if ch == "\n":
                    line += 1
                    column = 1
                    line_start = pos
                else:
                    column += 1

            if pos < text_len and text[pos] == "#" and text[line_start:pos].strip() == "":
                while pos < text_len and text[pos] not in "\r\n":
                    pos += 1
                    column += 1
                continue

            if pos == start_pos:
                self.pos = pos
                self.line = line
                self.column = column
                self.line_start = line_start
                return

    def _consume_expected(self, expected: str) -> None:
        if self.pos >= len(self.text) or self.text[self.pos] != expected:
            raise SexprLexError(
                f"Expected {expected!r}",
                offset=self.pos,
                line=self.line,
                column=self.column,
                source_path=self.source_path,
            )
        self._consume_char()

    def _consume_char(self) -> str:
        ch = self.text[self.pos]
        if ch == "\r":
            self.pos += 1
            if self.pos < len(self.text) and self.text[self.pos] == "\n":
                self.pos += 1
            self.line += 1
            self.column = 1
            self.line_start = self.pos
            return "\n"
        self.pos += 1
        if ch == "\n":
            self.line += 1
            self.column = 1
            self.line_start = self.pos
        else:
            self.column += 1
        return ch


class KicadSexprLexer:
    """KiCad-mode DSN/S-expression lexer.

    This mirrors the subset of ``DSNLEXER`` used by KiCad PCB/SCH/SYM files:
    parentheses are separators, quoted strings use KiCad escape decoding, and
    comments are whole-line only when ``#`` is the first nonblank character.
    """

    def __init__(self, text: str, *, knows_bar: bool = False) -> None:
        self.text = text
        self.knows_bar = knows_bar
        self.pos = 0
        self.line = 1
        self.column = 1
        self.line_start = 0
        self._pending_separator = ""

    def tokens(self) -> list[SexpToken]:
        result: list[SexpToken] = []
        token_re = _SEXPR_TOKEN_RE_KNOWS_BAR if self.knows_bar else _SEXPR_TOKEN_RE
        text = self.text
        text_len = len(text)

        while self.pos < text_len:
            if text[self.pos] == "#" and text[self.line_start:self.pos].strip() == "":
                self._skip_comment()
                continue

            match = token_re.match(text, self.pos)
            if match is None:
                raise SexprLexError(
                    "Unexpected token",
                    offset=self.pos,
                    line=self.line,
                    column=self.column,
                )

            kind = match.lastgroup
            token_text = match.group()
            if kind == "space":
                self._pending_separator += self._separator_text(token_text)
                self._advance_position(token_text)
                continue

            if kind == "unterminated_string":
                raise SexprLexError(
                    "Unterminated delimited string",
                    offset=self.pos,
                    line=self.line,
                    column=self.column,
                )

            start_offset = self.pos
            start_line = self.line
            start_column = self.column
            separator = self._pending_separator
            self._pending_separator = ""
            self._advance_position(token_text)

            if kind == "left":
                result.append(
                    SexpToken(
                        TOKEN_LEFT,
                        token_text,
                        token_text,
                        start_offset,
                        start_line,
                        start_column,
                        separator,
                    )
                )
                continue

            if kind == "right":
                result.append(
                    SexpToken(
                        TOKEN_RIGHT,
                        token_text,
                        token_text,
                        start_offset,
                        start_line,
                        start_column,
                        separator,
                    )
                )
                continue

            if kind == "string":
                result.append(
                    SexpToken(
                        TOKEN_STRING,
                        token_text,
                        self._quoted_value(token_text),
                        start_offset,
                        start_line,
                        start_column,
                        separator,
                    )
                )
                continue

            if kind == "number":
                result.append(
                    SexpToken(
                        TOKEN_NUMBER,
                        token_text,
                        self._number_value(token_text),
                        start_offset,
                        start_line,
                        start_column,
                        separator,
                    )
                )
                continue

            if kind == "atom":
                result.append(
                    SexpToken(
                        TOKEN_ATOM,
                        token_text,
                        token_text,
                        start_offset,
                        start_line,
                        start_column,
                        separator,
                    )
                )
                continue

            raise AssertionError(f"unhandled token kind {kind!r}")

        return result

    def _skip_comment(self) -> None:
        newline = _NEWLINE_RE.search(self.text, self.pos)
        if newline is None:
            skipped = len(self.text) - self.pos
            self.pos = len(self.text)
            self.column += skipped
            return
        end = newline.start()
        self.column += end - self.pos
        self.pos = end

    def _advance_position(self, token_text: str) -> None:
        token_len = len(token_text)
        self.pos += token_len
        if "\n" not in token_text and "\r" not in token_text:
            self.column += token_len
            return

        start_offset = self.pos
        last_newline_end = 0
        newline_count = 0
        for newline in _NEWLINE_RE.finditer(token_text):
            newline_count += 1
            last_newline_end = newline.end()

        self.line += newline_count
        self.column = token_len - last_newline_end + 1
        self.line_start = start_offset - token_len + last_newline_end

    @staticmethod
    def _separator_text(token_text: str) -> str:
        if "\r" not in token_text:
            return token_text
        return _NEWLINE_RE.sub("\n", token_text)

    @staticmethod
    def _quoted_value(token_text: str) -> QuotedString:
        body = token_text[1:-1]
        if "\\" not in body:
            if "\r" in body:
                body = _NEWLINE_RE.sub("\n", body)
            return QuotedString(body)
        return QuotedString(_unescape_kicad_string(_NEWLINE_RE.sub("\n", body)))

    @staticmethod
    def _number_value(token_text: str) -> int | float:
        if "." in token_text or "e" in token_text.lower():
            return float(token_text)
        return int(token_text)


_TEARDROP_VALUE_TOKENS = {
    "best_length_ratio",
    "max_length",
    "best_width_ratio",
    "max_width",
    "curve_points",
    "filter_ratio",
}
_TEARDROP_BOOL_TOKENS = {
    "enabled",
    "allow_two_segments",
    "prefer_zone_connections",
    "curved_edges",
}
_TEARDROP_TOKENS = _TEARDROP_VALUE_TOKENS | _TEARDROP_BOOL_TOKENS
_BOOL_VALUE_TOKENS = {"yes", "no", "true", "false"}


class KicadSexprParser:
    """Build the existing list-tree API from KiCad DSN-style tokens."""

    def __init__(self, tokens: list[SexpToken]) -> None:
        self.tokens = tokens
        self.pos = 0
        # Stack of currently-open `(` tokens so an unbalanced opener can be
        # blamed at its actual source position rather than at end-of-input.
        self._open_stack: list[SexpToken] = []

    def parse(self) -> Any:
        if not self.tokens:
            raise SexprTreeError("No or empty expression")

        if self._peek().kind != TOKEN_LEFT:
            token = self._peek()
            raise SexprTreeError(
                "Missing initial opening parenthesis",
                offset=token.offset,
                line=token.line,
                column=token.column,
                token_text=token.text,
            )

        result = self._parse_list()

        if self.pos < len(self.tokens):
            token = self._peek()
            if token.kind == TOKEN_RIGHT:
                raise SexprTreeError(
                    "Unbalanced closing parenthesis",
                    offset=token.offset,
                    line=token.line,
                    column=token.column,
                )
            raise SexprTreeError(
                "Leftover garbage after end of expression",
                offset=token.offset,
                line=token.line,
                column=token.column,
                token_text=token.text,
            )

        return result

    def _peek(self) -> SexpToken:
        return self.tokens[self.pos]

    def _consume(self) -> SexpToken:
        token = self.tokens[self.pos]
        self.pos += 1
        return token

    def _last_token(self) -> SexpToken | None:
        if not self.tokens:
            return None
        return self.tokens[min(self.pos, len(self.tokens) - 1)]

    def _unexpected_eof(self, message: str = "Unexpected end of expression") -> SexprError:
        token = self._last_token()
        if token is None:
            return SexprTreeError(message)
        return SexprTreeError(
            message,
            offset=token.offset,
            line=token.line,
            column=token.column,
        )

    def _unclosed_list_error(self) -> SexprError:
        if self._open_stack:
            opener = self._open_stack[-1]
            return SexprTreeError(
                "Unbalanced opening parenthesis",
                offset=opener.offset,
                line=opener.line,
                column=opener.column,
            )
        return self._unexpected_eof("Unbalanced opening parenthesis")

    def _consume_kind(self, kind: str) -> SexpToken:
        if self.pos >= len(self.tokens):
            raise self._unexpected_eof()
        token = self._consume()
        if token.kind != kind:
            raise SexprTreeError(
                f"Expected {kind}, got {token.text!r}",
                offset=token.offset,
                line=token.line,
                column=token.column,
                token_text=token.text,
            )
        return token

    def _parse_item(self) -> Any:
        if self.pos >= len(self.tokens):
            raise self._unexpected_eof()

        token = self._peek()

        if token.kind == TOKEN_LEFT:
            return self._parse_list()

        if token.kind == TOKEN_RIGHT:
            raise SexprTreeError(
                "Unbalanced closing parenthesis",
                offset=token.offset,
                line=token.line,
                column=token.column,
            )

        return self._consume().value

    def _parse_list(self) -> list[Any]:
        open_token = self._peek() if self.pos < len(self.tokens) else None
        self._consume_kind(TOKEN_LEFT)
        if open_token is not None:
            self._open_stack.append(open_token)
        result: list[Any] = []

        try:
            if self.pos < len(self.tokens) and self._peek().kind == TOKEN_RIGHT:
                self._consume()
                return result

            first = self._parse_item()
            result.append(first)

            if first == "teardrops":
                self._parse_teardrops_body(result)
                return result

            while self.pos < len(self.tokens):
                if self._peek().kind == TOKEN_RIGHT:
                    self._consume()
                    return result
                result.append(self._parse_item())

            raise self._unclosed_list_error()
        finally:
            if open_token is not None and self._open_stack and self._open_stack[-1] is open_token:
                self._open_stack.pop()

    def _parse_teardrops_body(self, result: list[Any]) -> None:
        while self.pos < len(self.tokens):
            token = self._peek()

            if token.kind == TOKEN_RIGHT:
                self._consume()
                return

            if token.kind == TOKEN_LEFT:
                self._consume()
                if self.pos >= len(self.tokens):
                    raise self._unexpected_eof("Unexpected end of teardrops block")
                key_token = self._consume()
                if not self._is_field_key(key_token):
                    result.append(self._parse_list_tail([key_token.value]))
                    continue
                result.append(self._parse_teardrop_field(str(key_token.value), parenthesized=True))
                continue

            if self._is_field_key(token):
                key = str(self._consume().value)
                result.append(self._parse_teardrop_field(key, parenthesized=False))
                continue

            raise SexprTreeError(
                f"Unexpected teardrops token {token.text!r}",
                offset=token.offset,
                line=token.line,
                column=token.column,
                token_text=token.text,
            )

        raise self._unclosed_list_error()

    def _parse_list_tail(self, result: list[Any]) -> list[Any]:
        while self.pos < len(self.tokens):
            if self._peek().kind == TOKEN_RIGHT:
                self._consume()
                return result
            result.append(self._parse_item())
        raise self._unclosed_list_error()

    def _parse_teardrop_field(self, key: str, *, parenthesized: bool) -> list[Any]:
        if key in _TEARDROP_BOOL_TOKENS:
            if parenthesized:
                if self.pos < len(self.tokens) and self._peek().kind == TOKEN_RIGHT:
                    self._consume()
                    return [key]

                value_token = self._consume()
                if not self._is_bool_value(value_token):
                    raise SexprTreeError(
                        f"Expected yes/no for teardrops {key}",
                        offset=value_token.offset,
                        line=value_token.line,
                        column=value_token.column,
                        token_text=value_token.text,
                    )
                self._consume_kind(TOKEN_RIGHT)
                return [key, value_token.value]

            return [key]

        if self.pos >= len(self.tokens):
            raise self._unexpected_eof(f"Missing value for teardrops {key}")

        value = self._parse_item()
        self._consume_kind(TOKEN_RIGHT)
        return [key, value]

    def _is_field_key(self, token: SexpToken) -> bool:
        return token.kind == TOKEN_ATOM and str(token.value) in _TEARDROP_TOKENS

    def _is_bool_value(self, token: SexpToken) -> bool:
        return token.kind == TOKEN_ATOM and str(token.value).lower() in _BOOL_VALUE_TOKENS


def lex_sexp(sexp: str) -> list[SexpToken]:
    """Tokenize a KiCad S-expression string using KiCad-mode DSN rules."""
    return KicadSexprLexer(sexp).tokens()


@dataclass(frozen=True)
class ParserDialectException:
    """One accepted KiCad-dialect deviation from generic S-expression syntax.

    The compatibility lane proves the parser intentionally accepts the form,
    names the production responsible, and points at a source file that
    requires it. Every entry must be exercised by a parser-only test.
    """

    name: str
    description: str
    production: str
    source_file_hint: str
    sample: str
    expected: Any


PARSER_DIALECT_EXCEPTIONS: tuple[ParserDialectException, ...] = (
    ParserDialectException(
        name="teardrops_bare_filter_ratio",
        description=(
            "KiCad 10 PCBs serialize teardrop value fields without a leading "
            "'(' after a parenthesized neighbour, e.g. "
            "'(curved_edges no)filter_ratio 0.9)'. KiCad's PCB parser accepts "
            "this; our parser normalizes it into a sibling list."
        ),
        production="KicadSexprParser._parse_teardrops_body",
        source_file_hint=(
            "projects/royalblue54L_feather/input/RoyalBlue54L-Feather.kicad_pcb"
        ),
        sample=(
            "(teardrops (curved_edges no)filter_ratio 0.9)(enabled yes))"
        ),
        expected=[
            "teardrops",
            ["curved_edges", "no"],
            ["filter_ratio", 0.9],
            ["enabled", "yes"],
        ],
    ),
)


@dataclass(frozen=True)
class SexpRoundtripResult:
    """Result of a parser-only round-trip.

    ``phase`` is the earliest stage that failed, or ``"ok"`` on success.
    Stages, in order: ``"lex"``, ``"tree"``, ``"build"``, ``"reparse"``,
    ``"compare"``, ``"ok"``. No typed KiCad OOP parse runs in the
    critical path; this contract is intentionally limited to
    ``parse_sexp`` / ``build_sexp`` / ``format_sexp`` so downstream
    SVG/IR failures cannot hide low-level parse defects.
    """

    phase: str
    error: str | None = None
    parsed: Any = None
    rebuilt: str | None = None


_PARSER_ROUNDTRIP_PHASES: tuple[str, ...] = (
    "lex",
    "tree",
    "build",
    "reparse",
    "compare",
    "ok",
)


def roundtrip_sexp_text(
    text: str, *, source_path: Any = None
) -> SexpRoundtripResult:
    """Exercise the parser-only round-trip and tag the earliest failure.

    The full sequence is:

        text -> lex_sexp -> KicadSexprParser.parse -> build_sexp
             -> parse_sexp(rebuilt) -> compare list trees

    No typed OOP parse is involved. The compare stage requires the
    re-parsed list tree to be structurally equal to the first parse, so
    the parser, writer, and re-lexer all share one regression surface.

    ``source_path`` is decorated onto any ``SexprError`` raised by the
    lex / tree / reparse phases so corpus-scale failure listings can
    name the offending file inline.
    """

    def _decorate(exc: SexprError) -> str:
        return str(exc.with_source_path(source_path))

    try:
        tokens = lex_sexp(text)
    except SexprError as exc:
        return SexpRoundtripResult(phase="lex", error=_decorate(exc))
    except Exception as exc:  # pragma: no cover - defensive
        return SexpRoundtripResult(phase="lex", error=f"{type(exc).__name__}: {exc}")

    try:
        parsed = KicadSexprParser(tokens).parse()
    except SexprError as exc:
        return SexpRoundtripResult(phase="tree", error=_decorate(exc))
    except Exception as exc:  # pragma: no cover - defensive
        return SexpRoundtripResult(phase="tree", error=f"{type(exc).__name__}: {exc}")

    try:
        rebuilt = build_sexp(parsed)
    except Exception as exc:
        return SexpRoundtripResult(
            phase="build",
            error=f"{type(exc).__name__}: {exc}",
            parsed=parsed,
        )

    try:
        reparsed = parse_sexp(rebuilt)
    except SexprError as exc:
        return SexpRoundtripResult(
            phase="reparse",
            error=_decorate(exc),
            parsed=parsed,
            rebuilt=rebuilt,
        )

    if reparsed != parsed:
        return SexpRoundtripResult(
            phase="compare",
            error="Re-parsed tree differs from first parse",
            parsed=parsed,
            rebuilt=rebuilt,
        )

    return SexpRoundtripResult(phase="ok", parsed=parsed, rebuilt=rebuilt)


def string_to_sexp(value: Any) -> str:
    """
    Convert a string value to its S-expression representation.
    """
    if isinstance(value, QuotedString):
        return value.get_as_sexp()
    elif isinstance(value, str):
        # For bare strings, validate they don't need quoting
        validate_bare_string(value)
        return value
    else:
        raise SexprError(f"Expected string type, got {type(value)}")

def parse_sexp(sexp: str, *, source_path: Any = None) -> Any:
    """Parse one KiCad S-expression into the existing list-tree representation.

    ``source_path`` is optional; when set, any ``SexprError`` raised
    out of this call carries the path so callers can surface
    ``file:line:col`` diagnostics without wrapping the call themselves.
    """
    try:
        return KicadSexprParser(lex_sexp(sexp)).parse()
    except SexprError as exc:
        if source_path is None or exc.source_path is not None:
            raise
        raise exc.with_source_path(source_path) from exc


def iter_sexp_form_spans(
    sexp: str,
    selector: SexpSelector | None = None,
    *,
    source_path: Any = None,
) -> Iterator[SexpFormSpan]:
    """Yield selected S-expression form spans without building a full tree."""
    active_selector = selector if selector is not None else SexpSelector()
    yield from _SexpProjectionScanner(
        sexp,
        active_selector,
        source_path=source_path,
    ).scan()


def iter_sexp_file_form_spans(
    path: Path | str,
    selector: SexpSelector | None = None,
) -> Iterator[SexpFormSpan]:
    """Yield selected form spans from an S-expression file."""
    source_path = Path(path)
    try:
        text = source_path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        text = source_path.read_text(encoding="utf-8-sig")
    yield from iter_sexp_form_spans(text, selector, source_path=source_path)


def parse_sexp_span(span: SexpFormSpan) -> Any:
    """Parse a selected form span into the normal nested-list representation."""
    return span.parse()


@dataclass(frozen=True)
class SexpSpan:
    """Source range for a parsed S-expression list node.

    ``offset`` / ``line`` / ``column`` point at the opening ``(`` of the
    list. ``end_offset`` / ``end_line`` / ``end_column`` point at the
    matching closing ``)``. The end column is one past the closing
    paren so consumers can slice ``text[span.offset:span.end_offset]``
    to recover the original substring.
    """

    offset: int
    line: int
    column: int
    end_offset: int
    end_line: int
    end_column: int


def parse_sexp_with_spans(
    sexp: str, *, source_path: Any = None
) -> tuple[Any, dict[int, SexpSpan]]:
    """Parse and also return per-list-node source spans.

    Returns ``(tree, spans)`` where ``spans`` is keyed by ``id(list)``
    so callers can look up a parsed list's source position without
    changing the list-tree data type. The spans dict is only valid for
    as long as the parsed lists are alive — ``id()`` may be reused
    once a list is garbage-collected.

    This is opt-in because the hot parse path does not need spans;
    use it for source-model audit, round-trip diagnostics, and tooling
    that wants to highlight or jump to a node's source range.
    """
    tokens = lex_sexp(sexp)
    parser = _SpanCapturingParser(tokens)
    try:
        tree = parser.parse()
    except SexprError as exc:
        if source_path is None or exc.source_path is not None:
            raise
        raise exc.with_source_path(source_path) from exc
    return tree, parser.spans


class _SpanCapturingParser(KicadSexprParser):
    """KicadSexprParser variant that records the source range of every list."""

    def __init__(self, tokens: list[SexpToken]) -> None:
        super().__init__(tokens)
        self.spans: dict[int, SexpSpan] = {}

    def _parse_list(self) -> list[Any]:
        open_token = self._peek() if self.pos < len(self.tokens) else None
        before = self.pos
        result = super()._parse_list()
        if open_token is None:
            return result
        # Position now points just past the closing `)`; that close token
        # is at self.pos - 1.
        close_token = self.tokens[self.pos - 1] if self.pos > before else open_token
        self.spans[id(result)] = SexpSpan(
            offset=open_token.offset,
            line=open_token.line,
            column=open_token.column,
            end_offset=close_token.offset + len(close_token.text),
            end_line=close_token.line,
            end_column=close_token.column + len(close_token.text),
        )
        return result


def debug_dump_tokens(
    sexp: str, *, limit: int | None = None, source_path: Any = None
) -> str:
    """Return a human-readable per-token diagnostic dump.

    Useful for triaging parse failures, comparing lexer output against
    a known-good KiCad file, and as a parser debugging API that does
    not couple callers to the typed OOP layer. Set ``limit`` to cap the
    number of tokens printed.
    """
    tokens = lex_sexp(sexp)
    header = f"# {len(tokens)} tokens"
    if source_path is not None:
        header = f"{header} from {source_path}"
    lines = [header]
    shown = tokens if limit is None else tokens[:limit]
    for idx, token in enumerate(shown):
        text = token.text if len(token.text) <= 60 else token.text[:57] + "..."
        lines.append(
            f"  [{idx:5d}] line={token.line:<5d} col={token.column:<4d}"
            f" off={token.offset:<8d} kind={token.kind:<6s} text={text!r}"
        )
    if limit is not None and len(tokens) > limit:
        lines.append(f"  ... ({len(tokens) - limit} more tokens)")
    return "\n".join(lines)


def SexprItem(val: Any, key: str | None = None) -> str:
    """
    Convert a value to S-expression format, handling QuotedString properly.
    """
    if key:
        fmt = "(" + key + " {val})"
    else:
        fmt = "{val}"

    if val is None:
        val = QuotedString("").get_as_sexp()
    elif isinstance(val, QuotedString):
        val = val.get_as_sexp()
    elif isinstance(val, str):
        # For bare strings, validate and use as-is
        try:
            validate_bare_string(val)
            # val remains as-is for bare strings
        except SexprError:
            # If validation fails, suggest using QuotedString
            raise SexprError(
                f"String '{val}' requires quoting. Use QuotedString('{val}') instead."
            )
    elif isinstance(val, (list, tuple)):
        val = " ".join([SexprItem(v) for v in val])
    elif isinstance(val, dict):
        values = []
        for key in val.keys():
            values.append(SexprItem(val[key], key))
        val = " ".join(values)
    elif isinstance(val, float):
        val = str(round(val, 10)).rstrip("0").rstrip(".")
    elif isinstance(val, int):
        val = str(val)

    return fmt.format(val=val)


class SexprBuilder:
    def __init__(self, key: Any | None) -> None:
        self.indent: int = 0
        self.output: str = ""
        self.items = []
        if key is not None:
            self.startGroup(key, newline=False)

    def _indent(self) -> None:
        self.output += " " * 2 * self.indent

    def _newline(self) -> None:
        self.output += "\n"

    def _addItems(self) -> None:
        self.output += " ".join(str(i) for i in self.items)
        self.items = []

    def startGroup(
        self, key: Any | None = None, newline: bool = True, indent: bool = False
    ) -> None:
        self._addItems()
        if newline and indent:
            self.indent += 1
        if newline:
            self._newline()
            self._indent()
        self.output += "("
        if key:
            self.output += str(key) + " "

    def endGroup(self, newline: bool = True) -> None:
        self._addItems()
        if newline:
            self._newline()
            if self.indent > 0:
                self.indent -= 1
            self._indent()
        self.output += ")"

    def addOptItem(self, key: Any, item: Any, newline: bool = True, indent: bool = False) -> None:
        if item in [None, 0, False]:
            return

        self.addItems({key: item}, newline=newline, indent=indent)

    def addItem(self, item: Any, newline: bool = True, indent: bool = False) -> None:
        self._addItems()
        if newline and indent:
            self.indent += 1
        if newline:
            self.newLine()
        self.items.append(SexprItem(item))

    # Add a (preformatted) item
    def addItems(self, items: Any, newline: bool = True, indent: bool = False) -> None:
        self._addItems()
        if indent:
            self.indent += 1
        if newline:
            self.newLine()
        if isinstance(items, (list, tuple)):
            for item in items:
                self.items.append(SexprItem(item))
        else:
            self.items.append(SexprItem(items))

    def newLine(self, indent: bool = False) -> None:
        self._addItems()
        self._newline()
        if indent:
            self.indent += 1
        self._indent()

    def unIndent(self) -> None:
        if self.indent > 0:
            self.indent -= 1


def build_sexp(exp: Any, indent: str = "") -> str:
    """Build s-expression string from parsed data. Uses list-based building for O(n) performance."""
    parts = []
    _build_sexp_recursive(exp, indent, parts)
    return ''.join(parts)


def _build_sexp_recursive(exp: Any, indent: str, parts: list[str]) -> None:
    """Internal recursive helper that appends to parts list for efficiency."""
    if isinstance(exp, list):
        parts.append("(")
        last_was_list = False
        for i, elem in enumerate(exp):
            if i > 0:
                if isinstance(elem, list):
                    parts.append("\n\t")
                    parts.append(indent)
                else:
                    parts.append(" ")
            _build_sexp_recursive(elem, indent + "\t", parts)
            last_was_list = isinstance(elem, list)
        if last_was_list:
            parts.append("\n")
            parts.append(indent)
        parts.append(")")
        return

    if isinstance(exp, QuotedString):
        parts.append(exp.get_as_sexp())
    elif isinstance(exp, str) and len(exp) == 0:
        parts.append('""')
    elif isinstance(exp, str) and re.search(r"[\s\(\)]", exp):
        if exp.startswith('"') and exp.endswith('"'):
            parts.append(exp)
        else:
            parts.append('"%s"' % _escape_kicad_string(exp))
    elif isinstance(exp, float):
        if exp.is_integer():
            parts.append(str(int(exp)))
        else:
            parts.append(str(exp))
    elif isinstance(exp, int):
        parts.append(str(exp))
    elif isinstance(exp, str):
        parts.append(exp)
    elif exp is None:
        parts.append('""')
    else:
        parts.append(str(exp))


def format_sexp(sexp: str, indentation_size: int = 2, max_nesting: int = 2) -> str:
    """Format an S-expression string using the KiCad lexer for tokenization."""
    parts = []
    n = 0
    last_char = ""

    for token in lex_sexp(sexp):
        indentation = "" if last_char != ")" else " "

        if token.kind == TOKEN_LEFT:
            if parts:
                if n <= max_nesting:
                    if last_char == " ":
                        parts.pop()  # Remove trailing space
                        last_char = parts[-1][-1] if parts and parts[-1] else ""
                    indentation = "\n" + (" " * indentation_size * n)
                # else: use the default indentation (a single space when the
                # previous char was ')'). Earlier this branch appended an
                # extra " " on top of the default, producing ')<sp><sp>(' for
                # adjacent inline siblings.
            n += 1
            parts.append(indentation)
            parts.append("(")
            last_char = "("
        elif token.kind == TOKEN_RIGHT:
            if parts and last_char == " ":
                parts.pop()  # Remove trailing space
                last_char = parts[-1][-1] if parts and parts[-1] else ""
            n -= 1
            if n < max_nesting:
                indentation = "\n" + (" " * indentation_size * n)
            parts.append(indentation)
            parts.append(")")
            last_char = ")"
        else:
            parts.append(indentation)
            parts.append(token.text)
            parts.append(" ")
            last_char = " "

    parts.append("\n")
    return ''.join(parts)


# =============================================================================
# S-Expression Formatting Constants (from KiCad source kicad_io_utils.cpp)
# =============================================================================

INDENT_CHAR = '\t'
INDENT_SIZE = 1
XY_COLUMN_LIMIT = 99  # Compact (xy ...) lists until this column
TOKEN_WRAP_THRESHOLD = 72  # Wrap long token sequences
MIME_BASE64_LENGTH = 76  # Base64 line length


# =============================================================================
# String Formatting Functions
# =============================================================================

def format_float(value: float, precision: int = 6) -> str:
    """Format a float with proper precision, removing trailing zeros.

    Also normalizes very small values to zero to avoid floating-point
    artifacts like -4.1e-9 becoming "-0" in output. Since we format with
    6 decimal places by default, values smaller than 5e-7 will round to 0,
    so we use 1e-6 as the threshold to be safe.
    """
    # Normalize very small values to zero (floating-point artifacts)
    # Threshold: values that would round to 0 at the given precision
    threshold = 0.5 * (10 ** -precision)  # e.g., 5e-7 for precision=6
    if abs(value) < threshold:
        value = 0.0
    if value == int(value):
        return str(int(value))
    formatted = f"{value:.{precision}f}".rstrip('0').rstrip('.')
    return formatted


def quote_string(s: str) -> str:
    """Quote a string if necessary for s-expression output."""
    if not s:
        return '""'
    # Check if quoting is needed
    if re.search(r'[\s()\"]', s) or s.startswith('"'):
        return f'"{_escape_kicad_string(s)}"'
    return s


# =============================================================================
# SexpWriter - KiCad-compatible S-Expression Writer
# =============================================================================

class SexpWriter:
    """
    Writes s-expressions with KiCad-compatible formatting.

    Formatting rules (from kicad_io_utils.cpp):
    - Indentation is one tab per level
    - New lists start on a new line (except for short forms)
    - Lists without sublists stay on one line
    - (xy ...) lists are compacted until column 99
    - Short forms (font, stroke, fill, etc.) stay on one line
    """

    SHORT_FORM_TOKENS = {'font', 'stroke', 'fill', 'teardrop', 'offset', 'rotate', 'scale'}

    def __init__(self) -> None:
        self.parts: list[str] = []
        self.depth: int = 0
        self.column: int = 0
        self.in_short_form: bool = False
        self.short_form_depth: int = 0
        self.in_xy_list: bool = False
        self.last_char: str = ''

    def _indent(self) -> str:
        return INDENT_CHAR * (self.depth * INDENT_SIZE)

    def _newline_indent(self) -> None:
        self.parts.append('\n')
        self.parts.append(self._indent())
        self.column = self.depth * INDENT_SIZE

    def _is_xy(self, elem: list) -> bool:
        """Check if element is an (xy ...) element."""
        return isinstance(elem, list) and len(elem) > 0 and elem[0] == 'xy'

    def _is_pts(self, elem: list) -> bool:
        """Check if element is a (pts ...) element."""
        return isinstance(elem, list) and len(elem) > 0 and elem[0] == 'pts'

    def _is_short_form(self, elem: list) -> bool:
        """Check if element should use short (compact) form."""
        return isinstance(elem, list) and len(elem) > 0 and elem[0] in self.SHORT_FORM_TOKENS

    def _has_sublists(self, elem: list) -> bool:
        """Check if a list contains sublists."""
        return any(isinstance(e, list) for e in elem[1:]) if isinstance(elem, list) else False

    def _has_multiline_content(self, elem: list) -> bool:
        """Check if element has content that spans multiple lines (sublists or FormattedDataBlock with newlines)."""
        if not isinstance(elem, list):
            return False
        for item in elem[1:]:
            if isinstance(item, list):
                return True
            if isinstance(item, FormattedDataBlock):
                payload = self._data_block_payload(item)
                if '\n' in str(item) or len(payload) > MIME_BASE64_LENGTH:
                    return True
        return False

    @staticmethod
    def _data_block_payload(value: FormattedDataBlock) -> str:
        return ''.join(
            ch for ch in str(value)
            if ch != '|' and not ch.isspace()
        )

    def _format_value(self, value: Any, add_indent_after_newlines: bool = False) -> str:
        """Format a single value for output.

        Args:
            value: The value to format
            add_indent_after_newlines: If True and value is FormattedDataBlock,
                add current indentation after each newline in the output
        """
        if isinstance(value, FormattedDataBlock):
            payload = self._data_block_payload(value)
            lines = [
                payload[i:i + MIME_BASE64_LENGTH]
                for i in range(0, len(payload), MIME_BASE64_LENGTH)
            ] or [""]
            if len(lines) == 1:
                return f"|{lines[0]}|"
            indent = self._indent() if add_indent_after_newlines else ""
            return ''.join(
                ("|" if index == 0 else "\n" + indent)
                + line
                + ("|" if index == len(lines) - 1 else "")
                for index, line in enumerate(lines)
            )
        elif isinstance(value, QuotedString):
            return f'"{_escape_kicad_string(str(value))}"'
        elif isinstance(value, str):
            return quote_string(value)
        elif isinstance(value, float):
            return format_float(value)
        elif isinstance(value, int):
            return str(value)
        elif isinstance(value, bool):
            return 'yes' if value else 'no'
        elif value is None:
            return '""'
        else:
            return str(value)

    def _write_simple_list(self, elem: list) -> None:
        """Write a simple list (no sublists) on one line."""
        self.parts.append('(')
        for i, item in enumerate(elem):
            if i > 0:
                self.parts.append(' ')
            if isinstance(item, list):
                self._write_simple_list(item)
            else:
                self.parts.append(self._format_value(item))
        self.parts.append(')')

    def _write_pts(self, elem: list) -> None:
        """Write a (pts ...) element with compact (xy ...) formatting."""
        self.parts.append('(pts')
        self.column = len('(pts')

        for i, item in enumerate(elem[1:]):
            if self._is_xy(item):
                if self.column + 20 > XY_COLUMN_LIMIT:  # Approximate xy length
                    self._newline_indent()
                    self.parts.append(INDENT_CHAR)  # Extra indent for pts content
                    self.column += 1
                else:
                    self.parts.append(' ')
                    self.column += 1

                self._write_simple_list(item)
                self.column += len(str(item))
            else:
                self.parts.append(' ')
                self.parts.append(self._format_value(item))

        self.parts.append(')')

    def _write_element(self, elem: list, is_root: bool = False) -> None:
        """Write an s-expression element with proper formatting."""
        if not isinstance(elem, list) or len(elem) == 0:
            return

        tag = elem[0]
        is_short = self._is_short_form(elem)
        is_pts = self._is_pts(elem)
        has_subs = self._has_sublists(elem)
        has_multiline = self._has_multiline_content(elem)

        # Handle entering short form
        was_in_short_form = self.in_short_form
        if is_short and not self.in_short_form:
            self.in_short_form = True
            self.short_form_depth = self.depth

        # Write opening
        if is_root:
            self.parts.append('(')
        elif self.in_short_form or is_short:
            self.parts.append(' (')
        else:
            self._newline_indent()
            self.parts.append('(')

        self.depth += 1

        # Write tag
        self.parts.append(str(tag))

        # Handle pts specially
        if is_pts:
            # Write all xy elements compactly
            for item in elem[1:]:
                if self._is_xy(item):
                    self.parts.append(' ')
                    self._write_simple_list(item)
                else:
                    self.parts.append(' ')
                    self.parts.append(self._format_value(item))
        else:
            # Write remaining elements
            for item in elem[1:]:
                if isinstance(item, list):
                    if self._is_pts(item):
                        self._newline_indent()
                        self._write_pts(item)
                    elif not has_subs or self.in_short_form:
                        self.parts.append(' ')
                        self._write_simple_list(item)
                    else:
                        self._write_element(item)
                elif isinstance(item, FormattedDataBlock):
                    # FormattedDataBlock needs indentation after its internal newlines
                    self.parts.append(' ')
                    self.parts.append(self._format_value(item, add_indent_after_newlines=True))
                else:
                    self.parts.append(' ')
                    self.parts.append(self._format_value(item))

        # Write closing
        self.depth -= 1

        if has_multiline and not self.in_short_form and not is_pts:
            self._newline_indent()

        self.parts.append(')')

        # Restore short form state
        if is_short and not was_in_short_form:
            self.in_short_form = False

    def write(self, sexp: list) -> str:
        """Write a complete s-expression to string."""
        self.parts = []
        self.depth = 0
        self._write_element(sexp, is_root=True)
        self.parts.append('\n')  # POSIX newline at end
        return ''.join(self.parts)

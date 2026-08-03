"""
Subtest: KiCad DSN-Style S-Expression Parser
Stratum: L0_foundation
Purpose: Pin lexer/tree-builder behavior that must stay portable to C++/WASM.
"""

import pytest

import kicad_monkey.kicad_sexpr as sexpr_mod
from kicad_monkey.kicad_sexpr import (
    KicadSexprLexer,
    PARSER_DIALECT_EXCEPTIONS,
    QuotedString,
    SexpSpan,
    SexprDialectError,
    SexprError,
    SexprLexError,
    SexprTreeError,
    TOKEN_ATOM,
    TOKEN_LEFT,
    TOKEN_NUMBER,
    TOKEN_STRING,
    build_sexp,
    debug_dump_tokens,
    format_sexp,
    lex_sexp,
    parse_sexp,
    parse_sexp_with_spans,
    roundtrip_sexp_text,
)


def test_lexer_preserves_simple_portable_token_model() -> None:
    tokens = lex_sexp('(root (child .5 -2. 1e-3 "txt"))')

    assert [token.kind for token in tokens[:4]] == [
        TOKEN_LEFT,
        TOKEN_ATOM,
        TOKEN_LEFT,
        TOKEN_ATOM,
    ]
    assert [token.value for token in tokens if token.kind == TOKEN_NUMBER] == [0.5, -2.0, 0.001]
    assert tokens[0].line == 1
    assert tokens[0].column == 1


def test_parse_supports_adjacent_lists_without_whitespace() -> None:
    assert parse_sexp("(root(child 1)(next 2))") == [
        "root",
        ["child", 1],
        ["next", 2],
    ]


def test_whole_line_comments_are_skipped_but_trailing_hash_is_data() -> None:
    assert parse_sexp(
        """
        # KiCad whole-line comment
        (root #not-a-comment)
        """
    ) == ["root", "#not-a-comment"]


def test_cr_only_newline_whole_line_comments_are_skipped() -> None:
    """CR-only line endings must still recognize whole-line ``#`` comments."""
    assert parse_sexp("# comment after CR\r(root 1)") == ["root", 1]
    assert parse_sexp("(root 1)\r# trailing comment") == ["root", 1]

    tokens = lex_sexp("(root 1)\r# comment after CR\r(child 2)")
    assert [t.value for t in tokens if t.kind == TOKEN_ATOM] == ["root", "child"]
    assert all(
        not (isinstance(t.value, str) and t.value.startswith("#"))
        for t in tokens
    )


def test_sexp_token_keeps_frozen_constructor_and_separator() -> None:
    from dataclasses import FrozenInstanceError

    from kicad_monkey.kicad_sexpr import SexpToken

    token = SexpToken(
        TOKEN_ATOM,
        "net",
        "net",
        4,
        line=2,
        column=3,
        separator=" ",
    )
    assert token.line == 2
    assert token.column == 3
    assert token.separator == " "
    assert hash(token) == hash(
        SexpToken(TOKEN_ATOM, "net", "net", 4, separator=" ")
    )
    with pytest.raises(FrozenInstanceError):
        token.kind = "string"  # type: ignore[misc]


def test_regex_lexer_preserves_number_and_atom_boundaries() -> None:
    tokens = lex_sexp("(root 123 -2. +0.5 .25 1e-3 1e 1abc +. -)")

    payload = [(token.kind, token.value) for token in tokens[2:-1]]

    assert payload == [
        (TOKEN_NUMBER, 123),
        (TOKEN_NUMBER, -2.0),
        (TOKEN_NUMBER, 0.5),
        (TOKEN_NUMBER, 0.25),
        (TOKEN_NUMBER, 0.001),
        (TOKEN_ATOM, "1e"),
        (TOKEN_ATOM, "1abc"),
        (TOKEN_ATOM, "+."),
        (TOKEN_ATOM, "-"),
    ]


def test_regex_lexer_unescapes_quoted_strings_lazily(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[str] = []

    def fake_unescape(value: str) -> str:
        calls.append(value)
        return f"decoded:{value}"

    monkeypatch.setattr(sexpr_mod, "_unescape_kicad_string", fake_unescape)

    tokens = lex_sexp(r'(root "plain text" "escaped\n")')
    strings = [token.value for token in tokens if token.kind == TOKEN_STRING]

    assert strings == [QuotedString("plain text"), QuotedString(r"decoded:escaped\n")]
    assert calls == [r"escaped\n"]


def test_regex_lexer_tracks_crlf_locations_inside_strings() -> None:
    tokens = lex_sexp('(root\r\n  "a\r\nb" 2)')
    strings = [token for token in tokens if token.kind == TOKEN_STRING]
    numbers = [token for token in tokens if token.kind == TOKEN_NUMBER]

    assert strings[0].line == 2
    assert strings[0].column == 3
    assert strings[0].value == QuotedString("a\nb")
    assert numbers[0].line == 3
    assert numbers[0].column == 4


def test_regex_lexer_keeps_whole_line_hash_comment_rule() -> None:
    tokens = lex_sexp("  # comment\r\n(root #not-a-comment)")

    assert [token.value for token in tokens] == ["(", "root", "#not-a-comment", ")"]
    assert tokens[0].line == 2
    assert tokens[0].column == 1


def test_regex_lexer_respects_knows_bar_separator() -> None:
    tokens = KicadSexprLexer("(data |)", knows_bar=False).tokens()
    assert [token.value for token in tokens] == ["(", "data", "|", ")"]

    with pytest.raises(SexprLexError, match="Unexpected token"):
        KicadSexprLexer("(data |)", knows_bar=True).tokens()


def test_kicad_string_escapes_match_dsnlexer() -> None:
    parsed = parse_sexp(r'(root "\101\x42\n\t\"\\")')

    assert parsed == ["root", QuotedString('AB\n\t"\\')]


def test_literal_newlines_inside_quoted_strings_remain_compatible() -> None:
    parsed = parse_sexp('(root "Line 1\nLine 2")')

    assert parsed == ["root", QuotedString("Line 1\nLine 2")]


def test_unterminated_string_raises_clear_error() -> None:
    with pytest.raises(SexprError, match="Unterminated delimited string"):
        parse_sexp('(root "unterminated)')


def test_royalblue_style_bare_filter_ratio_is_normalized_inside_teardrops() -> None:
    parsed = parse_sexp(
        """
        (teardrops
            (best_length_ratio 0.5)
            (max_length 2)
            (best_width_ratio 1)
            (max_width 2)
            (curved_edges no)filter_ratio 0.9)
            (enabled yes)
            (allow_two_segments yes)
            (prefer_zone_connections no)
        )
        """
    )

    assert parsed == [
        "teardrops",
        ["best_length_ratio", 0.5],
        ["max_length", 2],
        ["best_width_ratio", 1],
        ["max_width", 2],
        ["curved_edges", "no"],
        ["filter_ratio", 0.9],
        ["enabled", "yes"],
        ["allow_two_segments", "yes"],
        ["prefer_zone_connections", "no"],
    ]


def test_parser_handles_many_adjacent_sibling_lists_stress() -> None:
    siblings = "".join(f'(item_{idx} {idx} "value {idx}")' for idx in range(500))
    parsed = parse_sexp(f"(root{siblings})")

    assert len(parsed) == 501
    assert parsed[1] == ["item_0", 0, QuotedString("value 0")]
    assert parsed[-1] == ["item_499", 499, QuotedString("value 499")]


def test_parser_handles_many_kicad_teardrop_dialect_blocks_stress() -> None:
    block = "(teardrops (curved_edges no)filter_ratio 0.9) (enabled yes))"
    parsed = parse_sexp("(root " + " ".join(block for _ in range(100)) + ")")

    assert len(parsed) == 101
    assert all(
        item[1:] == [["curved_edges", "no"], ["filter_ratio", 0.9], ["enabled", "yes"]]
        for item in parsed[1:]
    )


# -----------------------------------------------------------------------------
# Phase 2: parser-only round-trip and stress fixture generator coverage.
# -----------------------------------------------------------------------------


def _make_deep_nest(depth: int) -> str:
    """Build a single deeply nested list ``(root (l1 (l2 ... (lN value))))``."""
    inner = "(deepest value)"
    for level in range(depth, 0, -1):
        inner = f"(level_{level} {inner})"
    return f"(root {inner})"


def test_roundtrip_succeeds_on_canonical_kicad_fragments() -> None:
    text = """
    (root
        (child 1.5 -2.25 1e-3)
        (text "Line 1\\nLine 2\\tcol2")
        (nested
            (deep
                (deeper "value"))))
    """

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert result.error is None
    assert result.parsed[0] == "root"
    # Rebuilt text round-trips back to the same list tree.
    assert parse_sexp(result.rebuilt) == result.parsed


def test_roundtrip_normalizes_teardrops_dialect_into_canonical_lists() -> None:
    text = "(teardrops (curved_edges no)filter_ratio 0.9)(enabled yes))"

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert result.parsed == [
        "teardrops",
        ["curved_edges", "no"],
        ["filter_ratio", 0.9],
        ["enabled", "yes"],
    ]
    # The rebuilt form is canonical S-expression; re-parsing yields the same tree.
    assert "(filter_ratio 0.9)" in result.rebuilt
    assert parse_sexp(result.rebuilt) == result.parsed


def test_roundtrip_phase_lex_for_unterminated_string() -> None:
    result = roundtrip_sexp_text('(root "unterminated)')

    assert result.phase == "lex"
    assert "Unterminated delimited string" in (result.error or "")


def test_roundtrip_phase_tree_for_missing_open_paren() -> None:
    result = roundtrip_sexp_text("root child 1)")

    assert result.phase == "tree"
    assert "Missing initial opening parenthesis" in (result.error or "")


def test_roundtrip_phase_tree_for_unbalanced_close() -> None:
    result = roundtrip_sexp_text("(root))")

    assert result.phase == "tree"
    assert (
        "Unbalanced closing parenthesis" in (result.error or "")
        or "Leftover garbage" in (result.error or "")
    )


def test_roundtrip_phase_tree_for_leftover_after_root() -> None:
    result = roundtrip_sexp_text("(root) extra")

    assert result.phase == "tree"
    assert "Leftover garbage" in (result.error or "")


def test_roundtrip_handles_deep_nested_list_stress() -> None:
    text = _make_deep_nest(200)

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert result.parsed[0] == "root"
    # Verify depth: drill down counting list children.
    node = result.parsed[1]
    depth = 0
    while isinstance(node, list) and isinstance(node[1], list):
        depth += 1
        node = node[1]
    assert depth == 200


def test_roundtrip_handles_500_sibling_lists_stress() -> None:
    siblings = "".join(f"(item_{i} {i} \"v{i}\")" for i in range(500))
    text = f"(root{siblings})"

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert len(result.parsed) == 501


def test_roundtrip_handles_exponent_and_signed_numbers() -> None:
    text = '(root 1e10 -2.5e-3 +0.5 .25 -1. 0)'

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert result.parsed == ["root", 1e10, -2.5e-3, 0.5, 0.25, -1.0, 0]


def test_roundtrip_handles_kicad_escape_sequences() -> None:
    # Octal, hex, simple escapes, escaped quote and backslash.
    text = r'(root "\101\x42\n\t\"\\")'

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert result.parsed == ["root", QuotedString("AB\n\t\"\\")]


def test_roundtrip_handles_whole_line_comments() -> None:
    text = """
    # leading comment
    (root
    # inner comment
        (child 1)
    )
    # trailing comment
    """

    result = roundtrip_sexp_text(text)

    assert result.phase == "ok"
    assert result.parsed == ["root", ["child", 1]]


def test_roundtrip_handles_inline_hash_in_atom_position() -> None:
    # A '#' that is not the first non-blank character on its line is a normal
    # atom character — KiCad's DSN lexer only treats whole-line '#' as comment.
    result = roundtrip_sexp_text("(root #notacomment)")

    assert result.phase == "ok"
    assert result.parsed == ["root", "#notacomment"]


def test_format_sexp_does_not_change_parsed_tree() -> None:
    text = '(root (child 1) (other "string") (xy 1.5 2.5))'

    formatted = format_sexp(text)

    assert parse_sexp(formatted) == parse_sexp(text)


def test_build_sexp_output_is_lex_clean_for_canonical_trees() -> None:
    tree = ["root", ["child", 1], ["text", QuotedString('value with "quotes"')]]
    rebuilt = build_sexp(tree)

    # Build output must be parseable and stable under repeat round-trips.
    again = parse_sexp(rebuilt)
    assert again == tree
    assert build_sexp(again) == rebuilt


@pytest.mark.parametrize(
    "exception",
    PARSER_DIALECT_EXCEPTIONS,
    ids=[exc.name for exc in PARSER_DIALECT_EXCEPTIONS],
)
def test_registered_parser_dialect_exception_round_trips(exception) -> None:
    """Every registered dialect exception must round-trip through parse + rebuild."""
    parsed = parse_sexp(exception.sample)
    assert parsed == exception.expected, (
        f"Dialect '{exception.name}' parse drifted from registered expected tree"
    )

    result = roundtrip_sexp_text(exception.sample)
    assert result.phase == "ok", (
        f"Dialect '{exception.name}' failed parser round-trip at phase "
        f"{result.phase!r}: {result.error}"
    )
    assert result.parsed == exception.expected


# -----------------------------------------------------------------------------
# Phase 3: structured diagnostics — source_path / line+column / token context,
# token spans on parsed lists, and debug APIs that bypass the typed OOP layer.
# -----------------------------------------------------------------------------


def test_sexpr_error_carries_structured_location_fields() -> None:
    """Lexer errors expose offset/line/column so tooling can jump to source."""
    with pytest.raises(SexprError) as exc_info:
        parse_sexp('(root "unterminated)')

    err = exc_info.value
    assert err.message == "Unterminated delimited string"
    assert err.line == 1
    assert err.column == 7
    assert err.offset == 6
    # str() exposes the message prefix so legacy substring matches still work.
    assert "Unterminated delimited string" in str(err)
    assert "line 1, column 7" in str(err)


def test_parser_error_carries_line_column_and_token_text() -> None:
    """Parser errors are upgraded from offset-only to full location + token."""
    with pytest.raises(SexprError) as exc_info:
        parse_sexp("root child 1")

    err = exc_info.value
    assert err.message == "Missing initial opening parenthesis"
    assert err.line == 1
    assert err.column == 1
    assert err.token_text == "root"
    assert "near 'root'" in str(err)


def test_unclosed_inner_list_error_blames_inner_opener() -> None:
    """Unbalanced opens report the position of the unclosed `(`, not EOF."""
    with pytest.raises(SexprError) as exc_info:
        parse_sexp("(root\n  (inner_unclosed 1 2 3")

    err = exc_info.value
    assert err.message == "Unbalanced opening parenthesis"
    assert err.line == 2
    assert err.column == 3


def test_leftover_garbage_error_includes_offending_token_text() -> None:
    with pytest.raises(SexprError) as exc_info:
        parse_sexp("(root) extra")

    err = exc_info.value
    assert "Leftover garbage after end of expression" in err.message
    assert err.token_text == "extra"
    assert err.line == 1
    assert err.column == 8


def test_parse_sexp_decorates_source_path_into_error_message() -> None:
    """Optional source_path surfaces in error str() for file-aware logs."""
    with pytest.raises(SexprError) as exc_info:
        parse_sexp("(root))", source_path="boards/example.kicad_pcb")

    err = exc_info.value
    assert err.source_path == "boards/example.kicad_pcb"
    assert "in boards/example.kicad_pcb" in str(err)


def test_roundtrip_sexp_text_decorates_source_path_in_error() -> None:
    """roundtrip_sexp_text records the source path for corpus failure listings."""
    result = roundtrip_sexp_text(
        '(root "unterminated)', source_path="broken.kicad_pcb"
    )

    assert result.phase == "lex"
    assert "broken.kicad_pcb" in (result.error or "")
    assert "Unterminated delimited string" in (result.error or "")


def test_sexpr_error_with_source_path_returns_decorated_copy() -> None:
    """with_source_path returns a copy preserving structured fields."""
    err = SexprError(
        "Bad thing",
        offset=42,
        line=3,
        column=7,
        token_text="oops",
    )

    decorated = err.with_source_path("file.kicad_pcb")
    assert decorated.message == "Bad thing"
    assert decorated.offset == 42
    assert decorated.line == 3
    assert decorated.column == 7
    assert decorated.token_text == "oops"
    assert decorated.source_path == "file.kicad_pcb"
    # No-op when source_path is None.
    assert err.with_source_path(None) is err


def test_parse_sexp_with_spans_records_every_list_node() -> None:
    """Every parsed list gets a SexpSpan keyed by id(list)."""
    text = "(root (child 1) (other 2))"
    tree, spans = parse_sexp_with_spans(text)

    assert tree == ["root", ["child", 1], ["other", 2]]
    # Root list + two inner lists = 3 spans.
    assert len(spans) == 3

    root_span = spans[id(tree)]
    assert isinstance(root_span, SexpSpan)
    assert root_span.offset == 0
    assert root_span.line == 1
    assert root_span.column == 1
    # End offset is one past the closing `)` so slicing recovers the source.
    assert text[root_span.offset:root_span.end_offset] == text

    inner_span = spans[id(tree[1])]
    assert text[inner_span.offset:inner_span.end_offset] == "(child 1)"


def test_parse_sexp_with_spans_handles_multiline_input() -> None:
    text = "(root\n  (child 1)\n  (other 2)\n)"
    tree, spans = parse_sexp_with_spans(text)

    assert tree[1] == ["child", 1]
    child_span = spans[id(tree[1])]
    assert child_span.line == 2
    assert child_span.column == 3
    assert text[child_span.offset:child_span.end_offset] == "(child 1)"

    other_span = spans[id(tree[2])]
    assert other_span.line == 3
    assert other_span.column == 3


def test_parse_sexp_with_spans_propagates_source_path_on_error() -> None:
    with pytest.raises(SexprError) as exc_info:
        parse_sexp_with_spans("(root))", source_path="board.kicad_pcb")

    assert exc_info.value.source_path == "board.kicad_pcb"


def test_debug_dump_tokens_emits_line_column_kind_and_text() -> None:
    dump = debug_dump_tokens("(root 1)", source_path="ex.kicad_pcb")

    # Header names the file.
    assert "ex.kicad_pcb" in dump
    assert "4 tokens" in dump
    # Each token line shows kind + position metadata.
    assert "kind=left" in dump
    assert "kind=atom" in dump
    assert "kind=number" in dump
    assert "kind=right" in dump
    assert "line=1" in dump
    assert "col=1" in dump


def test_debug_dump_tokens_respects_limit_and_reports_remainder() -> None:
    text = "(root 1 2 3 4 5 6 7 8 9 10)"
    total_tokens = len(lex_sexp(text))
    dump = debug_dump_tokens(text, limit=3)

    assert f"{total_tokens - 3} more tokens" in dump
    # Header + 3 token lines + ellipsis = at least 5 lines.
    assert dump.count("\n") >= 4


# -----------------------------------------------------------------------------
# Phase 4: typed parse error subclasses — lex vs tree vs dialect stage pinning.
# -----------------------------------------------------------------------------


def test_lexer_error_raises_typed_subclass() -> None:
    """Unterminated string is a lex-stage failure → SexprLexError."""
    with pytest.raises(SexprLexError) as exc_info:
        parse_sexp('(root "unterminated)')

    err = exc_info.value
    assert isinstance(err, SexprError), "subclass must remain a SexprError"
    assert err.phase == "lex"
    assert err.message == "Unterminated delimited string"


def test_parser_error_raises_typed_subclass() -> None:
    """Missing opening paren is a tree-stage failure → SexprTreeError."""
    with pytest.raises(SexprTreeError) as exc_info:
        parse_sexp("root child 1")

    err = exc_info.value
    assert isinstance(err, SexprError)
    assert err.phase == "tree"
    assert err.message == "Missing initial opening parenthesis"


def test_unbalanced_open_paren_is_tree_error() -> None:
    with pytest.raises(SexprTreeError) as exc_info:
        parse_sexp("(root\n  (inner_unclosed 1 2 3")

    assert exc_info.value.phase == "tree"


def test_leftover_garbage_is_tree_error() -> None:
    with pytest.raises(SexprTreeError) as exc_info:
        parse_sexp("(root) extra")

    assert exc_info.value.phase == "tree"


def test_legacy_sexpr_error_catch_still_matches_typed_subclasses() -> None:
    """Existing ``except SexprError`` blocks keep working after Phase 4."""
    with pytest.raises(SexprError) as lex_caught:
        parse_sexp('(root "unterminated)')
    assert isinstance(lex_caught.value, SexprLexError)

    with pytest.raises(SexprError) as tree_caught:
        parse_sexp("root child 1")
    assert isinstance(tree_caught.value, SexprTreeError)


def test_typed_error_with_source_path_preserves_subclass() -> None:
    """with_source_path returns the same concrete subclass, not a parent."""
    err = SexprLexError("Bad token", offset=3, line=1, column=4)
    decorated = err.with_source_path("file.kicad_pcb")

    assert type(decorated) is SexprLexError
    assert decorated.phase == "lex"
    assert decorated.source_path == "file.kicad_pcb"

    tree_err = SexprTreeError("Bad tree", offset=5, line=2, column=1)
    tree_decorated = tree_err.with_source_path("file.kicad_pcb")
    assert type(tree_decorated) is SexprTreeError
    assert tree_decorated.phase == "tree"


def test_dialect_error_subclass_is_reserved_for_future_use() -> None:
    """No production raises SexprDialectError today; the class is reserved."""
    err = SexprDialectError("placeholder")
    assert isinstance(err, SexprError)
    assert err.phase == "dialect"

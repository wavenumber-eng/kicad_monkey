use kicad_monkey_core::{
    ErrorKind, ErrorPhase, Limits, Patch, Sexp, TokenKind, apply_patches, apply_patches_with_limit,
    build, build_with_limit, lex, parse, parse_bytes, parse_with_limits,
};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

fn atom(value: &str) -> Sexp {
    Sexp::Atom(value.to_owned())
}

fn quoted(value: &str) -> Sexp {
    Sexp::Quoted(value.to_owned())
}

#[test]
fn bounded_builder_refuses_output_growth_before_appending_past_the_limit() {
    let tree = list(vec![atom("root"), quoted("a\nb")]);
    let built = build(&tree).expect("tree should build");
    assert_eq!(
        build_with_limit(&tree, built.len()).expect("exact limit should pass"),
        built
    );
    let error = build_with_limit(&tree, built.len() - 1).expect_err("limit should fail");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
    assert_eq!(error.phase, ErrorPhase::Build);
}

fn list(values: Vec<Sexp>) -> Sexp {
    Sexp::List(values)
}

#[test]
fn lexer_preserves_borrowed_token_kinds_and_byte_positions() {
    let source = "(root\r\n  \"a\r\nb\" 2)";
    let tokens = lex(source).expect("fixture should lex");

    assert_eq!(
        tokens.iter().map(|token| token.kind).collect::<Vec<_>>(),
        vec![
            TokenKind::Left,
            TokenKind::Atom,
            TokenKind::QuotedString,
            TokenKind::Integer,
            TokenKind::Right,
        ]
    );
    assert_eq!(tokens[2].position.line, 2);
    assert_eq!(tokens[2].position.column, 3);
    assert_eq!(tokens[3].position.line, 3);
    assert_eq!(tokens[3].position.column, 4);
    assert_eq!(
        &source[tokens[2].position.offset..][..tokens[2].lexeme.len()],
        tokens[2].lexeme
    );
}

#[test]
fn parser_supports_adjacent_lists_and_whole_line_comments() {
    let parsed = parse("\n  # whole-line comment\r\n(root(child 1)(next 2) #not-a-comment)\n")
        .expect("fixture should parse");

    assert_eq!(
        parsed,
        list(vec![
            atom("root"),
            list(vec![atom("child"), Sexp::Integer(1)]),
            list(vec![atom("next"), Sexp::Integer(2)]),
            atom("#not-a-comment"),
        ])
    );
}

#[test]
fn parser_matches_python_number_boundaries() {
    let parsed = parse("(root 123 -2. +0.5 .25 1e-3 1e 1abc +. -)").expect("fixture should parse");

    assert_eq!(
        parsed,
        list(vec![
            atom("root"),
            Sexp::Integer(123),
            Sexp::Float(-2.0),
            Sexp::Float(0.5),
            Sexp::Float(0.25),
            Sexp::Float(0.001),
            atom("1e"),
            atom("1abc"),
            atom("+."),
            atom("-"),
        ])
    );
}

#[test]
fn parser_decodes_kicad_string_escapes_and_normalizes_crlf() {
    let parsed =
        parse("(root \"\\101\\x42\\n\\t\\\"\\\\\" \"a\r\nb\")").expect("fixture should parse");

    assert_eq!(
        parsed,
        list(vec![atom("root"), quoted("AB\n\t\"\\"), quoted("a\nb")])
    );
}

#[test]
fn parser_normalizes_the_kicad_teardrops_dialect() {
    let parsed = parse("(teardrops (curved_edges no)filter_ratio 0.9)(enabled yes))")
        .expect("dialect fixture should parse");

    assert_eq!(
        parsed,
        list(vec![
            atom("teardrops"),
            list(vec![atom("curved_edges"), atom("no")]),
            list(vec![atom("filter_ratio"), Sexp::Float(0.9)]),
            list(vec![atom("enabled"), atom("yes")]),
        ])
    );
}

#[test]
fn deterministic_build_is_semantically_stable_on_second_write() {
    let source = "(root (child 1 2) (text \"Line 1\\nLine 2\") (value -2.5e-3))";
    let first_tree = parse(source).expect("fixture should parse");
    let first_write = build(&first_tree).expect("tree should build");
    let second_tree = parse(&first_write).expect("first output should parse");
    let second_write = build(&second_tree).expect("second tree should build");

    assert_eq!(second_tree, first_tree);
    assert_eq!(second_write, first_write);
}

#[test]
fn source_patches_preserve_untouched_bytes_and_semantics() {
    let source = "# retained\r\n(root  old-value\r\n  (child 1))\r\n";
    let start = source.find("old-value").expect("fixture token");
    let end = start + "old-value".len();
    let patched = apply_patches(source, &[Patch::new(start, end, "new-value")])
        .expect("valid patch should apply");

    assert_eq!(
        patched,
        "# retained\r\n(root  new-value\r\n  (child 1))\r\n"
    );
    assert_eq!(
        parse(&patched).expect("patched source should parse"),
        list(vec![
            atom("root"),
            atom("new-value"),
            list(vec![atom("child"), Sexp::Integer(1)]),
        ])
    );
}

#[test]
fn source_patches_reject_overlap_invalid_boundaries_and_output_growth() {
    let overlap = apply_patches(
        "(root value)",
        &[Patch::new(1, 5, "a"), Patch::new(4, 6, "b")],
    )
    .expect_err("overlapping patches must fail");
    assert_eq!(overlap.kind, ErrorKind::InvalidPatch);

    let unicode = "(root cafe\u{301})";
    let combining_mark = unicode.find('\u{301}').expect("combining mark");
    let invalid_boundary = apply_patches(
        unicode,
        &[Patch::new(combining_mark + 1, combining_mark + 1, "x")],
    )
    .expect_err("mid-codepoint patch must fail");
    assert_eq!(invalid_boundary.kind, ErrorKind::InvalidPatch);

    let growth = apply_patches_with_limit("(root)", &[Patch::new(5, 5, " too-large")], 8)
        .expect_err("output limit must fail");
    assert_eq!(growth.kind, ErrorKind::ResourceLimit);
}

#[test]
fn malformed_inputs_report_stable_phase_kind_and_position() {
    let unterminated = parse("(root \"unterminated)").expect_err("string must fail");
    assert_eq!(unterminated.phase, ErrorPhase::Lex);
    assert_eq!(unterminated.kind, ErrorKind::UnterminatedString);
    assert_eq!(unterminated.position.expect("position").column, 7);

    let missing_open = parse("root child 1)").expect_err("missing opener must fail");
    assert_eq!(missing_open.phase, ErrorPhase::Tree);
    assert_eq!(missing_open.kind, ErrorKind::MissingOpeningParenthesis);

    let extra_close = parse("(root))").expect_err("extra closer must fail");
    assert_eq!(extra_close.kind, ErrorKind::UnbalancedClosingParenthesis);

    let leftover = parse("(root) extra").expect_err("leftover token must fail");
    assert_eq!(leftover.kind, ErrorKind::LeftoverGarbage);
}

#[test]
fn byte_entry_point_rejects_invalid_utf8_at_the_first_bad_byte() {
    let error = parse_bytes(b"(root \xff)").expect_err("invalid UTF-8 must fail");

    assert_eq!(error.phase, ErrorPhase::Lex);
    assert_eq!(error.kind, ErrorKind::InvalidUtf8);
    assert_eq!(error.position.expect("position").offset, 6);
}

#[test]
fn explicit_limits_fail_closed() {
    let limits = Limits {
        max_source_bytes: 64,
        max_depth: 1,
        max_nodes: 100,
        max_decoded_string_bytes: 8,
    };
    let depth_error =
        parse_with_limits("(root (child (deep 1)))", limits).expect_err("depth limit must fail");
    assert_eq!(depth_error.kind, ErrorKind::ResourceLimit);

    let string_error = parse_with_limits("(root \"123456789\")", limits)
        .expect_err("decoded string limit must fail");
    assert_eq!(string_error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn parser_handles_deep_and_wide_stress_fixtures() {
    let mut nested = "(deepest value)".to_owned();
    for level in (1..=200).rev() {
        nested = format!("(level_{level} {nested})");
    }
    let deep = parse(&format!("(root {nested})")).expect("deep fixture should parse");
    assert!(matches!(deep, Sexp::List(_)));

    let siblings = (0..500)
        .map(|index| format!("(item_{index} {index} \"v{index}\")"))
        .collect::<String>();
    let wide = parse(&format!("(root{siblings})")).expect("wide fixture should parse");
    let Sexp::List(values) = wide else {
        panic!("root must be a list");
    };
    assert_eq!(values.len(), 501);
}

#[derive(Debug, Deserialize)]
struct VectorFile {
    schema: String,
    cases: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
struct VectorCase {
    id: String,
    source: String,
    phase: String,
    built: Option<String>,
    message_contains: Option<String>,
}

#[test]
fn rust_parser_matches_language_neutral_vectors() {
    let vector_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/parity/sexpr_l0_vectors.a0.json");
    let vector_text = fs::read_to_string(&vector_path).expect("vector file should be readable");
    let vectors: VectorFile =
        serde_json::from_str(&vector_text).expect("vector file should satisfy its a0 shape");
    assert_eq!(vectors.schema, "kicad_monkey.sexpr_parity_vectors.a0");

    for case in vectors.cases {
        match case.phase.as_str() {
            "ok" => {
                let parsed = parse(&case.source)
                    .unwrap_or_else(|error| panic!("{} should parse: {error}", case.id));
                let first = build(&parsed)
                    .unwrap_or_else(|error| panic!("{} should build: {error}", case.id));
                assert_eq!(Some(&first), case.built.as_ref(), "{}", case.id);
                let reparsed = parse(&first)
                    .unwrap_or_else(|error| panic!("{} output should parse: {error}", case.id));
                let second = build(&reparsed).unwrap_or_else(|error| {
                    panic!("{} second tree should build: {error}", case.id)
                });
                assert_eq!(reparsed, parsed, "{}", case.id);
                assert_eq!(second, first, "{}", case.id);
            }
            expected_phase => {
                let error = match parse(&case.source) {
                    Ok(value) => panic!("{} should fail, got {value:?}", case.id),
                    Err(error) => error,
                };
                let actual_phase = match error.phase {
                    ErrorPhase::Lex => "lex",
                    ErrorPhase::Tree => "tree",
                    ErrorPhase::Build => "build",
                };
                assert_eq!(actual_phase, expected_phase, "{}", case.id);
                if let Some(message) = case.message_contains {
                    assert!(error.to_string().contains(&message), "{}: {error}", case.id);
                }
            }
        }
    }
}

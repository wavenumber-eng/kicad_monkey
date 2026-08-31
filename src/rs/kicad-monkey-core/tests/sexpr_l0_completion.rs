use kicad_monkey_core::{
    Error, ErrorKind, ErrorPhase, FormatOptions, ProjectionLimits, Selector, Sexp, StructuralIndex,
    find_path, format, parse_form, read_form_bytes, remove_all_elements, remove_element,
    replace_element, scan_form_spans, scan_form_spans_with_limits, scan_reader_form_spans,
    set_value, transform_descendants, walk,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::io::{Cursor, Read};

const PCB: &str = "(kicad_pcb\n  # (footprint \"Ignored:Comment\")\n  (setup\n    (aux_axis_origin 1.0 2.0)\n  )\n  (footprint \"Demo:R_0805\"\n    (property \"Reference\" \"R1\")\n    (fp_text user \"text with ) in a string\")\n    (pad \"1\" smd rect (at 1 2))\n  )\n  (footprint \"Demo:C_0805\"\n    (property \"Reference\" \"C1\")\n    (model \"models/name(with-parens).step\")\n  )\n)\n";

fn atoms(values: &[&str]) -> Vec<Sexp> {
    values
        .iter()
        .map(|value| Sexp::Atom((*value).to_owned()))
        .collect()
}

fn named(name: &str, value: Sexp) -> Sexp {
    Sexp::List(vec![Sexp::Atom(name.to_owned()), value])
}

#[test]
fn formatter_matches_the_python_l0_examples() {
    assert_eq!(
        format(
            "(outer (inner 1 2) (another 3 4))",
            FormatOptions::default()
        )
        .expect("valid expression should format"),
        "(outer\n  (inner 1 2\n  )\n  (another 3 4\n  )\n)\n"
    );
    assert_eq!(
        format(
            "(a (b (c (d 1))))",
            FormatOptions {
                indentation_size: 4,
                max_nesting: 2,
            }
        )
        .expect("valid expression should format"),
        "(a\n    (b\n        (c (d 1) )\n    )\n)\n"
    );
}

#[test]
fn selectors_match_exact_paths_depths_and_source_bytes() {
    let selector = Selector {
        paths: Some(BTreeSet::from([vec![
            "kicad_pcb".to_owned(),
            "footprint".to_owned(),
        ]])),
        min_depth: Some(1),
        max_depth: Some(1),
        ..Selector::default()
    };
    let spans = scan_form_spans(PCB, &selector).expect("board should scan");

    assert_eq!(spans.len(), 2);
    assert_eq!(
        spans
            .iter()
            .map(|span| span.head.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("footprint"), Some("footprint")]
    );
    assert!(spans.iter().all(|span| span.depth == 1));
    assert!(
        spans[0]
            .text(PCB)
            .expect("span should belong to source")
            .starts_with("(footprint \"Demo:R_0805\"")
    );
    assert!(!spans.iter().any(|span| {
        span.text(PCB)
            .expect("valid span")
            .contains("Ignored:Comment")
    }));
}

#[test]
fn structural_scanners_preserve_legacy_teardrop_dialect_boundaries() {
    let source = r#"(kicad_pcb
  (footprint "Demo"
    (pad "1" smd rect
      (teardrops (curved_edges no)filter_ratio 0.9)
        (enabled yes) (allow_two_segments yes))
      (uuid pad-id))))"#;
    let selector = Selector {
        heads: Some(BTreeSet::from([
            "pad".to_owned(),
            "teardrops".to_owned(),
            "enabled".to_owned(),
            "uuid".to_owned(),
        ])),
        ..Selector::default()
    };
    let memory = scan_form_spans(source, &selector).expect("memory scanner");
    let streaming = scan_reader_form_spans(
        Cursor::new(source.as_bytes()),
        &selector,
        ProjectionLimits::default(),
    )
    .expect("streaming scanner");
    assert_eq!(streaming, memory);

    let pad = memory
        .iter()
        .find(|span| span.head.as_deref() == Some("pad"))
        .expect("pad span");
    assert!(
        pad.text(source)
            .expect("pad source")
            .contains("(uuid pad-id)")
    );
    let teardrops = memory
        .iter()
        .find(|span| span.head.as_deref() == Some("teardrops"))
        .expect("teardrop span");
    let teardrop_source = teardrops.text(source).expect("teardrop source");
    assert!(teardrop_source.contains("filter_ratio 0.9)"));
    assert!(teardrop_source.contains("(enabled yes)"));
    assert!(!teardrop_source.contains("(uuid pad-id)"));
    assert_eq!(
        memory
            .iter()
            .find(|span| span.head.as_deref() == Some("enabled"))
            .expect("enabled span")
            .path,
        ["kicad_pcb", "footprint", "pad", "teardrops", "enabled"]
    );

    let curve_points = source.replace("filter_ratio 0.9", "curve_points 4");
    let memory = scan_form_spans(&curve_points, &selector).expect("curve-points memory scanner");
    let streaming = scan_reader_form_spans(
        Cursor::new(curve_points.as_bytes()),
        &selector,
        ProjectionLimits::default(),
    )
    .expect("curve-points streaming scanner");
    assert_eq!(streaming, memory);
    assert!(
        memory
            .iter()
            .find(|span| span.head.as_deref() == Some("pad"))
            .expect("curve-points pad")
            .text(&curve_points)
            .expect("pad source")
            .contains("(uuid pad-id)")
    );
    assert_eq!(
        memory
            .iter()
            .find(|span| span.head.as_deref() == Some("enabled"))
            .expect("curve-points enabled")
            .path,
        ["kicad_pcb", "footprint", "pad", "teardrops", "enabled"]
    );
}

#[test]
fn selected_nested_form_has_exact_position_and_materializes_on_demand() {
    let selector = Selector {
        paths: Some(BTreeSet::from([vec![
            "kicad_pcb".to_owned(),
            "footprint".to_owned(),
            "model".to_owned(),
        ]])),
        ..Selector::default()
    };
    let spans = scan_form_spans(PCB, &selector).expect("board should scan");
    let [span] = spans.as_slice() else {
        panic!("expected one model form");
    };

    assert_eq!(span.start.line, 13);
    assert_eq!(
        span.text(PCB).expect("valid span"),
        "(model \"models/name(with-parens).step\")"
    );
    assert_eq!(
        parse_form(PCB, span).expect("selected form should parse"),
        Sexp::List(vec![
            Sexp::Atom("model".to_owned()),
            Sexp::Quoted("models/name(with-parens).step".to_owned()),
        ])
    );
}

#[test]
fn selector_pruning_skips_descendants_but_retains_the_parent() {
    let selector = Selector {
        heads: Some(BTreeSet::from(["footprint".to_owned(), "pad".to_owned()])),
        prune_heads: BTreeSet::from(["footprint".to_owned()]),
        ..Selector::default()
    };
    let spans = scan_form_spans(PCB, &selector).expect("board should scan");

    assert_eq!(
        spans
            .iter()
            .map(|span| span.head.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("footprint"), Some("footprint")]
    );
    assert!(spans[0].text(PCB).expect("valid span").contains("(pad "));
}

#[test]
fn reusable_index_answers_multiple_queries_without_owning_source() {
    let index = StructuralIndex::new(PCB).expect("board should index");
    assert_eq!(index.source_len(), PCB.len());
    assert!(index.forms().len() > 10);

    let selector = Selector {
        heads: Some(BTreeSet::from(["property".to_owned()])),
        ..Selector::default()
    };
    let selected = index.select(&selector).expect("selector should be valid");
    assert_eq!(selected.len(), 2);
    assert_eq!(
        selected[1].text(PCB).expect("valid indexed span"),
        "(property \"Reference\" \"C1\")"
    );
}

#[test]
fn projection_errors_pin_lex_and_tree_locations() {
    let string_error = scan_form_spans(
        "(kicad_pcb (title_block \"missing end)",
        &Selector::default(),
    )
    .expect_err("unterminated string must fail");
    assert_eq!(string_error.phase, ErrorPhase::Lex);
    assert_eq!(string_error.position.expect("position").column, 25);

    let open_error = scan_form_spans("(kicad_pcb\n  (setup)", &Selector::default())
        .expect_err("unbalanced opening parenthesis must fail");
    assert_eq!(open_error.phase, ErrorPhase::Tree);
    assert_eq!(open_error.position.expect("position").line, 1);
}

#[test]
fn mutation_primitives_match_python_tree_behavior() {
    let mut root = Sexp::List(vec![
        Sexp::Atom("root".to_owned()),
        named("version", Sexp::Integer(1)),
        named("version", Sexp::Integer(2)),
        named("name", Sexp::Atom("foo".to_owned())),
    ]);

    assert!(replace_element(
        &mut root,
        "version",
        named("version", Sexp::Integer(99))
    ));
    assert_eq!(
        remove_element(&mut root, "version"),
        Some(named("version", Sexp::Integer(99)))
    );
    set_value(&mut root, "enabled", Sexp::Atom("yes".to_owned()))
        .expect("list root should accept value");
    let expected = named("enabled", Sexp::Atom("yes".to_owned()));
    assert_eq!(find_path(&root, &["enabled"]), Some(&expected));
}

#[test]
fn mutation_walk_remove_all_and_transform_are_depth_first_and_stable() {
    let mut root = Sexp::List(vec![
        Sexp::Atom("root".to_owned()),
        Sexp::List(vec![
            Sexp::Atom("symbol".to_owned()),
            Sexp::List(vec![
                Sexp::Atom("property".to_owned()),
                Sexp::Atom("Reference".to_owned()),
                Sexp::Atom("U".to_owned()),
            ]),
        ]),
        named("line", Sexp::Integer(1)),
        named("line", Sexp::Integer(2)),
    ]);

    assert_eq!(walk(&root).count(), 5);
    assert_eq!(remove_all_elements(&mut root, "line").len(), 2);
    let count = transform_descendants(&mut root, "property", &mut |value| {
        let Sexp::List(values) = value else {
            return value.clone();
        };
        let mut replacement = values.clone();
        replacement[1] = Sexp::Atom("reference".to_owned());
        Sexp::List(replacement)
    });
    assert_eq!(count, 1);
    let expected = Sexp::List(atoms(&["property", "reference", "U"]));
    assert_eq!(find_path(&root, &["symbol", "property"]), Some(&expected));
}

#[test]
fn invalid_selector_and_foreign_span_fail_closed() {
    let invalid = Selector {
        min_depth: Some(2),
        max_depth: Some(1),
        ..Selector::default()
    };
    assert!(scan_form_spans("(root)", &invalid).is_err());

    let [span] = scan_form_spans("(root)", &Selector::default())
        .expect("source should scan")
        .try_into()
        .expect("one span");
    assert!(span.text("x").is_err());
}

struct ChunkedReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    chunk_size: usize,
}

impl Read for ChunkedReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if self.offset == self.bytes.len() {
            return Ok(0);
        }
        let count = output
            .len()
            .min(self.chunk_size)
            .min(self.bytes.len() - self.offset);
        output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

#[test]
fn native_stream_scan_matches_memory_scan_across_every_small_chunk_size() {
    let source = "\u{a0}# retained\r\n(kicad_pcb\u{2003}(footprint \"Démø:R\" (property \"x\\\"y\" \"µ\"))\n  (footprint \"C\" (model \"a(b).step\")))";
    let selector = Selector {
        heads: Some(BTreeSet::from([
            "footprint".to_owned(),
            "property".to_owned(),
            "model".to_owned(),
        ])),
        ..Selector::default()
    };
    let expected = scan_form_spans(source, &selector).expect("memory scan should pass");

    for chunk_size in 1..=17 {
        let reader = ChunkedReader {
            bytes: source.as_bytes(),
            offset: 0,
            chunk_size,
        };
        let actual = scan_reader_form_spans(reader, &selector, ProjectionLimits::default())
            .unwrap_or_else(|error| panic!("chunk {chunk_size} failed: {error}"));
        assert_eq!(actual, expected, "chunk size {chunk_size}");
    }
}

#[derive(Deserialize)]
struct ProjectionVectors {
    schema: String,
    source: String,
    spans: Vec<ProjectionVectorSpan>,
    selections: Vec<ProjectionVectorSelection>,
}

#[derive(Deserialize)]
struct ProjectionVectorSpan {
    head: Option<String>,
    path: Vec<String>,
    depth: usize,
    start_byte: usize,
    end_byte: usize,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
}

#[derive(Deserialize)]
struct ProjectionVectorSelection {
    id: String,
    selector: ProjectionVectorSelector,
    span_indices: Vec<usize>,
}

#[derive(Deserialize)]
struct ProjectionVectorSelector {
    heads: Option<Vec<String>>,
    paths: Option<Vec<Vec<String>>>,
    min_depth: Option<usize>,
    max_depth: Option<usize>,
    #[serde(default)]
    prune_heads: Vec<String>,
}

impl ProjectionVectorSelector {
    fn into_selector(self) -> Selector {
        Selector {
            heads: self.heads.map(BTreeSet::from_iter),
            paths: self.paths.map(BTreeSet::from_iter),
            min_depth: self.min_depth,
            max_depth: self.max_depth,
            prune_heads: BTreeSet::from_iter(self.prune_heads),
        }
    }
}

#[test]
fn memory_and_stream_projection_match_language_neutral_span_vectors() {
    let vectors: ProjectionVectors = serde_json::from_str(include_str!(
        "../../../../tests/parity/sexpr_projection_vectors.a0.json"
    ))
    .expect("projection vectors should decode");
    assert_eq!(vectors.schema, "kicad_monkey.sexpr_projection_vectors.a0");
    let memory = scan_form_spans(&vectors.source, &Selector::default()).expect("memory scan");
    let streaming = scan_reader_form_spans(
        Cursor::new(vectors.source.as_bytes()),
        &Selector::default(),
        ProjectionLimits::default(),
    )
    .expect("stream scan");
    assert_eq!(streaming, memory);
    assert_eq!(memory.len(), vectors.spans.len());

    for (actual, expected) in memory.iter().zip(&vectors.spans) {
        assert_eq!(actual.head, expected.head);
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.depth, expected.depth);
        assert_eq!(actual.range, expected.start_byte..expected.end_byte);
        assert_eq!(actual.start.offset, expected.start_byte);
        assert_eq!(actual.start.line, expected.line);
        assert_eq!(actual.start.column, expected.column);
        assert_eq!(actual.end.offset, expected.end_byte);
        assert_eq!(actual.end.line, expected.end_line);
        assert_eq!(actual.end.column, expected.end_column);
    }
}

#[test]
fn memory_and_stream_projection_match_across_selector_combinations() {
    let vectors: ProjectionVectors = serde_json::from_str(include_str!(
        "../../../../tests/parity/sexpr_projection_vectors.a0.json"
    ))
    .expect("projection vectors should decode");
    let all = scan_form_spans(&vectors.source, &Selector::default()).expect("all spans");
    let index = StructuralIndex::new(&vectors.source).expect("structural index");
    for case in vectors.selections {
        let selector = case.selector.into_selector();
        let expected = case
            .span_indices
            .iter()
            .map(|index| &all[*index])
            .collect::<Vec<_>>();
        let memory = scan_form_spans(&vectors.source, &selector).expect("memory scan");
        let streaming = scan_reader_form_spans(
            Cursor::new(vectors.source.as_bytes()),
            &selector,
            ProjectionLimits::default(),
        )
        .expect("stream scan");
        assert_eq!(
            memory.iter().collect::<Vec<_>>(),
            expected,
            "memory selector {}",
            case.id
        );
        assert_eq!(
            streaming.iter().collect::<Vec<_>>(),
            expected,
            "stream selector {}",
            case.id
        );
        assert_eq!(
            index.select(&selector).expect("indexed selection"),
            expected,
            "index selector {}",
            case.id
        );
    }

    let malformed = "(root (prune (hidden \"unterminated)))";
    let selector = Selector {
        prune_heads: BTreeSet::from(["prune".to_owned()]),
        ..Selector::default()
    };
    assert_eq!(
        scan_form_spans(malformed, &selector).expect_err("memory must still validate lexing"),
        scan_reader_form_spans(
            Cursor::new(malformed.as_bytes()),
            &selector,
            ProjectionLimits::default(),
        )
        .expect_err("stream must still validate lexing")
    );
}

#[test]
fn native_stream_scan_preserves_tokens_split_at_internal_buffer_boundary() {
    let boundary_sources = [
        format!("#{}µ\n(root (child 1))", "x".repeat(65_534)),
        format!("#{}\r\n(root (child 1))", "x".repeat(65_534)),
        format!("(root (\"{}\\x42\" 1))", "a".repeat(65_527)),
    ];
    for source in boundary_sources {
        let expected = scan_form_spans(&source, &Selector::default()).expect("memory scan");
        let actual = scan_reader_form_spans(
            Cursor::new(source.as_bytes()),
            &Selector::default(),
            ProjectionLimits::default(),
        )
        .expect("stream scan");
        assert_eq!(actual, expected);
    }
}

#[test]
fn native_stream_index_supports_seeked_partial_form_reads() {
    let selector = Selector {
        heads: Some(BTreeSet::from(["model".to_owned()])),
        ..Selector::default()
    };
    let [span] = scan_reader_form_spans(
        Cursor::new(PCB.as_bytes()),
        &selector,
        ProjectionLimits::default(),
    )
    .expect("stream should scan")
    .try_into()
    .expect("one model span");
    let mut source = Cursor::new(PCB.as_bytes());
    let selected = read_form_bytes(&mut source, &span, 1024).expect("partial read should pass");

    assert_eq!(
        selected,
        b"(model \"models/name(with-parens).step\")".to_vec()
    );
    assert_eq!(
        read_form_bytes(&mut source, &span, 8)
            .expect_err("form limit must fail")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn partial_read_rejects_an_inverted_public_span() {
    let [mut span] = scan_form_spans("(root)", &Selector::default())
        .expect("source should scan")
        .try_into()
        .expect("one span");
    let start = span.range.end.saturating_sub(1);
    let end = span.range.start.saturating_add(2);
    span.range = start..end;
    let mut source = Cursor::new(b"(root)");
    assert_eq!(
        read_form_bytes(&mut source, &span, 1024)
            .expect_err("inverted span must fail")
            .kind,
        ErrorKind::InvalidSpan
    );
}

#[test]
fn projection_limits_and_stream_utf8_validation_fail_closed() {
    let limits = ProjectionLimits {
        max_source_bytes: 64,
        max_depth: 0,
        max_selected_forms: 1,
        max_head_bytes: 4,
    };
    assert_eq!(
        scan_form_spans_with_limits("(root (child 1))", &Selector::default(), limits)
            .expect_err("depth must be limited")
            .kind,
        ErrorKind::ResourceLimit
    );

    let invalid = scan_reader_form_spans(
        Cursor::new(b"(root \xf0\x28\x8c\x28)"),
        &Selector::default(),
        ProjectionLimits::default(),
    )
    .expect_err("invalid UTF-8 must fail");
    assert_eq!(invalid.kind, ErrorKind::InvalidUtf8);
    assert_eq!(invalid.position.expect("position").offset, 7);
}

fn scan_both_with_limits(
    source: &str,
    limits: ProjectionLimits,
) -> (
    Result<Vec<kicad_monkey_core::FormSpan>, Error>,
    Result<Vec<kicad_monkey_core::FormSpan>, Error>,
) {
    (
        scan_form_spans_with_limits(source, &Selector::default(), limits),
        scan_reader_form_spans(Cursor::new(source.as_bytes()), &Selector::default(), limits),
    )
}

#[test]
fn memory_and_stream_projection_enforce_exact_source_boundary() {
    let defaults = ProjectionLimits::default();
    let source = "(root (child 1))";
    for max_source_bytes in [source.len(), source.len() - 1] {
        let limits = ProjectionLimits {
            max_source_bytes,
            ..defaults
        };
        let (memory, stream) = scan_both_with_limits(source, limits);
        if max_source_bytes == source.len() {
            assert_eq!(
                memory.expect("exact source limit"),
                stream.expect("exact source limit")
            );
        } else {
            assert_eq!(
                memory.expect_err("source limit").kind,
                ErrorKind::ResourceLimit
            );
            assert_eq!(
                stream.expect_err("source limit").kind,
                ErrorKind::ResourceLimit
            );
        }
    }
}

#[test]
fn memory_and_stream_projection_enforce_exact_depth_boundary() {
    let defaults = ProjectionLimits::default();
    let source = "(root (child 1))";
    for max_depth in [1, 0] {
        let limits = ProjectionLimits {
            max_depth,
            ..defaults
        };
        let (memory, stream) = scan_both_with_limits(source, limits);
        if max_depth == 1 {
            assert_eq!(
                memory.expect("exact depth limit"),
                stream.expect("exact depth limit")
            );
        } else {
            assert_eq!(
                memory.expect_err("depth limit"),
                stream.expect_err("depth limit")
            );
        }
    }
}

#[test]
fn memory_and_stream_projection_enforce_exact_selection_boundary() {
    let defaults = ProjectionLimits::default();
    let source = "(root (child 1))";
    for max_selected_forms in [2, 1] {
        let limits = ProjectionLimits {
            max_selected_forms,
            ..defaults
        };
        let (memory, stream) = scan_both_with_limits(source, limits);
        if max_selected_forms == 2 {
            assert_eq!(
                memory.expect("exact selection limit"),
                stream.expect("exact selection limit")
            );
        } else {
            assert_eq!(
                memory.expect_err("selection limit"),
                stream.expect_err("selection limit")
            );
        }
    }
}

#[test]
fn memory_and_stream_projection_enforce_exact_raw_head_boundaries() {
    let defaults = ProjectionLimits::default();
    for (source, raw_head_bytes) in [("(root)", 4), (r#"("a\x42")"#, 7), ("(\"µ\")", 4)] {
        for max_head_bytes in [raw_head_bytes, raw_head_bytes - 1] {
            let limits = ProjectionLimits {
                max_head_bytes,
                ..defaults
            };
            let (memory, stream) = scan_both_with_limits(source, limits);
            if max_head_bytes == raw_head_bytes {
                assert_eq!(
                    memory.expect("exact head limit"),
                    stream.expect("exact head limit")
                );
            } else {
                assert_eq!(
                    memory.expect_err("head limit"),
                    stream.expect_err("head limit")
                );
            }
        }
    }
}

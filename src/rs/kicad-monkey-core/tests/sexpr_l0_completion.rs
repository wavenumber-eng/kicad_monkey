use kicad_monkey_core::{
    ErrorKind, ErrorPhase, FormatOptions, ProjectionLimits, Selector, Sexp, StructuralIndex,
    find_path, format, parse_form, read_form_bytes, remove_all_elements, remove_element,
    replace_element, scan_form_spans, scan_form_spans_with_limits, scan_reader_form_spans,
    set_value, transform_descendants, walk,
};
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

use kicad_monkey_core::{
    SchematicBundleLimits, SchematicDocument, SchematicDocumentLimits, SourceBundleErrorKind,
};
use std::io::{Cursor, Write};

const SOURCE: &str = r#"(kicad_sch
  (version 20250114)
  (generator eeschema)
  (uuid root-id)
  (future_root (nested "preserve exactly"))
  (symbol (lib_id "Demo:One") (at 1 2 0)
    (uuid symbol-a)
    (property "Reference" "R1" (at 1 2 0))
    (property "Value" "Old"))
  (symbol (lib_id "Demo:One") (at 3 4 0)
    (uuid symbol-b)
    (property "Reference" "R2")))
"#;

#[test]
fn owned_schematic_writes_exact_source_and_rebuilds_typed_state() {
    let document = SchematicDocument::parse(SOURCE.to_owned(), limits()).expect("document");
    assert_eq!(document.source_path(), "document.kicad_sch");
    assert_eq!(document.limits(), limits());
    let definition = document.definition().expect("typed definition");
    assert_eq!(definition.uuid.as_deref(), Some("root-id"));
    assert_eq!(definition.symbols.len(), 2);
    assert_eq!(definition.symbols[0].properties[1].value, "Old");

    let mut output = Vec::new();
    document.write_to(&mut output).expect("exact write");
    assert_eq!(output, SOURCE.as_bytes());
}

#[test]
fn symbol_property_edits_are_transactional_source_preserving_and_stable() {
    let mut document =
        SchematicDocument::parse_named("design/root.kicad_sch", SOURCE.to_owned(), limits())
            .expect("document");
    assert!(
        document
            .set_symbol_property("symbol-a", "Value", "New \"value\"")
            .expect("set")
    );
    assert!(
        !document
            .set_symbol_property("symbol-a", "Value", "New \"value\"")
            .expect("stable set")
    );
    assert!(
        document
            .upsert_symbol_property("symbol-b", "Value", "Second")
            .expect("insert")
    );
    assert!(
        !document
            .upsert_symbol_property("symbol-b", "Value", "Second")
            .expect("stable insert")
    );
    assert!(
        document
            .remove_symbol_property("symbol-a", "Reference")
            .expect("remove")
    );
    assert!(
        !document
            .remove_symbol_property("symbol-a", "Reference")
            .expect("stable removal")
    );

    assert!(
        document
            .source()
            .contains("(future_root (nested \"preserve exactly\"))")
    );
    assert!(
        document
            .source()
            .contains("(property \"Value\" \"New \\\"value\\\"\")")
    );
    let definition = document.definition().expect("semantic reparse");
    assert_eq!(definition.symbols[0].properties.len(), 1);
    assert_eq!(definition.symbols[0].properties[0].value, "New \"value\"");
    assert_eq!(definition.symbols[1].properties.len(), 2);
    assert_eq!(definition.symbols[1].properties[1].key, "Value");
    assert_eq!(definition.symbols[1].properties[1].value, "Second");
}

#[test]
fn insertion_preserves_crlf_and_reparses_one_line_symbols() {
    let source = "(kicad_sch\r\n  (uuid root)\r\n  (symbol (lib_id \"D:R\") (uuid one))\r\n)";
    let mut document =
        SchematicDocument::parse(source.to_owned(), limits()).expect("one-line document");
    assert!(
        document
            .upsert_symbol_property("one", "Value", "10k")
            .expect("insert")
    );
    assert!(document.source().contains(
        "(symbol (lib_id \"D:R\") (uuid one)\r\n    (property \"Value\" \"10k\")\r\n  )"
    ));
    assert!(!document.source().replace("\r\n", "").contains('\n'));
    assert_eq!(
        document.definition().expect("reparse").symbols[0].properties[0].value,
        "10k"
    );
}

#[test]
fn ambiguous_identity_and_property_fail_without_changing_source() {
    let duplicate_uuid = SOURCE.replace("symbol-b", "symbol-a");
    let mut document =
        SchematicDocument::parse(duplicate_uuid.clone(), limits()).expect("duplicate UUID source");
    let error = document
        .set_symbol_property("symbol-a", "Value", "changed")
        .expect_err("ambiguous UUID");
    assert_eq!(error.kind, SourceBundleErrorKind::Schematic);
    assert!(error.message.contains("UUID is ambiguous"));
    assert_eq!(document.source(), duplicate_uuid);

    let duplicate_property = SOURCE.replace(
        "(property \"Value\" \"Old\")",
        "(property \"Value\" \"Old\") (property \"Value\" \"Other\")",
    );
    let mut document = SchematicDocument::parse(duplicate_property.clone(), limits())
        .expect("duplicate property source");
    let error = document
        .upsert_symbol_property("symbol-a", "Value", "changed")
        .expect_err("ambiguous property");
    assert!(error.message.contains("property name is ambiguous"));
    assert_eq!(document.source(), duplicate_property);
}

#[test]
fn source_property_and_output_limits_fail_before_publication() {
    let direct_source_error = SchematicDocument::parse(
        SOURCE.to_owned(),
        SchematicDocumentLimits {
            parse: SchematicBundleLimits {
                max_source_bytes: SOURCE.len() - 1,
                ..SchematicBundleLimits::default()
            },
            ..SchematicDocumentLimits::default()
        },
    )
    .expect_err("direct source ceiling");
    assert_eq!(
        direct_source_error.kind,
        SourceBundleErrorKind::ResourceLimit
    );

    let exact = SchematicDocumentLimits {
        parse: SchematicBundleLimits {
            max_source_bytes: SOURCE.len(),
            ..SchematicBundleLimits::default()
        },
        max_output_bytes: SOURCE.len(),
    };
    let mut document =
        SchematicDocument::from_reader(Cursor::new(SOURCE.as_bytes()), exact).expect("exact read");
    let original = document.source().to_owned();
    let error = document
        .upsert_symbol_property("symbol-b", "Value", "growth")
        .expect_err("output ceiling");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert!(error.message.contains("max_output_bytes"));
    assert_eq!(document.source(), original);

    let strict_properties = SchematicDocumentLimits {
        parse: SchematicBundleLimits {
            max_symbol_properties_per_symbol: 1,
            ..SchematicBundleLimits::default()
        },
        ..SchematicDocumentLimits::default()
    };
    let one_property = "(kicad_sch (symbol (uuid one) (property \"A\" \"1\")))";
    let mut document = SchematicDocument::parse(one_property.to_owned(), strict_properties)
        .expect("at property limit");
    let error = document
        .upsert_symbol_property("one", "B", "2")
        .expect_err("property ceiling");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert_eq!(document.source(), one_property);

    let decoded_scalar = "(kicad_sch (symbol (uuid x) (property \"A\" \"12\")))";
    let error = SchematicDocument::parse(
        decoded_scalar.to_owned(),
        SchematicDocumentLimits {
            parse: SchematicBundleLimits {
                max_decoded_string_bytes: 1,
                ..SchematicBundleLimits::default()
            },
            ..SchematicDocumentLimits::default()
        },
    )
    .expect_err("decoded scalar ceiling");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
}

#[test]
fn named_reader_reports_named_utf8_io_and_source_limit_failures() {
    let error = SchematicDocument::from_named_reader(
        "named.kicad_sch",
        Cursor::new([b'(', 0xff, b')']),
        limits(),
    )
    .expect_err("UTF-8");
    assert_eq!(error.kind, SourceBundleErrorKind::Utf8);
    assert_eq!(error.source_path.as_deref(), Some("named.kicad_sch"));

    let error = SchematicDocument::from_named_reader("named.kicad_sch", FailingReader, limits())
        .expect_err("read error");
    assert_eq!(error.kind, SourceBundleErrorKind::Schematic);
    assert_eq!(error.source_path.as_deref(), Some("named.kicad_sch"));

    let strict = SchematicDocumentLimits {
        parse: SchematicBundleLimits {
            max_source_bytes: SOURCE.len() - 1,
            ..SchematicBundleLimits::default()
        },
        ..SchematicDocumentLimits::default()
    };
    let error = SchematicDocument::from_named_reader(
        "named.kicad_sch",
        Cursor::new(SOURCE.as_bytes()),
        strict,
    )
    .expect_err("source ceiling");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
    assert_eq!(error.source_path.as_deref(), Some("named.kicad_sch"));
}

#[test]
fn writer_io_failure_is_structured_and_absent_removal_obeys_output_limit() {
    let document = SchematicDocument::parse(SOURCE.to_owned(), limits()).expect("document");
    let error = document.write_to(FailingWriter).expect_err("write error");
    assert_eq!(error.kind, SourceBundleErrorKind::Schematic);

    let mut document = SchematicDocument::parse(
        SOURCE.to_owned(),
        SchematicDocumentLimits {
            max_output_bytes: SOURCE.len() - 1,
            ..limits()
        },
    )
    .expect("read allowed");
    let error = document
        .remove_symbol_property("symbol-a", "Missing")
        .expect_err("bounded no-op");
    assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
}

fn limits() -> SchematicDocumentLimits {
    SchematicDocumentLimits::default()
}

struct FailingReader;

impl std::io::Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("read failure"))
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("write failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

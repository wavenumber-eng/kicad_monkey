use kicad_monkey_core::{ErrorKind, PcbDocument, PcbLimits};
use std::io::{Cursor, Write};

const SOURCE: &str = r#"(kicad_pcb
  (property "Owner" "old")
  (image (at 1 2) (layer "F.SilkS") (uuid image-id))
  (group "remove" (uuid group-id) (members image-id))
  (future_board_data (nested "preserve exactly"))
)"#;

#[test]
fn owned_document_commits_reparsed_edits_and_stable_noops() {
    let mut document =
        PcbDocument::parse(SOURCE.to_owned(), PcbLimits::default()).expect("owned board");
    assert_eq!(document.limits(), PcbLimits::default());
    assert_eq!(document.view().expect("view").counts().properties, 1);

    assert!(
        document
            .set_property("Owner", "new \"owner\"")
            .expect("property")
    );
    assert!(
        !document
            .set_property("Owner", "new \"owner\"")
            .expect("stable property")
    );
    assert!(
        document
            .set_top_level_layer_by_id("image-id", "B.SilkS")
            .expect("layer")
    );
    assert!(
        !document
            .set_top_level_layer_by_id("image-id", "B.SilkS")
            .expect("stable layer")
    );
    assert!(document.remove_top_level_by_id("group-id").expect("remove"));
    assert!(
        !document
            .remove_top_level_by_id("group-id")
            .expect("stable remove")
    );

    assert!(
        document
            .source()
            .contains("(future_board_data (nested \"preserve exactly\"))")
    );
    assert!(
        document
            .source()
            .contains("(property \"Owner\" \"new \\\"owner\\\"\")")
    );
    assert!(document.source().contains("(layer \"B.SilkS\")"));
    assert!(!document.source().contains("group-id"));
    let reparsed = document.view().expect("semantic reparse");
    assert_eq!(reparsed.counts().images, 1);
    assert_eq!(reparsed.counts().groups, 0);
}

#[test]
fn owned_property_upsert_and_removal_match_python_mutation_semantics() {
    let mut document = PcbDocument::parse(
        "(kicad_pcb\r\n  (property \"Owner\" \"old\")\r\n  (future keep)\r\n)".to_owned(),
        PcbLimits::default(),
    )
    .expect("document");
    assert!(
        document
            .upsert_property("Revision", "A\"1")
            .expect("insert")
    );
    assert!(!document.upsert_property("Revision", "A\"1").expect("no-op"));
    assert!(document.upsert_property("Revision", "B").expect("update"));
    assert!(document.remove_property("Owner").expect("remove"));
    assert!(!document.remove_property("Owner").expect("absent no-op"));
    assert!(
        document
            .source()
            .contains("\r\n  (property \"Revision\" \"B\")\r\n")
    );
    assert!(document.source().contains("(future keep)"));
    assert!(!document.source().contains("property \"Owner\""));
    let properties = document
        .view()
        .expect("reparse")
        .properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("properties");
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0].name, "Revision");
    assert_eq!(properties[0].value, "B");
}

#[test]
fn property_upsert_and_removal_fail_closed_on_ambiguity_and_output_limits() {
    let duplicate = "(kicad_pcb (property \"Owner\" \"a\") (property \"Owner\" \"b\"))";
    let mut ambiguous =
        PcbDocument::parse(duplicate.to_owned(), PcbLimits::default()).expect("document");
    assert_eq!(
        ambiguous
            .upsert_property("Owner", "c")
            .expect_err("upsert ambiguity")
            .kind,
        ErrorKind::UnexpectedToken
    );
    assert_eq!(
        ambiguous
            .remove_property("Owner")
            .expect_err("remove ambiguity")
            .kind,
        ErrorKind::UnexpectedToken
    );
    assert_eq!(ambiguous.source(), duplicate);

    let source = "(kicad_pcb)";
    let mut limited = PcbDocument::parse(
        source.to_owned(),
        PcbLimits {
            max_output_bytes: source.len(),
            ..PcbLimits::default()
        },
    )
    .expect("document");
    assert_eq!(
        limited
            .upsert_property("Revision", "A")
            .expect_err("output limit")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(limited.source(), source);
    assert!(!limited.remove_property("Missing").expect("bounded no-op"));
}

#[test]
fn property_insertion_enforces_exact_count_limit_at_view_and_document_boundaries() {
    let source = "(kicad_pcb (property \"Owner\" \"old\"))";
    let exact_limits = PcbLimits {
        max_properties: 2,
        ..PcbLimits::default()
    };
    let edit = kicad_monkey_core::PcbView::parse(source, exact_limits)
        .expect("view")
        .upsert_property("Revision", "A")
        .expect("exact limit");
    assert!(edit.changed);
    assert_eq!(
        kicad_monkey_core::PcbView::parse(&edit.source, exact_limits)
            .expect("reparse")
            .counts()
            .properties,
        2
    );

    let strict_limits = PcbLimits {
        max_properties: 1,
        ..PcbLimits::default()
    };
    let view = kicad_monkey_core::PcbView::parse(source, strict_limits).expect("strict view");
    assert_eq!(
        view.upsert_property("Revision", "A")
            .expect_err("direct view ceiling")
            .kind,
        ErrorKind::ResourceLimit
    );
    let mut document =
        PcbDocument::parse(source.to_owned(), strict_limits).expect("strict document");
    assert_eq!(
        document
            .upsert_property("Revision", "A")
            .expect_err("owned ceiling")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(document.source(), source);
}

#[test]
fn failed_owned_mutations_leave_the_document_unchanged() {
    let ambiguous = SOURCE.replace(
        "(property \"Owner\" \"old\")",
        "(property \"Owner\" \"old\")\n  (property \"Owner\" \"other\")",
    );
    let mut document =
        PcbDocument::parse(ambiguous.clone(), PcbLimits::default()).expect("owned board");
    assert_eq!(
        document
            .set_property("Owner", "new")
            .expect_err("ambiguous")
            .kind,
        ErrorKind::UnexpectedToken
    );
    assert_eq!(document.source(), ambiguous);
}

#[test]
fn owned_reader_and_writer_obey_exact_byte_limits() {
    let exact = PcbLimits {
        max_source_bytes: SOURCE.len(),
        max_output_bytes: SOURCE.len(),
        ..PcbLimits::default()
    };
    let document =
        PcbDocument::from_reader(Cursor::new(SOURCE.as_bytes()), exact).expect("exact read");
    let mut output = Vec::new();
    document.write_to(&mut output).expect("exact write");
    assert_eq!(output, SOURCE.as_bytes());

    let read_error = PcbDocument::from_reader(
        Cursor::new(SOURCE.as_bytes()),
        PcbLimits {
            max_source_bytes: SOURCE.len() - 1,
            ..PcbLimits::default()
        },
    )
    .expect_err("source ceiling");
    assert_eq!(read_error.kind, ErrorKind::ResourceLimit);
    assert_eq!(
        PcbDocument::from_reader(Cursor::new([b'(', 0xff, b')']), PcbLimits::default())
            .expect_err("UTF-8")
            .kind,
        ErrorKind::InvalidUtf8
    );

    let strict = PcbDocument::parse(
        SOURCE.to_owned(),
        PcbLimits {
            max_output_bytes: SOURCE.len() - 1,
            ..PcbLimits::default()
        },
    )
    .expect("read allowed");
    let mut untouched = Vec::new();
    assert_eq!(
        strict
            .write_to(&mut untouched)
            .expect_err("output ceiling")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert!(untouched.is_empty());
}

#[test]
fn owned_stream_errors_use_the_structured_io_diagnostic() {
    let error =
        PcbDocument::from_reader(FailingReader, PcbLimits::default()).expect_err("read error");
    assert_eq!(error.kind, ErrorKind::Io);

    let document = PcbDocument::parse(SOURCE.to_owned(), PcbLimits::default()).expect("document");
    let error = document.write_to(FailingWriter).expect_err("write error");
    assert_eq!(error.kind, ErrorKind::Io);
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

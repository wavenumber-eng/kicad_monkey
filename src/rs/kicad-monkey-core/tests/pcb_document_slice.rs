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

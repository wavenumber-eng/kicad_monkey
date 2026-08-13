use kicad_monkey_core::{ErrorKind, PcbLimits, PcbView};

const METADATA: &str = r#"(kicad_pcb
  (paper "User" 420 297 portrait)
  (title_block
    (title "Control Board")
    (date "2026-08-13")
    (rev "B")
    (company "Wavenumber")
    (comment 2 "second")
    (comment 1 "first")
    (comment 2 "updated")
    (future_field "preserved")))"#;

#[test]
fn paper_and_title_block_match_shared_kicad_document_semantics() {
    let view = PcbView::parse(METADATA, PcbLimits::default()).expect("board");
    let paper = view.paper().expect("paper");
    assert_eq!(paper.size, "User");
    assert_eq!((paper.width, paper.height), (Some(420.0), Some(297.0)));
    assert!(paper.portrait);
    let paper_range = paper.source_range.expect("paper source range");
    assert_eq!(&METADATA[paper_range], "(paper \"User\" 420 297 portrait)");

    let title = view
        .title_block()
        .expect("title block read")
        .expect("title block");
    assert_eq!(title.title, "Control Board");
    assert_eq!(title.date, "2026-08-13");
    assert_eq!(title.revision, "B");
    assert_eq!(title.company, "Wavenumber");
    assert_eq!(
        title.comments.into_iter().collect::<Vec<_>>(),
        [(1, "first".to_owned()), (2, "updated".to_owned())]
    );
    assert!(METADATA[title.source_range].starts_with("(title_block"));
}

#[test]
fn missing_document_metadata_uses_python_compatible_defaults() {
    let view = PcbView::parse("(kicad_pcb)", PcbLimits::default()).expect("board");
    let paper = view.paper().expect("default paper");
    assert_eq!(paper.size, "A4");
    assert_eq!((paper.width, paper.height), (None, None));
    assert!(!paper.portrait);
    assert!(paper.source_range.is_none());
    assert!(view.title_block().expect("missing title block").is_none());
}

#[test]
fn title_block_limits_fail_before_unbounded_collection_growth() {
    let two_comments = "(kicad_pcb (title_block (comment 1 \"one\") (comment 2 \"two\")))";
    let exact = PcbView::parse(
        two_comments,
        PcbLimits {
            max_title_block_comments: 2,
            ..PcbLimits::default()
        },
    )
    .expect("board")
    .title_block()
    .expect("exact comment limit")
    .expect("title block");
    assert_eq!(exact.comments.len(), 2);

    let error = PcbView::parse(
        two_comments,
        PcbLimits {
            max_title_block_comments: 1,
            ..PcbLimits::default()
        },
    )
    .expect("board")
    .title_block()
    .expect_err("comment limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    let error = PcbView::parse(
        METADATA,
        PcbLimits {
            max_title_block_children: 1,
            ..PcbLimits::default()
        },
    )
    .expect("board")
    .title_block()
    .expect_err("child limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

use kicad_monkey_core::{
    ErrorKind, WorksheetCorner, WorksheetDocument, WorksheetFormat, WorksheetItem, WorksheetLimits,
    WorksheetView,
};
use std::io::{Cursor, Write};
use std::path::PathBuf;

const SYNTHETIC: &str = r#"(kicad_wks
  (version 20231118)
  (generator "pytest")
  (generator_version "10.0")
  (future_root "preserve")
  (setup (textsize 1.2 1.3) (linewidth 0.2) (textlinewidth 0.3)
    (left_margin 1) (right_margin 2) (top_margin 3) (bottom_margin 4))
  (line (name "line") (start 1 2 ltcorner) (end 3 4 rbcorner)
    (option page1only) (linewidth 0.4) (repeat 2) (incrx 5) (incry 6) (comment "L"))
  (rect (name "rect") (start 7 8 rtcorner) (end 9 10 lbcorner))
  (polygon (name "poly") (pos 11 12) (rotate 45) (linewidth 0.5)
    (pts (xy 0 0) (xy 1 0) (xy 1 1)) (pts (xy .2 .2)))
  (tbtext "%T" (name "text") (pos 13 14) (rotate 90)
    (font (face "Arial") (linewidth 0.1) (size 1 2) bold italic (color 1 2 3 0.5))
    (justify right bottom) (maxlen 20) (maxheight 3) (incrlabel 1))
  (bitmap (name "logo") (pos 15 16) (scale 2) (data "abc" "def")))"#;

#[test]
fn typed_view_preserves_complete_source_order_and_values() {
    let view = WorksheetView::parse(SYNTHETIC, WorksheetLimits::default()).expect("worksheet");
    assert_eq!(
        view.metadata().expect("metadata").format,
        WorksheetFormat::Modern
    );
    assert_eq!(view.metadata().expect("metadata").version, 20_231_118);
    let setup = view.setup().expect("setup");
    assert_eq!(
        [setup.text_size_x, setup.text_size_y, setup.line_width],
        [1.2, 1.3, 0.2]
    );
    let items = view.items().collect::<Result<Vec<_>, _>>().expect("items");
    assert_eq!(items.len(), 5);
    assert_complete_items(&items);
}

fn assert_complete_items(items: &[WorksheetItem]) {
    let WorksheetItem::Line(line) = &items[0] else {
        panic!("line")
    };
    assert_eq!(line.name, "line");
    assert_eq!(line.start.corner, WorksheetCorner::LeftTop);
    assert_eq!(line.end.corner, WorksheetCorner::RightBottom);
    assert_eq!(line.repeat.count, 2);
    let WorksheetItem::Rect(rect) = &items[1] else {
        panic!("rect")
    };
    assert_eq!(rect.line_width, None);
    let WorksheetItem::Polygon(polygon) = &items[2] else {
        panic!("polygon")
    };
    assert_eq!(polygon.point_sets.len(), 2);
    assert_eq!(polygon.point_sets[0].len(), 3);
    let WorksheetItem::Text(text) = &items[3] else {
        panic!("text")
    };
    assert_eq!(text.text, "%T");
    assert_eq!(text.justify, ["right", "bottom"]);
    assert!(text.font.bold && text.font.italic);
    assert_eq!(text.font.face, "Arial");
    assert_eq!(text.font.color.expect("color").alpha, 0.5);
    let WorksheetItem::Bitmap(bitmap) = &items[4] else {
        panic!("bitmap")
    };
    assert_eq!(bitmap.data_parts, ["abc", "def"]);
}

#[test]
fn legacy_defaults_match_python_semantics() {
    let view = WorksheetView::parse(
        "(page_layout (line (start) (end)) (tbtext) (bitmap (pngdata \"x\")))",
        WorksheetLimits::default(),
    )
    .expect("legacy");
    assert_eq!(
        view.metadata().expect("metadata").format,
        WorksheetFormat::Legacy
    );
    assert_eq!(view.setup().expect("setup"), Default::default());
    let items = view.items().collect::<Result<Vec<_>, _>>().expect("items");
    let WorksheetItem::Text(text) = &items[1] else {
        panic!("text")
    };
    assert_eq!(text.text, "");
    let WorksheetItem::Bitmap(bitmap) = &items[2] else {
        panic!("bitmap")
    };
    assert_eq!(bitmap.scale, 1.0);
    assert_eq!(bitmap.data_parts, ["x"]);
}

#[test]
fn all_durable_worksheets_have_exact_stable_owned_writes_and_equal_models() {
    for path in worksheet_paths() {
        let source = std::fs::read(&path).expect("fixture");
        let limits = WorksheetLimits::default();
        let document = WorksheetDocument::from_reader(Cursor::new(&source), limits)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let first = document
            .view()
            .expect("view")
            .items()
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let metadata = document.view().expect("view").metadata().expect("metadata");
        let setup = document.view().expect("view").setup().expect("setup");
        let mut output = Vec::new();
        document.write_to(&mut output).expect("write");
        assert_eq!(output, source, "{}", path.display());
        let second = WorksheetDocument::from_reader(Cursor::new(&output), limits).expect("reparse");
        assert_eq!(
            second.view().expect("view").metadata().expect("metadata"),
            metadata
        );
        assert_eq!(second.view().expect("view").setup().expect("setup"), setup);
        assert_eq!(
            second
                .view()
                .expect("view")
                .items()
                .collect::<Result<Vec<_>, _>>()
                .expect("second items"),
            first
        );
        let mut stable = Vec::new();
        second.write_to(&mut stable).expect("stable write");
        assert_eq!(stable, source);
    }
}

#[test]
fn item_polygon_bitmap_and_output_limits_fail_closed() {
    assert_eq!(
        WorksheetView::parse(
            SYNTHETIC,
            WorksheetLimits {
                max_items: 4,
                ..WorksheetLimits::default()
            }
        )
        .expect_err("item count")
        .kind,
        ErrorKind::ResourceLimit
    );
    let polygon_view = WorksheetView::parse(
        SYNTHETIC,
        WorksheetLimits {
            max_points_per_polygon: 3,
            ..WorksheetLimits::default()
        },
    )
    .expect("lazy polygon limit");
    assert_eq!(
        polygon_view
            .items()
            .nth(2)
            .expect("polygon")
            .expect_err("point count")
            .kind,
        ErrorKind::ResourceLimit
    );
    let bitmap_view = WorksheetView::parse(
        SYNTHETIC,
        WorksheetLimits {
            max_bitmap_data_bytes: 5,
            ..WorksheetLimits::default()
        },
    )
    .expect("lazy bitmap limit");
    assert_eq!(
        bitmap_view
            .items()
            .nth(4)
            .expect("bitmap")
            .expect_err("bitmap bytes")
            .kind,
        ErrorKind::ResourceLimit
    );
    let document = WorksheetDocument::parse(
        SYNTHETIC.to_owned(),
        WorksheetLimits {
            max_output_bytes: SYNTHETIC.len() - 1,
            ..WorksheetLimits::default()
        },
    )
    .expect("read allowed");
    let mut output = Vec::new();
    assert_eq!(
        document
            .write_to(&mut output)
            .expect_err("output ceiling")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert!(output.is_empty());
}

#[test]
fn top_level_form_and_typed_item_limits_are_independent() {
    let unknowns = (0..17)
        .map(|index| format!("(future_{index} keep)"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("(kicad_wks {unknowns})");
    let exact = WorksheetView::parse(
        &source,
        WorksheetLimits {
            max_top_level_forms: 17,
            max_items: 0,
            ..WorksheetLimits::default()
        },
    )
    .expect("exact unknown-form limit");
    assert_eq!(exact.item_count(), 0);
    assert_eq!(
        WorksheetView::parse(
            &source,
            WorksheetLimits {
                max_top_level_forms: 16,
                max_items: 0,
                ..WorksheetLimits::default()
            },
        )
        .expect_err("one-over top-level forms")
        .kind,
        ErrorKind::ResourceLimit
    );

    let one_item = "(kicad_wks (future keep) (line))";
    assert_eq!(
        WorksheetView::parse(
            one_item,
            WorksheetLimits {
                max_top_level_forms: 2,
                max_items: 0,
                ..WorksheetLimits::default()
            },
        )
        .expect_err("independent item limit")
        .kind,
        ErrorKind::ResourceLimit
    );
    assert!(
        WorksheetView::parse(
            one_item,
            WorksheetLimits {
                max_top_level_forms: 2,
                max_items: 1,
                ..WorksheetLimits::default()
            },
        )
        .is_ok()
    );
}

#[test]
fn bitmap_modern_and_legacy_payload_selection_matches_python() {
    let source = r#"(kicad_wks
      (bitmap (data "a" 2 (nested "ignored") bare "b") (pngdata "unused"))
      (bitmap (pngdata "first" "second" 3)))"#;
    let view = WorksheetView::parse(
        source,
        WorksheetLimits {
            max_bitmap_data_parts: 3,
            max_bitmap_data_bytes: 6,
            ..WorksheetLimits::default()
        },
    )
    .expect("mixed bitmap children");
    let items = view.items().collect::<Result<Vec<_>, _>>().expect("items");
    let WorksheetItem::Bitmap(modern) = &items[0] else {
        panic!("modern bitmap")
    };
    assert_eq!(modern.data_parts, ["a", "bare", "b"]);
    let WorksheetItem::Bitmap(legacy) = &items[1] else {
        panic!("legacy bitmap")
    };
    assert_eq!(legacy.data_parts, ["first"]);

    let limited = WorksheetView::parse(
        source,
        WorksheetLimits {
            max_bitmap_data_parts: 2,
            ..WorksheetLimits::default()
        },
    )
    .expect("lazy bitmap limit");
    assert_eq!(
        limited
            .items()
            .next()
            .expect("modern bitmap")
            .expect_err("three retained strings")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn each_lazy_item_ceiling_has_independent_exact_or_over_evidence() {
    let cases = [
        WorksheetLimits {
            max_nodes_per_item: 1,
            ..WorksheetLimits::default()
        },
        WorksheetLimits {
            max_decoded_string_bytes: 3,
            ..WorksheetLimits::default()
        },
        WorksheetLimits {
            max_point_sets_per_polygon: 1,
            ..WorksheetLimits::default()
        },
        WorksheetLimits {
            max_justify_tokens: 1,
            ..WorksheetLimits::default()
        },
        WorksheetLimits {
            max_bitmap_data_parts: 1,
            ..WorksheetLimits::default()
        },
    ];
    let item_indices = [0, 0, 2, 3, 4];
    for (limits, item_index) in cases.into_iter().zip(item_indices) {
        let view = WorksheetView::parse(SYNTHETIC, limits).expect("lazy ceiling");
        assert_eq!(
            view.items()
                .nth(item_index)
                .expect("selected item")
                .expect_err("over ceiling")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let exact = WorksheetView::parse(
        SYNTHETIC,
        WorksheetLimits {
            max_point_sets_per_polygon: 2,
            max_points_per_polygon: 4,
            max_justify_tokens: 2,
            max_bitmap_data_parts: 2,
            max_bitmap_data_bytes: 6,
            ..WorksheetLimits::default()
        },
    )
    .expect("exact ceilings");
    assert!(exact.items().collect::<Result<Vec<_>, _>>().is_ok());

    let sparse = WorksheetView::parse(
        "(kicad_wks (polygon (pts (xy 0 0)) (pts)))",
        WorksheetLimits {
            max_point_sets_per_polygon: 1,
            max_points_per_polygon: 1,
            ..WorksheetLimits::default()
        },
    )
    .expect("sparse polygon");
    assert!(sparse.items().next().expect("polygon").is_ok());
}

#[test]
fn reader_and_lazy_diagnostics_are_bounded_and_absolute() {
    assert_eq!(
        WorksheetDocument::from_reader(
            Cursor::new(SYNTHETIC.as_bytes()),
            WorksheetLimits {
                max_source_bytes: SYNTHETIC.len() - 1,
                ..WorksheetLimits::default()
            }
        )
        .expect_err("source ceiling")
        .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(
        WorksheetDocument::from_reader(Cursor::new([b'(', 0xff, b')']), WorksheetLimits::default())
            .expect_err("UTF-8")
            .kind,
        ErrorKind::InvalidUtf8
    );
    let source = "(kicad_wks\n  (line (start nope 0) (end 0 0)))";
    let view = WorksheetView::parse(source, WorksheetLimits::default()).expect("lazy view");
    let error = view.items().next().expect("line").expect_err("number");
    assert_eq!(error.kind, ErrorKind::UnexpectedToken);
    assert!(error.position.expect("position").offset >= source.find("(line").unwrap());
}

#[test]
fn writer_io_failure_is_structured() {
    let document = WorksheetDocument::parse(SYNTHETIC.to_owned(), WorksheetLimits::default())
        .expect("document");
    assert_eq!(
        document.write_to(FailingWriter).expect_err("I/O").kind,
        ErrorKind::Io
    );
}

#[test]
fn setup_line_width_edit_updates_inserts_reparses_and_stabilizes() {
    let mut document = WorksheetDocument::parse(SYNTHETIC.to_owned(), WorksheetLimits::default())
        .expect("document");
    assert!(document.set_setup_line_width(0.75).expect("update"));
    assert!(!document.set_setup_line_width(0.75).expect("stable update"));
    assert_eq!(
        document
            .view()
            .expect("view")
            .setup()
            .expect("setup")
            .line_width,
        0.75
    );
    assert!(document.source().contains("(future_root \"preserve\")"));

    let missing = "(page_layout\r\n  (future keep)\r\n)";
    let mut document =
        WorksheetDocument::parse(missing.to_owned(), WorksheetLimits::default()).expect("legacy");
    assert!(document.set_setup_line_width(0.25).expect("insert setup"));
    assert!(
        document
            .source()
            .contains("\r\n  (setup (linewidth 0.25))\r\n")
    );
    assert!(document.source().contains("(future keep)"));
    assert_eq!(
        document
            .view()
            .expect("view")
            .setup()
            .expect("setup")
            .line_width,
        0.25
    );
}

#[test]
fn setup_edit_is_transactional_on_ambiguity_nonfinite_and_output_limits() {
    for source in [
        "(kicad_wks (setup) (setup))",
        "(kicad_wks (setup (linewidth 1) (linewidth 2)))",
    ] {
        let mut document =
            WorksheetDocument::parse(source.to_owned(), WorksheetLimits::default()).expect("view");
        assert_eq!(
            document
                .set_setup_line_width(3.0)
                .expect_err("ambiguous")
                .kind,
            ErrorKind::UnexpectedToken
        );
        assert_eq!(document.source(), source);
    }
    let mut document =
        WorksheetDocument::parse("(kicad_wks (setup))".to_owned(), WorksheetLimits::default())
            .expect("view");
    assert_eq!(
        document
            .set_setup_line_width(f64::NAN)
            .expect_err("finite")
            .kind,
        ErrorKind::UnexpectedToken
    );
    let source = "(kicad_wks (setup))";
    let mut limited = WorksheetDocument::parse(
        source.to_owned(),
        WorksheetLimits {
            max_output_bytes: source.len(),
            ..WorksheetLimits::default()
        },
    )
    .expect("view");
    assert_eq!(
        limited.set_setup_line_width(0.5).expect_err("output").kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(limited.source(), source);
}

fn worksheet_paths() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/L1_parsing/cases/worksheets/input");
    let mut paths = std::fs::read_dir(root)
        .expect("worksheet fixtures")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "kicad_wks")
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 5);
    paths
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

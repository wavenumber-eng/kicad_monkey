use kicad_monkey_core::{
    ErrorKind, SymbolBooleanField, SymbolLibraryLimits, SymbolLibraryView, parse,
};

const SOURCE: &str = r#"# library comment survives
(kicad_symbol_lib
  (version 20231120)
  (generator kicad_symbol_editor)
  (symbol "Base"
    (property "Reference" "U")
    (in_bom yes)
    (on_board no)
    (symbol "Base_1_1"
      (pin input line (at 0 0 0) (length 2.54) (name "A") (number "1")))
    (future_extension "preserve me"))
  (symbol "Derived"
    (extends "Base")
    (power local)
    (symbol "Derived_1_1"
      (pin power_in inverted (at 0 0 90) (length 2.54) (name "VCC") (number "2"))
      (pin power_in line (at 0 0 270) (length 2.54) (name "GND") (number "3"))))
)
"#;

#[test]
fn typed_view_iterates_ordered_symbol_summaries_without_a_generic_tree() {
    let view = SymbolLibraryView::parse(SOURCE, SymbolLibraryLimits::default()).expect("view");
    assert_eq!(view.symbol_count(), 2);
    let symbols = view
        .symbols()
        .collect::<Result<Vec<_>, _>>()
        .expect("summaries");
    assert_eq!(symbols[0].name, "Base");
    assert_eq!(symbols[0].extends, None);
    assert!(symbols[0].in_bom);
    assert!(!symbols[0].on_board);
    assert!(!symbols[0].power);
    assert_eq!(
        (
            symbols[0].property_count,
            symbols[0].subsymbol_count,
            symbols[0].pin_count
        ),
        (1, 1, 1)
    );
    assert_eq!(symbols[1].name, "Derived");
    assert_eq!(symbols[1].extends.as_deref(), Some("Base"));
    assert!(symbols[1].in_bom);
    assert!(symbols[1].on_board);
    assert!(symbols[1].power);
    assert_eq!(symbols[1].power_kind.as_deref(), Some("local"));
    assert_eq!(
        (
            symbols[1].property_count,
            symbols[1].subsymbol_count,
            symbols[1].pin_count
        ),
        (0, 1, 2)
    );
}

#[test]
fn focused_existing_and_inserted_flags_preserve_unknown_source_bytes() {
    let limits = SymbolLibraryLimits::default();
    let view = SymbolLibraryView::parse(SOURCE, limits).expect("view");
    let changed = view
        .set_boolean(
            "Base",
            SymbolBooleanField::OnBoard,
            true,
            limits.max_output_bytes,
        )
        .expect("existing flag edit");
    assert!(changed.changed);
    assert!(changed.source.contains("(on_board yes)"));
    assert!(
        changed
            .source
            .contains("(future_extension \"preserve me\")")
    );
    parse(&changed.source).expect("edited source parses");

    let view = SymbolLibraryView::parse(&changed.source, limits).expect("reparse");
    let inserted = view
        .set_boolean(
            "Derived",
            SymbolBooleanField::InBom,
            false,
            limits.max_output_bytes,
        )
        .expect("insert defaulted flag");
    assert!(inserted.changed);
    assert!(inserted.source.contains("(in_bom no)"));
    parse(&inserted.source).expect("inserted source parses");

    let second = SymbolLibraryView::parse(&inserted.source, limits)
        .expect("second view")
        .set_boolean(
            "Derived",
            SymbolBooleanField::InBom,
            false,
            limits.max_output_bytes,
        )
        .expect("stable second write");
    assert!(!second.changed);
    assert_eq!(second.source, inserted.source);
}

#[test]
fn root_target_and_resource_limits_fail_closed() {
    let extra_root = format!("(metadata)\n{SOURCE}");
    assert_eq!(
        SymbolLibraryView::parse(&extra_root, SymbolLibraryLimits::default())
            .expect_err("one root")
            .kind,
        ErrorKind::UnexpectedToken
    );
    let limits = SymbolLibraryLimits {
        max_pins: 1,
        ..SymbolLibraryLimits::default()
    };
    assert_eq!(
        SymbolLibraryView::parse(SOURCE, limits)
            .expect_err("pin bound")
            .kind,
        ErrorKind::ResourceLimit
    );
    let duplicate = SOURCE.replace("(symbol \"Derived\"", "(symbol \"Base\"");
    let view = SymbolLibraryView::parse(&duplicate, SymbolLibraryLimits::default()).expect("view");
    assert_eq!(
        view.set_boolean("Base", SymbolBooleanField::InBom, false, usize::MAX)
            .expect_err("ambiguous target")
            .kind,
        ErrorKind::UnexpectedToken
    );
    let duplicate_field = SOURCE.replace("(in_bom yes)", "(in_bom yes) (in_bom no)");
    let view =
        SymbolLibraryView::parse(&duplicate_field, SymbolLibraryLimits::default()).expect("view");
    assert_eq!(
        view.set_boolean("Base", SymbolBooleanField::InBom, false, usize::MAX)
            .expect_err("ambiguous field")
            .kind,
        ErrorKind::UnexpectedToken
    );
    let view = SymbolLibraryView::parse(SOURCE, SymbolLibraryLimits::default()).expect("view");
    assert_eq!(
        view.set_boolean("Base", SymbolBooleanField::InBom, false, 1)
            .expect_err("output bound")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn malformed_lazy_metadata_reports_an_absolute_position() {
    let source = "# prefix\n(kicad_symbol_lib\n  (symbol \"D\" (in_bom maybe)))\n";
    let view = SymbolLibraryView::parse(source, SymbolLibraryLimits::default()).expect("view");
    let error = view
        .symbols()
        .next()
        .expect("symbol")
        .expect_err("bad flag");
    let position = error.position.expect("position");
    assert_eq!(position.offset, source.find("(in_bom").expect("offset"));
    assert_eq!(position.line, 3);
}

use kicad_monkey_core::{
    ErrorKind, PlotterFill, PlotterLineStyle, PlotterOperation, SymbolPlotLimits,
    symbol_plot_document,
};

const LIBRARY: &str = r##"(kicad_symbol_lib
  (version 20241209)
  (generator "kicad_symbol_editor")
  (symbol "Other"
    (symbol "Other_1_1"
      (circle (center 0 0) (radius 99) (stroke (width 1) (type solid)) (fill (type none)))))
  (symbol "Demo"
    (in_bom no)
    (on_board yes)
    (power local)
    (symbol "Demo_0_0"
      (rectangle (start -1 2) (end 3 -4)
        (stroke (width 0.2) (type dash) (color 1 2 3 0.5))
        (fill (type background)))
      (circle (center 1 -2) (radius 0.5)
        (stroke (width 0) (type solid)) (fill (type none)))
      (arc (start 0 0) (mid 1 1) (end 2 0)
        (stroke (width -0.1) (type dot)) (fill (type outline)))
      (polyline (pts (xy 0 0) (xy 1 -1) (xy 2 0))
        (stroke (width 0) (type dash_dot)) (fill (type none)))
      (bezier (pts (xy 0 0) (xy 1 2) (xy 3 2) (xy 4 0))
        (stroke (width 0.1) (type dash_dot_dot)) (fill (type none)))
      (text "deferred" (at 0 0 0)))
    (symbol "Demo_2_1"
      (polyline (pts (xy 10 0) (xy 11 0))
        (stroke (width 0.1) (type solid)) (fill (type none))))
    (symbol "Demo_2_2"
      (polyline (pts (xy 20 0) (xy 21 0))
        (stroke (width 0.1) (type solid)) (fill (type none))))))"##;

#[test]
fn selected_symbol_geometry_matches_python_non_text_semantics() {
    let document = symbol_plot_document(LIBRARY, "Demo", Some(2), 0, SymbolPlotLimits::default())
        .expect("selected symbol");
    assert_eq!(document.name, "Demo");
    assert!(!document.in_bom);
    assert!(document.on_board);
    assert!(document.power);
    assert_eq!(document.records.len(), 2);
    assert_eq!(document.records[0].name, "Demo_0_0");
    assert_eq!(document.records[1].name, "Demo_2_1");
    assert_eq!(document.records[0].operations.len(), 6);

    assert_fill_rectangle(&document.records[0].operations[0]);
    assert_circle(&document.records[0].operations[1]);
    assert_bezier(&document.records[0].operations[4]);
    assert_outline_rectangle(&document.records[0].operations[5]);
}

fn assert_fill_rectangle(operation: &PlotterOperation) {
    let PlotterOperation::Rect(fill_pass) = operation else {
        panic!("rectangle fill pass first")
    };
    assert_eq!((fill_pass.x1, fill_pass.y1), (-1_000_000, -2_000_000));
    assert_eq!((fill_pass.x2, fill_pass.y2), (3_000_000, 4_000_000));
    assert_eq!(fill_pass.fill, PlotterFill::FilledWithBackgroundBodyColor);
    assert_eq!(fill_pass.width_nm, 0);
    assert_eq!(fill_pass.stroke_color.as_deref(), Some("#FFFFC2FF"));
    assert_eq!(fill_pass.line_style, Some(PlotterLineStyle::Dash));
}

fn assert_circle(operation: &PlotterOperation) {
    let PlotterOperation::Circle(circle) = operation else {
        panic!("circle follows rectangle")
    };
    assert_eq!(
        (circle.cx, circle.cy, circle.diameter_nm),
        (1_000_000, 2_000_000, 1_000_000)
    );
    assert_eq!(circle.width_nm, 152_400);
}

fn assert_bezier(operation: &PlotterOperation) {
    let PlotterOperation::BezierCurve(bezier) = operation else {
        panic!("cubic bezier is retained")
    };
    assert_eq!((bezier.ctrl1_x, bezier.ctrl1_y), (1_000_000, -2_000_000));
    assert_eq!(bezier.line_style, Some(PlotterLineStyle::DashDotDot));
}

fn assert_outline_rectangle(operation: &PlotterOperation) {
    let PlotterOperation::Rect(outline) = operation else {
        panic!("filled rectangle outline is deferred")
    };
    assert_eq!(outline.fill, PlotterFill::NoFill);
    assert_eq!(outline.width_nm, 200_000);
    assert_eq!(outline.stroke_color.as_deref(), Some("#01020380"));
    assert!(outline.fill_color.is_none());
}

#[test]
fn style_selection_and_resource_limits_fail_closed() {
    let demorgan = symbol_plot_document(LIBRARY, "Demo", Some(2), 2, SymbolPlotLimits::default())
        .expect("alternate style");
    assert_eq!(
        demorgan
            .records
            .iter()
            .map(|record| record.name.as_str())
            .collect::<Vec<_>>(),
        ["Demo_0_0", "Demo_2_2"]
    );

    for limits in [
        SymbolPlotLimits {
            max_symbols: 1,
            ..SymbolPlotLimits::default()
        },
        SymbolPlotLimits {
            max_subsymbols: 1,
            ..SymbolPlotLimits::default()
        },
        SymbolPlotLimits {
            max_operations: 1,
            ..SymbolPlotLimits::default()
        },
        SymbolPlotLimits {
            max_points: 1,
            ..SymbolPlotLimits::default()
        },
    ] {
        assert_eq!(
            symbol_plot_document(LIBRARY, "Demo", Some(2), 0, limits)
                .expect_err("resource limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let empty = "(kicad_symbol_lib (symbol \"Empty\" (symbol \"Empty_1_1\")))";
    let zero = SymbolPlotLimits {
        max_operations: 0,
        max_points: 0,
        ..SymbolPlotLimits::default()
    };
    assert!(symbol_plot_document(empty, "Empty", None, 0, zero).is_ok());
}

#[test]
fn outline_fill_uses_the_explicit_stroke_color() {
    let source = r#"(kicad_symbol_lib (symbol "D" (symbol "D_1_1"
      (circle (center 0 0) (radius 1)
        (stroke (width 0.1) (type solid) (color 7 8 9 1))
        (fill (type outline))))))"#;
    let document = symbol_plot_document(source, "D", None, 0, SymbolPlotLimits::default())
        .expect("outline symbol");
    let PlotterOperation::Circle(circle) = &document.records[0].operations[0] else {
        panic!("circle operation")
    };
    assert_eq!(circle.fill_color.as_deref(), Some("#070809FF"));
}

#[test]
fn root_target_and_duplicate_contracts_are_strict() {
    assert_eq!(
        symbol_plot_document(
            "(metadata)(kicad_symbol_lib)",
            "Demo",
            None,
            0,
            SymbolPlotLimits::default()
        )
        .expect_err("one root")
        .kind,
        ErrorKind::UnexpectedToken
    );
    assert_eq!(
        symbol_plot_document(LIBRARY, "Missing", None, 0, SymbolPlotLimits::default())
            .expect_err("missing symbol")
            .kind,
        ErrorKind::UnexpectedToken
    );
    let duplicate = "(kicad_symbol_lib (symbol \"D\") (symbol \"D\"))";
    assert_eq!(
        symbol_plot_document(duplicate, "D", None, 0, SymbolPlotLimits::default())
            .expect_err("duplicate symbol")
            .kind,
        ErrorKind::UnexpectedToken
    );
}

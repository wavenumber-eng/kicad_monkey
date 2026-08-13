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

#[test]
fn every_pin_graphic_style_matches_the_python_geometry_order() {
    let styles = [
        "line",
        "inverted",
        "clock",
        "inverted_clock",
        "input_low",
        "clock_low",
        "output_low",
        "edge_clock_high",
        "non_logic",
    ];
    let pins = styles
        .iter()
        .enumerate()
        .map(|(index, style)| {
            format!(
                "(pin passive {style} (at 0 {} 0) (length 2.54) (name \"\") (number \"\"))",
                index as f64 * 2.54
            )
        })
        .collect::<String>();
    let source = format!("(kicad_symbol_lib (symbol \"Pins\" (symbol \"Pins_1_1\" {pins})))");
    let document = symbol_plot_document(&source, "Pins", Some(1), 0, SymbolPlotLimits::default())
        .expect("pin styles");
    let operations = &document.records[0].operations;
    assert_eq!(operations.len(), 20);
    assert_circle_at(&operations[1], 1_905_000, -2_540_000);
    assert_circle_at(&operations[5], 1_905_000, -7_620_000);
    let actual_polys = operations
        .iter()
        .filter_map(|operation| match operation {
            PlotterOperation::PlotPoly(poly) => Some(poly.points.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_polys, expected_pin_polys());
}

fn assert_circle_at(operation: &PlotterOperation, cx: i64, cy: i64) {
    let PlotterOperation::Circle(circle) = operation else {
        panic!("pin circle")
    };
    assert_eq!(
        (circle.cx, circle.cy, circle.diameter_nm),
        (cx, cy, 1_270_000)
    );
    assert_eq!(circle.stroke_color.as_deref(), Some("#840000FF"));
}

fn expected_pin_polys() -> Vec<Vec<[i64; 2]>> {
    vec![
        vec![[2_540_000, 0], [0, 0]],
        vec![[1_270_000, -2_540_000], [0, -2_540_000]],
        vec![[2_540_000, -5_080_000], [0, -5_080_000]],
        vec![
            [2_540_000, -4_445_000],
            [3_810_000, -5_080_000],
            [2_540_000, -5_715_000],
        ],
        vec![[1_270_000, -7_620_000], [0, -7_620_000]],
        vec![
            [2_540_000, -6_985_000],
            [3_810_000, -7_620_000],
            [2_540_000, -8_255_000],
        ],
        vec![[2_540_000, -10_160_000], [0, -10_160_000]],
        vec![
            [1_270_000, -10_160_000],
            [1_270_000, -11_430_000],
            [2_540_000, -10_160_000],
        ],
        vec![[2_540_000, -12_700_000], [0, -12_700_000]],
        vec![
            [2_540_000, -12_065_000],
            [3_810_000, -12_700_000],
            [2_540_000, -13_335_000],
        ],
        vec![
            [1_270_000, -12_700_000],
            [1_270_000, -13_970_000],
            [2_540_000, -12_700_000],
        ],
        vec![[2_540_000, -15_240_000], [0, -15_240_000]],
        vec![[2_540_000, -16_510_000], [1_270_000, -15_240_000]],
        vec![
            [2_540_000, -17_145_000],
            [1_270_000, -17_780_000],
            [2_540_000, -18_415_000],
        ],
        vec![[1_270_000, -17_780_000], [0, -17_780_000]],
        vec![[2_540_000, -20_320_000], [0, -20_320_000]],
        vec![[3_175_000, -20_955_000], [1_905_000, -19_685_000]],
        vec![[3_175_000, -19_685_000], [1_905_000, -20_955_000]],
    ]
}

#[test]
fn hidden_and_rotated_pins_preserve_fail_closed_ordering() {
    let source = r#"(kicad_symbol_lib (symbol "D" (symbol "D_1_1"
      (rectangle (start -1 1) (end 1 -1) (stroke (width 0.1) (type solid)) (fill (type background)))
      (pin passive line (at 0 0 90) (length 2.54) (name "") (number ""))
      (pin passive line (at 0 0 0) (length 2.54) (hide yes) (name "") (number "")))))"#;
    let document = symbol_plot_document(source, "D", None, 0, SymbolPlotLimits::default())
        .expect("rotated pins");
    assert_eq!(document.records[0].operations.len(), 3);
    let PlotterOperation::PlotPoly(pin) = &document.records[0].operations[1] else {
        panic!("pin between fill and outline")
    };
    assert_eq!(pin.points, [[0, -2_540_000], [0, 0]]);
    let PlotterOperation::Rect(outline) = &document.records[0].operations[2] else {
        panic!("outline remains deferred after pins")
    };
    assert_eq!(outline.fill, PlotterFill::NoFill);
}

#[test]
fn inheritance_uses_base_geometry_and_requested_symbol_metadata() {
    let source = r#"(kicad_symbol_lib
      (symbol "Base" (in_bom yes) (on_board yes)
        (symbol "Base_1_1"
          (rectangle (start -1 1) (end 1 -1)
            (stroke (width 0.1) (type solid)) (fill (type background)))))
      (symbol "Middle" (extends "Base"))
      (symbol "Child" (extends "Middle") (in_bom no) (power local)))"#;
    let document = symbol_plot_document(source, "Child", Some(1), 0, SymbolPlotLimits::default())
        .expect("inherited plot");
    assert_eq!(document.name, "Child");
    assert_eq!(document.extends.as_deref(), Some("Middle"));
    assert!(!document.in_bom);
    assert!(document.power);
    assert_eq!(document.records.len(), 1);
    assert_eq!(document.records[0].name, "Base_1_1");
    assert_eq!(document.records[0].operations.len(), 2);
}

#[test]
fn missing_and_cyclic_inheritance_match_python_empty_geometry_behavior() {
    let missing = r#"(kicad_symbol_lib (symbol "Child" (extends "Missing")))"#;
    let document = symbol_plot_document(missing, "Child", Some(1), 0, SymbolPlotLimits::default())
        .expect("missing base is non-fatal");
    assert!(document.records.is_empty());

    let cyclic = r#"(kicad_symbol_lib
      (symbol "A" (extends "B"))
      (symbol "B" (extends "A")))"#;
    let document = symbol_plot_document(cyclic, "A", Some(1), 0, SymbolPlotLimits::default())
        .expect("cycle is bounded and non-fatal");
    assert!(document.records.is_empty());
}

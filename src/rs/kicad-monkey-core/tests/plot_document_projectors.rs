use kicad_monkey_contracts::{validate_footprint_plot_document, validate_symbol_plot_document};
use kicad_monkey_core::{
    FlashPadCustom, FootprintPlotDocument, FootprintPlotLimits, PlotDocumentMetadata,
    PlotDocumentProjectionLimits, PlotProjectionErrorKind, PlotterOperation, SymbolPlotLimits,
    SymbolTextVariables, footprint_plot_document, project_footprint_plot_document_a0,
    project_plotter_operation_a0, project_symbol_plot_document_a0,
    symbol_plot_document_with_text_variables,
};

const FOOTPRINT: &str = r#"(footprint "Direct"
  (version 20240108) (generator pcbnew) (generator_version "9.0")
  (layer "F.Cu")
  (fp_line (start 0 0) (end 2 0) (stroke (width 0.2) (type solid)) (layer "F.SilkS"))
  (pad "1" thru_hole circle (at 1 1) (size 1 1) (drill 0.5) (layers "*.Cu" "*.Mask")))"#;

const SYMBOL_LIBRARY: &str = r#"(kicad_symbol_lib (version 20241209)
  (symbol "Direct" (in_bom yes) (on_board yes)
    (symbol "Direct_1_1"
      (rectangle (start -1 1) (end 1 -1)
        (stroke (width 0.2) (type solid)) (fill (type background))))))"#;

#[test]
fn direct_footprint_projector_is_typed_validated_and_bounded() {
    let plot = footprint_plot_document(FOOTPRINT, FootprintPlotLimits::default()).unwrap();
    let metadata = PlotDocumentMetadata {
        document_id: "direct-mod".to_owned(),
        source_path: Some("Direct.kicad_mod".to_owned()),
    };
    let document = project_footprint_plot_document_a0(
        plot.clone(),
        metadata.clone(),
        PlotDocumentProjectionLimits::default(),
    )
    .unwrap();
    validate_footprint_plot_document(&document).unwrap();
    assert_eq!(document.document_id, "direct-mod");
    assert_eq!(document.total_operations as usize, plot.operations.len());

    let under = PlotDocumentProjectionLimits {
        max_operations: plot.operations.len() - 1,
        ..PlotDocumentProjectionLimits::default()
    };
    assert!(
        project_footprint_plot_document_a0(plot, metadata, under)
            .unwrap_err()
            .to_string()
            .contains("operations")
    );
}

#[test]
fn direct_symbol_projector_numbers_operations_across_subsymbols_without_json() {
    let plot = symbol_plot_document_with_text_variables(
        SYMBOL_LIBRARY,
        "Direct",
        Some(1),
        1,
        SymbolPlotLimits::default(),
        &SymbolTextVariables::default(),
    )
    .unwrap();
    let operation_count = plot
        .records
        .iter()
        .map(|record| record.operations.len())
        .sum::<usize>();
    let document = project_symbol_plot_document_a0(
        plot,
        PlotDocumentMetadata {
            document_id: "direct-sym".to_owned(),
            source_path: Some("Direct.kicad_sym".to_owned()),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .unwrap();
    validate_symbol_plot_document(&document).unwrap();
    assert_eq!(document.total_operations as usize, operation_count);
    assert_eq!(document.records.len(), 2);
    let indexes = document
        .records
        .iter()
        .flat_map(|record| match record {
            kicad_monkey_contracts::generated::symbol_plot_document::SymbolPlotRecord::SymbolHeaderPlotRecord(value) => value.operations.iter(),
            kicad_monkey_contracts::generated::symbol_plot_document::SymbolPlotRecord::LibSubsymbolPlotRecord(value) => value.operations.iter(),
        })
        .map(symbol_operation_index)
        .collect::<Vec<_>>();
    assert_eq!(indexes, (0..operation_count as u32).collect::<Vec<_>>());
}

#[test]
fn every_shared_projection_budget_is_exact_and_one_under() {
    let plot = footprint_plot_document(FOOTPRINT, FootprintPlotLimits::default()).unwrap();
    let metadata = PlotDocumentMetadata {
        document_id: "budget-mod".to_owned(),
        source_path: Some("Budget.kicad_mod".to_owned()),
    };
    for (setter, maximum) in [
        (
            set_records as fn(&mut PlotDocumentProjectionLimits, usize),
            PlotDocumentProjectionLimits::default().max_records,
        ),
        (
            set_operations,
            PlotDocumentProjectionLimits::default().max_operations,
        ),
        (
            set_points,
            PlotDocumentProjectionLimits::default().max_points,
        ),
        (
            set_string_bytes,
            PlotDocumentProjectionLimits::default().max_string_bytes,
        ),
        (
            set_nested_items,
            PlotDocumentProjectionLimits::default().max_nested_items,
        ),
        (
            set_materialized_bytes,
            PlotDocumentProjectionLimits::default().max_materialized_bytes,
        ),
    ] {
        let exact = minimum_accepted(&plot, &metadata, setter, maximum);
        let mut limits = PlotDocumentProjectionLimits::default();
        setter(&mut limits, exact);
        project_footprint_plot_document_a0(plot.clone(), metadata.clone(), limits)
            .expect("exact projection budget");
        assert!(exact > 0);
        setter(&mut limits, exact - 1);
        let error =
            project_footprint_plot_document_a0(plot.clone(), metadata.clone(), limits).unwrap_err();
        assert_eq!(error.kind, PlotProjectionErrorKind::ResourceLimit);
    }
}

#[test]
fn operation_index_overflow_fails_instead_of_clamping() {
    let plot = footprint_plot_document(FOOTPRINT, FootprintPlotLimits::default()).unwrap();
    let error = project_plotter_operation_a0(usize::MAX, plot.operations[0].clone()).unwrap_err();
    assert!(error.contains("index exceeds uint32"));
}

#[test]
fn direct_projector_classifies_unsafe_coordinates_as_numeric_range() {
    let mut plot = footprint_plot_document(FOOTPRINT, FootprintPlotLimits::default()).unwrap();
    let PlotterOperation::ThickSegment(segment) = &mut plot.operations[0] else {
        panic!("first fixture operation must be a segment");
    };
    segment.start_x = 9_007_199_254_740_992;
    let error = project_footprint_plot_document_a0(
        plot,
        PlotDocumentMetadata {
            document_id: "unsafe-coordinate".to_owned(),
            source_path: None,
        },
        PlotDocumentProjectionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, PlotProjectionErrorKind::NumericRange);
}

#[test]
fn custom_pad_layers_are_charged_as_nested_items() {
    let mut with_layers =
        footprint_plot_document(FOOTPRINT, FootprintPlotLimits::default()).unwrap();
    with_layers
        .operations
        .push(PlotterOperation::FlashPadCustom(FlashPadCustom {
            x: 0,
            y: 0,
            size_x_nm: 1,
            size_y_nm: 1,
            orient_deg: 0.0,
            polygons: vec![vec![[0, 0], [1, 0], [0, 1]]],
            polygon_widths_nm: Some(vec![0]),
            anchor_shape: Some("circle".to_owned()),
            layers: vec!["F.Cu".to_owned(), "B.Cu".to_owned()],
            mask_margin_nm: 0,
        }));
    let metadata = PlotDocumentMetadata {
        document_id: "custom-layers".to_owned(),
        source_path: None,
    };
    let with_count = minimum_accepted(
        &with_layers,
        &metadata,
        set_nested_items,
        PlotDocumentProjectionLimits::default().max_nested_items,
    );
    let PlotterOperation::FlashPadCustom(custom) = with_layers.operations.last_mut().unwrap()
    else {
        unreachable!();
    };
    custom.layers.pop();
    let without_count = minimum_accepted(
        &with_layers,
        &metadata,
        set_nested_items,
        PlotDocumentProjectionLimits::default().max_nested_items,
    );
    assert_eq!(with_count, without_count + 1);
}

fn minimum_accepted(
    plot: &FootprintPlotDocument,
    metadata: &PlotDocumentMetadata,
    setter: fn(&mut PlotDocumentProjectionLimits, usize),
    maximum: usize,
) -> usize {
    let mut low = 0usize;
    let mut high = maximum;
    while low < high {
        let middle = low + (high - low) / 2;
        let mut limits = PlotDocumentProjectionLimits::default();
        setter(&mut limits, middle);
        if project_footprint_plot_document_a0(plot.clone(), metadata.clone(), limits).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn set_records(limits: &mut PlotDocumentProjectionLimits, value: usize) {
    limits.max_records = value;
}

fn set_operations(limits: &mut PlotDocumentProjectionLimits, value: usize) {
    limits.max_operations = value;
}

fn set_points(limits: &mut PlotDocumentProjectionLimits, value: usize) {
    limits.max_points = value;
}

fn set_string_bytes(limits: &mut PlotDocumentProjectionLimits, value: usize) {
    limits.max_string_bytes = value;
}

fn set_nested_items(limits: &mut PlotDocumentProjectionLimits, value: usize) {
    limits.max_nested_items = value;
}

fn set_materialized_bytes(limits: &mut PlotDocumentProjectionLimits, value: usize) {
    limits.max_materialized_bytes = value;
}

fn symbol_operation_index(
    operation: &kicad_monkey_contracts::generated::symbol_plot_document::PlotterOperation,
) -> u32 {
    use kicad_monkey_contracts::generated::symbol_plot_document::PlotterOperation;
    match operation {
        PlotterOperation::ThickSegmentOperation(value) => value.index,
        PlotterOperation::ArcThreePointOperation(value) => value.index,
        PlotterOperation::CircleOperation(value) => value.index,
        PlotterOperation::RectOperation(value) => value.index,
        PlotterOperation::PlotPolyOperation(value) => value.index,
        PlotterOperation::BezierCurveOperation(value) => value.index,
        PlotterOperation::TextOperation(value) => value.index,
        PlotterOperation::PlotImageOperation(value) => value.index,
        PlotterOperation::FlashPadCircleOperation(value) => value.index,
        PlotterOperation::FlashPadOvalOperation(value) => value.index,
        PlotterOperation::FlashPadRectOperation(value) => value.index,
        PlotterOperation::FlashPadRoundRectOperation(value) => value.index,
        PlotterOperation::FlashPadCustomOperation(value) => value.index,
        PlotterOperation::FlashPadTrapezOperation(value) => value.index,
    }
}

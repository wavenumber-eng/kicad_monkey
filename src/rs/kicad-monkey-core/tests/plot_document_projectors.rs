use kicad_monkey_contracts::{
    validate_footprint_plot_document, validate_schematic_plot_document,
    validate_symbol_plot_document,
};
use kicad_monkey_core::{
    BoardDimensionRecord, BoardNetClassAssignments, BoardPlotLimits, BoardPlotRecord,
    BoardTextVariables, FlashPadCustom, FootprintPlotDocument, FootprintPlotLimits, PcbLimits,
    PlotDocumentMetadata, PlotDocumentProjectionLimits, PlotProjectionErrorKind, PlotterFill,
    PlotterOperation, PlotterPoly, SchematicPlotContext, SchematicPlotLimits,
    SchematicPlotOperation, SchematicPlotRecord, SchematicStyledThickSegment,
    SchematicSymbolInstanceRecord, SchematicSymbolPinAttrs, SchematicSymbolPinBlock,
    SchematicTitleBlock, SymbolPlotLimits, SymbolTextVariables, ThickSegment,
    board_plot_artifact_with_sidecars, board_plot_document, footprint_plot_document,
    project_board_plot_artifact_a0, project_board_plot_document_with_metadata_a0,
    project_footprint_plot_document_a0, project_plotter_operation_a0,
    project_schematic_plot_document_a0, project_symbol_plot_document_a0, schematic_plot_document,
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

const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (general (thickness 1.6)) (paper "A4")
  (layers (0 "F.Cu" signal) (2 "In1.Cu" power) (31 "B.Cu" signal)
    (36 "B.SilkS" user "Back Silk Screen"))
  (segment (start 0 0) (end 1 0) (width 0.2) (layer "F.Cu") (net 0) (uuid "s")))"#;

const SCHEMATIC: &str = r#"(kicad_sch (version 20250114) (generator eeschema)
  (generator_version "9.0") (uuid direct-schematic) (paper "A4")
  (wire (pts (xy 0 0) (xy 2 0))
    (stroke (width 0) (type default)) (uuid wire-1)))"#;

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
fn board_projection_keeps_complete_layer_facts_atomically_bound() {
    let source = board_plot_artifact_with_sidecars(
        BOARD,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .unwrap();
    assert_eq!(
        source.render_facts().enabled_layers(),
        ["F.Cu", "In1.Cu", "B.Cu", "B.SilkS"]
    );
    assert_eq!(
        source.render_facts().copper_stack(),
        ["F.Cu", "In1.Cu", "B.Cu"]
    );
    let projected = project_board_plot_artifact_a0(
        source,
        PlotDocumentMetadata {
            document_id: "board-facts".to_owned(),
            source_path: Some("board.kicad_pcb".to_owned()),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .unwrap();
    assert_eq!(projected.document().document_id, "board-facts");
    assert_eq!(
        projected.render_facts().enabled_layers(),
        ["F.Cu", "In1.Cu", "B.Cu", "B.SilkS"]
    );
}

#[test]
fn typed_board_projection_preserves_resource_numeric_and_model_error_kinds() {
    let metadata = PlotDocumentMetadata {
        document_id: "typed-board-errors".to_owned(),
        source_path: Some("board.kicad_pcb".to_owned()),
    };
    let document = board_plot_document(BOARD, BoardPlotLimits::default()).unwrap();
    let resource = project_board_plot_document_with_metadata_a0(
        document.clone(),
        metadata.clone(),
        PlotDocumentProjectionLimits {
            max_records: 0,
            ..PlotDocumentProjectionLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(resource.kind, PlotProjectionErrorKind::ResourceLimit);

    let mut numeric_document = document.clone();
    numeric_document.version = i64::MAX;
    let numeric = project_board_plot_document_with_metadata_a0(
        numeric_document,
        metadata.clone(),
        PlotDocumentProjectionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(numeric.kind, PlotProjectionErrorKind::NumericRange);

    let mut invalid_document = document;
    invalid_document
        .records
        .push(BoardPlotRecord::Dimension(BoardDimensionRecord {
            uuid: "invalid-dimension".to_owned(),
            layers: vec!["F.Cu".to_owned()],
            dimension_type: "future-kind".to_owned(),
            text: None,
            operations: Vec::new(),
        }));
    let invalid = project_board_plot_document_with_metadata_a0(
        invalid_document,
        metadata,
        PlotDocumentProjectionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(invalid.kind, PlotProjectionErrorKind::InvalidModel);
}

#[test]
fn direct_schematic_projector_is_typed_validated_and_fail_closed() {
    let plot = schematic_plot_document(
        SCHEMATIC,
        SchematicPlotLimits::default(),
        &SchematicPlotContext::default(),
    )
    .unwrap();
    let document =
        project_schematic_plot_document_a0(&plot, PlotDocumentProjectionLimits::default()).unwrap();
    validate_schematic_plot_document(&document).unwrap();
    assert_eq!(document.document_id, "direct-schematic");

    let resource = project_schematic_plot_document_a0(
        &plot,
        PlotDocumentProjectionLimits {
            max_records: plot.records.len() - 1,
            ..PlotDocumentProjectionLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(resource.kind, PlotProjectionErrorKind::ResourceLimit);

    let mut numeric_plot = plot.clone();
    numeric_plot.canvas.width_nm = i64::MAX;
    let numeric =
        project_schematic_plot_document_a0(&numeric_plot, PlotDocumentProjectionLimits::default())
            .unwrap_err();
    assert_eq!(numeric.kind, PlotProjectionErrorKind::NumericRange);

    let mut invalid_plot = plot;
    let SchematicPlotRecord::SheetHeader(header) = &mut invalid_plot.records[0] else {
        panic!("sheet header");
    };
    header.operations.push(SchematicPlotOperation::Plotter(
        PlotterOperation::FlashPadCircle(kicad_monkey_core::FlashPadCircle {
            x: 0,
            y: 0,
            diameter_nm: 1,
            layers: Vec::new(),
            mask_margin_nm: 0,
        }),
    ));
    let invalid =
        project_schematic_plot_document_a0(&invalid_plot, PlotDocumentProjectionLimits::default())
            .unwrap_err();
    assert_eq!(invalid.kind, PlotProjectionErrorKind::InvalidModel);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one rich fixture demonstrates every retained schematic accounting category"
)]
fn every_schematic_projection_budget_is_exact_and_one_under_for_retained_payloads() {
    let mut plot = schematic_plot_document(
        SCHEMATIC,
        SchematicPlotLimits::default(),
        &SchematicPlotContext::default(),
    )
    .unwrap();
    let SchematicPlotRecord::SheetHeader(header) = &mut plot.records[0] else {
        panic!("sheet header");
    };
    header.title_block = Some(SchematicTitleBlock {
        title: "Budget title".to_owned(),
        comments: [(42, "retained comment".to_owned())].into(),
        ..SchematicTitleBlock::default()
    });
    header
        .operations
        .push(SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(
            PlotterPoly {
                points: vec![[0, 0], [1, 0]],
                fill: PlotterFill::NoFill,
                width_nm: 152_400,
                layer: None,
                stroke_color: Some("#840000FF".to_owned()),
                fill_color: None,
                line_style: None,
            },
        )));
    plot.records.push(SchematicPlotRecord::SymbolInstance(
        SchematicSymbolInstanceRecord {
            uuid: "budget-symbol".to_owned(),
            lib_id: "Budget:Symbol".to_owned(),
            lib_name: "Symbol".to_owned(),
            reference: "U1".to_owned(),
            at_x_nm: 0,
            at_y_nm: 0,
            at_angle_deg: 0.0,
            mirror: Some("x".to_owned()),
            unit: 1,
            convert: 1,
            in_bom: true,
            on_board: true,
            dnp: false,
            exclude_from_sim: false,
            in_pos_files: true,
            operations: vec![
                SchematicPlotOperation::StartSymbolPinBlock(SchematicSymbolPinBlock {
                    label: "pin-uuid".to_owned(),
                    data_uuid: "pin-uuid".to_owned(),
                    object_id: "1".to_owned(),
                    extra_attrs: SchematicSymbolPinAttrs {
                        primitive: "pin".to_owned(),
                        object_type: "pin".to_owned(),
                        pin: "1".to_owned(),
                        symbol_uuid: "budget-symbol".to_owned(),
                        designator: String::new(),
                        lib_pin_uuid: "lib-pin".to_owned(),
                    },
                }),
                SchematicPlotOperation::Plotter(PlotterOperation::ThickSegment(ThickSegment {
                    start_x: 0,
                    start_y: 0,
                    end_x: 1,
                    end_y: 0,
                    width_nm: 1,
                    layer: None,
                    role: None,
                    layers: Vec::new(),
                    mask_margin_nm: None,
                    pad_size_x_nm: None,
                    pad_size_y_nm: None,
                })),
                SchematicPlotOperation::EndBlock,
            ],
        },
    ));
    project_schematic_plot_document_a0(&plot, PlotDocumentProjectionLimits::default())
        .expect("rich schematic projection");

    let rich_strings = minimum_schematic_accepted(
        &plot,
        set_string_bytes,
        PlotDocumentProjectionLimits::default().max_string_bytes,
    );
    let rich_nested = minimum_schematic_accepted(
        &plot,
        set_nested_items,
        PlotDocumentProjectionLimits::default().max_nested_items,
    );
    let mut stripped = plot.clone();
    let SchematicPlotRecord::SheetHeader(header) = &mut stripped.records[0] else {
        unreachable!();
    };
    header.title_block.as_mut().unwrap().comments.clear();
    let SchematicPlotRecord::SymbolInstance(symbol) = &mut stripped.records[2] else {
        unreachable!();
    };
    symbol.mirror = None;
    assert_eq!(
        rich_strings,
        minimum_schematic_accepted(
            &stripped,
            set_string_bytes,
            PlotDocumentProjectionLimits::default().max_string_bytes,
        ) + "42".len()
            + "retained comment".len()
            + "x".len()
    );
    assert_eq!(
        rich_nested,
        minimum_schematic_accepted(
            &stripped,
            set_nested_items,
            PlotDocumentProjectionLimits::default().max_nested_items,
        ) + 1
    );

    let mut with_optional_attr = plot.clone();
    let SchematicPlotRecord::SymbolInstance(symbol) = &mut with_optional_attr.records[2] else {
        unreachable!();
    };
    let SchematicPlotOperation::StartSymbolPinBlock(block) = &mut symbol.operations[0] else {
        unreachable!();
    };
    block.extra_attrs.designator = "D".to_owned();
    assert_eq!(
        minimum_schematic_accepted(
            &with_optional_attr,
            set_nested_items,
            PlotDocumentProjectionLimits::default().max_nested_items,
        ),
        rich_nested + 1
    );

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
        let exact = minimum_schematic_accepted(&plot, setter, maximum);
        assert!(exact > 0);
        let mut limits = PlotDocumentProjectionLimits::default();
        setter(&mut limits, exact);
        project_schematic_plot_document_a0(&plot, limits).expect("exact schematic budget");
        setter(&mut limits, exact - 1);
        let error = project_schematic_plot_document_a0(&plot, limits).unwrap_err();
        assert_eq!(error.kind, PlotProjectionErrorKind::ResourceLimit);
    }
}

#[test]
fn styled_schematic_segment_rejects_metadata_the_legacy_wrapper_dropped() {
    let mut plot = schematic_plot_document(
        SCHEMATIC,
        SchematicPlotLimits::default(),
        &SchematicPlotContext::default(),
    )
    .unwrap();
    let SchematicPlotRecord::SheetHeader(header) = &mut plot.records[0] else {
        panic!("sheet header");
    };
    header
        .operations
        .push(SchematicPlotOperation::StyledThickSegment(
            SchematicStyledThickSegment {
                segment: ThickSegment {
                    start_x: 0,
                    start_y: 0,
                    end_x: 1,
                    end_y: 1,
                    width_nm: 1,
                    layer: Some("F.Cu".to_owned()),
                    role: None,
                    layers: Vec::new(),
                    mask_margin_nm: None,
                    pad_size_x_nm: None,
                    pad_size_y_nm: None,
                },
                stroke_color: "#000000FF".to_owned(),
            },
        ));
    let error = project_schematic_plot_document_a0(&plot, PlotDocumentProjectionLimits::default())
        .unwrap_err();
    assert_eq!(error.kind, PlotProjectionErrorKind::InvalidModel);
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

fn minimum_schematic_accepted(
    plot: &kicad_monkey_core::SchematicPlotDocument,
    setter: fn(&mut PlotDocumentProjectionLimits, usize),
    maximum: usize,
) -> usize {
    let mut low = 0usize;
    let mut high = maximum;
    while low < high {
        let middle = low + (high - low) / 2;
        let mut limits = PlotDocumentProjectionLimits::default();
        setter(&mut limits, middle);
        if project_schematic_plot_document_a0(plot, limits).is_ok() {
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

use kicad_monkey_contracts::generated::board_plot_document::BoardPlotDocumentA0;
use kicad_monkey_contracts::validate_board_plot_document;

fn board_table_vector() -> serde_json::Value {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    vectors["vectors"][10]["expected"].clone()
}

#[test]
fn board_table_contract_enforces_identity_and_layers() {
    let tables = board_table_vector();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(tables.clone()).expect("table transport document");
    validate_board_plot_document(&document)
        .expect("table cache angle is intentionally not redundant with operation angle");

    let mut duplicate_layers = tables.clone();
    duplicate_layers["records"][1]["layers"] = serde_json::json!(["B.SilkS", "B.SilkS"]);
    let document: BoardPlotDocumentA0 = serde_json::from_value(duplicate_layers).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("table layers are sorted unique")
            .code,
        "invalid_board_operation"
    );

    for (field, value) in [("kind", "wrong"), ("object_id", "wrong")] {
        let mut wrong_identity = tables.clone();
        wrong_identity["records"][1][field] = serde_json::json!(value);
        let document: BoardPlotDocumentA0 = serde_json::from_value(wrong_identity).expect("shape");
        assert_eq!(
            validate_board_plot_document(&document)
                .expect_err("table identity is canonical")
                .code,
            "invalid_board_operation"
        );
    }

    let mut empty_layers = tables;
    empty_layers["records"][1]["layers"] = serde_json::json!([]);
    let document: BoardPlotDocumentA0 = serde_json::from_value(empty_layers).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("table layers include at least the table layer")
            .code,
        "invalid_board_operation"
    );
}

#[test]
fn board_table_contract_enforces_operation_phases_and_states() {
    let tables = board_table_vector();
    let mut text_before_grid = tables.clone();
    text_before_grid["records"][1]["operations"]
        .as_array_mut()
        .expect("operations")
        .swap(0, 8);
    let document: BoardPlotDocumentA0 = serde_json::from_value(text_before_grid).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("grid segments precede cell text")
            .code,
        "invalid_board_operation"
    );

    let mut undeclared_cell_layer = tables.clone();
    undeclared_cell_layer["records"][1]["operations"][8]["layer"] = serde_json::json!("Cmts.User");
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(undeclared_cell_layer).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("table text layer must be declared")
            .code,
        "invalid_board_operation"
    );

    let mut impossible_segment = tables.clone();
    impossible_segment["records"][1]["operations"][0]["role"] = serde_json::json!("via_drill");
    impossible_segment["records"][1]["operations"][0]["layers"] = serde_json::json!(["F.Cu"]);
    let document: BoardPlotDocumentA0 = serde_json::from_value(impossible_segment).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("table grid segments use only graphic state")
            .code,
        "invalid_board_operation"
    );

    let mut exact_cache = tables.clone();
    exact_cache["records"][1]["operations"][8]["render_cache_exact"] = serde_json::json!(true);
    exact_cache["records"][1]["operations"][8]["render_cache"]["exact"] = serde_json::json!(true);
    let document: BoardPlotDocumentA0 = serde_json::from_value(exact_cache).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("table cache requests omit angle context and remain inexact")
            .code,
        "invalid_board_operation"
    );

    let mut empty_resolved_text = tables.clone();
    let operation = &mut empty_resolved_text["records"][1]["operations"][8];
    operation["text"] = serde_json::json!("");
    operation["render_cache_polygons"] = serde_json::json!([]);
    let operation = operation.as_object_mut().expect("operation");
    operation.remove("render_cache");
    operation.remove("render_cache_source");
    operation.remove("render_cache_exact");
    let document: BoardPlotDocumentA0 = serde_json::from_value(empty_resolved_text).expect("shape");
    validate_board_plot_document(&document)
        .expect("empty resolved table text needs no render-cache polygons");

    let mut wrong_grid_kind = tables;
    wrong_grid_kind["records"][1]["operations"][0] = serde_json::json!({
        "kind": "PlotPoly",
        "index": 0,
        "points": [[0, 0], [1, 0], [1, 1]],
        "fill": "NO_FILL",
        "width_nm": 100000,
        "layer": "Dwgs.User"
    });
    let document: BoardPlotDocumentA0 = serde_json::from_value(wrong_grid_kind).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("table grids contain only segments")
            .code,
        "invalid_board_operation"
    );
}

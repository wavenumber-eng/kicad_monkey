#![recursion_limit = "256"]

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

fn board_dimension_vector() -> serde_json::Value {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    vectors["vectors"][11]["expected"].clone()
}

fn board_footprint_parity_vector() -> serde_json::Value {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    vectors["vectors"][12]["expected"].clone()
}

fn board_footprint_vector() -> serde_json::Value {
    serde_json::json!({
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "PCB",
        "total_operations": 4,
        "records": [{
            "uuid": "fp-1",
            "kind": "footprint",
            "object_id": "Package:Example",
            "operation_count": 4,
            "operations": [
                {
                    "kind": "Rect", "index": 0,
                    "x1": 0, "y1": 0, "x2": 1000000, "y2": 500000,
                    "fill": "NO_FILL", "width_nm": 100000, "corner_radius_nm": 0,
                    "layer": "F.SilkS", "label": "fp-1:fp_rect:0", "data_uuid": "rect-1",
                    "data_ref": "fp_rect", "object_id": "fp_rect",
                    "extra_attrs": {
                        "component": "U1", "component_uid": "fp-1", "component_uuid": "fp-1",
                        "footprint": "Package:Example", "layer_name": "F.SilkS",
                        "layer_role": "silkscreen", "primitive": "footprint-graphic",
                        "footprint_primitive": "fp_rect", "footprint_object_index": 0,
                        "footprint_graphic_kind": "rect"
                    }
                },
                {
                    "kind": "StartBlock", "index": 1, "label": "fp-1:pad:0",
                    "data_uuid": "fp-1:pad:0", "data_ref": "pad", "object_id": "1",
                    "layers": ["F.Cu"],
                    "extra_attrs": {
                        "primitive": "pad", "component": "U1", "component_uid": "fp-1",
                        "component_uuid": "fp-1", "footprint": "Package:Example",
                        "pad_number": "1", "pad_designator": "U1-1", "pad_type": "smd",
                        "pad_shape": "circle", "layer_names": "F.Cu"
                    }
                },
                {
                    "kind": "FlashPadCircle", "index": 2, "x": 0, "y": 0,
                    "diameter_nm": 800000, "layers": ["F.Cu"], "mask_margin_nm": 0
                },
                {"kind": "EndBlock", "index": 3}
            ],
            "library_link": "Package:Example", "reference": "U1", "value": "Example",
            "layer": "F.Cu", "locked": false, "descr": "", "tags": "", "attr": [],
            "placement": {"x_nm": 1000000, "y_nm": 2000000, "angle_deg": 90.0}
        }],
        "document_id": "footprint-board", "coordinate_space": {"unit": "nm", "y_axis": "down"},
        "version": 1, "generator": "test", "generator_version": "1", "thickness_mm": 1.6,
        "paper": "A4"
    })
}

fn board_footprint_text_vector() -> serde_json::Value {
    let mut value = board_footprint_vector();
    value["records"][0]["operations"][0] = serde_json::json!({
        "kind": "Text", "index": 0, "x": 0, "y": 0, "text": "U1", "color": "#000000",
        "orient_deg": 0.0, "size_x_nm": 1000000, "size_y_nm": 1000000,
        "h_align": "GR_TEXT_H_ALIGN_CENTER", "v_align": "GR_TEXT_V_ALIGN_CENTER", "pen_width_nm": 100000,
        "italic": false, "bold": false, "multiline": false, "font_face": "",
        "layer": "F.SilkS",
        "label": "fp-1:fp_text:0", "data_uuid": "text-1", "data_ref": "fp_text",
        "object_id": "fp_text", "extra_attrs": {
            "component": "U1", "component_uid": "fp-1", "component_uuid": "fp-1",
            "footprint": "Package:Example", "layer_name": "F.SilkS", "layer_role": "silkscreen",
            "primitive": "footprint-text", "footprint_primitive": "fp_text",
            "footprint_object_index": 0, "footprint_text_role": "user", "fp_text_type": "user"
        },
        "render_cache_source": "python_generated_cache", "render_cache_exact": false,
        "render_cache_polygons": [[[0, 0], [1000, 0], [0, 1000]]],
        "render_cache": {
            "schema": "kicad.render_cache.v1", "unit": "nm", "coordinate_space": "footprint_local",
            "text": "U1", "angle": 0.0, "source": "python_generated_cache", "exact": false,
            "polygons": [{"contours": [[[0, 0], [1000, 0], [0, 1000]]]}]
        }
    });
    value
}

#[test]
fn board_dimension_contract_enforces_identity_layers_indices_and_counts() {
    let dimensions = board_dimension_vector();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(dimensions.clone()).expect("dimension transport document");
    validate_board_plot_document(&document).expect("canonical dimension document");

    for layers in [
        serde_json::json!([]),
        serde_json::json!(["Dwgs.User", "Dwgs.User"]),
        serde_json::json!(["F.SilkS", "Dwgs.User"]),
    ] {
        let mut mutation = dimensions.clone();
        mutation["records"][1]["layers"] = layers;
        let document: BoardPlotDocumentA0 = serde_json::from_value(mutation).expect("shape");
        assert!(validate_board_plot_document(&document).is_err());
    }

    let mut wrong_index = dimensions.clone();
    wrong_index["records"][1]["operations"][0]["index"] = serde_json::json!(1);
    let document: BoardPlotDocumentA0 = serde_json::from_value(wrong_index).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());

    let mut wrong_count = dimensions.clone();
    wrong_count["records"][1]["operation_count"] = serde_json::json!(11);
    let document: BoardPlotDocumentA0 = serde_json::from_value(wrong_count).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());

    let mut undeclared_layer = dimensions;
    undeclared_layer["records"][1]["operations"][0]["layer"] = serde_json::json!("B.Cu");
    let document: BoardPlotDocumentA0 = serde_json::from_value(undeclared_layer).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());
}

#[test]
fn board_dimension_contract_rejects_wrong_kinds_duplicate_markers_and_null_text() {
    let dimensions = board_dimension_vector();

    let mut wrong_kind = dimensions.clone();
    wrong_kind["records"][1]["operations"][0]["kind"] = serde_json::json!("Circle");
    assert!(serde_json::from_value::<BoardPlotDocumentA0>(wrong_kind).is_err());

    let mut duplicate_marker = dimensions.clone();
    let marker = duplicate_marker["records"][2]["operations"][1].clone();
    let operations = duplicate_marker["records"][2]["operations"]
        .as_array_mut()
        .expect("operations");
    operations.insert(2, marker);
    for (index, operation) in operations.iter_mut().enumerate() {
        operation["index"] = serde_json::json!(index);
    }
    duplicate_marker["records"][2]["operation_count"] = serde_json::json!(10);
    duplicate_marker["total_operations"] = serde_json::json!(
        duplicate_marker["total_operations"]
            .as_u64()
            .expect("total")
            + 1
    );
    let document: BoardPlotDocumentA0 = serde_json::from_value(duplicate_marker).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());

    let mut null_text = dimensions.clone();
    null_text["records"][2]["text"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BoardPlotDocumentA0>(null_text).is_err());

    let mut empty_text = dimensions;
    empty_text["records"][2]["text"] = serde_json::json!("");
    let document: BoardPlotDocumentA0 = serde_json::from_value(empty_text).expect("empty text");
    validate_board_plot_document(&document).expect("empty dimension record text is explicit");
}

#[test]
fn board_dimension_contract_rejects_noncanonical_faced_text_state() {
    let dimensions = board_dimension_vector();
    for (field, value) in [
        ("mirror", serde_json::json!(false)),
        ("text_as_polygons", serde_json::json!(true)),
        ("polyline_per_segment", serde_json::json!(false)),
        ("knockout", serde_json::json!(false)),
    ] {
        let mut mutation = dimensions.clone();
        mutation["records"][3]["operations"][0][field] = value;
        let document: BoardPlotDocumentA0 = serde_json::from_value(mutation).expect("shape");
        assert!(validate_board_plot_document(&document).is_err(), "{field}");
    }

    let mut missing_layer = dimensions.clone();
    missing_layer["records"][3]["operations"][0]
        .as_object_mut()
        .expect("Text operation")
        .remove("layer");
    let document: BoardPlotDocumentA0 = serde_json::from_value(missing_layer).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());

    let mut mismatched_cache = dimensions;
    mismatched_cache["records"][3]["operations"][0]["render_cache_source"] =
        serde_json::json!("native_generated_cache");
    let document: BoardPlotDocumentA0 = serde_json::from_value(mismatched_cache).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());
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

#[test]
fn board_text_cache_contract_accepts_native_provenance_only_when_keys_agree() {
    let mut native = board_table_vector();
    let operation = &mut native["records"][1]["operations"][8];
    operation["render_cache_source"] = serde_json::json!("native_generated_cache");
    operation["render_cache"]["source"] = serde_json::json!("native_generated_cache");
    let document: BoardPlotDocumentA0 = serde_json::from_value(native.clone()).expect("shape");
    validate_board_plot_document(&document).expect("native cache provenance");

    let mut python = native.clone();
    python["records"][1]["operations"][8]["render_cache_source"] =
        serde_json::json!("python_generated_cache");
    python["records"][1]["operations"][8]["render_cache"]["source"] =
        serde_json::json!("python_generated_cache");
    let document: BoardPlotDocumentA0 = serde_json::from_value(python).expect("shape");
    validate_board_plot_document(&document).expect("Python cache provenance");

    native["records"][1]["operations"][8]["render_cache_source"] =
        serde_json::json!("existing_file_cache");
    let document: BoardPlotDocumentA0 = serde_json::from_value(native).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("cache provenance keys must agree")
            .code,
        "invalid_board_operation"
    );
}

#[test]
fn board_footprint_contract_accepts_canonical_children_and_pad_blocks() {
    let value = board_footprint_vector();
    let document: BoardPlotDocumentA0 = serde_json::from_value(value).expect("shape");
    validate_board_plot_document(&document).expect("canonical embedded footprint");
}

#[test]
fn board_footprint_contract_accepts_the_shared_python_parity_vector() {
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(board_footprint_parity_vector()).expect("shape");
    validate_board_plot_document(&document).expect("shared embedded-footprint vector");
}

#[test]
fn board_footprint_contract_ratchets_all_fifteen_exact_kinds() {
    let value = board_footprint_vector();
    for (operation_index, replacement) in [
        (0, "Circle"),
        (1, "EndBlock"),
        (2, "Rect"),
        (3, "StartBlock"),
    ] {
        let mut mutation = value.clone();
        mutation["records"][0]["operations"][operation_index]["kind"] =
            serde_json::json!(replacement);
        assert!(
            serde_json::from_value::<BoardPlotDocumentA0>(mutation).is_err(),
            "operation {operation_index} accepts a mismatched discriminator"
        );
    }
}

#[test]
fn board_footprint_contract_rejects_partial_or_inconsistent_child_metadata() {
    let value = board_footprint_vector();
    for mutation in [
        ("remove_label", serde_json::Value::Null),
        ("wrong_parent", serde_json::json!("other")),
        ("wrong_ref", serde_json::json!("fp_line")),
        ("wrong_graphic", serde_json::json!("line")),
    ] {
        let mut candidate = value.clone();
        match mutation.0 {
            "remove_label" => {
                candidate["records"][0]["operations"][0]
                    .as_object_mut()
                    .expect("operation")
                    .remove("label");
            }
            "wrong_parent" => {
                candidate["records"][0]["operations"][0]["extra_attrs"]["component_uuid"] =
                    mutation.1;
            }
            "wrong_ref" => {
                candidate["records"][0]["operations"][0]["data_ref"] = mutation.1;
            }
            "wrong_graphic" => {
                candidate["records"][0]["operations"][0]["extra_attrs"]["footprint_graphic_kind"] =
                    mutation.1;
            }
            _ => unreachable!(),
        }
        let document: BoardPlotDocumentA0 = serde_json::from_value(candidate).expect("shape");
        assert!(
            validate_board_plot_document(&document).is_err(),
            "{}",
            mutation.0
        );
    }
}

#[test]
fn board_footprint_contract_rejects_malformed_blocks_and_nonterminal_records() {
    let value = board_footprint_vector();

    let mut leaked_metadata = value.clone();
    leaked_metadata["records"][0]["operations"][2]["label"] = serde_json::json!("leak");
    let document: BoardPlotDocumentA0 = serde_json::from_value(leaked_metadata).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());

    let mut missing_end = value.clone();
    missing_end["records"][0]["operations"]
        .as_array_mut()
        .expect("operations")
        .pop();
    missing_end["records"][0]["operation_count"] = serde_json::json!(3);
    missing_end["total_operations"] = serde_json::json!(3);
    let document: BoardPlotDocumentA0 = serde_json::from_value(missing_end).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());

    let mut nonterminal = value;
    let trailing = board_table_vector()["records"][0].clone();
    nonterminal["records"]
        .as_array_mut()
        .expect("records")
        .push(trailing);
    let document: BoardPlotDocumentA0 = serde_json::from_value(nonterminal).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("footprints are terminal")
            .code,
        "invalid_board_record_order"
    );
}

#[test]
fn board_footprint_contract_requires_footprint_local_cache() {
    let value = board_footprint_text_vector();
    let document: BoardPlotDocumentA0 = serde_json::from_value(value.clone()).expect("shape");
    validate_board_plot_document(&document).expect("footprint-local text cache");

    let mut board_space = value;
    board_space["records"][0]["operations"][0]["render_cache"]["coordinate_space"] =
        serde_json::json!("board");
    let document: BoardPlotDocumentA0 = serde_json::from_value(board_space).expect("shape");
    assert!(validate_board_plot_document(&document).is_err());
}

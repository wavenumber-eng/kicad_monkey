use kicad_monkey_contracts::generated::board_plot_document::BoardPlotDocumentA0;
use kicad_monkey_contracts::generated::footprint_plot_document::FootprintPlotDocumentA0;
use kicad_monkey_contracts::generated::schematic_plot_document::SchematicPlotDocumentA0;
use kicad_monkey_contracts::generated::schematic_plot_request::SchematicPlotRequestA0;
use kicad_monkey_contracts::generated::symbol_plot_document::SymbolPlotDocumentA0;
use kicad_monkey_contracts::{
    validate_board_plot_document, validate_footprint_plot_document,
    validate_schematic_plot_document, validate_symbol_plot_document,
};

const ONE_BY_ONE_PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn schematic_document() -> serde_json::Value {
    serde_json::json!({
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "SCH",
        "total_operations": 8,
        "records": [
            {
                "uuid": "root", "kind": "sheet_header", "object_id": "root",
                "operation_count": 2,
                "operations": [
                    {
                        "kind": "Rect", "index": 0,
                        "x1": 0, "y1": 0, "x2": 10000000, "y2": 5000000,
                        "fill": "FILLED_SHAPE", "width_nm": 100,
                        "corner_radius_nm": 0, "stroke_color": "#F5F4EFFF",
                        "fill_color": "#F5F4EFFF"
                    },
                    {
                        "kind": "PlotImage", "index": 1,
                        "x": 1000000, "y": 1000000,
                        "width_nm": 84667, "height_nm": 84667, "scale": 1.0,
                        "image_data_b64": ONE_BY_ONE_PNG, "image_format": "png",
                        "stroke_color": "#840000FF"
                    }
                ],
                "paper_size": "A4", "paper_width_mm": null,
                "paper_height_mm": null, "paper_portrait": false,
                "sheet_width_nm": 10000000, "sheet_height_nm": 5000000,
                "version": 20260101, "generator": "eeschema",
                "generator_version": "10.0",
                "title_block": {
                    "title": "Demo", "date": "", "rev": "A",
                    "company": "Wavenumber", "comments": {"1": "strict"}
                }
            },
            {
                "uuid": "wire", "kind": "wire", "object_id": "wire",
                "operation_count": 1, "operations": [{
                    "kind": "PlotPoly", "index": 0,
                    "points": [[0, 0], [1000000, 0]], "fill": "NO_FILL",
                    "width_nm": 152400, "stroke_color": "#009600FF",
                    "line_style": "DEFAULT"
                }]
            },
            {
                "uuid": "bus", "kind": "bus", "object_id": "bus",
                "operation_count": 1, "operations": [{
                    "kind": "PlotPoly", "index": 0,
                    "points": [[0, 1000000], [1000000, 1000000]],
                    "fill": "NO_FILL", "width_nm": 304800,
                    "stroke_color": "#000084FF", "line_style": "SOLID"
                }]
            },
            {
                "uuid": "entry", "kind": "bus_entry", "object_id": "entry",
                "operation_count": 1, "operations": [{
                    "kind": "PlotPoly", "index": 0,
                    "points": [[1000000, 1000000], [2000000, 2000000]],
                    "fill": "NO_FILL", "width_nm": 152400,
                    "stroke_color": "#009600FF", "line_style": "DEFAULT"
                }]
            },
            {
                "uuid": "junction", "kind": "junction", "object_id": "junction",
                "operation_count": 1, "operations": [{
                    "kind": "Circle", "index": 0, "cx": 2000000, "cy": 2000000,
                    "diameter_nm": 914400, "fill": "FILLED_SHAPE", "width_nm": 0,
                    "stroke_color": "#11223344", "fill_color": "#11223344"
                }], "color": "#11223344"
            },
            {
                "uuid": "nc", "kind": "no_connect", "object_id": "nc",
                "operation_count": 2, "operations": [
                    {
                        "kind": "PlotPoly", "index": 0,
                        "points": [[-609600, -609600], [609600, 609600]],
                        "fill": "NO_FILL", "width_nm": 152400,
                        "stroke_color": "#000084FF"
                    },
                    {
                        "kind": "PlotPoly", "index": 1,
                        "points": [[-609600, 609600], [609600, -609600]],
                        "fill": "NO_FILL", "width_nm": 152400,
                        "stroke_color": "#000084FF"
                    }
                ]
            }
        ],
        "source_path": "demo.kicad_sch", "document_id": "root",
        "canvas": {"width_nm": 10000000, "height_nm": 5000000},
        "coordinate_space": {"unit": "nm", "y_axis": "down"}
    })
}

fn decode(value: &serde_json::Value) -> SchematicPlotDocumentA0 {
    serde_json::from_value(value.clone()).expect("schematic contract structure")
}

#[test]
fn schematic_foundation_accepts_strict_canonical_record_phases() {
    let document = decode(&schematic_document());
    validate_schematic_plot_document(&document).expect("valid schematic foundation");
    assert_eq!(
        serde_json::to_value(&document).expect("serialize"),
        schematic_document()
    );
}

#[test]
fn schematic_record_discriminators_are_exact_during_rust_decode() {
    let mut value = schematic_document();
    value["records"][1]["kind"] = serde_json::json!("bus");
    let decoded: SchematicPlotDocumentA0 = serde_json::from_value(value).expect("bus variant");
    assert!(matches!(
        decoded.records[1],
        kicad_monkey_contracts::generated::schematic_plot_document::SchematicPlotRecord::BusPlotRecord(_)
    ));

    let mut unknown = schematic_document();
    unknown["records"][1]["kind"] = serde_json::json!("unknown");
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(unknown).is_err());
}

#[test]
fn schematic_semantic_mutations_fail_closed() {
    let mut mutations = Vec::new();

    let mut reversed = schematic_document();
    reversed["records"]
        .as_array_mut()
        .expect("records")
        .swap(1, 2);
    mutations.push(reversed);

    let mut wrong_total = schematic_document();
    wrong_total["total_operations"] = serde_json::json!(7);
    mutations.push(wrong_total);

    let mut wrong_count = schematic_document();
    wrong_count["records"][1]["operation_count"] = serde_json::json!(0);
    mutations.push(wrong_count);

    let mut wrong_index = schematic_document();
    wrong_index["records"][5]["operations"][1]["index"] = serde_json::json!(0);
    mutations.push(wrong_index);

    let mut wrong_canvas = schematic_document();
    wrong_canvas["canvas"]["width_nm"] = serde_json::json!(999);
    mutations.push(wrong_canvas);

    let mut wrong_background = schematic_document();
    wrong_background["records"][0]["operations"][0]["fill_color"] = serde_json::json!("#FFFFFFFF");
    mutations.push(wrong_background);

    let mut invalid_image = schematic_document();
    invalid_image["records"][0]["operations"][1]["scale"] = serde_json::json!(0.0);
    mutations.push(invalid_image);

    let mut invalid_image_lexical = schematic_document();
    invalid_image_lexical["records"][0]["operations"][1]["image_data_b64"] =
        serde_json::json!(format!("{ONE_BY_ONE_PNG}===="));
    mutations.push(invalid_image_lexical);

    let mut invalid_image_whitespace = schematic_document();
    invalid_image_whitespace["records"][0]["operations"][1]["image_data_b64"] =
        serde_json::json!(ONE_BY_ONE_PNG.replacen("KGgo", "KG\ngo", 1));
    mutations.push(invalid_image_whitespace);

    let mut truncated_image_header = schematic_document();
    truncated_image_header["records"][0]["operations"][1]["image_data_b64"] =
        serde_json::json!("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAA=");
    mutations.push(truncated_image_header);

    let mut invalid_image_signature = schematic_document();
    invalid_image_signature["records"][0]["operations"][1]["image_data_b64"] =
        serde_json::json!("AAAA");
    mutations.push(invalid_image_signature);

    let mut invalid_image_chunk = schematic_document();
    invalid_image_chunk["records"][0]["operations"][1]["image_data_b64"] =
        serde_json::json!("iVBORw0KGgoAAAANSkhEUgAAAAEAAAABCAYAAAA=");
    mutations.push(invalid_image_chunk);

    let mut invalid_image_dimensions = schematic_document();
    invalid_image_dimensions["records"][0]["operations"][1]["image_data_b64"] =
        serde_json::json!("iVBORw0KGgoAAAANSUhEUgAAAAAAAAABCAYAAAA=");
    mutations.push(invalid_image_dimensions);

    let mut invalid_image_style = schematic_document();
    invalid_image_style["records"][0]["operations"][1]["stroke_color"] =
        serde_json::json!("#FFFFFFFF");
    mutations.push(invalid_image_style);

    let mut invalid_worksheet_rect = schematic_document();
    invalid_worksheet_rect["records"][0]["operations"][1] = serde_json::json!({
        "kind": "Rect", "index": 1,
        "x1": 0, "y1": 0, "x2": 1000, "y2": 1000,
        "fill": "FILLED_SHAPE", "width_nm": 152400,
        "corner_radius_nm": 0, "stroke_color": "#840000FF"
    });
    mutations.push(invalid_worksheet_rect);

    let mut invalid_worksheet_polyline = schematic_document();
    invalid_worksheet_polyline["records"][0]["operations"][1] = serde_json::json!({
        "kind": "PlotPoly", "index": 1, "points": [[0, 0], [1, 1], [2, 2]],
        "fill": "NO_FILL", "width_nm": 152400, "stroke_color": "#840000FF"
    });
    mutations.push(invalid_worksheet_polyline);

    let mut image_wire = schematic_document();
    image_wire["records"][1]["operations"][0] = image_wire["records"][0]["operations"][1].clone();
    mutations.push(image_wire);

    let mut drill_junction = schematic_document();
    drill_junction["records"][4]["operations"][0]["role"] = serde_json::json!("pad_drill");
    drill_junction["records"][4]["operations"][0]["layers"] = serde_json::json!(["F.Cu"]);
    mutations.push(drill_junction);

    let mut broken_cross = schematic_document();
    broken_cross["records"][5]["operations"][1]["points"][0][0] = serde_json::json!(0);
    mutations.push(broken_cross);

    for mutation in mutations {
        let document = decode(&mutation);
        assert!(validate_schematic_plot_document(&document).is_err());
    }
}

#[test]
fn schematic_junction_color_preserves_absent_null_and_authored_states() {
    let mut transparent = schematic_document();
    transparent["records"][4]["color"] = serde_json::Value::Null;
    transparent["records"][4]["operations"][0]["stroke_color"] = serde_json::json!("#009600FF");
    transparent["records"][4]["operations"][0]["fill_color"] = serde_json::json!("#009600FF");
    let transparent_document = decode(&transparent);
    validate_schematic_plot_document(&transparent_document).expect("transparent default junction");
    assert_eq!(
        serde_json::to_value(&transparent_document).expect("serialize transparent junction"),
        transparent
    );

    let mut absent = transparent.clone();
    absent["records"][4]
        .as_object_mut()
        .expect("junction record")
        .remove("color");
    let absent_document = decode(&absent);
    validate_schematic_plot_document(&absent_document).expect("unauthored default junction");
    assert_eq!(
        serde_json::to_value(&absent_document).expect("serialize absent junction color"),
        absent
    );

    let mut null_with_custom_circle = schematic_document();
    null_with_custom_circle["records"][4]["color"] = serde_json::Value::Null;
    assert!(validate_schematic_plot_document(&decode(&null_with_custom_circle)).is_err());

    let mut authored_mismatch = schematic_document();
    authored_mismatch["records"][4]["color"] = serde_json::json!("#01020304");
    assert!(validate_schematic_plot_document(&decode(&authored_mismatch)).is_err());
}

#[test]
fn shared_schematic_vectors_decode_validate_and_round_trip_exactly() {
    let vectors_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/parity/schematic_plotter_a0_vectors.json");
    let vectors: serde_json::Value = serde_json::from_slice(
        &std::fs::read(vectors_path).expect("read shared schematic vectors"),
    )
    .expect("decode shared schematic vectors");
    for vector in vectors["vectors"].as_array().expect("vector list") {
        let expected = &vector["expected"];
        let document = decode(expected);
        validate_schematic_plot_document(&document).expect("valid shared schematic vector");
        assert_eq!(
            serde_json::to_value(document).expect("serialize shared schematic vector"),
            *expected
        );
    }
}

#[test]
fn schematic_request_requires_every_independent_budget() {
    let request = serde_json::json!({
        "type": "kicad_monkey.schematic_plot.request", "version": "a0",
        "sheet_index": 1, "sheet_count": 1, "sheet_path": "/", "sheet_name": "",
        "worksheet_mode": "default", "max_source_bytes": "4096",
        "max_worksheet_bytes": "4096", "max_output_bytes": "65536",
        "max_depth": 64, "max_parse_nodes": 1000, "max_selected_forms": 1000,
        "max_records": 100, "max_operations": 1000, "max_points": 10000,
        "max_input_points": 10000, "max_text_bytes": "65536",
        "max_metadata_bytes": "65536", "max_wires": 100, "max_buses": 100,
        "max_bus_entries": 100, "max_junctions": 100, "max_no_connects": 100,
        "max_text_variables": 100, "max_text_variable_bytes": "4096",
        "max_worksheet_items": 100, "max_worksheet_repeats": 1000,
        "max_worksheet_point_sets": 100, "max_worksheet_points": 1000,
        "max_worksheet_bitmap_data_parts": 100,
        "max_worksheet_bitmap_encoded_bytes": "4096",
        "max_worksheet_bitmap_decoded_bytes": "4096",
        "max_worksheet_bitmap_width_px": 1000,
        "max_worksheet_bitmap_height_px": 1000,
        "max_worksheet_bitmap_pixels": "1000000",
        "max_worksheet_bitmap_decode_work": "4096"
    });
    serde_json::from_value::<SchematicPlotRequestA0>(request.clone()).expect("complete request");
    for field in ["sheet_index", "sheet_count"] {
        let mut zero = request.clone();
        zero[field] = serde_json::json!(0);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(zero).is_err());

        let mut over_u32 = request.clone();
        over_u32[field] = serde_json::json!(4_294_967_296_u64);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(over_u32).is_err());
    }
    for field in [
        "max_source_bytes",
        "max_worksheet_bytes",
        "max_records",
        "max_input_points",
        "max_worksheet_repeats",
        "max_worksheet_bitmap_decode_work",
    ] {
        let mut missing = request.clone();
        missing.as_object_mut().expect("request").remove(field);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(missing).is_err());
    }
}

#[test]
fn plot_image_remains_rejected_by_existing_producer_validators() {
    let image = serde_json::json!({
        "kind": "PlotImage", "index": 0, "x": 0, "y": 0,
        "width_nm": 0, "height_nm": 0, "scale": 1.0,
        "image_data_b64": "", "image_format": "png"
    });

    let footprint: FootprintPlotDocumentA0 = serde_json::from_value(serde_json::json!({
        "schema": "kicad.plotter_ir.a0", "source_kind": "MOD", "total_operations": 1,
        "records": [{"uuid": "", "kind": "footprint", "object_id": "demo",
            "operation_count": 1, "operations": [image.clone()], "name": "demo",
            "layer": "F.Cu", "locked": false, "placed": false, "descr": "",
            "tags": "", "attr": []}], "document_id": "demo",
        "coordinate_space": {"unit": "nm", "y_axis": "down"},
        "version": 1, "generator": "test", "generator_version": "1"
    }))
    .expect("footprint image structure");
    assert!(validate_footprint_plot_document(&footprint).is_err());

    let symbol: SymbolPlotDocumentA0 = serde_json::from_value(serde_json::json!({
        "schema": "kicad.plotter_ir.a0", "source_kind": "SYM", "total_operations": 1,
        "records": [
            {"uuid": "", "kind": "lib_symbol", "object_id": "demo",
             "operation_count": 0, "operations": [], "name": "demo", "style": 0,
             "in_bom": true, "on_board": true, "power": false},
            {"uuid": "", "kind": "lib_subsymbol", "object_id": "demo_0_0",
             "operation_count": 1, "operations": [image.clone()], "unit": 0, "style": 0}
        ], "document_id": "demo", "coordinate_space": {"unit": "nm", "y_axis": "down"}
    }))
    .expect("symbol image structure");
    assert!(validate_symbol_plot_document(&symbol).is_err());

    let board: BoardPlotDocumentA0 = serde_json::from_value(serde_json::json!({
        "schema": "kicad.plotter_ir.a0", "source_kind": "PCB", "total_operations": 1,
        "records": [{"uuid": "graphic", "kind": "gr_line", "object_id": "graphic",
            "operation_count": 1, "operations": [image], "layer": "F.SilkS"}],
        "document_id": "demo", "coordinate_space": {"unit": "nm", "y_axis": "down"},
        "version": 1, "generator": "test", "generator_version": "1",
        "thickness_mm": 1.6, "paper": "A4"
    }))
    .expect("board image structure");
    assert!(validate_board_plot_document(&board).is_err());
}

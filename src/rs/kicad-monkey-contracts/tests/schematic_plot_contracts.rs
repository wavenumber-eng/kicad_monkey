#![recursion_limit = "256"]

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

fn shared_vector(file: &str, id: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/parity")
        .join(file);
    let payload: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("read shared vectors"))
            .expect("decode shared vectors");
    payload["vectors"]
        .as_array()
        .expect("vector list")
        .iter()
        .find(|vector| vector["id"] == id)
        .expect("named shared vector")["expected"]
        .clone()
}

fn insert_first_operation_field(
    value: &mut serde_json::Value,
    kind: &str,
    field: &str,
    replacement: serde_json::Value,
) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("kind").and_then(serde_json::Value::as_str) == Some(kind) {
                object.insert(field.to_owned(), replacement);
                true
            } else {
                object.values_mut().any(|child| {
                    insert_first_operation_field(child, kind, field, replacement.clone())
                })
            }
        }
        serde_json::Value::Array(values) => values
            .iter_mut()
            .any(|child| insert_first_operation_field(child, kind, field, replacement.clone())),
        _ => false,
    }
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

fn sheet_with_undecorated_pin(canonical: &serde_json::Value, shape: &str) -> serde_json::Value {
    let mut document = canonical.clone();
    document["records"][5]["operations"][2]["extra_attrs"]["shape"] = serde_json::json!(shape);
    document["records"][5]["operations"]
        .as_array_mut()
        .expect("operations")
        .remove(4);
    for (index, operation) in document["records"][5]["operations"]
        .as_array_mut()
        .expect("operations")
        .iter_mut()
        .enumerate()
    {
        operation["index"] = serde_json::json!(index);
    }
    document["records"][5]["operation_count"] = serde_json::json!(26);
    document["total_operations"] = serde_json::json!(66);
    document
}

#[test]
fn schematic_sheet_accepts_canonical_and_authoritative_variants() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "hierarchical-sheets-follow-symbol-overplots",
    );
    validate_schematic_plot_document(&decode(&canonical)).expect("canonical sheet vector");

    let mut zero_outline_width = canonical.clone();
    zero_outline_width["records"][6]["operations"][0]["width_nm"] = serde_json::json!(0);
    zero_outline_width["records"][6]["operations"][1]["width_nm"] = serde_json::json!(0);
    validate_schematic_plot_document(&decode(&zero_outline_width))
        .expect("authored negative sheet stroke projects to zero width");

    let undecorated_round = sheet_with_undecorated_pin(&canonical, "round");
    validate_schematic_plot_document(&decode(&undecorated_round))
        .expect("undecorated round sheet pin");

    let undecorated_dot = sheet_with_undecorated_pin(&canonical, "dot");
    validate_schematic_plot_document(&decode(&undecorated_dot)).expect("undecorated dot sheet pin");
}

#[test]
fn schematic_sheet_semantic_mutations_fail_closed() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "hierarchical-sheets-follow-symbol-overplots",
    );
    let mut mutations = Vec::new();

    let mut wrong_phase = canonical.clone();
    wrong_phase["records"]
        .as_array_mut()
        .expect("records")
        .swap(4, 5);
    mutations.push(wrong_phase);

    let mut wrong_identity = canonical.clone();
    wrong_identity["records"][5]["object_id"] = serde_json::json!("Other");
    mutations.push(wrong_identity);

    let mut reversed_body = canonical.clone();
    let operations = reversed_body["records"][5]["operations"]
        .as_array_mut()
        .expect("operations");
    operations.swap(0, 1);
    for (index, operation) in operations.iter_mut().enumerate() {
        operation["index"] = serde_json::json!(index);
    }
    mutations.push(reversed_body);

    let mut mismatched_transparent_outline = canonical.clone();
    mismatched_transparent_outline["records"][6]["operations"][1]["x2"] =
        serde_json::json!(20_000_001);
    mutations.push(mismatched_transparent_outline);

    let mut undecorated_with_decoration = canonical.clone();
    undecorated_with_decoration["records"][5]["operations"][2]["extra_attrs"]["shape"] =
        serde_json::json!("round");
    mutations.push(undecorated_with_decoration);

    let mut decorated_without_decoration = sheet_with_undecorated_pin(&canonical, "round");
    decorated_without_decoration["records"][5]["operations"][2]["extra_attrs"]["shape"] =
        serde_json::json!("input");
    mutations.push(decorated_without_decoration);

    let mut text_before_pin_block = canonical.clone();
    let operations = text_before_pin_block["records"][5]["operations"]
        .as_array_mut()
        .expect("operations");
    operations.swap(2, 3);
    for (index, operation) in operations.iter_mut().enumerate() {
        operation["index"] = serde_json::json!(index);
    }
    mutations.push(text_before_pin_block);

    let mut wrong_pin_parent = canonical.clone();
    wrong_pin_parent["records"][5]["operations"][2]["extra_attrs"]["sheet-uuid"] =
        serde_json::json!("sheet-clear");
    mutations.push(wrong_pin_parent);

    let mut wrong_pin_shape = canonical.clone();
    wrong_pin_shape["records"][5]["operations"][10]["extra_attrs"]["shape"] =
        serde_json::json!("input");
    mutations.push(wrong_pin_shape);

    let mut open_pin_decoration = canonical.clone();
    open_pin_decoration["records"][5]["operations"][20]["points"][0][0] =
        serde_json::json!(10_000_001);
    mutations.push(open_pin_decoration);

    let mut inconsistent_dnp = canonical.clone();
    inconsistent_dnp["records"][5]["dnp"] = serde_json::json!(false);
    mutations.push(inconsistent_dnp);

    let mut wrong_marker = canonical.clone();
    wrong_marker["records"][5]["operations"][26]["width_nm"] = serde_json::json!(457_201);
    mutations.push(wrong_marker);

    for mutation in mutations {
        assert!(validate_schematic_plot_document(&decode(&mutation)).is_err());
    }
}

#[test]
fn schematic_sheet_decode_rejects_noncanonical_presence_and_operation_kinds() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "hierarchical-sheets-follow-symbol-overplots",
    );
    let mut null_fill_color = canonical.clone();
    null_fill_color["records"][6]["operations"][0]["fill_color"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(null_fill_color).is_err());

    let mut null_text_context = canonical.clone();
    null_text_context["records"][5]["operations"][7]["context"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(null_text_context).is_err());

    let mut present_empty_layers = canonical.clone();
    present_empty_layers["records"][5]["operations"][25]["layers"] = serde_json::json!([]);
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(present_empty_layers).is_err());

    let mut foreign_operation = canonical;
    foreign_operation["records"][6]["operations"][2] = serde_json::json!({
        "kind": "Circle", "index": 2, "cx": 0, "cy": 0,
        "diameter_nm": 1, "fill": "NO_FILL", "width_nm": 0
    });
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(foreign_operation).is_err());
}

#[test]
fn schematic_annotation_semantic_mutations_fail_closed() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "custom-worksheet-connectivity-and-annotation-family-order",
    );
    validate_schematic_plot_document(&decode(&canonical)).expect("canonical annotation vector");

    let mut mutations = Vec::new();
    let mut wrong_phase = canonical.clone();
    wrong_phase["records"]
        .as_array_mut()
        .expect("records")
        .swap(6, 7);
    mutations.push(wrong_phase);

    let mut reversed_global = canonical.clone();
    reversed_global["records"][7]["operations"]
        .as_array_mut()
        .expect("global operations")
        .reverse();
    mutations.push(reversed_global);

    let mut reversed_netclass = canonical.clone();
    reversed_netclass["records"][9]["operations"]
        .as_array_mut()
        .expect("netclass operations")
        .swap(0, 1);
    mutations.push(reversed_netclass);

    let mut text_before_rect = canonical.clone();
    text_before_rect["records"][11]["operations"]
        .as_array_mut()
        .expect("text box operations")
        .swap(0, 2);
    mutations.push(text_before_rect);

    for mutation in mutations {
        assert!(validate_schematic_plot_document(&decode(&mutation)).is_err());
    }

    let mut blank_href = canonical;
    assert!(insert_first_operation_field(
        &mut blank_href,
        "Text",
        "context",
        serde_json::json!({"hyperlink": {"href": ""}}),
    ));
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(blank_href).is_err());
}

#[test]
fn schematic_graphic_rule_image_and_table_mutations_fail_closed() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "schematic-graphics-rules-images-and-table-family-order",
    );
    validate_schematic_plot_document(&decode(&canonical)).expect("canonical P5_062 vector");

    let mut mutations = Vec::new();

    let mut wrong_phase = canonical.clone();
    wrong_phase["records"]
        .as_array_mut()
        .expect("records")
        .swap(1, 2);
    mutations.push(wrong_phase);

    let mut layered_polyline = canonical.clone();
    layered_polyline["records"][1]["operations"][0]["layer"] = serde_json::json!("Notes");
    mutations.push(layered_polyline);

    let mut short_polyline = canonical.clone();
    short_polyline["records"][1]["operations"][0]["points"] =
        serde_json::json!([[1_000_000, 1_000_000]]);
    mutations.push(short_polyline);

    let mut reversed_fill_pair = canonical.clone();
    reversed_fill_pair["records"][2]["operations"]
        .as_array_mut()
        .expect("arc operations")
        .reverse();
    mutations.push(reversed_fill_pair);

    let mut mismatched_fill_pair = canonical.clone();
    mismatched_fill_pair["records"][4]["operations"][0]["fill_color"] =
        serde_json::json!("#01020304");
    mutations.push(mismatched_fill_pair);

    let mut bezier_tolerance = canonical.clone();
    bezier_tolerance["records"][5]["operations"][0]["tolerance_nm"] = serde_json::json!(1);
    mutations.push(bezier_tolerance);

    let mut wrong_rule_shape = canonical.clone();
    wrong_rule_shape["records"][6]["shape"] = serde_json::json!("circle");
    mutations.push(wrong_rule_shape);

    let mut open_rule_polyline = canonical.clone();
    open_rule_polyline["records"][10]["operations"][0]["points"]
        .as_array_mut()
        .expect("rule points")
        .pop();
    mutations.push(open_rule_polyline);

    let mut mismatched_image_format = canonical.clone();
    mismatched_image_format["records"][11]["image_format"] = serde_json::json!("png");
    mutations.push(mismatched_image_format);

    let mut image_whitespace = canonical.clone();
    let encoded = image_whitespace["records"][12]["operations"][0]["image_data_b64"]
        .as_str()
        .expect("JPEG base64")
        .to_owned();
    image_whitespace["records"][12]["operations"][0]["image_data_b64"] =
        serde_json::json!(format!("{}\n{}", &encoded[..12], &encoded[12..]));
    mutations.push(image_whitespace);

    let mut wrong_image_extent = canonical.clone();
    wrong_image_extent["records"][13]["operations"][0]["width_nm"] = serde_json::json!(1_058_334);
    mutations.push(wrong_image_extent);

    let mut wrong_image_style = canonical.clone();
    wrong_image_style["records"][13]["operations"][0]["stroke_color"] =
        serde_json::json!("#0000C200");
    mutations.push(wrong_image_style);

    let mut wrong_cell_count = canonical.clone();
    wrong_cell_count["records"][14]["cell_count"] = serde_json::json!(2);
    mutations.push(wrong_cell_count);

    let mut text_before_cell = canonical;
    text_before_cell["records"][14]["operations"]
        .as_array_mut()
        .expect("table operations")
        .swap(0, 2);
    mutations.push(text_before_cell);

    for mutation in mutations {
        assert!(validate_schematic_plot_document(&decode(&mutation)).is_err());
    }
}

#[test]
fn schematic_symbol_pin_and_overplot_mutations_fail_closed() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "placed-symbols-pins-fields-dnp-and-overplots",
    );
    validate_schematic_plot_document(&decode(&canonical)).expect("canonical P5_070 vector");

    let mut mutations = Vec::new();

    let mut wrong_identity = canonical.clone();
    wrong_identity["records"][1]["object_id"] = serde_json::json!("other");
    mutations.push(wrong_identity);

    let mut invalid_mirror = canonical.clone();
    invalid_mirror["records"][1]["mirror"] = serde_json::json!("z");
    mutations.push(invalid_mirror);

    let mut overplot_before_instance = canonical.clone();
    overplot_before_instance["records"]
        .as_array_mut()
        .expect("records")
        .swap(2, 3);
    mutations.push(overplot_before_instance);

    let mut wrong_overplot_uuid = canonical.clone();
    wrong_overplot_uuid["records"][3]["uuid"] = serde_json::json!("bad:overplot");
    mutations.push(wrong_overplot_uuid);

    let mut foreign_attrs = canonical.clone();
    foreign_attrs["records"][1]["operations"][3]["extra_attrs"]["foreign"] = serde_json::json!("x");
    mutations.push(foreign_attrs);

    let mut wrong_parent = canonical.clone();
    wrong_parent["records"][1]["operations"][3]["extra_attrs"]["symbol-uuid"] =
        serde_json::json!("placed-2");
    mutations.push(wrong_parent);

    let mut nested_block = canonical.clone();
    let mut start = nested_block["records"][1]["operations"][3].clone();
    start["index"] = serde_json::json!(4);
    nested_block["records"][1]["operations"][4] = start;
    mutations.push(nested_block);

    let mut pin_href = canonical.clone();
    pin_href["records"][1]["operations"][5]["context"] =
        serde_json::json!({"hyperlink": {"href": "https://example.test/pin"}});
    mutations.push(pin_href);

    let mut image_in_symbol = canonical.clone();
    image_in_symbol["records"][1]["operations"][0] = serde_json::json!({
        "kind": "PlotImage", "index": 0,
        "x": 0, "y": 0, "width_nm": 84667, "height_nm": 84667,
        "scale": 1.0, "image_data_b64": ONE_BY_ONE_PNG,
        "image_format": "png"
    });
    mutations.push(image_in_symbol);

    let mut wrong_data_ref = canonical;
    wrong_data_ref["records"][1]["operations"][3]["data_ref"] = serde_json::json!("pad");
    mutations.push(wrong_data_ref);

    for mutation in mutations {
        assert!(validate_schematic_plot_document(&decode(&mutation)).is_err());
    }
}

#[test]
fn schematic_single_pass_filled_shape_preserves_authoritative_fill_states() {
    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "schematic-graphics-rules-images-and-table-family-order",
    );

    let mut explicit_graphic_fill = canonical.clone();
    explicit_graphic_fill["records"][3]["operations"][0]["fill_color"] =
        serde_json::json!("#01020304");
    validate_schematic_plot_document(&decode(&explicit_graphic_fill))
        .expect("explicit graphic fill may differ from its outline stroke");

    let mut absent_graphic_fill = canonical.clone();
    absent_graphic_fill["records"][3]["operations"][0]
        .as_object_mut()
        .expect("circle operation")
        .remove("fill_color");
    validate_schematic_plot_document(&decode(&absent_graphic_fill))
        .expect("single-pass filled shape may omit fill color");

    let mut absent_table_fill = canonical.clone();
    absent_table_fill["records"][14]["operations"][3]["fill"] = serde_json::json!("FILLED_SHAPE");
    validate_schematic_plot_document(&decode(&absent_table_fill))
        .expect("single-pass table-cell outline fill may omit fill color");

    let mut explicit_table_fill = absent_table_fill;
    explicit_table_fill["records"][14]["operations"][3]["fill_color"] =
        serde_json::json!("#11223344");
    validate_schematic_plot_document(&decode(&explicit_table_fill))
        .expect("table-cell filled shape may carry an explicit fill color");

    let mut invalid_color = canonical;
    invalid_color["records"][3]["operations"][0]["fill_color"] = serde_json::json!("#abcdef12");
    assert!(validate_schematic_plot_document(&decode(&invalid_color)).is_err());
}

#[test]
fn shared_context_and_segment_color_remain_fail_closed_for_existing_producers() {
    let context = serde_json::json!({"hyperlink": {"href": "https://example.test"}});
    let mut board = shared_vector(
        "board_plotter_a0_vectors.json",
        "board-text-follows-python-serializer",
    );
    assert!(insert_first_operation_field(
        &mut board,
        "Text",
        "context",
        context.clone(),
    ));
    let board: BoardPlotDocumentA0 = serde_json::from_value(board).expect("board context shape");
    assert!(validate_board_plot_document(&board).is_err());

    let mut footprint = shared_vector(
        "footprint_plotter_a0_vectors.json",
        "standalone-properties-text-and-text-box",
    );
    assert!(insert_first_operation_field(
        &mut footprint,
        "Text",
        "context",
        context.clone(),
    ));
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(footprint).expect("footprint context shape");
    assert!(validate_footprint_plot_document(&footprint).is_err());

    let mut symbol = shared_vector("symbol_plotter_a0_vectors.json", "styled-body-and-pin-text");
    assert!(insert_first_operation_field(
        &mut symbol,
        "Text",
        "context",
        context,
    ));
    let symbol: SymbolPlotDocumentA0 =
        serde_json::from_value(symbol).expect("symbol context shape");
    assert!(validate_symbol_plot_document(&symbol).is_err());

    let mut footprint = shared_vector(
        "footprint_plotter_a0_vectors.json",
        "solid-line-with-metadata",
    );
    assert!(insert_first_operation_field(
        &mut footprint,
        "ThickSegment",
        "stroke_color",
        serde_json::json!("#484848FF"),
    ));
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(footprint).expect("footprint segment color shape");
    assert!(validate_footprint_plot_document(&footprint).is_err());
}

#[test]
fn shared_context_and_segment_color_reject_explicit_null_for_existing_producers() {
    let mut board = shared_vector(
        "board_plotter_a0_vectors.json",
        "board-text-follows-python-serializer",
    );
    assert!(insert_first_operation_field(
        &mut board,
        "Text",
        "context",
        serde_json::Value::Null,
    ));
    assert!(serde_json::from_value::<BoardPlotDocumentA0>(board).is_err());

    let mut footprint = shared_vector(
        "footprint_plotter_a0_vectors.json",
        "standalone-properties-text-and-text-box",
    );
    assert!(insert_first_operation_field(
        &mut footprint,
        "Text",
        "context",
        serde_json::Value::Null,
    ));
    assert!(serde_json::from_value::<FootprintPlotDocumentA0>(footprint).is_err());

    let mut symbol = shared_vector("symbol_plotter_a0_vectors.json", "styled-body-and-pin-text");
    assert!(insert_first_operation_field(
        &mut symbol,
        "Text",
        "context",
        serde_json::Value::Null,
    ));
    assert!(serde_json::from_value::<SymbolPlotDocumentA0>(symbol).is_err());

    let mut board = shared_vector(
        "board_plotter_a0_vectors.json",
        "board-metadata-and-category-ordered-graphics",
    );
    assert!(insert_first_operation_field(
        &mut board,
        "ThickSegment",
        "stroke_color",
        serde_json::Value::Null,
    ));
    assert!(serde_json::from_value::<BoardPlotDocumentA0>(board).is_err());

    let mut footprint = shared_vector(
        "footprint_plotter_a0_vectors.json",
        "solid-line-with-metadata",
    );
    assert!(insert_first_operation_field(
        &mut footprint,
        "ThickSegment",
        "stroke_color",
        serde_json::Value::Null,
    ));
    assert!(serde_json::from_value::<FootprintPlotDocumentA0>(footprint).is_err());

    let mut symbol = shared_vector("symbol_plotter_a0_vectors.json", "styled-body-and-pin-text");
    symbol["records"][1]["operations"][0] = serde_json::json!({
        "kind": "ThickSegment", "index": 0,
        "start_x": 0, "start_y": 0, "end_x": 1, "end_y": 1,
        "width_nm": 1, "stroke_color": null,
    });
    assert!(serde_json::from_value::<SymbolPlotDocumentA0>(symbol).is_err());
}

#[test]
fn frozen_documents_reject_null_optionals_and_missing_required_nullables() {
    let mut board = shared_vector(
        "board_plotter_a0_vectors.json",
        "board-metadata-and-category-ordered-graphics",
    );
    board["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BoardPlotDocumentA0>(board).is_err());

    let mut footprint = shared_vector(
        "footprint_plotter_a0_vectors.json",
        "solid-line-with-metadata",
    );
    footprint["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<FootprintPlotDocumentA0>(footprint).is_err());

    let mut symbol = shared_vector("symbol_plotter_a0_vectors.json", "styled-body-and-pin-text");
    symbol["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SymbolPlotDocumentA0>(symbol).is_err());

    let canonical = shared_vector(
        "schematic_plotter_a0_vectors.json",
        "placed-symbols-pins-fields-dnp-and-overplots",
    );
    let mut schematic = canonical.clone();
    schematic["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(schematic).is_err());

    for field in ["paper_width_mm", "paper_height_mm"] {
        let mut missing = canonical.clone();
        missing["records"][0]
            .as_object_mut()
            .expect("sheet header")
            .remove(field);
        assert!(serde_json::from_value::<SchematicPlotDocumentA0>(missing).is_err());
    }
    let mut missing_mirror = canonical;
    missing_mirror["records"][1]
        .as_object_mut()
        .expect("symbol instance")
        .remove("mirror");
    assert!(serde_json::from_value::<SchematicPlotDocumentA0>(missing_mirror).is_err());

    let mut board = shared_vector(
        "board_plotter_a0_vectors.json",
        "board-metadata-and-category-ordered-graphics",
    );
    board["records"][0]
        .as_object_mut()
        .expect("board graphic")
        .remove("layer");
    assert!(serde_json::from_value::<BoardPlotDocumentA0>(board).is_err());
}

#[test]
fn schematic_request_requires_every_independent_budget() {
    let request = serde_json::json!({
        "type": "kicad_monkey.schematic_plot.request", "version": "a0",
        "sheet_index": 1, "sheet_count": 1, "sheet_path": "/", "sheet_name": "",
        "worksheet_mode": "default", "text_offset_ratio": 0.15,
        "default_line_width_nm": 152400, "max_source_bytes": "4096",
        "max_worksheet_bytes": "4096", "max_output_bytes": "65536",
        "max_depth": 64, "max_parse_nodes": 1000, "max_selected_forms": 1000,
        "max_records": 100, "max_operations": 1000, "max_points": 10000,
        "max_input_points": 10000, "max_text_bytes": "65536",
        "max_metadata_bytes": "65536", "max_wires": 100, "max_buses": 100,
        "max_bus_entries": 100, "max_junctions": 100, "max_no_connects": 100,
        "max_labels": 100, "max_global_labels": 100,
        "max_hierarchical_labels": 100, "max_netclass_flags": 100,
        "max_netclass_flag_properties": 100, "max_texts": 100,
        "max_text_boxes": 100, "max_text_box_lines": 1000,
        "max_polylines": 100, "max_arcs": 100, "max_circles": 100,
        "max_rectangles": 100, "max_beziers": 100,
        "max_rule_areas": 100, "max_images": 100, "max_tables": 100,
        "max_table_cells": 100, "max_table_cell_lines": 1000,
        "max_image_data_parts": 100,
        "max_image_encoded_bytes": "4096",
        "max_image_decoded_bytes": "4096",
        "max_image_width_px": 1000, "max_image_height_px": 1000,
        "max_image_pixels": "1000000", "max_image_decode_work": "8192",
        "max_symbols": 100, "max_symbol_overplots": 100,
        "max_symbol_properties": 1000, "max_symbol_pins": 1000,
        "max_library_symbols": 100, "max_library_subsymbols": 1000,
        "max_library_pins": 1000, "max_symbol_overlap_checks": "10000",
        "max_sheets": 100, "max_sheet_properties": 1000,
        "max_sheet_pins": 1000,
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
    let mut null_source_path = request.clone();
    null_source_path["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SchematicPlotRequestA0>(null_source_path).is_err());
    for field in ["sheet_index", "sheet_count"] {
        let mut zero = request.clone();
        zero[field] = serde_json::json!(0);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(zero).is_err());

        let mut over_u32 = request.clone();
        over_u32[field] = serde_json::json!(4_294_967_296_u64);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(over_u32).is_err());
    }
    let mut too_thin = request.clone();
    too_thin["default_line_width_nm"] = serde_json::json!(84_699);
    assert!(serde_json::from_value::<SchematicPlotRequestA0>(too_thin).is_err());

    let mut negative_ratio = request.clone();
    negative_ratio["text_offset_ratio"] = serde_json::json!(-0.01);
    assert!(serde_json::from_value::<SchematicPlotRequestA0>(negative_ratio).is_err());
    for (field, value) in [
        ("max_image_encoded_bytes", "not-a-number"),
        ("max_image_pixels", "18446744073709551616"),
    ] {
        let mut invalid_u64 = request.clone();
        invalid_u64[field] = serde_json::json!(value);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(invalid_u64).is_err());
    }
    for field in [
        "max_source_bytes",
        "max_worksheet_bytes",
        "max_records",
        "max_input_points",
        "max_worksheet_repeats",
        "max_worksheet_bitmap_decode_work",
        "max_labels",
        "max_netclass_flag_properties",
        "max_text_box_lines",
        "max_polylines",
        "max_rule_areas",
        "max_table_cell_lines",
        "max_image_decode_work",
        "max_symbol_overlap_checks",
        "max_sheets",
        "max_sheet_properties",
        "max_sheet_pins",
        "text_offset_ratio",
        "default_line_width_nm",
    ] {
        let mut missing = request.clone();
        missing.as_object_mut().expect("request").remove(field);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(missing).is_err());
    }
    for field in ["max_sheets", "max_sheet_properties", "max_sheet_pins"] {
        let mut over_u32 = request.clone();
        over_u32[field] = serde_json::json!(4_294_967_296_u64);
        assert!(serde_json::from_value::<SchematicPlotRequestA0>(over_u32).is_err());
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

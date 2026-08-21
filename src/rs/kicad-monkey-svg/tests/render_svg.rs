use kicad_monkey_contracts::decode_native_svg_render_request_a0;
use kicad_monkey_svg::render_svg;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

#[test]
fn renders_one_frozen_vector_for_every_producer_deterministically() {
    let mut rendered = 0usize;
    let mut governed_rejections = 0usize;
    for (file, kind, width, height) in [
        (
            "footprint_plotter_a0_vectors.json",
            "footprint",
            20_000_000,
            20_000_000,
        ),
        (
            "symbol_plotter_a0_vectors.json",
            "symbol",
            20_000_000,
            20_000_000,
        ),
        (
            "board_plotter_a0_vectors.json",
            "board",
            100_000_000,
            100_000_000,
        ),
        (
            "schematic_plotter_a0_vectors.json",
            "schematic",
            297_000_000,
            210_000_000,
        ),
    ] {
        for document in documents(file) {
            let document_id = document["document_id"].as_str().unwrap().to_owned();
            let (width, height) = if kind == "schematic" {
                let canvas = &document["canvas"];
                (
                    canvas["width_nm"].as_i64().unwrap(),
                    canvas["height_nm"].as_i64().unwrap(),
                )
            } else {
                (width, height)
            };
            let request = request(document, kind, width, height, 1_000_000);
            let decoded =
                decode_native_svg_render_request_a0(&serde_json::to_vec(&request).unwrap())
                    .unwrap_or_else(|error| panic!("strict {file} request: {error}"));
            if kind == "board" && document_id == "fixture-tracks" {
                let error = render_svg(&decoded).expect_err("negative-width board SVG rejection");
                assert!(error.to_string().contains("width_nm must be nonnegative"));
                governed_rejections += 1;
                continue;
            }
            let first = render_svg(&decoded)
                .unwrap_or_else(|error| panic!("render {file}/{document_id}: {error}"));
            let second = render_svg(&decoded).expect("repeat render SVG");
            assert_eq!(first.svg, second.svg);
            assert!(first.svg.starts_with("<?xml version=\"1.0\""));
            assert!(first.svg.ends_with("</svg>\n"));
            assert!(first.svg.contains("data-ref="));
            rendered += 1;
        }
    }
    assert_eq!(rendered, 29);
    assert_eq!(governed_rejections, 1);
}

#[test]
fn record_ceiling_is_inclusive_and_fails_closed() {
    let document = first_document("footprint_plotter_a0_vectors.json");
    let exact = request(document.clone(), "footprint", 20_000_000, 20_000_000, 1);
    let decoded =
        decode_native_svg_render_request_a0(&serde_json::to_vec(&exact).unwrap()).unwrap();
    assert!(render_svg(&decoded).is_ok());

    let one_under = request(document, "footprint", 20_000_000, 20_000_000, 0);
    let decoded =
        decode_native_svg_render_request_a0(&serde_json::to_vec(&one_under).unwrap()).unwrap();
    let error = render_svg(&decoded).unwrap_err();
    assert!(error.to_string().contains("records exceeds"));
}

#[test]
fn every_svg_ceiling_accepts_exact_and_rejects_one_under() {
    let graphics = document_by_id(
        "schematic_plotter_a0_vectors.json",
        "schematic-graphics-rules-images-and-table-family-order",
    );
    let canvas = graphics["canvas"].clone();
    let base = request(
        graphics,
        "schematic",
        canvas["width_nm"].as_i64().unwrap(),
        canvas["height_nm"].as_i64().unwrap(),
        1_000_000,
    );
    let decoded = decode_native_svg_render_request_a0(&serde_json::to_vec(&base).unwrap()).unwrap();
    let metrics = render_svg(&decoded).expect("rich SVG").metrics;
    for (field, exact) in [
        ("max_records", metrics.records),
        ("max_operations", metrics.operations),
        ("max_points", metrics.points),
        ("max_text_bytes", metrics.text_bytes),
        ("max_image_encoded_bytes", metrics.image_encoded_bytes),
        ("max_svg_elements", metrics.svg_elements),
        ("max_render_work", metrics.render_work),
        ("max_svg_bytes", metrics.svg_bytes),
    ] {
        assert_exact_and_one_under(&base, field, exact);
    }

    let blocked = document_by_id(
        "schematic_plotter_a0_vectors.json",
        "placed-symbols-pins-fields-dnp-and-overplots",
    );
    let canvas = blocked["canvas"].clone();
    let request = request(
        blocked,
        "schematic",
        canvas["width_nm"].as_i64().unwrap(),
        canvas["height_nm"].as_i64().unwrap(),
        1_000_000,
    );
    let decoded =
        decode_native_svg_render_request_a0(&serde_json::to_vec(&request).unwrap()).unwrap();
    let depth = render_svg(&decoded)
        .expect("blocked SVG")
        .metrics
        .block_depth;
    assert!(depth > 0);
    assert_exact_and_one_under(&request, "max_block_depth", depth);
}

#[test]
fn board_footprint_placement_widths_multiline_and_ownership_are_exact() {
    let footprint = document_by_id(
        "board_plotter_a0_vectors.json",
        "embedded-footprints-follow-zones-and-keep-local-ownership",
    );
    let decoded = decode_native_svg_render_request_a0(
        &serde_json::to_vec(&request(
            footprint,
            "board",
            200_000_000,
            200_000_000,
            1_000_000,
        ))
        .unwrap(),
    )
    .unwrap();
    let svg = render_svg(&decoded).expect("placed footprint SVG").svg;
    assert!(svg.contains(
        "id=\"footprint-rich\" data-ref=\"footprint\" data-object-id=\"Demo:Rich\" transform=\"translate(10000000 20000000) rotate(-90)\""
    ));
    assert!(svg.contains(
        "<g id=\"footprint-empty\" data-ref=\"footprint\" data-object-id=\"Demo:Empty\">"
    ));
    assert!(svg.contains(
        "id=\"property-reference:footprint-text:0\" data-uuid=\"property-reference\" data-ref=\"property\" data-object-id=\"Reference\""
    ));

    let widths = document_by_id(
        "board_plotter_a0_vectors.json",
        "python-defaults-and-unclamped-stroke-widths",
    );
    let decoded = decode_native_svg_render_request_a0(
        &serde_json::to_vec(&request(
            widths,
            "board",
            200_000_000,
            200_000_000,
            1_000_000,
        ))
        .unwrap(),
    )
    .unwrap();
    let svg = render_svg(&decoded).expect("authored width SVG").svg;
    assert!(svg.contains("stroke-width=\"50000\""));

    let multiline = document_by_id(
        "board_plotter_a0_vectors.json",
        "text-boxes-bundle-border-and-alignment",
    );
    let decoded = decode_native_svg_render_request_a0(
        &serde_json::to_vec(&request(
            multiline,
            "board",
            200_000_000,
            200_000_000,
            1_000_000,
        ))
        .unwrap(),
    )
    .unwrap();
    let svg = render_svg(&decoded).expect("multiline SVG").svg;
    assert!(svg.contains("x=\"1755000\" y=\"1000000\""));
    assert!(svg.contains("x=\"3435000\" y=\"1000000\""));
    assert!(svg.contains("rotate(-90 1755000 1000000)"));
}

#[test]
fn empty_record_ids_are_omitted_and_duplicate_nonempty_ids_fail_closed() {
    let symbol = first_document("symbol_plotter_a0_vectors.json");
    let decoded = decode_native_svg_render_request_a0(
        &serde_json::to_vec(&request(
            symbol, "symbol", 20_000_000, 20_000_000, 1_000_000,
        ))
        .unwrap(),
    )
    .unwrap();
    let svg = render_svg(&decoded).expect("empty record ids").svg;
    assert!(!svg.contains(" id=\"\""));

    let mut board = document_by_id(
        "board_plotter_a0_vectors.json",
        "python-defaults-and-unclamped-stroke-widths",
    );
    board["records"][0]["uuid"] = json!("duplicate");
    board["records"][1]["uuid"] = json!("duplicate");
    let decoded = decode_native_svg_render_request_a0(
        &serde_json::to_vec(&request(board, "board", 20_000_000, 20_000_000, 1_000_000)).unwrap(),
    )
    .unwrap();
    assert!(
        render_svg(&decoded)
            .expect_err("duplicate SVG ids")
            .to_string()
            .contains("duplicate nonempty SVG id")
    );
}

#[test]
fn oval_pads_are_stadiums_and_negative_geometry_mutations_fail_closed() {
    let pads = document_by_id(
        "footprint_plotter_a0_vectors.json",
        "standard-pad-flashes-and-drills",
    );
    let decoded = decode_native_svg_render_request_a0(
        &serde_json::to_vec(&request(
            pads.clone(),
            "footprint",
            20_000_000,
            20_000_000,
            1_000_000,
        ))
        .unwrap(),
    )
    .unwrap();
    let svg = render_svg(&decoded).expect("pad stadium SVG").svg;
    assert!(svg.contains(
        "<line x1=\"-1500000\" y1=\"0\" x2=\"-500000\" y2=\"0\" transform=\"rotate(-30 -1000000 0)\" fill=\"none\" stroke=\"#000000\" stroke-width=\"1000000\" stroke-linecap=\"round\""
    ));
    assert!(!svg.contains("<ellipse"));

    for (kind, field) in [
        ("ThickSegment", "width_nm"),
        ("Circle", "diameter_nm"),
        ("FlashPadCircle", "diameter_nm"),
        ("FlashPadOval", "size_x_nm"),
        ("FlashPadRect", "size_y_nm"),
        ("FlashPadRoundRect", "size_x_nm"),
        ("FlashPadRoundRect", "corner_radius_nm"),
    ] {
        let mut mutated = pads.clone();
        mutate_first_operation(&mut mutated, kind, field, json!(-1));
        let request = request(mutated, "footprint", 20_000_000, 20_000_000, 1_000_000);
        let decoded = decode_native_svg_render_request_a0(&serde_json::to_vec(&request).unwrap())
            .unwrap_or_else(|error| panic!("strict mutation {kind}.{field}: {error}"));
        let Err(error) = render_svg(&decoded) else {
            panic!("negative {kind}.{field} unexpectedly rendered");
        };
        assert!(error.to_string().contains("must be nonnegative"));
    }
}

fn mutate_first_operation(document: &mut Value, kind: &str, field: &str, replacement: Value) {
    for record in document["records"].as_array_mut().expect("record array") {
        for operation in record["operations"]
            .as_array_mut()
            .expect("operation array")
        {
            if operation["kind"] == kind {
                operation[field] = replacement;
                return;
            }
        }
    }
    panic!("missing {kind} operation");
}

fn assert_exact_and_one_under(request: &Value, field: &str, exact: usize) {
    assert!(
        exact > 0,
        "{field} fixture must exercise a positive boundary"
    );
    let mut exact_request = request.clone();
    exact_request["limits"][field] = limit_value(field, exact);
    let decoded =
        decode_native_svg_render_request_a0(&serde_json::to_vec(&exact_request).unwrap()).unwrap();
    render_svg(&decoded).unwrap_or_else(|error| panic!("exact {field}={exact}: {error}"));

    let mut one_under = request.clone();
    one_under["limits"][field] = limit_value(field, exact - 1);
    let decoded =
        decode_native_svg_render_request_a0(&serde_json::to_vec(&one_under).unwrap()).unwrap();
    assert!(
        render_svg(&decoded).is_err(),
        "one-under {field} unexpectedly passed"
    );
}

fn limit_value(field: &str, value: usize) -> Value {
    if matches!(field, "max_records" | "max_operations" | "max_block_depth") {
        json!(value)
    } else {
        json!(value.to_string())
    }
}

fn first_document(file: &str) -> Value {
    documents(file).into_iter().next().expect("one vector")
}

fn document_by_id(file: &str, id: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read vector"),
    )
    .expect("decode vector");
    parsed["vectors"]
        .as_array()
        .expect("vector array")
        .iter()
        .find(|vector| vector["id"] == id)
        .expect("vector id")["expected"]
        .clone()
}

fn documents(file: &str) -> Vec<Value> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read vector"),
    )
    .expect("decode vector");
    parsed["vectors"]
        .as_array()
        .expect("vector array")
        .iter()
        .map(|vector| vector["expected"].clone())
        .collect()
}

fn request(document: Value, kind: &str, width: i64, height: i64, records: u32) -> Value {
    json!({
        "type": "kicad_monkey.native.svg.request",
        "version": "a0",
        "profile": "plotter-base-a0",
        "document": {"kind": kind, "value": document},
        "viewport": {
            "min_x_nm": 0,
            "min_y_nm": 0,
            "width_nm": width,
            "height_nm": height
        },
        "limits": {
            "max_records": records,
            "max_operations": 100000,
            "max_points": "1000000",
            "max_text_bytes": "10000000",
            "max_image_encoded_bytes": "10000000",
            "max_block_depth": 100,
            "max_svg_elements": "1000000",
            "max_render_work": "100000000",
            "max_svg_bytes": "100000000",
            "max_result_bytes": "200000000"
        }
    })
}

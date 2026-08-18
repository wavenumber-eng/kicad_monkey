use kicad_monkey_contracts::{decode_native_handshake_a1, decode_native_svg_render_result_a0};
use kicad_monkey_native::{execute_svg_request_bytes, handshake_a1};
use serde_json::{Value, json};

#[test]
fn expanded_handshake_and_svg_result_satisfy_generated_contracts() {
    decode_native_handshake_a1(&serde_json::to_vec(&handshake_a1()).unwrap())
        .expect("strict a1 handshake");
    let output = execute_svg_request_bytes(&request()).expect("render SVG");
    let result = decode_native_svg_render_result_a0(&output).expect("strict SVG result");
    assert_eq!(result.source_kind.to_string(), "MOD");
    assert_eq!(result.document_id, "fixture");
    assert!(result.svg_utf8.contains("<line"));
    assert!(result.svg_utf8.contains("data-ref=\"footprint\""));
}

#[test]
fn svg_resource_failure_publishes_no_result() {
    let mut request = request_value();
    request["limits"]["max_svg_bytes"] = json!("1");
    let error = execute_svg_request_bytes(&serde_json::to_vec(&request).unwrap())
        .expect_err("one-byte output limit");
    assert!(error.to_string().contains("SVG bytes"));
}

#[test]
fn result_ceiling_accepts_exact_and_rejects_one_under() {
    let baseline = execute_svg_request_bytes(&request()).expect("baseline result");
    let mut exact = request_value();
    exact["limits"]["max_result_bytes"] = json!(baseline.len().to_string());
    let output = execute_svg_request_bytes(&serde_json::to_vec(&exact).unwrap())
        .expect("exact result ceiling");
    assert_eq!(output.len(), baseline.len());

    exact["limits"]["max_result_bytes"] = json!((baseline.len() - 1).to_string());
    let error = execute_svg_request_bytes(&serde_json::to_vec(&exact).unwrap())
        .expect_err("one-under result ceiling");
    assert!(error.to_string().contains("max_output_bytes"));
}

fn request() -> Vec<u8> {
    serde_json::to_vec(&request_value()).expect("request JSON")
}

fn request_value() -> Value {
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("footprint vectors");
    json!({
        "type": "kicad_monkey.native.svg.request",
        "version": "a0",
        "profile": "plotter-base-a0",
        "document": {"kind": "footprint", "value": vectors["vectors"][0]["expected"]},
        "viewport": {"min_x_nm": 0, "min_y_nm": -2000000, "width_nm": 2000000, "height_nm": 3000000},
        "limits": {
            "max_records": 1,
            "max_operations": 1,
            "max_points": "10",
            "max_text_bytes": "100",
            "max_image_encoded_bytes": "100",
            "max_block_depth": 1,
            "max_svg_elements": "10",
            "max_render_work": "100000",
            "max_svg_bytes": "100000",
            "max_result_bytes": "200000"
        }
    })
}

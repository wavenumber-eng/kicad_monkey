use kicad_monkey_contracts::{
    decode_native_design_facts_request_a0, decode_native_design_facts_result_a0,
    decode_native_error_a0, decode_native_handshake_a0,
};
use serde_json::{Value, json};

#[test]
fn native_transport_envelopes_decode_and_round_trip_exact_wire_names() {
    let handshake = json!({
        "type": "kicad_monkey.native.handshake",
        "version": "a0",
        "engine_version": "0.1.0",
        "operations": ["design-facts"]
    });
    let decoded = decode_native_handshake_a0(&encode(&handshake)).expect("strict handshake");
    assert_eq!(decoded.type_, handshake["type"]);
    assert_eq!(
        serde_json::to_value(decoded).expect("handshake encode"),
        handshake
    );

    let request = request_value();
    let decoded = decode_native_design_facts_request_a0(&encode(&request)).expect("strict request");
    assert_eq!(decoded.type_, request["type"]);
    assert_eq!(
        serde_json::to_value(decoded).expect("request encode"),
        request
    );

    let result = result_value();
    let decoded = decode_native_design_facts_result_a0(&encode(&result)).expect("strict result");
    assert_eq!(decoded.type_, result["type"]);
    assert_eq!(
        serde_json::to_value(decoded).expect("result encode"),
        result
    );

    let error = json!({
        "type": "kicad_monkey.native.error",
        "version": "a0",
        "kind": "resource_limit",
        "message": "bounded failure"
    });
    let decoded = decode_native_error_a0(&encode(&error)).expect("strict error");
    assert_eq!(decoded.type_, error["type"]);
    assert_eq!(serde_json::to_value(decoded).expect("error encode"), error);
}

#[test]
fn native_handshake_and_error_mutations_fail_closed() {
    let handshake = json!({
        "type": "kicad_monkey.native.handshake",
        "version": "a0",
        "engine_version": "0.1.0",
        "operations": ["design-facts"]
    });
    assert_rejected(
        handshake.clone(),
        "/type",
        json!("invented"),
        true,
        decode_native_handshake_a0,
    );
    assert_rejected(
        handshake.clone(),
        "/version",
        json!("a1"),
        true,
        decode_native_handshake_a0,
    );
    assert_rejected(
        handshake.clone(),
        "/engine_version",
        json!(""),
        true,
        decode_native_handshake_a0,
    );
    assert_rejected(
        handshake.clone(),
        "/operations",
        json!([]),
        true,
        decode_native_handshake_a0,
    );
    assert_rejected(
        handshake.clone(),
        "/operations",
        json!(["invented"]),
        true,
        decode_native_handshake_a0,
    );
    assert_rejected(
        handshake,
        "/unknown",
        json!(true),
        false,
        decode_native_handshake_a0,
    );

    let error = json!({
        "type": "kicad_monkey.native.error",
        "version": "a0",
        "kind": "request",
        "message": "failure"
    });
    assert_rejected(
        error.clone(),
        "/type",
        json!("invented"),
        true,
        decode_native_error_a0,
    );
    assert_rejected(
        error.clone(),
        "/version",
        json!("a1"),
        true,
        decode_native_error_a0,
    );
    assert_rejected(
        error.clone(),
        "/kind",
        json!("invented"),
        true,
        decode_native_error_a0,
    );
    assert_rejected(
        error,
        "/unknown",
        json!(true),
        false,
        decode_native_error_a0,
    );
}

#[test]
fn native_request_mutations_fail_closed() {
    let request = request_value();
    assert_rejected(
        request.clone(),
        "/manifest/project_path",
        Value::Null,
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/type",
        json!("invented"),
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/version",
        json!("a1"),
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/limits/max_output_bytes",
        json!("01"),
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/limits/max_source_bytes",
        json!("18446744073709551616"),
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/manifest/version",
        json!("a1"),
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/file_slots/0/slot",
        json!(-1),
        true,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request.clone(),
        "/unknown",
        json!(true),
        false,
        decode_native_design_facts_request_a0,
    );
    assert_rejected(
        request,
        "/file_slots",
        Value::Null,
        true,
        decode_native_design_facts_request_a0,
    );
}

#[test]
fn native_result_mutations_fail_closed() {
    let result = result_value();
    assert_rejected(
        result.clone(),
        "/compiled_schematic_graph/page_occurrences/0/address_key",
        Value::Null,
        true,
        decode_native_design_facts_result_a0,
    );
    assert_rejected(
        result.clone(),
        "/type",
        json!("invented"),
        true,
        decode_native_design_facts_result_a0,
    );
    assert_rejected(
        result.clone(),
        "/version",
        json!("a1"),
        true,
        decode_native_design_facts_result_a0,
    );
    assert_rejected(
        result.clone(),
        "/engine_version",
        json!(""),
        true,
        decode_native_design_facts_result_a0,
    );
    assert_rejected(
        result.clone(),
        "/kicad_netlist_version",
        json!("D"),
        true,
        decode_native_design_facts_result_a0,
    );
    assert_rejected(
        result.clone(),
        "/compiled_schematic_graph/unknown",
        json!(true),
        false,
        decode_native_design_facts_result_a0,
    );
    assert_rejected(
        result,
        "/unknown",
        json!(true),
        false,
        decode_native_design_facts_result_a0,
    );
}

fn request_value() -> Value {
    let source_bundle: Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/source_bundle_a0_vectors.json"
    ))
    .expect("source bundle vectors");
    json!({
        "type": "kicad_monkey.native.design_facts.request",
        "version": "a0",
        "bundle_root": "C:/bundle",
        "manifest": source_bundle["manifest"],
        "file_slots": [
            {"slot": 0, "path": "design/root.kicad_pro"},
            {"slot": 1, "path": "design/root.kicad_sch"}
        ],
        "limits": {
            "max_sources": 2,
            "max_source_bytes": "1048576",
            "max_total_source_bytes": "2097152",
            "max_path_bytes": 4096,
            "max_output_bytes": "8388608"
        },
        "netlist": {"source_path": "design/root.kicad_sch", "date": "", "tool": "kicad-monkey-native"}
    })
}

fn result_value() -> Value {
    let graph_vectors: Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/compiled_schematic_graph_a0_vectors.json"
    ))
    .expect("compiled graph vectors");
    json!({
        "type": "kicad_monkey.native.design_facts.result",
        "version": "a0",
        "engine_version": "0.1.0",
        "compiled_schematic_graph": graph_vectors["graph"],
        "kicad_netlist_version": "E",
        "kicad_netlist": "(export (version \"E\"))"
    })
}

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("JSON encoding")
}

fn assert_rejected<T, E>(
    mut value: Value,
    pointer: &str,
    replacement: Value,
    replace: bool,
    decode: fn(&[u8]) -> Result<T, E>,
) {
    if replace {
        *value.pointer_mut(pointer).expect("mutation path") = replacement;
    } else {
        let (parent, field) = pointer.rsplit_once('/').expect("object mutation path");
        value
            .pointer_mut(parent)
            .expect("mutation parent")
            .as_object_mut()
            .expect("mutation object")
            .insert(field.to_owned(), replacement);
    }
    assert!(
        decode(&encode(&value)).is_err(),
        "mutation accepted at {pointer}"
    );
}

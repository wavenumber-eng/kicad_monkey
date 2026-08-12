use kicad_monkey_contracts::generated::build_request::SExpressionBuildRequestA0;
use kicad_monkey_contracts::generated::footprint_plot_document::FootprintPlotDocumentA0;
use kicad_monkey_contracts::generated::scan_request::SExpressionScanRequestA0;
use kicad_monkey_contracts::{
    JAVASCRIPT_SAFE_INTEGER_MAX, JAVASCRIPT_SAFE_INTEGER_MIN, JavaScriptSafeInteger, ValidatedNode,
    validate_build_request,
};

#[test]
fn generated_scan_request_is_strict_and_round_trips_wire_names() {
    let json = r#"{
        "type":"kicad_monkey.sexpr_scan.request",
        "version":"a0",
        "selector":{"heads":["footprint"]},
        "max_source_bytes":"536870912",
        "max_depth":512,
        "max_selected_forms":1000
    }"#;
    let request: SExpressionScanRequestA0 =
        serde_json::from_str(json).expect("generated request should decode");
    assert_eq!(request.type_, "kicad_monkey.sexpr_scan.request");
    assert_eq!(request.selector.heads, ["footprint"]);

    let encoded = serde_json::to_value(request).expect("request should encode");
    assert_eq!(encoded["type"], "kicad_monkey.sexpr_scan.request");
    assert!(encoded.get("type_").is_none());

    let with_extra = json.replace("\"max_depth\":512", "\"extra\":1,\"max_depth\":512");
    assert!(serde_json::from_str::<SExpressionScanRequestA0>(&with_extra).is_err());
}

fn build_request(root: &str, max_depth: u32, max_nodes: u32) -> SExpressionBuildRequestA0 {
    serde_json::from_str(&format!(
        r#"{{"type":"kicad_monkey.sexpr_build.request","version":"a0","root":{root},"max_output_bytes":"1024","max_depth":{max_depth},"max_nodes":{max_nodes}}}"#
    ))
    .expect("generated build request should decode")
}

#[test]
fn build_node_semantics_accept_exact_payloads() {
    let request = build_request(
        r#"{"kind":"list","children":[{"kind":"atom","text":"root"},{"kind":"integer","integer":"42"},{"kind":"float","float":1.5},{"kind":"quoted","text":"a b"}]}"#,
        4,
        8,
    );
    let validated = validate_build_request(request).expect("valid union payloads");
    assert!(matches!(validated.root, ValidatedNode::List(children) if children.len() == 4));
}

#[test]
fn build_node_semantics_reject_conflicts_missing_payloads_and_bad_identity() {
    let conflict = build_request(r#"{"kind":"atom","text":"x","integer":"1"}"#, 1, 1);
    assert_eq!(
        validate_build_request(conflict).expect_err("conflict").code,
        "conflicting_payload"
    );
    let missing = build_request(r#"{"kind":"integer"}"#, 1, 1);
    assert_eq!(
        validate_build_request(missing).expect_err("missing").code,
        "missing_payload"
    );
    let mut identity = build_request(r#"{"kind":"atom","text":"x"}"#, 1, 1);
    identity.version = "a1".to_owned();
    assert_eq!(
        validate_build_request(identity).expect_err("identity").code,
        "unsupported_contract"
    );
}

#[test]
fn build_node_semantics_enforce_depth_count_and_integer_limits() {
    let depth = build_request(
        r#"{"kind":"list","children":[{"kind":"list","children":[]}]}"#,
        0,
        2,
    );
    assert_eq!(
        validate_build_request(depth).expect_err("depth").code,
        "resource_limit"
    );
    let count = build_request(
        r#"{"kind":"list","children":[{"kind":"atom","text":"a"}]}"#,
        1,
        1,
    );
    assert_eq!(
        validate_build_request(count).expect_err("count").code,
        "resource_limit"
    );
    let integer = build_request(
        r#"{"kind":"integer","integer":"9223372036854775808"}"#,
        1,
        1,
    );
    assert_eq!(
        validate_build_request(integer).expect_err("integer").code,
        "invalid_integer"
    );
}

#[test]
fn javascript_safe_integer_accepts_exact_boundaries_and_rejects_neighbors() {
    for value in [JAVASCRIPT_SAFE_INTEGER_MIN, JAVASCRIPT_SAFE_INTEGER_MAX] {
        let safe = JavaScriptSafeInteger::try_from(value).expect("safe boundary");
        assert_eq!(safe.get(), value);
        let json = serde_json::to_string(&safe).expect("safe integer JSON");
        let decoded: JavaScriptSafeInteger =
            serde_json::from_str(&json).expect("safe boundary JSON");
        assert_eq!(decoded, safe);
    }

    for value in [
        JAVASCRIPT_SAFE_INTEGER_MIN - 1,
        JAVASCRIPT_SAFE_INTEGER_MAX + 1,
    ] {
        assert!(JavaScriptSafeInteger::try_from(value).is_err());
        assert!(serde_json::from_str::<JavaScriptSafeInteger>(&value.to_string()).is_err());
    }
}

#[test]
fn every_plotter_safe_integer_field_rejects_precision_losing_values() {
    let base = serde_json::json!({
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "MOD",
        "total_operations": 1,
        "records": [{
            "uuid": "",
            "kind": "footprint",
            "object_id": "Demo",
            "operation_count": 1,
            "operations": [{
                "kind": "ThickSegment",
                "index": 0,
                "start_x": 0,
                "start_y": 0,
                "end_x": 0,
                "end_y": 0,
                "width_nm": 0,
                "layer": "F.SilkS"
            }],
            "name": "Demo",
            "layer": "F.Cu",
            "locked": false,
            "placed": false,
            "descr": "",
            "tags": "",
            "attr": []
        }],
        "document_id": "Demo",
        "coordinate_space": {"unit": "nm", "y_axis": "down"},
        "version": 0,
        "generator": "pcbnew",
        "generator_version": "10.0"
    });
    let paths = [
        "/version",
        "/records/0/operations/0/start_x",
        "/records/0/operations/0/start_y",
        "/records/0/operations/0/end_x",
        "/records/0/operations/0/end_y",
        "/records/0/operations/0/width_nm",
    ];

    for path in paths {
        for value in [JAVASCRIPT_SAFE_INTEGER_MIN, JAVASCRIPT_SAFE_INTEGER_MAX] {
            let mut document = base.clone();
            *document.pointer_mut(path).expect("safe integer field") = value.into();
            serde_json::from_value::<FootprintPlotDocumentA0>(document)
                .expect("safe boundary document");
        }
        for value in [
            JAVASCRIPT_SAFE_INTEGER_MIN - 1,
            JAVASCRIPT_SAFE_INTEGER_MAX + 1,
        ] {
            let mut document = base.clone();
            *document.pointer_mut(path).expect("safe integer field") = value.into();
            assert!(serde_json::from_value::<FootprintPlotDocumentA0>(document).is_err());
        }
    }
}

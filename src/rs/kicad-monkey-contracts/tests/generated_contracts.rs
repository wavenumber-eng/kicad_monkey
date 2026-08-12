use kicad_monkey_contracts::generated::build_request::SExpressionBuildRequestA0;
use kicad_monkey_contracts::generated::scan_request::SExpressionScanRequestA0;
use kicad_monkey_contracts::{ValidatedNode, validate_build_request};

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

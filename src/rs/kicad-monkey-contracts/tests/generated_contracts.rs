use kicad_monkey_contracts::generated::scan_request::SExpressionScanRequestA0;

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

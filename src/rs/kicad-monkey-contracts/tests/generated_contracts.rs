use kicad_monkey_contracts::generated::board_plot_document::BoardPlotDocumentA0;
use kicad_monkey_contracts::generated::build_request::SExpressionBuildRequestA0;
use kicad_monkey_contracts::generated::footprint_plot_document::FootprintPlotDocumentA0;
use kicad_monkey_contracts::generated::scan_request::SExpressionScanRequestA0;
use kicad_monkey_contracts::generated::symbol_plot_document::SymbolPlotDocumentA0;
use kicad_monkey_contracts::{
    JAVASCRIPT_SAFE_INTEGER_MAX, JAVASCRIPT_SAFE_INTEGER_MIN, JavaScriptSafeInteger, ValidatedNode,
    decode_compiled_schematic_graph_a0, decode_source_bundle_manifest_a0,
    validate_board_plot_document, validate_build_request, validate_footprint_plot_document,
    validate_symbol_plot_document,
};

#[test]
fn source_bundle_integer_transport_matches_shared_boundaries_and_failures() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/source_bundle_a0_vectors.json"
    ))
    .expect("source bundle vectors");
    for case in vectors["transport_cases"]
        .as_array()
        .expect("transport cases")
    {
        let mut candidate = vectors["manifest"].clone();
        candidate["sources"][0][case["field"].as_str().expect("field")] = case["value"].clone();
        let result = decode_source_bundle_manifest_a0(
            &serde_json::to_vec(&candidate).expect("candidate JSON"),
        );
        assert_eq!(result.is_ok(), case["valid"], "{}", case["id"]);
    }
}

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

#[test]
fn compiled_schematic_graph_vector_decodes_strictly_in_rust() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/compiled_schematic_graph_a0_vectors.json"
    ))
    .expect("compiled graph vectors");
    let graph = vectors["graph"].clone();
    let encoded = serde_json::to_vec(&graph).expect("compiled graph vector JSON");
    let decoded = decode_compiled_schematic_graph_a0(&encoded).expect("strict compiled graph");
    assert_eq!(
        decoded.identity_namespace,
        "sch.compiled_schematic_graph.a0"
    );
    assert_eq!(decoded.graphical_artifact_links.len(), 1);
    assert_eq!(
        serde_json::to_value(&decoded).expect("compiled graph re-encode"),
        graph
    );
    let allocations = vectors["identity"]["allocations"]
        .as_array()
        .expect("identity allocations");
    assert_eq!(allocations.len(), 10);
    let supporting = vectors["identity"]["supporting_allocations"]
        .as_array()
        .expect("supporting allocations");
    for allocation in allocations.iter().chain(supporting) {
        let expected = allocation["expected"].as_str().expect("expected UUID");
        assert_uuid_v7(expected);
        let collection = allocation["graph_collection"]
            .as_str()
            .expect("graph collection");
        let graph_index = allocation["graph_index"].as_u64().unwrap_or(0) as usize;
        assert_eq!(vectors["graph"][collection][graph_index]["id"], expected);
    }

    let mut invalid = graph;
    invalid["unknown_field"] = true.into();
    let encoded = serde_json::to_vec(&invalid).expect("invalid graph JSON");
    assert!(decode_compiled_schematic_graph_a0(&encoded).is_err());

    let mut invalid_role = vectors["graph"].clone();
    invalid_role["terminal_occurrences"][0]["role"] = "invented_role".into();
    let encoded = serde_json::to_vec(&invalid_role).expect("invalid role JSON");
    assert!(decode_compiled_schematic_graph_a0(&encoded).is_err());
}

fn assert_uuid_v7(value: &str) {
    let bytes = value.as_bytes();
    assert_eq!(bytes.len(), 36);
    assert_eq!(bytes[14], b'7', "UUID version: {value}");
    assert!(
        matches!(bytes[19], b'8' | b'9' | b'a' | b'b'),
        "UUID variant: {value}"
    );
}

#[test]
fn strict_compiled_graph_boundary_rejects_every_literal_mismatch() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/compiled_schematic_graph_a0_vectors.json"
    ))
    .expect("compiled graph vectors");
    let paths = [
        "/schema",
        "/type",
        "/identity_namespace",
        "/unit_definitions/0/type",
        "/page_definitions/0/type",
        "/unit_occurrences/0/type",
        "/page_occurrences/0/type",
        "/hierarchy_occurrences/0/type",
        "/component_occurrences/0/type",
        "/local_net_occurrences/0/type",
        "/terminal_occurrences/0/type",
        "/hierarchy_terminal_bindings/0/type",
        "/graphical_artifact_links/0/type",
        "/graphical_artifact_links/0/artifact_key",
    ];
    for path in paths {
        let mut graph = vectors["graph"].clone();
        *graph.pointer_mut(path).expect("registered literal path") = "wrong.a1".into();
        let encoded = serde_json::to_vec(&graph).expect("invalid literal JSON");
        let error = decode_compiled_schematic_graph_a0(&encoded).expect_err(path);
        assert!(error.to_string().contains("unsupported_contract"), "{path}");
    }
}

#[test]
fn symbol_plotter_contract_enforces_record_and_domain_semantics() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/symbol_plotter_a0_vectors.json"
    ))
    .expect("symbol vectors");
    let expected = vectors["vectors"][0]["expected"].clone();
    let document: SymbolPlotDocumentA0 =
        serde_json::from_value(expected.clone()).expect("symbol transport document");
    validate_symbol_plot_document(&document).expect("valid symbol semantics");

    let mut missing_header = expected.clone();
    missing_header["records"]
        .as_array_mut()
        .expect("records")
        .remove(0);
    let document: SymbolPlotDocumentA0 =
        serde_json::from_value(missing_header).expect("structurally valid missing header");
    assert_eq!(
        validate_symbol_plot_document(&document)
            .expect_err("header required")
            .code,
        "missing_symbol_header"
    );

    let mut layered = expected;
    layered["records"][1]["operations"][0]["layer"] = serde_json::json!("F.SilkS");
    let document: SymbolPlotDocumentA0 =
        serde_json::from_value(layered).expect("structurally valid layered symbol");
    assert_eq!(
        validate_symbol_plot_document(&document)
            .expect_err("symbol body must be layer-free")
            .code,
        "invalid_symbol_operation"
    );
}

#[test]
fn board_plotter_contract_enforces_layerless_graphic_state_and_counts() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/board_plotter_a0_vectors.json"
    ))
    .expect("board vectors");
    let expected = vectors["vectors"][0]["expected"].clone();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(expected.clone()).expect("board transport document");
    validate_board_plot_document(&document).expect("valid board semantics");

    for (path, replacement) in [
        ("schema", serde_json::json!("wrong")),
        ("source_kind", serde_json::json!("MOD")),
    ] {
        let mut identity = expected.clone();
        identity[path] = replacement;
        let document: BoardPlotDocumentA0 = serde_json::from_value(identity).expect("shape");
        assert_eq!(
            validate_board_plot_document(&document)
                .expect_err("document identity is semantic")
                .code,
            "unsupported_contract"
        );
    }

    let mut layered = expected.clone();
    layered["records"][0]["operations"][0]["layer"] = serde_json::json!("Edge.Cuts");
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(layered).expect("structurally valid layered graphic");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("board graphic operations must stay layerless")
            .code,
        "invalid_board_operation"
    );

    let mut flashed = expected.clone();
    flashed["records"][0]["operations"][0] = serde_json::json!({
        "kind": "FlashPadCircle",
        "index": 0,
        "x": 0,
        "y": 0,
        "diameter_nm": 1_000_000,
        "mask_margin_nm": 0,
        "layers": ["F.Cu"]
    });
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(flashed).expect("structurally valid flash payload");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("pad flashes belong to later slices")
            .code,
        "invalid_board_operation"
    );

    let mut miscounted_record = expected.clone();
    miscounted_record["records"][0]["operation_count"] = serde_json::json!(2);
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(miscounted_record).expect("structurally valid miscount");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("record counts must match")
            .code,
        "operation_count_mismatch"
    );

    let mut miscounted_document = expected;
    miscounted_document["total_operations"] = serde_json::json!(0);
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(miscounted_document).expect("structurally valid total miscount");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("document totals must match")
            .code,
        "operation_count_mismatch"
    );
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single semantics test intentionally walks every track and via rejection"
)]
fn board_plotter_contract_enforces_track_and_via_record_states() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    let tracks = vectors["vectors"][4]["expected"].clone();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(tracks.clone()).expect("track transport document");
    validate_board_plot_document(&document).expect("valid track semantics");
    let vias = vectors["vectors"][5]["expected"].clone();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(vias).expect("via transport document");
    validate_board_plot_document(&document).expect("valid via semantics");

    // Vector 4 record layout: gr_line, three segments, two arcs, one via.
    let mut layered_segment = tracks.clone();
    layered_segment["records"][1]["operations"][0]["layer"] = serde_json::json!("F.Cu");
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(layered_segment).expect("structurally valid layered segment");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("segment operations must stay layerless")
            .code,
        "invalid_board_operation"
    );

    let mut layered_arc = tracks.clone();
    layered_arc["records"][4]["operations"][0]["layer"] = serde_json::json!("F.Cu");
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(layered_arc).expect("structurally valid layered arc");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("track arc operations must stay layerless")
            .code,
        "invalid_board_operation"
    );

    let mut misordered_via = tracks.clone();
    misordered_via["records"][6]["operations"][0]["role"] = serde_json::json!("via_mask_opening");
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(misordered_via).expect("structurally valid misordered via");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("via records lead with the aperture flash")
            .code,
        "invalid_board_operation"
    );

    let mut odd_via = tracks.clone();
    odd_via["records"][6]["operations"]
        .as_array_mut()
        .expect("via operations")
        .pop();
    odd_via["records"][6]["operation_count"] = serde_json::json!(3);
    odd_via["total_operations"] = serde_json::json!(9);
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(odd_via).expect("structurally valid odd via");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("mask openings and drills arrive in pairs")
            .code,
        "invalid_board_operation"
    );

    let mut margined_drill = tracks;
    margined_drill["records"][6]["operations"][1]["mask_margin_nm"] = serde_json::json!(0);
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(margined_drill).expect("structurally valid margined drill");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("via drills reject pad-only fields")
            .code,
        "invalid_board_operation"
    );
}

#[test]
fn board_plotter_contract_enforces_zone_fill_ring_states() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    let zones = vectors["vectors"][6]["expected"].clone();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(zones.clone()).expect("zone transport document");
    validate_board_plot_document(&document).expect("valid zone semantics");
    let net_classes = vectors["vectors"][7]["expected"].clone();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(net_classes).expect("net-class transport document");
    validate_board_plot_document(&document).expect("valid net-class semantics");

    // Vector 6 record layout: zone-single (two rings), zone-multi,
    // zone-keepout (no rings), zone-empty-name.
    let mut mismatched = zones.clone();
    mismatched["records"][0]["fill_layers"]
        .as_array_mut()
        .expect("fill layers")
        .pop();
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(mismatched).expect("structurally valid mismatched annotations");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("ring annotations must match the operation count")
            .code,
        "invalid_board_operation"
    );

    let mut layered = zones.clone();
    layered["records"][0]["operations"][0]["layer"] = serde_json::json!("F.Cu");
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(layered).expect("structurally valid layered ring");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("zone fill operations must stay layerless")
            .code,
        "invalid_board_operation"
    );

    let mut stroked = zones;
    stroked["records"][0]["operations"][0] = serde_json::json!({
        "kind": "ThickSegment",
        "index": 0,
        "start_x": 0,
        "start_y": 0,
        "end_x": 1,
        "end_y": 0,
        "width_nm": 0
    });
    let document: BoardPlotDocumentA0 =
        serde_json::from_value(stroked).expect("structurally valid stroked zone");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("zone fills carry only filled polygons")
            .code,
        "invalid_board_operation"
    );
}

#[test]
fn board_text_contract_rejects_redundant_field_and_cache_drift() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    let text = vectors["vectors"][8]["expected"].clone();

    let mut empty_with_text = text.clone();
    empty_with_text["records"][2]["text"] = serde_json::json!("drift");
    let document: BoardPlotDocumentA0 = serde_json::from_value(empty_with_text).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("zero-op text must be empty")
            .code,
        "invalid_board_operation"
    );

    let mutation_paths = [
        ("text", serde_json::json!("wrong")),
        ("angle", serde_json::json!(44.0)),
        ("schema", serde_json::json!("wrong")),
        ("unit", serde_json::json!("mm")),
        ("coordinate_space", serde_json::json!("local")),
        ("source", serde_json::json!("generated")),
    ];
    for (field, replacement) in mutation_paths {
        let mut drift = text.clone();
        drift["records"][4]["operations"][0]["render_cache"][field] = replacement;
        let document: BoardPlotDocumentA0 = serde_json::from_value(drift).expect("shape");
        assert_eq!(
            validate_board_plot_document(&document)
                .expect_err("cache redundancy must be exact")
                .code,
            "invalid_board_operation",
            "{field}"
        );
    }

    let mut exterior = text.clone();
    exterior["records"][4]["operations"][0]["render_cache_polygons"][0][0][0] =
        serde_json::json!(123);
    let document: BoardPlotDocumentA0 = serde_json::from_value(exterior).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("cache exterior mirror must match")
            .code,
        "invalid_board_operation"
    );

    let mut knockout = text;
    knockout["records"][6]["operations"][0]["render_cache"]
        .as_object_mut()
        .expect("cache")
        .remove("knockout");
    let document: BoardPlotDocumentA0 = serde_json::from_value(knockout).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("knockout markers are bidirectional")
            .code,
        "invalid_board_operation"
    );
}

#[test]
fn board_text_box_record_text_must_match_its_text_operation() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_plotter_a0_vectors.json"
    )))
    .expect("board vectors");
    let mut text_boxes = vectors["vectors"][9]["expected"].clone();
    text_boxes["records"][3]["text"] = serde_json::json!("wrong");
    let document: BoardPlotDocumentA0 = serde_json::from_value(text_boxes).expect("shape");
    assert_eq!(
        validate_board_plot_document(&document)
            .expect_err("record text must match")
            .code,
        "invalid_board_operation"
    );
}

#[test]
fn footprint_flash_circles_require_pad_state_not_via_state() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("footprint vectors");
    let pads = vectors["vectors"][2]["expected"].clone();
    let document: FootprintPlotDocumentA0 =
        serde_json::from_value(pads.clone()).expect("pad transport document");
    validate_footprint_plot_document(&document).expect("valid pad semantics");

    let mut via_role = pads.clone();
    via_role["records"][0]["operations"][0]["role"] = serde_json::json!("via_aperture");
    let document: FootprintPlotDocumentA0 =
        serde_json::from_value(via_role).expect("structurally valid via-role flash");
    assert_eq!(
        validate_footprint_plot_document(&document)
            .expect_err("footprint flashes reject via roles")
            .code,
        "invalid_pad_operation"
    );

    let mut unmargined = pads;
    unmargined["records"][0]["operations"][0]
        .as_object_mut()
        .expect("flash operation")
        .remove("mask_margin_nm");
    let document: FootprintPlotDocumentA0 =
        serde_json::from_value(unmargined).expect("structurally valid unmargined flash");
    assert_eq!(
        validate_footprint_plot_document(&document)
            .expect_err("footprint flashes require mask_margin_nm")
            .code,
        "invalid_pad_operation"
    );
}

#[test]
fn footprint_semantics_reject_board_text_operations_from_the_shared_union() {
    let footprint_vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("footprint vectors");
    let board_vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/board_plotter_a0_vectors.json"
    ))
    .expect("board vectors");
    let mut footprint = footprint_vectors["vectors"][0]["expected"].clone();
    footprint["records"][0]["operations"][0] =
        board_vectors["vectors"][8]["expected"]["records"][0]["operations"][0].clone();
    let document: FootprintPlotDocumentA0 = serde_json::from_value(footprint).expect("shape");
    assert_eq!(
        validate_footprint_plot_document(&document)
            .expect_err("board text is outside footprint semantics")
            .code,
        "invalid_footprint_operation"
    );
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
#[allow(
    clippy::too_many_lines,
    reason = "boundary matrix stays below the reviewed 150-line test-function limit"
)]
fn every_plotter_safe_integer_field_rejects_precision_losing_values() {
    let base = serde_json::json!({
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "MOD",
        "total_operations": 5,
        "records": [{
            "uuid": "",
            "kind": "footprint",
            "object_id": "Demo",
            "operation_count": 5,
            "operations": [{
                "kind": "ThickSegment",
                "index": 0,
                "start_x": 0,
                "start_y": 0,
                "end_x": 0,
                "end_y": 0,
                "width_nm": 0,
                "layer": "F.SilkS"
            }, {
                "kind": "ArcThreePoint",
                "index": 1,
                "start_x": 0,
                "start_y": 0,
                "mid_x": 0,
                "mid_y": 0,
                "end_x": 0,
                "end_y": 0,
                "fill": "NO_FILL",
                "width_nm": 0,
                "layer": "F.Fab"
            }, {
                "kind": "Circle",
                "index": 2,
                "cx": 0,
                "cy": 0,
                "diameter_nm": 0,
                "fill": "FILLED_SHAPE",
                "width_nm": 0,
                "layer": "F.SilkS"
            }, {
                "kind": "Rect",
                "index": 3,
                "x1": 0,
                "y1": 0,
                "x2": 0,
                "y2": 0,
                "fill": "NO_FILL",
                "width_nm": 0,
                "corner_radius_nm": 0,
                "layer": "F.CrtYd"
            }, {
                "kind": "PlotPoly",
                "index": 4,
                "points": [[0, 0]],
                "fill": "FILLED_SHAPE",
                "width_nm": 0,
                "layer": "F.Cu"
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
        "/records/0/operations/1/start_x",
        "/records/0/operations/1/start_y",
        "/records/0/operations/1/mid_x",
        "/records/0/operations/1/mid_y",
        "/records/0/operations/1/end_x",
        "/records/0/operations/1/end_y",
        "/records/0/operations/1/width_nm",
        "/records/0/operations/2/cx",
        "/records/0/operations/2/cy",
        "/records/0/operations/2/diameter_nm",
        "/records/0/operations/2/width_nm",
        "/records/0/operations/3/x1",
        "/records/0/operations/3/y1",
        "/records/0/operations/3/x2",
        "/records/0/operations/3/y2",
        "/records/0/operations/3/width_nm",
        "/records/0/operations/3/corner_radius_nm",
        "/records/0/operations/4/points/0/0",
        "/records/0/operations/4/points/0/1",
        "/records/0/operations/4/width_nm",
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

#[test]
fn plotter_operation_union_preserves_exact_polygon_points() {
    let valid = serde_json::json!({
        "schema": "kicad.plotter_ir.a0",
        "source_kind": "MOD",
        "total_operations": 1,
        "records": [{
            "uuid": "",
            "kind": "footprint",
            "object_id": "Demo",
            "operation_count": 1,
            "operations": [{
                "kind": "PlotPoly",
                "index": 0,
                "points": [[0, 1]],
                "fill": "NO_FILL",
                "width_nm": 100000,
                "layer": "F.Cu"
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
        "version": 20260206,
        "generator": "pcbnew",
        "generator_version": "10.0"
    });
    serde_json::from_value::<FootprintPlotDocumentA0>(valid.clone())
        .expect("exact two-value polygon point");

    let mut short_point = valid.clone();
    *short_point
        .pointer_mut("/records/0/operations/0/points/0")
        .expect("point") = serde_json::json!([0]);
    assert!(serde_json::from_value::<FootprintPlotDocumentA0>(short_point).is_err());
}

#[test]
fn every_promoted_pad_integer_field_enforces_javascript_precision() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("shared plotter vectors");
    let mut document = vectors["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .find(|vector| vector["id"] == "standard-pad-flashes-and-drills")
        .expect("pad vector")["expected"]
        .clone();
    document["records"][0]["operations"][9]["mask_margin_nm"] = 0.into();
    document["records"][0]["operations"][9]["pad_size_x_nm"] = 0.into();
    document["records"][0]["operations"][9]["pad_size_y_nm"] = 0.into();

    let paths = [
        "/records/0/operations/0/x",
        "/records/0/operations/0/y",
        "/records/0/operations/0/diameter_nm",
        "/records/0/operations/0/mask_margin_nm",
        "/records/0/operations/1/x",
        "/records/0/operations/1/y",
        "/records/0/operations/1/size_x_nm",
        "/records/0/operations/1/size_y_nm",
        "/records/0/operations/1/mask_margin_nm",
        "/records/0/operations/2/x",
        "/records/0/operations/2/y",
        "/records/0/operations/2/size_x_nm",
        "/records/0/operations/2/size_y_nm",
        "/records/0/operations/2/mask_margin_nm",
        "/records/0/operations/3/x",
        "/records/0/operations/3/y",
        "/records/0/operations/3/size_x_nm",
        "/records/0/operations/3/size_y_nm",
        "/records/0/operations/3/corner_radius_nm",
        "/records/0/operations/3/mask_margin_nm",
        "/records/0/operations/4/x",
        "/records/0/operations/4/y",
        "/records/0/operations/4/corners/0/0",
        "/records/0/operations/4/corners/0/1",
        "/records/0/operations/4/corners/1/0",
        "/records/0/operations/4/corners/1/1",
        "/records/0/operations/4/corners/2/0",
        "/records/0/operations/4/corners/2/1",
        "/records/0/operations/4/corners/3/0",
        "/records/0/operations/4/corners/3/1",
        "/records/0/operations/4/mask_margin_nm",
        "/records/0/operations/7/mask_margin_nm",
        "/records/0/operations/7/pad_size_x_nm",
        "/records/0/operations/7/pad_size_y_nm",
        "/records/0/operations/9/mask_margin_nm",
        "/records/0/operations/9/pad_size_x_nm",
        "/records/0/operations/9/pad_size_y_nm",
    ];
    for path in paths {
        for value in [JAVASCRIPT_SAFE_INTEGER_MIN, JAVASCRIPT_SAFE_INTEGER_MAX] {
            let mut candidate = document.clone();
            *candidate.pointer_mut(path).expect("pad integer field") = value.into();
            serde_json::from_value::<FootprintPlotDocumentA0>(candidate)
                .expect("safe pad boundary");
        }
        for value in [
            JAVASCRIPT_SAFE_INTEGER_MIN - 1,
            JAVASCRIPT_SAFE_INTEGER_MAX + 1,
        ] {
            let mut candidate = document.clone();
            *candidate.pointer_mut(path).expect("pad integer field") = value.into();
            assert!(serde_json::from_value::<FootprintPlotDocumentA0>(candidate).is_err());
        }
    }
}

#[test]
fn custom_pad_integer_fields_enforce_javascript_precision() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("shared plotter vectors");
    let document = vectors["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .find(|vector| vector["id"] == "custom-and-chamfered-pad-flashes")
        .expect("custom pad vector")["expected"]
        .clone();
    let paths = [
        "/records/0/operations/0/x",
        "/records/0/operations/0/y",
        "/records/0/operations/0/size_x_nm",
        "/records/0/operations/0/size_y_nm",
        "/records/0/operations/0/polygons/0/0/0",
        "/records/0/operations/0/polygons/0/0/1",
        "/records/0/operations/0/polygon_widths_nm/0",
        "/records/0/operations/0/mask_margin_nm",
        "/records/0/operations/1/x",
        "/records/0/operations/1/y",
        "/records/0/operations/1/size_x_nm",
        "/records/0/operations/1/size_y_nm",
        "/records/0/operations/1/polygons/0/0/0",
        "/records/0/operations/1/polygons/0/0/1",
        "/records/0/operations/1/mask_margin_nm",
    ];
    for path in paths {
        for value in [JAVASCRIPT_SAFE_INTEGER_MIN, JAVASCRIPT_SAFE_INTEGER_MAX] {
            let mut candidate = document.clone();
            *candidate.pointer_mut(path).expect("custom integer field") = value.into();
            serde_json::from_value::<FootprintPlotDocumentA0>(candidate)
                .expect("safe custom boundary");
        }
        for value in [
            JAVASCRIPT_SAFE_INTEGER_MIN - 1,
            JAVASCRIPT_SAFE_INTEGER_MAX + 1,
        ] {
            let mut candidate = document.clone();
            *candidate.pointer_mut(path).expect("custom integer field") = value.into();
            assert!(serde_json::from_value::<FootprintPlotDocumentA0>(candidate).is_err());
        }
    }
}

#[test]
fn shared_circle_and_segment_semantics_reject_contradictory_states() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("shared plotter vectors");
    let valid = vectors["vectors"][1]["expected"].clone();
    let document: FootprintPlotDocumentA0 =
        serde_json::from_value(valid.clone()).expect("structural transport document");
    validate_footprint_plot_document(&document).expect("valid graphical semantics");

    let cases = [
        ("missing layer", serde_json::json!(null), None, None),
        (
            "graphic with drill fields",
            serde_json::json!("F.SilkS"),
            Some(serde_json::json!(["F.Cu"])),
            Some(serde_json::json!(0)),
        ),
    ];
    for (name, layer, layers, mask) in cases {
        let mut malformed = valid.clone();
        let operation = &mut malformed["records"][0]["operations"][0];
        if layer.is_null() {
            operation
                .as_object_mut()
                .expect("operation")
                .remove("layer");
        } else {
            operation["layer"] = layer;
        }
        if let Some(layers) = layers {
            operation["layers"] = layers;
        }
        if let Some(mask) = mask {
            operation["mask_margin_nm"] = mask;
        }
        let document: FootprintPlotDocumentA0 =
            serde_json::from_value(malformed).expect("structurally valid malformed document");
        let error = validate_footprint_plot_document(&document).expect_err(name);
        assert_eq!(error.code, "conflicting_plotter_fields", "{name}");
    }

    let arbitrary_role = serde_json::json!({
        "kind": "ThickSegment", "index": 0,
        "start_x": 0, "start_y": 0, "end_x": 1, "end_y": 1, "width_nm": 1,
        "role": "arbitrary", "layers": ["F.Cu"]
    });
    assert!(
        serde_json::from_value::<
            kicad_monkey_contracts::generated::footprint_plot_document::PlotterOperation,
        >(arbitrary_role)
        .is_err()
    );

    let custom = vectors["vectors"][3]["expected"].clone();
    let custom_document: FootprintPlotDocumentA0 =
        serde_json::from_value(custom.clone()).expect("custom transport document");
    validate_footprint_plot_document(&custom_document).expect("valid custom pad semantics");

    let mut mismatched_widths = custom;
    mismatched_widths["records"][0]["operations"][0]["polygon_widths_nm"] =
        serde_json::json!([50_000]);
    let mismatched_document: FootprintPlotDocumentA0 =
        serde_json::from_value(mismatched_widths).expect("width mismatch is structurally valid");
    let error = validate_footprint_plot_document(&mismatched_document)
        .expect_err("custom widths must align with polygons");
    assert_eq!(error.code, "polygon_width_count_mismatch");

    let mut missing_layers = vectors["vectors"][3]["expected"].clone();
    missing_layers["records"][0]["operations"][0]["layers"] = serde_json::json!([]);
    let missing_layers_document: FootprintPlotDocumentA0 =
        serde_json::from_value(missing_layers).expect("empty layers are structurally valid");
    let error = validate_footprint_plot_document(&missing_layers_document)
        .expect_err("custom pads require layers");
    assert_eq!(error.code, "missing_layers");
}

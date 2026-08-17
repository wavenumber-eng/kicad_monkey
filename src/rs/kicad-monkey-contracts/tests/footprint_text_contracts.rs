use kicad_monkey_contracts::generated::footprint_plot_document::FootprintPlotDocumentA0;
use kicad_monkey_contracts::{
    JAVASCRIPT_SAFE_INTEGER_MAX, JAVASCRIPT_SAFE_INTEGER_MIN, validate_footprint_plot_document,
};

fn text_vector_document() -> serde_json::Value {
    let footprint_vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/parity/footprint_plotter_a0_vectors.json"
    ))
    .expect("footprint vectors");
    footprint_vectors["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .find(|vector| vector["id"] == "standalone-properties-text-and-text-box")
        .expect("text vector")["expected"]
        .clone()
}

#[test]
fn footprint_semantics_accept_standalone_text_and_reject_noncanonical_states() {
    let valid = text_vector_document();
    let document: FootprintPlotDocumentA0 =
        serde_json::from_value(valid.clone()).expect("text shape");
    validate_footprint_plot_document(&document).expect("canonical standalone footprint text");
    let mut mutations = Vec::new();

    let mut wrong_document = valid.clone();
    wrong_document["schema"] = "wrong".into();
    mutations.push((
        "document identity",
        wrong_document,
        "invalid_footprint_document",
    ));

    let mut wrong_record = valid.clone();
    wrong_record["records"][0]["object_id"] = "wrong".into();
    mutations.push(("record identity", wrong_record, "invalid_footprint_record"));

    let mut wrong_index = valid.clone();
    wrong_index["records"][0]["operations"][0]["index"] = 7.into();
    mutations.push(("operation index", wrong_index, "operation_index_mismatch"));

    let mut missing_layer = valid.clone();
    missing_layer["records"][0]["operations"][0]
        .as_object_mut()
        .expect("operation")
        .remove("layer");
    mutations.push(("missing layer", missing_layer, "missing_layer"));

    for (field, value) in [
        ("mirror", serde_json::json!(true)),
        ("text_as_polygons", serde_json::json!(true)),
        ("polyline_per_segment", serde_json::json!(true)),
        ("knockout", serde_json::json!(true)),
        ("render_cache_exact", serde_json::json!(false)),
        (
            "render_cache_source",
            serde_json::json!("existing_file_cache"),
        ),
        (
            "render_cache",
            serde_json::json!({
                "schema": "kicad.render_cache.v1",
                "unit": "nm",
                "coordinate_space": "board",
                "text": "R${Value}",
                "angle": 30.0,
                "source": "existing_file_cache",
                "exact": false,
                "polygons": []
            }),
        ),
    ] {
        let mut mutation = valid.clone();
        mutation["records"][0]["operations"][0][field] = value;
        mutations.push((field, mutation, "invalid_footprint_text"));
    }

    for (name, mutation, expected_code) in mutations {
        let document: FootprintPlotDocumentA0 =
            serde_json::from_value(mutation).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            validate_footprint_plot_document(&document)
                .unwrap_err_or_else(name)
                .code,
            expected_code,
            "{name}"
        );
    }

    for polygons in [
        serde_json::json!([]),
        serde_json::json!([[[0, 0], [1, 0], [0, 1]]]),
    ] {
        let mut present_cache_polygons = valid.clone();
        present_cache_polygons["records"][0]["operations"][0]["render_cache_polygons"] = polygons;
        assert!(
            serde_json::from_value::<FootprintPlotDocumentA0>(present_cache_polygons).is_err(),
            "present render_cache_polygons must not normalize to absence"
        );
    }

    let mut wrong_kind = valid;
    wrong_kind["records"][0]["operations"][7]["kind"] = "WrongSegment".into();
    assert!(
        serde_json::from_value::<FootprintPlotDocumentA0>(wrong_kind).is_err(),
        "operation kind must select its exact structural variant"
    );
}

#[test]
fn footprint_text_safe_integer_fields_reject_precision_loss() {
    let valid = text_vector_document();
    for field in ["x", "y", "size_x_nm", "size_y_nm", "pen_width_nm"] {
        for value in [JAVASCRIPT_SAFE_INTEGER_MIN, JAVASCRIPT_SAFE_INTEGER_MAX] {
            let mut document = valid.clone();
            document["records"][0]["operations"][0][field] = value.into();
            serde_json::from_value::<FootprintPlotDocumentA0>(document)
                .unwrap_or_else(|error| panic!("{field} rejected safe boundary: {error}"));
        }
        for value in [
            JAVASCRIPT_SAFE_INTEGER_MIN - 1,
            JAVASCRIPT_SAFE_INTEGER_MAX + 1,
        ] {
            let mut document = valid.clone();
            document["records"][0]["operations"][0][field] = value.into();
            assert!(
                serde_json::from_value::<FootprintPlotDocumentA0>(document).is_err(),
                "{field} accepted precision-losing value"
            );
        }
    }
}

trait ValidationResultExt {
    fn unwrap_err_or_else(self, name: &str) -> kicad_monkey_contracts::ValidationError;
}

impl ValidationResultExt for Result<(), kicad_monkey_contracts::ValidationError> {
    fn unwrap_err_or_else(self, name: &str) -> kicad_monkey_contracts::ValidationError {
        match self {
            Err(error) => error,
            Ok(()) => panic!("{name} unexpectedly passed semantic validation"),
        }
    }
}

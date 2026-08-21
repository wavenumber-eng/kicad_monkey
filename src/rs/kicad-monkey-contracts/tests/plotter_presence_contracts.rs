use kicad_monkey_contracts::generated::board_plot_request::BoardPlotRequestA0;
use kicad_monkey_contracts::generated::board_plot_result::BoardPlotResultA0;
use kicad_monkey_contracts::generated::footprint_plot_request::FootprintPlotRequestA0;
use kicad_monkey_contracts::generated::footprint_plot_result::FootprintPlotResultA0;
use kicad_monkey_contracts::generated::schematic_plot_result::SchematicPlotResultA0;
use kicad_monkey_contracts::generated::symbol_plot_request::SymbolPlotRequestA0;
use kicad_monkey_contracts::generated::symbol_plot_result::SymbolPlotResultA0;
use serde::de::DeserializeOwned;

fn assert_null_diagnostic_fields_rejected<T: DeserializeOwned>(type_name: &str) {
    for field in ["position", "token"] {
        let mut value = serde_json::json!({
            "type": type_name,
            "version": "a0",
            "output_bytes": "0",
            "total_operations": 0,
            "diagnostics": [{
                "phase": "build",
                "code": "test",
                "message": "test"
            }]
        });
        value["diagnostics"][0][field] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<T>(value).is_err(),
            "{type_name}.{field}"
        );
    }
}

#[test]
fn frozen_requests_reject_explicit_null_for_optional_identifiers() {
    let mut board = serde_json::json!({
        "type": "kicad_monkey.board_plot.request", "version": "a0",
        "max_source_bytes": "4096", "max_output_bytes": "4096",
        "max_depth": 32, "max_graphics": 32, "max_operations": 32,
        "max_points": 32, "max_text_bytes": "4096", "max_parse_nodes": 32,
        "max_input_points": 32, "max_input_polygons": 32,
        "max_cache_polygons": 32, "max_cache_contours": 32
    });
    board["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BoardPlotRequestA0>(board).is_err());

    let mut footprint = serde_json::json!({
        "type": "kicad_monkey.footprint_plot.request", "version": "a0",
        "max_source_bytes": "4096", "max_output_bytes": "4096",
        "max_depth": 32, "max_metadata_forms": 32,
        "max_operations": 32, "max_points": 32
    });
    footprint["document_id"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<FootprintPlotRequestA0>(footprint).is_err());

    let mut symbol = serde_json::json!({
        "type": "kicad_monkey.symbol_plot.request", "version": "a0",
        "symbol_name": "Demo", "style": 0,
        "max_source_bytes": "4096", "max_output_bytes": "4096",
        "max_depth": 32, "max_symbols": 32, "max_subsymbols": 32,
        "max_operations": 32, "max_points": 32
    });
    symbol["source_path"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SymbolPlotRequestA0>(symbol).is_err());
}

#[test]
fn frozen_results_reject_explicit_null_for_optional_diagnostic_fields() {
    assert_null_diagnostic_fields_rejected::<BoardPlotResultA0>("kicad_monkey.board_plot.result");
    assert_null_diagnostic_fields_rejected::<FootprintPlotResultA0>(
        "kicad_monkey.footprint_plot.result",
    );
    assert_null_diagnostic_fields_rejected::<SymbolPlotResultA0>("kicad_monkey.symbol_plot.result");
    assert_null_diagnostic_fields_rejected::<SchematicPlotResultA0>(
        "kicad_monkey.schematic_plot.result",
    );
}

use kicad_monkey_wasm::{edit_symbol_library_boolean, read_symbol_library};
use serde_json::Value;
use wasm_bindgen_test::wasm_bindgen_test;

const SOURCE: &[u8] = br#"(kicad_symbol_lib
  (symbol "Base" (in_bom yes) (on_board no)
    (symbol "Base_1_1" (pin input line (at 0 0 0))))
  (symbol "Derived" (extends "Base") (future_extension "keep")))"#;
const READ_REQUEST: &[u8] = br#"{"type":"kicad_monkey.symbol_library_read.request","version":"a0","max_source_bytes":"4096","max_depth":32,"max_symbols":10,"max_metadata_forms":20,"max_subsymbols":10,"max_pins":10}"#;
const EDIT_REQUEST: &[u8] = br#"{"type":"kicad_monkey.symbol_library_edit.request","version":"a0","symbol_name":"Derived","field":"in_bom","value":false,"max_source_bytes":"4096","max_output_bytes":"4096","max_depth":32,"max_symbols":10,"max_metadata_forms":20,"max_subsymbols":10,"max_pins":10}"#;

#[wasm_bindgen_test]
fn wasm_symbol_library_read_and_semantic_write_are_byte_oriented() {
    let result: Value = serde_json::from_slice(
        &read_symbol_library(SOURCE, READ_REQUEST).expect("WASM symbol read"),
    )
    .expect("result JSON");
    assert_eq!(result["diagnostics"], serde_json::json!([]));
    assert_eq!(result["symbols"][0]["pin_count"], 1);
    assert_eq!(result["symbols"][1]["extends"], "Base");

    let output =
        edit_symbol_library_boolean(SOURCE, EDIT_REQUEST).expect("WASM symbol edit operation");
    let metadata: Value =
        serde_json::from_slice(&output.result_json()).expect("edit metadata JSON");
    let edited = String::from_utf8(output.take_output_bytes()).expect("edited UTF-8");
    assert_eq!(metadata["changed"], true);
    assert!(edited.contains("(in_bom no)"));
    assert!(edited.contains("(future_extension \"keep\")"));

    let second = edit_symbol_library_boolean(edited.as_bytes(), EDIT_REQUEST)
        .expect("stable WASM second write");
    let second_metadata: Value =
        serde_json::from_slice(&second.result_json()).expect("second metadata JSON");
    assert_eq!(second_metadata["changed"], false);
    assert_eq!(second.take_output_bytes(), edited.as_bytes());
}

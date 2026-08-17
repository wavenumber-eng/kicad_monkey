use kicad_monkey_wasm::plot_footprint_ir;
use serde_json::Value;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn wasm_custom_geometry_point_limit_fails_without_partial_output() {
    let source = br#"(footprint "Custom"
      (pad "1" smd custom (at 0 0) (size 1 1) (layers "F.Cu")
        (primitives
          (gr_poly (pts (xy 0 0) (xy 1 0) (xy 0 1)) (fill yes)))))"#;
    let request = br#"{"type":"kicad_monkey.footprint_plot.request","version":"a0","max_source_bytes":"4096","max_output_bytes":"4096","max_depth":32,"max_metadata_forms":32,"max_text_carriers":32,"max_text_bytes":"4096","max_operations":8,"max_points":2}"#;
    let output = plot_footprint_ir(source, request).expect("structured point-limit result");
    let metadata: Value =
        serde_json::from_slice(&output.result_json()).expect("result metadata JSON");
    assert_eq!(metadata["diagnostics"][0]["code"], "resource_limit");
    assert!(
        metadata["diagnostics"][0]["message"]
            .as_str()
            .expect("message")
            .contains("max_points")
    );
    assert_eq!(metadata["total_operations"], 0);
    assert!(output.take_output_bytes().is_empty());
}

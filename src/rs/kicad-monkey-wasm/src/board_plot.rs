//! Browser adapter for the source-selected board plotter-IR producer.

use crate::serialize_bounded;
use kicad_monkey_contracts::generated::board_plot_request::BoardPlotRequestA0;
use kicad_monkey_contracts::generated::board_plot_result::{
    BoardPlotResultA0, Diagnostic, DiagnosticPhase, SourcePosition,
};
use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, Error, ErrorPhase,
    board_plot_document_with_sidecars, utf8_text,
};
use wasm_bindgen::prelude::*;

const MAX_BOARD_PLOT_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_BOARD_TEXT_VARIABLES: usize = 16_384;
const MAX_BOARD_TEXT_VARIABLE_SIDECAR_BYTES: usize = 512 * 1024;
const MAX_BOARD_RETAINED_NET_CLASS_BYTES: usize = 16 * 1024 * 1024;
const MAX_BOARD_RETAINED_METADATA_BYTES: usize = 16 * 1024 * 1024;

/// Paired metadata and out-of-band board plotter-IR JSON bytes.
#[wasm_bindgen]
pub struct BoardPlotOutput {
    result_json: Vec<u8>,
    output_bytes: Vec<u8>,
}

#[wasm_bindgen]
impl BoardPlotOutput {
    /// TypeSpec-governed `BoardPlotResultA0` metadata bytes.
    #[wasm_bindgen(js_name = resultJson)]
    pub fn result_json(&self) -> Vec<u8> {
        self.result_json.clone()
    }

    /// `kicad.plotter_ir.a0` JSON bytes; empty when diagnostics are present.
    #[wasm_bindgen(js_name = outputBytes)]
    pub fn output_bytes(&self) -> Vec<u8> {
        self.output_bytes.clone()
    }

    /// Consume this result and transfer the plotter-IR bytes exactly once.
    #[wasm_bindgen(js_name = takeOutputBytes)]
    pub fn take_output_bytes(self) -> Vec<u8> {
        self.output_bytes
    }
}

/// Convert promoted board gr_* graphics to plotter-IR JSON bytes.
#[wasm_bindgen(js_name = plotBoardIr)]
pub fn plot_board_ir(source: &[u8], request_json: &[u8]) -> Result<BoardPlotOutput, JsValue> {
    plot_board_ir_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

fn plot_board_ir_impl(source: &[u8], request_json: &[u8]) -> Result<BoardPlotOutput, String> {
    if request_json.len() > MAX_BOARD_PLOT_REQUEST_BYTES {
        return Err("board plot request exceeds the fixed 1 MiB transport limit".to_owned());
    }
    let request: BoardPlotRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    if request.type_ != "kicad_monkey.board_plot.request" || request.version != "a0" {
        return Err("unsupported board plot contract identity".to_owned());
    }
    let max_source_bytes = decimal_usize(&request.max_source_bytes, "max_source_bytes")?;
    let max_output_bytes = decimal_usize(&request.max_output_bytes, "max_output_bytes")?;
    let max_text_bytes = decimal_usize(&request.max_text_bytes, "max_text_bytes")?;
    if source.len() > max_source_bytes {
        return diagnostic_output(source_limit_diagnostic());
    }
    if request.text_variables.len() > MAX_BOARD_TEXT_VARIABLES {
        return diagnostic_output(sidecar_limit_diagnostic(
            "text_variables exceeds the fixed entry limit",
        ));
    }
    let text_variable_bytes = request
        .text_variables
        .iter()
        .try_fold(0usize, |total, variable| {
            total
                .checked_add(variable.name.len())
                .and_then(|value| value.checked_add(variable.value.len()))
                .filter(|value| *value <= MAX_BOARD_TEXT_VARIABLE_SIDECAR_BYTES)
        });
    let Some(text_variable_bytes) = text_variable_bytes else {
        return diagnostic_output(sidecar_limit_diagnostic(
            "text_variables exceeds the fixed byte limit",
        ));
    };
    debug_assert!(text_variable_bytes <= MAX_BOARD_TEXT_VARIABLE_SIDECAR_BYTES);
    let net_classes = BoardNetClassAssignments::from_entries(
        request
            .net_class_assignments
            .iter()
            .map(|assignment| (assignment.net_name.clone(), assignment.classes.clone())),
    );
    let text_variables = BoardTextVariables::from_entries(
        request
            .text_variables
            .iter()
            .map(|variable| (variable.name.as_str(), variable.value.as_str())),
    );
    let operation = (|| {
        let text = utf8_text(source)?;
        board_plot_document_with_sidecars(
            text,
            BoardPlotLimits {
                max_source_bytes,
                max_depth: request.max_depth as usize,
                max_graphics: request.max_graphics as usize,
                max_operations: request.max_operations as usize,
                max_points: request.max_points as usize,
                max_text_bytes,
                max_net_class_bytes: max_output_bytes.min(MAX_BOARD_RETAINED_NET_CLASS_BYTES),
                max_metadata_bytes: max_output_bytes.min(MAX_BOARD_RETAINED_METADATA_BYTES),
                max_parse_nodes: request.max_parse_nodes as usize,
                max_input_points: request.max_input_points as usize,
                max_input_polygons: request.max_input_polygons as usize,
                max_cache_polygons: request.max_cache_polygons as usize,
                max_cache_contours: request.max_cache_contours as usize,
            },
            &net_classes,
            &text_variables,
        )
    })();
    let (result, output_bytes) = match operation {
        Ok(document) => success(document, request, max_output_bytes)?,
        Err(error) => failure(board_diagnostic(error)),
    };
    Ok(BoardPlotOutput {
        result_json: serde_json::to_vec(&result).map_err(|error| error.to_string())?,
        output_bytes,
    })
}

fn success(
    document: kicad_monkey_core::BoardPlotDocument,
    request: BoardPlotRequestA0,
    max_output_bytes: usize,
) -> Result<(BoardPlotResultA0, Vec<u8>), String> {
    let total = document
        .records
        .iter()
        .map(kicad_monkey_core::BoardPlotRecord::operation_count)
        .sum::<usize>();
    let total_operations = u32::try_from(total).unwrap_or(u32::MAX);
    let contract = project_board_plot_document_a0(
        document,
        request.source_path,
        request.document_id.unwrap_or_default(),
        kicad_monkey_core::BoardPlotContractLimits::default(),
    )?;
    let Some(output) = serialize_bounded(&contract, max_output_bytes)? else {
        return Ok(failure(limit_diagnostic()));
    };
    Ok((
        BoardPlotResultA0 {
            diagnostics: Vec::new(),
            output_bytes: output.len().to_string(),
            total_operations,
            type_: "kicad_monkey.board_plot.result".to_owned(),
            version: "a0".to_owned(),
        },
        output,
    ))
}
pub use kicad_monkey_core::project_board_plot_document_a0;

fn failure(diagnostic: Diagnostic) -> (BoardPlotResultA0, Vec<u8>) {
    (
        BoardPlotResultA0 {
            diagnostics: vec![diagnostic],
            output_bytes: "0".to_owned(),
            total_operations: 0,
            type_: "kicad_monkey.board_plot.result".to_owned(),
            version: "a0".to_owned(),
        },
        Vec::new(),
    )
}

fn diagnostic_output(diagnostic: Diagnostic) -> Result<BoardPlotOutput, String> {
    let (result, output_bytes) = failure(diagnostic);
    Ok(BoardPlotOutput {
        result_json: serde_json::to_vec(&result).map_err(|error| error.to_string())?,
        output_bytes,
    })
}

fn board_diagnostic(error: Error) -> Diagnostic {
    let code = if error.kind == kicad_monkey_core::ErrorKind::InvalidBuildValue
        && error.message.contains("outline-font bridge")
    {
        "unsupported_feature"
    } else {
        crate::error_code(error.kind)
    };
    Diagnostic {
        code: code.to_owned(),
        message: error.message.into_owned(),
        phase: match error.phase {
            ErrorPhase::Lex => DiagnosticPhase::Lex,
            ErrorPhase::Tree => DiagnosticPhase::Tree,
            ErrorPhase::Build => DiagnosticPhase::Build,
        },
        position: error.position.map(|position| SourcePosition {
            column: position.column.to_string(),
            line: position.line.to_string(),
            offset: position.offset.to_string(),
        }),
        token: error.token,
    }
}

fn limit_diagnostic() -> Diagnostic {
    Diagnostic {
        code: "resource_limit".to_owned(),
        message: "Serialized output exceeds max_output_bytes".to_owned(),
        phase: DiagnosticPhase::Build,
        position: None,
        token: None,
    }
}

fn source_limit_diagnostic() -> Diagnostic {
    Diagnostic {
        code: "resource_limit".to_owned(),
        message: "Source bytes exceed max_source_bytes".to_owned(),
        phase: DiagnosticPhase::Lex,
        position: None,
        token: None,
    }
}

fn sidecar_limit_diagnostic(message: &str) -> Diagnostic {
    Diagnostic {
        code: "resource_limit".to_owned(),
        message: message.to_owned(),
        phase: DiagnosticPhase::Tree,
        position: None,
        token: None,
    }
}

fn decimal_usize(value: &str, field: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{field} must be a platform-sized decimal string"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn board_plot_request(vector: &Value, max_output: &str, max_operations: u32) -> Vec<u8> {
        let mut request = serde_json::json!({
            "type": "kicad_monkey.board_plot.request",
            "version": "a0",
            "source_path": vector["source_path"],
            "document_id": vector["document_id"],
            "max_source_bytes": "65536",
            "max_output_bytes": max_output,
            "max_depth": 32,
            "max_graphics": 1000,
            "max_operations": max_operations,
            "max_points": 10000
            ,"max_text_bytes": "65536"
            ,"max_parse_nodes": 10000
            ,"max_input_points": 10000
            ,"max_input_polygons": 1000
            ,"max_cache_polygons": 1000
            ,"max_cache_contours": 10000
        });
        if let Some(assignments) = vector["net_class_assignments"].as_object() {
            request["net_class_assignments"] = assignments
                .iter()
                .map(|(net_name, classes)| {
                    serde_json::json!({ "net_name": net_name, "classes": classes })
                })
                .collect();
        }
        if let Some(variables) = vector["text_variables"].as_object() {
            request["text_variables"] = variables
                .iter()
                .map(|(name, value)| serde_json::json!({ "name": name, "value": value }))
                .collect();
        }
        serde_json::to_vec(&request).expect("request JSON")
    }

    #[test]
    fn board_plotter_host_path_emits_established_ir_shape_and_limits_output() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../../../../tests/parity/board_plotter_a0_vectors.json"
        ))
        .expect("shared board vectors");
        for vector in vectors["vectors"].as_array().expect("vectors") {
            let source = vector["source"].as_str().expect("source").as_bytes();
            let request = board_plot_request(vector, "65536", 10_000);
            let output = plot_board_ir_impl(source, &request).expect("plotter operation");
            let metadata: Value =
                serde_json::from_slice(&output.result_json).expect("result metadata JSON");
            let document: Value =
                serde_json::from_slice(&output.output_bytes).expect("plotter document JSON");
            assert_eq!(metadata["diagnostics"], serde_json::json!([]));
            assert_eq!(
                metadata["total_operations"],
                vector["expected"]["total_operations"]
            );
            assert_eq!(document, vector["expected"], "{}", vector["id"]);
            for record in document["records"].as_array().expect("record array") {
                for operation in record["operations"].as_array().into_iter().flatten() {
                    if operation["kind"] == "Text" {
                        assert!(
                            operation.get("context").is_none(),
                            "legacy board text must not acquire hyperlink context"
                        );
                    }
                }
            }
        }

        let vector = &vectors["vectors"][0];
        let source = vector["source"].as_str().expect("source").as_bytes();
        let limited_request = board_plot_request(vector, "8", 10_000);
        let limited = plot_board_ir_impl(source, &limited_request)
            .expect("output limit belongs in result metadata");
        let limited_metadata: Value =
            serde_json::from_slice(&limited.result_json).expect("limited result JSON");
        assert_eq!(limited_metadata["diagnostics"][0]["code"], "resource_limit");
        assert_eq!(limited_metadata["diagnostics"][0]["phase"], "build");
        assert!(limited.output_bytes.is_empty());
    }

    #[test]
    fn board_operation_budgets_fail_closed_in_the_result_envelope() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../../../../tests/parity/board_plotter_a0_vectors.json"
        ))
        .expect("shared board vectors");
        let vector = &vectors["vectors"][0];
        let source = vector["source"].as_str().expect("source").as_bytes();
        let request = board_plot_request(vector, "65536", 0);
        let output = plot_board_ir_impl(source, &request).expect("host operation");
        assert!(output.output_bytes.is_empty());
        let result: BoardPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("result contract");
        assert_eq!(result.diagnostics[0].code, "resource_limit");
    }

    #[test]
    fn dimension_variables_and_safe_integer_failures_stay_in_the_result_envelope() {
        let request = br#"{"type":"kicad_monkey.board_plot.request","version":"a0","source_path":"dimensions.kicad_pcb","document_id":"dimensions","max_source_bytes":"4096","max_output_bytes":"65536","max_depth":32,"max_graphics":10,"max_operations":100,"max_points":1000,"max_text_bytes":"4096","max_parse_nodes":1000,"max_input_points":100,"max_input_polygons":10,"max_cache_polygons":10,"max_cache_contours":100,"text_variables":[{"name":"PROJECT","value":"demo"}]}"#;
        let source = br#"(kicad_pcb
          (dimension (type center) (pts (xy 0 0) (xy 1 0))
            (format (override_value "${PROJECT}") (units_format 0))
            (gr_text "authored" (at 0 0) (effects (font (face "Arial") (size 1 1))))))"#;
        let output = plot_board_ir_impl(source, request).expect("dimension variable output");
        let metadata: Value =
            serde_json::from_slice(&output.result_json).expect("dimension metadata");
        let document: Value =
            serde_json::from_slice(&output.output_bytes).expect("dimension document");
        assert_eq!(metadata["diagnostics"], serde_json::json!([]));
        assert_eq!(document["records"][0]["text"], "${PROJECT}");
        assert_eq!(document["records"][0]["operations"][0]["text"], "demo");

        let unsafe_source = br#"(kicad_pcb
          (dimension (type center)
            (pts (xy 9007199254.740991 0) (xy 9007199254.740991 1))))"#;
        let output =
            plot_board_ir_impl(unsafe_source, request).expect("safe-integer diagnostic envelope");
        let metadata: Value =
            serde_json::from_slice(&output.result_json).expect("unsafe dimension metadata");
        assert!(
            !metadata["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .is_empty()
        );
        assert_eq!(metadata["total_operations"], 0);
        assert!(output.output_bytes.is_empty());
    }

    #[test]
    fn deferred_text_box_cache_wrapping_uses_a_stable_diagnostic() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../../../../tests/parity/board_plotter_a0_vectors.json"
        ))
        .expect("shared board vectors");
        let vector = &vectors["vectors"][0];
        let request = board_plot_request(vector, "65536", 10_000);
        let source = br#"(kicad_pcb
          (gr_text_box "A A" (start 0 0) (end 4 2)
            (effects (font (size 1 2.1)))
            (render_cache "A A" 0
              (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))))"#;
        let output = plot_board_ir_impl(source, &request).expect("diagnostic envelope");
        assert!(output.output_bytes.is_empty());
        let result: BoardPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("result contract");
        assert_eq!(result.diagnostics[0].code, "unsupported_feature");
    }

    #[test]
    fn board_request_and_text_variable_sidecars_are_bounded_before_expansion() {
        let oversized_request = vec![b' '; MAX_BOARD_PLOT_REQUEST_BYTES + 1];
        assert!(
            plot_board_ir_impl(b"(kicad_pcb)", &oversized_request)
                .err()
                .expect("request transport ceiling")
                .contains("1 MiB")
        );

        let vectors: Value = serde_json::from_str(include_str!(
            "../../../../tests/parity/board_plotter_a0_vectors.json"
        ))
        .expect("shared board vectors");
        let vector = &vectors["vectors"][0];
        let mut request: Value =
            serde_json::from_slice(&board_plot_request(vector, "65536", 10_000))
                .expect("request value");
        let mut source_limited = request.clone();
        source_limited["max_source_bytes"] = serde_json::json!("1");
        let output = plot_board_ir_impl(
            &vec![b'a'; 1024 * 1024],
            &serde_json::to_vec(&source_limited).expect("source-limited request"),
        )
        .expect("source limit uses the diagnostic envelope");
        let result: BoardPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("source limit result");
        assert_eq!(result.diagnostics[0].code, "resource_limit");

        request["text_variables"] = serde_json::json!([{
            "name": "A",
            "value": "x".repeat(MAX_BOARD_TEXT_VARIABLE_SIDECAR_BYTES)
        }]);
        let encoded = serde_json::to_vec(&request).expect("large sidecar request");
        assert!(encoded.len() < MAX_BOARD_PLOT_REQUEST_BYTES);
        let output = plot_board_ir_impl(b"(kicad_pcb)", &encoded)
            .expect("sidecar byte ceiling uses the diagnostic envelope");
        let result: BoardPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("sidecar byte result");
        assert_eq!(result.diagnostics[0].code, "resource_limit");
        assert!(result.diagnostics[0].message.contains("text_variables"));

        request["text_variables"] = serde_json::json!(
            (0..=MAX_BOARD_TEXT_VARIABLES)
                .map(|_| serde_json::json!({ "name": "", "value": "" }))
                .collect::<Vec<_>>()
        );
        let encoded = serde_json::to_vec(&request).expect("many sidecars request");
        assert!(encoded.len() < MAX_BOARD_PLOT_REQUEST_BYTES);
        let output = plot_board_ir_impl(b"(kicad_pcb)", &encoded)
            .expect("sidecar count ceiling uses the diagnostic envelope");
        let result: BoardPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("sidecar count result");
        assert_eq!(result.diagnostics[0].code, "resource_limit");
    }
}

//! Browser adapter for the source-selected board gr_* graphics producer.

use crate::{plotter_contract::contract_plotter_operation, serialize_bounded};
use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::board_plot_document::{
    BoardGraphicPlotRecord, BoardGraphicRecordKind, BoardPlotDocumentA0, PlotterCoordinateSpace,
    PlotterOperation,
};
use kicad_monkey_contracts::generated::board_plot_request::BoardPlotRequestA0;
use kicad_monkey_contracts::generated::board_plot_result::{
    BoardPlotResultA0, Diagnostic, DiagnosticPhase, SourcePosition,
};
use kicad_monkey_contracts::validate_board_plot_document;
use kicad_monkey_core::{
    BoardPlotLimits, BoardPlotRecordKind, Error, ErrorPhase, board_plot_document, utf8_text,
};
use wasm_bindgen::prelude::*;

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
    let request: BoardPlotRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    if request.type_ != "kicad_monkey.board_plot.request" || request.version != "a0" {
        return Err("unsupported board plot contract identity".to_owned());
    }
    let max_source_bytes = decimal_usize(&request.max_source_bytes, "max_source_bytes")?;
    let max_output_bytes = decimal_usize(&request.max_output_bytes, "max_output_bytes")?;
    let operation = (|| {
        let text = utf8_text(source)?;
        board_plot_document(
            text,
            BoardPlotLimits {
                max_source_bytes,
                max_depth: request.max_depth as usize,
                max_graphics: request.max_graphics as usize,
                max_operations: request.max_operations as usize,
                max_points: request.max_points as usize,
            },
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
        .map(|record| record.operations.len())
        .sum::<usize>();
    let total_operations = u32::try_from(total).unwrap_or(u32::MAX);
    let mut records = Vec::with_capacity(document.records.len());
    for record in document.records {
        let operation_count = u32::try_from(record.operations.len()).unwrap_or(u32::MAX);
        let mut operations = Vec::with_capacity(record.operations.len());
        // The established Python serializer numbers operations per record.
        for (index, operation) in record.operations.into_iter().enumerate() {
            let shared = contract_plotter_operation(index, operation)?;
            let value = serde_json::to_value(shared).map_err(|error| error.to_string())?;
            operations.push(
                serde_json::from_value::<PlotterOperation>(value)
                    .map_err(|error| error.to_string())?,
            );
        }
        records.push(BoardGraphicPlotRecord {
            kind: contract_record_kind(record.kind),
            layer: Some(record.layer),
            object_id: record.kind.as_str().to_owned(),
            operation_count,
            operations,
            uuid: record.uuid,
        });
    }
    let contract = BoardPlotDocumentA0 {
        coordinate_space: PlotterCoordinateSpace {
            unit: "nm".to_owned(),
            y_axis: "down".to_owned(),
        },
        document_id: request.document_id.unwrap_or_default(),
        generator: document.generator,
        generator_version: document.generator_version,
        paper: document.paper,
        records,
        schema: "kicad.plotter_ir.a0".to_owned(),
        source_kind: "PCB".to_owned(),
        source_path: request.source_path,
        thickness_mm: document.thickness_mm,
        total_operations,
        version: JavaScriptSafeInteger::try_from(document.version)
            .map_err(|error| error.to_string())?,
    };
    validate_board_plot_document(&contract).map_err(|error| error.to_string())?;
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

fn contract_record_kind(kind: BoardPlotRecordKind) -> BoardGraphicRecordKind {
    match kind {
        BoardPlotRecordKind::GrLine => BoardGraphicRecordKind::GrLine,
        BoardPlotRecordKind::GrArc => BoardGraphicRecordKind::GrArc,
        BoardPlotRecordKind::GrCircle => BoardGraphicRecordKind::GrCircle,
        BoardPlotRecordKind::GrRect => BoardGraphicRecordKind::GrRect,
        BoardPlotRecordKind::GrPoly => BoardGraphicRecordKind::GrPoly,
        BoardPlotRecordKind::GrCurve => BoardGraphicRecordKind::GrCurve,
    }
}

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

fn board_diagnostic(error: Error) -> Diagnostic {
    Diagnostic {
        code: crate::error_code(error.kind).to_owned(),
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
        serde_json::to_vec(&serde_json::json!({
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
        }))
        .expect("request JSON")
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
}

//! Browser adapter for the source-selected non-text symbol producer.

use crate::{plotter_contract::contract_plotter_operation, serialize_bounded};
use kicad_monkey_contracts::generated::symbol_plot_document::{
    LibSubsymbolPlotRecord, PlotterCoordinateSpace, PlotterOperation, SymbolHeaderPlotRecord,
    SymbolPlotDocumentA0, SymbolPlotRecord,
};
use kicad_monkey_contracts::generated::symbol_plot_request::SymbolPlotRequestA0;
use kicad_monkey_contracts::generated::symbol_plot_result::{
    Diagnostic, DiagnosticPhase, SourcePosition, SymbolPlotResultA0,
};
use kicad_monkey_contracts::validate_symbol_plot_document;
use kicad_monkey_core::{
    Error, ErrorKind, ErrorPhase, SymbolPlotLimits, symbol_plot_document, utf8_text,
};
use wasm_bindgen::prelude::*;

/// Paired metadata and out-of-band symbol plotter-IR JSON bytes.
#[wasm_bindgen]
pub struct SymbolPlotOutput {
    result_json: Vec<u8>,
    output_bytes: Vec<u8>,
}

#[wasm_bindgen]
impl SymbolPlotOutput {
    /// TypeSpec-governed `SymbolPlotResultA0` metadata bytes.
    #[wasm_bindgen(js_name = resultJson)]
    pub fn result_json(&self) -> Vec<u8> {
        self.result_json.clone()
    }

    /// `kicad.plotter_ir.a0` JSON bytes; empty when diagnostics are present.
    #[wasm_bindgen(js_name = outputBytes)]
    pub fn output_bytes(&self) -> Vec<u8> {
        self.output_bytes.clone()
    }
}

/// Convert one selected library symbol to non-text plotter-IR JSON bytes.
#[wasm_bindgen(js_name = plotSymbolIr)]
pub fn plot_symbol_ir(source: &[u8], request_json: &[u8]) -> Result<SymbolPlotOutput, JsValue> {
    plot_symbol_ir_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

fn plot_symbol_ir_impl(source: &[u8], request_json: &[u8]) -> Result<SymbolPlotOutput, String> {
    let request: SymbolPlotRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    if request.type_ != "kicad_monkey.symbol_plot.request" || request.version != "a0" {
        return Err("unsupported symbol plot contract identity".to_owned());
    }
    let max_source_bytes = decimal_usize(&request.max_source_bytes, "max_source_bytes")?;
    let max_output_bytes = decimal_usize(&request.max_output_bytes, "max_output_bytes")?;
    let operation = (|| {
        let text = utf8_text(source)?;
        symbol_plot_document(
            text,
            &request.symbol_name,
            request.unit,
            request.style,
            SymbolPlotLimits {
                max_source_bytes,
                max_depth: request.max_depth as usize,
                max_symbols: request.max_symbols as usize,
                max_subsymbols: request.max_subsymbols as usize,
                max_operations: request.max_operations as usize,
                max_points: request.max_points as usize,
            },
        )
    })();
    let (result, output_bytes) = match operation {
        Ok(document) => success(document, request, max_output_bytes)?,
        Err(error) => failure(symbol_diagnostic(error)),
    };
    Ok(SymbolPlotOutput {
        result_json: serde_json::to_vec(&result).map_err(|error| error.to_string())?,
        output_bytes,
    })
}

fn success(
    document: kicad_monkey_core::SymbolPlotDocument,
    request: SymbolPlotRequestA0,
    max_output_bytes: usize,
) -> Result<(SymbolPlotResultA0, Vec<u8>), String> {
    let total = document
        .records
        .iter()
        .map(|record| record.operations.len())
        .sum::<usize>();
    let total_operations = u32::try_from(total).unwrap_or(u32::MAX);
    let mut next_index = 0usize;
    let header = SymbolHeaderPlotRecord {
        extends: document.extends,
        in_bom: document.in_bom,
        kind: "lib_symbol".to_owned(),
        name: document.name.clone(),
        object_id: document.name.clone(),
        on_board: document.on_board,
        operation_count: 0,
        operations: Vec::new(),
        power: document.power,
        style: document.style,
        unit: document.unit,
        uuid: String::new(),
    };
    let mut records = vec![SymbolPlotRecord::from(header)];
    for record in document.records {
        let operation_count = u32::try_from(record.operations.len()).unwrap_or(u32::MAX);
        let mut operations = Vec::with_capacity(record.operations.len());
        for operation in record.operations {
            let shared = contract_plotter_operation(next_index, operation)?;
            let value = serde_json::to_value(shared).map_err(|error| error.to_string())?;
            operations.push(
                serde_json::from_value::<PlotterOperation>(value)
                    .map_err(|error| error.to_string())?,
            );
            next_index = next_index.saturating_add(1);
        }
        records.push(
            LibSubsymbolPlotRecord {
                kind: "lib_subsymbol".to_owned(),
                object_id: record.name.clone(),
                operation_count,
                operations,
                style: record.style,
                unit: record.unit,
                uuid: String::new(),
            }
            .into(),
        );
    }
    let contract = SymbolPlotDocumentA0 {
        coordinate_space: PlotterCoordinateSpace {
            unit: "nm".to_owned(),
            y_axis: "down".to_owned(),
        },
        document_id: request.document_id.unwrap_or(document.name),
        records,
        schema: "kicad.plotter_ir.a0".to_owned(),
        source_kind: "SYM".to_owned(),
        source_path: request.source_path,
        total_operations,
    };
    validate_symbol_plot_document(&contract).map_err(|error| error.to_string())?;
    let Some(output) = serialize_bounded(&contract, max_output_bytes)? else {
        return Ok(failure(limit_diagnostic()));
    };
    Ok((
        SymbolPlotResultA0 {
            diagnostics: Vec::new(),
            output_bytes: output.len().to_string(),
            total_operations,
            type_: "kicad_monkey.symbol_plot.result".to_owned(),
            version: "a0".to_owned(),
        },
        output,
    ))
}

fn failure(diagnostic: Diagnostic) -> (SymbolPlotResultA0, Vec<u8>) {
    (
        SymbolPlotResultA0 {
            diagnostics: vec![diagnostic],
            output_bytes: "0".to_owned(),
            total_operations: 0,
            type_: "kicad_monkey.symbol_plot.result".to_owned(),
            version: "a0".to_owned(),
        },
        Vec::new(),
    )
}

fn symbol_diagnostic(error: Error) -> Diagnostic {
    Diagnostic {
        code: error_code(error.kind).to_owned(),
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

fn error_code(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::InvalidUtf8 => "invalid_utf8",
        ErrorKind::ResourceLimit => "resource_limit",
        ErrorKind::EmptyExpression => "empty_expression",
        ErrorKind::MissingOpeningParenthesis => "missing_opening_parenthesis",
        ErrorKind::UnbalancedOpeningParenthesis => "unbalanced_opening_parenthesis",
        ErrorKind::UnbalancedClosingParenthesis => "unbalanced_closing_parenthesis",
        ErrorKind::LeftoverGarbage => "leftover_garbage",
        ErrorKind::UnterminatedString => "unterminated_string",
        _ => "invalid_symbol_model",
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

    const LIBRARY: &str = r#"(kicad_symbol_lib (version 20241209)
      (symbol "Demo" (in_bom yes) (on_board yes)
        (symbol "Demo_1_1"
          (rectangle (start -1 1) (end 1 -1)
            (stroke (width 0.2) (type solid)) (fill (type background))))))"#;

    #[test]
    fn host_symbol_adapter_returns_bounded_contract_bytes() {
        let request = br#"{"type":"kicad_monkey.symbol_plot.request","version":"a0","symbol_name":"Demo","unit":1,"style":0,"max_source_bytes":"4096","max_output_bytes":"65536","max_depth":32,"max_symbols":10,"max_subsymbols":10,"max_operations":10,"max_points":100}"#;
        let output = plot_symbol_ir_impl(LIBRARY.as_bytes(), request).expect("host operation");
        let result: SymbolPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("result contract");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.total_operations, 2);
        let document: SymbolPlotDocumentA0 =
            serde_json::from_slice(&output.output_bytes).expect("document contract");
        validate_symbol_plot_document(&document).expect("semantic contract");
    }

    #[test]
    fn host_symbol_adapter_fails_closed_on_limits() {
        let request = br#"{"type":"kicad_monkey.symbol_plot.request","version":"a0","symbol_name":"Demo","style":0,"max_source_bytes":"1","max_output_bytes":"65536","max_depth":32,"max_symbols":10,"max_subsymbols":10,"max_operations":10,"max_points":100}"#;
        let output = plot_symbol_ir_impl(LIBRARY.as_bytes(), request).expect("host operation");
        assert!(output.output_bytes.is_empty());
        let result: SymbolPlotResultA0 =
            serde_json::from_slice(&output.result_json).expect("result contract");
        assert_eq!(result.diagnostics[0].code, "resource_limit");
    }
}

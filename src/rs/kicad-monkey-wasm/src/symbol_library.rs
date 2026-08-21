//! Browser operations for typed symbol-library reads and focused writes.

use crate::{decimal_usize, error_code, validate_identity};
use kicad_monkey_contracts::generated::symbol_library_edit_request::{
    SymbolBooleanField as ContractBooleanField, SymbolLibraryEditRequestA0,
};
use kicad_monkey_contracts::generated::symbol_library_edit_result::{
    Diagnostic as EditDiagnostic, DiagnosticPhase as EditDiagnosticPhase,
    SourcePosition as EditSourcePosition, SymbolLibraryEditResultA0,
};
use kicad_monkey_contracts::generated::symbol_library_read_request::SymbolLibraryReadRequestA0;
use kicad_monkey_contracts::generated::symbol_library_read_result::{
    Diagnostic as ReadDiagnostic, DiagnosticPhase as ReadDiagnosticPhase,
    SourcePosition as ReadSourcePosition, SymbolLibraryReadResultA0,
    SymbolSummary as ContractSummary,
};
use kicad_monkey_core::{
    Error, ErrorPhase, SymbolBooleanField, SymbolLibraryLimits, SymbolLibraryView, utf8_text,
};
use wasm_bindgen::prelude::*;

/// Read ordered typed summaries from caller-owned symbol-library bytes.
#[wasm_bindgen(js_name = readSymbolLibrary)]
pub fn read_symbol_library(source: &[u8], request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
    read_symbol_library_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

fn read_symbol_library_impl(source: &[u8], request_json: &[u8]) -> Result<Vec<u8>, String> {
    let request: SymbolLibraryReadRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    validate_identity(
        &request.type_,
        &request.version,
        "kicad_monkey.symbol_library_read.request",
    )?;
    let limits = limits_from_read(&request)?;
    let operation = (|| {
        let view = SymbolLibraryView::parse(utf8_text(source)?, limits)?;
        view.symbols()
            .map(|summary| {
                summary.map(|value| ContractSummary {
                    extends: value.extends.map(|text| text.into_owned()),
                    in_bom: value.in_bom,
                    name: value.name.into_owned(),
                    on_board: value.on_board,
                    pin_count: u32::try_from(value.pin_count).unwrap_or(u32::MAX),
                    power: value.power,
                    power_kind: value.power_kind.map(|text| text.into_owned()),
                    property_count: u32::try_from(value.property_count).unwrap_or(u32::MAX),
                    subsymbol_count: u32::try_from(value.subsymbol_count).unwrap_or(u32::MAX),
                })
            })
            .collect::<Result<Vec<_>, Error>>()
    })();
    let result = match operation {
        Ok(symbols) => SymbolLibraryReadResultA0 {
            diagnostics: Vec::new(),
            source_bytes: source.len().to_string(),
            symbols,
            type_: "kicad_monkey.symbol_library_read.result".to_owned(),
            version: "a0".to_owned(),
        },
        Err(error) => SymbolLibraryReadResultA0 {
            diagnostics: vec![read_diagnostic(error)],
            source_bytes: source.len().to_string(),
            symbols: Vec::new(),
            type_: "kicad_monkey.symbol_library_read.result".to_owned(),
            version: "a0".to_owned(),
        },
    };
    serde_json::to_vec(&result).map_err(|error| error.to_string())
}

/// Metadata and separate KiCad bytes from a focused symbol-library edit.
#[wasm_bindgen]
pub struct SymbolLibraryEditOutput {
    result_json: Vec<u8>,
    output_bytes: Vec<u8>,
}

#[wasm_bindgen]
impl SymbolLibraryEditOutput {
    #[wasm_bindgen(js_name = resultJson)]
    pub fn result_json(&self) -> Vec<u8> {
        self.result_json.clone()
    }

    #[wasm_bindgen(js_name = outputBytes)]
    pub fn output_bytes(&self) -> Vec<u8> {
        self.output_bytes.clone()
    }

    /// Consume this result and transfer the edited bytes exactly once.
    #[wasm_bindgen(js_name = takeOutputBytes)]
    pub fn take_output_bytes(self) -> Vec<u8> {
        self.output_bytes
    }
}

/// Edit one symbol boolean while preserving all unrelated source bytes.
#[wasm_bindgen(js_name = editSymbolLibraryBoolean)]
pub fn edit_symbol_library_boolean(
    source: &[u8],
    request_json: &[u8],
) -> Result<SymbolLibraryEditOutput, JsValue> {
    edit_symbol_library_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

fn edit_symbol_library_impl(
    source: &[u8],
    request_json: &[u8],
) -> Result<SymbolLibraryEditOutput, String> {
    let request: SymbolLibraryEditRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    validate_identity(
        &request.type_,
        &request.version,
        "kicad_monkey.symbol_library_edit.request",
    )?;
    let limits = limits_from_edit(&request)?;
    let field = match request.field {
        ContractBooleanField::InBom => SymbolBooleanField::InBom,
        ContractBooleanField::OnBoard => SymbolBooleanField::OnBoard,
    };
    let operation = (|| {
        let view = SymbolLibraryView::parse(utf8_text(source)?, limits)?;
        view.set_boolean(
            &request.symbol_name,
            field,
            request.value,
            limits.max_output_bytes,
        )
    })();
    let (result, output_bytes) = match operation {
        Ok(edit) => {
            let output = edit.source.into_bytes();
            (
                SymbolLibraryEditResultA0 {
                    changed: edit.changed,
                    diagnostics: Vec::new(),
                    output_bytes: output.len().to_string(),
                    type_: "kicad_monkey.symbol_library_edit.result".to_owned(),
                    version: "a0".to_owned(),
                },
                output,
            )
        }
        Err(error) => (
            SymbolLibraryEditResultA0 {
                changed: false,
                diagnostics: vec![edit_diagnostic(error)],
                output_bytes: "0".to_owned(),
                type_: "kicad_monkey.symbol_library_edit.result".to_owned(),
                version: "a0".to_owned(),
            },
            Vec::new(),
        ),
    };
    Ok(SymbolLibraryEditOutput {
        result_json: serde_json::to_vec(&result).map_err(|error| error.to_string())?,
        output_bytes,
    })
}

fn limits_from_read(request: &SymbolLibraryReadRequestA0) -> Result<SymbolLibraryLimits, String> {
    Ok(SymbolLibraryLimits {
        max_source_bytes: decimal_usize(&request.max_source_bytes, "max_source_bytes")?,
        max_output_bytes: usize::MAX,
        max_depth: request.max_depth as usize,
        max_symbols: request.max_symbols as usize,
        max_metadata_forms: request.max_metadata_forms as usize,
        max_subsymbols: request.max_subsymbols as usize,
        max_pins: request.max_pins as usize,
    })
}

fn limits_from_edit(request: &SymbolLibraryEditRequestA0) -> Result<SymbolLibraryLimits, String> {
    Ok(SymbolLibraryLimits {
        max_source_bytes: decimal_usize(&request.max_source_bytes, "max_source_bytes")?,
        max_output_bytes: decimal_usize(&request.max_output_bytes, "max_output_bytes")?,
        max_depth: request.max_depth as usize,
        max_symbols: request.max_symbols as usize,
        max_metadata_forms: request.max_metadata_forms as usize,
        max_subsymbols: request.max_subsymbols as usize,
        max_pins: request.max_pins as usize,
    })
}

fn read_diagnostic(error: Error) -> ReadDiagnostic {
    ReadDiagnostic {
        code: error_code(error.kind).to_owned(),
        message: error.message.into_owned(),
        phase: read_phase(error.phase),
        position: error.position.map(|position| ReadSourcePosition {
            column: position.column.to_string(),
            line: position.line.to_string(),
            offset: position.offset.to_string(),
        }),
        token: error.token,
    }
}

fn edit_diagnostic(error: Error) -> EditDiagnostic {
    EditDiagnostic {
        code: error_code(error.kind).to_owned(),
        message: error.message.into_owned(),
        phase: edit_phase(error.phase),
        position: error.position.map(|position| EditSourcePosition {
            column: position.column.to_string(),
            line: position.line.to_string(),
            offset: position.offset.to_string(),
        }),
        token: error.token,
    }
}

fn read_phase(phase: ErrorPhase) -> ReadDiagnosticPhase {
    match phase {
        ErrorPhase::Lex => ReadDiagnosticPhase::Lex,
        ErrorPhase::Tree => ReadDiagnosticPhase::Tree,
        ErrorPhase::Build => ReadDiagnosticPhase::Build,
    }
}

fn edit_phase(phase: ErrorPhase) -> EditDiagnosticPhase {
    match phase {
        ErrorPhase::Lex => EditDiagnosticPhase::Lex,
        ErrorPhase::Tree => EditDiagnosticPhase::Tree,
        ErrorPhase::Build => EditDiagnosticPhase::Build,
    }
}

#[cfg(test)]
mod tests {
    use super::{edit_symbol_library_impl, read_symbol_library_impl};
    use serde_json::Value;

    const SOURCE: &[u8] = br#"(kicad_symbol_lib
      (symbol "Base" (in_bom yes) (on_board no)
        (symbol "Base_1_1" (pin input line (at 0 0 0))))
      (symbol "Derived" (extends "Base") (power local)))"#;

    #[test]
    fn host_read_and_edit_keep_source_bytes_out_of_band() {
        let read = br#"{"type":"kicad_monkey.symbol_library_read.request","version":"a0","max_source_bytes":"4096","max_depth":32,"max_symbols":10,"max_metadata_forms":20,"max_subsymbols":10,"max_pins":10}"#;
        let result: Value = serde_json::from_slice(
            &read_symbol_library_impl(SOURCE, read).expect("read operation"),
        )
        .expect("read JSON");
        assert_eq!(result["symbols"][0]["name"], "Base");
        assert_eq!(result["symbols"][0]["pin_count"], 1);
        assert_eq!(result["symbols"][1]["extends"], "Base");
        assert!(result.get("source").is_none());

        let edit = br#"{"type":"kicad_monkey.symbol_library_edit.request","version":"a0","symbol_name":"Derived","field":"in_bom","value":false,"max_source_bytes":"4096","max_output_bytes":"4096","max_depth":32,"max_symbols":10,"max_metadata_forms":20,"max_subsymbols":10,"max_pins":10}"#;
        let output = edit_symbol_library_impl(SOURCE, edit).expect("edit operation");
        let metadata: Value = serde_json::from_slice(&output.result_json).expect("metadata");
        assert_eq!(metadata["changed"], true);
        assert!(
            String::from_utf8(output.output_bytes)
                .expect("UTF-8")
                .contains("(in_bom no)")
        );
    }

    #[test]
    fn host_read_failures_use_the_structured_diagnostic_envelope() {
        let request = br#"{"type":"kicad_monkey.symbol_library_read.request","version":"a0","max_source_bytes":"4096","max_depth":32,"max_symbols":10,"max_metadata_forms":20,"max_subsymbols":10,"max_pins":0}"#;
        let result: Value = serde_json::from_slice(
            &read_symbol_library_impl(SOURCE, request).expect("structured result"),
        )
        .expect("read JSON");
        assert_eq!(result["symbols"], serde_json::json!([]));
        assert_eq!(result["diagnostics"][0]["code"], "resource_limit");
    }
}

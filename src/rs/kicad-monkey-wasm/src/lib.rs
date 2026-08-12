//! Thin browser adapters over the shared safe Rust core.

#![forbid(unsafe_code)]

use kicad_monkey_contracts::generated::scan_request::SExpressionScanRequestA0;
use kicad_monkey_contracts::generated::scan_result::{
    Diagnostic, DiagnosticPhase, FormSpan, SExpressionScanResultA0, SourcePosition,
};
use kicad_monkey_core::{
    Error, ErrorKind, ErrorPhase, ProjectionLimits, Selector, build, parse_bytes,
    scan_reader_form_spans,
};
use std::collections::BTreeSet;
use std::io::Cursor;
use wasm_bindgen::prelude::*;

/// Canonicalize one KiCad S-expression byte buffer for the WASM smoke gate.
#[wasm_bindgen(js_name = canonicalizeSexpr)]
pub fn canonicalize_sexpr(source: &[u8]) -> Result<Vec<u8>, JsValue> {
    let tree = parse_bytes(source).map_err(js_error)?;
    build(&tree).map(String::into_bytes).map_err(js_error)
}

/// Run the TypeSpec-governed structural scan operation over caller-owned bytes.
#[wasm_bindgen(js_name = scanSexpr)]
pub fn scan_sexpr(source: &[u8], request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
    scan_sexpr_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

fn scan_sexpr_impl(source: &[u8], request_json: &[u8]) -> Result<Vec<u8>, String> {
    let request: SExpressionScanRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    if request.type_ != "kicad_monkey.sexpr_scan.request" || request.version != "a0" {
        return Err("Unsupported S-expression scan contract identity".to_owned());
    }
    let max_source_bytes = request
        .max_source_bytes
        .parse::<usize>()
        .map_err(|_| "max_source_bytes must be a platform-sized decimal string".to_owned())?;
    let selector = Selector {
        heads: nonempty_set(request.selector.heads),
        paths: nonempty_paths(request.selector.paths),
        min_depth: request.selector.min_depth.map(|value| value as usize),
        max_depth: request.selector.max_depth.map(|value| value as usize),
        prune_heads: request.selector.prune_heads.into_iter().collect(),
    };
    let limits = ProjectionLimits {
        max_source_bytes,
        max_depth: request.max_depth as usize,
        max_selected_forms: request.max_selected_forms as usize,
        ..ProjectionLimits::default()
    };
    let scan = scan_reader_form_spans(Cursor::new(source), &selector, limits);
    let (forms, diagnostics) = match scan {
        Ok(spans) => (spans.into_iter().map(contract_span).collect(), Vec::new()),
        Err(error) => (Vec::new(), vec![contract_diagnostic(error)]),
    };
    let result = SExpressionScanResultA0 {
        diagnostics,
        forms,
        source_bytes: source.len().to_string(),
        type_: "kicad_monkey.sexpr_scan.result".to_owned(),
        version: "a0".to_owned(),
    };
    serde_json::to_vec(&result).map_err(|error| error.to_string())
}

fn nonempty_set(values: Vec<String>) -> Option<BTreeSet<String>> {
    (!values.is_empty()).then(|| values.into_iter().collect())
}

fn nonempty_paths(values: Vec<Vec<String>>) -> Option<BTreeSet<Vec<String>>> {
    (!values.is_empty()).then(|| values.into_iter().collect())
}

fn contract_span(span: kicad_monkey_core::FormSpan) -> FormSpan {
    FormSpan {
        column: span.start.column.to_string(),
        depth: u32::try_from(span.depth).unwrap_or(u32::MAX),
        end_column: span.end.column.to_string(),
        end_line: span.end.line.to_string(),
        end_offset: span.range.end.to_string(),
        head: span.head,
        line: span.start.line.to_string(),
        path: span.path,
        start_offset: span.range.start.to_string(),
    }
}

fn contract_diagnostic(error: Error) -> Diagnostic {
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
        ErrorKind::UnterminatedString => "unterminated_string",
        ErrorKind::EmptyExpression => "empty_expression",
        ErrorKind::MissingOpeningParenthesis => "missing_opening_parenthesis",
        ErrorKind::UnbalancedOpeningParenthesis => "unbalanced_opening_parenthesis",
        ErrorKind::UnbalancedClosingParenthesis => "unbalanced_closing_parenthesis",
        ErrorKind::LeftoverGarbage => "leftover_garbage",
        ErrorKind::UnexpectedToken => "unexpected_token",
        ErrorKind::IntegerOutOfRange => "integer_out_of_range",
        ErrorKind::ResourceLimit => "resource_limit",
        ErrorKind::InvalidBuildValue => "invalid_build_value",
        ErrorKind::InvalidPatch => "invalid_patch",
        ErrorKind::InvalidSelector => "invalid_selector",
        ErrorKind::InvalidSpan => "invalid_span",
        ErrorKind::Io => "io",
    }
}

fn js_error(error: Error) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_sexpr, scan_sexpr_impl};
    use serde_json::Value;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[test]
    fn typed_scan_keeps_input_bytes_out_of_band() {
        let request = br#"{
            "type":"kicad_monkey.sexpr_scan.request",
            "version":"a0",
            "selector":{"heads":["footprint"]},
            "max_source_bytes":"1024",
            "max_depth":32,
            "max_selected_forms":16
        }"#;
        let result = scan_sexpr_impl(
            b"(kicad_pcb (footprint \"A:R\") (footprint \"A:C\"))",
            request,
        )
        .expect("scan should serialize");
        let value: Value = serde_json::from_slice(&result).expect("result should be JSON");
        assert_eq!(value["forms"].as_array().expect("forms").len(), 2);
        assert_eq!(value["diagnostics"], serde_json::json!([]));
        assert!(value.get("source").is_none());
    }

    #[wasm_bindgen_test]
    fn wasm_byte_input_produces_canonical_byte_output() {
        assert_eq!(
            canonicalize_sexpr(b"(root(child 1))").expect("valid source"),
            b"(root\n\t(child 1)\n)"
        );
    }
}

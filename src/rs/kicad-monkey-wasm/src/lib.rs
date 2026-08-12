//! Thin browser adapters over the shared safe Rust core.

#![forbid(unsafe_code)]

use kicad_monkey_contracts::generated::build_request::SExpressionBuildRequestA0;
use kicad_monkey_contracts::generated::footprint_edit_request::FootprintEditRequestA0;
use kicad_monkey_contracts::generated::footprint_edit_result::{
    Diagnostic as FootprintEditDiagnostic, DiagnosticPhase as FootprintEditDiagnosticPhase,
    FootprintEditResultA0, SourcePosition as FootprintEditSourcePosition,
};
use kicad_monkey_contracts::generated::footprint_read_request::FootprintReadRequestA0;
use kicad_monkey_contracts::generated::footprint_read_result::{
    Diagnostic as FootprintDiagnostic, DiagnosticPhase as FootprintDiagnosticPhase,
    FootprintProperty as FootprintContractProperty, FootprintReadResultA0,
    SourcePosition as FootprintSourcePosition,
};
use kicad_monkey_contracts::generated::scan_request::SExpressionScanRequestA0;
use kicad_monkey_contracts::generated::scan_result::{
    Diagnostic, DiagnosticPhase, FormSpan, SExpressionScanResultA0, SourcePosition,
};
use kicad_monkey_contracts::{ValidatedNode, validate_build_request};
use kicad_monkey_core::{
    Error, ErrorKind, ErrorPhase, FootprintLimits, FootprintView, ProjectionLimits, Selector, Sexp,
    build, build_with_limit, parse_bytes, scan_reader_form_spans, utf8_text,
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

/// Build canonical UTF-8 bytes from a TypeSpec-governed generic node request.
#[wasm_bindgen(js_name = buildSexpr)]
pub fn build_sexpr(request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
    build_sexpr_impl(request_json).map_err(|message| JsValue::from_str(&message))
}

/// Read typed standalone-footprint facts from caller-owned KiCad bytes.
#[wasm_bindgen(js_name = readFootprint)]
pub fn read_footprint(source: &[u8], request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
    read_footprint_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

/// Paired metadata and out-of-band KiCad bytes from a footprint edit.
#[wasm_bindgen]
pub struct FootprintEditOutput {
    result_json: Vec<u8>,
    output_bytes: Vec<u8>,
}

#[wasm_bindgen]
impl FootprintEditOutput {
    /// TypeSpec-governed `FootprintEditResultA0` metadata bytes.
    #[wasm_bindgen(js_name = resultJson)]
    pub fn result_json(&self) -> Vec<u8> {
        self.result_json.clone()
    }

    /// Edited KiCad UTF-8 bytes; empty when diagnostics are present.
    #[wasm_bindgen(js_name = outputBytes)]
    pub fn output_bytes(&self) -> Vec<u8> {
        self.output_bytes.clone()
    }
}

/// Apply one source-preserving property edit and return metadata plus KiCad bytes.
#[wasm_bindgen(js_name = editFootprintProperty)]
pub fn edit_footprint_property(
    source: &[u8],
    request_json: &[u8],
) -> Result<FootprintEditOutput, JsValue> {
    edit_footprint_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
}

fn read_footprint_impl(source: &[u8], request_json: &[u8]) -> Result<Vec<u8>, String> {
    let request: FootprintReadRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    validate_identity(
        &request.type_,
        &request.version,
        "kicad_monkey.footprint_read.request",
    )?;
    let limits = footprint_limits(
        &request.max_source_bytes,
        None,
        request.max_depth,
        request.max_properties,
        request.max_pads,
    )?;
    let operation = (|| {
        let text = utf8_text(source)?;
        let view = FootprintView::parse(text, limits)?;
        let name = view.name()?.into_owned();
        let properties = view
            .properties()
            .map(|property| {
                property.map(|value| FootprintContractProperty {
                    name: value.name.into_owned(),
                    value: value.value.into_owned(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((name, properties, view.pad_count()))
    })();
    let result = match operation {
        Ok((name, properties, pad_count)) => FootprintReadResultA0 {
            diagnostics: Vec::new(),
            name,
            pad_count: u32::try_from(pad_count).unwrap_or(u32::MAX),
            properties,
            source_bytes: source.len().to_string(),
            type_: "kicad_monkey.footprint_read.result".to_owned(),
            version: "a0".to_owned(),
        },
        Err(error) => FootprintReadResultA0 {
            diagnostics: vec![footprint_diagnostic(error)],
            name: String::new(),
            pad_count: 0,
            properties: Vec::new(),
            source_bytes: source.len().to_string(),
            type_: "kicad_monkey.footprint_read.result".to_owned(),
            version: "a0".to_owned(),
        },
    };
    serde_json::to_vec(&result).map_err(|error| error.to_string())
}

fn edit_footprint_impl(source: &[u8], request_json: &[u8]) -> Result<FootprintEditOutput, String> {
    let request: FootprintEditRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    validate_identity(
        &request.type_,
        &request.version,
        "kicad_monkey.footprint_edit.request",
    )?;
    let limits = footprint_limits(
        &request.max_source_bytes,
        Some(&request.max_output_bytes),
        request.max_depth,
        request.max_properties,
        request.max_pads,
    )?;
    let operation = (|| {
        let text = utf8_text(source)?;
        let view = FootprintView::parse(text, limits)?;
        view.set_property(
            &request.property_name,
            &request.value,
            limits.max_output_bytes,
        )
    })();
    let (result, output_bytes) = match operation {
        Ok(edit) => {
            let output = edit.source.into_bytes();
            (
                FootprintEditResultA0 {
                    changed: edit.changed,
                    diagnostics: Vec::new(),
                    output_bytes: output.len().to_string(),
                    type_: "kicad_monkey.footprint_edit.result".to_owned(),
                    version: "a0".to_owned(),
                },
                output,
            )
        }
        Err(error) => (
            FootprintEditResultA0 {
                changed: false,
                diagnostics: vec![footprint_edit_diagnostic(error)],
                output_bytes: "0".to_owned(),
                type_: "kicad_monkey.footprint_edit.result".to_owned(),
                version: "a0".to_owned(),
            },
            Vec::new(),
        ),
    };
    Ok(FootprintEditOutput {
        result_json: serde_json::to_vec(&result).map_err(|error| error.to_string())?,
        output_bytes,
    })
}

fn footprint_limits(
    max_source_bytes: &str,
    max_output_bytes: Option<&str>,
    max_depth: u32,
    max_properties: u32,
    max_pads: u32,
) -> Result<FootprintLimits, String> {
    let source = max_source_bytes
        .parse::<usize>()
        .map_err(|_| "max_source_bytes must be a platform-sized decimal string".to_owned())?;
    let output = max_output_bytes
        .unwrap_or(max_source_bytes)
        .parse::<usize>()
        .map_err(|_| "max_output_bytes must be a platform-sized decimal string".to_owned())?;
    Ok(FootprintLimits {
        max_source_bytes: source,
        max_output_bytes: output,
        max_depth: max_depth as usize,
        max_properties: max_properties as usize,
        max_pads: max_pads as usize,
    })
}

fn validate_identity(type_: &str, version: &str, expected: &str) -> Result<(), String> {
    if type_ != expected || version != "a0" {
        return Err(format!("Unsupported contract identity: {type_}:{version}"));
    }
    Ok(())
}

fn build_sexpr_impl(request_json: &[u8]) -> Result<Vec<u8>, String> {
    let request: SExpressionBuildRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    let validated = validate_build_request(request).map_err(|error| error.to_string())?;
    let tree = core_node(validated.root);
    build_with_limit(&tree, validated.max_output_bytes)
        .map(String::into_bytes)
        .map_err(|error| error.to_string())
}

fn core_node(node: ValidatedNode) -> Sexp {
    match node {
        ValidatedNode::List(children) => Sexp::List(children.into_iter().map(core_node).collect()),
        ValidatedNode::Atom(value) => Sexp::Atom(value),
        ValidatedNode::Quoted(value) => Sexp::Quoted(value),
        ValidatedNode::Integer(value) => Sexp::Integer(value),
        ValidatedNode::Float(value) => Sexp::Float(value),
    }
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

fn footprint_diagnostic(error: Error) -> FootprintDiagnostic {
    FootprintDiagnostic {
        code: error_code(error.kind).to_owned(),
        message: error.message.into_owned(),
        phase: match error.phase {
            ErrorPhase::Lex => FootprintDiagnosticPhase::Lex,
            ErrorPhase::Tree => FootprintDiagnosticPhase::Tree,
            ErrorPhase::Build => FootprintDiagnosticPhase::Build,
        },
        position: error.position.map(|position| FootprintSourcePosition {
            column: position.column.to_string(),
            line: position.line.to_string(),
            offset: position.offset.to_string(),
        }),
        token: error.token,
    }
}

fn footprint_edit_diagnostic(error: Error) -> FootprintEditDiagnostic {
    FootprintEditDiagnostic {
        code: error_code(error.kind).to_owned(),
        message: error.message.into_owned(),
        phase: match error.phase {
            ErrorPhase::Lex => FootprintEditDiagnosticPhase::Lex,
            ErrorPhase::Tree => FootprintEditDiagnosticPhase::Tree,
            ErrorPhase::Build => FootprintEditDiagnosticPhase::Build,
        },
        position: error.position.map(|position| FootprintEditSourcePosition {
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
    use super::{
        build_sexpr, build_sexpr_impl, canonicalize_sexpr, edit_footprint_property, read_footprint,
        scan_sexpr, scan_sexpr_impl,
    };
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

    #[test]
    fn typed_build_validates_union_payloads_and_output_limits() {
        let request = br#"{
            "type":"kicad_monkey.sexpr_build.request",
            "version":"a0",
            "root":{"kind":"list","children":[
                {"kind":"atom","text":"root"},
                {"kind":"quoted","text":"a b"},
                {"kind":"integer","integer":"42"}
            ]},
            "max_output_bytes":"64",
            "max_depth":8,
            "max_nodes":8
        }"#;
        assert_eq!(
            build_sexpr_impl(request).expect("valid build"),
            b"(root \"a b\" 42)"
        );

        let conflict = request
            .windows(b"{\"kind\":\"atom\",\"text\":\"root\"}".len())
            .position(|window| window == b"{\"kind\":\"atom\",\"text\":\"root\"}")
            .expect("fixture contains atom");
        let mut invalid = request.to_vec();
        invalid.splice(
            conflict..conflict + b"{\"kind\":\"atom\",\"text\":\"root\"}".len(),
            b"{\"kind\":\"atom\",\"text\":\"root\",\"integer\":\"1\"}"
                .iter()
                .copied(),
        );
        assert!(
            build_sexpr_impl(&invalid)
                .expect_err("conflict")
                .contains("conflicting_payload")
        );

        let too_small = String::from_utf8(request.to_vec())
            .expect("JSON UTF-8")
            .replace("\"max_output_bytes\":\"64\"", "\"max_output_bytes\":\"4\"");
        assert!(
            build_sexpr_impl(too_small.as_bytes())
                .expect_err("output limit")
                .contains("max_output_bytes")
        );
    }

    #[wasm_bindgen_test]
    fn wasm_byte_input_produces_canonical_byte_output() {
        assert_eq!(
            canonicalize_sexpr(b"(root(child 1))").expect("valid source"),
            b"(root\n\t(child 1)\n)"
        );
    }

    #[wasm_bindgen_test]
    fn wasm_typed_build_produces_canonical_bytes() {
        let request = br#"{"type":"kicad_monkey.sexpr_build.request","version":"a0","root":{"kind":"list","children":[{"kind":"atom","text":"root"}]},"max_output_bytes":"64","max_depth":4,"max_nodes":4}"#;
        assert_eq!(
            build_sexpr(request).expect("valid build request"),
            b"(root)"
        );
    }

    #[wasm_bindgen_test]
    fn wasm_typed_scan_reports_resource_limits_in_the_contract_result() {
        let request = br#"{"type":"kicad_monkey.sexpr_scan.request","version":"a0","selector":{"heads":["footprint"]},"max_source_bytes":"8","max_depth":4,"max_selected_forms":4}"#;
        let result = scan_sexpr(b"(footprint \"too large\")", request)
            .expect("resource failures belong in the result envelope");
        let value: Value = serde_json::from_slice(&result).expect("result should be JSON");
        assert_eq!(value["forms"], serde_json::json!([]));
        assert_eq!(value["diagnostics"][0]["code"], "resource_limit");
        assert_eq!(value["diagnostics"][0]["phase"], "tree");
    }

    #[wasm_bindgen_test]
    fn wasm_footprint_read_and_source_preserving_edit_use_byte_boundaries() {
        let source =
            br#"(footprint "Demo" (property "Value" "old") (pad "1" smd rect) (future "keep"))"#;
        let read_request = br#"{"type":"kicad_monkey.footprint_read.request","version":"a0","max_source_bytes":"1024","max_depth":32,"max_properties":8,"max_pads":8}"#;
        let read = read_footprint(source, read_request).expect("typed footprint read");
        let value: Value = serde_json::from_slice(&read).expect("result JSON");
        assert_eq!(value["name"], "Demo");
        assert_eq!(value["properties"][0]["value"], "old");
        assert_eq!(value["pad_count"], 1);

        let edit_request = br#"{"type":"kicad_monkey.footprint_edit.request","version":"a0","property_name":"Value","value":"new value","max_source_bytes":"1024","max_output_bytes":"1024","max_depth":32,"max_properties":8,"max_pads":8}"#;
        let edited = edit_footprint_property(source, edit_request).expect("typed footprint edit");
        let metadata: Value =
            serde_json::from_slice(&edited.result_json()).expect("edit result JSON");
        let output = edited.output_bytes();
        assert_eq!(metadata["changed"], true);
        assert_eq!(metadata["output_bytes"], output.len().to_string());
        assert_eq!(metadata["diagnostics"], serde_json::json!([]));
        assert_eq!(
            output,
            br#"(footprint "Demo" (property "Value" "new value") (pad "1" smd rect) (future "keep"))"#
        );
    }

    #[wasm_bindgen_test]
    fn wasm_footprint_lazy_errors_are_structured_and_absolute() {
        let source = b"# prefix\n(footprint \"Demo\"\n  (property \"Value\")\n)";
        let request = br#"{"type":"kicad_monkey.footprint_read.request","version":"a0","max_source_bytes":"1024","max_depth":32,"max_properties":8,"max_pads":8}"#;
        let result = read_footprint(source, request).expect("model errors belong in result JSON");
        let value: Value = serde_json::from_slice(&result).expect("result JSON");
        assert_eq!(value["properties"], serde_json::json!([]));
        assert_eq!(value["diagnostics"][0]["code"], "unexpected_token");
        assert_eq!(value["diagnostics"][0]["position"]["line"], "3");
        assert_eq!(value["diagnostics"][0]["position"]["column"], "20");
        let property_start = source
            .windows(b"(property".len())
            .position(|window| window == b"(property")
            .expect("property start");
        let closing_offset = property_start
            + source[property_start..]
                .iter()
                .position(|byte| *byte == b')')
                .expect("property close");
        assert_eq!(
            value["diagnostics"][0]["position"]["offset"],
            closing_offset.to_string()
        );

        let foreign = b"(metadata (property \"Value\" \"wrong\"))\n(footprint \"Demo\")";
        let foreign_result =
            read_footprint(foreign, request).expect("extra root belongs in result JSON");
        let foreign_value: Value =
            serde_json::from_slice(&foreign_result).expect("foreign result JSON");
        assert_eq!(foreign_value["diagnostics"][0]["code"], "unexpected_token");
        assert_eq!(foreign_value["properties"], serde_json::json!([]));
    }

    #[wasm_bindgen_test]
    fn wasm_footprint_noop_edit_returns_metadata_and_original_bytes() {
        let source = br#"(footprint "Demo" (property "Value" "same"))"#;
        let request = br#"{"type":"kicad_monkey.footprint_edit.request","version":"a0","property_name":"Value","value":"same","max_source_bytes":"1024","max_output_bytes":"1024","max_depth":32,"max_properties":8,"max_pads":8}"#;
        let edited = edit_footprint_property(source, request).expect("no-op edit");
        let metadata: Value =
            serde_json::from_slice(&edited.result_json()).expect("edit result JSON");
        assert_eq!(metadata["changed"], false);
        assert_eq!(metadata["output_bytes"], source.len().to_string());
        assert_eq!(metadata["diagnostics"], serde_json::json!([]));
        assert_eq!(edited.output_bytes(), source);

        let missing_request = br#"{"type":"kicad_monkey.footprint_edit.request","version":"a0","property_name":"Missing","value":"same","max_source_bytes":"1024","max_output_bytes":"1024","max_depth":32,"max_properties":8,"max_pads":8}"#;
        let failed = edit_footprint_property(source, missing_request)
            .expect("model failures belong in edit metadata");
        let failed_metadata: Value =
            serde_json::from_slice(&failed.result_json()).expect("failed edit result JSON");
        assert_eq!(failed_metadata["changed"], false);
        assert_eq!(failed_metadata["output_bytes"], "0");
        assert_eq!(
            failed_metadata["diagnostics"][0]["code"],
            "unexpected_token"
        );
        assert!(failed.output_bytes().is_empty());
    }
}

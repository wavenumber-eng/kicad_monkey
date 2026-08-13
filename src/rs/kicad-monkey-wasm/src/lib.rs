//! Thin browser adapters over the shared safe Rust core.

#![forbid(unsafe_code)]

use kicad_monkey_contracts::generated::build_request::SExpressionBuildRequestA0;
use kicad_monkey_contracts::generated::footprint_edit_request::FootprintEditRequestA0;
use kicad_monkey_contracts::generated::footprint_edit_result::{
    Diagnostic as FootprintEditDiagnostic, DiagnosticPhase as FootprintEditDiagnosticPhase,
    FootprintEditResultA0, SourcePosition as FootprintEditSourcePosition,
};
use kicad_monkey_contracts::generated::footprint_plot_document::{
    ArcThreePointOperation, CircleOperation, FlashPadCircleOperation, FlashPadOvalOperation,
    FlashPadRectOperation, FlashPadRoundRectOperation, FlashPadTrapezOperation,
    FootprintPlotDocumentA0, FootprintPlotRecord, PlotPolyOperation, PlotterCoordinateSpace,
    PlotterDrillRole, PlotterFill, PlotterOperation, PlotterPoint, PlotterQuad, RectOperation,
    ThickSegmentOperation,
};
use kicad_monkey_contracts::generated::footprint_plot_request::FootprintPlotRequestA0;
use kicad_monkey_contracts::generated::footprint_plot_result::{
    Diagnostic as FootprintPlotDiagnostic, DiagnosticPhase as FootprintPlotDiagnosticPhase,
    FootprintPlotResultA0, SourcePosition as FootprintPlotSourcePosition,
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
use kicad_monkey_contracts::{
    JavaScriptSafeInteger, ValidatedNode, validate_build_request, validate_footprint_plot_document,
};
use kicad_monkey_core::{
    Error, ErrorKind, ErrorPhase, FootprintLimits, FootprintPlotLimits, FootprintView,
    PlotterFill as CorePlotterFill, PlotterOperation as CorePlotterOperation, ProjectionLimits,
    Selector, Sexp, build, build_with_limit, footprint_plot_document, parse_bytes,
    scan_reader_form_spans, utf8_text,
};
use std::collections::BTreeSet;
use std::io::{Cursor, Write};
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

/// Paired metadata and out-of-band plotter-IR JSON bytes.
#[wasm_bindgen]
pub struct FootprintPlotOutput {
    result_json: Vec<u8>,
    output_bytes: Vec<u8>,
}

#[wasm_bindgen]
impl FootprintPlotOutput {
    /// TypeSpec-governed `FootprintPlotResultA0` metadata bytes.
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

/// Convert supported standalone-footprint geometry to plotter-IR JSON bytes.
#[wasm_bindgen(js_name = plotFootprintIr)]
pub fn plot_footprint_ir(
    source: &[u8],
    request_json: &[u8],
) -> Result<FootprintPlotOutput, JsValue> {
    plot_footprint_ir_impl(source, request_json).map_err(|message| JsValue::from_str(&message))
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

fn plot_footprint_ir_impl(
    source: &[u8],
    request_json: &[u8],
) -> Result<FootprintPlotOutput, String> {
    let request: FootprintPlotRequestA0 =
        serde_json::from_slice(request_json).map_err(|error| error.to_string())?;
    validate_identity(
        &request.type_,
        &request.version,
        "kicad_monkey.footprint_plot.request",
    )?;
    let max_source_bytes = decimal_usize(&request.max_source_bytes, "max_source_bytes")?;
    let max_output_bytes = decimal_usize(&request.max_output_bytes, "max_output_bytes")?;
    let operation = (|| {
        let text = utf8_text(source)?;
        footprint_plot_document(
            text,
            FootprintPlotLimits {
                max_source_bytes,
                max_depth: request.max_depth as usize,
                max_metadata_forms: request.max_metadata_forms as usize,
                max_operations: request.max_operations as usize,
            },
        )
    })();
    let (result, output_bytes) = match operation {
        Ok(document) => {
            let total_operations = u32::try_from(document.operations.len()).unwrap_or(u32::MAX);
            let document_id = request.document_id.unwrap_or_else(|| document.name.clone());
            let operations = document
                .operations
                .into_iter()
                .enumerate()
                .map(|(index, operation)| contract_plotter_operation(index, operation))
                .collect::<Result<Vec<_>, String>>()?;
            let contract_document = FootprintPlotDocumentA0 {
                coordinate_space: PlotterCoordinateSpace {
                    unit: "nm".to_owned(),
                    y_axis: "down".to_owned(),
                },
                document_id,
                generator: document.generator,
                generator_version: document.generator_version,
                records: vec![FootprintPlotRecord {
                    attr: document.attr,
                    descr: document.descr,
                    kind: "footprint".to_owned(),
                    layer: document.layer,
                    locked: document.locked,
                    name: document.name.clone(),
                    object_id: document.name,
                    operation_count: total_operations,
                    operations,
                    placed: document.placed,
                    tags: document.tags,
                    uuid: document.uuid,
                }],
                schema: "kicad.plotter_ir.a0".to_owned(),
                source_kind: "MOD".to_owned(),
                source_path: request.source_path,
                total_operations,
                version: JavaScriptSafeInteger::try_from(document.version)
                    .map_err(|error| error.to_string())?,
            };
            validate_footprint_plot_document(&contract_document)
                .map_err(|error| error.to_string())?;
            match serialize_bounded(&contract_document, max_output_bytes)? {
                Some(output) => (
                    FootprintPlotResultA0 {
                        diagnostics: Vec::new(),
                        output_bytes: output.len().to_string(),
                        total_operations,
                        type_: "kicad_monkey.footprint_plot.result".to_owned(),
                        version: "a0".to_owned(),
                    },
                    output,
                ),
                None => (
                    FootprintPlotResultA0 {
                        diagnostics: vec![footprint_plot_limit_diagnostic()],
                        output_bytes: "0".to_owned(),
                        total_operations: 0,
                        type_: "kicad_monkey.footprint_plot.result".to_owned(),
                        version: "a0".to_owned(),
                    },
                    Vec::new(),
                ),
            }
        }
        Err(error) => (
            FootprintPlotResultA0 {
                diagnostics: vec![footprint_plot_diagnostic(error)],
                output_bytes: "0".to_owned(),
                total_operations: 0,
                type_: "kicad_monkey.footprint_plot.result".to_owned(),
                version: "a0".to_owned(),
            },
            Vec::new(),
        ),
    };
    Ok(FootprintPlotOutput {
        result_json: serde_json::to_vec(&result).map_err(|error| error.to_string())?,
        output_bytes,
    })
}

fn contract_plotter_operation(
    index: usize,
    operation: CorePlotterOperation,
) -> Result<PlotterOperation, String> {
    let index = u32::try_from(index).unwrap_or(u32::MAX);
    Ok(match operation {
        CorePlotterOperation::ThickSegment(operation) => ThickSegmentOperation {
            end_x: safe_integer(operation.end_x)?,
            end_y: safe_integer(operation.end_y)?,
            index,
            kind: "ThickSegment".to_owned(),
            layer: operation.layer,
            layers: operation.layers,
            mask_margin_nm: optional_safe_integer(operation.mask_margin_nm)?,
            pad_size_x_nm: optional_safe_integer(operation.pad_size_x_nm)?,
            pad_size_y_nm: optional_safe_integer(operation.pad_size_y_nm)?,
            role: contract_drill_role(operation.role.as_deref())?,
            start_x: safe_integer(operation.start_x)?,
            start_y: safe_integer(operation.start_y)?,
            width_nm: safe_integer(operation.width_nm)?,
        }
        .into(),
        CorePlotterOperation::ArcThreePoint(operation) => ArcThreePointOperation {
            end_x: safe_integer(operation.end_x)?,
            end_y: safe_integer(operation.end_y)?,
            fill: contract_fill(operation.fill),
            index,
            kind: "ArcThreePoint".to_owned(),
            layer: operation.layer,
            mid_x: safe_integer(operation.mid_x)?,
            mid_y: safe_integer(operation.mid_y)?,
            start_x: safe_integer(operation.start_x)?,
            start_y: safe_integer(operation.start_y)?,
            width_nm: safe_integer(operation.width_nm)?,
        }
        .into(),
        CorePlotterOperation::Circle(operation) => CircleOperation {
            cx: safe_integer(operation.cx)?,
            cy: safe_integer(operation.cy)?,
            diameter_nm: safe_integer(operation.diameter_nm)?,
            fill: contract_fill(operation.fill),
            index,
            kind: "Circle".to_owned(),
            layer: operation.layer,
            layers: operation.layers,
            mask_margin_nm: optional_safe_integer(operation.mask_margin_nm)?,
            pad_size_x_nm: optional_safe_integer(operation.pad_size_x_nm)?,
            pad_size_y_nm: optional_safe_integer(operation.pad_size_y_nm)?,
            role: contract_drill_role(operation.role.as_deref())?,
            width_nm: safe_integer(operation.width_nm)?,
        }
        .into(),
        CorePlotterOperation::Rect(operation) => RectOperation {
            corner_radius_nm: safe_integer(operation.corner_radius_nm)?,
            fill: contract_fill(operation.fill),
            index,
            kind: "Rect".to_owned(),
            layer: operation.layer,
            width_nm: safe_integer(operation.width_nm)?,
            x1: safe_integer(operation.x1)?,
            x2: safe_integer(operation.x2)?,
            y1: safe_integer(operation.y1)?,
            y2: safe_integer(operation.y2)?,
        }
        .into(),
        CorePlotterOperation::PlotPoly(operation) => PlotPolyOperation {
            fill: contract_fill(operation.fill),
            index,
            kind: "PlotPoly".to_owned(),
            layer: operation.layer,
            points: operation
                .points
                .into_iter()
                .map(|[x, y]| Ok(PlotterPoint::from([safe_integer(x)?, safe_integer(y)?])))
                .collect::<Result<Vec<_>, String>>()?,
            width_nm: safe_integer(operation.width_nm)?,
        }
        .into(),
        CorePlotterOperation::FlashPadCircle(operation) => FlashPadCircleOperation {
            diameter_nm: safe_integer(operation.diameter_nm)?,
            index,
            kind: "FlashPadCircle".to_owned(),
            layers: operation.layers,
            mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
            x: safe_integer(operation.x)?,
            y: safe_integer(operation.y)?,
        }
        .into(),
        CorePlotterOperation::FlashPadOval(operation) => FlashPadOvalOperation {
            index,
            kind: "FlashPadOval".to_owned(),
            layers: operation.layers,
            mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
            orient_deg: operation.orient_deg,
            size_x_nm: safe_integer(operation.size_x_nm)?,
            size_y_nm: safe_integer(operation.size_y_nm)?,
            x: safe_integer(operation.x)?,
            y: safe_integer(operation.y)?,
        }
        .into(),
        CorePlotterOperation::FlashPadRect(operation) => FlashPadRectOperation {
            index,
            kind: "FlashPadRect".to_owned(),
            layers: operation.layers,
            mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
            orient_deg: operation.orient_deg,
            size_x_nm: safe_integer(operation.size_x_nm)?,
            size_y_nm: safe_integer(operation.size_y_nm)?,
            x: safe_integer(operation.x)?,
            y: safe_integer(operation.y)?,
        }
        .into(),
        CorePlotterOperation::FlashPadRoundRect(operation) => FlashPadRoundRectOperation {
            corner_radius_nm: safe_integer(operation.corner_radius_nm)?,
            index,
            kind: "FlashPadRoundRect".to_owned(),
            layers: operation.layers,
            mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
            orient_deg: operation.orient_deg,
            size_x_nm: safe_integer(operation.size_x_nm)?,
            size_y_nm: safe_integer(operation.size_y_nm)?,
            x: safe_integer(operation.x)?,
            y: safe_integer(operation.y)?,
        }
        .into(),
        CorePlotterOperation::FlashPadTrapez(operation) => FlashPadTrapezOperation {
            corners: contract_quad(operation.corners)?,
            index,
            kind: "FlashPadTrapez".to_owned(),
            layers: operation.layers,
            mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
            orient_deg: operation.orient_deg,
            x: safe_integer(operation.x)?,
            y: safe_integer(operation.y)?,
        }
        .into(),
    })
}

fn contract_fill(fill: CorePlotterFill) -> PlotterFill {
    match fill {
        CorePlotterFill::NoFill => PlotterFill::NoFill,
        CorePlotterFill::FilledShape => PlotterFill::FilledShape,
    }
}

fn safe_integer(value: i64) -> Result<JavaScriptSafeInteger, String> {
    JavaScriptSafeInteger::try_from(value).map_err(|error| error.to_string())
}

fn optional_safe_integer(value: Option<i64>) -> Result<Option<JavaScriptSafeInteger>, String> {
    value.map(safe_integer).transpose()
}

fn contract_drill_role(value: Option<&str>) -> Result<Option<PlotterDrillRole>, String> {
    value
        .map(|role| PlotterDrillRole::try_from(role).map_err(|error| error.to_string()))
        .transpose()
}

fn contract_quad(corners: [[i64; 2]; 4]) -> Result<PlotterQuad, String> {
    let points = corners
        .into_iter()
        .map(|[x, y]| Ok(PlotterPoint::from([safe_integer(x)?, safe_integer(y)?])))
        .collect::<Result<Vec<_>, String>>()?;
    let points: [PlotterPoint; 4] = points
        .try_into()
        .map_err(|_| "plotter quad must contain four points".to_owned())?;
    Ok(PlotterQuad::from(points))
}

fn decimal_usize(value: &str, field: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{field} must be a platform-sized decimal string"))
}

fn serialize_bounded<T: serde::Serialize>(
    value: &T,
    max_output_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    let mut writer = BoundedWriter::new(max_output_bytes);
    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => Ok(Some(writer.bytes)),
        Err(_) if writer.exceeded => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(max_bytes.min(64 * 1024)),
            max_bytes,
            exceeded: false,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.max_bytes.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other(
                "serialized output exceeds max_output_bytes",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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

fn footprint_plot_diagnostic(error: Error) -> FootprintPlotDiagnostic {
    FootprintPlotDiagnostic {
        code: error_code(error.kind).to_owned(),
        message: error.message.into_owned(),
        phase: match error.phase {
            ErrorPhase::Lex => FootprintPlotDiagnosticPhase::Lex,
            ErrorPhase::Tree => FootprintPlotDiagnosticPhase::Tree,
            ErrorPhase::Build => FootprintPlotDiagnosticPhase::Build,
        },
        position: error.position.map(|position| FootprintPlotSourcePosition {
            column: position.column.to_string(),
            line: position.line.to_string(),
            offset: position.offset.to_string(),
        }),
        token: error.token,
    }
}

fn footprint_plot_limit_diagnostic() -> FootprintPlotDiagnostic {
    FootprintPlotDiagnostic {
        code: "resource_limit".to_owned(),
        message: "Plotter IR JSON exceeds max_output_bytes".to_owned(),
        phase: FootprintPlotDiagnosticPhase::Build,
        position: None,
        token: None,
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
        build_sexpr, build_sexpr_impl, canonicalize_sexpr, edit_footprint_property,
        plot_footprint_ir, plot_footprint_ir_impl, read_footprint, scan_sexpr, scan_sexpr_impl,
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

    #[test]
    fn footprint_plotter_host_path_emits_established_ir_shape_and_limits_output() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../../../../tests/parity/footprint_plotter_a0_vectors.json"
        ))
        .expect("shared footprint vectors");
        for vector in vectors["vectors"].as_array().expect("vectors") {
            let source = vector["source"].as_str().expect("source").as_bytes();
            let request = footprint_plot_request(vector, "16384", 1_000);
            let output = plot_footprint_ir_impl(source, &request).expect("plotter operation");
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
        let limited_request = footprint_plot_request(vector, "8", 8);
        let limited = plot_footprint_ir_impl(source, &limited_request)
            .expect("output limit belongs in result metadata");
        let limited_metadata: Value =
            serde_json::from_slice(&limited.result_json).expect("limited result JSON");
        assert_eq!(limited_metadata["diagnostics"][0]["code"], "resource_limit");
        assert_eq!(limited_metadata["diagnostics"][0]["phase"], "build");
        assert!(limited.output_bytes.is_empty());
    }

    fn footprint_plot_request(vector: &Value, max_output: &str, max_operations: u32) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "type": "kicad_monkey.footprint_plot.request",
            "version": "a0",
            "source_path": vector["source_path"],
            "document_id": vector["document_id"],
            "max_source_bytes": "16384",
            "max_output_bytes": max_output,
            "max_depth": 32,
            "max_metadata_forms": 32,
            "max_operations": max_operations
        }))
        .expect("request JSON")
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

    #[wasm_bindgen_test]
    fn wasm_footprint_plotter_returns_paired_ir_bytes() {
        let source = br#"(footprint "Demo"
          (fp_circle (center 2 2) (end 2 3) (stroke (width 0.1)) (fill yes))
          (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid)))
          (fp_rect (start 0 0) (end 1 1) (stroke (width 0.1)))
          (fp_arc (start 1 0) (mid 0 1) (end -1 0) (stroke (width 0.1)))
          (fp_poly (pts (xy 0 0) (xy 1 0) (xy 0 1)) (stroke (width 0.1)))
          (pad "1" smd roundrect (at 2 0 45) (size 1.5 0.8)
            (layers "F.Cu" "F.Mask") (roundrect_rratio 0.25)))"#;
        let request = br#"{"type":"kicad_monkey.footprint_plot.request","version":"a0","max_source_bytes":"4096","max_output_bytes":"4096","max_depth":32,"max_metadata_forms":32,"max_operations":8}"#;
        let output = plot_footprint_ir(source, request).expect("WASM plotter operation");
        let metadata: Value =
            serde_json::from_slice(&output.result_json()).expect("result metadata JSON");
        let document: Value =
            serde_json::from_slice(&output.output_bytes()).expect("plotter document JSON");
        assert_eq!(metadata["total_operations"], 6);
        assert_eq!(document["document_id"], "Demo");
        assert_eq!(document["records"][0]["operations"][0]["end_x"], 1_000_000);
        let kinds = document["records"][0]["operations"]
            .as_array()
            .expect("operation array")
            .iter()
            .map(|operation| operation["kind"].as_str().expect("operation kind"))
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                "ThickSegment",
                "ArcThreePoint",
                "Circle",
                "Rect",
                "PlotPoly",
                "FlashPadRoundRect"
            ]
        );
    }

    #[wasm_bindgen_test]
    fn wasm_footprint_plotter_rejects_precision_losing_integer_output() {
        let source = br#"(footprint "Demo" (version 9007199254740992))"#;
        let request = br#"{"type":"kicad_monkey.footprint_plot.request","version":"a0","max_source_bytes":"4096","max_output_bytes":"4096","max_depth":32,"max_metadata_forms":32,"max_operations":8}"#;
        let output = plot_footprint_ir(source, request).expect("range error result metadata");
        let metadata: Value =
            serde_json::from_slice(&output.result_json()).expect("result metadata JSON");
        assert_eq!(metadata["diagnostics"][0]["code"], "unexpected_token");
        assert!(
            metadata["diagnostics"][0]["message"]
                .as_str()
                .expect("diagnostic message")
                .contains("safe-integer")
        );
        assert!(output.output_bytes().is_empty());
    }

    #[wasm_bindgen_test]
    fn wasm_pattern_step_limit_returns_diagnostic_without_partial_geometry() {
        let source = br#"(footprint "Long"
          (fp_line (start 0 0) (end 7000000 0)
            (stroke (width 0.1) (type dash))))"#;
        let request = br#"{"type":"kicad_monkey.footprint_plot.request","version":"a0","max_source_bytes":"4096","max_output_bytes":"4096","max_depth":32,"max_metadata_forms":32,"max_operations":100000}"#;
        let output = plot_footprint_ir(source, request)
            .expect("decomposition limit belongs in result metadata");
        let metadata: Value =
            serde_json::from_slice(&output.result_json()).expect("result metadata JSON");
        assert_eq!(metadata["diagnostics"][0]["code"], "resource_limit");
        assert!(
            metadata["diagnostics"][0]["message"]
                .as_str()
                .expect("diagnostic message")
                .contains("safety step limit")
        );
        assert_eq!(metadata["total_operations"], 0);
        assert!(output.output_bytes().is_empty());
    }
}

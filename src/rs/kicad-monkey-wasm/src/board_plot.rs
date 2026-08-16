//! Browser adapter for the source-selected board plotter-IR producer.

use crate::{plotter_contract::contract_plotter_operation, serialize_bounded};
use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::board_plot_document::{
    BoardGraphicPlotRecord, BoardGraphicRecordKind, BoardPlotDocumentA0, BoardPlotRecord,
    BoardTextBoxPlotRecord, BoardTextPlotRecord, BoardViaType, CircleOperation,
    FlashPadCircleOperation, PlotterCoordinateSpace, PlotterDrillRole, PlotterFill,
    PlotterOperation, PlotterPoint, PlotterStringBool, PlotterTextHAlign, PlotterTextVAlign,
    PlotterViaFlashRole, TextOperation, TextRenderCache, TextRenderCachePolygon,
    TrackArcPlotRecord, TrackSegmentPlotRecord, ViaPlotRecord, ZoneFillPlotRecord,
};
use kicad_monkey_contracts::generated::board_plot_request::BoardPlotRequestA0;
use kicad_monkey_contracts::generated::board_plot_result::{
    BoardPlotResultA0, Diagnostic, DiagnosticPhase, SourcePosition,
};
use kicad_monkey_contracts::validate_board_plot_document;
use kicad_monkey_core::{
    BoardGraphicRecordKind as CoreGraphicRecordKind, BoardNetClassAssignments, BoardPlotLimits,
    BoardPlotRecord as CoreBoardPlotRecord, BoardTextBoxOperation, BoardTextHAlign,
    BoardTextOperation, BoardTextVAlign, BoardTextVariables, BoardViaOperation,
    BoardViaOperationKind, BoardViaRecord, Error, ErrorPhase, board_plot_document_with_sidecars,
    utf8_text,
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
            .map(|variable| (variable.name.clone(), variable.value.clone())),
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
    let mut records = Vec::with_capacity(document.records.len());
    for record in document.records {
        records.push(contract_record(record)?);
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

fn contract_record(record: CoreBoardPlotRecord) -> Result<BoardPlotRecord, String> {
    Ok(match record {
        CoreBoardPlotRecord::Graphic(record) => BoardGraphicPlotRecord {
            kind: contract_record_kind(record.kind),
            layer: Some(record.layer),
            object_id: record.kind.as_str().to_owned(),
            operation_count: contract_count(record.operations.len()),
            operations: shared_operations(record.operations)?,
            uuid: record.uuid,
        }
        .into(),
        CoreBoardPlotRecord::Segment(record) => TrackSegmentPlotRecord {
            kind: "segment".to_owned(),
            layer: record.layer,
            locked: record.locked,
            net_class: record.net_classes.net_class,
            net_classes: record.net_classes.net_classes,
            net_id: optional_safe_integer(record.net_id)?,
            net_name: record.net_name,
            object_id: "segment".to_owned(),
            operation_count: contract_count(record.operations.len()),
            operations: shared_operations(record.operations)?,
            uuid: record.uuid,
        }
        .into(),
        CoreBoardPlotRecord::TrackArc(record) => TrackArcPlotRecord {
            kind: "track_arc".to_owned(),
            layer: record.layer,
            net_class: record.net_classes.net_class,
            net_classes: record.net_classes.net_classes,
            net_id: optional_safe_integer(record.net_id)?,
            net_name: record.net_name,
            object_id: "track_arc".to_owned(),
            operation_count: contract_count(record.operations.len()),
            operations: shared_operations(record.operations)?,
            uuid: record.uuid,
        }
        .into(),
        CoreBoardPlotRecord::Text(record) => BoardTextPlotRecord {
            // Board gr_text carriers have no hide attribute upstream, so the
            // established serializer's getattr default is always false.
            hide: false,
            kind: "gr_text".to_owned(),
            layer: record.layer,
            object_id: "gr_text".to_owned(),
            operation_count: contract_count(record.operations.len()),
            operations: record
                .operations
                .into_iter()
                .enumerate()
                .map(|(index, operation)| {
                    contract_text_operation(index, operation).map(PlotterOperation::from)
                })
                .collect::<Result<Vec<_>, String>>()?,
            text: record.text,
            uuid: record.uuid,
        }
        .into(),
        CoreBoardPlotRecord::TextBox(record) => BoardTextBoxPlotRecord {
            border: record.border,
            kind: "gr_text_box".to_owned(),
            layer: record.layer,
            object_id: "gr_text_box".to_owned(),
            operation_count: contract_count(record.operations.len()),
            operations: record
                .operations
                .into_iter()
                .enumerate()
                .map(|(index, operation)| match operation {
                    BoardTextBoxOperation::Border(operation) => {
                        let shared = contract_plotter_operation(index, operation)?;
                        let value =
                            serde_json::to_value(shared).map_err(|error| error.to_string())?;
                        serde_json::from_value::<PlotterOperation>(value)
                            .map_err(|error| error.to_string())
                    }
                    BoardTextBoxOperation::Text(operation) => {
                        contract_text_operation(index, operation).map(PlotterOperation::from)
                    }
                })
                .collect::<Result<Vec<_>, String>>()?,
            text: record.text,
            uuid: record.uuid,
        }
        .into(),
        CoreBoardPlotRecord::Via(record) => contract_via_record(record)?.into(),
        CoreBoardPlotRecord::Zone(record) => ZoneFillPlotRecord {
            fill_island: record.fill_island,
            fill_layers: record.fill_layers,
            kind: "zone_fill".to_owned(),
            layers: record.layers,
            net_class: record.net_classes.net_class,
            net_classes: record.net_classes.net_classes,
            net_id: optional_safe_integer(record.net_id)?,
            net_name: record.net_name,
            object_id: "zone".to_owned(),
            operation_count: contract_count(record.operations.len()),
            operations: shared_operations(record.operations)?,
            uuid: record.uuid,
        }
        .into(),
    })
}

fn contract_points(points: Vec<[i64; 2]>) -> Result<Vec<PlotterPoint>, String> {
    points
        .into_iter()
        .map(|[x, y]| Ok(PlotterPoint([safe_integer(x)?, safe_integer(y)?])))
        .collect()
}

fn contract_text_operation(
    index: usize,
    operation: BoardTextOperation,
) -> Result<TextOperation, String> {
    // The established emitter serializes marker keys only when true.
    let marker = |value: bool| value.then_some(true);
    let render_cache = operation
        .render_cache
        .map(|cache| -> Result<TextRenderCache, String> {
            Ok(TextRenderCache {
                angle: cache.angle,
                coordinate_space: "board".to_owned(),
                exact: cache.exact,
                knockout: marker(cache.knockout),
                polygons: cache
                    .polygons
                    .into_iter()
                    .map(|contours| {
                        Ok(TextRenderCachePolygon {
                            contours: contours.into_iter().map(contract_points).collect::<Result<
                                Vec<_>,
                                String,
                            >>(
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                schema: "kicad.render_cache.v1".to_owned(),
                source: "existing_file_cache".to_owned(),
                text: cache.text,
                unit: "nm".to_owned(),
            })
        })
        .transpose()?;
    let render_cache_exact = render_cache.as_ref().map(|cache| cache.exact);
    let render_cache_source = render_cache
        .as_ref()
        .map(|_| "existing_file_cache".to_owned());
    Ok(TextOperation {
        bold: operation.bold,
        // PCB fonts carry no color attribute, so the established emitter
        // always falls back to the black default.
        color: "#000000".to_owned(),
        font_face: operation.font_face,
        h_align: match operation.h_align {
            BoardTextHAlign::Left => PlotterTextHAlign::GrTextHAlignLeft,
            BoardTextHAlign::Center => PlotterTextHAlign::GrTextHAlignCenter,
            BoardTextHAlign::Right => PlotterTextHAlign::GrTextHAlignRight,
        },
        index: contract_count(index),
        italic: operation.italic,
        kind: "Text".to_owned(),
        knockout: marker(operation.knockout),
        mirror: marker(operation.mirror),
        multiline: operation.multiline,
        orient_deg: operation.orient_deg,
        pen_width_nm: safe_integer(operation.pen_width_nm)?,
        polyline_per_segment: marker(operation.polyline_per_segment),
        render_cache,
        render_cache_exact,
        render_cache_polygons: operation
            .render_cache_polygons
            .into_iter()
            .map(contract_points)
            .collect::<Result<Vec<_>, String>>()?,
        render_cache_source,
        size_x_nm: safe_integer(operation.size_x_nm)?,
        size_y_nm: safe_integer(operation.size_y_nm)?,
        text: operation.text,
        text_as_polygons: marker(operation.text_as_polygons),
        v_align: match operation.v_align {
            BoardTextVAlign::Top => PlotterTextVAlign::GrTextVAlignTop,
            BoardTextVAlign::Center => PlotterTextVAlign::GrTextVAlignCenter,
            BoardTextVAlign::Bottom => PlotterTextVAlign::GrTextVAlignBottom,
        },
        x: safe_integer(operation.x)?,
        y: safe_integer(operation.y)?,
    })
}

fn contract_via_record(record: BoardViaRecord) -> Result<ViaPlotRecord, String> {
    let string_bool = |value: Option<bool>| {
        value.map(|value| {
            if value {
                PlotterStringBool::True
            } else {
                PlotterStringBool::False
            }
        })
    };
    let fabrication = record.fabrication;
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| contract_via_operation(index, operation))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ViaPlotRecord {
        drill: record.drill,
        hole_kind: "round".to_owned(),
        hole_plating: "plated".to_owned(),
        hole_render: "drill".to_owned(),
        ipc4761_capping: string_bool(fabrication.capping),
        ipc4761_covering_back: string_bool(fabrication.covering_back),
        ipc4761_covering_front: string_bool(fabrication.covering_front),
        ipc4761_filling: string_bool(fabrication.filling),
        ipc4761_metadata: fabrication.any().then(|| "true".to_owned()),
        ipc4761_plugging_back: string_bool(fabrication.plugging_back),
        ipc4761_plugging_front: string_bool(fabrication.plugging_front),
        ipc4761_tenting_back: string_bool(fabrication.tenting_back),
        ipc4761_tenting_front: string_bool(fabrication.tenting_front),
        kind: "via".to_owned(),
        layers: record.layers,
        net_class: record.net_classes.net_class,
        net_classes: record.net_classes.net_classes,
        net_id: optional_safe_integer(record.net_id)?,
        net_name: record.net_name,
        object_id: "via".to_owned(),
        operation_count: contract_count(operations.len()),
        operations,
        size: record.size,
        uuid: record.uuid,
        via_type: match record.via_type {
            kicad_monkey_core::BoardViaType::Through => BoardViaType::Through,
            kicad_monkey_core::BoardViaType::Blind => BoardViaType::Blind,
            kicad_monkey_core::BoardViaType::Buried => BoardViaType::Buried,
            kicad_monkey_core::BoardViaType::Micro => BoardViaType::Micro,
        },
    })
}

fn contract_via_operation(
    index: usize,
    operation: BoardViaOperation,
) -> Result<PlotterOperation, String> {
    let index = u32::try_from(index).unwrap_or(u32::MAX);
    let x = safe_integer(operation.x)?;
    let y = safe_integer(operation.y)?;
    let diameter_nm = safe_integer(operation.diameter_nm)?;
    Ok(match operation.kind {
        BoardViaOperationKind::Aperture | BoardViaOperationKind::MaskOpening => {
            FlashPadCircleOperation {
                diameter_nm,
                index,
                kind: "FlashPadCircle".to_owned(),
                layers: operation.layers,
                mask_margin_nm: None,
                role: Some(match operation.kind {
                    BoardViaOperationKind::Aperture => PlotterViaFlashRole::ViaAperture,
                    _ => PlotterViaFlashRole::ViaMaskOpening,
                }),
                x,
                y,
            }
            .into()
        }
        BoardViaOperationKind::Drill | BoardViaOperationKind::MaskDrill => CircleOperation {
            cx: x,
            cy: y,
            diameter_nm,
            fill: PlotterFill::FilledShape,
            fill_color: None,
            index,
            kind: "Circle".to_owned(),
            layer: None,
            // Present-but-empty layers match the established serializer's
            // verbatim via layer list, including unrouted vias.
            layers: Some(operation.layers),
            line_style: None,
            mask_margin_nm: None,
            pad_size_x_nm: None,
            pad_size_y_nm: None,
            role: Some(match operation.kind {
                BoardViaOperationKind::Drill => PlotterDrillRole::ViaDrill,
                _ => PlotterDrillRole::ViaMaskDrill,
            }),
            stroke_color: None,
            width_nm: JavaScriptSafeInteger::try_from(0).map_err(|error| error.to_string())?,
        }
        .into(),
    })
}

fn shared_operations(
    operations: Vec<kicad_monkey_core::PlotterOperation>,
) -> Result<Vec<PlotterOperation>, String> {
    // The established Python serializer numbers operations per record.
    operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| {
            let shared = contract_plotter_operation(index, operation)?;
            let value = serde_json::to_value(shared).map_err(|error| error.to_string())?;
            serde_json::from_value::<PlotterOperation>(value).map_err(|error| error.to_string())
        })
        .collect()
}

fn contract_count(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn safe_integer(value: i64) -> Result<JavaScriptSafeInteger, String> {
    JavaScriptSafeInteger::try_from(value).map_err(|error| error.to_string())
}

fn optional_safe_integer(value: Option<i64>) -> Result<Option<JavaScriptSafeInteger>, String> {
    value.map(safe_integer).transpose()
}

fn contract_record_kind(kind: CoreGraphicRecordKind) -> BoardGraphicRecordKind {
    match kind {
        CoreGraphicRecordKind::GrLine => BoardGraphicRecordKind::GrLine,
        CoreGraphicRecordKind::GrArc => BoardGraphicRecordKind::GrArc,
        CoreGraphicRecordKind::GrCircle => BoardGraphicRecordKind::GrCircle,
        CoreGraphicRecordKind::GrRect => BoardGraphicRecordKind::GrRect,
        CoreGraphicRecordKind::GrPoly => BoardGraphicRecordKind::GrPoly,
        CoreGraphicRecordKind::GrCurve => BoardGraphicRecordKind::GrCurve,
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

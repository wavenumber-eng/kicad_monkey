//! Bounded projection of the typed schematic plot model into the frozen a0 contract.

use crate::{
    PlotterCircle, PlotterFill, PlotterLineStyle, PlotterOperation, PlotterText, PlotterTextHAlign,
    PlotterTextVAlign, SchematicAnnotationRecord, SchematicConnectivityRecord,
    SchematicConnectivityRecordKind, SchematicPlotDocument, SchematicPlotOperation,
    SchematicPlotRecord, ThickSegment,
};
use serde_json::{Map, Value, json};
use std::fmt;
use std::io::{self, Write};
use std::mem::size_of;

// The converter briefly holds the source model, JSON Value tree, and decoded
// strict contract. Small plot documents have proportionally high map/vector
// overhead, so retain a conservative preflight ratio while still coupling a
// tiny output ceiling to bounded work before materialization.
const OUTPUT_TO_MODEL_RATIO: usize = 128;
const VALUE_ALLOCATION_MULTIPLIER: usize = 8;
const ROOT_VALUES: usize = 16;
const VALUES_PER_RECORD: usize = 32;
const VALUES_PER_OPERATION: usize = 32;
const VALUES_PER_POINT: usize = 2;

/// Resource ceilings for one typed plot-document contract projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicPlotContractLimits {
    pub max_derived_items: usize,
    pub max_materialized_bytes: usize,
    pub max_output_bytes: usize,
}

impl Default for SchematicPlotContractLimits {
    fn default() -> Self {
        Self {
            max_derived_items: 64_000_000,
            max_materialized_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
            max_output_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicPlotContractError(String);

impl fmt::Display for SchematicPlotContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SchematicPlotContractError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicPlotContractBudget {
    pub derived_items: usize,
    pub materialized_bytes: usize,
}

/// Project a typed schematic plot document into the strict a0 JSON vocabulary.
pub fn schematic_plot_document_json(
    document: &SchematicPlotDocument,
    limits: SchematicPlotContractLimits,
) -> Result<Value, SchematicPlotContractError> {
    let budget = schematic_plot_document_budget(document)?;
    validate_budget(budget, limits)?;
    let contract = crate::project_schematic_plot_document_a0(
        document,
        crate::PlotDocumentProjectionLimits {
            max_records: limits.max_derived_items,
            max_operations: limits.max_derived_items,
            max_points: limits.max_derived_items,
            max_string_bytes: limits.max_materialized_bytes,
            max_nested_items: limits.max_derived_items,
            max_materialized_bytes: limits.max_materialized_bytes,
        },
    )
    .map_err(|error| SchematicPlotContractError(error.to_string()))?;
    let value = serde_json::to_value(contract).map_err(|error| {
        SchematicPlotContractError(format!("schematic plot contract encode failed: {error}"))
    })?;
    let mut writer = LimitedWriter::new(limits.max_output_bytes);
    serde_json::to_writer(&mut writer, &value).map_err(|error| {
        SchematicPlotContractError(format!(
            "schematic plot contract output limit exceeded: {error}"
        ))
    })?;
    Ok(value)
}

pub fn schematic_plot_document_budget(
    document: &SchematicPlotDocument,
) -> Result<SchematicPlotContractBudget, SchematicPlotContractError> {
    let records = document.records.len();
    let operations = document.records.iter().try_fold(0_usize, |count, record| {
        count.checked_add(record.operation_count()).ok_or_else(|| {
            SchematicPlotContractError("schematic plot operation count overflow".to_owned())
        })
    })?;
    let points = document
        .records
        .iter()
        .flat_map(record_operations)
        .try_fold(0_usize, |count, operation| {
            count
                .checked_add(operation_point_count(operation))
                .ok_or_else(|| {
                    SchematicPlotContractError("schematic plot point count overflow".to_owned())
                })
        })?;
    let items = ROOT_VALUES
        .checked_add(
            records
                .checked_mul(VALUES_PER_RECORD)
                .ok_or_else(item_overflow)?,
        )
        .and_then(|value| {
            operations
                .checked_mul(VALUES_PER_OPERATION)
                .and_then(|count| value.checked_add(count))
        })
        .and_then(|value| {
            points
                .checked_mul(VALUES_PER_POINT)
                .and_then(|count| value.checked_add(count))
        })
        .ok_or_else(item_overflow)?;
    let payload = payload_bytes(document)?;
    let materialized = items
        .checked_mul(size_of::<Value>())
        .and_then(|bytes| bytes.checked_mul(VALUE_ALLOCATION_MULTIPLIER))
        .and_then(|bytes| {
            payload
                .checked_mul(VALUE_ALLOCATION_MULTIPLIER)
                .and_then(|payload| bytes.checked_add(payload))
        })
        .ok_or_else(item_overflow)?;
    Ok(SchematicPlotContractBudget {
        derived_items: items,
        materialized_bytes: materialized,
    })
}

fn validate_budget(
    budget: SchematicPlotContractBudget,
    limits: SchematicPlotContractLimits,
) -> Result<(), SchematicPlotContractError> {
    if budget.derived_items > limits.max_derived_items {
        return Err(SchematicPlotContractError(format!(
            "schematic plot derived item limit exceeded: {} > {}",
            budget.derived_items, limits.max_derived_items
        )));
    }
    let effective_limit = limits.max_materialized_bytes.min(
        limits
            .max_output_bytes
            .saturating_mul(OUTPUT_TO_MODEL_RATIO),
    );
    if budget.materialized_bytes > effective_limit {
        return Err(SchematicPlotContractError(format!(
            "schematic plot materialized byte limit exceeded: {} > {effective_limit}",
            budget.materialized_bytes
        )));
    }
    Ok(())
}

fn payload_bytes(document: &SchematicPlotDocument) -> Result<usize, SchematicPlotContractError> {
    let mut bytes = 0_usize;
    add_optional_text(&mut bytes, document.source_path.as_deref())?;
    add_text(&mut bytes, &document.document_id)?;
    for record in &document.records {
        match record {
            SchematicPlotRecord::SheetHeader(value) => {
                add_texts(
                    &mut bytes,
                    [
                        value.uuid.as_str(),
                        value.paper_size.as_str(),
                        value.generator.as_str(),
                        value.generator_version.as_str(),
                    ],
                )?;
                if let Some(title) = &value.title_block {
                    add_texts(
                        &mut bytes,
                        [
                            title.title.as_str(),
                            title.date.as_str(),
                            title.revision.as_str(),
                            title.company.as_str(),
                        ],
                    )?;
                    for comment in title.comments.values() {
                        add_text(&mut bytes, comment)?;
                    }
                }
            }
            SchematicPlotRecord::Connectivity(value) => {
                add_text(&mut bytes, &value.uuid)?;
                add_optional_text(&mut bytes, value.junction_color.as_deref())?;
            }
            SchematicPlotRecord::Annotation(value) => {
                add_texts(&mut bytes, [value.uuid.as_str(), value.object_id.as_str()])?;
                add_optional_text(&mut bytes, value.text.as_deref())?;
                add_optional_text(&mut bytes, value.shape.as_deref())?;
            }
            SchematicPlotRecord::Graphic(value) => add_text(&mut bytes, &value.uuid)?,
            SchematicPlotRecord::RuleArea(value) => add_text(&mut bytes, &value.uuid)?,
            SchematicPlotRecord::Image(value) => {
                add_texts(
                    &mut bytes,
                    [value.uuid.as_str(), value.image_format.as_str()],
                )?;
            }
            SchematicPlotRecord::Table(value) => add_text(&mut bytes, &value.uuid)?,
            SchematicPlotRecord::SymbolInstance(value) => add_texts(
                &mut bytes,
                [
                    value.uuid.as_str(),
                    value.lib_id.as_str(),
                    value.lib_name.as_str(),
                    value.reference.as_str(),
                ],
            )?,
            SchematicPlotRecord::SymbolOverplot(value) => add_texts(
                &mut bytes,
                [
                    value.uuid.as_str(),
                    value.source_symbol_uuid.as_str(),
                    value.lib_id.as_str(),
                ],
            )?,
            SchematicPlotRecord::Sheet(value) => add_texts(
                &mut bytes,
                [
                    value.uuid.as_str(),
                    value.sheet_name.as_str(),
                    value.sheet_file.as_str(),
                ],
            )?,
        }
        if let SchematicPlotRecord::SymbolInstance(value) = record {
            add_optional_text(&mut bytes, value.mirror.as_deref())?;
        }
        for operation in record_operations(record) {
            operation_payload_bytes(&mut bytes, operation)?;
        }
    }
    Ok(bytes)
}

fn operation_payload_bytes(
    bytes: &mut usize,
    operation: &SchematicPlotOperation,
) -> Result<(), SchematicPlotContractError> {
    match operation {
        SchematicPlotOperation::Text(value) => {
            plotter_text_payload_bytes(bytes, &value.text)?;
            add_optional_text(bytes, value.hyperlink_href.as_deref())?;
        }
        SchematicPlotOperation::StyledThickSegment(value) => {
            thick_segment_payload_bytes(bytes, &value.segment)?;
            add_text(bytes, &value.stroke_color)?;
        }
        SchematicPlotOperation::PlotImage(value) => {
            add_texts(
                bytes,
                [value.image_data_b64.as_str(), value.image_format.as_str()],
            )?;
            add_optional_text(bytes, value.stroke_color.as_deref())?;
        }
        SchematicPlotOperation::StartSymbolPinBlock(value) => {
            add_texts(
                bytes,
                [
                    value.label.as_str(),
                    value.data_uuid.as_str(),
                    value.object_id.as_str(),
                    value.extra_attrs.primitive.as_str(),
                    value.extra_attrs.object_type.as_str(),
                    value.extra_attrs.pin.as_str(),
                    value.extra_attrs.symbol_uuid.as_str(),
                    value.extra_attrs.designator.as_str(),
                    value.extra_attrs.lib_pin_uuid.as_str(),
                ],
            )?;
        }
        SchematicPlotOperation::StartSheetPinBlock(value) => {
            add_texts(
                bytes,
                [
                    value.label.as_str(),
                    value.data_uuid.as_str(),
                    value.object_id.as_str(),
                    value.extra_attrs.primitive.as_str(),
                    value.extra_attrs.object_type.as_str(),
                    value.extra_attrs.sheet_uuid.as_str(),
                    value.extra_attrs.sheet_name.as_str(),
                    value.extra_attrs.sheet_file.as_str(),
                    value.extra_attrs.pin.as_str(),
                    value.extra_attrs.pin_name.as_str(),
                    value.extra_attrs.shape.as_str(),
                ],
            )?;
        }
        SchematicPlotOperation::EndBlock => {}
        SchematicPlotOperation::Plotter(operation) => {
            plotter_operation_payload_bytes(bytes, operation)?;
        }
    }
    Ok(())
}

fn plotter_operation_payload_bytes(
    bytes: &mut usize,
    operation: &PlotterOperation,
) -> Result<(), SchematicPlotContractError> {
    match operation {
        PlotterOperation::Rect(value) => add_style_payload(
            bytes,
            value.layer.as_deref(),
            value.stroke_color.as_deref(),
            value.fill_color.as_deref(),
        )?,
        PlotterOperation::PlotPoly(value) => add_style_payload(
            bytes,
            value.layer.as_deref(),
            value.stroke_color.as_deref(),
            value.fill_color.as_deref(),
        )?,
        PlotterOperation::Circle(value) => {
            add_style_payload(
                bytes,
                value.layer.as_deref(),
                value.stroke_color.as_deref(),
                value.fill_color.as_deref(),
            )?;
            add_optional_text(bytes, value.role.as_deref())?;
            for layer in &value.layers {
                add_text(bytes, layer)?;
            }
        }
        PlotterOperation::Text(value) => plotter_text_payload_bytes(bytes, value)?,
        PlotterOperation::ArcThreePoint(value) => add_style_payload(
            bytes,
            value.layer.as_deref(),
            value.stroke_color.as_deref(),
            value.fill_color.as_deref(),
        )?,
        PlotterOperation::BezierCurve(value) => {
            add_optional_text(bytes, value.layer.as_deref())?;
            add_optional_text(bytes, value.stroke_color.as_deref())?;
        }
        PlotterOperation::ThickSegment(value) => thick_segment_payload_bytes(bytes, value)?,
        PlotterOperation::FlashPadCircle(_)
        | PlotterOperation::FlashPadOval(_)
        | PlotterOperation::FlashPadRect(_)
        | PlotterOperation::FlashPadRoundRect(_)
        | PlotterOperation::FlashPadCustom(_)
        | PlotterOperation::FlashPadTrapez(_) => {
            return Err(SchematicPlotContractError(
                "operation is outside the schematic plot vocabulary".to_owned(),
            ));
        }
    }
    Ok(())
}

fn plotter_text_payload_bytes(
    bytes: &mut usize,
    value: &PlotterText,
) -> Result<(), SchematicPlotContractError> {
    add_texts(
        bytes,
        [
            value.text.as_str(),
            value.color.as_str(),
            value.font_face.as_str(),
        ],
    )?;
    add_optional_text(bytes, value.layer.as_deref())
}

fn thick_segment_payload_bytes(
    bytes: &mut usize,
    value: &ThickSegment,
) -> Result<(), SchematicPlotContractError> {
    add_optional_text(bytes, value.layer.as_deref())?;
    add_optional_text(bytes, value.role.as_deref())?;
    for layer in &value.layers {
        add_text(bytes, layer)?;
    }
    Ok(())
}

fn add_style_payload(
    bytes: &mut usize,
    layer: Option<&str>,
    stroke: Option<&str>,
    fill: Option<&str>,
) -> Result<(), SchematicPlotContractError> {
    add_optional_text(bytes, layer)?;
    add_optional_text(bytes, stroke)?;
    add_optional_text(bytes, fill)
}

fn add_texts<'a>(
    bytes: &mut usize,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(), SchematicPlotContractError> {
    for value in values {
        add_text(bytes, value)?;
    }
    Ok(())
}

fn add_optional_text(
    bytes: &mut usize,
    value: Option<&str>,
) -> Result<(), SchematicPlotContractError> {
    value.map_or(Ok(()), |value| add_text(bytes, value))
}

fn add_text(bytes: &mut usize, value: &str) -> Result<(), SchematicPlotContractError> {
    *bytes = bytes.checked_add(value.len()).ok_or_else(item_overflow)?;
    Ok(())
}

fn item_overflow() -> SchematicPlotContractError {
    SchematicPlotContractError("schematic plot materialization count overflow".to_owned())
}

fn record_operations(record: &SchematicPlotRecord) -> std::slice::Iter<'_, SchematicPlotOperation> {
    match record {
        SchematicPlotRecord::SheetHeader(value) => value.operations.iter(),
        SchematicPlotRecord::Connectivity(value) => value.operations.iter(),
        SchematicPlotRecord::Annotation(value) => value.operations.iter(),
        SchematicPlotRecord::Graphic(value) => value.operations.iter(),
        SchematicPlotRecord::RuleArea(value) => value.operations.iter(),
        SchematicPlotRecord::Image(value) => value.operations.iter(),
        SchematicPlotRecord::Table(value) => value.operations.iter(),
        SchematicPlotRecord::SymbolInstance(value) => value.operations.iter(),
        SchematicPlotRecord::SymbolOverplot(value) => value.operations.iter(),
        SchematicPlotRecord::Sheet(value) => value.operations.iter(),
    }
}

fn operation_point_count(operation: &SchematicPlotOperation) -> usize {
    let SchematicPlotOperation::Plotter(operation) = operation else {
        return 0;
    };
    match operation {
        PlotterOperation::PlotPoly(value) => value.points.len(),
        PlotterOperation::BezierCurve(_) | PlotterOperation::FlashPadCustom(_) => 4,
        PlotterOperation::ArcThreePoint(_) => 3,
        _ => 0,
    }
}

#[allow(
    dead_code,
    reason = "retained temporarily as a parity oracle during typed migration"
)]
mod legacy_json_mapping {
    use super::*;

    fn insert_optional(object: &mut Map<String, Value>, name: &str, value: Option<Value>) {
        if let Some(value) = value {
            object.insert(name.to_owned(), value);
        }
    }

    const fn fill_name(fill: PlotterFill) -> &'static str {
        match fill {
            PlotterFill::NoFill => "NO_FILL",
            PlotterFill::FilledShape => "FILLED_SHAPE",
            PlotterFill::FilledWithBackgroundBodyColor => "FILLED_WITH_BG_BODYCOLOR",
            PlotterFill::FilledWithColor => "FILLED_WITH_COLOR",
            PlotterFill::Hatch => "HATCH",
            PlotterFill::ReverseHatch => "REVERSE_HATCH",
            PlotterFill::CrossHatch => "CROSS_HATCH",
        }
    }

    const fn line_style_name(style: PlotterLineStyle) -> &'static str {
        match style {
            PlotterLineStyle::Default => "DEFAULT",
            PlotterLineStyle::Solid => "SOLID",
            PlotterLineStyle::Dash => "DASH",
            PlotterLineStyle::Dot => "DOT",
            PlotterLineStyle::DashDot => "DASH_DOT",
            PlotterLineStyle::DashDotDot => "DASH_DOT_DOT",
        }
    }

    const fn h_align_name(align: PlotterTextHAlign) -> &'static str {
        match align {
            PlotterTextHAlign::Left => "GR_TEXT_H_ALIGN_LEFT",
            PlotterTextHAlign::Center => "GR_TEXT_H_ALIGN_CENTER",
            PlotterTextHAlign::Right => "GR_TEXT_H_ALIGN_RIGHT",
        }
    }

    const fn v_align_name(align: PlotterTextVAlign) -> &'static str {
        match align {
            PlotterTextVAlign::Top => "GR_TEXT_V_ALIGN_TOP",
            PlotterTextVAlign::Center => "GR_TEXT_V_ALIGN_CENTER",
            PlotterTextVAlign::Bottom => "GR_TEXT_V_ALIGN_BOTTOM",
        }
    }

    fn text_json(value: &PlotterText, index: usize, hyperlink: Option<&str>) -> Value {
        let mut object = json!({
            "kind": "Text", "index": index, "x": value.x, "y": value.y,
            "text": value.text, "color": value.color, "orient_deg": value.orient_deg,
            "size_x_nm": value.size_x_nm, "size_y_nm": value.size_y_nm,
            "h_align": h_align_name(value.h_align), "v_align": v_align_name(value.v_align),
            "pen_width_nm": value.pen_width_nm, "italic": value.italic, "bold": value.bold,
            "multiline": value.multiline, "font_face": value.font_face,
        })
        .as_object()
        .expect("text JSON object")
        .clone();
        insert_optional(
            &mut object,
            "layer",
            value.layer.as_ref().map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "context",
            hyperlink.map(|href| json!({"hyperlink": {"href": href}})),
        );
        Value::Object(object)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive contract mapping is a compile-time ratchet"
    )]
    fn operation_json(operation: &SchematicPlotOperation, index: usize) -> Value {
        match operation {
            SchematicPlotOperation::Text(value) => {
                text_json(&value.text, index, value.hyperlink_href.as_deref())
            }
            SchematicPlotOperation::StyledThickSegment(value) => {
                let segment = &value.segment;
                json!({"kind": "ThickSegment", "index": index,
                "start_x": segment.start_x, "start_y": segment.start_y,
                "end_x": segment.end_x, "end_y": segment.end_y,
                "width_nm": segment.width_nm, "stroke_color": value.stroke_color})
            }
            SchematicPlotOperation::PlotImage(value) => {
                let mut object = json!({"kind": "PlotImage", "index": index,
                "x": value.x, "y": value.y, "width_nm": value.width_nm,
                "height_nm": value.height_nm, "scale": value.scale,
                "image_data_b64": value.image_data_b64, "image_format": value.image_format})
                .as_object()
                .expect("image JSON object")
                .clone();
                insert_optional(
                    &mut object,
                    "stroke_color",
                    value.stroke_color.as_ref().map(|value| json!(value)),
                );
                Value::Object(object)
            }
            SchematicPlotOperation::StartSymbolPinBlock(value) => {
                let attrs = [
                    ("primitive", value.extra_attrs.primitive.as_str()),
                    ("object-type", value.extra_attrs.object_type.as_str()),
                    ("pin", value.extra_attrs.pin.as_str()),
                    ("symbol-uuid", value.extra_attrs.symbol_uuid.as_str()),
                    ("designator", value.extra_attrs.designator.as_str()),
                    ("lib-pin-uuid", value.extra_attrs.lib_pin_uuid.as_str()),
                ]
                .into_iter()
                .filter(|(_, value)| !value.is_empty())
                .map(|(name, value)| (name.to_owned(), json!(value)))
                .collect::<Map<_, _>>();
                json!({"kind": "StartBlock", "index": index, "label": value.label,
                "data_uuid": value.data_uuid, "data_ref": "symbol_pin",
                "object_id": value.object_id, "extra_attrs": attrs})
            }
            SchematicPlotOperation::StartSheetPinBlock(value) => {
                let attrs = [
                    ("primitive", value.extra_attrs.primitive.as_str()),
                    ("object-type", value.extra_attrs.object_type.as_str()),
                    ("sheet-uuid", value.extra_attrs.sheet_uuid.as_str()),
                    ("sheet-name", value.extra_attrs.sheet_name.as_str()),
                    ("sheet-file", value.extra_attrs.sheet_file.as_str()),
                    ("pin", value.extra_attrs.pin.as_str()),
                    ("pin-name", value.extra_attrs.pin_name.as_str()),
                    ("shape", value.extra_attrs.shape.as_str()),
                ]
                .into_iter()
                .filter(|(_, value)| !value.is_empty())
                .map(|(name, value)| (name.to_owned(), json!(value)))
                .collect::<Map<_, _>>();
                json!({"kind": "StartBlock", "index": index, "label": value.label,
                "data_uuid": value.data_uuid, "data_ref": "sheet_pin",
                "object_id": value.object_id, "extra_attrs": attrs})
            }
            SchematicPlotOperation::EndBlock => json!({"kind": "EndBlock", "index": index}),
            SchematicPlotOperation::Plotter(operation) => plotter_operation_json(operation, index),
        }
    }

    fn plotter_operation_json(operation: &PlotterOperation, index: usize) -> Value {
        match operation {
            PlotterOperation::Rect(value) => styled_shape_json(
                json!({"kind": "Rect", "index": index, "x1": value.x1, "y1": value.y1,
                "x2": value.x2, "y2": value.y2, "fill": fill_name(value.fill),
                "width_nm": value.width_nm, "corner_radius_nm": value.corner_radius_nm}),
                value.layer.as_deref(),
                value.stroke_color.as_deref(),
                value.fill_color.as_deref(),
                value.line_style,
            ),
            PlotterOperation::PlotPoly(value) => styled_shape_json(
                json!({"kind": "PlotPoly", "index": index, "points": value.points,
                "fill": fill_name(value.fill), "width_nm": value.width_nm}),
                value.layer.as_deref(),
                value.stroke_color.as_deref(),
                value.fill_color.as_deref(),
                value.line_style,
            ),
            PlotterOperation::Circle(value) => circle_json(value, index),
            PlotterOperation::Text(value) => text_json(value, index, None),
            PlotterOperation::ArcThreePoint(value) => styled_shape_json(
                json!({"kind": "ArcThreePoint", "index": index,
                "start_x": value.start_x, "start_y": value.start_y,
                "mid_x": value.mid_x, "mid_y": value.mid_y,
                "end_x": value.end_x, "end_y": value.end_y,
                "fill": fill_name(value.fill), "width_nm": value.width_nm}),
                value.layer.as_deref(),
                value.stroke_color.as_deref(),
                value.fill_color.as_deref(),
                value.line_style,
            ),
            PlotterOperation::BezierCurve(value) => styled_shape_json(
                json!({"kind": "BezierCurve", "index": index,
                "start_x": value.start_x, "start_y": value.start_y,
                "ctrl1_x": value.ctrl1_x, "ctrl1_y": value.ctrl1_y,
                "ctrl2_x": value.ctrl2_x, "ctrl2_y": value.ctrl2_y,
                "end_x": value.end_x, "end_y": value.end_y,
                "width_nm": value.width_nm, "tolerance_nm": value.tolerance_nm}),
                value.layer.as_deref(),
                value.stroke_color.as_deref(),
                None,
                value.line_style,
            ),
            PlotterOperation::ThickSegment(value) => thick_segment_json(value, index),
            PlotterOperation::FlashPadCircle(_)
            | PlotterOperation::FlashPadOval(_)
            | PlotterOperation::FlashPadRect(_)
            | PlotterOperation::FlashPadRoundRect(_)
            | PlotterOperation::FlashPadCustom(_)
            | PlotterOperation::FlashPadTrapez(_) => {
                unreachable!(
                    "operations outside the schematic vocabulary are rejected by the producer"
                )
            }
        }
    }

    fn styled_shape_json(
        base: Value,
        layer: Option<&str>,
        stroke_color: Option<&str>,
        fill_color: Option<&str>,
        line_style: Option<PlotterLineStyle>,
    ) -> Value {
        let mut object = base.as_object().expect("shape JSON object").clone();
        insert_optional(&mut object, "layer", layer.map(|value| json!(value)));
        insert_optional(
            &mut object,
            "stroke_color",
            stroke_color.map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "fill_color",
            fill_color.map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "line_style",
            line_style.map(|value| json!(line_style_name(value))),
        );
        Value::Object(object)
    }

    fn circle_json(value: &PlotterCircle, index: usize) -> Value {
        let styled = styled_shape_json(
            json!({"kind": "Circle", "index": index, "cx": value.cx, "cy": value.cy,
            "diameter_nm": value.diameter_nm, "fill": fill_name(value.fill),
            "width_nm": value.width_nm}),
            value.layer.as_deref(),
            value.stroke_color.as_deref(),
            value.fill_color.as_deref(),
            value.line_style,
        );
        let mut object = styled.as_object().expect("circle JSON object").clone();
        insert_pad_fields(
            &mut object,
            value.role.as_deref(),
            &value.layers,
            value.mask_margin_nm,
            value.pad_size_x_nm,
            value.pad_size_y_nm,
        );
        Value::Object(object)
    }

    fn thick_segment_json(value: &ThickSegment, index: usize) -> Value {
        let mut object = json!({"kind": "ThickSegment", "index": index,
        "start_x": value.start_x, "start_y": value.start_y,
        "end_x": value.end_x, "end_y": value.end_y, "width_nm": value.width_nm})
        .as_object()
        .expect("segment JSON object")
        .clone();
        insert_optional(
            &mut object,
            "layer",
            value.layer.as_deref().map(|value| json!(value)),
        );
        insert_pad_fields(
            &mut object,
            value.role.as_deref(),
            &value.layers,
            value.mask_margin_nm,
            value.pad_size_x_nm,
            value.pad_size_y_nm,
        );
        Value::Object(object)
    }

    fn insert_pad_fields(
        object: &mut Map<String, Value>,
        role: Option<&str>,
        layers: &[String],
        mask_margin_nm: Option<i64>,
        pad_size_x_nm: Option<i64>,
        pad_size_y_nm: Option<i64>,
    ) {
        insert_optional(object, "role", role.map(|value| json!(value)));
        if !layers.is_empty() {
            object.insert("layers".to_owned(), json!(layers));
        }
        insert_optional(
            object,
            "mask_margin_nm",
            mask_margin_nm.map(|value| json!(value)),
        );
        insert_optional(
            object,
            "pad_size_x_nm",
            pad_size_x_nm.map(|value| json!(value)),
        );
        insert_optional(
            object,
            "pad_size_y_nm",
            pad_size_y_nm.map(|value| json!(value)),
        );
    }

    fn document_json(document: &SchematicPlotDocument) -> Value {
        let records = document.records.iter().map(record_json).collect::<Vec<_>>();
        let mut object = json!({
        "schema": "kicad.plotter_ir.a0", "source_kind": "SCH",
        "total_operations": document.records.iter().map(SchematicPlotRecord::operation_count).sum::<usize>(),
        "records": records, "document_id": document.document_id,
        "canvas": {"width_nm": document.canvas.width_nm, "height_nm": document.canvas.height_nm},
        "coordinate_space": {"unit": "nm", "y_axis": "down"},
    }).as_object().expect("document JSON object").clone();
        insert_optional(
            &mut object,
            "source_path",
            document.source_path.as_ref().map(|value| json!(value)),
        );
        Value::Object(object)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive record mapping is a compile-time ratchet"
    )]
    fn record_json(record: &SchematicPlotRecord) -> Value {
        match record {
            SchematicPlotRecord::SheetHeader(value) => header_record_json(value),
            SchematicPlotRecord::Connectivity(value) => connectivity_record_json(value),
            SchematicPlotRecord::Annotation(value) => annotation_record_json(value),
            SchematicPlotRecord::Graphic(value) => json!({
                "uuid": value.uuid, "kind": value.kind.as_str(), "object_id": value.uuid,
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
            }),
            SchematicPlotRecord::RuleArea(value) => json!({
                "uuid": value.uuid, "kind": "rule_area", "object_id": value.uuid,
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
                "shape": value.shape.as_str(), "locked": value.locked,
                "exclude_from_sim": value.exclude_from_sim, "in_bom": value.in_bom,
                "on_board": value.on_board, "dnp": value.dnp,
            }),
            SchematicPlotRecord::Image(value) => json!({
                "uuid": value.uuid, "kind": "image", "object_id": value.uuid,
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
                "scale": value.scale, "image_format": value.image_format,
                "width_nm": value.width_nm, "height_nm": value.height_nm,
            }),
            SchematicPlotRecord::Table(value) => json!({
                "uuid": value.uuid, "kind": "table", "object_id": value.uuid,
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
                "cell_count": value.cell_count,
            }),
            SchematicPlotRecord::SymbolInstance(value) => json!({
                "uuid": value.uuid, "kind": "symbol_instance",
                "object_id": if value.lib_id.is_empty() { &value.uuid } else { &value.lib_id },
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
                "lib_id": value.lib_id, "lib_name": value.lib_name, "reference": value.reference,
                "at_x_nm": value.at_x_nm, "at_y_nm": value.at_y_nm,
                "at_angle_deg": value.at_angle_deg, "mirror": value.mirror,
                "unit": value.unit, "convert": value.convert, "in_bom": value.in_bom,
                "on_board": value.on_board, "dnp": value.dnp,
                "exclude_from_sim": value.exclude_from_sim, "in_pos_files": value.in_pos_files,
            }),
            SchematicPlotRecord::SymbolOverplot(value) => json!({
                "uuid": value.uuid, "kind": "symbol_overplot",
                "object_id": if value.lib_id.is_empty() { &value.source_symbol_uuid } else { &value.lib_id },
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
                "source_symbol_uuid": value.source_symbol_uuid, "lib_id": value.lib_id,
            }),
            SchematicPlotRecord::Sheet(value) => json!({
                "uuid": value.uuid, "kind": "sheet", "object_id": value.sheet_name,
                "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
                "sheet_name": value.sheet_name, "sheet_file": value.sheet_file,
                "at_x_nm": value.at_x_nm, "at_y_nm": value.at_y_nm,
                "size_x_nm": value.size_x_nm, "size_y_nm": value.size_y_nm, "dnp": value.dnp,
            }),
        }
    }

    fn operations_json(operations: &[SchematicPlotOperation]) -> Vec<Value> {
        operations
            .iter()
            .enumerate()
            .map(|(index, value)| operation_json(value, index))
            .collect()
    }

    fn header_record_json(value: &crate::SchematicSheetHeaderRecord) -> Value {
        let mut object = json!({
        "uuid": value.uuid, "kind": "sheet_header", "object_id": value.uuid,
        "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
        "paper_size": value.paper_size, "paper_width_mm": value.paper_width_mm,
        "paper_height_mm": value.paper_height_mm, "paper_portrait": value.paper_portrait,
        "sheet_width_nm": value.sheet_width_nm, "sheet_height_nm": value.sheet_height_nm,
        "version": value.version, "generator": value.generator,
        "generator_version": value.generator_version,
    })
    .as_object()
    .expect("header JSON object")
    .clone();
        insert_optional(
            &mut object,
            "title_block",
            value.title_block.as_ref().map(|title| {
                json!({
                    "title": title.title, "date": title.date, "rev": title.revision,
                    "company": title.company, "comments": title.comments,
                })
            }),
        );
        Value::Object(object)
    }

    fn connectivity_record_json(value: &SchematicConnectivityRecord) -> Value {
        let mut object = json!({
        "uuid": value.uuid, "kind": value.kind.as_str(), "object_id": value.uuid,
        "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
    })
    .as_object()
    .expect("connectivity JSON object")
    .clone();
        if value.kind == SchematicConnectivityRecordKind::Junction && value.junction_color_authored
        {
            object.insert(
                "color".to_owned(),
                value
                    .junction_color
                    .as_ref()
                    .map_or(Value::Null, |value| json!(value)),
            );
        }
        Value::Object(object)
    }

    fn annotation_record_json(value: &SchematicAnnotationRecord) -> Value {
        let mut object = json!({
        "uuid": value.uuid, "kind": value.kind.as_str(), "object_id": value.object_id,
        "operation_count": value.operations.len(), "operations": operations_json(&value.operations),
    })
    .as_object()
    .expect("annotation JSON object")
    .clone();
        insert_optional(
            &mut object,
            "text",
            value.text.as_ref().map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "shape",
            value.shape.as_ref().map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "at_x_nm",
            value.at_x_nm.map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "at_y_nm",
            value.at_y_nm.map(|value| json!(value)),
        );
        insert_optional(
            &mut object,
            "length_nm",
            value.length_nm.map(|value| json!(value)),
        );
        Value::Object(object)
    }
}

struct LimitedWriter {
    written: usize,
    limit: usize,
}

impl LimitedWriter {
    const fn new(limit: usize) -> Self {
        Self { written: 0, limit }
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(buffer.len())
            .filter(|next| *next <= self.limit)
            .ok_or_else(|| io::Error::other(format!("output exceeds {} bytes", self.limit)))?;
        self.written = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SchematicPlotContext, SchematicPlotLimits, SchematicPlotVariables, schematic_plot_document,
    };

    const SOURCE: &str = r#"(kicad_sch
      (version 20240101)
      (generator eeschema)
      (generator_version "10.0")
      (uuid "contract-test")
      (paper "A4")
      (wire (pts (xy 1 2) (xy 3 4))
        (stroke (width 0) (type default)) (uuid "wire-1")))"#;

    fn document() -> SchematicPlotDocument {
        schematic_plot_document(
            SOURCE,
            SchematicPlotLimits::default(),
            &SchematicPlotContext {
                source_path: Some("contract-test.kicad_sch".to_owned()),
                document_id: Some("contract-test".to_owned()),
                sheet_index: 1,
                sheet_count: 1,
                sheet_path: "/".to_owned(),
                sheet_instance_path: "/contract-test".to_owned(),
                sheet_name: "Root".to_owned(),
                project_variables: SchematicPlotVariables::default(),
                worksheet_source: None,
            },
        )
        .expect("plot document")
    }

    fn document_budget(document: &SchematicPlotDocument) -> (usize, usize) {
        let records = document.records.len();
        let operations = document
            .records
            .iter()
            .map(SchematicPlotRecord::operation_count)
            .sum::<usize>();
        let points = document
            .records
            .iter()
            .flat_map(record_operations)
            .map(operation_point_count)
            .sum::<usize>();
        let items = ROOT_VALUES
            + records * VALUES_PER_RECORD
            + operations * VALUES_PER_OPERATION
            + points * VALUES_PER_POINT;
        let bytes = items * size_of::<Value>() * VALUE_ALLOCATION_MULTIPLIER
            + payload_bytes(document).expect("payload bytes") * VALUE_ALLOCATION_MULTIPLIER;
        (items, bytes)
    }

    #[test]
    fn projection_limits_are_exact_and_fail_one_under() {
        let document = document();
        let baseline =
            schematic_plot_document_json(&document, SchematicPlotContractLimits::default())
                .expect("baseline projection");
        let output_bytes = serde_json::to_vec(&baseline)
            .expect("encoded projection")
            .len();
        let (items, materialized_bytes) = document_budget(&document);

        let exact = SchematicPlotContractLimits {
            max_derived_items: items,
            max_materialized_bytes: materialized_bytes,
            max_output_bytes: output_bytes,
        };
        schematic_plot_document_json(&document, exact).expect("exact limits");

        let mut under = exact;
        under.max_derived_items -= 1;
        assert!(
            schematic_plot_document_json(&document, under)
                .unwrap_err()
                .to_string()
                .contains("derived item limit")
        );
        under = exact;
        under.max_materialized_bytes -= 1;
        assert!(
            schematic_plot_document_json(&document, under)
                .unwrap_err()
                .to_string()
                .contains("materialized byte limit")
        );
        under = exact;
        under.max_output_bytes -= 1;
        assert!(
            schematic_plot_document_json(&document, under)
                .unwrap_err()
                .to_string()
                .contains("output limit")
        );
    }

    #[test]
    fn projection_rejects_publicly_constructible_invalid_documents() {
        let mut invalid_document = document();
        invalid_document.records.clear();
        assert!(
            schematic_plot_document_json(
                &invalid_document,
                SchematicPlotContractLimits::default(),
            )
                .unwrap_err()
                .to_string()
                .contains("validation failed")
        );

        let mut document = document();
        let SchematicPlotRecord::SheetHeader(header) = &mut document.records[0] else {
            panic!("first producer record must be the sheet header");
        };
        header.operations.push(SchematicPlotOperation::Plotter(
            PlotterOperation::FlashPadCircle(crate::FlashPadCircle {
                x: 0,
                y: 0,
                diameter_nm: 1,
                layers: Vec::new(),
                mask_margin_nm: 0,
            }),
        ));
        assert!(
            schematic_plot_document_json(&document, SchematicPlotContractLimits::default())
                .unwrap_err()
                .to_string()
                .contains("outside the schematic plot vocabulary")
        );
    }
}

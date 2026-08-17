//! Semantic validation for the strict schematic-foundation plot contract.

use crate::generated::schematic_plot_document::{
    ArcThreePointOperation, BezierCurveOperation, CircleOperation, PlotImageOperation,
    PlotPolyOperation, PlotterFill, PlotterOperation, RectOperation,
    SchematicGlobalLabelPlotRecord, SchematicGraphicBezierPlotRecord,
    SchematicHierarchicalLabelPlotRecord, SchematicImageFormat, SchematicImagePlotRecord,
    SchematicJunctionPlotRecord, SchematicLabelShape, SchematicNetclassFlagPlotRecord,
    SchematicNetclassFlagShape, SchematicNoConnectPlotRecord, SchematicPlotDocumentA0,
    SchematicPlotRecord, SchematicRuleAreaPlotRecord, SchematicRuleAreaShape,
    SchematicSheetHeaderPlotRecord, SchematicSymbolInstancePlotRecord, SchematicSymbolOperation,
    SchematicSymbolOverplotPlotRecord, SchematicTablePlotRecord, SchematicTextBoxPlotRecord,
    SchematicTextPlotRecord, TextOperation, ThickSegmentOperation,
};
use crate::{ValidationError, validation_error};

const BACKGROUND_COLOR: &str = "#F5F4EFFF";
const DRAWING_SHEET_COLOR: &str = "#840000FF";
const DEFAULT_JUNCTION_COLOR: &str = "#009600FF";
const NO_CONNECT_COLOR: &str = "#000084FF";
const GLOBAL_LABEL_COLOR: &str = "#840000FF";
const HIERARCHICAL_LABEL_COLOR: &str = "#725600FF";
const NETCLASS_COLOR: &str = "#484848FF";

/// Enforce document identity, record order, operation shape, and local indexes.
// The exhaustive producer phase dispatcher is intentionally kept in one place
// so newly generated record arms cannot bypass the common identity checks.
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive record-phase dispatcher is a deliberate compile-time ratchet"
)]
pub fn validate_schematic_plot_document(
    document: &SchematicPlotDocumentA0,
) -> Result<(), ValidationError> {
    if document.schema != "kicad.plotter_ir.a0"
        || document.source_kind != "SCH"
        || document.coordinate_space.unit != "nm"
        || document.coordinate_space.y_axis != "down"
    {
        return Err(error(
            "unsupported_contract",
            "$",
            "schematic plot document identity and coordinate space must match a0",
        ));
    }
    if !matches!(
        document.records.first(),
        Some(SchematicPlotRecord::SheetHeaderPlotRecord(_))
    ) {
        return Err(error(
            "missing_sheet_header",
            "$.records[0]",
            "the first schematic record must be the unique sheet header",
        ));
    }

    let mut previous_phase = 0_u8;
    let mut total_operations = 0usize;
    for (record_index, record) in document.records.iter().enumerate() {
        let phase = record_phase(record);
        if phase < previous_phase || (phase == 0 && record_index != 0) {
            return Err(error(
                "invalid_schematic_record_order",
                format!("$.records[{record_index}]"),
                "schematic foundation records must remain in canonical family order",
            ));
        }
        previous_phase = phase;
        let path = format!("$.records[{record_index}]");
        if let SchematicPlotRecord::SymbolInstancePlotRecord(value) = record {
            validate_symbol_instance(value, &path)?;
            total_operations = total_operations.saturating_add(value.operations.len());
            continue;
        }
        if let SchematicPlotRecord::SymbolOverplotPlotRecord(value) = record {
            validate_symbol_overplot(value, &path)?;
            total_operations = total_operations.saturating_add(value.operations.len());
            continue;
        }
        let (uuid, kind, expected_kind, object_id, declared, operations) = record_fields(record);
        let identity_matches = match record {
            SchematicPlotRecord::LabelPlotRecord(value) => value.object_id == value.text,
            SchematicPlotRecord::GlobalLabelPlotRecord(value) => value.object_id == value.text,
            SchematicPlotRecord::HierarchicalLabelPlotRecord(value) => {
                value.object_id == value.text
            }
            SchematicPlotRecord::NetclassFlagPlotRecord(_) => true,
            _ => uuid == object_id,
        };
        if kind != expected_kind || !identity_matches {
            return Err(error(
                "invalid_schematic_record_identity",
                &path,
                "foundation record object_id must equal uuid",
            ));
        }
        if declared as usize != operations.len() {
            return Err(error(
                "operation_count_mismatch",
                format!("{path}.operation_count"),
                "operation_count must equal the operation array length",
            ));
        }
        for (operation_index, operation) in operations.iter().enumerate() {
            validate_operation_header(operation, operation_index, &path)?;
        }
        match record {
            SchematicPlotRecord::SheetHeaderPlotRecord(value) => {
                validate_sheet_header(document, value, &path)?;
            }
            SchematicPlotRecord::WirePlotRecord(value) => {
                validate_polyline(&value.operations, false, &path)?;
            }
            SchematicPlotRecord::BusPlotRecord(value) => {
                validate_polyline(&value.operations, false, &path)?;
            }
            SchematicPlotRecord::BusEntryPlotRecord(value) => {
                validate_polyline(&value.operations, true, &path)?;
            }
            SchematicPlotRecord::JunctionPlotRecord(value) => {
                validate_junction(value, &path)?;
            }
            SchematicPlotRecord::NoConnectPlotRecord(value) => {
                validate_no_connect(value, &path)?;
            }
            SchematicPlotRecord::LabelPlotRecord(value) => {
                validate_label(&value.operations, &value.text, None, false, &path)?;
            }
            SchematicPlotRecord::GlobalLabelPlotRecord(value) => {
                validate_global_label(value, &path)?;
            }
            SchematicPlotRecord::HierarchicalLabelPlotRecord(value) => {
                validate_hierarchical_label(value, &path)?;
            }
            SchematicPlotRecord::NetclassFlagPlotRecord(value) => {
                validate_netclass_flag(value, &path)?;
            }
            SchematicPlotRecord::TextPlotRecord(value) => {
                validate_schematic_text(value, &path)?;
            }
            SchematicPlotRecord::TextBoxPlotRecord(value) => {
                validate_text_box(value, &path)?;
            }
            SchematicPlotRecord::GraphicPolylinePlotRecord(value) => {
                validate_graphic_record(&value.operations, GraphicKind::Polyline, false, &path)?;
            }
            SchematicPlotRecord::GraphicArcPlotRecord(value) => {
                validate_graphic_record(&value.operations, GraphicKind::Arc, false, &path)?;
            }
            SchematicPlotRecord::GraphicCirclePlotRecord(value) => {
                validate_graphic_record(&value.operations, GraphicKind::Circle, false, &path)?;
            }
            SchematicPlotRecord::GraphicRectanglePlotRecord(value) => {
                validate_graphic_record(&value.operations, GraphicKind::Rectangle, false, &path)?;
            }
            SchematicPlotRecord::GraphicBezierPlotRecord(value) => {
                validate_bezier_record(value, &path)?;
            }
            SchematicPlotRecord::RuleAreaPlotRecord(value) => {
                validate_rule_area(value, &path)?;
            }
            SchematicPlotRecord::ImagePlotRecord(value) => {
                validate_schematic_image(value, &path)?;
            }
            SchematicPlotRecord::TablePlotRecord(value) => {
                validate_table(value, &path)?;
            }
            SchematicPlotRecord::SymbolInstancePlotRecord(_)
            | SchematicPlotRecord::SymbolOverplotPlotRecord(_) => unreachable!(),
        }
        total_operations = total_operations.saturating_add(operations.len());
    }
    if document.total_operations as usize != total_operations {
        return Err(error(
            "operation_count_mismatch",
            "$.total_operations",
            "total_operations must equal all record operation counts",
        ));
    }
    Ok(())
}

fn record_phase(record: &SchematicPlotRecord) -> u8 {
    match record {
        SchematicPlotRecord::SheetHeaderPlotRecord(_) => 0,
        SchematicPlotRecord::WirePlotRecord(_) => 1,
        SchematicPlotRecord::BusPlotRecord(_) => 2,
        SchematicPlotRecord::BusEntryPlotRecord(_) => 3,
        SchematicPlotRecord::JunctionPlotRecord(_) => 4,
        SchematicPlotRecord::NoConnectPlotRecord(_) => 5,
        SchematicPlotRecord::LabelPlotRecord(_) => 6,
        SchematicPlotRecord::GlobalLabelPlotRecord(_) => 7,
        SchematicPlotRecord::HierarchicalLabelPlotRecord(_) => 8,
        SchematicPlotRecord::NetclassFlagPlotRecord(_) => 9,
        SchematicPlotRecord::TextPlotRecord(_) => 10,
        SchematicPlotRecord::TextBoxPlotRecord(_) => 11,
        SchematicPlotRecord::GraphicPolylinePlotRecord(_) => 12,
        SchematicPlotRecord::GraphicArcPlotRecord(_) => 13,
        SchematicPlotRecord::GraphicCirclePlotRecord(_) => 14,
        SchematicPlotRecord::GraphicRectanglePlotRecord(_) => 15,
        SchematicPlotRecord::GraphicBezierPlotRecord(_) => 16,
        SchematicPlotRecord::RuleAreaPlotRecord(_) => 17,
        SchematicPlotRecord::ImagePlotRecord(_) => 18,
        SchematicPlotRecord::TablePlotRecord(_) => 19,
        SchematicPlotRecord::SymbolInstancePlotRecord(_) => 20,
        SchematicPlotRecord::SymbolOverplotPlotRecord(_) => 21,
    }
}

// Keeping the generated union projection exhaustive makes missing record arms
// a compile error; splitting it would weaken that ratchet without simplifying it.
#[allow(
    clippy::too_many_lines,
    reason = "the generated record-union projection must remain visibly exhaustive"
)]
fn record_fields(
    record: &SchematicPlotRecord,
) -> (&str, &str, &'static str, &str, u32, &[PlotterOperation]) {
    match record {
        SchematicPlotRecord::SheetHeaderPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "sheet_header",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::WirePlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "wire",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::BusPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "bus",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::BusEntryPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "bus_entry",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::JunctionPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "junction",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::NoConnectPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "no_connect",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::LabelPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "label",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::GlobalLabelPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "global_label",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::HierarchicalLabelPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "hierarchical_label",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::NetclassFlagPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "netclass_flag",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::TextPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "text",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::TextBoxPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "text_box",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::GraphicPolylinePlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "graphic_polyline",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::GraphicArcPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "graphic_arc",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::GraphicCirclePlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "graphic_circle",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::GraphicRectanglePlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "graphic_rectangle",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::GraphicBezierPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "graphic_bezier",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::RuleAreaPlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "rule_area",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::ImagePlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "image",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::TablePlotRecord(value) => (
            &value.uuid,
            &value.kind,
            "table",
            &value.object_id,
            value.operation_count,
            &value.operations,
        ),
        SchematicPlotRecord::SymbolInstancePlotRecord(_)
        | SchematicPlotRecord::SymbolOverplotPlotRecord(_) => {
            unreachable!("symbol records have a distinct operation union")
        }
    }
}

fn validate_operation_header(
    operation: &PlotterOperation,
    expected_index: usize,
    record_path: &str,
) -> Result<(), ValidationError> {
    let (index, kind, expected_kind) = operation_header(operation);
    let path = format!("{record_path}.operations[{expected_index}]");
    if kind != expected_kind {
        return Err(error(
            "invalid_schematic_operation",
            format!("{path}.kind"),
            "operation kind must match its structural variant",
        ));
    }
    if index as usize != expected_index {
        return Err(error(
            "operation_index_mismatch",
            format!("{path}.index"),
            "operation index must equal its record-local position",
        ));
    }
    Ok(())
}

fn operation_header(operation: &PlotterOperation) -> (u32, &str, &'static str) {
    match operation {
        PlotterOperation::ThickSegmentOperation(value) => {
            (value.index, &value.kind, "ThickSegment")
        }
        PlotterOperation::ArcThreePointOperation(value) => {
            (value.index, &value.kind, "ArcThreePoint")
        }
        PlotterOperation::CircleOperation(value) => (value.index, &value.kind, "Circle"),
        PlotterOperation::RectOperation(value) => (value.index, &value.kind, "Rect"),
        PlotterOperation::PlotPolyOperation(value) => (value.index, &value.kind, "PlotPoly"),
        PlotterOperation::BezierCurveOperation(value) => (value.index, &value.kind, "BezierCurve"),
        PlotterOperation::TextOperation(value) => (value.index, &value.kind, "Text"),
        PlotterOperation::PlotImageOperation(value) => (value.index, &value.kind, "PlotImage"),
        PlotterOperation::FlashPadCircleOperation(value) => {
            (value.index, &value.kind, "FlashPadCircle")
        }
        PlotterOperation::FlashPadOvalOperation(value) => {
            (value.index, &value.kind, "FlashPadOval")
        }
        PlotterOperation::FlashPadRectOperation(value) => {
            (value.index, &value.kind, "FlashPadRect")
        }
        PlotterOperation::FlashPadRoundRectOperation(value) => {
            (value.index, &value.kind, "FlashPadRoundRect")
        }
        PlotterOperation::FlashPadCustomOperation(value) => {
            (value.index, &value.kind, "FlashPadCustom")
        }
        PlotterOperation::FlashPadTrapezOperation(value) => {
            (value.index, &value.kind, "FlashPadTrapez")
        }
    }
}

fn validate_symbol_instance(
    record: &SchematicSymbolInstancePlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    if record.kind != "symbol_instance"
        || record.object_id
            != if record.lib_id.is_empty() {
                record.uuid.as_str()
            } else {
                record.lib_id.as_str()
            }
        || !record.at_angle_deg.is_finite()
        || !matches!(record.mirror.as_deref(), None | Some("x") | Some("y"))
    {
        return Err(error(
            "invalid_symbol_instance",
            path,
            "placed-symbol identity, angle, and mirror must be canonical",
        ));
    }
    validate_symbol_operations(
        &record.operations,
        &record.uuid,
        record.operation_count,
        path,
    )
}

fn validate_symbol_overplot(
    record: &SchematicSymbolOverplotPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let expected_uuid = format!("{}:overplot", record.source_symbol_uuid);
    let expected_object = if record.lib_id.is_empty() {
        &record.source_symbol_uuid
    } else {
        &record.lib_id
    };
    if record.kind != "symbol_overplot"
        || record.uuid != expected_uuid
        || record.object_id != *expected_object
    {
        return Err(error(
            "invalid_symbol_overplot",
            path,
            "symbol overplot identity must remain linked to its source symbol",
        ));
    }
    validate_symbol_operations(
        &record.operations,
        &record.source_symbol_uuid,
        record.operation_count,
        path,
    )
}

// Pin-block state, the closed operation vocabulary, and shared drawing checks
// are deliberately validated in one linear state machine.
#[allow(
    clippy::too_many_lines,
    reason = "one linear state machine owns the complete pin-block validation invariant"
)]
fn validate_symbol_operations(
    operations: &[SchematicSymbolOperation],
    parent_uuid: &str,
    declared: u32,
    path: &str,
) -> Result<(), ValidationError> {
    if declared as usize != operations.len() {
        return Err(error(
            "operation_count_mismatch",
            format!("{path}.operation_count"),
            "operation_count must equal the operation array length",
        ));
    }
    let mut block_start = None;
    for (expected_index, operation) in operations.iter().enumerate() {
        let operation_path = format!("{path}.operations[{expected_index}]");
        let (index, kind, expected_kind) = symbol_operation_header(operation);
        if index as usize != expected_index || kind != expected_kind {
            return Err(error(
                "invalid_schematic_operation",
                &operation_path,
                "symbol operation kind and local index must match its structural variant",
            ));
        }
        match operation {
            SchematicSymbolOperation::SchematicSymbolStartBlockOperation(value) => {
                if block_start.is_some()
                    || value.label.is_empty()
                    || value.label != value.data_uuid
                    || value.data_ref != "symbol_pin"
                    || value.object_id.is_empty()
                {
                    return Err(error(
                        "invalid_symbol_pin_block",
                        &operation_path,
                        "pin blocks must be nonnested and retain exact ownership",
                    ));
                }
                let allowed = [
                    "primitive",
                    "object-type",
                    "pin",
                    "symbol-uuid",
                    "designator",
                    "lib-pin-uuid",
                ];
                if value
                    .extra_attrs
                    .keys()
                    .any(|key| !allowed.contains(&key.as_str()))
                    || value.extra_attrs.values().any(String::is_empty)
                    || value.extra_attrs.get("primitive").map(String::as_str) != Some("pin")
                    || value.extra_attrs.get("object-type").map(String::as_str) != Some("pin")
                    || value.extra_attrs.get("symbol-uuid").map(String::as_str) != Some(parent_uuid)
                {
                    return Err(error(
                        "invalid_symbol_pin_attrs",
                        format!("{operation_path}.extra_attrs"),
                        "pin block metadata must use the closed symbol-pin vocabulary",
                    ));
                }
                block_start = Some(expected_index);
            }
            SchematicSymbolOperation::SchematicSymbolEndBlockOperation(_) => {
                let Some(start) = block_start else {
                    return Err(error(
                        "invalid_symbol_pin_block",
                        &operation_path,
                        "pin block end must follow a matching start",
                    ));
                };
                if expected_index == start + 1 {
                    return Err(error(
                        "invalid_symbol_pin_block",
                        &operation_path,
                        "pin blocks must contain at least one operation",
                    ));
                }
                block_start = None;
            }
            SchematicSymbolOperation::PlotImageOperation(_)
            | SchematicSymbolOperation::FlashPadCircleOperation(_)
            | SchematicSymbolOperation::FlashPadOvalOperation(_)
            | SchematicSymbolOperation::FlashPadRectOperation(_)
            | SchematicSymbolOperation::FlashPadRoundRectOperation(_)
            | SchematicSymbolOperation::FlashPadCustomOperation(_)
            | SchematicSymbolOperation::FlashPadTrapezOperation(_) => {
                return Err(error(
                    "invalid_symbol_operation",
                    &operation_path,
                    "placed symbols do not admit images or pad flashes",
                ));
            }
            SchematicSymbolOperation::TextOperation(value) => {
                validate_annotation_text(value, &operation_path)?;
                if block_start.is_some() && value.context.is_some() {
                    return Err(error(
                        "invalid_symbol_pin_text",
                        &operation_path,
                        "pin text cannot carry hyperlink context",
                    ));
                }
            }
            _ => validate_symbol_draw_operation(operation, &operation_path)?,
        }
    }
    if block_start.is_some() {
        return Err(error(
            "invalid_symbol_pin_block",
            format!("{path}.operations"),
            "pin block must terminate before the record ends",
        ));
    }
    Ok(())
}

fn validate_symbol_draw_operation(
    operation: &SchematicSymbolOperation,
    path: &str,
) -> Result<(), ValidationError> {
    let layer = match operation {
        SchematicSymbolOperation::ThickSegmentOperation(value) => {
            if value.role.is_some()
                || !value.layers.is_empty()
                || value.mask_margin_nm.is_some()
                || value.pad_size_x_nm.is_some()
                || value.pad_size_y_nm.is_some()
            {
                return Err(error(
                    "invalid_symbol_operation",
                    path,
                    "symbol segment state is invalid",
                ));
            }
            value.layer.as_deref()
        }
        SchematicSymbolOperation::ArcThreePointOperation(value) => value.layer.as_deref(),
        SchematicSymbolOperation::CircleOperation(value) => {
            if value.role.is_some()
                || !value.layers.is_empty()
                || value.mask_margin_nm.is_some()
                || value.pad_size_x_nm.is_some()
                || value.pad_size_y_nm.is_some()
            {
                return Err(error(
                    "invalid_symbol_operation",
                    path,
                    "symbol circle state is invalid",
                ));
            }
            value.layer.as_deref()
        }
        SchematicSymbolOperation::RectOperation(value) => value.layer.as_deref(),
        SchematicSymbolOperation::PlotPolyOperation(value) => value.layer.as_deref(),
        SchematicSymbolOperation::BezierCurveOperation(value) => value.layer.as_deref(),
        _ => unreachable!("non-drawing symbol operation was handled by the caller"),
    };
    if layer.is_some() {
        return Err(error(
            "invalid_symbol_operation",
            path,
            "placed-symbol operations must remain layerless",
        ));
    }
    Ok(())
}

fn symbol_operation_header(operation: &SchematicSymbolOperation) -> (u32, &str, &'static str) {
    match operation {
        SchematicSymbolOperation::ThickSegmentOperation(value) => {
            (value.index, &value.kind, "ThickSegment")
        }
        SchematicSymbolOperation::ArcThreePointOperation(value) => {
            (value.index, &value.kind, "ArcThreePoint")
        }
        SchematicSymbolOperation::CircleOperation(value) => (value.index, &value.kind, "Circle"),
        SchematicSymbolOperation::RectOperation(value) => (value.index, &value.kind, "Rect"),
        SchematicSymbolOperation::PlotPolyOperation(value) => {
            (value.index, &value.kind, "PlotPoly")
        }
        SchematicSymbolOperation::BezierCurveOperation(value) => {
            (value.index, &value.kind, "BezierCurve")
        }
        SchematicSymbolOperation::TextOperation(value) => (value.index, &value.kind, "Text"),
        SchematicSymbolOperation::PlotImageOperation(value) => {
            (value.index, &value.kind, "PlotImage")
        }
        SchematicSymbolOperation::FlashPadCircleOperation(value) => {
            (value.index, &value.kind, "FlashPadCircle")
        }
        SchematicSymbolOperation::FlashPadOvalOperation(value) => {
            (value.index, &value.kind, "FlashPadOval")
        }
        SchematicSymbolOperation::FlashPadRectOperation(value) => {
            (value.index, &value.kind, "FlashPadRect")
        }
        SchematicSymbolOperation::FlashPadRoundRectOperation(value) => {
            (value.index, &value.kind, "FlashPadRoundRect")
        }
        SchematicSymbolOperation::FlashPadCustomOperation(value) => {
            (value.index, &value.kind, "FlashPadCustom")
        }
        SchematicSymbolOperation::FlashPadTrapezOperation(value) => {
            (value.index, &value.kind, "FlashPadTrapez")
        }
        SchematicSymbolOperation::SchematicSymbolStartBlockOperation(value) => {
            (value.index, &value.kind, "StartBlock")
        }
        SchematicSymbolOperation::SchematicSymbolEndBlockOperation(value) => {
            (value.index, &value.kind, "EndBlock")
        }
    }
}

fn validate_sheet_header(
    document: &SchematicPlotDocumentA0,
    record: &SchematicSheetHeaderPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    if record.kind != "sheet_header"
        || record.sheet_width_nm != document.canvas.width_nm
        || record.sheet_height_nm != document.canvas.height_nm
        || record.sheet_width_nm.get() <= 0
        || record.sheet_height_nm.get() <= 0
        || record
            .paper_width_mm
            .is_some_and(|value| !value.is_finite())
        || record
            .paper_height_mm
            .is_some_and(|value| !value.is_finite())
    {
        return Err(error(
            "invalid_sheet_header",
            path,
            "sheet header identity, canvas, paper, and dimensions must be canonical",
        ));
    }
    let Some(PlotterOperation::RectOperation(background)) = record.operations.first() else {
        return Err(error(
            "invalid_sheet_background",
            format!("{path}.operations[0]"),
            "sheet header must begin with the canonical page background",
        ));
    };
    if !valid_background(background, record) {
        return Err(error(
            "invalid_sheet_background",
            format!("{path}.operations[0]"),
            "sheet background must cover the canvas with canonical styling",
        ));
    }
    for (index, operation) in record.operations.iter().enumerate().skip(1) {
        let operation_path = format!("{path}.operations[{index}]");
        match operation {
            PlotterOperation::RectOperation(value) if valid_worksheet_rect(value) => {}
            PlotterOperation::PlotPolyOperation(value) if valid_worksheet_polyline(value) => {}
            PlotterOperation::TextOperation(value) => {
                validate_worksheet_text(value, &operation_path)?;
            }
            PlotterOperation::PlotImageOperation(value) => {
                validate_worksheet_image(value, &operation_path)?;
            }
            _ => {
                return Err(error(
                    "invalid_worksheet_operation",
                    operation_path,
                    "worksheet admits only layerless rect, polyline, text, and image operations",
                ));
            }
        }
    }
    Ok(())
}

fn valid_worksheet_rect(value: &RectOperation) -> bool {
    value.fill == PlotterFill::NoFill
        && value.width_nm.get() >= 152_400
        && value.corner_radius_nm.get() == 0
        && value.layer.is_none()
        && value.stroke_color.as_deref() == Some(DRAWING_SHEET_COLOR)
        && value.fill_color.is_none()
        && value.line_style.is_none()
}

fn valid_worksheet_polyline(value: &PlotPolyOperation) -> bool {
    value.points.len() == 2
        && value.fill == PlotterFill::NoFill
        && value.width_nm.get() >= 152_400
        && value.layer.is_none()
        && value.stroke_color.as_deref() == Some(DRAWING_SHEET_COLOR)
        && value.fill_color.is_none()
        && value.line_style.is_none()
}

fn valid_background(value: &RectOperation, record: &SchematicSheetHeaderPlotRecord) -> bool {
    value.x1.get() == 0
        && value.y1.get() == 0
        && value.x2 == record.sheet_width_nm
        && value.y2 == record.sheet_height_nm
        && value.fill == PlotterFill::FilledShape
        && value.width_nm.get() == 100
        && value.corner_radius_nm.get() == 0
        && value.layer.is_none()
        && value.stroke_color.as_deref() == Some(BACKGROUND_COLOR)
        && value.fill_color.as_deref() == Some(BACKGROUND_COLOR)
}

fn validate_worksheet_text(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    if value.context.is_some()
        || value.layer.is_some()
        || value.mirror.is_some()
        || value.text_as_polygons.is_some()
        || value.polyline_per_segment.is_some()
        || value.knockout.is_some()
        || !value.render_cache_polygons.is_empty()
        || value.render_cache.is_some()
        || value.render_cache_source.is_some()
        || value.render_cache_exact.is_some()
        || !value.orient_deg.is_finite()
    {
        return Err(error(
            "invalid_worksheet_text",
            path,
            "worksheet text must remain layerless, finite, stroke text without cache markers",
        ));
    }
    Ok(())
}

fn validate_worksheet_image(value: &PlotImageOperation, path: &str) -> Result<(), ValidationError> {
    if value.image_format != "png"
        || !value.scale.is_finite()
        || value.scale <= 0.0
        || value.width_nm.get() < 0
        || value.height_nm.get() < 0
        || value.stroke_color.as_deref() != Some(DRAWING_SHEET_COLOR)
        || !valid_png_base64(&value.image_data_b64)
    {
        return Err(error(
            "invalid_worksheet_image",
            path,
            "worksheet image must be a finite positive-scale PNG placement",
        ));
    }
    Ok(())
}

fn valid_png_base64(value: &str) -> bool {
    let mut prefix = [0_u8; 33];
    let mut prefix_len = 0_usize;
    let mut quartet = [0_u8; 4];
    let mut quartet_len = 0_usize;
    let mut ended = false;

    for byte in value.bytes() {
        if byte.is_ascii_whitespace() {
            return false;
        }
        if ended {
            return false;
        }
        let sextet = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return false,
        };
        quartet[quartet_len] = sextet;
        quartet_len += 1;
        if quartet_len != 4 {
            continue;
        }
        if quartet[0] >= 64 || quartet[1] >= 64 {
            return false;
        }
        let decoded_len = if quartet[2] == 64 {
            if quartet[3] != 64 || quartet[1] & 0x0f != 0 {
                return false;
            }
            ended = true;
            1
        } else if quartet[3] == 64 {
            if quartet[2] & 0x03 != 0 {
                return false;
            }
            ended = true;
            2
        } else {
            3
        };
        let decoded = [
            (quartet[0] << 2) | (quartet[1] >> 4),
            (quartet[1] << 4) | (quartet[2] >> 2),
            (quartet[2] << 6) | quartet[3],
        ];
        for byte in decoded.into_iter().take(decoded_len) {
            if prefix_len < prefix.len() {
                prefix[prefix_len] = byte;
                prefix_len += 1;
            }
        }
        quartet_len = 0;
    }
    if quartet_len != 0 || prefix_len < prefix.len() {
        return false;
    }
    let width = u32::from_be_bytes([prefix[16], prefix[17], prefix[18], prefix[19]]);
    let height = u32::from_be_bytes([prefix[20], prefix[21], prefix[22], prefix[23]]);
    prefix[..8] == *b"\x89PNG\r\n\x1a\n"
        && prefix[8..12] == [0, 0, 0, 13]
        && prefix[12..16] == *b"IHDR"
        && width > 0
        && height > 0
}

fn validate_polyline(
    operations: &[PlotterOperation],
    bus_entry: bool,
    path: &str,
) -> Result<(), ValidationError> {
    let [PlotterOperation::PlotPolyOperation(value)] = operations else {
        return Err(error(
            "invalid_connectivity_record",
            path,
            "wire, bus, and bus-entry records contain exactly one polygon operation",
        ));
    };
    if !valid_connectivity_poly(value) || (bus_entry && value.points.len() != 2) {
        return Err(error(
            if bus_entry {
                "invalid_bus_entry"
            } else {
                "invalid_connectivity_polyline"
            },
            format!("{path}.operations[0]"),
            "connectivity polyline state and point count must be canonical",
        ));
    }
    Ok(())
}

fn valid_connectivity_poly(value: &PlotPolyOperation) -> bool {
    value.layer.is_none()
        && value.fill == PlotterFill::NoFill
        && value.width_nm.get() >= 0
        && value
            .stroke_color
            .as_deref()
            .is_some_and(|color| !color.is_empty())
        && value.line_style.is_some()
        && !value.points.is_empty()
}

fn validate_junction(
    record: &SchematicJunctionPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    if record.kind != "junction" {
        return Err(error(
            "invalid_junction",
            path,
            "junction record kind must be canonical",
        ));
    }
    let [PlotterOperation::CircleOperation(value)] = record.operations.as_slice() else {
        return Err(error(
            "invalid_junction",
            path,
            "junction records contain exactly one circle operation",
        ));
    };
    let expected_color = match &record.color {
        None | Some(None) => DEFAULT_JUNCTION_COLOR,
        Some(Some(color)) => color,
    };
    if !valid_junction_circle(value) || value.stroke_color.as_deref() != Some(expected_color) {
        return Err(error(
            "invalid_junction",
            format!("{path}.operations[0]"),
            "junction circle and optional authored color must agree",
        ));
    }
    Ok(())
}

fn valid_junction_circle(value: &CircleOperation) -> bool {
    value.layer.is_none()
        && value.role.is_none()
        && value.layers.is_empty()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
        && value.fill == PlotterFill::FilledShape
        && value.width_nm.get() == 0
        && value.diameter_nm.get() > 0
        && value.stroke_color.is_some()
        && value.stroke_color == value.fill_color
}

fn validate_no_connect(
    record: &SchematicNoConnectPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    if record.kind != "no_connect" {
        return Err(error(
            "invalid_no_connect",
            path,
            "no-connect record kind must be canonical",
        ));
    }
    let [
        PlotterOperation::PlotPolyOperation(first),
        PlotterOperation::PlotPolyOperation(second),
    ] = record.operations.as_slice()
    else {
        return Err(error(
            "invalid_no_connect",
            path,
            "no-connect records contain exactly two polygon operations",
        ));
    };
    if !valid_no_connect_segment(first)
        || !valid_no_connect_segment(second)
        || first.width_nm != second.width_nm
        || !cross_geometry(first, second)
    {
        return Err(error(
            "invalid_no_connect",
            format!("{path}.operations"),
            "no-connect segments must form one canonical equal-width cross",
        ));
    }
    Ok(())
}

fn valid_no_connect_segment(value: &PlotPolyOperation) -> bool {
    value.layer.is_none()
        && value.fill == PlotterFill::NoFill
        && value.width_nm.get() > 0
        && value.stroke_color.as_deref() == Some(NO_CONNECT_COLOR)
        && value.line_style.is_none()
        && value.points.len() == 2
}

fn cross_geometry(first: &PlotPolyOperation, second: &PlotPolyOperation) -> bool {
    let left = [&first.points[0].0, &first.points[1].0];
    let right = [&second.points[0].0, &second.points[1].0];
    left[0][0] == right[0][0]
        && left[1][0] == right[1][0]
        && left[0][1] == right[1][1]
        && left[1][1] == right[0][1]
}

fn validate_annotation_text(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    if value.layer.is_some()
        || value.mirror.is_some()
        || value.text_as_polygons.is_some()
        || value.polyline_per_segment.is_some()
        || value.knockout.is_some()
        || !value.render_cache_polygons.is_empty()
        || value.render_cache.is_some()
        || value.render_cache_source.is_some()
        || value.render_cache_exact.is_some()
        || !value.orient_deg.is_finite()
        || value.context.as_ref().is_some_and(|context| {
            let href: &str = &context.hyperlink.href;
            href.is_empty() || href.trim() != href
        })
    {
        return Err(error(
            "invalid_annotation_text",
            path,
            "schematic annotation text must be canonical layerless stroke text",
        ));
    }
    Ok(())
}

fn validate_global_label(
    record: &SchematicGlobalLabelPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    validate_label(
        &record.operations,
        &record.text,
        Some(record.shape),
        true,
        path,
    )
}

fn validate_hierarchical_label(
    record: &SchematicHierarchicalLabelPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    validate_label(
        &record.operations,
        &record.text,
        Some(record.shape),
        false,
        path,
    )
}

fn validate_label(
    operations: &[PlotterOperation],
    metadata_text: &str,
    shape: Option<SchematicLabelShape>,
    global: bool,
    path: &str,
) -> Result<(), ValidationError> {
    let decorated = shape.is_some_and(|value| {
        matches!(
            value,
            SchematicLabelShape::Input
                | SchematicLabelShape::Output
                | SchematicLabelShape::Bidirectional
                | SchematicLabelShape::TriState
                | SchematicLabelShape::Passive
        )
    });
    let expected_count = if decorated { 2 } else { 1 };
    if operations.len() != expected_count {
        return Err(error(
            "invalid_label_record",
            format!("{path}.operations"),
            "label operation count and decoration presence must match its shape",
        ));
    }
    let PlotterOperation::TextOperation(text) = &operations[0] else {
        return Err(error(
            "invalid_label_record",
            format!("{path}.operations[0]"),
            "labels begin with exactly one text operation",
        ));
    };
    validate_annotation_text(text, &format!("{path}.operations[0]"))?;
    if text.text != metadata_text.replace("{slash}", "/") {
        return Err(error(
            "invalid_label_text",
            format!("{path}.text"),
            "label metadata and plotted display text must agree",
        ));
    }
    if !decorated {
        return Ok(());
    }
    let PlotterOperation::PlotPolyOperation(decoration) = &operations[1] else {
        return Err(error(
            "invalid_label_decoration",
            format!("{path}.operations[1]"),
            "decorated labels end with one polygon",
        ));
    };
    let shape = shape.expect("decorated label shape");
    let expected_points = if global {
        7
    } else if matches!(
        shape,
        SchematicLabelShape::Input | SchematicLabelShape::Output
    ) {
        6
    } else {
        5
    };
    let expected_color = if global {
        GLOBAL_LABEL_COLOR
    } else {
        HIERARCHICAL_LABEL_COLOR
    };
    let closed = decoration
        .points
        .first()
        .zip(decoration.points.last())
        .is_some_and(|(first, last)| first.0 == last.0);
    if decoration.layer.is_some()
        || decoration.fill != PlotterFill::NoFill
        || decoration.width_nm.get() != 152_400
        || decoration.stroke_color.as_deref() != Some(expected_color)
        || decoration.fill_color.is_some()
        || decoration.line_style.is_some()
        || decoration.points.len() != expected_points
        || !closed
    {
        return Err(error(
            "invalid_label_decoration",
            format!("{path}.operations[1]"),
            "label decoration geometry and style must be canonical",
        ));
    }
    Ok(())
}

fn validate_netclass_flag(
    record: &SchematicNetclassFlagPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let text_start = match record.shape {
        SchematicNetclassFlagShape::Round | SchematicNetclassFlagShape::Dot => {
            let [
                PlotterOperation::ThickSegmentOperation(segment),
                PlotterOperation::CircleOperation(marker),
                ..,
            ] = record.operations.as_slice()
            else {
                return Err(error(
                    "invalid_netclass_marker",
                    format!("{path}.operations"),
                    "round and dot flags begin with segment and circle markers",
                ));
            };
            validate_netclass_segment(segment, record, &format!("{path}.operations[0]"))?;
            validate_netclass_circle(marker, segment, record, &format!("{path}.operations[1]"))?;
            2
        }
        SchematicNetclassFlagShape::Diamond | SchematicNetclassFlagShape::Rectangle => {
            let Some(PlotterOperation::PlotPolyOperation(marker)) = record.operations.first()
            else {
                return Err(error(
                    "invalid_netclass_marker",
                    format!("{path}.operations[0]"),
                    "diamond and rectangle flags begin with one polygon marker",
                ));
            };
            let expected_points = if record.shape == SchematicNetclassFlagShape::Diamond {
                7
            } else {
                8
            };
            let anchor = [record.at_x_nm, record.at_y_nm];
            let closed_at_anchor = marker
                .points
                .first()
                .zip(marker.points.last())
                .is_some_and(|(first, last)| first.0 == anchor && first.0 == last.0);
            if marker.layer.is_some()
                || marker.fill != PlotterFill::NoFill
                || marker.width_nm.get() <= 0
                || marker.stroke_color.as_deref() != Some(NETCLASS_COLOR)
                || marker.fill_color.is_some()
                || marker.line_style.is_some()
                || marker.points.len() != expected_points
                || !closed_at_anchor
            {
                return Err(error(
                    "invalid_netclass_polygon",
                    format!("{path}.operations[0]"),
                    "netclass polygon marker must have canonical geometry and style",
                ));
            }
            1
        }
    };
    for (index, operation) in record.operations.iter().enumerate().skip(text_start) {
        let PlotterOperation::TextOperation(text) = operation else {
            return Err(error(
                "invalid_netclass_property",
                format!("{path}.operations[{index}]"),
                "netclass marker may only be followed by visible property text",
            ));
        };
        validate_annotation_text(text, &format!("{path}.operations[{index}]"))?;
    }
    Ok(())
}

fn validate_netclass_segment(
    segment: &ThickSegmentOperation,
    record: &SchematicNetclassFlagPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    if segment.layer.is_some()
        || segment.role.is_some()
        || !segment.layers.is_empty()
        || segment.mask_margin_nm.is_some()
        || segment.pad_size_x_nm.is_some()
        || segment.pad_size_y_nm.is_some()
        || segment.width_nm.get() <= 0
        || segment.stroke_color.as_deref() != Some(NETCLASS_COLOR)
        || segment.start_x != record.at_x_nm
        || segment.start_y != record.at_y_nm
    {
        return Err(error(
            "invalid_netclass_segment",
            path,
            "netclass segment must begin at the flag anchor with canonical style",
        ));
    }
    Ok(())
}

fn validate_netclass_circle(
    marker: &CircleOperation,
    segment: &ThickSegmentOperation,
    record: &SchematicNetclassFlagPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let dot = record.shape == SchematicNetclassFlagShape::Dot;
    let symbol_size = if dot { 355_600 } else { 508_000 };
    let expected_fill = if dot {
        PlotterFill::FilledShape
    } else {
        PlotterFill::NoFill
    };
    let expected_width = if dot { 0 } else { segment.width_nm.get() };
    let expected_fill_color = dot.then_some(NETCLASS_COLOR);
    if marker.layer.is_some()
        || marker.role.is_some()
        || !marker.layers.is_empty()
        || marker.mask_margin_nm.is_some()
        || marker.pad_size_x_nm.is_some()
        || marker.pad_size_y_nm.is_some()
        || marker.line_style.is_some()
        || marker.diameter_nm.get() != 2 * symbol_size
        || marker.fill != expected_fill
        || marker.width_nm.get() != expected_width
        || marker.stroke_color.as_deref() != Some(NETCLASS_COLOR)
        || marker.fill_color.as_deref() != expected_fill_color
    {
        return Err(error(
            "invalid_netclass_circle",
            path,
            "netclass circle marker must have canonical geometry and style",
        ));
    }
    Ok(())
}

fn validate_schematic_text(
    record: &SchematicTextPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let [PlotterOperation::TextOperation(text)] = record.operations.as_slice() else {
        return Err(error(
            "invalid_schematic_text",
            format!("{path}.operations"),
            "ordinary schematic text contains exactly one text operation",
        ));
    };
    validate_annotation_text(text, &format!("{path}.operations[0]"))?;
    let expected = record.text.strip_suffix('\n').unwrap_or(&record.text);
    if text.text != expected || text.multiline != text.text.contains('\n') {
        return Err(error(
            "invalid_schematic_text",
            format!("{path}.text"),
            "schematic text metadata and trailing-newline normalization must agree",
        ));
    }
    Ok(())
}

fn validate_text_box(
    record: &SchematicTextBoxPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let text_start = validate_text_box_prefix(&record.operations, path)?;
    validate_text_box_lines(&record.operations, text_start, path)
}

fn validate_text_box_prefix(
    operations: &[PlotterOperation],
    path: &str,
) -> Result<usize, ValidationError> {
    let Some(PlotterOperation::RectOperation(first)) = operations.first() else {
        return Err(error(
            "invalid_text_box",
            format!("{path}.operations"),
            "text box begins with an outline rectangle",
        ));
    };
    let single_fill_valid = match first.fill {
        PlotterFill::NoFill => first.fill_color.is_none(),
        PlotterFill::FilledShape => true,
        _ => true,
    };
    if first.layer.is_some()
        || first.corner_radius_nm.get() != 0
        || first.width_nm.get() < 0
        || !first.stroke_color.as_deref().is_some_and(valid_color)
        || !first.fill_color.as_deref().is_none_or(valid_color)
        || first.line_style.is_none()
        || !single_fill_valid
    {
        return Err(error(
            "invalid_text_box_outline",
            format!("{path}.operations[0]"),
            "text box rectangle style must be canonical",
        ));
    }
    let text_start = if matches!(first.fill, PlotterFill::NoFill | PlotterFill::FilledShape) {
        1
    } else {
        let Some(PlotterOperation::RectOperation(outline)) = operations.get(1) else {
            return Err(error(
                "invalid_text_box_fill_pass",
                format!("{path}.operations"),
                "noncanonical fills require a fill pass and outline pass",
            ));
        };
        let same_geometry = first.x1 == outline.x1
            && first.y1 == outline.y1
            && first.x2 == outline.x2
            && first.y2 == outline.y2
            && first.corner_radius_nm == outline.corner_radius_nm;
        if first.width_nm.get() != 0
            || first.fill_color.is_none()
            || first.stroke_color != first.fill_color
            || outline.layer.is_some()
            || !same_geometry
            || outline.fill != PlotterFill::NoFill
            || outline.width_nm.get() < 0
            || !outline.stroke_color.as_deref().is_some_and(valid_color)
            || outline.fill_color.is_some()
            || outline.line_style != first.line_style
        {
            return Err(error(
                "invalid_text_box_fill_pass",
                format!("{path}.operations[0]"),
                "text box fill and outline passes must be coherent",
            ));
        }
        2
    };
    Ok(text_start)
}

fn validate_text_box_lines(
    operations: &[PlotterOperation],
    text_start: usize,
    path: &str,
) -> Result<(), ValidationError> {
    for (index, operation) in operations.iter().enumerate().skip(text_start) {
        let PlotterOperation::TextOperation(text) = operation else {
            return Err(error(
                "invalid_text_box_line",
                format!("{path}.operations[{index}]"),
                "text box outline may only be followed by plotted text lines",
            ));
        };
        if text.text.is_empty() || text.multiline {
            return Err(error(
                "invalid_text_box_line",
                format!("{path}.operations[{index}]"),
                "text box lines are nonempty single-line text operations",
            ));
        }
        validate_annotation_text(text, &format!("{path}.operations[{index}]"))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum GraphicKind {
    Polyline,
    Arc,
    Circle,
    Rectangle,
}

#[derive(Clone, Copy)]
enum GraphicRef<'a> {
    Polyline(&'a PlotPolyOperation),
    Arc(&'a ArcThreePointOperation),
    Circle(&'a CircleOperation),
    Rectangle(&'a RectOperation),
}

impl<'a> GraphicRef<'a> {
    fn fill(self) -> PlotterFill {
        match self {
            Self::Polyline(value) => value.fill,
            Self::Arc(value) => value.fill,
            Self::Circle(value) => value.fill,
            Self::Rectangle(value) => value.fill,
        }
    }

    fn width(self) -> i64 {
        match self {
            Self::Polyline(value) => value.width_nm.get(),
            Self::Arc(value) => value.width_nm.get(),
            Self::Circle(value) => value.width_nm.get(),
            Self::Rectangle(value) => value.width_nm.get(),
        }
    }

    fn layer(self) -> Option<&'static str> {
        let present = match self {
            Self::Polyline(value) => value.layer.is_some(),
            Self::Arc(value) => value.layer.is_some(),
            Self::Circle(value) => value.layer.is_some(),
            Self::Rectangle(value) => value.layer.is_some(),
        };
        present.then_some("present")
    }

    fn stroke_color(self) -> Option<&'static str> {
        let valid = match self {
            Self::Polyline(value) => value.stroke_color.as_deref().is_some_and(valid_color),
            Self::Arc(value) => value.stroke_color.as_deref().is_some_and(valid_color),
            Self::Circle(value) => value.stroke_color.as_deref().is_some_and(valid_color),
            Self::Rectangle(value) => value.stroke_color.as_deref().is_some_and(valid_color),
        };
        valid.then_some("valid")
    }

    fn stroke(self) -> Option<&'a str> {
        match self {
            Self::Polyline(value) => value.stroke_color.as_deref(),
            Self::Arc(value) => value.stroke_color.as_deref(),
            Self::Circle(value) => value.stroke_color.as_deref(),
            Self::Rectangle(value) => value.stroke_color.as_deref(),
        }
    }

    fn fill_color(self) -> Option<&'a str> {
        match self {
            Self::Polyline(value) => value.fill_color.as_deref(),
            Self::Arc(value) => value.fill_color.as_deref(),
            Self::Circle(value) => value.fill_color.as_deref(),
            Self::Rectangle(value) => value.fill_color.as_deref(),
        }
    }

    fn line_style_present(self) -> bool {
        match self {
            Self::Polyline(value) => value.line_style.is_some(),
            Self::Arc(value) => value.line_style.is_some(),
            Self::Circle(value) => value.line_style.is_some(),
            Self::Rectangle(value) => value.line_style.is_some(),
        }
    }

    fn same_line_style(self, other: Self) -> bool {
        match (self, other) {
            (Self::Polyline(a), Self::Polyline(b)) => a.line_style == b.line_style,
            (Self::Arc(a), Self::Arc(b)) => a.line_style == b.line_style,
            (Self::Circle(a), Self::Circle(b)) => a.line_style == b.line_style,
            (Self::Rectangle(a), Self::Rectangle(b)) => a.line_style == b.line_style,
            _ => false,
        }
    }

    fn same_geometry(self, other: Self) -> bool {
        match (self, other) {
            (Self::Polyline(a), Self::Polyline(b)) => {
                a.points.len() == b.points.len()
                    && a.points
                        .iter()
                        .zip(&b.points)
                        .all(|(left, right)| left.0 == right.0)
            }
            (Self::Arc(a), Self::Arc(b)) => {
                a.start_x == b.start_x
                    && a.start_y == b.start_y
                    && a.mid_x == b.mid_x
                    && a.mid_y == b.mid_y
                    && a.end_x == b.end_x
                    && a.end_y == b.end_y
            }
            (Self::Circle(a), Self::Circle(b)) => {
                a.cx == b.cx && a.cy == b.cy && a.diameter_nm == b.diameter_nm
            }
            (Self::Rectangle(a), Self::Rectangle(b)) => {
                a.x1 == b.x1
                    && a.y1 == b.y1
                    && a.x2 == b.x2
                    && a.y2 == b.y2
                    && a.corner_radius_nm == b.corner_radius_nm
            }
            _ => false,
        }
    }
}

fn graphic_ref(operation: &PlotterOperation, kind: GraphicKind) -> Option<GraphicRef<'_>> {
    match (operation, kind) {
        (PlotterOperation::PlotPolyOperation(value), GraphicKind::Polyline) => {
            Some(GraphicRef::Polyline(value))
        }
        (PlotterOperation::ArcThreePointOperation(value), GraphicKind::Arc) => {
            Some(GraphicRef::Arc(value))
        }
        (PlotterOperation::CircleOperation(value), GraphicKind::Circle) => {
            Some(GraphicRef::Circle(value))
        }
        (PlotterOperation::RectOperation(value), GraphicKind::Rectangle) => {
            Some(GraphicRef::Rectangle(value))
        }
        _ => None,
    }
}

fn validate_graphic_record(
    operations: &[PlotterOperation],
    kind: GraphicKind,
    closed: bool,
    path: &str,
) -> Result<(), ValidationError> {
    if !matches!(operations.len(), 1 | 2) {
        return Err(error(
            "invalid_graphic_record",
            format!("{path}.operations"),
            "schematic graphics contain one operation or a coherent fill pair",
        ));
    }
    let first = graphic_ref(&operations[0], kind).ok_or_else(|| {
        error(
            "invalid_graphic_record",
            format!("{path}.operations[0]"),
            "schematic graphic operation kind must match its record",
        )
    })?;
    validate_graphic_operation(first, &format!("{path}.operations[0]"))?;
    if closed
        && !matches!(
            first,
            GraphicRef::Polyline(value)
                if !value.points.is_empty()
                    && value
                        .points
                        .first()
                        .zip(value.points.last())
                        .is_some_and(|(first, last)| first.0 == last.0)
        )
    {
        return Err(error(
            "open_rule_area",
            format!("{path}.operations[0].points"),
            "rule-area polylines must be closed",
        ));
    }
    if operations.len() == 1 {
        let canonical = match first.fill() {
            PlotterFill::NoFill => first.fill_color().is_none(),
            PlotterFill::FilledShape => true,
            _ => false,
        };
        return canonical.then_some(()).ok_or_else(|| {
            error(
                "invalid_graphic_fill",
                format!("{path}.operations[0]"),
                "single-pass schematic graphic fill state must be canonical",
            )
        });
    }
    let outline = graphic_ref(&operations[1], kind).ok_or_else(|| {
        error(
            "invalid_graphic_fill_pair",
            format!("{path}.operations[1]"),
            "schematic graphic fill pair must retain one operation kind",
        )
    })?;
    validate_graphic_operation(outline, &format!("{path}.operations[1]"))?;
    if matches!(first.fill(), PlotterFill::NoFill | PlotterFill::FilledShape)
        || first.width() != 0
        || first.fill_color().is_none()
        || first.stroke() != first.fill_color()
        || outline.fill() != PlotterFill::NoFill
        || outline.fill_color().is_some()
        || !first.same_line_style(outline)
        || !first.same_geometry(outline)
    {
        return Err(error(
            "invalid_graphic_fill_pair",
            format!("{path}.operations"),
            "schematic graphic fill and outline passes must be coherent",
        ));
    }
    Ok(())
}

fn validate_graphic_operation(value: GraphicRef<'_>, path: &str) -> Result<(), ValidationError> {
    let specialized = match value {
        GraphicRef::Polyline(polyline) => polyline.points.len() >= 2,
        GraphicRef::Circle(circle) => {
            circle.diameter_nm.get() >= 0
                && circle.role.is_none()
                && circle.layers.is_empty()
                && circle.mask_margin_nm.is_none()
                && circle.pad_size_x_nm.is_none()
                && circle.pad_size_y_nm.is_none()
        }
        GraphicRef::Rectangle(rectangle) => rectangle.corner_radius_nm.get() >= 0,
        GraphicRef::Arc(_) => true,
    };
    if value.layer().is_some()
        || value.width() < 0
        || value.stroke_color().is_none()
        || value.fill_color().is_some_and(|color| !valid_color(color))
        || !value.line_style_present()
        || !specialized
    {
        return Err(error(
            "invalid_graphic_style",
            path,
            "schematic graphic geometry and layerless style must be canonical",
        ));
    }
    Ok(())
}

fn valid_color(value: &str) -> bool {
    value.len() == 9
        && value.starts_with('#')
        && value[1..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'A'..=b'F'))
}

fn validate_bezier_operation(
    value: &BezierCurveOperation,
    path: &str,
) -> Result<(), ValidationError> {
    if value.layer.is_some()
        || value.width_nm.get() < 0
        || value.tolerance_nm.get() != 0
        || !value.stroke_color.as_deref().is_some_and(valid_color)
        || value.line_style.is_none()
    {
        return Err(error(
            "invalid_graphic_bezier",
            path,
            "schematic Bezier geometry and layerless style must be canonical",
        ));
    }
    Ok(())
}

fn validate_bezier_record(
    record: &SchematicGraphicBezierPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let [PlotterOperation::BezierCurveOperation(value)] = record.operations.as_slice() else {
        return Err(error(
            "invalid_graphic_bezier",
            format!("{path}.operations"),
            "schematic Bezier records contain exactly one cubic operation",
        ));
    };
    validate_bezier_operation(value, &format!("{path}.operations[0]"))
}

fn validate_rule_area(
    record: &SchematicRuleAreaPlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    match record.shape {
        SchematicRuleAreaShape::Polyline => {
            validate_graphic_record(&record.operations, GraphicKind::Polyline, true, path)
        }
        SchematicRuleAreaShape::Rectangle => {
            validate_graphic_record(&record.operations, GraphicKind::Rectangle, false, path)
        }
        SchematicRuleAreaShape::Arc => {
            validate_graphic_record(&record.operations, GraphicKind::Arc, false, path)
        }
        SchematicRuleAreaShape::Circle => {
            validate_graphic_record(&record.operations, GraphicKind::Circle, false, path)
        }
        SchematicRuleAreaShape::Bezier => {
            let [PlotterOperation::BezierCurveOperation(value)] = record.operations.as_slice()
            else {
                return Err(error(
                    "invalid_rule_area",
                    format!("{path}.operations"),
                    "Bezier rule areas contain exactly one cubic operation",
                ));
            };
            validate_bezier_operation(value, &format!("{path}.operations[0]"))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageFormat {
    Png,
    Jpeg,
    Bmp,
}

#[derive(Clone, Copy, Debug)]
struct ImageMetadata {
    format: ImageFormat,
    width: u32,
    height: u32,
    ppi_x: Option<u32>,
    ppi_y: Option<u32>,
}

fn validate_schematic_image(
    record: &SchematicImagePlotRecord,
    path: &str,
) -> Result<(), ValidationError> {
    let [PlotterOperation::PlotImageOperation(operation)] = record.operations.as_slice() else {
        return Err(error(
            "invalid_schematic_image",
            format!("{path}.operations"),
            "schematic image records contain exactly one image operation",
        ));
    };
    let metadata = decode_image_metadata(&operation.image_data_b64).ok_or_else(|| {
        error(
            "invalid_schematic_image",
            format!("{path}.operations[0].image_data_b64"),
            "schematic image data must be canonical supported base64",
        )
    })?;
    let expected_format = match metadata.format {
        ImageFormat::Png => SchematicImageFormat::Png,
        ImageFormat::Jpeg => SchematicImageFormat::Jpeg,
        ImageFormat::Bmp => SchematicImageFormat::Bmp,
    };
    let width_nm =
        image_extent(metadata.width, operation.scale, metadata.ppi_x).ok_or_else(|| {
            error(
                "invalid_schematic_image_extent",
                format!("{path}.operations[0].width_nm"),
                "schematic image width must remain finite and JavaScript-safe",
            )
        })?;
    let height_nm =
        image_extent(metadata.height, operation.scale, metadata.ppi_y).ok_or_else(|| {
            error(
                "invalid_schematic_image_extent",
                format!("{path}.operations[0].height_nm"),
                "schematic image height must remain finite and JavaScript-safe",
            )
        })?;
    if !operation.scale.is_finite()
        || operation.scale <= 0.0
        || operation.stroke_color.as_deref() != Some("#0000C2FF")
        || operation.image_format != expected_format.to_string()
        || record.image_format != expected_format
        || record.scale != operation.scale
        || record.width_nm != operation.width_nm
        || record.height_nm != operation.height_nm
        || operation.width_nm.get() != width_nm
        || operation.height_nm.get() != height_nm
        || width_nm <= 0
        || height_nm <= 0
    {
        return Err(error(
            "invalid_schematic_image_metadata",
            path,
            "schematic image record, operation, decoded metadata, and extent must agree",
        ));
    }
    Ok(())
}

fn decode_image_metadata(value: &str) -> Option<ImageMetadata> {
    let data = decode_base64(value)?;
    if data.len() >= 33
        && data.get(..8) == Some(b"\x89PNG\r\n\x1a\n")
        && data.get(8..16) == Some(b"\0\0\0\rIHDR")
    {
        return png_metadata(&data);
    }
    if data.len() >= 4 && data.get(..2) == Some(b"\xff\xd8") {
        return jpeg_metadata(&data);
    }
    if data.len() >= 26 && data.get(..2) == Some(b"BM") {
        return bmp_metadata(&data);
    }
    None
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(value.len().saturating_mul(3) / 4);
    let mut quartet = [0_u8; 4];
    let mut quartet_len = 0usize;
    let mut ended = false;
    for byte in value.bytes() {
        if byte.is_ascii_whitespace() || ended {
            return None;
        }
        quartet[quartet_len] = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return None,
        };
        quartet_len += 1;
        if quartet_len != 4 {
            continue;
        }
        if quartet[0] >= 64 || quartet[1] >= 64 {
            return None;
        }
        output.push((quartet[0] << 2) | (quartet[1] >> 4));
        if quartet[2] == 64 {
            if quartet[3] != 64 || quartet[1] & 0x0f != 0 {
                return None;
            }
            ended = true;
        } else {
            output.push((quartet[1] << 4) | (quartet[2] >> 2));
            if quartet[3] == 64 {
                if quartet[2] & 0x03 != 0 {
                    return None;
                }
                ended = true;
            } else {
                output.push((quartet[2] << 6) | quartet[3]);
            }
        }
        quartet_len = 0;
    }
    (quartet_len == 0).then_some(output)
}

fn png_metadata(data: &[u8]) -> Option<ImageMetadata> {
    let width = read_u32_be(data.get(16..20)?)?;
    let height = read_u32_be(data.get(20..24)?)?;
    if width == 0 || height == 0 {
        return None;
    }
    let mut ppi_x = None;
    let mut ppi_y = None;
    let mut position = 8usize;
    while position.checked_add(12)? <= data.len() {
        let length = usize::try_from(read_u32_be(data.get(position..position + 4)?)?).ok()?;
        let payload_start = position.checked_add(8)?;
        let payload_end = payload_start.checked_add(length)?;
        let end = payload_end.checked_add(4)?;
        if end > data.len() {
            return None;
        }
        let kind = data.get(position + 4..position + 8)?;
        if kind == b"pHYs" && length >= 9 {
            let payload = data.get(payload_start..payload_end)?;
            if payload[8] == 1 {
                ppi_x = ppi_from_ppm(read_u32_be(&payload[..4])?);
                ppi_y = ppi_from_ppm(read_u32_be(&payload[4..8])?);
            }
        }
        position = end;
        if kind == b"IEND" {
            break;
        }
    }
    Some(ImageMetadata {
        format: ImageFormat::Png,
        width,
        height,
        ppi_x,
        ppi_y,
    })
}

// This is a direct, allocation-free marker scanner. Keeping the marker state
// explicit makes the accepted JPEG subset auditable against the Python oracle.
#[allow(
    clippy::cognitive_complexity,
    reason = "the allocation-free marker scanner intentionally mirrors the Python authority"
)]
fn jpeg_metadata(data: &[u8]) -> Option<ImageMetadata> {
    let mut position = 2usize;
    let mut ppi_x = None;
    let mut ppi_y = None;
    while position.checked_add(9)? <= data.len() {
        if data[position] != 0xff {
            position += 1;
            continue;
        }
        let marker = data[position + 1];
        position += 2;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        let length = usize::from(read_u16_be(data.get(position..position + 2)?)?);
        if length < 2 || position.checked_add(length)? > data.len() {
            return None;
        }
        let payload = data.get(position + 2..position + length)?;
        if marker == 0xe0 && payload.starts_with(b"JFIF\0") && payload.len() >= 12 {
            let units = payload[7];
            let density_x = u32::from(read_u16_be(&payload[8..10])?);
            let density_y = u32::from(read_u16_be(&payload[10..12])?);
            if density_x > 0 && density_y > 0 {
                match units {
                    1 => {
                        ppi_x = Some(density_x);
                        ppi_y = Some(density_y);
                    }
                    2 => {
                        ppi_x = density_to_ppi(density_x);
                        ppi_y = density_to_ppi(density_y);
                    }
                    _ => {}
                }
            }
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if length < 7 {
                return None;
            }
            let height = u32::from(read_u16_be(data.get(position + 3..position + 5)?)?);
            let width = u32::from(read_u16_be(data.get(position + 5..position + 7)?)?);
            return (width > 0 && height > 0).then_some(ImageMetadata {
                format: ImageFormat::Jpeg,
                width,
                height,
                ppi_x,
                ppi_y,
            });
        }
        position += length;
    }
    None
}

fn bmp_metadata(data: &[u8]) -> Option<ImageMetadata> {
    let dib = read_u32_le(data.get(14..18)?)?;
    let (width, height, ppi_x, ppi_y) = if dib == 12 {
        (
            u32::from(read_u16_le(data.get(18..20)?)?),
            u32::from(read_u16_le(data.get(20..22)?)?),
            None,
            None,
        )
    } else if dib >= 40 && data.len() >= 54 {
        let width = read_i32_le(data.get(18..22)?)?.unsigned_abs();
        let height = read_i32_le(data.get(22..26)?)?.unsigned_abs();
        let ppi_x = bmp_ppi(read_i32_le(data.get(38..42)?)?);
        let ppi_y = bmp_ppi(read_i32_le(data.get(42..46)?)?);
        (width, height, ppi_x, ppi_y)
    } else {
        return None;
    };
    (width > 0 && height > 0).then_some(ImageMetadata {
        format: ImageFormat::Bmp,
        width,
        height,
        ppi_x,
        ppi_y,
    })
}

fn ppi_from_ppm(value: u32) -> Option<u32> {
    (value > 0)
        .then(|| (f64::from(value) * 0.0254).round_ties_even() as u32)
        .filter(|value| *value > 0)
}

fn density_to_ppi(value: u32) -> Option<u32> {
    (value > 0)
        .then(|| (f64::from(value) * 2.54).round_ties_even() as u32)
        .filter(|value| *value > 0)
}

fn bmp_ppi(value: i32) -> Option<u32> {
    (value > 0)
        .then(|| ((value / 100) as f64 * 2.54).round_ties_even() as u32)
        .filter(|value| *value > 0)
}

fn image_extent(size: u32, scale: f64, ppi: Option<u32>) -> Option<i64> {
    let density = f64::from(ppi.unwrap_or(300));
    let value = f64::from(size) * scale * 25.4 / density * 1_000_000.0;
    (value.is_finite()
        && value >= crate::JAVASCRIPT_SAFE_INTEGER_MIN as f64
        && value <= crate::JAVASCRIPT_SAFE_INTEGER_MAX as f64)
        .then(|| value.round_ties_even() as i64)
}

fn read_u16_be(value: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes(value.try_into().ok()?))
}

fn read_u16_le(value: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(value.try_into().ok()?))
}

fn read_u32_be(value: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(value.try_into().ok()?))
}

fn read_u32_le(value: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(value.try_into().ok()?))
}

fn read_i32_le(value: &[u8]) -> Option<i32> {
    Some(i32::from_le_bytes(value.try_into().ok()?))
}

fn validate_table(record: &SchematicTablePlotRecord, path: &str) -> Result<(), ValidationError> {
    let mut operation_index = 0usize;
    let mut cells = 0usize;
    while operation_index < record.operations.len() {
        let prefix = validate_text_box_prefix(
            &record.operations[operation_index..],
            &format!("{path}.operations[{operation_index}]"),
        )?;
        operation_index += prefix;
        while let Some(PlotterOperation::TextOperation(text)) =
            record.operations.get(operation_index)
        {
            if text.text.is_empty() || text.multiline {
                return Err(error(
                    "invalid_table_cell_line",
                    format!("{path}.operations[{operation_index}]"),
                    "table cell lines must be nonempty single-line text",
                ));
            }
            validate_annotation_text(text, &format!("{path}.operations[{operation_index}]"))?;
            operation_index += 1;
        }
        cells = cells.saturating_add(1);
    }
    if cells != record.cell_count as usize {
        return Err(error(
            "table_cell_count_mismatch",
            format!("{path}.cell_count"),
            "table cell_count must equal the rendered cell blocks",
        ));
    }
    Ok(())
}

fn error(code: &'static str, path: impl Into<String>, message: &'static str) -> ValidationError {
    validation_error(code, path, message)
}

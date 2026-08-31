//! Record-state validation for the board plotter IR document contract.

use crate::generated::board_plot_document::{
    BoardDimensionType, BoardFootprintChildAttrs, BoardFootprintChildAttrsFootprintGraphicKind,
    BoardFootprintChildAttrsPrimitive, BoardFootprintChildRef, BoardFootprintLayerRole,
    BoardFootprintOperation, BoardFootprintPadBlockAttrs, BoardFootprintPadBlockAttrsPrimitive,
    BoardFootprintPlotRecord, BoardFootprintStartBlockOperation,
    BoardFootprintStartBlockOperationDataRef, BoardFootprintTextOperation, BoardPlotDocumentA0,
    BoardPlotRecord, BoardTextBoxPlotRecord, BoardTextPlotRecord, CircleOperation,
    DimensionPlotRecord, FlashPadCircleOperation, PlotterDrillRole as BoardDrillRole,
    PlotterFill as BoardFill, PlotterOperation as BoardOperation,
    PlotterTextRenderCacheCoordinateSpace, PlotterViaFlashRole, RectOperation, TablePlotRecord,
    TextOperation, ThickSegmentOperation,
};
use crate::{ValidationError, validation_error};

/// Enforce record counts and record-kind-specific operation states for boards.
pub fn validate_board_plot_document(document: &BoardPlotDocumentA0) -> Result<(), ValidationError> {
    if document.schema != "kicad.plotter_ir.a0"
        || document.source_kind != "PCB"
        || document.coordinate_space.unit != "nm"
        || document.coordinate_space.y_axis != "down"
    {
        return Err(validation_error(
            "unsupported_contract",
            "$",
            "board plot document identity and coordinate space must match a0",
        ));
    }
    let mut total_operations = 0usize;
    let mut saw_footprint = false;
    for (record_index, record) in document.records.iter().enumerate() {
        if matches!(record, BoardPlotRecord::BoardFootprintPlotRecord(_)) {
            saw_footprint = true;
        } else if saw_footprint {
            return Err(validation_error(
                "invalid_board_record_order",
                format!("$.records[{record_index}]"),
                "embedded footprint records form the terminal board record phase",
            ));
        }
        let (declared, operation_count) = validate_board_record(record, record_index)?;
        if declared != operation_count {
            return Err(validation_error(
                "operation_count_mismatch",
                format!("$.records[{record_index}].operation_count"),
                "operation_count must equal the operation array length",
            ));
        }
        total_operations = total_operations.saturating_add(operation_count);
    }
    if document.total_operations as usize != total_operations {
        return Err(validation_error(
            "operation_count_mismatch",
            "$.total_operations",
            "total_operations must equal all record operation counts",
        ));
    }
    Ok(())
}

fn validate_board_record(
    record: &BoardPlotRecord,
    record_index: usize,
) -> Result<(usize, usize), ValidationError> {
    let counts = match record {
        BoardPlotRecord::BoardGraphicPlotRecord(value) => {
            validate_board_graphic_operations(&value.operations, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::TrackSegmentPlotRecord(value) => {
            validate_track_segment_operations(&value.operations, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::TrackArcPlotRecord(value) => {
            validate_track_arc_operations(&value.operations, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::ViaPlotRecord(value) => {
            validate_via_operations(&value.operations, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::TablePlotRecord(value) => {
            validate_table_operations(value, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::DimensionPlotRecord(value) => {
            validate_dimension_operations(value, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::ZoneFillPlotRecord(value) => {
            validate_zone_fill_operations(value, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::BoardTextPlotRecord(value) => {
            validate_board_text_operations(value, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::BoardTextBoxPlotRecord(value) => {
            validate_board_text_box_operations(value, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
        BoardPlotRecord::BoardFootprintPlotRecord(value) => {
            validate_board_footprint_record(value, record_index)?;
            (value.operation_count as usize, value.operations.len())
        }
    };
    Ok(counts)
}

struct FootprintChildMetadata<'a> {
    index: u32,
    kind: &'a str,
    label: Option<&'a str>,
    data_uuid: Option<&'a str>,
    data_ref: Option<BoardFootprintChildRef>,
    object_id: Option<&'a str>,
    extra_attrs: Option<&'a BoardFootprintChildAttrs>,
    layer: Option<&'a str>,
}

macro_rules! footprint_child_metadata {
    ($value:expr) => {
        FootprintChildMetadata {
            index: $value.index,
            kind: &$value.kind,
            label: $value.label.as_deref(),
            data_uuid: $value.data_uuid.as_deref(),
            data_ref: $value.data_ref,
            object_id: $value.object_id.as_deref(),
            extra_attrs: $value.extra_attrs.as_ref(),
            layer: $value.layer.as_deref(),
        }
    };
}

fn validate_board_footprint_record(
    record: &BoardFootprintPlotRecord,
    record_index: usize,
) -> Result<(), ValidationError> {
    let record_path = format!("$.records[{record_index}]");
    if record.kind != "footprint"
        || record.object_id != record.library_link
        || !record.placement.angle_deg.is_finite()
    {
        return Err(invalid_board_footprint(
            record_path,
            "embedded footprint identity and finite placement must be canonical",
        ));
    }
    let mut operation_index = 0usize;
    let mut pad_phase = false;
    let mut last_child_key: Option<(u8, u32, u32)> = None;
    while operation_index < record.operations.len() {
        let path = format!("$.records[{record_index}].operations[{operation_index}]");
        match &record.operations[operation_index] {
            BoardFootprintOperation::StartBlockOperation(start) => {
                pad_phase = true;
                let Some(inner) = record.operations.get(operation_index + 1) else {
                    return Err(invalid_board_footprint(
                        path,
                        "pad blocks must contain exactly one operation and an EndBlock",
                    ));
                };
                let Some(BoardFootprintOperation::EndBlockOperation(end)) =
                    record.operations.get(operation_index + 2)
                else {
                    return Err(invalid_board_footprint(
                        path,
                        "pad blocks must contain exactly one operation and an EndBlock",
                    ));
                };
                validate_board_footprint_header(
                    start.index,
                    &start.kind,
                    operation_index,
                    "StartBlock",
                    &path,
                )?;
                validate_board_footprint_header(
                    board_footprint_operation_index(inner),
                    board_footprint_operation_kind(inner),
                    operation_index + 1,
                    board_footprint_expected_kind(inner),
                    &format!(
                        "$.records[{record_index}].operations[{}]",
                        operation_index + 1
                    ),
                )?;
                validate_board_footprint_header(
                    end.index,
                    &end.kind,
                    operation_index + 2,
                    "EndBlock",
                    &format!(
                        "$.records[{record_index}].operations[{}]",
                        operation_index + 2
                    ),
                )?;
                validate_board_footprint_pad_block(record, start, inner, path)?;
                operation_index += 3;
            }
            BoardFootprintOperation::EndBlockOperation(_) if pad_phase => {
                return Err(invalid_board_footprint(
                    path,
                    "EndBlock must close a complete pad block",
                ));
            }
            _ if pad_phase => {
                return Err(invalid_board_footprint(
                    path,
                    "footprint child operations cannot follow the terminal pad-block phase",
                ));
            }
            operation => {
                validate_board_footprint_child(
                    record,
                    operation,
                    operation_index,
                    &mut last_child_key,
                    path,
                )?;
                operation_index += 1;
            }
        }
    }
    Ok(())
}

fn validate_board_footprint_child(
    record: &BoardFootprintPlotRecord,
    operation: &BoardFootprintOperation,
    expected_index: usize,
    last_key: &mut Option<(u8, u32, u32)>,
    path: String,
) -> Result<(), ValidationError> {
    let metadata = match operation {
        BoardFootprintOperation::ThickSegmentOperation(value) => footprint_child_metadata!(value),
        BoardFootprintOperation::ArcThreePointOperation(value) => footprint_child_metadata!(value),
        BoardFootprintOperation::CircleOperation(value) => footprint_child_metadata!(value),
        BoardFootprintOperation::RectOperation(value) => footprint_child_metadata!(value),
        BoardFootprintOperation::PlotPolyOperation(value) => footprint_child_metadata!(value),
        BoardFootprintOperation::TextOperation(value) => {
            validate_board_footprint_text(value, &path)?;
            footprint_child_metadata!(value)
        }
        BoardFootprintOperation::BezierCurveOperation(_)
        | BoardFootprintOperation::FlashPadCircleOperation(_)
        | BoardFootprintOperation::FlashPadOvalOperation(_)
        | BoardFootprintOperation::FlashPadRectOperation(_)
        | BoardFootprintOperation::FlashPadRoundRectOperation(_)
        | BoardFootprintOperation::FlashPadCustomOperation(_)
        | BoardFootprintOperation::FlashPadTrapezOperation(_)
        | BoardFootprintOperation::StartBlockOperation(_)
        | BoardFootprintOperation::EndBlockOperation(_) => {
            return Err(invalid_board_footprint(
                path,
                "direct footprint children admit only emitted text and graphic geometry",
            ));
        }
    };
    validate_board_footprint_header(
        metadata.index,
        metadata.kind,
        expected_index,
        board_footprint_expected_kind(operation),
        &path,
    )?;
    let (Some(label), Some(data_uuid), Some(data_ref), Some(object_id), Some(attrs)) = (
        metadata.label,
        metadata.data_uuid,
        metadata.data_ref,
        metadata.object_id,
        metadata.extra_attrs,
    ) else {
        return Err(invalid_board_footprint(
            path,
            "direct footprint child metadata fields are all required together",
        ));
    };
    if label.is_empty()
        || data_uuid.is_empty()
        || object_id.is_empty()
        || data_ref != attrs.footprint_primitive
        || attrs.component != record.reference
        || attrs.component_uid != record.uuid
        || attrs.component_uuid != record.uuid
        || attrs.footprint != record.library_link
        || attrs.layer_name.as_deref() != metadata.layer
        || attrs.layer_name.is_some() != attrs.layer_role.is_some()
        || attrs
            .layer_name
            .as_deref()
            .is_some_and(|layer| attrs.layer_role != Some(board_footprint_layer_role(layer)))
    {
        return Err(invalid_board_footprint(
            path,
            "child metadata must identify its source, parent footprint, and operation layer",
        ));
    }
    validate_board_footprint_child_shape(operation, data_ref, attrs, &path)?;
    let key = (
        board_footprint_child_phase(data_ref),
        attrs.footprint_object_index,
        attrs.footprint_subop_index.unwrap_or(0),
    );
    if last_key.is_some_and(|previous| previous >= key) {
        return Err(invalid_board_footprint(
            path,
            "footprint children must remain in canonical source-kind and object order",
        ));
    }
    *last_key = Some(key);
    Ok(())
}

fn validate_board_footprint_child_shape(
    operation: &BoardFootprintOperation,
    data_ref: BoardFootprintChildRef,
    attrs: &BoardFootprintChildAttrs,
    path: &str,
) -> Result<(), ValidationError> {
    let text_ref = matches!(
        data_ref,
        BoardFootprintChildRef::Property
            | BoardFootprintChildRef::FpText
            | BoardFootprintChildRef::FpTextBox
    );
    let is_text = matches!(operation, BoardFootprintOperation::TextOperation(_));
    let graphic_kind = match operation {
        BoardFootprintOperation::ThickSegmentOperation(value) => {
            if value.stroke_color.is_some() {
                return Err(invalid_board_footprint(
                    path.to_owned(),
                    "embedded footprint segments do not emit stroke_color",
                ));
            }
            Some(if data_ref == BoardFootprintChildRef::FpTextBox {
                BoardFootprintChildAttrsFootprintGraphicKind::TextBoxBorder
            } else {
                BoardFootprintChildAttrsFootprintGraphicKind::Line
            })
        }
        BoardFootprintOperation::ArcThreePointOperation(_) => {
            Some(BoardFootprintChildAttrsFootprintGraphicKind::Arc)
        }
        BoardFootprintOperation::CircleOperation(_) => {
            Some(BoardFootprintChildAttrsFootprintGraphicKind::Circle)
        }
        BoardFootprintOperation::RectOperation(_) => {
            Some(if data_ref == BoardFootprintChildRef::FpTextBox {
                BoardFootprintChildAttrsFootprintGraphicKind::TextBoxBorder
            } else {
                BoardFootprintChildAttrsFootprintGraphicKind::Rect
            })
        }
        BoardFootprintOperation::PlotPolyOperation(_) => {
            Some(BoardFootprintChildAttrsFootprintGraphicKind::Poly)
        }
        _ => None,
    };
    let valid_ref = match operation {
        BoardFootprintOperation::TextOperation(_) => text_ref,
        BoardFootprintOperation::ThickSegmentOperation(_) => matches!(
            data_ref,
            BoardFootprintChildRef::FpTextBox | BoardFootprintChildRef::FpLine
        ),
        BoardFootprintOperation::ArcThreePointOperation(_) => {
            data_ref == BoardFootprintChildRef::FpArc
        }
        BoardFootprintOperation::CircleOperation(_) => data_ref == BoardFootprintChildRef::FpCircle,
        BoardFootprintOperation::RectOperation(_) => matches!(
            data_ref,
            BoardFootprintChildRef::FpTextBox | BoardFootprintChildRef::FpRect
        ),
        BoardFootprintOperation::PlotPolyOperation(_) => data_ref == BoardFootprintChildRef::FpPoly,
        _ => false,
    };
    let shape_valid = if is_text {
        attrs.primitive == BoardFootprintChildAttrsPrimitive::FootprintText
            && attrs.footprint_text_role.is_some()
            && attrs.footprint_graphic_kind.is_none()
            && (data_ref == BoardFootprintChildRef::Property) == attrs.property_name.is_some()
            && (data_ref == BoardFootprintChildRef::FpText) == attrs.fp_text_type.is_some()
    } else {
        attrs.primitive == BoardFootprintChildAttrsPrimitive::FootprintGraphic
            && attrs.footprint_text_role.is_none()
            && attrs.property_name.is_none()
            && attrs.fp_text_type.is_none()
            && attrs.footprint_graphic_kind == graphic_kind
    };
    let subop_required = matches!(
        data_ref,
        BoardFootprintChildRef::FpTextBox
            | BoardFootprintChildRef::FpLine
            | BoardFootprintChildRef::FpArc
    );
    if valid_ref && shape_valid && (attrs.footprint_subop_index.is_some() == subop_required) {
        Ok(())
    } else {
        Err(invalid_board_footprint(
            path.to_owned(),
            "child ref, primitive, and typed attributes must match the operation shape",
        ))
    }
}

fn board_footprint_layer_role(layer: &str) -> BoardFootprintLayerRole {
    if layer.ends_with(".Cu") || matches!(layer, "*.Cu" | "F&B.Cu") {
        BoardFootprintLayerRole::Copper
    } else if layer.ends_with(".SilkS") {
        BoardFootprintLayerRole::Silkscreen
    } else if layer.ends_with(".Mask") || layer == "*.Mask" {
        BoardFootprintLayerRole::Soldermask
    } else if layer.ends_with(".Paste") {
        BoardFootprintLayerRole::Paste
    } else if layer.ends_with(".Fab") {
        BoardFootprintLayerRole::Fab
    } else if layer.ends_with(".Courtyard") {
        BoardFootprintLayerRole::Courtyard
    } else if layer == "Edge.Cuts" {
        BoardFootprintLayerRole::BoardOutline
    } else if layer == "DRILLS" {
        BoardFootprintLayerRole::Drill
    } else if layer.ends_with(".User") || layer.starts_with("User.") {
        BoardFootprintLayerRole::User
    } else {
        BoardFootprintLayerRole::Other
    }
}

fn board_footprint_child_phase(value: BoardFootprintChildRef) -> u8 {
    match value {
        BoardFootprintChildRef::Property => 0,
        BoardFootprintChildRef::FpText => 1,
        BoardFootprintChildRef::FpTextBox => 2,
        BoardFootprintChildRef::FpLine => 3,
        BoardFootprintChildRef::FpArc => 4,
        BoardFootprintChildRef::FpCircle => 5,
        BoardFootprintChildRef::FpRect => 6,
        BoardFootprintChildRef::FpPoly => 7,
    }
}

fn validate_board_footprint_pad_block(
    record: &BoardFootprintPlotRecord,
    start: &BoardFootprintStartBlockOperation,
    inner: &BoardFootprintOperation,
    path: String,
) -> Result<(), ValidationError> {
    let attrs = &start.extra_attrs;
    let pad_number = attrs.pad_number.as_deref();
    let pad_number_matches = pad_number
        .map(|number| number == start.object_id)
        .unwrap_or(start.object_id == "pad");
    let designator = match (record.reference.as_str(), pad_number) {
        ("", None) => None,
        ("", Some(number)) => Some(number.to_owned()),
        (component, Some(number)) => Some(format!("{component}-{number}")),
        (_, None) => None,
    };
    let layer_names = board_footprint_inner_layers(inner).join(",");
    let common = optional_nonempty_matches(attrs.component.as_deref(), &record.reference)
        && optional_nonempty_matches(attrs.component_uid.as_deref(), &record.uuid)
        && optional_nonempty_matches(attrs.component_uuid.as_deref(), &record.uuid)
        && optional_nonempty_matches(attrs.footprint.as_deref(), &record.library_link)
        && pad_number_matches
        && attrs.pad_designator.as_deref() == designator.as_deref()
        && attrs
            .pad_type
            .as_ref()
            .is_none_or(|value| !value.is_empty())
        && attrs
            .pad_shape
            .as_ref()
            .is_none_or(|value| !value.is_empty())
        && optional_nonempty_matches(attrs.layer_names.as_deref(), &layer_names)
        && start.label == start.data_uuid;
    if !common || !board_footprint_operation_metadata_absent(inner) {
        return Err(invalid_board_footprint(
            path,
            "pad block metadata must identify its parent and remain only on StartBlock",
        ));
    }
    match start.data_ref {
        BoardFootprintStartBlockOperationDataRef::Pad => {
            let no_hole_attrs = attrs.primitive == BoardFootprintPadBlockAttrsPrimitive::Pad
                && attrs.hole_owner.is_none()
                && attrs.hole_kind.is_none()
                && attrs.hole_plating.is_none()
                && attrs.hole_render.is_none()
                && attrs.hole_width_mm.is_none()
                && attrs.hole_height_mm.is_none()
                && attrs.hole_diameter_mm.is_none();
            let layers_are_canonical =
                !start.layers.is_empty() || attrs.pad_type.as_deref() == Some("np_thru_hole");
            if no_hole_attrs
                && layers_are_canonical
                && validate_board_footprint_pad_flash(inner, &start.layers)
            {
                Ok(())
            } else {
                Err(invalid_board_footprint(
                    path,
                    "pad blocks require one canonical layered flash and no hole attributes",
                ))
            }
        }
        BoardFootprintStartBlockOperationDataRef::PadHole => {
            if validate_board_footprint_hole_attrs(attrs, &start.label)
                && validate_board_footprint_drill(inner, attrs)
            {
                Ok(())
            } else {
                Err(invalid_board_footprint(
                    path,
                    "pad_hole blocks require one drill operation and complete matching hole attributes",
                ))
            }
        }
    }
}

fn optional_nonempty_matches(actual: Option<&str>, source: &str) -> bool {
    actual == (!source.is_empty()).then_some(source)
}

fn board_footprint_inner_layers(operation: &BoardFootprintOperation) -> &[String] {
    match operation {
        BoardFootprintOperation::ThickSegmentOperation(value) => &value.layers,
        BoardFootprintOperation::CircleOperation(value) => &value.layers,
        BoardFootprintOperation::FlashPadCircleOperation(value) => &value.layers,
        BoardFootprintOperation::FlashPadOvalOperation(value) => &value.layers,
        BoardFootprintOperation::FlashPadRectOperation(value) => &value.layers,
        BoardFootprintOperation::FlashPadRoundRectOperation(value) => &value.layers,
        BoardFootprintOperation::FlashPadCustomOperation(value) => &value.layers,
        BoardFootprintOperation::FlashPadTrapezOperation(value) => &value.layers,
        _ => &[],
    }
}

fn validate_board_footprint_hole_attrs(attrs: &BoardFootprintPadBlockAttrs, label: &str) -> bool {
    use crate::generated::board_plot_document::BoardFootprintPadBlockAttrsHoleKind as HoleKind;
    use crate::generated::board_plot_document::BoardFootprintPadBlockAttrsHolePlating as Plating;
    let dimensions = match attrs.hole_kind {
        Some(HoleKind::Round) => {
            attrs.hole_diameter_mm.is_some()
                && attrs.hole_width_mm.is_none()
                && attrs.hole_height_mm.is_none()
        }
        Some(HoleKind::Slot) => {
            attrs.hole_diameter_mm.is_none()
                && attrs.hole_width_mm.is_some()
                && attrs.hole_height_mm.is_some()
        }
        None => false,
    };
    let owner = label.strip_suffix(":hole");
    attrs.primitive == BoardFootprintPadBlockAttrsPrimitive::PadHole
        && owner.is_some()
        && attrs.hole_owner.as_deref() == owner
        && attrs
            .hole_plating
            .is_some_and(|value| matches!(value, Plating::Plated | Plating::NonPlated))
        && attrs.hole_render.is_some()
        && dimensions
}

fn validate_board_footprint_pad_flash(
    operation: &BoardFootprintOperation,
    start_layers: &[String],
) -> bool {
    match operation {
        BoardFootprintOperation::FlashPadCircleOperation(value) => {
            value.kind == "FlashPadCircle"
                && value.role.is_none()
                && value.mask_margin_nm.is_some()
                && value.layers == start_layers
        }
        BoardFootprintOperation::FlashPadOvalOperation(value) => {
            value.kind == "FlashPadOval" && value.layers == start_layers
        }
        BoardFootprintOperation::FlashPadRectOperation(value) => {
            value.kind == "FlashPadRect" && value.layers == start_layers
        }
        BoardFootprintOperation::FlashPadRoundRectOperation(value) => {
            value.kind == "FlashPadRoundRect" && value.layers == start_layers
        }
        BoardFootprintOperation::FlashPadCustomOperation(value) => {
            value.kind == "FlashPadCustom"
                && value.layers == start_layers
                && (value.polygon_widths_nm.is_empty()
                    || value.polygon_widths_nm.len() == value.polygons.len())
        }
        BoardFootprintOperation::FlashPadTrapezOperation(value) => {
            value.kind == "FlashPadTrapez" && value.layers == start_layers
        }
        _ => false,
    }
}

fn validate_board_footprint_drill(
    operation: &BoardFootprintOperation,
    attrs: &BoardFootprintPadBlockAttrs,
) -> bool {
    use crate::generated::board_plot_document::BoardFootprintPadBlockAttrsHolePlating as Plating;
    let (role, layer, layers, mask, size_x, size_y) = match operation {
        BoardFootprintOperation::CircleOperation(value) => (
            value.role,
            value.layer.as_deref(),
            value.layers.as_slice(),
            value.mask_margin_nm.is_some(),
            value.pad_size_x_nm.is_some(),
            value.pad_size_y_nm.is_some(),
        ),
        BoardFootprintOperation::ThickSegmentOperation(value) => (
            value.role,
            value.layer.as_deref(),
            value.layers.as_slice(),
            value.mask_margin_nm.is_some(),
            value.pad_size_x_nm.is_some(),
            value.pad_size_y_nm.is_some(),
        ),
        _ => return false,
    };
    if layer.is_some() {
        return false;
    }
    match attrs.hole_plating {
        Some(Plating::Plated) => {
            !layers.is_empty()
                && role == Some(BoardDrillRole::PadDrill)
                && !mask
                && !size_x
                && !size_y
        }
        Some(Plating::NonPlated) => {
            role == Some(BoardDrillRole::NpthHole) && mask && size_x && size_y
        }
        None => false,
    }
}

fn board_footprint_operation_metadata_absent(operation: &BoardFootprintOperation) -> bool {
    macro_rules! absent {
        ($value:expr) => {
            $value.label.is_none()
                && $value.data_uuid.is_none()
                && $value.data_ref.is_none()
                && $value.object_id.is_none()
                && $value.extra_attrs.is_none()
        };
    }
    match operation {
        BoardFootprintOperation::ThickSegmentOperation(value) => absent!(value),
        BoardFootprintOperation::ArcThreePointOperation(value) => absent!(value),
        BoardFootprintOperation::CircleOperation(value) => absent!(value),
        BoardFootprintOperation::RectOperation(value) => absent!(value),
        BoardFootprintOperation::PlotPolyOperation(value) => absent!(value),
        BoardFootprintOperation::BezierCurveOperation(value) => absent!(value),
        BoardFootprintOperation::TextOperation(value) => absent!(value),
        BoardFootprintOperation::FlashPadCircleOperation(value) => absent!(value),
        BoardFootprintOperation::FlashPadOvalOperation(value) => absent!(value),
        BoardFootprintOperation::FlashPadRectOperation(value) => absent!(value),
        BoardFootprintOperation::FlashPadRoundRectOperation(value) => absent!(value),
        BoardFootprintOperation::FlashPadCustomOperation(value) => absent!(value),
        BoardFootprintOperation::FlashPadTrapezOperation(value) => absent!(value),
        BoardFootprintOperation::StartBlockOperation(_)
        | BoardFootprintOperation::EndBlockOperation(_) => false,
    }
}

fn validate_board_footprint_text(
    value: &BoardFootprintTextOperation,
    path: &str,
) -> Result<(), ValidationError> {
    let marker_state = value.context.is_none()
        && value.mirror.is_none()
        && value.text_as_polygons.is_none()
        && value.polyline_per_segment.is_none()
        && value.knockout != Some(false);
    let has_cache = value.render_cache.is_some();
    let cache_keys = has_cache == value.render_cache_source.is_some()
        && has_cache == value.render_cache_exact.is_some()
        && has_cache != value.render_cache_polygons.is_empty();
    let cache_state = match &value.render_cache {
        Some(cache) => {
            cache.schema == "kicad.render_cache.v1"
                && cache.unit == "nm"
                && cache.coordinate_space == PlotterTextRenderCacheCoordinateSpace::FootprintLocal
                && value.render_cache_source == Some(cache.source)
                && value.render_cache_exact == Some(cache.exact)
                && cache.text == value.text
                && cache.angle.is_finite()
                && cache.knockout == value.knockout
                && cache.polygons.len() == value.render_cache_polygons.len()
                && cache.polygons.iter().zip(&value.render_cache_polygons).all(
                    |(polygon, exterior)| {
                        !polygon.contours.is_empty()
                            && polygon.contours.iter().all(|contour| contour.len() >= 3)
                            && points_equal(&polygon.contours[0], exterior)
                    },
                )
        }
        None => value.knockout.is_none(),
    };
    if value.orient_deg.is_finite() && marker_state && cache_keys && cache_state {
        Ok(())
    } else {
        Err(invalid_board_footprint(
            path.to_owned(),
            "footprint text markers and footprint-local render cache must be coherent",
        ))
    }
}

fn validate_board_footprint_header(
    actual_index: u32,
    actual_kind: &str,
    expected_index: usize,
    expected_kind: &str,
    path: &str,
) -> Result<(), ValidationError> {
    let expected_index = u32::try_from(expected_index).map_err(|_| {
        invalid_board_footprint(
            format!("{path}.index"),
            "operation index exceeds the contract range",
        )
    })?;
    if actual_index != expected_index || actual_kind != expected_kind {
        Err(invalid_board_footprint(
            path.to_owned(),
            "operation kind and index must match its structural variant and array position",
        ))
    } else {
        Ok(())
    }
}

fn board_footprint_operation_index(operation: &BoardFootprintOperation) -> u32 {
    match operation {
        BoardFootprintOperation::ThickSegmentOperation(value) => value.index,
        BoardFootprintOperation::ArcThreePointOperation(value) => value.index,
        BoardFootprintOperation::CircleOperation(value) => value.index,
        BoardFootprintOperation::RectOperation(value) => value.index,
        BoardFootprintOperation::PlotPolyOperation(value) => value.index,
        BoardFootprintOperation::BezierCurveOperation(value) => value.index,
        BoardFootprintOperation::TextOperation(value) => value.index,
        BoardFootprintOperation::FlashPadCircleOperation(value) => value.index,
        BoardFootprintOperation::FlashPadOvalOperation(value) => value.index,
        BoardFootprintOperation::FlashPadRectOperation(value) => value.index,
        BoardFootprintOperation::FlashPadRoundRectOperation(value) => value.index,
        BoardFootprintOperation::FlashPadCustomOperation(value) => value.index,
        BoardFootprintOperation::FlashPadTrapezOperation(value) => value.index,
        BoardFootprintOperation::StartBlockOperation(value) => value.index,
        BoardFootprintOperation::EndBlockOperation(value) => value.index,
    }
}

fn board_footprint_operation_kind(operation: &BoardFootprintOperation) -> &str {
    match operation {
        BoardFootprintOperation::ThickSegmentOperation(value) => &value.kind,
        BoardFootprintOperation::ArcThreePointOperation(value) => &value.kind,
        BoardFootprintOperation::CircleOperation(value) => &value.kind,
        BoardFootprintOperation::RectOperation(value) => &value.kind,
        BoardFootprintOperation::PlotPolyOperation(value) => &value.kind,
        BoardFootprintOperation::BezierCurveOperation(value) => &value.kind,
        BoardFootprintOperation::TextOperation(value) => &value.kind,
        BoardFootprintOperation::FlashPadCircleOperation(value) => &value.kind,
        BoardFootprintOperation::FlashPadOvalOperation(value) => &value.kind,
        BoardFootprintOperation::FlashPadRectOperation(value) => &value.kind,
        BoardFootprintOperation::FlashPadRoundRectOperation(value) => &value.kind,
        BoardFootprintOperation::FlashPadCustomOperation(value) => &value.kind,
        BoardFootprintOperation::FlashPadTrapezOperation(value) => &value.kind,
        BoardFootprintOperation::StartBlockOperation(value) => &value.kind,
        BoardFootprintOperation::EndBlockOperation(value) => &value.kind,
    }
}

fn board_footprint_expected_kind(operation: &BoardFootprintOperation) -> &'static str {
    match operation {
        BoardFootprintOperation::ThickSegmentOperation(_) => "ThickSegment",
        BoardFootprintOperation::ArcThreePointOperation(_) => "ArcThreePoint",
        BoardFootprintOperation::CircleOperation(_) => "Circle",
        BoardFootprintOperation::RectOperation(_) => "Rect",
        BoardFootprintOperation::PlotPolyOperation(_) => "PlotPoly",
        BoardFootprintOperation::BezierCurveOperation(_) => "BezierCurve",
        BoardFootprintOperation::TextOperation(_) => "Text",
        BoardFootprintOperation::FlashPadCircleOperation(_) => "FlashPadCircle",
        BoardFootprintOperation::FlashPadOvalOperation(_) => "FlashPadOval",
        BoardFootprintOperation::FlashPadRectOperation(_) => "FlashPadRect",
        BoardFootprintOperation::FlashPadRoundRectOperation(_) => "FlashPadRoundRect",
        BoardFootprintOperation::FlashPadCustomOperation(_) => "FlashPadCustom",
        BoardFootprintOperation::FlashPadTrapezOperation(_) => "FlashPadTrapez",
        BoardFootprintOperation::StartBlockOperation(_) => "StartBlock",
        BoardFootprintOperation::EndBlockOperation(_) => "EndBlock",
    }
}

fn invalid_board_footprint(path: String, message: &'static str) -> ValidationError {
    validation_error("invalid_board_footprint", path, message)
}

fn validate_dimension_operations(
    record: &DimensionPlotRecord,
    record_index: usize,
) -> Result<(), ValidationError> {
    if record.kind != "dimension"
        || record.object_id != "dimension"
        || record.layers.is_empty()
        || !record.layers.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(invalid_dimension(
            format!("$.records[{record_index}]"),
            "dimension identity must be canonical and layers must be nonempty, sorted, and unique",
        ));
    }
    let mut saw_text = false;
    let mut marker_count = 0usize;
    for (operation_index, operation) in record.operations.iter().enumerate() {
        let path = format!("$.records[{record_index}].operations[{operation_index}]");
        match operation {
            BoardOperation::TextOperation(value) => {
                if operation_index != 0 || saw_text || value.index as usize != operation_index {
                    return Err(invalid_dimension(
                        path,
                        "dimension faced text appears at most once and first",
                    ));
                }
                saw_text = true;
                validate_text_payload(value, &path)?;
                let layer_is_declared = value
                    .layer
                    .as_ref()
                    .is_some_and(|layer| record.layers.binary_search(layer).is_ok());
                if value.font_face.is_empty() || !layer_is_declared {
                    return Err(invalid_dimension(
                        path,
                        "dimension Text must be faced and use one declared layer",
                    ));
                }
            }
            BoardOperation::ThickSegmentOperation(value) => {
                if value.index as usize != operation_index
                    || value.kind != "ThickSegment"
                    || !dimension_layer_is_declared(value.layer.as_deref(), &record.layers)
                    || !table_segment_has_graphic_state(value)
                {
                    return Err(invalid_dimension(
                        path,
                        "dimension segments require exact kind/index and one declared layer",
                    ));
                }
            }
            BoardOperation::CircleOperation(value) => {
                marker_count += 1;
                if value.index as usize != operation_index
                    || record.dimension_type != BoardDimensionType::Orthogonal
                    || marker_count > 1
                    || value.kind != "Circle"
                    || !dimension_layer_is_declared(value.layer.as_deref(), &record.layers)
                    || !dimension_marker_state(value)
                {
                    return Err(invalid_dimension(
                        path,
                        "only orthogonal dimensions admit the canonical declared-layer marker circle",
                    ));
                }
            }
            _ => {
                return Err(invalid_dimension(
                    path,
                    "dimension records admit only layered ThickSegment, marker Circle, and leading faced Text operations",
                ));
            }
        }
    }
    Ok(())
}

fn dimension_layer_is_declared(layer: Option<&str>, layers: &[String]) -> bool {
    layer.is_some_and(|layer| {
        layers
            .binary_search_by(|value| value.as_str().cmp(layer))
            .is_ok()
    })
}

fn dimension_marker_state(value: &CircleOperation) -> bool {
    value.fill == BoardFill::FilledShape
        && value.diameter_nm.get() == 200_000
        && value.width_nm.get() == 0
        && value.role.is_none()
        && value.layers.as_ref().is_none_or(Vec::is_empty)
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
        && value.stroke_color.is_none()
        && value.fill_color.is_none()
        && value.line_style.is_none()
}

fn invalid_dimension(path: String, message: &'static str) -> ValidationError {
    validation_error("invalid_board_operation", path, message)
}

fn validate_board_graphic_operations(
    operations: &[BoardOperation],
    record_index: usize,
) -> Result<(), ValidationError> {
    for (operation_index, operation) in operations.iter().enumerate() {
        let path = format!("$.records[{record_index}].operations[{operation_index}]");
        validate_board_operation(operation, path)?;
    }
    Ok(())
}

fn validate_track_segment_operations(
    operations: &[BoardOperation],
    record_index: usize,
) -> Result<(), ValidationError> {
    if let [BoardOperation::ThickSegmentOperation(value)] = operations
        && board_segment_is_layer_free(value)
    {
        return Ok(());
    }
    Err(validation_error(
        "invalid_board_operation",
        format!("$.records[{record_index}].operations"),
        "track segment records carry exactly one layerless thick segment",
    ))
}

fn validate_track_arc_operations(
    operations: &[BoardOperation],
    record_index: usize,
) -> Result<(), ValidationError> {
    if let [BoardOperation::ArcThreePointOperation(value)] = operations
        && value.layer.is_none()
    {
        return Ok(());
    }
    Err(validation_error(
        "invalid_board_operation",
        format!("$.records[{record_index}].operations"),
        "track arc records carry exactly one layerless three-point arc",
    ))
}

fn validate_zone_fill_operations(
    record: &crate::generated::board_plot_document::ZoneFillPlotRecord,
    record_index: usize,
) -> Result<(), ValidationError> {
    // One filled ring per operation: the fill_layers/fill_island arrays
    // annotate the rings positionally in the established serializer.
    let rings = record.operations.len();
    if record.fill_layers.len() != rings || record.fill_island.len() != rings {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].fill_layers"),
            "zone fill ring annotations must match the operation count",
        ));
    }
    for (operation_index, operation) in record.operations.iter().enumerate() {
        let valid = matches!(
            operation,
            BoardOperation::PlotPolyOperation(value) if value.layer.is_none()
        );
        if !valid {
            return Err(validation_error(
                "invalid_board_operation",
                format!("$.records[{record_index}].operations[{operation_index}]"),
                "zone fill records carry only layerless filled polygons",
            ));
        }
    }
    Ok(())
}

fn validate_board_text_operations(
    record: &BoardTextPlotRecord,
    record_index: usize,
) -> Result<(), ValidationError> {
    if record.kind != "gr_text" || record.object_id != "gr_text" {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}]"),
            "board text record identity must be gr_text",
        ));
    }
    // The established serializer never suppresses board gr_text via hide.
    if record.hide {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].hide"),
            "board gr_text records always carry hide false",
        ));
    }
    match record.operations.as_slice() {
        // Empty resolved text emits a zero-operation record.
        [] if record.text.is_empty() => Ok(()),
        [BoardOperation::TextOperation(value)] => {
            let path = format!("$.records[{record_index}].operations[0]");
            validate_text_payload(value, &path)?;
            // gr_text is single-line and pairs polyline_per_segment with
            // text_as_polygons for stroke-font payloads.
            let valid = value.layer.is_none()
                && !value.multiline
                && value.polyline_per_segment.is_some() == value.font_face.is_empty()
                && record.text == value.text;
            if valid {
                Ok(())
            } else {
                Err(validation_error("invalid_board_operation", path, {
                    "gr_text records carry one matching single-line text operation"
                }))
            }
        }
        _ => Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].operations"),
            "gr_text records carry at most one text operation",
        )),
    }
}

fn validate_board_text_box_operations(
    record: &BoardTextBoxPlotRecord,
    record_index: usize,
) -> Result<(), ValidationError> {
    if record.kind != "gr_text_box" || record.object_id != "gr_text_box" {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}]"),
            "board text-box record identity must be gr_text_box",
        ));
    }
    let (rect, text) = text_box_operation_parts(record, record_index)?;
    validate_text_box_border(record, record_index, rect)?;
    if let Some(value) = text {
        validate_text_box_text(value, record_index, rect.is_some())?;
        if record.text != value.text {
            return Err(validation_error(
                "invalid_board_operation",
                format!("$.records[{record_index}].text"),
                "gr_text_box record text must match its text operation",
            ));
        }
    } else if !record.text.is_empty() {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].text"),
            "gr_text_box records without text operations carry empty text",
        ));
    }
    Ok(())
}

fn text_box_operation_parts(
    record: &BoardTextBoxPlotRecord,
    record_index: usize,
) -> Result<(Option<&RectOperation>, Option<&TextOperation>), ValidationError> {
    let parts = match record.operations.as_slice() {
        [] => (None, None),
        [BoardOperation::TextOperation(value)] => (None, Some(value)),
        [BoardOperation::RectOperation(rect)] => (Some(rect), None),
        [
            BoardOperation::RectOperation(rect),
            BoardOperation::TextOperation(value),
        ] => (Some(rect), Some(value)),
        _ => {
            return Err(validation_error(
                "invalid_board_operation",
                format!("$.records[{record_index}].operations"),
                "gr_text_box records carry an optional border rect then text",
            ));
        }
    };
    Ok(parts)
}

fn validate_text_box_border(
    record: &BoardTextBoxPlotRecord,
    record_index: usize,
    rect: Option<&RectOperation>,
) -> Result<(), ValidationError> {
    // The border flag and the leading rect operation travel together.
    if record.border != rect.is_some() {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].border"),
            "gr_text_box border must match the leading rect operation",
        ));
    }
    if let Some(rect) = rect
        && !(rect.layer.is_none()
            && rect.fill == BoardFill::NoFill
            && text_box_rect_is_square(rect))
    {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].operations[0]"),
            "gr_text_box borders are layerless unfilled square-corner rects",
        ));
    }
    Ok(())
}

fn validate_text_box_text(
    value: &TextOperation,
    record_index: usize,
    has_border: bool,
) -> Result<(), ValidationError> {
    let operation_index = usize::from(has_border);
    let path = format!("$.records[{record_index}].operations[{operation_index}]");
    validate_text_payload(value, &path)?;
    // Text boxes never emit the mirror or polyline markers.
    if value.layer.is_some() || value.mirror.is_some() || value.polyline_per_segment.is_some() {
        return Err(validation_error(
            "invalid_board_operation",
            path,
            "gr_text_box text operations omit mirror and polyline markers",
        ));
    }
    Ok(())
}

fn validate_table_operations(
    record: &TablePlotRecord,
    record_index: usize,
) -> Result<(), ValidationError> {
    if !table_identity_and_layers_are_valid(record) {
        return Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}]"),
            "table identity must be canonical and layers must be nonempty, sorted, and unique",
        ));
    }
    let mut saw_text = false;
    let mut grid_layer: Option<String> = None;
    for (operation_index, operation) in record.operations.iter().enumerate() {
        let path = format!("$.records[{record_index}].operations[{operation_index}]");
        match operation {
            BoardOperation::ThickSegmentOperation(value) => {
                if saw_text {
                    return Err(invalid_table_phase(path));
                }
                validate_table_segment(value, record, &mut grid_layer, path)?;
            }
            BoardOperation::TextOperation(value) => {
                saw_text = true;
                validate_table_text(value, record, path)?;
            }
            _ => return Err(invalid_table_phase(path)),
        }
    }
    Ok(())
}

fn table_identity_and_layers_are_valid(record: &TablePlotRecord) -> bool {
    record.kind == "table"
        && record.object_id == "table"
        && !record.layers.is_empty()
        && record.layers.windows(2).all(|pair| pair[0] < pair[1])
}

fn validate_table_segment(
    value: &ThickSegmentOperation,
    record: &TablePlotRecord,
    grid_layer: &mut Option<String>,
    path: String,
) -> Result<(), ValidationError> {
    let Some(layer) = value.layer.as_deref() else {
        return Err(invalid_table_segment(path));
    };
    if !table_segment_has_graphic_state(value)
        || record
            .layers
            .binary_search_by(|candidate| candidate.as_str().cmp(layer))
            .is_err()
        || grid_layer.as_deref().is_some_and(|grid| grid != layer)
    {
        return Err(invalid_table_segment(path));
    }
    if grid_layer.is_none() {
        *grid_layer = Some(layer.to_owned());
    }
    Ok(())
}

fn validate_table_text(
    value: &TextOperation,
    record: &TablePlotRecord,
    path: String,
) -> Result<(), ValidationError> {
    let layer_is_declared = value
        .layer
        .as_ref()
        .is_some_and(|layer| record.layers.binary_search(layer).is_ok());
    if !layer_is_declared {
        return Err(invalid_table_text(path));
    }
    validate_text_payload_without_cache_angle(value, &path)?;
    if !table_text_has_required_state(value) {
        return Err(invalid_table_text(path));
    }
    Ok(())
}

fn table_text_has_required_state(value: &TextOperation) -> bool {
    let cache_state = match value.render_cache.as_ref() {
        Some(_) => value.render_cache_exact == Some(false),
        None => value.text.is_empty(),
    };
    !value.font_face.is_empty()
        && cache_state
        && value.mirror.is_none()
        && value.polyline_per_segment.is_none()
}

fn invalid_table_segment(path: String) -> ValidationError {
    validation_error(
        "invalid_board_operation",
        path,
        "table grid segments share one declared graphic-state layer",
    )
}

fn invalid_table_text(path: String) -> ValidationError {
    validation_error(
        "invalid_board_operation",
        path,
        "table text requires a faced cached payload on a declared layer",
    )
}

fn invalid_table_phase(path: String) -> ValidationError {
    validation_error(
        "invalid_board_operation",
        path,
        "table records carry grid segments followed by faced cached text",
    )
}

fn table_segment_has_graphic_state(value: &ThickSegmentOperation) -> bool {
    value.kind == "ThickSegment"
        && value.role.is_none()
        && value.layers.is_empty()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
        && value.stroke_color.is_none()
}

fn text_box_rect_is_square(rect: &RectOperation) -> bool {
    i64::from(rect.corner_radius_nm) == 0
}

/// Marker-key and render-cache states shared by both board text records.
fn validate_text_payload(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    validate_text_payload_with_cache_angle(value, path, true)
}

fn validate_text_payload_without_cache_angle(
    value: &TextOperation,
    path: &str,
) -> Result<(), ValidationError> {
    validate_text_payload_with_cache_angle(value, path, false)
}

fn validate_text_payload_with_cache_angle(
    value: &TextOperation,
    path: &str,
    require_cache_angle: bool,
) -> Result<(), ValidationError> {
    if value.kind != "Text" {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "board text operation kind must be Text",
        ));
    }
    if value.context.is_some() {
        return Err(validation_error(
            "invalid_board_operation",
            format!("{path}.context"),
            "board text does not emit operation context",
        ));
    }
    validate_text_markers(value, path)?;
    validate_text_cache(value, path, require_cache_angle)
}

fn validate_text_markers(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    let markers_true = [
        value.mirror,
        value.text_as_polygons,
        value.polyline_per_segment,
        value.knockout,
    ]
    .iter()
    .all(|marker| *marker != Some(false));
    if !markers_true {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "text marker keys are present only when true",
        ));
    }
    // Stroke-font payloads flag text_as_polygons; font faces suppress it.
    if value.text_as_polygons.is_some() != value.font_face.is_empty() {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "text_as_polygons appears exactly when no font face is set",
        ));
    }
    Ok(())
}

fn validate_text_cache(
    value: &TextOperation,
    path: &str,
    require_cache_angle: bool,
) -> Result<(), ValidationError> {
    validate_text_cache_keys(value, path)?;
    if let Some(cache) = &value.render_cache {
        validate_text_cache_payload(value, cache, path, require_cache_angle)?;
    }
    if value.render_cache.is_none() && value.knockout.is_some() {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "knockout text operations require a restructured render cache",
        ));
    }
    Ok(())
}

fn validate_text_cache_keys(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    let has_cache = value.render_cache.is_some();
    let coherent = has_cache == value.render_cache_source.is_some()
        && has_cache == value.render_cache_exact.is_some()
        && has_cache != value.render_cache_polygons.is_empty();
    if !coherent {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "render-cache keys must appear together",
        ));
    }
    Ok(())
}

fn validate_text_cache_payload(
    value: &TextOperation,
    cache: &crate::generated::board_plot_document::TextRenderCache,
    path: &str,
    require_cache_angle: bool,
) -> Result<(), ValidationError> {
    if !cache_identities_match(value, cache)
        || !cache_payload_matches(value, cache, require_cache_angle)
    {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "render-cache payload must agree with text, angle, markers, polygons, and identities",
        ));
    }
    Ok(())
}

fn cache_identities_match(
    value: &TextOperation,
    cache: &crate::generated::board_plot_document::TextRenderCache,
) -> bool {
    let source_matches = value.render_cache_source == Some(cache.source);
    cache.schema == "kicad.render_cache.v1"
        && cache.unit == "nm"
        && cache.coordinate_space == PlotterTextRenderCacheCoordinateSpace::Board
        && source_matches
}

fn cache_payload_matches(
    value: &TextOperation,
    cache: &crate::generated::board_plot_document::TextRenderCache,
    require_cache_angle: bool,
) -> bool {
    let knockout = value.knockout == cache.knockout && !matches!(cache.knockout, Some(false));
    value.render_cache_exact == Some(cache.exact)
        && cache.text == value.text
        && (!require_cache_angle || cache.angle == value.orient_deg)
        && knockout
        && cache_polygons_match(value, cache)
}

fn cache_polygons_match(
    value: &TextOperation,
    cache: &crate::generated::board_plot_document::TextRenderCache,
) -> bool {
    cache.polygons.len() == value.render_cache_polygons.len()
        && cache
            .polygons
            .iter()
            .zip(&value.render_cache_polygons)
            .all(|(polygon, exterior)| {
                !polygon.contours.is_empty()
                    && polygon.contours.iter().all(|contour| contour.len() >= 3)
                    && points_equal(&polygon.contours[0], exterior)
            })
}

fn points_equal(
    left: &[crate::generated::board_plot_document::PlotterPoint],
    right: &[crate::generated::board_plot_document::PlotterPoint],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.0[0].get() == right.0[0].get() && left.0[1].get() == right.0[1].get()
        })
}

fn validate_via_operations(
    operations: &[BoardOperation],
    record_index: usize,
) -> Result<(), ValidationError> {
    let mut index = 0;
    if operations.first().is_some_and(|operation| {
        matches!(operation, BoardOperation::FlashPadCircleOperation(value)
            if via_flash_has_role(value, PlotterViaFlashRole::ViaAperture))
    }) {
        index += 1;
    }
    let mut valid = operations.get(index).is_some_and(|operation| {
        matches!(operation, BoardOperation::CircleOperation(value)
            if via_drill_has_role(value, BoardDrillRole::ViaDrill))
    });
    index += usize::from(valid);
    while valid && index < operations.len() {
        valid = operations.get(index).is_some_and(|operation| {
            matches!(operation, BoardOperation::FlashPadCircleOperation(value)
                if via_flash_has_role(value, PlotterViaFlashRole::ViaMaskOpening))
        }) && operations.get(index + 1).is_some_and(|operation| {
            matches!(operation, BoardOperation::CircleOperation(value)
                if via_drill_has_role(value, BoardDrillRole::ViaMaskDrill))
        });
        index += 2;
    }
    if valid {
        Ok(())
    } else {
        Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].operations"),
            "via records carry an optional copper aperture, a drill, then mask opening/drill pairs",
        ))
    }
}

// A via aperture carries an already-resolved effective flash-layer set and may
// be absent when every annulus is removed. The mandatory drill retains the
// authored physical span; only pad-only operation states are rejected here.
fn via_flash_has_role(value: &FlashPadCircleOperation, role: PlotterViaFlashRole) -> bool {
    value.role == Some(role) && value.mask_margin_nm.is_none()
}

fn via_drill_has_role(value: &CircleOperation, role: BoardDrillRole) -> bool {
    value.role == Some(role)
        && value.layer.is_none()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
}

fn validate_board_operation(
    operation: &BoardOperation,
    path: String,
) -> Result<(), ValidationError> {
    let valid = match operation {
        BoardOperation::ThickSegmentOperation(value) => board_segment_is_layer_free(value),
        BoardOperation::CircleOperation(value) => board_circle_is_layer_free(value),
        BoardOperation::ArcThreePointOperation(value) => value.layer.is_none(),
        BoardOperation::RectOperation(value) => value.layer.is_none(),
        BoardOperation::PlotPolyOperation(value) => value.layer.is_none(),
        BoardOperation::BezierCurveOperation(value) => value.layer.is_none(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(validation_error(
            "invalid_board_operation",
            path,
            "board graphic records accept only layerless graphic-state geometry",
        ))
    }
}

fn board_segment_is_layer_free(value: &ThickSegmentOperation) -> bool {
    value.layer.is_none()
        && value.role.is_none()
        && value.layers.is_empty()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
        && value.stroke_color.is_none()
}

fn board_circle_is_layer_free(value: &CircleOperation) -> bool {
    value.layer.is_none()
        && value.role.is_none()
        && value.layers.is_none()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
}

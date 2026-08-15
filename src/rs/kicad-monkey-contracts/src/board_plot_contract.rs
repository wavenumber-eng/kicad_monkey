//! Record-state validation for the board plotter IR document contract.

use crate::generated::board_plot_document::{
    BoardPlotDocumentA0, BoardPlotRecord, CircleOperation, FlashPadCircleOperation,
    PlotterDrillRole as BoardDrillRole, PlotterOperation as BoardOperation, PlotterViaFlashRole,
    ThickSegmentOperation,
};
use crate::{ValidationError, validation_error};

/// Enforce record counts and record-kind-specific operation states for boards.
pub fn validate_board_plot_document(document: &BoardPlotDocumentA0) -> Result<(), ValidationError> {
    let mut total_operations = 0usize;
    for (record_index, record) in document.records.iter().enumerate() {
        let (declared, operations) = match record {
            BoardPlotRecord::BoardGraphicPlotRecord(record) => {
                validate_board_graphic_operations(&record.operations, record_index)?;
                (record.operation_count, &record.operations)
            }
            BoardPlotRecord::TrackSegmentPlotRecord(record) => {
                validate_track_segment_operations(&record.operations, record_index)?;
                (record.operation_count, &record.operations)
            }
            BoardPlotRecord::TrackArcPlotRecord(record) => {
                validate_track_arc_operations(&record.operations, record_index)?;
                (record.operation_count, &record.operations)
            }
            BoardPlotRecord::ViaPlotRecord(record) => {
                validate_via_operations(&record.operations, record_index)?;
                (record.operation_count, &record.operations)
            }
        };
        if declared as usize != operations.len() {
            return Err(validation_error(
                "operation_count_mismatch",
                format!("$.records[{record_index}].operation_count"),
                "operation_count must equal the operation array length",
            ));
        }
        total_operations = total_operations.saturating_add(operations.len());
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

fn validate_via_operations(
    operations: &[BoardOperation],
    record_index: usize,
) -> Result<(), ValidationError> {
    let mut valid = operations.len() >= 2 && operations.len().is_multiple_of(2);
    if valid {
        for (index, operation) in operations.iter().enumerate() {
            valid = match (index, operation) {
                (0, BoardOperation::FlashPadCircleOperation(value)) => {
                    via_flash_has_role(value, PlotterViaFlashRole::ViaAperture)
                }
                (1, BoardOperation::CircleOperation(value)) => {
                    via_drill_has_role(value, BoardDrillRole::ViaDrill)
                }
                (index, BoardOperation::FlashPadCircleOperation(value))
                    if index.is_multiple_of(2) =>
                {
                    via_flash_has_role(value, PlotterViaFlashRole::ViaMaskOpening)
                }
                (index, BoardOperation::CircleOperation(value)) if !index.is_multiple_of(2) => {
                    via_drill_has_role(value, BoardDrillRole::ViaMaskDrill)
                }
                _ => false,
            };
            if !valid {
                break;
            }
        }
    }
    if valid {
        Ok(())
    } else {
        Err(validation_error(
            "invalid_board_operation",
            format!("$.records[{record_index}].operations"),
            "via records carry an aperture/drill pair then mask opening/drill pairs",
        ))
    }
}

// Via operations carry the via's layer list verbatim, which the established
// Python serializer leaves empty for unrouted vias; only the pad-only states
// are rejected here.
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
}

fn board_circle_is_layer_free(value: &CircleOperation) -> bool {
    value.layer.is_none()
        && value.role.is_none()
        && value.layers.is_none()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
}

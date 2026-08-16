//! Record-state validation for the board plotter IR document contract.

use crate::generated::board_plot_document::{
    BoardPlotDocumentA0, BoardPlotRecord, BoardTextBoxPlotRecord, BoardTextPlotRecord,
    CircleOperation, FlashPadCircleOperation, PlotterDrillRole as BoardDrillRole,
    PlotterFill as BoardFill, PlotterOperation as BoardOperation, PlotterViaFlashRole,
    RectOperation, TextOperation, ThickSegmentOperation,
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
            BoardPlotRecord::ZoneFillPlotRecord(record) => {
                validate_zone_fill_operations(record, record_index)?;
                (record.operation_count, &record.operations)
            }
            BoardPlotRecord::BoardTextPlotRecord(record) => {
                validate_board_text_operations(record, record_index)?;
                (record.operation_count, &record.operations)
            }
            BoardPlotRecord::BoardTextBoxPlotRecord(record) => {
                validate_board_text_box_operations(record, record_index)?;
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
        [] => Ok(()),
        [BoardOperation::TextOperation(value)] => {
            let path = format!("$.records[{record_index}].operations[0]");
            validate_text_payload(value, &path)?;
            // gr_text is single-line and pairs polyline_per_segment with
            // text_as_polygons for stroke-font payloads.
            let valid = !value.multiline
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
    let (rect, text) = match record.operations.as_slice() {
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
    if let Some(value) = text {
        let operation_index = usize::from(rect.is_some());
        let path = format!("$.records[{record_index}].operations[{operation_index}]");
        validate_text_payload(value, &path)?;
        // Text boxes never emit the mirror or polyline markers.
        if value.mirror.is_some() || value.polyline_per_segment.is_some() {
            return Err(validation_error(
                "invalid_board_operation",
                path,
                "gr_text_box text operations omit mirror and polyline markers",
            ));
        }
    }
    Ok(())
}

fn text_box_rect_is_square(rect: &RectOperation) -> bool {
    i64::from(rect.corner_radius_nm) == 0
}

/// Marker-key and render-cache states shared by both board text records.
fn validate_text_payload(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
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
    match &value.render_cache {
        Some(cache) => {
            if value.render_cache_exact != Some(cache.exact)
                || (value.knockout.is_some() && cache.knockout != Some(true))
            {
                return Err(validation_error(
                    "invalid_board_operation",
                    path.to_owned(),
                    "render-cache payload must agree with the operation markers",
                ));
            }
        }
        None => {
            if value.knockout.is_some() {
                return Err(validation_error(
                    "invalid_board_operation",
                    path.to_owned(),
                    "knockout text operations require a restructured render cache",
                ));
            }
        }
    }
    Ok(())
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

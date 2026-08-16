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
    for (record_index, record) in document.records.iter().enumerate() {
        let (declared, operations) = validate_board_record(record, record_index)?;
        if declared != operations.len() {
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

fn validate_board_record(
    record: &BoardPlotRecord,
    record_index: usize,
) -> Result<(usize, &[BoardOperation]), ValidationError> {
    let (declared, operations) = match record {
        BoardPlotRecord::BoardGraphicPlotRecord(value) => {
            validate_board_graphic_operations(&value.operations, record_index)?;
            (value.operation_count, &value.operations)
        }
        BoardPlotRecord::TrackSegmentPlotRecord(value) => {
            validate_track_segment_operations(&value.operations, record_index)?;
            (value.operation_count, &value.operations)
        }
        BoardPlotRecord::TrackArcPlotRecord(value) => {
            validate_track_arc_operations(&value.operations, record_index)?;
            (value.operation_count, &value.operations)
        }
        BoardPlotRecord::ViaPlotRecord(value) => {
            validate_via_operations(&value.operations, record_index)?;
            (value.operation_count, &value.operations)
        }
        BoardPlotRecord::ZoneFillPlotRecord(value) => {
            validate_zone_fill_operations(value, record_index)?;
            (value.operation_count, &value.operations)
        }
        BoardPlotRecord::BoardTextPlotRecord(value) => {
            validate_board_text_operations(value, record_index)?;
            (value.operation_count, &value.operations)
        }
        BoardPlotRecord::BoardTextBoxPlotRecord(value) => {
            validate_board_text_box_operations(value, record_index)?;
            (value.operation_count, &value.operations)
        }
    };
    Ok((declared as usize, operations))
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
    if value.mirror.is_some() || value.polyline_per_segment.is_some() {
        return Err(validation_error(
            "invalid_board_operation",
            path,
            "gr_text_box text operations omit mirror and polyline markers",
        ));
    }
    Ok(())
}

fn text_box_rect_is_square(rect: &RectOperation) -> bool {
    i64::from(rect.corner_radius_nm) == 0
}

/// Marker-key and render-cache states shared by both board text records.
fn validate_text_payload(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    if value.kind != "Text" {
        return Err(validation_error(
            "invalid_board_operation",
            path.to_owned(),
            "board text operation kind must be Text",
        ));
    }
    validate_text_markers(value, path)?;
    validate_text_cache(value, path)
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

fn validate_text_cache(value: &TextOperation, path: &str) -> Result<(), ValidationError> {
    validate_text_cache_keys(value, path)?;
    if let Some(cache) = &value.render_cache {
        validate_text_cache_payload(value, cache, path)?;
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
) -> Result<(), ValidationError> {
    if !cache_identities_match(value, cache) || !cache_payload_matches(value, cache) {
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
    cache.schema == "kicad.render_cache.v1"
        && cache.unit == "nm"
        && cache.coordinate_space == "board"
        && cache.source == "existing_file_cache"
        && value.render_cache_source.as_deref() == Some("existing_file_cache")
}

fn cache_payload_matches(
    value: &TextOperation,
    cache: &crate::generated::board_plot_document::TextRenderCache,
) -> bool {
    let knockout = value.knockout == cache.knockout && !matches!(cache.knockout, Some(false));
    value.render_cache_exact == Some(cache.exact)
        && cache.text == value.text
        && cache.angle == value.orient_deg
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

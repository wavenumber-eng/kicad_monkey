//! Semantic validation for the strict schematic-foundation plot contract.

use crate::generated::schematic_plot_document::{
    CircleOperation, PlotImageOperation, PlotPolyOperation, PlotterFill, PlotterOperation,
    RectOperation, SchematicJunctionPlotRecord, SchematicNoConnectPlotRecord,
    SchematicPlotDocumentA0, SchematicPlotRecord, SchematicSheetHeaderPlotRecord, TextOperation,
};
use crate::{ValidationError, validation_error};

const BACKGROUND_COLOR: &str = "#F5F4EFFF";
const DRAWING_SHEET_COLOR: &str = "#840000FF";
const DEFAULT_JUNCTION_COLOR: &str = "#009600FF";
const NO_CONNECT_COLOR: &str = "#000084FF";

/// Enforce document identity, record order, operation shape, and local indexes.
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
        let (uuid, kind, expected_kind, object_id, declared, operations) = record_fields(record);
        if kind != expected_kind || uuid != object_id {
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
    }
}

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

fn error(code: &'static str, path: impl Into<String>, message: &'static str) -> ValidationError {
    validation_error(code, path, message)
}

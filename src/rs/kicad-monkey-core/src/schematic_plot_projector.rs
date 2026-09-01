//! Direct typed projection of schematic Plotter IR into the frozen a0 contract.

use std::collections::BTreeMap;

use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::schematic_plot_document as c;
use kicad_monkey_contracts::validate_schematic_plot_document;

use crate::plot_document_contract::ProjectionUsage;
use crate::plotter_contract::contract_schematic_plotter_operation;
use crate::{
    PlotDocumentProjectionLimits, PlotProjectionError, PlotProjectionErrorKind,
    SchematicAnnotationRecord, SchematicAnnotationRecordKind, SchematicConnectivityRecord,
    SchematicConnectivityRecordKind, SchematicGraphicRecord, SchematicGraphicRecordKind,
    SchematicPlotDocument, SchematicPlotOperation, SchematicPlotRecord,
};

pub fn project_schematic_plot_document_a0(
    document: &SchematicPlotDocument,
    limits: PlotDocumentProjectionLimits,
) -> Result<c::SchematicPlotDocumentA0, PlotProjectionError> {
    projection_usage(document)?.enforce(limits)?;
    let records = document
        .records
        .iter()
        .map(contract_record)
        .collect::<Result<Vec<_>, _>>()?;
    let total_operations = document.records.iter().try_fold(0usize, |total, record| {
        total.checked_add(record.operation_count()).ok_or_else(|| {
            PlotProjectionError::new(
                PlotProjectionErrorKind::NumericRange,
                "schematic operation count overflowed",
            )
        })
    })?;
    let contract = c::SchematicPlotDocumentA0 {
        canvas: c::SchematicPlotCanvas {
            height_nm: safe_integer(document.canvas.height_nm)?,
            width_nm: safe_integer(document.canvas.width_nm)?,
        },
        coordinate_space: c::PlotterCoordinateSpace {
            unit: "nm".to_owned(),
            y_axis: "down".to_owned(),
        },
        document_id: document.document_id.clone(),
        records,
        schema: "kicad.plotter_ir.a0".to_owned(),
        source_kind: "SCH".to_owned(),
        source_path: document.source_path.clone(),
        total_operations: count(total_operations)?,
    };
    validate_schematic_plot_document(&contract).map_err(|error| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::ContractValidation,
            format!("schematic plot contract validation failed: {error}"),
        )
    })?;
    Ok(contract)
}

fn projection_usage(
    document: &SchematicPlotDocument,
) -> Result<ProjectionUsage, PlotProjectionError> {
    let records = document.records.len();
    let mut operations = 0usize;
    let mut points = 0usize;
    let mut strings = checked_lengths([
        document.document_id.len(),
        document.source_path.as_ref().map_or(0, String::len),
    ])?;
    let mut nested_items = records;
    for record in &document.records {
        strings = strings
            .checked_add(record_string_bytes(record)?)
            .ok_or_else(resource_overflow)?;
        let record_operations = operations_for(record);
        operations = operations
            .checked_add(record_operations.len())
            .ok_or_else(resource_overflow)?;
        nested_items = nested_items
            .checked_add(record_operations.len())
            .ok_or_else(resource_overflow)?;
        nested_items = nested_items
            .checked_add(record_nested_items(record))
            .ok_or_else(resource_overflow)?;
        for operation in record_operations {
            let (operation_points, operation_strings, operation_nested) =
                operation_usage(operation)?;
            points = points
                .checked_add(operation_points)
                .ok_or_else(resource_overflow)?;
            strings = strings
                .checked_add(operation_strings)
                .ok_or_else(resource_overflow)?;
            nested_items = nested_items
                .checked_add(operation_nested)
                .ok_or_else(resource_overflow)?;
        }
    }
    Ok(ProjectionUsage {
        records,
        operations,
        points,
        string_bytes: strings,
        nested_items,
    })
}

fn operations_for(record: &SchematicPlotRecord) -> &[SchematicPlotOperation] {
    match record {
        SchematicPlotRecord::SheetHeader(value) => &value.operations,
        SchematicPlotRecord::Connectivity(value) => &value.operations,
        SchematicPlotRecord::Annotation(value) => &value.operations,
        SchematicPlotRecord::Graphic(value) => &value.operations,
        SchematicPlotRecord::RuleArea(value) => &value.operations,
        SchematicPlotRecord::Image(value) => &value.operations,
        SchematicPlotRecord::Table(value) => &value.operations,
        SchematicPlotRecord::SymbolInstance(value) => &value.operations,
        SchematicPlotRecord::SymbolOverplot(value) => &value.operations,
        SchematicPlotRecord::Sheet(value) => &value.operations,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive operation resource inventory"
)]
fn operation_usage(
    operation: &SchematicPlotOperation,
) -> Result<(usize, usize, usize), PlotProjectionError> {
    use crate::PlotterOperation as P;
    let (points, strings, nested) = match operation {
        SchematicPlotOperation::Plotter(value) => match value {
            P::ThickSegment(value) => (
                2,
                checked_lengths([
                    optional_len(value.layer.as_ref()),
                    optional_len(value.role.as_ref()),
                    strings_len(&value.layers)?,
                ])?,
                value.layers.len(),
            ),
            P::ArcThreePoint(value) => (
                3,
                checked_lengths([
                    optional_len(value.layer.as_ref()),
                    optional_len(value.stroke_color.as_ref()),
                    optional_len(value.fill_color.as_ref()),
                ])?,
                0,
            ),
            P::Circle(value) => (
                1,
                checked_lengths([
                    optional_len(value.layer.as_ref()),
                    optional_len(value.role.as_ref()),
                    optional_len(value.stroke_color.as_ref()),
                    optional_len(value.fill_color.as_ref()),
                    strings_len(&value.layers)?,
                ])?,
                value.layers.len(),
            ),
            P::Rect(value) => (
                2,
                checked_lengths([
                    optional_len(value.layer.as_ref()),
                    optional_len(value.stroke_color.as_ref()),
                    optional_len(value.fill_color.as_ref()),
                ])?,
                0,
            ),
            P::PlotPoly(value) => (
                value.points.len(),
                checked_lengths([
                    optional_len(value.layer.as_ref()),
                    optional_len(value.stroke_color.as_ref()),
                    optional_len(value.fill_color.as_ref()),
                ])?,
                value.points.len(),
            ),
            P::BezierCurve(value) => (
                4,
                checked_lengths([
                    optional_len(value.layer.as_ref()),
                    optional_len(value.stroke_color.as_ref()),
                ])?,
                0,
            ),
            P::Text(value) => (
                1,
                checked_lengths([
                    value.text.len(),
                    value.color.len(),
                    value.font_face.len(),
                    optional_len(value.layer.as_ref()),
                ])?,
                0,
            ),
            P::FlashPadCircle(value) => (1, strings_len(&value.layers)?, value.layers.len()),
            P::FlashPadOval(value) => (1, strings_len(&value.layers)?, value.layers.len()),
            P::FlashPadRect(value) => (1, strings_len(&value.layers)?, value.layers.len()),
            P::FlashPadRoundRect(value) => (1, strings_len(&value.layers)?, value.layers.len()),
            P::FlashPadCustom(value) => {
                let polygon_points = value.polygons.iter().try_fold(0usize, |sum, polygon| {
                    sum.checked_add(polygon.len()).ok_or_else(resource_overflow)
                })?;
                let nested = polygon_points
                    .checked_add(value.polygons.len())
                    .and_then(|sum| sum.checked_add(value.layers.len()))
                    .and_then(|sum| {
                        sum.checked_add(value.polygon_widths_nm.as_ref().map_or(0, Vec::len))
                    })
                    .ok_or_else(resource_overflow)?;
                (
                    polygon_points
                        .checked_add(1)
                        .ok_or_else(resource_overflow)?,
                    checked_lengths([
                        optional_len(value.anchor_shape.as_ref()),
                        strings_len(&value.layers)?,
                    ])?,
                    nested,
                )
            }
            P::FlashPadTrapez(value) => (4, strings_len(&value.layers)?, value.layers.len()),
        },
        SchematicPlotOperation::PlotImage(value) => (
            1,
            checked_lengths([
                value.image_data_b64.len(),
                value.image_format.len(),
                optional_len(value.stroke_color.as_ref()),
            ])?,
            0,
        ),
        SchematicPlotOperation::Text(value) => (
            1,
            checked_lengths([
                value.text.text.len(),
                value.text.color.len(),
                value.text.font_face.len(),
                optional_len(value.text.layer.as_ref()),
                optional_len(value.hyperlink_href.as_ref()),
            ])?,
            0,
        ),
        SchematicPlotOperation::StyledThickSegment(value) => {
            require_canonical_styled_segment(value)?;
            (2, value.stroke_color.len(), 0)
        }
        SchematicPlotOperation::StartSymbolPinBlock(value) => {
            let attrs = &value.extra_attrs;
            (
                0,
                checked_lengths([
                    value.label.len(),
                    value.data_uuid.len(),
                    value.object_id.len(),
                    attrs.primitive.len(),
                    attrs.object_type.len(),
                    attrs.pin.len(),
                    attrs.symbol_uuid.len(),
                    attrs.designator.len(),
                    attrs.lib_pin_uuid.len(),
                ])?,
                nonempty_count([
                    &attrs.primitive,
                    &attrs.object_type,
                    &attrs.pin,
                    &attrs.symbol_uuid,
                    &attrs.designator,
                    &attrs.lib_pin_uuid,
                ]),
            )
        }
        SchematicPlotOperation::StartSheetPinBlock(value) => {
            let attrs = &value.extra_attrs;
            (
                0,
                checked_lengths([
                    value.label.len(),
                    value.data_uuid.len(),
                    value.object_id.len(),
                    attrs.primitive.len(),
                    attrs.object_type.len(),
                    attrs.sheet_uuid.len(),
                    attrs.sheet_name.len(),
                    attrs.sheet_file.len(),
                    attrs.pin.len(),
                    attrs.pin_name.len(),
                    attrs.shape.len(),
                ])?,
                nonempty_count([
                    &attrs.primitive,
                    &attrs.object_type,
                    &attrs.sheet_uuid,
                    &attrs.sheet_name,
                    &attrs.sheet_file,
                    &attrs.pin,
                    &attrs.pin_name,
                    &attrs.shape,
                ]),
            )
        }
        SchematicPlotOperation::EndBlock => (0, 0, 0),
    };
    Ok((points, strings, nested))
}

fn record_string_bytes(record: &SchematicPlotRecord) -> Result<usize, PlotProjectionError> {
    let mut values: Vec<&str> = Vec::new();
    match record {
        SchematicPlotRecord::SheetHeader(value) => {
            values.extend([
                value.uuid.as_str(),
                value.paper_size.as_str(),
                value.generator.as_str(),
                value.generator_version.as_str(),
            ]);
            if let Some(title) = &value.title_block {
                values.extend([
                    title.title.as_str(),
                    title.date.as_str(),
                    title.revision.as_str(),
                    title.company.as_str(),
                ]);
                values.extend(title.comments.values().map(String::as_str));
            }
        }
        SchematicPlotRecord::Connectivity(value) => {
            values.push(&value.uuid);
            if let Some(color) = &value.junction_color {
                values.push(color);
            }
        }
        SchematicPlotRecord::Annotation(value) => {
            values.extend([value.uuid.as_str(), value.object_id.as_str()]);
            values.extend(value.text.iter().map(String::as_str));
            values.extend(value.shape.iter().map(String::as_str));
        }
        SchematicPlotRecord::Graphic(value) => values.push(&value.uuid),
        SchematicPlotRecord::RuleArea(value) => values.push(&value.uuid),
        SchematicPlotRecord::Image(value) => {
            values.extend([value.uuid.as_str(), value.image_format.as_str()]);
        }
        SchematicPlotRecord::Table(value) => values.push(&value.uuid),
        SchematicPlotRecord::SymbolInstance(value) => {
            values.extend([
                value.uuid.as_str(),
                value.lib_id.as_str(),
                value.lib_name.as_str(),
                value.reference.as_str(),
            ]);
            if let Some(mirror) = &value.mirror {
                values.push(mirror);
            }
        }
        SchematicPlotRecord::SymbolOverplot(value) => values.extend([
            value.uuid.as_str(),
            value.source_symbol_uuid.as_str(),
            value.lib_id.as_str(),
        ]),
        SchematicPlotRecord::Sheet(value) => values.extend([
            value.uuid.as_str(),
            value.sheet_name.as_str(),
            value.sheet_file.as_str(),
        ]),
    }
    let mut total = values.into_iter().try_fold(0usize, |total, value| {
        total.checked_add(value.len()).ok_or_else(resource_overflow)
    })?;
    if let SchematicPlotRecord::SheetHeader(value) = record
        && let Some(title) = &value.title_block
    {
        for key in title.comments.keys() {
            total = total
                .checked_add(key.to_string().len())
                .ok_or_else(resource_overflow)?;
        }
    }
    Ok(total)
}

fn record_nested_items(record: &SchematicPlotRecord) -> usize {
    match record {
        SchematicPlotRecord::SheetHeader(value) => value
            .title_block
            .as_ref()
            .map_or(0, |title| title.comments.len()),
        _ => 0,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive frozen record-union adapter"
)]
fn contract_record(
    record: &SchematicPlotRecord,
) -> Result<c::SchematicPlotRecord, PlotProjectionError> {
    Ok(match record {
        SchematicPlotRecord::SheetHeader(value) => c::SchematicSheetHeaderPlotRecord {
            generator: value.generator.clone(),
            generator_version: value.generator_version.clone(),
            kind: "sheet_header".to_owned(),
            object_id: value.uuid.clone(),
            operation_count: count(value.operations.len())?,
            operations: plotter_operations(&value.operations)?,
            paper_height_mm: value.paper_height_mm,
            paper_portrait: value.paper_portrait,
            paper_size: value.paper_size.clone(),
            paper_width_mm: value.paper_width_mm,
            sheet_height_nm: safe_integer(value.sheet_height_nm)?,
            sheet_width_nm: safe_integer(value.sheet_width_nm)?,
            title_block: value
                .title_block
                .as_ref()
                .map(|title| c::SchematicPlotTitleBlock {
                    comments: c::RecordString::from(
                        title
                            .comments
                            .iter()
                            .map(|(key, value)| (key.to_string(), value.clone()))
                            .collect::<BTreeMap<_, _>>(),
                    ),
                    company: title.company.clone(),
                    date: title.date.clone(),
                    rev: title.revision.clone(),
                    title: title.title.clone(),
                }),
            uuid: value.uuid.clone(),
            version: safe_integer(value.version)?,
        }
        .into(),
        SchematicPlotRecord::Connectivity(value) => contract_connectivity_record(value)?,
        SchematicPlotRecord::Annotation(value) => contract_annotation_record(value)?,
        SchematicPlotRecord::Graphic(value) => contract_graphic_record(value)?,
        SchematicPlotRecord::RuleArea(value) => c::SchematicRuleAreaPlotRecord {
            dnp: value.dnp,
            exclude_from_sim: value.exclude_from_sim,
            in_bom: value.in_bom,
            kind: "rule_area".to_owned(),
            locked: value.locked,
            object_id: value.uuid.clone(),
            on_board: value.on_board,
            operation_count: count(value.operations.len())?,
            operations: plotter_operations(&value.operations)?,
            shape: enum_ref(value.shape.as_str())?,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicPlotRecord::Image(value) => c::SchematicImagePlotRecord {
            height_nm: safe_integer(value.height_nm)?,
            image_format: enum_ref(&value.image_format)?,
            kind: "image".to_owned(),
            object_id: value.uuid.clone(),
            operation_count: count(value.operations.len())?,
            operations: plotter_operations(&value.operations)?,
            scale: value.scale,
            uuid: value.uuid.clone(),
            width_nm: safe_integer(value.width_nm)?,
        }
        .into(),
        SchematicPlotRecord::Table(value) => c::SchematicTablePlotRecord {
            cell_count: count(value.cell_count)?,
            kind: "table".to_owned(),
            object_id: value.uuid.clone(),
            operation_count: count(value.operations.len())?,
            operations: plotter_operations(&value.operations)?,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicPlotRecord::SymbolInstance(value) => c::SchematicSymbolInstancePlotRecord {
            at_angle_deg: value.at_angle_deg,
            at_x_nm: safe_integer(value.at_x_nm)?,
            at_y_nm: safe_integer(value.at_y_nm)?,
            convert: value.convert,
            dnp: value.dnp,
            exclude_from_sim: value.exclude_from_sim,
            in_bom: value.in_bom,
            in_pos_files: value.in_pos_files,
            kind: "symbol_instance".to_owned(),
            lib_id: value.lib_id.clone(),
            lib_name: value.lib_name.clone(),
            mirror: value.mirror.clone(),
            object_id: if value.lib_id.is_empty() {
                value.uuid.clone()
            } else {
                value.lib_id.clone()
            },
            on_board: value.on_board,
            operation_count: count(value.operations.len())?,
            operations: symbol_operations(&value.operations)?,
            reference: value.reference.clone(),
            unit: value.unit,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicPlotRecord::SymbolOverplot(value) => c::SchematicSymbolOverplotPlotRecord {
            kind: "symbol_overplot".to_owned(),
            lib_id: value.lib_id.clone(),
            object_id: if value.lib_id.is_empty() {
                value.source_symbol_uuid.clone()
            } else {
                value.lib_id.clone()
            },
            operation_count: count(value.operations.len())?,
            operations: symbol_operations(&value.operations)?,
            source_symbol_uuid: value.source_symbol_uuid.clone(),
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicPlotRecord::Sheet(value) => c::SchematicSheetPlotRecord {
            at_x_nm: safe_integer(value.at_x_nm)?,
            at_y_nm: safe_integer(value.at_y_nm)?,
            dnp: value.dnp,
            kind: "sheet".to_owned(),
            object_id: value.sheet_name.clone(),
            operation_count: count(value.operations.len())?,
            operations: sheet_operations(&value.operations)?,
            sheet_file: value.sheet_file.clone(),
            sheet_name: value.sheet_name.clone(),
            size_x_nm: safe_integer(value.size_x_nm)?,
            size_y_nm: safe_integer(value.size_y_nm)?,
            uuid: value.uuid.clone(),
        }
        .into(),
    })
}

macro_rules! simple_record {
    ($target:ident, $value:ident, $kind:expr, $operations:expr) => {
        c::$target {
            kind: $kind.to_owned(),
            object_id: $value.uuid.clone(),
            operation_count: count($value.operations.len())?,
            operations: $operations,
            uuid: $value.uuid.clone(),
        }
        .into()
    };
}

fn contract_connectivity_record(
    value: &SchematicConnectivityRecord,
) -> Result<c::SchematicPlotRecord, PlotProjectionError> {
    let operations = plotter_operations(&value.operations)?;
    Ok(match value.kind {
        SchematicConnectivityRecordKind::Wire => {
            simple_record!(SchematicWirePlotRecord, value, "wire", operations)
        }
        SchematicConnectivityRecordKind::Bus => {
            simple_record!(SchematicBusPlotRecord, value, "bus", operations)
        }
        SchematicConnectivityRecordKind::BusEntry => {
            simple_record!(SchematicBusEntryPlotRecord, value, "bus_entry", operations)
        }
        SchematicConnectivityRecordKind::NoConnect => simple_record!(
            SchematicNoConnectPlotRecord,
            value,
            "no_connect",
            operations
        ),
        SchematicConnectivityRecordKind::Junction => c::SchematicJunctionPlotRecord {
            color: value
                .junction_color_authored
                .then(|| value.junction_color.clone()),
            kind: "junction".to_owned(),
            object_id: value.uuid.clone(),
            operation_count: count(value.operations.len())?,
            operations,
            uuid: value.uuid.clone(),
        }
        .into(),
    })
}

fn contract_annotation_record(
    value: &SchematicAnnotationRecord,
) -> Result<c::SchematicPlotRecord, PlotProjectionError> {
    let operations = plotter_operations(&value.operations)?;
    let text = || required(value.text.clone(), "schematic annotation text is missing");
    let shape = || {
        required(
            value.shape.as_deref(),
            "schematic annotation shape is missing",
        )
        .and_then(enum_ref)
    };
    Ok(match value.kind {
        SchematicAnnotationRecordKind::Label => c::SchematicLabelPlotRecord {
            kind: "label".to_owned(),
            object_id: value.object_id.clone(),
            operation_count: count(value.operations.len())?,
            operations,
            text: text()?,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicAnnotationRecordKind::GlobalLabel => c::SchematicGlobalLabelPlotRecord {
            kind: "global_label".to_owned(),
            object_id: value.object_id.clone(),
            operation_count: count(value.operations.len())?,
            operations,
            shape: enum_ref(required(
                value.shape.as_deref(),
                "schematic annotation shape is missing",
            )?)?,
            text: text()?,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicAnnotationRecordKind::HierarchicalLabel => {
            c::SchematicHierarchicalLabelPlotRecord {
                kind: "hierarchical_label".to_owned(),
                object_id: value.object_id.clone(),
                operation_count: count(value.operations.len())?,
                operations,
                shape: shape()?,
                text: text()?,
                uuid: value.uuid.clone(),
            }
            .into()
        }
        SchematicAnnotationRecordKind::NetclassFlag => c::SchematicNetclassFlagPlotRecord {
            at_x_nm: safe_integer(required(value.at_x_nm, "netclass flag x is missing")?)?,
            at_y_nm: safe_integer(required(value.at_y_nm, "netclass flag y is missing")?)?,
            kind: "netclass_flag".to_owned(),
            length_nm: safe_integer(required(
                value.length_nm,
                "netclass flag length is missing",
            )?)?,
            object_id: value.object_id.clone(),
            operation_count: count(value.operations.len())?,
            operations,
            shape: enum_ref(required(
                value.shape.as_deref(),
                "schematic annotation shape is missing",
            )?)?,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicAnnotationRecordKind::Text => c::SchematicTextPlotRecord {
            kind: "text".to_owned(),
            object_id: value.object_id.clone(),
            operation_count: count(value.operations.len())?,
            operations,
            text: text()?,
            uuid: value.uuid.clone(),
        }
        .into(),
        SchematicAnnotationRecordKind::TextBox => c::SchematicTextBoxPlotRecord {
            kind: "text_box".to_owned(),
            object_id: value.object_id.clone(),
            operation_count: count(value.operations.len())?,
            operations,
            text: text()?,
            uuid: value.uuid.clone(),
        }
        .into(),
    })
}

fn contract_graphic_record(
    value: &SchematicGraphicRecord,
) -> Result<c::SchematicPlotRecord, PlotProjectionError> {
    let operations = plotter_operations(&value.operations)?;
    Ok(match value.kind {
        SchematicGraphicRecordKind::GraphicPolyline => simple_record!(
            SchematicGraphicPolylinePlotRecord,
            value,
            "graphic_polyline",
            operations
        ),
        SchematicGraphicRecordKind::GraphicArc => simple_record!(
            SchematicGraphicArcPlotRecord,
            value,
            "graphic_arc",
            operations
        ),
        SchematicGraphicRecordKind::GraphicCircle => simple_record!(
            SchematicGraphicCirclePlotRecord,
            value,
            "graphic_circle",
            operations
        ),
        SchematicGraphicRecordKind::GraphicRectangle => simple_record!(
            SchematicGraphicRectanglePlotRecord,
            value,
            "graphic_rectangle",
            operations
        ),
        SchematicGraphicRecordKind::GraphicBezier => simple_record!(
            SchematicGraphicBezierPlotRecord,
            value,
            "graphic_bezier",
            operations
        ),
    })
}

enum ContractOperation {
    Plotter(c::PlotterOperation),
    SymbolStart(c::SchematicSymbolStartBlockOperation),
    SheetStart(c::SchematicSheetStartBlockOperation),
    End(u32),
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive schematic operation-union adapter"
)]
fn contract_operation(
    value: &SchematicPlotOperation,
    index: usize,
) -> Result<ContractOperation, PlotProjectionError> {
    let index_u32 = count(index)?;
    Ok(match value {
        SchematicPlotOperation::Plotter(value) => {
            if matches!(
                value,
                crate::PlotterOperation::FlashPadCircle(_)
                    | crate::PlotterOperation::FlashPadOval(_)
                    | crate::PlotterOperation::FlashPadRect(_)
                    | crate::PlotterOperation::FlashPadRoundRect(_)
                    | crate::PlotterOperation::FlashPadCustom(_)
                    | crate::PlotterOperation::FlashPadTrapez(_)
            ) {
                return Err(invalid(
                    "operation is outside the schematic plot vocabulary",
                ));
            }
            ContractOperation::Plotter(contract_schematic_plotter_operation(index, value.clone())?)
        }
        SchematicPlotOperation::Text(value) => {
            let mut operation = contract_schematic_plotter_operation(
                index,
                crate::PlotterOperation::Text(value.text.clone()),
            )?;
            let c::PlotterOperation::TextOperation(text) = &mut operation else {
                unreachable!("Text source projects to Text")
            };
            text.context = value
                .hyperlink_href
                .as_ref()
                .map(|href| {
                    Ok(c::PlotterOperationContext {
                        hyperlink: c::PlotterHyperlink {
                            href: href.parse().map_err(|error: c::error::ConversionError| {
                                invalid(error.to_string())
                            })?,
                        },
                    })
                })
                .transpose()?;
            ContractOperation::Plotter(operation)
        }
        SchematicPlotOperation::StyledThickSegment(value) => {
            let mut operation = contract_schematic_plotter_operation(
                index,
                crate::PlotterOperation::ThickSegment(value.segment.clone()),
            )?;
            let c::PlotterOperation::ThickSegmentOperation(segment) = &mut operation else {
                unreachable!("segment source projects to segment")
            };
            segment.stroke_color = Some(value.stroke_color.clone());
            ContractOperation::Plotter(operation)
        }
        SchematicPlotOperation::PlotImage(value) => ContractOperation::Plotter(
            c::PlotImageOperation {
                height_nm: safe_integer(value.height_nm)?,
                image_data_b64: value.image_data_b64.clone(),
                image_format: value.image_format.clone(),
                index: index_u32,
                kind: "PlotImage".to_owned(),
                scale: value.scale,
                stroke_color: value.stroke_color.clone(),
                width_nm: safe_integer(value.width_nm)?,
                x: safe_integer(value.x)?,
                y: safe_integer(value.y)?,
            }
            .into(),
        ),
        SchematicPlotOperation::StartSymbolPinBlock(value) => {
            ContractOperation::SymbolStart(c::SchematicSymbolStartBlockOperation {
                data_ref: "symbol_pin".to_owned(),
                data_uuid: value.data_uuid.clone(),
                extra_attrs: c::SchematicSymbolPinBlockAttrs::from(nonempty_map([
                    ("primitive", &value.extra_attrs.primitive),
                    ("object-type", &value.extra_attrs.object_type),
                    ("pin", &value.extra_attrs.pin),
                    ("symbol-uuid", &value.extra_attrs.symbol_uuid),
                    ("designator", &value.extra_attrs.designator),
                    ("lib-pin-uuid", &value.extra_attrs.lib_pin_uuid),
                ])),
                index: index_u32,
                kind: "StartBlock".to_owned(),
                label: value.label.clone(),
                object_id: value.object_id.clone(),
            })
        }
        SchematicPlotOperation::StartSheetPinBlock(value) => {
            ContractOperation::SheetStart(c::SchematicSheetStartBlockOperation {
                data_ref: "sheet_pin".to_owned(),
                data_uuid: value.data_uuid.clone(),
                extra_attrs: c::SchematicSheetPinBlockAttrs::from(nonempty_map([
                    ("primitive", &value.extra_attrs.primitive),
                    ("object-type", &value.extra_attrs.object_type),
                    ("sheet-uuid", &value.extra_attrs.sheet_uuid),
                    ("sheet-name", &value.extra_attrs.sheet_name),
                    ("sheet-file", &value.extra_attrs.sheet_file),
                    ("pin", &value.extra_attrs.pin),
                    ("pin-name", &value.extra_attrs.pin_name),
                    ("shape", &value.extra_attrs.shape),
                ])),
                index: index_u32,
                kind: "StartBlock".to_owned(),
                label: value.label.clone(),
                object_id: value.object_id.clone(),
            })
        }
        SchematicPlotOperation::EndBlock => ContractOperation::End(index_u32),
    })
}

fn plotter_operations(
    values: &[SchematicPlotOperation],
) -> Result<Vec<c::PlotterOperation>, PlotProjectionError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match contract_operation(value, index)? {
            ContractOperation::Plotter(value) => Ok(value),
            ContractOperation::SymbolStart(_)
            | ContractOperation::SheetStart(_)
            | ContractOperation::End(_) => Err(invalid(
                "block operation is not allowed on this schematic record",
            )),
        })
        .collect()
}

fn symbol_operations(
    values: &[SchematicPlotOperation],
) -> Result<Vec<c::SchematicSymbolOperation>, PlotProjectionError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match contract_operation(value, index)? {
            ContractOperation::Plotter(value) => plotter_into_symbol(value),
            ContractOperation::SymbolStart(value) => Ok(value.into()),
            ContractOperation::End(index) => Ok(c::SchematicSymbolEndBlockOperation {
                index,
                kind: "EndBlock".to_owned(),
            }
            .into()),
            ContractOperation::SheetStart(_) => {
                Err(invalid("sheet pin block is not allowed on a symbol record"))
            }
        })
        .collect()
}

fn sheet_operations(
    values: &[SchematicPlotOperation],
) -> Result<Vec<c::SchematicSheetOperation>, PlotProjectionError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match contract_operation(value, index)? {
            ContractOperation::Plotter(value) => plotter_into_sheet(value),
            ContractOperation::SheetStart(value) => Ok(value.into()),
            ContractOperation::End(index) => Ok(c::SchematicSheetEndBlockOperation {
                index,
                kind: "EndBlock".to_owned(),
            }
            .into()),
            ContractOperation::SymbolStart(_) => {
                Err(invalid("symbol pin block is not allowed on a sheet record"))
            }
        })
        .collect()
}

fn plotter_into_symbol(
    value: c::PlotterOperation,
) -> Result<c::SchematicSymbolOperation, PlotProjectionError> {
    Ok(match value {
        c::PlotterOperation::ThickSegmentOperation(v) => v.into(),
        c::PlotterOperation::ArcThreePointOperation(v) => v.into(),
        c::PlotterOperation::CircleOperation(v) => v.into(),
        c::PlotterOperation::RectOperation(v) => v.into(),
        c::PlotterOperation::PlotPolyOperation(v) => v.into(),
        c::PlotterOperation::BezierCurveOperation(v) => v.into(),
        c::PlotterOperation::TextOperation(v) => v.into(),
        c::PlotterOperation::PlotImageOperation(v) => v.into(),
        c::PlotterOperation::FlashPadCircleOperation(v) => v.into(),
        c::PlotterOperation::FlashPadOvalOperation(v) => v.into(),
        c::PlotterOperation::FlashPadRectOperation(v) => v.into(),
        c::PlotterOperation::FlashPadRoundRectOperation(v) => v.into(),
        c::PlotterOperation::FlashPadCustomOperation(v) => v.into(),
        c::PlotterOperation::FlashPadTrapezOperation(v) => v.into(),
    })
}

fn plotter_into_sheet(
    value: c::PlotterOperation,
) -> Result<c::SchematicSheetOperation, PlotProjectionError> {
    Ok(match value {
        c::PlotterOperation::ThickSegmentOperation(v) => v.into(),
        c::PlotterOperation::RectOperation(v) => v.into(),
        c::PlotterOperation::PlotPolyOperation(v) => v.into(),
        c::PlotterOperation::TextOperation(v) => v.into(),
        c::PlotterOperation::ArcThreePointOperation(_)
        | c::PlotterOperation::CircleOperation(_)
        | c::PlotterOperation::BezierCurveOperation(_)
        | c::PlotterOperation::PlotImageOperation(_)
        | c::PlotterOperation::FlashPadCircleOperation(_)
        | c::PlotterOperation::FlashPadOvalOperation(_)
        | c::PlotterOperation::FlashPadRectOperation(_)
        | c::PlotterOperation::FlashPadRoundRectOperation(_)
        | c::PlotterOperation::FlashPadCustomOperation(_)
        | c::PlotterOperation::FlashPadTrapezOperation(_) => {
            return Err(invalid(
                "operation is outside the hierarchical-sheet vocabulary",
            ));
        }
    })
}

fn nonempty_map<const N: usize>(values: [(&str, &String); N]) -> BTreeMap<String, String> {
    values
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| (key.to_owned(), value.clone()))
        .collect()
}

fn required<T>(value: Option<T>, message: &'static str) -> Result<T, PlotProjectionError> {
    value.ok_or_else(|| invalid(message))
}

fn enum_ref<T>(value: &str) -> Result<T, PlotProjectionError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse::<T>()
        .map_err(|error| invalid(error.to_string()))
}

fn safe_integer(value: i64) -> Result<JavaScriptSafeInteger, PlotProjectionError> {
    JavaScriptSafeInteger::try_from(value).map_err(|error| {
        PlotProjectionError::new(PlotProjectionErrorKind::NumericRange, error.to_string())
    })
}

fn count(value: usize) -> Result<u32, PlotProjectionError> {
    u32::try_from(value).map_err(|_| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::NumericRange,
            "schematic count or index exceeds uint32",
        )
    })
}

fn optional_len(value: Option<&String>) -> usize {
    value.map_or(0, String::len)
}

fn strings_len(values: &[String]) -> Result<usize, PlotProjectionError> {
    values.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value.len()).ok_or_else(resource_overflow)
    })
}

fn checked_lengths<const N: usize>(values: [usize; N]) -> Result<usize, PlotProjectionError> {
    values.into_iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or_else(resource_overflow)
    })
}

fn nonempty_count<const N: usize>(values: [&String; N]) -> usize {
    values.into_iter().filter(|value| !value.is_empty()).count()
}

fn require_canonical_styled_segment(
    value: &crate::SchematicStyledThickSegment,
) -> Result<(), PlotProjectionError> {
    let segment = &value.segment;
    if segment.layer.is_some()
        || segment.role.is_some()
        || !segment.layers.is_empty()
        || segment.mask_margin_nm.is_some()
        || segment.pad_size_x_nm.is_some()
        || segment.pad_size_y_nm.is_some()
    {
        return Err(invalid(
            "styled schematic segments cannot carry layer, role, mask, or pad metadata",
        ));
    }
    Ok(())
}

fn resource_overflow() -> PlotProjectionError {
    PlotProjectionError::new(
        PlotProjectionErrorKind::ResourceLimit,
        "schematic projection accounting overflowed",
    )
}

fn invalid(message: impl Into<String>) -> PlotProjectionError {
    PlotProjectionError::new(PlotProjectionErrorKind::InvalidModel, message)
}

//! TypeSpec projection for native board plotter documents.

use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::board_plot_document::{
    BoardDimensionType, BoardFootprintOperation as ContractBoardFootprintOperation,
    BoardFootprintPlacement as ContractBoardFootprintPlacement, BoardFootprintPlotRecord,
    BoardGraphicPlotRecord, BoardGraphicRecordKind, BoardPlotDocumentA0, BoardPlotRecord,
    BoardTextBoxPlotRecord, BoardTextPlotRecord, BoardViaType, CircleOperation,
    DimensionPlotRecord, FlashPadCircleOperation, PlotterCoordinateSpace, PlotterDrillRole,
    PlotterFill, PlotterOperation, PlotterPoint, PlotterStringBool, PlotterTextHAlign,
    PlotterTextRenderCacheCoordinateSpace, PlotterTextRenderCacheSource, PlotterTextVAlign,
    PlotterViaFlashRole, TablePlotRecord, TextOperation, TextRenderCache, TextRenderCachePolygon,
    TrackArcPlotRecord, TrackSegmentPlotRecord, ViaPlotRecord, ZoneFillPlotRecord,
};
use kicad_monkey_contracts::validate_board_plot_document;

use crate::project_plotter_operation_a0 as contract_plotter_operation;
use crate::{
    BoardDimensionOperation, BoardDimensionRecord, BoardFootprintChildMetadata,
    BoardFootprintOperation as CoreBoardFootprintOperation, BoardFootprintRecord,
    BoardGraphicRecordKind as CoreGraphicRecordKind, BoardPlotRecord as CoreBoardPlotRecord,
    BoardTableOperation, BoardTableRecord, BoardTextBoxOperation, BoardTextBoxRecord,
    BoardTextHAlign, BoardTextOperation, BoardTextRecord,
    BoardTextRenderCacheCoordinateSpace as CoreTextRenderCacheCoordinateSpace,
    BoardTextRenderCacheSource, BoardTextVAlign, BoardViaOperation, BoardViaOperationKind,
    BoardViaRecord,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardPlotContractLimits {
    pub max_records: usize,
    pub max_operations: usize,
    pub max_points: usize,
    pub max_text_bytes: usize,
    pub max_nested_items: usize,
    pub max_materialized_bytes: usize,
}

impl Default for BoardPlotContractLimits {
    fn default() -> Self {
        Self {
            max_records: 1_000_000,
            max_operations: 4_000_000,
            max_points: 16_000_000,
            max_text_bytes: 256 * 1024 * 1024,
            max_nested_items: 32_000_000,
            max_materialized_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Project one native board document through the same generated TypeSpec
/// binding and semantic validator used by the browser adapter.
pub fn project_board_plot_document_a0(
    document: crate::BoardPlotDocument,
    source_path: Option<String>,
    document_id: String,
    limits: BoardPlotContractLimits,
) -> Result<BoardPlotDocumentA0, String> {
    let usage = projection_usage(&document, source_path.as_deref(), &document_id)?;
    usage.enforce(limits)?;
    let total_operations = usage.operations;
    let mut records = Vec::with_capacity(document.records.len());
    for record in document.records {
        records.push(contract_record(record)?);
    }
    let contract = BoardPlotDocumentA0 {
        coordinate_space: PlotterCoordinateSpace {
            unit: "nm".to_owned(),
            y_axis: "down".to_owned(),
        },
        document_id,
        generator: document.generator,
        generator_version: document.generator_version,
        paper: document.paper,
        records,
        schema: "kicad.plotter_ir.a0".to_owned(),
        source_kind: "PCB".to_owned(),
        source_path,
        thickness_mm: document.thickness_mm,
        total_operations: u32::try_from(total_operations).unwrap_or(u32::MAX),
        version: JavaScriptSafeInteger::try_from(document.version)
            .map_err(|error| error.to_string())?,
    };
    validate_board_plot_document(&contract).map_err(|error| error.to_string())?;
    Ok(contract)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ProjectionUsage {
    records: usize,
    operations: usize,
    points: usize,
    text_bytes: usize,
    nested_items: usize,
    materialized_bytes: usize,
}

impl ProjectionUsage {
    fn enforce(self, limits: BoardPlotContractLimits) -> Result<(), String> {
        for (actual, maximum, label) in [
            (self.records, limits.max_records, "record count"),
            (self.operations, limits.max_operations, "operation count"),
            (self.points, limits.max_points, "point count"),
            (self.text_bytes, limits.max_text_bytes, "text byte count"),
            (
                self.nested_items,
                limits.max_nested_items,
                "nested item count",
            ),
            (
                self.materialized_bytes,
                limits.max_materialized_bytes,
                "materialized byte count",
            ),
        ] {
            if actual > maximum {
                return Err(format!("board plot projection {label} exceeds its limit"));
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct ProjectionCounter {
    usage: ProjectionUsage,
}

impl ProjectionCounter {
    fn add(
        &mut self,
        target: fn(&mut ProjectionUsage) -> &mut usize,
        value: usize,
    ) -> Result<(), String> {
        let slot = target(&mut self.usage);
        *slot = slot
            .checked_add(value)
            .ok_or_else(|| "board plot projection preflight overflowed".to_owned())?;
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), String> {
        self.add(|usage| &mut usage.text_bytes, value.len())
    }

    fn optional_text(&mut self, value: Option<&str>) -> Result<(), String> {
        value.map_or(Ok(()), |value| self.text(value))
    }

    fn strings<'a>(&mut self, values: impl IntoIterator<Item = &'a String>) -> Result<(), String> {
        for value in values {
            self.text(value)?;
            self.add(|usage| &mut usage.nested_items, 1)?;
        }
        Ok(())
    }

    fn points(&mut self, count: usize) -> Result<(), String> {
        self.add(|usage| &mut usage.points, count)?;
        self.add(|usage| &mut usage.nested_items, count)
    }

    fn items(&mut self, count: usize) -> Result<(), String> {
        self.add(|usage| &mut usage.nested_items, count)
    }

    fn finish(mut self) -> Result<ProjectionUsage, String> {
        // Every retained source string can coexist with JSON/contract copies.
        // Each nested item receives ample Value/map/vector allocation headroom.
        self.usage.materialized_bytes = 64_usize
            .checked_mul(1024)
            .and_then(|value| value.checked_add(self.usage.text_bytes.checked_mul(4)?))
            .and_then(|value| value.checked_add(self.usage.nested_items.checked_mul(4096)?))
            .ok_or_else(|| {
                "board plot projection materialized byte estimate overflowed".to_owned()
            })?;
        Ok(self.usage)
    }
}

fn projection_usage(
    document: &crate::BoardPlotDocument,
    source_path: Option<&str>,
    document_id: &str,
) -> Result<ProjectionUsage, String> {
    let mut counter = ProjectionCounter::default();
    counter.usage.records = document.records.len();
    counter.usage.operations = document
        .records
        .iter()
        .map(crate::BoardPlotRecord::operation_count)
        .try_fold(0_usize, |total, count| total.checked_add(count))
        .ok_or_else(|| "board plot projection operation count overflowed".to_owned())?;
    counter.items(
        document
            .records
            .len()
            .saturating_add(counter.usage.operations),
    )?;
    for value in [
        &document.generator,
        &document.generator_version,
        &document.paper,
    ] {
        counter.text(value)?;
    }
    counter.optional_text(source_path)?;
    counter.text(document_id)?;
    for record in &document.records {
        preflight_record(record, &mut counter)?;
    }
    counter.finish()
}

fn preflight_record(
    record: &CoreBoardPlotRecord,
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    match record {
        CoreBoardPlotRecord::Graphic(value) => {
            counter.text(&value.uuid)?;
            counter.text(&value.layer)?;
            preflight_operations(&value.operations, counter)
        }
        CoreBoardPlotRecord::Segment(value) => {
            counter.text(&value.uuid)?;
            counter.text(&value.layer)?;
            preflight_net_facts(value.net_name.as_deref(), &value.net_classes, counter)?;
            preflight_operations(&value.operations, counter)
        }
        CoreBoardPlotRecord::TrackArc(value) => {
            counter.text(&value.uuid)?;
            counter.text(&value.layer)?;
            preflight_net_facts(value.net_name.as_deref(), &value.net_classes, counter)?;
            preflight_operations(&value.operations, counter)
        }
        CoreBoardPlotRecord::Text(value) => {
            counter.text(&value.uuid)?;
            counter.text(&value.layer)?;
            counter.text(&value.text)?;
            for operation in &value.operations {
                preflight_text_operation(operation, counter)?;
            }
            Ok(())
        }
        CoreBoardPlotRecord::TextBox(value) => {
            counter.text(&value.uuid)?;
            counter.text(&value.layer)?;
            counter.text(&value.text)?;
            for operation in &value.operations {
                match operation {
                    BoardTextBoxOperation::Border(operation) => {
                        preflight_operation(operation, counter)?
                    }
                    BoardTextBoxOperation::Text(operation) => {
                        preflight_text_operation(operation, counter)?
                    }
                }
            }
            Ok(())
        }
        CoreBoardPlotRecord::Via(value) => {
            counter.text(&value.uuid)?;
            counter.strings(&value.layers)?;
            preflight_net_facts(value.net_name.as_deref(), &value.net_classes, counter)?;
            for operation in &value.operations {
                counter.strings(&operation.layers)?;
            }
            Ok(())
        }
        CoreBoardPlotRecord::Table(value) => {
            counter.text(&value.uuid)?;
            counter.strings(&value.layers)?;
            counter.items(value.cell_bounds_nm.len())?;
            for operation in &value.operations {
                match operation {
                    BoardTableOperation::Segment(operation) => {
                        preflight_operation(operation, counter)?
                    }
                    BoardTableOperation::Text(operation) => {
                        preflight_text_operation(operation, counter)?
                    }
                }
            }
            Ok(())
        }
        CoreBoardPlotRecord::Dimension(value) => {
            counter.text(&value.uuid)?;
            counter.strings(&value.layers)?;
            counter.text(&value.dimension_type)?;
            counter.optional_text(value.text.as_deref())?;
            for operation in &value.operations {
                match operation {
                    BoardDimensionOperation::Geometry(operation) => {
                        preflight_operation(operation, counter)?
                    }
                    BoardDimensionOperation::Text(operation) => {
                        preflight_text_operation(operation, counter)?
                    }
                }
            }
            Ok(())
        }
        CoreBoardPlotRecord::Zone(value) => {
            counter.text(&value.uuid)?;
            counter.strings(&value.layers)?;
            counter.strings(&value.fill_layers)?;
            counter.items(value.fill_island.len())?;
            preflight_net_facts(value.net_name.as_deref(), &value.net_classes, counter)?;
            preflight_operations(&value.operations, counter)
        }
        CoreBoardPlotRecord::Footprint(value) => preflight_footprint(value, counter),
    }
}

fn preflight_net_facts(
    name: Option<&str>,
    classes: &crate::BoardNetClassExtras,
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    counter.optional_text(name)?;
    counter.optional_text(classes.net_class.as_deref())?;
    counter.strings(&classes.net_classes)
}

fn preflight_footprint(
    value: &BoardFootprintRecord,
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    for text in [
        &value.uuid,
        &value.library_link,
        &value.reference,
        &value.value,
        &value.layer,
        &value.descr,
        &value.tags,
    ] {
        counter.text(text)?;
    }
    counter.strings(&value.attr)?;
    for operation in &value.operations {
        match operation {
            CoreBoardFootprintOperation::Geometry {
                operation,
                metadata,
            } => {
                preflight_operation(operation, counter)?;
                preflight_child_metadata(metadata, counter)?;
            }
            CoreBoardFootprintOperation::Text {
                operation,
                metadata,
            } => {
                preflight_text_operation(operation, counter)?;
                preflight_child_metadata(metadata, counter)?;
            }
            CoreBoardFootprintOperation::Pad(operation) => preflight_operation(operation, counter)?,
            CoreBoardFootprintOperation::StartBlock(block) => {
                for text in [
                    &block.label,
                    &block.data_uuid,
                    &block.data_ref,
                    &block.object_id,
                ] {
                    counter.text(text)?;
                }
                counter.strings(&block.layers)?;
                counter.text(&block.extra_attrs.primitive)?;
                for text in [
                    block.extra_attrs.component.as_deref(),
                    block.extra_attrs.component_uid.as_deref(),
                    block.extra_attrs.component_uuid.as_deref(),
                    block.extra_attrs.footprint.as_deref(),
                    block.extra_attrs.pad_number.as_deref(),
                    block.extra_attrs.pad_designator.as_deref(),
                    block.extra_attrs.pad_type.as_deref(),
                    block.extra_attrs.pad_shape.as_deref(),
                    block.extra_attrs.layer_names.as_deref(),
                    block.extra_attrs.net_index.as_deref(),
                    block.extra_attrs.net_id.as_deref(),
                    block.extra_attrs.net.as_deref(),
                    block.extra_attrs.net_class.as_deref(),
                    block.extra_attrs.net_classes.as_deref(),
                    block.extra_attrs.hole_owner.as_deref(),
                    block.extra_attrs.hole_kind.as_deref(),
                    block.extra_attrs.hole_plating.as_deref(),
                    block.extra_attrs.hole_render.as_deref(),
                    block.extra_attrs.hole_diameter_mm.as_deref(),
                    block.extra_attrs.hole_width_mm.as_deref(),
                    block.extra_attrs.hole_height_mm.as_deref(),
                ] {
                    counter.optional_text(text)?;
                }
            }
            CoreBoardFootprintOperation::EndBlock => {}
        }
    }
    Ok(())
}

fn preflight_child_metadata(
    value: &BoardFootprintChildMetadata,
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    let attrs = &value.extra_attrs;
    for text in [
        &value.label,
        &value.data_uuid,
        &value.data_ref,
        &value.object_id,
        &attrs.component,
        &attrs.component_uid,
        &attrs.component_uuid,
        &attrs.footprint,
        &attrs.primitive,
        &attrs.footprint_primitive,
    ] {
        counter.text(text)?;
    }
    for text in [
        attrs.layer_name.as_deref(),
        attrs.layer_role.as_deref(),
        attrs.footprint_text_role.as_deref(),
        attrs.property_name.as_deref(),
        attrs.fp_text_type.as_deref(),
        attrs.footprint_graphic_kind.as_deref(),
    ] {
        counter.optional_text(text)?;
    }
    Ok(())
}

fn preflight_operations(
    operations: &[crate::PlotterOperation],
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    for operation in operations {
        preflight_operation(operation, counter)?;
    }
    Ok(())
}

fn preflight_operation(
    operation: &crate::PlotterOperation,
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    use crate::PlotterOperation as Operation;
    let strings =
        |counter: &mut ProjectionCounter, layer: Option<&str>, colors: &[Option<&str>]| {
            counter.optional_text(layer)?;
            for color in colors {
                counter.optional_text(*color)?;
            }
            Ok::<(), String>(())
        };
    match operation {
        Operation::ThickSegment(value) => {
            strings(counter, value.layer.as_deref(), &[value.role.as_deref()])?;
            counter.strings(&value.layers)
        }
        Operation::ArcThreePoint(value) => strings(
            counter,
            value.layer.as_deref(),
            &[value.stroke_color.as_deref(), value.fill_color.as_deref()],
        ),
        Operation::Circle(value) => {
            strings(
                counter,
                value.layer.as_deref(),
                &[
                    value.role.as_deref(),
                    value.stroke_color.as_deref(),
                    value.fill_color.as_deref(),
                ],
            )?;
            counter.strings(&value.layers)
        }
        Operation::Rect(value) => strings(
            counter,
            value.layer.as_deref(),
            &[value.stroke_color.as_deref(), value.fill_color.as_deref()],
        ),
        Operation::PlotPoly(value) => {
            strings(
                counter,
                value.layer.as_deref(),
                &[value.stroke_color.as_deref(), value.fill_color.as_deref()],
            )?;
            counter.points(value.points.len())
        }
        Operation::BezierCurve(value) => {
            strings(
                counter,
                value.layer.as_deref(),
                &[value.stroke_color.as_deref()],
            )?;
            counter.points(4)
        }
        Operation::Text(value) => {
            for text in [&value.text, &value.color, &value.font_face] {
                counter.text(text)?;
            }
            counter.optional_text(value.layer.as_deref())
        }
        Operation::FlashPadCircle(value) => counter.strings(&value.layers),
        Operation::FlashPadOval(value) => counter.strings(&value.layers),
        Operation::FlashPadRect(value) => counter.strings(&value.layers),
        Operation::FlashPadRoundRect(value) => counter.strings(&value.layers),
        Operation::FlashPadCustom(value) => {
            counter.optional_text(value.anchor_shape.as_deref())?;
            counter.strings(&value.layers)?;
            counter.items(value.polygons.len())?;
            counter.items(value.polygon_widths_nm.as_ref().map_or(0, Vec::len))?;
            for polygon in &value.polygons {
                counter.points(polygon.len())?;
            }
            Ok(())
        }
        Operation::FlashPadTrapez(value) => {
            counter.strings(&value.layers)?;
            counter.points(value.corners.len())
        }
    }
}

fn preflight_text_operation(
    operation: &BoardTextOperation,
    counter: &mut ProjectionCounter,
) -> Result<(), String> {
    for text in [&operation.text, &operation.color, &operation.font_face] {
        counter.text(text)?;
    }
    counter.optional_text(operation.layer.as_deref())?;
    counter.items(operation.render_cache_polygons.len())?;
    for polygon in &operation.render_cache_polygons {
        counter.points(polygon.len())?;
    }
    if let Some(cache) = &operation.render_cache {
        counter.text(&cache.text)?;
        counter.items(cache.polygons.len())?;
        for polygon in &cache.polygons {
            counter.items(polygon.len())?;
            for contour in polygon {
                counter.points(contour.len())?;
            }
        }
    }
    Ok(())
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
        CoreBoardPlotRecord::Text(record) => contract_text_record(record)?,
        CoreBoardPlotRecord::TextBox(record) => contract_text_box_record(record)?,
        CoreBoardPlotRecord::Via(record) => contract_via_record(record)?.into(),
        CoreBoardPlotRecord::Table(record) => contract_table_record(record)?.into(),
        CoreBoardPlotRecord::Dimension(record) => contract_dimension_record(record)?.into(),
        CoreBoardPlotRecord::Footprint(record) => contract_footprint_record(record)?.into(),
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

fn contract_footprint_record(
    record: BoardFootprintRecord,
) -> Result<BoardFootprintPlotRecord, String> {
    let operation_count = contract_count(record.operations.len());
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| contract_footprint_operation(index, operation))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BoardFootprintPlotRecord {
        attr: record.attr,
        descr: record.descr,
        kind: "footprint".to_owned(),
        layer: record.layer,
        library_link: record.library_link.clone(),
        locked: record.locked,
        object_id: record.library_link,
        operation_count,
        operations,
        placement: ContractBoardFootprintPlacement {
            angle_deg: record.placement.angle_deg,
            x_nm: safe_integer(record.placement.x_nm)?,
            y_nm: safe_integer(record.placement.y_nm)?,
        },
        reference: record.reference,
        tags: record.tags,
        uuid: record.uuid,
        value: record.value,
    })
}

fn contract_footprint_operation(
    index: usize,
    operation: CoreBoardFootprintOperation,
) -> Result<ContractBoardFootprintOperation, String> {
    let mut value = match operation {
        CoreBoardFootprintOperation::Geometry {
            operation,
            metadata,
        } => {
            let shared = contract_plotter_operation(index, operation)?;
            operation_with_footprint_metadata(shared, metadata)?
        }
        CoreBoardFootprintOperation::Text {
            operation,
            metadata,
        } => {
            let text = contract_text_operation(index, operation)?;
            operation_with_footprint_metadata(text, metadata)?
        }
        CoreBoardFootprintOperation::Pad(operation) => {
            let shared = contract_plotter_operation(index, operation)?;
            serde_json::to_value(shared).map_err(|error| error.to_string())?
        }
        CoreBoardFootprintOperation::StartBlock(block) => serde_json::json!({
            "kind": "StartBlock",
            "index": contract_count(index),
            "label": block.label,
            "data_uuid": block.data_uuid,
            "data_ref": block.data_ref,
            "object_id": block.object_id,
            "layers": block.layers,
            "extra_attrs": {
                "primitive": block.extra_attrs.primitive,
                "component": block.extra_attrs.component,
                "component_uid": block.extra_attrs.component_uid,
                "component_uuid": block.extra_attrs.component_uuid,
                "footprint": block.extra_attrs.footprint,
                "pad_number": block.extra_attrs.pad_number,
                "pad_designator": block.extra_attrs.pad_designator,
                "pad_type": block.extra_attrs.pad_type,
                "pad_shape": block.extra_attrs.pad_shape,
                "layer_names": block.extra_attrs.layer_names,
                "net_index": block.extra_attrs.net_index,
                "net_id": block.extra_attrs.net_id,
                "net": block.extra_attrs.net,
                "net_class": block.extra_attrs.net_class,
                "net_classes": block.extra_attrs.net_classes,
                "hole_owner": block.extra_attrs.hole_owner,
                "hole_kind": block.extra_attrs.hole_kind,
                "hole_plating": block.extra_attrs.hole_plating,
                "hole_render": block.extra_attrs.hole_render,
                "hole_diameter_mm": block.extra_attrs.hole_diameter_mm,
                "hole_width_mm": block.extra_attrs.hole_width_mm,
                "hole_height_mm": block.extra_attrs.hole_height_mm,
            },
        }),
        CoreBoardFootprintOperation::EndBlock => serde_json::json!({
            "kind": "EndBlock",
            "index": contract_count(index),
        }),
    };
    omit_null_object_fields(&mut value);
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn omit_null_object_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.retain(|_, child| !child.is_null());
            object.values_mut().for_each(omit_null_object_fields);
        }
        serde_json::Value::Array(values) => {
            values.iter_mut().for_each(omit_null_object_fields);
        }
        _ => {}
    }
}

fn operation_with_footprint_metadata<T: serde::Serialize>(
    operation: T,
    metadata: BoardFootprintChildMetadata,
) -> Result<serde_json::Value, String> {
    let mut value = serde_json::to_value(operation).map_err(|error| error.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "projected plotter operation is not an object".to_owned())?;
    object.insert(
        "label".to_owned(),
        serde_json::Value::String(metadata.label),
    );
    object.insert(
        "data_uuid".to_owned(),
        serde_json::Value::String(metadata.data_uuid),
    );
    object.insert(
        "data_ref".to_owned(),
        serde_json::Value::String(metadata.data_ref),
    );
    object.insert(
        "object_id".to_owned(),
        serde_json::Value::String(metadata.object_id),
    );
    let attrs = metadata.extra_attrs;
    object.insert(
        "extra_attrs".to_owned(),
        serde_json::json!({
            "component": attrs.component,
            "component_uid": attrs.component_uid,
            "component_uuid": attrs.component_uuid,
            "footprint": attrs.footprint,
            "layer_name": attrs.layer_name,
            "layer_role": attrs.layer_role,
            "primitive": attrs.primitive,
            "footprint_primitive": attrs.footprint_primitive,
            "footprint_object_index": contract_count(attrs.footprint_object_index),
            "footprint_subop_index": attrs.footprint_subop_index.map(contract_count),
            "footprint_text_role": attrs.footprint_text_role,
            "property_name": attrs.property_name,
            "fp_text_type": attrs.fp_text_type,
            "footprint_graphic_kind": attrs.footprint_graphic_kind,
        }),
    );
    Ok(value)
}

fn contract_dimension_record(record: BoardDimensionRecord) -> Result<DimensionPlotRecord, String> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| match operation {
            BoardDimensionOperation::Geometry(operation) => {
                let shared = contract_plotter_operation(index, operation)?;
                let value = serde_json::to_value(shared).map_err(|error| error.to_string())?;
                serde_json::from_value(value).map_err(|error| error.to_string())
            }
            BoardDimensionOperation::Text(operation) => {
                contract_text_operation(index, operation).map(PlotterOperation::from)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(DimensionPlotRecord {
        dimension_type: match record.dimension_type.as_str() {
            "aligned" => BoardDimensionType::Aligned,
            "orthogonal" => BoardDimensionType::Orthogonal,
            "radial" => BoardDimensionType::Radial,
            "leader" => BoardDimensionType::Leader,
            "center" => BoardDimensionType::Center,
            _ => return Err("unsupported board dimension type".to_owned()),
        },
        kind: "dimension".to_owned(),
        layers: record.layers,
        object_id: "dimension".to_owned(),
        operation_count: contract_count(operations.len()),
        operations,
        text: record.text,
        uuid: record.uuid,
    })
}

fn contract_table_record(record: BoardTableRecord) -> Result<TablePlotRecord, String> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| match operation {
            BoardTableOperation::Segment(operation) => {
                let shared = contract_plotter_operation(index, operation)?;
                let value = serde_json::to_value(shared).map_err(|error| error.to_string())?;
                serde_json::from_value(value).map_err(|error| error.to_string())
            }
            BoardTableOperation::Text(operation) => {
                contract_text_operation(index, operation).map(PlotterOperation::from)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TablePlotRecord {
        cell_count: contract_count(record.cell_count),
        kind: "table".to_owned(),
        layers: record.layers,
        object_id: "table".to_owned(),
        operation_count: contract_count(operations.len()),
        operations,
        uuid: record.uuid,
    })
}

fn contract_text_record(record: BoardTextRecord) -> Result<BoardPlotRecord, String> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| {
            contract_text_operation(index, operation).map(PlotterOperation::from)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(BoardTextPlotRecord {
        // Board gr_text carriers have no hide attribute upstream, so the
        // established serializer's getattr default is always false.
        hide: false,
        kind: "gr_text".to_owned(),
        layer: record.layer,
        object_id: "gr_text".to_owned(),
        operation_count: contract_count(operations.len()),
        operations,
        text: record.text,
        uuid: record.uuid,
    }
    .into())
}

fn contract_text_box_record(record: BoardTextBoxRecord) -> Result<BoardPlotRecord, String> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| contract_text_box_operation(index, operation))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(BoardTextBoxPlotRecord {
        border: record.border,
        kind: "gr_text_box".to_owned(),
        layer: record.layer,
        object_id: "gr_text_box".to_owned(),
        operation_count: contract_count(operations.len()),
        operations,
        text: record.text,
        uuid: record.uuid,
    }
    .into())
}

fn contract_text_box_operation(
    index: usize,
    operation: BoardTextBoxOperation,
) -> Result<PlotterOperation, String> {
    match operation {
        BoardTextBoxOperation::Border(operation) => {
            let shared = contract_plotter_operation(index, operation)?;
            let value = serde_json::to_value(shared).map_err(|error| error.to_string())?;
            serde_json::from_value(value).map_err(|error| error.to_string())
        }
        BoardTextBoxOperation::Text(operation) => {
            contract_text_operation(index, operation).map(PlotterOperation::from)
        }
    }
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
            let source = match cache.source {
                BoardTextRenderCacheSource::ExistingFile => {
                    PlotterTextRenderCacheSource::ExistingFileCache
                }
                BoardTextRenderCacheSource::NativeGenerated => {
                    PlotterTextRenderCacheSource::NativeGeneratedCache
                }
            };
            Ok(TextRenderCache {
                angle: cache.angle,
                coordinate_space: match cache.coordinate_space {
                    CoreTextRenderCacheCoordinateSpace::Board => {
                        PlotterTextRenderCacheCoordinateSpace::Board
                    }
                    CoreTextRenderCacheCoordinateSpace::FootprintLocal => {
                        PlotterTextRenderCacheCoordinateSpace::FootprintLocal
                    }
                },
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
                source,
                text: cache.text,
                unit: "nm".to_owned(),
            })
        })
        .transpose()?;
    let render_cache_exact = render_cache.as_ref().map(|cache| cache.exact);
    let render_cache_source = render_cache.as_ref().map(|cache| cache.source);
    Ok(TextOperation {
        bold: operation.bold,
        color: operation.color,
        context: None,
        font_face: operation.font_face,
        h_align: match operation.h_align {
            BoardTextHAlign::Left => PlotterTextHAlign::GrTextHAlignLeft,
            BoardTextHAlign::Center => PlotterTextHAlign::GrTextHAlignCenter,
            BoardTextHAlign::Right => PlotterTextHAlign::GrTextHAlignRight,
        },
        index: contract_count(index),
        italic: operation.italic,
        kind: "Text".to_owned(),
        layer: operation.layer,
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
            crate::BoardViaType::Through => BoardViaType::Through,
            crate::BoardViaType::Blind => BoardViaType::Blind,
            crate::BoardViaType::Buried => BoardViaType::Buried,
            crate::BoardViaType::Micro => BoardViaType::Micro,
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
    operations: Vec<crate::PlotterOperation>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardPlotLimits, board_plot_document};

    const SOURCE: &str = r#"(kicad_pcb
      (version 20240108) (generator pcbnew)
      (general (thickness 1.6)) (paper "A4")
      (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.SilkS" user "b.silkscreen"))
      (gr_line (start 0 0) (end 1 1)
        (stroke (width 0.1) (type solid)) (layer "B.SilkS"))
      (gr_poly (pts (xy 0 0) (xy 2 0) (xy 1 1))
        (stroke (width 0.1) (type solid)) (fill none) (layer "B.SilkS")))"#;

    fn project(limits: BoardPlotContractLimits) -> Result<BoardPlotDocumentA0, String> {
        project_board_plot_document_a0(
            board_plot_document(SOURCE, BoardPlotLimits::default()).expect("board plot document"),
            Some("fixture.kicad_pcb".to_owned()),
            "fixture".to_owned(),
            limits,
        )
    }

    #[test]
    fn projection_limits_accept_exact_and_reject_one_under() {
        let source = board_plot_document(SOURCE, BoardPlotLimits::default()).expect("document");
        let usage = projection_usage(&source, Some("fixture.kicad_pcb"), "fixture")
            .expect("projection preflight");
        let exact_limits = BoardPlotContractLimits {
            max_records: usage.records,
            max_operations: usage.operations,
            max_points: usage.points,
            max_text_bytes: usage.text_bytes,
            max_nested_items: usage.nested_items,
            max_materialized_bytes: usage.materialized_bytes,
        };
        let exact = project(exact_limits).expect("inclusive projection boundaries");
        assert_eq!(exact.records.len(), usage.records);
        for limits in [
            BoardPlotContractLimits {
                max_records: usage.records - 1,
                ..exact_limits
            },
            BoardPlotContractLimits {
                max_operations: usage.operations - 1,
                ..exact_limits
            },
            BoardPlotContractLimits {
                max_points: usage.points - 1,
                ..exact_limits
            },
            BoardPlotContractLimits {
                max_text_bytes: usage.text_bytes - 1,
                ..exact_limits
            },
            BoardPlotContractLimits {
                max_nested_items: usage.nested_items - 1,
                ..exact_limits
            },
            BoardPlotContractLimits {
                max_materialized_bytes: usage.materialized_bytes - 1,
                ..exact_limits
            },
        ] {
            assert!(project(limits).is_err());
        }
    }
}

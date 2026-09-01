//! TypeSpec projection for native board plotter documents.

use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::board_plot_document as board_contract;
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

use crate::plotter_contract::contract_board_plotter_operation as contract_plotter_operation;
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
use crate::{
    BoardPlotSourceArtifact, BoardRenderFacts, PlotDocumentMetadata, PlotDocumentProjectionLimits,
    PlotProjectionError, PlotProjectionErrorKind,
};

#[derive(Clone, Debug)]
pub struct ProjectedBoardPlotArtifact {
    document: BoardPlotDocumentA0,
    render_facts: BoardRenderFacts,
}

impl ProjectedBoardPlotArtifact {
    pub fn document(&self) -> &BoardPlotDocumentA0 {
        &self.document
    }

    pub fn render_facts(&self) -> &BoardRenderFacts {
        &self.render_facts
    }
}

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
    project_board_plot_document_with_limits_a0(
        document,
        PlotDocumentMetadata {
            document_id,
            source_path,
        },
        limits,
    )
    .map_err(|error| error.to_string())
}

/// Project one board document through a fully typed, bounded direct-Rust API.
pub fn project_board_plot_document_with_metadata_a0(
    document: crate::BoardPlotDocument,
    metadata: PlotDocumentMetadata,
    limits: PlotDocumentProjectionLimits,
) -> Result<BoardPlotDocumentA0, PlotProjectionError> {
    project_board_plot_document_with_limits_a0(
        document,
        metadata,
        BoardPlotContractLimits {
            max_records: limits.max_records,
            max_operations: limits.max_operations,
            max_points: limits.max_points,
            max_text_bytes: limits.max_string_bytes,
            max_nested_items: limits.max_nested_items,
            max_materialized_bytes: limits.max_materialized_bytes,
        },
    )
}

fn project_board_plot_document_with_limits_a0(
    document: crate::BoardPlotDocument,
    metadata: PlotDocumentMetadata,
    limits: BoardPlotContractLimits,
) -> Result<BoardPlotDocumentA0, PlotProjectionError> {
    let PlotDocumentMetadata {
        source_path,
        document_id,
    } = metadata;
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
        total_operations: contract_count(total_operations)?,
        version: safe_integer(document.version)?,
    };
    validate_board_plot_document(&contract).map_err(|error| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::ContractValidation,
            error.to_string(),
        )
    })?;
    Ok(contract)
}

/// Project an atomically bound board document/layer-facts pair. The existing
/// document-only projector remains available for compatibility callers that do
/// not request layer filtering.
pub fn project_board_plot_artifact_a0(
    source: BoardPlotSourceArtifact,
    metadata: PlotDocumentMetadata,
    limits: PlotDocumentProjectionLimits,
) -> Result<ProjectedBoardPlotArtifact, PlotProjectionError> {
    let (document, render_facts) = source.into_parts();
    let contract = project_board_plot_document_with_metadata_a0(document, metadata, limits)?;
    Ok(ProjectedBoardPlotArtifact {
        document: contract,
        render_facts,
    })
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
    fn enforce(self, limits: BoardPlotContractLimits) -> Result<(), PlotProjectionError> {
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
                return Err(resource_error(format!(
                    "board plot projection {label} exceeds its limit"
                )));
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
    ) -> Result<(), PlotProjectionError> {
        let slot = target(&mut self.usage);
        *slot = slot
            .checked_add(value)
            .ok_or_else(|| resource_error("board plot projection preflight overflowed"))?;
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), PlotProjectionError> {
        self.add(|usage| &mut usage.text_bytes, value.len())
    }

    fn optional_text(&mut self, value: Option<&str>) -> Result<(), PlotProjectionError> {
        value.map_or(Ok(()), |value| self.text(value))
    }

    fn strings<'a>(
        &mut self,
        values: impl IntoIterator<Item = &'a String>,
    ) -> Result<(), PlotProjectionError> {
        for value in values {
            self.text(value)?;
            self.add(|usage| &mut usage.nested_items, 1)?;
        }
        Ok(())
    }

    fn points(&mut self, count: usize) -> Result<(), PlotProjectionError> {
        self.add(|usage| &mut usage.points, count)?;
        self.add(|usage| &mut usage.nested_items, count)
    }

    fn items(&mut self, count: usize) -> Result<(), PlotProjectionError> {
        self.add(|usage| &mut usage.nested_items, count)
    }

    fn finish(mut self) -> Result<ProjectionUsage, PlotProjectionError> {
        // Every retained source string can coexist with JSON/contract copies.
        // Each nested item receives ample Value/map/vector allocation headroom.
        self.usage.materialized_bytes = 64_usize
            .checked_mul(1024)
            .and_then(|value| value.checked_add(self.usage.text_bytes.checked_mul(4)?))
            .and_then(|value| value.checked_add(self.usage.nested_items.checked_mul(4096)?))
            .ok_or_else(|| {
                resource_error("board plot projection materialized byte estimate overflowed")
            })?;
        Ok(self.usage)
    }
}

fn projection_usage(
    document: &crate::BoardPlotDocument,
    source_path: Option<&str>,
    document_id: &str,
) -> Result<ProjectionUsage, PlotProjectionError> {
    let mut counter = ProjectionCounter::default();
    counter.usage.records = document.records.len();
    counter.usage.operations = document
        .records
        .iter()
        .map(crate::BoardPlotRecord::operation_count)
        .try_fold(0_usize, |total, count| total.checked_add(count))
        .ok_or_else(|| resource_error("board plot projection operation count overflowed"))?;
    counter.items(
        document
            .records
            .len()
            .checked_add(counter.usage.operations)
            .ok_or_else(|| resource_error("board plot projection nested item count overflowed"))?,
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
) -> Result<(), PlotProjectionError> {
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
) -> Result<(), PlotProjectionError> {
    counter.optional_text(name)?;
    counter.optional_text(classes.net_class.as_deref())?;
    counter.strings(&classes.net_classes)
}

fn preflight_footprint(
    value: &BoardFootprintRecord,
    counter: &mut ProjectionCounter,
) -> Result<(), PlotProjectionError> {
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
) -> Result<(), PlotProjectionError> {
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
) -> Result<(), PlotProjectionError> {
    for operation in operations {
        preflight_operation(operation, counter)?;
    }
    Ok(())
}

fn preflight_operation(
    operation: &crate::PlotterOperation,
    counter: &mut ProjectionCounter,
) -> Result<(), PlotProjectionError> {
    use crate::PlotterOperation as Operation;
    let strings =
        |counter: &mut ProjectionCounter, layer: Option<&str>, colors: &[Option<&str>]| {
            counter.optional_text(layer)?;
            for color in colors {
                counter.optional_text(*color)?;
            }
            Ok::<(), PlotProjectionError>(())
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
) -> Result<(), PlotProjectionError> {
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

fn contract_record(record: CoreBoardPlotRecord) -> Result<BoardPlotRecord, PlotProjectionError> {
    Ok(match record {
        CoreBoardPlotRecord::Graphic(record) => BoardGraphicPlotRecord {
            kind: contract_record_kind(record.kind),
            layer: Some(record.layer),
            object_id: record.kind.as_str().to_owned(),
            operation_count: contract_count(record.operations.len())?,
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
            operation_count: contract_count(record.operations.len())?,
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
            operation_count: contract_count(record.operations.len())?,
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
            operation_count: contract_count(record.operations.len())?,
            operations: shared_operations(record.operations)?,
            uuid: record.uuid,
        }
        .into(),
    })
}

fn contract_footprint_record(
    record: BoardFootprintRecord,
) -> Result<BoardFootprintPlotRecord, PlotProjectionError> {
    let operation_count = contract_count(record.operations.len())?;
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
) -> Result<ContractBoardFootprintOperation, PlotProjectionError> {
    match operation {
        CoreBoardFootprintOperation::Geometry {
            operation,
            metadata,
        } => {
            let shared = contract_plotter_operation(index, operation)?;
            contract_enriched_footprint_operation(shared, Some(metadata))
        }
        CoreBoardFootprintOperation::Text {
            operation,
            metadata,
        } => {
            let text = contract_text_operation(index, operation)?;
            contract_enriched_footprint_operation(text.into(), Some(metadata))
        }
        CoreBoardFootprintOperation::Pad(operation) => {
            let shared = contract_plotter_operation(index, operation)?;
            contract_enriched_footprint_operation(shared, None)
        }
        CoreBoardFootprintOperation::StartBlock(block) => {
            let attrs = block.extra_attrs;
            Ok(board_contract::BoardFootprintStartBlockOperation {
                data_ref: enum_value(block.data_ref)?,
                data_uuid: block.data_uuid,
                extra_attrs: board_contract::BoardFootprintPadBlockAttrs {
                    component: attrs.component,
                    component_uid: attrs.component_uid,
                    component_uuid: attrs.component_uuid,
                    footprint: attrs.footprint,
                    hole_diameter_mm: attrs.hole_diameter_mm,
                    hole_height_mm: attrs.hole_height_mm,
                    hole_kind: optional_enum_value(attrs.hole_kind)?,
                    hole_owner: attrs.hole_owner,
                    hole_plating: optional_enum_value(attrs.hole_plating)?,
                    hole_render: attrs.hole_render,
                    hole_width_mm: attrs.hole_width_mm,
                    layer_names: attrs.layer_names,
                    net: attrs.net,
                    net_class: attrs.net_class,
                    net_classes: attrs.net_classes,
                    net_id: attrs.net_id,
                    net_index: attrs.net_index,
                    pad_designator: attrs.pad_designator,
                    pad_number: attrs.pad_number,
                    pad_shape: attrs.pad_shape,
                    pad_type: attrs.pad_type,
                    primitive: enum_value(attrs.primitive)?,
                },
                index: contract_count(index)?,
                kind: "StartBlock".to_owned(),
                label: block.label,
                layers: block.layers,
                object_id: block.object_id,
            }
            .into())
        }
        CoreBoardFootprintOperation::EndBlock => {
            Ok(board_contract::BoardFootprintEndBlockOperation {
                index: contract_count(index)?,
                kind: "EndBlock".to_owned(),
            }
            .into())
        }
    }
}

struct ContractFootprintChildMetadata {
    data_ref: Option<board_contract::BoardFootprintChildRef>,
    data_uuid: Option<String>,
    extra_attrs: Option<board_contract::BoardFootprintChildAttrs>,
    label: Option<String>,
    object_id: Option<String>,
}

fn contract_footprint_child_metadata(
    metadata: Option<BoardFootprintChildMetadata>,
) -> Result<ContractFootprintChildMetadata, PlotProjectionError> {
    let Some(metadata) = metadata else {
        return Ok(ContractFootprintChildMetadata {
            data_ref: None,
            data_uuid: None,
            extra_attrs: None,
            label: None,
            object_id: None,
        });
    };
    let attrs = metadata.extra_attrs;
    Ok(ContractFootprintChildMetadata {
        data_ref: Some(enum_value(metadata.data_ref)?),
        data_uuid: Some(metadata.data_uuid),
        extra_attrs: Some(board_contract::BoardFootprintChildAttrs {
            component: attrs.component,
            component_uid: attrs.component_uid,
            component_uuid: attrs.component_uuid,
            footprint: attrs.footprint,
            footprint_graphic_kind: optional_enum_value(attrs.footprint_graphic_kind)?,
            footprint_object_index: contract_count(attrs.footprint_object_index)?,
            footprint_primitive: enum_value(attrs.footprint_primitive)?,
            footprint_subop_index: attrs
                .footprint_subop_index
                .map(contract_count)
                .transpose()?,
            footprint_text_role: optional_enum_value(attrs.footprint_text_role)?,
            fp_text_type: attrs.fp_text_type,
            layer_name: attrs.layer_name,
            layer_role: optional_enum_value(attrs.layer_role)?,
            primitive: enum_value(attrs.primitive)?,
            property_name: attrs.property_name,
        }),
        label: Some(metadata.label),
        object_id: Some(metadata.object_id),
    })
}

macro_rules! enriched_operation {
    ($value:ident, $metadata:ident, $target:ident, $variant:ident, [$($field:ident),+ $(,)?]) => {
        ContractBoardFootprintOperation::$variant(board_contract::$target {
            $($field: $value.$field,)+
            data_ref: $metadata.data_ref,
            data_uuid: $metadata.data_uuid,
            extra_attrs: $metadata.extra_attrs,
            label: $metadata.label,
            object_id: $metadata.object_id,
        })
    };
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive frozen-contract union adapter"
)]
fn contract_enriched_footprint_operation(
    operation: PlotterOperation,
    metadata: Option<BoardFootprintChildMetadata>,
) -> Result<ContractBoardFootprintOperation, PlotProjectionError> {
    let metadata = contract_footprint_child_metadata(metadata)?;
    Ok(match operation {
        PlotterOperation::PlotImageOperation(_) => {
            return Err(invalid_model_error(
                "board embedded-footprint operations cannot contain PlotImage",
            ));
        }
        PlotterOperation::ThickSegmentOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintThickSegmentOperation,
            ThickSegmentOperation,
            [
                end_x,
                end_y,
                index,
                kind,
                layer,
                layers,
                mask_margin_nm,
                pad_size_x_nm,
                pad_size_y_nm,
                role,
                start_x,
                start_y,
                stroke_color,
                width_nm
            ]
        ),
        PlotterOperation::ArcThreePointOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintArcThreePointOperation,
            ArcThreePointOperation,
            [
                end_x,
                end_y,
                fill,
                fill_color,
                index,
                kind,
                layer,
                line_style,
                mid_x,
                mid_y,
                start_x,
                start_y,
                stroke_color,
                width_nm
            ]
        ),
        PlotterOperation::CircleOperation(value) => {
            let layers = value.layers.unwrap_or_default();
            ContractBoardFootprintOperation::CircleOperation(
                board_contract::BoardFootprintCircleOperation {
                    cx: value.cx,
                    cy: value.cy,
                    diameter_nm: value.diameter_nm,
                    fill: value.fill,
                    fill_color: value.fill_color,
                    index: value.index,
                    kind: value.kind,
                    layer: value.layer,
                    layers,
                    line_style: value.line_style,
                    mask_margin_nm: value.mask_margin_nm,
                    pad_size_x_nm: value.pad_size_x_nm,
                    pad_size_y_nm: value.pad_size_y_nm,
                    role: value.role,
                    stroke_color: value.stroke_color,
                    width_nm: value.width_nm,
                    data_ref: metadata.data_ref,
                    data_uuid: metadata.data_uuid,
                    extra_attrs: metadata.extra_attrs,
                    label: metadata.label,
                    object_id: metadata.object_id,
                },
            )
        }
        PlotterOperation::RectOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintRectOperation,
            RectOperation,
            [
                corner_radius_nm,
                fill,
                fill_color,
                index,
                kind,
                layer,
                line_style,
                stroke_color,
                width_nm,
                x1,
                x2,
                y1,
                y2
            ]
        ),
        PlotterOperation::PlotPolyOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintPlotPolyOperation,
            PlotPolyOperation,
            [
                fill,
                fill_color,
                index,
                kind,
                layer,
                line_style,
                points,
                stroke_color,
                width_nm
            ]
        ),
        PlotterOperation::BezierCurveOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintBezierCurveOperation,
            BezierCurveOperation,
            [
                ctrl1_x,
                ctrl1_y,
                ctrl2_x,
                ctrl2_y,
                end_x,
                end_y,
                index,
                kind,
                layer,
                line_style,
                start_x,
                start_y,
                stroke_color,
                tolerance_nm,
                width_nm
            ]
        ),
        PlotterOperation::TextOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintTextOperation,
            TextOperation,
            [
                bold,
                color,
                context,
                font_face,
                h_align,
                index,
                italic,
                kind,
                knockout,
                layer,
                mirror,
                multiline,
                orient_deg,
                pen_width_nm,
                polyline_per_segment,
                render_cache,
                render_cache_exact,
                render_cache_polygons,
                render_cache_source,
                size_x_nm,
                size_y_nm,
                text,
                text_as_polygons,
                v_align,
                x,
                y
            ]
        ),
        PlotterOperation::FlashPadCircleOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintFlashPadCircleOperation,
            FlashPadCircleOperation,
            [diameter_nm, index, kind, layers, mask_margin_nm, role, x, y]
        ),
        PlotterOperation::FlashPadOvalOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintFlashPadOvalOperation,
            FlashPadOvalOperation,
            [
                index,
                kind,
                layers,
                mask_margin_nm,
                orient_deg,
                size_x_nm,
                size_y_nm,
                x,
                y
            ]
        ),
        PlotterOperation::FlashPadRectOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintFlashPadRectOperation,
            FlashPadRectOperation,
            [
                index,
                kind,
                layers,
                mask_margin_nm,
                orient_deg,
                size_x_nm,
                size_y_nm,
                x,
                y
            ]
        ),
        PlotterOperation::FlashPadRoundRectOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintFlashPadRoundRectOperation,
            FlashPadRoundRectOperation,
            [
                corner_radius_nm,
                index,
                kind,
                layers,
                mask_margin_nm,
                orient_deg,
                size_x_nm,
                size_y_nm,
                x,
                y
            ]
        ),
        PlotterOperation::FlashPadCustomOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintFlashPadCustomOperation,
            FlashPadCustomOperation,
            [
                anchor_shape,
                index,
                kind,
                layers,
                mask_margin_nm,
                orient_deg,
                polygon_widths_nm,
                polygons,
                size_x_nm,
                size_y_nm,
                x,
                y
            ]
        ),
        PlotterOperation::FlashPadTrapezOperation(value) => enriched_operation!(
            value,
            metadata,
            BoardFootprintFlashPadTrapezOperation,
            FlashPadTrapezOperation,
            [
                corners,
                index,
                kind,
                layers,
                mask_margin_nm,
                orient_deg,
                x,
                y
            ]
        ),
    })
}

fn enum_value<T>(value: String) -> Result<T, PlotProjectionError>
where
    T: TryFrom<String>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| invalid_model_error(error.to_string()))
}

fn optional_enum_value<T>(value: Option<String>) -> Result<Option<T>, PlotProjectionError>
where
    T: TryFrom<String>,
    T::Error: std::fmt::Display,
{
    value.map(enum_value).transpose()
}

fn contract_dimension_record(
    record: BoardDimensionRecord,
) -> Result<DimensionPlotRecord, PlotProjectionError> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| match operation {
            BoardDimensionOperation::Geometry(operation) => {
                contract_plotter_operation(index, operation)
            }
            BoardDimensionOperation::Text(operation) => {
                contract_text_operation(index, operation).map(PlotterOperation::from)
            }
        })
        .collect::<Result<Vec<_>, PlotProjectionError>>()?;
    Ok(DimensionPlotRecord {
        dimension_type: match record.dimension_type.as_str() {
            "aligned" => BoardDimensionType::Aligned,
            "orthogonal" => BoardDimensionType::Orthogonal,
            "radial" => BoardDimensionType::Radial,
            "leader" => BoardDimensionType::Leader,
            "center" => BoardDimensionType::Center,
            _ => return Err(invalid_model_error("unsupported board dimension type")),
        },
        kind: "dimension".to_owned(),
        layers: record.layers,
        object_id: "dimension".to_owned(),
        operation_count: contract_count(operations.len())?,
        operations,
        text: record.text,
        uuid: record.uuid,
    })
}

fn contract_table_record(record: BoardTableRecord) -> Result<TablePlotRecord, PlotProjectionError> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| match operation {
            BoardTableOperation::Segment(operation) => contract_plotter_operation(index, operation),
            BoardTableOperation::Text(operation) => {
                contract_text_operation(index, operation).map(PlotterOperation::from)
            }
        })
        .collect::<Result<Vec<_>, PlotProjectionError>>()?;
    Ok(TablePlotRecord {
        cell_count: contract_count(record.cell_count)?,
        kind: "table".to_owned(),
        layers: record.layers,
        object_id: "table".to_owned(),
        operation_count: contract_count(operations.len())?,
        operations,
        uuid: record.uuid,
    })
}

fn contract_text_record(record: BoardTextRecord) -> Result<BoardPlotRecord, PlotProjectionError> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| {
            contract_text_operation(index, operation).map(PlotterOperation::from)
        })
        .collect::<Result<Vec<_>, PlotProjectionError>>()?;
    Ok(BoardTextPlotRecord {
        // Board gr_text carriers have no hide attribute upstream, so the
        // established serializer's getattr default is always false.
        hide: false,
        kind: "gr_text".to_owned(),
        layer: record.layer,
        object_id: "gr_text".to_owned(),
        operation_count: contract_count(operations.len())?,
        operations,
        text: record.text,
        uuid: record.uuid,
    }
    .into())
}

fn contract_text_box_record(
    record: BoardTextBoxRecord,
) -> Result<BoardPlotRecord, PlotProjectionError> {
    let operations = record
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| contract_text_box_operation(index, operation))
        .collect::<Result<Vec<_>, PlotProjectionError>>()?;
    Ok(BoardTextBoxPlotRecord {
        border: record.border,
        kind: "gr_text_box".to_owned(),
        layer: record.layer,
        object_id: "gr_text_box".to_owned(),
        operation_count: contract_count(operations.len())?,
        operations,
        text: record.text,
        uuid: record.uuid,
    }
    .into())
}

fn contract_text_box_operation(
    index: usize,
    operation: BoardTextBoxOperation,
) -> Result<PlotterOperation, PlotProjectionError> {
    match operation {
        BoardTextBoxOperation::Border(operation) => contract_plotter_operation(index, operation),
        BoardTextBoxOperation::Text(operation) => {
            contract_text_operation(index, operation).map(PlotterOperation::from)
        }
    }
}

fn contract_points(points: Vec<[i64; 2]>) -> Result<Vec<PlotterPoint>, PlotProjectionError> {
    points
        .into_iter()
        .map(|[x, y]| Ok(PlotterPoint([safe_integer(x)?, safe_integer(y)?])))
        .collect()
}

fn contract_text_operation(
    index: usize,
    operation: BoardTextOperation,
) -> Result<TextOperation, PlotProjectionError> {
    // The established emitter serializes marker keys only when true.
    let marker = |value: bool| value.then_some(true);
    let render_cache = operation
        .render_cache
        .map(|cache| -> Result<TextRenderCache, PlotProjectionError> {
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
                                PlotProjectionError,
                            >>(
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>, PlotProjectionError>>()?,
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
        index: contract_count(index)?,
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
            .collect::<Result<Vec<_>, PlotProjectionError>>()?,
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

fn contract_via_record(record: BoardViaRecord) -> Result<ViaPlotRecord, PlotProjectionError> {
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
        .collect::<Result<Vec<_>, PlotProjectionError>>()?;
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
        operation_count: contract_count(operations.len())?,
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
) -> Result<PlotterOperation, PlotProjectionError> {
    let index = contract_count(index)?;
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
            width_nm: safe_integer(0)?,
        }
        .into(),
    })
}

fn shared_operations(
    operations: Vec<crate::PlotterOperation>,
) -> Result<Vec<PlotterOperation>, PlotProjectionError> {
    // The established Python serializer numbers operations per record.
    operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| contract_plotter_operation(index, operation))
        .collect()
}

fn contract_count(count: usize) -> Result<u32, PlotProjectionError> {
    u32::try_from(count).map_err(|_| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::NumericRange,
            "board plot count or index exceeds uint32",
        )
    })
}

fn safe_integer(value: i64) -> Result<JavaScriptSafeInteger, PlotProjectionError> {
    JavaScriptSafeInteger::try_from(value).map_err(|error| {
        PlotProjectionError::new(PlotProjectionErrorKind::NumericRange, error.to_string())
    })
}

fn optional_safe_integer(
    value: Option<i64>,
) -> Result<Option<JavaScriptSafeInteger>, PlotProjectionError> {
    value.map(safe_integer).transpose()
}

fn resource_error(message: impl Into<String>) -> PlotProjectionError {
    PlotProjectionError::new(PlotProjectionErrorKind::ResourceLimit, message)
}

fn invalid_model_error(message: impl Into<String>) -> PlotProjectionError {
    PlotProjectionError::new(PlotProjectionErrorKind::InvalidModel, message)
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

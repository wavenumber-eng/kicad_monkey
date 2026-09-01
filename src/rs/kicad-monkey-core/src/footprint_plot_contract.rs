//! Typed standalone-footprint projection into the frozen a0 contract.

use crate::plot_document_contract::ProjectionUsage;
use crate::plotter_contract::contract_footprint_plotter_operation;
use crate::{
    FootprintPlotDocument, PlotDocumentMetadata, PlotDocumentProjectionLimits, PlotProjectionError,
    PlotProjectionErrorKind,
};
use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::footprint_plot_document::{
    FootprintPlotDocumentA0, FootprintPlotRecord, PlotterCoordinateSpace,
};
use kicad_monkey_contracts::validate_footprint_plot_document;

pub fn project_footprint_plot_document_a0(
    document: FootprintPlotDocument,
    metadata: PlotDocumentMetadata,
    limits: PlotDocumentProjectionLimits,
) -> Result<FootprintPlotDocumentA0, PlotProjectionError> {
    let mut usage = ProjectionUsage {
        records: 1,
        ..ProjectionUsage::default()
    };
    usage.add_string(&metadata.document_id)?;
    usage.add_optional_string(metadata.source_path.as_deref())?;
    usage.add_string(&document.name)?;
    usage.add_string(&document.generator)?;
    usage.add_string(&document.generator_version)?;
    usage.add_string(&document.uuid)?;
    usage.add_string(&document.layer)?;
    usage.add_string(&document.descr)?;
    usage.add_string(&document.tags)?;
    usage.add_strings(document.attr.iter())?;
    for operation in &document.operations {
        usage.add_operation(operation)?;
    }
    usage.enforce(limits)?;

    let total_operations = u32::try_from(document.operations.len()).map_err(|_| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::NumericRange,
            "footprint operation count exceeds uint32",
        )
    })?;
    let operations = document
        .operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| contract_footprint_plotter_operation(index, operation))
        .collect::<Result<Vec<_>, _>>()?;
    let contract = FootprintPlotDocumentA0 {
        coordinate_space: PlotterCoordinateSpace {
            unit: "nm".to_owned(),
            y_axis: "down".to_owned(),
        },
        document_id: metadata.document_id,
        generator: document.generator,
        generator_version: document.generator_version,
        records: vec![FootprintPlotRecord {
            attr: document.attr,
            descr: document.descr,
            kind: "footprint".to_owned(),
            layer: document.layer,
            locked: document.locked,
            name: document.name.clone(),
            object_id: document.name,
            operation_count: total_operations,
            operations,
            placed: document.placed,
            tags: document.tags,
            uuid: document.uuid,
        }],
        schema: "kicad.plotter_ir.a0".to_owned(),
        source_kind: "MOD".to_owned(),
        source_path: metadata.source_path,
        total_operations,
        version: JavaScriptSafeInteger::try_from(document.version).map_err(|error| {
            PlotProjectionError::new(PlotProjectionErrorKind::NumericRange, error.to_string())
        })?,
    };
    validate_footprint_plot_document(&contract).map_err(|error| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::ContractValidation,
            error.to_string(),
        )
    })?;
    assert_exhaustive_operation_union(&contract.records[0].operations);
    Ok(contract)
}

fn assert_exhaustive_operation_union(
    operations: &[kicad_monkey_contracts::generated::footprint_plot_document::PlotterOperation],
) {
    use kicad_monkey_contracts::generated::footprint_plot_document::PlotterOperation;
    for operation in operations {
        match operation {
            PlotterOperation::ThickSegmentOperation(_)
            | PlotterOperation::ArcThreePointOperation(_)
            | PlotterOperation::CircleOperation(_)
            | PlotterOperation::RectOperation(_)
            | PlotterOperation::PlotPolyOperation(_)
            | PlotterOperation::BezierCurveOperation(_)
            | PlotterOperation::TextOperation(_)
            | PlotterOperation::PlotImageOperation(_)
            | PlotterOperation::FlashPadCircleOperation(_)
            | PlotterOperation::FlashPadOvalOperation(_)
            | PlotterOperation::FlashPadRectOperation(_)
            | PlotterOperation::FlashPadRoundRectOperation(_)
            | PlotterOperation::FlashPadCustomOperation(_)
            | PlotterOperation::FlashPadTrapezOperation(_) => {}
        }
    }
}

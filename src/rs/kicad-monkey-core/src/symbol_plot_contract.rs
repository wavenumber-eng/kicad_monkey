//! Typed selected-library-symbol projection into the frozen a0 contract.

use crate::plot_document_contract::ProjectionUsage;
use crate::plotter_contract::contract_symbol_plotter_operation;
use crate::{
    PlotDocumentMetadata, PlotDocumentProjectionLimits, PlotProjectionError,
    PlotProjectionErrorKind, SymbolPlotDocument,
};
use kicad_monkey_contracts::generated::symbol_plot_document::{
    LibSubsymbolPlotRecord, PlotterCoordinateSpace, SymbolHeaderPlotRecord, SymbolPlotDocumentA0,
    SymbolPlotRecord,
};
use kicad_monkey_contracts::validate_symbol_plot_document;

pub fn project_symbol_plot_document_a0(
    document: SymbolPlotDocument,
    metadata: PlotDocumentMetadata,
    limits: PlotDocumentProjectionLimits,
) -> Result<SymbolPlotDocumentA0, PlotProjectionError> {
    let record_count = document.records.len().checked_add(1).ok_or_else(|| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::ResourceLimit,
            "symbol plot record count overflowed",
        )
    })?;
    let mut usage = ProjectionUsage {
        records: record_count,
        ..ProjectionUsage::default()
    };
    usage.add_string(&metadata.document_id)?;
    usage.add_optional_string(metadata.source_path.as_deref())?;
    usage.add_string(&document.name)?;
    usage.add_optional_string(document.extends.as_deref())?;
    for record in &document.records {
        usage.add_string(&record.name)?;
        for operation in &record.operations {
            usage.add_operation(operation)?;
        }
    }
    usage.enforce(limits)?;

    let total_operations = u32::try_from(usage.operations).map_err(|_| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::NumericRange,
            "symbol operation count exceeds uint32",
        )
    })?;
    let header = SymbolHeaderPlotRecord {
        extends: document.extends,
        in_bom: document.in_bom,
        kind: "lib_symbol".to_owned(),
        name: document.name.clone(),
        object_id: document.name.clone(),
        on_board: document.on_board,
        operation_count: 0,
        operations: Vec::new(),
        power: document.power,
        style: document.style,
        unit: document.unit,
        uuid: String::new(),
    };
    let mut records = Vec::with_capacity(record_count);
    records.push(SymbolPlotRecord::from(header));
    let mut next_index = 0usize;
    for record in document.records {
        let operation_count = u32::try_from(record.operations.len()).map_err(|_| {
            PlotProjectionError::new(
                PlotProjectionErrorKind::NumericRange,
                "symbol record operation count exceeds uint32",
            )
        })?;
        let mut operations = Vec::with_capacity(record.operations.len());
        for operation in record.operations {
            operations.push(contract_symbol_plotter_operation(next_index, operation)?);
            next_index = next_index.checked_add(1).ok_or_else(|| {
                PlotProjectionError::new(
                    PlotProjectionErrorKind::NumericRange,
                    "symbol plot operation index overflowed",
                )
            })?;
        }
        records.push(
            LibSubsymbolPlotRecord {
                kind: "lib_subsymbol".to_owned(),
                object_id: record.name,
                operation_count,
                operations,
                style: record.style,
                unit: record.unit,
                uuid: String::new(),
            }
            .into(),
        );
    }
    let contract = SymbolPlotDocumentA0 {
        coordinate_space: PlotterCoordinateSpace {
            unit: "nm".to_owned(),
            y_axis: "down".to_owned(),
        },
        document_id: metadata.document_id,
        records,
        schema: "kicad.plotter_ir.a0".to_owned(),
        source_kind: "SYM".to_owned(),
        source_path: metadata.source_path,
        total_operations,
    };
    validate_symbol_plot_document(&contract).map_err(|error| {
        PlotProjectionError::new(
            PlotProjectionErrorKind::ContractValidation,
            error.to_string(),
        )
    })?;
    assert_exhaustive_operation_union(&contract.records);
    Ok(contract)
}

fn assert_exhaustive_operation_union(records: &[SymbolPlotRecord]) {
    use kicad_monkey_contracts::generated::symbol_plot_document::PlotterOperation;
    for operation in records.iter().flat_map(|record| match record {
        SymbolPlotRecord::SymbolHeaderPlotRecord(value) => value.operations.iter(),
        SymbolPlotRecord::LibSubsymbolPlotRecord(value) => value.operations.iter(),
    }) {
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

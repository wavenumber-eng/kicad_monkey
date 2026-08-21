use super::CompiledOccurrence;
use super::bus_promotion::{BusCompilationBudget, index_bus_coords, index_bus_member_wires};
use super::types::SchematicDesignNetLimits;
use crate::schematic_connectivity::build_schematic_occurrence_design_subgraphs;
use crate::{
    SchematicBundleIndex, SchematicOccurrence, SchematicSubpartSettings, SchematicWireSubgraph,
    SourceBundleError, SourceBundleErrorKind,
};
use std::collections::HashMap;

pub(super) struct DesignCompilationBudget {
    subgraphs: usize,
    indexed_coords: usize,
    bus: BusCompilationBudget,
}

impl DesignCompilationBudget {
    pub(super) fn new() -> Self {
        Self {
            subgraphs: 0,
            indexed_coords: 0,
            bus: BusCompilationBudget::new(),
        }
    }

    pub(super) fn subgraph_count(&self) -> usize {
        self.subgraphs
    }

    fn add_wire_subgraphs(
        &mut self,
        subgraphs: &[SchematicWireSubgraph],
        limits: SchematicDesignNetLimits,
        source_path: &str,
    ) -> Result<usize, SourceBundleError> {
        self.subgraphs = checked_total(
            self.subgraphs,
            subgraphs.len(),
            limits.max_subgraphs,
            source_path,
            "design subgraph count",
        )?;
        let occurrence_coords = subgraphs.iter().try_fold(0_usize, |count, subgraph| {
            count
                .checked_add(subgraph.coords.len())
                .ok_or_else(|| limit_error(source_path, "indexed coordinate count overflows"))
        })?;
        self.indexed_coords = checked_total(
            self.indexed_coords,
            occurrence_coords,
            limits.max_indexed_coords,
            source_path,
            "indexed coordinate count",
        )?;
        Ok(occurrence_coords)
    }
}

pub(super) fn compile_design_occurrence<'a>(
    index: &'a SchematicBundleIndex,
    occurrence: &SchematicOccurrence,
    subparts: SchematicSubpartSettings,
    limits: SchematicDesignNetLimits,
    aliases: &HashMap<&'a str, &'a [String]>,
    budget: &mut DesignCompilationBudget,
) -> Result<CompiledOccurrence, SourceBundleError> {
    let design_subgraphs = build_schematic_occurrence_design_subgraphs(
        index,
        occurrence.index,
        subparts,
        true,
        limits.connectivity,
        aliases,
    )?;
    let subgraphs = design_subgraphs.wire_subgraphs;
    let bus_subgraphs = design_subgraphs.bus_subgraphs;
    let occurrence_coords =
        budget.add_wire_subgraphs(&subgraphs, limits, &occurrence.source_path)?;
    let mut coord_to_subgraph = HashMap::with_capacity(occurrence_coords);
    for (subgraph_index, subgraph) in subgraphs.iter().enumerate() {
        for point in &subgraph.coords {
            coord_to_subgraph.entry(*point).or_insert(subgraph_index);
        }
    }
    budget
        .bus
        .add_occurrence(&bus_subgraphs, limits, &occurrence.source_path)?;
    let bus_coord_to_subgraph = index_bus_coords(&bus_subgraphs);
    let bus_member_wire_subgraphs = index_bus_member_wires(
        &subgraphs,
        &bus_subgraphs,
        &coord_to_subgraph,
        limits,
        &occurrence.source_path,
    )?;
    Ok(CompiledOccurrence {
        occurrence_index: occurrence.index,
        source_path: occurrence.source_path.clone(),
        human_address: occurrence.human_address.clone(),
        legacy_address: occurrence.legacy_address.clone(),
        subgraphs,
        coord_to_subgraph,
        bus_subgraphs,
        bus_coord_to_subgraph,
        bus_member_wire_subgraphs,
    })
}

fn checked_total(
    current: usize,
    added: usize,
    maximum: usize,
    source_path: &str,
    family: &str,
) -> Result<usize, SourceBundleError> {
    let total = current
        .checked_add(added)
        .ok_or_else(|| limit_error(source_path, &format!("{family} overflows")))?;
    if total > maximum {
        return Err(limit_error(
            source_path,
            &format!("{family} exceeds its limit"),
        ));
    }
    Ok(total)
}

fn limit_error(path: &str, message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, Some(path), message)
}

use super::{KiCadDesignJsonError, KiCadDesignJsonLimits, KiCadDesignJsonPaths, KiCadDesignPcb};
use crate::{KiCadNet, KiCadNetlist, ProjectView, SchematicBundleIndex};
use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use serde::Serialize;
use std::io::{self, Write};
use std::mem::size_of;

const OUTPUT_TO_MODEL_RATIO: usize = 32;
const VALUE_ALLOCATION_MULTIPLIER: usize = 4;
const GRAPH_ROOT_VALUES: usize = 16;
const GRAPH_VALUES_PER_ROW: usize = 24;

#[allow(
    clippy::too_many_arguments,
    reason = "the preflight accounts for every independently owned presenter input"
)]
pub(super) fn preflight_design_json(
    index: &SchematicBundleIndex,
    project: Option<ProjectView<'_>>,
    netlist: &KiCadNetlist,
    graph: &CompiledSchematicGraphA0,
    paths: &KiCadDesignJsonPaths,
    pcb: Option<KiCadDesignPcb<'_>>,
    include_indexes: bool,
    limits: KiCadDesignJsonLimits,
) -> Result<(), KiCadDesignJsonError> {
    let output_bound = limits
        .max_output_bytes
        .saturating_mul(OUTPUT_TO_MODEL_RATIO);
    let mut budget = PreflightBudget::new(
        limits.max_derived_items,
        limits.max_materialized_bytes.min(output_bound),
    );
    budget.items(96)?;
    budget.optional_text(paths.project_name.as_deref(), 1)?;
    budget.optional_text(paths.project_filename.as_deref(), 1)?;
    budget.optional_text(paths.project_path.as_deref(), 1)?;
    for (source_path, source) in &paths.schematic_paths {
        budget.items(3)?;
        budget.text(source_path, 1)?;
        budget.text(&source.filename, 2)?;
        budget.text(&source.path, 2)?;
    }
    preflight_project(index, project, &mut budget)?;
    preflight_hierarchy(index, &mut budget)?;
    preflight_netlist(netlist, include_indexes, &mut budget)?;
    preflight_graph(graph, &mut budget)?;
    if let Some(pcb) = pcb {
        preflight_pcb(pcb, &mut budget)?;
    }
    Ok(())
}

fn preflight_graph(
    graph: &CompiledSchematicGraphA0,
    budget: &mut PreflightBudget,
) -> Result<(), KiCadDesignJsonError> {
    budget.items(graph_item_count(graph)?)?;
    budget.serialized(graph)
}

fn graph_item_count(graph: &CompiledSchematicGraphA0) -> Result<usize, KiCadDesignJsonError> {
    let rows = [
        graph.component_occurrences.len(),
        graph.graphical_artifact_links.len(),
        graph.hierarchy_occurrences.len(),
        graph.hierarchy_terminal_bindings.len(),
        graph.local_net_occurrences.len(),
        graph.page_definitions.len(),
        graph.page_occurrences.len(),
        graph.terminal_occurrences.len(),
        graph.unit_definitions.len(),
        graph.unit_occurrences.len(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .and_then(|count| count.checked_mul(GRAPH_VALUES_PER_ROW))
    .and_then(|count| count.checked_add(GRAPH_ROOT_VALUES))
    .ok_or_else(graph_budget_overflow)?;
    let nested = graph
        .local_net_occurrences
        .iter()
        .map(|row| row.aliases.len())
        .chain(
            graph
                .terminal_occurrences
                .iter()
                .map(|row| row.resolution_diagnostics.len()),
        )
        .chain(
            graph
                .unit_definitions
                .iter()
                .map(|row| row.page_definition_refs.len()),
        )
        .chain(
            graph
                .unit_occurrences
                .iter()
                .map(|row| row.page_occurrence_refs.len()),
        )
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(graph_budget_overflow)?;
    rows.checked_add(nested).ok_or_else(graph_budget_overflow)
}

fn graph_budget_overflow() -> KiCadDesignJsonError {
    KiCadDesignJsonError::context(
        "could not preflight design JSON",
        "compiled graph item count overflow",
    )
}

fn preflight_project(
    index: &SchematicBundleIndex,
    project: Option<ProjectView<'_>>,
    budget: &mut PreflightBudget,
) -> Result<(), KiCadDesignJsonError> {
    let variables = project
        .map(|view| view.text_variables())
        .transpose()
        .map_err(|error| KiCadDesignJsonError::context("could not read text variables", error))?
        .unwrap_or_default();
    budget.items(variables.len().saturating_mul(2))?;
    for (name, value) in variables {
        budget.text(&name, 1)?;
        budget.text(&value, 1)?;
    }
    let variants = project
        .map(|view| view.variants())
        .transpose()
        .map_err(|error| KiCadDesignJsonError::context("could not read variants", error))?
        .unwrap_or_default();
    budget.items(variants.len().saturating_mul(4))?;
    for variant in variants {
        budget.text(&variant.name, 2)?;
        budget.optional_text(variant.description.as_deref(), 2)?;
        for occurrence in index.occurrences() {
            let effective = index.effective_symbols(occurrence.index, Some(&variant.name))?;
            budget.items(effective.len())?;
            for symbol in effective {
                budget.text(&symbol.reference, 2)?;
                budget.text(&symbol.value, 1)?;
                budget.items(symbol.fields.len().saturating_mul(2))?;
                for (name, value) in symbol.fields {
                    budget.text(&name, 1)?;
                    budget.text(&value, 1)?;
                }
            }
        }
    }
    Ok(())
}

fn preflight_hierarchy(
    index: &SchematicBundleIndex,
    budget: &mut PreflightBudget,
) -> Result<(), KiCadDesignJsonError> {
    budget.items(index.definitions().len())?;
    budget.items(index.occurrences().len().saturating_mul(4))?;
    for definition in index.definitions() {
        budget.text(&definition.source_path, 2)?;
        budget.optional_text(definition.uuid.as_deref(), 2)?;
        budget.items(definition.sheets.len().saturating_mul(3))?;
        for sheet in &definition.sheets {
            budget.text(&sheet.uuid, 4)?;
            budget.text(&sheet.sheet_name, 3)?;
            budget.text(&sheet.sheet_file, 3)?;
            budget.items(sheet.pins.len().saturating_mul(2))?;
            for pin in &sheet.pins {
                budget.text(&pin.uuid, 2)?;
                budget.text(&pin.name, 2)?;
            }
        }
    }
    Ok(())
}

fn preflight_netlist(
    netlist: &KiCadNetlist,
    include_indexes: bool,
    budget: &mut PreflightBudget,
) -> Result<(), KiCadDesignJsonError> {
    budget.items(netlist.components.len().saturating_mul(8))?;
    budget.items(netlist.sheets.len().saturating_mul(2))?;
    budget.items(netlist.net_classes.len().saturating_mul(3))?;
    for component in &netlist.components {
        for value in [
            &component.reference,
            &component.value,
            &component.footprint,
            &component.datasheet,
            &component.description,
            &component.libsource_lib,
            &component.libsource_part,
            &component.libsource_description,
            &component.sheet_path_names,
            &component.sheet_path_uuids,
        ] {
            budget.text(value, 4)?;
        }
        budget.items(
            component
                .fields
                .len()
                .saturating_add(component.properties.len())
                .saturating_mul(4),
        )?;
        for (name, value) in component.fields.iter().chain(&component.properties) {
            budget.text(name, 4)?;
            budget.text(value, 4)?;
        }
        for uuid in &component.instance_uuids {
            budget.text(uuid, 4)?;
        }
        for unit in &component.units {
            budget.items(unit.pins.len().saturating_add(1))?;
            budget.text(&unit.name, 1)?;
            for pin in &unit.pins {
                budget.text(pin, 1)?;
            }
        }
    }
    for net in &netlist.nets {
        preflight_net(net, include_indexes, budget)?;
    }
    for sheet in &netlist.sheets {
        for value in [
            &sheet.name,
            &sheet.tstamps,
            &sheet.title,
            &sheet.company,
            &sheet.revision,
            &sheet.date,
        ] {
            budget.text(value, 2)?;
        }
    }
    for class in &netlist.net_classes {
        budget.text(&class.name, 3)?;
        budget.text(&class.description, 1)?;
    }
    Ok(())
}

fn preflight_net(
    net: &KiCadNet,
    include_indexes: bool,
    budget: &mut PreflightBudget,
) -> Result<(), KiCadDesignJsonError> {
    budget.items(8)?;
    budget.text(&net.name, if include_indexes { 12 } else { 4 })?;
    budget.text(&net.driver_kind, 1)?;
    budget.text(&net.net_class, 2)?;
    budget.items(net.aliases.len())?;
    for alias in &net.aliases {
        budget.text(alias, 1)?;
    }
    let graphics = net
        .graphical
        .wires
        .iter()
        .chain(&net.graphical.junctions)
        .chain(&net.graphical.labels)
        .chain(&net.graphical.power_ports)
        .chain(&net.graphical.ports)
        .chain(&net.graphical.sheet_entries);
    for value in graphics {
        budget.items(if include_indexes { 6 } else { 1 })?;
        budget.text(value, if include_indexes { 6 } else { 1 })?;
    }
    for terminal in &net.terminals {
        budget.items(if include_indexes { 12 } else { 3 })?;
        for value in [
            &terminal.designator,
            &terminal.pin,
            &terminal.pin_name,
            &terminal.pin_type,
            &terminal.sheet_path,
            &terminal.source_pin_id,
            &terminal.svg_id,
        ] {
            budget.text(value, if include_indexes { 6 } else { 2 })?;
        }
    }
    for endpoint in &net.endpoints {
        budget.items(if include_indexes { 8 } else { 2 })?;
        for value in [
            &endpoint.endpoint_id,
            &endpoint.role,
            &endpoint.element_id,
            &endpoint.object_id,
            &endpoint.name,
            &endpoint.source_sheet,
        ] {
            budget.text(value, if include_indexes { 4 } else { 1 })?;
        }
    }
    Ok(())
}

fn preflight_pcb(
    pcb: KiCadDesignPcb<'_>,
    budget: &mut PreflightBudget,
) -> Result<(), KiCadDesignJsonError> {
    let counts = pcb.view.counts();
    budget.items(counts.footprints.saturating_mul(3))?;
    budget.items(counts.footprint_properties.saturating_mul(2))?;
    budget.text(pcb.source_filename, 1)?;
    for footprint in pcb.view.footprints() {
        let footprint = footprint.map_err(|error| {
            KiCadDesignJsonError::context("could not preflight PCB footprint", error)
        })?;
        for value in [
            footprint.reference.as_deref(),
            footprint.value.as_deref(),
            footprint.layer.as_deref(),
            footprint.uuid.as_deref(),
        ] {
            budget.optional_text(value, 2)?;
        }
        budget.text(&footprint.library_link, 2)?;
        budget.text(&footprint.description, 2)?;
    }
    for property in pcb.view.footprint_properties() {
        let property = property.map_err(|error| {
            KiCadDesignJsonError::context("could not preflight PCB footprint property", error)
        })?;
        budget.text(&property.name, 1)?;
        budget.text(&property.value, 1)?;
    }
    Ok(())
}

struct PreflightBudget {
    items: usize,
    bytes: usize,
    max_items: usize,
    max_bytes: usize,
}

impl PreflightBudget {
    const fn new(max_items: usize, max_bytes: usize) -> Self {
        Self {
            items: 0,
            bytes: 0,
            max_items,
            max_bytes,
        }
    }

    fn items(&mut self, count: usize) -> Result<(), KiCadDesignJsonError> {
        self.items = self.items.checked_add(count).ok_or_else(|| {
            KiCadDesignJsonError::context("could not preflight design JSON", "item count overflow")
        })?;
        if self.items > self.max_items {
            return Err(KiCadDesignJsonError::context(
                "could not preflight design JSON",
                format!(
                    "derived item limit exceeded: {} > {}",
                    self.items, self.max_items
                ),
            ));
        }
        let allocation_bytes = count
            .checked_mul(size_of::<serde_json::Value>())
            .and_then(|bytes| bytes.checked_mul(VALUE_ALLOCATION_MULTIPLIER))
            .ok_or_else(|| {
                KiCadDesignJsonError::context(
                    "could not preflight design JSON",
                    "item allocation byte count overflow",
                )
            })?;
        self.bytes(allocation_bytes)
    }

    fn text(&mut self, value: &str, copies: usize) -> Result<(), KiCadDesignJsonError> {
        let bytes = value.len().checked_mul(copies).ok_or_else(|| {
            KiCadDesignJsonError::context(
                "could not preflight design JSON",
                "text allocation byte count overflow",
            )
        })?;
        self.bytes(bytes)
    }

    fn optional_text(
        &mut self,
        value: Option<&str>,
        copies: usize,
    ) -> Result<(), KiCadDesignJsonError> {
        value.map_or(Ok(()), |value| self.text(value, copies))
    }

    fn serialized(&mut self, value: &impl Serialize) -> Result<(), KiCadDesignJsonError> {
        let mut writer = CountingWriter::default();
        serde_json::to_writer(&mut writer, value).map_err(|error| {
            KiCadDesignJsonError::context("could not preflight serialized model", error)
        })?;
        self.bytes(writer.written)
    }

    fn bytes(&mut self, count: usize) -> Result<(), KiCadDesignJsonError> {
        self.bytes = self.bytes.checked_add(count).ok_or_else(|| {
            KiCadDesignJsonError::context(
                "could not preflight design JSON",
                "materialized byte count overflow",
            )
        })?;
        if self.bytes > self.max_bytes {
            return Err(KiCadDesignJsonError::context(
                "could not preflight design JSON",
                format!(
                    "materialized byte limit exceeded: {} > {}",
                    self.bytes, self.max_bytes
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct CountingWriter {
    written: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.written = self
            .written
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("serialized byte count overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GRAPH_VALUES_PER_ROW, PreflightBudget, VALUE_ALLOCATION_MULTIPLIER, graph_item_count,
        preflight_graph,
    };
    use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
    use serde_json::json;

    #[test]
    fn model_budget_accepts_exact_and_rejects_one_over() {
        let item_bytes = std::mem::size_of::<serde_json::Value>() * VALUE_ALLOCATION_MULTIPLIER;
        let mut exact = PreflightBudget::new(2, item_bytes * 2 + 3);
        exact.items(2).expect("exact items");
        exact.text("abc", 1).expect("exact bytes");
        assert!(exact.items(1).is_err());

        let mut one_over = PreflightBudget::new(2, item_bytes * 2 + 2);
        one_over.items(2).expect("item allocation");
        assert!(one_over.text("abc", 1).is_err());
    }

    #[test]
    fn graph_heavy_budget_rejects_before_value_materialization() {
        let row = json!({
            "aliases": ["A", "B"],
            "display_name": "N",
            "id": "net",
            "page_occurrence_ref": "page",
            "source_identity": {},
            "type": "sch.local_net_occurrence",
        });
        let graph: CompiledSchematicGraphA0 = serde_json::from_value(json!({
            "component_occurrences": [],
            "graphical_artifact_links": [],
            "hierarchy_occurrences": [],
            "hierarchy_terminal_bindings": [],
            "identity_namespace": "sch.compiled_schematic_graph.a0",
            "local_net_occurrences": vec![row; 128],
            "page_definitions": [],
            "page_occurrences": [],
            "schema": "kicad_monkey.compiled_schematic_graph.a0",
            "terminal_occurrences": [],
            "type": "sch.compiled_schematic_graph",
            "unit_definitions": [],
            "unit_occurrences": [],
        }))
        .expect("graph fixture");
        let items = graph_item_count(&graph).expect("graph item count");
        assert!(items >= 128 * GRAPH_VALUES_PER_ROW);
        let serialized_bytes = serde_json::to_vec(&graph).unwrap().len();
        let allocation_bytes =
            items * std::mem::size_of::<serde_json::Value>() * VALUE_ALLOCATION_MULTIPLIER;

        let mut exact = PreflightBudget::new(items, allocation_bytes + serialized_bytes);
        preflight_graph(&graph, &mut exact).expect("exact graph budget");

        let mut item_under = PreflightBudget::new(items - 1, usize::MAX);
        assert!(preflight_graph(&graph, &mut item_under).is_err());
        let mut byte_under = PreflightBudget::new(items, allocation_bytes + serialized_bytes - 1);
        assert!(preflight_graph(&graph, &mut byte_under).is_err());
    }
}

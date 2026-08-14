use super::{
    CompiledGraphIdentityAllocator, IdentityMapping, compiled_schematic_graph_design_scope,
    validate_compiled_schematic_graph,
};
use crate::{
    SchematicBundleIndex, SchematicDefinition, SchematicOccurrence, SchematicPlacedSymbol,
    SourceBundleError, SourceBundleErrorKind,
};
use kicad_monkey_contracts::generated::compiled_schematic_graph::{
    CompiledSchematicGraphA0, ComponentOccurrence, GraphicalArtifactLink, GraphicalTargetType,
    HierarchyOccurrence, PageDefinition, PageOccurrence, SourceIdentity, UnitDefinition,
    UnitOccurrence,
};
use serde_json::Value;
use std::collections::HashMap;
use std::num::NonZeroU64;
mod connectivity;

const GRAPH_SCHEMA: &str = "kicad_monkey.compiled_schematic_graph.a0";
const GRAPH_TYPE: &str = "sch.compiled_schematic_graph";
const IDENTITY_NAMESPACE: &str = "sch.compiled_schematic_graph.a0";
const DRAWING_ARTIFACT_KEY: &str = "sch.dwg_scene";
const UUID_TEXT_BYTES: usize = 36;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledSchematicGraphLimits {
    pub design: crate::SchematicDesignNetLimits,
    pub max_unit_definitions: usize,
    pub max_page_definitions: usize,
    pub max_unit_occurrences: usize,
    pub max_page_occurrences: usize,
    pub max_hierarchy_occurrences: usize,
    pub max_component_occurrences: usize,
    pub max_local_net_occurrences: usize,
    pub max_terminal_occurrences: usize,
    pub max_hierarchy_terminal_bindings: usize,
    pub max_graphical_artifact_links: usize,
    pub max_retained_string_bytes: usize,
}

impl Default for CompiledSchematicGraphLimits {
    fn default() -> Self {
        Self {
            design: crate::SchematicDesignNetLimits::default(),
            max_unit_definitions: 1_000_000,
            max_page_definitions: 1_000_000,
            max_unit_occurrences: 8_000_000,
            max_page_occurrences: 8_000_000,
            max_hierarchy_occurrences: 8_000_000,
            max_component_occurrences: 16_000_000,
            max_local_net_occurrences: 16_000_000,
            max_terminal_occurrences: 32_000_000,
            max_hierarchy_terminal_bindings: 8_000_000,
            max_graphical_artifact_links: 64_000_000,
            max_retained_string_bytes: 4_usize.saturating_mul(1024 * 1024 * 1024),
        }
    }
}

pub fn build_compiled_schematic_graph(
    index: &SchematicBundleIndex,
    limits: CompiledSchematicGraphLimits,
) -> Result<CompiledSchematicGraphA0, SourceBundleError> {
    StructuralGraphBuilder::new(index, limits)?.build()
}

#[derive(Clone)]
struct DefinitionRefs {
    unit: String,
    page: String,
}

#[derive(Clone)]
struct OccurrenceRefs {
    unit: String,
    page: String,
}

struct StructuralGraphBuilder<'a> {
    index: &'a SchematicBundleIndex,
    limits: CompiledSchematicGraphLimits,
    budget: RetainedStringBudget,
    allocator: CompiledGraphIdentityAllocator,
    graph: CompiledSchematicGraphA0,
    definitions: HashMap<&'a str, DefinitionRefs>,
    occurrences: Vec<OccurrenceRefs>,
    component_by_symbol: HashMap<usize, HashMap<String, String>>,
}

impl<'a> StructuralGraphBuilder<'a> {
    fn new(
        index: &'a SchematicBundleIndex,
        limits: CompiledSchematicGraphLimits,
    ) -> Result<Self, SourceBundleError> {
        let scope = compiled_schematic_graph_design_scope("kicad", index.project_file())
            .map_err(identity_error)?;
        let allocator = CompiledGraphIdentityAllocator::new(&scope).map_err(identity_error)?;
        let graph = CompiledSchematicGraphA0 {
            component_occurrences: Vec::new(),
            graphical_artifact_links: Vec::new(),
            hierarchy_occurrences: Vec::new(),
            hierarchy_terminal_bindings: Vec::new(),
            identity_namespace: IDENTITY_NAMESPACE.to_owned(),
            local_net_occurrences: Vec::new(),
            page_definitions: Vec::new(),
            page_occurrences: Vec::new(),
            schema: GRAPH_SCHEMA.to_owned(),
            terminal_occurrences: Vec::new(),
            type_: GRAPH_TYPE.to_owned(),
            unit_definitions: Vec::new(),
            unit_occurrences: Vec::new(),
        };
        let mut budget = RetainedStringBudget::new(limits.max_retained_string_bytes);
        budget.reserve([
            GRAPH_SCHEMA.len(),
            GRAPH_TYPE.len(),
            IDENTITY_NAMESPACE.len(),
        ])?;
        Ok(Self {
            index,
            limits,
            budget,
            allocator,
            graph,
            definitions: HashMap::new(),
            occurrences: Vec::with_capacity(index.occurrences().len()),
            component_by_symbol: HashMap::new(),
        })
    }

    fn build(mut self) -> Result<CompiledSchematicGraphA0, SourceBundleError> {
        let mut connectivity = self.prepare_connectivity()?;
        for (position, occurrence) in self.index.occurrences().enumerate() {
            let definition = self.definition(occurrence)?;
            let definition_refs = self.ensure_definition(definition, occurrence)?;
            let occurrence_refs =
                self.append_occurrence(occurrence, definition, &definition_refs)?;
            if occurrence.parent_index.is_some() {
                self.append_hierarchy(occurrence, &occurrence_refs)?;
            }
            self.append_components(occurrence, definition, &occurrence_refs)?;
            self.append_aggregate_drawing_links(definition, &occurrence_refs)?;
            self.occurrences.push(occurrence_refs);
            self.append_compiled_occurrence_connectivity(position, &mut connectivity)?;
        }
        self.finish_connectivity(connectivity)?;
        validate_compiled_schematic_graph(&self.graph)
            .map_err(|error| graph_error(error.to_string()))?;
        Ok(self.graph)
    }

    fn definition(
        &self,
        occurrence: &SchematicOccurrence,
    ) -> Result<&'a SchematicDefinition, SourceBundleError> {
        self.index
            .definition(&occurrence.source_path)
            .ok_or_else(|| {
                missing_error(
                    &occurrence.source_path,
                    "graph source definition is missing",
                )
            })
    }

    fn ensure_definition(
        &mut self,
        definition: &'a SchematicDefinition,
        occurrence: &SchematicOccurrence,
    ) -> Result<DefinitionRefs, SourceBundleError> {
        if let Some(existing) = self.definitions.get(definition.source_path.as_str()) {
            return Ok(existing.clone());
        }
        ensure_capacity(
            self.graph.unit_definitions.len(),
            self.limits.max_unit_definitions,
            "compiled graph unit-definition count",
        )?;
        ensure_capacity(
            self.graph.page_definitions.len(),
            self.limits.max_page_definitions,
            "compiled graph page-definition count",
        )?;
        let source_path = self.index.portable_source_path(&definition.source_path);
        let source_uuid = definition.uuid.as_deref().unwrap_or_default();
        let display_name = portable_stem(source_path).unwrap_or(&occurrence.sheet_name);
        self.budget.reserve([
            "sch.unit_definition".len(),
            "sch.page_definition".len(),
            UUID_TEXT_BYTES * 4,
            display_name.len() * 2,
            source_path.len() * 2,
            source_uuid.len() * 2,
        ])?;
        let source_identity = source_identity(source_path, source_uuid, "", "", "", "");
        let source_mapping = source_identity_mapping(&source_identity);
        let unit_id = self
            .allocator
            .allocate_source("sch.unit_definition", &source_mapping, &[])
            .map_err(identity_error)?;
        let page_id = self
            .allocator
            .allocate_source(
                "sch.page_definition",
                &source_mapping,
                std::slice::from_ref(&unit_id),
            )
            .map_err(identity_error)?;
        self.graph.unit_definitions.push(UnitDefinition {
            display_name: display_name.to_owned(),
            id: unit_id.clone(),
            page_definition_refs: vec![page_id.clone()],
            source_identity: source_identity.clone(),
            type_: "sch.unit_definition".to_owned(),
        });
        self.graph.page_definitions.push(PageDefinition {
            display_name: display_name.to_owned(),
            id: page_id.clone(),
            source_identity,
            type_: "sch.page_definition".to_owned(),
            unit_definition_ref: unit_id.clone(),
        });
        let refs = DefinitionRefs {
            unit: unit_id,
            page: page_id,
        };
        self.definitions
            .insert(definition.source_path.as_str(), refs.clone());
        Ok(refs)
    }

    fn append_occurrence(
        &mut self,
        occurrence: &SchematicOccurrence,
        source_definition: &SchematicDefinition,
        definition: &DefinitionRefs,
    ) -> Result<OccurrenceRefs, SourceBundleError> {
        ensure_capacity(
            self.graph.unit_occurrences.len(),
            self.limits.max_unit_occurrences,
            "compiled graph unit-occurrence count",
        )?;
        ensure_capacity(
            self.graph.page_occurrences.len(),
            self.limits.max_page_occurrences,
            "compiled graph page-occurrence count",
        )?;
        let source_uuid = self.occurrence_source_uuid(occurrence)?;
        let source_record = format!("instance-path:{}", occurrence.occurrence_address);
        let address_key = format!("sheet{}", occurrence.index);
        let sheet_number = occurrence.index.to_string();
        let display_name = if occurrence.parent_index.is_none() {
            portable_stem(
                self.index
                    .portable_source_path(&source_definition.source_path),
            )
            .unwrap_or(&occurrence.sheet_name)
        } else {
            &occurrence.sheet_name
        };
        self.budget.reserve([
            "sch.unit_occurrence".len(),
            "sch.page_occurrence".len(),
            UUID_TEXT_BYTES * 6,
            display_name.len() * 2,
            occurrence.legacy_address.len() * 2,
            source_record.len() * 2,
            source_uuid.len() * 2,
            address_key.len(),
            sheet_number.len(),
        ])?;
        let source_identity = source_identity(
            &occurrence.legacy_address,
            &source_uuid,
            &source_record,
            "",
            "",
            "",
        );
        let source_mapping = source_identity_mapping(&source_identity);
        let unit_id = self
            .allocator
            .allocate_source("sch.unit_occurrence", &source_mapping, &[])
            .map_err(identity_error)?;
        let page_id = self
            .allocator
            .allocate_source(
                "sch.page_occurrence",
                &source_mapping,
                std::slice::from_ref(&unit_id),
            )
            .map_err(identity_error)?;
        let instance_order = u32::try_from(occurrence.index.saturating_sub(1))
            .map_err(|_| graph_error("compiled graph instance order exceeds uint32"))?;
        self.graph.unit_occurrences.push(UnitOccurrence {
            display_name: display_name.to_owned(),
            id: unit_id.clone(),
            page_occurrence_refs: vec![page_id.clone()],
            parent_hierarchy_occurrence_ref: None,
            source_identity: source_identity.clone(),
            type_: "sch.unit_occurrence".to_owned(),
            unit_definition_ref: definition.unit.clone(),
        });
        self.graph.page_occurrences.push(PageOccurrence {
            address_key: Some(address_key),
            display_name: display_name.to_owned(),
            id: page_id.clone(),
            instance_order,
            page_definition_ref: definition.page.clone(),
            sheet_number: Some(sheet_number),
            source_identity,
            type_: "sch.page_occurrence".to_owned(),
            unit_occurrence_ref: unit_id.clone(),
        });
        Ok(OccurrenceRefs {
            unit: unit_id,
            page: page_id,
        })
    }

    fn append_hierarchy(
        &mut self,
        occurrence: &SchematicOccurrence,
        child: &OccurrenceRefs,
    ) -> Result<(), SourceBundleError> {
        ensure_capacity(
            self.graph.hierarchy_occurrences.len(),
            self.limits.max_hierarchy_occurrences,
            "compiled graph hierarchy-occurrence count",
        )?;
        let parent_index = occurrence
            .parent_index
            .and_then(|index| index.checked_sub(1))
            .ok_or_else(|| graph_error("compiled graph hierarchy parent index is invalid"))?;
        let parent = self
            .occurrences
            .get(parent_index)
            .ok_or_else(|| graph_error("compiled graph hierarchy parent is missing"))?
            .clone();
        let source_uuid = self.occurrence_source_uuid(occurrence)?;
        let source_record = format!("instance-path:{}", occurrence.occurrence_address);
        self.budget.reserve([
            "sch.hierarchy_occurrence".len(),
            UUID_TEXT_BYTES * 5,
            occurrence.legacy_address.len(),
            source_record.len(),
            source_uuid.len(),
        ])?;
        let source_identity = source_identity(
            &occurrence.legacy_address,
            &source_uuid,
            &source_record,
            "",
            "",
            "",
        );
        let source_mapping = source_identity_mapping(&source_identity);
        let hierarchy_id = self
            .allocator
            .allocate_source(
                "sch.hierarchy_occurrence",
                &source_mapping,
                &[parent.page.clone(), child.unit.clone()],
            )
            .map_err(identity_error)?;
        self.graph.hierarchy_occurrences.push(HierarchyOccurrence {
            child_unit_occurrence_ref: child.unit.clone(),
            id: hierarchy_id.clone(),
            parent_page_occurrence_ref: parent.page.clone(),
            parent_unit_occurrence_ref: parent.unit,
            source_identity,
            type_: "sch.hierarchy_occurrence".to_owned(),
        });
        self.graph
            .unit_occurrences
            .last_mut()
            .ok_or_else(|| graph_error("compiled graph child unit occurrence is missing"))?
            .parent_hierarchy_occurrence_ref = Some(hierarchy_id.clone());
        self.append_graphical_link(
            &parent.page,
            GraphicalTargetType::SchHierarchyOccurrence,
            &hierarchy_id,
            &source_uuid,
        )
    }

    fn append_components(
        &mut self,
        occurrence: &SchematicOccurrence,
        definition: &SchematicDefinition,
        owner: &OccurrenceRefs,
    ) -> Result<(), SourceBundleError> {
        let effective = self.index.effective_symbols(occurrence.index, None)?;
        if effective.len() != definition.symbols.len() {
            return Err(graph_error(
                "compiled graph effective symbol cardinality does not match its definition",
            ));
        }
        for (symbol, effective) in definition.symbols.iter().zip(effective.iter()) {
            if is_power_symbol(definition, symbol)
                || effective.reference.is_empty()
                || effective.reference.starts_with('#')
            {
                continue;
            }
            ensure_capacity(
                self.graph.component_occurrences.len(),
                self.limits.max_component_occurrences,
                "compiled graph component-occurrence count",
            )?;
            let source_designator = authored_reference(symbol);
            self.budget.reserve([
                "sch.component_occurrence".len(),
                UUID_TEXT_BYTES * 2,
                symbol.uuid.len(),
                occurrence.legacy_address.len(),
                source_designator.len(),
                effective.reference.len() * 2,
            ])?;
            let source_identity =
                source_identity(&occurrence.legacy_address, &symbol.uuid, "", "", "", "");
            let source_mapping = source_identity_mapping(&source_identity);
            let component_id = self
                .allocator
                .allocate_source(
                    "sch.component_occurrence",
                    &source_mapping,
                    std::slice::from_ref(&owner.page),
                )
                .map_err(identity_error)?;
            let unit = u64::try_from(effective.unit.max(1))
                .ok()
                .and_then(NonZeroU64::new)
                .ok_or_else(|| graph_error("compiled graph component unit is invalid"))?;
            let body_style = u32::try_from(symbol.convert)
                .map_err(|_| graph_error("compiled graph component body style exceeds uint32"))?;
            self.graph.component_occurrences.push(ComponentOccurrence {
                body_style,
                design_component_ref: None,
                display_designator: effective.reference.clone(),
                id: component_id.clone(),
                page_occurrence_ref: owner.page.clone(),
                physical_designator: effective.reference.clone(),
                source_designator: source_designator.to_owned(),
                source_identity,
                type_: "sch.component_occurrence".to_owned(),
                unit,
            });
            self.component_by_symbol
                .entry(occurrence.index)
                .or_default()
                .insert(symbol.uuid.clone(), component_id.clone());
            if !symbol.uuid.is_empty() {
                self.append_graphical_link(
                    &owner.page,
                    GraphicalTargetType::SchComponentOccurrence,
                    &component_id,
                    &symbol.uuid,
                )?;
            }
        }
        Ok(())
    }

    fn append_aggregate_drawing_links(
        &mut self,
        definition: &SchematicDefinition,
        owner: &OccurrenceRefs,
    ) -> Result<(), SourceBundleError> {
        for element_id in definition
            .buses
            .iter()
            .map(|value| value.uuid.as_str())
            .chain(
                definition
                    .bus_entries
                    .iter()
                    .map(|value| value.uuid.as_str()),
            )
            .filter(|value| !value.is_empty())
        {
            self.append_graphical_link(
                &owner.page,
                GraphicalTargetType::SchPageOccurrence,
                &owner.page,
                element_id,
            )?;
        }
        Ok(())
    }

    fn append_graphical_link(
        &mut self,
        page_ref: &str,
        target_type: GraphicalTargetType,
        target_ref: &str,
        element_id: &str,
    ) -> Result<(), SourceBundleError> {
        ensure_capacity(
            self.graph.graphical_artifact_links.len(),
            self.limits.max_graphical_artifact_links,
            "compiled graph graphical-artifact-link count",
        )?;
        self.budget.reserve([
            "sch.graphical_artifact_link".len(),
            UUID_TEXT_BYTES * 4,
            target_type.to_string().len(),
            DRAWING_ARTIFACT_KEY.len(),
            element_id.len() * 2,
        ])?;
        let identity = IdentityMapping::from([
            (
                "artifact_key".to_owned(),
                Value::String(DRAWING_ARTIFACT_KEY.to_owned()),
            ),
            (
                "element_id".to_owned(),
                Value::String(element_id.to_owned()),
            ),
            (
                "page_occurrence_ref".to_owned(),
                Value::String(page_ref.to_owned()),
            ),
            (
                "target_ref".to_owned(),
                Value::String(target_ref.to_owned()),
            ),
            (
                "target_type".to_owned(),
                Value::String(target_type.to_string()),
            ),
        ]);
        let id = self
            .allocator
            .allocate_derived("sch.graphical_artifact_link", &identity)
            .map_err(identity_error)?;
        self.graph
            .graphical_artifact_links
            .push(GraphicalArtifactLink {
                artifact_key: DRAWING_ARTIFACT_KEY.to_owned(),
                element_id: element_id.to_owned(),
                id,
                page_occurrence_ref: page_ref.to_owned(),
                source_identity: source_identity("", "", "", "", "", element_id),
                target_ref: target_ref.to_owned(),
                target_type,
                type_: "sch.graphical_artifact_link".to_owned(),
            });
        Ok(())
    }

    fn occurrence_source_uuid(
        &self,
        occurrence: &SchematicOccurrence,
    ) -> Result<String, SourceBundleError> {
        if let Some(uuid) = occurrence.sheet_uuid.as_deref() {
            return Ok(uuid.to_owned());
        }
        self.index
            .definition(&occurrence.source_path)
            .map(|definition| definition.uuid.as_deref().unwrap_or_default().to_owned())
            .ok_or_else(|| {
                missing_error(
                    &occurrence.source_path,
                    "graph occurrence source is missing",
                )
            })
    }
}

struct RetainedStringBudget {
    retained: usize,
    maximum: usize,
}

impl RetainedStringBudget {
    fn new(maximum: usize) -> Self {
        Self {
            retained: 0,
            maximum,
        }
    }

    fn reserve(
        &mut self,
        lengths: impl IntoIterator<Item = usize>,
    ) -> Result<(), SourceBundleError> {
        let mut added = 0_usize;
        for length in lengths {
            added = added
                .checked_add(length)
                .ok_or_else(|| limit_error("compiled graph retained string bytes overflow"))?;
        }
        let total = self
            .retained
            .checked_add(added)
            .ok_or_else(|| limit_error("compiled graph retained string bytes overflow"))?;
        if total > self.maximum {
            return Err(limit_error(
                "compiled graph retained string bytes exceed their limit",
            ));
        }
        self.retained = total;
        Ok(())
    }
}

fn source_identity(
    source_path: &str,
    source_uuid: &str,
    source_record: &str,
    source_subobject: &str,
    compiled_net: &str,
    artifact_element: &str,
) -> SourceIdentity {
    SourceIdentity {
        sch_source_key_artifact_element: nonempty_owned(artifact_element),
        sch_source_key_compiled_net: nonempty_owned(compiled_net),
        sch_source_key_source_path: nonempty_owned(source_path),
        sch_source_key_source_record: nonempty_owned(source_record),
        sch_source_key_source_subobject: nonempty_owned(source_subobject),
        sch_source_key_source_uuid: nonempty_owned(source_uuid),
    }
}

fn source_identity_mapping(source: &SourceIdentity) -> IdentityMapping {
    [
        (
            "sch.source_key.artifact_element",
            source.sch_source_key_artifact_element.as_ref(),
        ),
        (
            "sch.source_key.compiled_net",
            source.sch_source_key_compiled_net.as_ref(),
        ),
        (
            "sch.source_key.source_path",
            source.sch_source_key_source_path.as_ref(),
        ),
        (
            "sch.source_key.source_record",
            source.sch_source_key_source_record.as_ref(),
        ),
        (
            "sch.source_key.source_subobject",
            source.sch_source_key_source_subobject.as_ref(),
        ),
        (
            "sch.source_key.source_uuid",
            source.sch_source_key_source_uuid.as_ref(),
        ),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| (key.to_owned(), Value::String(value.clone()))))
    .collect()
}

fn nonempty_owned(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn authored_reference(symbol: &SchematicPlacedSymbol) -> &str {
    symbol
        .properties
        .iter()
        .find(|property| property.key == "Reference")
        .map_or("", |property| property.value.as_str())
}

fn is_power_symbol(definition: &SchematicDefinition, symbol: &SchematicPlacedSymbol) -> bool {
    symbol.lib_id != "power:PWR_FLAG"
        && (symbol.lib_id.starts_with("power:")
            || definition
                .library_symbol_for_placement(symbol)
                .is_some_and(|library| library.power))
}

fn portable_stem(path: &str) -> Option<&str> {
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename
        .rsplit_once('.')
        .map(|(stem, _extension)| stem)
        .filter(|stem| !stem.is_empty())
        .or((!filename.is_empty()).then_some(filename))
}

fn ensure_capacity(current: usize, maximum: usize, family: &str) -> Result<(), SourceBundleError> {
    if current >= maximum {
        return Err(limit_error(&format!("{family} exceeds its limit")));
    }
    Ok(())
}

fn identity_error(error: impl std::fmt::Display) -> SourceBundleError {
    graph_error(format!("compiled graph identity: {error}"))
}

fn graph_error(message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Schematic, None, message)
}

fn missing_error(source_path: &str, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::MissingSource,
        Some(source_path),
        message,
    )
}

fn limit_error(message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, None, message)
}

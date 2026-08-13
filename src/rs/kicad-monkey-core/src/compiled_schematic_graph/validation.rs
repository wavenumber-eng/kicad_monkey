use kicad_monkey_contracts::ValidationError;
use kicad_monkey_contracts::generated::compiled_schematic_graph::{
    CompiledSchematicGraphA0, ComponentOccurrence, GraphicalTargetType, HierarchyOccurrence,
    LocalNetOccurrence, PageDefinition, PageOccurrence, ResolutionDiagnostic, TerminalOccurrence,
    TerminalRole, UnitDefinition, UnitOccurrence,
};
use kicad_monkey_contracts::validate_compiled_schematic_graph_contract;
use std::collections::{HashMap, HashSet};

/// Validate graph identity, reference types, ownership, topology, and links.
pub fn validate_compiled_schematic_graph(
    document: &CompiledSchematicGraphA0,
) -> Result<(), ValidationError> {
    validate_compiled_schematic_graph_contract(document)?;
    let index = GraphIndex::build(document)?;
    validate_typed_refs(document, &index)?;
    validate_definition_ownership(document, &index)?;
    validate_occurrence_ownership(document, &index)?;
    let parent_by_child = validate_hierarchy_ownership(document, &index)?;
    validate_hierarchy_is_acyclic(&index, &parent_by_child)?;
    validate_terminal_rows(document, &index)?;
    validate_hierarchy_bindings(document, &index)?;
    validate_graphical_links(document, &index)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowKind {
    UnitDefinition,
    PageDefinition,
    UnitOccurrence,
    PageOccurrence,
    HierarchyOccurrence,
    ComponentOccurrence,
    LocalNetOccurrence,
    TerminalOccurrence,
    HierarchyTerminalBinding,
    GraphicalArtifactLink,
}

impl RowKind {
    const fn token(self) -> &'static str {
        match self {
            Self::UnitDefinition => "sch.unit_definition",
            Self::PageDefinition => "sch.page_definition",
            Self::UnitOccurrence => "sch.unit_occurrence",
            Self::PageOccurrence => "sch.page_occurrence",
            Self::HierarchyOccurrence => "sch.hierarchy_occurrence",
            Self::ComponentOccurrence => "sch.component_occurrence",
            Self::LocalNetOccurrence => "sch.local_net_occurrence",
            Self::TerminalOccurrence => "sch.terminal_occurrence",
            Self::HierarchyTerminalBinding => "sch.hierarchy_terminal_binding",
            Self::GraphicalArtifactLink => "sch.graphical_artifact_link",
        }
    }
}

struct GraphIndex<'a> {
    kinds: HashMap<&'a str, RowKind>,
    unit_definitions: HashMap<&'a str, &'a UnitDefinition>,
    page_definitions: HashMap<&'a str, &'a PageDefinition>,
    unit_occurrences: HashMap<&'a str, &'a UnitOccurrence>,
    page_occurrences: HashMap<&'a str, &'a PageOccurrence>,
    hierarchies: HashMap<&'a str, &'a HierarchyOccurrence>,
    components: HashMap<&'a str, &'a ComponentOccurrence>,
    local_nets: HashMap<&'a str, &'a LocalNetOccurrence>,
    terminals: HashMap<&'a str, &'a TerminalOccurrence>,
}

impl<'a> GraphIndex<'a> {
    fn build(document: &'a CompiledSchematicGraphA0) -> Result<Self, ValidationError> {
        let capacity = row_count(document);
        let mut index = Self {
            kinds: HashMap::with_capacity(capacity),
            unit_definitions: HashMap::with_capacity(document.unit_definitions.len()),
            page_definitions: HashMap::with_capacity(document.page_definitions.len()),
            unit_occurrences: HashMap::with_capacity(document.unit_occurrences.len()),
            page_occurrences: HashMap::with_capacity(document.page_occurrences.len()),
            hierarchies: HashMap::with_capacity(document.hierarchy_occurrences.len()),
            components: HashMap::with_capacity(document.component_occurrences.len()),
            local_nets: HashMap::with_capacity(document.local_net_occurrences.len()),
            terminals: HashMap::with_capacity(document.terminal_occurrences.len()),
        };
        index.index_definitions(document)?;
        index.index_occurrences(document)?;
        index.index_connectivity(document)?;
        Ok(index)
    }

    fn index_definitions(
        &mut self,
        document: &'a CompiledSchematicGraphA0,
    ) -> Result<(), ValidationError> {
        for row in &document.unit_definitions {
            self.insert(row.id.as_str(), RowKind::UnitDefinition)?;
            self.unit_definitions.insert(&row.id, row);
        }
        for row in &document.page_definitions {
            self.insert(row.id.as_str(), RowKind::PageDefinition)?;
            self.page_definitions.insert(&row.id, row);
        }
        Ok(())
    }

    fn index_occurrences(
        &mut self,
        document: &'a CompiledSchematicGraphA0,
    ) -> Result<(), ValidationError> {
        for row in &document.unit_occurrences {
            self.insert(row.id.as_str(), RowKind::UnitOccurrence)?;
            self.unit_occurrences.insert(&row.id, row);
        }
        for row in &document.page_occurrences {
            self.insert(row.id.as_str(), RowKind::PageOccurrence)?;
            self.page_occurrences.insert(&row.id, row);
        }
        for row in &document.hierarchy_occurrences {
            self.insert(row.id.as_str(), RowKind::HierarchyOccurrence)?;
            self.hierarchies.insert(&row.id, row);
        }
        Ok(())
    }

    fn index_connectivity(
        &mut self,
        document: &'a CompiledSchematicGraphA0,
    ) -> Result<(), ValidationError> {
        for row in &document.component_occurrences {
            self.insert(row.id.as_str(), RowKind::ComponentOccurrence)?;
            self.components.insert(&row.id, row);
        }
        for row in &document.local_net_occurrences {
            self.insert(row.id.as_str(), RowKind::LocalNetOccurrence)?;
            self.local_nets.insert(&row.id, row);
        }
        for row in &document.terminal_occurrences {
            self.insert(row.id.as_str(), RowKind::TerminalOccurrence)?;
            self.terminals.insert(&row.id, row);
        }
        for row in &document.hierarchy_terminal_bindings {
            self.insert(row.id.as_str(), RowKind::HierarchyTerminalBinding)?;
        }
        for row in &document.graphical_artifact_links {
            self.insert(row.id.as_str(), RowKind::GraphicalArtifactLink)?;
        }
        Ok(())
    }

    fn insert(&mut self, id: &'a str, kind: RowKind) -> Result<(), ValidationError> {
        if !is_uuid_v7(id) {
            return Err(error(
                "invalid_row_id",
                "$.id",
                "compiled schematic graph row ids must be RFC 4122 UUIDv7 values",
            ));
        }
        if self.kinds.insert(id, kind).is_some() {
            return Err(error(
                "duplicate_row_id",
                "$.id",
                "compiled schematic graph row ids must be unique",
            ));
        }
        Ok(())
    }

    fn expect_kind(
        &self,
        value: &str,
        expected: RowKind,
        path: String,
    ) -> Result<(), ValidationError> {
        if value.is_empty() {
            return Ok(());
        }
        let Some(actual) = self.kinds.get(value) else {
            return Err(error(
                "unresolved_reference",
                path,
                "compiled schematic graph reference does not resolve",
            ));
        };
        if *actual != expected {
            return Err(error("wrong_reference_type", path, expected.token()));
        }
        Ok(())
    }
}

fn row_count(document: &CompiledSchematicGraphA0) -> usize {
    document.unit_definitions.len()
        + document.page_definitions.len()
        + document.unit_occurrences.len()
        + document.page_occurrences.len()
        + document.hierarchy_occurrences.len()
        + document.component_occurrences.len()
        + document.local_net_occurrences.len()
        + document.terminal_occurrences.len()
        + document.hierarchy_terminal_bindings.len()
        + document.graphical_artifact_links.len()
}

fn validate_typed_refs(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    validate_definition_refs(document, index)?;
    validate_occurrence_refs(document, index)?;
    validate_connectivity_refs(document, index)?;
    for (row_index, row) in document.graphical_artifact_links.iter().enumerate() {
        index.expect_kind(
            &row.page_occurrence_ref,
            RowKind::PageOccurrence,
            format!("$.graphical_artifact_links[{row_index}].page_occurrence_ref"),
        )?;
    }
    Ok(())
}

fn validate_definition_refs(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    for (row_index, row) in document.unit_definitions.iter().enumerate() {
        for (ref_index, value) in row.page_definition_refs.iter().enumerate() {
            index.expect_kind(
                value,
                RowKind::PageDefinition,
                format!("$.unit_definitions[{row_index}].page_definition_refs[{ref_index}]"),
            )?;
        }
    }
    for (row_index, row) in document.page_definitions.iter().enumerate() {
        index.expect_kind(
            &row.unit_definition_ref,
            RowKind::UnitDefinition,
            format!("$.page_definitions[{row_index}].unit_definition_ref"),
        )?;
    }
    Ok(())
}

fn validate_occurrence_refs(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    for (row_index, row) in document.unit_occurrences.iter().enumerate() {
        index.expect_kind(
            &row.unit_definition_ref,
            RowKind::UnitDefinition,
            format!("$.unit_occurrences[{row_index}].unit_definition_ref"),
        )?;
        if let Some(value) = row.parent_hierarchy_occurrence_ref.as_deref() {
            index.expect_kind(
                value,
                RowKind::HierarchyOccurrence,
                format!("$.unit_occurrences[{row_index}].parent_hierarchy_occurrence_ref"),
            )?;
        }
        for (ref_index, value) in row.page_occurrence_refs.iter().enumerate() {
            index.expect_kind(
                value,
                RowKind::PageOccurrence,
                format!("$.unit_occurrences[{row_index}].page_occurrence_refs[{ref_index}]"),
            )?;
        }
    }
    for (row_index, row) in document.page_occurrences.iter().enumerate() {
        index.expect_kind(
            &row.page_definition_ref,
            RowKind::PageDefinition,
            format!("$.page_occurrences[{row_index}].page_definition_ref"),
        )?;
        index.expect_kind(
            &row.unit_occurrence_ref,
            RowKind::UnitOccurrence,
            format!("$.page_occurrences[{row_index}].unit_occurrence_ref"),
        )?;
    }
    for (row_index, row) in document.hierarchy_occurrences.iter().enumerate() {
        for (field, value, kind) in [
            (
                "parent_unit_occurrence_ref",
                row.parent_unit_occurrence_ref.as_str(),
                RowKind::UnitOccurrence,
            ),
            (
                "parent_page_occurrence_ref",
                row.parent_page_occurrence_ref.as_str(),
                RowKind::PageOccurrence,
            ),
            (
                "child_unit_occurrence_ref",
                row.child_unit_occurrence_ref.as_str(),
                RowKind::UnitOccurrence,
            ),
        ] {
            index.expect_kind(
                value,
                kind,
                format!("$.hierarchy_occurrences[{row_index}].{field}"),
            )?;
        }
    }
    Ok(())
}

fn validate_connectivity_refs(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    for (row_index, row) in document.component_occurrences.iter().enumerate() {
        index.expect_kind(
            &row.page_occurrence_ref,
            RowKind::PageOccurrence,
            format!("$.component_occurrences[{row_index}].page_occurrence_ref"),
        )?;
    }
    for (row_index, row) in document.local_net_occurrences.iter().enumerate() {
        index.expect_kind(
            &row.page_occurrence_ref,
            RowKind::PageOccurrence,
            format!("$.local_net_occurrences[{row_index}].page_occurrence_ref"),
        )?;
    }
    for (row_index, row) in document.terminal_occurrences.iter().enumerate() {
        index.expect_kind(
            &row.page_occurrence_ref,
            RowKind::PageOccurrence,
            format!("$.terminal_occurrences[{row_index}].page_occurrence_ref"),
        )?;
        validate_optional_ref(
            index,
            row.local_net_occurrence_ref.as_deref(),
            RowKind::LocalNetOccurrence,
            format!("$.terminal_occurrences[{row_index}].local_net_occurrence_ref"),
        )?;
        validate_optional_ref(
            index,
            row.component_occurrence_ref.as_deref(),
            RowKind::ComponentOccurrence,
            format!("$.terminal_occurrences[{row_index}].component_occurrence_ref"),
        )?;
    }
    for (row_index, row) in document.hierarchy_terminal_bindings.iter().enumerate() {
        for (field, value, kind) in [
            (
                "hierarchy_occurrence_ref",
                row.hierarchy_occurrence_ref.as_str(),
                RowKind::HierarchyOccurrence,
            ),
            (
                "parent_terminal_occurrence_ref",
                row.parent_terminal_occurrence_ref.as_str(),
                RowKind::TerminalOccurrence,
            ),
            (
                "child_terminal_occurrence_ref",
                row.child_terminal_occurrence_ref.as_str(),
                RowKind::TerminalOccurrence,
            ),
        ] {
            index.expect_kind(
                value,
                kind,
                format!("$.hierarchy_terminal_bindings[{row_index}].{field}"),
            )?;
        }
    }
    Ok(())
}

fn validate_optional_ref(
    index: &GraphIndex<'_>,
    value: Option<&str>,
    kind: RowKind,
    path: String,
) -> Result<(), ValidationError> {
    value.map_or(Ok(()), |value| index.expect_kind(value, kind, path))
}

fn validate_definition_ownership(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    for page in &document.page_definitions {
        let unit = index
            .unit_definitions
            .get(page.unit_definition_ref.as_str())
            .ok_or_else(|| ownership_error("page definition unit owner is unresolved"))?;
        if !unit.page_definition_refs.contains(&page.id) {
            return Err(ownership_error(
                "page definition is not listed by its owning unit",
            ));
        }
    }
    Ok(())
}

fn validate_occurrence_ownership(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    for page in &document.page_occurrences {
        let definition = index
            .page_definitions
            .get(page.page_definition_ref.as_str())
            .ok_or_else(|| ownership_error("page occurrence definition is unresolved"))?;
        let unit = index
            .unit_occurrences
            .get(page.unit_occurrence_ref.as_str())
            .ok_or_else(|| ownership_error("page occurrence unit owner is unresolved"))?;
        if definition.unit_definition_ref != unit.unit_definition_ref {
            return Err(ownership_error(
                "page occurrence definition has the wrong unit owner",
            ));
        }
        if !unit.page_occurrence_refs.contains(&page.id) {
            return Err(ownership_error(
                "page occurrence is not listed by its owning unit occurrence",
            ));
        }
    }
    Ok(())
}

fn validate_hierarchy_ownership<'a>(
    document: &'a CompiledSchematicGraphA0,
    index: &GraphIndex<'a>,
) -> Result<HashMap<&'a str, &'a str>, ValidationError> {
    let mut parent_by_child = HashMap::with_capacity(document.hierarchy_occurrences.len());
    let mut incoming_by_child = HashMap::with_capacity(document.hierarchy_occurrences.len());
    for hierarchy in &document.hierarchy_occurrences {
        let parent_page = index
            .page_occurrences
            .get(hierarchy.parent_page_occurrence_ref.as_str())
            .ok_or_else(|| ownership_error("hierarchy parent page is unresolved"))?;
        if parent_page.unit_occurrence_ref != hierarchy.parent_unit_occurrence_ref {
            return Err(ownership_error(
                "hierarchy parent page has the wrong unit owner",
            ));
        }
        if incoming_by_child
            .insert(
                hierarchy.child_unit_occurrence_ref.as_str(),
                hierarchy.id.as_str(),
            )
            .is_some()
        {
            return Err(ownership_error(
                "unit occurrence has multiple incoming hierarchy owners",
            ));
        }
        let child = index
            .unit_occurrences
            .get(hierarchy.child_unit_occurrence_ref.as_str())
            .ok_or_else(|| ownership_error("hierarchy child unit is unresolved"))?;
        if child.parent_hierarchy_occurrence_ref.as_deref() != Some(hierarchy.id.as_str()) {
            return Err(ownership_error(
                "unit occurrence hierarchy inverse is inconsistent",
            ));
        }
        parent_by_child.insert(
            hierarchy.child_unit_occurrence_ref.as_str(),
            hierarchy.parent_unit_occurrence_ref.as_str(),
        );
    }
    Ok(parent_by_child)
}

fn validate_hierarchy_is_acyclic(
    index: &GraphIndex<'_>,
    parent_by_child: &HashMap<&str, &str>,
) -> Result<(), ValidationError> {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Color {
        Visiting,
        Done,
    }
    let mut colors = HashMap::with_capacity(index.unit_occurrences.len());
    for start in index.unit_occurrences.keys().copied() {
        if colors.get(start) == Some(&Color::Done) {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(unit_ref) = current {
            match colors.get(unit_ref) {
                Some(Color::Visiting) => {
                    return Err(error(
                        "hierarchy_cycle",
                        "$.hierarchy_occurrences",
                        "schematic hierarchy occurrences must be acyclic",
                    ));
                }
                Some(Color::Done) => break,
                None => {
                    colors.insert(unit_ref, Color::Visiting);
                    path.push(unit_ref);
                    current = parent_by_child.get(unit_ref).copied();
                }
            }
        }
        for unit_ref in path {
            colors.insert(unit_ref, Color::Done);
        }
    }
    Ok(())
}

fn validate_terminal_rows(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    for terminal in &document.terminal_occurrences {
        validate_terminal_owners(terminal, index)?;
        validate_component_pin(terminal)?;
    }
    Ok(())
}

fn validate_terminal_owners(
    terminal: &TerminalOccurrence,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    if let Some(component_ref) = terminal.component_occurrence_ref.as_deref() {
        let component = index
            .components
            .get(component_ref)
            .ok_or_else(|| ownership_error("terminal component owner is unresolved"))?;
        if component.page_occurrence_ref != terminal.page_occurrence_ref {
            return Err(ownership_error(
                "terminal and component occurrence owners differ",
            ));
        }
    }
    if let Some(local_ref) = terminal.local_net_occurrence_ref.as_deref() {
        let local = index
            .local_nets
            .get(local_ref)
            .ok_or_else(|| ownership_error("terminal local-net owner is unresolved"))?;
        if local.page_occurrence_ref != terminal.page_occurrence_ref {
            return Err(ownership_error(
                "terminal and local-net occurrence owners differ",
            ));
        }
    }
    Ok(())
}

fn validate_component_pin(terminal: &TerminalOccurrence) -> Result<(), ValidationError> {
    if terminal.role != TerminalRole::ComponentPin {
        return Ok(());
    }
    if terminal.component_occurrence_ref.is_none()
        && !terminal
            .resolution_diagnostics
            .contains(&ResolutionDiagnostic::ComponentOccurrenceUnresolved)
    {
        return Err(ownership_error(
            "component-pin terminal needs component ownership or a diagnostic",
        ));
    }
    if terminal.design_component_pin_ref.is_none()
        && !terminal
            .resolution_diagnostics
            .contains(&ResolutionDiagnostic::LogicalPinUnresolved)
    {
        return Err(ownership_error(
            "component-pin terminal needs logical-pin ownership or a diagnostic",
        ));
    }
    Ok(())
}

fn validate_hierarchy_bindings(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    let mut pages_by_unit: HashMap<&str, HashSet<&str>> = HashMap::new();
    for page in &document.page_occurrences {
        pages_by_unit
            .entry(&page.unit_occurrence_ref)
            .or_default()
            .insert(&page.id);
    }
    let mut binding_parent_refs = HashSet::new();
    for binding in &document.hierarchy_terminal_bindings {
        let hierarchy = index
            .hierarchies
            .get(binding.hierarchy_occurrence_ref.as_str())
            .ok_or_else(|| ownership_error("binding hierarchy is unresolved"))?;
        let parent = index
            .terminals
            .get(binding.parent_terminal_occurrence_ref.as_str())
            .ok_or_else(|| ownership_error("binding parent terminal is unresolved"))?;
        let child = index
            .terminals
            .get(binding.child_terminal_occurrence_ref.as_str())
            .ok_or_else(|| ownership_error("binding child terminal is unresolved"))?;
        validate_binding_ownership(hierarchy, parent, child, &pages_by_unit)?;
        validate_binding_design_nets(binding.design_net_ref.as_deref(), parent, child)?;
        binding_parent_refs.insert(parent.id.as_str());
    }
    validate_binding_completeness(document, &binding_parent_refs)
}

fn validate_binding_ownership(
    hierarchy: &HierarchyOccurrence,
    parent: &TerminalOccurrence,
    child: &TerminalOccurrence,
    pages_by_unit: &HashMap<&str, HashSet<&str>>,
) -> Result<(), ValidationError> {
    if parent.role != TerminalRole::SheetEntry || child.role != TerminalRole::Port {
        return Err(ownership_error(
            "hierarchy binding must connect a sheet_entry to a port",
        ));
    }
    if parent.page_occurrence_ref != hierarchy.parent_page_occurrence_ref {
        return Err(ownership_error(
            "hierarchy binding parent terminal has wrong page owner",
        ));
    }
    let valid_child_page = pages_by_unit
        .get(hierarchy.child_unit_occurrence_ref.as_str())
        .is_some_and(|pages| pages.contains(child.page_occurrence_ref.as_str()));
    if !valid_child_page {
        return Err(ownership_error(
            "hierarchy binding child terminal has wrong unit owner",
        ));
    }
    Ok(())
}

fn validate_binding_design_nets(
    binding_net: Option<&str>,
    parent: &TerminalOccurrence,
    child: &TerminalOccurrence,
) -> Result<(), ValidationError> {
    let resolved: HashSet<&str> = [
        parent.design_net_ref.as_deref(),
        child.design_net_ref.as_deref(),
        binding_net,
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    .collect();
    if resolved.len() > 1 {
        Err(ownership_error(
            "hierarchy binding resolves to different design nets",
        ))
    } else {
        Ok(())
    }
}

fn validate_binding_completeness(
    document: &CompiledSchematicGraphA0,
    binding_parent_refs: &HashSet<&str>,
) -> Result<(), ValidationError> {
    let parent_pages: HashSet<&str> = document
        .hierarchy_occurrences
        .iter()
        .map(|row| row.parent_page_occurrence_ref.as_str())
        .collect();
    for terminal in &document.terminal_occurrences {
        if terminal.role != TerminalRole::SheetEntry
            || !parent_pages.contains(terminal.page_occurrence_ref.as_str())
        {
            continue;
        }
        if terminal.design_net_ref.is_none()
            && !terminal
                .resolution_diagnostics
                .contains(&ResolutionDiagnostic::DesignNetUnresolved)
        {
            return Err(ownership_error(
                "hierarchy sheet-entry terminal needs a design net or diagnostic",
            ));
        }
        if !binding_parent_refs.contains(terminal.id.as_str())
            && !terminal
                .resolution_diagnostics
                .contains(&ResolutionDiagnostic::HierarchyTerminalBindingUnresolved)
        {
            return Err(ownership_error(
                "hierarchy sheet-entry terminal needs a binding or diagnostic",
            ));
        }
    }
    Ok(())
}

fn validate_graphical_links(
    document: &CompiledSchematicGraphA0,
    index: &GraphIndex<'_>,
) -> Result<(), ValidationError> {
    let mut selectors = HashMap::with_capacity(document.graphical_artifact_links.len());
    for link in &document.graphical_artifact_links {
        let selector = (
            link.page_occurrence_ref.as_str(),
            link.artifact_key.as_str(),
            link.element_id.as_str(),
        );
        let target = (link.target_type, link.target_ref.as_str());
        if selectors
            .insert(selector, target)
            .is_some_and(|previous| previous != target)
        {
            return Err(ownership_error(
                "graphical artifact selector resolves to multiple targets",
            ));
        }
        let expected_kind = target_kind(link.target_type);
        index.expect_kind(
            &link.target_ref,
            expected_kind,
            "$.graphical_artifact_links[].target_ref".to_owned(),
        )?;
        if target_page(link, index)? != link.page_occurrence_ref {
            return Err(ownership_error(
                "graphical artifact target has wrong page owner",
            ));
        }
    }
    Ok(())
}

const fn target_kind(target_type: GraphicalTargetType) -> RowKind {
    match target_type {
        GraphicalTargetType::SchComponentOccurrence => RowKind::ComponentOccurrence,
        GraphicalTargetType::SchHierarchyOccurrence => RowKind::HierarchyOccurrence,
        GraphicalTargetType::SchTerminalOccurrence => RowKind::TerminalOccurrence,
        GraphicalTargetType::SchLocalNetOccurrence => RowKind::LocalNetOccurrence,
        GraphicalTargetType::SchPageOccurrence => RowKind::PageOccurrence,
    }
}

fn target_page<'a>(
    link: &'a kicad_monkey_contracts::generated::compiled_schematic_graph::GraphicalArtifactLink,
    index: &'a GraphIndex<'a>,
) -> Result<&'a str, ValidationError> {
    let target_ref = link.target_ref.as_str();
    match link.target_type {
        GraphicalTargetType::SchComponentOccurrence => index
            .components
            .get(target_ref)
            .map(|row| row.page_occurrence_ref.as_str()),
        GraphicalTargetType::SchHierarchyOccurrence => index
            .hierarchies
            .get(target_ref)
            .map(|row| row.parent_page_occurrence_ref.as_str()),
        GraphicalTargetType::SchTerminalOccurrence => index
            .terminals
            .get(target_ref)
            .map(|row| row.page_occurrence_ref.as_str()),
        GraphicalTargetType::SchLocalNetOccurrence => index
            .local_nets
            .get(target_ref)
            .map(|row| row.page_occurrence_ref.as_str()),
        GraphicalTargetType::SchPageOccurrence => index
            .page_occurrences
            .get(target_ref)
            .map(|row| row.id.as_str()),
    }
    .ok_or_else(|| ownership_error("graphical artifact target is unresolved"))
}

fn is_uuid_v7(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, value)| [8, 13, 18, 23].contains(&index) || value.is_ascii_hexdigit())
        && bytes[14] == b'7'
        && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

fn ownership_error(message: &'static str) -> ValidationError {
    error("invalid_ownership", "$", message)
}

fn error(code: &'static str, path: impl Into<String>, message: &'static str) -> ValidationError {
    ValidationError {
        code,
        path: path.into(),
        message,
    }
}

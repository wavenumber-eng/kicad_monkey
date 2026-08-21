use super::*;
use crate::schematic_design::{SchematicCompiledDesign, build_schematic_compiled_design};
use crate::{
    SchematicDesignNet, SchematicGraphicalIds, SchematicHierarchyNetBinding,
    SchematicWireDriverKind, SchematicWireSubgraph,
};
use kicad_monkey_contracts::generated::compiled_schematic_graph::{
    HierarchyTerminalBinding, LocalNetOccurrence, ResolutionDiagnostic, TerminalOccurrence,
    TerminalRole,
};
use std::collections::{BTreeSet, HashMap, HashSet};

type MemberKey = (usize, usize);
type TerminalSourceKey = (usize, TerminalRole, String);
type TerminalSemanticKey = (TerminalRole, String, String, String);

struct ConnectivityState {
    net_by_member: HashMap<MemberKey, usize>,
    unmaterialized_net_names: HashMap<MemberKey, String>,
    terminal_by_source: HashMap<TerminalSourceKey, String>,
    terminal_index_by_id: HashMap<String, usize>,
    hierarchical_port_by_name: HashMap<(usize, String), String>,
}

pub(super) struct ConnectivityCompilation {
    occurrences: Vec<crate::schematic_design::CompiledOccurrence>,
    nets: Vec<SchematicDesignNet>,
    hierarchy_bindings: Vec<SchematicHierarchyNetBinding>,
    state: ConnectivityState,
}

struct OccurrenceTerminalState {
    terminal_by_semantic_source: HashMap<TerminalSemanticKey, usize>,
    pin_element_counts: HashMap<String, usize>,
}

struct TerminalInput<'a> {
    role: TerminalRole,
    source_uuid: &'a str,
    source_subobject: &'a str,
    name: &'a str,
    pin_designator: &'a str,
    component_ref: Option<&'a str>,
    element_id: &'a str,
    hierarchical_port: bool,
}

struct SubgraphContext<'a> {
    occurrence_index: usize,
    subgraph_index: usize,
    owner: &'a OccurrenceRefs,
    nets: &'a [SchematicDesignNet],
    state: &'a mut ConnectivityState,
    terminal_state: &'a mut OccurrenceTerminalState,
}

struct TerminalCollection {
    indices: Vec<usize>,
    element_ids: HashSet<String>,
}

struct TerminalContext<'a> {
    occurrence_index: usize,
    owner: &'a OccurrenceRefs,
    state: &'a mut ConnectivityState,
    terminal_state: &'a mut OccurrenceTerminalState,
    collection: &'a mut TerminalCollection,
}

struct LocalNetInput<'a> {
    occurrence_index: usize,
    owner: &'a OccurrenceRefs,
    subgraph: &'a SchematicWireSubgraph,
    design_net: Option<&'a SchematicDesignNet>,
    unmaterialized_name: Option<&'a str>,
    terminal_indices: &'a [usize],
    terminal_refs: BTreeSet<String>,
    graphical_elements: Vec<String>,
}

impl StructuralGraphBuilder<'_> {
    pub(super) fn prepare_connectivity(
        &self,
    ) -> Result<ConnectivityCompilation, SourceBundleError> {
        let SchematicCompiledDesign {
            netlist,
            occurrences,
            unmaterialized_net_names,
        } = build_schematic_compiled_design(
            self.index,
            1,
            self.index.subpart_settings(),
            self.limits.design,
        )?;
        let mut net_by_member = HashMap::new();
        for (net_index, net) in netlist.nets.iter().enumerate() {
            for member in &net.members {
                net_by_member.insert((member.occurrence_index, member.subgraph_index), net_index);
            }
        }
        Ok(ConnectivityCompilation {
            occurrences,
            nets: netlist.nets,
            hierarchy_bindings: netlist.hierarchy_bindings,
            state: ConnectivityState {
                net_by_member,
                unmaterialized_net_names,
                terminal_by_source: HashMap::new(),
                terminal_index_by_id: HashMap::new(),
                hierarchical_port_by_name: HashMap::new(),
            },
        })
    }

    pub(super) fn append_compiled_occurrence_connectivity(
        &mut self,
        position: usize,
        compilation: &mut ConnectivityCompilation,
    ) -> Result<(), SourceBundleError> {
        let occurrence = compilation
            .occurrences
            .get(position)
            .ok_or_else(|| graph_error("compiled graph connectivity occurrence is missing"))?;
        self.append_occurrence_connectivity(occurrence, &compilation.nets, &mut compilation.state)
    }

    pub(super) fn finish_connectivity(
        &mut self,
        mut compilation: ConnectivityCompilation,
    ) -> Result<(), SourceBundleError> {
        self.append_hierarchy_bindings(&compilation.hierarchy_bindings, &mut compilation.state)?;
        Ok(())
    }

    fn append_occurrence_connectivity(
        &mut self,
        occurrence: &crate::schematic_design::CompiledOccurrence,
        nets: &[SchematicDesignNet],
        state: &mut ConnectivityState,
    ) -> Result<(), SourceBundleError> {
        let occurrence_refs = self
            .occurrences
            .get(occurrence.occurrence_index.saturating_sub(1))
            .ok_or_else(|| graph_error("compiled graph occurrence ownership is missing"))?
            .clone();
        let mut terminal_state = OccurrenceTerminalState {
            terminal_by_semantic_source: HashMap::new(),
            pin_element_counts: pin_element_counts(&occurrence.subgraphs),
        };
        for (subgraph_index, subgraph) in occurrence.subgraphs.iter().enumerate() {
            let mut context = SubgraphContext {
                occurrence_index: occurrence.occurrence_index,
                subgraph_index,
                owner: &occurrence_refs,
                nets,
                state,
                terminal_state: &mut terminal_state,
            };
            self.append_subgraph(subgraph, &mut context)?;
        }
        Ok(())
    }

    fn append_subgraph(
        &mut self,
        subgraph: &SchematicWireSubgraph,
        context: &mut SubgraphContext<'_>,
    ) -> Result<(), SourceBundleError> {
        let mut collection = TerminalCollection {
            indices: Vec::new(),
            element_ids: HashSet::new(),
        };
        {
            let mut terminal_context = TerminalContext {
                occurrence_index: context.occurrence_index,
                owner: context.owner,
                state: context.state,
                terminal_state: context.terminal_state,
                collection: &mut collection,
            };
            self.append_pin_terminals(subgraph, &mut terminal_context)?;
            self.append_label_terminals(subgraph, &mut terminal_context)?;
        }
        let graphical_elements =
            local_graphical_elements(&subgraph.graphical, &collection.element_ids);
        let terminal_refs = collection
            .indices
            .iter()
            .map(|index| self.graph.terminal_occurrences[*index].id.clone())
            .collect::<BTreeSet<_>>();
        if terminal_refs.is_empty() && graphical_elements.is_empty() {
            return Ok(());
        }
        let design_net = context
            .state
            .net_by_member
            .get(&(context.occurrence_index, context.subgraph_index))
            .and_then(|index| context.nets.get(*index));
        let unmaterialized_name = context
            .state
            .unmaterialized_net_names
            .get(&(context.occurrence_index, context.subgraph_index))
            .map(String::as_str);
        self.append_local_net(LocalNetInput {
            occurrence_index: context.occurrence_index,
            owner: context.owner,
            subgraph,
            design_net,
            unmaterialized_name,
            terminal_indices: &collection.indices,
            terminal_refs,
            graphical_elements,
        })
    }

    fn append_pin_terminals(
        &mut self,
        subgraph: &SchematicWireSubgraph,
        context: &mut TerminalContext<'_>,
    ) -> Result<(), SourceBundleError> {
        for pin in &subgraph.pin_drivers {
            if pin.reference.starts_with('#') {
                if pin.is_power {
                    let name = if pin.power_value.is_empty() {
                        pin.pin_name.as_str()
                    } else {
                        pin.power_value.as_str()
                    };
                    self.add_terminal(
                        TerminalInput {
                            role: TerminalRole::PowerPort,
                            source_uuid: &pin.symbol_uuid,
                            source_subobject: &pin.pin_number,
                            name,
                            pin_designator: &pin.pin_number,
                            component_ref: None,
                            // Power-port terminals address the complete placed
                            // symbol, not an independently rendered pin group.
                            element_id: &pin.symbol_uuid,
                            hierarchical_port: false,
                        },
                        context,
                    )?;
                }
                continue;
            }
            let element_id = unique_pin_element(pin, &context.terminal_state.pin_element_counts);
            let component_ref = self
                .component_by_symbol
                .get(&context.occurrence_index)
                .and_then(|symbols| symbols.get(&pin.symbol_uuid))
                .cloned();
            let source_uuid = if pin.source_pin_uuid.is_empty() {
                pin.symbol_uuid.as_str()
            } else {
                pin.source_pin_uuid.as_str()
            };
            self.add_terminal(
                TerminalInput {
                    role: TerminalRole::ComponentPin,
                    source_uuid,
                    source_subobject: &pin.pin_number,
                    name: &pin.pin_name,
                    pin_designator: &pin.pin_number,
                    component_ref: component_ref.as_deref(),
                    element_id,
                    hierarchical_port: false,
                },
                context,
            )?;
        }
        Ok(())
    }

    fn append_label_terminals(
        &mut self,
        subgraph: &SchematicWireSubgraph,
        context: &mut TerminalContext<'_>,
    ) -> Result<(), SourceBundleError> {
        for label in &subgraph.label_drivers {
            let (role, hierarchical_port) = match label.kind {
                SchematicWireDriverKind::HierarchicalLabel => (TerminalRole::Port, true),
                SchematicWireDriverKind::GlobalLabel => (TerminalRole::Port, false),
                SchematicWireDriverKind::SheetPin => (TerminalRole::SheetEntry, false),
                _ => continue,
            };
            let source_uuid = if label.source_uuid.is_empty() {
                label.render_id.as_str()
            } else {
                label.source_uuid.as_str()
            };
            self.add_terminal(
                TerminalInput {
                    role,
                    source_uuid,
                    source_subobject: "",
                    name: &label.text,
                    pin_designator: "",
                    component_ref: None,
                    element_id: &label.render_id,
                    hierarchical_port,
                },
                context,
            )?;
        }
        Ok(())
    }

    fn add_terminal(
        &mut self,
        input: TerminalInput<'_>,
        context: &mut TerminalContext<'_>,
    ) -> Result<(), SourceBundleError> {
        let track_hierarchical_port =
            input.hierarchical_port && self.has_parent_occurrence(context.occurrence_index);
        let semantic_key = (
            input.role,
            input.component_ref.unwrap_or_default().to_owned(),
            input.source_uuid.to_owned(),
            input.source_subobject.to_owned(),
        );
        if let Some(index) = context
            .terminal_state
            .terminal_by_semantic_source
            .get(&semantic_key)
            .copied()
        {
            return self.reuse_terminal(index, input, context, track_hierarchical_port);
        }
        self.append_new_terminal(semantic_key, input, context, track_hierarchical_port)
    }

    fn append_new_terminal(
        &mut self,
        semantic_key: TerminalSemanticKey,
        input: TerminalInput<'_>,
        context: &mut TerminalContext<'_>,
        track_hierarchical_port: bool,
    ) -> Result<(), SourceBundleError> {
        ensure_capacity(
            self.graph.terminal_occurrences.len(),
            self.limits.max_terminal_occurrences,
            "compiled graph terminal-occurrence count",
        )?;
        let component_ref = input.component_ref.unwrap_or_default();
        self.budget.reserve([
            "sch.terminal_occurrence".len(),
            UUID_TEXT_BYTES * 3,
            input.source_uuid.len(),
            input.source_subobject.len(),
            input.name.len(),
            input.pin_designator.len(),
            component_ref.len(),
        ])?;
        let source_identity = source_identity(
            &self
                .index
                .occurrence(context.occurrence_index)
                .ok_or_else(|| graph_error("compiled graph terminal occurrence is missing"))?
                .legacy_address,
            input.source_uuid,
            "",
            input.source_subobject,
            "",
            "",
        );
        let mapping = source_identity_mapping(&source_identity);
        let owner_refs = [context.owner.page.clone(), component_ref.to_owned()];
        let id = self
            .allocator
            .allocate_source("sch.terminal_occurrence", &mapping, &owner_refs)
            .map_err(identity_error)?;
        let diagnostics = terminal_diagnostics(input.role, input.component_ref.is_some());
        let index = self.graph.terminal_occurrences.len();
        self.graph.terminal_occurrences.push(TerminalOccurrence {
            component_occurrence_ref: input.component_ref.map(str::to_owned),
            design_component_pin_ref: None,
            design_net_ref: None,
            id: id.clone(),
            local_net_occurrence_ref: None,
            name: input.name.to_owned(),
            page_occurrence_ref: context.owner.page.clone(),
            pin_designator: input.pin_designator.to_owned(),
            resolution_diagnostics: diagnostics,
            role: input.role,
            source_identity,
            type_: "sch.terminal_occurrence".to_owned(),
        });
        context
            .terminal_state
            .terminal_by_semantic_source
            .insert(semantic_key, index);
        context.state.terminal_by_source.insert(
            (
                context.occurrence_index,
                input.role,
                input.source_uuid.to_owned(),
            ),
            id.clone(),
        );
        context.state.terminal_index_by_id.insert(id.clone(), index);
        context.collection.indices.push(index);
        if track_hierarchical_port {
            context
                .state
                .hierarchical_port_by_name
                .entry((context.occurrence_index, input.name.to_owned()))
                .or_insert_with(|| id.clone());
        }
        if !input.element_id.is_empty() {
            context
                .collection
                .element_ids
                .insert(input.element_id.to_owned());
            self.append_graphical_link(
                &context.owner.page,
                GraphicalTargetType::SchTerminalOccurrence,
                &id,
                input.element_id,
            )?;
        }
        Ok(())
    }

    fn reuse_terminal(
        &mut self,
        index: usize,
        input: TerminalInput<'_>,
        context: &mut TerminalContext<'_>,
        track_hierarchical_port: bool,
    ) -> Result<(), SourceBundleError> {
        let terminal = &mut self.graph.terminal_occurrences[index];
        if terminal.local_net_occurrence_ref.is_some() {
            return Err(graph_error(
                "one semantic terminal source resolves to multiple local nets",
            ));
        }
        if !input.name.is_empty()
            && (terminal.name.is_empty() || input.name < terminal.name.as_str())
        {
            self.budget.reserve([input.name.len()])?;
            terminal.name = input.name.to_owned();
        }
        context.state.terminal_by_source.insert(
            (
                context.occurrence_index,
                input.role,
                input.source_uuid.to_owned(),
            ),
            terminal.id.clone(),
        );
        context.collection.indices.push(index);
        if !input.element_id.is_empty() {
            context
                .collection
                .element_ids
                .insert(input.element_id.to_owned());
        }
        if track_hierarchical_port {
            context
                .state
                .hierarchical_port_by_name
                .entry((context.occurrence_index, input.name.to_owned()))
                .or_insert_with(|| terminal.id.clone());
        }
        Ok(())
    }

    fn append_local_net(&mut self, input: LocalNetInput<'_>) -> Result<(), SourceBundleError> {
        ensure_capacity(
            self.graph.local_net_occurrences.len(),
            self.limits.max_local_net_occurrences,
            "compiled graph local-net-occurrence count",
        )?;
        let display_name = input
            .design_net
            .map(|net| net.name.as_str())
            .or(input.unmaterialized_name)
            .unwrap_or(input.subgraph.chosen_name.as_str());
        let aliases = input
            .subgraph
            .label_drivers
            .iter()
            .filter_map(|driver| (!driver.text.is_empty()).then_some(driver.text.as_str()))
            .collect::<BTreeSet<_>>();
        let source_record = input
            .design_net
            .map_or_else(String::new, |net| format!("net-uid:{:012x}", net.code));
        let occurrence = self
            .index
            .occurrence(input.occurrence_index)
            .ok_or_else(|| graph_error("compiled graph local-net occurrence is missing"))?;
        let alias_bytes = aliases.iter().map(|value| value.len()).sum::<usize>();
        self.budget.reserve([
            "sch.local_net_occurrence".len(),
            UUID_TEXT_BYTES * 2,
            display_name.len() * 2,
            occurrence.legacy_address.len(),
            source_record.len(),
            alias_bytes,
        ])?;
        let mut identity = IdentityMapping::from([(
            "page_occurrence_ref".to_owned(),
            Value::String(input.owner.page.clone()),
        )]);
        if input.terminal_refs.is_empty() {
            identity.insert(
                "graphical_selectors".to_owned(),
                Value::Array(
                    input
                        .graphical_elements
                        .iter()
                        .map(|element| {
                            Value::String(format!("{DRAWING_ARTIFACT_KEY}\x1f{element}"))
                        })
                        .collect(),
                ),
            );
        } else {
            identity.insert(
                "terminal_occurrence_refs".to_owned(),
                Value::Array(
                    input
                        .terminal_refs
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
        }
        let id = self
            .allocator
            .allocate_derived("sch.local_net_occurrence", &identity)
            .map_err(identity_error)?;
        let source_identity =
            source_identity(&occurrence.legacy_address, "", &source_record, "", "", "");
        self.graph.local_net_occurrences.push(LocalNetOccurrence {
            aliases: aliases.into_iter().map(str::to_owned).collect(),
            design_net_ref: None,
            display_name: display_name.to_owned(),
            id: id.clone(),
            page_occurrence_ref: input.owner.page.clone(),
            qualified_name: Some(display_name.to_owned()),
            source_identity,
            type_: "sch.local_net_occurrence".to_owned(),
        });
        for index in input.terminal_indices {
            self.graph.terminal_occurrences[*index].local_net_occurrence_ref = Some(id.clone());
        }
        for element_id in input.graphical_elements {
            self.append_graphical_link(
                &input.owner.page,
                GraphicalTargetType::SchLocalNetOccurrence,
                &id,
                &element_id,
            )?;
        }
        Ok(())
    }

    fn append_hierarchy_bindings(
        &mut self,
        bindings: &[SchematicHierarchyNetBinding],
        state: &mut ConnectivityState,
    ) -> Result<(), SourceBundleError> {
        let hierarchy_by_child = self
            .graph
            .hierarchy_occurrences
            .iter()
            .map(|row| (row.child_unit_occurrence_ref.clone(), row.id.clone()))
            .collect::<HashMap<_, _>>();
        let mut bound_children = HashSet::new();
        for binding in bindings {
            let parent_ref = state
                .terminal_by_source
                .get(&(
                    binding.parent_occurrence_index,
                    TerminalRole::SheetEntry,
                    binding.sheet_pin_uuid.clone(),
                ))
                .cloned();
            let child_ref = binding.hierarchical_label_uuid.as_ref().and_then(|uuid| {
                state
                    .terminal_by_source
                    .get(&(
                        binding.child_occurrence_index,
                        TerminalRole::Port,
                        uuid.clone(),
                    ))
                    .cloned()
            });
            let (parent_ref, child_ref) = match (parent_ref, child_ref) {
                (Some(parent_ref), Some(child_ref)) => (parent_ref, child_ref),
                (Some(parent_ref), None) => {
                    self.mark_hierarchy_unresolved(&parent_ref, state);
                    continue;
                }
                (None, _) => continue,
            };
            let child_unit = self
                .occurrences
                .get(binding.child_occurrence_index.saturating_sub(1))
                .ok_or_else(|| graph_error("compiled graph binding child occurrence is missing"))?
                .unit
                .clone();
            let hierarchy_ref = hierarchy_by_child
                .get(&child_unit)
                .ok_or_else(|| graph_error("compiled graph binding hierarchy is missing"))?
                .clone();
            self.append_hierarchy_binding_row(binding, &parent_ref, &child_ref, &hierarchy_ref)?;
            bound_children.insert(child_ref);
        }
        for child_ref in state
            .hierarchical_port_by_name
            .values()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|child_ref| !bound_children.contains(*child_ref))
        {
            self.mark_hierarchy_unresolved(child_ref, state);
        }
        Ok(())
    }

    fn append_hierarchy_binding_row(
        &mut self,
        binding: &SchematicHierarchyNetBinding,
        parent_ref: &str,
        child_ref: &str,
        hierarchy_ref: &str,
    ) -> Result<(), SourceBundleError> {
        ensure_capacity(
            self.graph.hierarchy_terminal_bindings.len(),
            self.limits.max_hierarchy_terminal_bindings,
            "compiled graph hierarchy-terminal-binding count",
        )?;
        self.budget.reserve([
            "sch.hierarchy_terminal_binding".len(),
            UUID_TEXT_BYTES * 4,
            binding.sheet_pin_uuid.len(),
            binding.sheet_pin_name.len(),
        ])?;
        let identity = IdentityMapping::from([
            (
                "child_terminal_occurrence_ref".to_owned(),
                Value::String(child_ref.to_owned()),
            ),
            (
                "hierarchy_occurrence_ref".to_owned(),
                Value::String(hierarchy_ref.to_owned()),
            ),
            (
                "parent_terminal_occurrence_ref".to_owned(),
                Value::String(parent_ref.to_owned()),
            ),
        ]);
        let id = self
            .allocator
            .allocate_derived("sch.hierarchy_terminal_binding", &identity)
            .map_err(identity_error)?;
        self.graph
            .hierarchy_terminal_bindings
            .push(HierarchyTerminalBinding {
                child_terminal_occurrence_ref: child_ref.to_owned(),
                design_net_ref: None,
                hierarchy_occurrence_ref: hierarchy_ref.to_owned(),
                id,
                parent_terminal_occurrence_ref: parent_ref.to_owned(),
                source_identity: source_identity(
                    "",
                    &binding.sheet_pin_uuid,
                    "",
                    &binding.sheet_pin_name,
                    "",
                    "",
                ),
                type_: "sch.hierarchy_terminal_binding".to_owned(),
            });
        Ok(())
    }

    fn mark_hierarchy_unresolved(&mut self, terminal_ref: &str, state: &ConnectivityState) {
        let Some(index) = state.terminal_index_by_id.get(terminal_ref).copied() else {
            return;
        };
        let terminal = &mut self.graph.terminal_occurrences[index];
        if !terminal
            .resolution_diagnostics
            .contains(&ResolutionDiagnostic::HierarchyTerminalBindingUnresolved)
        {
            terminal
                .resolution_diagnostics
                .push(ResolutionDiagnostic::HierarchyTerminalBindingUnresolved);
        }
    }

    fn has_parent_occurrence(&self, occurrence_index: usize) -> bool {
        self.index
            .occurrence(occurrence_index)
            .is_some_and(|occurrence| occurrence.parent_index.is_some())
    }
}

fn pin_element_counts(subgraphs: &[SchematicWireSubgraph]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for element_id in subgraphs
        .iter()
        .flat_map(|subgraph| &subgraph.pin_drivers)
        .map(|pin| pin.pin_svg_id.as_str())
        .filter(|value| !value.is_empty())
    {
        *counts.entry(element_id.to_owned()).or_insert(0) += 1;
    }
    counts
}

fn unique_pin_element<'a>(
    pin: &'a crate::SchematicPinDriver,
    counts: &HashMap<String, usize>,
) -> &'a str {
    if counts.get(&pin.pin_svg_id) == Some(&1) {
        &pin.pin_svg_id
    } else {
        ""
    }
}

fn local_graphical_elements(
    graphical: &SchematicGraphicalIds,
    terminal_element_ids: &HashSet<String>,
) -> Vec<String> {
    graphical
        .wires
        .iter()
        .chain(&graphical.junctions)
        .chain(&graphical.labels)
        .filter(|value| !terminal_element_ids.contains(value.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn terminal_diagnostics(role: TerminalRole, component_resolved: bool) -> Vec<ResolutionDiagnostic> {
    let mut diagnostics = Vec::new();
    if role == TerminalRole::ComponentPin {
        if !component_resolved {
            diagnostics.push(ResolutionDiagnostic::ComponentOccurrenceUnresolved);
        }
        diagnostics.push(ResolutionDiagnostic::LogicalPinUnresolved);
    }
    diagnostics.push(ResolutionDiagnostic::DesignNetUnresolved);
    diagnostics
}

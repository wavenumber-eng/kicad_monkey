use crate::schematic_netlist::name_schematic_subgraph;
use crate::{
    SchematicBundleIndex, SchematicDriverPriority, SchematicPinDriver, SchematicSubpartSettings,
    SchematicWireDriverKind, SchematicWireSubgraph, SourceBundleError, SourceBundleErrorKind,
};
use std::collections::{HashMap, HashSet};
mod bus_promotion;
mod driver_choice;
mod driver_storage;
mod occurrence_compile;
mod sheet_pin_targets;
mod types;
mod union_find;
use bus_promotion::{BusPromotion, collect_design_bus_aliases, promote_bus_members};
use driver_choice::{DriverChoice, checked_join, consider_choice, sheet_depth};
use driver_storage::merged_driver_shape;
use occurrence_compile::{DesignCompilationBudget, compile_design_occurrence};
use sheet_pin_targets::SheetPinTargetIndex;
pub use types::{
    SchematicDesignNet, SchematicDesignNetLimits, SchematicDesignNetMember,
    SchematicDesignNetTerminal, SchematicHierarchyNetBinding, SchematicScalarDesignNetlist,
};
use union_find::UnionFind;

pub fn build_schematic_scalar_design_nets(
    index: &SchematicBundleIndex,
    code_offset: u64,
    limits: SchematicDesignNetLimits,
) -> Result<SchematicScalarDesignNetlist, SourceBundleError> {
    build_schematic_scalar_design_nets_with_settings(
        index,
        code_offset,
        SchematicSubpartSettings::default(),
        limits,
    )
}

pub fn build_schematic_scalar_design_nets_with_settings(
    index: &SchematicBundleIndex,
    code_offset: u64,
    subparts: SchematicSubpartSettings,
    limits: SchematicDesignNetLimits,
) -> Result<SchematicScalarDesignNetlist, SourceBundleError> {
    Ok(build_schematic_compiled_design(index, code_offset, subparts, limits)?.netlist)
}

pub(crate) fn build_schematic_compiled_design(
    index: &SchematicBundleIndex,
    code_offset: u64,
    subparts: SchematicSubpartSettings,
    limits: SchematicDesignNetLimits,
) -> Result<SchematicCompiledDesign, SourceBundleError> {
    DesignBuilder::new(index, code_offset, subparts, limits)?.build()
}

pub(crate) struct SchematicCompiledDesign {
    pub(crate) netlist: SchematicScalarDesignNetlist,
    pub(crate) occurrences: Vec<CompiledOccurrence>,
}

pub(crate) struct CompiledOccurrence {
    pub(crate) occurrence_index: usize,
    source_path: String,
    human_address: String,
    legacy_address: String,
    pub(crate) subgraphs: Vec<SchematicWireSubgraph>,
    coord_to_subgraph: HashMap<crate::SchematicPoint, usize>,
    bus_subgraphs: Vec<crate::SchematicBusSubgraph>,
    bus_coord_to_subgraph: HashMap<crate::SchematicPoint, usize>,
    bus_member_wire_subgraphs: Vec<Vec<Option<usize>>>,
}

struct OutputShape {
    members: usize,
    terminals: usize,
}

struct DesignBuilder<'a> {
    index: &'a SchematicBundleIndex,
    code_offset: u64,
    limits: SchematicDesignNetLimits,
    sheet_pin_targets: SheetPinTargetIndex,
    design_bus_aliases: HashMap<&'a str, &'a [String]>,
    compiled: Vec<CompiledOccurrence>,
    flat: Vec<(usize, usize)>,
    offsets: Vec<usize>,
    union: UnionFind,
    union_work: usize,
    retained_string_bytes: usize,
}

impl<'a> DesignBuilder<'a> {
    fn new(
        index: &'a SchematicBundleIndex,
        code_offset: u64,
        subparts: SchematicSubpartSettings,
        limits: SchematicDesignNetLimits,
    ) -> Result<Self, SourceBundleError> {
        let sheet_pin_targets = SheetPinTargetIndex::build(index, limits)?;
        let design_bus_aliases = collect_design_bus_aliases(index, limits)?;
        let mut compiled = Vec::with_capacity(index.occurrences().len());
        let mut budget = DesignCompilationBudget::new();
        for occurrence in index.occurrences() {
            compiled.push(compile_design_occurrence(
                index,
                occurrence,
                subparts,
                limits,
                &design_bus_aliases,
                &mut budget,
            )?);
        }
        let mut flat = Vec::with_capacity(budget.subgraph_count());
        let mut offsets = Vec::with_capacity(compiled.len() + 1);
        for (occurrence_array_index, occurrence) in compiled.iter().enumerate() {
            offsets.push(flat.len());
            flat.extend(
                (0..occurrence.subgraphs.len())
                    .map(|subgraph_index| (occurrence_array_index, subgraph_index)),
            );
        }
        offsets.push(flat.len());
        Ok(Self {
            index,
            code_offset,
            limits,
            sheet_pin_targets,
            design_bus_aliases,
            compiled,
            union: UnionFind::new(flat.len()),
            flat,
            offsets,
            union_work: 0,
            retained_string_bytes: 0,
        })
    }

    fn build(mut self) -> Result<SchematicCompiledDesign, SourceBundleError> {
        let hierarchy_bindings = self.bind_hierarchy()?;
        self.merge_global_labels()?;
        self.merge_global_power()?;
        let promotion = promote_bus_members(
            &self.compiled,
            self.index,
            &self.offsets,
            &self.design_bus_aliases,
            self.limits,
        )?;
        for &(left, right) in &promotion.wire_unions {
            self.bounded_union(left, right)?;
        }
        let groups = self.ordered_groups();
        self.preflight_output_shape(&groups)?;
        let suffix_indices = self.sheet_pin_suffix_indices()?;
        let nets = self.materialize(groups, &suffix_indices, &promotion)?;
        Ok(SchematicCompiledDesign {
            netlist: SchematicScalarDesignNetlist {
                nets,
                hierarchy_bindings,
            },
            occurrences: self.compiled,
        })
    }

    fn flat_index(&self, occurrence_array_index: usize, subgraph_index: usize) -> usize {
        self.offsets[occurrence_array_index] + subgraph_index
    }

    fn bounded_union(&mut self, left: usize, right: usize) -> Result<(), SourceBundleError> {
        self.union_work = self
            .union_work
            .checked_add(1)
            .ok_or_else(|| self.limit_error("design union work overflows"))?;
        if self.union_work > self.limits.max_union_work {
            return Err(self.limit_error("design union work exceeds its limit"));
        }
        self.union.union(left, right);
        Ok(())
    }

    fn bind_hierarchy(&mut self) -> Result<Vec<SchematicHierarchyNetBinding>, SourceBundleError> {
        let mut bindings = Vec::new();
        let mut unions = Vec::new();
        let mut binding_string_bytes = 0_usize;
        for child_array_index in 0..self.compiled.len() {
            self.collect_child_bindings(
                child_array_index,
                &mut bindings,
                &mut unions,
                &mut binding_string_bytes,
            )?;
        }
        for (left, right) in unions {
            self.bounded_union(left, right)?;
        }
        self.retained_string_bytes += binding_string_bytes;
        Ok(bindings)
    }

    fn collect_child_bindings(
        &self,
        child_array_index: usize,
        bindings: &mut Vec<SchematicHierarchyNetBinding>,
        unions: &mut Vec<(usize, usize)>,
        binding_string_bytes: &mut usize,
    ) -> Result<(), SourceBundleError> {
        let occurrence_index = self.compiled[child_array_index].occurrence_index;
        let occurrence = self.index.occurrence(occurrence_index).ok_or_else(|| {
            self.error("compiled schematic occurrence disappeared from its index")
        })?;
        let (Some(parent_index), Some(parent_sheet_index)) =
            (occurrence.parent_index, occurrence.parent_sheet_index)
        else {
            return Ok(());
        };
        let parent_array_index = parent_index - 1;
        let parent_definition = self
            .index
            .definition(&self.compiled[parent_array_index].source_path)
            .ok_or_else(|| self.error("parent schematic definition is missing"))?;
        let sheet = parent_definition
            .sheets
            .get(parent_sheet_index)
            .ok_or_else(|| self.error("hierarchy occurrence parent sheet index is out of range"))?;
        let child_by_name = self.child_hierarchical_labels(child_array_index)?;
        for pin in &sheet.pins {
            if bindings.len() >= self.limits.max_hierarchy_bindings {
                return Err(self.limit_error("hierarchy binding count exceeds its limit"));
            }
            let parent_subgraph = self.compiled[parent_array_index]
                .coord_to_subgraph
                .get(&pin.at)
                .copied();
            let child_match = child_by_name.get(pin.name.as_str()).copied();
            self.push_binding_union(
                parent_array_index,
                child_array_index,
                parent_subgraph,
                child_match,
                unions,
            )?;
            let child_uuid = child_match.map(|(_, uuid)| uuid);
            let sheet_pin_id =
                crate::schematic_connectivity::schematic_sheet_pin_group_id(sheet, pin);
            let added = pin
                .name
                .len()
                .checked_add(sheet_pin_id.len())
                .and_then(|bytes| bytes.checked_add(child_uuid.unwrap_or_default().len()))
                .ok_or_else(|| self.limit_error("design retained string bytes overflow"))?;
            *binding_string_bytes = binding_string_bytes
                .checked_add(added)
                .ok_or_else(|| self.limit_error("design retained string bytes overflow"))?;
            self.ensure_retained_add(*binding_string_bytes)?;
            bindings.push(SchematicHierarchyNetBinding {
                parent_occurrence_index: parent_index,
                child_occurrence_index: occurrence_index,
                sheet_pin_name: pin.name.clone(),
                sheet_pin_uuid: sheet_pin_id,
                hierarchical_label_uuid: child_uuid.map(str::to_owned),
                parent_subgraph_index: parent_subgraph,
                child_subgraph_index: child_match.map(|(subgraph, _)| subgraph),
            });
        }
        Ok(())
    }

    fn child_hierarchical_labels(
        &self,
        child_array_index: usize,
    ) -> Result<HashMap<&str, (usize, &str)>, SourceBundleError> {
        let mut by_name = HashMap::new();
        for (subgraph_index, subgraph) in self.compiled[child_array_index]
            .subgraphs
            .iter()
            .enumerate()
        {
            for label in &subgraph.label_drivers {
                if label.kind != SchematicWireDriverKind::HierarchicalLabel
                    || by_name.contains_key(label.text.as_str())
                {
                    continue;
                }
                if by_name.len() >= self.limits.max_merge_keys {
                    return Err(
                        self.limit_error("hierarchy label merge-key count exceeds its limit")
                    );
                }
                by_name.insert(
                    label.text.as_str(),
                    (subgraph_index, label.source_uuid.as_str()),
                );
            }
        }
        Ok(by_name)
    }

    fn push_binding_union(
        &self,
        parent_occurrence: usize,
        child_occurrence: usize,
        parent_subgraph: Option<usize>,
        child_match: Option<(usize, &str)>,
        unions: &mut Vec<(usize, usize)>,
    ) -> Result<(), SourceBundleError> {
        let (Some(parent_subgraph), Some((child_subgraph, _))) = (parent_subgraph, child_match)
        else {
            return Ok(());
        };
        self.ensure_pending_union_capacity(unions.len())?;
        unions.push((
            self.flat_index(parent_occurrence, parent_subgraph),
            self.flat_index(child_occurrence, child_subgraph),
        ));
        Ok(())
    }

    fn merge_global_labels(&mut self) -> Result<(), SourceBundleError> {
        let mut first_by_name = HashMap::<&str, usize>::new();
        let mut unions = Vec::new();
        for (flat_index, (occurrence_index, subgraph_index)) in self.flat.iter().enumerate() {
            for label in &self.compiled[*occurrence_index].subgraphs[*subgraph_index].label_drivers
            {
                if label.kind != SchematicWireDriverKind::GlobalLabel {
                    continue;
                }
                if let Some(first) = first_by_name.get(label.text.as_str()) {
                    self.ensure_pending_union_capacity(unions.len())?;
                    unions.push((*first, flat_index));
                } else {
                    if first_by_name.len() >= self.limits.max_merge_keys {
                        return Err(
                            self.limit_error("global-label merge-key count exceeds its limit")
                        );
                    }
                    first_by_name.insert(label.text.as_str(), flat_index);
                }
            }
        }
        for (left, right) in unions {
            self.bounded_union(left, right)?;
        }
        Ok(())
    }

    fn merge_global_power(&mut self) -> Result<(), SourceBundleError> {
        let mut first_by_name = HashMap::<&str, usize>::new();
        let mut unions = Vec::new();
        for (flat_index, (occurrence_index, subgraph_index)) in self.flat.iter().enumerate() {
            for pin in &self.compiled[*occurrence_index].subgraphs[*subgraph_index].pin_drivers {
                if !pin.is_power || pin.priority != SchematicDriverPriority::GlobalPowerPin {
                    continue;
                }
                if let Some(first) = first_by_name.get(pin.power_value.as_str()) {
                    self.ensure_pending_union_capacity(unions.len())?;
                    unions.push((*first, flat_index));
                } else {
                    if first_by_name.len() >= self.limits.max_merge_keys {
                        return Err(
                            self.limit_error("global-power merge-key count exceeds its limit")
                        );
                    }
                    first_by_name.insert(pin.power_value.as_str(), flat_index);
                }
            }
        }
        for (left, right) in unions {
            self.bounded_union(left, right)?;
        }
        Ok(())
    }

    fn ordered_groups(&mut self) -> Vec<Vec<usize>> {
        let mut group_index_by_root = HashMap::new();
        let mut groups = Vec::<Vec<usize>>::new();
        for flat_index in 0..self.flat.len() {
            let root = self.union.find(flat_index);
            let group_index = *group_index_by_root.entry(root).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[group_index].push(flat_index);
        }
        groups
    }

    fn preflight_output_shape(&self, groups: &[Vec<usize>]) -> Result<(), SourceBundleError> {
        let mut net_count = 0_usize;
        let mut member_count = 0_usize;
        let mut terminal_count = 0_usize;
        for group in groups {
            let Some(shape) = self.group_output_shape(group)? else {
                continue;
            };
            net_count = net_count
                .checked_add(1)
                .ok_or_else(|| self.limit_error("design net count overflows"))?;
            member_count = member_count
                .checked_add(shape.members)
                .ok_or_else(|| self.limit_error("design net member count overflows"))?;
            terminal_count = terminal_count
                .checked_add(shape.terminals)
                .ok_or_else(|| self.limit_error("design terminal count overflows"))?;
        }
        if net_count > self.limits.max_nets {
            return Err(self.limit_error("design net count exceeds its limit"));
        }
        if member_count > self.limits.max_net_members {
            return Err(self.limit_error("design net member count exceeds its limit"));
        }
        if terminal_count > self.limits.max_terminals {
            return Err(self.limit_error("design terminal count exceeds its limit"));
        }
        if net_count != 0 {
            let last = u64::try_from(net_count - 1)
                .map_err(|_| self.limit_error("design net code exceeds the platform size"))?;
            self.code_offset
                .checked_add(last)
                .ok_or_else(|| self.limit_error("design net code overflows"))?;
        }
        Ok(())
    }

    fn group_output_shape(
        &self,
        group: &[usize],
    ) -> Result<Option<OutputShape>, SourceBundleError> {
        let mut terminals = HashSet::new();
        let mut has_driver = false;
        let mut driver_count = 0_usize;
        for &flat_index in group {
            let (occurrence, subgraph) = self.flat[flat_index];
            let subgraph = &self.compiled[occurrence].subgraphs[subgraph];
            has_driver |= !subgraph.pin_drivers.is_empty() || !subgraph.label_drivers.is_empty();
            driver_count = driver_count
                .checked_add(subgraph.pin_drivers.len())
                .and_then(|value| value.checked_add(subgraph.label_drivers.len()))
                .ok_or_else(|| self.limit_error("merged design driver count overflows"))?;
            if driver_count > self.limits.max_drivers_per_net {
                return Err(self.limit_error("merged design driver count exceeds its limit"));
            }
            terminals.extend(
                subgraph
                    .pin_drivers
                    .iter()
                    .filter(|pin| !pin.reference.is_empty() && !pin.reference.starts_with('#'))
                    .map(|pin| (pin.reference.as_str(), pin.pin_number.as_str())),
            );
        }
        Ok(
            (has_driver && !terminals.is_empty()).then_some(OutputShape {
                members: group.len(),
                terminals: terminals.len(),
            }),
        )
    }

    fn materialize(
        &mut self,
        groups: Vec<Vec<usize>>,
        suffix_indices: &HashMap<(usize, usize), usize>,
        promotion: &BusPromotion,
    ) -> Result<Vec<SchematicDesignNet>, SourceBundleError> {
        let mut nets = Vec::new();
        let mut seen_synthetic_sheet_pin_names = HashMap::<String, usize>::new();
        let mut synthetic_sheet_pin_name_bytes = 0_usize;
        for group in groups {
            let terminal_refs = self.terminal_refs(&group);
            if terminal_refs.is_empty() {
                continue;
            }
            let (mut merged, choice) = self.merged_subgraph(&group, promotion)?;
            let Some(choice) = choice else {
                continue;
            };
            merged.chosen_name = choice.raw_name;
            merged.chosen_priority = choice.priority;
            merged.chosen_kind = Some(choice.kind);
            let source_path = &self.compiled[self.flat[group[0]].0].source_path;
            let (mut name, auto_named) = name_schematic_subgraph(
                source_path,
                &choice.sheet_path,
                &merged,
                self.limits.max_name_bytes,
            )?;
            self.apply_sheet_pin_suffix(
                &mut name,
                choice.kind,
                choice.sheet_pin_key,
                suffix_indices,
                &mut seen_synthetic_sheet_pin_names,
                &mut synthetic_sheet_pin_name_bytes,
            )?;
            self.retain_output_bytes(name.len())?;
            let terminals = self.materialize_terminals(terminal_refs)?;
            let members = group
                .iter()
                .map(|flat_index| {
                    let (occurrence, subgraph_index) = self.flat[*flat_index];
                    SchematicDesignNetMember {
                        occurrence_index: self.compiled[occurrence].occurrence_index,
                        subgraph_index,
                    }
                })
                .collect();
            let code =
                self.code_offset
                    .checked_add(u64::try_from(nets.len()).map_err(|_| {
                        self.limit_error("design net code exceeds the platform size")
                    })?)
                    .ok_or_else(|| self.limit_error("design net code overflows"))?;
            nets.push(SchematicDesignNet {
                name,
                code,
                driver_priority: merged.chosen_priority,
                driver_kind: merged.chosen_kind,
                auto_named,
                members,
                terminals,
            });
        }
        Ok(nets)
    }

    fn apply_sheet_pin_suffix(
        &self,
        name: &mut String,
        kind: SchematicWireDriverKind,
        sheet_pin_key: Option<(usize, usize)>,
        suffix_indices: &HashMap<(usize, usize), usize>,
        seen_synthetic_names: &mut HashMap<String, usize>,
        synthetic_name_bytes: &mut usize,
    ) -> Result<(), SourceBundleError> {
        if kind != SchematicWireDriverKind::SheetPin {
            return Ok(());
        }
        let suffix = if let Some(key) = sheet_pin_key {
            suffix_indices
                .get(&key)
                .copied()
                .filter(|value| *value != 0)
        } else {
            if !seen_synthetic_names.contains_key(name.as_str()) {
                *synthetic_name_bytes = synthetic_name_bytes
                    .checked_add(name.len())
                    .ok_or_else(|| self.limit_error("design work string bytes overflow"))?;
                if *synthetic_name_bytes > self.limits.max_work_string_bytes {
                    return Err(self.limit_error("design work string bytes exceed their limit"));
                }
            }
            let next = seen_synthetic_names.entry(name.clone()).or_default();
            let suffix = (*next != 0).then_some(*next);
            *next = next
                .checked_add(1)
                .ok_or_else(|| self.limit_error("sheet-pin suffix count overflows"))?;
            suffix
        };
        let Some(suffix) = suffix else {
            return Ok(());
        };
        let suffix = format!("_{suffix}");
        let final_bytes = name
            .len()
            .checked_add(suffix.len())
            .ok_or_else(|| self.limit_error("design net name bytes overflow"))?;
        self.ensure_name_bytes(final_bytes)?;
        name.push_str(&suffix);
        Ok(())
    }

    fn terminal_refs(&self, group: &[usize]) -> Vec<PinLocator> {
        let mut pins = Vec::new();
        for &flat_index in group {
            let (occurrence, subgraph_index) = self.flat[flat_index];
            pins.extend(
                self.compiled[occurrence].subgraphs[subgraph_index]
                    .pin_drivers
                    .iter()
                    .enumerate()
                    .filter(|(_, pin)| !pin.reference.is_empty() && !pin.reference.starts_with('#'))
                    .map(|(pin_index, _)| PinLocator {
                        occurrence,
                        subgraph_index,
                        pin_index,
                    }),
            );
        }
        pins.sort_by(|left, right| {
            let left = left.pin(self);
            let right = right.pin(self);
            left.reference
                .cmp(&right.reference)
                .then_with(|| left.pin_number.cmp(&right.pin_number))
        });
        pins.dedup_by(|left, right| {
            let left = left.pin(self);
            let right = right.pin(self);
            left.reference == right.reference && left.pin_number == right.pin_number
        });
        pins
    }

    fn materialize_terminals(
        &mut self,
        pins: Vec<PinLocator>,
    ) -> Result<Vec<SchematicDesignNetTerminal>, SourceBundleError> {
        let mut terminals = Vec::with_capacity(pins.len());
        for locator in pins {
            let bytes = {
                let pin = locator.pin(self);
                let svg_id = if pin.pin_svg_id.is_empty() {
                    &pin.symbol_uuid
                } else {
                    &pin.pin_svg_id
                };
                let sheet_path = &self.compiled[locator.occurrence].legacy_address;
                [
                    pin.reference.len(),
                    pin.pin_number.len(),
                    pin.pin_name.len(),
                    pin.electrical_type.len(),
                    sheet_path.len(),
                    pin.source_pin_uuid.len(),
                    svg_id.len(),
                ]
                .into_iter()
                .try_fold(0_usize, usize::checked_add)
                .ok_or_else(|| self.limit_error("design retained string bytes overflow"))?
            };
            self.retain_output_bytes(bytes)?;
            let pin = locator.pin(self);
            let svg_id = if pin.pin_svg_id.is_empty() {
                &pin.symbol_uuid
            } else {
                &pin.pin_svg_id
            };
            let sheet_path = &self.compiled[locator.occurrence].legacy_address;
            terminals.push(SchematicDesignNetTerminal {
                occurrence_index: self.compiled[locator.occurrence].occurrence_index,
                symbol_index: pin.symbol_index,
                designator: pin.reference.clone(),
                pin: pin.pin_number.clone(),
                pin_name: pin.pin_name.clone(),
                pin_type: pin.electrical_type.clone(),
                sheet_path: sheet_path.clone(),
                source_pin_id: pin.source_pin_uuid.clone(),
                svg_id: svg_id.clone(),
            });
        }
        Ok(terminals)
    }

    fn merged_subgraph(
        &self,
        group: &[usize],
        promotion: &BusPromotion,
    ) -> Result<(SchematicWireSubgraph, Option<DriverChoice>), SourceBundleError> {
        let choice = self.choose_driver(group, promotion)?;
        let choice_strings = choice.as_ref().map(|choice| {
            [
                choice.full_name.as_str(),
                choice.sheet_path.as_str(),
                choice.raw_name.as_str(),
            ]
        });
        let shape = merged_driver_shape(
            group.iter().map(|flat_index| {
                let (occurrence, subgraph_index) = self.flat[*flat_index];
                &self.compiled[occurrence].subgraphs[subgraph_index]
            }),
            choice_strings,
        )
        .ok_or_else(|| self.limit_error("merged design driver bytes overflow"))?;
        let driver_count = shape
            .pins
            .checked_add(shape.labels)
            .ok_or_else(|| self.limit_error("merged design driver count overflows"))?;
        if driver_count > self.limits.max_drivers_per_net {
            return Err(self.limit_error("merged design driver count exceeds its limit"));
        }
        if shape.bytes > self.limits.max_merged_driver_bytes {
            return Err(self.limit_error("merged design driver bytes exceed their limit"));
        }
        let mut merged = SchematicWireSubgraph {
            coords: Vec::new(),
            pin_drivers: Vec::with_capacity(shape.pins),
            label_drivers: Vec::with_capacity(shape.labels),
            graphical: Default::default(),
            chosen_name: String::new(),
            chosen_priority: SchematicDriverPriority::None,
            chosen_kind: None,
            no_connect: false,
        };
        for &flat_index in group {
            let (occurrence, subgraph_index) = self.flat[flat_index];
            let subgraph = &self.compiled[occurrence].subgraphs[subgraph_index];
            merged.no_connect |= subgraph.no_connect;
            merged
                .pin_drivers
                .extend(subgraph.pin_drivers.iter().cloned());
            merged
                .label_drivers
                .extend(subgraph.label_drivers.iter().cloned());
        }
        Ok((merged, choice))
    }

    fn choose_driver(
        &self,
        group: &[usize],
        promotion: &BusPromotion,
    ) -> Result<Option<DriverChoice>, SourceBundleError> {
        let mut best = None;
        let mut order = 0_usize;
        for &flat_index in group {
            let (occurrence, subgraph_index) = self.flat[flat_index];
            let compiled = &self.compiled[occurrence];
            let subgraph = &compiled.subgraphs[subgraph_index];
            for label in &subgraph.label_drivers {
                consider_choice(&mut best, self.label_choice(occurrence, label, order)?);
                order += 1;
            }
            for pin in &subgraph.pin_drivers {
                consider_choice(&mut best, self.pin_choice(occurrence, pin, order)?);
                order += 1;
            }
        }
        let mut override_order = 0_usize;
        for &flat_index in group {
            for value in promotion.overrides_for(flat_index) {
                consider_choice(
                    &mut best,
                    DriverChoice {
                        priority: value.priority.max(SchematicDriverPriority::LocalPowerPin),
                        depth: value.depth,
                        shape_rank: 1,
                        implicit: false,
                        full_name: value.text.clone(),
                        sheet_path: value.sheet_path.clone(),
                        order: override_order,
                        kind: value.kind,
                        raw_name: value.text.clone(),
                        sheet_pin_key: None,
                    },
                );
                override_order = override_order
                    .checked_add(1)
                    .ok_or_else(|| self.limit_error("design bus override order overflows"))?;
            }
        }
        Ok(best)
    }

    fn label_choice(
        &self,
        occurrence: usize,
        label: &crate::SchematicLabelDriver,
        order: usize,
    ) -> Result<DriverChoice, SourceBundleError> {
        let compiled = &self.compiled[occurrence];
        let sheet_path = self.label_sheet_path(occurrence, label);
        let full_name = if label.kind == SchematicWireDriverKind::GlobalLabel {
            label.text.clone()
        } else {
            checked_join(&sheet_path, &label.text, self.limits.max_name_bytes)
                .ok_or_else(|| self.limit_error("design driver name bytes exceed their limit"))?
        };
        Ok(DriverChoice {
            priority: label.priority,
            depth: sheet_depth(&compiled.human_address),
            shape_rank: usize::from(
                label.kind != SchematicWireDriverKind::SheetPin || label.shape != "output",
            ),
            implicit: false,
            full_name,
            sheet_path,
            order,
            kind: label.kind,
            raw_name: label.text.clone(),
            sheet_pin_key: (label.kind == SchematicWireDriverKind::SheetPin)
                .then_some((occurrence, label.source_order)),
        })
    }

    fn pin_choice(
        &self,
        occurrence: usize,
        pin: &SchematicPinDriver,
        order: usize,
    ) -> Result<DriverChoice, SourceBundleError> {
        let compiled = &self.compiled[occurrence];
        let raw_name = if pin.is_power && !pin.power_value.is_empty() {
            pin.power_value.clone()
        } else {
            format!("{}-{}", pin.reference, pin.pin_number)
        };
        self.ensure_name_bytes(raw_name.len())?;
        let power = pin.is_power
            && matches!(
                pin.priority,
                SchematicDriverPriority::GlobalPowerPin | SchematicDriverPriority::LocalPowerPin
            );
        let full_name = if power {
            raw_name.clone()
        } else {
            checked_join(
                &compiled.human_address,
                &raw_name,
                self.limits.max_name_bytes,
            )
            .ok_or_else(|| self.limit_error("design driver name bytes exceed their limit"))?
        };
        Ok(DriverChoice {
            priority: pin.priority,
            depth: sheet_depth(&compiled.human_address),
            shape_rank: 1,
            implicit: pin.is_implicit_hidden_power,
            full_name,
            sheet_path: compiled.human_address.clone(),
            order,
            kind: pin.kind,
            raw_name,
            sheet_pin_key: None,
        })
    }

    fn label_sheet_path(
        &self,
        occurrence_array_index: usize,
        label: &crate::SchematicLabelDriver,
    ) -> String {
        if label.kind == SchematicWireDriverKind::SheetPin {
            let parent_occurrence = self.compiled[occurrence_array_index].occurrence_index;
            if let Some(target) = self.sheet_pin_targets.target_path(parent_occurrence, label) {
                return target.to_owned();
            }
        }
        self.compiled[occurrence_array_index].human_address.clone()
    }

    fn sheet_pin_suffix_indices(
        &self,
    ) -> Result<HashMap<(usize, usize), usize>, SourceBundleError> {
        let mut grouped = HashMap::<String, Vec<(usize, usize, (usize, usize))>>::new();
        let mut fallback = 0_usize;
        let mut work_string_bytes = 0_usize;
        for occurrence in 0..self.compiled.len() {
            for subgraph in &self.compiled[occurrence].subgraphs {
                for label in &subgraph.label_drivers {
                    if label.kind != SchematicWireDriverKind::SheetPin {
                        continue;
                    }
                    let sheet_path = self.label_sheet_path(occurrence, label);
                    let fake = SchematicWireSubgraph {
                        coords: Vec::new(),
                        pin_drivers: Vec::new(),
                        label_drivers: Vec::new(),
                        graphical: Default::default(),
                        chosen_name: label.text.clone(),
                        chosen_priority: label.priority,
                        chosen_kind: Some(label.kind),
                        no_connect: false,
                    };
                    let (base, _) = name_schematic_subgraph(
                        &self.compiled[occurrence].source_path,
                        &sheet_path,
                        &fake,
                        self.limits.max_name_bytes,
                    )?;
                    work_string_bytes = work_string_bytes
                        .checked_add(base.len())
                        .ok_or_else(|| self.limit_error("design work string bytes overflow"))?;
                    if work_string_bytes > self.limits.max_work_string_bytes {
                        return Err(self.limit_error("design work string bytes exceed their limit"));
                    }
                    grouped.entry(base).or_default().push((
                        label.source_order,
                        fallback,
                        (occurrence, label.source_order),
                    ));
                    fallback = fallback
                        .checked_add(1)
                        .ok_or_else(|| self.limit_error("sheet-pin suffix order overflows"))?;
                }
            }
        }
        let mut result = HashMap::new();
        for entries in grouped.values_mut() {
            entries.sort_by_key(|value| (value.0, value.1));
            for (suffix, (_, _, key)) in entries.iter().enumerate() {
                result.insert(*key, suffix);
            }
        }
        Ok(result)
    }

    fn ensure_name_bytes(&self, bytes: usize) -> Result<(), SourceBundleError> {
        if bytes > self.limits.max_name_bytes {
            Err(self.limit_error("design net name bytes exceed their limit"))
        } else {
            Ok(())
        }
    }

    fn ensure_pending_union_capacity(&self, pending: usize) -> Result<(), SourceBundleError> {
        if pending >= self.limits.max_union_work.saturating_sub(self.union_work) {
            Err(self.limit_error("design union work exceeds its limit"))
        } else {
            Ok(())
        }
    }

    fn retain_output_bytes(&mut self, bytes: usize) -> Result<(), SourceBundleError> {
        self.ensure_retained_add(bytes)?;
        self.retained_string_bytes += bytes;
        Ok(())
    }

    fn ensure_retained_add(&self, bytes: usize) -> Result<(), SourceBundleError> {
        let total = self
            .retained_string_bytes
            .checked_add(bytes)
            .ok_or_else(|| self.limit_error("design retained string bytes overflow"))?;
        if total > self.limits.max_retained_string_bytes {
            return Err(self.limit_error("design retained string bytes exceed their limit"));
        }
        Ok(())
    }

    fn error(&self, message: &str) -> SourceBundleError {
        SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            self.compiled
                .first()
                .map(|value| value.source_path.as_str()),
            message,
        )
    }

    fn limit_error(&self, message: &str) -> SourceBundleError {
        limit_error(
            self.compiled
                .first()
                .map(|value| value.source_path.as_str()),
            message,
        )
    }
}

#[derive(Clone, Copy)]
struct PinLocator {
    occurrence: usize,
    subgraph_index: usize,
    pin_index: usize,
}

impl PinLocator {
    fn pin<'a>(&self, builder: &'a DesignBuilder<'_>) -> &'a SchematicPinDriver {
        &builder.compiled[self.occurrence].subgraphs[self.subgraph_index].pin_drivers
            [self.pin_index]
    }
}

fn limit_error(path: Option<&str>, message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, path, message)
}

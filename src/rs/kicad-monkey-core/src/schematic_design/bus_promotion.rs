use super::CompiledOccurrence;
use super::types::SchematicDesignNetLimits;
use super::union_find::UnionFind;
use crate::{
    SchematicBundleIndex, SchematicBusDriverKind, SchematicBusExpansionLimits,
    SchematicBusSubgraph, SchematicDriverPriority, SchematicPoint, SchematicWireDriverKind,
    SchematicWireSubgraph, SourceBundleError, SourceBundleErrorKind, canonical_bus_member_name,
    is_schematic_bus_label,
};
use std::collections::HashMap;

pub(super) struct BusCompilationBudget {
    subgraphs: usize,
    members: usize,
    indexed_coords: usize,
}

impl BusCompilationBudget {
    pub(super) fn new() -> Self {
        Self {
            subgraphs: 0,
            members: 0,
            indexed_coords: 0,
        }
    }

    pub(super) fn add_occurrence(
        &mut self,
        bus_subgraphs: &[SchematicBusSubgraph],
        limits: SchematicDesignNetLimits,
        source_path: &str,
    ) -> Result<(), SourceBundleError> {
        self.subgraphs = checked_total(
            self.subgraphs,
            bus_subgraphs.len(),
            limits.max_bus_subgraphs,
            source_path,
            "design bus subgraph count",
        )?;
        for bus in bus_subgraphs {
            self.members = checked_total(
                self.members,
                bus.members.len(),
                limits.max_bus_members,
                source_path,
                "design bus member count",
            )?;
            self.indexed_coords = checked_total(
                self.indexed_coords,
                bus.coords.len(),
                limits.max_bus_indexed_coords,
                source_path,
                "design bus indexed coordinate count",
            )?;
        }
        Ok(())
    }
}

pub(super) fn collect_design_bus_aliases(
    index: &SchematicBundleIndex,
    limits: SchematicDesignNetLimits,
) -> Result<HashMap<&str, &[String]>, SourceBundleError> {
    let mut aliases = HashMap::new();
    for occurrence in index.occurrences() {
        let definition = index.definition(&occurrence.source_path).ok_or_else(|| {
            schematic_error(
                Some(&occurrence.source_path),
                "design bus alias source definition is missing",
            )
        })?;
        for alias in &definition.bus_aliases {
            if !aliases.contains_key(alias.name.as_str())
                && aliases.len() >= limits.max_design_bus_aliases
            {
                return Err(limit_error(
                    Some(&occurrence.source_path),
                    "design bus alias count exceeds its limit",
                ));
            }
            aliases.insert(alias.name.as_str(), alias.members.as_slice());
        }
    }
    // KiCad 10 project aliases are authoritative over same-name legacy
    // schematic declarations, including aliases with no surviving members.
    for alias in index.project_bus_aliases() {
        if !aliases.contains_key(alias.name.as_str())
            && aliases.len() >= limits.max_design_bus_aliases
        {
            return Err(limit_error(
                None,
                "design bus alias count exceeds its limit",
            ));
        }
        aliases.insert(alias.name.as_str(), alias.members.as_slice());
    }
    Ok(aliases)
}

pub(super) fn index_bus_member_wires(
    wire_subgraphs: &[SchematicWireSubgraph],
    bus_subgraphs: &[SchematicBusSubgraph],
    coord_to_subgraph: &HashMap<SchematicPoint, usize>,
    limits: SchematicDesignNetLimits,
    source_path: &str,
) -> Result<Vec<Vec<Option<usize>>>, SourceBundleError> {
    let (label_to_subgraph, label_key_bytes) =
        index_wire_labels(wire_subgraphs, limits, source_path)?;
    let context = BusWireIndexContext {
        wire_subgraphs,
        coord_to_subgraph,
        label_to_subgraph: &label_to_subgraph,
        label_key_bytes,
        limits,
        source_path,
    };
    bus_subgraphs
        .iter()
        .map(|bus| context.index_bus(bus))
        .collect()
}

fn index_wire_labels(
    wire_subgraphs: &[SchematicWireSubgraph],
    limits: SchematicDesignNetLimits,
    source_path: &str,
) -> Result<(HashMap<String, usize>, usize), SourceBundleError> {
    let mut label_to_subgraph = HashMap::<String, usize>::new();
    let mut label_key_bytes = 0_usize;
    for (subgraph_index, subgraph) in wire_subgraphs.iter().enumerate() {
        for label in &subgraph.label_drivers {
            if !matches!(
                label.kind,
                SchematicWireDriverKind::LocalLabel | SchematicWireDriverKind::HierarchicalLabel
            ) || label.text.is_empty()
            {
                continue;
            }
            let canonical = canonical_bus_member_name(&label.text);
            if label_to_subgraph.contains_key(canonical.as_str()) {
                continue;
            }
            label_key_bytes =
                checked_work_bytes(label_key_bytes, canonical.len(), limits, source_path)?;
            label_to_subgraph.insert(canonical, subgraph_index);
        }
    }
    Ok((label_to_subgraph, label_key_bytes))
}

struct BusWireIndexContext<'a> {
    wire_subgraphs: &'a [SchematicWireSubgraph],
    coord_to_subgraph: &'a HashMap<SchematicPoint, usize>,
    label_to_subgraph: &'a HashMap<String, usize>,
    label_key_bytes: usize,
    limits: SchematicDesignNetLimits,
    source_path: &'a str,
}

impl BusWireIndexContext<'_> {
    fn index_bus(
        &self,
        bus: &SchematicBusSubgraph,
    ) -> Result<Vec<Option<usize>>, SourceBundleError> {
        let mut member_by_canonical = HashMap::<String, usize>::new();
        let mut work_bytes = self.label_key_bytes;
        for (position, member) in bus.members.iter().enumerate() {
            let canonical = canonical_bus_member_name(member);
            if member_by_canonical.contains_key(canonical.as_str()) {
                continue;
            }
            work_bytes = self.checked_work(work_bytes, canonical.len())?;
            member_by_canonical.insert(canonical, position);
        }
        let mut per_bus = vec![None; bus.members.len()];
        self.map_taps(bus, &member_by_canonical, &mut per_bus, work_bytes)?;
        self.map_name_only(bus, &mut per_bus, work_bytes)?;
        Ok(per_bus)
    }

    fn map_taps(
        &self,
        bus: &SchematicBusSubgraph,
        member_by_canonical: &HashMap<String, usize>,
        per_bus: &mut [Option<usize>],
        work_bytes: usize,
    ) -> Result<(), SourceBundleError> {
        for tap in &bus.tap_wire_coords {
            let Some(wire_subgraph_index) = self.coord_to_subgraph.get(tap).copied() else {
                continue;
            };
            if let Some(position) =
                self.tap_member_position(wire_subgraph_index, member_by_canonical, work_bytes)?
            {
                per_bus[position].get_or_insert(wire_subgraph_index);
            }
        }
        Ok(())
    }

    fn tap_member_position(
        &self,
        wire_subgraph_index: usize,
        member_by_canonical: &HashMap<String, usize>,
        work_bytes: usize,
    ) -> Result<Option<usize>, SourceBundleError> {
        for label in &self.wire_subgraphs[wire_subgraph_index].label_drivers {
            if label.kind != SchematicWireDriverKind::LocalLabel {
                continue;
            }
            let canonical = canonical_bus_member_name(&label.text);
            self.checked_work(work_bytes, canonical.len())?;
            if let Some(position) = member_by_canonical.get(canonical.as_str()) {
                return Ok(Some(*position));
            }
        }
        Ok(None)
    }

    fn map_name_only(
        &self,
        bus: &SchematicBusSubgraph,
        per_bus: &mut [Option<usize>],
        work_bytes: usize,
    ) -> Result<(), SourceBundleError> {
        for (position, member) in bus.members.iter().enumerate() {
            if per_bus[position].is_some() {
                continue;
            }
            let canonical = canonical_bus_member_name(member);
            self.checked_work(work_bytes, canonical.len())?;
            per_bus[position] = self.label_to_subgraph.get(canonical.as_str()).copied();
        }
        Ok(())
    }

    fn checked_work(&self, current: usize, added: usize) -> Result<usize, SourceBundleError> {
        checked_work_bytes(current, added, self.limits, self.source_path)
    }
}

pub(super) fn index_bus_coords(
    bus_subgraphs: &[SchematicBusSubgraph],
) -> HashMap<SchematicPoint, usize> {
    let capacity = bus_subgraphs.iter().map(|bus| bus.coords.len()).sum();
    let mut result = HashMap::with_capacity(capacity);
    for (bus_index, bus) in bus_subgraphs.iter().enumerate() {
        for point in &bus.coords {
            result.entry(*point).or_insert(bus_index);
        }
    }
    result
}

pub(super) struct BusMemberOverride {
    pub(super) text: String,
    pub(super) priority: SchematicDriverPriority,
    pub(super) kind: SchematicWireDriverKind,
    pub(super) depth: usize,
    pub(super) sheet_path: String,
}

#[derive(Default)]
pub(super) struct BusPromotion {
    pub(super) wire_unions: Vec<(usize, usize)>,
    overrides: Vec<BusMemberOverride>,
    override_indices_by_flat: HashMap<usize, Vec<usize>>,
}

impl BusPromotion {
    pub(super) fn overrides_for(
        &self,
        flat_index: usize,
    ) -> impl Iterator<Item = &BusMemberOverride> {
        self.override_indices_by_flat
            .get(&flat_index)
            .into_iter()
            .flatten()
            .map(|index| &self.overrides[*index])
    }
}

#[derive(Clone, Copy)]
struct BusMemberKey {
    occurrence: usize,
    bus: usize,
    position: usize,
}

struct BusCandidate<'a> {
    priority: SchematicDriverPriority,
    depth: usize,
    sheet_path: &'a str,
    driver_text: &'a str,
    driver_index: usize,
}

#[derive(Clone, Copy)]
struct BusWinner {
    member: BusMemberKey,
    driver_index: usize,
}

struct PromotionBuilder<'a> {
    flat: &'a [BusMemberKey],
    compiled: &'a [CompiledOccurrence],
    offsets: &'a [usize],
    selected_drivers: &'a [Vec<Option<usize>>],
    limits: SchematicDesignNetLimits,
    source_path: Option<&'a str>,
    output: BusPromotion,
    union_work: usize,
    override_string_bytes: usize,
    override_refs: usize,
}

struct MemberUnionContext<'a> {
    union: &'a mut UnionFind,
    work: &'a mut usize,
    limits: SchematicDesignNetLimits,
    source_path: Option<&'a str>,
}

impl MemberUnionContext<'_> {
    fn union(&mut self, left: usize, right: usize) -> Result<(), SourceBundleError> {
        increment_union_work(self.work, self.limits, self.source_path)?;
        self.union.union(left, right);
        Ok(())
    }
}

impl BusCandidate<'_> {
    fn precedes(&self, other: &Self) -> bool {
        (
            std::cmp::Reverse(self.priority),
            self.depth,
            self.sheet_path,
            self.driver_text,
            self.driver_index,
        ) < (
            std::cmp::Reverse(other.priority),
            other.depth,
            other.sheet_path,
            other.driver_text,
            other.driver_index,
        )
    }
}

pub(super) fn promote_bus_members(
    compiled: &[CompiledOccurrence],
    index: &SchematicBundleIndex,
    offsets: &[usize],
    aliases: &HashMap<&str, &[String]>,
    limits: SchematicDesignNetLimits,
) -> Result<BusPromotion, SourceBundleError> {
    let source_path = compiled.first().map(|value| value.source_path.as_str());
    let mut flat = Vec::new();
    let mut member_offsets = Vec::with_capacity(compiled.len());
    for (occurrence, compiled_occurrence) in compiled.iter().enumerate() {
        let mut bus_offsets = Vec::with_capacity(compiled_occurrence.bus_subgraphs.len());
        for (bus, subgraph) in compiled_occurrence.bus_subgraphs.iter().enumerate() {
            bus_offsets.push(flat.len());
            for position in 0..subgraph.members.len() {
                if flat.len() >= limits.max_bus_members {
                    return Err(limit_error(
                        source_path,
                        "design bus member count exceeds its limit",
                    ));
                }
                flat.push(BusMemberKey {
                    occurrence,
                    bus,
                    position,
                });
            }
        }
        member_offsets.push(bus_offsets);
    }
    if flat.is_empty() {
        return Ok(BusPromotion::default());
    }

    let mut union = UnionFind::new(flat.len());
    let mut union_work = 0_usize;
    {
        let mut context = MemberUnionContext {
            union: &mut union,
            work: &mut union_work,
            limits,
            source_path,
        };
        union_within_occurrences(compiled, &member_offsets, &mut context)?;
        union_across_hierarchy(compiled, index, aliases, &member_offsets, &mut context)?;
    }

    let mut group_index_by_root = HashMap::new();
    let mut groups = Vec::<Vec<usize>>::new();
    for member_index in 0..flat.len() {
        let root = union.find(member_index);
        let group_index = *group_index_by_root.entry(root).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group_index].push(member_index);
    }
    let selected_drivers = selected_bus_drivers(compiled, aliases, limits)?;
    let mut builder = PromotionBuilder {
        flat: &flat,
        compiled,
        offsets,
        selected_drivers: &selected_drivers,
        limits,
        source_path,
        output: BusPromotion::default(),
        union_work,
        override_string_bytes: 0,
        override_refs: 0,
    };
    for group in &groups {
        builder.promote_group(group)?;
    }
    Ok(builder.output)
}

fn union_within_occurrences(
    compiled: &[CompiledOccurrence],
    offsets: &[Vec<usize>],
    context: &mut MemberUnionContext<'_>,
) -> Result<(), SourceBundleError> {
    for (occurrence, compiled_occurrence) in compiled.iter().enumerate() {
        let mut first_by_name = HashMap::<String, usize>::new();
        let mut work_bytes = 0_usize;
        for (bus, subgraph) in compiled_occurrence.bus_subgraphs.iter().enumerate() {
            for (position, member) in subgraph.members.iter().enumerate() {
                let canonical = canonical_bus_member_name(member);
                if let Some(first) = first_by_name.get(canonical.as_str()).copied() {
                    checked_optional_work_bytes(
                        work_bytes,
                        canonical.len(),
                        context.limits,
                        context.source_path,
                    )?;
                    context.union(first, offsets[occurrence][bus] + position)?;
                } else {
                    work_bytes = checked_optional_work_bytes(
                        work_bytes,
                        canonical.len(),
                        context.limits,
                        context.source_path,
                    )?;
                    first_by_name.insert(canonical, offsets[occurrence][bus] + position);
                }
            }
        }
    }
    Ok(())
}

fn union_across_hierarchy(
    compiled: &[CompiledOccurrence],
    index: &SchematicBundleIndex,
    aliases: &HashMap<&str, &[String]>,
    offsets: &[Vec<usize>],
    context: &mut MemberUnionContext<'_>,
) -> Result<(), SourceBundleError> {
    let source_path = context.source_path;
    for (child_array_index, child) in compiled.iter().enumerate() {
        let occurrence = index.occurrence(child.occurrence_index).ok_or_else(|| {
            schematic_error(source_path, "design bus child occurrence is missing")
        })?;
        let (Some(parent_index), Some(parent_sheet_index)) =
            (occurrence.parent_index, occurrence.parent_sheet_index)
        else {
            continue;
        };
        let parent_array_index = parent_index.checked_sub(1).ok_or_else(|| {
            schematic_error(source_path, "design bus parent occurrence index is invalid")
        })?;
        let parent = compiled.get(parent_array_index).ok_or_else(|| {
            schematic_error(source_path, "design bus parent occurrence is missing")
        })?;
        let definition = index.definition(&parent.source_path).ok_or_else(|| {
            schematic_error(source_path, "design bus parent definition is missing")
        })?;
        let sheet = definition.sheets.get(parent_sheet_index).ok_or_else(|| {
            schematic_error(source_path, "design bus parent sheet index is out of range")
        })?;
        let mut child_bus_by_name = HashMap::<&str, usize>::new();
        for (bus_index, bus) in child.bus_subgraphs.iter().enumerate() {
            for driver in &bus.drivers {
                if driver.kind == SchematicBusDriverKind::HierarchicalLabel
                    && is_bus_form(
                        driver.text.as_str(),
                        aliases,
                        context.limits.connectivity.bus.expansion,
                    )?
                {
                    child_bus_by_name
                        .entry(driver.text.as_str())
                        .or_insert(bus_index);
                }
            }
        }
        for pin in &sheet.pins {
            if !is_bus_form(
                pin.name.as_str(),
                aliases,
                context.limits.connectivity.bus.expansion,
            )? {
                continue;
            }
            let Some(parent_bus) = parent.bus_coord_to_subgraph.get(&pin.at).copied() else {
                continue;
            };
            let Some(child_bus) = child_bus_by_name.get(pin.name.as_str()).copied() else {
                continue;
            };
            union_bus_pair(
                &parent.bus_subgraphs[parent_bus],
                &child.bus_subgraphs[child_bus],
                offsets[parent_array_index][parent_bus],
                offsets[child_array_index][child_bus],
                context,
            )?;
        }
    }
    Ok(())
}

fn union_bus_pair(
    parent: &SchematicBusSubgraph,
    child: &SchematicBusSubgraph,
    parent_offset: usize,
    child_offset: usize,
    context: &mut MemberUnionContext<'_>,
) -> Result<(), SourceBundleError> {
    let mut child_by_name = HashMap::<String, usize>::new();
    let mut work_bytes = 0_usize;
    for (position, member) in child.members.iter().enumerate() {
        let canonical = canonical_bus_member_name(member);
        if child_by_name.contains_key(canonical.as_str()) {
            continue;
        }
        work_bytes = checked_optional_work_bytes(
            work_bytes,
            canonical.len(),
            context.limits,
            context.source_path,
        )?;
        child_by_name.insert(canonical, position);
    }
    let mut matched_parent = vec![false; parent.members.len()];
    let mut matched_child = vec![false; child.members.len()];
    for (parent_position, member) in parent.members.iter().enumerate() {
        let canonical = canonical_bus_member_name(member);
        checked_optional_work_bytes(
            work_bytes,
            canonical.len(),
            context.limits,
            context.source_path,
        )?;
        let Some(child_position) = child_by_name.get(canonical.as_str()).copied() else {
            continue;
        };
        if matched_child[child_position] {
            continue;
        }
        context.union(
            parent_offset + parent_position,
            child_offset + child_position,
        )?;
        matched_parent[parent_position] = true;
        matched_child[child_position] = true;
    }
    let unmatched_parent = matched_parent
        .iter()
        .enumerate()
        .filter_map(|(position, matched)| (!matched).then_some(position));
    let unmatched_child = matched_child
        .iter()
        .enumerate()
        .filter_map(|(position, matched)| (!matched).then_some(position));
    for (parent_position, child_position) in unmatched_parent.zip(unmatched_child) {
        context.union(
            parent_offset + parent_position,
            child_offset + child_position,
        )?;
    }
    Ok(())
}

impl PromotionBuilder<'_> {
    fn promote_group(&mut self, group: &[usize]) -> Result<(), SourceBundleError> {
        let Some(winner) = self.best_candidate(group) else {
            return Ok(());
        };
        let wire_count = self.wire_key_count(group);
        if wire_count == 0 {
            return Ok(());
        }
        self.preflight_group(winner, wire_count)?;
        let wire_keys = self.wire_keys(group, wire_count);
        let Some(base) = wire_keys.first().copied() else {
            return Ok(());
        };
        for wire in wire_keys.iter().copied().skip(1) {
            self.output.wire_unions.push((base, wire));
        }
        self.push_override(winner, wire_keys);
        Ok(())
    }

    fn best_candidate(&self, group: &[usize]) -> Option<BusWinner> {
        let mut best: Option<(BusCandidate<'_>, BusWinner)> = None;
        for member_index in group {
            let key = self.flat[*member_index];
            let occurrence = &self.compiled[key.occurrence];
            let bus = &occurrence.bus_subgraphs[key.bus];
            let Some(driver_index) = self.selected_drivers[key.occurrence][key.bus] else {
                continue;
            };
            let driver = &bus.drivers[driver_index];
            let candidate = BusCandidate {
                priority: driver.priority,
                depth: sheet_depth(&occurrence.human_address),
                sheet_path: &occurrence.human_address,
                driver_text: &driver.text,
                driver_index,
            };
            if best
                .as_ref()
                .is_none_or(|(current, _)| candidate.precedes(current))
            {
                best = Some((
                    candidate,
                    BusWinner {
                        member: key,
                        driver_index,
                    },
                ));
            }
        }
        best.map(|(_, winner)| winner)
    }

    fn wire_key_count(&self, group: &[usize]) -> usize {
        group
            .iter()
            .filter(|member_index| {
                let key = self.flat[**member_index];
                self.compiled[key.occurrence].bus_member_wire_subgraphs[key.bus][key.position]
                    .is_some()
            })
            .count()
    }

    fn wire_keys(&self, group: &[usize], count: usize) -> Vec<usize> {
        let mut wire_keys = Vec::with_capacity(count);
        for member_index in group {
            let key = self.flat[*member_index];
            let Some(wire) =
                self.compiled[key.occurrence].bus_member_wire_subgraphs[key.bus][key.position]
            else {
                continue;
            };
            wire_keys.push(self.offsets[key.occurrence] + wire);
        }
        wire_keys
    }

    fn preflight_group(
        &mut self,
        winner: BusWinner,
        wire_count: usize,
    ) -> Result<(), SourceBundleError> {
        if self.output.overrides.len() >= self.limits.max_bus_overrides {
            return Err(limit_error(
                self.source_path,
                "design bus override count exceeds its limit",
            ));
        }
        let occurrence = &self.compiled[winner.member.occurrence];
        let bus = &occurrence.bus_subgraphs[winner.member.bus];
        let text = &bus.members[winner.member.position];
        let added_strings = text
            .len()
            .checked_add(occurrence.human_address.len())
            .ok_or_else(|| {
                limit_error(
                    self.source_path,
                    "design bus override string bytes overflow",
                )
            })?;
        self.override_string_bytes = self
            .override_string_bytes
            .checked_add(added_strings)
            .ok_or_else(|| {
                limit_error(
                    self.source_path,
                    "design bus override string bytes overflow",
                )
            })?;
        if self.override_string_bytes > self.limits.max_bus_override_string_bytes {
            return Err(limit_error(
                self.source_path,
                "design bus override string bytes exceed their limit",
            ));
        }
        self.override_refs = self
            .override_refs
            .checked_add(wire_count)
            .ok_or_else(|| limit_error(self.source_path, "design bus override refs overflow"))?;
        if self.override_refs > self.limits.max_bus_override_refs {
            return Err(limit_error(
                self.source_path,
                "design bus override refs exceed their limit",
            ));
        }
        let added_unions = wire_count - 1;
        self.union_work = self
            .union_work
            .checked_add(added_unions)
            .ok_or_else(|| limit_error(self.source_path, "design bus union work overflows"))?;
        if self.union_work > self.limits.max_bus_member_union_work {
            return Err(limit_error(
                self.source_path,
                "design bus union work exceeds its limit",
            ));
        }
        Ok(())
    }

    fn push_override(&mut self, winner: BusWinner, wire_keys: Vec<usize>) {
        let occurrence = &self.compiled[winner.member.occurrence];
        let bus = &occurrence.bus_subgraphs[winner.member.bus];
        let driver = &bus.drivers[winner.driver_index];
        let text = &bus.members[winner.member.position];
        let override_index = self.output.overrides.len();
        self.output.overrides.push(BusMemberOverride {
            text: text.clone(),
            priority: driver.priority,
            kind: wire_kind(driver.kind),
            depth: sheet_depth(&occurrence.human_address),
            sheet_path: occurrence.human_address.clone(),
        });
        for wire in wire_keys {
            self.output
                .override_indices_by_flat
                .entry(wire)
                .or_default()
                .push(override_index);
        }
    }
}

fn selected_bus_drivers(
    compiled: &[CompiledOccurrence],
    aliases: &HashMap<&str, &[String]>,
    limits: SchematicDesignNetLimits,
) -> Result<Vec<Vec<Option<usize>>>, SourceBundleError> {
    let mut result = Vec::with_capacity(compiled.len());
    for occurrence in compiled {
        let mut selected = Vec::with_capacity(occurrence.bus_subgraphs.len());
        for bus in &occurrence.bus_subgraphs {
            let mut best: Option<usize> = None;
            for (driver_index, driver) in bus.drivers.iter().enumerate() {
                if !is_bus_form(
                    driver.text.as_str(),
                    aliases,
                    limits.connectivity.bus.expansion,
                )? {
                    continue;
                }
                if best.is_none_or(|current_index| {
                    let current = &bus.drivers[current_index];
                    (
                        std::cmp::Reverse(driver.priority),
                        driver.text.as_str(),
                        driver_index,
                    ) < (
                        std::cmp::Reverse(current.priority),
                        current.text.as_str(),
                        current_index,
                    )
                }) {
                    best = Some(driver_index);
                }
            }
            selected.push(best);
        }
        result.push(selected);
    }
    Ok(result)
}

fn increment_union_work(
    work: &mut usize,
    limits: SchematicDesignNetLimits,
    source_path: Option<&str>,
) -> Result<(), SourceBundleError> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| limit_error(source_path, "design bus union work overflows"))?;
    if *work > limits.max_bus_member_union_work {
        return Err(limit_error(
            source_path,
            "design bus union work exceeds its limit",
        ));
    }
    Ok(())
}

fn is_bus_form(
    text: &str,
    aliases: &HashMap<&str, &[String]>,
    limits: SchematicBusExpansionLimits,
) -> Result<bool, SourceBundleError> {
    if aliases.contains_key(text) {
        return Ok(true);
    }
    is_schematic_bus_label(text, limits).map_err(|error| {
        let kind = match error.kind {
            crate::SchematicBusExpansionErrorKind::ResourceLimit => {
                SourceBundleErrorKind::ResourceLimit
            }
            crate::SchematicBusExpansionErrorKind::AliasCycle => SourceBundleErrorKind::Schematic,
        };
        SourceBundleError::new(kind, None, error.message)
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
        .ok_or_else(|| limit_error(Some(source_path), &format!("{family} overflows")))?;
    if total > maximum {
        return Err(limit_error(
            Some(source_path),
            &format!("{family} exceeds its limit"),
        ));
    }
    Ok(total)
}

fn checked_work_bytes(
    current: usize,
    added: usize,
    limits: SchematicDesignNetLimits,
    source_path: &str,
) -> Result<usize, SourceBundleError> {
    checked_optional_work_bytes(current, added, limits, Some(source_path))
}

fn checked_optional_work_bytes(
    current: usize,
    added: usize,
    limits: SchematicDesignNetLimits,
    source_path: Option<&str>,
) -> Result<usize, SourceBundleError> {
    let total = current
        .checked_add(added)
        .ok_or_else(|| limit_error(source_path, "design bus mapping work bytes overflow"))?;
    if total > limits.max_bus_mapping_work_bytes {
        return Err(limit_error(
            source_path,
            "design bus mapping work bytes exceed their limit",
        ));
    }
    Ok(total)
}

fn wire_kind(kind: SchematicBusDriverKind) -> SchematicWireDriverKind {
    match kind {
        SchematicBusDriverKind::LocalLabel => SchematicWireDriverKind::LocalLabel,
        SchematicBusDriverKind::GlobalLabel => SchematicWireDriverKind::GlobalLabel,
        SchematicBusDriverKind::HierarchicalLabel => SchematicWireDriverKind::HierarchicalLabel,
        SchematicBusDriverKind::SheetPin => SchematicWireDriverKind::SheetPin,
    }
}

fn sheet_depth(path: &str) -> usize {
    path.bytes().filter(|value| *value == b'/').count()
}

fn limit_error(path: Option<&str>, message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, path, message)
}

fn schematic_error(path: Option<&str>, message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Schematic, path, message)
}

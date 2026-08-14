use crate::schematic_bus::expand_schematic_bus_label_from_map;
use crate::schematic_segment_index::{SchematicSegment, SchematicSegmentIndex};
use crate::{
    SchematicBusExpansionErrorKind, SchematicBusExpansionLimits, SchematicDefinition,
    SchematicLabelScope, SchematicPoint, SourceBundleError, SourceBundleErrorKind,
    is_schematic_bus_label,
};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(i8)]
pub enum SchematicDriverPriority {
    Invalid = -1,
    None = 0,
    Pin = 1,
    SheetPin = 2,
    HierarchicalLabel = 3,
    LocalLabel = 4,
    LocalPowerPin = 5,
    GlobalPowerPin = 6,
    Global = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicBusDriverKind {
    LocalLabel,
    GlobalLabel,
    HierarchicalLabel,
    SheetPin,
}

impl SchematicBusDriverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalLabel => "local_label",
            Self::GlobalLabel => "global_label",
            Self::HierarchicalLabel => "hier_label",
            Self::SheetPin => "sheet_pin",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicBusDriver {
    pub text: String,
    pub at: SchematicPoint,
    pub priority: SchematicDriverPriority,
    pub kind: SchematicBusDriverKind,
    pub source_uuid: String,
    pub source_order: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicBusSubgraph {
    pub coords: Vec<SchematicPoint>,
    pub drivers: Vec<SchematicBusDriver>,
    pub tap_wire_coords: Vec<SchematicPoint>,
    pub chosen_name: String,
    pub chosen_priority: SchematicDriverPriority,
    pub chosen_kind: Option<SchematicBusDriverKind>,
    pub members: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicBusConnectivityLimits {
    pub max_segments: usize,
    pub max_segment_index_nodes: usize,
    pub max_segment_query_work: usize,
    pub max_subgraphs: usize,
    pub max_drivers: usize,
    pub max_taps: usize,
    pub max_aliases: usize,
    pub max_graph_points: usize,
    pub max_retained_points: usize,
    pub max_retained_string_bytes: usize,
    pub max_expanded_members: usize,
    pub max_expanded_member_bytes: usize,
    pub expansion: SchematicBusExpansionLimits,
}

impl Default for SchematicBusConnectivityLimits {
    fn default() -> Self {
        Self {
            max_segments: 8_000_000,
            max_segment_index_nodes: 16_000_000,
            max_segment_query_work: 128_000_000,
            max_subgraphs: 8_000_000,
            max_drivers: 8_000_000,
            max_taps: 8_000_000,
            max_aliases: 1_000_000,
            max_graph_points: 16_000_000,
            max_retained_points: 16_000_000,
            max_retained_string_bytes: 512 * 1024 * 1024,
            max_expanded_members: 16_000_000,
            max_expanded_member_bytes: 512 * 1024 * 1024,
            expansion: SchematicBusExpansionLimits::default(),
        }
    }
}

pub fn build_schematic_bus_subgraphs(
    definition: &SchematicDefinition,
    limits: SchematicBusConnectivityLimits,
) -> Result<Vec<SchematicBusSubgraph>, SourceBundleError> {
    BusBuilder::new(definition, limits)?.build()
}

struct BusBuilder<'a> {
    definition: &'a SchematicDefinition,
    limits: SchematicBusConnectivityLimits,
    aliases: HashMap<&'a str, &'a [String]>,
    bus_union: PointUnion,
    bus_index: SchematicSegmentIndex,
    wire_index: SchematicSegmentIndex,
    query_work: usize,
    retained_points: usize,
    retained_string_bytes: usize,
    expanded_members: usize,
    expanded_member_bytes: usize,
}

impl<'a> BusBuilder<'a> {
    fn new(
        definition: &'a SchematicDefinition,
        limits: SchematicBusConnectivityLimits,
    ) -> Result<Self, SourceBundleError> {
        let (bus_segments, bus_union) = collect_bus_segments(definition, limits)?;
        let wire_segments = collect_wire_segments(definition, limits, bus_segments.len())?;
        let bus_index = SchematicSegmentIndex::build(
            bus_segments,
            limits.max_segment_index_nodes,
            &definition.source_path,
        )?;
        let remaining_nodes = limits
            .max_segment_index_nodes
            .saturating_sub(bus_index.node_count());
        let wire_index =
            SchematicSegmentIndex::build(wire_segments, remaining_nodes, &definition.source_path)?;
        if definition.bus_aliases.len() > limits.max_aliases {
            return Err(limit_error(
                definition,
                "schematic bus alias index count exceeds its limit",
            ));
        }
        let mut aliases = HashMap::with_capacity(definition.bus_aliases.len());
        for alias in &definition.bus_aliases {
            aliases.insert(alias.name.as_str(), alias.members.as_slice());
        }
        Ok(Self {
            definition,
            limits,
            aliases,
            bus_union,
            bus_index,
            wire_index,
            query_work: 0,
            retained_points: 0,
            retained_string_bytes: 0,
            expanded_members: 0,
            expanded_member_bytes: 0,
        })
    }

    fn build(mut self) -> Result<Vec<SchematicBusSubgraph>, SourceBundleError> {
        let taps = self.collect_taps()?;
        let (mut out, root_to_index) = self.realize_physical_subgraphs(taps)?;
        let mut orphans = Vec::new();
        for driver in self.collect_drivers()? {
            if let Some(index) = self.find_subgraph(driver.at, &root_to_index)? {
                out[index].drivers.push(driver);
            } else if self.is_bus_form(&driver.text)? {
                orphans.push(driver);
            }
        }
        self.add_orphan_subgraphs(&mut out, orphans)?;
        self.resolve_names(&mut out)?;
        Ok(out)
    }

    fn collect_taps(&mut self) -> Result<Vec<(SchematicPoint, SchematicPoint)>, SourceBundleError> {
        let mut taps = Vec::new();
        for entry in &self.definition.bus_entries {
            if taps.len() >= self.limits.max_taps {
                return Err(limit_error(
                    self.definition,
                    "schematic bus tap count exceeds its limit",
                ));
            }
            let a = entry.at;
            let b = entry
                .wire_end()
                .ok_or_else(|| schematic_error(self.definition, "bus-entry endpoint overflows"))?;
            let (bus_side, wire_side) = self.classify_tap(a, b)?;
            let bus_point = self.bus_union.add_bounded(
                bus_side,
                self.limits.max_graph_points,
                self.definition,
            )?;
            if let Some(segment) = self.first_bus_segment(bus_side)? {
                let endpoint = self.bus_union.add_bounded(
                    segment.a,
                    self.limits.max_graph_points,
                    self.definition,
                )?;
                self.bus_union.union(bus_point, endpoint);
            }
            taps.push((bus_side, wire_side));
        }
        Ok(taps)
    }

    fn classify_tap(
        &mut self,
        a: SchematicPoint,
        b: SchematicPoint,
    ) -> Result<(SchematicPoint, SchematicPoint), SourceBundleError> {
        let a_bus = self.on_bus(a)?;
        let b_bus = self.on_bus(b)?;
        if a_bus && !b_bus {
            return Ok((a, b));
        }
        if b_bus && !a_bus {
            return Ok((b, a));
        }
        if a_bus && b_bus {
            let a_wire = self.on_wire(a)?;
            let b_wire = self.on_wire(b)?;
            if a_wire && !b_wire {
                return Ok((b, a));
            }
            return Ok((a, b));
        }
        Ok((b, a))
    }

    fn realize_physical_subgraphs(
        &mut self,
        taps: Vec<(SchematicPoint, SchematicPoint)>,
    ) -> Result<(Vec<SchematicBusSubgraph>, HashMap<usize, usize>), SourceBundleError> {
        self.retain_points(self.bus_union.len())?;
        let groups = self
            .bus_union
            .groups_bounded(self.limits.max_subgraphs, self.definition)?;
        let mut out = Vec::with_capacity(groups.len());
        let mut root_to_index = HashMap::with_capacity(groups.len());
        for (root, mut coords) in groups {
            coords.sort_unstable();
            root_to_index.insert(root, out.len());
            out.push(empty_subgraph(coords));
        }
        for (bus_side, wire_side) in taps {
            let root = self
                .bus_union
                .root_for_point(bus_side)
                .ok_or_else(|| schematic_error(self.definition, "bus tap root is missing"))?;
            let index = *root_to_index
                .get(&root)
                .ok_or_else(|| schematic_error(self.definition, "bus tap subgraph is missing"))?;
            out[index].tap_wire_coords.push(wire_side);
        }
        Ok((out, root_to_index))
    }

    fn collect_drivers(&mut self) -> Result<Vec<SchematicBusDriver>, SourceBundleError> {
        let mut drivers = Vec::new();
        for scope in [
            SchematicLabelScope::Local,
            SchematicLabelScope::Global,
            SchematicLabelScope::Hierarchical,
        ] {
            for label in self
                .definition
                .labels
                .iter()
                .filter(|label| label.scope == scope)
            {
                let (priority, kind) = label_driver_type(scope);
                self.push_driver(
                    &mut drivers,
                    &label.text,
                    label.at,
                    priority,
                    kind,
                    &label.uuid,
                )?;
            }
        }
        for sheet in &self.definition.sheets {
            for pin in &sheet.pins {
                self.push_driver(
                    &mut drivers,
                    &pin.name,
                    pin.at,
                    SchematicDriverPriority::SheetPin,
                    SchematicBusDriverKind::SheetPin,
                    &pin.uuid,
                )?;
            }
        }
        Ok(drivers)
    }

    fn push_driver(
        &mut self,
        drivers: &mut Vec<SchematicBusDriver>,
        text: &str,
        at: SchematicPoint,
        priority: SchematicDriverPriority,
        kind: SchematicBusDriverKind,
        source_uuid: &str,
    ) -> Result<(), SourceBundleError> {
        if drivers.len() >= self.limits.max_drivers {
            return Err(limit_error(
                self.definition,
                "schematic bus driver count exceeds its limit",
            ));
        }
        let bytes = text
            .len()
            .checked_add(source_uuid.len())
            .ok_or_else(|| limit_error(self.definition, "bus driver bytes overflow"))?;
        self.retain_string_bytes(bytes)?;
        drivers.push(SchematicBusDriver {
            text: text.to_owned(),
            at,
            priority,
            kind,
            source_uuid: source_uuid.to_owned(),
            source_order: drivers.len(),
        });
        Ok(())
    }

    fn find_subgraph(
        &mut self,
        point: SchematicPoint,
        root_to_index: &HashMap<usize, usize>,
    ) -> Result<Option<usize>, SourceBundleError> {
        if let Some(root) = self.bus_union.root_for_point(point) {
            return Ok(root_to_index.get(&root).copied());
        }
        let Some(segment) = self.first_bus_segment(point)? else {
            return Ok(None);
        };
        let root = self
            .bus_union
            .root_for_point(segment.a)
            .ok_or_else(|| schematic_error(self.definition, "bus segment root is missing"))?;
        Ok(root_to_index.get(&root).copied())
    }

    fn add_orphan_subgraphs(
        &mut self,
        out: &mut Vec<SchematicBusSubgraph>,
        orphans: Vec<SchematicBusDriver>,
    ) -> Result<(), SourceBundleError> {
        let mut by_text = HashMap::<String, usize>::new();
        for driver in orphans {
            let index = if let Some(index) = by_text.get(&driver.text) {
                *index
            } else {
                if out.len() >= self.limits.max_subgraphs {
                    return Err(limit_error(
                        self.definition,
                        "schematic bus subgraph count exceeds its limit",
                    ));
                }
                self.retain_points(1)?;
                self.retain_string_bytes(driver.text.len())?;
                let index = out.len();
                by_text.insert(driver.text.clone(), index);
                out.push(empty_subgraph(vec![driver.at]));
                index
            };
            if !out[index].coords.contains(&driver.at) {
                self.retain_points(1)?;
                out[index].coords.push(driver.at);
                out[index].coords.sort_unstable();
            }
            out[index].drivers.push(driver);
        }
        Ok(())
    }

    fn resolve_names(&mut self, out: &mut [SchematicBusSubgraph]) -> Result<(), SourceBundleError> {
        for subgraph in out {
            let mut best: Option<&SchematicBusDriver> = None;
            for driver in &subgraph.drivers {
                if !self.is_bus_form(&driver.text)? {
                    continue;
                }
                if best.is_none_or(|current| driver_precedes(driver, current)) {
                    best = Some(driver);
                }
            }
            let Some(best) = best else {
                continue;
            };
            self.retain_string_bytes(best.text.len())?;
            let chosen_name = best.text.clone();
            let chosen_priority = best.priority;
            let chosen_kind = best.kind;
            let member_limit = self
                .limits
                .max_expanded_members
                .saturating_sub(self.expanded_members);
            let byte_limit = self
                .limits
                .max_expanded_member_bytes
                .saturating_sub(self.expanded_member_bytes);
            let members = expand_schematic_bus_label_from_map(
                &chosen_name,
                &self.aliases,
                SchematicBusExpansionLimits {
                    max_expanded_members: self
                        .limits
                        .expansion
                        .max_expanded_members
                        .min(member_limit),
                    max_expanded_output_bytes: self
                        .limits
                        .expansion
                        .max_expanded_output_bytes
                        .min(byte_limit),
                    ..self.limits.expansion
                },
            )
            .map_err(|error| bus_expansion_error(self.definition, error))?;
            let member_bytes = members.iter().try_fold(0_usize, |total, member| {
                total
                    .checked_add(member.len())
                    .ok_or_else(|| limit_error(self.definition, "bus member bytes overflow"))
            })?;
            self.expanded_members = self
                .expanded_members
                .checked_add(members.len())
                .ok_or_else(|| limit_error(self.definition, "bus member count overflows"))?;
            self.expanded_member_bytes = self
                .expanded_member_bytes
                .checked_add(member_bytes)
                .ok_or_else(|| limit_error(self.definition, "bus member bytes overflow"))?;
            subgraph.chosen_name = chosen_name;
            subgraph.chosen_priority = chosen_priority;
            subgraph.chosen_kind = Some(chosen_kind);
            subgraph.members = members;
        }
        Ok(())
    }

    fn is_bus_form(&self, text: &str) -> Result<bool, SourceBundleError> {
        if self.aliases.contains_key(text) {
            return Ok(true);
        }
        is_schematic_bus_label(text, self.limits.expansion)
            .map_err(|error| bus_expansion_error(self.definition, error))
    }

    fn on_bus(&mut self, point: SchematicPoint) -> Result<bool, SourceBundleError> {
        self.bus_index.any_containing(
            point,
            &mut self.query_work,
            self.limits.max_segment_query_work,
        )
    }

    fn on_wire(&mut self, point: SchematicPoint) -> Result<bool, SourceBundleError> {
        self.wire_index.any_containing(
            point,
            &mut self.query_work,
            self.limits.max_segment_query_work,
        )
    }

    fn first_bus_segment(
        &mut self,
        point: SchematicPoint,
    ) -> Result<Option<SchematicSegment>, SourceBundleError> {
        self.bus_index.first_containing(
            point,
            &mut self.query_work,
            self.limits.max_segment_query_work,
        )
    }

    fn retain_points(&mut self, count: usize) -> Result<(), SourceBundleError> {
        self.retained_points = self
            .retained_points
            .checked_add(count)
            .ok_or_else(|| limit_error(self.definition, "bus retained point count overflows"))?;
        if self.retained_points > self.limits.max_retained_points {
            return Err(limit_error(
                self.definition,
                "schematic bus retained points exceed their limit",
            ));
        }
        Ok(())
    }

    fn retain_string_bytes(&mut self, bytes: usize) -> Result<(), SourceBundleError> {
        self.retained_string_bytes = self
            .retained_string_bytes
            .checked_add(bytes)
            .ok_or_else(|| limit_error(self.definition, "bus retained string bytes overflow"))?;
        if self.retained_string_bytes > self.limits.max_retained_string_bytes {
            return Err(limit_error(
                self.definition,
                "schematic bus retained string bytes exceed their limit",
            ));
        }
        Ok(())
    }
}

fn collect_bus_segments(
    definition: &SchematicDefinition,
    limits: SchematicBusConnectivityLimits,
) -> Result<(Vec<SchematicSegment>, PointUnion), SourceBundleError> {
    let mut segments = Vec::new();
    let mut union = PointUnion::default();
    for bus in &definition.buses {
        let mut previous = None;
        for point in &bus.points {
            let current = union.add_bounded(*point, limits.max_graph_points, definition)?;
            if let Some((previous_index, previous_point)) = previous
                && let Some(segment) = SchematicSegment::new(previous_point, *point, segments.len())
            {
                if segments.len() >= limits.max_segments {
                    return Err(limit_error(
                        definition,
                        "schematic segment count exceeds its limit",
                    ));
                }
                union.union(previous_index, current);
                segments.push(segment);
            }
            previous = Some((current, *point));
        }
    }
    Ok((segments, union))
}

fn collect_wire_segments(
    definition: &SchematicDefinition,
    limits: SchematicBusConnectivityLimits,
    prior_segments: usize,
) -> Result<Vec<SchematicSegment>, SourceBundleError> {
    let mut segments = Vec::new();
    for wire in &definition.wires {
        for points in wire.points.windows(2) {
            if let Some(segment) = SchematicSegment::new(points[0], points[1], segments.len()) {
                let total = prior_segments
                    .checked_add(segments.len())
                    .ok_or_else(|| limit_error(definition, "schematic segment count overflows"))?;
                if total >= limits.max_segments {
                    return Err(limit_error(
                        definition,
                        "schematic segment count exceeds its limit",
                    ));
                }
                segments.push(segment);
            }
        }
    }
    Ok(segments)
}

fn label_driver_type(
    scope: SchematicLabelScope,
) -> (SchematicDriverPriority, SchematicBusDriverKind) {
    match scope {
        SchematicLabelScope::Local => (
            SchematicDriverPriority::LocalLabel,
            SchematicBusDriverKind::LocalLabel,
        ),
        SchematicLabelScope::Global => (
            SchematicDriverPriority::Global,
            SchematicBusDriverKind::GlobalLabel,
        ),
        SchematicLabelScope::Hierarchical => (
            SchematicDriverPriority::HierarchicalLabel,
            SchematicBusDriverKind::HierarchicalLabel,
        ),
    }
}

fn driver_precedes(candidate: &SchematicBusDriver, current: &SchematicBusDriver) -> bool {
    candidate.priority > current.priority
        || (candidate.priority == current.priority
            && (candidate.text < current.text
                || (candidate.text == current.text
                    && candidate.source_order < current.source_order)))
}

fn empty_subgraph(coords: Vec<SchematicPoint>) -> SchematicBusSubgraph {
    SchematicBusSubgraph {
        coords,
        drivers: Vec::new(),
        tap_wire_coords: Vec::new(),
        chosen_name: String::new(),
        chosen_priority: SchematicDriverPriority::None,
        chosen_kind: None,
        members: Vec::new(),
    }
}

#[derive(Default)]
struct PointUnion {
    index_by_point: HashMap<SchematicPoint, usize>,
    points: Vec<SchematicPoint>,
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl PointUnion {
    fn len(&self) -> usize {
        self.points.len()
    }

    fn add_bounded(
        &mut self,
        point: SchematicPoint,
        max_points: usize,
        definition: &SchematicDefinition,
    ) -> Result<usize, SourceBundleError> {
        if let Some(index) = self.index_by_point.get(&point) {
            return Ok(*index);
        }
        if self.points.len() >= max_points {
            return Err(limit_error(
                definition,
                "schematic bus graph point count exceeds its limit",
            ));
        }
        let index = self.points.len();
        self.index_by_point.insert(point, index);
        self.points.push(point);
        self.parent.push(index);
        self.rank.push(0);
        Ok(index)
    }

    fn root(&mut self, index: usize) -> usize {
        let mut root = index;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = index;
        while self.parent[current] != root {
            let next = self.parent[current];
            self.parent[current] = root;
            current = next;
        }
        root
    }

    fn root_for_point(&mut self, point: SchematicPoint) -> Option<usize> {
        let index = self.index_by_point.get(&point).copied()?;
        Some(self.root(index))
    }

    fn union(&mut self, left: usize, right: usize) {
        let mut left = self.root(left);
        let mut right = self.root(right);
        if left == right {
            return;
        }
        if self.rank[left] < self.rank[right] {
            std::mem::swap(&mut left, &mut right);
        }
        self.parent[right] = left;
        if self.rank[left] == self.rank[right] {
            self.rank[left] = self.rank[left].saturating_add(1);
        }
    }

    fn groups_bounded(
        &mut self,
        max_groups: usize,
        definition: &SchematicDefinition,
    ) -> Result<Vec<(usize, Vec<SchematicPoint>)>, SourceBundleError> {
        let mut groups = BTreeMap::<usize, Vec<SchematicPoint>>::new();
        for index in 0..self.points.len() {
            let root = self.root(index);
            if !groups.contains_key(&root) && groups.len() >= max_groups {
                return Err(limit_error(
                    definition,
                    "schematic bus subgraph count exceeds its limit",
                ));
            }
            groups.entry(root).or_default().push(self.points[index]);
        }
        let mut groups = groups.into_iter().collect::<Vec<_>>();
        groups.sort_by_key(|(_, points)| points.iter().min().copied());
        Ok(groups)
    }
}

fn limit_error(definition: &SchematicDefinition, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(&definition.source_path),
        message,
    )
}

fn schematic_error(
    definition: &SchematicDefinition,
    message: impl Into<String>,
) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Schematic,
        Some(&definition.source_path),
        message,
    )
}

fn bus_expansion_error(
    definition: &SchematicDefinition,
    error: crate::SchematicBusExpansionError,
) -> SourceBundleError {
    let kind = if error.kind == SchematicBusExpansionErrorKind::ResourceLimit {
        SourceBundleErrorKind::ResourceLimit
    } else {
        SourceBundleErrorKind::Schematic
    };
    SourceBundleError::new(kind, Some(&definition.source_path), error.to_string())
}

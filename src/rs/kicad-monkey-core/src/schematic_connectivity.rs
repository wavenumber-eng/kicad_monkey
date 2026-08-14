use crate::schematic_bus_connectivity::{
    SchematicConnectivityGeometry, build_schematic_bus_subgraphs_with_geometry,
    collect_schematic_bus_aliases,
};
use crate::schematic_segment_index::{SchematicSegment, SchematicSegmentIndex};
use crate::{
    SchematicBundleIndex, SchematicBusConnectivityLimits, SchematicDriverPriority,
    SchematicLabelScope, SchematicPoint, SourceBundleError, SourceBundleErrorKind,
    canonical_bus_member_name,
};
use std::collections::{HashMap, HashSet};

mod driver_selection;
mod driver_types;
mod graphical_ids;
mod pin_naming;
mod render_ids;
mod stacked_pins;
mod wire_union;
use driver_selection::resolve_driver;
pub use driver_types::{
    SchematicGraphicalIds, SchematicLabelDriver, SchematicPinDriver, SchematicWireDriverKind,
    SchematicWireSubgraph,
};
pub use pin_naming::SchematicSubpartSettings;
use pin_naming::{PinNaming, build_pin_namings};
pub(crate) use render_ids::schematic_sheet_pin_group_id;
use stacked_pins::expand_stacked_pin;
use wire_union::WirePointUnion;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicOccurrenceConnectivityLimits {
    pub bus: SchematicBusConnectivityLimits,
    pub max_entry_segments: usize,
    pub max_entry_index_nodes: usize,
    pub max_attachment_query_work: usize,
    pub max_graph_points: usize,
    pub max_pin_drivers: usize,
    pub max_label_drivers: usize,
    pub max_subgraphs: usize,
    pub max_retained_points: usize,
    pub max_retained_string_bytes: usize,
    pub max_expanded_pins: usize,
    pub max_expanded_pin_bytes: usize,
    pub max_jumper_union_work: usize,
}

impl Default for SchematicOccurrenceConnectivityLimits {
    fn default() -> Self {
        Self {
            bus: SchematicBusConnectivityLimits::default(),
            max_entry_segments: 8_000_000,
            max_entry_index_nodes: 16_000_000,
            max_attachment_query_work: 128_000_000,
            max_graph_points: 16_000_000,
            max_pin_drivers: 8_000_000,
            max_label_drivers: 8_000_000,
            max_subgraphs: 8_000_000,
            max_retained_points: 16_000_000,
            max_retained_string_bytes: 512 * 1024 * 1024,
            max_expanded_pins: 8_000_000,
            max_expanded_pin_bytes: 256 * 1024 * 1024,
            max_jumper_union_work: 128_000_000,
        }
    }
}

pub fn build_schematic_occurrence_subgraphs(
    index: &SchematicBundleIndex,
    occurrence_index: usize,
    limits: SchematicOccurrenceConnectivityLimits,
) -> Result<Vec<SchematicWireSubgraph>, SourceBundleError> {
    build_schematic_occurrence_subgraphs_with_settings(
        index,
        occurrence_index,
        SchematicSubpartSettings::default(),
        limits,
    )
}

pub fn build_schematic_occurrence_subgraphs_with_settings(
    index: &SchematicBundleIndex,
    occurrence_index: usize,
    subparts: SchematicSubpartSettings,
    limits: SchematicOccurrenceConnectivityLimits,
) -> Result<Vec<SchematicWireSubgraph>, SourceBundleError> {
    build_schematic_occurrence_subgraphs_with_options(
        index,
        occurrence_index,
        subparts,
        false,
        limits,
    )
}

pub(crate) fn build_schematic_occurrence_subgraphs_with_options(
    index: &SchematicBundleIndex,
    occurrence_index: usize,
    subparts: SchematicSubpartSettings,
    include_off_board_symbols: bool,
    limits: SchematicOccurrenceConnectivityLimits,
) -> Result<Vec<SchematicWireSubgraph>, SourceBundleError> {
    let builder = OccurrenceBuilder::new(
        index,
        occurrence_index,
        subparts,
        include_off_board_symbols,
        limits,
    )?;
    let aliases = collect_schematic_bus_aliases(builder.definition, limits.bus)?;
    Ok(builder.build_with_aliases(&aliases)?.wire_subgraphs)
}

pub(crate) struct SchematicOccurrenceDesignSubgraphs {
    pub(crate) wire_subgraphs: Vec<SchematicWireSubgraph>,
    pub(crate) bus_subgraphs: Vec<crate::SchematicBusSubgraph>,
}

pub(crate) fn build_schematic_occurrence_design_subgraphs<'a>(
    index: &'a SchematicBundleIndex,
    occurrence_index: usize,
    subparts: SchematicSubpartSettings,
    include_off_board_symbols: bool,
    limits: SchematicOccurrenceConnectivityLimits,
    aliases: &HashMap<&'a str, &'a [String]>,
) -> Result<SchematicOccurrenceDesignSubgraphs, SourceBundleError> {
    OccurrenceBuilder::new(
        index,
        occurrence_index,
        subparts,
        include_off_board_symbols,
        limits,
    )?
    .build_with_aliases(aliases)
}

struct OccurrenceBuilder<'a> {
    index: &'a SchematicBundleIndex,
    definition: &'a crate::SchematicDefinition,
    occurrence_index: usize,
    subparts: SchematicSubpartSettings,
    include_off_board_symbols: bool,
    limits: SchematicOccurrenceConnectivityLimits,
    retained_string_bytes: usize,
    expanded_pins: usize,
    expanded_pin_bytes: usize,
    attachment_query_work: usize,
}

struct PinSemantics {
    at: SchematicPoint,
    priority: SchematicDriverPriority,
    power_value: String,
    is_power: bool,
    implicit: bool,
}

struct PinPowerSemantics {
    priority: SchematicDriverPriority,
    power_value: String,
    is_power: bool,
    implicit: bool,
}

impl<'a> OccurrenceBuilder<'a> {
    fn new(
        index: &'a SchematicBundleIndex,
        occurrence_index: usize,
        subparts: SchematicSubpartSettings,
        include_off_board_symbols: bool,
        limits: SchematicOccurrenceConnectivityLimits,
    ) -> Result<Self, SourceBundleError> {
        let occurrence = index.occurrence(occurrence_index).ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::Schematic,
                None,
                "schematic occurrence index is out of range",
            )
        })?;
        let definition = index.definition(&occurrence.source_path).ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::MissingSource,
                Some(&occurrence.source_path),
                "schematic occurrence definition is missing",
            )
        })?;
        Ok(Self {
            index,
            definition,
            occurrence_index,
            subparts,
            include_off_board_symbols,
            limits,
            retained_string_bytes: 0,
            expanded_pins: 0,
            expanded_pin_bytes: 0,
            attachment_query_work: 0,
        })
    }

    fn build_with_aliases(
        mut self,
        aliases: &HashMap<&'a str, &'a [String]>,
    ) -> Result<SchematicOccurrenceDesignSubgraphs, SourceBundleError> {
        let mut geometry = SchematicConnectivityGeometry::build(self.definition, self.limits.bus)?;
        let bus_subgraphs = build_schematic_bus_subgraphs_with_geometry(
            self.definition,
            self.limits.bus,
            aliases,
            &mut geometry,
        )?;
        let entry_index = self.build_entry_index()?;
        let mut union = WirePointUnion::default();
        self.seed_carrier_points(&mut union)?;
        self.attach_junctions(&mut union, geometry.wire_index())?;
        let mut labels = self.collect_labels(&mut union)?;
        self.attach_labels(&mut union, &labels, &geometry, &entry_index)?;
        let mut pins = self.collect_pins(&mut union)?;
        self.merge_jumper_pins(&mut union, &pins)?;
        self.merge_same_sheet_names(&mut union, &labels, &pins);
        self.merge_bus_member_taps(&mut union, &labels, &bus_subgraphs)?;
        let wire_subgraphs = self.realize_subgraphs(&mut union, &mut pins, &mut labels)?;
        Ok(SchematicOccurrenceDesignSubgraphs {
            wire_subgraphs,
            bus_subgraphs,
        })
    }

    fn build_entry_index(&self) -> Result<SchematicSegmentIndex, SourceBundleError> {
        if self.definition.bus_entries.len() > self.limits.max_entry_segments {
            return Err(self.limit_error("schematic bus-entry segment count exceeds its limit"));
        }
        let mut segments = Vec::with_capacity(self.definition.bus_entries.len());
        for entry in &self.definition.bus_entries {
            let end = entry
                .wire_end()
                .ok_or_else(|| self.error("bus-entry endpoint overflows"))?;
            if let Some(segment) = SchematicSegment::new(entry.at, end, segments.len()) {
                segments.push(segment);
            }
        }
        SchematicSegmentIndex::build(
            segments,
            self.limits.max_entry_index_nodes,
            &self.definition.source_path,
        )
    }

    fn seed_carrier_points(&self, union: &mut WirePointUnion) -> Result<(), SourceBundleError> {
        for wire in &self.definition.wires {
            let mut previous = None;
            for point in &wire.points {
                let current = union.add(*point, self.limits.max_graph_points, self.definition)?;
                if let Some(previous) = previous {
                    union.union(previous, current);
                }
                previous = Some(current);
            }
        }
        for bus in &self.definition.buses {
            for point in &bus.points {
                union.add(*point, self.limits.max_graph_points, self.definition)?;
            }
        }
        for entry in &self.definition.bus_entries {
            union.add(entry.at, self.limits.max_graph_points, self.definition)?;
            union.add(
                entry
                    .wire_end()
                    .ok_or_else(|| self.error("bus-entry endpoint overflows"))?,
                self.limits.max_graph_points,
                self.definition,
            )?;
        }
        Ok(())
    }

    fn attach_junctions(
        &mut self,
        union: &mut WirePointUnion,
        wire_index: &SchematicSegmentIndex,
    ) -> Result<(), SourceBundleError> {
        for junction in &self.definition.junctions {
            union.add(junction.at, self.limits.max_graph_points, self.definition)?;
            self.attach_point(union, junction.at, wire_index)?;
        }
        Ok(())
    }

    fn collect_labels(
        &mut self,
        union: &mut WirePointUnion,
    ) -> Result<Vec<SchematicLabelDriver>, SourceBundleError> {
        let mut out = Vec::new();
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
                let (priority, kind) = label_type(scope);
                let shape = if scope == SchematicLabelScope::Hierarchical {
                    label.shape.clone()
                } else {
                    String::new()
                };
                self.push_label(
                    &mut out,
                    SchematicLabelDriver {
                        text: label.text.clone(),
                        at: label.at,
                        priority,
                        kind,
                        shape,
                        source_uuid: label.uuid.clone(),
                        render_id: label.uuid.clone(),
                        source_order: 0,
                    },
                )?;
                union.add(label.at, self.limits.max_graph_points, self.definition)?;
            }
        }
        for sheet in &self.definition.sheets {
            for pin in &sheet.pins {
                self.push_label(
                    &mut out,
                    SchematicLabelDriver {
                        text: pin.name.clone(),
                        at: pin.at,
                        priority: SchematicDriverPriority::SheetPin,
                        kind: SchematicWireDriverKind::SheetPin,
                        shape: pin.shape.as_str().to_owned(),
                        source_uuid: pin.uuid.clone(),
                        render_id: schematic_sheet_pin_group_id(sheet, pin),
                        source_order: 0,
                    },
                )?;
                union.add(pin.at, self.limits.max_graph_points, self.definition)?;
            }
        }
        Ok(out)
    }

    fn push_label(
        &mut self,
        out: &mut Vec<SchematicLabelDriver>,
        mut driver: SchematicLabelDriver,
    ) -> Result<(), SourceBundleError> {
        if out.len() >= self.limits.max_label_drivers {
            return Err(self.limit_error("schematic label driver count exceeds its limit"));
        }
        self.retain_strings([
            &driver.text,
            &driver.shape,
            &driver.source_uuid,
            &driver.render_id,
        ])?;
        driver.source_order = out.len();
        out.push(driver);
        Ok(())
    }

    fn attach_labels(
        &mut self,
        union: &mut WirePointUnion,
        labels: &[SchematicLabelDriver],
        geometry: &SchematicConnectivityGeometry,
        entry_index: &SchematicSegmentIndex,
    ) -> Result<(), SourceBundleError> {
        for label in labels {
            self.attach_point(union, label.at, geometry.wire_index())?;
            self.attach_point(union, label.at, geometry.bus_index())?;
            self.attach_point(union, label.at, entry_index)?;
        }
        Ok(())
    }

    fn attach_point(
        &mut self,
        union: &mut WirePointUnion,
        point: SchematicPoint,
        index: &SchematicSegmentIndex,
    ) -> Result<(), SourceBundleError> {
        index.for_each_containing(
            point,
            &mut self.attachment_query_work,
            self.limits.max_attachment_query_work,
            |segment| {
                if point == segment.a || point == segment.b {
                    return true;
                }
                union.union_points(point, segment.a);
                union.union_points(point, segment.b);
                true
            },
        )
    }

    fn collect_pins(
        &mut self,
        union: &mut WirePointUnion,
    ) -> Result<Vec<SchematicPinDriver>, SourceBundleError> {
        let effective = self.index.effective_symbols(self.occurrence_index, None)?;
        let terminals = self.index.symbol_terminals(self.occurrence_index)?;
        let terminals = terminals
            .iter()
            .filter(|terminal| {
                self.include_off_board_symbols
                    || effective
                        .get(terminal.symbol_index)
                        .is_some_and(|symbol| symbol.on_board)
            })
            .collect::<Vec<_>>();
        let namings = build_pin_namings(
            self.definition,
            &terminals,
            &effective,
            self.subparts,
            self.limits.max_retained_string_bytes,
        )?;
        let mut out = Vec::new();
        let mut hidden_nc_sequence = 0_i64;
        for (terminal, naming) in terminals.iter().copied().zip(&namings) {
            let Some(symbol) = effective.get(terminal.symbol_index) else {
                continue;
            };
            let semantics = self.pin_semantics(terminal, symbol, &mut hidden_nc_sequence)?;
            self.push_terminal_pins(&mut out, union, terminal, semantics, naming)?;
        }
        Ok(out)
    }

    fn pin_semantics(
        &self,
        terminal: &crate::SchematicSymbolTerminal,
        symbol: &crate::SchematicEffectiveSymbol,
        hidden_nc_sequence: &mut i64,
    ) -> Result<PinSemantics, SourceBundleError> {
        let placed = &self.definition.symbols[terminal.symbol_index];
        let library = self.definition.library_symbol_for_placement(placed);
        let power = classify_pin_power(terminal, symbol, library);
        let at = self.pin_connectivity_point(terminal, hidden_nc_sequence)?;
        Ok(PinSemantics {
            at,
            priority: power.priority,
            power_value: power.power_value,
            is_power: power.is_power,
            implicit: power.implicit,
        })
    }

    fn pin_connectivity_point(
        &self,
        terminal: &crate::SchematicSymbolTerminal,
        hidden_nc_sequence: &mut i64,
    ) -> Result<SchematicPoint, SourceBundleError> {
        if terminal.hidden && terminal.electrical_type == "no_connect" {
            let offset = 1_000_000_000_000_i64
                .checked_add(*hidden_nc_sequence)
                .ok_or_else(|| self.error("hidden no-connect sequence overflows"))?;
            *hidden_nc_sequence = hidden_nc_sequence
                .checked_add(1)
                .ok_or_else(|| self.error("hidden no-connect sequence overflows"))?;
            Ok(SchematicPoint {
                x_iu: terminal
                    .at
                    .x_iu
                    .checked_sub(offset)
                    .ok_or_else(|| self.error("hidden no-connect coordinate overflows"))?,
                y_iu: terminal.at.y_iu,
            })
        } else {
            Ok(terminal.at)
        }
    }

    fn push_terminal_pins(
        &mut self,
        out: &mut Vec<SchematicPinDriver>,
        union: &mut WirePointUnion,
        terminal: &crate::SchematicSymbolTerminal,
        semantics: PinSemantics,
        naming: &PinNaming,
    ) -> Result<(), SourceBundleError> {
        let remaining_drivers = self.limits.max_pin_drivers.saturating_sub(out.len());
        let expanded = self.expand_pin_number(&terminal.pin_number, remaining_drivers)?;
        for pin_number in expanded {
            if out.len() >= self.limits.max_pin_drivers {
                return Err(self.limit_error("schematic pin driver count exceeds its limit"));
            }
            let stacked = pin_number != terminal.pin_number;
            let pin_name = if stacked && terminal.pin_name.is_empty() {
                pin_number.clone()
            } else if stacked {
                format!("{}_{}", terminal.pin_name, pin_number)
            } else {
                terminal.pin_name.clone()
            };
            let normalized_number = if pin_number == "~" {
                String::new()
            } else {
                pin_number
            };
            self.retain_strings([
                terminal.symbol_uuid.as_str(),
                terminal.reference.as_str(),
                normalized_number.as_str(),
                pin_name.as_str(),
                terminal.electrical_type.as_str(),
                semantics.power_value.as_str(),
                naming.designator_with_unit.as_str(),
                naming.source_pin_uuid.as_str(),
                naming.pin_svg_id.as_str(),
            ])?;
            out.push(SchematicPinDriver {
                symbol_index: terminal.symbol_index,
                symbol_uuid: terminal.symbol_uuid.clone(),
                reference: terminal.reference.clone(),
                pin_number: normalized_number,
                pin_name,
                electrical_type: terminal.electrical_type.clone(),
                hidden: terminal.hidden,
                at: semantics.at,
                priority: semantics.priority,
                kind: pin_kind(semantics.priority),
                power_value: semantics.power_value.clone(),
                has_multiple: naming.has_multiple,
                designator_with_unit: naming.designator_with_unit.clone(),
                parent_pin_count: naming.parent_pin_count,
                is_power: semantics.is_power,
                is_implicit_hidden_power: semantics.implicit,
                source_pin_uuid: naming.source_pin_uuid.clone(),
                pin_svg_id: naming.pin_svg_id.clone(),
                source_order: out.len(),
            });
        }
        union.add(semantics.at, self.limits.max_graph_points, self.definition)?;
        Ok(())
    }

    fn expand_pin_number(
        &mut self,
        value: &str,
        remaining_drivers: usize,
    ) -> Result<Vec<String>, SourceBundleError> {
        let remaining_pins = self
            .limits
            .max_expanded_pins
            .saturating_sub(self.expanded_pins)
            .min(remaining_drivers);
        let count_message = if remaining_drivers
            < self
                .limits
                .max_expanded_pins
                .saturating_sub(self.expanded_pins)
        {
            "schematic pin driver count exceeds its limit"
        } else {
            "expanded pin count exceeds its limit"
        };
        let remaining_bytes = self
            .limits
            .max_expanded_pin_bytes
            .saturating_sub(self.expanded_pin_bytes);
        let expanded = expand_stacked_pin(
            value,
            remaining_pins,
            remaining_bytes,
            &self.definition.source_path,
            count_message,
        )?;
        let bytes = expanded.iter().try_fold(0_usize, |total, member| {
            total
                .checked_add(member.len())
                .ok_or_else(|| self.limit_error("expanded pin bytes overflow"))
        })?;
        self.expanded_pins = self
            .expanded_pins
            .checked_add(expanded.len())
            .ok_or_else(|| self.limit_error("expanded pin count overflows"))?;
        self.expanded_pin_bytes = self
            .expanded_pin_bytes
            .checked_add(bytes)
            .ok_or_else(|| self.limit_error("expanded pin bytes overflow"))?;
        if self.expanded_pins > self.limits.max_expanded_pins {
            return Err(self.limit_error("expanded pin count exceeds its limit"));
        }
        if self.expanded_pin_bytes > self.limits.max_expanded_pin_bytes {
            return Err(self.limit_error("expanded pin bytes exceed their limit"));
        }
        Ok(expanded)
    }

    fn merge_same_sheet_names(
        &self,
        union: &mut WirePointUnion,
        labels: &[SchematicLabelDriver],
        pins: &[SchematicPinDriver],
    ) {
        let mut by_name = HashMap::<&str, usize>::new();
        for label in labels.iter().filter(|label| {
            matches!(
                label.kind,
                SchematicWireDriverKind::LocalLabel | SchematicWireDriverKind::HierarchicalLabel
            )
        }) {
            merge_named_point(&mut by_name, union, &label.text, label.at);
        }
        for pin in pins
            .iter()
            .filter(|pin| pin.is_power && !pin.power_value.is_empty())
        {
            merge_named_point(&mut by_name, union, &pin.power_value, pin.at);
        }
    }

    fn merge_jumper_pins(
        &self,
        union: &mut WirePointUnion,
        pins: &[SchematicPinDriver],
    ) -> Result<(), SourceBundleError> {
        let active_symbols = self
            .definition
            .symbols
            .iter()
            .enumerate()
            .filter_map(|(index, placed)| {
                self.definition
                    .library_symbol_for_placement(placed)
                    .filter(|library| {
                        library.duplicate_pin_numbers_are_jumpers
                            || !library.jumper_pin_groups.is_empty()
                    })
                    .map(|_| index)
            })
            .collect::<HashSet<_>>();
        let mut by_symbol = HashMap::<usize, HashMap<&str, Vec<usize>>>::new();
        for pin in pins
            .iter()
            .filter(|pin| active_symbols.contains(&pin.symbol_index))
        {
            let Some(root) = union.root_for_point(pin.at) else {
                continue;
            };
            by_symbol
                .entry(pin.symbol_index)
                .or_default()
                .entry(&pin.pin_number)
                .or_default()
                .push(root);
        }
        let mut work = 0_usize;
        for (symbol_index, pins_by_number) in by_symbol {
            let placed = &self.definition.symbols[symbol_index];
            let Some(library) = self.definition.library_symbol_for_placement(placed) else {
                continue;
            };
            if library.duplicate_pin_numbers_are_jumpers {
                for roots in pins_by_number.values() {
                    union_roots_bounded(
                        union,
                        roots.iter().copied(),
                        &mut work,
                        self.limits.max_jumper_union_work,
                        self.definition,
                    )?;
                }
            }
            for group in &library.jumper_pin_groups {
                let roots = group
                    .iter()
                    .filter_map(|number| pins_by_number.get(number.as_str()))
                    .flatten()
                    .copied();
                union_roots_bounded(
                    union,
                    roots,
                    &mut work,
                    self.limits.max_jumper_union_work,
                    self.definition,
                )?;
            }
        }
        Ok(())
    }

    fn merge_bus_member_taps(
        &self,
        union: &mut WirePointUnion,
        labels: &[SchematicLabelDriver],
        bus_subgraphs: &[crate::SchematicBusSubgraph],
    ) -> Result<(), SourceBundleError> {
        let mut label_by_root = HashMap::<usize, String>::new();
        for label in labels
            .iter()
            .filter(|label| label.kind == SchematicWireDriverKind::LocalLabel)
        {
            if let Some(root) = union.root_for_point(label.at) {
                label_by_root
                    .entry(root)
                    .or_insert_with(|| canonical_bus_member_name(&label.text));
            }
        }
        for bus in bus_subgraphs.iter().filter(|bus| !bus.members.is_empty()) {
            let members = bus
                .members
                .iter()
                .map(|member| canonical_bus_member_name(member))
                .collect::<HashSet<_>>();
            let mut first_by_member = HashMap::<&str, usize>::new();
            for tap in &bus.tap_wire_coords {
                let Some(root) = union.root_for_point(*tap) else {
                    continue;
                };
                let Some(member) = label_by_root.get(&root).map(String::as_str) else {
                    continue;
                };
                if !members.contains(member) {
                    continue;
                }
                if let Some(first) = first_by_member.get(member) {
                    union.union(*first, root);
                } else {
                    first_by_member.insert(member, root);
                }
            }
        }
        Ok(())
    }

    fn realize_subgraphs(
        &mut self,
        union: &mut WirePointUnion,
        pins: &mut Vec<SchematicPinDriver>,
        labels: &mut Vec<SchematicLabelDriver>,
    ) -> Result<Vec<SchematicWireSubgraph>, SourceBundleError> {
        if union.point_count() > self.limits.max_retained_points {
            return Err(self.limit_error("retained subgraph points exceed their limit"));
        }
        union.ensure_group_limit(self.limits.max_subgraphs, self.definition)?;
        let mut pins_by_root = HashMap::<usize, Vec<SchematicPinDriver>>::new();
        for pin in pins.drain(..) {
            let root = union
                .root_for_point(pin.at)
                .ok_or_else(|| self.error("pin connectivity root is missing"))?;
            pins_by_root.entry(root).or_default().push(pin);
        }
        let mut labels_by_root = HashMap::<usize, Vec<SchematicLabelDriver>>::new();
        for label in labels.drain(..) {
            let root = union
                .root_for_point(label.at)
                .ok_or_else(|| self.error("label connectivity root is missing"))?;
            labels_by_root.entry(root).or_default().push(label);
        }
        let no_connects = self
            .definition
            .no_connects
            .iter()
            .map(|marker| marker.at)
            .collect::<HashSet<_>>();
        let mut graphical_by_root = self.index_source_graphics(union)?;
        let groups = union.groups(self.limits.max_subgraphs, self.definition)?;
        let mut out = Vec::with_capacity(groups.len());
        for (root, mut coords) in groups {
            coords.sort_unstable();
            let pin_drivers = pins_by_root.remove(&root).unwrap_or_default();
            let label_drivers = labels_by_root.remove(&root).unwrap_or_default();
            let mut graphical = graphical_by_root.remove(&root).unwrap_or_default();
            self.attach_driver_graphics(&mut graphical, &pin_drivers, &label_drivers)?;
            let (chosen_name, chosen_priority, chosen_kind) =
                resolve_driver(&label_drivers, &pin_drivers);
            self.retain_strings([chosen_name.as_str()])?;
            let no_connect = coords.iter().any(|point| no_connects.contains(point));
            out.push(SchematicWireSubgraph {
                coords,
                pin_drivers,
                label_drivers,
                graphical,
                chosen_name,
                chosen_priority,
                chosen_kind,
                no_connect,
            });
        }
        out.sort_by(|left, right| {
            left.chosen_name.cmp(&right.chosen_name).then_with(|| {
                left.coords
                    .first()
                    .copied()
                    .cmp(&right.coords.first().copied())
            })
        });
        Ok(out)
    }

    fn retain_strings<const N: usize>(
        &mut self,
        values: [&str; N],
    ) -> Result<(), SourceBundleError> {
        let bytes = values.into_iter().try_fold(0_usize, |total, value| {
            total
                .checked_add(value.len())
                .ok_or_else(|| self.limit_error("connectivity retained string bytes overflow"))
        })?;
        self.retain_string_bytes(bytes)
    }

    fn retain_string_bytes(&mut self, bytes: usize) -> Result<(), SourceBundleError> {
        self.retained_string_bytes = self
            .retained_string_bytes
            .checked_add(bytes)
            .ok_or_else(|| self.limit_error("connectivity retained string bytes overflow"))?;
        if self.retained_string_bytes > self.limits.max_retained_string_bytes {
            return Err(self.limit_error("connectivity retained string bytes exceed their limit"));
        }
        Ok(())
    }

    fn error(&self, message: &str) -> SourceBundleError {
        SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(&self.definition.source_path),
            message,
        )
    }

    fn limit_error(&self, message: &str) -> SourceBundleError {
        SourceBundleError::new(
            SourceBundleErrorKind::ResourceLimit,
            Some(&self.definition.source_path),
            message,
        )
    }
}

fn classify_pin_power(
    terminal: &crate::SchematicSymbolTerminal,
    symbol: &crate::SchematicEffectiveSymbol,
    library: Option<&crate::SchematicLibrarySymbol>,
) -> PinPowerSemantics {
    let parent_is_power = symbol.lib_id != "power:PWR_FLAG"
        && (symbol.lib_id.starts_with("power:") || library.is_some_and(|value| value.power));
    if implicit_hidden_power(terminal, parent_is_power) {
        return PinPowerSemantics {
            priority: SchematicDriverPriority::GlobalPowerPin,
            power_value: terminal.pin_name.clone(),
            is_power: true,
            implicit: true,
        };
    }
    let is_power = parent_is_power && terminal.electrical_type == "power_in";
    let priority =
        if is_power && library.is_some_and(|value| value.power_kind.as_deref() == Some("local")) {
            SchematicDriverPriority::LocalPowerPin
        } else if is_power {
            SchematicDriverPriority::GlobalPowerPin
        } else {
            SchematicDriverPriority::Pin
        };
    PinPowerSemantics {
        priority,
        power_value: if is_power {
            symbol.value.clone()
        } else {
            String::new()
        },
        is_power,
        implicit: false,
    }
}

fn implicit_hidden_power(terminal: &crate::SchematicSymbolTerminal, parent_is_power: bool) -> bool {
    !parent_is_power
        && terminal.electrical_type == "power_in"
        && terminal.hidden
        && !terminal.pin_name.is_empty()
        && terminal.pin_name != "~"
        && terminal.pin_name != terminal.pin_number
}

fn merge_named_point<'a>(
    by_name: &mut HashMap<&'a str, usize>,
    union: &mut WirePointUnion,
    name: &'a str,
    point: SchematicPoint,
) {
    if name.is_empty() {
        return;
    }
    let Some(root) = union.root_for_point(point) else {
        return;
    };
    if let Some(first) = by_name.get(name) {
        union.union(*first, root);
    } else {
        by_name.insert(name, root);
    }
}

fn union_roots_bounded(
    union: &mut WirePointUnion,
    roots: impl Iterator<Item = usize>,
    work: &mut usize,
    max_work: usize,
    definition: &crate::SchematicDefinition,
) -> Result<(), SourceBundleError> {
    let mut first = None;
    for root in roots {
        *work = work.checked_add(1).ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::ResourceLimit,
                Some(&definition.source_path),
                "schematic jumper union work overflows",
            )
        })?;
        if *work > max_work {
            return Err(SourceBundleError::new(
                SourceBundleErrorKind::ResourceLimit,
                Some(&definition.source_path),
                "schematic jumper union work exceeds its limit",
            ));
        }
        if let Some(first) = first {
            union.union(first, root);
        } else {
            first = Some(root);
        }
    }
    Ok(())
}

use driver_types::{label_type, pin_kind};

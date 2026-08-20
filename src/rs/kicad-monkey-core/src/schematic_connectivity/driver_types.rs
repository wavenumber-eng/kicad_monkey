use crate::{SchematicDriverPriority, SchematicLabelScope, SchematicPoint};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicWireDriverKind {
    Pin,
    LocalPowerPin,
    GlobalPowerPin,
    LocalLabel,
    GlobalLabel,
    HierarchicalLabel,
    SheetPin,
}

impl SchematicWireDriverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pin => "pin",
            Self::LocalPowerPin => "local_power_pin",
            Self::GlobalPowerPin => "global_power_pin",
            Self::LocalLabel => "local_label",
            Self::GlobalLabel => "global_label",
            Self::HierarchicalLabel => "hier_label",
            Self::SheetPin => "sheet_pin",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicPinDriver {
    pub symbol_index: usize,
    pub symbol_uuid: String,
    pub reference: String,
    pub pin_number: String,
    pub pin_name: String,
    pub electrical_type: String,
    pub hidden: bool,
    pub at: SchematicPoint,
    pub priority: SchematicDriverPriority,
    pub kind: SchematicWireDriverKind,
    pub power_value: String,
    pub has_multiple: bool,
    pub designator_with_unit: String,
    pub parent_pin_count: usize,
    pub is_power: bool,
    pub is_implicit_hidden_power: bool,
    pub source_pin_uuid: String,
    pub pin_svg_id: String,
    pub source_order: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLabelDriver {
    pub text: String,
    pub at: SchematicPoint,
    pub priority: SchematicDriverPriority,
    pub kind: SchematicWireDriverKind,
    pub shape: String,
    pub source_uuid: String,
    pub render_id: String,
    pub source_order: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicWireSubgraph {
    pub coords: Vec<SchematicPoint>,
    pub pin_drivers: Vec<SchematicPinDriver>,
    pub label_drivers: Vec<SchematicLabelDriver>,
    pub graphical: SchematicGraphicalIds,
    pub chosen_name: String,
    pub chosen_priority: SchematicDriverPriority,
    pub chosen_kind: Option<SchematicWireDriverKind>,
    pub no_connect: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchematicGraphicalIds {
    pub wires: Vec<String>,
    pub junctions: Vec<String>,
    pub labels: Vec<String>,
    pub power_ports: Vec<String>,
    pub ports: Vec<String>,
    pub sheet_entries: Vec<String>,
}

impl SchematicGraphicalIds {
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.wires
            .iter()
            .chain(&self.junctions)
            .chain(&self.labels)
            .chain(&self.power_ports)
            .chain(&self.ports)
            .chain(&self.sheet_entries)
    }
}

pub(super) fn label_type(
    scope: SchematicLabelScope,
) -> (SchematicDriverPriority, SchematicWireDriverKind) {
    match scope {
        SchematicLabelScope::Local => (
            SchematicDriverPriority::LocalLabel,
            SchematicWireDriverKind::LocalLabel,
        ),
        SchematicLabelScope::Global => (
            SchematicDriverPriority::Global,
            SchematicWireDriverKind::GlobalLabel,
        ),
        SchematicLabelScope::Hierarchical => (
            SchematicDriverPriority::HierarchicalLabel,
            SchematicWireDriverKind::HierarchicalLabel,
        ),
    }
}

pub(super) fn pin_kind(priority: SchematicDriverPriority) -> SchematicWireDriverKind {
    match priority {
        SchematicDriverPriority::GlobalPowerPin => SchematicWireDriverKind::GlobalPowerPin,
        SchematicDriverPriority::LocalPowerPin => SchematicWireDriverKind::LocalPowerPin,
        _ => SchematicWireDriverKind::Pin,
    }
}

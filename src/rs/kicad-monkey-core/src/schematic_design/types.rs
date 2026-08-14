use crate::{
    SchematicDriverPriority, SchematicOccurrenceConnectivityLimits, SchematicWireDriverKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicDesignNetLimits {
    pub connectivity: SchematicOccurrenceConnectivityLimits,
    pub max_subgraphs: usize,
    pub max_indexed_coords: usize,
    pub max_union_work: usize,
    pub max_merge_keys: usize,
    pub max_sheet_pin_targets: usize,
    pub max_target_index_bytes: usize,
    pub max_hierarchy_bindings: usize,
    pub max_drivers_per_net: usize,
    pub max_nets: usize,
    pub max_net_members: usize,
    pub max_terminals: usize,
    pub max_name_bytes: usize,
    pub max_retained_string_bytes: usize,
    pub max_work_string_bytes: usize,
    pub max_merged_driver_bytes: usize,
}

impl Default for SchematicDesignNetLimits {
    fn default() -> Self {
        Self {
            connectivity: SchematicOccurrenceConnectivityLimits::default(),
            max_subgraphs: 16_000_000,
            max_indexed_coords: 32_000_000,
            max_union_work: 128_000_000,
            max_merge_keys: 16_000_000,
            max_sheet_pin_targets: 8_000_000,
            max_target_index_bytes: 1024 * 1024 * 1024,
            max_hierarchy_bindings: 8_000_000,
            max_drivers_per_net: 16_000_000,
            max_nets: 8_000_000,
            max_net_members: 16_000_000,
            max_terminals: 16_000_000,
            max_name_bytes: 512 * 1024 * 1024,
            max_retained_string_bytes: 1024 * 1024 * 1024,
            max_work_string_bytes: 1024 * 1024 * 1024,
            max_merged_driver_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicDesignNetMember {
    pub occurrence_index: usize,
    pub subgraph_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicDesignNetTerminal {
    pub occurrence_index: usize,
    pub symbol_index: usize,
    pub designator: String,
    pub pin: String,
    pub pin_name: String,
    pub pin_type: String,
    pub sheet_path: String,
    pub source_pin_id: String,
    pub svg_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicDesignNet {
    pub name: String,
    pub code: u64,
    pub driver_priority: SchematicDriverPriority,
    pub driver_kind: Option<SchematicWireDriverKind>,
    pub auto_named: bool,
    pub members: Vec<SchematicDesignNetMember>,
    pub terminals: Vec<SchematicDesignNetTerminal>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicHierarchyNetBinding {
    pub parent_occurrence_index: usize,
    pub child_occurrence_index: usize,
    pub sheet_pin_name: String,
    pub sheet_pin_uuid: String,
    pub hierarchical_label_uuid: Option<String>,
    pub parent_subgraph_index: Option<usize>,
    pub child_subgraph_index: Option<usize>,
}

impl SchematicHierarchyNetBinding {
    pub fn is_resolved(&self) -> bool {
        self.parent_subgraph_index.is_some() && self.child_subgraph_index.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicScalarDesignNetlist {
    pub nets: Vec<SchematicDesignNet>,
    pub hierarchy_bindings: Vec<SchematicHierarchyNetBinding>,
}

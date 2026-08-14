use crate::SchematicDesignNetLimits;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KiCadNetlistLimits {
    pub design: SchematicDesignNetLimits,
    pub max_nets: usize,
    pub max_terminals: usize,
    pub max_components: usize,
    pub max_component_candidates: usize,
    pub max_component_fields: usize,
    pub max_component_units: usize,
    pub max_component_unit_pins: usize,
    pub max_libparts: usize,
    pub max_libpart_pins: usize,
    pub max_sheets: usize,
    pub max_retained_string_bytes: usize,
    pub max_output_bytes: usize,
}

impl Default for KiCadNetlistLimits {
    fn default() -> Self {
        Self {
            design: SchematicDesignNetLimits::default(),
            max_nets: 8_000_000,
            max_terminals: 16_000_000,
            max_components: 16_000_000,
            max_component_candidates: 32_000_000,
            max_component_fields: 32_000_000,
            max_component_units: 16_000_000,
            max_component_unit_pins: 32_000_000,
            max_libparts: 1_000_000,
            max_libpart_pins: 16_000_000,
            max_sheets: 8_000_000,
            max_retained_string_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
            max_output_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadNetlistTerminal {
    pub designator: String,
    pub pin: String,
    pub pin_name: String,
    pub pin_type: String,
    pub sheet_path: String,
    pub source_pin_id: String,
    pub svg_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadNet {
    pub name: String,
    pub code: u64,
    pub terminals: Vec<KiCadNetlistTerminal>,
    pub driver_priority: i8,
    pub driver_kind: String,
    pub auto_named: bool,
    pub net_class: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadNetlistComponentUnit {
    pub name: String,
    pub pins: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadNetlistComponent {
    pub reference: String,
    pub value: String,
    pub footprint: String,
    pub datasheet: String,
    pub description: String,
    pub fields: BTreeMap<String, String>,
    pub libsource_lib: String,
    pub libsource_part: String,
    pub libsource_description: String,
    pub sheet_path_names: String,
    pub sheet_path_uuids: String,
    pub instance_uuids: Vec<String>,
    pub properties: BTreeMap<String, String>,
    pub units: Vec<KiCadNetlistComponentUnit>,
    pub in_bom: bool,
    pub on_board: bool,
    pub dnp: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadLibPartPin {
    pub number: String,
    pub name: String,
    pub pin_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadLibPart {
    pub lib: String,
    pub part: String,
    pub description: String,
    pub docs: String,
    pub footprints_filter: Vec<String>,
    pub fields: BTreeMap<String, String>,
    pub pins: Vec<KiCadLibPartPin>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadDesignSheet {
    pub number: usize,
    pub name: String,
    pub tstamps: String,
    pub title: String,
    pub company: String,
    pub revision: String,
    pub date: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KiCadNetlist {
    pub nets: Vec<KiCadNet>,
    pub components: Vec<KiCadNetlistComponent>,
    pub libparts: Vec<KiCadLibPart>,
    pub libraries: Vec<String>,
    pub sheets: Vec<KiCadDesignSheet>,
}

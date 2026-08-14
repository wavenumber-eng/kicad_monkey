use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicBusExpansionLimits {
    pub max_input_bytes: usize,
    pub max_group_members: usize,
    pub max_expanded_members: usize,
    pub max_parsed_member_bytes: usize,
    pub max_expansion_work_items: usize,
    pub max_expansion_work_bytes: usize,
    pub max_expanded_output_bytes: usize,
    pub max_nesting_depth: usize,
}

impl Default for SchematicBusExpansionLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1024 * 1024,
            max_group_members: 1_000_000,
            max_expanded_members: 1_000_000,
            max_parsed_member_bytes: 64 * 1024 * 1024,
            max_expansion_work_items: 1_000_000,
            max_expansion_work_bytes: 64 * 1024 * 1024,
            max_expanded_output_bytes: 64 * 1024 * 1024,
            max_nesting_depth: 512,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicBusExpansionErrorKind {
    ResourceLimit,
    AliasCycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicBusExpansionError {
    pub kind: SchematicBusExpansionErrorKind,
    pub message: String,
}

impl fmt::Display for SchematicBusExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SchematicBusExpansionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicBusPattern {
    pub prefix: String,
    pub members: Vec<String>,
}

/// Normalize the escaped slash form used by KiCad net names for matching.
pub fn canonical_bus_member_name(text: &str) -> String {
    text.replace("{slash}", "/")
}

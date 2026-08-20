//! Resolved native KiCad netlist model and bounded version-E writer.

mod build;
mod emit;
mod glob;
mod json;
mod merge;
mod resource;
mod types;
mod variables;

pub use build::build_kicad_netlist;
pub use emit::emit_kicad_netlist;
pub use json::{KiCadNetlistJsonMetadata, build_kicad_netlist_json};
pub use types::{
    KiCadDesignSheet, KiCadLibPart, KiCadLibPartPin, KiCadNet, KiCadNetClass, KiCadNetlist,
    KiCadNetlistComponent, KiCadNetlistComponentUnit, KiCadNetlistLimits, KiCadNetlistTerminal,
};

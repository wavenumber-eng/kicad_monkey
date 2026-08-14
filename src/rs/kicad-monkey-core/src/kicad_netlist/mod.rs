//! Resolved native KiCad netlist model and bounded version-E writer.

mod build;
mod emit;
mod glob;
mod merge;
mod resource;
mod types;
mod variables;

pub use build::build_kicad_netlist;
pub use emit::emit_kicad_netlist;
pub use types::{
    KiCadDesignSheet, KiCadLibPart, KiCadLibPartPin, KiCadNet, KiCadNetlist, KiCadNetlistComponent,
    KiCadNetlistComponentUnit, KiCadNetlistLimits, KiCadNetlistTerminal,
};

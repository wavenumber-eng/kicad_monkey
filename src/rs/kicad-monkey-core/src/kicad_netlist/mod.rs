//! Resolved native KiCad netlist model and bounded version-E writer.

mod build;
mod emit;
mod glob;
mod resource;
mod types;

pub use build::build_kicad_netlist;
pub use emit::emit_kicad_netlist;
pub use types::{
    KiCadDesignSheet, KiCadLibPart, KiCadLibPartPin, KiCadNet, KiCadNetlist, KiCadNetlistComponent,
    KiCadNetlistComponentUnit, KiCadNetlistLimits, KiCadNetlistTerminal,
};

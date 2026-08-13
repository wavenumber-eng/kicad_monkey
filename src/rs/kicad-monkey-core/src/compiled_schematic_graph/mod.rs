//! Native deterministic identity and semantic validation for compiled graphs.

mod identity;
mod validation;

pub use identity::{
    CompiledGraphIdentityAllocator, CompiledGraphIdentityError, IdentityMapping,
    compiled_schematic_graph_design_scope,
};
pub use validation::validate_compiled_schematic_graph;

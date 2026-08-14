//! Native deterministic identity and semantic validation for compiled graphs.

mod identity;
mod producer;
mod validation;

pub use identity::{
    CompiledGraphIdentityAllocator, CompiledGraphIdentityError, IdentityMapping,
    compiled_schematic_graph_design_scope,
};
pub use producer::{CompiledSchematicGraphLimits, build_compiled_schematic_graph};
pub use validation::validate_compiled_schematic_graph;

//! Shared native PCB SVG infrastructure for configurable views and Toon presets.
//!
//! Keep the source-bound plotter artifact alive for the whole job. Layer providers
//! consume its structured operations; SVG is an output artifact, not geometry IR.

pub(crate) mod designator_layout;
#[allow(
    clippy::derivable_impls,
    reason = "Typify emits explicit Default implementations for generated contract DTOs"
)]
pub mod generated;
pub mod geometer;
mod job;
pub mod models;
pub mod presence;
pub mod preview;
mod virtual_layers;

pub use job::{PcbSvgJob, PcbSvgJobProfile};

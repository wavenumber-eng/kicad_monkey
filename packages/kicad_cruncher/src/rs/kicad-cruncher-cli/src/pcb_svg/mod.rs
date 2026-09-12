//! Shared native PCB SVG infrastructure for configurable views and Toon presets.
//!
//! Keep the source-bound plotter artifact alive for the whole job. Layer providers
//! consume its structured operations; SVG is an output artifact, not geometry IR.

pub mod generated;
pub mod presence;
mod job;
pub mod models;

pub use job::{PcbSvgJob, PcbSvgJobProfile};

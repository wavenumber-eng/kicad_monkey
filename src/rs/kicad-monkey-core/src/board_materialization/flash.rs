//! Effective copper layers of pads and vias with conditional flashing.
//!
//! Pads and vias that remove unused layers, keep end layers, or carry
//! per-zone layer connections flash only where KiCad's board plotter would.
//! Core's flash resolver (the one its board plotter uses) is the authority,
//! run on the parsed view only when a board uses those options.
//!
//! This module also builds the presentation plot document that places text.

use super::MaterializationError;
use crate::board_plotter_ir::{
    BoardLayerFlashResolver, BoardPlotLimits, FlashInputs, board_plot_document,
};
use crate::pcb::{PcbPad, PcbVia, PcbView};
use crate::sexpr_projection::{Selector, scan_form_spans};
use std::collections::HashMap;

/// Effective copper layers keyed by source UID.
#[derive(Default)]
pub(crate) struct FlashLayers {
    pub vias: HashMap<String, Vec<String>>,
    pub pads: HashMap<String, Vec<String>>,
    pub used_plot_facts: bool,
    /// Why conditional flashing could not be resolved, when it could not.
    pub unresolved: Option<String>,
}

fn needs_plot_facts(vias: &[PcbVia], pads: &[PcbPad]) -> bool {
    vias.iter().any(|via| {
        via.remove_unused_layers == Some(true)
            || via.keep_end_layers.is_some()
            || via.start_end_only == Some(true)
            || via.zone_layer_connections.is_some()
    }) || pads.iter().any(|pad| {
        pad.remove_unused_layers == Some(true)
            || pad.keep_end_layers.is_some()
            || pad.zone_layer_connections.is_some()
    })
}

fn blank(bytes: &mut [u8], range: std::ops::Range<usize>) {
    if let Some(span) = bytes.get_mut(range) {
        for byte in span
            .iter_mut()
            .filter(|byte| !matches!(**byte, b'\n' | b'\r'))
        {
            *byte = b' ';
        }
    }
}

fn into_source(bytes: Vec<u8>) -> Result<String, MaterializationError> {
    String::from_utf8(bytes)
        .map_err(|_| super::error("source_parse", "blanked source is not UTF-8"))
}

fn blank_top_level(
    source: &str,
    view: &PcbView<'_>,
    blank_head: impl Fn(&str) -> bool,
) -> Result<String, MaterializationError> {
    let mut bytes = source.as_bytes().to_vec();
    for span in view.top_level_forms() {
        if span.head.as_deref().is_some_and(&blank_head) {
            blank(&mut bytes, span.range.clone());
        }
    }
    into_source(bytes)
}

fn blank_heads(source: &str, heads: &[&str]) -> Result<String, MaterializationError> {
    let selector = Selector {
        heads: Some(heads.iter().map(|head| (*head).to_owned()).collect()),
        ..Selector::default()
    };
    let spans = scan_form_spans(source, &selector)
        .map_err(|error| super::error("source_parse", error.to_string()))?;
    let mut bytes = source.as_bytes().to_vec();
    for span in spans {
        blank(&mut bytes, span.range);
    }
    into_source(bytes)
}

/// Plot limits scaled to the source. Every count in a board is bounded by
/// its byte length, so these never reject a board the parser accepted,
/// while still bounding work by the input size.
fn plot_limits(source_len: usize) -> BoardPlotLimits {
    let defaults = BoardPlotLimits::default();
    let count = |default: usize| default.max(source_len);
    let bytes = |default: usize| default.max(source_len.saturating_mul(2));
    BoardPlotLimits {
        max_source_bytes: source_len.saturating_add(1).max(defaults.max_source_bytes),
        max_depth: defaults.max_depth,
        max_graphics: count(defaults.max_graphics),
        max_operations: count(defaults.max_operations),
        max_points: count(defaults.max_points),
        max_text_bytes: bytes(defaults.max_text_bytes),
        max_net_class_bytes: bytes(defaults.max_net_class_bytes),
        max_metadata_bytes: bytes(defaults.max_metadata_bytes),
        max_parse_nodes: count(defaults.max_parse_nodes),
        max_input_points: count(defaults.max_input_points),
        max_input_polygons: count(defaults.max_input_polygons),
        max_cache_polygons: count(defaults.max_cache_polygons),
        max_cache_contours: count(defaults.max_cache_contours),
    }
}

fn plot(source: &str) -> Result<crate::board_plotter_ir::BoardPlotDocument, crate::sexpr::Error> {
    board_plot_document(source, plot_limits(source.len()))
}

/// Footprint children the presentation plot never needs: pads, models,
/// zones, and shapes, which the materializer takes from the parsed source.
/// Text, text boxes (with their borders), and properties stay.
const PRESENTATION_BLANKED: [&str; 9] = [
    "pad",
    "model",
    "zone",
    "fp_line",
    "fp_arc",
    "fp_circle",
    "fp_rect",
    "fp_poly",
    "fp_curve",
];

/// Board plot document for text placement: routing, zones, pads, models,
/// and footprint shapes are blanked first, since only text, text boxes,
/// dimensions, and tables are read from it. Blanking keeps byte offsets.
/// `None` when the board cannot be plotted.
pub(crate) fn presentation_plot(
    source: &str,
    view: &PcbView<'_>,
) -> Option<crate::board_plotter_ir::BoardPlotDocument> {
    let stripped = blank_top_level(source, view, |head| {
        matches!(head, "segment" | "arc" | "via" | "zone")
    })
    .ok()?;
    let stripped = blank_heads(&stripped, &PRESENTATION_BLANKED).ok()?;
    plot(&stripped).ok()
}

/// Resolve flash layers with core's flash resolver (the one its board
/// plotter uses) from the decoded board, only when a board needs it.
pub(crate) fn flash_layers(inputs: &FlashInputs<'_>) -> FlashLayers {
    if !needs_plot_facts(inputs.vias, inputs.pads) {
        return FlashLayers::default();
    }
    let resolver = BoardLayerFlashResolver::from_decoded(inputs);
    let uid =
        |kind: &str, uuid: Option<&str>, start: usize| super::emit::source_uid(kind, uuid, start);
    FlashLayers {
        vias: inputs
            .vias
            .iter()
            .map(|via| {
                let key = uid("via", via.uuid.as_deref(), via.source_range.start);
                (key, copper_only(&resolver.via_flash_layers(via)))
            })
            .collect(),
        pads: inputs
            .pads
            .iter()
            .map(|pad| {
                let key = uid("pad", pad.uuid.as_deref(), pad.source_range.start);
                (key, copper_only(&resolver.pad_item_layers(pad)))
            })
            .collect(),
        used_plot_facts: true,
        unresolved: None,
    }
}

fn copper_only(layers: &[String]) -> Vec<String> {
    layers
        .iter()
        .filter(|layer| layer.ends_with(".Cu"))
        .cloned()
        .collect()
}

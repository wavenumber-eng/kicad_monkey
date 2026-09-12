//! Standalone-footprint property and text carrier decoding.

use crate::KiCadTextEffects;
use crate::footprint::FootprintView;
use crate::pcb::{
    standalone_property_from_span, standalone_text_box_from_span, standalone_text_from_span,
};
use crate::sexpr::Error;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct FootprintGraphicalProperty {
    pub name: String,
    pub value: String,
    pub at_x: f64,
    pub at_y: f64,
    pub angle: f64,
    pub layer: String,
    pub hidden: bool,
    pub unlocked: bool,
    pub graphical: bool,
    pub effects: KiCadTextEffects,
    pub render_cache_range: Option<Range<usize>>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootprintText {
    pub kind: String,
    pub text: String,
    pub at_x: f64,
    pub at_y: f64,
    pub angle: f64,
    pub layer: String,
    pub knockout: bool,
    pub hidden: bool,
    pub unlocked: bool,
    pub effects: KiCadTextEffects,
    pub render_cache_range: Option<Range<usize>>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootprintTextBox {
    pub text: String,
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub margins: [f64; 4],
    pub angle: f64,
    pub polygon_points: Vec<[f64; 2]>,
    pub layer: String,
    pub locked: bool,
    pub effects: Option<KiCadTextEffects>,
    pub stroke_width: Option<f64>,
    pub stroke_kind: Option<String>,
    pub border: Option<bool>,
    pub knockout: Option<bool>,
    pub render_cache_range: Option<Range<usize>>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

impl FootprintView<'_> {
    /// Decode graphical property facts from their selected top-level spans.
    pub fn graphical_properties(
        &self,
    ) -> impl Iterator<Item = Result<FootprintGraphicalProperty, Error>> + '_ {
        self.properties.iter().map(|span| {
            let value = standalone_property_from_span(self.source, span, self.pcb_limits())?;
            Ok(FootprintGraphicalProperty {
                name: value.name,
                value: value.value,
                at_x: value.at.x,
                at_y: value.at.y,
                angle: value.angle,
                layer: value.layer,
                hidden: value.hidden,
                unlocked: value.unlocked,
                graphical: value.graphical,
                effects: value.effects,
                render_cache_range: value.render_cache_range,
                uuid: value.uuid,
                source_range: value.source_range,
            })
        })
    }

    /// Decode footprint-local `fp_text` carriers in source order.
    pub fn texts(&self) -> impl Iterator<Item = Result<FootprintText, Error>> + '_ {
        self.texts.iter().map(|span| {
            let value = standalone_text_from_span(self.source, span, self.pcb_limits())?;
            Ok(FootprintText {
                kind: value.kind,
                text: value.text,
                at_x: value.at.x,
                at_y: value.at.y,
                angle: value.angle,
                layer: value.layer,
                knockout: value.knockout,
                hidden: value.hidden,
                unlocked: value.unlocked,
                effects: value.effects,
                render_cache_range: value.render_cache_range,
                uuid: value.uuid,
                source_range: value.source_range,
            })
        })
    }

    /// Decode standalone `fp_text_box` carriers in source order.
    pub fn text_boxes(&self) -> impl Iterator<Item = Result<FootprintTextBox, Error>> + '_ {
        self.text_boxes.iter().map(|span| {
            let value = standalone_text_box_from_span(self.source, span, self.pcb_limits())?;
            Ok(FootprintTextBox {
                text: value.text,
                start_x: value.start.x,
                start_y: value.start.y,
                end_x: value.end.x,
                end_y: value.end.y,
                margins: value.margins,
                angle: value.angle,
                polygon_points: value
                    .polygon_points
                    .into_iter()
                    .map(|point| [point.x, point.y])
                    .collect(),
                layer: value.layer,
                locked: value.locked,
                effects: value.effects,
                stroke_width: value.stroke_width,
                stroke_kind: value.stroke_kind,
                border: value.border,
                knockout: value.knockout,
                render_cache_range: value.render_cache_range,
                uuid: value.uuid,
                source_range: value.source_range,
            })
        })
    }
}

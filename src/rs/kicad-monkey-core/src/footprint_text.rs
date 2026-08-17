//! Standalone-footprint property and text carrier decoding.

use crate::footprint::{FootprintLimits, FootprintView, rebase_error};
use crate::plotter_ir::{child, model_error, numeric_at as parse_numeric_at, value_at};
use crate::sexpr::{Error, Limits, Position, Sexp, parse_with_limits};
use crate::{KiCadColor, KiCadFont, KiCadTextEffects};
use std::ops::Range;

const DEFAULT_TEXT_SIZE_MM: f64 = 1.27;

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
    pub border: Option<bool>,
    pub knockout: Option<bool>,
    pub source_range: Range<usize>,
}

impl FootprintView<'_> {
    /// Decode graphical property facts from their selected top-level spans.
    pub fn graphical_properties(
        &self,
    ) -> impl Iterator<Item = Result<FootprintGraphicalProperty, Error>> + '_ {
        self.properties.iter().map(|span| {
            let form = parse_span(self.source, span, self.limits)?;
            (|| {
                let effects = text_effects(&form)?.unwrap_or_default();
                let at = vector3(child(&form, "at"), [0.0, 0.0, 0.0])?;
                let layer = child(&form, "layer");
                Ok(FootprintGraphicalProperty {
                    name: required_value(&form, 1, "Expected footprint property name")?,
                    value: required_value(&form, 2, "Expected footprint property value")?,
                    at_x: at[0],
                    at_y: at[1],
                    angle: at[2],
                    layer: layer
                        .and_then(|value| value_at(value, 1))
                        .unwrap_or("F.SilkS")
                        .to_owned(),
                    hidden: named_bool(&form, "hide").unwrap_or(false) || effects.hidden,
                    unlocked: named_bool(&form, "unlocked").unwrap_or(false),
                    graphical: child(&form, "at").is_some() && layer.is_some(),
                    effects,
                    source_range: span.range.clone(),
                })
            })()
            .map_err(|error| rebase_error(error, span))
        })
    }

    /// Decode footprint-local `fp_text` carriers in source order.
    pub fn texts(&self) -> impl Iterator<Item = Result<FootprintText, Error>> + '_ {
        self.texts.iter().map(|span| {
            let form = parse_span(self.source, span, self.limits)?;
            (|| {
                let effects = text_effects(&form)?.unwrap_or_default();
                let at = vector3(child(&form, "at"), [0.0, 0.0, 0.0])?;
                let layer_form = child(&form, "layer");
                Ok(FootprintText {
                    kind: required_value(&form, 1, "Expected footprint text kind")?,
                    text: required_value(&form, 2, "Expected footprint text value")?,
                    at_x: at[0],
                    at_y: at[1],
                    angle: at[2],
                    layer: layer_form
                        .and_then(|value| value_at(value, 1))
                        .unwrap_or("F.SilkS")
                        .to_owned(),
                    knockout: layer_form.is_some_and(|value| has_flag(value, "knockout")),
                    hidden: named_bool(&form, "hide").unwrap_or(false) || effects.hidden,
                    unlocked: named_bool(&form, "unlocked").unwrap_or(false),
                    effects,
                    source_range: span.range.clone(),
                })
            })()
            .map_err(|error| rebase_error(error, span))
        })
    }

    /// Decode standalone `fp_text_box` carriers in source order.
    pub fn text_boxes(&self) -> impl Iterator<Item = Result<FootprintTextBox, Error>> + '_ {
        self.text_boxes.iter().map(|span| {
            let form = parse_span(self.source, span, self.limits)?;
            (|| {
                let polygon_points = points(child(&form, "pts"))?;
                let mut start = vector2(child(&form, "start"), [0.0, 0.0])?;
                let mut end = vector2(child(&form, "end"), [0.0, 0.0])?;
                if !polygon_points.is_empty()
                    && (child(&form, "start").is_none() || child(&form, "end").is_none())
                {
                    start = [
                        extent(&polygon_points, 0, f64::min),
                        extent(&polygon_points, 1, f64::min),
                    ];
                    end = [
                        extent(&polygon_points, 0, f64::max),
                        extent(&polygon_points, 1, f64::max),
                    ];
                }
                let margins = match child(&form, "margins") {
                    Some(value) if list_values(value).is_some_and(|values| values.len() >= 5) => {
                        vector4(Some(value), [0.0; 4])?
                    }
                    _ => [0.0; 4],
                };
                let stroke = child(&form, "stroke");
                Ok(FootprintTextBox {
                    text: value_at(&form, 1).unwrap_or_default().to_owned(),
                    start_x: start[0],
                    start_y: start[1],
                    end_x: end[0],
                    end_y: end[1],
                    margins,
                    angle: scalar(child(&form, "angle"), 0.0)?,
                    polygon_points,
                    layer: child(&form, "layer")
                        .and_then(|value| value_at(value, 1))
                        .unwrap_or("F.SilkS")
                        .to_owned(),
                    locked: named_bool(&form, "locked").unwrap_or(false),
                    effects: text_effects(&form)?,
                    stroke_width: stroke
                        .and_then(|value| child(value, "width"))
                        .map(|value| numeric_at(value, 1, Position::START))
                        .transpose()?,
                    border: named_bool(&form, "border"),
                    knockout: named_bool(&form, "knockout"),
                    source_range: span.range.clone(),
                })
            })()
            .map_err(|error| rebase_error(error, span))
        })
    }
}

fn parse_span(
    source: &str,
    span: &crate::sexpr_projection::FormSpan,
    limits: FootprintLimits,
) -> Result<Sexp, Error> {
    let text = span.text(source)?;
    parse_with_limits(
        text,
        Limits {
            max_source_bytes: text.len(),
            max_depth: limits.max_depth,
            max_nodes: limits.max_object_nodes,
            max_decoded_string_bytes: limits.max_source_bytes,
        },
    )
    .map_err(|error| rebase_error(error, span))
}

fn required_value(form: &Sexp, index: usize, message: &'static str) -> Result<String, Error> {
    value_at(form, index)
        .map(str::to_owned)
        .ok_or_else(|| model_error(message, Position::START))
}

fn numeric_at(form: &Sexp, index: usize, position: Position) -> Result<f64, Error> {
    let value = parse_numeric_at(form, index, position)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(model_error(
            "Expected finite footprint text numeric value",
            position,
        ))
    }
}

fn text_effects(form: &Sexp) -> Result<Option<KiCadTextEffects>, Error> {
    let Some(effects) = child(form, "effects") else {
        return Ok(None);
    };
    let font_form = child(effects, "font");
    let size = font_form.and_then(|font| child(font, "size"));
    let font = KiCadFont {
        face: font_form
            .and_then(|font| child(font, "face"))
            .and_then(|value| value_at(value, 1))
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        size_y: numeric_or(size, 1, DEFAULT_TEXT_SIZE_MM)?,
        size_x: numeric_or(size, 2, DEFAULT_TEXT_SIZE_MM)?,
        thickness: font_form
            .and_then(|font| child(font, "thickness"))
            .map(|value| numeric_at(value, 1, Position::START))
            .transpose()?,
        bold: font_form.is_some_and(|font| named_bool(font, "bold").unwrap_or(false)),
        italic: font_form.is_some_and(|font| named_bool(font, "italic").unwrap_or(false)),
        line_spacing: font_form
            .and_then(|font| child(font, "line_spacing"))
            .map(|value| numeric_at(value, 1, Position::START))
            .transpose()?,
        color: font_form
            .and_then(|font| child(font, "color"))
            .map(color)
            .transpose()?,
    };
    let justify = child(effects, "justify")
        .and_then(list_values)
        .map(|values| {
            values[1..]
                .iter()
                .filter_map(text_value)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(KiCadTextEffects {
        font,
        justify,
        hidden: named_bool(effects, "hide").unwrap_or(false),
        href: child(effects, "href")
            .and_then(|value| value_at(value, 1))
            .map(str::to_owned),
        source_range: None,
    }))
}

fn color(form: &Sexp) -> Result<KiCadColor, Error> {
    Ok(KiCadColor {
        red: numeric_at(form, 1, Position::START)? as i64,
        green: numeric_at(form, 2, Position::START)? as i64,
        blue: numeric_at(form, 3, Position::START)? as i64,
        alpha: numeric_at(form, 4, Position::START)?,
    })
}

fn points(form: Option<&Sexp>) -> Result<Vec<[f64; 2]>, Error> {
    let Some(values) = form.and_then(list_values) else {
        return Ok(Vec::new());
    };
    values[1..]
        .iter()
        .filter(|value| {
            list_values(value).is_some_and(|point| point.len() >= 3)
                && value_at(value, 0) == Some("xy")
        })
        .map(|value| {
            Ok([
                numeric_at(value, 1, Position::START)?,
                numeric_at(value, 2, Position::START)?,
            ])
        })
        .collect()
}

fn extent(points: &[[f64; 2]], axis: usize, operation: fn(f64, f64) -> f64) -> f64 {
    points
        .iter()
        .map(|point| point[axis])
        .reduce(operation)
        .unwrap_or(0.0)
}

fn vector2(form: Option<&Sexp>, default: [f64; 2]) -> Result<[f64; 2], Error> {
    Ok([
        numeric_or(form, 1, default[0])?,
        numeric_or(form, 2, default[1])?,
    ])
}

fn vector3(form: Option<&Sexp>, default: [f64; 3]) -> Result<[f64; 3], Error> {
    Ok([
        numeric_or(form, 1, default[0])?,
        numeric_or(form, 2, default[1])?,
        numeric_or(form, 3, default[2])?,
    ])
}

fn vector4(form: Option<&Sexp>, default: [f64; 4]) -> Result<[f64; 4], Error> {
    Ok([
        numeric_or(form, 1, default[0])?,
        numeric_or(form, 2, default[1])?,
        numeric_or(form, 3, default[2])?,
        numeric_or(form, 4, default[3])?,
    ])
}

fn scalar(form: Option<&Sexp>, default: f64) -> Result<f64, Error> {
    numeric_or(form, 1, default)
}

fn numeric_or(form: Option<&Sexp>, index: usize, default: f64) -> Result<f64, Error> {
    match form {
        Some(value) if list_values(value).is_some_and(|values| values.len() > index) => {
            numeric_at(value, index, Position::START)
        }
        _ => Ok(default),
    }
}

fn named_bool(form: &Sexp, name: &str) -> Option<bool> {
    if has_flag(form, name) {
        return Some(true);
    }
    let value = child(form, name)?;
    if list_values(value).is_none_or(|values| values.len() <= 1) {
        return Some(true);
    }
    Some(value_at(value, 1) == Some("yes"))
}

fn has_flag(form: &Sexp, name: &str) -> bool {
    list_values(form)
        .into_iter()
        .flatten()
        .any(|value| text_value(value) == Some(name))
}

fn list_values(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text_value(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

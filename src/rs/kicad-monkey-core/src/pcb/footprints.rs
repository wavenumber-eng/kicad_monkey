//! Embedded-footprint child views shared by PCB readers and later plotters.

use super::*;
use crate::{KiCadColor, KiCadFont};

const MAX_PROPERTY_HEADER_SCALARS: usize = 256;
const MAX_EFFECTS_FLAGS: usize = 64;

/// One source-authored property owned by an embedded board footprint.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbFootprintProperty {
    pub footprint_index: usize,
    pub name: String,
    pub value: String,
    pub at: PcbPoint,
    pub angle: f64,
    pub layer: String,
    pub hidden: bool,
    pub unlocked: bool,
    pub graphical: bool,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

/// One non-text graphic owned by an embedded board footprint.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbFootprintGraphic {
    pub footprint_index: usize,
    pub graphic: PcbGraphic,
}

/// One source-authored footprint-local text item.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbFootprintText {
    pub footprint_index: usize,
    pub kind: String,
    pub text: String,
    pub at: PcbPoint,
    pub angle: f64,
    pub layer: String,
    pub knockout: bool,
    pub hidden: bool,
    pub uuid: Option<String>,
    pub effects: KiCadTextEffects,
    pub render_cache_range: Option<Range<usize>>,
    pub source_range: Range<usize>,
}

/// One source-authored footprint-local text box.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbFootprintTextBox {
    pub footprint_index: usize,
    pub text: String,
    pub start: PcbPoint,
    pub end: PcbPoint,
    pub margins: [f64; 4],
    pub angle: f64,
    pub polygon_points: Vec<PcbPoint>,
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

impl<'a> PcbView<'a> {
    /// Iterate requested footprint properties in board and child source order.
    pub fn footprint_properties(
        &self,
    ) -> impl Iterator<Item = Result<PcbFootprintProperty, Error>> + '_ {
        self.footprint_properties
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::FootprintProperties))
            .map(|indexed| footprint_property_from_span(self.source, indexed, self.limits))
    }

    /// Iterate requested non-text footprint graphics in board and child source order.
    pub fn footprint_graphics(
        &self,
    ) -> impl Iterator<Item = Result<PcbFootprintGraphic, Error>> + '_ {
        self.footprint_graphics
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::FootprintGraphics))
            .map(|indexed| {
                Ok(PcbFootprintGraphic {
                    footprint_index: indexed.parent_index,
                    graphic: graphic_from_span(self.source, &indexed.span, self.limits)?,
                })
            })
    }

    /// Iterate footprint-local text in board and child source order.
    pub fn footprint_texts(&self) -> impl Iterator<Item = Result<PcbFootprintText, Error>> + '_ {
        self.footprint_texts
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::FootprintTexts))
            .map(|indexed| footprint_text_from_span(self.source, indexed, self.limits))
    }

    /// Iterate footprint-local text boxes in board and child source order.
    pub fn footprint_text_boxes(
        &self,
    ) -> impl Iterator<Item = Result<PcbFootprintTextBox, Error>> + '_ {
        self.footprint_text_boxes
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::FootprintTextBoxes))
            .map(|indexed| footprint_text_box_from_span(self.source, indexed, self.limits))
    }
}

fn footprint_text_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbFootprintText, Error> {
    let header = bounded_scalar_values(source, &indexed.span, MAX_PROPERTY_HEADER_SCALARS)?;
    let children = direct_children(source, &indexed.span, limits.max_object_children, limits)?;
    let at = optional_vector(source, &children, "at", [0.0, 0.0, 0.0])?;
    let effects = text_effects_from_children(source, &children, limits)?.unwrap_or_default();
    let (layer, knockout) = text_layer(source, &children)?;
    Ok(PcbFootprintText {
        footprint_index: indexed.parent_index,
        kind: required_string(
            header.first(),
            "Expected footprint text kind",
            &indexed.span,
        )?,
        text: required_string(header.get(1), "Expected footprint text", &indexed.span)?,
        at: PcbPoint { x: at[0], y: at[1] },
        angle: at[2],
        layer,
        knockout,
        hidden: has_flag(&header, "hide")
            || child_bool(source, &children, "hide")?
            || effects.hidden,
        uuid: optional_uuid(source, &children)?,
        effects,
        render_cache_range: child(&children, "render_cache").map(|span| span.range.clone()),
        source_range: indexed.span.range.clone(),
    })
}

fn footprint_text_box_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbFootprintTextBox, Error> {
    let header = bounded_scalar_values(source, &indexed.span, MAX_PROPERTY_HEADER_SCALARS)?;
    let children = direct_children(source, &indexed.span, limits.max_object_children, limits)?;
    let polygon_points = child(&children, "pts")
        .map(|span| text_box_points(source, span, limits))
        .transpose()?
        .unwrap_or_default();
    let mut start =
        optional_child_point(source, &children, "start")?.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    let mut end =
        optional_child_point(source, &children, "end")?.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    if !polygon_points.is_empty()
        && (child(&children, "start").is_none() || child(&children, "end").is_none())
    {
        let (min_x, max_x) = point_extents(&polygon_points, |point| point.x);
        let (min_y, max_y) = point_extents(&polygon_points, |point| point.y);
        start = PcbPoint { x: min_x, y: min_y };
        end = PcbPoint { x: max_x, y: max_y };
    }
    let margins = text_box_margins(source, &children)?;
    let (stroke_width, stroke_kind) = text_box_stroke(source, &children, limits)?;
    Ok(PcbFootprintTextBox {
        footprint_index: indexed.parent_index,
        text: header.first().map(token_string).unwrap_or_default(),
        start,
        end,
        margins,
        angle: optional_child_f64(source, &children, "angle")?.unwrap_or(0.0),
        polygon_points,
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.SilkS".to_owned()),
        locked: optional_named_bool(source, &header, &children, "locked")?.unwrap_or(false),
        effects: text_effects_from_children(source, &children, limits)?,
        stroke_width,
        stroke_kind,
        border: optional_named_bool(source, &header, &children, "border")?,
        knockout: optional_named_bool(source, &header, &children, "knockout")?,
        render_cache_range: child(&children, "render_cache").map(|span| span.range.clone()),
        uuid: optional_uuid(source, &children)?,
        source_range: indexed.span.range.clone(),
    })
}

fn text_effects_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<KiCadTextEffects>, Error> {
    let Some(effects) = child(children, "effects") else {
        return Ok(None);
    };
    let header = bounded_scalar_values(source, effects, MAX_EFFECTS_FLAGS)?;
    let fields = direct_children(source, effects, limits.max_text_effect_children, limits)?;
    let justify = child(&fields, "justify")
        .map(|span| bounded_scalar_values(source, span, limits.max_text_justify_tokens))
        .transpose()?
        .unwrap_or_default()
        .iter()
        .map(token_string)
        .collect();
    Ok(Some(KiCadTextEffects {
        font: text_font_from_children(source, &fields, limits)?,
        justify,
        hidden: has_flag(&header, "hide") || child_bool(source, &fields, "hide")?,
        href: optional_child_string(source, &fields, "href")?,
        source_range: Some(effects.range.clone()),
    }))
}

fn text_font_from_children(
    source: &str,
    effects_children: &[FormSpan],
    limits: PcbLimits,
) -> Result<KiCadFont, Error> {
    let Some(font) = child(effects_children, "font") else {
        return Ok(KiCadFont::default());
    };
    let header = bounded_scalar_values(source, font, MAX_EFFECTS_FLAGS)?;
    let fields = direct_children(source, font, limits.max_text_font_children, limits)?;
    let size = child(&fields, "size")
        .map(|span| bounded_scalar_values(source, span, 2))
        .transpose()?
        .unwrap_or_default();
    Ok(KiCadFont {
        face: optional_child_string(source, &fields, "face")?,
        size_y: optional_f64(size.first(), font)?.unwrap_or(1.27),
        size_x: optional_f64(size.get(1), font)?.unwrap_or(1.27),
        thickness: optional_child_f64(source, &fields, "thickness")?,
        bold: has_flag(&header, "bold") || child_bool(source, &fields, "bold")?,
        italic: has_flag(&header, "italic") || child_bool(source, &fields, "italic")?,
        line_spacing: optional_child_f64(source, &fields, "line_spacing")?,
        color: optional_color(source, &fields)?,
    })
}

fn optional_color(source: &str, children: &[FormSpan]) -> Result<Option<KiCadColor>, Error> {
    let Some(color) = child(children, "color") else {
        return Ok(None);
    };
    let values = bounded_scalar_values(source, color, 4)?;
    if values.len() < 4 {
        return Ok(None);
    }
    Ok(Some(KiCadColor {
        red: parse_i64(&values[0], color)?,
        green: parse_i64(&values[1], color)?,
        blue: parse_i64(&values[2], color)?,
        alpha: parse_f64(&values[3], color)?,
    }))
}

fn text_layer(source: &str, children: &[FormSpan]) -> Result<(String, bool), Error> {
    let Some(layer) = child(children, "layer") else {
        return Ok(("F.SilkS".to_owned(), false));
    };
    let values = bounded_scalar_values(source, layer, 8)?;
    Ok((
        values
            .first()
            .map(token_string)
            .unwrap_or_else(|| "F.SilkS".to_owned()),
        has_flag(&values, "knockout"),
    ))
}

fn text_box_points(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<Vec<PcbPoint>, Error> {
    let mut result = Vec::new();
    for point in direct_children(source, span, limits.max_text_box_points, limits)?
        .into_iter()
        .filter(|point| point.head.as_deref() == Some("xy"))
    {
        let values = first_two_scalar_values(source, &point)?;
        let [x, y] = values.as_slice() else {
            continue;
        };
        result.push(PcbPoint {
            x: parse_f64(x, &point)?,
            y: parse_f64(y, &point)?,
        });
    }
    Ok(result)
}

fn text_box_margins(source: &str, children: &[FormSpan]) -> Result<[f64; 4], Error> {
    let Some(margins) = child(children, "margins") else {
        return Ok([0.0; 4]);
    };
    let values = bounded_scalar_values(source, margins, 4)?;
    let mut result = [0.0; 4];
    for (index, value) in result.iter_mut().enumerate() {
        *value = optional_f64(values.get(index), margins)?.unwrap_or(0.0);
    }
    Ok(result)
}

fn point_extents(points: &[PcbPoint], coordinate: impl Fn(&PcbPoint) -> f64) -> (f64, f64) {
    points.iter().map(coordinate).fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
    )
}

fn text_box_stroke(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<(Option<f64>, Option<String>), Error> {
    let Some(stroke) = child(children, "stroke") else {
        return Ok((None, None));
    };
    let fields = direct_children(source, stroke, limits.max_text_effect_children, limits)?;
    Ok((
        optional_child_f64(source, &fields, "width")?,
        optional_child_string(source, &fields, "type")?,
    ))
}

fn optional_named_bool(
    source: &str,
    header: &[Token<'_>],
    children: &[FormSpan],
    name: &str,
) -> Result<Option<bool>, Error> {
    if has_flag(header, name) {
        return Ok(Some(true));
    }
    let Some(field) = child(children, name) else {
        return Ok(None);
    };
    Ok(Some(
        first_string(source, field)?.is_none_or(|value| value == "yes"),
    ))
}

fn footprint_property_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbFootprintProperty, Error> {
    let header = bounded_scalar_values(source, &indexed.span, MAX_PROPERTY_HEADER_SCALARS)?;
    let children = direct_children(source, &indexed.span, limits.max_object_children, limits)?;
    let at = optional_vector(source, &children, "at", [0.0, 0.0, 0.0])?;
    let graphical = child(&children, "at").is_some() && child(&children, "layer").is_some();
    let hidden = has_flag(&header, "hide")
        || child_bool(source, &children, "hide")?
        || effects_hidden(source, &children, limits)?;
    Ok(PcbFootprintProperty {
        footprint_index: indexed.parent_index,
        name: required_string(
            header.first(),
            "Expected footprint property name",
            &indexed.span,
        )?,
        value: required_string(
            header.get(1),
            "Expected footprint property value",
            &indexed.span,
        )?,
        at: PcbPoint { x: at[0], y: at[1] },
        angle: at[2],
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.SilkS".to_owned()),
        hidden,
        unlocked: has_flag(&header, "unlocked") || child_bool(source, &children, "unlocked")?,
        graphical,
        uuid: optional_uuid(source, &children)?,
        source_range: indexed.span.range.clone(),
    })
}

fn effects_hidden(source: &str, children: &[FormSpan], limits: PcbLimits) -> Result<bool, Error> {
    let Some(effects) = child(children, "effects") else {
        return Ok(false);
    };
    let header = bounded_scalar_values(source, effects, MAX_EFFECTS_FLAGS)?;
    let fields = direct_children(source, effects, limits.max_object_children, limits)?;
    Ok(has_flag(&header, "hide") || child_bool(source, &fields, "hide")?)
}

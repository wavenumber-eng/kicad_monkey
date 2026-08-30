//! Shared pad/via manufacturing subrecords.

use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PcbDrillLayerSpan {
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbDrillProperties {
    pub size: Option<f64>,
    pub layers: PcbDrillLayerSpan,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbPostMachiningProperties {
    pub mode: String,
    pub size: Option<f64>,
    pub depth: Option<f64>,
    pub angle: Option<f64>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbZoneLayerConnections {
    pub forced_layers: Vec<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbTeardropParameters {
    pub best_length_ratio: Option<f64>,
    pub max_length: Option<f64>,
    pub best_width_ratio: Option<f64>,
    pub max_width: Option<f64>,
    pub curved_edges: Option<bool>,
    pub filter_ratio: Option<f64>,
    pub enabled: Option<bool>,
    pub allow_two_segments: Option<bool>,
    pub prefer_zone_connections: Option<bool>,
    pub source_range: Range<usize>,
}

pub(super) fn drill_properties_from_children(
    source: &str,
    children: &[FormSpan],
    head: &str,
    limits: PcbLimits,
) -> Result<Option<PcbDrillProperties>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_manufacturing_children, limits)?;
    let size = optional_child_f64(source, &fields, "size")?;
    let layers = if let Some(layer_span) = child(&fields, "layers") {
        let values = first_two_scalar_values(source, layer_span)?;
        if let [start, end] = values.as_slice() {
            PcbDrillLayerSpan {
                start: token_string(start),
                end: token_string(end),
            }
        } else {
            PcbDrillLayerSpan::default()
        }
    } else {
        PcbDrillLayerSpan::default()
    };
    if size.is_none() && layers == PcbDrillLayerSpan::default() {
        return Ok(None);
    }
    Ok(Some(PcbDrillProperties {
        size,
        layers,
        source_range: span.range.clone(),
    }))
}

pub(super) fn post_machining_from_children(
    source: &str,
    children: &[FormSpan],
    head: &str,
    limits: PcbLimits,
) -> Result<Option<PcbPostMachiningProperties>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let Some(mode) = first_string(source, span)?.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_manufacturing_children, limits)?;
    Ok(Some(PcbPostMachiningProperties {
        mode,
        size: optional_child_f64(source, &fields, "size")?,
        depth: optional_child_f64(source, &fields, "depth")?,
        angle: optional_child_f64(source, &fields, "angle")?,
        source_range: span.range.clone(),
    }))
}

pub(super) fn zone_layer_connections_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbZoneLayerConnections>, Error> {
    let Some(span) = child(children, "zone_layer_connections") else {
        return Ok(None);
    };
    Ok(Some(PcbZoneLayerConnections {
        forced_layers: bounded_scalar_values(source, span, limits.max_zone_layer_connections)?
            .iter()
            .map(token_string)
            .collect(),
        source_range: span.range.clone(),
    }))
}

pub(super) fn optional_presence_bool(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<bool>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    Ok(Some(first_string(source, span)?.is_none_or(|value| {
        matches!(value.to_ascii_lowercase().as_str(), "yes" | "true" | "1")
    })))
}

pub(super) fn teardrop_parameters_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbTeardropParameters>, Error> {
    let Some(span) = child(children, "teardrops") else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_manufacturing_children, limits)?;
    let bare = bare_teardrop_values(source, span, limits.max_teardrop_scalars)?;
    Ok(Some(PcbTeardropParameters {
        best_length_ratio: teardrop_f64(source, &fields, &bare, "best_length_ratio", span)?,
        max_length: teardrop_f64(source, &fields, &bare, "max_length", span)?,
        best_width_ratio: teardrop_f64(source, &fields, &bare, "best_width_ratio", span)?,
        max_width: teardrop_f64(source, &fields, &bare, "max_width", span)?,
        curved_edges: teardrop_bool(source, &fields, &bare, "curved_edges", span)?,
        filter_ratio: teardrop_f64(source, &fields, &bare, "filter_ratio", span)?,
        enabled: teardrop_bool(source, &fields, &bare, "enabled", span)?,
        allow_two_segments: teardrop_bool(source, &fields, &bare, "allow_two_segments", span)?,
        prefer_zone_connections: teardrop_bool(
            source,
            &fields,
            &bare,
            "prefer_zone_connections",
            span,
        )?,
        source_range: span.range.clone(),
    }))
}

fn teardrop_f64(
    source: &str,
    fields: &[FormSpan],
    bare: &[(String, Token<'_>)],
    head: &str,
    span: &FormSpan,
) -> Result<Option<f64>, Error> {
    if let Some(value) = optional_child_f64(source, fields, head)? {
        return Ok(Some(value));
    }
    bare.iter()
        .find(|(key, _)| key == head)
        .map(|(_, value)| parse_f64(value, span))
        .transpose()
}

fn teardrop_bool(
    source: &str,
    fields: &[FormSpan],
    bare: &[(String, Token<'_>)],
    head: &str,
    parent: &FormSpan,
) -> Result<Option<bool>, Error> {
    if let Some(span) = child(fields, head) {
        return first_scalar_value(source, span)?
            .as_ref()
            .map(|token| parse_teardrop_bool(token, span))
            .transpose();
    }
    bare.iter()
        .find(|(key, _)| key == head)
        .map(|(_, token)| parse_teardrop_bool(token, parent))
        .transpose()
}

fn parse_teardrop_bool(token: &Token<'_>, span: &FormSpan) -> Result<bool, Error> {
    match token.lexeme.to_ascii_lowercase().as_str() {
        "yes" | "true" => Ok(true),
        "no" | "false" => Ok(false),
        _ => Err(source_error(
            "Expected yes/no for teardrops field",
            rebase_position(token.position, span),
        )),
    }
}

fn bare_teardrop_values<'a>(
    source: &'a str,
    span: &FormSpan,
    maximum: usize,
) -> Result<Vec<(String, Token<'a>)>, Error> {
    let values = bounded_scalar_values(source, span, maximum)?;
    Ok(values
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| (token_string(&pair[0]), pair[1].clone()))
        .collect())
}

//! Detailed embedded-footprint pad inputs for native readers and plotters.

use super::*;

/// Custom-pad clearance and anchor policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbPadCustomOptions {
    pub clearance: Option<String>,
    pub anchor: Option<String>,
    pub source_range: Range<usize>,
}

/// One custom-pad primitive. Polygon geometry is typed; unsupported kinds are
/// retained as named, source-evidenced records for deterministic deferral.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbPadCustomPrimitive {
    pub kind: String,
    pub points: Vec<PcbPoint>,
    pub width: Option<f64>,
    pub fill: Option<String>,
    pub source_range: Range<usize>,
}

/// One typed footprint pad in board source order.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbPad {
    pub footprint_index: usize,
    pub number: String,
    pub kind: String,
    pub shape: String,
    pub at_x: f64,
    pub at_y: f64,
    pub angle: f64,
    pub size_x: f64,
    pub size_y: f64,
    pub drill: Option<PcbPadDrill>,
    pub layers: Vec<String>,
    pub net: PcbNetRef,
    pub uuid: Option<String>,
    pub pin_function: Option<String>,
    pub pin_type: Option<String>,
    pub die_length: Option<f64>,
    pub rect_delta_x: Option<f64>,
    pub rect_delta_y: Option<f64>,
    pub roundrect_rratio: Option<f64>,
    pub chamfer_ratio: Option<f64>,
    pub chamfer_corners: Vec<String>,
    pub solder_mask_margin: Option<f64>,
    pub solder_paste_margin: Option<f64>,
    pub solder_paste_margin_ratio: Option<f64>,
    pub clearance: Option<f64>,
    pub thermal_bridge_width: Option<f64>,
    pub thermal_bridge_angle: Option<f64>,
    pub thermal_gap: Option<f64>,
    pub zone_connect: Option<i64>,
    pub remove_unused_layers: Option<bool>,
    pub keep_end_layers: Option<bool>,
    pub teardrops: Option<PcbTeardropParameters>,
    pub backdrill: Option<PcbDrillProperties>,
    pub tertiary_drill: Option<PcbDrillProperties>,
    pub front_post_machining: Option<PcbPostMachiningProperties>,
    pub back_post_machining: Option<PcbPostMachiningProperties>,
    pub zone_layer_connections: Option<PcbZoneLayerConnections>,
    pub custom_options: Option<PcbPadCustomOptions>,
    pub custom_primitives: Vec<PcbPadCustomPrimitive>,
    pub source_range: Range<usize>,
}

pub(super) fn pad_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbPad, Error> {
    let header = bounded_scalar_values(source, &indexed.span, limits.max_pad_header_scalars)?;
    let children = direct_children(source, &indexed.span, limits.max_pad_children, limits)?;
    let at = optional_vector(source, &children, "at", [0.0, 0.0, 0.0])?;
    let size = optional_pair(source, &children, "size", [0.0, 0.0])?;
    let (rect_delta_x, rect_delta_y) = optional_complete_pair(source, &children, "rect_delta")?;
    Ok(PcbPad {
        footprint_index: indexed.parent_index,
        number: required_string(header.first(), "Expected pad number", &indexed.span)?,
        kind: required_string(header.get(1), "Expected pad kind", &indexed.span)?,
        shape: required_string(header.get(2), "Expected pad shape", &indexed.span)?,
        at_x: at[0],
        at_y: at[1],
        angle: at[2],
        size_x: size[0],
        size_y: size[1],
        drill: physical::pad_drill_from_children(source, &children, limits)?,
        layers: child_strings(source, &children, "layers", limits.max_layers)?,
        net: child_net_ref(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        pin_function: optional_child_string(source, &children, "pinfunction")?,
        pin_type: optional_child_string(source, &children, "pintype")?,
        die_length: optional_child_f64(source, &children, "die_length")?,
        rect_delta_x,
        rect_delta_y,
        roundrect_rratio: optional_child_f64(source, &children, "roundrect_rratio")?,
        chamfer_ratio: optional_child_f64(source, &children, "chamfer_ratio")?,
        chamfer_corners: child_strings(
            source,
            &children,
            "chamfer",
            limits.max_pad_chamfer_corners,
        )?,
        solder_mask_margin: optional_child_f64(source, &children, "solder_mask_margin")?,
        solder_paste_margin: optional_child_f64(source, &children, "solder_paste_margin")?,
        solder_paste_margin_ratio: optional_child_f64(
            source,
            &children,
            "solder_paste_margin_ratio",
        )?,
        clearance: tolerant_optional_child_f64(source, &children, "clearance")?,
        thermal_bridge_width: optional_child_f64(source, &children, "thermal_bridge_width")?,
        thermal_bridge_angle: optional_child_f64(source, &children, "thermal_bridge_angle")?,
        thermal_gap: optional_child_f64(source, &children, "thermal_gap")?,
        zone_connect: optional_child_i64(source, &children, "zone_connect")?,
        remove_unused_layers: optional_presence_bool(source, &children, "remove_unused_layers")?,
        keep_end_layers: optional_presence_bool(source, &children, "keep_end_layers")?,
        teardrops: manufacturing::teardrop_parameters_from_children(source, &children, limits)?,
        backdrill: manufacturing::drill_properties_from_children(
            source,
            &children,
            "backdrill",
            limits,
        )?,
        tertiary_drill: manufacturing::drill_properties_from_children(
            source,
            &children,
            "tertiary_drill",
            limits,
        )?,
        front_post_machining: manufacturing::post_machining_from_children(
            source,
            &children,
            "front_post_machining",
            limits,
        )?,
        back_post_machining: manufacturing::post_machining_from_children(
            source,
            &children,
            "back_post_machining",
            limits,
        )?,
        zone_layer_connections: manufacturing::zone_layer_connections_from_children(
            source, &children, limits,
        )?,
        custom_options: custom_options_from_children(source, &children, limits)?,
        custom_primitives: custom_primitives_from_children(source, &children, limits)?,
        source_range: indexed.span.range.clone(),
    })
}

fn optional_complete_pair(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<(Option<f64>, Option<f64>), Error> {
    let Some(span) = child(children, head) else {
        return Ok((None, None));
    };
    let values = first_two_scalar_values(source, span)?;
    let [x, y] = values.as_slice() else {
        return Ok((None, None));
    };
    Ok((Some(parse_f64(x, span)?), Some(parse_f64(y, span)?)))
}

fn tolerant_optional_child_f64(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<f64>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    Ok(first_string(source, span)?.and_then(|value| value.parse().ok()))
}

fn optional_presence_bool(
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

fn custom_options_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbPadCustomOptions>, Error> {
    let Some(options) = child(children, "options") else {
        return Ok(None);
    };
    let fields = direct_children(source, options, limits.max_pad_children, limits)?;
    Ok(Some(PcbPadCustomOptions {
        clearance: optional_child_string(source, &fields, "clearance")?
            .filter(|value| !value.is_empty()),
        anchor: optional_child_string(source, &fields, "anchor")?.filter(|value| !value.is_empty()),
        source_range: options.range.clone(),
    }))
}

fn custom_primitives_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Vec<PcbPadCustomPrimitive>, Error> {
    let Some(primitives) = child(children, "primitives") else {
        return Ok(Vec::new());
    };
    let forms = direct_children(source, primitives, limits.max_pad_custom_primitives, limits)?;
    let mut point_count = 0usize;
    forms
        .into_iter()
        .map(|primitive| custom_primitive_from_span(source, primitive, limits, &mut point_count))
        .collect()
}

fn custom_primitive_from_span(
    source: &str,
    primitive: FormSpan,
    limits: PcbLimits,
    point_count: &mut usize,
) -> Result<PcbPadCustomPrimitive, Error> {
    let fields = direct_children(source, &primitive, limits.max_pad_children, limits)?;
    let mut points = Vec::new();
    if let Some(container) = child(&fields, "pts") {
        for point in direct_children(source, container, limits.max_pad_custom_point_forms, limits)?
            .into_iter()
            .filter(|point| point.head.as_deref() == Some("xy"))
        {
            let values = first_two_scalar_values(source, &point)?;
            let [x, y] = values.as_slice() else {
                continue;
            };
            if *point_count >= limits.max_pad_custom_points {
                return Err(limit_error());
            }
            points.push(PcbPoint {
                x: parse_f64(x, &point)?,
                y: parse_f64(y, &point)?,
            });
            *point_count += 1;
        }
    }
    Ok(PcbPadCustomPrimitive {
        kind: primitive.head.clone().unwrap_or_default(),
        points,
        width: optional_child_f64(source, &fields, "width")?,
        fill: optional_child_string(source, &fields, "fill")?
            .filter(|value| matches!(value.as_str(), "yes" | "solid" | "no")),
        source_range: primitive.range,
    })
}

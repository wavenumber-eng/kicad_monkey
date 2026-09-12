//! Detailed embedded-footprint pad inputs for native readers and plotters.

use super::*;
mod custom;
pub use custom::{PcbPadPolygonPoint, PcbPadPrimitiveGeometry};

/// Custom-pad clearance and anchor policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbPadCustomOptions {
    pub clearance: Option<String>,
    pub anchor: Option<String>,
    pub source_range: Range<usize>,
}

/// One source-local custom-pad primitive, with policy declaration evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbPadCustomPrimitive {
    pub kind: String,
    /// Compatibility XY projection of `pts`; use geometry for complete polygons
    /// containing arcs and for non-polygon primitives.
    pub points: Vec<PcbPoint>,
    /// None marks an unsupported or incomplete geometry, not an empty shape.
    pub geometry: Option<PcbPadPrimitiveGeometry>,
    pub width: Option<f64>,
    pub fill: Option<String>,
    pub source_range: Range<usize>,
}

/// One typed footprint pad in board source order.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbPad {
    pub owner: PcbFootprintMemberOwner,
    /// Compatibility index for board consumers; zero for standalone records.
    /// Prefer `owner` when distinguishing document scope.
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
    /// Drill plating when a drill is authored; `None` for undrilled pads.
    pub plated: Option<bool>,
    /// Whether a direct `(layers ...)` declaration is present, including an
    /// explicitly empty `(layers)`. This is source evidence, not an effective
    /// layer mask: absent declarations do not synthesize entries in `layers`.
    pub has_layers: bool,
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
    pub padstack: Option<PcbPadstack>,
    pub source_range: Range<usize>,
}

pub(super) fn pad_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbPad, Error> {
    let header = bounded_scalar_values(source, &indexed.span, limits.max_pad_header_scalars)?;
    let children = direct_children(source, &indexed.span, limits.max_pad_children, limits)?;
    let (at, size) = pad_placement(source, &children, limits)?;
    let (rect_delta_x, rect_delta_y) = optional_complete_pair(source, &children, "rect_delta")?;
    let kind = required_string(header.get(1), "Expected pad kind", &indexed.span)?;
    let (drill, plated) = pad_drill_and_plating(source, &children, &kind, limits)?;
    let custom = custom_pad_fields(source, &children, limits)?;
    Ok(PcbPad {
        owner: PcbFootprintMemberOwner::EmbeddedFootprint {
            footprint_index: indexed.parent_index,
        },
        footprint_index: indexed.parent_index,
        number: required_string(header.first(), "Expected pad number", &indexed.span)?,
        kind,
        shape: required_string(header.get(2), "Expected pad shape", &indexed.span)?,
        at_x: at[0],
        at_y: at[1],
        angle: at[2],
        size_x: size[0],
        size_y: size[1],
        drill,
        plated,
        has_layers: child(&children, "layers").is_some(),
        layers: child_strings(source, &children, "layers", limits.max_layers)?,
        net: bounded_child_net_ref(source, &children, limits.max_pad_header_scalars)?,
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
        remove_unused_layers: manufacturing::optional_presence_bool(
            source,
            &children,
            "remove_unused_layers",
        )?,
        keep_end_layers: manufacturing::optional_presence_bool(
            source,
            &children,
            "keep_end_layers",
        )?,
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
        custom_options: custom.options,
        custom_primitives: custom.primitives,
        padstack: custom.stack,
        source_range: indexed.span.range.clone(),
    })
}

struct CustomPadFields {
    options: Option<PcbPadCustomOptions>,
    primitives: Vec<PcbPadCustomPrimitive>,
    stack: Option<PcbPadstack>,
}

fn pad_placement(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<([f64; 3], [f64; 2]), Error> {
    Ok((
        optional_vector(
            source,
            children,
            "at",
            [0.0, 0.0, 0.0],
            limits.max_pad_header_scalars,
        )?,
        optional_pair(source, children, "size", [0.0, 0.0])?,
    ))
}

#[derive(Default)]
pub(super) struct CustomPadBudget {
    points: usize,
    primitives: usize,
}

fn custom_pad_fields(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<CustomPadFields, Error> {
    let mut budget = CustomPadBudget::default();
    Ok(CustomPadFields {
        options: custom_options_from_children(source, children, limits)?,
        primitives: custom_primitives_from_children(source, children, limits, &mut budget)?,
        stack: padstacks::padstack(source, children, limits, &mut budget)?,
    })
}

fn pad_drill_and_plating(
    source: &str,
    children: &[FormSpan],
    kind: &str,
    limits: PcbLimits,
) -> Result<(Option<PcbPadDrill>, Option<bool>), Error> {
    let drill = physical::pad_drill_from_children(source, children, limits)?;
    let plated = drill.as_ref().map(|_| kind != "np_thru_hole");
    Ok((drill, plated))
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

pub(super) fn custom_options_from_children(
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

pub(super) fn custom_primitives_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
    budget: &mut CustomPadBudget,
) -> Result<Vec<PcbPadCustomPrimitive>, Error> {
    let Some(primitives) = child(children, "primitives") else {
        return Ok(Vec::new());
    };
    let forms = direct_children(
        source,
        primitives,
        limits
            .max_pad_custom_primitives
            .saturating_sub(budget.primitives),
        limits,
    )?;
    budget.primitives += forms.len();
    forms
        .into_iter()
        .map(|primitive| custom_primitive_from_span(source, primitive, limits, &mut budget.points))
        .collect()
}

fn custom_primitive_from_span(
    source: &str,
    primitive: FormSpan,
    limits: PcbLimits,
    point_count: &mut usize,
) -> Result<PcbPadCustomPrimitive, Error> {
    let fields = direct_children(source, &primitive, limits.max_pad_children, limits)?;
    let (points, geometry) =
        custom::primitive_geometry(source, &primitive, &fields, limits, point_count)?;
    Ok(PcbPadCustomPrimitive {
        kind: primitive.head.clone().unwrap_or_default(),
        points,
        geometry,
        width: custom_primitive_width(source, &fields, limits)?,
        fill: optional_child_string(source, &fields, "fill")?,
        source_range: primitive.range,
    })
}

fn custom_primitive_width(
    source: &str,
    fields: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<f64>, Error> {
    // Custom primitives use KiCad's PCB_SHAPE grammar: both legacy width and
    // modern nested stroke are accepted. Only a width token updates the running
    // stroke; a later style-only block does not reset a previous width.
    let mut width = None;
    for field in fields {
        match field.head.as_deref() {
            Some("width") => {
                width = optional_child_f64(source, std::slice::from_ref(field), "width")?;
            }
            Some("stroke") => {
                for child in direct_children(source, field, limits.max_object_children, limits)? {
                    if child.head.as_deref() == Some("width") {
                        width = optional_child_f64(source, std::slice::from_ref(&child), "width")?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(width)
}

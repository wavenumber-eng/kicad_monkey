//! Detailed board-via inputs shared by native readers and later plotters.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PcbFrontBackOptionalBool {
    pub front: Option<bool>,
    pub back: Option<bool>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbVia {
    pub at_x: f64,
    pub at_y: f64,
    pub size: f64,
    pub drill: f64,
    pub layers: Vec<String>,
    pub free: bool,
    pub tenting: Option<PcbFrontBackOptionalBool>,
    pub covering: Option<PcbFrontBackOptionalBool>,
    pub plugging: Option<PcbFrontBackOptionalBool>,
    pub capping: Option<bool>,
    pub filling: Option<bool>,
    pub net: PcbNetRef,
    pub uuid: Option<String>,
    pub via_type: Option<String>,
    pub backdrill: Option<PcbDrillProperties>,
    pub tertiary_drill: Option<PcbDrillProperties>,
    pub front_post_machining: Option<PcbPostMachiningProperties>,
    pub back_post_machining: Option<PcbPostMachiningProperties>,
    pub zone_layer_connections: Option<PcbZoneLayerConnections>,
    pub remove_unused_layers: Option<bool>,
    pub keep_end_layers: Option<bool>,
    pub start_end_only: Option<bool>,
    pub source_range: Range<usize>,
}

pub(super) fn via_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbVia, Error> {
    let header = bounded_scalar_values(source, span, limits.max_via_header_scalars)?;
    let children = direct_children(source, span, limits.max_via_children, limits)?;
    let at = optional_coordinate_pair(source, &children, "at")?;
    Ok(PcbVia {
        at_x: at[0],
        at_y: at[1],
        size: optional_child_f64(source, &children, "size")?.unwrap_or_default(),
        drill: optional_child_f64(source, &children, "drill")?.unwrap_or_default(),
        layers: child_strings(source, &children, "layers", limits.max_layers)?,
        free: optional_child_string(source, &children, "free")?
            .is_some_and(|value| value.eq_ignore_ascii_case("yes")),
        tenting: front_back_optional_bool(source, &children, "tenting", limits)?,
        covering: front_back_optional_bool(source, &children, "covering", limits)?,
        plugging: front_back_optional_bool(source, &children, "plugging", limits)?,
        capping: optional_yes_no(source, &children, "capping")?,
        filling: optional_yes_no(source, &children, "filling")?,
        net: child_net_ref_or_zero(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        via_type: ["blind", "buried", "micro"]
            .into_iter()
            .find(|kind| header.iter().any(|token| token.lexeme == *kind))
            .map(str::to_owned),
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
        start_end_only: manufacturing::optional_presence_bool(source, &children, "start_end_only")?,
        source_range: span.range.clone(),
    })
}

fn front_back_optional_bool(
    source: &str,
    children: &[FormSpan],
    head: &str,
    limits: PcbLimits,
) -> Result<Option<PcbFrontBackOptionalBool>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_via_policy_children, limits)?;
    let front = optional_yes_no(source, &fields, "front")?;
    let back = optional_yes_no(source, &fields, "back")?;
    if front.is_none() && back.is_none() {
        return Ok(None);
    }
    Ok(Some(PcbFrontBackOptionalBool {
        front,
        back,
        source_range: span.range.clone(),
    }))
}

fn optional_yes_no(source: &str, children: &[FormSpan], head: &str) -> Result<Option<bool>, Error> {
    Ok(
        optional_child_string(source, children, head)?.and_then(|value| {
            match value.to_ascii_lowercase().as_str() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            }
        }),
    )
}

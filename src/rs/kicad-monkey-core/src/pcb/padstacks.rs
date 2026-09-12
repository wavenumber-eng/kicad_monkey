//! Authored per-layer pad/via source records; no effective geometry resolution.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PcbPadstack {
    pub mode: Option<String>,
    pub layers: Vec<PcbPadstackLayer>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbPadstackLayer {
    /// `Inner` is the source front/inner/back selector, not a board layer name.
    pub layer: String,
    pub shape: Option<String>,
    pub size: Option<PcbPoint>,
    pub offset: Option<PcbPoint>,
    pub rect_delta: Option<PcbPoint>,
    pub roundrect_rratio: Option<f64>,
    pub chamfer_ratio: Option<f64>,
    pub chamfer_corners: Vec<String>,
    pub clearance: Option<f64>,
    pub thermal_bridge_width: Option<f64>,
    pub thermal_bridge_angle: Option<f64>,
    pub thermal_gap: Option<f64>,
    /// Explicit -1 (inherited) remains distinct from an absent declaration.
    pub zone_connect: Option<i64>,
    pub custom_options: Option<PcbPadCustomOptions>,
    pub custom_primitives: Vec<PcbPadCustomPrimitive>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbViaStack {
    pub mode: Option<String>,
    pub layers: Vec<PcbViaStackLayer>,
    pub source_range: Range<usize>,
}

/// KiCad via-stack rows support only a circular diameter, not pad shapes/policies.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbViaStackLayer {
    pub layer: String,
    pub size: Option<f64>,
    pub source_range: Range<usize>,
}

pub(super) fn padstack(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
    budget: &mut pads::CustomPadBudget,
) -> Result<Option<PcbPadstack>, Error> {
    let Some(span) = child(children, "padstack") else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_pad_children, limits)?;
    let mut layers = Vec::new();
    for layer in fields
        .iter()
        .filter(|field| field.head.as_deref() == Some("layer"))
    {
        if layers.len() >= limits.max_layers {
            return Err(limit_error());
        }
        let values = direct_children(source, layer, limits.max_pad_children, limits)?;
        layers.push(PcbPadstackLayer {
            layer: first_string(source, layer)?.unwrap_or_default(),
            shape: optional_child_string(source, &values, "shape")?,
            size: optional_child_point(source, &values, "size")?,
            offset: optional_child_point(source, &values, "offset")?,
            rect_delta: optional_child_point(source, &values, "rect_delta")?,
            roundrect_rratio: optional_child_f64(source, &values, "roundrect_rratio")?,
            chamfer_ratio: optional_child_f64(source, &values, "chamfer_ratio")?,
            chamfer_corners: child_strings(
                source,
                &values,
                "chamfer",
                limits.max_pad_chamfer_corners,
            )?,
            clearance: optional_child_f64(source, &values, "clearance")?,
            thermal_bridge_width: optional_child_f64(source, &values, "thermal_bridge_width")?,
            thermal_bridge_angle: optional_child_f64(source, &values, "thermal_bridge_angle")?,
            thermal_gap: optional_child_f64(source, &values, "thermal_gap")?,
            zone_connect: optional_child_i64(source, &values, "zone_connect")?,
            custom_options: pads::custom_options_from_children(source, &values, limits)?,
            custom_primitives: pads::custom_primitives_from_children(
                source, &values, limits, budget,
            )?,
            source_range: layer.range.clone(),
        });
    }
    Ok(Some(PcbPadstack {
        mode: optional_child_string(source, &fields, "mode")?,
        layers,
        source_range: span.range.clone(),
    }))
}

pub(super) fn viastack(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbViaStack>, Error> {
    let Some(span) = child(children, "padstack") else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_via_children, limits)?;
    let mut layers = Vec::new();
    for layer in fields
        .iter()
        .filter(|field| field.head.as_deref() == Some("layer"))
    {
        if layers.len() >= limits.max_layers {
            return Err(limit_error());
        }
        let values = direct_children(source, layer, limits.max_via_children, limits)?;
        layers.push(PcbViaStackLayer {
            layer: first_string(source, layer)?.unwrap_or_default(),
            size: optional_child_f64(source, &values, "size")?,
            source_range: layer.range.clone(),
        });
    }
    Ok(Some(PcbViaStack {
        mode: optional_child_string(source, &fields, "mode")?,
        layers,
        source_range: span.range.clone(),
    }))
}

//! Source-backed board setup and physical stackup records.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PcbSetup {
    /// Direct declaration presence, independent of the effective origin value.
    pub has_aux_axis_origin: bool,
    pub aux_axis_origin: PcbPoint,
    /// Distinguishes an authored zero origin from the absent-origin default.
    pub has_grid_origin: bool,
    pub grid_origin: PcbPoint,
    pub allow_soldermask_bridges_in_footprints: bool,
    pub tenting_front: bool,
    pub tenting_back: bool,
    /// Whether `(tenting ...)` is declared. Without it, KiCad uses the
    /// legacy `viasonmask` plot option, and otherwise tents both sides.
    pub has_tenting: bool,
    /// Legacy `(pcbplotparams (viasonmask ...))`, when declared.
    pub legacy_vias_on_mask: Option<bool>,
    /// Board solder-mask expansion (`pad_to_mask_clearance`), mm.
    pub pad_to_mask_clearance: f64,
    /// Minimum solder-mask web width (`solder_mask_min_width`), mm.
    pub solder_mask_min_width: f64,
    /// Board solder-paste margin (`pad_to_paste_clearance`), mm.
    pub pad_to_paste_clearance: f64,
    /// Board solder-paste margin ratio (`pad_to_paste_clearance_ratio`).
    pub pad_to_paste_clearance_ratio: f64,
    pub covering_front: bool,
    pub covering_back: bool,
    pub plugging_front: bool,
    pub plugging_back: bool,
    pub capping: bool,
    pub filling: bool,
    pub stackup: Option<PcbStackup>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbStackup {
    pub layers: Vec<PcbStackupLayer>,
    pub copper_finish: String,
    pub dielectric_constraints: bool,
    pub edge_connector: String,
    pub edge_plating: bool,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbStackupLayer {
    pub name: String,
    pub type_name: String,
    pub thickness: f64,
    pub thickness_locked: bool,
    pub material: String,
    pub epsilon_r: Option<f64>,
    pub loss_tangent: Option<f64>,
    pub color: String,
    /// Dielectric sub-layers after the first (`addsublayer`), in order. The
    /// first sub-layer is described by the fields above.
    pub sublayers: Vec<PcbStackupSublayer>,
    pub source_range: Range<usize>,
}

/// One further dielectric sub-layer of a stackup layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbStackupSublayer {
    pub thickness: f64,
    pub thickness_locked: bool,
    pub material: String,
    pub epsilon_r: Option<f64>,
    pub loss_tangent: Option<f64>,
    pub color: String,
}

impl<'a> PcbView<'a> {
    pub fn setup(&self) -> Result<Option<PcbSetup>, Error> {
        if !self.selection.contains(PcbFamily::Setup) {
            return Ok(None);
        }
        self.first_top_level("setup")
            .map(|span| setup_from_span(self.source, span, self.limits))
            .transpose()
    }
}

fn setup_from_span(source: &str, span: &FormSpan, limits: PcbLimits) -> Result<PcbSetup, Error> {
    let children = direct_children(source, span, limits.max_setup_children, limits)?;
    let aux = optional_pair(source, &children, "aux_axis_origin", [0.0, 0.0])?;
    let grid = optional_pair(source, &children, "grid_origin", [0.0, 0.0])?;
    Ok(PcbSetup {
        has_aux_axis_origin: child(&children, "aux_axis_origin").is_some(),
        aux_axis_origin: PcbPoint {
            x: aux[0],
            y: aux[1],
        },
        has_grid_origin: child(&children, "grid_origin").is_some(),
        grid_origin: PcbPoint {
            x: grid[0],
            y: grid[1],
        },
        allow_soldermask_bridges_in_footprints: child_bool(
            source,
            &children,
            "allow_soldermask_bridges_in_footprints",
        )?,
        tenting_front: side_enabled(source, &children, "tenting", "front", limits)?,
        tenting_back: side_enabled(source, &children, "tenting", "back", limits)?,
        has_tenting: child(&children, "tenting").is_some(),
        legacy_vias_on_mask: legacy_vias_on_mask(source, &children, limits)?,
        pad_to_mask_clearance: optional_child_f64(source, &children, "pad_to_mask_clearance")?
            .unwrap_or(0.0),
        solder_mask_min_width: optional_child_f64(source, &children, "solder_mask_min_width")?
            .unwrap_or(0.0),
        pad_to_paste_clearance: optional_child_f64(source, &children, "pad_to_paste_clearance")?
            .unwrap_or(0.0),
        pad_to_paste_clearance_ratio: optional_child_f64(
            source,
            &children,
            "pad_to_paste_clearance_ratio",
        )?
        .unwrap_or(0.0),
        covering_front: side_enabled(source, &children, "covering", "front", limits)?,
        covering_back: side_enabled(source, &children, "covering", "back", limits)?,
        plugging_front: side_enabled(source, &children, "plugging", "front", limits)?,
        plugging_back: side_enabled(source, &children, "plugging", "back", limits)?,
        capping: child_bool(source, &children, "capping")?,
        filling: child_bool(source, &children, "filling")?,
        stackup: child(&children, "stackup")
            .map(|stackup| stackup_from_span(source, stackup, limits))
            .transpose()?,
        source_range: span.range.clone(),
    })
}

fn legacy_vias_on_mask(
    source: &str,
    setup_children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<bool>, Error> {
    let Some(span) = child(setup_children, "pcbplotparams") else {
        return Ok(None);
    };
    let fields = direct_children(source, span, limits.max_setup_children, limits)?;
    if child(&fields, "viasonmask").is_none() {
        return Ok(None);
    }
    child_bool(source, &fields, "viasonmask").map(Some)
}

fn side_enabled(
    source: &str,
    setup_children: &[FormSpan],
    head: &str,
    side: &str,
    limits: PcbLimits,
) -> Result<bool, Error> {
    let Some(span) = child(setup_children, head) else {
        return Ok(false);
    };
    let flags = bounded_scalar_values(source, span, 8)?;
    if has_flag(&flags, side) {
        return Ok(true);
    }
    let fields = direct_children(source, span, 8, limits)?;
    child_bool(source, &fields, side)
}

fn stackup_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbStackup, Error> {
    let children = direct_children(source, span, limits.max_setup_children, limits)?;
    let mut layers = Vec::new();
    for layer in children
        .iter()
        .filter(|child| child.head.as_deref() == Some("layer"))
    {
        if layers.len() == limits.max_stackup_layers {
            return Err(limit_error());
        }
        layers.push(stackup_layer_from_span(source, layer, limits)?);
    }
    Ok(PcbStackup {
        layers,
        copper_finish: optional_child_string(source, &children, "copper_finish")?
            .unwrap_or_default(),
        dielectric_constraints: child_bool(source, &children, "dielectric_constraints")?,
        edge_connector: optional_child_string(source, &children, "edge_connector")?
            .unwrap_or_else(|| "none".to_owned()),
        edge_plating: child_bool(source, &children, "edge_plating")?,
        source_range: span.range.clone(),
    })
}

fn stackup_layer_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbStackupLayer, Error> {
    let name = first_string(source, span)?.unwrap_or_default();
    let children = direct_children(source, span, 32 * MAX_SUBLAYERS, limits)?;
    // KiCad starts a new sub-layer at each bare `addsublayer` token.
    let breaks: Vec<usize> = bounded_scalar_values(source, span, MAX_SUBLAYERS + 8)?
        .iter()
        .filter(|token| token.lexeme == "addsublayer")
        .map(|token| span.range.start + token.position.offset)
        .collect();
    let mut parts = (0..=breaks.len())
        .map(|part| {
            let group: Vec<FormSpan> = children
                .iter()
                .filter(|child| breaks.partition_point(|at| *at < child.range.start) == part)
                .cloned()
                .collect();
            sublayer_from_children(source, span, &group)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let first = parts.remove(0);
    Ok(PcbStackupLayer {
        name,
        type_name: optional_child_string(source, &children, "type")?.unwrap_or_default(),
        thickness: first.thickness,
        thickness_locked: first.thickness_locked,
        material: first.material,
        epsilon_r: first.epsilon_r,
        loss_tangent: first.loss_tangent,
        color: first.color,
        sublayers: parts,
        source_range: span.range.clone(),
    })
}

/// Most sub-layers one stackup layer may declare.
const MAX_SUBLAYERS: usize = 64;

fn sublayer_from_children(
    source: &str,
    span: &FormSpan,
    children: &[FormSpan],
) -> Result<PcbStackupSublayer, Error> {
    let thickness = child(children, "thickness");
    let thickness_flags = thickness
        .map(|value| bounded_scalar_values(source, value, 8))
        .transpose()?
        .unwrap_or_default();
    Ok(PcbStackupSublayer {
        thickness: thickness_flags
            .first()
            .map(|token| parse_f64(token, span))
            .transpose()?
            .unwrap_or(0.0),
        thickness_locked: has_flag(&thickness_flags, "locked"),
        material: optional_child_string(source, children, "material")?.unwrap_or_default(),
        epsilon_r: optional_child_f64(source, children, "epsilon_r")?,
        loss_tangent: optional_child_f64(source, children, "loss_tangent")?,
        color: optional_child_string(source, children, "color")?.unwrap_or_default(),
    })
}

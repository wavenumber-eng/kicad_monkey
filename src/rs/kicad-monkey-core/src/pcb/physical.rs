//! Iterable physical facts derived directly from PCB source records.
//!
//! Footprint-local coordinates stay local and are paired with an explicit
//! placement transform. Consumers therefore choose when and how to compose
//! transforms without losing KiCad source semantics.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PcbPadDrill {
    pub shape: PcbHoleShape,
    pub width: f64,
    pub height: Option<f64>,
    pub offset: PcbPoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcbHoleShape {
    Round,
    Oval,
}

impl PcbHoleShape {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Round => "round",
            Self::Oval => "oval",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcbHoleOwner {
    Pad,
    Via,
}

impl PcbHoleOwner {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pad => "pad",
            Self::Via => "via",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbHole {
    pub owner: PcbHoleOwner,
    pub owner_index: usize,
    pub footprint_index: Option<usize>,
    /// Board coordinates for vias; footprint-local coordinates for pads.
    pub center: PcbPoint,
    /// Drill offset in the pad's local coordinate system.
    pub offset: PcbPoint,
    pub shape: PcbHoleShape,
    pub width: f64,
    pub height: f64,
    pub angle: f64,
    pub plated: bool,
    pub layers: Vec<String>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbFootprintTransform {
    pub footprint_index: usize,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub layer: String,
    pub locked: bool,
    pub path: Option<String>,
    pub sheet_name: Option<String>,
    pub sheet_file: Option<String>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcbProfileOwner {
    Board,
    Footprint { footprint_index: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbProfilePrimitive {
    pub owner: PcbProfileOwner,
    pub graphic: PcbGraphic,
}

impl<'a> PcbView<'a> {
    /// Iterate source-authored holes: footprint-local pad drills, then board vias.
    pub fn holes(&self) -> impl Iterator<Item = Result<PcbHole, Error>> + '_ {
        let pads =
            self.pads.iter().enumerate().filter_map(|(index, span)| {
                match pad_hole_from_span(self.source, span, index, self.limits) {
                    Ok(Some(hole)) => Some(Ok(hole)),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            });
        let vias =
            self.vias.iter().enumerate().filter_map(|(index, span)| {
                match via_hole_from_span(self.source, span, index, self.limits) {
                    Ok(Some(hole)) => Some(Ok(hole)),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            });
        pads.chain(vias)
    }

    /// Iterate explicit footprint-local to board placement facts in source order.
    pub fn footprint_transforms(
        &self,
    ) -> impl Iterator<Item = Result<PcbFootprintTransform, Error>> + '_ {
        self.footprints.iter().enumerate().map(|(index, span)| {
            footprint_transform_from_span(self.source, span, index, self.limits)
        })
    }

    /// Iterate top-level and footprint-local Edge.Cuts carriers.
    pub fn profile_primitives(
        &self,
    ) -> impl Iterator<Item = Result<PcbProfilePrimitive, Error>> + '_ {
        let board = self.graphics.iter().filter_map(|span| {
            match profile_from_span(self.source, span, PcbProfileOwner::Board, self.limits) {
                Ok(Some(profile)) => Some(Ok(profile)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }
        });
        let footprints = self.footprint_graphics.iter().filter_map(|indexed| {
            let owner = PcbProfileOwner::Footprint {
                footprint_index: indexed.parent_index,
            };
            match profile_from_span(self.source, &indexed.span, owner, self.limits) {
                Ok(Some(profile)) => Some(Ok(profile)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }
        });
        board.chain(footprints)
    }
}

pub(super) fn is_footprint_profile_head(head: &str) -> bool {
    matches!(
        head,
        "fp_line" | "fp_arc" | "fp_circle" | "fp_rect" | "fp_poly"
    )
}

pub(super) fn pad_drill_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbPadDrill>, Error> {
    let Some(drill) = child(children, "drill") else {
        return Ok(None);
    };
    let values = bounded_scalar_values(source, drill, 3)?;
    let oval = values
        .first()
        .is_some_and(|value| token_string(value) == "oval");
    let width_index = usize::from(oval);
    let Some(width) = optional_f64(values.get(width_index), drill)? else {
        return Ok(None);
    };
    let height = oval
        .then(|| optional_f64(values.get(width_index + 1), drill))
        .transpose()?
        .flatten();
    let drill_children = direct_children(source, drill, 4, limits)?;
    let offset = optional_pair(source, &drill_children, "offset", [0.0, 0.0])?;
    Ok(Some(PcbPadDrill {
        shape: if oval {
            PcbHoleShape::Oval
        } else {
            PcbHoleShape::Round
        },
        width,
        height,
        offset: PcbPoint {
            x: offset[0],
            y: offset[1],
        },
    }))
}

fn pad_hole_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    owner_index: usize,
    limits: PcbLimits,
) -> Result<Option<PcbHole>, Error> {
    let children = direct_children(source, &indexed.span, limits.max_pad_children, limits)?;
    let Some(drill) = pad_drill_from_children(source, &children, limits)? else {
        return Ok(None);
    };
    if drill.width <= 0.0 || drill.height.is_some_and(|height| height <= 0.0) {
        return Ok(None);
    }
    let at = optional_vector(source, &children, "at", [0.0, 0.0, 0.0])?;
    let header = bounded_scalar_values(source, &indexed.span, 3)?;
    let kind = header.get(1).map(token_string).unwrap_or_default();
    Ok(Some(PcbHole {
        owner: PcbHoleOwner::Pad,
        owner_index,
        footprint_index: Some(indexed.parent_index),
        center: PcbPoint { x: at[0], y: at[1] },
        offset: drill.offset,
        shape: drill.shape,
        width: drill.width,
        height: drill.height.unwrap_or(drill.width),
        angle: at[2],
        plated: kind != "np_thru_hole",
        layers: child_strings(source, &children, "layers", limits.max_layers)?,
        uuid: optional_uuid(source, &children)?,
        source_range: indexed.span.range.clone(),
    }))
}

fn via_hole_from_span(
    source: &str,
    span: &FormSpan,
    owner_index: usize,
    limits: PcbLimits,
) -> Result<Option<PcbHole>, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let Some(drill) = optional_child_f64(source, &children, "drill")?.filter(|drill| *drill > 0.0)
    else {
        return Ok(None);
    };
    let at = required_point(source, &children, "at", span)?;
    Ok(Some(PcbHole {
        owner: PcbHoleOwner::Via,
        owner_index,
        footprint_index: None,
        center: at,
        offset: PcbPoint { x: 0.0, y: 0.0 },
        shape: PcbHoleShape::Round,
        width: drill,
        height: drill,
        angle: 0.0,
        plated: true,
        layers: child_strings(source, &children, "layers", limits.max_layers)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    }))
}

fn footprint_transform_from_span(
    source: &str,
    indexed: &IndexedFootprint,
    footprint_index: usize,
    limits: PcbLimits,
) -> Result<PcbFootprintTransform, Error> {
    let children = direct_children(source, &indexed.span, limits.max_footprint_children, limits)?;
    let at = optional_vector(source, &children, "at", [0.0, 0.0, 0.0])?;
    Ok(PcbFootprintTransform {
        footprint_index,
        x: at[0],
        y: at[1],
        angle: at[2],
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.Cu".to_owned()),
        locked: child_bool(source, &children, "locked")?,
        path: optional_child_string(source, &children, "path")?.filter(|value| !value.is_empty()),
        sheet_name: optional_child_string(source, &children, "sheetname")?
            .filter(|value| !value.is_empty()),
        sheet_file: optional_child_string(source, &children, "sheetfile")?
            .filter(|value| !value.is_empty()),
        uuid: optional_uuid(source, &children)?,
        source_range: indexed.span.range.clone(),
    })
}

fn profile_from_span(
    source: &str,
    span: &FormSpan,
    owner: PcbProfileOwner,
    limits: PcbLimits,
) -> Result<Option<PcbProfilePrimitive>, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    if optional_child_string(source, &children, "layer")?.as_deref() != Some("Edge.Cuts") {
        return Ok(None);
    }
    Ok(Some(PcbProfilePrimitive {
        owner,
        graphic: graphic_from_span(source, span, limits)?,
    }))
}

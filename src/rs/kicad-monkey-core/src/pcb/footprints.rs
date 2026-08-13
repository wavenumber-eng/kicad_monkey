//! Embedded-footprint child views shared by PCB readers and later plotters.

use super::*;

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

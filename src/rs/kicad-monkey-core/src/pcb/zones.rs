//! Source-backed PCB zone records.
//!
//! KiCad stores both authored outlines and cached filled polygons in a zone.
//! This module exposes those facts without running a polygon kernel or
//! constructing the generic compatibility tree.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbZoneKeepout {
    pub tracks: String,
    pub vias: String,
    pub pads: String,
    pub copperpour: String,
    pub footprints: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcbZonePlacementSource {
    SheetName,
    ComponentClass,
    Group,
}

impl PcbZonePlacementSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SheetName => "sheetname",
            Self::ComponentClass => "component_class",
            Self::Group => "group",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbZonePlacement {
    pub enabled: bool,
    pub source_type: PcbZonePlacementSource,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbZoneLayerProperty {
    pub layer: String,
    pub hatch_offset: PcbPoint,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbZonePolygon {
    pub points: Vec<PcbPoint>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbZoneFilledPolygon {
    pub layer: String,
    pub island: bool,
    pub points: Vec<PcbPoint>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbZone {
    pub net: PcbNetRef,
    /// Exact optional `(net_name ...)` source value, before board-net resolution.
    pub net_name: Option<String>,
    pub has_explicit_net_name: bool,
    pub layers: Vec<String>,
    pub layers_plural: bool,
    pub locked: bool,
    pub uuid: Option<String>,
    pub name: Option<String>,
    pub hatch_style: String,
    pub hatch_pitch: f64,
    pub priority: i64,
    pub connect_pads_clearance: f64,
    pub min_thickness: f64,
    pub filled_areas_thickness: bool,
    pub fill_enabled: bool,
    pub thermal_gap: f64,
    pub thermal_bridge_width: f64,
    pub island_removal_mode: Option<i64>,
    pub island_area_min: Option<f64>,
    pub keepout: Option<PcbZoneKeepout>,
    pub placement: Option<PcbZonePlacement>,
    pub layer_properties: Vec<PcbZoneLayerProperty>,
    pub polygons: Vec<PcbZonePolygon>,
    pub filled_polygons: Vec<PcbZoneFilledPolygon>,
    pub source_range: Range<usize>,
}

struct ZoneCollections {
    layer_properties: Vec<PcbZoneLayerProperty>,
    polygons: Vec<PcbZonePolygon>,
    filled_polygons: Vec<PcbZoneFilledPolygon>,
}

pub(super) fn zone_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbZone, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let plural_layer_values = child(&children, "layers")
        .map(|item| bounded_scalar_values(source, item, limits.max_layers))
        .transpose()?;
    let layers_plural = plural_layer_values
        .as_ref()
        .is_some_and(|values| !values.is_empty());
    let layers = if let Some(values) = plural_layer_values.filter(|values| !values.is_empty()) {
        values.iter().map(token_string).collect()
    } else if let Some(layer) = optional_child_string(source, &children, "layer")? {
        vec![layer]
    } else {
        Vec::new()
    };

    let net_name_span = child(&children, "net_name");
    let net_name = net_name_span
        .map(|item| first_string(source, item))
        .transpose()?
        .flatten()
        .or_else(|| net_name_span.map(|_| String::new()));
    let mut net = child_net_ref_or_zero(source, &children)?;
    if let Some(explicit_name) = &net_name {
        net.name = Some(explicit_name.clone());
    }

    let hatch = child(&children, "hatch");
    let hatch_values = hatch
        .map(|item| bounded_scalar_values(source, item, 2))
        .transpose()?
        .unwrap_or_default();
    let connect_pads_clearance = nested_f64(
        source,
        child(&children, "connect_pads"),
        "clearance",
        0.5,
        limits,
    )?;
    let fill = child(&children, "fill");
    let fill_enabled = fill
        .map(|item| bounded_scalar_values(source, item, 1))
        .transpose()?
        .and_then(|values| values.first().map(token_string))
        .is_some_and(|value| value == "yes");
    let fill_children = fill
        .map(|item| direct_children(source, item, limits.max_object_children, limits))
        .transpose()?
        .unwrap_or_default();

    let collections = zone_collections(source, &children, limits)?;

    Ok(PcbZone {
        net,
        net_name,
        has_explicit_net_name: net_name_span.is_some(),
        layers,
        layers_plural,
        locked: child_bool(source, &children, "locked")?,
        uuid: optional_uuid(source, &children)?,
        name: optional_child_string(source, &children, "name")?.filter(|name| !name.is_empty()),
        hatch_style: hatch_values
            .first()
            .map(token_string)
            .unwrap_or_else(|| "edge".to_owned()),
        hatch_pitch: optional_f64(hatch_values.get(1), hatch.unwrap_or(span))?.unwrap_or(0.5),
        priority: optional_child_i64(source, &children, "priority")?.unwrap_or(0),
        connect_pads_clearance,
        min_thickness: optional_child_f64(source, &children, "min_thickness")?.unwrap_or(0.25),
        filled_areas_thickness: optional_child_string(source, &children, "filled_areas_thickness")?
            .is_some_and(|value| value == "yes"),
        fill_enabled,
        thermal_gap: optional_child_f64(source, &fill_children, "thermal_gap")?.unwrap_or(0.5),
        thermal_bridge_width: optional_child_f64(source, &fill_children, "thermal_bridge_width")?
            .unwrap_or(0.5),
        island_removal_mode: optional_child_i64(source, &fill_children, "island_removal_mode")?,
        island_area_min: optional_child_f64(source, &fill_children, "island_area_min")?,
        keepout: keepout_from_children(source, &children, limits)?,
        placement: placement_from_children(source, &children, limits)?,
        layer_properties: collections.layer_properties,
        polygons: collections.polygons,
        filled_polygons: collections.filled_polygons,
        source_range: span.range.clone(),
    })
}

fn zone_collections(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<ZoneCollections, Error> {
    let mut result = ZoneCollections {
        layer_properties: Vec::new(),
        polygons: Vec::new(),
        filled_polygons: Vec::new(),
    };
    let mut point_count = 0usize;
    for item in children {
        match item.head.as_deref() {
            Some("property") => {
                if let Some(property) = layer_property_from_span(source, item, limits)? {
                    bounded_push(
                        &mut result.layer_properties,
                        property,
                        limits.max_zone_layer_properties,
                    )?;
                }
            }
            Some("polygon") => {
                ensure_polygon_capacity(&result, limits)?;
                result.polygons.push(PcbZonePolygon {
                    points: polygon_points(source, item, limits, &mut point_count)?,
                    source_range: item.range.clone(),
                });
            }
            Some("filled_polygon") => {
                ensure_polygon_capacity(&result, limits)?;
                let item_children =
                    direct_children(source, item, limits.max_object_children, limits)?;
                result.filled_polygons.push(PcbZoneFilledPolygon {
                    layer: optional_child_string(source, &item_children, "layer")?
                        .unwrap_or_default(),
                    island: child(&item_children, "island").is_some(),
                    points: polygon_points_from_children(
                        source,
                        &item_children,
                        limits,
                        &mut point_count,
                    )?,
                    source_range: item.range.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(result)
}

fn ensure_polygon_capacity(result: &ZoneCollections, limits: PcbLimits) -> Result<(), Error> {
    if result
        .polygons
        .len()
        .saturating_add(result.filled_polygons.len())
        >= limits.max_zone_polygons
    {
        Err(limit_error())
    } else {
        Ok(())
    }
}

fn nested_f64(
    source: &str,
    parent: Option<&FormSpan>,
    head: &str,
    default: f64,
    limits: PcbLimits,
) -> Result<f64, Error> {
    let Some(parent) = parent else {
        return Ok(default);
    };
    let children = direct_children(source, parent, limits.max_object_children, limits)?;
    Ok(optional_child_f64(source, &children, head)?.unwrap_or(default))
}

fn keepout_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbZoneKeepout>, Error> {
    let Some(keepout) = child(children, "keepout") else {
        return Ok(None);
    };
    let values = direct_children(source, keepout, limits.max_object_children, limits)?;
    let setting = |head| {
        optional_child_string(source, &values, head)
            .map(|value| value.unwrap_or_else(|| "not_allowed".to_owned()))
    };
    Ok(Some(PcbZoneKeepout {
        tracks: setting("tracks")?,
        vias: setting("vias")?,
        pads: setting("pads")?,
        copperpour: setting("copperpour")?,
        footprints: setting("footprints")?,
    }))
}

fn placement_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<Option<PcbZonePlacement>, Error> {
    let Some(placement) = child(children, "placement") else {
        return Ok(None);
    };
    let values = direct_children(source, placement, limits.max_object_children, limits)?;
    let (source_type, source_value) = if let Some(value) = child(&values, "sheetname") {
        (
            PcbZonePlacementSource::SheetName,
            first_string(source, value)?.unwrap_or_default(),
        )
    } else if let Some(value) = child(&values, "component_class") {
        (
            PcbZonePlacementSource::ComponentClass,
            first_string(source, value)?.unwrap_or_default(),
        )
    } else if let Some(value) = child(&values, "group") {
        (
            PcbZonePlacementSource::Group,
            first_string(source, value)?.unwrap_or_default(),
        )
    } else {
        (PcbZonePlacementSource::SheetName, String::new())
    };
    Ok(Some(PcbZonePlacement {
        enabled: optional_child_string(source, &values, "enabled")?
            .is_some_and(|value| value == "yes"),
        source_type,
        source: source_value,
    }))
}

fn layer_property_from_span(
    source: &str,
    property: &FormSpan,
    limits: PcbLimits,
) -> Result<Option<PcbZoneLayerProperty>, Error> {
    let children = direct_children(source, property, limits.max_object_children, limits)?;
    let Some(hatch) = child(&children, "hatch_position") else {
        return Ok(None);
    };
    let hatch_children = direct_children(source, hatch, limits.max_object_children, limits)?;
    let Some(xy) = child(&hatch_children, "xy") else {
        return Ok(None);
    };
    let values = bounded_scalar_values(source, xy, 2)?;
    if values.len() < 2 {
        return Ok(None);
    }
    Ok(Some(PcbZoneLayerProperty {
        layer: optional_child_string(source, &children, "layer")?.unwrap_or_default(),
        hatch_offset: PcbPoint {
            x: required_f64(values.first(), "Expected zone hatch x", xy)?,
            y: required_f64(values.get(1), "Expected zone hatch y", xy)?,
        },
        source_range: property.range.clone(),
    }))
}

fn polygon_points(
    source: &str,
    polygon: &FormSpan,
    limits: PcbLimits,
    total: &mut usize,
) -> Result<Vec<PcbPoint>, Error> {
    let children = direct_children(source, polygon, limits.max_object_children, limits)?;
    polygon_points_from_children(source, &children, limits, total)
}

fn polygon_points_from_children(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
    total: &mut usize,
) -> Result<Vec<PcbPoint>, Error> {
    let Some(points) = child(children, "pts") else {
        return Ok(Vec::new());
    };
    let remaining = limits.max_zone_points.saturating_sub(*total);
    let point_spans = direct_children(source, points, remaining, limits)?;
    let mut decoded = Vec::with_capacity(point_spans.len());
    for point in point_spans {
        if point.head.as_deref() != Some("xy") {
            continue;
        }
        let values = bounded_scalar_values(source, &point, 2)?;
        decoded.push(PcbPoint {
            x: required_f64(values.first(), "Expected zone point x", &point)?,
            y: required_f64(values.get(1), "Expected zone point y", &point)?,
        });
    }
    *total += decoded.len();
    Ok(decoded)
}

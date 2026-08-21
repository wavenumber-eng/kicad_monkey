use super::{KiCadDesignJsonError, KiCadDesignPcb};
use crate::KiCadNetlist;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn pnp_json(
    pcb: KiCadDesignPcb<'_>,
    netlist: &KiCadNetlist,
    netlist_json: &Value,
) -> Result<Option<Value>, KiCadDesignJsonError> {
    let components = netlist
        .components
        .iter()
        .enumerate()
        .map(|(index, component)| (component.reference.as_str(), (index, component)))
        .collect::<BTreeMap<_, _>>();
    let raw_components = netlist_json["components"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let mut footprint_parameters = BTreeMap::<usize, BTreeMap<String, String>>::new();
    for property in pcb.view.footprint_properties() {
        let property = property.map_err(|error| {
            KiCadDesignJsonError::context("could not read PCB footprint property", error)
        })?;
        if !property.name.is_empty() {
            footprint_parameters
                .entry(property.footprint_index)
                .or_default()
                .insert(property.name, property.value);
        }
    }
    let mut placements = Vec::new();
    for (footprint_index, footprint) in pcb.view.footprints().enumerate() {
        let footprint = footprint.map_err(|error| {
            KiCadDesignJsonError::context("could not read PCB footprint", error)
        })?;
        let reference = footprint.reference.as_deref().unwrap_or("");
        if reference.is_empty() {
            continue;
        }
        let component = components.get(reference);
        let raw = component
            .and_then(|(index, _)| raw_components.get(*index))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let value = component.map_or_else(
            || footprint.value.clone().unwrap_or_default(),
            |(_, component)| component.value.clone(),
        );
        let description = component.map_or_else(
            || footprint.description.clone(),
            |(_, component)| component.libsource_description.clone(),
        );
        let parameters = component
            .and_then(|_| raw.get("parameters").cloned())
            .unwrap_or_else(|| {
                json!(
                    footprint_parameters
                        .remove(&footprint_index)
                        .unwrap_or_default()
                )
            });
        placements.push(json!({
            "designator": reference,
            "comment": value,
            "layer": pnp_layer(footprint.layer.as_deref().unwrap_or("")),
            "footprint": footprint.library_link,
            "center_x": round4(footprint.at_x.unwrap_or(0.0)),
            "center_y": round4(footprint.at_y.unwrap_or(0.0)),
            "rotation": round4(footprint.angle.unwrap_or(0.0)),
            "description": description,
            "parameters": parameters,
            "kicad_uuid": footprint.uuid.unwrap_or_default(),
        }));
    }
    placements.sort_by(|left, right| {
        left["designator"]
            .as_str()
            .cmp(&right["designator"].as_str())
    });
    Ok((!placements.is_empty()).then(|| {
        json!({
            "units": "mm",
            "source_pcb": pcb.source_filename,
            "placements": placements,
        })
    }))
}

fn pnp_layer(layer: &str) -> &str {
    if layer.starts_with("B.") {
        "bottom"
    } else if layer.starts_with("F.") {
        "top"
    } else {
        layer
    }
}

fn round4(value: f64) -> f64 {
    // Formatting uses correctly rounded decimal conversion and ties-to-even,
    // matching Python's `round(value, 4)` without a lossy scale/multiply step.
    format!("{value:.4}").parse().unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::round4;

    #[test]
    fn pnp_rounding_matches_python_decimal_precision() {
        assert_eq!(round4(152.19825), 152.1983);
        assert_eq!(round4(149.89825), 149.8982);
    }
}

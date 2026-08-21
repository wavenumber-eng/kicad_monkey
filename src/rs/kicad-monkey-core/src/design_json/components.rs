use crate::KiCadNetlist;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn components_json(netlist: &KiCadNetlist, netlist_json: &Value) -> Value {
    let raw = netlist_json["components"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let pin_counts = component_pin_counts(netlist);
    Value::Array(
        netlist
            .components
            .iter()
            .enumerate()
            .map(|(index, component)| {
                let raw = raw.get(index).cloned().unwrap_or_else(|| json!({}));
                let prefix = component
                    .reference
                    .chars()
                    .take_while(char::is_ascii_alphabetic)
                    .collect::<String>()
                    .to_uppercase();
                json!({
                    "designator": component.reference,
                    "svg_id": component.instance_uuids.last().cloned().unwrap_or_default(),
                    "value": component.value,
                    "footprint": component.footprint,
                    "library_ref": raw["library_ref"],
                    "description": component.libsource_description,
                    "hierarchy": {
                        "base_designator": component.reference,
                        "channel": Value::Null,
                        "channel_index": Value::Null,
                        "sheet": component.sheet_path_names,
                        "sheet_path": component.sheet_path_names,
                        "sheet_path_uuids": component.sheet_path_uuids,
                    },
                    "classification": {
                        "prefix": prefix,
                        "type": component_type(&prefix),
                        "pin_count": pin_counts.get(component.reference.as_str()).copied().unwrap_or(0),
                    },
                    "parameters": raw["parameters"],
                })
            })
            .collect(),
    )
}

fn component_pin_counts(netlist: &KiCadNetlist) -> BTreeMap<&str, usize> {
    let mut pins = BTreeMap::<&str, BTreeSet<&str>>::new();
    for net in &netlist.nets {
        for terminal in &net.terminals {
            pins.entry(&terminal.designator)
                .or_default()
                .insert(&terminal.pin);
        }
    }
    pins.into_iter()
        .map(|(reference, pins)| (reference, pins.len()))
        .collect()
}

fn component_type(prefix: &str) -> &'static str {
    match prefix {
        "R" | "C" | "L" | "D" | "LED" => "passive_2pin",
        "U" | "IC" => "ic",
        "J" | "P" | "CON" => "connector",
        "Q" => "transistor",
        "T" | "TR" => "transformer",
        "Y" | "X" => "crystal",
        "F" => "fuse",
        "S" | "SW" => "switch",
        "K" | "RY" => "relay",
        "TP" => "test_point",
        "FID" => "fiducial",
        "MH" => "mounting_hole",
        _ => "unknown",
    }
}

use super::{KiCadNet, KiCadNetlist, KiCadNetlistComponent, KiCadNetlistTerminal};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const KICAD_NETLIST_JSON_SCHEMA: &str = "kicad_monkey.netlist.a0";
pub const KICAD_NETLIST_JSON_GENERATOR: &str = "kicad_monkey";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KiCadNetlistJsonMetadata<'a> {
    pub source: &'a str,
    pub date: &'a str,
    pub tool: &'a str,
}

impl Default for KiCadNetlistJsonMetadata<'static> {
    fn default() -> Self {
        Self {
            source: "",
            date: "",
            tool: KICAD_NETLIST_JSON_GENERATOR,
        }
    }
}

#[must_use]
pub fn build_kicad_netlist_json(
    netlist: &KiCadNetlist,
    metadata: KiCadNetlistJsonMetadata<'_>,
) -> Value {
    let component_svg_ids = netlist
        .components
        .iter()
        .filter_map(|component| {
            component
                .instance_uuids
                .last()
                .filter(|uuid| !component.reference.is_empty() && !uuid.is_empty())
                .map(|uuid| (component.reference.as_str(), uuid.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    json!({
        "schema": KICAD_NETLIST_JSON_SCHEMA,
        "generator": KICAD_NETLIST_JSON_GENERATOR,
        "components": netlist.components.iter().map(component_json).collect::<Vec<_>>(),
        "nets": netlist.nets.iter().enumerate().map(|(index, net)| {
            net_json(net, index + 1, &component_svg_ids)
        }).collect::<Vec<_>>(),
        "net_classes": net_classes_json(netlist),
        "design": {
            "source": metadata.source,
            "date": metadata.date,
            "tool": metadata.tool,
            "sheets": netlist.sheets,
        },
    })
}

fn component_json(component: &KiCadNetlistComponent) -> Value {
    let library_ref = match (
        component.libsource_lib.is_empty(),
        component.libsource_part.is_empty(),
    ) {
        (false, false) => format!("{}:{}", component.libsource_lib, component.libsource_part),
        (false, true) => component.libsource_lib.clone(),
        (true, false) => component.libsource_part.clone(),
        (true, true) => String::new(),
    };
    json!({
        "designator": component.reference,
        "value": component.value,
        "footprint": component.footprint,
        "library_ref": library_ref,
        "description": component.libsource_description,
        "parameters": component_parameters(component),
    })
}

fn component_parameters(component: &KiCadNetlistComponent) -> BTreeMap<String, String> {
    let mut parameters = component.properties.clone();
    parameters
        .entry("_source_cad".to_owned())
        .or_insert_with(|| "kicad".to_owned());
    insert_nonempty(
        &mut parameters,
        "kicad_libsource_lib",
        &component.libsource_lib,
    );
    insert_nonempty(
        &mut parameters,
        "kicad_libsource_part",
        &component.libsource_part,
    );
    insert_nonempty(
        &mut parameters,
        "kicad_sheet_path_names",
        &component.sheet_path_names,
    );
    insert_nonempty(
        &mut parameters,
        "kicad_sheet_path_uuids",
        &component.sheet_path_uuids,
    );
    if let Some(uuid) = component.instance_uuids.last() {
        insert_nonempty(&mut parameters, "kicad_instance_uuid", uuid);
    }
    parameters.insert("kicad_in_bom".to_owned(), component.in_bom.to_string());
    parameters.insert("kicad_on_board".to_owned(), component.on_board.to_string());
    parameters.insert("kicad_dnp".to_owned(), component.dnp.to_string());
    parameters
}

fn insert_nonempty(parameters: &mut BTreeMap<String, String>, key: &str, value: &str) {
    if !value.is_empty() {
        parameters.insert(key.to_owned(), value.to_owned());
    }
}

fn net_json(net: &KiCadNet, index: usize, component_svg_ids: &BTreeMap<&str, &str>) -> Value {
    let terminals = sorted_terminals(&net.terminals);
    let source_sheets = terminals
        .iter()
        .filter_map(|terminal| (!terminal.sheet_path.is_empty()).then_some(&terminal.sheet_path))
        .collect::<BTreeSet<_>>();
    json!({
        "uid": format!("{index:012x}"),
        "name": net.name,
        "auto_named": net.auto_named,
        "source_sheets": source_sheets,
        "terminals": terminals.iter().map(|terminal| json!({
            "designator": terminal.designator,
            "pin": terminal.pin,
            "pin_name": terminal.pin_name,
            "pin_type": pin_type(&terminal.pin_type),
        })).collect::<Vec<_>>(),
        "graphical": graphical_json(net, &terminals, component_svg_ids),
        "aliases": net.aliases.iter().filter(|alias| !alias.is_empty()).collect::<BTreeSet<_>>(),
        "endpoints": net_endpoints(net, &terminals, component_svg_ids),
        "driver_priority": net.driver_priority,
        "driver_kind": net.driver_kind,
        "net_class": net.net_class,
    })
}

fn sorted_terminals(terminals: &[KiCadNetlistTerminal]) -> Vec<&KiCadNetlistTerminal> {
    let mut terminals = terminals.iter().collect::<Vec<_>>();
    terminals.sort_by(|left, right| {
        (&left.designator, &left.pin, &left.pin_name, &left.pin_type).cmp(&(
            &right.designator,
            &right.pin,
            &right.pin_name,
            &right.pin_type,
        ))
    });
    terminals
}

fn graphical_json(
    net: &KiCadNet,
    terminals: &[&KiCadNetlistTerminal],
    component_svg_ids: &BTreeMap<&str, &str>,
) -> Value {
    let mut pins = Vec::new();
    let mut seen = BTreeSet::new();
    for terminal in terminals {
        let svg_id = if terminal.svg_id.is_empty() {
            component_svg_ids
                .get(terminal.designator.as_str())
                .copied()
                .unwrap_or("")
        } else {
            terminal.svg_id.as_str()
        };
        if svg_id.is_empty()
            || !seen.insert((terminal.designator.as_str(), terminal.pin.as_str(), svg_id))
        {
            continue;
        }
        let mut row = Map::new();
        row.insert("designator".to_owned(), json!(terminal.designator));
        row.insert("pin".to_owned(), json!(terminal.pin));
        row.insert("svg_id".to_owned(), json!(svg_id));
        if !terminal.source_pin_id.is_empty() && terminal.source_pin_id != svg_id {
            row.insert("source_pin_id".to_owned(), json!(terminal.source_pin_id));
        }
        pins.push(Value::Object(row));
    }
    json!({
        "wires": sorted_unique(&net.graphical.wires),
        "junctions": sorted_unique(&net.graphical.junctions),
        "labels": sorted_unique(&net.graphical.labels),
        "power_ports": sorted_unique(&net.graphical.power_ports),
        "ports": sorted_unique(&net.graphical.ports),
        "sheet_entries": sorted_unique(&net.graphical.sheet_entries),
        "pins": pins,
    })
}

fn net_endpoints(
    net: &KiCadNet,
    terminals: &[&KiCadNetlistTerminal],
    component_svg_ids: &BTreeMap<&str, &str>,
) -> Vec<Value> {
    let mut endpoints = net
        .endpoints
        .iter()
        .map(|endpoint| {
            let mut row = Map::new();
            row.insert("endpoint_id".to_owned(), json!(endpoint.endpoint_id));
            row.insert("role".to_owned(), json!(endpoint.role));
            row.insert("element_id".to_owned(), json!(endpoint.element_id));
            row.insert("object_id".to_owned(), json!(endpoint.object_id));
            row.insert("name".to_owned(), json!(endpoint.name));
            row.insert("source_sheet".to_owned(), json!(endpoint.source_sheet));
            if let Some((x, y)) = endpoint.connection_point {
                row.insert(
                    "connection_point".to_owned(),
                    json!({
                        "x": round_schematic_mm(x),
                        "y": round_schematic_mm(y),
                        "units": "mm",
                    }),
                );
            }
            Value::Object(row)
        })
        .collect::<Vec<_>>();
    for terminal in terminals {
        let svg_id = if terminal.svg_id.is_empty() {
            component_svg_ids
                .get(terminal.designator.as_str())
                .copied()
                .unwrap_or("")
        } else {
            terminal.svg_id.as_str()
        };
        let endpoint_id = format!("pin:{}:{}", terminal.designator, terminal.pin);
        let mut endpoint = Map::new();
        endpoint.insert("endpoint_id".to_owned(), json!(endpoint_id));
        endpoint.insert("role".to_owned(), json!("pin"));
        endpoint.insert("element_id".to_owned(), json!(svg_id));
        endpoint.insert(
            "object_id".to_owned(),
            json!(if terminal.source_pin_id.is_empty() {
                svg_id
            } else {
                terminal.source_pin_id.as_str()
            }),
        );
        endpoint.insert(
            "name".to_owned(),
            json!(if terminal.pin_name.is_empty() {
                endpoint_id.as_str()
            } else {
                terminal.pin_name.as_str()
            }),
        );
        endpoint.insert("source_sheet".to_owned(), json!(terminal.sheet_path));
        endpoint.insert("designator".to_owned(), json!(terminal.designator));
        endpoint.insert("pin".to_owned(), json!(terminal.pin));
        endpoint.insert("pin_type".to_owned(), json!(pin_type(&terminal.pin_type)));
        if !terminal.pin_name.is_empty() {
            endpoint.insert("pin_name".to_owned(), json!(terminal.pin_name));
        }
        endpoints.push(Value::Object(endpoint));
    }
    endpoints.sort_by_key(endpoint_sort_key);
    endpoints.dedup();
    endpoints
}

fn endpoint_sort_key(endpoint: &Value) -> [String; 10] {
    let text = |key| {
        endpoint
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    [
        text("endpoint_id"),
        text("role"),
        text("element_id"),
        text("object_id"),
        text("name"),
        text("source_sheet"),
        text("designator"),
        text("pin"),
        text("pin_name"),
        text("pin_type"),
    ]
}

fn round_schematic_mm(value: i64) -> f64 {
    let value = value as f64 / 10_000.0;
    (value * 10_000.0).round() / 10_000.0
}

fn sorted_unique(values: &[String]) -> BTreeSet<&str> {
    values
        .iter()
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .collect()
}

fn pin_type(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "PASSIVE".to_owned()
    } else {
        value.to_uppercase()
    }
}

fn net_classes_json(netlist: &KiCadNetlist) -> Vec<Value> {
    let mut assigned = netlist
        .net_classes
        .iter()
        .map(|class| (class.name.as_str(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for net in &netlist.nets {
        if !net.net_class.is_empty() {
            assigned
                .entry(net.net_class.as_str())
                .or_default()
                .insert(net.name.as_str());
        }
    }
    netlist
        .net_classes
        .iter()
        .map(|class| {
            json!({
                "name": class.name,
                "description": class.description,
                "nets": assigned.get(class.name.as_str()).cloned().unwrap_or_default(),
            })
        })
        .collect()
}

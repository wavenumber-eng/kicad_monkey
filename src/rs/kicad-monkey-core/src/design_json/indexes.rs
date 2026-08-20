use crate::{KiCadNet, KiCadNetlist, KiCadNetlistTerminal};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn net_name_to_classes(netlist: &KiCadNetlist) -> Value {
    Value::Object(
        netlist
            .nets
            .iter()
            .filter(|net| !net.net_class.is_empty())
            .map(|net| (net.name.clone(), json!([net.net_class])))
            .collect(),
    )
}

pub(super) fn indexes_json(netlist: &KiCadNetlist) -> Value {
    let svg_to_component = netlist
        .components
        .iter()
        .filter_map(|component| {
            component
                .instance_uuids
                .last()
                .filter(|uuid| !uuid.is_empty() && !component.reference.is_empty())
                .map(|uuid| (uuid.clone(), json!(component.reference)))
        })
        .collect::<Map<_, _>>();
    let mut indexes = NetIndexes::default();
    for net in &netlist.nets {
        indexes.add_net(net);
    }
    let svg_to_net = indexes
        .svg_to_nets
        .iter()
        .filter(|(_, nets)| nets.len() == 1)
        .map(|(svg, nets)| (svg.clone(), json!(nets.first().expect("one net"))))
        .collect::<Map<_, _>>();
    json!({
        "svg_to_component": svg_to_component,
        "component_to_nets": indexes.component_to_nets,
        "net_to_components": indexes.net_to_components,
        "svg_to_net": svg_to_net,
        "svg_to_nets": indexes.svg_to_nets,
        "sheet_svg_to_nets": indexes.sheet_svg_to_nets,
        "net_to_graphics": indexes.net_to_graphics,
    })
}

type NetSetMap = BTreeMap<String, BTreeSet<String>>;
type SheetNetMap = BTreeMap<String, NetSetMap>;

#[derive(Default)]
struct NetIndexes {
    component_to_nets: NetSetMap,
    net_to_components: NetSetMap,
    svg_to_nets: NetSetMap,
    sheet_svg_to_nets: SheetNetMap,
    net_to_graphics: NetSetMap,
}

impl NetIndexes {
    fn add_net(&mut self, net: &KiCadNet) {
        for svg_id in net
            .graphical
            .wires
            .iter()
            .chain(&net.graphical.junctions)
            .chain(&net.graphical.labels)
            .chain(&net.graphical.power_ports)
            .chain(&net.graphical.ports)
            .chain(&net.graphical.sheet_entries)
        {
            self.add_graphic(&net.name, svg_id);
        }
        for terminal in &net.terminals {
            self.add_terminal(net, terminal);
        }
        for endpoint in &net.endpoints {
            for svg_id in [&endpoint.element_id, &endpoint.object_id] {
                self.add_sheet_graphic(&net.name, &endpoint.source_sheet, svg_id);
            }
        }
        self.net_to_components.entry(net.name.clone()).or_default();
    }

    fn add_terminal(&mut self, net: &KiCadNet, terminal: &KiCadNetlistTerminal) {
        if !terminal.designator.is_empty() {
            self.component_to_nets
                .entry(terminal.designator.clone())
                .or_default()
                .insert(net.name.clone());
            self.net_to_components
                .entry(net.name.clone())
                .or_default()
                .insert(terminal.designator.clone());
        }
        for svg_id in [&terminal.svg_id, &terminal.source_pin_id] {
            self.add_graphic(&net.name, svg_id);
            self.add_sheet_graphic(&net.name, &terminal.sheet_path, svg_id);
        }
    }

    fn add_graphic(&mut self, net_name: &str, svg_id: &str) {
        if svg_id.is_empty() {
            return;
        }
        self.svg_to_nets
            .entry(svg_id.to_owned())
            .or_default()
            .insert(net_name.to_owned());
        self.net_to_graphics
            .entry(net_name.to_owned())
            .or_default()
            .insert(svg_id.to_owned());
    }

    fn add_sheet_graphic(&mut self, net_name: &str, sheet: &str, svg_id: &str) {
        if sheet.is_empty() || svg_id.is_empty() {
            return;
        }
        self.sheet_svg_to_nets
            .entry(sheet.to_owned())
            .or_default()
            .entry(svg_id.to_owned())
            .or_default()
            .insert(net_name.to_owned());
    }
}

//! Board-scoped KiCad PTH pad/via copper-flash resolution.

use crate::pcb::{PcbNetRef, PcbPad, PcbRoutingArc, PcbSegment, PcbVia, PcbView};
use crate::sexpr::Error;
use std::collections::{HashMap, HashSet};
use std::f64::consts::{PI, TAU};

const GEOMETRY_TOLERANCE_MM: f64 = 1e-6;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum NetKey {
    Ordinal(i64),
    Name(String),
}

type Point = (f64, f64);
type RoutingKey = (NetKey, String);

#[derive(Clone, Debug)]
enum CopperShape {
    Circle {
        center: Point,
        radius: f64,
    },
    Capsule {
        start: Point,
        end: Point,
        radius: f64,
    },
    Polygon(Vec<Point>),
}

#[derive(Clone, Debug)]
struct ConnectedItem {
    source_start: usize,
    full_shapes: Vec<CopperShape>,
    hole_shapes: Vec<CopperShape>,
    layers: Vec<String>,
    conditional_layers: HashSet<String>,
    footprint_owner: Option<usize>,
    is_pad: bool,
    conditional_pad: bool,
}

struct ConnectionTarget<'a> {
    source_start: usize,
    shapes: &'a [CopperShape],
    physical_layers: &'a [String],
    footprint_owner: Option<usize>,
    is_pad: bool,
    conditional_pad: bool,
}

pub(super) struct BoardLayerFlashResolver {
    copper_stack: Vec<String>,
    routing_shapes: HashMap<RoutingKey, Vec<CopperShape>>,
    items_by_net: HashMap<NetKey, Vec<ConnectedItem>>,
    footprint_placements: Vec<(f64, f64, f64)>,
}

impl BoardLayerFlashResolver {
    pub(super) fn from_view(view: &PcbView<'_>) -> Result<Self, Error> {
        let copper_stack = view
            .layers()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|layer| layer.name)
            .filter(|name| name.ends_with(".Cu"))
            .collect::<Vec<_>>();
        let footprint_placements = view
            .footprints()
            .map(|footprint| {
                footprint.map(|footprint| {
                    (
                        footprint.at_x.unwrap_or_default(),
                        footprint.at_y.unwrap_or_default(),
                        footprint.angle.unwrap_or_default(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut routing_shapes: HashMap<RoutingKey, Vec<CopperShape>> = HashMap::new();
        for segment in view.segments() {
            let segment = segment?;
            let (Some(net), Some(layer)) = (net_key(&segment.net), segment.layer.clone()) else {
                continue;
            };
            if !layer.ends_with(".Cu") {
                continue;
            }
            routing_shapes
                .entry((net, layer))
                .or_default()
                .push(segment_shape(&segment));
        }
        for arc in view.arcs() {
            let arc = arc?;
            let (Some(net), Some(layer)) = (net_key(&arc.net), arc.layer.clone()) else {
                continue;
            };
            if !layer.ends_with(".Cu") {
                continue;
            }
            routing_shapes
                .entry((net, layer))
                .or_default()
                .extend(arc_shapes(&arc));
        }

        let mut items_by_net: HashMap<NetKey, Vec<ConnectedItem>> = HashMap::new();
        for via in view.vias() {
            let via = via?;
            if let Some(net) = net_key(&via.net) {
                let layers = copper_span_layers(&via.layers, &copper_stack);
                items_by_net.entry(net).or_default().push(ConnectedItem {
                    source_start: via.source_range.start,
                    full_shapes: vec![via_shape(&via)],
                    hole_shapes: vec![via_hole_shape(&via)],
                    conditional_layers: via_conditional_layers(&via, &layers),
                    layers,
                    footprint_owner: None,
                    is_pad: false,
                    conditional_pad: false,
                });
            }
        }
        for pad in view.pads() {
            let pad = pad?;
            let Some(&(fp_x, fp_y, fp_angle)) = footprint_placements.get(pad.footprint_index)
            else {
                continue;
            };
            if let Some(net) = net_key(&pad.net) {
                let layers = pad_copper_layers(&pad.layers, &copper_stack);
                items_by_net.entry(net).or_default().push(ConnectedItem {
                    source_start: pad.source_range.start,
                    full_shapes: pad_shapes(&pad, fp_x, fp_y, fp_angle),
                    hole_shapes: pad_hole_shapes(&pad, fp_x, fp_y, fp_angle),
                    conditional_layers: pad_conditional_layers(&pad, &layers),
                    layers,
                    footprint_owner: Some(pad.footprint_index),
                    is_pad: true,
                    conditional_pad: pad_policy_applies(&pad)
                        && pad.remove_unused_layers == Some(true),
                });
            }
        }

        Ok(Self {
            copper_stack,
            routing_shapes,
            items_by_net,
            footprint_placements,
        })
    }

    pub(super) fn via_flash_layers(&self, via: &PcbVia) -> Vec<String> {
        if self.copper_stack.is_empty() {
            return via.layers.clone();
        }
        let connected =
            if via.remove_unused_layers == Some(true) && via.start_end_only != Some(true) {
                let shapes = [via_hole_shape(via)];
                let physical_layers = copper_span_layers(&via.layers, &self.copper_stack);
                self.connected_layers(
                    &via.net,
                    ConnectionTarget {
                        source_start: via.source_range.start,
                        shapes: &shapes,
                        physical_layers: &physical_layers,
                        footprint_owner: None,
                        is_pad: false,
                        conditional_pad: false,
                    },
                )
            } else {
                HashSet::new()
            };
        effective_copper_layers(via, &self.copper_stack, connected)
    }

    pub(super) fn pad_item_layers(&self, pad: &PcbPad) -> Vec<String> {
        if self.copper_stack.is_empty() {
            return pad.layers.clone();
        }
        let connected = if pad_policy_applies(pad) && pad.remove_unused_layers == Some(true) {
            self.footprint_placements
                .get(pad.footprint_index)
                .map_or_else(HashSet::new, |&(fp_x, fp_y, fp_angle)| {
                    let shapes = pad_hole_shapes(pad, fp_x, fp_y, fp_angle);
                    let physical_layers = pad_copper_layers(&pad.layers, &self.copper_stack);
                    self.connected_layers(
                        &pad.net,
                        ConnectionTarget {
                            source_start: pad.source_range.start,
                            shapes: &shapes,
                            physical_layers: &physical_layers,
                            footprint_owner: Some(pad.footprint_index),
                            is_pad: true,
                            conditional_pad: true,
                        },
                    )
                })
        } else {
            HashSet::new()
        };
        effective_item_layers(pad, &self.copper_stack, connected)
    }

    pub(super) fn pad_drill_layers(&self, pad: &PcbPad) -> Vec<String> {
        if pad.kind == "np_thru_hole" {
            Vec::new()
        } else if self.copper_stack.is_empty() {
            pad.layers.clone()
        } else {
            self.copper_stack.clone()
        }
    }

    fn connected_layers(&self, net: &PcbNetRef, target: ConnectionTarget<'_>) -> HashSet<String> {
        let Some(net) = net_key(net) else {
            return HashSet::new();
        };
        let mut connected = self.connected_routing_layers(&net, &target);
        for other in self.items_by_net.get(&net).into_iter().flatten() {
            if connection_candidate(&target, other) {
                append_connected_item_layers(&target, other, &mut connected);
            }
        }
        connected
    }

    fn connected_routing_layers(
        &self,
        net: &NetKey,
        target: &ConnectionTarget<'_>,
    ) -> HashSet<String> {
        let mut connected = HashSet::new();
        for layer in target.physical_layers {
            if self
                .routing_shapes
                .get(&(net.clone(), layer.clone()))
                .is_some_and(|routes| shape_sets_intersect(target.shapes, routes))
            {
                connected.insert(layer.clone());
            }
        }
        connected
    }
}

fn connection_candidate(target: &ConnectionTarget<'_>, other: &ConnectedItem) -> bool {
    let same_source = other.source_start == target.source_start;
    let same_footprint = target
        .footprint_owner
        .is_some_and(|owner| other.footprint_owner == Some(owner));
    let conditional_pad_pair =
        target.is_pad && target.conditional_pad && other.is_pad && other.conditional_pad;
    !same_source && !same_footprint && !conditional_pad_pair
}

fn append_connected_item_layers(
    target: &ConnectionTarget<'_>,
    other: &ConnectedItem,
    connected: &mut HashSet<String>,
) {
    for layer in target.physical_layers {
        if !other.layers.contains(layer) {
            continue;
        }
        let shapes = if other.conditional_layers.contains(layer) {
            &other.hole_shapes
        } else {
            &other.full_shapes
        };
        if shape_sets_intersect(target.shapes, shapes) {
            connected.insert(layer.clone());
        }
    }
}

trait FlashPolicyItem {
    fn layers(&self) -> &[String];
    fn remove_unused_layers(&self) -> Option<bool>;
    fn keep_end_layers(&self) -> Option<bool>;
    fn start_end_only(&self) -> Option<bool> {
        None
    }
    fn forced_layers(&self) -> &[String];
    fn is_via(&self) -> bool {
        false
    }
    fn unused_layer_policy_applies(&self) -> bool {
        true
    }
}

impl FlashPolicyItem for PcbVia {
    fn layers(&self) -> &[String] {
        &self.layers
    }
    fn remove_unused_layers(&self) -> Option<bool> {
        self.remove_unused_layers
    }
    fn keep_end_layers(&self) -> Option<bool> {
        self.keep_end_layers
    }
    fn start_end_only(&self) -> Option<bool> {
        self.start_end_only
    }
    fn forced_layers(&self) -> &[String] {
        self.zone_layer_connections
            .as_ref()
            .map_or(&[], |value| value.forced_layers.as_slice())
    }
    fn is_via(&self) -> bool {
        true
    }
}

impl FlashPolicyItem for PcbPad {
    fn layers(&self) -> &[String] {
        &self.layers
    }
    fn remove_unused_layers(&self) -> Option<bool> {
        self.remove_unused_layers
    }
    fn keep_end_layers(&self) -> Option<bool> {
        self.keep_end_layers
    }
    fn forced_layers(&self) -> &[String] {
        self.zone_layer_connections
            .as_ref()
            .map_or(&[], |value| value.forced_layers.as_slice())
    }
    fn unused_layer_policy_applies(&self) -> bool {
        pad_policy_applies(self)
    }
}

fn effective_copper_layers<T: FlashPolicyItem>(
    item: &T,
    copper_stack: &[String],
    connected: HashSet<String>,
) -> Vec<String> {
    let span = if item.is_via() {
        copper_span_layers(item.layers(), copper_stack)
    } else {
        pad_copper_layers(item.layers(), copper_stack)
    };
    if span.is_empty() {
        return Vec::new();
    }
    if item.start_end_only() == Some(true) {
        return span
            .iter()
            .enumerate()
            .filter(|(index, _)| *index == 0 || *index + 1 == span.len())
            .map(|(_, layer)| layer.clone())
            .collect();
    }
    if item.remove_unused_layers() != Some(true) || !item.unused_layer_policy_applies() {
        return span;
    }
    let mut retained = connected;
    retained.extend(item.forced_layers().iter().cloned());
    if item.keep_end_layers() == Some(true) {
        if item.is_via() {
            retained.insert(span[0].clone());
            retained.insert(span[span.len() - 1].clone());
        } else {
            retained.extend(
                ["F.Cu", "B.Cu"]
                    .into_iter()
                    .filter(|layer| span.iter().any(|candidate| candidate == layer))
                    .map(str::to_owned),
            );
        }
    }
    span.into_iter()
        .filter(|layer| retained.contains(layer))
        .collect()
}

fn effective_item_layers<T: FlashPolicyItem>(
    item: &T,
    copper_stack: &[String],
    connected: HashSet<String>,
) -> Vec<String> {
    let flashed = effective_copper_layers(item, copper_stack, connected);
    let flashed = flashed.into_iter().collect::<HashSet<_>>();
    let mut result = Vec::new();
    for layer in item.layers() {
        append_effective_authored_layer(layer, copper_stack, &flashed, &mut result);
    }
    for layer in copper_stack {
        push_flashed_layer(layer, &flashed, &mut result);
    }
    result
}

fn append_effective_authored_layer(
    layer: &str,
    copper_stack: &[String],
    flashed: &HashSet<String>,
    result: &mut Vec<String>,
) {
    match layer {
        "*.Cu" => {
            for candidate in copper_stack {
                push_flashed_layer(candidate, flashed, result);
            }
        }
        "F&B.Cu" => {
            for candidate in ["F.Cu", "B.Cu"] {
                push_flashed_layer(candidate, flashed, result);
            }
        }
        _ if !layer.ends_with(".Cu") && !result.iter().any(|item| item == layer) => {
            result.push(layer.to_owned());
        }
        _ => {}
    }
}

fn push_flashed_layer(layer: &str, flashed: &HashSet<String>, result: &mut Vec<String>) {
    if flashed.contains(layer) && !result.iter().any(|item| item == layer) {
        result.push(layer.to_owned());
    }
}

fn copper_span_layers(authored: &[String], copper_stack: &[String]) -> Vec<String> {
    if authored.iter().any(|layer| layer == "*.Cu") {
        return copper_stack.to_vec();
    }
    let endpoints = authored
        .iter()
        .filter_map(|layer| copper_stack.iter().position(|candidate| candidate == layer))
        .collect::<Vec<_>>();
    match endpoints.as_slice() {
        [one] => vec![copper_stack[*one].clone()],
        [first, .., last] => {
            let (low, high) = if first <= last {
                (*first, *last)
            } else {
                (*last, *first)
            };
            copper_stack[low..=high].to_vec()
        }
        [] => Vec::new(),
    }
}

fn pad_copper_layers(authored: &[String], copper_stack: &[String]) -> Vec<String> {
    if authored.iter().any(|layer| layer == "*.Cu") {
        return copper_stack.to_vec();
    }
    copper_stack
        .iter()
        .filter(|layer| {
            authored.contains(layer)
                || (authored.iter().any(|item| item == "F&B.Cu")
                    && matches!(layer.as_str(), "F.Cu" | "B.Cu"))
        })
        .cloned()
        .collect()
}

fn net_key(net: &PcbNetRef) -> Option<NetKey> {
    net.ordinal.map(NetKey::Ordinal).or_else(|| {
        net.name
            .as_ref()
            .filter(|name| !name.is_empty())
            .cloned()
            .map(NetKey::Name)
    })
}

fn transformed_pad_position(pad: &PcbPad, fp_x: f64, fp_y: f64, fp_angle: f64) -> (f64, f64) {
    let angle = fp_angle.to_radians();
    (
        pad.at_x * angle.cos() + pad.at_y * angle.sin() + fp_x,
        -pad.at_x * angle.sin() + pad.at_y * angle.cos() + fp_y,
    )
}

fn segment_shape(segment: &PcbSegment) -> CopperShape {
    CopperShape::Capsule {
        start: (segment.start_x, segment.start_y),
        end: (segment.end_x, segment.end_y),
        radius: segment.width.unwrap_or_default().abs() / 2.0,
    }
}

fn via_shape(via: &PcbVia) -> CopperShape {
    CopperShape::Circle {
        center: (via.at_x, via.at_y),
        radius: via.size.abs() / 2.0,
    }
}

fn via_hole_shape(via: &PcbVia) -> CopperShape {
    CopperShape::Circle {
        center: (via.at_x, via.at_y),
        radius: via.drill.abs() / 2.0,
    }
}

fn pad_policy_applies(pad: &PcbPad) -> bool {
    pad.kind == "thru_hole"
}

fn via_conditional_layers(via: &PcbVia, layers: &[String]) -> HashSet<String> {
    let ends = layers
        .first()
        .into_iter()
        .chain(layers.last())
        .collect::<HashSet<_>>();
    if via.start_end_only == Some(true) {
        return layers
            .iter()
            .filter(|layer| !ends.contains(layer))
            .cloned()
            .collect();
    }
    if via.remove_unused_layers != Some(true) {
        return HashSet::new();
    }
    layers
        .iter()
        .filter(|layer| !(via.keep_end_layers == Some(true) && ends.contains(layer)))
        .cloned()
        .collect()
}

fn pad_conditional_layers(pad: &PcbPad, layers: &[String]) -> HashSet<String> {
    if !pad_policy_applies(pad) || pad.remove_unused_layers != Some(true) {
        return HashSet::new();
    }
    layers
        .iter()
        .filter(|layer| {
            !(pad.keep_end_layers == Some(true) && matches!(layer.as_str(), "F.Cu" | "B.Cu"))
        })
        .cloned()
        .collect()
}

fn pad_shapes(pad: &PcbPad, fp_x: f64, fp_y: f64, fp_angle: f64) -> Vec<CopperShape> {
    let anchor = transformed_pad_position(pad, fp_x, fp_y, fp_angle);
    let angle = -pad.angle.to_radians();
    let center = pad.drill.as_ref().map_or(anchor, |drill| {
        place_local((drill.offset.x, drill.offset.y), anchor, angle)
    });
    let size_x = pad.size_x.abs();
    let size_y = pad.size_y.abs();
    match pad.shape.as_str() {
        "circle" => vec![CopperShape::Circle {
            center,
            radius: size_x / 2.0,
        }],
        "oval" => {
            let (start, end, width) = if size_x >= size_y {
                let half = (size_x - size_y) / 2.0;
                ((-half, 0.0), (half, 0.0), size_y)
            } else {
                let half = (size_y - size_x) / 2.0;
                ((0.0, -half), (0.0, half), size_x)
            };
            vec![CopperShape::Capsule {
                start: place_local(start, center, angle),
                end: place_local(end, center, angle),
                radius: width / 2.0,
            }]
        }
        "roundrect" => vec![CopperShape::Polygon(roundrect_points(
            center,
            angle,
            size_x,
            size_y,
            pad.roundrect_rratio.unwrap_or(0.25),
        ))],
        "trapezoid" => vec![CopperShape::Polygon(trapezoid_points(pad, center, angle))],
        "custom" => custom_pad_shapes(pad, center, angle),
        _ => vec![CopperShape::Polygon(rectangle_points(
            center, angle, size_x, size_y,
        ))],
    }
}

fn pad_hole_shapes(pad: &PcbPad, fp_x: f64, fp_y: f64, fp_angle: f64) -> Vec<CopperShape> {
    let center = transformed_pad_position(pad, fp_x, fp_y, fp_angle);
    let Some(drill) = &pad.drill else {
        return Vec::new();
    };
    let height = drill.height.unwrap_or(drill.width).abs();
    let width = drill.width.abs();
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    if (width - height).abs() <= GEOMETRY_TOLERANCE_MM {
        return vec![CopperShape::Circle {
            center,
            radius: width / 2.0,
        }];
    }
    let angle = -pad.angle.to_radians();
    let (start, end, diameter) = if width >= height {
        let half = (width - height) / 2.0;
        ((-half, 0.0), (half, 0.0), height)
    } else {
        let half = (height - width) / 2.0;
        ((0.0, -half), (0.0, half), width)
    };
    vec![CopperShape::Capsule {
        start: place_local(start, center, angle),
        end: place_local(end, center, angle),
        radius: diameter / 2.0,
    }]
}

fn custom_pad_shapes(pad: &PcbPad, center: Point, angle: f64) -> Vec<CopperShape> {
    let mut shapes = if pad
        .custom_options
        .as_ref()
        .and_then(|options| options.anchor.as_deref())
        == Some("circle")
    {
        vec![CopperShape::Circle {
            center,
            radius: pad.size_x.abs().min(pad.size_y.abs()) / 2.0,
        }]
    } else {
        vec![CopperShape::Polygon(rectangle_points(
            center,
            angle,
            pad.size_x.abs(),
            pad.size_y.abs(),
        ))]
    };
    for primitive in &pad.custom_primitives {
        if primitive.kind != "gr_poly" || primitive.points.len() < 2 {
            continue;
        }
        let points = primitive
            .points
            .iter()
            .map(|point| place_local((point.x, point.y), center, angle))
            .collect::<Vec<_>>();
        if points.len() >= 3 {
            shapes.push(CopperShape::Polygon(points.clone()));
        }
        let radius = primitive.width.unwrap_or_default().abs() / 2.0;
        if radius > 0.0 {
            for edge in points.windows(2) {
                shapes.push(CopperShape::Capsule {
                    start: edge[0],
                    end: edge[1],
                    radius,
                });
            }
        }
    }
    shapes
}

fn rectangle_points(center: Point, angle: f64, size_x: f64, size_y: f64) -> Vec<Point> {
    let half_x = size_x / 2.0;
    let half_y = size_y / 2.0;
    [
        (-half_x, -half_y),
        (half_x, -half_y),
        (half_x, half_y),
        (-half_x, half_y),
    ]
    .into_iter()
    .map(|point| place_local(point, center, angle))
    .collect()
}

fn trapezoid_points(pad: &PcbPad, center: Point, angle: f64) -> Vec<Point> {
    let half_x = pad.size_x.abs() / 2.0;
    let half_y = pad.size_y.abs() / 2.0;
    let delta_x = pad.rect_delta_x.unwrap_or_default() / 2.0;
    let delta_y = pad.rect_delta_y.unwrap_or_default() / 2.0;
    [
        (-half_x - delta_y, half_y + delta_x),
        (half_x + delta_y, half_y - delta_x),
        (half_x - delta_y, -half_y + delta_x),
        (-half_x + delta_y, -half_y - delta_x),
    ]
    .into_iter()
    .map(|point| place_local(point, center, angle))
    .collect()
}

fn roundrect_points(center: Point, angle: f64, size_x: f64, size_y: f64, ratio: f64) -> Vec<Point> {
    let half_x = size_x / 2.0;
    let half_y = size_y / 2.0;
    let radius = (size_x.min(size_y) * ratio).clamp(0.0, half_x.min(half_y));
    if radius <= GEOMETRY_TOLERANCE_MM {
        return rectangle_points(center, angle, size_x, size_y);
    }
    let corners = [
        (-half_x + radius, -half_y + radius, PI),
        (half_x - radius, -half_y + radius, 1.5 * PI),
        (half_x - radius, half_y - radius, 0.0),
        (-half_x + radius, half_y - radius, 0.5 * PI),
    ];
    let mut points = Vec::with_capacity(36);
    for (cx, cy, start) in corners {
        for step in 0..=8 {
            let theta = start + step as f64 * PI / 16.0;
            points.push(place_local(
                (cx + radius * theta.cos(), cy + radius * theta.sin()),
                center,
                angle,
            ));
        }
    }
    points
}

fn place_local(point: Point, center: Point, angle: f64) -> Point {
    (
        center.0 + point.0 * angle.cos() - point.1 * angle.sin(),
        center.1 + point.0 * angle.sin() + point.1 * angle.cos(),
    )
}

fn arc_shapes(arc: &PcbRoutingArc) -> Vec<CopperShape> {
    let start = (arc.start.x, arc.start.y);
    let mid = (arc.mid.x, arc.mid.y);
    let end = (arc.end.x, arc.end.y);
    let radius = arc.width.unwrap_or_default().abs() / 2.0;
    let Some(center) = circumcenter(start, mid, end) else {
        return vec![CopperShape::Capsule { start, end, radius }];
    };
    let circle_radius = distance(center, start);
    if circle_radius <= GEOMETRY_TOLERANCE_MM {
        return vec![CopperShape::Circle {
            center: start,
            radius,
        }];
    }
    let start_angle = (start.1 - center.1).atan2(start.0 - center.0);
    let mid_angle = (mid.1 - center.1).atan2(mid.0 - center.0);
    let end_angle = (end.1 - center.1).atan2(end.0 - center.0);
    let ccw_end = positive_angle(end_angle - start_angle);
    let ccw_mid = positive_angle(mid_angle - start_angle);
    let sweep = if ccw_mid <= ccw_end + GEOMETRY_TOLERANCE_MM {
        ccw_end
    } else {
        -(TAU - ccw_end)
    };
    let steps = ((sweep.abs() / (PI / 72.0)).ceil() as usize).clamp(1, 288);
    let step_angle = sweep / steps as f64;
    let sagitta = circle_radius * (1.0 - (step_angle.abs() / 2.0).cos());
    let mut shapes = Vec::with_capacity(steps);
    let mut previous = start;
    for index in 1..=steps {
        let theta = start_angle + step_angle * index as f64;
        let current = (
            center.0 + circle_radius * theta.cos(),
            center.1 + circle_radius * theta.sin(),
        );
        shapes.push(CopperShape::Capsule {
            start: previous,
            end: current,
            radius: radius + sagitta,
        });
        previous = current;
    }
    shapes
}

fn positive_angle(value: f64) -> f64 {
    value.rem_euclid(TAU)
}

fn circumcenter(a: Point, b: Point, c: Point) -> Option<Point> {
    let denominator = 2.0 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if denominator.abs() <= GEOMETRY_TOLERANCE_MM {
        return None;
    }
    let aa = a.0 * a.0 + a.1 * a.1;
    let bb = b.0 * b.0 + b.1 * b.1;
    let cc = c.0 * c.0 + c.1 * c.1;
    Some((
        (aa * (b.1 - c.1) + bb * (c.1 - a.1) + cc * (a.1 - b.1)) / denominator,
        (aa * (c.0 - b.0) + bb * (a.0 - c.0) + cc * (b.0 - a.0)) / denominator,
    ))
}

fn shape_sets_intersect(left: &[CopperShape], right: &[CopperShape]) -> bool {
    left.iter().any(|left_shape| {
        right
            .iter()
            .any(|right_shape| shapes_intersect(left_shape, right_shape))
    })
}

fn shapes_intersect(left: &CopperShape, right: &CopperShape) -> bool {
    match (left, right) {
        (
            CopperShape::Circle {
                center: left_center,
                radius: left_radius,
            },
            CopperShape::Circle {
                center: right_center,
                radius: right_radius,
            },
        ) => circles_intersect(*left_center, *left_radius, *right_center, *right_radius),
        (
            CopperShape::Capsule {
                start: left_start,
                end: left_end,
                radius: left_radius,
            },
            CopperShape::Capsule {
                start: right_start,
                end: right_end,
                radius: right_radius,
            },
        ) => capsules_intersect(
            (*left_start, *left_end, *left_radius),
            (*right_start, *right_end, *right_radius),
        ),
        (
            CopperShape::Circle { center, radius },
            CopperShape::Capsule {
                start,
                end,
                radius: other,
            },
        )
        | (
            CopperShape::Capsule {
                start,
                end,
                radius: other,
            },
            CopperShape::Circle { center, radius },
        ) => circle_capsule_intersect(*center, *radius, *start, *end, *other),
        (CopperShape::Circle { center, radius }, CopperShape::Polygon(points))
        | (CopperShape::Polygon(points), CopperShape::Circle { center, radius }) => {
            circle_polygon_intersect(*center, *radius, points)
        }
        (CopperShape::Capsule { start, end, radius }, CopperShape::Polygon(points))
        | (CopperShape::Polygon(points), CopperShape::Capsule { start, end, radius }) => {
            capsule_polygon_intersect(*start, *end, *radius, points)
        }
        (CopperShape::Polygon(left_points), CopperShape::Polygon(right_points)) => {
            polygons_intersect(left_points, right_points)
        }
    }
}

fn circles_intersect(left: Point, left_radius: f64, right: Point, right_radius: f64) -> bool {
    distance(left, right) <= left_radius + right_radius + GEOMETRY_TOLERANCE_MM
}

fn capsules_intersect(left: (Point, Point, f64), right: (Point, Point, f64)) -> bool {
    segment_distance(left.0, left.1, right.0, right.1) <= left.2 + right.2 + GEOMETRY_TOLERANCE_MM
}

fn circle_capsule_intersect(
    center: Point,
    radius: f64,
    start: Point,
    end: Point,
    capsule_radius: f64,
) -> bool {
    point_segment_distance(center, start, end) <= radius + capsule_radius + GEOMETRY_TOLERANCE_MM
}

fn circle_polygon_intersect(center: Point, radius: f64, points: &[Point]) -> bool {
    point_in_polygon(center, points)
        || polygon_edges(points).any(|(start, end)| {
            point_segment_distance(center, start, end) <= radius + GEOMETRY_TOLERANCE_MM
        })
}

fn capsule_polygon_intersect(start: Point, end: Point, radius: f64, points: &[Point]) -> bool {
    point_in_polygon(start, points)
        || point_in_polygon(end, points)
        || polygon_edges(points).any(|(edge_start, edge_end)| {
            segment_distance(start, end, edge_start, edge_end) <= radius + GEOMETRY_TOLERANCE_MM
        })
}

fn polygons_intersect(left: &[Point], right: &[Point]) -> bool {
    left.first()
        .is_some_and(|point| point_in_polygon(*point, right))
        || right
            .first()
            .is_some_and(|point| point_in_polygon(*point, left))
        || polygon_edges(left).any(|(left_start, left_end)| {
            polygon_edges(right).any(|(right_start, right_end)| {
                segments_intersect(left_start, left_end, right_start, right_end)
            })
        })
}

fn polygon_edges(points: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
}

fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    for (start, end) in polygon_edges(polygon) {
        if point_segment_distance(point, start, end) <= GEOMETRY_TOLERANCE_MM {
            return true;
        }
        if (start.1 > point.1) != (end.1 > point.1) {
            let intersect = start.0 + (point.1 - start.1) * (end.0 - start.0) / (end.1 - start.1);
            if point.0 < intersect {
                inside = !inside;
            }
        }
    }
    inside
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    segment_distance(a, b, c, d) <= GEOMETRY_TOLERANCE_MM
}

fn segment_distance(a: Point, b: Point, c: Point, d: Point) -> f64 {
    if proper_segments_intersect(a, b, c, d) {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

fn proper_segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let orient =
        |p: Point, q: Point, r: Point| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let o1 = orient(a, b, c);
    let o2 = orient(a, b, d);
    let o3 = orient(c, d, a);
    let o4 = orient(c, d, b);
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn point_segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= GEOMETRY_TOLERANCE_MM * GEOMETRY_TOLERANCE_MM {
        return distance(point, start);
    }
    let projection =
        (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared).clamp(0.0, 1.0);
    distance(
        point,
        (start.0 + projection * dx, start.1 + projection * dy),
    )
}

fn distance(left: Point, right: Point) -> f64 {
    (right.0 - left.0).hypot(right.1 - left.1)
}

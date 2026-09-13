use super::*;
mod construction;
mod custom_pads;
mod footprints;
mod groups;
mod pads;
mod padstacks;
mod presentation;
mod routing;
mod rule_areas;
mod zones;
use crate::board_plotter_ir::BoardTextVariables;
use crate::sexpr::ErrorKind;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn board(value: &AuthoredPcb, limits: PcbAuthoringLimits) -> Result<(), Error> {
    supported_version(value.version)?;
    let mut state = Validation::new(limits);
    state.text(&value.generator)?;
    state.text(&value.generator_version)?;
    state.text(&value.paper)?;
    supported_token(
        &value.paper,
        "paper size",
        &[
            "A0", "A1", "A2", "A3", "A4", "A5", "A", "B", "C", "D", "E", "USLetter", "USLegal",
            "USLedger",
        ],
    )?;
    positive(value.thickness_mm, "board thickness")?;
    let layers = state.layers(&value.layers)?;
    state.setup(&value.setup, &layers)?;
    let nets = state.nets(&value.nets)?;
    state.board_properties(&value.properties)?;
    state.board_artwork(value, &nets, &layers)?;
    for occurrence in &value.footprints {
        state.occurrence(occurrence, &nets, &layers)?;
    }
    for via in &value.vias {
        state.via(via, &nets, &layers)?;
    }
    for segment in &value.segments {
        state.segment(segment, &nets, &layers)?;
    }
    for arc in &value.arcs {
        state.routing_arc(arc, &nets, &layers)?;
    }
    for zone in &value.zones {
        state.zone(zone, &nets, &layers)?;
    }
    for area in &value.rule_areas {
        state.rule_area(area, &layers)?;
    }
    state.groups(value)?;
    state.resources(&value.embedded_files)?;
    validate_board_resource_namespace(value)?;
    validate_board_model_resource_associations(value)?;
    state.finish()
}

pub(super) fn footprint(
    value: &AuthoredStandaloneFootprint,
    limits: PcbAuthoringLimits,
) -> Result<(), Error> {
    supported_version(value.version)?;
    let mut state = Validation::new(limits);
    for text in [&value.generator, &value.generator_version, &value.layer] {
        state.text(text)?;
    }
    if value.generator.is_empty() || value.generator_version.is_empty() || value.layer.is_empty() {
        return Err(invalid(
            "standalone footprint generator, generator version, and layer must be nonempty",
        ));
    }
    if value.footprint.name.contains(':') {
        return Err(invalid(
            "standalone footprint names cannot contain a library-link colon",
        ));
    }
    require_footprint_root_layer(&value.layer, None)?;
    state.footprint(&value.footprint, None, None, None)?;
    validate_standalone_model_resource_associations(&value.footprint)?;
    state.finish()
}

fn supported_version(version: i64) -> Result<(), Error> {
    if version != KICAD_SOURCE_VERSION_2024_12_29 {
        return Err(invalid(format!(
            "unsupported authored KiCad source version {version}; supported: {KICAD_SOURCE_VERSION_2024_12_29}"
        )));
    }
    Ok(())
}

fn validate_board_resource_namespace(value: &AuthoredPcb) -> Result<(), Error> {
    let resources = value.embedded_files.iter().chain(
        value
            .footprints
            .iter()
            .flat_map(|occurrence| &occurrence.footprint.embedded_files),
    );
    let mut namespace: BTreeMap<&str, (&str, Option<&AuthoredResourceData>)> = BTreeMap::new();
    for resource in resources {
        let payload = (!matches!(resource.data, AuthoredResourceData::DeclarationOnly))
            .then_some(&resource.data);
        let Some((file_type, authoritative_payload)) = namespace.get_mut(resource.name.as_str())
        else {
            namespace.insert(&resource.name, (&resource.file_type, payload));
            continue;
        };
        if *file_type != resource.file_type {
            return Err(invalid(format!(
                "resource {:?} has conflicting types in the effective board-wide namespace",
                resource.name
            )));
        }
        if let Some(payload) = payload {
            match authoritative_payload {
                Some(previous) if *previous != payload => {
                    return Err(invalid(format!(
                        "resource {:?} has conflicting payloads in the effective board-wide namespace; KiCad hoists footprint payloads into one board resource",
                        resource.name
                    )));
                }
                Some(_) => {}
                None => *authoritative_payload = Some(payload),
            }
        }
    }
    Ok(())
}

fn embedded_model_resource_name(model: &AuthoredModel) -> Result<Option<&str>, Error> {
    let Some(name) = model.path.strip_prefix("kicad-embed://") else {
        return Ok(None);
    };
    if name.is_empty() {
        return Err(invalid(
            "embedded model paths require a nonempty resource name",
        ));
    }
    Ok(Some(name))
}

fn model_resource_names(resources: &[AuthoredEmbeddedFile]) -> BTreeSet<&str> {
    resources
        .iter()
        .filter(|resource| resource.file_type == "model")
        .map(|resource| resource.name.as_str())
        .collect()
}

fn validate_standalone_model_resource_associations(
    footprint: &AuthoredFootprint,
) -> Result<(), Error> {
    let local = model_resource_names(&footprint.embedded_files);
    for model in &footprint.models {
        let Some(name) = embedded_model_resource_name(model)? else {
            continue;
        };
        if !local.contains(name) {
            return Err(invalid(format!(
                "embedded model path {:?} requires a matching local model resource declaration",
                model.path
            )));
        }
    }
    Ok(())
}

fn validate_board_model_resource_associations(value: &AuthoredPcb) -> Result<(), Error> {
    let board = model_resource_names(&value.embedded_files);
    for occurrence in &value.footprints {
        let local = model_resource_names(&occurrence.footprint.embedded_files);
        for model in &occurrence.footprint.models {
            let Some(name) = embedded_model_resource_name(model)? else {
                continue;
            };
            if !local.contains(name) && !board.contains(name) {
                return Err(invalid(format!(
                    "embedded model path {:?} requires a matching local or board model resource declaration",
                    model.path
                )));
            }
        }
    }
    Ok(())
}

struct Validation {
    limits: PcbAuthoringLimits,
    objects: usize,
    points: usize,
    string_bytes: usize,
    resource_input_bytes: usize,
    identities: BTreeSet<String>,
}

impl Validation {
    fn board_artwork(
        &mut self,
        value: &AuthoredPcb,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        for graphic in &value.profile {
            if graphic.layer != "Edge.Cuts" {
                return Err(invalid("board profile graphics must use Edge.Cuts"));
            }
            self.graphic(graphic, Some(nets), Some(layers), false)?;
        }
        for graphic in &value.graphics {
            self.graphic(graphic, Some(nets), Some(layers), false)?;
        }
        for text in &value.texts {
            self.board_text(text, layers)?;
        }
        for text_box in &value.text_boxes {
            self.text_box(text_box, Some(layers), None, None)?;
        }
        Ok(())
    }

    fn board_properties(&mut self, properties: &[AuthoredProperty]) -> Result<(), Error> {
        let mut property_names = BTreeSet::new();
        for property in properties {
            self.object()?;
            self.text(&property.name)?;
            self.text(&property.value)?;
            if !property_names.insert(&property.name) {
                return Err(invalid(
                    "duplicate board property names are discarded by KiCad",
                ));
            }
        }
        Ok(())
    }

    fn new(limits: PcbAuthoringLimits) -> Self {
        Self {
            limits,
            objects: 0,
            points: 0,
            string_bytes: 0,
            resource_input_bytes: 0,
            identities: BTreeSet::new(),
        }
    }

    fn finish(&self) -> Result<(), Error> {
        // Every authored string byte must occur at least once in the output.
        // Reject impossible small output caps before allocating the S-expression tree.
        if self.string_bytes > self.limits.max_output_bytes {
            return Err(limit());
        }
        Ok(())
    }

    fn object(&mut self) -> Result<(), Error> {
        self.objects = self.objects.checked_add(1).ok_or_else(limit)?;
        if self.objects > self.limits.max_objects {
            return Err(limit());
        }
        Ok(())
    }

    fn point(&mut self, point: AuthoredPoint) -> Result<(), Error> {
        finite(point.x_mm, "point X")?;
        finite(point.y_mm, "point Y")?;
        self.points = self.points.checked_add(1).ok_or_else(limit)?;
        if self.points > self.limits.max_points {
            return Err(limit());
        }
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.string_bytes = self
            .string_bytes
            .checked_add(value.len())
            .ok_or_else(limit)?;
        if self.string_bytes > self.limits.max_string_bytes {
            return Err(limit());
        }
        Ok(())
    }

    fn identity(&mut self, value: &str) -> Result<(), Error> {
        self.text(value)?;
        if !is_uuid(value) {
            return Err(invalid(format!("invalid KiCad UUID {value:?}")));
        }
        // UUID hex casing does not change identity; retain the authored spelling
        // in the source value and normalize only this comparison key.
        if !self.identities.insert(value.to_ascii_lowercase()) {
            return Err(invalid(format!("duplicate KiCad UUID {value:?}")));
        }
        Ok(())
    }

    fn model(&mut self, model: &AuthoredModel) -> Result<(), Error> {
        self.object()?;
        self.text(&model.path)?;
        if model.path.is_empty() {
            return Err(invalid("model path must be nonempty"));
        }
        for value in model
            .offset_mm
            .into_iter()
            .chain(model.scale)
            .chain(model.rotate_degrees)
        {
            finite(value, "model transform")?;
        }
        Ok(())
    }

    fn resources(&mut self, resources: &[AuthoredEmbeddedFile]) -> Result<(), Error> {
        let mut names = BTreeSet::new();
        for resource in resources {
            self.object()?;
            self.text(&resource.name)?;
            self.text(&resource.file_type)?;
            if resource.name.is_empty() || resource.file_type.is_empty() {
                return Err(invalid("resource name and type must be nonempty"));
            }
            supported_token(
                &resource.file_type,
                "embedded resource type",
                &["model", "font", "other"],
            )?;
            if !names.insert(&resource.name) {
                return Err(invalid(format!(
                    "duplicate embedded resource name {:?} in one owner",
                    resource.name
                )));
            }
            if let AuthoredResourceData::Bytes(bytes) = &resource.data {
                self.resource_input_bytes = self
                    .resource_input_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(limit)?;
                if self.resource_input_bytes > self.limits.max_resource_input_bytes {
                    return Err(limit());
                }
            }
        }
        Ok(())
    }
}

fn require_layer(
    layer: &str,
    board_layers: Option<&BTreeMap<String, String>>,
    allow_pad_wildcard: bool,
) -> Result<(), Error> {
    if layer.is_empty() {
        return Err(invalid("authored layer references must be nonempty"));
    }
    if allow_pad_wildcard && matches!(layer, "*.Cu" | "*.Mask" | "F&B.Cu") {
        return Ok(());
    }
    if let Some(layers) = board_layers
        && !layers.contains_key(layer)
    {
        return Err(invalid(format!(
            "authored layer reference {layer:?} is absent from the board layer table"
        )));
    }
    if board_layers.is_none() && canonical_layer_ordinal(layer).is_none() {
        return Err(invalid(format!(
            "unsupported standalone footprint layer reference {layer:?}"
        )));
    }
    Ok(())
}

fn require_footprint_layer(
    layer: &str,
    board_layers: Option<&BTreeMap<String, String>>,
    allow_pad_wildcard: bool,
) -> Result<(), Error> {
    if let Some(layers) = board_layers
        && !layers.contains_key(layer)
        && canonical_layer_ordinal(layer).is_some()
        && canonical_copper_order(layer).is_none()
    {
        // KiCad permits footprint presentation members on a known user layer
        // even when that layer is not enabled in the owning board table.
        return Ok(());
    }
    require_layer(layer, board_layers, allow_pad_wildcard)
}

fn require_footprint_root_layer(
    layer: &str,
    board_layers: Option<&BTreeMap<String, String>>,
) -> Result<(), Error> {
    require_layer(layer, board_layers, false)?;
    if !matches!(layer, "F.Cu" | "B.Cu") {
        return Err(invalid(format!(
            "footprint root layer must be F.Cu or B.Cu, got {layer:?}"
        )));
    }
    Ok(())
}

fn require_copper_layer(
    layer: &str,
    board_layers: Option<&BTreeMap<String, String>>,
) -> Result<(), Error> {
    require_layer(layer, board_layers, false)?;
    let is_copper = board_layers.map_or_else(
        || canonical_copper_order(layer).is_some(),
        |layers| layers.get(layer).is_some_and(|kind| kind != "user"),
    );
    if !is_copper {
        return Err(invalid(format!(
            "authored copper-layer reference {layer:?} is not a declared copper layer"
        )));
    }
    Ok(())
}

fn declared_copper_order(layers: &BTreeMap<String, String>) -> Vec<String> {
    let mut copper = layers
        .iter()
        .filter(|(_, kind)| kind.as_str() != "user")
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    copper.sort_by_key(|name| canonical_copper_order(name).unwrap_or(i64::MAX));
    copper
}

fn validate_layer_slot(layer: &AuthoredLayer) -> Result<(), Error> {
    let Some(expected_name) = canonical_layer_name(layer.ordinal) else {
        return Err(invalid(format!(
            "unsupported KiCad 20241229 layer ordinal {}",
            layer.ordinal
        )));
    };
    if layer.name != expected_name {
        return Err(invalid(format!(
            "layer ordinal {} requires canonical name {expected_name:?}, got {:?}; use user_name for display renames",
            layer.ordinal, layer.name
        )));
    }
    let copper = canonical_copper_order(&layer.name).is_some();
    if copper == (layer.kind == "user") {
        return Err(invalid(format!(
            "layer {:?} requires a {} kind",
            layer.name,
            if copper { "copper" } else { "user" }
        )));
    }
    Ok(())
}

fn canonical_copper_order(name: &str) -> Option<i64> {
    match name {
        "F.Cu" => Some(0),
        "B.Cu" => Some(31),
        _ => name
            .strip_prefix("In")
            .and_then(|value| value.strip_suffix(".Cu"))
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|ordinal| (1..=30).contains(ordinal)),
    }
}

const CANONICAL_LAYER_SLOTS: &[(&str, i64)] = &[
    ("F.Cu", 0),
    ("B.Cu", 2),
    ("F.Mask", 1),
    ("B.Mask", 3),
    ("F.SilkS", 5),
    ("B.SilkS", 7),
    ("F.Adhes", 9),
    ("B.Adhes", 11),
    ("F.Paste", 13),
    ("B.Paste", 15),
    ("Dwgs.User", 17),
    ("Cmts.User", 19),
    ("Eco1.User", 21),
    ("Eco2.User", 23),
    ("Edge.Cuts", 25),
    ("Margin", 27),
    ("B.CrtYd", 29),
    ("F.CrtYd", 31),
    ("B.Fab", 33),
    ("F.Fab", 35),
];

fn canonical_layer_ordinal(name: &str) -> Option<i64> {
    CANONICAL_LAYER_SLOTS
        .iter()
        .find_map(|(token, ordinal)| (*token == name).then_some(*ordinal))
        .or_else(|| {
            name.strip_prefix("In")
                .and_then(|value| value.strip_suffix(".Cu"))
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|ordinal| (1..=30).contains(ordinal))
                .map(|ordinal| (ordinal + 1) * 2)
                .or_else(|| {
                    name.strip_prefix("User.")
                        .and_then(|value| value.parse::<i64>().ok())
                        .filter(|ordinal| (1..=45).contains(ordinal))
                        .map(|ordinal| ordinal * 2 + 37)
                })
        })
}

fn canonical_layer_name(ordinal: i64) -> Option<String> {
    match ordinal {
        4..=62 if ordinal % 2 == 0 => Some(format!("In{}.Cu", ordinal / 2 - 1)),
        39..=127 if ordinal % 2 == 1 => Some(format!("User.{}", (ordinal - 37) / 2)),
        _ => CANONICAL_LAYER_SLOTS
            .iter()
            .find_map(|(name, slot)| (*slot == ordinal).then(|| (*name).to_owned())),
    }
}

fn supported_token(value: &str, label: &str, supported: &[&str]) -> Result<(), Error> {
    if !supported.contains(&value) {
        return Err(invalid(format!(
            "unsupported {label} {value:?}; supported: {}",
            supported.join(", ")
        )));
    }
    Ok(())
}

fn require_net(code: i64, nets: &BTreeMap<i64, String>) -> Result<(), Error> {
    if !nets.contains_key(&code) {
        return Err(invalid(format!("unknown authored net code {code}")));
    }
    Ok(())
}

fn require_unique_strings(values: &[String], label: &str) -> Result<(), Error> {
    let mut unique = BTreeSet::new();
    if values.iter().all(|value| unique.insert(value.as_str())) {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be unique")))
    }
}

fn roundrect_ratio(value: f64) -> Result<(), Error> {
    finite(value, "roundrect radius ratio")?;
    if !(0.0..=0.5).contains(&value) {
        return Err(invalid("roundrect radius ratio must be between 0 and 0.5"));
    }
    Ok(())
}

fn finite(value: f64, label: &str) -> Result<(), Error> {
    if !value.is_finite() {
        return Err(invalid(format!("{label} must be finite")));
    }
    Ok(())
}

fn positive(value: f64, label: &str) -> Result<(), Error> {
    finite(value, label)?;
    if value <= 0.0 {
        return Err(invalid(format!("{label} must be positive")));
    }
    Ok(())
}

fn nonnegative(value: f64, label: &str) -> Result<(), Error> {
    finite(value, label)?;
    if value < 0.0 {
        return Err(invalid(format!("{label} must be nonnegative")));
    }
    Ok(())
}

fn optional_positive(value: Option<f64>, label: &str) -> Result<(), Error> {
    value.map_or(Ok(()), |value| positive(value, label))
}

fn optional_nonnegative(value: Option<f64>, label: &str) -> Result<(), Error> {
    if let Some(value) = value {
        finite(value, label)?;
        if value < 0.0 {
            return Err(invalid(format!("{label} must be nonnegative")));
        }
    }
    Ok(())
}

fn optional_finite(value: Option<f64>, label: &str) -> Result<(), Error> {
    value.map_or(Ok(()), |value| finite(value, label))
}

fn validate_occurrence_derived_coordinates(
    occurrence: &AuthoredFootprintOccurrence,
) -> Result<(), Error> {
    let (sin, cos) = occurrence.angle_degrees.to_radians().sin_cos();
    for (cache, anchor) in occurrence
        .footprint
        .properties
        .iter()
        .filter_map(|value| value.render_cache.as_ref().map(|cache| (cache, value.at)))
        .chain(
            occurrence
                .footprint
                .texts
                .iter()
                .filter_map(|value| value.render_cache.as_ref().map(|cache| (cache, value.at))),
        )
    {
        let board_anchor_x = occurrence.at.x_mm + anchor.x_mm * cos + anchor.y_mm * sin;
        let board_anchor_y = occurrence.at.y_mm - anchor.x_mm * sin + anchor.y_mm * cos;
        let delta_x = board_anchor_x - anchor.x_mm;
        let delta_y = board_anchor_y - anchor.y_mm;
        finite(delta_x, "embedded-footprint render-cache anchor delta X")?;
        finite(delta_y, "embedded-footprint render-cache anchor delta Y")?;
        for point in cache
            .polygons
            .iter()
            .flat_map(|polygon| &polygon.contours)
            .flat_map(|contour| &contour.points)
        {
            finite(point.x + delta_x, "embedded-footprint render-cache board X")?;
            finite(point.y + delta_y, "embedded-footprint render-cache board Y")?;
        }
    }
    for point in occurrence
        .footprint
        .text_boxes
        .iter()
        .filter_map(|value| value.render_cache.as_ref())
        .flat_map(|cache| &cache.polygons)
        .flat_map(|polygon| &polygon.contours)
        .flat_map(|contour| &contour.points)
    {
        let board_x = occurrence.at.x_mm + point.x * cos + point.y * sin;
        let board_y = occurrence.at.y_mm - point.x * sin + point.y * cos;
        finite(board_x, "embedded-footprint text-box cache board X")?;
        finite(board_y, "embedded-footprint text-box cache board Y")?;
    }
    Ok(())
}

fn footprint_cache_angle(angle_degrees: f64, unlocked: bool) -> f64 {
    if unlocked {
        return angle_degrees;
    }
    let mut angle = angle_degrees.rem_euclid(360.0);
    while angle > 90.0 {
        angle -= 180.0;
    }
    while angle <= -90.0 {
        angle += 180.0;
    }
    angle
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::build(ErrorKind::InvalidBuildValue, message.into())
}

pub(super) fn limit() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "authored PCB/footprint values exceed their resource limits",
    )
}

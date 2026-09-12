use super::*;
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
    for graphic in &value.profile {
        if graphic.layer != "Edge.Cuts" {
            return Err(invalid("board profile graphics must use Edge.Cuts"));
        }
        state.graphic(graphic, Some(&layers))?;
    }
    for graphic in &value.graphics {
        state.graphic(graphic, Some(&layers))?;
    }
    for text in &value.texts {
        state.board_text(text, &layers)?;
    }
    for text_box in &value.text_boxes {
        state.text_box(text_box, Some(&layers), None, None)?;
    }
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
        if !self.identities.insert(value.to_owned()) {
            return Err(invalid(format!("duplicate KiCad UUID {value:?}")));
        }
        Ok(())
    }

    fn layers(&mut self, layers: &[AuthoredLayer]) -> Result<BTreeMap<String, String>, Error> {
        let mut ordinals = BTreeSet::new();
        let mut names = BTreeMap::new();
        for layer in layers {
            self.object()?;
            self.text(&layer.name)?;
            self.text(&layer.kind)?;
            if let Some(user_name) = &layer.user_name {
                self.text(user_name)?;
            }
            if layer.name.is_empty() || layer.kind.is_empty() {
                return Err(invalid("layer name and kind must be nonempty"));
            }
            supported_token(
                &layer.kind,
                "layer kind",
                &["signal", "power", "mixed", "jumper", "user"],
            )?;
            validate_layer_slot(layer)?;
            if !ordinals.insert(layer.ordinal)
                || names
                    .insert(layer.name.clone(), layer.kind.clone())
                    .is_some()
            {
                return Err(invalid("layer ordinals and names must be unique"));
            }
        }
        if !names.contains_key("F.Cu") || !names.contains_key("B.Cu") {
            return Err(invalid("authored boards require F.Cu and B.Cu layers"));
        }
        let copper_count = names
            .values()
            .filter(|kind| kind.as_str() != "user")
            .count();
        if copper_count % 2 != 0 {
            return Err(invalid(
                "authored boards require an even number of declared copper layers",
            ));
        }
        let mut copper_slots = names
            .iter()
            .filter(|(_, kind)| kind.as_str() != "user")
            .filter_map(|(name, _)| canonical_layer_ordinal(name))
            .collect::<Vec<_>>();
        copper_slots.sort_unstable();
        let last_inner = copper_count.saturating_sub(2);
        let expected_slots = std::iter::once(0)
            .chain((1..=last_inner).map(|slot| i64::try_from(slot).unwrap_or(i64::MAX)))
            .chain(std::iter::once(31))
            .collect::<Vec<_>>();
        if copper_slots != expected_slots {
            return Err(invalid(
                "authored inner copper layers must be contiguous from In1.Cu",
            ));
        }
        Ok(names)
    }

    fn setup(
        &mut self,
        setup: &AuthoredSetup,
        board_layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.point(setup.aux_axis_origin)?;
        self.point(setup.grid_origin)?;
        finite(
            setup.pad_to_mask_clearance_mm,
            "board pad-to-mask clearance",
        )?;
        finite(
            setup.pad_to_paste_clearance_mm,
            "board pad-to-paste clearance",
        )?;
        finite(
            setup.pad_to_paste_clearance_ratio,
            "board pad-to-paste clearance ratio",
        )?;
        let Some(stackup) = &setup.stackup else {
            return Ok(());
        };
        for layer in &stackup.layers {
            self.object()?;
            self.text(&layer.name)?;
            self.text(&layer.type_name)?;
            if layer.name.is_empty() || layer.type_name.is_empty() {
                return Err(invalid("stackup layer name and type must be nonempty"));
            }
            if layer.type_name == "copper" {
                let kind = board_layers.get(&layer.name).map(String::as_str);
                if !matches!(kind, Some("signal" | "power" | "mixed" | "jumper")) {
                    return Err(invalid(format!(
                        "copper stackup layer {:?} is absent from the board copper layer table",
                        layer.name
                    )));
                }
            }
            if layer.thickness_locked && layer.thickness_mm.is_none() {
                return Err(invalid(
                    "stackup thickness cannot be locked when thickness is absent",
                ));
            }
            optional_nonnegative(layer.thickness_mm, "stackup thickness")?;
            optional_finite(layer.epsilon_r, "stackup epsilon_r")?;
            optional_finite(layer.loss_tangent, "stackup loss tangent")?;
            for value in [&layer.material, &layer.color].into_iter().flatten() {
                self.text(value)?;
            }
        }
        for value in [&stackup.copper_finish, &stackup.edge_connector]
            .into_iter()
            .flatten()
        {
            self.text(value)?;
        }
        if let Some(edge_connector) = &stackup.edge_connector {
            supported_token(
                edge_connector,
                "stackup edge connector",
                &["yes", "bevelled"],
            )?;
        }
        Ok(())
    }

    fn nets(&mut self, nets: &[AuthoredNet]) -> Result<BTreeMap<i64, String>, Error> {
        let mut by_code = BTreeMap::new();
        let mut names = BTreeSet::new();
        for net in nets {
            self.object()?;
            self.text(&net.name)?;
            if net.code <= 0 || net.name.is_empty() {
                return Err(invalid(
                    "authored nets require a positive code and nonempty name",
                ));
            }
            if by_code.insert(net.code, net.name.clone()).is_some() || !names.insert(&net.name) {
                return Err(invalid("net codes and names must be unique"));
            }
        }
        Ok(by_code)
    }

    fn footprint(
        &mut self,
        footprint: &AuthoredFootprint,
        board_nets: Option<&BTreeMap<i64, String>>,
        board_layers: Option<&BTreeMap<String, String>>,
        occurrence: Option<&AuthoredFootprintOccurrence>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&footprint.name)?;
        if footprint.name.is_empty() {
            return Err(invalid("footprint name must be nonempty"));
        }
        for value in [&footprint.description, &footprint.tags]
            .into_iter()
            .flatten()
        {
            self.text(value)?;
        }
        for attribute in &footprint.attributes {
            self.object()?;
            self.text(attribute)?;
            supported_token(
                attribute,
                "footprint attribute",
                &[
                    "through_hole",
                    "smd",
                    "board_only",
                    "exclude_from_pos_files",
                    "exclude_from_bom",
                ],
            )?;
        }
        optional_finite(
            footprint.solder_mask_margin_mm,
            "footprint solder-mask margin",
        )?;
        optional_finite(
            footprint.solder_paste_margin_mm,
            "footprint solder-paste margin",
        )?;
        optional_finite(
            footprint.solder_paste_margin_ratio,
            "footprint solder-paste margin ratio",
        )?;
        let mut property_names = BTreeSet::new();
        let variables = BoardTextVariables::from_entries(
            footprint
                .properties
                .iter()
                .map(|property| (&property.name, &property.value)),
        );
        for property in &footprint.properties {
            if !property_names.insert(property.name.as_str()) {
                return Err(invalid(format!(
                    "duplicate footprint property name {:?} in one owner",
                    property.name
                )));
            }
            self.footprint_property(property, board_layers, &variables)?;
        }
        for text in &footprint.texts {
            self.footprint_text(text, board_layers, &variables)?;
        }
        for text_box in &footprint.text_boxes {
            if board_layers.is_none() && text_box.locked {
                return Err(invalid(
                    "standalone footprint text-box locking is discarded by KiCad 10",
                ));
            }
            self.text_box(text_box, board_layers, Some(&variables), occurrence)?;
        }
        for graphic in &footprint.graphics {
            self.graphic(graphic, board_layers)?;
        }
        for pad in &footprint.pads {
            self.pad(pad, board_nets, board_layers)?;
        }
        for model in &footprint.models {
            self.model(model)?;
        }
        self.resources(&footprint.embedded_files)
    }

    fn occurrence(
        &mut self,
        occurrence: &AuthoredFootprintOccurrence,
        board_nets: &BTreeMap<i64, String>,
        board_layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&occurrence.layer)?;
        require_footprint_root_layer(&occurrence.layer, Some(board_layers))?;
        self.point(occurrence.at)?;
        finite(occurrence.angle_degrees, "footprint occurrence angle")?;
        if occurrence.angle_degrees.rem_euclid(360.0) != 0.0
            && !occurrence.footprint.text_boxes.is_empty()
        {
            return Err(invalid(
                "rotated embedded-footprint text boxes are rejected because KiCad 10 corrupts both polygon and start/end geometry across saves",
            ));
        }
        self.identity(&occurrence.uuid)?;
        self.footprint(
            &occurrence.footprint,
            Some(board_nets),
            Some(board_layers),
            Some(occurrence),
        )?;
        validate_occurrence_derived_coordinates(occurrence)
    }

    fn footprint_property(
        &mut self,
        value: &AuthoredFootprintProperty,
        board_layers: Option<&BTreeMap<String, String>>,
        variables: &BoardTextVariables,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&value.name)?;
        self.text(&value.value)?;
        self.text(&value.layer)?;
        require_layer(&value.layer, board_layers, false)?;
        self.point(value.at)?;
        finite(value.angle_degrees, "property angle")?;
        self.effects(&value.effects)?;
        let resolved = value
            .render_cache
            .as_ref()
            .map(|_| self.resolved_cache_text(&value.value, variables, false))
            .transpose()?
            .flatten();
        self.render_cache(
            value.render_cache.as_ref(),
            resolved.as_deref().unwrap_or(&value.value),
            footprint_cache_angle(value.angle_degrees, value.unlocked),
            &value.effects,
        )?;
        self.identity(&value.uuid)
    }

    fn footprint_text(
        &mut self,
        value: &AuthoredFootprintText,
        board_layers: Option<&BTreeMap<String, String>>,
        variables: &BoardTextVariables,
    ) -> Result<(), Error> {
        if value.hidden {
            return Err(invalid(
                "hidden user footprint text is rewritten as a property by KiCad 10",
            ));
        }
        self.object()?;
        self.text(&value.text)?;
        self.text(&value.layer)?;
        require_layer(&value.layer, board_layers, false)?;
        self.point(value.at)?;
        finite(value.angle_degrees, "text angle")?;
        self.effects(&value.effects)?;
        let resolved = value
            .render_cache
            .as_ref()
            .map(|_| self.resolved_cache_text(&value.text, variables, false))
            .transpose()?
            .flatten();
        self.render_cache(
            value.render_cache.as_ref(),
            resolved.as_deref().unwrap_or(&value.text),
            footprint_cache_angle(value.angle_degrees, value.unlocked),
            &value.effects,
        )?;
        self.identity(&value.uuid)
    }

    fn effects(&mut self, value: &AuthoredTextEffects) -> Result<(), Error> {
        if let Some(face) = &value.face {
            self.text(face)?;
            if face.is_empty() {
                return Err(invalid(
                    "authored text font face must be nonempty when present",
                ));
            }
        }
        positive(value.size_x_mm, "text size X")?;
        positive(value.size_y_mm, "text size Y")?;
        optional_nonnegative(value.thickness_mm, "text thickness")?;
        optional_nonnegative(value.line_spacing, "text line spacing")?;
        if value.color.is_some() {
            return Err(invalid(
                "authored text color is unsupported by the 20241229 PCB/footprint grammar",
            ));
        }
        if value.href.is_some() {
            return Err(invalid(
                "authored text hyperlinks are unsupported by the 20241229 PCB/footprint grammar",
            ));
        }
        Ok(())
    }

    fn board_text(
        &mut self,
        value: &AuthoredBoardText,
        board_layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&value.text)?;
        self.text(&value.layer)?;
        require_layer(&value.layer, Some(board_layers), false)?;
        self.point(value.at)?;
        finite(value.angle_degrees, "board text angle")?;
        self.effects(&value.effects)?;
        if value.render_cache.is_some() && value.text.contains("${") {
            return Err(invalid(
                "board text render caches with unresolved project variables require companion context",
            ));
        }
        self.render_cache(
            value.render_cache.as_ref(),
            &value.text,
            value.angle_degrees,
            &value.effects,
        )?;
        self.identity(&value.uuid)
    }

    fn text_box(
        &mut self,
        value: &AuthoredTextBox,
        board_layers: Option<&BTreeMap<String, String>>,
        variables: Option<&BoardTextVariables>,
        _occurrence: Option<&AuthoredFootprintOccurrence>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&value.text)?;
        self.text(&value.layer)?;
        self.text(&value.stroke_kind)?;
        require_layer(&value.layer, board_layers, false)?;
        match &value.geometry {
            AuthoredTextBoxGeometry::Rectangle { start, end } => {
                self.point(*start)?;
                self.point(*end)?;
                if start == end {
                    return Err(invalid("text boxes require distinct start and end points"));
                }
            }
            AuthoredTextBoxGeometry::Polygon { points } => {
                if points.len() < 3 {
                    return Err(invalid(
                        "polygon text boxes require at least three source points",
                    ));
                }
                for point in points {
                    self.point(*point)?;
                }
            }
        }
        for margin in value.margins_mm {
            nonnegative(margin, "text box margin")?;
        }
        finite(value.angle_degrees, "text box angle")?;
        positive(value.stroke_width_mm, "text box stroke width")?;
        supported_token(
            &value.stroke_kind,
            "text box stroke kind",
            &[
                "default",
                "solid",
                "dash",
                "dot",
                "dash_dot",
                "dash_dot_dot",
            ],
        )?;
        self.effects(&value.effects)?;
        let resolved = match (value.render_cache.as_ref(), variables) {
            (None, _) => None,
            (Some(_), Some(variables)) => self.resolved_cache_text(&value.text, variables, true)?,
            (Some(_), None) if value.text.contains("${") => {
                return Err(invalid(
                    "board text-box render caches with unresolved project variables require companion context",
                ));
            }
            (Some(_), None) => None,
        };
        self.render_cache(
            value.render_cache.as_ref(),
            resolved.as_deref().unwrap_or(&value.text),
            value.angle_degrees,
            &value.effects,
        )?;
        self.identity(&value.uuid)
    }

    fn resolved_cache_text(
        &self,
        raw: &str,
        variables: &BoardTextVariables,
        text_box: bool,
    ) -> Result<Option<String>, Error> {
        if !raw.contains("${") && !text_box {
            return Ok(None);
        }
        let resolved = variables.substitute_bounded(raw, self.limits.max_string_bytes)?;
        if resolved.contains("${") {
            return Err(invalid(
                "authored render caches cannot retain unresolved text variables",
            ));
        }
        if text_box && resolved.chars().any(char::is_whitespace) {
            return Err(invalid(
                "text-box render-cache wrapping requires pre-realized outline-font context",
            ));
        }
        Ok(Some(resolved))
    }

    fn render_cache(
        &mut self,
        value: Option<&TextRenderCache>,
        authored_text: &str,
        authored_angle: f64,
        effects: &AuthoredTextEffects,
    ) -> Result<(), Error> {
        let Some(cache) = value else {
            return Ok(());
        };
        if effects.face.is_none() {
            return Err(invalid(
                "authored render caches require a named source font face; Newstroke text has no TTF cache",
            ));
        }
        if cache.text != authored_text || cache.angle_degrees != authored_angle {
            return Err(invalid(
                "authored render-cache text and angle must match the owning text carrier",
            ));
        }
        self.object()?;
        self.text(&cache.text)?;
        finite(cache.angle_degrees, "render-cache angle")?;
        for polygon in &cache.polygons {
            self.object()?;
            for contour in &polygon.contours {
                self.object()?;
                for point in &contour.points {
                    self.point(AuthoredPoint {
                        x_mm: point.x,
                        y_mm: point.y,
                    })?;
                }
            }
        }
        Ok(())
    }

    fn graphic(
        &mut self,
        graphic: &AuthoredGraphic,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&graphic.layer)?;
        require_layer(&graphic.layer, board_layers, false)?;
        self.text(&graphic.stroke_kind)?;
        supported_token(
            &graphic.stroke_kind,
            "graphic stroke kind",
            &[
                "default",
                "solid",
                "dash",
                "dot",
                "dash_dot",
                "dash_dot_dot",
            ],
        )?;
        positive(graphic.stroke_width_mm, "graphic stroke width")?;
        if let Some(fill) = &graphic.fill {
            self.text(fill)?;
            supported_token(fill, "graphic fill", &["none", "solid"])?;
            if matches!(
                graphic.geometry,
                AuthoredGraphicGeometry::Line { .. } | AuthoredGraphicGeometry::Arc { .. }
            ) {
                return Err(invalid(
                    "line and arc graphics cannot author fill because KiCad discards it",
                ));
            }
        }
        self.identity(&graphic.uuid)?;
        match &graphic.geometry {
            AuthoredGraphicGeometry::Line { start, end }
            | AuthoredGraphicGeometry::Rect { start, end } => {
                self.point(*start)?;
                self.point(*end)?;
            }
            AuthoredGraphicGeometry::Arc { start, mid, end } => {
                self.point(*start)?;
                self.point(*mid)?;
                self.point(*end)?;
            }
            AuthoredGraphicGeometry::Circle { center, end } => {
                self.point(*center)?;
                self.point(*end)?;
            }
            AuthoredGraphicGeometry::Polygon { points } => {
                if points.len() < 3 {
                    return Err(invalid("graphic polygons require at least three points"));
                }
                for point in points {
                    self.point(*point)?;
                }
            }
        }
        Ok(())
    }

    fn pad(
        &mut self,
        pad: &AuthoredPad,
        board_nets: Option<&BTreeMap<i64, String>>,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&pad.number)?;
        for layer in &pad.layers {
            self.object()?;
            self.text(layer)?;
            require_layer(layer, board_layers, true)?;
        }
        if pad.layers.is_empty() {
            return Err(invalid("pads require at least one layer"));
        }
        require_unique_strings(&pad.layers, "pad layers")?;
        self.point(pad.at)?;
        finite(pad.angle_degrees, "pad angle")?;
        positive(pad.size_x_mm, "pad size X")?;
        positive(pad.size_y_mm, "pad size Y")?;
        self.identity(&pad.uuid)?;
        self.pad_drill(pad)?;
        if let Some(net) = &pad.net {
            if pad.kind == AuthoredPadKind::NonPlatedThroughHole {
                return Err(invalid(
                    "non-plated through-hole pads cannot author net associations",
                ));
            }
            self.text(&net.name)?;
            let Some(board_nets) = board_nets else {
                return Err(invalid(
                    "standalone footprint pads cannot author board net associations",
                ));
            };
            if board_nets.get(&net.code) != Some(&net.name) {
                return Err(invalid(
                    "pad net reference does not match the authored board net",
                ));
            }
        }
        self.pad_policies(pad, board_layers)?;
        self.pad_shape(&pad.shape)
    }

    fn pad_policies(
        &mut self,
        pad: &AuthoredPad,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
        optional_finite(pad.solder_mask_margin_mm, "pad mask margin")?;
        optional_finite(pad.solder_paste_margin_mm, "pad paste margin")?;
        optional_finite(pad.solder_paste_margin_ratio, "pad paste margin ratio")?;
        optional_nonnegative(pad.clearance_mm, "pad clearance")?;
        optional_nonnegative(pad.thermal_bridge_width_mm, "pad thermal bridge width")?;
        optional_finite(pad.thermal_bridge_angle_degrees, "pad thermal bridge angle")?;
        optional_nonnegative(pad.thermal_gap_mm, "pad thermal gap")?;
        for layer in &pad.zone_layer_connections {
            self.object()?;
            self.text(layer)?;
            require_copper_layer(layer, board_layers)?;
        }
        require_unique_strings(&pad.zone_layer_connections, "pad zone_layer_connections")?;
        if !pad.zone_layer_connections.is_empty() && pad.kind != AuthoredPadKind::ThroughHole {
            return Err(invalid(
                "zone_layer_connections are supported only on plated through-hole pads",
            ));
        }
        if !pad.zone_layer_connections.is_empty() && pad.remove_unused_layers != Some(true) {
            return Err(invalid(
                "pad zone_layer_connections require remove_unused_layers=yes",
            ));
        }
        if pad.kind != AuthoredPadKind::ThroughHole
            && (pad.remove_unused_layers.is_some() || pad.keep_end_layers.is_some())
        {
            return Err(invalid(
                "remove_unused_layers and keep_end_layers are supported only on plated through-hole pads",
            ));
        }
        if pad.keep_end_layers.is_some() && pad.remove_unused_layers != Some(true) {
            return Err(invalid(
                "pad keep_end_layers requires remove_unused_layers=yes",
            ));
        }
        Ok(())
    }

    fn pad_shape(&mut self, shape: &AuthoredPadShape) -> Result<(), Error> {
        match shape {
            AuthoredPadShape::Trapezoid {
                delta_x_mm,
                delta_y_mm,
            } => {
                finite(*delta_x_mm, "trapezoid delta X")?;
                finite(*delta_y_mm, "trapezoid delta Y")?;
            }
            AuthoredPadShape::RoundRect { radius_ratio } => {
                roundrect_ratio(*radius_ratio)?;
            }
            AuthoredPadShape::ChamferedRoundRect {
                radius_ratio,
                chamfer_ratio,
                corners,
            } => {
                roundrect_ratio(*radius_ratio)?;
                finite(*chamfer_ratio, "pad chamfer ratio")?;
                if !(0.0..=0.5).contains(chamfer_ratio) {
                    return Err(invalid("pad chamfer ratio must be between 0 and 0.5"));
                }
                if corners.is_empty() {
                    return Err(invalid(
                        "chamfered roundrect pads require at least one chamfer corner",
                    ));
                }
                let mut unique = BTreeSet::new();
                for corner in corners {
                    self.object()?;
                    if !unique.insert(*corner) {
                        return Err(invalid("pad chamfer corners must be unique"));
                    }
                }
            }
            AuthoredPadShape::CustomPolygon { points } => {
                if points.len() < 3 {
                    return Err(invalid("custom pad polygons require at least three points"));
                }
                for point in points {
                    self.point(*point)?;
                }
            }
            AuthoredPadShape::Circle | AuthoredPadShape::Oval | AuthoredPadShape::Rect => {}
        }
        Ok(())
    }

    fn pad_drill(&mut self, pad: &AuthoredPad) -> Result<(), Error> {
        match (pad.kind, &pad.drill) {
            (AuthoredPadKind::Smd, Some(_)) => return Err(invalid("SMD pads cannot have drills")),
            (AuthoredPadKind::ThroughHole | AuthoredPadKind::NonPlatedThroughHole, None) => {
                return Err(invalid("through-hole pads require a drill"));
            }
            _ => {}
        }
        if let Some(drill) = &pad.drill {
            positive(drill.width_mm, "drill width")?;
            optional_positive(drill.height_mm, "drill height")?;
            self.point(drill.offset)?;
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

    fn via(
        &mut self,
        value: &AuthoredVia,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.point(value.at)?;
        positive(value.size_mm, "via size")?;
        positive(value.drill_mm, "via drill")?;
        if value.drill_mm > value.size_mm {
            return Err(invalid("via drill cannot exceed via size"));
        }
        for layer in [&value.start_layer, &value.end_layer] {
            self.text(layer)?;
            require_copper_layer(layer, Some(layers))?;
        }
        let copper_order = declared_copper_order(layers);
        let start = copper_order
            .iter()
            .position(|layer| layer == &value.start_layer)
            .ok_or_else(|| invalid("via start layer is absent from copper order"))?;
        let end = copper_order
            .iter()
            .position(|layer| layer == &value.end_layer)
            .ok_or_else(|| invalid("via end layer is absent from copper order"))?;
        if start >= end {
            return Err(invalid(
                "via layer spans must run from front toward back across distinct copper layers",
            ));
        }
        match value.kind {
            AuthoredViaKind::Through
                if value.start_layer != "F.Cu" || value.end_layer != "B.Cu" =>
            {
                return Err(invalid("through vias must span F.Cu to B.Cu"));
            }
            AuthoredViaKind::BlindBuried
                if value.start_layer == "F.Cu" && value.end_layer == "B.Cu" =>
            {
                return Err(invalid(
                    "blind/buried vias must not span the complete copper stack",
                ));
            }
            AuthoredViaKind::Micro if end != start + 1 => {
                return Err(invalid(
                    "microvias must span adjacent declared copper layers",
                ));
            }
            _ => {}
        }
        if value.net_code != 0 {
            require_net(value.net_code, nets)?;
        }
        self.identity(&value.uuid)?;
        for (name, policy) in [
            ("tenting", &value.tenting),
            ("covering", &value.covering),
            ("plugging", &value.plugging),
        ] {
            if let Some(policy) = policy {
                self.object()?;
                if policy.front.is_none() && policy.back.is_none() {
                    return Err(invalid(format!(
                        "via {name} policy requires a front or back value"
                    )));
                }
            }
        }
        for layer in &value.zone_layer_connections {
            self.object()?;
            self.text(layer)?;
            require_copper_layer(layer, Some(layers))?;
        }
        require_unique_strings(&value.zone_layer_connections, "via zone_layer_connections")?;
        if !value.zone_layer_connections.is_empty() && value.remove_unused_layers != Some(true) {
            return Err(invalid(
                "via zone_layer_connections require remove_unused_layers=yes",
            ));
        }
        if value.keep_end_layers.is_some() && value.remove_unused_layers != Some(true) {
            return Err(invalid(
                "via keep_end_layers requires remove_unused_layers=yes",
            ));
        }
        if value.remove_unused_layers == Some(false) {
            return Err(invalid(
                "via remove_unused_layers=no is not supported because KiCad canonicalizes it to absence",
            ));
        }
        if value.start_end_only == Some(false) {
            return Err(invalid(
                "via start_end_only=no is not supported because KiCad canonicalizes it to absence",
            ));
        }
        if value.start_end_only == Some(true)
            && (value.remove_unused_layers.is_some() || value.keep_end_layers.is_some())
        {
            return Err(invalid(
                "via start_end_only=yes is an alternative to remove_unused_layers/keep_end_layers",
            ));
        }
        Ok(())
    }

    fn segment(
        &mut self,
        value: &AuthoredSegment,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.point(value.start)?;
        self.point(value.end)?;
        positive(value.width_mm, "segment width")?;
        self.text(&value.layer)?;
        require_copper_layer(&value.layer, Some(layers))?;
        if value.net_code != 0 {
            require_net(value.net_code, nets)?;
        }
        self.identity(&value.uuid)
    }

    fn routing_arc(
        &mut self,
        value: &AuthoredRoutingArc,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.point(value.start)?;
        self.point(value.mid)?;
        self.point(value.end)?;
        positive(value.width_mm, "routing arc width")?;
        self.text(&value.layer)?;
        require_copper_layer(&value.layer, Some(layers))?;
        if value.net_code != 0 {
            require_net(value.net_code, nets)?;
        }
        self.identity(&value.uuid)
    }

    fn zone(
        &mut self,
        value: &AuthoredZone,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        if value.net.code == 0 {
            if !value.net.name.is_empty() {
                return Err(invalid("zone net code 0 requires an empty net name"));
            }
        } else if nets.get(&value.net.code) != Some(&value.net.name) {
            return Err(invalid(
                "zone net reference does not match the authored board net",
            ));
        }
        self.text(&value.net.name)?;
        if value.layers.is_empty() {
            return Err(invalid("zones require at least one copper layer"));
        }
        for layer in &value.layers {
            self.object()?;
            self.text(layer)?;
            require_copper_layer(layer, Some(layers))?;
        }
        require_unique_strings(&value.layers, "zone layers")?;
        self.identity(&value.uuid)?;
        if let Some(name) = &value.name {
            self.text(name)?;
            if name.is_empty() {
                return Err(invalid("authored zone names must be nonempty when present"));
            }
        }
        positive(value.hatch_pitch_mm, "zone hatch pitch")?;
        if value.priority < 0 {
            return Err(invalid("zone priority must be nonnegative"));
        }
        nonnegative(value.connect_pads_clearance_mm, "zone pad clearance")?;
        positive(value.min_thickness_mm, "zone minimum thickness")?;
        if value.filled_areas_thickness == Some(true) {
            return Err(invalid(
                "filled_areas_thickness=yes is not supported by the 20241229 zone grammar",
            ));
        }
        if let Some(fill) = &value.fill {
            positive(fill.thermal_gap_mm, "zone thermal gap")?;
            positive(fill.thermal_bridge_width_mm, "zone thermal bridge width")?;
            if let Some(mode) = fill.island_removal_mode
                && !(0..=2).contains(&mode)
            {
                return Err(invalid("zone island removal mode must be 0, 1, or 2"));
            }
            optional_nonnegative(fill.island_area_min_mm2, "zone island minimum area")?;
            if fill.island_area_min_mm2.is_some() && fill.island_removal_mode != Some(2) {
                return Err(invalid(
                    "zone island_area_min requires island_removal_mode=2",
                ));
            }
        }
        if value.outlines.is_empty() {
            return Err(invalid("zones require at least one authored outline ring"));
        }
        for polygon in &value.outlines {
            self.zone_points(&polygon.points, "zone outline")?;
        }
        if !value.filled_polygons.is_empty() && value.fill.is_none() {
            return Err(invalid(
                "retained zone filled polygons require enabled fill settings",
            ));
        }
        for polygon in &value.filled_polygons {
            self.object()?;
            self.text(&polygon.layer)?;
            require_copper_layer(&polygon.layer, Some(layers))?;
            if !value.layers.contains(&polygon.layer) {
                return Err(invalid(
                    "filled polygon layer must be included in its zone layers",
                ));
            }
            self.zone_points(&polygon.points, "zone filled polygon")?;
        }
        Ok(())
    }

    fn zone_points(&mut self, points: &[AuthoredPoint], label: &str) -> Result<(), Error> {
        self.object()?;
        if points.len() < 3 {
            return Err(invalid(format!("{label} requires at least three points")));
        }
        for point in points {
            self.point(*point)?;
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
        || canonical_layer_ordinal(layer).is_some_and(|ordinal| ordinal <= 31),
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
    copper.sort_by_key(|name| canonical_layer_ordinal(name).unwrap_or(i64::MAX));
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
    let copper = layer.ordinal <= 31;
    if copper == (layer.kind == "user") {
        return Err(invalid(format!(
            "layer {:?} requires a {} kind",
            layer.name,
            if copper { "copper" } else { "user" }
        )));
    }
    Ok(())
}

fn canonical_layer_ordinal(name: &str) -> Option<i64> {
    match name {
        "F.Cu" => Some(0),
        "B.Cu" => Some(31),
        "B.Adhes" => Some(32),
        "F.Adhes" => Some(33),
        "B.Paste" => Some(34),
        "F.Paste" => Some(35),
        "B.SilkS" => Some(36),
        "F.SilkS" => Some(37),
        "B.Mask" => Some(38),
        "F.Mask" => Some(39),
        "Dwgs.User" => Some(40),
        "Cmts.User" => Some(41),
        "Eco1.User" => Some(42),
        "Eco2.User" => Some(43),
        "Edge.Cuts" => Some(44),
        "Margin" => Some(45),
        "B.CrtYd" => Some(46),
        "F.CrtYd" => Some(47),
        "B.Fab" => Some(48),
        "F.Fab" => Some(49),
        _ => name
            .strip_prefix("In")
            .and_then(|value| value.strip_suffix(".Cu"))
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|ordinal| (1..=30).contains(ordinal))
            .or_else(|| {
                name.strip_prefix("User.")
                    .and_then(|value| value.parse::<i64>().ok())
                    .filter(|ordinal| (1..=9).contains(ordinal))
                    .map(|ordinal| ordinal + 49)
            }),
    }
}

fn canonical_layer_name(ordinal: i64) -> Option<String> {
    Some(match ordinal {
        0 => "F.Cu".to_owned(),
        1..=30 => format!("In{ordinal}.Cu"),
        31 => "B.Cu".to_owned(),
        32 => "B.Adhes".to_owned(),
        33 => "F.Adhes".to_owned(),
        34 => "B.Paste".to_owned(),
        35 => "F.Paste".to_owned(),
        36 => "B.SilkS".to_owned(),
        37 => "F.SilkS".to_owned(),
        38 => "B.Mask".to_owned(),
        39 => "F.Mask".to_owned(),
        40 => "Dwgs.User".to_owned(),
        41 => "Cmts.User".to_owned(),
        42 => "Eco1.User".to_owned(),
        43 => "Eco2.User".to_owned(),
        44 => "Edge.Cuts".to_owned(),
        45 => "Margin".to_owned(),
        46 => "B.CrtYd".to_owned(),
        47 => "F.CrtYd".to_owned(),
        48 => "B.Fab".to_owned(),
        49 => "F.Fab".to_owned(),
        50..=58 => format!("User.{}", ordinal - 49),
        _ => return None,
    })
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

use super::*;

impl Validation {
    pub(super) fn pad(
        &mut self,
        pad: &AuthoredPad,
        board_nets: Option<&BTreeMap<i64, String>>,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&pad.number)?;
        self.pad_metadata(pad, board_nets.is_some())?;
        for layer in &pad.layers {
            self.object()?;
            self.text(layer)?;
            require_layer(layer, board_layers, true)?;
        }
        if pad.layers.is_empty() && pad.kind != AuthoredPadKind::NonPlatedThroughHole {
            return Err(invalid(
                "only non-plated through-hole pads may have an empty layer list",
            ));
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
        if let Some(stack) = &pad.padstack {
            self.padstack(stack, board_layers)?;
        }
        self.pad_shape(&pad.shape)
    }

    pub(super) fn pad_metadata(
        &mut self,
        pad: &AuthoredPad,
        board_owned: bool,
    ) -> Result<(), Error> {
        for text in [&pad.pin_function, &pad.pin_type].into_iter().flatten() {
            self.text(text)?;
            if !board_owned || text.is_empty() {
                return Err(invalid(
                    "KiCad discards empty or standalone footprint pin metadata",
                ));
            }
        }
        optional_finite(pad.die_length_mm, "pad-to-die length")?;
        if pad.die_length_mm == Some(0.0) {
            return Err(invalid(
                "explicit zero pad-to-die length is omitted by KiCad",
            ));
        }
        Ok(())
    }

    pub(super) fn pad_policies(
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
        self.pad_land_presence(pad, board_layers)
    }

    fn pad_land_presence(
        &mut self,
        pad: &AuthoredPad,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
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
        Self::pad_end_layer_policy(pad)
    }

    fn pad_end_layer_policy(pad: &AuthoredPad) -> Result<(), Error> {
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

    pub(super) fn pad_shape(&mut self, shape: &AuthoredPadShape) -> Result<(), Error> {
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
                self.pad_chamfer(*chamfer_ratio, corners)?;
            }
            AuthoredPadShape::CustomPolygon { points } => {
                self.custom_polygon_count(points.len())?;
                for point in points {
                    self.point(*point)?;
                }
            }
            AuthoredPadShape::Custom { primitives, .. } => {
                for primitive in primitives {
                    self.custom_primitive(primitive)?;
                }
            }
            AuthoredPadShape::Circle | AuthoredPadShape::Oval | AuthoredPadShape::Rect => {}
        }
        Ok(())
    }

    fn pad_chamfer(&mut self, ratio: f64, corners: &[AuthoredChamferCorner]) -> Result<(), Error> {
        finite(ratio, "pad chamfer ratio")?;
        if !(0.0..=0.5).contains(&ratio) {
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
        Ok(())
    }

    pub(super) fn pad_drill(&mut self, pad: &AuthoredPad) -> Result<(), Error> {
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
}

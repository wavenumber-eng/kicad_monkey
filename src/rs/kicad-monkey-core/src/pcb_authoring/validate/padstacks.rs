use super::*;

impl Validation {
    pub(super) fn padstack(
        &mut self,
        stack: &AuthoredPadstack,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
        self.object()?;
        self.stack_selectors(
            stack.mode,
            stack.layers.iter().map(AuthoredPadstackLayer::layer),
            board_layers,
        )?;
        for row in &stack.layers {
            self.object()?;
            if let AuthoredPadstackLayer::Land {
                shape,
                size_x_mm,
                size_y_mm,
                offset,
                clearance_mm,
                thermal_bridge_width_mm,
                thermal_gap_mm,
                thermal_bridge_angle_degrees,
                ..
            } = row
            {
                positive(*size_x_mm, "padstack layer size X")?;
                positive(*size_y_mm, "padstack layer size Y")?;
                self.point(*offset)?;
                self.pad_shape(shape)?;
                if matches!(
                    shape,
                    AuthoredPadShape::Custom {
                        clearance: Some(_),
                        ..
                    } | AuthoredPadShape::CustomPolygon { .. }
                ) {
                    return Err(invalid(
                        "custom clearance mode is pad-wide; layer Custom requires clearance=None",
                    ));
                }
                optional_nonnegative(*clearance_mm, "padstack clearance")?;
                optional_nonnegative(*thermal_bridge_width_mm, "padstack thermal width")?;
                optional_nonnegative(*thermal_gap_mm, "padstack thermal gap")?;
                if thermal_bridge_angle_degrees.is_some() {
                    return Err(invalid(
                        "KiCad 10.0.6 parses layer thermal_bridge_angle onto F.Cu, not the selected layer",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn viastack(
        &mut self,
        stack: &AuthoredViaStack,
        board_layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.stack_selectors(
            stack.mode,
            stack.layers.iter().map(|row| &row.layer),
            Some(board_layers),
        )?;
        for row in &stack.layers {
            self.object()?;
            positive(row.size_mm, "via-stack diameter")?;
        }
        Ok(())
    }

    fn stack_selectors<'a>(
        &mut self,
        mode: AuthoredPadstackMode,
        selectors: impl Iterator<Item = &'a AuthoredPadstackLayerSelector>,
        board_layers: Option<&BTreeMap<String, String>>,
    ) -> Result<(), Error> {
        let mut unique = BTreeSet::new();
        for selector in selectors {
            self.text(selector.as_str())?;
            if !unique.insert(selector.as_str()) {
                return Err(invalid("padstack layer selectors must be unique"));
            }
            match selector {
                AuthoredPadstackLayerSelector::Inner
                    if mode == AuthoredPadstackMode::FrontInnerBack => {}
                AuthoredPadstackLayerSelector::Inner => {
                    return Err(invalid("Inner requires front_inner_back mode"));
                }
                AuthoredPadstackLayerSelector::CopperLayer(layer) => {
                    require_copper_layer(layer, board_layers)?;
                    if layer == "F.Cu" {
                        return Err(invalid(
                            "F.Cu geometry belongs to the parent pad/via, not a padstack row",
                        ));
                    }
                    if mode == AuthoredPadstackMode::FrontInnerBack && layer != "B.Cu" {
                        return Err(invalid("front_inner_back rows select Inner or B.Cu"));
                    }
                }
            }
        }
        Ok(())
    }
}

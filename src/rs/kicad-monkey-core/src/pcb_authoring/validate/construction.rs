use super::*;

impl Validation {
    pub(super) fn layers(
        &mut self,
        layers: &[AuthoredLayer],
    ) -> Result<BTreeMap<String, String>, Error> {
        let mut ordinals = BTreeSet::new();
        let mut names = BTreeMap::new();
        for layer in layers {
            self.layer(layer)?;
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

    fn layer(&mut self, layer: &AuthoredLayer) -> Result<(), Error> {
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
        validate_layer_slot(layer)
    }

    pub(super) fn setup(
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
            self.stackup_layer(layer, board_layers)?;
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

    pub(super) fn nets(&mut self, nets: &[AuthoredNet]) -> Result<BTreeMap<i64, String>, Error> {
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

    fn stackup_layer(
        &mut self,
        layer: &AuthoredStackupLayer,
        board_layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
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
        Ok(())
    }
}

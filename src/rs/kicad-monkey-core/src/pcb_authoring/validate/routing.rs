use super::*;

impl Validation {
    pub(super) fn backdrill(
        &mut self,
        value: Option<&AuthoredBackdrill>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let Some(drill) = value else { return Ok(()) };
        self.object()?;
        positive(drill.size_mm, "backdrill size")?;
        for layer in [&drill.start_layer, &drill.end_layer] {
            self.text(layer)?;
            require_copper_layer(layer, Some(layers))?;
        }
        if drill.start_layer == drill.end_layer {
            return Err(invalid("backdrill endpoints must be distinct"));
        }
        Ok(())
    }

    pub(super) fn via(
        &mut self,
        value: &AuthoredVia,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.point(value.at)?;
        positive(value.size_mm, "via size")?;
        if let Some(stack) = &value.padstack {
            self.viastack(stack, layers)?;
        }
        positive(value.drill_mm, "via drill")?;
        if value.drill_mm > value.size_mm {
            return Err(invalid("via drill cannot exceed via size"));
        }
        self.backdrill(value.backdrill.as_ref(), layers)?;
        self.via_span(value, layers)?;
        if value.net_code != 0 {
            require_net(value.net_code, nets)?;
        }
        self.identity(&value.uuid)?;
        self.via_treatments(value)?;
        self.via_land_presence(value, layers)
    }

    fn via_span(
        &mut self,
        value: &AuthoredVia,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
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
        Ok(())
    }

    fn via_treatments(&mut self, value: &AuthoredVia) -> Result<(), Error> {
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
        Ok(())
    }

    fn via_land_presence(
        &mut self,
        value: &AuthoredVia,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
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
        Self::via_start_end_policy(value)
    }

    fn via_start_end_policy(value: &AuthoredVia) -> Result<(), Error> {
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

    pub(super) fn segment(
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

    pub(super) fn routing_arc(
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
}

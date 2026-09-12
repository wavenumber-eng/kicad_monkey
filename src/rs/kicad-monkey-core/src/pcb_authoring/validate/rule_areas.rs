use super::*;

impl Validation {
    pub(super) fn rule_area(
        &mut self,
        value: &AuthoredRuleArea,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.object()?;
        self.identity(&value.uuid)?;
        if value.layers.is_empty() {
            return Err(invalid("rule areas require at least one layer"));
        }
        for layer in &value.layers {
            self.object()?;
            self.text(layer)?;
            require_layer(layer, Some(layers), false)?;
        }
        require_unique_strings(&value.layers, "rule area layers")?;
        if let Some(name) = &value.name {
            self.text(name)?;
            if name.is_empty() {
                return Err(invalid("rule area names must be nonempty when present"));
            }
        }
        positive(value.hatch_pitch_mm, "rule area hatch pitch")?;
        if let Some(placement) = &value.placement {
            self.text(placement.source.source_pair().1)?;
        }
        if value.outlines.is_empty() {
            return Err(invalid("rule areas require an authored outline"));
        }
        for polygon in &value.outlines {
            self.zone_points(&polygon.points, "rule area outline")?;
        }
        Ok(())
    }
}

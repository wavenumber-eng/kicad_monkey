use super::*;

impl Validation {
    pub(super) fn zone(
        &mut self,
        value: &AuthoredZone,
        nets: &BTreeMap<i64, String>,
        layers: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        self.zone_header(value, nets, layers)?;
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

    pub(super) fn zone_points(
        &mut self,
        points: &[AuthoredPoint],
        label: &str,
    ) -> Result<(), Error> {
        self.object()?;
        if points.len() < 3 {
            return Err(invalid(format!("{label} requires at least three points")));
        }
        for point in points {
            self.point(*point)?;
        }
        Ok(())
    }

    fn zone_header(
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
        self.zone_fill(value.fill.as_ref())?;
        Ok(())
    }
    fn zone_fill(&mut self, fill: Option<&AuthoredZoneFill>) -> Result<(), Error> {
        if let Some(fill) = fill {
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
        Ok(())
    }
}

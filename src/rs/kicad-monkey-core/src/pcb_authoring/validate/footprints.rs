use super::*;

impl Validation {
    pub(super) fn footprint(
        &mut self,
        footprint: &AuthoredFootprint,
        board_nets: Option<&BTreeMap<i64, String>>,
        board_layers: Option<&BTreeMap<String, String>>,
        occurrence: Option<&AuthoredFootprintOccurrence>,
    ) -> Result<(), Error> {
        self.footprint_header(footprint)?;
        let mut property_names = BTreeSet::new();
        let variables = BoardTextVariables::from_entries(
            footprint
                .scalar_properties
                .iter()
                .map(|property| (&property.name, &property.value))
                .chain(
                    footprint
                        .properties
                        .iter()
                        .map(|property| (&property.name, &property.value)),
                ),
        );
        for property in &footprint.scalar_properties {
            self.text(&property.name)?;
            self.text(&property.value)?;
            if property.name.is_empty() {
                return Err(invalid("footprint scalar property name must be nonempty"));
            }
            if !property_names.insert(property.name.as_str()) {
                return Err(invalid(format!(
                    "duplicate footprint property name {:?} in one owner",
                    property.name
                )));
            }
        }
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
            self.graphic(graphic, board_layers, true)?;
        }
        for pad in &footprint.pads {
            self.pad(pad, board_nets, board_layers)?;
        }
        for model in &footprint.models {
            self.model(model)?;
        }
        self.resources(&footprint.embedded_files)
    }

    pub(super) fn occurrence(
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
        for (label, value) in [
            ("path", &occurrence.placement_path),
            ("sheetname", &occurrence.placement_sheet_name),
            ("sheetfile", &occurrence.placement_sheet_file),
        ] {
            if let Some(value) = value {
                self.text(value)?;
                if value.is_empty() {
                    return Err(invalid(format!(
                        "empty footprint {label} is discarded by KiCad"
                    )));
                }
            }
        }
        if let Some(path) = &occurrence.placement_path {
            let valid = path
                .strip_prefix('/')
                .is_some_and(|tail| !tail.is_empty() && tail.split('/').all(is_uuid));
            if !valid {
                return Err(invalid(
                    "footprint path requires slash-separated UUIDs with a leading slash",
                ));
            }
        }
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

    fn footprint_header(&mut self, footprint: &AuthoredFootprint) -> Result<(), Error> {
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
                    "dnp",
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
        optional_nonnegative(footprint.clearance_mm, "footprint clearance")?;
        Ok(())
    }
}

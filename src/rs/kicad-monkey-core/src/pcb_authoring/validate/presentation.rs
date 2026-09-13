use super::*;

impl Validation {
    pub(super) fn footprint_property(
        &mut self,
        value: &AuthoredFootprintProperty,
        board_layers: Option<&BTreeMap<String, String>>,
        variables: &BoardTextVariables,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&value.name)?;
        self.text(&value.value)?;
        self.text(&value.layer)?;
        require_footprint_layer(&value.layer, board_layers, false)?;
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

    pub(super) fn footprint_text(
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
        require_footprint_layer(&value.layer, board_layers, false)?;
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

    pub(super) fn effects(&mut self, value: &AuthoredTextEffects) -> Result<(), Error> {
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

    pub(super) fn board_text(
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
        let cache_text = match value.render_cache.as_ref() {
            Some(cache) if value.text.contains("${") => {
                if cache.text.contains("${") {
                    return Err(invalid(
                        "board text render caches must contain resolved display text",
                    ));
                }
                cache.text.as_str()
            }
            _ => value.text.as_str(),
        };
        self.render_cache(
            value.render_cache.as_ref(),
            cache_text,
            value.angle_degrees,
            &value.effects,
        )?;
        self.identity(&value.uuid)
    }

    pub(super) fn text_box(
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
        if variables.is_some() {
            require_footprint_layer(&value.layer, board_layers, false)?;
        } else {
            require_layer(&value.layer, board_layers, false)?;
        }
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

    pub(super) fn resolved_cache_text(
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

    pub(super) fn render_cache(
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

    pub(super) fn graphic(
        &mut self,
        graphic: &AuthoredGraphic,
        board_nets: Option<&BTreeMap<i64, String>>,
        board_layers: Option<&BTreeMap<String, String>>,
        footprint_member: bool,
    ) -> Result<(), Error> {
        self.object()?;
        self.text(&graphic.layer)?;
        if footprint_member {
            require_footprint_layer(&graphic.layer, board_layers, false)?;
        } else {
            require_layer(&graphic.layer, board_layers, false)?;
        }
        if let Some(net) = &graphic.net {
            let Some(board_nets) = board_nets else {
                return Err(invalid(
                    "standalone footprint graphics cannot author board net references",
                ));
            };
            if !graphic.layer.ends_with(".Cu") {
                return Err(invalid("graphic net references require a copper layer"));
            }
            if board_nets.get(&net.code) != Some(&net.name) {
                return Err(invalid(
                    "graphic net reference does not match the authored board net",
                ));
            }
        }
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
        self.graphic_fill(graphic)?;
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
            AuthoredGraphicGeometry::Curve { points } => {
                for point in points {
                    self.point(*point)?;
                }
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
                    match point {
                        AuthoredPolygonPoint::Xy(point) => self.point(*point)?,
                        AuthoredPolygonPoint::Arc { start, mid, end } => {
                            self.point(*start)?;
                            self.point(*mid)?;
                            self.point(*end)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn graphic_fill(&mut self, graphic: &AuthoredGraphic) -> Result<(), Error> {
        if matches!(
            graphic.fill.as_deref(),
            Some("solid" | "yes" | "hatch" | "reverse_hatch" | "cross_hatch")
        ) {
            nonnegative(graphic.stroke_width_mm, "filled graphic stroke width")?;
        } else {
            positive(graphic.stroke_width_mm, "graphic stroke width")?;
        }
        let Some(fill) = &graphic.fill else {
            return Ok(());
        };
        self.text(fill)?;
        supported_token(
            fill,
            "graphic fill",
            &[
                "no",
                "none",
                "yes",
                "solid",
                "hatch",
                "reverse_hatch",
                "cross_hatch",
            ],
        )?;
        if matches!(
            graphic.geometry,
            AuthoredGraphicGeometry::Line { .. }
                | AuthoredGraphicGeometry::Arc { .. }
                | AuthoredGraphicGeometry::Curve { .. }
        ) {
            return Err(invalid(
                "line, arc and curve graphics cannot author fill because KiCad discards it",
            ));
        }
        Ok(())
    }
}

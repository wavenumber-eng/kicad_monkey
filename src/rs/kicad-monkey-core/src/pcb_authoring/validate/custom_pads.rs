use super::*;

impl Validation {
    pub(super) fn custom_primitive(
        &mut self,
        primitive: &AuthoredPadPrimitive,
    ) -> Result<(), Error> {
        self.object()?;
        custom_primitive_style(primitive)?;
        self.custom_primitive_geometry(&primitive.geometry)
    }

    fn custom_primitive_geometry(
        &mut self,
        geometry: &AuthoredPadPrimitiveGeometry,
    ) -> Result<(), Error> {
        match geometry {
            AuthoredPadPrimitiveGeometry::Line { start, end } => {
                self.point(*start)?;
                self.point(*end)?;
            }
            AuthoredPadPrimitiveGeometry::Arc { start, mid, end } => {
                self.point(*start)?;
                self.point(*mid)?;
                self.point(*end)?;
            }
            AuthoredPadPrimitiveGeometry::Circle { center, end } => {
                self.point(*center)?;
                self.point(*end)?;
            }
            AuthoredPadPrimitiveGeometry::Rect {
                start,
                end,
                radius_mm,
            } => {
                self.point(*start)?;
                self.point(*end)?;
                // KiCad emits a radius only when positive; explicit zero would
                // be lost, and negative radius is not an authored roundrect.
                optional_positive(*radius_mm, "custom rectangle radius")?;
            }
            AuthoredPadPrimitiveGeometry::Curve { points } => {
                for point in points {
                    self.point(*point)?;
                }
            }
            AuthoredPadPrimitiveGeometry::Polygon { points } => {
                self.custom_polygon_points(points)?
            }
        }
        Ok(())
    }

    fn custom_polygon_points(&mut self, points: &[AuthoredPadPolygonPoint]) -> Result<(), Error> {
        // Arc endpoints count toward the contour; do not flatten it.
        let count = points.iter().fold(0usize, |count, point| {
            count.saturating_add(match point {
                AuthoredPadPolygonPoint::Xy(_) => 1,
                AuthoredPadPolygonPoint::Arc { .. } => 3,
            })
        });
        self.custom_polygon_count(count)?;
        for point in points {
            match point {
                AuthoredPadPolygonPoint::Xy(point) => self.point(*point)?,
                AuthoredPadPolygonPoint::Arc { start, mid, end } => {
                    self.point(*start)?;
                    self.point(*mid)?;
                    self.point(*end)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn custom_polygon_count(&self, count: usize) -> Result<(), Error> {
        if count < 3 {
            return Err(invalid("custom pad polygons require at least three points"));
        }
        Ok(())
    }
}

fn custom_primitive_style(primitive: &AuthoredPadPrimitive) -> Result<(), Error> {
    let closed = matches!(
        primitive.geometry,
        AuthoredPadPrimitiveGeometry::Circle { .. }
            | AuthoredPadPrimitiveGeometry::Rect { .. }
            | AuthoredPadPrimitiveGeometry::Polygon { .. }
    );
    if !closed && primitive.fill.is_some() {
        return Err(invalid(
            "KiCad discards fill on custom line, arc and curve primitives",
        ));
    }
    // Absence uses the source default: polygons fill; zero-width circles
    // and rectangles fill. Unfilled nonpositive strokes are repaired by
    // KiCad, so reject them instead of claiming lossless emission.
    let filled = match primitive.fill {
        Some(AuthoredPadPrimitiveFill::Solid) => true,
        Some(AuthoredPadPrimitiveFill::Unfilled) => false,
        None => {
            matches!(
                primitive.geometry,
                AuthoredPadPrimitiveGeometry::Polygon { .. }
            ) || closed && primitive.width_mm == 0.0
        }
    };
    if filled {
        nonnegative(primitive.width_mm, "custom primitive width")?;
    } else {
        positive(primitive.width_mm, "custom primitive width")?;
    }
    Ok(())
}

use super::*;

/// Ordered KiCad polygon outline elements, without tessellating embedded arcs.
#[derive(Clone, Debug, PartialEq)]
pub enum PcbPadPolygonPoint {
    Xy(PcbPoint),
    Arc {
        start: PcbPoint,
        mid: PcbPoint,
        end: PcbPoint,
    },
}

/// Source-local geometry. Proxy shapes are not copper primitives.
#[derive(Clone, Debug, PartialEq)]
pub enum PcbPadPrimitiveGeometry {
    Line {
        start: PcbPoint,
        end: PcbPoint,
    },
    Arc {
        start: PcbPoint,
        mid: PcbPoint,
        end: PcbPoint,
    },
    Circle {
        center: PcbPoint,
        end: PcbPoint,
    },
    Rect {
        start: PcbPoint,
        end: PcbPoint,
        radius: Option<f64>,
    },
    Polygon {
        points: Vec<PcbPadPolygonPoint>,
    },
    Curve {
        points: [PcbPoint; 4],
    },
    BoundingBoxProxy {
        start: PcbPoint,
        end: PcbPoint,
    },
    VectorProxy {
        start: PcbPoint,
        end: PcbPoint,
    },
}

pub(super) fn primitive_geometry(
    source: &str,
    primitive: &FormSpan,
    fields: &[FormSpan],
    limits: PcbLimits,
    count: &mut usize,
) -> Result<(Vec<PcbPoint>, Option<PcbPadPrimitiveGeometry>), Error> {
    let (legacy_points, outline, complete) = outline_points(source, fields, limits, count)?;
    let geometry = match primitive.head.as_deref() {
        Some("gr_poly") if complete => Some(PcbPadPrimitiveGeometry::Polygon { points: outline }),
        Some("gr_curve") if complete && outline.len() == 4 && legacy_points.len() == 4 => {
            Some(PcbPadPrimitiveGeometry::Curve {
                points: legacy_points.as_slice().try_into().expect("four XY points"),
            })
        }
        _ => endpoint_geometry(source, primitive, fields, limits, count)?,
    };
    Ok((legacy_points, geometry))
}

fn outline_points(
    source: &str,
    fields: &[FormSpan],
    limits: PcbLimits,
    count: &mut usize,
) -> Result<(Vec<PcbPoint>, Vec<PcbPadPolygonPoint>, bool), Error> {
    let mut legacy_points = Vec::new();
    let mut outline = Vec::new();
    let mut complete = child(fields, "pts").is_some();
    if let Some(container) = child(fields, "pts") {
        for point in direct_children(source, container, limits.max_pad_custom_point_forms, limits)?
        {
            match point.head.as_deref() {
                Some("xy") => {
                    let values = first_two_scalar_values(source, &point)?;
                    let [x, y] = values.as_slice() else {
                        complete = false;
                        continue;
                    };
                    charge(count, 1, limits)?;
                    let xy = PcbPoint {
                        x: parse_f64(x, &point)?,
                        y: parse_f64(y, &point)?,
                    };
                    legacy_points.push(xy);
                    outline.push(PcbPadPolygonPoint::Xy(xy));
                }
                Some("arc") => {
                    let fields = direct_children(source, &point, limits.max_pad_children, limits)?;
                    let (Some(start), Some(mid), Some(end)) = (
                        optional_child_point(source, &fields, "start")?,
                        optional_child_point(source, &fields, "mid")?,
                        optional_child_point(source, &fields, "end")?,
                    ) else {
                        complete = false;
                        continue;
                    };
                    charge(count, 3, limits)?;
                    outline.push(PcbPadPolygonPoint::Arc { start, mid, end });
                }
                _ => complete = false,
            }
        }
    }
    Ok((legacy_points, outline, complete))
}

fn endpoint_geometry(
    source: &str,
    primitive: &FormSpan,
    fields: &[FormSpan],
    limits: PcbLimits,
    count: &mut usize,
) -> Result<Option<PcbPadPrimitiveGeometry>, Error> {
    Ok(match primitive.head.as_deref() {
        Some("gr_line" | "gr_rect" | "gr_arc" | "gr_circle" | "gr_bbox" | "gr_vector") => {
            let start_name = if primitive.head.as_deref() == Some("gr_circle") {
                "center"
            } else {
                "start"
            };
            let start = optional_child_point(source, fields, start_name)?;
            let end = optional_child_point(source, fields, "end")?;
            let mid = optional_child_point(source, fields, "mid")?;
            // Charge all newly exposed coordinates, not only polygon XY forms.
            charge(
                count,
                usize::from(start.is_some())
                    + usize::from(end.is_some())
                    + usize::from(mid.is_some()),
                limits,
            )?;
            match (primitive.head.as_deref(), start, mid, end) {
                (Some("gr_line"), Some(start), _, Some(end)) => {
                    Some(PcbPadPrimitiveGeometry::Line { start, end })
                }
                (Some("gr_arc"), Some(start), Some(mid), Some(end)) => {
                    Some(PcbPadPrimitiveGeometry::Arc { start, mid, end })
                }
                (Some("gr_circle"), Some(center), _, Some(end)) => {
                    Some(PcbPadPrimitiveGeometry::Circle { center, end })
                }
                (Some("gr_rect"), Some(start), _, Some(end)) => {
                    Some(PcbPadPrimitiveGeometry::Rect {
                        start,
                        end,
                        radius: optional_child_f64(source, fields, "radius")?,
                    })
                }
                (Some("gr_bbox"), Some(start), _, Some(end)) => {
                    Some(PcbPadPrimitiveGeometry::BoundingBoxProxy { start, end })
                }
                (Some("gr_vector"), Some(start), _, Some(end)) => {
                    Some(PcbPadPrimitiveGeometry::VectorProxy { start, end })
                }
                _ => None,
            }
        }
        _ => None,
    })
}

fn charge(count: &mut usize, added: usize, limits: PcbLimits) -> Result<(), Error> {
    if added > limits.max_pad_custom_points.saturating_sub(*count) {
        return Err(limit_error());
    }
    *count += added;
    Ok(())
}

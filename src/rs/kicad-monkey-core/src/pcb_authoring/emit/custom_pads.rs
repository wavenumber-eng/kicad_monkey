use super::*;

pub(super) fn append_custom(
    anchor: Option<AuthoredPadAnchor>,
    clearance: Option<AuthoredCustomPadClearance>,
    primitives: impl IntoIterator<Item = Sexp>,
    children: &mut Vec<Sexp>,
) {
    let mut options = Vec::new();
    if let Some(clearance) = clearance {
        options.push(form(
            "clearance",
            [atom(match clearance {
                AuthoredCustomPadClearance::Outline => "outline",
                AuthoredCustomPadClearance::ConvexHull => "convexhull",
            })],
        ));
    }
    if let Some(anchor) = anchor {
        options.push(form(
            "anchor",
            [atom(match anchor {
                AuthoredPadAnchor::Circle => "circle",
                AuthoredPadAnchor::Rect => "rect",
            })],
        ));
    }
    if !options.is_empty() {
        children.push(form("options", options));
    }
    children.push(form("primitives", primitives));
}

pub(super) fn polygon(
    points: impl IntoIterator<Item = Sexp>,
    width: f64,
    fill: Option<AuthoredPadPrimitiveFill>,
) -> Sexp {
    styled("gr_poly", vec![form("pts", points)], width, fill)
}

pub(super) fn primitive(value: &AuthoredPadPrimitive) -> Sexp {
    let (head, points) = match &value.geometry {
        AuthoredPadPrimitiveGeometry::Line { start, end } => (
            "gr_line",
            vec![point_form("start", *start), point_form("end", *end)],
        ),
        AuthoredPadPrimitiveGeometry::Arc { start, mid, end } => (
            "gr_arc",
            vec![
                point_form("start", *start),
                point_form("mid", *mid),
                point_form("end", *end),
            ],
        ),
        AuthoredPadPrimitiveGeometry::Circle { center, end } => (
            "gr_circle",
            vec![point_form("center", *center), point_form("end", *end)],
        ),
        AuthoredPadPrimitiveGeometry::Rect {
            start,
            end,
            radius_mm,
        } => {
            let mut fields = vec![point_form("start", *start), point_form("end", *end)];
            if let Some(radius) = radius_mm {
                fields.push(form("radius", [float(*radius)]));
            }
            ("gr_rect", fields)
        }
        AuthoredPadPrimitiveGeometry::Curve { points } => ("gr_curve", vec![points_form(points)]),
        AuthoredPadPrimitiveGeometry::Polygon { points } => {
            return polygon(
                points.iter().map(|point| match point {
                    AuthoredPadPolygonPoint::Xy(point) => point_form("xy", *point),
                    AuthoredPadPolygonPoint::Arc { start, mid, end } => form(
                        "arc",
                        [
                            point_form("start", *start),
                            point_form("mid", *mid),
                            point_form("end", *end),
                        ],
                    ),
                }),
                value.width_mm,
                value.fill,
            );
        }
    };
    styled(head, points, value.width_mm, value.fill)
}

fn styled(
    head: &str,
    mut fields: Vec<Sexp>,
    width: f64,
    fill: Option<AuthoredPadPrimitiveFill>,
) -> Sexp {
    fields.push(form("width", [float(width)]));
    if let Some(fill) = fill {
        fields.push(form(
            "fill",
            [atom(match fill {
                AuthoredPadPrimitiveFill::Solid => "yes",
                AuthoredPadPrimitiveFill::Unfilled => "no",
            })],
        ));
    }
    form(head, fields)
}

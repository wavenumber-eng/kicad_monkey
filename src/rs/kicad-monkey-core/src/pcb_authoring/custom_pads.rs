//! Source-native custom-pad composition; no Boolean union or flattening.

use super::AuthoredPoint;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredPadAnchor {
    Circle,
    Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredCustomPadClearance {
    Outline,
    ConvexHull,
}

/// KiCad retains solid/unfilled custom primitive policy on closed shapes.
/// Hatch modes and stroke styles are not retained by its custom-pad writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredPadPrimitiveFill {
    Solid,
    Unfilled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredPadPolygonPoint {
    Xy(AuthoredPoint),
    Arc {
        start: AuthoredPoint,
        mid: AuthoredPoint,
        end: AuthoredPoint,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredPadPrimitiveGeometry {
    Line {
        start: AuthoredPoint,
        end: AuthoredPoint,
    },
    Arc {
        start: AuthoredPoint,
        mid: AuthoredPoint,
        end: AuthoredPoint,
    },
    Circle {
        center: AuthoredPoint,
        end: AuthoredPoint,
    },
    Rect {
        start: AuthoredPoint,
        end: AuthoredPoint,
        radius_mm: Option<f64>,
    },
    Polygon {
        points: Vec<AuthoredPadPolygonPoint>,
    },
    Curve {
        points: [AuthoredPoint; 4],
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredPadPrimitive {
    pub geometry: AuthoredPadPrimitiveGeometry,
    pub width_mm: f64,
    /// None retains an absent fill declaration, using KiCad's source default.
    /// Line, arc and curve primitives require None (KiCad drops their fill).
    pub fill: Option<AuthoredPadPrimitiveFill>,
}

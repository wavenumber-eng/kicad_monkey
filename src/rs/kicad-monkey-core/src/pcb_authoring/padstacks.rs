//! Source-native copper layer variations. Source parsing/saving owns defaults.

use super::{AuthoredPadShape, AuthoredPoint, AuthoredZoneConnection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredPadstackMode {
    FrontInnerBack,
    Custom,
}

impl AuthoredPadstackMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrontInnerBack => "front_inner_back",
            Self::Custom => "custom",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AuthoredPadstackLayerSelector {
    Inner,
    CopperLayer(String),
}

impl AuthoredPadstackLayerSelector {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Inner => "Inner",
            Self::CopperLayer(layer) => layer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredPadstackZoneConnection {
    /// An explicit source `(zone_connect -1)`, not absence.
    Inherited,
    Connection(AuthoredZoneConnection),
}

impl AuthoredPadstackZoneConnection {
    pub const fn source_code(self) -> i64 {
        match self {
            Self::Inherited => -1,
            Self::Connection(value) => value.source_code(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredPadstack {
    pub mode: AuthoredPadstackMode,
    /// Ordered source rows. Omitted rows use KiCad's source defaults; saving
    /// may materialize them, including all 32 copper slots for a library pad.
    pub layers: Vec<AuthoredPadstackLayer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredPadstackLayer {
    pub layer: AuthoredPadstackLayerSelector,
    pub shape: AuthoredPadShape,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    /// This is a land-shape offset, never a separate drill position.
    pub offset: AuthoredPoint,
    pub clearance_mm: Option<f64>,
    pub thermal_bridge_width_mm: Option<f64>,
    pub thermal_gap_mm: Option<f64>,
    /// Readable source evidence, but currently rejected on fresh layer writes:
    /// KiCad 10.0.6 parses this setting onto F.Cu instead of the selected row.
    pub thermal_bridge_angle_degrees: Option<f64>,
    pub zone_connect: Option<AuthoredPadstackZoneConnection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredViaStack {
    pub mode: AuthoredPadstackMode,
    pub layers: Vec<AuthoredViaStackLayer>,
}

/// KiCad via stacks support circular diameters only; no per-layer nets/shapes.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredViaStackLayer {
    pub layer: AuthoredPadstackLayerSelector,
    pub size_mm: f64,
}

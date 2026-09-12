//! Source-native board rule areas, separate from electrical copper zones.

use super::{AuthoredZoneHatch, AuthoredZonePolygon};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredRestriction {
    Allowed,
    NotAllowed,
}

impl AuthoredRestriction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::NotAllowed => "not_allowed",
        }
    }
}

/// Complete effective restrictions, emitted explicitly rather than inheriting
/// source-version-dependent omission defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredKeepout {
    pub tracks: AuthoredRestriction,
    pub vias: AuthoredRestriction,
    pub pads: AuthoredRestriction,
    pub copperpour: AuthoredRestriction,
    pub footprints: AuthoredRestriction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoredPlacementSource {
    SheetName(String),
    ComponentClass(String),
    Group(String),
}

impl AuthoredPlacementSource {
    pub fn source_pair(&self) -> (&'static str, &str) {
        match self {
            Self::SheetName(value) => ("sheetname", value),
            Self::ComponentClass(value) => ("component_class", value),
            Self::Group(value) => ("group", value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredPlacementConstraint {
    pub enabled: bool,
    pub source: AuthoredPlacementSource,
}

/// One board-frame source zone used for restrictions and/or placement.
/// No net, fill cache, or thermal/pour policy can be attached to this type.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredRuleArea {
    pub layers: Vec<String>,
    pub locked: bool,
    pub uuid: String,
    pub name: Option<String>,
    pub hatch: AuthoredZoneHatch,
    pub hatch_pitch_mm: f64,
    pub keepout: AuthoredKeepout,
    pub placement: Option<AuthoredPlacementConstraint>,
    /// Source order is retained: first exterior, subsequent interior outlines.
    pub outlines: Vec<AuthoredZonePolygon>,
}

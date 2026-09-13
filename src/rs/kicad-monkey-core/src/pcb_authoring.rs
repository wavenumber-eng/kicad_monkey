//! Typed fresh-source authoring for KiCad PCB and footprint files.

mod custom_pads;
mod emit;
mod padstacks;
mod resource;
mod rule_areas;
mod validate;

pub use rule_areas::{
    AuthoredKeepout, AuthoredPlacementConstraint, AuthoredPlacementSource, AuthoredRestriction,
    AuthoredRuleArea,
};

use crate::footprint::{FootprintDocument, FootprintLimits};
use crate::pcb::{PcbDocument, PcbLimits};
use crate::sexpr::Error;
use crate::text_render_cache::TextRenderCache;
pub use custom_pads::{
    AuthoredCustomPadClearance, AuthoredPadAnchor, AuthoredPadPolygonPoint, AuthoredPadPrimitive,
    AuthoredPadPrimitiveFill, AuthoredPadPrimitiveGeometry,
};
pub use padstacks::{
    AuthoredPadstack, AuthoredPadstackLayer, AuthoredPadstackLayerSelector, AuthoredPadstackMode,
    AuthoredPadstackZoneConnection, AuthoredViaStack, AuthoredViaStackLayer,
};

/// First explicitly supported PCB and footprint source format revision.
pub const KICAD_SOURCE_VERSION_2024_12_29: i64 = 20_241_229;

/// Independent ceilings for fresh typed source construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcbAuthoringLimits {
    pub max_objects: usize,
    pub max_points: usize,
    pub max_string_bytes: usize,
    pub max_resource_input_bytes: usize,
    /// Conservative aggregate zstd/base64 allocation bound checked before encoding.
    pub max_resource_work_bytes: usize,
    pub max_resource_encoded_bytes: usize,
    pub max_output_bytes: usize,
    pub pcb_limits: PcbLimits,
    pub footprint_limits: FootprintLimits,
}

impl Default for PcbAuthoringLimits {
    fn default() -> Self {
        Self {
            max_objects: 1_000_000,
            max_points: 16_000_000,
            max_string_bytes: 256 * 1024 * 1024,
            max_resource_input_bytes: 256 * 1024 * 1024,
            max_resource_work_bytes: 512 * 1024 * 1024,
            max_resource_encoded_bytes: 384 * 1024 * 1024,
            max_output_bytes: 256 * 1024 * 1024,
            pcb_limits: PcbLimits::default(),
            footprint_limits: FootprintLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AuthoredPoint {
    pub x_mm: f64,
    pub y_mm: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredLayer {
    pub ordinal: i64,
    pub name: String,
    pub kind: String,
    pub user_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredStackupLayer {
    pub name: String,
    pub type_name: String,
    pub thickness_mm: Option<f64>,
    pub thickness_locked: bool,
    pub material: Option<String>,
    pub epsilon_r: Option<f64>,
    pub loss_tangent: Option<f64>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthoredStackup {
    pub layers: Vec<AuthoredStackupLayer>,
    pub copper_finish: Option<String>,
    pub dielectric_constraints: bool,
    pub edge_connector: Option<String>,
    pub edge_plating: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthoredSetup {
    pub aux_axis_origin: AuthoredPoint,
    pub grid_origin: AuthoredPoint,
    /// Board-level surface-policy fallbacks used when neither the footprint
    /// nor the pad authors a corresponding override.
    pub pad_to_mask_clearance_mm: f64,
    pub pad_to_paste_clearance_mm: f64,
    pub pad_to_paste_clearance_ratio: f64,
    pub allow_soldermask_bridges_in_footprints: bool,
    /// Effective board defaults; via-local optional overrides remain separate.
    pub tenting_front: bool,
    pub tenting_back: bool,
    pub stackup: Option<AuthoredStackup>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredNet {
    pub code: i64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredPolygonPoint {
    Xy(AuthoredPoint),
    Arc {
        start: AuthoredPoint,
        mid: AuthoredPoint,
        end: AuthoredPoint,
    },
}

impl From<AuthoredPoint> for AuthoredPolygonPoint {
    fn from(value: AuthoredPoint) -> Self {
        Self::Xy(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredGraphicGeometry {
    Line {
        start: AuthoredPoint,
        end: AuthoredPoint,
    },
    Arc {
        start: AuthoredPoint,
        mid: AuthoredPoint,
        end: AuthoredPoint,
    },
    /// KiCad's ordered cubic Bezier tuple: start, control 1, control 2, end.
    Curve {
        points: [AuthoredPoint; 4],
    },
    Rect {
        start: AuthoredPoint,
        end: AuthoredPoint,
    },
    Circle {
        center: AuthoredPoint,
        end: AuthoredPoint,
    },
    Polygon {
        points: Vec<AuthoredPolygonPoint>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredGraphic {
    pub geometry: AuthoredGraphicGeometry,
    pub layer: String,
    /// Board connectivity for copper graphics. KiCad 10 serializes shape nets
    /// by name; the code is retained here so validation can bind the name to
    /// the authored board net table before source emission.
    pub net: Option<AuthoredNetRef>,
    pub locked: bool,
    pub stroke_width_mm: f64,
    pub stroke_kind: String,
    pub fill: Option<String>,
    pub uuid: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredTextEffects {
    /// `None` selects KiCad's native Newstroke source font. A named face
    /// selects a TrueType/OpenType source font and permits a render cache.
    pub face: Option<String>,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    pub thickness_mm: Option<f64>,
    pub line_spacing: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub color: Option<AuthoredColor>,
    pub horizontal_justify: AuthoredTextHorizontalJustification,
    pub vertical_justify: AuthoredTextVerticalJustification,
    pub mirrored: bool,
    pub href: Option<String>,
}

impl Default for AuthoredTextEffects {
    fn default() -> Self {
        Self {
            face: None,
            size_x_mm: 1.0,
            size_y_mm: 1.0,
            thickness_mm: Some(0.15),
            line_spacing: None,
            bold: false,
            italic: false,
            color: None,
            horizontal_justify: AuthoredTextHorizontalJustification::Center,
            vertical_justify: AuthoredTextVerticalJustification::Center,
            mirrored: false,
            href: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredColor {
    pub red: i64,
    pub green: i64,
    pub blue: i64,
    pub alpha: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AuthoredTextHorizontalJustification {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AuthoredTextVerticalJustification {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredFootprintText {
    /// User-authored footprint text. Reference and value source facts use
    /// `AuthoredFootprintProperty`, matching current KiCad canonical output.
    pub text: String,
    pub at: AuthoredPoint,
    pub angle_degrees: f64,
    pub layer: String,
    /// Must be `false` for target revision 20241229: KiCad 10 rewrites a
    /// hidden user `fp_text` as a generated property with a new identity.
    pub hidden: bool,
    pub unlocked: bool,
    pub knockout: bool,
    pub effects: AuthoredTextEffects,
    /// Optional realized named-font geometry. Points use the reusable
    /// footprint-local cache frame; board emission applies KiCad's
    /// carrier-anchor translation. The cache text is resolved display text.
    pub render_cache: Option<TextRenderCache>,
    pub uuid: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredFootprintScalarProperty {
    pub name: String,
    pub value: String,
}

/// A graphical footprint property with an explicit local presentation.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredFootprintProperty {
    pub name: String,
    pub value: String,
    pub at: AuthoredPoint,
    pub angle_degrees: f64,
    pub layer: String,
    pub knockout: bool,
    pub hidden: bool,
    pub unlocked: bool,
    pub effects: AuthoredTextEffects,
    /// Optional realized named-font geometry in the reusable footprint-local
    /// cache frame. The cache text is resolved after footprint properties.
    pub render_cache: Option<TextRenderCache>,
    pub uuid: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredTextBox {
    pub text: String,
    pub geometry: AuthoredTextBoxGeometry,
    /// KiCad source order: `[left, top, right, bottom]`.
    pub margins_mm: [f64; 4],
    pub angle_degrees: f64,
    pub layer: String,
    pub locked: bool,
    pub border: bool,
    pub knockout: bool,
    pub stroke_width_mm: f64,
    pub stroke_kind: String,
    pub effects: AuthoredTextEffects,
    /// Optional realized named-font geometry. Footprint text-box cache points
    /// are local in a reusable definition and become board-frame when placed.
    pub render_cache: Option<TextRenderCache>,
    pub uuid: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredTextBoxGeometry {
    Rectangle {
        start: AuthoredPoint,
        end: AuthoredPoint,
    },
    Polygon {
        points: Vec<AuthoredPoint>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredBoardText {
    pub text: String,
    pub at: AuthoredPoint,
    pub angle_degrees: f64,
    pub layer: String,
    pub locked: bool,
    pub knockout: bool,
    pub effects: AuthoredTextEffects,
    pub render_cache: Option<TextRenderCache>,
    pub uuid: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredPadKind {
    Smd,
    ThroughHole,
    NonPlatedThroughHole,
}

impl AuthoredPadKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Smd => "smd",
            Self::ThroughHole => "thru_hole",
            Self::NonPlatedThroughHole => "np_thru_hole",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredPadShape {
    Circle,
    Oval,
    Rect,
    Trapezoid {
        delta_x_mm: f64,
        delta_y_mm: f64,
    },
    RoundRect {
        radius_ratio: f64,
    },
    ChamferedRoundRect {
        radius_ratio: f64,
        chamfer_ratio: f64,
        corners: Vec<AuthoredChamferCorner>,
    },
    CustomPolygon {
        points: Vec<AuthoredPoint>,
    },
    /// Complete source composition. Prefer this over the legacy single-polygon
    /// convenience when retaining an imported custom pad's policies.
    Custom {
        anchor: Option<AuthoredPadAnchor>,
        clearance: Option<AuthoredCustomPadClearance>,
        primitives: Vec<AuthoredPadPrimitive>,
    },
}

impl AuthoredPadShape {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Oval => "oval",
            Self::Rect => "rect",
            Self::Trapezoid { .. } => "trapezoid",
            Self::RoundRect { .. } | Self::ChamferedRoundRect { .. } => "roundrect",
            Self::CustomPolygon { .. } | Self::Custom { .. } => "custom",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AuthoredChamferCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl AuthoredChamferCorner {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredDrill {
    pub width_mm: f64,
    pub height_mm: Option<f64>,
    pub offset: AuthoredPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredNetRef {
    pub code: i64,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredZoneConnection {
    NoConnection,
    ThermalRelief,
    Solid,
    ThermalReliefForThroughHole,
}

impl AuthoredZoneConnection {
    pub const fn source_code(self) -> i64 {
        match self {
            Self::NoConnection => 0,
            Self::ThermalRelief => 1,
            Self::Solid => 2,
            Self::ThermalReliefForThroughHole => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredPad {
    pub padstack: Option<AuthoredPadstack>,
    pub number: String,
    /// Board occurrence pin metadata; KiCad omits these in library footprints.
    pub pin_function: Option<String>,
    pub pin_type: Option<String>,
    /// Signed, nonzero source length. Zero is KiCad's omitted default.
    pub die_length_mm: Option<f64>,
    pub kind: AuthoredPadKind,
    pub shape: AuthoredPadShape,
    pub at: AuthoredPoint,
    pub angle_degrees: f64,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    pub drill: Option<AuthoredDrill>,
    pub layers: Vec<String>,
    pub net: Option<AuthoredNetRef>,
    pub uuid: String,
    pub solder_mask_margin_mm: Option<f64>,
    pub solder_paste_margin_mm: Option<f64>,
    pub solder_paste_margin_ratio: Option<f64>,
    pub clearance_mm: Option<f64>,
    pub thermal_bridge_width_mm: Option<f64>,
    pub thermal_bridge_angle_degrees: Option<f64>,
    pub thermal_gap_mm: Option<f64>,
    pub zone_connect: Option<AuthoredZoneConnection>,
    pub zone_layer_connections: Vec<String>,
    pub remove_unused_layers: Option<bool>,
    pub keep_end_layers: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredViaKind {
    Through,
    /// KiCad uses the `blind` source token for both blind and buried spans;
    /// the selected copper endpoints preserve which case was authored.
    BlindBuried,
    Micro,
}

impl AuthoredViaKind {
    pub const fn source_token(self) -> Option<&'static str> {
        match self {
            Self::Through => None,
            Self::BlindBuried => Some("blind"),
            Self::Micro => Some("micro"),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuthoredFrontBackPolicy {
    pub front: Option<bool>,
    pub back: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredBackdrill {
    pub size_mm: f64,
    /// Source drill direction, which may run from B.Cu toward an inner layer.
    pub start_layer: String,
    pub end_layer: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredVia {
    pub padstack: Option<AuthoredViaStack>,
    pub kind: AuthoredViaKind,
    pub at: AuthoredPoint,
    pub size_mm: f64,
    pub drill_mm: f64,
    pub backdrill: Option<AuthoredBackdrill>,
    pub start_layer: String,
    pub end_layer: String,
    pub free: bool,
    pub net_code: i64,
    pub uuid: String,
    pub tenting: Option<AuthoredFrontBackPolicy>,
    pub covering: Option<AuthoredFrontBackPolicy>,
    pub plugging: Option<AuthoredFrontBackPolicy>,
    pub capping: Option<bool>,
    pub filling: Option<bool>,
    pub zone_layer_connections: Vec<String>,
    pub remove_unused_layers: Option<bool>,
    pub keep_end_layers: Option<bool>,
    pub start_end_only: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredModel {
    pub path: String,
    pub offset_mm: [f64; 3],
    pub scale: [f64; 3],
    pub rotate_degrees: [f64; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoredResourceData {
    DeclarationOnly,
    Empty,
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredEmbeddedFile {
    pub name: String,
    pub file_type: String,
    pub data: AuthoredResourceData,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredFootprint {
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub attributes: Vec<String>,
    /// Footprint-level surface-policy defaults. A pad-local value takes
    /// precedence; otherwise these take precedence over the board setup.
    pub solder_mask_margin_mm: Option<f64>,
    pub solder_paste_margin_mm: Option<f64>,
    pub solder_paste_margin_ratio: Option<f64>,
    pub clearance_mm: Option<f64>,
    /// Absent means inherit the zone's connection policy.
    pub zone_connect: Option<AuthoredZoneConnection>,
    /// Properties with no authored presentation children. These remain
    /// distinct from hidden graphical properties: emitting default coordinates
    /// would create source geometry that did not exist.
    pub scalar_properties: Vec<AuthoredFootprintScalarProperty>,
    pub properties: Vec<AuthoredFootprintProperty>,
    pub texts: Vec<AuthoredFootprintText>,
    pub text_boxes: Vec<AuthoredTextBox>,
    pub graphics: Vec<AuthoredGraphic>,
    pub pads: Vec<AuthoredPad>,
    pub models: Vec<AuthoredModel>,
    pub embedded_files: Vec<AuthoredEmbeddedFile>,
}

impl AuthoredFootprint {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            tags: None,
            attributes: Vec::new(),
            solder_mask_margin_mm: None,
            solder_paste_margin_mm: None,
            solder_paste_margin_ratio: None,
            clearance_mm: None,
            zone_connect: None,
            scalar_properties: Vec::new(),
            properties: Vec::new(),
            texts: Vec::new(),
            text_boxes: Vec::new(),
            graphics: Vec::new(),
            pads: Vec::new(),
            models: Vec::new(),
            embedded_files: Vec::new(),
        }
    }
}

/// A fresh standalone `.kicad_mod` envelope around one local footprint definition.
///
/// This type owns document-only revision, producer, and default-layer facts so
/// they cannot be silently ignored when the same definition is placed on a board.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredStandaloneFootprint {
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
    pub layer: String,
    pub footprint: AuthoredFootprint,
}

impl AuthoredStandaloneFootprint {
    pub fn new(name: impl Into<String>, layer: impl Into<String>) -> Self {
        Self {
            version: KICAD_SOURCE_VERSION_2024_12_29,
            generator: "kicad_monkey".to_owned(),
            generator_version: crate::ENGINE_VERSION.to_owned(),
            layer: layer.into(),
            footprint: AuthoredFootprint::new(name),
        }
    }

    pub fn canonical_text(&self, limits: PcbAuthoringLimits) -> Result<String, Error> {
        emit::footprint_text(self, limits)
    }

    pub fn to_document(&self, limits: PcbAuthoringLimits) -> Result<FootprintDocument, Error> {
        FootprintDocument::parse(self.canonical_text(limits)?, limits.footprint_limits)
    }
}

/// One placed board occurrence of a footprint definition.
///
/// Occurrence identity and transform are deliberately separate from the
/// standalone document envelope. Member UUIDs inside `footprint` must also
/// be distinct across repeated occurrences.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredFootprintOccurrence {
    pub footprint: AuthoredFootprint,
    pub layer: String,
    pub at: AuthoredPoint,
    pub angle_degrees: f64,
    pub uuid: String,
    pub locked: bool,
    /// Schematic UUID path, not a filesystem path. Owned by this occurrence.
    pub placement_path: Option<String>,
    pub placement_sheet_name: Option<String>,
    pub placement_sheet_file: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredSegment {
    pub start: AuthoredPoint,
    pub end: AuthoredPoint,
    pub width_mm: f64,
    pub layer: String,
    pub net_code: i64,
    pub uuid: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredRoutingArc {
    pub start: AuthoredPoint,
    pub mid: AuthoredPoint,
    pub end: AuthoredPoint,
    pub width_mm: f64,
    pub layer: String,
    pub net_code: i64,
    pub uuid: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredZoneHatch {
    None,
    Edge,
    Full,
}

impl AuthoredZoneHatch {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Edge => "edge",
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredZonePadConnection {
    ThermalRelief,
    NoConnection,
    Solid,
    ThroughHoleOnly,
}

impl AuthoredZonePadConnection {
    pub const fn source_token(self) -> Option<&'static str> {
        match self {
            Self::ThermalRelief => None,
            Self::NoConnection => Some("no"),
            Self::Solid => Some("yes"),
            Self::ThroughHoleOnly => Some("thru_hole_only"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredZoneFill {
    pub thermal_gap_mm: f64,
    pub thermal_bridge_width_mm: f64,
    pub island_removal_mode: Option<i64>,
    pub island_area_min_mm2: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredZonePolygon {
    /// One source ring in board coordinates. Winding/topology remains caller-owned.
    pub points: Vec<AuthoredPoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredZoneFilledPolygon {
    pub layer: String,
    pub island: bool,
    /// One authoritative KiCad fill-cache point chain in board coordinates.
    /// Holes are already bridge/fracture-encoded within this chain; they are
    /// not separate `filled_polygon` records and are not inferred here.
    pub points: Vec<AuthoredPoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredZone {
    pub net: AuthoredNetRef,
    pub layers: Vec<String>,
    pub locked: bool,
    pub uuid: String,
    pub name: Option<String>,
    pub hatch: AuthoredZoneHatch,
    pub hatch_pitch_mm: f64,
    pub priority: i64,
    pub pad_connection: AuthoredZonePadConnection,
    pub connect_pads_clearance_mm: f64,
    pub min_thickness_mm: f64,
    pub filled_areas_thickness: Option<bool>,
    pub fill: Option<AuthoredZoneFill>,
    pub outlines: Vec<AuthoredZonePolygon>,
    pub filled_polygons: Vec<AuthoredZoneFilledPolygon>,
}

/// One board-root source property, independent of footprint graphical fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredProperty {
    pub name: String,
    pub value: String,
}

/// A board-owned editor group. Members are UUIDs of board-root items or groups,
/// not footprint children. This does not author design-block library links.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredGroup {
    pub name: String,
    pub uuid: String,
    pub locked: bool,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredPcb {
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
    pub thickness_mm: f64,
    pub paper: String,
    pub layers: Vec<AuthoredLayer>,
    pub setup: AuthoredSetup,
    pub properties: Vec<AuthoredProperty>,
    pub nets: Vec<AuthoredNet>,
    pub profile: Vec<AuthoredGraphic>,
    pub graphics: Vec<AuthoredGraphic>,
    pub texts: Vec<AuthoredBoardText>,
    pub text_boxes: Vec<AuthoredTextBox>,
    pub footprints: Vec<AuthoredFootprintOccurrence>,
    pub vias: Vec<AuthoredVia>,
    pub segments: Vec<AuthoredSegment>,
    pub arcs: Vec<AuthoredRoutingArc>,
    pub zones: Vec<AuthoredZone>,
    pub rule_areas: Vec<AuthoredRuleArea>,
    pub groups: Vec<AuthoredGroup>,
    pub embedded_files: Vec<AuthoredEmbeddedFile>,
}

impl Default for AuthoredPcb {
    fn default() -> Self {
        Self {
            version: KICAD_SOURCE_VERSION_2024_12_29,
            generator: "kicad_monkey".to_owned(),
            generator_version: crate::ENGINE_VERSION.to_owned(),
            thickness_mm: 1.6,
            paper: "A4".to_owned(),
            layers: Vec::new(),
            setup: AuthoredSetup::default(),
            properties: Vec::new(),
            nets: Vec::new(),
            profile: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            text_boxes: Vec::new(),
            footprints: Vec::new(),
            vias: Vec::new(),
            segments: Vec::new(),
            arcs: Vec::new(),
            zones: Vec::new(),
            rule_areas: Vec::new(),
            groups: Vec::new(),
            embedded_files: Vec::new(),
        }
    }
}

impl AuthoredPcb {
    pub fn canonical_text(&self, limits: PcbAuthoringLimits) -> Result<String, Error> {
        emit::board_text(self, limits)
    }

    pub fn to_document(&self, limits: PcbAuthoringLimits) -> Result<PcbDocument, Error> {
        PcbDocument::parse(self.canonical_text(limits)?, limits.pcb_limits)
    }
}

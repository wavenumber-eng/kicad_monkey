/// Error types.
pub mod error {
    /// Error from a `TryFrom` or `FromStr` implementation.
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
///Solid three-point arc.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Solid three-point arc.",
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "fill",
///    "index",
///    "kind",
///    "mid_x",
///    "mid_y",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "ArcThreePoint"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "mid_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "mid_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ArcThreePointOperation {
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_arc_three_point_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub mid_x: crate::JavaScriptSafeInteger,
    pub mid_y: crate::JavaScriptSafeInteger,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Cubic Bézier shared by symbol and schematic producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Cubic Bézier shared by symbol and schematic producers.",
///  "type": "object",
///  "required": [
///    "ctrl1_x",
///    "ctrl1_y",
///    "ctrl2_x",
///    "ctrl2_y",
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "start_x",
///    "start_y",
///    "tolerance_nm",
///    "width_nm"
///  ],
///  "properties": {
///    "ctrl1_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl1_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl2_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl2_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "BezierCurve"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "tolerance_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BezierCurveOperation {
    pub ctrl1_x: crate::JavaScriptSafeInteger,
    pub ctrl1_y: crate::JavaScriptSafeInteger,
    pub ctrl2_x: crate::JavaScriptSafeInteger,
    pub ctrl2_y: crate::JavaScriptSafeInteger,
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_bezier_curve_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub tolerance_nm: crate::JavaScriptSafeInteger,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Board dimension construction styles supported by KiCad's PCB plotter.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Board dimension construction styles supported by KiCad's PCB plotter.",
///  "type": "string",
///  "enum": [
///    "aligned",
///    "orthogonal",
///    "radial",
///    "leader",
///    "center"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardDimensionType {
    #[serde(rename = "aligned")]
    Aligned,
    #[serde(rename = "orthogonal")]
    Orthogonal,
    #[serde(rename = "radial")]
    Radial,
    #[serde(rename = "leader")]
    Leader,
    #[serde(rename = "center")]
    Center,
}
impl ::std::fmt::Display for BoardDimensionType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Aligned => f.write_str("aligned"),
            Self::Orthogonal => f.write_str("orthogonal"),
            Self::Radial => f.write_str("radial"),
            Self::Leader => f.write_str("leader"),
            Self::Center => f.write_str("center"),
        }
    }
}
impl ::std::str::FromStr for BoardDimensionType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "aligned" => Ok(Self::Aligned),
            "orthogonal" => Ok(Self::Orthogonal),
            "radial" => Ok(Self::Radial),
            "leader" => Ok(Self::Leader),
            "center" => Ok(Self::Center),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardDimensionType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardDimensionType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardDimensionType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintArcThreePointOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "fill",
///    "index",
///    "kind",
///    "mid_x",
///    "mid_y",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "ArcThreePoint"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "mid_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "mid_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintArcThreePointOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_arc_three_point_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub mid_x: crate::JavaScriptSafeInteger,
    pub mid_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///`BoardFootprintBezierCurveOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "ctrl1_x",
///    "ctrl1_y",
///    "ctrl2_x",
///    "ctrl2_y",
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "start_x",
///    "start_y",
///    "tolerance_nm",
///    "width_nm"
///  ],
///  "properties": {
///    "ctrl1_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl1_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl2_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl2_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "BezierCurve"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "tolerance_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintBezierCurveOperation {
    pub ctrl1_x: crate::JavaScriptSafeInteger,
    pub ctrl1_y: crate::JavaScriptSafeInteger,
    pub ctrl2_x: crate::JavaScriptSafeInteger,
    pub ctrl2_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_bezier_curve_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub tolerance_nm: crate::JavaScriptSafeInteger,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///SVG-enrichment metadata retained on one embedded-footprint child operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "SVG-enrichment metadata retained on one embedded-footprint child operation.",
///  "type": "object",
///  "required": [
///    "component",
///    "component_uid",
///    "component_uuid",
///    "footprint",
///    "footprint_object_index",
///    "footprint_primitive",
///    "primitive"
///  ],
///  "properties": {
///    "component": {
///      "type": "string"
///    },
///    "component_uid": {
///      "type": "string"
///    },
///    "component_uuid": {
///      "type": "string"
///    },
///    "footprint": {
///      "type": "string"
///    },
///    "footprint_graphic_kind": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "text-box-border"
///        },
///        {
///          "type": "string",
///          "const": "line"
///        },
///        {
///          "type": "string",
///          "const": "arc"
///        },
///        {
///          "type": "string",
///          "const": "circle"
///        },
///        {
///          "type": "string",
///          "const": "rect"
///        },
///        {
///          "type": "string",
///          "const": "poly"
///        }
///      ]
///    },
///    "footprint_object_index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "footprint_primitive": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "footprint_subop_index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "footprint_text_role": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "designator"
///        },
///        {
///          "type": "string",
///          "const": "value"
///        },
///        {
///          "type": "string",
///          "const": "property"
///        },
///        {
///          "type": "string",
///          "const": "user"
///        }
///      ]
///    },
///    "fp_text_type": {
///      "type": "string"
///    },
///    "layer_name": {
///      "type": "string"
///    },
///    "layer_role": {
///      "$ref": "#/$defs/BoardFootprintLayerRole"
///    },
///    "primitive": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "footprint-text"
///        },
///        {
///          "type": "string",
///          "const": "footprint-graphic"
///        }
///      ]
///    },
///    "property_name": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintChildAttrs {
    pub component: ::std::string::String,
    pub component_uid: ::std::string::String,
    pub component_uuid: ::std::string::String,
    pub footprint: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub footprint_graphic_kind: ::std::option::Option<BoardFootprintChildAttrsFootprintGraphicKind>,
    pub footprint_object_index: u32,
    pub footprint_primitive: BoardFootprintChildRef,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub footprint_subop_index: ::std::option::Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub footprint_text_role: ::std::option::Option<BoardFootprintChildAttrsFootprintTextRole>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fp_text_type: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer_name: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer_role: ::std::option::Option<BoardFootprintLayerRole>,
    pub primitive: BoardFootprintChildAttrsPrimitive,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub property_name: ::std::option::Option<::std::string::String>,
}
///`BoardFootprintChildAttrsFootprintGraphicKind`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "text-box-border"
///    },
///    {
///      "type": "string",
///      "const": "line"
///    },
///    {
///      "type": "string",
///      "const": "arc"
///    },
///    {
///      "type": "string",
///      "const": "circle"
///    },
///    {
///      "type": "string",
///      "const": "rect"
///    },
///    {
///      "type": "string",
///      "const": "poly"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintChildAttrsFootprintGraphicKind {
    #[serde(rename = "text-box-border")]
    TextBoxBorder,
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "arc")]
    Arc,
    #[serde(rename = "circle")]
    Circle,
    #[serde(rename = "rect")]
    Rect,
    #[serde(rename = "poly")]
    Poly,
}
impl ::std::fmt::Display for BoardFootprintChildAttrsFootprintGraphicKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::TextBoxBorder => f.write_str("text-box-border"),
            Self::Line => f.write_str("line"),
            Self::Arc => f.write_str("arc"),
            Self::Circle => f.write_str("circle"),
            Self::Rect => f.write_str("rect"),
            Self::Poly => f.write_str("poly"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintChildAttrsFootprintGraphicKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "text-box-border" => Ok(Self::TextBoxBorder),
            "line" => Ok(Self::Line),
            "arc" => Ok(Self::Arc),
            "circle" => Ok(Self::Circle),
            "rect" => Ok(Self::Rect),
            "poly" => Ok(Self::Poly),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintChildAttrsFootprintGraphicKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
    for BoardFootprintChildAttrsFootprintGraphicKind
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for BoardFootprintChildAttrsFootprintGraphicKind
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintChildAttrsFootprintTextRole`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "designator"
///    },
///    {
///      "type": "string",
///      "const": "value"
///    },
///    {
///      "type": "string",
///      "const": "property"
///    },
///    {
///      "type": "string",
///      "const": "user"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintChildAttrsFootprintTextRole {
    #[serde(rename = "designator")]
    Designator,
    #[serde(rename = "value")]
    Value,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "user")]
    User,
}
impl ::std::fmt::Display for BoardFootprintChildAttrsFootprintTextRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Designator => f.write_str("designator"),
            Self::Value => f.write_str("value"),
            Self::Property => f.write_str("property"),
            Self::User => f.write_str("user"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintChildAttrsFootprintTextRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "designator" => Ok(Self::Designator),
            "value" => Ok(Self::Value),
            "property" => Ok(Self::Property),
            "user" => Ok(Self::User),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintChildAttrsFootprintTextRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintChildAttrsFootprintTextRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintChildAttrsFootprintTextRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintChildAttrsPrimitive`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "footprint-text"
///    },
///    {
///      "type": "string",
///      "const": "footprint-graphic"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintChildAttrsPrimitive {
    #[serde(rename = "footprint-text")]
    FootprintText,
    #[serde(rename = "footprint-graphic")]
    FootprintGraphic,
}
impl ::std::fmt::Display for BoardFootprintChildAttrsPrimitive {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::FootprintText => f.write_str("footprint-text"),
            Self::FootprintGraphic => f.write_str("footprint-graphic"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintChildAttrsPrimitive {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "footprint-text" => Ok(Self::FootprintText),
            "footprint-graphic" => Ok(Self::FootprintGraphic),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintChildAttrsPrimitive {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintChildAttrsPrimitive {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintChildAttrsPrimitive {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Source child kinds emitted directly on embedded-footprint drawing operations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Source child kinds emitted directly on embedded-footprint drawing operations.",
///  "type": "string",
///  "enum": [
///    "property",
///    "fp_text",
///    "fp_text_box",
///    "fp_line",
///    "fp_arc",
///    "fp_circle",
///    "fp_rect",
///    "fp_poly"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintChildRef {
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "fp_text")]
    FpText,
    #[serde(rename = "fp_text_box")]
    FpTextBox,
    #[serde(rename = "fp_line")]
    FpLine,
    #[serde(rename = "fp_arc")]
    FpArc,
    #[serde(rename = "fp_circle")]
    FpCircle,
    #[serde(rename = "fp_rect")]
    FpRect,
    #[serde(rename = "fp_poly")]
    FpPoly,
}
impl ::std::fmt::Display for BoardFootprintChildRef {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Property => f.write_str("property"),
            Self::FpText => f.write_str("fp_text"),
            Self::FpTextBox => f.write_str("fp_text_box"),
            Self::FpLine => f.write_str("fp_line"),
            Self::FpArc => f.write_str("fp_arc"),
            Self::FpCircle => f.write_str("fp_circle"),
            Self::FpRect => f.write_str("fp_rect"),
            Self::FpPoly => f.write_str("fp_poly"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintChildRef {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "property" => Ok(Self::Property),
            "fp_text" => Ok(Self::FpText),
            "fp_text_box" => Ok(Self::FpTextBox),
            "fp_line" => Ok(Self::FpLine),
            "fp_arc" => Ok(Self::FpArc),
            "fp_circle" => Ok(Self::FpCircle),
            "fp_rect" => Ok(Self::FpRect),
            "fp_poly" => Ok(Self::FpPoly),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintChildRef {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintChildRef {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintChildRef {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintCircleOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "cx",
///    "cy",
///    "diameter_nm",
///    "fill",
///    "index",
///    "kind",
///    "width_nm"
///  ],
///  "properties": {
///    "cx": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "cy": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "diameter_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "Circle"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "pad_size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterDrillRole"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintCircleOperation {
    pub cx: crate::JavaScriptSafeInteger,
    pub cy: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    pub diameter_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_circle_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub role: ::std::option::Option<PlotterDrillRole>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Closing operation for one embedded pad or drill SVG group.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Closing operation for one embedded pad or drill SVG group.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind"
///  ],
///  "properties": {
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "EndBlock"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintEndBlockOperation {
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_end_block_kind")]
    pub kind: ::std::string::String,
}
///`BoardFootprintFlashPadCircleOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "diameter_nm",
///    "index",
///    "kind",
///    "layers",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "diameter_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadCircle"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterViaFlashRole"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintFlashPadCircleOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    pub diameter_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_circle_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub role: ::std::option::Option<PlotterViaFlashRole>,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///`BoardFootprintFlashPadCustomOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "polygons",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "anchor_shape": {
///      "type": "string"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadCustom"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "polygon_widths_nm": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/JavaScriptSafeInteger"
///      }
///    },
///    "polygons": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintFlashPadCustomOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub anchor_shape: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_custom_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub orient_deg: f64,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub polygon_widths_nm: ::std::vec::Vec<crate::JavaScriptSafeInteger>,
    pub polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///`BoardFootprintFlashPadOvalOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadOval"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintFlashPadOvalOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_oval_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///`BoardFootprintFlashPadRectOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadRect"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintFlashPadRectOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_rect_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///`BoardFootprintFlashPadRoundRectOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "corner_radius_nm",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "corner_radius_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadRoundRect"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintFlashPadRoundRectOperation {
    pub corner_radius_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_round_rect_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///`BoardFootprintFlashPadTrapezOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "corners",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "corners": {
///      "$ref": "#/$defs/PlotterQuad"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadTrapez"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintFlashPadTrapezOperation {
    pub corners: PlotterQuad,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_trapez_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub orient_deg: f64,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Normalized PCB layer roles mirrored by enriched footprint-child metadata.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Normalized PCB layer roles mirrored by enriched footprint-child metadata.",
///  "type": "string",
///  "enum": [
///    "copper",
///    "silkscreen",
///    "soldermask",
///    "paste",
///    "fab",
///    "courtyard",
///    "board-outline",
///    "drill",
///    "user",
///    "other"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintLayerRole {
    #[serde(rename = "copper")]
    Copper,
    #[serde(rename = "silkscreen")]
    Silkscreen,
    #[serde(rename = "soldermask")]
    Soldermask,
    #[serde(rename = "paste")]
    Paste,
    #[serde(rename = "fab")]
    Fab,
    #[serde(rename = "courtyard")]
    Courtyard,
    #[serde(rename = "board-outline")]
    BoardOutline,
    #[serde(rename = "drill")]
    Drill,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "other")]
    Other,
}
impl ::std::fmt::Display for BoardFootprintLayerRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Copper => f.write_str("copper"),
            Self::Silkscreen => f.write_str("silkscreen"),
            Self::Soldermask => f.write_str("soldermask"),
            Self::Paste => f.write_str("paste"),
            Self::Fab => f.write_str("fab"),
            Self::Courtyard => f.write_str("courtyard"),
            Self::BoardOutline => f.write_str("board-outline"),
            Self::Drill => f.write_str("drill"),
            Self::User => f.write_str("user"),
            Self::Other => f.write_str("other"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintLayerRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "copper" => Ok(Self::Copper),
            "silkscreen" => Ok(Self::Silkscreen),
            "soldermask" => Ok(Self::Soldermask),
            "paste" => Ok(Self::Paste),
            "fab" => Ok(Self::Fab),
            "courtyard" => Ok(Self::Courtyard),
            "board-outline" => Ok(Self::BoardOutline),
            "drill" => Ok(Self::Drill),
            "user" => Ok(Self::User),
            "other" => Ok(Self::Other),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintLayerRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintLayerRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintLayerRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Strict operation vocabulary for one board-embedded footprint record.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Strict operation vocabulary for one board-embedded footprint record.",
///  "oneOf": [
///    {
///      "$ref": "#/$defs/BoardFootprintThickSegmentOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintArcThreePointOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintCircleOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintRectOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintPlotPolyOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintBezierCurveOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintTextOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintFlashPadCircleOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintFlashPadOvalOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintFlashPadRectOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintFlashPadRoundRectOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintFlashPadCustomOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintFlashPadTrapezOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintStartBlockOperation"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintEndBlockOperation"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum BoardFootprintOperation {
    ThickSegmentOperation(BoardFootprintThickSegmentOperation),
    ArcThreePointOperation(BoardFootprintArcThreePointOperation),
    CircleOperation(BoardFootprintCircleOperation),
    RectOperation(BoardFootprintRectOperation),
    PlotPolyOperation(BoardFootprintPlotPolyOperation),
    BezierCurveOperation(BoardFootprintBezierCurveOperation),
    TextOperation(BoardFootprintTextOperation),
    FlashPadCircleOperation(BoardFootprintFlashPadCircleOperation),
    FlashPadOvalOperation(BoardFootprintFlashPadOvalOperation),
    FlashPadRectOperation(BoardFootprintFlashPadRectOperation),
    FlashPadRoundRectOperation(BoardFootprintFlashPadRoundRectOperation),
    FlashPadCustomOperation(BoardFootprintFlashPadCustomOperation),
    FlashPadTrapezOperation(BoardFootprintFlashPadTrapezOperation),
    StartBlockOperation(BoardFootprintStartBlockOperation),
    EndBlockOperation(BoardFootprintEndBlockOperation),
}
impl ::std::convert::From<BoardFootprintThickSegmentOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintThickSegmentOperation) -> Self {
        Self::ThickSegmentOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintArcThreePointOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintArcThreePointOperation) -> Self {
        Self::ArcThreePointOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintCircleOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintCircleOperation) -> Self {
        Self::CircleOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintRectOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintRectOperation) -> Self {
        Self::RectOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintPlotPolyOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintPlotPolyOperation) -> Self {
        Self::PlotPolyOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintBezierCurveOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintBezierCurveOperation) -> Self {
        Self::BezierCurveOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintTextOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintTextOperation) -> Self {
        Self::TextOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintFlashPadCircleOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintFlashPadCircleOperation) -> Self {
        Self::FlashPadCircleOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintFlashPadOvalOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintFlashPadOvalOperation) -> Self {
        Self::FlashPadOvalOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintFlashPadRectOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintFlashPadRectOperation) -> Self {
        Self::FlashPadRectOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintFlashPadRoundRectOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintFlashPadRoundRectOperation) -> Self {
        Self::FlashPadRoundRectOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintFlashPadCustomOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintFlashPadCustomOperation) -> Self {
        Self::FlashPadCustomOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintFlashPadTrapezOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintFlashPadTrapezOperation) -> Self {
        Self::FlashPadTrapezOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintStartBlockOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintStartBlockOperation) -> Self {
        Self::StartBlockOperation(value)
    }
}
impl ::std::convert::From<BoardFootprintEndBlockOperation> for BoardFootprintOperation {
    fn from(value: BoardFootprintEndBlockOperation) -> Self {
        Self::EndBlockOperation(value)
    }
}
///Stringified SVG-enrichment attributes on an embedded pad block.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Stringified SVG-enrichment attributes on an embedded pad block.",
///  "type": "object",
///  "required": [
///    "primitive"
///  ],
///  "properties": {
///    "component": {
///      "type": "string"
///    },
///    "component_uid": {
///      "type": "string"
///    },
///    "component_uuid": {
///      "type": "string"
///    },
///    "footprint": {
///      "type": "string"
///    },
///    "hole_diameter_mm": {
///      "type": "string"
///    },
///    "hole_height_mm": {
///      "type": "string"
///    },
///    "hole_kind": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "round"
///        },
///        {
///          "type": "string",
///          "const": "slot"
///        }
///      ]
///    },
///    "hole_owner": {
///      "type": "string"
///    },
///    "hole_plating": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "plated"
///        },
///        {
///          "type": "string",
///          "const": "non_plated"
///        }
///      ]
///    },
///    "hole_render": {
///      "type": "string",
///      "const": "drill"
///    },
///    "hole_width_mm": {
///      "type": "string"
///    },
///    "layer_names": {
///      "type": "string"
///    },
///    "net": {
///      "type": "string"
///    },
///    "net_class": {
///      "type": "string"
///    },
///    "net_classes": {
///      "type": "string"
///    },
///    "net_id": {
///      "type": "string"
///    },
///    "net_index": {
///      "type": "string"
///    },
///    "pad_designator": {
///      "type": "string"
///    },
///    "pad_number": {
///      "type": "string"
///    },
///    "pad_shape": {
///      "type": "string"
///    },
///    "pad_type": {
///      "type": "string"
///    },
///    "primitive": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "pad"
///        },
///        {
///          "type": "string",
///          "const": "pad-hole"
///        }
///      ]
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintPadBlockAttrs {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub component: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub component_uid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub component_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub footprint: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_diameter_mm: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_height_mm: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_kind: ::std::option::Option<BoardFootprintPadBlockAttrsHoleKind>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_owner: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_plating: ::std::option::Option<BoardFootprintPadBlockAttrsHolePlating>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_render: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub hole_width_mm: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer_names: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_class: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_classes: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_id: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_index: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_designator: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_number: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_shape: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_type: ::std::option::Option<::std::string::String>,
    pub primitive: BoardFootprintPadBlockAttrsPrimitive,
}
///`BoardFootprintPadBlockAttrsHoleKind`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "round"
///    },
///    {
///      "type": "string",
///      "const": "slot"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintPadBlockAttrsHoleKind {
    #[serde(rename = "round")]
    Round,
    #[serde(rename = "slot")]
    Slot,
}
impl ::std::fmt::Display for BoardFootprintPadBlockAttrsHoleKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Round => f.write_str("round"),
            Self::Slot => f.write_str("slot"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintPadBlockAttrsHoleKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "round" => Ok(Self::Round),
            "slot" => Ok(Self::Slot),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintPadBlockAttrsHoleKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintPadBlockAttrsHoleKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintPadBlockAttrsHoleKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintPadBlockAttrsHolePlating`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "plated"
///    },
///    {
///      "type": "string",
///      "const": "non_plated"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintPadBlockAttrsHolePlating {
    #[serde(rename = "plated")]
    Plated,
    #[serde(rename = "non_plated")]
    NonPlated,
}
impl ::std::fmt::Display for BoardFootprintPadBlockAttrsHolePlating {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Plated => f.write_str("plated"),
            Self::NonPlated => f.write_str("non_plated"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintPadBlockAttrsHolePlating {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "plated" => Ok(Self::Plated),
            "non_plated" => Ok(Self::NonPlated),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintPadBlockAttrsHolePlating {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintPadBlockAttrsHolePlating {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintPadBlockAttrsHolePlating {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintPadBlockAttrsPrimitive`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "pad"
///    },
///    {
///      "type": "string",
///      "const": "pad-hole"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintPadBlockAttrsPrimitive {
    #[serde(rename = "pad")]
    Pad,
    #[serde(rename = "pad-hole")]
    PadHole,
}
impl ::std::fmt::Display for BoardFootprintPadBlockAttrsPrimitive {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pad => f.write_str("pad"),
            Self::PadHole => f.write_str("pad-hole"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintPadBlockAttrsPrimitive {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pad" => Ok(Self::Pad),
            "pad-hole" => Ok(Self::PadHole),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintPadBlockAttrsPrimitive {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintPadBlockAttrsPrimitive {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintPadBlockAttrsPrimitive {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Footprint-local placement applied by board renderers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Footprint-local placement applied by board renderers.",
///  "type": "object",
///  "required": [
///    "angle_deg",
///    "x_nm",
///    "y_nm"
///  ],
///  "properties": {
///    "angle_deg": {
///      "type": "number"
///    },
///    "x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintPlacement {
    pub angle_deg: f64,
    pub x_nm: crate::JavaScriptSafeInteger,
    pub y_nm: crate::JavaScriptSafeInteger,
}
///`BoardFootprintPlotPolyOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "fill",
///    "index",
///    "kind",
///    "points",
///    "width_nm"
///  ],
///  "properties": {
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "PlotPoly"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "points": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterPoint"
///      }
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintPlotPolyOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_plot_poly_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub points: ::std::vec::Vec<PlotterPoint>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///One board-embedded footprint in canonical child and pad-block order.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board-embedded footprint in canonical child and pad-block order.",
///  "type": "object",
///  "required": [
///    "attr",
///    "descr",
///    "kind",
///    "layer",
///    "library_link",
///    "locked",
///    "object_id",
///    "operation_count",
///    "operations",
///    "placement",
///    "reference",
///    "tags",
///    "uuid",
///    "value"
///  ],
///  "properties": {
///    "attr": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "descr": {
///      "type": "string"
///    },
///    "kind": {
///      "type": "string",
///      "const": "footprint"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "library_link": {
///      "type": "string"
///    },
///    "locked": {
///      "type": "boolean"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/BoardFootprintOperation"
///      }
///    },
///    "placement": {
///      "$ref": "#/$defs/BoardFootprintPlacement"
///    },
///    "reference": {
///      "type": "string"
///    },
///    "tags": {
///      "type": "string"
///    },
///    "uuid": {
///      "type": "string"
///    },
///    "value": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintPlotRecord {
    pub attr: ::std::vec::Vec<::std::string::String>,
    pub descr: ::std::string::String,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub library_link: ::std::string::String,
    pub locked: bool,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<BoardFootprintOperation>,
    pub placement: BoardFootprintPlacement,
    pub reference: ::std::string::String,
    pub tags: ::std::string::String,
    pub uuid: ::std::string::String,
    pub value: ::std::string::String,
}
///`BoardFootprintRectOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "corner_radius_nm",
///    "fill",
///    "index",
///    "kind",
///    "width_nm",
///    "x1",
///    "x2",
///    "y1",
///    "y2"
///  ],
///  "properties": {
///    "corner_radius_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "Rect"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x1": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x2": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y1": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y2": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintRectOperation {
    pub corner_radius_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_rect_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
    pub x1: crate::JavaScriptSafeInteger,
    pub x2: crate::JavaScriptSafeInteger,
    pub y1: crate::JavaScriptSafeInteger,
    pub y2: crate::JavaScriptSafeInteger,
}
///Opening operation for one embedded pad or drill SVG group.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Opening operation for one embedded pad or drill SVG group.",
///  "type": "object",
///  "required": [
///    "data_ref",
///    "data_uuid",
///    "extra_attrs",
///    "index",
///    "kind",
///    "label",
///    "object_id"
///  ],
///  "properties": {
///    "data_ref": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "pad"
///        },
///        {
///          "type": "string",
///          "const": "pad_hole"
///        }
///      ]
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintPadBlockAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "StartBlock"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "object_id": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintStartBlockOperation {
    pub data_ref: BoardFootprintStartBlockOperationDataRef,
    pub data_uuid: ::std::string::String,
    pub extra_attrs: BoardFootprintPadBlockAttrs,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_start_block_kind")]
    pub kind: ::std::string::String,
    pub label: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub object_id: ::std::string::String,
}
///`BoardFootprintStartBlockOperationDataRef`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "pad"
///    },
///    {
///      "type": "string",
///      "const": "pad_hole"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardFootprintStartBlockOperationDataRef {
    #[serde(rename = "pad")]
    Pad,
    #[serde(rename = "pad_hole")]
    PadHole,
}
impl ::std::fmt::Display for BoardFootprintStartBlockOperationDataRef {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pad => f.write_str("pad"),
            Self::PadHole => f.write_str("pad_hole"),
        }
    }
}
impl ::std::str::FromStr for BoardFootprintStartBlockOperationDataRef {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pad" => Ok(Self::Pad),
            "pad_hole" => Ok(Self::PadHole),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardFootprintStartBlockOperationDataRef {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardFootprintStartBlockOperationDataRef {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardFootprintStartBlockOperationDataRef {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`BoardFootprintTextOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "bold",
///    "color",
///    "font_face",
///    "h_align",
///    "index",
///    "italic",
///    "kind",
///    "multiline",
///    "orient_deg",
///    "pen_width_nm",
///    "size_x_nm",
///    "size_y_nm",
///    "text",
///    "v_align",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "bold": {
///      "type": "boolean"
///    },
///    "color": {
///      "type": "string"
///    },
///    "context": {
///      "$ref": "#/$defs/PlotterOperationContext"
///    },
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "font_face": {
///      "type": "string"
///    },
///    "h_align": {
///      "$ref": "#/$defs/PlotterTextHAlign"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "italic": {
///      "type": "boolean"
///    },
///    "kind": {
///      "type": "string",
///      "const": "Text"
///    },
///    "knockout": {
///      "type": "boolean"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "mirror": {
///      "type": "boolean"
///    },
///    "multiline": {
///      "type": "boolean"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "pen_width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "polyline_per_segment": {
///      "type": "boolean"
///    },
///    "render_cache": {
///      "$ref": "#/$defs/TextRenderCache"
///    },
///    "render_cache_exact": {
///      "type": "boolean"
///    },
///    "render_cache_polygons": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    },
///    "render_cache_source": {
///      "$ref": "#/$defs/PlotterTextRenderCacheSource"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "text": {
///      "type": "string"
///    },
///    "text_as_polygons": {
///      "type": "boolean"
///    },
///    "v_align": {
///      "$ref": "#/$defs/PlotterTextVAlign"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintTextOperation {
    pub bold: bool,
    pub color: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub context: ::std::option::Option<PlotterOperationContext>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub font_face: ::std::string::String,
    pub h_align: PlotterTextHAlign,
    pub index: u32,
    pub italic: bool,
    #[serde(deserialize_with = "crate::deserialize_text_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub knockout: ::std::option::Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mirror: ::std::option::Option<bool>,
    pub multiline: bool,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    pub orient_deg: f64,
    pub pen_width_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub polyline_per_segment: ::std::option::Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub render_cache: ::std::option::Option<TextRenderCache>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub render_cache_exact: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub render_cache_source: ::std::option::Option<PlotterTextRenderCacheSource>,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub text: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub text_as_polygons: ::std::option::Option<bool>,
    pub v_align: PlotterTextVAlign,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///`BoardFootprintThickSegmentOperation`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "data_ref": {
///      "$ref": "#/$defs/BoardFootprintChildRef"
///    },
///    "data_uuid": {
///      "type": "string"
///    },
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "extra_attrs": {
///      "$ref": "#/$defs/BoardFootprintChildAttrs"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "ThickSegment"
///    },
///    "label": {
///      "type": "string"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "pad_size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterDrillRole"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardFootprintThickSegmentOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_ref: ::std::option::Option<BoardFootprintChildRef>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub data_uuid: ::std::option::Option<::std::string::String>,
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub extra_attrs: ::std::option::Option<BoardFootprintChildAttrs>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_thick_segment_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub label: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub object_id: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub role: ::std::option::Option<PlotterDrillRole>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
/**One board-level graphic record. The carrier layer travels on the record;
the contained operations are layerless graphic-state operations.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board-level graphic record. The carrier layer travels on the record;\nthe contained operations are layerless graphic-state operations.",
///  "type": "object",
///  "required": [
///    "kind",
///    "layer",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "$ref": "#/$defs/BoardGraphicRecordKind"
///    },
///    "layer": {
///      "anyOf": [
///        {
///          "type": "string"
///        },
///        {
///          "type": "null"
///        }
///      ]
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardGraphicPlotRecord {
    pub kind: BoardGraphicRecordKind,
    #[serde(deserialize_with = "crate::deserialize_required_nullable")]
    pub layer: ::std::option::Option<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
///Board graphic record kinds promoted in the first board slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Board graphic record kinds promoted in the first board slice.",
///  "type": "string",
///  "enum": [
///    "gr_line",
///    "gr_arc",
///    "gr_circle",
///    "gr_rect",
///    "gr_poly",
///    "gr_curve"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardGraphicRecordKind {
    #[serde(rename = "gr_line")]
    GrLine,
    #[serde(rename = "gr_arc")]
    GrArc,
    #[serde(rename = "gr_circle")]
    GrCircle,
    #[serde(rename = "gr_rect")]
    GrRect,
    #[serde(rename = "gr_poly")]
    GrPoly,
    #[serde(rename = "gr_curve")]
    GrCurve,
}
impl ::std::fmt::Display for BoardGraphicRecordKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GrLine => f.write_str("gr_line"),
            Self::GrArc => f.write_str("gr_arc"),
            Self::GrCircle => f.write_str("gr_circle"),
            Self::GrRect => f.write_str("gr_rect"),
            Self::GrPoly => f.write_str("gr_poly"),
            Self::GrCurve => f.write_str("gr_curve"),
        }
    }
}
impl ::std::str::FromStr for BoardGraphicRecordKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "gr_line" => Ok(Self::GrLine),
            "gr_arc" => Ok(Self::GrArc),
            "gr_circle" => Ok(Self::GrCircle),
            "gr_rect" => Ok(Self::GrRect),
            "gr_poly" => Ok(Self::GrPoly),
            "gr_curve" => Ok(Self::GrCurve),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardGraphicRecordKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardGraphicRecordKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardGraphicRecordKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
/**Strict board graphics, text, tracks, vias, tables, dimensions, authored
zone fills, and embedded footprints subset of kicad.plotter_ir.a0. Producers
and consumers must run generated semantic validation after structural decoding.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.board_plot.document:a0",
///  "title": "Board plot document a0",
///  "description": "Strict board graphics, text, tracks, vias, tables, dimensions, authored\nzone fills, and embedded footprints subset of kicad.plotter_ir.a0. Producers\nand consumers must run generated semantic validation after structural decoding.",
///  "type": "object",
///  "required": [
///    "coordinate_space",
///    "document_id",
///    "generator",
///    "generator_version",
///    "paper",
///    "records",
///    "schema",
///    "source_kind",
///    "thickness_mm",
///    "total_operations",
///    "version"
///  ],
///  "properties": {
///    "coordinate_space": {
///      "$ref": "#/$defs/PlotterCoordinateSpace"
///    },
///    "document_id": {
///      "type": "string"
///    },
///    "generator": {
///      "type": "string"
///    },
///    "generator_version": {
///      "type": "string"
///    },
///    "paper": {
///      "type": "string"
///    },
///    "records": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/BoardPlotRecord"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad.plotter_ir.a0"
///    },
///    "source_kind": {
///      "type": "string",
///      "const": "PCB"
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "thickness_mm": {
///      "type": "number"
///    },
///    "total_operations": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "version": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardPlotDocumentA0 {
    pub coordinate_space: PlotterCoordinateSpace,
    pub document_id: ::std::string::String,
    pub generator: ::std::string::String,
    pub generator_version: ::std::string::String,
    pub paper: ::std::string::String,
    pub records: ::std::vec::Vec<BoardPlotRecord>,
    pub schema: ::std::string::String,
    pub source_kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub source_path: ::std::option::Option<::std::string::String>,
    pub thickness_mm: f64,
    pub total_operations: u32,
    pub version: crate::JavaScriptSafeInteger,
}
///`BoardPlotRecord`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "oneOf": [
///    {
///      "$ref": "#/$defs/BoardGraphicPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/TrackSegmentPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/TrackArcPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/ViaPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/TablePlotRecord"
///    },
///    {
///      "$ref": "#/$defs/DimensionPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/ZoneFillPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/BoardTextPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/BoardTextBoxPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/BoardFootprintPlotRecord"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum BoardPlotRecord {
    BoardGraphicPlotRecord(BoardGraphicPlotRecord),
    TrackSegmentPlotRecord(TrackSegmentPlotRecord),
    TrackArcPlotRecord(TrackArcPlotRecord),
    ViaPlotRecord(ViaPlotRecord),
    TablePlotRecord(TablePlotRecord),
    DimensionPlotRecord(DimensionPlotRecord),
    ZoneFillPlotRecord(ZoneFillPlotRecord),
    BoardTextPlotRecord(BoardTextPlotRecord),
    BoardTextBoxPlotRecord(BoardTextBoxPlotRecord),
    BoardFootprintPlotRecord(BoardFootprintPlotRecord),
}
impl ::std::convert::From<BoardGraphicPlotRecord> for BoardPlotRecord {
    fn from(value: BoardGraphicPlotRecord) -> Self {
        Self::BoardGraphicPlotRecord(value)
    }
}
impl ::std::convert::From<TrackSegmentPlotRecord> for BoardPlotRecord {
    fn from(value: TrackSegmentPlotRecord) -> Self {
        Self::TrackSegmentPlotRecord(value)
    }
}
impl ::std::convert::From<TrackArcPlotRecord> for BoardPlotRecord {
    fn from(value: TrackArcPlotRecord) -> Self {
        Self::TrackArcPlotRecord(value)
    }
}
impl ::std::convert::From<ViaPlotRecord> for BoardPlotRecord {
    fn from(value: ViaPlotRecord) -> Self {
        Self::ViaPlotRecord(value)
    }
}
impl ::std::convert::From<TablePlotRecord> for BoardPlotRecord {
    fn from(value: TablePlotRecord) -> Self {
        Self::TablePlotRecord(value)
    }
}
impl ::std::convert::From<DimensionPlotRecord> for BoardPlotRecord {
    fn from(value: DimensionPlotRecord) -> Self {
        Self::DimensionPlotRecord(value)
    }
}
impl ::std::convert::From<ZoneFillPlotRecord> for BoardPlotRecord {
    fn from(value: ZoneFillPlotRecord) -> Self {
        Self::ZoneFillPlotRecord(value)
    }
}
impl ::std::convert::From<BoardTextPlotRecord> for BoardPlotRecord {
    fn from(value: BoardTextPlotRecord) -> Self {
        Self::BoardTextPlotRecord(value)
    }
}
impl ::std::convert::From<BoardTextBoxPlotRecord> for BoardPlotRecord {
    fn from(value: BoardTextBoxPlotRecord) -> Self {
        Self::BoardTextBoxPlotRecord(value)
    }
}
impl ::std::convert::From<BoardFootprintPlotRecord> for BoardPlotRecord {
    fn from(value: BoardFootprintPlotRecord) -> Self {
        Self::BoardFootprintPlotRecord(value)
    }
}
/**One board text-box record. A visible border contributes a leading Rect
operation; empty resolved text drops the Text operation.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board text-box record. A visible border contributes a leading Rect\noperation; empty resolved text drops the Text operation.",
///  "type": "object",
///  "required": [
///    "border",
///    "kind",
///    "layer",
///    "object_id",
///    "operation_count",
///    "operations",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "border": {
///      "type": "boolean"
///    },
///    "kind": {
///      "type": "string",
///      "const": "gr_text_box"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "text": {
///      "type": "string"
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardTextBoxPlotRecord {
    pub border: bool,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
/**One board free-text record. `hide` mirrors the established serializer's
getattr default and is always false for board gr_text carriers.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board free-text record. `hide` mirrors the established serializer's\ngetattr default and is always false for board gr_text carriers.",
///  "type": "object",
///  "required": [
///    "hide",
///    "kind",
///    "layer",
///    "object_id",
///    "operation_count",
///    "operations",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "hide": {
///      "type": "boolean"
///    },
///    "kind": {
///      "type": "string",
///      "const": "gr_text"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "text": {
///      "type": "string"
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardTextPlotRecord {
    pub hide: bool,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
///Via construction kinds mirrored from the established producer.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Via construction kinds mirrored from the established producer.",
///  "type": "string",
///  "enum": [
///    "through",
///    "blind",
///    "buried",
///    "micro"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum BoardViaType {
    #[serde(rename = "through")]
    Through,
    #[serde(rename = "blind")]
    Blind,
    #[serde(rename = "buried")]
    Buried,
    #[serde(rename = "micro")]
    Micro,
}
impl ::std::fmt::Display for BoardViaType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Through => f.write_str("through"),
            Self::Blind => f.write_str("blind"),
            Self::Buried => f.write_str("buried"),
            Self::Micro => f.write_str("micro"),
        }
    }
}
impl ::std::str::FromStr for BoardViaType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "through" => Ok(Self::Through),
            "blind" => Ok(Self::Blind),
            "buried" => Ok(Self::Buried),
            "micro" => Ok(Self::Micro),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for BoardViaType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for BoardViaType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for BoardViaType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
/**Circle shared by graphical and drill producers. Graphic state requires only
layer. Drill state requires role plus layers; NPTH state additionally
requires all mask and pad-size hints. The generated semantic validator
enforces these mutually exclusive states.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Circle shared by graphical and drill producers. Graphic state requires only\nlayer. Drill state requires role plus layers; NPTH state additionally\nrequires all mask and pad-size hints. The generated semantic validator\nenforces these mutually exclusive states.",
///  "type": "object",
///  "required": [
///    "cx",
///    "cy",
///    "diameter_nm",
///    "fill",
///    "index",
///    "kind",
///    "width_nm"
///  ],
///  "properties": {
///    "cx": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "cy": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "diameter_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "Circle"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "layers": {
///      "type": [
///        "array",
///        "null"
///      ],
///      "items": {
///        "type": "string"
///      }
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterDrillRole"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CircleOperation {
    pub cx: crate::JavaScriptSafeInteger,
    pub cy: crate::JavaScriptSafeInteger,
    pub diameter_nm: crate::JavaScriptSafeInteger,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_circle_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layers: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub role: ::std::option::Option<PlotterDrillRole>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Dimension text (when present) followed by layered construction geometry.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Dimension text (when present) followed by layered construction geometry.",
///  "type": "object",
///  "required": [
///    "dimension_type",
///    "kind",
///    "layers",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "dimension_type": {
///      "$ref": "#/$defs/BoardDimensionType"
///    },
///    "kind": {
///      "type": "string",
///      "const": "dimension"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "object_id": {
///      "type": "string",
///      "const": "dimension"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "text": {
///      "type": "string"
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DimensionPlotRecord {
    pub dimension_type: BoardDimensionType,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub text: ::std::option::Option<::std::string::String>,
    pub uuid: ::std::string::String,
}
/**Circular pad flash shared by footprint and PCB producers. Footprint pad
state requires mask_margin_nm and forbids role. Board via state requires
role and forbids mask_margin_nm. The generated semantic validator enforces
these mutually exclusive states.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Circular pad flash shared by footprint and PCB producers. Footprint pad\nstate requires mask_margin_nm and forbids role. Board via state requires\nrole and forbids mask_margin_nm. The generated semantic validator enforces\nthese mutually exclusive states.",
///  "type": "object",
///  "required": [
///    "diameter_nm",
///    "index",
///    "kind",
///    "layers",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "diameter_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadCircle"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterViaFlashRole"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadCircleOperation {
    pub diameter_nm: crate::JavaScriptSafeInteger,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_circle_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub role: ::std::option::Option<PlotterViaFlashRole>,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
/**Custom pad flash shared by footprint and PCB producers. Polygon coordinates
are pad-local. A non-empty polygon_widths_nm has one entry per polygon;
generated semantic validation enforces that relationship. An empty array is
equivalent to omission for generated Rust transport bindings.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Custom pad flash shared by footprint and PCB producers. Polygon coordinates\nare pad-local. A non-empty polygon_widths_nm has one entry per polygon;\ngenerated semantic validation enforces that relationship. An empty array is\nequivalent to omission for generated Rust transport bindings.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "polygons",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "anchor_shape": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadCustom"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "polygon_widths_nm": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/JavaScriptSafeInteger"
///      }
///    },
///    "polygons": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadCustomOperation {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub anchor_shape: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_custom_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub polygon_widths_nm: ::std::vec::Vec<crate::JavaScriptSafeInteger>,
    pub polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Oval pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Oval pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadOval"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadOvalOperation {
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_oval_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Rectangular pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Rectangular pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadRect"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadRectOperation {
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_rect_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Rounded-rectangle pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Rounded-rectangle pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "corner_radius_nm",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "corner_radius_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadRoundRect"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadRoundRectOperation {
    pub corner_radius_nm: crate::JavaScriptSafeInteger,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_round_rect_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Trapezoid pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Trapezoid pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "corners",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "corners": {
///      "$ref": "#/$defs/PlotterQuad"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadTrapez"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadTrapezOperation {
    pub corners: PlotterQuad,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_flash_pad_trapez_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Decoded image placement shared by worksheet and schematic producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Decoded image placement shared by worksheet and schematic producers.",
///  "type": "object",
///  "required": [
///    "height_nm",
///    "image_data_b64",
///    "image_format",
///    "index",
///    "kind",
///    "scale",
///    "width_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "height_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "image_data_b64": {
///      "type": "string"
///    },
///    "image_format": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "PlotImage"
///    },
///    "scale": {
///      "type": "number"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotImageOperation {
    pub height_nm: crate::JavaScriptSafeInteger,
    pub image_data_b64: ::std::string::String,
    pub image_format: ::std::string::String,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_plot_image_kind")]
    pub kind: ::std::string::String,
    pub scale: f64,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Filled or outlined polygon operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Filled or outlined polygon operation.",
///  "type": "object",
///  "required": [
///    "fill",
///    "index",
///    "kind",
///    "points",
///    "width_nm"
///  ],
///  "properties": {
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "PlotPoly"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "points": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterPoint"
///      }
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotPolyOperation {
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_plot_poly_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub points: ::std::vec::Vec<PlotterPoint>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Coordinate convention for the footprint plotter slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Coordinate convention for the footprint plotter slice.",
///  "type": "object",
///  "required": [
///    "unit",
///    "y_axis"
///  ],
///  "properties": {
///    "unit": {
///      "type": "string",
///      "const": "nm"
///    },
///    "y_axis": {
///      "type": "string",
///      "const": "down"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotterCoordinateSpace {
    pub unit: ::std::string::String,
    pub y_axis: ::std::string::String,
}
///Semantic roles allowed on shared circle and segment drill operations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Semantic roles allowed on shared circle and segment drill operations.",
///  "type": "string",
///  "enum": [
///    "pad_drill",
///    "npth_hole",
///    "via_drill",
///    "via_mask_drill"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterDrillRole {
    #[serde(rename = "pad_drill")]
    PadDrill,
    #[serde(rename = "npth_hole")]
    NpthHole,
    #[serde(rename = "via_drill")]
    ViaDrill,
    #[serde(rename = "via_mask_drill")]
    ViaMaskDrill,
}
impl ::std::fmt::Display for PlotterDrillRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::PadDrill => f.write_str("pad_drill"),
            Self::NpthHole => f.write_str("npth_hole"),
            Self::ViaDrill => f.write_str("via_drill"),
            Self::ViaMaskDrill => f.write_str("via_mask_drill"),
        }
    }
}
impl ::std::str::FromStr for PlotterDrillRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pad_drill" => Ok(Self::PadDrill),
            "npth_hole" => Ok(Self::NpthHole),
            "via_drill" => Ok(Self::ViaDrill),
            "via_mask_drill" => Ok(Self::ViaMaskDrill),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterDrillRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterDrillRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterDrillRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Fill values shared by plotter operation producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Fill values shared by plotter operation producers.",
///  "type": "string",
///  "enum": [
///    "NO_FILL",
///    "FILLED_SHAPE",
///    "FILLED_WITH_BG_BODYCOLOR",
///    "FILLED_WITH_COLOR",
///    "HATCH",
///    "REVERSE_HATCH",
///    "CROSS_HATCH"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterFill {
    #[serde(rename = "NO_FILL")]
    NoFill,
    #[serde(rename = "FILLED_SHAPE")]
    FilledShape,
    #[serde(rename = "FILLED_WITH_BG_BODYCOLOR")]
    FilledWithBgBodycolor,
    #[serde(rename = "FILLED_WITH_COLOR")]
    FilledWithColor,
    #[serde(rename = "HATCH")]
    Hatch,
    #[serde(rename = "REVERSE_HATCH")]
    ReverseHatch,
    #[serde(rename = "CROSS_HATCH")]
    CrossHatch,
}
impl ::std::fmt::Display for PlotterFill {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NoFill => f.write_str("NO_FILL"),
            Self::FilledShape => f.write_str("FILLED_SHAPE"),
            Self::FilledWithBgBodycolor => f.write_str("FILLED_WITH_BG_BODYCOLOR"),
            Self::FilledWithColor => f.write_str("FILLED_WITH_COLOR"),
            Self::Hatch => f.write_str("HATCH"),
            Self::ReverseHatch => f.write_str("REVERSE_HATCH"),
            Self::CrossHatch => f.write_str("CROSS_HATCH"),
        }
    }
}
impl ::std::str::FromStr for PlotterFill {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "NO_FILL" => Ok(Self::NoFill),
            "FILLED_SHAPE" => Ok(Self::FilledShape),
            "FILLED_WITH_BG_BODYCOLOR" => Ok(Self::FilledWithBgBodycolor),
            "FILLED_WITH_COLOR" => Ok(Self::FilledWithColor),
            "HATCH" => Ok(Self::Hatch),
            "REVERSE_HATCH" => Ok(Self::ReverseHatch),
            "CROSS_HATCH" => Ok(Self::CrossHatch),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterFill {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterFill {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterFill {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///One exact hyperlink attached to an authored plotter text carrier.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One exact hyperlink attached to an authored plotter text carrier.",
///  "type": "object",
///  "required": [
///    "href"
///  ],
///  "properties": {
///    "href": {
///      "type": "string",
///      "minLength": 1
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotterHyperlink {
    pub href: PlotterHyperlinkHref,
}
///`PlotterHyperlinkHref`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "minLength": 1
///}
/// ```
/// </details>
#[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PlotterHyperlinkHref(::std::string::String);
impl ::std::ops::Deref for PlotterHyperlinkHref {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PlotterHyperlinkHref> for ::std::string::String {
    fn from(value: PlotterHyperlinkHref) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PlotterHyperlinkHref {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() < 1usize {
            return Err("shorter than 1 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for PlotterHyperlinkHref {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterHyperlinkHref {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterHyperlinkHref {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PlotterHyperlinkHref {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
///KiCad stroke styles carried without producer-specific decomposition.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "KiCad stroke styles carried without producer-specific decomposition.",
///  "type": "string",
///  "enum": [
///    "DEFAULT",
///    "SOLID",
///    "DASH",
///    "DOT",
///    "DASH_DOT",
///    "DASH_DOT_DOT"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterLineStyle {
    #[serde(rename = "DEFAULT")]
    Default,
    #[serde(rename = "SOLID")]
    Solid,
    #[serde(rename = "DASH")]
    Dash,
    #[serde(rename = "DOT")]
    Dot,
    #[serde(rename = "DASH_DOT")]
    DashDot,
    #[serde(rename = "DASH_DOT_DOT")]
    DashDotDot,
}
impl ::std::fmt::Display for PlotterLineStyle {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Default => f.write_str("DEFAULT"),
            Self::Solid => f.write_str("SOLID"),
            Self::Dash => f.write_str("DASH"),
            Self::Dot => f.write_str("DOT"),
            Self::DashDot => f.write_str("DASH_DOT"),
            Self::DashDotDot => f.write_str("DASH_DOT_DOT"),
        }
    }
}
impl ::std::str::FromStr for PlotterLineStyle {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "DEFAULT" => Ok(Self::Default),
            "SOLID" => Ok(Self::Solid),
            "DASH" => Ok(Self::Dash),
            "DOT" => Ok(Self::Dot),
            "DASH_DOT" => Ok(Self::DashDot),
            "DASH_DOT_DOT" => Ok(Self::DashDotDot),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterLineStyle {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterLineStyle {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterLineStyle {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Shared plotter operation vocabulary promoted across source producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Shared plotter operation vocabulary promoted across source producers.",
///  "anyOf": [
///    {
///      "$ref": "#/$defs/ThickSegmentOperation"
///    },
///    {
///      "$ref": "#/$defs/ArcThreePointOperation"
///    },
///    {
///      "$ref": "#/$defs/CircleOperation"
///    },
///    {
///      "$ref": "#/$defs/RectOperation"
///    },
///    {
///      "$ref": "#/$defs/PlotPolyOperation"
///    },
///    {
///      "$ref": "#/$defs/BezierCurveOperation"
///    },
///    {
///      "$ref": "#/$defs/TextOperation"
///    },
///    {
///      "$ref": "#/$defs/PlotImageOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadCircleOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadOvalOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadRectOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadRoundRectOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadCustomOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadTrapezOperation"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum PlotterOperation {
    ThickSegmentOperation(ThickSegmentOperation),
    ArcThreePointOperation(ArcThreePointOperation),
    CircleOperation(CircleOperation),
    RectOperation(RectOperation),
    PlotPolyOperation(PlotPolyOperation),
    BezierCurveOperation(BezierCurveOperation),
    TextOperation(TextOperation),
    PlotImageOperation(PlotImageOperation),
    FlashPadCircleOperation(FlashPadCircleOperation),
    FlashPadOvalOperation(FlashPadOvalOperation),
    FlashPadRectOperation(FlashPadRectOperation),
    FlashPadRoundRectOperation(FlashPadRoundRectOperation),
    FlashPadCustomOperation(FlashPadCustomOperation),
    FlashPadTrapezOperation(FlashPadTrapezOperation),
}
impl ::std::convert::From<ThickSegmentOperation> for PlotterOperation {
    fn from(value: ThickSegmentOperation) -> Self {
        Self::ThickSegmentOperation(value)
    }
}
impl ::std::convert::From<ArcThreePointOperation> for PlotterOperation {
    fn from(value: ArcThreePointOperation) -> Self {
        Self::ArcThreePointOperation(value)
    }
}
impl ::std::convert::From<CircleOperation> for PlotterOperation {
    fn from(value: CircleOperation) -> Self {
        Self::CircleOperation(value)
    }
}
impl ::std::convert::From<RectOperation> for PlotterOperation {
    fn from(value: RectOperation) -> Self {
        Self::RectOperation(value)
    }
}
impl ::std::convert::From<PlotPolyOperation> for PlotterOperation {
    fn from(value: PlotPolyOperation) -> Self {
        Self::PlotPolyOperation(value)
    }
}
impl ::std::convert::From<BezierCurveOperation> for PlotterOperation {
    fn from(value: BezierCurveOperation) -> Self {
        Self::BezierCurveOperation(value)
    }
}
impl ::std::convert::From<TextOperation> for PlotterOperation {
    fn from(value: TextOperation) -> Self {
        Self::TextOperation(value)
    }
}
impl ::std::convert::From<PlotImageOperation> for PlotterOperation {
    fn from(value: PlotImageOperation) -> Self {
        Self::PlotImageOperation(value)
    }
}
impl ::std::convert::From<FlashPadCircleOperation> for PlotterOperation {
    fn from(value: FlashPadCircleOperation) -> Self {
        Self::FlashPadCircleOperation(value)
    }
}
impl ::std::convert::From<FlashPadOvalOperation> for PlotterOperation {
    fn from(value: FlashPadOvalOperation) -> Self {
        Self::FlashPadOvalOperation(value)
    }
}
impl ::std::convert::From<FlashPadRectOperation> for PlotterOperation {
    fn from(value: FlashPadRectOperation) -> Self {
        Self::FlashPadRectOperation(value)
    }
}
impl ::std::convert::From<FlashPadRoundRectOperation> for PlotterOperation {
    fn from(value: FlashPadRoundRectOperation) -> Self {
        Self::FlashPadRoundRectOperation(value)
    }
}
impl ::std::convert::From<FlashPadCustomOperation> for PlotterOperation {
    fn from(value: FlashPadCustomOperation) -> Self {
        Self::FlashPadCustomOperation(value)
    }
}
impl ::std::convert::From<FlashPadTrapezOperation> for PlotterOperation {
    fn from(value: FlashPadTrapezOperation) -> Self {
        Self::FlashPadTrapezOperation(value)
    }
}
///Strict operation-local context emitted by current plotter producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Strict operation-local context emitted by current plotter producers.",
///  "type": "object",
///  "required": [
///    "hyperlink"
///  ],
///  "properties": {
///    "hyperlink": {
///      "$ref": "#/$defs/PlotterHyperlink"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotterOperationContext {
    pub hyperlink: PlotterHyperlink,
}
///Plotter point encoded as an exact coordinate pair.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Plotter point encoded as an exact coordinate pair.",
///  "type": "array",
///  "items": {
///    "$ref": "#/$defs/JavaScriptSafeInteger"
///  },
///  "maxItems": 2,
///  "minItems": 2
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlotterPoint(pub [crate::JavaScriptSafeInteger; 2usize]);
impl ::std::ops::Deref for PlotterPoint {
    type Target = [crate::JavaScriptSafeInteger; 2usize];
    fn deref(&self) -> &[crate::JavaScriptSafeInteger; 2usize] {
        &self.0
    }
}
impl ::std::convert::From<PlotterPoint> for [crate::JavaScriptSafeInteger; 2usize] {
    fn from(value: PlotterPoint) -> Self {
        value.0
    }
}
impl ::std::convert::From<[crate::JavaScriptSafeInteger; 2usize]> for PlotterPoint {
    fn from(value: [crate::JavaScriptSafeInteger; 2usize]) -> Self {
        Self(value)
    }
}
///Four pad-local trapezoid corners.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Four pad-local trapezoid corners.",
///  "type": "array",
///  "items": {
///    "$ref": "#/$defs/PlotterPoint"
///  },
///  "maxItems": 4,
///  "minItems": 4
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlotterQuad(pub [PlotterPoint; 4usize]);
impl ::std::ops::Deref for PlotterQuad {
    type Target = [PlotterPoint; 4usize];
    fn deref(&self) -> &[PlotterPoint; 4usize] {
        &self.0
    }
}
impl ::std::convert::From<PlotterQuad> for [PlotterPoint; 4usize] {
    fn from(value: PlotterQuad) -> Self {
        value.0
    }
}
impl ::std::convert::From<[PlotterPoint; 4usize]> for PlotterQuad {
    fn from(value: [PlotterPoint; 4usize]) -> Self {
        Self(value)
    }
}
///Stringified boolean metadata mirrored from the established producer.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Stringified boolean metadata mirrored from the established producer.",
///  "type": "string",
///  "enum": [
///    "true",
///    "false"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterStringBool {
    #[serde(rename = "true")]
    True,
    #[serde(rename = "false")]
    False,
}
impl ::std::fmt::Display for PlotterStringBool {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::True => f.write_str("true"),
            Self::False => f.write_str("false"),
        }
    }
}
impl ::std::str::FromStr for PlotterStringBool {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "true" => Ok(Self::True),
            "false" => Ok(Self::False),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterStringBool {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterStringBool {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterStringBool {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Horizontal text alignments emitted by the board producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Horizontal text alignments emitted by the board producers.",
///  "type": "string",
///  "enum": [
///    "GR_TEXT_H_ALIGN_LEFT",
///    "GR_TEXT_H_ALIGN_CENTER",
///    "GR_TEXT_H_ALIGN_RIGHT"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterTextHAlign {
    #[serde(rename = "GR_TEXT_H_ALIGN_LEFT")]
    GrTextHAlignLeft,
    #[serde(rename = "GR_TEXT_H_ALIGN_CENTER")]
    GrTextHAlignCenter,
    #[serde(rename = "GR_TEXT_H_ALIGN_RIGHT")]
    GrTextHAlignRight,
}
impl ::std::fmt::Display for PlotterTextHAlign {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GrTextHAlignLeft => f.write_str("GR_TEXT_H_ALIGN_LEFT"),
            Self::GrTextHAlignCenter => f.write_str("GR_TEXT_H_ALIGN_CENTER"),
            Self::GrTextHAlignRight => f.write_str("GR_TEXT_H_ALIGN_RIGHT"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextHAlign {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "GR_TEXT_H_ALIGN_LEFT" => Ok(Self::GrTextHAlignLeft),
            "GR_TEXT_H_ALIGN_CENTER" => Ok(Self::GrTextHAlignCenter),
            "GR_TEXT_H_ALIGN_RIGHT" => Ok(Self::GrTextHAlignRight),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextHAlign {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextHAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextHAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Coordinate space carried by one typed text render cache.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Coordinate space carried by one typed text render cache.",
///  "type": "string",
///  "enum": [
///    "board",
///    "footprint_local"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterTextRenderCacheCoordinateSpace {
    #[serde(rename = "board")]
    Board,
    #[serde(rename = "footprint_local")]
    FootprintLocal,
}
impl ::std::fmt::Display for PlotterTextRenderCacheCoordinateSpace {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Board => f.write_str("board"),
            Self::FootprintLocal => f.write_str("footprint_local"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextRenderCacheCoordinateSpace {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "board" => Ok(Self::Board),
            "footprint_local" => Ok(Self::FootprintLocal),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextRenderCacheCoordinateSpace {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextRenderCacheCoordinateSpace {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextRenderCacheCoordinateSpace {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Provenance of one attached text render cache.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Provenance of one attached text render cache.",
///  "type": "string",
///  "enum": [
///    "existing_file_cache",
///    "python_generated_cache",
///    "native_generated_cache"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterTextRenderCacheSource {
    #[serde(rename = "existing_file_cache")]
    ExistingFileCache,
    #[serde(rename = "python_generated_cache")]
    PythonGeneratedCache,
    #[serde(rename = "native_generated_cache")]
    NativeGeneratedCache,
}
impl ::std::fmt::Display for PlotterTextRenderCacheSource {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ExistingFileCache => f.write_str("existing_file_cache"),
            Self::PythonGeneratedCache => f.write_str("python_generated_cache"),
            Self::NativeGeneratedCache => f.write_str("native_generated_cache"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextRenderCacheSource {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "existing_file_cache" => Ok(Self::ExistingFileCache),
            "python_generated_cache" => Ok(Self::PythonGeneratedCache),
            "native_generated_cache" => Ok(Self::NativeGeneratedCache),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextRenderCacheSource {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextRenderCacheSource {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextRenderCacheSource {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Vertical text alignments emitted by the board producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Vertical text alignments emitted by the board producers.",
///  "type": "string",
///  "enum": [
///    "GR_TEXT_V_ALIGN_TOP",
///    "GR_TEXT_V_ALIGN_CENTER",
///    "GR_TEXT_V_ALIGN_BOTTOM"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterTextVAlign {
    #[serde(rename = "GR_TEXT_V_ALIGN_TOP")]
    GrTextVAlignTop,
    #[serde(rename = "GR_TEXT_V_ALIGN_CENTER")]
    GrTextVAlignCenter,
    #[serde(rename = "GR_TEXT_V_ALIGN_BOTTOM")]
    GrTextVAlignBottom,
}
impl ::std::fmt::Display for PlotterTextVAlign {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GrTextVAlignTop => f.write_str("GR_TEXT_V_ALIGN_TOP"),
            Self::GrTextVAlignCenter => f.write_str("GR_TEXT_V_ALIGN_CENTER"),
            Self::GrTextVAlignBottom => f.write_str("GR_TEXT_V_ALIGN_BOTTOM"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextVAlign {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "GR_TEXT_V_ALIGN_TOP" => Ok(Self::GrTextVAlignTop),
            "GR_TEXT_V_ALIGN_CENTER" => Ok(Self::GrTextVAlignCenter),
            "GR_TEXT_V_ALIGN_BOTTOM" => Ok(Self::GrTextVAlignBottom),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextVAlign {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextVAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextVAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Semantic roles allowed on board via flash operations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Semantic roles allowed on board via flash operations.",
///  "type": "string",
///  "enum": [
///    "via_aperture",
///    "via_mask_opening"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum PlotterViaFlashRole {
    #[serde(rename = "via_aperture")]
    ViaAperture,
    #[serde(rename = "via_mask_opening")]
    ViaMaskOpening,
}
impl ::std::fmt::Display for PlotterViaFlashRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ViaAperture => f.write_str("via_aperture"),
            Self::ViaMaskOpening => f.write_str("via_mask_opening"),
        }
    }
}
impl ::std::str::FromStr for PlotterViaFlashRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "via_aperture" => Ok(Self::ViaAperture),
            "via_mask_opening" => Ok(Self::ViaMaskOpening),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterViaFlashRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterViaFlashRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterViaFlashRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Rectangle with square corners.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Rectangle with square corners.",
///  "type": "object",
///  "required": [
///    "corner_radius_nm",
///    "fill",
///    "index",
///    "kind",
///    "width_nm",
///    "x1",
///    "x2",
///    "y1",
///    "y2"
///  ],
///  "properties": {
///    "corner_radius_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "Rect"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x1": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x2": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y1": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y2": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RectOperation {
    pub corner_radius_nm: crate::JavaScriptSafeInteger,
    pub fill: PlotterFill,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_rect_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
    pub x1: crate::JavaScriptSafeInteger,
    pub x2: crate::JavaScriptSafeInteger,
    pub y1: crate::JavaScriptSafeInteger,
    pub y2: crate::JavaScriptSafeInteger,
}
///Board table grid/border segments followed by optional faced cell text.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Board table grid/border segments followed by optional faced cell text.",
///  "type": "object",
///  "required": [
///    "cell_count",
///    "kind",
///    "layers",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "cell_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "table"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "object_id": {
///      "type": "string",
///      "const": "table"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TablePlotRecord {
    pub cell_count: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
/**Stroke or cached text operation. Boolean marker keys (`mirror`,
`text_as_polygons`, `polyline_per_segment`, `knockout`) are present-only
-when-true, matching the established Python emitter. Render-cache keys
appear together when an authored cache resolves; `render_cache_polygons`
carries the exterior rings in nanometres.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Stroke or cached text operation. Boolean marker keys (`mirror`,\n`text_as_polygons`, `polyline_per_segment`, `knockout`) are present-only\n-when-true, matching the established Python emitter. Render-cache keys\nappear together when an authored cache resolves; `render_cache_polygons`\ncarries the exterior rings in nanometres.",
///  "type": "object",
///  "required": [
///    "bold",
///    "color",
///    "font_face",
///    "h_align",
///    "index",
///    "italic",
///    "kind",
///    "multiline",
///    "orient_deg",
///    "pen_width_nm",
///    "size_x_nm",
///    "size_y_nm",
///    "text",
///    "v_align",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "bold": {
///      "type": "boolean"
///    },
///    "color": {
///      "type": "string"
///    },
///    "context": {
///      "$ref": "#/$defs/PlotterOperationContext"
///    },
///    "font_face": {
///      "type": "string"
///    },
///    "h_align": {
///      "$ref": "#/$defs/PlotterTextHAlign"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "italic": {
///      "type": "boolean"
///    },
///    "kind": {
///      "type": "string",
///      "const": "Text"
///    },
///    "knockout": {
///      "type": "boolean"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "mirror": {
///      "type": "boolean"
///    },
///    "multiline": {
///      "type": "boolean"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "pen_width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "polyline_per_segment": {
///      "type": "boolean"
///    },
///    "render_cache": {
///      "$ref": "#/$defs/TextRenderCache"
///    },
///    "render_cache_exact": {
///      "type": "boolean"
///    },
///    "render_cache_polygons": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    },
///    "render_cache_source": {
///      "$ref": "#/$defs/PlotterTextRenderCacheSource"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "text": {
///      "type": "string"
///    },
///    "text_as_polygons": {
///      "type": "boolean"
///    },
///    "v_align": {
///      "$ref": "#/$defs/PlotterTextVAlign"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TextOperation {
    pub bold: bool,
    pub color: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub context: ::std::option::Option<PlotterOperationContext>,
    pub font_face: ::std::string::String,
    pub h_align: PlotterTextHAlign,
    pub index: u32,
    pub italic: bool,
    #[serde(deserialize_with = "crate::deserialize_text_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub knockout: ::std::option::Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mirror: ::std::option::Option<bool>,
    pub multiline: bool,
    pub orient_deg: f64,
    pub pen_width_nm: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub polyline_per_segment: ::std::option::Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub render_cache: ::std::option::Option<TextRenderCache>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub render_cache_exact: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub render_cache_source: ::std::option::Option<PlotterTextRenderCacheSource>,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub text: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub text_as_polygons: ::std::option::Option<bool>,
    pub v_align: PlotterTextVAlign,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
/**Typed render cache from an authored `(render_cache ...)` form, the Python
resolver, or the deterministic native hinted outline engine. `knockout` appears when the
knockout background restructure replaced the polygons.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Typed render cache from an authored `(render_cache ...)` form, the Python\nresolver, or the deterministic native hinted outline engine. `knockout` appears when the\nknockout background restructure replaced the polygons.",
///  "type": "object",
///  "required": [
///    "angle",
///    "coordinate_space",
///    "exact",
///    "polygons",
///    "schema",
///    "source",
///    "text",
///    "unit"
///  ],
///  "properties": {
///    "angle": {
///      "type": "number"
///    },
///    "coordinate_space": {
///      "$ref": "#/$defs/PlotterTextRenderCacheCoordinateSpace"
///    },
///    "exact": {
///      "type": "boolean"
///    },
///    "knockout": {
///      "type": "boolean"
///    },
///    "polygons": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/TextRenderCachePolygon"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad.render_cache.v1"
///    },
///    "source": {
///      "$ref": "#/$defs/PlotterTextRenderCacheSource"
///    },
///    "text": {
///      "type": "string"
///    },
///    "unit": {
///      "type": "string",
///      "const": "nm"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TextRenderCache {
    pub angle: f64,
    pub coordinate_space: PlotterTextRenderCacheCoordinateSpace,
    pub exact: bool,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub knockout: ::std::option::Option<bool>,
    pub polygons: ::std::vec::Vec<TextRenderCachePolygon>,
    pub schema: ::std::string::String,
    pub source: PlotterTextRenderCacheSource,
    pub text: ::std::string::String,
    pub unit: ::std::string::String,
}
///One render-cache polygon as ordered contours, exterior ring first.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One render-cache polygon as ordered contours, exterior ring first.",
///  "type": "object",
///  "required": [
///    "contours"
///  ],
///  "properties": {
///    "contours": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TextRenderCachePolygon {
    pub contours: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
}
/**Solid or decomposed segment shared by PCB, footprint, and drill producers.
Graphic state requires only layer. Drill state requires role plus layers;
NPTH drill state additionally requires all mask and pad-size hints. The
generated semantic validator enforces these mutually exclusive states.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Solid or decomposed segment shared by PCB, footprint, and drill producers.\nGraphic state requires only layer. Drill state requires role plus layers;\nNPTH drill state additionally requires all mask and pad-size hints. The\ngenerated semantic validator enforces these mutually exclusive states.",
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "ThickSegment"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterDrillRole"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ThickSegmentOperation {
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_thick_segment_kind")]
    pub kind: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub role: ::std::option::Option<PlotterDrillRole>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///One board track arc record with its net attribution.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board track arc record with its net attribution.",
///  "type": "object",
///  "required": [
///    "kind",
///    "layer",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "track_arc"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "net_class": {
///      "type": "string"
///    },
///    "net_classes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_id": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "net_name": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TrackArcPlotRecord {
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_class: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub net_classes: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_id: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_name: ::std::option::Option<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
///One board track segment record with its net attribution.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board track segment record with its net attribution.",
///  "type": "object",
///  "required": [
///    "kind",
///    "layer",
///    "locked",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "segment"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "locked": {
///      "type": "boolean"
///    },
///    "net_class": {
///      "type": "string"
///    },
///    "net_classes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_id": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "net_name": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TrackSegmentPlotRecord {
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub locked: bool,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_class: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub net_classes: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_id: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_name: ::std::option::Option<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
/**One board via record: optional resolved copper aperture, mandatory physical
drill, and per-side mask opening/drill pairs when tenting explicitly exposes
that side. A fully removed annulus is represented by the drill alone.
IPC-4761 fabrication metadata mirrors the established stringified booleans.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One board via record: optional resolved copper aperture, mandatory physical\ndrill, and per-side mask opening/drill pairs when tenting explicitly exposes\nthat side. A fully removed annulus is represented by the drill alone.\nIPC-4761 fabrication metadata mirrors the established stringified booleans.",
///  "type": "object",
///  "required": [
///    "drill",
///    "hole_kind",
///    "hole_plating",
///    "hole_render",
///    "kind",
///    "layers",
///    "object_id",
///    "operation_count",
///    "operations",
///    "size",
///    "uuid",
///    "via_type"
///  ],
///  "properties": {
///    "drill": {
///      "type": "number"
///    },
///    "hole_kind": {
///      "type": "string",
///      "const": "round"
///    },
///    "hole_plating": {
///      "type": "string",
///      "const": "plated"
///    },
///    "hole_render": {
///      "type": "string",
///      "const": "drill"
///    },
///    "ipc4761_capping": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_covering_back": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_covering_front": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_filling": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_metadata": {
///      "type": "string",
///      "const": "true"
///    },
///    "ipc4761_plugging_back": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_plugging_front": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_tenting_back": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "ipc4761_tenting_front": {
///      "$ref": "#/$defs/PlotterStringBool"
///    },
///    "kind": {
///      "type": "string",
///      "const": "via"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_class": {
///      "type": "string"
///    },
///    "net_classes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_id": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "net_name": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "size": {
///      "type": "number"
///    },
///    "uuid": {
///      "type": "string"
///    },
///    "via_type": {
///      "$ref": "#/$defs/BoardViaType"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ViaPlotRecord {
    pub drill: f64,
    pub hole_kind: ::std::string::String,
    pub hole_plating: ::std::string::String,
    pub hole_render: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_capping: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_covering_back: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_covering_front: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_filling: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_metadata: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_plugging_back: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_plugging_front: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_tenting_back: ::std::option::Option<PlotterStringBool>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub ipc4761_tenting_front: ::std::option::Option<PlotterStringBool>,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_class: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub net_classes: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_id: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_name: ::std::option::Option<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub size: f64,
    pub uuid: ::std::string::String,
    pub via_type: BoardViaType,
}
/**One zone fill record bundling every `filled_polygon` ring. The parallel
`fill_layers`/`fill_island` arrays annotate the rings so consumers can
split or colour-key without re-walking the source zone.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One zone fill record bundling every `filled_polygon` ring. The parallel\n`fill_layers`/`fill_island` arrays annotate the rings so consumers can\nsplit or colour-key without re-walking the source zone.",
///  "type": "object",
///  "required": [
///    "fill_island",
///    "fill_layers",
///    "kind",
///    "layers",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "fill_island": {
///      "type": "array",
///      "items": {
///        "type": "boolean"
///      }
///    },
///    "fill_layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "kind": {
///      "type": "string",
///      "const": "zone_fill"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_class": {
///      "type": "string"
///    },
///    "net_classes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_id": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "net_name": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterOperation"
///      }
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ZoneFillPlotRecord {
    pub fill_island: ::std::vec::Vec<bool>,
    pub fill_layers: ::std::vec::Vec<::std::string::String>,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_class: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub net_classes: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_id: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub net_name: ::std::option::Option<::std::string::String>,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}

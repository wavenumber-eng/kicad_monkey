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
///`ComponentOverride`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "assembly_designators": {
///      "$ref": "#/$defs/ComponentOverrideAssemblyDesignators"
///    },
///    "assembly_hlr": {
///      "$ref": "#/$defs/ComponentOverrideAssemblyHlr"
///    },
///    "cathode_pad": {
///      "type": "string"
///    },
///    "diode": {
///      "type": "boolean"
///    },
///    "diode_line_art": {
///      "type": "boolean"
///    },
///    "pin1_enabled": {
///      "type": "boolean"
///    },
///    "pin1_pad": {
///      "type": "string"
///    },
///    "projection": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "detail"
///        },
///        {
///          "type": "string",
///          "const": "outline"
///        },
///        {
///          "type": "string",
///          "const": "bounding_box"
///        },
///        {
///          "type": "string",
///          "const": "model_bounds"
///        },
///        {
///          "type": "string",
///          "const": "pad_bounds"
///        },
///        {
///          "type": "string",
///          "const": "none"
///        }
///      ]
///    },
///    "show_designator": {
///      "type": "boolean"
///    },
///    "side": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "top"
///        },
///        {
///          "type": "string",
///          "const": "bottom"
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
pub struct ComponentOverride {
    #[serde(
        rename = "assembly_designators",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub assembly_designators: crate::pcb_svg::presence::Field<ComponentOverrideAssemblyDesignators>,
    #[serde(
        rename = "assembly_hlr",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub assembly_hlr: crate::pcb_svg::presence::Field<ComponentOverrideAssemblyHlr>,
    #[serde(
        rename = "cathode_pad",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub cathode_pad: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "diode",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub diode: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "diode_line_art",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub diode_line_art: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "pin1_enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pin1_enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "pin1_pad",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pin1_pad: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "projection",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub projection: crate::pcb_svg::presence::Field<ComponentOverrideProjection>,
    #[serde(
        rename = "show_designator",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub show_designator: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "side",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub side: crate::pcb_svg::presence::Field<ComponentOverrideSide>,
}
impl ::std::default::Default for ComponentOverride {
    fn default() -> Self {
        Self {
            assembly_designators: Default::default(),
            assembly_hlr: Default::default(),
            cathode_pad: Default::default(),
            diode: Default::default(),
            diode_line_art: Default::default(),
            pin1_enabled: Default::default(),
            pin1_pad: Default::default(),
            projection: Default::default(),
            show_designator: Default::default(),
            side: Default::default(),
        }
    }
}
///`ComponentOverrideAssemblyDesignators`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ComponentOverrideAssemblyDesignators(
    pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
);
impl ::std::ops::Deref for ComponentOverrideAssemblyDesignators {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<ComponentOverrideAssemblyDesignators>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: ComponentOverrideAssemblyDesignators) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for ComponentOverrideAssemblyDesignators
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
///`ComponentOverrideAssemblyHlr`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ComponentOverrideAssemblyHlr(
    pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
);
impl ::std::ops::Deref for ComponentOverrideAssemblyHlr {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<ComponentOverrideAssemblyHlr>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: ComponentOverrideAssemblyHlr) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for ComponentOverrideAssemblyHlr
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
///`ComponentOverrideProjection`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "detail"
///    },
///    {
///      "type": "string",
///      "const": "outline"
///    },
///    {
///      "type": "string",
///      "const": "bounding_box"
///    },
///    {
///      "type": "string",
///      "const": "model_bounds"
///    },
///    {
///      "type": "string",
///      "const": "pad_bounds"
///    },
///    {
///      "type": "string",
///      "const": "none"
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
pub enum ComponentOverrideProjection {
    #[serde(rename = "detail")]
    Detail,
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "bounding_box")]
    BoundingBox,
    #[serde(rename = "model_bounds")]
    ModelBounds,
    #[serde(rename = "pad_bounds")]
    PadBounds,
    #[serde(rename = "none")]
    None,
}
impl ::std::fmt::Display for ComponentOverrideProjection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Detail => f.write_str("detail"),
            Self::Outline => f.write_str("outline"),
            Self::BoundingBox => f.write_str("bounding_box"),
            Self::ModelBounds => f.write_str("model_bounds"),
            Self::PadBounds => f.write_str("pad_bounds"),
            Self::None => f.write_str("none"),
        }
    }
}
impl ::std::str::FromStr for ComponentOverrideProjection {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "detail" => Ok(Self::Detail),
            "outline" => Ok(Self::Outline),
            "bounding_box" => Ok(Self::BoundingBox),
            "model_bounds" => Ok(Self::ModelBounds),
            "pad_bounds" => Ok(Self::PadBounds),
            "none" => Ok(Self::None),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ComponentOverrideProjection {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ComponentOverrideProjection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ComponentOverrideProjection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`ComponentOverrideSide`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "top"
///    },
///    {
///      "type": "string",
///      "const": "bottom"
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
pub enum ComponentOverrideSide {
    #[serde(rename = "top")]
    Top,
    #[serde(rename = "bottom")]
    Bottom,
}
impl ::std::fmt::Display for ComponentOverrideSide {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Top => f.write_str("top"),
            Self::Bottom => f.write_str("bottom"),
        }
    }
}
impl ::std::str::FromStr for ComponentOverrideSide {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ComponentOverrideSide {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ComponentOverrideSide {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ComponentOverrideSide {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`CounterClockwiseHyphenated`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "const": "counter-clockwise"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct CounterClockwiseHyphenated(pub ::std::string::String);
impl ::std::ops::Deref for CounterClockwiseHyphenated {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CounterClockwiseHyphenated> for ::std::string::String {
    fn from(value: CounterClockwiseHyphenated) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for CounterClockwiseHyphenated {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for CounterClockwiseHyphenated {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for CounterClockwiseHyphenated {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///`HoleStyle`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "enabled": {
///      "type": "boolean"
///    },
///    "non_plated_color": {
///      "type": "string"
///    },
///    "opacity": {
///      "type": "number",
///      "maximum": 1.0,
///      "minimum": 0.0
///    },
///    "plated_color": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct HoleStyle {
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "non_plated_color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub non_plated_color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "opacity",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub opacity: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "plated_color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub plated_color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
///`Negative90`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "const": "-90"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct Negative90(pub ::std::string::String);
impl ::std::ops::Deref for Negative90 {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Negative90> for ::std::string::String {
    fn from(value: Negative90) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Negative90 {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for Negative90 {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for Negative90 {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///`Ninety`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "const": "90"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct Ninety(pub ::std::string::String);
impl ::std::ops::Deref for Ninety {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Ninety> for ::std::string::String {
    fn from(value: Ninety) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Ninety {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for Ninety {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for Ninety {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///`PcbSvgConfig`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "pcb_svg_config.a0.schema.json",
///  "title": "PcbSvgConfig",
///  "type": "object",
///  "required": [
///    "global",
///    "layer_outputs",
///    "schema",
///    "views"
///  ],
///  "properties": {
///    "assembly": {
///      "$ref": "#/$defs/PcbSvgConfigAssembly"
///    },
///    "components": {
///      "$ref": "#/$defs/PcbSvgConfigComponents"
///    },
///    "diodes": {
///      "$ref": "#/$defs/PcbSvgConfigDiodes"
///    },
///    "dnp": {
///      "$ref": "#/$defs/PcbSvgConfigDnp"
///    },
///    "global": {
///      "$ref": "#/$defs/PcbSvgConfigGlobal"
///    },
///    "layer_outputs": {
///      "$ref": "#/$defs/PcbSvgConfigLayerOutputs"
///    },
///    "pin1": {
///      "$ref": "#/$defs/PcbSvgConfigPin1"
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad_cruncher.pcb_svg.config.a0"
///    },
///    "views": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PcbSvgConfigViewsItem"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfig {
    #[serde(
        rename = "assembly",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub assembly: crate::pcb_svg::presence::Field<PcbSvgConfigAssembly>,
    #[serde(
        rename = "components",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub components: crate::pcb_svg::presence::Field<PcbSvgConfigComponents>,
    #[serde(
        rename = "diodes",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub diodes: crate::pcb_svg::presence::Field<PcbSvgConfigDiodes>,
    #[serde(
        rename = "dnp",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub dnp: crate::pcb_svg::presence::Field<PcbSvgConfigDnp>,
    pub global: PcbSvgConfigGlobal,
    pub layer_outputs: PcbSvgConfigLayerOutputs,
    #[serde(
        rename = "pin1",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pin1: crate::pcb_svg::presence::Field<PcbSvgConfigPin1>,
    pub schema: ::std::string::String,
    pub views: ::std::vec::Vec<PcbSvgConfigViewsItem>,
}
///`PcbSvgConfigAssembly`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "default_projection": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "detail"
///        },
///        {
///          "type": "string",
///          "const": "outline"
///        },
///        {
///          "type": "string",
///          "const": "bounding_box"
///        },
///        {
///          "type": "string",
///          "const": "model_bounds"
///        },
///        {
///          "type": "string",
///          "const": "pad_bounds"
///        },
///        {
///          "type": "string",
///          "const": "none"
///        }
///      ]
///    },
///    "designator_color": {
///      "type": "string"
///    },
///    "dnp_designator_color": {
///      "type": "string"
///    },
///    "dnp_projection": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "detail"
///        },
///        {
///          "type": "string",
///          "const": "outline"
///        },
///        {
///          "type": "string",
///          "const": "bounding_box"
///        },
///        {
///          "type": "string",
///          "const": "model_bounds"
///        },
///        {
///          "type": "string",
///          "const": "pad_bounds"
///        },
///        {
///          "type": "string",
///          "const": "none"
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
pub struct PcbSvgConfigAssembly {
    #[serde(
        rename = "default_projection",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub default_projection: crate::pcb_svg::presence::Field<PcbSvgConfigAssemblyDefaultProjection>,
    #[serde(
        rename = "designator_color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub designator_color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "dnp_designator_color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub dnp_designator_color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "dnp_projection",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub dnp_projection: crate::pcb_svg::presence::Field<PcbSvgConfigAssemblyDnpProjection>,
}
impl ::std::default::Default for PcbSvgConfigAssembly {
    fn default() -> Self {
        Self {
            default_projection: Default::default(),
            designator_color: Default::default(),
            dnp_designator_color: Default::default(),
            dnp_projection: Default::default(),
        }
    }
}
///`PcbSvgConfigAssemblyDefaultProjection`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "detail"
///    },
///    {
///      "type": "string",
///      "const": "outline"
///    },
///    {
///      "type": "string",
///      "const": "bounding_box"
///    },
///    {
///      "type": "string",
///      "const": "model_bounds"
///    },
///    {
///      "type": "string",
///      "const": "pad_bounds"
///    },
///    {
///      "type": "string",
///      "const": "none"
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
pub enum PcbSvgConfigAssemblyDefaultProjection {
    #[serde(rename = "detail")]
    Detail,
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "bounding_box")]
    BoundingBox,
    #[serde(rename = "model_bounds")]
    ModelBounds,
    #[serde(rename = "pad_bounds")]
    PadBounds,
    #[serde(rename = "none")]
    None,
}
impl ::std::fmt::Display for PcbSvgConfigAssemblyDefaultProjection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Detail => f.write_str("detail"),
            Self::Outline => f.write_str("outline"),
            Self::BoundingBox => f.write_str("bounding_box"),
            Self::ModelBounds => f.write_str("model_bounds"),
            Self::PadBounds => f.write_str("pad_bounds"),
            Self::None => f.write_str("none"),
        }
    }
}
impl ::std::str::FromStr for PcbSvgConfigAssemblyDefaultProjection {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "detail" => Ok(Self::Detail),
            "outline" => Ok(Self::Outline),
            "bounding_box" => Ok(Self::BoundingBox),
            "model_bounds" => Ok(Self::ModelBounds),
            "pad_bounds" => Ok(Self::PadBounds),
            "none" => Ok(Self::None),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PcbSvgConfigAssemblyDefaultProjection {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PcbSvgConfigAssemblyDefaultProjection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PcbSvgConfigAssemblyDefaultProjection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`PcbSvgConfigAssemblyDnpProjection`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "detail"
///    },
///    {
///      "type": "string",
///      "const": "outline"
///    },
///    {
///      "type": "string",
///      "const": "bounding_box"
///    },
///    {
///      "type": "string",
///      "const": "model_bounds"
///    },
///    {
///      "type": "string",
///      "const": "pad_bounds"
///    },
///    {
///      "type": "string",
///      "const": "none"
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
pub enum PcbSvgConfigAssemblyDnpProjection {
    #[serde(rename = "detail")]
    Detail,
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "bounding_box")]
    BoundingBox,
    #[serde(rename = "model_bounds")]
    ModelBounds,
    #[serde(rename = "pad_bounds")]
    PadBounds,
    #[serde(rename = "none")]
    None,
}
impl ::std::fmt::Display for PcbSvgConfigAssemblyDnpProjection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Detail => f.write_str("detail"),
            Self::Outline => f.write_str("outline"),
            Self::BoundingBox => f.write_str("bounding_box"),
            Self::ModelBounds => f.write_str("model_bounds"),
            Self::PadBounds => f.write_str("pad_bounds"),
            Self::None => f.write_str("none"),
        }
    }
}
impl ::std::str::FromStr for PcbSvgConfigAssemblyDnpProjection {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "detail" => Ok(Self::Detail),
            "outline" => Ok(Self::Outline),
            "bounding_box" => Ok(Self::BoundingBox),
            "model_bounds" => Ok(Self::ModelBounds),
            "pad_bounds" => Ok(Self::PadBounds),
            "none" => Ok(Self::None),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PcbSvgConfigAssemblyDnpProjection {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PcbSvgConfigAssemblyDnpProjection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PcbSvgConfigAssemblyDnpProjection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`PcbSvgConfigComponents`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {
///    "$ref": "#/$defs/ComponentOverride"
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PcbSvgConfigComponents(
    pub ::std::collections::HashMap<::std::string::String, ComponentOverride>,
);
impl ::std::ops::Deref for PcbSvgConfigComponents {
    type Target = ::std::collections::HashMap<::std::string::String, ComponentOverride>;
    fn deref(&self) -> &::std::collections::HashMap<::std::string::String, ComponentOverride> {
        &self.0
    }
}
impl ::std::convert::From<PcbSvgConfigComponents>
    for ::std::collections::HashMap<::std::string::String, ComponentOverride>
{
    fn from(value: PcbSvgConfigComponents) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::collections::HashMap<::std::string::String, ComponentOverride>>
    for PcbSvgConfigComponents
{
    fn from(value: ::std::collections::HashMap<::std::string::String, ComponentOverride>) -> Self {
        Self(value)
    }
}
///`PcbSvgConfigDiodes`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "cathode_pad_names": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "designator_prefixes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "line_art": {
///      "type": "boolean"
///    },
///    "marker_color": {
///      "type": "string"
///    },
///    "numeric_cathode_pad": {
///      "type": "string"
///    },
///    "parameter_terms": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfigDiodes {
    #[serde(
        rename = "cathode_pad_names",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub cathode_pad_names: crate::pcb_svg::presence::Field<::std::vec::Vec<::std::string::String>>,
    #[serde(
        rename = "designator_prefixes",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub designator_prefixes:
        crate::pcb_svg::presence::Field<::std::vec::Vec<::std::string::String>>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "line_art",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub line_art: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "marker_color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub marker_color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "numeric_cathode_pad",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub numeric_cathode_pad: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "parameter_terms",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub parameter_terms: crate::pcb_svg::presence::Field<::std::vec::Vec<::std::string::String>>,
}
impl ::std::default::Default for PcbSvgConfigDiodes {
    fn default() -> Self {
        Self {
            cathode_pad_names: Default::default(),
            designator_prefixes: Default::default(),
            enabled: Default::default(),
            line_art: Default::default(),
            marker_color: Default::default(),
            numeric_cathode_pad: Default::default(),
            parameter_terms: Default::default(),
        }
    }
}
///`PcbSvgConfigDnp`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "color": {
///      "type": "string"
///    },
///    "hatch": {
///      "type": "boolean"
///    },
///    "hatch_angle_deg": {
///      "type": "number"
///    },
///    "hatch_line_width_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "hatch_spacing_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfigDnp {
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "hatch",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "hatch_angle_deg",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch_angle_deg: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "hatch_line_width_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch_line_width_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "hatch_spacing_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch_spacing_mm: crate::pcb_svg::presence::Field<f64>,
}
impl ::std::default::Default for PcbSvgConfigDnp {
    fn default() -> Self {
        Self {
            color: Default::default(),
            hatch: Default::default(),
            hatch_angle_deg: Default::default(),
            hatch_line_width_mm: Default::default(),
            hatch_spacing_mm: Default::default(),
        }
    }
}
///`PcbSvgConfigGlobal`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "canvas": {
///      "$ref": "#/$defs/PcbSvgConfigGlobalCanvas"
///    },
///    "clean_output": {
///      "type": "boolean"
///    },
///    "clip_holes_from_copper": {
///      "type": "boolean"
///    },
///    "clip_to_outline": {
///      "type": "boolean"
///    },
///    "include_metadata": {
///      "type": "boolean"
///    },
///    "mirror_bottom_view": {
///      "type": "boolean"
///    },
///    "pcbdoc": {
///      "anyOf": [
///        {
///          "type": "string"
///        },
///        {
///          "type": "null"
///        }
///      ]
///    },
///    "show_empty_layers": {
///      "type": "boolean"
///    },
///    "styles": {
///      "$ref": "#/$defs/StyleTable"
///    },
///    "svg_scale": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "svg_size_unit": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PcbSvgConfigGlobal {
    #[serde(
        rename = "canvas",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub canvas: crate::pcb_svg::presence::Field<PcbSvgConfigGlobalCanvas>,
    #[serde(
        rename = "clean_output",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub clean_output: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "clip_holes_from_copper",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub clip_holes_from_copper: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "clip_to_outline",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub clip_to_outline: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "include_metadata",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub include_metadata: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "mirror_bottom_view",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub mirror_bottom_view: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "pcbdoc",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pcbdoc: crate::pcb_svg::presence::Field<::std::option::Option<::std::string::String>>,
    #[serde(
        rename = "show_empty_layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub show_empty_layers: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "styles",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub styles: crate::pcb_svg::presence::Field<StyleTable>,
    #[serde(
        rename = "svg_scale",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub svg_scale: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "svg_size_unit",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub svg_size_unit: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
///`PcbSvgConfigGlobalCanvas`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "bounds": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "board_outline"
///        },
///        {
///          "type": "string",
///          "const": "all_geometry"
///        }
///      ]
///    },
///    "margin_mm": {
///      "type": "number",
///      "minimum": 0.0
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfigGlobalCanvas {
    #[serde(
        rename = "bounds",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub bounds: crate::pcb_svg::presence::Field<PcbSvgConfigGlobalCanvasBounds>,
    #[serde(
        rename = "margin_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub margin_mm: crate::pcb_svg::presence::Field<f64>,
}
impl ::std::default::Default for PcbSvgConfigGlobalCanvas {
    fn default() -> Self {
        Self {
            bounds: Default::default(),
            margin_mm: Default::default(),
        }
    }
}
///`PcbSvgConfigGlobalCanvasBounds`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "board_outline"
///    },
///    {
///      "type": "string",
///      "const": "all_geometry"
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
pub enum PcbSvgConfigGlobalCanvasBounds {
    #[serde(rename = "board_outline")]
    BoardOutline,
    #[serde(rename = "all_geometry")]
    AllGeometry,
}
impl ::std::fmt::Display for PcbSvgConfigGlobalCanvasBounds {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::BoardOutline => f.write_str("board_outline"),
            Self::AllGeometry => f.write_str("all_geometry"),
        }
    }
}
impl ::std::str::FromStr for PcbSvgConfigGlobalCanvasBounds {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "board_outline" => Ok(Self::BoardOutline),
            "all_geometry" => Ok(Self::AllGeometry),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PcbSvgConfigGlobalCanvasBounds {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PcbSvgConfigGlobalCanvasBounds {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PcbSvgConfigGlobalCanvasBounds {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`PcbSvgConfigLayerOutputs`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "add_drills_to_physical_layers": {
///      "description": "Include computed round drill overlays as context in each non-Edge.Cuts physical layer output.",
///      "type": "boolean"
///    },
///    "add_edge_cuts_to_physical_layers": {
///      "description": "Include raw Edge.Cuts geometry as context in each non-Edge.Cuts physical layer output.",
///      "type": "boolean"
///    },
///    "add_slots_to_physical_layers": {
///      "description": "Include computed slot overlays as context in each non-Edge.Cuts physical layer output.",
///      "type": "boolean"
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "include_special_layers": {
///      "type": "array",
///      "items": {
///        "anyOf": [
///          {
///            "type": "string",
///            "const": "BOARD_OUTLINE"
///          },
///          {
///            "type": "string",
///            "const": "BOARD_CUTOUTS"
///          },
///          {
///            "type": "string",
///            "const": "DRILLS"
///          },
///          {
///            "type": "string",
///            "const": "SLOTS"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_HLR_TOP"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_HLR_BOTTOM"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_DESIGNATORS_TOP"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_DESIGNATORS_BOTTOM"
///          },
///          {
///            "type": "string",
///            "const": "PIN1_TOP"
///          },
///          {
///            "type": "string",
///            "const": "PIN1_BOTTOM"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_HLR_TOP_OUTLINE"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_HLR_BOTTOM_OUTLINE"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_HLR_TOP_DETAIL"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_HLR_BOTTOM_DETAIL"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_BOUNDS_TOP_MODEL"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_BOUNDS_BOTTOM_MODEL"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_BOUNDS_TOP_PADS"
///          },
///          {
///            "type": "string",
///            "const": "ASSEMBLY_BOUNDS_BOTTOM_PADS"
///          }
///        ]
///      }
///    },
///    "layers": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "auto"
///        },
///        {
///          "type": "array",
///          "items": {
///            "type": "string"
///          }
///        }
///      ]
///    },
///    "output_dir": {
///      "type": "string"
///    },
///    "write_virtual_layers": {
///      "description": "Write standalone __virtual__ layer SVG files selected by include_special_layers and synthetic layer tokens.",
///      "type": "boolean"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfigLayerOutputs {
    ///Include computed round drill overlays as context in each non-Edge.Cuts physical layer output.
    #[serde(
        rename = "add_drills_to_physical_layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub add_drills_to_physical_layers: crate::pcb_svg::presence::Field<bool>,
    ///Include raw Edge.Cuts geometry as context in each non-Edge.Cuts physical layer output.
    #[serde(
        rename = "add_edge_cuts_to_physical_layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub add_edge_cuts_to_physical_layers: crate::pcb_svg::presence::Field<bool>,
    ///Include computed slot overlays as context in each non-Edge.Cuts physical layer output.
    #[serde(
        rename = "add_slots_to_physical_layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub add_slots_to_physical_layers: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "include_special_layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub include_special_layers: crate::pcb_svg::presence::Field<
        ::std::vec::Vec<PcbSvgConfigLayerOutputsIncludeSpecialLayersItem>,
    >,
    #[serde(
        rename = "layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub layers: crate::pcb_svg::presence::Field<PcbSvgConfigLayerOutputsLayers>,
    #[serde(
        rename = "output_dir",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub output_dir: crate::pcb_svg::presence::Field<::std::string::String>,
    ///Write standalone __virtual__ layer SVG files selected by include_special_layers and synthetic layer tokens.
    #[serde(
        rename = "write_virtual_layers",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub write_virtual_layers: crate::pcb_svg::presence::Field<bool>,
}
impl ::std::default::Default for PcbSvgConfigLayerOutputs {
    fn default() -> Self {
        Self {
            add_drills_to_physical_layers: Default::default(),
            add_edge_cuts_to_physical_layers: Default::default(),
            add_slots_to_physical_layers: Default::default(),
            enabled: Default::default(),
            include_special_layers: Default::default(),
            layers: Default::default(),
            output_dir: Default::default(),
            write_virtual_layers: Default::default(),
        }
    }
}
///`PcbSvgConfigLayerOutputsIncludeSpecialLayersItem`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "BOARD_OUTLINE"
///    },
///    {
///      "type": "string",
///      "const": "BOARD_CUTOUTS"
///    },
///    {
///      "type": "string",
///      "const": "DRILLS"
///    },
///    {
///      "type": "string",
///      "const": "SLOTS"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_HLR_TOP"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_HLR_BOTTOM"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_DESIGNATORS_TOP"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_DESIGNATORS_BOTTOM"
///    },
///    {
///      "type": "string",
///      "const": "PIN1_TOP"
///    },
///    {
///      "type": "string",
///      "const": "PIN1_BOTTOM"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_HLR_TOP_OUTLINE"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_HLR_BOTTOM_OUTLINE"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_HLR_TOP_DETAIL"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_HLR_BOTTOM_DETAIL"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_BOUNDS_TOP_MODEL"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_BOUNDS_BOTTOM_MODEL"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_BOUNDS_TOP_PADS"
///    },
///    {
///      "type": "string",
///      "const": "ASSEMBLY_BOUNDS_BOTTOM_PADS"
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
pub enum PcbSvgConfigLayerOutputsIncludeSpecialLayersItem {
    #[serde(rename = "BOARD_OUTLINE")]
    BoardOutline,
    #[serde(rename = "BOARD_CUTOUTS")]
    BoardCutouts,
    #[serde(rename = "DRILLS")]
    Drills,
    #[serde(rename = "SLOTS")]
    Slots,
    #[serde(rename = "ASSEMBLY_HLR_TOP")]
    AssemblyHlrTop,
    #[serde(rename = "ASSEMBLY_HLR_BOTTOM")]
    AssemblyHlrBottom,
    #[serde(rename = "ASSEMBLY_DESIGNATORS_TOP")]
    AssemblyDesignatorsTop,
    #[serde(rename = "ASSEMBLY_DESIGNATORS_BOTTOM")]
    AssemblyDesignatorsBottom,
    #[serde(rename = "PIN1_TOP")]
    Pin1Top,
    #[serde(rename = "PIN1_BOTTOM")]
    Pin1Bottom,
    #[serde(rename = "ASSEMBLY_HLR_TOP_OUTLINE")]
    AssemblyHlrTopOutline,
    #[serde(rename = "ASSEMBLY_HLR_BOTTOM_OUTLINE")]
    AssemblyHlrBottomOutline,
    #[serde(rename = "ASSEMBLY_HLR_TOP_DETAIL")]
    AssemblyHlrTopDetail,
    #[serde(rename = "ASSEMBLY_HLR_BOTTOM_DETAIL")]
    AssemblyHlrBottomDetail,
    #[serde(rename = "ASSEMBLY_BOUNDS_TOP_MODEL")]
    AssemblyBoundsTopModel,
    #[serde(rename = "ASSEMBLY_BOUNDS_BOTTOM_MODEL")]
    AssemblyBoundsBottomModel,
    #[serde(rename = "ASSEMBLY_BOUNDS_TOP_PADS")]
    AssemblyBoundsTopPads,
    #[serde(rename = "ASSEMBLY_BOUNDS_BOTTOM_PADS")]
    AssemblyBoundsBottomPads,
}
impl ::std::fmt::Display for PcbSvgConfigLayerOutputsIncludeSpecialLayersItem {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::BoardOutline => f.write_str("BOARD_OUTLINE"),
            Self::BoardCutouts => f.write_str("BOARD_CUTOUTS"),
            Self::Drills => f.write_str("DRILLS"),
            Self::Slots => f.write_str("SLOTS"),
            Self::AssemblyHlrTop => f.write_str("ASSEMBLY_HLR_TOP"),
            Self::AssemblyHlrBottom => f.write_str("ASSEMBLY_HLR_BOTTOM"),
            Self::AssemblyDesignatorsTop => f.write_str("ASSEMBLY_DESIGNATORS_TOP"),
            Self::AssemblyDesignatorsBottom => f.write_str("ASSEMBLY_DESIGNATORS_BOTTOM"),
            Self::Pin1Top => f.write_str("PIN1_TOP"),
            Self::Pin1Bottom => f.write_str("PIN1_BOTTOM"),
            Self::AssemblyHlrTopOutline => f.write_str("ASSEMBLY_HLR_TOP_OUTLINE"),
            Self::AssemblyHlrBottomOutline => f.write_str("ASSEMBLY_HLR_BOTTOM_OUTLINE"),
            Self::AssemblyHlrTopDetail => f.write_str("ASSEMBLY_HLR_TOP_DETAIL"),
            Self::AssemblyHlrBottomDetail => f.write_str("ASSEMBLY_HLR_BOTTOM_DETAIL"),
            Self::AssemblyBoundsTopModel => f.write_str("ASSEMBLY_BOUNDS_TOP_MODEL"),
            Self::AssemblyBoundsBottomModel => f.write_str("ASSEMBLY_BOUNDS_BOTTOM_MODEL"),
            Self::AssemblyBoundsTopPads => f.write_str("ASSEMBLY_BOUNDS_TOP_PADS"),
            Self::AssemblyBoundsBottomPads => f.write_str("ASSEMBLY_BOUNDS_BOTTOM_PADS"),
        }
    }
}
impl ::std::str::FromStr for PcbSvgConfigLayerOutputsIncludeSpecialLayersItem {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "BOARD_OUTLINE" => Ok(Self::BoardOutline),
            "BOARD_CUTOUTS" => Ok(Self::BoardCutouts),
            "DRILLS" => Ok(Self::Drills),
            "SLOTS" => Ok(Self::Slots),
            "ASSEMBLY_HLR_TOP" => Ok(Self::AssemblyHlrTop),
            "ASSEMBLY_HLR_BOTTOM" => Ok(Self::AssemblyHlrBottom),
            "ASSEMBLY_DESIGNATORS_TOP" => Ok(Self::AssemblyDesignatorsTop),
            "ASSEMBLY_DESIGNATORS_BOTTOM" => Ok(Self::AssemblyDesignatorsBottom),
            "PIN1_TOP" => Ok(Self::Pin1Top),
            "PIN1_BOTTOM" => Ok(Self::Pin1Bottom),
            "ASSEMBLY_HLR_TOP_OUTLINE" => Ok(Self::AssemblyHlrTopOutline),
            "ASSEMBLY_HLR_BOTTOM_OUTLINE" => Ok(Self::AssemblyHlrBottomOutline),
            "ASSEMBLY_HLR_TOP_DETAIL" => Ok(Self::AssemblyHlrTopDetail),
            "ASSEMBLY_HLR_BOTTOM_DETAIL" => Ok(Self::AssemblyHlrBottomDetail),
            "ASSEMBLY_BOUNDS_TOP_MODEL" => Ok(Self::AssemblyBoundsTopModel),
            "ASSEMBLY_BOUNDS_BOTTOM_MODEL" => Ok(Self::AssemblyBoundsBottomModel),
            "ASSEMBLY_BOUNDS_TOP_PADS" => Ok(Self::AssemblyBoundsTopPads),
            "ASSEMBLY_BOUNDS_BOTTOM_PADS" => Ok(Self::AssemblyBoundsBottomPads),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PcbSvgConfigLayerOutputsIncludeSpecialLayersItem {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String>
    for PcbSvgConfigLayerOutputsIncludeSpecialLayersItem
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for PcbSvgConfigLayerOutputsIncludeSpecialLayersItem
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`PcbSvgConfigLayerOutputsLayers`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "auto"
///    },
///    {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum PcbSvgConfigLayerOutputsLayers {
    String(::std::string::String),
    Array(::std::vec::Vec<::std::string::String>),
}
impl ::std::convert::From<::std::vec::Vec<::std::string::String>>
    for PcbSvgConfigLayerOutputsLayers
{
    fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
        Self::Array(value)
    }
}
///`PcbSvgConfigPin1`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "exclude_designator_prefixes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "exclude_designators": {
///      "anyOf": [
///        {
///          "type": "string"
///        },
///        {
///          "type": "array",
///          "items": {
///            "type": "string"
///          }
///        }
///      ]
///    },
///    "exclude_single_pin": {
///      "type": "boolean"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfigPin1 {
    #[serde(
        rename = "exclude_designator_prefixes",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub exclude_designator_prefixes:
        crate::pcb_svg::presence::Field<::std::vec::Vec<::std::string::String>>,
    #[serde(
        rename = "exclude_designators",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub exclude_designators: crate::pcb_svg::presence::Field<PcbSvgConfigPin1ExcludeDesignators>,
    #[serde(
        rename = "exclude_single_pin",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub exclude_single_pin: crate::pcb_svg::presence::Field<bool>,
}
impl ::std::default::Default for PcbSvgConfigPin1 {
    fn default() -> Self {
        Self {
            exclude_designator_prefixes: Default::default(),
            exclude_designators: Default::default(),
            exclude_single_pin: Default::default(),
        }
    }
}
///`PcbSvgConfigPin1ExcludeDesignators`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string"
///    },
///    {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum PcbSvgConfigPin1ExcludeDesignators {
    String(::std::string::String),
    Array(::std::vec::Vec<::std::string::String>),
}
impl ::std::convert::From<::std::vec::Vec<::std::string::String>>
    for PcbSvgConfigPin1ExcludeDesignators
{
    fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
        Self::Array(value)
    }
}
///`PcbSvgConfigViewsItem`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "layers",
///    "name"
///  ],
///  "properties": {
///    "assembly_hlr_mode": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "outline"
///        },
///        {
///          "type": "string",
///          "const": "detail"
///        },
///        {
///          "type": "string",
///          "const": "bounding_box"
///        },
///        {
///          "type": "string",
///          "const": "model_bounds"
///        },
///        {
///          "type": "string",
///          "const": "pad_bounds"
///        },
///        {
///          "type": "string",
///          "const": "none"
///        }
///      ]
///    },
///    "description": {
///      "type": "string"
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "group_id": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mirror": {
///      "type": "boolean"
///    },
///    "name": {
///      "type": "string",
///      "minLength": 1
///    },
///    "output_svg": {
///      "type": "string"
///    },
///    "pin1": {
///      "$ref": "#/$defs/Pin1"
///    },
///    "styles": {
///      "$ref": "#/$defs/StyleTable"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PcbSvgConfigViewsItem {
    #[serde(
        rename = "assembly_hlr_mode",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub assembly_hlr_mode: crate::pcb_svg::presence::Field<PcbSvgConfigViewsItemAssemblyHlrMode>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub description: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "group_id",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub group_id: crate::pcb_svg::presence::Field<::std::string::String>,
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(
        rename = "mirror",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub mirror: crate::pcb_svg::presence::Field<bool>,
    pub name: PcbSvgConfigViewsItemName,
    #[serde(
        rename = "output_svg",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub output_svg: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "pin1",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pin1: crate::pcb_svg::presence::Field<Pin1>,
    #[serde(
        rename = "styles",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub styles: crate::pcb_svg::presence::Field<StyleTable>,
}
///`PcbSvgConfigViewsItemAssemblyHlrMode`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "outline"
///    },
///    {
///      "type": "string",
///      "const": "detail"
///    },
///    {
///      "type": "string",
///      "const": "bounding_box"
///    },
///    {
///      "type": "string",
///      "const": "model_bounds"
///    },
///    {
///      "type": "string",
///      "const": "pad_bounds"
///    },
///    {
///      "type": "string",
///      "const": "none"
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
pub enum PcbSvgConfigViewsItemAssemblyHlrMode {
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "detail")]
    Detail,
    #[serde(rename = "bounding_box")]
    BoundingBox,
    #[serde(rename = "model_bounds")]
    ModelBounds,
    #[serde(rename = "pad_bounds")]
    PadBounds,
    #[serde(rename = "none")]
    None,
}
impl ::std::fmt::Display for PcbSvgConfigViewsItemAssemblyHlrMode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Outline => f.write_str("outline"),
            Self::Detail => f.write_str("detail"),
            Self::BoundingBox => f.write_str("bounding_box"),
            Self::ModelBounds => f.write_str("model_bounds"),
            Self::PadBounds => f.write_str("pad_bounds"),
            Self::None => f.write_str("none"),
        }
    }
}
impl ::std::str::FromStr for PcbSvgConfigViewsItemAssemblyHlrMode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "outline" => Ok(Self::Outline),
            "detail" => Ok(Self::Detail),
            "bounding_box" => Ok(Self::BoundingBox),
            "model_bounds" => Ok(Self::ModelBounds),
            "pad_bounds" => Ok(Self::PadBounds),
            "none" => Ok(Self::None),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PcbSvgConfigViewsItemAssemblyHlrMode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PcbSvgConfigViewsItemAssemblyHlrMode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PcbSvgConfigViewsItemAssemblyHlrMode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`PcbSvgConfigViewsItemName`
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
pub struct PcbSvgConfigViewsItemName(::std::string::String);
impl ::std::ops::Deref for PcbSvgConfigViewsItemName {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PcbSvgConfigViewsItemName> for ::std::string::String {
    fn from(value: PcbSvgConfigViewsItemName) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PcbSvgConfigViewsItemName {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() < 1usize {
            return Err("shorter than 1 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for PcbSvgConfigViewsItemName {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PcbSvgConfigViewsItemName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PcbSvgConfigViewsItemName {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PcbSvgConfigViewsItemName {
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
///`Pin1`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "exclude_designator_prefixes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "exclude_designators": {
///      "anyOf": [
///        {
///          "type": "string"
///        },
///        {
///          "type": "array",
///          "items": {
///            "type": "string"
///          }
///        }
///      ]
///    },
///    "exclude_single_pin": {
///      "type": "boolean"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Pin1 {
    #[serde(
        rename = "exclude_designator_prefixes",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub exclude_designator_prefixes:
        crate::pcb_svg::presence::Field<::std::vec::Vec<::std::string::String>>,
    #[serde(
        rename = "exclude_designators",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub exclude_designators: crate::pcb_svg::presence::Field<Pin1ExcludeDesignators>,
    #[serde(
        rename = "exclude_single_pin",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub exclude_single_pin: crate::pcb_svg::presence::Field<bool>,
}
impl ::std::default::Default for Pin1 {
    fn default() -> Self {
        Self {
            exclude_designator_prefixes: Default::default(),
            exclude_designators: Default::default(),
            exclude_single_pin: Default::default(),
        }
    }
}
///`Pin1ExcludeDesignators`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string"
///    },
///    {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Pin1ExcludeDesignators {
    String(::std::string::String),
    Array(::std::vec::Vec<::std::string::String>),
}
impl ::std::convert::From<::std::vec::Vec<::std::string::String>> for Pin1ExcludeDesignators {
    fn from(value: ::std::vec::Vec<::std::string::String>) -> Self {
        Self::Array(value)
    }
}
///`Positive90`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "const": "+90"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct Positive90(pub ::std::string::String);
impl ::std::ops::Deref for Positive90 {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Positive90> for ::std::string::String {
    fn from(value: Positive90) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Positive90 {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for Positive90 {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for Positive90 {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///`RotationDirection`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "cw"
///    },
///    {
///      "type": "string",
///      "const": "clockwise"
///    },
///    {
///      "type": "string",
///      "const": "right"
///    },
///    {
///      "$ref": "#/$defs/Positive90"
///    },
///    {
///      "$ref": "#/$defs/Ninety"
///    },
///    {
///      "type": "string",
///      "const": "ccw"
///    },
///    {
///      "type": "string",
///      "const": "counterclockwise"
///    },
///    {
///      "$ref": "#/$defs/CounterClockwiseHyphenated"
///    },
///    {
///      "type": "string",
///      "const": "left"
///    },
///    {
///      "$ref": "#/$defs/Negative90"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum RotationDirection {
    Variant0(::std::string::String),
    Variant1(::std::string::String),
    Variant2(::std::string::String),
    Variant3(Positive90),
    Variant4(Ninety),
    Variant5(::std::string::String),
    Variant6(::std::string::String),
    Variant7(CounterClockwiseHyphenated),
    Variant8(::std::string::String),
    Variant9(Negative90),
}
impl ::std::fmt::Display for RotationDirection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Variant0(x) => x.fmt(f),
            Self::Variant1(x) => x.fmt(f),
            Self::Variant2(x) => x.fmt(f),
            Self::Variant3(x) => x.fmt(f),
            Self::Variant4(x) => x.fmt(f),
            Self::Variant5(x) => x.fmt(f),
            Self::Variant6(x) => x.fmt(f),
            Self::Variant7(x) => x.fmt(f),
            Self::Variant8(x) => x.fmt(f),
            Self::Variant9(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<Positive90> for RotationDirection {
    fn from(value: Positive90) -> Self {
        Self::Variant3(value)
    }
}
impl ::std::convert::From<Ninety> for RotationDirection {
    fn from(value: Ninety) -> Self {
        Self::Variant4(value)
    }
}
impl ::std::convert::From<CounterClockwiseHyphenated> for RotationDirection {
    fn from(value: CounterClockwiseHyphenated) -> Self {
        Self::Variant7(value)
    }
}
impl ::std::convert::From<Negative90> for RotationDirection {
    fn from(value: Negative90) -> Self {
        Self::Variant9(value)
    }
}
///`StyleTable`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "assembly_designators": {
///      "$ref": "#/$defs/StyleTableAssemblyDesignators"
///    },
///    "assembly_hlr": {
///      "$ref": "#/$defs/StyleTableAssemblyHlr"
///    },
///    "board_cutouts": {
///      "$ref": "#/$defs/StyleTableBoardCutouts"
///    },
///    "board_outline": {
///      "$ref": "#/$defs/StyleTableBoardOutline"
///    },
///    "drills": {
///      "$ref": "#/$defs/HoleStyle"
///    },
///    "pin1_marker": {
///      "$ref": "#/$defs/StyleTablePin1Marker"
///    },
///    "slots": {
///      "$ref": "#/$defs/HoleStyle"
///    }
///  },
///  "additionalProperties": {
///    "$ref": "#/$defs/StyleTableExtension"
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StyleTable {
    #[serde(
        rename = "assembly_designators",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub assembly_designators: crate::pcb_svg::presence::Field<StyleTableAssemblyDesignators>,
    #[serde(
        rename = "assembly_hlr",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub assembly_hlr: crate::pcb_svg::presence::Field<StyleTableAssemblyHlr>,
    #[serde(
        rename = "board_cutouts",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub board_cutouts: crate::pcb_svg::presence::Field<StyleTableBoardCutouts>,
    #[serde(
        rename = "board_outline",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub board_outline: crate::pcb_svg::presence::Field<StyleTableBoardOutline>,
    #[serde(
        rename = "drills",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub drills: crate::pcb_svg::presence::Field<HoleStyle>,
    #[serde(
        rename = "pin1_marker",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pin1_marker: crate::pcb_svg::presence::Field<StyleTablePin1Marker>,
    #[serde(
        rename = "slots",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub slots: crate::pcb_svg::presence::Field<HoleStyle>,
    #[serde(flatten)]
    pub extra: ::std::collections::HashMap<::std::string::String, StyleTableExtension>,
}
///`StyleTableAssemblyDesignators`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "box_fill_ratio": {
///      "type": "number",
///      "maximum": 1.0,
///      "exclusiveMinimum": 0.0
///    },
///    "color": {
///      "type": "string"
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "font_family": {
///      "type": "string"
///    },
///    "font_weight": {
///      "anyOf": [
///        {
///          "type": "string"
///        },
///        {
///          "type": "number"
///        }
///      ]
///    },
///    "max_font_size_mm": {
///      "type": "number",
///      "minimum": 0.0
///    },
///    "min_font_size_mm": {
///      "type": "number",
///      "minimum": 0.0
///    },
///    "opacity": {
///      "type": "number",
///      "maximum": 1.0,
///      "minimum": 0.0
///    },
///    "rotation_aspect_threshold": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "rotation_direction": {
///      "$ref": "#/$defs/RotationDirection"
///    },
///    "selector_overrides": {
///      "$ref": "#/$defs/StyleTableAssemblyDesignatorsSelectorOverrides"
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StyleTableAssemblyDesignators {
    #[serde(
        rename = "box_fill_ratio",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub box_fill_ratio: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "font_family",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub font_family: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "font_weight",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub font_weight: crate::pcb_svg::presence::Field<StyleTableAssemblyDesignatorsFontWeight>,
    #[serde(
        rename = "max_font_size_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_font_size_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "min_font_size_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub min_font_size_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "opacity",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub opacity: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "rotation_aspect_threshold",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub rotation_aspect_threshold: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "rotation_direction",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub rotation_direction: crate::pcb_svg::presence::Field<RotationDirection>,
    #[serde(
        rename = "selector_overrides",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub selector_overrides:
        crate::pcb_svg::presence::Field<StyleTableAssemblyDesignatorsSelectorOverrides>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
///`StyleTableAssemblyDesignatorsFontWeight`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string"
///    },
///    {
///      "type": "number"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum StyleTableAssemblyDesignatorsFontWeight {
    String(::std::string::String),
    Number(f64),
}
impl ::std::fmt::Display for StyleTableAssemblyDesignatorsFontWeight {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::String(x) => x.fmt(f),
            Self::Number(x) => x.fmt(f),
        }
    }
}
impl ::std::convert::From<f64> for StyleTableAssemblyDesignatorsFontWeight {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
///`StyleTableAssemblyDesignatorsSelectorOverrides`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {
///    "$ref": "#/$defs/StyleTableAssemblyDesignatorsSelectorOverridesExtension"
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct StyleTableAssemblyDesignatorsSelectorOverrides(
    pub  ::std::collections::HashMap<
        ::std::string::String,
        StyleTableAssemblyDesignatorsSelectorOverridesExtension,
    >,
);
impl ::std::ops::Deref for StyleTableAssemblyDesignatorsSelectorOverrides {
    type Target = ::std::collections::HashMap<
        ::std::string::String,
        StyleTableAssemblyDesignatorsSelectorOverridesExtension,
    >;
    fn deref(
        &self,
    ) -> &::std::collections::HashMap<
        ::std::string::String,
        StyleTableAssemblyDesignatorsSelectorOverridesExtension,
    > {
        &self.0
    }
}
impl ::std::convert::From<StyleTableAssemblyDesignatorsSelectorOverrides>
    for ::std::collections::HashMap<
        ::std::string::String,
        StyleTableAssemblyDesignatorsSelectorOverridesExtension,
    >
{
    fn from(value: StyleTableAssemblyDesignatorsSelectorOverrides) -> Self {
        value.0
    }
}
impl
    ::std::convert::From<
        ::std::collections::HashMap<
            ::std::string::String,
            StyleTableAssemblyDesignatorsSelectorOverridesExtension,
        >,
    > for StyleTableAssemblyDesignatorsSelectorOverrides
{
    fn from(
        value: ::std::collections::HashMap<
            ::std::string::String,
            StyleTableAssemblyDesignatorsSelectorOverridesExtension,
        >,
    ) -> Self {
        Self(value)
    }
}
///`StyleTableAssemblyDesignatorsSelectorOverridesExtension`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct StyleTableAssemblyDesignatorsSelectorOverridesExtension(
    pub ::serde_json::Map<::std::string::String, ::serde_json::Value>,
);
impl ::std::ops::Deref for StyleTableAssemblyDesignatorsSelectorOverridesExtension {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<StyleTableAssemblyDesignatorsSelectorOverridesExtension>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: StyleTableAssemblyDesignatorsSelectorOverridesExtension) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for StyleTableAssemblyDesignatorsSelectorOverridesExtension
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
///`StyleTableAssemblyHlr`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "color": {
///      "type": "string"
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "line_width_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "opacity": {
///      "type": "number",
///      "maximum": 1.0,
///      "minimum": 0.0
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StyleTableAssemblyHlr {
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "line_width_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub line_width_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "opacity",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub opacity: crate::pcb_svg::presence::Field<f64>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
///`StyleTableBoardCutouts`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "color": {
///      "type": "string"
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "hatch": {
///      "type": "boolean"
///    },
///    "hatch_angle_deg": {
///      "type": "number"
///    },
///    "hatch_line_width_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "hatch_spacing_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "outline_dash_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "outline_style": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "solid"
///        },
///        {
///          "type": "string",
///          "const": "dashed"
///        }
///      ]
///    },
///    "outline_width_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StyleTableBoardCutouts {
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "hatch",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "hatch_angle_deg",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch_angle_deg: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "hatch_line_width_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch_line_width_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "hatch_spacing_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub hatch_spacing_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "outline_dash_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub outline_dash_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "outline_style",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub outline_style: crate::pcb_svg::presence::Field<StyleTableBoardCutoutsOutlineStyle>,
    #[serde(
        rename = "outline_width_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub outline_width_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
///`StyleTableBoardCutoutsOutlineStyle`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "solid"
///    },
///    {
///      "type": "string",
///      "const": "dashed"
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
pub enum StyleTableBoardCutoutsOutlineStyle {
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "dashed")]
    Dashed,
}
impl ::std::fmt::Display for StyleTableBoardCutoutsOutlineStyle {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Solid => f.write_str("solid"),
            Self::Dashed => f.write_str("dashed"),
        }
    }
}
impl ::std::str::FromStr for StyleTableBoardCutoutsOutlineStyle {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "solid" => Ok(Self::Solid),
            "dashed" => Ok(Self::Dashed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for StyleTableBoardCutoutsOutlineStyle {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for StyleTableBoardCutoutsOutlineStyle {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for StyleTableBoardCutoutsOutlineStyle {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`StyleTableBoardOutline`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "color": {
///      "type": "string"
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "line_width_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "max_arc_segment_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "max_arc_segments": {
///      "type": "integer",
///      "minimum": 8.0
///    },
///    "max_circle_segment_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "max_circle_segments": {
///      "type": "integer",
///      "minimum": 8.0
///    },
///    "max_curve_segment_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "max_curve_segments": {
///      "type": "integer",
///      "minimum": 8.0
///    },
///    "min_arc_segments": {
///      "type": "integer",
///      "minimum": 1.0
///    },
///    "min_circle_segments": {
///      "type": "integer",
///      "minimum": 8.0
///    },
///    "min_curve_segments": {
///      "type": "integer",
///      "minimum": 1.0
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StyleTableBoardOutline {
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "line_width_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub line_width_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "max_arc_segment_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_arc_segment_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "max_arc_segments",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_arc_segments: crate::pcb_svg::presence::Field<i64>,
    #[serde(
        rename = "max_circle_segment_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_circle_segment_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "max_circle_segments",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_circle_segments: crate::pcb_svg::presence::Field<i64>,
    #[serde(
        rename = "max_curve_segment_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_curve_segment_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "max_curve_segments",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_curve_segments: crate::pcb_svg::presence::Field<i64>,
    #[serde(
        rename = "min_arc_segments",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub min_arc_segments: crate::pcb_svg::presence::Field<::std::num::NonZeroU64>,
    #[serde(
        rename = "min_circle_segments",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub min_circle_segments: crate::pcb_svg::presence::Field<i64>,
    #[serde(
        rename = "min_curve_segments",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub min_curve_segments: crate::pcb_svg::presence::Field<::std::num::NonZeroU64>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}
///`StyleTableExtension`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct StyleTableExtension(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
impl ::std::ops::Deref for StyleTableExtension {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<StyleTableExtension>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: StyleTableExtension) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for StyleTableExtension
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
///`StyleTablePin1Marker`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "color": {
///      "type": "string"
///    },
///    "dot_diameter_mm": {
///      "type": "number",
///      "exclusiveMinimum": 0.0
///    },
///    "enabled": {
///      "type": "boolean"
///    },
///    "max_dot_diameter_mm": {
///      "type": "number",
///      "minimum": 0.0
///    },
///    "min_dot_diameter_mm": {
///      "type": "number",
///      "minimum": 0.0
///    },
///    "pad_diameter_ratio": {
///      "type": "number",
///      "minimum": 0.0
///    }
///  },
///  "additionalProperties": {}
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StyleTablePin1Marker {
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub color: crate::pcb_svg::presence::Field<::std::string::String>,
    #[serde(
        rename = "dot_diameter_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub dot_diameter_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub enabled: crate::pcb_svg::presence::Field<bool>,
    #[serde(
        rename = "max_dot_diameter_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub max_dot_diameter_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "min_dot_diameter_mm",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub min_dot_diameter_mm: crate::pcb_svg::presence::Field<f64>,
    #[serde(
        rename = "pad_diameter_ratio",
        default,
        skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing"
    )]
    pub pad_diameter_ratio: crate::pcb_svg::presence::Field<f64>,
    #[serde(flatten)]
    pub extra: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
}

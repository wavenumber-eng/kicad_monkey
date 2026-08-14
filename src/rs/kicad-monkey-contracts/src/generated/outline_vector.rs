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
///Absolute tolerance in the enclosing record's declared coordinate unit.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Absolute tolerance in the enclosing record's declared coordinate unit.",
///  "type": "object",
///  "required": [
///    "absolute_tolerance",
///    "mode"
///  ],
///  "properties": {
///    "absolute_tolerance": {
///      "$ref": "#/$defs/NonNegativeFiniteFloat"
///    },
///    "mode": {
///      "type": "string",
///      "const": "absolute_tolerance"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct AbsoluteToleranceComparisonPolicy {
    pub absolute_tolerance: crate::NonNegativeFiniteFloat,
    pub mode: ::std::string::String,
}
///`CoordinateComparisonPolicy`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "$ref": "#/$defs/ExactComparisonPolicy"
///    },
///    {
///      "$ref": "#/$defs/AbsoluteToleranceComparisonPolicy"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum CoordinateComparisonPolicy {
    ExactComparisonPolicy(ExactComparisonPolicy),
    AbsoluteToleranceComparisonPolicy(AbsoluteToleranceComparisonPolicy),
}
impl ::std::convert::From<ExactComparisonPolicy> for CoordinateComparisonPolicy {
    fn from(value: ExactComparisonPolicy) -> Self {
        Self::ExactComparisonPolicy(value)
    }
}
impl ::std::convert::From<AbsoluteToleranceComparisonPolicy> for CoordinateComparisonPolicy {
    fn from(value: AbsoluteToleranceComparisonPolicy) -> Self {
        Self::AbsoluteToleranceComparisonPolicy(value)
    }
}
///`ExactComparisonPolicy`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "mode"
///  ],
///  "properties": {
///    "mode": {
///      "type": "string",
///      "const": "exact"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ExactComparisonPolicy {
    pub mode: ::std::string::String,
}
///One ordered OpenType variation coordinate.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One ordered OpenType variation coordinate.",
///  "type": "object",
///  "required": [
///    "axis",
///    "value"
///  ],
///  "properties": {
///    "axis": {
///      "$ref": "#/$defs/OpenTypeTag"
///    },
///    "value": {
///      "$ref": "#/$defs/FiniteFloat"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FontVariationCoordinate {
    pub axis: OpenTypeTag,
    pub value: crate::FiniteFloat,
}
///Four-byte OpenType variation or feature tag.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Four-byte OpenType variation or feature tag.",
///  "type": "string"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct OpenTypeTag(pub ::std::string::String);
impl ::std::ops::Deref for OpenTypeTag {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<OpenTypeTag> for ::std::string::String {
    fn from(value: OpenTypeTag) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for OpenTypeTag {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for OpenTypeTag {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for OpenTypeTag {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///`OutlineClose`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "close"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OutlineClose {
    pub kind: ::std::string::String,
}
///`OutlineCommand`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "$ref": "#/$defs/OutlineMoveTo"
///    },
///    {
///      "$ref": "#/$defs/OutlineLineTo"
///    },
///    {
///      "$ref": "#/$defs/OutlineQuadTo"
///    },
///    {
///      "$ref": "#/$defs/OutlineCurveTo"
///    },
///    {
///      "$ref": "#/$defs/OutlineClose"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum OutlineCommand {
    MoveTo(OutlineMoveTo),
    LineTo(OutlineLineTo),
    QuadTo(OutlineQuadTo),
    CurveTo(OutlineCurveTo),
    Close(OutlineClose),
}
impl ::std::convert::From<OutlineMoveTo> for OutlineCommand {
    fn from(value: OutlineMoveTo) -> Self {
        Self::MoveTo(value)
    }
}
impl ::std::convert::From<OutlineLineTo> for OutlineCommand {
    fn from(value: OutlineLineTo) -> Self {
        Self::LineTo(value)
    }
}
impl ::std::convert::From<OutlineQuadTo> for OutlineCommand {
    fn from(value: OutlineQuadTo) -> Self {
        Self::QuadTo(value)
    }
}
impl ::std::convert::From<OutlineCurveTo> for OutlineCommand {
    fn from(value: OutlineCurveTo) -> Self {
        Self::CurveTo(value)
    }
}
impl ::std::convert::From<OutlineClose> for OutlineCommand {
    fn from(value: OutlineClose) -> Self {
        Self::Close(value)
    }
}
///`OutlineCurveTo`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "control1_x",
///    "control1_y",
///    "control2_x",
///    "control2_y",
///    "kind",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "control1_x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "control1_y": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "control2_x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "control2_y": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "kind": {
///      "type": "string",
///      "const": "curve_to"
///    },
///    "x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "y": {
///      "$ref": "#/$defs/FiniteFloat"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OutlineCurveTo {
    pub control1_x: crate::FiniteFloat,
    pub control1_y: crate::FiniteFloat,
    pub control2_x: crate::FiniteFloat,
    pub control2_y: crate::FiniteFloat,
    pub kind: ::std::string::String,
    pub x: crate::FiniteFloat,
    pub y: crate::FiniteFloat,
}
///`OutlineLineTo`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "line_to"
///    },
///    "x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "y": {
///      "$ref": "#/$defs/FiniteFloat"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OutlineLineTo {
    pub kind: ::std::string::String,
    pub x: crate::FiniteFloat,
    pub y: crate::FiniteFloat,
}
///`OutlineMoveTo`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "move_to"
///    },
///    "x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "y": {
///      "$ref": "#/$defs/FiniteFloat"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OutlineMoveTo {
    pub kind: ::std::string::String,
    pub x: crate::FiniteFloat,
    pub y: crate::FiniteFloat,
}
///`OutlineQuadTo`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "control_x",
///    "control_y",
///    "kind",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "control_x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "control_y": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "kind": {
///      "type": "string",
///      "const": "quad_to"
///    },
///    "x": {
///      "$ref": "#/$defs/FiniteFloat"
///    },
///    "y": {
///      "$ref": "#/$defs/FiniteFloat"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OutlineQuadTo {
    pub control_x: crate::FiniteFloat,
    pub control_y: crate::FiniteFloat,
    pub kind: ::std::string::String,
    pub x: crate::FiniteFloat,
    pub y: crate::FiniteFloat,
}
///Raw glyph outline oracle in font units, separate from shaping and placement.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.outline_vector:a0",
///  "title": "Outline vector a0",
///  "description": "Raw glyph outline oracle in font units, separate from shaping and placement.",
///  "type": "object",
///  "required": [
///    "case_id",
///    "commands",
///    "coordinate_comparison",
///    "coordinate_format",
///    "face_index",
///    "font_id",
///    "font_sha256",
///    "glyph_id",
///    "schema",
///    "type",
///    "units_per_em",
///    "variations",
///    "version"
///  ],
///  "properties": {
///    "case_id": {
///      "$ref": "#/$defs/StableTextId"
///    },
///    "commands": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/OutlineCommand"
///      }
///    },
///    "coordinate_comparison": {
///      "$ref": "#/$defs/CoordinateComparisonPolicy"
///    },
///    "coordinate_format": {
///      "type": "string",
///      "const": "font_design_units_f64"
///    },
///    "face_index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "font_id": {
///      "$ref": "#/$defs/StableTextId"
///    },
///    "font_sha256": {
///      "$ref": "#/$defs/Sha256Hex"
///    },
///    "glyph_id": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad_monkey.outline_vector.a0"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.outline_vector"
///    },
///    "units_per_em": {
///      "$ref": "#/$defs/PositiveUint32"
///    },
///    "variations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/FontVariationCoordinate"
///      }
///    },
///    "version": {
///      "type": "string",
///      "const": "a0"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OutlineVectorA0 {
    pub case_id: crate::StableTextId,
    pub commands: ::std::vec::Vec<OutlineCommand>,
    pub coordinate_comparison: CoordinateComparisonPolicy,
    pub coordinate_format: ::std::string::String,
    pub face_index: u32,
    pub font_id: crate::StableTextId,
    pub font_sha256: Sha256Hex,
    pub glyph_id: u32,
    pub schema: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub units_per_em: crate::PositiveU32,
    pub variations: ::std::vec::Vec<FontVariationCoordinate>,
    pub version: ::std::string::String,
}
///Lowercase SHA-256 digest for one out-of-band font buffer.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Lowercase SHA-256 digest for one out-of-band font buffer.",
///  "type": "string"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct Sha256Hex(pub ::std::string::String);
impl ::std::ops::Deref for Sha256Hex {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Sha256Hex> for ::std::string::String {
    fn from(value: Sha256Hex) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Sha256Hex {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for Sha256Hex {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for Sha256Hex {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}

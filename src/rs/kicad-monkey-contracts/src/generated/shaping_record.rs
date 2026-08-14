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
///`DefaultIgnorablePolicy`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "normal",
///    "preserve",
///    "remove"
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
pub enum DefaultIgnorablePolicy {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "preserve")]
    Preserve,
    #[serde(rename = "remove")]
    Remove,
}
impl ::std::fmt::Display for DefaultIgnorablePolicy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Normal => f.write_str("normal"),
            Self::Preserve => f.write_str("preserve"),
            Self::Remove => f.write_str("remove"),
        }
    }
}
impl ::std::str::FromStr for DefaultIgnorablePolicy {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "normal" => Ok(Self::Normal),
            "preserve" => Ok(Self::Preserve),
            "remove" => Ok(Self::Remove),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DefaultIgnorablePolicy {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for DefaultIgnorablePolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DefaultIgnorablePolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
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
///      "type": "number"
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
    pub value: f64,
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
///One shaped glyph in logical buffer order.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One shaped glyph in logical buffer order.",
///  "type": "object",
///  "required": [
///    "cluster",
///    "glyph_id",
///    "safe_to_insert_tatweel",
///    "unsafe_to_break",
///    "unsafe_to_concat",
///    "x_advance",
///    "x_offset",
///    "y_advance",
///    "y_offset"
///  ],
///  "properties": {
///    "cluster": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "glyph_id": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "safe_to_insert_tatweel": {
///      "type": "boolean"
///    },
///    "unsafe_to_break": {
///      "type": "boolean"
///    },
///    "unsafe_to_concat": {
///      "type": "boolean"
///    },
///    "x_advance": {
///      "$ref": "#/$defs/TextSafeInteger"
///    },
///    "x_offset": {
///      "$ref": "#/$defs/TextSafeInteger"
///    },
///    "y_advance": {
///      "$ref": "#/$defs/TextSafeInteger"
///    },
///    "y_offset": {
///      "$ref": "#/$defs/TextSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShapedGlyph {
    pub cluster: u32,
    pub glyph_id: u32,
    pub safe_to_insert_tatweel: bool,
    pub unsafe_to_break: bool,
    pub unsafe_to_concat: bool,
    pub x_advance: crate::JavaScriptSafeInteger,
    pub x_offset: crate::JavaScriptSafeInteger,
    pub y_advance: crate::JavaScriptSafeInteger,
    pub y_offset: crate::JavaScriptSafeInteger,
}
///Explicit HarfBuzz buffer state; no ambient library defaults are implied.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Explicit HarfBuzz buffer state; no ambient library defaults are implied.",
///  "type": "object",
///  "required": [
///    "beginning_of_text",
///    "cluster_level",
///    "default_ignorables",
///    "do_not_insert_dotted_circle",
///    "end_of_text",
///    "produce_safe_to_insert_tatweel",
///    "produce_unsafe_to_concat"
///  ],
///  "properties": {
///    "beginning_of_text": {
///      "type": "boolean"
///    },
///    "cluster_level": {
///      "$ref": "#/$defs/ShapingClusterLevel"
///    },
///    "default_ignorables": {
///      "$ref": "#/$defs/DefaultIgnorablePolicy"
///    },
///    "do_not_insert_dotted_circle": {
///      "type": "boolean"
///    },
///    "end_of_text": {
///      "type": "boolean"
///    },
///    "produce_safe_to_insert_tatweel": {
///      "type": "boolean"
///    },
///    "produce_unsafe_to_concat": {
///      "type": "boolean"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShapingBufferProperties {
    pub beginning_of_text: bool,
    pub cluster_level: ShapingClusterLevel,
    pub default_ignorables: DefaultIgnorablePolicy,
    pub do_not_insert_dotted_circle: bool,
    pub end_of_text: bool,
    pub produce_safe_to_insert_tatweel: bool,
    pub produce_unsafe_to_concat: bool,
}
///`ShapingClusterLevel`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "monotone_graphemes",
///    "monotone_characters",
///    "characters"
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
pub enum ShapingClusterLevel {
    #[serde(rename = "monotone_graphemes")]
    MonotoneGraphemes,
    #[serde(rename = "monotone_characters")]
    MonotoneCharacters,
    #[serde(rename = "characters")]
    Characters,
}
impl ::std::fmt::Display for ShapingClusterLevel {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::MonotoneGraphemes => f.write_str("monotone_graphemes"),
            Self::MonotoneCharacters => f.write_str("monotone_characters"),
            Self::Characters => f.write_str("characters"),
        }
    }
}
impl ::std::str::FromStr for ShapingClusterLevel {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "monotone_graphemes" => Ok(Self::MonotoneGraphemes),
            "monotone_characters" => Ok(Self::MonotoneCharacters),
            "characters" => Ok(Self::Characters),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ShapingClusterLevel {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ShapingClusterLevel {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ShapingClusterLevel {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///HarfBuzz-compatible feature range over input scalar indices.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "HarfBuzz-compatible feature range over input scalar indices.",
///  "type": "object",
///  "required": [
///    "end",
///    "start",
///    "tag",
///    "value"
///  ],
///  "properties": {
///    "end": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "start": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "tag": {
///      "$ref": "#/$defs/OpenTypeTag"
///    },
///    "value": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShapingFeature {
    pub end: u32,
    pub start: u32,
    pub tag: OpenTypeTag,
    pub value: u32,
}
///Complete deterministic shaping input retained with an oracle record.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Complete deterministic shaping input retained with an oracle record.",
///  "type": "object",
///  "required": [
///    "buffer_properties",
///    "direction",
///    "face_index",
///    "features",
///    "font_id",
///    "font_sha256",
///    "scale_x",
///    "scale_y",
///    "text",
///    "variations"
///  ],
///  "properties": {
///    "buffer_properties": {
///      "$ref": "#/$defs/ShapingBufferProperties"
///    },
///    "direction": {
///      "$ref": "#/$defs/TextDirection"
///    },
///    "face_index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "features": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/ShapingFeature"
///      }
///    },
///    "font_id": {
///      "type": "string"
///    },
///    "font_sha256": {
///      "$ref": "#/$defs/Sha256Hex"
///    },
///    "language": {
///      "type": "string"
///    },
///    "scale_x": {
///      "$ref": "#/$defs/TextSafeInteger"
///    },
///    "scale_y": {
///      "$ref": "#/$defs/TextSafeInteger"
///    },
///    "script": {
///      "$ref": "#/$defs/OpenTypeTag"
///    },
///    "text": {
///      "type": "string"
///    },
///    "variations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/FontVariationCoordinate"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShapingInput {
    pub buffer_properties: ShapingBufferProperties,
    pub direction: TextDirection,
    pub face_index: u32,
    pub features: ::std::vec::Vec<ShapingFeature>,
    pub font_id: ::std::string::String,
    pub font_sha256: Sha256Hex,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub language: ::std::option::Option<::std::string::String>,
    pub scale_x: crate::JavaScriptSafeInteger,
    pub scale_y: crate::JavaScriptSafeInteger,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub script: ::std::option::Option<OpenTypeTag>,
    pub text: ::std::string::String,
    pub variations: ::std::vec::Vec<FontVariationCoordinate>,
}
///Intermediate shaping oracle, intentionally separate from glyph outlines.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.shaping_record:a0",
///  "title": "Shaping record a0",
///  "description": "Intermediate shaping oracle, intentionally separate from glyph outlines.",
///  "type": "object",
///  "required": [
///    "case_id",
///    "comparison",
///    "glyphs",
///    "input",
///    "schema",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "case_id": {
///      "type": "string"
///    },
///    "comparison": {
///      "$ref": "#/$defs/ExactComparisonPolicy"
///    },
///    "glyphs": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/ShapedGlyph"
///      }
///    },
///    "input": {
///      "$ref": "#/$defs/ShapingInput"
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad_monkey.shaping_record.a0"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.shaping_record"
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
pub struct ShapingRecordA0 {
    pub case_id: ::std::string::String,
    pub comparison: ExactComparisonPolicy,
    pub glyphs: ::std::vec::Vec<ShapedGlyph>,
    pub input: ShapingInput,
    pub schema: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///`TextDirection`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "left_to_right",
///    "right_to_left",
///    "top_to_bottom",
///    "bottom_to_top"
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
pub enum TextDirection {
    #[serde(rename = "left_to_right")]
    LeftToRight,
    #[serde(rename = "right_to_left")]
    RightToLeft,
    #[serde(rename = "top_to_bottom")]
    TopToBottom,
    #[serde(rename = "bottom_to_top")]
    BottomToTop,
}
impl ::std::fmt::Display for TextDirection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::LeftToRight => f.write_str("left_to_right"),
            Self::RightToLeft => f.write_str("right_to_left"),
            Self::TopToBottom => f.write_str("top_to_bottom"),
            Self::BottomToTop => f.write_str("bottom_to_top"),
        }
    }
}
impl ::std::str::FromStr for TextDirection {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "left_to_right" => Ok(Self::LeftToRight),
            "right_to_left" => Ok(Self::RightToLeft),
            "top_to_bottom" => Ok(Self::TopToBottom),
            "bottom_to_top" => Ok(Self::BottomToTop),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TextDirection {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TextDirection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TextDirection {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}

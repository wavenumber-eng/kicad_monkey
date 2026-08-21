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
///One font face whose bytes are supplied in a separate binary slot.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One font face whose bytes are supplied in a separate binary slot.",
///  "type": "object",
///  "required": [
///    "aliases",
///    "face_index",
///    "id",
///    "sha256",
///    "slot",
///    "variations"
///  ],
///  "properties": {
///    "aliases": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "face_index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "family": {
///      "type": "string"
///    },
///    "id": {
///      "$ref": "#/$defs/StableTextId"
///    },
///    "postscript_name": {
///      "type": "string"
///    },
///    "sha256": {
///      "$ref": "#/$defs/Sha256Hex"
///    },
///    "slot": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "style": {
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
pub struct FontBundleEntry {
    pub aliases: ::std::vec::Vec<::std::string::String>,
    pub face_index: u32,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub family: ::std::option::Option<::std::string::String>,
    pub id: crate::StableTextId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub postscript_name: ::std::option::Option<::std::string::String>,
    pub sha256: Sha256Hex,
    pub slot: u32,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub style: ::std::option::Option<::std::string::String>,
    pub variations: ::std::vec::Vec<FontVariationCoordinate>,
}
///Metadata for font buffers supplied out of band in matching numeric slots.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.font_bundle:a0",
///  "title": "Font bundle manifest a0",
///  "description": "Metadata for font buffers supplied out of band in matching numeric slots.",
///  "type": "object",
///  "required": [
///    "fonts",
///    "schema",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "fonts": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/FontBundleEntry"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad_monkey.font_bundle.a0"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.font_bundle"
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
pub struct FontBundleManifestA0 {
    pub fonts: ::std::vec::Vec<FontBundleEntry>,
    pub schema: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
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

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
///Canonical decimal wire encoding for an unsigned 64-bit byte count.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Canonical decimal wire encoding for an unsigned 64-bit byte count.",
///  "type": "string"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct CanonicalUint64Decimal(pub ::std::string::String);
impl ::std::ops::Deref for CanonicalUint64Decimal {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CanonicalUint64Decimal> for ::std::string::String {
    fn from(value: CanonicalUint64Decimal) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for CanonicalUint64Decimal {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for CanonicalUint64Decimal {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for CanonicalUint64Decimal {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///Portable inventory for a named multi-file schematic compiler input.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.source_bundle_manifest:a0",
///  "title": "Source bundle manifest a0",
///  "description": "Portable inventory for a named multi-file schematic compiler input.",
///  "type": "object",
///  "required": [
///    "root_schematic_path",
///    "schema",
///    "sources",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "project_path": {
///      "type": "string"
///    },
///    "root_schematic_path": {
///      "type": "string"
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad_monkey.source_bundle_manifest.a0"
///    },
///    "sources": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/SourceBundleSource"
///      }
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.source_bundle_manifest"
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
pub struct SourceBundleManifestA0 {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub project_path: ::std::option::Option<::std::string::String>,
    pub root_schematic_path: ::std::string::String,
    pub schema: ::std::string::String,
    pub sources: ::std::vec::Vec<SourceBundleSource>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///Metadata for one named byte buffer supplied out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Metadata for one named byte buffer supplied out of band.",
///  "type": "object",
///  "required": [
///    "kind",
///    "path",
///    "slot",
///    "source_bytes"
///  ],
///  "properties": {
///    "kind": {
///      "$ref": "#/$defs/SourceKind"
///    },
///    "path": {
///      "type": "string"
///    },
///    "slot": {
///      "$ref": "#/$defs/SourceSlot"
///    },
///    "source_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SourceBundleSource {
    pub kind: SourceKind,
    pub path: ::std::string::String,
    pub slot: SourceSlot,
    pub source_bytes: CanonicalUint64Decimal,
}
///KiCad source role within one schematic compiler input bundle.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "KiCad source role within one schematic compiler input bundle.",
///  "type": "string",
///  "enum": [
///    "project",
///    "schematic",
///    "symbol_library",
///    "symbol_table",
///    "worksheet",
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
pub enum SourceKind {
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "schematic")]
    Schematic,
    #[serde(rename = "symbol_library")]
    SymbolLibrary,
    #[serde(rename = "symbol_table")]
    SymbolTable,
    #[serde(rename = "worksheet")]
    Worksheet,
    #[serde(rename = "other")]
    Other,
}
impl ::std::fmt::Display for SourceKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Project => f.write_str("project"),
            Self::Schematic => f.write_str("schematic"),
            Self::SymbolLibrary => f.write_str("symbol_library"),
            Self::SymbolTable => f.write_str("symbol_table"),
            Self::Worksheet => f.write_str("worksheet"),
            Self::Other => f.write_str("other"),
        }
    }
}
impl ::std::str::FromStr for SourceKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "project" => Ok(Self::Project),
            "schematic" => Ok(Self::Schematic),
            "symbol_library" => Ok(Self::SymbolLibrary),
            "symbol_table" => Ok(Self::SymbolTable),
            "worksheet" => Ok(Self::Worksheet),
            "other" => Ok(Self::Other),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SourceKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Zero-based byte-buffer slot within one manifest.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Zero-based byte-buffer slot within one manifest.",
///  "type": "integer",
///  "maximum": 4294967295.0,
///  "minimum": 0.0
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct SourceSlot(pub u32);
impl ::std::ops::Deref for SourceSlot {
    type Target = u32;
    fn deref(&self) -> &u32 {
        &self.0
    }
}
impl ::std::convert::From<SourceSlot> for u32 {
    fn from(value: SourceSlot) -> Self {
        value.0
    }
}
impl ::std::convert::From<u32> for SourceSlot {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for SourceSlot {
    type Err = <u32 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for SourceSlot {
    type Error = <u32 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for SourceSlot {
    type Error = <u32 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::fmt::Display for SourceSlot {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}

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
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "source_bytes": {
///      "type": "string"
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
    pub slot: u32,
    pub source_bytes: ::std::string::String,
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

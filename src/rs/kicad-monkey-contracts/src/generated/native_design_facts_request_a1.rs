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
///Caller-selected resource ceilings for one native design-facts operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Caller-selected resource ceilings for one native design-facts operation.",
///  "type": "object",
///  "required": [
///    "max_output_bytes",
///    "max_path_bytes",
///    "max_source_bytes",
///    "max_sources",
///    "max_total_source_bytes"
///  ],
///  "properties": {
///    "max_output_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_path_bytes": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_source_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_sources": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_total_source_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeDesignFactsLimits {
    pub max_output_bytes: CanonicalUint64Decimal,
    pub max_path_bytes: u32,
    pub max_source_bytes: CanonicalUint64Decimal,
    pub max_sources: u32,
    pub max_total_source_bytes: CanonicalUint64Decimal,
}
///Strict bounded a1 request for native compiled-graph and version-E netlist facts.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.design_facts.request:a1",
///  "title": "Native design facts request a1",
///  "description": "Strict bounded a1 request for native compiled-graph and version-E netlist facts.",
///  "type": "object",
///  "required": [
///    "bundle_root",
///    "file_slots",
///    "limits",
///    "manifest",
///    "netlist",
///    "resource_profile",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "bundle_root": {
///      "type": "string"
///    },
///    "file_slots": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/NativeFileSlot"
///      }
///    },
///    "limits": {
///      "$ref": "#/$defs/NativeDesignFactsLimits"
///    },
///    "manifest": {
///      "$ref": "#/$defs/NativeSourceBundleManifestProjection"
///    },
///    "netlist": {
///      "$ref": "#/$defs/NativeNetlistMetadata"
///    },
///    "resource_profile": {
///      "type": "string",
///      "const": "design-facts-bounded-a1"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.design_facts.request"
///    },
///    "version": {
///      "type": "string",
///      "const": "a1"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeDesignFactsRequestA1 {
    pub bundle_root: ::std::string::String,
    pub file_slots: ::std::vec::Vec<NativeFileSlot>,
    pub limits: NativeDesignFactsLimits,
    pub manifest: crate::generated::source_bundle_manifest::SourceBundleManifestA0,
    pub netlist: NativeNetlistMetadata,
    pub resource_profile: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///File-system carrier for one zero-based source-bundle byte slot.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "File-system carrier for one zero-based source-bundle byte slot.",
///  "type": "object",
///  "required": [
///    "path",
///    "slot"
///  ],
///  "properties": {
///    "path": {
///      "type": "string"
///    },
///    "slot": {
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
pub struct NativeFileSlot {
    pub path: ::std::string::String,
    pub slot: u32,
}
///Metadata written into the version-E KiCad netlist header.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Metadata written into the version-E KiCad netlist header.",
///  "type": "object",
///  "required": [
///    "date",
///    "source_path",
///    "tool"
///  ],
///  "properties": {
///    "date": {
///      "type": "string"
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "tool": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeNetlistMetadata {
    pub date: ::std::string::String,
    pub source_path: ::std::string::String,
    pub tool: ::std::string::String,
}

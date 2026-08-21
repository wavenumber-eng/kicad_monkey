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
///Strict bounded a1 result with source identity and netlist byte integrity.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.design_facts.result:a1",
///  "title": "Native design facts result a1",
///  "description": "Strict bounded a1 result with source identity and netlist byte integrity.",
///  "type": "object",
///  "required": [
///    "compiled_schematic_graph",
///    "engine_version",
///    "kicad_netlist",
///    "kicad_netlist_bytes",
///    "kicad_netlist_sha256",
///    "kicad_netlist_version",
///    "resource_profile",
///    "source_snapshot_sha256",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "compiled_schematic_graph": {
///      "$ref": "#/$defs/NativeCompiledSchematicGraphProjection"
///    },
///    "engine_version": {
///      "type": "string"
///    },
///    "kicad_netlist": {
///      "type": "string"
///    },
///    "kicad_netlist_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "kicad_netlist_sha256": {
///      "type": "string"
///    },
///    "kicad_netlist_version": {
///      "type": "string",
///      "const": "E"
///    },
///    "resource_profile": {
///      "type": "string",
///      "const": "design-facts-bounded-a1"
///    },
///    "source_snapshot_sha256": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.design_facts.result"
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
pub struct NativeDesignFactsResultA1 {
    pub compiled_schematic_graph:
        crate::generated::compiled_schematic_graph::CompiledSchematicGraphA0,
    pub engine_version: ::std::string::String,
    pub kicad_netlist: ::std::string::String,
    pub kicad_netlist_bytes: CanonicalUint64Decimal,
    pub kicad_netlist_sha256: ::std::string::String,
    pub kicad_netlist_version: ::std::string::String,
    pub resource_profile: ::std::string::String,
    pub source_snapshot_sha256: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}

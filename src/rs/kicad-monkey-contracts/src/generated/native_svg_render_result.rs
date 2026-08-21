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
///Strict result for deterministic, presentation-neutral base SVG.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.svg.result:a0",
///  "title": "Native SVG render result a0",
///  "description": "Strict result for deterministic, presentation-neutral base SVG.",
///  "type": "object",
///  "required": [
///    "document_id",
///    "engine_version",
///    "profile",
///    "source_kind",
///    "svg_bytes",
///    "svg_sha256",
///    "svg_utf8",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "document_id": {
///      "type": "string"
///    },
///    "engine_version": {
///      "type": "string"
///    },
///    "profile": {
///      "type": "string",
///      "const": "plotter-base-a0"
///    },
///    "source_kind": {
///      "anyOf": [
///        {
///          "type": "string",
///          "const": "MOD"
///        },
///        {
///          "type": "string",
///          "const": "SYM"
///        },
///        {
///          "type": "string",
///          "const": "PCB"
///        },
///        {
///          "type": "string",
///          "const": "SCH"
///        }
///      ]
///    },
///    "svg_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "svg_sha256": {
///      "type": "string"
///    },
///    "svg_utf8": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.svg.result"
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
pub struct NativeSvgRenderResultA0 {
    pub document_id: ::std::string::String,
    pub engine_version: ::std::string::String,
    pub profile: ::std::string::String,
    pub source_kind: NativeSvgRenderResultA0SourceKind,
    pub svg_bytes: CanonicalUint64Decimal,
    pub svg_sha256: ::std::string::String,
    pub svg_utf8: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///`NativeSvgRenderResultA0SourceKind`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "MOD"
///    },
///    {
///      "type": "string",
///      "const": "SYM"
///    },
///    {
///      "type": "string",
///      "const": "PCB"
///    },
///    {
///      "type": "string",
///      "const": "SCH"
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
pub enum NativeSvgRenderResultA0SourceKind {
    #[serde(rename = "MOD")]
    Mod,
    #[serde(rename = "SYM")]
    Sym,
    #[serde(rename = "PCB")]
    Pcb,
    #[serde(rename = "SCH")]
    Sch,
}
impl ::std::fmt::Display for NativeSvgRenderResultA0SourceKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Mod => f.write_str("MOD"),
            Self::Sym => f.write_str("SYM"),
            Self::Pcb => f.write_str("PCB"),
            Self::Sch => f.write_str("SCH"),
        }
    }
}
impl ::std::str::FromStr for NativeSvgRenderResultA0SourceKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "MOD" => Ok(Self::Mod),
            "SYM" => Ok(Self::Sym),
            "PCB" => Ok(Self::Pcb),
            "SCH" => Ok(Self::Sch),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for NativeSvgRenderResultA0SourceKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for NativeSvgRenderResultA0SourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NativeSvgRenderResultA0SourceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}

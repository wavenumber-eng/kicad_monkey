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
///Stable operation diagnostic shared by native, Python, and browser adapters.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Stable operation diagnostic shared by native, Python, and browser adapters.",
///  "type": "object",
///  "required": [
///    "code",
///    "message",
///    "phase"
///  ],
///  "properties": {
///    "code": {
///      "type": "string"
///    },
///    "message": {
///      "type": "string"
///    },
///    "phase": {
///      "$ref": "#/$defs/DiagnosticPhase"
///    },
///    "position": {
///      "$ref": "#/$defs/SourcePosition"
///    },
///    "token": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: ::std::string::String,
    pub message: ::std::string::String,
    pub phase: DiagnosticPhase,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub position: ::std::option::Option<SourcePosition>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub token: ::std::option::Option<::std::string::String>,
}
///`DiagnosticPhase`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "lex",
///    "tree",
///    "build"
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
pub enum DiagnosticPhase {
    #[serde(rename = "lex")]
    Lex,
    #[serde(rename = "tree")]
    Tree,
    #[serde(rename = "build")]
    Build,
}
impl ::std::fmt::Display for DiagnosticPhase {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Lex => f.write_str("lex"),
            Self::Tree => f.write_str("tree"),
            Self::Build => f.write_str("build"),
        }
    }
}
impl ::std::str::FromStr for DiagnosticPhase {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "lex" => Ok(Self::Lex),
            "tree" => Ok(Self::Tree),
            "build" => Ok(Self::Build),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DiagnosticPhase {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for DiagnosticPhase {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DiagnosticPhase {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///One decoded top-level KiCad footprint property.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One decoded top-level KiCad footprint property.",
///  "type": "object",
///  "required": [
///    "name",
///    "value"
///  ],
///  "properties": {
///    "name": {
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
pub struct FootprintProperty {
    pub name: ::std::string::String,
    pub value: ::std::string::String,
}
///Typed footprint facts; KiCad source bytes remain in the caller-owned input.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.footprint_read.result:a0",
///  "title": "Footprint read result a0",
///  "description": "Typed footprint facts; KiCad source bytes remain in the caller-owned input.",
///  "type": "object",
///  "required": [
///    "diagnostics",
///    "name",
///    "pad_count",
///    "properties",
///    "source_bytes",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "diagnostics": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/Diagnostic"
///      }
///    },
///    "name": {
///      "type": "string"
///    },
///    "pad_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "properties": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/FootprintProperty"
///      }
///    },
///    "source_bytes": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.footprint_read.result"
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
pub struct FootprintReadResultA0 {
    pub diagnostics: ::std::vec::Vec<Diagnostic>,
    pub name: ::std::string::String,
    pub pad_count: u32,
    pub properties: ::std::vec::Vec<FootprintProperty>,
    pub source_bytes: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///Zero-based UTF-8 byte offset with one-based source coordinates.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Zero-based UTF-8 byte offset with one-based source coordinates.",
///  "type": "object",
///  "required": [
///    "column",
///    "line",
///    "offset"
///  ],
///  "properties": {
///    "column": {
///      "type": "string"
///    },
///    "line": {
///      "type": "string"
///    },
///    "offset": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SourcePosition {
    pub column: ::std::string::String,
    pub line: ::std::string::String,
    pub offset: ::std::string::String,
}

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
///One complete selected form in the original UTF-8 byte buffer.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One complete selected form in the original UTF-8 byte buffer.",
///  "type": "object",
///  "required": [
///    "column",
///    "depth",
///    "end_column",
///    "end_line",
///    "end_offset",
///    "line",
///    "path",
///    "start_offset"
///  ],
///  "properties": {
///    "column": {
///      "type": "string"
///    },
///    "depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "end_column": {
///      "type": "string"
///    },
///    "end_line": {
///      "type": "string"
///    },
///    "end_offset": {
///      "type": "string"
///    },
///    "head": {
///      "type": "string"
///    },
///    "line": {
///      "type": "string"
///    },
///    "path": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "start_offset": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FormSpan {
    pub column: ::std::string::String,
    pub depth: u32,
    pub end_column: ::std::string::String,
    pub end_line: ::std::string::String,
    pub end_offset: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub head: ::std::option::Option<::std::string::String>,
    pub line: ::std::string::String,
    pub path: ::std::vec::Vec<::std::string::String>,
    pub start_offset: ::std::string::String,
}
///Result envelope; selected source bytes remain in the caller-owned input buffer.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.sexpr_scan.result:a0",
///  "title": "S-expression scan result a0",
///  "description": "Result envelope; selected source bytes remain in the caller-owned input buffer.",
///  "type": "object",
///  "required": [
///    "diagnostics",
///    "forms",
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
///    "forms": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/FormSpan"
///      }
///    },
///    "source_bytes": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.sexpr_scan.result"
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
pub struct SExpressionScanResultA0 {
    pub diagnostics: ::std::vec::Vec<Diagnostic>,
    pub forms: ::std::vec::Vec<FormSpan>,
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

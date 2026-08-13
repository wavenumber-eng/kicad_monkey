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
///Typed symbol-library facts retaining source order.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.symbol_library_read.result:a0",
///  "title": "Symbol library read result a0",
///  "description": "Typed symbol-library facts retaining source order.",
///  "type": "object",
///  "required": [
///    "diagnostics",
///    "source_bytes",
///    "symbols",
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
///    "source_bytes": {
///      "type": "string"
///    },
///    "symbols": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/SymbolSummary"
///      }
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.symbol_library_read.result"
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
pub struct SymbolLibraryReadResultA0 {
    pub diagnostics: ::std::vec::Vec<Diagnostic>,
    pub source_bytes: ::std::string::String,
    pub symbols: ::std::vec::Vec<SymbolSummary>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///One source-backed top-level symbol summary.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One source-backed top-level symbol summary.",
///  "type": "object",
///  "required": [
///    "in_bom",
///    "name",
///    "on_board",
///    "pin_count",
///    "power",
///    "property_count",
///    "subsymbol_count"
///  ],
///  "properties": {
///    "extends": {
///      "type": "string"
///    },
///    "in_bom": {
///      "type": "boolean"
///    },
///    "name": {
///      "type": "string"
///    },
///    "on_board": {
///      "type": "boolean"
///    },
///    "pin_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "power": {
///      "type": "boolean"
///    },
///    "power_kind": {
///      "type": "string"
///    },
///    "property_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "subsymbol_count": {
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
pub struct SymbolSummary {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub extends: ::std::option::Option<::std::string::String>,
    pub in_bom: bool,
    pub name: ::std::string::String,
    pub on_board: bool,
    pub pin_count: u32,
    pub power: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub power_kind: ::std::option::Option<::std::string::String>,
    pub property_count: u32,
    pub subsymbol_count: u32,
}

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
///Resource-bounded library-symbol plot operation. Source bytes are out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.symbol_plot.request:a0",
///  "title": "Symbol plot request a0",
///  "description": "Resource-bounded library-symbol plot operation. Source bytes are out of band.",
///  "type": "object",
///  "required": [
///    "max_depth",
///    "max_operations",
///    "max_output_bytes",
///    "max_points",
///    "max_source_bytes",
///    "max_subsymbols",
///    "max_symbols",
///    "style",
///    "symbol_name",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "document_id": {
///      "type": "string"
///    },
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_operations": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_output_bytes": {
///      "type": "string"
///    },
///    "max_points": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_source_bytes": {
///      "type": "string"
///    },
///    "max_subsymbols": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_symbols": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_text_bytes": {
///      "type": "string"
///    },
///    "max_text_carriers": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "style": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "symbol_name": {
///      "type": "string"
///    },
///    "text_variables": {
///      "description": "Exact-case project variables used only by library-symbol body text.",
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/SymbolTextVariable"
///      }
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.symbol_plot.request"
///    },
///    "unit": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
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
pub struct SymbolPlotRequestA0 {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub document_id: ::std::option::Option<::std::string::String>,
    pub max_depth: u32,
    pub max_operations: u32,
    pub max_output_bytes: ::std::string::String,
    pub max_points: u32,
    pub max_source_bytes: ::std::string::String,
    pub max_subsymbols: u32,
    pub max_symbols: u32,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub max_text_bytes: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub max_text_carriers: ::std::option::Option<u32>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_path: ::std::option::Option<::std::string::String>,
    pub style: u32,
    pub symbol_name: ::std::string::String,
    ///Exact-case project variables used only by library-symbol body text.
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub text_variables: ::std::vec::Vec<SymbolTextVariable>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub unit: ::std::option::Option<u32>,
    pub version: ::std::string::String,
}
///One exact-case project-sidecar variable for library-symbol body text.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One exact-case project-sidecar variable for library-symbol body text.",
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
pub struct SymbolTextVariable {
    pub name: ::std::string::String,
    pub value: ::std::string::String,
}

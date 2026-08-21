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
///Metadata envelope for a byte-buffer structural scan operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.sexpr_scan.request:a0",
///  "title": "S-expression scan request a0",
///  "description": "Metadata envelope for a byte-buffer structural scan operation.",
///  "type": "object",
///  "required": [
///    "max_depth",
///    "max_selected_forms",
///    "max_source_bytes",
///    "selector",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_selected_forms": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_source_bytes": {
///      "type": "string"
///    },
///    "selector": {
///      "$ref": "#/$defs/Selector"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.sexpr_scan.request"
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
pub struct SExpressionScanRequestA0 {
    pub max_depth: u32,
    pub max_selected_forms: u32,
    pub max_source_bytes: ::std::string::String,
    pub selector: Selector,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///Generic structural selector; source bytes are always supplied out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Generic structural selector; source bytes are always supplied out of band.",
///  "type": "object",
///  "properties": {
///    "heads": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "min_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "paths": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "type": "string"
///        }
///      }
///    },
///    "prune_heads": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub heads: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub max_depth: ::std::option::Option<u32>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub min_depth: ::std::option::Option<u32>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub paths: ::std::vec::Vec<::std::vec::Vec<::std::string::String>>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub prune_heads: ::std::vec::Vec<::std::string::String>,
}
impl ::std::default::Default for Selector {
    fn default() -> Self {
        Self {
            heads: Default::default(),
            max_depth: Default::default(),
            min_depth: Default::default(),
            paths: Default::default(),
            prune_heads: Default::default(),
        }
    }
}

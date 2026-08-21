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
///`SymbolBooleanField`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "in_bom",
///    "on_board"
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
pub enum SymbolBooleanField {
    #[serde(rename = "in_bom")]
    InBom,
    #[serde(rename = "on_board")]
    OnBoard,
}
impl ::std::fmt::Display for SymbolBooleanField {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::InBom => f.write_str("in_bom"),
            Self::OnBoard => f.write_str("on_board"),
        }
    }
}
impl ::std::str::FromStr for SymbolBooleanField {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "in_bom" => Ok(Self::InBom),
            "on_board" => Ok(Self::OnBoard),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SymbolBooleanField {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SymbolBooleanField {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SymbolBooleanField {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Focused source-preserving edit of one symbol boolean field.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.symbol_library_edit.request:a0",
///  "title": "Symbol library edit request a0",
///  "description": "Focused source-preserving edit of one symbol boolean field.",
///  "type": "object",
///  "required": [
///    "field",
///    "max_depth",
///    "max_metadata_forms",
///    "max_output_bytes",
///    "max_pins",
///    "max_source_bytes",
///    "max_subsymbols",
///    "max_symbols",
///    "symbol_name",
///    "type",
///    "value",
///    "version"
///  ],
///  "properties": {
///    "field": {
///      "$ref": "#/$defs/SymbolBooleanField"
///    },
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_metadata_forms": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_output_bytes": {
///      "type": "string"
///    },
///    "max_pins": {
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
///    "symbol_name": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.symbol_library_edit.request"
///    },
///    "value": {
///      "type": "boolean"
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
pub struct SymbolLibraryEditRequestA0 {
    pub field: SymbolBooleanField,
    pub max_depth: u32,
    pub max_metadata_forms: u32,
    pub max_output_bytes: ::std::string::String,
    pub max_pins: u32,
    pub max_source_bytes: ::std::string::String,
    pub max_subsymbols: u32,
    pub max_symbols: u32,
    pub symbol_name: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub value: bool,
    pub version: ::std::string::String,
}

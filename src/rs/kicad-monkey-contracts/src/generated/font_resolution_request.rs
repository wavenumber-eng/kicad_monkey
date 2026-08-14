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
///Selection metadata paired with a FontBundle and out-of-band buffers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.font_resolution.request:a0",
///  "title": "Font resolution request a0",
///  "description": "Selection metadata paired with a FontBundle and out-of-band buffers.",
///  "type": "object",
///  "required": [
///    "schema",
///    "selection",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "schema": {
///      "type": "string",
///      "const": "kicad_monkey.font_resolution_request.a0"
///    },
///    "selection": {
///      "$ref": "#/$defs/FontSelection"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.font_resolution_request"
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
pub struct FontResolutionRequestA0 {
    pub schema: ::std::string::String,
    pub selection: FontSelection,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///Deterministic font request: explicit ID wins, otherwise aliases are matched.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Deterministic font request: explicit ID wins, otherwise aliases are matched.",
///  "type": "object",
///  "required": [
///    "aliases"
///  ],
///  "properties": {
///    "aliases": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "font_id": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FontSelection {
    pub aliases: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub font_id: ::std::option::Option<::std::string::String>,
}

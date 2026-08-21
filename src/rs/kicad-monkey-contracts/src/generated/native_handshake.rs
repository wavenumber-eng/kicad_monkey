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
///Exact native process handshake.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.handshake:a0",
///  "title": "Native handshake a0",
///  "description": "Exact native process handshake.",
///  "type": "object",
///  "required": [
///    "engine_version",
///    "operations",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "engine_version": {
///      "type": "string"
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "type": "string",
///        "const": "design-facts"
///      },
///      "maxItems": 1,
///      "minItems": 1
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.handshake"
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
pub struct NativeHandshakeA0 {
    pub engine_version: ::std::string::String,
    pub operations: [::std::string::String; 1usize],
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}

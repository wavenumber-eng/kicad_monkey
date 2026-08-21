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
///Exact native process handshake with the bounded a1 design-facts operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.handshake:a2",
///  "title": "Native handshake a2",
///  "description": "Exact native process handshake with the bounded a1 design-facts operation.",
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
///        "anyOf": [
///          {
///            "type": "string",
///            "const": "design-facts"
///          },
///          {
///            "type": "string",
///            "const": "render-svg"
///          },
///          {
///            "type": "string",
///            "const": "design-facts-a1"
///          }
///        ]
///      },
///      "maxItems": 3,
///      "minItems": 3
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.handshake"
///    },
///    "version": {
///      "type": "string",
///      "const": "a2"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeHandshakeA2 {
    pub engine_version: ::std::string::String,
    pub operations: [NativeHandshakeA2OperationsItem; 3usize],
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///`NativeHandshakeA2OperationsItem`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "anyOf": [
///    {
///      "type": "string",
///      "const": "design-facts"
///    },
///    {
///      "type": "string",
///      "const": "render-svg"
///    },
///    {
///      "type": "string",
///      "const": "design-facts-a1"
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
pub enum NativeHandshakeA2OperationsItem {
    #[serde(rename = "design-facts")]
    DesignFacts,
    #[serde(rename = "render-svg")]
    RenderSvg,
    #[serde(rename = "design-facts-a1")]
    DesignFactsA1,
}
impl ::std::fmt::Display for NativeHandshakeA2OperationsItem {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::DesignFacts => f.write_str("design-facts"),
            Self::RenderSvg => f.write_str("render-svg"),
            Self::DesignFactsA1 => f.write_str("design-facts-a1"),
        }
    }
}
impl ::std::str::FromStr for NativeHandshakeA2OperationsItem {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "design-facts" => Ok(Self::DesignFacts),
            "render-svg" => Ok(Self::RenderSvg),
            "design-facts-a1" => Ok(Self::DesignFactsA1),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for NativeHandshakeA2OperationsItem {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for NativeHandshakeA2OperationsItem {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NativeHandshakeA2OperationsItem {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}

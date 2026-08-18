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
///Structured stderr payload for a failed native process operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.error:a0",
///  "title": "Native error a0",
///  "description": "Structured stderr payload for a failed native process operation.",
///  "type": "object",
///  "required": [
///    "kind",
///    "message",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "kind": {
///      "$ref": "#/$defs/NativeErrorKind"
///    },
///    "message": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.error"
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
pub struct NativeErrorA0 {
    pub kind: NativeErrorKind,
    pub message: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
///Supported native process error category.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Supported native process error category.",
///  "type": "string",
///  "enum": [
///    "request",
///    "path",
///    "io",
///    "resource_limit",
///    "core"
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
pub enum NativeErrorKind {
    #[serde(rename = "request")]
    Request,
    #[serde(rename = "path")]
    Path,
    #[serde(rename = "io")]
    Io,
    #[serde(rename = "resource_limit")]
    ResourceLimit,
    #[serde(rename = "core")]
    Core,
}
impl ::std::fmt::Display for NativeErrorKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Request => f.write_str("request"),
            Self::Path => f.write_str("path"),
            Self::Io => f.write_str("io"),
            Self::ResourceLimit => f.write_str("resource_limit"),
            Self::Core => f.write_str("core"),
        }
    }
}
impl ::std::str::FromStr for NativeErrorKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "request" => Ok(Self::Request),
            "path" => Ok(Self::Path),
            "io" => Ok(Self::Io),
            "resource_limit" => Ok(Self::ResourceLimit),
            "core" => Ok(Self::Core),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for NativeErrorKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for NativeErrorKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NativeErrorKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}

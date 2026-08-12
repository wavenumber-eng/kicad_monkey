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
///Portable generic S-expression tree node used only by explicit build operations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Portable generic S-expression tree node used only by explicit build operations.",
///  "type": "object",
///  "required": [
///    "kind"
///  ],
///  "properties": {
///    "children": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/Node"
///      }
///    },
///    "float": {
///      "type": "number"
///    },
///    "integer": {
///      "type": "string"
///    },
///    "kind": {
///      "$ref": "#/$defs/NodeKind"
///    },
///    "text": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Node {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub children: ::std::vec::Vec<Node>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub float: ::std::option::Option<f64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub integer: ::std::option::Option<::std::string::String>,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub text: ::std::option::Option<::std::string::String>,
}
///`NodeKind`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "list",
///    "atom",
///    "quoted",
///    "integer",
///    "float"
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
pub enum NodeKind {
    #[serde(rename = "list")]
    List,
    #[serde(rename = "atom")]
    Atom,
    #[serde(rename = "quoted")]
    Quoted,
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "float")]
    Float,
}
impl ::std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::List => f.write_str("list"),
            Self::Atom => f.write_str("atom"),
            Self::Quoted => f.write_str("quoted"),
            Self::Integer => f.write_str("integer"),
            Self::Float => f.write_str("float"),
        }
    }
}
impl ::std::str::FromStr for NodeKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "list" => Ok(Self::List),
            "atom" => Ok(Self::Atom),
            "quoted" => Ok(Self::Quoted),
            "integer" => Ok(Self::Integer),
            "float" => Ok(Self::Float),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for NodeKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for NodeKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for NodeKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Deterministic generic-tree build request.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.sexpr_build.request:a0",
///  "title": "S-expression build request a0",
///  "description": "Deterministic generic-tree build request.",
///  "type": "object",
///  "required": [
///    "max_depth",
///    "max_nodes",
///    "max_output_bytes",
///    "root",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_nodes": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_output_bytes": {
///      "type": "string"
///    },
///    "root": {
///      "$ref": "#/$defs/Node"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.sexpr_build.request"
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
pub struct SExpressionBuildRequestA0 {
    pub max_depth: u32,
    pub max_nodes: u32,
    pub max_output_bytes: ::std::string::String,
    pub root: Node,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}

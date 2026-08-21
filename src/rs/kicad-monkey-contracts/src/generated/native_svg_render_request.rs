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
///Canonical decimal wire encoding for an unsigned 64-bit byte count.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Canonical decimal wire encoding for an unsigned 64-bit byte count.",
///  "type": "string"
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize, ::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct CanonicalUint64Decimal(pub ::std::string::String);
impl ::std::ops::Deref for CanonicalUint64Decimal {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<CanonicalUint64Decimal> for ::std::string::String {
    fn from(value: CanonicalUint64Decimal) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for CanonicalUint64Decimal {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for CanonicalUint64Decimal {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl ::std::fmt::Display for CanonicalUint64Decimal {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///`NativeBoardSvgDocument`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind",
///    "value"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "board"
///    },
///    "value": {
///      "$ref": "#/$defs/NativeBoardPlotDocumentProjection"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeBoardSvgDocument {
    pub kind: ::std::string::String,
    pub value: ::serde_json::Value,
}
///`NativeFootprintSvgDocument`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind",
///    "value"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "footprint"
///    },
///    "value": {
///      "$ref": "#/$defs/NativeFootprintPlotDocumentProjection"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeFootprintSvgDocument {
    pub kind: ::std::string::String,
    pub value: ::serde_json::Value,
}
///`NativeSchematicSvgDocument`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind",
///    "value"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "schematic"
///    },
///    "value": {
///      "$ref": "#/$defs/NativeSchematicPlotDocumentProjection"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeSchematicSvgDocument {
    pub kind: ::std::string::String,
    pub value: ::serde_json::Value,
}
///One explicitly discriminated frozen Phase-5 plot-document root.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One explicitly discriminated frozen Phase-5 plot-document root.",
///  "anyOf": [
///    {
///      "$ref": "#/$defs/NativeFootprintSvgDocument"
///    },
///    {
///      "$ref": "#/$defs/NativeSymbolSvgDocument"
///    },
///    {
///      "$ref": "#/$defs/NativeBoardSvgDocument"
///    },
///    {
///      "$ref": "#/$defs/NativeSchematicSvgDocument"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum NativeSvgPlotDocument {
    FootprintSvgDocument(NativeFootprintSvgDocument),
    SymbolSvgDocument(NativeSymbolSvgDocument),
    BoardSvgDocument(NativeBoardSvgDocument),
    SchematicSvgDocument(NativeSchematicSvgDocument),
}
impl ::std::convert::From<NativeFootprintSvgDocument> for NativeSvgPlotDocument {
    fn from(value: NativeFootprintSvgDocument) -> Self {
        Self::FootprintSvgDocument(value)
    }
}
impl ::std::convert::From<NativeSymbolSvgDocument> for NativeSvgPlotDocument {
    fn from(value: NativeSymbolSvgDocument) -> Self {
        Self::SymbolSvgDocument(value)
    }
}
impl ::std::convert::From<NativeBoardSvgDocument> for NativeSvgPlotDocument {
    fn from(value: NativeBoardSvgDocument) -> Self {
        Self::BoardSvgDocument(value)
    }
}
impl ::std::convert::From<NativeSchematicSvgDocument> for NativeSvgPlotDocument {
    fn from(value: NativeSchematicSvgDocument) -> Self {
        Self::SchematicSvgDocument(value)
    }
}
///Positive viewport dimension that remains exact in JavaScript.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Positive viewport dimension that remains exact in JavaScript.",
///  "type": "integer",
///  "maximum": 9007199254740991.0,
///  "minimum": 1.0
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct NativeSvgPositiveSafeInteger(pub ::std::num::NonZeroU64);
impl ::std::ops::Deref for NativeSvgPositiveSafeInteger {
    type Target = ::std::num::NonZeroU64;
    fn deref(&self) -> &::std::num::NonZeroU64 {
        &self.0
    }
}
impl ::std::convert::From<NativeSvgPositiveSafeInteger> for ::std::num::NonZeroU64 {
    fn from(value: NativeSvgPositiveSafeInteger) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::num::NonZeroU64> for NativeSvgPositiveSafeInteger {
    fn from(value: ::std::num::NonZeroU64) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for NativeSvgPositiveSafeInteger {
    type Err = <::std::num::NonZeroU64 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for NativeSvgPositiveSafeInteger {
    type Error = <::std::num::NonZeroU64 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for NativeSvgPositiveSafeInteger {
    type Error = <::std::num::NonZeroU64 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::fmt::Display for NativeSvgPositiveSafeInteger {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///Aggregate resource ceilings for one native SVG serialization.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Aggregate resource ceilings for one native SVG serialization.",
///  "type": "object",
///  "required": [
///    "max_block_depth",
///    "max_image_encoded_bytes",
///    "max_operations",
///    "max_points",
///    "max_records",
///    "max_render_work",
///    "max_result_bytes",
///    "max_svg_bytes",
///    "max_svg_elements",
///    "max_text_bytes"
///  ],
///  "properties": {
///    "max_block_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_image_encoded_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_operations": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_points": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_records": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_render_work": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_result_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_svg_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_svg_elements": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    },
///    "max_text_bytes": {
///      "$ref": "#/$defs/CanonicalUint64Decimal"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeSvgRenderLimits {
    pub max_block_depth: u32,
    pub max_image_encoded_bytes: CanonicalUint64Decimal,
    pub max_operations: u32,
    pub max_points: CanonicalUint64Decimal,
    pub max_records: u32,
    pub max_render_work: CanonicalUint64Decimal,
    pub max_result_bytes: CanonicalUint64Decimal,
    pub max_svg_bytes: CanonicalUint64Decimal,
    pub max_svg_elements: CanonicalUint64Decimal,
    pub max_text_bytes: CanonicalUint64Decimal,
}
///Strict request for deterministic, presentation-neutral base SVG.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.svg.request:a0",
///  "title": "Native SVG render request a0",
///  "description": "Strict request for deterministic, presentation-neutral base SVG.",
///  "type": "object",
///  "required": [
///    "document",
///    "limits",
///    "profile",
///    "type",
///    "version",
///    "viewport"
///  ],
///  "properties": {
///    "document": {
///      "$ref": "#/$defs/NativeSvgPlotDocument"
///    },
///    "limits": {
///      "$ref": "#/$defs/NativeSvgRenderLimits"
///    },
///    "profile": {
///      "type": "string",
///      "const": "plotter-base-a0"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.svg.request"
///    },
///    "version": {
///      "type": "string",
///      "const": "a0"
///    },
///    "viewport": {
///      "$ref": "#/$defs/NativeSvgViewport"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeSvgRenderRequestA0 {
    pub document: NativeSvgPlotDocument,
    pub limits: NativeSvgRenderLimits,
    pub profile: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
    pub viewport: NativeSvgViewport,
}
///Explicit document viewport. Frozen MOD/SYM/PCB plot documents do not carry one.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Explicit document viewport. Frozen MOD/SYM/PCB plot documents do not carry one.",
///  "type": "object",
///  "required": [
///    "height_nm",
///    "min_x_nm",
///    "min_y_nm",
///    "width_nm"
///  ],
///  "properties": {
///    "height_nm": {
///      "$ref": "#/$defs/NativeSvgPositiveSafeInteger"
///    },
///    "min_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "min_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/NativeSvgPositiveSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeSvgViewport {
    pub height_nm: NativeSvgPositiveSafeInteger,
    pub min_x_nm: crate::JavaScriptSafeInteger,
    pub min_y_nm: crate::JavaScriptSafeInteger,
    pub width_nm: NativeSvgPositiveSafeInteger,
}
///`NativeSymbolSvgDocument`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "kind",
///    "value"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "symbol"
///    },
///    "value": {
///      "$ref": "#/$defs/NativeSymbolPlotDocumentProjection"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeSymbolSvgDocument {
    pub kind: ::std::string::String,
    pub value: ::serde_json::Value,
}

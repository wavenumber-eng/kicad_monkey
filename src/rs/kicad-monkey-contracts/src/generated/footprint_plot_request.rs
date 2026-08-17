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
///Resource-bounded footprint plotter operation. Source bytes are out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.footprint_plot.request:a0",
///  "title": "Footprint plot request a0",
///  "description": "Resource-bounded footprint plotter operation. Source bytes are out of band.",
///  "type": "object",
///  "required": [
///    "max_depth",
///    "max_metadata_forms",
///    "max_operations",
///    "max_output_bytes",
///    "max_points",
///    "max_source_bytes",
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
///    "max_metadata_forms": {
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
///    "max_text_bytes": {
///      "type": "string"
///    },
///    "max_text_carriers": {
///      "description": "Optional Phase 5 ceilings; older a0 requests receive bounded defaults.",
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.footprint_plot.request"
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
pub struct FootprintPlotRequestA0 {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub document_id: ::std::option::Option<::std::string::String>,
    pub max_depth: u32,
    pub max_metadata_forms: u32,
    pub max_operations: u32,
    pub max_output_bytes: ::std::string::String,
    pub max_points: u32,
    pub max_source_bytes: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub max_text_bytes: ::std::option::Option<::std::string::String>,
    ///Optional Phase 5 ceilings; older a0 requests receive bounded defaults.
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub max_text_carriers: ::std::option::Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub source_path: ::std::option::Option<::std::string::String>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}

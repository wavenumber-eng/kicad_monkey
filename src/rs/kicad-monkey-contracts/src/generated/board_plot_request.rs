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
/**One exact net-name to ordered net-class assignment mirrored from the
project sidecar's `net_settings.netclass_assignments` entries.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One exact net-name to ordered net-class assignment mirrored from the\nproject sidecar's `net_settings.netclass_assignments` entries.",
///  "type": "object",
///  "required": [
///    "classes",
///    "net_name"
///  ],
///  "properties": {
///    "classes": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "net_name": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoardNetClassAssignment {
    pub classes: ::std::vec::Vec<::std::string::String>,
    pub net_name: ::std::string::String,
}
///Resource-bounded board plotter operation. Source bytes are out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.board_plot.request:a0",
///  "title": "Board plot request a0",
///  "description": "Resource-bounded board plotter operation. Source bytes are out of band.",
///  "type": "object",
///  "required": [
///    "max_cache_contours",
///    "max_cache_polygons",
///    "max_depth",
///    "max_graphics",
///    "max_input_points",
///    "max_input_polygons",
///    "max_operations",
///    "max_output_bytes",
///    "max_parse_nodes",
///    "max_points",
///    "max_source_bytes",
///    "max_text_bytes",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "document_id": {
///      "type": "string"
///    },
///    "max_cache_contours": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_cache_polygons": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_graphics": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_input_points": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_input_polygons": {
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
///    "max_parse_nodes": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
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
///    "net_class_assignments": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/BoardNetClassAssignment"
///      }
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "text_variables": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/BoardTextVariable"
///      }
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.board_plot.request"
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
pub struct BoardPlotRequestA0 {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub document_id: ::std::option::Option<::std::string::String>,
    pub max_cache_contours: u32,
    pub max_cache_polygons: u32,
    pub max_depth: u32,
    pub max_graphics: u32,
    pub max_input_points: u32,
    pub max_input_polygons: u32,
    pub max_operations: u32,
    pub max_output_bytes: ::std::string::String,
    pub max_parse_nodes: u32,
    pub max_points: u32,
    pub max_source_bytes: ::std::string::String,
    pub max_text_bytes: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub net_class_assignments: ::std::vec::Vec<BoardNetClassAssignment>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub source_path: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub text_variables: ::std::vec::Vec<BoardTextVariable>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
/**One project-sidecar text variable. The producer case-expands names to
original/lower/upper aliases and overlays board `(property ...)` values,
matching the established `board_text_variables` merge.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One project-sidecar text variable. The producer case-expands names to\noriginal/lower/upper aliases and overlays board `(property ...)` values,\nmatching the established `board_text_variables` merge.",
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
pub struct BoardTextVariable {
    pub name: ::std::string::String,
    pub value: ::std::string::String,
}

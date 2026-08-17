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
///Resource-bounded schematic plot operation. Source bytes are out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.schematic_plot.request:a0",
///  "title": "Schematic plot request a0",
///  "description": "Resource-bounded schematic plot operation. Source bytes are out of band.",
///  "type": "object",
///  "required": [
///    "max_bus_entries",
///    "max_buses",
///    "max_depth",
///    "max_input_points",
///    "max_junctions",
///    "max_metadata_bytes",
///    "max_no_connects",
///    "max_operations",
///    "max_output_bytes",
///    "max_parse_nodes",
///    "max_points",
///    "max_records",
///    "max_selected_forms",
///    "max_source_bytes",
///    "max_text_bytes",
///    "max_text_variable_bytes",
///    "max_text_variables",
///    "max_wires",
///    "max_worksheet_bitmap_data_parts",
///    "max_worksheet_bitmap_decode_work",
///    "max_worksheet_bitmap_decoded_bytes",
///    "max_worksheet_bitmap_encoded_bytes",
///    "max_worksheet_bitmap_height_px",
///    "max_worksheet_bitmap_pixels",
///    "max_worksheet_bitmap_width_px",
///    "max_worksheet_bytes",
///    "max_worksheet_items",
///    "max_worksheet_point_sets",
///    "max_worksheet_points",
///    "max_worksheet_repeats",
///    "sheet_count",
///    "sheet_index",
///    "sheet_name",
///    "sheet_path",
///    "type",
///    "version",
///    "worksheet_mode"
///  ],
///  "properties": {
///    "document_id": {
///      "type": "string"
///    },
///    "max_bus_entries": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_buses": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_depth": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_input_points": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_junctions": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_metadata_bytes": {
///      "type": "string"
///    },
///    "max_no_connects": {
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
///    "max_records": {
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
///    "max_text_bytes": {
///      "type": "string"
///    },
///    "max_text_variable_bytes": {
///      "type": "string"
///    },
///    "max_text_variables": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_wires": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_bitmap_data_parts": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_bitmap_decode_work": {
///      "type": "string"
///    },
///    "max_worksheet_bitmap_decoded_bytes": {
///      "type": "string"
///    },
///    "max_worksheet_bitmap_encoded_bytes": {
///      "type": "string"
///    },
///    "max_worksheet_bitmap_height_px": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_bitmap_pixels": {
///      "type": "string"
///    },
///    "max_worksheet_bitmap_width_px": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_bytes": {
///      "type": "string"
///    },
///    "max_worksheet_items": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_point_sets": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_points": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_worksheet_repeats": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "sheet_count": {
///      "$ref": "#/$defs/SchematicPositiveUint32"
///    },
///    "sheet_index": {
///      "$ref": "#/$defs/SchematicPositiveUint32"
///    },
///    "sheet_name": {
///      "type": "string"
///    },
///    "sheet_path": {
///      "type": "string"
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "text_variables": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/SchematicTextVariable"
///      }
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.schematic_plot.request"
///    },
///    "version": {
///      "type": "string",
///      "const": "a0"
///    },
///    "worksheet_mode": {
///      "$ref": "#/$defs/SchematicWorksheetMode"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SchematicPlotRequestA0 {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub document_id: ::std::option::Option<::std::string::String>,
    pub max_bus_entries: u32,
    pub max_buses: u32,
    pub max_depth: u32,
    pub max_input_points: u32,
    pub max_junctions: u32,
    pub max_metadata_bytes: ::std::string::String,
    pub max_no_connects: u32,
    pub max_operations: u32,
    pub max_output_bytes: ::std::string::String,
    pub max_parse_nodes: u32,
    pub max_points: u32,
    pub max_records: u32,
    pub max_selected_forms: u32,
    pub max_source_bytes: ::std::string::String,
    pub max_text_bytes: ::std::string::String,
    pub max_text_variable_bytes: ::std::string::String,
    pub max_text_variables: u32,
    pub max_wires: u32,
    pub max_worksheet_bitmap_data_parts: u32,
    pub max_worksheet_bitmap_decode_work: ::std::string::String,
    pub max_worksheet_bitmap_decoded_bytes: ::std::string::String,
    pub max_worksheet_bitmap_encoded_bytes: ::std::string::String,
    pub max_worksheet_bitmap_height_px: u32,
    pub max_worksheet_bitmap_pixels: ::std::string::String,
    pub max_worksheet_bitmap_width_px: u32,
    pub max_worksheet_bytes: ::std::string::String,
    pub max_worksheet_items: u32,
    pub max_worksheet_point_sets: u32,
    pub max_worksheet_points: u32,
    pub max_worksheet_repeats: u32,
    pub sheet_count: ::std::num::NonZeroU32,
    pub sheet_index: ::std::num::NonZeroU32,
    pub sheet_name: ::std::string::String,
    pub sheet_path: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_path: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub text_variables: ::std::vec::Vec<SchematicTextVariable>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
    pub worksheet_mode: SchematicWorksheetMode,
}
///One exact-case project text variable supplied by the caller.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One exact-case project text variable supplied by the caller.",
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
pub struct SchematicTextVariable {
    pub name: ::std::string::String,
    pub value: ::std::string::String,
}
///Selection of the drawing-sheet byte sidecar supplied out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Selection of the drawing-sheet byte sidecar supplied out of band.",
///  "type": "string",
///  "enum": [
///    "default",
///    "provided"
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
pub enum SchematicWorksheetMode {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "provided")]
    Provided,
}
impl ::std::fmt::Display for SchematicWorksheetMode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Default => f.write_str("default"),
            Self::Provided => f.write_str("provided"),
        }
    }
}
impl ::std::str::FromStr for SchematicWorksheetMode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "default" => Ok(Self::Default),
            "provided" => Ok(Self::Provided),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SchematicWorksheetMode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SchematicWorksheetMode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SchematicWorksheetMode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}

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
///Strict subset of kicad.plotter_ir.a0 emitted by the initial footprint slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.footprint_plot.document:a0",
///  "title": "Footprint plot document a0",
///  "description": "Strict subset of kicad.plotter_ir.a0 emitted by the initial footprint slice.",
///  "type": "object",
///  "required": [
///    "coordinate_space",
///    "document_id",
///    "generator",
///    "generator_version",
///    "records",
///    "schema",
///    "source_kind",
///    "total_operations",
///    "version"
///  ],
///  "properties": {
///    "coordinate_space": {
///      "$ref": "#/$defs/PlotterCoordinateSpace"
///    },
///    "document_id": {
///      "type": "string"
///    },
///    "generator": {
///      "type": "string"
///    },
///    "generator_version": {
///      "type": "string"
///    },
///    "records": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/FootprintPlotRecord"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad.plotter_ir.a0"
///    },
///    "source_kind": {
///      "type": "string",
///      "const": "MOD"
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "total_operations": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "version": {
///      "type": "integer"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FootprintPlotDocumentA0 {
    pub coordinate_space: PlotterCoordinateSpace,
    pub document_id: ::std::string::String,
    pub generator: ::std::string::String,
    pub generator_version: ::std::string::String,
    pub records: ::std::vec::Vec<FootprintPlotRecord>,
    pub schema: ::std::string::String,
    pub source_kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_path: ::std::option::Option<::std::string::String>,
    pub total_operations: u32,
    pub version: i64,
}
///One footprint record in the first typed plotter slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One footprint record in the first typed plotter slice.",
///  "type": "object",
///  "required": [
///    "attr",
///    "descr",
///    "kind",
///    "layer",
///    "locked",
///    "name",
///    "object_id",
///    "operation_count",
///    "operations",
///    "placed",
///    "tags",
///    "uuid"
///  ],
///  "properties": {
///    "attr": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "descr": {
///      "type": "string"
///    },
///    "kind": {
///      "type": "string",
///      "const": "footprint"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "locked": {
///      "type": "boolean"
///    },
///    "name": {
///      "type": "string"
///    },
///    "object_id": {
///      "type": "string"
///    },
///    "operation_count": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "operations": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/ThickSegmentOperation"
///      }
///    },
///    "placed": {
///      "type": "boolean"
///    },
///    "tags": {
///      "type": "string"
///    },
///    "uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FootprintPlotRecord {
    pub attr: ::std::vec::Vec<::std::string::String>,
    pub descr: ::std::string::String,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub locked: bool,
    pub name: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<ThickSegmentOperation>,
    pub placed: bool,
    pub tags: ::std::string::String,
    pub uuid: ::std::string::String,
}
///Coordinate convention for the initial footprint plotter slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Coordinate convention for the initial footprint plotter slice.",
///  "type": "object",
///  "required": [
///    "unit",
///    "y_axis"
///  ],
///  "properties": {
///    "unit": {
///      "type": "string",
///      "const": "nm"
///    },
///    "y_axis": {
///      "type": "string",
///      "const": "down"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotterCoordinateSpace {
    pub unit: ::std::string::String,
    pub y_axis: ::std::string::String,
}
///Solid footprint line operation supported by the first typed plotter slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Solid footprint line operation supported by the first typed plotter slice.",
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "layer",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "end_x": {
///      "type": "integer"
///    },
///    "end_y": {
///      "type": "integer"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "ThickSegment"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "start_x": {
///      "type": "integer"
///    },
///    "start_y": {
///      "type": "integer"
///    },
///    "width_nm": {
///      "type": "integer"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ThickSegmentOperation {
    pub end_x: i64,
    pub end_y: i64,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub start_x: i64,
    pub start_y: i64,
    pub width_nm: i64,
}

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
///Solid three-point arc.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Solid three-point arc.",
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "fill",
///    "index",
///    "kind",
///    "layer",
///    "mid_x",
///    "mid_y",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "ArcThreePoint"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "mid_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "mid_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ArcThreePointOperation {
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    pub fill: PlotterFill,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub mid_x: crate::JavaScriptSafeInteger,
    pub mid_y: crate::JavaScriptSafeInteger,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    pub width_nm: crate::JavaScriptSafeInteger,
}
/**Circle shared by graphical and drill producers. Graphic state requires only
layer. Drill state requires role plus layers; NPTH state additionally
requires all mask and pad-size hints. The generated semantic validator
enforces these mutually exclusive states.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Circle shared by graphical and drill producers. Graphic state requires only\nlayer. Drill state requires role plus layers; NPTH state additionally\nrequires all mask and pad-size hints. The generated semantic validator\nenforces these mutually exclusive states.",
///  "type": "object",
///  "required": [
///    "cx",
///    "cy",
///    "diameter_nm",
///    "fill",
///    "index",
///    "kind",
///    "width_nm"
///  ],
///  "properties": {
///    "cx": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "cy": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "diameter_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "Circle"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterDrillRole"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CircleOperation {
    pub cx: crate::JavaScriptSafeInteger,
    pub cy: crate::JavaScriptSafeInteger,
    pub diameter_nm: crate::JavaScriptSafeInteger,
    pub fill: PlotterFill,
    pub index: u32,
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub role: ::std::option::Option<PlotterDrillRole>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Circular pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Circular pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "diameter_nm",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "diameter_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadCircle"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadCircleOperation {
    pub diameter_nm: crate::JavaScriptSafeInteger,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
/**Custom pad flash shared by footprint and PCB producers. Polygon coordinates
are pad-local. A non-empty polygon_widths_nm has one entry per polygon;
generated semantic validation enforces that relationship. An empty array is
equivalent to omission for generated Rust transport bindings.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Custom pad flash shared by footprint and PCB producers. Polygon coordinates\nare pad-local. A non-empty polygon_widths_nm has one entry per polygon;\ngenerated semantic validation enforces that relationship. An empty array is\nequivalent to omission for generated Rust transport bindings.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "polygons",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "anchor_shape": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadCustom"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "polygon_widths_nm": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/JavaScriptSafeInteger"
///      }
///    },
///    "polygons": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadCustomOperation {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub anchor_shape: ::std::option::Option<::std::string::String>,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub polygon_widths_nm: ::std::vec::Vec<crate::JavaScriptSafeInteger>,
    pub polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Oval pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Oval pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadOval"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadOvalOperation {
    pub index: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Rectangular pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Rectangular pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadRect"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadRectOperation {
    pub index: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Rounded-rectangle pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Rounded-rectangle pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "corner_radius_nm",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "size_x_nm",
///    "size_y_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "corner_radius_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadRoundRect"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadRoundRectOperation {
    pub corner_radius_nm: crate::JavaScriptSafeInteger,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Trapezoid pad flash shared by footprint and PCB producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Trapezoid pad flash shared by footprint and PCB producers.",
///  "type": "object",
///  "required": [
///    "corners",
///    "index",
///    "kind",
///    "layers",
///    "mask_margin_nm",
///    "orient_deg",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "corners": {
///      "$ref": "#/$defs/PlotterQuad"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "FlashPadTrapez"
///    },
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashPadTrapezOperation {
    pub corners: PlotterQuad,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
/**Strict non-text footprint subset of kicad.plotter_ir.a0. Producers and
consumers must run generated semantic validation after structural decoding.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.footprint_plot.document:a0",
///  "title": "Footprint plot document a0",
///  "description": "Strict non-text footprint subset of kicad.plotter_ir.a0. Producers and\nconsumers must run generated semantic validation after structural decoding.",
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
///      "$ref": "#/$defs/JavaScriptSafeInteger"
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
    pub version: crate::JavaScriptSafeInteger,
}
///One footprint record in the promoted non-text graphics slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One footprint record in the promoted non-text graphics slice.",
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
///        "$ref": "#/$defs/PlotterOperation"
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
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub placed: bool,
    pub tags: ::std::string::String,
    pub uuid: ::std::string::String,
}
///Filled or outlined polygon operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Filled or outlined polygon operation.",
///  "type": "object",
///  "required": [
///    "fill",
///    "index",
///    "kind",
///    "layer",
///    "points",
///    "width_nm"
///  ],
///  "properties": {
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "PlotPoly"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "points": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterPoint"
///      }
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotPolyOperation {
    pub fill: PlotterFill,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub points: ::std::vec::Vec<PlotterPoint>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Coordinate convention for the footprint plotter slice.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Coordinate convention for the footprint plotter slice.",
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
///Semantic roles allowed on shared circle and segment drill operations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Semantic roles allowed on shared circle and segment drill operations.",
///  "type": "string",
///  "enum": [
///    "pad_drill",
///    "npth_hole"
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
pub enum PlotterDrillRole {
    #[serde(rename = "pad_drill")]
    PadDrill,
    #[serde(rename = "npth_hole")]
    NpthHole,
}
impl ::std::fmt::Display for PlotterDrillRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::PadDrill => f.write_str("pad_drill"),
            Self::NpthHole => f.write_str("npth_hole"),
        }
    }
}
impl ::std::str::FromStr for PlotterDrillRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pad_drill" => Ok(Self::PadDrill),
            "npth_hole" => Ok(Self::NpthHole),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterDrillRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterDrillRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterDrillRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Fill values shared by plotter operation producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Fill values shared by plotter operation producers.",
///  "type": "string",
///  "enum": [
///    "NO_FILL",
///    "FILLED_SHAPE"
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
pub enum PlotterFill {
    #[serde(rename = "NO_FILL")]
    NoFill,
    #[serde(rename = "FILLED_SHAPE")]
    FilledShape,
}
impl ::std::fmt::Display for PlotterFill {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NoFill => f.write_str("NO_FILL"),
            Self::FilledShape => f.write_str("FILLED_SHAPE"),
        }
    }
}
impl ::std::str::FromStr for PlotterFill {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "NO_FILL" => Ok(Self::NoFill),
            "FILLED_SHAPE" => Ok(Self::FilledShape),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterFill {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterFill {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterFill {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Shared plotter operation vocabulary promoted across source producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Shared plotter operation vocabulary promoted across source producers.",
///  "anyOf": [
///    {
///      "$ref": "#/$defs/ThickSegmentOperation"
///    },
///    {
///      "$ref": "#/$defs/ArcThreePointOperation"
///    },
///    {
///      "$ref": "#/$defs/CircleOperation"
///    },
///    {
///      "$ref": "#/$defs/RectOperation"
///    },
///    {
///      "$ref": "#/$defs/PlotPolyOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadCircleOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadOvalOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadRectOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadRoundRectOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadCustomOperation"
///    },
///    {
///      "$ref": "#/$defs/FlashPadTrapezOperation"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum PlotterOperation {
    ThickSegmentOperation(ThickSegmentOperation),
    ArcThreePointOperation(ArcThreePointOperation),
    CircleOperation(CircleOperation),
    RectOperation(RectOperation),
    PlotPolyOperation(PlotPolyOperation),
    FlashPadCircleOperation(FlashPadCircleOperation),
    FlashPadOvalOperation(FlashPadOvalOperation),
    FlashPadRectOperation(FlashPadRectOperation),
    FlashPadRoundRectOperation(FlashPadRoundRectOperation),
    FlashPadCustomOperation(FlashPadCustomOperation),
    FlashPadTrapezOperation(FlashPadTrapezOperation),
}
impl ::std::convert::From<ThickSegmentOperation> for PlotterOperation {
    fn from(value: ThickSegmentOperation) -> Self {
        Self::ThickSegmentOperation(value)
    }
}
impl ::std::convert::From<ArcThreePointOperation> for PlotterOperation {
    fn from(value: ArcThreePointOperation) -> Self {
        Self::ArcThreePointOperation(value)
    }
}
impl ::std::convert::From<CircleOperation> for PlotterOperation {
    fn from(value: CircleOperation) -> Self {
        Self::CircleOperation(value)
    }
}
impl ::std::convert::From<RectOperation> for PlotterOperation {
    fn from(value: RectOperation) -> Self {
        Self::RectOperation(value)
    }
}
impl ::std::convert::From<PlotPolyOperation> for PlotterOperation {
    fn from(value: PlotPolyOperation) -> Self {
        Self::PlotPolyOperation(value)
    }
}
impl ::std::convert::From<FlashPadCircleOperation> for PlotterOperation {
    fn from(value: FlashPadCircleOperation) -> Self {
        Self::FlashPadCircleOperation(value)
    }
}
impl ::std::convert::From<FlashPadOvalOperation> for PlotterOperation {
    fn from(value: FlashPadOvalOperation) -> Self {
        Self::FlashPadOvalOperation(value)
    }
}
impl ::std::convert::From<FlashPadRectOperation> for PlotterOperation {
    fn from(value: FlashPadRectOperation) -> Self {
        Self::FlashPadRectOperation(value)
    }
}
impl ::std::convert::From<FlashPadRoundRectOperation> for PlotterOperation {
    fn from(value: FlashPadRoundRectOperation) -> Self {
        Self::FlashPadRoundRectOperation(value)
    }
}
impl ::std::convert::From<FlashPadCustomOperation> for PlotterOperation {
    fn from(value: FlashPadCustomOperation) -> Self {
        Self::FlashPadCustomOperation(value)
    }
}
impl ::std::convert::From<FlashPadTrapezOperation> for PlotterOperation {
    fn from(value: FlashPadTrapezOperation) -> Self {
        Self::FlashPadTrapezOperation(value)
    }
}
///Plotter point encoded as an exact coordinate pair.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Plotter point encoded as an exact coordinate pair.",
///  "type": "array",
///  "items": {
///    "$ref": "#/$defs/JavaScriptSafeInteger"
///  },
///  "maxItems": 2,
///  "minItems": 2
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlotterPoint(pub [crate::JavaScriptSafeInteger; 2usize]);
impl ::std::ops::Deref for PlotterPoint {
    type Target = [crate::JavaScriptSafeInteger; 2usize];
    fn deref(&self) -> &[crate::JavaScriptSafeInteger; 2usize] {
        &self.0
    }
}
impl ::std::convert::From<PlotterPoint> for [crate::JavaScriptSafeInteger; 2usize] {
    fn from(value: PlotterPoint) -> Self {
        value.0
    }
}
impl ::std::convert::From<[crate::JavaScriptSafeInteger; 2usize]> for PlotterPoint {
    fn from(value: [crate::JavaScriptSafeInteger; 2usize]) -> Self {
        Self(value)
    }
}
///Four pad-local trapezoid corners.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Four pad-local trapezoid corners.",
///  "type": "array",
///  "items": {
///    "$ref": "#/$defs/PlotterPoint"
///  },
///  "maxItems": 4,
///  "minItems": 4
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PlotterQuad(pub [PlotterPoint; 4usize]);
impl ::std::ops::Deref for PlotterQuad {
    type Target = [PlotterPoint; 4usize];
    fn deref(&self) -> &[PlotterPoint; 4usize] {
        &self.0
    }
}
impl ::std::convert::From<PlotterQuad> for [PlotterPoint; 4usize] {
    fn from(value: PlotterQuad) -> Self {
        value.0
    }
}
impl ::std::convert::From<[PlotterPoint; 4usize]> for PlotterQuad {
    fn from(value: [PlotterPoint; 4usize]) -> Self {
        Self(value)
    }
}
///Rectangle with square corners.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Rectangle with square corners.",
///  "type": "object",
///  "required": [
///    "corner_radius_nm",
///    "fill",
///    "index",
///    "kind",
///    "layer",
///    "width_nm",
///    "x1",
///    "x2",
///    "y1",
///    "y2"
///  ],
///  "properties": {
///    "corner_radius_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "Rect"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x1": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "x2": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y1": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "y2": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RectOperation {
    pub corner_radius_nm: crate::JavaScriptSafeInteger,
    pub fill: PlotterFill,
    pub index: u32,
    pub kind: ::std::string::String,
    pub layer: ::std::string::String,
    pub width_nm: crate::JavaScriptSafeInteger,
    pub x1: crate::JavaScriptSafeInteger,
    pub x2: crate::JavaScriptSafeInteger,
    pub y1: crate::JavaScriptSafeInteger,
    pub y2: crate::JavaScriptSafeInteger,
}
/**Solid or decomposed segment shared by PCB, footprint, and drill producers.
Graphic state requires only layer. Drill state requires role plus layers;
NPTH drill state additionally requires all mask and pad-size hints. The
generated semantic validator enforces these mutually exclusive states.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Solid or decomposed segment shared by PCB, footprint, and drill producers.\nGraphic state requires only layer. Drill state requires role plus layers;\nNPTH drill state additionally requires all mask and pad-size hints. The\ngenerated semantic validator enforces these mutually exclusive states.",
///  "type": "object",
///  "required": [
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "start_x",
///    "start_y",
///    "width_nm"
///  ],
///  "properties": {
///    "end_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "end_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
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
///    "layers": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "mask_margin_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "pad_size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "role": {
///      "$ref": "#/$defs/PlotterDrillRole"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ThickSegmentOperation {
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    pub index: u32,
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub role: ::std::option::Option<PlotterDrillRole>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    pub width_nm: crate::JavaScriptSafeInteger,
}

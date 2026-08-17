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
///    "fill_color": {
///      "type": "string"
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
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
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
///    "stroke_color": {
///      "type": "string"
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
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_arc_three_point_kind")]
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub mid_x: crate::JavaScriptSafeInteger,
    pub mid_y: crate::JavaScriptSafeInteger,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Cubic Bézier shared by symbol and schematic producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Cubic Bézier shared by symbol and schematic producers.",
///  "type": "object",
///  "required": [
///    "ctrl1_x",
///    "ctrl1_y",
///    "ctrl2_x",
///    "ctrl2_y",
///    "end_x",
///    "end_y",
///    "index",
///    "kind",
///    "start_x",
///    "start_y",
///    "tolerance_nm",
///    "width_nm"
///  ],
///  "properties": {
///    "ctrl1_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl1_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl2_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "ctrl2_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
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
///      "const": "BezierCurve"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "start_x": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "start_y": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "tolerance_nm": {
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
pub struct BezierCurveOperation {
    pub ctrl1_x: crate::JavaScriptSafeInteger,
    pub ctrl1_y: crate::JavaScriptSafeInteger,
    pub ctrl2_x: crate::JavaScriptSafeInteger,
    pub ctrl2_y: crate::JavaScriptSafeInteger,
    pub end_x: crate::JavaScriptSafeInteger,
    pub end_y: crate::JavaScriptSafeInteger,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_bezier_curve_kind")]
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub start_x: crate::JavaScriptSafeInteger,
    pub start_y: crate::JavaScriptSafeInteger,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub tolerance_nm: crate::JavaScriptSafeInteger,
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
///    "fill_color": {
///      "type": "string"
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
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
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
///    "stroke_color": {
///      "type": "string"
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
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_circle_kind")]
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub pad_size_x_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub pad_size_y_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub role: ::std::option::Option<PlotterDrillRole>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}
/**Circular pad flash shared by footprint and PCB producers. Footprint pad
state requires mask_margin_nm and forbids role. Board via state requires
role and forbids mask_margin_nm. The generated semantic validator enforces
these mutually exclusive states.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Circular pad flash shared by footprint and PCB producers. Footprint pad\nstate requires mask_margin_nm and forbids role. Board via state requires\nrole and forbids mask_margin_nm. The generated semantic validator enforces\nthese mutually exclusive states.",
///  "type": "object",
///  "required": [
///    "diameter_nm",
///    "index",
///    "kind",
///    "layers",
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
///    "role": {
///      "$ref": "#/$defs/PlotterViaFlashRole"
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
    #[serde(deserialize_with = "crate::deserialize_flash_pad_circle_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mask_margin_nm: ::std::option::Option<crate::JavaScriptSafeInteger>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub role: ::std::option::Option<PlotterViaFlashRole>,
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
    #[serde(deserialize_with = "crate::deserialize_flash_pad_custom_kind")]
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
    #[serde(deserialize_with = "crate::deserialize_flash_pad_oval_kind")]
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
    #[serde(deserialize_with = "crate::deserialize_flash_pad_rect_kind")]
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
    #[serde(deserialize_with = "crate::deserialize_flash_pad_round_rect_kind")]
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
    #[serde(deserialize_with = "crate::deserialize_flash_pad_trapez_kind")]
    pub kind: ::std::string::String,
    pub layers: ::std::vec::Vec<::std::string::String>,
    pub mask_margin_nm: crate::JavaScriptSafeInteger,
    pub orient_deg: f64,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
///Decoded image placement shared by worksheet and schematic producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Decoded image placement shared by worksheet and schematic producers.",
///  "type": "object",
///  "required": [
///    "height_nm",
///    "image_data_b64",
///    "image_format",
///    "index",
///    "kind",
///    "scale",
///    "width_nm",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "height_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "image_data_b64": {
///      "type": "string"
///    },
///    "image_format": {
///      "type": "string"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "kind": {
///      "type": "string",
///      "const": "PlotImage"
///    },
///    "scale": {
///      "type": "number"
///    },
///    "stroke_color": {
///      "type": "string"
///    },
///    "width_nm": {
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
pub struct PlotImageOperation {
    pub height_nm: crate::JavaScriptSafeInteger,
    pub image_data_b64: ::std::string::String,
    pub image_format: ::std::string::String,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_plot_image_kind")]
    pub kind: ::std::string::String,
    pub scale: f64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
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
///    "points",
///    "width_nm"
///  ],
///  "properties": {
///    "fill": {
///      "$ref": "#/$defs/PlotterFill"
///    },
///    "fill_color": {
///      "type": "string"
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
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "points": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PlotterPoint"
///      }
///    },
///    "stroke_color": {
///      "type": "string"
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
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_plot_poly_kind")]
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    pub points: ::std::vec::Vec<PlotterPoint>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
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
///    "npth_hole",
///    "via_drill",
///    "via_mask_drill"
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
    #[serde(rename = "via_drill")]
    ViaDrill,
    #[serde(rename = "via_mask_drill")]
    ViaMaskDrill,
}
impl ::std::fmt::Display for PlotterDrillRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::PadDrill => f.write_str("pad_drill"),
            Self::NpthHole => f.write_str("npth_hole"),
            Self::ViaDrill => f.write_str("via_drill"),
            Self::ViaMaskDrill => f.write_str("via_mask_drill"),
        }
    }
}
impl ::std::str::FromStr for PlotterDrillRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pad_drill" => Ok(Self::PadDrill),
            "npth_hole" => Ok(Self::NpthHole),
            "via_drill" => Ok(Self::ViaDrill),
            "via_mask_drill" => Ok(Self::ViaMaskDrill),
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
///    "FILLED_SHAPE",
///    "FILLED_WITH_BG_BODYCOLOR",
///    "FILLED_WITH_COLOR",
///    "HATCH",
///    "REVERSE_HATCH",
///    "CROSS_HATCH"
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
    #[serde(rename = "FILLED_WITH_BG_BODYCOLOR")]
    FilledWithBgBodycolor,
    #[serde(rename = "FILLED_WITH_COLOR")]
    FilledWithColor,
    #[serde(rename = "HATCH")]
    Hatch,
    #[serde(rename = "REVERSE_HATCH")]
    ReverseHatch,
    #[serde(rename = "CROSS_HATCH")]
    CrossHatch,
}
impl ::std::fmt::Display for PlotterFill {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NoFill => f.write_str("NO_FILL"),
            Self::FilledShape => f.write_str("FILLED_SHAPE"),
            Self::FilledWithBgBodycolor => f.write_str("FILLED_WITH_BG_BODYCOLOR"),
            Self::FilledWithColor => f.write_str("FILLED_WITH_COLOR"),
            Self::Hatch => f.write_str("HATCH"),
            Self::ReverseHatch => f.write_str("REVERSE_HATCH"),
            Self::CrossHatch => f.write_str("CROSS_HATCH"),
        }
    }
}
impl ::std::str::FromStr for PlotterFill {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "NO_FILL" => Ok(Self::NoFill),
            "FILLED_SHAPE" => Ok(Self::FilledShape),
            "FILLED_WITH_BG_BODYCOLOR" => Ok(Self::FilledWithBgBodycolor),
            "FILLED_WITH_COLOR" => Ok(Self::FilledWithColor),
            "HATCH" => Ok(Self::Hatch),
            "REVERSE_HATCH" => Ok(Self::ReverseHatch),
            "CROSS_HATCH" => Ok(Self::CrossHatch),
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
///One exact hyperlink attached to an authored plotter text carrier.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One exact hyperlink attached to an authored plotter text carrier.",
///  "type": "object",
///  "required": [
///    "href"
///  ],
///  "properties": {
///    "href": {
///      "type": "string",
///      "minLength": 1
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotterHyperlink {
    pub href: PlotterHyperlinkHref,
}
///`PlotterHyperlinkHref`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "minLength": 1
///}
/// ```
/// </details>
#[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct PlotterHyperlinkHref(::std::string::String);
impl ::std::ops::Deref for PlotterHyperlinkHref {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<PlotterHyperlinkHref> for ::std::string::String {
    fn from(value: PlotterHyperlinkHref) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for PlotterHyperlinkHref {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() < 1usize {
            return Err("shorter than 1 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for PlotterHyperlinkHref {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterHyperlinkHref {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterHyperlinkHref {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for PlotterHyperlinkHref {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
///KiCad stroke styles carried without producer-specific decomposition.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "KiCad stroke styles carried without producer-specific decomposition.",
///  "type": "string",
///  "enum": [
///    "DEFAULT",
///    "SOLID",
///    "DASH",
///    "DOT",
///    "DASH_DOT",
///    "DASH_DOT_DOT"
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
pub enum PlotterLineStyle {
    #[serde(rename = "DEFAULT")]
    Default,
    #[serde(rename = "SOLID")]
    Solid,
    #[serde(rename = "DASH")]
    Dash,
    #[serde(rename = "DOT")]
    Dot,
    #[serde(rename = "DASH_DOT")]
    DashDot,
    #[serde(rename = "DASH_DOT_DOT")]
    DashDotDot,
}
impl ::std::fmt::Display for PlotterLineStyle {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Default => f.write_str("DEFAULT"),
            Self::Solid => f.write_str("SOLID"),
            Self::Dash => f.write_str("DASH"),
            Self::Dot => f.write_str("DOT"),
            Self::DashDot => f.write_str("DASH_DOT"),
            Self::DashDotDot => f.write_str("DASH_DOT_DOT"),
        }
    }
}
impl ::std::str::FromStr for PlotterLineStyle {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "DEFAULT" => Ok(Self::Default),
            "SOLID" => Ok(Self::Solid),
            "DASH" => Ok(Self::Dash),
            "DOT" => Ok(Self::Dot),
            "DASH_DOT" => Ok(Self::DashDot),
            "DASH_DOT_DOT" => Ok(Self::DashDotDot),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterLineStyle {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterLineStyle {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterLineStyle {
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
///      "$ref": "#/$defs/BezierCurveOperation"
///    },
///    {
///      "$ref": "#/$defs/TextOperation"
///    },
///    {
///      "$ref": "#/$defs/PlotImageOperation"
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
    BezierCurveOperation(BezierCurveOperation),
    TextOperation(TextOperation),
    PlotImageOperation(PlotImageOperation),
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
impl ::std::convert::From<BezierCurveOperation> for PlotterOperation {
    fn from(value: BezierCurveOperation) -> Self {
        Self::BezierCurveOperation(value)
    }
}
impl ::std::convert::From<TextOperation> for PlotterOperation {
    fn from(value: TextOperation) -> Self {
        Self::TextOperation(value)
    }
}
impl ::std::convert::From<PlotImageOperation> for PlotterOperation {
    fn from(value: PlotImageOperation) -> Self {
        Self::PlotImageOperation(value)
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
///Strict operation-local context emitted by current plotter producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Strict operation-local context emitted by current plotter producers.",
///  "type": "object",
///  "required": [
///    "hyperlink"
///  ],
///  "properties": {
///    "hyperlink": {
///      "$ref": "#/$defs/PlotterHyperlink"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlotterOperationContext {
    pub hyperlink: PlotterHyperlink,
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
///Horizontal text alignments emitted by the board producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Horizontal text alignments emitted by the board producers.",
///  "type": "string",
///  "enum": [
///    "GR_TEXT_H_ALIGN_LEFT",
///    "GR_TEXT_H_ALIGN_CENTER",
///    "GR_TEXT_H_ALIGN_RIGHT"
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
pub enum PlotterTextHAlign {
    #[serde(rename = "GR_TEXT_H_ALIGN_LEFT")]
    GrTextHAlignLeft,
    #[serde(rename = "GR_TEXT_H_ALIGN_CENTER")]
    GrTextHAlignCenter,
    #[serde(rename = "GR_TEXT_H_ALIGN_RIGHT")]
    GrTextHAlignRight,
}
impl ::std::fmt::Display for PlotterTextHAlign {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GrTextHAlignLeft => f.write_str("GR_TEXT_H_ALIGN_LEFT"),
            Self::GrTextHAlignCenter => f.write_str("GR_TEXT_H_ALIGN_CENTER"),
            Self::GrTextHAlignRight => f.write_str("GR_TEXT_H_ALIGN_RIGHT"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextHAlign {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "GR_TEXT_H_ALIGN_LEFT" => Ok(Self::GrTextHAlignLeft),
            "GR_TEXT_H_ALIGN_CENTER" => Ok(Self::GrTextHAlignCenter),
            "GR_TEXT_H_ALIGN_RIGHT" => Ok(Self::GrTextHAlignRight),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextHAlign {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextHAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextHAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Coordinate space carried by one typed text render cache.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Coordinate space carried by one typed text render cache.",
///  "type": "string",
///  "enum": [
///    "board",
///    "footprint_local"
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
pub enum PlotterTextRenderCacheCoordinateSpace {
    #[serde(rename = "board")]
    Board,
    #[serde(rename = "footprint_local")]
    FootprintLocal,
}
impl ::std::fmt::Display for PlotterTextRenderCacheCoordinateSpace {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Board => f.write_str("board"),
            Self::FootprintLocal => f.write_str("footprint_local"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextRenderCacheCoordinateSpace {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "board" => Ok(Self::Board),
            "footprint_local" => Ok(Self::FootprintLocal),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextRenderCacheCoordinateSpace {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextRenderCacheCoordinateSpace {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextRenderCacheCoordinateSpace {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Provenance of one attached text render cache.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Provenance of one attached text render cache.",
///  "type": "string",
///  "enum": [
///    "existing_file_cache",
///    "python_generated_cache",
///    "native_generated_cache"
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
pub enum PlotterTextRenderCacheSource {
    #[serde(rename = "existing_file_cache")]
    ExistingFileCache,
    #[serde(rename = "python_generated_cache")]
    PythonGeneratedCache,
    #[serde(rename = "native_generated_cache")]
    NativeGeneratedCache,
}
impl ::std::fmt::Display for PlotterTextRenderCacheSource {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ExistingFileCache => f.write_str("existing_file_cache"),
            Self::PythonGeneratedCache => f.write_str("python_generated_cache"),
            Self::NativeGeneratedCache => f.write_str("native_generated_cache"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextRenderCacheSource {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "existing_file_cache" => Ok(Self::ExistingFileCache),
            "python_generated_cache" => Ok(Self::PythonGeneratedCache),
            "native_generated_cache" => Ok(Self::NativeGeneratedCache),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextRenderCacheSource {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextRenderCacheSource {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextRenderCacheSource {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Vertical text alignments emitted by the board producers.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Vertical text alignments emitted by the board producers.",
///  "type": "string",
///  "enum": [
///    "GR_TEXT_V_ALIGN_TOP",
///    "GR_TEXT_V_ALIGN_CENTER",
///    "GR_TEXT_V_ALIGN_BOTTOM"
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
pub enum PlotterTextVAlign {
    #[serde(rename = "GR_TEXT_V_ALIGN_TOP")]
    GrTextVAlignTop,
    #[serde(rename = "GR_TEXT_V_ALIGN_CENTER")]
    GrTextVAlignCenter,
    #[serde(rename = "GR_TEXT_V_ALIGN_BOTTOM")]
    GrTextVAlignBottom,
}
impl ::std::fmt::Display for PlotterTextVAlign {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GrTextVAlignTop => f.write_str("GR_TEXT_V_ALIGN_TOP"),
            Self::GrTextVAlignCenter => f.write_str("GR_TEXT_V_ALIGN_CENTER"),
            Self::GrTextVAlignBottom => f.write_str("GR_TEXT_V_ALIGN_BOTTOM"),
        }
    }
}
impl ::std::str::FromStr for PlotterTextVAlign {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "GR_TEXT_V_ALIGN_TOP" => Ok(Self::GrTextVAlignTop),
            "GR_TEXT_V_ALIGN_CENTER" => Ok(Self::GrTextVAlignCenter),
            "GR_TEXT_V_ALIGN_BOTTOM" => Ok(Self::GrTextVAlignBottom),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterTextVAlign {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterTextVAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterTextVAlign {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Semantic roles allowed on board via flash operations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Semantic roles allowed on board via flash operations.",
///  "type": "string",
///  "enum": [
///    "via_aperture",
///    "via_mask_opening"
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
pub enum PlotterViaFlashRole {
    #[serde(rename = "via_aperture")]
    ViaAperture,
    #[serde(rename = "via_mask_opening")]
    ViaMaskOpening,
}
impl ::std::fmt::Display for PlotterViaFlashRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ViaAperture => f.write_str("via_aperture"),
            Self::ViaMaskOpening => f.write_str("via_mask_opening"),
        }
    }
}
impl ::std::str::FromStr for PlotterViaFlashRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "via_aperture" => Ok(Self::ViaAperture),
            "via_mask_opening" => Ok(Self::ViaMaskOpening),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PlotterViaFlashRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PlotterViaFlashRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PlotterViaFlashRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`RecordString`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "additionalProperties": {
///    "type": "string"
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct RecordString(
    pub ::std::collections::BTreeMap<::std::string::String, ::std::string::String>,
);
impl ::std::ops::Deref for RecordString {
    type Target = ::std::collections::BTreeMap<::std::string::String, ::std::string::String>;
    fn deref(&self) -> &::std::collections::BTreeMap<::std::string::String, ::std::string::String> {
        &self.0
    }
}
impl ::std::convert::From<RecordString>
    for ::std::collections::BTreeMap<::std::string::String, ::std::string::String>
{
    fn from(value: RecordString) -> Self {
        value.0
    }
}
impl
    ::std::convert::From<::std::collections::BTreeMap<::std::string::String, ::std::string::String>>
    for RecordString
{
    fn from(
        value: ::std::collections::BTreeMap<::std::string::String, ::std::string::String>,
    ) -> Self {
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
///    "fill_color": {
///      "type": "string"
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
///    "line_style": {
///      "$ref": "#/$defs/PlotterLineStyle"
///    },
///    "stroke_color": {
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
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub fill_color: ::std::option::Option<::std::string::String>,
    pub index: u32,
    #[serde(deserialize_with = "crate::deserialize_rect_kind")]
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line_style: ::std::option::Option<PlotterLineStyle>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
    pub x1: crate::JavaScriptSafeInteger,
    pub x2: crate::JavaScriptSafeInteger,
    pub y1: crate::JavaScriptSafeInteger,
    pub y2: crate::JavaScriptSafeInteger,
}
///One schematic bus-entry segment.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One schematic bus-entry segment.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "bus_entry"
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
pub struct SchematicBusEntryPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_bus_entry_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
///One schematic bus polyline.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One schematic bus polyline.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "bus"
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
pub struct SchematicBusPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_bus_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
///One global schematic label and its optional decoration.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One global schematic label and its optional decoration.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "shape",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "global_label"
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
///    "shape": {
///      "$ref": "#/$defs/SchematicLabelShape"
///    },
///    "text": {
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
pub struct SchematicGlobalLabelPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_global_label_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub shape: SchematicLabelShape,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
///One hierarchical schematic label and its optional decoration.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One hierarchical schematic label and its optional decoration.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "shape",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "hierarchical_label"
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
///    "shape": {
///      "$ref": "#/$defs/SchematicLabelShape"
///    },
///    "text": {
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
pub struct SchematicHierarchicalLabelPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_hierarchical_label_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub shape: SchematicLabelShape,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
///One schematic junction marker.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One schematic junction marker.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "color": {
///      "description": "Authored junction color; null preserves an authored transparent color.",
///      "anyOf": [
///        {
///          "type": "string"
///        },
///        {
///          "type": "null"
///        }
///      ]
///    },
///    "kind": {
///      "type": "string",
///      "const": "junction"
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
pub struct SchematicJunctionPlotRecord {
    ///Authored junction color; null preserves an authored transparent color.
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nullable_string",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub color: ::std::option::Option<::std::option::Option<::std::string::String>>,
    #[serde(deserialize_with = "crate::deserialize_junction_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
///One local schematic label.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One local schematic label.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "label"
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
///    "text": {
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
pub struct SchematicLabelPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_label_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
///Signal-direction shapes preserved on global and hierarchical labels.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Signal-direction shapes preserved on global and hierarchical labels.",
///  "type": "string",
///  "enum": [
///    "input",
///    "output",
///    "bidirectional",
///    "tri_state",
///    "passive",
///    "dot",
///    "round",
///    "diamond",
///    "rectangle"
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
pub enum SchematicLabelShape {
    #[serde(rename = "input")]
    Input,
    #[serde(rename = "output")]
    Output,
    #[serde(rename = "bidirectional")]
    Bidirectional,
    #[serde(rename = "tri_state")]
    TriState,
    #[serde(rename = "passive")]
    Passive,
    #[serde(rename = "dot")]
    Dot,
    #[serde(rename = "round")]
    Round,
    #[serde(rename = "diamond")]
    Diamond,
    #[serde(rename = "rectangle")]
    Rectangle,
}
impl ::std::fmt::Display for SchematicLabelShape {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Input => f.write_str("input"),
            Self::Output => f.write_str("output"),
            Self::Bidirectional => f.write_str("bidirectional"),
            Self::TriState => f.write_str("tri_state"),
            Self::Passive => f.write_str("passive"),
            Self::Dot => f.write_str("dot"),
            Self::Round => f.write_str("round"),
            Self::Diamond => f.write_str("diamond"),
            Self::Rectangle => f.write_str("rectangle"),
        }
    }
}
impl ::std::str::FromStr for SchematicLabelShape {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "input" => Ok(Self::Input),
            "output" => Ok(Self::Output),
            "bidirectional" => Ok(Self::Bidirectional),
            "tri_state" => Ok(Self::TriState),
            "passive" => Ok(Self::Passive),
            "dot" => Ok(Self::Dot),
            "round" => Ok(Self::Round),
            "diamond" => Ok(Self::Diamond),
            "rectangle" => Ok(Self::Rectangle),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SchematicLabelShape {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SchematicLabelShape {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SchematicLabelShape {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///One netclass/directive flag with its visible property text.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One netclass/directive flag with its visible property text.",
///  "type": "object",
///  "required": [
///    "at_x_nm",
///    "at_y_nm",
///    "kind",
///    "length_nm",
///    "object_id",
///    "operation_count",
///    "operations",
///    "shape",
///    "uuid"
///  ],
///  "properties": {
///    "at_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "at_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "kind": {
///      "type": "string",
///      "const": "netclass_flag"
///    },
///    "length_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
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
///    "shape": {
///      "$ref": "#/$defs/SchematicNetclassFlagShape"
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
pub struct SchematicNetclassFlagPlotRecord {
    pub at_x_nm: crate::JavaScriptSafeInteger,
    pub at_y_nm: crate::JavaScriptSafeInteger,
    #[serde(deserialize_with = "crate::deserialize_netclass_flag_record_kind")]
    pub kind: ::std::string::String,
    pub length_nm: crate::JavaScriptSafeInteger,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub shape: SchematicNetclassFlagShape,
    pub uuid: ::std::string::String,
}
///Marker shapes admitted by schematic netclass/directive flags.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Marker shapes admitted by schematic netclass/directive flags.",
///  "type": "string",
///  "enum": [
///    "round",
///    "dot",
///    "diamond",
///    "rectangle"
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
pub enum SchematicNetclassFlagShape {
    #[serde(rename = "round")]
    Round,
    #[serde(rename = "dot")]
    Dot,
    #[serde(rename = "diamond")]
    Diamond,
    #[serde(rename = "rectangle")]
    Rectangle,
}
impl ::std::fmt::Display for SchematicNetclassFlagShape {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Round => f.write_str("round"),
            Self::Dot => f.write_str("dot"),
            Self::Diamond => f.write_str("diamond"),
            Self::Rectangle => f.write_str("rectangle"),
        }
    }
}
impl ::std::str::FromStr for SchematicNetclassFlagShape {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "round" => Ok(Self::Round),
            "dot" => Ok(Self::Dot),
            "diamond" => Ok(Self::Diamond),
            "rectangle" => Ok(Self::Rectangle),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SchematicNetclassFlagShape {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for SchematicNetclassFlagShape {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SchematicNetclassFlagShape {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///One schematic no-connect cross.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One schematic no-connect cross.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "no_connect"
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
pub struct SchematicNoConnectPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_no_connect_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
///Exact page extent of one schematic instance.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Exact page extent of one schematic instance.",
///  "type": "object",
///  "required": [
///    "height_nm",
///    "width_nm"
///  ],
///  "properties": {
///    "height_nm": {
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
pub struct SchematicPlotCanvas {
    pub height_nm: crate::JavaScriptSafeInteger,
    pub width_nm: crate::JavaScriptSafeInteger,
}
///Strict schematic subset through the P5_061 annotation families.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.schematic_plot.document:a0",
///  "title": "Schematic plot document a0",
///  "description": "Strict schematic subset through the P5_061 annotation families.",
///  "type": "object",
///  "required": [
///    "canvas",
///    "coordinate_space",
///    "document_id",
///    "records",
///    "schema",
///    "source_kind",
///    "total_operations"
///  ],
///  "properties": {
///    "canvas": {
///      "$ref": "#/$defs/SchematicPlotCanvas"
///    },
///    "coordinate_space": {
///      "$ref": "#/$defs/PlotterCoordinateSpace"
///    },
///    "document_id": {
///      "type": "string"
///    },
///    "records": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/SchematicPlotRecord"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad.plotter_ir.a0"
///    },
///    "source_kind": {
///      "type": "string",
///      "const": "SCH"
///    },
///    "source_path": {
///      "type": "string"
///    },
///    "total_operations": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SchematicPlotDocumentA0 {
    pub canvas: SchematicPlotCanvas,
    pub coordinate_space: PlotterCoordinateSpace,
    pub document_id: ::std::string::String,
    pub records: ::std::vec::Vec<SchematicPlotRecord>,
    pub schema: ::std::string::String,
    pub source_kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_path: ::std::option::Option<::std::string::String>,
    pub total_operations: u32,
}
///Strict source-record vocabulary through the P5_061 schematic annotations.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Strict source-record vocabulary through the P5_061 schematic annotations.",
///  "oneOf": [
///    {
///      "$ref": "#/$defs/SchematicSheetHeaderPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicWirePlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicBusPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicBusEntryPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicJunctionPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicNoConnectPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicLabelPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicGlobalLabelPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicHierarchicalLabelPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicNetclassFlagPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicTextPlotRecord"
///    },
///    {
///      "$ref": "#/$defs/SchematicTextBoxPlotRecord"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SchematicPlotRecord {
    SheetHeaderPlotRecord(SchematicSheetHeaderPlotRecord),
    WirePlotRecord(SchematicWirePlotRecord),
    BusPlotRecord(SchematicBusPlotRecord),
    BusEntryPlotRecord(SchematicBusEntryPlotRecord),
    JunctionPlotRecord(SchematicJunctionPlotRecord),
    NoConnectPlotRecord(SchematicNoConnectPlotRecord),
    LabelPlotRecord(SchematicLabelPlotRecord),
    GlobalLabelPlotRecord(SchematicGlobalLabelPlotRecord),
    HierarchicalLabelPlotRecord(SchematicHierarchicalLabelPlotRecord),
    NetclassFlagPlotRecord(SchematicNetclassFlagPlotRecord),
    TextPlotRecord(SchematicTextPlotRecord),
    TextBoxPlotRecord(SchematicTextBoxPlotRecord),
}
impl ::std::convert::From<SchematicSheetHeaderPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicSheetHeaderPlotRecord) -> Self {
        Self::SheetHeaderPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicWirePlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicWirePlotRecord) -> Self {
        Self::WirePlotRecord(value)
    }
}
impl ::std::convert::From<SchematicBusPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicBusPlotRecord) -> Self {
        Self::BusPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicBusEntryPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicBusEntryPlotRecord) -> Self {
        Self::BusEntryPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicJunctionPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicJunctionPlotRecord) -> Self {
        Self::JunctionPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicNoConnectPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicNoConnectPlotRecord) -> Self {
        Self::NoConnectPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicLabelPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicLabelPlotRecord) -> Self {
        Self::LabelPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicGlobalLabelPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicGlobalLabelPlotRecord) -> Self {
        Self::GlobalLabelPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicHierarchicalLabelPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicHierarchicalLabelPlotRecord) -> Self {
        Self::HierarchicalLabelPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicNetclassFlagPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicNetclassFlagPlotRecord) -> Self {
        Self::NetclassFlagPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicTextPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicTextPlotRecord) -> Self {
        Self::TextPlotRecord(value)
    }
}
impl ::std::convert::From<SchematicTextBoxPlotRecord> for SchematicPlotRecord {
    fn from(value: SchematicTextBoxPlotRecord) -> Self {
        Self::TextBoxPlotRecord(value)
    }
}
///Typed title-block metadata carried by the leading sheet-header record.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Typed title-block metadata carried by the leading sheet-header record.",
///  "type": "object",
///  "required": [
///    "comments",
///    "company",
///    "date",
///    "rev",
///    "title"
///  ],
///  "properties": {
///    "comments": {
///      "$ref": "#/$defs/RecordString"
///    },
///    "company": {
///      "type": "string"
///    },
///    "date": {
///      "type": "string"
///    },
///    "rev": {
///      "type": "string"
///    },
///    "title": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SchematicPlotTitleBlock {
    pub comments: RecordString,
    pub company: ::std::string::String,
    pub date: ::std::string::String,
    pub rev: ::std::string::String,
    pub title: ::std::string::String,
}
///Leading paper, title-block, background, and worksheet record.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Leading paper, title-block, background, and worksheet record.",
///  "type": "object",
///  "required": [
///    "generator",
///    "generator_version",
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "paper_height_mm",
///    "paper_portrait",
///    "paper_size",
///    "paper_width_mm",
///    "sheet_height_nm",
///    "sheet_width_nm",
///    "uuid",
///    "version"
///  ],
///  "properties": {
///    "generator": {
///      "type": "string"
///    },
///    "generator_version": {
///      "type": "string"
///    },
///    "kind": {
///      "type": "string",
///      "const": "sheet_header"
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
///    "paper_height_mm": {
///      "anyOf": [
///        {
///          "type": "number"
///        },
///        {
///          "type": "null"
///        }
///      ]
///    },
///    "paper_portrait": {
///      "type": "boolean"
///    },
///    "paper_size": {
///      "type": "string"
///    },
///    "paper_width_mm": {
///      "anyOf": [
///        {
///          "type": "number"
///        },
///        {
///          "type": "null"
///        }
///      ]
///    },
///    "sheet_height_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "sheet_width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "title_block": {
///      "$ref": "#/$defs/SchematicPlotTitleBlock"
///    },
///    "uuid": {
///      "type": "string"
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
pub struct SchematicSheetHeaderPlotRecord {
    pub generator: ::std::string::String,
    pub generator_version: ::std::string::String,
    #[serde(deserialize_with = "crate::deserialize_sheet_header_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub paper_height_mm: ::std::option::Option<f64>,
    pub paper_portrait: bool,
    pub paper_size: ::std::string::String,
    pub paper_width_mm: ::std::option::Option<f64>,
    pub sheet_height_nm: crate::JavaScriptSafeInteger,
    pub sheet_width_nm: crate::JavaScriptSafeInteger,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub title_block: ::std::option::Option<SchematicPlotTitleBlock>,
    pub uuid: ::std::string::String,
    pub version: crate::JavaScriptSafeInteger,
}
///One schematic text box with its canonical outline and plotted lines.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One schematic text box with its canonical outline and plotted lines.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "text_box"
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
///    "text": {
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
pub struct SchematicTextBoxPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_text_box_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
///One ordinary top-level schematic text annotation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One ordinary top-level schematic text annotation.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "text",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "text"
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
///    "text": {
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
pub struct SchematicTextPlotRecord {
    #[serde(deserialize_with = "crate::deserialize_text_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub text: ::std::string::String,
    pub uuid: ::std::string::String,
}
///One schematic wire polyline.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One schematic wire polyline.",
///  "type": "object",
///  "required": [
///    "kind",
///    "object_id",
///    "operation_count",
///    "operations",
///    "uuid"
///  ],
///  "properties": {
///    "kind": {
///      "type": "string",
///      "const": "wire"
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
pub struct SchematicWirePlotRecord {
    #[serde(deserialize_with = "crate::deserialize_wire_record_kind")]
    pub kind: ::std::string::String,
    pub object_id: ::std::string::String,
    pub operation_count: u32,
    pub operations: ::std::vec::Vec<PlotterOperation>,
    pub uuid: ::std::string::String,
}
/**Stroke or cached text operation. Boolean marker keys (`mirror`,
`text_as_polygons`, `polyline_per_segment`, `knockout`) are present-only
-when-true, matching the established Python emitter. Render-cache keys
appear together when an authored cache resolves; `render_cache_polygons`
carries the exterior rings in nanometres.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Stroke or cached text operation. Boolean marker keys (`mirror`,\n`text_as_polygons`, `polyline_per_segment`, `knockout`) are present-only\n-when-true, matching the established Python emitter. Render-cache keys\nappear together when an authored cache resolves; `render_cache_polygons`\ncarries the exterior rings in nanometres.",
///  "type": "object",
///  "required": [
///    "bold",
///    "color",
///    "font_face",
///    "h_align",
///    "index",
///    "italic",
///    "kind",
///    "multiline",
///    "orient_deg",
///    "pen_width_nm",
///    "size_x_nm",
///    "size_y_nm",
///    "text",
///    "v_align",
///    "x",
///    "y"
///  ],
///  "properties": {
///    "bold": {
///      "type": "boolean"
///    },
///    "color": {
///      "type": "string"
///    },
///    "context": {
///      "$ref": "#/$defs/PlotterOperationContext"
///    },
///    "font_face": {
///      "type": "string"
///    },
///    "h_align": {
///      "$ref": "#/$defs/PlotterTextHAlign"
///    },
///    "index": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "italic": {
///      "type": "boolean"
///    },
///    "kind": {
///      "type": "string",
///      "const": "Text"
///    },
///    "knockout": {
///      "type": "boolean"
///    },
///    "layer": {
///      "type": "string"
///    },
///    "mirror": {
///      "type": "boolean"
///    },
///    "multiline": {
///      "type": "boolean"
///    },
///    "orient_deg": {
///      "type": "number"
///    },
///    "pen_width_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "polyline_per_segment": {
///      "type": "boolean"
///    },
///    "render_cache": {
///      "$ref": "#/$defs/TextRenderCache"
///    },
///    "render_cache_exact": {
///      "type": "boolean"
///    },
///    "render_cache_polygons": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    },
///    "render_cache_source": {
///      "$ref": "#/$defs/PlotterTextRenderCacheSource"
///    },
///    "size_x_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "size_y_nm": {
///      "$ref": "#/$defs/JavaScriptSafeInteger"
///    },
///    "text": {
///      "type": "string"
///    },
///    "text_as_polygons": {
///      "type": "boolean"
///    },
///    "v_align": {
///      "$ref": "#/$defs/PlotterTextVAlign"
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
pub struct TextOperation {
    pub bold: bool,
    pub color: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub context: ::std::option::Option<PlotterOperationContext>,
    pub font_face: ::std::string::String,
    pub h_align: PlotterTextHAlign,
    pub index: u32,
    pub italic: bool,
    #[serde(deserialize_with = "crate::deserialize_text_kind")]
    pub kind: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub knockout: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub layer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub mirror: ::std::option::Option<bool>,
    pub multiline: bool,
    pub orient_deg: f64,
    pub pen_width_nm: crate::JavaScriptSafeInteger,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub polyline_per_segment: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub render_cache: ::std::option::Option<TextRenderCache>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub render_cache_exact: ::std::option::Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::reject_present_render_cache_polygons",
        skip_serializing_if = "::std::vec::Vec::is_empty"
    )]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub render_cache_source: ::std::option::Option<PlotterTextRenderCacheSource>,
    pub size_x_nm: crate::JavaScriptSafeInteger,
    pub size_y_nm: crate::JavaScriptSafeInteger,
    pub text: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub text_as_polygons: ::std::option::Option<bool>,
    pub v_align: PlotterTextVAlign,
    pub x: crate::JavaScriptSafeInteger,
    pub y: crate::JavaScriptSafeInteger,
}
/**Typed render cache from an authored `(render_cache ...)` form, the Python
resolver, or the deterministic native hinted outline engine. `knockout` appears when the
knockout background restructure replaced the polygons.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Typed render cache from an authored `(render_cache ...)` form, the Python\nresolver, or the deterministic native hinted outline engine. `knockout` appears when the\nknockout background restructure replaced the polygons.",
///  "type": "object",
///  "required": [
///    "angle",
///    "coordinate_space",
///    "exact",
///    "polygons",
///    "schema",
///    "source",
///    "text",
///    "unit"
///  ],
///  "properties": {
///    "angle": {
///      "type": "number"
///    },
///    "coordinate_space": {
///      "$ref": "#/$defs/PlotterTextRenderCacheCoordinateSpace"
///    },
///    "exact": {
///      "type": "boolean"
///    },
///    "knockout": {
///      "type": "boolean"
///    },
///    "polygons": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/TextRenderCachePolygon"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad.render_cache.v1"
///    },
///    "source": {
///      "$ref": "#/$defs/PlotterTextRenderCacheSource"
///    },
///    "text": {
///      "type": "string"
///    },
///    "unit": {
///      "type": "string",
///      "const": "nm"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TextRenderCache {
    pub angle: f64,
    pub coordinate_space: PlotterTextRenderCacheCoordinateSpace,
    pub exact: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub knockout: ::std::option::Option<bool>,
    pub polygons: ::std::vec::Vec<TextRenderCachePolygon>,
    pub schema: ::std::string::String,
    pub source: PlotterTextRenderCacheSource,
    pub text: ::std::string::String,
    pub unit: ::std::string::String,
}
///One render-cache polygon as ordered contours, exterior ring first.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "One render-cache polygon as ordered contours, exterior ring first.",
///  "type": "object",
///  "required": [
///    "contours"
///  ],
///  "properties": {
///    "contours": {
///      "type": "array",
///      "items": {
///        "type": "array",
///        "items": {
///          "$ref": "#/$defs/PlotterPoint"
///        }
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TextRenderCachePolygon {
    pub contours: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,
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
///    "stroke_color": {
///      "type": "string"
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
    #[serde(deserialize_with = "crate::deserialize_thick_segment_kind")]
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
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub stroke_color: ::std::option::Option<::std::string::String>,
    pub width_nm: crate::JavaScriptSafeInteger,
}

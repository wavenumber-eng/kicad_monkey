//! Generated transport types for KiCad Monkey operation boundaries.

#![forbid(unsafe_code)]
#![allow(
    clippy::derivable_impls,
    reason = "Typify emits explicit schema defaults in committed generated modules"
)]

mod compiled_graph_contract;
mod font_bundle_contract;
pub mod generated;
mod source_bundle_contract;

pub use compiled_graph_contract::{
    CompiledGraphDecodeError, decode_compiled_schematic_graph_a0,
    validate_compiled_schematic_graph_contract,
};
pub use font_bundle_contract::{
    FontBundleLimits, FontResolutionLimits, ValidatedFontBundle, resolve_font_selection_contract,
    validate_font_bundle_contract,
};
pub use source_bundle_contract::{
    SourceBundleDecodeError, decode_source_bundle_manifest_a0,
    validate_source_bundle_manifest_contract,
};

use generated::build_request::{Node, NodeKind, SExpressionBuildRequestA0};
use generated::footprint_plot_document::{
    CircleOperation, FlashPadCustomOperation, FootprintPlotDocumentA0, PlotterDrillRole,
    PlotterOperation, ThickSegmentOperation,
};
use generated::symbol_plot_document::{
    PlotterOperation as SymbolOperation, SymbolPlotDocumentA0, SymbolPlotRecord,
};
use std::fmt;

/// Largest integer represented exactly by JavaScript's IEEE-754 `number`.
pub const JAVASCRIPT_SAFE_INTEGER_MAX: i64 = 9_007_199_254_740_991;
/// Smallest integer represented exactly by JavaScript's IEEE-754 `number`.
pub const JAVASCRIPT_SAFE_INTEGER_MIN: i64 = -JAVASCRIPT_SAFE_INTEGER_MAX;

/// Integer guaranteed to remain exact across JSON and JavaScript/WASM boundaries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct JavaScriptSafeInteger(i64);

impl JavaScriptSafeInteger {
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for JavaScriptSafeInteger {
    type Error = JavaScriptSafeIntegerError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if (JAVASCRIPT_SAFE_INTEGER_MIN..=JAVASCRIPT_SAFE_INTEGER_MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(JavaScriptSafeIntegerError { value })
        }
    }
}

impl From<JavaScriptSafeInteger> for i64 {
    fn from(value: JavaScriptSafeInteger) -> Self {
        value.0
    }
}

impl<'de> serde::Deserialize<'de> for JavaScriptSafeInteger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = i64::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JavaScriptSafeIntegerError {
    value: i64,
}

impl fmt::Display for JavaScriptSafeIntegerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} is outside the JavaScript safe-integer range",
            self.value
        )
    }
}

impl std::error::Error for JavaScriptSafeIntegerError {}

/// Finite nonnegative floating-point value used for governed comparison tolerances.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct NonNegativeFiniteFloat(f64);

impl NonNegativeFiniteFloat {
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for NonNegativeFiniteFloat {
    type Error = NonNegativeFiniteFloatError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_finite() && value >= 0.0 {
            Ok(Self(value))
        } else {
            Err(NonNegativeFiniteFloatError { value })
        }
    }
}

impl<'de> serde::Deserialize<'de> for NonNegativeFiniteFloat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonNegativeFiniteFloatError {
    value: f64,
}

impl fmt::Display for NonNegativeFiniteFloatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} is not a finite nonnegative number",
            self.value
        )
    }
}

impl std::error::Error for NonNegativeFiniteFloatError {}

/// Validated, payload-exclusive generic node ready for conversion by adapters.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidatedNode {
    List(Vec<Self>),
    Atom(String),
    Quoted(String),
    Integer(i64),
    Float(f64),
}

/// A semantically validated build request with platform-sized resource limits.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedBuildRequest {
    pub root: ValidatedNode,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
}

/// Stable semantic contract validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub code: &'static str,
    pub path: String,
    pub message: &'static str,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

impl std::error::Error for ValidationError {}

/// Enforce identity, resource limits, and the `Node.kind` payload union.
pub fn validate_build_request(
    request: SExpressionBuildRequestA0,
) -> Result<ValidatedBuildRequest, ValidationError> {
    if request.type_ != "kicad_monkey.sexpr_build.request" || request.version != "a0" {
        return Err(validation_error(
            "unsupported_contract",
            "$",
            "unsupported S-expression build contract identity",
        ));
    }
    let max_output_bytes = request.max_output_bytes.parse::<usize>().map_err(|_| {
        validation_error(
            "invalid_limit",
            "$.max_output_bytes",
            "max_output_bytes must be a platform-sized decimal string",
        )
    })?;
    let max_depth = request.max_depth as usize;
    let max_nodes = request.max_nodes as usize;
    if max_output_bytes == 0 || max_nodes == 0 {
        return Err(validation_error(
            "invalid_limit",
            "$",
            "max_output_bytes and max_nodes must be greater than zero",
        ));
    }

    let mut node_count = 0_usize;
    let root = validate_node(
        request.root,
        "$.root".to_owned(),
        0,
        max_depth,
        max_nodes,
        &mut node_count,
    )?;
    Ok(ValidatedBuildRequest {
        root,
        max_output_bytes,
        max_depth,
        max_nodes,
    })
}

/// Enforce the cross-field semantics of shared graphical and drill operations.
pub fn validate_footprint_plot_document(
    document: &FootprintPlotDocumentA0,
) -> Result<(), ValidationError> {
    let mut total_operations = 0usize;
    for (record_index, record) in document.records.iter().enumerate() {
        if record.operation_count as usize != record.operations.len() {
            return Err(validation_error(
                "operation_count_mismatch",
                format!("$.records[{record_index}].operation_count"),
                "operation_count must equal the operation array length",
            ));
        }
        total_operations = total_operations.saturating_add(record.operations.len());
        for (operation_index, operation) in record.operations.iter().enumerate() {
            let path = format!("$.records[{record_index}].operations[{operation_index}]");
            validate_footprint_operation(operation, path)?;
        }
    }
    if document.total_operations as usize != total_operations {
        return Err(validation_error(
            "operation_count_mismatch",
            "$.total_operations",
            "total_operations must equal all record operation counts",
        ));
    }
    Ok(())
}

fn validate_footprint_operation(
    operation: &PlotterOperation,
    path: String,
) -> Result<(), ValidationError> {
    match operation {
        PlotterOperation::ThickSegmentOperation(operation) => {
            validate_shared_segment(operation, path)
        }
        PlotterOperation::CircleOperation(operation) => validate_shared_circle(operation, path),
        _ => validate_footprint_static_operation(operation, path),
    }
}

fn validate_footprint_static_operation(
    operation: &PlotterOperation,
    path: String,
) -> Result<(), ValidationError> {
    match operation {
        PlotterOperation::ArcThreePointOperation(value) => {
            require_layer(value.layer.as_deref(), path)
        }
        PlotterOperation::RectOperation(value) => require_layer(value.layer.as_deref(), path),
        PlotterOperation::PlotPolyOperation(value) => require_layer(value.layer.as_deref(), path),
        PlotterOperation::BezierCurveOperation(value) => {
            require_layer(value.layer.as_deref(), path)
        }
        _ => validate_footprint_pad_operation(operation, path),
    }
}

fn validate_footprint_pad_operation(
    operation: &PlotterOperation,
    path: String,
) -> Result<(), ValidationError> {
    match operation {
        PlotterOperation::FlashPadCircleOperation(value) => require_layers(&value.layers, path),
        PlotterOperation::FlashPadOvalOperation(value) => require_layers(&value.layers, path),
        PlotterOperation::FlashPadRectOperation(value) => require_layers(&value.layers, path),
        PlotterOperation::FlashPadRoundRectOperation(value) => require_layers(&value.layers, path),
        PlotterOperation::FlashPadCustomOperation(value) => validate_custom_pad(value, path),
        PlotterOperation::FlashPadTrapezOperation(value) => require_layers(&value.layers, path),
        _ => Ok(()),
    }
}

/// Enforce record counts and producer-specific states for symbol geometry.
pub fn validate_symbol_plot_document(
    document: &SymbolPlotDocumentA0,
) -> Result<(), ValidationError> {
    if !matches!(
        document.records.first(),
        Some(SymbolPlotRecord::SymbolHeaderPlotRecord(_))
    ) {
        return Err(validation_error(
            "missing_symbol_header",
            "$.records[0]",
            "the first symbol record must be the lib_symbol header",
        ));
    }
    let mut total_operations = 0usize;
    for (record_index, record) in document.records.iter().enumerate() {
        let (declared, operations) = match record {
            SymbolPlotRecord::SymbolHeaderPlotRecord(record) => {
                if record_index != 0 || record.operation_count != 0 || !record.operations.is_empty()
                {
                    return Err(validation_error(
                        "invalid_symbol_header",
                        format!("$.records[{record_index}]"),
                        "lib_symbol header operations must be empty",
                    ));
                }
                (record.operation_count, &record.operations)
            }
            SymbolPlotRecord::LibSubsymbolPlotRecord(record) => {
                (record.operation_count, &record.operations)
            }
        };
        if declared as usize != operations.len() {
            return Err(validation_error(
                "operation_count_mismatch",
                format!("$.records[{record_index}].operation_count"),
                "operation_count must equal the operation array length",
            ));
        }
        total_operations = total_operations.saturating_add(operations.len());
        for (operation_index, operation) in operations.iter().enumerate() {
            let path = format!("$.records[{record_index}].operations[{operation_index}]");
            validate_symbol_operation(operation, path)?;
        }
    }
    if document.total_operations as usize != total_operations {
        return Err(validation_error(
            "operation_count_mismatch",
            "$.total_operations",
            "total_operations must equal all record operation counts",
        ));
    }
    Ok(())
}

fn validate_symbol_operation(
    operation: &SymbolOperation,
    path: String,
) -> Result<(), ValidationError> {
    let valid = match operation {
        SymbolOperation::ArcThreePointOperation(value) => value.layer.is_none(),
        SymbolOperation::RectOperation(value) => value.layer.is_none(),
        SymbolOperation::PlotPolyOperation(value) => value.layer.is_none(),
        SymbolOperation::BezierCurveOperation(value) => value.layer.is_none(),
        SymbolOperation::CircleOperation(value) => symbol_circle_is_layer_free(value),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(validation_error(
            "invalid_symbol_operation",
            path,
            "symbol records accept only layer-free body geometry",
        ))
    }
}

fn symbol_circle_is_layer_free(value: &generated::symbol_plot_document::CircleOperation) -> bool {
    value.layer.is_none()
        && value.role.is_none()
        && value.layers.is_empty()
        && value.mask_margin_nm.is_none()
        && value.pad_size_x_nm.is_none()
        && value.pad_size_y_nm.is_none()
}

fn validate_custom_pad(
    operation: &FlashPadCustomOperation,
    path: String,
) -> Result<(), ValidationError> {
    require_layers(&operation.layers, path.clone())?;
    if !operation.polygon_widths_nm.is_empty()
        && operation.polygon_widths_nm.len() != operation.polygons.len()
    {
        return Err(validation_error(
            "polygon_width_count_mismatch",
            format!("{path}.polygon_widths_nm"),
            "non-empty polygon_widths_nm must contain one width per polygon",
        ));
    }
    Ok(())
}

fn validate_shared_segment(
    operation: &ThickSegmentOperation,
    path: String,
) -> Result<(), ValidationError> {
    validate_graphic_or_drill(
        operation.layer.as_deref(),
        operation.role,
        &operation.layers,
        operation.mask_margin_nm.is_some(),
        operation.pad_size_x_nm.is_some(),
        operation.pad_size_y_nm.is_some(),
        path,
    )
}

fn validate_shared_circle(
    operation: &CircleOperation,
    path: String,
) -> Result<(), ValidationError> {
    validate_graphic_or_drill(
        operation.layer.as_deref(),
        operation.role,
        &operation.layers,
        operation.mask_margin_nm.is_some(),
        operation.pad_size_x_nm.is_some(),
        operation.pad_size_y_nm.is_some(),
        path,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the validator receives the complete shared graphic/drill state as separate fields"
)]
fn validate_graphic_or_drill(
    layer: Option<&str>,
    role: Option<PlotterDrillRole>,
    layers: &[String],
    has_mask_margin: bool,
    has_pad_size_x: bool,
    has_pad_size_y: bool,
    path: String,
) -> Result<(), ValidationError> {
    match role {
        None if layer.is_some()
            && layers.is_empty()
            && !has_mask_margin
            && !has_pad_size_x
            && !has_pad_size_y =>
        {
            Ok(())
        }
        Some(PlotterDrillRole::PadDrill)
            if layer.is_none()
                && !layers.is_empty()
                && !has_mask_margin
                && !has_pad_size_x
                && !has_pad_size_y =>
        {
            Ok(())
        }
        Some(PlotterDrillRole::NpthHole)
            if layer.is_none()
                && !layers.is_empty()
                && has_mask_margin
                && has_pad_size_x
                && has_pad_size_y =>
        {
            Ok(())
        }
        _ => Err(validation_error(
            "conflicting_plotter_fields",
            path,
            "circle/segment must be one complete graphic, pad-drill, or NPTH variant",
        )),
    }
}

fn require_layers(layers: &[String], path: String) -> Result<(), ValidationError> {
    if layers.is_empty() {
        Err(validation_error(
            "missing_layers",
            path,
            "pad flash operations require at least one layer",
        ))
    } else {
        Ok(())
    }
}

fn require_layer(layer: Option<&str>, path: String) -> Result<(), ValidationError> {
    if layer.is_some_and(|value| !value.is_empty()) {
        Ok(())
    } else {
        Err(validation_error(
            "missing_layer",
            path,
            "footprint graphic operations require a layer",
        ))
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "pre-standard recursive union validator retained under the structural ratchet"
)]
fn validate_node(
    node: Node,
    path: String,
    depth: usize,
    max_depth: usize,
    max_nodes: usize,
    node_count: &mut usize,
) -> Result<ValidatedNode, ValidationError> {
    if depth > max_depth {
        return Err(validation_error(
            "resource_limit",
            path,
            "node nesting exceeds max_depth",
        ));
    }
    *node_count = node_count.saturating_add(1);
    if *node_count > max_nodes {
        return Err(validation_error(
            "resource_limit",
            path,
            "node count exceeds max_nodes",
        ));
    }

    let Node {
        children,
        float,
        integer,
        kind,
        text,
    } = node;
    match kind {
        NodeKind::List => {
            require_absent(
                &path,
                text.is_none() && integer.is_none() && float.is_none(),
            )?;
            let mut validated = Vec::with_capacity(children.len());
            for (index, child) in children.into_iter().enumerate() {
                validated.push(validate_node(
                    child,
                    format!("{path}.children[{index}]"),
                    depth + 1,
                    max_depth,
                    max_nodes,
                    node_count,
                )?);
            }
            Ok(ValidatedNode::List(validated))
        }
        NodeKind::Atom | NodeKind::Quoted => {
            require_absent(
                &path,
                children.is_empty() && integer.is_none() && float.is_none(),
            )?;
            let text = text.ok_or_else(|| {
                validation_error(
                    "missing_payload",
                    &path,
                    "text payload is required for this kind",
                )
            })?;
            Ok(if kind == NodeKind::Atom {
                ValidatedNode::Atom(text)
            } else {
                ValidatedNode::Quoted(text)
            })
        }
        NodeKind::Integer => {
            require_absent(
                &path,
                children.is_empty() && text.is_none() && float.is_none(),
            )?;
            let integer = integer.ok_or_else(|| {
                validation_error(
                    "missing_payload",
                    &path,
                    "integer payload is required for this kind",
                )
            })?;
            let value = integer.parse::<i64>().map_err(|_| {
                validation_error(
                    "invalid_integer",
                    &path,
                    "integer payload must fit signed 64-bit decimal",
                )
            })?;
            Ok(ValidatedNode::Integer(value))
        }
        NodeKind::Float => {
            require_absent(
                &path,
                children.is_empty() && text.is_none() && integer.is_none(),
            )?;
            let value = float.ok_or_else(|| {
                validation_error(
                    "missing_payload",
                    &path,
                    "float payload is required for this kind",
                )
            })?;
            if !value.is_finite() {
                return Err(validation_error(
                    "invalid_float",
                    path,
                    "float payload must be finite",
                ));
            }
            Ok(ValidatedNode::Float(value))
        }
    }
}

fn require_absent(path: &str, valid: bool) -> Result<(), ValidationError> {
    if valid {
        Ok(())
    } else {
        Err(validation_error(
            "conflicting_payload",
            path,
            "node contains payload fields not allowed by its kind",
        ))
    }
}

pub(crate) fn validation_error(
    code: &'static str,
    path: impl Into<String>,
    message: &'static str,
) -> ValidationError {
    ValidationError {
        code,
        path: path.into(),
        message,
    }
}

//! Generated transport types for KiCad Monkey operation boundaries.

#![forbid(unsafe_code)]
#![allow(
    clippy::derivable_impls,
    reason = "Typify emits explicit schema defaults in committed generated modules"
)]

mod board_plot_contract;
mod compiled_graph_contract;
mod font_bundle_contract;
mod font_text_contract;
pub mod generated;
mod source_bundle_contract;

pub use board_plot_contract::validate_board_plot_document;
pub use compiled_graph_contract::{
    CompiledGraphDecodeError, decode_compiled_schematic_graph_a0,
    validate_compiled_schematic_graph_contract,
};
pub use font_bundle_contract::{
    FontBundleLimits, FontResolutionLimits, ValidatedFontBundle, resolve_font_selection_contract,
    validate_font_bundle_contract,
};
pub use font_text_contract::{
    validate_outline_vector_contract, validate_shaping_input_contract,
    validate_shaping_record_contract,
};
pub use source_bundle_contract::{
    SourceBundleDecodeError, decode_source_bundle_manifest_a0,
    validate_source_bundle_manifest_contract,
};

use generated::build_request::{Node, NodeKind, SExpressionBuildRequestA0};
use generated::footprint_plot_document::{
    CircleOperation, FlashPadCustomOperation, FootprintPlotDocumentA0, PlotterDrillRole,
    PlotterOperation, TextOperation, ThickSegmentOperation,
};
use generated::symbol_plot_document::{
    PlotterOperation as SymbolOperation, SymbolPlotDocumentA0, SymbolPlotRecord,
};

#[doc(hidden)]
pub fn reject_present_render_cache_polygons<'de, D, T>(_: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Err(serde::de::Error::custom(
        "standalone footprint text forbids render_cache_polygons",
    ))
}

macro_rules! literal_kind_deserializer {
    ($name:ident, $expected:literal) => {
        #[doc(hidden)]
        pub fn $name<'de, D>(deserializer: D) -> Result<String, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = <String as serde::Deserialize>::deserialize(deserializer)?;
            if value == $expected {
                Ok(value)
            } else {
                Err(serde::de::Error::custom(concat!(
                    "expected operation kind ",
                    $expected
                )))
            }
        }
    };
}

literal_kind_deserializer!(deserialize_thick_segment_kind, "ThickSegment");
literal_kind_deserializer!(deserialize_arc_three_point_kind, "ArcThreePoint");
literal_kind_deserializer!(deserialize_circle_kind, "Circle");
literal_kind_deserializer!(deserialize_rect_kind, "Rect");
literal_kind_deserializer!(deserialize_plot_poly_kind, "PlotPoly");
literal_kind_deserializer!(deserialize_bezier_curve_kind, "BezierCurve");
literal_kind_deserializer!(deserialize_text_kind, "Text");
literal_kind_deserializer!(deserialize_flash_pad_circle_kind, "FlashPadCircle");
literal_kind_deserializer!(deserialize_flash_pad_oval_kind, "FlashPadOval");
literal_kind_deserializer!(deserialize_flash_pad_rect_kind, "FlashPadRect");
literal_kind_deserializer!(deserialize_flash_pad_round_rect_kind, "FlashPadRoundRect");
literal_kind_deserializer!(deserialize_flash_pad_custom_kind, "FlashPadCustom");
literal_kind_deserializer!(deserialize_flash_pad_trapez_kind, "FlashPadTrapez");
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

/// Finite float64 value used by programmatically constructed text DTOs.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct FiniteFloat(f64);

impl FiniteFloat {
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteFloat {
    type Error = FiniteFloatError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(FiniteFloatError { value })
        }
    }
}

impl<'de> serde::Deserialize<'de> for FiniteFloat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteFloatError {
    value: f64,
}

impl fmt::Display for FiniteFloatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} is not finite", self.value)
    }
}

impl std::error::Error for FiniteFloatError {}

/// Strictly positive OpenType units-per-em transport value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct PositiveU32(u32);

impl PositiveU32 {
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for PositiveU32 {
    type Error = PositiveU32Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(PositiveU32Error)
        }
    }
}

impl<'de> serde::Deserialize<'de> for PositiveU32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositiveU32Error;

impl fmt::Display for PositiveU32Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("value must be greater than zero")
    }
}

impl std::error::Error for PositiveU32Error {}

/// Nonempty stable ASCII identifier for font entries and oracle cases.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct StableTextId(String);

impl StableTextId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for StableTextId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for StableTextId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for StableTextId {
    type Err = StableTextIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if valid_stable_text_id(value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(StableTextIdError)
        }
    }
}

impl<'de> serde::Deserialize<'de> for StableTextId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

fn valid_stable_text_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StableTextIdError;

impl fmt::Display for StableTextIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("value is not a stable text identifier")
    }
}

impl std::error::Error for StableTextIdError {}

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
    if document.schema != "kicad.plotter_ir.a0"
        || document.source_kind != "MOD"
        || document.coordinate_space.unit != "nm"
        || document.coordinate_space.y_axis != "down"
        || document.records.len() != 1
    {
        return Err(validation_error(
            "invalid_footprint_document",
            "$".to_owned(),
            "footprint plot documents require canonical identity, coordinates, and one record",
        ));
    }
    let mut total_operations = 0usize;
    for (record_index, record) in document.records.iter().enumerate() {
        if record.kind != "footprint" || record.object_id != record.name {
            return Err(validation_error(
                "invalid_footprint_record",
                format!("$.records[{record_index}]"),
                "footprint records require kind=footprint and object_id=name",
            ));
        }
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
            let expected_index = u32::try_from(operation_index).map_err(|_| {
                validation_error(
                    "operation_index_mismatch",
                    format!("{path}.index"),
                    "operation index exceeds the contract range",
                )
            })?;
            validate_footprint_operation(operation, expected_index, path)?;
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

fn validate_footprint_operation_header(
    actual_index: u32,
    actual_kind: &str,
    expected_index: u32,
    expected_kind: &str,
    path: &str,
) -> Result<(), ValidationError> {
    if actual_kind != expected_kind {
        return Err(validation_error(
            "invalid_footprint_operation",
            format!("{path}.kind"),
            "operation kind must match its structural variant",
        ));
    }
    if actual_index != expected_index {
        return Err(validation_error(
            "operation_index_mismatch",
            format!("{path}.index"),
            "operation index must equal its position in the record",
        ));
    }
    Ok(())
}

macro_rules! validate_footprint_header {
    ($value:expr, $index:expr, $kind:literal, $path:expr) => {
        validate_footprint_operation_header($value.index, &$value.kind, $index, $kind, $path)?
    };
}

fn validate_footprint_operation(
    operation: &PlotterOperation,
    expected_index: u32,
    path: String,
) -> Result<(), ValidationError> {
    match operation {
        PlotterOperation::TextOperation(operation) => {
            validate_footprint_header!(operation, expected_index, "Text", &path);
            validate_footprint_text(operation, path)
        }
        PlotterOperation::ThickSegmentOperation(operation) => {
            validate_footprint_header!(operation, expected_index, "ThickSegment", &path);
            validate_shared_segment(operation, path)
        }
        PlotterOperation::CircleOperation(operation) => {
            validate_footprint_header!(operation, expected_index, "Circle", &path);
            validate_shared_circle(operation, path)
        }
        _ => validate_footprint_static_operation(operation, expected_index, path),
    }
}

fn validate_footprint_text(operation: &TextOperation, path: String) -> Result<(), ValidationError> {
    require_layer(operation.layer.as_deref(), path.clone())?;
    if operation.kind != "Text"
        || operation.mirror.is_some()
        || operation.text_as_polygons.is_some()
        || operation.polyline_per_segment.is_some()
        || operation.knockout.is_some()
        || !operation.render_cache_polygons.is_empty()
        || operation.render_cache.is_some()
        || operation.render_cache_source.is_some()
        || operation.render_cache_exact.is_some()
    {
        return Err(validation_error(
            "invalid_footprint_text",
            path,
            "standalone footprint Text operations require a layer and cache-free canonical state",
        ));
    }
    Ok(())
}

fn validate_footprint_static_operation(
    operation: &PlotterOperation,
    expected_index: u32,
    path: String,
) -> Result<(), ValidationError> {
    match operation {
        PlotterOperation::ArcThreePointOperation(value) => {
            validate_footprint_header!(value, expected_index, "ArcThreePoint", &path);
            require_layer(value.layer.as_deref(), path)
        }
        PlotterOperation::RectOperation(value) => {
            validate_footprint_header!(value, expected_index, "Rect", &path);
            require_layer(value.layer.as_deref(), path)
        }
        PlotterOperation::PlotPolyOperation(value) => {
            validate_footprint_header!(value, expected_index, "PlotPoly", &path);
            require_layer(value.layer.as_deref(), path)
        }
        PlotterOperation::BezierCurveOperation(value) => {
            validate_footprint_header!(value, expected_index, "BezierCurve", &path);
            require_layer(value.layer.as_deref(), path)
        }
        _ => validate_footprint_pad_operation(operation, expected_index, path),
    }
}

fn validate_footprint_pad_operation(
    operation: &PlotterOperation,
    expected_index: u32,
    path: String,
) -> Result<(), ValidationError> {
    match operation {
        PlotterOperation::FlashPadCircleOperation(value) => {
            validate_footprint_header!(value, expected_index, "FlashPadCircle", &path);
            // The shared flash-circle model carries optional via roles for the
            // board document; footprint pads keep the pad-margin state.
            if value.mask_margin_nm.is_none() || value.role.is_some() {
                return Err(validation_error(
                    "invalid_pad_operation",
                    path,
                    "footprint flash circles require mask_margin_nm and no via role",
                ));
            }
            require_layers(&value.layers, path)
        }
        PlotterOperation::FlashPadOvalOperation(value) => {
            validate_footprint_header!(value, expected_index, "FlashPadOval", &path);
            require_layers(&value.layers, path)
        }
        PlotterOperation::FlashPadRectOperation(value) => {
            validate_footprint_header!(value, expected_index, "FlashPadRect", &path);
            require_layers(&value.layers, path)
        }
        PlotterOperation::FlashPadRoundRectOperation(value) => {
            validate_footprint_header!(value, expected_index, "FlashPadRoundRect", &path);
            require_layers(&value.layers, path)
        }
        PlotterOperation::FlashPadCustomOperation(value) => {
            validate_footprint_header!(value, expected_index, "FlashPadCustom", &path);
            validate_custom_pad(value, path)
        }
        PlotterOperation::FlashPadTrapezOperation(value) => {
            validate_footprint_header!(value, expected_index, "FlashPadTrapez", &path);
            require_layers(&value.layers, path)
        }
        _ => Err(validation_error(
            "invalid_footprint_operation",
            path,
            "operation is outside the footprint producer contract",
        )),
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

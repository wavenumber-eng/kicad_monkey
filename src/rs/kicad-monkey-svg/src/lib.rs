//! Bounded deterministic SVG serialization over frozen Phase-5 Plotter-IR contracts.

#![forbid(unsafe_code)]

mod context;
mod direct;
mod operation;
mod sink;

pub use context::{
    LayerPattern, LayerSelection, PlotterOperationKind, SvgBackground, SvgColor, SvgContextLimits,
    SvgFillMode, SvgIdentityMode, SvgLineStyle, SvgProfile, SvgRenderContextA1,
    SvgRenderContextBuilder, SvgSemanticRole, SvgStyleOverride, SvgVisibility,
    ValidatedSvgRenderContextA1,
};
pub use direct::{
    SvgBounds, SvgFitOptions, SvgRenderLimits, SvgViewport, SvgWarning, ViewportPolicy,
    render_board_document_svg, render_board_svg, render_footprint_svg, render_schematic_page_svg,
    render_schematic_svg, render_symbol_svg,
};

use kicad_monkey_contracts::generated::native_svg_render_request::{
    NativeSvgPlotDocument, NativeSvgRenderLimits, NativeSvgRenderRequestA0, NativeSvgViewport,
};
use kicad_monkey_contracts::validate_native_svg_render_request_contract;
use operation::render_operation;
use serde_json::Value;
use sink::SvgSink;

const MAX_RECORDS: usize = 1_000_000;
const MAX_OPERATIONS: usize = 4_000_000;
const MAX_POINTS: usize = 16_000_000;
const MAX_TEXT_BYTES: usize = 256 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;
const MAX_BLOCK_DEPTH: usize = 4096;
const MAX_ELEMENTS: usize = 8_000_000;
const MAX_WORK: usize = 64_000_000;
const MAX_SVG_BYTES: usize = 512 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 768 * 1024 * 1024;

#[derive(Debug)]
pub struct SvgError {
    kind: SvgErrorKind,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SvgErrorKind {
    InvalidContext,
    InvalidDocument,
    InvalidViewport,
    UnsupportedSelector,
    ResourceLimit,
    UnsupportedFitText,
    EmptyBounds,
    ArithmeticOverflow,
    UnbalancedBlock,
    Serialization,
    Other,
}

impl SvgError {
    pub fn new(kind: SvgErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> SvgErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SvgError {}

// Internal compatibility constructor while the frozen renderer is migrated to
// the typed error constructors above. New public-boundary code must use
// `SvgError::new` with an explicit kind.
#[allow(
    non_snake_case,
    reason = "temporary compatibility constructor for the frozen renderer internals"
)]
pub(crate) fn SvgError(message: String) -> SvgError {
    SvgError::new(SvgErrorKind::Other, message)
}

#[cfg(test)]
mod error_tests {
    use super::{SvgError, SvgErrorKind, render_svg, render_svg_legacy};
    use kicad_monkey_contracts::decode_native_svg_render_request_a0;
    use serde_json::{Value, json};
    use std::{fs, path::PathBuf};

    #[test]
    fn every_public_error_kind_is_stored_explicitly() {
        for kind in [
            SvgErrorKind::InvalidContext,
            SvgErrorKind::InvalidDocument,
            SvgErrorKind::InvalidViewport,
            SvgErrorKind::UnsupportedSelector,
            SvgErrorKind::ResourceLimit,
            SvgErrorKind::UnsupportedFitText,
            SvgErrorKind::EmptyBounds,
            SvgErrorKind::ArithmeticOverflow,
            SvgErrorKind::UnbalancedBlock,
            SvgErrorKind::Serialization,
            SvgErrorKind::Other,
        ] {
            let error = SvgError::new(kind, "message deliberately has no classification words");
            assert_eq!(error.kind(), kind);
        }
    }

    #[test]
    fn delegated_adapter_preserves_legacy_multi_fault_error_winners() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let vectors: Value = serde_json::from_slice(
            &fs::read(root.join("tests/parity/footprint_plotter_a0_vectors.json"))
                .expect("read footprint vectors"),
        )
        .expect("decode footprint vectors");
        let document = vectors["vectors"][0]["expected"].clone();
        let base = json!({
            "type": "kicad_monkey.native.svg.request",
            "version": "a0",
            "profile": "plotter-base-a0",
            "document": {"kind": "footprint", "value": document},
            "viewport": {
                "min_x_nm": 0,
                "min_y_nm": 0,
                "width_nm": 20_000_000,
                "height_nm": 20_000_000
            },
            "limits": {
                "max_records": 1_000_000,
                "max_operations": 4_000_000,
                "max_points": "16000000",
                "max_text_bytes": "268435456",
                "max_image_encoded_bytes": "268435456",
                "max_block_depth": 4096,
                "max_svg_elements": "8000000",
                "max_render_work": "64000000",
                "max_svg_bytes": "536870912",
                "max_result_bytes": "805306368"
            }
        });
        let mut cases = Vec::new();
        let mut record_and_operation_limits = base.clone();
        record_and_operation_limits["limits"]["max_records"] = json!(0);
        record_and_operation_limits["limits"]["max_operations"] = json!(0);
        cases.push(record_and_operation_limits);
        let mut operation_and_point_limits = base.clone();
        operation_and_point_limits["limits"]["max_operations"] = json!(0);
        operation_and_point_limits["limits"]["max_points"] = json!("0");
        cases.push(operation_and_point_limits);
        let mut point_and_output_limits = base;
        point_and_output_limits["limits"]["max_points"] = json!("0");
        point_and_output_limits["limits"]["max_svg_bytes"] = json!("0");
        cases.push(point_and_output_limits);

        for value in cases {
            let request = decode_native_svg_render_request_a0(&serde_json::to_vec(&value).unwrap())
                .expect("typed compatibility request");
            let legacy = render_svg_legacy(&request).unwrap_err();
            let delegated = render_svg(&request).unwrap_err();
            assert_eq!(delegated.message(), legacy.message());
        }
    }
}

#[derive(Debug)]
pub struct SvgArtifact {
    pub source_kind: &'static str,
    pub document_id: String,
    pub svg: String,
    pub occurrence_address: Option<String>,
    pub viewport: SvgViewport,
    pub visible_bounds: Option<SvgBounds>,
    pub warnings: Vec<SvgWarning>,
    pub max_result_bytes: usize,
    pub metrics: SvgMetrics,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgMetrics {
    pub records: usize,
    pub operations: usize,
    pub points: usize,
    pub text_bytes: usize,
    pub image_encoded_bytes: usize,
    pub block_depth: usize,
    pub svg_elements: usize,
    pub render_work: usize,
    pub svg_bytes: usize,
    pub result_bytes: usize,
    pub bounds_work: usize,
}

#[derive(Clone, Copy, Default)]
struct Preflight {
    operations: usize,
    points: usize,
    text_bytes: usize,
    image_bytes: usize,
    work: usize,
}

#[derive(Clone, Copy)]
struct Limits {
    records: usize,
    operations: usize,
    points: usize,
    text_bytes: usize,
    image_bytes: usize,
    block_depth: usize,
    elements: usize,
    work: usize,
    svg_bytes: usize,
    result_bytes: usize,
}

pub fn render_svg(request: &NativeSvgRenderRequestA0) -> Result<SvgArtifact, SvgError> {
    validate_native_svg_render_request_contract(request).map_err(|error| {
        SvgError::new(
            SvgErrorKind::InvalidDocument,
            format!("invalid SVG request: {error}"),
        )
    })?;
    let limits = Limits::from_wire(&request.limits)?;
    // Preserve the frozen adapter's document-id/records validation order before
    // decoding the opaque compatibility payload into its typed family.
    let (_, _, compatibility_document) = document_value(&request.document)?;
    compatibility_document
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| SvgError("plot document records are missing".to_owned()))?;
    let viewport = ViewportPolicy::Explicit(SvgViewport {
        min_x_nm: request.viewport.min_x_nm.get(),
        min_y_nm: request.viewport.min_y_nm.get(),
        width_nm: request.viewport.width_nm.get(),
        height_nm: request.viewport.height_nm.get(),
    });
    let typed_limits = SvgRenderLimits {
        max_records: limits.records,
        max_operations: limits.operations,
        max_points: limits.points,
        max_text_bytes: limits.text_bytes,
        max_image_encoded_bytes: limits.image_bytes,
        max_block_depth: limits.block_depth,
        max_svg_elements: limits.elements,
        max_render_work: limits.work,
        max_svg_bytes: limits.svg_bytes,
        // The frozen native adapter enforces this after it serializes the
        // envelope so its historical error ordering/message remain exact.
        max_result_bytes: MAX_RESULT_BYTES,
        max_bounds_work: limits.work,
    };
    let context = ValidatedSvgRenderContextA1::defaults();
    let mut artifact = match &request.document {
        NativeSvgPlotDocument::FootprintSvgDocument(wrapper) => render_footprint_svg(
            &decode_compatibility_document(wrapper.value.clone(), "footprint")?,
            viewport,
            &context,
            typed_limits,
        ),
        NativeSvgPlotDocument::SymbolSvgDocument(wrapper) => render_symbol_svg(
            &decode_compatibility_document(wrapper.value.clone(), "symbol")?,
            viewport,
            &context,
            typed_limits,
        ),
        NativeSvgPlotDocument::BoardSvgDocument(wrapper) => render_board_document_svg(
            &decode_compatibility_document(wrapper.value.clone(), "board")?,
            viewport,
            &context,
            typed_limits,
        ),
        NativeSvgPlotDocument::SchematicSvgDocument(wrapper) => render_schematic_svg(
            &decode_compatibility_document(wrapper.value.clone(), "schematic")?,
            viewport,
            &context,
            typed_limits,
        ),
    }?;
    artifact.max_result_bytes = limits.result_bytes;
    Ok(artifact)
}

fn decode_compatibility_document<T: serde::de::DeserializeOwned>(
    value: Value,
    family: &str,
) -> Result<T, SvgError> {
    serde_json::from_value(value).map_err(|error| {
        SvgError::new(
            SvgErrorKind::InvalidDocument,
            format!("invalid {family} plot document: {error}"),
        )
    })
}

#[allow(
    dead_code,
    reason = "retained temporarily as an internal parity oracle"
)]
fn render_svg_legacy(request: &NativeSvgRenderRequestA0) -> Result<SvgArtifact, SvgError> {
    validate_native_svg_render_request_contract(request)
        .map_err(|error| SvgError(format!("invalid SVG request: {error}")))?;
    let limits = Limits::from_wire(&request.limits)?;
    let (source_kind, document_id, document) = document_value(&request.document)?;
    let records = document
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| SvgError("plot document records are missing".to_owned()))?;
    let preflight = preflight(records, limits)?;
    let (svg, svg_elements, serialization_work, block_depth) =
        render_document(records, &request.viewport, limits, preflight.work)?;
    let render_work = checked_add(preflight.work, serialization_work, "render work")?;
    let svg_bytes = svg.len();
    Ok(SvgArtifact {
        source_kind,
        document_id,
        svg,
        occurrence_address: None,
        viewport: SvgViewport {
            min_x_nm: request.viewport.min_x_nm.get(),
            min_y_nm: request.viewport.min_y_nm.get(),
            width_nm: request.viewport.width_nm.get(),
            height_nm: request.viewport.height_nm.get(),
        },
        visible_bounds: None,
        warnings: Vec::new(),
        max_result_bytes: limits.result_bytes,
        metrics: SvgMetrics {
            records: records.len(),
            operations: preflight.operations,
            points: preflight.points,
            text_bytes: preflight.text_bytes,
            image_encoded_bytes: preflight.image_bytes,
            block_depth,
            svg_elements,
            render_work,
            svg_bytes,
            result_bytes: 0,
            bounds_work: 0,
        },
    })
}

fn document_value(
    document: &NativeSvgPlotDocument,
) -> Result<(&'static str, String, &Value), SvgError> {
    match document {
        NativeSvgPlotDocument::FootprintSvgDocument(wrapper) => {
            document_parts("MOD", &wrapper.value)
        }
        NativeSvgPlotDocument::SymbolSvgDocument(wrapper) => document_parts("SYM", &wrapper.value),
        NativeSvgPlotDocument::BoardSvgDocument(wrapper) => document_parts("PCB", &wrapper.value),
        NativeSvgPlotDocument::SchematicSvgDocument(wrapper) => {
            document_parts("SCH", &wrapper.value)
        }
    }
}

fn document_parts<'a>(
    source_kind: &'static str,
    document: &'a Value,
) -> Result<(&'static str, String, &'a Value), SvgError> {
    let document_id = document
        .get("document_id")
        .and_then(Value::as_str)
        .ok_or_else(|| SvgError("plot document_id is missing".to_owned()))?;
    Ok((source_kind, document_id.to_owned(), document))
}

fn preflight(records: &[Value], limits: Limits) -> Result<Preflight, SvgError> {
    ensure_at_most(records.len(), limits.records, "records")?;
    let mut counts = Preflight::default();
    for record in records {
        let record_operations = record
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| SvgError("plot record operations are missing".to_owned()))?;
        counts.operations = checked_add(counts.operations, record_operations.len(), "operations")?;
        for operation in record_operations {
            count_operation_points(operation, &mut counts.points)?;
            count_payload(
                operation,
                &mut counts.points,
                &mut counts.text_bytes,
                &mut counts.image_bytes,
            )?;
        }
    }
    ensure_at_most(counts.operations, limits.operations, "operations")?;
    ensure_at_most(counts.points, limits.points, "points")?;
    ensure_at_most(counts.text_bytes, limits.text_bytes, "text bytes")?;
    ensure_at_most(counts.image_bytes, limits.image_bytes, "image bytes")?;
    counts.work = checked_add(counts.operations, counts.points, "render work")?;
    counts.work = checked_add(counts.work, counts.text_bytes, "render work")?;
    counts.work = checked_add(counts.work, counts.image_bytes, "render work")?;
    ensure_at_most(counts.work, limits.work, "render work")?;
    Ok(counts)
}

fn count_operation_points(operation: &Value, points: &mut usize) -> Result<(), SvgError> {
    let kind = operation
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| SvgError("plot operation kind is missing".to_owned()))?;
    let fixed = match kind {
        "ThickSegment" | "Rect" => 2,
        "ArcThreePoint" => 3,
        "Circle" | "Text" | "PlotImage" | "FlashPadCircle" | "FlashPadOval" | "FlashPadRect"
        | "FlashPadRoundRect" => 1,
        "BezierCurve" => 4,
        "FlashPadCustom" | "FlashPadTrapez" => 1,
        _ => 0,
    };
    *points = checked_add(*points, fixed, "points")?;
    Ok(())
}

fn count_payload(
    value: &Value,
    points: &mut usize,
    text_bytes: &mut usize,
    image_bytes: &mut usize,
) -> Result<(), SvgError> {
    match value {
        Value::Array(items) => {
            if items.len() == 2 && items.iter().all(Value::is_number) {
                *points = checked_add(*points, 1, "points")?;
            }
            for item in items {
                count_payload(item, points, text_bytes, image_bytes)?;
            }
        }
        Value::Object(object) => {
            for (key, item) in object {
                if key == "text" {
                    if let Some(text) = item.as_str() {
                        *text_bytes = checked_add(*text_bytes, text.len(), "text bytes")?;
                    }
                } else if key == "image_data_b64" {
                    if let Some(data) = item.as_str() {
                        *image_bytes = checked_add(*image_bytes, data.len(), "image bytes")?;
                    }
                } else if key == "image_data_parts"
                    && let Some(parts) = item.as_array()
                {
                    for part in parts.iter().filter_map(Value::as_str) {
                        *image_bytes = checked_add(*image_bytes, part.len(), "image bytes")?;
                    }
                }
                count_payload(item, points, text_bytes, image_bytes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn render_document(
    records: &[Value],
    viewport: &NativeSvgViewport,
    limits: Limits,
    preflight_work: usize,
) -> Result<(String, usize, usize, usize), SvgError> {
    let width = viewport.width_nm.get();
    let height = viewport.height_nm.get();
    let remaining_work = limits.work.checked_sub(preflight_work).ok_or_else(|| {
        SvgError("render work exceeds the configured limit before serialization".to_owned())
    })?;
    let mut sink = SvgSink::new(limits.svg_bytes, limits.elements, remaining_work);
    sink.raw("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    sink.element()?;
    sink.raw(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {width} {height}\">\n",
        format_mm(width),
        format_mm(height),
    ))?;
    sink.element()?;
    sink.raw(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"#FFFFFF\"/>\n"
    ))?;
    sink.element()?;
    sink.raw(&format!(
        "<g transform=\"translate({} {})\">\n",
        viewport
            .min_x_nm
            .get()
            .checked_neg()
            .ok_or_else(|| SvgError("viewport X offset overflowed".to_owned()))?,
        viewport
            .min_y_nm
            .get()
            .checked_neg()
            .ok_or_else(|| SvgError("viewport Y offset overflowed".to_owned()))?,
    ))?;
    let mut block_depth = 0usize;
    let mut maximum_block_depth = 0usize;
    for record in records {
        render_record(
            record,
            &mut sink,
            &mut block_depth,
            &mut maximum_block_depth,
            limits.block_depth,
        )?;
    }
    if block_depth != 0 {
        return Err(SvgError(
            "plot document contains an unclosed block".to_owned(),
        ));
    }
    sink.raw("</g>\n</svg>\n")?;
    let (svg, elements, work) = sink.finish()?;
    Ok((svg, elements, work, maximum_block_depth))
}

fn render_record(
    record: &Value,
    sink: &mut SvgSink,
    block_depth: &mut usize,
    maximum_block_depth: &mut usize,
    max_block_depth: usize,
) -> Result<(), SvgError> {
    let object = record
        .as_object()
        .ok_or_else(|| SvgError("plot record must be an object".to_owned()))?;
    let uuid = string_field(object, "uuid")?;
    let kind = string_field(object, "kind")?;
    let object_id = string_field(object, "object_id")?;
    sink.element()?;
    sink.raw("<g")?;
    sink.id_attribute(uuid)?;
    sink.attribute("data-ref", kind)?;
    sink.attribute("data-object-id", object_id)?;
    if kind == "footprint"
        && let Some(placement) = object.get("placement").and_then(Value::as_object)
    {
        let x = placement
            .get("x_nm")
            .and_then(Value::as_i64)
            .ok_or_else(|| SvgError("footprint placement X is missing".to_owned()))?;
        let y = placement
            .get("y_nm")
            .and_then(Value::as_i64)
            .ok_or_else(|| SvgError("footprint placement Y is missing".to_owned()))?;
        let angle = placement
            .get("angle_deg")
            .and_then(Value::as_f64)
            .ok_or_else(|| SvgError("footprint placement angle is missing".to_owned()))?;
        let mut transforms = Vec::with_capacity(2);
        if x != 0 || y != 0 {
            transforms.push(format!("translate({x} {y})"));
        }
        if angle != 0.0 {
            transforms.push(format!("rotate({})", format_number(-angle)));
        }
        if !transforms.is_empty() {
            sink.attribute("transform", &transforms.join(" "))?;
        }
    }
    sink.raw(">\n")?;
    let operations = object
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| SvgError("plot record operations must be an array".to_owned()))?;
    let record_depth = *block_depth;
    for operation in operations {
        render_operation(
            operation,
            sink,
            block_depth,
            maximum_block_depth,
            max_block_depth,
        )?;
    }
    if *block_depth != record_depth {
        return Err(SvgError(
            "plot record contains an unclosed block".to_owned(),
        ));
    }
    sink.raw("</g>\n")
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a str, SvgError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| SvgError(format!("missing string field {name}")))
}

fn format_mm(value_nm: u64) -> String {
    let whole = value_nm / 1_000_000;
    let fraction = value_nm % 1_000_000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:06}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn format_number(value: f64) -> String {
    let normalized = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    format!("{normalized:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn checked_add(left: usize, right: usize, name: &str) -> Result<usize, SvgError> {
    left.checked_add(right)
        .ok_or_else(|| SvgError(format!("{name} overflowed")))
}

fn ensure_at_most(actual: usize, maximum: usize, name: &str) -> Result<(), SvgError> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(SvgError(format!("{name} exceeds the configured limit")))
    }
}

fn parse_limit(value: &str, maximum: usize, name: &str) -> Result<usize, SvgError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| SvgError(format!("{name} is not a canonical uint64")))?;
    let narrowed = usize::try_from(parsed)
        .map_err(|_| SvgError(format!("{name} cannot be represented on this host")))?;
    Ok(narrowed.min(maximum))
}

impl Limits {
    fn from_wire(value: &NativeSvgRenderLimits) -> Result<Self, SvgError> {
        Ok(Self {
            records: (value.max_records as usize).min(MAX_RECORDS),
            operations: (value.max_operations as usize).min(MAX_OPERATIONS),
            points: parse_limit(&value.max_points, MAX_POINTS, "max_points")?,
            text_bytes: parse_limit(&value.max_text_bytes, MAX_TEXT_BYTES, "max_text_bytes")?,
            image_bytes: parse_limit(
                &value.max_image_encoded_bytes,
                MAX_IMAGE_BYTES,
                "max_image_encoded_bytes",
            )?,
            block_depth: (value.max_block_depth as usize).min(MAX_BLOCK_DEPTH),
            elements: parse_limit(&value.max_svg_elements, MAX_ELEMENTS, "max_svg_elements")?,
            work: parse_limit(&value.max_render_work, MAX_WORK, "max_render_work")?,
            svg_bytes: parse_limit(&value.max_svg_bytes, MAX_SVG_BYTES, "max_svg_bytes")?,
            result_bytes: parse_limit(
                &value.max_result_bytes,
                MAX_RESULT_BYTES,
                "max_result_bytes",
            )?,
        })
    }
}

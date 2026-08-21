//! Bounded deterministic SVG serialization over frozen Phase-5 Plotter-IR contracts.

#![forbid(unsafe_code)]

mod operation;
mod sink;

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
pub struct SvgError(pub String);

impl std::fmt::Display for SvgError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SvgError {}

#[derive(Debug)]
pub struct SvgArtifact {
    pub source_kind: &'static str,
    pub document_id: String,
    pub svg: String,
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

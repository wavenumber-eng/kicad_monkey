//! Strict promoted boundary for native process transport envelopes.

use crate::generated::board_plot_document::BoardPlotDocumentA0;
use crate::generated::footprint_plot_document::FootprintPlotDocumentA0;
use crate::generated::native_design_facts_request::NativeDesignFactsRequestA0;
use crate::generated::native_design_facts_result::NativeDesignFactsResultA0;
use crate::generated::native_error::NativeErrorA0;
use crate::generated::native_handshake::NativeHandshakeA0;
use crate::generated::native_handshake_a1::{NativeHandshakeA1, NativeHandshakeA1OperationsItem};
use crate::generated::native_svg_render_request::{
    NativeBoardSvgDocument, NativeFootprintSvgDocument, NativeSchematicSvgDocument,
    NativeSvgPlotDocument, NativeSvgRenderRequestA0, NativeSymbolSvgDocument,
};
use crate::generated::native_svg_render_result::NativeSvgRenderResultA0;
use crate::generated::schematic_plot_document::SchematicPlotDocumentA0;
use crate::generated::symbol_plot_document::SymbolPlotDocumentA0;
use crate::{
    ValidationError, validate_board_plot_document, validate_compiled_schematic_graph_contract,
    validate_footprint_plot_document, validate_schematic_plot_document,
    validate_source_bundle_manifest_contract, validate_symbol_plot_document, validation_error,
};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: &str = "a0";
const HANDSHAKE_TYPE: &str = "kicad_monkey.native.handshake";
const REQUEST_TYPE: &str = "kicad_monkey.native.design_facts.request";
const RESULT_TYPE: &str = "kicad_monkey.native.design_facts.result";
const ERROR_TYPE: &str = "kicad_monkey.native.error";
const DESIGN_FACTS_OPERATION: &str = "design-facts";
const SVG_REQUEST_TYPE: &str = "kicad_monkey.native.svg.request";
const SVG_RESULT_TYPE: &str = "kicad_monkey.native.svg.result";
const SVG_PROFILE: &str = "plotter-base-a0";
const JAVASCRIPT_SAFE_MAX: u64 = 9_007_199_254_740_991;

pub fn decode_native_handshake_a0(
    source: &[u8],
) -> Result<NativeHandshakeA0, NativeTransportDecodeError> {
    decode_and_validate(source, validate_native_handshake_contract)
}

pub fn decode_native_design_facts_request_a0(
    source: &[u8],
) -> Result<NativeDesignFactsRequestA0, NativeTransportDecodeError> {
    decode_and_validate(source, validate_native_design_facts_request_contract)
}

pub fn decode_native_design_facts_result_a0(
    source: &[u8],
) -> Result<NativeDesignFactsResultA0, NativeTransportDecodeError> {
    decode_and_validate(source, validate_native_design_facts_result_contract)
}

pub fn decode_native_error_a0(source: &[u8]) -> Result<NativeErrorA0, NativeTransportDecodeError> {
    decode_and_validate(source, validate_native_error_contract)
}

pub fn decode_native_handshake_a1(
    source: &[u8],
) -> Result<NativeHandshakeA1, NativeTransportDecodeError> {
    decode_and_validate(source, validate_native_handshake_a1_contract)
}

pub fn decode_native_svg_render_request_a0(
    source: &[u8],
) -> Result<NativeSvgRenderRequestA0, NativeTransportDecodeError> {
    let mut value: NativeSvgRenderRequestA0 =
        serde_json::from_slice(source).map_err(NativeTransportDecodeError::Transport)?;
    value.document =
        normalize_svg_document(value.document).map_err(NativeTransportDecodeError::Validation)?;
    validate_native_svg_render_request_contract(&value)
        .map_err(NativeTransportDecodeError::Validation)?;
    Ok(value)
}

fn normalize_svg_document(
    document: NativeSvgPlotDocument,
) -> Result<NativeSvgPlotDocument, ValidationError> {
    let (kind, value) = match document {
        NativeSvgPlotDocument::FootprintSvgDocument(wrapper) => (wrapper.kind, wrapper.value),
        NativeSvgPlotDocument::SymbolSvgDocument(wrapper) => (wrapper.kind, wrapper.value),
        NativeSvgPlotDocument::BoardSvgDocument(wrapper) => (wrapper.kind, wrapper.value),
        NativeSvgPlotDocument::SchematicSvgDocument(wrapper) => (wrapper.kind, wrapper.value),
    };
    match kind.as_str() {
        "footprint" => Ok(NativeFootprintSvgDocument { kind, value }.into()),
        "symbol" => Ok(NativeSymbolSvgDocument { kind, value }.into()),
        "board" => Ok(NativeBoardSvgDocument { kind, value }.into()),
        "schematic" => Ok(NativeSchematicSvgDocument { kind, value }.into()),
        _ => Err(validation_error(
            "unsupported_contract",
            "$.document.kind",
            "native SVG document kind is unsupported",
        )),
    }
}

pub fn decode_native_svg_render_result_a0(
    source: &[u8],
) -> Result<NativeSvgRenderResultA0, NativeTransportDecodeError> {
    decode_and_validate(source, validate_native_svg_render_result_contract)
}

fn decode_and_validate<T>(
    source: &[u8],
    validate: fn(&T) -> Result<(), ValidationError>,
) -> Result<T, NativeTransportDecodeError>
where
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::from_slice(source).map_err(NativeTransportDecodeError::Transport)?;
    validate(&value).map_err(NativeTransportDecodeError::Validation)?;
    Ok(value)
}

pub fn validate_native_handshake_contract(
    value: &NativeHandshakeA0,
) -> Result<(), ValidationError> {
    require_literal(&value.type_, HANDSHAKE_TYPE, "$.type")?;
    require_literal(&value.version, PROTOCOL_VERSION, "$.version")?;
    require_nonempty(&value.engine_version, "$.engine_version")?;
    require_literal(
        &value.operations[0],
        DESIGN_FACTS_OPERATION,
        "$.operations[0]",
    )
}

pub fn validate_native_handshake_a1_contract(
    value: &NativeHandshakeA1,
) -> Result<(), ValidationError> {
    require_literal(&value.type_, HANDSHAKE_TYPE, "$.type")?;
    require_literal(&value.version, "a1", "$.version")?;
    require_nonempty(&value.engine_version, "$.engine_version")?;
    if value.operations
        == [
            NativeHandshakeA1OperationsItem::DesignFacts,
            NativeHandshakeA1OperationsItem::RenderSvg,
        ]
    {
        Ok(())
    } else {
        Err(validation_error(
            "unsupported_contract",
            "$.operations",
            "native a1 operations must be design-facts then render-svg",
        ))
    }
}

pub fn validate_native_svg_render_request_contract(
    value: &NativeSvgRenderRequestA0,
) -> Result<(), ValidationError> {
    require_literal(&value.type_, SVG_REQUEST_TYPE, "$.type")?;
    require_literal(&value.version, PROTOCOL_VERSION, "$.version")?;
    require_literal(&value.profile, SVG_PROFILE, "$.profile")?;
    if value.viewport.width_nm.get() > JAVASCRIPT_SAFE_MAX
        || value.viewport.height_nm.get() > JAVASCRIPT_SAFE_MAX
    {
        return Err(validation_error(
            "invalid_viewport",
            "$.viewport",
            "native SVG viewport dimensions must remain JavaScript-safe",
        ));
    }
    for (path, encoded) in [
        ("$.limits.max_points", &*value.limits.max_points),
        ("$.limits.max_text_bytes", &*value.limits.max_text_bytes),
        (
            "$.limits.max_image_encoded_bytes",
            &*value.limits.max_image_encoded_bytes,
        ),
        ("$.limits.max_svg_elements", &*value.limits.max_svg_elements),
        ("$.limits.max_render_work", &*value.limits.max_render_work),
        ("$.limits.max_svg_bytes", &*value.limits.max_svg_bytes),
        ("$.limits.max_result_bytes", &*value.limits.max_result_bytes),
    ] {
        require_canonical_uint64(encoded, path)?;
    }
    match &value.document {
        NativeSvgPlotDocument::FootprintSvgDocument(document) => {
            require_literal(&document.kind, "footprint", "$.document.kind")?;
            let value: FootprintPlotDocumentA0 =
                decode_nested(&document.value, "$.document.value")?;
            require_nonempty(&value.document_id, "$.document.value.document_id")?;
            validate_footprint_plot_document(&value)
        }
        NativeSvgPlotDocument::SymbolSvgDocument(document) => {
            require_literal(&document.kind, "symbol", "$.document.kind")?;
            let value: SymbolPlotDocumentA0 = decode_nested(&document.value, "$.document.value")?;
            require_nonempty(&value.document_id, "$.document.value.document_id")?;
            validate_symbol_plot_document(&value)
        }
        NativeSvgPlotDocument::BoardSvgDocument(document) => {
            require_literal(&document.kind, "board", "$.document.kind")?;
            let value: BoardPlotDocumentA0 = decode_nested(&document.value, "$.document.value")?;
            require_nonempty(&value.document_id, "$.document.value.document_id")?;
            validate_board_plot_document(&value)
        }
        NativeSvgPlotDocument::SchematicSvgDocument(document) => {
            require_literal(&document.kind, "schematic", "$.document.kind")?;
            let document: SchematicPlotDocumentA0 =
                decode_nested(&document.value, "$.document.value")?;
            require_nonempty(&document.document_id, "$.document.value.document_id")?;
            validate_schematic_plot_document(&document)?;
            if value.viewport.min_x_nm.get() != 0
                || value.viewport.min_y_nm.get() != 0
                || i64::try_from(value.viewport.width_nm.get()).ok()
                    != Some(document.canvas.width_nm.get())
                || i64::try_from(value.viewport.height_nm.get()).ok()
                    != Some(document.canvas.height_nm.get())
            {
                return Err(validation_error(
                    "viewport_mismatch",
                    "$.viewport",
                    "schematic viewport must equal its zero-origin canvas",
                ));
            }
            Ok(())
        }
    }
}

fn decode_nested<T>(value: &serde_json::Value, path: &str) -> Result<T, ValidationError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value.clone()).map_err(|_| {
        validation_error(
            "invalid_document",
            path,
            "embedded plot document does not match its frozen contract",
        )
    })
}

pub fn validate_native_svg_render_result_contract(
    value: &NativeSvgRenderResultA0,
) -> Result<(), ValidationError> {
    require_literal(&value.type_, SVG_RESULT_TYPE, "$.type")?;
    require_literal(&value.version, PROTOCOL_VERSION, "$.version")?;
    require_literal(&value.profile, SVG_PROFILE, "$.profile")?;
    require_nonempty(&value.engine_version, "$.engine_version")?;
    require_nonempty(&value.document_id, "$.document_id")?;
    require_nonempty(&value.svg_utf8, "$.svg_utf8")?;
    let actual_bytes = value.svg_utf8.len() as u64;
    let declared_bytes = require_canonical_uint64(&value.svg_bytes, "$.svg_bytes")?;
    if actual_bytes != declared_bytes {
        return Err(validation_error(
            "length_mismatch",
            "$.svg_bytes",
            "SVG byte count does not match svg_utf8",
        ));
    }
    let actual_hash = hex_digest(Sha256::digest(value.svg_utf8.as_bytes()).as_slice());
    if value.svg_sha256 != actual_hash {
        return Err(validation_error(
            "hash_mismatch",
            "$.svg_sha256",
            "SVG SHA-256 does not match svg_utf8",
        ));
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub fn validate_native_design_facts_request_contract(
    value: &NativeDesignFactsRequestA0,
) -> Result<(), ValidationError> {
    require_literal(&value.type_, REQUEST_TYPE, "$.type")?;
    require_literal(&value.version, PROTOCOL_VERSION, "$.version")?;
    validate_source_bundle_manifest_contract(&value.manifest)?;
    for (path, encoded) in [
        ("$.limits.max_source_bytes", &*value.limits.max_source_bytes),
        (
            "$.limits.max_total_source_bytes",
            &*value.limits.max_total_source_bytes,
        ),
        ("$.limits.max_output_bytes", &*value.limits.max_output_bytes),
    ] {
        require_canonical_uint64(encoded, path)?;
    }
    Ok(())
}

pub fn validate_native_design_facts_result_contract(
    value: &NativeDesignFactsResultA0,
) -> Result<(), ValidationError> {
    require_literal(&value.type_, RESULT_TYPE, "$.type")?;
    require_literal(&value.version, PROTOCOL_VERSION, "$.version")?;
    require_nonempty(&value.engine_version, "$.engine_version")?;
    require_literal(&value.kicad_netlist_version, "E", "$.kicad_netlist_version")?;
    validate_compiled_schematic_graph_contract(&value.compiled_schematic_graph)
}

pub fn validate_native_error_contract(value: &NativeErrorA0) -> Result<(), ValidationError> {
    require_literal(&value.type_, ERROR_TYPE, "$.type")?;
    require_literal(&value.version, PROTOCOL_VERSION, "$.version")
}

fn require_literal(value: &str, expected: &str, path: &str) -> Result<(), ValidationError> {
    if value == expected {
        Ok(())
    } else {
        Err(validation_error(
            "unsupported_contract",
            path,
            "native transport literal does not match the a0 contract",
        ))
    }
}

fn require_nonempty(value: &str, path: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        Err(validation_error(
            "invalid_value",
            path,
            "native engine_version must be non-empty",
        ))
    } else {
        Ok(())
    }
}

fn require_canonical_uint64(value: &str, path: &str) -> Result<u64, ValidationError> {
    let digits = value == "0"
        || value
            .as_bytes()
            .first()
            .is_some_and(|first| (b'1'..=b'9').contains(first))
            && value.as_bytes().iter().all(u8::is_ascii_digit);
    if digits && value.parse::<u64>().is_ok() {
        Ok(value.parse::<u64>().expect("validated canonical u64"))
    } else {
        Err(validation_error(
            "invalid_uint64",
            path,
            "native limit must be canonical unsigned 64-bit decimal",
        ))
    }
}

#[derive(Debug)]
pub enum NativeTransportDecodeError {
    Transport(serde_json::Error),
    Validation(ValidationError),
}

impl std::fmt::Display for NativeTransportDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "native transport JSON: {error}"),
            Self::Validation(error) => write!(formatter, "native transport contract: {error}"),
        }
    }
}

impl std::error::Error for NativeTransportDecodeError {}

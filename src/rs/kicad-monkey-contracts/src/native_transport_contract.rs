//! Strict promoted boundary for native process transport envelopes.

use crate::generated::native_design_facts_request::NativeDesignFactsRequestA0;
use crate::generated::native_design_facts_result::NativeDesignFactsResultA0;
use crate::generated::native_error::NativeErrorA0;
use crate::generated::native_handshake::NativeHandshakeA0;
use crate::{
    ValidationError, validate_compiled_schematic_graph_contract,
    validate_source_bundle_manifest_contract, validation_error,
};

const PROTOCOL_VERSION: &str = "a0";
const HANDSHAKE_TYPE: &str = "kicad_monkey.native.handshake";
const REQUEST_TYPE: &str = "kicad_monkey.native.design_facts.request";
const RESULT_TYPE: &str = "kicad_monkey.native.design_facts.result";
const ERROR_TYPE: &str = "kicad_monkey.native.error";
const DESIGN_FACTS_OPERATION: &str = "design-facts";

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

fn require_canonical_uint64(value: &str, path: &str) -> Result<(), ValidationError> {
    let digits = value == "0"
        || value
            .as_bytes()
            .first()
            .is_some_and(|first| (b'1'..=b'9').contains(first))
            && value.as_bytes().iter().all(u8::is_ascii_digit);
    if digits && value.parse::<u64>().is_ok() {
        Ok(())
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

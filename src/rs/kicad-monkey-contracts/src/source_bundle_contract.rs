//! Strict promoted boundary for the generated source-bundle manifest DTO.

use crate::generated::source_bundle_manifest::SourceBundleManifestA0;
use crate::{ValidationError, validation_error};

const SCHEMA: &str = "kicad_monkey.source_bundle_manifest.a0";
const DOCUMENT_TYPE: &str = "kicad_monkey.source_bundle_manifest";
const VERSION: &str = "a0";

/// Decode JSON and enforce literals that Typify represents as strings.
pub fn decode_source_bundle_manifest_a0(
    source: &[u8],
) -> Result<SourceBundleManifestA0, SourceBundleDecodeError> {
    let manifest = serde_json::from_slice(source).map_err(SourceBundleDecodeError::Transport)?;
    validate_source_bundle_manifest_contract(&manifest)
        .map_err(SourceBundleDecodeError::Validation)?;
    Ok(manifest)
}

/// Enforce the a0 envelope before semantic path/slot validation in core.
pub fn validate_source_bundle_manifest_contract(
    manifest: &SourceBundleManifestA0,
) -> Result<(), ValidationError> {
    require_literal(&manifest.schema, SCHEMA, "$.schema")?;
    require_literal(&manifest.type_, DOCUMENT_TYPE, "$.type")?;
    require_literal(&manifest.version, VERSION, "$.version")
}

fn require_literal(value: &str, expected: &str, path: &str) -> Result<(), ValidationError> {
    if value == expected {
        Ok(())
    } else {
        Err(validation_error(
            "unsupported_contract",
            path,
            "source bundle manifest literal does not match the a0 contract",
        ))
    }
}

/// Structural JSON or strict-literal failure at the promoted boundary.
#[derive(Debug)]
pub enum SourceBundleDecodeError {
    Transport(serde_json::Error),
    Validation(ValidationError),
}

impl std::fmt::Display for SourceBundleDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "source bundle transport: {error}"),
            Self::Validation(error) => write!(formatter, "source bundle contract: {error}"),
        }
    }
}

impl std::error::Error for SourceBundleDecodeError {}

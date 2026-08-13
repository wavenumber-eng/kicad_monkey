//! Strict promoted boundary for the generated compiled-graph transport DTO.

use crate::ValidationError;
use crate::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use crate::validation_error;
use std::fmt;

const SCHEMA: &str = "kicad_monkey.compiled_schematic_graph.a0";
const DOCUMENT_TYPE: &str = "sch.compiled_schematic_graph";
const IDENTITY_NAMESPACE: &str = "sch.compiled_schematic_graph.a0";

/// Decode JSON and enforce every TypeSpec literal that Typify represents as a string.
pub fn decode_compiled_schematic_graph_a0(
    source: &[u8],
) -> Result<CompiledSchematicGraphA0, CompiledGraphDecodeError> {
    let document = serde_json::from_slice(source).map_err(CompiledGraphDecodeError::Transport)?;
    validate_compiled_schematic_graph_contract(&document)
        .map_err(CompiledGraphDecodeError::Validation)?;
    Ok(document)
}

/// Enforce the a0 envelope and row-family discriminators after structural decoding.
pub fn validate_compiled_schematic_graph_contract(
    document: &CompiledSchematicGraphA0,
) -> Result<(), ValidationError> {
    require_literal(&document.schema, SCHEMA, "$.schema")?;
    require_literal(&document.type_, DOCUMENT_TYPE, "$.type")?;
    require_literal(
        &document.identity_namespace,
        IDENTITY_NAMESPACE,
        "$.identity_namespace",
    )?;

    validate_row_types(
        &document.unit_definitions,
        "unit_definitions",
        "sch.unit_definition",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.page_definitions,
        "page_definitions",
        "sch.page_definition",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.unit_occurrences,
        "unit_occurrences",
        "sch.unit_occurrence",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.page_occurrences,
        "page_occurrences",
        "sch.page_occurrence",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.hierarchy_occurrences,
        "hierarchy_occurrences",
        "sch.hierarchy_occurrence",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.component_occurrences,
        "component_occurrences",
        "sch.component_occurrence",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.local_net_occurrences,
        "local_net_occurrences",
        "sch.local_net_occurrence",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.terminal_occurrences,
        "terminal_occurrences",
        "sch.terminal_occurrence",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.hierarchy_terminal_bindings,
        "hierarchy_terminal_bindings",
        "sch.hierarchy_terminal_binding",
        |row| &row.type_,
    )?;
    validate_row_types(
        &document.graphical_artifact_links,
        "graphical_artifact_links",
        "sch.graphical_artifact_link",
        |row| &row.type_,
    )?;
    for (index, link) in document.graphical_artifact_links.iter().enumerate() {
        require_literal(
            &link.artifact_key,
            "sch.dwg_scene",
            format!("$.graphical_artifact_links[{index}].artifact_key"),
        )?;
    }
    Ok(())
}

fn validate_row_types<T>(
    rows: &[T],
    collection: &str,
    expected: &'static str,
    row_type: impl Fn(&T) -> &str,
) -> Result<(), ValidationError> {
    for (index, row) in rows.iter().enumerate() {
        require_literal(
            row_type(row),
            expected,
            format!("$.{collection}[{index}].type"),
        )?;
    }
    Ok(())
}

fn require_literal(
    actual: &str,
    expected: &'static str,
    path: impl Into<String>,
) -> Result<(), ValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(validation_error(
            "unsupported_contract",
            path,
            "value does not match the registered a0 contract literal",
        ))
    }
}

/// Failure from either strict JSON transport decoding or literal validation.
#[derive(Debug)]
pub enum CompiledGraphDecodeError {
    Transport(serde_json::Error),
    Validation(ValidationError),
}

impl fmt::Display for CompiledGraphDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "compiled graph transport: {error}"),
            Self::Validation(error) => write!(formatter, "compiled graph validation: {error}"),
        }
    }
}

impl std::error::Error for CompiledGraphDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Validation(error) => Some(error),
        }
    }
}

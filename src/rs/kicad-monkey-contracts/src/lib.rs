//! Generated transport types for KiCad Monkey operation boundaries.

#![forbid(unsafe_code)]
#![allow(clippy::derivable_impls)] // Typify emits explicit schema defaults.

pub mod generated;

use generated::build_request::{Node, NodeKind, SExpressionBuildRequestA0};
use std::fmt;

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

fn validation_error(
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

//! Rust core for KiCad Monkey.
//!
//! The first implementation slice is the byte-oriented KiCad S-expression
//! reader/writer foundation. Higher-level typed source models and interchange
//! operations are layered on this crate after parser parity is established.

#![forbid(unsafe_code)]

pub mod sexpr;
pub mod sexpr_mutation;
pub mod sexpr_projection;

pub use sexpr::{
    Error, ErrorKind, ErrorPhase, FormatOptions, Lexer, Limits, Patch, Position, Sexp, Token,
    TokenKind, apply_patches, apply_patches_with_limit, build, format, lex, parse, parse_bytes,
    parse_with_limits,
};
pub use sexpr_mutation::{
    Walk, find_path, remove_all_elements, remove_element, replace_element, set_value,
    transform_descendants, walk,
};
pub use sexpr_projection::{
    FormSpan, ProjectionLimits, Selector, StructuralIndex, parse_form, read_form_bytes,
    scan_form_spans, scan_form_spans_with_limits, scan_reader_form_spans,
};
#[cfg(feature = "measurement")]
#[doc(hidden)]
pub use sexpr_projection::{measure_form_span_sort, measure_reader_form_span_sort};

//! Owned schematic source with transactional, source-preserving mutations.

use super::{
    SchematicBundleLimits, SchematicDefinition, parse_schematic_definition_text, schematic_error,
    schematic_limit,
};
use crate::sexpr::{
    Lexer, Patch, Sexp, Token, TokenKind, apply_patches_with_limit, build_with_limit,
    decode_quoted_with_limit,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::{SourceBundleError, SourceBundleErrorKind};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::ops::Range;

const DEFAULT_SOURCE_PATH: &str = "document.kicad_sch";

/// Resource ceilings for an owned schematic document and its writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicDocumentLimits {
    pub parse: SchematicBundleLimits,
    pub max_output_bytes: usize,
}

impl Default for SchematicDocumentLimits {
    fn default() -> Self {
        let parse = SchematicBundleLimits::default();
        Self {
            max_output_bytes: parse.max_source_bytes,
            parse,
        }
    }
}

/// One owned KiCad schematic source.
///
/// Unknown forms remain in the exact source buffer. Typed state is rebuilt on
/// demand, avoiding a self-referential model and ensuring that every mutation
/// is semantically reparsed before it replaces the current source.
#[derive(Clone, Debug)]
pub struct SchematicDocument {
    source_path: String,
    source: String,
    limits: SchematicDocumentLimits,
}

impl SchematicDocument {
    /// Validate and own one UTF-8 `kicad_sch` source using a virtual filename.
    pub fn parse(
        source: String,
        limits: SchematicDocumentLimits,
    ) -> Result<Self, SourceBundleError> {
        Self::parse_named(DEFAULT_SOURCE_PATH, source, limits)
    }

    /// Validate and own one UTF-8 `kicad_sch` source with its portable path.
    pub fn parse_named(
        source_path: impl Into<String>,
        source: String,
        limits: SchematicDocumentLimits,
    ) -> Result<Self, SourceBundleError> {
        let source_path = source_path.into();
        validate_path(&source_path, limits.parse.max_path_bytes)?;
        parse_schematic_definition_text(&source, &source_path, limits.parse)?;
        Ok(Self {
            source_path,
            source,
            limits,
        })
    }

    /// Read at most the configured source ceiling plus one sentinel byte.
    pub fn from_reader(
        reader: impl Read,
        limits: SchematicDocumentLimits,
    ) -> Result<Self, SourceBundleError> {
        Self::from_named_reader(DEFAULT_SOURCE_PATH, reader, limits)
    }

    /// Read and validate one named schematic without an unbounded input copy.
    pub fn from_named_reader(
        source_path: impl Into<String>,
        mut reader: impl Read,
        limits: SchematicDocumentLimits,
    ) -> Result<Self, SourceBundleError> {
        let source_path = source_path.into();
        validate_path(&source_path, limits.parse.max_path_bytes)?;
        let read_limit = limits
            .parse
            .max_source_bytes
            .checked_add(1)
            .ok_or_else(|| schematic_limit(&source_path, "source byte limit overflows"))?;
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(read_limit as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error(&source_path, error))?;
        if bytes.len() > limits.parse.max_source_bytes {
            return Err(schematic_limit(
                &source_path,
                "schematic source exceeds max_source_bytes",
            ));
        }
        let source = String::from_utf8(bytes).map_err(|error| {
            SourceBundleError::new(
                SourceBundleErrorKind::Utf8,
                Some(&source_path),
                format!(
                    "source is not UTF-8 at byte {}",
                    error.utf8_error().valid_up_to()
                ),
            )
        })?;
        Self::parse_named(source_path, source, limits)
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn into_source(self) -> String {
        self.source
    }

    pub fn limits(&self) -> SchematicDocumentLimits {
        self.limits
    }

    /// Rebuild the promoted typed model over the current exact source.
    pub fn definition(&self) -> Result<SchematicDefinition, SourceBundleError> {
        parse_schematic_definition_text(&self.source, &self.source_path, self.limits.parse)
    }

    /// Write the current source verbatim after checking the output ceiling.
    pub fn write_to(&self, mut writer: impl Write) -> Result<(), SourceBundleError> {
        self.check_output_source()?;
        writer
            .write_all(self.source.as_bytes())
            .map_err(|error| io_error(&self.source_path, error))
    }

    /// Replace one existing property on one unambiguous placed symbol UUID.
    pub fn set_symbol_property(
        &mut self,
        symbol_uuid: &str,
        name: &str,
        value: &str,
    ) -> Result<bool, SourceBundleError> {
        let index = self.mutation_index()?;
        let symbol = index.unique_symbol(symbol_uuid, &self.source_path)?;
        let property = unique_property(symbol, name, &self.source_path)?.ok_or_else(|| {
            schematic_source_error(&self.source_path, "schematic symbol property was not found")
        })?;
        if property.value == value {
            return Ok(false);
        }
        let replacement = quoted(value, self.limits.max_output_bytes, &self.source_path)?;
        self.commit_patch(Patch::new(
            property.value_range.start,
            property.value_range.end,
            replacement,
        ))
    }

    /// Update or insert one property on one unambiguous placed symbol UUID.
    pub fn upsert_symbol_property(
        &mut self,
        symbol_uuid: &str,
        name: &str,
        value: &str,
    ) -> Result<bool, SourceBundleError> {
        let index = self.mutation_index()?;
        let symbol = index.unique_symbol(symbol_uuid, &self.source_path)?;
        if unique_property(symbol, name, &self.source_path)?.is_some() {
            return self.set_symbol_property(symbol_uuid, name, value);
        }
        if symbol.properties.len() >= self.limits.parse.max_symbol_properties_per_symbol {
            return Err(schematic_limit(
                &self.source_path,
                "symbol property count exceeds its limit",
            ));
        }
        let form =
            inserted_property_form(name, value, self.limits.max_output_bytes, &self.source_path)?;
        let (offset, replacement) = insertion(&self.source, &symbol.span, &form);
        self.commit_patch(Patch::new(offset, offset, replacement))
    }

    /// Remove one property from one unambiguous placed symbol UUID.
    pub fn remove_symbol_property(
        &mut self,
        symbol_uuid: &str,
        name: &str,
    ) -> Result<bool, SourceBundleError> {
        self.check_output_source()?;
        let index = self.mutation_index()?;
        let symbol = index.unique_symbol(symbol_uuid, &self.source_path)?;
        let Some(property) = unique_property(symbol, name, &self.source_path)? else {
            return Ok(false);
        };
        self.commit_patch(Patch::new(
            property.source_range.start,
            property.source_range.end,
            "",
        ))
    }

    fn mutation_index(&self) -> Result<MutationIndex, SourceBundleError> {
        self.check_output_source()?;
        MutationIndex::build(&self.source, &self.source_path, self.limits.parse)
    }

    fn check_output_source(&self) -> Result<(), SourceBundleError> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(schematic_limit(
                &self.source_path,
                "schematic output exceeds max_output_bytes",
            ));
        }
        Ok(())
    }

    fn commit_patch(&mut self, patch: Patch<'_>) -> Result<bool, SourceBundleError> {
        let candidate =
            apply_patches_with_limit(&self.source, &[patch], self.limits.max_output_bytes)
                .map_err(|error| schematic_error(&self.source_path, error))?;
        parse_schematic_definition_text(&candidate, &self.source_path, self.limits.parse)?;
        self.source = candidate;
        Ok(true)
    }
}

#[derive(Debug)]
struct MutationIndex {
    symbols: Vec<SymbolRecord>,
}

impl MutationIndex {
    fn build(
        source: &str,
        source_path: &str,
        limits: SchematicBundleLimits,
    ) -> Result<Self, SourceBundleError> {
        let paths = [
            vec!["kicad_sch".to_owned(), "symbol".to_owned()],
            vec![
                "kicad_sch".to_owned(),
                "symbol".to_owned(),
                "uuid".to_owned(),
            ],
            vec![
                "kicad_sch".to_owned(),
                "symbol".to_owned(),
                "property".to_owned(),
            ],
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let spans = scan_form_spans_with_limits(
            source,
            &Selector {
                paths: Some(paths),
                min_depth: Some(1),
                max_depth: Some(2),
                ..Selector::default()
            },
            ProjectionLimits {
                max_source_bytes: limits.max_source_bytes,
                max_depth: limits.max_depth,
                max_selected_forms: limits.max_selected_forms_per_source,
                ..ProjectionLimits::default()
            },
        )
        .map_err(|error| schematic_error(source_path, error))?;
        let mut symbols: Vec<SymbolRecord> = Vec::new();
        for span in spans {
            if span.depth == 1 {
                symbols.push(SymbolRecord {
                    span,
                    uuid: String::new(),
                    uuid_seen: false,
                    properties: Vec::new(),
                });
                continue;
            }
            let symbol = symbols.last_mut().ok_or_else(|| {
                schematic_source_error(source_path, "symbol child has no owning placed symbol")
            })?;
            if span.range.end >= symbol.span.range.end {
                return Err(schematic_source_error(
                    source_path,
                    "symbol child falls outside its owning placed symbol",
                ));
            }
            match span.head.as_deref() {
                Some("uuid") => {
                    let value = scalar_form(source, &span, source_path, limits)?;
                    if symbol.uuid_seen {
                        return Err(schematic_source_error(
                            source_path,
                            "placed symbol has ambiguous uuid forms",
                        ));
                    }
                    symbol.uuid = value;
                    symbol.uuid_seen = true;
                }
                Some("property") => {
                    symbol
                        .properties
                        .push(property_form(source, &span, source_path, limits)?)
                }
                _ => {}
            }
        }
        Ok(Self { symbols })
    }

    fn unique_symbol(
        &self,
        symbol_uuid: &str,
        source_path: &str,
    ) -> Result<&SymbolRecord, SourceBundleError> {
        if symbol_uuid.is_empty() {
            return Err(schematic_source_error(
                source_path,
                "schematic symbol UUID cannot be empty",
            ));
        }
        let mut matches = self
            .symbols
            .iter()
            .filter(|symbol| symbol.uuid == symbol_uuid);
        let symbol = matches.next().ok_or_else(|| {
            schematic_source_error(source_path, "schematic symbol UUID was not found")
        })?;
        if matches.next().is_some() {
            return Err(schematic_source_error(
                source_path,
                "schematic symbol UUID is ambiguous",
            ));
        }
        Ok(symbol)
    }
}

#[derive(Debug)]
struct SymbolRecord {
    span: FormSpan,
    uuid: String,
    uuid_seen: bool,
    properties: Vec<PropertyRecord>,
}

#[derive(Debug)]
struct PropertyRecord {
    name: String,
    value: String,
    source_range: Range<usize>,
    value_range: Range<usize>,
}

fn unique_property<'a>(
    symbol: &'a SymbolRecord,
    name: &str,
    source_path: &str,
) -> Result<Option<&'a PropertyRecord>, SourceBundleError> {
    let mut matches = symbol
        .properties
        .iter()
        .filter(|property| property.name == name);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(schematic_source_error(
            source_path,
            "schematic symbol property name is ambiguous",
        ));
    }
    Ok(first)
}

fn scalar_form(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<String, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| schematic_error(source_path, error))?;
    let mut lexer = Lexer::new(text);
    expect(&mut lexer, TokenKind::Left, source_path)?;
    next_scalar(&mut lexer, source_path, limits.max_decoded_string_bytes)?;
    next_scalar(&mut lexer, source_path, limits.max_decoded_string_bytes)
}

fn property_form(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<PropertyRecord, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| schematic_error(source_path, error))?;
    let mut lexer = Lexer::new(text);
    expect(&mut lexer, TokenKind::Left, source_path)?;
    next_scalar(&mut lexer, source_path, limits.max_decoded_string_bytes)?;
    let name = next_scalar_token(&mut lexer, source_path)?;
    let value = next_scalar_token(&mut lexer, source_path)?;
    Ok(PropertyRecord {
        name: decode(name.clone(), limits.max_decoded_string_bytes, source_path)?,
        value: decode(value.clone(), limits.max_decoded_string_bytes, source_path)?,
        source_range: span.range.clone(),
        value_range: (span.range.start + value.position.offset)
            ..(span.range.start + value.position.offset + value.lexeme.len()),
    })
}

fn next_scalar(
    lexer: &mut Lexer<'_>,
    source_path: &str,
    max_bytes: usize,
) -> Result<String, SourceBundleError> {
    let token = next_scalar_token(lexer, source_path)?;
    decode(token, max_bytes, source_path)
}

fn next_scalar_token<'a>(
    lexer: &mut Lexer<'a>,
    source_path: &str,
) -> Result<Token<'a>, SourceBundleError> {
    let token = lexer
        .next()
        .transpose()
        .map_err(|error| schematic_error(source_path, error))?
        .ok_or_else(|| schematic_source_error(source_path, "expected scalar value"))?;
    if matches!(token.kind, TokenKind::Left | TokenKind::Right) {
        return Err(schematic_source_error(source_path, "expected scalar value"));
    }
    Ok(token)
}

fn expect(
    lexer: &mut Lexer<'_>,
    expected: TokenKind,
    source_path: &str,
) -> Result<(), SourceBundleError> {
    let token = lexer
        .next()
        .transpose()
        .map_err(|error| schematic_error(source_path, error))?
        .ok_or_else(|| schematic_source_error(source_path, "expected schematic form"))?;
    if token.kind != expected {
        return Err(schematic_source_error(
            source_path,
            "expected schematic form",
        ));
    }
    Ok(())
}

fn decode(
    token: Token<'_>,
    max_bytes: usize,
    source_path: &str,
) -> Result<String, SourceBundleError> {
    if token.kind != TokenKind::QuotedString {
        if token.lexeme.len() > max_bytes {
            return Err(schematic_limit(
                source_path,
                "decoded scalar exceeds max_decoded_string_bytes",
            ));
        }
        return Ok(token.lexeme.to_owned());
    }
    decode_quoted_with_limit(token.lexeme, max_bytes).ok_or_else(|| {
        schematic_limit(
            source_path,
            "decoded scalar exceeds max_decoded_string_bytes",
        )
    })
}

fn quoted(
    value: &str,
    max_output_bytes: usize,
    source_path: &str,
) -> Result<String, SourceBundleError> {
    build_with_limit(&Sexp::Quoted(value.to_owned()), max_output_bytes)
        .map_err(|error| schematic_error(source_path, error))
}

fn inserted_property_form(
    name: &str,
    value: &str,
    max_output_bytes: usize,
    source_path: &str,
) -> Result<String, SourceBundleError> {
    let name = quoted(name, max_output_bytes, source_path)?;
    let value = quoted(value, max_output_bytes, source_path)?;
    let required = "(property   (at 0 0 0))"
        .len()
        .checked_add(name.len())
        .and_then(|bytes| bytes.checked_add(value.len()))
        .ok_or_else(|| schematic_limit(source_path, "inserted property byte count overflows"))?;
    if required > max_output_bytes {
        return Err(schematic_limit(
            source_path,
            "schematic output exceeds max_output_bytes",
        ));
    }
    let mut form = String::with_capacity(required);
    form.push_str("(property ");
    form.push_str(&name);
    form.push(' ');
    form.push_str(&value);
    form.push_str(" (at 0 0 0))");
    debug_assert_eq!(form.len(), required);
    Ok(form)
}

fn insertion(source: &str, symbol: &FormSpan, form: &str) -> (usize, String) {
    let close = symbol.range.end.saturating_sub(1);
    let line_start = source[..close].rfind('\n').map_or(0, |offset| offset + 1);
    let close_prefix = &source[line_start..close];
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let symbol_line_start = source[..symbol.range.start]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let symbol_indent = source[symbol_line_start..symbol.range.start]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect::<String>();
    let child_indent = format!("{symbol_indent}  ");
    if close_prefix.trim().is_empty() {
        (line_start, format!("{child_indent}{form}{newline}"))
    } else {
        (
            close,
            format!("{newline}{child_indent}{form}{newline}{symbol_indent}"),
        )
    }
}

fn validate_path(path: &str, max_bytes: usize) -> Result<(), SourceBundleError> {
    if path.is_empty() || path.len() > max_bytes {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Path,
            Some(path),
            "schematic source path is empty or exceeds max_path_bytes",
        ));
    }
    Ok(())
}

fn schematic_source_error(path: &str, message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Schematic, Some(path), message)
}

fn io_error(path: &str, error: std::io::Error) -> SourceBundleError {
    schematic_source_error(path, format!("schematic source I/O failed: {error}"))
}

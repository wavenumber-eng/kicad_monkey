//! Typed, source-backed views and focused edits for standalone footprints.

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Patch, Position, Sexp, Token, TokenKind,
    apply_patches_with_limit, build_with_limit, decode_quoted,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ops::Range;

/// Resource ceilings for one typed standalone-footprint read or edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FootprintLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_properties: usize,
    pub max_pads: usize,
}

impl Default for FootprintLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_properties: 4096,
            max_pads: 100_000,
        }
    }
}

/// One lazily decoded top-level property backed by the original source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintProperty<'a> {
    pub name: Cow<'a, str>,
    pub value: Cow<'a, str>,
    value_range: Range<usize>,
}

/// A typed standalone-footprint view that retains only selected source spans.
#[derive(Clone, Debug)]
pub struct FootprintView<'a> {
    source: &'a str,
    root: FormSpan,
    properties: Vec<FormSpan>,
    pad_count: usize,
}

impl<'a> FootprintView<'a> {
    /// Validate and index the top-level footprint, properties, and pads.
    pub fn parse(source: &'a str, limits: FootprintLimits) -> Result<Self, Error> {
        let child_limit = limits
            .max_properties
            .checked_add(limits.max_pads)
            .ok_or_else(limit_error)?;
        let projection_limits = |max_selected_forms| ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms,
            ..ProjectionLimits::default()
        };
        let root_selector = Selector {
            min_depth: Some(0),
            max_depth: Some(0),
            ..Selector::default()
        };
        let roots = scan_form_spans_with_limits(source, &root_selector, projection_limits(2))?;
        let [root] = roots.try_into().map_err(|_| {
            source_error(
                "Expected exactly one top-level footprint form",
                Position::START,
            )
        })?;
        if root.head.as_deref() != Some("footprint") {
            return Err(source_error("Expected a footprint root", root.start));
        }

        let child_selector = Selector {
            paths: Some(BTreeSet::from([
                vec!["footprint".to_owned(), "property".to_owned()],
                vec!["footprint".to_owned(), "pad".to_owned()],
            ])),
            min_depth: Some(1),
            max_depth: Some(1),
            ..Selector::default()
        };
        let spans =
            scan_form_spans_with_limits(source, &child_selector, projection_limits(child_limit))?;
        let mut properties = Vec::new();
        let mut pad_count = 0usize;
        for span in spans {
            match (span.depth, span.head.as_deref()) {
                (1, Some("property")) => properties.push(span),
                (1, Some("pad")) => pad_count = pad_count.saturating_add(1),
                _ => {}
            }
        }
        if properties.len() > limits.max_properties || pad_count > limits.max_pads {
            return Err(limit_error());
        }
        Ok(Self {
            source,
            root,
            properties,
            pad_count,
        })
    }

    /// Decode the footprint name without materializing its child forms.
    pub fn name(&self) -> Result<Cow<'a, str>, Error> {
        let text = self.root.text(self.source)?;
        (|| {
            let mut lexer = Lexer::new(text);
            expect_kind(
                lexer.next(),
                TokenKind::Left,
                "Expected footprint opening parenthesis",
            )?;
            expect_atom(lexer.next(), "footprint", "Expected footprint root")?;
            let token = next_value(lexer.next(), "Expected footprint name")?;
            Ok(decoded(token))
        })()
        .map_err(|error| rebase_error(error, &self.root))
    }

    pub fn pad_count(&self) -> usize {
        self.pad_count
    }

    /// Decode properties one at a time from their selected source ranges.
    pub fn properties(&self) -> impl Iterator<Item = Result<FootprintProperty<'a>, Error>> + '_ {
        self.properties
            .iter()
            .map(|span| property_from_span(self.source, span))
    }

    /// Replace an existing top-level property value and preserve every other byte.
    pub fn set_property(
        &self,
        name: &str,
        value: &str,
        max_output_bytes: usize,
    ) -> Result<FootprintEdit, Error> {
        if self.source.len() > max_output_bytes {
            return Err(Error::build(
                ErrorKind::ResourceLimit,
                "Footprint output exceeds max_output_bytes",
            ));
        }
        let mut matching = self.properties().filter_map(|property| match property {
            Ok(property) if property.name == name => Some(Ok(property)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        });
        let property = matching
            .next()
            .transpose()?
            .ok_or_else(|| source_error("Footprint property was not found", self.root.start))?;
        if matching.next().transpose()?.is_some() {
            return Err(source_error(
                "Footprint property name is ambiguous",
                self.root.start,
            ));
        }
        if property.value == value {
            return Ok(FootprintEdit {
                source: self.source.to_owned(),
                changed: false,
            });
        }
        let replacement = build_with_limit(&Sexp::Quoted(value.to_owned()), max_output_bytes)?;
        let source = apply_patches_with_limit(
            self.source,
            &[Patch::new(
                property.value_range.start,
                property.value_range.end,
                replacement,
            )],
            max_output_bytes,
        )?;
        Ok(FootprintEdit {
            source,
            changed: true,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintEdit {
    pub source: String,
    pub changed: bool,
}

fn property_from_span<'a>(
    source: &'a str,
    span: &FormSpan,
) -> Result<FootprintProperty<'a>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected property opening parenthesis",
        )?;
        expect_atom(lexer.next(), "property", "Expected property form")?;
        let name = next_value(lexer.next(), "Expected property name")?;
        let value = next_value(lexer.next(), "Expected property value")?;
        Ok(FootprintProperty {
            name: decoded(name),
            value: decoded(value.clone()),
            value_range: (span.range.start + value.position.offset)
                ..(span.range.start + value.position.offset + value.lexeme.len()),
        })
    })()
    .map_err(|error| rebase_error(error, span))
}

fn rebase_error(mut error: Error, span: &FormSpan) -> Error {
    if let Some(position) = error.position {
        error.position = Some(Position {
            offset: span.range.start.saturating_add(position.offset),
            line: span
                .start
                .line
                .saturating_add(position.line.saturating_sub(1)),
            column: if position.line == 1 {
                span.start
                    .column
                    .saturating_add(position.column.saturating_sub(1))
            } else {
                position.column
            },
        });
    }
    error
}

fn decoded(token: Token<'_>) -> Cow<'_, str> {
    if token.kind == TokenKind::QuotedString {
        Cow::Owned(decode_quoted(token.lexeme))
    } else {
        Cow::Borrowed(token.lexeme)
    }
}

fn next_value<'a>(
    token: Option<Result<Token<'a>, Error>>,
    message: &'static str,
) -> Result<Token<'a>, Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if matches!(token.kind, TokenKind::Left | TokenKind::Right) {
        return Err(source_error(message, token.position));
    }
    Ok(token)
}

fn expect_kind(
    token: Option<Result<Token<'_>, Error>>,
    kind: TokenKind,
    message: &'static str,
) -> Result<(), Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if token.kind != kind {
        return Err(source_error(message, token.position));
    }
    Ok(())
}

fn expect_atom(
    token: Option<Result<Token<'_>, Error>>,
    atom: &str,
    message: &'static str,
) -> Result<(), Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if token.kind != TokenKind::Atom || token.lexeme != atom {
        return Err(source_error(message, token.position));
    }
    Ok(())
}

fn source_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Footprint typed read exceeds configured limits",
        Position::START,
    )
}

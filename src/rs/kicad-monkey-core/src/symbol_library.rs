//! Typed, source-backed reads and focused writes for KiCad symbol libraries.

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Patch, Position, Token, TokenKind,
    apply_patches_with_limit, decode_quoted,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ops::Range;

/// Resource ceilings for one symbol-library read or edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolLibraryLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_symbols: usize,
    pub max_metadata_forms: usize,
    pub max_subsymbols: usize,
    pub max_pins: usize,
}

impl Default for SymbolLibraryLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_symbols: 100_000,
            max_metadata_forms: 1_000_000,
            max_subsymbols: 1_000_000,
            max_pins: 4_000_000,
        }
    }
}

/// Supported focused boolean writes on a top-level library symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolBooleanField {
    InBom,
    OnBoard,
}

impl SymbolBooleanField {
    fn head(self) -> &'static str {
        match self {
            Self::InBom => "in_bom",
            Self::OnBoard => "on_board",
        }
    }
}

/// One lazily decoded top-level symbol summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolSummary<'a> {
    pub name: Cow<'a, str>,
    pub extends: Option<Cow<'a, str>>,
    pub in_bom: bool,
    pub on_board: bool,
    pub power: bool,
    pub power_kind: Option<Cow<'a, str>>,
    pub property_count: usize,
    pub subsymbol_count: usize,
    pub pin_count: usize,
}

#[derive(Clone, Debug)]
struct SymbolIndex {
    root: FormSpan,
    extends: Option<FormSpan>,
    in_bom: Option<FormSpan>,
    in_bom_ambiguous: bool,
    on_board: Option<FormSpan>,
    on_board_ambiguous: bool,
    power: Option<FormSpan>,
    property_count: usize,
    subsymbol_count: usize,
    pin_count: usize,
}

/// A symbol-library view retaining only selected source spans and counts.
#[derive(Clone, Debug)]
pub struct SymbolLibraryView<'a> {
    source: &'a str,
    symbols: Vec<SymbolIndex>,
}

impl<'a> SymbolLibraryView<'a> {
    /// Validate one library root and index summaries without a generic tree.
    pub fn parse(source: &'a str, limits: SymbolLibraryLimits) -> Result<Self, Error> {
        validate_root(source, limits)?;
        let selected_limit = selected_limit(limits)?;
        let selector = Selector {
            paths: Some(selected_paths()),
            min_depth: Some(1),
            max_depth: Some(3),
            ..Selector::default()
        };
        let spans = scan_form_spans_with_limits(
            source,
            &selector,
            ProjectionLimits {
                max_source_bytes: limits.max_source_bytes,
                max_depth: limits.max_depth,
                max_selected_forms: selected_limit,
                ..ProjectionLimits::default()
            },
        )?;
        let mut symbols = spans
            .iter()
            .filter(|span| span.depth == 1 && span.head.as_deref() == Some("symbol"))
            .cloned()
            .map(SymbolIndex::new)
            .collect::<Vec<_>>();
        if symbols.len() > limits.max_symbols {
            return Err(limit_error("Symbol library symbol limit exceeded"));
        }
        associate_children(&spans, &mut symbols, limits)?;
        Ok(Self { source, symbols })
    }

    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Decode symbol summaries in source order, one entry at a time.
    pub fn symbols(&self) -> impl Iterator<Item = Result<SymbolSummary<'a>, Error>> + '_ {
        self.symbols
            .iter()
            .map(|index| summary_from_index(self.source, index))
    }

    /// Replace or insert one supported symbol flag while preserving all other bytes.
    pub fn set_boolean(
        &self,
        symbol_name: &str,
        field: SymbolBooleanField,
        value: bool,
        max_output_bytes: usize,
    ) -> Result<SymbolLibraryEdit, Error> {
        if self.source.len() > max_output_bytes {
            return Err(build_limit_error());
        }
        let index = self.unique_symbol(symbol_name)?;
        let form = match field {
            SymbolBooleanField::InBom => index.in_bom.as_ref(),
            SymbolBooleanField::OnBoard => index.on_board.as_ref(),
        };
        let ambiguous = match field {
            SymbolBooleanField::InBom => index.in_bom_ambiguous,
            SymbolBooleanField::OnBoard => index.on_board_ambiguous,
        };
        if ambiguous {
            return Err(source_error(
                "Symbol boolean field is ambiguous",
                index.root.start,
            ));
        }
        edit_boolean(self.source, index, form, field, value, max_output_bytes)
    }

    fn unique_symbol(&self, name: &str) -> Result<&SymbolIndex, Error> {
        let mut matching =
            self.symbols
                .iter()
                .filter_map(|index| match header_value(self.source, &index.root) {
                    Ok(value) if value == name => Some(Ok(index)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                });
        let index = matching
            .next()
            .transpose()?
            .ok_or_else(|| source_error("Symbol was not found", Position::START))?;
        if matching.next().transpose()?.is_some() {
            return Err(source_error("Symbol name is ambiguous", index.root.start));
        }
        Ok(index)
    }
}

/// Result of a source-preserving symbol-library edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolLibraryEdit {
    pub source: String,
    pub changed: bool,
}

impl SymbolIndex {
    fn new(root: FormSpan) -> Self {
        Self {
            root,
            extends: None,
            in_bom: None,
            in_bom_ambiguous: false,
            on_board: None,
            on_board_ambiguous: false,
            power: None,
            property_count: 0,
            subsymbol_count: 0,
            pin_count: 0,
        }
    }
}

fn validate_root(source: &str, limits: SymbolLibraryLimits) -> Result<(), Error> {
    let selector = Selector {
        min_depth: Some(0),
        max_depth: Some(0),
        ..Selector::default()
    };
    let roots = scan_form_spans_with_limits(
        source,
        &selector,
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: 2,
            ..ProjectionLimits::default()
        },
    )?;
    let [root] = roots.as_slice() else {
        return Err(source_error(
            "Expected exactly one top-level symbol-library form",
            Position::START,
        ));
    };
    if root.head.as_deref() != Some("kicad_symbol_lib") {
        return Err(source_error("Expected a kicad_symbol_lib root", root.start));
    }
    Ok(())
}

fn selected_paths() -> BTreeSet<Vec<String>> {
    [
        &["kicad_symbol_lib", "symbol"][..],
        &["kicad_symbol_lib", "symbol", "extends"],
        &["kicad_symbol_lib", "symbol", "in_bom"],
        &["kicad_symbol_lib", "symbol", "on_board"],
        &["kicad_symbol_lib", "symbol", "power"],
        &["kicad_symbol_lib", "symbol", "property"],
        &["kicad_symbol_lib", "symbol", "symbol"],
        &["kicad_symbol_lib", "symbol", "symbol", "pin"],
    ]
    .into_iter()
    .map(|path| path.iter().map(|value| (*value).to_owned()).collect())
    .collect()
}

fn selected_limit(limits: SymbolLibraryLimits) -> Result<usize, Error> {
    limits
        .max_symbols
        .checked_add(limits.max_metadata_forms)
        .and_then(|value| value.checked_add(limits.max_subsymbols))
        .and_then(|value| value.checked_add(limits.max_pins))
        .ok_or_else(|| limit_error("Symbol library selected-form limit overflow"))
}

fn associate_children(
    spans: &[FormSpan],
    symbols: &mut [SymbolIndex],
    limits: SymbolLibraryLimits,
) -> Result<(), Error> {
    let mut current = 0usize;
    let mut metadata_count = 0usize;
    let mut subsymbol_count = 0usize;
    let mut pin_count = 0usize;
    for span in spans.iter().filter(|span| span.depth > 1) {
        while current + 1 < symbols.len()
            && span.range.start >= symbols[current + 1].root.range.start
        {
            current += 1;
        }
        let Some(symbol) = symbols.get_mut(current) else {
            return Err(source_error("Orphaned symbol child form", span.start));
        };
        if !symbol.root.range.contains(&span.range.start) {
            return Err(source_error(
                "Symbol child lies outside its parent",
                span.start,
            ));
        }
        classify_child(
            symbol,
            span,
            &mut metadata_count,
            &mut subsymbol_count,
            &mut pin_count,
        );
    }
    if metadata_count > limits.max_metadata_forms
        || subsymbol_count > limits.max_subsymbols
        || pin_count > limits.max_pins
    {
        return Err(limit_error(
            "Symbol library typed read exceeds configured limits",
        ));
    }
    Ok(())
}

fn classify_child(
    symbol: &mut SymbolIndex,
    span: &FormSpan,
    metadata_count: &mut usize,
    subsymbol_count: &mut usize,
    pin_count: &mut usize,
) {
    match (span.depth, span.head.as_deref()) {
        (2, Some("extends")) => set_first(&mut symbol.extends, span, metadata_count),
        (2, Some("in_bom")) => set_unique(
            &mut symbol.in_bom,
            &mut symbol.in_bom_ambiguous,
            span,
            metadata_count,
        ),
        (2, Some("on_board")) => set_unique(
            &mut symbol.on_board,
            &mut symbol.on_board_ambiguous,
            span,
            metadata_count,
        ),
        (2, Some("power")) => set_first(&mut symbol.power, span, metadata_count),
        (2, Some("property")) => {
            symbol.property_count = symbol.property_count.saturating_add(1);
            *metadata_count = metadata_count.saturating_add(1);
        }
        (2, Some("symbol")) => {
            symbol.subsymbol_count = symbol.subsymbol_count.saturating_add(1);
            *subsymbol_count = subsymbol_count.saturating_add(1);
        }
        (3, Some("pin")) => {
            symbol.pin_count = symbol.pin_count.saturating_add(1);
            *pin_count = pin_count.saturating_add(1);
        }
        _ => {}
    }
}

fn set_first(target: &mut Option<FormSpan>, span: &FormSpan, count: &mut usize) {
    *count = count.saturating_add(1);
    if target.is_none() {
        *target = Some(span.clone());
    }
}

fn set_unique(
    target: &mut Option<FormSpan>,
    ambiguous: &mut bool,
    span: &FormSpan,
    count: &mut usize,
) {
    if target.is_some() {
        *ambiguous = true;
    }
    set_first(target, span, count);
}

fn summary_from_index<'a>(
    source: &'a str,
    index: &SymbolIndex,
) -> Result<SymbolSummary<'a>, Error> {
    let extends = index
        .extends
        .as_ref()
        .map(|span| header_value(source, span))
        .transpose()?;
    let power_kind = index
        .power
        .as_ref()
        .map(|span| optional_header_value(source, span))
        .transpose()?
        .flatten();
    Ok(SymbolSummary {
        name: header_value(source, &index.root)?,
        extends,
        in_bom: boolean_value(source, index.in_bom.as_ref(), true)?,
        on_board: boolean_value(source, index.on_board.as_ref(), true)?,
        power: index.power.is_some(),
        power_kind,
        property_count: index.property_count,
        subsymbol_count: index.subsymbol_count,
        pin_count: index.pin_count,
    })
}

fn boolean_value(source: &str, span: Option<&FormSpan>, default: bool) -> Result<bool, Error> {
    let Some(span) = span else { return Ok(default) };
    match header_value(source, span)?.as_ref() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(source_error("Expected yes or no symbol flag", span.start)),
    }
}

fn edit_boolean(
    source: &str,
    index: &SymbolIndex,
    form: Option<&FormSpan>,
    field: SymbolBooleanField,
    value: bool,
    max_output_bytes: usize,
) -> Result<SymbolLibraryEdit, Error> {
    if let Some(span) = form {
        if boolean_value(source, Some(span), true)? == value {
            return unchanged(source);
        }
        let range = header_value_range(source, span)?;
        let replacement = if value { "yes" } else { "no" };
        return patched(source, range, replacement, max_output_bytes);
    }
    if value {
        return unchanged(source);
    }
    let offset = index.root.range.end.saturating_sub(1);
    patched(
        source,
        offset..offset,
        &format!("\n  ({} no)", field.head()),
        max_output_bytes,
    )
}

fn unchanged(source: &str) -> Result<SymbolLibraryEdit, Error> {
    Ok(SymbolLibraryEdit {
        source: source.to_owned(),
        changed: false,
    })
}

fn patched(
    source: &str,
    range: Range<usize>,
    replacement: &str,
    max_output_bytes: usize,
) -> Result<SymbolLibraryEdit, Error> {
    Ok(SymbolLibraryEdit {
        source: apply_patches_with_limit(
            source,
            &[Patch::new(range.start, range.end, replacement.to_owned())],
            max_output_bytes,
        )?,
        changed: true,
    })
}

fn header_value<'a>(source: &'a str, span: &FormSpan) -> Result<Cow<'a, str>, Error> {
    let token = header_value_token(source, span)?;
    Ok(decoded(token))
}

fn optional_header_value<'a>(
    source: &'a str,
    span: &FormSpan,
) -> Result<Option<Cow<'a, str>>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        lexer.next().transpose()?;
        lexer.next().transpose()?;
        let Some(token) = lexer.next().transpose()? else {
            return Ok(None);
        };
        if token.kind == TokenKind::Right {
            Ok(None)
        } else {
            Ok(Some(decoded(token)))
        }
    })()
    .map_err(|error| rebase_error(error, span))
}

fn header_value_range(source: &str, span: &FormSpan) -> Result<Range<usize>, Error> {
    let token = header_value_token(source, span)?;
    Ok((span.range.start + token.position.offset)
        ..(span.range.start + token.position.offset + token.lexeme.len()))
}

fn header_value_token<'a>(source: &'a str, span: &FormSpan) -> Result<Token<'a>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(lexer.next(), TokenKind::Left)?;
        let head = next_value(lexer.next())?;
        if head.kind != TokenKind::Atom {
            return Err(source_error("Expected form head", head.position));
        }
        next_value(lexer.next())
    })()
    .map_err(|error| rebase_error(error, span))
}

fn decoded(token: Token<'_>) -> Cow<'_, str> {
    if token.kind == TokenKind::QuotedString {
        Cow::Owned(decode_quoted(token.lexeme))
    } else {
        Cow::Borrowed(token.lexeme)
    }
}

fn next_value<'a>(token: Option<Result<Token<'a>, Error>>) -> Result<Token<'a>, Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error("Expected form value", Position::START))?;
    if matches!(token.kind, TokenKind::Left | TokenKind::Right) {
        return Err(source_error("Expected form value", token.position));
    }
    Ok(token)
}

fn expect_kind(token: Option<Result<Token<'_>, Error>>, kind: TokenKind) -> Result<(), Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error("Expected opening parenthesis", Position::START))?;
    if token.kind != kind {
        return Err(source_error("Expected opening parenthesis", token.position));
    }
    Ok(())
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

fn source_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}

fn build_limit_error() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "Symbol library output exceeds max_output_bytes",
    )
}

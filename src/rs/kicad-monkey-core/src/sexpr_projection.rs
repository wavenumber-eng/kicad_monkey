//! Allocation-bounded structural selection over KiCad S-expression source.

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Position, Sexp, Token, TokenKind, decode_quoted, parse,
};
use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;
#[cfg(feature = "measurement")]
use std::time::Instant;

/// Resource ceilings for allocation-bounded structural scans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_selected_forms: usize,
    pub max_head_bytes: usize,
}

impl Default for ProjectionLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 512 * 1024 * 1024,
            max_depth: 512,
            max_selected_forms: 4_000_000,
            max_head_bytes: 1024 * 1024,
        }
    }
}

/// A source-form selector. Empty optional sets mean no restriction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selector {
    pub heads: Option<BTreeSet<String>>,
    pub paths: Option<BTreeSet<Vec<String>>>,
    pub min_depth: Option<usize>,
    pub max_depth: Option<usize>,
    pub prune_heads: BTreeSet<String>,
}

impl Selector {
    /// Validate depth bounds before scanning untrusted input.
    pub fn validate(&self) -> Result<(), Error> {
        if self
            .min_depth
            .zip(self.max_depth)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::InvalidSelector,
                "Selector min_depth cannot exceed max_depth",
                Position::START,
            ));
        }
        Ok(())
    }

    fn matches(&self, span: &FormSpan) -> bool {
        if self
            .heads
            .as_ref()
            .is_some_and(|heads| span.head.as_ref().is_none_or(|head| !heads.contains(head)))
        {
            return false;
        }
        if self
            .paths
            .as_ref()
            .is_some_and(|paths| !paths.contains(&span.path))
        {
            return false;
        }
        if self.min_depth.is_some_and(|minimum| span.depth < minimum) {
            return false;
        }
        if self.max_depth.is_some_and(|maximum| span.depth > maximum) {
            return false;
        }
        true
    }

    fn should_scan_children(&self, head: Option<&str>, path: &[String], depth: usize) -> bool {
        if head.is_some_and(|value| self.prune_heads.contains(value)) {
            return false;
        }
        if self.max_depth.is_some_and(|maximum| depth >= maximum) {
            return false;
        }
        self.paths.as_ref().is_none_or(|paths| {
            paths
                .iter()
                .any(|target| target.len() > path.len() && target[..path.len()] == *path)
        })
    }
}

/// Exact byte and line/column range for one complete list form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormSpan {
    pub head: Option<String>,
    pub path: Vec<String>,
    pub depth: usize,
    pub range: Range<usize>,
    pub start: Position,
    pub end: Position,
}

impl FormSpan {
    /// Borrow this form from the original source.
    pub fn text<'a>(&self, source: &'a str) -> Result<&'a str, Error> {
        source.get(self.range.clone()).ok_or_else(|| {
            Error::at(
                ErrorPhase::Tree,
                ErrorKind::InvalidSpan,
                "Form span is not a valid range for this source",
                self.start,
            )
        })
    }
}

/// Reusable structural form index. It owns metadata, never source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralIndex {
    source_len: usize,
    forms: Vec<FormSpan>,
}

impl StructuralIndex {
    /// Scan all visible forms once for repeated selectors.
    pub fn new(source: &str) -> Result<Self, Error> {
        Ok(Self {
            source_len: source.len(),
            forms: scan_form_spans(source, &Selector::default())?,
        })
    }

    pub fn source_len(&self) -> usize {
        self.source_len
    }

    pub fn forms(&self) -> &[FormSpan] {
        &self.forms
    }

    /// Select indexed forms without rescanning the source.
    pub fn select<'a>(&'a self, selector: &'a Selector) -> Result<Vec<&'a FormSpan>, Error> {
        selector.validate()?;
        Ok(self
            .forms
            .iter()
            .filter(|span| selector.matches(span) && !is_pruned(span, selector))
            .collect())
    }
}

fn is_pruned(span: &FormSpan, selector: &Selector) -> bool {
    let ancestor_count = span
        .path
        .len()
        .saturating_sub(usize::from(span.head.is_some()));
    span.path[..ancestor_count]
        .iter()
        .any(|head| selector.prune_heads.contains(head))
}

#[derive(Debug)]
struct Frame {
    head: Option<String>,
    path: Vec<String>,
    depth: usize,
    start: Position,
    visible: bool,
    scan_children: bool,
    awaiting_head: bool,
    teardrop_bare_field: TeardropBareField,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TeardropBareField {
    #[default]
    None,
    AwaitingValue,
    AwaitingClose,
}

/// Select complete list forms without materializing a token vector or tree.
pub fn scan_form_spans(source: &str, selector: &Selector) -> Result<Vec<FormSpan>, Error> {
    scan_form_spans_with_limits(source, selector, ProjectionLimits::default())
}

/// Select complete forms with explicit untrusted-input limits.
pub fn scan_form_spans_with_limits(
    source: &str,
    selector: &Selector,
    limits: ProjectionLimits,
) -> Result<Vec<FormSpan>, Error> {
    let mut spans = scan_form_spans_unsorted(source, selector, limits)?;
    spans.sort_by_key(|span| span.range.start);
    Ok(spans)
}

#[allow(
    clippy::too_many_lines,
    reason = "pre-standard single-pass scanner retained under the structural ratchet"
)]
fn scan_form_spans_unsorted(
    source: &str,
    selector: &Selector,
    limits: ProjectionLimits,
) -> Result<Vec<FormSpan>, Error> {
    selector.validate()?;
    if source.len() > limits.max_source_bytes {
        return Err(resource_error(
            "Projection source exceeds max_source_bytes",
            Position::START,
        ));
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut spans = Vec::new();
    let mut saw_root = false;

    for token in Lexer::new(source) {
        let token = token?;
        match token.kind {
            TokenKind::Left => {
                if stack.len() > limits.max_depth {
                    return Err(resource_error(
                        "Projection nesting exceeds max_depth",
                        token.position,
                    ));
                }
                if let Some(parent) = stack.last_mut()
                    && parent.awaiting_head
                {
                    parent.awaiting_head = false;
                    parent.scan_children = parent.visible
                        && selector.should_scan_children(None, &parent.path, parent.depth);
                }
                let visible = stack.last().is_none_or(|parent| parent.scan_children);
                let path = stack
                    .last()
                    .map_or_else(Vec::new, |parent| parent.path.clone());
                let depth = stack.len();
                stack.push(Frame {
                    head: None,
                    path,
                    depth,
                    start: token.position,
                    visible,
                    scan_children: visible,
                    awaiting_head: true,
                    teardrop_bare_field: TeardropBareField::None,
                });
                saw_root = true;
            }
            TokenKind::Right => {
                if stack.last().is_some_and(|frame| {
                    frame.head.as_deref() == Some("teardrops")
                        && frame.teardrop_bare_field == TeardropBareField::AwaitingClose
                }) {
                    stack.last_mut().expect("checked above").teardrop_bare_field =
                        TeardropBareField::None;
                    continue;
                }
                let Some(frame) = stack.pop() else {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnbalancedClosingParenthesis,
                        "Unbalanced closing parenthesis",
                        token.position,
                    ));
                };
                let span = FormSpan {
                    head: frame.head,
                    path: frame.path,
                    depth: frame.depth,
                    range: frame.start.offset..token.position.offset + 1,
                    start: frame.start,
                    end: Position {
                        offset: token.position.offset + 1,
                        line: token.position.line,
                        column: token.position.column + 1,
                    },
                };
                if frame.visible && selector.matches(&span) {
                    spans.push(span);
                    if spans.len() > limits.max_selected_forms {
                        return Err(resource_error(
                            "Projection selection exceeds max_selected_forms",
                            token.position,
                        ));
                    }
                }
            }
            _ => {
                let Some(frame) = stack.last_mut() else {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::MissingOpeningParenthesis,
                        "Missing initial opening parenthesis",
                        token.position,
                    )
                    .with_token(token.lexeme));
                };
                if frame.awaiting_head {
                    if token.lexeme.len() > limits.max_head_bytes {
                        return Err(resource_error(
                            "Projection form head exceeds max_head_bytes",
                            token.position,
                        ));
                    }
                    let head = token_head(&token);
                    frame.path.push(head.clone());
                    frame.head = Some(head);
                    frame.awaiting_head = false;
                    frame.scan_children = frame.visible
                        && selector.should_scan_children(
                            frame.head.as_deref(),
                            &frame.path,
                            frame.depth,
                        );
                } else if frame.head.as_deref() == Some("teardrops") {
                    frame.teardrop_bare_field = match frame.teardrop_bare_field {
                        TeardropBareField::None if is_teardrop_numeric_field(token.lexeme) => {
                            TeardropBareField::AwaitingValue
                        }
                        TeardropBareField::AwaitingValue => TeardropBareField::AwaitingClose,
                        state => state,
                    };
                }
            }
        }
    }

    if let Some(frame) = stack.last() {
        return Err(Error::at(
            ErrorPhase::Tree,
            ErrorKind::UnbalancedOpeningParenthesis,
            "Unbalanced opening parenthesis",
            frame.start,
        ));
    }
    if !saw_root {
        return Err(Error::at(
            ErrorPhase::Tree,
            ErrorKind::EmptyExpression,
            "No or empty expression",
            Position::START,
        ));
    }
    Ok(spans)
}

fn is_teardrop_numeric_field(value: &str) -> bool {
    matches!(
        value,
        "best_length_ratio" | "max_length" | "best_width_ratio" | "max_width" | "filter_ratio"
    )
}

/// Test-only timing split for select-all scan and source-order sorting.
#[cfg(feature = "measurement")]
#[doc(hidden)]
pub fn measure_form_span_sort(
    source: &str,
    selector: &Selector,
    limits: ProjectionLimits,
) -> Result<(Vec<FormSpan>, u128, u128), Error> {
    let scan_started = Instant::now();
    let mut spans = scan_form_spans_unsorted(source, selector, limits)?;
    let scan_ns = scan_started.elapsed().as_nanos();
    let sort_started = Instant::now();
    spans.sort_by_key(|span| span.range.start);
    let sort_ns = sort_started.elapsed().as_nanos();
    Ok((spans, scan_ns, sort_ns))
}

/// Materialize only one previously selected form.
pub fn parse_form(source: &str, span: &FormSpan) -> Result<Sexp, Error> {
    parse(span.text(source)?)
}

fn token_head(token: &Token<'_>) -> String {
    match token.kind {
        TokenKind::QuotedString => decode_quoted(token.lexeme),
        _ => token.lexeme.to_owned(),
    }
}

fn resource_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        position,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StreamLexState {
    Normal,
    Atom,
    Unicode { in_atom: bool },
    Quoted { escaped: bool },
    Comment,
}

struct Utf8Validator {
    remaining: u8,
    next_min: u8,
    next_max: u8,
    sequence_start: Position,
}

impl Utf8Validator {
    fn new() -> Self {
        Self {
            remaining: 0,
            next_min: 0x80,
            next_max: 0xbf,
            sequence_start: Position::START,
        }
    }

    fn accept(&mut self, byte: u8, position: Position) -> Result<(), Error> {
        if self.remaining != 0 {
            if !(self.next_min..=self.next_max).contains(&byte) {
                return Err(invalid_utf8(position));
            }
            self.remaining -= 1;
            self.next_min = 0x80;
            self.next_max = 0xbf;
            return Ok(());
        }
        if byte <= 0x7f {
            return Ok(());
        }
        self.sequence_start = position;
        match byte {
            0xc2..=0xdf => self.remaining = 1,
            0xe0 => {
                self.remaining = 2;
                self.next_min = 0xa0;
            }
            0xe1..=0xec | 0xee..=0xef => self.remaining = 2,
            0xed => {
                self.remaining = 2;
                self.next_max = 0x9f;
            }
            0xf0 => {
                self.remaining = 3;
                self.next_min = 0x90;
            }
            0xf1..=0xf3 => self.remaining = 3,
            0xf4 => {
                self.remaining = 3;
                self.next_max = 0x8f;
            }
            _ => return Err(invalid_utf8(position)),
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), Error> {
        if self.remaining == 0 {
            Ok(())
        } else {
            Err(invalid_utf8(self.sequence_start))
        }
    }
}

fn invalid_utf8(position: Position) -> Error {
    Error::at(
        ErrorPhase::Lex,
        ErrorKind::InvalidUtf8,
        "KiCad S-expression input is not valid UTF-8",
        position,
    )
}

struct StreamingProjection<'a> {
    selector: &'a Selector,
    limits: ProjectionLimits,
    stack: Vec<Frame>,
    spans: Vec<FormSpan>,
    state: StreamLexState,
    position: Position,
    token_start: Position,
    head_bytes: Vec<u8>,
    unicode_bytes: Vec<u8>,
    collect_head: bool,
    collect_teardrop_scalar: bool,
    saw_root: bool,
    line_has_content: bool,
    last_was_cr: bool,
    utf8: Utf8Validator,
}

impl<'a> StreamingProjection<'a> {
    fn new(selector: &'a Selector, limits: ProjectionLimits) -> Self {
        Self {
            selector,
            limits,
            stack: Vec::new(),
            spans: Vec::new(),
            state: StreamLexState::Normal,
            position: Position::START,
            token_start: Position::START,
            head_bytes: Vec::new(),
            unicode_bytes: Vec::new(),
            collect_head: false,
            collect_teardrop_scalar: false,
            saw_root: false,
            line_has_content: false,
            last_was_cr: false,
            utf8: Utf8Validator::new(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "pre-standard byte-state transition retained under the structural ratchet"
    )]
    fn feed(&mut self, byte: u8) -> Result<(), Error> {
        if self.position.offset >= self.limits.max_source_bytes {
            return Err(resource_error(
                "Projection source exceeds max_source_bytes",
                self.position,
            ));
        }
        self.utf8.accept(byte, self.position)?;
        let mut reprocess = true;
        while reprocess {
            reprocess = false;
            match self.state {
                StreamLexState::Comment => {
                    if matches!(byte, b'\r' | b'\n') {
                        self.state = StreamLexState::Normal;
                        reprocess = true;
                    } else {
                        self.advance(byte);
                    }
                }
                StreamLexState::Atom => {
                    if is_ascii_separator(byte) {
                        self.finish_head(false)?;
                        self.state = StreamLexState::Normal;
                        reprocess = true;
                    } else if !byte.is_ascii() {
                        self.unicode_bytes.clear();
                        self.unicode_bytes.push(byte);
                        self.state = StreamLexState::Unicode { in_atom: true };
                        self.advance(byte);
                    } else {
                        self.push_head_byte(byte)?;
                        self.advance(byte);
                    }
                }
                StreamLexState::Unicode { in_atom } => {
                    self.unicode_bytes.push(byte);
                    self.advance(byte);
                    if self.utf8.remaining == 0 {
                        self.finish_unicode_scalar(in_atom)?;
                    }
                }
                StreamLexState::Quoted { escaped } => {
                    self.push_head_byte(byte)?;
                    self.advance(byte);
                    if escaped {
                        self.state = StreamLexState::Quoted { escaped: false };
                    } else if byte == b'\\' {
                        self.state = StreamLexState::Quoted { escaped: true };
                    } else if byte == b'"' {
                        self.finish_head(true)?;
                        self.state = StreamLexState::Normal;
                        self.line_has_content = true;
                    }
                }
                StreamLexState::Normal => match byte {
                    b if is_ascii_space(b) => self.advance(b),
                    b'#' if !self.line_has_content => {
                        self.state = StreamLexState::Comment;
                        self.advance(byte);
                    }
                    b'(' => {
                        self.open_form()?;
                        self.line_has_content = true;
                        self.advance(byte);
                    }
                    b')' => {
                        let close = self.position;
                        self.line_has_content = true;
                        self.advance(byte);
                        self.close_form(close)?;
                    }
                    b'"' => {
                        self.begin_head();
                        self.push_head_byte(byte)?;
                        self.state = StreamLexState::Quoted { escaped: false };
                        self.line_has_content = true;
                        self.advance(byte);
                    }
                    non_ascii if !non_ascii.is_ascii() => {
                        self.begin_head();
                        self.unicode_bytes.clear();
                        self.unicode_bytes.push(non_ascii);
                        self.state = StreamLexState::Unicode { in_atom: false };
                        self.advance(non_ascii);
                    }
                    _ => {
                        if self.stack.is_empty() {
                            return Err(Error::at(
                                ErrorPhase::Tree,
                                ErrorKind::MissingOpeningParenthesis,
                                "Missing initial opening parenthesis",
                                self.position,
                            ));
                        }
                        self.begin_head();
                        self.push_head_byte(byte)?;
                        self.state = StreamLexState::Atom;
                        self.line_has_content = true;
                        self.advance(byte);
                    }
                },
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<FormSpan>, Error> {
        if self.state == StreamLexState::Atom {
            self.finish_head(false)?;
        } else if matches!(self.state, StreamLexState::Quoted { .. }) {
            return Err(Error::at(
                ErrorPhase::Lex,
                ErrorKind::UnterminatedString,
                "Unterminated delimited string",
                self.token_start,
            ));
        }
        self.utf8.finish()?;
        if matches!(self.state, StreamLexState::Unicode { .. }) {
            return Err(invalid_utf8(self.token_start));
        }
        if let Some(frame) = self.stack.last() {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedOpeningParenthesis,
                "Unbalanced opening parenthesis",
                frame.start,
            ));
        }
        if !self.saw_root {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::EmptyExpression,
                "No or empty expression",
                Position::START,
            ));
        }
        Ok(self.spans)
    }

    fn open_form(&mut self) -> Result<(), Error> {
        if self.stack.len() > self.limits.max_depth {
            return Err(resource_error(
                "Projection nesting exceeds max_depth",
                self.position,
            ));
        }
        if let Some(parent) = self.stack.last_mut()
            && parent.awaiting_head
        {
            parent.awaiting_head = false;
            parent.scan_children = parent.visible
                && self
                    .selector
                    .should_scan_children(None, &parent.path, parent.depth);
        }
        let visible = self.stack.last().is_none_or(|parent| parent.scan_children);
        let path = self
            .stack
            .last()
            .map_or_else(Vec::new, |parent| parent.path.clone());
        self.stack.push(Frame {
            head: None,
            path,
            depth: self.stack.len(),
            start: self.position,
            visible,
            scan_children: visible,
            awaiting_head: true,
            teardrop_bare_field: TeardropBareField::None,
        });
        self.saw_root = true;
        Ok(())
    }

    fn close_form(&mut self, close: Position) -> Result<(), Error> {
        if self.stack.last().is_some_and(|frame| {
            frame.head.as_deref() == Some("teardrops")
                && frame.teardrop_bare_field == TeardropBareField::AwaitingClose
        }) {
            self.stack
                .last_mut()
                .expect("checked above")
                .teardrop_bare_field = TeardropBareField::None;
            return Ok(());
        }
        let Some(frame) = self.stack.pop() else {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedClosingParenthesis,
                "Unbalanced closing parenthesis",
                close,
            ));
        };
        let span = FormSpan {
            head: frame.head,
            path: frame.path,
            depth: frame.depth,
            range: frame.start.offset..self.position.offset,
            start: frame.start,
            end: self.position,
        };
        if frame.visible && self.selector.matches(&span) {
            self.spans.push(span);
            if self.spans.len() > self.limits.max_selected_forms {
                return Err(resource_error(
                    "Projection selection exceeds max_selected_forms",
                    close,
                ));
            }
        }
        Ok(())
    }

    fn begin_head(&mut self) {
        self.token_start = self.position;
        self.collect_head = self.stack.last().is_some_and(|frame| frame.awaiting_head);
        self.collect_teardrop_scalar = self.stack.last().is_some_and(|frame| {
            frame.head.as_deref() == Some("teardrops")
                && frame.teardrop_bare_field != TeardropBareField::AwaitingClose
        });
        self.head_bytes.clear();
    }

    fn push_head_byte(&mut self, byte: u8) -> Result<(), Error> {
        if self.collect_head || self.collect_teardrop_scalar {
            if self.head_bytes.len() >= self.limits.max_head_bytes {
                return Err(resource_error(
                    "Projection form head exceeds max_head_bytes",
                    self.token_start,
                ));
            }
            self.head_bytes.push(byte);
        }
        Ok(())
    }

    fn finish_head(&mut self, quoted: bool) -> Result<(), Error> {
        if !self.collect_head && !self.collect_teardrop_scalar {
            return Ok(());
        }
        let text =
            std::str::from_utf8(&self.head_bytes).map_err(|_| invalid_utf8(self.token_start))?;
        let head = if quoted {
            decode_quoted(text)
        } else {
            text.to_owned()
        };
        if !self.collect_head {
            let frame = self.stack.last_mut().ok_or_else(|| {
                Error::at(
                    ErrorPhase::Tree,
                    ErrorKind::MissingOpeningParenthesis,
                    "Missing initial opening parenthesis",
                    self.token_start,
                )
            })?;
            frame.teardrop_bare_field = match frame.teardrop_bare_field {
                TeardropBareField::None if is_teardrop_numeric_field(&head) => {
                    TeardropBareField::AwaitingValue
                }
                TeardropBareField::AwaitingValue => TeardropBareField::AwaitingClose,
                state => state,
            };
            self.collect_teardrop_scalar = false;
            return Ok(());
        }
        let Some(frame) = self.stack.last_mut() else {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::MissingOpeningParenthesis,
                "Missing initial opening parenthesis",
                self.token_start,
            ));
        };
        frame.path.push(head.clone());
        frame.head = Some(head);
        frame.awaiting_head = false;
        frame.scan_children = frame.visible
            && self
                .selector
                .should_scan_children(frame.head.as_deref(), &frame.path, frame.depth);
        self.collect_head = false;
        self.collect_teardrop_scalar = false;
        Ok(())
    }

    fn finish_unicode_scalar(&mut self, in_atom: bool) -> Result<(), Error> {
        let text =
            std::str::from_utf8(&self.unicode_bytes).map_err(|_| invalid_utf8(self.token_start))?;
        let Some(character) = text.chars().next() else {
            return Err(invalid_utf8(self.token_start));
        };
        if character.is_whitespace() {
            if in_atom {
                self.finish_head(false)?;
            } else {
                self.collect_head = false;
                self.head_bytes.clear();
            }
            self.state = StreamLexState::Normal;
            return Ok(());
        }
        if !in_atom && self.stack.is_empty() {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::MissingOpeningParenthesis,
                "Missing initial opening parenthesis",
                self.token_start,
            ));
        }
        let bytes = std::mem::take(&mut self.unicode_bytes);
        for byte in bytes {
            self.push_head_byte(byte)?;
        }
        self.line_has_content = true;
        self.state = StreamLexState::Atom;
        Ok(())
    }

    fn advance(&mut self, byte: u8) {
        self.position.offset = self.position.offset.saturating_add(1);
        match byte {
            b'\r' => {
                self.position.line = self.position.line.saturating_add(1);
                self.position.column = 1;
                self.line_has_content = false;
                self.last_was_cr = true;
            }
            b'\n' => {
                if !self.last_was_cr {
                    self.position.line = self.position.line.saturating_add(1);
                }
                self.position.column = 1;
                self.line_has_content = false;
                self.last_was_cr = false;
            }
            continuation if continuation & 0xc0 == 0x80 => {
                self.last_was_cr = false;
            }
            _ => {
                self.position.column = self.position.column.saturating_add(1);
                self.last_was_cr = false;
            }
        }
    }
}

fn is_ascii_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
}

fn is_ascii_separator(byte: u8) -> bool {
    is_ascii_space(byte) || matches!(byte, b'(' | b')')
}

/// Scan selected metadata from a native byte stream without retaining the file.
pub fn scan_reader_form_spans<R: Read>(
    mut reader: R,
    selector: &Selector,
    limits: ProjectionLimits,
) -> Result<Vec<FormSpan>, Error> {
    selector.validate()?;
    let mut scanner = StreamingProjection::new(selector, limits);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|_| {
            Error::at(
                ErrorPhase::Lex,
                ErrorKind::Io,
                "Failed to read S-expression source",
                scanner.position,
            )
        })?;
        if count == 0 {
            break;
        }
        for byte in &buffer[..count] {
            scanner.feed(*byte)?;
        }
    }
    let mut spans = scanner.finish()?;
    spans.sort_by_key(|span| span.range.start);
    Ok(spans)
}

/// Test-only timing split for streaming select-all scan and source-order sort.
#[cfg(feature = "measurement")]
#[doc(hidden)]
pub fn measure_reader_form_span_sort<R: Read>(
    mut reader: R,
    selector: &Selector,
    limits: ProjectionLimits,
) -> Result<(Vec<FormSpan>, u128, u128), Error> {
    selector.validate()?;
    let scan_started = Instant::now();
    let mut scanner = StreamingProjection::new(selector, limits);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|_| {
            Error::at(
                ErrorPhase::Lex,
                ErrorKind::Io,
                "Failed to read S-expression source",
                scanner.position,
            )
        })?;
        if count == 0 {
            break;
        }
        for byte in &buffer[..count] {
            scanner.feed(*byte)?;
        }
    }
    let mut spans = scanner.finish()?;
    let scan_ns = scan_started.elapsed().as_nanos();
    let sort_started = Instant::now();
    spans.sort_by_key(|span| span.range.start);
    let sort_ns = sort_started.elapsed().as_nanos();
    Ok((spans, scan_ns, sort_ns))
}

/// Seek and read only one selected form from an indexed native source.
pub fn read_form_bytes<R: Read + Seek>(
    reader: &mut R,
    span: &FormSpan,
    max_form_bytes: usize,
) -> Result<Vec<u8>, Error> {
    let Some(length) = span.range.end.checked_sub(span.range.start) else {
        return Err(Error::at(
            ErrorPhase::Tree,
            ErrorKind::InvalidSpan,
            "Selected form has an inverted byte range",
            span.start,
        ));
    };
    if length > max_form_bytes {
        return Err(resource_error(
            "Selected form exceeds max_form_bytes",
            span.start,
        ));
    }
    let offset = u64::try_from(span.range.start).map_err(|_| {
        Error::at(
            ErrorPhase::Lex,
            ErrorKind::Io,
            "Selected form offset cannot be represented by native I/O",
            span.start,
        )
    })?;
    reader.seek(SeekFrom::Start(offset)).map_err(|_| {
        Error::at(
            ErrorPhase::Lex,
            ErrorKind::Io,
            "Failed to seek to selected form",
            span.start,
        )
    })?;
    let mut output = vec![0_u8; length];
    reader.read_exact(&mut output).map_err(|_| {
        Error::at(
            ErrorPhase::Lex,
            ErrorKind::Io,
            "Failed to read complete selected form",
            span.start,
        )
    })?;
    Ok(output)
}

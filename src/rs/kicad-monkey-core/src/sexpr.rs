//! KiCad DSN-style S-expression parsing and deterministic building.
//!
//! The lexer borrows token text from the source and feeds the parser lazily.
//! An owned tree is allocated only when [`parse`] is requested; future typed
//! readers can consume [`Lexer`] directly without constructing this tree.

use std::borrow::Cow;
use std::fmt;
use std::fmt::Write as _;

/// The phase that produced an S-expression error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorPhase {
    /// UTF-8 validation or lexical scanning.
    Lex,
    /// Parenthesis or tree construction.
    Tree,
    /// Deterministic serialization.
    Build,
}

/// Stable error classification for programmatic diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Input bytes were not valid UTF-8.
    InvalidUtf8,
    /// A quoted string did not terminate.
    UnterminatedString,
    /// No expression was present.
    EmptyExpression,
    /// The first token was not an opening parenthesis.
    MissingOpeningParenthesis,
    /// An opening parenthesis did not have a matching close.
    UnbalancedOpeningParenthesis,
    /// A closing parenthesis did not have a matching open.
    UnbalancedClosingParenthesis,
    /// Tokens remained after the root expression.
    LeftoverGarbage,
    /// A token did not satisfy a KiCad dialect production.
    UnexpectedToken,
    /// A numeric token exceeded the initial integer representation.
    IntegerOutOfRange,
    /// A configured parser resource limit was exceeded.
    ResourceLimit,
    /// A value could not be emitted safely.
    InvalidBuildValue,
    /// A source-preserving patch was invalid or overlapped another patch.
    InvalidPatch,
    /// Selector depth or path constraints were inconsistent.
    InvalidSelector,
    /// A form span did not belong to the supplied source.
    InvalidSpan,
    /// Native source reading or seeking failed.
    Io,
}

/// One-based source line/column plus a zero-based byte offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    /// Zero-based byte offset.
    pub offset: usize,
    /// One-based line.
    pub line: usize,
    /// One-based Unicode-scalar column.
    pub column: usize,
}

impl Position {
    pub(crate) const START: Self = Self {
        offset: 0,
        line: 1,
        column: 1,
    };
}

/// Structured parse/build failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    /// Stage that raised the error.
    pub phase: ErrorPhase,
    /// Stable error category.
    pub kind: ErrorKind,
    /// Human-readable message compatible with the Python behavior family.
    pub message: Cow<'static, str>,
    /// Source position, when applicable.
    pub position: Option<Position>,
    /// Nearby token text, when useful.
    pub token: Option<String>,
}

impl Error {
    pub(crate) fn at(
        phase: ErrorPhase,
        kind: ErrorKind,
        message: &'static str,
        position: Position,
    ) -> Self {
        Self {
            phase,
            kind,
            message: Cow::Borrowed(message),
            position: Some(position),
            token: None,
        }
    }

    pub(crate) fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    pub(crate) fn build(kind: ErrorKind, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            phase: ErrorPhase::Build,
            kind,
            message: message.into(),
            position: None,
            token: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if let Some(position) = self.position {
            write!(
                formatter,
                " at line {}, column {} (byte {})",
                position.line, position.column, position.offset
            )?;
        }
        if let Some(token) = &self.token {
            write!(formatter, " near {token:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// Parser resource limits for untrusted and browser inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Maximum source size in bytes.
    pub max_source_bytes: usize,
    /// Maximum nested list depth, with the root at depth zero.
    pub max_depth: usize,
    /// Maximum allocated tree nodes, including lists.
    pub max_nodes: usize,
    /// Maximum decoded byte length for one quoted string.
    pub max_decoded_string_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_source_bytes: 512 * 1024 * 1024,
            max_depth: 512,
            max_nodes: 16_000_000,
            max_decoded_string_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Owned compatibility tree produced on demand.
#[derive(Clone, Debug)]
pub enum Sexp {
    /// Parenthesized child sequence.
    List(Vec<Self>),
    /// Unquoted atom.
    Atom(String),
    /// Quoted string after KiCad escape decoding.
    Quoted(String),
    /// Base-10 integer.
    Integer(i64),
    /// Decimal or exponent-form floating-point value.
    Float(f64),
}

impl PartialEq for Sexp {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::List(left), Self::List(right)) => left == right,
            (Self::Atom(left), Self::Atom(right))
            | (Self::Quoted(left), Self::Quoted(right))
            | (Self::Atom(left), Self::Quoted(right))
            | (Self::Quoted(left), Self::Atom(right)) => left == right,
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::Integer(integer), Self::Float(float))
            | (Self::Float(float), Self::Integer(integer)) => (*integer as f64) == *float,
            _ => false,
        }
    }
}

impl Sexp {
    pub(crate) fn is_atom(&self, expected: &str) -> bool {
        matches!(self, Self::Atom(value) if value == expected)
    }
}

/// Borrowed lexical token category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    /// `(`.
    Left,
    /// `)`.
    Right,
    /// Bare non-numeric atom.
    Atom,
    /// Quoted string, including source quotes in [`Token::lexeme`].
    QuotedString,
    /// Integer lexical form.
    Integer,
    /// Decimal or exponent lexical form.
    Float,
}

/// A token borrowing its exact text from the input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token<'a> {
    /// Token category.
    pub kind: TokenKind,
    /// Exact source bytes interpreted as UTF-8.
    pub lexeme: &'a str,
    /// Token start.
    pub position: Position,
}

/// One source-preserving replacement over UTF-8 byte offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Patch<'a> {
    /// Inclusive replacement start byte.
    pub start_offset: usize,
    /// Exclusive replacement end byte.
    pub end_offset: usize,
    /// Replacement text.
    pub replacement: Cow<'a, str>,
}

/// Options for the generic token-preserving formatter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatOptions {
    /// Spaces used for each displayed nesting level.
    pub indentation_size: usize,
    /// Lists deeper than this level remain inline.
    pub max_nesting: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indentation_size: 2,
            max_nesting: 2,
        }
    }
}

impl<'a> Patch<'a> {
    /// Construct a patch borrowing or owning its replacement text.
    pub fn new(
        start_offset: usize,
        end_offset: usize,
        replacement: impl Into<Cow<'a, str>>,
    ) -> Self {
        Self {
            start_offset,
            end_offset,
            replacement: replacement.into(),
        }
    }
}

/// Streaming lexer for the KiCad DSN-style dialect.
#[derive(Clone, Debug)]
pub struct Lexer<'a> {
    source: &'a str,
    position: Position,
    line_start: usize,
    finished: bool,
}

impl<'a> Lexer<'a> {
    /// Create a lexer over UTF-8 source text.
    #[must_use]
    pub const fn new(source: &'a str) -> Self {
        Self {
            source,
            position: Position::START,
            line_start: 0,
            finished: false,
        }
    }

    /// Current scanner position.
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    fn scan_token(&mut self) -> Result<Option<Token<'a>>, Error> {
        self.skip_layout();
        if self.position.offset >= self.source.len() {
            return Ok(None);
        }

        let start = self.position;
        let byte = self.source.as_bytes()[start.offset];
        match byte {
            b'(' => {
                self.advance_to(start.offset + 1);
                Ok(Some(Token {
                    kind: TokenKind::Left,
                    lexeme: &self.source[start.offset..start.offset + 1],
                    position: start,
                }))
            }
            b')' => {
                self.advance_to(start.offset + 1);
                Ok(Some(Token {
                    kind: TokenKind::Right,
                    lexeme: &self.source[start.offset..start.offset + 1],
                    position: start,
                }))
            }
            b'"' => self.scan_quoted(start).map(Some),
            _ => Ok(Some(self.scan_atom(start))),
        }
    }

    fn skip_layout(&mut self) {
        loop {
            while let Some(character) = self.current_char() {
                if !character.is_whitespace() {
                    break;
                }
                self.advance_one();
            }

            if self.position.offset >= self.source.len()
                || self.source.as_bytes()[self.position.offset] != b'#'
                || !self.source[self.line_start..self.position.offset]
                    .trim()
                    .is_empty()
            {
                return;
            }

            while let Some(character) = self.current_char() {
                if character == '\r' || character == '\n' {
                    break;
                }
                self.advance_one();
            }
        }
    }

    fn scan_quoted(&mut self, start: Position) -> Result<Token<'a>, Error> {
        self.advance_one();
        let mut escaped = false;
        while let Some(character) = self.current_char() {
            if escaped {
                escaped = false;
                self.advance_one();
                continue;
            }
            if character == '\\' {
                escaped = true;
                self.advance_one();
                continue;
            }
            self.advance_one();
            if character == '"' {
                return Ok(Token {
                    kind: TokenKind::QuotedString,
                    lexeme: &self.source[start.offset..self.position.offset],
                    position: start,
                });
            }
        }

        Err(Error::at(
            ErrorPhase::Lex,
            ErrorKind::UnterminatedString,
            "Unterminated delimited string",
            start,
        ))
    }

    fn scan_atom(&mut self, start: Position) -> Token<'a> {
        while let Some(character) = self.current_char() {
            if character.is_whitespace() || character == '(' || character == ')' {
                break;
            }
            self.advance_one();
        }
        let lexeme = &self.source[start.offset..self.position.offset];
        Token {
            kind: classify_bare_token(lexeme),
            lexeme,
            position: start,
        }
    }

    fn current_char(&self) -> Option<char> {
        self.source[self.position.offset..].chars().next()
    }

    fn advance_one(&mut self) {
        let offset = self.position.offset;
        if offset >= self.source.len() {
            return;
        }

        let remaining = &self.source[offset..];
        if remaining.starts_with("\r\n") {
            self.position.offset += 2;
            self.position.line += 1;
            self.position.column = 1;
            self.line_start = self.position.offset;
            return;
        }

        let Some(character) = remaining.chars().next() else {
            return;
        };
        self.position.offset += character.len_utf8();
        if character == '\r' || character == '\n' {
            self.position.line += 1;
            self.position.column = 1;
            self.line_start = self.position.offset;
        } else {
            self.position.column += 1;
        }
    }

    fn advance_to(&mut self, end: usize) {
        while self.position.offset < end {
            self.advance_one();
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.scan_token() {
            Ok(Some(token)) => Some(Ok(token)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

/// Collect borrowed tokens for diagnostics and compatibility tests.
pub fn lex(source: &str) -> Result<Vec<Token<'_>>, Error> {
    Lexer::new(source).collect()
}

/// Parse one UTF-8 KiCad S-expression using default resource limits.
pub fn parse(source: &str) -> Result<Sexp, Error> {
    parse_with_limits(source, Limits::default())
}

/// Parse bytes after validating UTF-8.
pub fn parse_bytes(source: &[u8]) -> Result<Sexp, Error> {
    utf8_text(source).and_then(parse)
}

/// Validate bytes and borrow them as UTF-8 without constructing a syntax tree.
pub fn utf8_text(source: &[u8]) -> Result<&str, Error> {
    match std::str::from_utf8(source) {
        Ok(text) => Ok(text),
        Err(error) => {
            let offset = error.valid_up_to();
            let position = position_at_valid_prefix(&source[..offset]);
            Err(Error::at(
                ErrorPhase::Lex,
                ErrorKind::InvalidUtf8,
                "KiCad S-expression input is not valid UTF-8",
                position,
            ))
        }
    }
}

/// Parse one expression with explicit untrusted-input limits.
pub fn parse_with_limits(source: &str, limits: Limits) -> Result<Sexp, Error> {
    if source.len() > limits.max_source_bytes {
        return Err(Error::at(
            ErrorPhase::Lex,
            ErrorKind::ResourceLimit,
            "S-expression source exceeds max_source_bytes",
            Position::START,
        ));
    }
    Parser::new(source, limits).parse()
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    lookahead: Option<Token<'a>>,
    limits: Limits,
    node_count: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, limits: Limits) -> Self {
        Self {
            lexer: Lexer::new(source),
            lookahead: None,
            limits,
            node_count: 0,
        }
    }

    fn parse(mut self) -> Result<Sexp, Error> {
        let Some(first) = self.peek_token()? else {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::EmptyExpression,
                "No or empty expression",
                Position::START,
            ));
        };
        if first.kind != TokenKind::Left {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::MissingOpeningParenthesis,
                "Missing initial opening parenthesis",
                first.position,
            )
            .with_token(first.lexeme));
        }

        let result = self.parse_list(0)?;
        if let Some(token) = self.peek_token()? {
            if token.kind == TokenKind::Right {
                return Err(Error::at(
                    ErrorPhase::Tree,
                    ErrorKind::UnbalancedClosingParenthesis,
                    "Unbalanced closing parenthesis",
                    token.position,
                ));
            }
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::LeftoverGarbage,
                "Leftover garbage after end of expression",
                token.position,
            )
            .with_token(token.lexeme));
        }
        Ok(result)
    }

    fn parse_list(&mut self, depth: usize) -> Result<Sexp, Error> {
        let open = self.expect(TokenKind::Left, "Expected opening parenthesis")?;
        if depth > self.limits.max_depth {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::ResourceLimit,
                "S-expression nesting exceeds max_depth",
                open.position,
            ));
        }
        self.account_node(open.position)?;

        if self.peek_kind()? == Some(TokenKind::Right) {
            self.take_token()?;
            return Ok(Sexp::List(Vec::new()));
        }

        let first = self.parse_item(depth)?;
        let mut values = vec![first];
        if values[0].is_atom("teardrops") {
            self.parse_teardrops_body(&mut values, depth, open.position)?;
            return Ok(Sexp::List(values));
        }

        loop {
            match self.peek_kind()? {
                Some(TokenKind::Right) => {
                    self.take_token()?;
                    return Ok(Sexp::List(values));
                }
                Some(_) => values.push(self.parse_item(depth)?),
                None => {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnbalancedOpeningParenthesis,
                        "Unbalanced opening parenthesis",
                        open.position,
                    ));
                }
            }
        }
    }

    fn parse_item(&mut self, parent_depth: usize) -> Result<Sexp, Error> {
        match self.peek_kind()? {
            Some(TokenKind::Left) => self.parse_list(parent_depth + 1),
            Some(TokenKind::Right) => {
                let token = self.take_required("Expected closing parenthesis")?;
                Err(Error::at(
                    ErrorPhase::Tree,
                    ErrorKind::UnbalancedClosingParenthesis,
                    "Unbalanced closing parenthesis",
                    token.position,
                ))
            }
            Some(_) => {
                let token = self.take_required("Expected value token")?;
                self.value_from_token(token)
            }
            None => Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedOpeningParenthesis,
                "Unbalanced opening parenthesis",
                self.lexer.position(),
            )),
        }
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "pre-standard dialect parser retained under the structural ratchet"
    )]
    fn parse_teardrops_body(
        &mut self,
        values: &mut Vec<Sexp>,
        depth: usize,
        open_position: Position,
    ) -> Result<(), Error> {
        loop {
            match self.peek_kind()? {
                Some(TokenKind::Right) => {
                    self.take_token()?;
                    return Ok(());
                }
                Some(TokenKind::Left) => {
                    let list_open = self.take_required("Expected opening parenthesis")?;
                    self.account_node(list_open.position)?;
                    let Some(key_token) = self.take_token()? else {
                        return Err(Error::at(
                            ErrorPhase::Tree,
                            ErrorKind::UnbalancedOpeningParenthesis,
                            "Unexpected end of teardrops block",
                            list_open.position,
                        ));
                    };
                    if is_teardrop_field_key(&key_token) {
                        values.push(self.parse_teardrop_field(key_token, true, depth)?);
                    } else {
                        let first = self.value_from_token(key_token)?;
                        values.push(self.parse_list_tail(
                            vec![first],
                            depth,
                            list_open.position,
                        )?);
                    }
                }
                Some(TokenKind::Atom) => {
                    let Some(key_token) = self.peek_token()? else {
                        return Err(Error::at(
                            ErrorPhase::Tree,
                            ErrorKind::UnbalancedOpeningParenthesis,
                            "Unbalanced opening parenthesis",
                            open_position,
                        ));
                    };
                    if !is_teardrop_field_key(&key_token) {
                        return Err(Error::at(
                            ErrorPhase::Tree,
                            ErrorKind::UnexpectedToken,
                            "Unexpected teardrops token",
                            key_token.position,
                        )
                        .with_token(key_token.lexeme));
                    }
                    let key_token = self.take_required("Expected teardrops field")?;
                    self.account_node(key_token.position)?;
                    values.push(self.parse_teardrop_field(key_token, false, depth)?);
                }
                Some(_) => {
                    let Some(token) = self.peek_token()? else {
                        return Err(Error::at(
                            ErrorPhase::Tree,
                            ErrorKind::UnbalancedOpeningParenthesis,
                            "Unbalanced opening parenthesis",
                            open_position,
                        ));
                    };
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnexpectedToken,
                        "Unexpected teardrops token",
                        token.position,
                    )
                    .with_token(token.lexeme));
                }
                None => {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnbalancedOpeningParenthesis,
                        "Unbalanced opening parenthesis",
                        open_position,
                    ));
                }
            }
        }
    }

    fn parse_teardrop_field(
        &mut self,
        key_token: Token<'a>,
        parenthesized: bool,
        depth: usize,
    ) -> Result<Sexp, Error> {
        self.account_node(key_token.position)?;
        let key = key_token.lexeme.to_owned();
        let mut field = vec![Sexp::Atom(key.clone())];
        if is_teardrop_bool_key(&key) {
            if parenthesized {
                if self.peek_kind()? == Some(TokenKind::Right) {
                    self.take_token()?;
                    return Ok(Sexp::List(field));
                }
                let Some(value_token) = self.take_token()? else {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnexpectedToken,
                        "Expected yes/no for teardrops field",
                        key_token.position,
                    ));
                };
                if value_token.kind != TokenKind::Atom
                    || !matches!(
                        value_token.lexeme.to_ascii_lowercase().as_str(),
                        "yes" | "no" | "true" | "false"
                    )
                {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnexpectedToken,
                        "Expected yes/no for teardrops field",
                        value_token.position,
                    )
                    .with_token(value_token.lexeme));
                }
                field.push(self.value_from_token(value_token)?);
                self.expect(TokenKind::Right, "Expected closing parenthesis")?;
            }
            return Ok(Sexp::List(field));
        }

        if self.peek_kind()?.is_none() {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnexpectedToken,
                "Missing value for teardrops field",
                key_token.position,
            ));
        }
        field.push(self.parse_item(depth)?);
        self.expect(TokenKind::Right, "Expected closing parenthesis")?;
        Ok(Sexp::List(field))
    }

    fn parse_list_tail(
        &mut self,
        mut values: Vec<Sexp>,
        depth: usize,
        open_position: Position,
    ) -> Result<Sexp, Error> {
        loop {
            match self.peek_kind()? {
                Some(TokenKind::Right) => {
                    self.take_token()?;
                    return Ok(Sexp::List(values));
                }
                Some(_) => values.push(self.parse_item(depth)?),
                None => {
                    return Err(Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnbalancedOpeningParenthesis,
                        "Unbalanced opening parenthesis",
                        open_position,
                    ));
                }
            }
        }
    }

    fn value_from_token(&mut self, token: Token<'a>) -> Result<Sexp, Error> {
        self.account_node(token.position)?;
        match token.kind {
            TokenKind::Atom => Ok(Sexp::Atom(token.lexeme.to_owned())),
            TokenKind::QuotedString => {
                let decoded =
                    decode_quoted_with_limit(token.lexeme, self.limits.max_decoded_string_bytes)
                        .ok_or_else(|| {
                            Error::at(
                                ErrorPhase::Tree,
                                ErrorKind::ResourceLimit,
                                "Decoded string exceeds max_decoded_string_bytes",
                                token.position,
                            )
                        })?;
                Ok(Sexp::Quoted(decoded))
            }
            TokenKind::Integer => token.lexeme.parse::<i64>().map(Sexp::Integer).map_err(|_| {
                Error::at(
                    ErrorPhase::Tree,
                    ErrorKind::IntegerOutOfRange,
                    "Integer token is outside the initial i64 representation",
                    token.position,
                )
                .with_token(token.lexeme)
            }),
            TokenKind::Float => token.lexeme.parse::<f64>().map(Sexp::Float).map_err(|_| {
                Error::at(
                    ErrorPhase::Tree,
                    ErrorKind::UnexpectedToken,
                    "Invalid floating-point token",
                    token.position,
                )
                .with_token(token.lexeme)
            }),
            TokenKind::Left | TokenKind::Right => Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnexpectedToken,
                "Expected value token",
                token.position,
            )),
        }
    }

    fn account_node(&mut self, position: Position) -> Result<(), Error> {
        self.node_count = self.node_count.saturating_add(1);
        if self.node_count > self.limits.max_nodes {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::ResourceLimit,
                "S-expression tree exceeds max_nodes",
                position,
            ));
        }
        Ok(())
    }

    fn peek_kind(&mut self) -> Result<Option<TokenKind>, Error> {
        Ok(self.peek_token()?.map(|token| token.kind))
    }

    fn peek_token(&mut self) -> Result<Option<Token<'a>>, Error> {
        self.fill_lookahead()?;
        Ok(self.lookahead.clone())
    }

    fn take_token(&mut self) -> Result<Option<Token<'a>>, Error> {
        self.fill_lookahead()?;
        Ok(self.lookahead.take())
    }

    fn take_required(&mut self, message: &'static str) -> Result<Token<'a>, Error> {
        let position = self.lexer.position();
        self.take_token()?.ok_or_else(|| {
            Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedOpeningParenthesis,
                message,
                position,
            )
        })
    }

    fn fill_lookahead(&mut self) -> Result<(), Error> {
        if self.lookahead.is_some() {
            return Ok(());
        }
        self.lookahead = match self.lexer.next() {
            Some(result) => Some(result?),
            None => None,
        };
        Ok(())
    }

    fn expect(&mut self, expected: TokenKind, message: &'static str) -> Result<Token<'a>, Error> {
        let Some(token) = self.take_token()? else {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedOpeningParenthesis,
                message,
                self.lexer.position(),
            ));
        };
        if token.kind != expected {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnexpectedToken,
                message,
                token.position,
            )
            .with_token(token.lexeme));
        }
        Ok(token)
    }
}

/// Deterministically build a KiCad S-expression from an owned tree.
pub fn build(value: &Sexp) -> Result<String, Error> {
    build_with_limit(value, usize::MAX)
}

/// Deterministically build while refusing to allocate beyond an output limit.
pub fn build_with_limit(value: &Sexp, max_output_bytes: usize) -> Result<String, Error> {
    let mut output = BuildOutput::new(max_output_bytes);
    build_into(value, "", &mut output)?;
    Ok(output.text)
}

/// Reformat source tokens without materializing an owned expression tree.
///
/// Token spelling is retained exactly. Whitespace and whole-line comments are
/// intentionally normalized, matching the Python foundation formatter.
pub fn format(source: &str, options: FormatOptions) -> Result<String, Error> {
    let mut output = String::with_capacity(source.len().saturating_add(1));
    let mut depth = 0_usize;
    let mut last_kind = None;

    for token in Lexer::new(source) {
        let token = token?;
        match token.kind {
            TokenKind::Left => {
                if !output.is_empty() {
                    if depth <= options.max_nesting {
                        trim_one_trailing_space(&mut output);
                        output.push('\n');
                        push_spaces(&mut output, options.indentation_size.saturating_mul(depth));
                    } else if last_kind == Some(TokenKind::Right) {
                        output.push(' ');
                    }
                }
                depth = depth.saturating_add(1);
                output.push('(');
            }
            TokenKind::Right => {
                trim_one_trailing_space(&mut output);
                depth = depth.checked_sub(1).ok_or_else(|| {
                    Error::at(
                        ErrorPhase::Tree,
                        ErrorKind::UnbalancedClosingParenthesis,
                        "Unbalanced closing parenthesis",
                        token.position,
                    )
                })?;
                if depth < options.max_nesting {
                    output.push('\n');
                    push_spaces(&mut output, options.indentation_size.saturating_mul(depth));
                } else if last_kind == Some(TokenKind::Right) {
                    output.push(' ');
                }
                output.push(')');
            }
            _ => {
                if last_kind == Some(TokenKind::Right) {
                    output.push(' ');
                }
                output.push_str(token.lexeme);
                output.push(' ');
            }
        }
        last_kind = Some(token.kind);
    }

    if depth != 0 {
        return Err(Error::at(
            ErrorPhase::Tree,
            ErrorKind::UnbalancedOpeningParenthesis,
            "Unbalanced opening parenthesis",
            Position::START,
        ));
    }
    output.push('\n');
    Ok(output)
}

fn trim_one_trailing_space(output: &mut String) {
    if output.ends_with(' ') {
        output.pop();
    }
}

fn push_spaces(output: &mut String, count: usize) {
    output.extend(std::iter::repeat_n(' ', count));
}

/// Apply sorted, non-overlapping source patches with a conservative output limit.
pub fn apply_patches(source: &str, patches: &[Patch<'_>]) -> Result<String, Error> {
    apply_patches_with_limit(source, patches, Limits::default().max_source_bytes)
}

/// Apply sorted, non-overlapping patches while bounding result bytes.
pub fn apply_patches_with_limit(
    source: &str,
    patches: &[Patch<'_>],
    max_output_bytes: usize,
) -> Result<String, Error> {
    let mut last_end = 0;
    let mut output_size = source.len();

    for patch in patches {
        let valid_range = patch.start_offset <= patch.end_offset
            && patch.end_offset <= source.len()
            && source.is_char_boundary(patch.start_offset)
            && source.is_char_boundary(patch.end_offset);
        if !valid_range || patch.start_offset < last_end {
            let bounded_offset = patch.start_offset.min(source.len());
            return Err(Error {
                phase: ErrorPhase::Build,
                kind: ErrorKind::InvalidPatch,
                message: Cow::Borrowed(
                    "Patches must be sorted, non-overlapping, in-range UTF-8 byte ranges",
                ),
                position: Some(position_at_text_offset(source, bounded_offset)),
                token: None,
            });
        }

        let removed = patch.end_offset - patch.start_offset;
        output_size = output_size
            .checked_sub(removed)
            .and_then(|size| size.checked_add(patch.replacement.len()))
            .ok_or_else(|| {
                Error::build(
                    ErrorKind::ResourceLimit,
                    "Patched S-expression output size overflowed",
                )
            })?;
        if output_size > max_output_bytes {
            return Err(Error::build(
                ErrorKind::ResourceLimit,
                "Patched S-expression output exceeds max_output_bytes",
            ));
        }
        last_end = patch.end_offset;
    }

    let mut output = String::with_capacity(output_size);
    last_end = 0;
    for patch in patches {
        output.push_str(&source[last_end..patch.start_offset]);
        output.push_str(&patch.replacement);
        last_end = patch.end_offset;
    }
    output.push_str(&source[last_end..]);
    Ok(output)
}

struct BuildOutput {
    text: String,
    max_bytes: usize,
}

impl BuildOutput {
    fn new(max_bytes: usize) -> Self {
        Self {
            text: String::new(),
            max_bytes,
        }
    }

    fn push(&mut self, character: char) -> Result<(), Error> {
        let required = character.len_utf8();
        self.reserve(required)?;
        self.text.push(character);
        Ok(())
    }

    fn push_str(&mut self, value: &str) -> Result<(), Error> {
        self.reserve(value.len())?;
        self.text.push_str(value);
        Ok(())
    }

    fn reserve(&self, additional: usize) -> Result<(), Error> {
        if self
            .text
            .len()
            .checked_add(additional)
            .is_none_or(|length| length > self.max_bytes)
        {
            Err(Error::build(
                ErrorKind::ResourceLimit,
                "Built S-expression output exceeds max_output_bytes",
            ))
        } else {
            Ok(())
        }
    }
}

impl fmt::Write for BuildOutput {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.push_str(value).map_err(|_| fmt::Error)
    }
}

fn build_into(value: &Sexp, indent: &str, output: &mut BuildOutput) -> Result<(), Error> {
    match value {
        Sexp::List(values) => {
            output.push('(')?;
            let mut last_was_list = false;
            for (index, child) in values.iter().enumerate() {
                if index > 0 {
                    if matches!(child, Sexp::List(_)) {
                        output.push_str("\n\t")?;
                        output.push_str(indent)?;
                    } else {
                        output.push(' ')?;
                    }
                }
                let mut child_indent = String::with_capacity(indent.len() + 1);
                child_indent.push_str(indent);
                child_indent.push('\t');
                build_into(child, &child_indent, output)?;
                last_was_list = matches!(child, Sexp::List(_));
            }
            if last_was_list {
                output.push('\n')?;
                output.push_str(indent)?;
            }
            output.push(')')?;
            Ok(())
        }
        Sexp::Atom(value) => {
            if value.is_empty() {
                output.push_str("\"\"")?;
            } else if value
                .chars()
                .any(|character| character.is_whitespace() || matches!(character, '(' | ')'))
                || value.starts_with('"')
            {
                output.push('"')?;
                escape_quoted_into(value, output)?;
                output.push('"')?;
            } else {
                output.push_str(value)?;
            }
            Ok(())
        }
        Sexp::Quoted(value) => {
            output.push('"')?;
            escape_quoted_into(value, output)?;
            output.push('"')?;
            Ok(())
        }
        Sexp::Integer(value) => write!(output, "{value}").map_err(|_| build_output_error()),
        Sexp::Float(value) => {
            if !value.is_finite() {
                return Err(Error::build(
                    ErrorKind::InvalidBuildValue,
                    "Cannot build a non-finite float",
                ));
            }
            if *value == 0.0 {
                output.push('0')?;
            } else {
                write!(output, "{value}").map_err(|_| build_output_error())?;
            }
            Ok(())
        }
    }
}

fn build_output_error() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "Built S-expression output exceeds max_output_bytes",
    )
}

fn escape_quoted_into(value: &str, output: &mut BuildOutput) -> Result<(), Error> {
    for character in value.chars() {
        match character {
            '\n' => output.push_str("\\n")?,
            '\r' => output.push_str("\\r")?,
            '\\' => output.push_str("\\\\")?,
            '"' => output.push_str("\\\"")?,
            _ => output.push(character)?,
        }
    }
    Ok(())
}

pub(crate) fn decode_quoted(lexeme: &str) -> String {
    decode_quoted_with_limit(lexeme, usize::MAX).unwrap_or_default()
}

#[allow(
    clippy::cognitive_complexity,
    reason = "pre-standard bounded escape decoder retained under the structural ratchet"
)]
fn decode_quoted_with_limit(lexeme: &str, max_bytes: usize) -> Option<String> {
    let body = &lexeme[1..lexeme.len() - 1];
    let mut output = String::with_capacity(body.len().min(max_bytes));
    let mut characters = body.chars().peekable();

    while let Some(mut character) = characters.next() {
        if character == '\r' {
            if characters.peek() == Some(&'\n') {
                characters.next();
            }
            character = '\n';
        }
        if character != '\\' {
            output.push(character);
        } else {
            let Some(mut next) = characters.next() else {
                output.push('\\');
                return (output.len() <= max_bytes).then_some(output);
            };
            if next == '\r' {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                next = '\n';
            }
            match next {
                '"' | '\\' => output.push(next),
                'a' => output.push('\u{0007}'),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                'v' => output.push('\u{000b}'),
                'x' => {
                    let mut digits = String::new();
                    for _ in 0..2 {
                        match characters.peek().copied() {
                            Some(digit) if digit.is_ascii_hexdigit() => {
                                digits.push(digit);
                                characters.next();
                            }
                            _ => break,
                        }
                    }
                    if digits.is_empty() {
                        output.push('x');
                    } else if let Ok(value) = u8::from_str_radix(&digits, 16) {
                        output.push(char::from(value));
                    }
                }
                digit @ '0'..='7' => {
                    let mut digits = String::from(digit);
                    for _ in 1..3 {
                        match characters.peek().copied() {
                            Some(next_digit @ '0'..='7') => {
                                digits.push(next_digit);
                                characters.next();
                            }
                            _ => break,
                        }
                    }
                    if let Ok(value) = u8::from_str_radix(&digits, 8) {
                        output.push(char::from(value));
                    }
                }
                other => {
                    output.push('\\');
                    output.push(other);
                }
            }
        }
        if output.len() > max_bytes {
            return None;
        }
    }
    Some(output)
}

fn classify_bare_token(token: &str) -> TokenKind {
    if !matches_number_grammar(token) {
        return TokenKind::Atom;
    }
    if token.contains('.') || token.contains('e') || token.contains('E') {
        TokenKind::Float
    } else {
        TokenKind::Integer
    }
}

fn matches_number_grammar(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut index = usize::from(matches!(bytes[0], b'+' | b'-'));
    if index >= bytes.len() {
        return false;
    }

    let digits_before = consume_digits(bytes, &mut index);
    let mut digits_after = 0;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        digits_after = consume_digits(bytes, &mut index);
    }
    if digits_before == 0 && digits_after == 0 {
        return false;
    }

    if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
        index += 1;
        if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
            index += 1;
        }
        if consume_digits(bytes, &mut index) == 0 {
            return false;
        }
    }
    index == bytes.len()
}

fn consume_digits(bytes: &[u8], index: &mut usize) -> usize {
    let start = *index;
    while *index < bytes.len() && bytes[*index].is_ascii_digit() {
        *index += 1;
    }
    *index - start
}

fn is_teardrop_field_key(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Atom
        && (is_teardrop_numeric_key(token.lexeme) || is_teardrop_bool_key(token.lexeme))
}

pub(crate) fn is_teardrop_numeric_key(key: &str) -> bool {
    matches!(
        key,
        "best_length_ratio"
            | "max_length"
            | "best_width_ratio"
            | "max_width"
            | "curve_points"
            | "filter_ratio"
    )
}

pub(crate) fn is_teardrop_bool_key(key: &str) -> bool {
    matches!(
        key,
        "enabled" | "allow_two_segments" | "prefer_zone_connections" | "curved_edges"
    )
}

fn position_at_valid_prefix(prefix: &[u8]) -> Position {
    let Ok(text) = std::str::from_utf8(prefix) else {
        return Position {
            offset: prefix.len(),
            line: 1,
            column: 1,
        };
    };
    let mut lexer = Lexer::new(text);
    lexer.advance_to(text.len());
    lexer.position()
}

fn position_at_text_offset(source: &str, offset: usize) -> Position {
    let mut lexer = Lexer::new(source);
    lexer.advance_to(offset);
    lexer.position()
}

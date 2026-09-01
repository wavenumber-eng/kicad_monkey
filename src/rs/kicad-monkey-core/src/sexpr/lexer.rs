use super::*;

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
        match self.source.as_bytes()[start.offset] {
            b'(' => Ok(Some(self.single_byte_token(start, TokenKind::Left))),
            b')' => Ok(Some(self.single_byte_token(start, TokenKind::Right))),
            b'"' => self.scan_quoted(start).map(Some),
            _ => Ok(Some(self.scan_atom(start))),
        }
    }

    fn single_byte_token(&mut self, start: Position, kind: TokenKind) -> Token<'a> {
        self.advance_ascii_run(start.offset + 1);
        Token {
            kind,
            lexeme: &self.source[start.offset..start.offset + 1],
            position: start,
        }
    }

    fn skip_layout(&mut self) {
        loop {
            self.skip_whitespace();
            if !self.starts_line_comment() {
                return;
            }
            self.skip_line_comment();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.position.offset < self.source.len() {
            let offset = self.position.offset;
            match self.source.as_bytes()[offset] {
                b' ' | b'\t' | 0x0b | 0x0c => self.skip_ascii_horizontal_space(),
                b'\r' | b'\n' => self.advance_newline(),
                byte if byte.is_ascii() => break,
                _ => {
                    let character = self.current_non_ascii_char();
                    if !character.is_whitespace() {
                        break;
                    }
                    self.advance_non_ascii(character);
                }
            }
        }
    }

    fn skip_ascii_horizontal_space(&mut self) {
        let mut end = self.position.offset + 1;
        while end < self.source.len()
            && matches!(self.source.as_bytes()[end], b' ' | b'\t' | 0x0b | 0x0c)
        {
            end += 1;
        }
        self.advance_ascii_run(end);
    }

    fn starts_line_comment(&self) -> bool {
        self.position.offset < self.source.len()
            && self.source.as_bytes()[self.position.offset] == b'#'
            && self.source[self.line_start..self.position.offset]
                .trim()
                .is_empty()
    }

    fn skip_line_comment(&mut self) {
        let start = self.position.offset;
        let end = self.source.as_bytes()[start..]
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map_or(self.source.len(), |relative| start + relative);
        self.advance_text_run(end);
    }

    fn scan_quoted(&mut self, start: Position) -> Result<Token<'a>, Error> {
        self.advance_ascii_run(start.offset + 1);
        while self.position.offset < self.source.len() {
            let offset = self.position.offset;
            match self.source.as_bytes()[offset] {
                b'"' => {
                    self.advance_ascii_run(offset + 1);
                    return Ok(Token {
                        kind: TokenKind::QuotedString,
                        lexeme: &self.source[start.offset..self.position.offset],
                        position: start,
                    });
                }
                b'\\' => {
                    self.advance_ascii_run(offset + 1);
                    self.advance_one_scalar();
                }
                b'\r' | b'\n' => self.advance_newline(),
                byte if byte.is_ascii() => self.advance_quoted_ascii_run(),
                _ => {
                    let character = self.current_non_ascii_char();
                    self.advance_non_ascii(character);
                }
            }
        }
        Err(Error::at(
            ErrorPhase::Lex,
            ErrorKind::UnterminatedString,
            "Unterminated delimited string",
            start,
        ))
    }

    fn advance_quoted_ascii_run(&mut self) {
        let mut end = self.position.offset + 1;
        while end < self.source.len()
            && self.source.as_bytes()[end].is_ascii()
            && !matches!(self.source.as_bytes()[end], b'"' | b'\\' | b'\r' | b'\n')
        {
            end += 1;
        }
        self.advance_ascii_run(end);
    }

    fn scan_atom(&mut self, start: Position) -> Token<'a> {
        while self.position.offset < self.source.len() {
            let offset = self.position.offset;
            let byte = self.source.as_bytes()[offset];
            if byte.is_ascii() {
                if byte.is_ascii_whitespace() || matches!(byte, b'(' | b')') {
                    break;
                }
                self.advance_atom_ascii_run();
            } else {
                let character = self.current_non_ascii_char();
                if character.is_whitespace() {
                    break;
                }
                self.advance_non_ascii(character);
            }
        }
        let lexeme = &self.source[start.offset..self.position.offset];
        Token {
            kind: classify_bare_token(lexeme),
            lexeme,
            position: start,
        }
    }

    fn advance_atom_ascii_run(&mut self) {
        let mut end = self.position.offset + 1;
        while end < self.source.len() {
            let candidate = self.source.as_bytes()[end];
            if !candidate.is_ascii()
                || candidate.is_ascii_whitespace()
                || matches!(candidate, b'(' | b')')
            {
                break;
            }
            end += 1;
        }
        self.advance_ascii_run(end);
    }

    fn current_non_ascii_char(&self) -> char {
        self.source[self.position.offset..]
            .chars()
            .next()
            .expect("offset is in bounds and source is valid UTF-8")
    }

    fn advance_one_scalar(&mut self) {
        let offset = self.position.offset;
        if offset >= self.source.len() {
            return;
        }
        match self.source.as_bytes()[offset] {
            b'\r' | b'\n' => self.advance_newline(),
            byte if byte.is_ascii() => self.advance_ascii_run(offset + 1),
            _ => {
                let character = self.current_non_ascii_char();
                self.advance_non_ascii(character);
            }
        }
    }

    fn advance_newline(&mut self) {
        let offset = self.position.offset;
        if self.source.as_bytes()[offset] == b'\r'
            && self.source.as_bytes().get(offset + 1) == Some(&b'\n')
        {
            self.position.offset += 2;
        } else {
            self.position.offset += 1;
        }
        self.position.line += 1;
        self.position.column = 1;
        self.line_start = self.position.offset;
    }

    fn advance_ascii_run(&mut self, end: usize) {
        debug_assert!(end >= self.position.offset);
        debug_assert!(
            self.source.as_bytes()[self.position.offset..end]
                .iter()
                .all(u8::is_ascii)
        );
        debug_assert!(
            !self.source.as_bytes()[self.position.offset..end]
                .iter()
                .any(|byte| matches!(byte, b'\r' | b'\n'))
        );
        self.position.column += end - self.position.offset;
        self.position.offset = end;
    }

    fn advance_non_ascii(&mut self, character: char) {
        debug_assert!(!character.is_ascii());
        self.position.offset += character.len_utf8();
        self.position.column += 1;
    }

    fn advance_text_run(&mut self, end: usize) {
        debug_assert!(
            !self.source.as_bytes()[self.position.offset..end]
                .iter()
                .any(|byte| matches!(byte, b'\r' | b'\n'))
        );
        self.position.column += self.source[self.position.offset..end].chars().count();
        self.position.offset = end;
    }

    pub(super) fn advance_to(&mut self, end: usize) {
        while self.position.offset < end {
            let offset = self.position.offset;
            match self.source.as_bytes()[offset] {
                b'\r' | b'\n' => self.advance_newline(),
                byte if byte.is_ascii() => {
                    let mut run_end = offset + 1;
                    while run_end < end
                        && self.source.as_bytes()[run_end].is_ascii()
                        && !matches!(self.source.as_bytes()[run_end], b'\r' | b'\n')
                    {
                        run_end += 1;
                    }
                    self.advance_ascii_run(run_end);
                }
                _ => {
                    let character = self.current_non_ascii_char();
                    self.advance_non_ascii(character);
                }
            }
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

use super::*;

pub(super) fn scan_form_spans_unsorted(
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
    let mut scanner = FormScanner::new(selector, limits);
    for token in Lexer::new(source) {
        scanner.consume(token?)?;
    }
    scanner.finish()
}

struct FormScanner<'selector, 'source> {
    selector: &'selector Selector,
    limits: ProjectionLimits,
    stack: Vec<Frame<Cow<'source, str>>>,
    spans: Vec<FormSpan>,
    saw_root: bool,
}

impl<'selector, 'source> FormScanner<'selector, 'source> {
    fn new(selector: &'selector Selector, limits: ProjectionLimits) -> Self {
        Self {
            selector,
            limits,
            stack: Vec::new(),
            spans: Vec::new(),
            saw_root: false,
        }
    }

    fn consume(&mut self, token: Token<'source>) -> Result<(), Error> {
        match token.kind {
            TokenKind::Left => self.open(token.position),
            TokenKind::Right => self.close(token.position),
            _ => self.scalar(token),
        }
    }

    fn open(&mut self, position: Position) -> Result<(), Error> {
        if self.stack.len() > self.limits.max_depth {
            return Err(resource_error(
                "Projection nesting exceeds max_depth",
                position,
            ));
        }
        self.finish_missing_parent_head();
        let visible = self.stack.last().is_none_or(|parent| parent.scan_children);
        self.stack.push(Frame {
            head: None,
            depth: self.stack.len(),
            start: position,
            visible,
            scan_children: visible,
            awaiting_head: true,
            teardrop_bare_field: TeardropBareField::None,
        });
        self.saw_root = true;
        Ok(())
    }

    fn finish_missing_parent_head(&mut self) {
        if self.stack.last().is_some_and(|parent| parent.awaiting_head) {
            let (visible, depth) = {
                let parent = self.stack.last_mut().expect("checked above");
                parent.awaiting_head = false;
                (parent.visible, parent.depth)
            };
            let scan_children = visible
                && self.selector.should_scan_children(
                    None,
                    self.stack.iter().filter_map(|frame| frame.head.as_deref()),
                    depth,
                );
            self.stack.last_mut().expect("checked above").scan_children = scan_children;
        }
    }

    fn close(&mut self, position: Position) -> Result<(), Error> {
        if self.consume_teardrop_close() {
            return Ok(());
        }
        let frame = self.stack.last().ok_or_else(|| {
            Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedClosingParenthesis,
                "Unbalanced closing parenthesis",
                position,
            )
        })?;
        let selected = frame.visible
            && self.selector.matches_parts(
                frame.head.as_deref(),
                self.stack.iter().filter_map(|item| item.head.as_deref()),
                frame.depth,
            );
        let frame = self.stack.pop().expect("checked above");
        if !selected {
            return Ok(());
        }
        let mut path = self
            .stack
            .iter()
            .filter_map(|item| item.head.as_ref().map(|head| head.to_string()))
            .collect::<Vec<_>>();
        if let Some(head) = frame.head.as_ref() {
            path.push(head.to_string());
        }
        let span = closed_span(frame, path, position);
        self.retain_selected_span(span, position)
    }

    fn consume_teardrop_close(&mut self) -> bool {
        let Some(frame) = self.stack.last_mut() else {
            return false;
        };
        if frame.head.as_deref() != Some("teardrops")
            || frame.teardrop_bare_field != TeardropBareField::AwaitingClose
        {
            return false;
        }
        frame.teardrop_bare_field = TeardropBareField::None;
        true
    }

    fn retain_selected_span(&mut self, span: FormSpan, position: Position) -> Result<(), Error> {
        self.spans.push(span);
        if self.spans.len() > self.limits.max_selected_forms {
            return Err(resource_error(
                "Projection selection exceeds max_selected_forms",
                position,
            ));
        }
        Ok(())
    }

    fn scalar(&mut self, token: Token<'source>) -> Result<(), Error> {
        let Some(frame) = self.stack.last() else {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::MissingOpeningParenthesis,
                "Missing initial opening parenthesis",
                token.position,
            )
            .with_token(token.lexeme));
        };
        if frame.awaiting_head {
            return self.set_frame_head(token);
        }
        let frame = self.stack.last_mut().expect("checked above");
        update_frame_teardrop_state(frame, token.lexeme);
        Ok(())
    }

    fn finish(self) -> Result<Vec<FormSpan>, Error> {
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

    fn set_frame_head(&mut self, token: Token<'source>) -> Result<(), Error> {
        if token.lexeme.len() > self.limits.max_head_bytes {
            return Err(resource_error(
                "Projection form head exceeds max_head_bytes",
                token.position,
            ));
        }
        let head = token_head(&token);
        let (visible, depth) = {
            let frame = self.stack.last_mut().expect("head has an active frame");
            frame.head = Some(head);
            frame.awaiting_head = false;
            (frame.visible, frame.depth)
        };
        let scan_children = visible
            && self.selector.should_scan_children(
                self.stack.last().and_then(|frame| frame.head.as_deref()),
                self.stack.iter().filter_map(|frame| frame.head.as_deref()),
                depth,
            );
        self.stack
            .last_mut()
            .expect("head has an active frame")
            .scan_children = scan_children;
        Ok(())
    }
}

fn closed_span(frame: Frame<Cow<'_, str>>, path: Vec<String>, position: Position) -> FormSpan {
    FormSpan {
        head: frame.head.map(Cow::into_owned),
        path,
        depth: frame.depth,
        range: frame.start.offset..position.offset + 1,
        start: frame.start,
        end: Position {
            offset: position.offset + 1,
            line: position.line,
            column: position.column + 1,
        },
    }
}

fn update_frame_teardrop_state(frame: &mut Frame<Cow<'_, str>>, lexeme: &str) {
    if frame.head.as_deref() != Some("teardrops") {
        return;
    }
    frame.teardrop_bare_field = match frame.teardrop_bare_field {
        TeardropBareField::None if is_teardrop_numeric_key(lexeme) => {
            TeardropBareField::AwaitingValue
        }
        TeardropBareField::AwaitingValue => TeardropBareField::AwaitingClose,
        state => state,
    };
}

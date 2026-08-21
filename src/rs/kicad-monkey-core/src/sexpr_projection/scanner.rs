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

struct FormScanner<'a> {
    selector: &'a Selector,
    limits: ProjectionLimits,
    stack: Vec<Frame>,
    spans: Vec<FormSpan>,
    saw_root: bool,
}

impl<'a> FormScanner<'a> {
    fn new(selector: &'a Selector, limits: ProjectionLimits) -> Self {
        Self {
            selector,
            limits,
            stack: Vec::new(),
            spans: Vec::new(),
            saw_root: false,
        }
    }

    fn consume(&mut self, token: Token<'_>) -> Result<(), Error> {
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
        let path = self
            .stack
            .last()
            .map_or_else(Vec::new, |parent| parent.path.clone());
        self.stack.push(Frame {
            head: None,
            path,
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
        if let Some(parent) = self.stack.last_mut()
            && parent.awaiting_head
        {
            parent.awaiting_head = false;
            parent.scan_children = parent.visible
                && self
                    .selector
                    .should_scan_children(None, &parent.path, parent.depth);
        }
    }

    fn close(&mut self, position: Position) -> Result<(), Error> {
        if self.consume_teardrop_close() {
            return Ok(());
        }
        let frame = self.stack.pop().ok_or_else(|| {
            Error::at(
                ErrorPhase::Tree,
                ErrorKind::UnbalancedClosingParenthesis,
                "Unbalanced closing parenthesis",
                position,
            )
        })?;
        let visible = frame.visible;
        let span = closed_span(frame, position);
        self.retain_selected_span(visible, span, position)
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

    fn retain_selected_span(
        &mut self,
        visible: bool,
        span: FormSpan,
        position: Position,
    ) -> Result<(), Error> {
        if !visible || !self.selector.matches(&span) {
            return Ok(());
        }
        self.spans.push(span);
        if self.spans.len() > self.limits.max_selected_forms {
            return Err(resource_error(
                "Projection selection exceeds max_selected_forms",
                position,
            ));
        }
        Ok(())
    }

    fn scalar(&mut self, token: Token<'_>) -> Result<(), Error> {
        let Some(frame) = self.stack.last_mut() else {
            return Err(Error::at(
                ErrorPhase::Tree,
                ErrorKind::MissingOpeningParenthesis,
                "Missing initial opening parenthesis",
                token.position,
            )
            .with_token(token.lexeme));
        };
        if frame.awaiting_head {
            return set_frame_head(frame, token, self.selector, self.limits);
        }
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
}

fn closed_span(frame: Frame, position: Position) -> FormSpan {
    FormSpan {
        head: frame.head,
        path: frame.path,
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

fn set_frame_head(
    frame: &mut Frame,
    token: Token<'_>,
    selector: &Selector,
    limits: ProjectionLimits,
) -> Result<(), Error> {
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
        && selector.should_scan_children(frame.head.as_deref(), &frame.path, frame.depth);
    Ok(())
}

fn update_frame_teardrop_state(frame: &mut Frame, lexeme: &str) {
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

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Position, Token, TokenKind, decode_quoted_with_limit,
};
use crate::worksheet::WorksheetLimits;

#[derive(Default)]
struct Frame<'a> {
    head: Option<&'a str>,
    scalar_count: usize,
    has_point: bool,
    data_parts: usize,
    data_bytes: usize,
    data_overflow: bool,
}

#[derive(Clone, Copy, Default)]
struct BitmapDataSummary {
    parts: usize,
    bytes: usize,
    overflow: bool,
}

pub(super) fn preflight_item(source: &str, limits: WorksheetLimits) -> Result<(), Error> {
    let mut state = State::default();
    for token in Lexer::new(source) {
        let token = token?;
        match token.kind {
            TokenKind::Left => state.open(limits),
            TokenKind::Right => state.close(limits)?,
            _ => state.scalar(token, limits)?,
        }
    }
    state.finish(limits)
}

#[derive(Default)]
struct State<'a> {
    frames: Vec<Frame<'a>>,
    point_sets: usize,
    points: usize,
    justify: usize,
    data: Option<BitmapDataSummary>,
    pngdata: Option<BitmapDataSummary>,
}

impl<'a> State<'a> {
    fn open(&mut self, limits: WorksheetLimits) {
        if self.frames.len() == 2
            && matches!(
                self.frames.last().and_then(|frame| frame.head),
                Some("data" | "pngdata")
            )
        {
            account_bitmap_part(
                self.frames.last_mut().expect("bitmap data frame"),
                0,
                limits,
            );
        }
        self.frames.push(Frame::default());
    }

    fn close(&mut self, limits: WorksheetLimits) -> Result<(), Error> {
        let Some(closed) = self.frames.pop() else {
            return Ok(());
        };
        let parent_head = self.frames.last().and_then(|frame| frame.head);
        self.close_geometry(&closed, parent_head, limits)?;
        if self.frames.len() == 1 {
            self.remember_bitmap_summary(&closed);
        }
        Ok(())
    }

    fn close_geometry(
        &mut self,
        closed: &Frame<'_>,
        parent_head: Option<&str>,
        limits: WorksheetLimits,
    ) -> Result<(), Error> {
        if closed.head == Some("xy") && parent_head == Some("pts") && closed.scalar_count >= 2 {
            self.points = self.points.checked_add(1).ok_or_else(limit_error)?;
            if self.points > limits.max_points_per_polygon {
                return Err(limit_error());
            }
            if let Some(parent) = self.frames.last_mut() {
                parent.has_point = true;
            }
        } else if closed.head == Some("pts") && parent_head == Some("polygon") && closed.has_point {
            self.point_sets = self.point_sets.checked_add(1).ok_or_else(limit_error)?;
            if self.point_sets > limits.max_point_sets_per_polygon {
                return Err(limit_error());
            }
        }
        Ok(())
    }

    fn scalar(&mut self, token: Token<'a>, limits: WorksheetLimits) -> Result<(), Error> {
        let depth = self.frames.len();
        let Some(frame) = self.frames.last_mut() else {
            return Ok(());
        };
        if frame.head.is_none() {
            frame.head = Some(token.lexeme);
            return Ok(());
        }
        frame.scalar_count = frame.scalar_count.saturating_add(1);
        if depth == 2 && frame.head == Some("justify") && is_justify_token(&token) {
            self.justify = self.justify.checked_add(1).ok_or_else(limit_error)?;
            if self.justify > limits.max_justify_tokens {
                return Err(limit_error());
            }
        }
        if depth == 2 && matches!(frame.head, Some("data" | "pngdata")) {
            let decoded_bytes = decoded_scalar_bytes(&token, limits.max_bitmap_data_bytes);
            account_bitmap_part(frame, decoded_bytes, limits);
        }
        Ok(())
    }

    fn remember_bitmap_summary(&mut self, closed: &Frame<'_>) {
        let summary = BitmapDataSummary {
            parts: closed.data_parts,
            bytes: closed.data_bytes,
            overflow: closed.data_overflow,
        };
        match closed.head {
            Some("data") if self.data.is_none() => self.data = Some(summary),
            Some("pngdata") if self.pngdata.is_none() => self.pngdata = Some(summary),
            _ => {}
        }
    }

    fn finish(self, limits: WorksheetLimits) -> Result<(), Error> {
        let selected = self.data.or(self.pngdata).unwrap_or_default();
        if selected.parts > limits.max_bitmap_data_parts
            || selected.bytes > limits.max_bitmap_data_bytes
            || selected.overflow
        {
            return Err(limit_error());
        }
        Ok(())
    }
}

fn decoded_scalar_bytes(token: &Token<'_>, maximum: usize) -> usize {
    if token.kind == TokenKind::QuotedString {
        decode_quoted_with_limit(token.lexeme, maximum)
            .map_or(maximum.saturating_add(1), |value| value.len())
    } else {
        token.lexeme.len()
    }
}

fn account_bitmap_part(frame: &mut Frame<'_>, decoded_bytes: usize, limits: WorksheetLimits) {
    frame.data_parts = frame.data_parts.saturating_add(1);
    frame.data_bytes = frame.data_bytes.saturating_add(decoded_bytes);
    frame.data_overflow = frame.data_overflow
        || frame.data_parts > limits.max_bitmap_data_parts
        || frame.data_bytes > limits.max_bitmap_data_bytes;
}

fn is_justify_token(token: &Token<'_>) -> bool {
    if token.kind == TokenKind::QuotedString {
        return decode_quoted_with_limit(token.lexeme, 6).is_some_and(|value| {
            matches!(
                value.as_str(),
                "left" | "center" | "right" | "top" | "bottom"
            )
        });
    }
    matches!(token.lexeme, "left" | "center" | "right" | "top" | "bottom")
}

fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "worksheet item exceeds configured limits",
        Position::START,
    )
}

//! Bounded KiCad outline-font text-box line breaking.

use crate::text_block_layout::{
    AggregateWork, LineBuildInputs, TextBlockLayoutLimits, TextBlockLayoutRequest, build_line,
    shaping_metadata_bytes, validate_request,
};
use crate::text_contours::HintedTextContourSession;
use crate::{TextContourError, TextContourErrorKind};
use std::ops::Range;

/// Independent ceilings for KiCad outline-font text-box line breaking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextLinebreakLimits {
    /// Shared shaping, outline, markup, and work ceilings for measuring words.
    pub layout: TextBlockLayoutLimits,
    /// Maximum word/space tokens retained while breaking one request.
    pub max_tokens: usize,
    /// Maximum UTF-8 bytes retained in the line-broken result.
    pub max_output_bytes: usize,
}

impl Default for TextLinebreakLimits {
    fn default() -> Self {
        Self {
            layout: TextBlockLayoutLimits::default(),
            max_tokens: 2 * 1024 * 1024,
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Insert KiCad-compatible outline-font text-box line breaks with one bounded
/// hinted face session. Markup groups remain atomic words, normal text keeps
/// one trailing space per token, and pending spaces trigger wrapping exactly
/// as `KIFONT::FONT::LinebreakText()` does.
pub fn linebreak_text_block_hinted_a0(
    font_bytes: &[u8],
    request: TextBlockLayoutRequest<'_>,
    column_width: f64,
    limits: TextLinebreakLimits,
) -> Result<String, TextContourError> {
    validate_request(request, limits.layout)?;
    if !column_width.is_finite() {
        return Err(invalid("$.column_width", "column width must be finite"));
    }
    if column_width <= 0.0 || request.shaping.text.is_empty() {
        return bounded_linebreak_output(request.shaping.text.to_owned(), limits);
    }
    if !request.shaping.text.contains(' ') {
        return bounded_linebreak_output(request.shaping.text.to_owned(), limits);
    }

    let session =
        HintedTextContourSession::new(font_bytes, request.shaping, limits.layout.contours)?;
    let run_metadata_bytes = shaping_metadata_bytes(request.shaping)?;
    let inputs = LineBuildInputs {
        session: &session,
        request,
        limits: limits.layout,
        run_metadata_bytes,
    };
    let mut state = LinebreakWork {
        markup_budget: 0,
        layout: AggregateWork::default(),
        token_count: 0,
        output: String::with_capacity(
            request
                .shaping
                .text
                .len()
                .min(limits.max_output_bytes)
                .min(4096),
        ),
    };

    for line_span in request.shaping.text.split_inclusive('\n') {
        let line_start = request.shaping.text[..].as_ptr() as usize;
        let span_start = line_span.as_ptr() as usize - line_start;
        let has_newline = line_span.ends_with('\n');
        let line_end = span_start + line_span.len() - usize::from(has_newline);
        linebreak_one_line(
            &request.shaping.text,
            span_start..line_end,
            has_newline,
            &inputs,
            column_width,
            limits,
            &mut state,
        )?;
    }
    bounded_linebreak_output(state.output, limits)
}

struct LinebreakWork {
    markup_budget: usize,
    layout: AggregateWork,
    token_count: usize,
    output: String,
}

fn linebreak_one_line(
    source: &str,
    line: Range<usize>,
    has_newline: bool,
    inputs: &LineBuildInputs<'_>,
    column_width: f64,
    limits: TextLinebreakLimits,
    state: &mut LinebreakWork,
) -> Result<(), TextContourError> {
    let tokens = linebreak_tokens(source, line, &mut state.token_count, limits.max_tokens)?;
    let space_span = tokens
        .iter()
        .flat_map(|token| token.parts.iter())
        .find_map(|part| {
            source[part.clone()]
                .find(' ')
                .map(|offset| (part.start + offset)..(part.start + offset + 1))
        });
    let space_width = match space_span {
        Some(span) => measure_line_span(inputs, span, &mut state.markup_budget, &mut state.layout)?,
        None => 0.0,
    };
    let mut line_width = 0.0;
    let mut pending_spaces = 0usize;
    let mut bury_mode = false;
    for token in tokens {
        let word_width = measure_linebreak_token(source, &token, inputs, state)?;
        let pending_width = pending_spaces as f64 * space_width;
        let overflow =
            line_width + pending_width + word_width > column_width - inputs.request.stroke_width;
        if overflow && pending_spaces > 0 {
            push_linebreak_text(&mut state.output, "\n", limits.max_output_bytes)?;
            line_width = 0.0;
            pending_spaces = 0;
            bury_mode = true;
        }
        if token.is_space_only(source) {
            pending_spaces = pending_spaces
                .checked_add(1)
                .ok_or_else(|| resource("$.tokens", "pending space count overflowed"))?;
            continue;
        }
        if bury_mode {
            bury_mode = false;
        } else {
            for _ in 0..pending_spaces {
                push_linebreak_text(&mut state.output, " ", limits.max_output_bytes)?;
            }
            line_width += pending_spaces as f64 * space_width;
        }
        let trailing_space = token.ends_with_space(source);
        append_linebreak_token(source, &token, trailing_space, limits, state)?;
        pending_spaces = usize::from(trailing_space);
        line_width += word_width;
    }
    if has_newline {
        push_linebreak_text(&mut state.output, "\n", limits.max_output_bytes)?;
    }
    Ok(())
}

fn measure_linebreak_token(
    source: &str,
    token: &LinebreakToken,
    inputs: &LineBuildInputs<'_>,
    state: &mut LinebreakWork,
) -> Result<f64, TextContourError> {
    token.parts.iter().try_fold(0.0, |width, part| {
        let measured = trim_one_trailing_space(source, part.clone());
        Ok(width
            + measure_line_span(
                inputs,
                measured,
                &mut state.markup_budget,
                &mut state.layout,
            )?)
    })
}

fn append_linebreak_token(
    source: &str,
    token: &LinebreakToken,
    trailing_space: bool,
    limits: TextLinebreakLimits,
    state: &mut LinebreakWork,
) -> Result<(), TextContourError> {
    for part in &token.parts {
        let value = &source[part.clone()];
        let value = if trailing_space && part == token.parts.last().unwrap() {
            value.strip_suffix(' ').unwrap_or(value)
        } else {
            value
        };
        push_linebreak_text(&mut state.output, value, limits.max_output_bytes)?;
    }
    Ok(())
}

#[derive(Debug)]
struct LinebreakToken {
    parts: Vec<Range<usize>>,
}

impl LinebreakToken {
    fn ends_with_space(&self, source: &str) -> bool {
        self.parts
            .last()
            .is_some_and(|part| source[part.clone()].ends_with(' '))
    }

    fn is_space_only(&self, source: &str) -> bool {
        self.parts.len() == 1 && source[self.parts[0].clone()] == *" "
    }
}

fn linebreak_tokens(
    source: &str,
    line: Range<usize>,
    token_count: &mut usize,
    max_tokens: usize,
) -> Result<Vec<LinebreakToken>, TextContourError> {
    let mut tokens: Vec<LinebreakToken> = Vec::new();
    let bytes = source.as_bytes();
    let mut index = line.start;
    while index < line.end {
        let part = next_linebreak_part(bytes, line.end, &mut index);
        *token_count = token_count
            .checked_add(1)
            .filter(|count| *count <= max_tokens)
            .ok_or_else(|| resource("$.tokens", "linebreak token limit exceeded"))?;
        let joins_previous = tokens
            .last()
            .is_some_and(|token| !token.ends_with_space(source));
        if joins_previous {
            tokens.last_mut().unwrap().parts.push(part);
        } else {
            tokens.push(LinebreakToken { parts: vec![part] });
        }
    }
    Ok(tokens)
}

fn next_linebreak_part(bytes: &[u8], line_end: usize, index: &mut usize) -> Range<usize> {
    if markup_group_starts(bytes, *index) {
        markup_group_part(bytes, line_end, index)
    } else {
        plain_linebreak_part(bytes, line_end, index)
    }
}

fn markup_group_starts(bytes: &[u8], index: usize) -> bool {
    matches!(bytes.get(index), Some(b'_' | b'^' | b'~')) && bytes.get(index + 1) == Some(&b'{')
}

fn markup_group_part(bytes: &[u8], line_end: usize, index: &mut usize) -> Range<usize> {
    let start = *index;
    *index += 2;
    let mut depth = 1usize;
    while *index < line_end && depth > 0 {
        if markup_group_starts(bytes, *index) {
            depth += 1;
            *index += 2;
        } else {
            depth -= usize::from(bytes[*index] == b'}');
            *index += 1;
        }
    }
    start..*index
}

fn plain_linebreak_part(bytes: &[u8], line_end: usize, index: &mut usize) -> Range<usize> {
    let start = *index;
    if bytes[*index] == b' ' {
        *index += 1;
        return start..*index;
    }
    while *index < line_end && bytes[*index] != b' ' && !markup_group_starts(bytes, *index) {
        *index += 1;
    }
    if *index < line_end && bytes[*index] == b' ' {
        *index += 1;
    }
    start..*index
}

fn trim_one_trailing_space(source: &str, span: Range<usize>) -> Range<usize> {
    if span.len() > 1 && source[span.clone()].ends_with(' ') {
        span.start..span.end - 1
    } else {
        span
    }
}

fn measure_line_span(
    inputs: &LineBuildInputs<'_>,
    span: Range<usize>,
    markup_budget: &mut usize,
    work: &mut AggregateWork,
) -> Result<f64, TextContourError> {
    Ok(build_line(inputs, span, markup_budget, work)?.width)
}

fn push_linebreak_text(
    output: &mut String,
    value: &str,
    max_output_bytes: usize,
) -> Result<(), TextContourError> {
    output
        .len()
        .checked_add(value.len())
        .filter(|length| *length <= max_output_bytes)
        .ok_or_else(|| resource("$.output", "linebreak output byte limit exceeded"))?;
    output.push_str(value);
    Ok(())
}

fn bounded_linebreak_output(
    output: String,
    limits: TextLinebreakLimits,
) -> Result<String, TextContourError> {
    if output.len() > limits.max_output_bytes {
        return Err(resource("$.output", "linebreak output byte limit exceeded"));
    }
    Ok(output)
}

fn invalid(path: &'static str, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::InvalidInput,
        path: path.to_owned(),
        message,
    }
}

fn resource(path: &'static str, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::ResourceLimit,
        path: path.to_owned(),
        message,
    }
}

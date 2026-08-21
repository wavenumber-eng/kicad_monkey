use super::*;

pub(super) fn scalar_values<'a>(source: &'a str, span: &FormSpan) -> Result<Vec<Token<'a>>, Error> {
    bounded_scalar_values(source, span, usize::MAX)
}

pub(super) fn bounded_scalar_values<'a>(
    source: &'a str,
    span: &FormSpan,
    maximum: usize,
) -> Result<Vec<Token<'a>>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected form opening parenthesis",
        )?;
        let _head = next_scalar(lexer.next(), "Expected form head")?;
        let mut values = Vec::new();
        let mut state = ScalarCollectionState::default();
        for token in lexer {
            let token = token?;
            if collect_scalar_token(&mut values, token, span, maximum, &mut state)? {
                break;
            }
        }
        Ok(values)
    })()
    .map_err(|error| rebase_error(error, span))
}

struct ScalarCollectionState {
    depth: usize,
    teardrop_bare_field: bool,
    teardrop_bare_value: bool,
}

impl Default for ScalarCollectionState {
    fn default() -> Self {
        Self {
            depth: 1,
            teardrop_bare_field: false,
            teardrop_bare_value: false,
        }
    }
}

fn collect_scalar_token<'a>(
    values: &mut Vec<Token<'a>>,
    token: Token<'a>,
    span: &FormSpan,
    maximum: usize,
    state: &mut ScalarCollectionState,
) -> Result<bool, Error> {
    match token.kind {
        TokenKind::Left => state.depth += 1,
        TokenKind::Right => return Ok(close_scalar_level(state)),
        _ if state.depth == 1 => push_scalar_token(values, token, span, maximum, state)?,
        _ => {}
    }
    Ok(false)
}

fn close_scalar_level(state: &mut ScalarCollectionState) -> bool {
    if state.depth == 1 && state.teardrop_bare_value {
        state.teardrop_bare_value = false;
        return false;
    }
    state.depth -= 1;
    state.depth == 0
}

fn push_scalar_token<'a>(
    values: &mut Vec<Token<'a>>,
    token: Token<'a>,
    span: &FormSpan,
    maximum: usize,
    state: &mut ScalarCollectionState,
) -> Result<(), Error> {
    if values.len() >= maximum {
        return Err(limit_error());
    }
    if span.head.as_deref() == Some("teardrops") {
        update_teardrop_scalar_state(token.lexeme, state);
    }
    values.push(token);
    Ok(())
}

fn update_teardrop_scalar_state(lexeme: &str, state: &mut ScalarCollectionState) {
    if state.teardrop_bare_field {
        state.teardrop_bare_field = false;
        state.teardrop_bare_value = true;
    } else if is_teardrop_numeric_key(lexeme) {
        state.teardrop_bare_field = true;
    }
}

pub(super) fn first_two_scalar_values<'a>(
    source: &'a str,
    span: &FormSpan,
) -> Result<Vec<Token<'a>>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected form opening parenthesis",
        )?;
        let _head = next_scalar(lexer.next(), "Expected form head")?;
        let mut values = Vec::with_capacity(2);
        let mut depth = 1usize;
        for token in lexer {
            let token = token?;
            match token.kind {
                TokenKind::Left => depth += 1,
                TokenKind::Right => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ if depth == 1 && values.len() < 2 => values.push(token),
                _ => {}
            }
        }
        Ok(values)
    })()
    .map_err(|error| rebase_error(error, span))
}

pub(super) fn direct_children(
    source: &str,
    parent: &FormSpan,
    max_selected_forms: usize,
    limits: PcbLimits,
) -> Result<Vec<FormSpan>, Error> {
    let text = parent.text(source)?;
    let local = scan_form_spans_with_limits(
        text,
        &Selector {
            min_depth: Some(1),
            max_depth: Some(1),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| rebase_error(error, parent))?;
    Ok(local
        .into_iter()
        .map(|mut span| {
            span.range.start += parent.range.start;
            span.range.end += parent.range.start;
            span.start = rebase_position(span.start, parent);
            span.end = rebase_position(span.end, parent);
            span.depth = 1;
            span.path = vec![
                parent.head.clone().unwrap_or_default(),
                span.head.clone().unwrap_or_default(),
            ];
            span
        })
        .collect())
}

pub(super) fn required_xy(
    source: &str,
    children: &[FormSpan],
    head: &str,
    parent: &FormSpan,
) -> Result<(f64, f64), Error> {
    let span = child(children, head)
        .ok_or_else(|| source_error("Expected coordinate form", parent.start))?;
    let values = scalar_values(source, span)?;
    Ok((
        required_f64(values.first(), "Expected x coordinate", span)?,
        required_f64(values.get(1), "Expected y coordinate", span)?,
    ))
}

pub(super) fn required_point(
    source: &str,
    children: &[FormSpan],
    head: &str,
    parent: &FormSpan,
) -> Result<PcbPoint, Error> {
    let (x, y) = required_xy(source, children, head, parent)?;
    Ok(PcbPoint { x, y })
}

pub(super) fn optional_child_point(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<PcbPoint>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    Ok(Some(PcbPoint {
        x: required_f64(values.first(), "Expected x coordinate", span)?,
        y: required_f64(values.get(1), "Expected y coordinate", span)?,
    }))
}

pub(super) fn points_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<Vec<PcbPoint>, Error> {
    direct_children(source, span, limits.max_graphic_points, limits)
        .map_err(|error| {
            if error.kind == ErrorKind::ResourceLimit {
                Error::at(
                    ErrorPhase::Tree,
                    ErrorKind::ResourceLimit,
                    "PCB graphic points exceed max_graphic_points",
                    error.position.unwrap_or(Position::START),
                )
            } else {
                error
            }
        })?
        .into_iter()
        .filter(|point| point.head.as_deref() == Some("xy"))
        .map(|point| {
            let values = scalar_values(source, &point)?;
            Ok(PcbPoint {
                x: required_f64(values.first(), "Expected point x", &point)?,
                y: required_f64(values.get(1), "Expected point y", &point)?,
            })
        })
        .collect()
}

pub(super) fn optional_pair(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 2],
) -> Result<[f64; 2], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    Ok([
        required_f64(values.first(), "Expected first numeric value", span)?,
        required_f64(values.get(1), "Expected second numeric value", span)?,
    ])
}

pub(super) fn optional_coordinate_pair(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<[f64; 2], Error> {
    let Some(span) = child(children, head) else {
        return Ok([0.0, 0.0]);
    };
    let values = first_two_scalar_values(source, span)?;
    Ok([
        optional_f64(values.first(), span)?.unwrap_or_default(),
        optional_f64(values.get(1), span)?.unwrap_or_default(),
    ])
}

pub(super) fn optional_vector(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 3],
) -> Result<[f64; 3], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    Ok([
        required_f64(values.first(), "Expected first numeric value", span)?,
        required_f64(values.get(1), "Expected second numeric value", span)?,
        optional_f64(values.get(2), span)?.unwrap_or(default[2]),
    ])
}

pub(super) fn nested_xyz(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 3],
    limits: PcbLimits,
) -> Result<[f64; 3], Error> {
    let Some(container) = child(children, head) else {
        return Ok(default);
    };
    let nested = direct_children(source, container, limits.max_model_children, limits)?;
    let Some(xyz) = child(&nested, "xyz") else {
        return Ok(default);
    };
    let values = scalar_values(source, xyz)?;
    Ok([
        required_f64(values.first(), "Expected model x value", xyz)?,
        required_f64(values.get(1), "Expected model y value", xyz)?,
        required_f64(values.get(2), "Expected model z value", xyz)?,
    ])
}

pub(super) fn child<'a>(children: &'a [FormSpan], head: &str) -> Option<&'a FormSpan> {
    children
        .iter()
        .find(|span| span.head.as_deref() == Some(head))
}

pub(super) fn first_string(source: &str, span: &FormSpan) -> Result<Option<String>, Error> {
    Ok(first_scalar_value(source, span)?.as_ref().map(token_string))
}

pub(super) fn first_f64(source: &str, span: &FormSpan) -> Result<Option<f64>, Error> {
    let value = first_scalar_value(source, span)?;
    optional_f64(value.as_ref(), span)
}

pub(super) fn first_i64(source: &str, span: &FormSpan) -> Result<Option<i64>, Error> {
    first_scalar_value(source, span)?
        .as_ref()
        .map(|token| parse_i64(token, span))
        .transpose()
}

pub(super) fn first_scalar_value<'a>(
    source: &'a str,
    span: &FormSpan,
) -> Result<Option<Token<'a>>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected form opening parenthesis",
        )?;
        let _head = next_scalar(lexer.next(), "Expected form head")?;
        let mut depth = 1usize;
        for token in lexer {
            let token = token?;
            match token.kind {
                TokenKind::Left => depth += 1,
                TokenKind::Right => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(None);
                    }
                }
                _ if depth == 1 => return Ok(Some(token)),
                _ => {}
            }
        }
        Ok(None)
    })()
    .map_err(|error| rebase_error(error, span))
}

pub(super) fn optional_child_string(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<String>, Error> {
    child(children, head)
        .map(|span| first_string(source, span))
        .transpose()
        .map(Option::flatten)
}

pub(super) fn optional_child_f64(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<f64>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    optional_f64(values.first(), span)
}

pub(super) fn optional_child_i64(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<i64>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    values
        .first()
        .map(|token| parse_i64(token, span))
        .transpose()
}

pub(super) fn child_strings(
    source: &str,
    children: &[FormSpan],
    head: &str,
    maximum: usize,
) -> Result<Vec<String>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(Vec::new());
    };
    let values = bounded_scalar_values(source, span, maximum)?;
    Ok(values.iter().map(token_string).collect())
}

pub(super) fn child_bool(source: &str, children: &[FormSpan], head: &str) -> Result<bool, Error> {
    let Some(span) = child(children, head) else {
        return Ok(false);
    };
    let values = scalar_values(source, span)?;
    Ok(values
        .first()
        .is_none_or(|value| matches!(token_string(value).as_str(), "yes" | "true" | "1")))
}

pub(super) fn has_flag(values: &[Token<'_>], expected: &str) -> bool {
    values
        .iter()
        .any(|value| value.kind == TokenKind::Atom && value.lexeme == expected)
}

pub(super) fn child_net_ref(source: &str, children: &[FormSpan]) -> Result<PcbNetRef, Error> {
    let Some(span) = child(children, "net") else {
        return Ok(PcbNetRef::default());
    };
    let values = scalar_values(source, span)?;
    let Some(token) = values.first() else {
        return Ok(PcbNetRef::default());
    };
    if let Ok(ordinal) = token.lexeme.parse() {
        Ok(PcbNetRef {
            ordinal: Some(ordinal),
            name: values
                .get(1)
                .map(token_string)
                .filter(|name| !name.is_empty()),
        })
    } else {
        Ok(PcbNetRef {
            ordinal: None,
            name: Some(token_string(token)),
        })
    }
}

pub(super) fn child_net_ref_or_zero(
    source: &str,
    children: &[FormSpan],
) -> Result<PcbNetRef, Error> {
    if child(children, "net").is_none() {
        Ok(PcbNetRef {
            ordinal: Some(0),
            name: None,
        })
    } else {
        child_net_ref(source, children)
    }
}

pub(super) fn optional_uuid(source: &str, children: &[FormSpan]) -> Result<Option<String>, Error> {
    if let Some(span) = child(children, "uuid").or_else(|| child(children, "tstamp")) {
        first_string(source, span)
    } else {
        Ok(None)
    }
}

pub(super) fn optional_uuid_or_id(
    source: &str,
    children: &[FormSpan],
) -> Result<Option<String>, Error> {
    if let Some(span) = child(children, "uuid").or_else(|| child(children, "id")) {
        first_string(source, span)
    } else {
        Ok(None)
    }
}

pub(super) fn required_string(
    token: Option<&Token<'_>>,
    message: &'static str,
    span: &FormSpan,
) -> Result<String, Error> {
    token
        .map(token_string)
        .ok_or_else(|| source_error(message, span.end))
}

pub(super) fn token_string(token: &Token<'_>) -> String {
    if token.kind == TokenKind::QuotedString {
        decode_quoted(token.lexeme)
    } else {
        token.lexeme.to_owned()
    }
}

pub(super) fn required_i64(
    token: Option<&Token<'_>>,
    message: &'static str,
    span: &FormSpan,
) -> Result<i64, Error> {
    token
        .map(|value| parse_i64(value, span))
        .unwrap_or_else(|| Err(source_error(message, span.end)))
}

pub(super) fn parse_i64(token: &Token<'_>, span: &FormSpan) -> Result<i64, Error> {
    token.lexeme.parse().map_err(|_| {
        source_error(
            "Expected integer value",
            rebase_position(token.position, span),
        )
    })
}

pub(super) fn required_f64(
    token: Option<&Token<'_>>,
    message: &'static str,
    span: &FormSpan,
) -> Result<f64, Error> {
    token
        .map(|value| parse_f64(value, span))
        .unwrap_or_else(|| Err(source_error(message, span.end)))
}

pub(super) fn optional_f64(
    token: Option<&Token<'_>>,
    span: &FormSpan,
) -> Result<Option<f64>, Error> {
    token.map(|value| parse_f64(value, span)).transpose()
}

pub(super) fn parse_f64(token: &Token<'_>, span: &FormSpan) -> Result<f64, Error> {
    token.lexeme.parse().map_err(|_| {
        source_error(
            "Expected numeric value",
            rebase_position(token.position, span),
        )
    })
}

pub(super) fn expect_kind(
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

pub(super) fn next_scalar<'a>(
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

pub(super) fn bounded_push<T>(values: &mut Vec<T>, value: T, maximum: usize) -> Result<(), Error> {
    if values.len() == maximum {
        return Err(limit_error());
    }
    values.push(value);
    Ok(())
}

pub(super) fn projection_limits(limits: PcbLimits, max_selected_forms: usize) -> ProjectionLimits {
    ProjectionLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_selected_forms,
        ..ProjectionLimits::default()
    }
}

pub(super) fn is_known_metadata(head: &str) -> bool {
    matches!(
        head,
        "version"
            | "generator"
            | "generator_version"
            | "general"
            | "paper"
            | "title_block"
            | "setup"
            | "variants"
            | "embedded_fonts"
    )
}

pub(super) fn graphic_kind(head: &str) -> Option<PcbGraphicKind> {
    match head {
        "gr_text" | "fp_text" => Some(PcbGraphicKind::Text),
        "gr_line" | "fp_line" => Some(PcbGraphicKind::Line),
        "gr_rect" | "fp_rect" => Some(PcbGraphicKind::Rect),
        "gr_arc" | "fp_arc" => Some(PcbGraphicKind::Arc),
        "gr_circle" | "fp_circle" => Some(PcbGraphicKind::Circle),
        "gr_poly" | "fp_poly" => Some(PcbGraphicKind::Poly),
        "gr_curve" | "fp_curve" => Some(PcbGraphicKind::Curve),
        "gr_text_box" | "fp_text_box" => Some(PcbGraphicKind::TextBox),
        _ => None,
    }
}

pub(super) fn is_known_top_level(head: &str) -> bool {
    is_known_metadata(head)
        || matches!(
            head,
            "layers"
                | "net"
                | "property"
                | "footprint"
                | "module"
                | "zone"
                | "dimension"
                | "segment"
                | "via"
                | "arc"
                | "group"
                | "generated"
                | "embedded_files"
                | "gr_text"
                | "gr_line"
                | "gr_rect"
                | "gr_arc"
                | "gr_circle"
                | "gr_poly"
                | "gr_curve"
                | "gr_text_box"
                | "image"
                | "barcode"
                | "table"
        )
}

pub(super) fn rebase_error(mut error: Error, span: &FormSpan) -> Error {
    if let Some(position) = error.position {
        error.position = Some(rebase_position(position, span));
    }
    error
}

pub(super) fn rebase_position(position: Position, span: &FormSpan) -> Position {
    Position {
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
    }
}

pub(super) fn source_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

pub(super) fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "PCB typed read exceeds configured limits",
        Position::START,
    )
}

pub(super) fn output_limit_error() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "PCB output exceeds max_output_bytes",
    )
}

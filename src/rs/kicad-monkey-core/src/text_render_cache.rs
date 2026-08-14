//! Typed native KiCad text render-cache generation and S-expression I/O.

use crate::{
    Lexer, TextContour, TextContourError, TextContourErrorKind, TextContourLimits,
    TextLayoutRequest, TextPoint, TextTopologyLimits, Token, TokenKind, fracture_text_contours_a0,
    layout_single_line_text_a0, layout_single_line_text_hinted_a0,
};
use std::fmt::{self, Write};

/// Independent limits for cache construction and close-to-format I/O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRenderCacheLimits {
    pub contours: TextContourLimits,
    pub topology: TextTopologyLimits,
    pub max_source_bytes: usize,
    pub max_text_bytes: usize,
    pub max_polygons: usize,
    pub max_contours: usize,
    pub max_points: usize,
    pub max_output_bytes: usize,
}

impl Default for TextRenderCacheLimits {
    fn default() -> Self {
        Self {
            contours: TextContourLimits::default(),
            topology: TextTopologyLimits::default(),
            max_source_bytes: 64 * 1024 * 1024,
            max_text_bytes: 16 * 1024 * 1024,
            max_polygons: 1024 * 1024,
            max_contours: 1024 * 1024,
            max_points: 32 * 1024 * 1024,
            max_output_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRenderCachePolygon {
    pub contours: Vec<TextContour>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRenderCache {
    pub text: String,
    pub angle_degrees: f64,
    pub polygons: Vec<TextRenderCachePolygon>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextRenderCacheErrorKind {
    ResourceLimit,
    InvalidInput,
    Syntax,
    TextPipeline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextRenderCacheError {
    pub kind: TextRenderCacheErrorKind,
    pub path: String,
    pub message: &'static str,
}

impl fmt::Display for TextRenderCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.message, self.path)
    }
}

impl std::error::Error for TextRenderCacheError {}

/// Compose horizontal layout and cache topology into one typed cache document.
pub fn generate_text_render_cache_a0(
    font_bytes: &[u8],
    request: TextLayoutRequest<'_>,
    limits: TextRenderCacheLimits,
) -> Result<TextRenderCache, TextRenderCacheError> {
    generate_text_render_cache_with_outline_mode(font_bytes, request, limits, false)
}

/// Compose a KiCad-compatible embedded-hinted layout and cache topology.
pub fn generate_text_render_cache_hinted_a0(
    font_bytes: &[u8],
    request: TextLayoutRequest<'_>,
    limits: TextRenderCacheLimits,
) -> Result<TextRenderCache, TextRenderCacheError> {
    generate_text_render_cache_with_outline_mode(font_bytes, request, limits, true)
}

fn generate_text_render_cache_with_outline_mode(
    font_bytes: &[u8],
    request: TextLayoutRequest<'_>,
    limits: TextRenderCacheLimits,
    hinted: bool,
) -> Result<TextRenderCache, TextRenderCacheError> {
    if request.shaping.text.len() > limits.max_text_bytes {
        return Err(limit("$.text", "render-cache text limit exceeded"));
    }
    let layout = if hinted {
        layout_single_line_text_hinted_a0(font_bytes, request, limits.contours)
    } else {
        layout_single_line_text_a0(font_bytes, request, limits.contours)
    }
    .map_err(map_contour_error)?;
    let mut topology_limits = limits.topology;
    topology_limits.max_output_polygons = topology_limits
        .max_output_polygons
        .min(limits.max_polygons)
        .min(limits.max_contours);
    topology_limits.max_output_points = topology_limits.max_output_points.min(limits.max_points);
    let topology =
        fracture_text_contours_a0(&layout.contours, topology_limits).map_err(map_contour_error)?;
    let polygons = topology
        .contours
        .into_iter()
        .map(|contour| TextRenderCachePolygon {
            contours: vec![contour],
        })
        .collect();
    Ok(TextRenderCache {
        text: request.shaping.text.clone(),
        angle_degrees: request.angle_degrees,
        polygons,
    })
}

/// Parse exactly one `(render_cache ...)` form without building a generic tree.
pub fn read_text_render_cache_a0(
    source: &[u8],
    limits: TextRenderCacheLimits,
) -> Result<TextRenderCache, TextRenderCacheError> {
    if source.len() > limits.max_source_bytes {
        return Err(limit("$", "render-cache source limit exceeded"));
    }
    let text = std::str::from_utf8(source)
        .map_err(|_| syntax("$", "render-cache source must be UTF-8"))?;
    CacheParser::new(text, limits).parse()
}

/// Emit one typed cache using KiCad-compatible nested S-expression layout.
pub fn write_text_render_cache_a0(
    cache: &TextRenderCache,
    limits: TextRenderCacheLimits,
) -> Result<Vec<u8>, TextRenderCacheError> {
    validate_cache(cache, limits)?;
    let mut output = BoundedText::new(limits.max_output_bytes);
    output.push_str("(render_cache \"")?;
    escape_quoted(&cache.text, &mut output)?;
    output.push_str("\" ")?;
    write_number(cache.angle_degrees, &mut output)?;
    for polygon in &cache.polygons {
        output.push_str("\n\t(polygon")?;
        for contour in &polygon.contours {
            output.push_str("\n\t\t(pts")?;
            for point in &contour.points {
                output.push_str("\n\t\t\t(xy ")?;
                write_number(point.x, &mut output)?;
                output.push(' ')?;
                write_number(point.y, &mut output)?;
                output.push(')')?;
            }
            output.push_str("\n\t\t)")?;
        }
        output.push_str("\n\t)")?;
    }
    output.push_str("\n)")?;
    Ok(output.text.into_bytes())
}

fn validate_cache(
    cache: &TextRenderCache,
    limits: TextRenderCacheLimits,
) -> Result<(), TextRenderCacheError> {
    if cache.text.len() > limits.max_text_bytes {
        return Err(limit("$.text", "render-cache text limit exceeded"));
    }
    if !cache.angle_degrees.is_finite() {
        return Err(invalid("$.angle", "render-cache angle must be finite"));
    }
    if cache.polygons.len() > limits.max_polygons {
        return Err(limit("$.polygons", "render-cache polygon limit exceeded"));
    }
    let mut contours = 0usize;
    let mut points = 0usize;
    for (polygon_index, polygon) in cache.polygons.iter().enumerate() {
        charge(
            &mut contours,
            polygon.contours.len(),
            limits.max_contours,
            "$.polygons[*].contours",
            "render-cache contour limit exceeded",
        )?;
        for (contour_index, contour) in polygon.contours.iter().enumerate() {
            charge(
                &mut points,
                contour.points.len(),
                limits.max_points,
                "$.polygons[*].contours[*].points",
                "render-cache point limit exceeded",
            )?;
            if contour
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
            {
                return Err(invalid_owned(
                    format!("$.polygons[{polygon_index}].contours[{contour_index}].points"),
                    "render-cache coordinates must be finite",
                ));
            }
        }
    }
    Ok(())
}

struct CacheParser<'a> {
    tokens: Lexer<'a>,
    limits: TextRenderCacheLimits,
    contour_count: usize,
    point_count: usize,
}

impl<'a> CacheParser<'a> {
    fn new(source: &'a str, limits: TextRenderCacheLimits) -> Self {
        Self {
            tokens: Lexer::new(source),
            limits,
            contour_count: 0,
            point_count: 0,
        }
    }

    fn parse(mut self) -> Result<TextRenderCache, TextRenderCacheError> {
        self.expect(TokenKind::Left, "expected render-cache opening parenthesis")?;
        self.expect_atom("render_cache")?;
        let text_token = self.next("expected render-cache text")?;
        let text = self.parse_text(text_token)?;
        let angle_token = self.next("expected render-cache angle")?;
        let angle_degrees = parse_number(angle_token, "$.angle")?;
        let mut polygons = Vec::new();
        loop {
            let token = self.next("expected polygon or render-cache closing parenthesis")?;
            match token.kind {
                TokenKind::Right => break,
                TokenKind::Left => {
                    self.expect_atom("polygon")?;
                    if polygons.len() >= self.limits.max_polygons {
                        return Err(limit("$.polygons", "render-cache polygon limit exceeded"));
                    }
                    polygons.push(self.parse_polygon()?);
                }
                _ => return Err(syntax("$.polygons", "expected render-cache polygon")),
            }
        }
        if self.tokens.next().is_some() {
            return Err(syntax("$", "unexpected trailing render-cache tokens"));
        }
        Ok(TextRenderCache {
            text,
            angle_degrees,
            polygons,
        })
    }

    fn parse_polygon(&mut self) -> Result<TextRenderCachePolygon, TextRenderCacheError> {
        let mut contours = Vec::new();
        loop {
            let token = self.next("expected points or polygon closing parenthesis")?;
            match token.kind {
                TokenKind::Right => break,
                TokenKind::Left => {
                    self.expect_atom("pts")?;
                    charge(
                        &mut self.contour_count,
                        1,
                        self.limits.max_contours,
                        "$.polygons[*].contours",
                        "render-cache contour limit exceeded",
                    )?;
                    contours.push(self.parse_points()?);
                }
                _ => return Err(syntax("$.polygons[*]", "expected points contour")),
            }
        }
        Ok(TextRenderCachePolygon { contours })
    }

    fn parse_points(&mut self) -> Result<TextContour, TextRenderCacheError> {
        let mut points = Vec::new();
        loop {
            let token = self.next("expected point or points closing parenthesis")?;
            match token.kind {
                TokenKind::Right => break,
                TokenKind::Left => {
                    self.expect_atom("xy")?;
                    charge(
                        &mut self.point_count,
                        1,
                        self.limits.max_points,
                        "$.polygons[*].contours[*].points",
                        "render-cache point limit exceeded",
                    )?;
                    let x = self.next("expected point x coordinate")?;
                    let y = self.next("expected point y coordinate")?;
                    let point = TextPoint {
                        x: parse_number(x, "$.point.x")?,
                        y: parse_number(y, "$.point.y")?,
                    };
                    self.expect(TokenKind::Right, "expected point closing parenthesis")?;
                    points.push(point);
                }
                _ => return Err(syntax("$.points", "expected xy point")),
            }
        }
        Ok(TextContour { points })
    }

    fn parse_text(&self, token: Token<'_>) -> Result<String, TextRenderCacheError> {
        match token.kind {
            TokenKind::QuotedString => {
                crate::sexpr::decode_quoted_with_limit(token.lexeme, self.limits.max_text_bytes)
                    .ok_or_else(|| limit("$.text", "render-cache text limit exceeded"))
            }
            TokenKind::Atom => {
                if token.lexeme.len() > self.limits.max_text_bytes {
                    Err(limit("$.text", "render-cache text limit exceeded"))
                } else {
                    Ok(token.lexeme.to_owned())
                }
            }
            _ => Err(syntax("$.text", "expected render-cache text scalar")),
        }
    }

    fn expect_atom(&mut self, expected: &'static str) -> Result<(), TextRenderCacheError> {
        let token = self.next("expected S-expression head")?;
        if token.kind == TokenKind::Atom && token.lexeme == expected {
            Ok(())
        } else {
            Err(syntax("$", "unexpected render-cache S-expression head"))
        }
    }

    fn expect(
        &mut self,
        expected: TokenKind,
        message: &'static str,
    ) -> Result<(), TextRenderCacheError> {
        if self.next(message)?.kind == expected {
            Ok(())
        } else {
            Err(syntax("$", message))
        }
    }

    fn next(&mut self, message: &'static str) -> Result<Token<'a>, TextRenderCacheError> {
        self.tokens
            .next()
            .ok_or_else(|| syntax("$", message))?
            .map_err(|_| syntax("$", "invalid render-cache token"))
    }
}

fn parse_number(token: Token<'_>, path: &'static str) -> Result<f64, TextRenderCacheError> {
    if !matches!(token.kind, TokenKind::Integer | TokenKind::Float) {
        return Err(syntax(path, "expected finite numeric scalar"));
    }
    let value = token
        .lexeme
        .parse::<f64>()
        .map_err(|_| syntax(path, "invalid numeric scalar"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid(path, "numeric scalar must be finite"))
    }
}

struct BoundedText {
    text: String,
    max_bytes: usize,
}

impl BoundedText {
    fn new(max_bytes: usize) -> Self {
        Self {
            text: String::new(),
            max_bytes,
        }
    }

    fn push(&mut self, character: char) -> Result<(), TextRenderCacheError> {
        self.reserve(character.len_utf8())?;
        self.text.push(character);
        Ok(())
    }

    fn push_str(&mut self, value: &str) -> Result<(), TextRenderCacheError> {
        self.reserve(value.len())?;
        self.text.push_str(value);
        Ok(())
    }

    fn reserve(&self, additional: usize) -> Result<(), TextRenderCacheError> {
        if self
            .text
            .len()
            .checked_add(additional)
            .is_none_or(|length| length > self.max_bytes)
        {
            Err(limit("$", "render-cache output byte limit exceeded"))
        } else {
            Ok(())
        }
    }
}

impl Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.push_str(value).map_err(|_| fmt::Error)
    }
}

fn write_number(value: f64, output: &mut BoundedText) -> Result<(), TextRenderCacheError> {
    if !value.is_finite() {
        return Err(invalid("$", "cannot write nonfinite render-cache scalar"));
    }
    if value == 0.0 {
        output.push('0')
    } else {
        write!(output, "{value}").map_err(|_| limit("$", "render-cache output byte limit exceeded"))
    }
}

fn escape_quoted(value: &str, output: &mut BoundedText) -> Result<(), TextRenderCacheError> {
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

fn charge(
    value: &mut usize,
    amount: usize,
    maximum: usize,
    path: &'static str,
    message: &'static str,
) -> Result<(), TextRenderCacheError> {
    let next = value
        .checked_add(amount)
        .ok_or_else(|| limit(path, message))?;
    if next > maximum {
        return Err(limit(path, message));
    }
    *value = next;
    Ok(())
}

fn map_contour_error(error: TextContourError) -> TextRenderCacheError {
    TextRenderCacheError {
        kind: match error.kind {
            TextContourErrorKind::ResourceLimit => TextRenderCacheErrorKind::ResourceLimit,
            TextContourErrorKind::InvalidInput => TextRenderCacheErrorKind::InvalidInput,
            TextContourErrorKind::Shaping | TextContourErrorKind::Outline => {
                TextRenderCacheErrorKind::TextPipeline
            }
        },
        path: error.path,
        message: error.message,
    }
}

fn limit(path: &'static str, message: &'static str) -> TextRenderCacheError {
    TextRenderCacheError {
        kind: TextRenderCacheErrorKind::ResourceLimit,
        path: path.to_owned(),
        message,
    }
}

fn invalid(path: &'static str, message: &'static str) -> TextRenderCacheError {
    invalid_owned(path.to_owned(), message)
}

fn invalid_owned(path: String, message: &'static str) -> TextRenderCacheError {
    TextRenderCacheError {
        kind: TextRenderCacheErrorKind::InvalidInput,
        path,
        message,
    }
}

fn syntax(path: &'static str, message: &'static str) -> TextRenderCacheError {
    TextRenderCacheError {
        kind: TextRenderCacheErrorKind::Syntax,
        path: path.to_owned(),
        message,
    }
}

//! Public, bounded KiCad Newstroke polyline realization.

use crate::board_plotter_ir::stroke_font_widths::{
    NEWSTROKE_GLYPH_DATA, NEWSTROKE_GLYPH_OFFSETS, NEWSTROKE_WIDTH_UNITS,
};
use crate::text_markup::{TextMarkupMarker, TextMarkupNode, parse_text_markup};
use crate::{TextContourErrorKind, TextHorizontalAlignment, TextVerticalAlignment};
use std::fmt;

const STROKE_SCALE: f64 = 1.0 / 21.0;
const FONT_OFFSET: f64 = -8.0;
const ITALIC_TILT: f64 = 1.0 / 8.0;
const SUPER_SUB_SIZE_MULTIPLIER: f64 = 0.8;
const SUPER_HEIGHT_OFFSET: f64 = 0.35;
const SUB_HEIGHT_OFFSET: f64 = 0.15;
const OVERBAR_POSITION_FACTOR: f64 = 1.23;
const OVERBAR_TRIM_RATIO: f64 = 0.1;

/// One Newstroke realization request in KiCad source millimetres.
///
/// Positive source angles use KiCad's y-down placement convention. Newlines
/// are not laid out as a text block by this single-line API; like the existing
/// KiCad Monkey realizer, an embedded newline follows missing-glyph fallback.
#[derive(Clone, Copy, Debug)]
pub struct NewstrokeRequest<'a> {
    pub text: &'a str,
    pub position_x_mm: f64,
    pub position_y_mm: f64,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    pub angle_degrees: f64,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub mirrored: bool,
    pub italic: bool,
    pub bold: bool,
    /// Authored thickness in millimetres. `None` applies KiCad Monkey's
    /// existing normal/bold automatic Newstroke thickness rule.
    pub stroke_width_mm: Option<f64>,
}

/// Independent ceilings for one Newstroke realization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NewstrokeLimits {
    pub max_text_bytes: usize,
    pub max_markup_nodes: usize,
    pub max_polylines: usize,
    pub max_points: usize,
}

impl Default for NewstrokeLimits {
    fn default() -> Self {
        Self {
            max_text_bytes: 64 * 1024 * 1024,
            max_markup_nodes: 1_000_000,
            max_polylines: 16_000_000,
            max_points: 16_000_000,
        }
    }
}

/// One placed Newstroke point in KiCad source millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewstrokePoint {
    pub x_mm: f64,
    pub y_mm: f64,
}

/// One connected pen-down path. Pen-up markers always split polylines.
#[derive(Clone, Debug, PartialEq)]
pub struct NewstrokePolyline {
    pub points: Vec<NewstrokePoint>,
}

/// Realized centerlines and source-font metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct NewstrokeOutput {
    pub polylines: Vec<NewstrokePolyline>,
    /// Centerline bounds `[min_x, min_y, max_x, max_y]` in millimetres.
    /// Stroke-radius expansion is intentionally left to the consumer.
    pub bounds_mm: Option<[f64; 4]>,
    /// Actual cursor advance, including `?` fallback for unsupported glyphs.
    pub advance_width_mm: f64,
    /// Width used by the existing KiCad Monkey/Python alignment behavior.
    /// Unsupported glyphs contribute no width here even though realization
    /// draws and advances by `?`.
    pub alignment_width_mm: f64,
    pub effective_stroke_width_mm: f64,
    pub missing_glyphs: usize,
    pub markup_nodes: usize,
    pub point_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NewstrokeErrorKind {
    ResourceLimit,
    InvalidInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewstrokeError {
    pub kind: NewstrokeErrorKind,
    pub message: &'static str,
}

impl fmt::Display for NewstrokeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for NewstrokeError {}

/// Realize KiCad Newstroke text as ordered, disconnected polylines.
///
/// The returned coordinates use the same y-down millimetre space as PCB and
/// footprint source values. Alignment is applied before mirroring, the source
/// rotation transform, and translation. A source angle of `+90` maps local
/// `+X` toward global `-Y`. Markup supports KiCad Monkey's existing
/// `~{overbar}`, `_{subscript}`, and `^{superscript}` rules. Unsupported
/// codepoints are drawn with `?` and counted in `missing_glyphs`.
pub fn realize_newstroke_a0(
    request: NewstrokeRequest<'_>,
    limits: NewstrokeLimits,
) -> Result<NewstrokeOutput, NewstrokeError> {
    validate_request(request, limits)?;
    if request.text.is_empty() {
        return Ok(NewstrokeOutput {
            polylines: Vec::new(),
            bounds_mm: None,
            advance_width_mm: 0.0,
            alignment_width_mm: 0.0,
            effective_stroke_width_mm: effective_stroke_width(request),
            missing_glyphs: 0,
            markup_nodes: 0,
            point_count: 0,
        });
    }
    let (nodes, markup_nodes) = parsed_markup(request.text, limits.max_markup_nodes)?;
    let alignment_width_mm = markup_width(request.text, &nodes) * request.size_x_mm;
    if !alignment_width_mm.is_finite() {
        return Err(invalid_geometry_error());
    }
    let cursor = match request.horizontal_alignment {
        TextHorizontalAlignment::Left => 0.0,
        TextHorizontalAlignment::Center => -alignment_width_mm / 2.0,
        TextHorizontalAlignment::Right => -alignment_width_mm,
    };
    let initial_cursor = cursor;
    let mut state = RealizationState {
        request,
        limits,
        polylines: Vec::new(),
        point_count: 0,
        missing_glyphs: 0,
        cursor,
        offset_y: vertical_offset(request),
    };
    state.walk(request.text, &nodes)?;
    let advance_width_mm = state.cursor - initial_cursor;
    if !advance_width_mm.is_finite() {
        return Err(invalid_geometry_error());
    }
    let bounds_mm = bounds(&state.polylines);
    Ok(NewstrokeOutput {
        polylines: state.polylines,
        bounds_mm,
        advance_width_mm,
        alignment_width_mm,
        effective_stroke_width_mm: effective_stroke_width(request),
        missing_glyphs: state.missing_glyphs,
        markup_nodes,
        point_count: state.point_count,
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Style {
    Normal,
    Subscript,
    Superscript,
}

struct Frame<'a> {
    nodes: &'a [TextMarkupNode],
    index: usize,
    marker: Option<TextMarkupMarker>,
    bar_start: f64,
    style: Style,
}

struct RealizationState<'a> {
    request: NewstrokeRequest<'a>,
    limits: NewstrokeLimits,
    polylines: Vec<NewstrokePolyline>,
    point_count: usize,
    missing_glyphs: usize,
    cursor: f64,
    offset_y: f64,
}

impl RealizationState<'_> {
    fn walk(&mut self, text: &str, nodes: &[TextMarkupNode]) -> Result<(), NewstrokeError> {
        let mut frames = vec![Frame {
            nodes,
            index: 0,
            marker: None,
            bar_start: self.cursor,
            style: Style::Normal,
        }];
        while let Some(frame) = frames.last_mut() {
            let Some(node) = frame.nodes.get(frame.index) else {
                let closed = frames.pop().expect("frame presence was checked");
                if closed.marker == Some(TextMarkupMarker::Overbar) {
                    self.push_polyline([
                        self.transform(
                            closed.bar_start + self.request.size_x_mm * OVERBAR_TRIM_RATIO,
                            self.offset_y - self.request.size_y_mm * OVERBAR_POSITION_FACTOR,
                            false,
                        )?,
                        self.transform(
                            self.cursor - self.request.size_x_mm * OVERBAR_TRIM_RATIO,
                            self.offset_y - self.request.size_y_mm * OVERBAR_POSITION_FACTOR,
                            false,
                        )?,
                    ])?;
                }
                continue;
            };
            frame.index += 1;
            match node {
                TextMarkupNode::Text(span) => self.emit_chars(&text[span.clone()], frame.style)?,
                TextMarkupNode::Group { marker, children } => {
                    let style = child_style(frame.style, *marker);
                    frames.push(Frame {
                        nodes: children,
                        index: 0,
                        marker: Some(*marker),
                        bar_start: self.cursor,
                        style,
                    });
                }
            }
        }
        Ok(())
    }

    fn emit_chars(&mut self, characters: &str, style: Style) -> Result<(), NewstrokeError> {
        let scale = if style == Style::Normal {
            1.0
        } else {
            SUPER_SUB_SIZE_MULTIPLIER
        };
        let size_x = self.request.size_x_mm * scale;
        let size_y = self.request.size_y_mm * scale;
        let style_y = match style {
            Style::Normal => 0.0,
            Style::Subscript => size_y * SUB_HEIGHT_OFFSET,
            Style::Superscript => -size_y * SUPER_HEIGHT_OFFSET,
        };
        for character in characters.chars() {
            let Some((glyph, width)) = glyph(character).or_else(|| {
                self.missing_glyphs = self.missing_glyphs.saturating_add(1);
                glyph('?')
            }) else {
                continue;
            };
            if character == ' ' {
                self.cursor += width * size_x;
                continue;
            }
            let start_x = glyph.first().map_or(0.0, |value| {
                (f64::from(*value) - f64::from(b'R')) * STROKE_SCALE
            });
            let mut current = Vec::new();
            let mut index = 2usize;
            while index + 1 < glyph.len() {
                if glyph[index] == b' ' && glyph[index + 1] == b'R' {
                    self.finish_glyph_stroke(&mut current)?;
                    index += 2;
                    continue;
                }
                let x = (f64::from(glyph[index]) - f64::from(b'R')) * STROKE_SCALE - start_x;
                let y =
                    (f64::from(glyph[index + 1]) - f64::from(b'R') + FONT_OFFSET) * STROKE_SCALE;
                current.push(self.transform(
                    x * size_x + self.cursor,
                    y * size_y + self.offset_y + style_y,
                    self.request.italic,
                )?);
                index += 2;
            }
            self.finish_glyph_stroke(&mut current)?;
            self.cursor += width * size_x;
        }
        Ok(())
    }

    fn finish_glyph_stroke(
        &mut self,
        points: &mut Vec<NewstrokePoint>,
    ) -> Result<(), NewstrokeError> {
        if points.len() >= 2 {
            self.push_polyline(std::mem::take(points))?;
        } else {
            points.clear();
        }
        Ok(())
    }

    fn push_polyline(
        &mut self,
        points: impl IntoIterator<Item = NewstrokePoint>,
    ) -> Result<(), NewstrokeError> {
        if self.polylines.len() >= self.limits.max_polylines {
            return Err(limit_error("Newstroke polylines exceed max_polylines"));
        }
        let points = points.into_iter().collect::<Vec<_>>();
        self.point_count = self
            .point_count
            .checked_add(points.len())
            .filter(|count| *count <= self.limits.max_points)
            .ok_or_else(|| limit_error("Newstroke points exceed max_points"))?;
        self.polylines.push(NewstrokePolyline { points });
        Ok(())
    }

    fn transform(
        &self,
        mut x: f64,
        y: f64,
        italic: bool,
    ) -> Result<NewstrokePoint, NewstrokeError> {
        if italic {
            x += y * ITALIC_TILT;
        }
        if self.request.mirrored {
            x = -x;
        }
        let radians = (-self.request.angle_degrees).to_radians();
        let (sine, cosine) = radians.sin_cos();
        let point = NewstrokePoint {
            x_mm: x * cosine - y * sine + self.request.position_x_mm,
            y_mm: x * sine + y * cosine + self.request.position_y_mm,
        };
        if !point.x_mm.is_finite() || !point.y_mm.is_finite() {
            return Err(invalid_geometry_error());
        }
        Ok(point)
    }
}

pub(crate) fn newstroke_alignment_width_mm(
    text: &str,
    size_x_mm: f64,
    max_text_bytes: usize,
    max_markup_nodes: usize,
) -> Result<f64, NewstrokeError> {
    if text.len() > max_text_bytes {
        return Err(limit_error("Newstroke text exceeds max_text_bytes"));
    }
    if !size_x_mm.is_finite() {
        return Err(invalid_geometry_error());
    }
    let (nodes, _) = parsed_markup(text, max_markup_nodes)?;
    let width = markup_width(text, &nodes) * size_x_mm;
    if !width.is_finite() {
        return Err(invalid_geometry_error());
    }
    Ok(width)
}

fn validate_request(
    request: NewstrokeRequest<'_>,
    limits: NewstrokeLimits,
) -> Result<(), NewstrokeError> {
    if request.text.len() > limits.max_text_bytes {
        return Err(limit_error("Newstroke text exceeds max_text_bytes"));
    }
    if ![
        request.position_x_mm,
        request.position_y_mm,
        request.size_x_mm,
        request.size_y_mm,
        request.angle_degrees,
    ]
    .into_iter()
    .all(f64::is_finite)
        || request
            .stroke_width_mm
            .is_some_and(|width| !width.is_finite() || width < 0.0)
    {
        return Err(NewstrokeError {
            kind: NewstrokeErrorKind::InvalidInput,
            message: "Newstroke request contains invalid geometry or stroke width",
        });
    }
    Ok(())
}

fn parsed_markup(
    text: &str,
    maximum: usize,
) -> Result<(Vec<TextMarkupNode>, usize), NewstrokeError> {
    let mut nodes = 0usize;
    let parsed = parse_text_markup(text, &mut nodes, maximum).map_err(|error| NewstrokeError {
        kind: if error.kind == TextContourErrorKind::ResourceLimit {
            NewstrokeErrorKind::ResourceLimit
        } else {
            NewstrokeErrorKind::InvalidInput
        },
        message: error.message,
    })?;
    Ok((parsed, nodes))
}

fn vertical_offset(request: NewstrokeRequest<'_>) -> f64 {
    let cap_top = -20.0 / 21.0;
    let cap_bottom = 1.0 / 21.0;
    let cap_center = (cap_top + cap_bottom) / 2.0;
    match request.vertical_alignment {
        TextVerticalAlignment::Center => (-cap_center - cap_bottom + 0.0024) * request.size_y_mm,
        TextVerticalAlignment::Top => (-cap_top + 0.0024) * request.size_y_mm,
        TextVerticalAlignment::Bottom => (-cap_bottom + 0.0024) * request.size_y_mm,
    }
}

fn effective_stroke_width(request: NewstrokeRequest<'_>) -> f64 {
    if let Some(width) = request.stroke_width_mm {
        return width;
    }
    let text_width = if request.size_x_mm.abs() != 0.0 {
        request.size_x_mm.abs()
    } else {
        request.size_y_mm.abs()
    };
    if text_width == 0.0 {
        return 0.15;
    }
    let mut width = if request.bold {
        text_width / 5.0
    } else {
        text_width / 8.0
    };
    let minimum_size = request.size_x_mm.abs().min(request.size_y_mm.abs());
    if minimum_size != 0.0 {
        width = width.min(minimum_size * 0.25);
    }
    width
}

fn markup_width(text: &str, nodes: &[TextMarkupNode]) -> f64 {
    let mut width = 0.0;
    let mut frames = vec![Frame {
        nodes,
        index: 0,
        marker: None,
        bar_start: 0.0,
        style: Style::Normal,
    }];
    while let Some(frame) = frames.last_mut() {
        let Some(node) = frame.nodes.get(frame.index) else {
            frames.pop();
            continue;
        };
        frame.index += 1;
        match node {
            TextMarkupNode::Text(span) => {
                let scale = if frame.style == Style::Normal {
                    1.0
                } else {
                    SUPER_SUB_SIZE_MULTIPLIER
                };
                width += text[span.clone()]
                    .chars()
                    .filter_map(glyph)
                    .map(|(_, value)| value * scale)
                    .sum::<f64>();
            }
            TextMarkupNode::Group { marker, children } => {
                let style = child_style(frame.style, *marker);
                frames.push(Frame {
                    nodes: children,
                    index: 0,
                    marker: Some(*marker),
                    bar_start: width,
                    style,
                });
            }
        }
    }
    width
}

fn child_style(style: Style, marker: TextMarkupMarker) -> Style {
    match marker {
        TextMarkupMarker::Overbar => style,
        TextMarkupMarker::Subscript => Style::Subscript,
        TextMarkupMarker::Superscript if style == Style::Subscript => Style::Subscript,
        TextMarkupMarker::Superscript => Style::Superscript,
    }
}

fn glyph(character: char) -> Option<(&'static [u8], f64)> {
    let index = (character as usize).checked_sub(0x20)?;
    let start = usize::try_from(*NEWSTROKE_GLYPH_OFFSETS.get(index)?).ok()?;
    let end = usize::try_from(*NEWSTROKE_GLYPH_OFFSETS.get(index + 1)?).ok()?;
    Some((
        NEWSTROKE_GLYPH_DATA.as_bytes().get(start..end)?,
        f64::from(*NEWSTROKE_WIDTH_UNITS.get(index)?) * STROKE_SCALE,
    ))
}

fn bounds(polylines: &[NewstrokePolyline]) -> Option<[f64; 4]> {
    let mut result: Option<[f64; 4]> = None;
    for point in polylines.iter().flat_map(|polyline| &polyline.points) {
        match &mut result {
            Some(value) => {
                value[0] = value[0].min(point.x_mm);
                value[1] = value[1].min(point.y_mm);
                value[2] = value[2].max(point.x_mm);
                value[3] = value[3].max(point.y_mm);
            }
            None => result = Some([point.x_mm, point.y_mm, point.x_mm, point.y_mm]),
        }
    }
    result
}

fn limit_error(message: &'static str) -> NewstrokeError {
    NewstrokeError {
        kind: NewstrokeErrorKind::ResourceLimit,
        message,
    }
}

fn invalid_geometry_error() -> NewstrokeError {
    NewstrokeError {
        kind: NewstrokeErrorKind::InvalidInput,
        message: "Newstroke realization produced nonfinite geometry",
    }
}

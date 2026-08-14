//! Bounded composition of native shaping and outlines into positioned contours.

use crate::{
    FontOutlineError, FontOutlineErrorKind, FontOutlineFace, FontOutlineFaceRequest,
    FontOutlineLimits, TextBezierError, TextBezierErrorKind, TextBezierLimits, TextPoint,
    TextShapingError, TextShapingErrorKind, TextShapingLimits, flatten_cubic_bezier,
    flatten_quadratic_bezier, shape_text_a0,
};
use kicad_monkey_contracts::FiniteFloat;
use kicad_monkey_contracts::generated::outline_vector::{
    FontVariationCoordinate as OutlineVariation, OpenTypeTag as OutlineTag, OutlineCommand,
};
use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use std::fmt;

/// KiCad's truncated `16 * 64 * 1.4` outline face size.
pub const KICAD_OUTLINE_FACE_SCALER: f64 = 1433.0;
/// KiCad's outline-font visual size compensation.
pub const KICAD_OUTLINE_SIZE_COMPENSATION: f64 = 1.4;
/// KiCad's default curve error in outline-face coordinate units.
pub const KICAD_TEXT_BEZIER_ERROR: f64 = 2.0;

/// One raw text run before alignment, markup, mirroring, or rotation.
#[derive(Clone, Copy, Debug)]
pub struct TextContourRequest<'a> {
    pub shaping: &'a ShapingInput,
    pub size_x: f64,
    pub size_y: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub max_error: f64,
}

/// Independent limits for one shaping-to-contour operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextContourLimits {
    pub shaping: TextShapingLimits,
    pub outline: FontOutlineLimits,
    pub max_outline_commands: usize,
    pub max_bezier_work_items: usize,
    pub max_temporary_bezier_points: usize,
    pub max_contours: usize,
    pub max_points: usize,
}

impl Default for TextContourLimits {
    fn default() -> Self {
        Self {
            shaping: TextShapingLimits::default(),
            outline: FontOutlineLimits::default(),
            max_outline_commands: 16 * 1024 * 1024,
            max_bezier_work_items: 16 * 1024 * 1024,
            max_temporary_bezier_points: 16 * 1024 * 1024,
            max_contours: 16 * 1024 * 1024,
            max_points: 16 * 1024 * 1024,
        }
    }
}

/// Ordered contour points preserving the outline callback's closing edge.
#[derive(Clone, Debug, PartialEq)]
pub struct TextContour {
    pub points: Vec<TextPoint>,
}

/// Positioned contours plus the final run advance in caller coordinates.
#[derive(Clone, Debug)]
pub struct TextContourOutput {
    pub contours: Vec<TextContour>,
    pub advance_x: f64,
    pub advance_y: f64,
    pub units_per_em: u16,
    pub outline_commands: usize,
    pub bezier_work_items: usize,
    pub peak_temporary_bezier_points: usize,
}

/// Stable failure categories for native contour composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextContourErrorKind {
    ResourceLimit,
    InvalidInput,
    Shaping,
    Outline,
}

/// Fail-closed text contour error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextContourError {
    pub kind: TextContourErrorKind,
    pub path: String,
    pub message: &'static str,
}

impl fmt::Display for TextContourError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.message, self.path)
    }
}

impl std::error::Error for TextContourError {}

/// Shape and place one plain text run using one reusable outline face.
pub fn shape_text_contours_a0(
    font_bytes: &[u8],
    request: TextContourRequest<'_>,
    limits: TextContourLimits,
) -> Result<TextContourOutput, TextContourError> {
    validate_request(request)?;
    preflight_outline_metadata(font_bytes, request.shaping, limits.outline)?;
    let shaped =
        shape_text_a0(font_bytes, request.shaping, limits.shaping).map_err(map_shaping_error)?;
    let variations = request
        .shaping
        .variations
        .iter()
        .map(|variation| OutlineVariation {
            axis: OutlineTag(variation.axis.0.clone()),
            value: FiniteFloat::try_from(variation.value.get())
                .expect("shaping variation coordinates are finite"),
        })
        .collect::<Vec<_>>();
    let face = FontOutlineFace::new(
        font_bytes,
        FontOutlineFaceRequest {
            font_id: request.shaping.font_id.as_str(),
            font_sha256: &request.shaping.font_sha256.0,
            face_index: request.shaping.face_index,
            variations: &variations,
        },
        limits.outline,
    )
    .map_err(map_outline_error)?;
    if face.units_per_em() != shaped.units_per_em {
        return Err(contour_error(
            TextContourErrorKind::InvalidInput,
            "$.face_index",
            "shaping and outline backends disagree on units per em",
        ));
    }

    let units_per_em = f64::from(shaped.units_per_em);
    let outline_to_internal = KICAD_OUTLINE_FACE_SCALER / units_per_em;
    let position_x_to_internal = KICAD_OUTLINE_FACE_SCALER / f64::from(request.shaping.scale_x);
    let position_y_to_internal = KICAD_OUTLINE_FACE_SCALER / f64::from(request.shaping.scale_y);
    let internal_to_x =
        request.size_x / KICAD_OUTLINE_FACE_SCALER * KICAD_OUTLINE_SIZE_COMPENSATION;
    let internal_to_y =
        request.size_y / KICAD_OUTLINE_FACE_SCALER * KICAD_OUTLINE_SIZE_COMPENSATION;
    let transform = PlacementTransform {
        outline_to_internal,
        position_x_to_internal,
        position_y_to_internal,
        internal_to_x,
        internal_to_y,
        origin_x: request.origin_x,
        origin_y: request.origin_y,
    };

    let mut output = ContourBuilder::new(limits, request.max_error, transform);
    let mut cursor_x = 0.0;
    let mut cursor_y = 0.0;
    for glyph in &shaped.glyphs {
        let remaining_commands = limits
            .max_outline_commands
            .saturating_sub(output.outline_commands);
        match face.extract_glyph_with_limit(glyph.glyph_id, remaining_commands) {
            Ok(outline) => {
                output.charge_commands(outline.commands.len())?;
                output.append_outline(
                    &outline.commands,
                    cursor_x + glyph.x_offset.get() as f64,
                    cursor_y + glyph.y_offset.get() as f64,
                )?;
            }
            Err(error) if error.kind == FontOutlineErrorKind::MissingOutline => {}
            Err(error) => return Err(map_outline_error(error)),
        }
        cursor_x += glyph.x_advance.get() as f64;
        cursor_y += glyph.y_advance.get() as f64;
        if !cursor_x.is_finite() || !cursor_y.is_finite() {
            return Err(contour_error(
                TextContourErrorKind::InvalidInput,
                "$.glyphs",
                "shaped cursor accumulation is not finite",
            ));
        }
    }
    let advance_x = cursor_x * position_x_to_internal * internal_to_x;
    let advance_y = -cursor_y * position_y_to_internal * internal_to_y;
    if !advance_x.is_finite() || !advance_y.is_finite() {
        return Err(contour_error(
            TextContourErrorKind::InvalidInput,
            "$.advance",
            "text run advance is not finite",
        ));
    }
    Ok(output.finish(advance_x, advance_y, shaped.units_per_em))
}

#[derive(Clone, Copy)]
struct PlacementTransform {
    outline_to_internal: f64,
    position_x_to_internal: f64,
    position_y_to_internal: f64,
    internal_to_x: f64,
    internal_to_y: f64,
    origin_x: f64,
    origin_y: f64,
}

struct ContourBuilder {
    contours: Vec<TextContour>,
    current: Vec<TextPoint>,
    last_outline_point: TextPoint,
    limits: TextContourLimits,
    max_error: f64,
    transform: PlacementTransform,
    retained_points: usize,
    outline_commands: usize,
    bezier_work_items: usize,
    peak_temporary_bezier_points: usize,
}

impl ContourBuilder {
    fn new(limits: TextContourLimits, max_error: f64, transform: PlacementTransform) -> Self {
        Self {
            contours: Vec::with_capacity(limits.max_contours.min(256)),
            current: Vec::new(),
            last_outline_point: TextPoint { x: 0.0, y: 0.0 },
            limits,
            max_error,
            transform,
            retained_points: 0,
            outline_commands: 0,
            bezier_work_items: 0,
            peak_temporary_bezier_points: 0,
        }
    }

    fn charge_commands(&mut self, count: usize) -> Result<(), TextContourError> {
        self.outline_commands = self
            .outline_commands
            .checked_add(count)
            .filter(|total| *total <= self.limits.max_outline_commands)
            .ok_or_else(|| {
                resource_error("$.outline_commands", "outline command limit exceeded")
            })?;
        Ok(())
    }

    fn append_outline(
        &mut self,
        commands: &[OutlineCommand],
        position_x: f64,
        position_y: f64,
    ) -> Result<(), TextContourError> {
        for command in commands {
            match command {
                OutlineCommand::MoveTo(command) => {
                    self.flush()?;
                    self.last_outline_point = self.outline_point(command.x.get(), command.y.get());
                    self.push_transformed(self.last_outline_point, position_x, position_y)?;
                }
                OutlineCommand::LineTo(command) => {
                    self.last_outline_point = self.outline_point(command.x.get(), command.y.get());
                    self.push_transformed(self.last_outline_point, position_x, position_y)?;
                }
                OutlineCommand::QuadTo(command) => {
                    let end = self.outline_point(command.x.get(), command.y.get());
                    let control =
                        self.outline_point(command.control_x.get(), command.control_y.get());
                    let flattened = flatten_quadratic_bezier(
                        [self.last_outline_point, control, end],
                        self.max_error,
                        self.bezier_limits(),
                    )
                    .map_err(map_bezier_error)?;
                    self.bezier_work_items += flattened.work_items;
                    self.peak_temporary_bezier_points = self
                        .peak_temporary_bezier_points
                        .max(flattened.points.len());
                    for point in flattened.points {
                        self.push_transformed(point, position_x, position_y)?;
                    }
                    self.last_outline_point = end;
                }
                OutlineCommand::CurveTo(command) => {
                    let end = self.outline_point(command.x.get(), command.y.get());
                    let control1 =
                        self.outline_point(command.control1_x.get(), command.control1_y.get());
                    let control2 =
                        self.outline_point(command.control2_x.get(), command.control2_y.get());
                    let flattened = flatten_cubic_bezier(
                        [self.last_outline_point, control1, control2, end],
                        self.max_error,
                        self.bezier_limits(),
                    )
                    .map_err(map_bezier_error)?;
                    self.bezier_work_items += flattened.work_items;
                    self.peak_temporary_bezier_points = self
                        .peak_temporary_bezier_points
                        .max(flattened.points.len());
                    for point in flattened.points {
                        self.push_transformed(point, position_x, position_y)?;
                    }
                    self.last_outline_point = end;
                }
                OutlineCommand::Close(_) => self.flush()?,
            }
        }
        self.flush()
    }

    fn outline_point(&self, x: f64, y: f64) -> TextPoint {
        TextPoint {
            x: x * self.transform.outline_to_internal,
            y: y * self.transform.outline_to_internal,
        }
    }

    fn bezier_limits(&self) -> TextBezierLimits {
        TextBezierLimits {
            max_points: self.limits.max_temporary_bezier_points,
            max_work_items: self
                .limits
                .max_bezier_work_items
                .saturating_sub(self.bezier_work_items),
        }
    }

    fn push_transformed(
        &mut self,
        point: TextPoint,
        position_x: f64,
        position_y: f64,
    ) -> Result<(), TextContourError> {
        let transformed = TextPoint {
            x: self.transform.origin_x
                + (point.x + position_x * self.transform.position_x_to_internal)
                    * self.transform.internal_to_x,
            y: self.transform.origin_y
                - (point.y + position_y * self.transform.position_y_to_internal)
                    * self.transform.internal_to_y,
        };
        if !transformed.x.is_finite() || !transformed.y.is_finite() {
            return Err(contour_error(
                TextContourErrorKind::InvalidInput,
                "$.contours",
                "positioned contour coordinate is not finite",
            ));
        }
        if self.current.last() == Some(&transformed) {
            return Ok(());
        }
        if self.retained_points >= self.limits.max_points {
            return Err(resource_error(
                "$.contours.points",
                "retained contour point limit exceeded",
            ));
        }
        self.current.push(transformed);
        self.retained_points += 1;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), TextContourError> {
        if self.current.is_empty() {
            return Ok(());
        }
        if self.contours.len() >= self.limits.max_contours {
            return Err(resource_error(
                "$.contours",
                "retained contour count limit exceeded",
            ));
        }
        self.contours.push(TextContour {
            points: std::mem::take(&mut self.current),
        });
        Ok(())
    }

    fn finish(self, advance_x: f64, advance_y: f64, units_per_em: u16) -> TextContourOutput {
        TextContourOutput {
            contours: self.contours,
            advance_x,
            advance_y,
            units_per_em,
            outline_commands: self.outline_commands,
            bezier_work_items: self.bezier_work_items,
            peak_temporary_bezier_points: self.peak_temporary_bezier_points,
        }
    }
}

fn validate_request(request: TextContourRequest<'_>) -> Result<(), TextContourError> {
    if !request.size_x.is_finite()
        || !request.size_y.is_finite()
        || !request.origin_x.is_finite()
        || !request.origin_y.is_finite()
        || !request.max_error.is_finite()
        || request.size_x <= 0.0
        || request.size_y <= 0.0
        || request.shaping.scale_x <= 0
        || request.shaping.scale_y <= 0
    {
        return Err(contour_error(
            TextContourErrorKind::InvalidInput,
            "$",
            "text contour sizes and scales must be positive; origins and tolerance must be finite",
        ));
    }
    Ok(())
}

fn preflight_outline_metadata(
    font_bytes: &[u8],
    input: &ShapingInput,
    limits: FontOutlineLimits,
) -> Result<(), TextContourError> {
    if font_bytes.len() > limits.max_font_bytes || input.variations.len() > limits.max_variations {
        return Err(resource_error(
            "$",
            "outline input exceeds a configured count or byte limit",
        ));
    }
    let metadata_bytes = input
        .font_id
        .as_str()
        .len()
        .checked_add(input.font_sha256.0.len())
        .and_then(|total| {
            input
                .variations
                .iter()
                .try_fold(total, |total, value| total.checked_add(value.axis.0.len()))
        })
        .ok_or_else(|| resource_error("$", "outline metadata byte count overflowed"))?;
    if metadata_bytes > limits.max_metadata_bytes {
        return Err(resource_error(
            "$",
            "outline metadata exceeds the configured byte limit",
        ));
    }
    Ok(())
}

fn map_shaping_error(error: TextShapingError) -> TextContourError {
    TextContourError {
        kind: if error.kind == TextShapingErrorKind::ResourceLimit {
            TextContourErrorKind::ResourceLimit
        } else {
            TextContourErrorKind::Shaping
        },
        path: error.path,
        message: error.message,
    }
}

fn map_outline_error(error: FontOutlineError) -> TextContourError {
    TextContourError {
        kind: if error.kind == FontOutlineErrorKind::ResourceLimit {
            TextContourErrorKind::ResourceLimit
        } else {
            TextContourErrorKind::Outline
        },
        path: error.path,
        message: error.message,
    }
}

fn map_bezier_error(error: TextBezierError) -> TextContourError {
    TextContourError {
        kind: if error.kind == TextBezierErrorKind::ResourceLimit {
            TextContourErrorKind::ResourceLimit
        } else {
            TextContourErrorKind::InvalidInput
        },
        path: "$.contours".to_owned(),
        message: error.message,
    }
}

fn resource_error(path: &'static str, message: &'static str) -> TextContourError {
    contour_error(TextContourErrorKind::ResourceLimit, path, message)
}

fn contour_error(
    kind: TextContourErrorKind,
    path: impl Into<String>,
    message: &'static str,
) -> TextContourError {
    TextContourError {
        kind,
        path: path.into(),
        message,
    }
}

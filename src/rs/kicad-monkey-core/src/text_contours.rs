//! Bounded composition of native shaping and outlines into positioned contours.

use crate::text_shaping::TextShapingFace;
use crate::{
    FontOutlineError, FontOutlineErrorKind, FontOutlineFace, FontOutlineFaceRequest,
    FontOutlineLimits, HintedFontOutlineFace, TextBezierError, TextBezierErrorKind,
    TextBezierLimits, TextPoint, TextShapingError, TextShapingErrorKind, TextShapingLimits,
    TextShapingOutput, flatten_cubic_bezier, flatten_quadratic_bezier, shape_text_a0,
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
/// KiCad's subscript/superscript glyph size ratio from `outline_font.h`.
pub const KICAD_SUBSCRIPT_SUPERSCRIPT_SIZE_RATIO: f64 = 0.64;
/// KiCad's rounded subscript/superscript face size: `round(1433 * 0.64)`.
pub const KICAD_SUBSCRIPT_FACE_SCALER: f64 = 917.0;
/// KiCad's subscript baseline offset as a fraction of the styled face size.
pub const KICAD_SUBSCRIPT_VERTICAL_OFFSET_RATIO: f64 = -0.25;
/// KiCad's superscript baseline offset as a fraction of the styled face size.
pub const KICAD_SUPERSCRIPT_VERTICAL_OFFSET_RATIO: f64 = 0.45;

/// One raw text run before alignment, markup, mirroring, or rotation.
#[derive(Clone, Copy, Debug)]
pub struct TextContourRequest<'a> {
    pub shaping: &'a ShapingInput,
    pub size_x: f64,
    pub size_y: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub max_error: f64,
    pub fake_bold: bool,
    pub fake_italic: bool,
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
    pub glyphs: usize,
    pub outline_commands: usize,
    pub bezier_work_items: usize,
    pub peak_temporary_bezier_points: usize,
}

/// Reusable hinted shaping/outline state for one bounded internal text block.
pub(crate) struct HintedTextContourSession<'a> {
    shaping: TextShapingFace<'a>,
    outline: HintedFontOutlineFace<'a>,
    subscript_outline: Option<HintedFontOutlineFace<'a>>,
}

/// KiCad text-run style selecting the face scaler and baseline offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextRunStyle {
    Normal,
    Subscript,
    Superscript,
}

impl TextRunStyle {
    /// Internal-unit face scaler used for cursor and offset conversion.
    fn face_scaler(self) -> f64 {
        match self {
            Self::Normal => KICAD_OUTLINE_FACE_SCALER,
            Self::Subscript | Self::Superscript => KICAD_SUBSCRIPT_FACE_SCALER,
        }
    }

    /// Baseline offset as a fraction of the styled face scaler.
    fn vertical_offset_ratio(self) -> f64 {
        match self {
            Self::Normal => 0.0,
            Self::Subscript => KICAD_SUBSCRIPT_VERTICAL_OFFSET_RATIO,
            Self::Superscript => KICAD_SUPERSCRIPT_VERTICAL_OFFSET_RATIO,
        }
    }
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
    shape_text_contours_with_outline_mode(font_bytes, request, limits, OutlineMode::Unhinted)
}

/// Shape and place one plain text run using KiCad-compatible embedded hinting.
pub fn shape_text_contours_hinted_a0(
    font_bytes: &[u8],
    request: TextContourRequest<'_>,
    limits: TextContourLimits,
) -> Result<TextContourOutput, TextContourError> {
    shape_text_contours_with_outline_mode(font_bytes, request, limits, OutlineMode::Hinted)
}

impl<'a> HintedTextContourSession<'a> {
    pub(crate) fn new(
        font_bytes: &'a [u8],
        shaping: &ShapingInput,
        limits: TextContourLimits,
    ) -> Result<Self, TextContourError> {
        preflight_outline_metadata(font_bytes, shaping, limits.outline)?;
        let shaping_face =
            TextShapingFace::new(font_bytes, shaping, limits.shaping).map_err(map_shaping_error)?;
        let variations = shaping
            .variations
            .iter()
            .map(|variation| OutlineVariation {
                axis: OutlineTag(variation.axis.0.clone()),
                value: FiniteFloat::try_from(variation.value.get())
                    .expect("shaping variation coordinates are finite"),
            })
            .collect::<Vec<_>>();
        let face_request = FontOutlineFaceRequest {
            font_id: shaping.font_id.as_str(),
            font_sha256: &shaping.font_sha256.0,
            face_index: shaping.face_index,
            variations: &variations,
        };
        let outline = HintedFontOutlineFace::new(font_bytes, face_request, limits.outline)
            .map_err(map_outline_error)?;
        // The styled face is built only when the block can request styled runs,
        // so plain blocks keep exactly one hinting instance.
        let subscript_outline = if shaping.text.contains("_{") || shaping.text.contains("^{") {
            Some(
                HintedFontOutlineFace::new_subscript(font_bytes, face_request, limits.outline)
                    .map_err(map_outline_error)?,
            )
        } else {
            None
        };
        Ok(Self {
            shaping: shaping_face,
            outline,
            subscript_outline,
        })
    }

    pub(crate) fn shape_input_styled(
        &self,
        request: TextContourRequest<'_>,
        limits: TextContourLimits,
        style: TextRunStyle,
    ) -> Result<TextContourOutput, TextContourError> {
        validate_request(request)?;
        let shaped = self
            .shaping
            .shape_input(request.shaping, limits.shaping)
            .map_err(map_shaping_error)?;
        let face = match style {
            TextRunStyle::Normal => &self.outline,
            TextRunStyle::Subscript | TextRunStyle::Superscript => {
                self.subscript_outline.as_ref().ok_or_else(|| {
                    contour_error(
                        TextContourErrorKind::InvalidInput,
                        "$.markup",
                        "styled run requested without a styled outline face",
                    )
                })?
            }
        };
        compose_shaped_contours(face, &shaped, request, limits, style)
    }
}

#[derive(Clone, Copy)]
enum OutlineMode {
    Unhinted,
    Hinted,
}

trait ContourOutlineFace {
    fn units_per_em(&self) -> u16;

    fn outline_to_internal(&self) -> f64;

    fn quantizes_cursor_to_internal_units(&self) -> bool;

    fn extract_glyph_with_limit(
        &self,
        glyph_id: u32,
        max_commands: usize,
    ) -> Result<crate::FontOutlineOutput, FontOutlineError>;
}

impl ContourOutlineFace for FontOutlineFace<'_> {
    fn units_per_em(&self) -> u16 {
        FontOutlineFace::units_per_em(self)
    }

    fn outline_to_internal(&self) -> f64 {
        KICAD_OUTLINE_FACE_SCALER / f64::from(self.units_per_em())
    }

    fn quantizes_cursor_to_internal_units(&self) -> bool {
        false
    }

    fn extract_glyph_with_limit(
        &self,
        glyph_id: u32,
        max_commands: usize,
    ) -> Result<crate::FontOutlineOutput, FontOutlineError> {
        FontOutlineFace::extract_glyph_with_limit(self, glyph_id, max_commands)
    }
}

impl ContourOutlineFace for HintedFontOutlineFace<'_> {
    fn units_per_em(&self) -> u16 {
        HintedFontOutlineFace::units_per_em(self)
    }

    fn outline_to_internal(&self) -> f64 {
        1.0
    }

    fn quantizes_cursor_to_internal_units(&self) -> bool {
        true
    }

    fn extract_glyph_with_limit(
        &self,
        glyph_id: u32,
        max_commands: usize,
    ) -> Result<crate::FontOutlineOutput, FontOutlineError> {
        HintedFontOutlineFace::extract_glyph_with_limit(self, glyph_id, max_commands)
    }
}

fn shape_text_contours_with_outline_mode(
    font_bytes: &[u8],
    request: TextContourRequest<'_>,
    limits: TextContourLimits,
    outline_mode: OutlineMode,
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
    let face_request = FontOutlineFaceRequest {
        font_id: request.shaping.font_id.as_str(),
        font_sha256: &request.shaping.font_sha256.0,
        face_index: request.shaping.face_index,
        variations: &variations,
    };
    match outline_mode {
        OutlineMode::Unhinted => {
            let face = FontOutlineFace::new(font_bytes, face_request, limits.outline)
                .map_err(map_outline_error)?;
            compose_shaped_contours(&face, &shaped, request, limits, TextRunStyle::Normal)
        }
        OutlineMode::Hinted => {
            let face = HintedFontOutlineFace::new(font_bytes, face_request, limits.outline)
                .map_err(map_outline_error)?;
            compose_shaped_contours(&face, &shaped, request, limits, TextRunStyle::Normal)
        }
    }
}

fn compose_shaped_contours(
    face: &impl ContourOutlineFace,
    shaped: &TextShapingOutput,
    request: TextContourRequest<'_>,
    limits: TextContourLimits,
    style: TextRunStyle,
) -> Result<TextContourOutput, TextContourError> {
    if face.units_per_em() != shaped.units_per_em {
        return Err(contour_error(
            TextContourErrorKind::InvalidInput,
            "$.face_index",
            "shaping and outline backends disagree on units per em",
        ));
    }

    let outline_to_internal = face.outline_to_internal();
    // Styled runs live on KiCad's rounded 917-unit face, so shaped positions
    // convert through the styled scaler while the mm conversion below stays on
    // the base 1433-unit scale, yielding the 0.64 visual size ratio.
    let position_x_to_internal = style.face_scaler() / f64::from(request.shaping.scale_x);
    let position_y_to_internal = style.face_scaler() / f64::from(request.shaping.scale_y);
    // `ratio * face_scaler` internal units divided by `face_scaler / scale_y`
    // internal units per position unit leaves `ratio * scale_y` position units.
    let style_offset_y = style.vertical_offset_ratio() * f64::from(request.shaping.scale_y);
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
        if let Some(outline) =
            extract_styled_outline(face, glyph.glyph_id, remaining_commands, request)?
        {
            output.charge_commands(outline.commands.len())?;
            output.append_outline(
                &outline.commands,
                cursor_x + glyph.x_offset.get() as f64,
                cursor_y + glyph.y_offset.get() as f64 + style_offset_y,
            )?;
        }
        cursor_x = advance_cursor(
            cursor_x,
            glyph.x_advance.get() as f64,
            position_x_to_internal,
            face.quantizes_cursor_to_internal_units(),
        );
        cursor_y = advance_cursor(
            cursor_y,
            glyph.y_advance.get() as f64,
            position_y_to_internal,
            face.quantizes_cursor_to_internal_units(),
        );
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
    Ok(output.finish(
        advance_x,
        advance_y,
        shaped.units_per_em,
        shaped.glyphs.len(),
    ))
}

/// Extract one glyph outline and apply any requested fake bold/italic pass.
///
/// A missing outline (for example whitespace) yields `Ok(None)`. Fake styles
/// are defined on FreeType's hinted 26.6 grid, so the unhinted design-unit
/// path rejects them.
fn extract_styled_outline(
    face: &impl ContourOutlineFace,
    glyph_id: u32,
    max_commands: usize,
    request: TextContourRequest<'_>,
) -> Result<Option<crate::FontOutlineOutput>, TextContourError> {
    match face.extract_glyph_with_limit(glyph_id, max_commands) {
        Ok(mut outline) => {
            if request.fake_bold || request.fake_italic {
                if !face.quantizes_cursor_to_internal_units() {
                    return Err(contour_error(
                        TextContourErrorKind::InvalidInput,
                        "$.fake_style",
                        "fake bold/italic synthesis requires the hinted outline path",
                    ));
                }
                crate::fake_style::apply_fake_style(
                    &mut outline.commands,
                    request.fake_bold,
                    request.fake_italic,
                )?;
            }
            Ok(Some(outline))
        }
        Err(error) if error.kind == FontOutlineErrorKind::MissingOutline => Ok(None),
        Err(error) => Err(map_outline_error(error)),
    }
}

fn advance_cursor(current: f64, advance: f64, to_internal: f64, quantize: bool) -> f64 {
    let next = current + advance;
    if quantize {
        (next * to_internal).trunc() / to_internal
    } else {
        next
    }
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

    fn finish(
        self,
        advance_x: f64,
        advance_y: f64,
        units_per_em: u16,
        glyphs: usize,
    ) -> TextContourOutput {
        TextContourOutput {
            contours: self.contours,
            advance_x,
            advance_y,
            units_per_em,
            glyphs,
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

#[cfg(test)]
mod tests {
    use super::advance_cursor;

    #[test]
    fn hinted_cursor_quantizes_each_advance_to_internal_units() {
        let to_internal = 1433.0 / 2048.0;
        let first = advance_cursor(0.0, 1366.0, to_internal, true);
        let second = advance_cursor(first, 1366.0, to_internal, true);

        let expected_first = (1366.0_f64 * to_internal).trunc() / to_internal;
        let expected_second = ((expected_first + 1366.0) * to_internal).trunc() / to_internal;
        assert_eq!(first, expected_first);
        assert_eq!(second, expected_second);
        assert_ne!(second, advance_cursor(0.0, 2732.0, to_internal, true));
    }

    #[test]
    fn hinted_cursor_truncates_negative_internal_positions_toward_zero() {
        let to_internal = 1433.0 / 2048.0;
        let position = advance_cursor(0.0, -1366.0, to_internal, true);

        assert_eq!(position, (-1366.0_f64 * to_internal).trunc() / to_internal);
        assert!(position > -1366.0);
    }

    #[test]
    fn unhinted_cursor_preserves_the_shaping_position() {
        assert_eq!(advance_cursor(12.5, 3.25, 0.75, false), 15.75);
    }
}

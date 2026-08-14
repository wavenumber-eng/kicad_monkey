//! Bounded KiCad single-line alignment and authored-origin transforms.

use crate::{
    TextContourError, TextContourErrorKind, TextContourLimits, TextContourOutput,
    TextContourRequest, shape_text_contours_a0,
};
use kicad_monkey_contracts::generated::shaping_record::ShapingInput;

/// KiCad outline-text height compensation used by vertical alignment.
pub const KICAD_TEXT_HEIGHT_FUDGE_FACTOR: f64 = 1.17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextHorizontalAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextVerticalAlignment {
    Top,
    Center,
    Bottom,
}

/// One plain single-line text layout request.
#[derive(Clone, Copy, Debug)]
pub struct TextLayoutRequest<'a> {
    pub shaping: &'a ShapingInput,
    pub size_x: f64,
    pub size_y: f64,
    pub position_x: f64,
    pub position_y: f64,
    pub angle_degrees: f64,
    pub mirrored: bool,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub max_error: f64,
}

/// Render one plain line with KiCad alignment, mirroring, and rotation order.
pub fn layout_single_line_text_a0(
    font_bytes: &[u8],
    request: TextLayoutRequest<'_>,
    limits: TextContourLimits,
) -> Result<TextContourOutput, TextContourError> {
    let transform = validated_transform(request)?;
    let mut output = shape_text_contours_a0(
        font_bytes,
        TextContourRequest {
            shaping: request.shaping,
            size_x: request.size_x,
            size_y: request.size_y,
            origin_x: 0.0,
            origin_y: 0.0,
            max_error: request.max_error,
        },
        limits,
    )?;
    let translation = aligned_translation(request, output.advance_x)?;
    transform_output(&mut output, translation, transform)?;
    Ok(output)
}

#[derive(Clone, Copy)]
struct LayoutTransform {
    origin_x: f64,
    origin_y: f64,
    sin_angle: f64,
    cos_angle: f64,
    mirrored: bool,
    rotated: bool,
}

fn validated_transform(
    request: TextLayoutRequest<'_>,
) -> Result<LayoutTransform, TextContourError> {
    if [
        request.angle_degrees,
        request.position_x,
        request.position_y,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
    {
        return Err(invalid_layout(
            "$.transform",
            "text angle and authored position must be finite",
        ));
    }
    let radians = request.angle_degrees.to_radians();
    let (sin_angle, cos_angle) = radians.sin_cos();
    if [radians, sin_angle, cos_angle]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(invalid_layout(
            "$.angle_degrees",
            "text angle is outside the finite transform range",
        ));
    }
    Ok(LayoutTransform {
        origin_x: request.position_x,
        origin_y: request.position_y,
        sin_angle,
        cos_angle,
        mirrored: request.mirrored,
        rotated: request.angle_degrees != 0.0,
    })
}

fn aligned_translation(
    request: TextLayoutRequest<'_>,
    line_width: f64,
) -> Result<(f64, f64), TextContourError> {
    let horizontal_offset = match request.horizontal_alignment {
        TextHorizontalAlignment::Left => 0.0,
        TextHorizontalAlignment::Center => -line_width / 2.0,
        TextHorizontalAlignment::Right => -line_width,
    };
    let height = request.size_y * KICAD_TEXT_HEIGHT_FUDGE_FACTOR;
    let vertical_offset = match request.vertical_alignment {
        TextVerticalAlignment::Top => request.size_y,
        TextVerticalAlignment::Center => request.size_y - height / 2.0,
        TextVerticalAlignment::Bottom => request.size_y - height,
    };
    let translation_x = request.position_x + horizontal_offset;
    let translation_y = request.position_y + vertical_offset;
    if [translation_x, translation_y]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(invalid_layout(
            "$.position",
            "aligned text position is not finite",
        ));
    }
    Ok((translation_x, translation_y))
}

fn transform_output(
    output: &mut TextContourOutput,
    translation: (f64, f64),
    transform: LayoutTransform,
) -> Result<(), TextContourError> {
    for contour in &mut output.contours {
        for point in &mut contour.points {
            let mut x = point.x + translation.0;
            let mut y = point.y + translation.1;
            if transform.mirrored {
                x = transform.origin_x - (x - transform.origin_x);
            }
            if transform.rotated {
                let delta_x = x - transform.origin_x;
                let delta_y = y - transform.origin_y;
                x = transform.origin_x
                    + delta_x * transform.cos_angle
                    + delta_y * transform.sin_angle;
                y = transform.origin_y + delta_y * transform.cos_angle
                    - delta_x * transform.sin_angle;
            }
            if [x, y].into_iter().any(|value| !value.is_finite()) {
                return Err(invalid_layout(
                    "$.contours",
                    "transformed text coordinate is not finite",
                ));
            }
            point.x = x;
            point.y = y;
        }
    }
    Ok(())
}

fn invalid_layout(path: &'static str, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::InvalidInput,
        path: path.to_owned(),
        message,
    }
}

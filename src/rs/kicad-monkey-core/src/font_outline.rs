//! Bounded native glyph-outline extraction over caller-supplied font bytes.

use kicad_monkey_contracts::FiniteFloat;
use kicad_monkey_contracts::generated::outline_vector::{
    FontVariationCoordinate, OutlineClose, OutlineCommand, OutlineCurveTo, OutlineLineTo,
    OutlineMoveTo, OutlineQuadTo,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag};

/// Outline implementation and upstream parser version pinned by this workspace.
pub const FONT_OUTLINE_ENGINE: &str = "ttf-parser-0.25.1";

/// Explicit ceilings for one outline extraction call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontOutlineLimits {
    pub max_font_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_variations: usize,
    pub max_commands: usize,
}

impl Default for FontOutlineLimits {
    fn default() -> Self {
        Self {
            max_font_bytes: 256 * 1024 * 1024,
            max_metadata_bytes: 16 * 1024 * 1024,
            max_variations: 4_096,
            max_commands: 16 * 1024 * 1024,
        }
    }
}

/// Contract-aligned metadata for one out-of-band glyph request.
#[derive(Clone, Copy, Debug)]
pub struct FontOutlineRequest<'a> {
    pub font_id: &'a str,
    pub font_sha256: &'a str,
    pub face_index: u32,
    pub variations: &'a [FontVariationCoordinate],
    pub glyph_id: u32,
}

/// Native outline in unscaled font design units.
#[derive(Clone, Debug)]
pub struct FontOutlineOutput {
    pub commands: Vec<OutlineCommand>,
    pub units_per_em: u16,
}

/// Stable failure categories for native outline extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontOutlineErrorKind {
    ResourceLimit,
    InvalidContract,
    HashMismatch,
    InvalidFont,
    InvalidInput,
    MissingOutline,
}

/// Fail-closed native outline extraction error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontOutlineError {
    pub kind: FontOutlineErrorKind,
    pub path: String,
    pub message: &'static str,
}

impl fmt::Display for FontOutlineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.message, self.path)
    }
}

impl std::error::Error for FontOutlineError {}

/// Extract one validated glyph outline without exposing the parser dependency.
pub fn extract_font_outline_a0(
    font_bytes: &[u8],
    request: FontOutlineRequest<'_>,
    limits: FontOutlineLimits,
) -> Result<FontOutlineOutput, FontOutlineError> {
    preflight(font_bytes, request, limits)?;
    validate_hash(font_bytes, request.font_sha256)?;
    let mut face = Face::parse(font_bytes, request.face_index).map_err(|_| {
        outline_error(
            FontOutlineErrorKind::InvalidFont,
            "$.face_index",
            "font bytes or face index are invalid",
        )
    })?;
    apply_variations(&mut face, request.variations)?;
    let glyph_id = u16::try_from(request.glyph_id).map(GlyphId).map_err(|_| {
        outline_error(
            FontOutlineErrorKind::InvalidInput,
            "$.glyph_id",
            "glyph ID exceeds the OpenType uint16 range",
        )
    })?;
    let mut builder = BoundedOutlineBuilder::new(limits.max_commands);
    let outlined = face.outline_glyph(glyph_id, &mut builder).is_some();
    if outlined {
        builder.finish_open_contour();
    }
    if builder.exceeded {
        return Err(outline_error(
            FontOutlineErrorKind::ResourceLimit,
            "$.commands",
            "outline command count exceeds the configured limit",
        ));
    }
    if !outlined {
        return Err(outline_error(
            FontOutlineErrorKind::MissingOutline,
            "$.glyph_id",
            "font does not contain an outline for the requested glyph",
        ));
    }
    Ok(FontOutlineOutput {
        commands: builder.commands,
        units_per_em: face.units_per_em(),
    })
}

fn preflight(
    font_bytes: &[u8],
    request: FontOutlineRequest<'_>,
    limits: FontOutlineLimits,
) -> Result<(), FontOutlineError> {
    validate_resource_limits(font_bytes, request, limits)?;
    validate_request_contract(request)
}

fn validate_resource_limits(
    font_bytes: &[u8],
    request: FontOutlineRequest<'_>,
    limits: FontOutlineLimits,
) -> Result<(), FontOutlineError> {
    if font_bytes.len() > limits.max_font_bytes || request.variations.len() > limits.max_variations
    {
        return Err(outline_error(
            FontOutlineErrorKind::ResourceLimit,
            "$",
            "outline input exceeds a configured count or byte limit",
        ));
    }
    let metadata_bytes = request
        .font_id
        .len()
        .checked_add(request.font_sha256.len())
        .and_then(|total| {
            request
                .variations
                .iter()
                .try_fold(total, |total, value| total.checked_add(value.axis.0.len()))
        })
        .ok_or_else(|| {
            outline_error(
                FontOutlineErrorKind::ResourceLimit,
                "$",
                "outline metadata byte count overflowed",
            )
        })?;
    if metadata_bytes > limits.max_metadata_bytes {
        return Err(outline_error(
            FontOutlineErrorKind::ResourceLimit,
            "$",
            "outline metadata exceeds the configured byte limit",
        ));
    }
    Ok(())
}

fn validate_request_contract(request: FontOutlineRequest<'_>) -> Result<(), FontOutlineError> {
    if !valid_stable_text_id(request.font_id) || !valid_sha256(request.font_sha256) {
        return Err(outline_error(
            FontOutlineErrorKind::InvalidContract,
            "$",
            "outline request metadata does not satisfy the a0 contract",
        ));
    }
    validate_variation_metadata(request.variations)
}

fn validate_variation_metadata(
    variations: &[FontVariationCoordinate],
) -> Result<(), FontOutlineError> {
    let mut variation_tags = HashSet::with_capacity(variations.len());
    for (index, variation) in variations.iter().enumerate() {
        let tag = variation.axis.0.as_str();
        if tag.len() != 4
            || !tag.bytes().all(|byte| (b' '..=b'~').contains(&byte))
            || !variation_tags.insert(tag)
        {
            return Err(outline_error(
                FontOutlineErrorKind::InvalidContract,
                format!("$.variations[{index}].axis"),
                "variation axes must be printable, four-byte, and unique",
            ));
        }
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_stable_text_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn validate_hash(font_bytes: &[u8], expected: &str) -> Result<(), FontOutlineError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let matches = Sha256::digest(font_bytes)
        .iter()
        .zip(expected.as_bytes().chunks_exact(2))
        .all(|(byte, pair)| pair == [HEX[(byte >> 4) as usize], HEX[(byte & 0x0f) as usize]]);
    if matches {
        Ok(())
    } else {
        Err(outline_error(
            FontOutlineErrorKind::HashMismatch,
            "$.font_sha256",
            "font buffer does not match the declared SHA-256",
        ))
    }
}

fn apply_variations(
    face: &mut Face<'_>,
    variations: &[FontVariationCoordinate],
) -> Result<(), FontOutlineError> {
    if variations.is_empty() {
        return Ok(());
    }
    let axis_tags = face
        .variation_axes()
        .into_iter()
        .map(|axis| axis.tag)
        .collect::<HashSet<_>>();
    if axis_tags.is_empty() {
        return Err(outline_error(
            FontOutlineErrorKind::InvalidInput,
            "$.variations",
            "font does not support variation coordinates",
        ));
    }
    for (index, variation) in variations.iter().enumerate() {
        let bytes: [u8; 4] = variation.axis.0.as_bytes().try_into().map_err(|_| {
            outline_error(
                FontOutlineErrorKind::InvalidContract,
                format!("$.variations[{index}].axis"),
                "variation axis must be a four-byte OpenType tag",
            )
        })?;
        let tag = Tag::from_bytes(&bytes);
        if !axis_tags.contains(&tag) {
            return Err(outline_error(
                FontOutlineErrorKind::InvalidInput,
                format!("$.variations[{index}].axis"),
                "font does not define the requested variation axis",
            ));
        }
        let value = variation.value.get() as f32;
        if !value.is_finite() {
            return Err(outline_error(
                FontOutlineErrorKind::InvalidInput,
                format!("$.variations[{index}].value"),
                "variation value is outside the finite f32 range",
            ));
        }
        face.set_variation(tag, value).ok_or_else(|| {
            outline_error(
                FontOutlineErrorKind::InvalidFont,
                format!("$.variations[{index}]"),
                "font variation coordinates could not be applied",
            )
        })?;
    }
    Ok(())
}

struct BoundedOutlineBuilder {
    commands: Vec<OutlineCommand>,
    limit: usize,
    exceeded: bool,
    contour_open: bool,
}

impl BoundedOutlineBuilder {
    fn new(limit: usize) -> Self {
        Self {
            commands: Vec::with_capacity(limit.min(256)),
            limit,
            exceeded: false,
            contour_open: false,
        }
    }

    fn push(&mut self, command: OutlineCommand) {
        if self.commands.len() < self.limit {
            self.commands.push(command);
        } else {
            self.exceeded = true;
        }
    }

    fn finish_open_contour(&mut self) {
        if self.contour_open {
            self.close();
        }
    }
}

impl OutlineBuilder for BoundedOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contour_open = true;
        self.push(
            OutlineMoveTo {
                kind: "move_to".to_owned(),
                x: finite(x),
                y: finite(y),
            }
            .into(),
        );
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push(
            OutlineLineTo {
                kind: "line_to".to_owned(),
                x: finite(x),
                y: finite(y),
            }
            .into(),
        );
    }

    fn quad_to(&mut self, control_x: f32, control_y: f32, x: f32, y: f32) {
        self.push(
            OutlineQuadTo {
                control_x: finite(control_x),
                control_y: finite(control_y),
                kind: "quad_to".to_owned(),
                x: finite(x),
                y: finite(y),
            }
            .into(),
        );
    }

    fn curve_to(
        &mut self,
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        x: f32,
        y: f32,
    ) {
        self.push(
            OutlineCurveTo {
                control1_x: finite(control1_x),
                control1_y: finite(control1_y),
                control2_x: finite(control2_x),
                control2_y: finite(control2_y),
                kind: "curve_to".to_owned(),
                x: finite(x),
                y: finite(y),
            }
            .into(),
        );
    }

    fn close(&mut self) {
        self.push(
            OutlineClose {
                kind: "close".to_owned(),
            }
            .into(),
        );
        self.contour_open = false;
    }
}

fn finite(value: f32) -> FiniteFloat {
    FiniteFloat::try_from(f64::from(value)).expect("OpenType outline coordinates are finite")
}

fn outline_error(
    kind: FontOutlineErrorKind,
    path: impl Into<String>,
    message: &'static str,
) -> FontOutlineError {
    FontOutlineError {
        kind,
        path: path.into(),
        message,
    }
}

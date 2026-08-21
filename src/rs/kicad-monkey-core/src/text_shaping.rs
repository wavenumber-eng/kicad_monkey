//! Deterministic native OpenType shaping over caller-supplied font bytes.

use harfrust::{
    BufferClusterLevel, BufferFlags, Direction, Feature, FontRef, Language, Script, ShapeOptions,
    ShaperData, ShaperInstance, Tag, UnicodeBuffer, Variation,
};
use kicad_monkey_contracts::generated::shaping_record::{
    DefaultIgnorablePolicy, ShapedGlyph, ShapingClusterLevel, ShapingInput, TextDirection,
};
use kicad_monkey_contracts::{JavaScriptSafeInteger, validate_shaping_input_contract};
use read_fonts::TableProvider;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;

/// Shaping implementation and upstream compatibility level pinned by this workspace.
pub const TEXT_SHAPING_ENGINE: &str = "harfrust-0.13.0-harfbuzz-14.3.0";

/// Explicit ceilings for one shaping call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextShapingLimits {
    pub max_font_bytes: usize,
    pub max_text_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_features: usize,
    pub max_variations: usize,
    pub max_glyphs: usize,
}

impl Default for TextShapingLimits {
    fn default() -> Self {
        Self {
            max_font_bytes: 256 * 1024 * 1024,
            max_text_bytes: 16 * 1024 * 1024,
            max_metadata_bytes: 16 * 1024 * 1024,
            max_features: 65_536,
            max_variations: 4_096,
            max_glyphs: 16 * 1024 * 1024,
        }
    }
}

/// Native shaping output before outline extraction or KiCad placement.
#[derive(Clone, Debug)]
pub struct TextShapingOutput {
    pub glyphs: Vec<ShapedGlyph>,
    pub units_per_em: u16,
}

/// Validated font and variation state reused by one internal multi-run operation.
pub(crate) struct TextShapingFace<'a> {
    font: FontRef<'a>,
    shaper_data: ShaperData,
    instance: ShaperInstance,
    font_id: String,
    font_sha256: String,
    face_index: u32,
    variations: Vec<(String, u64)>,
}

/// Stable failure categories for native shaping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextShapingErrorKind {
    ResourceLimit,
    InvalidContract,
    HashMismatch,
    InvalidFont,
    InvalidInput,
}

/// Fail-closed native shaping error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextShapingError {
    pub kind: TextShapingErrorKind,
    pub path: String,
    pub message: &'static str,
}

impl fmt::Display for TextShapingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.message, self.path)
    }
}

impl std::error::Error for TextShapingError {}

/// Shape one validated a0 input against an out-of-band font buffer.
pub fn shape_text_a0(
    font_bytes: &[u8],
    input: &ShapingInput,
    limits: TextShapingLimits,
) -> Result<TextShapingOutput, TextShapingError> {
    let face = TextShapingFace::new(font_bytes, input, limits)?;
    face.shape_validated(input, limits)
}

impl<'a> TextShapingFace<'a> {
    pub(crate) fn new(
        font_bytes: &'a [u8],
        input: &ShapingInput,
        limits: TextShapingLimits,
    ) -> Result<Self, TextShapingError> {
        preflight(font_bytes, input, limits)?;
        validate_shaping_input_contract(input).map_err(|error| TextShapingError {
            kind: TextShapingErrorKind::InvalidContract,
            path: error.path,
            message: error.message,
        })?;
        validate_font_hash(font_bytes, input)?;

        let font = FontRef::from_index(font_bytes, input.face_index).map_err(|_| {
            shaping_error(
                TextShapingErrorKind::InvalidFont,
                "$.face_index",
                "font bytes or face index are invalid",
            )
        })?;
        validate_variation_axes(&font, input)?;
        let instance = ShaperInstance::from_variations(&font, variations(input)?);
        let shaper_data = ShaperData::new(&font);
        Ok(Self {
            font,
            shaper_data,
            instance,
            font_id: input.font_id.as_str().to_owned(),
            font_sha256: input.font_sha256.0.clone(),
            face_index: input.face_index,
            variations: input
                .variations
                .iter()
                .map(|variation| (variation.axis.0.clone(), variation.value.get().to_bits()))
                .collect(),
        })
    }

    pub(crate) fn shape_input(
        &self,
        input: &ShapingInput,
        limits: TextShapingLimits,
    ) -> Result<TextShapingOutput, TextShapingError> {
        preflight_input(input, limits)?;
        validate_shaping_input_contract(input).map_err(|error| TextShapingError {
            kind: TextShapingErrorKind::InvalidContract,
            path: error.path,
            message: error.message,
        })?;
        self.validate_identity(input)?;
        self.shape_validated(input, limits)
    }

    fn shape_validated(
        &self,
        input: &ShapingInput,
        limits: TextShapingLimits,
    ) -> Result<TextShapingOutput, TextShapingError> {
        let shaper = self
            .shaper_data
            .shaper(&self.font)
            .instance(Some(&self.instance))
            .build();
        let features = features(input);
        let buffer = shaping_buffer(input)?;
        let output = shaper.shape(
            buffer,
            ShapeOptions::new()
                .scale_separate(Some((input.scale_x, input.scale_y)))
                .features(&features),
        );
        if output.len() > limits.max_glyphs {
            return Err(shaping_error(
                TextShapingErrorKind::ResourceLimit,
                "$.glyphs",
                "shaped glyph count exceeds the configured limit",
            ));
        }
        let glyphs = output
            .glyph_infos()
            .iter()
            .zip(output.glyph_positions())
            .map(|(info, position)| ShapedGlyph {
                glyph_id: info.glyph_id,
                cluster: info.cluster,
                x_advance: safe_integer(position.x_advance),
                y_advance: safe_integer(position.y_advance),
                x_offset: safe_integer(position.x_offset),
                y_offset: safe_integer(position.y_offset),
                unsafe_to_break: info.unsafe_to_break(),
                safe_to_insert_tatweel: info.safe_to_insert_tatweel(),
                unsafe_to_concat: info.unsafe_to_concat(),
            })
            .collect();
        Ok(TextShapingOutput {
            glyphs,
            units_per_em: u16::try_from(shaper.units_per_em())
                .expect("HarfRust derives units per em from a u16 font field"),
        })
    }

    fn validate_identity(&self, input: &ShapingInput) -> Result<(), TextShapingError> {
        let variations_match =
            input.variations.len() == self.variations.len()
                && input.variations.iter().zip(&self.variations).all(
                    |(variation, (axis, value))| {
                        variation.axis.0 == *axis && variation.value.get().to_bits() == *value
                    },
                );
        if input.font_id.as_str() != self.font_id
            || input.font_sha256.0 != self.font_sha256
            || input.face_index != self.face_index
            || !variations_match
        {
            return Err(shaping_error(
                TextShapingErrorKind::InvalidInput,
                "$.font",
                "shaping run does not match the reusable font face",
            ));
        }
        Ok(())
    }
}

fn validate_variation_axes(
    font: &FontRef<'_>,
    input: &ShapingInput,
) -> Result<(), TextShapingError> {
    if input.variations.is_empty() {
        return Ok(());
    }
    let fvar_tag = Tag::new(b"fvar");
    let fvar = font.fvar().map_err(|_| {
        if font.data_for_tag(fvar_tag).is_some() {
            shaping_error(
                TextShapingErrorKind::InvalidFont,
                "$.variations",
                "font variation axis table is malformed",
            )
        } else {
            shaping_error(
                TextShapingErrorKind::InvalidInput,
                "$.variations",
                "font does not support variation coordinates",
            )
        }
    })?;
    let axes = fvar.axes().map_err(|_| {
        shaping_error(
            TextShapingErrorKind::InvalidFont,
            "$.variations",
            "font variation axis records are malformed",
        )
    })?;
    let axis_tags = axes
        .iter()
        .map(|axis| axis.axis_tag())
        .collect::<HashSet<_>>();
    for (index, variation) in input.variations.iter().enumerate() {
        if !axis_tags.contains(&tag(&variation.axis.0)) {
            return Err(shaping_error(
                TextShapingErrorKind::InvalidInput,
                format!("$.variations[{index}].axis"),
                "font does not define the requested variation axis",
            ));
        }
    }
    Ok(())
}

fn preflight(
    font_bytes: &[u8],
    input: &ShapingInput,
    limits: TextShapingLimits,
) -> Result<(), TextShapingError> {
    if font_bytes.len() > limits.max_font_bytes {
        return Err(shaping_error(
            TextShapingErrorKind::ResourceLimit,
            "$.font",
            "font buffer exceeds the configured byte limit",
        ));
    }
    preflight_input(input, limits)
}

fn preflight_input(
    input: &ShapingInput,
    limits: TextShapingLimits,
) -> Result<(), TextShapingError> {
    let count_over = input.text.len() > limits.max_text_bytes
        || input.features.len() > limits.max_features
        || input.variations.len() > limits.max_variations;
    if count_over {
        return Err(shaping_error(
            TextShapingErrorKind::ResourceLimit,
            "$",
            "shaping input exceeds a configured count or byte limit",
        ));
    }
    let metadata_bytes = input_metadata_bytes(input).ok_or_else(|| {
        shaping_error(
            TextShapingErrorKind::ResourceLimit,
            "$",
            "shaping metadata byte count overflowed",
        )
    })?;
    if metadata_bytes > limits.max_metadata_bytes {
        return Err(shaping_error(
            TextShapingErrorKind::ResourceLimit,
            "$",
            "shaping metadata exceeds the configured byte limit",
        ));
    }
    Ok(())
}

fn input_metadata_bytes(input: &ShapingInput) -> Option<usize> {
    let mut total = input.font_id.len().checked_add(input.font_sha256.0.len())?;
    total = total.checked_add(input.text_index_unit.len())?;
    total = total.checked_add(input.language.as_deref().map_or(0, str::len))?;
    total = total.checked_add(input.script.as_ref().map_or(0, |tag| tag.0.len()))?;
    for feature in &input.features {
        total = total.checked_add(feature.tag.0.len())?;
    }
    for variation in &input.variations {
        total = total.checked_add(variation.axis.0.len())?;
    }
    Some(total)
}

fn validate_font_hash(font_bytes: &[u8], input: &ShapingInput) -> Result<(), TextShapingError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let matches = Sha256::digest(font_bytes)
        .iter()
        .zip(input.font_sha256.0.as_bytes().as_chunks::<2>().0)
        .all(|(byte, pair)| *pair == [HEX[(byte >> 4) as usize], HEX[(byte & 0x0f) as usize]]);
    if matches {
        Ok(())
    } else {
        Err(shaping_error(
            TextShapingErrorKind::HashMismatch,
            "$.font_sha256",
            "font buffer does not match the declared SHA-256",
        ))
    }
}

fn variations(input: &ShapingInput) -> Result<Vec<Variation>, TextShapingError> {
    input
        .variations
        .iter()
        .map(|variation| {
            let value = variation.value.get() as f32;
            if !value.is_finite() {
                return Err(shaping_error(
                    TextShapingErrorKind::InvalidInput,
                    "$.variations",
                    "variation value is outside HarfRust's finite f32 range",
                ));
            }
            Ok(Variation {
                tag: tag(&variation.axis.0),
                value,
            })
        })
        .collect()
}

fn features(input: &ShapingInput) -> Vec<Feature> {
    input
        .features
        .iter()
        .map(|feature| Feature {
            tag: tag(&feature.tag.0),
            value: feature.value,
            start: feature.start,
            end: feature.end,
        })
        .collect()
}

fn shaping_buffer(input: &ShapingInput) -> Result<UnicodeBuffer, TextShapingError> {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(&input.text);
    buffer.set_direction(match input.direction {
        TextDirection::LeftToRight => Direction::LeftToRight,
        TextDirection::RightToLeft => Direction::RightToLeft,
        TextDirection::TopToBottom => Direction::TopToBottom,
        TextDirection::BottomToTop => Direction::BottomToTop,
    });
    if let Some(script) = &input.script {
        buffer.set_script(script.0.parse::<Script>().map_err(|_| {
            shaping_error(
                TextShapingErrorKind::InvalidInput,
                "$.script",
                "script is not accepted by HarfRust",
            )
        })?);
    }
    if let Some(language) = &input.language {
        buffer.set_language(language.parse::<Language>().map_err(|_| {
            shaping_error(
                TextShapingErrorKind::InvalidInput,
                "$.language",
                "language must be nonempty",
            )
        })?);
    }
    buffer.set_cluster_level(match input.buffer_properties.cluster_level {
        ShapingClusterLevel::MonotoneGraphemes => BufferClusterLevel::MonotoneGraphemes,
        ShapingClusterLevel::MonotoneCharacters => BufferClusterLevel::MonotoneCharacters,
        ShapingClusterLevel::Characters => BufferClusterLevel::Characters,
    });
    buffer.set_flags(buffer_flags(input));
    Ok(buffer)
}

fn buffer_flags(input: &ShapingInput) -> BufferFlags {
    let properties = &input.buffer_properties;
    let mut flags = BufferFlags::empty();
    flags.set(BufferFlags::BEGINNING_OF_TEXT, properties.beginning_of_text);
    flags.set(BufferFlags::END_OF_TEXT, properties.end_of_text);
    flags.set(
        BufferFlags::DO_NOT_INSERT_DOTTED_CIRCLE,
        properties.do_not_insert_dotted_circle,
    );
    flags.set(
        BufferFlags::PRODUCE_UNSAFE_TO_CONCAT,
        properties.produce_unsafe_to_concat,
    );
    flags.set(
        BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL,
        properties.produce_safe_to_insert_tatweel,
    );
    match properties.default_ignorables {
        DefaultIgnorablePolicy::Normal => {}
        DefaultIgnorablePolicy::Preserve => flags.insert(BufferFlags::PRESERVE_DEFAULT_IGNORABLES),
        DefaultIgnorablePolicy::Remove => flags.insert(BufferFlags::REMOVE_DEFAULT_IGNORABLES),
    }
    flags
}

fn tag(value: &str) -> Tag {
    let bytes: [u8; 4] = value.as_bytes().try_into().expect("validated OpenType tag");
    Tag::new(&bytes)
}

fn safe_integer(value: i32) -> JavaScriptSafeInteger {
    JavaScriptSafeInteger::try_from(i64::from(value)).expect("i32 is JavaScript-safe")
}

fn shaping_error(
    kind: TextShapingErrorKind,
    path: impl Into<String>,
    message: &'static str,
) -> TextShapingError {
    TextShapingError {
        kind,
        path: path.into(),
        message,
    }
}

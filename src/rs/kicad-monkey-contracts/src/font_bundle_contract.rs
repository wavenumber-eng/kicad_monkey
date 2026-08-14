use crate::generated::font_bundle_manifest::{FontBundleEntry, FontBundleManifestA0};
use crate::generated::font_resolution_request::FontResolutionRequestA0;
use crate::{ValidationError, validation_error};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// Caller-owned ceilings applied before hashing or retaining font metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontBundleLimits {
    pub max_fonts: usize,
    pub max_font_bytes: usize,
    pub max_total_font_bytes: usize,
    pub max_aliases_per_font: usize,
    pub max_variations_per_font: usize,
    pub max_metadata_string_bytes: usize,
}

impl Default for FontBundleLimits {
    fn default() -> Self {
        Self {
            max_fonts: 4_096,
            max_font_bytes: 256 * 1024 * 1024,
            max_total_font_bytes: 1024 * 1024 * 1024,
            max_aliases_per_font: 4_096,
            max_variations_per_font: 4_096,
            max_metadata_string_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Caller-owned ceilings for one lookup against an already validated bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontResolutionLimits {
    pub max_request_aliases: usize,
    pub max_request_string_bytes: usize,
}

impl Default for FontResolutionLimits {
    fn default() -> Self {
        Self {
            max_request_aliases: 4_096,
            max_request_string_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AliasTarget {
    Unique(usize),
    Ambiguous,
}

/// A manifest whose metadata, out-of-band buffers, and lookup indexes passed validation.
#[derive(Debug)]
pub struct ValidatedFontBundle<'a> {
    manifest: &'a FontBundleManifestA0,
    id_index: HashMap<&'a str, usize>,
    alias_index: HashMap<&'a str, AliasTarget>,
}

/// Validate one manifest against its complete out-of-band font buffer array.
pub fn validate_font_bundle_contract<'a>(
    manifest: &'a FontBundleManifestA0,
    buffers: &[&[u8]],
    limits: FontBundleLimits,
) -> Result<ValidatedFontBundle<'a>, ValidationError> {
    validate_identity(manifest)?;
    if manifest.fonts.len() > limits.max_fonts {
        return Err(error(
            "resource_limit",
            "$.fonts",
            "font count exceeds its limit",
        ));
    }
    if manifest.fonts.len() != buffers.len() {
        return Err(error(
            "buffer_count_mismatch",
            "$.fonts",
            "every supplied buffer must be referenced exactly once",
        ));
    }
    let mut ids = HashSet::with_capacity(manifest.fonts.len());
    let mut slots = HashSet::with_capacity(manifest.fonts.len());
    let mut total_bytes = 0usize;
    let mut metadata_string_bytes = 0usize;
    for (index, font) in manifest.fonts.iter().enumerate() {
        validate_font_metadata(font, index, limits, &mut ids, &mut slots)?;
        metadata_string_bytes = metadata_string_bytes
            .checked_add(font_metadata_string_bytes(font)?)
            .ok_or_else(|| error("resource_limit", "$.fonts", "font metadata bytes overflow"))?;
        if metadata_string_bytes > limits.max_metadata_string_bytes {
            return Err(error(
                "resource_limit",
                "$.fonts",
                "font metadata string bytes exceed their limit",
            ));
        }
        let slot = usize::try_from(font.slot).map_err(|_| {
            error(
                "invalid_slot",
                format!("$.fonts[{index}].slot"),
                "font slot is not platform-sized",
            )
        })?;
        let buffer = buffers.get(slot).ok_or_else(|| {
            error(
                "invalid_slot",
                format!("$.fonts[{index}].slot"),
                "font slot is out of range",
            )
        })?;
        if buffer.len() > limits.max_font_bytes {
            return Err(error(
                "resource_limit",
                format!("$.fonts[{index}].slot"),
                "font buffer exceeds its byte limit",
            ));
        }
        total_bytes = total_bytes.checked_add(buffer.len()).ok_or_else(|| {
            error(
                "resource_limit",
                "$.fonts",
                "total font buffer bytes overflow",
            )
        })?;
        if total_bytes > limits.max_total_font_bytes {
            return Err(error(
                "resource_limit",
                "$.fonts",
                "total font buffer bytes exceed their limit",
            ));
        }
    }
    for (index, font) in manifest.fonts.iter().enumerate() {
        let buffer = buffers[font.slot as usize];
        if !sha256_matches(buffer, &font.sha256.0) {
            return Err(error(
                "hash_mismatch",
                format!("$.fonts[{index}].sha256"),
                "font bytes do not match the declared SHA-256",
            ));
        }
    }
    Ok(ValidatedFontBundle {
        manifest,
        id_index: manifest
            .fonts
            .iter()
            .enumerate()
            .map(|(index, font)| (font.id.as_str(), index))
            .collect(),
        alias_index: build_alias_index(manifest),
    })
}

/// Resolve an explicit font ID or alias through a validated, bounded lookup index.
pub fn resolve_font_selection_contract<'a>(
    bundle: &'a ValidatedFontBundle<'a>,
    request: &FontResolutionRequestA0,
    limits: FontResolutionLimits,
) -> Result<&'a FontBundleEntry, ValidationError> {
    if request.schema != "kicad_monkey.font_resolution_request.a0"
        || request.type_ != "kicad_monkey.font_resolution_request"
        || request.version != "a0"
    {
        return Err(error(
            "unsupported_contract",
            "$",
            "unsupported font resolution contract identity",
        ));
    }
    validate_resolution_request(request, limits)?;
    if let Some(id) = request.selection.font_id.as_deref() {
        let index = bundle.id_index.get(id).ok_or_else(|| {
            error(
                "missing_font",
                "$.selection.font_id",
                "font ID is not present",
            )
        })?;
        return Ok(&bundle.manifest.fonts[*index]);
    }
    let mut matched_index = None;
    for requested in &request.selection.aliases {
        match bundle.alias_index.get(requested.as_str()) {
            Some(AliasTarget::Ambiguous) => {
                return Err(error(
                    "ambiguous_font",
                    "$.selection.aliases",
                    "font aliases resolve to more than one bundle entry",
                ));
            }
            Some(AliasTarget::Unique(index)) => match matched_index {
                Some(previous) if previous != *index => {
                    return Err(error(
                        "ambiguous_font",
                        "$.selection.aliases",
                        "font aliases resolve to more than one bundle entry",
                    ));
                }
                None => matched_index = Some(*index),
                _ => {}
            },
            None => {}
        }
    }
    matched_index
        .map(|index| &bundle.manifest.fonts[index])
        .ok_or_else(|| error("missing_font", "$.selection", "font selection has no match"))
}

fn validate_resolution_request(
    request: &FontResolutionRequestA0,
    limits: FontResolutionLimits,
) -> Result<(), ValidationError> {
    if request.selection.aliases.len() > limits.max_request_aliases {
        return Err(error(
            "resource_limit",
            "$.selection.aliases",
            "font request alias count exceeds its limit",
        ));
    }
    let mut strings = request.selection.aliases.iter().map(String::as_str);
    let request_bytes = strings
        .try_fold(
            request.selection.font_id.as_deref().map_or(0, str::len),
            |total, value| total.checked_add(value.len()),
        )
        .ok_or_else(|| {
            error(
                "resource_limit",
                "$.selection",
                "font request bytes overflow",
            )
        })?;
    if request_bytes > limits.max_request_string_bytes {
        return Err(error(
            "resource_limit",
            "$.selection",
            "font request string bytes exceed their limit",
        ));
    }
    if request.selection.font_id.as_deref() == Some("") {
        return Err(error(
            "invalid_selection",
            "$.selection.font_id",
            "explicit font ID must be nonempty",
        ));
    }
    let mut aliases = HashSet::with_capacity(request.selection.aliases.len());
    if request
        .selection
        .aliases
        .iter()
        .any(|alias| alias.is_empty() || !aliases.insert(alias))
    {
        return Err(error(
            "invalid_selection",
            "$.selection.aliases",
            "request aliases must be nonempty and unique",
        ));
    }
    Ok(())
}

fn build_alias_index(manifest: &FontBundleManifestA0) -> HashMap<&str, AliasTarget> {
    let mut index = HashMap::new();
    for (font_index, font) in manifest.fonts.iter().enumerate() {
        for alias in &font.aliases {
            index
                .entry(alias.as_str())
                .and_modify(|target| {
                    if *target != AliasTarget::Unique(font_index) {
                        *target = AliasTarget::Ambiguous;
                    }
                })
                .or_insert(AliasTarget::Unique(font_index));
        }
    }
    index
}

fn font_metadata_string_bytes(font: &FontBundleEntry) -> Result<usize, ValidationError> {
    let fixed = [
        Some(font.id.as_str()),
        Some(font.sha256.0.as_str()),
        font.family.as_deref(),
        font.style.as_deref(),
        font.postscript_name.as_deref(),
    ];
    fixed
        .into_iter()
        .flatten()
        .chain(font.aliases.iter().map(String::as_str))
        .chain(font.variations.iter().map(|value| value.axis.0.as_str()))
        .try_fold(0usize, |total, value| total.checked_add(value.len()))
        .ok_or_else(|| error("resource_limit", "$.fonts", "font metadata bytes overflow"))
}

fn validate_identity(manifest: &FontBundleManifestA0) -> Result<(), ValidationError> {
    if manifest.schema == "kicad_monkey.font_bundle.a0"
        && manifest.type_ == "kicad_monkey.font_bundle"
        && manifest.version == "a0"
    {
        Ok(())
    } else {
        Err(error(
            "unsupported_contract",
            "$",
            "unsupported font bundle contract identity",
        ))
    }
}

fn validate_font_metadata<'a>(
    font: &'a FontBundleEntry,
    index: usize,
    limits: FontBundleLimits,
    ids: &mut HashSet<&'a str>,
    slots: &mut HashSet<u32>,
) -> Result<(), ValidationError> {
    if font.id.is_empty() || !ids.insert(&font.id) {
        return Err(error(
            "duplicate_font_id",
            format!("$.fonts[{index}].id"),
            "font IDs must be nonempty and unique",
        ));
    }
    if !slots.insert(font.slot) {
        return Err(error(
            "duplicate_font_slot",
            format!("$.fonts[{index}].slot"),
            "font slots must be unique",
        ));
    }
    if !valid_sha256(&font.sha256.0) {
        return Err(error(
            "invalid_hash",
            format!("$.fonts[{index}].sha256"),
            "SHA-256 must be 64 lowercase hexadecimal characters",
        ));
    }
    if font.aliases.len() > limits.max_aliases_per_font
        || font.variations.len() > limits.max_variations_per_font
    {
        return Err(error(
            "resource_limit",
            format!("$.fonts[{index}]"),
            "font metadata exceeds its limit",
        ));
    }
    validate_aliases(font, index)?;
    validate_variations(font, index)
}

fn validate_aliases(font: &FontBundleEntry, index: usize) -> Result<(), ValidationError> {
    let mut aliases = HashSet::with_capacity(font.aliases.len());
    if font
        .aliases
        .iter()
        .any(|alias| alias.is_empty() || !aliases.insert(alias))
    {
        return Err(error(
            "invalid_alias",
            format!("$.fonts[{index}].aliases"),
            "font aliases must be nonempty and unique within an entry",
        ));
    }
    Ok(())
}

fn validate_variations(font: &FontBundleEntry, index: usize) -> Result<(), ValidationError> {
    let mut axes = HashSet::with_capacity(font.variations.len());
    for (variation_index, variation) in font.variations.iter().enumerate() {
        if !valid_tag(&variation.axis.0)
            || !variation.value.is_finite()
            || !axes.insert(&variation.axis.0)
        {
            return Err(error(
                "invalid_variation",
                format!("$.fonts[{index}].variations[{variation_index}]"),
                "variation axes must be finite, valid, and unique",
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

fn sha256_matches(buffer: &[u8], expected: &str) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    Sha256::digest(buffer)
        .iter()
        .zip(expected.as_bytes().chunks_exact(2))
        .all(|(byte, pair)| pair == [HEX[(byte >> 4) as usize], HEX[(byte & 0x0f) as usize]])
}

fn valid_tag(value: &str) -> bool {
    value.len() == 4 && value.bytes().all(|byte| (b' '..=b'~').contains(&byte))
}

fn error(code: &'static str, path: impl Into<String>, message: &'static str) -> ValidationError {
    validation_error(code, path, message)
}

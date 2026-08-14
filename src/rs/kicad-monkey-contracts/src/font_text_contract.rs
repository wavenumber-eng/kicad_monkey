use crate::generated::outline_vector::{
    CoordinateComparisonPolicy, OutlineCommand, OutlineVectorA0,
};
use crate::generated::shaping_record::{ShapedGlyph, ShapingInput, ShapingRecordA0};
use crate::{ValidationError, validation_error};
use std::collections::HashSet;

/// Enforce exact shaping identity and the shared UTF-8 byte-offset index contract.
pub fn validate_shaping_record_contract(record: &ShapingRecordA0) -> Result<(), ValidationError> {
    if record.schema != "kicad_monkey.shaping_record.a0"
        || record.type_ != "kicad_monkey.shaping_record"
        || record.version != "a0"
        || record.comparison.mode != "exact"
    {
        return Err(error(
            "unsupported_contract",
            "$",
            "unsupported shaping record identity or index policy",
        ));
    }
    validate_shaping_input_contract(&record.input).map_err(rebase_input_error)?;
    validate_glyph_clusters(&record.input.text, &record.glyphs)
}

fn rebase_input_error(mut error: ValidationError) -> ValidationError {
    error.path = if error.path == "$" {
        "$.input".to_owned()
    } else {
        format!("$.input{}", &error.path[1..])
    };
    error
}

/// Validate one shaping input before a shaper consumes caller-supplied font bytes.
pub fn validate_shaping_input_contract(input: &ShapingInput) -> Result<(), ValidationError> {
    if input.text_index_unit != "utf8_byte_offset" {
        return Err(error(
            "unsupported_contract",
            "$.text_index_unit",
            "shaping indices must use UTF-8 byte offsets",
        ));
    }
    validate_hash(&input.font_sha256.0, "$.font_sha256")?;
    validate_variations(
        input
            .variations
            .iter()
            .map(|variation| variation.axis.0.as_str()),
        "$.variations",
    )?;
    if input
        .script
        .as_ref()
        .is_some_and(|script| !valid_tag(&script.0))
    {
        return Err(error(
            "invalid_tag",
            "$.script",
            "script must be a printable four-byte OpenType tag",
        ));
    }
    if input.language.as_ref().is_some_and(String::is_empty) {
        return Err(error(
            "invalid_language",
            "$.language",
            "language must be nonempty when supplied",
        ));
    }
    validate_feature_indices(input)
}

/// Enforce exact outline metadata and coordinate-only comparison semantics.
pub fn validate_outline_vector_contract(record: &OutlineVectorA0) -> Result<(), ValidationError> {
    if record.schema != "kicad_monkey.outline_vector.a0"
        || record.type_ != "kicad_monkey.outline_vector"
        || record.version != "a0"
        || record.coordinate_format != "font_design_units_f64"
    {
        return Err(error(
            "unsupported_contract",
            "$",
            "unsupported outline vector identity or coordinate format",
        ));
    }
    validate_outline_comparison(&record.coordinate_comparison)?;
    validate_hash(&record.font_sha256.0, "$.font_sha256")?;
    validate_variations(
        record
            .variations
            .iter()
            .map(|variation| variation.axis.0.as_str()),
        "$.variations",
    )?;
    validate_outline_commands(&record.commands)
}

fn validate_outline_comparison(
    comparison: &CoordinateComparisonPolicy,
) -> Result<(), ValidationError> {
    match comparison {
        CoordinateComparisonPolicy::ExactComparisonPolicy(value) if value.mode == "exact" => {}
        CoordinateComparisonPolicy::AbsoluteToleranceComparisonPolicy(value)
            if value.mode == "absolute_tolerance" => {}
        _ => {
            return Err(error(
                "invalid_comparison",
                "$.coordinate_comparison",
                "outline comparison applies only through a registered coordinate mode",
            ));
        }
    }
    Ok(())
}

fn validate_outline_commands(commands: &[OutlineCommand]) -> Result<(), ValidationError> {
    for (index, command) in commands.iter().enumerate() {
        // `move_to` and `line_to` have the same JSON shape, so typify's
        // untagged enum decodes either into the first variant. The literal
        // remains authoritative and is validated here.
        let valid = match command {
            OutlineCommand::MoveTo(value) => matches!(value.kind.as_str(), "move_to" | "line_to"),
            OutlineCommand::LineTo(value) => value.kind == "line_to",
            OutlineCommand::QuadTo(value) => value.kind == "quad_to",
            OutlineCommand::CurveTo(value) => value.kind == "curve_to",
            OutlineCommand::Close(value) => value.kind == "close",
        };
        if !valid {
            return Err(error(
                "invalid_outline_command",
                format!("$.commands[{index}]"),
                "outline command kind does not match its payload",
            ));
        }
    }
    Ok(())
}

fn validate_feature_indices(input: &ShapingInput) -> Result<(), ValidationError> {
    let text_bytes = u32::try_from(input.text.len()).map_err(|_| {
        error(
            "resource_limit",
            "$.text",
            "UTF-8 text length exceeds the shaping index range",
        )
    })?;
    let mut char_starts = HashSet::with_capacity(input.text.chars().count());
    char_starts.extend(input.text.char_indices().map(|(index, _)| index as u32));
    for (index, feature) in input.features.iter().enumerate() {
        if !valid_tag(&feature.tag.0) {
            return Err(error(
                "invalid_tag",
                format!("$.features[{index}].tag"),
                "feature must use a printable four-byte OpenType tag",
            ));
        }
        let global = feature.start == 0 && feature.end == u32::MAX;
        let bounded = feature.start <= feature.end
            && valid_feature_endpoint(feature.start, text_bytes, &char_starts)
            && valid_feature_endpoint(feature.end, text_bytes, &char_starts);
        if !global && !bounded {
            return Err(error(
                "invalid_text_index",
                format!("$.features[{index}]"),
                "feature range must use half-open UTF-8 code-point boundaries",
            ));
        }
    }
    Ok(())
}

fn validate_glyph_clusters(text: &str, glyphs: &[ShapedGlyph]) -> Result<(), ValidationError> {
    let char_starts = text
        .char_indices()
        .map(|(index, _)| index as u32)
        .collect::<HashSet<_>>();
    for (index, glyph) in glyphs.iter().enumerate() {
        if !char_starts.contains(&glyph.cluster) {
            return Err(error(
                "invalid_text_index",
                format!("$.glyphs[{index}].cluster"),
                "glyph cluster must be a UTF-8 code-point boundary",
            ));
        }
    }
    Ok(())
}

fn valid_feature_endpoint(value: u32, text_bytes: u32, char_starts: &HashSet<u32>) -> bool {
    value == text_bytes || char_starts.contains(&value)
}

fn validate_variations<'a>(
    axes: impl Iterator<Item = &'a str>,
    path: &'static str,
) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    if axes
        .into_iter()
        .any(|axis| !valid_tag(axis) || !seen.insert(axis))
    {
        Err(error(
            "invalid_variation",
            path,
            "variation axes must be printable, four-byte, and unique",
        ))
    } else {
        Ok(())
    }
}

fn validate_hash(value: &str, path: &'static str) -> Result<(), ValidationError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(error(
            "invalid_hash",
            path,
            "SHA-256 must be 64 lowercase hexadecimal characters",
        ))
    }
}

fn valid_tag(value: &str) -> bool {
    value.len() == 4 && value.bytes().all(|byte| (b' '..=b'~').contains(&byte))
}

fn error(code: &'static str, path: impl Into<String>, message: &'static str) -> ValidationError {
    validation_error(code, path, message)
}

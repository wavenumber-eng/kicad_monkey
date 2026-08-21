//! Bounded extraction of KiCad's zstd-compressed schematic sidecars.

use crate::sexpr::{Lexer, Token, TokenKind, decode_quoted};
use crate::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::collections::BTreeSet;
use std::fmt;
use std::io::Read;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicEmbeddedLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_files: usize,
    pub max_encoded_bytes: usize,
    pub max_compressed_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_name_bytes: usize,
}

impl Default for SchematicEmbeddedLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 512 * 1024 * 1024,
            max_depth: 512,
            max_files: 256,
            max_encoded_bytes: 384 * 1024 * 1024,
            max_compressed_bytes: 288 * 1024 * 1024,
            max_decoded_bytes: 256 * 1024 * 1024,
            max_name_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicEmbeddedFile {
    pub name: String,
    pub file_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicEmbeddedError(String);

impl fmt::Display for SchematicEmbeddedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SchematicEmbeddedError {}

pub fn schematic_embedded_files(
    source: &str,
    limits: SchematicEmbeddedLimits,
) -> Result<Vec<SchematicEmbeddedFile>, SchematicEmbeddedError> {
    let spans = scan_form_spans_with_limits(
        source,
        &Selector {
            paths: Some(BTreeSet::from([vec![
                "kicad_sch".to_owned(),
                "embedded_files".to_owned(),
                "file".to_owned(),
            ]])),
            min_depth: Some(2),
            max_depth: Some(2),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: limits.max_files,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| embedded_error(error.to_string()))?;
    if spans.len() > limits.max_files {
        return Err(embedded_error("embedded file count exceeds its limit"));
    }
    let mut budget = EmbeddedBudget::default();
    spans
        .iter()
        .map(|span| decode_file(source, span, limits, &mut budget))
        .collect()
}

#[derive(Default)]
struct EmbeddedBudget {
    encoded: usize,
    compressed: usize,
    decoded: usize,
    names: usize,
}

fn decode_file(
    source: &str,
    span: &FormSpan,
    limits: SchematicEmbeddedLimits,
    budget: &mut EmbeddedBudget,
) -> Result<SchematicEmbeddedFile, SchematicEmbeddedError> {
    let form = span
        .text(source)
        .map_err(|error| embedded_error(error.to_string()))?;
    let mut lexer = Lexer::new(form);
    let mut depth = 0_usize;
    let mut expecting_head = false;
    let mut field = "";
    let mut name = String::new();
    let mut file_type = String::new();
    let mut encoded = String::new();
    while let Some(token) = lexer
        .next()
        .transpose()
        .map_err(|error| embedded_error(error.to_string()))?
    {
        match token.kind {
            TokenKind::Left => {
                depth = depth.saturating_add(1);
                expecting_head = true;
            }
            TokenKind::Right => {
                if depth == 2 {
                    field = "";
                }
                depth = depth.saturating_sub(1);
                expecting_head = false;
            }
            _ if expecting_head => {
                if depth == 2 {
                    field = token.lexeme;
                }
                expecting_head = false;
            }
            _ if depth == 2 && field == "name" && name.is_empty() => {
                name = token_text(token);
            }
            _ if depth == 2 && field == "type" && file_type.is_empty() => {
                file_type = token_text(token);
            }
            _ if depth == 2 && field == "data" => encoded.push_str(token.lexeme.trim()),
            _ => {}
        }
    }
    budget.names = checked_add(budget.names, name.len().saturating_add(file_type.len()))?;
    if budget.names > limits.max_name_bytes {
        return Err(embedded_error(
            "embedded file names exceed their byte limit",
        ));
    }
    let encoded = encoded.trim_matches('|');
    budget.encoded = checked_add(budget.encoded, encoded.len())?;
    if budget.encoded > limits.max_encoded_bytes {
        return Err(embedded_error("embedded encoded bytes exceed their limit"));
    }
    let compressed = decode_base64(encoded, limits.max_compressed_bytes - budget.compressed)?;
    budget.compressed = checked_add(budget.compressed, compressed.len())?;
    let remaining = limits.max_decoded_bytes.saturating_sub(budget.decoded);
    let mut decoder = zstd::stream::read::Decoder::new(compressed.as_slice())
        .map_err(|error| embedded_error(format!("embedded zstd payload is invalid: {error}")))?;
    let mut bytes = Vec::new();
    decoder
        .by_ref()
        .take(remaining.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| embedded_error(format!("could not decompress embedded file: {error}")))?;
    if bytes.len() > remaining {
        return Err(embedded_error("embedded decoded bytes exceed their limit"));
    }
    budget.decoded = checked_add(budget.decoded, bytes.len())?;
    Ok(SchematicEmbeddedFile {
        name,
        file_type,
        bytes,
    })
}

fn token_text(token: Token<'_>) -> String {
    if token.kind == TokenKind::QuotedString {
        decode_quoted(token.lexeme)
    } else {
        token.lexeme.to_owned()
    }
}

fn decode_base64(value: &str, maximum: usize) -> Result<Vec<u8>, SchematicEmbeddedError> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(embedded_error("embedded base64 length is invalid"));
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    let decoded_len = (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(|| embedded_error("embedded base64 size overflowed"))?;
    if padding > 2 || decoded_len > maximum {
        return Err(embedded_error(
            "embedded compressed bytes exceed their limit",
        ));
    }
    let mut output = Vec::with_capacity(decoded_len);
    for (block_index, encoded) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let mut block = [0_u8; 4];
        for (index, byte) in encoded.iter().copied().enumerate() {
            block[index] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 64,
                _ => return Err(embedded_error("embedded base64 data is invalid")),
            };
        }
        let last = block_index + 1 == bytes.len() / 4;
        if block[0] == 64
            || block[1] == 64
            || (!last && block[3] == 64)
            || (block[2] == 64 && block[3] != 64)
            || (block[2] == 64 && block[1] & 0x0f != 0)
            || (block[3] == 64 && block[2] != 64 && block[2] & 0x03 != 0)
        {
            return Err(embedded_error("embedded base64 padding is invalid"));
        }
        output.push((block[0] << 2) | (block[1] >> 4));
        if block[2] != 64 {
            output.push((block[1] << 4) | (block[2] >> 2));
        }
        if block[3] != 64 {
            output.push((block[2] << 6) | block[3]);
        }
    }
    Ok(output)
}

fn checked_add(left: usize, right: usize) -> Result<usize, SchematicEmbeddedError> {
    left.checked_add(right)
        .ok_or_else(|| embedded_error("embedded file budget overflowed"))
}

fn embedded_error(message: impl Into<String>) -> SchematicEmbeddedError {
    SchematicEmbeddedError(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_file_limits_accept_exact_and_reject_one_under() {
        let payload = b"(kicad_wks (version 20210606))";
        let compressed = zstd::stream::encode_all(payload.as_slice(), 0).unwrap();
        let encoded = base64(&compressed);
        let source = format!(
            "(kicad_sch (embedded_files (file (name \"review.kicad_wks\") (type worksheet) (data |{encoded}|))))"
        );
        let exact = SchematicEmbeddedLimits {
            max_source_bytes: source.len(),
            max_depth: 3,
            max_files: 1,
            max_encoded_bytes: encoded.len(),
            max_compressed_bytes: compressed.len(),
            max_decoded_bytes: payload.len(),
            max_name_bytes: "review.kicad_wks".len() + "worksheet".len(),
        };
        let files = schematic_embedded_files(&source, exact).expect("exact limits");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].bytes, payload);

        for limits in [
            SchematicEmbeddedLimits {
                max_files: 0,
                ..exact
            },
            SchematicEmbeddedLimits {
                max_encoded_bytes: encoded.len() - 1,
                ..exact
            },
            SchematicEmbeddedLimits {
                max_compressed_bytes: compressed.len() - 1,
                ..exact
            },
            SchematicEmbeddedLimits {
                max_decoded_bytes: payload.len() - 1,
                ..exact
            },
            SchematicEmbeddedLimits {
                max_name_bytes: exact.max_name_bytes - 1,
                ..exact
            },
        ] {
            assert!(schematic_embedded_files(&source, limits).is_err());
        }
    }

    #[test]
    fn embedded_base64_rejects_noncanonical_padding() {
        for encoded in ["AA=A", "AB==", "AAB="] {
            assert!(decode_base64(encoded, usize::MAX).is_err());
        }
    }

    fn base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let first = chunk[0];
            let second = chunk.get(1).copied().unwrap_or(0);
            let third = chunk.get(2).copied().unwrap_or(0);
            encoded.push(char::from(ALPHABET[(first >> 2) as usize]));
            encoded.push(char::from(
                ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize],
            ));
            encoded.push(if chunk.len() > 1 {
                char::from(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize])
            } else {
                '='
            });
            encoded.push(if chunk.len() > 2 {
                char::from(ALPHABET[(third & 0x3f) as usize])
            } else {
                '='
            });
        }
        encoded
    }
}

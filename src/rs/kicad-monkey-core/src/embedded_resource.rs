//! Shared, bounded access to KiCad embedded-resource payloads.

use crate::sexpr::{Error, ErrorKind, ErrorPhase, Lexer, Token, TokenKind, decode_quoted};
use crate::sexpr_projection::FormSpan;
#[cfg(feature = "embedded-resource-zstd")]
use sha2::{Digest, Sha256};
use std::borrow::Cow;
#[cfg(feature = "embedded-resource-zstd")]
use std::fmt::Write as _;
#[cfg(feature = "embedded-resource-zstd")]
use std::io::Read;
use std::ops::Range;

/// Source owner for an embedded resource declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedFileOwner {
    Board,
    EmbeddedFootprint { footprint_index: usize },
    StandaloneFootprint,
}

/// Authored state of an embedded resource's `(data ...)` form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedDataPresence {
    /// No `(data ...)` form was authored; another scope may own the payload.
    Absent,
    /// A `(data ...)` form was authored but contains no encoded bytes.
    Empty,
    /// A nonempty encoded payload is present and may be decoded on demand.
    Encoded,
}

/// Metadata for one source-backed embedded resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedFile {
    pub owner: EmbeddedFileOwner,
    pub name: String,
    pub file_type: String,
    pub checksum: Option<String>,
    pub data_presence: EmbeddedDataPresence,
    pub encoded_data_bytes: usize,
    pub source_range: Range<usize>,
}

/// Per-resource ceilings for explicit payload access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedDecodeLimits {
    pub max_encoded_bytes: usize,
    pub max_compressed_bytes: usize,
    pub max_decoded_bytes: usize,
}

impl Default for EmbeddedDecodeLimits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 384 * 1024 * 1024,
            max_compressed_bytes: 288 * 1024 * 1024,
            max_decoded_bytes: 256 * 1024 * 1024,
        }
    }
}

pub(crate) fn metadata_from_span(
    source: &str,
    span: &FormSpan,
    owner: EmbeddedFileOwner,
) -> Result<EmbeddedFile, Error> {
    let fields = parse_fields(span.text(source)?, None)?;
    Ok(EmbeddedFile {
        owner,
        name: fields.name,
        file_type: if fields.file_type.is_empty() {
            "other".to_owned()
        } else {
            fields.file_type
        },
        checksum: fields.checksum.filter(|value| !value.is_empty()),
        data_presence: match (fields.has_data, fields.encoded_bytes) {
            (false, _) => EmbeddedDataPresence::Absent,
            (true, 0) => EmbeddedDataPresence::Empty,
            (true, _) => EmbeddedDataPresence::Encoded,
        },
        encoded_data_bytes: fields.encoded_bytes,
        source_range: span.range.clone(),
    })
}

pub(crate) fn encoded_data(
    source: &str,
    file: &EmbeddedFile,
    maximum: usize,
) -> Result<Option<String>, Error> {
    let form = source.get(file.source_range.clone()).ok_or_else(|| {
        resource_error(
            file,
            ErrorKind::InvalidSpan,
            "source range does not belong to this source",
        )
    })?;
    let fields = parse_fields(form, Some(maximum)).map_err(|error| label_error(error, file))?;
    if fields.name != file.name
        || fields.file_type_or_default() != file.file_type
        || fields.checksum.as_deref().filter(|value| !value.is_empty()) != file.checksum.as_deref()
        || fields.data_presence() != file.data_presence
        || fields.encoded_bytes != file.encoded_data_bytes
    {
        return Err(resource_error(
            file,
            ErrorKind::InvalidSpan,
            "metadata does not match the supplied source",
        ));
    }
    Ok(fields.has_data.then_some(fields.encoded))
}

#[cfg(feature = "embedded-resource-zstd")]
pub(crate) fn decoded_data(
    source: &str,
    file: &EmbeddedFile,
    limits: EmbeddedDecodeLimits,
) -> Result<Option<Vec<u8>>, Error> {
    let Some(encoded) = encoded_data(source, file, limits.max_encoded_bytes)? else {
        return Ok(None);
    };
    let bytes = if encoded.is_empty() {
        Vec::new()
    } else {
        let compressed = decode_base64(&encoded, limits.max_compressed_bytes).map_err(|error| {
            resource_error(
                file,
                match error {
                    Base64DecodeError::Invalid(_) => ErrorKind::UnexpectedToken,
                    Base64DecodeError::ResourceLimit => ErrorKind::ResourceLimit,
                },
                error,
            )
        })?;
        let mut decoder =
            zstd::stream::read::Decoder::new(compressed.as_slice()).map_err(|error| {
                resource_error(
                    file,
                    ErrorKind::UnexpectedToken,
                    format!("zstd payload is invalid: {error}"),
                )
            })?;
        let mut bytes = Vec::new();
        decoder
            .by_ref()
            .take(limits.max_decoded_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                resource_error(
                    file,
                    ErrorKind::UnexpectedToken,
                    format!("could not decompress payload: {error}"),
                )
            })?;
        bytes
    };
    if bytes.len() > limits.max_decoded_bytes {
        return Err(resource_error(
            file,
            ErrorKind::ResourceLimit,
            "decoded bytes exceed max_decoded_bytes",
        ));
    }
    // KiCad 9.0.0 could author an explicitly empty `(data)` form. Its loader
    // deliberately skips decoding and checksum validation for that case.
    if !encoded.is_empty() {
        verify_checksum(file, &bytes)?;
    }
    Ok(Some(bytes))
}

#[cfg(feature = "embedded-resource-zstd")]
fn verify_checksum(file: &EmbeddedFile, bytes: &[u8]) -> Result<(), Error> {
    let Some(expected) = &file.checksum else {
        return Err(resource_error(
            file,
            ErrorKind::UnexpectedToken,
            "nonempty embedded payload is missing its checksum",
        ));
    };
    let (kind, actual, legacy_actual) = if expected.len() == 64 {
        let digest = Sha256::digest(bytes);
        let mut actual = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut actual, "{byte:02x}").expect("writing SHA-256 hex to a String cannot fail");
        }
        ("SHA-256", actual, None)
    } else {
        let actual = mmh3_128(bytes, false);
        let legacy = mmh3_128(bytes, true);
        ("MMH3-128", actual, Some(legacy))
    };
    if expected.eq_ignore_ascii_case(&actual)
        || legacy_actual
            .as_deref()
            .is_some_and(|legacy| expected.eq_ignore_ascii_case(legacy))
    {
        return Ok(());
    }
    Err(resource_error(
        file,
        ErrorKind::UnexpectedToken,
        format!("{kind} checksum mismatch (expected {expected}, actual {actual})"),
    ))
}

/// KiCad's seeded MurmurHash3 x64 128-bit checksum. `legacy_tail` reproduces
/// the pre-fix tail padding accepted by newer KiCad loaders for old files.
#[cfg(feature = "embedded-resource-zstd")]
pub(crate) fn mmh3_128(bytes: &[u8], legacy_tail: bool) -> String {
    const SEED: u64 = 0xABBA_2345;
    const C1: u64 = 0x87c3_7b91_1142_53d5;
    const C2: u64 = 0x4cf5_ad43_2745_937f;

    let mut h1 = SEED;
    let mut h2 = SEED;
    let (chunks, remainder) = bytes.as_chunks::<16>();
    for chunk in chunks {
        let k1 = u64::from_le_bytes(chunk[..8].try_into().expect("fixed chunk"));
        let k2 = u64::from_le_bytes(chunk[8..].try_into().expect("fixed chunk"));
        mix_mmh3_block(&mut h1, &mut h2, k1, k2, C1, C2);
    }

    let mut tail = [0_u8; 16];
    tail[..remainder.len()].copy_from_slice(remainder);
    let (tail_length, total_length) = mmh3_tail_lengths(bytes.len(), remainder.len(), legacy_tail);

    let mut k1 = 0_u64;
    let mut k2 = 0_u64;
    for (index, byte) in tail[..tail_length.min(8)].iter().copied().enumerate() {
        k1 ^= u64::from(byte) << (index * 8);
    }
    if tail_length > 8 {
        for (index, byte) in tail[8..tail_length].iter().copied().enumerate() {
            k2 ^= u64::from(byte) << (index * 8);
        }
        k2 = k2.wrapping_mul(C2).rotate_left(33).wrapping_mul(C1);
        h2 ^= k2;
    }
    if tail_length > 0 {
        k1 = k1.wrapping_mul(C1).rotate_left(31).wrapping_mul(C2);
        h1 ^= k1;
    }

    let length = total_length as u64;
    h1 ^= length;
    h2 ^= length;
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    h1 = fmix64(h1);
    h2 = fmix64(h2);
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    format!("{h1:016X}{h2:016X}")
}

#[cfg(feature = "embedded-resource-zstd")]
fn mmh3_tail_lengths(length: usize, remainder: usize, legacy: bool) -> (usize, usize) {
    if legacy && remainder != 0 {
        let padded = remainder + 4 - (remainder + 4) % 4;
        (padded & 15, length - remainder + padded)
    } else {
        (remainder, length)
    }
}

#[cfg(feature = "embedded-resource-zstd")]
fn mix_mmh3_block(h1: &mut u64, h2: &mut u64, mut k1: u64, mut k2: u64, c1: u64, c2: u64) {
    k1 = k1.wrapping_mul(c1).rotate_left(31).wrapping_mul(c2);
    *h1 ^= k1;
    *h1 = (*h1)
        .rotate_left(27)
        .wrapping_add(*h2)
        .wrapping_mul(5)
        .wrapping_add(0x52dc_e729);
    k2 = k2.wrapping_mul(c2).rotate_left(33).wrapping_mul(c1);
    *h2 ^= k2;
    *h2 = (*h2)
        .rotate_left(31)
        .wrapping_add(*h1)
        .wrapping_mul(5)
        .wrapping_add(0x3849_5ab5);
}

#[cfg(feature = "embedded-resource-zstd")]
fn fmix64(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

#[derive(Default)]
struct ParsedFields {
    name: String,
    file_type: String,
    checksum: Option<String>,
    has_data: bool,
    encoded_bytes: usize,
    encoded: String,
}

impl ParsedFields {
    fn field_head<'a>(
        &mut self,
        token: &Token<'a>,
        depth: usize,
        field: &mut &'a str,
    ) -> Result<(), Error> {
        if depth == 1 && token.lexeme != "file" {
            return Err(source_error("Expected embedded file form"));
        }
        if depth == 2 {
            *field = token.lexeme;
            if *field == "data" {
                self.has_data = true;
            }
        }
        Ok(())
    }

    fn scalar(
        &mut self,
        field: &str,
        token: Token<'_>,
        encoded_limit: Option<usize>,
    ) -> Result<(), Error> {
        match field {
            "name" if self.name.is_empty() => self.name = token_text(token),
            "type" if self.file_type.is_empty() => self.file_type = token_text(token),
            "checksum" if self.checksum.is_none() => self.checksum = Some(token_text(token)),
            "data" => self.data_token(&token, encoded_limit)?,
            _ => {}
        }
        Ok(())
    }

    fn data_token(&mut self, token: &Token<'_>, encoded_limit: Option<usize>) -> Result<(), Error> {
        self.encoded_bytes = self
            .encoded_bytes
            .checked_add(encoded_token_len(token))
            .ok_or_else(resource_limit_error)?;
        if encoded_limit.is_some_and(|maximum| self.encoded_bytes > maximum) {
            return Err(resource_limit_error());
        }
        if encoded_limit.is_some() {
            self.encoded.push_str(&encoded_token_text(token));
        }
        Ok(())
    }

    fn file_type_or_default(&self) -> &str {
        if self.file_type.is_empty() {
            "other"
        } else {
            &self.file_type
        }
    }

    fn data_presence(&self) -> EmbeddedDataPresence {
        match (self.has_data, self.encoded_bytes) {
            (false, _) => EmbeddedDataPresence::Absent,
            (true, 0) => EmbeddedDataPresence::Empty,
            (true, _) => EmbeddedDataPresence::Encoded,
        }
    }
}

fn parse_fields(form: &str, encoded_limit: Option<usize>) -> Result<ParsedFields, Error> {
    let mut lexer = Lexer::new(form);
    let mut result = ParsedFields::default();
    let mut depth = 0usize;
    let mut expecting_head = false;
    let mut field = "";
    while let Some(token) = lexer.next().transpose()? {
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
                result.field_head(&token, depth, &mut field)?;
                expecting_head = false;
            }
            _ if depth == 2 => result.scalar(field, token, encoded_limit)?,
            _ => {}
        }
    }
    Ok(result)
}

fn token_text(token: Token<'_>) -> String {
    if token.kind == TokenKind::QuotedString {
        decode_quoted(token.lexeme)
    } else {
        token.lexeme.to_owned()
    }
}

fn encoded_token_text(token: &Token<'_>) -> String {
    if token.kind == TokenKind::QuotedString {
        decode_quoted(token.lexeme)
    } else {
        token.lexeme.trim_matches('|').to_owned()
    }
}

fn encoded_token_len(token: &Token<'_>) -> usize {
    if token.kind != TokenKind::QuotedString {
        return token.lexeme.trim_matches('|').len();
    }
    let body = &token.lexeme[1..token.lexeme.len() - 1];
    let mut length = 0usize;
    let mut characters = body.chars().peekable();
    while let Some(character) = next_normalized_character(&mut characters) {
        if character != '\\' {
            length = length.saturating_add(character.len_utf8());
            continue;
        }
        let Some(escaped) = next_normalized_character(&mut characters) else {
            return length.saturating_add(1);
        };
        length = length.saturating_add(encoded_escape_len(escaped, &mut characters));
    }
    length
}

fn next_normalized_character(
    characters: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<char> {
    let character = characters.next()?;
    if character != '\r' {
        return Some(character);
    }
    if characters.peek() == Some(&'\n') {
        characters.next();
    }
    Some('\n')
}

fn encoded_escape_len(
    escaped: char,
    characters: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> usize {
    match escaped {
        '"' | '\\' | 'a' | 'b' | 'f' | 'n' | 'r' | 't' | 'v' => 1,
        'x' => encoded_hex_len(characters),
        digit @ '0'..='7' => encoded_octal_len(digit, characters),
        other => 1 + other.len_utf8(),
    }
}

fn encoded_hex_len(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> usize {
    let mut value = 0_u8;
    let mut digits = 0_u8;
    for _ in 0..2 {
        let Some(digit) = characters
            .peek()
            .copied()
            .and_then(|item| item.to_digit(16))
        else {
            break;
        };
        characters.next();
        value = value.saturating_mul(16).saturating_add(digit as u8);
        digits += 1;
    }
    if digits == 0 {
        1
    } else {
        char::from(value).len_utf8()
    }
}

fn encoded_octal_len(
    digit: char,
    characters: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> usize {
    let mut value = u16::from(digit as u8 - b'0');
    for _ in 1..3 {
        let Some(next @ '0'..='7') = characters.peek().copied() else {
            break;
        };
        characters.next();
        value = value * 8 + u16::from(next as u8 - b'0');
    }
    u8::try_from(value).map_or(0, |value| char::from(value).len_utf8())
}

#[cfg(feature = "embedded-resource-zstd")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Base64DecodeError {
    Invalid(&'static str),
    ResourceLimit,
}

#[cfg(feature = "embedded-resource-zstd")]
impl std::fmt::Display for Base64DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::ResourceLimit => {
                formatter.write_str("compressed bytes exceed max_compressed_bytes")
            }
        }
    }
}

#[cfg(feature = "embedded-resource-zstd")]
fn decode_base64(value: &str, maximum: usize) -> Result<Vec<u8>, Base64DecodeError> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(Base64DecodeError::Invalid("base64 length is invalid"));
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    let decoded_len = (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|length| length.checked_sub(padding))
        .ok_or(Base64DecodeError::Invalid("base64 size overflowed"))?;
    if padding > 2 {
        return Err(Base64DecodeError::Invalid("base64 padding is invalid"));
    }
    if decoded_len > maximum {
        return Err(Base64DecodeError::ResourceLimit);
    }
    let mut output = Vec::with_capacity(decoded_len);
    for (block_index, encoded) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let block = decode_base64_block(encoded)?;
        let last = block_index + 1 == bytes.len() / 4;
        if !base64_padding_valid(block, last) || !base64_tail_bits_valid(block) {
            return Err(Base64DecodeError::Invalid("base64 padding is invalid"));
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

#[cfg(feature = "embedded-resource-zstd")]
fn decode_base64_block(encoded: &[u8; 4]) -> Result<[u8; 4], Base64DecodeError> {
    let mut block = [0_u8; 4];
    for (index, byte) in encoded.iter().copied().enumerate() {
        block[index] = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return Err(Base64DecodeError::Invalid("base64 data is invalid")),
        };
    }
    Ok(block)
}

#[cfg(feature = "embedded-resource-zstd")]
fn base64_padding_valid(block: [u8; 4], last: bool) -> bool {
    !(block[0] == 64
        || block[1] == 64
        || (!last && block[3] == 64)
        || (block[2] == 64 && block[3] != 64))
}

#[cfg(feature = "embedded-resource-zstd")]
fn base64_tail_bits_valid(block: [u8; 4]) -> bool {
    !((block[2] == 64 && block[1] & 0x0f != 0)
        || (block[3] == 64 && block[2] != 64 && block[2] & 0x03 != 0))
}

fn source_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        crate::Position {
            offset: 0,
            line: 1,
            column: 1,
        },
    )
}

fn resource_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Embedded resource encoded bytes exceed their limit",
        crate::Position {
            offset: 0,
            line: 1,
            column: 1,
        },
    )
}

fn label_error(mut error: Error, file: &EmbeddedFile) -> Error {
    error.message = Cow::Owned(format!(
        "Embedded resource {:?}: {}",
        file.name, error.message
    ));
    error.token = Some(file.name.clone());
    error
}

fn resource_error(file: &EmbeddedFile, kind: ErrorKind, message: impl std::fmt::Display) -> Error {
    Error {
        phase: ErrorPhase::Tree,
        kind,
        message: Cow::Owned(format!("Embedded resource {:?}: {message}", file.name)),
        position: None,
        token: Some(file.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "embedded-resource-zstd")]
    use super::mmh3_128;
    use super::parse_fields;

    #[test]
    fn metadata_encoded_length_matches_materialized_escaped_tokens() {
        let source = r#"(file (data "A\x42\101\\C\777" |REVG|))"#;
        let metadata = parse_fields(source, None).expect("metadata fields");
        let materialized = parse_fields(source, Some(usize::MAX)).expect("encoded fields");
        assert_eq!(metadata.encoded_bytes, materialized.encoded.len());
        assert_eq!(materialized.encoded, "ABA\\CREVG");
    }

    #[cfg(feature = "embedded-resource-zstd")]
    #[test]
    fn kicad_mmh3_matches_independent_reference_vectors() {
        assert_eq!(
            mmh3_128(b"board-owned bytes", false),
            "41B3C1140F84B2B746E8E9CACA2E64FD"
        );
        assert_eq!(
            mmh3_128(b"legacy-tail-bytes", true),
            "E6D9A62EE8B1C316DAA13A46B903B89F"
        );
    }
}

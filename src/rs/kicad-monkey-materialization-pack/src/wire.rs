//! Little-endian column writing and reading, and the shared string table.

use crate::PackError;
use std::collections::HashMap;

pub(crate) fn corrupt(what: &'static str) -> PackError {
    PackError::Corrupt(what)
}

pub(crate) fn put_u32(out: &mut Vec<u8>, values: &[u32]) {
    values
        .iter()
        .for_each(|value| out.extend(value.to_le_bytes()));
}

pub(crate) fn put_i32(out: &mut Vec<u8>, values: &[i32]) {
    values
        .iter()
        .for_each(|value| out.extend(value.to_le_bytes()));
}

pub(crate) fn put_f64(out: &mut Vec<u8>, values: &[f64]) {
    values
        .iter()
        .for_each(|value| out.extend(value.to_le_bytes()));
}

pub(crate) fn pad8(out: &mut Vec<u8>) {
    out.resize(out.len().div_ceil(8) * 8, 0);
}

pub(crate) fn count_u32(count: usize) -> Result<u32, PackError> {
    u32::try_from(count).map_err(|_| corrupt("count outside uint32"))
}

pub(crate) fn index_i32(index: usize) -> Result<i32, PackError> {
    i32::try_from(index).map_err(|_| corrupt("index outside int32"))
}

/// Reads columns from one section, failing closed past its end.
pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'a [u8], PackError> {
        let end = self
            .at
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(PackError::Truncated)?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn words<const N: usize>(&mut self, count: usize) -> Result<Vec<[u8; N]>, PackError> {
        let length = count.checked_mul(N).ok_or(PackError::Truncated)?;
        Ok(self.take(length)?.as_chunks::<N>().0.to_vec())
    }

    pub(crate) fn u32s(&mut self, count: usize) -> Result<Vec<u32>, PackError> {
        Ok(self
            .words::<4>(count)?
            .into_iter()
            .map(u32::from_le_bytes)
            .collect())
    }

    pub(crate) fn i32s(&mut self, count: usize) -> Result<Vec<i32>, PackError> {
        Ok(self
            .words::<4>(count)?
            .into_iter()
            .map(i32::from_le_bytes)
            .collect())
    }

    pub(crate) fn f64s(&mut self, count: usize) -> Result<Vec<f64>, PackError> {
        Ok(self
            .words::<8>(count)?
            .into_iter()
            .map(f64::from_le_bytes)
            .collect())
    }

    pub(crate) fn u64(&mut self) -> Result<usize, PackError> {
        let word = self.words::<8>(1)?[0];
        usize::try_from(u64::from_le_bytes(word)).map_err(|_| PackError::Truncated)
    }

    pub(crate) fn usize32(&mut self) -> Result<usize, PackError> {
        usize::try_from(self.u32s(1)?[0]).map_err(|_| PackError::Truncated)
    }
}

/// Every string a pack refers to, stored once, in order of first use.
#[derive(Default)]
pub(crate) struct StringTable<'a> {
    pub(crate) values: Vec<&'a str>,
    index: HashMap<&'a str, i32>,
}

impl<'a> StringTable<'a> {
    pub(crate) fn intern(&mut self, value: &'a str) -> Result<i32, PackError> {
        if let Some(index) = self.index.get(value) {
            return Ok(*index);
        }
        let index = index_i32(self.values.len())?;
        self.values.push(value);
        self.index.insert(value, index);
        Ok(index)
    }

    pub(crate) fn optional(&mut self, value: Option<&'a str>) -> Result<i32, PackError> {
        value.map_or(Ok(-1), |value| self.intern(value))
    }

    pub(crate) fn get(&self, value: &str) -> Option<i32> {
        self.index.get(value).copied()
    }

    /// `u32` count, then each string as a `u32` byte length and UTF-8
    /// bytes, or, for a lowercase canonical UUID, [`UUID_LENGTH`] and its 16
    /// bytes.
    pub(crate) fn encode(&self) -> Result<Vec<u8>, PackError> {
        let mut out = Vec::new();
        put_u32(&mut out, &[count_u32(self.values.len())?]);
        for value in &self.values {
            if let Some(bytes) = uuid_bytes(value) {
                put_u32(&mut out, &[UUID_LENGTH]);
                out.extend(bytes);
            } else {
                let length = count_u32(value.len())?;
                if length & UUID_FLAG != 0 {
                    return Err(corrupt("string too long"));
                }
                put_u32(&mut out, &[length]);
                out.extend(value.as_bytes());
            }
        }
        Ok(out)
    }
}

/// Decode a string table section.
pub(crate) fn decode_strings(section: &[u8]) -> Result<Vec<String>, PackError> {
    let mut cursor = Cursor::new(section);
    let count = cursor.usize32()?;
    if count.saturating_mul(4) > cursor.remaining() {
        return Err(corrupt("string count exceeds its section"));
    }
    let mut strings = Vec::with_capacity(count);
    for _ in 0..count {
        let length = cursor.u32s(1)?[0];
        if length == UUID_LENGTH {
            strings.push(uuid_text(cursor.take(16)?));
            continue;
        }
        if length & UUID_FLAG != 0 {
            return Err(corrupt("bad string length"));
        }
        let text = cursor.take(usize::try_from(length).map_err(|_| PackError::Truncated)?)?;
        strings.push(String::from_utf8(text.to_vec()).map_err(|_| corrupt("string is not UTF-8"))?);
    }
    Ok(strings)
}

/// High bit of a string length: the string is a UUID in 16 bytes.
const UUID_FLAG: u32 = 1 << 31;
/// The length word of a UUID string.
const UUID_LENGTH: u32 = UUID_FLAG | 16;
/// Hyphen positions in a canonical UUID.
const UUID_HYPHENS: [usize; 4] = [8, 13, 18, 23];

/// The 16 bytes of a lowercase canonical UUID (`8-4-4-4-12` hex digits),
/// `None` for any other string, which then stays text.
fn uuid_bytes(value: &str) -> Option<[u8; 16]> {
    let text = value.as_bytes();
    if text.len() != 36 {
        return None;
    }
    let mut digits = Vec::with_capacity(32);
    for (position, byte) in text.iter().enumerate() {
        if UUID_HYPHENS.contains(&position) {
            if *byte != b'-' {
                return None;
            }
            continue;
        }
        digits.push(match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => return None,
        });
    }
    let mut bytes = [0u8; 16];
    for (byte, pair) in bytes.iter_mut().zip(digits.as_chunks::<2>().0) {
        *byte = (pair[0] << 4) | pair[1];
    }
    Some(bytes)
}

fn uuid_text(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// The string at `index`, `None` for `-1`.
pub(crate) fn optional_string(
    strings: &[String],
    index: i32,
    what: &'static str,
) -> Result<Option<String>, PackError> {
    if index == -1 {
        return Ok(None);
    }
    usize::try_from(index)
        .ok()
        .and_then(|index| strings.get(index))
        .cloned()
        .map(Some)
        .ok_or(corrupt(what))
}

/// The string at `index`, which must exist.
pub(crate) fn string(
    strings: &[String],
    index: i32,
    what: &'static str,
) -> Result<String, PackError> {
    optional_string(strings, index, what)?.ok_or(corrupt(what))
}

/// Distinct affine linear parts (`m11 m12 m21 m22`), by exact bits, in
/// order of first use. Boards use a handful, so operations store an index.
#[derive(Default)]
pub(crate) struct LinearTable {
    pub(crate) values: Vec<[f64; 4]>,
    index: HashMap<[u64; 4], i32>,
}

impl LinearTable {
    pub(crate) fn intern(&mut self, linear: [f64; 4]) -> Result<i32, PackError> {
        let key = linear.map(f64::to_bits);
        if let Some(index) = self.index.get(&key) {
            return Ok(*index);
        }
        let index = index_i32(self.values.len())?;
        self.values.push(linear);
        self.index.insert(key, index);
        Ok(index)
    }

    pub(crate) fn get(&self, linear: [f64; 4]) -> Option<i32> {
        self.index.get(&linear.map(f64::to_bits)).copied()
    }

    /// `u32` count, `u32` zero, then four `f64` per entry.
    pub(crate) fn encode(&self) -> Result<Vec<u8>, PackError> {
        let mut out = Vec::new();
        put_u32(&mut out, &[count_u32(self.values.len())?, 0]);
        for linear in &self.values {
            put_f64(&mut out, linear);
        }
        Ok(out)
    }
}

/// Decode a linear-part table section.
pub(crate) fn decode_linear(section: &[u8]) -> Result<Vec<[f64; 4]>, PackError> {
    let mut cursor = Cursor::new(section);
    let count = cursor.usize32()?;
    cursor.usize32()?;
    if count.saturating_mul(32) > cursor.remaining() {
        return Err(PackError::Truncated);
    }
    let values = cursor.f64s(4 * count)?;
    Ok(values.as_chunks::<4>().0.to_vec())
}

#[cfg(test)]
mod tests {
    use super::{StringTable, decode_strings, uuid_bytes};

    #[test]
    fn strings_round_trip_with_canonical_uuids_compacted() {
        let values = [
            "8f75b3a2-843c-46f3-8ba1-ced896a61854",
            "8F75B3A2-843C-46F3-8BA1-CED896A61854",
            "8f75b3a2843c46f38ba1ced896a61854",
            "8f75b3a2-843c-46f3-8ba1-ced896a6185g",
            "",
            "U1",
        ];
        assert!(uuid_bytes(values[0]).is_some());
        assert!(values[1..].iter().all(|value| uuid_bytes(value).is_none()));
        let mut table = StringTable::default();
        for value in values {
            table.intern(value).expect("intern");
        }
        let bytes = table.encode().expect("encode");
        assert_eq!(decode_strings(&bytes).expect("decode"), values);
        // The UUID costs a length word and 16 bytes, not 36.
        assert_eq!(&bytes[4..8], &(super::UUID_LENGTH).to_le_bytes());
    }
}

//! Strict bounded bitmap metadata decoding shared by schematic carriers.

use super::*;

const DEFAULT_DPI: f64 = 300.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImageFormat {
    Png,
    Jpeg,
    Bmp,
}

impl ImageFormat {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Bmp => "bmp",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ImageMetadata {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub ppi_x: Option<u32>,
    pub ppi_y: Option<u32>,
    pub work: usize,
}

pub(super) fn decode_base64(value: &str, maximum: usize) -> Result<Vec<u8>, Error> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(model_error("Invalid schematic image base64 length"));
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return Err(model_error("Invalid schematic image base64 padding"));
    }
    let decoded_len = (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(limit_error)?;
    if decoded_len > maximum {
        return Err(limit_error());
    }
    let mut output = Vec::with_capacity(decoded_len);
    for (block_index, encoded) in bytes.chunks_exact(4).enumerate() {
        let mut block = [0u8; 4];
        for (index, byte) in encoded.iter().copied().enumerate() {
            block[index] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 64,
                _ => return Err(model_error("Invalid schematic image base64")),
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
            return Err(model_error("Invalid schematic image base64 padding"));
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

pub(super) fn image_metadata(data: &[u8], maximum_work: usize) -> Result<ImageMetadata, Error> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        png_metadata(data, maximum_work)
    } else if data.starts_with(b"\xff\xd8") {
        jpeg_metadata(data, maximum_work)
    } else if data.starts_with(b"BM") {
        bmp_metadata(data, maximum_work)
    } else {
        Err(model_error("Unsupported or malformed schematic image"))
    }
}

pub(super) fn extent_nm(size: u32, scale: f64, ppi: Option<u32>) -> Result<i64, Error> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(model_error(
            "Schematic image scale must be finite and positive",
        ));
    }
    let mm = size as f64 * scale * 25.4 / ppi.map_or(DEFAULT_DPI, f64::from);
    mm_to_nm(mm)
}

fn png_metadata(data: &[u8], maximum_work: usize) -> Result<ImageMetadata, Error> {
    if data.len() < 33 {
        return Err(model_error("Malformed schematic PNG"));
    }
    let first_length = be_u32(data, 8)? as usize;
    if first_length != 13 || data.get(12..16) != Some(b"IHDR") {
        return Err(model_error(
            "Schematic PNG must begin with a canonical IHDR",
        ));
    }
    let (mut width, mut height, mut ppm_x, mut ppm_y) = (0, 0, None, None);
    let (mut offset, mut work) = (8usize, 0usize);
    let mut saw_iend = false;
    while offset.checked_add(8).is_some_and(|end| end <= data.len()) {
        let length = be_u32(data, offset)? as usize;
        let end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .ok_or_else(limit_error)?;
        if end > data.len() {
            return Err(model_error("Malformed schematic PNG"));
        }
        work = checked_limit(work, length.saturating_add(12), maximum_work)?;
        let kind = &data[offset + 4..offset + 8];
        let chunk = &data[offset + 8..offset + 8 + length];
        if offset == 8 && kind == b"IHDR" {
            width = be_u32(chunk, 0)?;
            height = be_u32(chunk, 4)?;
        } else if kind == b"pHYs" && length >= 9 && chunk[8] == 1 {
            ppm_x = nonzero(be_u32(chunk, 0)?);
            ppm_y = nonzero(be_u32(chunk, 4)?);
        }
        if kind == b"IEND" {
            if length != 0 || end != data.len() {
                return Err(model_error("Malformed schematic PNG terminator"));
            }
            saw_iend = true;
            offset = end;
            break;
        }
        offset = end;
    }
    if !saw_iend || offset != data.len() || width == 0 || height == 0 {
        return Err(model_error("Malformed schematic PNG dimensions"));
    }
    Ok(ImageMetadata {
        format: ImageFormat::Png,
        width,
        height,
        ppi_x: ppm_x.and_then(ppm_to_ppi),
        ppi_y: ppm_y.and_then(ppm_to_ppi),
        work,
    })
}

fn jpeg_metadata(data: &[u8], maximum_work: usize) -> Result<ImageMetadata, Error> {
    let (mut offset, mut work) = (2usize, 2usize);
    let (mut ppi_x, mut ppi_y) = (None, None);
    while offset.checked_add(9).is_some_and(|end| end <= data.len()) {
        if data[offset] != 0xff {
            offset += 1;
            work = checked_limit(work, 1, maximum_work)?;
            continue;
        }
        let marker = data[offset + 1];
        offset += 2;
        work = checked_limit(work, 2, maximum_work)?;
        if marker == 0xd8 || marker == 0xd9 {
            continue;
        }
        let length = be_u16(data, offset)? as usize;
        if length < 2 {
            return Err(model_error("Malformed schematic JPEG segment"));
        }
        let end = offset.checked_add(length).ok_or_else(limit_error)?;
        if end > data.len() {
            return Err(model_error("Malformed schematic JPEG segment"));
        }
        work = checked_limit(work, length, maximum_work)?;
        let segment = &data[offset + 2..end];
        if marker == 0xe0 && segment.starts_with(b"JFIF\0") && segment.len() >= 12 {
            let x = u16::from_be_bytes([segment[8], segment[9]]) as u32;
            let y = u16::from_be_bytes([segment[10], segment[11]]) as u32;
            if x > 0 && y > 0 {
                match segment[7] {
                    1 => {
                        ppi_x = Some(x);
                        ppi_y = Some(y);
                    }
                    2 => {
                        ppi_x = nonzero((x as f64 * 2.54).round_ties_even() as u32);
                        ppi_y = nonzero((y as f64 * 2.54).round_ties_even() as u32);
                    }
                    _ => {}
                }
            }
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if segment.len() < 5 {
                return Err(model_error("Malformed schematic JPEG dimensions"));
            }
            let height = u16::from_be_bytes([segment[1], segment[2]]) as u32;
            let width = u16::from_be_bytes([segment[3], segment[4]]) as u32;
            if width == 0 || height == 0 {
                return Err(model_error("Malformed schematic JPEG dimensions"));
            }
            return Ok(ImageMetadata {
                format: ImageFormat::Jpeg,
                width,
                height,
                ppi_x,
                ppi_y,
                work,
            });
        }
        offset = end;
    }
    Err(model_error("Malformed schematic JPEG dimensions"))
}

fn bmp_metadata(data: &[u8], maximum_work: usize) -> Result<ImageMetadata, Error> {
    if data.len() < 26 || data.len() > maximum_work {
        return if data.len() > maximum_work {
            Err(limit_error())
        } else {
            Err(model_error("Malformed schematic BMP"))
        };
    }
    let dib = le_u32(data, 14)?;
    let (width, height, ppi_x, ppi_y) = if dib == 12 {
        (
            le_u16(data, 18)? as u32,
            le_u16(data, 20)? as u32,
            None,
            None,
        )
    } else if dib >= 40 && data.len() >= 54 {
        let width = le_i32(data, 18)?.unsigned_abs();
        let height = le_i32(data, 22)?.unsigned_abs();
        let ppm_x = le_i32(data, 38)?;
        let ppm_y = le_i32(data, 42)?;
        (width, height, bmp_ppm_to_ppi(ppm_x), bmp_ppm_to_ppi(ppm_y))
    } else {
        return Err(model_error("Unsupported schematic BMP header"));
    };
    if width == 0 || height == 0 {
        return Err(model_error("Malformed schematic BMP dimensions"));
    }
    Ok(ImageMetadata {
        format: ImageFormat::Bmp,
        width,
        height,
        ppi_x,
        ppi_y,
        work: data.len(),
    })
}

fn ppm_to_ppi(ppm: u32) -> Option<u32> {
    nonzero((ppm as f64 * 0.0254).round_ties_even() as u32)
}

fn bmp_ppm_to_ppi(ppm: i32) -> Option<u32> {
    if ppm <= 0 {
        return None;
    }
    let pixels_per_cm = ppm as u32 / 100;
    nonzero((pixels_per_cm as f64 * 2.54).round_ties_even() as u32)
}

fn nonzero(value: u32) -> Option<u32> {
    (value > 0).then_some(value)
}

fn be_u16(data: &[u8], offset: usize) -> Result<u16, Error> {
    let bytes = data
        .get(offset..offset.saturating_add(2))
        .ok_or_else(|| model_error("Malformed schematic image metadata"))?;
    Ok(u16::from_be_bytes(bytes.try_into().unwrap()))
}

fn be_u32(data: &[u8], offset: usize) -> Result<u32, Error> {
    let bytes = data
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| model_error("Malformed schematic image metadata"))?;
    Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
}

fn le_u16(data: &[u8], offset: usize) -> Result<u16, Error> {
    let bytes = data
        .get(offset..offset.saturating_add(2))
        .ok_or_else(|| model_error("Malformed schematic image metadata"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn le_u32(data: &[u8], offset: usize) -> Result<u32, Error> {
    let bytes = data
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| model_error("Malformed schematic image metadata"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn le_i32(data: &[u8], offset: usize) -> Result<i32, Error> {
    let bytes = data
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| model_error("Malformed schematic image metadata"))?;
    Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
}

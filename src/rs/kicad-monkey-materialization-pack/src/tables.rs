//! The binding table and the chunk directory as column sections.
//!
//! Binding section, for `n` entries and `k` tied-net references: `u32 n`,
//! `u32 k`, then `u32` operation index, `i32` source UID, refdes, pad number
//! (string indices, `-1` for none), `i32` net (net table index, `-1` for
//! none), `i32` die length (`-1` for none), each `× n`, `u32` tied-net start
//! `× (n + 1)`, `i32` tied nets `× k`, `f64` die delay `× n` (NaN for none,
//! which JSON cannot hold), then `u8` source kind `× n`, padded to 8.
//!
//! Chunk directory section, for `m` chunks and `k` net references: `u32 m`,
//! `u32 k`, then `i32` slice (stack index), tile X, tile Y, each `× m`,
//! `i32` bounds `× 4m`, `u32` first operation and operation count, each
//! `× m`, `u32` net start `× (m + 1)`, `i32` nets `× k`, padded to 8.

use crate::PackError;
use crate::wire::{
    Cursor, StringTable, corrupt, count_u32, optional_string, pad8, put_f64, put_i32, put_u32,
    string,
};

fn length_i32(value: i64) -> Result<i32, PackError> {
    i32::try_from(value).map_err(|_| corrupt("length outside int32"))
}
use kicad_monkey_contracts::generated::materialization_document::{
    BindingEntry, BindingSourceKind, ChunkEntry, Nm, NmBounds, NonNegativeNm,
};
use std::collections::HashMap;

const SOURCE_KINDS: [BindingSourceKind; 11] = [
    BindingSourceKind::Track,
    BindingSourceKind::TrackArc,
    BindingSourceKind::Via,
    BindingSourceKind::Pad,
    BindingSourceKind::ZoneFill,
    BindingSourceKind::BoardGraphic,
    BindingSourceKind::FootprintGraphic,
    BindingSourceKind::BoardText,
    BindingSourceKind::FootprintText,
    BindingSourceKind::Profile,
    BindingSourceKind::Derived,
];

/// Net and slice keys by index, for the sections that refer to them.
pub(crate) struct Keys<'a> {
    pub(crate) slices: HashMap<&'a str, i32>,
    pub(crate) nets: HashMap<&'a str, i32>,
}

impl Keys<'_> {
    pub(crate) fn net(&self, key: Option<&str>) -> Result<i32, PackError> {
        key.map_or(Ok(-1), |key| {
            self.nets
                .get(key)
                .copied()
                .ok_or(corrupt("net not in the net table"))
        })
    }

    pub(crate) fn slice(&self, key: &str) -> Result<i32, PackError> {
        self.slices
            .get(key)
            .copied()
            .ok_or(corrupt("slice not in the stack"))
    }
}

/// Intern the binding's strings (before any other section is written, so
/// the table is complete).
pub(crate) fn intern_binding<'a>(
    binding: &'a [BindingEntry],
    strings: &mut StringTable<'a>,
) -> Result<(), PackError> {
    for entry in binding {
        strings.intern(&entry.source_uid)?;
        strings.optional(entry.refdes.as_deref())?;
        strings.optional(entry.pad_number.as_deref())?;
    }
    Ok(())
}

fn string_index(strings: &StringTable<'_>, value: Option<&str>) -> Result<i32, PackError> {
    value.map_or(Ok(-1), |value| {
        strings.get(value).ok_or(corrupt("string not interned"))
    })
}

pub(crate) fn encode_binding(
    binding: &[BindingEntry],
    strings: &StringTable<'_>,
    keys: &Keys<'_>,
) -> Result<Vec<u8>, PackError> {
    let tied: Vec<i32> = binding
        .iter()
        .flat_map(|entry| entry.tied_net_keys.iter())
        .map(|key| keys.net(Some(key)))
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    put_u32(
        &mut out,
        &[count_u32(binding.len())?, count_u32(tied.len())?],
    );
    let indices: Vec<u32> = binding.iter().map(|entry| entry.operation_index).collect();
    put_u32(&mut out, &indices);
    let column = |value: &dyn Fn(&BindingEntry) -> Option<&str>| {
        binding
            .iter()
            .map(|entry| string_index(strings, value(entry)))
            .collect::<Result<Vec<_>, _>>()
    };
    put_i32(&mut out, &column(&|entry| Some(entry.source_uid.as_str()))?);
    put_i32(&mut out, &column(&|entry| entry.refdes.as_deref())?);
    put_i32(&mut out, &column(&|entry| entry.pad_number.as_deref())?);
    let nets = binding
        .iter()
        .map(|entry| keys.net(entry.net_key.as_deref()))
        .collect::<Result<Vec<_>, _>>()?;
    put_i32(&mut out, &nets);
    let lengths: Vec<i32> = binding
        .iter()
        .map(|entry| {
            entry
                .die_length_nm
                .as_ref()
                .map_or(Ok(-1), |nm| length_i32(**nm))
        })
        .collect::<Result<_, _>>()?;
    put_i32(&mut out, &lengths);
    let mut starts = vec![0u32];
    for entry in binding {
        let last = *starts.last().unwrap_or(&0);
        starts.push(last + count_u32(entry.tied_net_keys.len())?);
    }
    put_u32(&mut out, &starts);
    put_i32(&mut out, &tied);
    let delays: Vec<f64> = binding
        .iter()
        .map(|entry| entry.die_delay_ps.unwrap_or(f64::NAN))
        .collect();
    put_f64(&mut out, &delays);
    for entry in binding {
        let kind = SOURCE_KINDS
            .iter()
            .position(|kind| *kind == entry.source_kind)
            .unwrap_or_default();
        out.push(u8::try_from(kind).unwrap_or_default());
    }
    pad8(&mut out);
    Ok(out)
}

pub(crate) fn decode_binding(
    section: &[u8],
    strings: &[String],
    nets: &[String],
) -> Result<Vec<BindingEntry>, PackError> {
    let mut cursor = Cursor::new(section);
    let count = cursor.usize32()?;
    let tied_count = cursor.usize32()?;
    let needed = count
        .checked_mul(37)
        .and_then(|bytes| bytes.checked_add(tied_count.checked_mul(4)?.checked_add(4)?))
        .ok_or(PackError::Truncated)?;
    if needed > cursor.remaining() {
        return Err(PackError::Truncated);
    }
    let indices = cursor.u32s(count)?;
    let uids = cursor.i32s(count)?;
    let refdes = cursor.i32s(count)?;
    let pads = cursor.i32s(count)?;
    let net_indices = cursor.i32s(count)?;
    let lengths = cursor.i32s(count)?;
    let starts = cursor.u32s(count + 1)?;
    let tied = cursor.i32s(tied_count)?;
    let delays = cursor.f64s(count)?;
    let kinds = cursor.take(count)?;
    (0..count)
        .map(|row| {
            let span = |value: u32| usize::try_from(value).map_err(|_| corrupt("tied start"));
            let tied_net_keys = tied
                .get(span(starts[row])?..span(starts[row + 1])?)
                .ok_or(corrupt("tied nets out of range"))?
                .iter()
                .map(|index| string(nets, *index, "tied net out of range"))
                .collect::<Result<Vec<_>, _>>()?;
            let die_length_nm = match lengths[row] {
                -1 => None,
                value if value >= 0 => Some(NonNegativeNm(i64::from(value))),
                _ => return Err(corrupt("bad die length")),
            };
            Ok(BindingEntry {
                tied_net_keys,
                die_length_nm,
                die_delay_ps: Some(delays[row]).filter(|delay| !delay.is_nan()),
                operation_index: indices[row],
                source_kind: *SOURCE_KINDS
                    .get(usize::from(kinds[row]))
                    .ok_or(corrupt("unknown source kind"))?,
                source_uid: string(strings, uids[row], "source UID out of range")?,
                refdes: optional_string(strings, refdes[row], "refdes out of range")?,
                pad_number: optional_string(strings, pads[row], "pad number out of range")?,
                net_key: optional_string(nets, net_indices[row], "net out of range")?,
            })
        })
        .collect()
}

pub(crate) fn encode_chunks(chunks: &[ChunkEntry], keys: &Keys<'_>) -> Result<Vec<u8>, PackError> {
    let net_refs: Vec<i32> = chunks
        .iter()
        .flat_map(|chunk| chunk.net_keys.iter())
        .map(|key| keys.net(Some(key)))
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    put_u32(
        &mut out,
        &[count_u32(chunks.len())?, count_u32(net_refs.len())?],
    );
    let slices = chunks
        .iter()
        .map(|chunk| keys.slice(&chunk.slice_key))
        .collect::<Result<Vec<_>, _>>()?;
    put_i32(&mut out, &slices);
    put_i32(
        &mut out,
        &chunks.iter().map(|chunk| chunk.tile_x).collect::<Vec<_>>(),
    );
    put_i32(
        &mut out,
        &chunks.iter().map(|chunk| chunk.tile_y).collect::<Vec<_>>(),
    );
    let bounds: Vec<i32> = chunks
        .iter()
        .flat_map(|chunk| {
            let b = &chunk.bounds;
            [b.min_x_nm.0, b.min_y_nm.0, b.max_x_nm.0, b.max_y_nm.0]
        })
        .collect();
    put_i32(&mut out, &bounds);
    put_u32(
        &mut out,
        &chunks
            .iter()
            .map(|chunk| chunk.first_operation_index)
            .collect::<Vec<_>>(),
    );
    put_u32(
        &mut out,
        &chunks
            .iter()
            .map(|chunk| chunk.operation_count)
            .collect::<Vec<_>>(),
    );
    let mut starts = vec![0u32];
    for chunk in chunks {
        let last = *starts.last().unwrap_or(&0);
        starts.push(last + count_u32(chunk.net_keys.len())?);
    }
    put_u32(&mut out, &starts);
    put_i32(&mut out, &net_refs);
    pad8(&mut out);
    Ok(out)
}

pub(crate) fn decode_chunks(
    section: &[u8],
    slices: &[String],
    nets: &[String],
) -> Result<Vec<ChunkEntry>, PackError> {
    let mut cursor = Cursor::new(section);
    let count = cursor.usize32()?;
    let net_count = cursor.usize32()?;
    let needed = count
        .checked_mul(40)
        .and_then(|bytes| bytes.checked_add(net_count.checked_mul(4)?))
        .ok_or(PackError::Truncated)?;
    if needed > cursor.remaining() {
        return Err(PackError::Truncated);
    }
    let slice_indices = cursor.i32s(count)?;
    let tile_x = cursor.i32s(count)?;
    let tile_y = cursor.i32s(count)?;
    let bounds = cursor.i32s(4 * count)?;
    let firsts = cursor.u32s(count)?;
    let counts = cursor.u32s(count)?;
    let starts = cursor.u32s(count + 1)?;
    let net_refs = cursor.i32s(net_count)?;
    (0..count)
        .map(|row| {
            let span = |value: u32| usize::try_from(value).map_err(|_| corrupt("net start"));
            let net_keys = net_refs
                .get(span(starts[row])?..span(starts[row + 1])?)
                .ok_or(corrupt("chunk nets out of range"))?
                .iter()
                .map(|index| string(nets, *index, "chunk net out of range"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ChunkEntry {
                slice_key: string(slices, slice_indices[row], "chunk slice out of range")?,
                tile_x: tile_x[row],
                tile_y: tile_y[row],
                bounds: NmBounds {
                    min_x_nm: Nm(bounds[4 * row]),
                    min_y_nm: Nm(bounds[4 * row + 1]),
                    max_x_nm: Nm(bounds[4 * row + 2]),
                    max_y_nm: Nm(bounds[4 * row + 3]),
                },
                net_keys,
                first_operation_index: firsts[row],
                operation_count: counts[row],
            })
        })
        .collect()
}

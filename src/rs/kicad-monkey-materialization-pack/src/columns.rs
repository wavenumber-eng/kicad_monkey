//! One chunk's operations as columns (a struct of arrays).
//!
//! | Column | Type | Per operation |
//! |---|---|---|
//! | header | `u32` × 2 | operation count `n`, parameter count `p` (once) |
//! | affine linear | `i32` | index into the pack's linear-part table |
//! | affine translation | `i32` × 2 | `tx_nm ty_nm` |
//! | bounds | `i32` × 4 | `min_x min_y max_x max_y` |
//! | slice | `i32` | stack index (a hole's start slice) |
//! | net | `i32` | net table index, `-1` for none |
//! | group | `i32` | string table index, `-1` for none |
//! | offset | `i32` | `offset_nm`, `0` for none |
//! | parameter start | `u32` | into the parameter stream (`n + 1` values) |
//! | parameters | `i32` × `p` | shapes and hole details (see `params.rs`) |
//! | kind | `u8` | `0` add, `1` subtract, `2` plated hole, `3` unplated hole |
//! | material | `u8` | surface material code, `255` for holes |
//! | shape | `u8` | shape code |
//!
//! The block is padded to 8 bytes. An operation's index is its chunk's first
//! index plus its row.

use crate::PackError;
use crate::params::{HoleDetails, ParamReader, ParamWriter, shape_code};
use crate::tables::Keys;
use crate::wire::{
    Cursor, LinearTable, StringTable, corrupt, count_u32, optional_string, pad8, put_i32, put_u32,
};
use kicad_monkey_contracts::generated::materialization_document::{
    Affine2D, Backdrill, HoleOperation, HolePlating, MaterialAction, MaterialOperation, Nm,
    NmBounds, PostMachining, SurfaceMaterial, SurfaceOperation,
};

/// Fixed bytes per operation across every column but the parameters.
pub(crate) const BYTES_PER_OPERATION: usize = 4 + 8 + 16 + 16 + 4 + 3;

const MATERIALS: [SurfaceMaterial; 5] = [
    SurfaceMaterial::Copper,
    SurfaceMaterial::Dielectric,
    SurfaceMaterial::SolderMask,
    SurfaceMaterial::Paste,
    SurfaceMaterial::Silkscreen,
];

/// What a column block refers to by index while encoding.
pub(crate) struct Tables<'k, 's> {
    pub(crate) keys: &'k Keys<'k>,
    pub(crate) strings: &'k StringTable<'s>,
    pub(crate) linear: &'k LinearTable,
}

impl Tables<'_, '_> {
    fn slice(&self, key: &str) -> Result<i32, PackError> {
        self.keys.slice(key)
    }

    fn net(&self, key: Option<&String>) -> Result<i32, PackError> {
        self.keys.net(key.map(String::as_str))
    }
}

#[derive(Default)]
struct Columns {
    linear: Vec<i32>,
    translation: Vec<i32>,
    bounds: Vec<i32>,
    slice: Vec<i32>,
    net: Vec<i32>,
    group: Vec<i32>,
    offset: Vec<i32>,
    param_start: Vec<u32>,
    params: ParamWriter,
    kind: Vec<u8>,
    material: Vec<u8>,
    shape: Vec<u8>,
}

impl Columns {
    fn placement(
        &mut self,
        affine: &Affine2D,
        bounds: &NmBounds,
        tables: &Tables<'_, '_>,
    ) -> Result<(), PackError> {
        self.linear.push(
            tables
                .linear
                .get([affine.m11, affine.m12, affine.m21, affine.m22])
                .ok_or(corrupt("affine not in the linear table"))?,
        );
        self.translation.extend([affine.tx_nm.0, affine.ty_nm.0]);
        self.bounds.extend([
            bounds.min_x_nm.0,
            bounds.min_y_nm.0,
            bounds.max_x_nm.0,
            bounds.max_y_nm.0,
        ]);
        Ok(())
    }

    fn surface(
        &mut self,
        surface: &SurfaceOperation,
        tables: &Tables<'_, '_>,
    ) -> Result<(), PackError> {
        self.placement(&surface.affine, &surface.bounds, tables)?;
        self.slice.push(tables.slice(&surface.slice_key)?);
        self.net.push(tables.net(surface.net_key.as_ref())?);
        self.group
            .push(surface.group_key.as_deref().map_or(Ok(-1), |key| {
                tables
                    .strings
                    .get(key)
                    .ok_or(corrupt("group key not in the string table"))
            })?);
        self.offset
            .push(surface.offset_nm.as_ref().map_or(0, |offset| offset.0));
        self.kind
            .push(u8::from(surface.action == MaterialAction::Subtract));
        let material = MATERIALS
            .iter()
            .position(|material| *material == surface.material)
            .unwrap_or(0);
        self.material
            .push(u8::try_from(material).unwrap_or_default());
        self.shape.push(shape_code(&surface.shape));
        self.params.shape(&surface.shape)
    }

    fn hole(&mut self, hole: &HoleOperation, tables: &Tables<'_, '_>) -> Result<(), PackError> {
        self.placement(&hole.affine, &hole.bounds, tables)?;
        self.slice.push(tables.slice(&hole.start_slice_key)?);
        self.net.push(tables.net(hole.net_key.as_ref())?);
        self.group.push(-1);
        self.offset.push(0);
        self.kind.push(if hole.plating == HolePlating::Plated {
            2
        } else {
            3
        });
        self.material.push(255);
        self.shape.push(shape_code(&hole.shape));
        self.params.shape(&hole.shape)?;
        let backdrills = hole
            .backdrills
            .iter()
            .map(|drill| {
                let until = drill
                    .until_slice_key
                    .as_deref()
                    .map_or(Ok(-1), |key| tables.slice(key))?;
                Ok((drill.clone(), until))
            })
            .collect::<Result<Vec<_>, PackError>>()?;
        self.params.hole_details(
            tables.slice(&hole.end_slice_key)?,
            hole.plating_thickness_nm.as_ref(),
            hole.protection.as_ref(),
            &backdrills,
        )?;
        self.params
            .hole_machining(&hole.post_machining, hole.castellated)
    }
}

/// Encode one chunk's operations as a padded column block.
pub(crate) fn encode_chunk(
    operations: &[MaterialOperation],
    tables: &Tables<'_, '_>,
) -> Result<Vec<u8>, PackError> {
    let mut columns = Columns::default();
    for operation in operations {
        columns
            .param_start
            .push(count_u32(columns.params.values.len())?);
        match operation {
            MaterialOperation::SurfaceOperation(surface) => columns.surface(surface, tables)?,
            MaterialOperation::HoleOperation(hole) => columns.hole(hole, tables)?,
        }
    }
    columns
        .param_start
        .push(count_u32(columns.params.values.len())?);
    let mut bytes = Vec::new();
    put_u32(
        &mut bytes,
        &[
            count_u32(operations.len())?,
            count_u32(columns.params.values.len())?,
        ],
    );
    for column in [
        &columns.linear,
        &columns.translation,
        &columns.bounds,
        &columns.slice,
        &columns.net,
        &columns.group,
        &columns.offset,
    ] {
        put_i32(&mut bytes, column);
    }
    put_u32(&mut bytes, &columns.param_start);
    put_i32(&mut bytes, &columns.params.values);
    bytes.extend(&columns.kind);
    bytes.extend(&columns.material);
    bytes.extend(&columns.shape);
    pad8(&mut bytes);
    Ok(bytes)
}

/// The document tables a decoded chunk refers to.
pub(crate) struct Names<'a> {
    pub(crate) slices: &'a [String],
    pub(crate) nets: &'a [String],
    pub(crate) strings: &'a [String],
    pub(crate) linear: &'a [[f64; 4]],
}

impl Names<'_> {
    fn slice(&self, index: i32) -> Result<String, PackError> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.slices.get(index))
            .cloned()
            .ok_or(corrupt("slice index out of range"))
    }
}

/// One chunk block's columns, read but not yet assembled.
struct Block<'a> {
    linear: Vec<i32>,
    translation: Vec<i32>,
    bounds: Vec<i32>,
    slice: Vec<i32>,
    net: Vec<i32>,
    group: Vec<i32>,
    offset: Vec<i32>,
    param_start: Vec<u32>,
    params: Vec<i32>,
    kind: &'a [u8],
    material: &'a [u8],
    shape: &'a [u8],
}

impl<'a> Block<'a> {
    fn read(block: &'a [u8]) -> Result<Self, PackError> {
        let mut cursor = Cursor::new(block);
        let head = cursor.u32s(2)?;
        let (count, param_count) = (
            usize::try_from(head[0]).map_err(|_| corrupt("count"))?,
            usize::try_from(head[1]).map_err(|_| corrupt("count"))?,
        );
        let needed = count
            .checked_mul(BYTES_PER_OPERATION)
            .and_then(|bytes| bytes.checked_add(param_count.checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(12))
            .ok_or(PackError::Truncated)?;
        if needed > block.len() {
            return Err(PackError::Truncated);
        }
        Ok(Self {
            linear: cursor.i32s(count)?,
            translation: cursor.i32s(2 * count)?,
            bounds: cursor.i32s(4 * count)?,
            slice: cursor.i32s(count)?,
            net: cursor.i32s(count)?,
            group: cursor.i32s(count)?,
            offset: cursor.i32s(count)?,
            param_start: cursor.u32s(count + 1)?,
            params: cursor.i32s(param_count)?,
            kind: cursor.take(count)?,
            material: cursor.take(count)?,
            shape: cursor.take(count)?,
        })
    }

    fn placement(&self, row: usize, names: &Names<'_>) -> Result<(Affine2D, NmBounds), PackError> {
        let [m11, m12, m21, m22] = *usize::try_from(self.linear[row])
            .ok()
            .and_then(|index| names.linear.get(index))
            .ok_or(corrupt("linear index out of range"))?;
        let affine = Affine2D {
            m11,
            m12,
            m21,
            m22,
            tx_nm: Nm(self.translation[2 * row]),
            ty_nm: Nm(self.translation[2 * row + 1]),
        };
        let bounds = NmBounds {
            min_x_nm: Nm(self.bounds[4 * row]),
            min_y_nm: Nm(self.bounds[4 * row + 1]),
            max_x_nm: Nm(self.bounds[4 * row + 2]),
            max_y_nm: Nm(self.bounds[4 * row + 3]),
        };
        Ok((affine, bounds))
    }

    fn operation(
        &self,
        row: usize,
        index: u32,
        names: &Names<'_>,
    ) -> Result<MaterialOperation, PackError> {
        let start = usize::try_from(self.param_start[row]).map_err(|_| corrupt("param start"))?;
        let end = usize::try_from(self.param_start[row + 1]).map_err(|_| corrupt("param start"))?;
        let values = self
            .params
            .get(start..end)
            .ok_or(corrupt("parameter range out of bounds"))?;
        let mut reader = ParamReader::new(values);
        let shape = reader.shape(self.shape[row])?;
        let (affine, bounds) = self.placement(row, names)?;
        let net_key = optional_string(names.nets, self.net[row], "net index out of range")?;
        let kind = self.kind[row];
        let operation = match kind {
            0 | 1 => MaterialOperation::SurfaceOperation(SurfaceOperation {
                kind: "surface".to_owned(),
                index,
                action: if kind == 1 {
                    MaterialAction::Subtract
                } else {
                    MaterialAction::Add
                },
                material: *MATERIALS
                    .get(usize::from(self.material[row]))
                    .ok_or(corrupt("unknown material code"))?,
                slice_key: names.slice(self.slice[row])?,
                net_key,
                group_key: optional_string(
                    names.strings,
                    self.group[row],
                    "group index out of range",
                )?,
                offset_nm: (self.offset[row] != 0).then_some(Nm(self.offset[row])),
                shape,
                affine,
                bounds,
            }),
            2 | 3 => {
                let details = reader.hole_details()?;
                let machining = reader.hole_machining()?;
                let row = HoleRow {
                    index,
                    plated: kind == 2,
                    start_slice: names.slice(self.slice[row])?,
                    net_key,
                    shape,
                    affine,
                    bounds,
                };
                hole(details, machining, names, row)?
            }
            _ => return Err(corrupt("unknown operation kind")),
        };
        if !reader.finished() {
            return Err(corrupt("operation parameters not fully read"));
        }
        Ok(operation)
    }
}

/// Decode one chunk block whose first operation has index `first`.
pub(crate) fn decode_chunk(
    block: &[u8],
    first: u32,
    names: &Names<'_>,
) -> Result<Vec<MaterialOperation>, PackError> {
    let columns = Block::read(block)?;
    (0..columns.kind.len())
        .map(|row| {
            let index = first
                .checked_add(u32::try_from(row).map_err(|_| corrupt("row"))?)
                .ok_or(corrupt("operation index overflows"))?;
            columns.operation(row, index, names)
        })
        .collect()
}

/// The column values of one hole row.
struct HoleRow {
    index: u32,
    plated: bool,
    start_slice: String,
    net_key: Option<String>,
    shape: kicad_monkey_contracts::generated::materialization_document::Shape2d,
    affine: Affine2D,
    bounds: NmBounds,
}

fn hole(
    details: HoleDetails,
    (post_machining, castellated): (Vec<PostMachining>, Option<bool>),
    names: &Names<'_>,
    row: HoleRow,
) -> Result<MaterialOperation, PackError> {
    let backdrills = details
        .backdrills
        .into_iter()
        .map(|(side, diameter_nm, until)| {
            Ok(Backdrill {
                side,
                diameter_nm,
                until_slice_key: optional_string(names.slices, until, "slice index out of range")?,
            })
        })
        .collect::<Result<Vec<_>, PackError>>()?;
    Ok(MaterialOperation::HoleOperation(HoleOperation {
        kind: "hole".to_owned(),
        index: row.index,
        shape: row.shape,
        affine: row.affine,
        plating: if row.plated {
            HolePlating::Plated
        } else {
            HolePlating::Unplated
        },
        start_slice_key: row.start_slice,
        end_slice_key: names.slice(details.end_slice)?,
        net_key: row.net_key,
        plating_thickness_nm: details.plating_thickness,
        protection: details.protection,
        backdrills,
        post_machining,
        castellated,
        bounds: row.bounds,
    }))
}

//! Operation ordering and (slice, tile) chunk assembly.
//!
//! The producer and the scope filter both finish through
//! [`chunk_operations`], so a selective materialization and the same filter
//! applied to a complete one order, index, and chunk identically.

use crate::generated::materialization_document::{
    ChunkEntry, MaterialOperation, MaterialProduct, Nm, NmBounds, SurfaceMaterial,
};
use std::collections::{BTreeMap, BTreeSet};

/// Product an operation belongs to. Every operation maps to exactly one.
pub fn operation_product(operation: &MaterialOperation) -> MaterialProduct {
    match operation {
        MaterialOperation::HoleOperation(_) => MaterialProduct::Holes,
        MaterialOperation::SurfaceOperation(surface) => match surface.material {
            SurfaceMaterial::Copper => MaterialProduct::Copper,
            SurfaceMaterial::Dielectric => MaterialProduct::Dielectric,
            SurfaceMaterial::SolderMask => MaterialProduct::SolderMask,
            SurfaceMaterial::Paste => MaterialProduct::Paste,
            SurfaceMaterial::Silkscreen => MaterialProduct::Silkscreen,
        },
    }
}

/// Slice an operation is chunked on: its own slice, or a hole's start slice.
pub fn operation_slice(operation: &MaterialOperation) -> &str {
    match operation {
        MaterialOperation::SurfaceOperation(surface) => &surface.slice_key,
        MaterialOperation::HoleOperation(hole) => &hole.start_slice_key,
    }
}

/// Board-frame bounds of an operation after its affine.
pub fn operation_bounds(operation: &MaterialOperation) -> &NmBounds {
    match operation {
        MaterialOperation::SurfaceOperation(surface) => &surface.bounds,
        MaterialOperation::HoleOperation(hole) => &hole.bounds,
    }
}

/// Net that owns an operation, when it has one.
pub fn operation_net(operation: &MaterialOperation) -> Option<&str> {
    match operation {
        MaterialOperation::SurfaceOperation(surface) => surface.net_key.as_deref(),
        MaterialOperation::HoleOperation(hole) => hole.net_key.as_deref(),
    }
}

fn set_index(operation: &mut MaterialOperation, index: u32) {
    match operation {
        MaterialOperation::SurfaceOperation(surface) => surface.index = index,
        MaterialOperation::HoleOperation(hole) => hole.index = index,
    }
}

/// Tile containing the centre of `bounds`, using floor division so negative
/// coordinates fall into negative tiles.
pub(super) fn tile_of(bounds: &NmBounds, tile_size_nm: i64) -> (i32, i32) {
    let centre = |low: &Nm, high: &Nm| (i64::from(**low) + i64::from(**high)).div_euclid(2);
    let tile = |value: i64| {
        let index = value.div_euclid(tile_size_nm.max(1));
        i32::try_from(index).unwrap_or(if index < 0 { i32::MIN } else { i32::MAX })
    };
    (
        tile(centre(&bounds.min_x_nm, &bounds.max_x_nm)),
        tile(centre(&bounds.min_y_nm, &bounds.max_y_nm)),
    )
}

fn union(into: &mut NmBounds, other: &NmBounds) {
    into.min_x_nm = Nm((*into.min_x_nm).min(*other.min_x_nm));
    into.min_y_nm = Nm((*into.min_y_nm).min(*other.min_y_nm));
    into.max_x_nm = Nm((*into.max_x_nm).max(*other.max_x_nm));
    into.max_y_nm = Nm((*into.max_y_nm).max(*other.max_y_nm));
}

/// Result of [`chunk_operations`]: reindexed operations, their chunks, and
/// the new index of each input operation.
pub struct ChunkedOperations {
    pub operations: Vec<MaterialOperation>,
    pub chunks: Vec<ChunkEntry>,
    pub new_index_of_input: Vec<u32>,
}

/// Order operations by (slice order, tile row, tile column, input order),
/// assign dense indices, and build one chunk per non-empty (slice, tile).
///
/// `slice_order` maps every referenced slice key to its stack order; keys
/// missing from it sort last. Chunk bounds are the union of member bounds,
/// and chunk net keys are the sorted distinct member nets.
pub fn chunk_operations(
    operations: Vec<MaterialOperation>,
    slice_order: &BTreeMap<String, u32>,
    tile_size_nm: i64,
) -> ChunkedOperations {
    let mut keyed: Vec<((u32, i32, i32), usize, MaterialOperation)> = operations
        .into_iter()
        .enumerate()
        .map(|(input, operation)| {
            let order = slice_order
                .get(operation_slice(&operation))
                .copied()
                .unwrap_or(u32::MAX);
            let (tile_x, tile_y) = tile_of(operation_bounds(&operation), tile_size_nm);
            ((order, tile_y, tile_x), input, operation)
        })
        .collect();
    keyed.sort_by_key(|(key, input, _)| (*key, *input));

    let mut new_index_of_input = vec![0u32; keyed.len()];
    let mut ordered = Vec::with_capacity(keyed.len());
    let mut chunks: Vec<ChunkEntry> = Vec::new();
    let mut chunk_nets: Vec<BTreeSet<String>> = Vec::new();
    for (position, (key, input, mut operation)) in keyed.into_iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        set_index(&mut operation, index);
        new_index_of_input[input] = index;
        let (_, tile_y, tile_x) = key;
        let starts_chunk = chunks.last().is_none_or(|chunk| {
            chunk.slice_key != operation_slice(&operation)
                || chunk.tile_x != tile_x
                || chunk.tile_y != tile_y
        });
        if starts_chunk {
            chunks.push(ChunkEntry {
                slice_key: operation_slice(&operation).to_owned(),
                tile_x,
                tile_y,
                bounds: operation_bounds(&operation).clone(),
                net_keys: Vec::new(),
                first_operation_index: index,
                operation_count: 0,
            });
            chunk_nets.push(BTreeSet::new());
        }
        if let (Some(chunk), Some(nets)) = (chunks.last_mut(), chunk_nets.last_mut()) {
            union(&mut chunk.bounds, operation_bounds(&operation));
            chunk.operation_count += 1;
            if let Some(net) = operation_net(&operation) {
                nets.insert(net.to_owned());
            }
        }
        ordered.push(operation);
    }
    for (chunk, nets) in chunks.iter_mut().zip(chunk_nets) {
        chunk.net_keys = nets.into_iter().collect();
    }
    ChunkedOperations {
        operations: ordered,
        chunks,
        new_index_of_input,
    }
}

//! Server-only Parry voxel collider construction and cross-chunk reconciliation.
//!
//! Every collider is built from fresh prospective `Voxels` values for the complete dirty
//! union of one terrain transaction. Pairwise neighbor reconciliation mutates only those
//! fresh values; installed colliders are replaced atomically after the whole batch builds.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "coordinate quantization over checked integer ranges"
)]

use super::model::{
    TERRAIN_CELL_SIZE_WORLD, TERRAIN_CHUNK_SIDE_CELLS, TerrainBits, TerrainChunkId,
};
use avian2d::parry::shape::{SharedShape, Voxels};
use avian2d::prelude::Collider;
use bevy::math::IVec2;
use bevy::prelude::Vec2;

/// Construct one fresh voxel shape from current occupancy. `None` for an empty chunk.
#[must_use]
pub(crate) fn build_voxels(bits: &TerrainBits) -> Option<Voxels> {
    if bits.is_empty() {
        return None;
    }
    let coordinates: Vec<IVec2> = bits
        .iter_occupied()
        .map(|(local_x, local_y)| IVec2::new(local_x as i32, local_y as i32))
        .collect();
    Some(Voxels::new(
        Vec2::splat(TERRAIN_CELL_SIZE_WORLD),
        &coordinates,
    ))
}

/// Wrap one reconciled voxel value into a fresh Avian collider.
#[must_use]
pub(crate) fn voxels_collider(voxels: Voxels) -> Collider {
    Collider::from(SharedShape::new(voxels))
}

/// Reconcile every adjacent pair of the dirty union exactly once, in stable chunk-ID
/// order, so voxel neighborhood states agree across chunk boundaries and collision
/// never snags on internal edges at Brawler chunk seams.
///
/// `entries` must already be sorted by chunk ID; both members of each pair are fresh
/// prospective values belonging to the same transaction, so the pinned Parry API that
/// mutates both sides is safe here.
pub(crate) fn reconcile_neighbors(entries: &mut [(TerrainChunkId, Voxels)]) {
    for left_index in 0..entries.len() {
        for right_index in (left_index + 1)..entries.len() {
            let (left_id, right_id) = (entries[left_index].0, entries[right_index].0);
            let orthogonal = (left_id.x == right_id.x && left_id.y.abs_diff(right_id.y) == 1)
                || (left_id.y == right_id.y && left_id.x.abs_diff(right_id.x) == 1);
            if !orthogonal {
                continue;
            }
            let shift = IVec2::new(
                i32::from(right_id.x - left_id.x) * TERRAIN_CHUNK_SIDE_CELLS as i32,
                i32::from(right_id.y - left_id.y) * TERRAIN_CHUNK_SIDE_CELLS as i32,
            );
            let (left_entries, right_entries) = entries.split_at_mut(right_index);
            let left = &mut left_entries[left_index].1;
            let right = &mut right_entries[0].1;
            left.combine_voxel_states(right, shift);
        }
    }
}

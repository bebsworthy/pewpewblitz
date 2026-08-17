//! Pure terrain coordinate, bitset, brush, digest, and fingerprint logic.
//!
//! Every helper here is deterministic, allocation-bounded by the engine ceilings, and
//! exercised at zero, boundaries, negative values, and overflow inputs. Nothing in this
//! module touches ECS, colliders, or the network.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::wildcard_imports,
    reason = "coordinate quantization over checked integer ranges and the shared model mirror"
)]

use super::model::*;
use crate::map::{MapPlacementId, MapShape};
use bevy::prelude::Vec2;
use std::collections::BTreeMap;

/// Euclidean floor division for possibly negative dividends.
#[must_use]
pub fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

/// Euclidean ceiling division for positive divisors.
#[must_use]
pub fn ceil_div(value: i32, divisor: i32) -> i32 {
    -((-value).div_euclid(divisor))
}

/// Convert one world axis to its global cell coordinate using floor division by the cell
/// size. Returns `None` for non-finite or unrepresentable inputs.
#[must_use]
fn world_axis_to_cell(value: f32) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let scaled = (value / TERRAIN_CELL_SIZE_WORLD).floor();
    if !(-2_147_483_648.0..=2_147_483_647.0).contains(&scaled) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    Some(scaled as i32)
}

/// World point to global cell. Cell `(x, y)` covers `[x*8, (x+1)*8] × [y*8, (y+1)*8]`.
#[must_use]
pub fn world_to_cell(point: Vec2) -> Option<(i32, i32)> {
    Some((world_axis_to_cell(point.x)?, world_axis_to_cell(point.y)?))
}

/// Global cell to its chunk, checked into `i16` chunk coordinates.
#[must_use]
fn cell_axis_to_chunk(cell: i32) -> Option<i16> {
    let chunk = floor_div(cell, TERRAIN_CHUNK_SIDE_CELLS as i32);
    i16::try_from(chunk).ok()
}

/// Global cell to `(chunk, local)` with the local index in `0..32`.
#[must_use]
pub fn cell_to_chunk_and_local(cell: (i32, i32)) -> Option<(TerrainChunkId, (u32, u32))> {
    let chunk = TerrainChunkId {
        x: cell_axis_to_chunk(cell.0)?,
        y: cell_axis_to_chunk(cell.1)?,
    };
    let local_x = cell.0.rem_euclid(TERRAIN_CHUNK_SIDE_CELLS as i32) as u32;
    let local_y = cell.1.rem_euclid(TERRAIN_CHUNK_SIDE_CELLS as i32) as u32;
    debug_assert!(local_x < TERRAIN_CHUNK_SIDE_CELLS && local_y < TERRAIN_CHUNK_SIDE_CELLS);
    Some((chunk, (local_x, local_y)))
}

/// The chunk's world-space minimum corner, `(chunk * 32) * 8`.
#[must_use]
pub fn chunk_min_world(chunk: TerrainChunkId) -> Vec2 {
    Vec2::new(
        f32::from(chunk.x) * TERRAIN_CHUNK_SIDE_WORLD,
        f32::from(chunk.y) * TERRAIN_CHUNK_SIDE_WORLD,
    )
}

/// The cell's world-space minimum corner.
#[must_use]
pub fn cell_min_world(cell: (i32, i32)) -> Vec2 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "world cells stay far inside f32 exactness"
    )]
    let axis = |value: i32| -> f32 { (i64::from(value) * 8) as f64 as f32 };
    Vec2::new(axis(cell.0), axis(cell.1))
}

/// The cell's world-space center, `((x + 0.5) * 8, (y + 0.5) * 8)`.
#[must_use]
pub fn cell_center_world(cell: (i32, i32)) -> Vec2 {
    cell_min_world(cell) + Vec2::splat(TERRAIN_CELL_SIZE_WORLD * 0.5)
}

/// True when the circle overlaps any occupied cell of the committed chunk occupancy.
/// Presentation-resolution twin of the server's collider-based landing clearance: the
/// lob preview repairs its landing marker exactly where authoritative resolution will.
#[must_use]
pub fn circle_overlaps_occupied(
    center: Vec2,
    radius: f32,
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
) -> bool {
    let (Some(min_cell), Some(max_cell)) = (
        world_to_cell(center - Vec2::splat(radius)),
        world_to_cell(center + Vec2::splat(radius)),
    ) else {
        return false;
    };
    for cell_y in min_cell.1..=max_cell.1 {
        for cell_x in min_cell.0..=max_cell.0 {
            let cell = (cell_x, cell_y);
            let Some((chunk, (local_x, local_y))) = cell_to_chunk_and_local(cell) else {
                continue;
            };
            if !chunks
                .get(&chunk)
                .is_some_and(|bits| bits.get(local_x, local_y))
            {
                continue;
            }
            let min = cell_min_world(cell);
            let closest = center.clamp(min, min + Vec2::splat(TERRAIN_CELL_SIZE_WORLD));
            if center.distance_squared(closest) <= radius * radius {
                return true;
            }
        }
    }
    false
}

/// The cell center expressed in half-cell units: `(2x + 1, 2y + 1)`.
#[must_use]
pub fn cell_center_half_cells(cell: (i32, i32)) -> (i32, i32) {
    (cell.0 * 2 + 1, cell.1 * 2 + 1)
}

/// Quantize one world axis to a signed half-cell coordinate with checked nearest rounding.
#[must_use]
fn quantize_half_cell(value: f32) -> Option<i16> {
    if !value.is_finite() {
        return None;
    }
    let units = (value / TERRAIN_SUBCELL_SIZE_WORLD).round();
    if !(-32_768.0..=32_767.0).contains(&units) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    Some(units as i16)
}

/// Canonical world value of one half-cell coordinate. Every wire brush round-trips through
/// exactly this value.
#[must_use]
pub fn half_cell_to_world(units: i16) -> f32 {
    f32::from(units) * TERRAIN_SUBCELL_SIZE_WORLD
}

/// The canonical world center of a quantized brush.
#[must_use]
pub fn brush_center_world(brush: TerrainBrush) -> Vec2 {
    Vec2::new(
        half_cell_to_world(brush.center_half_cells_x),
        half_cell_to_world(brush.center_half_cells_y),
    )
}

/// Quantize a world-space brush. The radius must be a finite multiple of the half-cell size
/// within the authored engine range.
#[must_use]
pub fn quantize_brush(center: Vec2, radius_world: f32) -> Option<TerrainBrush> {
    let center_half_cells_x = quantize_half_cell(center.x)?;
    let center_half_cells_y = quantize_half_cell(center.y)?;
    if !radius_world.is_finite() || radius_world < TERRAIN_CELL_SIZE_WORLD {
        return None;
    }
    let units = (radius_world / TERRAIN_SUBCELL_SIZE_WORLD).round();
    if (radius_world - units * TERRAIN_SUBCELL_SIZE_WORLD).abs() > 1.0e-4 {
        return None;
    }
    if !(2.0..=16.0).contains(&units) {
        return None;
    }
    Some(TerrainBrush {
        center_half_cells_x,
        center_half_cells_y,
        radius_half_cells: units as u16,
    })
}

impl TerrainBits {
    #[must_use]
    pub fn bit_index(local_x: u32, local_y: u32) -> usize {
        (local_y as usize) * TERRAIN_CHUNK_SIDE_CELLS as usize + local_x as usize
    }

    #[must_use]
    pub fn get(&self, local_x: u32, local_y: u32) -> bool {
        let index = Self::bit_index(local_x, local_y);
        self.0[index / 64] & (1_u64 << (index % 64)) != 0
    }

    pub fn set(&mut self, local_x: u32, local_y: u32) {
        let index = Self::bit_index(local_x, local_y);
        self.0[index / 64] |= 1_u64 << (index % 64);
    }

    pub fn clear(&mut self, local_x: u32, local_y: u32) {
        let index = Self::bit_index(local_x, local_y);
        self.0[index / 64] &= !(1_u64 << (index % 64));
    }

    #[must_use]
    pub fn count(&self) -> u32 {
        self.0.iter().map(|word| word.count_ones()).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|word| *word == 0)
    }

    /// Iterate occupied cells row-major as `(local_x, local_y)`.
    pub fn iter_occupied(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        (0..TERRAIN_CHUNK_SIDE_CELLS)
            .flat_map(move |local_y| {
                (0..TERRAIN_CHUNK_SIDE_CELLS).map(move |local_x| (local_x, local_y))
            })
            .filter(move |(local_x, local_y)| self.get(*local_x, *local_y))
    }
}

/// The inclusive candidate cell range of a brush.
#[must_use]
pub fn brush_cell_range(brush: TerrainBrush) -> ((i32, i32), (i32, i32)) {
    let center_x = i32::from(brush.center_half_cells_x);
    let center_y = i32::from(brush.center_half_cells_y);
    let radius = i32::from(brush.radius_half_cells);
    // A cell center is the odd half-cell coordinate `2x + 1`; containment needs
    // `|2x + 1 - center| <= radius`, solved for x by Euclidean division.
    let x_min = ceil_div(center_x - 1 - radius, 2);
    let x_max = floor_div(center_x - 1 + radius, 2);
    let y_min = ceil_div(center_y - 1 - radius, 2);
    let y_max = floor_div(center_y - 1 + radius, 2);
    ((x_min, x_max), (y_min, y_max))
}

/// Integer containment test for one cell against a brush.
#[must_use]
pub fn brush_erases_cell(brush: TerrainBrush, cell: (i32, i32)) -> bool {
    let center = cell_center_half_cells(cell);
    let dx = center.0 - i32::from(brush.center_half_cells_x);
    let dy = center.1 - i32::from(brush.center_half_cells_y);
    let radius = i32::from(brush.radius_half_cells);
    dx * dx + dy * dy <= radius * radius
}

/// The result of applying one brush to an occupancy map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrushOutcome {
    /// Sorted unique chunks whose occupancy changed.
    pub affected_chunks: Vec<TerrainChunkId>,
    pub erased_cells: u16,
}

/// Apply one brush to a chunk occupancy map, clearing occupied cells inside the brush.
/// Chunks absent from the map are empty forever and cannot change.
pub fn apply_brush(
    chunks: &mut BTreeMap<TerrainChunkId, TerrainBits>,
    brush: TerrainBrush,
) -> BrushOutcome {
    let ((x_min, x_max), (y_min, y_max)) = brush_cell_range(brush);
    let mut affected = Vec::new();
    let mut erased_cells: u32 = 0;
    for cell_y in y_min..=y_max {
        for cell_x in x_min..=x_max {
            if !brush_erases_cell(brush, (cell_x, cell_y)) {
                continue;
            }
            let Some((chunk, (local_x, local_y))) = cell_to_chunk_and_local((cell_x, cell_y))
            else {
                continue;
            };
            let Some(bits) = chunks.get_mut(&chunk) else {
                continue;
            };
            if bits.get(local_x, local_y) {
                bits.clear(local_x, local_y);
                erased_cells += 1;
                if !affected.contains(&chunk) {
                    affected.push(chunk);
                }
            }
        }
    }
    affected.sort_unstable();
    BrushOutcome {
        affected_chunks: affected,
        erased_cells: u16::try_from(erased_cells).unwrap_or(u16::MAX),
    }
}

/// Deterministic digest over sorted chunk IDs and their current bits.
#[must_use]
pub fn occupancy_digest(chunks: &BTreeMap<TerrainChunkId, TerrainBits>) -> u64 {
    let mut hash = crate::content::fnv1a64(b"brawler-terrain-occupancy-v1");
    for (chunk, bits) in chunks {
        let mut material = [0_u8; 4 + TERRAIN_WORDS_PER_CHUNK * 8];
        material[0..2].copy_from_slice(&chunk.x.to_le_bytes());
        material[2..4].copy_from_slice(&chunk.y.to_le_bytes());
        for (index, word) in bits.0.iter().enumerate() {
            material[4 + index * 8..4 + (index + 1) * 8].copy_from_slice(&word.to_le_bytes());
        }
        hash = crate::content::fnv1a64_seeded(hash, &material);
    }
    hash
}

/// Build the recovery snapshot for one occupancy map at one generation and revision.
#[must_use]
pub fn recovery_snapshot(
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
    generation: TerrainGeneration,
    revision: u64,
) -> TerrainRecoverySnapshot {
    TerrainRecoverySnapshot {
        generation,
        revision,
        chunks: chunks
            .iter()
            .map(|(chunk_id, occupancy)| TerrainChunkSnapshot {
                chunk_id: *chunk_id,
                occupancy: *occupancy,
            })
            .collect(),
    }
}

/// Serialized size of one recovery snapshot, if it serializes.
#[must_use]
pub fn recovery_snapshot_bytes(snapshot: &TerrainRecoverySnapshot) -> Option<usize> {
    postcard::to_allocvec(snapshot)
        .ok()
        .map(|bytes| bytes.len())
}

/// Serialized size of one live destruction event, if it serializes.
#[must_use]
pub fn destruction_event_bytes(event: &TerrainDestructionEvent) -> Option<usize> {
    postcard::to_allocvec(event).ok().map(|bytes| bytes.len())
}

/// Fingerprint the immutable terrain description: format/constants, sorted region
/// placements, and the initial chunk bits. Never hashes mutable occupancy or revision.
#[must_use]
pub fn terrain_fingerprint(
    regions: &[(MapPlacementId, Vec2, f32, MapShape)],
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
) -> u64 {
    let material = (
        TERRAIN_FORMAT_VERSION,
        TERRAIN_CELL_SIZE_WORLD,
        TERRAIN_SUBCELL_SIZE_WORLD,
        TERRAIN_CHUNK_SIDE_CELLS,
        TERRAIN_WORDS_PER_CHUNK,
        regions,
        chunks.iter().collect::<Vec<_>>(),
    );
    let bytes = postcard::to_allocvec(&material).unwrap_or_default();
    crate::content::fnv1a64(&bytes)
}

//! Initial destructible-terrain rasterization, validation, and fingerprinting.
//!
//! Map recipes keep describing immutable destructible *reservations*; this module turns
//! those reservations into the authoritative initial occupancy grid and rejects any layout
//! that violates playable bounds, permanent geometry, spawn safety, reachability, or the
//! aggregate engine/catalog budgets.

use super::resolver::overlaps_geometry;
use super::*;
use crate::terrain::grid as terrain_grid;
use crate::terrain::{
    MAX_TERRAIN_CELLS, MAX_TERRAIN_CHUNKS, MAX_TERRAIN_RECOVERY_BYTES, TerrainBits, TerrainChunkId,
    TerrainGeneration,
};
use bevy::prelude::Vec2;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The one region profile that declares initial destructible terrain occupancy.
pub const DESTRUCTIBLE_TERRAIN_REGION_PROFILE: RegionProfileId = RegionProfileId(1);

/// Spawn clearance used for terrain safety: matches the permanent-geometry spawn rule.
const SPAWN_CLEARANCE: f32 = 32.0;
/// Standard fighter radius used by layout reachability probes.
const FIGHTER_CLEARANCE: f32 = 24.0;
/// Reachability probe grid resolution.
const PROBE_CELL: f32 = 32.0;

/// The derived immutable initial terrain for one resolved map. Never part of the replicated
/// snapshot; authoritative and client roles derive it from their validated snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct InitialTerrainLayout {
    pub terrain_fingerprint: u64,
    pub chunks: BTreeMap<TerrainChunkId, TerrainBits>,
    pub total_cells: usize,
}

impl InitialTerrainLayout {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Closed circle-versus-occupied-cells test used by validation and tests.
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn circle_hits(&self, center: Vec2, radius: f32) -> bool {
        let mut occupied = BTreeSet::new();
        for (chunk, bits) in &self.chunks {
            for (local_x, local_y) in bits.iter_occupied() {
                let side = 32_i32;
                let global_x = i32::from(chunk.x) * side + local_x as i32;
                let global_y = i32::from(chunk.y) * side + local_y as i32;
                occupied.insert((global_x, global_y));
            }
        }
        circle_hits_occupied(center, radius, &occupied)
    }

    #[must_use]
    pub fn is_occupied(&self, cell: (i32, i32)) -> bool {
        terrain_grid::cell_to_chunk_and_local(cell).is_some_and(|(chunk, (local_x, local_y))| {
            self.chunks
                .get(&chunk)
                .is_some_and(|bits| bits.get(local_x, local_y))
        })
    }
}

/// Rasterize, validate, and fingerprint the initial terrain described by one normalized map.
///
/// Every rejected condition returns an exact reason; a partially instantiated layout is
/// never returned.
pub fn resolve_initial_terrain(
    playable_bounds: AxisAlignedMapRect,
    geometry: &[GeometryPlacement],
    regions: &[MapRegionPlacement],
    spawn_points: &[TeamSpawnPoint],
    mode_anchors: &[ModeAnchorPlacement],
    limits: EngineMapLimits,
) -> Result<InitialTerrainLayout, String> {
    let mut destructible: Vec<&MapRegionPlacement> = regions
        .iter()
        .filter(|region| region.profile_id == DESTRUCTIBLE_TERRAIN_REGION_PROFILE)
        .collect();
    destructible.sort_by_key(|region| region.placement_id);
    if destructible.len() > limits.max_destructible_reservations {
        return Err("too many destructible terrain reservations".to_string());
    }

    let mut chunks: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
    let mut occupied: BTreeSet<(i32, i32)> = BTreeSet::new();
    for region in &destructible {
        let before = occupied.len();
        rasterize_region(
            region,
            playable_bounds,
            geometry,
            &mut occupied,
            &mut chunks,
        )?;
        if occupied.len() == before {
            return Err(format!(
                "destructible reservation {:?} selects no complete cell",
                region.placement_id
            ));
        }
    }

    let total_cells = occupied.len();
    let max_cells = limits.max_destructible_cells.min(MAX_TERRAIN_CELLS);
    let max_chunks = limits.max_destructible_chunks.min(MAX_TERRAIN_CHUNKS);
    if total_cells > max_cells {
        return Err(format!(
            "destructible terrain exceeds the cell ceiling: {total_cells} > {max_cells}"
        ));
    }
    if chunks.len() > max_chunks {
        return Err(format!(
            "destructible terrain exceeds the chunk ceiling: {} > {max_chunks}",
            chunks.len()
        ));
    }
    let snapshot = terrain_grid::recovery_snapshot(
        &chunks,
        TerrainGeneration {
            map_instance_id: MapInstanceId(0),
            match_id: crate::matchplay::MatchId(0),
            terrain_fingerprint: 0,
        },
        0,
    );
    let max_bytes = limits
        .max_terrain_recovery_bytes
        .min(MAX_TERRAIN_RECOVERY_BYTES);
    if let Some(bytes) = terrain_grid::recovery_snapshot_bytes(&snapshot)
        && bytes > max_bytes
    {
        return Err(format!(
            "destructible terrain recovery exceeds the byte ceiling: {bytes} > {max_bytes}"
        ));
    }

    validate_terrain_spawn_safety(spawn_points, &occupied)?;
    // Reachability exists to protect spawns and objective areas; a pure layout fixture
    // without either has nothing to protect and may legitimately cover every cell.
    if !spawn_points.is_empty() || !mode_anchors.is_empty() {
        validate_terrain_reachability(
            playable_bounds,
            geometry,
            &occupied,
            mode_anchors,
            spawn_points,
        )?;
    }

    let fingerprint_material: Vec<_> = destructible
        .iter()
        .map(|region| {
            (
                region.placement_id,
                region.position,
                region.rotation,
                region.shape,
            )
        })
        .collect();
    Ok(InitialTerrainLayout {
        terrain_fingerprint: terrain_grid::terrain_fingerprint(&fingerprint_material, &chunks),
        chunks,
        total_cells,
    })
}

/// Inclusive point-in-shape test at one cell center, honoring authored rotation.
fn region_contains_cell(region: &MapRegionPlacement, center: Vec2) -> bool {
    match region.shape {
        MapShape::Rectangle { half_extents } => {
            let local = Vec2::from_angle(-region.rotation).rotate(center - region.position);
            local.x.abs() <= half_extents.x && local.y.abs() <= half_extents.y
        }
        MapShape::Circle { radius } => center.distance_squared(region.position) <= radius * radius,
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn rasterize_region(
    region: &MapRegionPlacement,
    playable_bounds: AxisAlignedMapRect,
    geometry: &[GeometryPlacement],
    occupied: &mut BTreeSet<(i32, i32)>,
    chunks: &mut BTreeMap<TerrainChunkId, TerrainBits>,
) -> Result<(), String> {
    let half = region.shape.bounding_half_extents(region.rotation);
    let min = region.position - half;
    let max = region.position + half;
    // Cell centers are (x + 0.5) * 8; select the complete candidate range.
    let x_min = ((min.x / 8.0) - 0.5).ceil() as i64;
    let x_max = ((max.x / 8.0) - 0.5).floor() as i64;
    let y_min = ((min.y / 8.0) - 0.5).ceil() as i64;
    let y_max = ((max.y / 8.0) - 0.5).floor() as i64;
    for cell_y in y_min..=y_max {
        for cell_x in x_min..=x_max {
            let Ok(cell_x) = i32::try_from(cell_x) else {
                return Err("destructible region exceeds the cell coordinate range".to_string());
            };
            let Ok(cell_y) = i32::try_from(cell_y) else {
                return Err("destructible region exceeds the cell coordinate range".to_string());
            };
            let center = terrain_grid::cell_center_world((cell_x, cell_y));
            if !region_contains_cell(region, center) {
                continue;
            }
            let cell_min = terrain_grid::cell_min_world((cell_x, cell_y));
            let cell_max = cell_min + Vec2::splat(crate::terrain::TERRAIN_CELL_SIZE_WORLD);
            if !playable_bounds.contains(cell_min) || !playable_bounds.contains(cell_max) {
                return Err(format!(
                    "destructible cell ({cell_x}, {cell_y}) leaves the playable bounds"
                ));
            }
            if cell_intersects_geometry(cell_min, cell_max, geometry) {
                return Err(format!(
                    "destructible cell ({cell_x}, {cell_y}) intersects permanent geometry"
                ));
            }
            if !occupied.insert((cell_x, cell_y)) {
                return Err(format!(
                    "destructible cell ({cell_x}, {cell_y}) is selected by two reservations"
                ));
            }
            let Some((chunk, (local_x, local_y))) =
                terrain_grid::cell_to_chunk_and_local((cell_x, cell_y))
            else {
                return Err("destructible cell exceeds the chunk coordinate range".to_string());
            };
            chunks.entry(chunk).or_default().set(local_x, local_y);
        }
    }
    Ok(())
}

/// Closed intersection test between one cell AABB and permanent geometry. Touching counts
/// so gameplay solidity can never overlap or sit flush inside a permanent collider.
fn cell_intersects_geometry(
    cell_min: Vec2,
    cell_max: Vec2,
    geometry: &[GeometryPlacement],
) -> bool {
    let cell_corners = [
        cell_min,
        Vec2::new(cell_max.x, cell_min.y),
        cell_max,
        Vec2::new(cell_min.x, cell_max.y),
    ];
    geometry.iter().any(|placement| match placement.shape {
        MapShape::Circle { radius } => {
            let closest = placement.position.clamp(cell_min, cell_max);
            closest.distance_squared(placement.position) <= radius * radius
        }
        MapShape::Rectangle { half_extents } => {
            // Separating-axis test between the cell AABB and the rotated rectangle.
            let rotation = Vec2::from_angle(placement.rotation);
            let rect_corners = [
                placement.position + rotation.rotate(Vec2::new(-half_extents.x, -half_extents.y)),
                placement.position + rotation.rotate(Vec2::new(half_extents.x, -half_extents.y)),
                placement.position + rotation.rotate(Vec2::new(half_extents.x, half_extents.y)),
                placement.position + rotation.rotate(Vec2::new(-half_extents.x, half_extents.y)),
            ];
            [
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
                rotation,
                Vec2::new(-rotation.y, rotation.x),
            ]
            .iter()
            .all(|axis| {
                let cell = project_axis(&cell_corners, *axis);
                let rect = project_axis(&rect_corners, *axis);
                cell.0 <= rect.1 && rect.0 <= cell.1
            })
        }
    })
}

fn project_axis(corners: &[Vec2; 4], axis: Vec2) -> (f32, f32) {
    let mut projections = corners.iter().map(|corner| corner.dot(axis));
    let first = projections.next().unwrap_or(0.0);
    projections.fold((first, first), |(min, max), value| {
        (min.min(value), max.max(value))
    })
}

/// Every spawn circle must clear the initial occupied cells.
fn validate_terrain_spawn_safety(
    spawn_points: &[TeamSpawnPoint],
    occupied: &BTreeSet<(i32, i32)>,
) -> Result<(), String> {
    for point in spawn_points {
        if circle_hits_occupied(point.position, SPAWN_CLEARANCE, occupied) {
            return Err(format!(
                "spawn point {:?} intersects initial destructible terrain",
                point.spawn_point_id
            ));
        }
    }
    Ok(())
}

/// Closed circle-versus-occupied-cells test.
fn circle_hits_occupied(center: Vec2, radius: f32, occupied: &BTreeSet<(i32, i32)>) -> bool {
    let Some(min_cell) = terrain_grid::world_to_cell(center - Vec2::splat(radius)) else {
        return false;
    };
    let Some(max_cell) = terrain_grid::world_to_cell(center + Vec2::splat(radius)) else {
        return false;
    };
    for cell_y in min_cell.1..=max_cell.1 {
        for cell_x in min_cell.0..=max_cell.0 {
            if !occupied.contains(&(cell_x, cell_y)) {
                continue;
            }
            let cell_min = terrain_grid::cell_min_world((cell_x, cell_y));
            let closest = center.clamp(
                cell_min,
                cell_min + Vec2::splat(crate::terrain::TERRAIN_CELL_SIZE_WORLD),
            );
            if closest.distance_squared(center) <= radius * radius {
                return true;
            }
        }
    }
    false
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
fn validate_terrain_reachability(
    playable_bounds: AxisAlignedMapRect,
    geometry: &[GeometryPlacement],
    occupied: &BTreeSet<(i32, i32)>,
    mode_anchors: &[ModeAnchorPlacement],
    spawn_points: &[TeamSpawnPoint],
) -> Result<(), String> {
    let size = playable_bounds.size();
    let width = (size.x / PROBE_CELL).floor() as usize;
    let height = (size.y / PROBE_CELL).floor() as usize;
    if width == 0 || height == 0 || width.saturating_mul(height) > 32_768 {
        return Err("invalid terrain reachability grid".to_string());
    }
    let to_cell = |point: Vec2| -> Option<(usize, usize)> {
        let relative = point - playable_bounds.min;
        let x = (relative.x / PROBE_CELL).floor() as isize;
        let y = (relative.y / PROBE_CELL).floor() as isize;
        (x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height)
            .then_some((x as usize, y as usize))
    };
    let center_of = |x: usize, y: usize| -> Vec2 {
        playable_bounds.min
            + Vec2::new((x as f32 + 0.5) * PROBE_CELL, (y as f32 + 0.5) * PROBE_CELL)
    };
    let is_clear = |x: usize, y: usize| -> bool {
        let point = center_of(x, y);
        playable_bounds.contains_with_inset(point, FIGHTER_CLEARANCE)
            && !overlaps_geometry(point, FIGHTER_CLEARANCE, geometry)
            && !circle_hits_occupied(point, FIGHTER_CLEARANCE, occupied)
    };

    // Destructible cover may legally claim the exact map center, so the common combat
    // probe is the deterministic nearest clear cell to the playable center (squared
    // distance, then y, then x).
    let center_point = playable_bounds.center();
    let mut probe = None;
    for y in 0..height {
        for x in 0..width {
            if !is_clear(x, y) {
                continue;
            }
            let delta = center_of(x, y) - center_point;
            let candidate = (delta.length_squared(), y, x);
            if probe.is_none_or(|current| candidate < current) {
                probe = Some(candidate);
            }
        }
    }
    let Some((_, probe_y, probe_x)) = probe else {
        return Err("map has no clear fighter position for the combat probe".to_string());
    };

    let mut reachable = vec![false; width * height];
    let mut queue = VecDeque::from([(probe_x, probe_y)]);
    reachable[probe_y * width + probe_x] = true;
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(1_isize, 0_isize), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx as usize >= width || ny as usize >= height {
                continue;
            }
            let next = (nx as usize, ny as usize);
            let index = next.1 * width + next.0;
            if !reachable[index] && is_clear(next.0, next.1) {
                reachable[index] = true;
                queue.push_back(next);
            }
        }
    }
    for point in spawn_points {
        let Some(cell) = to_cell(point.position) else {
            return Err("spawn is outside the terrain reachability grid".to_string());
        };
        if !reachable[cell.1 * width + cell.0] {
            return Err("spawn cannot reach the terrain-aware combat probe".to_string());
        }
    }
    for anchor in mode_anchors {
        let ModeAnchorShape::Area { position, shape } = anchor.shape else {
            continue;
        };
        let area = NormalizedArea {
            center: position,
            shape,
        };
        let has_reachable_position = (0..height).any(|y| {
            (0..width).any(|x| reachable[y * width + x] && area.contains_point(center_of(x, y)))
        });
        if !has_reachable_position {
            return Err(format!(
                "objective area {:?} has no reachable legal fighter position with terrain",
                anchor.anchor_id
            ));
        }
    }
    Ok(())
}

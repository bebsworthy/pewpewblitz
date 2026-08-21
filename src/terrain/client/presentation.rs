//! Windowed 3D terrain presentation derived from committed client occupancy.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "terrain indices and local coordinates are bounded to 32x32 chunks"
)]

use super::ExpectedClientTerrainSlot;
use crate::client::presentation_3d::{
    Material3dAssets, Primitive3dAssets, coordinates::ground_position,
};
use crate::terrain::model::{
    MAX_TERRAIN_DEBRIS_EFFECTS, TERRAIN_CHUNK_SIDE_CELLS, TerrainBits, TerrainChunkId,
    TerrainGeneration,
};
use crate::terrain::network::{ClientTerrainConvergence, TerrainConvergencePhase};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::collections::{BTreeMap, BTreeSet};

const TERRAIN_HEIGHT: f32 = 48.0;
const TERRAIN_CANOPY_RISE: f32 = 5.0;
const TERRAIN_DEBRIS_LIFETIME: std::time::Duration = std::time::Duration::from_millis(500);
const REDUCED_TERRAIN_DEBRIS_LIFETIME: std::time::Duration = std::time::Duration::from_millis(220);
const REDUCED_TERRAIN_DEBRIS_LIMIT: usize = 16;

#[derive(Component)]
pub struct TerrainChunkVisual {
    pub chunk: TerrainChunkId,
    pub generation: TerrainGeneration,
    mesh: Handle<Mesh>,
}

#[derive(Component)]
pub(crate) struct TerrainDebris {
    generation: TerrainGeneration,
    expires_at: std::time::Duration,
}

fn orthogonal_neighbors(chunk: TerrainChunkId) -> [TerrainChunkId; 4] {
    [
        TerrainChunkId {
            x: chunk.x.saturating_sub(1),
            y: chunk.y,
        },
        TerrainChunkId {
            x: chunk.x.saturating_add(1),
            y: chunk.y,
        },
        TerrainChunkId {
            x: chunk.x,
            y: chunk.y.saturating_sub(1),
        },
        TerrainChunkId {
            x: chunk.x,
            y: chunk.y.saturating_add(1),
        },
    ]
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own committed terrain presentation state"
)]
pub(crate) fn update_terrain_visuals(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<Res<Material3dAssets>>,
    expected: Res<ExpectedClientTerrainSlot>,
    mut convergence: ResMut<ClientTerrainConvergence>,
    visuals: Query<(Entity, &TerrainChunkVisual)>,
) {
    let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref()) else {
        return;
    };
    let ExpectedClientTerrainSlot::Derived(expected) = &*expected else {
        for (entity, _) in &visuals {
            commands.entity(entity).try_despawn();
        }
        return;
    };
    let TerrainConvergencePhase::Ready { generation } = convergence.phase else {
        for (entity, _) in &visuals {
            commands.entity(entity).try_despawn();
        }
        return;
    };
    if generation != expected.generation {
        for (entity, _) in &visuals {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    let existing: BTreeMap<_, _> = visuals
        .iter()
        .map(|(entity, visual)| (visual.chunk, (entity, visual)))
        .collect();
    let expected_chunks: BTreeSet<_> = expected.layout.chunks.keys().copied().collect();
    let generation_changed = existing
        .values()
        .any(|(_, visual)| visual.generation != generation);
    let mut rebuild: BTreeSet<_> = convergence.take_dirty().into_iter().collect();
    if generation_changed || existing.is_empty() {
        rebuild.extend(expected_chunks.iter().copied());
    }
    for dirty in rebuild.iter().copied().collect::<Vec<_>>() {
        rebuild.extend(
            orthogonal_neighbors(dirty)
                .into_iter()
                .filter(|neighbor| expected_chunks.contains(neighbor)),
        );
    }

    let committed = convergence.chunks();
    for chunk in &expected_chunks {
        let bits = committed.get(chunk).copied().unwrap_or_default();
        if bits.is_empty() {
            if let Some((entity, _)) = existing.get(chunk) {
                commands.entity(*entity).try_despawn();
            }
            continue;
        }
        if let Some((entity, visual)) = existing.get(chunk) {
            if visual.generation == generation {
                if rebuild.contains(chunk)
                    && let Some(mut mesh) = meshes.get_mut(&visual.mesh)
                {
                    *mesh = build_terrain_chunk_mesh(*chunk, &bits, committed);
                }
                continue;
            }
            if let Some(mut mesh) = meshes.get_mut(&visual.mesh) {
                *mesh = build_terrain_chunk_mesh(*chunk, &bits, committed);
            }
            commands.entity(*entity).insert(TerrainChunkVisual {
                chunk: *chunk,
                generation,
                mesh: visual.mesh.clone(),
            });
            continue;
        }
        let mesh = meshes.add(build_terrain_chunk_mesh(*chunk, &bits, committed));
        commands.spawn((
            TerrainChunkVisual {
                chunk: *chunk,
                generation,
                mesh: mesh.clone(),
            },
            Mesh3d(mesh),
            MeshMaterial3d(materials.terrain.clone()),
            Transform::from_translation(ground_position(crate::terrain::grid::chunk_min_world(
                *chunk,
            ))),
            Name::new("V3 destructible terrain chunk"),
        ));
    }
    for (chunk, (entity, _)) in existing {
        if !expected_chunks.contains(&chunk) {
            commands.entity(entity).try_despawn();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own bounded terrain feedback"
)]
pub(crate) fn spawn_terrain_debris(
    mut commands: Commands,
    primitives: Option<Res<Primitive3dAssets>>,
    materials: Option<Res<Material3dAssets>>,
    settings: Option<Res<crate::client::ClientShellSettings>>,
    time: Res<Time<Virtual>>,
    mut convergence: ResMut<ClientTerrainConvergence>,
    debris: Query<(Entity, &TerrainDebris)>,
) {
    let (Some(primitives), Some(materials)) = (primitives, materials) else {
        return;
    };
    let brushes = convergence.take_applied_brushes();
    let TerrainConvergencePhase::Ready { generation } = convergence.phase else {
        return;
    };
    let reduced = settings.is_some_and(|settings| settings.reduced_combat_effects);
    let debris_limit = if reduced {
        REDUCED_TERRAIN_DEBRIS_LIMIT
    } else {
        MAX_TERRAIN_DEBRIS_EFFECTS
    };
    let mut live: Vec<_> = debris.iter().collect();
    live.sort_by_key(|(entity, _)| *entity);
    let overflow = live
        .len()
        .saturating_add(brushes.len())
        .saturating_sub(debris_limit);
    for (entity, _) in live.into_iter().take(overflow) {
        commands.entity(entity).try_despawn();
    }
    let newest = brushes.len().min(debris_limit);
    let expires_at = time.elapsed()
        + if reduced {
            REDUCED_TERRAIN_DEBRIS_LIFETIME
        } else {
            TERRAIN_DEBRIS_LIFETIME
        };
    for brush in &brushes[brushes.len() - newest..] {
        let center = crate::terrain::grid::brush_center_world(*brush);
        commands.spawn((
            TerrainDebris {
                generation,
                expires_at,
            },
            Mesh3d(primitives.debris.clone()),
            MeshMaterial3d(materials.terrain.clone()),
            bevy::light::NotShadowCaster,
            Transform::from_translation(ground_position(center) + Vec3::Y * 5.0)
                .with_scale(Vec3::splat(if reduced { 0.65 } else { 1.0 })),
            Name::new("V3 terrain debris"),
        ));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own bounded terrain feedback"
)]
pub(crate) fn expire_terrain_debris(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    convergence: Res<ClientTerrainConvergence>,
    debris: Query<(Entity, &TerrainDebris)>,
) {
    let now = time.elapsed();
    let current = match convergence.phase {
        TerrainConvergencePhase::Ready { generation } => Some(generation),
        _ => None,
    };
    for (entity, debris) in &debris {
        if now >= debris.expires_at || Some(debris.generation) != current {
            commands.entity(entity).try_despawn();
        }
    }
}

fn occupied(
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
    chunk: TerrainChunkId,
    local_x: i32,
    local_y: i32,
) -> bool {
    let side = TERRAIN_CHUNK_SIDE_CELLS as i32;
    let chunk_x = i32::from(chunk.x) + local_x.div_euclid(side);
    let chunk_y = i32::from(chunk.y) + local_y.div_euclid(side);
    let (Ok(chunk_x), Ok(chunk_y)) = (i16::try_from(chunk_x), i16::try_from(chunk_y)) else {
        return false;
    };
    chunks
        .get(&TerrainChunkId {
            x: chunk_x,
            y: chunk_y,
        })
        .is_some_and(|bits| {
            bits.get(
                local_x.rem_euclid(side) as u32,
                local_y.rem_euclid(side) as u32,
            )
        })
}

#[must_use]
pub fn build_terrain_chunk_mesh(
    chunk: TerrainChunkId,
    bits: &TerrainBits,
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
) -> Mesh {
    let cell = crate::terrain::model::TERRAIN_CELL_SIZE_WORLD;
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();
    for (x, y) in bits.iter_occupied() {
        let x0 = x as f32 * cell;
        let x1 = x0 + cell;
        let z0 = -(y as f32 * cell);
        let z1 = z0 - cell;
        let chunk_side = TERRAIN_CHUNK_SIDE_CELLS as i32;
        let global_x = i32::from(chunk.x) * chunk_side + x as i32;
        let global_y = i32::from(chunk.y) * chunk_side + y as i32;
        let h00 = terrain_canopy_height(global_x, global_y);
        let h10 = terrain_canopy_height(global_x + 1, global_y);
        let h11 = terrain_canopy_height(global_x + 1, global_y + 1);
        let h01 = terrain_canopy_height(global_x, global_y + 1);
        let center_height = (h00 + h10 + h11 + h01) * 0.25 + TERRAIN_CANOPY_RISE;
        let north_west = [x0, h00, z0];
        let north_east = [x1, h10, z0];
        let south_east = [x1, h11, z1];
        let south_west = [x0, h01, z1];
        let center = [(x0 + x1) * 0.5, center_height, (z0 + z1) * 0.5];
        for triangle in [
            [north_west, north_east, center],
            [north_east, south_east, center],
            [south_east, south_west, center],
            [south_west, north_west, center],
        ] {
            add_triangle(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut indices,
                triangle,
            );
        }
        if !occupied(chunks, chunk, x as i32 - 1, y as i32) {
            add_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut indices,
                [[x0, 0.0, z1], [x0, 0.0, z0], north_west, south_west],
                [-1.0, 0.0, 0.0],
            );
        }
        if !occupied(chunks, chunk, x as i32 + 1, y as i32) {
            add_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut indices,
                [[x1, 0.0, z0], [x1, 0.0, z1], south_east, north_east],
                [1.0, 0.0, 0.0],
            );
        }
        if !occupied(chunks, chunk, x as i32, y as i32 - 1) {
            add_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut indices,
                [[x0, 0.0, z0], [x1, 0.0, z0], north_east, north_west],
                [0.0, 0.0, 1.0],
            );
        }
        if !occupied(chunks, chunk, x as i32, y as i32 + 1) {
            add_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut indices,
                [[x1, 0.0, z1], [x0, 0.0, z1], south_west, south_east],
                [0.0, 0.0, -1.0],
            );
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn terrain_canopy_height(global_x: i32, global_y: i32) -> f32 {
    let hash = global_x
        .wrapping_mul(73_856_093)
        .wrapping_add(global_y.wrapping_mul(19_349_663))
        ^ 0x045D_9F3B;
    let variation = (hash.unsigned_abs() % 9) as f32 - 4.0;
    TERRAIN_HEIGHT + variation * 0.7
}

fn add_triangle(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    vertices: [[f32; 3]; 3],
) {
    let a = Vec3::from_array(vertices[0]);
    let b = Vec3::from_array(vertices[1]);
    let c = Vec3::from_array(vertices[2]);
    let normal = (b - a).cross(c - a).normalize_or_zero().to_array();
    let base = u32::try_from(positions.len()).expect("one terrain chunk mesh fits u32 indices");
    positions.extend(vertices);
    normals.extend([normal; 3]);
    uvs.extend([[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]]);
    indices.extend([base, base + 1, base + 2]);
}

fn add_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    vertices: [[f32; 3]; 4],
    normal: [f32; 3],
) {
    let base = u32::try_from(positions.len()).expect("one terrain chunk mesh fits u32 indices");
    positions.extend(vertices);
    normals.extend([normal; 4]);
    uvs.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_terrain_cell_emits_faceted_canopy_and_four_exposed_sides() {
        let chunk = TerrainChunkId::default();
        let mut bits = TerrainBits::default();
        bits.set(0, 0);
        let chunks = BTreeMap::from([(chunk, bits)]);
        let mesh = build_terrain_chunk_mesh(chunk, &bits, &chunks);
        assert_eq!(mesh.count_vertices(), 28);
        assert_eq!(mesh.indices().map(Indices::len), Some(36));
    }

    #[test]
    fn adjacent_cells_and_cross_chunk_neighbors_remove_internal_sides() {
        let west = TerrainChunkId::default();
        let east = TerrainChunkId { x: 1, y: 0 };
        let mut west_bits = TerrainBits::default();
        west_bits.set(TERRAIN_CHUNK_SIDE_CELLS - 1, 0);
        let mut east_bits = TerrainBits::default();
        east_bits.set(0, 0);
        let chunks = BTreeMap::from([(west, west_bits), (east, east_bits)]);
        let mesh = build_terrain_chunk_mesh(west, &west_bits, &chunks);
        assert_eq!(mesh.count_vertices(), 24, "east seam face is hidden");
    }

    #[test]
    fn terrain_canopy_height_is_bounded_varied_and_world_stable() {
        let samples = [
            terrain_canopy_height(0, 0),
            terrain_canopy_height(1, 0),
            terrain_canopy_height(0, 1),
            terrain_canopy_height(-1, 0),
        ];

        assert!(
            samples
                .iter()
                .all(|height| { (TERRAIN_HEIGHT - 2.8..=TERRAIN_HEIGHT + 2.8).contains(height) })
        );
        assert!(
            samples
                .windows(2)
                .any(|pair| (pair[0] - pair[1]).abs() > f32::EPSILON)
        );
        assert!((samples[0] - terrain_canopy_height(0, 0)).abs() <= f32::EPSILON);
    }
}

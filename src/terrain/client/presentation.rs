//! Windowed terrain presentation: chunk images, sprites, and bounded debris.
//!
//! Everything here is derived exclusively from committed convergence occupancy. The
//! headless composition simply lacks `Assets<Image>` and skips all of it.

use crate::map::MapInstanceId;
use crate::terrain::model::{
    MAX_TERRAIN_DEBRIS_EFFECTS, TERRAIN_CHUNK_SIDE_CELLS, TerrainBits, TerrainChunkId,
    TerrainGeneration,
};
use crate::terrain::network::{ClientTerrainConvergence, TerrainConvergencePhase};
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::{BTreeMap, BTreeSet};

use super::ExpectedClientTerrainSlot;

/// Presentation depth: above the floor, below spawn areas, the Hot Zone objective, and
/// every dynamic entity.
pub const TERRAIN_PRESENTATION_Z: f32 = -6.0;

/// Opaque interior rock, distinct from the dark floor and the blue permanent walls.
pub(crate) const TERRAIN_FILL_PIXEL: [u8; 4] = [112, 96, 74, 255];
/// Brighter rim for occupied cells beside an empty neighbor or an open seam.
pub(crate) const TERRAIN_EDGE_PIXEL: [u8; 4] = [186, 158, 112, 255];
/// Cosmetic debris lifetime in client presentation time.
const TERRAIN_DEBRIS_LIFETIME: std::time::Duration = std::time::Duration::from_millis(500);

/// One retained per-chunk visual: a nearest-sampled 32x32 image sprite scaled to one
/// 256x256 world-unit chunk.
#[derive(Component)]
pub struct TerrainChunkVisual {
    pub chunk: TerrainChunkId,
    pub map_instance_id: MapInstanceId,
    pub(super) image: Handle<Image>,
}

/// One bounded cosmetic destruction burst. Never collides, replicates, or plays audio.
/// Carries its terrain generation so a reset, map replacement, or disconnect despawns it
/// immediately instead of outliving its generation by the presentation timer.
#[derive(Component)]
pub(crate) struct TerrainDebris {
    generation: TerrainGeneration,
    expires_at: std::time::Duration,
}

/// Paint one chunk's 32x32 RGBA rows from occupancy plus the orthogonal neighbors that
/// decide crater-edge colors across seams. Image rows run top-down; cell y grows up.
#[must_use]
pub fn paint_chunk_pixels(
    bits: &TerrainBits,
    west: Option<&TerrainBits>,
    east: Option<&TerrainBits>,
    north: Option<&TerrainBits>,
    south: Option<&TerrainBits>,
) -> Vec<u8> {
    let side = TERRAIN_CHUNK_SIDE_CELLS;
    let mut data = vec![0_u8; (side * side * 4) as usize];
    for local_y in 0..side {
        for local_x in 0..side {
            if !bits.get(local_x, local_y) {
                continue;
            }
            let east_empty = if local_x + 1 < side {
                !bits.get(local_x + 1, local_y)
            } else {
                east.is_none_or(|neighbor| !neighbor.get(0, local_y))
            };
            let west_empty = if local_x > 0 {
                !bits.get(local_x - 1, local_y)
            } else {
                west.is_none_or(|neighbor| !neighbor.get(side - 1, local_y))
            };
            let north_empty = if local_y + 1 < side {
                !bits.get(local_x, local_y + 1)
            } else {
                north.is_none_or(|neighbor| !neighbor.get(local_x, 0))
            };
            let south_empty = if local_y > 0 {
                !bits.get(local_x, local_y - 1)
            } else {
                south.is_none_or(|neighbor| !neighbor.get(local_x, side - 1))
            };
            let pixel = if east_empty || west_empty || north_empty || south_empty {
                TERRAIN_EDGE_PIXEL
            } else {
                TERRAIN_FILL_PIXEL
            };
            let row = side - 1 - local_y;
            let index = ((row * side + local_x) * 4) as usize;
            data[index..index + 4].copy_from_slice(&pixel);
        }
    }
    data
}

/// Build the tiny nearest-sampled chunk image from painted pixel rows.
pub(crate) fn chunk_image(data: Vec<u8>) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: TERRAIN_CHUNK_SIDE_CELLS,
            height: TERRAIN_CHUNK_SIDE_CELLS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    image.data = Some(data);
    image.sampler = ImageSampler::nearest();
    image
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

fn neighbor_bits(
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
    neighbor: TerrainChunkId,
) -> Option<&TerrainBits> {
    chunks.get(&neighbor)
}

/// Ensure one sprite per expected chunk, repaint dirty chunks and their orthogonal
/// visual neighbors, and retire sprites that left the expected generation.
pub(crate) fn update_terrain_visuals(
    mut commands: Commands,
    mut images: Option<ResMut<Assets<Image>>>,
    expected: Res<ExpectedClientTerrainSlot>,
    mut convergence: ResMut<ClientTerrainConvergence>,
    visuals: Query<(Entity, &TerrainChunkVisual)>,
) {
    let Some(images) = images.as_deref_mut() else {
        return;
    };
    let ExpectedClientTerrainSlot::Derived(expected) = &*expected else {
        for (entity, _) in &visuals {
            commands.entity(entity).try_despawn();
        }
        return;
    };
    if matches!(
        convergence.phase,
        TerrainConvergencePhase::WaitingForMap | TerrainConvergencePhase::Invalid { .. }
    ) {
        for (entity, _) in &visuals {
            commands.entity(entity).try_despawn();
        }
        return;
    }
    let mut repaint: BTreeSet<TerrainChunkId> = convergence.take_dirty().into_iter().collect();
    let committed = convergence.chunks();
    let expected_chunks: BTreeSet<_> = expected.layout.chunks.keys().copied().collect();
    for chunk in repaint.iter().copied().collect::<Vec<_>>() {
        for neighbor in orthogonal_neighbors(chunk) {
            if expected_chunks.contains(&neighbor) {
                repaint.insert(neighbor);
            }
        }
    }
    let existing: BTreeMap<_, _> = visuals
        .iter()
        .map(|(entity, visual)| {
            (
                visual.chunk,
                (entity, visual.map_instance_id, visual.image.clone()),
            )
        })
        .collect();
    for chunk in &expected_chunks {
        let Some((entity, instance, handle)) = existing.get(chunk) else {
            let bits = committed.get(chunk).copied().unwrap_or_default();
            let data = paint_chunk_pixels(
                &bits,
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[0]),
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[1]),
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[2]),
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[3]),
            );
            let handle = images.add(chunk_image(data));
            let min = crate::terrain::grid::chunk_min_world(*chunk);
            let center = min + Vec2::splat(crate::terrain::TERRAIN_CHUNK_SIDE_WORLD * 0.5);
            commands.spawn((
                TerrainChunkVisual {
                    chunk: *chunk,
                    map_instance_id: expected.generation.map_instance_id,
                    image: handle.clone(),
                },
                Sprite {
                    image: handle,
                    custom_size: Some(Vec2::splat(crate::terrain::TERRAIN_CHUNK_SIDE_WORLD)),
                    ..default()
                },
                Transform::from_translation(center.extend(TERRAIN_PRESENTATION_Z)),
            ));
            continue;
        };
        if instance != &expected.generation.map_instance_id {
            commands.entity(*entity).try_despawn();
            continue;
        }
        if repaint.contains(chunk) {
            let bits = committed.get(chunk).copied().unwrap_or_default();
            let data = paint_chunk_pixels(
                &bits,
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[0]),
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[1]),
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[2]),
                neighbor_bits(committed, orthogonal_neighbors(*chunk)[3]),
            );
            if let Some(mut image) = images.get_mut(handle) {
                image.data = Some(data);
            }
        }
    }
    for (chunk, (entity, _, _)) in &existing {
        if !expected_chunks.contains(chunk) {
            commands.entity(*entity).try_despawn();
        }
    }
}

pub(crate) fn spawn_terrain_debris(
    mut commands: Commands,
    images: Option<ResMut<Assets<Image>>>,
    time: Res<Time<Virtual>>,
    mut convergence: ResMut<ClientTerrainConvergence>,
    debris: Query<(Entity, &Transform), With<TerrainDebris>>,
) {
    if images.as_deref().is_none() {
        return;
    }
    let brushes = convergence.take_applied_brushes();
    let TerrainConvergencePhase::Ready { generation } = convergence.phase else {
        return;
    };
    // Budget the ceiling across live debris plus this tick's applied brushes, keeping
    // the newest feedback: retire the oldest existing effects first and, when a single
    // burst exceeds the ceiling on its own, present only its newest brushes.
    let mut live: Vec<_> = debris.iter().collect();
    live.sort_by_key(|(entity, _)| *entity);
    let overflow = live
        .len()
        .saturating_add(brushes.len())
        .saturating_sub(MAX_TERRAIN_DEBRIS_EFFECTS);
    for _ in 0..overflow.min(live.len()) {
        let (expire, _) = live.remove(0);
        commands.entity(expire).try_despawn();
    }
    let newest = brushes.len().min(MAX_TERRAIN_DEBRIS_EFFECTS);
    let expires_at = time.elapsed() + TERRAIN_DEBRIS_LIFETIME;
    for brush in &brushes[brushes.len() - newest..] {
        let center = crate::terrain::grid::brush_center_world(*brush);
        commands.spawn((
            TerrainDebris {
                generation,
                expires_at,
            },
            Sprite::from_color(
                Color::srgba(0.85, 0.66, 0.34, 0.85),
                Vec2::splat(
                    f32::from(brush.radius_half_cells)
                        * crate::terrain::model::TERRAIN_SUBCELL_SIZE_WORLD
                        * 0.5,
                ),
            ),
            Transform::from_translation(center.extend(TERRAIN_PRESENTATION_Z + 2.0)),
        ));
    }
}

/// Expire debris by client presentation time and immediately retire any debris whose
/// terrain generation left the convergence machine (reset, map replacement, or
/// disconnect); the durable crater stays.
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

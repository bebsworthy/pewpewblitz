//! Client terrain convergence: derive the expected generation from replicated map and
//! match state, request and apply recovery, and process live terrain traffic.
//!
//! This module owns no gameplay collision. Occupancy committed here is presentation
//! input; images and debris arrive with the windowed composition.
#![allow(
    clippy::needless_pass_by_value,
    reason = "system parameters follow the sibling client modules' shared-resource style"
)]

use super::TerrainCorePlugin;
use super::model::{
    TERRAIN_CHUNK_SIDE_CELLS, TerrainBits, TerrainChunkId, TerrainDestructionEvent,
    TerrainGeneration, TerrainRecoveryRequest, TerrainRecoverySnapshot, TerrainResetEvent,
};
use super::network::{ClientTerrainConvergence, TerrainConvergenceAction, TerrainConvergencePhase};
use super::telemetry::{TerrainTelemetry, TerrainTelemetryOutcome, TerrainTelemetryRecord};
use crate::map::{InitialTerrainLayout, MapInstanceId, MapRoot, ResolvedMapSnapshot};
use crate::matchplay::{MatchId, MatchRoot as MatchRootMarker, MatchState};
use crate::protocol::TerrainChannel;
use crate::timing::SimulationTick;
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Disconnected};
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::{BTreeMap, BTreeSet};

/// User-facing terrain synchronization state derived from the pure convergence phase.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientTerrainReadiness {
    #[default]
    WaitingForMap,
    SyncingTerrain,
    RecoveringTerrain,
    Ready,
    Invalid(String),
}

/// One locally derived expectation from the replicated map and match state.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExpectedClientTerrain {
    pub(super) generation: TerrainGeneration,
    pub(super) layout: InitialTerrainLayout,
    pub(super) derived_from: (MapInstanceId, MatchId),
}

/// Derivation cache so layout resolution runs only when the replicated pair changes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(super) enum ExpectedClientTerrainSlot {
    #[default]
    Waiting,
    Failed(String),
    Derived(ExpectedClientTerrain),
}

pub struct ClientTerrainPlugin;

impl Plugin for ClientTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TerrainCorePlugin)
            .init_resource::<ClientTerrainReadiness>()
            .init_resource::<ExpectedClientTerrainSlot>()
            .init_resource::<TerrainTelemetry>()
            .add_systems(
                Update,
                (
                    derive_expected_client_terrain,
                    drive_terrain_wire_convergence,
                    clear_terrain_convergence_on_disconnect,
                    update_terrain_visuals,
                    spawn_terrain_debris,
                    expire_terrain_debris,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, gate_inputs_on_terrain_readiness);
    }
}

/// Observe the replicated map snapshot and match state, re-derive the expected terrain
/// generation when either changes, and request recovery through the pure machine.
fn derive_expected_client_terrain(
    mut convergence: ResMut<ClientTerrainConvergence>,
    mut expected: ResMut<ExpectedClientTerrainSlot>,
    snapshots: Query<&ResolvedMapSnapshot, With<MapRoot>>,
    matches: Query<&MatchState, With<MatchRootMarker>>,
) {
    let derived = match (snapshots.single(), matches.single()) {
        (Ok(snapshot), Ok(match_state)) => {
            let pair = (snapshot.identity.instance_id, match_state.match_id);
            if let ExpectedClientTerrainSlot::Derived(current) = &*expected
                && current.derived_from == pair
            {
                return;
            }
            let layout = crate::map::resolve_initial_terrain(
                snapshot.playable_bounds,
                &snapshot.geometry,
                &snapshot.regions,
                &snapshot.spawn_points,
                &snapshot.mode_anchors,
                crate::map::EngineMapLimits::default(),
            );
            Some((pair, layout))
        }
        _ => None,
    };
    let derived = match derived {
        Some((pair, Ok(layout))) => ExpectedClientTerrain {
            generation: TerrainGeneration {
                map_instance_id: pair.0,
                match_id: pair.1,
                terrain_fingerprint: layout.terrain_fingerprint,
            },
            layout,
            derived_from: pair,
        },
        Some((_, Err(_))) => {
            if *expected != ExpectedClientTerrainSlot::Waiting {
                convergence.clear();
            }
            *expected = ExpectedClientTerrainSlot::Failed(
                "replicated map snapshot failed terrain layout validation".to_string(),
            );
            return;
        }
        None => {
            if *expected != ExpectedClientTerrainSlot::Waiting {
                convergence.clear();
                *expected = ExpectedClientTerrainSlot::Waiting;
            }
            return;
        }
    };
    let _ = convergence.observe_generation(derived.generation, &derived.layout.chunks);
    *expected = ExpectedClientTerrainSlot::Derived(derived);
}

/// Silent ticks after one recovery request before the client re-arms it. Doubles the
/// server's per-link cooldown so a served exchange is never double-counted.
const RECOVERY_REQUEST_RETRY_TICKS: u64 = 60;

/// One client-local convergence telemetry record: only the tick, generation identity,
/// and revision it observed are meaningful.
fn client_convergence_record(
    tick: u64,
    generation: TerrainGeneration,
    revision: u64,
    outcome: TerrainTelemetryOutcome,
) -> TerrainTelemetryRecord {
    TerrainTelemetryRecord {
        tick,
        map_instance_id: generation.map_instance_id,
        revision,
        source_attack_id: None,
        delivery_index: None,
        brush: None,
        affected_chunks: Vec::new(),
        erased_cells: 0,
        rebuilt_colliders: 0,
        serialized_event_bytes: None,
        outcome,
    }
}

/// Record the convergence facts the pure machine signals only through its action: a
/// duplicate revision and a revision gap observed from an already-committed state.
pub(super) fn classify_client_event(
    convergence: &ClientTerrainConvergence,
    event: &TerrainDestructionEvent,
    tick: u64,
    telemetry: &mut TerrainTelemetry,
) {
    let TerrainConvergencePhase::Ready { generation } = convergence.phase else {
        return;
    };
    if event.generation != generation {
        return;
    }
    let committed = convergence.revision();
    let outcome = if event.revision <= committed {
        TerrainTelemetryOutcome::ClientDuplicateIgnored
    } else if event.revision > committed.saturating_add(1) {
        TerrainTelemetryOutcome::ClientGapObserved
    } else {
        return;
    };
    telemetry.record(client_convergence_record(
        tick,
        generation,
        event.revision,
        outcome,
    ));
}

/// Record one applied recovery snapshot against the convergence machine's committed
/// generation. Called only after `apply_snapshot` committed new authoritative state.
pub(super) fn record_snapshot_application(
    convergence: &ClientTerrainConvergence,
    snapshot_revision: u64,
    tick: u64,
    telemetry: &mut TerrainTelemetry,
) {
    if let TerrainConvergencePhase::Ready { generation } = convergence.phase {
        telemetry.record(client_convergence_record(
            tick,
            generation,
            snapshot_revision,
            TerrainTelemetryOutcome::ClientSnapshotApplied,
        ));
    }
}

/// Receive terrain traffic, drive the pure convergence machine, and send at most one
/// outstanding recovery request for the awaited generation.
#[allow(clippy::too_many_arguments)]
fn drive_terrain_wire_convergence(
    tick: Option<Res<SimulationTick>>,
    mut last_request_tick: Local<Option<u64>>,
    expected: Res<ExpectedClientTerrainSlot>,
    mut convergence: ResMut<ClientTerrainConvergence>,
    mut telemetry: ResMut<TerrainTelemetry>,
    mut readiness: ResMut<ClientTerrainReadiness>,
    mut requests: Query<&mut MessageSender<TerrainRecoveryRequest>, With<Client>>,
    mut snapshots: Query<Option<&mut MessageReceiver<TerrainRecoverySnapshot>>, With<Client>>,
    mut resets: Query<Option<&mut MessageReceiver<TerrainResetEvent>>, With<Client>>,
    mut events: Query<Option<&mut MessageReceiver<TerrainDestructionEvent>>, With<Client>>,
) {
    let observed = match &*expected {
        ExpectedClientTerrainSlot::Derived(current) => Some(current.generation),
        _ => None,
    };
    let empty = BTreeMap::new();
    let initial_chunks = match &*expected {
        ExpectedClientTerrainSlot::Derived(current) => &current.layout.chunks,
        _ => &empty,
    };
    let tick = tick.map_or(0, |tick| tick.0);
    for receiver in &mut snapshots {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for snapshot in receiver.receive() {
            let action = convergence.apply_snapshot(&snapshot, initial_chunks);
            if action == TerrainConvergenceAction::Applied {
                record_snapshot_application(&convergence, snapshot.revision, tick, &mut telemetry);
            }
            report_invalid(action);
        }
    }
    for receiver in &mut resets {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for reset in receiver.receive() {
            report_invalid(convergence.apply_reset(reset, observed, initial_chunks));
        }
    }
    for receiver in &mut events {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for event in receiver.receive() {
            classify_client_event(&convergence, &event, tick, &mut telemetry);
            report_invalid(convergence.apply_event(event));
        }
    }
    // One outstanding request, re-armed after a bounded silent window so a lost request
    // or response on an unreliable transport cannot wedge convergence forever.
    let tick = SimulationTick(tick);
    let mut resend = false;
    match convergence.phase {
        TerrainConvergencePhase::AwaitingRecovery {
            generation,
            request_pending: false,
            ..
        } => {
            for mut sender in &mut requests {
                sender.send::<TerrainChannel>(TerrainRecoveryRequest { generation });
            }
            convergence.mark_request_sent();
            *last_request_tick = Some(tick.0);
        }
        TerrainConvergencePhase::AwaitingRecovery {
            request_pending: true,
            ..
        } => {
            resend = last_request_tick
                .is_some_and(|sent| tick.0.saturating_sub(sent) >= RECOVERY_REQUEST_RETRY_TICKS);
        }
        _ => {}
    }
    if resend
        && let TerrainConvergencePhase::AwaitingRecovery { generation, .. } = convergence.phase
    {
        for mut sender in &mut requests {
            sender.send::<TerrainChannel>(TerrainRecoveryRequest { generation });
        }
        *last_request_tick = Some(tick.0);
    }
    refresh_terrain_readiness(&mut readiness, &convergence, &expected);
}

/// Derive the user-facing readiness observation from the committed convergence phase.
/// Runs after the Update-stage readiness writers so the clamp is authoritative for the
/// next sampled frame.
fn refresh_terrain_readiness(
    readiness: &mut ClientTerrainReadiness,
    convergence: &ClientTerrainConvergence,
    expected: &ExpectedClientTerrainSlot,
) {
    let was_ready = matches!(*readiness, ClientTerrainReadiness::Ready);
    *readiness = match &convergence.phase {
        TerrainConvergencePhase::WaitingForMap => match expected {
            ExpectedClientTerrainSlot::Failed(reason) => {
                ClientTerrainReadiness::Invalid(reason.clone())
            }
            _ => ClientTerrainReadiness::WaitingForMap,
        },
        TerrainConvergencePhase::AwaitingRecovery {
            request_pending, ..
        } => {
            if *request_pending {
                ClientTerrainReadiness::RecoveringTerrain
            } else {
                ClientTerrainReadiness::SyncingTerrain
            }
        }
        TerrainConvergencePhase::Ready { generation } => {
            if !was_ready {
                info!(
                    map_instance = generation.map_instance_id.0,
                    revision = convergence.revision(),
                    "client terrain converged to authoritative state"
                );
            }
            ClientTerrainReadiness::Ready
        }
        TerrainConvergencePhase::Invalid { reason, .. } => {
            ClientTerrainReadiness::Invalid(reason.clone())
        }
    };
}

fn report_invalid(action: TerrainConvergenceAction) {
    if let TerrainConvergenceAction::Invalidated(reason) = action {
        warn!(
            reason,
            "client terrain convergence entered an invalid state"
        );
    }
}

/// Disconnect clears every generation-scoped convergence state, including `Invalid`.
fn clear_terrain_convergence_on_disconnect(
    clients: Query<(), (With<Client>, Without<Disconnected>)>,
    mut convergence: ResMut<ClientTerrainConvergence>,
) {
    if !clients.is_empty() {
        return;
    }
    if convergence.phase != TerrainConvergencePhase::WaitingForMap {
        convergence.clear();
    }
}

// ---------------------------------------------------------------------------
// Windowed presentation: chunk images, sprites, and bounded debris.
//
// Everything below is derived exclusively from committed convergence occupancy. The
// headless composition simply lacks `Assets<Image>` and skips all of it.
// ---------------------------------------------------------------------------

use bevy::image::{Image, ImageSampler};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Presentation depth: above the floor, below spawn areas, the Hot Zone objective, and
/// every dynamic entity.
pub const TERRAIN_PRESENTATION_Z: f32 = -6.0;

/// Opaque interior rock, distinct from the dark floor and the blue permanent walls.
const TERRAIN_FILL_PIXEL: [u8; 4] = [112, 96, 74, 255];
/// Brighter rim for occupied cells beside an empty neighbor or an open seam.
const TERRAIN_EDGE_PIXEL: [u8; 4] = [186, 158, 112, 255];
/// Cosmetic debris lifetime in client presentation time.
const TERRAIN_DEBRIS_LIFETIME: std::time::Duration = std::time::Duration::from_millis(500);

/// One retained per-chunk visual: a nearest-sampled 32x32 image sprite scaled to one
/// 256x256 world-unit chunk.
#[derive(Component)]
pub struct TerrainChunkVisual {
    pub chunk: TerrainChunkId,
    pub map_instance_id: MapInstanceId,
    image: Handle<Image>,
}

/// One bounded cosmetic destruction burst. Never collides, replicates, or plays audio.
/// Carries its terrain generation so a reset, map replacement, or disconnect despawns it
/// immediately instead of outliving its generation by the presentation timer.
#[derive(Component)]
pub(super) struct TerrainDebris {
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
pub(super) fn update_terrain_visuals(
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
pub(super) fn spawn_terrain_debris(
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
        .saturating_sub(super::model::MAX_TERRAIN_DEBRIS_EFFECTS);
    for _ in 0..overflow.min(live.len()) {
        let (expire, _) = live.remove(0);
        commands.entity(expire).try_despawn();
    }
    let newest = brushes.len().min(super::model::MAX_TERRAIN_DEBRIS_EFFECTS);
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
                        * super::model::TERRAIN_SUBCELL_SIZE_WORLD
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
pub(super) fn expire_terrain_debris(
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

/// Terrain is the fourth readiness observation: inputs stay suppressed until convergence
/// reports the matching generation committed. Runs after the Update-stage readiness
/// writers so the clamp is authoritative for the next sampled frame.
fn gate_inputs_on_terrain_readiness(
    readiness: Res<ClientTerrainReadiness>,
    config: Option<Res<crate::config::ClientNetworkConfig>>,
    joins: Query<&crate::client::ClientJoinStatus>,
    mut playable: ResMut<crate::client::ClientPlayableGate>,
    mut suppressed: Local<bool>,
) {
    if !matches!(&*readiness, ClientTerrainReadiness::Ready) {
        *suppressed = true;
        playable.0 = false;
        return;
    }
    if !*suppressed {
        return;
    }
    *suppressed = false;
    // The windowed composition recomputes the full playable formula every Update; the
    // headless composition has no asset writer, so this gate restores it directly once
    // an accepted client's terrain has converged.
    let headless = config.is_none_or(|config| config.headless);
    if headless
        && joins
            .iter()
            .any(|status| matches!(status.phase, crate::client::ClientJoinPhase::Active { .. }))
    {
        playable.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matchplay::MatchPhase;
    use crate::terrain::grid as terrain_grid;
    use crate::terrain::network::TerrainConvergenceAction;

    fn full_chunk(id: TerrainChunkId) -> (TerrainChunkId, TerrainBits) {
        let mut bits = TerrainBits::default();
        for local_y in 0..TERRAIN_CHUNK_SIDE_CELLS {
            for local_x in 0..TERRAIN_CHUNK_SIDE_CELLS {
                bits.set(local_x, local_y);
            }
        }
        (id, bits)
    }

    /// Row-major pixel lookup matching the painted top-down image rows.
    fn pixel(data: &[u8], local_x: u32, local_y: u32) -> [u8; 4] {
        let row = TERRAIN_CHUNK_SIDE_CELLS - 1 - local_y;
        let index = ((row * TERRAIN_CHUNK_SIDE_CELLS + local_x) * 4) as usize;
        [
            data[index],
            data[index + 1],
            data[index + 2],
            data[index + 3],
        ]
    }

    #[test]
    fn intact_chunk_paints_fill_interior_and_edge_rim() {
        let (_, bits) = full_chunk(TerrainChunkId { x: 0, y: 0 });
        let data = paint_chunk_pixels(&bits, None, None, None, None);
        assert_eq!(pixel(&data, 16, 16), TERRAIN_FILL_PIXEL, "interior fills");
        // Every boundary cell borders the unallocated outside, so it rims.
        assert_eq!(pixel(&data, 0, 0), TERRAIN_EDGE_PIXEL);
        assert_eq!(pixel(&data, 31, 16), TERRAIN_EDGE_PIXEL);
    }

    #[test]
    fn cross_seam_edges_follow_the_neighbor_occupancy() {
        let (_, west_bits) = full_chunk(TerrainChunkId { x: 0, y: 0 });
        let (_, east_bits) = full_chunk(TerrainChunkId { x: 1, y: 0 });
        let data = paint_chunk_pixels(&west_bits, None, Some(&east_bits), None, None);
        assert_eq!(
            pixel(&data, 31, 16),
            TERRAIN_FILL_PIXEL,
            "a solid east neighbor leaves the seam cell as interior"
        );
        let mut carved = east_bits;
        carved.clear(0, 16);
        let data = paint_chunk_pixels(&west_bits, None, Some(&carved), None, None);
        assert_eq!(
            pixel(&data, 31, 16),
            TERRAIN_EDGE_PIXEL,
            "erasing the neighbor cell rims the seam cell"
        );
    }

    #[test]
    fn empty_chunk_and_erased_crater_paint_transparent_holes() {
        let data = paint_chunk_pixels(&TerrainBits::default(), None, None, None, None);
        assert!(data.iter().all(|byte| *byte == 0));
        let (_, bits) = full_chunk(TerrainChunkId { x: 0, y: 0 });
        let mut chunks = BTreeMap::from([full_chunk(TerrainChunkId { x: 0, y: 0 })]);
        let brush = crate::terrain::TerrainBrush {
            center_half_cells_x: 1,
            center_half_cells_y: 1,
            radius_half_cells: 2,
        };
        let _ = terrain_grid::apply_brush(&mut chunks, brush);
        let data = paint_chunk_pixels(
            &chunks[&TerrainChunkId { x: 0, y: 0 }],
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            pixel(&data, 0, 0),
            [0, 0, 0, 0],
            "the crater core is a hole"
        );
        let _ = bits;
    }

    #[test]
    fn chunk_images_are_tiny_nearest_sampled_rgba8_quads() {
        let image = chunk_image(vec![
            7_u8;
            (TERRAIN_CHUNK_SIDE_CELLS * TERRAIN_CHUNK_SIDE_CELLS * 4)
                as usize
        ]);
        assert_eq!(
            image.texture_descriptor.size.width,
            TERRAIN_CHUNK_SIDE_CELLS
        );
        assert_eq!(
            image.texture_descriptor.size.height,
            TERRAIN_CHUNK_SIDE_CELLS
        );
        assert!(matches!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        ));
        assert!(
            matches!(&image.sampler, ImageSampler::Descriptor(descriptor)
            if descriptor.mag_filter == bevy::image::ImageFilterMode::Nearest)
        );
    }

    /// A client app whose replicated map and match state drive the same three-chunk
    /// expectation the convergence machine commits for these tests.
    fn visual_app() -> (
        App,
        TerrainGeneration,
        BTreeMap<TerrainChunkId, TerrainBits>,
    ) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .insert_resource(crate::client::ClientPlayableGate(true))
            .add_plugins(ClientTerrainPlugin);

        let resolved = crate::map::MapContentCatalog::embedded()
            .expect("embedded map catalog")
            .resolve_preset(
                crate::map::MapPresetId(1),
                MapInstanceId(1),
                &crate::map::MapLayoutRequirements::wipeout(),
            )
            .expect("preset resolves");
        let snapshot = resolved.snapshot;
        let rules_revision = snapshot.recipe_schema_version;
        let mode_definition_id = snapshot.mode_definition_id;
        let layout = crate::map::resolve_initial_terrain(
            snapshot.playable_bounds,
            &snapshot.geometry,
            &snapshot.regions,
            &snapshot.spawn_points,
            &snapshot.mode_anchors,
            crate::map::EngineMapLimits::default(),
        )
        .expect("preset terrain layout resolves");
        let generation = TerrainGeneration {
            map_instance_id: MapInstanceId(1),
            match_id: MatchId(1),
            terrain_fingerprint: layout.terrain_fingerprint,
        };
        let world = app.world_mut();
        // A bare client marker keeps the disconnect-clear system from treating this
        // single-app fixture as an already-disconnected peer.
        world.spawn(Client);
        world.spawn((
            MapRoot,
            snapshot.identity.instance_id,
            snapshot.identity,
            snapshot,
        ));
        world.spawn((
            MatchRootMarker,
            MatchState {
                match_id: MatchId(1),
                mode_definition_id,
                phase: MatchPhase::Waiting,
                rules_revision,
            },
        ));
        // Commit an authoritative snapshot for the exact derived generation so the
        // presentation works from committed occupancy like a recovered client.
        {
            let mut convergence = world.resource_mut::<ClientTerrainConvergence>();
            assert!(matches!(
                convergence.observe_generation(generation, &layout.chunks),
                TerrainConvergenceAction::RequestRecovery(_)
            ));
            convergence.mark_request_sent();
            assert_eq!(
                convergence.apply_snapshot(
                    &terrain_grid::recovery_snapshot(&layout.chunks, generation, 0),
                    &layout.chunks
                ),
                TerrainConvergenceAction::Applied
            );
            convergence.take_dirty();
        }
        (app, generation, layout.chunks)
    }

    #[test]
    fn visuals_spawn_one_sprite_per_expected_chunk_with_terrain_depth() {
        let (mut app, _, layout_chunks) = visual_app();
        app.update();
        let world = app.world_mut();
        let mut visuals = world.query::<(&TerrainChunkVisual, &Transform, &Sprite)>();
        let mut count = 0;
        for (visual, transform, sprite) in visuals.iter(world) {
            count += 1;
            assert_eq!(
                sprite.custom_size,
                Some(Vec2::splat(crate::terrain::TERRAIN_CHUNK_SIDE_WORLD))
            );
            assert!((transform.translation.z - TERRAIN_PRESENTATION_Z).abs() <= f32::EPSILON);
            let min = terrain_grid::chunk_min_world(visual.chunk);
            assert_eq!(
                transform.translation.truncate(),
                min + Vec2::splat(crate::terrain::TERRAIN_CHUNK_SIDE_WORLD * 0.5)
            );
        }
        assert_eq!(count, layout_chunks.len());
        // Ready terrain leaves the pre-set playable gate untouched.
        assert!(world.resource::<crate::client::ClientPlayableGate>().0);
    }

    #[test]
    fn applied_brush_repaints_the_chunk_its_neighbors_and_spawns_debris() {
        let (mut app, generation, layout_chunks) = visual_app();
        app.update();
        // A small brush at the origin erases chunk (0,0)'s west-boundary cells, so the
        // (-1,0) neighbor must repaint its seam rim even though its bits never changed.
        let brush = crate::terrain::TerrainBrush {
            center_half_cells_x: 1,
            center_half_cells_y: 1,
            radius_half_cells: 2,
        };
        let event = {
            let mut touched = layout_chunks.clone();
            let outcome = terrain_grid::apply_brush(&mut touched, brush);
            TerrainDestructionEvent {
                generation,
                revision: 1,
                source_attack_id: crate::combat::AttackId(1),
                source_delivery_index: 0,
                brush,
                affected_chunks: outcome.affected_chunks,
                erased_cells: outcome.erased_cells,
            }
        };
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            assert_eq!(
                convergence.apply_event(event),
                TerrainConvergenceAction::Applied
            );
        }
        app.update();
        let world = app.world_mut();
        // The seam-facing west neighbor repainted too: its x=31 rim follows the crater.
        let mut visuals = world.query_filtered::<&TerrainChunkVisual, ()>();
        let west = visuals
            .iter(world)
            .find(|visual| visual.chunk == TerrainChunkId { x: -1, y: 0 })
            .expect("west neighbor visual");
        let images = world.resource::<Assets<Image>>();
        let image = images.get(&west.image).expect("west image");
        // The erased seam cell is now a hole; the occupied cell one further west rims it,
        // proving the unchanged neighbor chunk repainted from the new occupancy.
        let rim = pixel(image.data.as_deref().unwrap_or_default(), 30, 0);
        assert_eq!(rim, TERRAIN_EDGE_PIXEL);
        // One cosmetic burst for the committed brush.
        let mut debris = world.query::<&TerrainDebris>();
        assert_eq!(debris.iter(world).count(), 1);
    }
}

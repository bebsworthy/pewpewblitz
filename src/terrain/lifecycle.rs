//! Authoritative terrain generation lifecycle: map reconcile/install/teardown and the
//! match-restart environment reset.
//!
//! Per-tick brush admission and collision commits stay in `authority.rs`; this module
//! owns when a terrain generation exists and exactly which bounded state survives its
//! boundaries.

use super::authority::{
    PendingTerrainBrushes, TerrainAdmissionCapacity, TerrainBrushBatch, TerrainBrushEpoch,
    TerrainChunk, TerrainChunkCollision, TerrainChunkIndex, TerrainChunkState, TerrainOutbox,
    TerrainRecoveryCache, TerrainRoot, TerrainTransaction, commit_terrain_collision,
};
use super::collider;
use super::grid as terrain_grid;
use super::model::{
    MAX_TERRAIN_ACTIVE_FIGHTERS, MAX_TERRAIN_BRUSHES_PER_TICK, TERRAIN_WORDS_PER_CHUNK,
    TerrainBits, TerrainChunkId, TerrainGeneration, TerrainResetEvent,
};
use super::telemetry::{TerrainTelemetry, TerrainTelemetryOutcome, TerrainTelemetryRecord};
use crate::map::{
    EngineMapLimits, InitialTerrainLayout, MapInstanceId, ResolvedMap, resolve_initial_terrain,
};
use crate::matchplay::MatchId;
use avian2d::prelude::{Position, RigidBody, Rotation};
use bevy::prelude::*;
use std::collections::BTreeMap;

/// Exact-generation terrain teardown invoked by authoritative map teardown. No stale
/// collider, root, or queued record survives into an unrelated frame; the registered
/// fixed-post systems hold the shared resources as unconditional parameters, so teardown
/// resets them to a valid empty generation instead of removing them.
pub fn teardown_authoritative_terrain(world: &mut World) {
    let mut terrain_entities: Vec<Entity> = world
        .query_filtered::<Entity, With<TerrainRoot>>()
        .iter(world)
        .collect();
    terrain_entities.extend(
        world
            .query_filtered::<Entity, With<TerrainChunk>>()
            .iter(world),
    );
    for entity in terrain_entities {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
    world.insert_resource(TerrainChunkIndex::default());
    world.insert_resource(PendingTerrainBrushes::default());
    world.insert_resource(TerrainOutbox::default());
    world.insert_resource(TerrainRecoveryCache::default());
    world.insert_resource(TerrainBrushBatch::default());
    world.insert_resource(TerrainTransaction::default());
    world.insert_resource(TerrainBrushEpoch::default());
    // Telemetry is match-scoped: a removed or replaced generation leaves no records or
    // aggregates behind for the next one to inherit.
    world.insert_resource(TerrainTelemetry::default());
}

/// Derive the initial layout from one validated resolved map snapshot.
fn derive_layout(resolved: &ResolvedMap) -> Result<InitialTerrainLayout, String> {
    resolve_initial_terrain(
        resolved.snapshot.playable_bounds,
        &resolved.snapshot.geometry,
        &resolved.snapshot.regions,
        &resolved.snapshot.spawn_points,
        &resolved.snapshot.mode_anchors,
        EngineMapLimits::default(),
    )
}

pub(super) fn reconcile_authoritative_terrain(world: &mut World) {
    let Some(resolved) = world.get_resource::<ResolvedMap>().cloned() else {
        if world.query::<&TerrainRoot>().iter(world).next().is_some() {
            teardown_authoritative_terrain(world);
        }
        return;
    };
    let instance_id = resolved.snapshot.identity.instance_id;
    let existing = world.query::<&TerrainRoot>().iter(world).next().copied();
    if let Some(root) = existing
        && root.map_instance_id == instance_id
    {
        adopt_match_generation(world, root);
        refresh_admission_capacity(world);
        return;
    }
    teardown_authoritative_terrain(world);

    let layout = derive_layout(&resolved).expect("validated map snapshot re-derives its terrain");
    if !layout.is_empty()
        && let Some(capacity) = world.get_resource::<crate::matchplay::ResolvedMatchCapacity>()
        && usize::from(capacity.maximum_active_fighters) > MAX_TERRAIN_ACTIVE_FIGHTERS
    {
        panic!("terrain-enabled match capacity exceeds the engine fighter ceiling");
    }
    install_terrain(
        world,
        instance_id,
        layout.terrain_fingerprint,
        &layout.chunks,
    );
    let root = world
        .query::<&TerrainRoot>()
        .iter(world)
        .next()
        .copied()
        .expect("terrain root exists after installation");
    adopt_match_generation(world, root);
    refresh_admission_capacity(world);
    info!(
        instance_id = instance_id.0,
        chunks = layout.chunks.len(),
        cells = layout.total_cells,
        fingerprint = layout.terrain_fingerprint,
        "authoritative terrain installed"
    );
}

/// Adopt the current match generation onto the terrain root. Live terrain events carry
/// the exact map-plus-match generation.
fn adopt_match_generation(world: &mut World, root: TerrainRoot) {
    if root.match_id.is_some() {
        return;
    }
    let match_id = world
        .query_filtered::<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>()
        .iter(world)
        .next()
        .map(|state| state.match_id);
    if let Some(match_id) = match_id
        && let Some(entity) = world
            .query_filtered::<Entity, With<TerrainRoot>>()
            .iter(world)
            .next()
    {
        world.entity_mut(entity).insert(TerrainRoot {
            match_id: Some(match_id),
            ..root
        });
    }
}

fn refresh_admission_capacity(world: &mut World) {
    if let Some(capacity) = world.get_resource::<crate::matchplay::ResolvedMatchCapacity>() {
        let admitted =
            usize::from(capacity.maximum_active_fighters).clamp(1, MAX_TERRAIN_BRUSHES_PER_TICK);
        world.insert_resource(TerrainAdmissionCapacity(admitted));
    }
}

/// Spawn the deterministic ascending chunk entities with seam-reconciled colliders, the
/// root, the index, and the recovery cache.
#[allow(clippy::cast_possible_truncation)]
fn install_terrain(
    world: &mut World,
    instance_id: MapInstanceId,
    fingerprint: u64,
    chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
) {
    let mut prospective: Vec<(TerrainChunkId, avian2d::parry::shape::Voxels)> = chunks
        .iter()
        .filter_map(|(chunk, bits)| Some((*chunk, collider::build_voxels(bits)?)))
        .collect();
    collider::reconcile_neighbors(&mut prospective);
    let colliders: BTreeMap<_, _> = prospective
        .into_iter()
        .map(|(chunk, voxels)| (chunk, collider::voxels_collider(voxels)))
        .collect();

    world.spawn(TerrainRoot {
        map_instance_id: instance_id,
        terrain_fingerprint: fingerprint,
        match_id: None,
        revision: 0,
    });
    let mut index = TerrainChunkIndex::default();
    for (chunk_id, bits) in chunks {
        let min = terrain_grid::chunk_min_world(*chunk_id);
        let entity = world
            .spawn((
                TerrainChunk {
                    id: *chunk_id,
                    map_instance_id: instance_id,
                },
                TerrainChunkState {
                    initial: *bits,
                    current: *bits,
                    last_modified_revision: 0,
                },
                TerrainChunkCollision {
                    occupied_cells: bits.count() as u16,
                    collider_revision: 0,
                },
            ))
            .id();
        if let Some(collider_value) = colliders.get(chunk_id) {
            world.entity_mut(entity).insert((
                RigidBody::Static,
                collider_value.clone(),
                crate::movement::destructible_terrain_collision_layers(),
                Position::from_xy(min.x, min.y),
                Rotation::default(),
                Transform::from_translation(min.extend(0.0)),
            ));
        }
        index.0.insert(*chunk_id, entity);
    }
    world.insert_resource(index);
    world.insert_resource(TerrainRecoveryCache {
        revision: 0,
        chunks: chunks.clone(),
    });
    // A fresh generation starts from empty bounded queues and an open brush epoch,
    // independent of what any previous generation left behind.
    world.insert_resource(PendingTerrainBrushes::default());
    world.insert_resource(TerrainOutbox::default());
    world.insert_resource(TerrainBrushBatch::default());
    world.insert_resource(TerrainTransaction::default());
    world.insert_resource(TerrainBrushEpoch::default());
    world.insert_resource(TerrainTelemetry::default());
}

fn pending_restart_slot(
    restart: &crate::matchplay::PendingMatchRestart,
) -> Option<crate::matchplay::PendingMatchRestartSlot> {
    restart.slot()
}

/// Match-restart environment reset: restore every initial bitset and collider, reset the
/// revision for the new match generation, clear every brush queued by the previous match
/// and its telemetry epoch, and stage one reset event. Runs inside the chained restart
/// transaction before common commit.
pub(crate) fn reset_terrain_on_match_restart(world: &mut World) {
    let Some(slot) = world
        .get_resource::<crate::matchplay::PendingMatchRestart>()
        .and_then(pending_restart_slot)
    else {
        return;
    };
    let Some(root) = world.query::<&TerrainRoot>().iter(world).next().copied() else {
        return;
    };
    if root
        .match_id
        .is_some_and(|match_id| match_id != slot.previous_id)
    {
        return;
    }
    // The new match must never observe queued work from the previous one. Drop deferred
    // whole-brush facts, the collected batch, and combat's current-tick world-effect
    // buffer, then advance the brush epoch past the restart tick: payload resolution
    // runs later in this same fixed-post chain and can still stage facts for deliveries
    // resolved at the restart tick itself.
    world
        .resource_mut::<crate::combat::CombatWorldEffectFacts>()
        .0
        .clear();
    *world.resource_mut::<PendingTerrainBrushes>() = PendingTerrainBrushes::default();
    *world.resource_mut::<TerrainBrushBatch>() = TerrainBrushBatch::default();
    world.insert_resource(TerrainBrushEpoch(slot.restart_tick.saturating_add(1)));
    let index = world.resource::<TerrainChunkIndex>().clone();
    let mut changed: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
    let mut changed_masks: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
    for (chunk, entity) in &index.0 {
        let Some(state) = world.get::<TerrainChunkState>(*entity) else {
            continue;
        };
        if state.current != state.initial {
            let mut mask = TerrainBits::default();
            for word_index in 0..TERRAIN_WORDS_PER_CHUNK {
                mask.0[word_index] = state.current.0[word_index] ^ state.initial.0[word_index];
            }
            changed.insert(*chunk, state.initial);
            changed_masks.insert(*chunk, mask);
        }
    }
    let rebuilt_colliders = super::authority::compute_dirty_union(&index, &changed_masks).len();
    let transaction = TerrainTransaction {
        changed,
        changed_masks,
        facts: Vec::new(),
        staged_events: Vec::new(),
        pending_records: Vec::new(),
        revision: 0,
        active: true,
    };
    // The new match's telemetry epoch starts empty: the reset's own collider rebuilds
    // and its Reset record must be the first facts of the new generation, not appended
    // to the previous match's counters, records, and dirty sets.
    *world.resource_mut::<TerrainTelemetry>() = TerrainTelemetry::default();
    // Commit the restored occupancy and colliders exactly like a brush transaction, then
    // finish the generation switch on the root. Re-occupied chunks regain colliders.
    *world.resource_mut::<TerrainTransaction>() = transaction;
    commit_terrain_collision(world);
    if let Some(root_entity) = world
        .query_filtered::<Entity, With<TerrainRoot>>()
        .iter(world)
        .next()
    {
        let previous_generation = root.generation();
        world.entity_mut(root_entity).insert(TerrainRoot {
            match_id: Some(slot.next_id),
            revision: 0,
            ..root
        });
        *world.resource_mut::<TerrainOutbox>() = TerrainOutbox {
            reset: Some(TerrainResetEvent {
                previous_generation,
                next_generation: root.generation().map_match(slot.next_id),
            }),
            ..Default::default()
        };
    }
    let mut telemetry = world.resource_mut::<TerrainTelemetry>();
    telemetry.record(TerrainTelemetryRecord {
        tick: slot.restart_tick,
        map_instance_id: root.map_instance_id,
        revision: 0,
        source_attack_id: None,
        delivery_index: None,
        brush: None,
        affected_chunks: Vec::new(),
        erased_cells: 0,
        rebuilt_colliders,
        serialized_event_bytes: None,
        outcome: TerrainTelemetryOutcome::Reset,
    });
}

impl TerrainGeneration {
    #[must_use]
    fn map_match(self, match_id: MatchId) -> Self {
        Self {
            map_instance_id: self.map_instance_id,
            match_id,
            terrain_fingerprint: self.terrain_fingerprint,
        }
    }
}

//! Server-authoritative terrain ownership: chunk state, brush transactions, Avian
//! collider lifecycle, match restart reset, and defensive fighter repair.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::wildcard_imports,
    reason = "integer grid math, small copied facts, and the shared model re-export mirror the sibling modules"
)]

use super::TerrainSet;
use super::collider;
use super::grid as terrain_grid;
use super::model::*;
use super::telemetry::{TerrainTelemetry, TerrainTelemetryOutcome, TerrainTelemetryRecord};
use crate::combat::{CombatWorldEffectFact, SpawnState, WorldEffectDefinition};
use crate::map::{
    EngineMapLimits, InitialTerrainLayout, MapInstanceId, ResolvedMap, resolve_initial_terrain,
};
use crate::matchplay::MatchId;
use avian2d::prelude::{Collider, CollisionLayers, Position, RigidBody, Rotation};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Root of the authoritative terrain for one exact map generation.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainRoot {
    pub map_instance_id: MapInstanceId,
    pub terrain_fingerprint: u64,
    pub match_id: Option<MatchId>,
    pub revision: u64,
}

impl TerrainRoot {
    #[must_use]
    pub fn generation(&self) -> TerrainGeneration {
        TerrainGeneration {
            map_instance_id: self.map_instance_id,
            match_id: self.match_id.unwrap_or(MatchId(0)),
            terrain_fingerprint: self.terrain_fingerprint,
        }
    }
}

/// One allocated terrain chunk. Stable across the whole map generation, including while
/// empty; never carries process-local entity identity onto the wire.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainChunk {
    pub id: TerrainChunkId,
    pub map_instance_id: MapInstanceId,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainChunkState {
    pub initial: TerrainBits,
    pub current: TerrainBits,
    pub last_modified_revision: u64,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainChunkCollision {
    pub occupied_cells: u16,
    pub collider_revision: u64,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct TerrainChunkIndex(pub BTreeMap<TerrainChunkId, Entity>);

/// One deferred whole-brush fact waiting for admission capacity.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingTerrainBrush {
    pub fact: CombatWorldEffectFact,
}

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct PendingTerrainBrushes {
    pub queue: VecDeque<PendingTerrainBrush>,
}

/// Bounded server-to-client terrain publications waiting for the network publisher.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TerrainOutbox {
    pub events: VecDeque<TerrainDestructionEvent>,
    pub reset: Option<TerrainResetEvent>,
    pub recovery_responses: VecDeque<(Entity, TerrainRecoverySnapshot)>,
    pub dropped_events: u64,
}

impl TerrainOutbox {
    const MAX_EVENTS: usize = 256;
    const MAX_RECOVERY_RESPONSES: usize = 32;

    pub(crate) fn push_event(&mut self, event: TerrainDestructionEvent) {
        if self.events.len() >= Self::MAX_EVENTS {
            self.events.pop_front();
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
        self.events.push_back(event);
    }

    pub(crate) fn push_recovery_response(
        &mut self,
        link: Entity,
        snapshot: TerrainRecoverySnapshot,
    ) {
        if self.recovery_responses.len() >= Self::MAX_RECOVERY_RESPONSES {
            self.recovery_responses.pop_front();
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
        self.recovery_responses.push_back((link, snapshot));
    }
}

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TerrainRecoveryCache {
    pub revision: u64,
    pub chunks: BTreeMap<TerrainChunkId, TerrainBits>,
}

/// The brush admission ceiling derived from the resolved map/mode capacity. Terrain never
/// derives concurrency from team topology; it consumes one checked fighter count.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainAdmissionCapacity(pub usize);

impl Default for TerrainAdmissionCapacity {
    fn default() -> Self {
        Self(MAX_TERRAIN_BRUSHES_PER_TICK)
    }
}

/// The deterministic brush batch collected in one fixed post-update tick.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TerrainBrushBatch {
    pub brushes: Vec<CombatWorldEffectFact>,
}

/// The prospective transaction between `ApplyBrushes` and `RebuildCollision`. Holding
/// occupancy in scratch until collider construction succeeds is what makes the commit
/// atomic: on any failure nothing installs.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TerrainTransaction {
    /// Prospective current bits for every chunk whose occupancy changed.
    pub changed: BTreeMap<TerrainChunkId, TerrainBits>,
    /// XOR masks of changed cells per changed chunk, for boundary-neighbor detection.
    pub changed_masks: BTreeMap<TerrainChunkId, TerrainBits>,
    /// The admitted facts that produced this transaction, retained for whole-batch
    /// deferral when collider construction refuses.
    pub facts: Vec<CombatWorldEffectFact>,
    pub staged_events: Vec<TerrainDestructionEvent>,
    pub revision: u64,
    pub active: bool,
}

/// The authoritative terrain plugin: exact-generation chunk state, the fixed-post terrain
/// chain, restart reset, and defensive repair.
pub struct AuthoritativeTerrainPlugin;

impl Plugin for AuthoritativeTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::combat::CombatWorldEffectFacts>()
            .init_resource::<TerrainChunkIndex>()
            .init_resource::<PendingTerrainBrushes>()
            .init_resource::<TerrainOutbox>()
            .init_resource::<TerrainRecoveryCache>()
            .init_resource::<TerrainTelemetry>()
            .init_resource::<TerrainAdmissionCapacity>()
            .init_resource::<TerrainBrushBatch>()
            .init_resource::<TerrainTransaction>()
            .add_systems(
                FixedUpdate,
                reconcile_authoritative_terrain.before(crate::gameplay::GameplaySet::Input),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    collect_terrain_brushes.in_set(TerrainSet::CollectBrushes),
                    apply_terrain_brushes.in_set(TerrainSet::ApplyBrushes),
                    commit_terrain_collision.in_set(TerrainSet::RebuildCollision),
                    repair_embedded_fighters.in_set(TerrainSet::ValidateFighters),
                ),
            );
        register_terrain_schedule(app);
        crate::terrain::network::register_terrain_network(app);
        crate::matchplay::register_environment_reset_system(app, reset_terrain_on_match_restart);
    }
}

/// Configure the terrain chain against the existing fixed-post composition: after combat
/// damage and ability outcome observation, before mode rules.
pub(crate) fn register_terrain_schedule(app: &mut App) {
    app.configure_sets(
        FixedPostUpdate,
        (
            TerrainSet::CollectBrushes,
            TerrainSet::ApplyBrushes,
            TerrainSet::RebuildCollision,
            TerrainSet::ValidateFighters,
            TerrainSet::Publish,
        )
            .chain()
            .after(crate::abilities::AbilitySet::ObserveOutcomes)
            .after(crate::combat::CombatSet::Damage)
            .before(crate::matchplay::MatchSet::ModeRules),
    );
    // Collider replacement must be complete before fighters are validated against it.
    app.add_systems(
        FixedPostUpdate,
        ApplyDeferred
            .after(TerrainSet::RebuildCollision)
            .before(TerrainSet::ValidateFighters),
    );
}

/// Exact-generation terrain teardown invoked by authoritative map teardown. No stale
/// collider, root, resource, or queue survives into an unrelated frame.
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
    world.remove_resource::<TerrainChunkIndex>();
    world.remove_resource::<PendingTerrainBrushes>();
    world.remove_resource::<TerrainOutbox>();
    world.remove_resource::<TerrainRecoveryCache>();
    world.remove_resource::<TerrainBrushBatch>();
    world.remove_resource::<TerrainTransaction>();
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

fn reconcile_authoritative_terrain(world: &mut World) {
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
    // Teardown removed the previous generation's queues and transaction state; a fresh
    // generation starts from empty bounded state, not from stale leftovers.
    if world.get_resource::<PendingTerrainBrushes>().is_none() {
        world.init_resource::<PendingTerrainBrushes>();
    }
    if world.get_resource::<TerrainOutbox>().is_none() {
        world.init_resource::<TerrainOutbox>();
    }
    if world.get_resource::<TerrainBrushBatch>().is_none() {
        world.init_resource::<TerrainBrushBatch>();
    }
    if world.get_resource::<TerrainTransaction>().is_none() {
        world.init_resource::<TerrainTransaction>();
    }
}

/// Merge deferred and new world-effect facts into one deterministic sorted batch.
fn collect_terrain_brushes(
    mut facts: ResMut<crate::combat::CombatWorldEffectFacts>,
    mut deferred: ResMut<PendingTerrainBrushes>,
    mut batch: ResMut<TerrainBrushBatch>,
) {
    let mut brushes = std::mem::take(&mut batch.brushes);
    brushes.append(&mut facts.0);
    brushes.extend(deferred.queue.drain(..).map(|pending| pending.fact));
    brushes.sort_by(brush_order);
    brushes.dedup_by(|left, right| brush_key(left) == brush_key(right));
    batch.brushes = brushes;
}

fn brush_key(fact: &CombatWorldEffectFact) -> (u64, u64, u8, u8) {
    (
        fact.tick,
        fact.source.attack_id.0,
        fact.delivery_index,
        fact.effect_index,
    )
}

fn brush_order(left: &CombatWorldEffectFact, right: &CombatWorldEffectFact) -> std::cmp::Ordering {
    brush_key(left).cmp(&brush_key(right))
}

/// Apply the admitted brushes to scratch occupancy, staging events against the state
/// produced by all earlier sorted facts. No ECS bits change here.
#[allow(clippy::too_many_lines)]
#[derive(bevy::ecs::system::SystemParam)]
struct TerrainMutationState<'w> {
    telemetry: ResMut<'w, TerrainTelemetry>,
    transaction: ResMut<'w, TerrainTransaction>,
}

fn apply_terrain_brushes(
    mut batch: ResMut<TerrainBrushBatch>,
    mut deferred: ResMut<PendingTerrainBrushes>,
    capacity: Res<TerrainAdmissionCapacity>,
    roots: Query<&TerrainRoot>,
    chunks: Query<&TerrainChunkState>,
    index: Res<TerrainChunkIndex>,
    mut mutation: TerrainMutationState,
) {
    if batch.brushes.is_empty() {
        return;
    }
    let Ok(root) = roots.single() else {
        batch.brushes.clear();
        return;
    };
    let root = *root;
    let admission = capacity.0.clamp(1, MAX_TERRAIN_BRUSHES_PER_TICK);
    let mut admitted = std::mem::take(&mut batch.brushes);

    defer_excess_brushes(
        &mut admitted,
        admission,
        &mut deferred,
        root,
        &mut mutation.telemetry,
    );

    let mut scratch: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
    let mut previous: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
    let mut revision = root.revision;
    let mut staged_events = Vec::new();
    let mut applied = 0_u64;
    for fact in &admitted {
        mutation.telemetry.record_request();
        let WorldEffectDefinition::DestroyTerrain { radius } = fact.effect;
        let Some(brush) = terrain_grid::quantize_brush(fact.position.as_vec2(), radius) else {
            mutation.telemetry.record(no_op_record(
                root,
                fact,
                TerrainTelemetryOutcome::NoOccupiedCell,
            ));
            continue;
        };
        seed_scratch(&index, &chunks, brush, &mut scratch, &mut previous);
        let outcome = terrain_grid::apply_brush(&mut scratch, brush);
        if outcome.erased_cells == 0 {
            mutation.telemetry.record(no_op_record(
                root,
                fact,
                TerrainTelemetryOutcome::NoOccupiedCell,
            ));
            continue;
        }
        revision = revision.saturating_add(1);
        applied = applied.saturating_add(1);
        let event = TerrainDestructionEvent {
            generation: root.generation(),
            revision,
            source_attack_id: fact.source.attack_id,
            source_delivery_index: fact.delivery_index,
            brush,
            affected_chunks: outcome.affected_chunks,
            erased_cells: outcome.erased_cells,
        };
        mutation.telemetry.record(TerrainTelemetryRecord {
            tick: fact.tick,
            map_instance_id: root.map_instance_id,
            revision,
            source_attack_id: Some(fact.source.attack_id),
            delivery_index: Some(fact.delivery_index),
            brush: Some(brush),
            affected_chunks: event.affected_chunks.clone(),
            erased_cells: event.erased_cells,
            rebuilt_colliders: 0,
            serialized_event_bytes: terrain_grid::destruction_event_bytes(&event),
            outcome: TerrainTelemetryOutcome::Applied,
        });
        staged_events.push(event);
    }

    // Fold the final scratch state into per-chunk changed bits and XOR masks.
    let mut changed = BTreeMap::new();
    let mut changed_masks = BTreeMap::new();
    for (chunk, previous_bits) in &previous {
        if let Some(final_bits) = scratch.get(chunk)
            && final_bits != previous_bits
        {
            let mut mask = TerrainBits::default();
            for word_index in 0..TERRAIN_WORDS_PER_CHUNK {
                mask.0[word_index] = previous_bits.0[word_index] ^ final_bits.0[word_index];
            }
            changed.insert(*chunk, *final_bits);
            changed_masks.insert(*chunk, mask);
        }
    }
    *mutation.transaction = TerrainTransaction {
        changed,
        changed_masks,
        facts: admitted,
        staged_events,
        revision,
        active: true,
    };
    mutation.telemetry.record_tick_maxima(applied, 0);
}

/// Defer each complete excess fact before evaluating it; never split a brush. Queue
/// overflow is a diagnostic failure that rejects the newest excess fact.
fn defer_excess_brushes(
    admitted: &mut Vec<CombatWorldEffectFact>,
    admission: usize,
    deferred: &mut PendingTerrainBrushes,
    root: TerrainRoot,
    telemetry: &mut TerrainTelemetry,
) {
    while admitted.len() > admission {
        let overflow = admitted
            .pop()
            .expect("admitted batch exceeds the admission ceiling");
        if deferred.queue.len() >= MAX_PENDING_TERRAIN_BRUSHES {
            telemetry.record(no_op_record(
                root,
                &overflow,
                TerrainTelemetryOutcome::RejectedQueueFull,
            ));
            continue;
        }
        telemetry.record(no_op_record(
            root,
            &overflow,
            TerrainTelemetryOutcome::DeferredRebuildBudget,
        ));
        deferred
            .queue
            .push_back(PendingTerrainBrush { fact: overflow });
    }
}

/// Copy current occupancy for every chunk the brush can touch into the scratch map.
fn seed_scratch(
    index: &TerrainChunkIndex,
    chunks: &Query<&TerrainChunkState>,
    brush: TerrainBrush,
    scratch: &mut BTreeMap<TerrainChunkId, TerrainBits>,
    previous: &mut BTreeMap<TerrainChunkId, TerrainBits>,
) {
    let ((x_min, x_max), (y_min, y_max)) = terrain_grid::brush_cell_range(brush);
    for cell_y in y_min..=y_max {
        for cell_x in x_min..=x_max {
            let Some((chunk, _)) = terrain_grid::cell_to_chunk_and_local((cell_x, cell_y)) else {
                continue;
            };
            if scratch.contains_key(&chunk) {
                continue;
            }
            if let Some(entity) = index.0.get(&chunk)
                && let Ok(state) = chunks.get(*entity)
            {
                scratch.insert(chunk, state.current);
                previous.insert(chunk, state.current);
            }
        }
    }
}

fn no_op_record(
    root: TerrainRoot,
    fact: &CombatWorldEffectFact,
    outcome: TerrainTelemetryOutcome,
) -> TerrainTelemetryRecord {
    TerrainTelemetryRecord {
        tick: fact.tick,
        map_instance_id: root.map_instance_id,
        revision: root.revision,
        source_attack_id: Some(fact.source.attack_id),
        delivery_index: Some(fact.delivery_index),
        brush: None,
        affected_chunks: Vec::new(),
        erased_cells: 0,
        rebuilt_colliders: 0,
        serialized_event_bytes: None,
        outcome,
    }
}

/// Which chunk edge a boundary test addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    East,
    West,
    North,
    South,
}

/// Does any changed cell sit on this chunk edge, altering cross-chunk voxel topology?
fn mask_touches_boundary(mask: &TerrainBits, boundary: Boundary) -> bool {
    match boundary {
        Boundary::East => (0..TERRAIN_CHUNK_SIDE_CELLS).any(|y| mask.get(31, y)),
        Boundary::West => (0..TERRAIN_CHUNK_SIDE_CELLS).any(|y| mask.get(0, y)),
        Boundary::North => (0..TERRAIN_CHUNK_SIDE_CELLS).any(|x| mask.get(x, 31)),
        Boundary::South => (0..TERRAIN_CHUNK_SIDE_CELLS).any(|x| mask.get(x, 0)),
    }
}

/// The collision-dirty union: every occupancy-changed chunk plus an allocated orthogonal
/// neighbor only where a changed boundary cell alters cross-chunk topology.
fn compute_dirty_union(
    index: &TerrainChunkIndex,
    changed_masks: &BTreeMap<TerrainChunkId, TerrainBits>,
) -> BTreeSet<TerrainChunkId> {
    let mut union: BTreeSet<TerrainChunkId> = changed_masks.keys().copied().collect();
    for (chunk, mask) in changed_masks {
        let neighbors = [
            (
                Boundary::East,
                TerrainChunkId {
                    x: chunk.x.saturating_add(1),
                    y: chunk.y,
                },
            ),
            (
                Boundary::West,
                TerrainChunkId {
                    x: chunk.x.saturating_sub(1),
                    y: chunk.y,
                },
            ),
            (
                Boundary::North,
                TerrainChunkId {
                    x: chunk.x,
                    y: chunk.y.saturating_add(1),
                },
            ),
            (
                Boundary::South,
                TerrainChunkId {
                    x: chunk.x,
                    y: chunk.y.saturating_sub(1),
                },
            ),
        ];
        for (boundary, neighbor) in neighbors {
            if index.0.contains_key(&neighbor) && mask_touches_boundary(mask, boundary) {
                union.insert(neighbor);
            }
        }
    }
    union
}

/// Build and reconcile every prospective collider for the dirty union, then atomically
/// install occupancy, colliders, recovery cache, revision, and staged events. A refusal
/// defers the complete batch without committing partial state.
#[allow(clippy::too_many_lines)]
fn commit_terrain_collision(world: &mut World) {
    let Some(transaction) = world.get_resource::<TerrainTransaction>() else {
        return;
    };
    if !transaction.active {
        return;
    }
    let transaction = transaction.clone();
    let Some(root) = world.query::<&TerrainRoot>().iter(world).next().copied() else {
        *world.resource_mut::<TerrainTransaction>() = TerrainTransaction::default();
        return;
    };
    let index = world.resource::<TerrainChunkIndex>().clone();
    let mut current_state: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
    for (chunk, entity) in &index.0 {
        if let Some(state) = world.get::<TerrainChunkState>(*entity) {
            current_state.insert(*chunk, state.current);
        }
    }

    let union = compute_dirty_union(&index, &transaction.changed_masks);
    if union.len() > MAX_TERRAIN_COLLIDER_REBUILDS_PER_TICK {
        // Malformed or injected workload: defer the whole batch, mutate nothing.
        let mut deferred = world.resource_mut::<PendingTerrainBrushes>();
        for fact in &transaction.facts {
            deferred
                .queue
                .push_back(PendingTerrainBrush { fact: fact.clone() });
        }
        let mut telemetry = world.resource_mut::<TerrainTelemetry>();
        for event in &transaction.staged_events {
            telemetry.record(TerrainTelemetryRecord {
                tick: 0,
                map_instance_id: root.map_instance_id,
                revision: root.revision,
                source_attack_id: Some(event.source_attack_id),
                delivery_index: Some(event.source_delivery_index),
                brush: Some(event.brush),
                affected_chunks: event.affected_chunks.clone(),
                erased_cells: 0,
                rebuilt_colliders: 0,
                serialized_event_bytes: None,
                outcome: TerrainTelemetryOutcome::DeferredRebuildBudget,
            });
        }
        *world.resource_mut::<TerrainTransaction>() = TerrainTransaction::default();
        return;
    }

    // Prospective bits for the whole union, then fresh reconciled voxel shapes.
    let prospective_bits: BTreeMap<TerrainChunkId, TerrainBits> = union
        .iter()
        .map(|chunk| {
            (
                *chunk,
                transaction
                    .changed
                    .get(chunk)
                    .copied()
                    .or_else(|| current_state.get(chunk).copied())
                    .unwrap_or_default(),
            )
        })
        .collect();
    let mut prospective: Vec<(TerrainChunkId, avian2d::parry::shape::Voxels)> = prospective_bits
        .iter()
        .filter_map(|(chunk, bits)| Some((*chunk, collider::build_voxels(bits)?)))
        .collect();
    collider::reconcile_neighbors(&mut prospective);

    let voxels_before: u64 = current_state
        .values()
        .map(|bits| u64::from(bits.count()))
        .sum();
    {
        let mut recovery = world.resource_mut::<TerrainRecoveryCache>();
        for (chunk, bits) in &transaction.changed {
            recovery.chunks.insert(*chunk, *bits);
        }
        recovery.revision = transaction.revision;
    }

    for (chunk, bits) in &transaction.changed {
        let Some(entity) = index.0.get(chunk) else {
            continue;
        };
        let Ok(mut entity_mut) = world.get_entity_mut(*entity) else {
            continue;
        };
        let initial = entity_mut
            .get::<TerrainChunkState>()
            .map_or(*bits, |state| state.initial);
        entity_mut.insert(TerrainChunkState {
            initial,
            current: *bits,
            last_modified_revision: transaction.revision,
        });
        entity_mut.insert(TerrainChunkCollision {
            occupied_cells: bits.count() as u16,
            collider_revision: transaction.revision,
        });
        if bits.is_empty() {
            entity_mut.remove::<(Collider, RigidBody, CollisionLayers)>();
        }
    }
    for (chunk, voxels) in prospective {
        let Some(entity) = index.0.get(&chunk) else {
            continue;
        };
        let Ok(mut entity_mut) = world.get_entity_mut(*entity) else {
            continue;
        };
        let min = terrain_grid::chunk_min_world(chunk);
        // Every union chunk gets its fresh reconciled collider, including unchanged
        // boundary neighbors whose cross-seam topology changed.
        entity_mut.insert((
            RigidBody::Static,
            collider::voxels_collider(voxels),
            crate::movement::destructible_terrain_collision_layers(),
            Position::from_xy(min.x, min.y),
            Rotation::default(),
            Transform::from_translation(min.extend(0.0)),
        ));
    }
    if let Some(root_entity) = world
        .query_filtered::<Entity, With<TerrainRoot>>()
        .iter(world)
        .next()
    {
        world.entity_mut(root_entity).insert(TerrainRoot {
            revision: transaction.revision,
            ..root
        });
    }

    let voxels_after: u64 = index
        .0
        .values()
        .map(|entity| {
            world
                .get::<TerrainChunkState>(*entity)
                .map_or(0, |state| u64::from(state.current.count()))
        })
        .sum();
    let empty_chunks = index
        .0
        .values()
        .filter(|entity| {
            world
                .get::<TerrainChunkState>(**entity)
                .is_some_and(|state| state.current.is_empty())
        })
        .count();
    {
        let mut telemetry = world.resource_mut::<TerrainTelemetry>();
        telemetry
            .aggregates
            .collision_rebuilt_chunks
            .extend(union.iter().copied());
        telemetry.record_tick_maxima(0, union.len() as u64);
        telemetry.record_collider_state(voxels_before, voxels_after, empty_chunks);
    }
    let mut outbox = world.resource_mut::<TerrainOutbox>();
    for event in transaction.staged_events {
        outbox.push_event(event);
    }
    *world.resource_mut::<TerrainTransaction>() = TerrainTransaction::default();
}

fn pending_restart_slot(
    restart: &crate::matchplay::PendingMatchRestart,
) -> Option<crate::matchplay::PendingMatchRestartSlot> {
    restart.slot()
}

/// Match-restart environment reset: restore every initial bitset and collider, reset the
/// revision for the new match generation, and stage one reset event. Runs inside the
/// chained restart transaction before common commit.
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
    let transaction = TerrainTransaction {
        changed,
        changed_masks,
        facts: Vec::new(),
        staged_events: Vec::new(),
        revision: 0,
        active: true,
    };
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
        rebuilt_colliders: 0,
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

/// Defensive server-only fighter repair for malformed, recovered, or injected states. A
/// valid monotonic erasure never triggers this path.
#[derive(bevy::ecs::system::SystemParam)]
struct TerrainRepairAccess<'w, 's> {
    roots: Query<'w, 's, &'static TerrainRoot>,
    index: Res<'w, TerrainChunkIndex>,
    chunks: Query<'w, 's, &'static TerrainChunkState>,
    resolved: Option<Res<'w, ResolvedMap>>,
    bounds: Option<Res<'w, crate::map::PlayableBounds>>,
    tuning: Option<Res<'w, crate::movement::MovementTuning>>,
    fighters: Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            Option<&'static crate::map::SpawnAssignment>,
        ),
        With<crate::protocol::Fighter>,
    >,
    spawn_states: Query<'w, 's, &'static SpawnState>,
}

/// True when any currently occupied terrain cell overlaps the fighter circle at `point`.
fn repair_occupied_near(
    index: &TerrainChunkIndex,
    chunks: &Query<&TerrainChunkState>,
    point: Vec2,
    radius: f32,
) -> bool {
    let (Some(min_cell), Some(max_cell)) = (
        terrain_grid::world_to_cell(point - Vec2::splat(radius)),
        terrain_grid::world_to_cell(point + Vec2::splat(radius)),
    ) else {
        return false;
    };
    for cell_y in min_cell.1..=max_cell.1 {
        for cell_x in min_cell.0..=max_cell.0 {
            let Some((chunk, (local_x, local_y))) =
                terrain_grid::cell_to_chunk_and_local((cell_x, cell_y))
            else {
                continue;
            };
            let Some(entity) = index.0.get(&chunk) else {
                continue;
            };
            let Ok(state) = chunks.get(*entity) else {
                continue;
            };
            if !state.current.get(local_x, local_y) {
                continue;
            }
            let cell_min = terrain_grid::cell_min_world((cell_x, cell_y));
            let closest = point.clamp(cell_min, cell_min + Vec2::splat(TERRAIN_CELL_SIZE_WORLD));
            if closest.distance_squared(point) < radius * radius {
                return true;
            }
        }
    }
    false
}

/// True when permanent map geometry overlaps the fighter circle at `point`.
fn repair_hits_permanent(resolved: &ResolvedMap, point: Vec2, radius: f32) -> bool {
    resolved
        .snapshot
        .geometry
        .iter()
        .any(|placement| match placement.shape {
            crate::map::MapShape::Circle { radius: obstacle } => {
                point.distance_squared(placement.position) < (radius + obstacle).powi(2)
            }
            crate::map::MapShape::Rectangle { half_extents } => {
                let local =
                    Vec2::from_angle(-placement.rotation).rotate(point - placement.position);
                let closest = local.clamp(-half_extents, half_extents);
                local.distance_squared(closest) < radius * radius
            }
        })
}

/// Deterministic nearest playable, terrain-clear, permanent-geometry-free cell center to
/// `point`: squared distance, then y, then x. Returns `None` when no cell qualifies.
fn nearest_repair_candidate(
    bounds: &crate::map::PlayableBounds,
    resolved: &ResolvedMap,
    index: &TerrainChunkIndex,
    chunks: &Query<&TerrainChunkState>,
    radius: f32,
    point: Vec2,
) -> Option<Vec2> {
    let (min_cell, max_cell) = (
        terrain_grid::world_to_cell(bounds.0.min)?,
        terrain_grid::world_to_cell(bounds.0.max)?,
    );
    let mut best: Option<(f32, i32, i32)> = None;
    for cell_y in min_cell.1..=max_cell.1 {
        for cell_x in min_cell.0..=max_cell.0 {
            let candidate = terrain_grid::cell_center_world((cell_x, cell_y));
            if !bounds.0.contains_with_inset(candidate, radius)
                || repair_occupied_near(index, chunks, candidate, radius)
                || repair_hits_permanent(resolved, candidate, radius)
            {
                continue;
            }
            let key = (candidate.distance_squared(point), cell_y, cell_x);
            if best.is_none_or(|current| key < current) {
                best = Some(key);
            }
        }
    }
    best.map(|(_, cell_y, cell_x)| terrain_grid::cell_center_world((cell_x, cell_y)))
}

fn repair_embedded_fighters(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    mut telemetry: ResMut<TerrainTelemetry>,
    access: TerrainRepairAccess,
) {
    if access.roots.is_empty() {
        return;
    }
    let (Some(resolved), Some(bounds)) = (access.resolved, access.bounds) else {
        return;
    };
    let radius = access.tuning.as_ref().map_or(24.0, |tuning| tuning.radius);
    for (entity, position, assignment) in &access.fighters {
        let point = position.0;
        if point.is_finite()
            && bounds.0.contains_with_inset(point, radius)
            && !repair_occupied_near(&access.index, &access.chunks, point, radius)
            && !repair_hits_permanent(&resolved, point, radius)
        {
            continue;
        }
        let repaired = nearest_repair_candidate(
            &bounds,
            &resolved,
            &access.index,
            &access.chunks,
            radius,
            point,
        )
        .or_else(|| {
            assignment
                .and_then(|assignment| {
                    resolved
                        .snapshot
                        .spawn_points
                        .iter()
                        .find(|point| point.spawn_point_id == assignment.spawn_point_id)
                })
                .map(|point| point.position)
        })
        .or_else(|| {
            access
                .spawn_states
                .get(entity)
                .ok()
                .map(|state| state.position)
        })
        .unwrap_or(bounds.0.center());
        warn!(
            ?entity,
            from = ?point,
            to = ?repaired,
            "defensively repaired an embedded fighter"
        );
        commands
            .entity(entity)
            .insert(Position::from_xy(repaired.x, repaired.y));
        telemetry.record(TerrainTelemetryRecord {
            tick: tick.0,
            map_instance_id: MapInstanceId(0),
            revision: 0,
            source_attack_id: None,
            delivery_index: None,
            brush: None,
            affected_chunks: Vec::new(),
            erased_cells: 0,
            rebuilt_colliders: 0,
            serialized_event_bytes: None,
            outcome: TerrainTelemetryOutcome::DefensiveRepair,
        });
    }
}

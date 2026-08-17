//! Pure client terrain convergence rules: revision/generation validation, snapshot and
//! event application, and reset handling with no ECS or transport access.

#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::wildcard_imports,
    reason = "the shared model mirror, small copied wire facts, and the multi-receiver wire queries match the sibling terrain and transport modules"
)]

use crate::terrain::grid as terrain_grid;
use crate::terrain::model::*;
use bevy::prelude::Resource;
use std::collections::{BTreeMap, BTreeSet};

/// What one wire input did to client convergence. Role plugins interpret only
/// `RequestRecovery` (send one request) and `Invalidated` (surface the exact reason).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerrainConvergenceAction {
    /// Stale, duplicate, or not-yet-applicable input that changed no committed state.
    Ignored,
    /// Input accepted into the pending buffer while recovery is outstanding.
    Buffered,
    /// Input committed new authoritative occupancy.
    Applied,
    /// Intermediate state is unsafe or unknown: request one full snapshot.
    RequestRecovery(TerrainGeneration),
    /// Irrecoverable validation failure for the current generation.
    Invalidated(String),
}

/// Pure client terrain convergence over one expected generation at a time.
///
/// The role plugin feeds the locally derived expected generation and the received wire
/// values in; this machine owns the committed occupancy, revision, buffering, and
/// transition rules. It never touches ECS, images, colliders, or the network itself.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ClientTerrainConvergence {
    pub phase: TerrainConvergencePhase,
    expected_chunks: BTreeSet<TerrainChunkId>,
    chunks: BTreeMap<TerrainChunkId, TerrainBits>,
    revision: u64,
    dirty: Vec<TerrainChunkId>,
    applied_brushes: Vec<TerrainBrush>,
    pending_reset: Option<TerrainResetEvent>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TerrainConvergencePhase {
    #[default]
    WaitingForMap,
    AwaitingRecovery {
        generation: TerrainGeneration,
        request_pending: bool,
        buffered: Vec<TerrainDestructionEvent>,
    },
    Ready {
        generation: TerrainGeneration,
    },
    Invalid {
        generation: TerrainGeneration,
        reason: String,
    },
}

/// How one wire generation relates to the currently expected generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GenerationRelation {
    Same,
    /// A different map instance or match: old traffic that must be discarded.
    Stale,
    /// Same map and match but a different terrain fingerprint: corrupt or foreign.
    Corrupt,
}

fn classify_generation(
    wire: &TerrainGeneration,
    expected: &TerrainGeneration,
) -> GenerationRelation {
    if wire == expected {
        GenerationRelation::Same
    } else if wire.map_instance_id != expected.map_instance_id || wire.match_id != expected.match_id
    {
        GenerationRelation::Stale
    } else {
        GenerationRelation::Corrupt
    }
}

/// Validate one live event's shape against the engine bounds before any application. A
/// real erasure always reports at least one erased cell and one affected chunk; a
/// zero-effect event is corrupt input that must never consume a revision.
fn event_shape_is_valid(
    event: &TerrainDestructionEvent,
    expected: &BTreeSet<TerrainChunkId>,
) -> bool {
    let radius = f32::from(event.brush.radius_half_cells) * TERRAIN_SUBCELL_SIZE_WORLD;
    let mut sorted = event.affected_chunks.clone();
    sorted.sort();
    sorted.dedup();
    radius.is_finite()
        && (TERRAIN_CELL_SIZE_WORLD..=MAX_TERRAIN_BRUSH_RADIUS_WORLD).contains(&radius)
        && event.erased_cells > 0
        && !event.affected_chunks.is_empty()
        && event.affected_chunks.len() <= MAX_TERRAIN_BRUSH_CHUNKS
        && sorted == event.affected_chunks
        && event
            .affected_chunks
            .iter()
            .all(|chunk| expected.contains(chunk))
}

/// True when `bits` occupies at least one cell that `initial` does not author.
fn bits_construct_outside(bits: &TerrainBits, initial: &TerrainBits) -> bool {
    bits.0
        .iter()
        .zip(initial.0.iter())
        .any(|(current, authored)| current & !authored != 0)
}

impl ClientTerrainConvergence {
    /// The committed occupancy, authoritative once a snapshot or reset has committed.
    #[must_use]
    pub fn chunks(&self) -> &BTreeMap<TerrainChunkId, TerrainBits> {
        &self.chunks
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn expected_chunk_set(&self) -> &BTreeSet<TerrainChunkId> {
        &self.expected_chunks
    }

    /// Drain the presentation-dirty chunk IDs accumulated since the last read.
    pub fn take_dirty(&mut self) -> Vec<TerrainChunkId> {
        core::mem::take(&mut self.dirty)
    }

    /// Drain the committed brushes whose craters may present cosmetic feedback.
    pub fn take_applied_brushes(&mut self) -> Vec<TerrainBrush> {
        core::mem::take(&mut self.applied_brushes)
    }

    /// Mark the outstanding recovery request as sent.
    pub fn mark_request_sent(&mut self) {
        if let TerrainConvergencePhase::AwaitingRecovery {
            request_pending, ..
        } = &mut self.phase
        {
            *request_pending = true;
        }
    }

    /// Disconnect clears every generation-scoped state, including terminal `Invalid`.
    pub fn clear(&mut self) {
        self.phase = TerrainConvergencePhase::WaitingForMap;
        self.expected_chunks.clear();
        self.chunks.clear();
        self.revision = 0;
        self.dirty.clear();
        self.applied_brushes.clear();
        self.pending_reset = None;
    }

    /// Observe the locally derived expected generation (map snapshot plus match state).
    /// A changed generation discards all prior terrain state and requests recovery.
    pub fn observe_generation(
        &mut self,
        expected: TerrainGeneration,
        initial_chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
    ) -> TerrainConvergenceAction {
        self.expected_chunks = initial_chunks.keys().copied().collect();
        let restart_recovery = |state: &mut Self| {
            state.chunks.clear();
            state.revision = 0;
            state.pending_reset = None;
            state.dirty.clear();
            state.applied_brushes.clear();
            state.phase = TerrainConvergencePhase::AwaitingRecovery {
                generation: expected,
                request_pending: false,
                buffered: Vec::new(),
            };
            TerrainConvergenceAction::RequestRecovery(expected)
        };
        match &self.phase {
            TerrainConvergencePhase::WaitingForMap => restart_recovery(self),
            TerrainConvergencePhase::AwaitingRecovery { generation, .. } => {
                if *generation == expected {
                    return TerrainConvergenceAction::Ignored;
                }
                if let Some(pending) = &self.pending_reset
                    && pending.previous_generation == expected
                {
                    // A reset chaining from exactly this observed generation is already
                    // outstanding: the match view has not replicated the post-restart id
                    // yet. Keep awaiting the reset's generation instead of churning back
                    // to a request the server must reject as stale.
                    return TerrainConvergenceAction::Ignored;
                }
                restart_recovery(self)
            }
            TerrainConvergencePhase::Ready { generation } if *generation == expected => {
                TerrainConvergenceAction::Ignored
            }
            // Invalid stays terminal for exactly the generation that produced it.
            TerrainConvergencePhase::Invalid { generation, .. } if *generation == expected => {
                TerrainConvergenceAction::Ignored
            }
            TerrainConvergencePhase::Ready { .. } | TerrainConvergencePhase::Invalid { .. } => {
                restart_recovery(self)
            }
        }
    }

    /// Apply one live destruction event. Duplicates are ignored, gaps request recovery,
    /// and verified events commit exactly the erased cells the server reported.
    pub fn apply_event(&mut self, event: TerrainDestructionEvent) -> TerrainConvergenceAction {
        let Some(expected) = self.phase_generation() else {
            return TerrainConvergenceAction::Ignored;
        };
        match classify_generation(&event.generation, &expected) {
            GenerationRelation::Same => {}
            GenerationRelation::Stale => return TerrainConvergenceAction::Ignored,
            GenerationRelation::Corrupt => {
                let reason = "live event carried a foreign terrain fingerprint".to_string();
                self.invalidate(reason.clone());
                return TerrainConvergenceAction::Invalidated(reason);
            }
        }
        if !event_shape_is_valid(&event, &self.expected_chunks) {
            return self.recover_after_corrupt_event();
        }
        match &mut self.phase {
            TerrainConvergencePhase::WaitingForMap | TerrainConvergencePhase::Invalid { .. } => {
                TerrainConvergenceAction::Ignored
            }
            TerrainConvergencePhase::AwaitingRecovery {
                generation,
                request_pending,
                buffered,
            } => {
                if buffered
                    .iter()
                    .any(|prior| prior.revision == event.revision)
                {
                    return TerrainConvergenceAction::Ignored;
                }
                if buffered.len() >= MAX_BUFFERED_TERRAIN_EVENTS {
                    buffered.clear();
                    *request_pending = false;
                    return TerrainConvergenceAction::RequestRecovery(*generation);
                }
                buffered.push(event);
                TerrainConvergenceAction::Buffered
            }
            TerrainConvergencePhase::Ready { generation } => {
                let generation = *generation;
                let Some(next) = self.revision.checked_add(1) else {
                    // The committed revision fills the whole space; every event duplicates.
                    return TerrainConvergenceAction::Ignored;
                };
                match event.revision.cmp(&next) {
                    std::cmp::Ordering::Less => TerrainConvergenceAction::Ignored,
                    std::cmp::Ordering::Greater => {
                        self.transition_to_recovery();
                        TerrainConvergenceAction::RequestRecovery(generation)
                    }
                    std::cmp::Ordering::Equal => self.commit_verified_event(event),
                }
            }
        }
    }

    /// Apply one authoritative reset event for a match restart. Accepted only when the
    /// reset chains from the currently committed generation onto the same map and the
    /// client already observes the new `MatchState.match_id`. A reset that outruns match
    /// replication holds convergence in the syncing state and recovers the post-restart
    /// generation with one bounded snapshot instead of committing terrain the match view
    /// cannot vouch for.
    pub fn apply_reset(
        &mut self,
        reset: TerrainResetEvent,
        observed: Option<TerrainGeneration>,
        initial_chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
    ) -> TerrainConvergenceAction {
        let TerrainConvergencePhase::Ready {
            generation: committed,
        } = &self.phase
        else {
            return TerrainConvergenceAction::Ignored;
        };
        let committed = *committed;
        if reset.previous_generation != committed
            || reset.next_generation.map_instance_id != committed.map_instance_id
            || reset.next_generation.terrain_fingerprint != committed.terrain_fingerprint
        {
            return TerrainConvergenceAction::Ignored;
        }
        let Some(observed) = observed else {
            return TerrainConvergenceAction::Ignored;
        };
        if observed.match_id != reset.next_generation.match_id {
            if observed != committed {
                // Neither the pre-restart nor the post-restart match: not this reset's
                // generation to apply.
                return TerrainConvergenceAction::Ignored;
            }
            // The reset outran match replication. Hold it against the committed
            // generation, leave the syncing state, and let one recovery exchange land
            // the post-restart occupancy; a later observation of the new match id keeps
            // the held request armed instead of churning to a stale one.
            self.chunks.clear();
            self.revision = 0;
            self.pending_reset = Some(reset);
            self.phase = TerrainConvergencePhase::AwaitingRecovery {
                generation: reset.next_generation,
                request_pending: false,
                buffered: Vec::new(),
            };
            return TerrainConvergenceAction::RequestRecovery(reset.next_generation);
        }
        self.pending_reset = None;
        self.chunks = initial_chunks.clone();
        self.revision = 0;
        self.dirty.extend(self.expected_chunks.iter().copied());
        self.phase = TerrainConvergencePhase::Ready {
            generation: reset.next_generation,
        };
        TerrainConvergenceAction::Applied
    }

    /// Apply one authoritative recovery snapshot. Validation failures for the expected
    /// generation are irrecoverable; buffered-event problems re-request instead. Erase-only
    /// monotonicity is part of validation: recovered occupancy must be a subset of the
    /// authored initial occupancy, and a revision-zero snapshot is exactly the initial state.
    pub fn apply_snapshot(
        &mut self,
        snapshot: &TerrainRecoverySnapshot,
        initial_chunks: &BTreeMap<TerrainChunkId, TerrainBits>,
    ) -> TerrainConvergenceAction {
        let Some(expected) = self.phase_generation() else {
            return TerrainConvergenceAction::Ignored;
        };
        if !matches!(self.phase, TerrainConvergencePhase::AwaitingRecovery { .. }) {
            // A duplicate or late snapshot must never regress committed state.
            return TerrainConvergenceAction::Ignored;
        }
        match classify_generation(&snapshot.generation, &expected) {
            GenerationRelation::Same => {}
            GenerationRelation::Stale => return TerrainConvergenceAction::Ignored,
            GenerationRelation::Corrupt => {
                let reason = "recovery snapshot carried a foreign terrain fingerprint".to_string();
                self.invalidate(reason.clone());
                return TerrainConvergenceAction::Invalidated(reason);
            }
        }
        let snapshot_chunks: BTreeMap<_, _> = snapshot
            .chunks
            .iter()
            .map(|chunk| (chunk.chunk_id, chunk.occupancy))
            .collect();
        let set_matches = snapshot.chunks.len() == snapshot_chunks.len()
            && snapshot_chunks.keys().copied().collect::<BTreeSet<_>>() == self.expected_chunks;
        let serialized = terrain_grid::recovery_snapshot_bytes(snapshot);
        if snapshot.chunks.len() > MAX_TERRAIN_CHUNKS
            || !set_matches
            || serialized.is_none_or(|bytes| bytes > MAX_TERRAIN_RECOVERY_BYTES)
        {
            let reason = if serialized.is_some_and(|bytes| bytes > MAX_TERRAIN_RECOVERY_BYTES) {
                format!("recovery snapshot exceeded {MAX_TERRAIN_RECOVERY_BYTES} serialized bytes")
            } else {
                "recovery snapshot chunk set mismatch".to_string()
            };
            self.invalidate(reason.clone());
            return TerrainConvergenceAction::Invalidated(reason);
        }
        let constructed_outside = snapshot_chunks.iter().any(|(chunk, bits)| {
            initial_chunks
                .get(chunk)
                .is_some_and(|initial| bits_construct_outside(bits, initial))
        });
        let initial_mismatch = snapshot.revision == 0 && snapshot_chunks != *initial_chunks;
        if constructed_outside || initial_mismatch {
            let reason = if constructed_outside {
                "recovery snapshot constructed occupied cells outside the authored terrain"
                    .to_string()
            } else {
                "revision-zero recovery snapshot does not equal initial occupancy".to_string()
            };
            self.invalidate(reason.clone());
            return TerrainConvergenceAction::Invalidated(reason);
        }
        // Commit the complete snapshot, then replay contiguous buffered events on top.
        self.chunks = snapshot_chunks;
        self.revision = snapshot.revision;
        self.dirty.extend(self.expected_chunks.iter().copied());
        let mut replay: Vec<_> = match &mut self.phase {
            TerrainConvergencePhase::AwaitingRecovery { buffered, .. } => core::mem::take(buffered),
            _ => Vec::new(),
        };
        replay.sort_by_key(|event| event.revision);
        replay.dedup_by_key(|event| event.revision);
        self.phase = TerrainConvergencePhase::Ready {
            generation: expected,
        };
        for event in replay {
            match self.apply_event(event) {
                TerrainConvergenceAction::Applied | TerrainConvergenceAction::Ignored => {}
                other => return other,
            }
        }
        TerrainConvergenceAction::Applied
    }

    pub(crate) fn phase_generation(&self) -> Option<TerrainGeneration> {
        match &self.phase {
            TerrainConvergencePhase::WaitingForMap => None,
            TerrainConvergencePhase::AwaitingRecovery { generation, .. }
            | TerrainConvergencePhase::Ready { generation }
            | TerrainConvergencePhase::Invalid { generation, .. } => Some(*generation),
        }
    }

    fn invalidate(&mut self, reason: String) {
        if let Some(generation) = self.phase_generation() {
            self.phase = TerrainConvergencePhase::Invalid { generation, reason };
        }
    }

    /// A revision gap or corrupt event from a Ready state: retain no guessed revision and
    /// request a full snapshot for the same generation.
    fn transition_to_recovery(&mut self) {
        if let Some(generation) = self.phase_generation() {
            self.phase = TerrainConvergencePhase::AwaitingRecovery {
                generation,
                request_pending: false,
                buffered: Vec::new(),
            };
        }
    }

    fn recover_after_corrupt_event(&mut self) -> TerrainConvergenceAction {
        if matches!(
            self.phase,
            TerrainConvergencePhase::WaitingForMap | TerrainConvergencePhase::Invalid { .. }
        ) {
            return TerrainConvergenceAction::Ignored;
        }
        self.transition_to_recovery();
        let generation = self
            .phase_generation()
            .expect("recovery transition preserves the generation");
        TerrainConvergenceAction::RequestRecovery(generation)
    }

    /// Apply one event whose revision is exactly the next expected revision, verifying
    /// the locally rasterized result against the server's erased-cell and chunk report.
    fn commit_verified_event(
        &mut self,
        event: TerrainDestructionEvent,
    ) -> TerrainConvergenceAction {
        let ((x_min, x_max), (y_min, y_max)) = terrain_grid::brush_cell_range(event.brush);
        let mut touched: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
        for cell_y in y_min..=y_max {
            for cell_x in x_min..=x_max {
                let Some((chunk, _)) = terrain_grid::cell_to_chunk_and_local((cell_x, cell_y))
                else {
                    continue;
                };
                if let Some(bits) = self.chunks.get(&chunk) {
                    touched.entry(chunk).or_insert(*bits);
                }
            }
        }
        let outcome = terrain_grid::apply_brush(&mut touched, event.brush);
        if outcome.erased_cells != event.erased_cells
            || outcome.affected_chunks != event.affected_chunks
        {
            // Never guess: disagreement with the authoritative report re-syncs fully.
            self.transition_to_recovery();
            let generation = self
                .phase_generation()
                .expect("recovery transition preserves the generation");
            return TerrainConvergenceAction::RequestRecovery(generation);
        }
        for (chunk, bits) in touched {
            self.chunks.insert(chunk, bits);
        }
        self.revision = event.revision;
        self.dirty.extend(outcome.affected_chunks);
        self.applied_brushes.push(event.brush);
        TerrainConvergenceAction::Applied
    }
}

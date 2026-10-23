//! Owner movement prediction for the locally controlled fighter.
//!
//! The server remains the only movement authority. The client runs the shared movement step
//! (`crate::movement::step`) for its own controlled fighter on the local input timeline, keeps
//! a bounded per-tick history of predicted positions, and reconciles every newly confirmed
//! authoritative pose against the prediction made for that same Lightyear tick. A misprediction
//! shifts the in-flight prediction by the error; the presented pose absorbs small errors with a
//! decaying correction offset and snaps above the tuned threshold (respawn, teleport).
//!
//! Remote fighters stay interpolated. Dash, defeat, and inactive-participant phases suspend
//! local integration and follow authority through reconciliation; slow, freeze, effect tiles,
//! adrenaline, and replicated knockback are predicted with the same rules the server uses.
//! Collision against current map blockers (including destroyed or replaced cover) uses the
//! replicated map snapshot and dynamic state for presentation only.

use crate::builds::{AbilityState, PassiveKind, PassiveRuntimeState, ResolvedMatchLoadout};
use crate::combat::{
    ActiveEffects, AuthoritativePose, Defeated, ExternalMotion, KnockbackFeedback,
};
use crate::map::{
    EffectTileOccupancy, MapCatalogResource, MapDynamicState, MapRoot, ResolvedMapSnapshot,
    resolve_circle_against_blocking_map,
};
use crate::matchplay::{MatchParticipant, MatchPhase, MatchState, RespawnState};
use crate::movement::{
    InputTuning, MovementModifiers, movement_intent, movement_is_suspended,
    resolved_movement_velocity,
};
use crate::protocol::{Fighter, FighterInput};
use avian2d::prelude::{Position, Rotation};
use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::{
    ConfirmHistory, ConfirmedHistory, Controlled, InterpolationSystems, LocalTimeline,
    ReplicationCheckpointMap, Tick,
};
use std::collections::VecDeque;

/// Predicted ticks retained for reconciliation (about one second at 60 Hz).
pub const OWNER_PREDICTION_HISTORY: usize = 64;

/// Presentation tuning for owner-prediction corrections. Client-only; never gameplay balance.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct OwnerPredictionTuning {
    /// A reconciliation error at or above this distance snaps the presented pose instead of
    /// blending (respawn, teleport, or a correction larger than ordinary forced motion).
    pub snap_distance: f32,
    /// Half-life of the presented correction offset for errors below `snap_distance`.
    pub correction_half_life_seconds: f32,
}

impl Default for OwnerPredictionTuning {
    fn default() -> Self {
        Self {
            // Two map cells: above one tick of dash (20 units) or knockback, below a respawn.
            snap_distance: 2.0 * crate::map::MAP_CELL_SIZE_WORLD,
            correction_half_life_seconds: 0.08,
        }
    }
}

/// One predicted fixed step on the local input timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PredictedStep {
    tick: Tick,
    position: Vec2,
    /// Whether this step's input committed a new facing.
    aimed: bool,
}

/// One authoritative pose paired with the Lightyear server tick at which it was confirmed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AuthoritativeSample {
    pub tick: Tick,
    pub position: Vec2,
    pub facing: f32,
}

/// How one authoritative sample changed the prediction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Reconciliation {
    /// The prediction for that tick matched or was corrected below the snap threshold.
    Smoothed { error: f32 },
    /// The correction met the snap threshold; the presented pose jumps to the new prediction.
    Snapped { error: f32 },
    /// The sample is newer than every local prediction (history overrun or the input timeline
    /// fell behind); the prediction restarts from the authoritative pose.
    Rebased { error: f32 },
    /// The sample predates every local prediction; only the server clock advanced.
    Pending,
    /// Local movement was suspended, so the authoritative motion is presented directly.
    Followed { error: f32 },
}

/// Client-only predicted pose of the controlled fighter. Never replicated or authoritative.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct OwnerPrediction {
    /// Predicted simulation position at the newest local input tick.
    pub position: Vec2,
    pub facing: f32,
    /// Presented minus predicted position; decays toward zero after a small correction.
    pub correction_offset: Vec2,
    history: VecDeque<PredictedStep>,
    /// Whether the newest predicted step suspended local movement.
    following_authority: bool,
    last_sample_tick: Option<Tick>,
    /// The newest (Lightyear tick, server simulation tick) pair from `AuthoritativePose`, used
    /// to estimate the server simulation tick that status/tile/knockback deadlines refer to.
    server_clock: Option<(Tick, u64)>,
}

impl OwnerPrediction {
    #[must_use]
    pub fn from_pose(position: Vec2, facing: f32) -> Self {
        Self {
            position,
            facing,
            correction_offset: Vec2::ZERO,
            history: VecDeque::with_capacity(OWNER_PREDICTION_HISTORY),
            following_authority: false,
            last_sample_tick: None,
            server_clock: None,
        }
    }

    /// The pose the client presents for its own fighter.
    #[must_use]
    pub fn presented_position(&self) -> Vec2 {
        self.position + self.correction_offset
    }

    /// Estimated server simulation tick for a local input tick, once a sample has arrived.
    #[must_use]
    fn simulation_tick_at(&self, tick: Tick) -> Option<u64> {
        self.server_clock.map(|(confirmed, simulation)| {
            simulation.saturating_add_signed(i64::from(tick - confirmed))
        })
    }

    /// Whether a sample confirmed at `tick` is newer than every sample already folded in.
    #[must_use]
    fn accepts(&self, tick: Tick) -> bool {
        self.last_sample_tick.is_none_or(|last| tick - last > 0)
    }

    /// Record which server simulation tick an authoritative pose confirmed at `tick` carried.
    pub(crate) fn observe_server_clock(&mut self, tick: Tick, simulation_tick: u64) {
        self.server_clock = Some((tick, simulation_tick));
    }

    /// Fold one authoritative sample into the prediction.
    pub(crate) fn reconcile(
        &mut self,
        sample: AuthoritativeSample,
        tuning: OwnerPredictionTuning,
    ) -> Reconciliation {
        self.last_sample_tick = Some(sample.tick);
        let predicted_at = self
            .history
            .iter()
            .find(|step| step.tick == sample.tick)
            .map(|step| step.position);
        if predicted_at.is_none()
            && self
                .history
                .front()
                .is_some_and(|oldest| oldest.tick - sample.tick > 0)
        {
            // The sample predates every local prediction (prediction just started or the
            // input timeline moved ahead); its round trip has not completed yet.
            return Reconciliation::Pending;
        }
        self.history.retain(|step| step.tick - sample.tick > 0);
        let outcome = if let Some(predicted) = predicted_at {
            let error = sample.position - predicted;
            self.position += error;
            for step in &mut self.history {
                step.position += error;
            }
            self.present_correction(error, tuning, false)
        } else {
            let error = sample.position - self.position;
            self.history.clear();
            self.position = sample.position;
            self.present_correction(error, tuning, true)
        };
        if !self.history.iter().any(|step| step.aimed) {
            self.facing = sample.facing;
        }
        outcome
    }

    fn present_correction(
        &mut self,
        error: Vec2,
        tuning: OwnerPredictionTuning,
        rebased: bool,
    ) -> Reconciliation {
        let magnitude = error.length();
        if magnitude >= tuning.snap_distance {
            self.correction_offset = Vec2::ZERO;
            return Reconciliation::Snapped { error: magnitude };
        }
        if self.following_authority {
            // Suspended movement (dash, defeat, inactive): the error is authoritative motion
            // the client never predicted, so present it as it arrives instead of smoothing.
            return Reconciliation::Followed { error: magnitude };
        }
        self.correction_offset -= error;
        if rebased {
            Reconciliation::Rebased { error: magnitude }
        } else {
            Reconciliation::Smoothed { error: magnitude }
        }
    }

    /// Record the prediction for one local input tick and decay the presented correction.
    pub(crate) fn record_step(
        &mut self,
        tick: Tick,
        position: Vec2,
        aim: Option<Vec2>,
        tuning: OwnerPredictionTuning,
        delta_seconds: f32,
    ) {
        self.position = position;
        if let Some(aim) = aim {
            self.facing = aim.y.atan2(aim.x);
        }
        self.history.push_back(PredictedStep {
            tick,
            position,
            aimed: aim.is_some(),
        });
        while self.history.len() > OWNER_PREDICTION_HISTORY {
            self.history.pop_front();
        }
        let half_life = tuning.correction_half_life_seconds.max(f32::EPSILON);
        self.correction_offset *= 0.5_f32.powf(delta_seconds / half_life);
        if self.correction_offset.length_squared() < 1e-4 {
            self.correction_offset = Vec2::ZERO;
        }
    }
}

/// Bounded measured facts for tests and diagnostics.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct OwnerPredictionStats {
    pub predicted_ticks: u64,
    pub reconciliations: u64,
    pub smoothed_corrections: u64,
    pub snaps: u64,
    pub rebases: u64,
    pub pending: u64,
    pub followed: u64,
    pub last_error: f32,
    pub max_smoothed_error: f32,
}

impl OwnerPredictionStats {
    fn record(&mut self, outcome: Reconciliation) {
        self.reconciliations = self.reconciliations.saturating_add(1);
        match outcome {
            Reconciliation::Smoothed { error } => {
                self.last_error = error;
                if error > 0.0 {
                    self.smoothed_corrections = self.smoothed_corrections.saturating_add(1);
                }
                self.max_smoothed_error = self.max_smoothed_error.max(error);
            }
            Reconciliation::Snapped { error } => {
                self.last_error = error;
                self.snaps = self.snaps.saturating_add(1);
            }
            Reconciliation::Rebased { error } => {
                self.last_error = error;
                self.rebases = self.rebases.saturating_add(1);
            }
            Reconciliation::Pending => self.pending = self.pending.saturating_add(1),
            Reconciliation::Followed { error } => {
                self.last_error = error;
                self.followed = self.followed.saturating_add(1);
            }
        }
    }
}

/// Ordering of owner prediction inside the client's fixed simulation.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnerPredictionSet {
    Attach,
    Reconcile,
    Predict,
}

/// Predicts and presents the local fighter's movement on every client.
pub struct OwnerPredictionPlugin;

impl Plugin for OwnerPredictionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputTuning>()
            .init_resource::<OwnerPredictionTuning>()
            .init_resource::<OwnerPredictionStats>()
            .configure_sets(
                FixedUpdate,
                (
                    OwnerPredictionSet::Attach,
                    OwnerPredictionSet::Reconcile,
                    OwnerPredictionSet::Predict,
                )
                    .chain()
                    .in_set(crate::gameplay::GameplaySet::Simulation),
            )
            .add_systems(
                FixedUpdate,
                (
                    attach_owner_prediction.in_set(OwnerPredictionSet::Attach),
                    ApplyDeferred
                        .after(OwnerPredictionSet::Attach)
                        .before(OwnerPredictionSet::Reconcile),
                    reconcile_owner_prediction.in_set(OwnerPredictionSet::Reconcile),
                    predict_owner_movement.in_set(OwnerPredictionSet::Predict),
                ),
            )
            // Lightyear interpolation writes every replicated fighter's pose in Update; the
            // owner's presented pose replaces it afterwards, before any PostUpdate presentation.
            .add_systems(
                Update,
                present_owner_prediction.after(InterpolationSystems::All),
            );
    }
}

/// Start predicting from the controlled fighter's first authoritative pose.
#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
fn attach_owner_prediction(
    mut commands: Commands,
    owners: Query<
        (Entity, &AuthoritativePose),
        (With<Fighter>, With<Controlled>, Without<OwnerPrediction>),
    >,
) {
    for (entity, pose) in &owners {
        commands.entity(entity).insert(OwnerPrediction::from_pose(
            Vec2::new(pose.position.x, pose.position.y),
            pose.facing,
        ));
    }
}

/// Fold the newest confirmed authoritative pose into the prediction.
///
/// Replicon confirms the whole fighter entity at a Lightyear server tick; the raw replicated
/// `Position`/`Rotation` histories (kept by interpolation) hold the latest authoritative values
/// at or before that tick, including dash, knockback, and respawn motion that
/// `AuthoritativePose` does not record. `AuthoritativePose` supplies the server simulation tick.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn reconcile_owner_prediction(
    tuning: Res<OwnerPredictionTuning>,
    checkpoints: Res<ReplicationCheckpointMap>,
    mut stats: ResMut<OwnerPredictionStats>,
    mut owners: Query<
        (
            &ConfirmHistory,
            &ConfirmedHistory<Position>,
            Option<&ConfirmedHistory<Rotation>>,
            Ref<AuthoritativePose>,
            &mut OwnerPrediction,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    for (confirm_history, positions, rotations, pose, mut prediction) in &mut owners {
        let Some(tick) = checkpoints.get(confirm_history.last_tick()) else {
            continue;
        };
        if pose.is_changed() {
            prediction.observe_server_clock(tick, pose.tick);
        }
        if !prediction.accepts(tick) {
            continue;
        }
        let Some(position) = positions.get_present(tick) else {
            continue;
        };
        let facing = rotations
            .and_then(|rotations| rotations.get_present(tick))
            .map_or(prediction.facing, |rotation| rotation.as_radians());
        let outcome = prediction.reconcile(
            AuthoritativeSample {
                tick,
                position: position.0,
                facing,
            },
            *tuning,
        );
        stats.record(outcome);
    }
}

/// Advance the owner's prediction one local input tick with the shared movement step and the
/// current replicated map blockers.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn predict_owner_movement(
    timeline: Res<LocalTimeline>,
    time: Res<Time<Fixed>>,
    tuning: Res<OwnerPredictionTuning>,
    input_tuning: Res<InputTuning>,
    builds: Res<crate::builds::BuildCatalogResource>,
    catalog: Res<MapCatalogResource>,
    mut stats: ResMut<OwnerPredictionStats>,
    maps: Query<(&ResolvedMapSnapshot, &MapDynamicState), With<MapRoot>>,
    matches: Query<&MatchState>,
    mut owners: Query<
        (
            &mut OwnerPrediction,
            Option<&ActionState<FighterInput>>,
            Option<&ResolvedMatchLoadout>,
            (
                Option<&ActiveEffects>,
                Option<&EffectTileOccupancy>,
                Option<&PassiveRuntimeState>,
                Option<&KnockbackFeedback>,
            ),
            (
                Option<&AbilityState>,
                Has<Defeated>,
                Has<MatchParticipant>,
                Has<RespawnState>,
            ),
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let tick = timeline.tick();
    let delta = time.delta().as_secs_f32();
    let map = maps.single().ok();
    let match_active = matches
        .iter()
        .next()
        .is_none_or(|state| matches!(state.phase, MatchPhase::Active { .. }));
    for (
        mut prediction,
        action,
        loadout,
        (effects, tile, passive_state, knockback),
        (ability, defeated, participant, respawning),
    ) in &mut owners
    {
        let intent = movement_intent(
            action.map_or(FighterInput::default(), |action| action.0),
            &input_tuning,
        );
        let suspended = movement_is_suspended(
            ability,
            defeated,
            participant && (!match_active || respawning),
        );
        let simulation_tick = prediction.simulation_tick_at(tick);
        let mut position = prediction.position;
        if let (false, Some(loadout), Some(simulation_tick)) = (suspended, loadout, simulation_tick)
        {
            let velocity = resolved_movement_velocity(
                simulation_tick,
                intent.movement,
                loadout.fighter_stats.movement_speed,
                MovementModifiers {
                    active_effects: effects,
                    effect_tile: tile,
                    adrenal_response: loadout
                        .passives
                        .iter()
                        .find(|passive| passive.kind == PassiveKind::AdrenalResponse)
                        .copied(),
                    passive_state,
                    external_motion: knockback.map(|knockback| ExternalMotion {
                        velocity: Vec2::new(knockback.velocity.x, knockback.velocity.y),
                        expires_at_tick: knockback.expires_at_tick,
                    }),
                },
            );
            position += velocity * delta;
        }
        if let Some((snapshot, state)) = map {
            position = resolve_circle_against_blocking_map(
                position,
                builds.0.fighter_body.radius,
                snapshot,
                state,
                &catalog.0,
            );
        }
        let aim = (!suspended).then_some(intent.aim).flatten();
        prediction.following_authority = suspended;
        prediction.record_step(tick, position, aim, *tuning, delta);
        stats.predicted_ticks = stats.predicted_ticks.saturating_add(1);
    }
}

/// Present the predicted owner pose in place of the interpolated one.
#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
fn present_owner_prediction(
    mut owners: Query<
        (&OwnerPrediction, &mut Position, &mut Rotation),
        (With<Fighter>, With<Controlled>),
    >,
) {
    for (prediction, mut position, mut rotation) in &mut owners {
        position.0 = prediction.presented_position();
        *rotation = Rotation::radians(prediction.facing);
    }
}

#[cfg(test)]
mod tests;

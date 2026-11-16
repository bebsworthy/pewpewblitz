//! Owner movement prediction for the locally controlled fighter.
//!
//! The server remains the only movement authority. The client runs the shared movement step
//! (`brawler::movement::step`) for its own controlled fighter on the local input timeline, keeps
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

use avian2d::prelude::{Position, Rotation};
use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use brawler::builds::{AbilityState, PassiveKind, PassiveRuntimeState, ResolvedMatchLoadout};
use brawler::combat::{
    ActiveEffects, AuthoritativePose, Defeated, ExternalMotion, KnockbackFeedback,
};
use brawler::map::{
    EffectTileOccupancy, MapCatalogResource, MapDynamicState, MapRoot, ResolvedMapSnapshot,
    resolve_circle_against_blocking_map,
};
use brawler::matchplay::{MatchParticipant, MatchPhase, MatchState, RespawnState};
use brawler::movement::{
    InputTuning, MovementModifiers, MovementTuning, movement_intent, movement_is_suspended,
    resolved_movement_velocity,
};
use brawler::protocol::{Fighter, FighterInput};
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
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
            snap_distance: 2.0 * brawler::map::MAP_CELL_SIZE_WORLD,
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

/// One locally re-simulated fixed step: the input timeline tick's predicted position, the facing
/// its input committed, and whether movement was suspended for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ReplayedStep {
    pub position: Vec2,
    pub aim: Option<Vec2>,
    pub suspended: bool,
}

/// Client-only predicted pose of the controlled fighter. Never replicated or authoritative.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct OwnerPrediction {
    /// Predicted simulation position at the newest local input tick.
    pub position: Vec2,
    pub facing: f32,
    /// Presented minus predicted position; decays toward zero after a small correction.
    pub correction_offset: Vec2,
    /// Realized (post-collision) velocity of the newest predicted step; zero while following
    /// authority. Presentation extrapolates it across the fixed-step overstep.
    step_velocity: Vec2,
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
            step_velocity: Vec2::ZERO,
            history: VecDeque::with_capacity(OWNER_PREDICTION_HISTORY),
            following_authority: false,
            last_sample_tick: None,
            server_clock: None,
        }
    }

    /// The pose the client presents for its own fighter at a fixed-tick boundary.
    #[must_use]
    pub fn presented_position(&self) -> Vec2 {
        self.position + self.correction_offset
    }

    /// The pose presented `overstep` after the newest predicted tick: a partial-tick
    /// extrapolation of the newest step, resolved by `resolve_body` (the same planar map
    /// resolution the prediction step uses), plus the correction offset decayed for that time.
    ///
    /// Presentation only. `overstep` is clamped to one `fixed_step`, so a stalled fixed loop
    /// never extrapolates further than one tick of travel; at zero it equals
    /// [`Self::presented_position`] exactly. Resolving the extrapolation stops the presented
    /// pose at the wall contact the next tick will realize instead of overshooting into the
    /// wall and snapping back.
    #[must_use]
    pub fn presented_position_at(
        &self,
        overstep: f32,
        fixed_step: f32,
        tuning: OwnerPredictionTuning,
        resolve_body: impl FnOnce(Vec2) -> Vec2,
    ) -> Vec2 {
        let overstep = overstep.clamp(0.0, fixed_step.max(0.0));
        if overstep <= 0.0 {
            return self.presented_position();
        }
        let extrapolated = if self.step_velocity == Vec2::ZERO {
            self.position
        } else {
            resolve_body(self.position + self.step_velocity * overstep)
        };
        let half_life = tuning.correction_half_life_seconds.max(f32::EPSILON);
        extrapolated + self.correction_offset * 0.5_f32.powf(overstep / half_life)
    }

    /// Realized velocity of the newest predicted step (zero while following authority).
    #[cfg(any(test, feature = "automation"))]
    #[must_use]
    pub(crate) const fn step_velocity(&self) -> Vec2 {
        self.step_velocity
    }

    /// Estimated server simulation tick for a local input tick, once a sample has arrived.
    #[must_use]
    pub(crate) fn simulation_tick_at(&self, tick: Tick) -> Option<u64> {
        simulation_tick_at(self.server_clock, tick)
    }

    /// The Lightyear tick of the newest authoritative sample folded into the prediction.
    #[must_use]
    pub(crate) const fn last_confirmed_tick(&self) -> Option<Tick> {
        self.last_sample_tick
    }

    /// Whether the newest predicted step suspended local movement (dash, defeat, inactive).
    #[must_use]
    pub(crate) const fn is_following_authority(&self) -> bool {
        self.following_authority
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

    /// Fold one authoritative sample into the prediction without re-simulating any input.
    #[cfg(test)]
    pub(crate) fn reconcile(
        &mut self,
        sample: AuthoritativeSample,
        tuning: OwnerPredictionTuning,
    ) -> Reconciliation {
        self.reconcile_replaying(sample, tuning, sample.tick + 1, |_, position| {
            ReplayedStep {
                position,
                aim: None,
                suspended: true,
            }
        })
    }

    /// Fold one authoritative sample into the prediction before predicting the `current` input
    /// timeline tick.
    ///
    /// A sample without a matching prediction (the first sample after prediction attaches, or a
    /// history overrun) rebases onto authority and then re-simulates, with `replay`, every
    /// local input tick after the sample and before `current`: the server applies those inputs
    /// too, so starting the prediction from the sample at `current` would drop their motion and
    /// reconcile it later as a correction (the first walk after a match starts).
    pub(crate) fn reconcile_replaying(
        &mut self,
        sample: AuthoritativeSample,
        tuning: OwnerPredictionTuning,
        current: Tick,
        mut replay: impl FnMut(Tick, Vec2) -> ReplayedStep,
    ) -> Reconciliation {
        let attached_pose = self.last_sample_tick.is_none().then_some(self.position);
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
            self.history.clear();
            let mut position = sample.position;
            let gap = current - sample.tick;
            if gap > 1 && usize::try_from(gap).is_ok_and(|gap| gap <= OWNER_PREDICTION_HISTORY) {
                let mut tick = sample.tick + 1;
                while current - tick > 0 {
                    let step = replay(tick, position);
                    position = step.position;
                    self.following_authority = step.suspended;
                    if let Some(aim) = step.aim {
                        self.facing = aim.y.atan2(aim.x);
                    }
                    self.history.push_back(PredictedStep {
                        tick,
                        position,
                        aimed: step.aim.is_some(),
                    });
                    tick = tick + 1;
                }
            }
            // The first sample measures the attach pose against authority; the replayed inputs
            // are prediction, presented as such. A later rebase compares like with like: the
            // replayed and the previous prediction for the same input tick.
            let error = attached_pose.map_or(position - self.position, |attached| {
                sample.position - attached
            });
            self.position = position;
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
        self.step_velocity = if self.following_authority || delta_seconds <= 0.0 {
            Vec2::ZERO
        } else {
            (position - self.position) / delta_seconds
        };
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

/// Estimated server simulation tick for a local input tick from a (confirmed tick, simulation
/// tick) server clock pair.
fn simulation_tick_at(server_clock: Option<(Tick, u64)>, tick: Tick) -> Option<u64> {
    server_clock.map(|(confirmed, simulation)| {
        simulation.saturating_add_signed(i64::from(tick - confirmed))
    })
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
            .init_resource::<MovementTuning>()
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
                    .in_set(brawler::gameplay::GameplaySet::Simulation),
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

/// The world the shared owner movement step reads: fixed time, tuning, builds, and the current
/// replicated map blockers and match phase.
#[derive(SystemParam)]
struct OwnerStepWorld<'w, 's> {
    time: Res<'w, Time<Fixed>>,
    input_tuning: Res<'w, InputTuning>,
    movement_tuning: Res<'w, MovementTuning>,
    builds: Res<'w, brawler::builds::BuildCatalogResource>,
    catalog: Res<'w, MapCatalogResource>,
    maps: Query<'w, 's, (&'static ResolvedMapSnapshot, &'static MapDynamicState), With<MapRoot>>,
    matches: Query<'w, 's, &'static MatchState>,
}

/// The controlled fighter's replicated movement state the shared step reads.
#[derive(QueryData)]
struct OwnerStepState {
    loadout: Option<&'static ResolvedMatchLoadout>,
    effects: Option<&'static ActiveEffects>,
    tile: Option<&'static EffectTileOccupancy>,
    passive_state: Option<&'static PassiveRuntimeState>,
    knockback: Option<&'static KnockbackFeedback>,
    ability: Option<&'static AbilityState>,
    defeated: Has<Defeated>,
    participant: Has<MatchParticipant>,
    respawning: Has<RespawnState>,
}

impl OwnerStepWorld<'_, '_> {
    /// One owner movement step from `position` with `input`, using the shared movement rules
    /// and the current map blockers.
    fn step(
        &self,
        state: &OwnerStepStateItem<'_, '_>,
        simulation_tick: Option<u64>,
        input: FighterInput,
        mut position: Vec2,
    ) -> ReplayedStep {
        let delta = self.time.delta().as_secs_f32();
        let match_active = self
            .matches
            .iter()
            .next()
            .is_none_or(|state| matches!(state.phase, MatchPhase::Active { .. }));
        let intent = movement_intent(input, &self.input_tuning);
        let suspended = movement_is_suspended(
            state.ability,
            state.defeated,
            state.participant && (!match_active || state.respawning),
        );
        if let (false, Some(loadout), Some(simulation_tick)) =
            (suspended, state.loadout, simulation_tick)
        {
            let velocity = resolved_movement_velocity(
                simulation_tick,
                intent.movement,
                loadout.fighter_stats.movement_speed,
                MovementModifiers {
                    active_effects: state.effects,
                    effect_tile: state.tile,
                    adrenal_response: loadout
                        .passives
                        .iter()
                        .find(|passive| passive.kind == PassiveKind::AdrenalResponse)
                        .copied(),
                    passive_state: state.passive_state,
                    external_motion: state.knockback.map(|knockback| ExternalMotion {
                        velocity: Vec2::new(knockback.velocity.x, knockback.velocity.y),
                        expires_at_tick: knockback.expires_at_tick,
                    }),
                },
            );
            position += velocity * delta;
        }
        if let Ok((snapshot, map_state)) = self.maps.single() {
            position = resolve_circle_against_blocking_map(
                position,
                owner_contact_radius(&self.builds, &self.movement_tuning),
                snapshot,
                map_state,
                &self.catalog.0,
            );
        }
        ReplayedStep {
            position,
            aim: (!suspended).then_some(intent.aim).flatten(),
            suspended,
        }
    }
}

/// Fold the newest confirmed authoritative pose into the prediction.
///
/// Replicon confirms the whole fighter entity at a Lightyear server tick; the raw replicated
/// `Position`/`Rotation` histories (kept by interpolation) hold the latest authoritative values
/// at or before that tick, including dash, knockback, and respawn motion that
/// `AuthoritativePose` does not record. `AuthoritativePose` supplies the server simulation tick.
/// A rebase re-simulates the local inputs (from the input buffer) after the sample tick.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn reconcile_owner_prediction(
    timeline: Res<LocalTimeline>,
    tuning: Res<OwnerPredictionTuning>,
    checkpoints: Res<ReplicationCheckpointMap>,
    step_world: OwnerStepWorld,
    mut stats: ResMut<OwnerPredictionStats>,
    mut owners: Query<
        (
            &ConfirmHistory,
            &ConfirmedHistory<Position>,
            Option<&ConfirmedHistory<Rotation>>,
            Ref<AuthoritativePose>,
            &mut OwnerPrediction,
            Option<&NativeBuffer<FighterInput>>,
            OwnerStepState,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let current = timeline.tick();
    for (confirm_history, positions, rotations, pose, mut prediction, inputs, state) in &mut owners
    {
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
        let clock = prediction.server_clock;
        let outcome = prediction.reconcile_replaying(
            AuthoritativeSample {
                tick,
                position: position.0,
                facing,
            },
            *tuning,
            current,
            |input_tick, from| {
                let input = inputs
                    .and_then(|inputs| inputs.get(input_tick))
                    .map_or(FighterInput::default(), |action| action.0);
                step_world.step(&state, simulation_tick_at(clock, input_tick), input, from)
            },
        );
        stats.record(outcome);
    }
}

/// Advance the owner's prediction one local input tick with the shared movement step and the
/// current replicated map blockers.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn predict_owner_movement(
    timeline: Res<LocalTimeline>,
    tuning: Res<OwnerPredictionTuning>,
    step_world: OwnerStepWorld,
    mut stats: ResMut<OwnerPredictionStats>,
    mut owners: Query<
        (
            &mut OwnerPrediction,
            Option<&ActionState<FighterInput>>,
            OwnerStepState,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let tick = timeline.tick();
    let delta = step_world.time.delta().as_secs_f32();
    for (mut prediction, action, state) in &mut owners {
        let step = step_world.step(
            &state,
            prediction.simulation_tick_at(tick),
            action.map_or(FighterInput::default(), |action| action.0),
            prediction.position,
        );
        prediction.following_authority = step.suspended;
        prediction.record_step(tick, step.position, step.aim, *tuning, delta);
        stats.predicted_ticks = stats.predicted_ticks.saturating_add(1);
    }
}

/// Radius the owner prediction resolves map contact with: the fighter body plus the clearance
/// authoritative move-and-slide keeps from every blocker. Resolving at the bare body radius
/// rests the prediction one skin deeper than authority, so every contact tick reconciles a
/// correction that vibrates the presented fighter into the wall and bounces it back on release.
fn owner_contact_radius(
    builds: &brawler::builds::BuildCatalogResource,
    movement_tuning: &MovementTuning,
) -> f32 {
    builds.0.fighter_body.radius + movement_tuning.contact_skin_world()
}

/// Present the predicted owner pose in place of the interpolated one.
///
/// Runs every rendered frame after the fixed loop, so `Time<Fixed>::overstep` is the time since
/// the newest predicted tick; the partial-tick extrapolation keeps the owner (and the camera that
/// follows it) moving on frames without a fixed tick, without delaying the input tick's motion.
/// The extrapolation is resolved against the same replicated map blockers as prediction, so the
/// presented pose never runs ahead into a wall the next tick will stop at.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn present_owner_prediction(
    time: Res<Time<Fixed>>,
    tuning: Res<OwnerPredictionTuning>,
    movement_tuning: Res<MovementTuning>,
    builds: Res<brawler::builds::BuildCatalogResource>,
    catalog: Res<MapCatalogResource>,
    maps: Query<(&ResolvedMapSnapshot, &MapDynamicState), With<MapRoot>>,
    mut owners: Query<
        (&OwnerPrediction, &mut Position, &mut Rotation),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let overstep = time.overstep().as_secs_f32();
    let fixed_step = time.timestep().as_secs_f32();
    let map = maps.single().ok();
    let radius = owner_contact_radius(&builds, &movement_tuning);
    for (prediction, mut position, mut rotation) in &mut owners {
        position.0 = prediction.presented_position_at(overstep, fixed_step, *tuning, |body| {
            map.map_or(body, |(snapshot, state)| {
                resolve_circle_against_blocking_map(body, radius, snapshot, state, &catalog.0)
            })
        });
        *rotation = Rotation::radians(prediction.facing);
    }
}

#[cfg(test)]
mod tests;

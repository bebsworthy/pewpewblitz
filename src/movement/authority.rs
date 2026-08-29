//! Server-authoritative movement coordinator and input-message validation.
//!
//! `authoritative_movement` remains one fixed-tick coordinator: eligibility gating, input
//! freshness, aim commitment, modifier resolution, Avian move-and-slide, defensive pose
//! repair, and the deferred component commit are visible in one schedule-facing function.
//! Focused decision helpers stay pure so ownership and ordering remain reviewable.

use avian2d::prelude::*;
use bevy::prelude::*;
use lightyear::input::input_buffer::Compressed;
use lightyear::input::input_message::ActionStateSequence;
use lightyear::input::server::{InputValidationAppExt, authorize_controlled_targets};
use lightyear::prelude::input::native::{ActionState, NativeBuffer, NativeStateSequence};
use lightyear::prelude::{ControlledBy, LocalTimeline, MessageReceiver};

use crate::combat::AuthoritativePose;
use crate::protocol::{Fighter, FighterInput};
use crate::timing::{SIMULATION_TICK, SimulationTick};

use super::input::{
    InputFreshness, InputTuning, InputValidationState, committed_aim, decoded_input_is_valid,
    decoded_move, input_end_tick_is_acceptable, input_history_len_is_valid,
    input_sequence_ends_with_present_state, input_should_neutralize, input_target_is_entity,
    latest_present_remote_tick,
};
use super::{
    AuthoritativeInputTrace, DESTRUCTIBLE_MAP_LAYER, MovementTuning, STATIC_MAP_LAYER,
    pose_is_valid,
};

/// One fighter's per-tick movement decision plus the freshness value to commit.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MovementDecision {
    pub freshness: InputFreshness,
    pub stale: bool,
    pub movement: Vec2,
    pub aim: Option<Vec2>,
    /// Raw dequantized move axis and button mask, kept for the input-change trace only.
    pub trace_input: (Vec2, u8),
}

#[derive(Clone, Copy, Default)]
pub(crate) struct MovementModifiers<'a> {
    pub active_effects: Option<&'a crate::combat::ActiveEffects>,
    pub effect_tile: Option<&'a crate::map::EffectTileOccupancy>,
    pub passive_state: Option<&'a crate::builds::PassiveRuntimeState>,
    pub external_motion: Option<&'a crate::combat::ExternalMotion>,
}

/// Pure freshness/eligibility/aim/movement decision shared by the coordinator and tests.
///
/// Returns the updated local freshness copy so the coordinator commits exactly the value
/// the decision was computed from.
#[must_use]
pub(crate) fn movement_decision(
    tick: u64,
    freshness: &InputFreshness,
    action: Option<&ActionState<FighterInput>>,
    buffer: Option<&NativeBuffer<FighterInput>>,
    tuning: &InputTuning,
    stale_input_ticks: u64,
) -> MovementDecision {
    let mut freshness = *freshness;
    if let Some(remote_tick) = buffer.and_then(latest_present_remote_tick)
        && freshness
            .last_fresh_tick
            .is_none_or(|last| remote_tick > last)
    {
        freshness.last_fresh_tick = Some(remote_tick);
    }
    let stale = input_should_neutralize(tick, freshness.last_fresh_tick, stale_input_ticks);
    let input = action.map_or(FighterInput::default(), |action| action.0);
    let input = if !stale && input.is_valid() {
        input
    } else {
        FighterInput::default()
    };
    MovementDecision {
        freshness,
        stale,
        movement: decoded_move(input.move_axis, *tuning),
        aim: input
            .aim_update
            .and_then(|axis| committed_aim(axis.to_vec2(), *tuning)),
        trace_input: (input.move_axis.to_vec2(), input.gameplay_buttons),
    }
}

/// Resolve the per-tick desired velocity from loadout, effects, adrenaline, and external
/// motion.
#[must_use]
pub(crate) fn resolved_movement_velocity(
    tick: u64,
    decision: &MovementDecision,
    loadout_speed: Option<f32>,
    base_speed: f32,
    modifiers: MovementModifiers<'_>,
) -> Vec2 {
    let movement_multiplier = modifiers
        .active_effects
        .and_then(|effects| effects.slow)
        .filter(|slow| tick < slow.expires_at_tick)
        .map_or(1.0, |slow| {
            f32::from(slow.movement_multiplier_milli) / 1000.0
        });
    let resolved_speed = loadout_speed.unwrap_or(base_speed);
    let tile_multiplier = modifiers.effect_tile.map_or(1.0, |occupancy| {
        f32::from(occupancy.behavior.movement_multiplier_milli()) / 1000.0
    });
    let adrenaline_multiplier = modifiers
        .passive_state
        .and_then(|state| state.adrenaline_until_tick)
        .filter(|deadline| tick < *deadline)
        .map_or(1.0, |_| 1.15);
    decision.movement
        * resolved_speed
        * movement_multiplier
        * tile_multiplier
        * adrenaline_multiplier
        + modifiers
            .external_motion
            .filter(|motion| tick < motion.expires_at_tick)
            .map_or(Vec2::ZERO, |motion| motion.velocity)
}

/// Defensive repair for an invalid authoritative pose. In a valid v1 scenario this never
/// runs, because destruction only removes solidity; any use is a test failure signal.
#[must_use]
pub(crate) fn repaired_pose(
    position: Vec2,
    facing: f32,
    bounds: &crate::map::PlayableBounds,
    radius: f32,
    spawn_facing: f32,
) -> (Vec2, f32) {
    let repaired_position = if position.is_finite() {
        bounds.0.clamp_circle(position, radius)
    } else {
        bounds.0.center()
    };
    let repaired_facing = if facing.is_finite() {
        facing
    } else {
        spawn_facing
    };
    (repaired_position, repaired_facing)
}

/// Install the server input validators around Lightyear's target authorization.
pub(super) fn install_input_validators(app: &mut App) {
    app.add_input_validator(authorize_controlled_targets::<NativeStateSequence<FighterInput>>)
        .add_input_validator(
            record_unauthorized_input_targets
                .before(authorize_controlled_targets::<NativeStateSequence<FighterInput>>),
        )
        .add_input_validator(
            validate_fighter_input_messages
                .after(authorize_controlled_targets::<NativeStateSequence<FighterInput>>),
        );
}

#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn record_unauthorized_input_targets(
    mut receivers: Query<(
        Entity,
        &mut InputValidationState,
        &mut MessageReceiver<
            lightyear::input::input_message::InputMessage<NativeStateSequence<FighterInput>>,
        >,
    )>,
    controlled: Query<(Entity, &ControlledBy)>,
) {
    for (connection, mut state, mut receiver) in &mut receivers {
        receiver.retain_messages(|message| {
            for target in &message.inputs {
                if let lightyear::input::input_message::InputTarget::Entity(entity) = target.target
                {
                    let authorized = controlled.iter().any(|(controlled_entity, controlled)| {
                        controlled.owner == connection && controlled_entity == entity
                    });
                    if !authorized {
                        state.ownership_rejections = state.ownership_rejections.saturating_add(1);
                    }
                }
            }
            true
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn validate_fighter_input_messages(
    timeline: Res<LocalTimeline>,
    time: Res<Time<Real>>,
    input_tuning: Res<InputTuning>,
    mut receivers: Query<(
        &mut InputValidationState,
        &mut MessageReceiver<
            lightyear::input::input_message::InputMessage<NativeStateSequence<FighterInput>>,
        >,
    )>,
) {
    let now = time.elapsed_secs();
    let server_tick = i64::from(timeline.tick().0);
    for (mut state, mut receiver) in &mut receivers {
        let elapsed = (now - state.last_refill_seconds).max(0.0);
        state.tokens =
            (state.tokens + elapsed * input_tuning.input_rate).min(input_tuning.input_burst);
        state.last_refill_seconds = now;
        receiver.retain_messages(|message| {
            let Some(target) = message.inputs.first() else {
                state.target_rejections = state.target_rejections.saturating_add(1);
                return false;
            };
            if message.inputs.len() != 1 || !input_target_is_entity(target.target) {
                state.target_rejections = state.target_rejections.saturating_add(1);
                return false;
            }
            if !input_history_len_is_valid(target.states.len(), *input_tuning) {
                state.malformed_rejections = state.malformed_rejections.saturating_add(1);
                return false;
            }

            if !input_sequence_ends_with_present_state(
                target
                    .states
                    .clone()
                    .get_snapshots_from_message(SIMULATION_TICK),
            ) {
                state.malformed_rejections = state.malformed_rejections.saturating_add(1);
                return false;
            }

            let valid_states = target
                .states
                .clone()
                .get_snapshots_from_message(SIMULATION_TICK)
                .all(|state| match state {
                    Compressed::Input(state) => decoded_input_is_valid(state.0),
                    Compressed::Absent | Compressed::SameAsPrecedent => true,
                });
            if !valid_states {
                state.malformed_rejections = state.malformed_rejections.saturating_add(1);
                return false;
            }

            let end_tick = i64::from(message.end_tick.0);
            if !input_end_tick_is_acceptable(
                end_tick,
                server_tick,
                state.last_accepted_end_tick,
                state.tokens,
                *input_tuning,
            ) {
                if end_tick < server_tick + input_tuning.min_tick_delta
                    || end_tick > server_tick + input_tuning.max_tick_delta
                {
                    state.old_or_future_rejections =
                        state.old_or_future_rejections.saturating_add(1);
                } else if state
                    .last_accepted_end_tick
                    .is_some_and(|last| end_tick <= i64::from(last))
                {
                    state.stale_or_reordered_rejections =
                        state.stale_or_reordered_rejections.saturating_add(1);
                } else {
                    state.rate_rejections = state.rate_rejections.saturating_add(1);
                }
                return false;
            }

            state.tokens -= 1.0;
            state.last_accepted_end_tick = Some(message.end_tick.0);
            true
        });
    }
}

/// Fixed-tick authoritative movement coordinator. One system by design: intermediate
/// movement state must never become visible to combat or mode systems mid-tick.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn authoritative_movement(
    mut commands: Commands,
    mut trace: ResMut<AuthoritativeInputTrace>,
    time: Res<Time<Fixed>>,
    tick: Res<SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    tuning: Res<MovementTuning>,
    input_tuning: Res<InputTuning>,
    move_and_slide: MoveAndSlide,
    fighters: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &Collider,
            &LinearVelocity,
            &InputFreshness,
            Option<&ActionState<FighterInput>>,
            Option<&NativeBuffer<FighterInput>>,
            Option<&crate::combat::Defeated>,
            Option<&crate::combat::ActiveEffects>,
            Option<&crate::map::EffectTileOccupancy>,
            Option<&crate::combat::ExternalMotion>,
            Option<&crate::matchplay::MatchParticipant>,
            Option<&crate::matchplay::ActiveCombatant>,
        ),
        With<Fighter>,
    >,
    loadouts: Query<&crate::builds::ResolvedMatchLoadout>,
    passive_states: Query<&crate::builds::PassiveRuntimeState>,
    ability_states: Query<&crate::builds::AbilityState>,
) {
    let config = MoveAndSlideConfig {
        move_and_slide_iterations: tuning.move_iterations,
        penetration_rejection_threshold: 2.0,
        skin_width: tuning.skin_width,
        ..default()
    };
    for (
        entity,
        position,
        rotation,
        collider,
        velocity,
        freshness,
        action,
        buffer,
        defeated,
        active_effects,
        effect_tile,
        external_motion,
        participant,
        active_combatant,
    ) in &fighters
    {
        if ability_states
            .get(entity)
            .is_ok_and(|state| matches!(state.phase, crate::builds::AbilityPhase::Dashing { .. }))
            || defeated.is_some()
            || (participant.is_some() && active_combatant.is_none())
        {
            continue;
        }
        let previous_position = position.0;
        let mut position = *position;
        let mut rotation = *rotation;
        let mut velocity = *velocity;
        let decision = movement_decision(
            tick.0,
            freshness,
            action,
            buffer,
            &input_tuning,
            tuning.stale_input_ticks,
        );
        let freshness = decision.freshness;
        if let Some(aim) = decision.aim {
            rotation = Rotation::radians(aim.y.atan2(aim.x));
        }
        let desired_velocity = if active_effects.is_some_and(|effects| effects.is_frozen(tick.0)) {
            Vec2::ZERO
        } else {
            resolved_movement_velocity(
                tick.0,
                &decision,
                loadouts
                    .get(entity)
                    .ok()
                    .map(|loadout| loadout.fighter_stats.movement_speed),
                tuning.speed,
                MovementModifiers {
                    active_effects,
                    effect_tile,
                    passive_state: passive_states.get(entity).ok(),
                    external_motion,
                },
            )
        };
        let filter = SpatialQueryFilter::from_mask(
            STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER | crate::movement::PLAYER_ONLY_MAP_LAYER,
        )
        .with_excluded_entities([entity]);
        let output = move_and_slide.move_and_slide(
            collider,
            position.0,
            rotation.as_radians(),
            desired_velocity,
            time.delta(),
            &config,
            &filter,
            |_| MoveAndSlideHitResponse::Accept,
        );
        position.0 = output.position;
        velocity.0 = output.projected_velocity;

        let facing = rotation.as_radians();
        if !pose_is_valid(position.0, facing, *bounds, tuning.radius) {
            let (repaired_position, repaired_facing) = repaired_pose(
                position.0,
                facing,
                &bounds,
                tuning.radius,
                tuning.spawn_facing,
            );
            position.0 = repaired_position;
            rotation = Rotation::radians(repaired_facing);
            warn!(?entity, "repaired invalid authoritative fighter pose");
        }
        if trace.enabled {
            let last_input = trace
                .last_inputs
                .iter()
                .find(|(candidate, _, _)| *candidate == entity)
                .map(|(_, move_axis, buttons)| (*move_axis, *buttons));
            if last_input != Some(decision.trace_input) {
                info!(
                    tick = tick.0,
                    ?entity,
                    stale = decision.stale,
                    last_fresh_tick = ?freshness.last_fresh_tick,
                    move_axis = ?decision.trace_input.0,
                    position_before = ?previous_position,
                    position_after = ?position.0,
                    "live server authoritative input changed"
                );
                trace
                    .last_inputs
                    .retain(|(candidate, _, _)| *candidate != entity);
                trace
                    .last_inputs
                    .push((entity, decision.trace_input.0, decision.trace_input.1));
            }
        }
        commands.entity(entity).insert((
            position,
            rotation,
            velocity,
            freshness,
            AuthoritativePose {
                position: crate::combat::WorldPoint::from(position.0),
                facing: rotation.as_radians(),
                tick: tick.0,
            },
        ));
    }
}

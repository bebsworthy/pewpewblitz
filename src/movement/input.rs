//! Input shaping, freshness, and server-side native-input validation rules.

use bevy::prelude::{Component, Resource, Vec2};
use core::time::Duration;
#[cfg(feature = "server")]
use lightyear::input::input_buffer::Compressed;
#[cfg(feature = "server")]
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::input::native::NativeBuffer;

use crate::protocol::{FighterInput, QuantizedAxis2};

/// Input shaping thresholds shared by controller, mouse, and focused tests.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct InputTuning {
    pub move_deadzone: f32,
    pub aim_deadzone: f32,
    pub aim_commit_threshold: f32,
    pub trigger_press: f32,
    pub trigger_release: f32,
    pub min_tick_delta: i64,
    pub max_tick_delta: i64,
    pub max_history_ticks: usize,
    pub input_rate: f32,
    pub input_burst: f32,
}

impl Default for InputTuning {
    fn default() -> Self {
        Self {
            move_deadzone: 0.20,
            aim_deadzone: 0.25,
            aim_commit_threshold: 0.35,
            trigger_press: 0.55,
            trigger_release: 0.45,
            min_tick_delta: -120,
            max_tick_delta: 16,
            max_history_ticks: 16,
            input_rate: 120.0,
            input_burst: 30.0,
        }
    }
}

/// Slow multiplier from active effects for one tick, shared by the authoritative and
/// owner-predicted movement paths so both apply identical rules.
#[must_use]
pub fn active_slow_multiplier(effects: Option<&crate::combat::ActiveEffects>, tick: u64) -> f32 {
    effects
        .and_then(|effects| effects.slow)
        .filter(|slow| tick < slow.expires_at_tick)
        .map_or(1.0, |slow| {
            f32::from(slow.movement_multiplier_milli) / 1000.0
        })
}

/// Adrenaline multiplier from the passive runtime state, shared by both movement paths.
#[must_use]
pub fn adrenaline_multiplier(
    loadout: Option<&crate::builds::ResolvedMatchLoadout>,
    state: Option<&crate::builds::PassiveRuntimeState>,
    tick: u64,
) -> f32 {
    let active = state
        .and_then(|state| state.adrenaline_until_tick)
        .is_some_and(|deadline| tick < deadline);
    if !active {
        return 1.0;
    }
    loadout
        .and_then(|loadout| {
            loadout.passives.iter().find_map(|passive| {
                let crate::builds::PassiveParameters::AdrenalResponse {
                    movement_bonus_basis_points,
                    ..
                } = passive.parameters
                else {
                    return None;
                };
                Some(movement_bonus_basis_points)
            })
        })
        .map_or(1.0, |bonus| 1.0 + f32::from(bonus) / 10_000.0)
}

/// Server-side input freshness used to turn a prolonged missing stream neutral.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputFreshness {
    pub last_fresh_tick: Option<u64>,
}

/// Per-connection guard state for the unordered native input channel.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct InputValidationState {
    pub last_accepted_end_tick: Option<u32>,
    pub tokens: f32,
    pub last_refill_seconds: f32,
    pub ownership_rejections: u32,
    pub target_rejections: u32,
    pub malformed_rejections: u32,
    pub stale_or_reordered_rejections: u32,
    pub old_or_future_rejections: u32,
    pub rate_rejections: u32,
}

#[cfg(feature = "server")]
impl Default for InputValidationState {
    fn default() -> Self {
        Self {
            last_accepted_end_tick: None,
            tokens: InputTuning::default().input_burst,
            last_refill_seconds: 0.0,
            ownership_rejections: 0,
            target_rejections: 0,
            malformed_rejections: 0,
            stale_or_reordered_rejections: 0,
            old_or_future_rejections: 0,
            rate_rejections: 0,
        }
    }
}

#[cfg(feature = "server")]
pub(super) fn input_history_len_is_valid(len: usize, tuning: InputTuning) -> bool {
    (1..=tuning.max_history_ticks).contains(&len)
}

#[cfg(feature = "server")]
pub(super) fn input_target_is_entity(target: lightyear::input::input_message::InputTarget) -> bool {
    matches!(
        target,
        lightyear::input::input_message::InputTarget::Entity(_)
    )
}

#[cfg(feature = "server")]
pub(super) fn input_end_tick_is_acceptable(
    end_tick: i64,
    server_tick: i64,
    last_accepted_end_tick: Option<u32>,
    tokens: f32,
    tuning: InputTuning,
) -> bool {
    end_tick >= server_tick + tuning.min_tick_delta
        && end_tick <= server_tick + tuning.max_tick_delta
        && last_accepted_end_tick.is_none_or(|last| end_tick > i64::from(last))
        && tokens >= 1.0
}

#[cfg(feature = "server")]
pub(crate) fn decoded_input_is_valid(input: FighterInput) -> bool {
    input.is_valid()
        && input.move_axis.to_vec2().length_squared() <= 1.0002
        && input
            .aim_update
            .is_none_or(|axis| axis.to_vec2().length_squared() <= 1.0002)
}

#[cfg(feature = "server")]
pub(super) fn input_sequence_ends_with_present_state(
    states: impl Iterator<Item = Compressed<ActionState<FighterInput>>>,
) -> bool {
    let mut present = false;
    for state in states {
        match state {
            Compressed::Absent => present = false,
            Compressed::Input(_) => present = true,
            Compressed::SameAsPrecedent => {}
        }
    }
    present
}

/// Returns the newest remote tick whose resolved buffer value is present.
///
/// `InputBuffer::last_remote_tick` is a transport watermark, not a freshness
/// signal: Lightyear advances it even when a received state resolves to
/// `Compressed::Absent`. Only a present value (including a resolved
/// `SameAsPrecedent`) is evidence that the client supplied input for the tick.
#[must_use]
pub fn latest_present_remote_tick(buffer: &NativeBuffer<FighterInput>) -> Option<u64> {
    let tick = buffer.last_remote_tick?;
    buffer.get(tick).map(|_| u64::from(tick.0))
}

/// Apply a radial deadzone and remap the remaining magnitude to the full range.
#[must_use]
pub fn radial_deadzone(axis: Vec2, deadzone: f32) -> Vec2 {
    if !axis.is_finite() {
        return Vec2::ZERO;
    }
    let magnitude = axis.length();
    if magnitude <= deadzone || magnitude <= f32::EPSILON {
        Vec2::ZERO
    } else {
        axis / magnitude * ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
    }
}

/// Return a normalized facing update only when the post-deadzone aim is meaningful.
#[must_use]
pub fn committed_aim(axis: Vec2, tuning: InputTuning) -> Option<Vec2> {
    let remapped = radial_deadzone(axis, tuning.aim_deadzone);
    (remapped.length() >= tuning.aim_commit_threshold).then(|| remapped.normalize())
}

/// Hysteresis for an analog trigger represented as a held gameplay button.
#[must_use]
pub fn trigger_pressed(previous: bool, value: f32, tuning: InputTuning) -> bool {
    let value = if value.is_finite() { value } else { 0.0 };
    if previous {
        value >= tuning.trigger_release
    } else {
        value >= tuning.trigger_press
    }
}

/// Return the normalized movement axis used by the fixed simulation.
#[must_use]
pub fn decoded_move(input: QuantizedAxis2, tuning: InputTuning) -> Vec2 {
    radial_deadzone(input.to_vec2(), tuning.move_deadzone).clamp_length_max(1.0)
}

/// Return the exact movement/facing result before collision queries.
#[must_use]
pub fn desired_pose_step(
    position: Vec2,
    facing: f32,
    input: FighterInput,
    movement_speed: f32,
    input_tuning: InputTuning,
    delta: Duration,
) -> (Vec2, f32, Vec2) {
    let direction = decoded_move(input.move_axis, input_tuning);
    let aim = input
        .aim_update
        .and_then(|axis| committed_aim(axis.to_vec2(), input_tuning));
    let facing = aim.map_or(facing, |aim| aim.y.atan2(aim.x));
    let velocity = direction * movement_speed;
    let position = position + velocity * delta.as_secs_f32();
    (position, facing, velocity)
}

#[must_use]
pub fn input_should_neutralize(
    current_tick: u64,
    last_fresh_tick: Option<u64>,
    limit: u64,
) -> bool {
    last_fresh_tick.is_none_or(|last| current_tick.saturating_sub(last) > limit)
}

//! The one fighter movement step shared by server authority and owner prediction.
//!
//! The authoritative coordinator and the client owner predictor both call these pure rules,
//! so input shaping, eligibility, status modifiers, and external motion cannot drift between
//! the two execution roles. Collision resolution and commit stay role-owned: the server uses
//! Avian move-and-slide against its installed colliders, and the client resolves the same
//! derived map blockers analytically for presentation only.

use bevy::prelude::Vec2;

use crate::protocol::FighterInput;

use super::input::{InputTuning, active_slow_multiplier, committed_aim, decoded_move};

/// Normalized movement and committed aim derived from one input frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MovementIntent {
    pub movement: Vec2,
    pub aim: Option<Vec2>,
}

/// Shape one input frame exactly as the authoritative decoder does. Invalid frames are neutral.
#[must_use]
pub(crate) fn movement_intent(input: FighterInput, tuning: &InputTuning) -> MovementIntent {
    let input = if input.is_valid() {
        input
    } else {
        FighterInput::default()
    };
    MovementIntent {
        movement: decoded_move(input.move_axis, *tuning),
        aim: input
            .aim_update
            .and_then(|axis| committed_aim(axis.to_vec2(), *tuning)),
    }
}

/// Whether ordinary movement is suspended this tick: a dash owns the pose, the fighter is
/// defeated, or a match participant is not an active combatant.
#[must_use]
pub(crate) fn movement_is_suspended(
    ability: Option<&crate::builds::AbilityState>,
    defeated: bool,
    inactive_participant: bool,
) -> bool {
    ability.is_some_and(|state| matches!(state.phase, crate::builds::AbilityPhase::Dashing { .. }))
        || defeated
        || inactive_participant
}

/// Status, tile, passive, and external-motion inputs to one movement step.
#[derive(Clone, Copy, Default)]
pub(crate) struct MovementModifiers<'a> {
    pub active_effects: Option<&'a crate::combat::ActiveEffects>,
    pub effect_tile: Option<&'a crate::map::EffectTileOccupancy>,
    pub adrenal_response: Option<crate::builds::ResolvedPassive>,
    pub passive_state: Option<&'a crate::builds::PassiveRuntimeState>,
    pub external_motion: Option<crate::combat::ExternalMotion>,
}

/// Resolve the per-tick desired velocity from intent, speed, effects, tile, adrenaline, and
/// external motion. A frozen fighter does not move at all.
#[must_use]
pub(crate) fn resolved_movement_velocity(
    tick: u64,
    movement: Vec2,
    resolved_speed: f32,
    modifiers: MovementModifiers<'_>,
) -> Vec2 {
    if modifiers
        .active_effects
        .is_some_and(|effects| effects.is_frozen(tick))
    {
        return Vec2::ZERO;
    }
    let tile_multiplier = modifiers.effect_tile.map_or(1.0, |occupancy| {
        f32::from(occupancy.capabilities.movement_multiplier_milli()) / 1000.0
    });
    let adrenaline_multiplier = super::input::adrenaline_multiplier(
        modifiers.adrenal_response,
        modifiers.passive_state,
        tick,
    );
    movement
        * resolved_speed
        * active_slow_multiplier(modifiers.active_effects, tick)
        * tile_multiplier
        * adrenaline_multiplier
        + modifiers
            .external_motion
            .filter(|motion| tick < motion.expires_at_tick)
            .map_or(Vec2::ZERO, |motion| motion.velocity)
}

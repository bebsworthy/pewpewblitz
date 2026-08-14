//! Deterministic immediate effect policies.

use super::{ActiveEffects, AttackId, ExternalMotion, NetworkEntityId, SlowEffect};
use bevy::prelude::Vec2;

pub const MAX_EXTERNAL_MOTION_SPEED: f32 = 900.0;

#[must_use]
pub fn combine_knockback(
    existing: Option<ExternalMotion>,
    velocity: Vec2,
    expires_at_tick: u64,
) -> ExternalMotion {
    let combined = existing.map_or(velocity, |old| old.velocity + velocity);
    ExternalMotion {
        velocity: combined.clamp_length_max(MAX_EXTERNAL_MOTION_SPEED),
        expires_at_tick: existing.map_or(expires_at_tick, |old| {
            old.expires_at_tick.max(expires_at_tick)
        }),
    }
}

pub fn refresh_strongest_slow(
    effects: &mut ActiveEffects,
    source_attack_id: AttackId,
    source_network_entity_id: NetworkEntityId,
    movement_multiplier_milli: u16,
    expires_at_tick: u64,
) {
    let next = SlowEffect {
        source_attack_id,
        source_network_entity_id,
        movement_multiplier_milli,
        expires_at_tick,
    };
    match effects.slow {
        None => effects.slow = Some(next),
        Some(current) if movement_multiplier_milli < current.movement_multiplier_milli => {
            effects.slow = Some(next);
        }
        Some(mut current) => {
            current.expires_at_tick = current.expires_at_tick.max(expires_at_tick);
            effects.slow = Some(current);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_keeps_strongest_magnitude_and_latest_expiry() {
        let mut effects = ActiveEffects::default();
        refresh_strongest_slow(&mut effects, AttackId(1), NetworkEntityId(1), 700, 20);
        refresh_strongest_slow(&mut effects, AttackId(2), NetworkEntityId(2), 800, 30);
        assert_eq!(effects.slow.unwrap().movement_multiplier_milli, 700);
        refresh_strongest_slow(&mut effects, AttackId(3), NetworkEntityId(3), 500, 25);
        assert_eq!(effects.slow.unwrap().movement_multiplier_milli, 500);
    }
}

//! Server-authoritative fighter health recovery after an accepted-attack idle window.

use super::{ActiveEffects, CurrentHealth, Defeated, HealthRecoveryState};
use crate::{
    builds::ResolvedMatchLoadout,
    matchplay::{ActiveCombatant, MatchParticipant},
    protocol::Fighter,
    timing::{SIMULATION_TICK_HZ, SimulationTick},
};
use bevy::prelude::*;

#[must_use]
fn advance_recovery(
    current: u16,
    maximum: u16,
    rate_per_second: u16,
    remainder: u64,
) -> (u16, u64) {
    if current >= maximum {
        return (maximum, 0);
    }
    let numerator = remainder.saturating_add(u64::from(rate_per_second));
    let restored = numerator / SIMULATION_TICK_HZ;
    let remainder = numerator % SIMULATION_TICK_HZ;
    let restored = u16::try_from(restored).unwrap_or(u16::MAX);
    (current.saturating_add(restored).min(maximum), remainder)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the fixed-tick system declares its complete authority view"
)]
pub(super) fn restore_attack_idle_health(
    tick: Res<SimulationTick>,
    active: Query<(), With<ActiveCombatant>>,
    mut fighters: Query<
        (
            Entity,
            &ResolvedMatchLoadout,
            &mut CurrentHealth,
            &mut HealthRecoveryState,
            &ActiveEffects,
            Option<&MatchParticipant>,
        ),
        (With<Fighter>, Without<Defeated>),
    >,
) {
    for (entity, loadout, mut health, mut recovery, effects, participant) in &mut fighters {
        if participant.is_some() && !active.contains(entity) {
            recovery.recovery_remainder = 0;
            continue;
        }
        let stats = loadout.fighter_stats;
        if tick.0
            < recovery
                .last_accepted_attack_tick
                .saturating_add(stats.idle_attack_delay_ticks)
        {
            recovery.recovery_remainder = 0;
            continue;
        }
        if effects.is_poisoned(tick.0) {
            recovery.recovery_remainder = 0;
            continue;
        }
        let (next, remainder) = advance_recovery(
            health.0,
            stats.maximum_health,
            stats.health_recovery_rate,
            recovery.recovery_remainder,
        );
        health.0 = next;
        recovery.recovery_remainder = remainder;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_recovery_is_exact_and_clamped() {
        let mut health = 1;
        let mut remainder = 0;
        for _ in 0..60 {
            (health, remainder) = advance_recovery(health, 100, 10, remainder);
        }
        assert_eq!((health, remainder), (11, 0));

        assert_eq!(advance_recovery(99, 100, 120, 0), (100, 0));
        assert_eq!(advance_recovery(100, 100, 10, 42), (100, 0));
    }
}

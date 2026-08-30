//! Server-authoritative fighter health recovery after an accepted-attack idle window.

use super::{ActiveEffects, CurrentHealth, Defeated, HealthRecoveryState};
use crate::{
    builds::ResolvedFighterStats,
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
            &ResolvedFighterStats,
            &mut CurrentHealth,
            &mut HealthRecoveryState,
            &ActiveEffects,
            Option<&MatchParticipant>,
            Option<&crate::map::EffectTileOccupancy>,
        ),
        (With<Fighter>, Without<Defeated>),
    >,
) {
    for (entity, stats, mut health, mut recovery, effects, participant, effect_tile) in
        &mut fighters
    {
        if participant.is_some() && !active.contains(entity) {
            recovery.recovery_remainder = 0;
            continue;
        }
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
        if effect_tile.is_some_and(crate::map::EffectTileOccupancy::blocks_healing) {
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

    #[test]
    fn recovery_consumes_projected_stats_without_replicated_loadout() {
        let mut app = App::new();
        app.insert_resource(SimulationTick(60))
            .add_systems(Update, restore_attack_idle_health);
        let fighter = app
            .world_mut()
            .spawn((
                Fighter,
                ResolvedFighterStats {
                    maximum_health: 100,
                    movement_speed: 200.0,
                    health_recovery_rate: 60,
                    idle_attack_delay_ticks: 30,
                    reveal_proximity_radius: 160.0,
                    cold_capacity: 100,
                    cold_resistance_basis_points: 0,
                    poison_resistance_basis_points: 0,
                    fire_resistance_basis_points: 0,
                },
                CurrentHealth(50),
                HealthRecoveryState {
                    last_accepted_attack_tick: 0,
                    recovery_remainder: 0,
                },
                ActiveEffects::default(),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<CurrentHealth>(fighter),
            Some(&CurrentHealth(51))
        );
        assert!(
            app.world()
                .get::<crate::builds::ResolvedMatchLoadout>(fighter)
                .is_none()
        );
    }
}

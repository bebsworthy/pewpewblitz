//! Solo Last Stand: six fighters, one life each, a closing boundary, last fighter standing wins.
//!
//! The shared half defines the replicated root state, the deterministic boundary schedule, and
//! the pure elimination and drop rules. The server-only authority applies outside-zone damage
//! through the combat environment transaction, ranks eliminations, places death drops, and
//! offers the round outcome to the common lifecycle.

use super::{MAX_MATCH_TEAMS, MatchId, MatchResult};
use crate::combat::TeamId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Composed revision of the validated common lifecycle plus Last Stand rules.
pub const LAST_STAND_RULES_REVISION: u16 = 1;

/// Solo fighters per round (owner decision q-9xg5t3j): six teams of one.
pub const LAST_STAND_FIGHTERS: u8 = 6;

/// Boundary phases in one round.
pub const LAST_STAND_BOUNDARY_PHASES: usize = 4;

/// Most boost pickups one defeat drops; fewest is one, so a kill always leaves a boost.
pub const MAX_DEATH_DROPS: u8 = 2;

/// One boundary phase: hold the current radius, then shrink to `radius_percent` of the
/// starting radius. Durations are per-mille of the active round so verification and production
/// share one schedule shape.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundaryPhase {
    pub hold_permille: u16,
    pub shrink_permille: u16,
    pub radius_percent: u8,
    pub damage_per_pulse: u16,
}

/// Outside-zone damage pulses every half second, through `src/timing.rs`.
const PULSE_INTERVAL_TICKS: u16 = match crate::timing::simulation_ticks_from_millis(500) {
    Some(ticks) if ticks > 0 && ticks < 1 << 16 => {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the guard keeps the tick count inside u16"
        )]
        let ticks = ticks as u16;
        ticks
    }
    _ => panic!("Last Stand pulse interval must be a whole, nonzero u16 tick count"),
};

/// Last Stand rules layered on the common `MatchLifecycleRules`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LastStandRules {
    pub phases: [BoundaryPhase; LAST_STAND_BOUNDARY_PHASES],
    /// Ticks between outside-zone damage pulses.
    pub pulse_interval_ticks: u16,
    pub boost_stack_limit: u8,
    pub pickup_radius: f32,
}

impl Default for LastStandRules {
    /// Production schedule for a 180 s round: closes at 30 s, 85 s, 115 s, and 140 s, leaving
    /// a small final ring for the last 40 s. Outside damage climbs each phase.
    fn default() -> Self {
        Self {
            phases: [
                BoundaryPhase {
                    hold_permille: 167,
                    shrink_permille: 111,
                    radius_percent: 70,
                    damage_per_pulse: 30,
                },
                BoundaryPhase {
                    hold_permille: 111,
                    shrink_permille: 83,
                    radius_percent: 45,
                    damage_per_pulse: 45,
                },
                BoundaryPhase {
                    hold_permille: 83,
                    shrink_permille: 83,
                    radius_percent: 22,
                    damage_per_pulse: 70,
                },
                BoundaryPhase {
                    hold_permille: 56,
                    shrink_permille: 83,
                    radius_percent: 8,
                    damage_per_pulse: 100,
                },
            ],
            pulse_interval_ticks: PULSE_INTERVAL_TICKS,
            boost_stack_limit: 6,
            pickup_radius: 30.0,
        }
    }
}

impl LastStandRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        let total: u32 = self
            .phases
            .iter()
            .map(|phase| u32::from(phase.hold_permille) + u32::from(phase.shrink_permille))
            .sum();
        if total >= 1_000 {
            return Err("Last Stand boundary must close before the round deadline");
        }
        let mut previous = 100_u8;
        for phase in self.phases {
            if phase.shrink_permille == 0
                || phase.radius_percent == 0
                || phase.radius_percent >= previous
                || phase.damage_per_pulse == 0
            {
                return Err("Last Stand boundary phases must shrink and damage");
            }
            previous = phase.radius_percent;
        }
        if self.pulse_interval_ticks == 0 {
            return Err("Last Stand damage pulse interval must be positive");
        }
        if self.boost_stack_limit == 0 || self.boost_stack_limit > super::MAX_BOOST_STACKS {
            return Err("Last Stand boost stack limit must be within 1..=20");
        }
        if !self.pickup_radius.is_finite() || self.pickup_radius <= 0.0 {
            return Err("Last Stand pickup radius must be positive");
        }
        Ok(self)
    }
}

/// The boundary as it stands for one phase: hold at `radius_from`, shrink linearly to
/// `radius_to` between the two shrink ticks. `phase` counts phases already closed; at
/// `LAST_STAND_BOUNDARY_PHASES` the ring is final and stays at `radius_to`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct LastStandBoundary {
    pub center: Vec2,
    pub phase: u8,
    pub radius_from: f32,
    pub radius_to: f32,
    pub shrink_starts_at_tick: u64,
    pub shrink_ends_at_tick: u64,
    pub damage_per_pulse: u16,
}

impl LastStandBoundary {
    /// Interpolated safe radius at `tick`; clients evaluate it from the match clock.
    #[must_use]
    pub fn radius_at(&self, tick: u64) -> f32 {
        if tick <= self.shrink_starts_at_tick {
            return self.radius_from;
        }
        if tick >= self.shrink_ends_at_tick {
            return self.radius_to;
        }
        let span = self.shrink_ends_at_tick - self.shrink_starts_at_tick;
        #[allow(
            clippy::cast_precision_loss,
            reason = "bounded round tick spans are far below f32 integer precision"
        )]
        let progress = (tick - self.shrink_starts_at_tick) as f32 / span as f32;
        self.radius_from + (self.radius_to - self.radius_from) * progress
    }

    #[must_use]
    pub fn contains(&self, position: Vec2, tick: u64) -> bool {
        position.distance_squared(self.center) <= self.radius_at(tick).powi(2)
    }

    /// Whether every phase has closed and the ring no longer moves.
    #[must_use]
    pub fn is_final(&self) -> bool {
        usize::from(self.phase) >= LAST_STAND_BOUNDARY_PHASES
    }
}

/// Deterministic boundary for `tick` of a round that went active at `started_at_tick` and
/// lasts `active_ticks`. The ring starts at `initial_radius` around `center`.
#[must_use]
pub fn boundary_at(
    rules: &LastStandRules,
    started_at_tick: u64,
    active_ticks: u64,
    center: Vec2,
    initial_radius: f32,
    tick: u64,
) -> LastStandBoundary {
    let scaled = |permille: u16| active_ticks.saturating_mul(u64::from(permille)) / 1_000;
    let radius = |percent: u8| initial_radius * f32::from(percent) / 100.0;
    let mut phase_start = started_at_tick;
    let mut radius_from = initial_radius;
    for (index, phase) in rules.phases.iter().enumerate() {
        let shrink_starts_at_tick = phase_start.saturating_add(scaled(phase.hold_permille));
        let shrink_ends_at_tick =
            shrink_starts_at_tick.saturating_add(scaled(phase.shrink_permille).max(1));
        let radius_to = radius(phase.radius_percent);
        if tick < shrink_ends_at_tick {
            return LastStandBoundary {
                center,
                phase: u8::try_from(index).unwrap_or(u8::MAX),
                radius_from,
                radius_to,
                shrink_starts_at_tick,
                shrink_ends_at_tick,
                damage_per_pulse: phase.damage_per_pulse,
            };
        }
        phase_start = shrink_ends_at_tick;
        radius_from = radius_to;
    }
    let last = rules.phases[LAST_STAND_BOUNDARY_PHASES - 1];
    LastStandBoundary {
        center,
        phase: u8::try_from(LAST_STAND_BOUNDARY_PHASES).unwrap_or(u8::MAX),
        radius_from,
        radius_to: radius_from,
        shrink_starts_at_tick: phase_start,
        shrink_ends_at_tick: phase_start,
        damage_per_pulse: last.damage_per_pulse,
    }
}

/// Durable replicated Last Stand state on the match root. `placements[team]` is 0 while that
/// fighter is alive or the round has not ranked it, otherwise its finishing place (1 = winner;
/// fighters eliminated on the same tick share a place).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct LastStandState {
    pub match_id: MatchId,
    pub fighters: u8,
    pub alive: u8,
    pub boundary: LastStandBoundary,
    pub placements: [u8; MAX_MATCH_TEAMS],
}

/// Boost pickups one defeat drops: `clamp(carried, 1, 2)`.
#[must_use]
pub const fn death_drop_count(carried_stacks: u8) -> u8 {
    if carried_stacks == 0 {
        1
    } else if carried_stacks > MAX_DEATH_DROPS {
        MAX_DEATH_DROPS
    } else {
        carried_stacks
    }
}

/// Rank one tick's eliminations: every fighter eliminated together takes the place just below
/// the fighters still alive. Returns the alive count afterwards.
pub fn rank_eliminations(
    placements: &mut [u8; MAX_MATCH_TEAMS],
    alive_before: u8,
    eliminated: &[TeamId],
) -> u8 {
    let alive_after =
        alive_before.saturating_sub(u8::try_from(eliminated.len()).unwrap_or(u8::MAX));
    for team in eliminated {
        if let Some(place) = placements.get_mut(usize::from(team.0)) {
            *place = alive_after.saturating_add(1);
        }
    }
    alive_after
}

/// Round outcome once eliminations leave at most one survivor: the survivor wins; a final
/// simultaneous elimination is a draw.
#[must_use]
pub fn elimination_result(survivors: &[TeamId]) -> Option<MatchResult> {
    match survivors {
        [] => Some(MatchResult::Draw),
        [team] => Some(MatchResult::TeamVictory { team: *team }),
        _ => None,
    }
}

/// Terminal Last Stand telemetry attached to one common match summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LastStandSummary {
    pub fighters: u8,
    pub survivors: u8,
    pub boundary_phase_reached: u8,
    pub boundary_defeats: u16,
    pub chests_opened: u16,
    pub drops_spawned: u16,
    pub drops_skipped: u16,
}

/// Last Stand rules for one rules profile; the schedule is a fraction of the round, so both
/// profiles share it.
#[must_use]
pub fn last_stand_rules_for_profile(_profile: crate::config::MatchRulesProfile) -> LastStandRules {
    LastStandRules::default()
}

#[cfg(feature = "server")]
mod authority;

#[cfg(feature = "server")]
pub use authority::LastStandModePlugin;
#[cfg(feature = "server")]
pub(crate) use authority::LastStandTelemetry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_rules_validate_and_reject_non_shrinking_phases() {
        let rules = LastStandRules::default();
        assert_eq!(rules.validate(), Ok(rules));
        let mut grows = rules;
        grows.phases[2].radius_percent = 50;
        assert!(grows.validate().is_err());
        let mut late = rules;
        late.phases[0].hold_permille = 900;
        assert!(late.validate().is_err());
        assert!(
            LastStandRules {
                boost_stack_limit: 0,
                ..rules
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn boundary_holds_shrinks_and_settles_deterministically() {
        let rules = LastStandRules::default();
        let at = |tick| boundary_at(&rules, 100, 10_800, Vec2::ZERO, 800.0, tick);
        let opening = at(100);
        assert_eq!(opening.phase, 0);
        assert!((opening.radius_at(100) - 800.0).abs() < f32::EPSILON);
        // Phase 1 holds for 16.7 % of the round (1 803 ticks), then shrinks to 70 %.
        assert_eq!(opening.shrink_starts_at_tick, 100 + 1_803);
        assert_eq!(opening.shrink_ends_at_tick, 100 + 1_803 + 1_198);
        let midway = u64::midpoint(opening.shrink_starts_at_tick, opening.shrink_ends_at_tick);
        assert!((opening.radius_at(midway) - 680.0).abs() < 1.0);
        let second = at(opening.shrink_ends_at_tick);
        assert_eq!(second.phase, 1);
        assert!((second.radius_from - 560.0).abs() < 0.01);
        assert!((second.radius_to - 360.0).abs() < 0.01);
        let closed = at(100 + 10_799);
        assert!(closed.is_final());
        assert!((closed.radius_at(100 + 10_799) - 64.0).abs() < 0.01);
        assert_eq!(closed.damage_per_pulse, 100);
        assert_eq!(at(5_000), at(5_000));
        for tick in (100..10_900).step_by(97) {
            let boundary = at(tick);
            let next = at(tick + 1);
            assert!(next.radius_at(tick + 1) <= boundary.radius_at(tick) + f32::EPSILON);
        }
    }

    #[test]
    fn death_drops_clamp_between_one_and_two() {
        assert_eq!(death_drop_count(0), 1);
        assert_eq!(death_drop_count(1), 1);
        assert_eq!(death_drop_count(2), 2);
        assert_eq!(death_drop_count(3), 2);
        assert_eq!(death_drop_count(20), 2);
    }

    #[test]
    fn simultaneous_eliminations_share_a_place_and_the_survivor_wins() {
        let mut placements = [0; MAX_MATCH_TEAMS];
        let alive = rank_eliminations(&mut placements, 6, &[TeamId(4)]);
        assert_eq!((alive, placements[4]), (5, 6));
        let alive = rank_eliminations(&mut placements, alive, &[TeamId(0), TeamId(2)]);
        assert_eq!((alive, placements[0], placements[2]), (3, 4, 4));
        let alive = rank_eliminations(&mut placements, alive, &[TeamId(1), TeamId(3)]);
        assert_eq!(alive, 1);
        assert_eq!(placements[1], 2);
        assert_eq!(
            elimination_result(&[TeamId(5)]),
            Some(MatchResult::TeamVictory { team: TeamId(5) })
        );
        assert_eq!(elimination_result(&[]), Some(MatchResult::Draw));
        assert_eq!(elimination_result(&[TeamId(1), TeamId(2)]), None);
    }
}

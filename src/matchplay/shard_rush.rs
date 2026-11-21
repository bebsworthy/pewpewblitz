//! Server-authoritative Shard Rush rules, replicated state, and mode plugin.
//!
//! Teams collect shards that spawn around one map source, carry them as a public per-fighter
//! count, scatter them on defeat or disconnect, and win by holding the target team total
//! through a visible countdown that resets whenever the leader stops qualifying.

use super::MatchId;
use crate::combat::TeamId;
use crate::map::ModeAnchorId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Composed revision of the validated common lifecycle plus Shard Rush rules.
pub const SHARD_RUSH_RULES_REVISION: u16 = 1;

/// Network-ID namespace for server-spawned shard entities.
pub const SHARD_NETWORK_NAMESPACE: u64 = 0x1000_0000_0000_0000;

/// Representation ceiling for the authored team target (and so the per-fighter carry limit).
pub const MAX_SHARD_RUSH_TARGET: u8 = 32;

/// Representation ceiling for the authored on-ground spawn ceiling.
pub const MAX_SHARD_RUSH_LIVE_CEILING: u8 = 24;

/// World-unit distance from a fighter centre that collects a shard.
pub const SHARD_PICKUP_RADIUS: f32 = 30.0;

/// World-unit body radius used to keep spawned and dropped shards clear of blocking geometry.
pub const SHARD_BODY_RADIUS: f32 = 10.0;

/// Validated Shard Rush operator policy: the hold countdown, the source cadence, and the
/// on-ground spawn ceiling, all in simulation ticks or counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardRushPolicy {
    pub hold_ticks: u16,
    pub spawn_interval_ticks: u16,
    pub live_shard_ceiling: u8,
}

/// A whole-second policy duration in simulation ticks, through `src/timing.rs`.
const fn policy_ticks(seconds: u64) -> u16 {
    match crate::timing::simulation_ticks_from_seconds(seconds) {
        Some(ticks) if ticks < 1 << 16 => {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "the guard keeps the tick count inside u16"
            )]
            let ticks = ticks as u16;
            ticks
        }
        _ => panic!("Shard Rush policy duration fits u16 ticks"),
    }
}

impl ShardRushPolicy {
    /// Shorter verification policy with the same semantics: a 1 s hold and a 0.5 s cadence.
    pub const PROCESS_VERIFICATION: Self = Self {
        hold_ticks: policy_ticks(1),
        spawn_interval_ticks: policy_ticks(1) / 2,
        live_shard_ceiling: 12,
    };

    /// Built-in production fallback matching the checked-in catalog: a 12 s hold, one shard
    /// every 6 s, at most 12 on the ground.
    pub const PRODUCTION: Self = Self {
        hold_ticks: policy_ticks(12),
        spawn_interval_ticks: policy_ticks(6),
        live_shard_ceiling: 12,
    };

    pub fn validate(self) -> Result<Self, &'static str> {
        if self.hold_ticks < 2 {
            return Err("Shard Rush hold countdown must last at least two ticks");
        }
        if self.spawn_interval_ticks == 0 {
            return Err("Shard Rush spawn interval must be positive");
        }
        if self.live_shard_ceiling == 0 || self.live_shard_ceiling > MAX_SHARD_RUSH_LIVE_CEILING {
            return Err("Shard Rush live shard ceiling must be within 1..=24");
        }
        Ok(self)
    }
}

impl From<ShardRushPolicy> for brawler_routing::ShardRushPolicy {
    fn from(policy: ShardRushPolicy) -> Self {
        Self {
            hold_ticks: policy.hold_ticks,
            spawn_interval_ticks: policy.spawn_interval_ticks,
            live_shard_ceiling: policy.live_shard_ceiling,
        }
    }
}

impl From<brawler_routing::ShardRushPolicy> for ShardRushPolicy {
    fn from(policy: brawler_routing::ShardRushPolicy) -> Self {
        Self {
            hold_ticks: policy.hold_ticks,
            spawn_interval_ticks: policy.spawn_interval_ticks,
            live_shard_ceiling: policy.live_shard_ceiling,
        }
    }
}

/// Shard Rush objective rules layered on the common `MatchLifecycleRules`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardRushRules {
    pub target_shards: u8,
    pub policy: ShardRushPolicy,
}

impl ShardRushRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.target_shards == 0 || self.target_shards > MAX_SHARD_RUSH_TARGET {
            return Err("Shard Rush target must be within 1..=32 shards");
        }
        self.policy.validate()?;
        Ok(self)
    }

    /// Every fighter may carry up to the whole team target.
    #[must_use]
    pub const fn maximum_carried(self) -> u8 {
        self.target_shards
    }
}

/// Public replicated count of shards one fighter carries. Present on every fighter (zero
/// outside Shard Rush), reset on respawn and restart.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShardCarrier(pub u8);

/// Marker for one shard lying on the ground.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shard;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardIdentity {
    pub match_id: MatchId,
    pub shard_id: u32,
}

/// Whether a ground shard came from the source or was scattered by a carrier.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShardOrigin {
    Source,
    Dropped,
}

/// The running hold countdown: `team` wins at `ends_at_tick` unless it stops qualifying.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardRushCountdown {
    pub team: TeamId,
    pub started_at_tick: u64,
    pub ends_at_tick: u64,
}

/// Durable replicated Shard Rush state on the match root.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShardRushState {
    pub match_id: MatchId,
    pub source_anchor_id: ModeAnchorId,
    pub team_shards: [u8; 2],
    pub target_shards: u8,
    pub hold_ticks: u16,
    pub countdown: Option<ShardRushCountdown>,
    pub ground_shards: u8,
    pub next_spawn_at_tick: u64,
}

impl ShardRushState {
    /// A zero spawn deadline marks an activation that has not spawned its first shard yet.
    pub const UNSCHEDULED_SPAWN_TICK: u64 = 0;
}

/// Terminal Shard Rush telemetry snapshot attached to one common match summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShardRushSummary {
    pub final_team_shards: [u8; 2],
    pub target_shards: u8,
    pub shards_spawned: u16,
    pub shards_collected: u16,
    pub shards_dropped: u16,
    pub countdown_starts_by_team: [u16; 2],
    pub countdown_resets: u16,
}

/// The team holding the countdown: at or above the target and strictly ahead.
#[must_use]
pub fn leading_team(team_shards: [u8; 2], target: u8) -> Option<TeamId> {
    match team_shards[0].cmp(&team_shards[1]) {
        std::cmp::Ordering::Greater if team_shards[0] >= target => Some(TeamId(0)),
        std::cmp::Ordering::Less if team_shards[1] >= target => Some(TeamId(1)),
        _ => None,
    }
}

/// Reset rule: a countdown survives only while the same team keeps leading; any other leader
/// starts again from the full hold duration.
#[must_use]
#[cfg(feature = "server")]
pub(crate) fn next_countdown(
    previous: Option<ShardRushCountdown>,
    leader: Option<TeamId>,
    tick: u64,
    hold_ticks: u16,
) -> Option<ShardRushCountdown> {
    let team = leader?;
    match previous {
        Some(countdown) if countdown.team == team => Some(countdown),
        _ => Some(ShardRushCountdown {
            team,
            started_at_tick: tick,
            ends_at_tick: tick.saturating_add(u64::from(hold_ticks)),
        }),
    }
}

/// Deadline comparison: more held shards wins, equal totals draw.
#[must_use]
pub fn timeout_result(team_shards: [u8; 2]) -> super::MatchResult {
    match team_shards[0].cmp(&team_shards[1]) {
        std::cmp::Ordering::Greater => super::MatchResult::TeamVictory { team: TeamId(0) },
        std::cmp::Ordering::Less => super::MatchResult::TeamVictory { team: TeamId(1) },
        std::cmp::Ordering::Equal => super::MatchResult::Draw,
    }
}

/// Deterministic golden-angle fan used for both source spawns and carrier drops.
#[must_use]
#[cfg(feature = "server")]
pub(crate) fn shard_fan_offset(index: u16, base_radius: f32, step: f32) -> Vec2 {
    let angle = f32::from(index) * 2.399_963_1;
    let ring = f32::from(index % 8);
    Vec2::from_angle(angle) * (base_radius + ring * step)
}

/// Shard Rush rules for one rules profile when no authored policy reached the process.
#[must_use]
pub fn shard_rush_rules_for_profile(profile: crate::config::MatchRulesProfile) -> ShardRushRules {
    match profile {
        crate::config::MatchRulesProfile::Production => ShardRushRules {
            target_shards: 8,
            policy: ShardRushPolicy::PRODUCTION,
        },
        crate::config::MatchRulesProfile::ProcessVerification => ShardRushRules {
            target_shards: 3,
            policy: ShardRushPolicy::PROCESS_VERIFICATION,
        },
    }
}

#[cfg(feature = "server")]
mod authority;

#[cfg(feature = "server")]
pub(crate) use authority::ShardRushTelemetry;
#[cfg(feature = "server")]
pub use authority::{ShardRushModePlugin, ShardRushSource};

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn countdown(team: u8, start: u64) -> ShardRushCountdown {
        ShardRushCountdown {
            team: TeamId(team),
            started_at_tick: start,
            ends_at_tick: start + 720,
        }
    }

    #[test]
    fn leader_needs_the_target_and_a_strict_lead() {
        assert_eq!(leading_team([7, 0], 8), None);
        assert_eq!(leading_team([8, 0], 8), Some(TeamId(0)));
        assert_eq!(leading_team([8, 9], 8), Some(TeamId(1)));
        assert_eq!(leading_team([9, 9], 8), None);
        assert_eq!(leading_team([3, 12], 8), Some(TeamId(1)));
    }

    #[test]
    fn countdown_keeps_running_for_the_same_leader_and_resets_otherwise() {
        let first = next_countdown(None, Some(TeamId(0)), 100, 720);
        assert_eq!(first, Some(countdown(0, 100)));
        assert_eq!(
            next_countdown(first, Some(TeamId(0)), 400, 720),
            Some(countdown(0, 100))
        );
        assert_eq!(next_countdown(first, None, 401, 720), None);
        assert_eq!(
            next_countdown(None, Some(TeamId(0)), 500, 720),
            Some(countdown(0, 500)),
            "regaining the lead restarts from the full duration"
        );
        assert_eq!(
            next_countdown(first, Some(TeamId(1)), 402, 720),
            Some(countdown(1, 402))
        );
    }

    #[test]
    fn timeout_compares_team_totals() {
        assert_eq!(
            timeout_result([4, 2]),
            super::super::MatchResult::TeamVictory { team: TeamId(0) }
        );
        assert_eq!(
            timeout_result([0, 1]),
            super::super::MatchResult::TeamVictory { team: TeamId(1) }
        );
        assert_eq!(timeout_result([5, 5]), super::super::MatchResult::Draw);
    }

    #[test]
    fn rules_and_policy_bounds_fail_closed() {
        let rules = shard_rush_rules_for_profile(crate::config::MatchRulesProfile::Production);
        assert_eq!(rules.validate(), Ok(rules));
        assert_eq!(rules.maximum_carried(), 8);
        assert!(
            ShardRushRules {
                target_shards: 0,
                ..rules
            }
            .validate()
            .is_err()
        );
        assert!(
            ShardRushRules {
                target_shards: 33,
                ..rules
            }
            .validate()
            .is_err()
        );
        for policy in [
            ShardRushPolicy {
                hold_ticks: 1,
                ..rules.policy
            },
            ShardRushPolicy {
                spawn_interval_ticks: 0,
                ..rules.policy
            },
            ShardRushPolicy {
                live_shard_ceiling: 0,
                ..rules.policy
            },
            ShardRushPolicy {
                live_shard_ceiling: 25,
                ..rules.policy
            },
        ] {
            assert!(policy.validate().is_err(), "{policy:?}");
        }
    }

    #[test]
    fn fan_offsets_are_deterministic_and_spread() {
        assert_eq!(
            shard_fan_offset(3, 20.0, 6.0),
            shard_fan_offset(3, 20.0, 6.0)
        );
        let offsets = (0..8)
            .map(|i| shard_fan_offset(i, 20.0, 6.0))
            .collect::<Vec<_>>();
        for (index, offset) in offsets.iter().enumerate() {
            assert!(offset.length() >= 19.9);
            for other in &offsets[index + 1..] {
                assert!(offset.distance(*other) > 5.0);
            }
        }
    }
}

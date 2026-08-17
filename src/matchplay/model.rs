use crate::{combat::TeamId, map::ModeDefinitionId};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Reflect,
)]
pub struct MatchId(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum MatchResult {
    TeamVictory {
        team: TeamId,
    },
    Draw,
    Forfeit {
        winner: TeamId,
        departed_team: TeamId,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum MatchPhase {
    Waiting,
    Countdown {
        starts_at_tick: u64,
    },
    Active {
        ends_at_tick: u64,
    },
    Completed {
        completed_at_tick: u64,
        restart_unlocked_at_tick: u64,
        result: MatchResult,
    },
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchRoot;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchState {
    pub match_id: MatchId,
    pub mode_definition_id: ModeDefinitionId,
    pub phase: MatchPhase,
    pub rules_revision: u16,
}

/// Generation-tagged shared match clock published on the match root.
///
/// Clients derive countdown/remaining/restart deadlines only from the phase deadline minus
/// `completed_tick`, and only while this generation tag agrees with `MatchState::match_id` and
/// the installed mode state. It is updated in fixed finalize before `SimulationTick` advances.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchClock {
    pub match_id: MatchId,
    pub completed_tick: u64,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchParticipant {
    pub match_id: MatchId,
    pub ready: bool,
    pub restart_ready: bool,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RespawnState {
    pub respawn_at_tick: u64,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnProtection {
    pub expires_at_tick: u64,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveCombatant;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchMember(pub MatchId);

/// Mode-owned per-team participant capacity for one validated match composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamSlotCapacity {
    pub team_slot: u8,
    pub minimum_participants: u8,
    pub maximum_participants: u8,
}

/// The resolved match capacity derived from game-mode rules and map validation. Terrain
/// consumes only the checked maximum-active-fighter count and never encodes team
/// topology; operational connection limits must not under-provision it.
#[derive(bevy::prelude::Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedMatchCapacity {
    pub team_slots: Vec<TeamSlotCapacity>,
    pub maximum_active_fighters: u8,
}

impl ResolvedMatchCapacity {
    /// Derive the capacity from validated lifecycle rules with a checked fighter sum.
    #[cfg(feature = "server")]
    #[must_use]
    pub fn from_rules(rules: &crate::matchplay::MatchLifecycleRules) -> Option<Self> {
        let mut team_slots = Vec::new();
        let mut total = 0_u32;
        for slot in 0..u32::from(rules.team_count) {
            let Ok(team_slot) = u8::try_from(slot) else {
                return None;
            };
            total = total.checked_add(u32::from(rules.maximum_participants_per_team))?;
            team_slots.push(TeamSlotCapacity {
                team_slot,
                minimum_participants: rules.minimum_participants_per_team,
                maximum_participants: rules.maximum_participants_per_team,
            });
        }
        Some(Self {
            team_slots,
            maximum_active_fighters: u8::try_from(total).ok()?,
        })
    }
}

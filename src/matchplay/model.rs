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

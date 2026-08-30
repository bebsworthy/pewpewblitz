use crate::{combat::TeamId, map::ModeDefinitionId};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Stable match identity shared by gameplay state and the routed match manifest.
///
/// Match workers receive this value from the routing layer as a nonzero `u128`. Keeping the
/// gameplay identity at the same width avoids a second identity or lossy admission conversion.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Reflect,
)]
pub struct MatchId(pub u128);

impl fmt::Display for MatchId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for MatchId {
    type Err = <u128 as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

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

impl MatchResult {
    /// Deterministic closeout-report label for this result: `victory:<team>`, `draw`, or
    /// `forfeit:<winner>:<departed>`. Paired with [`Self::parse_report_label`] so report
    /// writers and readers share one result vocabulary.
    #[must_use]
    pub fn report_label(&self) -> String {
        match *self {
            Self::TeamVictory { team } => format!("victory:{}", team.0),
            Self::Draw => "draw".to_string(),
            Self::Forfeit {
                winner,
                departed_team,
            } => format!("forfeit:{}:{}", winner.0, departed_team.0),
        }
    }

    /// Inverse of [`Self::report_label`]; returns `None` for anything that is not a
    /// result label, including the report block's `none` sentinel.
    #[must_use]
    pub fn parse_report_label(label: &str) -> Option<Self> {
        if label == "draw" {
            return Some(Self::Draw);
        }
        let (head, rest) = label.split_once(':')?;
        match head {
            "victory" => Some(Self::TeamVictory {
                team: TeamId(rest.parse().ok()?),
            }),
            "forfeit" => {
                let (winner, departed) = rest.split_once(':')?;
                Some(Self::Forfeit {
                    winner: TeamId(winner.parse().ok()?),
                    departed_team: TeamId(departed.parse().ok()?),
                })
            }
            _ => None,
        }
    }
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

/// Bounded, process-local objective projection published by the installed mode plugin for
/// authoritative AI consumers. It deliberately describes objective shape rather than mode ID.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) enum BotObjectiveView {
    Elimination,
    ControlArea { center: Vec2, radius: f32 },
    AttackAndDefend,
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

/// Lobby-accepted, bounded presentation name replicated with a match fighter.
/// Stable player IDs remain the authority identity; clients never mutate this component.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FighterDisplayName(pub String);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicParticipantStatus {
    Alive,
    Ready,
    RestartReady,
    Defeated,
    Respawning { respawn_at_tick: u64 },
    Protected { expires_at_tick: u64 },
}

/// Always-public roster projection kept separate from the cullable spatial fighter.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicParticipantState {
    pub player_id: crate::protocol::PlayerId,
    pub fighter_network_id: crate::protocol::NetworkEntityId,
    pub team_id: TeamId,
    pub display_name: String,
    pub participant: MatchParticipant,
    pub selected: bool,
    pub weapon_preset_id: Option<u16>,
    pub status: PublicParticipantStatus,
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

    /// Validate the resolved capacity against the selected map snapshot: the map must
    /// serve exactly the team slots the rules declare, with enough spawn points per team
    /// for every simultaneous participant. Returns the exact mismatch otherwise.
    pub fn validate_against_spawn_catalog(
        &self,
        spawn_points: &crate::map::SpawnPointCatalog,
    ) -> Result<(), String> {
        let per_team_points: std::collections::BTreeMap<u8, usize> = spawn_points
            .0
            .iter()
            .map(|(team, points)| (*team, points.len()))
            .collect();
        let capacity_slots: std::collections::BTreeSet<u8> =
            self.team_slots.iter().map(|slot| slot.team_slot).collect();
        let map_slots: std::collections::BTreeSet<u8> = per_team_points.keys().copied().collect();
        if capacity_slots != map_slots {
            return Err(format!(
                "selected map serves team slots {map_slots:?} but the profile resolved {capacity_slots:?}"
            ));
        }
        for slot in &self.team_slots {
            let points = per_team_points
                .get(&slot.team_slot)
                .copied()
                .unwrap_or_default();
            if points < usize::from(slot.maximum_participants) {
                return Err(format!(
                    "selected map provides {points} spawn points for team slot {} but the profile admits up to {} simultaneous participants",
                    slot.team_slot, slot.maximum_participants
                ));
            }
        }
        Ok(())
    }
}

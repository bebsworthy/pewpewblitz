//! Mode-neutral authoritative combat facts consumed by game-mode rules and telemetry.

use super::{AttackId, CombatEventId, TeamId, WeaponPresetId, WeaponRecipeFingerprint, WorldPoint};
use crate::protocol::{NetworkEntityId, PlayerId};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatOutcomeKind {
    ProtectedContact,
    Damage { amount: u16 },
    Defeat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatOutcomeFact {
    pub event_id: CombatEventId,
    pub tick: u64,
    pub attack_id: AttackId,
    pub source_player: Option<PlayerId>,
    pub source_network_id: Option<NetworkEntityId>,
    pub source_team: Option<TeamId>,
    pub target_network_id: NetworkEntityId,
    pub target_team: TeamId,
    pub preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub position: WorldPoint,
    pub engagement_distance: f32,
    pub kind: CombatOutcomeKind,
}

#[derive(Resource, Default, Debug)]
pub struct CombatOutcomeFacts(pub Vec<CombatOutcomeFact>);

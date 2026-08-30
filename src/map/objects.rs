//! Server-owned health-bearing map placements and bounded environment-damage facts.

use super::{
    MapAssetId, MapDamageProfileId, MapDynamicGeneration, MapPlacementId, MapPlacementOutcome,
};
use crate::combat::{AttackId, AttackSource, CombatEventId, CurrentHealth, WorldPoint};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const MAX_DAMAGEABLE_MAP_OBJECTS: usize = 32;
pub const MAX_TERMINAL_REACTIONS_PER_TICK: usize = 16;
pub const MAX_WORLD_TARGET_FACTS: usize = 256;
pub const MAX_WORLD_OBJECT_CUES: usize = 256;
pub const MAX_SECONDARY_DAMAGE_APPLICATIONS: usize = 128;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageableTargetIdentity {
    MapObject {
        generation: MapDynamicGeneration,
        placement_id: MapPlacementId,
    },
    HeistSafe {
        match_id: crate::matchplay::MatchId,
        anchor_id: super::ModeAnchorId,
        defending_team: crate::combat::TeamId,
    },
}

impl DamageableTargetIdentity {
    #[must_use]
    pub const fn stable_order_key(self) -> (u8, u128, u32) {
        match self {
            Self::MapObject {
                generation,
                placement_id,
            } => (0, generation.generation as u128, placement_id.0),
            Self::HeistSafe {
                match_id,
                anchor_id,
                ..
            } => (1, match_id.0, anchor_id.0),
        }
    }

    #[must_use]
    pub const fn placement_id(self) -> MapPlacementId {
        match self {
            Self::MapObject { placement_id, .. } => placement_id,
            Self::HeistSafe { .. } => panic!("Heist safe has no map placement ID"),
        }
    }

    #[must_use]
    pub const fn generation(self) -> MapDynamicGeneration {
        match self {
            Self::MapObject { generation, .. } => generation,
            Self::HeistSafe { .. } => panic!("Heist safe has no map dynamic generation identity"),
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageableTargetClass {
    EnvironmentObject,
    ModeObjective,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageableMaximumHealth(pub u16);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageableObjectProfile(pub MapDamageProfileId);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageableObjectAsset(pub MapAssetId);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageableLifeState {
    Live,
    TerminalCommitted,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageableWorldObject;

/// Semantic capability projected by the owning terminal-reaction plugin. This marker is
/// process-local: authoritative and AI systems use it without exposing asset identity.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HazardousDamageableTarget;

/// Semantic capability for a damageable target whose terminal reaction yields strategic value.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ValuableDamageableTarget;

/// Semantic capability for an attackable mode objective owned by one defending team.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DefendedDamageableObjective {
    pub(crate) defending_team: crate::combat::TeamId,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingWorldTargetDamage {
    pub target: DamageableTargetIdentity,
    pub source: AttackSource,
    pub attack_id: AttackId,
    pub requested_damage: u16,
    pub delivery_index: u8,
    pub bundle_index: u8,
    pub effect_index: u8,
}

#[cfg(feature = "server")]
#[derive(Resource, Default)]
pub struct PendingWorldTargetDamages(pub Vec<PendingWorldTargetDamage>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTargetDamageFact {
    pub event_id: CombatEventId,
    pub tick: u64,
    pub attack_id: AttackId,
    pub source: AttackSource,
    pub target: DamageableTargetIdentity,
    pub requested_damage: u16,
    pub applied_damage: u16,
    pub health_after: u16,
    pub terminal: Option<WorldTargetTerminalFact>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldTargetTerminalFact {
    MapPlacement(MapPlacementOutcome),
    ModeObjectiveDestroyed,
}

#[derive(Resource, Default)]
pub struct WorldTargetDamageFacts(pub Vec<WorldTargetDamageFact>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldObjectExplosionFact {
    pub event_id: CombatEventId,
    pub tick: u64,
    pub source: AttackSource,
    pub target: DamageableTargetIdentity,
    pub position: Vec2,
    pub radius: f32,
    pub damage: u16,
}

#[derive(Resource, Default)]
pub struct WorldObjectExplosionFacts(pub Vec<WorldObjectExplosionFact>);

#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldObjectTelemetry {
    pub primary_requests: u64,
    pub damage_applications: u64,
    pub applied_damage: u64,
    pub terminal_reactions: u64,
    pub chained_object_applications: u64,
    pub secondary_combatant_applications: u64,
    pub stale_or_invalid_rejections: u64,
    pub capacity_rejections: u64,
}

#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum WorldObjectCue {
    Damaged {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source_subject: Option<crate::protocol::NetworkEntityId>,
        target: DamageableTargetIdentity,
        position: WorldPoint,
        amount: u16,
        health_after: u16,
    },
    Exploded {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source_subject: Option<crate::protocol::NetworkEntityId>,
        target: DamageableTargetIdentity,
        position: WorldPoint,
        radius_world_units: u16,
        damage: u16,
    },
}

impl WorldObjectCue {
    #[must_use]
    pub const fn event_id(self) -> CombatEventId {
        match self {
            Self::Damaged { event_id, .. } | Self::Exploded { event_id, .. } => event_id,
        }
    }

    #[must_use]
    pub const fn source_subject(self) -> Option<crate::protocol::NetworkEntityId> {
        match self {
            Self::Damaged { source_subject, .. } | Self::Exploded { source_subject, .. } => {
                source_subject
            }
        }
    }

    #[must_use]
    pub const fn target(self) -> DamageableTargetIdentity {
        match self {
            Self::Damaged { target, .. } | Self::Exploded { target, .. } => target,
        }
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Default)]
pub struct WorldObjectOutbox(pub Vec<WorldObjectCue>);

#[cfg(feature = "client")]
#[derive(Resource, Default)]
pub struct ReceivedWorldObjectCues(pub Vec<WorldObjectCue>);

#[must_use]
pub const fn object_is_live(health: CurrentHealth, life: DamageableLifeState) -> bool {
    health.0 > 0 && matches!(life, DamageableLifeState::Live)
}

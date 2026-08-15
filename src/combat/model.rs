//! Shared combat identities, replicated state, and server-internal delivery messages.

#[cfg(feature = "server")]
use super::{PayloadBundleDefinition, WeaponRecipe};
use super::{
    WeaponDefinitionId, WeaponPresentationProfileId, WeaponPresetId, WeaponRecipeFingerprint,
};
use crate::protocol::{NetworkEntityId, PlayerId};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::collections::BTreeMap;
/// A selected build is a stable choice, not mutable weapon state.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedBuild {
    pub primary_weapon: WeaponDefinitionId,
    pub source_preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
}

/// Stable source identity for a player action. Delivery entities and payloads refer to this
/// identity rather than a process-local ECS entity or a preset-specific behavior class.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Reflect,
)]
pub struct AttackId(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AttackSource {
    pub attack_id: AttackId,
    pub player_id: PlayerId,
    pub owner_network_entity_id: NetworkEntityId,
    pub team_id: TeamId,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
    pub presentation_profile_id: WeaponPresentationProfileId,
    pub legacy_compatibility: bool,
    pub source_preset_id: Option<WeaponPresetId>,
    pub origin: WorldPoint,
    pub facing: f32,
}

/// Replicated attack identity carried by composed delivery entities. The server remains the
/// authority for the private runtime recipe; clients use this bounded identity for presentation
/// and diagnostics only.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ReplicatedAttackSource {
    pub attack: AttackSource,
}

/// Replicated identity installed by the server during the one-time selection transaction.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedWeapon {
    pub source_preset_id: WeaponPresetId,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
}

/// Presence means that an accepted fighter has not crossed the sandbox weapon gate yet.
#[derive(
    Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect,
)]
pub struct SelectingWeapon;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwaitingPostSelectionInput {
    pub accepted_at_tick: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlowEffect {
    pub source_attack_id: AttackId,
    pub source_network_entity_id: NetworkEntityId,
    pub movement_multiplier_milli: u16,
    pub expires_at_tick: u64,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveEffects {
    pub slow: Option<SlowEffect>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ExternalMotion {
    pub velocity: Vec2,
    pub expires_at_tick: u64,
}

/// Replicated feedback state corresponding to server-applied knockback. The pose remains the
/// authoritative movement result; this component only lets clients render and audit the effect.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct KnockbackFeedback {
    pub velocity: WorldPoint,
    pub expires_at_tick: u64,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AttackDelivery {
    pub attack_id: AttackId,
    pub delivery_index: u8,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct LobbedFlight {
    pub launch: WorldPoint,
    pub landing: WorldPoint,
    pub launched_at_tick: u64,
    pub lands_at_tick: u64,
    pub visual_arc_height: f32,
}

/// The server-owned deadline for a live delivery. Keeping it replicated makes late-join and
/// process evidence independent of a client's local simulation clock.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileDeadline {
    pub expires_at_tick: u64,
}

/// Latest server-computed fighter pose carried without client interpolation for evidence and
/// late-join auditing. The ordinary Position/Rotation pair remains the presentation pose.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AuthoritativePose {
    pub position: WorldPoint,
    pub facing: f32,
    pub tick: u64,
}

/// Replicated straight-flight inputs let evidence reconstruct a canonical position from the
/// authoritative tick instead of comparing an interpolated render position.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StraightFlight {
    pub origin: WorldPoint,
    pub facing: f32,
    pub speed: f32,
    pub maximum_range: f32,
    pub launched_at_tick: u64,
}

#[cfg(feature = "server")]
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveAttackTracker {
    pub source: AttackSource,
    pub expected_deliveries: u8,
    pub resolved_deliveries: u8,
    pub had_hostile_contact: bool,
}

#[cfg(feature = "server")]
#[derive(Clone, Debug, PartialEq)]
pub struct CompletedAttack {
    pub source_preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
    pub had_hostile_contact: bool,
}

#[cfg(feature = "server")]
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ActiveAttackTrackers {
    pub active: BTreeMap<AttackId, ActiveAttackTracker>,
    pub completed: Vec<CompletedAttack>,
}

/// Sandbox affiliation used by the direct-hit policy.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct TeamId(pub u8);

/// Integer authoritative health.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct CurrentHealth(pub u16);

/// The weapon phase is replicated together with ammo and its authoritative deadline.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WeaponPhase {
    Ready,
    Cooldown { ready_at_tick: u64 },
    Reloading { ready_at_tick: u64 },
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct WeaponState {
    pub ammo: u8,
    pub phase: WeaponPhase,
}

/// The latest authoritative server tick observed alongside replicated fighter state.
///
/// Clients use this reference for deadline displays instead of comparing a server-owned
/// `WeaponPhase` deadline with a local tick that may have started at a different point after
/// late join or reconnect.
#[derive(
    Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect,
)]
pub struct AuthoritativeTick(pub u64);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct Defeated {
    pub event_id: CombatEventId,
}

/// Stable shot identity, never a Bevy entity identity.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct ShotId(pub u64);

/// Stable ordered combat outcome identity.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct CombatEventId(pub u64);

/// Server-owned projectile marker.
#[derive(
    Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect,
)]
pub struct Projectile;

/// Stable projectile source metadata replicated to every observer.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileSource {
    pub shot_id: ShotId,
    pub player_id: PlayerId,
    pub owner_network_entity_id: NetworkEntityId,
    pub team_id: TeamId,
    pub weapon_definition_id: WeaponDefinitionId,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpawnState {
    pub position: Vec2,
    pub facing: f32,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ComposedProjectileRuntime {
    pub owner_entity: Entity,
    pub source: AttackSource,
    pub delivery_index: u8,
    pub velocity: Vec2,
    pub travelled: f32,
    pub expires_at_tick: u64,
    pub maximum_range: f32,
    pub radius: f32,
    pub landing: Option<Vec2>,
    pub recipe: WeaponRecipe,
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Debug, PartialEq)]
pub struct MeleeAttack {
    pub source: AttackSource,
    pub origin: Vec2,
    pub facing: f32,
    pub tick: u64,
    pub recipe: WeaponRecipe,
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Debug, PartialEq)]
pub struct PendingPayload {
    pub source: AttackSource,
    pub delivery_index: u8,
    pub bundle_index: u8,
    pub target: Entity,
    pub target_network_id: NetworkEntityId,
    pub position: Vec2,
    pub engagement_distance: f32,
    pub delivery_travel: f32,
    pub contact_fraction: f32,
    pub bundle: PayloadBundleDefinition,
}

/// A delivery outcome that has been geometrically resolved but is not committed until the
/// complete delivery-plus-payload event range is reserved. Keeping the delivery entity alive
/// until that commit makes event exhaustion a transaction failure rather than a half-applied hit.
#[cfg(feature = "server")]
#[derive(Message, Clone, Debug, PartialEq)]
pub struct PendingDelivery {
    pub entity: Option<Entity>,
    pub source: AttackSource,
    pub delivery_index: u8,
    pub tick: u64,
    pub engagement_distance: f32,
    pub delivery_travel: f32,
    pub kind: PendingDeliveryKind,
}

#[cfg(feature = "server")]
#[derive(Clone, Debug, PartialEq)]
pub enum PendingDeliveryKind {
    StraightImpact {
        target: Option<NetworkEntityId>,
        position: WorldPoint,
        normal: WorldPoint,
        distance_band: DistanceBand,
    },
    LobLanded {
        position: WorldPoint,
    },
    MeleeContact {
        target: NetworkEntityId,
        position: WorldPoint,
    },
}

/// A finite world-space point used by networked combat cues.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct WorldPoint {
    pub x: f32,
    pub y: f32,
}

impl WorldPoint {
    #[must_use]
    pub fn as_vec2(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

impl From<Vec2> for WorldPoint {
    fn from(value: Vec2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum DistanceBand {
    Close,
    Mid,
    Long,
}

#[must_use]
pub fn distance_band(distance: f32) -> DistanceBand {
    if distance < 250.0 {
        DistanceBand::Close
    } else if distance < 600.0 {
        DistanceBand::Mid
    } else {
        DistanceBand::Long
    }
}
